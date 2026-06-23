"""Build Reserved-lineage production proof evidence JSON for Kagemusha."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import stat
import sys
import tempfile
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402
import kagemusha_production_readiness as readiness  # noqa: E402


DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND = (
    readiness.expected_lineage_proof_command(
        readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"]
    )
)


def _sha256_file(path: Path, label: str) -> tuple[str | None, list[str]]:
    expected_stat, file_errors = readiness._validate_lineage_local_file_for_read(
        path,
        label,
    )
    if file_errors:
        return None, file_errors
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, [f"{label} must be a regular file"]
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} could not be read"]
    return digest.hexdigest(), []


def _sha256_file_with_size(
    path: Path,
    label: str,
) -> tuple[str | None, int | None, bytes | None, list[str]]:
    expected_stat, file_errors = readiness._validate_lineage_local_file_for_read(
        path,
        label,
    )
    if file_errors:
        return None, None, None, file_errors
    digest = hashlib.sha256()
    prefix_parts: list[bytes] = []
    prefix_remaining = 4096
    size = 0
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if stat.S_ISLNK(path_stat.st_mode):
                return None, None, None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, None, None, [f"{label} must be a regular file"]
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, None, None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, None, None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if prefix_remaining > 0:
                    prefix_parts.append(chunk[:prefix_remaining])
                    prefix_remaining -= min(prefix_remaining, len(chunk))
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return None, None, None, [f"{label} changed while being read"]
    except OSError:
        return None, None, None, [f"{label} could not be read"]
    if size <= 0:
        return None, None, None, [f"{label} must be non-empty"]
    return digest.hexdigest(), size, b"".join(prefix_parts), []


def _secret_path_error(path: str | None, label: str) -> str | None:
    if path is None:
        return None
    if device_lab.SECRET_RE.search(path):
        return f"{label} must not contain secret-looking material"
    if device_lab._contains_control_character(path):
        return f"{label} must not contain control characters"
    candidate = Path(path)
    if (
        path != path.strip()
        or device_lab._path_has_surrounding_whitespace_component(candidate)
    ):
        return f"{label} must not contain surrounding whitespace"
    if "\\" in path:
        return f"{label} must not contain backslashes"
    if ".." in candidate.parts:
        return f"{label} must be canonical"
    return None


def _validate_command(command: str) -> list[str]:
    return readiness.validate_lineage_proof_command(
        command, readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"]
    )


def _validate_elapsed_seconds(value: float) -> list[str]:
    if not math.isfinite(value) or value <= 0:
        return ["--elapsed-seconds must be a positive finite number"]
    return []


def _validate_generated_at_utc(value: str) -> list[str]:
    if device_lab.SIGNED_AT_UTC_RE.fullmatch(value) is None:
        return ["--generated-at-utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ"]
    return []


def _validate_generated_at_future_skew(
    generated_at: dt.datetime | None,
    max_future_skew_seconds: int,
) -> list[str]:
    if max_future_skew_seconds < 0:
        return ["--max-generated-at-future-skew-seconds must be non-negative"]
    if generated_at is None:
        return []
    max_generated_at = (
        dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
        + dt.timedelta(seconds=max_future_skew_seconds)
    )
    if generated_at > max_generated_at:
        return ["--generated-at-utc must not be ahead of the helper clock skew allowance"]
    return []


def _validate_proof_log(path: Path) -> tuple[str | None, list[str]]:
    return readiness.validate_lineage_proof_log(
        path, readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"]
    )


def _cleanup_validation_temp_output(
    path: Path,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    if expected_identity is None:
        return ["lineage proof evidence validation file could not be removed"]
    try:
        parent_fd = os.open(path.parent, device_lab._directory_open_flags())
    except OSError:
        return ["lineage proof evidence validation file could not be removed"]
    try:
        try:
            validation_temp_stat = os.stat(
                path.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            return []
        except OSError:
            return ["lineage proof evidence validation file could not be removed"]
        if (
            not stat.S_ISREG(validation_temp_stat.st_mode)
            or _file_identity(validation_temp_stat) != expected_identity
        ):
            return ["lineage proof evidence validation file changed before cleanup"]
        try:
            os.unlink(path.name, dir_fd=parent_fd)
        except FileNotFoundError:
            return []
        except OSError:
            return ["lineage proof evidence validation file could not be removed"]
        try:
            os.fsync(parent_fd)
        except OSError:
            return ["lineage proof evidence validation file cleanup could not be synced"]
    finally:
        os.close(parent_fd)
    return []


def _validation_temp_identity(handle: Any, path: Path) -> tuple[int, int] | None:
    try:
        return _file_identity(os.fstat(handle.fileno()))
    except (AttributeError, OSError):
        pass
    try:
        path_stat = path.lstat()
    except OSError:
        return None
    if not stat.S_ISREG(path_stat.st_mode):
        return None
    return _file_identity(path_stat)


def build_evidence(
    *,
    artifact_dir: Path,
    proof_log: Path,
    command: str,
    elapsed_seconds: float,
    generated_at_utc: str,
    max_generated_at_future_skew_seconds: int = (
        readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS
    ),
) -> tuple[dict[str, Any] | None, list[str]]:
    """Build a Reserved-lineage proof evidence document from local artifacts."""

    errors: list[str] = []
    errors.extend(_validate_generated_at_utc(generated_at_utc))
    generated_at, timestamp_error = readiness.parse_utc_timestamp(
        generated_at_utc,
        "--generated-at-utc",
    )
    if timestamp_error is not None:
        errors.append(timestamp_error["message"])
    errors.extend(
        _validate_generated_at_future_skew(
            generated_at,
            max_generated_at_future_skew_seconds,
        )
    )
    errors.extend(_validate_command(command))
    errors.extend(_validate_elapsed_seconds(elapsed_seconds))
    if errors:
        return None, errors

    errors.extend(validate_lineage_input_paths(artifact_dir, proof_log))
    if errors:
        return None, errors

    artifact_digests: dict[str, str] = {}
    artifact_sizes: dict[str, int] = {}
    for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS:
        path = artifact_dir / artifact
        digest, artifact_size, artifact_prefix, file_errors = _sha256_file_with_size(
            path,
            f"lineage artifact {artifact}",
        )
        if file_errors:
            if file_errors == [f"lineage artifact {artifact} is missing"]:
                errors.append(f"missing lineage artifact {artifact}")
            else:
                errors.extend(file_errors)
            continue
        assert (
            digest is not None
            and artifact_size is not None
            and artifact_prefix is not None
        )
        content_errors = readiness.validate_lineage_artifact_prefix(artifact_prefix, artifact)
        if content_errors:
            errors.extend(content_errors)
            continue
        artifact_digests[artifact] = digest
        artifact_sizes[artifact] = artifact_size

    proof_log_digest, proof_log_errors = _validate_proof_log(proof_log)
    errors.extend(proof_log_errors)

    if errors:
        return None, errors

    assert generated_at is not None
    assert proof_log_digest is not None
    return (
        {
            "schema": readiness.LINEAGE_PROOF_EVIDENCE_SCHEMA,
            "generated_at_utc": generated_at.isoformat().replace("+00:00", "Z"),
            "opening_len": readiness.EXPECTED_LINEAGE_PROOF_OPENING_LEN,
            "ipa_k": readiness.EXPECTED_LINEAGE_PROOF_IPA_K,
            "verifier_backend": readiness.EXPECTED_LINEAGE_PROOF_BACKEND,
            "verifier_witness_profile": readiness.EXPECTED_LINEAGE_VERIFIER_WITNESS_PROFILE,
            "record_archive_proof_runtime_keygen_env": "unset",
            "circuit_ids": dict(readiness.EXPECTED_LINEAGE_CIRCUIT_IDS),
            "artifacts": artifact_digests,
            "artifact_size_bytes": artifact_sizes,
            "tests": {
                "record_archive_proof": {
                    "name": readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"],
                    "status": "passed",
                    "ignored": True,
                    "command": command,
                    "elapsed_seconds": elapsed_seconds,
                    "log_path": readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                        "record_archive_proof"
                    ],
                    "log_sha256": proof_log_digest,
                }
            }
        },
        [],
    )


def validate_evidence_document(evidence: dict[str, Any], artifact_dir: Path) -> list[str]:
    """Return readiness-validator blocker messages for a generated evidence document."""

    secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")
    if secret_error is not None:
        return [secret_error]
    pre_create_dir_errors = validate_artifact_dir_path(artifact_dir)
    if pre_create_dir_errors:
        return pre_create_dir_errors
    try:
        evidence_text = json.dumps(
            evidence,
            indent=2,
            sort_keys=True,
            allow_nan=False,
        ) + "\n"
    except ValueError:
        return ["lineage proof evidence validation file is not strict JSON"]
    try:
        artifact_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    except OSError:
        return ["--artifact-dir could not be created for evidence validation"]
    post_create_dir_errors = validate_artifact_dir_path(artifact_dir)
    if post_create_dir_errors:
        return post_create_dir_errors
    permission_errors = _set_private_directory_permissions(
        artifact_dir,
        "--artifact-dir",
    )
    if permission_errors:
        return permission_errors
    path: Path | None = None
    tmp_identity: tuple[int, int] | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w",
            encoding="utf-8",
            dir=artifact_dir,
            prefix=".lineage-proof-evidence-",
            suffix=".json",
            delete=False,
        ) as handle:
            path = Path(handle.name)
            tmp_identity = _validation_temp_identity(handle, path)
            os.fchmod(handle.fileno(), 0o600)
            handle.write(evidence_text)
            handle.flush()
            os.fsync(handle.fileno())
    except (AttributeError, OSError):
        errors = ["lineage proof evidence validation file could not be written"]
        if path is not None:
            errors.extend(_cleanup_validation_temp_output(path, tmp_identity))
        return errors
    result = readiness.check_lineage_proof_evidence(
        path, require_canonical_filename=False
    )
    cleanup_errors = _cleanup_validation_temp_output(path, tmp_identity)
    if cleanup_errors:
        return cleanup_errors
    return [item["message"] for item in result["blockers"]]


def validate_artifact_dir_path(artifact_dir: Path) -> list[str]:
    """Reject artifact directories that could alias external release bytes."""

    secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")
    if secret_error is not None:
        return [secret_error]
    try:
        artifact_dir_mode = artifact_dir.lstat().st_mode
    except FileNotFoundError:
        artifact_dir_mode = None
    except OSError:
        return ["--artifact-dir metadata could not be read"]
    if artifact_dir_mode is not None and stat.S_ISLNK(artifact_dir_mode):
        return ["--artifact-dir must not be a symlink"]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        artifact_dir,
        "--artifact-dir ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    if artifact_dir_mode is None:
        return []
    if not stat.S_ISDIR(artifact_dir_mode):
        return ["--artifact-dir must be a directory"]
    return []


def _resolve_corridor_path(path: Path, label: str) -> tuple[Path | None, list[str]]:
    try:
        return path.resolve(), []
    except OSError:
        return None, [f"{label} could not be resolved"]


def _same_resolved_parent(child: Path, parent: Path) -> tuple[bool | None, list[str]]:
    child_parent, child_errors = _resolve_corridor_path(child.parent, "--proof-log parent")
    if child_errors:
        return None, child_errors
    parent_resolved, parent_errors = _resolve_corridor_path(parent, "--artifact-dir")
    if parent_errors:
        return None, parent_errors
    assert child_parent is not None
    assert parent_resolved is not None
    return child_parent == parent_resolved, []


def validate_output_corridor(out_path: Path, artifact_dir: Path) -> list[str]:
    """Validate that --out resolves directly under --artifact-dir."""

    out_secret_error = _secret_path_error(str(out_path), "--out")
    if out_secret_error is not None:
        return [out_secret_error]
    artifact_dir_secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")
    if artifact_dir_secret_error is not None:
        return [artifact_dir_secret_error]
    output_parent, output_parent_errors = _resolve_corridor_path(
        out_path.parent,
        "--out parent",
    )
    if output_parent_errors:
        return output_parent_errors
    artifact_dir_resolved, artifact_dir_errors = _resolve_corridor_path(
        artifact_dir,
        "--artifact-dir",
    )
    if artifact_dir_errors:
        return artifact_dir_errors
    assert output_parent is not None
    assert artifact_dir_resolved is not None
    if output_parent != artifact_dir_resolved:
        return ["--out must be written directly under --artifact-dir"]
    return []


def validate_lineage_input_paths(artifact_dir: Path, proof_log: Path) -> list[str]:
    """Reject detached or aliased lineage proof inputs before reading bytes."""

    proof_log_secret_error = _secret_path_error(str(proof_log), "--proof-log")
    if proof_log_secret_error is not None:
        return [proof_log_secret_error]
    expected_proof_log_name = readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
        "record_archive_proof"
    ]
    if proof_log.name != expected_proof_log_name:
        return [
            "--proof-log must be written directly under --artifact-dir as "
            f"{expected_proof_log_name}"
        ]
    errors = validate_artifact_dir_path(artifact_dir)
    if errors:
        return errors
    proof_log_ancestor_errors = device_lab.validate_no_symlink_ancestors(
        proof_log,
        "--proof-log ancestor directory",
    )
    if proof_log_ancestor_errors:
        return proof_log_ancestor_errors
    same_parent, corridor_errors = _same_resolved_parent(proof_log, artifact_dir)
    if corridor_errors:
        return corridor_errors
    if not same_parent:
        return [
            "--proof-log must be written directly under --artifact-dir as "
            f"{expected_proof_log_name}"
        ]
    return []


def preflight_output_path(path: Path, label: str) -> list[str]:
    """Reject aliased output paths before evidence inputs are read."""

    secret_error = _secret_path_error(str(path), label)
    if secret_error is not None:
        return [secret_error]
    parent = path.parent
    parent_exists, parent_errors = _validate_output_parent(path, label)
    if parent_errors:
        return parent_errors
    output_ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if output_ancestor_errors:
        return output_ancestor_errors
    if not parent_exists:
        try:
            parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        except OSError:
            return [f"{label} parent directory could not be created"]
    parent_exists, parent_errors = _validate_output_parent(
        path,
        label,
        missing_error=f"{label} parent must be a directory",
    )
    if parent_errors:
        return parent_errors
    if not parent_exists:
        return [f"{label} parent must be a directory"]
    output_ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if output_ancestor_errors:
        return output_ancestor_errors
    try:
        output_mode = path.lstat().st_mode
    except FileNotFoundError:
        return []
    except OSError:
        return [f"{label} file metadata could not be read"]
    if stat.S_ISLNK(output_mode):
        return [f"{label} must not be a symlink"]
    if not stat.S_ISREG(output_mode):
        return [f"{label} must be a regular file"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return [f"{label} must not be hardlinked"]
    return []


def _validate_output_parent(
    path: Path,
    label: str,
    *,
    missing_error: str | None = None,
) -> tuple[bool, list[str]]:
    """Classify an output parent without following symlink aliases."""

    parent = path.parent
    try:
        parent_mode = parent.lstat().st_mode
    except FileNotFoundError:
        if missing_error is None:
            return False, []
        return False, [missing_error]
    except OSError:
        return False, [f"{label} parent directory metadata could not be read"]
    if stat.S_ISLNK(parent_mode):
        return True, [f"{label} parent directory must not be a symlink"]
    if not stat.S_ISDIR(parent_mode):
        return True, [f"{label} parent must be a directory"]
    return True, []


def _file_identity(file_stat: os.stat_result) -> tuple[int, int]:
    return file_stat.st_dev, file_stat.st_ino


def _directory_open_flags() -> int:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _set_private_directory_permissions(path: Path, label: str) -> list[str]:
    try:
        dir_fd = os.open(path, _directory_open_flags())
    except OSError:
        return [f"{label} permissions could not be set"]
    try:
        try:
            directory_stat = os.fstat(dir_fd)
        except OSError:
            return [f"{label} permissions could not be verified"]
        if not stat.S_ISDIR(directory_stat.st_mode):
            return [f"{label} permissions could not be verified"]
        try:
            os.fchmod(dir_fd, 0o700)
        except OSError:
            return [f"{label} permissions could not be set"]
        try:
            directory_stat = os.fstat(dir_fd)
        except OSError:
            return [f"{label} permissions could not be verified"]
        if not stat.S_ISDIR(directory_stat.st_mode):
            return [f"{label} permissions could not be verified"]
        if stat.S_IMODE(directory_stat.st_mode) != 0o700:
            return [f"{label} permissions must be 0700"]
    finally:
        os.close(dir_fd)
    return []


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
        return _sync_output_parent_fd(
            parent_fd,
            label,
            expected_identity=expected_identity,
        )
    finally:
        os.close(parent_fd)


def _sync_output_parent_fd(
    parent_fd: int,
    label: str,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    try:
        parent_stat = os.fstat(parent_fd)
        if not stat.S_ISDIR(parent_stat.st_mode):
            return [f"{label} parent directory could not be synced"]
        if expected_identity is not None and _file_identity(parent_stat) != expected_identity:
            return [f"{label} parent directory changed before sync"]
        os.fsync(parent_fd)
    except OSError:
        return [f"{label} parent directory could not be synced"]
    return []


def validate_output_path(path: Path, label: str) -> list[str]:
    """Reject output paths that could overwrite aliased local files."""

    secret_error = _secret_path_error(str(path), label)
    if secret_error is not None:
        return [secret_error]
    errors = preflight_output_path(path, label)
    if errors:
        return errors
    parent = path.parent
    parent_exists, parent_errors = _validate_output_parent(path, label)
    if parent_errors:
        return parent_errors
    if not parent_exists:
        try:
            parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        except OSError:
            return [f"{label} parent directory could not be created"]
    permission_errors = _set_private_directory_permissions(parent, f"{label} parent")
    if permission_errors:
        return permission_errors
    return preflight_output_path(path, label)


def _read_output_text(
    path: Path,
    expected_stat: os.stat_result,
    label: str,
    *,
    max_bytes: int | None = None,
) -> tuple[str | None, list[str]]:
    """Read helper output text without trusting a stale path."""

    chunks: list[bytes] = []
    output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                return None, [f"{label} must be a regular file"]
            output_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if output_open_identity != output_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != output_expected_identity:
                return None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            if max_bytes is not None and open_stat.st_size > max_bytes:
                return None, [f"{label} evidence exceeds maximum size"]
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if max_bytes is not None and size > max_bytes:
                    return None, [f"{label} evidence exceeds maximum size"]
                chunks.append(chunk)
            final_path_stat = path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != output_expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} write verification failed"]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [f"{label} write verification failed"]


def write_evidence(
    path: Path,
    evidence: dict[str, Any],
    *,
    max_bytes: int | None = None,
) -> list[str]:
    errors = validate_output_path(path, "--out")
    if errors:
        return errors
    try:
        parent_stat = path.parent.lstat()
    except OSError:
        return ["--out parent directory metadata could not be read"]
    if stat.S_ISLNK(parent_stat.st_mode) or not stat.S_ISDIR(parent_stat.st_mode):
        return ["--out parent directory could not be synced"]
    parent_identity = _file_identity(parent_stat)
    try:
        evidence_text = json.dumps(
            evidence,
            indent=2,
            sort_keys=True,
            allow_nan=False,
        ) + "\n"
    except ValueError:
        return ["--out evidence is not strict JSON"]
    if max_bytes is None:
        max_bytes = readiness.MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES
    if len(evidence_text.encode("utf-8")) > max_bytes:
        return ["--out evidence exceeds maximum size"]
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return ["--out parent directory metadata could not be read"]
    try:
        try:
            opened_parent_stat = os.fstat(parent_fd)
        except OSError:
            return ["--out parent directory metadata could not be read"]
        if (
            not stat.S_ISDIR(opened_parent_stat.st_mode)
            or _file_identity(opened_parent_stat) != parent_identity
        ):
            return ["--out parent directory changed before sync"]
        return _write_evidence_with_parent_fd(
            path,
            evidence_text,
            max_bytes=max_bytes,
            parent_fd=parent_fd,
            parent_identity=parent_identity,
        )
    finally:
        os.close(parent_fd)


def _write_evidence_with_parent_fd(
    path: Path,
    evidence_text: str,
    *,
    max_bytes: int,
    parent_fd: int,
    parent_identity: tuple[int, int],
) -> list[str]:
    tmp_path: Path | None = None
    tmp_identity: tuple[int, int] | None = None
    write_errors: list[str] = []
    try:
        with tempfile.NamedTemporaryFile(
            "w",
            dir=path.parent,
            encoding="utf-8",
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            tmp_path = Path(handle.name)
            tmp_identity = _file_identity(os.fstat(handle.fileno()))
            os.fchmod(handle.fileno(), 0o600)
            handle.write(evidence_text)
            handle.flush()
            os.fsync(handle.fileno())
        errors = validate_output_path(path, "--out")
        if errors:
            write_errors.extend(errors)
        else:
            os.replace(
                tmp_path.name,
                path.name,
                src_dir_fd=parent_fd,
                dst_dir_fd=parent_fd,
            )
            tmp_path = None
    except OSError:
        write_errors.append("--out could not be written")
    finally:
        if tmp_path is not None:
            write_errors.extend(_cleanup_temp_output(tmp_path, tmp_identity))
    if write_errors:
        return write_errors
    try:
        expected_stat = os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
    except (FileNotFoundError, OSError):
        return ["--out write verification failed"]
    if not stat.S_ISREG(expected_stat.st_mode):
        return ["--out write verification failed"]
    output_identity = _file_identity(expected_stat)
    try:
        current_parent_stat = path.parent.lstat()
    except OSError:
        cleanup_errors = _unlink_file_if_identity_at(
            parent_fd,
            path.name,
            output_identity,
        )
        return ["--out parent directory metadata could not be read", *cleanup_errors]
    if _file_identity(current_parent_stat) != parent_identity:
        cleanup_errors = _unlink_file_if_identity_at(
            parent_fd,
            path.name,
            output_identity,
        )
        return ["--out parent directory changed before sync", *cleanup_errors]
    sync_errors = _sync_output_parent_fd(
        parent_fd,
        "--out",
        expected_identity=parent_identity,
    )
    if sync_errors:
        cleanup_errors = _unlink_file_if_identity_at(
            parent_fd,
            path.name,
            output_identity,
        )
        return [*sync_errors, *cleanup_errors]
    errors = validate_output_path(path, "--out")
    if errors:
        return errors
    if stat.S_IMODE(expected_stat.st_mode) != 0o600:
        return ["--out permissions must be 0600"]
    readback_text, readback_errors = _read_output_text(
        path,
        expected_stat,
        "--out",
        max_bytes=max_bytes,
    )
    if readback_errors:
        return readback_errors
    if readback_text != evidence_text:
        return ["--out write verification failed"]
    return []


def _unlink_file_if_identity_at(
    parent_fd: int,
    name: str,
    expected_identity: tuple[int, int],
) -> list[str]:
    try:
        file_stat = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return []
    except OSError:
        return ["--out rollback cleanup metadata could not be read"]
    if not stat.S_ISREG(file_stat.st_mode) or _file_identity(file_stat) != expected_identity:
        return []
    try:
        os.unlink(name, dir_fd=parent_fd)
    except FileNotFoundError:
        return []
    except OSError:
        return ["--out could not be removed after parent sync failure"]
    try:
        os.fsync(parent_fd)
    except OSError:
        return ["--out cleanup could not be synced after parent sync failure"]
    return []


def _cleanup_temp_output(
    path: Path,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    if expected_identity is None:
        return ["--out temporary file metadata could not be read"]
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return ["--out temporary file could not be removed"]
    try:
        try:
            temp_stat = os.stat(
                path.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            return []
        except OSError:
            return ["--out temporary file could not be removed"]
        if (
            not stat.S_ISREG(temp_stat.st_mode)
            or _file_identity(temp_stat) != expected_identity
        ):
            return ["--out temporary file changed before cleanup"]
        try:
            os.unlink(path.name, dir_fd=parent_fd)
        except FileNotFoundError:
            return []
        except OSError:
            return ["--out temporary file could not be removed"]
        try:
            os.fsync(parent_fd)
        except OSError:
            return ["--out temporary file cleanup could not be synced"]
    finally:
        os.close(parent_fd)
    return []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Build Kagemusha Reserved-lineage production proof evidence JSON."
    )
    parser.add_argument(
        "--artifact-dir",
        default="artifacts/kagemusha",
        help="Directory containing lineage-init/lineage-append key packages and records.",
    )
    parser.add_argument(
        "--proof-log",
        required=True,
        help="Captured stdout/stderr log from the production ignored record-archive proof run.",
    )
    parser.add_argument(
        "--command",
        default=DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND,
        help="Exact command used to run the production ignored record-archive proof test.",
    )
    parser.add_argument(
        "--elapsed-seconds",
        required=True,
        type=float,
        help="Wall-clock seconds consumed by the production proof run.",
    )
    parser.add_argument(
        "--generated-at-utc",
        default=readiness.utc_now(),
        help="Canonical ISO-8601 UTC timestamp for the evidence document.",
    )
    parser.add_argument(
        "--max-generated-at-future-skew-seconds",
        type=int,
        default=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help=(
            "Maximum number of seconds generated_at_utc may be ahead of the "
            "helper clock."
        ),
    )
    parser.add_argument(
        "--out",
        default=readiness.DEFAULT_LINEAGE_PROOF_EVIDENCE_PATH,
        help="Output evidence JSON path.",
    )
    args = parser.parse_args(argv)

    path_errors = [
        error
        for error in (
            _secret_path_error(args.artifact_dir, "--artifact-dir"),
            _secret_path_error(args.proof_log, "--proof-log"),
            _secret_path_error(args.out, "--out"),
        )
        if error is not None
    ]
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1

    artifact_dir = Path(args.artifact_dir)
    proof_log = Path(args.proof_log)
    out_path = Path(args.out)
    if out_path.name != readiness.LINEAGE_PROOF_EVIDENCE_FILENAME:
        path_errors.append(
            f"--out must be named {readiness.LINEAGE_PROOF_EVIDENCE_FILENAME}"
        )
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1

    scalar_errors: list[str] = []
    scalar_errors.extend(_validate_generated_at_utc(args.generated_at_utc))
    generated_at, timestamp_error = readiness.parse_utc_timestamp(
        args.generated_at_utc,
        "--generated-at-utc",
    )
    if timestamp_error is not None:
        scalar_errors.append(timestamp_error["message"])
    scalar_errors.extend(
        _validate_generated_at_future_skew(
            generated_at,
            args.max_generated_at_future_skew_seconds,
        )
    )
    scalar_errors.extend(_validate_command(args.command))
    scalar_errors.extend(_validate_elapsed_seconds(args.elapsed_seconds))
    if scalar_errors:
        for error in scalar_errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1

    path_errors.extend(validate_lineage_input_paths(artifact_dir, proof_log))
    path_errors.extend(validate_output_corridor(out_path, artifact_dir))
    early_output_errors = preflight_output_path(out_path, "--out")
    path_errors.extend(early_output_errors)
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1

    evidence, errors = build_evidence(
        artifact_dir=artifact_dir,
        proof_log=proof_log,
        command=args.command,
        elapsed_seconds=args.elapsed_seconds,
        generated_at_utc=args.generated_at_utc,
        max_generated_at_future_skew_seconds=args.max_generated_at_future_skew_seconds,
    )
    if errors:
        for error in errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1

    assert evidence is not None
    validation_errors = validate_evidence_document(evidence, artifact_dir)
    if validation_errors:
        for error in validation_errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1

    write_errors = write_evidence(out_path, evidence)
    if write_errors:
        for error in write_errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1
    print("[kagemusha-lineage-proof-evidence] wrote evidence")
    return 0


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
