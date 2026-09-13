#!/usr/bin/env python3
"""Regenerate specs/benchmarks/history.md from FASTPQ artefacts."""
from __future__ import annotations

import argparse
import json
import textwrap
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


ARTIFACTS_DIR = Path("artifacts/fastpq_benchmarks")
MERKLE_DIR = Path("benchmarks/merkle_threshold")
HISTORY_DOC = Path("specs/benchmarks/history.md")


try:
    from .benchmark_operations import OPERATION_LABELS, require_filter
    from .report_projection import project_bundle, render_evidence, validate_projection
except ImportError:  # Direct script invocation.
    from benchmark_operations import OPERATION_LABELS, require_filter
    from report_projection import project_bundle, render_evidence, validate_projection


@dataclass
class BenchmarkRow:
    bundle: Path
    backend: str
    execution_mode: str
    gpu_backend: str
    gpu_available: str
    operation_filter: str
    device_class: str
    gpu_model: str
    operation_evidence: dict[str, Any]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--history",
        type=Path,
        default=HISTORY_DOC,
        help=f"Destination history document (default: {HISTORY_DOC})",
    )
    parser.add_argument(
        "--artifacts",
        type=Path,
        default=ARTIFACTS_DIR,
        help=f"Directory containing wrapped fastpq_*_bench_*.json files (default: {ARTIFACTS_DIR})",
    )
    parser.add_argument(
        "--merkle-dir",
        type=Path,
        default=MERKLE_DIR,
        help=f"Directory containing merkle_threshold JSON captures (default: {MERKLE_DIR})",
    )
    return parser.parse_args()


def fmt_ms(value: Any) -> str:
    if isinstance(value, (int, float)):
        return f"{value:.1f}"
    return "—"


def fmt_speedup(value: Any) -> str:
    if isinstance(value, (int, float)):
        return f"{value:.2f}"
    return "—"


def collect_benchmark_rows(artifacts_dir: Path) -> list[BenchmarkRow]:
    rows: list[BenchmarkRow] = []
    if not artifacts_dir.exists():
        return rows
    bundle_paths = sorted(artifacts_dir.glob("fastpq_*_bench_*.json"))
    for path in bundle_paths:
        data = json.loads(path.read_text())
        evidence = project_bundle(data, require_wrapped=True)
        metadata = data.get("metadata") or {}
        labels = metadata.get("labels") or {}
        backend = "cuda" if evidence["producer_schema"] == "cuda_nested" else "metal"
        bench = data.get("benchmarks") or {}
        execution_mode = str(bench.get("execution_mode") or "—")
        gpu_backend = str(bench.get("gpu_backend") or labels.get("backend") or "—")
        gpu_available = bench.get("gpu_available")
        gpu_available_str = "—"
        if isinstance(gpu_available, bool):
            gpu_available_str = "yes" if gpu_available else "no"
        device_class = labels.get("device_class", "—")
        gpu_model = labels.get("gpu_model", labels.get("chip_type", "—"))
        operation_filter = format_operation_filter(bench)
        rows.append(
            BenchmarkRow(
                bundle=path,
                backend=backend,
                execution_mode=execution_mode,
                gpu_backend=gpu_backend,
                gpu_available=gpu_available_str,
                operation_filter=operation_filter,
                device_class=device_class,
                gpu_model=gpu_model,
                operation_evidence=evidence,
            )
        )
    return rows


def format_operation_filter(bench: dict[str, Any]) -> str:
    return require_filter(bench.get("operation_filter"))


def format_operation(operation: dict[str, Any] | None) -> str:
    if not isinstance(operation, dict):
        return "—/—/—"
    cpu = fmt_ms(operation.get("cpu_mean_ms"))
    gpu = fmt_ms(operation.get("gpu_mean_ms"))
    speedup = fmt_speedup(operation.get("speedup_ratio"))
    return f"{cpu}/{gpu}/{speedup}"


def gpu_table(rows: Iterable[BenchmarkRow]) -> str:
    columns = list(OPERATION_LABELS)
    header = "| Bundle | Backend | Mode | GPU backend | GPU available | Filter | Device class | GPU | "
    header += " | ".join(f"{OPERATION_LABELS[name]} ms (CPU/GPU/SU)" for name in columns) + " |\n"
    header += "|" + "---|" * (8 + len(columns)) + "\n"
    body = []
    for row in rows:
        validated = validate_projection(row.operation_evidence)
        if not validated["flattened"]:
            raise ValueError("history timings require wrapped flattened operations")
        if require_filter(row.operation_filter) != validated["report"]["operation_filter"]:
            raise ValueError("history filter disagrees with retained measurement evidence")
        measured = validated["report"]
        expected_backend = "cuda" if validated["producer_schema"] == "cuda_nested" else "metal"
        if (row.backend, row.execution_mode, row.gpu_backend, row.gpu_available) != (
            expected_backend, measured["execution_mode"], measured["gpu_backend"],
            "yes" if measured["gpu_available"] else "no",
        ):
            raise ValueError("history execution context disagrees with retained measurement evidence")
        operations = {entry["operation"]: entry for entry in validated["report"]["operations"]}
        timings = " | ".join(format_operation(operations.get(name)) for name in columns)
        body.append(
            f"| `{row.bundle.name}` | {row.backend} | {row.execution_mode} | {row.gpu_backend} | "
            f"{row.gpu_available} | {row.operation_filter} | {row.device_class} | {row.gpu_model} | {timings} |"
        )
    if not body:
        body.append("| _No wrapped benchmarks found_ |" + " |" * (7 + len(columns)))
    return header + "\n".join(body)


