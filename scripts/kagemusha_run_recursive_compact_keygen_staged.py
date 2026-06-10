#!/usr/bin/env python3
"""Run ABI-7 recursive compact key generation into a staging directory."""

from __future__ import annotations

import argparse
import json
import math
import os
import time
from pathlib import Path
import shlex
import stat
import subprocess
import sys
from typing import Callable

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402
import kagemusha_production_readiness as readiness  # noqa: E402
import kagemusha_recursive_compact_key_evidence as compact_evidence  # noqa: E402


DEFAULT_TEMP_ROOT = Path("/tmp").resolve()
DEFAULT_STAGED_ROOT = DEFAULT_TEMP_ROOT / "iroha-codex-recursive-compact-keygen-staged"
DEFAULT_STAGED_ARTIFACT_DIR = DEFAULT_STAGED_ROOT / "artifacts" / "kagemusha"
DEFAULT_EXIT_FILE = DEFAULT_TEMP_ROOT / "iroha-codex-kagemusha-compact-keygen-staged.exit"
GENERATOR_LOG_FILENAME = readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
RUN_REPORT_FILENAME = "recursive-compact-key-staged-run.json"
STAGED_RUN_REPORT_SCHEMA = "iroha.kagemusha.recursive_compact_key_staged_run.v1"
EXECUTION_REPORT_FILENAME = "recursive-compact-key-execution.json"
EXECUTION_REPORT_SCHEMA = "iroha.kagemusha.recursive_compact_key_execution.v1"
MAX_EXECUTION_REPORT_BYTES = 16 * 1024
MAX_RUN_REPORT_BYTES = 16 * 1024
MAX_EXIT_MARKER_BYTES = 32
DEFAULT_COMPACT_KEY_COMMAND = compact_evidence.DEFAULT_COMPACT_KEY_COMMAND
CommandRunner = Callable[[list[str], Path, Path], int]


def _secret_path_error(path: Path, label: str) -> str | None:
    if device_lab.SECRET_RE.search(str(path)):
        return f"{label} must not contain secret-looking material"
    return None


def _wrapper_exit_status(command_status: int) -> int:
    """Return a conventional process status for the staging wrapper itself."""

    return 0 if command_status == 0 else 1


def validate_directory_path(path: Path, label: str, *, must_exist: bool) -> list[str]:
    """Reject staging directory aliases before command output is written."""

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
    """Reject output aliases and accidental overwrites before keygen starts."""

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


def _staged_root_from_artifact_dir(path: Path) -> tuple[Path | None, list[str]]:
    if path.name != "kagemusha" or path.parent.name != "artifacts":
        return None, ["--staged-artifact-dir must end with artifacts/kagemusha"]
    return path.parent.parent, []


def _preflight_paths(args: argparse.Namespace) -> tuple[Path | None, list[str]]:
    errors: list[str] = []
    replace_outputs = args.replace or args.resume_keygen
    staged_root, root_errors = _staged_root_from_artifact_dir(args.staged_artifact_dir)
    errors.extend(root_errors)
    if staged_root is not None:
        errors.extend(validate_directory_path(staged_root, "--staged-root", must_exist=False))
    errors.extend(
        validate_directory_path(
            args.staged_artifact_dir,
            "--staged-artifact-dir",
            must_exist=False,
        )
    )
    for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS:
        errors.extend(
            validate_output_file_path(
                args.staged_artifact_dir / artifact,
                f"staged recursive compact key artifact {artifact}",
                replace=replace_outputs,
            )
        )
    errors.extend(
        validate_output_file_path(
            args.staged_artifact_dir / GENERATOR_LOG_FILENAME,
            "staged recursive compact key generator log",
            replace=replace_outputs,
        )
    )
    errors.extend(
        validate_output_file_path(
            args.staged_artifact_dir / RUN_REPORT_FILENAME,
            "staged recursive compact key run report",
            replace=replace_outputs,
        )
    )
    errors.extend(
        validate_output_file_path(
            args.staged_artifact_dir / EXECUTION_REPORT_FILENAME,
            "staged recursive compact key execution report",
            replace=replace_outputs,
        )
    )
    errors.extend(
        validate_output_file_path(
            args.exit_file,
            "staged keygen exit marker",
            replace=replace_outputs,
        )
    )
    return staged_root, errors


