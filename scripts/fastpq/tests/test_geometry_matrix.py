"""Tests for FASTPQ geometry stability classification."""

import json
import sys
from pathlib import Path

import pytest

from scripts.fastpq import geometry_matrix


def _timed_entry() -> dict:
    return {
        "status": "ok",
        "operations": {
            name: {"gpu_mean_ms": 1.0} for name in geometry_matrix.REQUIRED_OPERATIONS
        },
    }


def test_classify_entry_requires_accelerator_metadata() -> None:
    classification, reasons = geometry_matrix.classify_entry(_timed_entry())

    assert classification == "unstable"
    assert "gpu_unavailable" in reasons
    assert "backend=unknown" in reasons


def test_classify_entry_accepts_explicit_metal_accelerator() -> None:
    entry = _timed_entry()
    entry.update({"gpu_available": True, "gpu_backend": "metal"})

    assert geometry_matrix.classify_entry(entry) == ("stable", [])


def test_explicit_stable_classification_cannot_bypass_accelerator_checks() -> None:
    entry = _timed_entry()
    entry["classification"] = {"stable": True, "reasons": []}

    row = geometry_matrix.build_matrix_entries([entry])[0]

    assert row["classification"] == "unstable"
    assert "gpu_unavailable" in row["classification_reasons"]
    assert "backend=unknown" in row["classification_reasons"]


def test_explicit_stable_classification_cannot_bypass_invalid_timings() -> None:
    entry = _timed_entry()
    entry.update(
        {
            "gpu_available": True,
            "gpu_backend": "metal",
            "classification": {"stable": True, "reasons": []},
        }
    )
    entry["operations"][geometry_matrix.REQUIRED_OPERATIONS[0]]["gpu_mean_ms"] = float(
        "nan"
    )

    row = geometry_matrix.build_matrix_entries([entry])[0]

    assert row["classification"] == "unstable"
    assert (
        f"{geometry_matrix.REQUIRED_OPERATIONS[0]}_missing_gpu"
        in row["classification_reasons"]
    )


@pytest.mark.parametrize("invalid", [True, float("nan"), float("inf")])
def test_classify_entry_rejects_non_finite_or_boolean_timings(invalid: object) -> None:
    entry = _timed_entry()
    entry.update({"gpu_available": True, "gpu_backend": "metal"})
    for operation in entry["operations"].values():
        operation["gpu_mean_ms"] = invalid

    classification, reasons = geometry_matrix.classify_entry(entry)

    assert classification == "unstable"
    assert all(f"{name}_missing_gpu" in reasons for name in geometry_matrix.REQUIRED_OPERATIONS)


def test_build_matrix_handles_malformed_operations_and_mixed_env_types() -> None:
    malformed = {
        "status": "ok",
        "gpu_available": True,
        "gpu_backend": "metal",
        "operations": "invalid",
        "env": {geometry_matrix.ENV_COLUMNS[0][0]: "auto"},
    }
    numeric = _timed_entry()
    numeric.update(
        {
            "gpu_available": True,
            "gpu_backend": "metal",
            "env": {geometry_matrix.ENV_COLUMNS[0][0]: 1},
        }
    )

    rows = geometry_matrix.build_matrix_entries([malformed, numeric])

    assert len(rows) == 2
    malformed_row = next(row for row in rows if row["env"][geometry_matrix.ENV_COLUMNS[0][0]] == "auto")
    assert malformed_row["classification"] == "unstable"
    assert all(value is None for value in malformed_row["metrics"].values())


def test_build_env_summary_normalises_mixed_environment_values() -> None:
    first_env = geometry_matrix.ENV_COLUMNS[0][0]
    second_env = geometry_matrix.ENV_COLUMNS[1][0]
    rows = [
        {"env": {first_env: "auto", second_env: [2, 1]}},
        {"env": {first_env: 1, second_env: [2, 1]}},
        {"env": {first_env: "1", second_env: [2, 1]}},
    ]

    summary = geometry_matrix.build_env_summary(rows)

    assert [entry["env"][first_env] for entry in summary] == ["1", "auto"]
    assert summary[0]["total_runs"] == 2
    assert all(entry["env"][second_env] == "[2,1]" for entry in summary)


def test_build_env_summary_ignores_unsafe_numeric_samples() -> None:
    valid = {
        "env": {},
        "duration_seconds": 2.0,
        "metrics": {op: 4.0 for op in geometry_matrix.REQUIRED_OPERATIONS},
    }
    invalid_values = [True, float("nan"), float("inf"), 10**10_000]
    rows = [valid]
    rows.extend(
        {
            "env": {},
            "duration_seconds": value,
            "metrics": {op: value for op in geometry_matrix.REQUIRED_OPERATIONS},
        }
        for value in invalid_values
    )

    summary = geometry_matrix.build_env_summary(rows)

    assert len(summary) == 1
    assert summary[0]["average_duration_seconds"] == 2.0
    assert summary[0]["average_metrics_ms"] == {
        op: 4.0 for op in geometry_matrix.REQUIRED_OPERATIONS
    }


def test_load_summary_rejects_non_object_run(tmp_path: Path) -> None:
    summary_path = tmp_path / "summary.json"
    summary_path.write_text(json.dumps({"runs": [None]}), encoding="utf-8")

    with pytest.raises(ValueError, match="run 0 must be an object"):
        geometry_matrix.load_summary(summary_path)


@pytest.mark.parametrize(
    "contents",
    ["{", json.dumps({"runs": [None]})],
)
def test_main_reports_malformed_summary_as_parser_error(
    contents: str, tmp_path: Path, monkeypatch, capsys
) -> None:
    summary_path = tmp_path / "summary.json"
    summary_path.write_text(contents, encoding="utf-8")
    monkeypatch.setattr(
        sys,
        "argv",
        ["geometry_matrix.py", "--summary", str(summary_path)],
    )

    with pytest.raises(SystemExit) as error:
        geometry_matrix.main()

    assert error.value.code == 2
    stderr = capsys.readouterr().err
    assert "failed to load summary" in stderr
    assert "Traceback" not in stderr
    assert not (tmp_path / "geometry_matrix.md").exists()


def test_main_creates_nested_output_directories(tmp_path: Path, monkeypatch) -> None:
    entry = _timed_entry()
    entry.update({"gpu_available": True, "gpu_backend": "metal"})
    summary_path = tmp_path / "summary.json"
    summary_path.write_text(json.dumps([entry]), encoding="utf-8")
    outputs = {
        "--markdown-out": tmp_path / "markdown" / "nested" / "matrix.md",
        "--json-out": tmp_path / "matrix" / "nested" / "matrix.json",
        "--host-summary-out": tmp_path / "hosts" / "nested" / "hosts.json",
        "--env-summary-out": tmp_path / "env" / "nested" / "env.json",
        "--source-summary-out": tmp_path / "sources" / "nested" / "sources.json",
        "--reason-summary-out": tmp_path / "reasons" / "nested" / "reasons.json",
    }
    argv = ["geometry_matrix.py", "--summary", str(summary_path)]
    for option, path in outputs.items():
        argv.extend([option, str(path)])
    monkeypatch.setattr(sys, "argv", argv)

    geometry_matrix.main()

    assert all(path.is_file() for path in outputs.values())
