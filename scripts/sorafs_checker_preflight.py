"""Shared fail-closed preflight checks for SoraFS evidence checkers."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Iterable, Mapping
from pathlib import Path
from typing import Any

from sorafs_evidence_paths import record_reserved_output_evidence_conflicts


def resolve_checker_preflight_path(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> Path | None:
    """Return a checker preflight path identity, recording resolver failures."""

    try:
        return path.resolve()
    except (OSError, RuntimeError) as error:
        errors.append(f"{label} `{path}` cannot be resolved: {error}")
        return None


def inspect_checker_preflight_path_exists(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a preflight path exists, recording inspection failures."""

    try:
        return path.exists()
    except (OSError, RuntimeError) as error:
        errors.append(f"{label} `{path}` cannot be inspected: {error}")
        return None


def inspect_checker_preflight_path_is_dir(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a preflight path is a directory, recording failures."""

    try:
        return path.is_dir()
    except (OSError, RuntimeError) as error:
        errors.append(f"{label} `{path}` cannot be inspected: {error}")
        return None


def validate_checker_preflight(args: argparse.Namespace) -> list[str]:
    """Validate checker CLI inputs before reading evidence artifacts."""

    errors = validate_checker_evidence_inputs(args)
    summary_out = getattr(args, "summary_out", None)
    if summary_out is None:
        return errors
    if not isinstance(summary_out, Path):
        errors.append(f"--summary-out `{summary_out}` must be a path")
        return errors
    summary_out_exists = inspect_checker_preflight_path_exists(
        summary_out,
        errors,
        label="--summary-out",
    )
    if summary_out_exists is None:
        return errors
    if summary_out_exists:
        summary_out_is_dir = inspect_checker_preflight_path_is_dir(
            summary_out,
            errors,
            label="--summary-out",
        )
        if summary_out_is_dir is None:
            return errors
        if summary_out_is_dir:
            errors.append(f"--summary-out `{summary_out}` must not be a directory")
            return errors
    parent = summary_out.parent
    parent_exists = inspect_checker_preflight_path_exists(
        parent,
        errors,
        label="--summary-out parent",
    )
    if parent_exists is None:
        return errors
    if parent_exists:
        parent_is_dir = inspect_checker_preflight_path_is_dir(
            parent,
            errors,
            label="--summary-out parent",
        )
        if parent_is_dir is None:
            return errors
        if not parent_is_dir:
            errors.append(
                f"--summary-out parent `{parent}` must be a directory when it exists"
            )
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


def artifact_path_label(artifact: Mapping[str, Any]) -> str:
    """Return an artifact path label for diagnostics."""

    path = artifact.get("path")
    if isinstance(path, str) and path:
        return path
    return "<unknown>"


def record_artifact_error(
    artifact: dict[str, Any],
    error: str,
    summary_errors: list[str],
    *,
    summary_error: str | None = None,
) -> None:
    """Mark an evidence artifact invalid and mirror the error to summary errors."""

    artifact["valid"] = False
    artifact_errors = artifact.get("errors")
    if not isinstance(artifact_errors, list):
        artifact_errors = []
        artifact["errors"] = artifact_errors
    artifact_errors.append(error)
    summary_errors.append(
        f"{artifact_path_label(artifact)}: {summary_error or error}"
    )
