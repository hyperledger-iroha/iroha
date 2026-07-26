"""Shared evidence file discovery for SoraFS rollout gates."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from sorafs_path_identity import (
    diagnostic_text_is_canonical,
    error_diagnostic_label,
    path_diagnostic_label,
    resolve_path_identity,
)

EVIDENCE_PATH_RESOLUTION_DIAGNOSTIC = "evidence path cannot be resolved"
EVIDENCE_FILE_PATH_DIAGNOSTIC = "evidence file must be a path"
EVIDENCE_FILE_INSPECTION_DIAGNOSTIC = "evidence file cannot be inspected"
EVIDENCE_FILE_SYMLINK_DIAGNOSTIC = "evidence file must not be a symlink"
EVIDENCE_FILE_PARENT_INSPECTION_DIAGNOSTIC = (
    "evidence file parent cannot be inspected"
)
EVIDENCE_FILE_PARENT_SYMLINK_DIAGNOSTIC = (
    "evidence file parent must not be a symlink"
)
EVIDENCE_FILE_PARENT_DIRECTORY_DIAGNOSTIC = (
    "evidence file parent must be a directory when it exists"
)
EVIDENCE_FILE_MISSING_DIAGNOSTIC = "evidence file must exist and be a file"
EVIDENCE_FILE_RESERVED_CONFLICT_DIAGNOSTIC = (
    "evidence file conflicts with reserved output"
)
EVIDENCE_FILE_DUPLICATE_EXPLICIT_DIAGNOSTIC = "duplicate explicit evidence file"
EVIDENCE_FILE_SOURCE_OVERLAP_DIAGNOSTIC = (
    "evidence file provided by multiple evidence sources"
)
EVIDENCE_FILE_DUPLICATE_DISCOVERED_DIAGNOSTIC = "duplicate evidence file"
EVIDENCE_DIRECTORY_PATH_DIAGNOSTIC = "evidence directory must be a path"
EVIDENCE_DIRECTORY_INSPECTION_DIAGNOSTIC = "evidence directory cannot be inspected"
EVIDENCE_DIRECTORY_SYMLINK_DIAGNOSTIC = "evidence directory must not be a symlink"
EVIDENCE_DIRECTORY_PARENT_INSPECTION_DIAGNOSTIC = (
    "evidence directory parent cannot be inspected"
)
EVIDENCE_DIRECTORY_PARENT_SYMLINK_DIAGNOSTIC = (
    "evidence directory parent must not be a symlink"
)
EVIDENCE_DIRECTORY_PARENT_DIRECTORY_DIAGNOSTIC = (
    "evidence directory parent must be a directory when it exists"
)
EVIDENCE_DIRECTORY_MISSING_DIAGNOSTIC = (
    "evidence directory must exist and be a directory"
)
EVIDENCE_DIRECTORY_SCAN_DIAGNOSTIC = "evidence directory cannot be scanned"


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("evidence path errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("evidence path errors must be a list of strings")
        if not diagnostic_text_is_canonical(error):
            raise ValueError(
                "evidence path errors must contain non-empty canonical strings"
            )
    return errors


def _require_label(label: Any) -> str:
    if not diagnostic_text_is_canonical(label):
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

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if path_label == "evidence path":
        resolution_errors: list[str] = []
        resolved = resolve_path_identity(path, resolution_errors, label=path_label)
        if resolution_errors:
            error_list.append(EVIDENCE_PATH_RESOLUTION_DIAGNOSTIC)
        return resolved
    return resolve_path_identity(path, error_list, label=path_label)


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
    error_list = _require_error_list(errors)
    path_items = evidence_path_collection(paths, error_list, label="evidence")
    if path_items is None:
        return identities
    if error_list:
        return identities
    for path in path_items:
        is_file = inspect_evidence_file(path, error_list)
        if is_file is None:
            continue
        if not is_file:
            error_list.append(EVIDENCE_FILE_MISSING_DIAGNOSTIC)
            continue
        resolved = resolve_evidence_path(path, error_list)
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
    if identities is None or not identities:
        return False
    is_file = inspect_evidence_file(path, errors)
    if is_file is None:
        return False
    if not is_file:
        errors.append(EVIDENCE_FILE_MISSING_DIAGNOSTIC)
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
    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    path_items = evidence_path_collection(paths, error_list, label=path_label)
    if path_items is None:
        return identities
    for path in path_items:
        if not isinstance(path, Path):
            error_list.append(f"{path_label} `{_path_label(path)}` must be a path")
            continue
        try:
            if path.is_symlink():
                error_list.append(
                    f"{path_label} `{_path_label(path)}` must not be a symlink"
                )
                continue
        except (OSError, RuntimeError) as error:
            output_path_label = _path_label(path)
            error_list.append(
                f"{path_label} `{output_path_label}` cannot be inspected: "
                f"{_error_label(error, path_label=output_path_label)}"
            )
            continue
        if not validate_evidence_parent_chain(path, error_list, label=path_label):
            continue
        resolved = resolve_evidence_path(path, error_list, label=path_label)
        if resolved is not None:
            previous_path = identities.get(resolved)
            if previous_path is not None:
                error_list.append(
                    f"duplicate {path_label} path `{_path_label(path)}` "
                    f"matches `{_path_label(previous_path)}`"
                )
                continue
            identities[resolved] = path
    return identities


def inspect_evidence_directory(
    directory: Path,
    errors: list[str],
) -> bool | None:
    """Return whether `directory` is a directory, recording inspection failures."""

    error_list = _require_error_list(errors)
    if not isinstance(directory, Path):
        error_list.append(EVIDENCE_DIRECTORY_PATH_DIAGNOSTIC)
        return None
    try:
        if directory.is_symlink():
            error_list.append(EVIDENCE_DIRECTORY_SYMLINK_DIAGNOSTIC)
            return None
        if not validate_evidence_parent_chain(
            directory,
            error_list,
            label="evidence directory",
        ):
            return None
        return directory.is_dir()
    except (OSError, RuntimeError) as error:
        del error
        error_list.append(EVIDENCE_DIRECTORY_INSPECTION_DIAGNOSTIC)
        return None


def inspect_evidence_file(
    path: Path,
    errors: list[str],
) -> bool | None:
    """Return whether `path` is a regular file, recording inspection failures."""

    error_list = _require_error_list(errors)
    if not isinstance(path, Path):
        error_list.append(EVIDENCE_FILE_PATH_DIAGNOSTIC)
        return None
    try:
        if path.is_symlink():
            error_list.append(EVIDENCE_FILE_SYMLINK_DIAGNOSTIC)
            return None
        if not validate_evidence_parent_chain(
            path,
            error_list,
            label="evidence file",
        ):
            return None
        return path.is_file()
    except (OSError, RuntimeError) as error:
        del error
        error_list.append(EVIDENCE_FILE_INSPECTION_DIAGNOSTIC)
        return None


def validate_evidence_parent_chain(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool:
    """Validate parent directories before trusting an evidence path's identity.

    Final-path symlink rejection remains the caller's responsibility.
    """

    error_list = _require_error_list(errors)
    evidence_label = _require_label(label)
    if not isinstance(path, Path):
        if evidence_label == "evidence file":
            error_list.append(EVIDENCE_FILE_PATH_DIAGNOSTIC)
        elif evidence_label == "evidence directory":
            error_list.append(EVIDENCE_DIRECTORY_PATH_DIAGNOSTIC)
        else:
            error_list.append(f"{evidence_label} `{_path_label(path)}` must be a path")
        return False
    for parent in (path.parent, *path.parent.parents):
        parent_label = f"{evidence_label} parent"
        try:
            if parent.is_symlink():
                if evidence_label == "evidence file":
                    error_list.append(EVIDENCE_FILE_PARENT_SYMLINK_DIAGNOSTIC)
                elif evidence_label == "evidence directory":
                    error_list.append(EVIDENCE_DIRECTORY_PARENT_SYMLINK_DIAGNOSTIC)
                else:
                    error_list.append(
                        f"{parent_label} `{_path_label(parent)}` "
                        "must not be a symlink"
                    )
                return False
            if parent.exists() and not parent.is_dir():
                if evidence_label == "evidence file":
                    error_list.append(EVIDENCE_FILE_PARENT_DIRECTORY_DIAGNOSTIC)
                elif evidence_label == "evidence directory":
                    error_list.append(EVIDENCE_DIRECTORY_PARENT_DIRECTORY_DIAGNOSTIC)
                else:
                    error_list.append(
                        f"{parent_label} `{_path_label(parent)}` "
                        "must be a directory when it exists"
                    )
                return False
        except (OSError, RuntimeError) as error:
            del error
            if evidence_label == "evidence file":
                error_list.append(EVIDENCE_FILE_PARENT_INSPECTION_DIAGNOSTIC)
            elif evidence_label == "evidence directory":
                error_list.append(EVIDENCE_DIRECTORY_PARENT_INSPECTION_DIAGNOSTIC)
            else:
                parent_path_label = _path_label(parent)
                error_list.append(
                    f"{parent_label} `{parent_path_label}` cannot be inspected"
                )
            return False
    return True


def scan_evidence_directory_json(
    directory: Path,
    errors: list[str],
) -> list[Path]:
    """Return JSON evidence files under `directory`, recording scan failures."""

    error_list = _require_error_list(errors)
    if not isinstance(directory, Path):
        error_list.append(EVIDENCE_DIRECTORY_PATH_DIAGNOSTIC)
        return []
    is_dir = inspect_evidence_directory(directory, error_list)
    if is_dir is None:
        return []
    if not is_dir:
        error_list.append(EVIDENCE_DIRECTORY_MISSING_DIAGNOSTIC)
        return []
    try:
        return sorted(directory.rglob("*.json"))
    except (OSError, RuntimeError) as error:
        del error
        error_list.append(EVIDENCE_DIRECTORY_SCAN_DIAGNOSTIC)
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
    reserved_error_count = len(error_list)
    reserved_outputs = reserved_output_path_identities(
        reserved_output_paths,
        error_list,
        label=output_label,
    )
    if len(error_list) != reserved_error_count:
        return
    if not reserved_outputs:
        return

    def check(path: object) -> None:
        is_file = inspect_evidence_file(path, error_list)
        if is_file is None:
            return
        if not is_file:
            error_list.append(EVIDENCE_FILE_MISSING_DIAGNOSTIC)
            return
        resolved = resolve_evidence_path(path, error_list)
        if resolved is None:
            return
        reserved_output = reserved_outputs.get(resolved)
        if reserved_output is not None:
            del reserved_output
            error_list.append(EVIDENCE_FILE_RESERVED_CONFLICT_DIAGNOSTIC)

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
        is_file = inspect_evidence_file(path, error_list)
        if is_file is None:
            return
        if not is_file:
            error_list.append(EVIDENCE_FILE_MISSING_DIAGNOSTIC)
            return
        resolved = resolve_evidence_path(path, error_list)
        if resolved is None:
            return
        reserved_output = reserved_outputs.get(resolved)
        if reserved_output is not None:
            del reserved_output
            error_list.append(EVIDENCE_FILE_RESERVED_CONFLICT_DIAGNOSTIC)
            return
        previous = seen.get(resolved)
        if previous is not None:
            previous_path, previous_explicit = previous
            try:
                files.remove(previous_path)
            except ValueError:
                pass
            if explicit and previous_explicit:
                del previous_path
                error_list.append(EVIDENCE_FILE_DUPLICATE_EXPLICIT_DIAGNOSTIC)
            elif explicit or previous_explicit:
                error_list.append(EVIDENCE_FILE_SOURCE_OVERLAP_DIAGNOSTIC)
            else:
                del previous_path
                error_list.append(EVIDENCE_FILE_DUPLICATE_DISCOVERED_DIAGNOSTIC)
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
            error_list.append(EVIDENCE_DIRECTORY_MISSING_DIAGNOSTIC)
            continue
        discovered = scan_evidence_directory_json(directory, error_list)
        for path in discovered:
            add(path, explicit=False)

    return files
