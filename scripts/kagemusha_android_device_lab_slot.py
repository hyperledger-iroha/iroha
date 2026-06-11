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
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402
import sign_android_device_lab_evidence as evidence_signer  # noqa: E402


DEFAULT_APP_PACKAGE_NAME = "org.hyperledger.iroha.sdk.offline.wallet.lab"
DEFAULT_POLICY_BYTES = b"kagemusha-offline-wallet-policy-v1"
DEFAULT_ATTESTATION_HARNESS_RESULT_PATH = "attestation/harness-result.json"
DEFAULT_ATTESTATION_CHAIN_PATH = "attestation/keymint-certificate-chain.pem"
DEFAULT_OFFLINE_WALLET_APK_PATH = "evidence/offline-wallet-release.apk"
DEFAULT_D2D_TRANSCRIPT_PATH = "handoff/d2d-payment-transcript.json"
DEFAULT_WALLET_TRANSCRIPT_PATH = "wallet/wallet-integrity-transcript.json"
ATTESTATION_REPORT_DEVICE_FINGERPRINT_MISMATCH = (
    "attestation/report.json device_fingerprint must match device identity"
)
ATTESTATION_REPORT_OS_BUILD_MISMATCH = (
    "attestation/report.json os_build_id must match device identity"
)
WALLET_ROLLBACK_REQUIRED = (
    "wallet integrity transcript rollback_rejection_passed must be true"
)

DEVICE_FAMILY_MODEL_PREFIXES: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("Google Pixel 6 / 6a", ("pixel 6", "pixel 6a", "oriole", "bluejay")),
    ("Google Pixel 7 / 7 Pro", ("pixel 7", "pixel 7 pro", "panther", "cheetah")),
    (
        "Google Pixel 8 / 8a / 8 Pro",
        ("pixel 8", "pixel 8a", "pixel 8 pro", "shiba", "akita", "husky"),
    ),
    ("Google Pixel Fold / Tablet", ("pixel fold", "pixel tablet", "felix", "tangorpro")),
    ("Samsung Galaxy S23", ("galaxy s23", "sm-s911", "sm-s916", "sm-s918")),
    ("Samsung Galaxy S24", ("galaxy s24", "sm-s921", "sm-s926", "sm-s928")),
)


def _json_dumps(payload: dict[str, Any]) -> str:
    return json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"


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
    label: str,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    try:
        dir_fd = os.open(path, _directory_open_flags())
    except OSError:
        return [f"{label} parent directory could not be synced"]
    try:
        dir_stat = os.fstat(dir_fd)
        if not stat.S_ISDIR(dir_stat.st_mode):
            return [f"{label} parent directory could not be synced"]
        if expected_identity is not None and _file_identity(dir_stat) != expected_identity:
            return [f"{label} parent directory changed before sync"]
        os.fsync(dir_fd)
    except OSError:
        return [f"{label} parent directory could not be synced"]
    finally:
        os.close(dir_fd)
    return []


def _verify_written_bytes(path: Path, expected_bytes: bytes, label: str) -> list[str]:
    try:
        expected_stat = path.lstat()
    except OSError:
        return [f"{label} metadata could not be read after write"]
    if stat.S_ISLNK(expected_stat.st_mode) or not stat.S_ISREG(expected_stat.st_mode):
        return [f"{label} changed after write"]
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
    finally:
        os.close(parent_fd)
    return []


