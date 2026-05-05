"""Tests for the FASTPQ benchmark wrapper summaries."""

from scripts.fastpq import wrap_benchmark


def test_summarize_operations_preserves_shape_and_transfer_fields():
    capture = {
        "operations": [
            {
                "operation": "lde",
                "columns": 16,
                "input_len": 32_768,
                "output_len": 262_144,
                "input_bytes": 4_194_304,
                "output_bytes": 33_554_432,
                "estimated_gpu_transfer_bytes": 37_748_736,
                "gpu": {"mean_ms": 65.0},
                "cpu": {"mean_ms": 80.0},
                "speedup": {"ratio": 1.231, "delta_ms": 15.0},
            }
        ]
    }

    operations, hotspots = wrap_benchmark.summarize_operations(capture)

    assert hotspots == []
    assert len(operations) == 1
    operation = operations[0]
    assert operation["operation"] == "lde"
    assert operation["input_len"] == 32_768
    assert operation["output_len"] == 262_144
    assert operation["input_bytes"] == 4_194_304
    assert operation["output_bytes"] == 33_554_432
    assert operation["estimated_gpu_transfer_bytes"] == 37_748_736
    assert operation["gpu_mean_ms"] == 65.0


def test_summarize_operations_keeps_legacy_entries_compatible():
    capture = {
        "operations": [
            {
                "operation": "fft",
                "columns": 16,
                "input_len": 32_768,
                "gpu": {"mean_ms": 10.0},
                "cpu": {"mean_ms": 20.0},
                "speedup": {"ratio": 2.0, "delta_ms": 10.0},
            }
        ]
    }

    operations, _ = wrap_benchmark.summarize_operations(capture)

    assert len(operations) == 1
    operation = operations[0]
    assert operation["operation"] == "fft"
    assert operation["input_len"] == 32_768
    assert operation["output_len"] is None
    assert operation["estimated_gpu_transfer_bytes"] is None


def test_normalize_report_accepts_nested_fastpq_cuda_bundle():
    payload = {
        "metadata": {
            "generated_at": "2026-03-27T00:00:00Z",
            "platform": "linux",
            "machine": "x86_64",
            "command": "target/debug/fastpq_cuda_bench --operation lde",
        },
        "benchmarks": {
            "rows": 8,
            "padded_rows": 8,
            "iterations": 1,
            "warmups": 0,
            "column_count": 2,
            "execution_mode": "gpu",
            "gpu_backend": "cuda",
            "gpu_available": True,
            "operation_filter": "lde",
            "operations": [
                {
                    "operation": "lde",
                    "columns": 2,
                    "input_len": 8,
                    "output_len": 64,
                    "input_bytes": 128,
                    "output_bytes": 1024,
                    "estimated_gpu_transfer_bytes": 1152,
                }
            ],
        },
        "report": {
            "rows": 8,
            "padded_rows": 8,
            "iterations": 1,
            "warmups": 0,
            "execution_mode": "gpu",
            "gpu_backend": "cuda",
            "gpu_available": True,
            "operations": [
                {
                    "operation": "lde",
                    "input_len": 8,
                    "output_len": 64,
                    "cpu": {"mean_ms": 0.2},
                    "gpu": {"mean_ms": 0.1},
                }
            ],
        },
    }

    report = wrap_benchmark.normalize_report(payload)

    assert report["column_count"] == 2
    assert report["operation_filter"] == "lde"
    assert report["metadata"]["platform"] == "linux"
    assert report["metadata"]["command"] == "target/debug/fastpq_cuda_bench --operation lde"
    assert report["operations"][0]["columns"] == 2
    assert report["operations"][0]["estimated_gpu_transfer_bytes"] == 1152
    assert report["operations"][0]["operation"] == "lde"


def test_normalize_report_keeps_flat_payloads_unchanged():
    payload = {
        "rows": 8,
        "iterations": 1,
        "operations": [{"operation": "fft"}],
        "operation_filter": "fft",
    }

    report = wrap_benchmark.normalize_report(payload)

    assert report is payload
    assert report["operation_filter"] == "fft"


def test_normalize_report_preserves_bn254_warnings_from_nested_bundle():
    payload = {
        "benchmarks": {
            "operation_filter": "fft",
            "bn254_warnings": ["bn254 fft gpu timing skipped: cudaError_t(1)"],
        },
        "report": {
            "operations": [],
        },
    }

    report = wrap_benchmark.normalize_report(payload)

    assert report["bn254_warnings"] == ["bn254 fft gpu timing skipped: cudaError_t(1)"]


def test_require_poseidon_telemetry_skips_non_metal_backends():
    report = {
        "gpu_backend": "cuda",
        "operations": [{"operation": "poseidon_hash_columns"}],
    }

    wrap_benchmark.require_poseidon_telemetry(report)
