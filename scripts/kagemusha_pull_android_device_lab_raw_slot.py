#!/usr/bin/env python3
"""Pull raw Kagemusha Android device-lab artifacts from an attached device."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import io
import json
import os
from pathlib import Path
from pathlib import PurePosixPath
import secrets
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
from typing import Any, Callable, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402


RAW_PULL_SUMMARY_SCHEMA = "iroha.android.device_lab.kagemusha.raw_pull.v1"
DEFAULT_RUN_AS_PACKAGE = "org.hyperledger.iroha.sdk.offline.wallet.lab"
DEFAULT_DEVICE_LAB_DEVICE_ROOT = "files/kagemusha-device-lab"
DEFAULT_OUT_ROOT = Path("target/kagemusha-android-raw")
MAX_RAW_SLOT_TAR_BYTES = 128 * 1024 * 1024
MAX_RAW_SLOT_FILE_BYTES = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
MAX_RAW_SLOT_ENTRIES = 256
MAX_RAW_SLOT_FILES = 128
RAW_SLOT_REQUIRED_PATHS: tuple[str, ...] = (
    "attestation/challenge.hex",
    "attestation/harness-result.json",
    "attestation/keymint-certificate-chain.pem",
    "attestation/result.json",
    "handoff/d2d-payment.json",
    "handoff/d2d-payment-nfc_hce.json",
    "handoff/d2d-payment-qr.json",
    "wallet/integrity.json",
    "telemetry/telemetry.json",
    "telemetry/status.ndjson",
    "queue/pending_queue.json",
    "logs/runtime.log",
)
RAW_D2D_PAYMENT_TRANSCRIPT_TRANSPORTS: dict[str, str] = {
    "handoff/d2d-payment.json": "nearby_offline",
    "handoff/d2d-payment-nfc_hce.json": "nfc_hce",
    "handoff/d2d-payment-qr.json": "qr",
}
RAW_SLOT_ALLOWED_PATHS: frozenset[str] = frozenset(RAW_SLOT_REQUIRED_PATHS)
RAW_SLOT_ALLOWED_DIRECTORIES: frozenset[str] = frozenset(
    {
        "attestation",
        "handoff",
        "wallet",
        "telemetry",
        "queue",
        "logs",
    }
)
PENDING_QUEUE_FIELDS: frozenset[str] = frozenset(
    {
        "slot_id",
        "pending_transactions",
    }
)
TELEMETRY_FIELDS: frozenset[str] = frozenset(
    {
        "schema_version",
        "slot_id",
        "suite",
        "device_model",
        "device_codename",
        "app_package_name",
    }
)
TELEMETRY_STRING_FIELDS: tuple[str, ...] = (
    "device_model",
    "device_codename",
    "app_package_name",
)
HARNESS_RESULT_ALLOWED_FIELDS: frozenset[str] = frozenset(
    {
        "alias",
        "attestation_security_level",
        "keymaster_security_level",
        "strongbox_attestation",
        "challenge_hex",
        "chain_length",
    }
)
RAW_RESULT_ALLOWED_FIELDS: frozenset[str] = frozenset(
    {
        "slot",
        "slot_id",
        "status",
        "device_fingerprint",
        "os_build_id",
        "app_package_name",
        "app_signing_certificate_sha256",
        "attestation_challenge_sha256",
        "attestation_certificate_chain_path",
        "attestation_certificate_chain_sha256",
        "offline_wallet_policy_sha256",
        "attestation_security_level",
        "keymaster_security_level",
        "keymint_security_level",
        "strongbox_attestation",
        "physical_device_attestation",
    }
)
RAW_RESULT_STRING_FIELDS: tuple[str, ...] = (
    "device_fingerprint",
    "os_build_id",
    "app_package_name",
)
RAW_RESULT_APP_SIGNING_DIGEST_FIELD = "app_signing_certificate_sha256"
RAW_RESULT_CHALLENGE_DIGEST_FIELD = "attestation_challenge_sha256"
RAW_RESULT_CHAIN_DIGEST_FIELD = "attestation_certificate_chain_sha256"
RAW_RESULT_POLICY_DIGEST_FIELD = "offline_wallet_policy_sha256"
RAW_RESULT_SHA256_FIELDS: tuple[str, ...] = (
    RAW_RESULT_APP_SIGNING_DIGEST_FIELD,
    RAW_RESULT_CHALLENGE_DIGEST_FIELD,
    RAW_RESULT_CHAIN_DIGEST_FIELD,
    RAW_RESULT_POLICY_DIGEST_FIELD,
)
RAW_RESULT_STRONGBOX_FIELDS: tuple[str, ...] = (
    "attestation_security_level",
    "keymaster_security_level",
    "keymint_security_level",
)
ADB_LATEST_SLOT_COMMAND_HELP = (
    "adb shell run-as <package> cat files/kagemusha-device-lab/latest-slot.txt"
)
ADB_PULL_TAR_COMMAND_HELP = (
    "adb exec-out run-as <package> tar -C files/kagemusha-device-lab "
    "-cf - <slot-id> latest-slot.txt"
)
MAX_ADB_COMMAND_DISPLAY_CHARS = 240
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


Runner = Callable[..., subprocess.CompletedProcess]


def _timeout_arg(timeout_seconds: int) -> int | None:
    return None if timeout_seconds == 0 else timeout_seconds


def _json_dumps(payload: dict[str, Any]) -> str:
    return json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"


def _is_adb_executable(command: Sequence[str]) -> bool:
    if not command:
        return False
    executable = str(command[0]).replace("\\", "/").rsplit("/", 1)[-1].lower()
    return executable in {"adb", "adb.exe"}


CONTROL_OUTPUT_REDACTION = "<unsafe-adb-output>"
NON_UTF8_OUTPUT_REDACTION = "<non-utf8-adb-output>"
ADB_SERIAL_REDACTION = "<redacted-adb-serial>"


def _safe_detail(
    value: object,
    limit: int = 512,
    *,
    redact_tokens: Sequence[str] = (),
) -> str:
    if isinstance(value, bytes):
        try:
            value = value.decode("utf-8")
        except UnicodeDecodeError:
            return NON_UTF8_OUTPUT_REDACTION
    if not isinstance(value, str):
        return ""
    text = value
    text = text.replace("\r", "\n").strip()
    if not text:
        return ""
    if device_lab.SECRET_RE.search(text):
        return "<redacted-secret-output>"
    if device_lab._contains_control_character(text):
        return CONTROL_OUTPUT_REDACTION
    for token in redact_tokens:
        if token:
            text = text.replace(token, ADB_SERIAL_REDACTION)
    if len(text) > limit:
        return text[:limit] + "...<truncated>"
    return text


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
    normalized_tokens = [token.casefold() for token in tokens]
    executable = tokens[0].replace("\\", "/").rsplit("/", 1)[-1].casefold()
    if executable in DISRUPTIVE_EXECUTABLE_NAMES:
        return [
            f"{label} must not manage other running jobs: "
            f"{_safe_adb_command_display(tokens)}"
        ]
    if any(token in DISRUPTIVE_COMMAND_TOKENS for token in normalized_tokens):
        return [
            f"{label} must not manage other running jobs: "
            f"{_safe_adb_command_display(tokens)}"
        ]
    for sequence in DISRUPTIVE_TOKEN_SEQUENCES:
        width = len(sequence)
        for index in range(0, len(normalized_tokens) - width + 1):
            if tuple(normalized_tokens[index : index + width]) == sequence:
                return [
                    f"{label} must not manage other running jobs: "
                    f"{_safe_adb_command_display(tokens)}"
                ]
    return []


def _single_safe_slot_id(raw_slot_id: str) -> tuple[str | None, list[str]]:
    normalised, errors = device_lab.validate_slot_ids([raw_slot_id])
    if errors:
        return None, errors
    if not normalised or len(normalised) != 1:
        return None, ["slot id must be a single safe directory name"]
    return normalised[0], []


def _validate_non_secret_adb_string(value: object, label: str) -> list[str]:
    if not isinstance(value, str) or not value:
        return [f"{label} must be a non-empty string"]
    if value != value.strip():
        return [f"{label} must not contain surrounding whitespace"]
    if device_lab._contains_control_character(value):
        return [f"{label} must not contain control characters"]
    if device_lab.SECRET_RE.search(value):
        return [f"{label} must not contain secret-looking material"]
    return []


def _path_shape_errors(path: Path, label: str) -> list[str]:
    text = str(path)
    if device_lab.SECRET_RE.search(text):
        return [f"{label} must not contain secret-looking material"]
    if device_lab._contains_control_character(text):
        return [f"{label} must not contain control characters"]
    if text != text.strip() or device_lab._path_has_surrounding_whitespace_component(  # type: ignore[attr-defined]
        path
    ):
        return [f"{label} must not contain surrounding whitespace"]
    if "\\" in text:
        if label == "raw output root path":
            return ["raw output root path must not contain backslashes"]
        return [f"{label} must not contain backslashes"]
    if ".." in path.parts:
        if label == "raw output root path":
            return ["raw output root path must be canonical"]
        return [f"{label} must be canonical"]
    return []


def _pem_certificate_count(chain_text: str) -> int:
    return chain_text.count("-----BEGIN CERTIFICATE-----")


def _validate_sha256_hex(value: object, label: str, errors: list[str]) -> str | None:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(char not in "0123456789abcdef" for char in value)
    ):
        errors.append(f"{label} must be a lowercase SHA-256 hex digest")
        return None
    if value == "0" * 64:
        errors.append(f"{label} must be a non-zero lowercase SHA-256 hex digest")
        return None
    return value


def _validate_raw_result_string(
    result: dict[str, Any],
    field: str,
    errors: list[str],
) -> str | None:
    value = result.get(field)
    label = f"attestation/result.json {field}"
    if not isinstance(value, str) or not value:
        errors.append(f"{label} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(f"{label} must not have surrounding whitespace")
        return None
    if device_lab._contains_control_character(value):
        errors.append(f"{label} must not contain control characters")
        return None
    if device_lab.SECRET_RE.search(value):
        errors.append(f"{label} must not contain secret-looking material")
        return None
    return value


def _validate_raw_json_slot_id(
    payload: dict[str, Any],
    label: str,
    slot_id: str,
    errors: list[str],
) -> None:
    slot_value = payload.get("slot_id")
    if not isinstance(slot_value, str) or not slot_value:
        errors.append(f"{label} slot_id must be a non-empty string")
        return
    if slot_value != slot_value.strip():
        errors.append(f"{label} slot_id must not contain surrounding whitespace")
        return
    if device_lab._contains_control_character(slot_value):
        errors.append(f"{label} slot_id must not contain control characters")
        return
    if slot_value != slot_id:
        errors.append(f"{label} slot_id must match slot id")


def _validate_raw_json_schema(
    payload: dict[str, Any],
    label: str,
    expected_schema: str,
    errors: list[str],
) -> None:
    if payload.get("schema") != expected_schema:
        errors.append(f"{label} schema must be {expected_schema}")


def _validate_raw_json_true(
    payload: dict[str, Any],
    label: str,
    field: str,
    errors: list[str],
) -> None:
    if payload.get(field) is not True:
        errors.append(f"{label} {field} must be true")


def _validate_raw_d2d_payment_transcript(
    slot_path: Path,
    relative: str,
    slot_id: str,
    errors: list[str],
) -> None:
    d2d = device_lab._load_json(slot_path / relative, relative, errors)
    if d2d is None:
        return
    _validate_raw_json_schema(
        d2d,
        relative,
        device_lab.D2D_PAYMENT_TRANSCRIPT_SCHEMA,
        errors,
    )
    _validate_raw_json_slot_id(d2d, relative, slot_id, errors)
    if d2d.get("payload_schema") != device_lab.D2D_PAYMENT_PAYLOAD_SCHEMA:
        errors.append(
            f"{relative} payload_schema must be {device_lab.D2D_PAYMENT_PAYLOAD_SCHEMA}"
        )
    expected_transport = RAW_D2D_PAYMENT_TRANSCRIPT_TRANSPORTS[relative]
    transport = d2d.get("transport")
    if transport != expected_transport:
        errors.append(f"{relative} transport must be {expected_transport}")
    elif transport not in device_lab.D2D_PAYMENT_TRANSPORTS:
        errors.append(f"{relative} transport must be an accepted offline transport")
    for field in (
        "transport_offline",
        "payer_wallet_offline",
        "payee_wallet_offline",
        "one_use_key_consumed",
        "receiver_redeem_accepted",
        "double_spend_rejected",
    ):
        _validate_raw_json_true(d2d, relative, field, errors)


def _validate_raw_json_artifacts(
    slot_path: Path,
    slot_id: str,
    errors: list[str],
    expected_app_package_name: str | None = None,
) -> None:
    queue = device_lab._load_json(
        slot_path / "queue" / "pending_queue.json",
        "queue/pending_queue.json",
        errors,
    )
    if queue is not None:
        for field in sorted(set(queue) - PENDING_QUEUE_FIELDS):
            errors.append(
                "queue/pending_queue.json contains unexpected field "
                f"{device_lab._display_path(field)}"
            )
        _validate_raw_json_slot_id(queue, "queue/pending_queue.json", slot_id, errors)
        pending_transactions = queue.get("pending_transactions")
        if not isinstance(pending_transactions, list):
            errors.append("queue/pending_queue.json pending_transactions must be an array")
        elif pending_transactions:
            errors.append(
                "queue/pending_queue.json pending_transactions must be empty after D2D handoff"
            )

    telemetry = device_lab._load_json(
        slot_path / "telemetry" / "telemetry.json",
        "telemetry/telemetry.json",
        errors,
    )
    if telemetry is not None:
        for field in sorted(set(telemetry) - TELEMETRY_FIELDS):
            errors.append(
                "telemetry/telemetry.json contains unexpected field "
                f"{device_lab._display_path(field)}"
            )
        if telemetry.get("schema_version") != 1:
            errors.append("telemetry/telemetry.json schema_version must be 1")
        _validate_raw_json_slot_id(telemetry, "telemetry/telemetry.json", slot_id, errors)
        suite = telemetry.get("suite")
        if not isinstance(suite, str) or not suite:
            errors.append("telemetry/telemetry.json suite must be a non-empty string")
        elif suite != suite.strip():
            errors.append("telemetry/telemetry.json suite must not contain surrounding whitespace")
        elif device_lab._contains_control_character(suite):
            errors.append("telemetry/telemetry.json suite must not contain control characters")
        elif suite != device_lab.KAGEMUSHA_TELEMETRY_SUITE:
            errors.append("telemetry/telemetry.json suite must identify a Kagemusha device-lab run")
        for field in TELEMETRY_STRING_FIELDS:
            errors.extend(
                _validate_non_secret_adb_string(
                    telemetry.get(field),
                    f"telemetry/telemetry.json {field}",
                )
            )
        telemetry_app_package_name = telemetry.get("app_package_name")
        if (
            expected_app_package_name is not None
            and isinstance(telemetry_app_package_name, str)
            and telemetry_app_package_name == telemetry_app_package_name.strip()
            and not device_lab._contains_control_character(telemetry_app_package_name)
            and not device_lab.SECRET_RE.search(telemetry_app_package_name)
            and telemetry_app_package_name != expected_app_package_name
        ):
            errors.append(
                "telemetry/telemetry.json app_package_name must match "
                "attestation/result.json app_package_name"
            )

    for relative in sorted(RAW_D2D_PAYMENT_TRANSCRIPT_TRANSPORTS):
        _validate_raw_d2d_payment_transcript(slot_path, relative, slot_id, errors)

    wallet = device_lab._load_json(
        slot_path / "wallet" / "integrity.json",
        "wallet/integrity.json",
        errors,
    )
    if wallet is not None:
        _validate_raw_json_schema(
            wallet,
            "wallet/integrity.json",
            device_lab.WALLET_INTEGRITY_TRANSCRIPT_SCHEMA,
            errors,
        )
        _validate_raw_json_slot_id(wallet, "wallet/integrity.json", slot_id, errors)
        if wallet.get("keymint_security_level") != "STRONGBOX":
            errors.append("wallet/integrity.json keymint_security_level must be STRONGBOX")
        for field in (
            "one_use_key_rotation_passed",
            "old_key_invalidated",
            "rollback_rejection_passed",
            "stale_snapshot_rejected",
            "active_wallet_state_preserved_after_reject",
        ):
            _validate_raw_json_true(wallet, "wallet/integrity.json", field, errors)


def _validate_raw_status_ndjson(status_text: str, slot_id: str, errors: list[str]) -> None:
    if "\r" in status_text:
        errors.append("telemetry/status.ndjson must use LF line endings")
    if status_text and not status_text.endswith("\n"):
        errors.append("telemetry/status.ndjson must end with a trailing newline")
    saw_record = False
    saw_ok = False
    for line_no, raw_line in enumerate(status_text.splitlines(), start=1):
        line = raw_line.strip()
        if not line:
            continue
        saw_record = True
        if raw_line != line:
            errors.append(
                f"telemetry/status.ndjson line {line_no} must not contain surrounding whitespace"
            )
            continue
        try:
            status_event = device_lab._loads_json_without_duplicate_keys(line)
        except json.JSONDecodeError:
            errors.append(f"telemetry/status.ndjson line {line_no} must be JSON")
            continue
        except device_lab.DuplicateJsonKeyError as exc:
            errors.append(
                "telemetry/status.ndjson line "
                f"{line_no} contains duplicate JSON object key {device_lab._display_path(exc.key)}"
            )
            continue
        except device_lab.NonFiniteJsonConstantError:
            errors.append(
                f"telemetry/status.ndjson line {line_no} contains non-finite constant "
                f"{device_lab.JSON_NONFINITE_CONSTANT_REDACTION}"
            )
            continue
        if not isinstance(status_event, dict):
            errors.append(f"telemetry/status.ndjson line {line_no} must be a JSON object")
            continue
        for field in sorted(set(status_event) - device_lab.STATUS_EVENT_FIELDS):
            errors.append(
                f"telemetry/status.ndjson line {line_no} contains unexpected field "
                f"{device_lab._display_path(field)}"
            )
        status = status_event.get("status")
        if not isinstance(status, str) or not status:
            errors.append(f"telemetry/status.ndjson line {line_no} status must be a non-empty string")
            continue
        if status != status.strip():
            errors.append(f"telemetry/status.ndjson line {line_no} status must not contain surrounding whitespace")
            continue
        if device_lab._contains_control_character(status):
            errors.append(f"telemetry/status.ndjson line {line_no} status must not contain control characters")
            continue
        if status != status.lower():
            errors.append(f"telemetry/status.ndjson line {line_no} status must be lowercase")
            continue
        slot_value = status_event.get("slot_id")
        if slot_value is None:
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must be a non-empty string")
        elif not isinstance(slot_value, str):
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must be a string")
        elif not slot_value:
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must be a non-empty string")
        elif isinstance(slot_value, str) and slot_value != slot_value.strip():
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must not contain surrounding whitespace")
        elif isinstance(slot_value, str) and device_lab._contains_control_character(slot_value):
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must not contain control characters")
        elif slot_value != slot_id:
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must match slot id")
        if status == "ok":
            saw_ok = True
        elif status in device_lab.KAGEMUSHA_STATUS_FAILURE_VALUES:
            errors.append(f"telemetry/status.ndjson line {line_no} status must not be {status!r}")
        else:
            errors.append(f"telemetry/status.ndjson line {line_no} status must be ok")
    if not saw_record:
        errors.append("telemetry/status.ndjson must contain at least one JSON status record")
    elif not saw_ok:
        errors.append("telemetry/status.ndjson must contain at least one ok status")


def _validate_challenge_hex_file(
    challenge_text: str,
    errors: list[str],
) -> tuple[str | None, bytes | None]:
    if not challenge_text.endswith("\n") or challenge_text.count("\n") != 1:
        errors.append(
            "attestation/challenge.hex must be canonical lowercase hexadecimal plus trailing newline"
        )
        return None, None
    challenge_hex = challenge_text[:-1]
    if (
        not challenge_hex
        or len(challenge_hex) % 2 != 0
        or any(char not in "0123456789abcdef" for char in challenge_hex)
    ):
        errors.append(
            "attestation/challenge.hex must be canonical lowercase hexadecimal plus trailing newline"
        )
        return None, None
    return challenge_hex, bytes.fromhex(challenge_hex)


def _validate_harness_result(
    *,
    harness: dict[str, Any],
    challenge_hex_file: str | None,
    chain_text: str | None,
    errors: list[str],
) -> None:
    for field in sorted(set(harness) - HARNESS_RESULT_ALLOWED_FIELDS):
        errors.append(
            "attestation/harness-result.json contains unexpected field "
            f"{device_lab._display_path(field)}"
        )
    alias = harness.get("alias")
    if not isinstance(alias, str) or not alias:
        errors.append("attestation/harness-result.json alias must be a non-empty string")
    elif alias != alias.strip():
        errors.append("attestation/harness-result.json alias must not have surrounding whitespace")
    elif device_lab._contains_control_character(alias):
        errors.append("attestation/harness-result.json alias must not contain control characters")
    elif device_lab.SECRET_RE.search(alias):
        errors.append("attestation/harness-result.json alias must not contain secret-looking material")
    for key in ("attestation_security_level", "keymaster_security_level"):
        level = harness.get(key)
        if not isinstance(level, str) or not level:
            errors.append(f"attestation/harness-result.json {key} must be a non-empty string")
        elif level != level.strip():
            errors.append(f"attestation/harness-result.json {key} must not have surrounding whitespace")
        elif device_lab._contains_control_character(level):
            errors.append(f"attestation/harness-result.json {key} must not contain control characters")
        elif device_lab.SECRET_RE.search(level):
            errors.append(
                f"attestation/harness-result.json {key} must not contain secret-looking material"
            )
        elif level not in device_lab.STRONGBOX_LEVELS:
            errors.append(f"attestation/harness-result.json {key} must be STRONGBOX")
    if harness.get("strongbox_attestation") is not True:
        errors.append("attestation/harness-result.json strongbox_attestation must be true")
    chain_length = harness.get("chain_length")
    if not isinstance(chain_length, int) or chain_length < 2:
        errors.append("attestation/harness-result.json chain_length must be at least 2")
    elif chain_text is not None:
        certificate_count = _pem_certificate_count(chain_text)
        if certificate_count < 2:
            errors.append("attestation/keymint-certificate-chain.pem must contain at least two PEM certificates")
        elif chain_length != certificate_count:
            errors.append(
                "attestation/harness-result.json chain_length must match "
                "attestation/keymint-certificate-chain.pem certificate count"
            )
    challenge_hex = harness.get("challenge_hex")
    if not isinstance(challenge_hex, str) or not challenge_hex.strip():
        errors.append("attestation/harness-result.json challenge_hex must be a non-empty string")
        return
    if device_lab._contains_control_character(challenge_hex):
        errors.append("attestation/harness-result.json challenge_hex must not contain control characters")
        return
    normalized = "".join(challenge_hex.split()).lower()
    if challenge_hex != normalized:
        errors.append("attestation/harness-result.json challenge_hex must be lowercase hexadecimal without whitespace")
        return
    if len(normalized) % 2 != 0:
        errors.append("attestation/harness-result.json challenge_hex must have even length")
        return
    try:
        bytes.fromhex(normalized)
    except ValueError:
        errors.append("attestation/harness-result.json challenge_hex must be hex")
        return
    if challenge_hex_file is not None and normalized != challenge_hex_file:
        errors.append("attestation/harness-result.json challenge_hex must match attestation/challenge.hex")


def _adb_command(adb: str, serial: str | None, args: list[str]) -> list[str]:
    command = [adb]
    if serial:
        command.extend(["-s", serial])
    command.extend(args)
    return command


def _run_latest_slot_query(
    *,
    adb: str,
    serial: str | None,
    run_as_package: str,
    device_lab_root: str,
    timeout_seconds: int,
    runner: Runner,
) -> tuple[str | None, list[str]]:
    command = _adb_command(
        adb,
        serial,
        [
            "shell",
            "run-as",
            run_as_package,
            "cat",
            f"{device_lab_root}/latest-slot.txt",
        ],
    )
    errors = _command_disruption_errors(command, "latest raw slot ADB query")
    if errors:
        return None, errors
    try:
        result = runner(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=_timeout_arg(timeout_seconds),
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        detail = _safe_detail(str(exc), redact_tokens=(serial or "",))
        suffix = f": {detail}" if detail else ""
        return None, [f"failed to read latest raw slot from attached device{suffix}"]
    if result.returncode != 0:
        detail = _safe_detail(result.stderr, redact_tokens=(serial or "",))
        suffix = f": {detail}" if detail else ""
        return None, [f"failed to read latest raw slot from attached device{suffix}"]
    latest_text = str(result.stdout)
    if (
        not latest_text.endswith("\n")
        or latest_text.count("\n") != 1
        or latest_text[:-1].strip() != latest_text[:-1]
        or not latest_text[:-1]
    ):
        return None, ["latest-slot.txt must be canonical and contain exactly one slot id"]
    return latest_text[:-1], []


def _run_raw_slot_tar_pull(
    *,
    adb: str,
    serial: str | None,
    run_as_package: str,
    device_lab_root: str,
    slot_id: str,
    timeout_seconds: int,
    runner: Runner,
) -> tuple[bytes | None, list[str]]:
    command = _adb_command(
        adb,
        serial,
        [
            "exec-out",
            "run-as",
            run_as_package,
            "tar",
            "-C",
            device_lab_root,
            "-cf",
            "-",
            slot_id,
            "latest-slot.txt",
        ],
    )
    errors = _command_disruption_errors(command, "raw slot tar ADB pull")
    if errors:
        return None, errors
    try:
        result = runner(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=_timeout_arg(timeout_seconds),
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        detail = _safe_detail(str(exc), redact_tokens=(serial or "",))
        suffix = f": {detail}" if detail else ""
        return None, [f"failed to pull raw slot tar from attached device{suffix}"]
    if result.returncode != 0:
        detail = _safe_detail(result.stderr, redact_tokens=(serial or "",))
        suffix = f": {detail}" if detail else ""
        return None, [f"failed to pull raw slot tar from attached device{suffix}"]
    data = bytes(result.stdout)
    if not data:
        return None, ["raw slot tar stream must be non-empty"]
    if len(data) > MAX_RAW_SLOT_TAR_BYTES:
        return None, [f"raw slot tar stream must not exceed {MAX_RAW_SLOT_TAR_BYTES} bytes"]
    return data, []


def _normalise_tar_member_name(
    name: str,
    errors: list[str],
    *,
    allow_trailing_slash: bool = False,
) -> str | None:
    if device_lab.SECRET_RE.search(name):
        errors.append("raw slot tar member path must not contain secret-looking material")
        return None
    if device_lab._contains_control_character(name):
        errors.append("raw slot tar member path must not contain control characters")
        return None
    candidate = PurePosixPath(name)
    normalised = candidate.as_posix()
    has_single_directory_slash = name.endswith("/") and not name.endswith("//")
    is_trailing_slash_form = (
        allow_trailing_slash and has_single_directory_slash and normalised == name[:-1]
    )
    if (
        not name.strip()
        or name.startswith("/")
        or "\\" in name
        or candidate.is_absolute()
        or normalised in {"", "."}
        or ".." in candidate.parts
    ):
        errors.append(f"raw slot tar member has unsafe path {device_lab._display_path(name)!r}")
        return None
    if normalised != name and not is_trailing_slash_form:
        errors.append(
            f"raw slot tar member has noncanonical path {device_lab._display_path(name)!r}"
        )
        return None
    return normalised


def _member_allowed_for_slot(relative: str, slot_id: str) -> bool:
    return relative == "latest-slot.txt" or relative == slot_id or relative.startswith(
        f"{slot_id}/"
    )


def _ensure_directory_at(
    root_fd: int,
    relative: str,
    error: str,
) -> tuple[int | None, list[str]]:
    try:
        current_fd = os.dup(root_fd)
    except OSError:
        return None, [error]
    try:
        parts = PurePosixPath(relative).parts
        for part in parts:
            if part in ("", "."):
                continue
            try:
                os.mkdir(part, 0o700, dir_fd=current_fd)
            except FileExistsError:
                pass
            except OSError:
                os.close(current_fd)
                return None, [error]
            try:
                next_fd = os.open(part, _directory_open_flags(), dir_fd=current_fd)
            except OSError:
                os.close(current_fd)
                return None, [error]
            try:
                next_stat = os.fstat(next_fd)
                if not stat.S_ISDIR(next_stat.st_mode):
                    os.close(next_fd)
                    os.close(current_fd)
                    return None, [error]
                os.fchmod(next_fd, 0o700)
            except OSError:
                os.close(next_fd)
                os.close(current_fd)
                return None, [error]
            os.close(current_fd)
            current_fd = next_fd
        return current_fd, []
    except Exception:
        os.close(current_fd)
        raise


def _write_regular_member(
    *,
    tar: tarfile.TarFile,
    member: tarfile.TarInfo,
    destination_root_fd: int,
    relative: str,
    errors: list[str],
) -> int:
    if member.size < 0:
        errors.append(f"raw slot tar member {relative} has invalid size")
        return 0
    if member.size > MAX_RAW_SLOT_FILE_BYTES:
        errors.append(
            f"raw slot tar member {relative} must not exceed {MAX_RAW_SLOT_FILE_BYTES} bytes"
        )
        return 0
    source = tar.extractfile(member)
    if source is None:
        errors.append(f"raw slot tar member {relative} could not be read")
        return 0
    try:
        data = source.read()
    except OSError:
        errors.append(f"raw slot tar member {relative} could not be read")
        return 0
    if len(data) != member.size:
        errors.append(f"raw slot tar member {relative} changed while being read")
        return 0
    relative_path = PurePosixPath(relative)
    parent_fd, parent_errors = _ensure_directory_at(
        destination_root_fd,
        relative_path.parent.as_posix(),
        f"raw slot tar member {relative} parent directory could not be created",
    )
    if parent_errors:
        errors.extend(parent_errors)
        return 0
    assert parent_fd is not None
    output_fd: int | None = None
    try:
        try:
            output_fd = os.open(
                relative_path.name,
                _private_create_open_flags(),
                0o600,
                dir_fd=parent_fd,
            )
        except FileExistsError:
            errors.append(f"raw slot tar member {relative} is duplicated")
            return 0
        except OSError:
            errors.append(f"raw slot tar member {relative} could not be written")
            return 0
        os.fchmod(output_fd, 0o600)
        _write_all(output_fd, data)
        os.fsync(output_fd)
    except OSError:
        errors.append(f"raw slot tar member {relative} could not be written")
        return 0
    finally:
        if output_fd is not None:
            os.close(output_fd)
        os.close(parent_fd)
    return len(data)


def _open_extraction_root(destination_root: Path) -> tuple[int | None, list[str]]:
    try:
        root_fd = os.open(destination_root, _directory_open_flags())
    except OSError:
        return None, ["raw slot extraction root metadata could not be read"]
    try:
        root_stat = os.fstat(root_fd)
    except OSError:
        os.close(root_fd)
        return None, ["raw slot extraction root metadata could not be read"]
    if not stat.S_ISDIR(root_stat.st_mode):
        os.close(root_fd)
        return None, ["raw slot extraction root must be a directory"]
    return root_fd, []


def _append_directory_errors(
    errors: list[str],
    root_fd: int,
    relative: str,
) -> None:
    directory_fd, directory_errors = _ensure_directory_at(
        root_fd,
        relative,
        f"raw slot tar directory {relative} could not be created",
    )
    if directory_errors:
        errors.extend(directory_errors)
        return
    assert directory_fd is not None
    os.close(directory_fd)


def _private_create_open_flags() -> int:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def extract_raw_slot_tar(
    tar_bytes: bytes,
    destination_root: Path,
    slot_id: str,
) -> list[str]:
    """Strictly extract a raw device-lab tar stream under ``destination_root``."""

    errors: list[str] = []
    seen: set[str] = set()
    file_count = 0
    total_bytes = 0
    try:
        tar = tarfile.open(fileobj=io.BytesIO(tar_bytes), mode="r:")
    except tarfile.TarError:
        return ["raw slot tar stream could not be parsed"]
    root_fd, root_errors = _open_extraction_root(destination_root)
    if root_errors:
        return root_errors
    assert root_fd is not None
    with tar:
        try:
            entry_count = 0
            for member in tar:
                entry_count += 1
                if entry_count > MAX_RAW_SLOT_ENTRIES:
                    errors.append(
                        f"raw slot tar must not contain more than {MAX_RAW_SLOT_ENTRIES} entries"
                    )
                    break
                relative = _normalise_tar_member_name(
                    member.name,
                    errors,
                    allow_trailing_slash=member.isdir(),
                )
                if relative is None:
                    continue
                if not _member_allowed_for_slot(relative, slot_id):
                    errors.append(f"raw slot tar member {relative} is outside requested slot")
                    continue
                if relative in seen:
                    errors.append(f"raw slot tar member {relative} is duplicated")
                    continue
                seen.add(relative)
                if member.isdir():
                    _append_directory_errors(errors, root_fd, relative)
                    continue
                if member.issym() or member.islnk():
                    errors.append(
                        f"raw slot tar member {relative} must not be a symlink or hardlink"
                    )
                    continue
                if not member.isfile():
                    errors.append(f"raw slot tar member {relative} must be a regular file")
                    continue
                file_count += 1
                if file_count > MAX_RAW_SLOT_FILES:
                    errors.append(
                        f"raw slot tar must not contain more than {MAX_RAW_SLOT_FILES} files"
                    )
                    continue
                total_bytes += _write_regular_member(
                    tar=tar,
                    member=member,
                    destination_root_fd=root_fd,
                    relative=relative,
                    errors=errors,
                )
                if total_bytes > MAX_RAW_SLOT_TAR_BYTES:
                    errors.append(
                        f"raw slot extracted bytes must not exceed {MAX_RAW_SLOT_TAR_BYTES}"
                    )
        finally:
            os.close(root_fd)
    return errors


def _read_text_file(path: Path, label: str, errors: list[str], max_bytes: int = 64 * 1024) -> str | None:
    try:
        expected_stat = path.lstat()
    except FileNotFoundError:
        errors.append(f"{label} is missing")
        return None
    except OSError:
        errors.append(f"{label} metadata could not be read")
        return None
    if stat.S_ISLNK(expected_stat.st_mode):
        errors.append(f"{label} must not be a symlink")
        return None
    if not stat.S_ISREG(expected_stat.st_mode):
        errors.append(f"{label} must be a regular file")
        return None
    if expected_stat.st_nlink > 1:
        errors.append(f"{label} must not be hardlinked")
        return None
    if expected_stat.st_size > max_bytes:
        errors.append(f"{label} must not exceed {max_bytes} bytes")
        return None
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
                errors.append(f"{label} changed while being read")
                return None
            if not stat.S_ISREG(open_stat.st_mode):
                errors.append(f"{label} must be a regular file")
                return None
            if open_stat.st_nlink > 1:
                errors.append(f"{label} must not be hardlinked")
                return None
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > max_bytes:
                    errors.append(f"{label} must not exceed {max_bytes} bytes")
                    return None
                chunks.append(chunk)
            final_stat = path.lstat()
            if _file_identity(final_stat) != expected_identity:
                errors.append(f"{label} changed while being read")
                return None
    except OSError:
        errors.append(f"{label} could not be read")
        return None
    try:
        return b"".join(chunks).decode("utf-8")
    except UnicodeDecodeError:
        errors.append(f"{label} could not be read")
        return None


def _validate_raw_slot_files(slot_path: Path, slot_id: str, root_latest: Path) -> list[str]:
    errors: list[str] = []
    path_errors = _path_shape_errors(slot_path, "raw slot path")
    if path_errors:
        return path_errors
    try:
        slot_mode = slot_path.lstat().st_mode
    except FileNotFoundError:
        return ["raw slot directory is missing"]
    except OSError:
        return ["raw slot directory metadata could not be read"]
    if stat.S_ISLNK(slot_mode):
        return ["raw slot directory must not be a symlink"]
    if not stat.S_ISDIR(slot_mode):
        return ["raw slot path must be a directory"]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        slot_path,
        "raw slot ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors

    for path in slot_path.rglob("*"):
        relative = path.relative_to(slot_path).as_posix()
        if device_lab.SECRET_RE.search(relative):
            errors.append("raw slot artifact paths must not contain secret-looking material")
            continue
        if device_lab._contains_control_character(relative):
            errors.append("raw slot artifact paths must not contain control characters")
            continue
        try:
            mode = path.lstat().st_mode
        except OSError:
            errors.append(f"raw slot artifact {relative} metadata could not be read")
            continue
        if stat.S_ISLNK(mode):
            errors.append(f"raw slot artifact {relative} must not be a symlink")
            continue
        if stat.S_ISDIR(mode):
            if relative not in RAW_SLOT_ALLOWED_DIRECTORIES:
                errors.append(f"raw slot artifact {relative} is not an allowed path")
            continue
        if not stat.S_ISREG(mode):
            errors.append(f"raw slot artifact {relative} must be a regular file")
            continue
        if relative not in RAW_SLOT_ALLOWED_PATHS:
            errors.append(f"raw slot artifact {relative} is not an allowed path")
            continue
        try:
            link_count = path.stat().st_nlink
        except OSError:
            errors.append(f"raw slot artifact {relative} hardlink metadata could not be read")
            continue
        if link_count > 1:
            errors.append(f"raw slot artifact {relative} must not be hardlinked")
        try:
            size = path.stat().st_size
        except OSError:
            errors.append(f"raw slot artifact {relative} size could not be read")
            continue
        if size == 0:
            errors.append(f"raw slot artifact {relative} must be non-empty")
        if size > MAX_RAW_SLOT_FILE_BYTES:
            errors.append(
                f"raw slot artifact {relative} must not exceed {MAX_RAW_SLOT_FILE_BYTES} bytes"
            )

    for relative in RAW_SLOT_REQUIRED_PATHS:
        if not (slot_path / relative).exists():
            errors.append(f"raw slot artifact {relative} is missing")

    latest_text = _read_text_file(root_latest, "latest-slot.txt", errors)
    if latest_text is not None and latest_text != f"{slot_id}\n":
        errors.append("latest-slot.txt must be canonical and match slot id")

    challenge_text = _read_text_file(
        slot_path / "attestation" / "challenge.hex",
        "attestation/challenge.hex",
        errors,
    )
    challenge_hex_file: str | None = None
    challenge_bytes: bytes | None = None
    if challenge_text is not None:
        challenge_hex_file, challenge_bytes = _validate_challenge_hex_file(
            challenge_text,
            errors,
        )
    chain_text = _read_text_file(
        slot_path / "attestation" / "keymint-certificate-chain.pem",
        "attestation/keymint-certificate-chain.pem",
        errors,
    )
    harness = device_lab._load_json(
        slot_path / "attestation" / "harness-result.json",
        "attestation harness result",
        errors,
    )
    if harness is not None:
        _validate_harness_result(
            harness=dict(harness),
            challenge_hex_file=challenge_hex_file,
            chain_text=chain_text,
            errors=errors,
        )

    result = device_lab._load_json(slot_path / "attestation" / "result.json", "attestation result", errors)
    if result is not None:
        for field in sorted(set(result) - RAW_RESULT_ALLOWED_FIELDS):
            errors.append(
                "attestation/result.json contains unexpected field "
                f"{device_lab._display_path(field)}"
            )
        if result.get("slot_id") != slot_id:
            errors.append("attestation/result.json slot_id must match slot id")
        if result.get("slot") != slot_id:
            errors.append("attestation/result.json slot must match slot id")
        if result.get("status") != "ok":
            errors.append("attestation/result.json status must be ok")
        if result.get("strongbox_attestation") is not True:
            errors.append("attestation/result.json strongbox_attestation must be true")
        if result.get("physical_device_attestation") is not True:
            errors.append("attestation/result.json physical_device_attestation must be true")
        for field in RAW_RESULT_STRING_FIELDS:
            _validate_raw_result_string(result, field, errors)
        for field in RAW_RESULT_STRONGBOX_FIELDS:
            if result.get(field) != "STRONGBOX":
                errors.append(f"attestation/result.json {field} must be STRONGBOX")
        chain_path = result.get("attestation_certificate_chain_path")
        if chain_path != "attestation/keymint-certificate-chain.pem":
            errors.append(
                "attestation/result.json attestation_certificate_chain_path must be "
                "attestation/keymint-certificate-chain.pem"
            )
        raw_digests: dict[str, str | None] = {}
        for field in RAW_RESULT_SHA256_FIELDS:
            raw_digests[field] = _validate_sha256_hex(
                result.get(field),
                f"attestation/result.json {field}",
                errors,
            )
        chain_digest = raw_digests[RAW_RESULT_CHAIN_DIGEST_FIELD]
        if chain_digest is not None and chain_text is not None:
            digest = hashlib.sha256(chain_text.encode("utf-8")).hexdigest()
            if chain_digest != digest:
                errors.append("attestation/result.json certificate-chain SHA-256 mismatch")
        challenge_digest = raw_digests[RAW_RESULT_CHALLENGE_DIGEST_FIELD]
        if challenge_bytes is not None and challenge_digest is not None:
            if challenge_bytes:
                if hashlib.sha256(challenge_bytes).hexdigest() != challenge_digest:
                    errors.append("attestation/result.json attestation challenge SHA-256 mismatch")

    expected_app_package_name = None
    if isinstance(result, dict) and isinstance(result.get("app_package_name"), str):
        expected_app_package_name = result["app_package_name"]
    _validate_raw_json_artifacts(
        slot_path,
        slot_id,
        errors,
        expected_app_package_name=expected_app_package_name,
    )

    status_text = _read_text_file(
        slot_path / "telemetry" / "status.ndjson",
        "telemetry/status.ndjson",
        errors,
    )
    if status_text is not None:
        _validate_raw_status_ndjson(status_text, slot_id, errors)

    runtime_text = _read_text_file(
        slot_path / "logs" / "runtime.log",
        "logs/runtime.log",
        errors,
    )
    if runtime_text is not None:
        if device_lab.KAGEMUSHA_RUNTIME_LOG_COMPLETE_MARKER not in runtime_text:
            errors.append("logs/runtime.log must contain Kagemusha device-lab completion marker")
        for marker in device_lab.KAGEMUSHA_RUNTIME_LOG_FAILURE_MARKERS:
            if marker in runtime_text:
                errors.append(f"logs/runtime.log must not contain failure marker {marker}")

    return errors


def _validate_output_root(root: Path) -> list[str]:
    path_errors = _path_shape_errors(root, "raw output root path")
    if path_errors:
        return path_errors
    root_exists, errors = device_lab.classify_device_lab_root_path(root)
    if errors:
        return errors
    if not root_exists:
        try:
            root.mkdir(parents=True, exist_ok=True)
        except OSError:
            return ["raw output root directory could not be created"]
        root_exists, errors = device_lab.classify_device_lab_root_path(root)
        if errors:
            return errors
        if not root_exists:
            return ["raw output root must be an existing directory"]
    permission_errors = _set_private_directory_permissions(
        root,
        "raw output root directory",
    )
    if permission_errors:
        return permission_errors
    return []


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


def _cleanup_temp_output_at(
    parent_fd: int,
    name: str,
    label: str,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    if expected_identity is None:
        return [f"{label} temporary output metadata could not be read"]
    try:
        temp_stat = os.stat(
            name,
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
        os.unlink(name, dir_fd=parent_fd)
    except FileNotFoundError:
        return []
    except OSError:
        return [f"{label} temporary output could not be removed"]
    try:
        os.fsync(parent_fd)
    except OSError:
        return [f"{label} temporary output cleanup could not be synced"]
    return []


def _unlink_file_if_identity_at(
    parent_fd: int,
    name: str,
    expected_identity: tuple[int, int],
    label: str,
) -> list[str]:
    try:
        file_stat = os.stat(
            name,
            dir_fd=parent_fd,
            follow_symlinks=False,
        )
    except FileNotFoundError:
        return []
    except OSError:
        return [f"{label} cleanup metadata could not be read"]
    if (
        not stat.S_ISREG(file_stat.st_mode)
        or _file_identity(file_stat) != expected_identity
    ):
        return []
    try:
        os.unlink(name, dir_fd=parent_fd)
    except FileNotFoundError:
        return []
    except OSError:
        return [f"{label} could not be removed after parent sync failure"]
    try:
        os.fsync(parent_fd)
    except OSError:
        return [f"{label} cleanup could not be synced after parent sync failure"]
    return []


def _temp_output_open_flags() -> int:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _read_output_open_flags() -> int:
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _write_all(file_fd: int, data: bytes) -> None:
    view = memoryview(data)
    offset = 0
    while offset < len(view):
        written = os.write(file_fd, view[offset:])
        if written == 0:
            raise OSError("short write")
        offset += written


def _open_temp_output_at(
    parent_fd: int,
    prefix: str,
    suffix: str,
    label: str,
) -> tuple[int | None, str | None, tuple[int, int] | None, list[str]]:
    for _attempt in range(128):
        name = f"{prefix}{secrets.token_hex(16)}{suffix}"
        try:
            file_fd = os.open(name, _temp_output_open_flags(), 0o600, dir_fd=parent_fd)
        except FileExistsError:
            continue
        except OSError:
            return None, None, None, [f"{label} could not be written"]
        try:
            os.fchmod(file_fd, 0o600)
            identity = _file_identity(os.fstat(file_fd))
        except OSError:
            try:
                os.close(file_fd)
            finally:
                cleanup_errors = _cleanup_temp_output_at(parent_fd, name, label, None)
            return None, name, None, [f"{label} could not be written", *cleanup_errors]
        return file_fd, name, identity, []
    return None, None, None, [f"{label} temporary output could not be created"]


def _readback_published_output_at(
    parent_fd: int,
    name: str,
    label: str,
    *,
    max_bytes: int,
    size_error: str,
) -> tuple[tuple[int, int] | None, bytes | None, list[str]]:
    try:
        expected_stat = os.stat(
            name,
            dir_fd=parent_fd,
            follow_symlinks=False,
        )
    except OSError:
        return None, None, [f"{label} could not be read back after writing"]
    if stat.S_ISLNK(expected_stat.st_mode):
        return None, None, [f"{label} must not be a symlink after writing"]
    if not stat.S_ISREG(expected_stat.st_mode):
        return None, None, [f"{label} must be a regular file after writing"]
    if expected_stat.st_nlink > 1:
        return None, None, [f"{label} must not be hardlinked after writing"]
    if stat.S_IMODE(expected_stat.st_mode) != 0o600:
        return None, None, [f"{label} permissions must be 0600"]
    if expected_stat.st_size > max_bytes:
        return None, None, [size_error]
    expected_identity = _file_identity(expected_stat)
    read_fd: int | None = None
    try:
        read_fd = os.open(name, _read_output_open_flags(), dir_fd=parent_fd)
        open_stat = os.fstat(read_fd)
        if _file_identity(open_stat) != expected_identity:
            return None, None, [f"{label} changed while being read back"]
        if not stat.S_ISREG(open_stat.st_mode):
            return None, None, [f"{label} must be a regular file after writing"]
        if open_stat.st_nlink > 1:
            return None, None, [f"{label} must not be hardlinked after writing"]
        if stat.S_IMODE(open_stat.st_mode) != 0o600:
            return None, None, [f"{label} permissions must be 0600"]
        if open_stat.st_size > max_bytes:
            return None, None, [size_error]
        readback = os.read(read_fd, max_bytes + 1)
    except OSError:
        return None, None, [f"{label} could not be read back after writing"]
    finally:
        if read_fd is not None:
            os.close(read_fd)
    if len(readback) > max_bytes:
        return None, None, [size_error]
    try:
        final_stat = os.stat(
            name,
            dir_fd=parent_fd,
            follow_symlinks=False,
        )
    except OSError:
        return None, None, [f"{label} could not be read back after writing"]
    if _file_identity(final_stat) != expected_identity:
        return None, None, [f"{label} changed while being read back"]
    return expected_identity, readback, []


def _write_latest_slot(root: Path, slot_id: str) -> list[str]:
    latest_path = root / "latest-slot.txt"
    errors = device_lab.validate_summary_output_path(latest_path, "raw latest-slot output")
    if errors:
        return errors
    try:
        root_stat = root.lstat()
    except OSError:
        return ["raw latest-slot output parent directory metadata could not be read"]
    if stat.S_ISLNK(root_stat.st_mode) or not stat.S_ISDIR(root_stat.st_mode):
        return ["raw latest-slot output parent must be a directory"]
    root_identity = _file_identity(root_stat)
    try:
        root_fd = os.open(root, _directory_open_flags())
    except OSError:
        return ["raw latest-slot output parent directory metadata could not be read"]
    try:
        try:
            open_root_stat = os.fstat(root_fd)
        except OSError:
            return ["raw latest-slot output parent directory metadata could not be read"]
        if not stat.S_ISDIR(open_root_stat.st_mode):
            return ["raw latest-slot output parent must be a directory"]
        if _file_identity(open_root_stat) != root_identity:
            return ["raw latest-slot output parent directory changed before writing"]
        fd, temp_name, temp_identity, temp_errors = _open_temp_output_at(
            root_fd,
            ".latest-slot.",
            ".tmp",
            "raw latest-slot output",
        )
        if temp_errors:
            return temp_errors
        assert fd is not None
        assert temp_name is not None
        assert temp_identity is not None
        encoded = (slot_id + "\n").encode("utf-8")
        try:
            try:
                _write_all(fd, encoded)
                os.fsync(fd)
            finally:
                os.close(fd)
            os.replace(
                temp_name,
                latest_path.name,
                src_dir_fd=root_fd,
                dst_dir_fd=root_fd,
            )
            temp_name = None
            expected_identity, readback, read_errors = _readback_published_output_at(
                root_fd,
                latest_path.name,
                "raw latest-slot output",
                max_bytes=len(encoded),
                size_error="raw latest-slot output readback mismatch",
            )
            if read_errors:
                return read_errors
            assert expected_identity is not None
            assert readback is not None
            if readback != encoded:
                return ["raw latest-slot output readback mismatch"]
            try:
                current_root_stat = root.lstat()
            except OSError:
                cleanup_errors = _unlink_file_if_identity_at(
                    root_fd,
                    latest_path.name,
                    expected_identity,
                    "raw latest-slot output",
                )
                return [
                    "raw latest-slot output parent directory could not be synced",
                    *cleanup_errors,
                ]
            if _file_identity(current_root_stat) != root_identity:
                cleanup_errors = _unlink_file_if_identity_at(
                    root_fd,
                    latest_path.name,
                    expected_identity,
                    "raw latest-slot output",
                )
                return [
                    "raw latest-slot output parent directory could not be synced",
                    *cleanup_errors,
                ]
            sync_errors = _sync_directory(
                root,
                "raw latest-slot output parent directory could not be synced",
                expected_identity=root_identity,
            )
            if sync_errors:
                cleanup_errors = _unlink_file_if_identity_at(
                    root_fd,
                    latest_path.name,
                    expected_identity,
                    "raw latest-slot output",
                )
                return [*sync_errors, *cleanup_errors]
        except OSError:
            cleanup_errors = []
            if temp_name is not None:
                cleanup_errors = _cleanup_temp_output_at(
                    root_fd,
                    temp_name,
                    "raw latest-slot output",
                    temp_identity,
                )
            return ["raw latest-slot output could not be written", *cleanup_errors]
    finally:
        os.close(root_fd)
    return []


def _write_summary(path: Path, payload: dict[str, Any]) -> list[str]:
    errors = device_lab.validate_summary_output_path(path, "raw pull summary output")
    if errors:
        return errors
    try:
        encoded = _json_dumps(payload).encode("utf-8")
    except ValueError:
        return ["raw pull summary output is not strict JSON"]
    if len(encoded) > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
        return [
            "raw pull summary output must be no more than "
            f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
        ]
    try:
        parent_stat = path.parent.lstat()
    except OSError:
        return ["raw pull summary output parent directory metadata could not be read"]
    if stat.S_ISLNK(parent_stat.st_mode) or not stat.S_ISDIR(parent_stat.st_mode):
        return ["raw pull summary output parent directory could not be synced"]
    parent_identity = _file_identity(parent_stat)
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return ["raw pull summary output parent directory metadata could not be read"]
    try:
        try:
            open_parent_stat = os.fstat(parent_fd)
        except OSError:
            return ["raw pull summary output parent directory metadata could not be read"]
        if not stat.S_ISDIR(open_parent_stat.st_mode):
            return ["raw pull summary output parent directory could not be synced"]
        if _file_identity(open_parent_stat) != parent_identity:
            return ["raw pull summary output parent directory changed before writing"]
        fd, temp_name, temp_identity, temp_errors = _open_temp_output_at(
            parent_fd,
            f".{path.name}.",
            ".tmp",
            "raw pull summary output",
        )
        if temp_errors:
            return temp_errors
        assert fd is not None
        assert temp_name is not None
        assert temp_identity is not None
        try:
            try:
                _write_all(fd, encoded)
                os.fsync(fd)
            finally:
                os.close(fd)
            os.replace(
                temp_name,
                path.name,
                src_dir_fd=parent_fd,
                dst_dir_fd=parent_fd,
            )
            temp_name = None
            expected_identity, readback, read_errors = _readback_published_output_at(
                parent_fd,
                path.name,
                "raw pull summary output",
                max_bytes=device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES,
                size_error=(
                    "raw pull summary output must be no more than "
                    f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
                ),
            )
            if read_errors:
                return read_errors
            assert expected_identity is not None
            assert readback is not None
            if readback != encoded:
                return ["raw pull summary output readback mismatch"]
            try:
                current_parent_stat = path.parent.lstat()
            except OSError:
                cleanup_errors = _unlink_file_if_identity_at(
                    parent_fd,
                    path.name,
                    expected_identity,
                    "raw pull summary output",
                )
                return [
                    "raw pull summary output parent directory could not be synced",
                    *cleanup_errors,
                ]
            if _file_identity(current_parent_stat) != parent_identity:
                cleanup_errors = _unlink_file_if_identity_at(
                    parent_fd,
                    path.name,
                    expected_identity,
                    "raw pull summary output",
                )
                return [
                    "raw pull summary output parent directory could not be synced",
                    *cleanup_errors,
                ]
            sync_errors = _sync_directory(
                path.parent,
                "raw pull summary output parent directory could not be synced",
                expected_identity=parent_identity,
            )
            if sync_errors:
                cleanup_errors = _unlink_file_if_identity_at(
                    parent_fd,
                    path.name,
                    expected_identity,
                    "raw pull summary output",
                )
                return [*sync_errors, *cleanup_errors]
        except OSError:
            cleanup_errors = []
            if temp_name is not None:
                cleanup_errors = _cleanup_temp_output_at(
                    parent_fd,
                    temp_name,
                    "raw pull summary output",
                    temp_identity,
                )
            return ["raw pull summary output could not be written", *cleanup_errors]
    finally:
        os.close(parent_fd)
    return []


def _raw_artifact_digest(slot_path: Path, relative: str) -> tuple[str | None, list[str]]:
    path = slot_path / relative
    label = f"raw artifact digest {relative}"
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
    try:
        if path.stat().st_nlink > 1:
            return None, [f"{label} must not be hardlinked"]
    except OSError:
        return None, [f"{label} hardlink metadata could not be read"]

    digest = hashlib.sha256()
    expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            path_stat = path.lstat()
            path_identity = (path_stat.st_dev, path_stat.st_ino)
            if open_identity != expected_identity or path_identity != expected_identity:
                return None, [f"{label} changed while being read"]
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(open_stat.st_mode) or not stat.S_ISREG(path_stat.st_mode):
                return None, [f"{label} must be a regular file"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            if open_stat.st_size > MAX_RAW_SLOT_FILE_BYTES:
                return None, [f"{label} must not exceed {MAX_RAW_SLOT_FILE_BYTES} bytes"]
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > MAX_RAW_SLOT_FILE_BYTES:
                    return None, [f"{label} must not exceed {MAX_RAW_SLOT_FILE_BYTES} bytes"]
                digest.update(chunk)
            final_stat = path.lstat()
            if (final_stat.st_dev, final_stat.st_ino) != expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} could not be read"]
    return digest.hexdigest(), []


def _raw_artifact_digests(slot_path: Path) -> tuple[dict[str, str], list[str]]:
    digests: dict[str, str] = {}
    errors: list[str] = []
    for relative in RAW_SLOT_REQUIRED_PATHS:
        digest, digest_errors = _raw_artifact_digest(slot_path, relative)
        if digest_errors:
            errors.extend(digest_errors)
            continue
        assert digest is not None
        digests[relative] = digest
    if set(digests) != set(RAW_SLOT_REQUIRED_PATHS):
        errors.append("raw artifact digest inventory must include every required artifact")
    return digests, errors


def _file_identity(file_stat: os.stat_result) -> tuple[int, int]:
    return file_stat.st_dev, file_stat.st_ino


def _directory_open_flags() -> int:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _sync_directory(
    path: Path,
    error: str,
    *,
    expected_identity: tuple[int, int] | None = None,
) -> list[str]:
    try:
        dir_fd = os.open(path, _directory_open_flags())
    except OSError:
        return [error]
    try:
        if expected_identity is not None:
            try:
                open_stat = os.fstat(dir_fd)
            except OSError:
                return [error]
            if _file_identity(open_stat) != expected_identity:
                return [error]
        os.fsync(dir_fd)
    except OSError:
        return [error]
    finally:
        os.close(dir_fd)
    return []


def _slot_entry_identity(
    path: Path,
    parent_path: Path,
    parent_identity: tuple[int, int],
) -> tuple[tuple[int, int] | None, list[str]]:
    try:
        parent_fd = os.open(parent_path, _directory_open_flags())
    except OSError:
        return None, ["raw output root directory metadata could not be read"]
    try:
        try:
            parent_stat = os.fstat(parent_fd)
        except OSError:
            return None, ["raw output root directory metadata could not be read"]
        if _file_identity(parent_stat) != parent_identity:
            return None, ["raw output root directory changed during install"]
        try:
            path_stat = os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
        except OSError:
            return None, ["raw slot directory metadata could not be read"]
    finally:
        os.close(parent_fd)
    if stat.S_ISLNK(path_stat.st_mode) or not stat.S_ISDIR(path_stat.st_mode):
        return None, ["raw slot directory changed during install"]
    return _file_identity(path_stat), []


def _created_slot_identity_errors(
    path: Path,
    expected_identity: tuple[int, int],
    parent_path: Path,
    parent_identity: tuple[int, int],
) -> list[str]:
    actual_identity, errors = _slot_entry_identity(path, parent_path, parent_identity)
    if errors:
        return errors
    if actual_identity != expected_identity:
        return ["raw slot directory changed during install"]
    return []


def _remove_created_slot(
    path: Path,
    expected_identity: tuple[int, int],
    parent_path: Path,
    parent_identity: tuple[int, int],
) -> list[str]:
    try:
        parent_fd = os.open(parent_path, _directory_open_flags())
    except OSError:
        return ["raw slot partial install cleanup parent could not be opened"]
    try:
        try:
            parent_stat = os.fstat(parent_fd)
        except OSError:
            return ["raw slot partial install cleanup parent metadata could not be read"]
        if _file_identity(parent_stat) != parent_identity:
            return []
        try:
            path_stat = os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            return []
        except OSError:
            return ["raw slot partial install cleanup metadata could not be read"]
        return _remove_created_slot_at(
            parent_fd,
            path.name,
            expected_identity,
            path_stat,
        )
    finally:
        os.close(parent_fd)


def _remove_created_slot_at(
    parent_fd: int,
    name: str,
    expected_identity: tuple[int, int],
    path_stat: os.stat_result | None = None,
) -> list[str]:
    if path_stat is None:
        try:
            path_stat = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            return []
        except OSError:
            return ["raw slot partial install cleanup metadata could not be read"]
    if (
        not stat.S_ISLNK(path_stat.st_mode)
        and stat.S_ISDIR(path_stat.st_mode)
        and _file_identity(path_stat) == expected_identity
    ):
        try:
            shutil.rmtree(name, dir_fd=parent_fd)
        except OSError:
            return ["raw slot partial install could not be removed"]
        try:
            os.fsync(parent_fd)
        except OSError:
            return ["raw slot partial install cleanup could not be synced"]
    return []


def _cleanup_temp_parent(
    temp_parent: Path,
    *,
    expected_identity: tuple[int, int],
) -> list[str]:
    try:
        parent_fd = os.open(temp_parent.parent, _directory_open_flags())
    except OSError:
        return ["raw pull temporary directory cleanup parent could not be opened"]
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
            return ["raw pull temporary directory metadata could not be read"]
        if (
            not stat.S_ISDIR(temp_parent_stat.st_mode)
            or _file_identity(temp_parent_stat) != expected_identity
        ):
            return []
        try:
            shutil.rmtree(temp_parent.name, dir_fd=parent_fd)
        except OSError:
            return ["raw pull temporary directory could not be removed"]
        try:
            os.fsync(parent_fd)
        except OSError:
            return ["raw pull temporary directory cleanup could not be synced"]
    finally:
        os.close(parent_fd)
    return []


def _open_verified_directory(
    path: Path,
    error: str,
    *,
    expected_identity: tuple[int, int] | None = None,
) -> tuple[int | None, tuple[int, int] | None, list[str]]:
    try:
        dir_fd = os.open(path, _directory_open_flags())
    except OSError:
        return None, None, [error]
    try:
        try:
            dir_stat = os.fstat(dir_fd)
        except OSError:
            os.close(dir_fd)
            return None, None, [error]
        if not stat.S_ISDIR(dir_stat.st_mode):
            os.close(dir_fd)
            return None, None, [error]
        identity = _file_identity(dir_stat)
        if expected_identity is not None and identity != expected_identity:
            os.close(dir_fd)
            return None, None, [error]
        return dir_fd, identity, []
    except Exception:
        os.close(dir_fd)
        raise


def _remove_empty_stage_slot(
    stage_slot: Path,
    *,
    expected_identity: tuple[int, int],
) -> list[str]:
    try:
        parent_fd = os.open(stage_slot.parent, _directory_open_flags())
    except OSError:
        return ["raw slot directory could not be installed"]
    try:
        try:
            stage_stat = os.stat(
                stage_slot.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except OSError:
            return ["raw slot directory could not be installed"]
        if (
            not stat.S_ISDIR(stage_stat.st_mode)
            or _file_identity(stage_stat) != expected_identity
        ):
            return ["raw slot directory could not be installed"]
        try:
            os.rmdir(stage_slot.name, dir_fd=parent_fd)
        except OSError:
            return ["raw slot directory could not be installed"]
    finally:
        os.close(parent_fd)
    return []


def _install_validated_slot(
    stage_slot: Path,
    final_slot: Path,
    output_root: Path,
) -> list[str]:
    try:
        output_root_stat = output_root.lstat()
    except OSError:
        return ["raw output root directory metadata could not be read"]
    if stat.S_ISLNK(output_root_stat.st_mode) or not stat.S_ISDIR(output_root_stat.st_mode):
        return ["raw output root directory changed during install"]
    output_root_identity = _file_identity(output_root_stat)

    output_root_fd: int | None = None
    try:
        output_root_fd = os.open(output_root, _directory_open_flags())
    except OSError:
        return ["raw output root directory metadata could not be read"]
    try:
        try:
            open_root_stat = os.fstat(output_root_fd)
        except OSError:
            return ["raw output root directory metadata could not be read"]
        if (
            not stat.S_ISDIR(open_root_stat.st_mode)
            or _file_identity(open_root_stat) != output_root_identity
        ):
            return ["raw output root directory changed during install"]

        try:
            os.stat(final_slot.name, dir_fd=output_root_fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        except OSError:
            return ["raw slot directory metadata could not be read"]
        else:
            return ["slot directory already exists; refuse to overwrite raw evidence"]

        final_slot_identity: tuple[int, int] | None = None
        try:
            os.mkdir(final_slot.name, 0o700, dir_fd=output_root_fd)
        except FileExistsError:
            return ["slot directory already exists; refuse to overwrite raw evidence"]
        except OSError:
            return ["raw slot directory could not be installed"]

        try:
            final_slot_stat = os.stat(
                final_slot.name,
                dir_fd=output_root_fd,
                follow_symlinks=False,
            )
        except OSError:
            return ["raw slot directory metadata could not be read"]
        final_slot_identity = _file_identity(final_slot_stat)
        if stat.S_ISLNK(final_slot_stat.st_mode) or not stat.S_ISDIR(
            final_slot_stat.st_mode
        ):
            return ["raw slot directory changed during install"]

        installed = False

        def _install_contents() -> list[str]:
            nonlocal installed

            stage_fd, stage_slot_identity, stage_errors = _open_verified_directory(
                stage_slot,
                "raw slot directory could not be installed",
            )
            if stage_errors:
                return stage_errors
            assert stage_fd is not None
            assert stage_slot_identity is not None
            try:
                try:
                    child_names = os.listdir(stage_fd)
                except OSError:
                    return ["raw slot directory could not be installed"]

                seen_top_level: set[str] = set()
                for child_name in child_names:
                    if child_name in seen_top_level:
                        return ["raw slot directory could not be installed"]
                    seen_top_level.add(child_name)
                    if child_name not in RAW_SLOT_ALLOWED_DIRECTORIES:
                        display_child_name = device_lab._display_path(child_name)
                        return [
                            "raw slot install source contains unexpected top-level entry "
                            f"{display_child_name}"
                        ]
                    try:
                        child_mode = os.stat(
                            child_name,
                            dir_fd=stage_fd,
                            follow_symlinks=False,
                        ).st_mode
                    except OSError:
                        return ["raw slot directory could not be installed"]
                    if stat.S_ISLNK(child_mode) or not stat.S_ISDIR(child_mode):
                        display_child_name = device_lab._display_path(child_name)
                        return [
                            "raw slot install source contains unexpected top-level entry "
                            f"{display_child_name}"
                        ]

                if seen_top_level != set(RAW_SLOT_ALLOWED_DIRECTORIES):
                    return ["raw slot directory could not be installed"]

                final_fd, _final_identity, final_errors = _open_verified_directory(
                    final_slot,
                    "raw slot directory changed during install",
                    expected_identity=final_slot_identity,
                )
                if final_errors:
                    return final_errors
                assert final_fd is not None
                try:
                    for child_name in child_names:
                        identity_errors = _created_slot_identity_errors(
                            final_slot,
                            final_slot_identity,
                            output_root,
                            output_root_identity,
                        )
                        if identity_errors:
                            return identity_errors
                        try:
                            os.rename(
                                child_name,
                                child_name,
                                src_dir_fd=stage_fd,
                                dst_dir_fd=final_fd,
                            )
                        except OSError:
                            return ["raw slot directory could not be installed"]
                finally:
                    os.close(final_fd)
            finally:
                os.close(stage_fd)

            stage_remove_errors = _remove_empty_stage_slot(
                stage_slot,
                expected_identity=stage_slot_identity,
            )
            if stage_remove_errors:
                return stage_remove_errors

            identity_errors = _created_slot_identity_errors(
                final_slot,
                final_slot_identity,
                output_root,
                output_root_identity,
            )
            if identity_errors:
                return identity_errors
            sync_errors = _sync_directory(
                final_slot,
                "raw slot directory could not be synced",
                expected_identity=final_slot_identity,
            )
            if sync_errors:
                return sync_errors
            sync_errors = _sync_directory(
                output_root,
                "raw slot directory parent could not be synced",
                expected_identity=output_root_identity,
            )
            if sync_errors:
                return sync_errors
            installed = True
            return []

        install_errors: list[str] = []
        try:
            install_errors = _install_contents()
        finally:
            cleanup_errors: list[str] = []
            if not installed and final_slot_identity is not None:
                cleanup_errors = _remove_created_slot_at(
                    output_root_fd,
                    final_slot.name,
                    final_slot_identity,
                )
        return [*install_errors, *cleanup_errors]
    finally:
        if output_root_fd is not None:
            os.close(output_root_fd)


def pull_raw_slot(
    args: argparse.Namespace,
    *,
    runner: Runner = subprocess.run,
) -> tuple[int, Path | None, list[str]]:
    """Pull and validate one raw Kagemusha Android device-lab slot."""

    errors: list[str] = []
    for value, label in (
        (args.adb, "adb executable"),
        (args.run_as_package, "run-as package"),
        (args.device_lab_root, "device lab root"),
    ):
        errors.extend(_validate_non_secret_adb_string(value, label))
    if args.serial is not None:
        errors.extend(_validate_non_secret_adb_string(args.serial, "ADB serial"))
    errors.extend(_path_shape_errors(args.out_root, "raw output root path"))
    if args.summary_out is not None:
        errors.extend(_path_shape_errors(args.summary_out, "raw pull summary output"))
    if args.adb_timeout_seconds < 0:
        errors.append("--adb-timeout-seconds must be non-negative")
    if errors:
        return 1, None, errors

    if args.slot_id:
        raw_slot_id = args.slot_id
    else:
        raw_slot_id, latest_errors = _run_latest_slot_query(
            adb=args.adb,
            serial=args.serial,
            run_as_package=args.run_as_package,
            device_lab_root=args.device_lab_root,
            timeout_seconds=args.adb_timeout_seconds,
            runner=runner,
        )
        if latest_errors:
            return 1, None, latest_errors
        assert raw_slot_id is not None
    slot_id, slot_errors = _single_safe_slot_id(raw_slot_id)
    if slot_errors or slot_id is None:
        return 1, None, slot_errors

    output_root = args.out_root
    root_errors = _validate_output_root(output_root)
    if root_errors:
        return 1, None, root_errors
    final_slot = output_root / slot_id
    try:
        final_slot_mode = final_slot.lstat().st_mode
    except FileNotFoundError:
        final_slot_mode = None
    except OSError:
        return 1, None, ["raw slot directory metadata could not be read"]
    if final_slot_mode is not None:
        return 1, None, ["slot directory already exists; refuse to overwrite raw evidence"]

    tar_bytes, tar_errors = _run_raw_slot_tar_pull(
        adb=args.adb,
        serial=args.serial,
        run_as_package=args.run_as_package,
        device_lab_root=args.device_lab_root,
        slot_id=slot_id,
        timeout_seconds=args.adb_timeout_seconds,
        runner=runner,
    )
    if tar_errors or tar_bytes is None:
        return 1, None, tar_errors

    temp_parent = Path(tempfile.mkdtemp(prefix=f".{slot_id}.", dir=output_root))
    try:
        temp_parent_identity = _file_identity(temp_parent.lstat())
    except OSError:
        return 1, None, ["raw pull temporary directory metadata could not be read"]
    pull_status = 1
    pull_slot: Path | None = None
    pull_errors: list[str] = []
    try:
        extract_errors = extract_raw_slot_tar(tar_bytes, temp_parent, slot_id)
        if extract_errors:
            pull_errors = extract_errors
        else:
            stage_slot = temp_parent / slot_id
            validate_errors = _validate_raw_slot_files(
                stage_slot,
                slot_id,
                temp_parent / "latest-slot.txt",
            )
            if validate_errors:
                pull_errors = validate_errors
            else:
                install_errors = _install_validated_slot(stage_slot, final_slot, output_root)
                if install_errors:
                    pull_errors = install_errors
                else:
                    pull_status = 0
                    pull_slot = final_slot
    finally:
        cleanup_errors = _cleanup_temp_parent(
            temp_parent,
            expected_identity=temp_parent_identity,
        )
    if pull_errors or cleanup_errors:
        return 1, pull_slot, [*pull_errors, *cleanup_errors]
    if pull_status != 0:
        return pull_status, pull_slot, pull_errors

    latest_errors = _write_latest_slot(output_root, slot_id)
    if latest_errors:
        return 1, final_slot, latest_errors
    if args.summary_out is not None:
        artifact_digests, artifact_digest_errors = _raw_artifact_digests(final_slot)
        if artifact_digest_errors:
            return 1, final_slot, artifact_digest_errors
        summary = {
            "schema": RAW_PULL_SUMMARY_SCHEMA,
            "pulled_at_utc": dt.datetime.now(dt.timezone.utc)
            .replace(microsecond=0)
            .isoformat()
            .replace("+00:00", "Z"),
            "slot_id": slot_id,
            "run_as_package": args.run_as_package,
            "adb_serial": args.serial or "",
            "device_lab_root": args.device_lab_root,
            "output_root": str(output_root),
            "slot_path": str(final_slot),
            "artifact_sha256": artifact_digests,
        }
        summary_errors = _write_summary(args.summary_out, summary)
        if summary_errors:
            return 1, final_slot, summary_errors
    return 0, final_slot, []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Pull raw Kagemusha Android device-lab artifacts written by "
            "KagemushaDeviceLabArtifactExportTest from an attached physical device."
        )
    )
    parser.add_argument("--adb", default="adb")
    parser.add_argument("--serial")
    parser.add_argument("--run-as-package", default=DEFAULT_RUN_AS_PACKAGE)
    parser.add_argument("--device-lab-root", default=DEFAULT_DEVICE_LAB_DEVICE_ROOT)
    parser.add_argument("--slot-id")
    parser.add_argument("--out-root", type=Path, default=DEFAULT_OUT_ROOT)
    parser.add_argument("--summary-out", type=Path)
    parser.add_argument(
        "--adb-timeout-seconds",
        type=int,
        default=120,
        help="ADB subprocess timeout in seconds; 0 disables the timeout.",
    )
    parser.epilog = (
        f"Latest-slot command: {ADB_LATEST_SLOT_COMMAND_HELP}\n"
        f"Raw-tar command: {ADB_PULL_TAR_COMMAND_HELP}\n"
        "The extractor rejects symlink, hardlink, special-file, traversal, "
        "duplicate, oversized, and slot-mismatched tar members."
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    status, slot_path, errors = pull_raw_slot(args)
    if status != 0:
        for error in errors:
            print(f"[kagemusha-android-raw-pull] {error}", file=sys.stderr)
        return status
    assert slot_path is not None
    print(f"[kagemusha-android-raw-pull] wrote {slot_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
