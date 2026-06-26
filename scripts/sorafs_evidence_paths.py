"""Shared evidence file discovery for SoraFS rollout gates."""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path


def resolve_evidence_path(
    path: Path,
    errors: list[str],
    *,
    label: str = "evidence path",
) -> Path | None:
    """Return the canonical identity for an evidence path, recording failures."""

    try:
        return path.resolve()
    except (OSError, RuntimeError) as error:
        errors.append(f"{label} `{path}` cannot be resolved: {error}")
        return None


def evidence_path_identities(paths: Sequence[Path], errors: list[str]) -> set[Path]:
    """Return canonical identities for paths, skipping paths already recorded as errors."""

    identities: set[Path] = set()
    for path in paths:
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
    paths: Sequence[Path],
    errors: list[str],
    *,
    label: str = "reserved output",
) -> dict[Path, Path]:
    """Return canonical identities for output paths that evidence must not reuse."""

    identities: dict[Path, Path] = {}
    for path in paths:
        resolved = resolve_evidence_path(path, errors, label=label)
        if resolved is not None:
            identities[resolved] = path
    return identities


def inspect_evidence_directory(
    directory: Path,
    errors: list[str],
) -> bool | None:
    """Return whether `directory` is a directory, recording inspection failures."""

    try:
        return directory.is_dir()
    except (OSError, RuntimeError) as error:
        errors.append(f"evidence directory `{directory}` cannot be inspected: {error}")
        return None


def scan_evidence_directory_json(
    directory: Path,
    errors: list[str],
) -> list[Path]:
    """Return JSON evidence files under `directory`, recording scan failures."""

    try:
        return sorted(directory.rglob("*.json"))
    except (OSError, RuntimeError) as error:
        errors.append(f"failed to scan evidence directory `{directory}`: {error}")
        return []


def record_reserved_output_evidence_conflicts(
    evidence_dirs: Sequence[Path],
    evidence_files: Sequence[Path],
    reserved_output_paths: Sequence[Path],
    errors: list[str],
    *,
    reserved_label: str = "reserved output",
) -> None:
    """Record evidence files that resolve to a reserved output path."""

    reserved_outputs = reserved_output_path_identities(
        reserved_output_paths,
        errors,
        label=reserved_label,
    )
    if not reserved_outputs:
        return

    def check(path: Path) -> None:
        resolved = resolve_evidence_path(path, errors)
        if resolved is None:
            return
        reserved_output = reserved_outputs.get(resolved)
        if reserved_output is not None:
            errors.append(
                f"evidence file `{path}` conflicts with {reserved_label} "
                f"`{reserved_output}`"
            )

    for path in evidence_files:
        check(path)

    for directory in evidence_dirs:
        is_dir = inspect_evidence_directory(directory, errors)
        if not is_dir:
            continue
        discovered = scan_evidence_directory_json(directory, errors)
        for path in discovered:
            check(path)


def discover_evidence_files(
    evidence_dirs: Sequence[Path],
    evidence_files: Sequence[Path],
    errors: list[str],
    *,
    reserved_output_paths: Sequence[Path] = (),
) -> list[Path]:
    """Discover evidence JSON files while rejecting ambiguous identities."""

    files: list[Path] = []
    seen: dict[Path, tuple[Path, bool]] = {}
    reserved_outputs = reserved_output_path_identities(reserved_output_paths, errors)

    def add(path: Path, *, explicit: bool) -> None:
        resolved = resolve_evidence_path(path, errors)
        if resolved is None:
            return
        reserved_output = reserved_outputs.get(resolved)
        if reserved_output is not None:
            errors.append(
                f"evidence file `{path}` conflicts with reserved output "
                f"`{reserved_output}`"
            )
            return
        previous = seen.get(resolved)
        if previous is not None:
            previous_path, previous_explicit = previous
            if explicit and previous_explicit:
                errors.append(
                    f"duplicate explicit evidence file `{path}` matches `{previous_path}`"
                )
            elif explicit or previous_explicit:
                duplicate_source = "both --evidence and --evidence-dir"
                errors.append(
                    f"evidence file `{path}` is provided by {duplicate_source}"
                )
            else:
                errors.append(
                    f"duplicate evidence file `{path}` also discovered from "
                    f"`{previous_path}`"
                )
            return
        seen[resolved] = (path, explicit)
        files.append(path)

    for path in evidence_files:
        add(path, explicit=True)

    for directory in evidence_dirs:
        is_dir = inspect_evidence_directory(directory, errors)
        if is_dir is None:
            continue
        if not is_dir:
            errors.append(
                f"evidence directory `{directory}` must exist and be a directory"
            )
            continue
        discovered = scan_evidence_directory_json(directory, errors)
        for path in discovered:
            add(path, explicit=False)

    return files
