"""Tests for the FASTPQ stage timing aggregator."""

import json

from scripts.fastpq import aggregate_stage_timings


def test_normalize_report_accepts_wrapped_bundle_shape():
    payload = {
        "benchmarks": {
            "operation_filter": "lde",
        },
        "report": {
            "operations": [
                {
                    "operation": "lde",
                    "cpu": {"mean_ms": 2.0},
                    "gpu": {"mean_ms": 1.0},
                }
            ]
        },
    }

    report = aggregate_stage_timings.normalize_report(payload)

    assert report["operation_filter"] == "lde"
    assert report["operations"][0]["operation"] == "lde"


def test_load_samples_reports_bundle_filter(tmp_path):
    payload = {
        "benchmarks": {
            "operation_filter": "lde",
        },
        "report": {
            "operations": [
                {
                    "operation": "lde",
                    "cpu": {"mean_ms": 2.0},
                    "gpu": {"mean_ms": 1.0},
                }
            ]
        },
    }
    path = tmp_path / "wrapped_cuda.json"
    path.write_text(json.dumps(payload), encoding="utf-8")

    samples = aggregate_stage_timings.load_samples([str(path)], None)

    assert len(samples) == 1
    sample = samples[0]
    assert sample.bundle_filter == "lde"
    assert sample.operation == "lde"
    assert sample.cpu_ms == 2.0
    assert sample.gpu_ms == 1.0


def test_render_table_includes_filter_column():
    sample = aggregate_stage_timings.StageSample(
        path=aggregate_stage_timings.Path("wrapped_cuda.json"),
        bundle_filter="fft",
        operation="fft",
        gpu_ms=1.0,
        cpu_ms=2.0,
    )

    table = aggregate_stage_timings.render_table([sample])

    assert "| Report | Filter | Operation | GPU mean (ms) | CPU mean (ms) | Speedup |" in table
    assert "| `wrapped_cuda.json` | fft | fft | 1.000 | 2.000 | 2.000 |" in table
