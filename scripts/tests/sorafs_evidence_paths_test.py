"""Tests for shared SoraFS evidence path discovery."""

from __future__ import annotations

import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_evidence_paths import (  # noqa: E402
    discover_evidence_files,
    evidence_path_identities,
    is_explicit_evidence_path,
    record_reserved_output_evidence_conflicts,
    resolve_evidence_path,
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


def test_evidence_directory_file_path_reports_directory_requirement(tmp_path: Path) -> None:
    evidence_file = tmp_path / "not-a-directory.json"
    evidence_file.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    files = discover_evidence_files([evidence_file], [], errors)

    assert files == []
    assert errors == [
        f"evidence directory `{evidence_file}` must exist and be a directory"
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
