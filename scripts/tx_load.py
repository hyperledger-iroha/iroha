#!/usr/bin/env python3
"""Submit transaction pings in parallel (optionally sharded) and estimate TPS from /status + /metrics."""

from __future__ import annotations

import argparse
import json
import math
import re
import subprocess
import tempfile
import threading
import time
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

DEFAULT_STATUS_URL = "http://127.0.0.1:8080/status"
DEFAULT_METRICS_URL = "http://127.0.0.1:8080/metrics"


def parse_metric_value(metrics_text: str, metric: str) -> Optional[float]:
    """Parse a Prometheus metric value, summing across labeled samples when present."""
    total = 0.0
    found = False
    for raw in metrics_text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if not line.startswith(metric):
            continue
        parts = line.split()
        if len(parts) < 2:
            continue
        name = parts[0]
        value = parts[1]
        if name != metric and not name.startswith(metric + "{"):
            continue
        try:
            total += float(value.strip())
        except ValueError:
            continue
        found = True
    if not found:
        return None
    return total


def extract_status_snapshot(payload: dict) -> dict:
    """Extract core counters from a /status JSON payload."""
    return {
        "blocks": int(payload.get("blocks", 0) or 0),
        "blocks_non_empty": int(payload.get("blocks_non_empty", 0) or 0),
        "txs_approved": int(payload.get("txs_approved", 0) or 0),
        "txs_rejected": int(payload.get("txs_rejected", 0) or 0),
        "queue_size": int(payload.get("queue_size", 0) or 0),
    }


@dataclass(frozen=True)
class LoadShard:
    torii_url: str
    status_url: str
    count: int
    parallel: int
    batch_size: int
    batch_interval: float
    client_config: Path
    baseline_queue_size: int


@dataclass(frozen=True)
class LoadShardResult:
    shard: LoadShard
    returncode: int
    stdout: str
    stderr: str
    elapsed: float
    rate_limit_hits: int
    submitted: int
    timed_out: bool


def normalize_torii_url(url: str) -> str:
    stripped = url.rstrip("/")
    return f"{stripped}/"


def split_even(total: int, parts: int) -> list[int]:
    if parts <= 0:
        return []
    base = total // parts
    extra = total % parts
    return [base + (1 if idx < extra else 0) for idx in range(parts)]


def torii_url_from_status(status_url: str) -> str:
    parsed = urllib.parse.urlparse(status_url)
    if not parsed.scheme or not parsed.netloc:
        raise ValueError(f"status url must include scheme and host: {status_url}")
    return normalize_torii_url(f"{parsed.scheme}://{parsed.netloc}")


def render_client_config(base_text: str, torii_url: str) -> str:
    lines = base_text.splitlines()
    rendered = []
    replaced = False
    for line in lines:
        if line.strip().startswith("torii_url"):
            rendered.append(f'torii_url = "{torii_url}"')
            replaced = True
        else:
            rendered.append(line)
    if not replaced:
        inserted = False
        for idx, line in enumerate(rendered):
            if line.strip().startswith("chain"):
                rendered.insert(idx + 1, f'torii_url = "{torii_url}"')
                inserted = True
                break
        if not inserted:
            rendered.insert(0, f'torii_url = "{torii_url}"')
    return "\n".join(rendered) + "\n"


def count_rate_limit_hits(output: str) -> int:
    if not output:
        return 0
    hits = output.count("status: 429")
    hits += output.lower().count("too many requests")
    return hits


PING_SUBMIT_RE = re.compile(r"Submitted\s+(?P<submitted>\d+)\s*/\s*(?P<attempted>\d+)\s+ping transactions", re.IGNORECASE)


