#!/usr/bin/env python3
"""Capybara clips wrapper around the shared WhisperX forced-alignment helper."""

from __future__ import annotations

import runpy
import sys
from pathlib import Path


def main() -> int:
    repo_root = Path(__file__).resolve().parents[3]
    helper = repo_root / "crates" / "capy-tts" / "scripts" / "align_ffa.py"
    if not helper.is_file():
        print(f"shared align helper not found: {helper}", file=sys.stderr)
        return 2
    sys.path.insert(0, str(helper.parent))
    sys.argv[0] = str(helper)
    runpy.run_path(str(helper), run_name="__main__")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
