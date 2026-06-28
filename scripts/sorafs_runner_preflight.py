"""Shared fail-closed preflight checks for SoraFS evidence runners."""

from __future__ import annotations

import argparse
import json
import re
import shlex
import subprocess
import sys
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any

from sorafs_path_identity import error_diagnostic_label, path_diagnostic_label
from sorafs_path_identity import resolve_path_identity


RUNNER_ARG_FIELD_RE = re.compile(r"[a-z][a-z0-9_]*\Z")


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("runner preflight errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("runner preflight errors must be a list of strings")
        if (
            not error.strip()
            or error != error.strip()
            or any(ord(character) < 32 or ord(character) == 127 for character in error)
        ):
            raise ValueError(
                "runner preflight errors must contain non-empty canonical strings"
            )
    return errors


def _require_label(label: Any) -> str:
    if (
        not isinstance(label, str)
        or not label.strip()
        or label != label.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in label)
    ):
        raise ValueError("runner preflight label must be a non-empty canonical string")
    return label


def _require_runner_arg_field(field: Any) -> str:
    if not isinstance(field, str) or RUNNER_ARG_FIELD_RE.fullmatch(field) is None:
        raise ValueError("runner argument field must be a snake_case string")
    return field


def _runner_error_messages(errors: Any) -> tuple[str, ...]:
    """Return runner error messages or reject scalar/object containers."""

    if isinstance(errors, (str, bytes, bytearray, Mapping)) or not isinstance(
        errors,
        Iterable,
    ):
        raise ValueError("runner error messages must be a sequence of strings")
    messages = tuple(errors)
    for error in messages:
        if not isinstance(error, str):
            raise ValueError("runner error messages must be a sequence of strings")
        if (
            not error.strip()
            or error != error.strip()
            or any(ord(character) < 32 or ord(character) == 127 for character in error)
        ):
            raise ValueError(
                "runner error message must be a non-empty canonical string"
            )
    return messages


def _runner_notice_message(message: Any) -> str:
    """Return a runner notice message or reject unsafe stderr text."""

    if (
        not isinstance(message, str)
        or not message.strip()
        or message != message.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in message)
    ):
        raise ValueError("runner notice message must be a non-empty canonical string")
    return message


def _runner_path_sequence(paths: Any, errors: list[str], *, label: str) -> Sequence[Any] | None:
    if isinstance(paths, (str, bytes, bytearray, Mapping)) or not isinstance(
        paths, Sequence
    ):
        errors.append(f"{label} paths must be a sequence")
        return None
    return paths


def _runner_input_identity_map(
    seen: Any,
    errors: list[str],
    *,
    label: str,
) -> dict[Path, tuple[str, Path]] | None:
    if seen is None:
        return {}
    if not isinstance(seen, dict):
        errors.append(f"{label} identity map must be a dictionary")
        return None
    for identity, previous in seen.items():
        if (
            not isinstance(identity, Path)
            or not isinstance(previous, tuple)
            or len(previous) != 2
        ):
            errors.append(
                f"{label} identity map entries must be path identities and "
                "(label, path) pairs"
            )
            return None
        previous_label, previous_path = previous
        try:
            _require_label(previous_label)
        except ValueError:
            errors.append(
                f"{label} identity map entries must be path identities and "
                "(label, path) pairs"
            )
            return None
        if not isinstance(previous_path, Path):
            errors.append(
                f"{label} identity map entries must be path identities and "
                "(label, path) pairs"
            )
            return None
    return seen


def _reserved_output_path_sequence(
    paths: Any,
    errors: list[str],
) -> Sequence[Any] | None:
    return _runner_path_sequence(paths, errors, label="reserved output")


def _record_path_inspection_failure(
    errors: list[str],
    *,
    label: str,
    path: Any,
    error: BaseException,
) -> None:
    path_display = path_diagnostic_label(path)
    errors.append(
        f"{label} `{path_display}` cannot be inspected: "
        f"{error_diagnostic_label(error, path_label=path_display)}"
    )


