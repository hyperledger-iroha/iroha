"""Tests for the FASTPQ benchmark wrapper summaries."""

import json

import pytest

from scripts.fastpq import wrap_benchmark


def current_cuda_payload():
    operation = {
        "operation": "lde",
        "columns": 2,
        "input_len": 8,
        "output_len": 64,
        "input_bytes": 128,
        "output_bytes": 1024,
        "estimated_gpu_transfer_bytes": 1152,
    }
    return {
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
                    **operation,
                    "cpu_mean_ms": 0.2,
                    "gpu_mean_ms": 0.1,
                    "speedup_ratio": 2.0,
                    "speedup_delta_ms": 0.1,
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
            "operation_filter": "lde",
            "operations": [
                {
                    **operation,
                    "cpu": {"mean_ms": 0.2},
                    "gpu": {"mean_ms": 0.1},
                    "speedup": {"ratio": 2.0, "delta_ms": 0.1},
                }
            ],
            "metadata": {"generated_at": "2026-03-27T00:00:00Z"},
        },
    }


def current_metal_report():
    return {
        "rows": 8,
        "padded_rows": 8,
        "iterations": 1,
        "warmups": 0,
        "column_count": 2,
        "execution_mode": "cpu",
        "gpu_backend": "none",
        "gpu_available": False,
        "operation_filter": "fft",
        "operations": [
            {
                "operation": "fft",
                "columns": 2,
                "input_len": 8,
                "gpu_recorded": False,
                "cpu": {"mean_ms": 0.2, "min_ms": 0.1, "max_ms": 0.3},
            }
        ],
    }


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

    operations, hotspots = wrap_benchmark.summarize_operations(
        capture, wrap_benchmark.CUDA_NESTED_SCHEMA
    )

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


@pytest.mark.parametrize(
    "missing_field",
    (
        "output_len",
        "input_bytes",
        "output_bytes",
        "estimated_gpu_transfer_bytes",
        "cpu",
    ),
)
def test_summarize_operations_rejects_incomplete_cuda_entries(missing_field):
    capture = {
        "operations": [
            {
                "operation": "fft",
                "columns": 16,
                "input_len": 32_768,
                "output_len": 32_768,
                "input_bytes": 4_194_304,
                "output_bytes": 4_194_304,
                "estimated_gpu_transfer_bytes": 8_388_608,
                "gpu": {"mean_ms": 10.0},
                "cpu": {"mean_ms": 20.0},
                "speedup": {"ratio": 2.0, "delta_ms": 10.0},
            }
        ]
    }
    del capture["operations"][0][missing_field]

    with pytest.raises(SystemExit, match=missing_field):
        wrap_benchmark.summarize_operations(
            capture, wrap_benchmark.CUDA_NESTED_SCHEMA
        )


def test_summarize_operations_accepts_current_metal_shape_without_cuda_fields():
    capture = {
        "operations": [
            {
                "operation": "fft",
                "columns": 16,
                "input_len": 32_768,
                "gpu_recorded": True,
                "gpu": {"mean_ms": 10.0, "min_ms": 9.0, "max_ms": 11.0},
                "cpu": {"mean_ms": 20.0, "min_ms": 19.0, "max_ms": 21.0},
                "speedup": {"ratio": 2.0, "delta_ms": 10.0},
            }
        ]
    }

    operations, _ = wrap_benchmark.summarize_operations(
        capture, wrap_benchmark.METAL_FLAT_SCHEMA
    )

    assert operations == [
        {
            "operation": "fft",
            "columns": 16,
            "input_len": 32_768,
            "cpu_mean_ms": 20.0,
            "gpu_mean_ms": 10.0,
            "speedup_ratio": 2.0,
            "speedup_delta_ms": 10.0,
        }
    ]


def test_summarize_operations_rejects_incomplete_metal_entry():
    capture = {
        "operations": [
            {
                "operation": "fft",
                "columns": 16,
                "input_len": 32_768,
                "cpu": {"mean_ms": 20.0, "min_ms": 19.0, "max_ms": 21.0},
            }
        ]
    }

    with pytest.raises(SystemExit, match="gpu_recorded"):
        wrap_benchmark.summarize_operations(
            capture, wrap_benchmark.METAL_FLAT_SCHEMA
        )


def test_summarize_row_usage_snapshot_rejects_incomplete_v1_counts(tmp_path):
    snapshot = {
        "fastpq_batches": [
            {
                "entry_hash": "batch-a",
                "row_usage": {
                    "total_rows": 4,
                    "transfer_rows": 3,
                    "non_transfer_rows": 1,
                    "transfer_ratio": 0.75,
                },
            }
        ]
    }
    snapshot_path = tmp_path / "row_usage.json"
    snapshot_path.write_text(json.dumps(snapshot), encoding="utf-8")

    with pytest.raises(SystemExit, match="meta_set_rows missing"):
        wrap_benchmark.summarize_row_usage_snapshot(snapshot_path)


def test_normalize_report_accepts_nested_fastpq_cuda_bundle():
    payload = current_cuda_payload()

    report = wrap_benchmark.normalize_report(payload)

    assert report["column_count"] == 2
    assert report["operation_filter"] == "lde"
    assert report["metadata"]["platform"] == "linux"
    assert report["metadata"]["command"] == "target/debug/fastpq_cuda_bench --operation lde"
    assert report["operations"][0]["columns"] == 2
    assert report["operations"][0]["estimated_gpu_transfer_bytes"] == 1152
    assert report["operations"][0]["operation"] == "lde"


