#!/usr/bin/env python3
"""Finalize a completed staged ABI-7 recursive compact keygen run."""

from __future__ import annotations

import argparse
import datetime as dt
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
MAX_STAGED_RUN_REPORT_BYTES = 16 * 1024
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
    if device_lab.SECRET_RE.search(str(path)):
        return f"{label} must not contain secret-looking material"
    if device_lab._contains_control_character(str(path)):
        return f"{label} must not contain control characters"
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
        dir_stat = os.fstat(dir_fd)
        if not stat.S_ISDIR(dir_stat.st_mode):
            return ["artifact directory could not be synced"]
        if expected_identity is not None and _file_identity(dir_stat) != expected_identity:
            return ["artifact directory changed before sync"]
        os.fsync(dir_fd)
    except OSError:
        return ["artifact directory could not be synced"]
    finally:
        os.close(dir_fd)
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
                return []
            except OSError:
                return [f"{label} rollback cleanup could not remove file"]
    finally:
        os.close(parent_fd)
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
    except device_lab.NonFiniteJsonConstantError as exc:
        return None, [f"{label} is not strict JSON: non-finite constant {exc.constant} is not allowed"]
    except json.JSONDecodeError:
        return None, [f"{label} is not valid JSON"]


def validate_staged_run_report(
    *,
    staged_artifact_dir: Path,
    expected_exit_code: int,
    expected_command: str,
) -> list[str]:
    """Validate the staged runner report before trusting a successful marker."""

    label = "staged recursive compact key run report"
    path = staged_artifact_dir / RUN_REPORT_FILENAME
    text, errors = _read_small_text_file(path, label, max_bytes=MAX_STAGED_RUN_REPORT_BYTES)
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
    if isinstance(elapsed_seconds, bool) or not isinstance(elapsed_seconds, (int, float)):
        return [f"{label} elapsed_seconds must be a finite non-negative number"]
    if not math.isfinite(float(elapsed_seconds)) or float(elapsed_seconds) < 0:
        return [f"{label} elapsed_seconds must be a finite non-negative number"]
    if document["generator_log_path"] != readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME:
        return [
            f"{label} generator_log_path must be "
            f"{readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME}"
        ]
    size = document["generator_log_size_bytes"]
    if isinstance(size, bool) or not isinstance(size, int) or size < 0:
        return [f"{label} generator_log_size_bytes must be a non-negative integer"]
    try:
        actual_size = (
            staged_artifact_dir / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
        ).stat().st_size
    except OSError:
        return [f"{label} generator log size could not be checked"]
    if size != actual_size:
        return [
            f"{label} generator_log_size_bytes must match staged generator log "
            f"size {actual_size}, got {size}"
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
    destination.parent.mkdir(parents=True, exist_ok=True)
    expected_stat = source.lstat()
    expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    destination_identity: tuple[int, int] | None = None
    try:
        with source.open("rb") as src, destination.open("xb") as dst:
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
    copied_digest, copied_size, _copied_prefix, copied_errors = (
        compact_evidence._sha256_file_with_size(destination, f"published {destination.name}")
    )
    if copied_errors:
        return copied_errors, destination_identity
    if copied_digest != digest or copied_size != size:
        return [f"published {destination.name} does not match staged bytes"], destination_identity
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
    artifact_dir.mkdir(parents=True, exist_ok=True)
    errors.extend(validate_directory_path(artifact_dir, "artifact directory", must_exist=True))
    if errors:
        return errors
    try:
        artifact_dir_stat = artifact_dir.lstat()
    except OSError:
        return ["artifact directory metadata could not be read"]
    artifact_dir_identity = _file_identity(artifact_dir_stat)
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
    installed: list[tuple[Path, tuple[int, int]]] = []
    for name in _required_publish_filenames():
        source = stage_dir / name
        destination = artifact_dir / name
        tmp_destination = artifact_dir / f".{name}.staged-finalizer.tmp"
        tmp_identity: tuple[int, int] | None = None
        try:
            if tmp_destination.exists() or tmp_destination.is_symlink():
                return [f"temporary output for {name} already exists"]
            copy_errors, tmp_identity = _copy_validated_file(
                source,
                tmp_destination,
                f"validated staged {name}",
            )
            if tmp_identity is None:
                tmp_identity = _regular_file_identity(tmp_destination)
            if copy_errors:
                cleanup_errors: list[str] = []
                if tmp_identity is not None:
                    cleanup_errors.extend(
                        _unlink_file_if_identity(
                            tmp_destination,
                            tmp_identity,
                            label=f"temporary output for {name}",
                        )
                    )
                cleanup_errors.extend(_cleanup_published_files(installed))
                return [*copy_errors, *cleanup_errors]
            if replace:
                os.replace(tmp_destination, destination)
            else:
                tmp_destination.rename(destination)
            destination_identity = _regular_file_identity(destination)
            if destination_identity is None:
                cleanup_errors = _cleanup_published_files(installed)
                return [f"published {name} could not be installed", *cleanup_errors]
            verify_errors = _verify_published_file(
                source,
                destination,
                f"published {name}",
            )
            if verify_errors:
                cleanup_errors = _unlink_file_if_identity(
                    destination,
                    destination_identity,
                    label=f"published {name}",
                )
                cleanup_errors.extend(_cleanup_published_files(installed))
                return [*verify_errors, *cleanup_errors]
            installed.append((destination, destination_identity))
        except OSError:
            cleanup_errors = []
            if tmp_identity is None:
                tmp_identity = _regular_file_identity(tmp_destination)
            if tmp_identity is not None:
                cleanup_errors.extend(
                    _unlink_file_if_identity(
                        tmp_destination,
                        tmp_identity,
                        label=f"temporary output for {name}",
                    )
                )
            cleanup_errors.extend(_cleanup_published_files(installed))
            return [f"published {name} could not be installed", *cleanup_errors]
    return _sync_artifact_dir(
        artifact_dir,
        expected_identity=artifact_dir_identity,
    )


def finalize_staged_run(args: argparse.Namespace) -> tuple[int, Path | None, list[str]]:
    """Finalize a completed staged ABI-7 compact keygen run."""

    errors: list[str] = []
    errors.extend(validate_directory_path(args.staged_artifact_dir, "--staged-artifact-dir", must_exist=True))
    errors.extend(validate_directory_path(args.artifact_dir, "--artifact-dir", must_exist=False))
    exit_code_text, exit_errors = validate_exit_marker(args.exit_file)
    errors.extend(exit_errors)
    if exit_errors:
        return 1, None, errors
    assert exit_code_text is not None
    errors.extend(compact_evidence._validate_generated_at_utc(args.generated_at_utc))
    errors.extend(readiness.validate_compact_key_command(args.command))
    if exit_code_text == "0":
        errors.extend(
            validate_staged_run_report(
                staged_artifact_dir=args.staged_artifact_dir,
                expected_exit_code=0,
                expected_command=args.command,
            )
        )
    if errors:
        return 1, None, errors

    try:
        args.artifact_dir.mkdir(parents=True, exist_ok=True)
    except OSError:
        return 1, None, ["--artifact-dir could not be created"]
    errors = validate_directory_path(args.artifact_dir, "--artifact-dir", must_exist=True)
    if errors:
        return 1, None, errors

    temp_parent = Path(tempfile.mkdtemp(prefix=".recursive-compact-finalize.", dir=args.artifact_dir))
    try:
        temp_parent_identity = _file_identity(temp_parent.lstat())
    except OSError:
        return 1, None, ["staged finalizer temporary directory metadata could not be read"]
    stage_dir = temp_parent / "stage"
    finalizer_errors: list[str] = []
    try:
        stage_dir.mkdir()
        _evidence, stage_errors = stage_compact_key_evidence(
            staged_artifact_dir=args.staged_artifact_dir,
            stage_dir=stage_dir,
            generated_at_utc=args.generated_at_utc,
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
