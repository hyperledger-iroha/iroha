#!/usr/bin/env python3
"""Render a slot-bound Kagemusha Android attestation verifier report."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import tempfile
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402


DEFAULT_APP_PACKAGE_NAME = "org.hyperledger.iroha.sdk.offline.wallet.lab"
DEFAULT_VERIFIER = "android-keystore-attestation-harness"
HARNESS_RESULT_FIELDS: frozenset[str] = frozenset(
    {
        "alias",
        "attestation_security_level",
        "keymaster_security_level",
        "strongbox_attestation",
        "challenge_hex",
        "chain_length",
    }
)
PHYSICAL_DEVICE_ASSERTION_REQUIRED = (
    "physical device attestation must be explicitly asserted with "
    "--physical-device-attestation"
)
ATTESTATION_STRONGBOX_REQUIRED = (
    "attestation harness result attestation_security_level must be STRONGBOX"
)
KEYMASTER_STRONGBOX_REQUIRED = (
    "attestation harness result keymaster_security_level must be STRONGBOX"
)


def _json_dumps(payload: dict[str, Any]) -> str:
    return json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"


def _reject_whitespace(value: str, label: str, errors: list[str]) -> bool:
    if value != value.strip() or any(ch.isspace() for ch in value):
        errors.append(f"{label} must not contain whitespace")
        return True
    return False


def _reject_control(value: str, label: str, errors: list[str]) -> bool:
    if device_lab._contains_control_character(value):
        errors.append(f"{label} must not contain control characters")
        return True
    return False


def _safe_single_name(value: str, label: str, errors: list[str]) -> str | None:
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{label} must be a non-empty string")
        return None
    if _reject_whitespace(value, label, errors):
        return None
    if _reject_control(value, label, errors):
        return None
    candidate = PurePosixPath(value)
    if (
        device_lab.SECRET_RE.search(value)
        or candidate.is_absolute()
        or "\\" in value
        or len(candidate.parts) != 1
        or candidate.name in {"", ".", ".."}
        or ".." in candidate.parts
    ):
        errors.append(f"{label} must be a single safe directory name")
        return None
    if candidate.as_posix() != value:
        errors.append(f"{label} must be a canonical single directory name")
        return None
    return candidate.name


def _string_value(value: str | None, label: str, errors: list[str]) -> str | None:
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{label} must be a non-empty string")
        return None
    if _reject_whitespace(value, label, errors):
        return None
    if _reject_control(value, label, errors):
        return None
    if device_lab.SECRET_RE.search(value):
        errors.append(f"{label} must not contain secret-looking material")
        return None
    return value


def _normalise_strongbox_level(
    value: Any,
    label: str,
    errors: list[str],
    strongbox_error: str | None = None,
) -> str | None:
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{label} must be a non-empty string")
        return None
    if _reject_whitespace(value, label, errors):
        return None
    if _reject_control(value, label, errors):
        return None
    if value not in device_lab.STRONGBOX_LEVELS:
        errors.append(strongbox_error or f"{label} must be STRONGBOX")
        return None
    return "STRONGBOX"


def _decode_challenge_hex(
    value: Any,
    errors: list[str],
    label: str = "attestation harness result challenge_hex",
) -> bytes | None:
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{label} must be a non-empty string")
        return None
    if value != value.strip() or any(ch.isspace() for ch in value):
        errors.append(f"{label} must be lowercase hexadecimal without whitespace")
        return None
    if device_lab._contains_control_character(value):
        errors.append(f"{label} must not contain control characters")
        return None
    if any(ch not in "0123456789abcdef" for ch in value):
        errors.append(f"{label} must be lowercase hexadecimal without whitespace")
        return None
    if len(value) % 2 != 0:
        errors.append(f"{label} must have even length")
        return None
    try:
        decoded = bytes.fromhex(value)
    except ValueError:
        errors.append(f"{label} must be hex")
        return None
    if not decoded:
        errors.append(f"{label} must be non-empty")
        return None
    return decoded


def _pem_certificate_count(payload: bytes) -> int:
    return payload.count(b"-----BEGIN CERTIFICATE-----")


def _slot_relative_chain_path(
    source: Path,
    requested: str | None,
    errors: list[str],
) -> str | None:
    raw = requested if isinstance(requested, str) and requested != "" else None
    if raw is None:
        raw = f"attestation/{source.name}"
    elif raw != raw.strip() or any(ch.isspace() for ch in raw):
        errors.append("attestation certificate chain path must not contain whitespace")
        return None
    if device_lab._contains_control_character(raw):
        errors.append("attestation certificate chain path must not contain control characters")
        return None
    if "\\" in raw:
        errors.append("attestation certificate chain path must not contain backslashes")
        return None
    if device_lab.SECRET_RE.search(raw):
        errors.append("attestation certificate chain path must not contain secret-looking material")
        return None
    candidate = PurePosixPath(raw)
    if (
        candidate.is_absolute()
        or ".." in candidate.parts
        or len(candidate.parts) != 2
        or candidate.parts[0] != "attestation"
        or candidate.name in {"", ".", ".."}
    ):
        errors.append("attestation certificate chain path must stay under attestation/")
        return None
    if candidate.as_posix() != raw:
        errors.append("attestation certificate chain path must be canonical")
        return None
    if Path(candidate.name).suffix.lower() not in device_lab.ATTESTATION_CERTIFICATE_CHAIN_SUFFIXES:
        errors.append("attestation certificate chain path must end in .pem or .der")
        return None
    return candidate.as_posix()


def _read_validated_chain(path: Path, errors: list[str]) -> tuple[bytes | None, str | None]:
    label = "attestation certificate chain"
    path_text = str(path)
    if device_lab.SECRET_RE.search(path_text):
        errors.append(f"{label} path must not contain secret-looking material")
        return None, None
    if device_lab._contains_control_character(str(path)):
        errors.append(f"{label} path must not contain control characters")
        return None, None
    if device_lab._contains_control_character(path_text):
        errors.append(f"{label} path must not contain control characters")
        return None, None
    if "\\" in path_text:
        errors.append(f"{label} path must not contain backslashes")
        return None, None
    if ".." in path.parts:
        errors.append(f"{label} path must be canonical")
        return None, None
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        errors.extend(ancestor_errors)
        return None, None
    try:
        expected_stat = path.lstat()
    except FileNotFoundError:
        errors.append(f"{label} is missing")
        return None, None
    except OSError:
        errors.append(f"{label} file metadata could not be read")
        return None, None
    if stat.S_ISLNK(expected_stat.st_mode):
        errors.append(f"{label} must not be a symlink")
        return None, None
    if not stat.S_ISREG(expected_stat.st_mode):
        errors.append(f"{label} must be a regular file")
        return None, None
    try:
        if path.stat().st_nlink > 1:
            errors.append(f"{label} must not be hardlinked")
            return None, None
    except OSError:
        errors.append(f"{label} hardlink metadata could not be read")
        return None, None

    chunks: list[bytes] = []
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            if open_identity != expected_identity:
                errors.append(f"{label} changed while being read")
                return None, None
            if not stat.S_ISREG(open_stat.st_mode):
                errors.append(f"{label} must be a regular file")
                return None, None
            if open_stat.st_nlink > 1:
                errors.append(f"{label} must not be hardlinked")
                return None, None
            if open_stat.st_size > device_lab.MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES:
                errors.append(
                    f"{label} must be no more than "
                    f"{device_lab.MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES} bytes"
                )
                return None, None
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > device_lab.MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES:
                    errors.append(
                        f"{label} must be no more than "
                        f"{device_lab.MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES} bytes"
                    )
                    return None, None
                chunks.append(chunk)
            final_stat = path.lstat()
            if (final_stat.st_dev, final_stat.st_ino) != expected_identity:
                errors.append(f"{label} changed while being read")
                return None, None
    except OSError:
        errors.append(f"{label} could not be read")
        return None, None

    data = b"".join(chunks)
    if not data:
        errors.append(f"{label} must be non-empty")
        return None, None
    suffix = path.suffix.lower()
    if suffix == ".pem" and b"-----BEGIN CERTIFICATE-----" not in data:
        errors.append(f"{label} PEM must contain certificate boundaries")
    if suffix == ".der" and data[:1] != b"\x30":
        errors.append(f"{label} DER must start with an ASN.1 SEQUENCE")
    return data, hashlib.sha256(data).hexdigest()


def _load_harness_result(path: Path, errors: list[str]) -> dict[str, Any] | None:
    result = device_lab._load_json(path, "attestation harness result", errors)
    if result is None:
        return None
    for field in sorted(set(result) - HARNESS_RESULT_FIELDS):
        errors.append(
            "attestation harness result contains unexpected field "
            f"{device_lab._display_path(field)}"
        )
    return result


def build_report(args: argparse.Namespace) -> tuple[dict[str, Any] | None, list[str]]:
    """Build the closed-schema verifier report from harness output."""

    errors: list[str] = []
    slot_id = _safe_single_name(args.slot_id, "slot id", errors)
    device_fingerprint = _string_value(args.device_fingerprint, "device fingerprint", errors)
    os_build_id = _string_value(args.os_build_id, "os build id", errors)
    app_package_name = _string_value(args.app_package_name, "app package name", errors)
    verifier = _string_value(args.verifier, "verifier", errors)
    if not args.physical_device_attestation:
        errors.append(PHYSICAL_DEVICE_ASSERTION_REQUIRED)

    result = _load_harness_result(args.harness_result, errors)
    chain_relative = _slot_relative_chain_path(
        args.attestation_certificate_chain,
        args.attestation_certificate_chain_path,
        errors,
    )
    chain_data, chain_digest = _read_validated_chain(args.attestation_certificate_chain, errors)
    if result is None or chain_relative is None or chain_data is None or chain_digest is None:
        return None, errors

    _string_value(result.get("alias"), "attestation harness result alias", errors)
    attestation_level = _normalise_strongbox_level(
        result.get("attestation_security_level"),
        "attestation harness result attestation_security_level",
        errors,
        ATTESTATION_STRONGBOX_REQUIRED,
    )
    keymaster_level = _normalise_strongbox_level(
        result.get("keymaster_security_level"),
        "attestation harness result keymaster_security_level",
        errors,
        KEYMASTER_STRONGBOX_REQUIRED,
    )
    if result.get("strongbox_attestation") is not True:
        errors.append("attestation harness result strongbox_attestation must be true")
    chain_length = result.get("chain_length")
    if not isinstance(chain_length, int) or chain_length < 2:
        errors.append("attestation harness result chain_length must be at least 2")
    elif chain_relative.endswith(".pem"):
        certificate_count = _pem_certificate_count(chain_data)
        if certificate_count < 2:
            errors.append(
                "attestation certificate chain PEM must contain at least two certificates"
            )
        elif chain_length != certificate_count:
            errors.append(
                "attestation harness result chain_length must match "
                "attestation certificate-chain certificate count"
            )
    challenge = _decode_challenge_hex(result.get("challenge_hex"), errors)
    if challenge is not None:
        computed_challenge_sha256 = hashlib.sha256(challenge).hexdigest()
        expected_challenge_hex = (
            args.expected_challenge_hex
            if isinstance(args.expected_challenge_hex, str)
            and args.expected_challenge_hex != ""
            else None
        )
        if expected_challenge_hex is not None:
            expected_bytes = _decode_challenge_hex(
                expected_challenge_hex,
                errors,
                "--expected-challenge-hex",
            )
            if expected_bytes is not None and expected_bytes != challenge:
                errors.append("attestation harness result challenge_hex must match --expected-challenge-hex")
        expected_digest = (
            args.attestation_challenge_sha256
            if isinstance(args.attestation_challenge_sha256, str)
            and args.attestation_challenge_sha256 != ""
            else None
        )
        if expected_digest is not None:
            if not device_lab.SHA256_HEX_RE.fullmatch(expected_digest):
                errors.append("--attestation-challenge-sha256 must be lowercase sha256 hex")
            elif expected_digest != computed_challenge_sha256:
                errors.append(
                    "attestation challenge SHA-256 must match attestation harness result challenge_hex"
                )
    else:
        computed_challenge_sha256 = None

    if errors:
        return None, errors
    assert slot_id is not None
    assert device_fingerprint is not None
    assert os_build_id is not None
    assert app_package_name is not None
    assert verifier is not None
    assert attestation_level is not None
    assert keymaster_level is not None
    assert computed_challenge_sha256 is not None

    return (
        {
            "schema": device_lab.ATTESTATION_REPORT_SCHEMA,
            "slot_id": slot_id,
            "device_fingerprint": device_fingerprint,
            "os_build_id": os_build_id,
            "app_package_name": app_package_name,
            "attestation_challenge_sha256": computed_challenge_sha256,
            "attestation_certificate_chain_path": chain_relative,
            "attestation_certificate_chain_sha256": chain_digest,
            "verifier": verifier,
            "verification": {
                "status": "ok",
                "strongbox_attestation": True,
                "physical_device_attestation": True,
                "keymint_security_level": keymaster_level,
                "attestation_security_level": attestation_level,
                "keymaster_security_level": keymaster_level,
            },
        },
        [],
    )


def _cleanup_temp_output(
    path: Path,
    label: str,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    if expected_identity is None:
        return [f"{label} temporary file metadata could not be read"]
    try:
        parent_fd = os.open(path.parent, device_lab._directory_open_flags())
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
            or device_lab._file_identity(temp_stat) != expected_identity
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


def write_report(path: Path, payload: dict[str, Any]) -> list[str]:
    """Write the report after output path preflight and readback verification."""

    label = "attestation report output"
    errors = device_lab.validate_summary_output_path(path, label)
    if errors:
        return errors
    try:
        parent_stat = path.parent.lstat()
    except OSError:
        return [f"{label} parent directory metadata could not be read"]
    if stat.S_ISLNK(parent_stat.st_mode) or not stat.S_ISDIR(parent_stat.st_mode):
        return [f"{label} parent directory could not be synced"]
    parent_identity = device_lab._file_identity(parent_stat)
    try:
        text = _json_dumps(payload)
    except ValueError:
        return [f"{label} is not strict JSON"]
    if len(text.encode("utf-8")) > device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
        return [
            f"{label} must be no more than "
            f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
        ]

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
            tmp_identity = device_lab._file_identity(os.fstat(handle.fileno()))
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        errors = device_lab.validate_summary_output_path(path, label)
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

    errors = device_lab.validate_summary_output_path(path, label)
    if errors:
        return errors
    sync_errors = device_lab._sync_summary_output_parent(
        path.parent,
        label,
        expected_identity=parent_identity,
    )
    if sync_errors:
        return sync_errors
    errors = device_lab.validate_summary_output_path(path, label)
    if errors:
        return errors
    try:
        expected_stat = path.lstat()
    except (FileNotFoundError, OSError):
        return [f"{label} write verification failed"]
    readback = device_lab._load_json(path, label, errors)
    if errors:
        return errors
    if readback != payload:
        return [f"{label} write verification failed"]
    try:
        final_stat = path.lstat()
    except OSError:
        return [f"{label} write verification failed"]
    if (final_stat.st_dev, final_stat.st_ino) != (expected_stat.st_dev, expected_stat.st_ino):
        return [f"{label} changed while being read"]
    return []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Render attestation/report.json from scripts/android_keystore_attestation.sh "
            "result JSON and slot identity."
        )
    )
    parser.add_argument("--harness-result", type=Path, required=True)
    parser.add_argument("--slot-id", required=True)
    parser.add_argument("--device-fingerprint", required=True)
    parser.add_argument("--os-build-id", required=True)
    parser.add_argument("--app-package-name", default=DEFAULT_APP_PACKAGE_NAME)
    parser.add_argument("--attestation-certificate-chain", type=Path, required=True)
    parser.add_argument("--attestation-certificate-chain-path")
    parser.add_argument("--attestation-challenge-sha256")
    parser.add_argument("--expected-challenge-hex")
    parser.add_argument("--verifier", default=DEFAULT_VERIFIER)
    parser.add_argument(
        "--physical-device-attestation",
        action="store_true",
        help="Assert the bundle was collected from a physical Android device.",
    )
    parser.add_argument("--out", type=Path, required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    report, errors = build_report(args)
    if report is not None:
        errors.extend(write_report(args.out, report))
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    assert report is not None
    print(args.out)
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
