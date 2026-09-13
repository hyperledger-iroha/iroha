"""Tests for the FASTPQ dashboard panel updater."""

from pathlib import Path

from scripts.fastpq import update_dashboard_panel
from scripts.fastpq.tests.report_fixtures import complete_flat_report, add_synthetic_raw_copy, flat_report


def test_build_markdown_includes_focused_cuda_filter_and_regen_hint():
    bundle = {
        "metadata": {
            "generated_at": "2026-03-27T12:00:00Z",
            "labels": {
                "device_class": "xeon-rtx",
                "gpu_model": "NVIDIA RTX 6000 Ada",
            },
        },
        "producer_schema": "cuda_nested",
        "benchmarks": flat_report(),
    }

    add_synthetic_raw_copy(bundle)
    markdown = update_dashboard_panel.build_markdown(
        Path("artifacts/fastpq_cuda_bench_lde.json"),
        bundle,
    )

    assert "- Operation filter: `lde` (focused capture)" in markdown
    assert "- Column count: **2**" in markdown
    assert "`fastpq_cuda_bench`" in markdown
    assert "Reuse `--operation lde` for focused reruns." in markdown


def test_build_markdown_preserves_explicit_all_filter_for_multi_operation_bundles():
    bundle = {
        "metadata": {
            "generated_at": "2026-03-27T12:00:00Z",
        },
        "producer_schema": "metal_flat",
        "benchmarks": complete_flat_report("metal"),
    }

    add_synthetic_raw_copy(bundle)
    markdown = update_dashboard_panel.build_markdown(
        Path("artifacts/fastpq_metal_bench.json"),
        bundle,
    )

    assert "- Operation filter: `all`" in markdown
    assert "`fastpq_metal_bench`" in markdown
    assert "focused capture" not in markdown

