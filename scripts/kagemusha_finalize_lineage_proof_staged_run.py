#!/usr/bin/env python3
"""Finalize a completed staged Reserved-lineage proof run."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import stat
import sys
import tempfile
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402
import kagemusha_lineage_proof_evidence as lineage_evidence  # noqa: E402
import kagemusha_production_readiness as readiness  # noqa: E402


DEFAULT_TEMP_ROOT = Path("/tmp").resolve()
DEFAULT_STAGED_ARTIFACT_DIR = (
    DEFAULT_TEMP_ROOT / "iroha-codex-lineage-proof-staged" / "artifacts" / "kagemusha"
)
DEFAULT_EXIT_FILE = DEFAULT_TEMP_ROOT / "iroha-codex-lineage-proof-staged.exit"
DEFAULT_ELAPSED_SECONDS_FILE = (
    DEFAULT_TEMP_ROOT / "iroha-codex-lineage-proof-staged.elapsed-seconds"
)
DEFAULT_ARTIFACT_DIR = Path("artifacts/kagemusha")
EXIT_MARKER_MAX_BYTES = 32
RUN_REPORT_FILENAME = "lineage-proof-staged-run.json"
STAGED_RUN_REPORT_SCHEMA = "iroha.kagemusha.lineage_proof_staged_run.v1"
EXECUTION_REPORT_SCHEMA = "iroha.kagemusha.lineage_staged_execution.v1"
LINEAGE_EXECUTION_REPORT_FILENAMES = {
    "init": "lineage-init-key-artifacts-execution.json",
    "append": "lineage-append-key-artifacts-execution.json",
    "proof": "lineage-proof-execution.json",
}
LINEAGE_KEY_ARTIFACT_LOG_FILENAMES = {
    "init": "lineage-init-key-artifacts.log",
    "append": "lineage-append-key-artifacts.log",
}
LINEAGE_KEY_ARTIFACT_COMMANDS = {
    "init": (
        "iroha app zk kagemusha lineage-key-artifacts "
        "--profile init "
        f"--opening-len {readiness.EXPECTED_LINEAGE_PROOF_OPENING_LEN} "
        "--out artifacts/kagemusha/lineage-init-len128.norito "
        "--record-out artifacts/kagemusha/lineage-init-len128.record.norito "
        "--vk-out artifacts/kagemusha/lineage-init-len128.vk "
        "--pk-out artifacts/kagemusha/lineage-init-len128.pk"
    ),
    "append": (
        "iroha app zk kagemusha lineage-key-artifacts "
        "--profile append "
        f"--opening-len {readiness.EXPECTED_LINEAGE_PROOF_OPENING_LEN} "
        "--out artifacts/kagemusha/lineage-append-len128.norito "
        "--record-out artifacts/kagemusha/lineage-append-len128.record.norito "
        "--vk-out artifacts/kagemusha/lineage-append-len128.vk "
        "--pk-out artifacts/kagemusha/lineage-append-len128.pk"
    ),
}
MAX_STAGED_RUN_REPORT_BYTES = 16 * 1024
MAX_EXECUTION_REPORT_BYTES = 16 * 1024
STAGED_RUNNER_TEMP_SUFFIX = ".staged-runner.tmp"
CONTROL_EXIT_MARKER_REDACTION = "<unsafe-exit-marker>"
SECRET_EXIT_MARKER_REDACTION = "<redacted-secret-marker>"
CANONICAL_ELAPSED_SECONDS_RE = re.compile(r"(?:0|[1-9][0-9]*)\.[0-9]{6}\n\Z")


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
    """Reject directory aliases before reading or publishing lineage evidence."""

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


def validate_elapsed_seconds_file_path_shape(path: Path | None) -> list[str]:
    """Reject unsafe elapsed-seconds path strings before directory metadata."""

    if path is None:
        return []
    secret_error = _secret_path_error(path, "--elapsed-seconds-file")
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
    max_bytes: int = EXIT_MARKER_MAX_BYTES,
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
            data = handle.read(max_bytes + 1)
            if len(data) > max_bytes:
                return None, [f"{label} must not exceed {max_bytes} bytes"]
            final_stat = path.lstat()
            if (final_stat.st_dev, final_stat.st_ino) != expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} could not be read"]
    try:
        return data.decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [f"{label} could not be read"]


def read_exit_marker(path: Path) -> tuple[str | None, list[str]]:
    """Read the staged lineage proof exit marker."""

    text, errors = _read_small_text_file(path, "staged lineage proof exit marker")
    if errors:
        return None, errors
    assert text is not None
    marker = text.strip()
    if text != "0\n" and marker == "0":
        return marker, [
            "staged lineage proof exit marker must be exactly 0 followed by newline"
        ]
    return marker, []


def validate_exit_marker(path: Path) -> tuple[str | None, list[str]]:
    """Return errors if the staged lineage proof did not complete successfully."""

    stripped, errors = read_exit_marker(path)
    if errors:
        return None, errors
    assert stripped is not None
    if stripped != "0":
        return stripped, [
            "staged lineage proof exit code must be 0, got "
            f"{_display_exit_marker(stripped)}"
        ]
    return stripped, []


def _parse_canonical_elapsed_seconds_file_text(
    text: str,
) -> tuple[float | None, list[str]]:
    if CANONICAL_ELAPSED_SECONDS_RE.fullmatch(text) is None:
        return None, [
            (
                "staged lineage proof elapsed-seconds file must be exactly a "
                "positive finite decimal with six fractional digits followed by newline"
            )
        ]
    value_text = text[:-1]
    try:
        value = float(value_text)
    except ValueError:
        return None, [
            "staged lineage proof elapsed-seconds file must contain a positive finite number"
        ]
    if not math.isfinite(value) or value <= 0:
        return None, [
            "staged lineage proof elapsed-seconds file must contain a positive finite number"
        ]
    if f"{value:.6f}\n" != text:
        return None, [
            (
                "staged lineage proof elapsed-seconds file must be exactly a "
                "positive finite decimal with six fractional digits followed by newline"
            )
        ]
    return value, []


def resolve_elapsed_seconds(args: argparse.Namespace) -> tuple[float | None, list[str]]:
    """Resolve elapsed seconds from the CLI value or staged-runner file."""

    errors: list[str] = []
    file_value: float | None = None
    if args.elapsed_seconds_file is not None:
        text, read_errors = _read_small_text_file(
            args.elapsed_seconds_file,
            "staged lineage proof elapsed-seconds file",
        )
        errors.extend(read_errors)
        if text is not None:
            file_value, parse_errors = _parse_canonical_elapsed_seconds_file_text(text)
            errors.extend(parse_errors)
    if args.elapsed_seconds is None and file_value is None:
        errors.append("--elapsed-seconds or --elapsed-seconds-file is required")
    if args.elapsed_seconds is not None and file_value is not None:
        if args.elapsed_seconds != file_value:
            errors.append("--elapsed-seconds must match --elapsed-seconds-file")
    elapsed_seconds = file_value if file_value is not None else args.elapsed_seconds
    if elapsed_seconds is not None and (
        not math.isfinite(elapsed_seconds) or elapsed_seconds <= 0
    ):
        errors.append("--elapsed-seconds must be a positive finite number")
    return elapsed_seconds, errors


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
    expected_elapsed_seconds: float,
) -> list[str]:
    """Validate the staged proof runner report before trusting a success marker."""

    label = "staged lineage proof run report"
    path = staged_artifact_dir / RUN_REPORT_FILENAME
    text, errors = _read_small_text_file(
        path,
        label,
        max_bytes=MAX_STAGED_RUN_REPORT_BYTES,
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
        "command",
        "exit_code",
        "elapsed_seconds",
        "lineage_key_artifact_logs",
        "proof_log_path",
        "proof_log_size_bytes",
    }
    extra_keys = sorted(set(document) - allowed_keys)
    if extra_keys:
        return [
            f"{label} contains unexpected field {device_lab._display_path(extra_keys[0])}"
        ]
    missing_keys = sorted(allowed_keys - set(document))
    if missing_keys:
        return [f"{label} is missing {missing_keys[0]}"]
    if document["schema"] != STAGED_RUN_REPORT_SCHEMA:
        return [f"{label} schema must be {STAGED_RUN_REPORT_SCHEMA}"]
    command_errors = _validate_report_command(
        document["command"],
        label,
        expected_command,
        "Reserved-lineage proof command",
    )
    if command_errors:
        return command_errors
    exit_code = document["exit_code"]
    if isinstance(exit_code, bool) or not isinstance(exit_code, int):
        return [f"{label} exit_code must be an integer"]
    if exit_code != expected_exit_code:
        return [
            f"{label} exit_code must match staged lineage proof exit marker "
            f"{expected_exit_code}, got {exit_code}"
        ]
    elapsed_seconds = document["elapsed_seconds"]
    if isinstance(elapsed_seconds, bool) or not isinstance(elapsed_seconds, (int, float)):
        return [f"{label} elapsed_seconds must be a finite positive number"]
    if not math.isfinite(float(elapsed_seconds)) or float(elapsed_seconds) <= 0:
        return [f"{label} elapsed_seconds must be a finite positive number"]
    if float(elapsed_seconds) != expected_elapsed_seconds:
        return [
            f"{label} elapsed_seconds must match staged elapsed seconds "
            f"{expected_elapsed_seconds}, got {float(elapsed_seconds)}"
        ]
    proof_log_name = readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
    if document["proof_log_path"] != proof_log_name:
        return [f"{label} proof_log_path must be {proof_log_name}"]
    size = document["proof_log_size_bytes"]
    if isinstance(size, bool) or not isinstance(size, int) or size < 0:
        return [f"{label} proof_log_size_bytes must be a non-negative integer"]
    try:
        actual_size = (staged_artifact_dir / proof_log_name).stat().st_size
    except OSError:
        return [f"{label} proof log size could not be checked"]
    if size != actual_size:
        return [
            f"{label} proof_log_size_bytes must match staged proof log "
            f"size {actual_size}, got {size}"
        ]
    key_logs = document["lineage_key_artifact_logs"]
    expected_key_logs = {
        "init": "lineage-init-key-artifacts.log",
        "append": "lineage-append-key-artifacts.log",
    }
    if not isinstance(key_logs, dict):
        return [f"{label} lineage_key_artifact_logs must be a JSON object"]
    unexpected_profiles = sorted(set(key_logs) - set(expected_key_logs))
    if unexpected_profiles:
        return [
            f"{label} lineage_key_artifact_logs contains unexpected profile "
            f"{device_lab._display_path(unexpected_profiles[0])}"
        ]
    missing_profiles = sorted(set(expected_key_logs) - set(key_logs))
    if missing_profiles:
        return [
            f"{label} lineage_key_artifact_logs is missing profile "
            f"{missing_profiles[0]}"
        ]
    for profile, log_name in expected_key_logs.items():
        entry = key_logs.get(profile)
        if not isinstance(entry, dict):
            return [f"{label} {profile} lineage key artifact log must be a JSON object"]
        allowed_entry_keys = {"path", "size_bytes"}
        entry_extra = sorted(set(entry) - allowed_entry_keys)
        if entry_extra:
            return [
                f"{label} {profile} lineage key artifact log contains unexpected "
                f"field {device_lab._display_path(entry_extra[0])}"
            ]
        entry_missing = sorted(allowed_entry_keys - set(entry))
        if entry_missing:
            return [
                f"{label} {profile} lineage key artifact log is missing "
                f"{entry_missing[0]}"
            ]
        if entry["path"] != log_name:
            return [f"{label} {profile} lineage key artifact log path must be {log_name}"]
        entry_size = entry["size_bytes"]
        if (
            isinstance(entry_size, bool)
            or not isinstance(entry_size, int)
            or entry_size < 0
        ):
            return [
                f"{label} {profile} lineage key artifact log size_bytes must be "
                "a non-negative integer"
            ]
        try:
            actual_entry_size = (staged_artifact_dir / log_name).stat().st_size
        except OSError:
            return [f"{label} {profile} lineage key artifact log size could not be checked"]
        if entry_size != actual_entry_size:
            return [
                f"{label} {profile} lineage key artifact log size_bytes must match "
                f"staged log size {actual_entry_size}, got {entry_size}"
            ]
    return []


def _validate_sha256_hex(value: object, label: str, field: str) -> list[str]:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(char not in "0123456789abcdef" for char in value)
        or value == "0" * 64
    ):
        return [f"{label} {field} must be a non-zero SHA-256 hex digest"]
    return []


def _validate_staged_execution_report(
    *,
    staged_artifact_dir: Path,
    profile: str,
    expected_command: str,
    expected_log_name: str,
    expected_phase: str,
    expected_elapsed_seconds: float | None = None,
) -> list[str]:
    label = (
        "staged lineage proof execution report"
        if profile == "proof"
        else f"staged {profile} lineage key artifact execution report"
    )
    report_path = staged_artifact_dir / LINEAGE_EXECUTION_REPORT_FILENAMES[profile]
    text, errors = _read_small_text_file(
        report_path,
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
        "log_path",
        "log_sha256",
        "log_size_bytes",
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
    if document["phase"] != expected_phase:
        return [f"{label} phase must be {expected_phase}"]
    command_errors = _validate_report_command(
        document["command"],
        label,
        expected_command,
        "Reserved-lineage proof command"
        if profile == "proof"
        else f"{profile} lineage key artifact command",
    )
    if command_errors:
        return command_errors
    exit_code = document["exit_code"]
    if isinstance(exit_code, bool) or not isinstance(exit_code, int):
        return [f"{label} exit_code must be an integer"]
    if exit_code != 0:
        return [f"{label} exit_code must be 0, got {exit_code}"]
    elapsed_seconds = document["elapsed_seconds"]
    if (
        isinstance(elapsed_seconds, bool)
        or not isinstance(elapsed_seconds, (int, float))
        or not math.isfinite(float(elapsed_seconds))
        or float(elapsed_seconds) <= 0
    ):
        return [f"{label} elapsed_seconds must be a finite positive number"]
    if (
        expected_elapsed_seconds is not None
        and float(elapsed_seconds) != expected_elapsed_seconds
    ):
        return [
            f"{label} elapsed_seconds must match staged elapsed seconds "
            f"{expected_elapsed_seconds}, got {float(elapsed_seconds)}"
        ]
    if document["log_path"] != expected_log_name:
        return [f"{label} log_path must be {expected_log_name}"]
    log_digest = document["log_sha256"]
    digest_errors = _validate_sha256_hex(log_digest, label, "log_sha256")
    if digest_errors:
        return digest_errors
    log_size = document["log_size_bytes"]
    if isinstance(log_size, bool) or not isinstance(log_size, int) or log_size <= 0:
        return [f"{label} log_size_bytes must be a positive integer"]
    log_path = staged_artifact_dir / expected_log_name
    actual_digest, actual_errors = lineage_evidence._sha256_file(
        log_path,
        f"{label} log",
    )
    if actual_errors:
        return actual_errors
    assert actual_digest is not None
    if log_digest != actual_digest:
        return [f"{label} log_sha256 must match staged log SHA-256"]
    try:
        actual_size = log_path.stat().st_size
    except OSError:
        return [f"{label} log size could not be checked"]
    if log_size != actual_size:
        return [
            f"{label} log_size_bytes must match staged log size "
            f"{actual_size}, got {log_size}"
        ]
    return []


def validate_staged_execution_reports(
    *,
    staged_artifact_dir: Path,
    expected_command: str,
    expected_elapsed_seconds: float,
) -> list[str]:
    """Validate staged phase execution reports before publishing proof evidence."""

    errors: list[str] = []
    for profile, log_name in LINEAGE_KEY_ARTIFACT_LOG_FILENAMES.items():
        errors.extend(
            _validate_staged_execution_report(
                staged_artifact_dir=staged_artifact_dir,
                profile=profile,
                expected_command=LINEAGE_KEY_ARTIFACT_COMMANDS[profile],
                expected_log_name=log_name,
                expected_phase=f"{profile} lineage key artifact command",
            )
        )
        if errors:
            return errors
    proof_log_name = readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
    return _validate_staged_execution_report(
        staged_artifact_dir=staged_artifact_dir,
        profile="proof",
        expected_command=expected_command,
        expected_log_name=proof_log_name,
        expected_phase="lineage proof command",
        expected_elapsed_seconds=expected_elapsed_seconds,
    )


def _copy_validated_file(
    source: Path,
    destination: Path,
    label: str,
) -> tuple[list[str], tuple[int, int] | None]:
    digest, size, _prefix, errors = lineage_evidence._sha256_file_with_size(source, label)
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
        lineage_evidence._sha256_file_with_size(destination, f"published {destination.name}")
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
    digest, size, _prefix, errors = lineage_evidence._sha256_file_with_size(source, label)
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
        lineage_evidence._sha256_file_with_size(source, f"validated staged {destination.name}")
    )
    if source_errors:
        return source_errors
    published_digest, published_size, _published_prefix, published_errors = (
        lineage_evidence._sha256_file_with_size(destination, label)
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
        lineage_evidence._sha256_file_with_size(source, f"validated staged {destination_name}")
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
        *readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS,
        readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"],
        readiness.LINEAGE_PROOF_EVIDENCE_FILENAME,
    )


def stage_lineage_proof_evidence(
    *,
    staged_artifact_dir: Path,
    stage_dir: Path,
    generated_at_utc: str,
    max_generated_at_future_skew_seconds: int,
    command: str,
    elapsed_seconds: float,
) -> tuple[dict[str, Any] | None, list[str]]:
    """Copy staged lineage proof artifacts into ``stage_dir`` and build evidence."""

    errors: list[str] = []
    for name in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS:
        copy_errors, _copy_identity = _copy_validated_file(
            staged_artifact_dir / name,
            stage_dir / name,
            f"staged lineage proof artifact {name}",
        )
        errors.extend(copy_errors)
    proof_log_name = readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
    copy_errors, _copy_identity = _copy_validated_file(
        staged_artifact_dir / proof_log_name,
        stage_dir / proof_log_name,
        f"staged lineage proof log {proof_log_name}",
    )
    errors.extend(copy_errors)
    if errors:
        return None, errors
    evidence, evidence_errors = lineage_evidence.build_evidence(
        artifact_dir=stage_dir,
        proof_log=stage_dir / proof_log_name,
        command=command,
        elapsed_seconds=elapsed_seconds,
        generated_at_utc=generated_at_utc,
        max_generated_at_future_skew_seconds=max_generated_at_future_skew_seconds,
    )
    if evidence_errors:
        return None, evidence_errors
    assert evidence is not None
    validation_errors = lineage_evidence.validate_evidence_document(evidence, stage_dir)
    if validation_errors:
        return None, validation_errors
    write_errors = lineage_evidence.write_evidence(
        stage_dir / readiness.LINEAGE_PROOF_EVIDENCE_FILENAME,
        evidence,
    )
    if write_errors:
        return None, write_errors
    return evidence, []


def publish_stage(*, stage_dir: Path, artifact_dir: Path, replace: bool) -> list[str]:
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
    """Finalize a completed staged Reserved-lineage proof run."""

    errors: list[str] = []
    exit_path_errors = validate_exit_file_path_shape(args.exit_file)
    if exit_path_errors:
        return 1, None, exit_path_errors
    elapsed_path_errors = validate_elapsed_seconds_file_path_shape(
        args.elapsed_seconds_file
    )
    if elapsed_path_errors:
        return 1, None, elapsed_path_errors
    errors.extend(validate_directory_path(args.staged_artifact_dir, "--staged-artifact-dir", must_exist=True))
    errors.extend(validate_directory_path(args.artifact_dir, "--artifact-dir", must_exist=False))
    if not errors:
        temp_errors = validate_no_staged_runner_temp_outputs(
            args.staged_artifact_dir,
            "staged lineage proof artifact directory",
        )
        if temp_errors:
            return 1, None, temp_errors
    exit_code_text, exit_errors = validate_exit_marker(args.exit_file)
    errors.extend(exit_errors)
    if exit_errors:
        return 1, None, errors
    assert exit_code_text is not None
    errors.extend(lineage_evidence._validate_generated_at_utc(args.generated_at_utc))
    generated_at, timestamp_error = readiness.parse_utc_timestamp(
        args.generated_at_utc,
        "--generated-at-utc",
    )
    if timestamp_error is not None:
        errors.append(timestamp_error["message"])
    errors.extend(
        lineage_evidence._validate_generated_at_future_skew(
            generated_at,
            args.max_generated_at_future_skew_seconds,
        )
    )
    errors.extend(readiness.validate_lineage_proof_command(
        args.command,
        readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"],
    ))
    elapsed_seconds, elapsed_errors = resolve_elapsed_seconds(args)
    errors.extend(elapsed_errors)
    if exit_code_text == "0" and not elapsed_errors:
        assert elapsed_seconds is not None
        report_errors = validate_staged_run_report(
            staged_artifact_dir=args.staged_artifact_dir,
            expected_exit_code=0,
            expected_command=args.command,
            expected_elapsed_seconds=elapsed_seconds,
        )
        errors.extend(report_errors)
        if not report_errors:
            errors.extend(
                validate_staged_execution_reports(
                    staged_artifact_dir=args.staged_artifact_dir,
                    expected_command=args.command,
                    expected_elapsed_seconds=elapsed_seconds,
                )
            )
    if errors:
        return 1, None, errors
    assert elapsed_seconds is not None

    errors = _ensure_private_directory(args.artifact_dir, "--artifact-dir")
    if errors:
        return 1, None, errors
    errors = _set_private_directory_permissions(args.artifact_dir, "--artifact-dir")
    if errors:
        return 1, None, errors

    temp_parent = Path(tempfile.mkdtemp(prefix=".lineage-proof-finalize.", dir=args.artifact_dir))
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
            _evidence, stage_errors = stage_lineage_proof_evidence(
                staged_artifact_dir=args.staged_artifact_dir,
                stage_dir=stage_dir,
                generated_at_utc=args.generated_at_utc,
                max_generated_at_future_skew_seconds=(
                    args.max_generated_at_future_skew_seconds
                ),
                command=args.command,
                elapsed_seconds=elapsed_seconds,
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

    final_evidence_path = args.artifact_dir / readiness.LINEAGE_PROOF_EVIDENCE_FILENAME
    blockers = readiness.check_lineage_proof_evidence(final_evidence_path)["blockers"]
    if blockers:
        return 1, None, [blocker["message"] for blocker in blockers]
    return 0, final_evidence_path, []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Finalize a completed staged Kagemusha Reserved-lineage proof run by "
            "validating its exit marker, copying artifacts, and writing canonical "
            "lineage-proof-evidence.json."
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
    parser.add_argument("--command", default=lineage_evidence.DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND)
    parser.add_argument("--elapsed-seconds", type=float)
    parser.add_argument("--elapsed-seconds-file", type=Path)
    parser.add_argument(
        "--replace",
        action="store_true",
        help="Replace existing lineage proof artifacts after staged validation succeeds.",
    )
    args = parser.parse_args(argv)
    if args.elapsed_seconds is None and args.elapsed_seconds_file is None:
        args.elapsed_seconds_file = DEFAULT_ELAPSED_SECONDS_FILE
    if args.out is None:
        args.out = args.artifact_dir / readiness.LINEAGE_PROOF_EVIDENCE_FILENAME
    expected_out = args.artifact_dir / readiness.LINEAGE_PROOF_EVIDENCE_FILENAME
    if args.out != expected_out:
        parser.error("--out must equal <artifact-dir>/lineage-proof-evidence.json")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    status, evidence_path, errors = finalize_staged_run(args)
    if status != 0:
        for error in errors:
            print(f"[kagemusha-lineage-finalizer] {error}", file=sys.stderr)
        return status
    assert evidence_path is not None
    print(f"[kagemusha-lineage-finalizer] wrote {evidence_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
