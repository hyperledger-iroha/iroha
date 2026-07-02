"""Tests for shared SoraFS evidence path discovery."""

from __future__ import annotations

import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_evidence_paths import (  # noqa: E402
    EVIDENCE_DIRECTORY_INSPECTION_DIAGNOSTIC,
    EVIDENCE_DIRECTORY_MISSING_DIAGNOSTIC,
    EVIDENCE_DIRECTORY_PARENT_INSPECTION_DIAGNOSTIC,
    EVIDENCE_DIRECTORY_PATH_DIAGNOSTIC,
    EVIDENCE_DIRECTORY_SCAN_DIAGNOSTIC,
    EVIDENCE_DIRECTORY_SYMLINK_DIAGNOSTIC,
    EVIDENCE_FILE_DUPLICATE_DISCOVERED_DIAGNOSTIC,
    EVIDENCE_FILE_DUPLICATE_EXPLICIT_DIAGNOSTIC,
    EVIDENCE_FILE_INSPECTION_DIAGNOSTIC,
    EVIDENCE_FILE_MISSING_DIAGNOSTIC,
    EVIDENCE_FILE_PARENT_INSPECTION_DIAGNOSTIC,
    EVIDENCE_FILE_PATH_DIAGNOSTIC,
    EVIDENCE_FILE_RESERVED_CONFLICT_DIAGNOSTIC,
    EVIDENCE_FILE_SOURCE_OVERLAP_DIAGNOSTIC,
    EVIDENCE_FILE_SYMLINK_DIAGNOSTIC,
    discover_evidence_files,
    evidence_path_collection,
    evidence_path_identities,
    inspect_evidence_directory,
    inspect_evidence_file,
    is_explicit_evidence_path,
    record_reserved_output_evidence_conflicts,
    reserved_output_path_identities,
    resolve_evidence_path,
    scan_evidence_directory_json,
)


def test_discovers_explicit_then_directory_files(tmp_path: Path) -> None:
    explicit = tmp_path / "explicit.json"
    directory_file = tmp_path / "nested" / "directory.json"
    directory_file.parent.mkdir()
    explicit.write_text("{}", encoding="utf-8")
    directory_file.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([tmp_path / "nested"], [explicit], errors)

    assert files == [explicit, directory_file]
    assert errors == []


