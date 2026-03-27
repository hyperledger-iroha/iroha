"""Tests for the Poseidon microbench export and aggregation helpers."""

import json

from scripts.fastpq import aggregate_poseidon_microbench, export_poseidon_microbench


def test_build_payload_preserves_filter_and_command():
    bundle = {
        "metadata": {
            "generated_at": "2026-03-27T12:00:00Z",
            "command": "target/debug/fastpq_cuda_bench --operation poseidon_hash_columns",
        },
        "benchmarks": {
            "execution_mode": "gpu",
            "gpu_backend": "cuda",
            "operation_filter": "poseidon_hash_columns",
            "column_count": 16,
        },
    }
    summary = {
        "default": {"mean_ms": 1.0},
        "scalar_lane": {"mean_ms": 2.0},
        "speedup_vs_scalar": 2.0,
    }

    payload = export_poseidon_microbench.build_payload(
        export_poseidon_microbench.Path("artifacts/fastpq_cuda_bench_poseidon.json"),
        bundle,
        summary,
    )

    assert payload["operation_filter"] == "poseidon_hash_columns"
    assert payload["column_count"] == 16
    assert payload["source_command"] == "target/debug/fastpq_cuda_bench --operation poseidon_hash_columns"


def test_aggregate_summary_preserves_filter_and_command(tmp_path):
    payload = {
        "bundle": "artifacts/fastpq_cuda_bench_poseidon.json",
        "capture_timestamp": "2026-03-27T12:00:00Z",
        "execution_mode": "gpu",
        "gpu_backend": "cuda",
        "host": "lab-host",
        "operation_filter": "poseidon_hash_columns",
        "column_count": 16,
        "source_command": "target/debug/fastpq_cuda_bench --operation poseidon_hash_columns",
        "poseidon_microbench": {
            "default": {"mean_ms": 1.0},
            "scalar_lane": {"mean_ms": 2.0},
            "speedup_vs_scalar": 2.0,
        },
    }
    path = tmp_path / "poseidon_microbench_cuda.json"
    path.write_text(json.dumps(payload), encoding="utf-8")

    entry = aggregate_poseidon_microbench.summarize_entry(path)

    assert entry is not None
    assert entry["operation_filter"] == "poseidon_hash_columns"
    assert entry["column_count"] == 16
    assert entry["source_command"] == "target/debug/fastpq_cuda_bench --operation poseidon_hash_columns"
