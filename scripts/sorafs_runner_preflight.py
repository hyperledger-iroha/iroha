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

from sorafs_path_identity import resolve_path_identity


RUNNER_ARG_FIELD_RE = re.compile(r"[a-z][a-z0-9_]*\Z")


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("runner preflight errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("runner preflight errors must be a list of strings")
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
        error_list.append(f"{path_label} `{path}` must be a path")
        return None
    try:
        return path.exists()
    except (OSError, RuntimeError) as error:
        error_list.append(f"{path_label} `{path}` cannot be inspected: {error}")
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
        error_list.append(f"{path_label} `{path}` must be a path")
        return None
    try:
        return path.is_symlink()
    except (OSError, RuntimeError) as error:
        error_list.append(f"{path_label} `{path}` cannot be inspected: {error}")
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
        error_list.append(f"{path_label} `{path}` must be a path")
        return None
    try:
        return path.is_file()
    except (OSError, RuntimeError) as error:
        error_list.append(f"{path_label} `{path}` cannot be inspected: {error}")
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
        error_list.append(f"{path_label} `{path}` must be a path")
        return None
    try:
        return path.stat().st_size
    except (OSError, RuntimeError) as error:
        error_list.append(f"{path_label} `{path}` cannot be inspected: {error}")
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
        error_list.append(f"{path_label} `{path}` must be a path")
        return None
    try:
        return path.is_dir()
    except (OSError, RuntimeError) as error:
        error_list.append(f"{path_label} `{path}` cannot be inspected: {error}")
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
        error_list.append(f"{output_label} `{path}` must be a path")
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
            error_list.append(f"{parent_label} `{parent}` must not be a symlink")
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
                    f"{parent_label} `{parent}` must be a directory when it exists"
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
        error_list.append(f"{output_label} `{out_dir}` must be a path")
        return False
    out_dir_is_symlink = inspect_runner_path_is_symlink(
        out_dir,
        error_list,
        label=output_label,
    )
    if out_dir_is_symlink is None:
        return False
    if out_dir_is_symlink:
        error_list.append(f"{output_label} `{out_dir}` must not be a symlink")
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
                f"{output_label} `{out_dir}` must be a directory when it exists"
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
        errors.append(f"--verifier `{verifier}` must exist and be a file")
    else:
        verifier_is_file = inspect_runner_path_is_file(
            verifier,
            errors,
            label="--verifier",
        )
        if verifier_is_file is False:
            errors.append(f"--verifier `{verifier}` must exist and be a file")

    out_dir = getattr(args, "out_dir", None)
    if not isinstance(out_dir, Path):
        errors.append(f"--out-dir `{out_dir}` must be a path")
        return errors
    out_dir_identity = resolve_runner_output_path(out_dir, errors)
    out_dir_valid = validate_runner_output_dir(out_dir, errors)

    configured_summary_out = getattr(args, "summary_out", None)
    if configured_summary_out is None and not out_dir_valid:
        return errors
    summary_out = configured_summary_out or out_dir / summary_filename
    if not isinstance(summary_out, Path):
        errors.append(f"--summary-out `{summary_out}` must be a path")
    else:
        summary_out_is_symlink = inspect_runner_path_is_symlink(
            summary_out,
            errors,
            label="--summary-out",
        )
        if summary_out_is_symlink:
            errors.append(f"--summary-out `{summary_out}` must not be a symlink")
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
                errors.append(f"--summary-out `{summary_out}` must not be a directory")
            elif summary_out_is_dir is False:
                summary_out_identity = resolve_runner_output_path(summary_out, errors)
                if (
                    out_dir_identity is not None
                    and summary_out_identity is not None
                    and summary_out_identity == out_dir_identity
                ):
                    errors.append(
                        "--summary-out `{}` must not be the same path as --out-dir `{}`".format(
                            summary_out, out_dir
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
                        summary_out, out_dir
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
    if seen is None:
        seen = {}
    for path in paths:
        path_exists = inspect_runner_path_exists(path, errors, label=label)
        if path_exists is None:
            continue
        path_is_symlink = False
        if not path_exists:
            inspected_symlink = inspect_runner_path_is_symlink(
                path,
                errors,
                label=label,
            )
            if inspected_symlink is None:
                continue
            path_is_symlink = inspected_symlink
        if not path_exists and not path_is_symlink:
            errors.append(f"{label} `{path}` must exist and be a file")
            continue
        resolved = resolve_runner_input_file(path, errors)
        if resolved is None:
            continue
        path_is_file = inspect_runner_path_is_file(path, errors, label=label)
        if path_is_file is None:
            continue
        if not path_is_file:
            errors.append(f"{label} `{path}` must exist and be a file")
            continue
        previous = seen.get(resolved)
        if previous is not None:
            previous_label, previous_path = previous
            errors.append(
                f"duplicate {label} input `{path}` matches "
                f"{previous_label} `{previous_path}`"
            )
            continue
        seen[resolved] = (label, path)
    return errors


def require_existing_dirs(
    paths: Sequence[Path],
    label: str,
    *,
    seen: InputDirIdentities | None = None,
) -> list[str]:
    """Validate existing runner input directories and reject repeated identities."""

    errors: list[str] = []
    if seen is None:
        seen = {}
    for path in paths:
        path_exists = inspect_runner_path_exists(path, errors, label=label)
        if path_exists is None:
            continue
        path_is_symlink = False
        if not path_exists:
            inspected_symlink = inspect_runner_path_is_symlink(
                path,
                errors,
                label=label,
            )
            if inspected_symlink is None:
                continue
            path_is_symlink = inspected_symlink
        if not path_exists and not path_is_symlink:
            errors.append(f"{label} `{path}` must exist and be a directory")
            continue
        resolved = resolve_runner_input_file(path, errors)
        if resolved is None:
            continue
        path_is_dir = inspect_runner_path_is_dir(path, errors, label=label)
        if path_is_dir is None:
            continue
        if not path_is_dir:
            errors.append(f"{label} `{path}` must exist and be a directory")
            continue
        previous = seen.get(resolved)
        if previous is not None:
            previous_label, previous_path = previous
            errors.append(
                f"duplicate {label} directory `{path}` matches "
                f"{previous_label} `{previous_path}`"
            )
            continue
        seen[resolved] = (label, path)
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
    steps = command_plan_steps(plan)
    if steps is None:
        return [COMMAND_PLAN_SHAPE_DIAGNOSTIC]
    seen: dict[Path, tuple[str, Path]] = {}
    reserved: dict[Path, Path] = {}
    for path in reserved_output_paths:
        resolved = resolve_runner_output_path(path, errors)
        if resolved is not None:
            reserved[resolved] = path

    for step in steps:
        artifact = getattr(step, "artifact", None)
        if artifact is None:
            continue
        label = str(getattr(step, "label", "<unknown>"))
        if not isinstance(artifact, Path):
            errors.append(f"{label} artifact `{artifact}` must be a path")
            continue
        artifact_is_symlink = inspect_runner_path_is_symlink(
            artifact,
            errors,
            label=f"{label} artifact",
        )
        if artifact_is_symlink is None:
            continue
        if artifact_is_symlink:
            errors.append(f"{label} artifact `{artifact}` must not be a symlink")
            continue
        artifact_exists = inspect_runner_path_exists(
            artifact,
            errors,
            label=f"{label} artifact",
        )
        if artifact_exists is None:
            continue
        if artifact_exists:
            errors.append(f"{label} artifact `{artifact}` must not already exist")
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
                f"{label} artifact `{artifact}` "
                f"{RESERVED_OUTPUT_ARTIFACT_DIAGNOSTIC} `{reserved_path}`"
            )
            continue
        previous = seen.get(resolved)
        if previous is not None:
            previous_label, previous_artifact = previous
            errors.append(
                f"duplicate planned artifact `{artifact}` for {label} matches "
                f"{previous_label} `{previous_artifact}`"
            )
            continue
        seen[resolved] = (label, artifact)
    return errors


def render_runner_plan(plan: Mapping[str, Any]) -> str:
    """Render a SoraFS runner command plan with the shared dry-run JSON shape."""

    return json.dumps(plan, indent=2, sort_keys=True, allow_nan=False) + "\n"


def write_runner_plan(plan: Mapping[str, Any]) -> list[str]:
    """Write a SoraFS runner command plan to stdout."""

    try:
        sys.stdout.write(render_runner_plan(plan))
    except (TypeError, ValueError) as error:
        return [f"failed to render runner plan JSON: {error}"]
    return []


def emit_runner_error_lines(errors: Iterable[str]) -> None:
    """Emit one stderr ERROR line for each runner error."""

    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)


def emit_runner_error_block(title: str, errors: Iterable[str]) -> None:
    """Emit a runner error heading followed by bullet diagnostics."""

    print(title, file=sys.stderr)
    for error in errors:
        print(f"- {error}", file=sys.stderr)


def emit_runner_notice(message: str) -> None:
    """Emit a human runner notice on stderr."""

    print(message, file=sys.stderr)


def run_command_plan(plan: Any, out_dir: Path) -> int:
    """Run a SoraFS collection command plan with structured launch/output errors."""

    errors: list[str] = []
    steps = command_plan_steps(plan)
    if steps is None:
        emit_runner_error_lines((COMMAND_PLAN_SHAPE_DIAGNOSTIC,))
        return 1
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
        emit_runner_error_lines((f"failed to create --out-dir `{out_dir}`: {error}",))
        return 1

    for step in steps:
        label = str(getattr(step, "label", "<unknown>"))
        artifact = getattr(step, "artifact", None)
        command = getattr(step, "command", None)
        if not isinstance(command, list) or not all(
            isinstance(part, str) for part in command
        ):
            emit_runner_error_lines((f"{label} command must be a list of strings",))
            return 1
        emit_runner_notice(f"RUN {label}: {shlex.join(command)}")
        try:
            result = subprocess.run(command, check=False)
        except OSError as error:
            emit_runner_error_lines((f"{label} failed to launch: {error}",))
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
                        f"{label} expected artifact `{artifact}` "
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
                    (f"{label} did not write expected artifact `{artifact}`",)
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
                    (f"{label} wrote empty expected artifact `{artifact}`",)
                )
                return 1
    return 0
