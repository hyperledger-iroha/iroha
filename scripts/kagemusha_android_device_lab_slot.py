#!/usr/bin/env python3
"""Assemble a signed Kagemusha Android device-lab slot from lab artifacts."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
from pathlib import PurePosixPath
import shutil
import stat
import subprocess
import sys
import tempfile
from typing import Any, Iterable, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402
import sign_android_device_lab_evidence as evidence_signer  # noqa: E402


DEFAULT_APP_PACKAGE_NAME = "org.hyperledger.iroha.sdk.offline.wallet.lab"
DEFAULT_ATTESTATION_HARNESS_RESULT_PATH = "attestation/harness-result.json"
DEFAULT_ATTESTATION_CHAIN_PATH = "attestation/keymint-certificate-chain.pem"
DEFAULT_OFFLINE_WALLET_APK_PATH = "evidence/offline-wallet-release.apk"
DEFAULT_D2D_TRANSCRIPT_PATH = "handoff/d2d-payment-transcript.json"
DEFAULT_WALLET_TRANSCRIPT_PATH = "wallet/wallet-integrity-transcript.json"
PRIMARY_D2D_PAYMENT_TRANSPORT = "nearby_offline"
ATTESTATION_REPORT_DEVICE_FINGERPRINT_MISMATCH = (
    "attestation/report.json device_fingerprint must match device identity"
)
ATTESTATION_REPORT_OS_BUILD_MISMATCH = (
    "attestation/report.json os_build_id must match device identity"
)
WALLET_ROLLBACK_REQUIRED = (
    "wallet integrity transcript rollback_rejection_passed must be true"
)
MAX_ADB_COMMAND_DISPLAY_CHARS = 240
ADB_SERIAL_REDACTION = "<redacted-adb-serial>"
DEFAULT_ADB_TIMEOUT_SECONDS = 120
DISRUPTIVE_EXECUTABLE_NAMES = frozenset(
    ("halt", "kill", "killall", "pkill", "poweroff", "reboot", "shutdown")
)
DISRUPTIVE_COMMAND_TOKENS = frozenset(
    (
        "kill-server",
        "reconnect",
        "disconnect",
        "reboot",
        "root",
        "unroot",
        "remount",
        "shutdown",
        "poweroff",
        "halt",
        "uninstall",
    )
)
DISRUPTIVE_TOKEN_SEQUENCES: tuple[tuple[str, ...], ...] = (
    ("am", "kill"),
    ("am", "kill-all"),
    ("am", "force-stop"),
    ("cmd", "activity", "kill"),
    ("cmd", "activity", "kill-all"),
    ("cmd", "activity", "force-stop"),
    ("cmd", "activity", "stop-app"),
    ("pm", "clear"),
    ("pm", "disable"),
    ("pm", "disable-user"),
    ("pm", "enable"),
    ("pm", "grant"),
    ("pm", "revoke"),
    ("pm", "reset-permissions"),
    ("pm", "suspend"),
    ("pm", "unsuspend"),
    ("cmd", "package", "clear"),
    ("cmd", "package", "disable"),
    ("cmd", "package", "disable-user"),
    ("cmd", "package", "enable"),
    ("cmd", "package", "grant"),
    ("cmd", "package", "revoke"),
    ("cmd", "package", "reset-permissions"),
    ("cmd", "package", "suspend"),
    ("cmd", "package", "unsuspend"),
    ("appops", "set"),
    ("appops", "reset"),
    ("cmd", "appops", "set"),
    ("cmd", "appops", "reset"),
    ("emu", "kill"),
    ("shell", "stop"),
    ("shell", "start"),
    ("setprop", "ctl.stop"),
    ("setprop", "ctl.restart"),
    ("setprop", "ctl.start"),
    ("setprop", "sys.powerctl"),
)

DEVICE_FAMILY_MODEL_RULES: tuple[
    tuple[str, tuple[str, ...], tuple[str, ...], tuple[str, ...]], ...
] = (
    ("Google Pixel 6 / 6a", ("pixel 6", "pixel 6a"), ("oriole", "bluejay"), ()),
    ("Google Pixel 7 / 7 Pro", ("pixel 7", "pixel 7 pro"), ("panther", "cheetah"), ()),
    (
        "Google Pixel 8 / 8a / 8 Pro",
        ("pixel 8", "pixel 8a", "pixel 8 pro"),
        ("shiba", "akita", "husky"),
        (),
    ),
    (
        "Google Pixel Fold / Tablet",
        ("pixel fold", "pixel tablet"),
        ("felix", "tangorpro"),
        (),
    ),
    (
        "Samsung Galaxy S23",
        ("galaxy s23", "galaxy s23+", "galaxy s23 ultra"),
        ("dm1q", "dm2q", "dm3q"),
        ("sm-s911", "sm-s916", "sm-s918"),
    ),
    (
        "Samsung Galaxy S24",
        ("galaxy s24", "galaxy s24+", "galaxy s24 ultra"),
        ("e1q", "e2q", "e3q"),
        ("sm-s921", "sm-s926", "sm-s928"),
    ),
)


def _timeout_arg(timeout_seconds: int) -> int | None:
    return None if timeout_seconds == 0 else timeout_seconds


def _json_dumps(payload: dict[str, Any]) -> str:
    return json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"


def _is_adb_executable(command: Sequence[str]) -> bool:
    if not command:
        return False
    executable = str(command[0]).replace("\\", "/").rsplit("/", 1)[-1].lower()
    return executable in {"adb", "adb.exe"}


def _safe_adb_command_display(command: Sequence[str]) -> str:
    display_tokens = [str(token) for token in command]
    if _is_adb_executable(display_tokens):
        for index, token in enumerate(display_tokens[:-1]):
            if token == "-s":
                display_tokens[index + 1] = ADB_SERIAL_REDACTION
    rendered = " ".join(display_tokens)
    if device_lab.SECRET_RE.search(rendered):
        return "<redacted-adb-command>"
    if device_lab._contains_control_character(rendered):
        return "<unsafe-adb-command>"
    if len(rendered) > MAX_ADB_COMMAND_DISPLAY_CHARS:
        return f"{rendered[:MAX_ADB_COMMAND_DISPLAY_CHARS]}..."
    return rendered


def _command_disruption_errors(command: Sequence[str], label: str) -> list[str]:
    if not command:
        return [f"{label} command must not be empty"]
    tokens = [str(token) for token in command]
    executable = Path(tokens[0]).name
    if executable in DISRUPTIVE_EXECUTABLE_NAMES:
        return [
            f"{label} must not manage other running jobs: "
            f"{_safe_adb_command_display(tokens)}"
        ]
    if any(token in DISRUPTIVE_COMMAND_TOKENS for token in tokens):
        return [
            f"{label} must not manage other running jobs: "
            f"{_safe_adb_command_display(tokens)}"
        ]
    for sequence in DISRUPTIVE_TOKEN_SEQUENCES:
        width = len(sequence)
        for index in range(0, len(tokens) - width + 1):
            if tuple(tokens[index : index + width]) == sequence:
                return [
                    f"{label} must not manage other running jobs: "
                    f"{_safe_adb_command_display(tokens)}"
                ]
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


def _sync_directory(
    path: Path,
    label: str,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    try:
        dir_fd = os.open(path, _directory_open_flags())
    except OSError:
        return [f"{label} parent directory could not be synced"]
    try:
        return _sync_directory_fd(
            dir_fd,
            label,
            expected_identity=expected_identity,
        )
    finally:
        os.close(dir_fd)


def _sync_directory_fd(
    dir_fd: int,
    label: str,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    try:
        dir_stat = os.fstat(dir_fd)
        if not stat.S_ISDIR(dir_stat.st_mode):
            return [f"{label} parent directory could not be synced"]
        if expected_identity is not None and _file_identity(dir_stat) != expected_identity:
            return [f"{label} parent directory changed before sync"]
        os.fsync(dir_fd)
    except OSError:
        return [f"{label} parent directory could not be synced"]
    return []


def _write_all(file_fd: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        written = os.write(file_fd, view)
        if written <= 0:
            raise OSError("short write")
        view = view[written:]


def _verify_written_bytes(path: Path, expected_bytes: bytes, label: str) -> list[str]:
    try:
        expected_stat = path.lstat()
    except OSError:
        return [f"{label} metadata could not be read after write"]
    if stat.S_ISLNK(expected_stat.st_mode) or not stat.S_ISREG(expected_stat.st_mode):
        return [f"{label} changed after write"]
    if stat.S_IMODE(expected_stat.st_mode) != 0o600:
        return [f"{label} permissions must be 0600"]
    expected_identity = _file_identity(expected_stat)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if (
                _file_identity(open_stat) != expected_identity
                or _file_identity(path_stat) != expected_identity
            ):
                return [f"{label} changed after write"]
            data = handle.read(len(expected_bytes) + 1)
            final_stat = path.lstat()
            if _file_identity(final_stat) != expected_identity or data != expected_bytes:
                return [f"{label} changed after write"]
    except OSError:
        return [f"{label} could not be verified after write"]
    return []


def _verify_copied_file(
    path: Path,
    *,
    expected_digest: str,
    expected_size: int,
    label: str,
    max_bytes: int,
) -> list[str]:
    try:
        expected_stat = path.lstat()
    except OSError:
        return [f"{label} metadata could not be read after write"]
    if stat.S_ISLNK(expected_stat.st_mode) or not stat.S_ISREG(expected_stat.st_mode):
        return [f"{label} changed after write"]
    if stat.S_IMODE(expected_stat.st_mode) != 0o600:
        return [f"{label} permissions must be 0600"]
    if expected_stat.st_nlink > 1:
        return [f"{label} must not be hardlinked"]
    expected_identity = _file_identity(expected_stat)
    digest = hashlib.sha256()
    size = 0
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if (
                _file_identity(open_stat) != expected_identity
                or _file_identity(path_stat) != expected_identity
            ):
                return [f"{label} changed after write"]
            if not stat.S_ISREG(open_stat.st_mode) or open_stat.st_nlink > 1:
                return [f"{label} changed after write"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > max_bytes:
                    return [f"{label} must not exceed {max_bytes} bytes"]
                digest.update(chunk)
            final_stat = path.lstat()
            if _file_identity(final_stat) != expected_identity:
                return [f"{label} changed after write"]
    except OSError:
        return [f"{label} could not be verified after write"]
    if size != expected_size or digest.hexdigest() != expected_digest:
        return [f"{label} changed after write"]
    return []


def _cleanup_temp_output(
    path: Path,
    label: str,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    if expected_identity is None:
        return [f"{label} temporary output metadata could not be read"]
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return [f"{label} temporary output could not be removed"]
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
            return [f"{label} temporary output could not be removed"]
        if (
            not stat.S_ISREG(temp_stat.st_mode)
            or _file_identity(temp_stat) != expected_identity
        ):
            return [f"{label} temporary output changed before cleanup"]
        try:
            os.unlink(path.name, dir_fd=parent_fd)
        except FileNotFoundError:
            return []
        except OSError:
            return [f"{label} temporary output could not be removed"]
        try:
            os.fsync(parent_fd)
        except OSError:
            return [f"{label} temporary output cleanup could not be synced"]
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
        output_stat = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return []
    except OSError:
        return [f"{label} rollback cleanup metadata could not be read"]
    if not stat.S_ISREG(output_stat.st_mode) or _file_identity(output_stat) != expected_identity:
        return []
    try:
        os.unlink(name, dir_fd=parent_fd)
    except FileNotFoundError:
        return []
    except OSError:
        return [f"{label} rollback cleanup could not remove file"]
    try:
        os.fsync(parent_fd)
    except OSError:
        return [f"{label} rollback cleanup could not be synced"]
    return []


def _write_json(path: Path, payload: dict[str, Any], label: str) -> list[str]:
    try:
        encoded = _json_dumps(payload).encode("utf-8")
    except (TypeError, ValueError):
        return [f"{label} is not strict JSON"]
    try:
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    except OSError:
        return [f"{label} parent directory could not be created"]
    permission_errors = _set_private_directory_permissions(
        path.parent,
        f"{label} parent directory",
    )
    if permission_errors:
        return permission_errors
    try:
        json_parent_stat = path.parent.lstat()
    except OSError:
        return [f"{label} parent metadata could not be read"]
    if stat.S_ISLNK(json_parent_stat.st_mode) or not stat.S_ISDIR(json_parent_stat.st_mode):
        return [f"{label} parent directory could not be synced"]
    json_parent_identity = _file_identity(json_parent_stat)
    tmp_path = path.parent / f".{path.name}.android-slot.tmp"
    tmp_identity: tuple[int, int] | None = None
    parent_fd: int | None = None
    try:
        try:
            parent_fd = os.open(path.parent, _directory_open_flags())
        except OSError:
            return [f"{label} parent directory could not be synced"]
        try:
            parent_fd_stat = os.fstat(parent_fd)
        except OSError:
            return [f"{label} parent directory could not be synced"]
        if (
            not stat.S_ISDIR(parent_fd_stat.st_mode)
            or _file_identity(parent_fd_stat) != json_parent_identity
        ):
            return [f"{label} parent directory changed before sync"]
        if tmp_path.exists() or tmp_path.is_symlink():
            return [f"{label} temporary output already exists"]
        with tmp_path.open("xb") as handle:
            tmp_identity = _file_identity(os.fstat(handle.fileno()))
            os.fchmod(handle.fileno(), 0o600)
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(
            tmp_path.name,
            path.name,
            src_dir_fd=parent_fd,
            dst_dir_fd=parent_fd,
        )
        tmp_path = None
        try:
            installed_stat = os.stat(
                path.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except OSError:
            return [f"{label} metadata could not be read after write"]
        installed_identity = _file_identity(installed_stat)
        try:
            current_parent_stat = path.parent.lstat()
        except OSError:
            cleanup_errors = _unlink_file_if_identity_at(
                parent_fd,
                path.name,
                installed_identity,
                label=label,
            )
            return [
                f"{label} parent directory metadata could not be read",
                *cleanup_errors,
            ]
        if _file_identity(current_parent_stat) != json_parent_identity:
            cleanup_errors = _unlink_file_if_identity_at(
                parent_fd,
                path.name,
                installed_identity,
                label=label,
            )
            return [f"{label} parent directory changed before sync", *cleanup_errors]
        sync_errors = _sync_directory_fd(
            parent_fd,
            label,
            expected_identity=json_parent_identity,
        )
        if sync_errors:
            cleanup_errors = _unlink_file_if_identity_at(
                parent_fd,
                path.name,
                installed_identity,
                label=label,
            )
            return [*sync_errors, *cleanup_errors]
    except OSError:
        cleanup_errors = _cleanup_temp_output(tmp_path, label, tmp_identity)
        return [f"{label} could not be written", *cleanup_errors]
    finally:
        if parent_fd is not None:
            os.close(parent_fd)
    return _verify_written_bytes(path, encoded, label)


def _publish_stage_slot(
    *,
    stage_slot: Path,
    root: Path,
    slot_id: str,
    expected_root_identity: tuple[int, int],
    expected_temp_parent_identity: tuple[int, int],
    expected_stage_identity: tuple[int, int],
) -> list[str]:
    try:
        root_fd = os.open(root, _directory_open_flags())
    except OSError:
        return ["slot root directory could not be synced"]
    try:
        root_stat = os.fstat(root_fd)
        if not stat.S_ISDIR(root_stat.st_mode):
            return ["slot root directory could not be synced"]
        if stat.S_IMODE(root_stat.st_mode) != 0o700:
            return ["slot root directory permissions must be 0700"]
        if _file_identity(root_stat) != expected_root_identity:
            return ["slot root directory changed before publish"]
        try:
            os.stat(slot_id, dir_fd=root_fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        except OSError:
            return ["slot directory metadata could not be read before publish"]
        else:
            return ["slot directory already exists; refuse to overwrite evidence"]

        try:
            temp_parent_fd = os.open(stage_slot.parent, _directory_open_flags())
        except OSError:
            return ["staged slot parent directory could not be opened"]
        try:
            temp_parent_stat = os.fstat(temp_parent_fd)
            if not stat.S_ISDIR(temp_parent_stat.st_mode):
                return ["staged slot parent directory could not be opened"]
            if stat.S_IMODE(temp_parent_stat.st_mode) != 0o700:
                return ["staged slot parent directory permissions must be 0700"]
            if _file_identity(temp_parent_stat) != expected_temp_parent_identity:
                return ["staged slot parent directory changed before publish"]
            stage_stat = os.stat(
                stage_slot.name,
                dir_fd=temp_parent_fd,
                follow_symlinks=False,
            )
            if not stat.S_ISDIR(stage_stat.st_mode):
                return ["staged slot must be a directory"]
            if stat.S_IMODE(stage_stat.st_mode) != 0o700:
                return ["staged slot directory permissions must be 0700"]
            if _file_identity(stage_stat) != expected_stage_identity:
                return ["staged slot directory changed before publish"]
            os.rename(
                stage_slot.name,
                slot_id,
                src_dir_fd=temp_parent_fd,
                dst_dir_fd=root_fd,
            )
            os.fsync(root_fd)
        except OSError:
            return ["slot directory could not be published"]
        finally:
            os.close(temp_parent_fd)
    finally:
        os.close(root_fd)
    return []


def _cleanup_temp_parent(
    temp_parent: Path,
    *,
    expected_identity: tuple[int, int],
) -> list[str]:
    try:
        parent_fd = os.open(temp_parent.parent, _directory_open_flags())
    except OSError:
        return ["staged slot temporary directory cleanup parent could not be opened"]
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
            return ["staged slot temporary directory metadata could not be read"]
        if (
            not stat.S_ISDIR(temp_parent_stat.st_mode)
            or _file_identity(temp_parent_stat) != expected_identity
        ):
            return []
        try:
            shutil.rmtree(temp_parent.name, dir_fd=parent_fd)
        except OSError:
            return ["staged slot temporary directory could not be removed"]
        try:
            os.fsync(parent_fd)
        except OSError:
            return ["staged slot temporary directory cleanup could not be synced"]
    finally:
        os.close(parent_fd)
    return []


def _sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _single_safe_slot_id(slot_id: str) -> str | None:
    candidate = PurePosixPath(slot_id)
    if (
        not slot_id
        or any(character.isspace() for character in slot_id)
        or device_lab._contains_control_character(slot_id)
        or device_lab.SECRET_RE.search(slot_id)
        or candidate.is_absolute()
        or "\\" in slot_id
        or len(candidate.parts) != 1
        or candidate.name in {"", ".", ".."}
        or ".." in candidate.parts
    ):
        return None
    if candidate.as_posix() != slot_id:
        return None
    return candidate.name


def _normalise_source_path(
    path: Path,
    label: str,
    errors: list[str],
) -> tuple[Path, os.stat_result] | None:
    path_text = str(path)
    if device_lab.SECRET_RE.search(path_text):
        errors.append(f"{label} path must not contain secret-looking material")
        return None
    if device_lab._contains_control_character(path_text):
        errors.append(f"{label} path must not contain control characters")
        return None
    if path_text != path_text.strip() or device_lab._path_has_surrounding_whitespace_component(  # type: ignore[attr-defined]
        path
    ):
        errors.append(f"{label} path must not contain surrounding whitespace")
        return None
    if "\\" in path_text:
        errors.append(f"{label} path must not contain backslashes")
        return None
    if ".." in path.parts:
        errors.append(f"{label} path must be canonical")
        return None
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        errors.extend(ancestor_errors)
        return None
    try:
        file_stat = path.lstat()
    except FileNotFoundError:
        errors.append(f"{label} is missing")
        return None
    except OSError:
        errors.append(f"{label} metadata could not be read")
        return None
    if stat.S_ISLNK(file_stat.st_mode):
        errors.append(f"{label} must not be a symlink")
        return None
    if not stat.S_ISREG(file_stat.st_mode):
        errors.append(f"{label} must be a regular file")
        return None
    try:
        link_count = path.stat().st_nlink
    except OSError:
        errors.append(f"{label} hardlink metadata could not be read")
        return None
    if link_count > 1:
        errors.append(f"{label} must not be hardlinked")
        return None
    return path, file_stat


def _copy_source_file(
    *,
    source: Path,
    destination: Path,
    label: str,
    errors: list[str],
    max_bytes: int = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES,
) -> str | None:
    normalised = _normalise_source_path(source, label, errors)
    if normalised is None:
        return None
    source_path, expected_stat = normalised
    digest = hashlib.sha256()
    size = 0
    try:
        with source_path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = source_path.lstat()
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            path_identity = (path_stat.st_dev, path_stat.st_ino)
            if open_identity != expected_identity or path_identity != expected_identity:
                errors.append(f"{label} changed while being read")
                return None
            if not stat.S_ISREG(open_stat.st_mode) or not stat.S_ISREG(path_stat.st_mode):
                errors.append(f"{label} must be a regular file")
                return None
            if open_stat.st_nlink > 1:
                errors.append(f"{label} must not be hardlinked")
                return None
            destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            permission_errors = _set_private_directory_permissions(
                destination.parent,
                f"{label} destination parent directory",
            )
            if permission_errors:
                errors.extend(permission_errors)
                return None
            try:
                destination_parent_stat = destination.parent.lstat()
            except OSError:
                errors.append(f"{label} destination parent metadata could not be read")
                return None
            if stat.S_ISLNK(destination_parent_stat.st_mode) or not stat.S_ISDIR(
                destination_parent_stat.st_mode
            ):
                errors.append(f"{label} destination parent directory could not be synced")
                return None
            destination_parent_identity = _file_identity(destination_parent_stat)
            destination_identity: tuple[int, int] | None = None
            try:
                destination_parent_fd = os.open(
                    destination.parent,
                    _directory_open_flags(),
                )
            except OSError:
                errors.append(f"{label} destination parent directory could not be synced")
                return None
            try:
                try:
                    destination_parent_fd_stat = os.fstat(destination_parent_fd)
                except OSError:
                    errors.append(f"{label} destination parent directory could not be synced")
                    return None
                if (
                    not stat.S_ISDIR(destination_parent_fd_stat.st_mode)
                    or _file_identity(destination_parent_fd_stat)
                    != destination_parent_identity
                ):
                    errors.append(f"{label} destination parent directory changed before sync")
                    return None
                out_fd = os.open(
                    destination.name,
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL,
                    0o600,
                    dir_fd=destination_parent_fd,
                )
                try:
                    os.fchmod(out_fd, 0o600)
                    try:
                        destination_identity = _file_identity(os.fstat(out_fd))
                    except OSError:
                        errors.append(f"{label} metadata could not be read after write")
                        return None
                    for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                        size += len(chunk)
                        if size > max_bytes:
                            errors.append(f"{label} must not exceed {max_bytes} bytes")
                            return None
                        digest.update(chunk)
                        _write_all(out_fd, chunk)
                    os.fsync(out_fd)
                finally:
                    os.close(out_fd)
                final_stat = source_path.lstat()
                if (final_stat.st_dev, final_stat.st_ino) != expected_identity:
                    errors.append(f"{label} changed while being read")
                    return None
                try:
                    current_destination_parent_stat = destination.parent.lstat()
                except OSError:
                    cleanup_errors: list[str] = []
                    if destination_identity is not None:
                        cleanup_errors = _unlink_file_if_identity_at(
                            destination_parent_fd,
                            destination.name,
                            destination_identity,
                            label=label,
                        )
                    errors.extend(
                        [
                            f"{label} destination parent metadata could not be read",
                            *cleanup_errors,
                        ]
                    )
                    return None
                if (
                    _file_identity(current_destination_parent_stat)
                    != destination_parent_identity
                ):
                    cleanup_errors = []
                    if destination_identity is not None:
                        cleanup_errors = _unlink_file_if_identity_at(
                            destination_parent_fd,
                            destination.name,
                            destination_identity,
                            label=label,
                        )
                    errors.extend(
                        [
                            f"{label} destination parent directory changed before sync",
                            *cleanup_errors,
                        ]
                    )
                    return None
                sync_errors = _sync_directory_fd(
                    destination_parent_fd,
                    label,
                    expected_identity=destination_parent_identity,
                )
                if sync_errors:
                    cleanup_errors: list[str] = []
                    if destination_identity is not None:
                        cleanup_errors = _unlink_file_if_identity_at(
                            destination_parent_fd,
                            destination.name,
                            destination_identity,
                            label=label,
                        )
                    errors.extend([*sync_errors, *cleanup_errors])
                    return None
            finally:
                os.close(destination_parent_fd)
    except OSError:
        errors.append(f"{label} could not be read")
        return None
    if size <= 0:
        errors.append(f"{label} must be non-empty")
        return None
    copied_digest = digest.hexdigest()
    verify_errors = _verify_copied_file(
        destination,
        expected_digest=copied_digest,
        expected_size=size,
        label=label,
        max_bytes=max_bytes,
    )
    if verify_errors:
        errors.extend(verify_errors)
        return None
    return copied_digest


def _load_source_json(path: Path, label: str, errors: list[str]) -> dict[str, Any] | None:
    loaded = device_lab._load_json(path, label, errors)
    if loaded is None:
        return None
    return dict(loaded)


def _require_source_string(
    payload: dict[str, Any],
    key: str,
    label: str,
    errors: list[str],
) -> str | None:
    value = payload.get(key)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{label} {key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(f"{label} {key} must not have surrounding whitespace")
        return None
    if device_lab._contains_control_character(value):
        errors.append(f"{label} {key} must not contain control characters")
        return None
    if device_lab.SECRET_RE.search(value):
        errors.append(f"{label} {key} must not contain secret-looking material")
        return None
    return value


def _require_source_sha256(
    payload: dict[str, Any],
    key: str,
    label: str,
    errors: list[str],
) -> str | None:
    value = _require_source_string(payload, key, label, errors)
    if value is None:
        return None
    if not device_lab.SHA256_HEX_RE.fullmatch(value):
        errors.append(f"{label} {key} must be lowercase sha256 hex")
        return None
    if value == "0" * 64:
        errors.append(f"{label} {key} must be non-zero lowercase sha256 hex")
        return None
    return value


def _require_metadata_sha256(value: Any, label: str) -> str:
    if not isinstance(value, str) or not device_lab.SHA256_HEX_RE.fullmatch(value):
        raise ValueError(f"{label} must be lowercase sha256 hex")
    if value == "0" * 64:
        raise ValueError(f"{label} must be non-zero lowercase sha256 hex")
    return value


def _d2d_payment_transcript_relative_path(transport: str) -> str:
    if transport == PRIMARY_D2D_PAYMENT_TRANSPORT:
        return DEFAULT_D2D_TRANSCRIPT_PATH
    return f"handoff/d2d-payment-{transport}-transcript.json"


def _normalise_d2d_payment_transcripts_metadata(
    d2d_payment_transcripts: dict[str, dict[str, str]] | None,
) -> dict[str, dict[str, str]] | None:
    if d2d_payment_transcripts is None:
        return None
    normalised: dict[str, dict[str, str]] = {}
    for transport, entry in sorted(d2d_payment_transcripts.items()):
        if transport not in device_lab.D2D_PAYMENT_TRANSPORTS:
            raise ValueError(
                "d2d_payment_transcripts transport must be one of "
                f"{sorted(device_lab.D2D_PAYMENT_TRANSPORTS)}"
            )
        path = entry.get("path")
        if not isinstance(path, str) or not path:
            raise ValueError("d2d_payment_transcripts path must be a non-empty string")
        normalised[transport] = {
            "path": path,
            "sha256": _require_metadata_sha256(
                entry.get("sha256"),
                f"d2d_payment_transcripts[{transport}].sha256",
            ),
        }
    return normalised


def _require_source_true(
    payload: dict[str, Any],
    key: str,
    label: str,
    errors: list[str],
) -> None:
    if payload.get(key) is not True:
        errors.append(f"{label} {key} must be true")


def _run_adb_getprop(
    adb: str,
    serial: str | None,
    prop: str,
    *,
    timeout_seconds: int,
) -> str:
    if timeout_seconds < 0:
        raise ValueError("ADB getprop timeout must be non-negative")
    command = [adb]
    if serial:
        command.extend(["-s", serial])
    command.extend(["shell", "getprop", prop])
    errors = _command_disruption_errors(command, f"ADB getprop {prop}")
    if errors:
        raise ValueError(errors[0])
    try:
        result = subprocess.run(
            command,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=_timeout_arg(timeout_seconds),
        )
    except subprocess.TimeoutExpired as exc:
        raise ValueError(
            f"ADB getprop {prop} timed out after {timeout_seconds} seconds"
        ) from exc
    except subprocess.CalledProcessError as exc:
        raise ValueError(
            f"ADB getprop {prop} failed with exit code {exc.returncode}"
        ) from exc
    except OSError as exc:
        raise ValueError(f"ADB getprop {prop} could not be executed") from exc
    stdout = result.stdout
    if stdout.count("\n") != 1 or not stdout.endswith("\n"):
        raise ValueError("adb getprop output must be exactly one LF-terminated value")
    return stdout[:-1]


def _device_identity_override(
    override: str | None,
    key: str,
    errors: list[str],
) -> str | None:
    if override is None:
        return None
    if override == "":
        errors.append(f"{key} must be a non-empty string")
        return None
    if override != override.strip():
        errors.append(f"{key} must not contain surrounding whitespace")
        return None
    if device_lab._contains_control_character(override):
        errors.append(f"{key} must not contain control characters")
        return None
    if device_lab.SECRET_RE.search(override):
        errors.append(f"{key} must not contain secret-looking material")
        return None
    return override


def _source_identity_hint(
    payload: dict[str, Any],
    key: str,
    label: str,
    errors: list[str],
) -> str | None:
    value = payload.get(key)
    if value is None:
        return None
    if not isinstance(value, str):
        errors.append(f"{label} {key} must be a string")
        return None
    if value == "":
        errors.append(f"{label} {key} must be a non-empty string")
        return None
    return _device_identity_override(value, f"{label} {key}", errors)


def build_device_identity_hints(
    *,
    attestation_result: dict[str, Any],
    attestation_report: dict[str, Any],
    telemetry: dict[str, Any],
    errors: list[str],
) -> dict[str, str]:
    """Return validated device identity hints from captured source artifacts."""

    hints: dict[str, str] = {}
    hint_sources: dict[str, str] = {}
    sources = (
        ("device_fingerprint", attestation_result, "attestation/result.json"),
        ("device_fingerprint", attestation_report, "attestation/report.json"),
        ("os_build_id", attestation_result, "attestation/result.json"),
        ("os_build_id", attestation_report, "attestation/report.json"),
        ("device_model", telemetry, "telemetry/telemetry.json"),
        ("device_codename", telemetry, "telemetry/telemetry.json"),
    )
    for key, payload, label in sources:
        value = _source_identity_hint(payload, key, label, errors)
        if value is not None:
            if key in hints:
                if hints[key] != value:
                    errors.append(
                        f"{label} {key} must match {hint_sources[key]} {key}"
                    )
                continue
            hints[key] = value
            hint_sources[key] = label
    return hints


def read_device_identity(
    *,
    adb: str,
    serial: str | None,
    device_fingerprint: str | None,
    os_build_id: str | None,
    device_model: str | None,
    device_codename: str | None,
    adb_timeout_seconds: int = DEFAULT_ADB_TIMEOUT_SECONDS,
    identity_hints: dict[str, str] | None = None,
    errors: list[str],
) -> dict[str, str]:
    """Return device identity from overrides, captured artifacts, or ADB."""

    facts: dict[str, str] = {}
    identity_hints = identity_hints or {}
    queries = {
        "device_fingerprint": ("ro.build.fingerprint", device_fingerprint),
        "os_build_id": ("ro.build.id", os_build_id),
        "device_model": ("ro.product.model", device_model),
        "device_codename": ("ro.product.device", device_codename),
    }
    for key, (prop, override) in queries.items():
        error_count = len(errors)
        value = _device_identity_override(override, key, errors)
        if len(errors) != error_count:
            continue
        hint_value = _device_identity_override(identity_hints.get(key), key, errors)
        if len(errors) != error_count:
            continue
        if value is not None and hint_value is not None and value != hint_value:
            errors.append(f"{key} override must match captured source identity")
            continue
        if value is None:
            value = hint_value
        if value is None:
            try:
                value = _run_adb_getprop(
                    adb,
                    serial,
                    prop,
                    timeout_seconds=adb_timeout_seconds,
                )
            except ValueError as exc:
                errors.append(f"adb getprop {prop} failed: {exc}")
                continue
        if not value:
            errors.append(f"{key} could not be determined")
            continue
        if value != value.strip():
            errors.append(f"{key} must not contain surrounding whitespace")
            continue
        if device_lab._contains_control_character(value):
            errors.append(f"{key} must not contain control characters")
            continue
        if device_lab.SECRET_RE.search(value):
            errors.append(f"{key} must not contain secret-looking material")
            continue
        facts[key] = value
    return facts


def infer_device_family(model: str | None, codename: str | None) -> str | None:
    """Infer a standard Kagemusha device family from ADB model/codename."""

    model_text = model.lower() if isinstance(model, str) else ""
    codename_text = codename.lower() if isinstance(codename, str) else ""
    model_family = _match_device_model_family(model_text)
    codename_family = _match_device_codename_family(codename_text)
    if model_family is None or codename_family is None:
        return None
    if model_family != codename_family:
        return None
    return model_family


def _match_device_model_family(model_text: str) -> str | None:
    for family, exact_models, _codenames, model_prefixes in DEVICE_FAMILY_MODEL_RULES:
        if model_text in exact_models:
            return family
        if any(model_text.startswith(prefix) for prefix in model_prefixes):
            return family
    return None


def _match_device_codename_family(codename_text: str) -> str | None:
    for family, _exact_models, codenames, _model_prefixes in DEVICE_FAMILY_MODEL_RULES:
        if codename_text in codenames:
            return family
    return None


def resolve_device_family(
    requested: str | None,
    facts: dict[str, str],
    errors: list[str],
) -> str | None:
    inferred = infer_device_family(facts.get("device_model"), facts.get("device_codename"))
    has_device_identity = bool(facts.get("device_model") or facts.get("device_codename"))
    family: str | None = None
    if isinstance(requested, str) and requested != "":
        if requested != requested.strip():
            errors.append("device family must not contain surrounding whitespace")
            return None
        if device_lab._contains_control_character(requested):
            errors.append("device family must not contain control characters")
            return None
        if device_lab.SECRET_RE.search(requested):
            errors.append("device family must not contain secret-looking material")
            return None
        family = requested
        if family not in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES:
            errors.append("device family must be one of the standard Kagemusha families")
            return None
        if has_device_identity and inferred != family:
            errors.append("device family must match attached device model/codename")
            return None
    if family is None:
        family = inferred
    if family is None:
        errors.append("device family could not be inferred; pass --device-family")
        return None
    if family not in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES:
        errors.append("device family must be one of the standard Kagemusha families")
        return None
    return family


def normalise_attestation_payloads(
    *,
    attestation_result: dict[str, Any],
    attestation_report: dict[str, Any],
    slot_id: str,
    device_fingerprint: str,
    os_build_id: str,
    chain_relative: str,
    chain_digest: str,
    errors: list[str],
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Bind attestation result/report copies to the output slot artifact path."""

    result = dict(attestation_result)
    report = dict(attestation_report)
    result["attestation_certificate_chain_path"] = chain_relative
    result["attestation_certificate_chain_sha256"] = chain_digest
    report["attestation_certificate_chain_path"] = chain_relative
    report["attestation_certificate_chain_sha256"] = chain_digest
    for payload, label in (
        (result, "attestation/result.json"),
        (report, "attestation/report.json"),
    ):
        if payload.get("slot_id") != slot_id:
            errors.append(f"{label} slot_id must match --slot-id")
        if label == "attestation/result.json" and payload.get("slot") not in (None, slot_id):
            errors.append("attestation/result.json slot must match --slot-id")
        if payload.get("device_fingerprint") != device_fingerprint:
            if label == "attestation/report.json":
                errors.append(ATTESTATION_REPORT_DEVICE_FINGERPRINT_MISMATCH)
            else:
                errors.append(f"{label} device_fingerprint must match device identity")
        if payload.get("os_build_id") != os_build_id:
            if label == "attestation/report.json":
                errors.append(ATTESTATION_REPORT_OS_BUILD_MISMATCH)
            else:
                errors.append(f"{label} os_build_id must match device identity")
    return result, report