def _staged_output_entries(args: argparse.Namespace) -> tuple[tuple[Path, str], ...]:
    artifact_entries = tuple(
        (
            args.staged_artifact_dir / artifact,
            f"staged recursive compact key artifact {artifact}",
        )
        for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS
    )
    return (
        *artifact_entries,
        (
            args.staged_artifact_dir / GENERATOR_LOG_FILENAME,
            "staged recursive compact key generator log",
        ),
        (
            args.staged_artifact_dir / RUN_REPORT_FILENAME,
            "staged recursive compact key run report",
        ),
        (
            args.staged_artifact_dir / EXECUTION_REPORT_FILENAME,
            "staged recursive compact key execution report",
        ),
        (args.exit_file, "staged keygen exit marker"),
    )


def _unlink_replace_outputs(staged_artifact_dir: Path) -> list[str]:
    errors: list[str] = []
    for name in (
        *readiness.COMPACT_KEY_REQUIRED_ARTIFACTS,
        GENERATOR_LOG_FILENAME,
        RUN_REPORT_FILENAME,
        EXECUTION_REPORT_FILENAME,
    ):
        path = staged_artifact_dir / name
        try:
            path.unlink(missing_ok=True)
        except OSError:
            errors.append(f"staged recursive compact key output {name} could not be replaced")
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
        try:
            path.unlink(missing_ok=True)
        except OSError:
            errors.append(f"{label} could not be replaced")
    return errors


def _has_any_staged_output(args: argparse.Namespace) -> bool:
    return any(path.exists() or path.is_symlink() for path, _label in _staged_output_entries(args))


def _write_text_atomic(path: Path, text: str, label: str, *, replace: bool) -> list[str]:
    expected_bytes = text.encode("utf-8")
    errors = validate_output_file_path(path, label, replace=replace)
    if errors:
        return errors
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
    except OSError:
        return [f"{label} parent directory could not be created"]
    tmp_path = path.parent / f".{path.name}.staged-runner.tmp"
    try:
        if tmp_path.exists() or tmp_path.is_symlink():
            return [f"{label} temporary output already exists"]
        with tmp_path.open("xb") as handle:
            handle.write(expected_bytes)
            handle.flush()
            os.fsync(handle.fileno())
        if replace:
            os.replace(tmp_path, path)
        else:
            tmp_path.rename(path)
        dir_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(dir_fd)
        finally:
            os.close(dir_fd)
    except OSError:
        tmp_path.unlink(missing_ok=True)
        return [f"{label} could not be written"]
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
        key = (
            device_lab.SECRET_PATH_REDACTION
            if device_lab.SECRET_RE.search(exc.key)
            else exc.key
        )
        return None, [f"{label} contains duplicate JSON object key {key}"]
    except device_lab.NonFiniteJsonConstantError as exc:
        return None, [f"{label} is not strict JSON: non-finite constant {exc.constant} is not allowed"]
    except json.JSONDecodeError:
        return None, [f"{label} is not valid JSON"]


