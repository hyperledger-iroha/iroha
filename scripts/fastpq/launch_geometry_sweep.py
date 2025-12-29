#!/usr/bin/env python3
"""
Launch geometry sweep helper for FASTPQ Metal benchmarks.

This utility runs ``fastpq_metal_bench`` across a set of environment variable
overrides (FFT/LDE column batches, queue fan-out, Poseidon pipeline knobs) and
captures the resulting JSON artefacts alongside per-run stdout/stderr logs. It
also classifies each run (stable vs unstable) using configurable heuristics and
emits a CSV matrix so performance engineers can sort and compare launch shapes.
Host metadata (labels, hostname, platform, device tags) is embedded in both the
summary JSON and CSV matrix to make cross-device comparisons deterministic, and
each run records start/end timestamps so Stage7 evidence bundles can reconstruct
capture windows across machines. GPU execution is required by default; use
``--allow-cpu-fallback`` when a sweep is intentionally limited to CPU-only hosts.
"""

from __future__ import annotations

import argparse
import copy
import csv
import datetime
import importlib.util
import itertools
import json
import os
import pathlib
import platform
import shlex
import socket
import subprocess
import sys
import time
from typing import Any, Dict, Iterable, List, Optional, Sequence, Tuple

from scripts.fastpq import geometry_matrix


SCRIPT_DIR = pathlib.Path(__file__).resolve().parent
WRAP_BENCHMARK_PATH = SCRIPT_DIR / "wrap_benchmark.py"


def _load_wrap_helpers():
    if not WRAP_BENCHMARK_PATH.exists():
        return None
    spec = importlib.util.spec_from_file_location(
        "fastpq_wrap_benchmark", WRAP_BENCHMARK_PATH
    )
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)  # type: ignore[union-attr]
    return module


WRAP_HELPERS = _load_wrap_helpers()


def _detect_device_labels() -> dict[str, Any] | None:
    if WRAP_HELPERS is None:
        return None
    detector = getattr(WRAP_HELPERS, "detect_device_labels", None)
    if detector is None:
        return None
    try:
        labels = detector()
    except Exception:
        return None
    if not isinstance(labels, dict):
        return None
    return labels


def _now_iso() -> str:
    """Return a UTC timestamp formatted for evidence captures."""

    return (
        datetime.datetime.now(datetime.timezone.utc)
        .isoformat(timespec="seconds")
        .replace("+00:00", "Z")
    )


