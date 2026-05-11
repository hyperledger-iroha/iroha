#!/usr/bin/env python3
"""Run Izanami 20k liveness matrix rows and summarize block cadence."""

from __future__ import annotations

import argparse
import csv
import os
import re
import statistics
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path


ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")
TS_RE = re.compile(r"\s*(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+)Z")
HEIGHT_RE = re.compile(r"height:\s*(\d+)")
MAX_TX_RE = re.compile(r"max_tx_param:\s*(\d+)")
SUMMARY_RE = re.compile(r"(\w+)=([^\s]+)")
STATUS_FIELD_RE = re.compile(r"(\w+):\s*([^,\)\s]+)")
TIME_FMT = "%Y-%m-%dT%H:%M:%S.%f"


@dataclass(frozen=True)
class MatrixRow:
    name: str
    block_cap: int
    scan_multiplier: int
    pipeline_ms: int
    collectors_k: int | None = None
    redundant_send_r: int | None = None
    inline_backup_rbc: bool | None = None
    latency_threshold_s: int = 3


DEFAULT_ROWS = [
    MatrixRow("cap1024_scan1_pipe300", 1024, 1, 300),
    MatrixRow("cap1280_scan1_pipe300", 1280, 1, 300),
    MatrixRow("cap1536_scan1_pipe300", 1536, 1, 300),
    MatrixRow("cap1536_scan2_pipe300", 1536, 2, 300),
    MatrixRow("cap1536_scan1_pipe400", 1536, 1, 400),
    MatrixRow("cap2048_scan1_pipe400", 2048, 1, 400),
]


def parse_rows(value: str | None) -> list[MatrixRow]:
    if not value:
        return DEFAULT_ROWS
    rows: list[MatrixRow] = []
    for raw in value.split(","):
        parts = raw.split(":")
        if len(parts) not in (4, 6, 7):
            raise ValueError(
                "matrix rows must be name:cap:scan:pipeline_ms or "
                "name:cap:scan:pipeline_ms:collectors_k:redundant_send_r or "
                "name:cap:scan:pipeline_ms:collectors_k:redundant_send_r:inline_backup_rbc"
            )
        name, cap, scan, pipeline = parts[:4]
        collectors_k = int(parts[4]) if len(parts) >= 6 else None
        redundant_send_r = int(parts[5]) if len(parts) >= 6 else None
        inline_backup_rbc = parse_bool(parts[6]) if len(parts) == 7 else None
        rows.append(
            MatrixRow(
                name,
                int(cap),
                int(scan),
                int(pipeline),
                collectors_k,
                redundant_send_r,
                inline_backup_rbc,
            )
        )
    return rows


def parse_bool(value: str) -> bool:
    normalized = value.lower()
    if normalized in {"1", "true", "yes", "on"}:
        return True
    if normalized in {"0", "false", "no", "off"}:
        return False
    raise ValueError(f"invalid boolean value: {value}")


def percentile(sorted_values: list[float], quantile: float) -> float:
    if not sorted_values:
        return 0.0
    rank = int(len(sorted_values) * quantile + 0.999999)
    return sorted_values[max(0, min(len(sorted_values) - 1, rank - 1))]


def parse_peer_gaps(run_dir: Path) -> dict[str, object]:
    gaps: list[float] = []
    max_tx: set[int] = set()
    targeted_payload_total = 0
    targeted_ready_total = 0
    deliver_rebroadcast_total = 0
    ready_quorum_deferral_total = 0
    for log in (run_dir / "test-network").glob("*/run-1-stdout.log"):
        commits: dict[int, datetime] = {}
        for raw in log.read_text(errors="ignore").splitlines():
            line = ANSI_RE.sub("", raw)
            if "sending targeted RBC payload to peers missing READY" in line:
                targeted_payload_total += 1
            if "sending targeted RBC READY set to ready-repair peers" in line:
                targeted_ready_total += 1
            if "rebroadcasting RBC DELIVER to commit topology after delivery" in line:
                deliver_rebroadcast_total += 1
            if "deferring RBC DELIVER: READY quorum not reached" in line:
                ready_quorum_deferral_total += 1
            if "proposal assembly budget" in line:
                match = MAX_TX_RE.search(line)
                if match:
                    max_tx.add(int(match.group(1)))
            ts_match = TS_RE.match(line)
            height_match = HEIGHT_RE.search(line)
            if (
                ts_match
                and height_match
                and "stored committed block to kura" in line
            ):
                commits[int(height_match.group(1))] = datetime.strptime(
                    ts_match.group(1), TIME_FMT
                )
        previous: datetime | None = None
        for height in sorted(commits):
            current = commits[height]
            if previous is not None:
                gaps.append((current - previous).total_seconds())
            previous = current
    values = sorted(gaps)
    return {
        "max_tx": ";".join(str(item) for item in sorted(max_tx)),
        "gap_samples": len(values),
        "gap_avg_s": statistics.mean(values) if values else 0.0,
        "gap_p50_s": percentile(values, 0.50),
        "gap_p95_s": percentile(values, 0.95),
        "gap_max_s": max(values) if values else 0.0,
        "gap_over_3s": sum(value > 3.0 for value in values),
        "rbc_targeted_payload_total": targeted_payload_total,
        "rbc_targeted_ready_total": targeted_ready_total,
        "rbc_deliver_rebroadcast_total": deliver_rebroadcast_total,
        "rbc_ready_quorum_deferral_total": ready_quorum_deferral_total,
    }


