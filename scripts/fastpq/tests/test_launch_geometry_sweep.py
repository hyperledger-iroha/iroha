"""Tests for the FASTPQ Metal launch-geometry sweep."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from scripts.fastpq import launch_geometry_sweep


def _run_report_payload(payload: object, tmp_path: Path, monkeypatch) -> dict:
    def complete(cmd, **_kwargs):
        output_path = Path(cmd[cmd.index("--output") + 1])
        output_path.write_text(json.dumps(payload), encoding="utf-8")
        return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")

    monkeypatch.setattr(launch_geometry_sweep.subprocess, "run", complete)
    monkeypatch.setattr(launch_geometry_sweep, "collect_host_metadata", lambda _label: {})
    result = launch_geometry_sweep.main(
        [
            "--artifact-dir",
            str(tmp_path),
            "--bench-prefix",
            "fake-benchmark",
        ]
    )
    assert result == 0
    summary = json.loads((tmp_path / "summary.json").read_text(encoding="utf-8"))
    return summary[0]


def test_classification_requires_explicit_metal_accelerator() -> None:
    classification, _totals = launch_geometry_sweep._classify_entry(
        {
            "status": "ok",
            "operations": {"fft": {"gpu_mean_ms": 1.0, "cpu_mean_ms": 2.0}},
            "metal_dispatch_queue": {},
        },
        min_total_speedup=0,
        min_queue_busy=0,
        min_dispatch_count=0,
    )

    assert classification["stable"] is False
    assert "gpu_unavailable" in classification["reasons"]
    assert "backend=unknown" in classification["reasons"]


@pytest.mark.parametrize("invalid", [True, float("nan"), float("inf")])
def test_classification_rejects_non_finite_or_boolean_timings(invalid: object) -> None:
    classification, totals = launch_geometry_sweep._classify_entry(
        {
            "gpu_available": True,
            "gpu_backend": "metal",
            "status": "ok",
            "operations": {
                "fft": {"gpu_mean_ms": invalid, "cpu_mean_ms": 2.0},
            },
            "metal_dispatch_queue": {"busy_ratio": 1.0, "dispatch_count": 1},
        },
        min_total_speedup=1.0,
        min_queue_busy=0.1,
        min_dispatch_count=1,
    )

    assert classification["stable"] is False
    assert "missing_total_speedup" in classification["reasons"]
    assert totals["speedup_ratio"] is None


def test_classification_rejects_non_finite_derived_speedup() -> None:
    classification, totals = launch_geometry_sweep._classify_entry(
        {
            "gpu_available": True,
            "gpu_backend": "metal",
            "status": "ok",
            "operations": {
                "fft": {"gpu_mean_ms": 5e-324, "cpu_mean_ms": 1e308},
            },
            "metal_dispatch_queue": {"busy_ratio": 1.0, "dispatch_count": 1},
        },
        min_total_speedup=1.0,
        min_queue_busy=0.1,
        min_dispatch_count=1,
    )

    assert totals["speedup_ratio"] is None
    assert classification["stable"] is False
    assert "missing_total_speedup" in classification["reasons"]


@pytest.mark.parametrize("invalid", [True, float("nan"), float("inf")])
def test_classification_rejects_non_finite_or_boolean_queue_values(invalid: object) -> None:
    classification, _totals = launch_geometry_sweep._classify_entry(
        {
            "gpu_available": True,
            "gpu_backend": "metal",
            "status": "ok",
            "operations": {
                "fft": {"gpu_mean_ms": 1.0, "cpu_mean_ms": 2.0},
            },
            "metal_dispatch_queue": {
                "busy_ratio": invalid,
                "dispatch_count": invalid,
            },
        },
        min_total_speedup=1.0,
        min_queue_busy=0.1,
        min_dispatch_count=1,
    )

    assert classification["stable"] is False
    assert "missing_queue_busy" in classification["reasons"]
    assert "missing_dispatch_count" in classification["reasons"]


@pytest.mark.parametrize(
    ("operations", "expected_reason"),
    [
        ([], "missing_operations"),
        ("invalid", "missing_operations"),
        ({"fft": None}, "invalid_operation=fft"),
    ],
)
def test_classification_handles_malformed_operations(
    operations: object, expected_reason: str
) -> None:
    classification, totals = launch_geometry_sweep._classify_entry(
        {
            "gpu_available": True,
            "gpu_backend": "metal",
            "status": "ok",
            "operations": operations,
            "metal_dispatch_queue": {"busy_ratio": 1.0, "dispatch_count": 1},
        },
        min_total_speedup=1.0,
        min_queue_busy=0.1,
        min_dispatch_count=1,
    )

    assert classification["stable"] is False
    assert expected_reason in classification["reasons"]
    assert totals["speedup_ratio"] is None


def test_classification_handles_malformed_queue() -> None:
    classification, _totals = launch_geometry_sweep._classify_entry(
        {
            "gpu_available": True,
            "gpu_backend": "metal",
            "status": "ok",
            "operations": {
                "fft": {"gpu_mean_ms": 1.0, "cpu_mean_ms": 2.0},
            },
            "metal_dispatch_queue": [],
        },
        min_total_speedup=1.0,
        min_queue_busy=0.1,
        min_dispatch_count=1,
    )

    assert classification["stable"] is False
    assert "missing_queue_stats" in classification["reasons"]


@pytest.mark.parametrize(
    ("overrides", "message"),
    [
        ({"min_total_speedup": float("nan")}, "min_total_speedup"),
        ({"min_queue_busy": float("inf")}, "min_queue_busy"),
        ({"min_dispatch_count": True}, "min_dispatch_count"),
    ],
)
def test_classification_rejects_invalid_thresholds(
    overrides: dict, message: str
) -> None:
    thresholds = {
        "min_total_speedup": 0.0,
        "min_queue_busy": 0.0,
        "min_dispatch_count": 0,
    }
    thresholds.update(overrides)

    with pytest.raises(ValueError, match=message):
        launch_geometry_sweep._classify_entry(
            {
                "gpu_available": True,
                "gpu_backend": "metal",
                "status": "ok",
                "operations": {
                    "fft": {"gpu_mean_ms": 1.0, "cpu_mean_ms": 2.0}
                },
                "metal_dispatch_queue": {},
            },
            **thresholds,
        )


@pytest.mark.parametrize(
    ("payload", "error_fragment"),
    [
        ([], "payload must be an object"),
        ({"operations": {}}, "operations must be a list"),
        ({"operations": [None]}, "operation 0 must be an object"),
        (
            {"operations": [{"operation": []}]},
            "operation 0.operation must be a non-empty string",
        ),
        (
            {"operations": [{"operation": "fft", "gpu": []}]},
            "operation 0.gpu must be an object",
        ),
        (
            {"operations": [], "metal_dispatch_queue": []},
            "metal_dispatch_queue must be an object",
        ),
    ],
)
def test_malformed_report_payload_becomes_error_entry(
    payload: object, error_fragment: str, tmp_path: Path, monkeypatch
) -> None:
    entry = _run_report_payload(payload, tmp_path, monkeypatch)

    assert entry["status"] == "error"
    assert error_fragment in entry["error"]


@pytest.mark.parametrize(
    ("gpu_available", "expected_stable"),
    [(True, True), ("true", False), (1, False)],
)
def test_report_requires_literal_true_gpu_availability(
    gpu_available: object, expected_stable: bool, tmp_path: Path, monkeypatch
) -> None:
    entry = _run_report_payload(
        {
            "gpu_available": gpu_available,
            "gpu_backend": "metal",
            "operations": [
                {
                    "operation": "fft",
                    "gpu": {"mean_ms": 1.0},
                    "cpu": {"mean_ms": 2.0},
                    "speedup": {"ratio": 2.0},
                }
            ],
            "metal_dispatch_queue": {"busy_ratio": 1.0, "dispatch_count": 1},
        },
        tmp_path,
        monkeypatch,
    )

    assert entry["gpu_available"] is (gpu_available is True)
    assert entry["classification"]["stable"] is expected_stable
    if not expected_stable:
        assert "gpu_unavailable" in entry["classification"]["reasons"]


def test_timeout_captures_byte_output(tmp_path: Path, monkeypatch) -> None:
    def timeout(*args, **kwargs):
        raise subprocess.TimeoutExpired(
            cmd=args[0],
            timeout=kwargs["timeout"],
            output=b"partial stdout\n",
            stderr=b"partial stderr\n",
        )

    monkeypatch.setattr(launch_geometry_sweep.subprocess, "run", timeout)
    monkeypatch.setattr(launch_geometry_sweep, "collect_host_metadata", lambda _label: {})
    summary_path = tmp_path / "summaries" / "nested" / "summary.json"
    matrix_path = tmp_path / "matrices" / "nested" / "matrix.csv"
    reasons_path = tmp_path / "reasons" / "nested" / "reasons.json"

    result = launch_geometry_sweep.main(
        [
            "--artifact-dir",
            str(tmp_path),
            "--timeout-seconds",
            "1",
            "--bench-prefix",
            "fake-benchmark",
            "--summary",
            str(summary_path),
            "--matrix-out",
            str(matrix_path),
            "--reason-summary-out",
            str(reasons_path),
        ]
    )

    assert result == 0
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary[0]["status"] == "timeout"
    assert Path(summary[0]["stdout"]).read_text(encoding="utf-8") == "partial stdout\n"
    assert Path(summary[0]["stderr"]).read_text(encoding="utf-8") == "partial stderr\n"
    assert matrix_path.is_file()
    assert reasons_path.is_file()


def test_timeout_rejects_integer_too_large_for_finite_check() -> None:
    with pytest.raises(SystemExit) as exc_info:
        launch_geometry_sweep.main(["--timeout-seconds", "9" * 400])

    assert exc_info.value.code == 2
