"""Tests for reviewer-facing FASTPQ rollout manifest summaries."""

from __future__ import annotations

import json
from pathlib import Path

from scripts.fastpq import rollout_manifest_summary


def _write_bundle(root: Path, name: str, payload: dict) -> Path:
    path = root / name
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def test_build_rollout_summary_preserves_filter_scope_and_device_labels(tmp_path: Path) -> None:
    metal_bundle = {
        "metadata": {
            "host": "mac-lab",
            "platform": "macOS",
            "machine": "arm64",
            "command": "target/debug/fastpq_metal_bench --rows 20000",
            "labels": {
                "device_class": "apple-m4",
                "gpu_kind": "integrated",
                "gpu_model": "Apple M4 GPU",
            },
        },
        "benchmarks": {
            "rows": 20_000,
            "padded_rows": 32_768,
            "gpu_backend": "metal",
            "gpu_available": True,
            "operation_filter": "all",
            "column_count": 16,
            "operations": [
                {"operation": "fft"},
                {"operation": "lde"},
                {"operation": "poseidon_hash_columns"},
            ],
        },
    }
    cuda_bundle = {
        "metadata": {
            "host": "sm80-lab",
            "platform": "linux",
            "machine": "x86_64",
            "command": "target/debug/fastpq_cuda_bench --operation lde",
            "labels": {
                "device_class": "xeon-rtx-sm80",
                "gpu_kind": "discrete",
                "gpu_model": "NVIDIA A100",
            },
        },
        "benchmarks": {
            "rows": 20_000,
            "padded_rows": 32_768,
            "gpu_backend": "cuda",
            "gpu_available": True,
            "operation_filter": "lde",
            "column_count": 16,
            "operations": [
                {"operation": "lde"},
            ],
        },
    }

    _write_bundle(tmp_path, "fastpq_metal_bench_probe.json", metal_bundle)
    _write_bundle(tmp_path, "fastpq_cuda_bench_probe.json", cuda_bundle)
    manifest_path = _write_bundle(
        tmp_path,
        "fastpq_bench_manifest.json",
        {
            "payload": {
                "version": 1,
                "generated_unix_ms": 1_742_000_000_000,
                "generator_commit": "abc123",
                "constraints": {
                    "require_rows": 20_000,
                    "max_operation_ms": {"lde": 950.0},
                    "min_operation_speedup": {"fft": 1.0},
                },
                "benches": [
                    {
                        "label": "metal",
                        "path": "artifacts/fastpq_benchmarks/fastpq_metal_bench_probe.json",
                        "rows": 20_000,
                        "padded_rows": 32_768,
                        "iterations": 5,
                        "warmups": 1,
                        "operation_filter": "all",
                        "matrix_operation_filters": ["all", "poseidon_hash_columns"],
                        "gpu_backend": "metal",
                        "gpu_available": True,
                        "metadata": {"host": "mac-lab", "platform": "macOS", "machine": "arm64"},
                    },
                    {
                        "label": "cuda",
                        "path": "artifacts/fastpq_benchmarks/fastpq_cuda_bench_probe.json",
                        "rows": 20_000,
                        "padded_rows": 32_768,
                        "iterations": 5,
                        "warmups": 1,
                        "operation_filter": "lde",
                        "matrix_operation_filters": ["fft", "lde", "poseidon_hash_columns"],
                        "gpu_backend": "cuda",
                        "gpu_available": True,
                        "metadata": {"host": "sm80-lab", "platform": "linux", "machine": "x86_64"},
                    },
                ],
            },
            "signature": {
                "algorithm": "ed25519",
                "public_key_hex": "00",
                "signature_hex": "11",
            },
        },
    )

    summary = rollout_manifest_summary.build_rollout_summary(
        manifest_path,
        bundle_dir=tmp_path,
        repo_root=tmp_path,
    )

    assert summary["signature_present"] is True
    assert summary["bench_labels"] == ["metal", "cuda"]
    cuda_entry = next(bench for bench in summary["benches"] if bench["label"] == "cuda")
    assert cuda_entry["resolved_bench_path"] == "fastpq_cuda_bench_probe.json"
    assert cuda_entry["operation_filter"] == "lde"
    assert cuda_entry["matrix_operation_filters"] == ["fft", "lde", "poseidon_hash_columns"]
    assert cuda_entry["device_class"] == "xeon-rtx-sm80"
    assert cuda_entry["gpu_kind"] == "discrete"
    assert cuda_entry["available_operations"] == ["lde"]


def test_render_markdown_mentions_filters_and_constraints() -> None:
    summary = {
        "manifest": "fastpq_bench_manifest.json",
        "signature_present": True,
        "generator_commit": "abc123",
        "bench_labels": ["cuda"],
        "constraints": {
            "require_rows": 20_000,
            "max_operation_ms": {"lde": 950.0},
            "min_operation_speedup": {"fft": 1.0},
        },
        "benches": [
            {
                "label": "cuda",
                "gpu_backend": "cuda",
                "device_class": "xeon-rtx-sm80",
                "gpu_kind": "discrete",
                "gpu_model": "NVIDIA A100",
                "rows": 20_000,
                "padded_rows": 32_768,
                "operation_filter": "lde",
                "matrix_operation_filters": ["fft", "lde"],
                "available_operations": ["lde"],
                "column_count": 16,
                "resolved_bench_path": "fastpq_cuda_bench_probe.json",
                "source_command": "target/debug/fastpq_cuda_bench --operation lde",
            }
        ],
    }

    markdown = rollout_manifest_summary.render_markdown(summary)

    assert "Require rows: **20,000**" in markdown
    assert "Operation filter: `lde` (focused capture)" in markdown
    assert "Matrix filters: `fft`, `lde`" in markdown
    assert "Bench file: `fastpq_cuda_bench_probe.json`" in markdown
    assert "Command: `target/debug/fastpq_cuda_bench --operation lde`" in markdown


def test_resolve_bench_path_falls_back_to_bundle_basename(tmp_path: Path) -> None:
    bundle_dir = tmp_path / "release_bundle"
    bundle_dir.mkdir()
    bench_path = bundle_dir / "fastpq_cuda_bench_probe.json"
    bench_path.write_text("{}", encoding="utf-8")

    resolved = rollout_manifest_summary.resolve_bench_path(
        "artifacts/fastpq_benchmarks/fastpq_cuda_bench_probe.json",
        bundle_dir=bundle_dir,
        repo_root=tmp_path / "repo",
    )

    assert resolved == bench_path.resolve()