def collect_host_metadata(host_label: str | None = None) -> dict[str, Any]:
    metadata: dict[str, Any] = {
        "host": socket.gethostname(),
        "platform": platform.platform(),
        "machine": platform.machine(),
    }
    if host_label:
        trimmed = host_label.strip()
        if trimmed:
            metadata["label"] = trimmed
    labels = _detect_device_labels()
    if labels:
        metadata["labels"] = labels
    return metadata


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Sweep FASTPQ Metal launch geometry env overrides."
    )
    parser.add_argument(
        "--fft-columns",
        default="auto",
        help="Comma-separated FASTPQ_METAL_FFT_COLUMNS values (1-32 or 'auto').",
    )
    parser.add_argument(
        "--lde-columns",
        default="auto",
        help="Comma-separated FASTPQ_METAL_LDE_COLUMNS values (1-32 or 'auto').",
    )
    parser.add_argument(
        "--queue-fanout",
        default="auto",
        help="Comma-separated FASTPQ_METAL_QUEUE_FANOUT values (1-4 or 'auto').",
    )
    parser.add_argument(
        "--poseidon-columns",
        default="auto",
        help="Comma-separated FASTPQ_POSEIDON_PIPE_COLUMNS values (16-256 or 'auto').",
    )
    parser.add_argument(
        "--poseidon-lanes",
        default="auto",
        help="Comma-separated FASTPQ_METAL_POSEIDON_LANES values (32-256 or 'auto').",
    )
    parser.add_argument(
        "--poseidon-batch",
        default="auto",
        help="Comma-separated FASTPQ_METAL_POSEIDON_BATCH values (1-32 or 'auto').",
    )
    parser.add_argument(
        "--rows",
        type=int,
        default=20000,
        help="Number of rows supplied to fastpq_metal_bench (default: 20000).",
    )
    parser.add_argument(
        "--warmups",
        type=int,
        default=1,
        help="Benchmark warm-up iterations (default: 1).",
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=5,
        help="Benchmark measurement iterations (default: 5).",
    )
    parser.add_argument(
        "--bench-prefix",
        default="cargo run -p fastpq_prover --bin fastpq_metal_bench --features fastpq-gpu --",
        help="Command prefix used to invoke fastpq_metal_bench (default runs via cargo). "
        "The script appends --rows/--warmups/--iterations/--output automatically.",
    )
    parser.add_argument(
        "--extra-args",
        default="",
        help="Additional arguments appended to the fastpq_metal_bench invocation.",
    )
    parser.add_argument(
        "--operation",
        choices=["fft", "ifft", "lde", "poseidon_hash_columns", "all"],
        default="all",
        help="Limit the benchmark to a specific operation (default: all).",
    )
    parser.add_argument(
        "--artifact-dir",
        default=None,
        help="Directory for artefacts (default: artifacts/fastpq_geometry/<UTC timestamp>).",
    )
    parser.add_argument(
        "--summary",
        default=None,
        help="Summary JSON path (default: <artifact_dir>/summary.json).",
    )
    parser.add_argument(
        "--matrix-out",
        default=None,
        help="Optional CSV matrix path (default: <artifact_dir>/matrix.csv).",
    )
    parser.add_argument(
        "--reason-summary-out",
        default=None,
        help="Optional JSON histogram path for instability reasons (default: <artifact_dir>/reason_summary.json).",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=int,
        default=0,
        help="Optional timeout for each benchmark run (default: no timeout).",
    )
    parser.add_argument(
        "--halt-on-error",
        action="store_true",
        help="Stop immediately when a run fails instead of continuing.",
    )
    parser.add_argument(
        "--host-label",
        default=None,
        help="Optional alias recorded in the summary metadata (defaults to the hostname).",
    )
    parser.add_argument(
        "--min-total-speedup",
        type=float,
        default=1.0,
        help="Minimum CPU/GPU total speedup ratio required for a run to be marked stable "
        "(set to 0 to disable).",
    )
    parser.add_argument(
        "--min-queue-busy",
        type=float,
        default=0.05,
        help="Minimum metal_dispatch_queue busy_ratio required for a stable run "
        "(set to 0 to disable).",
    )
    parser.add_argument(
        "--min-dispatch-count",
        type=int,
        default=1,
        help="Minimum dispatch_count required for a stable run (set to 0 to disable).",
    )
    parser.add_argument(
        "--allow-cpu-fallback",
        action="store_true",
        help="Do not pass --require-gpu to fastpq_metal_bench (default: require GPU and fail when unavailable).",
    )
    return parser


def _parse_value_list(
    parser: argparse.ArgumentParser,
    raw_value: str,
    *,
    minimum: int,
    maximum: int,
) -> List[Optional[int]]:
    parsed_values: List[Optional[int]] = []
    for token in raw_value.split(","):
        stripped = token.strip()
        if not stripped:
            continue
        lowered = stripped.lower()
        if lowered in ("auto", "default"):
            parsed_values.append(None)
            continue
        try:
            parsed = int(stripped, 0)
        except ValueError as exc:  # pragma: no cover - argparse handles user IO.
            parser.error(f"override '{stripped}' is invalid: {exc}")
        if parsed < minimum or parsed > maximum:
            parser.error(
                f"override '{stripped}' outside supported range [{minimum}, {maximum}]"
            )
        parsed_values.append(parsed)
    if not parsed_values:
        parsed_values = [None]
    return parsed_values


def _format_threshold(value: float) -> str:
    formatted = f"{value:.3f}"
    return formatted.rstrip("0").rstrip(".")


def _is_number(value: Any) -> bool:
    return isinstance(value, (int, float))


def _compute_totals(operations: Dict[str, Dict[str, Any]]) -> Dict[str, Optional[float]]:
    total_gpu = 0.0
    total_cpu = 0.0
    counted = 0
    for stats in operations.values():
        gpu_value = stats.get("gpu_mean_ms")
        cpu_value = stats.get("cpu_mean_ms")
        if not (_is_number(gpu_value) and _is_number(cpu_value)):
            continue
        total_gpu += float(gpu_value)
        total_cpu += float(cpu_value)
        counted += 1
    if counted == 0 or total_gpu <= 0 or total_cpu <= 0:
        return {"gpu_ms": None, "cpu_ms": None, "speedup_ratio": None}
    total_speedup = total_cpu / total_gpu
    return {
        "gpu_ms": round(total_gpu, 3),
        "cpu_ms": round(total_cpu, 3),
        "speedup_ratio": round(total_speedup, 3),
    }


