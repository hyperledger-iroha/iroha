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

    assert files == [evidence]
    assert len(errors) == 1
    assert "duplicate explicit evidence file" in errors[0]


def test_explicit_directory_overlap_fails(tmp_path: Path) -> None:
    evidence = tmp_path / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([tmp_path], [evidence], errors)

    assert files == [evidence]
    assert len(errors) == 1
    assert "both --evidence and --evidence-dir" in errors[0]


def test_overlapping_directories_fail(tmp_path: Path) -> None:
    nested = tmp_path / "nested"
    nested.mkdir()
    evidence = nested / "evidence.json"
    evidence.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([tmp_path, nested], [], errors)

    assert files == [evidence]
    assert len(errors) == 1
    assert "duplicate evidence file" in errors[0]


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


def test_missing_evidence_directory_fails_closed(tmp_path: Path) -> None:
    missing = tmp_path / "missing"

    errors: list[str] = []
    files = discover_evidence_files([missing], [], errors)

    assert files == []
    assert errors == [f"evidence directory `{missing}` must exist and be a directory"]


def test_non_path_evidence_directory_fails_closed_without_traceback() -> None:
    errors: list[str] = []
    files = discover_evidence_files(["reviewed"], [], errors)

    assert files == []
    assert errors == ["evidence directory `reviewed` must be a path"]


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
    for helper in (inspect_evidence_directory, scan_evidence_directory_json):
        for errors in ("", (), {"error": "old"}, ["old", 7]):
            try:
                helper(tmp_path, errors)
            except ValueError as error:
                assert "evidence path errors must be a list of strings" in str(error)
            else:
                raise AssertionError(
                    f"{helper.__name__} accepted malformed errors {errors!r}"
                )


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
    assert all("cannot be resolved" in error for error in errors)


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
