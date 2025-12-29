#!/usr/bin/env python3
"""
Aggregate Norito-RPC fixture cadence summaries (NRPC-4F1).

This helper scans the JSON artefacts emitted by
`scripts/run_norito_rpc_fixtures.sh` and produces an aggregated report that
highlights the latest run per SDK, manifest/schema digests, and stale cadence
slots. Use this to automate the NRPC-4 fixture tracker and the status updates
referenced in `roadmap.md`.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Sequence
import sys


def _parse_timestamp(value: str) -> datetime:
    stripped = value.strip()
    if stripped.endswith("Z"):
        stripped = stripped[:-1] + "+00:00"
    parsed = datetime.fromisoformat(stripped)
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def _format_timestamp(dt: datetime) -> str:
    return dt.astimezone(timezone.utc).replace(microsecond=0).isoformat().replace(
        "+00:00", "Z"
    )


@dataclass
class FixtureSummary:
    path: Path
    sdk: str
    timestamp: datetime
    status: str
    rotation: str | None
    note: str | None
    duration_seconds: float | None
    manifest: dict
    schema_manifest: dict
    log_path: str | None
    summary_path: str | None
    xtask_artifact: dict | None
    xtask_report: dict | None

    @classmethod
    def from_file(cls, path: Path) -> "FixtureSummary":
        data = json.loads(path.read_text(encoding="utf-8"))
        try:
            sdk = data["sdk"]
            timestamp = _parse_timestamp(data["timestamp"])
            status = data["status"]
            fixtures = data["fixtures"]
            manifest = fixtures["manifest"]
            schema_manifest = fixtures["schema_manifest"]
        except KeyError as err:  # pragma: no cover - defensive validation
            raise ValueError(f"{path} missing required field: {err}") from err
        artifacts = data.get("artifacts", {})
        verification = data.get("verification", {})
        return cls(
            path=path,
            sdk=sdk,
            timestamp=timestamp,
            status=status,
            rotation=data.get("rotation"),
            note=data.get("note"),
            duration_seconds=data.get("duration_seconds"),
            manifest=manifest,
            schema_manifest=schema_manifest,
            log_path=artifacts.get("log"),
            summary_path=artifacts.get("summary"),
            xtask_artifact=verification.get("artifact"),
            xtask_report=verification.get("report"),
        )

    def serialize(self, now: datetime, max_age_days: float | None) -> dict:
        delta = now - self.timestamp
        age_hours = max(delta.total_seconds() / 3600.0, 0.0)
        stale = bool(max_age_days is not None and age_hours > max_age_days * 24.0)
        report = self.xtask_report or {}
        sdk_hashes = {}
        for entry in report.get("sdk_manifests", []):
            manifest = entry.get("manifest") or {}
            sdk_hashes[entry.get("sdk", "unknown")] = manifest.get("sha256")
        serialized = {
            "sdk": self.sdk,
            "timestamp": _format_timestamp(self.timestamp),
            "status": self.status,
            "rotation": self.rotation,
            "note": self.note,
            "duration_seconds": self.duration_seconds,
            "manifest": self.manifest,
            "schema_manifest": self.schema_manifest,
            "log_path": self.log_path,
            "summary_path": self.summary_path,
            "xtask_artifact_path": (self.xtask_artifact or {}).get("path"),
            "fixture_count": report.get("fixture_count"),
            "sdk_manifest_hashes": sdk_hashes,
            "canonical_manifest": report.get("canonical_manifest"),
            "schema_manifest_report": report.get("schema_manifest"),
            "stale": stale,
            "age_hours": round(age_hours, 2),
            "source_path": str(self.path),
        }
        return serialized


def discover_input_files(
    root: Path, pattern: str, explicit: Sequence[Path] | None
) -> list[Path]:
    if explicit:
        paths = [path if path.is_absolute() else root / path for path in explicit]
    else:
        paths = sorted(root.glob(pattern))
    return [path for path in paths if path.is_file()]


def build_report(
    summaries: Sequence[FixtureSummary],
    now: datetime,
    max_age_days: float | None,
) -> dict:
    serialized = [
        summary.serialize(now, max_age_days) for summary in sorted(
            summaries, key=lambda item: item.timestamp, reverse=True
        )
    ]
    per_sdk: dict[str, dict] = {}
    for entry in serialized:
        sdk = entry["sdk"]
        if sdk in per_sdk:
            continue
        per_sdk[sdk] = {
            "latest_timestamp": entry["timestamp"],
            "status": entry["status"],
            "rotation": entry.get("rotation"),
            "manifest_sha256": entry["manifest"].get("sha256"),
            "schema_sha256": entry["schema_manifest"].get("sha256"),
            "stale": entry["stale"],
            "age_hours": entry["age_hours"],
            "summary_path": entry.get("summary_path"),
        }
    return {
        "generated_at": _format_timestamp(now),
        "entry_count": len(serialized),
        "max_age_days": max_age_days,
        "entries": serialized,
        "per_sdk": per_sdk,
    }


def render_markdown(entries: Sequence[dict]) -> str:
    lines = [
        "| SDK | Timestamp (UTC) | Status | Rotation | Manifest SHA256 | Schema SHA256 | Note |",
        "|-----|-----------------|--------|----------|-----------------|---------------|------|",
    ]
    for entry in entries:
        status = entry["status"]
        if entry["stale"]:
            status = f"{status} ⚠️ stale"
        rotation = entry.get("rotation") or "—"
        note = entry.get("note") or "—"
        manifest_sha = entry["manifest"].get("sha256", "—")
        schema_sha = entry["schema_manifest"].get("sha256", "—")
        lines.append(
            f"| `{entry['sdk']}` | {entry['timestamp']} | {status} | {rotation} | "
            f"`{manifest_sha}` | `{schema_sha}` | {note} |"
        )
    return "\n".join(lines).rstrip() + "\n"


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Summarise Norito-RPC fixture cadence runs."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path("artifacts/norito_rpc"),
        help="Directory containing cadence summaries (default: artifacts/norito_rpc).",
    )
    parser.add_argument(
        "--glob",
        default="*-norito-rpc.json",
        help="Glob used to find summary JSON files (default: *-norito-rpc.json).",
    )
    parser.add_argument(
        "--input",
        type=Path,
        action="append",
        help="Explicit summary JSON path (can be repeated).",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Optional JSON output path for the aggregated report.",
    )
    parser.add_argument(
        "--markdown",
        type=Path,
        help="Optional Markdown output path for a human-readable table.",
    )
    parser.add_argument(
        "--max-age-days",
        type=float,
        default=None,
        help="Flag entries older than this many days as stale.",
    )
    parser.add_argument(
        "--now",
        type=str,
        help="Override the current timestamp (ISO 8601, UTC).",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress informational stdout output.",
    )
    return parser.parse_args(list(argv) if argv is not None else None)


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)
    root = args.root.resolve()
    files = discover_input_files(root, args.glob, args.input)
    if not files:
        print(
            f"[norito-rpc] no summary files found under {root} (glob={args.glob})",
            file=sys.stderr,
        )
        return 1
    summaries = [FixtureSummary.from_file(path) for path in files]
    now = _parse_timestamp(args.now) if args.now else datetime.now(timezone.utc)
    report = build_report(summaries, now, args.max_age_days)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if args.markdown:
        args.markdown.parent.mkdir(parents=True, exist_ok=True)
        args.markdown.write_text(render_markdown(report["entries"]), encoding="utf-8")
    if not args.quiet:
        print(
            f"[norito-rpc] summarised {len(files)} run(s); "
            f"latest SDKs: {', '.join(report['per_sdk'])}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
