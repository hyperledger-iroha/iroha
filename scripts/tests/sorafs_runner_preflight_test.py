"""Tests for shared SoraFS runner preflight checks."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_runner_preflight import (  # noqa: E402
    emit_runner_error_block,
    emit_runner_error_lines,
    emit_runner_notice,
    render_runner_plan,
    require_existing_dirs,
    require_existing_files,
    require_runner_non_negative_int,
    require_runner_positive_int,
    run_command_plan,
    runner_arg_label,
    validate_command_plan_artifacts,
    validate_runner_preflight,
    write_runner_plan,
)


@dataclass(frozen=True)
class Step:
    """Small command-plan step used by shared runner preflight tests."""

    label: str
    artifact: Path | None
    command: list[str]


def test_runner_arg_label_formats_namespace_field() -> None:
    assert runner_arg_label("max_route_latency_ms") == "--max-route-latency-ms"


def test_require_runner_positive_int_accepts_positive_value() -> None:
    errors: list[str] = []

    assert require_runner_positive_int(
        argparse.Namespace(max_route_latency_ms=1),
        "max_route_latency_ms",
        errors,
    )

    assert errors == []


def test_require_runner_positive_int_rejects_direct_non_int_values() -> None:
    errors: list[str] = []

    assert not require_runner_positive_int(
        argparse.Namespace(limit="1"),
        "limit",
        errors,
    )
    assert not require_runner_positive_int(
        argparse.Namespace(quorum=True),
        "quorum",
        errors,
    )
    assert not require_runner_positive_int(
        argparse.Namespace(screened_at=None),
        "screened_at",
        errors,
    )

    assert errors == [
        "--limit must be positive",
        "--quorum must be positive",
        "--screened-at must be positive",
    ]


def test_require_runner_positive_int_allows_optional_none() -> None:
    errors: list[str] = []

    assert require_runner_positive_int(
        argparse.Namespace(now_unix=None),
        "now_unix",
        errors,
        allow_none=True,
    )

    assert errors == []


def test_require_runner_positive_int_labels_optional_invalid_values() -> None:
    errors: list[str] = []

    assert not require_runner_positive_int(
        argparse.Namespace(now_unix=False),
        "now_unix",
        errors,
        allow_none=True,
    )

    assert errors == ["--now-unix must be positive when supplied"]


def test_require_runner_non_negative_int_accepts_zero() -> None:
    errors: list[str] = []

    assert require_runner_non_negative_int(
        argparse.Namespace(watch_poll_interval_ms=0),
        "watch_poll_interval_ms",
        errors,
    )

    assert errors == []


def test_require_runner_non_negative_int_rejects_direct_invalid_values() -> None:
    errors: list[str] = []

    assert not require_runner_non_negative_int(
        argparse.Namespace(max_evidence_age_secs=-1),
        "max_evidence_age_secs",
        errors,
    )
    assert not require_runner_non_negative_int(
        argparse.Namespace(max_feed_lag_secs="0"),
        "max_feed_lag_secs",
        errors,
    )
    assert not require_runner_non_negative_int(
        argparse.Namespace(max_cycle_age_secs=True),
        "max_cycle_age_secs",
        errors,
    )

    assert errors == [
        "--max-evidence-age-secs must be non-negative",
        "--max-feed-lag-secs must be non-negative",
        "--max-cycle-age-secs must be non-negative",
    ]


def test_out_dir_parent_file_fails(tmp_path: Path) -> None:
    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    parent = tmp_path / "not-a-directory"
    parent.write_text("", encoding="utf-8")

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=parent / "evidence",
            summary_out=None,
        ),
        summary_filename="rollout-summary.json",
    )

    assert len(errors) == 1
    assert "must be a directory when it exists" in errors[0]


def test_verifier_inspection_failure_fails_preflight(
    tmp_path: Path,
    monkeypatch,
) -> None:
    verifier = tmp_path / "verifier.py"
    out_dir = tmp_path / "evidence"
    original_is_file = Path.is_file

    def is_file(path: Path) -> bool:
        if path == verifier:
            raise OSError("verifier stat denied")
        return original_is_file(path)

    monkeypatch.setattr(Path, "is_file", is_file)

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=out_dir,
            summary_out=tmp_path / "summary.json",
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == [f"--verifier `{verifier}` cannot be inspected: verifier stat denied"]


def test_out_dir_inspection_failure_fails_preflight(
    tmp_path: Path,
    monkeypatch,
) -> None:
    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    out_dir = tmp_path / "evidence"
    original_exists = Path.exists

    def exists(path: Path) -> bool:
        if path == out_dir:
            raise OSError("out dir stat denied")
        return original_exists(path)

    monkeypatch.setattr(Path, "exists", exists)

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=out_dir,
            summary_out=tmp_path / "summary.json",
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == [f"--out-dir `{out_dir}` cannot be inspected: out dir stat denied"]


def test_summary_out_parent_file_fails(tmp_path: Path) -> None:
    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    parent = tmp_path / "not-a-directory"
    parent.write_text("", encoding="utf-8")

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=tmp_path / "evidence",
            summary_out=parent / "summary.json",
        ),
        summary_filename="rollout-summary.json",
    )

    assert len(errors) == 1
    assert "must be a directory when it exists" in errors[0]


def test_summary_out_directory_inspection_failure_fails_preflight(
    tmp_path: Path,
    monkeypatch,
) -> None:
    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    summary = tmp_path / "summary.json"
    summary.write_text("{}", encoding="utf-8")
    original_is_dir = Path.is_dir

    def is_dir(path: Path) -> bool:
        if path == summary:
            raise OSError("summary type denied")
        return original_is_dir(path)

    monkeypatch.setattr(Path, "is_dir", is_dir)

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=tmp_path / "evidence",
            summary_out=summary,
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == [f"--summary-out `{summary}` cannot be inspected: summary type denied"]


def test_summary_out_same_as_out_dir_fails_before_execution(tmp_path: Path) -> None:
    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    out_dir = tmp_path / "evidence"

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=out_dir,
            summary_out=tmp_path / "nested" / ".." / "evidence",
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == [
        f"--summary-out `{tmp_path / 'nested' / '..' / 'evidence'}` "
        f"must not be the same path as --out-dir `{out_dir}`"
    ]


def test_duplicate_input_file_identity_fails(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors = require_existing_files([evidence, evidence], "--evidence")

    assert len(errors) == 1
    assert "duplicate --evidence input" in errors[0]


def test_missing_input_file_reports_only_file_requirement(tmp_path: Path) -> None:
    missing = tmp_path / "missing.json"

    errors = require_existing_files([missing], "--evidence")

    assert errors == [f"--evidence `{missing}` must exist and be a file"]


def test_input_file_inspection_failure_is_reported(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence = tmp_path / "evidence.json"
    original_exists = Path.exists

    def exists(path: Path) -> bool:
        if path == evidence:
            raise OSError("input stat denied")
        return original_exists(path)

    monkeypatch.setattr(Path, "exists", exists)

    errors = require_existing_files([evidence], "--evidence")

    assert errors == [f"--evidence `{evidence}` cannot be inspected: input stat denied"]


def test_cross_label_duplicate_input_file_identity_fails(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")
    seen = {}

    errors = [
        *require_existing_files([evidence], "--first-evidence", seen=seen),
        *require_existing_files([evidence], "--second-evidence", seen=seen),
    ]

    assert len(errors) == 1
    assert "duplicate --second-evidence input" in errors[0]
    assert "--first-evidence" in errors[0]


def test_input_file_resolver_failure_is_reported(tmp_path: Path) -> None:
    loop = tmp_path / "loop.json"
    loop.symlink_to(loop)

    errors = require_existing_files([loop], "--evidence")

    assert any("cannot be resolved" in error for error in errors)


def test_duplicate_input_directory_identity_fails(tmp_path: Path) -> None:
    evidence_dir = tmp_path / "bundle"
    evidence_dir.mkdir()

    errors = require_existing_dirs([evidence_dir, evidence_dir], "--bundle")

    assert len(errors) == 1
    assert "duplicate --bundle directory" in errors[0]


def test_missing_input_directory_reports_only_directory_requirement(
    tmp_path: Path,
) -> None:
    missing = tmp_path / "missing-bundle"

    errors = require_existing_dirs([missing], "--bundle")

    assert errors == [f"--bundle `{missing}` must exist and be a directory"]


def test_input_directory_type_inspection_failure_is_reported(
    tmp_path: Path,
    monkeypatch,
) -> None:
    bundle = tmp_path / "bundle"
    bundle.mkdir()
    original_is_dir = Path.is_dir

    def is_dir(path: Path) -> bool:
        if path == bundle:
            raise OSError("directory type denied")
        return original_is_dir(path)

    monkeypatch.setattr(Path, "is_dir", is_dir)

    errors = require_existing_dirs([bundle], "--bundle")

    assert errors == [
        f"--bundle `{bundle}` cannot be inspected: directory type denied"
    ]


def test_input_directory_resolver_failure_is_reported(tmp_path: Path) -> None:
    loop = tmp_path / "loop"
    loop.symlink_to(loop)

    errors = require_existing_dirs([loop], "--bundle")

    assert any("cannot be resolved" in error for error in errors)


def test_duplicate_planned_artifact_identity_fails(tmp_path: Path) -> None:
    artifact = tmp_path / "summary.json"
    errors = validate_command_plan_artifacts(
        [
            Step("first", artifact, ["true"]),
            Step("second", artifact, ["true"]),
        ]
    )

    assert len(errors) == 1
    assert "duplicate planned artifact" in errors[0]


def test_planned_artifact_same_as_reserved_output_fails(tmp_path: Path) -> None:
    out_dir = tmp_path / "evidence"
    artifact = tmp_path / "nested" / ".." / "evidence"

    errors = validate_command_plan_artifacts(
        [Step("gate", artifact, ["true"])],
        reserved_output_paths=(out_dir,),
    )

    assert errors == [
        f"gate artifact `{artifact}` must not be the same path as "
        f"reserved output `{out_dir}`"
    ]


def test_planned_artifact_symlink_fails(tmp_path: Path) -> None:
    target = tmp_path / "target.json"
    target.write_text("{}", encoding="utf-8")
    artifact = tmp_path / "artifact.json"
    artifact.symlink_to(target)

    errors = validate_command_plan_artifacts([Step("gate", artifact, ["true"])])

    assert errors == [f"gate artifact `{artifact}` must not be a symlink"]


def test_render_runner_plan_uses_sorted_newline_terminated_json() -> None:
    rendered = render_runner_plan({"z": 1, "a": {"b": 2}})

    assert rendered == '{\n  "a": {\n    "b": 2\n  },\n  "z": 1\n}\n'


def test_render_runner_plan_rejects_non_finite_numbers() -> None:
    try:
        render_runner_plan({"schema": "example", "latency_ms": float("nan")})
    except ValueError as error:
        assert "Out of range float values" in str(error)
    else:
        raise AssertionError("expected non-finite runner plan values to be rejected")


def test_write_runner_plan_writes_rendered_plan(capsys) -> None:
    errors = write_runner_plan({"schema": "example", "steps": []})

    captured = capsys.readouterr()
    assert errors == []
    assert captured.out == '{\n  "schema": "example",\n  "steps": []\n}\n'


def test_write_runner_plan_reports_non_finite_plan_without_stdout(capsys) -> None:
    errors = write_runner_plan({"schema": "example", "latency_ms": float("inf")})

    captured = capsys.readouterr()
    assert len(errors) == 1
    assert "failed to render runner plan JSON" in errors[0]
    assert "Out of range float values" in errors[0]
    assert captured.out == ""


def test_write_runner_plan_reports_non_serializable_plan_without_stdout(
    tmp_path: Path, capsys
) -> None:
    errors = write_runner_plan({"schema": "example", "path": tmp_path})

    captured = capsys.readouterr()
    assert len(errors) == 1
    assert "failed to render runner plan JSON" in errors[0]
    assert "is not JSON serializable" in errors[0]
    assert captured.out == ""


def test_emit_runner_error_lines_writes_prefixed_stderr(capsys) -> None:
    emit_runner_error_lines(("one", "two"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: one\nERROR: two\n"


def test_emit_runner_error_block_writes_heading_and_bullets(capsys) -> None:
    emit_runner_error_block("ERROR: runner inputs are incomplete:", ("one", "two"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: runner inputs are incomplete:\n- one\n- two\n"


def test_emit_runner_notice_writes_stderr(capsys) -> None:
    emit_runner_notice("RUN step: command")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "RUN step: command\n"


def test_run_command_plan_reports_launch_failure(tmp_path: Path, capsys) -> None:
    exit_code = run_command_plan(
        [Step("missing", tmp_path / "missing.json", [str(tmp_path / "missing-bin")])],
        tmp_path / "out",
    )

    assert exit_code == 1
    captured = capsys.readouterr()
    assert "failed to launch" in captured.err


def test_run_command_plan_reports_artifact_inspection_failure(
    tmp_path: Path,
    capsys,
    monkeypatch,
) -> None:
    artifact = tmp_path / "artifact.json"
    original_is_file = Path.is_file

    def is_file(path: Path) -> bool:
        if path == artifact:
            raise OSError("artifact stat denied")
        return original_is_file(path)

    monkeypatch.setattr(Path, "is_file", is_file)

    exit_code = run_command_plan(
        [Step("gate", artifact, ["true"])],
        tmp_path / "out",
    )

    assert exit_code == 1
    captured = capsys.readouterr()
    assert "gate expected artifact" in captured.err
    assert "artifact stat denied" in captured.err


def test_run_command_plan_rejects_preexisting_artifact_symlink_before_create(
    tmp_path: Path,
    capsys,
) -> None:
    out_dir = tmp_path / "out"
    target = tmp_path / "target.json"
    target.write_text("{}", encoding="utf-8")
    artifact = tmp_path / "artifact.json"
    artifact.symlink_to(target)

    exit_code = run_command_plan([Step("gate", artifact, ["true"])], out_dir)

    assert exit_code == 1
    assert not out_dir.exists()
    captured = capsys.readouterr()
    assert f"gate artifact `{artifact}` must not be a symlink" in captured.err


def test_run_command_plan_rejects_artifact_symlink_written_by_command(
    tmp_path: Path,
    capsys,
) -> None:
    artifact = tmp_path / "artifact.json"
    target = tmp_path / "target.json"
    script = (
        "from pathlib import Path\n"
        "import sys\n"
        "Path(sys.argv[1]).write_text('{}', encoding='utf-8')\n"
        "Path(sys.argv[2]).symlink_to(Path(sys.argv[1]))\n"
    )

    exit_code = run_command_plan(
        [
            Step(
                "gate",
                artifact,
                [sys.executable, "-c", script, str(target), str(artifact)],
            )
        ],
        tmp_path / "out",
    )

    assert exit_code == 1
    captured = capsys.readouterr()
    assert (
        f"gate expected artifact `{artifact}` must not be a symlink"
        in captured.err
    )


def test_run_command_plan_rejects_empty_artifact_written_by_command(
    tmp_path: Path,
    capsys,
) -> None:
    artifact = tmp_path / "artifact.json"
    script = (
        "from pathlib import Path\n"
        "import sys\n"
        "Path(sys.argv[1]).write_bytes(b'')\n"
    )

    exit_code = run_command_plan(
        [Step("gate", artifact, [sys.executable, "-c", script, str(artifact)])],
        tmp_path / "out",
    )

    assert exit_code == 1
    captured = capsys.readouterr()
    assert f"gate wrote empty expected artifact `{artifact}`" in captured.err


def test_run_command_plan_rejects_artifact_same_as_out_dir_before_create(
    tmp_path: Path,
    capsys,
) -> None:
    out_dir = tmp_path / "evidence"

    exit_code = run_command_plan([Step("gate", out_dir, ["true"])], out_dir)

    assert exit_code == 1
    assert not out_dir.exists()
    captured = capsys.readouterr()
    assert "must not be the same path as reserved output" in captured.err
