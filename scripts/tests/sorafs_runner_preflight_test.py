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
    command_plan_steps,
    emit_runner_error_block,
    emit_runner_exception,
    emit_runner_error_lines,
    emit_runner_notice,
    inspect_runner_path_exists,
    inspect_runner_path_is_dir,
    inspect_runner_path_is_file,
    inspect_runner_path_is_symlink,
    inspect_runner_path_size,
    render_runner_plan,
    require_existing_dirs,
    require_existing_files,
    require_runner_non_negative_int,
    require_runner_positive_int,
    resolve_runner_input_file,
    resolve_runner_output_path,
    run_command_plan,
    runner_arg_label,
    validate_command_plan_artifacts,
    validate_command_plan_step_shapes,
    validate_runner_output_dir,
    validate_runner_output_parent,
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


def test_runner_arg_label_rejects_malformed_field_names() -> None:
    for field in ("", "max-route-latency-ms", "MaxRouteLatencyMs", "_limit", 7):
        try:
            runner_arg_label(field)
        except ValueError as error:
            assert "runner argument field must be a snake_case string" in str(error)
        else:
            raise AssertionError(f"accepted malformed field {field!r}")


def test_runner_path_resolution_uses_shared_identity_helper() -> None:
    assert (
        resolve_runner_input_file.__globals__["resolve_path_identity"].__module__
        == "sorafs_path_identity"
    )
    assert (
        resolve_runner_output_path.__globals__["resolve_path_identity"].__module__
        == "sorafs_path_identity"
    )


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


def test_require_runner_positive_int_rejects_malformed_error_container() -> None:
    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            require_runner_positive_int(
                argparse.Namespace(limit=1),
                "limit",
                errors,
            )
        except ValueError as error:
            assert "runner preflight errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


def test_require_runner_positive_int_rejects_malformed_existing_error_text() -> None:
    for errors in ([""], [" old"], ["old "], ["old\nerror"]):
        try:
            require_runner_positive_int(
                argparse.Namespace(limit=1),
                "limit",
                errors,
            )
        except ValueError as error:
            assert (
                "runner preflight errors must contain non-empty canonical strings"
                in str(error)
            )
        else:
            raise AssertionError(f"accepted malformed error text {errors!r}")


def test_require_runner_positive_int_rejects_malformed_field_name() -> None:
    for field in ("", "limit-ms", "LimitMs", "_limit", 7):
        errors: list[str] = []
        try:
            require_runner_positive_int(argparse.Namespace(limit=1), field, errors)
        except ValueError as error:
            assert "runner argument field must be a snake_case string" in str(error)
            assert errors == []
        else:
            raise AssertionError(f"accepted malformed field {field!r}")


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


def test_require_runner_non_negative_int_rejects_malformed_error_container() -> None:
    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            require_runner_non_negative_int(
                argparse.Namespace(max_evidence_age_secs=0),
                "max_evidence_age_secs",
                errors,
            )
        except ValueError as error:
            assert "runner preflight errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


def test_require_runner_non_negative_int_rejects_malformed_field_name() -> None:
    for field in ("", "max-evidence-age-secs", "MaxEvidenceAgeSecs", "_limit", 7):
        errors: list[str] = []
        try:
            require_runner_non_negative_int(
                argparse.Namespace(max_evidence_age_secs=0),
                field,
                errors,
            )
        except ValueError as error:
            assert "runner argument field must be a snake_case string" in str(error)
            assert errors == []
        else:
            raise AssertionError(f"accepted malformed field {field!r}")


def test_validate_runner_output_dir_rejects_non_path_without_traceback() -> None:
    errors: list[str] = []

    assert not validate_runner_output_dir("evidence", errors)
    assert errors == ["--out-dir `evidence` must be a path"]


def test_validate_runner_output_parent_rejects_non_path_without_traceback() -> None:
    errors: list[str] = []

    assert not validate_runner_output_parent(
        "summary.json",
        errors,
        label="--summary-out",
    )
    assert errors == ["--summary-out `summary.json` must be a path"]


