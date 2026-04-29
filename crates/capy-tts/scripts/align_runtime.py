#!/usr/bin/env python3
"""Runtime checks and downloads for align_ffa.py."""
from __future__ import annotations

import os
import sys
from typing import Any, Callable

EmitJson = Callable[[dict[str, Any]], None]

# Canonical wav2vec2 CTC model id per language code. For languages that
# whisperX ships as "torch" (torchaudio.pipelines) defaults we pin an HF
# alternative instead so every language lives in ~/.cache/huggingface and
# the offline-detection path is uniform.
DEFAULT_ALIGN_MODELS = {
    "zh": "jonatasgrosman/wav2vec2-large-xlsr-53-chinese-zh-cn",
    "ja": "jonatasgrosman/wav2vec2-large-xlsr-53-japanese",
    "ko": "kresnik/wav2vec2-large-xlsr-korean",
    "en": "jonatasgrosman/wav2vec2-large-xlsr-53-english",
    "fr": "jonatasgrosman/wav2vec2-large-xlsr-53-french",
    "de": "jonatasgrosman/wav2vec2-large-xlsr-53-german",
    "es": "jonatasgrosman/wav2vec2-large-xlsr-53-spanish",
    "it": "jonatasgrosman/wav2vec2-large-xlsr-53-italian",
}

RUNTIME_PACKAGES = [
    "whisperx",
    "huggingface_hub",
    "torch",
    "torchaudio",
    "transformers",
]


def cached_align_model_dir(language: str) -> str | None:
    """Return a fully cached HF snapshot dir for the language model."""
    repo = DEFAULT_ALIGN_MODELS.get(language)
    if not repo:
        return None
    cache_root = os.environ.get("HF_HOME") or os.path.join(
        os.path.expanduser("~"), ".cache", "huggingface"
    )
    slug = "models--" + repo.replace("/", "--")
    snapshots_dir = os.path.join(cache_root, "hub", slug, "snapshots")
    if not os.path.isdir(snapshots_dir):
        return None
    for entry in os.listdir(snapshots_dir):
        snap = os.path.join(snapshots_dir, entry)
        if not os.path.isdir(snap):
            continue
        has_cfg = os.path.exists(os.path.join(snap, "config.json"))
        has_weights = any(
            os.path.exists(os.path.join(snap, f))
            for f in ("pytorch_model.bin", "model.safetensors")
        )
        if has_cfg and has_weights:
            return snap
    return None


def _package_status() -> list[dict[str, Any]]:
    try:
        from importlib import metadata
    except Exception as e:
        return [
            {
                "name": name,
                "available": False,
                "version": None,
                "error": f"importlib.metadata unavailable: {e}",
            }
            for name in RUNTIME_PACKAGES
        ]

    packages: list[dict[str, Any]] = []
    for name in RUNTIME_PACKAGES:
        try:
            version = metadata.version(name)
            packages.append({"name": name, "available": True, "version": version})
        except metadata.PackageNotFoundError:
            packages.append({"name": name, "available": False, "version": None})
        except Exception as e:
            packages.append(
                {
                    "name": name,
                    "available": False,
                    "version": None,
                    "error": str(e),
                }
            )
    return packages


def _normalize_languages(values: list[str]) -> list[str]:
    languages = [value.strip().lower() for value in values if value.strip()]
    if not languages:
        languages = ["zh"]
    unknown = [lang for lang in languages if lang not in DEFAULT_ALIGN_MODELS]
    if unknown:
        raise ValueError(
            "unsupported language(s): "
            + ", ".join(unknown)
            + ". supported: "
            + ", ".join(sorted(DEFAULT_ALIGN_MODELS))
        )
    return sorted(set(languages))


def _model_status(languages: list[str]) -> list[dict[str, Any]]:
    models: list[dict[str, Any]] = []
    for language in languages:
        repo = DEFAULT_ALIGN_MODELS[language]
        snapshot = cached_align_model_dir(language)
        models.append(
            {
                "language": language,
                "repo": repo,
                "cached": snapshot is not None,
                "snapshot": snapshot,
            }
        )
    return models


def _runtime_status(languages: list[str]) -> dict[str, Any]:
    packages = _package_status()
    models = _model_status(languages)
    return {
        "ok": all(package["available"] for package in packages)
        and all(model["cached"] for model in models),
        "kind": "tts-align-status",
        "python": sys.executable,
        "hf_home": os.environ.get("HF_HOME")
        or os.path.join(os.path.expanduser("~"), ".cache", "huggingface"),
        "languages": languages,
        "packages": packages,
        "models": models,
    }


def runtime_status_command(args: list[str], emit_json: EmitJson) -> int:
    try:
        languages = _normalize_languages(args)
    except ValueError as e:
        emit_json({"ok": False, "kind": "tts-align-status", "error": str(e)})
        return 2
    emit_json(_runtime_status(languages))
    return 0


def runtime_download_command(args: list[str], emit_json: EmitJson) -> int:
    try:
        languages = _normalize_languages(args)
    except ValueError as e:
        emit_json({"ok": False, "kind": "tts-align-download", "error": str(e)})
        return 2

    try:
        from huggingface_hub import snapshot_download
    except Exception as e:
        emit_json(
            {
                "ok": False,
                "kind": "tts-align-download",
                "error": f"huggingface_hub unavailable: {e}",
                "languages": languages,
            }
        )
        return 1

    results: list[dict[str, Any]] = []
    ok = True
    for language in languages:
        repo = DEFAULT_ALIGN_MODELS[language]
        before = cached_align_model_dir(language)
        if before:
            results.append(
                {
                    "language": language,
                    "repo": repo,
                    "cached_before": True,
                    "cached_after": True,
                    "snapshot": before,
                    "downloaded": False,
                }
            )
            continue
        try:
            snapshot = snapshot_download(repo_id=repo)
            after = cached_align_model_dir(language) or snapshot
            results.append(
                {
                    "language": language,
                    "repo": repo,
                    "cached_before": False,
                    "cached_after": after is not None,
                    "snapshot": after,
                    "downloaded": True,
                }
            )
            ok = ok and after is not None
        except Exception as e:
            ok = False
            results.append(
                {
                    "language": language,
                    "repo": repo,
                    "cached_before": False,
                    "cached_after": False,
                    "downloaded": False,
                    "error": str(e),
                }
            )

    emit_json(
        {
            "ok": ok,
            "kind": "tts-align-download",
            "hf_home": os.environ.get("HF_HOME")
            or os.path.join(os.path.expanduser("~"), ".cache", "huggingface"),
            "languages": languages,
            "models": results,
            "status": _runtime_status(languages),
        }
    )
    return 0 if ok else 1
