#!/usr/bin/env python3
"""Run a Kagemusha V4 production command under the reviewed memory ceiling."""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

from kagemusha_staged_resource_guard import (
    DEFAULT_FOOTPRINT_INTERVAL_SECONDS,
    DEFAULT_MAX_MEMORY_GIB,
    DEFAULT_MINIMUM_HEADROOM_GIB,
    DEFAULT_SAMPLE_INTERVAL_SECONDS,
    HeavyJobLockUnavailable,
    run_guarded_command,
)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse the runner command line."""

    parser = argparse.ArgumentParser(
        description=(
            "Run one Kagemusha V4 generation/acceptance command with a 16 GiB "
            "maximum and reserved host headroom."
        )
    )
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--max-memory-gib", type=float, default=DEFAULT_MAX_MEMORY_GIB)
    parser.add_argument(
        "--minimum-headroom-gib", type=float, default=DEFAULT_MINIMUM_HEADROOM_GIB
    )
    parser.add_argument(
        "--sample-interval-seconds",
        type=float,
        default=DEFAULT_SAMPLE_INTERVAL_SECONDS,
    )
    parser.add_argument(
        "--footprint-interval-seconds",
        type=float,
        default=DEFAULT_FOOTPRINT_INTERVAL_SECONDS,
    )
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    if args.command and args.command[0] == "--":
        args.command = args.command[1:]
    if not args.command:
        parser.error("a command is required after --")
    return args


def main(argv: list[str] | None = None) -> int:
    """Run the guarded command and return its guarded status."""

    args = parse_args(argv)
    try:
        result = run_guarded_command(
            args.command,
            report_path=args.report,
            max_memory_gib=args.max_memory_gib,
            minimum_headroom_gib=args.minimum_headroom_gib,
            sample_interval_seconds=args.sample_interval_seconds,
            footprint_interval_seconds=args.footprint_interval_seconds,
        )
    except (
        HeavyJobLockUnavailable,
        OSError,
        RuntimeError,
        ValueError,
        subprocess.SubprocessError,
    ) as error:
        print(f"Kagemusha V4 resource guard refused to start: {error}", file=sys.stderr)
        return 2
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
