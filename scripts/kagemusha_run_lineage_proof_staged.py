#!/usr/bin/env python3
"""Run the Kagemusha Reserved-lineage production proof into a staging directory."""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import shlex
import stat
import subprocess
import sys
import time
from typing import Callable

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402
import kagemusha_lineage_proof_evidence as lineage_evidence  # noqa: E402
import kagemusha_production_readiness as readiness  # noqa: E402


DEFAULT_TEMP_ROOT = Path("/tmp").resolve()
DEFAULT_STAGED_ROOT = DEFAULT_TEMP_ROOT / "iroha-codex-lineage-proof-staged"
DEFAULT_STAGED_ARTIFACT_DIR = DEFAULT_STAGED_ROOT / "artifacts" / "kagemusha"
DEFAULT_EXIT_FILE = DEFAULT_TEMP_ROOT / "iroha-codex-lineage-proof-staged.exit"
DEFAULT_ELAPSED_SECONDS_FILE = (
    DEFAULT_TEMP_ROOT / "iroha-codex-lineage-proof-staged.elapsed-seconds"
)
PROOF_LOG_FILENAME = readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS["record_archive_proof"]
LINEAGE_KEY_ARTIFACT_LOG_FILENAMES = {
    "init": "lineage-init-key-artifacts.log",
    "append": "lineage-append-key-artifacts.log",
}
RUN_REPORT_FILENAME = "lineage-proof-staged-run.json"
STAGED_RUN_REPORT_SCHEMA = "iroha.kagemusha.lineage_proof_staged_run.v1"
EXECUTION_REPORT_SCHEMA = "iroha.kagemusha.lineage_staged_execution.v1"
LINEAGE_EXECUTION_REPORT_FILENAMES = {
    "init": "lineage-init-key-artifacts-execution.json",
    "append": "lineage-append-key-artifacts-execution.json",
    "proof": "lineage-proof-execution.json",
}
MAX_EXECUTION_REPORT_BYTES = 16 * 1024
DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND = (
    lineage_evidence.DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND
)
CommandRunner = Callable[[list[str], Path, Path], int]

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
LINEAGE_KEY_ARTIFACTS_BY_PROFILE = {
    "init": readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS[:4],
    "append": readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS[4:],
}


def _secret_path_error(path: Path, label: str) -> str | None:
    if device_lab.SECRET_RE.search(str(path)):
        return f"{label} must not contain secret-looking material"
    if device_lab._contains_control_character(str(path)):
        return f"{label} must not contain control characters"
    return None


def _wrapper_exit_status(command_status: int) -> int:
    """Return a conventional process status for the staging wrapper itself."""

    return 0 if command_status == 0 else 1


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
    """Reject staged directory aliases before command output is written."""

    secret_error = _secret_path_error(path, label)
    if secret_error is not None:
        return [secret_error]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        if must_exist:
            return [f"{label} is missing"]
        return []
    except OSError:
        return [f"{label} metadata could not be read"]
    if stat.S_ISLNK(mode):
        return [f"{label} must not be a symlink"]
    if not stat.S_ISDIR(mode):
        return [f"{label} must be a directory"]
    return []


def validate_output_file_path(path: Path, label: str, *, replace: bool) -> list[str]:
    """Reject output aliases and accidental overwrites before running the proof."""

    secret_error = _secret_path_error(path, label)
    if secret_error is not None:
        return [secret_error]
    parent_errors = validate_directory_path(path.parent, f"{label} parent", must_exist=False)
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