def test_runner_preflight_sanitizes_malformed_non_path_targets(
    tmp_path: Path,
) -> None:
    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=" verifier.py",
            out_dir="evidence\nout",
            summary_out=None,
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == [
        "--verifier `<non-canonical-path>` must exist and be a file",
        "--out-dir `<non-canonical-path>` must be a path",
    ]

    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=tmp_path / "evidence",
            summary_out=b"summary.json",
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == ["--summary-out `<non-path>` must be a path"]


def test_runner_path_inspectors_sanitize_malformed_path_labels() -> None:
    for helper in (
        inspect_runner_path_exists,
        inspect_runner_path_is_symlink,
        inspect_runner_path_is_file,
        inspect_runner_path_size,
        inspect_runner_path_is_dir,
    ):
        for path, expected_label in (
            (" evidence", "<non-canonical-path>"),
            ("evidence\npath", "<non-canonical-path>"),
            (b"evidence", "<non-path>"),
            (7, "<non-path>"),
        ):
            errors: list[str] = []

            assert helper(path, errors, label="--out-dir") is None
            assert errors == [f"--out-dir `{expected_label}` must be a path"]


def test_runner_path_inspectors_sanitize_noncanonical_failures(
    tmp_path: Path,
    monkeypatch,
) -> None:
    target = tmp_path / "bad\npath"
    original_stat = Path.stat

    def fail_bool(path: Path) -> bool:
        if path == target:
            raise OSError(f"inspection denied for {path}")
        return False

    def fail_stat(path: Path, *args, **kwargs):
        if path == target:
            raise OSError(f"inspection denied for {path}")
        return original_stat(path, *args, **kwargs)

    for helper, attribute, replacement in (
        (inspect_runner_path_exists, "exists", fail_bool),
        (inspect_runner_path_is_symlink, "is_symlink", fail_bool),
        (inspect_runner_path_is_file, "is_file", fail_bool),
        (inspect_runner_path_is_dir, "is_dir", fail_bool),
        (inspect_runner_path_size, "stat", fail_stat),
    ):
        monkeypatch.setattr(Path, attribute, replacement)
        errors: list[str] = []

        assert helper(target, errors, label="--out-dir") is None
        assert errors == [
            "--out-dir `<non-canonical-path>` cannot be inspected: "
            "<non-canonical-error>"
        ]


def test_runner_path_inspectors_reject_malformed_error_container(
    tmp_path: Path,
) -> None:
    for helper in (
        inspect_runner_path_exists,
        inspect_runner_path_is_symlink,
        inspect_runner_path_is_file,
        inspect_runner_path_size,
        inspect_runner_path_is_dir,
    ):
        for errors in ("", (), {"error": "old"}, ["old", 7]):
            try:
                helper(tmp_path, errors, label="--out-dir")
            except ValueError as error:
                assert "runner preflight errors must be a list of strings" in str(
                    error
                )
            else:
                raise AssertionError(
                    f"{helper.__name__} accepted malformed errors {errors!r}"
                )


def test_runner_path_inspectors_reject_malformed_existing_error_text(
    tmp_path: Path,
) -> None:
    for helper in (
        inspect_runner_path_exists,
        inspect_runner_path_is_symlink,
        inspect_runner_path_is_file,
        inspect_runner_path_size,
        inspect_runner_path_is_dir,
    ):
        for errors in ([""], [" old"], ["old "], ["old\nerror"]):
            try:
                helper(tmp_path, errors, label="--out-dir")
            except ValueError as error:
                assert (
                    "runner preflight errors must contain non-empty canonical strings"
                    in str(error)
                )
            else:
                raise AssertionError(
                    f"{helper.__name__} accepted malformed error text {errors!r}"
                )


def test_runner_path_inspectors_reject_malformed_labels(tmp_path: Path) -> None:
    for helper in (
        inspect_runner_path_exists,
        inspect_runner_path_is_symlink,
        inspect_runner_path_is_file,
        inspect_runner_path_size,
        inspect_runner_path_is_dir,
    ):
        for label in ("", " --out-dir", "--out-dir ", "--out\nDir", 7):
            errors: list[str] = []
            try:
                helper(tmp_path, errors, label=label)
            except ValueError as error:
                assert "runner preflight label must be a non-empty canonical string" in str(
                    error
                )
                assert errors == []
            else:
                raise AssertionError(
                    f"{helper.__name__} accepted malformed label {label!r}"
                )