def inspect_runner_path_exists(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a runner path exists, recording inspection failures."""

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
        _record_path_inspection_failure(
            error_list,
            label=path_label,
            path=path,
            error=error,
        )
        return None


def inspect_runner_path_is_symlink(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a runner path is a symlink, recording failures."""

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
        _record_path_inspection_failure(
            error_list,
            label=path_label,
            path=path,
            error=error,
        )
        return None


def inspect_runner_path_is_file(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a runner path is a file, recording inspection failures."""

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(
            f"{path_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return None
    try:
        return path.is_file()
    except (OSError, RuntimeError) as error:
        _record_path_inspection_failure(
            error_list,
            label=path_label,
            path=path,
            error=error,
        )
        return None


def inspect_runner_path_size(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> int | None:
    """Return a path size in bytes, recording inspection failures."""

    error_list = _require_error_list(errors)
    path_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(
            f"{path_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return None
    try:
        return path.stat().st_size
    except (OSError, RuntimeError) as error:
        _record_path_inspection_failure(
            error_list,
            label=path_label,
            path=path,
            error=error,
        )
        return None


def inspect_runner_path_is_dir(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool | None:
    """Return whether a runner path is a directory, recording failures."""

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
        _record_path_inspection_failure(
            error_list,
            label=path_label,
            path=path,
            error=error,
        )
        return None


def validate_runner_output_parent(
    path: Path,
    errors: list[str],
    *,
    label: str,
) -> bool:
    """Validate an output path's parent chain before creating files."""

    error_list = _require_error_list(errors)
    output_label = _require_label(label)
    if not isinstance(path, Path):
        error_list.append(
            f"{output_label} `{path_diagnostic_label(path)}` must be a path"
        )
        return False
    for parent in (path.parent, *path.parent.parents):
        parent_label = f"{output_label} parent"
        parent_is_symlink = inspect_runner_path_is_symlink(
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
        parent_exists = inspect_runner_path_exists(
            parent,
            error_list,
            label=parent_label,
        )
        if parent_exists is None:
            return False
        if parent_exists:
            parent_is_dir = inspect_runner_path_is_dir(
                parent,
                error_list,
                label=parent_label,
            )
            if parent_is_dir is None:
                return False
            if parent_is_dir is False:
                error_list.append(
                    f"{parent_label} `{path_diagnostic_label(parent)}` "
                    "must be a directory when it exists"
                )
                return False
    return True


def validate_runner_output_dir(
    out_dir: Path,
    errors: list[str],
    *,
    label: str = "--out-dir",
) -> bool:
    """Validate a runner output directory target before command execution."""

    error_list = _require_error_list(errors)
    output_label = _require_label(label)
    if not isinstance(out_dir, Path):
        error_list.append(
            f"{output_label} `{path_diagnostic_label(out_dir)}` must be a path"
        )
        return False
    out_dir_is_symlink = inspect_runner_path_is_symlink(
        out_dir,
        error_list,
        label=output_label,
    )
    if out_dir_is_symlink is None:
        return False
    if out_dir_is_symlink:
        error_list.append(
            f"{output_label} `{path_diagnostic_label(out_dir)}` "
            "must not be a symlink"
        )
        return False
    out_dir_exists = inspect_runner_path_exists(out_dir, error_list, label=output_label)
    if out_dir_exists is None:
        return False
    if out_dir_exists:
        out_dir_is_dir = inspect_runner_path_is_dir(
            out_dir,
            error_list,
            label=output_label,
        )
        if out_dir_is_dir is None:
            return False
        if out_dir_is_dir is False:
            error_list.append(
                f"{output_label} `{path_diagnostic_label(out_dir)}` "
                "must be a directory when it exists"
            )
            return False
        return True
    return validate_runner_output_parent(out_dir, error_list, label=output_label)


def validate_runner_preflight(
    args: argparse.Namespace,
    *,
    summary_filename: str,
) -> list[str]:
    """Validate verifier and output targets before building a command plan."""

    errors: list[str] = []
    verifier = getattr(args, "verifier", None)
    if not isinstance(verifier, Path):
        errors.append(
            f"--verifier `{path_diagnostic_label(verifier)}` "
            "must exist and be a file"
        )
    else:
        verifier_is_file = inspect_runner_path_is_file(
            verifier,
            errors,
            label="--verifier",
        )
        if verifier_is_file is False:
            errors.append(
                f"--verifier `{path_diagnostic_label(verifier)}` "
                "must exist and be a file"
            )

    out_dir = getattr(args, "out_dir", None)
    if not isinstance(out_dir, Path):
        errors.append(f"--out-dir `{path_diagnostic_label(out_dir)}` must be a path")
        return errors
    out_dir_identity = resolve_runner_output_path(out_dir, errors)
    out_dir_valid = validate_runner_output_dir(out_dir, errors)

    configured_summary_out = getattr(args, "summary_out", None)
    if configured_summary_out is None and not out_dir_valid:
        return errors
    summary_out = configured_summary_out or out_dir / summary_filename
    if not isinstance(summary_out, Path):
        errors.append(
            f"--summary-out `{path_diagnostic_label(summary_out)}` must be a path"
        )
    else:
        summary_out_is_symlink = inspect_runner_path_is_symlink(
            summary_out,
            errors,
            label="--summary-out",
        )
        if summary_out_is_symlink:
            errors.append(
                f"--summary-out `{path_diagnostic_label(summary_out)}` "
                "must not be a symlink"
            )
        summary_out_exists = inspect_runner_path_exists(
            summary_out,
            errors,
            label="--summary-out",
        )
        if summary_out_exists:
            summary_out_is_dir = inspect_runner_path_is_dir(
                summary_out,
                errors,
                label="--summary-out",
            )
            if summary_out_is_dir:
                errors.append(
                    f"--summary-out `{path_diagnostic_label(summary_out)}` "
                    "must not be a directory"
                )
            elif summary_out_is_dir is False:
                summary_out_identity = resolve_runner_output_path(summary_out, errors)
                if (
                    out_dir_identity is not None
                    and summary_out_identity is not None
                    and summary_out_identity == out_dir_identity
                ):
                    errors.append(
                        "--summary-out `{}` must not be the same path as --out-dir `{}`".format(
                            path_diagnostic_label(summary_out),
                            path_diagnostic_label(out_dir),
                        )
                    )
        elif summary_out_exists is False:
            validate_runner_output_parent(summary_out, errors, label="--summary-out")
            summary_out_identity = resolve_runner_output_path(summary_out, errors)
            if (
                out_dir_identity is not None
                and summary_out_identity is not None
                and summary_out_identity == out_dir_identity
            ):
                errors.append(
                    "--summary-out `{}` must not be the same path as --out-dir `{}`".format(
                        path_diagnostic_label(summary_out),
                        path_diagnostic_label(out_dir),
                    )
                )
    return errors


def resolve_runner_input_file(path: Path, errors: list[str]) -> Path | None:
    """Return a canonical runner input path identity, recording resolver failures."""

    return resolve_path_identity(path, errors, label="input file")


InputFileIdentities = dict[Path, tuple[str, Path]]
InputDirIdentities = dict[Path, tuple[str, Path]]
RESERVED_OUTPUT_ARTIFACT_DIAGNOSTIC = "must not be the same path as reserved output"
COMMAND_PLAN_SHAPE_DIAGNOSTIC = "command plan must be a sequence of steps"


def command_plan_steps(plan: Any) -> Sequence[Any] | None:
    """Return command-plan steps or reject scalar/object containers."""

    if isinstance(plan, (str, bytes, bytearray, Mapping)) or not isinstance(
        plan, Sequence
    ):
        return None
    return plan


def _command_vector_errors(step_label: str, command: Any) -> list[str]:
    if not isinstance(command, list) or not command or not all(
        isinstance(part, str) for part in command
    ):
        return [f"{step_label} command must be a non-empty list of strings"]
    errors: list[str] = []
    if not command[0] or command[0] != command[0].strip():
        errors.append(
            f"{step_label} command executable must be a non-empty canonical string"
        )
    for index, part in enumerate(command):
        if "\0" in part:
            errors.append(
                f"{step_label} command argument {index} must not contain NUL bytes"
            )
        elif any(ord(character) < 32 or ord(character) == 127 for character in part):
            errors.append(
                f"{step_label} command argument {index} "
                "must not contain control characters"
            )
    return errors


def validate_command_plan_step_shapes(plan: Any) -> list[str]:
    """Reject malformed command-plan step fields before filesystem mutation."""

    errors: list[str] = []
    steps = command_plan_steps(plan)
    if steps is None:
        return [COMMAND_PLAN_SHAPE_DIAGNOSTIC]
    for index, step in enumerate(steps):
        label = getattr(step, "label", None)
        try:
            step_label = _require_label(label)
        except ValueError:
            errors.append(
                f"command-plan step {index} label must be a non-empty canonical string"
            )
            step_label = f"command-plan step {index}"
        artifact = getattr(step, "artifact", None)
        if artifact is not None and not isinstance(artifact, Path):
            errors.append(
                f"{step_label} artifact `{path_diagnostic_label(artifact)}` "
                "must be a path"
            )
        command = getattr(step, "command", None)
        errors.extend(_command_vector_errors(step_label, command))
    return errors


def runner_arg_label(field: str) -> str:
    """Return the CLI option label for an argparse namespace field."""

    field_name = _require_runner_arg_field(field)
    return f"--{field_name.replace('_', '-')}"


def require_runner_positive_int(
    args: argparse.Namespace,
    field: str,
    errors: list[str],
    *,
    allow_none: bool = False,
) -> bool:
    """Require a direct runner namespace value to be a positive integer."""

    error_list = _require_error_list(errors)
    field_name = _require_runner_arg_field(field)
    value = getattr(args, field_name, None)
    if value is None and allow_none:
        return True
    valid = isinstance(value, int) and not isinstance(value, bool) and value > 0
    if not valid:
        suffix = " when supplied" if allow_none else ""
        error_list.append(f"{runner_arg_label(field_name)} must be positive{suffix}")
    return valid


def require_runner_non_negative_int(
    args: argparse.Namespace,
    field: str,
    errors: list[str],
) -> bool:
    """Require a direct runner namespace value to be a non-negative integer."""

    error_list = _require_error_list(errors)
    field_name = _require_runner_arg_field(field)
    value = getattr(args, field_name, None)
    valid = isinstance(value, int) and not isinstance(value, bool) and value >= 0
    if not valid:
        error_list.append(f"{runner_arg_label(field_name)} must be non-negative")
    return valid


def require_existing_files(
    paths: Sequence[Path],
    label: str,
    *,
    seen: InputFileIdentities | None = None,
) -> list[str]:
    """Validate existing runner input files and reject repeated path identities."""

    errors: list[str] = []
    path_label = _require_label(label)
    path_items = _runner_path_sequence(paths, errors, label=path_label)
    if path_items is None:
        return errors
    seen_map = _runner_input_identity_map(seen, errors, label=path_label)
    if seen_map is None:
        return errors
    for path in path_items:
        path_exists = inspect_runner_path_exists(path, errors, label=path_label)
        if path_exists is None:
            continue
        path_is_symlink = False
        if not path_exists:
            inspected_symlink = inspect_runner_path_is_symlink(
                path,
                errors,
                label=path_label,
            )
            if inspected_symlink is None:
                continue
            path_is_symlink = inspected_symlink
        if not path_exists and not path_is_symlink:
            errors.append(
                f"{path_label} `{path_diagnostic_label(path)}` "
                "must exist and be a file"
            )
            continue
        resolved = resolve_runner_input_file(path, errors)
        if resolved is None:
            continue
        path_is_file = inspect_runner_path_is_file(path, errors, label=path_label)
        if path_is_file is None:
            continue
        if not path_is_file:
            errors.append(
                f"{path_label} `{path_diagnostic_label(path)}` "
                "must exist and be a file"
            )
            continue
        previous = seen_map.get(resolved)
        if previous is not None:
            previous_label, previous_path = previous
            errors.append(
                f"duplicate {path_label} input `{path_diagnostic_label(path)}` "
                f"matches {previous_label} `{path_diagnostic_label(previous_path)}`"
            )
            continue
        seen_map[resolved] = (path_label, path)
    return errors


def require_existing_dirs(
    paths: Sequence[Path],
    label: str,
    *,
    seen: InputDirIdentities | None = None,
) -> list[str]:
    """Validate existing runner input directories and reject repeated identities."""

    errors: list[str] = []
    path_label = _require_label(label)
    path_items = _runner_path_sequence(paths, errors, label=path_label)
    if path_items is None:
        return errors
    seen_map = _runner_input_identity_map(seen, errors, label=path_label)
    if seen_map is None:
        return errors
    for path in path_items:
        path_exists = inspect_runner_path_exists(path, errors, label=path_label)
        if path_exists is None:
            continue
        path_is_symlink = False
        if not path_exists:
            inspected_symlink = inspect_runner_path_is_symlink(
                path,
                errors,
                label=path_label,
            )
            if inspected_symlink is None:
                continue
            path_is_symlink = inspected_symlink
        if not path_exists and not path_is_symlink:
            errors.append(
                f"{path_label} `{path_diagnostic_label(path)}` "
                "must exist and be a directory"
            )
            continue
        resolved = resolve_runner_input_file(path, errors)
        if resolved is None:
            continue
        path_is_dir = inspect_runner_path_is_dir(path, errors, label=path_label)
        if path_is_dir is None:
            continue
        if not path_is_dir:
            errors.append(
                f"{path_label} `{path_diagnostic_label(path)}` "
                "must exist and be a directory"
            )
            continue
        previous = seen_map.get(resolved)
        if previous is not None:
            previous_label, previous_path = previous
            errors.append(
                f"duplicate {path_label} directory `{path_diagnostic_label(path)}` "
                f"matches {previous_label} `{path_diagnostic_label(previous_path)}`"
            )
            continue
        seen_map[resolved] = (path_label, path)
    return errors


def resolve_runner_output_path(path: Path, errors: list[str]) -> Path | None:
    """Return a canonical runner output path identity, recording resolver failures."""

    return resolve_path_identity(path, errors, label="output path")


def validate_command_plan_artifacts(
    plan: Any,
    *,
    reserved_output_paths: Sequence[Path] = (),
) -> list[str]:
    """Reject ambiguous planned artifact outputs before executing commands."""

    errors: list[str] = []
    shape_errors = validate_command_plan_step_shapes(plan)
    if shape_errors:
        return shape_errors
    steps = command_plan_steps(plan)
    assert steps is not None
    seen: dict[Path, tuple[str, Path]] = {}
    reserved: dict[Path, Path] = {}
    reserved_output_items = _reserved_output_path_sequence(
        reserved_output_paths,
        errors,
    )
    if reserved_output_items is None:
        return errors
    for path in reserved_output_items:
        if not isinstance(path, Path):
            errors.append(
                f"reserved output path `{path_diagnostic_label(path)}` "
                "must be a path"
            )
            continue
        resolved = resolve_runner_output_path(path, errors)
        if resolved is not None:
            previous = reserved.get(resolved)
            if previous is not None:
                errors.append(
                    "duplicate reserved output path `{}` matches `{}`".format(
                        path_diagnostic_label(path),
                        path_diagnostic_label(previous),
                    )
                )
                continue
            reserved[resolved] = path
    if errors:
        return errors

    for step in steps:
        artifact = getattr(step, "artifact", None)
        if artifact is None:
            continue
        label = getattr(step, "label")
        artifact_is_symlink = inspect_runner_path_is_symlink(
            artifact,
            errors,
            label=f"{label} artifact",
        )
        if artifact_is_symlink is None:
            continue
        if artifact_is_symlink:
            errors.append(
                f"{label} artifact `{path_diagnostic_label(artifact)}` "
                "must not be a symlink"
            )
            continue
        artifact_exists = inspect_runner_path_exists(
            artifact,
            errors,
            label=f"{label} artifact",
        )
        if artifact_exists is None:
            continue
        if artifact_exists:
            errors.append(
                f"{label} artifact `{path_diagnostic_label(artifact)}` "
                "must not already exist"
            )
            continue
        if not validate_runner_output_parent(
            artifact,
            errors,
            label=f"{label} artifact",
        ):
            continue
        resolved = resolve_runner_output_path(artifact, errors)
        if resolved is None:
            continue
        reserved_path = reserved.get(resolved)
        if reserved_path is not None:
            errors.append(
                f"{label} artifact `{path_diagnostic_label(artifact)}` "
                f"{RESERVED_OUTPUT_ARTIFACT_DIAGNOSTIC} "
                f"`{path_diagnostic_label(reserved_path)}`"
            )
            continue
        previous = seen.get(resolved)
        if previous is not None:
            previous_label, previous_artifact = previous
            errors.append(
                f"duplicate planned artifact `{path_diagnostic_label(artifact)}` "
                f"for {label} matches {previous_label} "
                f"`{path_diagnostic_label(previous_artifact)}`"
            )
            continue
        seen[resolved] = (label, artifact)
    return errors


def render_runner_plan(plan: Mapping[str, Any]) -> str:
    """Render a SoraFS runner command plan with the shared dry-run JSON shape."""

    if not isinstance(plan, Mapping):
        raise ValueError("runner plan must be an object")
    return json.dumps(plan, indent=2, sort_keys=True, allow_nan=False) + "\n"


def write_runner_plan(plan: Mapping[str, Any]) -> list[str]:
    """Write a SoraFS runner command plan to stdout."""

    try:
        sys.stdout.write(render_runner_plan(plan))
    except (TypeError, ValueError) as error:
        return [
            f"failed to render runner plan JSON: {error_diagnostic_label(error)}"
        ]
    return []


def emit_runner_error_lines(errors: Iterable[str]) -> None:
    """Emit one stderr ERROR line for each runner error."""

    for error in _runner_error_messages(errors):
        print(f"ERROR: {error}", file=sys.stderr)


def emit_runner_exception(error: BaseException) -> None:
    """Emit one sanitized stderr ERROR line for a caught runner exception."""

    emit_runner_error_lines((error_diagnostic_label(error),))


def emit_runner_error_block(title: str, errors: Iterable[str]) -> None:
    """Emit a runner error heading followed by bullet diagnostics."""

    error_messages = _runner_error_messages(errors)
    print(title, file=sys.stderr)
    for error in error_messages:
        print(f"- {error}", file=sys.stderr)


def emit_runner_notice(message: str) -> None:
    """Emit a human runner notice on stderr."""

    print(_runner_notice_message(message), file=sys.stderr)


def run_command_plan(plan: Any, out_dir: Path) -> int:
    """Run a SoraFS collection command plan with structured launch/output errors."""

    errors: list[str] = []
    shape_errors = validate_command_plan_step_shapes(plan)
    if shape_errors:
        emit_runner_error_lines(shape_errors)
        return 1
    steps = command_plan_steps(plan)
    assert steps is not None
    if validate_runner_output_dir(out_dir, errors):
        errors.extend(
            validate_command_plan_artifacts(
                steps,
                reserved_output_paths=(out_dir,),
            )
        )
    if errors:
        emit_runner_error_lines(errors)
        return 1

    try:
        out_dir.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        out_dir_label = path_diagnostic_label(out_dir)
        emit_runner_error_lines(
            (
                f"failed to create --out-dir `{out_dir_label}`: "
                f"{error_diagnostic_label(error, path_label=out_dir_label)}",
            )
        )
        return 1

    for step in steps:
        label = getattr(step, "label")
        artifact = getattr(step, "artifact", None)
        command = getattr(step, "command", None)
        command_errors = _command_vector_errors(label, command)
        if command_errors:
            emit_runner_error_lines(command_errors)
            return 1
        emit_runner_notice(f"RUN {label}: {shlex.join(command)}")
        try:
            result = subprocess.run(command, check=False)
        except OSError as error:
            emit_runner_error_lines(
                (f"{label} failed to launch: {error_diagnostic_label(error)}",)
            )
            return 1
        if result.returncode != 0:
            emit_runner_error_lines(
                (f"{label} failed with exit code {result.returncode}",)
            )
            return result.returncode
        if artifact is not None:
            artifact_errors: list[str] = []
            artifact_is_symlink = inspect_runner_path_is_symlink(
                artifact,
                artifact_errors,
                label=f"{label} expected artifact",
            )
            if artifact_errors:
                emit_runner_error_lines(artifact_errors)
                return 1
            if artifact_is_symlink:
                emit_runner_error_lines(
                    (
                        f"{label} expected artifact "
                        f"`{path_diagnostic_label(artifact)}` "
                        "must not be a symlink",
                    )
                )
                return 1
            artifact_is_file = inspect_runner_path_is_file(
                artifact,
                artifact_errors,
                label=f"{label} expected artifact",
            )
            if artifact_errors:
                emit_runner_error_lines(artifact_errors)
                return 1
            if not artifact_is_file:
                emit_runner_error_lines(
                    (
                        f"{label} did not write expected artifact "
                        f"`{path_diagnostic_label(artifact)}`",
                    )
                )
                return 1
            artifact_size = inspect_runner_path_size(
                artifact,
                artifact_errors,
                label=f"{label} expected artifact",
            )
            if artifact_errors:
                emit_runner_error_lines(artifact_errors)
                return 1
            if artifact_size == 0:
                emit_runner_error_lines(
                    (
                        f"{label} wrote empty expected artifact "
                        f"`{path_diagnostic_label(artifact)}`",
                    )
                )
                return 1
    return 0
