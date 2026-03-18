#!/usr/bin/env python3
"""
Summarise Android telemetry redaction health for AND7 drills.

The script consumes a JSON status payload (either from an HTTP endpoint or the
local cache in ``artifacts/android/telemetry/status.json``) and emits the
human-readable lines referenced by the runbook, chaos checklist, and lab
reports. Exit codes follow the usual monitoring convention:

* ``0`` — all counters healthy
* ``1`` — exporter/backlog warning
* ``2`` — redaction mismatches detected
* ``3`` — source unavailable or payload invalid

Usage examples::

    # Inspect the default status cache (created by the injection helper)
    scripts/telemetry/check_redaction_status.py

    # Fetch from a staging endpoint and update the local cache
    scripts/telemetry/check_redaction_status.py \
        --status-url https://android-telemetry-stg/api/redaction/status \
        --write-cache

    # Emit machine-readable JSON only (suppresses STATUS/ALERT lines)
    scripts/telemetry/check_redaction_status.py --json > status.json

    # Keep the text summary and persist JSON artefacts alongside it
    scripts/telemetry/check_redaction_status.py \
        --status-url https://android-telemetry-stg/api/redaction/status \
        --json-out artifacts/android/telemetry/status.json
"""

from __future__ import annotations

import argparse
import io
import json
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, TextIO

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in os.sys.path:
    os.sys.path.insert(0, str(SCRIPT_DIR))

from _redaction_status import (  # pylint: disable=import-error
    DEFAULT_STATUS_PATH,
    fetch_status_from_url,
    normalise_status,
    now_iso,
    read_status_file,
    write_status_file,
)


@dataclass
class Summary:
    timestamp: str
    hashed: int
    mismatched: int
    salt_epoch: Optional[str]
    salt_rotation: Optional[str]
    failures: List[Dict[str, Any]]
    exporters: List[Dict[str, Any]]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Summarise Android telemetry redaction health."
    )
    parser.add_argument(
        "--status-json",
        help="Path to a cached status JSON file "
        "(defaults to ANDROID_TELEMETRY_STATUS_JSON or "
        f"{DEFAULT_STATUS_PATH})",
    )
    parser.add_argument(
        "--status-url",
        help="HTTP(S) endpoint returning the status payload. "
        "Set ANDROID_TELEMETRY_STATUS_URL to avoid passing this every time.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=5.0,
        help="HTTP timeout (seconds) when using --status-url (default: 5s).",
    )
    parser.add_argument(
        "--write-cache",
        action="store_true",
        help="Persist the fetched payload to the local cache (handy when "
        "polling a staging endpoint).",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit the summary as JSON to stdout only "
        "(suppresses STATUS/ALERT lines).",
    )
    parser.add_argument(
        "--json-out",
        help="Write the summary JSON to PATH while keeping the human-readable "
        "output. Pass '-' to mirror --json without suppressing the text.",
    )
    parser.add_argument(
        "--max-failures",
        type=int,
        default=5,
        help="Show at most N recent failures (default: 5).",
    )
    parser.add_argument(
        "--warn-backlog",
        type=float,
        default=0.0,
        help="Warn when an exporter backlog exceeds this value (default: 0).",
    )
    parser.add_argument(
        "--expected-salt-epoch",
        help="Expected salt epoch (e.g., 2026Q1). "
        "Falls back to ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH if unset.",
    )
    parser.add_argument(
        "--expected-salt-rotation",
        help="Expected salt rotation identifier. "
        "Falls back to ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION if unset.",
    )
    parser.add_argument(
        "--min-hashed",
        type=int,
        help="Alert when hashed authorities drop below this value. "
        "Falls back to ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES if unset.",
    )
    return parser.parse_args()


def load_payload(args: argparse.Namespace) -> Dict[str, Any]:
    status_url = args.status_url or os.environ.get("ANDROID_TELEMETRY_STATUS_URL")
    status_path = Path(
        args.status_json
        or os.environ.get("ANDROID_TELEMETRY_STATUS_JSON", DEFAULT_STATUS_PATH)
    )
    if status_url:
        payload = fetch_status_from_url(status_url, timeout=args.timeout)
        if args.write_cache:
            write_status_file(status_path, payload)
        return payload
    try:
        return read_status_file(status_path, create=False)
    except FileNotFoundError as exc:
        raise SystemExit(
            f"[error] status file {status_path} not found. "
            "Provide --status-url or run the injection helper to seed the cache."
        ) from exc


def build_summary(raw: Dict[str, Any]) -> Summary:
    payload = normalise_status(raw)
    salt = payload.get("salt") or {}
    failures = payload.get("recent_failures") or []
    exporters = payload.get("exporters") or []
    return Summary(
        timestamp=payload.get("timestamp", now_iso()),
        hashed=int(payload.get("hashed_authorities", 0) or 0),
        mismatched=int(payload.get("mismatched_authorities", 0) or 0),
        salt_epoch=str(salt.get("epoch")) if salt.get("epoch") is not None else None,
        salt_rotation=salt.get("rotation_id"),
        failures=failures,
        exporters=exporters,
    )


def summary_to_payload(summary: Summary) -> Dict[str, Any]:
    """Convert a ``Summary`` into the JSON structure written for artefacts."""

    return {
        "timestamp": summary.timestamp,
        "hashed_authorities": summary.hashed,
        "mismatched_authorities": summary.mismatched,
        "salt": {
            "epoch": summary.salt_epoch,
            "rotation_id": summary.salt_rotation,
        },
        "recent_failures": summary.failures,
        "exporters": summary.exporters,
    }


