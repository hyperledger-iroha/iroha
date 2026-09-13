"""Tests for the FASTPQ Metal capture summary helper."""

from scripts.fastpq import metal_capture_summary
from scripts.fastpq.tests.test_digest384_evidence import primitive_report
import pytest
from scripts.fastpq.tests.report_fixtures import add_synthetic_raw_copy, complete_flat_report


def sample_capture():
    capture = {
        "producer_schema": "metal_flat",
        "iterations": 2, "warmups": 1, "rows": 8, "padded_rows": 8, "column_count": 2, "operation_filter": "all",
        "execution_mode": "gpu",
        "gpu_backend": "metal",
        "gpu_available": True,
        "metal_heuristics": {
            "lde_tile_stage_limit": 14,
            "batch_columns": {
                "fft": {"columns": 16},
                "lde": {"columns": 16},
            },
        },
        "metal_dispatch_queue": {
            "dispatch_count": 12,
            "overlap_ms": 42.0,
            "max_in_flight": 3,
            "limit": 8,
        },
        "operations": [
            {
                "operation": "fft",
                "columns": 16,
                "input_len": 32,
                "gpu_recorded": True,
                "gpu": {"mean_ms": 10.0, "min_ms": 9.0, "max_ms": 11.0},
                "cpu": {"mean_ms": 20.0, "min_ms": 19.0, "max_ms": 21.0},
                "speedup": {"ratio": 2.0, "delta_ms": 10.0},
            },
            {
                **primitive_report()["operations"][0],
                "gpu": {"mean_ms": 30.0, "min_ms": 29.0, "max_ms": 31.0},
                "cpu": {"mean_ms": 60.0, "min_ms": 59.0, "max_ms": 61.0},
                "speedup": {"ratio": 2.0, "delta_ms": 30.0},
            },
        ],
    }
    supplied = {entry["operation"]: entry for entry in capture["operations"]}
    complete = add_synthetic_raw_copy({"producer_schema": "metal_flat", "benchmarks": complete_flat_report("metal", iterations=2, warmups=1)})["report"]
    for entry in complete["operations"]:
        if entry["operation"] in supplied:
            entry.update(supplied[entry["operation"]])
        else:
            entry.update(cpu={"mean_ms": 4.0, "min_ms": 4.0, "max_ms": 4.0},
                         gpu={"mean_ms": 2.0, "min_ms": 2.0, "max_ms": 2.0},
                         speedup={"ratio": 2.0, "delta_ms": 2.0})
    capture["operations"] = complete["operations"]
    return capture


def test_build_stage_rows_computes_gpu_shares():
    capture = sample_capture()
    rows = metal_capture_summary.build_stage_rows(capture)

    assert len(rows) == 6
    fft, poseidon = rows[0], rows[3]
    assert fft.label == "FFT"
    assert abs((fft.gpu_share or 0) - 10.0 / 48.0) < 1e-6
    assert abs((poseidon.gpu_share or 0) - 30.0 / 48.0) < 1e-6


def test_render_markdown_table_formats_ranges():
    capture = sample_capture()
    rows = metal_capture_summary.build_stage_rows(capture)
    table = metal_capture_summary.render_markdown_table(rows)
    assert "| FFT | 16 | 32 | 10.000 ms (9.000-11.000) | 20.000 ms (19.000-21.000) |" in table
    assert "| Six-lane trace columns | 2 | 8 | 30.000 ms (29.000-31.000)" in table


def test_render_summary_mentions_queue_and_dominant_stage():
    capture = sample_capture()
    rows = metal_capture_summary.build_stage_rows(capture)
    summary = metal_capture_summary.render_summary(capture, rows)
    assert "GPU total mean: 48.000 ms" in summary
    assert "Metal dispatch queue: dispatch_count=12 overlap_ms=42.0" in summary
    assert "Heuristics: lde_tile_stage_limit=14" in summary
    assert "batch_columns: fft=16, lde=16" in summary
    assert "Dominant stage: Six-lane trace columns accounts for 62.5%" in summary


@pytest.mark.parametrize("mutation", ["retired", "partial"])
def test_summary_rejects_scalar_aliases_and_incomplete_lane_parity(mutation):
    capture = sample_capture()
    entry = capture["operations"][3]
    if mutation == "retired":
        entry["operation"] = "poseidon_hash_columns"
    else:
        entry["digest384"]["gpu"]["parity_checked_lanes"] = 6
    with pytest.raises((ValueError, SystemExit)):
        metal_capture_summary.build_stage_rows(capture)
