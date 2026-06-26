"""Tests for shared SoraFS checker preflight checks."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_checker_preflight import (  # noqa: E402
    artifact_path_label,
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_notice,
    record_artifact_error,
    render_and_write_checker_summary,
    render_checker_summary,
    validate_checker_preflight,
    validate_checker_evidence_inputs,
    write_checker_summary,
)


def test_absent_summary_out_passes() -> None:
    errors = validate_checker_preflight(argparse.Namespace(summary_out=None))

    assert errors == []


def test_absent_evidence_attrs_skip_evidence_input_check() -> None:
    assert validate_checker_evidence_inputs(argparse.Namespace()) == []


def test_missing_evidence_sources_fail_preflight() -> None:
    errors = validate_checker_preflight(
        argparse.Namespace(summary_out=None, evidence_dir=[], evidence=[])
    )

    assert errors == ["provide --evidence-dir or --evidence"]


def test_present_evidence_dir_passes_input_check(tmp_path: Path) -> None:
    errors = validate_checker_evidence_inputs(
        argparse.Namespace(evidence_dir=[tmp_path], evidence=[])
    )

    assert errors == []


def test_present_evidence_file_passes_input_check(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    errors = validate_checker_evidence_inputs(
        argparse.Namespace(evidence_dir=[], evidence=[evidence])
    )

    assert errors == []


def test_summary_out_directory_fails(tmp_path: Path) -> None:
    errors = validate_checker_preflight(argparse.Namespace(summary_out=tmp_path))

    assert len(errors) == 1
    assert "must not be a directory" in errors[0]


def test_summary_out_parent_file_fails(tmp_path: Path) -> None:
    parent = tmp_path / "not-a-directory"
    parent.write_text("", encoding="utf-8")

    errors = validate_checker_preflight(
        argparse.Namespace(summary_out=parent / "summary.json")
    )

    assert len(errors) == 1
    assert "must be a directory when it exists" in errors[0]


def test_summary_out_exists_inspection_failure_fails_preflight(
    tmp_path: Path,
    monkeypatch,
) -> None:
    summary = tmp_path / "summary.json"
    original_exists = Path.exists

    def exists(path: Path) -> bool:
        if path == summary:
            raise OSError("summary stat denied")
        return original_exists(path)

    monkeypatch.setattr(Path, "exists", exists)

    errors = validate_checker_preflight(argparse.Namespace(summary_out=summary))

    assert errors == [f"--summary-out `{summary}` cannot be inspected: summary stat denied"]


def test_summary_out_directory_inspection_failure_fails_preflight(
    tmp_path: Path,
    monkeypatch,
) -> None:
    summary = tmp_path / "summary.json"
    summary.write_text("{}", encoding="utf-8")
    original_is_dir = Path.is_dir

    def is_dir(path: Path) -> bool:
        if path == summary:
            raise OSError("summary type denied")
        return original_is_dir(path)

    monkeypatch.setattr(Path, "is_dir", is_dir)

    errors = validate_checker_preflight(argparse.Namespace(summary_out=summary))

    assert errors == [f"--summary-out `{summary}` cannot be inspected: summary type denied"]


def test_summary_out_parent_inspection_failure_fails_preflight(
    tmp_path: Path,
    monkeypatch,
) -> None:
    parent = tmp_path / "parent"
    summary = parent / "summary.json"
    original_exists = Path.exists

    def exists(path: Path) -> bool:
        if path == parent:
            raise OSError("parent stat denied")
        return original_exists(path)

    monkeypatch.setattr(Path, "exists", exists)

    errors = validate_checker_preflight(argparse.Namespace(summary_out=summary))

    assert errors == [
        f"--summary-out parent `{parent}` cannot be inspected: parent stat denied"
    ]


def test_summary_out_same_as_explicit_evidence_fails(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors = validate_checker_preflight(
        argparse.Namespace(
            summary_out=tmp_path / "nested" / ".." / "evidence.json",
            evidence=[evidence],
            evidence_dir=[],
        )
    )

    assert errors == [
        f"--summary-out `{tmp_path / 'nested' / '..' / 'evidence.json'}` "
        f"must not be the same path as --evidence `{evidence}`"
    ]


def test_summary_out_same_as_discovered_evidence_fails(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors = validate_checker_preflight(
        argparse.Namespace(
            summary_out=tmp_path / "nested" / ".." / "evidence.json",
            evidence=[],
            evidence_dir=[tmp_path],
        )
    )

    assert errors == [
        f"evidence file `{evidence}` conflicts with --summary-out "
        f"`{tmp_path / 'nested' / '..' / 'evidence.json'}`"
    ]


def test_write_checker_summary_ignores_absent_path() -> None:
    errors = write_checker_summary(None, "{}")

    assert errors == []


def test_render_checker_summary_sorts_keys_and_adds_trailing_newline() -> None:
    rendered = render_checker_summary({"status": "ready", "artifact_count": 2})

    assert rendered == '{\n  "artifact_count": 2,\n  "status": "ready"\n}\n'


def test_render_checker_summary_rejects_non_finite_numbers() -> None:
    try:
        render_checker_summary({"status": "ready", "latency_ms": float("nan")})
    except ValueError as error:
        assert "Out of range float values" in str(error)
    else:
        raise AssertionError("expected non-finite summary values to be rejected")


def test_render_and_write_checker_summary_reports_non_finite_values(capsys) -> None:
    rendered, errors = render_and_write_checker_summary(
        None, {"status": "ready", "latency_ms": float("inf")}
    )

    assert rendered == ""
    assert len(errors) == 1
    assert "failed to render checker summary JSON" in errors[0]
    assert "Out of range float values" in errors[0]
    assert capsys.readouterr().out == ""


def test_render_and_write_checker_summary_reports_non_serializable_values(
    tmp_path: Path, capsys
) -> None:
    summary = tmp_path / "summary.json"

    rendered, errors = render_and_write_checker_summary(
        summary, {"status": "ready", "path": tmp_path}
    )

    assert rendered == ""
    assert len(errors) == 1
    assert "failed to render checker summary JSON" in errors[0]
    assert "is not JSON serializable" in errors[0]
    assert not summary.exists()
    assert capsys.readouterr().out == ""


def test_render_and_write_checker_summary_prints_rendered_text_without_path(capsys) -> None:
    rendered, errors = render_and_write_checker_summary(
        None, {"status": "ready", "artifact_count": 2}
    )

    assert rendered == '{\n  "artifact_count": 2,\n  "status": "ready"\n}\n'
    assert errors == []
    assert capsys.readouterr().out == rendered


def test_render_and_write_checker_summary_writes_rendered_text(
    tmp_path: Path, capsys
) -> None:
    summary = tmp_path / "summary.json"

    rendered, errors = render_and_write_checker_summary(summary, {"status": "ready"})

    assert rendered == '{\n  "status": "ready"\n}\n'
    assert errors == []
    assert summary.read_text(encoding="utf-8") == rendered
    assert capsys.readouterr().out == ""


def test_write_checker_summary_creates_parent(tmp_path: Path) -> None:
    summary = tmp_path / "nested" / "summary.json"

    errors = write_checker_summary(summary, '{"status":"ready"}')

    assert errors == []
    assert summary.read_text(encoding="utf-8") == '{"status":"ready"}'


def test_write_checker_summary_rejects_parent_file(tmp_path: Path) -> None:
    parent = tmp_path / "not-a-directory"
    parent.write_text("", encoding="utf-8")

    errors = write_checker_summary(parent / "summary.json", "{}")

    assert len(errors) == 1
    assert "failed to create --summary-out parent" in errors[0]


def test_write_checker_summary_rejects_directory_target(tmp_path: Path) -> None:
    errors = write_checker_summary(tmp_path, "{}")

    assert len(errors) == 1
    assert "failed to write --summary-out" in errors[0]


def test_emit_checker_error_lines_writes_prefixed_stderr(capsys) -> None:
    emit_checker_error_lines(("one", "two"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: one\nERROR: two\n"


def test_emit_checker_error_block_writes_heading_and_bullets(capsys) -> None:
    emit_checker_error_block("ERROR: rollout evidence is incomplete:", ("one", "two"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: rollout evidence is incomplete:\n- one\n- two\n"


def test_emit_checker_notice_writes_stderr(capsys) -> None:
    emit_checker_notice("rollout evidence is ready")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "rollout evidence is ready\n"


def test_artifact_path_label_returns_path_or_unknown() -> None:
    assert artifact_path_label({"path": "evidence.json"}) == "evidence.json"
    assert artifact_path_label({"path": ""}) == "<unknown>"
    assert artifact_path_label({"path": None}) == "<unknown>"
    assert artifact_path_label({}) == "<unknown>"


def test_record_artifact_error_appends_existing_artifact_errors() -> None:
    artifact = {"path": "evidence.json", "valid": True, "errors": ["old"]}
    summary_errors: list[str] = []

    record_artifact_error(artifact, "new", summary_errors)

    assert artifact["valid"] is False
    assert artifact["errors"] == ["old", "new"]
    assert summary_errors == ["evidence.json: new"]


def test_record_artifact_error_rebuilds_malformed_artifact_errors() -> None:
    artifact = {"path": "evidence.json", "valid": True, "errors": "old"}
    summary_errors: list[str] = []

    record_artifact_error(artifact, "new", summary_errors)

    assert artifact["valid"] is False
    assert artifact["errors"] == ["new"]
    assert summary_errors == ["evidence.json: new"]


def test_record_artifact_error_can_use_summary_error() -> None:
    artifact = {"path": "evidence.json", "valid": True, "errors": []}
    summary_errors: list[str] = []

    record_artifact_error(
        artifact,
        "artifact detail",
        summary_errors,
        summary_error="summary detail",
    )

    assert artifact["valid"] is False
    assert artifact["errors"] == ["artifact detail"]
    assert summary_errors == ["evidence.json: summary detail"]


def test_record_artifact_error_uses_unknown_for_malformed_path() -> None:
    artifact = {"valid": True, "errors": []}
    summary_errors: list[str] = []

    record_artifact_error(artifact, "new", summary_errors)

    assert artifact["valid"] is False
    assert artifact["errors"] == ["new"]
    assert summary_errors == ["<unknown>: new"]
