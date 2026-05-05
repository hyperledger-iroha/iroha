"""Tests for the FASTPQ cross-device matrix builder."""

import json

from scripts.fastpq import capture_matrix


def test_format_operation_filter_prefers_explicit_value():
    benchmarks = {
        "operation_filter": "lde",
        "operations": [
            {"operation": "fft"},
            {"operation": "lde"},
        ],
    }

    result = capture_matrix.format_operation_filter(benchmarks)

    assert result == "lde"


def test_format_operation_filter_falls_back_to_multi_op_all():
    benchmarks = {
        "operations": [
            {"operation": "fft"},
            {"operation": "lde"},
        ],
    }

    result = capture_matrix.format_operation_filter(benchmarks)

    assert result == "all"


def test_summarize_device_records_operation_filters_in_manifest(tmp_path):
    fft_bundle = {
        "metadata": {"platform": "linux", "machine": "x86_64"},
        "benchmarks": {
            "gpu_backend": "cuda",
            "rows": 8,
            "padded_rows": 8,
            "operation_filter": "fft",
            "operations": [
                {
                    "operation": "fft",
                    "gpu_mean_ms": 1.0,
                    "speedup_ratio": 2.0,
                }
            ],
        },
    }
    lde_bundle = {
        "metadata": {"platform": "linux", "machine": "x86_64"},
        "benchmarks": {
            "gpu_backend": "cuda",
            "rows": 16,
            "padded_rows": 16,
            "operations": [
                {
                    "operation": "lde",
                    "gpu_mean_ms": 3.0,
                    "speedup_ratio": 1.5,
                }
            ],
        },
    }
    fft_path = tmp_path / "fft.json"
    lde_path = tmp_path / "lde.json"
    fft_path.write_text(json.dumps(fft_bundle), encoding="utf-8")
    lde_path.write_text(json.dumps(lde_bundle), encoding="utf-8")

    summary = capture_matrix.summarize_device("xeon-rtx", [fft_path, lde_path])
    entry = summary.to_manifest_entry(tmp_path, max_ms_slack=5.0, min_speedup_slack=5.0)

    assert summary.operation_filters == {"fft", "lde"}
    assert entry["operation_filters"] == ["fft", "lde"]
    assert entry["rows"] == {"min": 8, "max": 16, "padded": [8, 16]}
