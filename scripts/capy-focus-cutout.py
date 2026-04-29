#!/usr/bin/env python3
"""Capybara Focus cutout runner.

The Rust CLI owns argument parsing and process supervision. This runner owns the
Python-only model call so the product code keeps a narrow, JSON-shaped boundary.
"""

from __future__ import annotations

import importlib
import json
import os
import sys
import tempfile
import time
from contextlib import redirect_stdout
from pathlib import Path
from typing import Any


FOCUS_REPO = "withoutbg/focus"

# Hugging Face's optional Xet transfer backend can hang for large ONNX downloads
# on some macOS/network setups. The standard HTTP path is slower but predictable
# for this CLI's init flow.
os.environ.setdefault("HF_HUB_DISABLE_XET", "1")


def now_ms() -> int:
    return round(time.perf_counter() * 1000)


def ok_json(value: dict[str, Any]) -> None:
    print(json.dumps(value, indent=2))


def fail(message: str) -> None:
    print(json.dumps({"ok": False, "error": message}, indent=2))
    raise SystemExit(1)


def read_input() -> dict[str, Any]:
    raw = sys.stdin.read()
    if not raw.strip():
        return {}
    return json.loads(raw)


def file_tree_size(path: Path) -> int:
    if not path.exists():
        return 0
    total = 0
    seen: set[tuple[int, int]] = set()
    for root, _, files in os.walk(path):
        for filename in files:
            stat = (Path(root) / filename).stat()
            inode = (stat.st_dev, stat.st_ino)
            if inode in seen:
                continue
            seen.add(inode)
            total += stat.st_size
    return total


def import_status(name: str) -> dict[str, Any]:
    try:
        module = importlib.import_module(name)
    except Exception as error:  # noqa: BLE001 - report exact import failure in JSON
        return {"ok": False, "module": name, "error": str(error)}
    version = getattr(module, "__version__", None)
    return {"ok": True, "module": name, "version": version}


def find_file(root: Path, filename: str) -> Path | None:
    if not root.exists():
        return None
    for candidate in root.rglob(filename):
        if candidate.is_file():
            return candidate
    return None


def model_status(hf_cache_dir: Path, model_files: list[str]) -> list[dict[str, Any]]:
    status = []
    for filename in model_files:
        path = find_file(hf_cache_dir, filename)
        status.append(
            {
                "name": filename,
                "ok": path is not None,
                "path": str(path) if path else None,
                "size_bytes": path.stat().st_size if path else 0,
            }
        )
    return status


def doctor(payload: dict[str, Any]) -> None:
    hf_cache_dir = Path(payload["hf_cache_dir"])
    model_files = list(payload["model_files"])
    imports = [
        import_status("PIL"),
        import_status("numpy"),
        import_status("huggingface_hub"),
        import_status("onnxruntime"),
        import_status("withoutbg"),
    ]
    models = model_status(hf_cache_dir, model_files)
    ok_json(
        {
            "ok": all(item["ok"] for item in imports) and all(item["ok"] for item in models),
            "engine": "withoutbg/focus",
            "kind": "cutout-doctor",
            "python": sys.executable,
            "cache_dir": payload["cache_dir"],
            "hf_cache_dir": str(hf_cache_dir),
            "imports": imports,
            "model_files": models,
            "model_size_mb": round(sum(item["size_bytes"] for item in models) / 1024 / 1024, 1),
        }
    )


def download(payload: dict[str, Any]) -> None:
    from huggingface_hub import hf_hub_download

    hf_cache_dir = Path(payload["hf_cache_dir"])
    hf_cache_dir.mkdir(parents=True, exist_ok=True)
    started = now_ms()
    paths = []
    for filename in payload["model_files"]:
        with redirect_stdout(sys.stderr):
            path = Path(hf_hub_download(FOCUS_REPO, filename=filename, cache_dir=str(hf_cache_dir)))
        paths.append({"name": filename, "path": str(path), "size_bytes": path.stat().st_size})
    ok_json(
        {
            "ok": True,
            "engine": "withoutbg/focus",
            "kind": "cutout-download",
            "download_ms": now_ms() - started,
            "hf_cache_dir": str(hf_cache_dir),
            "model_files": paths,
            "model_size_mb": round(sum(item["size_bytes"] for item in paths) / 1024 / 1024, 1),
            "cache_mb": round(file_tree_size(hf_cache_dir) / 1024 / 1024, 1),
        }
    )


