from __future__ import annotations

import json
from pathlib import Path

import pytest

from scripts.fastpq.profile_queue import _render_markdown, main, summarize_report


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
    assert summary.poseidon_pipeline["columns"] == 128
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


def test_wait_ratio_fallback_avoids_duration_sum_overflow(tmp_path: Path) -> None:
    payload = {
        "metal_dispatch_queue": {"dispatch_count": 1},
        "column_staging": {
            "batches": 1,
            "phases": {
                "fft": {"flatten_ms": 1e308, "wait_ms": 1e308},
                "lde": {"wait_ratio": 0.1},
            },
            "samples": {
                "lde": [{"flatten_ms": 1e308, "wait_ms": 1e308}],
            },
        },
    }
    input_path = tmp_path / "overflow.json"
    input_path.write_text(json.dumps(payload), encoding="utf-8")

    summary = summarize_report(
        path=input_path,
        label="overflow",
        min_dispatch=1,
        min_batches=1,
        max_wait_ratio=0.4,
    )

    assert summary.phase_max_wait_ratio["fft"] == pytest.approx(0.5)
    assert summary.phase_max_wait_ratio["lde"] == pytest.approx(0.5)
    assert any("fft wait ratio" in issue for issue in summary.issues)
    assert any("lde wait ratio" in issue for issue in summary.issues)


@pytest.mark.parametrize(
    ("payload", "expected_issue"),
    [
        ({"metal_dispatch_queue": None}, "missing metal_dispatch_queue"),
        (
            {"metal_dispatch_queue": {}, "column_staging": []},
            "missing column_staging",
        ),
        ([], "JSON object"),
        (
            {
                "metal_dispatch_queue": {"dispatch_count": 1},
                "column_staging": {"batches": 1, "phases": None},
            },
            "column_staging phases missing",
        ),
        (
            {
                "metal_dispatch_queue": {"dispatch_count": 1},
                "column_staging": {"batches": 1, "phases": {}},
                "run_status": {"state": "error", "reasons": None},
            },
            "run_status.reasons must be a list",
        ),
    ],
)
def test_summarize_report_handles_malformed_blocks(
    tmp_path: Path, payload: object, expected_issue: str
) -> None:
    input_path = tmp_path / "bench.json"
    input_path.write_text(json.dumps(payload), encoding="utf-8")

    summary = summarize_report(
        path=input_path,
        label="malformed",
        min_dispatch=1,
        min_batches=1,
        max_wait_ratio=None,
    )

    assert any(expected_issue in issue for issue in summary.issues)


@pytest.mark.parametrize(
    "invalid_count",
    [True, float("nan"), float("inf"), -1, 1.5],
)
def test_summarize_report_flags_invalid_counts(
    tmp_path: Path, invalid_count: object
) -> None:
    payload = {
        "metal_dispatch_queue": {"dispatch_count": invalid_count},
        "column_staging": {"batches": invalid_count, "phases": {}},
    }
    input_path = tmp_path / "bench.json"
    input_path.write_text(json.dumps(payload), encoding="utf-8")

    summary = summarize_report(
        path=input_path,
        label="invalid-counts",
        min_dispatch=1,
        min_batches=1,
        max_wait_ratio=None,
    )

    assert any("dispatch_count missing or invalid" in issue for issue in summary.issues)
    assert any("column_staging.batches missing or invalid" in issue for issue in summary.issues)


def test_summarize_report_rejects_non_finite_ratio_as_missing(tmp_path: Path) -> None:
    payload = {
        "metal_dispatch_queue": {
            "dispatch_count": 1,
            "busy_ratio": "invalid",
            "overlap_ratio": float("nan"),
        },
        "column_staging": {
            "batches": 1,
            "phases": {
                "fft": {
                    "flatten_ms": "invalid",
                    "wait_ms": [],
                    "wait_ratio": float("nan"),
                }
            },
        },
    }
    input_path = tmp_path / "bench.json"
    input_path.write_text(json.dumps(payload), encoding="utf-8")

    summary = summarize_report(
        path=input_path,
        label="invalid-ratios",
        min_dispatch=1,
        min_batches=1,
        max_wait_ratio=0.1,
    )

    assert summary.phase_max_wait_ratio["fft"] is None
    assert any("fft wait ratio missing or invalid" in issue for issue in summary.issues)
    markdown = _render_markdown([summary])
    assert "invalid-ratios" in markdown


@pytest.mark.parametrize(
    "args",
    [
        ["--min-dispatch", "-1"],
        ["--min-batches", "-1"],
        ["--max-wait-ratio", "nan"],
        ["--max-wait-ratio", "inf"],
        ["--max-wait-ratio", "1.1"],
    ],
)
def test_main_rejects_invalid_thresholds(args: list[str]) -> None:
    with pytest.raises(SystemExit) as exc_info:
        main([str(FIXTURE), *args])

    assert exc_info.value.code == 2