def test_validate_runner_output_parent_rejects_malformed_error_container(
    tmp_path: Path,
) -> None:
    out_dir = tmp_path / "out"

    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            validate_runner_output_parent(out_dir, errors, label="--out-dir")
        except ValueError as error:
            assert "runner preflight errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


def test_validate_runner_output_parent_rejects_malformed_label(
    tmp_path: Path,
) -> None:
    out_dir = tmp_path / "out"

    for label in ("", " --out-dir", "--out-dir ", "--out\nDir", 7):
        errors: list[str] = []
        try:
            validate_runner_output_parent(out_dir, errors, label=label)
        except ValueError as error:
            assert "runner preflight label must be a non-empty canonical string" in str(
                error
            )
            assert errors == []
        else:
            raise AssertionError(f"accepted malformed label {label!r}")


def test_validate_runner_output_dir_rejects_malformed_error_container(
    tmp_path: Path,
) -> None:
    out_dir = tmp_path / "out"

    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            validate_runner_output_dir(out_dir, errors)
        except ValueError as error:
            assert "runner preflight errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


def test_validate_runner_output_dir_rejects_malformed_label(tmp_path: Path) -> None:
    out_dir = tmp_path / "out"

    for label in ("", " --out-dir", "--out-dir ", "--out\nDir", 7):
        errors: list[str] = []
        try:
            validate_runner_output_dir(out_dir, errors, label=label)
        except ValueError as error:
            assert "runner preflight label must be a non-empty canonical string" in str(
                error
            )
            assert errors == []
        else:
            raise AssertionError(f"accepted malformed label {label!r}")


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


def test_out_dir_symlink_fails_preflight(tmp_path: Path) -> None:
    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    target = tmp_path / "target"
    target.mkdir()
    out_dir = tmp_path / "evidence"
    out_dir.symlink_to(target, target_is_directory=True)

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=out_dir,
            summary_out=tmp_path / "summary.json",
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == [f"--out-dir `{out_dir}` must not be a symlink"]


def test_out_dir_parent_chain_symlink_fails_preflight(tmp_path: Path) -> None:
    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    target = tmp_path / "target"
    target.mkdir()
    ancestor = tmp_path / "redirect"
    ancestor.symlink_to(target, target_is_directory=True)
    out_dir = ancestor / "nested" / "evidence"

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=out_dir,
            summary_out=tmp_path / "summary.json",
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == [f"--out-dir parent `{ancestor}` must not be a symlink"]


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


def test_summary_out_symlink_fails_preflight(tmp_path: Path) -> None:
    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    target = tmp_path / "target-summary.json"
    target.write_text("{}", encoding="utf-8")
    summary = tmp_path / "summary.json"
    summary.symlink_to(target)

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=tmp_path / "evidence",
            summary_out=summary,
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == [f"--summary-out `{summary}` must not be a symlink"]


def test_summary_out_parent_symlink_fails_preflight(tmp_path: Path) -> None:
    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    target = tmp_path / "target"
    target.mkdir()
    parent = tmp_path / "summary-parent"
    parent.symlink_to(target, target_is_directory=True)

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=tmp_path / "evidence",
            summary_out=parent / "summary.json",
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == [f"--summary-out parent `{parent}` must not be a symlink"]


def test_summary_out_parent_chain_symlink_fails_preflight(tmp_path: Path) -> None:
    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    target = tmp_path / "target"
    target.mkdir()
    ancestor = tmp_path / "summary-root"
    ancestor.symlink_to(target, target_is_directory=True)

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=tmp_path / "evidence",
            summary_out=ancestor / "nested" / "summary.json",
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == [f"--summary-out parent `{ancestor}` must not be a symlink"]


def test_summary_out_parent_chain_file_fails_preflight(tmp_path: Path) -> None:
    verifier = tmp_path / "verifier.py"
    verifier.write_text("", encoding="utf-8")
    ancestor = tmp_path / "not-a-directory"
    ancestor.write_text("", encoding="utf-8")

    errors = validate_runner_preflight(
        argparse.Namespace(
            verifier=verifier,
            out_dir=tmp_path / "evidence",
            summary_out=ancestor / "nested" / "summary.json",
        ),
        summary_filename="rollout-summary.json",
    )

    assert errors == [
        f"--summary-out parent `{ancestor}` must be a directory when it exists"
    ]


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


