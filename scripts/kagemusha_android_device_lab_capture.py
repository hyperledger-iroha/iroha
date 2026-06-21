#!/usr/bin/env python3
"""Capture, assemble, and validate one Kagemusha Android device-lab slot."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess
import stat
import sys
from typing import Any, Callable, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402
import kagemusha_pull_android_device_lab_raw_slot as raw_puller  # noqa: E402


CAPTURE_SUMMARY_SCHEMA = "iroha.android.device_lab.kagemusha.capture.v1"
MAX_CAPTURE_JSON_BYTES = device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES
MAX_CAPTURE_CHALLENGE_BYTES = 4096
DEFAULT_APP_PACKAGE_NAME = raw_puller.DEFAULT_RUN_AS_PACKAGE
DEFAULT_INSTRUMENTATION_RUNNER = (
    "org.hyperledger.iroha.sdk.offline.wallet.lab.test/"
    "androidx.test.runner.AndroidJUnitRunner"
)
DEVICE_LAB_EXPORT_TEST_CLASS = (
    "org.hyperledger.iroha.android.offline.KagemushaDeviceLabArtifactExportTest"
)
DEFAULT_OFFLINE_WALLET_APK = Path(
    "kotlin/offline-wallet-lab-app/build/outputs/apk/release/"
    "offline-wallet-lab-app-release.apk"
)
GRADLE_LAB_APP_TASKS: tuple[str, ...] = (
    ":offline-wallet-lab-app:assembleRelease",
    ":offline-wallet-lab-app:assembleReleaseAndroidTest",
    ":offline-wallet-lab-app:installRelease",
    ":offline-wallet-lab-app:installReleaseAndroidTest",
)
PRIMARY_D2D_TRANSCRIPT = Path("handoff/d2d-payment.json")
EXTRA_D2D_TRANSCRIPTS: tuple[tuple[str, Path], ...] = (
    ("nfc_hce", Path("handoff/d2d-payment-nfc_hce.json")),
    ("qr", Path("handoff/d2d-payment-qr.json")),
)

Runner = Callable[..., subprocess.CompletedProcess[Any]]


def _json_dumps(payload: dict[str, Any]) -> str:
    return json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"


def _safe_command_display(command: Sequence[str]) -> str:
    rendered = " ".join(command)
    if device_lab.SECRET_RE.search(rendered):
        return "<redacted-command>"
    if device_lab._contains_control_character(rendered):
        return "<unsafe-command>"
    return rendered


def _validate_cli_string(value: object, label: str) -> list[str]:
    if not isinstance(value, str) or not value:
        return [f"{label} must be a non-empty string"]
    if value != value.strip():
        return [f"{label} must not contain surrounding whitespace"]
    if device_lab._contains_control_character(value):
        return [f"{label} must not contain control characters"]
    if device_lab.SECRET_RE.search(value):
        return [f"{label} must not contain secret-looking material"]
    return []


def _validate_path_shape(path: Path, label: str) -> list[str]:
    text = str(path)
    if device_lab.SECRET_RE.search(text):
        return [f"{label} must not contain secret-looking material"]
    if device_lab._contains_control_character(text):
        return [f"{label} must not contain control characters"]
    if "\\" in text:
        return [f"{label} must not contain backslashes"]
    if ".." in path.parts:
        return [f"{label} must be canonical"]
    return []


def _default_raw_summary_path(raw_root: Path) -> Path:
    return raw_root.parent / f"{raw_root.name}-summary.json"


def _default_validation_summary_path(slot_root: Path) -> Path:
    return slot_root.parent / f"{slot_root.name}-validation.json"


def _utc_now() -> str:
    return (
        dt.datetime.now(dt.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z")
    )


def _file_identity(file_stat: os.stat_result) -> tuple[int, int]:
    return (file_stat.st_dev, file_stat.st_ino)


def _directory_open_flags() -> int:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _write_all(file_fd: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        written = os.write(file_fd, view)
        if written <= 0:
            raise OSError("short write")
        view = view[written:]


def _open_capture_summary_parent(
    parent: Path,
) -> tuple[int | None, tuple[int, int] | None, list[str]]:
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        parent,
        "capture summary output ancestor directory",
    )
    if ancestor_errors:
        return None, None, ancestor_errors
    flags = _directory_open_flags()
    if parent.is_absolute():
        start_path = Path(parent.anchor)
        parts = list(parent.parts[1:])
        if parts:
            first_path = start_path / parts[0]
            try:
                if stat.S_ISLNK(first_path.lstat().st_mode):
                    start_path = first_path.resolve(strict=True)
                    parts = parts[1:]
            except FileNotFoundError:
                pass
            except OSError:
                return None, None, ["capture summary output parent metadata could not be read"]
    else:
        start_path = Path.cwd()
        parts = list(parent.parts)
    try:
        current_fd = os.open(start_path, flags)
    except OSError:
        return None, None, ["capture summary output parent metadata could not be read"]
    filtered_parts = [part for part in parts if part not in ("", ".")]
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
                os.close(current_fd)
                return None, None, ["capture summary output parent could not be created"]
            try:
                next_fd = os.open(part, flags, dir_fd=current_fd)
            except OSError:
                os.close(current_fd)
                return None, None, [
                    "capture summary output parent changed before permissions were tightened"
                ]
        except OSError:
            try:
                child_stat = os.stat(
                    part,
                    dir_fd=current_fd,
                    follow_symlinks=False,
                )
            except OSError:
                os.close(current_fd)
                return None, None, [
                    "capture summary output parent metadata could not be read"
                ]
            os.close(current_fd)
            if stat.S_ISLNK(child_stat.st_mode):
                if is_final:
                    return None, None, [
                        "capture summary output parent directory must not be a symlink"
                    ]
                return None, None, [
                    "capture summary output ancestor directory must not be a symlink"
                ]
            if not stat.S_ISDIR(child_stat.st_mode):
                return None, None, ["capture summary output parent must be a directory"]
            return None, None, ["capture summary output parent metadata could not be read"]
        try:
            next_stat = os.fstat(next_fd)
            if not stat.S_ISDIR(next_stat.st_mode):
                os.close(next_fd)
                os.close(current_fd)
                return None, None, ["capture summary output parent must be a directory"]
            if created:
                os.fchmod(next_fd, 0o700)
                next_stat = os.fstat(next_fd)
                if stat.S_IMODE(next_stat.st_mode) != 0o700:
                    os.close(next_fd)
                    os.close(current_fd)
                    return None, None, [
                        "capture summary output parent permissions must be 0700"
                    ]
        except OSError:
            os.close(next_fd)
            os.close(current_fd)
            return None, None, [
                "capture summary output parent permissions could not be tightened"
            ]
        os.close(current_fd)
        current_fd = next_fd
    try:
        parent_stat = os.fstat(current_fd)
    except OSError:
        os.close(current_fd)
        return None, None, ["capture summary output parent metadata could not be read"]
    if not stat.S_ISDIR(parent_stat.st_mode):
        os.close(current_fd)
        return None, None, ["capture summary output parent must be a directory"]
    return current_fd, _file_identity(parent_stat), []


def _unlink_file_if_identity_at(
    parent_fd: int,
    name: str,
    expected_identity: tuple[int, int],
) -> list[str]:
    try:
        path_stat = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return []
    except OSError:
        return ["capture summary output rollback cleanup metadata could not be read"]
    if stat.S_ISREG(path_stat.st_mode) and _file_identity(path_stat) == expected_identity:
        try:
            os.unlink(name, dir_fd=parent_fd)
            return []
        except OSError:
            return ["capture summary output rollback cleanup could not remove file"]
    return []


def _sync_directory(
    path: Path,
    label: str,
    *,
    expected_identity: tuple[int, int] | None = None,
) -> list[str]:
    try:
        current_stat = path.lstat()
    except OSError:
        return [label]
    if expected_identity is not None and _file_identity(current_stat) != expected_identity:
        return [label]
    if stat.S_ISLNK(current_stat.st_mode) or not stat.S_ISDIR(current_stat.st_mode):
        return [label]
    try:
        directory_fd = os.open(path, _directory_open_flags())
    except OSError:
        return [label]
    close_failed = False
    try:
        try:
            open_stat = os.fstat(directory_fd)
        except OSError:
            return [label]
        if not stat.S_ISDIR(open_stat.st_mode):
            return [label]
        if expected_identity is not None and _file_identity(open_stat) != expected_identity:
            return [label]
        try:
            os.fsync(directory_fd)
        except OSError:
            return [label]
    finally:
        try:
            os.close(directory_fd)
        except OSError:
            close_failed = True
    if close_failed:
        return [label]
    return []


def _read_regular_text_file(
    path: Path,
    label: str,
    *,
    max_bytes: int,
) -> tuple[str | None, list[str]]:
    errors = _validate_path_shape(path, label)
    if errors:
        return None, errors
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
    if expected_stat.st_nlink > 1:
        return None, [f"{label} must not be hardlinked"]
    if expected_stat.st_size > max_bytes:
        return None, [f"{label} must be no more than {max_bytes} bytes"]
    expected_identity = _file_identity(expected_stat)
    chunks: list[bytes] = []
    size = 0
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if (
                _file_identity(open_stat) != expected_identity
                or _file_identity(path_stat) != expected_identity
            ):
                return None, [f"{label} changed while being read"]
            if not stat.S_ISREG(open_stat.st_mode):
                return None, [f"{label} must be a regular file"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > max_bytes:
                    return None, [f"{label} must be no more than {max_bytes} bytes"]
                chunks.append(chunk)
            final_stat = path.lstat()
            if _file_identity(final_stat) != expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} could not be read"]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [f"{label} could not be read"]


def _read_regular_file_bytes(
    path: Path,
    label: str,
    *,
    max_bytes: int,
) -> tuple[bytes | None, list[str]]:
    errors = _validate_path_shape(path, label)
    if errors:
        return None, errors
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
    if expected_stat.st_nlink > 1:
        return None, [f"{label} must not be hardlinked"]
    if expected_stat.st_size > max_bytes:
        return None, [f"{label} must be no more than {max_bytes} bytes"]
    expected_identity = _file_identity(expected_stat)
    chunks: list[bytes] = []
    size = 0
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if (
                _file_identity(open_stat) != expected_identity
                or _file_identity(path_stat) != expected_identity
            ):
                return None, [f"{label} changed while being read"]
            if not stat.S_ISREG(open_stat.st_mode):
                return None, [f"{label} must be a regular file"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > max_bytes:
                    return None, [f"{label} must be no more than {max_bytes} bytes"]
                chunks.append(chunk)
            final_stat = path.lstat()
            if _file_identity(final_stat) != expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} could not be read"]
    return b"".join(chunks), []


def _sha256_regular_file(
    path: Path,
    label: str,
    *,
    max_bytes: int,
) -> tuple[str | None, list[str]]:
    payload, errors = _read_regular_file_bytes(path, label, max_bytes=max_bytes)
    if errors or payload is None:
        return None, errors
    return hashlib.sha256(payload).hexdigest(), []


def _run_step(
    *,
    label: str,
    command: Sequence[str],
    cwd: Path | None,
    env: dict[str, str] | None,
    timeout_seconds: int,
    runner: Runner,
) -> list[str]:
    try:
        result = runner(
            list(command),
            cwd=str(cwd) if cwd is not None else None,
            env=env,
            timeout=timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return [f"{label} timed out after {timeout_seconds} seconds"]
    except OSError:
        return [f"{label} could not be started"]
    if result.returncode != 0:
        return [
            f"{label} failed with exit code {result.returncode}: "
            f"{_safe_command_display(command)}"
        ]
    return []


def _strict_json_load(path: Path, label: str) -> tuple[dict[str, Any] | None, list[str]]:
    text, errors = _read_regular_text_file(
        path,
        label,
        max_bytes=MAX_CAPTURE_JSON_BYTES,
    )
    if errors or text is None:
        return None, errors
    try:
        payload = json.loads(
            text,
            object_pairs_hook=device_lab._reject_duplicate_json_object_pairs,
            parse_constant=device_lab._reject_nonfinite_json_constant,
        )
    except device_lab.DuplicateJsonKeyError as exc:
        return None, [f"{label} contains duplicate JSON object key {device_lab._display_path(exc.key)}"]
    except device_lab.NonFiniteJsonConstantError as exc:
        return None, [f"{label} is not strict JSON: non-finite constant {exc.constant} is not allowed"]
    except json.JSONDecodeError:
        return None, [f"{label} is not valid JSON"]
    if not isinstance(payload, dict):
        return None, [f"{label} must be a JSON object"]
    return payload, []


def _read_raw_pull_summary(
    raw_root: Path,
    summary_path: Path,
) -> tuple[str | None, Path | None, list[str]]:
    summary, errors = _strict_json_load(summary_path, "raw pull summary")
    if errors or summary is None:
        return None, None, errors
    if summary.get("schema") != raw_puller.RAW_PULL_SUMMARY_SCHEMA:
        return None, None, ["raw pull summary schema mismatch"]
    slot_id = summary.get("slot_id")
    if not isinstance(slot_id, str):
        return None, None, ["raw pull summary slot_id must be a string"]
    validated, slot_errors = device_lab.validate_slot_ids([slot_id])
    if slot_errors or not validated:
        return None, None, slot_errors or ["raw pull summary slot_id is invalid"]
    slot_id = validated[0]
    slot_path_value = summary.get("slot_path")
    if not isinstance(slot_path_value, str) or not slot_path_value:
        return None, None, ["raw pull summary slot_path must be a non-empty string"]
    if device_lab.SECRET_RE.search(slot_path_value) or device_lab._contains_control_character(slot_path_value):
        return None, None, ["raw pull summary slot_path must be safe"]
    slot_path = Path(slot_path_value)
    expected_slot_path = raw_root / slot_id
    if slot_path != expected_slot_path:
        return None, None, ["raw pull summary slot_path must match --raw-root and slot_id"]
    return slot_id, slot_path, []


def _read_raw_challenge(slot_path: Path) -> tuple[str | None, str | None, list[str]]:
    challenge_path = slot_path / "attestation" / "challenge.hex"
    text, errors = _read_regular_text_file(
        challenge_path,
        "attestation/challenge.hex",
        max_bytes=MAX_CAPTURE_CHALLENGE_BYTES,
    )
    if errors or text is None:
        return None, None, errors
    if not text.endswith("\n") or text.count("\n") != 1:
        return None, None, [
            "attestation/challenge.hex must be canonical lowercase hexadecimal plus trailing newline"
        ]
    challenge_hex = text[:-1]
    if (
        not challenge_hex
        or len(challenge_hex) % 2 != 0
        or any(char not in "0123456789abcdef" for char in challenge_hex)
    ):
        return None, None, [
            "attestation/challenge.hex must be canonical lowercase hexadecimal plus trailing newline"
        ]
    challenge_sha256 = hashlib.sha256(bytes.fromhex(challenge_hex)).hexdigest()
    return challenge_hex, challenge_sha256, []


def _validate_attestation_result_for_capture(
    result: dict[str, Any],
    *,
    slot_id: str,
    expected_app_package_name: str,
    expected_challenge_sha256: str,
    expected_certificate_chain_sha256: str,
) -> list[str]:
    errors: list[str] = []
    for field in sorted(set(result) - raw_puller.RAW_RESULT_ALLOWED_FIELDS):
        errors.append(
            "attestation result contains unexpected field "
            f"{device_lab._display_path(field)}"
        )
    if result.get("slot_id") != slot_id:
        errors.append("attestation result slot_id must match raw pull summary slot_id")
    if result.get("slot") != slot_id:
        errors.append("attestation result slot must match raw pull summary slot_id")
    if result.get("status") != "ok":
        errors.append("attestation result status must be ok")
    if result.get("strongbox_attestation") is not True:
        errors.append("attestation result strongbox_attestation must be true")
    if result.get("physical_device_attestation") is not True:
        errors.append("attestation result physical_device_attestation must be true")
    if result.get("app_package_name") != expected_app_package_name:
        errors.append("attestation result app_package_name must match --run-as-package")
    if result.get("attestation_challenge_sha256") != expected_challenge_sha256:
        errors.append("attestation result attestation_challenge_sha256 must match attestation/challenge.hex")
    if result.get("attestation_certificate_chain_path") != "attestation/keymint-certificate-chain.pem":
        errors.append(
            "attestation result attestation_certificate_chain_path must be "
            "attestation/keymint-certificate-chain.pem"
        )
    if result.get("attestation_certificate_chain_sha256") != expected_certificate_chain_sha256:
        errors.append(
            "attestation result attestation_certificate_chain_sha256 must match "
            "attestation/keymint-certificate-chain.pem"
        )
    for field in raw_puller.RAW_RESULT_STRING_FIELDS:
        errors.extend(_validate_cli_string(result.get(field), f"attestation result {field}"))
    for field in raw_puller.RAW_RESULT_STRONGBOX_FIELDS:
        if result.get(field) != "STRONGBOX":
            errors.append(f"attestation result {field} must be STRONGBOX")
    for field in raw_puller.RAW_RESULT_SHA256_FIELDS:
        value = result.get(field)
        if (
            not isinstance(value, str)
            or len(value) != 64
            or any(character not in "0123456789abcdef" for character in value)
        ):
            errors.append(f"attestation result {field} must be a lowercase SHA-256 hex digest")
        elif value == "0" * 64:
            errors.append(f"attestation result {field} must be a non-zero lowercase SHA-256 hex digest")
    return errors


def _capture_env(args: argparse.Namespace) -> dict[str, str]:
    env = os.environ.copy()
    env["ANDROID_SERIAL"] = args.serial
    if args.java_home is not None:
        env["JAVA_HOME"] = args.java_home
    if args.android_home is not None:
        env["ANDROID_HOME"] = args.android_home
    if args.android_sdk_root is not None:
        env["ANDROID_SDK_ROOT"] = args.android_sdk_root
    return env


def _gradle_command(args: argparse.Namespace) -> list[str]:
    return [
        args.gradlew,
        *GRADLE_LAB_APP_TASKS,
        "--console=plain",
    ]


def _instrumentation_command(args: argparse.Namespace) -> list[str]:
    return [
        args.adb,
        "-s",
        args.serial,
        "shell",
        "am",
        "instrument",
        "-w",
        "-e",
        "class",
        DEVICE_LAB_EXPORT_TEST_CLASS,
        args.instrumentation_runner,
    ]


def _raw_pull_command(args: argparse.Namespace, raw_summary_path: Path) -> list[str]:
    return [
        args.python,
        str(args.repo_root / "scripts" / "kagemusha_pull_android_device_lab_raw_slot.py"),
        "--adb",
        args.adb,
        "--serial",
        args.serial,
        "--run-as-package",
        args.run_as_package,
        "--device-lab-root",
        args.device_lab_root,
        "--out-root",
        str(args.raw_root),
        "--summary-out",
        str(raw_summary_path),
        "--adb-timeout-seconds",
        str(args.adb_timeout_seconds),
    ]


def _attestation_report_command(
    args: argparse.Namespace,
    *,
    slot_id: str,
    slot_path: Path,
    result: dict[str, Any],
    challenge_hex: str,
    challenge_sha256: str,
) -> list[str]:
    return [
        args.python,
        str(args.repo_root / "scripts" / "kagemusha_android_attestation_report.py"),
        "--harness-result",
        str(slot_path / "attestation" / "harness-result.json"),
        "--slot-id",
        slot_id,
        "--device-fingerprint",
        str(result["device_fingerprint"]),
        "--os-build-id",
        str(result["os_build_id"]),
        "--app-package-name",
        str(result["app_package_name"]),
        "--attestation-certificate-chain",
        str(slot_path / "attestation" / "keymint-certificate-chain.pem"),
        "--attestation-certificate-chain-path",
        "attestation/keymint-certificate-chain.pem",
        "--attestation-challenge-sha256",
        challenge_sha256,
        "--expected-challenge-hex",
        challenge_hex,
        "--physical-device-attestation",
        "--out",
        str(slot_path / "attestation" / "report.json"),
    ]


def _assemble_command(
    args: argparse.Namespace,
    *,
    slot_id: str,
    slot_path: Path,
) -> list[str]:
    command = [
        args.python,
        str(args.repo_root / "scripts" / "kagemusha_android_device_lab_slot.py"),
        "--slot-root",
        str(args.slot_root),
        "--slot-id",
        slot_id,
        "--adb",
        args.adb,
        "--serial",
        args.serial,
        "--attestation-result",
        str(slot_path / "attestation" / "result.json"),
        "--attestation-harness-result",
        str(slot_path / "attestation" / "harness-result.json"),
        "--attestation-report",
        str(slot_path / "attestation" / "report.json"),
        "--attestation-certificate-chain",
        str(slot_path / "attestation" / "keymint-certificate-chain.pem"),
        "--offline-wallet-apk",
        str(args.offline_wallet_apk),
        "--d2d-payment-transcript",
        str(slot_path / PRIMARY_D2D_TRANSCRIPT),
    ]
    for transport, relative in EXTRA_D2D_TRANSCRIPTS:
        command.extend(
            [
                "--d2d-payment-transcript-extra",
                f"{transport}={slot_path / relative}",
            ]
        )
    command.extend(
        [
            "--wallet-integrity-transcript",
            str(slot_path / "wallet" / "integrity.json"),
            "--telemetry-json",
            str(slot_path / "telemetry" / "telemetry.json"),
            "--status-ndjson",
            str(slot_path / "telemetry" / "status.ndjson"),
            "--pending-queue-json",
            str(slot_path / "queue" / "pending_queue.json"),
            "--runtime-log",
            str(slot_path / "logs" / "runtime.log"),
            "--private-key",
            str(args.private_key),
            "--public-key",
            str(args.public_key),
            "--signer-key-id",
            args.signer_key_id,
        ]
    )
    return command


def _validation_command(
    args: argparse.Namespace,
    *,
    slot_id: str,
    validation_summary_path: Path,
) -> list[str]:
    command = [
        args.python,
        str(args.repo_root / "scripts" / "check_android_device_lab_slot.py"),
        "--root",
        str(args.slot_root),
        "--slot",
        slot_id,
        "--require-slot",
        "--require-kagemusha-production-evidence",
        "--trusted-signer-public-key",
        str(args.public_key),
        "--json-out",
        str(validation_summary_path),
    ]
    if args.require_standard_matrix:
        command.append("--require-kagemusha-standard-matrix")
    return command


def _validate_preflight(args: argparse.Namespace) -> list[str]:
    errors: list[str] = []
    for value, label in (
        (args.python, "python executable"),
        (args.adb, "adb executable"),
        (args.gradlew, "Gradle wrapper"),
        (args.serial, "ADB serial"),
        (args.run_as_package, "run-as package"),
        (args.device_lab_root, "device lab root"),
        (args.instrumentation_runner, "instrumentation runner"),
        (args.signer_key_id, "signer key id"),
    ):
        errors.extend(_validate_cli_string(value, label))
    for value, label in (
        (args.java_home, "JAVA_HOME"),
        (args.android_home, "ANDROID_HOME"),
        (args.android_sdk_root, "ANDROID_SDK_ROOT"),
    ):
        if value is not None:
            errors.extend(_validate_cli_string(value, label))
    for path, label in (
        (args.repo_root, "--repo-root"),
        (args.kotlin_dir, "--kotlin-dir"),
        (args.raw_root, "--raw-root"),
        (args.slot_root, "--slot-root"),
        (args.raw_summary_out, "--raw-summary-out"),
        (args.validation_summary_out, "--validation-summary-out"),
        (args.capture_summary_out, "--capture-summary-out"),
        (args.private_key, "--private-key"),
        (args.public_key, "--public-key"),
        (args.offline_wallet_apk, "--offline-wallet-apk"),
    ):
        if path is not None:
            errors.extend(_validate_path_shape(path, label))
    if not args.physical_device_attestation:
        errors.append("--physical-device-attestation is required for production capture")
    for seconds, label in (
        (args.gradle_timeout_seconds, "--gradle-timeout-seconds"),
        (args.instrumentation_timeout_seconds, "--instrumentation-timeout-seconds"),
        (args.adb_timeout_seconds, "--adb-timeout-seconds"),
        (args.helper_timeout_seconds, "--helper-timeout-seconds"),
    ):
        if seconds <= 0:
            errors.append(f"{label} must be positive")
    return errors


def write_capture_summary(path: Path, payload: dict[str, Any]) -> list[str]:
    errors = _validate_path_shape(path, "capture summary output")
    if errors:
        return errors
    try:
        encoded = _json_dumps(payload).encode("utf-8")
    except ValueError:
        return ["capture summary output is not strict JSON"]
    if len(encoded) > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
        return [
            "capture summary output must be no more than "
            f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
        ]
    parent = path.parent
    parent_fd, parent_identity, parent_errors = _open_capture_summary_parent(parent)
    if parent_errors:
        return parent_errors
    assert parent_fd is not None
    assert parent_identity is not None
    temp_name: str | None = None
    temp_identity: tuple[int, int] | None = None
    output_identity: tuple[int, int] | None = None
    temp_fd: int | None = None
    try:
        try:
            output_mode = os.stat(
                path.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            ).st_mode
        except FileNotFoundError:
            pass
        except OSError:
            return ["capture summary output metadata could not be read"]
        else:
            if stat.S_ISLNK(output_mode):
                return ["capture summary output must not be a symlink"]
            if not stat.S_ISREG(output_mode):
                return ["capture summary output must be a regular file"]
            try:
                if os.stat(path.name, dir_fd=parent_fd).st_nlink > 1:
                    return ["capture summary output must not be hardlinked"]
            except OSError:
                return ["capture summary output hardlink metadata could not be read"]
        temp_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        if hasattr(os, "O_NOFOLLOW"):
            temp_flags |= os.O_NOFOLLOW
        for _attempt in range(100):
            candidate = f".{path.name}.{secrets.token_hex(8)}.tmp"
            try:
                temp_fd = os.open(candidate, temp_flags, 0o600, dir_fd=parent_fd)
            except FileExistsError:
                continue
            except OSError:
                return ["capture summary output could not be written"]
            temp_name = candidate
            break
        if temp_fd is None or temp_name is None:
            return ["capture summary output could not be written"]
        os.fchmod(temp_fd, 0o600)
        temp_identity = _file_identity(os.fstat(temp_fd))
        _write_all(temp_fd, encoded)
        os.fsync(temp_fd)
        os.close(temp_fd)
        temp_fd = None
        os.replace(
            temp_name,
            path.name,
            src_dir_fd=parent_fd,
            dst_dir_fd=parent_fd,
        )
        temp_name = None
        try:
            expected_stat = os.stat(
                path.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except OSError:
            return ["capture summary output could not be read back after writing"]
        if stat.S_ISLNK(expected_stat.st_mode):
            return ["capture summary output must not be a symlink after writing"]
        if not stat.S_ISREG(expected_stat.st_mode):
            return ["capture summary output must be a regular file after writing"]
        if expected_stat.st_nlink > 1:
            return ["capture summary output must not be hardlinked after writing"]
        if stat.S_IMODE(expected_stat.st_mode) != 0o600:
            return ["capture summary output permissions must be 0600"]
        if expected_stat.st_size > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
            return [
                "capture summary output must be no more than "
                f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
            ]
        output_identity = _file_identity(expected_stat)
        read_flags = os.O_RDONLY
        if hasattr(os, "O_NOFOLLOW"):
            read_flags |= os.O_NOFOLLOW
        try:
            readback_fd = os.open(path.name, read_flags, dir_fd=parent_fd)
        except OSError:
            return ["capture summary output could not be read back after writing"]
        chunks: list[bytes] = []
        readback_size = 0
        try:
            open_stat = os.fstat(readback_fd)
            if _file_identity(open_stat) != output_identity:
                return ["capture summary output changed while being read back"]
            if not stat.S_ISREG(open_stat.st_mode):
                return ["capture summary output must be a regular file after writing"]
            if open_stat.st_nlink > 1:
                return ["capture summary output must not be hardlinked after writing"]
            if stat.S_IMODE(open_stat.st_mode) != 0o600:
                return ["capture summary output permissions must be 0600"]
            if open_stat.st_size > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
                return [
                    "capture summary output must be no more than "
                    f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
                ]
            while True:
                chunk = os.read(readback_fd, 1024 * 1024)
                if not chunk:
                    break
                readback_size += len(chunk)
                if readback_size > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
                    return [
                        "capture summary output must be no more than "
                        f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
                    ]
                chunks.append(chunk)
        except OSError:
            return ["capture summary output could not be read back after writing"]
        finally:
            os.close(readback_fd)
        try:
            final_stat = os.stat(
                path.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except OSError:
            return ["capture summary output could not be read back after writing"]
        if _file_identity(final_stat) != output_identity:
            return ["capture summary output changed while being read back"]
        if b"".join(chunks) != encoded:
            return ["capture summary output readback mismatch"]
        try:
            current_parent_stat = parent.lstat()
        except OSError:
            cleanup_errors: list[str] = []
            if output_identity is not None:
                cleanup_errors.extend(
                    _unlink_file_if_identity_at(parent_fd, path.name, output_identity)
                )
            return [
                "capture summary output parent directory could not be synced",
                *cleanup_errors,
            ]
        if _file_identity(current_parent_stat) != parent_identity:
            cleanup_errors = []
            if output_identity is not None:
                cleanup_errors.extend(
                    _unlink_file_if_identity_at(parent_fd, path.name, output_identity)
                )
            return [
                "capture summary output parent directory could not be synced",
                *cleanup_errors,
            ]
        sync_errors = _sync_directory(
            parent,
            "capture summary output parent directory could not be synced",
            expected_identity=parent_identity,
        )
        if sync_errors:
            cleanup_errors: list[str] = []
            if output_identity is not None:
                cleanup_errors.extend(
                    _unlink_file_if_identity_at(parent_fd, path.name, output_identity)
                )
            return [*sync_errors, *cleanup_errors]
    except OSError:
        cleanup_errors: list[str] = []
        if temp_fd is not None:
            try:
                os.close(temp_fd)
            except OSError:
                pass
        if temp_name is not None and temp_identity is not None:
            cleanup_errors.extend(
                _unlink_file_if_identity_at(parent_fd, temp_name, temp_identity)
            )
        if output_identity is not None:
            cleanup_errors.extend(
                _unlink_file_if_identity_at(parent_fd, path.name, output_identity)
            )
        return ["capture summary output could not be written", *cleanup_errors]
    finally:
        os.close(parent_fd)
    return []


def capture_device_lab_slot(
    args: argparse.Namespace,
    *,
    runner: Runner = subprocess.run,
) -> tuple[int, dict[str, Any] | None, list[str]]:
    """Run the one-device physical Android capture pipeline."""

    raw_summary_path = args.raw_summary_out or _default_raw_summary_path(args.raw_root)
    validation_summary_path = (
        args.validation_summary_out or _default_validation_summary_path(args.slot_root)
    )
    args.raw_summary_out = raw_summary_path
    args.validation_summary_out = validation_summary_path

    errors = _validate_preflight(args)
    if errors:
        return 1, None, errors

    env = _capture_env(args)
    if not args.skip_build_install:
        errors = _run_step(
            label="Android lab app build/install",
            command=_gradle_command(args),
            cwd=args.repo_root / args.kotlin_dir,
            env=env,
            timeout_seconds=args.gradle_timeout_seconds,
            runner=runner,
        )
        if errors:
            return 1, None, errors

    if not args.skip_instrumentation:
        errors = _run_step(
            label="Kagemusha device-lab instrumentation export",
            command=_instrumentation_command(args),
            cwd=args.repo_root,
            env=env,
            timeout_seconds=args.instrumentation_timeout_seconds,
            runner=runner,
        )
        if errors:
            return 1, None, errors

    errors = _run_step(
        label="raw Android device-lab pull",
        command=_raw_pull_command(args, raw_summary_path),
        cwd=args.repo_root,
        env=env,
        timeout_seconds=args.helper_timeout_seconds,
        runner=runner,
    )
    if errors:
        return 1, None, errors

    slot_id, raw_slot_path, errors = _read_raw_pull_summary(args.raw_root, raw_summary_path)
    if errors or slot_id is None or raw_slot_path is None:
        return 1, None, errors
    challenge_hex, challenge_sha256, errors = _read_raw_challenge(raw_slot_path)
    if errors or challenge_hex is None or challenge_sha256 is None:
        return 1, None, errors
    attestation_result, errors = _strict_json_load(
        raw_slot_path / "attestation" / "result.json",
        "attestation result",
    )
    if errors or attestation_result is None:
        return 1, None, errors
    certificate_chain_sha256, errors = _sha256_regular_file(
        raw_slot_path / "attestation" / "keymint-certificate-chain.pem",
        "attestation/keymint-certificate-chain.pem",
        max_bytes=device_lab.MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES,
    )
    if errors or certificate_chain_sha256 is None:
        return 1, None, errors
    errors = _validate_attestation_result_for_capture(
        attestation_result,
        slot_id=slot_id,
        expected_app_package_name=args.run_as_package,
        expected_challenge_sha256=challenge_sha256,
        expected_certificate_chain_sha256=certificate_chain_sha256,
    )
    if errors:
        return 1, None, errors

    errors = _run_step(
        label="Kagemusha attestation report render",
        command=_attestation_report_command(
            args,
            slot_id=slot_id,
            slot_path=raw_slot_path,
            result=attestation_result,
            challenge_hex=challenge_hex,
            challenge_sha256=challenge_sha256,
        ),
        cwd=args.repo_root,
        env=env,
        timeout_seconds=args.helper_timeout_seconds,
        runner=runner,
    )
    if errors:
        return 1, None, errors

    errors = _run_step(
        label="signed Android device-lab slot assembly",
        command=_assemble_command(args, slot_id=slot_id, slot_path=raw_slot_path),
        cwd=args.repo_root,
        env=env,
        timeout_seconds=args.helper_timeout_seconds,
        runner=runner,
    )
    if errors:
        return 1, None, errors

    errors = _run_step(
        label="signed Android device-lab slot validation",
        command=_validation_command(
            args,
            slot_id=slot_id,
            validation_summary_path=validation_summary_path,
        ),
        cwd=args.repo_root,
        env=env,
        timeout_seconds=args.helper_timeout_seconds,
        runner=runner,
    )
    if errors:
        return 1, None, errors

    summary = {
        "schema": CAPTURE_SUMMARY_SCHEMA,
        "captured_at_utc": _utc_now(),
        "slot_id": slot_id,
        "adb_serial": args.serial,
        "raw_slot_path": str(raw_slot_path),
        "slot_path": str(args.slot_root / slot_id),
        "raw_summary_path": str(raw_summary_path),
        "validation_summary_path": str(validation_summary_path),
        "required_d2d_payment_transports": sorted(device_lab.D2D_PAYMENT_TRANSPORTS),
        "physical_device_attestation": True,
        "standard_matrix_required": bool(args.require_standard_matrix),
    }
    if args.capture_summary_out is not None:
        summary_errors = write_capture_summary(args.capture_summary_out, summary)
        if summary_errors:
            return 1, None, summary_errors
    return 0, summary, []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run the physical Android Kagemusha device-lab capture pipeline for one "
            "attached device without modifying other running jobs."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=Path("."))
    parser.add_argument("--kotlin-dir", type=Path, default=Path("kotlin"))
    parser.add_argument("--python", default=sys.executable)
    parser.add_argument("--adb", default="adb")
    parser.add_argument("--gradlew", default="./gradlew")
    parser.add_argument("--serial", required=True)
    parser.add_argument("--java-home")
    parser.add_argument("--android-home")
    parser.add_argument("--android-sdk-root")
    parser.add_argument("--run-as-package", default=DEFAULT_APP_PACKAGE_NAME)
    parser.add_argument("--device-lab-root", default=raw_puller.DEFAULT_DEVICE_LAB_DEVICE_ROOT)
    parser.add_argument("--instrumentation-runner", default=DEFAULT_INSTRUMENTATION_RUNNER)
    parser.add_argument("--raw-root", type=Path, required=True)
    parser.add_argument("--slot-root", type=Path, required=True)
    parser.add_argument("--raw-summary-out", type=Path)
    parser.add_argument("--validation-summary-out", type=Path)
    parser.add_argument("--capture-summary-out", type=Path)
    parser.add_argument("--offline-wallet-apk", type=Path, default=DEFAULT_OFFLINE_WALLET_APK)
    parser.add_argument("--private-key", type=Path, required=True)
    parser.add_argument("--public-key", type=Path, required=True)
    parser.add_argument("--signer-key-id", required=True)
    parser.add_argument(
        "--physical-device-attestation",
        action="store_true",
        help="Required assertion that this capture is from a physical Android device.",
    )
    parser.add_argument(
        "--require-standard-matrix",
        action="store_true",
        help="Validate the full standard matrix after assembling this slot.",
    )
    parser.add_argument("--skip-build-install", action="store_true")
    parser.add_argument("--skip-instrumentation", action="store_true")
    parser.add_argument("--gradle-timeout-seconds", type=int, default=1200)
    parser.add_argument("--instrumentation-timeout-seconds", type=int, default=300)
    parser.add_argument("--adb-timeout-seconds", type=int, default=120)
    parser.add_argument("--helper-timeout-seconds", type=int, default=300)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    status, summary, errors = capture_device_lab_slot(args)
    if errors:
        for error in errors:
            print(f"[kagemusha-android-device-lab-capture] {error}", file=sys.stderr)
        return status
    assert summary is not None
    print(_json_dumps(summary), end="")
    return status


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
