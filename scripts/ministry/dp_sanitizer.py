#!/usr/bin/env python3
"""
Wrapper around `cargo xtask ministry-transparency sanitize`.

This script keeps the command-line surface documented in
`docs/source/ministry/transparency_plan.md` so release engineers can call a
single entry point when generating the DP-sanitised metrics + audit report.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


def build_command(args: argparse.Namespace) -> list[str]:
    cmd = [
        "cargo",
        "xtask",
        "ministry-transparency",
        "sanitize",
        "--ingest",
        str(Path(args.ingest).resolve()),
        "--output",
        str(Path(args.output).resolve()),
        "--report",
        str(Path(args.report).resolve()),
    ]
    if args.epsilon_counts is not None:
        cmd.extend(["--epsilon-counts", str(args.epsilon_counts)])
    if args.epsilon_accuracy is not None:
        cmd.extend(["--epsilon-accuracy", str(args.epsilon_accuracy)])
    if args.delta is not None:
        cmd.extend(["--delta", str(args.delta)])
    if args.suppress_threshold is not None:
        cmd.extend(["--suppress-threshold", str(args.suppress_threshold)])
    if args.min_accuracy_samples is not None:
        cmd.extend(["--min-accuracy-samples", str(args.min_accuracy_samples)])
    if args.seed is not None:
        cmd.extend(["--seed", str(args.seed)])
    return cmd


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Run the Ministry transparency DP sanitizer"
    )
    parser.add_argument("--ingest", required=True, help="Path to ingest snapshot JSON")
    parser.add_argument(
        "--output",
        required=True,
        help="Destination for the sanitized metrics JSON",
    )
    parser.add_argument(
        "--report",
        required=True,
        help="Destination for the DP audit report JSON",
    )
    parser.add_argument(
        "--epsilon-counts",
        type=float,
        default=None,
        help="Override epsilon for count metrics (default 0.75)",
    )
    parser.add_argument(
        "--epsilon-accuracy",
        type=float,
        default=None,
        help="Override epsilon for AI accuracy metrics (default 0.5)",
    )
    parser.add_argument(
        "--delta",
        type=float,
        default=None,
        help="Override delta for Gaussian noise (default 1e-6)",
    )
    parser.add_argument(
        "--suppress-threshold",
        type=int,
        default=None,
        help="Override suppression threshold (default 5)",
    )
    parser.add_argument(
        "--min-accuracy-samples",
        type=int,
        default=None,
        help="Override minimum samples before publishing accuracy rows (default 100)",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=None,
        help="Optional deterministic RNG seed",
    )
    args = parser.parse_args()

    cmd = build_command(args)
    try:
        subprocess.run(cmd, check=True)
    except subprocess.CalledProcessError as exc:
        print(f"[dp_sanitizer] command failed with exit code {exc.returncode}", file=sys.stderr)
        raise SystemExit(exc.returncode) from exc


if __name__ == "__main__":
    main()