def test_missing_input_file_sanitizes_noncanonical_path() -> None:
    errors = require_existing_files([Path("missing\nfile.json")], "--evidence")

    assert errors == [
        "--evidence `<non-canonical-path>` must exist and be a file"
    ]


def test_input_file_rejects_non_path_without_traceback() -> None:
    errors = require_existing_files(["evidence.json"], "--evidence")

    assert errors == ["--evidence `evidence.json` must be a path"]


def test_input_file_rejects_scalar_and_mapping_path_collections() -> None:
    for paths in ("evidence.json", b"evidence.json", {"path": "evidence.json"}):
        errors = require_existing_files(paths, "--evidence")

        assert errors == ["--evidence paths must be a sequence"]


def test_input_file_rejects_malformed_label(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"

    for label in ("", " --evidence", "--evidence ", "--evidence\npath", 7):
        try:
            require_existing_files([evidence], label)
        except ValueError as error:
            assert "runner preflight label must be a non-empty canonical string" in str(
                error
            )
        else:
            raise AssertionError(f"accepted malformed label {label!r}")


def test_input_file_rejects_malformed_seen_identity_map(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    for seen in (
        "seen",
        {tmp_path: "not-a-pair"},
        {"not-a-path": ("--previous", evidence)},
        {tmp_path: (" previous", evidence)},
        {tmp_path: ("--previous", "evidence.json")},
    ):
        errors = require_existing_files([evidence], "--evidence", seen=seen)

        assert len(errors) == 1
        assert "--evidence identity map" in errors[0]


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


def test_missing_input_directory_sanitizes_noncanonical_path() -> None:
    errors = require_existing_dirs([Path("missing\nbundle")], "--bundle")

    assert errors == [
        "--bundle `<non-canonical-path>` must exist and be a directory"
    ]


def test_input_directory_rejects_non_path_without_traceback() -> None:
    errors = require_existing_dirs(["bundle"], "--bundle")

    assert errors == ["--bundle `bundle` must be a path"]


def test_input_directory_rejects_scalar_and_mapping_path_collections() -> None:
    for paths in ("bundle", b"bundle", {"path": "bundle"}):
        errors = require_existing_dirs(paths, "--bundle")

        assert errors == ["--bundle paths must be a sequence"]


def test_input_directory_rejects_malformed_label(tmp_path: Path) -> None:
    bundle = tmp_path / "bundle"

    for label in ("", " --bundle", "--bundle ", "--bundle\npath", 7):
        try:
            require_existing_dirs([bundle], label)
        except ValueError as error:
            assert "runner preflight label must be a non-empty canonical string" in str(
                error
            )
        else:
            raise AssertionError(f"accepted malformed label {label!r}")


def test_input_directory_rejects_malformed_seen_identity_map(tmp_path: Path) -> None:
    bundle = tmp_path / "bundle"
    bundle.mkdir()

    for seen in (
        "seen",
        {tmp_path: "not-a-pair"},
        {"not-a-path": ("--previous", bundle)},
        {tmp_path: (" previous", bundle)},
        {tmp_path: ("--previous", "bundle")},
    ):
        errors = require_existing_dirs([bundle], "--bundle", seen=seen)

        assert len(errors) == 1
        assert "--bundle identity map" in errors[0]


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


def test_command_plan_steps_rejects_scalar_and_mapping_containers() -> None:
    steps = [Step("gate", None, ["true"])]

    assert command_plan_steps(steps) is steps
    assert command_plan_steps("gate") is None
    assert command_plan_steps(b"gate") is None
    assert command_plan_steps({"label": "gate"}) is None
    assert command_plan_steps(None) is None


def test_validate_command_plan_artifacts_rejects_malformed_plan_shapes() -> None:
    assert validate_command_plan_artifacts("gate") == [
        "command plan must be a sequence of steps"
    ]
    assert validate_command_plan_artifacts({"label": "gate"}) == [
        "command plan must be a sequence of steps"
    ]


def test_validate_command_plan_step_shapes_rejects_malformed_fields(
    tmp_path: Path,
) -> None:
    errors = validate_command_plan_step_shapes(
        [
            Step("empty_command", None, []),
            Step("empty_executable", None, [""]),
            Step("blank_executable", None, [" "]),
            Step("nul_arg", None, ["true", "bad\0arg"]),
            Step("newline_arg", None, ["true", "bad\narg"]),
            argparse.Namespace(label=" bad_label", artifact=None, command=["true"]),
            argparse.Namespace(
                label="bad_artifact",
                artifact="summary.json",
                command=["true"],
            ),
            argparse.Namespace(
                label="tuple_command",
                artifact=None,
                command=("true",),
            ),
            argparse.Namespace(
                label="non_string_command",
                artifact=tmp_path / "artifact.json",
                command=["true", 7],
            ),
        ]
    )

    assert errors == [
        "empty_command command must be a non-empty list of strings",
        "empty_executable command executable must be a non-empty canonical string",
        "blank_executable command executable must be a non-empty canonical string",
        "nul_arg command argument 1 must not contain NUL bytes",
        "newline_arg command argument 1 must not contain control characters",
        "command-plan step 5 label must be a non-empty canonical string",
        "bad_artifact artifact `summary.json` must be a path",
        "tuple_command command must be a non-empty list of strings",
        "non_string_command command must be a non-empty list of strings",
    ]


def test_validate_command_plan_step_shapes_sanitizes_malformed_artifact_labels() -> None:
    errors = validate_command_plan_step_shapes(
        [
            argparse.Namespace(
                label="bad_artifact",
                artifact=" summary\njson",
                command=["true"],
            )
        ]
    )

    assert errors == [
        "bad_artifact artifact `<non-canonical-path>` must be a path"
    ]


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


def test_validate_command_plan_artifacts_rejects_malformed_reserved_outputs(
    tmp_path: Path,
) -> None:
    plan = [Step("gate", tmp_path / "artifact.json", ["true"])]

    assert validate_command_plan_artifacts(
        plan,
        reserved_output_paths="out",
    ) == ["reserved output paths must be a sequence"]
    assert validate_command_plan_artifacts(
        plan,
        reserved_output_paths={"out": tmp_path / "out"},
    ) == ["reserved output paths must be a sequence"]
    assert validate_command_plan_artifacts(
        plan,
        reserved_output_paths=[tmp_path / "out", "summary.json"],
    ) == ["reserved output path `summary.json` must be a path"]
    assert validate_command_plan_artifacts(
        plan,
        reserved_output_paths=[tmp_path / "out", " summary\njson"],
    ) == ["reserved output path `<non-canonical-path>` must be a path"]


def test_validate_command_plan_artifacts_rejects_duplicate_reserved_outputs(
    tmp_path: Path,
) -> None:
    out_dir = tmp_path / "out"
    alias = tmp_path / "nested" / ".." / "out"

    errors = validate_command_plan_artifacts(
        [Step("gate", tmp_path / "artifact.json", ["true"])],
        reserved_output_paths=(out_dir, alias),
    )

    assert errors == [
        f"duplicate reserved output path `{alias}` matches `{out_dir}`"
    ]


def test_validate_command_plan_artifacts_stops_after_reserved_output_errors(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target.json"
    target.write_text("{}", encoding="utf-8")
    artifact = tmp_path / "artifact.json"
    artifact.symlink_to(target)

    errors = validate_command_plan_artifacts(
        [Step("gate", artifact, ["true"])],
        reserved_output_paths=["summary.json"],
    )

    assert errors == ["reserved output path `summary.json` must be a path"]


def test_planned_artifact_symlink_fails(tmp_path: Path) -> None:
    target = tmp_path / "target.json"
    target.write_text("{}", encoding="utf-8")
    artifact = tmp_path / "artifact.json"
    artifact.symlink_to(target)

    errors = validate_command_plan_artifacts([Step("gate", artifact, ["true"])])

    assert errors == [f"gate artifact `{artifact}` must not be a symlink"]


def test_planned_artifact_existing_file_fails(tmp_path: Path) -> None:
    artifact = tmp_path / "artifact.json"
    artifact.write_text("stale", encoding="utf-8")

    errors = validate_command_plan_artifacts([Step("gate", artifact, ["true"])])

    assert errors == [f"gate artifact `{artifact}` must not already exist"]


def test_planned_artifact_parent_symlink_fails(tmp_path: Path) -> None:
    target = tmp_path / "target"
    target.mkdir()
    parent = tmp_path / "artifact-parent"
    parent.symlink_to(target, target_is_directory=True)
    artifact = parent / "artifact.json"

    errors = validate_command_plan_artifacts([Step("gate", artifact, ["true"])])

    assert errors == [f"gate artifact parent `{parent}` must not be a symlink"]


def test_planned_artifact_parent_chain_symlink_fails(tmp_path: Path) -> None:
    target = tmp_path / "target"
    target.mkdir()
    ancestor = tmp_path / "artifact-root"
    ancestor.symlink_to(target, target_is_directory=True)
    artifact = ancestor / "nested" / "artifact.json"

    errors = validate_command_plan_artifacts([Step("gate", artifact, ["true"])])

    assert errors == [f"gate artifact parent `{ancestor}` must not be a symlink"]


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


def test_render_runner_plan_rejects_non_object_plan() -> None:
    for plan in (["step"], "step", 7):
        try:
            render_runner_plan(plan)
        except ValueError as error:
            assert "runner plan must be an object" in str(error)
        else:
            raise AssertionError(f"accepted non-object runner plan {plan!r}")


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


def test_write_runner_plan_reports_non_object_plan_without_stdout(capsys) -> None:
    errors = write_runner_plan(["step"])

    captured = capsys.readouterr()
    assert errors == ["failed to render runner plan JSON: runner plan must be an object"]
    assert captured.out == ""


def test_write_runner_plan_sanitizes_malformed_render_error(
    capsys,
    monkeypatch,
) -> None:
    def render_raises(_plan):
        raise ValueError("render failed\nsecret")

    monkeypatch.setitem(
        write_runner_plan.__globals__,
        "render_runner_plan",
        render_raises,
    )

    errors = write_runner_plan({"schema": "example"})

    captured = capsys.readouterr()
    assert errors == ["failed to render runner plan JSON: <non-canonical-error>"]
    assert captured.out == ""


def test_emit_runner_error_lines_writes_prefixed_stderr(capsys) -> None:
    emit_runner_error_lines(("one", "two"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: one\nERROR: two\n"


def test_emit_runner_error_lines_rejects_malformed_messages(capsys) -> None:
    for errors in ("error", b"error", {"error": "old"}, ["old", 7], None):
        try:
            emit_runner_error_lines(errors)
        except ValueError as error:
            assert "runner error messages must be a sequence of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed messages {errors!r}")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == ""


def test_emit_runner_error_lines_rejects_malformed_message_content(capsys) -> None:
    for errors in ([""], [" old"], ["old "], ["old\nerror"]):
        try:
            emit_runner_error_lines(errors)
        except ValueError as error:
            assert "runner error message must be a non-empty canonical string" in str(
                error
            )
        else:
            raise AssertionError(f"accepted malformed message content {errors!r}")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == ""


def test_emit_runner_exception_sanitizes_malformed_message(capsys) -> None:
    emit_runner_exception(ValueError("bad\nargument"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: <non-canonical-error>\n"


def test_emit_runner_exception_preserves_canonical_message(capsys) -> None:
    emit_runner_exception(ValueError("bad argument"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: bad argument\n"


def test_emit_runner_error_block_writes_heading_and_bullets(capsys) -> None:
    emit_runner_error_block("ERROR: runner inputs are incomplete:", ("one", "two"))

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: runner inputs are incomplete:\n- one\n- two\n"


def test_emit_runner_error_block_rejects_malformed_messages_before_heading(
    capsys,
) -> None:
    for errors in ("error", b"error", {"error": "old"}, ["old", 7], None):
        try:
            emit_runner_error_block("ERROR: runner inputs are incomplete:", errors)
        except ValueError as error:
            assert "runner error messages must be a sequence of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed messages {errors!r}")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == ""


def test_emit_runner_error_block_rejects_malformed_message_content_before_heading(
    capsys,
) -> None:
    for errors in ([""], [" old"], ["old "], ["old\nerror"]):
        try:
            emit_runner_error_block("ERROR: runner inputs are incomplete:", errors)
        except ValueError as error:
            assert "runner error message must be a non-empty canonical string" in str(
                error
            )
        else:
            raise AssertionError(f"accepted malformed message content {errors!r}")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == ""


def test_emit_runner_notice_writes_stderr(capsys) -> None:
    emit_runner_notice("RUN step: command")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "RUN step: command\n"


def test_emit_runner_notice_rejects_malformed_message(capsys) -> None:
    for message in ("", " RUN step", "RUN step\nnext", 7):
        try:
            emit_runner_notice(message)
        except ValueError as error:
            assert "runner notice message must be a non-empty canonical string" in str(
                error
            )
        else:
            raise AssertionError(f"accepted malformed notice {message!r}")

    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == ""


def test_run_command_plan_reports_launch_failure(tmp_path: Path, capsys) -> None:
    exit_code = run_command_plan(
        [Step("missing", tmp_path / "missing.json", [str(tmp_path / "missing-bin")])],
        tmp_path / "out",
    )

    assert exit_code == 1
    captured = capsys.readouterr()
    assert "failed to launch" in captured.err


def test_run_command_plan_sanitizes_launch_failure(
    tmp_path: Path,
    capsys,
    monkeypatch,
) -> None:
    def run_raises(*_args, **_kwargs):
        raise OSError("launch denied\nsecret")

    monkeypatch.setattr("sorafs_runner_preflight.subprocess.run", run_raises)

    exit_code = run_command_plan([Step("gate", None, ["true"])], tmp_path / "out")

    assert exit_code == 1
    captured = capsys.readouterr()
    assert captured.out == ""
    assert (
        "RUN gate: true\n"
        "ERROR: gate failed to launch: <non-canonical-error>\n"
    ) == captured.err


def test_run_command_plan_sanitizes_output_creation_failure(
    tmp_path: Path,
    capsys,
    monkeypatch,
) -> None:
    out_dir = tmp_path / "bad\nout"
    original_mkdir = Path.mkdir

    def mkdir(path: Path, *args, **kwargs):
        if path == out_dir:
            raise OSError(f"mkdir denied for {path}")
        return original_mkdir(path, *args, **kwargs)

    monkeypatch.setattr(Path, "mkdir", mkdir)

    exit_code = run_command_plan([Step("gate", None, ["true"])], out_dir)

    assert exit_code == 1
    captured = capsys.readouterr()
    assert captured.out == ""
    assert (
        "ERROR: failed to create --out-dir `<non-canonical-path>`: "
        "<non-canonical-error>\n"
    ) == captured.err


def test_run_command_plan_rejects_malformed_plan_before_output_creation(
    tmp_path: Path,
    capsys,
) -> None:
    out_dir = tmp_path / "out"

    exit_code = run_command_plan("gate", out_dir)

    assert exit_code == 1
    assert not out_dir.exists()
    captured = capsys.readouterr()
    assert captured.out == ""
    assert captured.err == "ERROR: command plan must be a sequence of steps\n"


def test_run_command_plan_rejects_malformed_step_before_output_creation(
    tmp_path: Path,
    capsys,
) -> None:
    out_dir = tmp_path / "out"

    exit_code = run_command_plan([Step("gate", None, [])], out_dir)

    assert exit_code == 1
    assert not out_dir.exists()
    captured = capsys.readouterr()
    assert captured.out == ""
    assert (
        captured.err
        == "ERROR: gate command must be a non-empty list of strings\n"
    )


def test_run_command_plan_rejects_malformed_command_entries_before_output_creation(
    tmp_path: Path,
    capsys,
) -> None:
    cases = (
        ([""], "ERROR: gate command executable must be a non-empty canonical string\n"),
        (
            [" "],
            "ERROR: gate command executable must be a non-empty canonical string\n",
        ),
        (
            ["true", "bad\0arg"],
            "ERROR: gate command argument 1 must not contain NUL bytes\n",
        ),
        (
            ["true", "bad\narg"],
            "ERROR: gate command argument 1 must not contain control characters\n",
        ),
    )

    for index, (command, expected_error) in enumerate(cases):
        out_dir = tmp_path / f"out-{index}"

        exit_code = run_command_plan([Step("gate", None, command)], out_dir)

        assert exit_code == 1
        assert not out_dir.exists()
        captured = capsys.readouterr()
        assert captured.out == ""
        assert captured.err == expected_error


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


def test_run_command_plan_rejects_preexisting_artifact_before_create(
    tmp_path: Path,
    capsys,
) -> None:
    out_dir = tmp_path / "out"
    artifact = tmp_path / "artifact.json"
    launched = tmp_path / "launched"
    artifact.write_text("stale", encoding="utf-8")
    script = (
        "from pathlib import Path; import sys; "
        "Path(sys.argv[1]).write_text('launched', encoding='utf-8')"
    )

    exit_code = run_command_plan(
        [Step("gate", artifact, [sys.executable, "-c", script, str(launched)])],
        out_dir,
    )

    assert exit_code == 1
    assert not out_dir.exists()
    assert not launched.exists()
    captured = capsys.readouterr()
    assert f"gate artifact `{artifact}` must not already exist" in captured.err


def test_run_command_plan_rejects_output_dir_symlink_before_launch(
    tmp_path: Path,
    capsys,
) -> None:
    target = tmp_path / "target"
    target.mkdir()
    out_dir = tmp_path / "out"
    out_dir.symlink_to(target, target_is_directory=True)
    launched = tmp_path / "launched"
    script = (
        "from pathlib import Path; import sys; "
        "Path(sys.argv[1]).write_text('launched', encoding='utf-8')"
    )

    exit_code = run_command_plan(
        [Step("gate", None, [sys.executable, "-c", script, str(launched)])],
        out_dir,
    )

    assert exit_code == 1
    assert not launched.exists()
    captured = capsys.readouterr()
    assert f"--out-dir `{out_dir}` must not be a symlink" in captured.err


def test_run_command_plan_rejects_artifact_parent_symlink_before_create(
    tmp_path: Path,
    capsys,
) -> None:
    out_dir = tmp_path / "out"
    target = tmp_path / "target"
    target.mkdir()
    parent = tmp_path / "artifact-parent"
    parent.symlink_to(target, target_is_directory=True)
    artifact = parent / "artifact.json"
    launched = tmp_path / "launched"
    script = (
        "from pathlib import Path; import sys; "
        "Path(sys.argv[1]).write_text('launched', encoding='utf-8')"
    )

    exit_code = run_command_plan(
        [Step("gate", artifact, [sys.executable, "-c", script, str(launched)])],
        out_dir,
    )

    assert exit_code == 1
    assert not out_dir.exists()
    assert not launched.exists()
    captured = capsys.readouterr()
    assert f"gate artifact parent `{parent}` must not be a symlink" in captured.err


def test_run_command_plan_rejects_artifact_parent_chain_symlink_before_create(
    tmp_path: Path,
    capsys,
) -> None:
    out_dir = tmp_path / "out"
    target = tmp_path / "target"
    target.mkdir()
    ancestor = tmp_path / "artifact-root"
    ancestor.symlink_to(target, target_is_directory=True)
    artifact = ancestor / "nested" / "artifact.json"
    launched = tmp_path / "launched"
    script = (
        "from pathlib import Path; import sys; "
        "Path(sys.argv[1]).write_text('launched', encoding='utf-8')"
    )

    exit_code = run_command_plan(
        [Step("gate", artifact, [sys.executable, "-c", script, str(launched)])],
        out_dir,
    )

    assert exit_code == 1
    assert not out_dir.exists()
    assert not launched.exists()
    captured = capsys.readouterr()
    assert f"gate artifact parent `{ancestor}` must not be a symlink" in captured.err


def test_run_command_plan_rejects_artifact_symlink_written_by_command(
    tmp_path: Path,
    capsys,
) -> None:
    artifact = tmp_path / "artifact.json"
    target = tmp_path / "target.json"
    script = (
        "from pathlib import Path; import sys; "
        "Path(sys.argv[1]).write_text('{}', encoding='utf-8'); "
        "Path(sys.argv[2]).symlink_to(Path(sys.argv[1]))"
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
    script = "from pathlib import Path; import sys; Path(sys.argv[1]).write_bytes(b'')"

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
