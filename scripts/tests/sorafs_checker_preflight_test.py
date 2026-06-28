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
    emit_checker_exception,
    emit_checker_error_lines,
    emit_checker_notice,
    inspect_checker_preflight_path_exists,
    inspect_checker_preflight_path_is_dir,
    inspect_checker_preflight_path_is_symlink,
    record_artifact_error,
    render_and_write_checker_summary,
    render_checker_summary,
    resolve_checker_preflight_path,
    validate_checker_output_parent,
    validate_checker_preflight,
    validate_checker_evidence_inputs,
    validate_checker_summary_output,
    write_checker_summary,
)


def test_absent_summary_out_passes() -> None:
    errors = validate_checker_preflight(argparse.Namespace(summary_out=None))

    assert errors == []


def test_checker_path_resolution_uses_shared_identity_helper() -> None:
    assert (
        resolve_checker_preflight_path.__globals__["resolve_path_identity"].__module__
        == "sorafs_path_identity"
    )


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


def test_present_evidence_spec_passes_input_check(tmp_path: Path) -> None:
    errors = validate_checker_evidence_inputs(
        argparse.Namespace(evidence_dir=[], evidence=[f"latest={tmp_path}"])
    )

    assert errors == []


def test_evidence_input_check_rejects_malformed_collections(tmp_path: Path) -> None:
    assert validate_checker_evidence_inputs(
        argparse.Namespace(evidence_dir="evidence", evidence=[]),
    ) == ["--evidence-dir paths must be a sequence"]
    assert validate_checker_evidence_inputs(
        argparse.Namespace(evidence_dir=[], evidence={"path": tmp_path}),
    ) == ["--evidence paths must be a sequence"]


def test_evidence_input_check_rejects_non_path_entries(tmp_path: Path) -> None:
    assert validate_checker_evidence_inputs(
        argparse.Namespace(evidence_dir=[tmp_path, "evidence"], evidence=[7, " bad"]),
    ) == [
        "--evidence-dir `evidence` must be a path",
        "--evidence `<non-path>` must be a path or evidence spec",
        "--evidence `<non-canonical-path>` must be a path or evidence spec",
    ]


def test_evidence_input_check_sanitizes_malformed_entry_labels() -> None:
    errors = validate_checker_evidence_inputs(
        argparse.Namespace(
            evidence_dir=[" bad-dir", b"raw-dir"],
            evidence=[7, " bad-evidence", b"raw-evidence"],
        )
    )

    assert errors == [
        "--evidence-dir `<non-canonical-path>` must be a path",
        "--evidence-dir `<non-path>` must be a path",
        "--evidence `<non-path>` must be a path or evidence spec",
        "--evidence `<non-canonical-path>` must be a path or evidence spec",
        "--evidence `<non-path>` must be a path or evidence spec",
    ]


def test_checker_preflight_reports_malformed_evidence_inputs(
    tmp_path: Path,
) -> None:
    errors = validate_checker_preflight(
        argparse.Namespace(
            summary_out=None,
            evidence_dir=[tmp_path],
            evidence=[7],
        )
    )

    assert errors == ["--evidence `<non-path>` must be a path or evidence spec"]


