#!/usr/bin/env python3
"""Inject telemetry/redaction metadata into a Swift parity dashboard feed."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Dict, List, Optional


def _load_json(path: Path) -> Dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:  # pragma: no cover - surfaced to CLI
        raise RuntimeError(f"parity feed not found: {path}") from exc
    except json.JSONDecodeError as exc:  # pragma: no cover - surfaced to CLI
        raise RuntimeError(f"invalid JSON in {path}: {exc}") from exc


def _load_json_object(path: Path, description: str) -> Dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:  # pragma: no cover
        raise RuntimeError(f"{description} JSON not found: {path}") from exc
    except json.JSONDecodeError as exc:  # pragma: no cover
        raise RuntimeError(f"invalid {description} JSON in {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise RuntimeError(f"{description} JSON {path} must be an object")
    return payload


def _load_telemetry(path: Path) -> Dict[str, Any]:
    return _load_json_object(path, "telemetry")


def _load_pipeline_metadata(path: Path) -> Dict[str, Any]:
    return _load_json_object(path, "pipeline metadata")


def _maybe(value: Optional[str]) -> Optional[str]:
    return value if value not in {None, ""} else None


def enrich_parity_feed(
    *,
    parity_path: Path,
    output_path: Optional[Path],
    telemetry_json: Optional[Path],
    pipeline_metadata: Optional[Path],
    salt_epoch: Optional[str],
    salt_rotation_hours: Optional[float],
    overrides_open: Optional[int],
    profile_alignment: Optional[str],
    schema_version: Optional[str],
    notes: List[str],
) -> None:
    parity = _load_json(parity_path)
    telemetry_data: Dict[str, Any] = {}

    if telemetry_json:
        telemetry_data.update(_load_telemetry(telemetry_json))

    if salt_epoch is not None:
        telemetry_data["salt_epoch"] = salt_epoch
    if salt_rotation_hours is not None:
        telemetry_data["salt_rotation_age_hours"] = float(salt_rotation_hours)
    if overrides_open is not None:
        telemetry_data["overrides_open"] = int(overrides_open)
    if profile_alignment is not None:
        telemetry_data["device_profile_alignment"] = profile_alignment
    if schema_version is not None:
        telemetry_data["schema_version"] = schema_version
    if notes:
        telemetry_data["notes"] = notes
    elif "notes" not in telemetry_data:
        telemetry_data["notes"] = []

    # Normalize empty telemetry payload by clearing the key so the feed stays lean.
    cleaned = {k: v for k, v in telemetry_data.items() if v not in (None, [], {})}
    if cleaned:
        parity["telemetry"] = cleaned
    elif "telemetry" in parity:
        parity.pop("telemetry")

    if pipeline_metadata:
        metadata_payload = _load_pipeline_metadata(pipeline_metadata)
        pipeline_block = parity.get("pipeline")
        if not isinstance(pipeline_block, dict):
            pipeline_block = {}
            parity["pipeline"] = pipeline_block
        pipeline_block["metadata"] = metadata_payload
        pipeline_block["metadata_source"] = str(pipeline_metadata)

    dest = output_path or parity_path
    dest.write_text(json.dumps(parity, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Add telemetry metadata to a Swift parity feed JSON file.")
    parser.add_argument(
        "--input",
        type=Path,
        required=True,
        help="Path to the parity feed JSON (mobile_parity).",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Optional output path. Defaults to overwriting --input.",
    )
    parser.add_argument(
        "--telemetry-json",
        type=Path,
        help="Optional JSON file containing a telemetry object with the fields documented in ios_metrics.md.",
    )
    parser.add_argument(
        "--pipeline-metadata",
        type=Path,
        help="Optional JSON file containing pipeline metadata shared with the Android parity dashboard.",
    )
    parser.add_argument("--salt-epoch", help="Salt epoch identifier (e.g., 2026Q1).")
    parser.add_argument(
        "--salt-rotation-hours",
        type=float,
        help="Hours since the salt was rotated (floating point).",
    )
    parser.add_argument(
        "--overrides-open",
        type=int,
        help="Number of active telemetry redaction overrides.",
    )
    parser.add_argument(
        "--profile-alignment",
        help="Device profile alignment status (e.g., ok, drift, unknown).",
    )
    parser.add_argument(
        "--schema-version",
        help="Schema version string for the telemetry feed (e.g., ios_metrics/v1).",
    )
    parser.add_argument(
        "--note",
        action="append",
        default=[],
        dest="notes",
        help="Add a note entry to the telemetry block (repeatable).",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        enrich_parity_feed(
            parity_path=args.input,
            output_path=args.output,
            telemetry_json=args.telemetry_json,
            pipeline_metadata=args.pipeline_metadata,
            salt_epoch=_maybe(args.salt_epoch),
            salt_rotation_hours=args.salt_rotation_hours,
            overrides_open=args.overrides_open,
            profile_alignment=_maybe(args.profile_alignment),
            schema_version=_maybe(args.schema_version),
            notes=args.notes,
        )
    except RuntimeError as exc:  # pragma: no cover - CLI error handling
        raise SystemExit(str(exc)) from exc
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