def _write_json(path: Path, payload: dict[str, Any], label: str) -> list[str]:
    try:
        encoded = _json_dumps(payload).encode("utf-8")
    except (TypeError, ValueError):
        return [f"{label} is not strict JSON"]
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
    except OSError:
        return [f"{label} parent directory could not be created"]
    try:
        json_parent_stat = path.parent.lstat()
    except OSError:
        return [f"{label} parent metadata could not be read"]
    if stat.S_ISLNK(json_parent_stat.st_mode) or not stat.S_ISDIR(json_parent_stat.st_mode):
        return [f"{label} parent directory could not be synced"]
    json_parent_identity = _file_identity(json_parent_stat)
    tmp_path = path.parent / f".{path.name}.android-slot.tmp"
    tmp_identity: tuple[int, int] | None = None
    try:
        if tmp_path.exists() or tmp_path.is_symlink():
            return [f"{label} temporary output already exists"]
        with tmp_path.open("xb") as handle:
            tmp_identity = _file_identity(os.fstat(handle.fileno()))
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp_path, path)
        sync_errors = _sync_directory(
            path.parent,
            label,
            expected_identity=json_parent_identity,
        )
        if sync_errors:
            return sync_errors
    except OSError:
        cleanup_errors = _cleanup_temp_output(tmp_path, label, tmp_identity)
        return [f"{label} could not be written", *cleanup_errors]
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
            if _file_identity(temp_parent_stat) != expected_temp_parent_identity:
                return ["staged slot parent directory changed before publish"]
            stage_stat = os.stat(
                stage_slot.name,
                dir_fd=temp_parent_fd,
                follow_symlinks=False,
            )
            if not stat.S_ISDIR(stage_stat.st_mode):
                return ["staged slot must be a directory"]
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
) -> None:
    try:
        parent_fd = os.open(temp_parent.parent, _directory_open_flags())
    except OSError:
        return
    try:
        try:
            temp_parent_stat = os.stat(
                temp_parent.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except OSError:
            return
        if (
            not stat.S_ISDIR(temp_parent_stat.st_mode)
            or _file_identity(temp_parent_stat) != expected_identity
        ):
            return
        shutil.rmtree(temp_parent.name, ignore_errors=True, dir_fd=parent_fd)
    finally:
        os.close(parent_fd)


def _sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _single_safe_slot_id(slot_id: str) -> str | None:
    candidate = PurePosixPath(slot_id)
    if (
        not slot_id.strip()
        or device_lab.SECRET_RE.search(slot_id)
        or candidate.is_absolute()
        or len(candidate.parts) != 1
        or candidate.name in {"", ".", ".."}
        or ".." in candidate.parts
    ):
        return None
    return candidate.name


def _normalise_source_path(
    path: Path,
    label: str,
    errors: list[str],
) -> tuple[Path, os.stat_result] | None:
    if device_lab.SECRET_RE.search(str(path)):
        errors.append(f"{label} path must not contain secret-looking material")
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
            destination.parent.mkdir(parents=True, exist_ok=True)
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
            with destination.open("xb") as out:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    size += len(chunk)
                    if size > max_bytes:
                        errors.append(f"{label} must not exceed {max_bytes} bytes")
                        return None
                    digest.update(chunk)
                    out.write(chunk)
                out.flush()
                os.fsync(out.fileno())
            final_stat = source_path.lstat()
            if (final_stat.st_dev, final_stat.st_ino) != expected_identity:
                errors.append(f"{label} changed while being read")
                return None
    except OSError:
        errors.append(f"{label} could not be read")
        return None
    if size <= 0:
        errors.append(f"{label} must be non-empty")
        return None
    copied_digest = digest.hexdigest()
    sync_errors = _sync_directory(
        destination.parent,
        label,
        expected_identity=destination_parent_identity,
    )
    if sync_errors:
        errors.extend(sync_errors)
        return None
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
    if device_lab.SECRET_RE.search(value):
        errors.append(f"{label} {key} must not contain secret-looking material")
        return None
    return value.strip()


def _require_source_true(
    payload: dict[str, Any],
    key: str,
    label: str,
    errors: list[str],
) -> None:
    if payload.get(key) is not True:
        errors.append(f"{label} {key} must be true")


def _run_adb_getprop(adb: str, serial: str | None, prop: str) -> str:
    command = [adb]
    if serial:
        command.extend(["-s", serial])
    command.extend(["shell", "getprop", prop])
    result = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return result.stdout.strip()


def read_device_identity(
    *,
    adb: str,
    serial: str | None,
    device_fingerprint: str | None,
    os_build_id: str | None,
    device_model: str | None,
    device_codename: str | None,
    errors: list[str],
) -> dict[str, str]:
    """Return device identity from overrides or the attached Android device."""

    facts: dict[str, str] = {}
    queries = {
        "device_fingerprint": ("ro.build.fingerprint", device_fingerprint),
        "os_build_id": ("ro.build.id", os_build_id),
        "device_model": ("ro.product.model", device_model),
        "device_codename": ("ro.product.device", device_codename),
    }
    for key, (prop, override) in queries.items():
        value = override.strip() if isinstance(override, str) and override.strip() else None
        if value is None:
            try:
                value = _run_adb_getprop(adb, serial, prop)
            except (OSError, subprocess.CalledProcessError) as exc:
                errors.append(f"adb getprop {prop} failed: {exc}")
                continue
        if not value:
            errors.append(f"{key} could not be determined")
            continue
        if device_lab.SECRET_RE.search(value):
            errors.append(f"{key} must not contain secret-looking material")
            continue
        facts[key] = value
    return facts


def infer_device_family(model: str | None, codename: str | None) -> str | None:
    """Infer a standard Kagemusha device family from ADB model/codename."""

    haystack = " ".join(value.lower() for value in (model, codename) if value)
    for family, markers in DEVICE_FAMILY_MODEL_PREFIXES:
        if any(marker in haystack for marker in markers):
            return family
    return None


def resolve_device_family(
    requested: str | None,
    facts: dict[str, str],
    errors: list[str],
) -> str | None:
    family = requested.strip() if isinstance(requested, str) and requested.strip() else None
    if family is None:
        family = infer_device_family(facts.get("device_model"), facts.get("device_codename"))
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
        if level is not None and level.upper() not in device_lab.STRONGBOX_LEVELS:
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
        if len(challenge_hex) % 2 != 0:
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
            expected = payload.get("attestation_challenge_sha256")
            if isinstance(expected, str) and expected.strip() and expected != challenge_digest:
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
) -> dict[str, Any]:
    app_package_name = attestation_result.get("app_package_name") or DEFAULT_APP_PACKAGE_NAME
    offline_policy_sha256 = attestation_result.get("offline_wallet_policy_sha256")
    if not isinstance(offline_policy_sha256, str) or not offline_policy_sha256.strip():
        offline_policy_sha256 = _sha256_bytes(DEFAULT_POLICY_BYTES)
    return {
        "schema": "iroha.android.device_lab.kagemusha.v1",
        "slot_id": slot_id,
        "device_family": family,
        "device_fingerprint": facts["device_fingerprint"],
        "os_build_id": facts["os_build_id"],
        "minimum_os": device_lab.KAGEMUSHA_STANDARD_DEVICE_MINIMUM_OS[family],
        "app_package_name": app_package_name,
        "attestation_certificate_chain_path": attestation_chain_path,
        "offline_wallet_apk_path": DEFAULT_OFFLINE_WALLET_APK_PATH,
        "d2d_payment_transcript_path": DEFAULT_D2D_TRANSCRIPT_PATH,
        "wallet_integrity_transcript_path": DEFAULT_WALLET_TRANSCRIPT_PATH,
        "app_signing_certificate_sha256": attestation_result.get(
            "app_signing_certificate_sha256"
        ),
        "attestation_challenge_sha256": attestation_result.get(
            "attestation_challenge_sha256"
        ),
        "attestation_certificate_chain_sha256": attestation_chain_sha256,
        "offline_wallet_policy_sha256": offline_policy_sha256,
        "offline_wallet_apk_sha256": offline_wallet_apk_sha256,
        "d2d_payment_transcript_sha256": d2d_payment_transcript_sha256,
        "wallet_integrity_transcript_sha256": wallet_integrity_transcript_sha256,
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


def validate_slot_source_claims(
    *,
    attestation_result: dict[str, Any],
    attestation_report: dict[str, Any],
    wallet_integrity_transcript: dict[str, Any],
    errors: list[str],
) -> None:
    _require_source_true(attestation_result, "strongbox_attestation", "attestation/result.json", errors)
    _require_source_true(
        attestation_result,
        "physical_device_attestation",
        "attestation/result.json",
        errors,
    )
    verification = attestation_report.get("verification")
    if not isinstance(verification, dict):
        errors.append("attestation/report.json verification must be an object")
    else:
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
    _require_source_true(
        wallet_integrity_transcript,
        "one_use_key_rotation_passed",
        "wallet integrity transcript",
        errors,
    )
    if wallet_integrity_transcript.get("rollback_rejection_passed") is not True:
        errors.append(WALLET_ROLLBACK_REQUIRED)


def assemble_slot(args: argparse.Namespace) -> tuple[int, Path | None, list[str]]:
    """Assemble the requested slot and optionally sign it."""

    errors: list[str] = []
    slot_id = _single_safe_slot_id(args.slot_id)
    if slot_id is None:
        return 1, None, ["slot id must be a single safe directory name"]

    root = args.slot_root
    if device_lab.SECRET_RE.search(str(root)):
        return 1, None, ["device-lab root path must not contain secret-looking material"]
    root_exists, root_errors = device_lab.classify_device_lab_root_path(root)
    if root_errors:
        return 1, None, root_errors
    if not root_exists:
        root.parent.mkdir(parents=True, exist_ok=True)
        root.mkdir()
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

    facts = read_device_identity(
        adb=args.adb,
        serial=args.serial,
        device_fingerprint=args.device_fingerprint,
        os_build_id=args.os_build_id,
        device_model=args.device_model,
        device_codename=args.device_codename,
        errors=errors,
    )
    family = resolve_device_family(args.device_family, facts, errors)
    if errors or family is None:
        return 1, None, errors

    result = _load_source_json(args.attestation_result, "attestation result", errors)
    report = _load_source_json(args.attestation_report, "attestation verifier report", errors)
    d2d = _load_source_json(args.d2d_payment_transcript, "D2D payment transcript", errors)
    wallet = _load_source_json(
        args.wallet_integrity_transcript,
        "wallet integrity transcript",
        errors,
    )
    if result is None or report is None or d2d is None or wallet is None:
        return 1, None, errors

    validate_slot_source_claims(
        attestation_result=result,
        attestation_report=report,
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
    stage_slot = temp_parent / slot_id
    try:
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
        )
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
    finally:
        _cleanup_temp_parent(temp_parent, expected_identity=temp_parent_identity)


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