def _classify_entry(
    entry: Dict[str, Any],
    *,
    min_total_speedup: float,
    min_queue_busy: float,
    min_dispatch_count: int,
) -> Tuple[Dict[str, Any], Dict[str, Optional[float]]]:
    reasons: List[str] = []
    if entry.get("gpu_available") is False:
        reasons.append("gpu_unavailable")
    backend = entry.get("gpu_backend")
    if isinstance(backend, str) and backend.lower() != "metal":
        reasons.append(f"backend={backend}")
    run_status = entry.get("run_status")
    if isinstance(run_status, dict):
        state = run_status.get("state")
        if isinstance(state, str) and state.lower() != "ok":
            reasons.append(f"run_status={state}")

    status = entry.get("status")
    if status != "ok":
        reasons.append(f"status:{status or 'unknown'}")

    operations = entry.get("operations") or {}
    if not operations:
        reasons.append("missing_operations")
    totals = _compute_totals(operations)
    speedup = totals.get("speedup_ratio")
    if min_total_speedup > 0:
        if speedup is None:
            reasons.append("missing_total_speedup")
        elif speedup < min_total_speedup:
            reasons.append(f"total_speedup<{_format_threshold(min_total_speedup)}")

    queue_stats = entry.get("metal_dispatch_queue")
    if queue_stats is None:
        reasons.append("missing_queue_stats")
        busy_ratio = None
        dispatch_count = None
    else:
        busy_ratio = queue_stats.get("busy_ratio")
        dispatch_count = queue_stats.get("dispatch_count")
        if dispatch_count is None and isinstance(run_status, dict):
            dispatch_count = run_status.get("dispatch_count")

    if min_queue_busy > 0:
        if not _is_number(busy_ratio):
            reasons.append("missing_queue_busy")
        elif float(busy_ratio) < min_queue_busy:
            reasons.append(f"queue_busy<{_format_threshold(min_queue_busy)}")

    if min_dispatch_count > 0:
        if not _is_number(dispatch_count):
            reasons.append("missing_dispatch_count")
        elif int(dispatch_count) < min_dispatch_count:
            reasons.append("dispatch_count_below_threshold")

    classification = {"stable": not reasons, "reasons": reasons}
    return classification, totals


def _write_matrix(
    entries: Iterable[Dict[str, Any]],
    matrix_path: pathlib.Path,
) -> None:
    entries = list(entries)
    headers = [
        "index",
        "label",
        "status",
        "execution_mode",
        "gpu_backend",
        "operation",
        "host_label",
        "host_name",
        "host_machine",
        "host_platform",
        "started_at",
        "completed_at",
        "duration_seconds",
        "host_labels",
        "env_fft",
        "env_lde",
        "env_fanout",
        "env_pipe",
        "env_lanes",
        "env_batch",
        "total_gpu_ms",
        "total_cpu_ms",
        "total_speedup",
        "queue_busy_ratio",
        "queue_overlap_ratio",
        "queue_dispatch_count",
        "stable",
        "classification_reasons",
    ]
    matrix_path.parent.mkdir(parents=True, exist_ok=True)
    total_entries = len(entries)
    stable_entries = 0
    with matrix_path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle)
        writer.writerow(headers)
        for entry in entries:
            env = entry.get("env") or {}
            queue = entry.get("metal_dispatch_queue") or {}
            totals = entry.get("totals") or {}
            classification = entry.get("classification") or {}
            host = entry.get("host") if isinstance(entry.get("host"), dict) else {}
            host_labels = host.get("labels") if isinstance(host.get("labels"), dict) else None
            host_labels_json = json.dumps(host_labels, sort_keys=True) if host_labels else ""
            host_label = ""
            if isinstance(host, dict):
                for key in ("label", "host"):
                    value = host.get(key)
                    if isinstance(value, str) and value.strip():
                        host_label = value.strip()
                        break
            if classification.get("stable"):
                stable_entries += 1
            writer.writerow(
                [
                    entry.get("index"),
                    entry.get("label"),
                    entry.get("status"),
                    entry.get("execution_mode"),
                    entry.get("gpu_backend"),
                    entry.get("operation"),
                    host_label,
                    host.get("host") if isinstance(host, dict) else None,
                    host.get("machine") if isinstance(host, dict) else None,
                    host.get("platform") if isinstance(host, dict) else None,
                    entry.get("started_at"),
                    entry.get("completed_at"),
                    entry.get("duration_seconds"),
                    host_labels_json,
                    env.get("FASTPQ_METAL_FFT_COLUMNS"),
                    env.get("FASTPQ_METAL_LDE_COLUMNS"),
                    env.get("FASTPQ_METAL_QUEUE_FANOUT"),
                    env.get("FASTPQ_POSEIDON_PIPE_COLUMNS"),
                    env.get("FASTPQ_METAL_POSEIDON_LANES"),
                    env.get("FASTPQ_METAL_POSEIDON_BATCH"),
                    totals.get("gpu_ms"),
                    totals.get("cpu_ms"),
                    totals.get("speedup_ratio"),
                    queue.get("busy_ratio"),
                    queue.get("overlap_ratio"),
                    queue.get("dispatch_count"),
                    classification.get("stable"),
                    ";".join(classification.get("reasons", [])),
                ]
            )
        # Summary line: stable/total ratio for quick scan.
        summary_row = ["" for _ in headers]
        summary_row[1] = "summary"
        stable_idx = headers.index("stable")
        summary_row[stable_idx] = f"{stable_entries}/{total_entries}"
        writer.writerow(summary_row)