def _run_command_to_log(command: list[str], cwd: Path, log_path: Path) -> int:
    """Run compact keygen and stream combined output to ``log_path``."""

    with log_path.open("xb") as log_handle:
        process = subprocess.Popen(
            command,
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        assert process.stdout is not None
        for chunk in iter(lambda: process.stdout.read(1024 * 1024), b""):
            log_handle.write(chunk)
            log_handle.flush()
            os.fsync(log_handle.fileno())
            sys.stdout.buffer.write(chunk)
            sys.stdout.buffer.flush()
        return process.wait()


def _install_log_temp(temp_log: Path, final_log: Path, *, replace: bool) -> list[str]:
    errors = validate_output_file_path(
        final_log,
        "staged recursive compact key generator log",
        replace=replace,
    )
    if errors:
        temp_log.unlink(missing_ok=True)
        return errors
    try:
        if replace:
            os.replace(temp_log, final_log)
        else:
            temp_log.rename(final_log)
        dir_fd = os.open(final_log.parent, os.O_RDONLY)
        try:
            os.fsync(dir_fd)
        finally:
            os.close(dir_fd)
    except OSError:
        temp_log.unlink(missing_ok=True)
        return ["staged recursive compact key generator log could not be installed"]
    return []


def _write_execution_report(
    *,
    path: Path,
    command: str,
    exit_code: int,
    elapsed_seconds: float,
    generator_log_path: Path,
    replace: bool,
) -> list[str]:
    try:
        generator_log_size = generator_log_path.stat().st_size
    except OSError:
        return ["staged recursive compact key generator log size could not be read"]
    report = {
        "schema": EXECUTION_REPORT_SCHEMA,
        "phase": "recursive compact keygen command",
        "command": command,
        "exit_code": exit_code,
        "elapsed_seconds": round(max(elapsed_seconds, 0.0), 6),
        "generator_log_path": GENERATOR_LOG_FILENAME,
        "generator_log_size_bytes": generator_log_size,
    }
    try:
        text = json.dumps(
            report,
            allow_nan=False,
            indent=2,
            sort_keys=True,
        )
    except (TypeError, ValueError):
        return ["staged recursive compact key execution report is not strict JSON"]
    return _write_text_atomic(
        path,
        f"{text}\n",
        "staged recursive compact key execution report",
        replace=replace,
    )


def _write_run_report(
    *,
    path: Path,
    command: str,
    exit_code: int,
    elapsed_seconds: float,
    generator_log_path: Path,
    replace: bool,
) -> list[str]:
    try:
        generator_log_size = generator_log_path.stat().st_size
    except OSError:
        return ["staged recursive compact key generator log size could not be read"]
    report = {
        "schema": STAGED_RUN_REPORT_SCHEMA,
        "command": command,
        "exit_code": exit_code,
        "elapsed_seconds": round(max(elapsed_seconds, 0.0), 6),
        "generator_log_path": GENERATOR_LOG_FILENAME,
        "generator_log_size_bytes": generator_log_size,
    }
    try:
        text = json.dumps(
            report,
            allow_nan=False,
            indent=2,
            sort_keys=True,
        )
    except (TypeError, ValueError):
        return ["staged recursive compact key run report is not strict JSON"]
    return _write_text_atomic(
        path,
        f"{text}\n",
        "staged recursive compact key run report",
        replace=replace,
    )


def _compact_artifact_digests_and_sizes(
    staged_artifact_dir: Path,
) -> tuple[dict[str, str], dict[str, int], list[str]]:
    digests: dict[str, str] = {}
    sizes: dict[str, int] = {}
    errors: list[str] = []
    for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS:
        path = staged_artifact_dir / artifact
        digest, artifact_size, prefix, file_errors = compact_evidence._sha256_file_with_size(
            path,
            f"staged reusable recursive compact key artifact {artifact}",
        )
        if file_errors:
            errors.extend(file_errors)
            continue
        assert digest is not None and artifact_size is not None and prefix is not None
        if artifact_size <= 0:
            errors.append(
                f"staged reusable recursive compact key artifact {artifact} must be non-empty"
            )
            continue
        content_errors = readiness.validate_compact_key_artifact_prefix(prefix, artifact)
        if content_errors:
            errors.extend(content_errors)
            continue
        digests[artifact] = digest
        sizes[artifact] = artifact_size
    return digests, sizes, errors


def _validate_reusable_generator_log(
    staged_artifact_dir: Path,
    artifact_digests: dict[str, str],
    artifact_sizes: dict[str, int],
) -> list[str]:
    log_path = staged_artifact_dir / GENERATOR_LOG_FILENAME
    log_digest, digest_errors = compact_evidence._sha256_file(
        log_path,
        "staged reusable recursive compact key generator log",
    )
    if digest_errors:
        return digest_errors
    assert log_digest is not None
    _actual_digest, _parsed_sizes, _parsed_digests, blockers = (
        readiness.validate_compact_key_generator_log(
            log_path,
            log_digest,
            artifact_sizes,
            artifact_digests,
        )
    )
    return [str(blocker["message"]) for blocker in blockers]


def _validate_reusable_execution_report(args: argparse.Namespace) -> list[str]:
    label = "staged recursive compact key execution report"
    text, errors = _read_existing_text_file(
        args.staged_artifact_dir / EXECUTION_REPORT_FILENAME,
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
        "generator_log_size_bytes",
    }
    extra_keys = sorted(set(document) - allowed_keys)
    if extra_keys:
        return [f"{label} contains unexpected field {extra_keys[0]}"]
    missing_keys = sorted(allowed_keys - set(document))
    if missing_keys:
        return [f"{label} is missing {missing_keys[0]}"]
    if document["schema"] != EXECUTION_REPORT_SCHEMA:
        return [f"{label} schema must be {EXECUTION_REPORT_SCHEMA}"]
    if document["phase"] != "recursive compact keygen command":
        return [f"{label} phase must be recursive compact keygen command"]
    if document["command"] != DEFAULT_COMPACT_KEY_COMMAND:
        return [f"{label} command must match the canonical ABI-7 compact key command"]
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
        or float(elapsed_seconds) <= 0
    ):
        return [f"{label} elapsed_seconds must be a finite positive number"]
    if document["generator_log_path"] != GENERATOR_LOG_FILENAME:
        return [f"{label} generator_log_path must be {GENERATOR_LOG_FILENAME}"]
    log_size = document["generator_log_size_bytes"]
    if isinstance(log_size, bool) or not isinstance(log_size, int) or log_size <= 0:
        return [f"{label} generator_log_size_bytes must be a positive integer"]
    try:
        actual_size = (args.staged_artifact_dir / GENERATOR_LOG_FILENAME).stat().st_size
    except OSError:
        return [f"{label} generator log size could not be checked"]
    if log_size != actual_size:
        return [
            f"{label} generator_log_size_bytes must match staged generator log "
            f"size {actual_size}, got {log_size}"
        ]
    return []


