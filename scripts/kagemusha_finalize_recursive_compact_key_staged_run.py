#!/usr/bin/env python3
"""Finalize a completed staged ABI-7 recursive compact keygen run."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402
import kagemusha_production_readiness as readiness  # noqa: E402
import kagemusha_recursive_compact_key_evidence as compact_evidence  # noqa: E402


DEFAULT_TEMP_ROOT = Path("/tmp").resolve()
DEFAULT_STAGED_ARTIFACT_DIR = (
    DEFAULT_TEMP_ROOT
    / "iroha-codex-recursive-compact-keygen-staged"
    / "artifacts"
    / "kagemusha"
)
DEFAULT_EXIT_FILE = DEFAULT_TEMP_ROOT / "iroha-codex-kagemusha-compact-keygen-staged.exit"
DEFAULT_ARTIFACT_DIR = Path("artifacts/kagemusha")
STAGED_FINALIZER_SUMMARY_SCHEMA = (
    "iroha.kagemusha.recursive_compact_key_staged_finalizer.v1"
)
EXIT_MARKER_MAX_BYTES = 32
RUN_REPORT_FILENAME = "recursive-compact-key-staged-run.json"
STAGED_RUN_REPORT_SCHEMA = "iroha.kagemusha.recursive_compact_key_staged_run.v1"
EXECUTION_REPORT_FILENAME = "recursive-compact-key-execution.json"
EXECUTION_REPORT_SCHEMA = "iroha.kagemusha.recursive_compact_key_execution.v1"
MAX_STAGED_RUN_REPORT_BYTES = 16 * 1024
MAX_EXECUTION_REPORT_BYTES = 16 * 1024
STAGED_RUNNER_TEMP_SUFFIX = ".staged-runner.tmp"
CONTROL_EXIT_MARKER_REDACTION = "<unsafe-exit-marker>"
SECRET_EXIT_MARKER_REDACTION = "<redacted-secret-marker>"


def _default_generated_at_utc() -> str:
    return (
        dt.datetime.now(dt.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z")
    )


def _secret_path_error(path: Path, label: str) -> str | None:
    path_text = str(path)
    if device_lab.SECRET_RE.search(path_text):
        return f"{label} must not contain secret-looking material"
    if device_lab._contains_control_character(path_text):
        return f"{label} must not contain control characters"
    if (
        path_text != path_text.strip()
        or device_lab._path_has_surrounding_whitespace_component(path)
    ):
        return f"{label} must not contain surrounding whitespace"
    if "\\" in path_text:
        return f"{label} must not contain backslashes"
    if ".." in path.parts:
        return f"{label} must be canonical"
    return None


def _display_exit_marker(marker: str) -> str:
    if not marker:
        return "<empty>"
    if device_lab.SECRET_RE.search(marker):
        return SECRET_EXIT_MARKER_REDACTION
    if device_lab._contains_control_character(marker):
        return CONTROL_EXIT_MARKER_REDACTION
    return marker


def _validate_report_command(
    value: object,
    label: str,
    expected: str,
    description: str,
) -> list[str]:
    """Validate a staged report command without echoing unsafe bytes."""

    if not isinstance(value, str) or not value:
        return [f"{label} command must be a non-empty string"]
    errors: list[str] = []
    if value != value.strip():
        errors.append(f"{label} command must not contain surrounding whitespace")
    if device_lab._contains_control_character(value):
        errors.append(f"{label} command must not contain control characters")
    if device_lab.SECRET_RE.search(value):
        errors.append(f"{label} command must not contain secret-looking material")
    if value != expected:
        errors.append(f"{label} command must match the canonical {description}")
    return errors


def validate_directory_path(path: Path, label: str, *, must_exist: bool) -> list[str]:
    """Reject directory aliases before reading or publishing key evidence."""

    secret_error = _secret_path_error(path, label)
    if secret_error is not None:
        return [secret_error]
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        ancestor_errors = device_lab.validate_no_symlink_ancestors(
            path,
            f"{label} ancestor directory",
        )
        if ancestor_errors:
            return ancestor_errors
        if must_exist:
            return [f"{label} is missing"]
        return []
    except OSError:
        return [f"{label} metadata could not be read"]
    if stat.S_ISLNK(mode):
        return [f"{label} must not be a symlink"]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    if not stat.S_ISDIR(mode):
        return [f"{label} must be a directory"]
    return []


def validate_exit_file_path_shape(path: Path) -> list[str]:
    """Reject unsafe exit marker path strings before directory metadata."""

    secret_error = _secret_path_error(path, "--exit-file")
    if secret_error is not None:
        return [secret_error]
    return []


def validate_no_staged_runner_temp_outputs(
    staged_artifact_dir: Path,
    label: str,
) -> list[str]:
    """Reject incomplete staged runs that still have runner-owned temp files."""

    try:
        entries = list(staged_artifact_dir.iterdir())
    except OSError:
        return [f"{label} could not be listed"]
    if any(entry.name.endswith(STAGED_RUNNER_TEMP_SUFFIX) for entry in entries):
        return [f"{label} contains runner temporary outputs; staged run is incomplete"]
    return []


def _validate_output_file_path(path: Path, label: str, *, replace: bool) -> list[str]:
    secret_error = _secret_path_error(path, label)
    if secret_error is not None:
        return [secret_error]
    parent_errors = validate_directory_path(path.parent, f"{label} parent", must_exist=True)
    if parent_errors:
        return parent_errors
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return []
    except OSError:
        return [f"{label} metadata could not be read"]
    if stat.S_ISLNK(mode):
        return [f"{label} must not be a symlink"]
    if not stat.S_ISREG(mode):
        return [f"{label} must be a regular file"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return [f"{label} must not be hardlinked"]
    if not replace:
        return [f"{label} already exists; refuse to overwrite without --replace"]
    return []


def _file_identity(file_stat: os.stat_result) -> tuple[int, int]:
    return file_stat.st_dev, file_stat.st_ino


def _directory_open_flags() -> int:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _set_private_directory_permissions(
    path: Path,
    label: str,
    *,
    expected_identity: tuple[int, int] | None = None,
) -> list[str]:
    try:
        dir_fd = os.open(path, _directory_open_flags())
    except OSError:
        return [f"{label} permissions could not be tightened"]
    try:
        directory_stat = os.fstat(dir_fd)
        if not stat.S_ISDIR(directory_stat.st_mode):
            return [f"{label} permissions could not be tightened"]
        if expected_identity is not None and _file_identity(directory_stat) != expected_identity:
            return [f"{label} changed before permissions were tightened"]
        os.fchmod(dir_fd, 0o700)
        directory_stat = os.fstat(dir_fd)
        if stat.S_IMODE(directory_stat.st_mode) != 0o700:
            return [f"{label} permissions must be 0700"]
    except OSError:
        return [f"{label} permissions could not be tightened"]
    finally:
        os.close(dir_fd)
    return []


def _ensure_private_directory(path: Path, label: str) -> list[str]:
    """Create a private directory without following symlinked path components."""

    errors = validate_directory_path(path, label, must_exist=False)
    if errors:
        return errors
    flags = _directory_open_flags()
    if path.is_absolute():
        start_path = Path(path.anchor)
        parts = list(path.parts[1:])
        if parts:
            first_path = start_path / parts[0]
            try:
                if stat.S_ISLNK(first_path.lstat().st_mode):
                    start_path = first_path.resolve(strict=True)
                    parts = parts[1:]
            except FileNotFoundError:
                pass
            except OSError:
                return [f"{label} metadata could not be read"]
    else:
        start_path = Path.cwd()
        parts = list(path.parts)
    try:
        current_fd = os.open(start_path, flags)
    except OSError:
        return [f"{label} metadata could not be read"]
    try:
        filtered_parts = [part for part in parts if part not in ("", ".")]
        if not filtered_parts:
            return [f"{label} must be a directory"]
        for index, part in enumerate(filtered_parts):
            is_final = index == len(filtered_parts) - 1
            created = False
            try:
                next_fd = os.open(part, flags, dir_fd=current_fd)
            except FileNotFoundError:
                try:
                    os.mkdir(part, 0o700, dir_fd=current_fd)
                    created = True
                except FileExistsError:
                    pass
                except OSError:
                    return [f"{label} could not be created"]
                try:
                    next_fd = os.open(part, flags, dir_fd=current_fd)
                except OSError:
                    return [f"{label} changed before permissions were tightened"]
            except OSError:
                post_errors = validate_directory_path(path, label, must_exist=True)
                return post_errors or [f"{label} metadata could not be read"]
            try:
                next_stat = os.fstat(next_fd)
                if not stat.S_ISDIR(next_stat.st_mode):
                    os.close(next_fd)
                    return [f"{label} must be a directory"]
                if created or is_final:
                    os.fchmod(next_fd, 0o700)
                    next_stat = os.fstat(next_fd)
                    if stat.S_IMODE(next_stat.st_mode) != 0o700:
                        os.close(next_fd)
                        return [f"{label} permissions must be 0700"]
            except OSError:
                os.close(next_fd)
                return [f"{label} permissions could not be tightened"]
            os.close(current_fd)
            current_fd = next_fd
    finally:
        os.close(current_fd)
    return validate_directory_path(path, label, must_exist=True)


def _sync_artifact_dir(
    artifact_dir: Path,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    try:
        dir_fd = os.open(artifact_dir, _directory_open_flags())
    except OSError:
        return ["artifact directory could not be synced"]
    try:
        return _sync_artifact_dir_fd(
            dir_fd,
            artifact_dir=artifact_dir,
            expected_identity=expected_identity,
        )
    finally:
        os.close(dir_fd)


def _sync_artifact_dir_fd(
    artifact_dir_fd: int,
    *,
    artifact_dir: Path | None,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    try:
        dir_stat = os.fstat(artifact_dir_fd)
        if not stat.S_ISDIR(dir_stat.st_mode):
            return ["artifact directory could not be synced"]
        if expected_identity is not None and _file_identity(dir_stat) != expected_identity:
            return ["artifact directory changed before sync"]
        if artifact_dir is not None and expected_identity is not None:
            try:
                public_fd = os.open(artifact_dir, _directory_open_flags())
            except OSError:
                return ["artifact directory could not be synced"]
            try:
                public_stat = os.fstat(public_fd)
                if not stat.S_ISDIR(public_stat.st_mode):
                    return ["artifact directory could not be synced"]
                if _file_identity(public_stat) != expected_identity:
                    return ["artifact directory changed before sync"]
            except OSError:
                return ["artifact directory could not be synced"]
            finally:
                os.close(public_fd)
        os.fsync(artifact_dir_fd)
    except OSError:
        return ["artifact directory could not be synced"]
    return []


def _cleanup_temp_parent(
    temp_parent: Path,
    *,
    expected_identity: tuple[int, int],
) -> list[str]:
    try:
        parent_fd = os.open(temp_parent.parent, _directory_open_flags())
    except OSError:
        return ["staged finalizer temporary directory cleanup parent could not be opened"]
    try:
        try:
            temp_parent_stat = os.stat(
                temp_parent.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            return []
        except OSError:
            return ["staged finalizer temporary directory metadata could not be read"]
        if (
            not stat.S_ISDIR(temp_parent_stat.st_mode)
            or _file_identity(temp_parent_stat) != expected_identity
        ):
            return []
        try:
            shutil.rmtree(temp_parent.name, dir_fd=parent_fd)
        except OSError:
            return ["staged finalizer temporary directory could not be removed"]
        try:
            os.fsync(parent_fd)
        except OSError:
            return ["staged finalizer temporary directory cleanup could not be synced"]
    finally:
        os.close(parent_fd)
    return []


def _regular_file_identity(path: Path) -> tuple[int, int] | None:
    try:
        path_stat = path.lstat()
    except OSError:
        return None
    if stat.S_ISLNK(path_stat.st_mode) or not stat.S_ISREG(path_stat.st_mode):
        return None
    return _file_identity(path_stat)


def _regular_file_identity_at(parent_fd: int, name: str) -> tuple[int, int] | None:
    try:
        path_stat = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except OSError:
        return None
    if stat.S_ISLNK(path_stat.st_mode) or not stat.S_ISREG(path_stat.st_mode):
        return None
    return _file_identity(path_stat)


def _sha256_file_with_size_at(
    parent_fd: int,
    name: str,
    label: str,
) -> tuple[str | None, int | None, list[str]]:
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        file_fd = os.open(name, flags, dir_fd=parent_fd)
    except FileNotFoundError:
        return None, None, [f"{label} is missing"]
    except OSError:
        return None, None, [f"{label} could not be read"]
    digest = hashlib.sha256()
    size = 0
    try:
        file_stat = os.fstat(file_fd)
        if not stat.S_ISREG(file_stat.st_mode):
            return None, None, [f"{label} must be a regular file"]
        if file_stat.st_nlink > 1:
            return None, None, [f"{label} must not be hardlinked"]
        while True:
            chunk = os.read(file_fd, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
            size += len(chunk)
    except OSError:
        return None, None, [f"{label} could not be read"]
    finally:
        os.close(file_fd)
    return digest.hexdigest(), size, []


def _write_all(file_fd: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        written = os.write(file_fd, view)
        if written <= 0:
            raise OSError("short write")
        view = view[written:]


def _unlink_file_if_identity(
    path: Path,
    expected_identity: tuple[int, int],
    *,
    label: str | None = None,
) -> list[str]:
    label = label or f"published {path.name}"
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return [f"{label} rollback cleanup parent could not be opened"]
    try:
        try:
            path_stat = os.stat(
                path.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            return []
        except OSError:
            return [f"{label} rollback cleanup metadata could not be read"]
        if (
            stat.S_ISREG(path_stat.st_mode)
            and _file_identity(path_stat) == expected_identity
        ):
            try:
                os.unlink(path.name, dir_fd=parent_fd)
            except OSError:
                return [f"{label} rollback cleanup could not remove file"]
            try:
                os.fsync(parent_fd)
            except OSError:
                return [f"{label} rollback cleanup could not be synced"]
            return []
    finally:
        os.close(parent_fd)
    return []


def _unlink_file_if_identity_at(
    parent_fd: int,
    name: str,
    expected_identity: tuple[int, int],
    *,
    label: str,
) -> list[str]:
    try:
        path_stat = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return []
    except OSError:
        return [f"{label} rollback cleanup metadata could not be read"]
    if stat.S_ISREG(path_stat.st_mode) and _file_identity(path_stat) == expected_identity:
        try:
            os.unlink(name, dir_fd=parent_fd)
        except OSError:
            return [f"{label} rollback cleanup could not remove file"]
        try:
            os.fsync(parent_fd)
        except OSError:
            return [f"{label} rollback cleanup could not be synced"]
        return []
    return []


def _cleanup_published_files(installed: list[tuple[Path, tuple[int, int]]]) -> list[str]:
    errors: list[str] = []
    for path, identity in installed:
        errors.extend(
            _unlink_file_if_identity(
                path,
                identity,
                label=f"published {path.name}",
            )
        )
    return errors


def _cleanup_published_files_at(
    parent_fd: int,
    installed: list[tuple[str, tuple[int, int]]],
) -> list[str]:
    errors: list[str] = []
    for name, identity in installed:
        errors.extend(
            _unlink_file_if_identity_at(
                parent_fd,
                name,
                identity,
                label=f"published {name}",
            )
        )
    return errors


def _read_small_text_file(
    path: Path,
    label: str,
    *,
    max_bytes: int,
) -> tuple[str | None, list[str]]:
    secret_error = _secret_path_error(path, label)
    if secret_error is not None:
        return None, [secret_error]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        return None, ancestor_errors
    try:
        expected_stat = path.lstat()
    except FileNotFoundError:
        return None, [f"{label} is missing"]
    except OSError:
        return None, [f"{label} metadata could not be read"]
    if stat.S_ISLNK(expected_stat.st_mode):
        return None, [f"{label} must not be a symlink"]
    if not stat.S_ISREG(expected_stat.st_mode):
        return None, [f"{label} must be a regular file"]
    if expected_stat.st_size > max_bytes:
        return None, [f"{label} must not exceed {max_bytes} bytes"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return None, [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return None, [f"{label} must not be hardlinked"]
    chunks: list[bytes] = []
    expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if (open_stat.st_dev, open_stat.st_ino) != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, [f"{label} changed while being read"]
            chunks.append(handle.read(max_bytes + 1))
            if len(chunks[0]) > max_bytes:
                return None, [f"{label} must not exceed {max_bytes} bytes"]
            final_stat = path.lstat()
            if (final_stat.st_dev, final_stat.st_ino) != expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} could not be read"]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [f"{label} could not be read"]


def read_exit_marker(path: Path) -> tuple[str | None, list[str]]:
    """Read the staged keygen exit marker."""

    text, errors = _read_small_text_file(
        path,
        "staged keygen exit marker",
        max_bytes=EXIT_MARKER_MAX_BYTES,
    )
    if errors:
        return None, errors
    assert text is not None
    marker = text.strip()
    if text != "0\n" and marker == "0":
        return marker, ["staged keygen exit marker must be exactly 0 followed by newline"]
    return marker, []


def validate_exit_marker(path: Path) -> tuple[str | None, list[str]]:
    """Return errors if the staged keygen did not complete successfully."""

    stripped, errors = read_exit_marker(path)
    if errors:
        return None, errors
    assert stripped is not None
    if stripped != "0":
        return stripped, [
            f"staged keygen exit code must be 0, got {_display_exit_marker(stripped)}"
        ]
    return stripped, []


def _strict_json_loads(text: str, label: str) -> tuple[object | None, list[str]]:
    try:
        return (
            json.loads(
                text,
                object_pairs_hook=device_lab._reject_duplicate_json_object_pairs,
                parse_constant=device_lab._reject_nonfinite_json_constant,
            ),
            [],
        )
    except device_lab.DuplicateJsonKeyError as exc:
        key = device_lab._display_path(exc.key)
        return None, [f"{label} contains duplicate JSON object key {key}"]
    except device_lab.NonFiniteJsonConstantError:
        return None, [
            f"{label} is not strict JSON: non-finite constant "
            f"{device_lab.JSON_NONFINITE_CONSTANT_REDACTION} is not allowed"
        ]
    except json.JSONDecodeError:
        return None, [f"{label} is not valid JSON"]


def validate_staged_run_report(
    *,
    staged_artifact_dir: Path,
    expected_exit_code: int,
    expected_command: str,
) -> tuple[float | None, list[str]]:
    """Validate the staged runner report before trusting a successful marker."""

    label = "staged recursive compact key run report"
    path = staged_artifact_dir / RUN_REPORT_FILENAME
    text, errors = _read_small_text_file(path, label, max_bytes=MAX_STAGED_RUN_REPORT_BYTES)
    if errors:
        return None, errors
    assert text is not None
    document, errors = _strict_json_loads(text, label)
    if errors:
        return None, errors
    if not isinstance(document, dict):
        return None, [f"{label} must be a JSON object"]
    allowed_keys = {
        "schema",
        "command",
        "exit_code",
        "elapsed_seconds",
        "generator_log_path",
        "generator_log_size_bytes",
    }
    extra_keys = sorted(set(document) - allowed_keys)
    if extra_keys:
        return None, [
            f"{label} contains unexpected field {device_lab._display_path(extra_keys[0])}"
        ]
    missing_keys = sorted(allowed_keys - set(document))
    if missing_keys:
        return None, [f"{label} is missing {missing_keys[0]}"]
    if document["schema"] != STAGED_RUN_REPORT_SCHEMA:
        return None, [f"{label} schema must be {STAGED_RUN_REPORT_SCHEMA}"]
    command_errors = _validate_report_command(
        document["command"],
        label,
        expected_command,
        "ABI-7 compact key command",
    )
    if command_errors:
        return None, command_errors
    exit_code = document["exit_code"]
    if isinstance(exit_code, bool) or not isinstance(exit_code, int):
        return None, [f"{label} exit_code must be an integer"]
    if exit_code != expected_exit_code:
        return None, [
            f"{label} exit_code must match staged keygen exit marker "
            f"{expected_exit_code}, got {exit_code}"
        ]
    elapsed_seconds = document["elapsed_seconds"]
    if isinstance(elapsed_seconds, bool) or not isinstance(elapsed_seconds, (int, float)):
        return None, [f"{label} elapsed_seconds must be a finite positive number"]
    if not math.isfinite(float(elapsed_seconds)) or float(elapsed_seconds) <= 0:
        return None, [f"{label} elapsed_seconds must be a finite positive number"]
    elapsed_value = float(elapsed_seconds)
    if document["generator_log_path"] != readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME:
        return None, [
            f"{label} generator_log_path must be "
            f"{readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME}"
        ]
    size = document["generator_log_size_bytes"]
    if isinstance(size, bool) or not isinstance(size, int) or size < 0:
        return None, [f"{label} generator_log_size_bytes must be a non-negative integer"]
    try:
        actual_size = (
            staged_artifact_dir / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
        ).stat().st_size
    except OSError:
        return None, [f"{label} generator log size could not be checked"]
    if size != actual_size:
        return None, [
            f"{label} generator_log_size_bytes must match staged generator log "
            f"size {actual_size}, got {size}"
        ]
    return elapsed_value, []


def _validate_sha256_hex(value: object, label: str, field: str) -> list[str]:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(char not in "0123456789abcdef" for char in value)
        or value == "0" * 64
    ):
        return [f"{label} {field} must be a non-zero SHA-256 hex digest"]
    return []


def validate_staged_execution_report(
    *,
    staged_artifact_dir: Path,
    expected_exit_code: int,
    expected_command: str,
    expected_elapsed_seconds: float,
) -> list[str]:
    """Validate the staged execution report before publishing key evidence."""

    label = "staged recursive compact key execution report"
    path = staged_artifact_dir / EXECUTION_REPORT_FILENAME
    text, errors = _read_small_text_file(
        path,
        label,
        max_bytes=MAX_EXECUTION_REPORT_BYTES,
    )
    if errors:
        return errors
    assert text is not None
    document, errors = _strict_json_loads(text, label)
    if errors:
        return errors
    if not isinstance(document, dict):
        return [f"{label} must be a JSON object"]
    allowed_keys = {
        "schema",
        "phase",
        "command",
        "exit_code",
        "elapsed_seconds",
        "generator_log_path",
        "generator_log_sha256",
        "generator_log_size_bytes",
    }
    extra_keys = sorted(set(document) - allowed_keys)
    if extra_keys:
        return [
            f"{label} contains unexpected field {device_lab._display_path(extra_keys[0])}"
        ]
    missing_keys = sorted(allowed_keys - set(document))
    if missing_keys:
        return [f"{label} is missing {missing_keys[0]}"]
    if document["schema"] != EXECUTION_REPORT_SCHEMA:
        return [f"{label} schema must be {EXECUTION_REPORT_SCHEMA}"]
    if document["phase"] != "recursive compact keygen command":
        return [f"{label} phase must be recursive compact keygen command"]
    command_errors = _validate_report_command(
        document["command"],
        label,
        expected_command,
        "ABI-7 compact key command",
    )
    if command_errors:
        return command_errors
    exit_code = document["exit_code"]
    if isinstance(exit_code, bool) or not isinstance(exit_code, int):
        return [f"{label} exit_code must be an integer"]
    if exit_code != expected_exit_code:
        return [
            f"{label} exit_code must match staged keygen exit marker "
            f"{expected_exit_code}, got {exit_code}"
        ]
    elapsed_seconds = document["elapsed_seconds"]
    if (
        isinstance(elapsed_seconds, bool)
        or not isinstance(elapsed_seconds, (int, float))
        or not math.isfinite(float(elapsed_seconds))
        or float(elapsed_seconds) <= 0
    ):
        return [f"{label} elapsed_seconds must be a finite positive number"]
    if float(elapsed_seconds) != expected_elapsed_seconds:
        return [
            f"{label} elapsed_seconds must match staged run report "
            f"{expected_elapsed_seconds}, got {float(elapsed_seconds)}"
        ]
    if document["generator_log_path"] != readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME:
        return [
            f"{label} generator_log_path must be "
            f"{readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME}"
        ]
    log_digest = document["generator_log_sha256"]
    digest_errors = _validate_sha256_hex(log_digest, label, "generator_log_sha256")
    if digest_errors:
        return digest_errors
    log_size = document["generator_log_size_bytes"]
    if isinstance(log_size, bool) or not isinstance(log_size, int) or log_size <= 0:
        return [f"{label} generator_log_size_bytes must be a positive integer"]
    log_path = staged_artifact_dir / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
    actual_digest, actual_errors = compact_evidence._sha256_file(
        log_path,
        "staged recursive compact key execution report generator log",
    )
    if actual_errors:
        return actual_errors
    assert actual_digest is not None
    if log_digest != actual_digest:
        return [f"{label} generator_log_sha256 must match staged generator log SHA-256"]
    try:
        actual_size = log_path.stat().st_size
    except OSError:
        return [f"{label} generator log size could not be checked"]
    if log_size != actual_size:
        return [
            f"{label} generator_log_size_bytes must match staged generator log "
            f"size {actual_size}, got {log_size}"
        ]
    return []


def _copy_validated_file(
    source: Path,
    destination: Path,
    label: str,
) -> tuple[list[str], tuple[int, int] | None]:
    digest, size, _prefix, errors = compact_evidence._sha256_file_with_size(
        source,
        label,
    )
    if errors:
        return errors, None
    assert digest is not None and size is not None
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    parent_errors = _set_private_directory_permissions(
        destination.parent,
        f"{label} destination parent",
    )
    if parent_errors:
        return parent_errors, None
    expected_stat = source.lstat()
    expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    destination_identity: tuple[int, int] | None = None
    try:
        with source.open("rb") as src, destination.open("xb") as dst:
            os.fchmod(dst.fileno(), 0o600)
            destination_identity = _file_identity(os.fstat(dst.fileno()))
            open_stat = os.fstat(src.fileno())
            path_stat = source.lstat()
            if (open_stat.st_dev, open_stat.st_ino) != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return [f"{label} changed while being copied"], destination_identity
            shutil.copyfileobj(src, dst, length=1024 * 1024)
            dst.flush()
            os.fsync(dst.fileno())
            final_stat = source.lstat()
            if (final_stat.st_dev, final_stat.st_ino) != expected_identity:
                return [f"{label} changed while being copied"], destination_identity
    except FileExistsError:
        return [f"published {destination.name} already exists"], destination_identity
    except OSError:
        return [f"{label} could not be copied"], destination_identity
    try:
        copied_stat = destination.lstat()
    except OSError:
        return [f"published {destination.name} metadata could not be read"], destination_identity
    if stat.S_IMODE(copied_stat.st_mode) != 0o600:
        return [f"{label} copied file permissions must be 0600"], destination_identity
    copied_digest, copied_size, _copied_prefix, copied_errors = (
        compact_evidence._sha256_file_with_size(destination, f"published {destination.name}")
    )
    if copied_errors:
        return copied_errors, destination_identity
    if copied_digest != digest or copied_size != size:
        return [f"published {destination.name} does not match staged bytes"], destination_identity
    return [], destination_identity


def _copy_validated_file_to_dir(
    source: Path,
    parent_fd: int,
    destination_name: str,
    label: str,
) -> tuple[list[str], tuple[int, int] | None]:
    digest, size, _prefix, errors = compact_evidence._sha256_file_with_size(
        source,
        label,
    )
    if errors:
        return errors, None
    assert digest is not None and size is not None
    expected_stat = source.lstat()
    expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    destination_identity: tuple[int, int] | None = None
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        destination_fd = os.open(destination_name, flags, 0o600, dir_fd=parent_fd)
    except FileExistsError:
        return [f"published {destination_name} already exists"], destination_identity
    except OSError:
        return [f"{label} could not be copied"], destination_identity
    try:
        os.fchmod(destination_fd, 0o600)
        destination_identity = _file_identity(os.fstat(destination_fd))
        with source.open("rb") as src:
            open_stat = os.fstat(src.fileno())
            path_stat = source.lstat()
            if (open_stat.st_dev, open_stat.st_ino) != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return [f"{label} changed while being copied"], destination_identity
            while True:
                chunk = src.read(1024 * 1024)
                if not chunk:
                    break
                _write_all(destination_fd, chunk)
            os.fsync(destination_fd)
            final_stat = source.lstat()
            if (final_stat.st_dev, final_stat.st_ino) != expected_identity:
                return [f"{label} changed while being copied"], destination_identity
        copied_stat = os.fstat(destination_fd)
        if stat.S_IMODE(copied_stat.st_mode) != 0o600:
            return [f"{label} copied file permissions must be 0600"], destination_identity
    except OSError:
        return [f"{label} could not be copied"], destination_identity
    finally:
        os.close(destination_fd)
    copied_digest, copied_size, copied_errors = _sha256_file_with_size_at(
        parent_fd,
        destination_name,
        f"published {destination_name}",
    )
    if copied_errors:
        return copied_errors, destination_identity
    if copied_digest != digest or copied_size != size:
        return [f"published {destination_name} does not match staged bytes"], destination_identity
    return [], destination_identity


def _verify_published_file(source: Path, destination: Path, label: str) -> list[str]:
    source_digest, source_size, _source_prefix, source_errors = (
        compact_evidence._sha256_file_with_size(source, f"validated staged {destination.name}")
    )
    if source_errors:
        return source_errors
    published_digest, published_size, _published_prefix, published_errors = (
        compact_evidence._sha256_file_with_size(destination, label)
    )
    if published_errors:
        return published_errors
    if published_digest != source_digest or published_size != source_size:
        return [f"published {destination.name} does not match staged bytes"]
    return []


def _verify_published_file_at(
    source: Path,
    parent_fd: int,
    destination_name: str,
    label: str,
) -> list[str]:
    source_digest, source_size, _source_prefix, source_errors = (
        compact_evidence._sha256_file_with_size(source, f"validated staged {destination_name}")
    )
    if source_errors:
        return source_errors
    published_digest, published_size, published_errors = _sha256_file_with_size_at(
        parent_fd,
        destination_name,
        label,
    )
    if published_errors:
        return published_errors
    if published_digest != source_digest or published_size != source_size:
        return [f"published {destination_name} does not match staged bytes"]
    return []


def _required_publish_filenames() -> tuple[str, ...]:
    return (
        *readiness.COMPACT_KEY_REQUIRED_ARTIFACTS,
        readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME,
        readiness.COMPACT_KEY_EVIDENCE_FILENAME,
    )


def stage_compact_key_evidence(
    *,
    staged_artifact_dir: Path,
    stage_dir: Path,
    generated_at_utc: str,
    max_generated_at_future_skew_seconds: int,
    command: str,
) -> tuple[dict[str, Any] | None, list[str]]:
    """Copy staged keygen artifacts into ``stage_dir`` and build evidence."""

    errors: list[str] = []
    for name in (
        *readiness.COMPACT_KEY_REQUIRED_ARTIFACTS,
        readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME,
    ):
        copy_errors, _copy_identity = _copy_validated_file(
            staged_artifact_dir / name,
            stage_dir / name,
            f"staged recursive compact key artifact {name}",
        )
        errors.extend(copy_errors)
    if errors:
        return None, errors
    evidence, evidence_errors = compact_evidence.build_evidence(
        artifact_dir=stage_dir,
        command=command,
        generated_at_utc=generated_at_utc,
        max_generated_at_future_skew_seconds=max_generated_at_future_skew_seconds,
    )
    if evidence_errors:
        return None, evidence_errors
    assert evidence is not None
    validation_errors = compact_evidence.validate_evidence_document(evidence, stage_dir)
    if validation_errors:
        return None, validation_errors
    write_errors = compact_evidence.write_evidence(
        stage_dir / readiness.COMPACT_KEY_EVIDENCE_FILENAME,
        evidence,
    )
    if write_errors:
        return None, write_errors
    return evidence, []


def publish_stage(
    *,
    stage_dir: Path,
    artifact_dir: Path,
    replace: bool,
) -> list[str]:
    """Publish staged files to the final artifact directory."""

    errors: list[str] = []
    errors.extend(_ensure_private_directory(artifact_dir, "artifact directory"))
    if errors:
        return errors
    try:
        artifact_dir_stat = artifact_dir.lstat()
    except OSError:
        return ["artifact directory metadata could not be read"]
    artifact_dir_identity = _file_identity(artifact_dir_stat)
    errors.extend(
        _set_private_directory_permissions(
            artifact_dir,
            "artifact directory",
            expected_identity=artifact_dir_identity,
        )
    )
    if errors:
        return errors
    for name in _required_publish_filenames():
        errors.extend(
            _validate_output_file_path(
                artifact_dir / name,
                f"published {name}",
                replace=replace,
            )
        )
    if errors:
        return errors
    try:
        artifact_dir_fd = os.open(artifact_dir, _directory_open_flags())
    except OSError:
        return ["artifact directory could not be opened"]
    installed: list[tuple[str, tuple[int, int]]] = []
    try:
        opened_artifact_dir_stat = os.fstat(artifact_dir_fd)
        if (
            not stat.S_ISDIR(opened_artifact_dir_stat.st_mode)
            or _file_identity(opened_artifact_dir_stat) != artifact_dir_identity
        ):
            return ["artifact directory changed before publish"]
        for name in _required_publish_filenames():
            source = stage_dir / name
            tmp_name = f".{name}.staged-finalizer.tmp"
            tmp_identity: tuple[int, int] | None = None
            try:
                if _regular_file_identity_at(artifact_dir_fd, tmp_name) is not None:
                    return [f"temporary output for {name} already exists"]
                copy_errors, tmp_identity = _copy_validated_file_to_dir(
                    source,
                    artifact_dir_fd,
                    tmp_name,
                    f"validated staged {name}",
                )
                if tmp_identity is None:
                    tmp_identity = _regular_file_identity_at(artifact_dir_fd, tmp_name)
                if copy_errors:
                    cleanup_errors: list[str] = []
                    if tmp_identity is not None:
                        cleanup_errors.extend(
                            _unlink_file_if_identity_at(
                                artifact_dir_fd,
                                tmp_name,
                                tmp_identity,
                                label=f"temporary output for {name}",
                            )
                        )
                    cleanup_errors.extend(_cleanup_published_files_at(artifact_dir_fd, installed))
                    return [*copy_errors, *cleanup_errors]
                if replace:
                    os.replace(
                        tmp_name,
                        name,
                        src_dir_fd=artifact_dir_fd,
                        dst_dir_fd=artifact_dir_fd,
                    )
                else:
                    os.link(
                        tmp_name,
                        name,
                        src_dir_fd=artifact_dir_fd,
                        dst_dir_fd=artifact_dir_fd,
                        follow_symlinks=False,
                    )
                    os.unlink(tmp_name, dir_fd=artifact_dir_fd)
                destination_identity = _regular_file_identity_at(artifact_dir_fd, name)
                if destination_identity is None:
                    cleanup_errors = _cleanup_published_files_at(artifact_dir_fd, installed)
                    return [f"published {name} could not be installed", *cleanup_errors]
                verify_errors = _verify_published_file_at(
                    source,
                    artifact_dir_fd,
                    name,
                    f"published {name}",
                )
                if verify_errors:
                    cleanup_errors = _unlink_file_if_identity_at(
                        artifact_dir_fd,
                        name,
                        destination_identity,
                        label=f"published {name}",
                    )
                    cleanup_errors.extend(_cleanup_published_files_at(artifact_dir_fd, installed))
                    return [*verify_errors, *cleanup_errors]
                installed.append((name, destination_identity))
            except OSError:
                cleanup_errors = []
                if tmp_identity is None:
                    tmp_identity = _regular_file_identity_at(artifact_dir_fd, tmp_name)
                if tmp_identity is not None:
                    destination_identity = _regular_file_identity_at(artifact_dir_fd, name)
                    if destination_identity == tmp_identity:
                        cleanup_errors.extend(
                            _unlink_file_if_identity_at(
                                artifact_dir_fd,
                                name,
                                destination_identity,
                                label=f"published {name}",
                            )
                        )
                    cleanup_errors.extend(
                        _unlink_file_if_identity_at(
                            artifact_dir_fd,
                            tmp_name,
                            tmp_identity,
                            label=f"temporary output for {name}",
                        )
                    )
                cleanup_errors.extend(_cleanup_published_files_at(artifact_dir_fd, installed))
                return [f"published {name} could not be installed", *cleanup_errors]
        sync_errors = _sync_artifact_dir_fd(
            artifact_dir_fd,
            artifact_dir=artifact_dir,
            expected_identity=artifact_dir_identity,
        )
        if sync_errors:
            cleanup_errors = _cleanup_published_files_at(artifact_dir_fd, installed)
            return [*sync_errors, *cleanup_errors]
        return []
    finally:
        os.close(artifact_dir_fd)


def finalize_staged_run(args: argparse.Namespace) -> tuple[int, Path | None, list[str]]:
    """Finalize a completed staged ABI-7 compact keygen run."""

    errors: list[str] = []
    exit_path_errors = validate_exit_file_path_shape(args.exit_file)
    if exit_path_errors:
        return 1, None, exit_path_errors
    errors.extend(validate_directory_path(args.staged_artifact_dir, "--staged-artifact-dir", must_exist=True))
    errors.extend(validate_directory_path(args.artifact_dir, "--artifact-dir", must_exist=False))
    if not errors:
        temp_errors = validate_no_staged_runner_temp_outputs(
            args.staged_artifact_dir,
            "staged recursive compact key artifact directory",
        )
        if temp_errors:
            return 1, None, temp_errors
    exit_code_text, exit_errors = validate_exit_marker(args.exit_file)
    errors.extend(exit_errors)
    if exit_errors:
        return 1, None, errors
    assert exit_code_text is not None
    errors.extend(compact_evidence._validate_generated_at_utc(args.generated_at_utc))
    generated_at, timestamp_error = readiness.parse_utc_timestamp(
        args.generated_at_utc,
        "--generated-at-utc",
    )
    if timestamp_error is not None:
        errors.append(timestamp_error["message"])
    errors.extend(
        compact_evidence._validate_generated_at_future_skew(
            generated_at,
            args.max_generated_at_future_skew_seconds,
        )
    )
    errors.extend(readiness.validate_compact_key_command(args.command))
    if exit_code_text == "0":
        run_elapsed_seconds, report_errors = validate_staged_run_report(
            staged_artifact_dir=args.staged_artifact_dir,
            expected_exit_code=0,
            expected_command=args.command,
        )
        errors.extend(report_errors)
        if not report_errors:
            assert run_elapsed_seconds is not None
            errors.extend(
                validate_staged_execution_report(
                    staged_artifact_dir=args.staged_artifact_dir,
                    expected_exit_code=0,
                    expected_command=args.command,
                    expected_elapsed_seconds=run_elapsed_seconds,
                )
            )
    if errors:
        return 1, None, errors

    errors = _ensure_private_directory(args.artifact_dir, "--artifact-dir")
    if errors:
        return 1, None, errors
    errors = _set_private_directory_permissions(args.artifact_dir, "--artifact-dir")
    if errors:
        return 1, None, errors

    temp_parent = Path(tempfile.mkdtemp(prefix=".recursive-compact-finalize.", dir=args.artifact_dir))
    temp_parent_errors = _set_private_directory_permissions(
        temp_parent,
        "staged finalizer temporary directory",
    )
    if temp_parent_errors:
        return 1, None, temp_parent_errors
    try:
        temp_parent_identity = _file_identity(temp_parent.lstat())
    except OSError:
        return 1, None, ["staged finalizer temporary directory metadata could not be read"]
    stage_dir = temp_parent / "stage"
    finalizer_errors: list[str] = []
    try:
        stage_dir.mkdir(mode=0o700)
        finalizer_errors = _set_private_directory_permissions(
            stage_dir,
            "staged finalizer stage directory",
        )
        if not finalizer_errors:
            _evidence, stage_errors = stage_compact_key_evidence(
                staged_artifact_dir=args.staged_artifact_dir,
                stage_dir=stage_dir,
                generated_at_utc=args.generated_at_utc,
                max_generated_at_future_skew_seconds=(
                    args.max_generated_at_future_skew_seconds
                ),
                command=args.command,
            )
            if stage_errors:
                finalizer_errors = stage_errors
            else:
                publish_errors = publish_stage(
                    stage_dir=stage_dir,
                    artifact_dir=args.artifact_dir,
                    replace=args.replace,
                )
                if publish_errors:
                    finalizer_errors = publish_errors
    except OSError:
        finalizer_errors = ["staged finalizer temporary stage could not be created"]
    finally:
        cleanup_errors = _cleanup_temp_parent(
            temp_parent,
            expected_identity=temp_parent_identity,
        )
    if finalizer_errors or cleanup_errors:
        return 1, None, [*finalizer_errors, *cleanup_errors]

    final_evidence_path = args.artifact_dir / readiness.COMPACT_KEY_EVIDENCE_FILENAME
    blockers = readiness.check_compact_key_evidence(final_evidence_path)["blockers"]
    if blockers:
        return 1, None, [blocker["message"] for blocker in blockers]
    return 0, final_evidence_path, []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Finalize a completed staged ABI-7 recursive compact keygen run by "
            "validating its exit marker, copying artifacts, and writing canonical "
            "recursive-compact-key-evidence.json."
        )
    )
    parser.add_argument("--staged-artifact-dir", type=Path, default=DEFAULT_STAGED_ARTIFACT_DIR)
    parser.add_argument("--exit-file", type=Path, default=DEFAULT_EXIT_FILE)
    parser.add_argument("--artifact-dir", type=Path, default=DEFAULT_ARTIFACT_DIR)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--generated-at-utc", default=_default_generated_at_utc())
    parser.add_argument(
        "--max-generated-at-future-skew-seconds",
        type=int,
        default=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help=(
            "Maximum number of seconds generated_at_utc may be ahead of the "
            "finalizer clock."
        ),
    )
    parser.add_argument("--command", default=compact_evidence.DEFAULT_COMPACT_KEY_COMMAND)
    parser.add_argument(
        "--replace",
        action="store_true",
        help="Replace existing compact-key artifacts after staged validation succeeds.",
    )
    args = parser.parse_args(argv)
    if args.out is None:
        args.out = args.artifact_dir / readiness.COMPACT_KEY_EVIDENCE_FILENAME
    expected_out = args.artifact_dir / readiness.COMPACT_KEY_EVIDENCE_FILENAME
    if args.out != expected_out:
        parser.error("--out must equal <artifact-dir>/recursive-compact-key-evidence.json")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    status, evidence_path, errors = finalize_staged_run(args)
    if status != 0:
        for error in errors:
            print(f"[kagemusha-compact-finalizer] {error}", file=sys.stderr)
        return status
    assert evidence_path is not None
    print(f"[kagemusha-compact-finalizer] wrote {evidence_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