def validate_attestation_harness_source_claims(
    *,
    attestation_harness_result: dict[str, Any],
    attestation_result: dict[str, Any],
    attestation_report: dict[str, Any],
    attestation_certificate_chain_bytes: bytes,
    errors: list[str],
) -> None:
    """Validate the preserved raw StrongBox harness result before signing a slot."""

    for field in sorted(
        set(attestation_harness_result) - device_lab.ATTESTATION_HARNESS_RESULT_FIELDS
    ):
        errors.append(
            "attestation harness result contains unexpected field "
            f"{device_lab._display_path(field)}"
        )

    _require_source_string(attestation_harness_result, "alias", "attestation harness result", errors)
    for key in ("attestation_security_level", "keymaster_security_level"):
        level = _require_source_string(
            attestation_harness_result,
            key,
            "attestation harness result",
            errors,
        )
        if level is not None and level not in device_lab.STRONGBOX_LEVELS:
            errors.append(f"attestation harness result {key} must be STRONGBOX")

    _require_source_true(
        attestation_harness_result,
        "strongbox_attestation",
        "attestation harness result",
        errors,
    )

    challenge_hex = _require_source_string(
        attestation_harness_result,
        "challenge_hex",
        "attestation harness result",
        errors,
    )
    challenge: bytes | None = None
    if challenge_hex is not None:
        if (
            challenge_hex != challenge_hex.lower()
            or any(ch.isspace() for ch in challenge_hex)
            or not all(ch in "0123456789abcdef" for ch in challenge_hex)
        ):
            errors.append(
                "attestation harness result challenge_hex must be lowercase hexadecimal without whitespace"
            )
        elif len(challenge_hex) % 2 != 0:
            errors.append("attestation harness result challenge_hex must be even-length hex")
        else:
            try:
                challenge = bytes.fromhex(challenge_hex)
            except ValueError:
                errors.append("attestation harness result challenge_hex must be hex")
    if challenge is not None:
        challenge_digest = hashlib.sha256(challenge).hexdigest()
        for payload, label in (
            (attestation_result, "attestation/result.json"),
            (attestation_report, "attestation/report.json"),
        ):
            expected = _require_source_sha256(
                payload,
                "attestation_challenge_sha256",
                label,
                errors,
            )
            if expected is not None and expected != challenge_digest:
                errors.append(
                    "attestation harness result challenge_hex digest must match "
                    f"{label} attestation_challenge_sha256"
                )

    chain_length = attestation_harness_result.get("chain_length")
    if not isinstance(chain_length, int) or isinstance(chain_length, bool):
        errors.append("attestation harness result chain_length must be an integer")
    elif chain_length < 2:
        errors.append("attestation harness result chain_length must be at least 2")
    else:
        certificate_count = device_lab._certificate_chain_pem_count(
            attestation_certificate_chain_bytes
        )
        if certificate_count and chain_length != certificate_count:
            errors.append(
                "attestation harness result chain_length must match "
                "attestation certificate-chain certificate count"
            )


