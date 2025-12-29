#!/usr/bin/env python3
"""Collect Swift telemetry redaction status into the mobile_parity telemetry block format."""

from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional


def _now() -> datetime:
    return datetime.now(timezone.utc)


def _parse_iso(value: str) -> datetime:
    if value.endswith("Z"):
        value = value[:-1] + "+00:00"
    return datetime.fromisoformat(value).astimezone(timezone.utc)


def _load_json(path: Path) -> Dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise RuntimeError(f"missing JSON file: {path}") from exc
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"invalid JSON in {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise RuntimeError(f"JSON root must be an object: {path}")
    return data


def _load_notes(path: Path) -> List[str]:
    return [line.strip() for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def _count_overrides(path: Path, now: datetime) -> int:
    if not path.exists():
        return 0
    try:
        entries = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"invalid override JSON in {path}: {exc}") from exc
    if not isinstance(entries, list):
        return 0
    count = 0
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        revoked = entry.get("revoked_at")
        if revoked:
            continue
        expires_at = entry.get("expires_at")
        if not expires_at:
            continue
        try:
            expires = _parse_iso(str(expires_at))
        except ValueError:
            continue
        if expires > now:
            count += 1
    return count


def collect_status(
    *,
    salt_config: Optional[Path],
    profile_alignment: Optional[str],
    overrides_store: Optional[Path],
    schema_version: Optional[str],
    notes_file: Optional[Path],
    extra_notes: List[str],
    schema_diff_report: Optional[Path],
    now: Optional[datetime] = None,
) -> Dict[str, Any]:
    now = now or _now()
    telemetry: Dict[str, Any] = {}

    if salt_config:
        salt_data = _load_json(salt_config)
        salt_epoch = salt_data.get("salt_epoch")
        last_rotation = salt_data.get("last_rotation")
        if salt_epoch:
            telemetry["salt_epoch"] = str(salt_epoch)
        if last_rotation:
            try:
                rotation_time = _parse_iso(str(last_rotation))
                delta_hours = max(0.0, (now - rotation_time).total_seconds() / 3600.0)
                telemetry["salt_rotation_age_hours"] = round(delta_hours, 2)
            except ValueError:
                pass

    if overrides_store:
        telemetry["overrides_open"] = _count_overrides(overrides_store, now)

    if profile_alignment:
        telemetry["device_profile_alignment"] = profile_alignment

    if schema_version:
        telemetry["schema_version"] = schema_version

    if schema_diff_report:
        try:
            diff = _load_json(schema_diff_report)
            violations = diff.get("policy_violations", []) or []
            if isinstance(violations, list):
                normalized = [str(item) for item in violations if isinstance(item, (str, int, float, dict, list))]
                telemetry["schema_policy_violation_count"] = len(normalized)
                if normalized:
                    telemetry["schema_policy_violations"] = normalized
            else:
                raise RuntimeError("policy_violations must be an array in schema diff report")
        except RuntimeError:
            raise
        except Exception as exc:  # pragma: no cover - defensive
            raise RuntimeError(f"failed to read schema diff report {schema_diff_report}: {exc}") from exc

    notes: List[str] = []
    if notes_file:
        notes.extend(_load_notes(notes_file))
    notes.extend(extra_notes)
    if notes:
        telemetry["notes"] = notes

    return telemetry


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Generate a telemetry JSON block for the Swift parity feed.")
    parser.add_argument("--salt-config", type=Path, help="JSON file with keys salt_epoch and last_rotation.")
    parser.add_argument("--profile-alignment", help="Device profile alignment status (ok/drift/etc.).")
    parser.add_argument(
        "--overrides-store",
        type=Path,
        help="Override store JSON (managed via swift_status_export.py telemetry-override).",
    )
    parser.add_argument("--schema-version", default="ios_metrics/v1", help="Schema version identifier.")
    parser.add_argument("--notes-file", type=Path, help="Plain-text file with note lines to include.")
    parser.add_argument("--note", action="append", default=[], dest="notes", help="Add a single note entry.")
    parser.add_argument(
        "--schema-diff-report",
        type=Path,
        help="Optional telemetry-schema-diff JSON report; policy violations will be copied into the telemetry block.",
    )
    parser.add_argument("--output", type=Path, help="Write telemetry JSON to this path (defaults to stdout).")
    return parser


def main(argv: Optional[List[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        telemetry = collect_status(
            salt_config=args.salt_config,
            profile_alignment=args.profile_alignment,
            overrides_store=args.overrides_store,
            schema_version=args.schema_version,
            notes_file=args.notes_file,
            extra_notes=args.notes,
            schema_diff_report=args.schema_diff_report,
        )
    except RuntimeError as exc:
        raise SystemExit(str(exc)) from exc

    output = json.dumps(telemetry, indent=2)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output + "\n", encoding="utf-8")
    else:
        print(output)
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
