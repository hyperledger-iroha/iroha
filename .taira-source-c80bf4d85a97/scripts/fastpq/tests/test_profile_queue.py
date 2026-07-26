from __future__ import annotations

import json
from pathlib import Path

import pytest

from scripts.fastpq.profile_queue import _render_markdown, summarize_report


FIXTURE = Path(__file__).parent / "data" / "profile_sample.json"


def test_summarize_report_extracts_metrics() -> None:
    summary = summarize_report(
        path=FIXTURE,
        label="sample",
        min_dispatch=1,
        min_batches=1,
        max_wait_ratio=None,
    )
    assert not summary.issues
    assert summary.queue is not None
    assert summary.queue["dispatch_count"] == 24
    assert summary.column_staging is not None
    assert summary.column_staging["batches"] == 48
    assert summary.poseidon_pipeline is not None
    assert summary.poseidon_pipeline["pipe_depth"] == 2
    assert summary.phase_metrics["fft"]["wait_ratio"] == pytest.approx(0.142, rel=1e-3)
    assert summary.phase_max_wait_ratio["fft"] == pytest.approx(0.142, rel=1e-3)
    markdown = _render_markdown([summary])
    assert "sample" in markdown
    assert "24" in markdown
    lines = markdown.splitlines()
    assert "FFT wait %" in lines[0]
    assert "Poseidon wait %" in lines[0]
    assert "Run status" in lines[0]
    row_parts = [part.strip() for part in lines[2].strip().split("|") if part.strip()]
    assert row_parts[0] == "sample"
    assert row_parts[1] in ("-", "ok")
    assert row_parts[2] == "24"


def test_summarize_report_flags_missing(tmp_path: Path) -> None:
    payload = {"metal_dispatch_queue": {"dispatch_count": 0}}
    input_path = tmp_path / "bench.json"
    input_path.write_text(json.dumps(payload), encoding="utf-8")
    summary = summarize_report(
        path=input_path,
        label="missing",
        min_dispatch=1,
        min_batches=1,
        max_wait_ratio=None,
    )
    assert any("dispatch_count" in issue for issue in summary.issues)
    assert any("column_staging" in issue for issue in summary.issues)


def test_wait_ratio_threshold_flags_issue() -> None:
    summary = summarize_report(
        path=FIXTURE,
        label="sample",
        min_dispatch=1,
        min_batches=1,
        max_wait_ratio=0.05,
    )
    assert any("fft wait ratio" in issue for issue in summary.issues)
