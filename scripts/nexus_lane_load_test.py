#!/usr/bin/env python3
"""NX-7 load-test bundler: run the lane smoke helper, gate slot latency, and emit a manifest."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple


def parse_metadata(entries: Iterable[str]) -> Dict[str, str]:
    """Parse repeated key=value entries into a dict."""

    metadata: Dict[str, str] = {}
    for entry in entries:
        if "=" not in entry:
            raise ValueError(f"metadata entry '{entry}' must be key=value")
        key, raw_value = entry.split("=", 1)
        key = key.strip()
        value = raw_value.strip()
        if not key:
            raise ValueError(f"metadata entry '{entry}' has an empty key")
        metadata[key] = value
    return metadata


def parse_alias_migrations(entries: Iterable[str]) -> List[Tuple[str, str]]:
    """Convert OLD:NEW pairs into tuples."""

    migrations: List[Tuple[str, str]] = []
    for entry in entries:
        if ":" not in entry:
            raise ValueError(f"alias migration '{entry}' must be formatted as OLD:NEW")
        old, new = entry.split(":", 1)
        old = old.strip()
        new = new.strip()
        if not old or not new:
            raise ValueError(f"alias migration '{entry}' cannot be empty")
        migrations.append((old, new))
    return migrations


def run_command(command: Sequence[str], log_path: Path, description: str) -> None:
    """Run *command* and write stdout/stderr to *log_path*."""

    result = subprocess.run(command, capture_output=True, text=True)
    log_path.write_text(result.stdout + (result.stderr or ""), encoding="utf-8")
    if result.returncode != 0:
        raise RuntimeError(f"{description} failed (see {log_path})")


def build_smoke_command(args: argparse.Namespace, script: Path) -> List[str]:
    """Construct argv for the smoke helper based on parsed args."""

    command: List[str] = [
        args.python,
        str(script),
        "--status-file",
        str(args.status_file),
        "--metrics-file",
        str(args.metrics_file),
        "--min-da-quorum",
        str(args.min_da_quorum),
        "--max-oracle-staleness",
        str(args.max_oracle_staleness),
        "--expected-oracle-twap",
        str(args.expected_oracle_twap),
        "--oracle-twap-tolerance",
        str(args.oracle_twap_tolerance),
        "--max-oracle-haircut-bps",
        str(args.max_oracle_haircut_bps),
        "--min-settlement-buffer",
        str(args.min_settlement_buffer),
        "--min-block-height",
        str(args.min_block_height),
        "--max-finality-lag",
        str(args.max_finality_lag),
        "--max-settlement-backlog",
        str(args.max_settlement_backlog),
        "--min-teu-capacity",
        str(args.min_teu_capacity),
        "--max-teu-slot-commit-ratio",
        str(args.max_teu_slot_commit_ratio),
        "--max-teu-deferrals",
        str(args.max_teu_deferrals),
        "--max-must-serve-truncations",
        str(args.max_must_serve_truncations),
        "--min-slot-samples",
        str(args.min_slot_samples),
    ]
    if args.expected_lane_count is not None:
        command.extend(["--expected-lane-count", str(args.expected_lane_count)])
    if args.telemetry_file is not None:
        command.extend(["--telemetry-file", str(args.telemetry_file)])
    for old_alias, new_alias in args.alias_migrations:
        command.extend(["--require-alias-migration", f"{old_alias}:{new_alias}"])
    for alias in args.lane_alias:
        command.extend(["--lane-alias", alias])
    if args.max_headroom_events is not None:
        command.extend(["--max-headroom-events", str(args.max_headroom_events)])
    if args.max_slot_p95 is not None:
        command.extend(["--max-slot-p95", str(args.max_slot_p95)])
    if args.max_slot_p99 is not None:
        command.extend(["--max-slot-p99", str(args.max_slot_p99)])
    if args.allow_missing_lane_metrics:
        command.append("--allow-missing-lane-metrics")
    return command


def build_slot_summary_command(args: argparse.Namespace, script: Path, summary_path: Path) -> List[str]:
    """Construct argv for the slot-duration gate."""

    command: List[str] = [
        args.python,
        str(script),
        str(args.metrics_file),
        "--min-samples",
        str(args.min_slot_samples),
        "--json-out",
        str(summary_path),
        "--quiet",
    ]
    if args.max_slot_p95 is not None:
        command.extend(["--max-p95-ms", str(args.max_slot_p95)])
    if args.max_slot_p99 is not None:
        command.extend(["--max-p99-ms", str(args.max_slot_p99)])
    return command


def write_manifest(args: argparse.Namespace, out_dir: Path, artefacts: Dict[str, Path]) -> Path:
    """Materialise the JSON manifest describing the load-test bundle."""

    manifest = {
        "version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "lanes": args.lane_alias,
        "inputs": {
            "status_file": str(args.status_file),
            "metrics_file": str(args.metrics_file),
            "telemetry_file": str(args.telemetry_file) if args.telemetry_file else None,
            "alias_migrations": args.alias_migrations,
        },
        "artifacts": {key: str(path) for key, path in artefacts.items()},
        "slot_range": args.slot_range,
        "workload_seed": args.workload_seed,
        "thresholds": {
            "min_da_quorum": args.min_da_quorum,
            "max_oracle_staleness": args.max_oracle_staleness,
            "expected_oracle_twap": args.expected_oracle_twap,
            "oracle_twap_tolerance": args.oracle_twap_tolerance,
            "max_oracle_haircut_bps": args.max_oracle_haircut_bps,
            "min_settlement_buffer": args.min_settlement_buffer,
            "min_block_height": args.min_block_height,
            "max_finality_lag": args.max_finality_lag,
            "max_settlement_backlog": args.max_settlement_backlog,
            "max_headroom_events": args.max_headroom_events,
            "min_teu_capacity": args.min_teu_capacity,
            "max_teu_slot_commit_ratio": args.max_teu_slot_commit_ratio,
            "max_teu_deferrals": args.max_teu_deferrals,
            "max_must_serve_truncations": args.max_must_serve_truncations,
            "max_slot_p95": args.max_slot_p95,
            "max_slot_p99": args.max_slot_p99,
            "min_slot_samples": args.min_slot_samples,
            "expected_lane_count": args.expected_lane_count,
        },
    }
    if args.metadata:
        manifest["metadata"] = args.metadata
    manifest_path = out_dir / "load_test_manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest_path


def parse_args(argv: Optional[Sequence[str]]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--status-file", required=True, type=Path, help="Torii status JSON payload.")
    parser.add_argument("--metrics-file", required=True, type=Path, help="Prometheus metrics snapshot.")
    parser.add_argument(
        "--telemetry-file",
        type=Path,
        help="Newline-delimited telemetry log containing nexus.lane.topology events.",
    )
    parser.add_argument(
        "--require-alias-migration",
        action="append",
        default=[],
        metavar="OLD:NEW",
        help="Require the telemetry log to include alias migration events (OLD:NEW).",
    )
    parser.add_argument(
        "--lane-alias",
        action="append",
        required=True,
        help="Lane alias expected to be ready (repeatable).",
    )
    parser.add_argument(
        "--expected-lane-count",
        type=int,
        help="Expected nexus_lane_configured_total gauge value.",
    )
    parser.add_argument(
        "--out-dir",
        required=True,
        type=Path,
        help="Directory where load-test artefacts should be written.",
    )
    parser.add_argument(
        "--python",
        default=sys.executable,
        help="Python interpreter used to invoke helper scripts (default: current interpreter).",
    )
    parser.add_argument(
        "--slot-range",
        help="Optional slot range label to record in the manifest (e.g., 81200-81600).",
    )
    parser.add_argument(
        "--workload-seed",
        help="Optional workload seed recorded in the manifest.",
    )
    parser.add_argument(
        "--metadata",
        action="append",
        default=[],
        help="Additional metadata entries to embed in the manifest (key=value, repeatable).",
    )
    parser.add_argument(
        "--min-da-quorum",
        type=float,
        default=0.95,
        help="Minimum allowable iroha_da_quorum_ratio (default: 0.95).",
    )
    parser.add_argument(
        "--max-oracle-staleness",
        type=float,
        default=75.0,
        help="Maximum allowed iroha_oracle_staleness_seconds (default: 75).",
    )
    parser.add_argument(
        "--expected-oracle-twap",
        type=float,
        default=60.0,
        help="Expected iroha_oracle_twap_window_seconds (default: 60).",
    )
    parser.add_argument(
        "--oracle-twap-tolerance",
        type=float,
        default=5.0,
        help="Allowed drift for the TWAP expectation (default: 5).",
    )
    parser.add_argument(
        "--max-oracle-haircut-bps",
        type=float,
        default=100.0,
        help="Maximum allowed iroha_oracle_haircut_basis_points (default: 100).",
    )
    parser.add_argument(
        "--min-settlement-buffer",
        type=float,
        default=0.25,
        help="Minimum allowable iroha_settlement_buffer_xor (default: 0.25).",
    )
    parser.add_argument(
        "--min-block-height",
        type=int,
        default=1,
        help="Minimum acceptable nexus_lane_block_height per alias (default: 1).",
    )
    parser.add_argument(
        "--max-finality-lag",
        type=float,
        default=4.0,
        help="Maximum allowable nexus_lane_finality_lag_slots per alias (default: 4).",
    )
    parser.add_argument(
        "--max-settlement-backlog",
        type=float,
        default=1.0,
        help="Maximum allowable nexus_lane_settlement_backlog_xor per alias (default: 1).",
    )
    parser.add_argument(
        "--max-headroom-events",
        type=float,
        default=0.0,
        help="Maximum allowable nexus_scheduler_lane_headroom_events_total per alias (default: 0).",
    )
    parser.add_argument(
        "--min-teu-capacity",
        type=float,
        default=1.0,
        help="Minimum allowable nexus_scheduler_lane_teu_capacity per alias (default: 1).",
    )
    parser.add_argument(
        "--max-teu-slot-commit-ratio",
        type=float,
        default=0.9,
        help="Maximum slot_committed/capacity ratio per alias (default: 0.9).",
    )
    parser.add_argument(
        "--max-teu-deferrals",
        type=float,
        default=0.0,
        help="Maximum allowable nexus_scheduler_lane_teu_deferral_total per alias (default: 0).",
    )
    parser.add_argument(
        "--max-must-serve-truncations",
        type=float,
        default=0.0,
        help="Maximum allowable nexus_scheduler_must_serve_truncations_total per alias (default: 0).",
    )
    parser.add_argument(
        "--allow-missing-lane-metrics",
        action="store_true",
        help="Downgrade missing per-lane metrics to warnings instead of failures.",
    )
    parser.add_argument(
        "--max-slot-p95",
        type=float,
        default=1000.0,
        help="Maximum allowable slot-duration p95 in milliseconds (default: 1000).",
    )
    parser.add_argument(
        "--max-slot-p99",
        type=float,
        default=1100.0,
        help="Maximum allowable slot-duration p99 in milliseconds (default: 1100).",
    )
    parser.add_argument(
        "--min-slot-samples",
        type=int,
        default=10,
        help="Minimum slot-duration samples required before gating (default: 10).",
    )
    parsed = parser.parse_args(list(argv) if argv is not None else None)
    parsed.alias_migrations = parse_alias_migrations(parsed.require_alias_migration)
    parsed.metadata = parse_metadata(parsed.metadata)
    return parsed


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = parse_args(argv)

    out_dir = args.out_dir.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    smoke_log = out_dir / "smoke.log"
    slot_summary = out_dir / "slot_summary.json"
    slot_bundle_dir = out_dir / "slot_bundle"
    slot_bundle_manifest = slot_bundle_dir / "slot_bundle_manifest.json"

    smoke_script = Path(__file__).resolve().with_name("nexus_lane_smoke.py")
    slot_summary_script = Path(__file__).resolve().parent / "telemetry" / "check_slot_duration.py"
    slot_bundle_script = Path(__file__).resolve().parent / "telemetry" / "bundle_slot_artifacts.py"

    try:
        run_command(
            build_smoke_command(args, smoke_script),
            smoke_log,
            description="lane smoke check",
        )
        run_command(
            build_slot_summary_command(args, slot_summary_script, slot_summary),
            out_dir / "slot_summary.log",
            description="slot-duration check",
        )
        run_command(
            [
                args.python,
                str(slot_bundle_script),
                "--metrics",
                str(args.metrics_file),
                "--summary",
                str(slot_summary),
                "--out-dir",
                str(slot_bundle_dir),
                "--metadata",
                f"lanes={','.join(args.lane_alias)}",
            ],
            out_dir / "slot_bundle.log",
            description="slot bundle generation",
        )
    except RuntimeError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    manifest_path = write_manifest(
        args,
        out_dir,
        artefacts={
            "smoke_log": smoke_log,
            "slot_summary": slot_summary,
            "slot_bundle_manifest": slot_bundle_manifest,
            "slot_bundle_dir": slot_bundle_dir,
        },
    )
    print(f"[nx7] load-test bundle ready -> {manifest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
