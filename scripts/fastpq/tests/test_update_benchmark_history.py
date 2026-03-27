"""Tests for the FASTPQ benchmark history generator."""

import json

from scripts.fastpq import update_benchmark_history


def test_format_operation_filter_prefers_explicit_filter():
    bench = {"operation_filter": "lde"}
    operations = {
        "fft": {"operation": "fft"},
        "lde": {"operation": "lde"},
    }

    result = update_benchmark_history.format_operation_filter(bench, operations)

    assert result == "lde"


def test_format_operation_filter_falls_back_to_single_operation():
    bench = {}
    operations = {
        "ifft": {"operation": "ifft"},
    }

    result = update_benchmark_history.format_operation_filter(bench, operations)

    assert result == "ifft"


def test_collect_benchmark_rows_carries_operation_filter(tmp_path):
    bundle = {
        "metadata": {
            "labels": {
                "device_class": "xeon-rtx",
                "gpu_model": "NVIDIA RTX 6000 Ada",
            }
        },
        "benchmarks": {
            "execution_mode": "gpu",
            "gpu_backend": "cuda",
            "gpu_available": True,
            "operation_filter": "lde",
            "operations": [
                {
                    "operation": "lde",
                    "cpu_mean_ms": 12.0,
                    "gpu_mean_ms": 6.0,
                    "speedup_ratio": 2.0,
                }
            ],
        },
    }
    path = tmp_path / "fastpq_cuda_bench_probe.json"
    path.write_text(json.dumps(bundle), encoding="utf-8")

    rows = update_benchmark_history.collect_benchmark_rows(tmp_path)

    assert len(rows) == 1
    row = rows[0]
    assert row.operation_filter == "lde"
    assert row.gpu_backend == "cuda"
    assert row.lde == "12.0/6.0/2.00"


def test_gpu_table_mentions_filter_column():
    row = update_benchmark_history.BenchmarkRow(
        bundle=update_benchmark_history.Path("fastpq_cuda_bench_probe.json"),
        backend="cuda",
        execution_mode="gpu",
        gpu_backend="cuda",
        gpu_available="yes",
        operation_filter="lde",
        device_class="xeon-rtx",
        gpu_model="NVIDIA RTX 6000 Ada",
        lde="12.0/6.0/2.00",
        poseidon="—/—/—",
    )

    table = update_benchmark_history.gpu_table([row])

    assert "| Bundle | Backend | Mode | GPU backend | GPU available | Filter |" in table
    assert "| `fastpq_cuda_bench_probe.json` | cuda | gpu | cuda | yes | lde |" in table


def test_poseidon_table_mentions_filter_and_columns(tmp_path):
    manifest = {
        "entries": [
            {
                "file": "benchmarks/poseidon/poseidon_microbench_cuda.json",
                "bundle": "fastpq_cuda_bench_poseidon.json",
                "capture_timestamp": "2026-03-27T12:00:00Z",
                "operation_filter": "poseidon_hash_columns",
                "column_count": 16,
                "default_mean_ms": 1.0,
                "scalar_mean_ms": 2.0,
                "speedup_vs_scalar": 2.0,
            }
        ]
    }
    path = tmp_path / "manifest.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")

    table = update_benchmark_history.poseidon_table(path)

    assert "| Summary | Bundle | Timestamp | Filter | Columns | Default ms | Scalar ms | Speedup |" in table
    assert "| `benchmarks/poseidon/poseidon_microbench_cuda.json` | `fastpq_cuda_bench_poseidon.json` | 2026-03-27T12:00:00Z | poseidon_hash_columns | 16 | 1.0 | 2.0 | 2.00 |" in table


def test_poseidon_table_falls_back_to_all_and_blank_columns(tmp_path):
    manifest = {
        "entries": [
            {
                "file": "benchmarks/poseidon/poseidon_microbench_legacy.json",
                "bundle": "fastpq_metal_bench_legacy.json",
                "capture_timestamp": "2025-11-09T06:11:01Z",
                "default_mean_ms": 2.0,
                "scalar_mean_ms": 3.0,
                "speedup_vs_scalar": 1.5,
            }
        ]
    }
    path = tmp_path / "manifest.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")

    table = update_benchmark_history.poseidon_table(path)

    assert "| `benchmarks/poseidon/poseidon_microbench_legacy.json` | `fastpq_metal_bench_legacy.json` | 2025-11-09T06:11:01Z | all | — | 2.0 | 3.0 | 1.50 |" in table