def _validate_reusable_run_report(args: argparse.Namespace) -> list[str]:
    label = "staged recursive compact key run report"
    text, errors = _read_existing_text_file(
        args.staged_artifact_dir / RUN_REPORT_FILENAME,
        label,
        max_bytes=MAX_RUN_REPORT_BYTES,
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
        "generator_log_path",
        "generator_log_size_bytes",
    }
    extra_keys = sorted(set(document) - allowed_keys)
    if extra_keys:
        return [f"{label} contains unexpected field {extra_keys[0]}"]
    missing_keys = sorted(allowed_keys - set(document))
    if missing_keys:
        return [f"{label} is missing {missing_keys[0]}"]
    if document["schema"] != STAGED_RUN_REPORT_SCHEMA:
        return [f"{label} schema must be {STAGED_RUN_REPORT_SCHEMA}"]
    if document["command"] != DEFAULT_COMPACT_KEY_COMMAND:
        return [f"{label} command must match the canonical ABI-7 compact key command"]
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
        or float(elapsed_seconds) < 0
    ):
        return [f"{label} elapsed_seconds must be a finite non-negative number"]
    if document["generator_log_path"] != GENERATOR_LOG_FILENAME:
        return [f"{label} generator_log_path must be {GENERATOR_LOG_FILENAME}"]
    log_size = document["generator_log_size_bytes"]
    if isinstance(log_size, bool) or not isinstance(log_size, int) or log_size < 0:
        return [f"{label} generator_log_size_bytes must be a non-negative integer"]
    try:
        actual_size = (args.staged_artifact_dir / GENERATOR_LOG_FILENAME).stat().st_size
    except OSError:
        return [f"{label} generator log size could not be checked"]
    if log_size != actual_size:
        return [
            f"{label} generator_log_size_bytes must match staged generator log "
            f"size {actual_size}, got {log_size}"
        ]
    return []


def _validate_reusable_exit_marker(args: argparse.Namespace) -> list[str]:
    text, errors = _read_existing_text_file(
        args.exit_file,
        "staged keygen exit marker",
        max_bytes=MAX_EXIT_MARKER_BYTES,
    )
    if errors:
        return errors
    assert text is not None
    stripped = text.strip()
    if stripped != "0":
        return [f"staged keygen exit code must be 0 for resume, got {stripped or '<empty>'}"]
    return []


def _validate_reusable_staged_keygen(args: argparse.Namespace) -> list[str]:
    artifact_digests, artifact_sizes, errors = _compact_artifact_digests_and_sizes(
        args.staged_artifact_dir,
    )
    if errors:
        return errors
    errors.extend(
        _validate_reusable_generator_log(
            args.staged_artifact_dir,
            artifact_digests,
            artifact_sizes,
        )
    )
    errors.extend(_validate_reusable_execution_report(args))
    errors.extend(_validate_reusable_run_report(args))
    errors.extend(_validate_reusable_exit_marker(args))
    return errors