def parse_runner_summary(log_path: Path) -> dict[str, str]:
    summary: dict[str, str] = {}
    for raw in log_path.read_text(errors="ignore").splitlines():
        line = ANSI_RE.sub("", raw)
        if "sumeragi phase timing snapshot at target height" in line:
            for key, value in SUMMARY_RE.findall(line):
                summary[key] = value.strip(",")
            continue
        if (
            "target block height reached" in line
            or "strict block height advanced" in line
            or "block height advanced" in line
        ):
            fields = dict(SUMMARY_RE.findall(line))
            if strict_height := fields.get("strict_min_height"):
                summary["final_strict_min_height"] = strict_height.strip(",")
            if accepted := fields.get("ingress_accepted"):
                summary["ingress_accepted"] = accepted.strip(",")
            if offered := fields.get("offered"):
                summary["offered"] = offered.strip(",")
            if quorum_p95 := fields.get("interval_p95_ms"):
                summary["final_quorum_block_interval_p95_ms"] = quorum_p95.strip(",")
            if strict_p95 := fields.get("strict_interval_p95_ms"):
                summary["final_strict_block_interval_p95_ms"] = strict_p95.strip(",")
        if "izanami run complete" in line:
            summary["_summary_exit_code"] = "0"
        elif "izanami run finished with errors" in line:
            summary["_summary_exit_code"] = "1"
        else:
            continue
        for key, value in SUMMARY_RE.findall(line):
            summary[key] = value.strip(",")
        for key, value in STATUS_FIELD_RE.findall(line):
            summary.setdefault(key, value.strip(","))
    return summary


def integer(value: object) -> int | None:
    try:
        text = str(value)
        return int(text) if text else None
    except ValueError:
        return None


