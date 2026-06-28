#!/usr/bin/env python3
"""Run a high-load localnet with an RSS guard for peer processes."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence
from urllib.parse import urlparse


DEFAULT_OUT_DIR = Path("/tmp/iroha-oom-repro")
DEFAULT_MEMORY_LIMIT_GB = 8
DEFAULT_PEERS = 4
DEFAULT_COUNT = 100_000
DEFAULT_PARALLEL = 64
DEFAULT_BATCH_SIZE = 1_000
DEFAULT_BATCH_INTERVAL = 1.0
DEFAULT_QUEUE_SOFT_LIMIT = 0
DEFAULT_QUEUE_HARD_LIMIT = 0
DEFAULT_QUEUE_WAIT_TIMEOUT = 300.0
DEFAULT_POST_LOAD_SAMPLE_SECONDS = 30.0
DEFAULT_LOAD_RUNS = 1


@dataclass(frozen=True)
class PeerProcess:
    """A live localnet peer process owned by the guarded run directory."""

    pid: int
    config_path: Path
    command: str


@dataclass(frozen=True)
class MemorySample:
    """One aggregate RSS sample for guarded peer processes."""

    timestamp: float
    total_rss_bytes: int
    max_peer_rss_bytes: int
    peers: int
    phase: str
    run_index: int


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Deploy a localnet, drive tx_load.py, and stop the run if peer RSS exceeds "
            "the configured memory limit."
        )
    )
    parser.add_argument(
        "--iroha-dir",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Repository root containing scripts/deploy_localnet.sh.",
    )
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_OUT_DIR)
    parser.add_argument("--peers", type=int, default=DEFAULT_PEERS)
    parser.add_argument("--count", type=int, default=DEFAULT_COUNT)
    parser.add_argument("--parallel", type=int, default=DEFAULT_PARALLEL)
    parser.add_argument("--batch-size", type=int, default=DEFAULT_BATCH_SIZE)
    parser.add_argument("--batch-interval", type=float, default=DEFAULT_BATCH_INTERVAL)
    parser.add_argument("--memory-limit-gb", type=float, default=DEFAULT_MEMORY_LIMIT_GB)
    parser.add_argument("--poll-interval", type=float, default=1.0)
    parser.add_argument("--base-api-port", type=int, default=48080)
    parser.add_argument("--base-p2p-port", type=int, default=48337)
    parser.add_argument("--seed", default="memory-guard-repro")
    parser.add_argument("--perf-profile", default="10k-permissioned")
    parser.add_argument("--queue-soft-limit", type=int, default=DEFAULT_QUEUE_SOFT_LIMIT)
    parser.add_argument("--queue-hard-limit", type=int, default=DEFAULT_QUEUE_HARD_LIMIT)
    parser.add_argument("--queue-wait-timeout", type=float, default=DEFAULT_QUEUE_WAIT_TIMEOUT)
    parser.add_argument(
        "--post-load-sample-seconds",
        type=float,
        default=DEFAULT_POST_LOAD_SAMPLE_SECONDS,
        help="Seconds to keep sampling peer RSS after tx_load.py exits successfully.",
    )
    parser.add_argument(
        "--load-runs",
        type=int,
        default=DEFAULT_LOAD_RUNS,
        help="Number of tx_load.py runs to execute against the same localnet process lifetime.",
    )
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--python-bin", default=sys.executable)
    parser.add_argument("--debug", action="store_true", help="Use debug binaries instead of release.")
    parser.add_argument("--no-skip-build", action="store_true")
    parser.add_argument(
        "--report",
        type=Path,
        help="Optional JSON report path. Defaults to <out-dir>/memory_guard_report.json.",
    )
    args = parser.parse_args(argv)
    if args.peers < 4:
        parser.error("--peers must be at least 4 for representative localnet consensus")
    if args.count <= 0:
        parser.error("--count must be greater than zero")
    if args.parallel <= 0:
        parser.error("--parallel must be greater than zero")
    if args.batch_size <= 0:
        parser.error("--batch-size must be greater than zero")
    if args.batch_interval < 0:
        parser.error("--batch-interval must not be negative")
    if args.memory_limit_gb <= 0:
        parser.error("--memory-limit-gb must be greater than zero")
    if args.poll_interval <= 0:
        parser.error("--poll-interval must be greater than zero")
    if args.post_load_sample_seconds < 0:
        parser.error("--post-load-sample-seconds must not be negative")
    if args.load_runs <= 0:
        parser.error("--load-runs must be greater than zero")
    return args


def run_checked(cmd: Sequence[str], cwd: Path, env: dict[str, str] | None = None) -> None:
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def build_deploy_cmd(args: argparse.Namespace) -> list[str]:
    deploy = args.iroha_dir / "scripts" / "deploy_localnet.sh"
    cmd = [
        str(deploy),
        "--iroha-dir",
        str(args.iroha_dir),
        "--out-dir",
        str(args.out_dir),
        "--peers",
        str(args.peers),
        "--seed",
        args.seed,
        "--build-line",
        "iroha3",
        "--perf-profile",
        args.perf_profile,
        "--base-api-port",
        str(args.base_api_port),
        "--base-p2p-port",
        str(args.base_p2p_port),
        "--force",
        "--skip-asset-register",
    ]
    if not args.debug:
        cmd.append("--release")
    if args.target_dir is not None:
        cmd.extend(["--target-dir", str(args.target_dir)])
    return cmd


def build_tx_load_cmd(args: argparse.Namespace) -> list[str]:
    profile = "debug" if args.debug else "release"
    target_dir = args.target_dir if args.target_dir is not None else args.iroha_dir / "target"
    iroha_bin = target_dir / profile / "iroha"
    return [
        args.python_bin,
        str(args.iroha_dir / "scripts" / "tx_load.py"),
        "--iroha-bin",
        str(iroha_bin),
        "--client-config",
        str(args.out_dir / "client.toml"),
        "--peer-count",
        str(args.peers),
        "--base-api-port",
        str(args.base_api_port),
        "--count",
        str(args.count),
        "--parallel",
        str(args.parallel),
        "--batch-size",
        str(args.batch_size),
        "--batch-interval",
        str(args.batch_interval),
        "--queue-soft-limit",
        str(args.queue_soft_limit),
        "--queue-hard-limit",
        str(args.queue_hard_limit),
        "--queue-wait-timeout",
        str(args.queue_wait_timeout),
        "--no-wait",
        "--no-index",
    ]


def base_api_port_from_client_config(client_config: Path, fallback: int) -> int:
    """Return the Torii port recorded in a generated client config."""
    try:
        text = client_config.read_text(encoding="utf-8")
    except OSError:
        return fallback
    match = re.search(r'(?m)^\s*torii_url\s*=\s*["\']([^"\']+)["\']', text)
    if match is None:
        return fallback
    parsed = urlparse(match.group(1))
    return parsed.port or fallback


def binary_name(raw: str) -> str:
    if sys.platform.startswith(("win32", "cygwin", "msys")):
        return f"{raw}.exe"
    return raw


def profile_binary_exists(target_dir: Path, profile: str, raw: str) -> bool:
    return (target_dir / profile / binary_name(raw)).is_file()


def peer_config_from_pidfile(pidfile: Path) -> Path:
    stem = pidfile.stem
    return pidfile.with_name(f"{stem}.toml")


def ps_output(args: Sequence[str]) -> str:
    completed = subprocess.run(
        ["ps", *args],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if completed.returncode != 0:
        return ""
    return completed.stdout.strip()


def command_for_pid(pid: int) -> str:
    return ps_output(["-o", "command=", "-p", str(pid)])


def command_owns_peer(command: str, config_path: Path) -> bool:
    if not command:
        return False
    config = str(config_path)
    return "--config" in command and config in command


def peer_processes(out_dir: Path) -> list[PeerProcess]:
    processes = []
    for pidfile in sorted(out_dir.glob("peer*.pid")):
        try:
            pid = int(pidfile.read_text(encoding="utf-8").strip())
        except (OSError, ValueError):
            continue
        config_path = peer_config_from_pidfile(pidfile)
        command = command_for_pid(pid)
        if command_owns_peer(command, config_path):
            processes.append(PeerProcess(pid=pid, config_path=config_path, command=command))
    return processes


def rss_bytes_for_pid(pid: int) -> int:
    raw = ps_output(["-o", "rss=", "-p", str(pid)])
    if not raw:
        return 0
    try:
        return int(raw.splitlines()[-1].strip()) * 1024
    except ValueError:
        return 0


def sample_memory(processes: Iterable[PeerProcess], phase: str, run_index: int) -> MemorySample:
    rss_values = [rss_bytes_for_pid(process.pid) for process in processes]
    return MemorySample(
        timestamp=time.time(),
        total_rss_bytes=sum(rss_values),
        max_peer_rss_bytes=max(rss_values, default=0),
        peers=len(rss_values),
        phase=phase,
        run_index=run_index,
    )


def stop_localnet(out_dir: Path) -> None:
    stop_script = out_dir / "stop.sh"
    if stop_script.exists():
        subprocess.run(["bash", str(stop_script)], cwd=out_dir, check=False)


def write_report(
    path: Path,
    samples: list[MemorySample],
    tx_returncode: int | None,
    *,
    memory_limit_bytes: int,
    post_load_sample_seconds: float,
    load_runs: int = DEFAULT_LOAD_RUNS,
    tx_returncodes: Sequence[int | None] | None = None,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "tx_returncode": tx_returncode,
        "tx_returncodes": list(tx_returncodes or []),
        "load_runs": load_runs,
        "memory_limit_bytes": memory_limit_bytes,
        "post_load_sample_seconds": post_load_sample_seconds,
        "peak_total_rss_bytes": max((sample.total_rss_bytes for sample in samples), default=0),
        "peak_peer_rss_bytes": max((sample.max_peer_rss_bytes for sample in samples), default=0),
        "last_total_rss_bytes": samples[-1].total_rss_bytes if samples else 0,
        "last_peer_rss_bytes": samples[-1].max_peer_rss_bytes if samples else 0,
        "samples": [sample.__dict__ for sample in samples],
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def tx_load_log_path(out_dir: Path, run_index: int, load_runs: int) -> Path:
    """Return the tx_load log path for a guarded load run."""
    if load_runs == 1:
        return out_dir / "tx_load.log"
    return out_dir / f"tx_load_run_{run_index}.log"


def terminate_child(proc: subprocess.Popen[object]) -> None:
    if proc.poll() is not None:
        return
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=10)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    args.iroha_dir = args.iroha_dir.resolve()
    args.out_dir = args.out_dir.resolve()
    report = (args.report or (args.out_dir / "memory_guard_report.json")).resolve()
    limit_bytes = int(args.memory_limit_gb * 1024 * 1024 * 1024)
    env = None
    profile = "debug" if args.debug else "release"
    target_dir = args.target_dir if args.target_dir is not None else args.iroha_dir / "target"
    can_skip_build = all(
        profile_binary_exists(target_dir, profile, binary)
        for binary in ["kagami", "irohad", "iroha"]
    )
    if not args.no_skip_build and can_skip_build:
        env = os.environ.copy()
        env["SKIP_TOOL_BUILD"] = "true"

    run_checked(build_deploy_cmd(args), cwd=args.iroha_dir, env=env)
    args.base_api_port = base_api_port_from_client_config(
        args.out_dir / "client.toml",
        args.base_api_port,
    )

    samples: list[MemorySample] = []
    tx_returncodes: list[int | None] = []
    args.out_dir.mkdir(parents=True, exist_ok=True)

    def write_memory_report(tx_returncode: int | None) -> None:
        write_report(
            report,
            samples,
            tx_returncode,
            memory_limit_bytes=limit_bytes,
            post_load_sample_seconds=args.post_load_sample_seconds,
            load_runs=args.load_runs,
            tx_returncodes=tx_returncodes,
        )

    def append_guarded_sample(phase: str, run_index: int) -> bool:
        processes = peer_processes(args.out_dir)
        sample = sample_memory(processes, phase, run_index)
        samples.append(sample)
        if sample.total_rss_bytes <= limit_bytes:
            return False
        print(
            "memory guard tripped: "
            f"{sample.total_rss_bytes} bytes RSS across {sample.peers} peers "
            f"during {phase} (limit {limit_bytes})",
            file=sys.stderr,
        )
        return True

    try:
        final_returncode = 0
        for run_index in range(1, args.load_runs + 1):
            tx_log = tx_load_log_path(args.out_dir, run_index, args.load_runs)
            tx_env = os.environ.copy()
            tx_env["PYTHONUNBUFFERED"] = "1"
            with tx_log.open("w", encoding="utf-8") as log:
                tx_proc = subprocess.Popen(
                    build_tx_load_cmd(args),
                    cwd=args.iroha_dir,
                    env=tx_env,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    text=True,
                )
                try:
                    while tx_proc.poll() is None:
                        if append_guarded_sample("load", run_index):
                            terminate_child(tx_proc)
                            tx_returncodes.append(tx_proc.returncode)
                            write_memory_report(tx_proc.returncode)
                            return 3
                        time.sleep(args.poll_interval)
                finally:
                    terminate_child(tx_proc)

            tx_returncodes.append(tx_proc.returncode)
            if tx_proc.returncode != 0:
                final_returncode = int(tx_proc.returncode or 1)
                break

            if args.post_load_sample_seconds > 0:
                deadline = time.monotonic() + args.post_load_sample_seconds
                while time.monotonic() < deadline:
                    if append_guarded_sample("post_load", run_index):
                        write_memory_report(tx_proc.returncode)
                        return 3
                    time.sleep(args.poll_interval)

        write_memory_report(final_returncode)
        return int(final_returncode or 0)
    finally:
        stop_localnet(args.out_dir)


if __name__ == "__main__":
    raise SystemExit(main())
