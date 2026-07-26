"""Tests for the FASTPQ dashboard panel updater."""

from pathlib import Path

from scripts.fastpq import update_dashboard_panel


def test_build_markdown_includes_focused_cuda_filter_and_regen_hint():
    bundle = {
        "metadata": {
            "generated_at": "2026-03-27T12:00:00Z",
            "labels": {
                "device_class": "xeon-rtx",
                "gpu_model": "NVIDIA RTX 6000 Ada",
            },
        },
        "benchmarks": {
            "rows": 8,
            "padded_rows": 8,
            "iterations": 1,
            "column_count": 2,
            "gpu_backend": "cuda",
            "operation_filter": "lde",
            "operations": [
                {
                    "operation": "lde",
                    "cpu_mean_ms": 2.0,
                    "gpu_mean_ms": 1.0,
                    "speedup_ratio": 2.0,
                }
            ],
        },
    }

    markdown = update_dashboard_panel.build_markdown(
        Path("artifacts/fastpq_cuda_bench_lde.json"),
        bundle,
    )

    assert "- Operation filter: `lde` (focused capture)" in markdown
    assert "- Column count: **2**" in markdown
    assert "`fastpq_cuda_bench`" in markdown
    assert "Reuse `--operation lde` for focused reruns." in markdown


def test_build_markdown_derives_all_filter_for_multi_operation_bundles():
    bundle = {
        "metadata": {
            "generated_at": "2026-03-27T12:00:00Z",
        },
        "benchmarks": {
            "rows": 8,
            "padded_rows": 8,
            "iterations": 1,
            "gpu_backend": "metal",
            "operations": [
                {"operation": "fft", "cpu_mean_ms": 2.0, "gpu_mean_ms": 1.0, "speedup_ratio": 2.0},
                {"operation": "lde", "cpu_mean_ms": 5.0, "gpu_mean_ms": 4.0, "speedup_ratio": 1.25},
            ],
        },
    }

    markdown = update_dashboard_panel.build_markdown(
        Path("artifacts/fastpq_metal_bench.json"),
        bundle,
    )

    assert "- Operation filter: `all`" in markdown
    assert "`fastpq_metal_bench`" in markdown
    assert "focused capture" not in markdown