def build_slot_metadata(
    *,
    slot_id: str,
    family: str,
    facts: dict[str, str],
    attestation_result: dict[str, Any],
    attestation_chain_path: str,
    attestation_chain_sha256: str,
    offline_wallet_apk_sha256: str,
    d2d_payment_transcript_sha256: str,
    wallet_integrity_transcript: dict[str, Any],
    wallet_integrity_transcript_sha256: str,
    raw_test_commands: list[str],
    d2d_payment_transcripts: dict[str, dict[str, str]] | None = None,
) -> dict[str, Any]:
    app_package_name = attestation_result.get("app_package_name") or DEFAULT_APP_PACKAGE_NAME
    source_digests: dict[str, str] = {}
    for key in (
        "app_signing_certificate_sha256",
        "attestation_challenge_sha256",
        "offline_wallet_policy_sha256",
    ):
        source_digests[key] = _require_metadata_sha256(
            attestation_result.get(key),
            f"attestation_result {key}",
        )
    artifact_digests = {
        "attestation_certificate_chain_sha256": _require_metadata_sha256(
            attestation_chain_sha256,
            "attestation_certificate_chain_sha256",
        ),
        "offline_wallet_apk_sha256": _require_metadata_sha256(
            offline_wallet_apk_sha256,
            "offline_wallet_apk_sha256",
        ),
        "d2d_payment_transcript_sha256": _require_metadata_sha256(
            d2d_payment_transcript_sha256,
            "d2d_payment_transcript_sha256",
        ),
        "wallet_integrity_transcript_sha256": _require_metadata_sha256(
            wallet_integrity_transcript_sha256,
            "wallet_integrity_transcript_sha256",
        ),
    }
    transcript_bindings = _normalise_d2d_payment_transcripts_metadata(
        d2d_payment_transcripts
    )
    metadata = {
        "schema": "iroha.android.device_lab.kagemusha.v1",
        "slot_id": slot_id,
        "device_family": family,
        "device_model": facts["device_model"],
        "device_codename": facts["device_codename"],
        "device_fingerprint": facts["device_fingerprint"],
        "os_build_id": facts["os_build_id"],
        "minimum_os": device_lab.KAGEMUSHA_STANDARD_DEVICE_MINIMUM_OS[family],
        "app_package_name": app_package_name,
        "attestation_certificate_chain_path": attestation_chain_path,
        "offline_wallet_apk_path": DEFAULT_OFFLINE_WALLET_APK_PATH,
        "d2d_payment_transcript_path": DEFAULT_D2D_TRANSCRIPT_PATH,
        "wallet_integrity_transcript_path": DEFAULT_WALLET_TRANSCRIPT_PATH,
        "app_signing_certificate_sha256": source_digests[
            "app_signing_certificate_sha256"
        ],
        "attestation_challenge_sha256": source_digests[
            "attestation_challenge_sha256"
        ],
        "attestation_certificate_chain_sha256": artifact_digests[
            "attestation_certificate_chain_sha256"
        ],
        "offline_wallet_policy_sha256": source_digests[
            "offline_wallet_policy_sha256"
        ],
        "offline_wallet_apk_sha256": artifact_digests["offline_wallet_apk_sha256"],
        "d2d_payment_transcript_sha256": artifact_digests[
            "d2d_payment_transcript_sha256"
        ],
        "wallet_integrity_transcript_sha256": artifact_digests[
            "wallet_integrity_transcript_sha256"
        ],
        "native_bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
        "strongbox_attestation": attestation_result.get("strongbox_attestation"),
        "physical_device_attestation": attestation_result.get("physical_device_attestation"),
        "keymint_security_level": attestation_result.get("keymint_security_level"),
        "one_use_key_rotation_passed": wallet_integrity_transcript.get(
            "one_use_key_rotation_passed"
        ),
        "rollback_rejection_passed": wallet_integrity_transcript.get(
            "rollback_rejection_passed"
        ),
        "abi6_recursive_spend_jni_probe": "passed",
        "abi7_recursive_compact_jni_probe": "one_hop_verified",
        "abi7_recursive_compact_prover_state": "multi_hop_proof_composed",
        "signed_evidence_artifact_path": device_lab.KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH,
        "signed_evidence_artifact_sha256": "0" * 64,
        "raw_test_commands": raw_test_commands,
    }
    if transcript_bindings is not None:
        metadata[device_lab.D2D_PAYMENT_TRANSCRIPTS_FIELD] = transcript_bindings
    return metadata


