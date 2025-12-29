#!/usr/bin/env python3
"""Regenerate docs/source/benchmarks/history.md from FASTPQ artefacts."""
from __future__ import annotations

import argparse
import json
import textwrap
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


ARTIFACTS_DIR = Path("artifacts/fastpq_benchmarks")
POSEIDON_MANIFEST = Path("benchmarks/poseidon/manifest.json")
MERKLE_DIR = Path("benchmarks/merkle_threshold")
HISTORY_DOC = Path("docs/source/benchmarks/history.md")


@dataclass
class BenchmarkRow:
    bundle: Path
    backend: str
    execution_mode: str
    gpu_backend: str
    gpu_available: str
    device_class: str
    gpu_model: str
    lde: str
    poseidon: str


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
        "--poseidon-manifest",
        type=Path,
        default=POSEIDON_MANIFEST,
        help=f"Poseidon microbench manifest path (default: {POSEIDON_MANIFEST})",
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
        metadata = data.get("metadata") or {}
        labels = metadata.get("labels") or {}
        backend = path.name.split("_")[1] if "_" in path.name else labels.get("backend", "—")
        bench = data.get("benchmarks") or {}
        execution_mode = str(bench.get("execution_mode") or "—")
        gpu_backend = str(bench.get("gpu_backend") or labels.get("backend") or "—")
        gpu_available = bench.get("gpu_available")
        gpu_available_str = "—"
        if isinstance(gpu_available, bool):
            gpu_available_str = "yes" if gpu_available else "no"
        device_class = labels.get("device_class", "—")
        gpu_model = labels.get("gpu_model", labels.get("chip_type", "—"))
        operations: dict[str, dict[str, Any]] = {}
        raw_ops = (data.get("benchmarks") or {}).get("operations", [])
        if isinstance(raw_ops, list):
            for entry in raw_ops:
                if not isinstance(entry, dict):
                    continue
                op_name = entry.get("operation")
                if not isinstance(op_name, str):
                    continue
                operations[op_name] = entry
        lde_row = operations.get("lde")
        poseidon_row = operations.get("poseidon_hash_columns")
        lde = format_operation(lde_row)
        poseidon = format_operation(poseidon_row)
        rows.append(
            BenchmarkRow(
                bundle=path,
                backend=backend,
                execution_mode=execution_mode,
                gpu_backend=gpu_backend,
                gpu_available=gpu_available_str,
                device_class=device_class,
                gpu_model=gpu_model,
                lde=lde,
                poseidon=poseidon,
            )
        )
    return rows


def format_operation(operation: dict[str, Any] | None) -> str:
    if not isinstance(operation, dict):
        return "—/—/—"
    cpu = fmt_ms(operation.get("cpu_mean_ms"))
    gpu = fmt_ms(operation.get("gpu_mean_ms"))
    speedup = fmt_speedup(operation.get("speedup_ratio"))
    return f"{cpu}/{gpu}/{speedup}"


def poseidon_table(manifest_path: Path) -> str:
    if not manifest_path.exists():
        return "_No Poseidon microbench manifest found; run `python3 scripts/fastpq/aggregate_poseidon_microbench.py` first._"
    manifest = json.loads(manifest_path.read_text())
    entries = manifest.get("entries") or []
    if not isinstance(entries, list) or not entries:
        return "_Poseidon manifest is empty; export microbench captures before regenerating history._"
    header = "| Summary | Bundle | Timestamp | Default ms | Scalar ms | Speedup |\n"
    header += "|---------|--------|-----------|------------|-----------|---------|\n"
    rows = []
    for entry in entries:
        default_ms = fmt_ms(entry.get("default_mean_ms"))
        scalar_ms = fmt_ms(entry.get("scalar_mean_ms"))
        speedup = fmt_speedup(entry.get("speedup_vs_scalar"))
        rows.append(
            f"| `{entry.get('file')}` | `{entry.get('bundle')}` | "
            f"{entry.get('capture_timestamp')} | {default_ms} | {scalar_ms} | {speedup} |"
        )
    return header + "\n".join(rows)


def gpu_table(rows: Iterable[BenchmarkRow]) -> str:
    header = (
        "| Bundle | Backend | Mode | GPU backend | GPU available | Device class | GPU | "
        "LDE ms (CPU/GPU/SU) | Poseidon ms (CPU/GPU/SU) |\n"
    )
    header += (
        "|-------|---------|------|-------------|---------------|--------------|-----|"
        "----------------------|---------------------------|\n"
    )
    body = []
    for row in rows:
        body.append(
            f"| `{row.bundle.name}` | {row.backend} | {row.execution_mode} | {row.gpu_backend} | "
            f"{row.gpu_available} | {row.device_class} | {row.gpu_model} | {row.lde} | {row.poseidon} |"
        )
    return (
        header + "\n".join(body)
        if body
        else header + "| _No wrapped benchmarks found_ |  |  |  |  |  |  |  |  |"
    )


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
    poseidon_section: str,
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
        It satisfies the FASTPQ Stage 7 WP5-B deliverable by tracking every wrapped GPU
        benchmark artefact, the Poseidon microbench manifest, and auxiliary sweeps under
        `benchmarks/`. Update the underlying captures and rerun the script whenever a new
        bundle lands or telemetry needs fresh evidence.

        ## Scope and Update Process

        - Produce or wrap new GPU captures (via `scripts/fastpq/wrap_benchmark.py`),
          append them to the capture matrix, and rerun this generator to refresh the
          tables.
        - When Poseidon microbench data is present, export it with
          `scripts/fastpq/export_poseidon_microbench.py` and rebuild the manifest using
          `scripts/fastpq/aggregate_poseidon_microbench.py`.
        - Record Merkle threshold sweeps by storing their JSON outputs under
          `benchmarks/merkle_threshold/`; this generator lists the known files so audits
          can cross-reference CPU vs GPU availability.

        ## FASTPQ Stage 7 GPU Benchmarks

        """
    )
    poseidon_intro = textwrap.dedent(
        """\
        ## Poseidon Microbench Snapshots

        `benchmarks/poseidon/manifest.json` aggregates the default-vs-scalar Poseidon
        microbench runs exported from each Metal bundle. The table below is refreshed by
        the generator script, so CI and governance reviews can diff historical speedups
        without unpacking the wrapped FASTPQ reports.

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
    document = (
        intro
        + gpu_section
        + "\n\n"
        + poseidon_intro
        + poseidon_section
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
    rows = collect_benchmark_rows(args.artifacts)
    poseidon_section = poseidon_table(args.poseidon_manifest)
    merkle_notes = merkle_section(args.merkle_dir)
    row_usage_notes = row_usage_section(args.artifacts)
    document = render_document(rows, poseidon_section, merkle_notes, row_usage_notes)
    args.history.parent.mkdir(parents=True, exist_ok=True)
    args.history.write_text(document)
    print(f"Updated {args.history}")


if __name__ == "__main__":
    main()