def require_model_paths(payload: dict[str, Any]) -> dict[str, Path]:
    hf_cache_dir = Path(payload["hf_cache_dir"])
    paths: dict[str, Path] = {}
    missing = []
    for filename in payload["model_files"]:
        path = find_file(hf_cache_dir, filename)
        if path is None:
            missing.append(filename)
        else:
            paths[filename] = path
    if missing:
        fail("Focus model files missing; run `capy cutout init` first. Missing: " + ", ".join(missing))
    return paths


def load_model(payload: dict[str, Any]):
    from withoutbg.models import OpenSourceModel

    paths = require_model_paths(payload)
    started = now_ms()
    model = OpenSourceModel(
        depth_model_path=paths["depth_anything_v2_vits_slim.onnx"],
        isnet_model_path=paths["isnet.onnx"],
        matting_model_path=paths["focus_matting_1.0.0.onnx"],
        refiner_model_path=paths["focus_refiner_1.0.0.onnx"],
    )
    return model, now_ms() - started, paths


def alpha_stats(alpha) -> dict[str, Any]:
    import numpy as np

    data = np.array(alpha.convert("L"))
    transparent = int(np.sum(data <= 8))
    opaque = int(np.sum(data >= 247))
    edge = int(data.size - transparent - opaque)
    return {
        "transparent_pixels": transparent,
        "edge_pixels": edge,
        "opaque_pixels": opaque,
        "nontransparent_ratio": round(float(np.sum(data > 8) / data.size), 6),
        "edge_ratio": round(edge / data.size, 6),
        "has_alpha": transparent > 0 or edge > 0,
    }


def scaled_source_for_mask(source_path: Path, mask_max_side: int):
    from PIL import Image

    source = Image.open(source_path).convert("RGBA")
    if mask_max_side <= 0 or max(source.size) <= mask_max_side:
        return source, source_path, None
    scale = mask_max_side / max(source.size)
    resized_size = (max(1, round(source.width * scale)), max(1, round(source.height * scale)))
    resized = source.resize(resized_size, Image.Resampling.LANCZOS)
    temp = tempfile.NamedTemporaryFile(suffix=".png", delete=False)
    temp_path = Path(temp.name)
    temp.close()
    resized.save(temp_path)
    return source, temp_path, temp_path


def apply_alpha_to_source(source, alpha):
    import numpy as np
    from PIL import Image

    alpha = alpha.convert("L").resize(source.size, Image.Resampling.LANCZOS)
    rgba = np.array(source)
    rgba[:, :, 3] = np.array(alpha)
    return Image.fromarray(rgba, "RGBA"), alpha


def write_qa_previews(image, qa_dir: Path) -> list[str]:
    from PIL import Image

    qa_dir.mkdir(parents=True, exist_ok=True)
    backgrounds = [
        ("qa-black.png", (0, 0, 0, 255)),
        ("qa-white.png", (255, 255, 255, 255)),
        ("qa-deep.png", (3, 11, 31, 255)),
    ]
    paths = []
    for filename, color in backgrounds:
        background = Image.new("RGBA", image.size, color)
        background.alpha_composite(image)
        path = qa_dir / filename
        background.convert("RGB").save(path)
        paths.append(str(path))
    return paths


def write_json(path: Path | None, payload: dict[str, Any]) -> None:
    if path is None:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n")