def parse_ping_submitted(output: str) -> Optional[tuple[int, int]]:
    """Return (submitted, attempted) for `iroha ledger transaction ping` output."""
    if not output:
        return None
    last = None
    for line in output.splitlines():
        match = PING_SUBMIT_RE.search(line)
        if not match:
            continue
        try:
            submitted = int(match.group("submitted"))
            attempted = int(match.group("attempted"))
        except (TypeError, ValueError):
            continue
        last = (submitted, attempted)
    return last


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Drive transaction ping load via iroha CLI and estimate throughput from /status and "
            "/metrics."
        )
    )
    parser.add_argument("--iroha-bin", default="target/release/iroha")
    parser.add_argument("--client-config", required=True, help="Path to client.toml")
    parser.add_argument("--status-url", default=DEFAULT_STATUS_URL)
    parser.add_argument("--metrics-url", default=DEFAULT_METRICS_URL)
    parser.add_argument(
        "--peer-urls",
        help="Comma-separated Torii URLs to shard load across (overrides --peer-count).",
    )
    parser.add_argument(
        "--peer-count",
        type=int,
        help="Number of peers to shard across (derived from --base-api-port).",
    )
    parser.add_argument("--base-api-port", type=int, default=8080)
    parser.add_argument("--peer-host", default="127.0.0.1")
    parser.add_argument(
        "--per-peer",
        action="store_true",
        default=False,
        help="Treat --count/--parallel as per-peer values when sharding.",
    )
    parser.add_argument("--count", type=int, default=10_000)
    parser.add_argument("--parallel", type=int, default=64)
    parser.add_argument("--msg", default="load-ping")
    parser.add_argument("--no-wait", dest="no_wait", action="store_true", default=True)
    parser.add_argument("--wait", dest="no_wait", action="store_false")
    parser.add_argument("--no-index", dest="no_index", action="store_true", default=True)
    parser.add_argument("--with-index", dest="no_index", action="store_false")
    parser.add_argument(
        "--batch-size",
        type=int,
        default=0,
        help="Batch size for paced submission (0 = submit all at once).",
    )
    parser.add_argument(
        "--batch-interval",
        type=float,
        default=0.0,
        help="Seconds between paced submission batches (0 = submit all at once).",
    )
    parser.add_argument("--poll-interval", type=float, default=0.25)
    parser.add_argument("--drain-timeout", type=float, default=120.0)
    parser.add_argument(
        "--cli-timeout",
        type=float,
        default=60.0,
        help=(
            "Maximum seconds to wait for each `iroha ledger transaction ping` subprocess "
            "(0 = wait forever)."
        ),
    )
    parser.add_argument(
        "--queue-soft-limit",
        type=int,
        default=0,
        help=(
            "If >0, pause between batches while (queue_size - baseline_queue_size) exceeds this "
            "value (per shard)."
        ),
    )
    parser.add_argument(
        "--queue-hard-limit",
        type=int,
        default=0,
        help=(
            "If >0, abort load submission when (queue_size - baseline_queue_size) exceeds this "
            "value (per shard)."
        ),
    )
    parser.add_argument(
        "--queue-wait-timeout",
        type=float,
        default=60.0,
        help=(
            "Maximum seconds to wait for the queue to fall below --queue-soft-limit before "
            "aborting that shard batch (0 = wait forever)."
        ),
    )
    parser.add_argument("--block-max", type=int, default=10_000)
    parser.add_argument("--log-level", default="INFO")
    parser.add_argument(
        "--continue-on-failure",
        action="store_true",
        default=False,
        help="Continue to stats collection even if ping submissions fail.",
    )
    args = parser.parse_args()

    if args.count <= 0:
        parser.error("--count must be greater than zero")
    if args.parallel <= 0:
        parser.error("--parallel must be greater than zero")
    if args.peer_urls and args.peer_count:
        parser.error("--peer-urls and --peer-count are mutually exclusive")
    if args.peer_count is not None and args.peer_count <= 0:
        parser.error("--peer-count must be greater than zero")
    if args.batch_size < 0:
        parser.error("--batch-size must be >= 0")
    if args.batch_interval < 0:
        parser.error("--batch-interval must be >= 0")
    if (args.batch_size > 0) != (args.batch_interval > 0):
        parser.error("--batch-size and --batch-interval must be set together")
    if args.queue_soft_limit < 0:
        parser.error("--queue-soft-limit must be >= 0")
    if args.queue_hard_limit < 0:
        parser.error("--queue-hard-limit must be >= 0")
    if args.queue_hard_limit and args.queue_soft_limit and args.queue_hard_limit < args.queue_soft_limit:
        parser.error("--queue-hard-limit must be >= --queue-soft-limit")
    if args.queue_wait_timeout < 0:
        parser.error("--queue-wait-timeout must be >= 0")
    if args.cli_timeout < 0:
        parser.error("--cli-timeout must be >= 0")

    peer_urls = []
    if args.peer_urls:
        peer_urls = [
            normalize_torii_url(url.strip())
            for url in args.peer_urls.split(",")
            if url.strip()
        ]
        if not peer_urls:
            parser.error("--peer-urls did not contain any valid entries")
    elif args.peer_count:
        base_host = args.peer_host.rstrip("/")
        if "://" in base_host:
            base_prefix = base_host
        else:
            base_prefix = f"http://{base_host}"
        for idx in range(args.peer_count):
            port = args.base_api_port + idx
            peer_urls.append(normalize_torii_url(f"{base_prefix}:{port}"))
    else:
        peer_urls = [torii_url_from_status(args.status_url)]

    if (args.peer_urls or args.peer_count) and args.status_url == DEFAULT_STATUS_URL:
        args.status_url = urllib.parse.urljoin(peer_urls[0], "status")
    if (args.peer_urls or args.peer_count) and args.metrics_url == DEFAULT_METRICS_URL:
        args.metrics_url = urllib.parse.urljoin(peer_urls[0], "metrics")

    if args.per_peer:
        count_shards = [args.count for _ in peer_urls]
        parallel_shards = [args.parallel for _ in peer_urls]
        batch_shards = [args.batch_size for _ in peer_urls]
    else:
        count_shards = split_even(args.count, len(peer_urls))
        parallel_shards = split_even(args.parallel, len(peer_urls))
        batch_shards = split_even(args.batch_size, len(peer_urls))

    shard_specs: list[tuple[str, int, int, int]] = []
    for idx, torii_url in enumerate(peer_urls):
        count = count_shards[idx]
        if count == 0:
            continue
        parallel = parallel_shards[idx]
        if parallel <= 0:
            parallel = 1
        batch_size = batch_shards[idx]
        if batch_size <= 0 and args.batch_size > 0:
            batch_size = 1
        shard_specs.append((torii_url, count, parallel, batch_size))

    if not shard_specs:
        parser.error("no load shards configured; check --count and peer settings")

    if len(shard_specs) > 1:
        shard_counts = ", ".join(str(spec[1]) for spec in shard_specs)
        shard_parallel = ", ".join(str(spec[2]) for spec in shard_specs)
        print(f"Sharding load across {len(shard_specs)} peers (counts: {shard_counts})")
        print(f"Parallel per shard: {shard_parallel}")

    snapshots = []
    lock = threading.Lock()
    stop_event = threading.Event()

    def fetch_json(url: str) -> dict:
        with urllib.request.urlopen(url, timeout=5) as resp:
            return json.loads(resp.read().decode("utf-8"))

    def fetch_text(url: str) -> str:
        with urllib.request.urlopen(url, timeout=5) as resp:
            return resp.read().decode("utf-8")

    def poller() -> None:
        while not stop_event.is_set():
            try:
                status_raw = fetch_json(args.status_url)
                metrics_raw = fetch_text(args.metrics_url)
                status = extract_status_snapshot(status_raw)
                dag_vertices = parse_metric_value(metrics_raw, "pipeline_dag_vertices")
            except Exception:
                time.sleep(args.poll_interval)
                continue
            with lock:
                snapshots.append(
                    {
                        "ts": time.monotonic(),
                        "status": status,
                        "dag_vertices": dag_vertices,
                    }
                )
            time.sleep(args.poll_interval)

    thread = threading.Thread(target=poller, name="status-poller", daemon=True)
    thread.start()

    try:
        baseline = fetch_json(args.status_url)
        baseline_status = extract_status_snapshot(baseline)
    except Exception as exc:
        print(f"Failed to read baseline status: {exc}")
        stop_event.set()
        thread.join(timeout=2)
        return 1

    baseline_queue_by_peer: dict[str, int] = {}
    for torii_url in peer_urls:
        status_url = urllib.parse.urljoin(torii_url, "status")
        try:
            status_raw = fetch_json(status_url)
            baseline_queue_by_peer[torii_url] = int(status_raw.get("queue_size", 0) or 0)
        except Exception:
            baseline_queue_by_peer[torii_url] = baseline_status["queue_size"]

    base_config_text = Path(args.client_config).read_text(encoding="utf-8")
    temp_configs: list[Path] = []
    shards: list[LoadShard] = []
    for torii_url, count, parallel, batch_size in shard_specs:
        rendered = render_client_config(base_config_text, torii_url)
        with tempfile.NamedTemporaryFile("w", delete=False, suffix=".toml") as handle:
            handle.write(rendered)
            temp_path = Path(handle.name)
        temp_configs.append(temp_path)
        shards.append(
            LoadShard(
                torii_url=torii_url,
                status_url=urllib.parse.urljoin(torii_url, "status"),
                count=count,
                parallel=parallel,
                batch_size=batch_size,
                batch_interval=args.batch_interval,
                client_config=temp_path,
                baseline_queue_size=baseline_queue_by_peer.get(
                    torii_url, baseline_status["queue_size"]
                ),
            )
        )

    def wait_for_queue(shard: LoadShard) -> bool:
        if args.queue_soft_limit <= 0 and args.queue_hard_limit <= 0:
            return True
        deadline = None
        if args.queue_wait_timeout > 0:
            deadline = time.monotonic() + args.queue_wait_timeout
        while True:
            try:
                status_raw = fetch_json(shard.status_url)
                status = extract_status_snapshot(status_raw)
                delta = status["queue_size"] - shard.baseline_queue_size
            except Exception:
                if deadline is not None and time.monotonic() >= deadline:
                    print(
                        f"Shard {shard.torii_url}: failed to fetch /status within "
                        f"{args.queue_wait_timeout}s; aborting shard."
                    )
                    return False
                time.sleep(args.poll_interval)
                continue
            if args.queue_hard_limit > 0 and delta > args.queue_hard_limit:
                print(
                    f"Shard {shard.torii_url}: queue delta {delta} exceeds hard limit "
                    f"{args.queue_hard_limit}; aborting shard."
                )
                return False
            if args.queue_soft_limit > 0 and delta > args.queue_soft_limit:
                if deadline is not None and time.monotonic() >= deadline:
                    print(
                        f"Shard {shard.torii_url}: queue delta {delta} did not fall below soft "
                        f"limit {args.queue_soft_limit} before timeout; aborting shard."
                    )
                    return False
                time.sleep(args.poll_interval)
                continue
            return True

    def run_ping_shard(shard: LoadShard) -> LoadShardResult:
        ping_cmd = [
            args.iroha_bin,
            "--config",
            str(shard.client_config),
            "ledger",
            "transaction",
            "ping",
            "--msg",
            args.msg,
            "--log-level",
            args.log_level,
        ]
        if args.no_wait:
            ping_cmd.append("--no-wait")
        if args.no_index:
            ping_cmd.append("--no-index")

        stdout_chunks = []
        stderr_chunks = []
        rate_limit_hits = 0
        returncode = 0
        submitted = 0
        timed_out = False
        start = time.monotonic()

        def run_batch(batch_count: int, parallel: int) -> subprocess.CompletedProcess[str]:
            cmd = ping_cmd + [
                "--count",
                str(batch_count),
                "--parallel",
                str(parallel),
            ]
            timeout = None if args.cli_timeout <= 0 else args.cli_timeout
            try:
                return subprocess.run(
                    cmd,
                    check=False,
                    capture_output=True,
                    text=True,
                    timeout=timeout,
                )
            except subprocess.TimeoutExpired as exc:
                stdout = exc.stdout or ""
                stderr = exc.stderr or ""
                if isinstance(stdout, bytes):
                    stdout = stdout.decode("utf-8", errors="replace")
                if isinstance(stderr, bytes):
                    stderr = stderr.decode("utf-8", errors="replace")
                stderr = (
                    stderr
                    + f"\ncli-timeout after {args.cli_timeout:.2f}s: {' '.join(cmd)}\n"
                )
                return subprocess.CompletedProcess(
                    cmd,
                    returncode=124,
                    stdout=stdout,
                    stderr=stderr,
                )

        def extract_submitted(
            combined_output: str,
            attempted: int,
            returncode: int,
        ) -> int:
            parsed = parse_ping_submitted(combined_output)
            if parsed is not None:
                submitted_count, attempted_count = parsed
                # Trust the CLI when present, but keep it within reasonable bounds.
                if attempted_count > 0:
                    return max(0, min(submitted_count, attempted_count))
            if returncode == 0:
                return attempted
            return 0

        if shard.batch_size > 0 and shard.batch_interval > 0:
            parallel = shard.parallel
            attempted = 0
            planned_batches = int(math.ceil(shard.count / shard.batch_size))
            while attempted < shard.count:
                if not wait_for_queue(shard):
                    returncode = 2
                    break
                batch = min(shard.batch_size, shard.count - attempted)
                attempted += batch
                batch_start = time.monotonic()
                result = run_batch(batch, parallel)
                stdout = result.stdout or ""
                stderr = result.stderr or ""
                stdout_chunks.append(stdout)
                stderr_chunks.append(stderr)
                combined = stdout + stderr
                batch_rate_limits = count_rate_limit_hits(combined)
                rate_limit_hits += batch_rate_limits
                batch_submitted = extract_submitted(combined, batch, result.returncode)
                submitted += batch_submitted
                if result.returncode == 124:
                    timed_out = True
                if returncode == 0 and result.returncode != 0:
                    returncode = result.returncode

                missing = batch - batch_submitted
                if (
                    missing > 0
                    and batch_rate_limits == 0
                    and result.returncode not in (124, 2)
                    and parallel > 1
                ):
                    retry_parallel = max(1, parallel // 2)
                    retry = run_batch(missing, retry_parallel)
                    r_stdout = retry.stdout or ""
                    r_stderr = retry.stderr or ""
                    stdout_chunks.append(r_stdout)
                    stderr_chunks.append(r_stderr)
                    r_combined = r_stdout + r_stderr
                    r_limits = count_rate_limit_hits(r_combined)
                    rate_limit_hits += r_limits
                    r_submitted = extract_submitted(r_combined, missing, retry.returncode)
                    submitted += r_submitted
                    missing -= r_submitted
                    if retry.returncode == 124:
                        timed_out = True
                    if returncode == 0 and retry.returncode != 0:
                        returncode = retry.returncode
                    # If the retry also fails, reduce parallelism for subsequent batches.
                    if missing > 0 and retry_parallel > 1:
                        parallel = retry_parallel

                if missing > 0 and planned_batches > 0:
                    # We do not extend the run; load windows are time-based. Missing txs stay
                    # missing to avoid turning perf runs into multi-hour retries.
                    pass

                elapsed = time.monotonic() - batch_start
                sleep_for = shard.batch_interval - elapsed
                if sleep_for > 0:
                    time.sleep(sleep_for)
        else:
            if not wait_for_queue(shard):
                returncode = 2
            else:
                parallel = shard.parallel
                result = run_batch(shard.count, parallel)
                stdout = result.stdout or ""
                stderr = result.stderr or ""
                stdout_chunks.append(stdout)
                stderr_chunks.append(stderr)
                combined = stdout + stderr
                batch_rate_limits = count_rate_limit_hits(combined)
                rate_limit_hits += batch_rate_limits
                returncode = result.returncode
                submitted = extract_submitted(combined, shard.count, result.returncode)
                if result.returncode == 124:
                    timed_out = True
                missing = shard.count - submitted
                if (
                    missing > 0
                    and batch_rate_limits == 0
                    and result.returncode not in (124, 2)
                    and parallel > 1
                ):
                    retry_parallel = max(1, parallel // 2)
                    retry = run_batch(missing, retry_parallel)
                    r_stdout = retry.stdout or ""
                    r_stderr = retry.stderr or ""
                    stdout_chunks.append(r_stdout)
                    stderr_chunks.append(r_stderr)
                    r_combined = r_stdout + r_stderr
                    rate_limit_hits += count_rate_limit_hits(r_combined)
                    submitted += extract_submitted(r_combined, missing, retry.returncode)
                    if retry.returncode == 124:
                        timed_out = True
                    if returncode == 0 and retry.returncode != 0:
                        returncode = retry.returncode

        elapsed = time.monotonic() - start
        return LoadShardResult(
            shard=shard,
            returncode=returncode,
            stdout="".join(stdout_chunks),
            stderr="".join(stderr_chunks),
            elapsed=elapsed,
            rate_limit_hits=rate_limit_hits,
            submitted=submitted,
            timed_out=timed_out,
        )

    submit_start = time.monotonic()
    results = []
    submit_failed = False
    try:
        with ThreadPoolExecutor(max_workers=len(shards)) as executor:
            for result in executor.map(run_ping_shard, shards):
                results.append(result)
                if result.stdout:
                    print(result.stdout, end="")
                if result.stderr:
                    print(result.stderr, end="")
                if result.returncode != 0:
                    submit_failed = True
        submit_elapsed = time.monotonic() - submit_start
    finally:
        for path in temp_configs:
            path.unlink(missing_ok=True)

    if submit_failed and not args.continue_on_failure:
        stop_event.set()
        thread.join(timeout=2)
        print("load submission failed")
        return 1

    drain_deadline = time.monotonic() + args.drain_timeout
    while time.monotonic() < drain_deadline:
        with lock:
            if snapshots:
                last = snapshots[-1]["status"]
                if last["queue_size"] <= baseline_status["queue_size"]:
                    break
        time.sleep(args.poll_interval)

    stop_event.set()
    thread.join(timeout=2)

    with lock:
        samples = list(snapshots)

    if not samples:
        print("No status samples captured; cannot compute throughput.")
        return 1

    start_ts = samples[0]["ts"]
    end_ts = samples[-1]["ts"]
    elapsed = max(end_ts - start_ts, 1e-6)
    final_status = samples[-1]["status"]

    admitted = final_status["txs_approved"] - baseline_status["txs_approved"]
    rejected = final_status["txs_rejected"] - baseline_status["txs_rejected"]
    admit_tps = admitted / elapsed
    submitted_total = sum(result.submitted for result in results)
    submit_tps = submitted_total / max(submit_elapsed, 1e-6)

    committed = 0
    last_block = baseline_status["blocks_non_empty"]
    first_block_ts = None
    last_block_ts = None
    for sample in samples:
        status = sample["status"]
        blocks = status["blocks_non_empty"]
        if blocks <= last_block:
            continue
        dag_vertices = sample["dag_vertices"]
        txs_in_block = (
            int(round(dag_vertices))
            if dag_vertices is not None
            else args.block_max
        )
        committed += txs_in_block * (blocks - last_block)
        last_block = blocks
        if first_block_ts is None:
            first_block_ts = sample["ts"]
        last_block_ts = sample["ts"]

    if first_block_ts is None or last_block_ts is None or last_block_ts <= first_block_ts:
        commit_window = elapsed
    else:
        commit_window = last_block_ts - first_block_ts
    commit_tps = committed / max(commit_window, 1e-6)

    total_rate_limits = sum(result.rate_limit_hits for result in results)
    total_timeouts = sum(1 for result in results if result.timed_out)

    print("Load summary")
    print(f"- submitted: {submitted_total} tx in {submit_elapsed:.2f}s ({submit_tps:.1f} tps)")
    print(f"- admitted: {admitted} tx (rejected: {rejected}) in {elapsed:.2f}s ({admit_tps:.1f} tps)")
    print(
        f"- committed estimate: {committed} tx in {commit_window:.2f}s ({commit_tps:.1f} tps)"
    )
    print(
        f"- blocks_non_empty delta: {final_status['blocks_non_empty'] - baseline_status['blocks_non_empty']}"
    )
    print(f"- queue_size: baseline {baseline_status['queue_size']} -> final {final_status['queue_size']}")
    print(f"- rate_limit_hits (status 429): {total_rate_limits}")
    if total_timeouts:
        print(f"- cli_timeouts: {total_timeouts} shard(s)")
    return 1 if submit_failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