def test_validate_report_header_accepts_current_producer_shapes():
    cuda_report = wrap_benchmark.normalize_report(current_cuda_payload())
    wrap_benchmark.validate_report_header(
        cuda_report, wrap_benchmark.CUDA_NESTED_SCHEMA
    )
    wrap_benchmark.validate_report_header(
        current_metal_report(), wrap_benchmark.METAL_FLAT_SCHEMA
    )


@pytest.mark.parametrize(
    ("field", "value", "message"),
    (
        ("rows", 0, "positive"),
        ("iterations", 0, "positive"),
        ("warmups", -1, "non-negative"),
        ("column_count", 0, "positive"),
        ("padded_rows", 12, "power of two"),
        ("padded_rows", 4, "at least `rows`"),
        ("padded_rows", 1 << 17, "V1 trace bound"),
    ),
)
def test_validate_report_header_rejects_invalid_counts_and_bounds(
    field, value, message
):
    report = current_metal_report()
    report[field] = value

    with pytest.raises(SystemExit, match=message):
        wrap_benchmark.validate_report_header(
            report, wrap_benchmark.METAL_FLAT_SCHEMA
        )


def test_validate_report_header_rejects_unknown_and_inconsistent_filters():
    report = current_metal_report()
    report["operation_filter"] = "legacy_fft"
    with pytest.raises(SystemExit, match="canonical V1 operation"):
        wrap_benchmark.validate_report_header(
            report, wrap_benchmark.METAL_FLAT_SCHEMA
        )

    report["operation_filter"] = "lde"
    with pytest.raises(SystemExit, match="exactly that operation"):
        wrap_benchmark.validate_report_header(
            report, wrap_benchmark.METAL_FLAT_SCHEMA
        )


def test_validate_report_header_rejects_duplicate_operations():
    report = current_metal_report()
    report["operation_filter"] = "all"
    report["operations"].append(dict(report["operations"][0]))

    with pytest.raises(SystemExit, match="duplicated"):
        wrap_benchmark.validate_report_header(
            report, wrap_benchmark.METAL_FLAT_SCHEMA
        )


@pytest.mark.parametrize(
    ("operations", "message"),
    (
        ([], "non-empty"),
        ([{"operation": "legacy_fft"}], "canonical V1 operation"),
    ),
)
def test_validate_report_header_rejects_missing_or_unknown_operations(
    operations, message
):
    report = current_metal_report()
    report["operation_filter"] = "all"
    report["operations"] = operations

    with pytest.raises(SystemExit, match=message):
        wrap_benchmark.validate_report_header(
            report, wrap_benchmark.METAL_FLAT_SCHEMA
        )


@pytest.mark.parametrize(
    ("field", "value", "message"),
    (
        ("gpu_available", True, "must match"),
        ("gpu_backend", "metal", "gpu_backend='none'"),
    ),
)
def test_validate_report_header_rejects_execution_backend_mismatch(
    field, value, message
):
    report = current_metal_report()
    report[field] = value

    with pytest.raises(SystemExit, match=message):
        wrap_benchmark.validate_report_header(
            report, wrap_benchmark.METAL_FLAT_SCHEMA
        )


def test_normalize_report_rejects_inconsistent_cuda_header_copies():
    payload = current_cuda_payload()
    payload["report"]["rows"] = 7

    with pytest.raises(SystemExit, match="disagrees on `rows`"):
        wrap_benchmark.normalize_report(payload)


def test_normalize_report_rejects_inconsistent_cuda_operation_copies():
    payload = current_cuda_payload()
    payload["report"]["operations"][0]["cpu"]["mean_ms"] = 0.3

    with pytest.raises(SystemExit, match="operation 0 differs"):
        wrap_benchmark.normalize_report(payload)


def test_producer_schema_uses_current_payload_shape():
    assert (
        wrap_benchmark.producer_schema_for_payload({"report": {}})
        == wrap_benchmark.CUDA_NESTED_SCHEMA
    )
    assert (
        wrap_benchmark.producer_schema_for_payload({"operations": []})
        == wrap_benchmark.METAL_FLAT_SCHEMA
    )


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
    payload = current_cuda_payload()
    warnings = ["bn254 fft gpu timing skipped: cudaError_t(1)"]
    payload["benchmarks"]["bn254_warnings"] = warnings
    payload["report"]["bn254_warnings"] = warnings

    report = wrap_benchmark.normalize_report(payload)

    assert report["bn254_warnings"] == warnings


def test_require_poseidon_telemetry_skips_non_metal_backends():
    report = {
        "gpu_backend": "cuda",
        "operations": [{"operation": "poseidon_hash_columns"}],
    }

    wrap_benchmark.require_poseidon_telemetry(report)


def test_filter_metric_samples_does_not_fall_back_to_another_device():
    samples = [{"labels": {"device_class": "other"}, "value": 1}]

    assert wrap_benchmark.filter_metric_samples(samples, "wanted") == []


def test_poseidon_metric_summary_rejects_another_device(tmp_path):
    metrics_path = tmp_path / "metrics.prom"
    metrics_path.write_text(
        'fastpq_poseidon_pipeline_total{requested="gpu",resolved="gpu",path="gpu",'
        'device_class="other"} 1\n',
        encoding="utf-8",
    )

    with pytest.raises(SystemExit, match="missing fastpq_poseidon_pipeline_total samples"):
        wrap_benchmark.build_poseidon_metric_summary(metrics_path, "wanted")
