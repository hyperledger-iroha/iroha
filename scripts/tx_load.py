#!/usr/bin/env python3
"""Submit transaction pings in parallel and estimate TPS from /status + /metrics."""

from __future__ import annotations

import argparse
import json
import subprocess
import threading
import time
import urllib.request
from typing import Optional


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


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Drive transaction ping load via iroha CLI and estimate throughput from /status and "
            "/metrics."
        )
    )
    parser.add_argument("--iroha-bin", default="target/release/iroha")
    parser.add_argument("--client-config", required=True, help="Path to client.toml")
    parser.add_argument("--status-url", default="http://127.0.0.1:8080/status")
    parser.add_argument("--metrics-url", default="http://127.0.0.1:8080/metrics")
    parser.add_argument("--count", type=int, default=10_000)
    parser.add_argument("--parallel", type=int, default=64)
    parser.add_argument("--msg", default="load-ping")
    parser.add_argument("--no-wait", dest="no_wait", action="store_true", default=True)
    parser.add_argument("--wait", dest="no_wait", action="store_false")
    parser.add_argument("--no-index", dest="no_index", action="store_true", default=True)
    parser.add_argument("--with-index", dest="no_index", action="store_false")
    parser.add_argument("--poll-interval", type=float, default=0.25)
    parser.add_argument("--drain-timeout", type=float, default=120.0)
    parser.add_argument("--block-max", type=int, default=10_000)
    parser.add_argument("--log-level", default="INFO")
    args = parser.parse_args()

    if args.count <= 0:
        parser.error("--count must be greater than zero")
    if args.parallel <= 0:
        parser.error("--parallel must be greater than zero")

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

    ping_cmd = [
        args.iroha_bin,
        "--config",
        args.client_config,
        "transaction",
        "ping",
        "--msg",
        args.msg,
        "--log-level",
        args.log_level,
        "--count",
        str(args.count),
        "--parallel",
        str(args.parallel),
    ]
    if args.no_wait:
        ping_cmd.append("--no-wait")
    if args.no_index:
        ping_cmd.append("--no-index")

    submit_start = time.monotonic()
    result = subprocess.run(ping_cmd, check=False)
    submit_elapsed = time.monotonic() - submit_start
    if result.returncode != 0:
        stop_event.set()
        thread.join(timeout=2)
        print("load submission failed")
        return result.returncode

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
    submit_tps = args.count / max(submit_elapsed, 1e-6)

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

    print("Load summary")
    print(f"- submitted: {args.count} tx in {submit_elapsed:.2f}s ({submit_tps:.1f} tps)")
    print(f"- admitted: {admitted} tx (rejected: {rejected}) in {elapsed:.2f}s ({admit_tps:.1f} tps)")
    print(
        f"- committed estimate: {committed} tx in {commit_window:.2f}s ({commit_tps:.1f} tps)"
    )
    print(
        f"- blocks_non_empty delta: {final_status['blocks_non_empty'] - baseline_status['blocks_non_empty']}"
    )
    print(f"- queue_size: baseline {baseline_status['queue_size']} -> final {final_status['queue_size']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