def _sync_output_parent(
    parent: Path,
    label: str,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    try:
        parent_fd = os.open(parent, _directory_open_flags())
    except OSError:
        return [f"{label} parent directory could not be synced"]
    try:
        parent_stat = os.fstat(parent_fd)
        if not stat.S_ISDIR(parent_stat.st_mode):
            return [f"{label} parent directory could not be synced"]
        if expected_identity is not None and _file_identity(parent_stat) != expected_identity:
            return [f"{label} parent directory changed before sync"]
        os.fsync(parent_fd)
    except OSError:
        return [f"{label} parent directory could not be synced"]
    finally:
        os.close(parent_fd)
    return []


def _regular_file_identity_for_unlink(
    path: Path,
    label: str,
) -> tuple[tuple[int, int] | None, list[str]]:
    try:
        path_stat = path.lstat()
    except FileNotFoundError:
        return None, []
    except OSError:
        return None, [f"{label} metadata could not be read"]
    if stat.S_ISLNK(path_stat.st_mode):
        return None, [f"{label} must not be a symlink"]
    if not stat.S_ISREG(path_stat.st_mode):
        return None, [f"{label} must be a regular file"]
    if path_stat.st_nlink > 1:
        return None, [f"{label} must not be hardlinked"]
    return _file_identity(path_stat), []


def _unlink_file_if_identity(
    path: Path,
    expected_identity: tuple[int, int],
    *,
    changed_message: str,
    failure_message: str,
) -> list[str]:
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return [failure_message]
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
            return [failure_message]
        if (
            not stat.S_ISREG(path_stat.st_mode)
            or path_stat.st_nlink > 1
            or _file_identity(path_stat) != expected_identity
        ):
            return [changed_message]
        try:
            os.unlink(path.name, dir_fd=parent_fd)
        except OSError:
            return [failure_message]
    finally:
        os.close(parent_fd)
    return []


def _unlink_output_for_replace(path: Path, label: str) -> list[str]:
    errors = validate_output_file_path(path, label, replace=True)
    if errors:
        return errors
    expected_identity, identity_errors = _regular_file_identity_for_unlink(path, label)
    if identity_errors or expected_identity is None:
        return identity_errors
    return _unlink_file_if_identity(
        path,
        expected_identity,
        changed_message=f"{label} changed before cleanup",
        failure_message=f"{label} could not be replaced",
    )


def _cleanup_temp_output(
    path: Path,
    label: str,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    if expected_identity is None:
        return []
    return _unlink_file_if_identity(
        path,
        expected_identity,
        changed_message=f"{label} temporary output changed before cleanup",
        failure_message=f"{label} temporary output could not be removed",
    )


def _staged_root_from_artifact_dir(path: Path) -> tuple[Path | None, list[str]]:
    if path.name != "kagemusha" or path.parent.name != "artifacts":
        return None, ["--staged-artifact-dir must end with artifacts/kagemusha"]
    return path.parent.parent, []


def _repo_root_errors(repo_root: Path) -> list[str]:
    return [
        str(item["message"])
        for item in readiness.validate_repo_root_path(repo_root)
    ]


def _preflight_paths(args: argparse.Namespace) -> tuple[Path | None, list[str]]:
    errors: list[str] = []
    if args.replace and args.resume_key_artifacts:
        errors.append("--replace and --resume-key-artifacts cannot be combined")
    key_phase_replace = args.replace or args.resume_key_artifacts
    run_level_replace = args.replace or args.resume_key_artifacts
    staged_root, root_errors = _staged_root_from_artifact_dir(args.staged_artifact_dir)
    errors.extend(root_errors)
    if staged_root is not None:
        errors.extend(validate_directory_path(staged_root, "--staged-root", must_exist=False))
    errors.extend(_repo_root_errors(args.repo_root))
    errors.extend(validate_directory_path(args.staged_artifact_dir, "--staged-artifact-dir", must_exist=False))
    for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS:
        errors.extend(
            validate_output_file_path(
                args.staged_artifact_dir / artifact,
                f"staged lineage proof artifact {artifact}",
                replace=key_phase_replace,
            )
        )
    for profile, log_name in LINEAGE_KEY_ARTIFACT_LOG_FILENAMES.items():
        errors.extend(
            validate_output_file_path(
                args.staged_artifact_dir / log_name,
                f"staged {profile} lineage key artifact log",
                replace=key_phase_replace,
            )
        )
        errors.extend(
            validate_output_file_path(
                args.staged_artifact_dir / LINEAGE_EXECUTION_REPORT_FILENAMES[profile],
                f"staged {profile} lineage key artifact execution report",
                replace=key_phase_replace,
            )
        )
    proof_log = args.staged_artifact_dir / PROOF_LOG_FILENAME
    errors.extend(validate_output_file_path(proof_log, "staged proof log", replace=run_level_replace))
    errors.extend(
        validate_output_file_path(
            args.staged_artifact_dir / LINEAGE_EXECUTION_REPORT_FILENAMES["proof"],
            "staged lineage proof execution report",
            replace=run_level_replace,
        )
    )
    errors.extend(validate_output_file_path(args.exit_file, "staged lineage proof exit marker", replace=run_level_replace))
    errors.extend(
        validate_output_file_path(
            args.staged_artifact_dir / RUN_REPORT_FILENAME,
            "staged lineage proof run report",
            replace=run_level_replace,
        )
    )
    errors.extend(
        validate_output_file_path(
            args.elapsed_seconds_file,
            "staged lineage proof elapsed-seconds file",
            replace=run_level_replace,
        )
    )
    return staged_root, errors


def _profile_output_names(profile: str) -> tuple[str, ...]:
    return (
        *LINEAGE_KEY_ARTIFACTS_BY_PROFILE[profile],
        LINEAGE_KEY_ARTIFACT_LOG_FILENAMES[profile],
        LINEAGE_EXECUTION_REPORT_FILENAMES[profile],
    )


def _run_level_output_paths(args: argparse.Namespace) -> tuple[tuple[Path, str], ...]:
    return (
        (args.staged_artifact_dir / PROOF_LOG_FILENAME, "staged proof log"),
        (
            args.staged_artifact_dir / LINEAGE_EXECUTION_REPORT_FILENAMES["proof"],
            "staged lineage proof execution report",
        ),
        (
            args.staged_artifact_dir / RUN_REPORT_FILENAME,
            "staged lineage proof run report",
        ),
        (args.elapsed_seconds_file, "staged lineage proof elapsed-seconds file"),
        (args.exit_file, "staged lineage proof exit marker"),
    )


def _unlink_replace_outputs(staged_artifact_dir: Path) -> list[str]:
    errors: list[str] = []
    for name in (
        *readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS,
        *LINEAGE_KEY_ARTIFACT_LOG_FILENAMES.values(),
        *LINEAGE_EXECUTION_REPORT_FILENAMES.values(),
        PROOF_LOG_FILENAME,
        RUN_REPORT_FILENAME,
    ):
        path = staged_artifact_dir / name
        errors.extend(
            _unlink_output_for_replace(path, f"staged lineage proof output {name}")
        )
    return errors


def _validate_output_paths_can_be_replaced(
    entries: tuple[tuple[Path, str], ...],
) -> list[str]:
    errors: list[str] = []
    for path, label in entries:
        errors.extend(validate_output_file_path(path, label, replace=True))
    return errors


def _unlink_resume_outputs(entries: tuple[tuple[Path, str], ...]) -> list[str]:
    errors = _validate_output_paths_can_be_replaced(entries)
    if errors:
        return errors
    for path, label in entries:
        errors.extend(_unlink_output_for_replace(path, label))
    return errors


def _cleanup_profile_for_resume(profile: str, staged_artifact_dir: Path) -> list[str]:
    return _unlink_resume_outputs(
        tuple(
            (
                staged_artifact_dir / name,
                f"staged {profile} lineage key artifact output {name}",
            )
            for name in _profile_output_names(profile)
        )
    )


def _write_text_atomic(path: Path, text: str, label: str, *, replace: bool) -> list[str]:
    expected_bytes = text.encode("utf-8")
    errors = validate_output_file_path(path, label, replace=replace)
    if errors:
        return errors
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
    except OSError:
        return [f"{label} parent directory could not be created"]
    errors = validate_directory_path(path.parent, f"{label} parent", must_exist=True)
    if errors:
        return errors
    try:
        parent_stat = path.parent.lstat()
    except OSError:
        return [f"{label} parent metadata could not be read"]
    if stat.S_ISLNK(parent_stat.st_mode) or not stat.S_ISDIR(parent_stat.st_mode):
        return [f"{label} parent directory could not be synced"]
    parent_identity = _file_identity(parent_stat)
    tmp_path = path.parent / f".{path.name}.staged-runner.tmp"
    tmp_identity: tuple[int, int] | None = None
    try:
        if tmp_path.exists() or tmp_path.is_symlink():
            return [f"{label} temporary output already exists"]
        with tmp_path.open("xb") as handle:
            tmp_identity = _file_identity(os.fstat(handle.fileno()))
            handle.write(expected_bytes)
            handle.flush()
            os.fsync(handle.fileno())
        if replace:
            os.replace(tmp_path, path)
        else:
            tmp_path.rename(path)
        sync_errors = _sync_output_parent(
            path.parent,
            label,
            expected_identity=parent_identity,
        )
        if sync_errors:
            return sync_errors
    except OSError:
        cleanup_errors = _cleanup_temp_output(tmp_path, label, tmp_identity)
        return [f"{label} could not be written", *cleanup_errors]
    return _verify_written_text_file(path, expected_bytes, label)


def _verify_written_text_file(path: Path, expected_bytes: bytes, label: str) -> list[str]:
    errors = validate_output_file_path(path, label, replace=True)
    if errors:
        return errors
    try:
        expected_stat = path.lstat()
    except OSError:
        return [f"{label} metadata could not be read"]
    expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if (open_stat.st_dev, open_stat.st_ino) != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return [f"{label} changed after write"]
            data = handle.read(len(expected_bytes) + 1)
            if data != expected_bytes:
                return [f"{label} changed after write"]
            final_stat = path.lstat()
            if (final_stat.st_dev, final_stat.st_ino) != expected_identity:
                return [f"{label} changed after write"]
    except OSError:
        return [f"{label} could not be verified after write"]
    return []


def _read_existing_text_file(
    path: Path,
    label: str,
    *,
    max_bytes: int,
) -> tuple[str | None, list[str]]:
    expected_stat, errors = readiness._validate_lineage_local_file_for_read(
        path,
        label,
    )
    if errors:
        return None, errors
    assert expected_stat is not None
    if expected_stat.st_size > max_bytes:
        return None, [f"{label} must not exceed {max_bytes} bytes"]
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
    except device_lab.NonFiniteJsonConstantError as exc:
        return None, [f"{label} is not strict JSON: non-finite constant {exc.constant} is not allowed"]
    except json.JSONDecodeError:
        return None, [f"{label} is not valid JSON"]


def _run_command_to_log(command: list[str], cwd: Path, log_path: Path) -> int:
    """Run the canonical proof command with child output owned by ``log_path``."""

    with log_path.open("xb") as log_handle:
        process = subprocess.Popen(
            command,
            cwd=cwd,
            stdout=log_handle,
            stderr=subprocess.STDOUT,
        )
        exit_code = process.wait()
        log_handle.flush()
        os.fsync(log_handle.fileno())
        return exit_code


def _install_log_temp(temp_log: Path, final_log: Path, label: str, *, replace: bool) -> list[str]:
    temp_identity, temp_identity_errors = _regular_file_identity_for_unlink(
        temp_log,
        f"{label} temporary output",
    )
    if temp_identity_errors:
        return temp_identity_errors
    errors = validate_output_file_path(final_log, label, replace=replace)
    if errors:
        return [
            *errors,
            *_cleanup_temp_output(temp_log, label, temp_identity),
        ]
    try:
        log_parent_stat = final_log.parent.lstat()
    except OSError:
        return [
            f"{label} parent metadata could not be read",
            *_cleanup_temp_output(temp_log, label, temp_identity),
        ]
    if stat.S_ISLNK(log_parent_stat.st_mode) or not stat.S_ISDIR(log_parent_stat.st_mode):
        return [
            f"{label} parent directory could not be synced",
            *_cleanup_temp_output(temp_log, label, temp_identity),
        ]
    log_parent_identity = _file_identity(log_parent_stat)
    try:
        if replace:
            os.replace(temp_log, final_log)
        else:
            temp_log.rename(final_log)
        sync_errors = _sync_output_parent(
            final_log.parent,
            label,
            expected_identity=log_parent_identity,
        )
        if sync_errors:
            return sync_errors
    except OSError:
        return [
            f"{label} could not be installed",
            *_cleanup_temp_output(temp_log, label, temp_identity),
        ]
    return []


def _write_execution_report(
    *,
    path: Path,
    phase: str,
    command: str,
    exit_code: int,
    elapsed_seconds: float,
    log_path: Path,
    replace: bool,
) -> list[str]:
    log_digest, digest_errors = lineage_evidence._sha256_file(
        log_path,
        f"staged {phase} execution log",
    )
    if digest_errors:
        return digest_errors
    assert log_digest is not None
    try:
        log_size = log_path.stat().st_size
    except OSError:
        return [f"staged {phase} execution log size could not be read"]
    report = {
        "schema": EXECUTION_REPORT_SCHEMA,
        "phase": phase,
        "command": command,
        "exit_code": exit_code,
        "elapsed_seconds": round(max(elapsed_seconds, 0.0), 6),
        "log_path": log_path.name,
        "log_sha256": log_digest,
        "log_size_bytes": log_size,
    }
    try:
        text = json.dumps(report, allow_nan=False, indent=2, sort_keys=True)
    except (TypeError, ValueError):
        return [f"staged {phase} execution report is not strict JSON"]
    return _write_text_atomic(
        path,
        f"{text}\n",
        f"staged {phase} execution report",
        replace=replace,
    )


def _write_exit_marker(args: argparse.Namespace, exit_code: int) -> list[str]:
    return _write_text_atomic(
        args.exit_file,
        f"{exit_code}\n",
        "staged lineage proof exit marker",
        replace=args.replace or args.resume_key_artifacts,
    )


def _validate_reusable_lineage_artifact(
    staged_artifact_dir: Path,
    artifact: str,
) -> list[str]:
    path = staged_artifact_dir / artifact
    label = f"staged reusable lineage artifact {artifact}"
    file_stat, errors = readiness._validate_lineage_local_file_for_read(path, label)
    if errors:
        return errors
    assert file_stat is not None
    if file_stat.st_size <= 0:
        return [f"{label} must be non-empty"]
    return readiness.validate_lineage_artifact_content(path, artifact)


def _validate_reusable_execution_report(
    *,
    staged_artifact_dir: Path,
    profile: str,
) -> list[str]:
    label = f"staged {profile} lineage key artifact execution report"
    log_name = LINEAGE_KEY_ARTIFACT_LOG_FILENAMES[profile]
    report_name = LINEAGE_EXECUTION_REPORT_FILENAMES[profile]
    text, errors = _read_existing_text_file(
        staged_artifact_dir / report_name,
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
    expected_phase = f"{profile} lineage key artifact command"
    if document["phase"] != expected_phase:
        return [f"{label} phase must be {expected_phase}"]
    command_errors = _validate_report_command(
        document["command"],
        label,
        LINEAGE_KEY_ARTIFACT_COMMANDS[profile],
        f"{profile} lineage key artifact command",
    )
    if command_errors:
        return command_errors
    exit_code = document["exit_code"]
    if isinstance(exit_code, bool) or not isinstance(exit_code, int):
        return [f"{label} exit_code must be an integer"]
    if exit_code != 0:
        return [f"{label} exit_code must be 0 for resume, got {exit_code}"]
    elapsed_seconds = document["elapsed_seconds"]
    if (
        isinstance(elapsed_seconds, bool)
        or not isinstance(elapsed_seconds, (int, float))
        or not math.isfinite(float(elapsed_seconds))
        or not elapsed_seconds > 0
    ):
        return [f"{label} elapsed_seconds must be a finite positive number"]
    if document["log_path"] != log_name:
        return [f"{label} log_path must be {log_name}"]
    log_digest = document["log_sha256"]
    if (
        not isinstance(log_digest, str)
        or len(log_digest) != 64
        or any(char not in "0123456789abcdef" for char in log_digest)
    ):
        return [f"{label} log_sha256 must be a SHA-256 hex digest"]
    log_size = document["log_size_bytes"]
    if isinstance(log_size, bool) or not isinstance(log_size, int) or log_size <= 0:
        return [f"{label} log_size_bytes must be a positive integer"]
    log_path = staged_artifact_dir / log_name
    actual_log_digest, digest_errors = lineage_evidence._sha256_file(
        log_path,
        f"staged {profile} lineage key artifact execution report log",
    )
    if digest_errors:
        return digest_errors
    assert actual_log_digest is not None
    if log_digest != actual_log_digest:
        return [f"{label} log_sha256 must match staged {profile} lineage key artifact log SHA-256"]
    try:
        actual_log_size = log_path.stat().st_size
    except OSError:
        return [f"{label} log size could not be checked"]
    if log_size != actual_log_size:
        return [
            f"{label} log_size_bytes must match staged {profile} lineage key "
            f"artifact log size {actual_log_size}, got {log_size}"
        ]
    return []


def _validate_reusable_key_artifact_phase(
    *,
    staged_artifact_dir: Path,
    profile: str,
) -> list[str]:
    errors: list[str] = []
    for artifact in LINEAGE_KEY_ARTIFACTS_BY_PROFILE[profile]:
        errors.extend(_validate_reusable_lineage_artifact(staged_artifact_dir, artifact))
    log_name = LINEAGE_KEY_ARTIFACT_LOG_FILENAMES[profile]
    log_path = staged_artifact_dir / log_name
    log_stat, log_errors = readiness._validate_lineage_local_file_for_read(
        log_path,
        f"staged reusable {profile} lineage key artifact log",
    )
    errors.extend(log_errors)
    if log_stat is not None and log_stat.st_size <= 0:
        errors.append(f"staged reusable {profile} lineage key artifact log must be non-empty")
    errors.extend(
        _validate_reusable_execution_report(
            staged_artifact_dir=staged_artifact_dir,
            profile=profile,
        )
    )
    return errors


def _profile_has_any_output(profile: str, staged_artifact_dir: Path) -> bool:
    return any((staged_artifact_dir / name).exists() for name in _profile_output_names(profile))


def _try_resume_key_artifact_phase(
    *,
    staged_artifact_dir: Path,
    profile: str,
) -> tuple[bool, list[str]]:
    if not _profile_has_any_output(profile, staged_artifact_dir):
        return False, []
    if not _validate_reusable_key_artifact_phase(
        staged_artifact_dir=staged_artifact_dir,
        profile=profile,
    ):
        return True, []
    cleanup_errors = _cleanup_profile_for_resume(profile, staged_artifact_dir)
    if cleanup_errors:
        return False, cleanup_errors
    return False, []


def _run_lineage_key_artifact_command(
    *,
    profile: str,
    command: str,
    staged_root: Path,
    staged_artifact_dir: Path,
    replace: bool,
    runner: CommandRunner | None,
) -> tuple[int, list[str]]:
    log_name = LINEAGE_KEY_ARTIFACT_LOG_FILENAMES[profile]
    final_log = staged_artifact_dir / log_name
    temp_log = staged_artifact_dir / f".{log_name}.staged-runner.tmp"
    if temp_log.exists() or temp_log.is_symlink():
        return 1, [f"staged {profile} lineage key artifact log temporary output already exists"]
    start = time.monotonic()
    try:
        exit_code = (
            runner(shlex.split(command), staged_root, temp_log)
            if runner is not None
            else _run_command_to_log(shlex.split(command), staged_root, temp_log)
        )
    except OSError as exc:
        temp_identity, temp_identity_errors = _regular_file_identity_for_unlink(
            temp_log,
            f"staged {profile} lineage key artifact log temporary output",
        )
        return 1, [
            f"staged {profile} lineage key artifact command could not be run: {exc}",
            *temp_identity_errors,
            *_cleanup_temp_output(
                temp_log,
                f"staged {profile} lineage key artifact log",
                temp_identity,
            ),
        ]
    elapsed_seconds = max(time.monotonic() - start, 0.000001)
    log_errors = _install_log_temp(
        temp_log,
        final_log,
        f"staged {profile} lineage key artifact log",
        replace=replace,
    )
    if log_errors:
        return 1, log_errors
    report_errors = _write_execution_report(
        path=staged_artifact_dir / LINEAGE_EXECUTION_REPORT_FILENAMES[profile],
        phase=f"{profile} lineage key artifact command",
        command=command,
        exit_code=exit_code,
        elapsed_seconds=elapsed_seconds,
        log_path=final_log,
        replace=replace,
    )
    if report_errors:
        return 1, report_errors
    return exit_code, []


def _write_run_report(
    *,
    path: Path,
    command: str,
    exit_code: int,
    elapsed_seconds: float,
    staged_artifact_dir: Path,
    proof_log_path: Path,
    replace: bool,
) -> list[str]:
    try:
        proof_log_size = proof_log_path.stat().st_size
    except OSError:
        return ["staged lineage proof log size could not be read"]
    key_artifact_logs: dict[str, dict[str, object]] = {}
    for profile, log_name in LINEAGE_KEY_ARTIFACT_LOG_FILENAMES.items():
        log_path = staged_artifact_dir / log_name
        try:
            log_size = log_path.stat().st_size
        except OSError:
            return [f"staged {profile} lineage key artifact log size could not be read"]
        key_artifact_logs[profile] = {
            "path": log_name,
            "size_bytes": log_size,
        }
    report = {
        "schema": STAGED_RUN_REPORT_SCHEMA,
        "command": command,
        "exit_code": exit_code,
        "elapsed_seconds": round(max(elapsed_seconds, 0.0), 6),
        "lineage_key_artifact_logs": key_artifact_logs,
        "proof_log_path": PROOF_LOG_FILENAME,
        "proof_log_size_bytes": proof_log_size,
    }
    try:
        text = json.dumps(report, allow_nan=False, indent=2, sort_keys=True)
    except (TypeError, ValueError):
        return ["staged lineage proof run report is not strict JSON"]
    return _write_text_atomic(
        path,
        f"{text}\n",
        "staged lineage proof run report",
        replace=replace,
    )


def run_staged_lineage_proof(
    args: argparse.Namespace,
    *,
    runner: CommandRunner | None = None,
    monotonic: Callable[[], float] = time.monotonic,
) -> tuple[int, list[str]]:
    """Run the canonical production proof and publish staging metadata."""

    staged_root, errors = _preflight_paths(args)
    errors.extend(
        readiness.validate_lineage_proof_command(
            DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND,
            readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"],
        )
    )
    if errors:
        return 1, errors
    assert staged_root is not None
    try:
        args.staged_artifact_dir.mkdir(parents=True, exist_ok=True)
    except OSError:
        return 1, ["--staged-artifact-dir could not be created"]
    errors = validate_directory_path(
        args.staged_artifact_dir,
        "--staged-artifact-dir",
        must_exist=True,
    )
    if errors:
        return 1, errors
    if args.replace:
        replace_errors = _unlink_replace_outputs(args.staged_artifact_dir)
        if replace_errors:
            return 1, replace_errors

    for profile, command in LINEAGE_KEY_ARTIFACT_COMMANDS.items():
        if args.resume_key_artifacts and not args.replace:
            reused, resume_errors = _try_resume_key_artifact_phase(
                staged_artifact_dir=args.staged_artifact_dir,
                profile=profile,
            )
            if resume_errors:
                return 1, resume_errors
            if reused:
                continue
        keygen_exit, keygen_errors = _run_lineage_key_artifact_command(
            profile=profile,
            command=command,
            staged_root=staged_root,
            staged_artifact_dir=args.staged_artifact_dir,
            replace=args.replace or args.resume_key_artifacts,
            runner=runner,
        )
        if keygen_errors:
            return 1, keygen_errors
        if keygen_exit != 0:
            exit_errors = _write_exit_marker(args, keygen_exit)
            if exit_errors:
                return 1, exit_errors
            return keygen_exit, []

    if args.resume_key_artifacts and not args.replace:
        cleanup_errors = _unlink_resume_outputs(_run_level_output_paths(args))
        if cleanup_errors:
            return 1, cleanup_errors

    proof_log = args.staged_artifact_dir / PROOF_LOG_FILENAME
    temp_log = args.staged_artifact_dir / f".{PROOF_LOG_FILENAME}.staged-runner.tmp"
    if temp_log.exists() or temp_log.is_symlink():
        return 1, ["staged proof log temporary output already exists"]

    command = shlex.split(DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND)
    start = monotonic()
    try:
        exit_code = (
            runner(command, args.repo_root, temp_log)
            if runner is not None
            else _run_command_to_log(command, args.repo_root, temp_log)
        )
    except OSError as exc:
        temp_identity, temp_identity_errors = _regular_file_identity_for_unlink(
            temp_log,
            "staged proof log temporary output",
        )
        return 1, [
            f"staged lineage proof command could not be run: {exc}",
            *temp_identity_errors,
            *_cleanup_temp_output(temp_log, "staged proof log", temp_identity),
        ]
    elapsed_seconds = max(monotonic() - start, 0.000001)

    log_errors = _install_log_temp(
        temp_log,
        proof_log,
        "staged proof log",
        replace=args.replace or args.resume_key_artifacts,
    )
    if log_errors:
        return 1, log_errors
    execution_report_errors = _write_execution_report(
        path=args.staged_artifact_dir / LINEAGE_EXECUTION_REPORT_FILENAMES["proof"],
        phase="lineage proof command",
        command=DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND,
        exit_code=exit_code,
        elapsed_seconds=elapsed_seconds,
        log_path=proof_log,
        replace=args.replace or args.resume_key_artifacts,
    )
    if execution_report_errors:
        return 1, execution_report_errors

    elapsed_errors = _write_text_atomic(
        args.elapsed_seconds_file,
        f"{elapsed_seconds:.6f}\n",
        "staged lineage proof elapsed-seconds file",
        replace=args.replace or args.resume_key_artifacts,
    )
    if elapsed_errors:
        return 1, elapsed_errors
    report_errors = _write_run_report(
        path=args.staged_artifact_dir / RUN_REPORT_FILENAME,
        command=DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND,
        exit_code=exit_code,
        elapsed_seconds=elapsed_seconds,
        staged_artifact_dir=args.staged_artifact_dir,
        proof_log_path=proof_log,
        replace=args.replace or args.resume_key_artifacts,
    )
    if report_errors:
        return 1, report_errors
    exit_errors = _write_exit_marker(args, exit_code)
    if exit_errors:
        return 1, exit_errors
    return exit_code, []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run the canonical Kagemusha Reserved-lineage production proof into "
            "a staged artifact directory, capturing record-archive-proof.log, "
            "elapsed seconds, and the real process exit marker."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=Path("."))
    parser.add_argument("--staged-artifact-dir", type=Path, default=DEFAULT_STAGED_ARTIFACT_DIR)
    parser.add_argument("--exit-file", type=Path, default=DEFAULT_EXIT_FILE)
    parser.add_argument(
        "--elapsed-seconds-file",
        type=Path,
        default=DEFAULT_ELAPSED_SECONDS_FILE,
    )
    parser.add_argument(
        "--replace",
        action="store_true",
        help="Replace previous staged proof log, run report, elapsed file, and exit marker.",
    )
    parser.add_argument(
        "--resume-key-artifacts",
        action="store_true",
        help=(
            "Reuse completed init/append lineage key-artifact phases whose staged "
            "artifacts, logs, and zero-exit execution reports validate; incomplete "
            "regular staged phase outputs are replaced and rerun."
        ),
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    status, errors = run_staged_lineage_proof(args)
    if errors:
        for error in errors:
            print(f"[kagemusha-lineage-staged-runner] {error}", file=sys.stderr)
        return 1
    if status != 0:
        print(
            f"[kagemusha-lineage-staged-runner] staged run exited with {status}",
            file=sys.stderr,
        )
        print(f"[kagemusha-lineage-staged-runner] wrote {args.exit_file}")
        return _wrapper_exit_status(status)
    print(f"[kagemusha-lineage-staged-runner] wrote {args.staged_artifact_dir / PROOF_LOG_FILENAME}")
    print(f"[kagemusha-lineage-staged-runner] wrote {args.staged_artifact_dir / RUN_REPORT_FILENAME}")
    print(f"[kagemusha-lineage-staged-runner] wrote {args.elapsed_seconds_file}")
    print(f"[kagemusha-lineage-staged-runner] wrote {args.exit_file}")
    return _wrapper_exit_status(status)


if __name__ == "__main__":
    raise SystemExit(main())