def collect_result(
    args: argparse.Namespace,
    row: MatrixRow,
    output_root: Path,
    exit_code: int | None,
) -> dict[str, object]:
    run_dir = output_root / row.name
    runner_log = run_dir / "runner.log"
    summary = parse_runner_summary(runner_log)
    gaps = parse_peer_gaps(run_dir)
    final_txs = integer(summary.get("final_strict_min_txs_approved", ""))
    committed_tps = "" if final_txs is None else f"{final_txs / args.duration:.2f}"
    if exit_code is None:
        parsed_exit_code = integer(summary.get("_summary_exit_code", ""))
        exit_code = parsed_exit_code if parsed_exit_code is not None else 1
    peer_gap_p95_pass = (
        int(gaps["gap_samples"]) > 0
        and float(gaps["gap_p95_s"]) <= args.peer_gap_p95_threshold_s
    )
    row_pass = exit_code == 0 and peer_gap_p95_pass
    return {
        "name": row.name,
        "exit_code": exit_code,
        "row_pass": row_pass,
        "duration_s": args.duration,
        "block_cap": row.block_cap,
        "scan_multiplier": row.scan_multiplier,
        "pipeline_ms": row.pipeline_ms,
        "collectors_k": "" if row.collectors_k is None else row.collectors_k,
        "redundant_send_r": "" if row.redundant_send_r is None else row.redundant_send_r,
        "inline_backup_rbc": "" if row.inline_backup_rbc is None else row.inline_backup_rbc,
        "latency_threshold_s": row.latency_threshold_s,
        "progress_interval_s": args.progress_interval_s,
        "peer_gap_p95_threshold_s": args.peer_gap_p95_threshold_s,
        "peer_gap_p95_pass": peer_gap_p95_pass,
        "offered": summary.get("offered", ""),
        "ingress_accepted": summary.get("ingress_accepted", ""),
        "failures": summary.get("failures", ""),
        "submit_latency_p95_ms": summary.get("submit_latency_p95_ms", ""),
        "final_strict_min_height": summary.get("final_strict_min_height", ""),
        "final_strict_min_txs_approved": summary.get("final_strict_min_txs_approved", ""),
        "committed_tps": committed_tps,
        "runner_quorum_interval_p95_ms": summary.get("final_quorum_block_interval_p95_ms", ""),
        "runner_strict_interval_p95_ms": summary.get("final_strict_block_interval_p95_ms", ""),
        "phase_collect_da_ms": summary.get("phase_collect_da_ms", ""),
        "phase_collect_precommit_ms": summary.get("phase_collect_precommit_ms", ""),
        "phase_pipeline_total_ms": summary.get("phase_pipeline_total_ms", ""),
        "phase_collect_da_max_ms": summary.get("phase_collect_da_max_ms", ""),
        "phase_collect_precommit_max_ms": summary.get("phase_collect_precommit_max_ms", ""),
        "phase_pipeline_total_max_ms": summary.get("phase_pipeline_total_max_ms", ""),
        "phase_pipeline_total_ema_ms": summary.get("phase_pipeline_total_ema_ms", ""),
        "pipeline_conflict_rate_bps": summary.get("pipeline_conflict_rate_bps", ""),
        "lane_tx_vertices_total": summary.get("lane_tx_vertices_total", ""),
        "lane_tx_edges_total": summary.get("lane_tx_edges_total", ""),
        "lane_overlay_count_total": summary.get("lane_overlay_count_total", ""),
        "lane_overlay_instr_total": summary.get("lane_overlay_instr_total", ""),
        "detached_prepared_total": summary.get("detached_prepared_total", ""),
        "detached_merged_total": summary.get("detached_merged_total", ""),
        "detached_fallback_total": summary.get("detached_fallback_total", ""),
        "view_change_install_total": summary.get("view_change_install_total", ""),
        "tx_queue_depth": summary.get("tx_queue_depth", ""),
        "tx_queue_saturated": summary.get("tx_queue_saturated", ""),
        "pacemaker_backpressure_deferrals_total": summary.get(
            "pacemaker_backpressure_deferrals_total", ""
        ),
        "missing_block_fetch_total": summary.get("missing_block_fetch_total", ""),
        "consensus_missing_qc_reacquire_attempt_total": summary.get(
            "consensus_missing_qc_reacquire_attempt_total", ""
        ),
        "blocksync_range_pull_escalation_total": summary.get(
            "blocksync_range_pull_escalation_total", ""
        ),
        **gaps,
    }


def run_row(args: argparse.Namespace, row: MatrixRow, output_root: Path) -> dict[str, object]:
    run_dir = output_root / row.name
    run_dir.mkdir(parents=True, exist_ok=True)
    runner_log = run_dir / "runner.log"
    env = os.environ.copy()
    env.update(
        {
            "TEST_NETWORK_BIN_IROHAD": str(args.irohad),
            "TEST_NETWORK_IROHAD_FEATURES": args.irohad_features,
            "IROHA_TEST_SKIP_BUILD": "1",
            "IROHA_TEST_NETWORK_KEEP_DIRS": "1",
            "RUST_LOG": args.rust_log,
        }
    )
    command = [
        str(args.izanami),
        "--allow-net",
        "--peers",
        str(args.peers),
        "--faulty",
        "0",
        "--duration",
        f"{args.duration}s",
        "--pipeline-time",
        f"{row.pipeline_ms}ms",
        "--latency-p95-threshold",
        f"{row.latency_threshold_s}s",
        "--progress-interval",
        f"{args.progress_interval_s}s",
        "--tps",
        str(args.tps),
        "--max-inflight",
        str(args.max_inflight),
        "--submitters",
        str(args.submitters),
        "--prebuild-tx-buffer",
        str(int(args.duration * args.tps)),
        "--prebuild-tx-workers",
        "0",
        "--workload-profile",
        "stable",
        "--sumeragi-block-max-transactions",
        str(row.block_cap),
        "--sumeragi-proposal-queue-scan-multiplier",
        str(row.scan_multiplier),
        "--diagnostic-dir",
        str(run_dir),
    ]
    if row.collectors_k is not None:
        command.extend(["--sumeragi-collectors-k", str(row.collectors_k)])
    if row.redundant_send_r is not None:
        command.extend(
            [
                "--sumeragi-collectors-redundant-send-r",
                str(row.redundant_send_r),
            ]
        )
    if row.inline_backup_rbc is not None:
        command.extend(
            [
                "--sumeragi-inline-block-created-backup-rbc",
                str(row.inline_backup_rbc).lower(),
            ]
        )
    with runner_log.open("w") as log:
        proc = subprocess.run(
            command,
            env=env,
            cwd=args.repo,
            stdout=log,
            stderr=subprocess.STDOUT,
            text=True,
            check=False,
        )
    return collect_result(args, row, output_root, proc.returncode)


