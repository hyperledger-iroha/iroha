"""Tests for the FASTPQ Metal capture summary helper."""

from scripts.fastpq import metal_capture_summary


def sample_capture():
    return {
        "execution_mode": "gpu",
        "gpu_backend": "metal",
        "gpu_available": True,
        "metal_heuristics": {
            "poseidon_batch_multiplier": 3,
            "lde_tile_stage_limit": 14,
            "batch_columns": {
                "fft": {"columns": 16},
                "lde": {"columns": 16},
                "poseidon": {"columns": 32},
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
                "operation": "poseidon_hash_columns",
                "columns": 16,
                "input_len": 64,
                "gpu_recorded": True,
                "gpu": {"mean_ms": 30.0, "min_ms": 29.0, "max_ms": 31.0},
                "cpu": {"mean_ms": 60.0, "min_ms": 59.0, "max_ms": 61.0},
                "speedup": {"ratio": 2.0, "delta_ms": 30.0},
            },
        ],
    }


def test_build_stage_rows_computes_gpu_shares():
    capture = sample_capture()
    rows = metal_capture_summary.build_stage_rows(capture)

    assert len(rows) == 2
    fft, poseidon = rows
    assert fft.label == "FFT"
    assert abs((fft.gpu_share or 0) - 0.25) < 1e-6
    assert abs((poseidon.gpu_share or 0) - 0.75) < 1e-6


def test_render_markdown_table_formats_ranges():
    capture = sample_capture()
    rows = metal_capture_summary.build_stage_rows(capture)
    table = metal_capture_summary.render_markdown_table(rows)
    assert "| FFT | 16 | 32 | 10.000 ms (9.000-11.000) | 20.000 ms (19.000-21.000) |" in table
    assert "| Poseidon | 16 | 64 | 30.000 ms (29.000-31.000)" in table


def test_render_summary_mentions_queue_and_dominant_stage():
    capture = sample_capture()
    rows = metal_capture_summary.build_stage_rows(capture)
    summary = metal_capture_summary.render_summary(capture, rows)
    assert "GPU total mean: 40.000 ms" in summary
    assert "Metal dispatch queue: dispatch_count=12 overlap_ms=42.0" in summary
    assert "Heuristics: poseidon_batch_multiplier=3" in summary
    assert "batch_columns: fft=16, lde=16, poseidon=32" in summary
    assert "Dominant stage: Poseidon accounts for 75.0%" in summary
