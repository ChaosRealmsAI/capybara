#!/usr/bin/env python3
"""WhisperX transcription helper for `capy clips transcribe`.

Usage:
    python whisper_transcribe.py <audio-path> <model> <language>

Stdout is reserved for one JSON object:
    {"language":"en","words":[{"text":"Hello","start":0.0,"end":0.4}]}
"""

from __future__ import annotations

import contextlib
import json
import os
import re
import sys
from pathlib import Path
from typing import Any


def eprint(message: str) -> None:
    print(message, file=sys.stderr)


def load_whisperx() -> Any:
    with contextlib.redirect_stdout(sys.stderr):
        import whisperx  # type: ignore

    return whisperx


def language_arg(value: str) -> str | None:
    value = value.strip()
    if not value or value.lower() == "auto":
        return None
    return value


def load_asr_model(whisperx: Any, model_name: str, language: str | None) -> Any:
    device = os.environ.get("CAPY_CLIPS_WHISPER_DEVICE", "cpu")
    compute_type = os.environ.get("CAPY_CLIPS_WHISPER_COMPUTE", "int8")
    vad_method = os.environ.get("CAPY_CLIPS_VAD_METHOD", "silero")
    threads = int(os.environ.get("CAPY_CLIPS_WHISPER_THREADS", "4"))
    kwargs = {
        "device": device,
        "compute_type": compute_type,
        "language": language,
        "vad_method": vad_method,
        "threads": threads,
    }
    download_root = os.environ.get("CAPY_CLIPS_WHISPER_CACHE")
    if download_root:
        kwargs["download_root"] = download_root

    with contextlib.redirect_stdout(sys.stderr):
        try:
            return whisperx.load_model(model_name, **kwargs)
        except TypeError:
            kwargs.pop("vad_method", None)
            return whisperx.load_model(model_name, **kwargs)


def transcribe(model: Any, audio: Any, language: str | None) -> dict[str, Any]:
    kwargs: dict[str, Any] = {"batch_size": int(os.environ.get("CAPY_CLIPS_WHISPER_BATCH", "8"))}
    if language:
        kwargs["language"] = language
    with contextlib.redirect_stdout(sys.stderr):
        try:
            return model.transcribe(audio, **kwargs)
        except TypeError:
            kwargs.pop("language", None)
            return model.transcribe(audio, **kwargs)


def align_words(
    whisperx: Any,
    segments: list[dict[str, Any]],
    audio: Any,
    language: str,
) -> list[dict[str, Any]]:
    device = os.environ.get("CAPY_CLIPS_WHISPER_DEVICE", "cpu")
    with contextlib.redirect_stdout(sys.stderr):
        align_model, metadata = whisperx.load_align_model(language_code=language, device=device)
        aligned = whisperx.align(
            segments,
            align_model,
            metadata,
            audio,
            device,
            return_char_alignments=False,
            print_progress=False,
        )

    words = aligned.get("word_segments")
    if words:
        return words

    flattened: list[dict[str, Any]] = []
    for segment in aligned.get("segments", []):
        flattened.extend(segment.get("words", []))
    return flattened


def normalize_words(raw_words: list[dict[str, Any]]) -> list[dict[str, Any]]:
    words: list[dict[str, Any]] = []
    for raw in raw_words:
        text = str(raw.get("word") or raw.get("text") or "").strip()
        start = raw.get("start")
        end = raw.get("end")
        if not text or start is None or end is None:
            continue
        try:
            start_s = round(float(start), 3)
            end_s = round(float(end), 3)
        except (TypeError, ValueError):
            continue
        if end_s < start_s:
            continue
        words.append({"text": text, "start": start_s, "end": end_s})
    return words


def estimate_words(segments: list[dict[str, Any]]) -> list[dict[str, Any]]:
    estimated: list[dict[str, Any]] = []
    for segment in segments:
        start = float(segment.get("start") or 0.0)
        end = float(segment.get("end") or start)
        text = str(segment.get("text") or "").strip()
        parts = re.findall(r"\S+", text)
        if not parts:
            continue
        step = max(0.001, (end - start) / len(parts))
        for index, part in enumerate(parts):
            estimated.append(
                {
                    "text": part,
                    "start": round(start + index * step, 3),
                    "end": round(start + (index + 1) * step, 3),
                }
            )
    return estimated


def main(argv: list[str]) -> int:
    if len(argv) != 4:
        eprint(__doc__.strip())
        return 2

    audio_path = Path(argv[1]).expanduser()
    model_name = argv[2]
    requested_language = language_arg(argv[3])
    if not audio_path.is_file():
        eprint(f"audio file not found: {audio_path}")
        return 2

    whisperx = load_whisperx()
    with contextlib.redirect_stdout(sys.stderr):
        audio = whisperx.load_audio(str(audio_path))

    model = load_asr_model(whisperx, model_name, requested_language)
    result = transcribe(model, audio, requested_language)
    detected_language = result.get("language") or requested_language or "auto"
    segments = result.get("segments") or []
    if not segments:
        eprint("whisperx returned zero transcription segments")
        return 1

    try:
        aligned_words = align_words(whisperx, segments, audio, str(detected_language))
        words = normalize_words(aligned_words)
    except Exception as exc:  # noqa: BLE001 - helper must report external model failures cleanly.
        if os.environ.get("CAPY_CLIPS_ALLOW_ESTIMATED_WORDS") != "1":
            eprint(f"whisperx alignment failed: {exc}")
            return 1
        eprint(f"whisperx alignment failed; using estimated word timing: {exc}")
        words = estimate_words(segments)

    if not words:
        eprint("whisperx returned zero word timestamps")
        return 1

    json.dump({"language": detected_language, "words": words}, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