def validate_d2d_payment_transcript_source_claims(
    d2d_payment_transcript: dict[str, Any],
    label: str,
    errors: list[str],
) -> str | None:
    for field in sorted(
        set(d2d_payment_transcript) - device_lab.D2D_PAYMENT_TRANSCRIPT_FIELDS
    ):
        errors.append(
            f"{label} contains unexpected field {device_lab._display_path(field)}"
        )
    d2d_schema = d2d_payment_transcript.get("schema")
    if d2d_schema != device_lab.D2D_PAYMENT_TRANSCRIPT_SCHEMA:
        errors.append(
            f"{label} schema must be {device_lab.D2D_PAYMENT_TRANSCRIPT_SCHEMA}"
        )
    transport = _require_source_string(
        d2d_payment_transcript,
        "transport",
        label,
        errors,
    )
    if transport is not None and transport not in device_lab.D2D_PAYMENT_TRANSPORTS:
        errors.append(
            f"{label} transport must be one of {sorted(device_lab.D2D_PAYMENT_TRANSPORTS)}"
        )
        return None
    return transport


def validate_slot_source_claims(
    *,
    attestation_result: dict[str, Any],
    attestation_report: dict[str, Any],
    d2d_payment_transcript: dict[str, Any],
    extra_d2d_payment_transcripts: dict[str, dict[str, Any]] | None = None,
    wallet_integrity_transcript: dict[str, Any],
    errors: list[str],
) -> str | None:
    for field in sorted(set(attestation_result) - device_lab.ATTESTATION_RESULT_FIELDS):
        errors.append(
            "attestation/result.json contains unexpected field "
            f"{device_lab._display_path(field)}"
        )
    for field in sorted(set(attestation_report) - device_lab.ATTESTATION_REPORT_FIELDS):
        errors.append(
            "attestation/report.json contains unexpected field "
            f"{device_lab._display_path(field)}"
        )
    report_schema = attestation_report.get("schema")
    if report_schema != device_lab.ATTESTATION_REPORT_SCHEMA:
        errors.append(
            f"attestation/report.json schema must be {device_lab.ATTESTATION_REPORT_SCHEMA}"
        )
    _require_source_string(attestation_report, "verifier", "attestation/report.json", errors)
    for field in sorted(
        set(wallet_integrity_transcript) - device_lab.WALLET_INTEGRITY_TRANSCRIPT_FIELDS
    ):
        errors.append(
            "wallet integrity transcript contains unexpected field "
            f"{device_lab._display_path(field)}"
        )
    primary_transport = validate_d2d_payment_transcript_source_claims(
        d2d_payment_transcript,
        "d2d payment transcript",
        errors,
    )
    for transport, transcript in sorted((extra_d2d_payment_transcripts or {}).items()):
        transcript_transport = validate_d2d_payment_transcript_source_claims(
            transcript,
            f"d2d payment transcript {transport}",
            errors,
        )
        if transcript_transport is not None and transcript_transport != transport:
            errors.append(
                f"d2d payment transcript {transport} transport must match {transport}"
            )
    wallet_schema = wallet_integrity_transcript.get("schema")
    if wallet_schema != device_lab.WALLET_INTEGRITY_TRANSCRIPT_SCHEMA:
        errors.append(
            "wallet integrity transcript schema must be "
            f"{device_lab.WALLET_INTEGRITY_TRANSCRIPT_SCHEMA}"
        )
    _require_source_true(attestation_result, "strongbox_attestation", "attestation/result.json", errors)
    _require_source_true(
        attestation_result,
        "physical_device_attestation",
        "attestation/result.json",
        errors,
    )
    _require_source_sha256(
        attestation_result,
        "app_signing_certificate_sha256",
        "attestation/result.json",
        errors,
    )
    result_challenge = _require_source_sha256(
        attestation_result,
        "attestation_challenge_sha256",
        "attestation/result.json",
        errors,
    )
    _require_source_sha256(
        attestation_result,
        "offline_wallet_policy_sha256",
        "attestation/result.json",
        errors,
    )
    result_app_package = _require_source_string(
        attestation_result,
        "app_package_name",
        "attestation/result.json",
        errors,
    )
    report_app_package = _require_source_string(
        attestation_report,
        "app_package_name",
        "attestation/report.json",
        errors,
    )
    if (
        result_app_package is not None
        and report_app_package is not None
        and result_app_package != report_app_package
    ):
        errors.append(
            "attestation/report.json app_package_name must match "
            "attestation/result.json app_package_name"
        )
    report_challenge = _require_source_sha256(
        attestation_report,
        "attestation_challenge_sha256",
        "attestation/report.json",
        errors,
    )
    if (
        result_challenge is not None
        and report_challenge is not None
        and result_challenge != report_challenge
    ):
        errors.append(
            "attestation/report.json attestation_challenge_sha256 must match "
            "attestation/result.json attestation_challenge_sha256"
        )
    verification = attestation_report.get("verification")
    if not isinstance(verification, dict):
        errors.append("attestation/report.json verification must be an object")
    else:
        for field in sorted(
            set(verification) - device_lab.ATTESTATION_REPORT_VERIFICATION_FIELDS
        ):
            errors.append(
                "attestation/report.json verification contains unexpected field "
                f"{device_lab._display_path(field)}"
            )
        result_status = _require_source_string(
            attestation_result,
            "status",
            "attestation/result.json",
            errors,
        )
        if result_status is not None and result_status != "ok":
            errors.append("attestation/result.json status must be ok")
        report_status = _require_source_string(
            verification,
            "status",
            "attestation/report.json verification",
            errors,
        )
        if report_status is not None and report_status != "ok":
            errors.append("attestation/report.json verification.status must be ok")
        if (
            result_status is not None
            and report_status is not None
            and result_status != report_status
        ):
            errors.append(
                "attestation/report.json verification.status must match "
                "attestation/result.json status"
            )
        _require_source_true(
            verification,
            "strongbox_attestation",
            "attestation/report.json verification",
            errors,
        )
        _require_source_true(
            verification,
            "physical_device_attestation",
            "attestation/report.json verification",
            errors,
        )
        for level_key in (
            "keymint_security_level",
            "attestation_security_level",
            "keymaster_security_level",
        ):
            result_level = _require_source_string(
                attestation_result,
                level_key,
                "attestation/result.json",
                errors,
            )
            if result_level is not None and result_level not in device_lab.STRONGBOX_LEVELS:
                errors.append(f"attestation/result.json {level_key} must be STRONGBOX")
            report_level = _require_source_string(
                verification,
                level_key,
                "attestation/report.json verification",
                errors,
            )
            if report_level is not None and report_level not in device_lab.STRONGBOX_LEVELS:
                errors.append(
                    f"attestation/report.json verification.{level_key} must be STRONGBOX"
                )
            if (
                result_level is not None
                and report_level is not None
                and result_level != report_level
            ):
                errors.append(
                    f"attestation/report.json verification.{level_key} must match "
                    f"attestation/result.json {level_key}"
                )
    _require_source_true(
        wallet_integrity_transcript,
        "one_use_key_rotation_passed",
        "wallet integrity transcript",
        errors,
    )
    if wallet_integrity_transcript.get("rollback_rejection_passed") is not True:
        errors.append(WALLET_ROLLBACK_REQUIRED)
    return primary_transport