def _write_reason_summary(entries: Iterable[Dict[str, Any]], reason_path: pathlib.Path) -> None:
    """Persist aggregated instability reasons/warnings for Stage7-2 evidence."""

    matrix_entries = geometry_matrix.build_matrix_entries(entries)
    reasons = geometry_matrix.build_reason_summary(matrix_entries)
    reason_path.parent.mkdir(parents=True, exist_ok=True)
    reason_path.write_text(json.dumps(reasons, indent=2), encoding="utf-8")


def _reset_env_values(env_template: Dict[str, str], overrides: Dict[str, Optional[int]]) -> Dict[str, str]:
    run_env = env_template.copy()
    for key, value in overrides.items():
        if value is None:
            run_env.pop(key, None)
        else:
            run_env[key] = str(value)
    return run_env


def run(args: argparse.Namespace, parser: argparse.ArgumentParser) -> int:
    if args.rows <= 0 or args.warmups <= 0 or args.iterations <= 0:
        parser.error("--rows/--warmups/--iterations must be positive integers")
    if args.min_total_speedup < 0:
        parser.error("--min-total-speedup must be >= 0")
    if args.min_queue_busy < 0:
        parser.error("--min-queue-busy must be >= 0")
    if args.min_queue_busy > 1:
        parser.error("--min-queue-busy cannot exceed 1.0")
    if args.min_dispatch_count < 0:
        parser.error("--min-dispatch-count must be >= 0")

    timestamp = datetime.datetime.utcnow().strftime("%Y%m%dT%H%M%SZ")
    if args.artifact_dir:
        artifact_root = pathlib.Path(args.artifact_dir)
    else:
        artifact_root = pathlib.Path("artifacts") / "fastpq_geometry" / timestamp
    artifact_root.mkdir(parents=True, exist_ok=True)

    summary_path = pathlib.Path(args.summary) if args.summary else artifact_root / "summary.json"
    matrix_path = pathlib.Path(args.matrix_out) if args.matrix_out else artifact_root / "matrix.csv"
    reason_summary_path = (
        pathlib.Path(args.reason_summary_out)
        if args.reason_summary_out
        else artifact_root / "reason_summary.json"
    )

    prefix_tokens = shlex.split(args.bench_prefix)
    if not prefix_tokens:
        parser.error("--bench-prefix must contain a command")
    extra_args = shlex.split(args.extra_args)
    timeout_value = args.timeout_seconds if args.timeout_seconds > 0 else None

    spec_inputs = [
        ("fft_columns", args.fft_columns, "FASTPQ_METAL_FFT_COLUMNS", "fft", 1, 32),
        ("lde_columns", args.lde_columns, "FASTPQ_METAL_LDE_COLUMNS", "lde", 1, 32),
        ("queue_fanout", args.queue_fanout, "FASTPQ_METAL_QUEUE_FANOUT", "fanout", 1, 4),
        ("poseidon_columns", args.poseidon_columns, "FASTPQ_POSEIDON_PIPE_COLUMNS", "pipe", 16, 256),
        ("poseidon_lanes", args.poseidon_lanes, "FASTPQ_METAL_POSEIDON_LANES", "lanes", 32, 256),
        ("poseidon_batch", args.poseidon_batch, "FASTPQ_METAL_POSEIDON_BATCH", "batch", 1, 32),
    ]

    specs = []
    for name, raw_value, env_name, label, minimum, maximum in spec_inputs:
        parsed_values = _parse_value_list(
            parser,
            raw_value,
            minimum=minimum,
            maximum=maximum,
        )
        specs.append(
            {
                "name": name,
                "env": env_name,
                "label": label,
                "values": parsed_values,
            }
        )

    combinations = list(itertools.product(*[spec["values"] for spec in specs]))
    if not combinations:
        parser.error("no sweep combinations were generated")

    summary_entries: List[Dict[str, Any]] = []
    total_runs = len(combinations)
    env_template = os.environ.copy()
    require_gpu = not args.allow_cpu_fallback
    host_metadata = collect_host_metadata(args.host_label)

    for index, combination in enumerate(combinations, start=1):
        env_overrides = dict(zip([spec["env"] for spec in specs], combination))
        label_parts = []
        for spec, value in zip(specs, combination):
            suffix = str(value) if value is not None else "auto"
            label_parts.append(f"{spec['label']}{suffix}")
        label = "_".join(label_parts)
        run_slug = f"{index:03d}_{label}"
        output_path = artifact_root / f"{run_slug}.json"
        stdout_path = artifact_root / f"{run_slug}.stdout"
        stderr_path = artifact_root / f"{run_slug}.stderr"

        run_env = _reset_env_values(env_template, env_overrides)
        cmd = list(prefix_tokens)
        cmd.extend(
            [
                "--rows",
                str(args.rows),
                "--warmups",
                str(args.warmups),
                "--iterations",
                str(args.iterations),
                "--output",
                str(output_path),
            ]
        )
        if require_gpu:
            cmd.append("--require-gpu")
        if args.operation:
            cmd.extend(["--operation", args.operation])
        if extra_args:
            cmd.extend(extra_args)

        print(f"[{index}/{total_runs}] running {run_slug}")
        started_at = _now_iso()
        started = time.time()
        try:
            result = subprocess.run(
                cmd,
                env=run_env,
                capture_output=True,
                text=True,
                timeout=timeout_value,
            )
            completed_at = _now_iso()
        except subprocess.TimeoutExpired as exc:
            completed_at = _now_iso()
            stdout_path.write_text(exc.stdout or "", encoding="utf-8")
            stderr_path.write_text(exc.stderr or "", encoding="utf-8")
            entry = {
                "index": index,
                "label": label,
                "status": "timeout",
                "duration_seconds": round(time.time() - started, 3),
                "started_at": started_at,
                "completed_at": completed_at,
                "env": {k: ("auto" if v is None else v) for k, v in env_overrides.items()},
                "command": " ".join(shlex.quote(part) for part in cmd),
                "stdout": str(stdout_path),
                "stderr": str(stderr_path),
                "rows": args.rows,
                "warmups": args.warmups,
                "iterations": args.iterations,
                "host": copy.deepcopy(host_metadata),
                "operation": args.operation,
                "require_gpu": require_gpu,
            }
            summary_entries.append(entry)
            print(f"  timeout after {args.timeout_seconds}s")
            if args.halt_on_error:
                break
            continue

        stdout_path.write_text(result.stdout or "", encoding="utf-8")
        stderr_path.write_text(result.stderr or "", encoding="utf-8")
        entry: Dict[str, Any] = {
            "index": index,
            "label": label,
            "status": "ok",
            "duration_seconds": round(time.time() - started, 3),
            "started_at": started_at,
            "completed_at": completed_at,
            "env": {k: ("auto" if v is None else v) for k, v in env_overrides.items()},
            "command": " ".join(shlex.quote(part) for part in cmd),
            "output": str(output_path),
            "stdout": str(stdout_path),
            "stderr": str(stderr_path),
            "rows": args.rows,
            "warmups": args.warmups,
            "iterations": args.iterations,
            "host": copy.deepcopy(host_metadata),
            "operation": args.operation,
            "require_gpu": require_gpu,
        }

        if result.returncode != 0:
            entry["status"] = "error"
            entry["returncode"] = result.returncode
            entry["completed_at"] = completed_at
            print(f"  failed with code {result.returncode}")
            summary_entries.append(entry)
            if args.halt_on_error:
                break
            continue
        if not output_path.exists():
            entry["status"] = "error"
            entry["error"] = "benchmark completed but JSON output missing"
            entry["completed_at"] = completed_at
            print("  missing JSON output")
            summary_entries.append(entry)
            if args.halt_on_error:
                break
            continue

        try:
            with output_path.open("r", encoding="utf-8") as handle:
                report_payload = json.load(handle)
        except json.JSONDecodeError as exc:
            entry["status"] = "error"
            entry["error"] = f"failed to parse JSON: {exc}"
            entry["completed_at"] = completed_at
            print("  invalid JSON payload")
            summary_entries.append(entry)
            if args.halt_on_error:
                break
            continue

        operations_block: Dict[str, Dict[str, Any]] = {}
        for operation in report_payload.get("operations", []):
            name = operation.get("operation")
            if not name:
                continue
            operations_block[name] = {
                "columns": operation.get("columns"),
                "gpu_mean_ms": operation.get("gpu", {}).get("mean_ms"),
                "cpu_mean_ms": operation.get("cpu", {}).get("mean_ms"),
                "speedup_ratio": operation.get("speedup", {}).get("ratio"),
            }
        entry["operations"] = operations_block
        entry["execution_mode"] = report_payload.get("execution_mode")
        entry["gpu_backend"] = report_payload.get("gpu_backend")
        entry["gpu_available"] = bool(report_payload.get("gpu_available"))
        run_status = report_payload.get("run_status")
        if isinstance(run_status, dict):
            entry["run_status"] = run_status

        queue_stats = report_payload.get("metal_dispatch_queue")
        poseidon_pipeline = None
        if isinstance(queue_stats, dict):
            entry["metal_dispatch_queue"] = {
                "fanout_limit": queue_stats.get("limit"),
                "busy_ratio": queue_stats.get("busy_ratio"),
                "overlap_ratio": queue_stats.get("overlap_ratio"),
                "dispatch_count": queue_stats.get("dispatch_count"),
            }
            poseidon_pipeline = queue_stats.get("poseidon_pipeline")

        fft_tuning = report_payload.get("fft_tuning")
        if isinstance(fft_tuning, dict):
            entry["fft_tuning"] = fft_tuning
        if poseidon_pipeline is None:
            poseidon_pipeline = report_payload.get("poseidon_pipeline")
        if isinstance(poseidon_pipeline, dict):
            entry["poseidon_pipeline"] = poseidon_pipeline

        warnings: List[str] = []
        if isinstance(run_status, dict):
            state = run_status.get("state")
            if isinstance(state, str) and state.lower() != "ok":
                warnings.append(f"run_status={state}")
        if entry["gpu_available"] and isinstance(queue_stats, dict):
            if queue_stats.get("dispatch_count", 0) == 0:
                warnings.append(
                    "metal_dispatch_queue.dispatch_count==0 (likely CPU fallback despite GPU mode)"
                )
        if isinstance(poseidon_pipeline, dict):
            fallbacks = poseidon_pipeline.get("fallbacks")
            if isinstance(fallbacks, int) and fallbacks > 0:
                warnings.append(
                    f"poseidon_pipeline recorded {fallbacks} fallback dispatch(es)"
                )
        if warnings:
            entry["warnings"] = warnings

        classification, totals = _classify_entry(
            entry,
            min_total_speedup=args.min_total_speedup,
            min_queue_busy=args.min_queue_busy,
            min_dispatch_count=args.min_dispatch_count,
        )
        entry["classification"] = classification
        entry["totals"] = totals
        entry["completed_at"] = completed_at
        summary_entries.append(entry)

    with summary_path.open("w", encoding="utf-8") as handle:
        json.dump(summary_entries, handle, indent=2)
    print(f"Wrote summary for {len(summary_entries)} run(s) to {summary_path}")

    _write_matrix(summary_entries, matrix_path)
    print(f"Wrote classification matrix to {matrix_path}")

    _write_reason_summary(summary_entries, reason_summary_path)
    print(f"Wrote reason summary to {reason_summary_path}")
    return 0


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return run(args, parser)


if __name__ == "__main__":
    raise SystemExit(main())