def operation_evidence_section(rows: Iterable[BenchmarkRow]) -> str:
    sections = ["## Measured operation evidence", ""]
    for row in rows:
        sections.extend([f"### `{row.bundle.name}`", "", render_evidence(row.operation_evidence), ""])
    return "\n".join(sections)


def merkle_section(merkle_dir: Path) -> str:
    if not merkle_dir.exists():
        return (
            "No merkle threshold captures found. Run "
            "`cargo run --release -p ivm --features metal --example merkle_threshold -- --json` "
            "and store the output under `benchmarks/merkle_threshold/`."
        )
    rows = []
    for path in sorted(merkle_dir.glob("*.json")):
        data = json.loads(path.read_text())
        metal_available = data.get("metal_available")
        rows.append(f"- `{path}` — `metal_available={metal_available}`")
    if not rows:
        return (
            "No merkle threshold captures found. Run "
            "`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`."
        )
    return "\n".join(rows)


def row_usage_section(artifacts_dir: Path) -> str:
    files = sorted(artifacts_dir.glob("fastpq_row_usage_*.json"))
    if not files:
        return (
            "No row-usage evidence captured. Record witness decodes with "
            "`scripts/fastpq/check_row_usage.py --out artifacts/fastpq_benchmarks/fastpq_row_usage_<date>.json` "
            "before regenerating this history."
        )
    entries = []
    for path in files:
        try:
            data = json.loads(path.read_text())
        except json.JSONDecodeError:
            entries.append(f"- `{path}` — _invalid JSON_")
            continue
        batches = data.get("fastpq_batches")
        if not isinstance(batches, list) or not batches:
            entries.append(f"- `{path}` — _no batches present_")
            continue
        ratios: list[float] = []
        for batch in batches:
            usage = batch.get("row_usage") if isinstance(batch, dict) else None
            ratio = usage.get("transfer_ratio") if isinstance(usage, dict) else None
            if isinstance(ratio, (int, float)):
                ratios.append(float(ratio))
        if ratios:
            avg = sum(ratios) / len(ratios)
            entries.append(
                f"- `{path}` — batches={len(batches)}, transfer_ratio avg={avg:.3f} "
                f"(min={min(ratios):.3f}, max={max(ratios):.3f})"
            )
        else:
            entries.append(f"- `{path}` — batches={len(batches)}, transfer_ratio missing")
    return "\n".join(entries)


def render_document(
    rows: list[BenchmarkRow],
    merkle_notes: str,
    row_usage_notes: str,
) -> str:
    intro = textwrap.dedent(
        """\
        <!--
          SPDX-License-Identifier: Apache-2.0
        -->

        # GPU Benchmark Capture History (FASTPQ WP5-B)

        This file is generated by `python3 scripts/fastpq/update_benchmark_history.py`.
        It lists current-schema wrapped GPU benchmarks, complete measured operation
        evidence, and auxiliary sweeps found at the supplied paths. Release benchmark and
        row-usage bundles are generated evidence rather than checked-in fixtures; an empty
        table means no current input was supplied.

        ## Scope and Update Process

        - Produce or wrap new GPU captures (via `scripts/fastpq/wrap_benchmark.py`),
          append them to the capture matrix, and rerun this generator to refresh the
          tables.
        - Record Merkle threshold sweeps by storing their JSON outputs under
          `benchmarks/merkle_threshold/`; this generator lists the known files so audits
          can cross-reference CPU vs GPU availability.

        ## FASTPQ Stage 7 GPU Benchmarks

        """
    )
    merkle_intro = textwrap.dedent(
        """\
        ## Merkle Threshold Sweeps

        Reference captures gathered via
        `cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
        live under `benchmarks/merkle_threshold/`. List entries show whether the host
        exposed Metal devices when the sweep ran; GPU-enabled captures should report
        `metal_available=true`.

        """
    )
    row_usage_intro = textwrap.dedent(
        """\
        ## Row-Usage Snapshots

        Witness decodes captured via `scripts/fastpq/check_row_usage.py` prove the transfer
        gadget’s row efficiency. Keep the JSON artefacts under `artifacts/fastpq_benchmarks/`
        and this generator will summarise the recorded transfer ratios for auditors.

        """
    )
    gpu_section = gpu_table(rows)
    gpu_note = textwrap.dedent(
        """\
        > Columns: `Backend` is the explicit producer schema; `Mode`/`GPU backend`/`GPU available`
        > come from the wrapped `benchmarks` block to record the resolved execution mode and GPU discovery.
        > `Filter` records the selected operation filter (`all` for full bundles, otherwise the
        > focused stage name); current wrapped bundles must provide it explicitly.
        > SU = speedup ratio (CPU/GPU).

        """
    )
    document = (
        intro
        + gpu_section
        + "\n\n"
        + gpu_note
        + operation_evidence_section(rows)
        + "\n\n"
        + merkle_intro
        + merkle_notes
        + "\n\n"
        + row_usage_intro
        + row_usage_notes
        + "\n"
    )
    return document


def main() -> None:
    args = parse_args()
    try:
        rows = collect_benchmark_rows(args.artifacts)
        merkle_notes = merkle_section(args.merkle_dir)
        row_usage_notes = row_usage_section(args.artifacts)
    except ValueError as err:
        raise SystemExit(f"[error] {err}") from err
    document = render_document(rows, merkle_notes, row_usage_notes)
    args.history.parent.mkdir(parents=True, exist_ok=True)
    args.history.write_text(document)
    print(f"Updated {args.history}")


if __name__ == "__main__":
    main()