def write_json_to_path(path: Path, payload: Dict[str, Any]) -> None:
    """Write the payload to ``path`` (creating parent directories as needed)."""

    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2)
        handle.write("\n")


def print_summary(
    summary: Summary,
    *,
    max_failures: int,
    warn_backlog: float,
    expected_salt_epoch: Optional[str] = None,
    expected_salt_rotation: Optional[str] = None,
    min_hashed: Optional[int] = None,
    output: Optional[TextIO] = None,
) -> int:
    output = output or os.sys.stdout
    exit_code = 0
    salt_bits = []
    if summary.salt_epoch:
        salt_bits.append(f"salt_epoch={summary.salt_epoch}")
    if summary.salt_rotation:
        salt_bits.append(f"salt_rotation={summary.salt_rotation}")
    salt_suffix = "  " + "  ".join(salt_bits) if salt_bits else ""
    print(
        f"STATUS  timestamp={summary.timestamp}  "
        f"hashed_authorities={summary.hashed}  mismatched={summary.mismatched}"
        f"{salt_suffix}",
        file=output,
    )

    if summary.failures:
        limit = max_failures if max_failures >= 0 else None
        failures = (
            summary.failures if limit is None else summary.failures[:limit]
        )
        for entry in failures:
            ts = entry.get("timestamp")
            authority = entry.get("authority", "unknown")
            reason = entry.get("reason", "unknown")
            count = int(entry.get("count", 1) or 1)
            note = entry.get("note")
            parts = [
                f"FAILURE authority={authority}",
                f"reason={reason}",
                f"count={count}",
            ]
            if ts:
                parts.append(f"reported_at={ts}")
            if note:
                parts.append(f"note={note}")
            print("  ".join(parts), file=output)

    for exporter in summary.exporters:
        backend = exporter.get("backend", "unknown")
        status = (exporter.get("status") or "unknown").lower()
        backlog = float(exporter.get("backlog", 0) or 0)
        print(
            f"EXPORT  backend={backend}  status={status}  backlog={backlog:.2f}",
            file=output,
        )
        if status not in {"ok", "healthy"} or backlog > warn_backlog:
            exit_code = max(exit_code, 1)

    if min_hashed is not None and summary.hashed < min_hashed:
        exit_code = max(exit_code, 2)
        print(
            "ALERT   android.telemetry.redaction.hashed_authorities "
            f"threshold={min_hashed} observed={summary.hashed}",
            file=output,
        )

    if summary.mismatched > 0:
        exit_code = 2
        reason = summary.failures[0]["reason"] if summary.failures else "unknown"
        print(
            "ALERT   android.telemetry.redaction.failure_total"
            f'{{reason="{reason}"}} incremented (delta={summary.mismatched})',
            file=output,
        )

    def _salt_alert(kind: str, expected: str, observed: Optional[str]) -> None:
        nonlocal exit_code
        exit_code = max(exit_code, 2)
        observed_text = observed or "unknown"
        print(
            "ALERT   android.telemetry.redaction.salt_version "
            f"{kind}_expected={expected} {kind}_observed={observed_text}",
            file=output,
        )

    if expected_salt_epoch:
        if summary.salt_epoch != expected_salt_epoch:
            _salt_alert("salt_epoch", expected_salt_epoch, summary.salt_epoch)
    if expected_salt_rotation:
        if summary.salt_rotation != expected_salt_rotation:
            _salt_alert(
                "salt_rotation", expected_salt_rotation, summary.salt_rotation
            )
    return exit_code


def main() -> None:
    args = parse_args()
    try:
        payload = load_payload(args)
    except SystemExit:
        raise
    except Exception as exc:  # pragma: no cover - defensive guard
        raise SystemExit(f"[error] failed to load status payload: {exc}") from exc

    summary = build_summary(payload)
    expected_salt_epoch = args.expected_salt_epoch or os.environ.get(
        "ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH"
    )
    expected_salt_rotation = args.expected_salt_rotation or os.environ.get(
        "ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION"
    )
    min_hashed: Optional[int] = args.min_hashed
    if min_hashed is None:
        env_min = os.environ.get("ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES")
        if env_min:
            try:
                min_hashed = int(env_min)
            except ValueError as exc:  # pragma: no cover - defensive guard
                raise SystemExit(
                    "[error] ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES must be an integer"
                ) from exc
    suppress_text = args.json and not args.json_out
    text_output: TextIO = io.StringIO() if suppress_text else os.sys.stdout

    exit_code = print_summary(
        summary,
        max_failures=args.max_failures,
        warn_backlog=args.warn_backlog,
        expected_salt_epoch=expected_salt_epoch,
        expected_salt_rotation=expected_salt_rotation,
        min_hashed=min_hashed,
        output=text_output,
    )

    payload = summary_to_payload(summary)

    wrote_stdout = False
    if args.json:
        json.dump(payload, os.sys.stdout, indent=2)
        os.sys.stdout.write("\n")
        wrote_stdout = True

    if args.json_out:
        if args.json_out == "-":
            if not wrote_stdout:
                json.dump(payload, os.sys.stdout, indent=2)
                os.sys.stdout.write("\n")
        else:
            json_out_path = Path(args.json_out)
            write_json_to_path(json_out_path, payload)

    raise SystemExit(exit_code)


if __name__ == "__main__":
    main()
