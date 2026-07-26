#!/usr/bin/env python3
"""Build Android parity pipeline metadata for dashboards and release gates."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Iterable, List, Sequence, Tuple


def _parse_duration(value: str, label: str) -> float:
    try:
        parsed = float(value)
    except ValueError as exc:  # pragma: no cover - surfaced through argparse
        raise argparse.ArgumentTypeError(f"{label} must be a number") from exc
    if parsed < 0:
        raise argparse.ArgumentTypeError(f"{label} must be non-negative")
    return parsed


def _parse_test(values: Sequence[str]) -> Tuple[str, float]:
    if len(values) != 2:
        raise argparse.ArgumentTypeError("each --test requires NAME and DURATION_SECONDS")
    name, duration = values
    if not name:
        raise argparse.ArgumentTypeError("test name cannot be empty")
    return name, _parse_duration(duration, "test duration")


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Emit Android parity pipeline metadata JSON.")
    parser.add_argument("--job-name", required=True, help="Logical job name (e.g., android-parity).")
    parser.add_argument(
        "--duration-seconds",
        required=True,
        type=lambda value: _parse_duration(value, "job duration"),
        help="Total job duration in seconds.",
    )
    parser.add_argument("--buildkite-url", help="Optional Buildkite URL for the job/run.")
    parser.add_argument(
        "--test",
        action="append",
        nargs=2,
        metavar=("NAME", "DURATION_SECONDS"),
        default=[],
        help="Record an individual test step and its duration (may be repeated).",
    )
    parser.add_argument(
        "--note",
        action="append",
        default=[],
        help="Optional note to include in the metadata (may be repeated).",
    )
    parser.add_argument(
        "--output",
        required=True,
        type=Path,
        help="Destination path for the pipeline metadata JSON.",
    )
    return parser.parse_args(list(argv) if argv is not None else None)


def build_payload(
    *,
    job_name: str,
    duration_seconds: float,
    buildkite_url: str | None,
    tests: List[Tuple[str, float]],
    notes: List[str],
) -> dict:
    payload: dict = {
        "job_name": job_name,
        "duration_seconds": duration_seconds,
    }
    if tests:
        payload["tests"] = [
            {"name": name, "duration_seconds": duration} for name, duration in tests
        ]
    if buildkite_url:
        payload["buildkite_url"] = buildkite_url
    if notes:
        payload["notes"] = " ".join(note.strip() for note in notes if note.strip())
    return payload


def main(argv: Iterable[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as exc:  # argparse exits on validation errors
        return int(exc.code)

    tests: List[Tuple[str, float]] = []
    for raw in args.test:
        tests.append(_parse_test(raw))

    payload = build_payload(
        job_name=args.job_name,
        duration_seconds=args.duration_seconds,
        buildkite_url=args.buildkite_url,
        tests=tests,
        notes=args.note,
    )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    raise SystemExit(main())
