#!/usr/bin/env python3
"""Measure reproducible Iroha build scenarios in an isolated target directory.

The profiler never runs ``cargo clean`` and refuses to reuse a non-empty target
directory unless ``--reuse`` is explicit. This keeps performance measurements
from destroying or contaminating a developer's normal Cargo cache.

Reported CPU time excludes the profiler's own ``ps`` sampler children. RSS is
sampled at least once, including when Cargo exits before the first poll.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import resource
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Sequence


SCENARIOS: dict[str, tuple[str, ...]] = {
    "workspace": ("build", "--locked", "--workspace", "--timings"),
    "data-model": (
        "build",
        "--locked",
        "-p",
        "iroha_data_model",
        "--lib",
        "--timings",
    ),
    "daemon": (
        "build",
        "--locked",
        "-p",
        "irohad",
        "--bin",
        "iroha3d",
        "--timings",
    ),
    "cli": (
        "build",
        "--locked",
        "-p",
        "iroha_cli",
        "--bin",
        "iroha",
        "--timings",
    ),
}


@dataclass(frozen=True)
class Measurement:
    """One completed Cargo invocation and its sampler-adjusted measurements."""

    scenario: str
    command: list[str]
    target_dir: str
    elapsed_seconds: float
    user_cpu_seconds: float
    system_cpu_seconds: float
    peak_process_tree_rss_bytes: int
    target_bytes: int
    return_code: int


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "scenario",
        choices=sorted(SCENARIOS),
        help="Build surface to measure.",
    )
    parser.add_argument(
        "--target-dir",
        type=Path,
        required=True,
        help="Dedicated Cargo target directory; it is never deleted.",
    )
    parser.add_argument(
        "--jobs",
        type=int,
        help="Explicit Cargo job count; omit to measure Cargo's default jobserver.",
    )
    parser.add_argument(
        "--reuse",
        action="store_true",
        help="Allow a non-empty target directory for warm/no-op measurements.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Write the JSON result to this path instead of stdout.",
    )
    return parser.parse_args(argv)


def validate_target_dir(path: Path, *, reuse: bool) -> Path:
    """Create and validate an isolated target directory without deleting data."""
    resolved = path.resolve()
    if resolved == Path(resolved.anchor):
        raise ValueError("target directory must not be a filesystem root")
    if resolved.exists() and not resolved.is_dir():
        raise ValueError(f"target directory is not a directory: {resolved}")
    if resolved.exists() and any(resolved.iterdir()) and not reuse:
        raise ValueError(
            f"target directory is not empty: {resolved}; pass --reuse for a warm build"
        )
    resolved.mkdir(parents=True, exist_ok=True)
    return resolved


def directory_size(path: Path) -> int:
    """Return the total size of regular files below ``path`` without following links."""
    total = 0
    for root, directories, files in os.walk(path, followlinks=False):
        directories[:] = [
            name for name in directories if not (Path(root) / name).is_symlink()
        ]
        for name in files:
            candidate = Path(root) / name
            if not candidate.is_symlink():
                total += candidate.stat().st_size
    return total


def parse_process_table(output: str) -> dict[int, tuple[int, int]]:
    """Parse portable ``ps`` PID, parent PID, and RSS-kibibyte output."""
    processes: dict[int, tuple[int, int]] = {}
    for raw_line in output.splitlines():
        fields = raw_line.split()
        if len(fields) != 3:
            continue
        try:
            pid, parent_pid, rss_kib = (int(field) for field in fields)
        except ValueError:
            continue
        processes[pid] = (parent_pid, rss_kib * 1024)
    return processes


def process_tree_rss_bytes(root_pid: int) -> int:
    """Return aggregate resident bytes for a process and all descendants."""
    output = subprocess.run(
        ["ps", "-axo", "pid=,ppid=,rss="],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    processes = parse_process_table(output)
    descendants = {root_pid}
    changed = True
    while changed:
        changed = False
        for pid, (parent_pid, _rss) in processes.items():
            if parent_pid in descendants and pid not in descendants:
                descendants.add(pid)
                changed = True
    return sum(processes.get(pid, (0, 0))[1] for pid in descendants)


def _child_cpu_seconds() -> tuple[float, float]:
    """Return cumulative user and system CPU for reaped child processes."""
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return usage.ru_utime, usage.ru_stime


def measure(
    root: Path,
    scenario: str,
    target_dir: Path,
    jobs: int | None,
) -> Measurement:
    """Run and measure one Cargo scenario."""
    command = ["cargo", *SCENARIOS[scenario]]
    if jobs is not None:
        if jobs <= 0:
            raise ValueError("--jobs must be greater than zero")
        command.extend(("--jobs", str(jobs)))

    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target_dir)
    before_user_cpu, before_system_cpu = _child_cpu_seconds()
    started = time.monotonic()
    process = subprocess.Popen(command, cwd=root, env=environment)
    peak_process_tree_rss = 0
    sampler_user_cpu = 0.0
    sampler_system_cpu = 0.0
    while True:
        sampler_before_user, sampler_before_system = _child_cpu_seconds()
        try:
            peak_process_tree_rss = max(
                peak_process_tree_rss,
                process_tree_rss_bytes(process.pid),
            )
        except (OSError, subprocess.SubprocessError):
            # Resource sampling must never terminate the build being measured.
            pass
        finally:
            sampler_after_user, sampler_after_system = _child_cpu_seconds()
            sampler_user_cpu += max(0.0, sampler_after_user - sampler_before_user)
            sampler_system_cpu += max(
                0.0, sampler_after_system - sampler_before_system
            )
        if process.poll() is not None:
            break
        time.sleep(0.25)
    return_code = process.wait()
    elapsed = time.monotonic() - started
    after_user_cpu, after_system_cpu = _child_cpu_seconds()

    return Measurement(
        scenario=scenario,
        command=command,
        target_dir=str(target_dir),
        elapsed_seconds=elapsed,
        user_cpu_seconds=max(
            0.0, after_user_cpu - before_user_cpu - sampler_user_cpu
        ),
        system_cpu_seconds=max(
            0.0, after_system_cpu - before_system_cpu - sampler_system_cpu
        ),
        peak_process_tree_rss_bytes=peak_process_tree_rss,
        target_bytes=directory_size(target_dir),
        return_code=return_code,
    )


def render_report(root: Path, measurement: Measurement) -> dict[str, object]:
    """Add reproducibility metadata to a measurement."""
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    rustc = subprocess.run(
        ["rustc", "--version", "--verbose"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    return {
        "schema_version": 1,
        "git_revision": revision,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "rustc": rustc,
        "measurement": asdict(measurement),
    }


def main(argv: Sequence[str] | None = None) -> int:
    """Run the selected measurement and emit its JSON report."""
    args = parse_args(argv)
    root = Path(__file__).resolve().parents[1]
    try:
        target_dir = validate_target_dir(args.target_dir, reuse=args.reuse)
        measurement = measure(root, args.scenario, target_dir, args.jobs)
        report = render_report(root, measurement)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"ERROR: build profiling failed: {error}", file=sys.stderr)
        return 2

    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output is None:
        sys.stdout.write(rendered)
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
        print(f"wrote build profile to {args.output}")
    return measurement.return_code


if __name__ == "__main__":
    raise SystemExit(main())