def parse_d2d_payment_transcript_extra_specs(
    specs: list[str] | tuple[str, ...] | None,
    errors: list[str],
) -> dict[str, Path]:
    """Parse operator-supplied D2D transcript extras keyed by transport."""

    parsed: dict[str, Path] = {}
    for spec in specs or ():
        if not isinstance(spec, str) or not spec:
            errors.append("D2D payment transcript extra must be transport=path")
            continue
        if "=" not in spec:
            errors.append("D2D payment transcript extra must be transport=path")
            continue
        transport, raw_path = spec.split("=", 1)
        if transport not in device_lab.D2D_PAYMENT_TRANSPORTS:
            errors.append(
                "D2D payment transcript extra transport must be one of "
                f"{sorted(device_lab.D2D_PAYMENT_TRANSPORTS)}"
            )
            continue
        if transport == PRIMARY_D2D_PAYMENT_TRANSPORT:
            errors.append(
                f"D2D payment transcript extra must not override {PRIMARY_D2D_PAYMENT_TRANSPORT}"
            )
            continue
        if transport in parsed:
            errors.append(f"D2D payment transcript extra {transport} is duplicated")
            continue
        if not raw_path:
            errors.append("D2D payment transcript extra path must be non-empty")
            continue
        parsed[transport] = Path(raw_path)
    return parsed