def test_checker_preflight_stops_after_malformed_evidence_inputs(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target.json"
    target.write_text("{}", encoding="utf-8")
    summary = tmp_path / "summary.json"
    summary.symlink_to(target)

    errors = validate_checker_preflight(
        argparse.Namespace(
            summary_out=summary,
            evidence_dir="evidence",
            evidence=[],
        )
    )

    assert errors == ["--evidence-dir paths must be a sequence"]


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


def test_summary_out_symlink_fails_preflight(tmp_path: Path) -> None:
    target = tmp_path / "actual-summary.json"
    target.write_text("{}", encoding="utf-8")
    summary = tmp_path / "summary.json"
    summary.symlink_to(target)

    errors = validate_checker_preflight(argparse.Namespace(summary_out=summary))

    assert errors == [f"--summary-out `{summary}` must not be a symlink"]


def test_summary_out_parent_symlink_fails_preflight(tmp_path: Path) -> None:
    target = tmp_path / "actual-parent"
    target.mkdir()
    parent = tmp_path / "summary-parent"
    parent.symlink_to(target, target_is_directory=True)

    errors = validate_checker_preflight(
        argparse.Namespace(summary_out=parent / "summary.json")
    )

    assert errors == [f"--summary-out parent `{parent}` must not be a symlink"]


def test_summary_out_parent_chain_symlink_fails_preflight(tmp_path: Path) -> None:
    target = tmp_path / "actual-parent"
    target.mkdir()
    parent = tmp_path / "summary-parent"
    parent.symlink_to(target, target_is_directory=True)

    errors = validate_checker_preflight(
        argparse.Namespace(summary_out=parent / "nested" / "summary.json")
    )

    assert errors == [f"--summary-out parent `{parent}` must not be a symlink"]


def test_summary_out_parent_chain_file_fails_preflight(tmp_path: Path) -> None:
    parent = tmp_path / "not-a-directory"
    parent.write_text("", encoding="utf-8")
    summary = parent / "nested" / "summary.json"

    errors = validate_checker_preflight(argparse.Namespace(summary_out=summary))

    assert errors == [
        f"--summary-out parent `{parent}` must be a directory when it exists"
    ]


def test_validate_checker_summary_output_rejects_non_path_without_traceback() -> None:
    errors: list[str] = []

    assert not validate_checker_summary_output("summary.json", errors)
    assert errors == ["--summary-out `summary.json` must be a path"]


def test_validate_checker_output_parent_rejects_non_path_without_traceback() -> None:
    errors: list[str] = []

    assert not validate_checker_output_parent(
        "summary.json",
        errors,
        label="--summary-out",
    )
    assert errors == ["--summary-out `summary.json` must be a path"]


def test_checker_preflight_path_inspectors_sanitize_malformed_path_labels() -> None:
    for helper in (
        inspect_checker_preflight_path_exists,
        inspect_checker_preflight_path_is_dir,
        inspect_checker_preflight_path_is_symlink,
    ):
        for path, expected_label in (
            (" summary.json", "<non-canonical-path>"),
            ("summary\njson", "<non-canonical-path>"),
            (b"summary.json", "<non-path>"),
            (7, "<non-path>"),
        ):
            errors: list[str] = []

            assert helper(path, errors, label="--summary-out") is None
            assert errors == [f"--summary-out `{expected_label}` must be a path"]


def test_checker_preflight_path_inspectors_sanitize_noncanonical_failures(
    tmp_path: Path,
    monkeypatch,
) -> None:
    summary = tmp_path / "bad\nsummary.json"

    def fail_inspection(path: Path) -> bool:
        if path == summary:
            raise OSError(f"inspection denied for {path}")
        return False

    for helper, attribute in (
        (inspect_checker_preflight_path_exists, "exists"),
        (inspect_checker_preflight_path_is_dir, "is_dir"),
        (inspect_checker_preflight_path_is_symlink, "is_symlink"),
    ):
        monkeypatch.setattr(Path, attribute, fail_inspection)
        errors: list[str] = []

        assert helper(summary, errors, label="--summary-out") is None
        assert errors == [
            "--summary-out `<non-canonical-path>` cannot be inspected: "
            "<non-canonical-error>"
        ]


def test_checker_preflight_path_inspectors_reject_malformed_error_container(
    tmp_path: Path,
) -> None:
    for helper in (
        inspect_checker_preflight_path_exists,
        inspect_checker_preflight_path_is_dir,
        inspect_checker_preflight_path_is_symlink,
    ):
        for errors in ("", (), {"error": "old"}, ["old", 7]):
            try:
                helper(tmp_path, errors, label="--summary-out")
            except ValueError as error:
                assert "checker preflight errors must be a list of strings" in str(
                    error
                )
            else:
                raise AssertionError(
                    f"{helper.__name__} accepted malformed errors {errors!r}"
                )


def test_checker_preflight_path_inspectors_reject_malformed_existing_error_text(
    tmp_path: Path,
) -> None:
    for helper in (
        inspect_checker_preflight_path_exists,
        inspect_checker_preflight_path_is_dir,
        inspect_checker_preflight_path_is_symlink,
    ):
        for errors in ([""], [" old"], ["old "], ["old\nerror"]):
            try:
                helper(tmp_path, errors, label="--summary-out")
            except ValueError as error:
                assert (
                    "checker preflight errors must contain non-empty canonical strings"
                    in str(error)
                )
            else:
                raise AssertionError(
                    f"{helper.__name__} accepted malformed error text {errors!r}"
                )


def test_checker_preflight_path_inspectors_reject_malformed_labels(
    tmp_path: Path,
) -> None:
    for helper in (
        inspect_checker_preflight_path_exists,
        inspect_checker_preflight_path_is_dir,
        inspect_checker_preflight_path_is_symlink,
    ):
        for label in ("", " --summary-out", "--summary-out ", "--summary\nout", 7):
            errors: list[str] = []
            try:
                helper(tmp_path, errors, label=label)
            except ValueError as error:
                assert (
                    "checker preflight label must be a non-empty canonical string"
                    in str(error)
                )
                assert errors == []
            else:
                raise AssertionError(
                    f"{helper.__name__} accepted malformed label {label!r}"
                )


def test_validate_checker_output_parent_rejects_malformed_error_container(
    tmp_path: Path,
) -> None:
    summary = tmp_path / "summary.json"

    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            validate_checker_output_parent(summary, errors, label="--summary-out")
        except ValueError as error:
            assert "checker preflight errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


def test_validate_checker_output_parent_rejects_malformed_label(
    tmp_path: Path,
) -> None:
    summary = tmp_path / "summary.json"

    for label in ("", " --summary-out", "--summary-out ", "--summary\nout", 7):
        errors: list[str] = []
        try:
            validate_checker_output_parent(summary, errors, label=label)
        except ValueError as error:
            assert "checker preflight label must be a non-empty canonical string" in str(
                error
            )
            assert errors == []
        else:
            raise AssertionError(f"accepted malformed label {label!r}")


def test_validate_checker_summary_output_rejects_malformed_error_container(
    tmp_path: Path,
) -> None:
    summary = tmp_path / "summary.json"

    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            validate_checker_summary_output(summary, errors)
        except ValueError as error:
            assert "checker preflight errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


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


def test_summary_out_same_as_explicit_evidence_sanitizes_noncanonical_paths(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "bad\nsummary.json"
    evidence.write_text("{}", encoding="utf-8")

    errors = validate_checker_preflight(
        argparse.Namespace(
            summary_out=tmp_path / "nested" / ".." / "bad\nsummary.json",
            evidence=[evidence],
            evidence_dir=[],
        )
    )

    assert errors == [
        "--summary-out `<non-canonical-path>` must not be the same path as "
        "--evidence `<non-canonical-path>`"
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


def test_render_checker_summary_rejects_non_object_summary() -> None:
    for summary in ("ready", ["ready"], None):
        try:
            render_checker_summary(summary)
        except ValueError as error:
            assert "checker summary must be an object" in str(error)
        else:
            raise AssertionError(f"accepted malformed summary {summary!r}")


def test_render_checker_summary_rejects_malformed_summary_keys() -> None:
    summaries = (
        {"": "ready"},
        {" status": "ready"},
        {"status\nbad": "ready"},
        {7: "ready"},
        {"artifacts": [{"path": "evidence.json", " bad": True}]},
        {"required": {"provider\nbad": {"valid": True}}},
    )

    for summary in summaries:
        try:
            render_checker_summary(summary)
        except ValueError as error:
            assert "checker summary keys must be non-empty canonical strings" in str(
                error
            )
        else:
            raise AssertionError(f"accepted malformed summary keys {summary!r}")


def test_render_and_write_checker_summary_reports_non_object_summary(capsys) -> None:
    rendered, errors = render_and_write_checker_summary(None, ["ready"])

    assert rendered == ""
    assert errors == ["failed to render checker summary JSON: checker summary must be an object"]
    assert capsys.readouterr().out == ""


def test_render_and_write_checker_summary_reports_malformed_summary_keys(
    tmp_path: Path, capsys
) -> None:
    summary = tmp_path / "summary.json"

    rendered, errors = render_and_write_checker_summary(summary, {" bad": "ready"})

    assert rendered == ""
    assert errors == [
        (
            "failed to render checker summary JSON: checker summary keys must be "
            "non-empty canonical strings"
        )
    ]
    assert not summary.exists()
    assert capsys.readouterr().out == ""


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


def test_render_and_write_checker_summary_sanitizes_malformed_render_error(
    tmp_path: Path,
    capsys,
    monkeypatch,
) -> None:
    summary = tmp_path / "summary.json"

    def render_raises(_summary):
        raise ValueError("summary render failed\nsecret")

    monkeypatch.setitem(
        render_and_write_checker_summary.__globals__,
        "render_checker_summary",
        render_raises,
    )

    rendered, errors = render_and_write_checker_summary(
        summary, {"status": "ready"}
    )

    assert rendered == ""
    assert errors == [
        "failed to render checker summary JSON: <non-canonical-error>"
    ]
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


def test_write_checker_summary_rejects_non_string_text(tmp_path: Path) -> None:
    summary = tmp_path / "summary.json"

    errors = write_checker_summary(summary, {"status": "ready"})

    assert errors == ["checker summary text must be a string"]
    assert not summary.exists()


def test_write_checker_summary_rejects_parent_file(tmp_path: Path) -> None:
    parent = tmp_path / "not-a-directory"
    parent.write_text("", encoding="utf-8")

    errors = write_checker_summary(parent / "summary.json", "{}")

    assert len(errors) == 1
    assert "must be a directory when it exists" in errors[0]


def test_write_checker_summary_rejects_directory_target(tmp_path: Path) -> None:
    errors = write_checker_summary(tmp_path, "{}")

    assert len(errors) == 1
    assert "must not be a directory" in errors[0]


def test_write_checker_summary_rejects_summary_symlink(tmp_path: Path) -> None:
    target = tmp_path / "actual-summary.json"
    target.write_text("old", encoding="utf-8")
    summary = tmp_path / "summary.json"
    summary.symlink_to(target)

    errors = write_checker_summary(summary, "new")

    assert errors == [f"--summary-out `{summary}` must not be a symlink"]
    assert target.read_text(encoding="utf-8") == "old"


def test_write_checker_summary_rejects_parent_chain_symlink_before_create(
    tmp_path: Path,
) -> None:
    target = tmp_path / "actual-parent"
    target.mkdir()
    parent = tmp_path / "summary-parent"
    parent.symlink_to(target, target_is_directory=True)

    errors = write_checker_summary(parent / "nested" / "summary.json", "{}")

    assert errors == [f"--summary-out parent `{parent}` must not be a symlink"]
    assert not (target / "nested").exists()


def test_write_checker_summary_sanitizes_create_parent_failure(
    tmp_path: Path,
    monkeypatch,
) -> None:
    summary = tmp_path / "bad\nparent" / "summary.json"
    original_mkdir = Path.mkdir

    def mkdir(path: Path, *args, **kwargs):
        if path == summary.parent:
            raise OSError(f"mkdir denied for {path}")
        return original_mkdir(path, *args, **kwargs)

    monkeypatch.setattr(Path, "mkdir", mkdir)

    errors = write_checker_summary(summary, "{}")

    assert errors == [
        "failed to create --summary-out parent `<non-canonical-path>`: "
        "<non-canonical-error>"
    ]


def test_write_checker_summary_sanitizes_write_failure(
    tmp_path: Path,
    monkeypatch,
) -> None:
    summary = tmp_path / "bad\nsummary.json"
    original_write_text = Path.write_text

    def write_text(path: Path, *args, **kwargs):
        if path == summary:
            raise OSError(f"write denied for {path}")
        return original_write_text(path, *args, **kwargs)

    monkeypatch.setattr(Path, "write_text", write_text)

    errors = write_checker_summary(summary, "{}")

    assert errors == [
        "failed to write --summary-out `<non-canonical-path>`: "
        "<non-canonical-error>"
    ]


def test_emit_checker_error_lines_writes_prefixed_stderr(capsys) -> None:
    emit_checker_error_lines(("one", "two"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: one\nERROR: two\n"


def test_emit_checker_error_lines_rejects_malformed_messages(capsys) -> None:
    for errors in ("error", b"error", {"error": "old"}, ["old", 7], None):
        try:
            emit_checker_error_lines(errors)
        except ValueError as error:
            assert "checker error messages must be a sequence of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed messages {errors!r}")
    for errors in ([""], [" old"], ["old "], ["old\nline"]):
        try:
            emit_checker_error_lines(errors)
        except ValueError as error:
            assert "checker error message must be a non-empty canonical string" in str(
                error
            )
        else:
            raise AssertionError(f"accepted malformed message content {errors!r}")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == ""


def test_emit_checker_exception_sanitizes_malformed_message(capsys) -> None:
    emit_checker_exception(ValueError("bad\nargument"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: <non-canonical-error>\n"


def test_emit_checker_exception_preserves_canonical_message(capsys) -> None:
    emit_checker_exception(ValueError("bad argument"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: bad argument\n"


def test_emit_checker_error_block_writes_heading_and_bullets(capsys) -> None:
    emit_checker_error_block("ERROR: rollout evidence is incomplete:", ("one", "two"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: rollout evidence is incomplete:\n- one\n- two\n"


def test_emit_checker_error_block_rejects_malformed_messages_before_heading(
    capsys,
) -> None:
    for errors in ("error", b"error", {"error": "old"}, ["old", 7], None):
        try:
            emit_checker_error_block("ERROR: rollout evidence is incomplete:", errors)
        except ValueError as error:
            assert "checker error messages must be a sequence of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed messages {errors!r}")
    for errors in ([""], [" old"], ["old "], ["old\nline"]):
        try:
            emit_checker_error_block("ERROR: rollout evidence is incomplete:", errors)
        except ValueError as error:
            assert "checker error message must be a non-empty canonical string" in str(
                error
            )
        else:
            raise AssertionError(f"accepted malformed message content {errors!r}")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == ""


def test_emit_checker_notice_writes_stderr(capsys) -> None:
    emit_checker_notice("rollout evidence is ready")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "rollout evidence is ready\n"


def test_emit_checker_notice_rejects_malformed_message(capsys) -> None:
    for message in ("", " rollout evidence is ready", "ready\nnext", 7):
        try:
            emit_checker_notice(message)
        except ValueError as error:
            assert "checker notice message must be a non-empty canonical string" in str(
                error
            )
        else:
            raise AssertionError(f"accepted malformed notice {message!r}")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == ""


def test_artifact_path_label_returns_path_or_unknown() -> None:
    assert artifact_path_label({"path": "evidence.json"}) == "evidence.json"
    assert artifact_path_label({"path": ""}) == "<unknown>"
    assert artifact_path_label({"path": " evidence.json"}) == "<unknown>"
    assert artifact_path_label({"path": "evidence\njson"}) == "<unknown>"
    assert artifact_path_label({"path": None}) == "<unknown>"
    assert artifact_path_label({}) == "<unknown>"
    assert artifact_path_label("bad") == "<unknown>"


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


def test_record_artifact_error_rebuilds_dirty_artifact_error_buckets() -> None:
    for existing_errors in (["old", 7], [" old"], ["old\nline"]):
        artifact = {"path": "evidence.json", "valid": True, "errors": existing_errors}
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


def test_record_artifact_error_rejects_malformed_summary_error_container() -> None:
    for summary_errors in ("", (), {"error": "old"}, ["old", 7]):
        artifact = {"path": "evidence.json", "valid": True, "errors": []}

        try:
            record_artifact_error(artifact, "new", summary_errors)
        except ValueError as error:
            assert "checker preflight errors must be a list of strings" in str(error)
            assert artifact == {
                "path": "evidence.json",
                "valid": True,
                "errors": [],
            }
        else:
            raise AssertionError(f"accepted malformed summary errors {summary_errors!r}")


def test_record_artifact_error_rejects_malformed_error_messages() -> None:
    for error_message in ("", " new", "new ", "new\nline", 7):
        artifact = {"path": "evidence.json", "valid": True, "errors": []}
        summary_errors: list[str] = []

        try:
            record_artifact_error(artifact, error_message, summary_errors)
        except ValueError as error:
            assert "artifact error must be a non-empty canonical string" in str(error)
            assert artifact == {
                "path": "evidence.json",
                "valid": True,
                "errors": [],
            }
            assert summary_errors == []
        else:
            raise AssertionError(f"accepted malformed artifact error {error_message!r}")


def test_record_artifact_error_rejects_malformed_summary_error_messages() -> None:
    for summary_error in ("", " summary", "summary ", "summary\nline", 7):
        artifact = {"path": "evidence.json", "valid": True, "errors": []}
        summary_errors: list[str] = []

        try:
            record_artifact_error(
                artifact,
                "artifact detail",
                summary_errors,
                summary_error=summary_error,
            )
        except ValueError as error:
            assert "artifact summary error must be a non-empty canonical string" in str(
                error
            )
            assert artifact == {
                "path": "evidence.json",
                "valid": True,
                "errors": [],
            }
            assert summary_errors == []
        else:
            raise AssertionError(f"accepted malformed summary error {summary_error!r}")


def test_record_artifact_error_rejects_malformed_artifact_path_before_mutation() -> None:
    for path in (" evidence.json", "evidence.json ", "evidence\njson"):
        artifact = {"path": path, "valid": True, "errors": []}
        summary_errors: list[str] = []

        record_artifact_error(artifact, "new", summary_errors)

        assert artifact == {"path": path, "valid": True, "errors": []}
        assert summary_errors == [
            "artifact path label must be a non-empty canonical string"
        ]


def test_record_artifact_error_uses_unknown_for_malformed_path() -> None:
    artifact = {"valid": True, "errors": []}
    summary_errors: list[str] = []

    record_artifact_error(artifact, "new", summary_errors)

    assert artifact["valid"] is False
    assert artifact["errors"] == ["new"]
    assert summary_errors == ["<unknown>: new"]


def test_record_artifact_error_rejects_malformed_artifact_rows() -> None:
    summary_errors: list[str] = []

    record_artifact_error("bad", "new", summary_errors)
    record_artifact_error(
        ["bad"],
        "artifact detail",
        summary_errors,
        summary_error="summary detail",
    )

    assert summary_errors == ["<unknown>: new", "<unknown>: summary detail"]
