#!/usr/bin/env python3
"""Build a redacted anomaly summary for the XCFramework smoke harness."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import List, Sequence


@dataclass(frozen=True)
class LaneAnomaly:
    name: str
    status: str
    message: str | None
    logs: List[str]

    def to_dict(self) -> dict:
        payload = {"lane": self.name, "status": self.status}
        if self.message:
            payload["message"] = self.message
        if self.logs:
            payload["logs"] = self.logs
        return payload


def _iso_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _relative_logs(logs_root: Path, lane: str) -> List[str]:
    paths = []
    lane_root = logs_root / lane
    for candidate in ("build.log", "test.log"):
        path = lane_root / candidate
        if path.exists():
            paths.append(str(path.relative_to(logs_root)))
    return paths


def summarize_anomalies(result_path: Path, logs_root: Path | None = None) -> dict:
    data = json.loads(result_path.read_text(encoding="utf-8"))
    anomalies: List[LaneAnomaly] = []
    lanes = data.get("buildkite", {}).get("lanes", [])
    logs_base = logs_root if logs_root else None

    for lane in lanes:
        name = str(lane.get("name", ""))
        if not name:
            continue
        status = lane.get("status")
        if status is None:
            status = "pass" if float(lane.get("success_rate_14_runs", 0.0)) >= 1.0 else "fail"
        if status == "pass":
            continue
        logs = _relative_logs(logs_base, name.split(":")[-1]) if logs_base else []
        anomalies.append(
            LaneAnomaly(
                name=name,
                status=status,
                message=lane.get("notes"),
                logs=logs,
            )
        )

    return {
        "generated_at": _iso_now(),
        "open_incidents": data.get("alert_state", {}).get("open_incidents", []),
        "lanes": [entry.to_dict() for entry in anomalies],
    }


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Create XCFramework smoke anomaly summary.")
    parser.add_argument("--result", required=True, type=Path, help="Path to smoke harness JSON output.")
    parser.add_argument("--logs-root", type=Path, help="Root directory containing per-lane logs.")
    parser.add_argument("--out", required=True, type=Path, help="Output anomaly JSON path.")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    payload = summarize_anomalies(args.result, args.logs_root)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(f"[xcframework] anomaly summary written to {args.out}")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
