#!/usr/bin/env python3
"""Run the Sumeragi NPoS stress scenarios and collect summaries.

This helper orchestrates the targeted performance/chaos tests defined in
`integration_tests/tests/sumeragi_npos_performance.rs` and writes their
stdout/stderr into numbered artefact files for later inspection.

Example:

    python3 scripts/run_sumeragi_stress.py --artifacts out/stress

The script does not replace the full baseline harness (see
`scripts/run_sumeragi_baseline.py`) but focuses on fault-injection cases the
roadmap tracks for Milestone A6 (queue backpressure, RBC overflow, redundant
fan-out, jitter, chunk loss). It purposely runs each scenario separately so a
failure is easier to diagnose.

NOTE: The scenarios are resource intensive; run them on a dedicated machine and
    expect the entire suite to take several minutes. Nothing in this helper
    requires network access and it relies solely on `cargo test`.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Iterable


DEFAULT_TESTS = (
    "npos_queue_backpressure_triggers_metrics",
    "npos_rbc_store_backpressure_records_metrics",
    "npos_rbc_chunk_loss_fault_reports_backlog",
    "npos_redundant_send_retries_update_metrics",
    "npos_pacemaker_jitter_within_band",
)


def run_test(test: str, artifact_dir: Path, cargo_env: dict[str, str]) -> dict[str, object]:
    timestamp = time.strftime("%Y%m%d-%H%M%S")
    stem = f"{timestamp}_{test}"
    stdout_path = artifact_dir / f"{stem}.stdout.log"
    stderr_path = artifact_dir / f"{stem}.stderr.log"

    cmd = [
        "cargo",
        "test",
        "-p",
        "integration_tests",
        "--test",
        "sumeragi_npos_performance",
        test,
        "--",
        "--nocapture",
    ]

    with stdout_path.open("w", encoding="utf-8") as stdout_file, stderr_path.open(
        "w", encoding="utf-8"
    ) as stderr_file:
        proc = subprocess.run(
            cmd,
            cwd=cargo_env["CARGO_WORKSPACE"],
            env={**os.environ, **cargo_env},
            stdout=stdout_file,
            stderr=stderr_file,
        )

    return {
        "test": test,
        "returncode": proc.returncode,
        "stdout": stdout_path.as_posix(),
        "stderr": stderr_path.as_posix(),
    }


def parse_args(argv: Iterable[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--artifacts",
        type=Path,
        default=Path("integration_tests/fixtures/sumeragi_stress/latest"),
        help="Directory to write logs and summary JSON",
    )
    parser.add_argument(
        "--tests",
        nargs="*",
        default=list(DEFAULT_TESTS),
        help="Subset of stress tests to execute",
    )
    parser.add_argument(
        "--summary",
        type=Path,
        help="Optional path for aggregated JSON summary (defaults to artifacts directory)",
    )
    return parser.parse_args(argv)


def main(argv: Iterable[str]) -> int:
    args = parse_args(argv)
    artifact_dir = args.artifacts
    artifact_dir.mkdir(parents=True, exist_ok=True)

    workspace_root = Path(__file__).resolve().parents[1]
    cargo_env = {"CARGO_WORKSPACE": workspace_root.as_posix()}

    results: list[dict[str, object]] = []
    for test in args.tests:
        print(f"[run_sumeragi_stress] Running {test}")
        result = run_test(test, artifact_dir, cargo_env)
        results.append(result)
        if result["returncode"] != 0:
            print(
                f"[run_sumeragi_stress] WARNING: {test} exited with {result['returncode']} — see {result['stdout']} / {result['stderr']}",
                file=sys.stderr,
            )
        else:
            print(
                f"[run_sumeragi_stress] Completed {test}; logs stored in {result['stdout']} / {result['stderr']}",
            )

    summary_path = args.summary or (artifact_dir / "summary.json")
    if summary_path.exists():
        try:
            existing = json.loads(summary_path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            print(
                f"[run_sumeragi_stress] Failed to parse existing summary {summary_path}: {exc}",
                file=sys.stderr,
            )
            existing = []
    else:
        existing = []

    merged: dict[str, dict[str, object]] = {}
    order: list[str] = []
    for entry in existing:
        test_name = entry.get("test")
        if not isinstance(test_name, str):
            continue
        merged[test_name] = entry
        order.append(test_name)
    for result in results:
        test_name = result["test"]
        if test_name not in merged:
            order.append(test_name)
        merged[test_name] = result

    summary = [merged[name] for name in order]
    summary_path.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(f"[run_sumeragi_stress] Wrote summary to {summary_path}")

    failed = [r for r in results if r["returncode"] != 0]
    if failed:
        print(
            f"[run_sumeragi_stress] {len(failed)} test(s) failed — inspect the logs above.",
            file=sys.stderr,
        )
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