def test_duplicate_explicit_evidence_path_fails(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([], [evidence, evidence], errors)

    assert files == []
    assert len(errors) == 1
    assert errors == [EVIDENCE_FILE_DUPLICATE_EXPLICIT_DIAGNOSTIC]
    assert str(evidence) not in errors[0]


def test_explicit_directory_overlap_fails(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([tmp_path], [evidence], errors)

    assert files == []
    assert len(errors) == 1
    assert errors == [EVIDENCE_FILE_SOURCE_OVERLAP_DIAGNOSTIC]
    assert str(evidence) not in errors[0]


def test_overlapping_directories_fail(tmp_path: Path) -> None:
    nested = tmp_path / "nested"
    nested.mkdir()
    evidence = nested / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([tmp_path, nested], [], errors)

    assert files == []
    assert len(errors) == 1
    assert errors == [EVIDENCE_FILE_DUPLICATE_DISCOVERED_DIAGNOSTIC]
    assert str(evidence) not in errors[0]


def test_duplicate_evidence_identity_does_not_hide_distinct_files(
    tmp_path: Path,
) -> None:
    duplicate = tmp_path / "duplicate.json"
    distinct = tmp_path / "distinct.json"
    duplicate.write_text("{}", encoding="utf-8")
    distinct.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([tmp_path], [duplicate], errors)

    assert files == [distinct]
    assert len(errors) == 1
    assert errors == [EVIDENCE_FILE_SOURCE_OVERLAP_DIAGNOSTIC]
    assert str(duplicate) not in errors[0]


def test_explicit_evidence_conflicting_with_reserved_output_fails(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files(
        [],
        [evidence],
        errors,
        reserved_output_paths=[tmp_path / "nested" / ".." / "evidence.json"],
    )

    assert files == []
    assert len(errors) == 1
    assert errors == [EVIDENCE_FILE_RESERVED_CONFLICT_DIAGNOSTIC]
    assert str(evidence) not in errors[0]


def test_discovered_evidence_conflicting_with_reserved_output_fails(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files(
        [tmp_path],
        [],
        errors,
        reserved_output_paths=[tmp_path / "nested" / ".." / "evidence.json"],
    )

    assert files == []
    assert len(errors) == 1
    assert errors == [EVIDENCE_FILE_RESERVED_CONFLICT_DIAGNOSTIC]
    assert str(evidence) not in errors[0]


def test_missing_explicit_evidence_file_fails_closed(tmp_path: Path) -> None:
    missing = tmp_path / "missing.json"

    errors: list[str] = []
    files = discover_evidence_files([], [missing], errors)

    assert files == []
    assert errors == [EVIDENCE_FILE_MISSING_DIAGNOSTIC]
    assert str(missing) not in errors[0]


def test_explicit_evidence_directory_fails_closed(tmp_path: Path) -> None:
    evidence_dir = tmp_path / "evidence.json"
    evidence_dir.mkdir()

    errors: list[str] = []
    files = discover_evidence_files([], [evidence_dir], errors)

    assert files == []
    assert errors == [EVIDENCE_FILE_MISSING_DIAGNOSTIC]
    assert str(evidence_dir) not in errors[0]


def test_explicit_evidence_symlink_fails_closed(tmp_path: Path) -> None:
    target = tmp_path / "target.json"
    symlink = tmp_path / "evidence.json"
    target.write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    files = discover_evidence_files([], [symlink], errors)

    assert files == []
    assert errors == [EVIDENCE_FILE_SYMLINK_DIAGNOSTIC]
    assert str(symlink) not in errors[0]


def test_explicit_evidence_parent_symlink_directory_is_accepted(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target"
    parent = tmp_path / "evidence-root"
    target.mkdir()
    (target / "evidence.json").write_text("{}", encoding="utf-8")
    parent.symlink_to(target, target_is_directory=True)
    evidence = parent / "evidence.json"

    errors: list[str] = []
    files = discover_evidence_files([], [evidence], errors)

    assert files == [evidence]
    assert errors == []


def test_discovered_json_directory_fails_closed_without_hiding_files(
    tmp_path: Path,
) -> None:
    evidence_dir = tmp_path / "evidence"
    evidence_dir.mkdir()
    json_dir = evidence_dir / "not-a-file.json"
    regular_file = evidence_dir / "regular.json"
    json_dir.mkdir()
    regular_file.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([evidence_dir], [], errors)

    assert files == [regular_file]
    assert errors == [EVIDENCE_FILE_MISSING_DIAGNOSTIC]
    assert str(json_dir) not in errors[0]


def test_discovered_json_symlink_fails_closed_without_hiding_files(
    tmp_path: Path,
) -> None:
    evidence_dir = tmp_path / "evidence"
    evidence_dir.mkdir()
    target = evidence_dir / "target.json"
    symlink = evidence_dir / "link.json"
    regular_file = evidence_dir / "regular.json"
    target.write_text("{}", encoding="utf-8")
    regular_file.write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    files = discover_evidence_files([evidence_dir], [], errors)

    assert files == [regular_file, target]
    assert errors == [EVIDENCE_FILE_SYMLINK_DIAGNOSTIC]
    assert str(symlink) not in errors[0]


def test_noncanonical_missing_explicit_evidence_file_label_is_sanitized(
    tmp_path: Path,
) -> None:
    errors: list[str] = []
    files = discover_evidence_files([], [tmp_path / "bad\npath.json"], errors)

    assert files == []
    assert errors == [EVIDENCE_FILE_MISSING_DIAGNOSTIC]


def test_missing_evidence_directory_fails_closed(tmp_path: Path) -> None:
    missing = tmp_path / "missing"

    errors: list[str] = []
    files = discover_evidence_files([missing], [], errors)

    assert files == []
    assert errors == [EVIDENCE_DIRECTORY_MISSING_DIAGNOSTIC]
    assert str(missing) not in errors[0]


def test_evidence_directory_symlink_fails_closed(tmp_path: Path) -> None:
    target = tmp_path / "target"
    symlink = tmp_path / "evidence"
    target.mkdir()
    (target / "evidence.json").write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    files = discover_evidence_files([symlink], [], errors)

    assert files == []
    assert errors == [EVIDENCE_DIRECTORY_SYMLINK_DIAGNOSTIC]
    assert str(symlink) not in errors[0]


def test_evidence_directory_parent_symlink_directory_is_accepted(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target"
    parent = tmp_path / "evidence-root"
    target.mkdir()
    (target / "evidence").mkdir()
    (target / "evidence" / "evidence.json").write_text("{}", encoding="utf-8")
    parent.symlink_to(target, target_is_directory=True)
    evidence_dir = parent / "evidence"

    errors: list[str] = []
    files = discover_evidence_files([evidence_dir], [], errors)

    assert files == [evidence_dir / "evidence.json"]
    assert errors == []


def test_non_path_evidence_directory_fails_closed_without_traceback() -> None:
    errors: list[str] = []
    files = discover_evidence_files(["reviewed"], [], errors)

    assert files == []
    assert errors == [EVIDENCE_DIRECTORY_PATH_DIAGNOSTIC]


def test_noncanonical_evidence_file_labels_are_sanitized() -> None:
    for path_value, expected_label in (
        (" reviewed", "<non-canonical-path>"),
        ("reviewed\npath", "<non-canonical-path>"),
        (b"reviewed", "<non-path>"),
    ):
        errors: list[str] = []
        assert inspect_evidence_file(path_value, errors) is None
        assert errors == [EVIDENCE_FILE_PATH_DIAGNOSTIC]
        assert expected_label not in errors[0]


def test_noncanonical_evidence_directory_labels_are_sanitized() -> None:
    for directory, expected_label in (
        (" reviewed", "<non-canonical-path>"),
        ("reviewed\npath", "<non-canonical-path>"),
        (b"reviewed", "<non-path>"),
    ):
        errors: list[str] = []
        assert inspect_evidence_directory(directory, errors) is None
        assert errors == [EVIDENCE_DIRECTORY_PATH_DIAGNOSTIC]
        assert expected_label not in errors[0]


def test_noncanonical_missing_evidence_directory_label_is_sanitized(
    tmp_path: Path,
) -> None:
    errors: list[str] = []
    files = discover_evidence_files([tmp_path / "bad\npath"], [], errors)

    assert files == []
    assert errors == [EVIDENCE_DIRECTORY_MISSING_DIAGNOSTIC]


def test_evidence_path_collection_rejects_scalar_and_mapping_containers() -> None:
    errors: list[str] = []

    assert evidence_path_collection("abc", errors, label="evidence file") is None
    assert evidence_path_collection(b"abc", errors, label="evidence file") is None
    assert evidence_path_collection(
        {"evidence": "file.json"},
        errors,
        label="evidence file",
    ) is None

    assert errors == [
        "evidence file paths must be a sequence",
        "evidence file paths must be a sequence",
        "evidence file paths must be a sequence",
    ]


def test_evidence_path_collection_rejects_malformed_error_container() -> None:
    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            evidence_path_collection([], errors, label="evidence file")
        except ValueError as error:
            assert "evidence path errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


def test_evidence_path_collection_rejects_malformed_existing_error_text() -> None:
    for errors in ([""], [" old"], ["old "], ["old\nerror"]):
        try:
            evidence_path_collection([], errors, label="evidence file")
        except ValueError as error:
            assert (
                "evidence path errors must contain non-empty canonical strings"
                in str(error)
            )
        else:
            raise AssertionError(f"accepted malformed error text {errors!r}")


def test_evidence_path_collection_rejects_malformed_labels() -> None:
    for label in ("", " evidence", "evidence ", "evidence\nfile", 7):
        errors: list[str] = []
        try:
            evidence_path_collection([], errors, label=label)
        except ValueError as error:
            assert "evidence path label must be a non-empty canonical string" in str(
                error
            )
            assert errors == []
        else:
            raise AssertionError(f"accepted malformed label {label!r}")


def test_discover_evidence_files_rejects_malformed_path_collections() -> None:
    errors: list[str] = []

    assert discover_evidence_files("dir", [], errors) == []
    assert discover_evidence_files([], {"file": "evidence.json"}, errors) == []

    assert errors == [
        "evidence directory paths must be a sequence",
        "evidence file paths must be a sequence",
    ]


def test_discover_evidence_files_stops_after_malformed_reserved_outputs(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files(
        [tmp_path],
        [evidence],
        errors,
        reserved_output_paths="summary.json",
    )

    assert files == []
    assert errors == ["reserved output paths must be a sequence"]


def test_discover_evidence_files_stops_after_non_path_reserved_outputs(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files(
        [tmp_path],
        [evidence],
        errors,
        reserved_output_paths=[7],
    )

    assert files == []
    assert errors == ["reserved output `<non-path>` must be a path"]


def test_discover_evidence_files_stops_after_duplicate_reserved_outputs(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    reserved = tmp_path / "summary.json"
    reserved_alias = tmp_path / "nested" / ".." / "summary.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files(
        [tmp_path],
        [evidence],
        errors,
        reserved_output_paths=[reserved, reserved_alias],
    )

    assert files == []
    assert errors == [
        f"duplicate reserved output path `{reserved_alias}` matches `{reserved}`"
    ]


def test_discover_evidence_files_rejects_malformed_error_container() -> None:
    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            discover_evidence_files([], [], errors)
        except ValueError as error:
            assert "evidence path errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


def test_evidence_directory_file_path_reports_directory_requirement(tmp_path: Path) -> None:
    evidence_file = tmp_path / "not-a-directory.json"
    evidence_file.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([evidence_file], [], errors)

    assert files == []
    assert errors == [EVIDENCE_DIRECTORY_MISSING_DIAGNOSTIC]
    assert str(evidence_file) not in errors[0]


def test_evidence_directory_helpers_reject_malformed_error_container(
    tmp_path: Path,
) -> None:
    for helper in (
        inspect_evidence_file,
        inspect_evidence_directory,
        scan_evidence_directory_json,
    ):
        for errors in ("", (), {"error": "old"}, ["old", 7]):
            try:
                helper(tmp_path, errors)
            except ValueError as error:
                assert "evidence path errors must be a list of strings" in str(error)
            else:
                raise AssertionError(
                    f"{helper.__name__} accepted malformed errors {errors!r}"
                )


def test_evidence_file_inspection_failure_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence_file = tmp_path / "evidence.json"
    original_is_file = Path.is_file

    def is_file(path: Path) -> bool:
        if path == evidence_file:
            raise OSError("evidence file stat denied")
        return original_is_file(path)

    monkeypatch.setattr(Path, "is_file", is_file)

    errors: list[str] = []
    files = discover_evidence_files([], [evidence_file], errors)

    assert files == []
    assert errors == [EVIDENCE_FILE_INSPECTION_DIAGNOSTIC]
    assert str(evidence_file) not in errors[0]
    assert "evidence file stat denied" not in errors[0]


def test_evidence_file_symlink_inspection_failure_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence_file = tmp_path / "evidence.json"
    original_is_symlink = Path.is_symlink

    def is_symlink(path: Path) -> bool:
        if path == evidence_file:
            raise RuntimeError("evidence symlink stat denied")
        return original_is_symlink(path)

    monkeypatch.setattr(Path, "is_symlink", is_symlink)

    errors: list[str] = []
    files = discover_evidence_files([], [evidence_file], errors)

    assert files == []
    assert errors == [EVIDENCE_FILE_INSPECTION_DIAGNOSTIC]
    assert str(evidence_file) not in errors[0]
    assert "evidence symlink stat denied" not in errors[0]


def test_evidence_file_parent_inspection_failure_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    parent = tmp_path / "evidence"
    evidence_file = parent / "evidence.json"
    parent.mkdir()
    evidence_file.write_text("{}", encoding="utf-8")
    original_exists = Path.exists

    def exists(path: Path) -> bool:
        if path == parent:
            raise RuntimeError("evidence parent stat denied")
        return original_exists(path)

    monkeypatch.setattr(Path, "exists", exists)

    errors: list[str] = []
    files = discover_evidence_files([], [evidence_file], errors)

    assert files == []
    assert errors == [EVIDENCE_FILE_PARENT_INSPECTION_DIAGNOSTIC]
    assert str(parent) not in errors[0]
    assert "evidence parent stat denied" not in errors[0]


def test_evidence_file_inspection_failure_sanitizes_noncanonical_path(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence_file = tmp_path / "bad\npath.json"
    original_is_file = Path.is_file

    def is_file(path: Path) -> bool:
        if path == evidence_file:
            raise OSError("evidence file stat denied")
        return original_is_file(path)

    monkeypatch.setattr(Path, "is_file", is_file)

    errors: list[str] = []
    files = discover_evidence_files([], [evidence_file], errors)

    assert files == []
    assert errors == [EVIDENCE_FILE_INSPECTION_DIAGNOSTIC]


def test_discovered_evidence_file_inspection_failure_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence_dir = tmp_path / "evidence"
    evidence_file = evidence_dir / "evidence.json"
    evidence_dir.mkdir()
    evidence_file.write_text("{}", encoding="utf-8")
    original_is_file = Path.is_file

    def is_file(path: Path) -> bool:
        if path == evidence_file:
            raise RuntimeError("discovered evidence stat denied")
        return original_is_file(path)

    monkeypatch.setattr(Path, "is_file", is_file)

    errors: list[str] = []
    files = discover_evidence_files([evidence_dir], [], errors)

    assert files == []
    assert errors == [EVIDENCE_FILE_INSPECTION_DIAGNOSTIC]
    assert str(evidence_file) not in errors[0]
    assert "discovered evidence stat denied" not in errors[0]


def test_evidence_directory_inspection_failure_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence_dir = tmp_path / "evidence"
    original_is_dir = Path.is_dir

    def is_dir(path: Path) -> bool:
        if path == evidence_dir:
            raise OSError("evidence dir stat denied")
        return original_is_dir(path)

    monkeypatch.setattr(Path, "is_dir", is_dir)

    errors: list[str] = []
    files = discover_evidence_files([evidence_dir], [], errors)

    assert files == []
    assert errors == [EVIDENCE_DIRECTORY_INSPECTION_DIAGNOSTIC]
    assert str(evidence_dir) not in errors[0]
    assert "evidence dir stat denied" not in errors[0]


def test_evidence_directory_symlink_inspection_failure_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence_dir = tmp_path / "evidence"
    original_is_symlink = Path.is_symlink

    def is_symlink(path: Path) -> bool:
        if path == evidence_dir:
            raise RuntimeError("evidence dir symlink stat denied")
        return original_is_symlink(path)

    monkeypatch.setattr(Path, "is_symlink", is_symlink)

    errors: list[str] = []
    files = discover_evidence_files([evidence_dir], [], errors)

    assert files == []
    assert errors == [EVIDENCE_DIRECTORY_INSPECTION_DIAGNOSTIC]
    assert str(evidence_dir) not in errors[0]
    assert "evidence dir symlink stat denied" not in errors[0]


def test_evidence_directory_inspection_failure_sanitizes_noncanonical_path(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence_dir = tmp_path / "bad\npath"
    original_is_dir = Path.is_dir

    def is_dir(path: Path) -> bool:
        if path == evidence_dir:
            raise OSError("evidence dir stat denied")
        return original_is_dir(path)

    monkeypatch.setattr(Path, "is_dir", is_dir)

    errors: list[str] = []
    files = discover_evidence_files([evidence_dir], [], errors)

    assert files == []
    assert errors == [EVIDENCE_DIRECTORY_INSPECTION_DIAGNOSTIC]


def test_evidence_directory_scan_failure_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence_dir = tmp_path / "evidence"
    evidence_dir.mkdir()
    original_rglob = Path.rglob

    def rglob(path: Path, pattern: str):
        if path == evidence_dir:
            raise RuntimeError("evidence scan denied")
        return original_rglob(path, pattern)

    monkeypatch.setattr(Path, "rglob", rglob)

    errors: list[str] = []
    files = discover_evidence_files([evidence_dir], [], errors)

    assert files == []
    assert errors == [EVIDENCE_DIRECTORY_SCAN_DIAGNOSTIC]
    assert str(evidence_dir) not in errors[0]
    assert "evidence scan denied" not in errors[0]


def test_scan_evidence_directory_json_rejects_file_path(tmp_path: Path) -> None:
    evidence_file = tmp_path / "evidence.json"
    evidence_file.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    assert scan_evidence_directory_json(evidence_file, errors) == []

    assert errors == [EVIDENCE_DIRECTORY_MISSING_DIAGNOSTIC]
    assert str(evidence_file) not in errors[0]


def test_scan_evidence_directory_json_rejects_symlink_directory(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target"
    symlink = tmp_path / "evidence"
    target.mkdir()
    symlink.symlink_to(target, target_is_directory=True)

    errors: list[str] = []
    assert scan_evidence_directory_json(symlink, errors) == []

    assert errors == [EVIDENCE_DIRECTORY_SYMLINK_DIAGNOSTIC]
    assert str(symlink) not in errors[0]


def test_scan_evidence_directory_json_accepts_parent_symlink_directory(
    tmp_path: Path,
) -> None:
    target_root = tmp_path / "target-root"
    symlink_root = tmp_path / "evidence-root"
    target = target_root / "evidence"
    target.mkdir(parents=True)
    (target / "evidence.json").write_text("{}", encoding="utf-8")
    symlink_root.symlink_to(target_root, target_is_directory=True)

    evidence_dir = symlink_root / "evidence"
    errors: list[str] = []
    assert scan_evidence_directory_json(evidence_dir, errors) == [
        evidence_dir / "evidence.json"
    ]

    assert errors == []


def test_reserved_output_conflict_scan_inspection_failure_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence_dir = tmp_path / "evidence"
    reserved = tmp_path / "summary.json"
    original_is_dir = Path.is_dir

    def is_dir(path: Path) -> bool:
        if path == evidence_dir:
            raise RuntimeError("reserved scan stat denied")
        return original_is_dir(path)

    monkeypatch.setattr(Path, "is_dir", is_dir)

    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        [evidence_dir],
        [],
        [reserved],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == [
        EVIDENCE_DIRECTORY_INSPECTION_DIAGNOSTIC
    ]
    assert str(evidence_dir) not in errors[0]
    assert "reserved scan stat denied" not in errors[0]


def test_reserved_output_conflict_non_path_directory_fails_closed(
    tmp_path: Path,
) -> None:
    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        ["reviewed"],
        [],
        [tmp_path / "summary.json"],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == [EVIDENCE_DIRECTORY_PATH_DIAGNOSTIC]
    assert "reviewed" not in errors[0]


def test_reserved_output_conflict_evidence_directory_symlink_fails_closed(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target"
    symlink = tmp_path / "evidence"
    target.mkdir()
    (target / "evidence.json").write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        [symlink],
        [],
        [tmp_path / "summary.json"],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == [EVIDENCE_DIRECTORY_SYMLINK_DIAGNOSTIC]
    assert str(symlink) not in errors[0]


def test_reserved_output_conflict_evidence_directory_parent_symlink_is_accepted(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target"
    parent = tmp_path / "evidence-root"
    target.mkdir()
    (target / "evidence").mkdir()
    (target / "evidence" / "evidence.json").write_text("{}", encoding="utf-8")
    parent.symlink_to(target, target_is_directory=True)
    evidence_dir = parent / "evidence"

    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        [evidence_dir],
        [],
        [tmp_path / "summary.json"],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == []


def test_reserved_output_conflict_explicit_evidence_directory_fails_closed(
    tmp_path: Path,
) -> None:
    evidence_dir = tmp_path / "evidence.json"
    evidence_dir.mkdir()

    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        [],
        [evidence_dir],
        [tmp_path / "summary.json"],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == [EVIDENCE_FILE_MISSING_DIAGNOSTIC]
    assert str(evidence_dir) not in errors[0]


def test_reserved_output_conflict_explicit_evidence_symlink_fails_closed(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target.json"
    symlink = tmp_path / "evidence.json"
    target.write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        [],
        [symlink],
        [tmp_path / "summary.json"],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == [EVIDENCE_FILE_SYMLINK_DIAGNOSTIC]
    assert str(symlink) not in errors[0]


def test_reserved_output_conflict_explicit_evidence_parent_symlink_is_accepted(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target"
    parent = tmp_path / "evidence-root"
    target.mkdir()
    (target / "evidence.json").write_text("{}", encoding="utf-8")
    parent.symlink_to(target, target_is_directory=True)
    evidence = parent / "evidence.json"

    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        [],
        [evidence],
        [tmp_path / "summary.json"],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == []


def test_reserved_output_conflict_discovered_json_directory_fails_closed(
    tmp_path: Path,
) -> None:
    evidence_dir = tmp_path / "evidence"
    json_dir = evidence_dir / "not-a-file.json"
    evidence_dir.mkdir()
    json_dir.mkdir()

    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        [evidence_dir],
        [],
        [tmp_path / "summary.json"],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == [EVIDENCE_FILE_MISSING_DIAGNOSTIC]
    assert str(json_dir) not in errors[0]


def test_reserved_output_conflict_discovered_json_symlink_fails_closed(
    tmp_path: Path,
) -> None:
    evidence_dir = tmp_path / "evidence"
    target = evidence_dir / "target.json"
    symlink = evidence_dir / "link.json"
    evidence_dir.mkdir()
    target.write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        [evidence_dir],
        [],
        [tmp_path / "summary.json"],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == [EVIDENCE_FILE_SYMLINK_DIAGNOSTIC]
    assert str(symlink) not in errors[0]


def test_reserved_output_helpers_reject_malformed_path_collections(tmp_path: Path) -> None:
    errors: list[str] = []

    assert reserved_output_path_identities("summary.json", errors) == {}
    record_reserved_output_evidence_conflicts(
        [],
        {"file": tmp_path / "evidence.json"},
        [tmp_path / "summary.json"],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == [
        "reserved output paths must be a sequence",
        "evidence file paths must be a sequence",
    ]


def test_reserved_output_path_identities_rejects_duplicate_aliases(
    tmp_path: Path,
) -> None:
    reserved = tmp_path / "summary.json"
    reserved_alias = tmp_path / "nested" / ".." / "summary.json"

    errors: list[str] = []
    identities = reserved_output_path_identities(
        [reserved, reserved_alias],
        errors,
        label="--summary-out",
    )

    assert identities == {reserved.resolve(): reserved}
    assert errors == [
        f"duplicate --summary-out path `{reserved_alias}` matches `{reserved}`"
    ]


def test_reserved_output_path_identities_rejects_symlink(tmp_path: Path) -> None:
    target = tmp_path / "target.json"
    reserved = tmp_path / "summary.json"
    target.write_text("{}", encoding="utf-8")
    reserved.symlink_to(target)

    errors: list[str] = []
    identities = reserved_output_path_identities(
        [reserved],
        errors,
        label="--summary-out",
    )

    assert identities == {}
    assert errors == [f"--summary-out `{reserved}` must not be a symlink"]


def test_reserved_output_path_identities_accepts_parent_symlink_directory(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target"
    parent = tmp_path / "summary-root"
    target.mkdir()
    parent.symlink_to(target, target_is_directory=True)
    reserved = parent / "summary.json"

    errors: list[str] = []
    identities = reserved_output_path_identities(
        [reserved],
        errors,
        label="--summary-out",
    )

    assert identities == {reserved.resolve(): reserved}
    assert errors == []


def test_reserved_output_path_identities_symlink_inspection_failure(
    tmp_path: Path,
    monkeypatch,
) -> None:
    reserved = tmp_path / "summary.json"
    original_is_symlink = Path.is_symlink

    def is_symlink(path: Path) -> bool:
        if path == reserved:
            raise RuntimeError("reserved output symlink stat denied")
        return original_is_symlink(path)

    monkeypatch.setattr(Path, "is_symlink", is_symlink)

    errors: list[str] = []
    identities = reserved_output_path_identities(
        [reserved],
        errors,
        label="--summary-out",
    )

    assert identities == {}
    assert errors == [
        f"--summary-out `{reserved}` cannot be inspected: "
        "reserved output symlink stat denied"
    ]


def test_discover_evidence_files_stops_after_reserved_output_symlink(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    target = tmp_path / "target.json"
    reserved = tmp_path / "summary.json"
    evidence.write_text("{}", encoding="utf-8")
    target.write_text("{}", encoding="utf-8")
    reserved.symlink_to(target)

    errors: list[str] = []
    files = discover_evidence_files(
        [],
        [evidence],
        errors,
        reserved_output_paths=[reserved],
    )

    assert files == []
    assert errors == [f"reserved output `{reserved}` must not be a symlink"]


def test_reserved_output_conflict_scan_stops_after_duplicate_reserved_outputs(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "summary.json"
    reserved = tmp_path / "summary.json"
    reserved_alias = tmp_path / "nested" / ".." / "summary.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        [tmp_path],
        [evidence],
        [reserved, reserved_alias],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == [
        f"duplicate --summary-out path `{reserved_alias}` matches `{reserved}`"
    ]


def test_reserved_output_conflict_scan_accepts_reserved_output_parent_symlink(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target"
    parent = tmp_path / "summary-root"
    target.mkdir()
    evidence = target / "summary.json"
    evidence.write_text("{}", encoding="utf-8")
    parent.symlink_to(target, target_is_directory=True)
    reserved = parent / "summary.json"

    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        [],
        [evidence],
        [reserved],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == [EVIDENCE_FILE_RESERVED_CONFLICT_DIAGNOSTIC]


def test_reserved_output_helpers_reject_malformed_error_container(
    tmp_path: Path,
) -> None:
    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            reserved_output_path_identities([], errors)
        except ValueError as error:
            assert "evidence path errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")

    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            record_reserved_output_evidence_conflicts(
                [],
                [],
                [tmp_path / "summary.json"],
                errors,
            )
        except ValueError as error:
            assert "evidence path errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


def test_reserved_output_helpers_reject_malformed_labels(tmp_path: Path) -> None:
    for label in ("", " --summary-out", "--summary-out ", "summary\nout", 7):
        errors: list[str] = []
        try:
            reserved_output_path_identities([], errors, label=label)
        except ValueError as error:
            assert "evidence path label must be a non-empty canonical string" in str(
                error
            )
            assert errors == []
        else:
            raise AssertionError(f"accepted malformed label {label!r}")

    for label in ("", " --summary-out", "--summary-out ", "summary\nout", 7):
        errors = []
        try:
            record_reserved_output_evidence_conflicts(
                [],
                [],
                [tmp_path / "summary.json"],
                errors,
                reserved_label=label,
            )
        except ValueError as error:
            assert "evidence path label must be a non-empty canonical string" in str(
                error
            )
            assert errors == []
        else:
            raise AssertionError(f"accepted malformed reserved label {label!r}")


def test_reserved_output_conflict_scan_failure_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    evidence_dir = tmp_path / "evidence"
    evidence_dir.mkdir()
    reserved = tmp_path / "summary.json"
    original_rglob = Path.rglob

    def rglob(path: Path, pattern: str):
        if path == evidence_dir:
            raise OSError("reserved scan denied")
        return original_rglob(path, pattern)

    monkeypatch.setattr(Path, "rglob", rglob)

    errors: list[str] = []
    record_reserved_output_evidence_conflicts(
        [evidence_dir],
        [],
        [reserved],
        errors,
        reserved_label="--summary-out",
    )

    assert errors == [EVIDENCE_DIRECTORY_SCAN_DIAGNOSTIC]
    assert str(evidence_dir) not in errors[0]
    assert "reserved scan denied" not in errors[0]


def test_explicit_identity_helpers_record_resolver_failures(tmp_path: Path) -> None:
    loop = tmp_path / "loop.json"
    loop.symlink_to(loop)

    errors: list[str] = []
    assert resolve_evidence_path(loop, errors) is None
    assert evidence_path_identities([loop], errors) == set()
    assert is_explicit_evidence_path(loop, set(), errors) is False
    assert len(errors) == 1
    assert "cannot be resolved" in errors[0]


def test_evidence_path_identities_rejects_symlink_identity(tmp_path: Path) -> None:
    target = tmp_path / "target.json"
    symlink = tmp_path / "evidence.json"
    target.write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    identities = evidence_path_identities([symlink], errors)

    assert identities == set()
    assert errors == [EVIDENCE_FILE_SYMLINK_DIAGNOSTIC]
    assert str(symlink) not in errors[0]


def test_is_explicit_evidence_path_rejects_symlink_candidate(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target.json"
    symlink = tmp_path / "evidence.json"
    target.write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    assert is_explicit_evidence_path(symlink, {target.resolve()}, errors) is False

    assert errors == [EVIDENCE_FILE_SYMLINK_DIAGNOSTIC]
    assert str(symlink) not in errors[0]


def test_is_explicit_evidence_path_accepts_parent_symlink_candidate(
    tmp_path: Path,
) -> None:
    target_root = tmp_path / "target-root"
    symlink_root = tmp_path / "evidence-root"
    target = target_root / "evidence.json"
    target_root.mkdir()
    target.write_text("{}", encoding="utf-8")
    symlink_root.symlink_to(target_root, target_is_directory=True)

    candidate = symlink_root / "evidence.json"
    errors: list[str] = []
    assert is_explicit_evidence_path(candidate, {target.resolve()}, errors) is True

    assert errors == []


def test_is_explicit_evidence_path_skips_empty_identity_set_without_resolving(
    tmp_path: Path,
) -> None:
    loop = tmp_path / "loop.json"
    loop.symlink_to(loop)

    errors: list[str] = []
    assert is_explicit_evidence_path(loop, set(), errors) is False

    assert errors == []


def test_evidence_path_identities_skip_after_discovery_errors(
    tmp_path: Path,
) -> None:
    evidence_dir = tmp_path / "evidence"
    evidence_dir.mkdir()
    target = evidence_dir / "target.json"
    symlink = tmp_path / "explicit.json"
    target.write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    files = discover_evidence_files([evidence_dir], [symlink], errors)
    identities = evidence_path_identities([symlink], errors)

    assert files == [target]
    assert identities == set()
    assert errors == [EVIDENCE_FILE_SYMLINK_DIAGNOSTIC]
    assert str(symlink) not in errors[0]


def test_explicit_identity_helper_rejects_malformed_identity_container(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    for identities in (None, [], {"path": evidence}, {str(evidence)}):
        errors: list[str] = []

        assert is_explicit_evidence_path(evidence, identities, errors) is False

        assert errors == ["explicit evidence identities must be a set of paths"]


def test_evidence_path_identities_rejects_malformed_path_collections() -> None:
    errors: list[str] = []

    assert evidence_path_identities("evidence.json", errors) == set()

    assert errors == ["evidence paths must be a sequence"]


def test_evidence_path_resolution_uses_shared_identity_helper() -> None:
    import sorafs_evidence_paths

    assert (
        sorafs_evidence_paths.resolve_evidence_path.__globals__[
            "resolve_path_identity"
        ].__module__
        == "sorafs_path_identity"
    )