def run_staged_keygen(
    args: argparse.Namespace,
    *,
    runner: CommandRunner | None = None,
) -> tuple[int, list[str]]:
    """Run the canonical compact keygen command into staged artifacts."""

    staged_root, errors = _preflight_paths(args)
    errors.extend(readiness.validate_compact_key_command(DEFAULT_COMPACT_KEY_COMMAND))
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
    elif args.resume_keygen and _has_any_staged_output(args):
        reuse_errors = _validate_reusable_staged_keygen(args)
        if not reuse_errors:
            return 0, []
        cleanup_errors = _unlink_resume_outputs(_staged_output_entries(args))
        if cleanup_errors:
            return 1, cleanup_errors

    final_log = args.staged_artifact_dir / GENERATOR_LOG_FILENAME
    temp_log = args.staged_artifact_dir / f".{GENERATOR_LOG_FILENAME}.staged-runner.tmp"
    if temp_log.exists() or temp_log.is_symlink():
        return 1, ["staged recursive compact key generator log temporary output already exists"]

    command = shlex.split(DEFAULT_COMPACT_KEY_COMMAND)
    started = time.monotonic()
    try:
        exit_code = (
            runner(command, staged_root, temp_log)
            if runner is not None
            else _run_command_to_log(command, staged_root, temp_log)
        )
    except OSError as exc:
        temp_log.unlink(missing_ok=True)
        return 1, [f"staged recursive compact keygen command could not be run: {exc}"]

    elapsed_seconds = max(time.monotonic() - started, 0.000001)
    replace_outputs = args.replace or args.resume_keygen
    log_errors = _install_log_temp(temp_log, final_log, replace=replace_outputs)
    if log_errors:
        return 1, log_errors
    execution_report_errors = _write_execution_report(
        path=args.staged_artifact_dir / EXECUTION_REPORT_FILENAME,
        command=DEFAULT_COMPACT_KEY_COMMAND,
        exit_code=exit_code,
        elapsed_seconds=elapsed_seconds,
        generator_log_path=final_log,
        replace=replace_outputs,
    )
    if execution_report_errors:
        return 1, execution_report_errors
    report_errors = _write_run_report(
        path=args.staged_artifact_dir / RUN_REPORT_FILENAME,
        command=DEFAULT_COMPACT_KEY_COMMAND,
        exit_code=exit_code,
        elapsed_seconds=elapsed_seconds,
        generator_log_path=final_log,
        replace=replace_outputs,
    )
    if report_errors:
        return 1, report_errors
    exit_errors = _write_text_atomic(
        args.exit_file,
        f"{exit_code}\n",
        "staged keygen exit marker",
        replace=replace_outputs,
    )
    if exit_errors:
        return 1, exit_errors
    return exit_code, []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run the canonical ABI-7 recursive compact keygen into a staged "
            "artifact directory, capturing recursive-compact-key-artifacts.log "
            "and the real process exit marker."
        )
    )
    parser.add_argument("--staged-artifact-dir", type=Path, default=DEFAULT_STAGED_ARTIFACT_DIR)
    parser.add_argument("--exit-file", type=Path, default=DEFAULT_EXIT_FILE)
    parser.add_argument(
        "--replace",
        action="store_true",
        help="Replace previous staged key artifacts, generator log, and exit marker.",
    )
    parser.add_argument(
        "--resume-keygen",
        action="store_true",
        help=(
            "Reuse a completed zero-exit staged compact keygen whose artifacts, "
            "generator log, execution report, run report, and exit marker validate; "
            "otherwise replace invalid regular staged outputs and rerun."
        ),
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    status, errors = run_staged_keygen(args)
    if errors:
        for error in errors:
            print(f"[kagemusha-compact-keygen-staged-runner] {error}", file=sys.stderr)
        return 1
    if status != 0:
        print(
            f"[kagemusha-compact-keygen-staged-runner] staged keygen exited with {status}",
            file=sys.stderr,
        )
    print(f"[kagemusha-compact-keygen-staged-runner] wrote {args.staged_artifact_dir}")
    print(f"[kagemusha-compact-keygen-staged-runner] wrote {args.exit_file}")
    return _wrapper_exit_status(status)


if __name__ == "__main__":
    raise SystemExit(main())