def cut_with_loaded_model(model, payload: dict[str, Any], load_ms: int = 0) -> dict[str, Any]:
    from PIL import Image

    source_path = Path(payload["input"])
    output_path = Path(payload["output"])
    mask_path = Path(payload["mask_out"]) if payload.get("mask_out") else None
    qa_dir = Path(payload["qa_dir"]) if payload.get("qa_dir") else None
    report_path = Path(payload["report"]) if payload.get("report") else None
    mask_max_side = int(payload.get("mask_max_side") or 0)

    source, inference_path, temp_path = scaled_source_for_mask(source_path, mask_max_side)
    inference_width, inference_height = Image.open(inference_path).size
    try:
        started = now_ms()
        result = model.remove_background(inference_path)
        inference_ms = now_ms() - started
    finally:
        if temp_path is not None:
            temp_path.unlink(missing_ok=True)

    alpha = result.getchannel("A")
    cutout, final_alpha = apply_alpha_to_source(source, alpha)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    cutout.save(output_path)
    if mask_path is not None:
        mask_path.parent.mkdir(parents=True, exist_ok=True)
        final_alpha.save(mask_path)
    qa = write_qa_previews(cutout, qa_dir) if qa_dir else []
    source_width, source_height = source.size
    pixels = source_width * source_height
    report = {
        "ok": True,
        "engine": "withoutbg/focus",
        "kind": "cutout-run",
        "input": str(source_path),
        "output": str(output_path),
        "mask": str(mask_path) if mask_path else None,
        "width": source_width,
        "height": source_height,
        "mask_inference_width": inference_width,
        "mask_inference_height": inference_height,
        "alpha_strategy": "source RGB + withoutbg/focus alpha mask",
        "performance": {
            "load_ms": load_ms,
            "inference_ms": inference_ms,
            "megapixels_per_second": round((pixels / 1_000_000) / max(inference_ms / 1000, 0.001), 3),
        },
        "alpha": alpha_stats(final_alpha),
        "qa": qa,
    }
    write_json(report_path, report)
    return report


def cut(payload: dict[str, Any]) -> None:
    with redirect_stdout(sys.stderr):
        model, load_ms, paths = load_model(payload)
        report = cut_with_loaded_model(model, payload, load_ms)
    report["model"] = model_payload(paths, payload)
    ok_json(report)


def resolve_manifest_path(manifest_dir: Path, value: str) -> Path:
    path = Path(value)
    if path.is_absolute():
        return path
    candidate = manifest_dir / path
    if candidate.exists():
        return candidate
    return path


def batch(payload: dict[str, Any]) -> None:
    manifest_path = Path(payload["manifest"])
    manifest = json.loads(manifest_path.read_text())
    manifest_dir = manifest_path.parent
    out_dir = Path(payload["out_dir"])
    report_path = Path(payload["report"]) if payload.get("report") else None
    with redirect_stdout(sys.stderr):
        model, load_ms, paths = load_model(payload)
    started = now_ms()
    reports = []
    for item in manifest["items"]:
        key = item["key"]
        qa_dir = out_dir / "qa" / key
        item_payload = {
            **payload,
            "input": str(resolve_manifest_path(manifest_dir, item["source"])),
            "output": str(out_dir / "outputs" / f"{key}-focus.png"),
            "mask_out": str(out_dir / "masks" / f"{key}-mask.png"),
            "qa_dir": str(qa_dir),
            "report": str(out_dir / "reports" / f"{key}.json"),
        }
        with redirect_stdout(sys.stderr):
            report = cut_with_loaded_model(model, item_payload, load_ms if not reports else 0)
        report["key"] = key
        report["label"] = item.get("label", key)
        report["risk"] = item.get("risk")
        reports.append(report)
    summary = {
        "ok": True,
        "engine": "withoutbg/focus",
        "kind": "cutout-batch",
        "manifest": str(manifest_path),
        "out_dir": str(out_dir),
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "performance": {"total_ms": now_ms() - started},
        "model": model_payload(paths, payload),
        "reports": reports,
    }
    write_json(report_path, summary)
    ok_json(summary)


def model_payload(paths: dict[str, Path], payload: dict[str, Any]) -> dict[str, Any]:
    files = [{"name": name, "path": str(path), "size_bytes": path.stat().st_size} for name, path in paths.items()]
    return {
        "repo": FOCUS_REPO,
        "license": "Apache-2.0",
        "hf_cache_dir": payload["hf_cache_dir"],
        "files": files,
        "model_size_mb": round(sum(item["size_bytes"] for item in files) / 1024 / 1024, 1),
    }


def main() -> None:
    if len(sys.argv) != 2:
        fail("usage: capy-focus-cutout.py <doctor|download|cut|batch>")
    operation = sys.argv[1]
    payload = read_input()
    if operation == "doctor":
        doctor(payload)
    elif operation == "download":
        download(payload)
    elif operation == "cut":
        cut(payload)
    elif operation == "batch":
        batch(payload)
    else:
        fail(f"unknown operation: {operation}")


if __name__ == "__main__":
    main()
