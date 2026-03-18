#!/usr/bin/env python3
"""
Validate Swift pipeline metadata feeds.

Usage:
    scripts/check_swift_pipeline_metadata.py dashboards/data/mobile_pipeline_metadata.sample.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Dict, Iterable, List, Sequence


def _load(path: Path) -> Dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except FileNotFoundError as exc:  # pragma: no cover - surfaced to caller
        raise ValueError(f"unable to open {path}") from exc
    except json.JSONDecodeError as exc:  # pragma: no cover - surfaced to caller
        raise ValueError(f"{path}: invalid JSON ({exc})") from exc


def _validate_tests(tests: Any, *, path: str) -> None:
    if tests is None:
        return
    if not isinstance(tests, list):
        raise ValueError(f"{path} must be an array when present")
    for idx, entry in enumerate(tests):
        entry_path = f"{path}[{idx}]"
        if not isinstance(entry, dict):
            raise ValueError(f"{entry_path} must be an object")
        name = entry.get("name")
        if not isinstance(name, str) or not name.strip():
            raise ValueError(f"{entry_path}.name must be a non-empty string")
        duration = entry.get("duration_seconds")
        if not isinstance(duration, (int, float)):
            raise ValueError(f"{entry_path}.duration_seconds must be numeric")
        if duration < 0:
            raise ValueError(f"{entry_path}.duration_seconds must be non-negative")


def validate_metadata(payload: Dict[str, Any], path: str) -> None:
    if not isinstance(payload, dict):
        raise ValueError(f"{path}: payload must be a JSON object")

    job = payload.get("job_name")
    if not isinstance(job, str) or not job.strip():
        raise ValueError(f"{path}.job_name must be a non-empty string")

    duration = payload.get("duration_seconds")
    if not isinstance(duration, (int, float)):
        raise ValueError(f"{path}.duration_seconds must be numeric")
    if duration < 0:
        raise ValueError(f"{path}.duration_seconds must be non-negative")

    _validate_tests(payload.get("tests"), path=f"{path}.tests")

    buildkite_url = payload.get("buildkite_url")
    if buildkite_url is not None and not isinstance(buildkite_url, str):
        raise ValueError(f"{path}.buildkite_url must be a string when present")

    notes = payload.get("notes")
    if notes is not None and not isinstance(notes, str):
        raise ValueError(f"{path}.notes must be a string when present")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate Swift pipeline metadata feeds.",
    )
    parser.add_argument("files", nargs="+", type=Path, help="JSON files to validate")
    args = parser.parse_args(argv)

    has_error = False
    for file_path in args.files:
        try:
            payload = _load(file_path)
            validate_metadata(payload, str(file_path))
            print(f"[ok] {file_path}")
        except Exception as exc:  # pylint: disable=broad-except
            has_error = True
            print(f"[error] {file_path}: {exc}", file=sys.stderr)
    return 1 if has_error else 0


if __name__ == "__main__":
    sys.exit(main())
