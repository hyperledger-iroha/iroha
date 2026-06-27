"""Shared evidence file discovery for SoraFS rollout gates."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from sorafs_path_identity import resolve_path_identity


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("evidence path errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("evidence path errors must be a list of strings")
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

    resolved = resolve_evidence_path(path, errors)
    return resolved is not None and resolved in explicit_identities


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
        error_list.append(f"evidence directory `{directory}` must be a path")
        return None
    try:
        return directory.is_dir()
    except (OSError, RuntimeError) as error:
        error_list.append(
            f"evidence directory `{directory}` cannot be inspected: {error}"
        )
        return None


def scan_evidence_directory_json(
    directory: Path,
    errors: list[str],
) -> list[Path]:
    """Return JSON evidence files under `directory`, recording scan failures."""

    error_list = _require_error_list(errors)
    if not isinstance(directory, Path):
        error_list.append(f"evidence directory `{directory}` must be a path")
        return []
    try:
        return sorted(directory.rglob("*.json"))
    except (OSError, RuntimeError) as error:
        error_list.append(
            f"failed to scan evidence directory `{directory}`: {error}"
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
                f"evidence file `{path}` conflicts with {output_label} "
                f"`{reserved_output}`"
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
    reserved_outputs = reserved_output_path_identities(
        reserved_output_paths, error_list
    )

    def add(path: Path, *, explicit: bool) -> None:
        resolved = resolve_evidence_path(path, error_list)
        if resolved is None:
            return
        reserved_output = reserved_outputs.get(resolved)
        if reserved_output is not None:
            error_list.append(
                f"evidence file `{path}` conflicts with reserved output "
                f"`{reserved_output}`"
            )
            return
        previous = seen.get(resolved)
        if previous is not None:
            previous_path, previous_explicit = previous
            if explicit and previous_explicit:
                error_list.append(
                    f"duplicate explicit evidence file `{path}` matches `{previous_path}`"
                )
            elif explicit or previous_explicit:
                duplicate_source = "both --evidence and --evidence-dir"
                error_list.append(
                    f"evidence file `{path}` is provided by {duplicate_source}"
                )
            else:
                error_list.append(
                    f"duplicate evidence file `{path}` also discovered from "
                    f"`{previous_path}`"
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
                f"evidence directory `{directory}` must exist and be a directory"
            )
            continue
        discovered = scan_evidence_directory_json(directory, error_list)
        for path in discovered:
            add(path, explicit=False)

    return files
