"""Shared evidence file discovery for SoraFS rollout gates."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from sorafs_path_identity import (
    error_diagnostic_label,
    path_diagnostic_label,
    resolve_path_identity,
)


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("evidence path errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("evidence path errors must be a list of strings")
        if (
            not error.strip()
            or error != error.strip()
            or any(ord(character) < 32 or ord(character) == 127 for character in error)
        ):
            raise ValueError(
                "evidence path errors must contain non-empty canonical strings"
            )
    return errors


def _require_label(label: Any) -> str:
    if (
        not isinstance(label, str)
        or not label.strip()
        or label != label.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in label)
    ):
        raise ValueError("evidence path label must be a non-empty canonical string")
    return label


def _path_label(path: Any) -> str:
    return path_diagnostic_label(path)


def _error_label(error: BaseException, *, path_label: str | None = None) -> str:
    return error_diagnostic_label(error, path_label=path_label)


def _evidence_path_identity_set(
    identities: Any,
    errors: list[str],
    *,
    label: str,
) -> set[Path] | None:
    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if not isinstance(identities, set):
        error_list.append(f"{path_label} identities must be a set of paths")
        return None
    if not all(isinstance(identity, Path) for identity in identities):
        error_list.append(f"{path_label} identities must be a set of paths")
        return None
    return identities


def resolve_evidence_path(
    path: Path,
    errors: list[str],
    *,
    label: str = "evidence path",
) -> Path | None:
    """Return the canonical identity for an evidence path, recording failures."""

    return resolve_path_identity(path, errors, label=label)


def evidence_path_collection(
    paths: object,
    errors: list[str],
    *,
    label: str,
) -> Sequence[object] | None:
    """Return a path collection or reject scalar/object containers."""

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if isinstance(paths, (str, bytes, bytearray, Mapping)) or not isinstance(
        paths, Sequence
    ):
        error_list.append(f"{path_label} paths must be a sequence")
        return None
    return paths


def evidence_path_identities(paths: object, errors: list[str]) -> set[Path]:
    """Return canonical identities for paths, skipping paths already recorded as errors."""

    identities: set[Path] = set()
    path_items = evidence_path_collection(paths, errors, label="evidence")
    if path_items is None:
        return identities
    for path in path_items:
        resolved = resolve_evidence_path(path, errors)
        if resolved is not None:
            identities.add(resolved)
    return identities


def is_explicit_evidence_path(
    path: Path,
    explicit_identities: set[Path],
    errors: list[str],
) -> bool:
    """Return whether a discovered path came from an explicit --evidence input."""

    identities = _evidence_path_identity_set(
        explicit_identities,
        errors,
        label="explicit evidence",
    )
    if identities is None:
        return False
    resolved = resolve_evidence_path(path, errors)
    return resolved is not None and resolved in identities


def reserved_output_path_identities(
    paths: object,
    errors: list[str],
    *,
    label: str = "reserved output",
) -> dict[Path, Path]:
    """Return canonical identities for output paths that evidence must not reuse."""

    identities: dict[Path, Path] = {}
    path_items = evidence_path_collection(paths, errors, label=label)
    if path_items is None:
        return identities
    for path in path_items:
        resolved = resolve_evidence_path(path, errors, label=label)
        if resolved is not None:
            identities[resolved] = path
    return identities


def inspect_evidence_directory(
    directory: Path,
    errors: list[str],
) -> bool | None:
    """Return whether `directory` is a directory, recording inspection failures."""

    error_list = _require_error_list(errors)
    if not isinstance(directory, Path):
        error_list.append(f"evidence directory `{_path_label(directory)}` must be a path")
        return None
    try:
        return directory.is_dir()
    except (OSError, RuntimeError) as error:
        directory_label = _path_label(directory)
        error_list.append(
            f"evidence directory `{directory_label}` cannot be inspected: "
            f"{_error_label(error, path_label=directory_label)}"
        )
        return None


def scan_evidence_directory_json(
    directory: Path,
    errors: list[str],
) -> list[Path]:
    """Return JSON evidence files under `directory`, recording scan failures."""

    error_list = _require_error_list(errors)
    if not isinstance(directory, Path):
        error_list.append(f"evidence directory `{_path_label(directory)}` must be a path")
        return []
    try:
        return sorted(directory.rglob("*.json"))
    except (OSError, RuntimeError) as error:
        directory_label = _path_label(directory)
        error_list.append(
            f"failed to scan evidence directory `{directory_label}`: "
            f"{_error_label(error, path_label=directory_label)}"
        )
        return []


def record_reserved_output_evidence_conflicts(
    evidence_dirs: object,
    evidence_files: object,
    reserved_output_paths: object,
    errors: list[str],
    *,
    reserved_label: str = "reserved output",
) -> None:
    """Record evidence files that resolve to a reserved output path."""

    error_list = _require_error_list(errors)
    output_label = _require_label(reserved_label)
    evidence_dir_items = evidence_path_collection(
        evidence_dirs,
        error_list,
        label="evidence directory",
    )
    evidence_file_items = evidence_path_collection(
        evidence_files,
        error_list,
        label="evidence file",
    )
    if evidence_dir_items is None or evidence_file_items is None:
        return
    reserved_outputs = reserved_output_path_identities(
        reserved_output_paths,
        error_list,
        label=output_label,
    )
    if not reserved_outputs:
        return

    def check(path: Path) -> None:
        resolved = resolve_evidence_path(path, error_list)
        if resolved is None:
            return
        reserved_output = reserved_outputs.get(resolved)
        if reserved_output is not None:
            error_list.append(
                f"evidence file `{_path_label(path)}` conflicts with {output_label} "
                f"`{_path_label(reserved_output)}`"
            )

    for path in evidence_file_items:
        check(path)

    for directory in evidence_dir_items:
        is_dir = inspect_evidence_directory(directory, error_list)
        if not is_dir:
            continue
        discovered = scan_evidence_directory_json(directory, error_list)
        for path in discovered:
            check(path)


def discover_evidence_files(
    evidence_dirs: object,
    evidence_files: object,
    errors: list[str],
    *,
    reserved_output_paths: object = (),
) -> list[Path]:
    """Discover evidence JSON files while rejecting ambiguous identities."""

    error_list = _require_error_list(errors)
    files: list[Path] = []
    seen: dict[Path, tuple[Path, bool]] = {}
    evidence_dir_items = evidence_path_collection(
        evidence_dirs,
        error_list,
        label="evidence directory",
    )
    evidence_file_items = evidence_path_collection(
        evidence_files,
        error_list,
        label="evidence file",
    )
    if evidence_dir_items is None or evidence_file_items is None:
        return files
    reserved_error_count = len(error_list)
    reserved_outputs = reserved_output_path_identities(
        reserved_output_paths, error_list
    )
    if len(error_list) != reserved_error_count:
        return files

    def add(path: Path, *, explicit: bool) -> None:
        resolved = resolve_evidence_path(path, error_list)
        if resolved is None:
            return
        reserved_output = reserved_outputs.get(resolved)
        if reserved_output is not None:
            error_list.append(
                f"evidence file `{_path_label(path)}` conflicts with reserved output "
                f"`{_path_label(reserved_output)}`"
            )
            return
        previous = seen.get(resolved)
        if previous is not None:
            previous_path, previous_explicit = previous
            if explicit and previous_explicit:
                error_list.append(
                    f"duplicate explicit evidence file `{_path_label(path)}` "
                    f"matches `{_path_label(previous_path)}`"
                )
            elif explicit or previous_explicit:
                duplicate_source = "both --evidence and --evidence-dir"
                error_list.append(
                    f"evidence file `{_path_label(path)}` is provided by "
                    f"{duplicate_source}"
                )
            else:
                error_list.append(
                    f"duplicate evidence file `{_path_label(path)}` also discovered "
                    f"from `{_path_label(previous_path)}`"
                )
            return
        seen[resolved] = (path, explicit)
        files.append(path)

    for path in evidence_file_items:
        add(path, explicit=True)

    for directory in evidence_dir_items:
        is_dir = inspect_evidence_directory(directory, error_list)
        if is_dir is None:
            continue
        if not is_dir:
            error_list.append(
                f"evidence directory `{_path_label(directory)}` must exist and be a directory"
            )
            continue
        discovered = scan_evidence_directory_json(directory, error_list)
        for path in discovered:
            add(path, explicit=False)

    return files