def write_outputs(rows: list[dict[str, object]], output_root: Path) -> None:
    fieldnames = list(rows[0].keys()) if rows else []
    csv_path = output_root / "summary.csv"
    with csv_path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)
    md_path = output_root / "summary.md"
    with md_path.open("w") as handle:
        handle.write("# Izanami Liveness Matrix\n\n")
        handle.write(
            "| row | pass | exit | cap | scan | pipeline | k | r | backup RBC | accepted | strict height | "
            "approved | committed TPS | runner p95 | peer gap p95 | peer max | over 3s | DA ms | "
            "precommit ms | DA max | precommit max | pipeline max | conflict bps | detached merged | fallback | RBC payload | RBC READY | "
            "queue depth | view changes |\n"
        )
        handle.write(
            "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | "
            "---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n"
        )
        for row in rows:
            runner_p95 = row["runner_strict_interval_p95_ms"]
            runner_p95_text = "" if runner_p95 == "" else f"{runner_p95}ms"
            handle.write(
                f"| {row['name']} | {row['row_pass']} | {row['exit_code']} | {row['block_cap']} | "
                f"{row['scan_multiplier']} | {row['pipeline_ms']} | "
                f"{row['collectors_k']} | {row['redundant_send_r']} | "
                f"{row['inline_backup_rbc']} | "
                f"{row['ingress_accepted']} | {row['final_strict_min_height']} | "
                f"{row['final_strict_min_txs_approved']} | {row['committed_tps']} | "
                f"{runner_p95_text} | {float(row['gap_p95_s']):.3f}s | "
                f"{float(row['gap_max_s']):.3f}s | {row['gap_over_3s']} | "
                f"{row['phase_collect_da_ms']} | {row['phase_collect_precommit_ms']} | "
                f"{row['phase_collect_da_max_ms']} | {row['phase_collect_precommit_max_ms']} | "
                f"{row['phase_pipeline_total_max_ms']} | "
                f"{row['pipeline_conflict_rate_bps']} | {row['detached_merged_total']} | "
                f"{row['detached_fallback_total']} | "
                f"{row['rbc_targeted_payload_total']} | {row['rbc_targeted_ready_total']} | "
                f"{row['tx_queue_depth']} | {row['view_change_install_total']} |\n"
            )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--izanami", type=Path, default=Path("target/release/izanami"))
    parser.add_argument("--irohad", type=Path, default=Path("target/release/iroha3d"))
    parser.add_argument("--irohad-features", default="fastpq-gpu")
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument(
        "--rows",
        help=(
            "Comma-separated rows: name:cap:scan:pipeline_ms or "
            "name:cap:scan:pipeline_ms:collectors_k:redundant_send_r or "
            "name:cap:scan:pipeline_ms:collectors_k:redundant_send_r:inline_backup_rbc"
        ),
    )
    parser.add_argument("--duration", type=int, default=60)
    parser.add_argument("--tps", type=int, default=20_000)
    parser.add_argument("--peers", type=int, default=4)
    parser.add_argument("--max-inflight", type=int, default=300_000)
    parser.add_argument("--submitters", type=int, default=4096)
    parser.add_argument(
        "--progress-interval-s",
        type=int,
        default=5,
        help="Izanami block-height monitor interval; use a short interval for 2-3s block gates.",
    )
    parser.add_argument("--rust-log", default="info")
    parser.add_argument(
        "--peer-gap-p95-threshold-s",
        type=float,
        default=3.0,
        help="Fail matrix rows whose peer-observed committed block gap p95 exceeds this value.",
    )
    parser.add_argument(
        "--summarize-existing",
        action="store_true",
        help="Rebuild summary files from an existing output root without rerunning rows.",
    )
    args = parser.parse_args()
    args.repo = args.repo.resolve()
    args.izanami = (args.repo / args.izanami).resolve()
    args.irohad = (args.repo / args.irohad).resolve()
    args.output_root = (args.repo / args.output_root).resolve()
    args.output_root.mkdir(parents=True, exist_ok=True)

    results = []
    for row in parse_rows(args.rows):
        if args.summarize_existing:
            print(f"summarizing {row.name}", flush=True)
            result = collect_result(args, row, args.output_root, None)
        else:
            print(f"running {row.name}", flush=True)
            result = run_row(args, row, args.output_root)
        results.append(result)
        print(
            f"{row.name}: exit={result['exit_code']} "
            f"pass={result['row_pass']} "
            f"accepted={result['ingress_accepted']} "
            f"committed_tps={result['committed_tps']} "
            f"gap_p95={float(result['gap_p95_s']):.3f}s",
            flush=True,
        )
    write_outputs(results, args.output_root)
    return 0 if all(row["row_pass"] for row in results) else 1


if __name__ == "__main__":
    sys.exit(main())
