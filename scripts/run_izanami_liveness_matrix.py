#!/usr/bin/env python3
"""Run Izanami 20k liveness matrix rows and summarize block cadence."""

from __future__ import annotations

import argparse
import csv
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")
SUMMARY_RE = re.compile(r"(\w+)=([^\s]+)")
STATUS_FIELD_RE = re.compile(r"(\w+):\s*([^,\)\s]+)")
OPTION_RE = re.compile(r"Some\((\d+)\)")
# Native `SumeragiStatus` fields reported from Izanami's final status digest.
STATUS_REPORT_FIELDS = ("committed_height_advance", "view", "level", "halted")


@dataclass(frozen=True)
class MatrixRow:
    name: str
    block_cap: int
    pipeline_ms: int
    latency_threshold_s: int = 3


DEFAULT_ROWS = [
    MatrixRow("cap1024_pipe300", 1024, 300),
    MatrixRow("cap1280_pipe300", 1280, 300),
    MatrixRow("cap1536_pipe300", 1536, 300),
    MatrixRow("cap1536_pipe400", 1536, 400),
    MatrixRow("cap2048_pipe400", 2048, 400),
]


def is_admitted_committee_size(peers: int) -> bool:
    """Return whether ``peers`` is an admitted bounded ``3f + 1`` roster."""

    return 4 <= peers <= 31 and (peers - 1) % 3 == 0


def parse_rows(value: str | None) -> list[MatrixRow]:
    if not value:
        return DEFAULT_ROWS
    rows: list[MatrixRow] = []
    for raw in value.split(","):
        parts = raw.split(":")
        if len(parts) != 3:
            raise ValueError(
                "native matrix rows must be name:cap:pipeline_ms"
            )
        name, cap, pipeline = parts
        rows.append(MatrixRow(name, int(cap), int(pipeline)))
    return rows


def parse_runner_summary(log_path: Path) -> dict[str, str]:
    summary: dict[str, str] = {}
    for raw in log_path.read_text(errors="ignore").splitlines():
        line = ANSI_RE.sub("", raw)
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
    """Parse an integer field, accepting Izanami's `Some(N)` rendering of an option."""

    text = str(value)
    match = OPTION_RE.fullmatch(text)
    if match:
        text = match.group(1)
    try:
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
    final_txs = integer(summary.get("final_strict_min_txs_approved", ""))
    committed_tps = "" if final_txs is None else f"{final_txs / args.duration:.2f}"
    if exit_code is None:
        parsed_exit_code = integer(summary.get("_summary_exit_code", ""))
        exit_code = parsed_exit_code if parsed_exit_code is not None else 1
    strict_interval_p95_ms = integer(
        summary.get("final_strict_block_interval_p95_ms", "")
    )
    quorum_interval_p95_ms = integer(
        summary.get("final_quorum_block_interval_p95_ms", "")
    )
    strict_interval_pass = (
        strict_interval_p95_ms is not None
        and strict_interval_p95_ms <= args.strict_interval_p95_threshold_ms
    )
    row_pass = exit_code == 0 and strict_interval_pass
    return {
        "name": row.name,
        "exit_code": exit_code,
        "row_pass": row_pass,
        "duration_s": args.duration,
        "block_cap": row.block_cap,
        "pipeline_ms": row.pipeline_ms,
        "latency_threshold_s": row.latency_threshold_s,
        "progress_interval_s": args.progress_interval_s,
        "strict_interval_p95_threshold_ms": args.strict_interval_p95_threshold_ms,
        "strict_interval_pass": strict_interval_pass,
        "offered": summary.get("offered", ""),
        "ingress_accepted": summary.get("ingress_accepted", ""),
        "failures": summary.get("failures", ""),
        "submit_latency_p95_ms": summary.get("submit_latency_p95_ms", ""),
        "final_strict_min_height": summary.get("final_strict_min_height", ""),
        "final_strict_min_txs_approved": summary.get("final_strict_min_txs_approved", ""),
        "committed_tps": committed_tps,
        "runner_quorum_interval_p95_ms": ""
        if quorum_interval_p95_ms is None
        else quorum_interval_p95_ms,
        "runner_strict_interval_p95_ms": ""
        if strict_interval_p95_ms is None
        else strict_interval_p95_ms,
        **{field: summary.get(field, "") for field in STATUS_REPORT_FIELDS},
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
        "--diagnostic-dir",
        str(run_dir),
    ]
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
            "| row | pass | exit | cap | pipeline | accepted | strict height | "
            "approved | committed TPS | runner p95 | committed advance | view | "
            "level | halted |\n"
        )
        handle.write("| " + " | ".join(["---"] * 14) + " |\n")
        for row in rows:
            runner_p95 = row["runner_strict_interval_p95_ms"]
            runner_p95_text = "" if runner_p95 == "" else f"{runner_p95}ms"
            handle.write(
                f"| {row['name']} | {row['row_pass']} | {row['exit_code']} | {row['block_cap']} | "
                f"{row['pipeline_ms']} | "
                f"{row['ingress_accepted']} | {row['final_strict_min_height']} | "
                f"{row['final_strict_min_txs_approved']} | {row['committed_tps']} | "
                f"{runner_p95_text} | {row['committed_height_advance']} | {row['view']} | "
                f"{row['level']} | {row['halted']} |\n"
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
        help="Comma-separated rows: name:cap:pipeline_ms",
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
        "--strict-interval-p95-threshold-ms",
        type=int,
        default=3_000,
        help=(
            "Fail matrix rows whose Izanami strict block interval p95 (every peer's "
            "committed height) exceeds this value."
        ),
    )
    parser.add_argument(
        "--summarize-existing",
        action="store_true",
        help="Rebuild summary files from an existing output root without rerunning rows.",
    )
    args = parser.parse_args()
    if not is_admitted_committee_size(args.peers):
        parser.error("--peers must be an exact Sumeragi 3f+1 committee in 4..=31")
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
            f"strict_interval_p95_ms={result['runner_strict_interval_p95_ms']}",
            flush=True,
        )
    write_outputs(results, args.output_root)
    return 0 if all(row["row_pass"] for row in results) else 1


if __name__ == "__main__":
    sys.exit(main())
