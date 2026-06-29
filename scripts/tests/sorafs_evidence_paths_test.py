"""Tests for shared SoraFS evidence path discovery."""

from __future__ import annotations

import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_evidence_paths import (  # noqa: E402
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
    assert "duplicate explicit evidence file" in errors[0]


def test_explicit_directory_overlap_fails(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([tmp_path], [evidence], errors)

    assert files == []
    assert len(errors) == 1
    assert "both --evidence and --evidence-dir" in errors[0]


def test_overlapping_directories_fail(tmp_path: Path) -> None:
    nested = tmp_path / "nested"
    nested.mkdir()
    evidence = nested / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([tmp_path, nested], [], errors)

    assert files == []
    assert len(errors) == 1
    assert "duplicate evidence file" in errors[0]


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
    assert "both --evidence and --evidence-dir" in errors[0]


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
    assert "conflicts with reserved output" in errors[0]


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
    assert "conflicts with reserved output" in errors[0]


def test_missing_explicit_evidence_file_fails_closed(tmp_path: Path) -> None:
    missing = tmp_path / "missing.json"

    errors: list[str] = []
    files = discover_evidence_files([], [missing], errors)

    assert files == []
    assert errors == [f"evidence file `{missing}` must exist and be a file"]


def test_explicit_evidence_directory_fails_closed(tmp_path: Path) -> None:
    evidence_dir = tmp_path / "evidence.json"
    evidence_dir.mkdir()

    errors: list[str] = []
    files = discover_evidence_files([], [evidence_dir], errors)

    assert files == []
    assert errors == [f"evidence file `{evidence_dir}` must exist and be a file"]


def test_explicit_evidence_symlink_fails_closed(tmp_path: Path) -> None:
    target = tmp_path / "target.json"
    symlink = tmp_path / "evidence.json"
    target.write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    files = discover_evidence_files([], [symlink], errors)

    assert files == []
    assert errors == [f"evidence file `{symlink}` must not be a symlink"]


def test_explicit_evidence_parent_symlink_fails_closed(tmp_path: Path) -> None:
    target = tmp_path / "target"
    parent = tmp_path / "evidence-root"
    target.mkdir()
    (target / "evidence.json").write_text("{}", encoding="utf-8")
    parent.symlink_to(target, target_is_directory=True)
    evidence = parent / "evidence.json"

    errors: list[str] = []
    files = discover_evidence_files([], [evidence], errors)

    assert files == []
    assert errors == [f"evidence file parent `{parent}` must not be a symlink"]


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
    assert errors == [f"evidence file `{json_dir}` must exist and be a file"]


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
    assert errors == [f"evidence file `{symlink}` must not be a symlink"]


def test_noncanonical_missing_explicit_evidence_file_label_is_sanitized(
    tmp_path: Path,
) -> None:
    errors: list[str] = []
    files = discover_evidence_files([], [tmp_path / "bad\npath.json"], errors)

    assert files == []
    assert errors == [
        "evidence file `<non-canonical-path>` must exist and be a file"
    ]


def test_missing_evidence_directory_fails_closed(tmp_path: Path) -> None:
    missing = tmp_path / "missing"

    errors: list[str] = []
    files = discover_evidence_files([missing], [], errors)

    assert files == []
    assert errors == [f"evidence directory `{missing}` must exist and be a directory"]


def test_evidence_directory_symlink_fails_closed(tmp_path: Path) -> None:
    target = tmp_path / "target"
    symlink = tmp_path / "evidence"
    target.mkdir()
    (target / "evidence.json").write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    files = discover_evidence_files([symlink], [], errors)

    assert files == []
    assert errors == [f"evidence directory `{symlink}` must not be a symlink"]


def test_evidence_directory_parent_symlink_fails_closed(tmp_path: Path) -> None:
    target = tmp_path / "target"
    parent = tmp_path / "evidence-root"
    target.mkdir()
    (target / "evidence").mkdir()
    parent.symlink_to(target, target_is_directory=True)
    evidence_dir = parent / "evidence"

    errors: list[str] = []
    files = discover_evidence_files([evidence_dir], [], errors)

    assert files == []
    assert errors == [f"evidence directory parent `{parent}` must not be a symlink"]


def test_non_path_evidence_directory_fails_closed_without_traceback() -> None:
    errors: list[str] = []
    files = discover_evidence_files(["reviewed"], [], errors)

    assert files == []
    assert errors == ["evidence directory `reviewed` must be a path"]


def test_noncanonical_evidence_file_labels_are_sanitized() -> None:
    for path_value, expected_label in (
        (" reviewed", "<non-canonical-path>"),
        ("reviewed\npath", "<non-canonical-path>"),
        (b"reviewed", "<non-path>"),
    ):
        errors: list[str] = []
        assert inspect_evidence_file(path_value, errors) is None
        assert errors == [f"evidence file `{expected_label}` must be a path"]


def test_noncanonical_evidence_directory_labels_are_sanitized() -> None:
    for directory, expected_label in (
        (" reviewed", "<non-canonical-path>"),
        ("reviewed\npath", "<non-canonical-path>"),
        (b"reviewed", "<non-path>"),
    ):
        errors: list[str] = []
        assert inspect_evidence_directory(directory, errors) is None
        assert errors == [f"evidence directory `{expected_label}` must be a path"]


def test_noncanonical_missing_evidence_directory_label_is_sanitized(
    tmp_path: Path,
) -> None:
    errors: list[str] = []
    files = discover_evidence_files([tmp_path / "bad\npath"], [], errors)

    assert files == []
    assert errors == [
        "evidence directory `<non-canonical-path>` must exist and be a directory"
    ]


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
    assert errors == [
        f"evidence directory `{evidence_file}` must exist and be a directory"
    ]


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
    assert errors == [
        f"evidence file `{evidence_file}` cannot be inspected: "
        "evidence file stat denied"
    ]


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
    assert errors == [
        f"evidence file `{evidence_file}` cannot be inspected: "
        "evidence symlink stat denied"
    ]


def test_evidence_file_parent_inspection_failure_fails_closed(
    tmp_path: Path,
    monkeypatch,
) -> None:
    parent = tmp_path / "evidence"
    evidence_file = parent / "evidence.json"
    parent.mkdir()
    evidence_file.write_text("{}", encoding="utf-8")
    original_is_symlink = Path.is_symlink

    def is_symlink(path: Path) -> bool:
        if path == parent:
            raise RuntimeError("evidence parent stat denied")
        return original_is_symlink(path)

    monkeypatch.setattr(Path, "is_symlink", is_symlink)

    errors: list[str] = []
    files = discover_evidence_files([], [evidence_file], errors)

    assert files == []
    assert errors == [
        f"evidence file parent `{parent}` cannot be inspected: "
        "evidence parent stat denied"
    ]


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
    assert errors == [
        "evidence file `<non-canonical-path>` cannot be inspected: "
        "<non-canonical-error>"
    ]


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
    assert errors == [
        f"evidence file `{evidence_file}` cannot be inspected: "
        "discovered evidence stat denied"
    ]


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
    assert errors == [
        f"evidence directory `{evidence_dir}` cannot be inspected: "
        "evidence dir stat denied"
    ]


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
    assert errors == [
        f"evidence directory `{evidence_dir}` cannot be inspected: "
        "evidence dir symlink stat denied"
    ]


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
    assert errors == [
        "evidence directory `<non-canonical-path>` cannot be inspected: "
        "<non-canonical-error>"
    ]


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
    assert errors == [
        f"failed to scan evidence directory `{evidence_dir}`: evidence scan denied"
    ]


def test_scan_evidence_directory_json_rejects_file_path(tmp_path: Path) -> None:
    evidence_file = tmp_path / "evidence.json"
    evidence_file.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    assert scan_evidence_directory_json(evidence_file, errors) == []

    assert errors == [
        f"evidence directory `{evidence_file}` must exist and be a directory"
    ]


def test_scan_evidence_directory_json_rejects_symlink_directory(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target"
    symlink = tmp_path / "evidence"
    target.mkdir()
    symlink.symlink_to(target, target_is_directory=True)

    errors: list[str] = []
    assert scan_evidence_directory_json(symlink, errors) == []

    assert errors == [f"evidence directory `{symlink}` must not be a symlink"]


def test_scan_evidence_directory_json_rejects_parent_symlink_directory(
    tmp_path: Path,
) -> None:
    target_root = tmp_path / "target-root"
    symlink_root = tmp_path / "evidence-root"
    target = target_root / "evidence"
    target.mkdir(parents=True)
    symlink_root.symlink_to(target_root, target_is_directory=True)

    evidence_dir = symlink_root / "evidence"
    errors: list[str] = []
    assert scan_evidence_directory_json(evidence_dir, errors) == []

    assert errors == [
        f"evidence directory parent `{symlink_root}` must not be a symlink"
    ]


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
        f"evidence directory `{evidence_dir}` cannot be inspected: "
        "reserved scan stat denied"
    ]


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

    assert errors == ["evidence directory `reviewed` must be a path"]


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

    assert errors == [f"evidence directory `{symlink}` must not be a symlink"]


def test_reserved_output_conflict_evidence_directory_parent_symlink_fails_closed(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target"
    parent = tmp_path / "evidence-root"
    target.mkdir()
    (target / "evidence").mkdir()
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

    assert errors == [f"evidence directory parent `{parent}` must not be a symlink"]


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

    assert errors == [f"evidence file `{evidence_dir}` must exist and be a file"]


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

    assert errors == [f"evidence file `{symlink}` must not be a symlink"]


def test_reserved_output_conflict_explicit_evidence_parent_symlink_fails_closed(
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

    assert errors == [f"evidence file parent `{parent}` must not be a symlink"]


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

    assert errors == [f"evidence file `{json_dir}` must exist and be a file"]


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

    assert errors == [f"evidence file `{symlink}` must not be a symlink"]


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


def test_reserved_output_path_identities_rejects_parent_symlink(
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

    assert identities == {}
    assert errors == [f"--summary-out parent `{parent}` must not be a symlink"]


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


def test_reserved_output_conflict_scan_stops_after_reserved_output_parent_symlink(
    tmp_path: Path,
) -> None:
    evidence = tmp_path / "evidence.json"
    target = tmp_path / "target"
    parent = tmp_path / "summary-root"
    target.mkdir()
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

    assert errors == [f"--summary-out parent `{parent}` must not be a symlink"]


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

    assert errors == [
        f"failed to scan evidence directory `{evidence_dir}`: reserved scan denied"
    ]


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
    assert errors == [f"evidence file `{symlink}` must not be a symlink"]


def test_is_explicit_evidence_path_rejects_symlink_candidate(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target.json"
    symlink = tmp_path / "evidence.json"
    target.write_text("{}", encoding="utf-8")
    symlink.symlink_to(target)

    errors: list[str] = []
    assert is_explicit_evidence_path(symlink, {target.resolve()}, errors) is False

    assert errors == [f"evidence file `{symlink}` must not be a symlink"]


def test_is_explicit_evidence_path_rejects_parent_symlink_candidate(
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
    assert is_explicit_evidence_path(candidate, {target.resolve()}, errors) is False

    assert errors == [f"evidence file parent `{symlink_root}` must not be a symlink"]


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
    assert errors == [f"evidence file `{symlink}` must not be a symlink"]


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