def _device_lab_root_list(root: Path | Iterable[Path]) -> list[Path]:
    """Normalize one or more Android device-lab roots."""

    if isinstance(root, (str, os.PathLike)):
        return [Path(root)]
    return [Path(item) for item in root]


def _device_lab_root_path_error(
    root: Path,
) -> tuple[int, Path | None, list[str]] | None:
    root_text = str(root)
    if device_lab.SECRET_RE.search(root_text):
        return 1, None, ["device-lab root path must not contain secret-looking material"]
    if device_lab._contains_control_character(root_text):
        return 1, None, ["device-lab root path must not contain control characters"]
    if root_text != root_text.strip() or device_lab._path_has_surrounding_whitespace_component(  # type: ignore[attr-defined]
        root
    ):
        return 1, None, ["device-lab root path must not contain surrounding whitespace"]
    if "\\" in root_text:
        return 1, None, ["device-lab root path must not contain backslashes"]
    if ".." in root.parts:
        return 1, None, ["device-lab root path must be canonical"]
    return None


def assemble_slot(args: argparse.Namespace) -> tuple[int, Path | None, list[str]]:
    """Assemble the requested slot and optionally sign it."""

    errors: list[str] = []
    if any(character.isspace() for character in args.slot_id):
        return 1, None, ["slot id must not contain whitespace"]
    if device_lab._contains_control_character(args.slot_id):
        return 1, None, ["slot id must not contain control characters"]
    if args.adb_timeout_seconds < 0:
        return 1, None, ["--adb-timeout-seconds must be non-negative"]
    slot_id = _single_safe_slot_id(args.slot_id)
    if slot_id is None:
        return 1, None, ["slot id must be a single safe directory name"]

    root = args.slot_root
    roots = _device_lab_root_list(root)
    if not roots:
        return 1, None, ["device-lab root path is required"]
    existing_roots: list[tuple[int, Path]] = []
    for root_index, candidate_root in enumerate(roots):
        root_path_error = _device_lab_root_path_error(candidate_root)
        if root_path_error is not None:
            return root_path_error
        root_exists, root_errors = device_lab.classify_device_lab_root_path(candidate_root)
        if root_errors:
            return 1, None, root_errors
        if root_exists:
            existing_roots.append((root_index, candidate_root))
    if len(existing_roots) > 1:
        return 1, None, ["device-lab root path must resolve to exactly one existing root"]
    if existing_roots:
        _, root = existing_roots[0]
        root_exists = True
    else:
        root = roots[0]
        root_exists = False
    if not root_exists:
        root.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        root.mkdir(mode=0o700)
    root_permission_errors = _set_private_directory_permissions(
        root,
        "device-lab root directory",
    )
    if root_permission_errors:
        return 1, None, root_permission_errors
    try:
        root_stat = root.lstat()
    except OSError:
        return 1, None, ["device-lab root metadata could not be read"]
    if stat.S_ISLNK(root_stat.st_mode) or not stat.S_ISDIR(root_stat.st_mode):
        return 1, None, ["device-lab root must be a directory"]
    root_identity = _file_identity(root_stat)

    final_slot = root / slot_id
    try:
        final_slot.lstat()
    except FileNotFoundError:
        pass
    except OSError:
        return 1, None, ["slot directory metadata could not be read before publish"]
    else:
        return 1, None, ["slot directory already exists; refuse to overwrite evidence"]

    sign_args = [args.private_key, args.public_key, args.signer_key_id]
    sign_requested = any(value is not None for value in sign_args)
    if not sign_requested and not args.allow_unsigned:
        return 1, None, ["signing inputs are required unless --allow-unsigned is set"]
    if sign_requested and not all(value is not None for value in sign_args):
        return 1, None, ["--private-key, --public-key, and --signer-key-id must be supplied together"]

    result = _load_source_json(args.attestation_result, "attestation result", errors)
    report = _load_source_json(args.attestation_report, "attestation verifier report", errors)
    telemetry = _load_source_json(args.telemetry_json, "telemetry identity source", errors)
    if result is None or report is None or telemetry is None:
        return 1, None, errors

    identity_hints = build_device_identity_hints(
        attestation_result=result,
        attestation_report=report,
        telemetry=telemetry,
        errors=errors,
    )
    if errors:
        return 1, None, errors

    facts = read_device_identity(
        adb=args.adb,
        serial=args.serial,
        device_fingerprint=args.device_fingerprint,
        os_build_id=args.os_build_id,
        device_model=args.device_model,
        device_codename=args.device_codename,
        adb_timeout_seconds=args.adb_timeout_seconds,
        identity_hints=identity_hints,
        errors=errors,
    )
    family = resolve_device_family(args.device_family, facts, errors)
    if errors or family is None:
        return 1, None, errors

    d2d = _load_source_json(args.d2d_payment_transcript, "D2D payment transcript", errors)
    extra_d2d_specs = parse_d2d_payment_transcript_extra_specs(
        args.d2d_payment_transcript_extra,
        errors,
    )
    extra_d2d: dict[str, dict[str, Any]] = {}
    for transport, source_path in sorted(extra_d2d_specs.items()):
        loaded = _load_source_json(
            source_path,
            f"D2D payment transcript {transport}",
            errors,
        )
        if loaded is not None:
            extra_d2d[transport] = loaded
    wallet = _load_source_json(
        args.wallet_integrity_transcript,
        "wallet integrity transcript",
        errors,
    )
    if d2d is None or wallet is None:
        return 1, None, errors

    primary_d2d_transport = validate_slot_source_claims(
        attestation_result=result,
        attestation_report=report,
        d2d_payment_transcript=d2d,
        extra_d2d_payment_transcripts=extra_d2d,
        wallet_integrity_transcript=wallet,
        errors=errors,
    )
    if errors:
        return 1, None, errors

    temp_parent = Path(tempfile.mkdtemp(prefix=f".{slot_id}.", dir=root))
    try:
        temp_parent_identity = _file_identity(temp_parent.lstat())
    except OSError:
        return 1, None, ["staged slot parent metadata could not be read"]
    temp_permission_errors = _set_private_directory_permissions(
        temp_parent,
        "staged slot parent directory",
    )
    if temp_permission_errors:
        cleanup_errors = _cleanup_temp_parent(
            temp_parent,
            expected_identity=temp_parent_identity,
        )
        return 1, None, [*temp_permission_errors, *cleanup_errors]
    stage_slot = temp_parent / slot_id
    try:
        stage_slot.mkdir(mode=0o700)
    except OSError:
        cleanup_errors = _cleanup_temp_parent(
            temp_parent,
            expected_identity=temp_parent_identity,
        )
        return 1, None, ["staged slot directory could not be created", *cleanup_errors]
    stage_permission_errors = _set_private_directory_permissions(
        stage_slot,
        "staged slot directory",
    )
    if stage_permission_errors:
        cleanup_errors = _cleanup_temp_parent(
            temp_parent,
            expected_identity=temp_parent_identity,
        )
        return 1, None, [*stage_permission_errors, *cleanup_errors]

    def _stage_and_publish() -> tuple[int, Path | None, list[str]]:
        nonlocal result, report

        chain_name = args.attestation_certificate_chain.name
        if Path(chain_name).suffix.lower() not in device_lab.ATTESTATION_CERTIFICATE_CHAIN_SUFFIXES:
            errors.append("attestation certificate chain source must end in .pem or .der")
            return 1, None, errors
        chain_relative = f"attestation/{chain_name}"
        chain_digest = _copy_source_file(
            source=args.attestation_certificate_chain,
            destination=stage_slot / chain_relative,
            label="attestation certificate chain source",
            errors=errors,
            max_bytes=device_lab.MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES,
        )
        apk_digest = _copy_source_file(
            source=args.offline_wallet_apk,
            destination=stage_slot / DEFAULT_OFFLINE_WALLET_APK_PATH,
            label="offline wallet release APK source",
            errors=errors,
            max_bytes=device_lab.MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES,
        )
        d2d_digest = _copy_source_file(
            source=args.d2d_payment_transcript,
            destination=stage_slot / DEFAULT_D2D_TRANSCRIPT_PATH,
            label="D2D payment transcript source",
            errors=errors,
        )
        d2d_transcript_bindings: dict[str, dict[str, str]] = {}
        if primary_d2d_transport is not None and d2d_digest is not None:
            d2d_transcript_bindings[primary_d2d_transport] = {
                "path": DEFAULT_D2D_TRANSCRIPT_PATH,
                "sha256": d2d_digest,
            }
        for transport, source_path in sorted(extra_d2d_specs.items()):
            relative = _d2d_payment_transcript_relative_path(transport)
            extra_digest = _copy_source_file(
                source=source_path,
                destination=stage_slot / relative,
                label=f"D2D payment transcript {transport} source",
                errors=errors,
            )
            if extra_digest is not None:
                d2d_transcript_bindings[transport] = {
                    "path": relative,
                    "sha256": extra_digest,
                }
        wallet_digest = _copy_source_file(
            source=args.wallet_integrity_transcript,
            destination=stage_slot / DEFAULT_WALLET_TRANSCRIPT_PATH,
            label="wallet integrity transcript source",
            errors=errors,
        )
        harness_digest = _copy_source_file(
            source=args.attestation_harness_result,
            destination=stage_slot / DEFAULT_ATTESTATION_HARNESS_RESULT_PATH,
            label="attestation harness result source",
            errors=errors,
        )
        _copy_source_file(
            source=args.telemetry_json,
            destination=stage_slot / "telemetry" / "telemetry.json",
            label="telemetry JSON source",
            errors=errors,
        )
        _copy_source_file(
            source=args.status_ndjson,
            destination=stage_slot / "telemetry" / "status.ndjson",
            label="status NDJSON source",
            errors=errors,
        )
        _copy_source_file(
            source=args.pending_queue_json,
            destination=stage_slot / "queue" / "pending_queue.json",
            label="pending queue JSON source",
            errors=errors,
        )
        _copy_source_file(
            source=args.runtime_log,
            destination=stage_slot / "logs" / "runtime.log",
            label="runtime log source",
            errors=errors,
        )
        if (
            chain_digest is None
            or apk_digest is None
            or d2d_digest is None
            or wallet_digest is None
            or harness_digest is None
        ):
            return 1, None, errors

        result, report = normalise_attestation_payloads(
            attestation_result=result,
            attestation_report=report,
            slot_id=slot_id,
            device_fingerprint=facts["device_fingerprint"],
            os_build_id=facts["os_build_id"],
            chain_relative=chain_relative,
            chain_digest=chain_digest,
            errors=errors,
        )
        if errors:
            return 1, None, errors

        harness = _load_source_json(
            stage_slot / DEFAULT_ATTESTATION_HARNESS_RESULT_PATH,
            "attestation harness result",
            errors,
        )
        try:
            chain_payload = (stage_slot / chain_relative).read_bytes()
        except OSError:
            errors.append("attestation certificate chain staged copy could not be read")
            chain_payload = b""
        if harness is not None:
            validate_attestation_harness_source_claims(
                attestation_harness_result=harness,
                attestation_result=result,
                attestation_report=report,
                attestation_certificate_chain_bytes=chain_payload,
                errors=errors,
            )
        if errors:
            return 1, None, errors

        errors.extend(
            _write_json(
                stage_slot / "attestation" / "result.json",
                result,
                "attestation result",
            )
        )
        errors.extend(
            _write_json(
                stage_slot / "attestation" / "report.json",
                report,
                "attestation verifier report",
            )
        )
        result_app_package_name = result.get("app_package_name")
        device_lab.validate_required_kagemusha_slot_artifact_shapes(
            stage_slot,
            errors,
            expected_app_package_name=(
                result_app_package_name
                if isinstance(result_app_package_name, str)
                else None
            ),
            expected_app_package_label="attestation/result.json app_package_name",
            expected_device_model=facts["device_model"],
            expected_device_codename=facts["device_codename"],
        )
        if errors:
            return 1, None, errors

        metadata = build_slot_metadata(
            slot_id=slot_id,
            family=family,
            facts=facts,
            attestation_result=result,
            attestation_chain_path=chain_relative,
            attestation_chain_sha256=chain_digest,
            offline_wallet_apk_sha256=apk_digest,
            d2d_payment_transcript_sha256=d2d_digest,
            wallet_integrity_transcript=wallet,
            wallet_integrity_transcript_sha256=wallet_digest,
            raw_test_commands=list(device_lab.KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS),
            d2d_payment_transcripts=d2d_transcript_bindings,
        )
        device_lab.validate_d2d_payment_transcripts_binding(
            stage_slot,
            metadata,
            errors,
            primary_relative=DEFAULT_D2D_TRANSCRIPT_PATH,
            primary_digest=d2d_digest,
            primary_transport=primary_d2d_transport,
        )
        device_lab.validate_wallet_integrity_transcript(
            stage_slot / DEFAULT_WALLET_TRANSCRIPT_PATH,
            metadata,
            errors,
        )
        if errors:
            return 1, None, errors
        errors.extend(_write_json(stage_slot / "slot.json", metadata, "slot metadata"))
        if errors:
            return 1, None, errors

        manifest_errors = evidence_signer.rewrite_sha256_manifest(stage_slot)
        if manifest_errors:
            errors.extend(manifest_errors)
            return 1, None, errors

        if sign_requested:
            assert args.private_key is not None
            assert args.public_key is not None
            assert args.signer_key_id is not None
            status, _output_relative, sign_errors = evidence_signer.sign_slot_evidence(
                slot_path=stage_slot,
                private_key_path=args.private_key,
                public_key_path=args.public_key,
                signer_key_id=args.signer_key_id,
                signed_at_utc=args.signed_at_utc
                or evidence_signer.default_signed_at_utc(),
                output=None,
                update_slot_json=True,
                update_sha256sum=True,
            )
            if status != 0:
                errors.extend(sign_errors)
                return status, None, errors

        if errors:
            return 1, None, errors
        try:
            temp_parent_stat = temp_parent.lstat()
            stage_slot_stat = stage_slot.lstat()
        except OSError:
            return 1, None, ["staged slot metadata could not be read before publish"]
        if not stat.S_ISDIR(temp_parent_stat.st_mode) or not stat.S_ISDIR(
            stage_slot_stat.st_mode
        ):
            return 1, None, ["staged slot must be a directory"]
        if stat.S_IMODE(temp_parent_stat.st_mode) != 0o700:
            return 1, None, ["staged slot parent directory permissions must be 0700"]
        if stat.S_IMODE(stage_slot_stat.st_mode) != 0o700:
            return 1, None, ["staged slot directory permissions must be 0700"]
        publish_errors = _publish_stage_slot(
            stage_slot=stage_slot,
            root=root,
            slot_id=slot_id,
            expected_root_identity=root_identity,
            expected_temp_parent_identity=temp_parent_identity,
            expected_stage_identity=_file_identity(stage_slot_stat),
        )
        if publish_errors:
            return 1, None, publish_errors
        return 0, final_slot, []

    stage_status = 1
    stage_output: Path | None = None
    stage_errors: list[str] = []
    try:
        stage_status, stage_output, stage_errors = _stage_and_publish()
    finally:
        cleanup_errors = _cleanup_temp_parent(
            temp_parent,
            expected_identity=temp_parent_identity,
        )
    if stage_errors or cleanup_errors:
        return 1, stage_output, [*stage_errors, *cleanup_errors]
    return stage_status, stage_output, stage_errors


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Assemble a Kagemusha Android device-lab slot from completed lab "
            "artifacts and optionally sign it."
        )
    )
    parser.add_argument("--slot-root", type=Path, default=Path("artifacts/android/device_lab"))
    parser.add_argument("--slot-id", required=True)
    parser.add_argument("--device-family")
    parser.add_argument("--adb", default="adb")
    parser.add_argument(
        "--adb-timeout-seconds",
        type=int,
        default=DEFAULT_ADB_TIMEOUT_SECONDS,
        help="ADB subprocess timeout in seconds; 0 disables the timeout.",
    )
    parser.add_argument("--serial")
    parser.add_argument("--device-fingerprint")
    parser.add_argument("--os-build-id")
    parser.add_argument("--device-model")
    parser.add_argument("--device-codename")
    parser.add_argument("--attestation-result", type=Path, required=True)
    parser.add_argument("--attestation-harness-result", type=Path, required=True)
    parser.add_argument("--attestation-report", type=Path, required=True)
    parser.add_argument("--attestation-certificate-chain", type=Path, required=True)
    parser.add_argument("--offline-wallet-apk", type=Path, required=True)
    parser.add_argument("--d2d-payment-transcript", type=Path, required=True)
    parser.add_argument(
        "--d2d-payment-transcript-extra",
        action="append",
        default=[],
        help=(
            "Additional D2D transcript as transport=path. Use nfc_hce=... and "
            "qr=... for the production offline-offline matrix."
        ),
    )
    parser.add_argument("--wallet-integrity-transcript", type=Path, required=True)
    parser.add_argument("--telemetry-json", type=Path, required=True)
    parser.add_argument("--status-ndjson", type=Path, required=True)
    parser.add_argument("--pending-queue-json", type=Path, required=True)
    parser.add_argument("--runtime-log", type=Path, required=True)
    parser.add_argument("--private-key", type=Path)
    parser.add_argument("--public-key", type=Path)
    parser.add_argument("--signer-key-id")
    parser.add_argument("--signed-at-utc")
    parser.add_argument(
        "--allow-unsigned",
        action="store_true",
        help="Write an unsigned staging slot. The production readiness rollup will reject it.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    status, slot_path, errors = assemble_slot(args)
    if status != 0:
        for error in errors:
            print(f"[kagemusha-device-lab-slot] {error}", file=sys.stderr)
        return status
    assert slot_path is not None
    print(f"[kagemusha-device-lab-slot] wrote {slot_path}")
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
