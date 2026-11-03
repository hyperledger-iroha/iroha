#!/usr/bin/env python3
"""Sample Linux capacity while running an unchanged CI command (Python 3.11+).

Usage: python3 -I -S ci/observe_build_capacity.py -- /usr/bin/time -v cargo ...
GITHUB_WORKSPACE and RUNNER_TEMP select the filesystems to observe. Five-second
snapshots go directly to stderr, including before a runner shutdown can prevent
post-job steps. Reads, process counts and output frequency are bounded; absent
kernel views remain explicitly unavailable. These are point-in-time diagnostics,
not compiler peak measurements, performance qualification or a failure cause.
The command inherits its caller's environment, streams and working directory.
This observer never signals, retries, limits or changes the child command.
"""

from __future__ import annotations

import argparse
import heapq
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

INTERVAL_SECONDS = 5
MAX_SAMPLES = 1080
MAX_PROCESSES = 256
CGROUP_FILES = ("memory.max", "memory.current", "memory.peak", "memory.events",
                "memory.swap.max", "memory.swap.current")


def bounded_text(path: Path, limit: int = 4096) -> str:
    """Read at most limit+1 bytes and report unavailable or truncated data."""
    try:
        with path.open("rb") as stream:
            raw = stream.read(limit + 1)
        return raw[:limit].decode("utf-8", errors="replace") + (
            " [truncated]" if len(raw) > limit else "")
    except OSError as error:
        return f"unavailable ({type(error).__name__})"


def selected_numbers(text: str, names: tuple[str, ...]) -> dict[str, int]:
    """Extract only the named non-negative kernel counters, without arbitrary text."""
    result = {}
    for line in text.splitlines():
        parts = line.replace(":", " ").split()
        if (len(parts) >= 2 and parts[0] in names and len(parts[1]) <= 20
                and parts[1].isascii() and parts[1].isdigit()):
            result[parts[0]] = int(parts[1])
    return result


def cgroup_scopes(root: Path, membership: str) -> list[Path]:
    """Include the actual cgroup and its parents without accepting path traversal."""
    scopes = [root]
    if "[truncated]" in membership:
        return scopes
    for line in membership.splitlines():
        fields = line.split(":", 2)
        if len(fields) != 3 or fields[:2] != ["0", ""]:
            continue
        relative = Path(fields[2])
        if not relative.is_absolute() or ".." in relative.parts or len(relative.parts) > 17:
            continue
        for size in range(2, len(relative.parts) + 1):
            scope = root.joinpath(*relative.parts[1:size])
            try:
                if scope.resolve().is_relative_to(root.resolve()):
                    scopes.append(scope)
            except (OSError, RuntimeError):
                continue
        # A process has one unified-hierarchy membership, never multiple roots.
        break
    return list(dict.fromkeys(scopes))


def snapshot(proc: Path = Path("/proc"), cgroups: Path = Path("/sys/fs/cgroup")) -> dict:
    """Collect bounded host/cgroup/disk observations and at most five process names/RSS."""
    meminfo = bounded_text(proc / "meminfo", 16384)
    vmstat = bounded_text(proc / "vmstat", 16384)
    membership = bounded_text(proc / "self/cgroup")
    processes = []
    scanned = 0
    truncated = False
    process_scan_error = None
    try:
        with os.scandir(proc) as entries:
            for entry in entries:
                if not entry.name.isdecimal():
                    continue
                if scanned == MAX_PROCESSES:
                    truncated = True
                    break
                scanned += 1
                status = bounded_text(Path(entry.path) / "status")
                rss = selected_numbers(status, ("VmRSS",))
                names = [line[5:].strip()[:64] for line in status.splitlines()
                         if line.startswith("Name:")]
                if "VmRSS" in rss and names:
                    processes.append({"name": names[0], "rss_kib": rss["VmRSS"]})
    except OSError as error:
        process_scan_error = f"unavailable ({type(error).__name__})"
    disks = {}
    for name in ("GITHUB_WORKSPACE", "RUNNER_TEMP"):
        try:
            usage = shutil.disk_usage(os.environ[name])
            disks[name] = {"total_bytes": usage.total, "free_bytes": usage.free}
        except (KeyError, OSError) as error:
            disks[name] = f"unavailable ({type(error).__name__})"
    return {
        "utc_unix_seconds": time.time(),
        "host_memory_kib": selected_numbers(meminfo, ("MemTotal", "MemAvailable", "SwapTotal", "SwapFree")) or meminfo,
        "host_vm_counters": selected_numbers(vmstat, ("oom_kill", "pgmajfault", "pswpin", "pswpout")) or vmstat,
        "memory_pressure": bounded_text(proc / "pressure/memory", 1024),
        "cgroup_membership": membership,
        "cgroups": {str(scope): {name: bounded_text(scope / name, 1024) for name in CGROUP_FILES}
                    for scope in cgroup_scopes(cgroups, membership)},
        "disks": disks,
        "processes_scanned": scanned,
        "process_scan_truncated": truncated,
        "process_scan_error": process_scan_error,
        "largest_sampled_processes": heapq.nlargest(5, processes, key=lambda row: row["rss_kib"]),
    }


def emit_snapshot(**completion: object) -> None:
    """Flush diagnostics immediately without allowing observer errors to stop the child."""
    try:
        observation = snapshot()
    except Exception as error:
        # This is optional observation, never a condition for the command to run.
        # Do not log exception text: it can include unintended process inputs.
        observation = {"unavailable": type(error).__name__}
    record = {**completion, "final": observation} if completion else observation
    try:
        print("[build-capacity] " + json.dumps(record, sort_keys=True),
              file=sys.stderr, flush=True)
    except OSError:
        pass


def observe(command: list[str]) -> int:
    """Run exact argv, emit bounded diagnostics and preserve the child's exit status."""
    child = subprocess.Popen(command)
    samples = 0
    while True:
        if samples < MAX_SAMPLES:
            emit_snapshot()
            samples += 1
        try:
            code = child.wait(timeout=INTERVAL_SECONDS)
            break
        except subprocess.TimeoutExpired:
            continue
    emit_snapshot(completed=True, child_returncode=code, samples=samples)
    return code if code >= 0 else 128 - code


def main() -> int:
    """Require an explicit child command after the argument separator."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if not args.command or args.command[0] != "--" or len(args.command) == 1:
        parser.error("an explicit command after -- is required")
    return observe(args.command[1:])


if __name__ == "__main__":
    sys.exit(main())
