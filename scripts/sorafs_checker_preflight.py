"""Shared fail-closed preflight checks for SoraFS evidence checkers."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any

from sorafs_evidence_paths import record_reserved_output_evidence_conflicts
from sorafs_path_identity import error_diagnostic_label, path_diagnostic_label
from sorafs_path_identity import resolve_path_identity


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("checker preflight errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("checker preflight errors must be a list of strings")
        if (
            not error.strip()
            or error != error.strip()
            or any(ord(character) < 32 or ord(character) == 127 for character in error)
        ):
            raise ValueError(
                "checker preflight errors must contain non-empty canonical strings"
            )
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


def _checker_error_messages(errors: Any) -> tuple[str, ...]:
    """Return canonical checker error messages or reject malformed inputs."""

    if isinstance(errors, (str, bytes, bytearray, Mapping)) or not isinstance(
        errors,
        Iterable,
    ):
        raise ValueError("checker error messages must be a sequence of strings")
    messages = tuple(errors)
    for error in messages:
        if not isinstance(error, str):
            raise ValueError("checker error messages must be a sequence of strings")
        if (
            not error.strip()
            or error != error.strip()
            or any(ord(character) < 32 or ord(character) == 127 for character in error)
        ):
            raise ValueError(
                "checker error message must be a non-empty canonical string"
            )
    return messages


def _checker_notice_message(message: Any) -> str:
    """Return a checker notice message or reject unsafe stderr text."""

    if (
        not isinstance(message, str)
        or not message.strip()
        or message != message.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in message)
    ):
        raise ValueError("checker notice message must be a non-empty canonical string")
    return message


def _checker_artifact_error_message(message: Any, *, label: str) -> str:
    """Return a canonical artifact error message or reject unsafe text."""

    if (
        not isinstance(message, str)
        or not message.strip()
        or message != message.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in message)
    ):
        raise ValueError(f"{label} must be a non-empty canonical string")
    return message


def _checker_path_sequence(
    paths: Any,
    errors: list[str],
    *,
    label: str,
) -> Sequence[Any] | None:
    if isinstance(paths, (str, bytes, bytearray, Mapping)) or not isinstance(
        paths,
        Sequence,
    ):
        errors.append(f"{label} paths must be a sequence")
        return None
    return paths


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
        error_list.append(
            f"{path_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return None
    try:
        return path.exists()
    except (OSError, RuntimeError) as error:
        path_display = path_diagnostic_label(path)
        error_list.append(
            f"{path_label} `{path_display}` cannot be inspected: "
            f"{error_diagnostic_label(error, path_label=path_display)}"
        )
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
        error_list.append(
            f"{path_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return None
    try:
        return path.is_dir()
    except (OSError, RuntimeError) as error:
        path_display = path_diagnostic_label(path)
        error_list.append(
            f"{path_label} `{path_display}` cannot be inspected: "
            f"{error_diagnostic_label(error, path_label=path_display)}"
        )
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
        error_list.append(
            f"{path_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return None
    try:
        return path.is_symlink()
    except (OSError, RuntimeError) as error:
        path_display = path_diagnostic_label(path)
        error_list.append(
            f"{path_label} `{path_display}` cannot be inspected: "
            f"{error_diagnostic_label(error, path_label=path_display)}"
        )
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
        error_list.append(
            f"{output_label} `{path_diagnostic_label(path)}` must be a path"
        )
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
            error_list.append(
                f"{parent_label} `{path_diagnostic_label(parent)}` "
                "must not be a symlink"
            )
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
                    f"{parent_label} `{path_diagnostic_label(parent)}` "
                    "must be a directory when it exists"
                )
                return False
    return True


def validate_checker_summary_output(summary_out: Path, errors: list[str]) -> bool:
    """Validate the optional checker summary output target."""

    error_list = _require_error_list(errors)
    if not isinstance(summary_out, Path):
        error_list.append(
            f"--summary-out `{path_diagnostic_label(summary_out)}` must be a path"
        )
        return False
    summary_out_is_symlink = inspect_checker_preflight_path_is_symlink(
        summary_out,
        error_list,
        label="--summary-out",
    )
    if summary_out_is_symlink is None:
        return False
    if summary_out_is_symlink:
        error_list.append(
            f"--summary-out `{path_diagnostic_label(summary_out)}` "
            "must not be a symlink"
        )
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
            error_list.append(
                f"--summary-out `{path_diagnostic_label(summary_out)}` "
                "must not be a directory"
            )
            return False
    return validate_checker_output_parent(
        summary_out, error_list, label="--summary-out"
    )


def validate_checker_preflight(args: argparse.Namespace) -> list[str]:
    """Validate checker CLI inputs before reading evidence artifacts."""

    errors = validate_checker_evidence_inputs(args)
    if errors:
        return errors
    summary_out = getattr(args, "summary_out", None)
    if summary_out is None:
        return errors
    if not isinstance(summary_out, Path):
        errors.append(
            f"--summary-out `{path_diagnostic_label(summary_out)}` must be a path"
        )
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
                    path_diagnostic_label(summary_out),
                    path_diagnostic_label(evidence_file),
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
    errors: list[str] = []
    evidence_dirs = getattr(args, "evidence_dir", None)
    evidence_files = getattr(args, "evidence", None)
    evidence_dirs = [] if evidence_dirs is None else evidence_dirs
    evidence_files = [] if evidence_files is None else evidence_files
    evidence_dir_items = _checker_path_sequence(
        evidence_dirs,
        errors,
        label="--evidence-dir",
    )
    evidence_file_items = _checker_path_sequence(
        evidence_files,
        errors,
        label="--evidence",
    )
    if errors:
        return errors
    assert evidence_dir_items is not None
    assert evidence_file_items is not None
    for evidence_dir in evidence_dir_items:
        if not isinstance(evidence_dir, Path):
            errors.append(
                f"--evidence-dir `{path_diagnostic_label(evidence_dir)}` "
                "must be a path"
            )
    for evidence_file in evidence_file_items:
        if isinstance(evidence_file, Path):
            continue
        if (
            isinstance(evidence_file, str)
            and evidence_file.strip()
            and evidence_file == evidence_file.strip()
        ):
            continue
        errors.append(
            f"--evidence `{path_diagnostic_label(evidence_file)}` "
            "must be a path or evidence spec"
        )
    if errors:
        return errors
    evidence_dirs = evidence_dir_items
    evidence_files = evidence_file_items
    if evidence_dirs or evidence_files:
        return []
    return ["provide --evidence-dir or --evidence"]


def _validate_checker_summary_keys(value: Any) -> None:
    """Reject summary mappings with malformed keys before JSON rendering."""

    if isinstance(value, Mapping):
        for key, nested_value in value.items():
            if (
                not isinstance(key, str)
                or not key.strip()
                or key != key.strip()
                or any(ord(character) < 32 or ord(character) == 127 for character in key)
            ):
                raise ValueError(
                    "checker summary keys must be non-empty canonical strings"
                )
            _validate_checker_summary_keys(nested_value)
        return
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        for item in value:
            _validate_checker_summary_keys(item)


def render_checker_summary(summary: Mapping[str, Any]) -> str:
    """Render checker summary JSON in the canonical deterministic format."""

    if not isinstance(summary, Mapping):
        raise ValueError("checker summary must be an object")
    _validate_checker_summary_keys(summary)
    return json.dumps(summary, indent=2, sort_keys=True, allow_nan=False) + "\n"


def render_and_write_checker_summary(
    summary_out: Path | None,
    summary: Mapping[str, Any],
) -> tuple[str, list[str]]:
    """Render and optionally write a checker summary."""

    try:
        rendered_summary = render_checker_summary(summary)
    except (TypeError, ValueError) as error:
        return "", [
            f"failed to render checker summary JSON: {error_diagnostic_label(error)}"
        ]
    errors = write_checker_summary(summary_out, rendered_summary)
    if not errors and summary_out is None:
        sys.stdout.write(rendered_summary)
    return rendered_summary, errors


def write_checker_summary(summary_out: Path | None, summary_text: str) -> list[str]:
    """Write an optional checker summary while reporting filesystem errors."""

    if not isinstance(summary_text, str):
        return ["checker summary text must be a string"]
    if summary_out is None:
        return []
    if not isinstance(summary_out, Path):
        return [
            f"--summary-out `{path_diagnostic_label(summary_out)}` must be a path"
        ]

    errors: list[str] = []
    if not validate_checker_summary_output(summary_out, errors):
        return errors

    parent = summary_out.parent
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        parent_label = path_diagnostic_label(parent)
        return [
            "failed to create --summary-out parent `{}`: {}".format(
                parent_label,
                error_diagnostic_label(error, path_label=parent_label),
            )
        ]

    try:
        summary_out.write_text(summary_text, encoding="utf-8")
    except (OSError, RuntimeError) as error:
        summary_label = path_diagnostic_label(summary_out)
        return [
            "failed to write --summary-out `{}`: {}".format(
                summary_label,
                error_diagnostic_label(error, path_label=summary_label),
            )
        ]
    return []


def emit_checker_error_lines(errors: Iterable[str]) -> None:
    """Emit one stderr ERROR line for each checker error."""

    for error in _checker_error_messages(errors):
        print(f"ERROR: {error}", file=sys.stderr)


def emit_checker_exception(error: BaseException) -> None:
    """Emit one sanitized stderr ERROR line for a caught checker exception."""

    emit_checker_error_lines((error_diagnostic_label(error),))


def emit_checker_error_block(title: str, errors: Iterable[str]) -> None:
    """Emit a checker error heading followed by bullet diagnostics."""

    error_messages = _checker_error_messages(errors)
    print(title, file=sys.stderr)
    for error in error_messages:
        print(f"- {error}", file=sys.stderr)


def emit_checker_notice(message: str) -> None:
    """Emit a human checker notice on stderr."""

    print(_checker_notice_message(message), file=sys.stderr)


def artifact_path_label(artifact: Any) -> str:
    """Return an artifact path label for diagnostics."""

    if not isinstance(artifact, Mapping):
        return "<unknown>"
    path = artifact.get("path")
    if (
        isinstance(path, str)
        and path
        and path == path.strip()
        and not any(ord(character) < 32 or ord(character) == 127 for character in path)
    ):
        return path
    return "<unknown>"


def _recordable_artifact_path_label(
    artifact: dict[str, Any],
    summary_errors: list[str],
) -> str | None:
    path = artifact.get("path")
    if path is None or not isinstance(path, str) or not path:
        return "<unknown>"
    label = artifact_path_label(artifact)
    if label == "<unknown>":
        summary_errors.append("artifact path label must be a non-empty canonical string")
        return None
    return label


def _artifact_error_bucket_is_canonical(errors: list[Any]) -> bool:
    """Return whether an artifact error bucket contains canonical strings."""

    for error in errors:
        try:
            _checker_artifact_error_message(error, label="artifact existing error")
        except ValueError:
            return False
    return True


def record_artifact_error(
    artifact: Any,
    error: str,
    summary_errors: list[str],
    *,
    summary_error: str | None = None,
) -> None:
    """Mark an evidence artifact invalid and mirror the error to summary errors."""

    summary_error_list = _require_error_list(summary_errors)
    artifact_error = _checker_artifact_error_message(
        error,
        label="artifact error",
    )
    summary_error_text = (
        None
        if summary_error is None
        else _checker_artifact_error_message(
            summary_error,
            label="artifact summary error",
        )
    )
    if not isinstance(artifact, dict):
        summary_error_list.append(f"<unknown>: {summary_error_text or artifact_error}")
        return
    path_label = _recordable_artifact_path_label(artifact, summary_error_list)
    if path_label is None:
        return
    artifact["valid"] = False
    artifact_errors = artifact.get("errors")
    if not isinstance(artifact_errors, list) or not _artifact_error_bucket_is_canonical(
        artifact_errors
    ):
        artifact_errors = []
        artifact["errors"] = artifact_errors
    artifact_errors.append(artifact_error)
    summary_error_list.append(
        f"{path_label}: {summary_error_text or artifact_error}"
    )
