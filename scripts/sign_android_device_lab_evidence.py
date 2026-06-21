"""Build and sign Kagemusha Android device-lab evidence artifacts."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402


DEFAULT_SIGNED_EVIDENCE_PATH = device_lab.KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH


def _secret_key_path_error(path: Path, label: str) -> str | None:
    path_text = str(path)
    if device_lab.SECRET_RE.search(path_text):
        return f"{label} path must not contain secret-looking material"
    if device_lab._contains_control_character(path_text):
        return f"{label} path must not contain control characters"
    if "\\" in path_text:
        return f"{label} path must not contain backslashes"
    if ".." in path.parts:
        return f"{label} path must be canonical"
    return None


def _validate_json_output_path(path: Path, label: str) -> list[str]:
    """Validate a signer-controlled output immediately before writing."""

    path_text = str(path)
    if device_lab.SECRET_RE.search(path_text):
        return [f"{label} must not contain secret-looking material"]
    if device_lab._contains_control_character(path_text):
        return [f"{label} must not contain control characters"]
    if "\\" in path_text:
        return [f"{label} must not contain backslashes"]
    if ".." in path.parts:
        return [f"{label} must be canonical"]
    errors: list[str] = []
    parent = path.parent
    parent_exists, parent_errors = _validate_json_output_parent(path, label)
    errors.extend(parent_errors)
    if errors:
        return errors
    errors.extend(
        device_lab.validate_no_symlink_ancestors(
            path,
            f"{label} ancestor directory",
        )
    )
    if errors:
        return errors
    if not parent_exists:
        try:
            parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        except OSError:
            errors.append(f"{label} parent directory could not be created")
    if errors:
        return errors
    parent_exists, parent_errors = _validate_json_output_parent(
        path,
        label,
        missing_error=f"{label} parent must be a directory",
    )
    errors.extend(parent_errors)
    if not parent_exists and not errors:
        errors.append(f"{label} parent must be a directory")
    if errors:
        return errors
    errors.extend(
        device_lab.validate_no_symlink_ancestors(
            path,
            f"{label} ancestor directory",
        )
    )
    if errors:
        return errors
    permission_errors = _set_private_directory_permissions(
        parent,
        f"{label} parent directory",
    )
    if permission_errors:
        return permission_errors

    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return errors
    except OSError:
        errors.append(f"{label} file metadata could not be read")
        return errors
    if stat.S_ISLNK(mode):
        errors.append(f"{label} must not be a symlink")
    elif not stat.S_ISREG(mode):
        errors.append(f"{label} must be a regular file")
    else:
        try:
            link_count = path.stat().st_nlink
        except OSError:
            errors.append(f"{label} hardlink metadata could not be read")
        else:
            if link_count > 1:
                errors.append(f"{label} must not be hardlinked")
    return errors


def _validate_json_output_parent(
    path: Path,
    label: str,
    *,
    missing_error: str | None = None,
) -> tuple[bool, list[str]]:
    """Classify a signer-controlled output parent without following aliases."""

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


def _validate_existing_json_output_path(path: Path, label: str) -> list[str]:
    """Validate a signer-controlled output immediately before reading it back."""

    path_text = str(path)
    if device_lab.SECRET_RE.search(path_text):
        return [f"{label} must not contain secret-looking material"]
    if device_lab._contains_control_character(path_text):
        return [f"{label} must not contain control characters"]
    if "\\" in path_text:
        return [f"{label} must not contain backslashes"]
    if ".." in path.parts:
        return [f"{label} must be canonical"]
    _, parent_errors = _validate_json_output_parent(
        path,
        label,
        missing_error=f"{label} parent directory is missing",
    )
    if parent_errors:
        return parent_errors
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return [f"{label} must exist before digest"]
    except OSError:
        return [f"{label} file metadata could not be read"]
    if stat.S_ISLNK(mode):
        return [f"{label} must not be a symlink"]
    if not stat.S_ISREG(mode):
        return [f"{label} must be a regular file"]
    if stat.S_IMODE(mode) != 0o600:
        return [f"{label} permissions must be 0600"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return [f"{label} must not be hardlinked"]
    return []


def _output_file_sha256(path: Path, label: str) -> tuple[str | None, list[str]]:
    errors = _validate_existing_json_output_path(path, label)
    if errors:
        return None, errors
    try:
        expected_stat = path.lstat()
    except FileNotFoundError:
        return None, [f"{label} must exist before digest"]
    except OSError:
        return None, [f"{label} file metadata could not be read"]
    if stat.S_ISLNK(expected_stat.st_mode):
        return None, [f"{label} must not be a symlink"]
    if not stat.S_ISREG(expected_stat.st_mode):
        return None, [f"{label} must be a regular file"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return None, [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return None, [f"{label} must not be hardlinked"]
    payload, read_errors = _read_existing_output_bytes(path, expected_stat, label)
    if read_errors:
        return None, read_errors
    assert payload is not None
    return hashlib.sha256(payload).hexdigest(), []


def _read_existing_output_bytes(
    path: Path,
    expected_stat: os.stat_result,
    label: str,
    *,
    max_bytes: int | None = None,
) -> tuple[bytes | None, list[str]]:
    """Read signer-controlled output bytes without trusting a stale path."""

    chunks: list[bytes] = []
    byte_limit = (
        device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES
        if max_bytes is None
        else max_bytes
    )
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
            signer_output_expected_identity = (
                expected_stat.st_dev,
                expected_stat.st_ino,
            )
            signer_output_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if signer_output_open_identity != signer_output_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != signer_output_expected_identity:
                return None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            if open_stat.st_size > byte_limit:
                return None, [
                    f"{label} must be no more than {byte_limit} bytes"
                ]
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > byte_limit:
                    return None, [
                        f"{label} must be no more than {byte_limit} bytes"
                    ]
                chunks.append(chunk)
            final_path_stat = path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != signer_output_expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} could not be read"]
    return b"".join(chunks), []


def _read_existing_output_text(
    path: Path,
    expected_stat: os.stat_result,
    label: str,
    *,
    max_bytes: int | None = None,
) -> tuple[str | None, list[str]]:
    """Read signer-controlled output text for post-write verification."""

    payload, read_errors = _read_existing_output_bytes(
        path,
        expected_stat,
        label,
        max_bytes=max_bytes,
    )
    if read_errors:
        if read_errors == [f"{label} could not be read"]:
            return None, [f"{label} write verification failed"]
        return None, read_errors
    assert payload is not None
    try:
        return payload.decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [f"{label} write verification failed"]


def _write_json(path: Path, payload: dict[str, Any], label: str) -> list[str]:
    try:
        text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    except ValueError:
        return [f"{label} is not strict JSON"]
    if len(text.encode("utf-8")) > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
        return [
            f"{label} must be no more than "
            f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
        ]
    return _write_text_atomic(
        path,
        text,
        label,
    )


def _write_text(
    path: Path,
    text: str,
    label: str,
    *,
    max_bytes: int | None = None,
) -> list[str]:
    return _write_text_atomic(path, text, label, max_bytes=max_bytes)


def _write_text_atomic(
    path: Path,
    text: str,
    label: str,
    *,
    max_bytes: int | None = None,
) -> list[str]:
    byte_limit = (
        device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES
        if max_bytes is None
        else max_bytes
    )
    errors = _validate_json_output_path(path, label)
    if errors:
        return errors
    try:
        parent_stat = path.parent.lstat()
    except OSError:
        return [f"{label} parent directory metadata could not be read"]
    if stat.S_ISLNK(parent_stat.st_mode) or not stat.S_ISDIR(parent_stat.st_mode):
        return [f"{label} parent directory could not be synced"]
    parent_identity = _file_identity(parent_stat)
    if len(text.encode("utf-8")) > byte_limit:
        return [f"{label} must be no more than {byte_limit} bytes"]
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
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        errors = _validate_json_output_path(path, label)
        if errors:
            write_errors.extend(errors)
        else:
            os.replace(tmp_path, path)
            tmp_path = None
    except OSError:
        write_errors.append(f"{label} could not be written")
    finally:
        if tmp_path is not None:
            write_errors.extend(_cleanup_temp_output(tmp_path, label, tmp_identity))
    if write_errors:
        return write_errors
    errors = _validate_existing_json_output_path(path, label)
    if errors:
        return errors
    sync_errors = _sync_output_parent(
        path.parent,
        label,
        expected_identity=parent_identity,
    )
    if sync_errors:
        return sync_errors
    errors = _validate_existing_json_output_path(path, label)
    if errors:
        return errors
    try:
        expected_stat = path.lstat()
    except (FileNotFoundError, OSError):
        return [f"{label} write verification failed"]
    if stat.S_ISLNK(expected_stat.st_mode):
        return [f"{label} must not be a symlink"]
    if not stat.S_ISREG(expected_stat.st_mode):
        return [f"{label} must be a regular file"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return [f"{label} must not be hardlinked"]
    readback_text, readback_errors = _read_existing_output_text(
        path,
        expected_stat,
        label,
        max_bytes=max_bytes,
    )
    if readback_errors:
        return readback_errors
    if readback_text != text:
        return [f"{label} write verification failed"]
    return []


def _cleanup_temp_output(
    path: Path,
    label: str,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    if expected_identity is None:
        return [f"{label} temporary file metadata could not be read"]
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return [f"{label} temporary file could not be removed"]
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
            return [f"{label} temporary file could not be removed"]
        if (
            not stat.S_ISREG(temp_stat.st_mode)
            or _file_identity(temp_stat) != expected_identity
        ):
            return [f"{label} temporary file changed before cleanup"]
        try:
            os.unlink(path.name, dir_fd=parent_fd)
        except FileNotFoundError:
            return []
        except OSError:
            return [f"{label} temporary file could not be removed"]
    finally:
        os.close(parent_fd)
    return []


def _preflight_slot_metadata_reads(slot_path: Path) -> list[str]:
    """Validate slot paths before any signer-controlled metadata is parsed."""

    path_errors = _validate_slot_path_boundary(slot_path)
    if path_errors:
        return path_errors

    errors: list[str] = []
    device_lab.validate_no_slot_symlink_artifacts(slot_path, errors)
    device_lab.validate_slot_regular_file_artifacts(slot_path, errors)
    device_lab.validate_no_slot_hardlink_artifacts(slot_path, errors)
    return errors


def _validate_slot_path_boundary(slot_path: Path) -> list[str]:
    """Validate signer slot paths before reading mutable slot artifacts."""

    path_errors = device_lab._slot_path_boundary_errors(slot_path)  # type: ignore[attr-defined]
    if path_errors:
        return path_errors
    try:
        slot_mode = slot_path.lstat().st_mode
    except FileNotFoundError:
        slot_mode = None
    except OSError:
        return ["slot directory metadata could not be read"]
    if slot_mode is not None and stat.S_ISLNK(slot_mode):
        return ["slot directory must not be a symlink"]
    try:
        parent_mode = slot_path.parent.lstat().st_mode
    except FileNotFoundError:
        parent_mode = None
    except OSError:
        return ["slot parent directory metadata could not be read"]
    if parent_mode is not None and stat.S_ISLNK(parent_mode):
        return ["slot parent directory must not be a symlink"]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        slot_path,
        "slot ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    if slot_mode is None or not stat.S_ISDIR(slot_mode):
        return ["slot directory missing"]
    return []


def _require_slot_metadata(slot_path: Path) -> tuple[dict[str, Any] | None, list[str]]:
    errors = _preflight_slot_metadata_reads(slot_path)
    if errors:
        return None, errors
    metadata = device_lab._load_json(slot_path / "slot.json", "slot.json", errors)
    if metadata is None:
        return None, errors
    device_lab.validate_slot_metadata_fields(metadata, errors)
    if metadata.get("schema") != "iroha.android.device_lab.kagemusha.v1":
        errors.append("slot.json schema must be iroha.android.device_lab.kagemusha.v1")
    return metadata, errors


def _slot_string(metadata: dict[str, Any], key: str, errors: list[str]) -> str | None:
    value = metadata.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"slot.json {key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(f"slot.json {key} must not contain surrounding whitespace")
        return None
    if device_lab._contains_control_character(value):
        errors.append(f"slot.json {key} must not contain control characters")
        return None
    if device_lab.SECRET_RE.search(value):
        errors.append(f"slot.json {key} must not contain secret-looking material")
        return None
    return value


def _slot_sha256(metadata: dict[str, Any], key: str, errors: list[str]) -> str | None:
    value = metadata.get(key)
    if not isinstance(value, str) or not device_lab.SHA256_HEX_RE.fullmatch(value):
        errors.append(f"slot.json {key} must be lowercase sha256 hex")
        return None
    return value


def _slot_true(metadata: dict[str, Any], key: str, errors: list[str]) -> bool | None:
    if metadata.get(key) is not True:
        errors.append(f"slot.json {key} must be true")
        return None
    return True


def _slot_int(metadata: dict[str, Any], key: str, errors: list[str]) -> int | None:
    value = metadata.get(key)
    if not isinstance(value, int) or isinstance(value, bool):
        if key == "native_bridge_abi_version":
            errors.append("slot.json native_bridge_abi_version must be an integer")
        else:
            errors.append(f"slot.json {key} must be an integer")
        return None
    return value


def _slot_raw_test_commands(metadata: dict[str, Any], errors: list[str]) -> list[str] | None:
    commands = metadata.get("raw_test_commands")
    if not isinstance(commands, list) or not commands:
        errors.append("slot.json raw_test_commands must be a non-empty array")
        return None
    accepted: list[str] = []
    for index, command in enumerate(commands):
        if not isinstance(command, str) or not command.strip():
            errors.append(f"slot.json raw_test_commands[{index}] must be a non-empty string")
            continue
        if command != command.strip():
            errors.append(
                f"slot.json raw_test_commands[{index}] must not contain surrounding whitespace"
            )
            continue
        if device_lab._contains_control_character(command):
            errors.append(
                f"slot.json raw_test_commands[{index}] must not contain control characters"
            )
            continue
        if device_lab.SECRET_RE.search(command):
            errors.append(
                f"slot.json raw_test_commands[{index}] must not contain secret-looking material"
            )
            continue
        accepted.append(command)
    if len(accepted) == len(commands):
        device_lab._validate_raw_test_command_markers(
            commands,
            label="slot.json raw_test_commands",
            errors=errors,
        )
    return accepted if len(accepted) == len(commands) else None


def _validate_metadata_exactness_for_signing(
    metadata: dict[str, Any],
    errors: list[str],
) -> None:
    """Reject metadata that would require normalization before evidence signing."""

    for key in device_lab.SIGNED_EVIDENCE_SLOT_STRING_FIELDS:
        _slot_string(metadata, key, errors)
    _slot_raw_test_commands(metadata, errors)
    chain_relative = metadata.get("attestation_certificate_chain_path")
    if isinstance(chain_relative, str) and chain_relative:
        if chain_relative != chain_relative.strip():
            errors.append(
                "slot.json attestation_certificate_chain_path must not contain surrounding whitespace"
            )
        elif device_lab._contains_control_character(chain_relative):
            errors.append(
                "slot.json attestation_certificate_chain_path must not contain control characters"
            )


def _signer_public_key_sha256(public_key_path: Path, errors: list[str]) -> str | None:
    der = device_lab._openssl_public_key_der(
        public_key_path,
        errors=errors,
        label="signer public key",
    )
    if der is None:
        return None
    return hashlib.sha256(der).hexdigest()


def _sign_ed25519(private_key_path: Path, payload: bytes, errors: list[str]) -> bytes | None:
    secret_error = _secret_key_path_error(private_key_path, "private key")
    if secret_error is not None:
        errors.append(secret_error)
        return None
    try:
        private_key_mode = private_key_path.lstat().st_mode
    except FileNotFoundError:
        private_key_mode = None
    except OSError:
        errors.append("private key file metadata could not be read")
        return None
    if private_key_mode is not None and stat.S_ISLNK(private_key_mode):
        errors.append("private key must not be a symlink")
        return None
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        private_key_path,
        "private key ancestor directory",
    )
    if ancestor_errors:
        errors.extend(ancestor_errors)
        return None
    if private_key_mode is None:
        errors.append("private key must point to an existing file")
        return None
    if not stat.S_ISREG(private_key_mode):
        errors.append("private key must be a regular file")
        return None
    try:
        link_count = private_key_path.stat().st_nlink
    except OSError:
        errors.append("private key hardlink metadata could not be read")
        return None
    if link_count > 1:
        errors.append("private key must not be hardlinked")
        return None
    openssl = device_lab._require_openssl(errors)
    if openssl is None:
        return None
    try:
        with tempfile.TemporaryDirectory(
            prefix="iroha-kagemusha-evidence-sign-"
        ) as temp:
            temp_path = Path(temp)
            payload_path = temp_path / "payload.bin"
            signature_path = temp_path / "signature.bin"
            stage_errors = device_lab._write_staged_bytes(
                payload_path,
                payload,
                write_error="signature payload could not be staged",
                verification_error="signature payload staging verification failed",
            )
            if stage_errors:
                errors.extend(stage_errors)
                return None
            try:
                subprocess.run(
                    [
                        openssl,
                        "pkeyutl",
                        "-sign",
                        "-inkey",
                        str(private_key_path),
                        "-rawin",
                        "-in",
                        str(payload_path),
                        "-out",
                        str(signature_path),
                    ],
                    check=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                )
            except subprocess.CalledProcessError:
                errors.append("private key must be a valid OpenSSL Ed25519 private key")
                return None
            except OSError:
                errors.append("signature command could not be run")
                return None
            signature = _read_signature_output(signature_path, errors)
            if signature is None:
                return None
            if len(signature) != device_lab.ED25519_SIGNATURE_BYTES:
                errors.append("signature output must be 64 bytes")
                return None
            return signature
    except OSError:
        errors.append("signature temporary directory could not be created")
        return None


def _read_signature_output(signature_path: Path, errors: list[str]) -> bytes | None:
    """Read OpenSSL signature output without trusting a stale path."""

    try:
        expected_stat = signature_path.lstat()
    except OSError:
        errors.append("signature output could not be read")
        return None
    if stat.S_ISLNK(expected_stat.st_mode) or not stat.S_ISREG(expected_stat.st_mode):
        errors.append("signature output could not be read")
        return None
    chunks: list[bytes] = []
    signature_output_expected_identity = (
        expected_stat.st_dev,
        expected_stat.st_ino,
    )
    try:
        with signature_path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = signature_path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                errors.append("signature output could not be read")
                return None
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                errors.append("signature output could not be read")
                return None
            signature_output_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if (
                signature_output_open_identity
                != signature_output_expected_identity
                or (path_stat.st_dev, path_stat.st_ino)
                != signature_output_expected_identity
            ):
                errors.append("signature output could not be read")
                return None
            if open_stat.st_nlink > 1:
                errors.append("signature output could not be read")
                return None
            read_limit = device_lab.ED25519_SIGNATURE_BYTES + 1
            size = 0
            while size < read_limit:
                chunk = handle.read(read_limit - size)
                if not chunk:
                    break
                size += len(chunk)
                chunks.append(chunk)
            final_path_stat = signature_path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != signature_output_expected_identity:
                errors.append("signature output could not be read")
                return None
    except OSError:
        errors.append("signature output could not be read")
        return None
    return b"".join(chunks)


def _validate_private_public_pair(
    public_key_path: Path,
    payload: bytes,
    signature: bytes,
    errors: list[str],
) -> None:
    verify_errors: list[str] = []
    device_lab._verify_ed25519_signature(
        public_key_path=public_key_path,
        payload=payload,
        signature=signature,
        errors=verify_errors,
        label="signer public key",
    )
    if verify_errors == ["signed evidence artifact signature verification failed"]:
        errors.append(
            "private key did not produce a signature accepted by the signer public key"
        )
    elif verify_errors:
        errors.extend(verify_errors)


def _normalise_output_path(
    slot_path: Path,
    metadata: dict[str, Any],
    output: str | None,
    errors: list[str],
) -> tuple[Path, str] | None:
    raw_output = output
    if raw_output is None:
        metadata_output = metadata.get("signed_evidence_artifact_path")
        if isinstance(metadata_output, str) and metadata_output:
            if metadata_output != metadata_output.strip():
                errors.append(
                    "slot.json signed_evidence_artifact_path must not contain surrounding whitespace"
                )
                return None
            if device_lab._contains_control_character(metadata_output):
                errors.append(
                    "slot.json signed_evidence_artifact_path must not contain control characters"
                )
                return None
            raw_output = metadata_output
        else:
            raw_output = DEFAULT_SIGNED_EVIDENCE_PATH
    if raw_output != raw_output.strip():
        errors.append("signed evidence output path must not contain surrounding whitespace")
        return None
    if device_lab._contains_control_character(raw_output):
        errors.append("signed evidence output path must not contain control characters")
        return None
    if device_lab.SECRET_RE.search(raw_output):
        errors.append("signed evidence output path must not contain secret-looking material")
        return None
    if "\\" in raw_output:
        errors.append("signed evidence output path must not contain backslashes")
        return None

    candidate = Path(raw_output)
    if candidate.is_absolute():
        if ".." in candidate.parts:
            errors.append("signed evidence output path must be canonical")
            return None
        ancestor_errors = device_lab.validate_no_symlink_ancestors(
            candidate,
            "signed evidence output path ancestor directory",
        )
        if ancestor_errors:
            errors.extend(ancestor_errors)
            return None
        try:
            candidate_mode = candidate.lstat().st_mode
        except FileNotFoundError:
            candidate_mode = None
        except OSError:
            errors.append("signed evidence output path file metadata could not be read")
            return None
        if candidate_mode is not None and stat.S_ISLNK(candidate_mode):
            errors.append("signed evidence output path must not be a symlink")
            return None
        try:
            candidate_resolved = candidate.resolve()
            slot_resolved = slot_path.resolve()
            relative = candidate_resolved.relative_to(slot_resolved).as_posix()
        except OSError:
            errors.append("signed evidence output path could not be resolved")
            return None
        except ValueError:
            errors.append("signed evidence output path must stay inside the slot directory")
            return None
    else:
        relative = device_lab._normalise_safe_relative_path(
            raw_output,
            errors,
            "signed evidence output path",
        )
        if relative is None:
            return None
    if relative in {"slot.json", "sha256sum.txt"}:
        errors.append("signed evidence output path must not overwrite slot metadata")
        return None
    if relative.split("/", 1)[0] != "evidence":
        errors.append("signed evidence output path must stay under evidence/")
        return None
    if relative != DEFAULT_SIGNED_EVIDENCE_PATH:
        errors.append(f"signed evidence output path must be {DEFAULT_SIGNED_EVIDENCE_PATH}")
        return None
    return slot_path / relative, relative


def _attestation_certificate_chain_bytes_for_harness(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> bytes | None:
    chain_relative = metadata.get("attestation_certificate_chain_path")
    if not isinstance(chain_relative, str) or not chain_relative:
        return None
    if chain_relative != chain_relative.strip():
        errors.append(
            "slot.json attestation_certificate_chain_path must not contain surrounding whitespace"
        )
        return None
    if device_lab._contains_control_character(chain_relative):
        errors.append(
            "slot.json attestation_certificate_chain_path must not contain control characters"
        )
        return None
    relative = device_lab._normalise_safe_relative_path(
        chain_relative,
        errors,
        "slot.json attestation_certificate_chain_path",
    )
    if relative is None:
        return None
    if relative.split("/", 1)[0] != "attestation":
        errors.append(
            "slot.json attestation_certificate_chain_path must stay under attestation/"
        )
        return None
    artifact_path, artifact_stat, artifact_errors = _validate_slot_artifact_for_digest(
        slot_path,
        relative,
    )
    if artifact_errors:
        errors.extend(artifact_errors)
        return None
    assert artifact_path is not None and artifact_stat is not None
    chain_bytes, read_errors = _read_validated_slot_artifact_bytes(
        artifact_path,
        artifact_stat,
        relative,
    )
    if read_errors:
        errors.extend(read_errors)
        return None
    return chain_bytes


def _preflight_attestation_harness_result(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> None:
    chain_bytes = _attestation_certificate_chain_bytes_for_harness(
        slot_path,
        metadata,
        errors,
    )
    device_lab.validate_attestation_harness_result(
        slot_path,
        metadata,
        errors,
        attestation_certificate_chain_bytes=chain_bytes,
    )


def _artifact_digests(
    slot_path: Path,
    errors: list[str],
    metadata: dict[str, Any] | None = None,
) -> dict[str, str] | None:
    digests: dict[str, str] = {}
    initial_error_count = len(errors)
    preflight_errors = _preflight_slot_metadata_reads(slot_path)
    if preflight_errors:
        errors.extend(preflight_errors)
        return None
    device_lab.validate_required_kagemusha_slot_artifact_shapes(slot_path, errors)
    if metadata is not None:
        device_lab.validate_attestation_report(slot_path, metadata, errors)
        _preflight_attestation_harness_result(slot_path, metadata, errors)
    if len(errors) != initial_error_count:
        return None
    for relative in device_lab._required_signed_evidence_digest_paths(
        slot_path,
        metadata=metadata,
    ):
        digest, digest_errors = _slot_artifact_sha256(slot_path, relative)
        if digest_errors:
            errors.extend(digest_errors)
            return None
        assert digest is not None
        digests[relative] = digest
    return digests


def _slot_d2d_payment_transcripts(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> dict[str, dict[str, str]] | None:
    primary_relative, primary_digest, primary_transport = (
        device_lab.validate_d2d_payment_transcript_binding(slot_path, metadata, errors)
    )
    if device_lab.D2D_PAYMENT_TRANSCRIPTS_FIELD not in metadata:
        return None
    return device_lab.validate_d2d_payment_transcripts_binding(
        slot_path,
        metadata,
        errors,
        primary_relative=primary_relative,
        primary_digest=primary_digest,
        primary_transport=primary_transport,
    )


def build_signed_evidence(
    slot_path: Path,
    metadata: dict[str, Any],
    *,
    private_key_path: Path,
    public_key_path: Path,
    signer_key_id: str,
    signed_at_utc: str,
    errors: list[str],
    d2d_payment_transcripts: dict[str, dict[str, str]] | None = None,
) -> dict[str, Any] | None:
    """Build, sign, and return the signed evidence JSON object."""

    if not signer_key_id.strip() or device_lab.SECRET_RE.search(signer_key_id):
        errors.append("signer key id must be non-empty and must not contain secret-looking material")
        return None
    if signer_key_id != signer_key_id.strip():
        errors.append("signer key id must not contain surrounding whitespace")
        return None
    if device_lab._contains_control_character(signer_key_id):
        errors.append("signer key id must not contain control characters")
        return None
    signed_at_errors: list[str] = []
    device_lab._validate_signed_at_utc(signed_at_utc, signed_at_errors)
    if signed_at_errors:
        errors.extend(signed_at_errors)
        return None

    evidence: dict[str, Any] = {"schema": device_lab.SIGNED_EVIDENCE_SCHEMA}
    for key in device_lab.SIGNED_EVIDENCE_SLOT_STRING_FIELDS:
        value = _slot_string(metadata, key, errors)
        if value is not None:
            evidence[key] = value
    for key in device_lab.SIGNED_EVIDENCE_SLOT_SHA256_FIELDS:
        value = _slot_sha256(metadata, key, errors)
        if value is not None:
            evidence[key] = value
    for key in device_lab.SIGNED_EVIDENCE_SLOT_INT_FIELDS:
        value = _slot_int(metadata, key, errors)
        if value is not None:
            evidence[key] = value
    for key in device_lab.SIGNED_EVIDENCE_SLOT_TRUE_FIELDS:
        value = _slot_true(metadata, key, errors)
        if value is not None:
            evidence[key] = value

    if (
        d2d_payment_transcripts is None
        and device_lab.D2D_PAYMENT_TRANSCRIPTS_FIELD in metadata
    ):
        d2d_payment_transcripts = _slot_d2d_payment_transcripts(
            slot_path,
            metadata,
            errors,
        )
    if d2d_payment_transcripts is not None:
        evidence[device_lab.D2D_PAYMENT_TRANSCRIPTS_FIELD] = d2d_payment_transcripts

    commands = _slot_raw_test_commands(metadata, errors)
    if commands is not None:
        evidence["raw_test_commands"] = commands
    evidence["signed_at_utc"] = signed_at_utc
    evidence["signer_key_id"] = signer_key_id
    signer_public_key_sha256 = _signer_public_key_sha256(public_key_path, errors)
    if signer_public_key_sha256 is not None:
        evidence["signer_public_key_sha256"] = signer_public_key_sha256
    evidence["signature_algorithm"] = "ed25519"
    artifact_digests = _artifact_digests(slot_path, errors, metadata)
    if artifact_digests is not None:
        evidence["artifact_digests"] = artifact_digests

    if errors:
        return None

    try:
        payload = device_lab._canonical_signed_evidence_payload(evidence)
    except ValueError:
        errors.append("signed evidence payload is not strict JSON")
        return None
    signature = _sign_ed25519(private_key_path, payload, errors)
    if signature is None:
        return None
    _validate_private_public_pair(public_key_path, payload, signature, errors)
    if errors:
        return None
    evidence["signature_payload_sha256"] = hashlib.sha256(payload).hexdigest()
    evidence["signature"] = signature.hex()
    return evidence


def _validate_slot_for_manifest_rewrite(slot_path: Path) -> list[str]:
    """Validate a slot immediately before rewriting its SHA-256 manifest."""

    path_errors = _validate_slot_path_boundary(slot_path)
    if path_errors:
        return path_errors

    errors: list[str] = []
    device_lab.validate_no_slot_symlink_artifacts(slot_path, errors)
    device_lab.validate_slot_regular_file_artifacts(slot_path, errors)
    device_lab.validate_no_slot_hardlink_artifacts(slot_path, errors)
    if errors:
        return errors
    slot_files = device_lab._slot_files(slot_path, errors)
    if errors:
        return errors
    for relative in slot_files:
        if device_lab.SECRET_RE.search(relative):
            errors.append("slot artifacts must not contain secret-looking material")
            return errors
    return []


def _validate_slot_artifact_for_digest(
    slot_path: Path,
    relative: str,
) -> tuple[Path | None, os.stat_result | None, list[str]]:
    """Validate one slot artifact immediately before hashing it."""

    path_errors = device_lab._slot_path_boundary_errors(slot_path)  # type: ignore[attr-defined]
    if path_errors:
        return None, None, path_errors
    if device_lab.SECRET_RE.search(relative):
        return None, None, ["slot artifacts must not contain secret-looking material"]
    normalise_errors: list[str] = []
    safe_relative = device_lab._normalise_safe_relative_path(
        relative,
        normalise_errors,
        "slot artifact path",
    )
    if normalise_errors:
        return None, None, normalise_errors
    assert safe_relative is not None
    display = device_lab._display_path(safe_relative)
    artifact_path = slot_path / safe_relative
    symlink_ancestor = device_lab._slot_relative_symlink_ancestor(
        slot_path,
        safe_relative,
    )
    if symlink_ancestor is not None:
        return None, None, [
            f"slot artifact {display} ancestor directory must not be a symlink"
        ]
    try:
        artifact_stat = artifact_path.lstat()
    except FileNotFoundError:
        return None, None, [f"slot artifact {display} is missing"]
    except OSError:
        return None, None, [f"slot artifact {display} file metadata could not be read"]
    if stat.S_ISLNK(artifact_stat.st_mode):
        return None, None, [f"slot artifact {display} must not be a symlink"]
    if not stat.S_ISREG(artifact_stat.st_mode):
        return None, None, [f"slot artifact {display} must be a regular file"]
    try:
        link_count = artifact_path.stat().st_nlink
    except OSError:
        return None, None, [
            f"slot artifact {display} hardlink metadata could not be read"
        ]
    if link_count > 1:
        return None, None, [f"slot artifact {display} must not be hardlinked"]
    return artifact_path, artifact_stat, []


def _read_validated_slot_artifact_bytes(
    artifact_path: Path,
    expected_stat: os.stat_result,
    relative: str,
    max_bytes: int | None = None,
) -> tuple[bytes | None, list[str]]:
    """Read a signer slot artifact without trusting a stale path."""

    display = device_lab._display_path(relative)
    artifact_max_bytes = (
        device_lab._slot_artifact_max_bytes(relative)
        if max_bytes is None
        else max_bytes
    )
    chunks: list[bytes] = []
    try:
        with artifact_path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = artifact_path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [f"slot artifact {display} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, [f"slot artifact {display} must be a regular file"]
            signer_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            signer_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if signer_open_identity != signer_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != signer_expected_identity:
                return None, [f"slot artifact {display} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, [f"slot artifact {display} must not be hardlinked"]
            if open_stat.st_size > artifact_max_bytes:
                return None, [
                    f"slot artifact {display} must be no more than "
                    f"{artifact_max_bytes} bytes"
                ]
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > artifact_max_bytes:
                    return None, [
                        f"slot artifact {display} must be no more than "
                        f"{artifact_max_bytes} bytes"
                    ]
                chunks.append(chunk)
            final_path_stat = artifact_path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != signer_expected_identity:
                return None, [f"slot artifact {display} changed while being read"]
    except OSError:
        return None, [f"slot artifact {display} could not be read"]
    return b"".join(chunks), []


def _slot_artifact_sha256(slot_path: Path, relative: str) -> tuple[str | None, list[str]]:
    artifact_path, artifact_stat, errors = _validate_slot_artifact_for_digest(
        slot_path,
        relative,
    )
    if errors:
        return None, errors
    assert artifact_path is not None and artifact_stat is not None
    payload, read_errors = _read_validated_slot_artifact_bytes(
        artifact_path,
        artifact_stat,
        relative,
    )
    if read_errors:
        return None, read_errors
    assert payload is not None
    return hashlib.sha256(payload).hexdigest(), []


def rewrite_sha256_manifest(slot_path: Path) -> list[str]:
    """Rewrite sha256sum.txt so it exactly covers current slot artifacts."""

    errors = _validate_slot_for_manifest_rewrite(slot_path)
    if errors:
        return errors
    lines = []
    slot_files = device_lab._slot_files(slot_path, errors)
    if errors:
        return errors
    for relative in sorted(slot_files):
        digest, digest_errors = _slot_artifact_sha256(slot_path, relative)
        if digest_errors:
            return digest_errors
        assert digest is not None
        lines.append(f"{digest}  {relative}")
    manifest_text = "\n".join(lines) + "\n"
    return _write_text(
        slot_path / "sha256sum.txt",
        manifest_text,
        "sha256sum.txt",
        max_bytes=device_lab.MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES,
    )


def sign_slot_evidence(
    *,
    slot_path: Path,
    private_key_path: Path,
    public_key_path: Path,
    signer_key_id: str,
    signed_at_utc: str,
    output: str | None,
    update_slot_json: bool,
    update_sha256sum: bool,
) -> tuple[int, str | None, list[str]]:
    """Sign one slot and return status, artifact path, and errors."""

    runtime_arg_errors = [
        error
        for error in (
            *device_lab._slot_path_boundary_errors(slot_path),  # type: ignore[attr-defined]
            _secret_key_path_error(private_key_path, "private key"),
            _secret_key_path_error(public_key_path, "signer public key"),
            (
                "signed evidence output path must not contain secret-looking material"
                if output is not None and device_lab.SECRET_RE.search(output)
                else None
            ),
            (
                "signed evidence output path must not contain control characters"
                if output is not None and device_lab._contains_control_character(output)
                else None
            ),
            (
                "signer key id must be non-empty and must not contain secret-looking material"
                if not signer_key_id.strip() or device_lab.SECRET_RE.search(signer_key_id)
                else None
            ),
            (
                "signer key id must not contain surrounding whitespace"
                if signer_key_id and signer_key_id != signer_key_id.strip()
                else None
            ),
            (
                "signer key id must not contain control characters"
                if device_lab._contains_control_character(signer_key_id)
                else None
            ),
        )
        if error is not None
    ]
    if runtime_arg_errors:
        return 1, None, runtime_arg_errors

    metadata, errors = _require_slot_metadata(slot_path)
    if metadata is None:
        return 1, None, errors
    output_pair = _normalise_output_path(slot_path, metadata, output, errors)
    if output_pair is None:
        return 1, None, errors
    output_path, output_relative = output_pair
    _validate_metadata_exactness_for_signing(metadata, errors)
    if errors:
        return 1, None, errors
    device_lab.validate_no_slot_symlink_artifacts(slot_path, errors)
    device_lab.validate_slot_regular_file_artifacts(slot_path, errors)
    device_lab.validate_no_slot_hardlink_artifacts(slot_path, errors)
    device_lab.validate_attestation_result(slot_path, metadata, errors)
    device_lab.validate_attestation_report(slot_path, metadata, errors)
    d2d_payment_transcripts = _slot_d2d_payment_transcripts(
        slot_path,
        metadata,
        errors,
    )
    device_lab.validate_wallet_integrity_transcript_binding(slot_path, metadata, errors)
    if errors:
        return 1, None, errors
    evidence = build_signed_evidence(
        slot_path,
        metadata,
        private_key_path=private_key_path,
        public_key_path=public_key_path,
        signer_key_id=signer_key_id,
        signed_at_utc=signed_at_utc,
        errors=errors,
        d2d_payment_transcripts=d2d_payment_transcripts,
    )
    if evidence is None:
        return 1, None, errors

    write_errors = _write_json(output_path, evidence, "signed evidence output path")
    if write_errors:
        return 1, None, write_errors
    artifact_digest, digest_errors = _output_file_sha256(
        output_path,
        "signed evidence output path",
    )
    if digest_errors:
        return 1, output_relative, digest_errors
    assert artifact_digest is not None
    if update_slot_json:
        metadata["signed_evidence_artifact_path"] = output_relative
        metadata["signed_evidence_artifact_sha256"] = artifact_digest
        write_errors = _write_json(slot_path / "slot.json", metadata, "slot.json")
        if write_errors:
            return 1, output_relative, write_errors
    if update_sha256sum:
        write_errors = rewrite_sha256_manifest(slot_path)
        if write_errors:
            return 1, output_relative, write_errors

    trusted = {}
    trusted_errors: list[str] = []
    public_der = device_lab._openssl_public_key_der(
        public_key_path,
        errors=trusted_errors,
        label="signer public key",
    )
    if public_der is not None:
        trusted[hashlib.sha256(public_der).hexdigest()] = public_key_path
    validation_errors, _details = device_lab.validate_kagemusha_production_metadata(
        slot_path,
        trusted,
    )
    if trusted_errors or validation_errors:
        return 1, output_relative, trusted_errors + validation_errors
    return 0, output_relative, []


def default_signed_at_utc() -> str:
    """Return a canonical UTC timestamp for signatures."""

    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace(
        "+00:00",
        "Z",
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Build and sign a Kagemusha Android device-lab evidence artifact."
    )
    parser.add_argument("--slot", required=True, help="Device-lab slot directory.")
    parser.add_argument(
        "--private-key",
        required=True,
        help="Runtime-only OpenSSL Ed25519 private key used for signing.",
    )
    parser.add_argument(
        "--public-key",
        required=True,
        help="OpenSSL Ed25519 public key pinned by production validation.",
    )
    parser.add_argument("--signer-key-id", required=True, help="Stable lab signer key id.")
    parser.add_argument(
        "--signed-at-utc",
        default=None,
        help="ISO-8601 UTC signing timestamp. Defaults to the current UTC second.",
    )
    parser.add_argument(
        "--output",
        default=None,
        help="Signed evidence output path, relative to the slot by default.",
    )
    parser.add_argument(
        "--no-update-slot-json",
        action="store_true",
        help="Do not refresh signed_evidence_artifact_* fields in slot.json.",
    )
    parser.add_argument(
        "--no-update-sha256sum",
        action="store_true",
        help="Do not rewrite sha256sum.txt after writing evidence.",
    )
    args = parser.parse_args(argv)

    status, output_relative, errors = sign_slot_evidence(
        slot_path=Path(args.slot),
        private_key_path=Path(args.private_key),
        public_key_path=Path(args.public_key),
        signer_key_id=args.signer_key_id,
        signed_at_utc=args.signed_at_utc or default_signed_at_utc(),
        output=args.output,
        update_slot_json=not args.no_update_slot_json,
        update_sha256sum=not args.no_update_sha256sum,
    )
    if status != 0:
        for error in errors:
            print(f"[device-lab-sign] {error}", file=sys.stderr)
        return status
    print(f"[device-lab-sign] wrote signed evidence {output_relative}")
    return 0


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
