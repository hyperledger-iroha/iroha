"""Shared fail-closed preflight checks for SoraFS evidence checkers."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Iterable, Mapping
from pathlib import Path
from typing import Any

from sorafs_evidence_paths import record_reserved_output_evidence_conflicts
from sorafs_path_identity import resolve_path_identity


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("checker preflight errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("checker preflight errors must be a list of strings")
    return errors


def _require_label(label: Any) -> str:
    if (
        not isinstance(label, str)
        or not label.strip()
        or label != label.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in label)
    ):
        raise ValueError("checker preflight label must be a non-empty canonical string")
    return label


def resolve_checker_preflight_path(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> Path | None:
    """Return a checker preflight path identity, recording resolver failures."""

    return resolve_path_identity(path, errors, label=label)


def inspect_checker_preflight_path_exists(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a preflight path exists, recording inspection failures."""

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(f"{path_label} `{path}` must be a path")
        return None
    try:
        return path.exists()
    except (OSError, RuntimeError) as error:
        error_list.append(f"{path_label} `{path}` cannot be inspected: {error}")
        return None


def inspect_checker_preflight_path_is_dir(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a preflight path is a directory, recording failures."""

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(f"{path_label} `{path}` must be a path")
        return None
    try:
        return path.is_dir()
    except (OSError, RuntimeError) as error:
        error_list.append(f"{path_label} `{path}` cannot be inspected: {error}")
        return None


def inspect_checker_preflight_path_is_symlink(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a preflight path is a symlink, recording failures."""

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(f"{path_label} `{path}` must be a path")
        return None
    try:
        return path.is_symlink()
    except (OSError, RuntimeError) as error:
        error_list.append(f"{path_label} `{path}` cannot be inspected: {error}")
        return None


def validate_checker_output_parent(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool:
    """Validate a checker output path's parent chain before creating files."""

    error_list = _require_error_list(errors)
    output_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(f"{output_label} `{path}` must be a path")
        return False
    for parent in (path.parent, *path.parent.parents):
        parent_label = f"{output_label} parent"
        parent_is_symlink = inspect_checker_preflight_path_is_symlink(
            parent,
            error_list,
            label=parent_label,
        )
        if parent_is_symlink is None:
            return False
        if parent_is_symlink:
            error_list.append(f"{parent_label} `{parent}` must not be a symlink")
            return False
        parent_exists = inspect_checker_preflight_path_exists(
            parent,
            error_list,
            label=parent_label,
        )
        if parent_exists is None:
            return False
        if parent_exists:
            parent_is_dir = inspect_checker_preflight_path_is_dir(
                parent,
                error_list,
                label=parent_label,
            )
            if parent_is_dir is None:
                return False
            if not parent_is_dir:
                error_list.append(
                    f"{parent_label} `{parent}` must be a directory when it exists"
                )
                return False
    return True


def validate_checker_summary_output(summary_out: Path, errors: list[str]) -> bool:
    """Validate the optional checker summary output target."""

    error_list = _require_error_list(errors)
    if not isinstance(summary_out, Path):
        error_list.append(f"--summary-out `{summary_out}` must be a path")
        return False
    summary_out_is_symlink = inspect_checker_preflight_path_is_symlink(
        summary_out,
        error_list,
        label="--summary-out",
    )
    if summary_out_is_symlink is None:
        return False
    if summary_out_is_symlink:
        error_list.append(f"--summary-out `{summary_out}` must not be a symlink")
        return False
    summary_out_exists = inspect_checker_preflight_path_exists(
        summary_out,
        error_list,
        label="--summary-out",
    )
    if summary_out_exists is None:
        return False
    if summary_out_exists:
        summary_out_is_dir = inspect_checker_preflight_path_is_dir(
            summary_out,
            error_list,
            label="--summary-out",
        )
        if summary_out_is_dir is None:
            return False
        if summary_out_is_dir:
            error_list.append(f"--summary-out `{summary_out}` must not be a directory")
            return False
    return validate_checker_output_parent(
        summary_out, error_list, label="--summary-out"
    )


def validate_checker_preflight(args: argparse.Namespace) -> list[str]:
    """Validate checker CLI inputs before reading evidence artifacts."""

    errors = validate_checker_evidence_inputs(args)
    summary_out = getattr(args, "summary_out", None)
    if summary_out is None:
        return errors
    if not isinstance(summary_out, Path):
        errors.append(f"--summary-out `{summary_out}` must be a path")
        return errors
    if not validate_checker_summary_output(summary_out, errors):
        return errors
    summary_out_identity = resolve_checker_preflight_path(
        summary_out,
        errors,
        label="--summary-out",
    )
    evidence_files = getattr(args, "evidence", None) or []
    for evidence_file in evidence_files:
        if not isinstance(evidence_file, Path):
            continue
        evidence_identity = resolve_checker_preflight_path(
            evidence_file,
            errors,
            label="--evidence",
        )
        if (
            summary_out_identity is not None
            and evidence_identity is not None
            and summary_out_identity == evidence_identity
        ):
            errors.append(
                "--summary-out `{}` must not be the same path as --evidence `{}`".format(
                    summary_out, evidence_file
                )
            )
    evidence_dirs = [
        directory
        for directory in getattr(args, "evidence_dir", None) or []
        if isinstance(directory, Path)
    ]
    if summary_out_identity is not None:
        record_reserved_output_evidence_conflicts(
            evidence_dirs,
            [],
            [summary_out],
            errors,
            reserved_label="--summary-out",
        )
    return errors


def validate_checker_evidence_inputs(args: argparse.Namespace) -> list[str]:
    """Validate that a checker received at least one evidence source."""

    if not hasattr(args, "evidence_dir") and not hasattr(args, "evidence"):
        return []
    evidence_dirs = getattr(args, "evidence_dir", None) or []
    evidence_files = getattr(args, "evidence", None) or []
    if evidence_dirs or evidence_files:
        return []
    return ["provide --evidence-dir or --evidence"]


def render_checker_summary(summary: Mapping[str, Any]) -> str:
    """Render checker summary JSON in the canonical deterministic format."""

    return json.dumps(summary, indent=2, sort_keys=True, allow_nan=False) + "\n"


def render_and_write_checker_summary(
    summary_out: Path | None,
    summary: Mapping[str, Any],
) -> tuple[str, list[str]]:
    """Render and optionally write a checker summary."""

    try:
        rendered_summary = render_checker_summary(summary)
    except (TypeError, ValueError) as error:
        return "", [f"failed to render checker summary JSON: {error}"]
    errors = write_checker_summary(summary_out, rendered_summary)
    if not errors and summary_out is None:
        sys.stdout.write(rendered_summary)
    return rendered_summary, errors


def write_checker_summary(summary_out: Path | None, summary_text: str) -> list[str]:
    """Write an optional checker summary while reporting filesystem errors."""

    if summary_out is None:
        return []
    if not isinstance(summary_out, Path):
        return [f"--summary-out `{summary_out}` must be a path"]

    errors: list[str] = []
    if not validate_checker_summary_output(summary_out, errors):
        return errors

    parent = summary_out.parent
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        return [f"failed to create --summary-out parent `{parent}`: {error}"]

    try:
        summary_out.write_text(summary_text, encoding="utf-8")
    except (OSError, RuntimeError) as error:
        return [f"failed to write --summary-out `{summary_out}`: {error}"]
    return []


def emit_checker_error_lines(errors: Iterable[str]) -> None:
    """Emit one stderr ERROR line for each checker error."""

    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)


def emit_checker_error_block(title: str, errors: Iterable[str]) -> None:
    """Emit a checker error heading followed by bullet diagnostics."""

    print(title, file=sys.stderr)
    for error in errors:
        print(f"- {error}", file=sys.stderr)


def emit_checker_notice(message: str) -> None:
    """Emit a human checker notice on stderr."""

    print(message, file=sys.stderr)


def artifact_path_label(artifact: Any) -> str:
    """Return an artifact path label for diagnostics."""

    if not isinstance(artifact, Mapping):
        return "<unknown>"
    path = artifact.get("path")
    if isinstance(path, str) and path:
        return path
    return "<unknown>"


def record_artifact_error(
    artifact: Any,
    error: str,
    summary_errors: list[str],
    *,
    summary_error: str | None = None,
) -> None:
    """Mark an evidence artifact invalid and mirror the error to summary errors."""

    if not isinstance(artifact, dict):
        summary_errors.append(f"<unknown>: {summary_error or error}")
        return
    artifact["valid"] = False
    artifact_errors = artifact.get("errors")
    if not isinstance(artifact_errors, list):
        artifact_errors = []
        artifact["errors"] = artifact_errors
    artifact_errors.append(error)
    summary_errors.append(
        f"{artifact_path_label(artifact)}: {summary_error or error}"
    )
