"""Tests for scripts/check_android_device_lab_slot.py."""

from __future__ import annotations

import argparse
import ast
import copy
import gzip
import hashlib
import importlib.util
import io
import json
import os
import shutil
import stat
import subprocess
import sys
import tarfile
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
import tempfile
import unittest
from unittest import mock


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_android_device_lab_slot.py"
SPEC = importlib.util.spec_from_file_location("check_android_device_lab_slot", MODULE_PATH)
assert SPEC and SPEC.loader  # pragma: no cover - import guard
device_lab = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = device_lab
SPEC.loader.exec_module(device_lab)  # type: ignore[misc]

try:
    from scripts.tests import (
        android_apk_authority_fixtures as _android_apk_fixtures,
        android_attestation_certificate_profile_fixtures as _android_x509_fixtures,
    )
except ModuleNotFoundError:
    import android_apk_authority_fixtures as _android_apk_fixtures
    import android_attestation_certificate_profile_fixtures as _android_x509_fixtures

_android_x509_fixtures.bind_device_lab(device_lab)
test_android_attestation_chain = _android_x509_fixtures.test_android_attestation_chain
test_android_attestation_chain.__test__ = False
android_attestation_metadata = _android_x509_fixtures.android_attestation_metadata

SIGNER_MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "sign_android_device_lab_evidence.py"
)
SIGNER_SPEC = importlib.util.spec_from_file_location(
    "sign_android_device_lab_evidence",
    SIGNER_MODULE_PATH,
)
assert SIGNER_SPEC and SIGNER_SPEC.loader  # pragma: no cover - import guard
evidence_signer = importlib.util.module_from_spec(SIGNER_SPEC)
SIGNER_SPEC.loader.exec_module(evidence_signer)  # type: ignore[misc]


KAGEMUSHA_ANDROID_RAW_TEST_COMMANDS = (
    device_lab.KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS
)
_MISSING_PATH_METHOD = object()
_PATH_TYPE = type(Path("."))
_PATH_TYPE_METHODS = ("exists", "is_symlink", "lstat", "mkdir", "rglob", "stat")
_ORIGINAL_PATH_TYPE_METHODS = {
    name: _PATH_TYPE.__dict__.get(name, _MISSING_PATH_METHOD)
    for name in _PATH_TYPE_METHODS
}


def restore_path_type_method_shadows() -> None:
    for name, original in _ORIGINAL_PATH_TYPE_METHODS.items():
        if original is _MISSING_PATH_METHOD:
            if name in _PATH_TYPE.__dict__:
                delattr(_PATH_TYPE, name)
        else:
            setattr(_PATH_TYPE, name, original)


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    path.chmod(0o600)


def write_json(path: Path, payload: dict) -> None:
    write_text(path, json.dumps(payload, indent=2, sort_keys=True) + "\n")


def inject_duplicate_json_key(path: Path, key: str, first_value) -> None:
    text = path.read_text(encoding="utf-8")
    marker = f'"{key}":'
    index = text.find(marker)
    if index < 0:
        raise AssertionError(f"{path} does not contain JSON key {key}")
    line_start = text.rfind("\n", 0, index) + 1
    line_end = text.find("\n", index)
    if line_end < 0:
        line_end = len(text)
    line = text[line_start:line_end]
    indent = line[: len(line) - len(line.lstrip())]
    duplicate_line = f'{indent}"{key}": {json.dumps(first_value)},\n'
    path.write_text(text[:line_start] + duplicate_line + text[line_start:], encoding="utf-8")


def patch_slot_iterdir_failure(slot: Path):
    original_iterdir = Path.iterdir

    def failing_iterdir(path: Path):
        if path == slot:
            raise OSError("simulated slot listing failure")
        return original_iterdir(path)

    return mock.patch.object(Path, "iterdir", failing_iterdir)


def rewrite_sha256sum(slot: Path) -> None:
    lines = []
    for relative in sorted(device_lab._slot_files(slot)):  # type: ignore[attr-defined]
        path = slot / relative
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        lines.append(f"{digest}  {relative}")
    write_text(slot / "sha256sum.txt", "\n".join(lines) + "\n")


def replace_with_symlink(test_case: unittest.TestCase, link_path: Path, target_path: Path) -> None:
    link_path.unlink()
    try:
        link_path.symlink_to(target_path)
    except (NotImplementedError, OSError) as exc:
        test_case.skipTest(f"symlinks are not available in this test environment: {exc}")


def create_dir_symlink(test_case: unittest.TestCase, link_path: Path, target_path: Path) -> None:
    try:
        link_path.symlink_to(target_path, target_is_directory=True)
    except (NotImplementedError, OSError) as exc:
        test_case.skipTest(f"directory symlinks are not available in this test environment: {exc}")


def replace_with_hardlink(test_case: unittest.TestCase, link_path: Path, target_path: Path) -> None:
    link_path.unlink()
    try:
        os.link(target_path, link_path)
    except (AttributeError, NotImplementedError, OSError) as exc:
        test_case.skipTest(f"hardlinks are not available in this test environment: {exc}")


def replace_with_fifo(test_case: unittest.TestCase, fifo_path: Path) -> None:
    fifo_path.unlink()
    try:
        os.mkfifo(fifo_path)
    except (AttributeError, NotImplementedError, OSError) as exc:
        test_case.skipTest(f"FIFOs are not available in this test environment: {exc}")


def with_read_bytes_failure(target_path: Path, callback):
    original_read_bytes = Path.read_bytes

    def failing_read_bytes(path: Path) -> bytes:
        if path == target_path:
            raise OSError("simulated read failure")
        return original_read_bytes(path)

    try:
        Path.read_bytes = failing_read_bytes
        return callback()
    finally:
        Path.read_bytes = original_read_bytes


def with_open_failure(target_path: Path, callback):
    original_open = Path.open

    def failing_open(path: Path, *args, **kwargs):
        if path == target_path:
            raise OSError("simulated open failure")
        return original_open(path, *args, **kwargs)

    try:
        Path.open = failing_open
        return callback()
    finally:
        Path.open = original_open


def with_write_text_failure(target_path: Path, callback):
    original_write_text = Path.write_text

    def failing_write_text(path: Path, *args, **kwargs) -> int:
        if path == target_path:
            raise OSError("simulated write failure")
        return original_write_text(path, *args, **kwargs)

    try:
        Path.write_text = failing_write_text
        return callback()
    finally:
        Path.write_text = original_write_text


def canonical_signed_evidence_payload(evidence: dict) -> bytes:
    payload = {
        key: value
        for key, value in evidence.items()
        if key not in {"signature", "signature_payload_sha256"}
    }
    return json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")


def create_test_signer(root: Path) -> dict[str, Path | str]:
    signer_dir = root / "trusted-signer"
    signer_dir.mkdir(parents=True, exist_ok=True)
    private_key = signer_dir / "ed25519-private.pem"
    public_key = signer_dir / "ed25519-public.pem"
    subprocess.run(
        ["openssl", "genpkey", "-algorithm", "ED25519", "-out", str(private_key)],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    subprocess.run(
        ["openssl", "pkey", "-in", str(private_key), "-pubout", "-out", str(public_key)],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    der = subprocess.run(
        [
            "openssl",
            "pkey",
            "-pubin",
            "-in",
            str(public_key),
            "-pubout",
            "-outform",
            "DER",
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout
    return {
        "private_key": private_key,
        "public_key": public_key,
        "public_key_sha256": hashlib.sha256(der).hexdigest(),
    }


def openssl_sign_ed25519(private_key: Path, payload: bytes) -> str:
    with tempfile.TemporaryDirectory() as temp:
        temp_path = Path(temp)
        payload_path = temp_path / "payload.bin"
        signature_path = temp_path / "signature.bin"
        payload_path.write_bytes(payload)
        subprocess.run(
            [
                "openssl",
                "pkeyutl",
                "-sign",
                "-inkey",
                str(private_key),
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
        return signature_path.read_bytes().hex()


def sign_evidence(evidence: dict, signer: dict[str, Path | str]) -> dict:
    payload = canonical_signed_evidence_payload(evidence)
    evidence["signature_payload_sha256"] = hashlib.sha256(payload).hexdigest()
    evidence["signature"] = openssl_sign_ed25519(signer["private_key"], payload)  # type: ignore[arg-type]
    return evidence


def trusted_signers_for(signer: dict[str, Path | str]) -> dict[str, Path]:
    trusted, errors = device_lab.load_trusted_signer_public_keys([signer["public_key"]])
    if errors:
        raise AssertionError(errors)
    return trusted


def required_artifact_digests(slot: Path) -> dict[str, str]:
    digests = {}
    required_paths = set(
        device_lab._required_signed_evidence_digest_paths(slot)  # type: ignore[attr-defined]
    )
    stage_manifest = device_lab.KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2
    if (slot / stage_manifest).is_file():
        required_paths.add(stage_manifest)
    for relative in sorted(required_paths):
        digests[relative] = hashlib.sha256((slot / relative).read_bytes()).hexdigest()
    return digests


def refresh_signed_evidence_hash(slot: Path) -> None:
    evidence_path = slot / "evidence" / "signed-evidence.json"
    metadata_path = slot / "slot.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    metadata["signed_evidence_artifact_sha256"] = hashlib.sha256(
        evidence_path.read_bytes()
    ).hexdigest()
    write_json(metadata_path, metadata)
    rewrite_sha256sum(slot)


def refresh_d2d_payment_transcript_hash(
    slot: Path,
    signer: dict[str, Path | str] | None = None,
) -> None:
    metadata_path = slot / "slot.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    transcript_path = slot / metadata["d2d_payment_transcript_path"]
    digest = hashlib.sha256(transcript_path.read_bytes()).hexdigest()
    metadata["d2d_payment_transcript_sha256"] = digest
    write_json(metadata_path, metadata)
    if signer is not None:
        evidence_path = slot / "evidence" / "signed-evidence.json"
        evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
        evidence["d2d_payment_transcript_sha256"] = digest
        evidence["artifact_digests"] = required_artifact_digests(slot)
        write_json(evidence_path, sign_evidence(evidence, signer))
        refresh_signed_evidence_hash(slot)
    else:
        rewrite_sha256sum(slot)


def refresh_wallet_integrity_transcript_hash(
    slot: Path,
    signer: dict[str, Path | str] | None = None,
) -> None:
    metadata_path = slot / "slot.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    transcript_path = slot / metadata["wallet_integrity_transcript_path"]
    digest = hashlib.sha256(transcript_path.read_bytes()).hexdigest()
    metadata["wallet_integrity_transcript_sha256"] = digest
    write_json(metadata_path, metadata)
    if signer is not None:
        evidence_path = slot / "evidence" / "signed-evidence.json"
        evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
        evidence["wallet_integrity_transcript_sha256"] = digest
        evidence["artifact_digests"] = required_artifact_digests(slot)
        write_json(evidence_path, sign_evidence(evidence, signer))
        refresh_signed_evidence_hash(slot)
    else:
        rewrite_sha256sum(slot)


def refresh_attestation_certificate_chain_hash(
    slot: Path,
    signer: dict[str, Path | str],
) -> None:
    metadata_path = slot / "slot.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    chain_path = slot / metadata["attestation_certificate_chain_path"]
    digest = hashlib.sha256(chain_path.read_bytes()).hexdigest()
    metadata["attestation_certificate_chain_sha256"] = digest
    write_json(metadata_path, metadata)

    attestation_path = slot / "attestation" / "result.json"
    attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
    attestation["attestation_certificate_chain_path"] = metadata[
        "attestation_certificate_chain_path"
    ]
    attestation["attestation_certificate_chain_sha256"] = digest
    write_json(attestation_path, attestation)

    evidence_path = slot / "evidence" / "signed-evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    evidence["attestation_certificate_chain_path"] = metadata[
        "attestation_certificate_chain_path"
    ]
    evidence["attestation_certificate_chain_sha256"] = digest
    evidence["artifact_digests"] = required_artifact_digests(slot)
    write_json(evidence_path, sign_evidence(evidence, signer))
    refresh_signed_evidence_hash(slot)


def mutate_signed_evidence(slot: Path, mutator) -> None:
    evidence_path = slot / "evidence" / "signed-evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    mutator(evidence)
    write_json(evidence_path, evidence)
    refresh_signed_evidence_hash(slot)


def resign_signed_evidence_artifacts(slot: Path, signer: dict[str, Path | str]) -> None:
    evidence_path = slot / "evidence" / "signed-evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    evidence["artifact_digests"] = required_artifact_digests(slot)
    write_json(evidence_path, sign_evidence(evidence, signer))
    refresh_signed_evidence_hash(slot)


def write_d2d_payment_transcript(
    slot: Path,
    name: str,
    family: str,
    *,
    transcript_path: str = "handoff/d2d-payment.json",
    transport: str = "nfc_hce",
    device_fingerprint: str,
    os_build_id: str,
    app_package_name: str,
    app_signing_certificate_sha256: str,
    attestation_challenge_sha256: str,
    kagemusha_wallet_policy_sha256: str,
    kagemusha_wallet_apk_sha256: str,
) -> tuple[str, str]:
    queue_after_sha256 = hashlib.sha256(
        (slot / "queue" / "pending_queue.json").read_bytes()
    ).hexdigest()
    queue_before_sha256 = hashlib.sha256(
        f"{name}:queue-before-d2d-payment".encode("utf-8")
    ).hexdigest()
    payload_sha256 = hashlib.sha256(
        f"{name}:recursive-spend-d2d-payload".encode("utf-8")
    ).hexdigest()
    payer_before = hashlib.sha256(f"{name}:payer-wallet-before".encode("utf-8")).hexdigest()
    payer_after = hashlib.sha256(f"{name}:payer-wallet-after".encode("utf-8")).hexdigest()
    payee_before = hashlib.sha256(f"{name}:payee-wallet-before".encode("utf-8")).hexdigest()
    payee_after = hashlib.sha256(f"{name}:payee-wallet-after".encode("utf-8")).hexdigest()
    write_json(
        slot / transcript_path,
        {
            "schema": device_lab.D2D_PAYMENT_TRANSCRIPT_SCHEMA,
            "slot_id": name,
            "device_family": family,
            "device_fingerprint": device_fingerprint,
            "os_build_id": os_build_id,
            "app_package_name": app_package_name,
            "app_signing_certificate_sha256": app_signing_certificate_sha256,
            "attestation_challenge_sha256": attestation_challenge_sha256,
            "kagemusha_wallet_policy_sha256": kagemusha_wallet_policy_sha256,
            "kagemusha_wallet_apk_sha256": kagemusha_wallet_apk_sha256,
            "transport": transport,
            "transport_offline": True,
            "payer_wallet_offline": True,
            "payee_wallet_offline": True,
            "payload_schema": device_lab.D2D_PAYMENT_PAYLOAD_SCHEMA,
            "payload_bytes": 3847,
            "transport_session_id_sha256": hashlib.sha256(
                f"{name}:offline-handoff-session".encode("utf-8")
            ).hexdigest(),
            "payload_sha256": payload_sha256,
            "received_payload_sha256": payload_sha256,
            "receiver_ack_sha256": hashlib.sha256(
                f"{name}:receiver-ack".encode("utf-8")
            ).hexdigest(),
            "one_use_key_id_sha256": hashlib.sha256(
                f"{name}:one-use-key".encode("utf-8")
            ).hexdigest(),
            "payer_wallet_state_before_sha256": payer_before,
            "payer_wallet_state_after_sha256": payer_after,
            "payee_wallet_state_before_sha256": payee_before,
            "payee_wallet_state_after_sha256": payee_after,
            "queue_before_sha256": queue_before_sha256,
            "queue_after_sha256": queue_after_sha256,
            "one_use_key_consumed": True,
            "receiver_redeem_accepted": True,
            "double_spend_rejected": True,
        },
    )
    digest = hashlib.sha256((slot / transcript_path).read_bytes()).hexdigest()
    return transcript_path, digest


def write_wallet_integrity_transcript(
    slot: Path,
    name: str,
    family: str,
    *,
    device_fingerprint: str,
    os_build_id: str,
    app_package_name: str,
    app_signing_certificate_sha256: str,
    attestation_challenge_sha256: str,
    attestation_certificate_chain_sha256: str,
    kagemusha_wallet_policy_sha256: str,
    kagemusha_wallet_apk_sha256: str,
) -> tuple[str, str]:
    transcript_path = "wallet/integrity.json"
    key_before = hashlib.sha256(f"{name}:key-before".encode("utf-8")).hexdigest()
    key_after = hashlib.sha256(f"{name}:key-after".encode("utf-8")).hexdigest()
    wallet_before = hashlib.sha256(f"{name}:wallet-before".encode("utf-8")).hexdigest()
    wallet_after = hashlib.sha256(f"{name}:wallet-after-rotation".encode("utf-8")).hexdigest()
    rollback_snapshot = hashlib.sha256(
        f"{name}:rollback-snapshot".encode("utf-8")
    ).hexdigest()
    write_json(
        slot / transcript_path,
        {
            "schema": device_lab.WALLET_INTEGRITY_TRANSCRIPT_SCHEMA,
            "slot_id": name,
            "device_family": family,
            "device_fingerprint": device_fingerprint,
            "os_build_id": os_build_id,
            "app_package_name": app_package_name,
            "keymint_security_level": "STRONGBOX",
            "app_signing_certificate_sha256": app_signing_certificate_sha256,
            "attestation_challenge_sha256": attestation_challenge_sha256,
            "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
            "kagemusha_wallet_policy_sha256": kagemusha_wallet_policy_sha256,
            "kagemusha_wallet_apk_sha256": kagemusha_wallet_apk_sha256,
            "rotation_session_id_sha256": hashlib.sha256(
                f"{name}:rotation-session".encode("utf-8")
            ).hexdigest(),
            "key_id_before_sha256": key_before,
            "key_id_after_sha256": key_after,
            "wallet_state_before_sha256": wallet_before,
            "wallet_state_after_rotation_sha256": wallet_after,
            "rollback_snapshot_sha256": rollback_snapshot,
            "restored_snapshot_sha256": rollback_snapshot,
            "one_use_key_rotation_passed": True,
            "old_key_invalidated": True,
            "rollback_rejection_passed": True,
            "stale_snapshot_rejected": True,
            "active_wallet_state_preserved_after_reject": True,
        },
    )
    digest = hashlib.sha256((slot / transcript_path).read_bytes()).hexdigest()
    return transcript_path, digest


DEVICE_IDENTITY_BY_FAMILY: dict[str, tuple[str, str]] = {
    "Google Pixel 6 / 6a": ("Pixel 6", "oriole"),
    "Google Pixel 7 / 7 Pro": ("Pixel 7", "panther"),
    "Google Pixel 8 / 8a / 8 Pro": ("Pixel 8", "shiba"),
    "Google Pixel Fold / Tablet": ("Pixel Fold", "felix"),
    "Samsung Galaxy S23": ("SM-S911B", "dm1q"),
    "Samsung Galaxy S24": ("SM-S921B", "e1q"),
}


def device_identity_for_family(family: str) -> tuple[str, str]:
    identity = DEVICE_IDENTITY_BY_FAMILY.get(family)
    if identity is None:
        raise AssertionError(f"missing test device identity for {family}")
    return identity


def write_candidate_binding_v2(
    slot: Path,
    slot_id: str,
    *,
    app_signing_certificate_sha256: str | None = None,
    attestation_certificate_chain_sha256: str | None = None,
) -> tuple[dict[str, object], bytes]:
    """Write a byte-consistent candidate lab fixture for validator tests."""

    candidate_record_path = "evidence/candidate/candidate-v4.norito"
    candidate_manifest_path = "evidence/candidate/manifest-v4.norito"
    candidate_validation_path = device_lab.KAGEMUSHA_CANDIDATE_VALIDATION_REPORT_PATH_V2
    native_library_path = (
        "evidence/candidate/lib/arm64-v8a/libconnect_norito_bridge.so"
    )
    source_commit = "1" * 40
    source_tree_sha256 = hashlib.sha256(
        f"{slot_id}:source-tree".encode("utf-8")
    ).hexdigest()
    generation = f"candidate-{slot_id}"
    write_text(slot / candidate_record_path, f"{slot_id}:candidate-v4\n")
    write_text(slot / candidate_manifest_path, f"{slot_id}:manifest-v4\n")
    qualification_receipt_path = (
        "evidence/candidate/"
        f"{device_lab.KAGEMUSHA_QUALIFICATION_RECEIPT_FILE_NAME_V4}"
    )
    write_text(slot / qualification_receipt_path, f"{slot_id}:qualification-receipt\n")
    candidate_record_sha256 = hashlib.sha256(
        (slot / candidate_record_path).read_bytes()
    ).hexdigest()
    candidate_manifest_sha256 = hashlib.sha256(
        (slot / candidate_manifest_path).read_bytes()
    ).hexdigest()
    inventory: list[dict[str, object]] = []
    measured_inventory: list[dict[str, object]] = []
    for role, file_name in zip(
        device_lab.KAGEMUSHA_CANDIDATE_ARTIFACT_ROLES_V4,
        device_lab.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4,
    ):
        relative = f"evidence/candidate/artifacts/{file_name}"
        header = f"{slot_id}:{role}:header".encode("utf-8")
        payload = f"{slot_id}:{role}:payload".encode("utf-8")
        framed = b"KRV4KEY\0" + len(header).to_bytes(4, "little") + header + payload
        path = slot / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(framed)
        path.chmod(0o600)
        measurement: dict[str, object] = {
            "role": role,
            "framed_size_bytes": len(framed),
            "framed_sha256": hashlib.sha256(framed).hexdigest(),
            "payload_size_bytes": len(payload),
            "payload_sha256": hashlib.sha256(payload).hexdigest(),
        }
        measured_inventory.append(measurement)
        inventory.append({"path": relative, **measurement})
    inventory_sha256 = device_lab._candidate_inventory_sha256(measured_inventory)

    qualification_receipt_sha256 = hashlib.sha256(
        (slot / qualification_receipt_path).read_bytes()
    ).hexdigest()
    qualified_candidate_sha256 = (
        device_lab.derive_kagemusha_qualified_candidate_sha256_v4(
            candidate_record_sha256,
            qualification_receipt_sha256,
        )
    )
    report_artifacts = [
        {
            "role": role,
            "file_name": file_name,
            **measurement,
        }
        for role, file_name, measurement in zip(
            device_lab.KAGEMUSHA_CANDIDATE_ARTIFACT_ROLES_V4,
            device_lab.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4,
            measured_inventory,
        )
    ]
    write_json(
        slot / candidate_validation_path,
        {
            "schema": device_lab.KAGEMUSHA_CANDIDATE_VALIDATION_REPORT_SCHEMA_V2,
            "candidate_record_sha256": candidate_record_sha256,
            "candidate_manifest_sha256": candidate_manifest_sha256,
            "qualification_receipt_file_name": (
                device_lab.KAGEMUSHA_QUALIFICATION_RECEIPT_FILE_NAME_V4
            ),
            "qualification_receipt_sha256": qualification_receipt_sha256,
            "qualified_candidate_sha256": qualified_candidate_sha256,
            "source_commit": source_commit,
            "source_tree_sha256": source_tree_sha256,
            "source_repo_dirty": False,
            "reviewed_source_closure_descriptor_sha256": "6" * 64,
            "authenticated_source_seal_projection_sha256": "7" * 64,
            "reviewed_cargo_binary_sha256": "8" * 64,
            "reviewed_rustc_binary_sha256": "9" * 64,
            "generation": generation,
            "generation_memory_limit_bytes": 6 * 1024 * 1024 * 1024,
            "generation_memory_enforcement_profile": (
                device_lab.KAGEMUSHA_GENERATION_MEMORY_ENFORCEMENT_PROFILE_V1
            ),
            "bridge_abi_version": 23,
            "artifact_count": len(report_artifacts),
            "artifacts": report_artifacts,
            "topup_finality_roster_file_name": "topup-finality-roster-v4.norito",
            "topup_finality_roster_size_bytes": 1,
            "topup_finality_roster_sha256": hashlib.sha256(b"r").hexdigest(),
        },
    )

    for file_name in device_lab.KAGEMUSHA_CANDIDATE_SCENARIO_FILES_V1:
        write_text(slot / "scenario" / file_name, f"{slot_id}:{file_name}\n")

    stage_paths = sorted(
        {
            candidate_record_path,
            candidate_manifest_path,
            candidate_validation_path,
            qualification_receipt_path,
            *(
                f"evidence/candidate/artifacts/{name}"
                for name in device_lab.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4
            ),
            *(
                f"scenario/{name}"
                for name in device_lab.KAGEMUSHA_CANDIDATE_SCENARIO_FILES_V1
            ),
        },
        key=lambda value: value.encode("utf-8"),
    )
    stage_entries: list[dict[str, object]] = []
    for relative in stage_paths:
        payload = (slot / relative).read_bytes()
        stage_entries.append(
            {
                "path": relative,
                "mode": "0600",
                "size_bytes": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest(),
            }
        )
    scenario_entries = [
        entry for entry in stage_entries if str(entry["path"]).startswith("scenario/")
    ]
    scenario_digest = hashlib.sha256()
    scenario_digest.update(device_lab.KAGEMUSHA_CANDIDATE_SCENARIO_INVENTORY_DOMAIN_V1)
    scenario_digest.update(len(scenario_entries).to_bytes(4, "big"))
    for entry in scenario_entries:
        path_bytes = str(entry["path"]).encode("utf-8")
        scenario_digest.update(len(path_bytes).to_bytes(4, "big"))
        scenario_digest.update(path_bytes)
        scenario_digest.update(int(entry["size_bytes"]).to_bytes(8, "big"))
        scenario_digest.update(bytes.fromhex(str(entry["sha256"])))
    stage_manifest: dict[str, object] = {
        "schema": device_lab.KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_SCHEMA_V2,
        "version": 2,
        "stage_manifest_path": device_lab.KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2,
        "stage_manifest_mode": "0600",
        "stage_manifest_size_bytes": 0,
        "candidate_record_sha256": candidate_record_sha256,
        "candidate_manifest_sha256": candidate_manifest_sha256,
        "candidate_validation_report_sha256": hashlib.sha256(
            (slot / candidate_validation_path).read_bytes()
        ).hexdigest(),
        "qualification_receipt_sha256": qualification_receipt_sha256,
        "qualified_candidate_sha256": qualified_candidate_sha256,
        "scenario_inventory_sha256": scenario_digest.hexdigest(),
        "source_commit": source_commit,
        "source_tree_sha256": source_tree_sha256,
        "source_repo_dirty": False,
        "validator": {
            "schema": device_lab.KAGEMUSHA_CANDIDATE_STAGE_VALIDATOR_SCHEMA_V1,
            "candidate_binary_name": "kagemusha_recursive_spend_v4_bundle",
            "candidate_binary_sha256": "2" * 64,
            "scenario_binary_name": "kagemusha_candidate_scenario_validator",
            "scenario_binary_sha256": "3" * 64,
            "cargo_binary_sha256": "4" * 64,
            "cargo_version_verbose": "cargo test fixture\n",
            "rustc_binary_sha256": "5" * 64,
            "rustc_version_verbose": "rustc test fixture\n",
            "locked": True,
            "offline": True,
            "isolated_target": True,
            "build_jobs": 2,
            "candidate_package": "iroha_core",
            "scenario_package": "connect_norito_bridge",
            "features": ["kagemusha-candidate-evidence-lab"],
            "profile": "debug",
        },
        "entry_count": len(stage_entries),
        "scenario_entry_count": len(scenario_entries),
        "entries": stage_entries,
    }
    while True:
        encoded_manifest = (
            json.dumps(
                stage_manifest,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=True,
            )
            + "\n"
        ).encode("utf-8")
        if stage_manifest["stage_manifest_size_bytes"] == len(encoded_manifest):
            break
        stage_manifest["stage_manifest_size_bytes"] = len(encoded_manifest)
    stage_manifest_path = slot / device_lab.KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2
    stage_manifest_path.write_bytes(encoded_manifest)
    stage_manifest_path.chmod(0o600)
    stage_manifest_sha256 = hashlib.sha256(encoded_manifest).hexdigest()

    write_text(slot / native_library_path, f"{slot_id}:candidate-lab-native\n")
    native_library_sha256 = hashlib.sha256(
        (slot / native_library_path).read_bytes()
    ).hexdigest()
    main_apk, test_apk, lab_signing_certificate_sha256 = (
        _android_apk_fixtures.signed_candidate_apk_fixture(device_lab)
    )
    lab_apk_path = (
        "evidence/kagemusha-candidate-evidence-lab-DO-NOT-SHIP-"
        f"{candidate_record_sha256}-debug.apk"
    )
    lab_test_apk_path = (
        "evidence/kagemusha-candidate-evidence-lab-DO-NOT-SHIP-"
        f"{candidate_record_sha256}-debug-androidTest.apk"
    )
    for relative, payload in (
        (lab_apk_path, main_apk),
        (lab_test_apk_path, test_apk),
    ):
        path = slot / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        path.chmod(0o600)
    lab_apk_sha256 = hashlib.sha256(main_apk).hexdigest()
    lab_test_apk_sha256 = hashlib.sha256(test_apk).hexdigest()

    wallet_certificate = app_signing_certificate_sha256 or hashlib.sha256(
        f"{slot_id}:wallet-signer".encode()
    ).hexdigest()
    chain_sha256 = attestation_certificate_chain_sha256 or hashlib.sha256(
        f"{slot_id}:attestation-chain".encode()
    ).hexdigest()
    challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(
        {
            "slot_id": slot_id,
            "candidate_record_sha256": candidate_record_sha256,
            "candidate_manifest_sha256": candidate_manifest_sha256,
            "candidate_stage_manifest_sha256": stage_manifest_sha256,
            "candidate_lab_native_library_sha256": native_library_sha256,
            "candidate_lab_apk_sha256": lab_apk_sha256,
            "candidate_lab_test_apk_sha256": lab_test_apk_sha256,
            "candidate_source_commit": source_commit,
            "candidate_source_tree_sha256": source_tree_sha256,
        }
    )
    if _android_x509_fixtures.authority_is_configured():
        chain_payload = test_android_attestation_chain(
            challenge,
            device_lab.KAGEMUSHA_WALLET_PACKAGE_NAME,
            bytes.fromhex(wallet_certificate),
        )
        chain_path = slot / "attestation" / "keymint-certificate-chain.pem"
        chain_path.parent.mkdir(parents=True, exist_ok=True)
        chain_path.write_bytes(chain_payload)
        chain_path.chmod(0o600)
        chain_sha256 = hashlib.sha256(chain_payload).hexdigest()
    challenge_sha256 = hashlib.sha256(challenge).hexdigest()

    input_counts = (
        0, 5, 1, 8, 2, 8, 2, 0, 1, 1, 1, 4, 4, 4, 4, 4,
        3, 1, 3, 1, 7, 3, 8, 1, 8, 1, 8, 1,
    )
    causal_events: list[dict[str, object]] = []
    for sequence, (operation, input_count) in enumerate(
        zip(device_lab.KAGEMUSHA_CANDIDATE_CAUSAL_OPERATIONS_V1, input_counts)
    ):
        inputs = [
            hashlib.sha256(f"{slot_id}:{operation}:input:{index}".encode()).hexdigest()
            for index in range(input_count)
        ]
        event: dict[str, object] = {
            "sequence": sequence,
            "phase": "phase_1" if sequence < 7 else "phase_2",
            "operation": operation,
            "outcome": "succeeded",
            "duration_nanos": sequence + 1,
            "input_sha256": inputs,
            "output_sha256": hashlib.sha256(
                f"{slot_id}:{operation}:output".encode()
            ).hexdigest(),
            "output_size_bytes": 32,
            "rejection_classification": None,
            "exception_class": None,
            "error_message_sha256": None,
        }
        if operation in {
            "candidate_install",
            "candidate_reinstall_after_process_restart",
            "restore_init_result_after_restart",
            "restore_hop_01_result_after_restart",
            "restore_hop_02_result_after_restart",
        }:
            event["output_sha256"] = None
            event["output_size_bytes"] = 0
        if operation == "duplicate_input_rejection":
            event.update(
                {
                    "outcome": "rejected",
                    "output_sha256": None,
                    "output_size_bytes": 0,
                    "rejection_classification": "duplicate_input_bundle",
                    "exception_class": "java.lang.IllegalArgumentException",
                    "error_message_sha256": hashlib.sha256(
                        f"{slot_id}:duplicate-rejected".encode()
                    ).hexdigest(),
                }
            )
        causal_events.append(event)

    def link_output(output_event: int, input_event: int, input_index: int) -> None:
        digest = causal_events[output_event]["output_sha256"]
        assert isinstance(digest, str)
        inputs = causal_events[input_event]["input_sha256"]
        assert isinstance(inputs, list)
        inputs[input_index] = digest

    def link_input(
        source_event: int,
        source_input: int,
        target_event: int,
        target_input: int,
    ) -> None:
        source = causal_events[source_event]["input_sha256"]
        target = causal_events[target_event]["input_sha256"]
        assert isinstance(source, list) and isinstance(target, list)
        target[target_input] = source[source_input]

    for output_event, input_event, input_index in (
        (1, 2, 0),
        (3, 4, 0),
        (5, 6, 0),
        (2, 8, 0),
        (4, 9, 0),
        (6, 10, 0),
        (16, 17, 0),
        (18, 19, 0),
        (20, 21, 0),
        (22, 23, 0),
        (24, 25, 0),
        (26, 27, 0),
    ):
        link_output(output_event, input_event, input_index)
    for source_event, source_input, target_event, target_input in (
        (3, 0, 11, 0), (3, 1, 11, 1), (3, 3, 11, 2), (3, 2, 11, 3),
        (5, 0, 12, 0), (5, 1, 12, 1), (5, 3, 12, 2), (5, 2, 12, 3),
        (13, 0, 16, 0), (13, 1, 16, 2), (4, 1, 16, 1),
        (14, 0, 18, 0), (14, 1, 18, 2), (6, 1, 18, 1),
        (13, 0, 20, 0), (13, 1, 20, 1), (13, 3, 20, 2), (13, 2, 20, 3),
        (13, 0, 21, 2),
        (13, 0, 22, 0), (13, 1, 22, 1), (13, 3, 22, 2), (13, 2, 22, 3),
        (14, 0, 24, 0), (14, 1, 24, 1), (14, 3, 24, 2), (14, 2, 24, 3),
        (15, 0, 26, 0), (15, 1, 26, 1), (15, 3, 26, 2), (15, 2, 26, 3),
        (22, 4, 24, 4), (22, 4, 26, 4), (22, 6, 24, 6), (22, 6, 26, 6),
    ):
        link_input(source_event, source_input, target_event, target_input)

    lifecycle_path = device_lab.KAGEMUSHA_CANDIDATE_LIFECYCLE_TRANSCRIPT_PATH
    write_json(
        slot / lifecycle_path,
        {
            "schema": device_lab.KAGEMUSHA_CANDIDATE_LIFECYCLE_SCHEMA_V2,
            "slot_id": slot_id,
            "candidate_record_sha256": candidate_record_sha256,
            "candidate_manifest_sha256": candidate_manifest_sha256,
            "candidate_stage_manifest_path": (
                device_lab.KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2
            ),
            "candidate_stage_manifest_sha256": stage_manifest_sha256,
            "candidate_inventory_sha256": inventory_sha256,
            "source_commit": source_commit,
            "source_tree_sha256": source_tree_sha256,
            "source_repo_dirty": False,
            "generation": generation,
            "bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
            "production_capability_observed": False,
            "initial_atomic": "1075",
            "first_recipient_atomic": "625",
            "second_recipient_atomic": "210",
            "sender_change_atomic": "240",
            "redeemed_atomic": "1075",
            "final_unspent_atomic": "0",
            "proof_hops": 2,
            "init_proof_verified": True,
            "first_spend_verified": True,
            "multi_hop_proof_verified": True,
            "independent_branch_redemption_verified": True,
            "duplicate_rejected": True,
            "restart_recovered": True,
            "network_requests_during_peer_transfers": 0,
            "attestation_challenge_sha256": challenge_sha256,
            "attestation_certificate_chain_sha256": chain_sha256,
            "app_signing_certificate_sha256": wallet_certificate,
            "strongbox_attestation": True,
            "physical_device_attestation": True,
            "causal_events": causal_events,
        },
    )
    lifecycle_sha256 = hashlib.sha256((slot / lifecycle_path).read_bytes()).hexdigest()
    binding_path = device_lab.KAGEMUSHA_CANDIDATE_BINDING_ARTIFACT_PATH
    write_json(
        slot / binding_path,
        {
            "schema": device_lab.KAGEMUSHA_CANDIDATE_BINDING_SCHEMA_V2,
            "candidate_record_path": candidate_record_path,
            "candidate_record_sha256": candidate_record_sha256,
            "candidate_manifest_path": candidate_manifest_path,
            "candidate_manifest_sha256": candidate_manifest_sha256,
            "candidate_stage_manifest_path": (
                device_lab.KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2
            ),
            "candidate_stage_manifest_sha256": stage_manifest_sha256,
            "source_commit": source_commit,
            "source_tree_sha256": source_tree_sha256,
            "source_repo_dirty": False,
            "generation": generation,
            "bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
            "lab_native_library_path": native_library_path,
            "lab_native_library_sha256": native_library_sha256,
            "lab_apk_path": lab_apk_path,
            "lab_apk_sha256": lab_apk_sha256,
            "lab_apk_signing_cert_sha256": lab_signing_certificate_sha256,
            "lab_test_apk_path": lab_test_apk_path,
            "lab_test_apk_sha256": lab_test_apk_sha256,
            "lab_test_apk_signing_cert_sha256": lab_signing_certificate_sha256,
            "production_capability_observed": False,
            "native_accepted_candidate_record_sha256": candidate_record_sha256,
            "native_accepted_candidate_manifest_sha256": candidate_manifest_sha256,
            "native_accepted_source_commit": source_commit,
            "native_accepted_source_tree_sha256": source_tree_sha256,
            "native_accepted_source_repo_dirty": False,
            "native_accepted_generation": generation,
            "native_accepted_bridge_abi_version": (
                device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION
            ),
            "native_accepted_inventory_sha256": inventory_sha256,
            "lifecycle_transcript_path": lifecycle_path,
            "lifecycle_transcript_sha256": lifecycle_sha256,
            "artifact_inventory": inventory,
        },
    )
    binding_sha256 = hashlib.sha256((slot / binding_path).read_bytes()).hexdigest()
    return {
        "candidate_binding_path": binding_path,
        "candidate_binding_sha256": binding_sha256,
        "candidate_record_path": candidate_record_path,
        "candidate_record_sha256": candidate_record_sha256,
        "candidate_manifest_path": candidate_manifest_path,
        "candidate_manifest_sha256": candidate_manifest_sha256,
        "candidate_stage_manifest_path": (
            device_lab.KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2
        ),
        "candidate_stage_manifest_sha256": stage_manifest_sha256,
        "candidate_source_commit": source_commit,
        "candidate_source_tree_sha256": source_tree_sha256,
        "candidate_source_tree_sha256_before": source_tree_sha256,
        "candidate_source_tree_sha256_after": source_tree_sha256,
        "candidate_source_repo_dirty": False,
        "candidate_generation": generation,
        "candidate_lab_native_library_path": native_library_path,
        "candidate_lab_native_library_sha256": native_library_sha256,
        "candidate_lab_apk_path": lab_apk_path,
        "candidate_lab_apk_sha256": lab_apk_sha256,
        "candidate_lab_apk_signing_certificate_sha256": (
            lab_signing_certificate_sha256
        ),
        "candidate_lab_test_apk_path": lab_test_apk_path,
        "candidate_lab_test_apk_sha256": lab_test_apk_sha256,
        "candidate_lab_test_apk_signing_certificate_sha256": (
            lab_signing_certificate_sha256
        ),
        "candidate_lifecycle_transcript_path": lifecycle_path,
        "candidate_lifecycle_transcript_sha256": lifecycle_sha256,
        "candidate_inventory_sha256": inventory_sha256,
        "production_capability_observed": False,
    }, challenge


def summary_release_report(
    slot="slot-0",
    family=None,
    *,
    device_fingerprint_sha256=None,
    attestation_challenge_sha256=None,
    d2d_payment_transport="nfc_hce",
    signed_at_utc="2026-06-06T00:00:00Z",
    **kagemusha_overrides,
):
    family = family or device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]
    device_model, device_codename = device_identity_for_family(family)
    report = {
        "slot": slot,
        "status": "ok",
        "errors": [],
        "kagemusha": {
            "required": True,
            "device_family": family,
            "device_model": device_model,
            "device_codename": device_codename,
            "native_bridge_abi_version": (
                device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION
            ),
            "signed_at_utc": signed_at_utc,
            "signed_evidence_artifact_sha256": "3" * 64,
            "signed_evidence_signer_public_key_sha256": "4" * 64,
            "device_fingerprint_sha256": device_fingerprint_sha256 or "a" * 64,
            "attestation_challenge_sha256": (
                attestation_challenge_sha256 or "b" * 64
            ),
            "attestation_certificate_chain_path": (
                "attestation/keymint-certificate-chain.pem"
            ),
            "attestation_certificate_chain_sha256": "5" * 64,
            "app_signing_certificate_sha256": "2" * 64,
            "kagemusha_wallet_apk_path": "evidence/kagemusha-wallet-release.apk",
            "kagemusha_wallet_apk_sha256": "6" * 64,
            "d2d_payment_transcript_path": "handoff/d2d-payment.json",
            "d2d_payment_transcript_sha256": "7" * 64,
            "d2d_payment_transport": d2d_payment_transport,
            "wallet_integrity_transcript_path": "wallet/integrity.json",
            "wallet_integrity_transcript_sha256": "8" * 64,
            "candidate_binding_path": "evidence/candidate-binding-v2.json",
            "candidate_binding_sha256": "9" * 64,
            "candidate_record_path": "evidence/candidate/candidate-v4.norito",
            "candidate_record_sha256": "a" * 64,
            "candidate_manifest_path": "evidence/candidate/manifest-v4.norito",
            "candidate_manifest_sha256": "b" * 64,
            "candidate_stage_manifest_path": (
                device_lab.KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2
            ),
            "candidate_stage_manifest_sha256": "c" * 64,
            "candidate_source_commit": "c" * 40,
            "candidate_source_tree_sha256": "c" * 64,
            "candidate_source_tree_sha256_before": "c" * 64,
            "candidate_source_tree_sha256_after": "c" * 64,
            "candidate_generation": "candidate-summary-v2",
            "candidate_lab_native_library_path": (
                "evidence/candidate/lib/arm64-v8a/libconnect_norito_bridge.so"
            ),
            "candidate_lab_native_library_sha256": "d" * 64,
            "candidate_lab_apk_path": (
                "evidence/kagemusha-candidate-evidence-lab-DO-NOT-SHIP-"
                "summary-debug.apk"
            ),
            "candidate_lab_apk_sha256": "1" * 64,
            "candidate_lab_apk_signing_certificate_sha256": "3" * 64,
            "candidate_lab_test_apk_path": (
                "evidence/kagemusha-candidate-evidence-lab-DO-NOT-SHIP-"
                "summary-debug-androidTest.apk"
            ),
            "candidate_lab_test_apk_sha256": "2" * 64,
            "candidate_lab_test_apk_signing_certificate_sha256": "3" * 64,
            "candidate_lifecycle_transcript_path": (
                "evidence/lifecycle-transcript-v2.json"
            ),
            "candidate_lifecycle_transcript_sha256": "e" * 64,
            "candidate_inventory_sha256": "f" * 64,
            "production_capability_observed": False,
            "candidate_source_repo_dirty": False,
            "strongbox_attestation": True,
            "physical_device_attestation": True,
        },
    }
    report["kagemusha"].update(kagemusha_overrides)
    return report


def summary_d2d_transcript_bindings(
    transports: tuple[str, ...],
    *,
    primary_transport: str = "nfc_hce",
    primary_path: str = "handoff/d2d-payment.json",
    primary_sha256: str = "7" * 64,
) -> dict[str, dict[str, str]]:
    bindings: dict[str, dict[str, str]] = {}
    for transport in transports:
        if transport == primary_transport:
            path = primary_path
            digest = primary_sha256
        else:
            path = f"handoff/d2d-payment-{transport}.json"
            digest = hashlib.sha256(
                f"kagemusha-summary-d2d-{transport}".encode("utf-8")
            ).hexdigest()
        bindings[transport] = {"path": path, "sha256": digest}
    return bindings


def create_slot(
    root: Path,
    name: str,
    family: str | None = None,
    signer: dict[str, Path | str] | None = None,
    d2d_payment_transport: str = "nfc_hce",
    d2d_payment_transports: tuple[str, ...] | None = None,
) -> Path:
    slot = root / name
    slot_family = family or device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]
    device_model, device_codename = device_identity_for_family(slot_family)
    device_fingerprint = f"{name}/fingerprint"
    os_build_id = f"{name}-build"
    app_package_name = "org.hyperledger.iroha.kagemushawallet"
    wallet_apk_payload, app_signing_certificate_sha256 = (
        _android_apk_fixtures.signed_wallet_apk_fixture(device_lab)
    )
    attestation_certificate_chain_path = "attestation/keymint-certificate-chain.pem"
    write_text(
        slot / attestation_certificate_chain_path,
        "-----BEGIN CERTIFICATE-----\n"
        f"{name}-strongbox-keymint-certificate-leaf\n"
        "-----END CERTIFICATE-----\n"
        "-----BEGIN CERTIFICATE-----\n"
        f"{name}-strongbox-keymint-certificate-issuer\n"
        "-----END CERTIFICATE-----\n",
    )
    attestation_certificate_chain_sha256 = hashlib.sha256(
        (slot / attestation_certificate_chain_path).read_bytes()
    ).hexdigest()
    candidate_metadata: dict[str, object] = {}
    if family is not None:
        candidate_metadata, attestation_challenge = write_candidate_binding_v2(
            slot,
            name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_certificate_chain_sha256=(
                attestation_certificate_chain_sha256
            ),
        )
        attestation_certificate_chain_sha256 = hashlib.sha256(
            (slot / attestation_certificate_chain_path).read_bytes()
        ).hexdigest()
    else:
        attestation_challenge = f"{name}:attestation-challenge".encode("utf-8")
    attestation_challenge_sha256 = hashlib.sha256(attestation_challenge).hexdigest()
    kagemusha_wallet_policy_sha256 = hashlib.sha256(
        b"kagemusha-wallet-policy-v1"
    ).hexdigest()
    kagemusha_wallet_apk_path = "evidence/kagemusha-wallet-release.apk"
    wallet_apk_file = slot / kagemusha_wallet_apk_path
    wallet_apk_file.parent.mkdir(parents=True, exist_ok=True)
    wallet_apk_file.write_bytes(wallet_apk_payload)
    wallet_apk_file.chmod(0o600)
    kagemusha_wallet_apk_sha256 = hashlib.sha256(
        (slot / kagemusha_wallet_apk_path).read_bytes()
    ).hexdigest()
    raw_test_commands = list(KAGEMUSHA_ANDROID_RAW_TEST_COMMANDS)
    write_json(
        slot / "telemetry" / "telemetry.json",
        {
            "schema_version": 1,
            "slot_id": name,
            "suite": "kagemusha-device-lab",
            "device_model": device_model,
            "device_codename": device_codename,
            "app_package_name": app_package_name,
        },
    )
    write_text(
        slot / "telemetry" / "status.ndjson",
        f'{{"status":"ok","slot_id":"{name}"}}\n',
    )
    write_json(
        slot / "attestation" / "harness-result.json",
        {
            "alias": "android-keystore-alias",
            "attestation_security_level": "STRONG_BOX",
            "keymaster_security_level": "STRONG_BOX",
            "strongbox_attestation": True,
            "challenge_hex": attestation_challenge.hex(),
            "chain_length": 2,
        },
    )
    write_json(
        slot / "attestation" / "result.json",
        {
            "slot": name,
            "status": "ok",
            "slot_id": name,
            "device_fingerprint": device_fingerprint,
            "os_build_id": os_build_id,
            "app_package_name": app_package_name,
            "app_signing_certificate_sha256": app_signing_certificate_sha256,
            "attestation_challenge_sha256": attestation_challenge_sha256,
            "attestation_certificate_chain_path": attestation_certificate_chain_path,
            "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
            "kagemusha_wallet_policy_sha256": kagemusha_wallet_policy_sha256,
            "attestation_security_level": "STRONGBOX",
            "keymaster_security_level": "STRONGBOX",
            "keymint_security_level": "STRONGBOX",
            "strongbox_attestation": True,
            "physical_device_attestation": True,
        },
    )
    write_json(
        slot / "attestation" / "report.json",
        {
            "schema": device_lab.ATTESTATION_REPORT_SCHEMA,
            "slot_id": name,
            "device_fingerprint": device_fingerprint,
            "os_build_id": os_build_id,
            "app_package_name": app_package_name,
            "attestation_challenge_sha256": attestation_challenge_sha256,
            "attestation_certificate_chain_path": attestation_certificate_chain_path,
            "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
            "verifier": "android-keystore-attestation-harness",
            "verification": {
                "status": "ok",
                "strongbox_attestation": True,
                "physical_device_attestation": True,
                "keymint_security_level": "STRONGBOX",
                "attestation_security_level": "STRONGBOX",
                "keymaster_security_level": "STRONGBOX",
            },
        },
    )
    write_json(slot / "queue" / "pending_queue.json", {"slot_id": name, "pending_transactions": []})
    write_text(slot / "queue" / "pending.queue", "")
    write_text(slot / "logs" / "runtime.log", "kagemusha device-lab run complete\n")
    d2d_payment_transcript_path, d2d_payment_transcript_sha256 = (
        write_d2d_payment_transcript(
            slot,
            name,
            slot_family,
            transport=d2d_payment_transport,
            device_fingerprint=device_fingerprint,
            os_build_id=os_build_id,
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=attestation_challenge_sha256,
            kagemusha_wallet_policy_sha256=kagemusha_wallet_policy_sha256,
            kagemusha_wallet_apk_sha256=kagemusha_wallet_apk_sha256,
        )
    )
    d2d_payment_transcripts = {
        d2d_payment_transport: {
            "path": d2d_payment_transcript_path,
            "sha256": d2d_payment_transcript_sha256,
        }
    }
    for transport in d2d_payment_transports or ():
        if transport == d2d_payment_transport:
            continue
        extra_path, extra_sha256 = write_d2d_payment_transcript(
            slot,
            name,
            slot_family,
            transcript_path=f"handoff/d2d-payment-{transport}.json",
            transport=transport,
            device_fingerprint=device_fingerprint,
            os_build_id=os_build_id,
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=attestation_challenge_sha256,
            kagemusha_wallet_policy_sha256=kagemusha_wallet_policy_sha256,
            kagemusha_wallet_apk_sha256=kagemusha_wallet_apk_sha256,
        )
        d2d_payment_transcripts[transport] = {
            "path": extra_path,
            "sha256": extra_sha256,
        }
    wallet_integrity_transcript_path, wallet_integrity_transcript_sha256 = (
        write_wallet_integrity_transcript(
            slot,
            name,
            slot_family,
            device_fingerprint=device_fingerprint,
            os_build_id=os_build_id,
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=attestation_challenge_sha256,
            attestation_certificate_chain_sha256=attestation_certificate_chain_sha256,
            kagemusha_wallet_policy_sha256=kagemusha_wallet_policy_sha256,
            kagemusha_wallet_apk_sha256=kagemusha_wallet_apk_sha256,
        )
    )

    if family is not None:
        if signer is None:
            raise AssertionError("production test slots require a signer")
        minimum_os = device_lab.KAGEMUSHA_STANDARD_DEVICE_MINIMUM_OS[family]
        evidence_path = slot / "evidence" / "signed-evidence.json"
        write_json(
            evidence_path,
            sign_evidence(
                {
                    "schema": device_lab.SIGNED_EVIDENCE_SCHEMA,
                    "slot_id": name,
                    "device_family": family,
                    "device_model": device_model,
                    "device_codename": device_codename,
                    "device_fingerprint": device_fingerprint,
                    "os_build_id": os_build_id,
                    "minimum_os": minimum_os,
                    "app_package_name": app_package_name,
                    "attestation_certificate_chain_path": attestation_certificate_chain_path,
                    "kagemusha_wallet_apk_path": kagemusha_wallet_apk_path,
                    "d2d_payment_transcript_path": d2d_payment_transcript_path,
                    **(
                        {"d2d_payment_transcripts": d2d_payment_transcripts}
                        if d2d_payment_transports is not None
                        else {}
                    ),
                    "wallet_integrity_transcript_path": wallet_integrity_transcript_path,
                    "app_signing_certificate_sha256": app_signing_certificate_sha256,
                    "attestation_challenge_sha256": attestation_challenge_sha256,
                    "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
                    "kagemusha_wallet_policy_sha256": kagemusha_wallet_policy_sha256,
                    "kagemusha_wallet_apk_sha256": kagemusha_wallet_apk_sha256,
                    "d2d_payment_transcript_sha256": d2d_payment_transcript_sha256,
                    "wallet_integrity_transcript_sha256": wallet_integrity_transcript_sha256,
                    "native_bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
                    "strongbox_attestation": True,
                    "physical_device_attestation": True,
                    "keymint_security_level": "STRONGBOX",
                    "one_use_key_rotation_passed": True,
                    "rollback_rejection_passed": True,
                    "kagemusha_recursive_spend_ffi_surface": "passed",
                    "kagemusha_recursive_spend_jni_probe": "recursive_spend_verified",
                    "kagemusha_recursive_spend_prover_state": "multi_hop_proof_composed",
                    **candidate_metadata,
                    "raw_test_commands": raw_test_commands,
                    "signed_at_utc": "2026-06-06T00:00:00Z",
                    "signer_key_id": "android-lab-release-signer-v1",
                    "signer_public_key_sha256": signer["public_key_sha256"],
                    "signature_algorithm": "ed25519",
                    "artifact_digests": required_artifact_digests(slot),
                },
                signer,
            ),
        )
        evidence_digest = hashlib.sha256(evidence_path.read_bytes()).hexdigest()
        write_json(
            slot / "slot.json",
            {
                "schema": device_lab.KAGEMUSHA_SLOT_SCHEMA_V2,
                "slot_id": name,
                "device_family": family,
                "device_model": device_model,
                "device_codename": device_codename,
                "device_fingerprint": device_fingerprint,
                "os_build_id": os_build_id,
                "minimum_os": minimum_os,
                "app_package_name": app_package_name,
                "attestation_certificate_chain_path": attestation_certificate_chain_path,
                "kagemusha_wallet_apk_path": kagemusha_wallet_apk_path,
                "d2d_payment_transcript_path": d2d_payment_transcript_path,
                **(
                    {"d2d_payment_transcripts": d2d_payment_transcripts}
                    if d2d_payment_transports is not None
                    else {}
                ),
                "wallet_integrity_transcript_path": wallet_integrity_transcript_path,
                "app_signing_certificate_sha256": app_signing_certificate_sha256,
                "attestation_challenge_sha256": attestation_challenge_sha256,
                "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
                "kagemusha_wallet_policy_sha256": kagemusha_wallet_policy_sha256,
                "kagemusha_wallet_apk_sha256": kagemusha_wallet_apk_sha256,
                "d2d_payment_transcript_sha256": d2d_payment_transcript_sha256,
                "wallet_integrity_transcript_sha256": wallet_integrity_transcript_sha256,
                "native_bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
                "strongbox_attestation": True,
                "physical_device_attestation": True,
                "keymint_security_level": "STRONGBOX",
                "one_use_key_rotation_passed": True,
                "rollback_rejection_passed": True,
                "kagemusha_recursive_spend_ffi_surface": "passed",
                "kagemusha_recursive_spend_jni_probe": "recursive_spend_verified",
                "kagemusha_recursive_spend_prover_state": "multi_hop_proof_composed",
                **candidate_metadata,
                "signed_evidence_artifact_path": "evidence/signed-evidence.json",
                "signed_evidence_artifact_sha256": evidence_digest,
                "raw_test_commands": raw_test_commands,
            },
        )

    rewrite_sha256sum(slot)
    return slot


def copy_slot_binding(
    *,
    source: Path,
    target: Path,
    signer: dict[str, Path | str],
    key: str,
) -> None:
    """Copy a signed slot binding across all artifacts for adversarial tests."""

    source_metadata = json.loads((source / "slot.json").read_text(encoding="utf-8"))
    copied = source_metadata[key]

    metadata_path = target / "slot.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    metadata[key] = copied
    write_json(metadata_path, metadata)

    attestation_path = target / "attestation" / "result.json"
    attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
    if key in attestation:
        attestation[key] = copied
        write_json(attestation_path, attestation)

    attestation_report_path = target / "attestation" / "report.json"
    attestation_report = json.loads(attestation_report_path.read_text(encoding="utf-8"))
    if key in attestation_report:
        attestation_report[key] = copied
        write_json(attestation_report_path, attestation_report)

    if key == "attestation_challenge_sha256":
        source_harness_path = source / "attestation" / "harness-result.json"
        source_harness = json.loads(source_harness_path.read_text(encoding="utf-8"))
        harness_path = target / "attestation" / "harness-result.json"
        harness = json.loads(harness_path.read_text(encoding="utf-8"))
        harness["challenge_hex"] = source_harness["challenge_hex"]
        write_json(harness_path, harness)

    transcript_path = target / "handoff" / "d2d-payment.json"
    transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
    if key in transcript:
        transcript[key] = copied
        write_json(transcript_path, transcript)
        refresh_d2d_payment_transcript_hash(target, signer)

    wallet_transcript_path = target / "wallet" / "integrity.json"
    wallet_transcript = json.loads(wallet_transcript_path.read_text(encoding="utf-8"))
    if key in wallet_transcript:
        wallet_transcript[key] = copied
        write_json(wallet_transcript_path, wallet_transcript)
        refresh_wallet_integrity_transcript_hash(target, signer)

    evidence_path = target / "evidence" / "signed-evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    if key in evidence:
        evidence[key] = copied
    evidence["artifact_digests"] = required_artifact_digests(target)
    write_json(evidence_path, sign_evidence(evidence, signer))
    refresh_signed_evidence_hash(target)


def write_unsigned_production_slot_metadata(slot: Path, name: str, family: str) -> None:
    raw_test_commands = list(KAGEMUSHA_ANDROID_RAW_TEST_COMMANDS)
    kagemusha_wallet_apk_path = "evidence/kagemusha-wallet-release.apk"
    app_package_name = "org.hyperledger.iroha.kagemushawallet"
    device_model, device_codename = device_identity_for_family(family)
    telemetry_path = slot / "telemetry" / "telemetry.json"
    telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
    telemetry["device_model"] = device_model
    telemetry["device_codename"] = device_codename
    telemetry["app_package_name"] = app_package_name
    write_json(telemetry_path, telemetry)
    app_signing_certificate_sha256 = (
        device_lab.extract_apk_signing_certificate_sha256(
            slot / kagemusha_wallet_apk_path
        )
    )
    kagemusha_wallet_policy_sha256 = hashlib.sha256(
        b"kagemusha-wallet-policy-v1"
    ).hexdigest()
    attestation_certificate_chain_path = "attestation/keymint-certificate-chain.pem"
    attestation_certificate_chain_sha256 = hashlib.sha256(
        (slot / attestation_certificate_chain_path).read_bytes()
    ).hexdigest()
    kagemusha_wallet_apk_sha256 = hashlib.sha256(
        (slot / kagemusha_wallet_apk_path).read_bytes()
    ).hexdigest()
    candidate_metadata, attestation_challenge = write_candidate_binding_v2(
        slot,
        name,
        app_signing_certificate_sha256=app_signing_certificate_sha256,
        attestation_certificate_chain_sha256=attestation_certificate_chain_sha256,
    )
    attestation_certificate_chain_sha256 = hashlib.sha256(
        (slot / attestation_certificate_chain_path).read_bytes()
    ).hexdigest()
    attestation_challenge_sha256 = hashlib.sha256(attestation_challenge).hexdigest()
    attestation_harness_path = slot / "attestation" / "harness-result.json"
    attestation_harness = json.loads(attestation_harness_path.read_text(encoding="utf-8"))
    attestation_harness["challenge_hex"] = attestation_challenge.hex()
    write_json(attestation_harness_path, attestation_harness)
    for attestation_name in ("result.json", "report.json"):
        attestation_path = slot / "attestation" / attestation_name
        attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
        attestation["attestation_challenge_sha256"] = attestation_challenge_sha256
        attestation["attestation_certificate_chain_sha256"] = (
            attestation_certificate_chain_sha256
        )
        if attestation_name == "result.json":
            attestation["app_signing_certificate_sha256"] = (
                app_signing_certificate_sha256
            )
        write_json(attestation_path, attestation)
    d2d_payment_transcript_path, d2d_payment_transcript_sha256 = (
        write_d2d_payment_transcript(
            slot,
            name,
            family,
            device_fingerprint=f"{name}/fingerprint",
            os_build_id=f"{name}-build",
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=attestation_challenge_sha256,
            kagemusha_wallet_policy_sha256=kagemusha_wallet_policy_sha256,
            kagemusha_wallet_apk_sha256=kagemusha_wallet_apk_sha256,
        )
    )
    wallet_integrity_transcript_path, wallet_integrity_transcript_sha256 = (
        write_wallet_integrity_transcript(
            slot,
            name,
            family,
            device_fingerprint=f"{name}/fingerprint",
            os_build_id=f"{name}-build",
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=attestation_challenge_sha256,
            attestation_certificate_chain_sha256=attestation_certificate_chain_sha256,
            kagemusha_wallet_policy_sha256=kagemusha_wallet_policy_sha256,
            kagemusha_wallet_apk_sha256=kagemusha_wallet_apk_sha256,
        )
    )
    write_json(
        slot / "slot.json",
        {
            "schema": device_lab.KAGEMUSHA_SLOT_SCHEMA_V2,
            "slot_id": name,
            "device_family": family,
            "device_model": device_model,
            "device_codename": device_codename,
            "device_fingerprint": f"{name}/fingerprint",
            "os_build_id": f"{name}-build",
            "minimum_os": device_lab.KAGEMUSHA_STANDARD_DEVICE_MINIMUM_OS[family],
            "app_package_name": app_package_name,
            "attestation_certificate_chain_path": attestation_certificate_chain_path,
            "kagemusha_wallet_apk_path": kagemusha_wallet_apk_path,
            "d2d_payment_transcript_path": d2d_payment_transcript_path,
            "wallet_integrity_transcript_path": wallet_integrity_transcript_path,
            "app_signing_certificate_sha256": app_signing_certificate_sha256,
            "attestation_challenge_sha256": attestation_challenge_sha256,
            "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
            "kagemusha_wallet_policy_sha256": kagemusha_wallet_policy_sha256,
            "kagemusha_wallet_apk_sha256": kagemusha_wallet_apk_sha256,
            "d2d_payment_transcript_sha256": d2d_payment_transcript_sha256,
            "wallet_integrity_transcript_sha256": wallet_integrity_transcript_sha256,
            "native_bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
            "strongbox_attestation": True,
            "physical_device_attestation": True,
            "keymint_security_level": "STRONGBOX",
            "one_use_key_rotation_passed": True,
            "rollback_rejection_passed": True,
            "kagemusha_recursive_spend_ffi_surface": "passed",
            "kagemusha_recursive_spend_jni_probe": "recursive_spend_verified",
            "kagemusha_recursive_spend_prover_state": "multi_hop_proof_composed",
            **candidate_metadata,
            "signed_evidence_artifact_path": "evidence/signed-evidence.json",
            "signed_evidence_artifact_sha256": "0" * 64,
            "raw_test_commands": raw_test_commands,
        },
    )




class AndroidDeviceLabSlotTest(unittest.TestCase):

    def test_isolated_cli_resolves_repository_modules(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            result = subprocess.run(
                [sys.executable, "-I", "-B", str(MODULE_PATH), "--help"],
                cwd=temporary,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertIn("Validate Android device-lab slots", result.stdout)

    def test_candidate_android_scripts_pin_cargo_for_cargo_ndk_identity(self) -> None:
        scripts = MODULE_PATH.parent
        native_runner = (scripts / "build_kagemusha_candidate_android_native.sh").read_text(
            encoding="utf-8"
        )
        lab_runner = (scripts / "run_kagemusha_candidate_android_lab.sh").read_text(
            encoding="utf-8"
        )

        direct_version_call = '"$CARGO_NDK_BINARY" --version'
        pinned_version_call = (
            'CARGO="$CARGO_BINARY" "$CARGO_NDK_BINARY" --version'
        )
        self.assertEqual(3, native_runner.count(direct_version_call))
        self.assertEqual(3, native_runner.count(pinned_version_call))
        self.assertIn(
            'version(\n'
            '        [sys.argv[7], "--version"],\n'
            '        extra_environment={"CARGO": sys.argv[5]},\n'
            '    )',
            lab_runner,
        )

    def test_candidate_android_scripts_thread_reviewed_source_closure(self) -> None:
        scripts = MODULE_PATH.parent
        native_runner = (scripts / "build_kagemusha_candidate_android_native.sh").read_text(
            encoding="utf-8"
        )
        lab_runner = (scripts / "run_kagemusha_candidate_android_lab.sh").read_text(
            encoding="utf-8"
        )
        native_fingerprint = native_runner.split("source_fingerprint() {", 1)[1].split(
            "}", 1
        )[0]
        lab_fingerprint = lab_runner.split("source_snapshot() {", 1)[1].split(
            "}", 1
        )[0]
        for fingerprint in (native_fingerprint, lab_fingerprint):
            self.assertIn(
                '--reviewed-source-closure "$REVIEWED_SOURCE_CLOSURE"',
                fingerprint,
            )
            self.assertIn(
                '--reviewed-source-closure-sha256 '
                '"$REVIEWED_SOURCE_CLOSURE_SHA256"',
                fingerprint,
            )
        self.assertIn(
            'reviewed_source_closure_descriptor_sha256=sys.argv[7]',
            native_runner,
        )
        self.assertIn(
            'reviewed_source_closure_descriptor_sha256=sys.argv[7]',
            lab_runner,
        )
        self.assertIn(
            '--reviewed-source-closure "$REVIEWED_SOURCE_CLOSURE"',
            lab_runner,
        )

    def test_stage_catalog_binds_reviewed_source_closure_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            slot = Path(temporary) / "candidate-slot"
            slot.mkdir(mode=0o700)
            binding, _ = write_candidate_binding_v2(slot, slot.name)
            arguments = {
                "candidate_sha256": str(binding["candidate_record_sha256"]),
                "stage_sha256": str(binding["candidate_stage_manifest_sha256"]),
                "source_commit": str(binding["candidate_source_commit"]),
                "source_tree_sha256": str(binding["candidate_source_tree_sha256"]),
            }

            device_lab.validate_kagemusha_candidate_stage_manifest_v2(
                slot,
                reviewed_source_closure_descriptor_sha256="6" * 64,
                **arguments,
            )
            with self.assertRaisesRegex(ValueError, "does not match its explicit pin"):
                device_lab.validate_kagemusha_candidate_stage_manifest_v2(
                    slot,
                    reviewed_source_closure_descriptor_sha256="a" * 64,
                    **arguments,
                )

    def test_stage_catalog_metadata_mode_defers_content_authentication_to_streamer(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            slot = Path(temporary) / "candidate-slot"
            slot.mkdir(mode=0o700)
            binding, _ = write_candidate_binding_v2(slot, slot.name)
            artifact = slot / "evidence/candidate/artifacts/step-eq.params-ipa.krv4"
            original = artifact.read_bytes()
            artifact.write_bytes(bytes((original[0] ^ 1,)) + original[1:])
            artifact.chmod(0o600)
            arguments = {
                "candidate_sha256": str(binding["candidate_record_sha256"]),
                "stage_sha256": str(binding["candidate_stage_manifest_sha256"]),
                "source_commit": str(binding["candidate_source_commit"]),
                "source_tree_sha256": str(binding["candidate_source_tree_sha256"]),
            }
            with self.assertRaisesRegex(ValueError, "digest is not exact"):
                device_lab.validate_kagemusha_candidate_stage_manifest_v2(
                    slot,
                    **arguments,
                )
            manifest = device_lab.validate_kagemusha_candidate_stage_manifest_v2(
                slot,
                verify_entry_digests=False,
                **arguments,
            )
            self.assertEqual(manifest["candidate_record_sha256"], arguments["candidate_sha256"])

    def test_stage_catalog_rejects_a_symlinked_entry_parent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            slot = Path(temporary) / "candidate-slot"
            slot.mkdir(mode=0o700)
            binding, _ = write_candidate_binding_v2(slot, slot.name)
            artifact_parent = slot / "evidence/candidate/artifacts"
            real_parent = slot / "real-artifacts"
            artifact_parent.rename(real_parent)
            artifact_parent.symlink_to(real_parent, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "parent is not a real directory"):
                device_lab.validate_kagemusha_candidate_stage_manifest_v2(
                    slot,
                    candidate_sha256=str(binding["candidate_record_sha256"]),
                    stage_sha256=str(binding["candidate_stage_manifest_sha256"]),
                    source_commit=str(binding["candidate_source_commit"]),
                    source_tree_sha256=str(binding["candidate_source_tree_sha256"]),
                    verify_entry_digests=False,
                )

    @classmethod
    def setUpClass(cls) -> None:
        cls._authority_directory = tempfile.TemporaryDirectory()
        authority = Path(cls._authority_directory.name).resolve(strict=True)
        openssl_found = shutil.which("openssl")
        if openssl_found is None:
            raise unittest.SkipTest("OpenSSL is required for Android attestation tests")
        openssl = Path(openssl_found).resolve(strict=True)
        sdk_roots = tuple(
            Path(value).expanduser()
            for value in (
                os.environ.get("ANDROID_SDK_ROOT"),
                os.environ.get("ANDROID_HOME"),
                str(Path.home() / "Library" / "Android" / "sdk"),
            )
            if value
        )
        apksigners = sorted(
            candidate.resolve(strict=True)
            for sdk_root in sdk_roots
            for candidate in (sdk_root / "build-tools").glob("*/apksigner")
            if candidate.is_file() and os.access(candidate, os.X_OK)
        )
        if not apksigners:
            raise unittest.SkipTest("Android build-tools apksigner is required")
        apksigner = apksigners[-1]
        apksigner_jar = (apksigner.parent / "lib" / "apksigner.jar").resolve(
            strict=True
        )
        java_candidates = []
        if os.environ.get("JAVA_HOME"):
            java_candidates.append(Path(os.environ["JAVA_HOME"]) / "bin" / "java")
        java_candidates.extend(
            (
                Path("/opt/homebrew/opt/openjdk@21/bin/java"),
                Path("/opt/homebrew/opt/openjdk/bin/java"),
                Path("/usr/local/opt/openjdk@21/bin/java"),
                Path("/usr/local/opt/openjdk/bin/java"),
            )
        )
        java_found = shutil.which("java")
        if java_found is not None:
            java_candidates.append(Path(java_found))
        java = next(
            (
                candidate.resolve(strict=True)
                for candidate in java_candidates
                if candidate.is_file() and os.access(candidate, os.X_OK)
            ),
            None,
        )
        if java is None:
            raise unittest.SkipTest("a Java executable is required")
        java, apksigner_jar = (
            _android_apk_fixtures.stage_private_android_authority_tools(
                authority,
                java,
                apksigner_jar,
            )
        )
        root_key = authority / "android-attestation-test-root.key"
        root_cert = authority / "android-attestation-test-root.pem"
        subprocess.run(
            [
                str(openssl),
                "req",
                "-x509",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:P-256",
                "-nodes",
                "-keyout",
                str(root_key),
                "-out",
                str(root_cert),
                "-days",
                "3650",
                "-sha256",
                "-subj",
                "/CN=Iroha Android Attestation Test Root",
                "-addext",
                "basicConstraints=critical,CA:TRUE,pathlen:1",
                "-addext",
                "keyUsage=critical,keyCertSign,cRLSign",
            ],
            check=True,
            capture_output=True,
        )
        root_key.chmod(0o600)
        root_cert.chmod(0o600)
        status = authority / "android-attestation-status.json"
        write_json(status, {"entries": {}})
        capture_time_ms = (device_lab.time.time_ns() // 1_000_000_000) * 1_000
        format_http_date = lambda milliseconds: device_lab.android_status_capture.email.utils.format_datetime(
            device_lab.android_status_capture.dt.datetime.fromtimestamp(
                milliseconds / 1_000,
                tz=device_lab.android_status_capture.dt.timezone.utc,
            ),
            usegmt=True,
        )
        _, capture_receipt = device_lab.android_status_capture.build_capture(
            status.read_bytes(),
            [
                ("Date", format_http_date(capture_time_ms)),
                ("Age", "0"),
                ("Cache-Control", "public, max-age=86400"),
                ("Expires", format_http_date(capture_time_ms + 86_400_000)),
                ("Last-Modified", format_http_date(capture_time_ms)),
            ],
            captured_at_ms=capture_time_ms,
        )
        capture_receipt_path = authority / "android-attestation-status-capture.json"
        write_json(capture_receipt_path, capture_receipt)
        cls._authority_kwargs = {
            "java": java,
            "java_sha256": hashlib.sha256(java.read_bytes()).hexdigest(),
            "apksigner_jar": apksigner_jar,
            "apksigner_jar_sha256": hashlib.sha256(
                apksigner_jar.read_bytes()
            ).hexdigest(),
            "openssl": openssl,
            "openssl_sha256": hashlib.sha256(openssl.read_bytes()).hexdigest(),
            "attestation_trust_roots": [root_cert],
            "attestation_trust_root_sha256": [
                hashlib.sha256(root_cert.read_bytes()).hexdigest()
            ],
            "attestation_revocation_status": status,
            "attestation_revocation_status_sha256": hashlib.sha256(
                status.read_bytes()
            ).hexdigest(),
            "attestation_status_capture_receipt": capture_receipt_path,
            "attestation_status_capture_receipt_sha256": hashlib.sha256(
                capture_receipt_path.read_bytes()
            ).hexdigest(),
        }
        authority_errors = device_lab.configure_android_evidence_authority(
            **cls._authority_kwargs,
        )
        if authority_errors:
            raise AssertionError(authority_errors)
        _android_x509_fixtures.configure_test_authority(
            openssl, root_key, root_cert
        )
        cls._authority_cli_args = [
            "--java", str(java),
            "--java-sha256", cls._authority_kwargs["java_sha256"],
            "--apksigner-jar", str(apksigner_jar),
            "--apksigner-jar-sha256",
            cls._authority_kwargs["apksigner_jar_sha256"],
            "--openssl", str(openssl),
            "--openssl-sha256", cls._authority_kwargs["openssl_sha256"],
            "--android-attestation-trust-root", str(root_cert),
            "--android-attestation-trust-root-sha256",
            cls._authority_kwargs["attestation_trust_root_sha256"][0],
            "--android-attestation-revocation-status", str(status),
            "--android-attestation-revocation-status-sha256",
            cls._authority_kwargs["attestation_revocation_status_sha256"],
            "--android-attestation-status-capture-receipt",
            str(capture_receipt_path),
            "--android-attestation-status-capture-receipt-sha256",
            cls._authority_kwargs["attestation_status_capture_receipt_sha256"],
        ]
        cls._original_device_lab_main = staticmethod(device_lab.main)

        def configured_main(argv=None):
            arguments = list(argv or [])
            if (
                any(
                    flag in arguments
                    for flag in (
                        "--require-kagemusha-production-evidence",
                        "--require-kagemusha-standard-matrix",
                    )
                )
                and "--openssl" not in arguments
            ):
                arguments.extend(cls._authority_cli_args)
            return cls._original_device_lab_main(arguments)

        device_lab.main = configured_main

    @classmethod
    def tearDownClass(cls) -> None:
        _android_x509_fixtures.clear_test_authority()
        device_lab.main = cls._original_device_lab_main
        device_lab._ANDROID_EVIDENCE_AUTHORITY = None  # type: ignore[attr-defined]
        cls._authority_directory.cleanup()

    def test_kagemusha_production_evidence_requires_current_v4_bridge(self) -> None:
        self.assertEqual(device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION, 23)

    def test_android_authority_tools_are_private_canonical_files(self) -> None:
        authority = device_lab._ANDROID_EVIDENCE_AUTHORITY
        assert authority is not None
        paths = (
            Path(authority["java"]["path"]),
            Path(authority["java"]["path"]).with_name("keytool"),
            Path(authority["apksigner_jar"]["path"]),
        )
        for path in paths:
            metadata = path.lstat()
            self.assertEqual(path, path.resolve(strict=True))
            self.assertFalse(path.is_symlink())
            self.assertTrue(stat.S_ISREG(metadata.st_mode))
            self.assertEqual(metadata.st_uid, os.geteuid())
            self.assertEqual(metadata.st_nlink, 1)
            self.assertEqual(metadata.st_mode & 0o022, 0)

    def test_apk_verifier_executes_the_complete_pinned_java_jar_authority(self) -> None:
        main_apk, _, certificate_sha256 = (
            _android_apk_fixtures.signed_candidate_apk_fixture(device_lab)
        )
        authority = device_lab._ANDROID_EVIDENCE_AUTHORITY
        assert authority is not None
        signer_labels = (
            "Signer #1",
            "Signer (minSdkVersion=28, maxSdkVersion=2147483647)",
            "Signer (minSdkVersion=35 (dev release=true), "
            "maxSdkVersion=2147483647)",
        )
        for signer_label in signer_labels:
            with self.subTest(signer_label=signer_label):
                with tempfile.TemporaryDirectory() as temporary:
                    apk = Path(temporary) / "candidate.apk"
                    apk.write_bytes(main_apk)
                    completed = subprocess.CompletedProcess(
                        args=[],
                        returncode=0,
                        stdout=(
                            f"{signer_label} certificate SHA-256 digest: "
                            f"{certificate_sha256}\n"
                        ),
                        stderr="",
                    )
                    with (
                        mock.patch.object(
                            device_lab.subprocess,
                            "run",
                            return_value=completed,
                        ) as run,
                        mock.patch.object(
                            device_lab,
                            "_read_pinned_authority_file",
                            wraps=device_lab._read_pinned_authority_file,
                        ) as authenticate,
                    ):
                        measured = device_lab.extract_apk_signing_certificate_sha256(
                            apk
                        )

                self.assertEqual(measured, certificate_sha256)
                command = run.call_args.args[0]
                self.assertEqual(
                    command[:3],
                    [
                        os.fspath(authority["java"]["path"]),
                        "-jar",
                        os.fspath(authority["apksigner_jar"]["path"]),
                    ],
                )
                self.assertEqual(
                    run.call_args.kwargs["env"],
                    {
                        "HOME": "/var/empty",
                        "LANG": "C",
                        "LC_ALL": "C",
                        "PATH": "/usr/bin:/bin",
                    },
                )
                labels = [
                    call.kwargs["label"] for call in authenticate.call_args_list
                ]
                self.assertEqual(labels.count("configured Java executable"), 2)
                self.assertEqual(labels.count("configured apksigner.jar"), 2)

    def test_candidate_bound_v2_slot_passes_exact_inventory_validation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "ok", report["errors"])
        self.assertIn("candidate_inventory_sha256", report["kagemusha"])
        self.assertIs(report["kagemusha"]["production_capability_observed"], False)

    def test_android_strongbox_chain_projects_cryptographic_facts(self) -> None:
        metadata = android_attestation_metadata()
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
        )
        errors: list[str] = []
        count = device_lab._validate_android_attestation_certificate_chain(
            "attestation/keymint-certificate-chain.pem",
            chain,
            metadata,
            errors,
        )
        self.assertEqual(count, 2, errors)
        self.assertEqual(errors, [])

    def test_android_attestation_rechecks_status_freshness_for_each_chain(self) -> None:
        metadata = android_attestation_metadata("pixel8-stale-status")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
        )
        authority = device_lab._ANDROID_EVIDENCE_AUTHORITY
        assert authority is not None
        receipt = authority["attestation_status_capture_receipt"]["payload"]
        fresh_until_ms = receipt["fresh_until_ms"]
        errors: list[str] = []

        with mock.patch.object(
            device_lab.time,
            "time_ns",
            return_value=fresh_until_ms * 1_000_000,
        ):
            self.assertIsNone(
                device_lab._validate_android_attestation_certificate_chain(
                    "attestation/keymint-certificate-chain.pem",
                    chain,
                    metadata,
                    errors,
                )
            )

        self.assertTrue(
            any(
                "Android attestation status capture is stale during chain validation"
                in error
                for error in errors
            ),
            errors,
        )

    def test_failed_reconfiguration_and_missing_cli_cannot_reuse_stale_authority(self) -> None:
        bad = dict(self._authority_kwargs)
        bad["openssl_sha256"] = "0" * 64
        errors = device_lab.configure_android_evidence_authority(**bad)
        self.assertTrue(errors)
        self.assertIsNone(device_lab._ANDROID_EVIDENCE_AUTHORITY)

        metadata = android_attestation_metadata("pixel8-stale-authority")
        validation_errors: list[str] = []
        self.assertIsNone(
            device_lab._validate_android_attestation_certificate_chain(
                "attestation/chain.pem",
                b"-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----\n",
                metadata,
                validation_errors,
            )
        )
        self.assertIn(
            "digest-pinned Android attestation authority inputs are required",
            validation_errors,
        )

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve() / "slots"
            (root / "slot-one").mkdir(parents=True)
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = self._original_device_lab_main(
                    [
                        "--root",
                        str(root),
                        "--require-kagemusha-production-evidence",
                    ]
                )
        self.assertEqual(status, 1)
        self.assertIsNone(device_lab._ANDROID_EVIDENCE_AUTHORITY)

    def test_cli_has_exactly_one_direct_authority_configurator_call(self) -> None:
        module = ast.parse(MODULE_PATH.read_text(encoding="utf-8"))
        direct_calls = [
            node
            for node in ast.walk(module)
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id == "configure_android_evidence_authority"
        ]

        self.assertEqual(len(direct_calls), 1)

    def test_confirmation_mode_delegates_authority_configuration_once(self) -> None:
        authority_args = [
            "--java",
            "/authority/java",
            "--java-sha256",
            "11" * 32,
            "--apksigner-jar",
            "/authority/apksigner.jar",
            "--apksigner-jar-sha256",
            "12" * 32,
            "--openssl",
            "/authority/openssl",
            "--openssl-sha256",
            "22" * 32,
            "--android-attestation-revocation-status",
            "/authority/status.json",
            "--android-attestation-revocation-status-sha256",
            "33" * 32,
            "--android-attestation-status-capture-receipt",
            "/authority/status-capture-receipt.json",
            "--android-attestation-status-capture-receipt-sha256",
            "44" * 32,
        ]
        with (
            mock.patch.object(
                device_lab,
                "_configure_android_evidence_authority_from_args",
                return_value=[],
            ) as configure,
            mock.patch.object(
                device_lab,
                "load_trusted_signer_public_keys",
                return_value=([object()], []),
            ),
            mock.patch.object(
                device_lab,
                "validate_kagemusha_android_confirmation",
                return_value={"status": "ok"},
            ),
            mock.patch.object(device_lab, "write_summary", return_value=[]),
            redirect_stdout(io.StringIO()),
            redirect_stderr(io.StringIO()),
        ):
            status = self._original_device_lab_main(
                [
                    "--confirmation-reference-slot",
                    "/evidence/reference",
                    "--confirmation-binding",
                    "/evidence/candidate-binding-v2.json",
                    "--confirmation-lifecycle",
                    "/evidence/lifecycle-transcript-v2.json",
                    "--confirmation-json-out",
                    "/evidence/confirmation.json",
                    "--trusted-signer-public-key",
                    "/authority/signer.pem",
                    *authority_args,
                ]
            )

        self.assertEqual(status, 0)
        configure.assert_called_once()

    def test_slot_mode_delegates_authority_configuration_once(self) -> None:
        def install_authority(_args: argparse.Namespace) -> list[str]:
            device_lab._ANDROID_EVIDENCE_AUTHORITY = {
                "java": {"sha256": "11" * 32},
                "apksigner_jar": {"sha256": "12" * 32},
                "openssl": {"sha256": "22" * 32},
                "attestation_trust_roots": (),
                "attestation_revocation_status": {"sha256": "33" * 32},
                "attestation_status_capture_receipt": {
                    "sha256": "44" * 32,
                    "snapshot": {
                        "payload_sha256": [0] * 32,
                        "response_date_ms": 1_800_000_000_000,
                        "last_modified_ms": 1_800_000_000_000,
                        "cache_max_age_seconds": 86_400,
                        "non_valid_serials": [],
                    },
                },
            }
            return []

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            slot = root / "slot-one"
            slot.mkdir(parents=True)
            with (
                mock.patch.object(
                    device_lab,
                    "_configure_android_evidence_authority_from_args",
                    side_effect=install_authority,
                ) as configure,
                mock.patch.object(
                    device_lab,
                    "discover_slots",
                    return_value=([slot], []),
                ),
                mock.patch.object(
                    device_lab,
                    "load_trusted_signer_public_keys",
                    return_value=([object()], []),
                ),
                mock.patch.object(
                    device_lab,
                    "scan_slot",
                    return_value={"slot": "slot-one", "status": "ok", "errors": []},
                ),
                redirect_stdout(io.StringIO()),
                redirect_stderr(io.StringIO()),
            ):
                status = self._original_device_lab_main(
                    [
                        "--root",
                        str(root),
                        "--require-kagemusha-production-evidence",
                        "--java",
                        "/authority/java",
                        "--java-sha256",
                        "11" * 32,
                        "--apksigner-jar",
                        "/authority/apksigner.jar",
                        "--apksigner-jar-sha256",
                        "12" * 32,
                        "--openssl",
                        "/authority/openssl",
                        "--openssl-sha256",
                        "22" * 32,
                        "--android-attestation-revocation-status",
                        "/authority/status.json",
                        "--android-attestation-revocation-status-sha256",
                        "33" * 32,
                        "--android-attestation-status-capture-receipt",
                        "/authority/status-capture-receipt.json",
                        "--android-attestation-status-capture-receipt-sha256",
                        "44" * 32,
                    ]
                )

        self.assertEqual(status, 0)
        configure.assert_called_once()

    def test_forged_attestation_report_cannot_rescue_fake_certificate_chain(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(
                Path(temp) / "slots",
                "pixel8-forged-report",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_payload = json.loads(
                (slot / "attestation" / "report.json").read_text(encoding="utf-8")
            )
            self.assertIs(report_payload["verification"]["strongbox_attestation"], True)
            self.assertIs(report_payload["verification"]["physical_device_attestation"], True)
            write_text(
                slot / "attestation" / "keymint-certificate-chain.pem",
                "-----BEGIN CERTIFICATE-----\nnot-a-certificate\n"
                "-----END CERTIFICATE-----\n",
            )
            rewrite_sha256sum(slot)
            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )
        self.assertEqual(report["status"], "error")
        self.assertTrue(
            any(
                "Android StrongBox certificate-chain validation failed" in error
                for error in report["errors"]
            ),
            report["errors"],
        )

    def test_production_wallet_apk_substitution_rejects_different_valid_signer(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(
                Path(temp) / "slots",
                "pixel8-wallet-substitution",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            wallet_path = slot / metadata["kagemusha_wallet_apk_path"]
            candidate_path = slot / metadata["candidate_lab_apk_path"]
            wallet_path.write_bytes(candidate_path.read_bytes())
            wallet_path.chmod(0o600)
            rewrite_sha256sum(slot)
            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )
        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json app_signing_certificate_sha256 must match the verified production wallet APK signer",
            report["errors"],
        )

    def test_android_attestation_rejects_untrusted_root_and_wrong_challenge(self) -> None:
        metadata = android_attestation_metadata("pixel8-root-challenge")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
        )
        certificates = device_lab._decode_attestation_certificate_chain(
            "chain.pem", chain
        )
        unrelated_root = (
            MODULE_PATH.parents[1] / "certs" / "google_attestation_root_ecdsa.der"
        ).read_bytes()
        untrusted_chain = device_lab._certificate_pem(certificates[0]) + (
            device_lab._certificate_pem(unrelated_root)
        )
        errors: list[str] = []
        self.assertIsNone(
            device_lab._validate_android_attestation_certificate_chain(
                "attestation/chain.pem", untrusted_chain, metadata, errors
            )
        )
        self.assertTrue(any("explicit trusted root" in error for error in errors), errors)

        wrong_challenge_chain = test_android_attestation_chain(
            b"\xff" * 32,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
        )
        errors = []
        self.assertIsNone(
            device_lab._validate_android_attestation_certificate_chain(
                "attestation/chain.pem", wrong_challenge_chain, metadata, errors
            )
        )
        self.assertTrue(any("exact candidate challenge" in error for error in errors), errors)

    def test_android_attestation_rejects_non_strongbox_wrong_app_and_bad_boot(self) -> None:
        metadata = android_attestation_metadata("pixel8-policy")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        signer = bytes.fromhex(metadata["app_signing_certificate_sha256"])
        variants = (
            (
                "attestation level",
                test_android_attestation_chain(
                    challenge,
                    metadata["app_package_name"],
                    signer,
                    attestation_level=1,
                ),
                "both be StrongBox(2)",
            ),
            (
                "keymint level",
                test_android_attestation_chain(
                    challenge,
                    metadata["app_package_name"],
                    signer,
                    keymint_level=1,
                ),
                "both be StrongBox(2)",
            ),
            (
                "package",
                test_android_attestation_chain(
                    challenge,
                    "org.example.substituted",
                    signer,
                ),
                "wallet package",
            ),
            (
                "signer",
                test_android_attestation_chain(
                    challenge,
                    metadata["app_package_name"],
                    b"\x77" * 32,
                ),
                "wallet signing digest",
            ),
            (
                "boot state",
                test_android_attestation_chain(
                    challenge,
                    metadata["app_package_name"],
                    signer,
                    verified_boot_state=1,
                ),
                "verifiedBootState=Verified",
            ),
            (
                "device lock",
                test_android_attestation_chain(
                    challenge,
                    metadata["app_package_name"],
                    signer,
                    device_locked=False,
                ),
                "deviceLocked=true",
            ),
            (
                "nonstandard ninth field",
                test_android_attestation_chain(
                    challenge,
                    metadata["app_package_name"],
                    signer,
                    append_ninth_sequence=True,
                ),
                "trailing DER data",
            ),
        )
        for label, chain, expected in variants:
            with self.subTest(label=label):
                errors: list[str] = []
                self.assertIsNone(
                    device_lab._validate_android_attestation_certificate_chain(
                        "attestation/chain.pem", chain, metadata, errors
                    )
                )
                self.assertTrue(any(expected in error for error in errors), errors)

    def test_android_attestation_rejects_every_authenticated_non_valid_serial(self) -> None:
        metadata = android_attestation_metadata("pixel8-revoked")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
            chain_kind="rkp",
        )
        certificates = device_lab._decode_attestation_certificate_chain("chain.pem", chain)
        authority = device_lab._ANDROID_EVIDENCE_AUTHORITY
        assert authority is not None
        status_record = authority["attestation_revocation_status"]
        original_payload = status_record["payload"]
        for certificate in certificates:
            serial = device_lab._x509_certificate_serial(certificate)
            try:
                status_record["payload"] = {
                    "entries": {
                        serial: {"status": "SUSPENDED", "reason": "KEY_COMPROMISE"}
                    }
                }
                errors: list[str] = []
                self.assertIsNone(
                    device_lab._validate_android_attestation_certificate_chain(
                        "attestation/chain.pem", chain, metadata, errors
                    )
                )
            finally:
                status_record["payload"] = original_payload
            self.assertTrue(
                any("authenticated revocation status" in error for error in errors), errors
            )

    def test_android_attestation_rejects_every_authenticated_revoked_tbs(self) -> None:
        metadata = android_attestation_metadata("pixel8-revoked-tbs")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
            chain_kind="rkp",
        )
        certificates = device_lab._decode_attestation_certificate_chain("chain.pem", chain)
        authority = device_lab._ANDROID_EVIDENCE_AUTHORITY
        assert authority is not None
        receipt_payload = authority["attestation_status_capture_receipt"]["payload"]
        original_tbs = receipt_payload["android_sdk_revoked_tbs_sha256"]
        for certificate in certificates:
            try:
                receipt_payload["android_sdk_revoked_tbs_sha256"] = [
                    device_lab._x509_certificate_tbs_sha256(certificate)
                ]
                errors: list[str] = []
                self.assertIsNone(
                    device_lab._validate_android_attestation_certificate_chain(
                        "attestation/chain.pem", chain, metadata, errors
                    )
                )
            finally:
                receipt_payload["android_sdk_revoked_tbs_sha256"] = original_tbs
            self.assertTrue(
                any("certificate TBS digest" in error for error in errors), errors
            )

    def test_candidate_causal_stream_rejects_unrelated_valid_digests(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "pixel8-causal"
            write_candidate_binding_v2(
                slot,
                slot.name,
                app_signing_certificate_sha256=hashlib.sha256(b"wallet").hexdigest(),
            )
            transcript = json.loads(
                (slot / device_lab.KAGEMUSHA_CANDIDATE_LIFECYCLE_TRANSCRIPT_PATH).read_text(
                    encoding="utf-8"
                )
            )
        events = transcript["causal_events"]
        for sequence, event in enumerate(events):
            event["input_sha256"] = [
                hashlib.sha256(f"unrelated:{sequence}:{index}".encode()).hexdigest()
                for index in range(len(event["input_sha256"]))
            ]
        errors: list[str] = []
        device_lab._validate_candidate_causal_events_v1(events, errors)
        self.assertTrue(any("causal linkage failed" in error for error in errors), errors)

    def test_raw_command_contract_rejects_omission_and_substitution(self) -> None:
        commands = list(device_lab.KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS)
        for label, mutated in (
            ("omitted export", commands[:-1]),
            (
                "substituted lifecycle",
                [
                    *commands[:2],
                    commands[2].replace(
                        "KagemushaCandidateLifecycleInstrumentedTest",
                        "UnrelatedInstrumentedTest",
                    ),
                    commands[3],
                ],
            ),
        ):
            with self.subTest(label=label):
                errors: list[str] = []
                device_lab._validate_raw_test_command_markers(
                    mutated, label="raw_test_commands", errors=errors
                )
                self.assertTrue(errors)
                self.assertTrue(
                    any("exactly match" in error or "must include" in error for error in errors),
                    errors,
                )

    def test_confirmation_comparator_allows_only_duration_variance(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            signer = create_test_signer(root / "keys")
            reference = create_slot(
                root / "slots",
                "pixel8-reference",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((reference / "slot.json").read_text(encoding="utf-8"))
            binding = json.loads(
                (reference / metadata["candidate_binding_path"]).read_text(encoding="utf-8")
            )
            lifecycle = json.loads(
                (reference / metadata["candidate_lifecycle_transcript_path"]).read_text(
                    encoding="utf-8"
                )
            )
            lifecycle["causal_events"][0]["duration_nanos"] += 100
            confirmation_dir = root / "confirmation"
            lifecycle_path = confirmation_dir / "lifecycle-transcript-v2.json"
            binding_path = confirmation_dir / "candidate-binding-v2.json"
            write_json(lifecycle_path, lifecycle)
            binding["lifecycle_transcript_sha256"] = hashlib.sha256(
                lifecycle_path.read_bytes()
            ).hexdigest()
            write_json(binding_path, binding)
            report = device_lab.validate_kagemusha_android_confirmation(
                reference_slot=reference,
                confirmation_binding_path=binding_path,
                confirmation_lifecycle_path=lifecycle_path,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )
            self.assertEqual(report["status"], "ok", report["errors"])
            self.assertEqual(
                set(report["artifacts"]),
                {
                    "reference_binding",
                    "reference_lifecycle",
                    "confirmation_binding",
                    "confirmation_lifecycle",
                },
            )

            lifecycle["causal_events"][2]["input_sha256"][0] = hashlib.sha256(
                b"substituted-init-request"
            ).hexdigest()
            write_json(lifecycle_path, lifecycle)
            binding["lifecycle_transcript_sha256"] = hashlib.sha256(
                lifecycle_path.read_bytes()
            ).hexdigest()
            write_json(binding_path, binding)
            rejected = device_lab.validate_kagemusha_android_confirmation(
                reference_slot=reference,
                confirmation_binding_path=binding_path,
                confirmation_lifecycle_path=lifecycle_path,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )
        self.assertEqual(rejected["status"], "error")
        self.assertTrue(
            any(
                "outside causal duration_nanos" in error
                or "causal linkage failed" in error
                for error in rejected["errors"]
            ),
            rejected["errors"],
        )

    def test_v1_slot_is_not_accepted_as_production_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["schema"] = device_lab.KAGEMUSHA_SLOT_SCHEMA_V1
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)
            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertTrue(
            any("V1 evidence is not production evidence" in error for error in report["errors"]),
            report["errors"],
        )

    def test_candidate_binding_rejects_reordered_exact_eight_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            binding_path = slot / device_lab.KAGEMUSHA_CANDIDATE_BINDING_ARTIFACT_PATH
            binding = json.loads(binding_path.read_text(encoding="utf-8"))
            binding["artifact_inventory"][0], binding["artifact_inventory"][1] = (
                binding["artifact_inventory"][1],
                binding["artifact_inventory"][0],
            )
            write_json(binding_path, binding)
            metadata["candidate_binding_sha256"] = hashlib.sha256(
                binding_path.read_bytes()
            ).hexdigest()
            errors: list[str] = []
            device_lab.validate_candidate_binding_v2(slot, metadata, errors)

        self.assertTrue(any("role must be" in error for error in errors), errors)

    def test_candidate_binding_rejects_substituted_krv4_payload(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            binding = json.loads(
                (slot / device_lab.KAGEMUSHA_CANDIDATE_BINDING_ARTIFACT_PATH).read_text(
                    encoding="utf-8"
                )
            )
            artifact_path = slot / binding["artifact_inventory"][0]["path"]
            artifact_path.write_bytes(artifact_path.read_bytes() + b"substitution")
            errors: list[str] = []
            device_lab.validate_candidate_binding_v2(slot, metadata, errors)

        self.assertTrue(
            any("does not match the KRV4 file" in error for error in errors), errors
        )

    def test_candidate_binding_requires_production_capability_to_remain_false(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            binding_path = slot / device_lab.KAGEMUSHA_CANDIDATE_BINDING_ARTIFACT_PATH
            binding = json.loads(binding_path.read_text(encoding="utf-8"))
            binding["production_capability_observed"] = True
            write_json(binding_path, binding)
            metadata["candidate_binding_sha256"] = hashlib.sha256(
                binding_path.read_bytes()
            ).hexdigest()
            errors: list[str] = []
            device_lab.validate_candidate_binding_v2(slot, metadata, errors)

        self.assertIn("candidate binding production_capability_observed must be false", errors)

    def test_candidate_binding_rejects_wallet_apk_as_candidate_lab_apk(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            binding_path = slot / device_lab.KAGEMUSHA_CANDIDATE_BINDING_ARTIFACT_PATH
            binding = json.loads(binding_path.read_text(encoding="utf-8"))
            binding["lab_apk_path"] = metadata["kagemusha_wallet_apk_path"]
            binding["lab_apk_sha256"] = metadata["kagemusha_wallet_apk_sha256"]
            metadata["candidate_lab_apk_path"] = binding["lab_apk_path"]
            metadata["candidate_lab_apk_sha256"] = binding["lab_apk_sha256"]
            write_json(binding_path, binding)
            metadata["candidate_binding_sha256"] = hashlib.sha256(
                binding_path.read_bytes()
            ).hexdigest()
            errors: list[str] = []
            device_lab.validate_candidate_binding_v2(slot, metadata, errors)

        self.assertIn(
            "candidate lab APK path must be distinct from the wallet APK path",
            errors,
        )
        self.assertIn(
            "candidate lab APK digest must be distinct from the wallet APK digest",
            errors,
        )

    def test_candidate_lifecycle_rejects_nonconserving_observed_value(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            lifecycle_path = slot / device_lab.KAGEMUSHA_CANDIDATE_LIFECYCLE_TRANSCRIPT_PATH
            lifecycle = json.loads(lifecycle_path.read_text(encoding="utf-8"))
            lifecycle["sender_change_atomic"] = "241"
            write_json(lifecycle_path, lifecycle)
            lifecycle_sha256 = hashlib.sha256(lifecycle_path.read_bytes()).hexdigest()
            binding_path = slot / device_lab.KAGEMUSHA_CANDIDATE_BINDING_ARTIFACT_PATH
            binding = json.loads(binding_path.read_text(encoding="utf-8"))
            binding["lifecycle_transcript_sha256"] = lifecycle_sha256
            write_json(binding_path, binding)
            metadata["candidate_lifecycle_transcript_sha256"] = lifecycle_sha256
            metadata["candidate_binding_sha256"] = hashlib.sha256(
                binding_path.read_bytes()
            ).hexdigest()
            errors: list[str] = []
            device_lab.validate_candidate_binding_v2(slot, metadata, errors)

        self.assertIn(
            "candidate lifecycle transcript does not conserve exact atomic value",
            errors,
        )

    def setUp(self) -> None:
        restore_path_type_method_shadows()
        errors = device_lab.configure_android_evidence_authority(
            **self._authority_kwargs
        )
        if errors:
            raise AssertionError(errors)

    def tearDown(self) -> None:
        restore_path_type_method_shadows()

    def test_control_character_helper_rejects_unicode_format_controls(self) -> None:
        unsafe_path = "slot-\u202ejson"

        self.assertTrue(device_lab._contains_control_character("\x1b[31m"))
        self.assertTrue(device_lab._contains_control_character(unsafe_path))
        self.assertEqual(
            device_lab._display_path(unsafe_path),
            device_lab.CONTROL_PATH_REDACTION,
        )

    def test_secret_detector_rejects_common_assignment_markers(self) -> None:
        unsafe_values = (
            "token=supersecret",
            "secret=supersecret",
            "client_secret=supersecret",
            "password=supersecret",
            "api_key=supersecret",
            "api-key=supersecret",
            "private_key=supersecret",
            "Authorization: Bearer supersecret",
        )
        for value in unsafe_values:
            with self.subTest(value=value):
                self.assertTrue(device_lab.SECRET_RE.search(value))

        self.assertFalse(device_lab.SECRET_RE.search("secretariat-release-notes"))

    def test_checked_in_sample_slot_passes_default_validation(self) -> None:
        root = Path(__file__).resolve().parents[2] / "fixtures" / "android" / "device_lab"
        report = device_lab.scan_slot(root / "slot-sample")
        self.assertEqual(report["status"], "ok", report["errors"])


    def test_scan_slot_rejects_missing_attestation_harness_result(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            (slot / "attestation" / "harness-result.json").unlink()
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn("missing attestation/harness-result.json", report["errors"])
        self.assertIn(
            "signed evidence artifact required slot artifact is missing "
            "attestation/harness-result.json",
            report["errors"],
        )

    def test_scan_slot_rejects_attestation_harness_challenge_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            harness_path = slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["challenge_hex"] = "00"
            write_json(harness_path, harness)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/harness-result.json challenge_hex digest must match "
            "slot.json attestation_challenge_sha256",
            report["errors"],
        )

    def test_scan_slot_rejects_noncanonical_attestation_harness_strings(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            harness_path = slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["alias"] = " android-keystore-alias "
            harness["attestation_security_level"] = " strong_box "
            harness["keymaster_security_level"] = "strongbox"
            harness["challenge_hex"] = "01 02 03 04"
            write_json(harness_path, harness)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/harness-result.json alias must not have surrounding whitespace",
            report["errors"],
        )
        self.assertIn(
            "attestation/harness-result.json attestation_security_level must not have surrounding whitespace",
            report["errors"],
        )
        self.assertIn(
            "attestation/harness-result.json keymaster_security_level must be STRONGBOX",
            report["errors"],
        )
        self.assertIn(
            "attestation/harness-result.json challenge_hex must be lowercase hexadecimal without whitespace",
            report["errors"],
        )

    def test_scan_slot_rejects_control_attestation_harness_strings(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            harness_path = slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["alias"] = "android-keystore-alias\x1b[31m"
            write_json(harness_path, harness)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/harness-result.json alias must not contain control characters",
            report["errors"],
        )
        self.assertNotIn("\x1b", "\n".join(report["errors"]))

    def test_scan_slot_rejects_sha256_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            write_text(slot / "logs" / "runtime.log", "tampered after manifest\n")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertIn("sha256sum.txt digest mismatch for logs/runtime.log", report["errors"])

    def test_scan_slot_rejects_padded_sha256sum_line(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            manifest = slot / "sha256sum.txt"
            lines = manifest.read_text(encoding="utf-8").splitlines()
            lines[0] = f"{lines[0]} "
            write_text(manifest, "\n".join(lines) + "\n")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "sha256sum.txt line 1: must not contain surrounding whitespace",
            report["errors"],
        )

    def test_scan_slot_rejects_zero_sha256sum_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            write_text(slot / "sha256sum.txt", f"{'0' * 64}  logs/runtime.log\n")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "sha256sum.txt line 1: non-canonical sha256 digest",
            report["errors"],
        )

    def test_scan_slot_rejects_star_normalized_sha256sum_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            runtime_log = slot / "logs" / "runtime.log"
            digest = hashlib.sha256(runtime_log.read_bytes()).hexdigest()
            write_text(slot / "sha256sum.txt", f"{digest}  *logs/runtime.log\n")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "sha256sum.txt line 1: unsafe path '*logs/runtime.log'",
            report["errors"],
        )

    def test_scan_slot_rejects_noncanonical_sha256sum_path(self) -> None:
        cases = ("logs/./runtime.log", "logs//runtime.log", "logs/runtime.log/")
        for relative in cases:
            with self.subTest(relative=relative):
                with tempfile.TemporaryDirectory() as temp:
                    slot = create_slot(Path(temp), "slot-a")
                    runtime_log = slot / "logs" / "runtime.log"
                    digest = hashlib.sha256(runtime_log.read_bytes()).hexdigest()
                    write_text(slot / "sha256sum.txt", f"{digest}  {relative}\n")

                    report = device_lab.scan_slot(slot)

                self.assertEqual(report["status"], "error")
                self.assertIn(
                    "sha256sum.txt line 1: unsafe path is not canonical",
                    report["errors"],
                )

    def test_normalise_safe_relative_path_rejects_control_before_strip(
        self,
    ) -> None:
        errors: list[str] = []

        normalised = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
            "\nlogs/runtime.log",
            errors,
            "slot artifact path",
        )

        rendered = "\n".join(errors)
        self.assertIsNone(normalised)
        self.assertEqual(
            errors,
            ["slot artifact path: unsafe path contains control characters"],
        )
        self.assertNotIn("\nlogs/runtime.log", rendered)

    def test_normalise_safe_relative_path_rejects_surrounding_whitespace(
        self,
    ) -> None:
        cases = (" logs/runtime.log ", "logs/ runtime.log")
        for relative in cases:
            with self.subTest(relative=relative):
                errors: list[str] = []

                normalised = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
                    relative,
                    errors,
                    "slot artifact path",
                )

                self.assertIsNone(normalised)
                self.assertEqual(
                    errors,
                    ["slot artifact path: unsafe path contains surrounding whitespace"],
                )

    def test_normalise_safe_relative_path_rejects_noncanonical_aliases(
        self,
    ) -> None:
        cases = (
            "logs/./runtime.log",
            "logs//runtime.log",
            "logs/runtime.log/",
        )
        for relative in cases:
            with self.subTest(relative=relative):
                errors: list[str] = []

                normalised = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
                    relative,
                    errors,
                    "slot artifact path",
                )

                self.assertIsNone(normalised)
                self.assertEqual(
                    errors,
                    ["slot artifact path: unsafe path is not canonical"],
                )

    def test_scan_slot_rejects_symlinked_slot_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = create_slot(root, "real-slot")
            slot_link = root / "slot-link"
            create_dir_symlink(self, slot_link, real_slot)

            report = device_lab.scan_slot(slot_link)

        self.assertEqual(report["status"], "error")
        self.assertEqual(report["errors"], ["slot directory must not be a symlink"])

    def test_scan_slot_rejects_slot_directory_metadata_failure(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated slot lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                report = device_lab.scan_slot(slot)
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(report["status"], "error")
        self.assertEqual(report["errors"], ["slot directory metadata could not be read"])

    def test_scan_slot_rejects_symlinked_slot_parent_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_root = root / "real-slots"
            create_slot(real_root, "slot-a")
            root_link = root / "linked-slots"
            create_dir_symlink(self, root_link, real_root)

            report = device_lab.scan_slot(root_link / "slot-a")

        self.assertEqual(report["status"], "error")
        self.assertEqual(report["errors"], ["slot parent directory must not be a symlink"])

    def test_scan_slot_rejects_slot_parent_metadata_failure(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp) / "slots", "slot-a")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot.parent:
                        raise OSError("simulated slot parent lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                report = device_lab.scan_slot(slot)
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(report["status"], "error")
        self.assertEqual(
            report["errors"],
            ["slot parent directory metadata could not be read"],
        )

    def test_scan_slot_uses_lstat_before_expected_directory_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                logs_dir = slot / "logs"

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == logs_dir:
                        raise OSError("simulated scan slot directory is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                report = device_lab.scan_slot(slot)
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(report["status"], "ok", report["errors"])
        self.assertTrue(report["present"]["logs"])
        self.assertEqual(report["file_counts"]["logs"], 1)

    def test_scan_slot_reports_expected_directory_metadata_failure_before_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                logs_dir = slot / "logs"

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == logs_dir:
                        raise OSError("simulated scan slot directory is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == logs_dir:
                        raise OSError("simulated scan slot directory metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir
                path_type.lstat = failing_lstat

                report = device_lab.scan_slot(slot)
        finally:
            path_type.lstat = original_lstat
            path_type.is_dir = original_is_dir

        self.assertEqual(report["status"], "error")
        self.assertIn("logs/ metadata could not be read", report["errors"])
        self.assertFalse(report["present"]["logs"])
        self.assertNotIn("missing logs/ directory", report["errors"])

    def test_scan_slot_counts_artifacts_with_lstat_before_is_file_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                runtime_log = slot / "logs" / "runtime.log"

                def failing_is_file(path: Path, *args, **kwargs):
                    if path == runtime_log:
                        raise OSError("simulated scan slot artifact is_file failure")
                    return original_is_file(path, *args, **kwargs)

                path_type.is_file = failing_is_file

                report = device_lab.scan_slot(slot)
        finally:
            path_type.is_file = original_is_file

        self.assertEqual(report["status"], "ok", report["errors"])
        self.assertEqual(report["file_counts"]["logs"], 1)

    def test_scan_slot_sha_presence_uses_lstat_before_is_file_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                sha_path = slot / "sha256sum.txt"

                def failing_is_file(path: Path, *args, **kwargs):
                    if path == sha_path:
                        raise OSError("simulated scan slot sha is_file failure")
                    return original_is_file(path, *args, **kwargs)

                path_type.is_file = failing_is_file

                report = device_lab.scan_slot(slot)
        finally:
            path_type.is_file = original_is_file

        self.assertEqual(report["status"], "ok", report["errors"])
        self.assertTrue(report["present"]["sha256sum.txt"])

    def test_scan_slot_rejects_symlinked_slot_ancestor_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            external_parent = root / "external-parent"
            real_root = external_parent / "device_lab"
            create_slot(real_root, "slot-a")
            linked_parent = root / "linked-parent"
            create_dir_symlink(self, linked_parent, external_parent)

            report = device_lab.scan_slot(linked_parent / "device_lab" / "slot-a")

        self.assertEqual(report["status"], "error")
        self.assertEqual(report["errors"], ["slot ancestor directory must not be a symlink"])

    def test_scan_slot_rejects_directory_traversal_failure_without_traceback(self) -> None:
        original_scandir = device_lab.os.scandir

        def failing_scandir(path: Path):
            if not isinstance(path, (str, bytes, device_lab.os.PathLike)):
                return original_scandir(path)
            if Path(path).name == "logs":
                raise OSError("simulated directory traversal failure")
            return original_scandir(path)

        try:
            device_lab.os.scandir = failing_scandir
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")

                report = device_lab.scan_slot(slot)
        finally:
            device_lab.os.scandir = original_scandir

        rendered = json.dumps(report)
        self.assertEqual(report["status"], "error")
        self.assertIn("logs/ could not be listed", report["errors"])
        self.assertNotIn("Traceback", rendered)

    def test_load_json_rejects_symlinked_ancestor_before_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_json_dir = root / "real-json"
            real_json_dir.mkdir()
            write_json(real_json_dir / "payload.json", {"status": "ok"})
            linked_json_dir = root / "linked-json"
            create_dir_symlink(self, linked_json_dir, real_json_dir)
            errors: list[str] = []

            data = device_lab._load_json(
                linked_json_dir / "payload.json",
                "test json",
                errors,
            )

        self.assertIsNone(data)
        self.assertEqual(errors, ["test json ancestor directory must not be a symlink"])

    def test_load_json_rejects_symlink_swap_after_preflight(self) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                payload = root / "payload.json"
                target = root / "aliased-payload.json"
                write_json(payload, {"status": "ok"})
                write_json(target, {"status": "aliased"})
                errors: list[str] = []
                swapped = False

                def swapping_stat(path: Path, *args, **kwargs):
                    nonlocal swapped
                    result = original_stat(path, *args, **kwargs)
                    if path == payload and not swapped:
                        replace_with_symlink(self, payload, target)
                        swapped = True
                    return result

                path_type.stat = swapping_stat

                data = device_lab._load_json(payload, "test json", errors)
                target_status = json.loads(target.read_text(encoding="utf-8"))["status"]
        finally:
            path_type.stat = original_stat

        self.assertTrue(swapped)
        self.assertIsNone(data)
        self.assertEqual(errors, ["test json must not be a symlink"])
        self.assertEqual(target_status, "aliased")

    def test_load_json_rejects_regular_file_swap_after_preflight(self) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                payload = root / "payload.json"
                replacement = root / "replacement-payload.json"
                write_json(payload, {"status": "ok"})
                write_json(replacement, {"status": "replacement"})
                errors: list[str] = []
                swapped = False

                def swapping_stat(path: Path, *args, **kwargs):
                    nonlocal swapped
                    result = original_stat(path, *args, **kwargs)
                    if path == payload and not swapped:
                        replacement.replace(payload)
                        swapped = True
                    return result

                path_type.stat = swapping_stat

                data = device_lab._load_json(payload, "test json", errors)
                final_status = json.loads(payload.read_text(encoding="utf-8"))["status"]
        finally:
            path_type.stat = original_stat

        self.assertTrue(swapped)
        self.assertIsNone(data)
        self.assertEqual(errors, ["test json changed while being read"])
        self.assertEqual(final_status, "replacement")

    def test_validate_no_symlink_ancestors_rejects_cwd_failure(self) -> None:
        with mock.patch.object(
            Path,
            "cwd",
            side_effect=OSError("simulated cwd failure"),
        ):
            errors = device_lab.validate_no_symlink_ancestors(
                Path("payload.json"),
                "test json ancestor directory",
            )

        self.assertEqual(errors, ["test json ancestor directory metadata could not be read"])

    def test_validate_no_symlink_ancestors_rejects_ancestor_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                payload = root / "slot" / "payload.json"
                payload.parent.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == payload.parent:
                        raise OSError("simulated ancestor metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.validate_no_symlink_ancestors(
                    payload,
                    "test json ancestor directory",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["test json ancestor directory metadata could not be read"])

    def test_validate_no_symlink_ancestors_uses_lstat_before_is_symlink_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_symlink = path_type.is_symlink

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                payload = root / "slot" / "payload.json"
                payload.parent.mkdir()

                def failing_is_symlink(path: Path, *args, **kwargs):
                    if path == payload.parent:
                        raise OSError("simulated ancestor is_symlink failure")
                    return original_is_symlink(path, *args, **kwargs)

                path_type.is_symlink = failing_is_symlink

                errors = device_lab.validate_no_symlink_ancestors(
                    payload,
                    "test json ancestor directory",
                )
        finally:
            path_type.is_symlink = original_is_symlink

        self.assertEqual(errors, [])

    def test_validate_no_symlink_ancestors_uses_lstat_before_exists_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_exists = path_type.exists

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                payload = root / "slot" / "payload.json"
                payload.parent.mkdir()

                def failing_exists(path: Path, *args, **kwargs):
                    if path == payload.parent:
                        raise OSError("simulated ancestor exists failure")
                    return original_exists(path, *args, **kwargs)

                path_type.exists = failing_exists

                errors = device_lab.validate_no_symlink_ancestors(
                    payload,
                    "test json ancestor directory",
                )
        finally:
            path_type.exists = original_exists

        self.assertEqual(errors, [])

    def test_load_json_rejects_secret_path_directly_before_parse(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_dir = Path(temp) / "token=supersecret-json"
            secret_dir.mkdir()
            payload = secret_dir / "payload.json"
            payload.write_text("{not-json", encoding="utf-8")
            errors: list[str] = []

            data = device_lab._load_json(payload, "test json", errors)
            rendered = "\n".join(errors)

        self.assertIsNone(data)
        self.assertEqual(
            errors,
            ["test json path must not contain secret-looking material"],
        )
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(payload), rendered)

    def test_load_json_rejects_control_path_directly_before_parse(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            control_dir = Path(temp) / "control\njson"
            control_dir.mkdir()
            payload = control_dir / "payload.json"
            payload.write_text("{not-json", encoding="utf-8")
            errors: list[str] = []

            data = device_lab._load_json(payload, "test json", errors)
            rendered = "\n".join(errors)

        self.assertIsNone(data)
        self.assertEqual(errors, ["test json path must not contain control characters"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn(str(payload), rendered)

    def test_load_json_rejects_alias_path_directly_before_metadata(self) -> None:
        cases = (
            (
                lambda base: base / "json" / " payload.json",
                "test json path must not contain surrounding whitespace",
            ),
            (
                lambda base: base / "json\\payload.json",
                "test json path must not contain backslashes",
            ),
            (
                lambda base: base / "json" / ".." / "payload.json",
                "test json path must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    payload = path_factory(Path(temp))
                    errors: list[str] = []

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias JSON path should fail before metadata"
                        ),
                    ):
                        data = device_lab._load_json(payload, "test json", errors)

                self.assertIsNone(data)
                self.assertEqual(errors, [expected_error])

    def test_load_json_rejects_nonfinite_json_constant(self) -> None:
        for constant in ("NaN", "Infinity", "-Infinity"):
            with self.subTest(constant=constant), tempfile.TemporaryDirectory() as temp:
                payload = Path(temp) / "payload.json"
                payload.write_text(f'{{"value": {constant}}}\n', encoding="utf-8")
                errors: list[str] = []

                data = device_lab._load_json(payload, "test json", errors)
                rendered = "\n".join(errors)

            self.assertIsNone(data)
            self.assertEqual(
                errors,
                [
                    "test json contains non-finite constant "
                    f"{device_lab.JSON_NONFINITE_CONSTANT_REDACTION}"
                ],
            )
            self.assertNotIn(constant, rendered)

    def test_load_json_rejects_oversized_json_before_parse(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            payload = Path(temp) / "payload.json"
            payload.write_bytes(
                b'{"value":"' + b"x" * device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES + b'"}\n'
            )
            errors: list[str] = []

            data = device_lab._load_json(payload, "test json", errors)

        self.assertIsNone(data)
        self.assertEqual(
            errors,
            [
                "test json must be no more than "
                f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
            ],
        )

    def test_load_json_rejects_non_utf8_bytes_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            payload = Path(temp) / "payload.json"
            payload.write_bytes(b"\xff\xfe\xfd")
            errors: list[str] = []

            data = device_lab._load_json(payload, "test json", errors)
            rendered = "\n".join(errors)

        self.assertIsNone(data)
        self.assertEqual(errors, ["test json could not be read"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn(str(payload), rendered)

    def test_load_json_rejects_file_metadata_failure_before_missing(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                payload = Path(temp) / "payload.json"
                write_json(payload, {"status": "ok"})
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == payload:
                        raise OSError("simulated JSON metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                data = device_lab._load_json(payload, "test json", errors)
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(data)
        self.assertEqual(errors, ["test json file metadata could not be read"])

    def test_scan_slot_rejects_symlinked_required_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact logs/runtime.log must not be a symlink",
            report["errors"],
        )

    def test_scan_slot_rejects_hardlinked_required_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact logs/runtime.log must not be hardlinked",
            report["errors"],
        )

    def test_scan_slot_rejects_non_regular_required_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            replace_with_fifo(self, slot / "logs" / "runtime.log")

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact logs/runtime.log must be a regular file",
            report["errors"],
        )
        self.assertIn(
            "sha256sum.txt references non-regular artifact logs/runtime.log",
            report["errors"],
        )

    def test_production_metadata_rejects_symlinked_signed_evidence_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            evidence_path = slot / "evidence" / "signed-evidence.json"
            target = root / "outside-signed-evidence.json"
            target.write_bytes(evidence_path.read_bytes())
            replace_with_symlink(self, evidence_path, target)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact evidence/signed-evidence.json must not be a symlink",
            report["errors"],
        )

    def test_production_metadata_rejects_hardlinked_signed_evidence_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            evidence_path = slot / "evidence" / "signed-evidence.json"
            target = root / "outside-signed-evidence.json"
            target.write_bytes(evidence_path.read_bytes())
            replace_with_hardlink(self, evidence_path, target)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact evidence/signed-evidence.json must not be hardlinked",
            report["errors"],
        )

    def test_production_metadata_rejects_non_regular_signed_evidence_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            replace_with_fifo(self, slot / "evidence" / "signed-evidence.json")

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact evidence/signed-evidence.json must be a regular file",
            report["errors"],
        )

    def test_explicit_missing_slot_returns_structured_error(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "missing-slot",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("[device-lab] missing-slot: slot directory missing", stderr.getvalue())
        self.assertNotIn("Traceback", stderr.getvalue())

    def test_validate_slot_ids_rejects_duplicate_explicit_slots(self) -> None:
        slot_ids, errors = device_lab.validate_slot_ids(
            ["slot-a", "slot-a", "slot-b", "slot-b"]
        )

        self.assertEqual(slot_ids, ["slot-a", "slot-b"])
        self.assertEqual(
            errors,
            [
                "slot id 1 must not duplicate slot id 0",
                "slot id 3 must not duplicate slot id 2",
            ],
        )

    def test_validate_slot_ids_rejects_noncanonical_slot_aliases(self) -> None:
        slot_ids, errors = device_lab.validate_slot_ids(
            ["./slot-a", "slot-b/", "slot-c/."]
        )

        self.assertEqual(slot_ids, [])
        self.assertEqual(
            errors,
            [
                "slot id './slot-a' must be a canonical single directory name",
                "slot id 'slot-b/' must be a canonical single directory name",
                "slot id 'slot-c/.' must be a canonical single directory name",
            ],
        )

    def test_explicit_duplicate_slot_id_rejected_before_scan(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            create_slot(root, "slot-a")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "slot-a",
                        "--slot",
                        "slot-a",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn("slot id 1 must not duplicate slot id 0", rendered)
        self.assertNotIn("[device-lab] slot-a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_root_validator_rejects_secret_path_directly_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_root = Path(temp) / "token=supersecret-slots"

            errors = device_lab.validate_device_lab_root_path(secret_root)
            rendered = json.dumps(errors)

        self.assertEqual(
            errors,
            ["device-lab root path must not contain secret-looking material"],
        )
        self.assertNotIn(str(secret_root), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_root_validator_rejects_control_path_directly_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            control_root = Path(temp) / "control\nslots"

            errors = device_lab.validate_device_lab_root_path(control_root)
            rendered = json.dumps(errors)

        self.assertEqual(errors, ["device-lab root path must not contain control characters"])
        self.assertNotIn(str(control_root), rendered)

    def test_root_validator_rejects_alias_path_directly_before_metadata(self) -> None:
        cases = (
            (
                lambda base: base / "slots\\alias",
                "device-lab root path must not contain backslashes",
            ),
            (
                lambda base: base / "slots" / ".." / "alias",
                "device-lab root path must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    root = path_factory(Path(temp))

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias device-lab root should fail before metadata"
                        ),
                    ):
                        errors = device_lab.validate_device_lab_root_path(root)

                self.assertEqual(errors, [expected_error])

    def test_root_validator_rejects_metadata_failure_directly_without_leak(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "slots"
                root.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == root:
                        raise OSError("simulated device-lab root metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.validate_device_lab_root_path(root)
                rendered = json.dumps(errors)
                root_exists = root.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["device-lab root metadata could not be read"])
        self.assertTrue(root_exists)
        self.assertNotIn(str(root), rendered)

    def test_main_uses_lstat_before_missing_root_exists_preflight(self) -> None:
        path_type = type(Path("."))
        original_exists = path_type.exists

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "missing-slots"

                def failing_exists(path: Path, *args, **kwargs):
                    if path == root:
                        raise OSError("simulated device-lab root exists failure")
                    return original_exists(path, *args, **kwargs)

                path_type.exists = failing_exists

                stdout = io.StringIO()
                stderr = io.StringIO()
                with redirect_stdout(stdout), redirect_stderr(stderr):
                    status = device_lab.main(
                        [
                            "--root",
                            str(root),
                            "--allow-missing-root",
                        ]
                    )
                rendered = stdout.getvalue() + stderr.getvalue()
        finally:
            path_type.exists = original_exists

        self.assertEqual(status, 0)
        self.assertIn("[device-lab] root missing; skipping", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_main_rejects_symlinked_device_lab_root_before_discovery(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_root = root / "real-slots"
            create_slot(real_root, "slot-a")
            root_link = root / "linked-slots"
            create_dir_symlink(self, root_link, real_root)

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root_link),
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn("device-lab root must not be a symlink", rendered)
        self.assertNotIn("[device-lab] slot-a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_main_rejects_symlinked_device_lab_root_ancestor_before_discovery(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            external_parent = root / "external-parent"
            real_root = external_parent / "device_lab"
            create_slot(real_root, "slot-a")
            linked_parent = root / "linked-parent"
            create_dir_symlink(self, linked_parent, external_parent)

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(linked_parent / "device_lab"),
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn(
            "device-lab root ancestor directory must not be a symlink",
            rendered,
        )
        self.assertNotIn("[device-lab] slot-a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_discover_slots_returns_structured_error_on_root_list_failure(self) -> None:
        original_iterdir = Path.iterdir

        def failing_iterdir(path: Path):
            if path.name == "device_lab":
                raise OSError("simulated root listing failure")
            return original_iterdir(path)

        try:
            Path.iterdir = failing_iterdir
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"
                root.mkdir()

                slot_paths, errors = device_lab.discover_slots(root, None)
        finally:
            Path.iterdir = original_iterdir

        self.assertEqual(slot_paths, [])
        self.assertEqual(errors, ["device-lab root could not be listed"])

    def test_discover_slots_uses_lstat_before_is_dir_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"
                slot = create_slot(root, "slot-a")

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated discovery is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                slot_paths, errors = device_lab.discover_slots(root, None)
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(slot_paths, [slot])
        self.assertEqual(errors, [])

    def test_discover_slots_reports_slot_metadata_failure_before_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"
                slot = create_slot(root, "slot-a")

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated discovery is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated discovery metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir
                path_type.lstat = failing_lstat

                slot_paths, errors = device_lab.discover_slots(root, None)
        finally:
            path_type.lstat = original_lstat
            path_type.is_dir = original_is_dir

        self.assertEqual(slot_paths, [])
        self.assertEqual(
            errors,
            ["device-lab slot directory metadata could not be read"],
        )

    def test_discover_slots_preserves_symlinked_slot_for_scan_slot_rejection(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            root = base / "device_lab"
            root.mkdir()
            target_slot = create_slot(base / "external_slots", "slot-a")
            linked_slot = root / "slot-link"
            create_dir_symlink(self, linked_slot, target_slot)

            slot_paths, errors = device_lab.discover_slots(root, None)

        self.assertEqual(slot_paths, [linked_slot])
        self.assertEqual(errors, [])

    def test_discover_slots_returns_stable_sorted_order(self) -> None:
        original_iterdir = Path.iterdir

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"
                slot_b = create_slot(root, "slot-b")
                slot_a = create_slot(root, "slot-a")

                def reverse_iterdir(path: Path):
                    if path == root:
                        return iter([slot_b, slot_a])
                    return original_iterdir(path)

                Path.iterdir = reverse_iterdir

                slot_paths, errors = device_lab.discover_slots(root, None)
        finally:
            Path.iterdir = original_iterdir

        self.assertEqual(slot_paths, [slot_a, slot_b])
        self.assertEqual(errors, [])

    def test_discover_slots_revalidates_explicit_slot_ids_directly(self) -> None:
        original_iterdir = Path.iterdir

        def unexpected_iterdir(path: Path):
            raise AssertionError("root discovery must not run for invalid slot ids")

        try:
            Path.iterdir = unexpected_iterdir
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"

                slot_paths, errors = device_lab.discover_slots(
                    root,
                    ["slot-a", "slot-a", "../outside"],
                )
        finally:
            Path.iterdir = original_iterdir

        self.assertEqual(slot_paths, [])
        self.assertIn("slot id 1 must not duplicate slot id 0", errors)
        self.assertIn(
            "slot id '../outside' must be a single safe directory name",
            errors,
        )

    def test_main_rejects_device_lab_root_list_failure_without_traceback(self) -> None:
        original_iterdir = Path.iterdir

        def failing_iterdir(path: Path):
            if path.name == "device_lab":
                raise OSError("simulated root listing failure")
            return original_iterdir(path)

        try:
            Path.iterdir = failing_iterdir
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"
                root.mkdir()
                stdout = io.StringIO()
                stderr = io.StringIO()

                with redirect_stdout(stdout), redirect_stderr(stderr):
                    status = device_lab.main(
                        [
                            "--root",
                            str(root),
                            "--require-slot",
                        ]
                    )
                rendered = stdout.getvalue() + stderr.getvalue()
        finally:
            Path.iterdir = original_iterdir

        self.assertEqual(status, 1)
        self.assertIn("device-lab root could not be listed", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_unsafe_slot_id_rejected_before_path_join(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            root.mkdir()
            create_slot(Path(temp), "outside")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "../outside",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn(
            "slot id '../outside' must be a single safe directory name",
            rendered,
        )
        self.assertNotIn("[device-lab] outside: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_noncanonical_slot_id_rejected_before_path_join(self) -> None:
        for slot_id in ("./slot-a", "slot-a/", "slot-a/."):
            with self.subTest(slot_id=slot_id):
                with tempfile.TemporaryDirectory() as temp:
                    root = Path(temp) / "slots"
                    create_slot(root, "slot-a")
                    stdout = io.StringIO()
                    stderr = io.StringIO()
                    with redirect_stdout(stdout), redirect_stderr(stderr):
                        status = device_lab.main(
                            [
                                "--root",
                                str(root),
                                "--slot",
                                slot_id,
                                "--require-slot",
                            ]
                        )

                self.assertEqual(status, 1)
                rendered = stdout.getvalue() + stderr.getvalue()
                self.assertIn(
                    f"slot id {slot_id!r} must be a canonical single directory name",
                    rendered,
                )
                self.assertNotIn("[device-lab] slot-a: ok", rendered)
                self.assertNotIn("Traceback", rendered)

    def test_explicit_slot_id_rejects_surrounding_whitespace_before_path_join(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            create_slot(root, "slot-a")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        " slot-a ",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn(
            "slot id 0 must not contain whitespace",
            rendered,
        )
        self.assertNotIn("[device-lab] slot-a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_slot_id_rejects_newline_before_path_join(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            create_slot(root, "slot-a")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "slot-a\n",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn(
            "slot id 0 must not contain whitespace",
            rendered,
        )
        self.assertNotIn("[device-lab] slot-a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_slot_id_rejects_internal_whitespace_before_path_join(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            create_slot(root, "slot a")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "slot a",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn("slot id 0 must not contain whitespace", rendered)
        self.assertNotIn("[device-lab] slot a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_slot_id_rejects_control_character_before_path_join(self) -> None:
        unsafe_slot_id = "slot-a\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            create_slot(root, unsafe_slot_id)
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        unsafe_slot_id,
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn("slot id 0 must not contain control characters", rendered)
        self.assertNotIn(unsafe_slot_id, rendered)
        self.assertNotIn("\x1b", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_secret_looking_slot_id_is_not_echoed(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            root.mkdir()
            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "token=supersecret",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot id 0 must not contain secret-looking material",
            stderr.getvalue(),
        )
        self.assertNotIn("token=supersecret", stderr.getvalue())
        self.assertNotIn("Traceback", stderr.getvalue())

    def test_discovered_secret_looking_slot_directory_is_not_echoed(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            secret_slot = root / "token=supersecret-slot"
            secret_slot.mkdir(parents=True)
            summary_path = Path(temp) / "summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--json-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary_text = summary_path.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertIn(
            "slot directory name must not contain secret-looking material",
            rendered,
        )
        self.assertIn(device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertIn(device_lab.SECRET_PATH_REDACTION, summary_text)
        self.assertNotIn(str(secret_slot), rendered)
        self.assertNotIn(str(secret_slot), summary_text)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("token=supersecret", summary_text)
        self.assertNotIn("Traceback", rendered)

    def test_discovered_whitespace_slot_directory_is_rejected_before_metadata(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            slot = create_slot(root, "slot a")
            summary_path = Path(temp) / "summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--json-out",
                        str(summary_path),
                    ]
            )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary_text = summary_path.read_text(encoding="utf-8")
            slot_still_exists = slot.exists()

        self.assertEqual(status, 1)
        self.assertIn("slot directory name must not contain whitespace", rendered)
        self.assertIn("slot directory name must not contain whitespace", summary_text)
        self.assertNotIn("[device-lab] slot a: ok", rendered)
        self.assertNotIn("Traceback", rendered)
        self.assertTrue(slot_still_exists)

    def test_discovered_control_slot_directory_is_rejected_without_echo(
        self,
    ) -> None:
        unsafe_slot_name = "slot-a\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            slot = create_slot(root, unsafe_slot_name)
            summary_path = Path(temp) / "summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--json-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary_text = summary_path.read_text(encoding="utf-8")
            slot_still_exists = slot.exists()

        self.assertEqual(status, 1)
        self.assertIn(
            "slot directory name must not contain control characters",
            rendered,
        )
        self.assertIn(
            "slot directory name must not contain control characters",
            summary_text,
        )
        self.assertIn("<unsafe-slot-name>", rendered)
        self.assertIn("<unsafe-slot-name>", summary_text)
        self.assertNotIn(unsafe_slot_name, rendered)
        self.assertNotIn(unsafe_slot_name, summary_text)
        self.assertNotIn("\x1b", rendered)
        self.assertNotIn("\x1b", summary_text)
        self.assertNotIn("Traceback", rendered)
        self.assertTrue(slot_still_exists)

    def test_discovered_backslash_slot_directory_is_rejected_before_metadata(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            slot = create_slot(root, "slot-a\\b")
            summary_path = Path(temp) / "summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--json-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary_text = summary_path.read_text(encoding="utf-8")
            slot_still_exists = slot.exists()

        self.assertEqual(status, 1)
        self.assertIn("slot directory name must not contain backslashes", rendered)
        self.assertIn(
            "slot directory name must not contain backslashes",
            summary_text,
        )
        self.assertNotIn("[device-lab] slot-a\\b: ok", rendered)
        self.assertNotIn("Traceback", rendered)
        self.assertTrue(slot_still_exists)

    def test_scan_slot_rejects_control_slot_directory_before_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a\x1b[31m")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertEqual(report["slot"], "<unsafe-slot-name>")
        self.assertEqual(
            report["errors"],
            ["slot directory name must not contain control characters"],
        )

    def test_scan_slot_rejects_backslash_slot_directory_before_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a\\b")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertEqual(
            report["errors"],
            ["slot directory name must not contain backslashes"],
        )

    def test_scan_slot_rejects_newline_slot_directory_before_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a\n")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertEqual(
            report["errors"],
            ["slot directory name must not contain whitespace"],
        )

    def test_scan_slot_redacts_secret_looking_manifest_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            secret_path = "logs/token=supersecret.log"
            write_text(slot / secret_path, "must not leak\n")
            write_text(
                slot / "sha256sum.txt",
                f"{'01' * 32}  {secret_path}\n",
            )

            report = device_lab.scan_slot(slot)

        rendered_errors = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            "sha256sum.txt line 1: unsafe path contains secret-looking material",
            rendered_errors,
        )
        self.assertNotIn(secret_path, rendered_errors)
        self.assertIn(device_lab.SECRET_PATH_REDACTION, rendered_errors)

    def test_slot_files_missing_slot_returns_empty_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "missing-slot"

            files = device_lab._slot_files(slot)  # type: ignore[attr-defined]

        self.assertEqual(files, set())

    def test_slot_root_entries_returns_stable_sorted_order(self) -> None:
        original_iterdir = Path.iterdir

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                z_entry = slot / "z-extra"
                a_entry = slot / "a-extra"
                write_text(z_entry, "z\n")
                write_text(a_entry, "a\n")

                def reverse_iterdir(path: Path):
                    if path == slot:
                        return iter([z_entry, a_entry])
                    return original_iterdir(path)

                Path.iterdir = reverse_iterdir

                errors: list[str] = []
                entries = device_lab._slot_root_entries(  # type: ignore[attr-defined]
                    slot,
                    errors,
                )
        finally:
            Path.iterdir = original_iterdir

        self.assertEqual(entries, [a_entry, z_entry])
        self.assertEqual(errors, [])

    def test_slot_files_non_directory_root_returns_empty_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "slot-a"
            write_text(slot, "not a directory\n")

            files = device_lab._slot_files(slot)  # type: ignore[attr-defined]

        self.assertEqual(files, set())

    def test_slot_files_reports_slot_metadata_failure_without_omission(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated slot lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                files = device_lab._slot_files(slot, errors)  # type: ignore[attr-defined]
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(files, set())
        self.assertEqual(errors, ["slot directory metadata could not be read"])

    def test_slot_files_secret_slot_path_returns_empty_without_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "token=supersecret-slot")

            files = device_lab._slot_files(slot)  # type: ignore[attr-defined]

        self.assertEqual(files, set())

    def test_slot_files_rejects_symlinked_slot_root_directly_without_traversal(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = create_slot(root / "real", "slot-a")
            linked_slot = root / "linked-slot"
            create_dir_symlink(self, linked_slot, real_slot)

            files = device_lab._slot_files(linked_slot)  # type: ignore[attr-defined]

        self.assertEqual(files, set())

    def test_slot_files_rejects_symlinked_slot_ancestor_directly_without_traversal(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_root = root / "real-root"
            create_slot(real_root, "slot-a")
            linked_root = root / "linked-root"
            create_dir_symlink(self, linked_root, real_root)

            files = device_lab._slot_files(  # type: ignore[attr-defined]
                linked_root / "slot-a"
            )

        self.assertEqual(files, set())

    def test_slot_files_skips_symlinked_artifact_directory_directly_without_traversal(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            external_logs = root / "external-logs"
            write_text(external_logs / "runtime.log", "external log\n")
            for entry in (slot / "logs").iterdir():
                entry.unlink()
            (slot / "logs").rmdir()
            create_dir_symlink(self, slot / "logs", external_logs)

            files = device_lab._slot_files(slot)  # type: ignore[attr-defined]

        self.assertNotIn("logs/runtime.log", files)

    def test_slot_files_reports_artifact_directory_metadata_failure_without_omission(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                directory = slot / "logs"
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == directory:
                        raise OSError("simulated artifact directory lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                files = device_lab._slot_files(slot, errors)  # type: ignore[attr-defined]
        finally:
            path_type.lstat = original_lstat

        self.assertNotIn("logs/runtime.log", files)
        self.assertIn("logs/ metadata could not be read", errors)

    def test_slot_files_reports_top_level_listing_failure_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            errors: list[str] = []

            with patch_slot_iterdir_failure(slot):
                files = device_lab._slot_files(slot, errors)  # type: ignore[attr-defined]

        self.assertIn("slot directory could not be listed", errors)
        self.assertIn("logs/runtime.log", files)

    def test_slot_files_reports_artifact_metadata_failure_without_omission(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                artifact = slot / "logs" / "runtime.log"
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == artifact:
                        raise OSError("simulated inventory metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                files = device_lab._slot_files(slot, errors)  # type: ignore[attr-defined]
        finally:
            path_type.lstat = original_lstat

        self.assertIn(
            "slot artifact logs/runtime.log file metadata could not be read",
            errors,
        )
        self.assertNotIn("logs/runtime.log", files)

    def test_artifact_shape_validators_report_top_level_listing_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            symlink_errors: list[str] = []
            hardlink_errors: list[str] = []
            regular_errors: list[str] = []

            with patch_slot_iterdir_failure(slot):
                device_lab.validate_no_slot_symlink_artifacts(slot, symlink_errors)
                device_lab.validate_no_slot_hardlink_artifacts(slot, hardlink_errors)
                device_lab.validate_slot_regular_file_artifacts(slot, regular_errors)

        self.assertIn("slot directory could not be listed", symlink_errors)
        self.assertIn("slot directory could not be listed", hardlink_errors)
        self.assertIn("slot directory could not be listed", regular_errors)

    def test_verify_sha256_manifest_reports_top_level_listing_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            rewrite_sha256sum(slot)

            with patch_slot_iterdir_failure(slot):
                errors = device_lab.verify_sha256_manifest(slot)

        self.assertIn("slot directory could not be listed", errors)

    def test_required_signed_evidence_digest_paths_reports_top_level_listing_failure(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            errors: list[str] = []

            with patch_slot_iterdir_failure(slot):
                paths = device_lab._required_signed_evidence_digest_paths(  # type: ignore[attr-defined]
                    slot,
                    errors,
                )

        self.assertIn("slot directory could not be listed", errors)
        self.assertIn("logs/runtime.log", paths)

    def test_required_signed_evidence_digest_path_predicate_rejects_root_only_dirs(
        self,
    ) -> None:
        for relative in (
            "telemetry",
            "telemetry/",
            "attestation",
            "attestation/",
            "queue",
            "queue/",
            "logs",
            "logs/",
            "handoff",
            "handoff/",
            "wallet",
            "wallet/",
            "evidence",
            "evidence/",
        ):
            with self.subTest(relative=relative):
                self.assertFalse(
                    device_lab._is_required_signed_evidence_digest_path(relative)  # type: ignore[attr-defined]
                )

        for relative in (
            "logs/runtime.log",
            "telemetry/status.ndjson",
            "handoff/d2d-payment.json",
            "wallet/integrity.json",
            "evidence/kagemusha-wallet-release.apk",
        ):
            with self.subTest(relative=relative):
                self.assertTrue(
                    device_lab._is_required_signed_evidence_digest_path(relative)  # type: ignore[attr-defined]
                )

        self.assertFalse(
            device_lab._is_required_signed_evidence_digest_path(  # type: ignore[attr-defined]
                device_lab.KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH
            )
        )

    def test_signer_manifest_rewrite_rejects_top_level_listing_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")

            with patch_slot_iterdir_failure(slot):
                errors = evidence_signer.rewrite_sha256_manifest(slot)

        self.assertEqual(errors, ["slot directory could not be listed"])

    def test_parse_sha256_manifest_rejects_secret_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            slot.mkdir()
            write_text(slot / "sha256sum.txt", "not-a-manifest-line\n")

            entries, errors = device_lab.parse_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_parse_sha256_manifest_rejects_control_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "control\nslot"
            slot.mkdir()
            write_text(slot / "sha256sum.txt", "not-a-manifest-line\n")

            entries, errors = device_lab.parse_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["slot path must not contain control characters"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_parse_sha256_manifest_rejects_whitespace_slot_path_before_metadata(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / " slot"

            with mock.patch.object(
                Path,
                "lstat",
                side_effect=AssertionError(
                    "whitespace manifest slot path should fail before metadata"
                ),
            ):
                entries, errors = device_lab.parse_sha256_manifest(slot)

        self.assertEqual(entries, {})
        self.assertEqual(
            errors,
            ["slot path must not contain surrounding whitespace"],
        )
        self.assertNotIn(str(slot), "\n".join(errors))

    def test_parse_sha256_manifest_rejects_alias_slot_path_before_metadata(
        self,
    ) -> None:
        cases = (
            (
                lambda base: base / "slot\\alias",
                "slot path must not contain backslashes",
            ),
            (
                lambda base: base / "slot" / ".." / "alias",
                "slot path must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    slot = path_factory(Path(temp))

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias manifest slot path should fail before metadata"
                        ),
                    ):
                        entries, errors = device_lab.parse_sha256_manifest(slot)

                self.assertEqual(entries, {})
                self.assertEqual(errors, [expected_error])

    def test_parse_sha256_manifest_rejects_symlinked_slot_root_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = create_slot(root / "real", "slot-a")
            write_text(real_slot / "sha256sum.txt", "not-a-manifest-line\n")
            linked_slot = root / "linked-slot"
            create_dir_symlink(self, linked_slot, real_slot)

            entries, errors = device_lab.parse_sha256_manifest(linked_slot)
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["slot directory must not be a symlink"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_parse_sha256_manifest_rejects_slot_metadata_failure_before_parse(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                write_text(slot / "sha256sum.txt", "not-a-manifest-line\n")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated slot lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                entries, errors = device_lab.parse_sha256_manifest(slot)
                rendered = "\n".join(errors)
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["slot directory metadata could not be read"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_parse_sha256_manifest_rejects_symlinked_slot_ancestor_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_root = root / "real-root"
            real_slot = create_slot(real_root, "slot-a")
            write_text(real_slot / "sha256sum.txt", "not-a-manifest-line\n")
            linked_root = root / "linked-root"
            create_dir_symlink(self, linked_root, real_root)

            entries, errors = device_lab.parse_sha256_manifest(linked_root / "slot-a")
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["slot ancestor directory must not be a symlink"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_parse_sha256_manifest_rejects_hardlinked_manifest_before_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            external_manifest = root / "external-sha256sum.txt"
            write_text(external_manifest, "not-a-manifest-line\n")
            replace_with_hardlink(self, slot / "sha256sum.txt", external_manifest)

            entries, errors = device_lab.parse_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["sha256sum.txt must not be hardlinked"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_parse_sha256_manifest_rejects_file_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                manifest_path = slot / "sha256sum.txt"
                write_text(manifest_path, "not-a-manifest-line\n")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == manifest_path:
                        raise OSError("simulated manifest file metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                entries, errors = device_lab.parse_sha256_manifest(slot)
                rendered = "\n".join(errors)
                manifest_text = manifest_path.read_text(encoding="utf-8")
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["sha256sum.txt file metadata could not be read"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)
        self.assertEqual(manifest_text, "not-a-manifest-line\n")

    def test_parse_sha256_manifest_rejects_hardlink_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                manifest_path = slot / "sha256sum.txt"
                write_text(manifest_path, "not-a-manifest-line\n")

                def failing_stat(path: Path, *args, **kwargs):
                    if path == manifest_path and kwargs.get("follow_symlinks", True):
                        raise OSError("simulated manifest hardlink metadata failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                entries, errors = device_lab.parse_sha256_manifest(slot)
                rendered = "\n".join(errors)
        finally:
            path_type.stat = original_stat

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["sha256sum.txt hardlink metadata could not be read"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_parse_sha256_manifest_rejects_non_utf8_bytes_without_traceback(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            (slot / "sha256sum.txt").write_bytes(b"\xff\xfe\xfd")

            entries, errors = device_lab.parse_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["sha256sum.txt could not be read"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_parse_sha256_manifest_rejects_oversized_manifest_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            manifest_path = slot / "sha256sum.txt"
            manifest_path.write_bytes(
                b"#"
                + b"x" * device_lab.MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES
                + b"\n"
            )

            entries, errors = device_lab.parse_sha256_manifest(slot)

        self.assertEqual(entries, {})
        self.assertEqual(
            errors,
            [
                "sha256sum.txt must be no more than "
                f"{device_lab.MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES} bytes"
            ],
        )

    def test_parse_sha256_manifest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                slot = create_slot(root, "slot-a")
                manifest_path = slot / "sha256sum.txt"
                replacement = root / "replacement-sha256sum.txt"
                write_text(replacement, "not-a-manifest-line\n")
                swapped = False

                def swapping_stat(path: Path, *args, **kwargs):
                    nonlocal swapped
                    result = original_stat(path, *args, **kwargs)
                    if path == manifest_path and not swapped:
                        replacement.replace(manifest_path)
                        swapped = True
                    return result

                path_type.stat = swapping_stat

                entries, errors = device_lab.parse_sha256_manifest(slot)
                rendered = "\n".join(errors)
        finally:
            path_type.stat = original_stat

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["sha256sum.txt changed while being read"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_verify_sha256_manifest_rejects_secret_slot_path_directly_before_traversal(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            slot.mkdir()
            logs_dir = slot / "logs"
            logs_dir.mkdir()
            write_text(logs_dir / "runtime.log", "must not be traversed\n")
            write_text(slot / "sha256sum.txt", "\n")

            errors = device_lab.verify_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("sha256sum.txt is empty", rendered)
        self.assertNotIn("missing entry", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_slot_files_rejects_alias_slot_path_before_metadata(self) -> None:
        cases = (
            (
                lambda base: base / "slot\\alias",
                "slot path must not contain backslashes",
            ),
            (
                lambda base: base / "slot" / ".." / "alias",
                "slot path must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    slot = path_factory(Path(temp))
                    errors: list[str] = []

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias slot path should fail before metadata"
                        ),
                    ):
                        files = device_lab._slot_files(slot, errors)  # type: ignore[attr-defined]

                self.assertEqual(files, set())
                self.assertEqual(errors, [expected_error])

    def test_verify_sha256_manifest_rejects_symlinked_slot_root_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = create_slot(root / "real", "slot-a")
            linked_slot = root / "linked-slot"
            create_dir_symlink(self, linked_slot, real_slot)

            errors = device_lab.verify_sha256_manifest(linked_slot)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot directory must not be a symlink"])
        self.assertNotIn("digest mismatch", rendered)
        self.assertNotIn("missing entry", rendered)

    def test_verify_sha256_manifest_rejects_slot_metadata_failure_before_parse(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated slot lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.verify_sha256_manifest(slot)
                rendered = "\n".join(errors)
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["slot directory metadata could not be read"])
        self.assertNotIn("digest mismatch", rendered)
        self.assertNotIn("missing entry", rendered)

    def test_verify_sha256_manifest_rejects_symlinked_slot_ancestor_before_discovery(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_root = root / "real-root"
            create_slot(real_root, "slot-a")
            linked_root = root / "linked-root"
            create_dir_symlink(self, linked_root, real_root)

            errors = device_lab.verify_sha256_manifest(linked_root / "slot-a")
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot ancestor directory must not be a symlink"])
        self.assertNotIn("digest mismatch", rendered)
        self.assertNotIn("missing entry", rendered)

    def test_verify_sha256_manifest_missing_slot_returns_missing_manifest_without_traceback(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "missing-slot"

            errors = device_lab.verify_sha256_manifest(slot)

        self.assertEqual(errors, ["missing sha256sum.txt"])

    def test_verify_sha256_manifest_rejects_hardlinked_manifest_before_discovery(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            external_manifest = root / "external-sha256sum.txt"
            write_text(external_manifest, "not-a-manifest-line\n")
            replace_with_hardlink(self, slot / "sha256sum.txt", external_manifest)

            errors = device_lab.verify_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["sha256sum.txt must not be hardlinked"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)
        self.assertNotIn("missing entry", rendered)

    def test_verify_sha256_manifest_rejects_symlinked_artifact_directory_before_digest_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = root / "slot-a"
            slot.mkdir()
            external_logs = root / "external-logs"
            external_log = external_logs / "runtime.log"
            write_text(external_log, "external log\n")
            create_dir_symlink(self, slot / "logs", external_logs)
            external_digest = hashlib.sha256(external_log.read_bytes()).hexdigest()
            write_text(slot / "sha256sum.txt", f"{external_digest}  logs/runtime.log\n")

            errors = device_lab.verify_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(
            errors,
            [
                "sha256sum.txt references artifact under symlink directory "
                "logs/runtime.log"
            ],
        )
        self.assertNotIn("digest mismatch", rendered)

    def test_manifest_artifact_digest_rejects_secret_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            secret_relative = "logs/token=supersecret.log"
            write_text(slot / secret_relative, "must not be hashed\n")

            digest, errors = device_lab._manifest_artifact_sha256(
                slot,
                secret_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot artifacts must not contain secret-looking material"])
        self.assertNotIn(secret_relative, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_manifest_artifact_digest_rejects_control_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            control_relative = "logs/runtime\x1b[31m.log"

            digest, errors = device_lab._manifest_artifact_sha256(
                slot,
                control_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["sha256sum.txt artifact path: unsafe path contains control characters"],
        )
        self.assertNotIn(control_relative, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_manifest_artifact_digest_rejects_symlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = device_lab._manifest_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["sha256sum.txt references symlink artifact logs/runtime.log"],
        )

    def test_manifest_artifact_digest_rejects_hardlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = device_lab._manifest_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["sha256sum.txt references hardlinked artifact logs/runtime.log"],
        )

    def test_manifest_artifact_digest_rejects_oversized_artifact_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            runtime_log = slot / "logs" / "runtime.log"
            with runtime_log.open("wb") as handle:
                handle.seek(device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES)
                handle.write(b"x")

            digest, errors = device_lab._manifest_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "sha256sum.txt references artifact logs/runtime.log "
                "must be no more than "
                f"{device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES} bytes"
            ],
        )

    def test_manifest_artifact_digest_rejects_file_metadata_failure(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                target = slot / "logs" / "runtime.log"

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == target:
                        raise OSError("simulated manifest artifact metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                digest, errors = device_lab._manifest_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "sha256sum.txt references artifact file metadata could not be read "
                "logs/runtime.log"
            ],
        )

    def test_manifest_artifact_digest_uses_lstat_before_relative_ancestor_is_symlink_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_symlink = path_type.is_symlink

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                logs_dir = slot / "logs"
                target = logs_dir / "runtime.log"
                expected_digest = hashlib.sha256(target.read_bytes()).hexdigest()

                def failing_is_symlink(path: Path, *args, **kwargs):
                    if path == logs_dir:
                        raise OSError("simulated relative ancestor is_symlink failure")
                    return original_is_symlink(path, *args, **kwargs)

                path_type.is_symlink = failing_is_symlink

                digest, errors = device_lab._manifest_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
        finally:
            path_type.is_symlink = original_is_symlink

        self.assertEqual(digest, expected_digest)
        self.assertEqual(errors, [])

    def test_manifest_artifact_digest_rejects_read_failure_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            target = slot / "logs" / "runtime.log"

            digest, errors = with_open_failure(
                target,
                lambda: device_lab._manifest_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                ),
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "sha256sum.txt references artifact that could not be read "
                "logs/runtime.log"
            ],
        )

    def test_manifest_artifact_digest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        original_validate = device_lab._validate_manifest_artifact_for_digest

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                artifact_path = slot / "logs" / "runtime.log"
                swapped = False

                def swapping_validate(slot_path: Path, relative: str):
                    nonlocal swapped
                    artifact, artifact_stat, errors = original_validate(
                        slot_path,
                        relative,
                    )
                    if artifact == artifact_path and not errors and not swapped:
                        artifact_path.unlink()
                        write_text(artifact_path, "replacement runtime log\n")
                        swapped = True
                    return artifact, artifact_stat, errors

                device_lab._validate_manifest_artifact_for_digest = swapping_validate

                digest, errors = device_lab._manifest_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
                replacement_bytes = artifact_path.read_bytes()
        finally:
            device_lab._validate_manifest_artifact_for_digest = original_validate

        self.assertTrue(swapped)
        self.assertIsNone(digest)
        self.assertEqual(replacement_bytes, b"replacement runtime log\n")
        self.assertEqual(
            errors,
            [
                "sha256sum.txt references artifact changed while being read "
                "logs/runtime.log"
            ],
        )

    def test_verify_sha256_manifest_revalidates_artifact_before_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            original_parse = device_lab.parse_sha256_manifest

            def parse_then_alias(slot_path: Path):
                entries, errors = original_parse(slot_path)
                if not errors:
                    replace_with_symlink(self, slot_path / "logs" / "runtime.log", target)
                return entries, errors

            try:
                device_lab.parse_sha256_manifest = parse_then_alias
                errors = device_lab.verify_sha256_manifest(slot)
            finally:
                device_lab.parse_sha256_manifest = original_parse

        self.assertIn(
            "sha256sum.txt references symlink artifact logs/runtime.log",
            errors,
        )
        self.assertNotIn("sha256sum.txt digest mismatch for logs/runtime.log", errors)

    def test_attestation_result_rejects_secret_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            attestation_dir = slot / "attestation"
            attestation_dir.mkdir(parents=True)
            write_text(attestation_dir / "result.json", "{not-json")
            errors: list[str] = []

            device_lab.validate_attestation_result(slot, {}, errors)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_d2d_transcript_rejects_secret_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            transcript_path = Path(temp) / "d2d-payment.json"
            write_text(transcript_path, "{not-json")
            errors: list[str] = []

            device_lab.validate_d2d_payment_transcript(
                slot,
                transcript_path,
                {},
                errors,
            )
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_d2d_transcript_binding_rejects_secret_slot_path_directly_before_artifact_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            handoff_dir = slot / "handoff"
            handoff_dir.mkdir(parents=True)
            write_text(handoff_dir / "d2d-payment.json", "{not-json")
            errors: list[str] = []

            relative, digest, transport = device_lab.validate_d2d_payment_transcript_binding(
                slot,
                {
                    "d2d_payment_transcript_path": "handoff/d2d-payment.json",
                    "d2d_payment_transcript_sha256": "00" * 64,
                },
                errors,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(relative)
        self.assertIsNone(digest)
        self.assertIsNone(transport)
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("does not match", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_wallet_transcript_binding_rejects_secret_slot_path_directly_before_artifact_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            wallet_dir = slot / "wallet"
            wallet_dir.mkdir(parents=True)
            write_text(wallet_dir / "integrity.json", "{not-json")
            errors: list[str] = []

            relative, digest = device_lab.validate_wallet_integrity_transcript_binding(
                slot,
                {
                    "wallet_integrity_transcript_path": "wallet/integrity.json",
                    "wallet_integrity_transcript_sha256": "00" * 64,
                },
                errors,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(relative)
        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("does not match", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_d2d_transcript_binding_rejects_symlink_path_before_digest_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            target = root / "outside-d2d-payment.json"
            write_text(target, (slot / "handoff" / "d2d-payment.json").read_text())
            replace_with_symlink(self, slot / "handoff" / "d2d-payment.json", target)
            errors: list[str] = []

            relative, digest, transport = device_lab.validate_d2d_payment_transcript_binding(
                slot,
                metadata,
                errors,
            )

        self.assertEqual(relative, "handoff/d2d-payment.json")
        self.assertIsNone(digest)
        self.assertIsNone(transport)
        self.assertIn(
            "slot.json d2d_payment_transcript_path references symlink artifact "
            "handoff/d2d-payment.json",
            errors,
        )
        self.assertNotIn(
            "slot.json d2d_payment_transcript_sha256 does not match "
            "d2d_payment_transcript_path",
            errors,
        )

    def test_wallet_transcript_binding_rejects_hardlink_path_before_digest_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            target = root / "outside-integrity.json"
            write_text(target, (slot / "wallet" / "integrity.json").read_text())
            replace_with_hardlink(self, slot / "wallet" / "integrity.json", target)
            errors: list[str] = []

            relative, digest = device_lab.validate_wallet_integrity_transcript_binding(
                slot,
                metadata,
                errors,
            )

        self.assertEqual(relative, "wallet/integrity.json")
        self.assertIsNone(digest)
        self.assertIn(
            "slot.json wallet_integrity_transcript_path references hardlinked "
            "artifact wallet/integrity.json",
            errors,
        )
        self.assertNotIn(
            "slot.json wallet_integrity_transcript_sha256 does not match "
            "wallet_integrity_transcript_path",
            errors,
        )

    def test_d2d_transcript_rejects_symlinked_queue_before_digest_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            target = root / "outside-pending-queue.json"
            write_text(target, (slot / "queue" / "pending_queue.json").read_text())
            replace_with_symlink(self, slot / "queue" / "pending_queue.json", target)
            errors: list[str] = []

            device_lab.validate_d2d_payment_transcript(
                slot,
                slot / "handoff" / "d2d-payment.json",
                metadata,
                errors,
            )

        self.assertIn(
            "d2d payment transcript queue_after_sha256 references symlink "
            "artifact queue/pending_queue.json",
            errors,
        )
        self.assertNotIn(
            "d2d payment transcript queue_after_sha256 must match "
            "queue/pending_queue.json",
            errors,
        )

    def test_d2d_transcript_uses_lstat_before_queue_is_file_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            queue_path = slot / "queue" / "pending_queue.json"

            def failing_is_file(path: Path, *args, **kwargs):
                if path == queue_path:
                    raise OSError("simulated queue is_file failure")
                return original_is_file(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == queue_path:
                    raise OSError("simulated queue lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_d2d_payment_transcript(
                    slot,
                    slot / "handoff" / "d2d-payment.json",
                    metadata,
                    errors,
                )
            finally:
                path_type.is_file = original_is_file
                path_type.lstat = original_lstat

        self.assertIn(
            "d2d payment transcript queue_after_sha256 references artifact file "
            "metadata could not be read queue/pending_queue.json",
            errors,
        )

    def test_required_artifact_shapes_rejects_secret_slot_path_directly_before_stat(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            log_dir = slot / "logs"
            log_dir.mkdir(parents=True)
            write_text(log_dir / "runtime.log", "")
            errors: list[str] = []

            device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("must be non-empty", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_required_artifact_shapes_rejects_oversized_artifact_directly(
        self,
    ) -> None:
        old_limit = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
        try:
            device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 8
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "pixel8")
                write_text(slot / "logs" / "runtime.log", "runtime log too large\n")
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
        finally:
            device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = old_limit

        self.assertIn(
            "required slot artifact logs/runtime.log must be no more than 8 bytes",
            errors,
        )

    def test_required_status_artifact_rejects_symlink_before_text_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "pixel8")
            target = root / "outside-status.ndjson"
            write_text(target, '{"status":"failed"}\n')
            replace_with_symlink(self, slot / "telemetry" / "status.ndjson", target)
            errors: list[str] = []

            device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)

        self.assertIn(
            "telemetry/status.ndjson references symlink artifact "
            "telemetry/status.ndjson",
            errors,
        )
        self.assertNotIn(
            "telemetry/status.ndjson line 1 status must not be 'failed'",
            errors,
        )

    def test_required_runtime_log_rejects_hardlink_before_text_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "pixel8")
            target = root / "outside-runtime.log"
            write_text(target, "missing completion marker\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)
            errors: list[str] = []

            device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)

        self.assertIn(
            "logs/runtime.log references hardlinked artifact logs/runtime.log",
            errors,
        )
        self.assertNotIn(
            "logs/runtime.log must contain Kagemusha device-lab completion marker",
            errors,
        )

    def test_required_runtime_log_rejects_symlink_swap_after_preflight(self) -> None:
        original_validate = device_lab._validate_metadata_artifact_for_read

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                slot = create_slot(root, "pixel8")
                runtime_log = slot / "logs" / "runtime.log"
                target = root / "outside-runtime.log"
                write_text(target, "TEST FAILED\nmissing completion marker\n")
                swapped = False

                def swapping_validate(
                    slot_path: Path,
                    relative: str,
                    label: str,
                    missing_error: str,
                ):
                    nonlocal swapped
                    artifact, artifact_stat, validate_errors = original_validate(
                        slot_path,
                        relative,
                        label,
                        missing_error,
                    )
                    if artifact == runtime_log and not validate_errors and not swapped:
                        replace_with_symlink(self, runtime_log, target)
                        swapped = True
                    return artifact, artifact_stat, validate_errors

                device_lab._validate_metadata_artifact_for_read = swapping_validate
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
                target_text = target.read_text(encoding="utf-8")
        finally:
            device_lab._validate_metadata_artifact_for_read = original_validate

        self.assertTrue(swapped)
        self.assertEqual(target_text, "TEST FAILED\nmissing completion marker\n")
        self.assertIn(
            "logs/runtime.log references symlink artifact logs/runtime.log",
            errors,
        )
        self.assertNotIn("logs/runtime.log contains failure marker TEST FAILED", errors)
        self.assertNotIn(
            "logs/runtime.log must contain Kagemusha device-lab completion marker",
            errors,
        )

    def test_required_status_artifact_uses_lstat_before_is_file_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            status_path = slot / "telemetry" / "status.ndjson"

            def failing_is_file(path: Path, *args, **kwargs):
                if path == status_path:
                    raise OSError("simulated status is_file failure")
                return original_is_file(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == status_path:
                    raise OSError("simulated status lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
            finally:
                path_type.is_file = original_is_file
                path_type.lstat = original_lstat

        self.assertIn("telemetry/status.ndjson file metadata could not be read", errors)

    def test_required_runtime_log_uses_lstat_before_is_file_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            runtime_log = slot / "logs" / "runtime.log"

            def failing_is_file(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log is_file failure")
                return original_is_file(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
            finally:
                path_type.is_file = original_is_file
                path_type.lstat = original_lstat

        self.assertIn("logs/runtime.log file metadata could not be read", errors)

    def test_slot_symlink_artifact_validator_rejects_secret_slot_path_directly_before_traversal(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            log_dir = slot / "logs"
            log_dir.mkdir(parents=True)
            target = Path(temp) / "outside-runtime.log"
            write_text(target, "outside\n")
            write_text(log_dir / "runtime.log", "placeholder\n")
            replace_with_symlink(self, log_dir / "runtime.log", target)
            errors: list[str] = []

            device_lab.validate_no_slot_symlink_artifacts(slot, errors)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("must not be a symlink", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_slot_symlink_artifact_validator_reports_slot_metadata_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            slot_metadata_path = slot / "slot.json"
            write_text(slot_metadata_path, "{}\n")

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot_metadata_path:
                    raise OSError("simulated slot metadata lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_no_slot_symlink_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn("slot.json file metadata could not be read", errors)

    def test_slot_symlink_artifact_validator_reports_directory_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            logs_dir = slot / "logs"

            def failing_lstat(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs directory lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_no_slot_symlink_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn("logs/ metadata could not be read", errors)

    def test_slot_symlink_artifact_validator_reports_nested_artifact_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            runtime_log = slot / "logs" / "runtime.log"

            def failing_lstat(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_no_slot_symlink_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn(
            "slot artifact logs/runtime.log file metadata could not be read",
            errors,
        )

    def test_slot_hardlink_artifact_validator_rejects_secret_slot_path_directly_before_stat(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            log_dir = slot / "logs"
            log_dir.mkdir(parents=True)
            target = Path(temp) / "outside-runtime.log"
            write_text(target, "outside\n")
            write_text(log_dir / "runtime.log", "placeholder\n")
            replace_with_hardlink(self, log_dir / "runtime.log", target)
            errors: list[str] = []

            device_lab.validate_no_slot_hardlink_artifacts(slot, errors)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("must not be hardlinked", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_slot_hardlink_artifact_validator_reports_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "pixel8")
                artifact = slot / "logs" / "runtime.log"
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == artifact:
                        raise OSError("simulated hardlink validator metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                device_lab.validate_no_slot_hardlink_artifacts(slot, errors)
        finally:
            path_type.lstat = original_lstat

        self.assertIn(
            "slot artifact logs/runtime.log file metadata could not be read",
            errors,
        )

    def test_slot_hardlink_artifact_validator_uses_lstat_before_directory_exists_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_exists = path_type.exists
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            logs_dir = slot / "logs"

            def failing_exists(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs exists failure")
                return original_exists(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.exists = failing_exists
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_no_slot_hardlink_artifacts(slot, errors)
            finally:
                path_type.exists = original_exists
                path_type.lstat = original_lstat

        self.assertIn("logs/ metadata could not be read", errors)

    def test_slot_regular_artifact_validator_rejects_secret_slot_path_directly_before_shape(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            log_dir = slot / "logs"
            log_dir.mkdir(parents=True)
            directory_artifact = log_dir / "runtime.log"
            directory_artifact.mkdir()
            errors: list[str] = []

            device_lab.validate_slot_regular_file_artifacts(slot, errors)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("must be a regular file", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_slot_regular_artifact_validator_reports_slot_metadata_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            slot_metadata_path = slot / "slot.json"
            write_text(slot_metadata_path, "{}\n")

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot_metadata_path:
                    raise OSError("simulated slot metadata lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn("slot.json file metadata could not be read", errors)

    def test_slot_regular_artifact_validator_uses_lstat_before_exists_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_exists = path_type.exists
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            slot_metadata_path = slot / "slot.json"
            write_text(slot_metadata_path, "{}\n")

            def failing_exists(path: Path, *args, **kwargs):
                if path == slot_metadata_path:
                    raise OSError("simulated slot metadata exists failure")
                return original_exists(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot_metadata_path:
                    raise OSError("simulated slot metadata lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.exists = failing_exists
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.exists = original_exists
                path_type.lstat = original_lstat

        self.assertIn("slot.json file metadata could not be read", errors)

    def test_slot_regular_artifact_validator_reports_directory_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            logs_dir = slot / "logs"

            def failing_lstat(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs directory lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn("logs/ metadata could not be read", errors)

    def test_slot_regular_artifact_validator_uses_lstat_before_directory_exists_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_exists = path_type.exists
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            logs_dir = slot / "logs"

            def failing_exists(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs exists failure")
                return original_exists(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.exists = failing_exists
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.exists = original_exists
                path_type.lstat = original_lstat

        self.assertIn("logs/ metadata could not be read", errors)

    def test_slot_regular_artifact_validator_reports_nested_artifact_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            runtime_log = slot / "logs" / "runtime.log"

            def failing_lstat(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn(
            "slot artifact logs/runtime.log file metadata could not be read",
            errors,
        )

    def test_slot_regular_artifact_validator_uses_lstat_before_nested_symlink_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_symlink = path_type.is_symlink
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            runtime_log = slot / "logs" / "runtime.log"

            def failing_is_symlink(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log symlink preflight failure")
                return original_is_symlink(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_symlink = failing_is_symlink
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.is_symlink = original_is_symlink
                path_type.lstat = original_lstat

        self.assertIn(
            "slot artifact logs/runtime.log file metadata could not be read",
            errors,
        )

    def test_required_artifact_shapes_reports_required_artifact_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            status_path = slot / "telemetry" / "status.ndjson"

            def failing_stat(path: Path, *args, **kwargs):
                if path == status_path:
                    raise OSError("simulated required artifact stat failure")
                return original_stat(path, *args, **kwargs)

            try:
                path_type.stat = failing_stat
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
            finally:
                path_type.stat = original_stat

        self.assertIn(
            "required slot artifact metadata could not be read telemetry/status.ndjson",
            errors,
        )

    def test_required_artifact_shapes_uses_lstat_before_is_file_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            queue_path = slot / "queue" / "pending_queue.json"

            def failing_is_file(path: Path, *args, **kwargs):
                if path == queue_path:
                    raise OSError("simulated required artifact is_file failure")
                return original_is_file(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == queue_path:
                    raise OSError("simulated required artifact lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
            finally:
                path_type.is_file = original_is_file
                path_type.lstat = original_lstat

        self.assertIn(
            "required slot artifact metadata could not be read queue/pending_queue.json",
            errors,
        )

    def test_signed_evidence_artifact_rejects_secret_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            evidence_path = Path(temp) / "signed-evidence.json"
            write_text(evidence_path, "{not-json")
            errors: list[str] = []

            details = device_lab.validate_signed_evidence_artifact(
                slot,
                evidence_path,
                {},
                {},
                errors,
            )
            rendered = "\n".join(errors)

        self.assertEqual(details, {})
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_kagemusha_production_metadata_rejects_secret_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            slot.mkdir()
            write_text(slot / "slot.json", "{not-json")

            errors, details = device_lab.validate_kagemusha_production_metadata(slot)
            rendered = "\n".join(errors)

        self.assertEqual(details, {})
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_json_summary_redacts_secret_looking_unlisted_artifact_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            summary_path = root / "summary.json"
            slot = create_slot(root, "slot-a")
            secret_path = "logs/token=supersecret.log"
            write_text(slot / secret_path, "must not leak\n")

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary_text = summary_path.read_text(encoding="utf-8")
            stderr_text = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertNotIn(str(root), summary_text)
        self.assertNotIn(secret_path, stderr_text)
        self.assertNotIn(secret_path, summary_text)
        self.assertIn(device_lab.DEVICE_LAB_ROOT_SUMMARY_LABEL, summary_text)
        self.assertIn(device_lab.SECRET_PATH_REDACTION, stderr_text)
        self.assertIn(device_lab.SECRET_PATH_REDACTION, summary_text)

    def test_production_json_summary_redacts_secret_looking_required_artifact_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            summary_path = root / "summary.json"
            signer = create_test_signer(root / "keys")
            secret_path = "logs/token=supersecret.log"
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / secret_path, "must not leak\n")
            rewrite_sha256sum(slot)

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary_text = summary_path.read_text(encoding="utf-8")
            stdout_text = stdout.getvalue()
            rendered = stdout_text + stderr.getvalue() + summary_text

        self.assertEqual(status, 1)
        self.assertNotIn(str(root), summary_text)
        self.assertNotIn(str(summary_path), stdout_text)
        self.assertNotIn(secret_path, rendered)
        self.assertIn(device_lab.DEVICE_LAB_ROOT_SUMMARY_LABEL, summary_text)
        self.assertIn(device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertIn(
            f"signed evidence artifact artifact_digests[{device_lab.SECRET_PATH_REDACTION}] must be lowercase sha256 hex",
            rendered,
        )

    def test_production_metadata_rejects_duplicate_slot_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            inject_duplicate_json_key(slot / "slot.json", "schema", "shadow")
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json contains duplicate JSON object key schema",
            report["errors"],
        )

    def test_production_metadata_redacts_secret_duplicate_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "slot.json",
                '{"token=supersecret": 1, "token=supersecret": 2}\n',
            )
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )
            rendered = "\n".join(report["errors"])

        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"slot.json contains duplicate JSON object key {device_lab.SECRET_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_production_metadata_redacts_control_duplicate_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "slot.json",
                '{"debug\\u001b[31m": 1, "debug\\u001b[31m": 2}\n',
            )
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )
            rendered = "\n".join(report["errors"])

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json contains duplicate JSON object key "
            f"{device_lab.CONTROL_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("debug\x1b[31m", rendered)
        self.assertNotIn("\x1b", rendered)

    def test_production_metadata_rejects_duplicate_attestation_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            inject_duplicate_json_key(slot / "attestation" / "result.json", "slot", "shadow")
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json contains duplicate JSON object key slot",
            report["errors"],
        )

    def test_production_metadata_rejects_duplicate_signed_evidence_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            inject_duplicate_json_key(
                slot / "evidence" / "signed-evidence.json",
                "schema",
                "shadow",
            )
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact contains duplicate JSON object key schema",
            report["errors"],
        )

    def test_production_metadata_rejects_duplicate_d2d_transcript_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            inject_duplicate_json_key(slot / "handoff" / "d2d-payment.json", "schema", "shadow")
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript contains duplicate JSON object key schema",
            report["errors"],
        )

    def test_production_metadata_rejects_duplicate_wallet_integrity_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            inject_duplicate_json_key(slot / "wallet" / "integrity.json", "schema", "shadow")
            refresh_wallet_integrity_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "wallet integrity transcript contains duplicate JSON object key schema",
            report["errors"],
        )

    def test_production_metadata_rejects_unavailable_recursive_spend_one_hop_probe(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["kagemusha_recursive_spend_jni_probe"] = "unavailable"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json kagemusha_recursive_spend_jni_probe must be one of ['recursive_spend_verified']",
            report["errors"],
        )

    def test_production_metadata_rejects_generic_recursive_spend_prover_state(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["kagemusha_recursive_spend_prover_state"] = (
                "multi_hop_proof_composition_unavailable"
            )
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json kagemusha_recursive_spend_prover_state must be one of ['multi_hop_proof_composed']",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_ffi_surface_status(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["kagemusha_recursive_spend_ffi_surface"] = "ok"
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["kagemusha_recursive_spend_ffi_surface"] = "ok"
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json kagemusha_recursive_spend_ffi_surface must be one of ['passed']",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_probe_states(self) -> None:
        cases = (
            (
                "kagemusha_recursive_spend_ffi_surface",
                " passed ",
                "slot.json kagemusha_recursive_spend_ffi_surface must not contain surrounding whitespace",
            ),
            (
                "kagemusha_recursive_spend_ffi_surface",
                "PASSED",
                "slot.json kagemusha_recursive_spend_ffi_surface must be lowercase",
            ),
            (
                "kagemusha_recursive_spend_jni_probe",
                "recursive_spend_verified\u0000",
                "slot.json kagemusha_recursive_spend_jni_probe must not contain control characters",
            ),
            (
                "kagemusha_recursive_spend_jni_probe",
                "",
                "slot.json kagemusha_recursive_spend_jni_probe must be a non-empty string",
            ),
            (
                "kagemusha_recursive_spend_prover_state",
                7,
                "slot.json kagemusha_recursive_spend_prover_state must be a non-empty string",
            ),
        )
        for field, value, expected_error in cases:
            with self.subTest(field=field, expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    metadata_path = slot / "slot.json"
                    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
                    metadata[field] = value
                    write_json(metadata_path, metadata)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_signed_evidence_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_sha256"] = "01" * 32
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_sha256 does not match signed_evidence_artifact_path",
            report["errors"],
        )

    def test_production_metadata_rejects_zero_sha256_placeholders(self) -> None:
        fields = (
            "app_signing_certificate_sha256",
            "attestation_challenge_sha256",
            "attestation_certificate_chain_sha256",
            "kagemusha_wallet_policy_sha256",
            "kagemusha_wallet_apk_sha256",
            "d2d_payment_transcript_sha256",
            "wallet_integrity_transcript_sha256",
            "signed_evidence_artifact_sha256",
        )
        for field in fields:
            with self.subTest(field=field):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    metadata_path = slot / "slot.json"
                    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
                    metadata[field] = "0" * 64
                    write_json(metadata_path, metadata)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(
                    f"slot.json {field} must be non-zero lowercase sha256 hex",
                    report["errors"],
                )

    def test_production_metadata_uses_lstat_before_signed_evidence_is_file_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            evidence_path = slot / "evidence" / "signed-evidence.json"

            def failing_is_file(path: Path, *args, **kwargs):
                if path == evidence_path:
                    raise OSError("simulated signed evidence is_file failure")
                return original_is_file(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == evidence_path:
                    raise OSError("simulated signed evidence lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file
                path_type.lstat = failing_lstat

                errors, _details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted,
                )
            finally:
                path_type.is_file = original_is_file
                path_type.lstat = original_lstat

        self.assertIn(
            "slot.json signed_evidence_artifact_path references artifact file "
            "metadata could not be read evidence/signed-evidence.json",
            errors,
        )

    def test_metadata_artifact_digest_rejects_secret_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            secret_relative = "evidence/token=supersecret.apk"
            write_text(slot / secret_relative, "must not be hashed\n")

            payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                slot,
                secret_relative,
                "slot.json kagemusha_wallet_apk_path",
                "slot.json kagemusha_wallet_apk_path must point to an existing file",
            )
            rendered = "\n".join(errors)

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["slot.json kagemusha_wallet_apk_path must not contain secret-looking material"],
        )
        self.assertNotIn(secret_relative, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_metadata_artifact_digest_rejects_control_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            control_relative = "evidence/kagemusha-wallet\x1b[31m.apk"

            payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                slot,
                control_relative,
                "slot.json kagemusha_wallet_apk_path",
                "slot.json kagemusha_wallet_apk_path must point to an existing file",
            )
            rendered = "\n".join(errors)

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "slot.json kagemusha_wallet_apk_path: unsafe path contains "
                "control characters"
            ],
        )
        self.assertNotIn(control_relative, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_metadata_artifact_digest_rejects_file_metadata_failure(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                target = slot / "evidence" / "kagemusha-wallet-release.apk"

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == target:
                        raise OSError("simulated metadata artifact metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/kagemusha-wallet-release.apk",
                    "slot.json kagemusha_wallet_apk_path",
                    "slot.json kagemusha_wallet_apk_path must point to an existing file",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "slot.json kagemusha_wallet_apk_path references artifact file metadata "
                "could not be read evidence/kagemusha-wallet-release.apk"
            ],
        )

    def test_metadata_artifact_digest_rejects_oversized_artifact_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            apk_path = slot / "evidence" / "kagemusha-wallet-release.apk"
            with apk_path.open("wb") as handle:
                handle.seek(device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES)
                handle.write(b"x")

            payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                slot,
                "evidence/kagemusha-wallet-release.apk",
                "slot.json kagemusha_wallet_apk_path",
                "slot.json kagemusha_wallet_apk_path must point to an existing file",
            )

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "slot.json kagemusha_wallet_apk_path references artifact "
                "evidence/kagemusha-wallet-release.apk must be no more than "
                f"{device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES} bytes"
            ],
        )

    def test_metadata_artifact_digest_uses_release_apk_specific_limit(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            apk_path = slot / "evidence" / "kagemusha-wallet-release.apk"
            apk_path.write_bytes(b"x" * 16)
            old_base_limit = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
            old_apk_limit = device_lab.MAX_KAGEMUSHA_WALLET_APK_BYTES
            try:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 8
                device_lab.MAX_KAGEMUSHA_WALLET_APK_BYTES = 32
                payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/kagemusha-wallet-release.apk",
                    "slot.json kagemusha_wallet_apk_path",
                    "slot.json kagemusha_wallet_apk_path must point to an existing file",
                    device_lab._slot_artifact_max_bytes("evidence/kagemusha-wallet-release.apk"),
                )
            finally:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = old_base_limit
                device_lab.MAX_KAGEMUSHA_WALLET_APK_BYTES = old_apk_limit

        self.assertEqual(errors, [])
        self.assertEqual(payload, b"x" * 16)
        self.assertEqual(digest, hashlib.sha256(b"x" * 16).hexdigest())

    def test_metadata_artifact_digest_rejects_read_failure_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            target = slot / "evidence" / "kagemusha-wallet-release.apk"

            payload, digest, errors = with_open_failure(
                target,
                lambda: device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/kagemusha-wallet-release.apk",
                    "slot.json kagemusha_wallet_apk_path",
                    "slot.json kagemusha_wallet_apk_path must point to an existing file",
                ),
            )

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot.json kagemusha_wallet_apk_path could not be read"])

    def test_metadata_artifact_digest_rejects_symlink_swap_after_preflight(
        self,
    ) -> None:
        original_validate = device_lab._validate_metadata_artifact_for_read

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                slot = create_slot(root, "slot-a")
                artifact_path = slot / "evidence" / "kagemusha-wallet-release.apk"
                target = root / "outside-release.apk"
                write_text(target, "outside release apk\n")
                swapped = False

                def swapping_validate(
                    slot_path: Path,
                    relative: str,
                    label: str,
                    missing_error: str,
                ):
                    nonlocal swapped
                    artifact, artifact_stat, errors = original_validate(
                        slot_path,
                        relative,
                        label,
                        missing_error,
                    )
                    if artifact == artifact_path and not errors and not swapped:
                        replace_with_symlink(self, artifact_path, target)
                        swapped = True
                    return artifact, artifact_stat, errors

                device_lab._validate_metadata_artifact_for_read = swapping_validate

                payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/kagemusha-wallet-release.apk",
                    "slot.json kagemusha_wallet_apk_path",
                    "slot.json kagemusha_wallet_apk_path must point to an existing file",
                )
                target_bytes = target.read_bytes()
        finally:
            device_lab._validate_metadata_artifact_for_read = original_validate

        self.assertTrue(swapped)
        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(target_bytes, b"outside release apk\n")
        self.assertEqual(
            errors,
            [
                "slot.json kagemusha_wallet_apk_path references symlink artifact "
                "evidence/kagemusha-wallet-release.apk"
            ],
        )

    def test_metadata_artifact_digest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        original_validate = device_lab._validate_metadata_artifact_for_read

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                slot = create_slot(root, "slot-a")
                artifact_path = slot / "evidence" / "kagemusha-wallet-release.apk"
                swapped = False

                def swapping_validate(
                    slot_path: Path,
                    relative: str,
                    label: str,
                    missing_error: str,
                ):
                    nonlocal swapped
                    artifact, artifact_stat, errors = original_validate(
                        slot_path,
                        relative,
                        label,
                        missing_error,
                    )
                    if artifact == artifact_path and not errors and not swapped:
                        artifact_path.unlink()
                        write_text(artifact_path, "replacement release apk\n")
                        swapped = True
                    return artifact, artifact_stat, errors

                device_lab._validate_metadata_artifact_for_read = swapping_validate

                payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/kagemusha-wallet-release.apk",
                    "slot.json kagemusha_wallet_apk_path",
                    "slot.json kagemusha_wallet_apk_path must point to an existing file",
                )
                replacement_bytes = artifact_path.read_bytes()
        finally:
            device_lab._validate_metadata_artifact_for_read = original_validate

        self.assertTrue(swapped)
        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(replacement_bytes, b"replacement release apk\n")
        self.assertEqual(
            errors,
            [
                "slot.json kagemusha_wallet_apk_path references artifact changed "
                "while being read evidence/kagemusha-wallet-release.apk"
            ],
        )

    def test_production_metadata_rejects_symlinked_signed_evidence_digest_path(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            target = root / "outside-signed-evidence.json"
            write_text(target, (slot / "evidence" / "signed-evidence.json").read_text())
            replace_with_symlink(
                self,
                slot / "evidence" / "signed-evidence.json",
                target,
            )

            errors, _details = device_lab.validate_kagemusha_production_metadata(
                slot,
                trusted,
            )

        self.assertIn(
            "slot.json signed_evidence_artifact_path references symlink artifact "
            "evidence/signed-evidence.json",
            errors,
        )

    def test_production_metadata_rejects_hardlinked_release_apk_digest_path(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            target = root / "outside-release.apk"
            write_text(target, "apk bytes\n")
            replace_with_hardlink(
                self,
                slot / "evidence" / "kagemusha-wallet-release.apk",
                target,
            )

            errors, _details = device_lab.validate_kagemusha_production_metadata(
                slot,
                trusted,
            )

        self.assertIn(
            "slot.json kagemusha_wallet_apk_path references hardlinked artifact "
            "evidence/kagemusha-wallet-release.apk",
            errors,
        )

    def test_production_metadata_rejects_unsafe_signed_evidence_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = "../signed-evidence.json"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_path: unsafe path '../signed-evidence.json'",
            report["errors"],
        )

    def test_production_metadata_rejects_star_normalized_signed_evidence_path(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = "*evidence/signed-evidence.json"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            (
                "slot.json signed_evidence_artifact_path: unsafe path "
                "'*evidence/signed-evidence.json'"
            ),
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_signed_evidence_path(
        self,
    ) -> None:
        cases = (
            "evidence/./signed-evidence.json",
            "evidence//signed-evidence.json",
            "evidence/signed-evidence.json/",
        )
        for relative in cases:
            with self.subTest(relative=relative):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    metadata_path = slot / "slot.json"
                    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
                    metadata["signed_evidence_artifact_path"] = relative
                    write_json(metadata_path, metadata)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(
                    "slot.json signed_evidence_artifact_path: unsafe path is not canonical",
                    report["errors"],
                )

    def test_production_metadata_rejects_whitespace_normalized_signed_evidence_path(
        self,
    ) -> None:
        cases = (
            " evidence/signed-evidence.json ",
            "evidence/ signed-evidence.json",
        )
        for relative in cases:
            with self.subTest(relative=relative):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    metadata_path = slot / "slot.json"
                    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
                    metadata["signed_evidence_artifact_path"] = relative
                    write_json(metadata_path, metadata)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(
                    "slot.json signed_evidence_artifact_path must not contain surrounding whitespace",
                    report["errors"],
                )

    def test_production_metadata_rejects_control_signed_evidence_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = (
                "evidence/signed-evidence.json\x1b[31m"
            )
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_path must not contain control characters",
            report["errors"],
        )
        self.assertNotIn("\x1b", "\n".join(report["errors"]))

    def test_production_metadata_rejects_whitespace_normalized_signed_evidence_digest(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_sha256"] = (
                f" {metadata['signed_evidence_artifact_sha256']} "
            )
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_sha256 must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_artifact_outside_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            source = slot / "evidence" / "signed-evidence.json"
            target = slot / "telemetry" / "signed-evidence.json"
            target.write_bytes(source.read_bytes())

            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = "telemetry/signed-evidence.json"
            metadata["signed_evidence_artifact_sha256"] = hashlib.sha256(
                target.read_bytes()
            ).hexdigest()
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_path must stay under evidence/",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_signed_evidence_filename(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            source = slot / "evidence" / "signed-evidence.json"
            target = slot / "evidence" / "signed-evidence-copy.json"
            target.write_bytes(source.read_bytes())

            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = "evidence/signed-evidence-copy.json"
            metadata["signed_evidence_artifact_sha256"] = hashlib.sha256(
                target.read_bytes()
            ).hexdigest()
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_path must be evidence/signed-evidence.json",
            report["errors"],
        )

    def test_production_metadata_rejects_unexpected_slot_fields_with_redaction(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["token=supersecret"] = "must not ship"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        rendered = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"slot.json contains unexpected field {device_lab.SECRET_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_production_metadata_redacts_control_unexpected_slot_field(self) -> None:
        unsafe_key = "debug\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata[unsafe_key] = "must not ship"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        rendered = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json contains unexpected field "
            f"{device_lab.CONTROL_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn(unsafe_key, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_production_metadata_rejects_unexpected_attestation_fields_with_redaction(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["token=supersecret"] = "must not ship"
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        rendered = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"attestation/result.json contains unexpected field {device_lab.SECRET_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_production_metadata_rejects_noncanonical_attestation_sha(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["kagemusha_wallet_policy_sha256"] = "AA" * 32
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json kagemusha_wallet_policy_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_virtual_device_attestation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["physical_device_attestation"] = False
            write_json(metadata_path, metadata)
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["physical_device_attestation"] = False
            write_json(attestation_path, attestation)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["physical_device_attestation"] = False
            evidence["artifact_digests"] = required_artifact_digests(slot)
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn("slot.json physical_device_attestation must be true", report["errors"])
        self.assertIn(
            "attestation/result.json physical_device_attestation must be true",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact physical_device_attestation must be true",
            report["errors"],
        )

    def test_production_metadata_rejects_wrong_minimum_os_for_device_family(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel6",
                "Google Pixel 6 / 6a",
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["minimum_os"] = "Android 15"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json minimum_os for Google Pixel 6 / 6a must be Android 14",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_attestation_challenge_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("attestation_challenge_sha256")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json attestation_challenge_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_attestation_chain_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("attestation_certificate_chain_sha256")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json attestation_certificate_chain_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_chain_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "attestation" / "keymint-certificate-chain.pem",
                "tampered certificate chain\n",
            )
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json attestation_certificate_chain_sha256 does not match attestation_certificate_chain_path",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_chain_summary_file_substitution(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "attestation" / "chain-summary.txt", "summary only\n")
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["attestation_certificate_chain_path"] = "attestation/chain-summary.txt"
            write_json(metadata_path, metadata)
            refresh_attestation_certificate_chain_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json attestation_certificate_chain_path must end in .pem or .der",
            report["errors"],
        )

    def test_production_metadata_rejects_malformed_attestation_chain_pem(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "attestation" / "keymint-certificate-chain.pem",
                "not a pem certificate chain\n",
            )
            refresh_attestation_certificate_chain_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation certificate chain PEM must contain certificate boundaries",
            report["errors"],
        )

    def test_production_metadata_rejects_oversized_attestation_chain(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "attestation" / "keymint-certificate-chain.pem",
                "-----BEGIN CERTIFICATE-----\n"
                + ("A" * device_lab.MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES)
                + "\n-----END CERTIFICATE-----\n",
            )
            refresh_attestation_certificate_chain_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation certificate chain must be no more than "
            f"{device_lab.MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES} bytes",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_release_apk_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("kagemusha_wallet_apk_sha256")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json kagemusha_wallet_apk_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_release_apk_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "evidence" / "kagemusha-wallet-release.apk", "tampered apk\n")
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json kagemusha_wallet_apk_sha256 does not match kagemusha_wallet_apk_path",
            report["errors"],
        )

    def test_production_metadata_rejects_release_apk_path_outside_evidence(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            replacement_path = "logs/runtime.log"
            replacement_digest = hashlib.sha256(
                (slot / replacement_path).read_bytes()
            ).hexdigest()

            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["kagemusha_wallet_apk_path"] = replacement_path
            metadata["kagemusha_wallet_apk_sha256"] = replacement_digest
            write_json(metadata_path, metadata)

            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["kagemusha_wallet_apk_path"] = replacement_path
            evidence["kagemusha_wallet_apk_sha256"] = replacement_digest
            evidence["artifact_digests"] = required_artifact_digests(slot)
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json kagemusha_wallet_apk_path must stay under evidence/",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_d2d_payment_transcript_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("d2d_payment_transcript_sha256")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json d2d_payment_transcript_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["payload_bytes"] = 4096
            write_json(transcript_path, transcript)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json d2d_payment_transcript_sha256 does not match d2d_payment_transcript_path",
            report["errors"],
        )

    def test_production_metadata_accepts_multi_transport_d2d_transcripts(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
                d2d_payment_transport=transports[0],
                d2d_payment_transports=transports,
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "ok")
        self.assertEqual(report["kagemusha"]["d2d_payment_transports"], list(transports))
        self.assertEqual(
            sorted(report["kagemusha"]["d2d_payment_transcripts"]),
            list(transports),
        )

    def test_production_metadata_rejects_multi_transport_d2d_transcript_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
                d2d_payment_transport=transports[0],
                d2d_payment_transports=transports,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            qr_entry = metadata["d2d_payment_transcripts"]["qr"]
            nfc_entry = metadata["d2d_payment_transcripts"]["nfc_hce"]
            qr_entry["path"] = nfc_entry["path"]
            qr_entry["sha256"] = nfc_entry["sha256"]
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["d2d_payment_transcripts"] = metadata["d2d_payment_transcripts"]
            evidence["artifact_digests"] = required_artifact_digests(slot)
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json d2d_payment_transcripts[qr] transport must match transcript transport",
            report["errors"],
        )

    def test_d2d_transcript_binding_rejects_reused_digest_directly(self) -> None:
        transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
        metadata = {
            device_lab.D2D_PAYMENT_TRANSCRIPTS_FIELD: {
                transport: {
                    "path": f"handoff/d2d-payment-{transport}.json",
                    "sha256": hashlib.sha256(transport.encode("utf-8")).hexdigest(),
                }
                for transport in transports
            }
        }
        original_validator = device_lab._validate_d2d_payment_transcript_entry  # type: ignore[attr-defined]

        def reused_digest_validator(
            _slot_path: Path,
            _metadata: dict[str, object],
            transport: str,
            _entry: object,
            _errors: list[str],
        ) -> tuple[str, dict[str, str]]:
            return (
                transport,
                {
                    "path": f"handoff/d2d-payment-{transport}.json",
                    "sha256": "a" * 64,
                },
            )

        try:
            device_lab._validate_d2d_payment_transcript_entry = reused_digest_validator  # type: ignore[attr-defined]
            errors: list[str] = []
            bindings = device_lab.validate_d2d_payment_transcripts_binding(
                Path("slot-0"),
                metadata,
                errors,
                primary_relative=None,
                primary_digest=None,
                primary_transport=None,
            )
        finally:
            device_lab._validate_d2d_payment_transcript_entry = original_validator  # type: ignore[attr-defined]

        self.assertEqual(bindings, {transports[0]: {"path": f"handoff/d2d-payment-{transports[0]}.json", "sha256": "a" * 64}})
        self.assertIn(
            "slot.json d2d_payment_transcripts must not reuse sha256 digests for multiple transports",
            errors,
        )

    def test_d2d_transcript_binding_rejects_primary_binding_reuse_directly(
        self,
    ) -> None:
        transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
        primary_transport = transports[0]
        replay_transport = transports[-1]
        primary_path = "handoff/d2d-payment.json"
        primary_digest = "b" * 64
        cases = (
            (
                "path",
                {"path": primary_path, "sha256": "c" * 64},
                f"slot.json {device_lab.D2D_PAYMENT_TRANSCRIPTS_FIELD} must not reuse "
                f"{primary_path} for multiple transports",
            ),
            (
                "digest",
                {"path": "handoff/d2d-payment-replay.json", "sha256": primary_digest},
                f"slot.json {device_lab.D2D_PAYMENT_TRANSCRIPTS_FIELD} must not reuse "
                "sha256 digests for multiple transports",
            ),
        )
        original_validator = device_lab._validate_d2d_payment_transcript_entry  # type: ignore[attr-defined]
        for name, replay_binding, expected_error in cases:
            with self.subTest(name=name):
                metadata = {
                    device_lab.D2D_PAYMENT_TRANSCRIPTS_FIELD: {
                        replay_transport: dict(replay_binding)
                    }
                }

                def replay_validator(
                    _slot_path: Path,
                    _metadata: dict[str, object],
                    transport: str,
                    _entry: object,
                    _errors: list[str],
                ) -> tuple[str, dict[str, str]]:
                    return transport, dict(replay_binding)

                try:
                    device_lab._validate_d2d_payment_transcript_entry = replay_validator  # type: ignore[attr-defined]
                    errors: list[str] = []
                    bindings = device_lab.validate_d2d_payment_transcripts_binding(
                        Path("slot-0"),
                        metadata,
                        errors,
                        primary_relative=primary_path,
                        primary_digest=primary_digest,
                        primary_transport=primary_transport,
                    )
                finally:
                    device_lab._validate_d2d_payment_transcript_entry = original_validator  # type: ignore[attr-defined]

                self.assertEqual(
                    bindings,
                    {
                        primary_transport: {
                            "path": primary_path,
                            "sha256": primary_digest,
                        }
                    },
                )
                self.assertIn(expected_error, errors)

    def test_production_metadata_rejects_signed_multi_transport_d2d_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
                d2d_payment_transport=transports[0],
                d2d_payment_transports=transports,
            )
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["d2d_payment_transcripts"]["qr"]["sha256"] = "f" * 64
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact d2d_payment_transcripts must match "
            "slot.json d2d_payment_transcripts",
            report["errors"],
        )

    def test_d2d_payment_transcript_rejects_zero_sha256_placeholders(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["payer_wallet_state_before_sha256"] = "0" * 64
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript payer_wallet_state_before_sha256 must be non-zero lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_outside_handoff(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            source = slot / "handoff" / "d2d-payment.json"
            target = slot / "telemetry" / "d2d-payment.json"
            target.write_bytes(source.read_bytes())
            digest = hashlib.sha256(target.read_bytes()).hexdigest()

            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["d2d_payment_transcript_path"] = "telemetry/d2d-payment.json"
            metadata["d2d_payment_transcript_sha256"] = digest
            write_json(metadata_path, metadata)

            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["d2d_payment_transcript_path"] = "telemetry/d2d-payment.json"
            evidence["d2d_payment_transcript_sha256"] = digest
            evidence["artifact_digests"] = required_artifact_digests(slot)
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json d2d_payment_transcript_path must stay under handoff/",
            report["errors"],
        )

    def test_production_metadata_rejects_root_only_artifact_paths(self) -> None:
        cases = (
            (
                "d2d_payment_transcript_path",
                "handoff",
                "slot.json d2d_payment_transcript_path must stay under handoff/",
            ),
            (
                "wallet_integrity_transcript_path",
                "wallet",
                "slot.json wallet_integrity_transcript_path must stay under wallet/",
            ),
            (
                "attestation_certificate_chain_path",
                "attestation",
                "slot.json attestation_certificate_chain_path must stay under attestation/",
            ),
            (
                "kagemusha_wallet_apk_path",
                "evidence",
                "slot.json kagemusha_wallet_apk_path must stay under evidence/",
            ),
            (
                "signed_evidence_artifact_path",
                "evidence",
                "slot.json signed_evidence_artifact_path must stay under evidence/",
            ),
        )
        for field, root_path, expected_error in cases:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temp:
                signer = create_test_signer(Path(temp))
                trusted = trusted_signers_for(signer)
                slot = create_slot(
                    Path(temp),
                    "pixel8",
                    device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                    signer,
                )
                metadata_path = slot / "slot.json"
                metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
                metadata[field] = root_path
                write_json(metadata_path, metadata)

                report = device_lab.scan_slot(
                    slot,
                    require_kagemusha_production_evidence=True,
                    trusted_signer_public_keys=trusted,
                )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_missing_wallet_integrity_transcript_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("wallet_integrity_transcript_sha256")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json wallet_integrity_transcript_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_wallet_integrity_transcript_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "wallet" / "integrity.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["old_key_invalidated"] = False
            write_json(transcript_path, transcript)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json wallet_integrity_transcript_sha256 does not match wallet_integrity_transcript_path",
            report["errors"],
        )

    def test_wallet_integrity_transcript_rejects_zero_sha256_placeholders(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "wallet" / "integrity.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["rotation_session_id_sha256"] = "0" * 64
            write_json(transcript_path, transcript)
            refresh_wallet_integrity_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "wallet integrity transcript rotation_session_id_sha256 must be non-zero lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_pending_queue_shape(self) -> None:
        cases = (
            (
                {"slot_id": "pixel7", "pending_transactions": []},
                "queue/pending_queue.json slot_id must match slot id",
            ),
            (
                {"slot_id": " pixel8 ", "pending_transactions": []},
                "queue/pending_queue.json slot_id must not contain surrounding whitespace",
            ),
            (
                {"slot_id": "pixel8", "pending_transactions": {}},
                "queue/pending_queue.json pending_transactions must be an array",
            ),
            (
                {
                    "slot_id": "pixel8",
                    "pending_transactions": [{"id": "leftover-transfer"}],
                },
                "queue/pending_queue.json pending_transactions must be empty after D2D handoff",
            ),
            (
                {
                    "slot_id": "pixel8",
                    "pending_transactions": [],
                    "debug_note": "not production evidence",
                },
                "queue/pending_queue.json contains unexpected field debug_note",
            ),
        )
        for payload, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    write_json(slot / "queue" / "pending_queue.json", payload)
                    transcript_path = slot / "handoff" / "d2d-payment.json"
                    transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
                    transcript["queue_after_sha256"] = hashlib.sha256(
                        (slot / "queue" / "pending_queue.json").read_bytes()
                    ).hexdigest()
                    write_json(transcript_path, transcript)
                    refresh_d2d_payment_transcript_hash(slot, signer)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_wallet_integrity_false_rollback_claim(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "wallet" / "integrity.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["stale_snapshot_rejected"] = False
            write_json(transcript_path, transcript)
            refresh_wallet_integrity_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "wallet integrity transcript stale_snapshot_rejected must be true",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_transcript_strings(self) -> None:
        cases = (
            (
                "d2d_payment_transcript_path",
                "device_family",
                " Google Pixel 8 / 8a / 8 Pro ",
                "d2d payment transcript device_family must not contain surrounding whitespace",
                refresh_d2d_payment_transcript_hash,
            ),
            (
                "d2d_payment_transcript_path",
                "slot_id",
                "pixel8\u0000",
                "d2d payment transcript slot_id must not contain control characters",
                refresh_d2d_payment_transcript_hash,
            ),
            (
                "wallet_integrity_transcript_path",
                "device_family",
                " Google Pixel 8 / 8a / 8 Pro ",
                "wallet integrity transcript device_family must not contain surrounding whitespace",
                refresh_wallet_integrity_transcript_hash,
            ),
            (
                "wallet_integrity_transcript_path",
                "slot_id",
                "pixel8\u0000",
                "wallet integrity transcript slot_id must not contain control characters",
                refresh_wallet_integrity_transcript_hash,
            ),
        )
        for metadata_path_key, field, value, expected_error, refresh in cases:
            with self.subTest(metadata_path_key=metadata_path_key, field=field):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    metadata = json.loads(
                        (slot / "slot.json").read_text(encoding="utf-8")
                    )
                    relative = metadata[metadata_path_key]
                    transcript_path = slot / relative
                    transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
                    transcript[field] = value
                    write_json(transcript_path, transcript)
                    refresh(slot, signer)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_wallet_integrity_unchanged_rotation_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "wallet" / "integrity.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["key_id_after_sha256"] = transcript["key_id_before_sha256"]
            write_json(transcript_path, transcript)
            refresh_wallet_integrity_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "wallet integrity transcript key_id_before_sha256 must differ from key_id_after_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_secret_field_with_redaction(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["token=supersecret"] = "must not ship"
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        rendered = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"d2d payment transcript contains unexpected field {device_lab.SECRET_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_production_metadata_rejects_d2d_payment_transcript_queue_splice(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["queue_after_sha256"] = "11" * 32
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript queue_after_sha256 must match queue/pending_queue.json",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_online_wallets(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["payer_wallet_offline"] = False
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript payer_wallet_offline must be true",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_attestation_challenge_splice(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["attestation_challenge_sha256"] = "33" * 32
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript attestation_challenge_sha256 must match slot.json attestation_challenge_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_unchanged_payer_wallet_state(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["payer_wallet_state_after_sha256"] = transcript[
                "payer_wallet_state_before_sha256"
            ]
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript payer_wallet_state_before_sha256 must differ from payer_wallet_state_after_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_oversized_d2d_payment_payload(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["payload_bytes"] = device_lab.MAX_D2D_PAYMENT_PAYLOAD_BYTES + 1
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript payload_bytes must be no more than "
            f"{device_lab.MAX_D2D_PAYMENT_PAYLOAD_BYTES}",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_lifecycle_raw_command_marker(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            stale_commands = [
                command
                for command in KAGEMUSHA_ANDROID_RAW_TEST_COMMANDS
                if "KagemushaCandidateLifecycleInstrumentedTest" not in command
            ]
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = stale_commands
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["raw_test_commands"] = stale_commands
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json raw_test_commands must include org.hyperledger.iroha.sdk.kagemusha.candidate.lab.KagemushaCandidateLifecycleInstrumentedTest",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include org.hyperledger.iroha.sdk.kagemusha.candidate.lab.KagemushaCandidateLifecycleInstrumentedTest",
            report["errors"],
        )

    def test_production_metadata_rejects_stale_native_bridge_abi_version(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["native_bridge_abi_version"] = (
                device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION - 1
            )
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["native_bridge_abi_version"] = metadata["native_bridge_abi_version"]
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"slot.json native_bridge_abi_version must be {device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION}",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_result_challenge_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["attestation_challenge_sha256"] = "22" * 32
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json attestation_challenge_sha256 must match slot.json attestation_challenge_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_result_chain_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["attestation_certificate_chain_sha256"] = "44" * 32
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json attestation_certificate_chain_sha256 must match slot.json attestation_certificate_chain_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_slot_alias_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["slot"] = "stale-slot"
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json slot must match the slot directory name",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_attestation_slot_alias(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["slot"] = f" {slot.name} "
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json slot must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_attestation_status(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["status"] = "OK"
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn("attestation/result.json status must be ok", report["errors"])

    def test_production_metadata_rejects_attestation_passed_status_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["status"] = "passed"
            write_json(attestation_path, attestation)
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verification"]["status"] = "passed"
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn("attestation/result.json status must be ok", report["errors"])
        self.assertIn(
            "attestation/report.json verification.status must be ok",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_result_without_strongbox(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["attestation_security_level"] = "TEE"
            attestation["keymaster_security_level"] = "TEE"
            attestation["keymint_security_level"] = "TEE"
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json must report STRONGBOX security level",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_attestation_strongbox_level(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["keymint_security_level"] = " STRONGBOX "
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json keymint_security_level must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_slot_keymint_level(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["keymint_security_level"] = "strongbox"
            write_json(metadata_path, metadata)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json keymint_security_level must be STRONGBOX",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_result_slot_keymint_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["keymint_security_level"] = "STRONG_BOX"
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["keymint_security_level"] = "STRONG_BOX"
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json keymint_security_level must match "
            "slot.json keymint_security_level",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_attestation_report(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            (slot / "attestation" / "report.json").unlink()
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn("missing attestation/report.json", report["errors"])

    def test_production_metadata_rejects_attestation_report_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["attestation_certificate_chain_sha256"] = "55" * 32
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json attestation_certificate_chain_sha256 must match slot.json attestation_certificate_chain_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_attestation_report_binding(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["device_fingerprint"] = (
                f" {attestation_report['device_fingerprint']} "
            )
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json device_fingerprint must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_report_without_strongbox(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verification"]["keymint_security_level"] = "TEE"
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json verification.keymint_security_level must be STRONGBOX",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_attestation_report_strongbox(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verification"]["keymint_security_level"] = " STRONGBOX "
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json verification.keymint_security_level "
            "must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_attestation_report_level_fields(
        self,
    ) -> None:
        for level_key in ("attestation_security_level", "keymaster_security_level"):
            with self.subTest(level_key=level_key):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    report_path = slot / "attestation" / "report.json"
                    attestation_report = json.loads(
                        report_path.read_text(encoding="utf-8")
                    )
                    del attestation_report["verification"][level_key]
                    write_json(report_path, attestation_report)
                    resign_signed_evidence_artifacts(slot, signer)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(
                    f"attestation/report.json verification.{level_key} "
                    "must be a non-empty string",
                    report["errors"],
                )

    def test_production_metadata_rejects_attestation_report_result_level_mismatch(
        self,
    ) -> None:
        for level_key in (
            "keymint_security_level",
            "attestation_security_level",
            "keymaster_security_level",
        ):
            with self.subTest(level_key=level_key):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    report_path = slot / "attestation" / "report.json"
                    attestation_report = json.loads(
                        report_path.read_text(encoding="utf-8")
                    )
                    attestation_report["verification"][level_key] = "STRONG_BOX"
                    write_json(report_path, attestation_report)
                    resign_signed_evidence_artifacts(slot, signer)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(
                    f"attestation/report.json verification.{level_key} must match "
                    f"attestation/result.json {level_key}",
                    report["errors"],
                )

    def test_production_metadata_rejects_attestation_report_result_status_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verification"]["status"] = "passed"
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json verification.status must match "
            "attestation/result.json status",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_report_unexpected_fields(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["unexpected"] = "drift"
            attestation_report["verification"]["debug"] = True
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json contains unexpected field unexpected",
            report["errors"],
        )
        self.assertIn(
            "attestation/report.json verification contains unexpected field debug",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_report_weak_verifier_status(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verifier"] = "token=supersecret"
            attestation_report["verification"]["status"] = "failed"
            attestation_report["verification"]["physical_device_attestation"] = False
            attestation_report["verification"]["keymaster_security_level"] = "TEE"
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json verifier must not contain secret-looking material",
            report["errors"],
        )
        self.assertIn(
            "attestation/report.json verification.status must be ok",
            report["errors"],
        )
        self.assertIn(
            "attestation/report.json verification.physical_device_attestation must be true",
            report["errors"],
        )
        self.assertIn(
            "attestation/report.json verification.keymaster_security_level must be STRONGBOX",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_attestation_report_status(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verification"]["status"] = "OK"
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json verification.status must be ok",
            report["errors"],
        )

    def test_production_metadata_rejects_zero_attestation_sha256_bindings(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["attestation_challenge_sha256"] = "0" * 64
            write_json(attestation_path, attestation)

            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["attestation_challenge_sha256"] = "0" * 64
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json attestation_challenge_sha256 must be non-zero lowercase sha256 hex",
            report["errors"],
        )
        self.assertIn(
            "attestation/report.json attestation_challenge_sha256 must be non-zero lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_challenge_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "attestation_challenge_sha256",
                    "11" * 32,
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact attestation_challenge_sha256 must match slot.json attestation_challenge_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_attestation_chain_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "attestation_certificate_chain_sha256",
                    "55" * 32,
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact attestation_certificate_chain_sha256 must match slot.json attestation_certificate_chain_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_apk_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("kagemusha_wallet_apk_sha256", "11" * 32),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact kagemusha_wallet_apk_sha256 must match slot.json kagemusha_wallet_apk_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_d2d_transcript_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "d2d_payment_transcript_sha256",
                    "11" * 32,
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact d2d_payment_transcript_sha256 must match slot.json d2d_payment_transcript_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_wallet_integrity_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "wallet_integrity_transcript_sha256",
                    "66" * 32,
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact wallet_integrity_transcript_sha256 must match slot.json wallet_integrity_transcript_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_unexpected_signed_evidence_fields_with_redaction(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["token=supersecret"] = "must not ship"
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        rendered = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"signed evidence artifact contains unexpected field {device_lab.SECRET_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_production_metadata_rejects_signed_evidence_probe_state_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["kagemusha_recursive_spend_ffi_surface"] = "ok"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact kagemusha_recursive_spend_ffi_surface must match slot.json kagemusha_recursive_spend_ffi_surface",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_raw_command_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = [
                "./gradlew :core-jvm:test --rerun"
            ]
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact raw_test_commands must match slot.json raw_test_commands",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_raw_test_command_strings(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            commands = list(device_lab.KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS)
            commands[0] = f" {commands[0]} "
            commands[1] = f"{commands[1]}\x1b[31m"
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = commands
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["raw_test_commands"] = commands
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )
            rendered = "\n".join(report["errors"])

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json raw_test_commands[0] must not contain surrounding whitespace",
            rendered,
        )
        self.assertIn(
            "slot.json raw_test_commands[1] must not contain control characters",
            rendered,
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands[0] must not contain surrounding whitespace",
            rendered,
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands[1] must not contain control characters",
            rendered,
        )
        self.assertNotIn("\x1b", rendered)

    def test_production_metadata_rejects_irrelevant_raw_test_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            unrelated_commands = ["./gradlew test --tests unrelated.HealthCheck"]
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = unrelated_commands
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["raw_test_commands"] = unrelated_commands
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        for marker in device_lab.RAW_TEST_COMMAND_REQUIRED_MARKERS:
            self.assertIn(
                f"slot.json raw_test_commands must include {marker}",
                report["errors"],
            )
            self.assertIn(
                f"signed evidence artifact raw_test_commands must include {marker}",
                report["errors"],
            )

    def test_production_metadata_rejects_marker_stuffed_raw_test_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            stuffed_commands = [
                "echo " + " ".join(device_lab.RAW_TEST_COMMAND_REQUIRED_MARKERS)
            ]
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = stuffed_commands
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["raw_test_commands"] = stuffed_commands
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json raw_test_commands must exactly match the Kagemusha Android production raw test command",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must exactly match the Kagemusha Android production raw test command",
            report["errors"],
        )
        for marker in device_lab.RAW_TEST_COMMAND_REQUIRED_MARKERS:
            self.assertNotIn(
                f"slot.json raw_test_commands must include {marker}",
                report["errors"],
            )

    def test_production_metadata_rejects_noncanonical_signed_evidence_timestamp(self) -> None:
        for timestamp, expected_error in (
            (
                "2026-06-06T00:00:00+00:00",
                "signed evidence artifact signed_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
            ),
            (
                " 2026-06-06T00:00:00Z ",
                "signed evidence artifact signed_at_utc must not contain surrounding whitespace",
            ),
        ):
            with self.subTest(timestamp=timestamp):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    evidence_path = slot / "evidence" / "signed-evidence.json"
                    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
                    evidence["signed_at_utc"] = timestamp
                    write_json(evidence_path, sign_evidence(evidence, signer))
                    refresh_signed_evidence_hash(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_signed_evidence_schema_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("schema", "iroha.android.device_lab.legacy"),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"signed evidence artifact schema must be {device_lab.SIGNED_EVIDENCE_SCHEMA}",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_slot_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("device_family", "Samsung Galaxy S24"),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact device_family must match slot.json device_family",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_device_model_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("device_model", "Pixel 7"),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact device_model must match slot.json device_model",
            report["errors"],
        )

    def test_production_metadata_rejects_slot_family_model_codename_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["device_model"] = "Pixel 7"
            metadata["device_codename"] = "panther"
            write_json(metadata_path, metadata)

            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["device_model"] = "Pixel 7"
            telemetry["device_codename"] = "panther"
            write_json(telemetry_path, telemetry)

            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["device_model"] = "Pixel 7"
            evidence["device_codename"] = "panther"
            evidence["artifact_digests"] = required_artifact_digests(slot)
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json device_family must match device_model/device_codename",
            report["errors"],
        )

    def test_production_metadata_rejects_conflicting_model_codename(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["device_model"] = "Pixel 8 Pro"
            metadata["device_codename"] = "panther"
            write_json(metadata_path, metadata)

            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["device_model"] = "Pixel 8 Pro"
            telemetry["device_codename"] = "panther"
            write_json(telemetry_path, telemetry)

            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["device_model"] = "Pixel 8 Pro"
            evidence["device_codename"] = "panther"
            evidence["artifact_digests"] = required_artifact_digests(slot)
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json device_model/device_codename must identify a standard Kagemusha family",
            report["errors"],
        )

    def test_production_metadata_rejects_unknown_model_with_known_codename(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["device_model"] = "Pixel 6 Pro"
            metadata["device_codename"] = "oriole"
            write_json(metadata_path, metadata)

            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["device_model"] = "Pixel 6 Pro"
            telemetry["device_codename"] = "oriole"
            write_json(telemetry_path, telemetry)

            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["device_model"] = "Pixel 6 Pro"
            evidence["device_codename"] = "oriole"
            evidence["artifact_digests"] = required_artifact_digests(slot)
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json device_model/device_codename must identify a standard Kagemusha family",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_signed_evidence_slot_field(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "device_family",
                    f" {metadata['device_family']} ",
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact device_family must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_control_signed_evidence_slot_field(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "device_family",
                    f"{metadata['device_family']}\x1b[31m",
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact device_family must not contain control characters",
            report["errors"],
        )
        self.assertNotIn("\x1b", "\n".join(report["errors"]))

    def test_production_metadata_rejects_whitespace_normalized_signed_evidence_algorithm(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("signature_algorithm", " ed25519 "),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact signature_algorithm must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_digest_map_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            def corrupt_runtime_log_digest(evidence: dict) -> None:
                evidence["artifact_digests"]["logs/runtime.log"] = "01" * 32

            mutate_signed_evidence(slot, corrupt_runtime_log_digest)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact digest mismatch for logs/runtime.log",
            report["errors"],
        )

    def test_production_metadata_rejects_zero_signed_evidence_artifact_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            def zero_runtime_log_digest(evidence: dict) -> None:
                evidence["artifact_digests"]["logs/runtime.log"] = "0" * 64

            mutate_signed_evidence(slot, zero_runtime_log_digest)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact artifact_digests[logs/runtime.log] must be non-zero lowercase sha256 hex",
            report["errors"],
        )

    def test_signed_evidence_artifact_digest_rejects_secret_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            secret_relative = "logs/token=supersecret.log"
            write_text(slot / secret_relative, "must not be hashed\n")

            digest, errors = device_lab._signed_evidence_artifact_sha256(
                slot,
                secret_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest path must not contain "
                "secret-looking material"
            ],
        )
        self.assertNotIn(secret_relative, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_signed_evidence_artifact_digest_rejects_control_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            control_relative = "logs/runtime\x1b[31m.log"

            digest, errors = device_lab._signed_evidence_artifact_sha256(
                slot,
                control_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest path: unsafe path contains "
                "control characters"
            ],
        )
        self.assertNotIn(control_relative, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_signed_evidence_artifact_digest_rejects_symlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = device_lab._signed_evidence_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references symlink artifact "
                "logs/runtime.log"
            ],
        )

    def test_signed_evidence_artifact_digest_rejects_hardlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = device_lab._signed_evidence_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references hardlinked artifact "
                "logs/runtime.log"
            ],
        )

    def test_signed_evidence_artifact_digest_rejects_oversized_artifact_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            runtime_log = slot / "logs" / "runtime.log"
            with runtime_log.open("wb") as handle:
                handle.seek(device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES)
                handle.write(b"x")

            digest, errors = device_lab._signed_evidence_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references artifact "
                "logs/runtime.log must be no more than "
                f"{device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES} bytes"
            ],
        )

    def test_signed_evidence_artifact_digest_uses_release_apk_specific_limit(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            apk_path = slot / "evidence" / "kagemusha-wallet-release.apk"
            apk_path.write_bytes(b"x" * 16)
            old_base_limit = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
            old_apk_limit = device_lab.MAX_KAGEMUSHA_WALLET_APK_BYTES
            try:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 8
                device_lab.MAX_KAGEMUSHA_WALLET_APK_BYTES = 32
                digest, errors = device_lab._signed_evidence_artifact_sha256(
                    slot,
                    "evidence/kagemusha-wallet-release.apk",
                )
            finally:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = old_base_limit
                device_lab.MAX_KAGEMUSHA_WALLET_APK_BYTES = old_apk_limit

        self.assertEqual(errors, [])
        self.assertEqual(digest, hashlib.sha256(b"x" * 16).hexdigest())

    def test_signed_evidence_artifact_digest_rejects_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                target = slot / "logs" / "runtime.log"

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == target:
                        raise OSError(
                            "simulated signed evidence artifact metadata failure"
                        )
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                digest, errors = device_lab._signed_evidence_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references artifact file metadata "
                "could not be read logs/runtime.log"
            ],
        )

    def test_signed_evidence_artifact_digest_rejects_read_failure_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            target = slot / "logs" / "runtime.log"

            digest, errors = with_open_failure(
                target,
                lambda: device_lab._signed_evidence_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                ),
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references artifact that could not be read "
                "logs/runtime.log"
            ],
        )

    def test_signed_evidence_artifact_digest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        original_validate = device_lab._validate_signed_evidence_artifact_for_digest

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                artifact_path = slot / "logs" / "runtime.log"
                swapped = False

                def swapping_validate(slot_path: Path, relative: str):
                    nonlocal swapped
                    artifact, artifact_stat, errors = original_validate(
                        slot_path,
                        relative,
                    )
                    if artifact == artifact_path and not errors and not swapped:
                        artifact_path.unlink()
                        write_text(artifact_path, "replacement runtime log\n")
                        swapped = True
                    return artifact, artifact_stat, errors

                device_lab._validate_signed_evidence_artifact_for_digest = (
                    swapping_validate
                )

                digest, errors = device_lab._signed_evidence_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
                replacement_bytes = artifact_path.read_bytes()
        finally:
            device_lab._validate_signed_evidence_artifact_for_digest = original_validate

        self.assertTrue(swapped)
        self.assertIsNone(digest)
        self.assertEqual(replacement_bytes, b"replacement runtime log\n")
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references artifact changed while "
                "being read logs/runtime.log"
            ],
        )

    def test_signed_evidence_artifact_revalidates_required_digest_before_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            evidence_path = slot / "evidence" / "signed-evidence.json"
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            original_validate = device_lab.validate_required_kagemusha_slot_artifact_shapes

            def validate_then_alias(
                slot_path: Path,
                errors: list[str],
                *args: object,
                **kwargs: object,
            ) -> None:
                original_validate(slot_path, errors, *args, **kwargs)
                if not errors:
                    replace_with_symlink(
                        self,
                        slot_path / "logs" / "runtime.log",
                        target,
                    )

            errors: list[str] = []
            try:
                device_lab.validate_required_kagemusha_slot_artifact_shapes = (
                    validate_then_alias
                )
                device_lab.validate_signed_evidence_artifact(
                    slot,
                    evidence_path,
                    metadata,
                    trusted,
                    errors,
                )
            finally:
                device_lab.validate_required_kagemusha_slot_artifact_shapes = (
                    original_validate
                )

        self.assertIn(
            "signed evidence artifact digest references symlink artifact "
            "logs/runtime.log",
            errors,
        )
        self.assertNotIn(
            "signed evidence artifact digest mismatch for logs/runtime.log",
            errors,
        )

    def test_production_metadata_rejects_signed_evidence_missing_required_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            def remove_runtime_log_digest(evidence: dict) -> None:
                evidence["artifact_digests"].pop("logs/runtime.log")

            mutate_signed_evidence(slot, remove_runtime_log_digest)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact artifact_digests[logs/runtime.log] must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_missing_release_apk_digest(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            apk_path = metadata["kagemusha_wallet_apk_path"]

            def remove_release_apk_digest(evidence: dict) -> None:
                evidence["artifact_digests"].pop(apk_path, None)

            mutate_signed_evidence(slot, remove_release_apk_digest)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"signed evidence artifact artifact_digests[{apk_path}] must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_missing_attestation_chain_digest(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            chain_path = metadata["attestation_certificate_chain_path"]

            def remove_attestation_chain_digest(evidence: dict) -> None:
                evidence["artifact_digests"].pop(chain_path, None)

            mutate_signed_evidence(slot, remove_attestation_chain_digest)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"signed evidence artifact artifact_digests[{chain_path}] must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_required_slot_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            (slot / "logs" / "runtime.log").unlink()
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact required slot artifact is missing logs/runtime.log",
            report["errors"],
        )

    def test_production_metadata_rejects_empty_required_slot_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "logs" / "runtime.log", "")
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "required slot artifact logs/runtime.log must be non-empty",
            report["errors"],
        )

    def test_production_metadata_rejects_oversized_required_slot_artifact(self) -> None:
        old_limit = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
        try:
            device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 8
            with tempfile.TemporaryDirectory() as temp:
                signer = create_test_signer(Path(temp))
                trusted = trusted_signers_for(signer)
                slot = create_slot(
                    Path(temp),
                    "pixel8",
                    device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                    signer,
                )
                write_text(slot / "logs" / "runtime.log", "runtime log too large\n")
                rewrite_sha256sum(slot)

                report = device_lab.scan_slot(
                    slot,
                    require_kagemusha_production_evidence=True,
                    trusted_signer_public_keys=trusted,
                )
        finally:
            device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = old_limit

        self.assertEqual(report["status"], "error")
        self.assertTrue(
            any(
                "logs/runtime.log must be no more than 8 bytes" in error
                for error in report["errors"]
            ),
            report["errors"],
        )

    def test_production_metadata_rejects_telemetry_slot_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["slot_id"] = "other-slot"
            write_json(telemetry_path, telemetry)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/telemetry.json slot_id must match the slot directory name",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_telemetry_slot(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["slot_id"] = " pixel8 "
            write_json(telemetry_path, telemetry)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/telemetry.json slot_id must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_telemetry_extra_field(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["debug_note"] = "not production evidence"
            write_json(telemetry_path, telemetry)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/telemetry.json contains unexpected field debug_note",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_telemetry_identity_strings(
        self,
    ) -> None:
        cases = (
            (
                "device_model",
                " Pixel 8 ",
                "telemetry/telemetry.json device_model must not contain surrounding whitespace",
            ),
            (
                "device_codename",
                "husky\u0000",
                "telemetry/telemetry.json device_codename must not contain control characters",
            ),
            (
                "app_package_name",
                "",
                "telemetry/telemetry.json app_package_name must be a non-empty string",
            ),
            (
                "app_package_name",
                "token=not-for-telemetry",
                "telemetry/telemetry.json app_package_name must not contain secret-looking material",
            ),
        )
        for field, value, expected_error in cases:
            with self.subTest(field=field, expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    telemetry_path = slot / "telemetry" / "telemetry.json"
                    telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
                    telemetry[field] = value
                    write_json(telemetry_path, telemetry)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_telemetry_model_slot_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["device_model"] = "Pixel 7"
            write_json(telemetry_path, telemetry)

            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["artifact_digests"] = required_artifact_digests(slot)
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/telemetry.json device_model must match slot.json device_model",
            report["errors"],
        )

    def test_production_metadata_rejects_telemetry_app_package_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["app_package_name"] = "org.hyperledger.iroha.android.other"
            write_json(telemetry_path, telemetry)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/telemetry.json app_package_name must match slot.json app_package_name",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_telemetry_slot_binding(
        self,
    ) -> None:
        cases = (
            (
                "pixel8\u0000",
                "telemetry/telemetry.json slot_id must not contain control characters",
            ),
            (
                "",
                "telemetry/telemetry.json slot_id must be a non-empty string",
            ),
            (
                8,
                "telemetry/telemetry.json slot_id must be a non-empty string",
            ),
        )
        for slot_value, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    telemetry_path = slot / "telemetry" / "telemetry.json"
                    telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
                    telemetry["slot_id"] = slot_value
                    write_json(telemetry_path, telemetry)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_noncanonical_telemetry_suite(self) -> None:
        cases = (
            (
                "KAGEMUSHA-DEVICE-LAB",
                "telemetry/telemetry.json suite must identify a Kagemusha device-lab run",
            ),
            (
                " kagemusha-device-lab ",
                "telemetry/telemetry.json suite must not contain surrounding whitespace",
            ),
            (
                "kagemusha-device-lab\u0000",
                "telemetry/telemetry.json suite must not contain control characters",
            ),
            (
                "",
                "telemetry/telemetry.json suite must be a non-empty string",
            ),
            (
                7,
                "telemetry/telemetry.json suite must be a non-empty string",
            ),
        )
        for suite, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    telemetry_path = slot / "telemetry" / "telemetry.json"
                    telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
                    telemetry["suite"] = suite
                    write_json(telemetry_path, telemetry)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_failed_status_ndjson(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "telemetry" / "status.ndjson", '{"status":"failed"}\n')
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/status.ndjson line 1 status must not be 'failed'",
            report["errors"],
        )
        self.assertIn(
            "telemetry/status.ndjson must contain at least one ok status",
            report["errors"],
        )

    def test_production_metadata_rejects_status_ndjson_unexpected_field(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "telemetry" / "status.ndjson",
                '{"status":"ok","slot_id":"pixel8","debug_note":"ignored"}\n',
            )
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/status.ndjson line 1 contains unexpected field debug_note",
            report["errors"],
        )

    def test_production_metadata_redacts_status_ndjson_nonfinite_constant(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "telemetry" / "status.ndjson",
                '{"status":"ok","slot_id":"pixel8","elapsed":Infinity}\n',
            )
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )
            rendered = json.dumps(report, sort_keys=True)

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/status.ndjson line 1 contains non-finite constant "
            f"{device_lab.JSON_NONFINITE_CONSTANT_REDACTION}",
            report["errors"],
        )
        self.assertNotIn("Infinity", rendered)

    def test_production_metadata_rejects_unknown_status_ndjson(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "telemetry" / "status.ndjson",
                (
                    '{"status":"ok","slot_id":"pixel8"}\n'
                    '{"status":"skipped","slot_id":"pixel8"}\n'
                ),
            )
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/status.ndjson line 2 status must be ok",
            report["errors"],
        )

    def test_production_metadata_requires_status_ndjson_slot_id(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "telemetry" / "status.ndjson", '{"status":"ok"}\n')
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/status.ndjson line 1 slot_id must be a non-empty string",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_status_ndjson(self) -> None:
        cases = (
            (
                ' {"status":"ok","slot_id":"pixel8"}\n',
                "telemetry/status.ndjson line 1 must not contain surrounding whitespace",
            ),
            (
                '{"status":"ok","slot_id":"pixel8"} \n',
                "telemetry/status.ndjson line 1 must not contain surrounding whitespace",
            ),
            (
                '{"status":"ok","slot_id":"pixel8"}\r\n',
                "telemetry/status.ndjson must use LF line endings",
            ),
            (
                '{"status":"ok","slot_id":"pixel8"}',
                "telemetry/status.ndjson must end with a trailing newline",
            ),
            (
                '{"status":"OK","slot_id":"pixel8"}\n',
                "telemetry/status.ndjson line 1 status must be lowercase",
            ),
            (
                '{"status":" ok ","slot_id":"pixel8"}\n',
                "telemetry/status.ndjson line 1 status must not contain surrounding whitespace",
            ),
            (
                '{"status":"ok\\u0000","slot_id":"pixel8"}\n',
                "telemetry/status.ndjson line 1 status must not contain control characters",
            ),
        )
        for payload, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    write_text(slot / "telemetry" / "status.ndjson", payload)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])
                if not expected_error.startswith("telemetry/status.ndjson must "):
                    self.assertIn(
                        "telemetry/status.ndjson must contain at least one ok status",
                        report["errors"],
                    )

    def test_production_metadata_rejects_status_ndjson_slot_mismatch(self) -> None:
        cases = (
            (
                '{"status":"ok","slot_id":" pixel8 "}\n',
                "telemetry/status.ndjson line 1 slot_id must not contain surrounding whitespace",
            ),
            (
                '{"status":"ok","slot_id":"pixel8\\u0000"}\n',
                "telemetry/status.ndjson line 1 slot_id must not contain control characters",
            ),
            (
                '{"status":"ok","slot_id":8}\n',
                "telemetry/status.ndjson line 1 slot_id must be a string",
            ),
        )
        for payload, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    write_text(slot / "telemetry" / "status.ndjson", payload)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_runtime_log_without_completion_marker(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "logs" / "runtime.log", "device-lab run started\n")
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "logs/runtime.log must contain Kagemusha device-lab completion marker",
            report["errors"],
        )

    def test_production_metadata_rejects_runtime_log_failure_marker(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "logs" / "runtime.log",
                "kagemusha device-lab run complete\nBUILD FAILED\n",
            )
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "logs/runtime.log must not contain failure marker BUILD FAILED",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_missing_handoff_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            def remove_handoff_digest(evidence: dict) -> None:
                evidence["artifact_digests"].pop("handoff/d2d-payment.json")

            mutate_signed_evidence(slot, remove_handoff_digest)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact artifact_digests[handoff/d2d-payment.json] must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_trusted_signer_public_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            report = device_lab.scan_slot(slot, require_kagemusha_production_evidence=True)

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "trusted signer public key required for Kagemusha production evidence",
            report["errors"],
        )
        self.assertNotIn(
            "signed evidence artifact signer_public_key_sha256 must match a trusted signer public key",
            report["errors"],
        )

    def test_production_metadata_rejects_untrusted_signed_evidence_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "signer-a")
            other_signer = create_test_signer(root / "signer-b")
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(other_signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact signer_public_key_sha256 must match a trusted signer public key",
            report["errors"],
        )

    def test_production_metadata_rejects_direct_trusted_signer_digest_key_misbind(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "signer-a")
            other_signer = create_test_signer(root / "signer-b")
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            trusted = {signer["public_key_sha256"]: other_signer["public_key"]}

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,  # type: ignore[arg-type]
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "trusted signer public key digest must match public key DER sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_trusted_signer_public_key_symlinked_ancestor_from_direct_map(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-keys"
            signer = create_test_signer(real_parent / "keys")
            linked_parent = root / "linked-keys"
            create_dir_symlink(self, linked_parent, real_parent)
            linked_public_key = linked_parent / signer["public_key"].relative_to(real_parent)
            trusted = {signer["public_key_sha256"]: linked_public_key}
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "trusted signer public key ancestor directory must not be a symlink",
            report["errors"],
        )
        rendered = "\n".join(report["errors"])
        self.assertNotIn(str(linked_public_key), rendered)
        self.assertNotIn(str(real_parent), rendered)
        self.assertNotIn(str(linked_parent), rendered)

    def test_production_metadata_rejects_zero_trusted_signer_digest_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "pixel8"
            trusted = {"0" * 64: Path(temp) / "safe.pem"}

            with mock.patch.object(
                device_lab,
                "_load_json",
                side_effect=AssertionError("slot metadata must not be read"),
            ) as load_json:
                errors, details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted_signer_public_keys=trusted,
                )

        load_json.assert_not_called()
        self.assertEqual(details, {})
        self.assertEqual(
            errors,
            ["trusted signer public key digest must be non-zero lowercase sha256 hex"],
        )

    def test_production_metadata_rejects_misbound_trusted_signer_digest_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            digest_signer = create_test_signer(root / "digest-key")
            path_signer = create_test_signer(root / "path-key")
            slot = root / "pixel8"
            trusted = {
                digest_signer["public_key_sha256"]: path_signer["public_key"]
            }

            with mock.patch.object(
                device_lab,
                "_load_json",
                side_effect=AssertionError("slot metadata must not be read"),
            ) as load_json:
                errors, details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted_signer_public_keys=trusted,  # type: ignore[arg-type]
                )

            rendered = "\n".join(errors)

        load_json.assert_not_called()
        self.assertEqual(details, {})
        self.assertEqual(
            errors,
            ["trusted signer public key digest must match public key DER sha256"],
        )
        self.assertNotIn(str(path_signer["public_key"]), rendered)

    def test_production_metadata_rejects_private_trusted_signer_key_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "signer")
            private_key_as_public = root / "release-verifier.pem"
            private_key_as_public.write_bytes(signer["private_key"].read_bytes())  # type: ignore[union-attr]
            slot = root / "pixel8"
            trusted = {
                signer["public_key_sha256"]: private_key_as_public,
            }

            with mock.patch.object(
                device_lab,
                "_load_json",
                side_effect=AssertionError("slot metadata must not be read"),
            ) as load_json:
                errors, details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted_signer_public_keys=trusted,  # type: ignore[arg-type]
                )
            rendered = "\n".join(errors)

        load_json.assert_not_called()
        self.assertEqual(details, {})
        self.assertEqual(
            errors,
            ["trusted signer public key must contain public key material, not a private key"],
        )
        self.assertNotIn(str(private_key_as_public), rendered)

    def test_production_metadata_rejects_non_path_trusted_signer_map_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "pixel8"
            trusted = {"1" * 64: "safe.pem"}

            with mock.patch.object(
                device_lab,
                "_load_json",
                side_effect=AssertionError("slot metadata must not be read"),
            ) as load_json:
                errors, details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted_signer_public_keys=trusted,  # type: ignore[arg-type]
                )

        load_json.assert_not_called()
        self.assertEqual(details, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must be a pathlib Path"],
        )

    def test_production_metadata_rejects_non_mapping_trusted_signer_map_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "pixel8"

            with mock.patch.object(
                device_lab,
                "_load_json",
                side_effect=AssertionError("slot metadata must not be read"),
            ) as load_json:
                errors, details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted_signer_public_keys=[("1" * 64, Path(temp) / "safe.pem")],  # type: ignore[arg-type]
                )

        load_json.assert_not_called()
        self.assertEqual(details, {})
        self.assertEqual(errors, ["trusted signer public key map must be a mapping"])

    def test_production_metadata_rejects_mixed_trusted_signer_digest_keys_without_crash(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "pixel8"
            trusted = {
                7: Path(temp) / "integer.pem",
                "1" * 64: Path(temp) / "safe.pem",
            }

            with mock.patch.object(
                device_lab,
                "_load_json",
                side_effect=AssertionError("slot metadata must not be read"),
            ) as load_json:
                errors, details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted_signer_public_keys=trusted,  # type: ignore[arg-type]
                )

        load_json.assert_not_called()
        self.assertEqual(details, {})
        self.assertIn(
            "trusted signer public key digest must be non-zero lowercase sha256 hex",
            errors,
        )
        self.assertNotIn("TypeError", "\n".join(errors))

    def test_production_metadata_rejects_unrepresentable_trusted_signer_digest_without_crash(
        self,
    ) -> None:
        class UnrepresentableDigest:
            def __repr__(self) -> str:
                raise AssertionError("digest repr must not be used")

        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "pixel8"
            trusted = {
                UnrepresentableDigest(): Path(temp) / "unrepresentable.pem",
                "1" * 64: Path(temp) / "safe.pem",
            }

            with mock.patch.object(
                device_lab,
                "_load_json",
                side_effect=AssertionError("slot metadata must not be read"),
            ) as load_json:
                errors, details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted_signer_public_keys=trusted,  # type: ignore[arg-type]
                )

        load_json.assert_not_called()
        self.assertEqual(details, {})
        self.assertIn(
            "trusted signer public key digest must be non-zero lowercase sha256 hex",
            errors,
        )

    def test_production_metadata_rejects_control_trusted_signer_map_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "pixel8"
            trusted = {"1" * 64: Path(temp) / "control\nsigner.pem"}

            with mock.patch.object(
                device_lab,
                "_load_json",
                side_effect=AssertionError("slot metadata must not be read"),
            ) as load_json:
                errors, details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted_signer_public_keys=trusted,
                )

            rendered = "\n".join(errors)

        load_json.assert_not_called()
        self.assertEqual(details, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must not contain control characters"],
        )
        self.assertNotIn("control\nsigner.pem", rendered)

    def test_production_metadata_rejects_whitespace_trusted_signer_map_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "pixel8"
            trusted = {"1" * 64: Path(temp) / " signer.pem"}

            with mock.patch.object(
                device_lab,
                "_load_json",
                side_effect=AssertionError("slot metadata must not be read"),
            ) as load_json:
                errors, details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted_signer_public_keys=trusted,
                )

        load_json.assert_not_called()
        self.assertEqual(details, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must not contain surrounding whitespace"],
        )

    def test_production_metadata_rejects_alias_trusted_signer_map_before_metadata_read(
        self,
    ) -> None:
        cases = (
            (
                Path("keys") / ".." / "signer.pem",
                "trusted signer public key path must be canonical",
            ),
            (
                Path("keys\\signer.pem"),
                "trusted signer public key path must not contain backslashes",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for key_path, expected_error in cases:
                key_path = root / key_path
                with self.subTest(key_path=key_path):
                    slot = root / "pixel8"
                    trusted = {"1" * 64: key_path}

                    with mock.patch.object(
                        device_lab,
                        "_load_json",
                        side_effect=AssertionError("slot metadata must not be read"),
                    ) as load_json:
                        errors, details = device_lab.validate_kagemusha_production_metadata(
                            slot,
                            trusted_signer_public_keys=trusted,
                        )
                    rendered = "\n".join(errors)

                    load_json.assert_not_called()
                    self.assertEqual(details, {})
                    self.assertEqual(errors, [expected_error])
                    self.assertNotIn(str(key_path), rendered)

    def test_production_metadata_rejects_signed_evidence_payload_hash_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("signature_payload_sha256", "01" * 32),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact signature_payload_sha256 mismatch",
            report["errors"],
        )

    def test_production_metadata_rejects_zero_signed_evidence_sha256_placeholders(
        self,
    ) -> None:
        fields = (
            "attestation_challenge_sha256",
            "attestation_certificate_chain_sha256",
            "kagemusha_wallet_policy_sha256",
            "kagemusha_wallet_apk_sha256",
            "d2d_payment_transcript_sha256",
            "wallet_integrity_transcript_sha256",
            "signer_public_key_sha256",
            "signature_payload_sha256",
        )
        for field in fields:
            with self.subTest(field=field):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    mutate_signed_evidence(
                        slot,
                        lambda evidence, field=field: evidence.__setitem__(
                            field,
                            "0" * 64,
                        ),
                    )

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(
                    f"signed evidence artifact {field} must be non-zero lowercase sha256 hex",
                    report["errors"],
                )

    def test_signed_evidence_canonical_payload_rejects_nonfinite_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            errors: list[str] = []

            with mock.patch.object(
                device_lab,
                "_load_json",
                return_value={
                    "schema": device_lab.SIGNED_EVIDENCE_SCHEMA,
                    "nonfinite": float("nan"),
                },
            ):
                device_lab.validate_signed_evidence_artifact(
                    Path(temp),
                    Path(temp) / "signed-evidence.json",
                    {},
                    {},
                    errors,
                )

        self.assertIn(
            "signed evidence artifact signature payload is not strict JSON",
            errors,
        )

    def test_production_metadata_rejects_signed_evidence_signature_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("signature", "00" * 64),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact signature verification failed",
            report["errors"],
        )

    def test_standard_matrix_requires_every_kagemusha_device_family(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-standard-matrix",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                    ]
                )

        self.assertEqual(status, 1)

    def test_signer_helper_generates_validator_accepted_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel6")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
            )

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                sign_status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
                validate_status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "pixel6",
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                    ]
                )

            evidence = json.loads(
                (slot / "evidence" / "signed-evidence.json").read_text(encoding="utf-8")
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))

        self.assertEqual(sign_status, 0)
        self.assertEqual(validate_status, 0)
        self.assertEqual(evidence["signer_public_key_sha256"], signer["public_key_sha256"])
        self.assertEqual(
            metadata["signed_evidence_artifact_sha256"],
            hashlib.sha256(
                json.dumps(evidence, indent=2, sort_keys=True).encode("utf-8") + b"\n"
            ).hexdigest(),
        )

    def test_signer_helper_preserves_multi_transport_d2d_transcripts(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(
                root,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
                d2d_payment_transport="nearby_offline",
                d2d_payment_transports=tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS)),
            )

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                sign_status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
                validate_status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "pixel6",
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                    ]
                )

            evidence = json.loads(
                (slot / "evidence" / "signed-evidence.json").read_text(encoding="utf-8")
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))

        self.assertEqual(sign_status, 0)
        self.assertEqual(validate_status, 0)
        self.assertEqual(
            set(evidence["d2d_payment_transcripts"]),
            device_lab.D2D_PAYMENT_TRANSPORTS,
        )
        self.assertEqual(
            evidence["d2d_payment_transcripts"],
            metadata["d2d_payment_transcripts"],
        )

    def test_signer_helper_rejects_nonfinite_canonical_payload_before_signing(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel6")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            errors: list[str] = []

            with (
                mock.patch.object(
                    evidence_signer,
                    "_artifact_digests",
                    return_value={"nonfinite": float("nan")},
                ),
                mock.patch.object(
                    evidence_signer,
                    "_sign_ed25519",
                    side_effect=AssertionError("signature should not be attempted"),
                ),
            ):
                evidence = evidence_signer.build_signed_evidence(
                    slot,
                    metadata,
                    private_key_path=signer["private_key"],
                    public_key_path=signer["public_key"],
                    signer_key_id="android-lab-release-signer-v1",
                    signed_at_utc="2026-06-06T00:00:00Z",
                    errors=errors,
                )

        self.assertIsNone(evidence)
        self.assertEqual(errors, ["signed evidence payload is not strict JSON"])

    def test_signer_helper_rejects_mismatched_private_and_public_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            private_signer = create_test_signer(Path(temp) / "private-signer")
            public_signer = create_test_signer(Path(temp) / "public-signer")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(private_signer["private_key"]),
                        "--public-key",
                        str(public_signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "private key did not produce a signature accepted by the signer public key",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_trusted_signer_public_key_rejects_symlink_without_path_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            public_key_link = root / "trusted-public-key-link.pem"
            try:
                public_key_link.symlink_to(signer["public_key"])
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            trusted, errors = device_lab.load_trusted_signer_public_keys(
                [public_key_link]
            )

        self.assertEqual(trusted, {})
        self.assertEqual(errors, ["trusted signer public key must not be a symlink"])
        self.assertNotIn(str(public_key_link), "\n".join(errors))

    def test_trusted_signer_public_key_rejects_secret_looking_path_without_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_public_key = Path(temp) / "token=supersecret-public.pem"
            secret_public_key.write_text("not a public key\n", encoding="utf-8")

            trusted, errors = device_lab.load_trusted_signer_public_keys(
                [secret_public_key]
            )

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must not contain secret-looking material"],
        )
        rendered = "\n".join(errors)
        self.assertNotIn(str(secret_public_key), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_trusted_signer_public_key_rejects_secret_path_before_openssl_lookup(
        self,
    ) -> None:
        with mock.patch.object(
            device_lab,
            "_require_openssl",
            side_effect=AssertionError("OpenSSL lookup must not run"),
        ):
            with tempfile.TemporaryDirectory() as temp:
                secret_public_key = Path(temp) / "token=supersecret-public.pem"

                trusted, errors = device_lab.load_trusted_signer_public_keys(
                    [secret_public_key]
                )
                rendered = "\n".join(errors)

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must not contain secret-looking material"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(secret_public_key), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_trusted_signer_public_key_rejects_control_path_before_openssl_lookup(
        self,
    ) -> None:
        with mock.patch.object(
            device_lab,
            "_require_openssl",
            side_effect=AssertionError("OpenSSL lookup must not run"),
        ):
            with tempfile.TemporaryDirectory() as temp:
                control_public_key = Path(temp) / "control\npublic.pem"

                trusted, errors = device_lab.load_trusted_signer_public_keys(
                    [control_public_key]
                )
                rendered = "\n".join(errors)

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must not contain control characters"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(control_public_key), rendered)

    def test_trusted_signer_public_key_rejects_surrounding_whitespace_before_openssl(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            unsafe_key = Path(temp) / " trusted-public.pem"
            with mock.patch.object(
                device_lab,
                "_require_openssl",
                side_effect=AssertionError("OpenSSL lookup must not run"),
            ):
                trusted, errors = device_lab.load_trusted_signer_public_keys(
                    [unsafe_key]
                )

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must not contain surrounding whitespace"],
        )
        self.assertNotIn(str(unsafe_key), "\n".join(errors))

    def test_trusted_signer_public_key_loader_accepts_single_path_input(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            cases = (signer["public_key"], str(signer["public_key"]))
            results = []
            for raw_path in cases:
                with self.subTest(raw_path_type=type(raw_path).__name__):
                    trusted, errors = device_lab.load_trusted_signer_public_keys(
                        raw_path
                    )
                    results.append((trusted, errors))

        for trusted, errors in results:
            self.assertEqual(errors, [])
            self.assertEqual(
                trusted,
                {signer["public_key_sha256"]: signer["public_key"]},
            )

    def test_trusted_signer_public_key_loader_rejects_non_iterable_input(
        self,
    ) -> None:
        trusted, errors = device_lab.load_trusted_signer_public_keys(42)  # type: ignore[arg-type]

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key paths must be an iterable of paths"],
        )

    def test_trusted_signer_public_key_loader_rejects_non_path_entry(
        self,
    ) -> None:
        with mock.patch.object(
            device_lab,
            "_require_openssl",
            side_effect=AssertionError("OpenSSL lookup must not run"),
        ):
            trusted, errors = device_lab.load_trusted_signer_public_keys(
                [object()]  # type: ignore[list-item]
            )

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must be a string or pathlib Path"],
        )

    def test_trusted_signer_public_key_rejects_aliases_before_openssl_lookup(
        self,
    ) -> None:
        cases = (
            (
                Path("keys") / ".." / "trusted-public.pem",
                "trusted signer public key path must be canonical",
            ),
            (
                Path("keys\\trusted-public.pem"),
                "trusted signer public key path must not contain backslashes",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for public_key, expected_error in cases:
                public_key = root / public_key
                with self.subTest(public_key=public_key):
                    with mock.patch.object(
                        device_lab,
                        "_require_openssl",
                        side_effect=AssertionError("OpenSSL lookup must not run"),
                    ):
                        trusted, errors = device_lab.load_trusted_signer_public_keys(
                            [public_key]
                        )
                    rendered = "\n".join(errors)

                    self.assertEqual(trusted, {})
                    self.assertEqual(errors, [expected_error])
                    self.assertNotIn(str(public_key), rendered)

    def test_trusted_signer_public_key_rejects_private_key_material_before_openssl_lookup(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            private_key_as_public = root / "release-verifier.pem"
            private_key_as_public.write_bytes(signer["private_key"].read_bytes())  # type: ignore[union-attr]

            with mock.patch.object(
                device_lab,
                "_require_openssl",
                side_effect=AssertionError("OpenSSL lookup must not run"),
            ):
                trusted, errors = device_lab.load_trusted_signer_public_keys(
                    [private_key_as_public]
                )
            rendered = "\n".join(errors)

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key must contain public key material, not a private key"],
        )
        self.assertNotIn(str(private_key_as_public), rendered)

    def test_verify_signature_rejects_secret_public_key_path_before_openssl_lookup(
        self,
    ) -> None:
        with mock.patch.object(
            device_lab,
            "_require_openssl",
            side_effect=AssertionError("OpenSSL lookup must not run"),
        ):
            with tempfile.TemporaryDirectory() as temp:
                secret_public_key = Path(temp) / "token=supersecret-public.pem"
                errors: list[str] = []

                device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                    public_key_path=secret_public_key,
                    payload=b"payload",
                    signature=b"signature",
                    errors=errors,
                    label="signer public key",
                )
                rendered = "\n".join(errors)

        self.assertEqual(
            errors,
            ["signer public key path must not contain secret-looking material"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(secret_public_key), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_openssl_public_key_der_rejects_spawn_failure_after_path_shape(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run

        def failing_run(*args, **kwargs):
            raise OSError("simulated OpenSSL spawn failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = failing_run
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                    public_key,
                    errors=errors,
                    label="trusted signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run

        self.assertIsNone(der)
        self.assertEqual(
            errors,
            ["trusted signer public key OpenSSL public key command could not be run"],
        )

    def test_openssl_public_key_der_scrubs_operator_openssl_env(self) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run
        captured_env: dict[str, str] = {}

        def fake_run(command, **kwargs):
            captured_env.update(kwargs["env"])
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=b"canonical-public-key-der",
                stderr=b"",
            )

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = fake_run
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not private key material\n", encoding="utf-8")
                errors: list[str] = []
                with mock.patch.dict(
                    os.environ,
                    {
                        "PATH": "/usr/bin",
                        **{
                            key: f"/tmp/unsafe-{key.lower()}"
                            for key in device_lab.FORBIDDEN_OPENSSL_CHILD_ENV_KEYS  # type: ignore[attr-defined]
                        },
                    },
                    clear=True,
                ):
                    der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                        public_key,
                        errors=errors,
                        label="trusted signer public key",
                    )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run

        self.assertEqual(errors, [])
        self.assertEqual(der, b"canonical-public-key-der")
        self.assertEqual(captured_env["PATH"], "/usr/bin")
        for key in device_lab.FORBIDDEN_OPENSSL_CHILD_ENV_KEYS:  # type: ignore[attr-defined]
            self.assertNotIn(key, captured_env)

    def test_openssl_public_key_der_rejects_invalid_public_key_after_openssl_failure(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run

        def failing_run(*args, **kwargs):
            return subprocess.CompletedProcess(
                args[0], 1, stdout=b"", stderr=b"invalid public key"
            )

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = failing_run
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("invalid public key data\n", encoding="utf-8")
                errors: list[str] = []

                der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                    public_key,
                    errors=errors,
                    label="trusted signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run

        self.assertIsNone(der)
        self.assertEqual(
            errors,
            ["trusted signer public key must be a valid OpenSSL public key"],
        )

    def test_openssl_public_key_der_rejects_missing_public_key_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for a missing public key")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                missing_public_key = Path(temp) / "missing-public.pem"
                errors: list[str] = []

                der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                    missing_public_key,
                    errors=errors,
                    label="trusted signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(der)
        self.assertEqual(
            errors,
            ["trusted signer public key must point to an existing public key file"],
        )

    def test_openssl_public_key_der_rejects_non_regular_public_key_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for a public key directory")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                public_key_directory = Path(temp) / "public-directory.pem"
                public_key_directory.mkdir()
                errors: list[str] = []

                der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                    public_key_directory,
                    errors=errors,
                    label="trusted signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(der)
        self.assertEqual(errors, ["trusted signer public key must be a regular file"])

    def test_openssl_public_key_der_rejects_oversized_public_key_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for an oversized public key")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_bytes(
                    b"x" * (device_lab.MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES + 1)
                )
                errors: list[str] = []

                der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                    public_key,
                    errors=errors,
                    label="trusted signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(der)
        self.assertEqual(
            errors,
            [
                "trusted signer public key must be no more than "
                f"{device_lab.MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES} bytes"
            ],
        )

    def test_openssl_public_key_der_rejects_file_metadata_failure_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up after public key metadata failure")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == public_key:
                        raise OSError("simulated public key lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                    public_key,
                    errors=errors,
                    label="trusted signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            path_type.lstat = original_lstat

        self.assertIsNone(der)
        self.assertEqual(
            errors,
            ["trusted signer public key file metadata could not be read"],
        )
        self.assertNotIn(str(public_key), "\n".join(errors))

    def test_verify_signature_rejects_staging_write_failure_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging write failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = unexpected_run
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                with mock.patch.object(
                    device_lab.os,
                    "fsync",
                    side_effect=OSError("simulated payload staging fsync failure"),
                ):
                    device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                        public_key_path=public_key,
                        payload=b"payload",
                        signature=b"signature",
                        errors=errors,
                        label="signer public key",
                    )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run

        self.assertEqual(
            errors,
            ["signature verification staging files could not be written"],
        )

    def test_verify_signature_rejects_payload_staging_readback_mismatch_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run
        original_read_staged_bytes = device_lab._read_staged_bytes  # type: ignore[attr-defined]

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging readback drift")

        def drifting_payload_read(
            path: Path,
            expected_stat: os.stat_result,
            verification_error: str,
        ) -> tuple[bytes | None, list[str]]:
            if path.name == "payload.bin":
                return b"mutated payload", []
            return original_read_staged_bytes(path, expected_stat, verification_error)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = unexpected_run
            device_lab._read_staged_bytes = drifting_payload_read  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                    public_key_path=public_key,
                    payload=b"payload",
                    signature=b"signature",
                    errors=errors,
                    label="signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run
            device_lab._read_staged_bytes = original_read_staged_bytes  # type: ignore[attr-defined]

        self.assertEqual(
            errors,
            ["signature verification staged payload did not match input"],
        )

    def test_verify_signature_rejects_signature_staging_readback_mismatch_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run
        original_read_staged_bytes = device_lab._read_staged_bytes  # type: ignore[attr-defined]

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging readback drift")

        def drifting_signature_read(
            path: Path,
            expected_stat: os.stat_result,
            verification_error: str,
        ) -> tuple[bytes | None, list[str]]:
            if path.name == "signature.bin":
                return b"mutated signature", []
            return original_read_staged_bytes(path, expected_stat, verification_error)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = unexpected_run
            device_lab._read_staged_bytes = drifting_signature_read  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                    public_key_path=public_key,
                    payload=b"payload",
                    signature=b"signature",
                    errors=errors,
                    label="signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run
            device_lab._read_staged_bytes = original_read_staged_bytes  # type: ignore[attr-defined]

        self.assertEqual(
            errors,
            ["signature verification staged signature did not match input"],
        )

    def test_write_staged_bytes_rejects_regular_file_swap_before_readback(
        self,
    ) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            stage = root / "payload.bin"
            replacement = root / "replacement.bin"
            replacement.write_bytes(b"replacement")
            swapped = False

            def swapping_open(path: Path, *args, **kwargs):
                nonlocal swapped
                mode = args[0] if args else kwargs.get("mode", "r")
                if path == stage and "r" in mode and not swapped:
                    replacement.replace(stage)
                    swapped = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", swapping_open):
                errors = device_lab._write_staged_bytes(  # type: ignore[attr-defined]
                    stage,
                    b"payload",
                    write_error="stage could not be written",
                    verification_error="stage readback mismatch",
                )
            stage_payload = stage.read_bytes()

        self.assertEqual(errors, ["stage readback mismatch"])
        self.assertEqual(stage_payload, b"replacement")

    def test_write_staged_bytes_rejects_hardlink_created_before_readback(
        self,
    ) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            stage = root / "payload.bin"
            alias = root / "payload-hardlink.bin"
            linked = False

            def hardlinking_open(path: Path, *args, **kwargs):
                nonlocal linked
                mode = args[0] if args else kwargs.get("mode", "r")
                if path == stage and "r" in mode and not linked:
                    try:
                        os.link(stage, alias)
                    except (AttributeError, NotImplementedError, OSError) as exc:
                        self.skipTest(
                            f"hardlinks are not available in this test environment: {exc}"
                        )
                    linked = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", hardlinking_open):
                errors = device_lab._write_staged_bytes(  # type: ignore[attr-defined]
                    stage,
                    b"payload",
                    write_error="stage could not be written",
                    verification_error="stage readback mismatch",
                )
            link_count = stage.stat().st_nlink

        self.assertEqual(errors, ["stage readback mismatch"])
        self.assertGreater(link_count, 1)

    def test_verify_signature_rejects_tempdir_failure_before_staging(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run
        original_tempdir = device_lab.tempfile.TemporaryDirectory

        def failing_tempdir(*args, **kwargs):
            raise OSError("simulated temporary directory failure")

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after tempdir failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = unexpected_run
            device_lab.tempfile.TemporaryDirectory = failing_tempdir
            with original_tempdir() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                    public_key_path=public_key,
                    payload=b"payload",
                    signature=b"signature",
                    errors=errors,
                    label="signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run
            device_lab.tempfile.TemporaryDirectory = original_tempdir

        self.assertEqual(
            errors,
            ["signature verification temporary directory could not be created"],
        )

    def test_verify_signature_rejects_spawn_failure_after_staging(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run

        def failing_run(*args, **kwargs):
            raise OSError("simulated OpenSSL spawn failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = failing_run
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                    public_key_path=public_key,
                    payload=b"payload",
                    signature=b"signature",
                    errors=errors,
                    label="signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run

        self.assertEqual(errors, ["signature verification command could not be run"])

    def test_verify_signature_scrubs_operator_openssl_env(self) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run
        captured_env: dict[str, str] = {}

        def fake_run(command, **kwargs):
            captured_env.update(kwargs["env"])
            return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = fake_run
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not private key material\n", encoding="utf-8")
                errors: list[str] = []
                with mock.patch.dict(
                    os.environ,
                    {
                        "PATH": "/usr/bin",
                        **{
                            key: f"/tmp/unsafe-{key.lower()}"
                            for key in device_lab.FORBIDDEN_OPENSSL_CHILD_ENV_KEYS  # type: ignore[attr-defined]
                        },
                    },
                    clear=True,
                ):
                    device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                        public_key_path=public_key,
                        payload=b"payload",
                        signature=b"x" * device_lab.ED25519_SIGNATURE_BYTES,
                        errors=errors,
                        label="signer public key",
                    )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run

        self.assertEqual(errors, [])
        self.assertEqual(captured_env["PATH"], "/usr/bin")
        for key in device_lab.FORBIDDEN_OPENSSL_CHILD_ENV_KEYS:  # type: ignore[attr-defined]
            self.assertNotIn(key, captured_env)

    def test_private_public_pair_preserves_public_key_path_error_before_mismatch(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors):
            self.fail("OpenSSL must not be resolved after a public-key path error")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                secret_public_key = Path(temp) / "token=supersecret-public.pem"
                errors: list[str] = []

                evidence_signer._validate_private_public_pair(  # type: ignore[attr-defined]
                    secret_public_key,
                    b"payload",
                    b"signature",
                    errors,
                )
                rendered = "\n".join(errors)
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertEqual(
            errors,
            ["signer public key path must not contain secret-looking material"],
        )
        self.assertNotIn(
            "private key did not produce a signature accepted by the signer public key",
            rendered,
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(secret_public_key), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_trusted_signer_public_key_rejects_symlinked_ancestor_without_path_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-keys"
            signer = create_test_signer(real_parent / "keys")
            linked_parent = root / "linked-keys"
            create_dir_symlink(self, linked_parent, real_parent)
            linked_public_key = linked_parent / signer["public_key"].relative_to(real_parent)

            trusted, errors = device_lab.load_trusted_signer_public_keys(
                [linked_public_key]
            )

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key ancestor directory must not be a symlink"],
        )
        rendered = "\n".join(errors)
        self.assertNotIn(str(linked_public_key), rendered)
        self.assertNotIn(str(real_parent), rendered)
        self.assertNotIn(str(linked_parent), rendered)

    def test_trusted_signer_public_key_rejects_hardlink_without_path_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            public_key_link = root / "trusted-public-key-hardlink.pem"
            write_text(public_key_link, "placeholder\n")
            replace_with_hardlink(self, public_key_link, signer["public_key"])

            trusted, errors = device_lab.load_trusted_signer_public_keys(
                [public_key_link]
            )

        self.assertEqual(trusted, {})
        self.assertEqual(errors, ["trusted signer public key must not be hardlinked"])
        self.assertNotIn(str(public_key_link), "\n".join(errors))

    def test_trusted_signer_public_key_rejects_hardlink_metadata_failure_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        path_type = type(Path("."))
        original_stat = path_type.stat

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up after metadata failure")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                signer = create_test_signer(Path(temp) / "keys")
                public_key = signer["public_key"]
                public_key_stat_calls = 0

                def failing_public_key_stat(path: Path, *args, **kwargs):
                    nonlocal public_key_stat_calls
                    if path == public_key and kwargs.get("follow_symlinks", True):
                        public_key_stat_calls += 1
                        if public_key_stat_calls > 0:
                            raise OSError("simulated public key stat failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_public_key_stat
                trusted, errors = device_lab.load_trusted_signer_public_keys(
                    [public_key]
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            path_type.stat = original_stat

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key hardlink metadata could not be read"],
        )
        self.assertNotIn(str(public_key), "\n".join(errors))

    def test_signer_helper_rejects_symlinked_private_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            private_key_link = Path(temp) / "signing-key-link.pem"
            try:
                private_key_link.symlink_to(signer["private_key"])
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(private_key_link),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("private key must not be a symlink", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_symlinked_private_key_ancestor_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            real_parent = Path(temp) / "real-keys"
            signer = create_test_signer(real_parent / "keys")
            linked_parent = Path(temp) / "linked-keys"
            create_dir_symlink(self, linked_parent, real_parent)
            linked_private_key = linked_parent / signer["private_key"].relative_to(
                real_parent
            )
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(linked_private_key),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        rendered = stderr.getvalue()
        self.assertEqual(status, 1)
        self.assertIn("private key ancestor directory must not be a symlink", rendered)
        self.assertNotIn(str(linked_private_key), rendered)
        self.assertNotIn(str(real_parent), rendered)
        self.assertNotIn(str(linked_parent), rendered)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_symlinked_public_key_ancestor_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            real_parent = Path(temp) / "real-keys"
            signer = create_test_signer(real_parent / "keys")
            linked_parent = Path(temp) / "linked-keys"
            create_dir_symlink(self, linked_parent, real_parent)
            linked_public_key = linked_parent / signer["public_key"].relative_to(
                real_parent
            )
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(linked_public_key),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        rendered = stderr.getvalue()
        self.assertEqual(status, 1)
        self.assertIn(
            "signer public key ancestor directory must not be a symlink",
            rendered,
        )
        self.assertNotIn(str(linked_public_key), rendered)
        self.assertNotIn(str(real_parent), rendered)
        self.assertNotIn(str(linked_parent), rendered)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_hardlinked_public_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            public_key_link = Path(temp) / "public-key-hardlink.pem"
            write_text(public_key_link, "placeholder\n")
            replace_with_hardlink(self, public_key_link, signer["public_key"])

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(public_key_link),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("signer public key must not be hardlinked", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_secret_looking_public_key_path_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            secret_public_key = Path(temp) / "token=supersecret-public.pem"
            secret_public_key.write_text("not an ed25519 public key\n", encoding="utf-8")

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(secret_public_key),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "signer public key path must not contain secret-looking material",
            rendered,
        )
        self.assertNotIn(str(secret_public_key), rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_control_public_key_path_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            control_public_key = Path(temp) / "control\npublic.pem"
            control_public_key.write_text("not an ed25519 public key\n", encoding="utf-8")

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(control_public_key),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("signer public key path must not contain control characters", rendered)
        self.assertNotIn(str(control_public_key), rendered)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_key_aliases_before_metadata_read(self) -> None:
        cases = (
            (
                "private",
                Path(" private.pem"),
                "private key path must not contain surrounding whitespace",
            ),
            (
                "private",
                Path("keys") / ".." / "private.pem",
                "private key path must be canonical",
            ),
            (
                "private",
                Path("keys\\private.pem"),
                "private key path must not contain backslashes",
            ),
            (
                "public",
                Path(" public.pem"),
                "signer public key path must not contain surrounding whitespace",
            ),
            (
                "public",
                Path("keys") / ".." / "public.pem",
                "signer public key path must be canonical",
            ),
            (
                "public",
                Path("keys\\public.pem"),
                "signer public key path must not contain backslashes",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = root / "pixel8"
            for key_kind, key_path, expected_error in cases:
                key_path = root / key_path
                private_key = key_path if key_kind == "private" else root / "private.pem"
                public_key = key_path if key_kind == "public" else root / "public.pem"
                with self.subTest(key_kind=key_kind, key_path=key_path):
                    with mock.patch.object(
                        evidence_signer,
                        "_require_slot_metadata",
                        side_effect=AssertionError("slot metadata must not be read"),
                    ) as require_slot_metadata:
                        status, output_relative, errors = evidence_signer.sign_slot_evidence(
                            slot_path=slot,
                            private_key_path=private_key,
                            public_key_path=public_key,
                            signer_key_id="android-lab-release-signer-v1",
                            signed_at_utc="2026-06-06T00:00:00Z",
                            output=None,
                            update_slot_json=False,
                            update_sha256sum=False,
                        )
                    rendered = "\n".join(errors)

                    require_slot_metadata.assert_not_called()
                    self.assertEqual(status, 1)
                    self.assertIsNone(output_relative)
                    self.assertEqual(errors, [expected_error])
                    self.assertNotIn(str(key_path), rendered)

    def test_signer_helper_rejects_secret_looking_slot_path_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            secret_slot = Path(temp) / "token=supersecret-slot"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(secret_slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("slot path must not contain secret-looking material", rendered)
        self.assertNotIn(str(secret_slot), rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_control_slot_path_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            control_slot = Path(temp) / "control\nslot"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(control_slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("slot path must not contain control characters", rendered)
        self.assertNotIn(str(control_slot), rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_alias_slot_path_before_metadata_read(
        self,
    ) -> None:
        cases = (
            (
                lambda base: base / " slot",
                "slot path must not contain surrounding whitespace",
            ),
            (
                lambda base: base / "slot\\alias",
                "slot path must not contain backslashes",
            ),
            (
                lambda base: base / "slot" / ".." / "alias",
                "slot path must be canonical",
            ),
        )
        path_type = type(Path("."))
        original_lstat = path_type.lstat
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    root = Path(temp)
                    signer = create_test_signer(root / "keys")
                    slot = path_factory(root)

                    def failing_lstat(path: Path, *args, **kwargs):
                        if path == slot:
                            raise AssertionError(
                                "alias signer slot path should fail before metadata"
                            )
                        return original_lstat(path, *args, **kwargs)

                    stdout = io.StringIO()
                    stderr = io.StringIO()
                    with mock.patch.object(path_type, "lstat", failing_lstat):
                        with redirect_stdout(stdout), redirect_stderr(stderr):
                            status = evidence_signer.main(
                                [
                                    "--slot",
                                    str(slot),
                                    "--private-key",
                                    str(signer["private_key"]),
                                    "--public-key",
                                    str(signer["public_key"]),
                                    "--signer-key-id",
                                    "android-lab-release-signer-v1",
                                    "--signed-at-utc",
                                    "2026-06-06T00:00:00Z",
                                ]
                            )
                    rendered = stdout.getvalue() + stderr.getvalue()

                self.assertEqual(status, 1)
                self.assertIn(expected_error, rendered)
                self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_slot_directory_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            stderr = io.StringIO()

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot:
                    raise OSError("simulated slot directory lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = evidence_signer.main(
                        [
                            "--slot",
                            str(slot),
                            "--private-key",
                            str(signer["private_key"]),
                            "--public-key",
                            str(signer["public_key"]),
                            "--signer-key-id",
                            "android-lab-release-signer-v1",
                            "--signed-at-utc",
                            "2026-06-06T00:00:00Z",
                        ]
                    )
            finally:
                path_type.lstat = original_lstat

        self.assertEqual(status, 1)
        self.assertIn("slot directory metadata could not be read", stderr.getvalue())
        self.assertNotIn("slot.json could not be read", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_slot_parent_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            stderr = io.StringIO()

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot.parent:
                    raise OSError("simulated slot parent lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = evidence_signer.main(
                        [
                            "--slot",
                            str(slot),
                            "--private-key",
                            str(signer["private_key"]),
                            "--public-key",
                            str(signer["public_key"]),
                            "--signer-key-id",
                            "android-lab-release-signer-v1",
                            "--signed-at-utc",
                            "2026-06-06T00:00:00Z",
                        ]
                    )
            finally:
                path_type.lstat = original_lstat

        self.assertEqual(status, 1)
        self.assertIn("slot parent directory metadata could not be read", stderr.getvalue())
        self.assertNotIn("slot.json could not be read", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_secret_looking_output_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = Path(temp) / "slots" / "pixel8"
            secret_output = "evidence/token=supersecret-signed-evidence.json"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        secret_output,
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "signed evidence output path must not contain secret-looking material",
            rendered,
        )
        self.assertNotIn(secret_output, rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_control_output_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = Path(temp) / "slots" / "pixel8"
            control_output = "evidence/control\nsigned-evidence.json"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        control_output,
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("signed evidence output path must not contain control characters", rendered)
        self.assertNotIn(control_output, rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_output_aliases_before_metadata_read(
        self,
    ) -> None:
        cases = (
            (
                " evidence/signed-evidence.json ",
                "signed evidence output path must not contain surrounding whitespace",
            ),
            (
                "evidence/ signed-evidence.json",
                "signed evidence output path must not contain surrounding whitespace",
            ),
            (
                "evidence\\signed-evidence.json",
                "signed evidence output path must not contain backslashes",
            ),
            (
                "evidence/../evidence/signed-evidence.json",
                "signed evidence output path must be canonical",
            ),
        )
        for output, expected_error in cases:
            with self.subTest(output=output):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp) / "keys")
                    slot = Path(temp) / "slots" / "pixel8"

                    stdout = io.StringIO()
                    stderr = io.StringIO()
                    with mock.patch.object(
                        evidence_signer,
                        "_require_slot_metadata",
                        side_effect=AssertionError("slot metadata must not be read"),
                    ), redirect_stdout(stdout), redirect_stderr(stderr):
                        status = evidence_signer.main(
                            [
                                "--slot",
                                str(slot),
                                "--private-key",
                                str(signer["private_key"]),
                                "--public-key",
                                str(signer["public_key"]),
                                "--signer-key-id",
                                "android-lab-release-signer-v1",
                                "--signed-at-utc",
                                "2026-06-06T00:00:00Z",
                                "--output",
                                output,
                            ]
                        )
                    rendered = stdout.getvalue() + stderr.getvalue()

                self.assertEqual(status, 1)
                self.assertIn(expected_error, rendered)
                self.assertNotIn(output, rendered)
                self.assertNotIn("slot metadata must not be read", rendered)
                self.assertNotIn("slot directory missing", rendered)

    def test_signer_explicit_output_arg_errors_reject_aliases_directly(self) -> None:
        cases = (
            (
                " evidence/signed-evidence.json ",
                "signed evidence output path must not contain surrounding whitespace",
            ),
            (
                "evidence/ signed-evidence.json",
                "signed evidence output path must not contain surrounding whitespace",
            ),
            (
                "evidence/token=supersecret-signed-evidence.json",
                "signed evidence output path must not contain secret-looking material",
            ),
            (
                "evidence/control\nsigned-evidence.json",
                "signed evidence output path must not contain control characters",
            ),
            (
                "evidence\\signed-evidence.json",
                "signed evidence output path must not contain backslashes",
            ),
            (
                "evidence/../evidence/signed-evidence.json",
                "signed evidence output path must be canonical",
            ),
        )
        for output, expected_error in cases:
            with self.subTest(output=output):
                errors = evidence_signer._explicit_output_arg_errors(output)  # type: ignore[attr-defined]

                rendered = "\n".join(errors)
                self.assertEqual(errors, [expected_error])
                self.assertNotIn(output, rendered)

    def test_signer_helper_rejects_secret_looking_signer_key_id_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = Path(temp) / "slots" / "pixel8"
            secret_signer_key_id = "token=supersecret"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        secret_signer_key_id,
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "signer key id must be non-empty and must not contain secret-looking material",
            rendered,
        )
        self.assertNotIn(secret_signer_key_id, rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_padded_signer_key_id_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = Path(temp) / "slots" / "pixel8"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        " android-lab-release-signer-v1 ",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("signer key id must not contain surrounding whitespace", rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_control_signer_key_id_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = Path(temp) / "slots" / "pixel8"
            unsafe_signer_key_id = "android-lab\x1b[31m"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        unsafe_signer_key_id,
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("signer key id must not contain control characters", rendered)
        self.assertNotIn(unsafe_signer_key_id, rendered)
        self.assertNotIn("\x1b", rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_output_outside_evidence_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        "telemetry/signed-evidence.json",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "signed evidence output path must stay under evidence/",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "telemetry" / "signed-evidence.json").exists())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_noncanonical_output_filename_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        "evidence/signed-evidence-copy.json",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "signed evidence output path must be evidence/signed-evidence.json",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence-copy.json").exists())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_backslash_output_path_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        "evidence\\signed-evidence.json",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "signed evidence output path must not contain backslashes",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_absolute_parent_segment_output_path_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            alias_output = (
                slot
                / "evidence"
                / ".."
                / "evidence"
                / "signed-evidence.json"
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        str(alias_output),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("signed evidence output path must be canonical", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_padded_metadata_output_path_before_write(self) -> None:
        cases = (
            " evidence/signed-evidence.json ",
            "evidence/ signed-evidence.json",
        )
        for relative in cases:
            with self.subTest(relative=relative):
                with tempfile.TemporaryDirectory() as temp:
                    root = Path(temp) / "slots"
                    signer = create_test_signer(Path(temp) / "keys")
                    slot = create_slot(root, "pixel8")
                    write_unsigned_production_slot_metadata(
                        slot,
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                    )
                    metadata_path = slot / "slot.json"
                    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
                    metadata["signed_evidence_artifact_path"] = relative
                    write_json(metadata_path, metadata)
                    rewrite_sha256sum(slot)

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = evidence_signer.main(
                            [
                                "--slot",
                                str(slot),
                                "--private-key",
                                str(signer["private_key"]),
                                "--public-key",
                                str(signer["public_key"]),
                                "--signer-key-id",
                                "android-lab-release-signer-v1",
                                "--signed-at-utc",
                                "2026-06-06T00:00:00Z",
                            ]
                        )

                self.assertEqual(status, 1)
                self.assertIn(
                    "slot.json signed_evidence_artifact_path must not contain surrounding whitespace",
                    stderr.getvalue(),
                )
                self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_output_normalise_rejects_root_only_evidence_output(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "slots" / "pixel8"
            slot.mkdir(parents=True)
            errors: list[str] = []

            result = evidence_signer._normalise_output_path(  # type: ignore[attr-defined]
                slot,
                {"signed_evidence_artifact_path": "evidence"},
                None,
                errors,
            )

        self.assertIsNone(result)
        self.assertEqual(errors, ["signed evidence output path must stay under evidence/"])
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_kagemusha_wallet_apk_path_outside_evidence_before_write(
        self,
    ) -> None:
        for forged_path in ("logs/runtime.log", "evidence"):
            with (
                self.subTest(forged_path=forged_path),
                tempfile.TemporaryDirectory() as temp,
            ):
                root = Path(temp)
                signer = create_test_signer(root / "keys")
                slot = create_slot(root / "slots", "pixel8")
                write_unsigned_production_slot_metadata(
                    slot,
                    "pixel8",
                    device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                )
                metadata_path = slot / "slot.json"
                metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
                metadata["kagemusha_wallet_apk_path"] = forged_path
                if forged_path == "logs/runtime.log":
                    metadata["kagemusha_wallet_apk_sha256"] = hashlib.sha256(
                        (slot / "logs" / "runtime.log").read_bytes()
                    ).hexdigest()
                write_json(metadata_path, metadata)
                rewrite_sha256sum(slot)

                stderr = io.StringIO()
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = evidence_signer.main(
                        [
                            "--slot",
                            str(slot),
                            "--private-key",
                            str(signer["private_key"]),
                            "--public-key",
                            str(signer["public_key"]),
                            "--signer-key-id",
                            "android-lab-release-signer-v1",
                            "--signed-at-utc",
                            "2026-06-06T00:00:00Z",
                        ]
                    )

                self.assertEqual(status, 1)
                self.assertIn(
                    "slot.json kagemusha_wallet_apk_path must stay under evidence/",
                    stderr.getvalue(),
                )
                self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_output_normalise_rejects_output_resolve_failure(self) -> None:
        path_type = type(Path("."))
        original_resolve = path_type.resolve

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = Path(temp) / "slots" / "pixel8"
                slot.mkdir(parents=True)
                output = slot / "evidence" / "signed-evidence.json"
                errors: list[str] = []

                def failing_resolve(path: Path, *args, **kwargs):
                    if path == output:
                        raise OSError("simulated output resolve failure")
                    return original_resolve(path, *args, **kwargs)

                path_type.resolve = failing_resolve

                result = evidence_signer._normalise_output_path(  # type: ignore[attr-defined]
                    slot,
                    {},
                    str(output),
                    errors,
                )
        finally:
            path_type.resolve = original_resolve

        self.assertIsNone(result)
        self.assertEqual(errors, ["signed evidence output path could not be resolved"])
        self.assertFalse(output.exists())

    def test_signer_output_normalise_rejects_slot_resolve_failure(self) -> None:
        path_type = type(Path("."))
        original_resolve = path_type.resolve

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = Path(temp) / "slots" / "pixel8"
                slot.mkdir(parents=True)
                output = slot / "evidence" / "signed-evidence.json"
                output_resolved = False
                errors: list[str] = []

                def failing_resolve(path: Path, *args, **kwargs):
                    nonlocal output_resolved
                    if path == output and not output_resolved:
                        output_resolved = True
                        return original_resolve(path, *args, **kwargs)
                    if path == slot:
                        raise OSError("simulated slot resolve failure")
                    return original_resolve(path, *args, **kwargs)

                path_type.resolve = failing_resolve

                result = evidence_signer._normalise_output_path(  # type: ignore[attr-defined]
                    slot,
                    {},
                    str(output),
                    errors,
                )
        finally:
            path_type.resolve = original_resolve

        self.assertIsNone(result)
        self.assertEqual(errors, ["signed evidence output path could not be resolved"])
        self.assertFalse(output.exists())

    def test_signer_output_normalise_rejects_absolute_symlinked_output_ancestor(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = root / "slots" / "pixel8"
            slot.joinpath("evidence").mkdir(parents=True)
            alias = root / "linked-slot"
            create_dir_symlink(self, alias, slot)
            errors: list[str] = []

            result = evidence_signer._normalise_output_path(  # type: ignore[attr-defined]
                slot,
                {},
                str(alias / "evidence" / "signed-evidence.json"),
                errors,
            )

        self.assertIsNone(result)
        self.assertEqual(
            errors,
            ["signed evidence output path ancestor directory must not be a symlink"],
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_output_normalise_rejects_absolute_symlinked_output_leaf(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = root / "slots" / "pixel8"
            output = slot / "evidence" / "signed-evidence.json"
            target = root / "external-signed-evidence.json"
            output.parent.mkdir(parents=True)
            target.write_text("external\n", encoding="utf-8")
            try:
                output.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")
            errors: list[str] = []

            result = evidence_signer._normalise_output_path(  # type: ignore[attr-defined]
                slot,
                {},
                str(output),
                errors,
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertIsNone(result)
        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(target_text, "external\n")

    def test_signer_write_json_rejects_symlinked_output_parent_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            external = root / "external-evidence"
            external.mkdir()
            output_parent = root / "slot" / "evidence"
            output_parent.parent.mkdir(parents=True)
            create_dir_symlink(self, output_parent, external)

            errors = evidence_signer._write_json(
                output_parent / "signed-evidence.json",
                {"schema": "test"},
                "signed evidence output path",
            )

        self.assertEqual(
            errors,
            ["signed evidence output path parent directory must not be a symlink"],
        )
        self.assertFalse((external / "signed-evidence.json").exists())

    def test_signer_write_json_uses_lstat_before_parent_is_dir_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                output.parent.mkdir(parents=True)

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == output.parent:
                        raise OSError("simulated output parent is_dir preflight failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertEqual(output_text, '{\n  "schema": "test"\n}\n')

    def test_signer_write_json_rejects_parent_metadata_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                output.parent.mkdir(parents=True)

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == output.parent:
                        raise OSError("simulated output parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_exists = output.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(
            errors,
            ["signed evidence output path parent directory metadata could not be read"],
        )
        self.assertFalse(output_exists)

    def test_signer_write_json_rejects_symlinked_output_ancestor_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = root / "real-slot"
            real_slot.joinpath("evidence").mkdir(parents=True)
            linked_slot = root / "linked-slot"
            create_dir_symlink(self, linked_slot, real_slot)

            errors = evidence_signer._write_json(
                linked_slot / "evidence" / "signed-evidence.json",
                {"schema": "test"},
                "signed evidence output path",
            )

            self.assertEqual(
                errors,
                ["signed evidence output path ancestor directory must not be a symlink"],
            )
            self.assertFalse((real_slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_write_json_rejects_symlinked_output_ancestor_before_creating_parent(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = root / "real-slot"
            real_slot.mkdir()
            linked_slot = root / "linked-slot"
            create_dir_symlink(self, linked_slot, real_slot)

            errors = evidence_signer._write_json(
                linked_slot / "evidence" / "signed-evidence.json",
                {"schema": "test"},
                "signed evidence output path",
            )

            self.assertEqual(
                errors,
                ["signed evidence output path ancestor directory must not be a symlink"],
            )
            self.assertFalse((real_slot / "evidence").exists())

    def test_signer_write_json_rejects_symlinked_output_leaf_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            target = root / "external-signed-evidence.json"
            write_text(target, "external\n")
            output.parent.mkdir(parents=True)
            try:
                output.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            errors = evidence_signer._write_json(
                output,
                {"schema": "test"},
                "signed evidence output path",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(target_text, "external\n")

    def test_signer_write_json_rejects_hardlinked_output_leaf_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            target = root / "external-signed-evidence.json"
            write_text(target, "external\n")
            write_text(output, "placeholder\n")
            replace_with_hardlink(self, output, target)

            errors = evidence_signer._write_json(
                output,
                {"schema": "test"},
                "signed evidence output path",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(errors, ["signed evidence output path must not be hardlinked"])
        self.assertEqual(target_text, "external\n")

    def test_signer_write_json_rejects_hardlink_metadata_failure_before_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, "placeholder\n")

                def failing_stat(path: Path, *args, **kwargs):
                    if path == output and kwargs.get("follow_symlinks", True):
                        raise OSError("simulated output hardlink metadata failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            path_type.stat = original_stat

        self.assertEqual(
            errors,
            ["signed evidence output path hardlink metadata could not be read"],
        )
        self.assertEqual(output_text, "placeholder\n")

    def test_signer_write_json_rejects_file_metadata_failure_before_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, "placeholder\n")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == output:
                        raise OSError("simulated output file metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(
            errors,
            ["signed evidence output path file metadata could not be read"],
        )
        self.assertEqual(output_text, "placeholder\n")

    def test_signer_write_json_rejects_secret_output_path_directly_without_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = (
                Path(temp)
                / "slot"
                / "token=supersecret-output"
                / "signed-evidence.json"
            )

            errors = evidence_signer._write_json(
                output,
                {"schema": "test"},
                "signed evidence output path",
            )
            rendered = "\n".join(errors)

        self.assertEqual(
            errors,
            ["signed evidence output path must not contain secret-looking material"],
        )
        self.assertFalse(output.exists())
        self.assertFalse(output.parent.exists())
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(output), rendered)

    def test_signer_write_json_rejects_control_output_path_directly_without_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = (
                Path(temp)
                / "slot"
                / "control\noutput"
                / "signed-evidence.json"
            )

            errors = evidence_signer._write_json(
                output,
                {"schema": "test"},
                "signed evidence output path",
            )
            rendered = "\n".join(errors)

        self.assertEqual(
            errors,
            ["signed evidence output path must not contain control characters"],
        )
        self.assertFalse(output.exists())
        self.assertFalse(output.parent.exists())
        self.assertNotIn(str(output), rendered)

    def test_signer_write_json_rejects_nonfinite_json_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

            errors = evidence_signer._write_json(
                output,
                {"schema": "test", "value": float("nan")},
                "signed evidence output path",
            )
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))

        self.assertEqual(errors, ["signed evidence output path is not strict JSON"])
        self.assertFalse(output.exists())
        self.assertEqual(temp_files, [])

    def test_signer_write_json_rejects_oversized_json_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            payload = {"schema": "test"}
            text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
            limit = len(text.encode("utf-8")) - 1

            with mock.patch.object(
                evidence_signer.device_lab,
                "MAX_ANDROID_DEVICE_LAB_JSON_BYTES",
                limit,
            ):
                errors = evidence_signer._write_json(
                    output,
                    payload,
                    "signed evidence output path",
                )
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))

        self.assertEqual(
            errors,
            [f"signed evidence output path must be no more than {limit} bytes"],
        )
        self.assertFalse(output.exists())
        self.assertEqual(temp_files, [])

    def test_signer_write_json_rejects_write_failure_after_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

            with mock.patch.object(
                evidence_signer.os,
                "fsync",
                side_effect=OSError("simulated signed evidence fsync failure"),
            ):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )

        self.assertEqual(
            errors,
            [
                "signed evidence output path could not be written",
                "signed evidence output path temporary file cleanup could not be synced",
            ],
        )
        self.assertFalse(output.exists())

    def test_signer_write_json_preserves_existing_output_on_replace_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            write_text(output, "existing signed evidence\n")

            with mock.patch.object(
                evidence_signer.os,
                "replace",
                side_effect=OSError("simulated signed evidence replace failure"),
            ):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            output_text = output.read_text(encoding="utf-8")
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))

        self.assertEqual(errors, ["signed evidence output path could not be written"])
        self.assertEqual(output_text, "existing signed evidence\n")
        self.assertEqual(temp_files, [])

    def test_signer_write_json_reports_temp_cleanup_failure_after_write_failure(
        self,
    ) -> None:
        original_unlink = evidence_signer.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

            def failing_replace(src: Path, dst: Path, *args, **kwargs) -> None:
                raise OSError("simulated signer replace failure")

            def failing_temp_unlink(path: str, *args, **kwargs):
                if (
                    Path(path).name.startswith(f".{output.name}.")
                    and Path(path).suffix == ".tmp"
                ):
                    raise OSError("simulated signer temp cleanup failure")
                return original_unlink(path, *args, **kwargs)

            with (
                mock.patch.object(evidence_signer.os, "replace", failing_replace),
                mock.patch.object(evidence_signer.os, "unlink", failing_temp_unlink),
            ):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))

        self.assertEqual(
            errors,
            [
                "signed evidence output path could not be written",
                "signed evidence output path temporary file could not be removed",
            ],
        )
        self.assertEqual(len(temp_files), 1)

    def test_signer_json_output_validators_reject_alias_paths_before_metadata(
        self,
    ) -> None:
        cases = (
            (
                "write-whitespace",
                evidence_signer._validate_json_output_path,  # type: ignore[attr-defined]
                lambda base: base / "slot" / "evidence" / " signed-evidence.json",
                "signed evidence output path must not contain surrounding whitespace",
            ),
            (
                "write-backslash",
                evidence_signer._validate_json_output_path,  # type: ignore[attr-defined]
                lambda base: base / "slot" / "evidence\\signed-evidence.json",
                "signed evidence output path must not contain backslashes",
            ),
            (
                "write-parent",
                evidence_signer._validate_json_output_path,  # type: ignore[attr-defined]
                lambda base: base
                / "slot"
                / "evidence"
                / ".."
                / "evidence"
                / "signed-evidence.json",
                "signed evidence output path must be canonical",
            ),
            (
                "existing-whitespace",
                evidence_signer._validate_existing_json_output_path,  # type: ignore[attr-defined]
                lambda base: base / "slot" / "evidence" / " signed-evidence.json",
                "signed evidence output path must not contain surrounding whitespace",
            ),
            (
                "existing-backslash",
                evidence_signer._validate_existing_json_output_path,  # type: ignore[attr-defined]
                lambda base: base / "slot" / "evidence\\signed-evidence.json",
                "signed evidence output path must not contain backslashes",
            ),
            (
                "existing-parent",
                evidence_signer._validate_existing_json_output_path,  # type: ignore[attr-defined]
                lambda base: base
                / "slot"
                / "evidence"
                / ".."
                / "evidence"
                / "signed-evidence.json",
                "signed evidence output path must be canonical",
            ),
        )
        for name, validator, path_factory, expected_error in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as temp:
                    output = path_factory(Path(temp))

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias signed-evidence output should fail before metadata"
                        ),
                    ):
                        errors = validator(output, "signed evidence output path")

                self.assertEqual(errors, [expected_error])

    def test_signer_write_json_reports_temp_cleanup_failure_after_post_stage_validation_failure(
        self,
    ) -> None:
        original_validate = evidence_signer._validate_json_output_path
        original_unlink = evidence_signer.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            validation_calls = 0

            def racing_validate(path: Path, label: str) -> list[str]:
                nonlocal validation_calls
                if path == output and label == "signed evidence output path":
                    validation_calls += 1
                    if validation_calls == 2:
                        return ["signed evidence output path changed after staging"]
                return original_validate(path, label)

            def failing_temp_unlink(path: str, *args, **kwargs):
                if (
                    Path(path).name.startswith(f".{output.name}.")
                    and Path(path).suffix == ".tmp"
                ):
                    raise OSError("simulated signer temp cleanup failure")
                return original_unlink(path, *args, **kwargs)

            with (
                mock.patch.object(
                    evidence_signer,
                    "_validate_json_output_path",
                    racing_validate,
                ),
                mock.patch.object(evidence_signer.os, "unlink", failing_temp_unlink),
            ):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))
            output_exists = output.exists()

        self.assertEqual(
            errors,
            [
                "signed evidence output path changed after staging",
                "signed evidence output path temporary file could not be removed",
            ],
        )
        self.assertEqual(validation_calls, 2)
        self.assertFalse(output_exists)
        self.assertEqual(len(temp_files), 1)

    def test_signer_write_json_temp_cleanup_rejects_swapped_temp_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp) / ".signed-evidence.json.swap.tmp"
            temp_path.write_text("original\n", encoding="utf-8")
            temp_identity = evidence_signer._file_identity(temp_path.lstat())
            swapped_temp = Path(temp) / "original-signed-evidence-temp-file"
            temp_path.rename(swapped_temp)
            temp_path.write_text("do not remove\n", encoding="utf-8")

            errors = evidence_signer._cleanup_temp_output(
                temp_path,
                "signed evidence output path",
                temp_identity,
            )
            victim_survived = temp_path.read_text(encoding="utf-8")
            original_survived = swapped_temp.read_text(encoding="utf-8")

        self.assertEqual(
            errors,
            ["signed evidence output path temporary file changed before cleanup"],
        )
        self.assertEqual(victim_survived, "do not remove\n")
        self.assertEqual(original_survived, "original\n")

    def test_signer_write_json_temp_cleanup_reports_sync_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp) / ".signed-evidence.json.sync.tmp"
            temp_path.write_text("scratch\n", encoding="utf-8")
            temp_identity = evidence_signer._file_identity(temp_path.lstat())

            with mock.patch.object(
                evidence_signer.os,
                "fsync",
                side_effect=OSError("simulated signer temp cleanup sync failure"),
            ):
                errors = evidence_signer._cleanup_temp_output(
                    temp_path,
                    "signed evidence output path",
                    temp_identity,
                )
            temp_exists = temp_path.exists()

        self.assertEqual(
            errors,
            ["signed evidence output path temporary file cleanup could not be synced"],
        )
        self.assertFalse(temp_exists)

    def test_signer_write_json_rejects_parent_directory_sync_failure_after_replace(
        self,
    ) -> None:
        original_fsync = evidence_signer.os.fsync

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            sync_calls = 0

            def failing_parent_fsync(fd: int) -> None:
                nonlocal sync_calls
                sync_calls += 1
                if sync_calls == 2:
                    raise OSError("simulated signer parent sync failure")
                original_fsync(fd)

            with mock.patch.object(evidence_signer.os, "fsync", failing_parent_fsync):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            output_exists = output.exists()
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))

        self.assertEqual(sync_calls, 3)
        self.assertEqual(
            errors,
            ["signed evidence output path parent directory could not be synced"],
        )
        self.assertFalse(output_exists)
        self.assertEqual(temp_files, [])

    def test_signer_write_json_parent_sync_cleanup_reports_failure(self) -> None:
        original_sync = evidence_signer._sync_output_parent_fd
        original_unlink = evidence_signer.os.unlink

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

                def failing_sync(_parent_fd, label, **_kwargs):  # type: ignore[no-untyped-def]
                    return [f"{label} parent directory could not be synced"]

                def failing_unlink(path: str, *args, **kwargs):  # type: ignore[no-untyped-def]
                    if path == output.name and kwargs.get("dir_fd") is not None:
                        raise OSError("simulated signed evidence rollback failure")
                    return original_unlink(path, *args, **kwargs)

                evidence_signer._sync_output_parent_fd = failing_sync  # type: ignore[attr-defined]
                evidence_signer.os.unlink = failing_unlink

                try:
                    errors = evidence_signer._write_json(
                        output,
                        {"schema": "test"},
                        "signed evidence output path",
                    )
                    output_exists = output.exists()
                finally:
                    evidence_signer._sync_output_parent_fd = original_sync  # type: ignore[attr-defined]
                    evidence_signer.os.unlink = original_unlink
        finally:
            evidence_signer._sync_output_parent_fd = original_sync  # type: ignore[attr-defined]
            evidence_signer.os.unlink = original_unlink

        self.assertEqual(
            errors,
            [
                "signed evidence output path parent directory could not be synced",
                "signed evidence output path could not be removed after parent sync failure",
            ],
        )
        self.assertTrue(output_exists)

    def test_signer_write_json_published_cleanup_preserves_swap(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            output_name = "signed-evidence.json"
            output = temp_path / output_name
            output.write_text('{"schema":"test"}\n', encoding="utf-8")
            output_identity = evidence_signer._file_identity(output.lstat())
            original_output = temp_path / "original-signed-evidence.json"
            output.rename(original_output)
            output.write_text("do not remove\n", encoding="utf-8")
            parent_fd = os.open(temp_path, evidence_signer._directory_open_flags())
            try:
                errors = evidence_signer._unlink_file_if_identity_at(
                    parent_fd,
                    output_name,
                    output_identity,
                    "signed evidence output path",
                )
            finally:
                os.close(parent_fd)
            replacement = output.read_text(encoding="utf-8")
            original = original_output.read_text(encoding="utf-8")

        self.assertEqual(errors, [])
        self.assertEqual(replacement, "do not remove\n")
        self.assertEqual(original, '{"schema":"test"}\n')

    def test_signer_write_json_published_cleanup_reports_sync_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            output_name = "signed-evidence.json"
            output = temp_path / output_name
            output.write_text('{"schema":"test"}\n', encoding="utf-8")
            output_identity = evidence_signer._file_identity(output.lstat())
            parent_fd = os.open(temp_path, evidence_signer._directory_open_flags())
            try:
                with mock.patch.object(
                    evidence_signer.os,
                    "fsync",
                    side_effect=OSError("simulated signer cleanup sync failure"),
                ):
                    errors = evidence_signer._unlink_file_if_identity_at(
                        parent_fd,
                        output_name,
                        output_identity,
                        "signed evidence output path",
                    )
            finally:
                os.close(parent_fd)
            output_exists = output.exists()

        self.assertEqual(
            errors,
            [
                "signed evidence output path cleanup could not be synced after parent sync failure"
            ],
        )
        self.assertFalse(output_exists)

    def test_signer_write_json_rejects_parent_directory_identity_swap_before_sync(
        self,
    ) -> None:
        original_replace = evidence_signer.os.replace

        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            root = wrapper / "signed-evidence-root"
            output = root / "signed-evidence.json"
            root.mkdir()
            swapped_root = wrapper / "signed-evidence-root-swapped"
            swapped = False

            def swapping_replace(src, dst, *args, **kwargs):
                nonlocal swapped
                original_replace(src, dst, *args, **kwargs)
                output.parent.rename(swapped_root)
                output.parent.mkdir()
                swapped = True

            with mock.patch.object(evidence_signer.os, "replace", swapping_replace):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            original_output_exists = (swapped_root / output.name).exists()
            swapped_output_exists = output.exists()

        self.assertTrue(swapped)
        self.assertEqual(
            errors,
            ["signed evidence output path parent directory changed before sync"],
        )
        self.assertFalse(original_output_exists)
        self.assertFalse(swapped_output_exists)

    def test_signer_write_json_rejects_symlink_swap_before_replace(self) -> None:
        original_validate = evidence_signer._validate_json_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                output = root / "slot" / "evidence" / "signed-evidence.json"
                target = root / "external-signed-evidence.json"
                output.parent.mkdir(parents=True)
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == output and calls == 2:
                        write_text(target, "external\n")
                        try:
                            path.symlink_to(target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                evidence_signer._validate_json_output_path = validate_then_alias
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                target_text = target.read_text(encoding="utf-8")
                temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))
        finally:
            evidence_signer._validate_json_output_path = original_validate

        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertEqual(temp_files, [])

    def test_signer_write_json_rejects_readback_mismatch(self) -> None:
        original_read_output_text = evidence_signer._read_existing_output_text

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

                def mismatching_read_output_text(
                    path: Path,
                    expected_stat: os.stat_result,
                    label: str,
                    *,
                    max_bytes: int | None = None,
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return "mismatched signed evidence\n", []
                    return original_read_output_text(
                        path,
                        expected_stat,
                        label,
                        max_bytes=max_bytes,
                    )

                evidence_signer._read_existing_output_text = mismatching_read_output_text
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            evidence_signer._read_existing_output_text = original_read_output_text

        self.assertEqual(
            errors,
            ["signed evidence output path write verification failed"],
        )
        self.assertEqual(output_text, '{\n  "schema": "test"\n}\n')

    def test_signer_write_json_rejects_readback_failure(self) -> None:
        original_read_output_text = evidence_signer._read_existing_output_text

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

                def failing_read_output_text(
                    path: Path,
                    _expected_stat: os.stat_result,
                    label: str,
                    *,
                    max_bytes: int | None = None,
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return None, [f"{label} write verification failed"]
                    return original_read_output_text(
                        path,
                        _expected_stat,
                        label,
                        max_bytes=max_bytes,
                    )

                evidence_signer._read_existing_output_text = failing_read_output_text
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
                temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))
        finally:
            evidence_signer._read_existing_output_text = original_read_output_text

        self.assertEqual(
            errors,
            ["signed evidence output path write verification failed"],
        )
        self.assertEqual(output_text, '{\n  "schema": "test"\n}\n')
        self.assertEqual(temp_files, [])

    def test_signer_write_json_rejects_oversized_readback_after_replace(self) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            payload = {"schema": "test"}
            text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
            limit = len(text.encode("utf-8")) + 4
            mutated = False

            def appending_open(path: Path, *args, **kwargs):
                nonlocal mutated
                mode = args[0] if args else kwargs.get("mode", "r")
                if path == output and "r" in str(mode) and not mutated:
                    with original_open(path, "a", encoding="utf-8") as handle:
                        handle.write("X" * 16)
                    mutated = True
                return original_open(path, *args, **kwargs)

            with (
                mock.patch.object(
                    evidence_signer.device_lab,
                    "MAX_ANDROID_DEVICE_LAB_JSON_BYTES",
                    limit,
                ),
                mock.patch.object(Path, "open", appending_open),
            ):
                errors = evidence_signer._write_json(
                    output,
                    payload,
                    "signed evidence output path",
                )
            output_text = output.read_text(encoding="utf-8")

        self.assertTrue(mutated)
        self.assertGreater(
            len(output_text.encode("utf-8")),
            len(text.encode("utf-8")),
        )
        self.assertEqual(
            errors,
            [f"signed evidence output path must be no more than {limit} bytes"],
        )

    def test_signer_write_json_rejects_regular_file_swap_before_readback(
        self,
    ) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            replacement = root / "replacement-signed-evidence.json"
            write_text(replacement, '{"schema":"replacement"}\n')
            swapped = False

            def swapping_open(path: Path, *args, **kwargs):
                nonlocal swapped
                if path == output and not swapped:
                    replacement.replace(output)
                    swapped = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", swapping_open):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            output_text = output.read_text(encoding="utf-8")

        self.assertEqual(
            errors,
            ["signed evidence output path changed while being read"],
        )
        self.assertEqual(output_text, '{"schema":"replacement"}\n')

    def test_signer_write_json_rejects_symlink_swap_after_replace(self) -> None:
        original_validate = evidence_signer._validate_existing_json_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                output = root / "slot" / "evidence" / "signed-evidence.json"
                target = root / "external-signed-evidence.json"
                write_text(target, "external\n")
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == output and calls == 1:
                        path.unlink(missing_ok=True)
                        try:
                            path.symlink_to(target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                evidence_signer._validate_existing_json_output_path = validate_then_alias
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                target_text = target.read_text(encoding="utf-8")
                output_is_symlink = output.is_symlink()
                temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))
        finally:
            evidence_signer._validate_existing_json_output_path = original_validate

        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertTrue(output_is_symlink)
        self.assertEqual(temp_files, [])

    def test_signer_write_json_rejects_parent_create_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

            def failing_mkdir(path: Path, *args, **kwargs):
                if path == output.parent:
                    raise OSError("simulated signed evidence parent mkdir failure")
                return original_mkdir(path, *args, **kwargs)

            with mock.patch.object(path_type, "mkdir", failing_mkdir):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )

        self.assertEqual(
            errors,
            ["signed evidence output path parent directory could not be created"],
        )
        self.assertFalse(output.exists())

    def test_signer_write_json_rechecks_parent_after_create_before_write(self) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "late-linked-output" / "signed-evidence.json"
            alias_target = root / "external-output"
            alias_target.mkdir()

            def replacing_mkdir(path: Path, *args, **kwargs):
                if path == output.parent:
                    create_dir_symlink(self, path, alias_target)
                    return None
                return original_mkdir(path, *args, **kwargs)

            with mock.patch.object(path_type, "mkdir", replacing_mkdir):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )

        self.assertEqual(
            errors,
            ["signed evidence output path parent directory must not be a symlink"],
        )
        self.assertFalse((alias_target / "signed-evidence.json").exists())

    def test_signer_output_digest_rejects_secret_path_directly_without_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "token=supersecret-signed-evidence.json"
            write_text(output, '{"schema":"test"}\n')

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path must not contain secret-looking material"],
        )
        self.assertNotIn(str(output), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_signer_output_digest_rejects_missing_parent_before_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path parent directory is missing"],
        )

    def test_signer_output_digest_uses_lstat_before_parent_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir
        payload = '{"schema":"test"}\n'

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, payload)

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == output.parent:
                        raise OSError("simulated output digest parent is_dir preflight failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                digest, errors = evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                )
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertEqual(digest, hashlib.sha256(payload.encode("utf-8")).hexdigest())

    def test_signer_output_digest_rejects_parent_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, '{"schema":"test"}\n')

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == output.parent:
                        raise OSError("simulated output digest parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                digest, errors = evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path parent directory metadata could not be read"],
        )

    def test_signer_output_digest_rejects_missing_leaf_before_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            output.parent.mkdir(parents=True)

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path must exist before digest"],
        )

    def test_signer_output_digest_rejects_symlinked_leaf_after_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            target = root / "external-signed-evidence.json"
            write_text(output, '{"schema":"test"}\n')
            write_text(target, "external\n")
            replace_with_symlink(self, output, target)

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertIsNone(digest)
        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(target_text, "external\n")

    def test_signer_output_digest_rejects_hardlinked_leaf_after_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            target = root / "external-signed-evidence.json"
            write_text(output, '{"schema":"test"}\n')
            write_text(target, "external\n")
            replace_with_hardlink(self, output, target)

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertIsNone(digest)
        self.assertEqual(errors, ["signed evidence output path must not be hardlinked"])
        self.assertEqual(target_text, "external\n")

    def test_signer_output_digest_rejects_oversized_output_after_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            output.parent.mkdir(parents=True)
            with output.open("wb") as handle:
                handle.seek(device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES)
                handle.write(b"x")
            output.chmod(0o600)

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence output path must be no more than "
                f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
            ],
        )

    def test_signer_output_digest_rejects_hardlink_metadata_failure_after_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, '{"schema":"test"}\n')

                def failing_stat(path: Path, *args, **kwargs):
                    if path == output and kwargs.get("follow_symlinks", True):
                        raise OSError("simulated output digest hardlink metadata failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                digest, errors = evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            path_type.stat = original_stat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path hardlink metadata could not be read"],
        )
        self.assertEqual(output_text, '{"schema":"test"}\n')

    def test_signer_output_digest_rejects_file_metadata_failure_after_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, '{"schema":"test"}\n')

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == output:
                        raise OSError("simulated output digest file metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                digest, errors = evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path file metadata could not be read"],
        )
        self.assertEqual(output_text, '{"schema":"test"}\n')

    def test_signer_output_digest_rejects_read_failure_after_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            write_text(output, '{"schema":"test"}\n')

            digest, errors = with_open_failure(
                output,
                lambda: evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                ),
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["signed evidence output path could not be read"])

    def test_signer_output_digest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            replacement = root / "replacement-signed-evidence.json"
            write_text(output, '{"schema":"test"}\n')
            write_text(replacement, '{"schema":"replacement"}\n')
            swapped = False

            def swapping_open(path: Path, *args, **kwargs):
                nonlocal swapped
                if path == output and not swapped:
                    replacement.replace(output)
                    swapped = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", swapping_open):
                digest, errors = evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                )
            output_text = output.read_text(encoding="utf-8")

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path changed while being read"],
        )
        self.assertEqual(output_text, '{"schema":"replacement"}\n')

    def test_signer_helper_revalidates_output_digest_before_slot_json_update(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            original_metadata = (slot / "slot.json").read_text(encoding="utf-8")
            alias_target = root / "external-signed-evidence.json"
            original_build_signed_evidence = evidence_signer.build_signed_evidence
            original_write_json = evidence_signer._write_json

            def fake_build_signed_evidence(*_args, **_kwargs):
                return {"schema": device_lab.SIGNED_EVIDENCE_SCHEMA}

            def write_json_then_alias(path: Path, payload: dict, label: str) -> list[str]:
                errors = original_write_json(path, payload, label)
                if not errors and label == "signed evidence output path":
                    alias_target.write_text(path.read_text(encoding="utf-8"), encoding="utf-8")
                    path.unlink()
                    try:
                        path.symlink_to(alias_target)
                    except (NotImplementedError, OSError) as exc:
                        self.skipTest(
                            f"symlinks are not available in this test environment: {exc}"
                        )
                return errors

            try:
                evidence_signer.build_signed_evidence = fake_build_signed_evidence
                evidence_signer._write_json = write_json_then_alias
                status, output_relative, errors = evidence_signer.sign_slot_evidence(
                    slot_path=slot,
                    private_key_path=root / "private.pem",
                    public_key_path=root / "public.pem",
                    signer_key_id="android-lab-release-signer-v1",
                    signed_at_utc="2026-06-06T00:00:00Z",
                    output=None,
                    update_slot_json=True,
                    update_sha256sum=False,
                )
            finally:
                evidence_signer.build_signed_evidence = original_build_signed_evidence
                evidence_signer._write_json = original_write_json
            metadata_after = (slot / "slot.json").read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertEqual(output_relative, "evidence/signed-evidence.json")
        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(metadata_after, original_metadata)

    def test_signer_write_text_rejects_symlinked_manifest_leaf_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "sha256sum.txt"
            target = root / "external-sha256sum.txt"
            write_text(target, "external\n")
            output.parent.mkdir(parents=True)
            try:
                output.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            errors = evidence_signer._write_text(
                output,
                "replacement\n",
                "sha256sum.txt",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(errors, ["sha256sum.txt must not be a symlink"])
        self.assertEqual(target_text, "external\n")

    def test_signer_write_text_rejects_dangling_symlinked_manifest_leaf_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "sha256sum.txt"
            target = root / "missing-sha256sum.txt"
            output.parent.mkdir(parents=True)
            try:
                output.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            errors = evidence_signer._write_text(
                output,
                "replacement\n",
                "sha256sum.txt",
            )

        self.assertEqual(errors, ["sha256sum.txt must not be a symlink"])
        self.assertFalse(target.exists())

    def test_signer_write_text_rejects_hardlinked_manifest_leaf_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "sha256sum.txt"
            target = root / "external-sha256sum.txt"
            write_text(target, "external\n")
            write_text(output, "placeholder\n")
            replace_with_hardlink(self, output, target)

            errors = evidence_signer._write_text(
                output,
                "replacement\n",
                "sha256sum.txt",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(errors, ["sha256sum.txt must not be hardlinked"])
        self.assertEqual(target_text, "external\n")

    def test_signer_write_text_rejects_secret_manifest_path_directly_without_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "token=supersecret-sha256sum.txt"

            errors = evidence_signer._write_text(
                output,
                "replacement\n",
                "sha256sum.txt",
            )
            rendered = "\n".join(errors)

        self.assertEqual(
            errors,
            ["sha256sum.txt must not contain secret-looking material"],
        )
        self.assertFalse(output.exists())
        self.assertFalse(output.parent.exists())
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(output), rendered)

    def test_signer_write_text_rejects_oversized_manifest_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "sha256sum.txt"
            text = "replacement\n"
            limit = len(text.encode("utf-8")) - 1

            errors = evidence_signer._write_text(
                output,
                text,
                "sha256sum.txt",
                max_bytes=limit,
            )
            temp_files = list(output.parent.glob(".sha256sum.txt.*.tmp"))

        self.assertEqual(errors, [f"sha256sum.txt must be no more than {limit} bytes"])
        self.assertFalse(output.exists())
        self.assertEqual(temp_files, [])

    def test_signer_write_text_rejects_write_failure_after_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "sha256sum.txt"

            with mock.patch.object(
                evidence_signer.os,
                "fsync",
                side_effect=OSError("simulated sha256sum fsync failure"),
            ):
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )

        self.assertEqual(
            errors,
            [
                "sha256sum.txt could not be written",
                "sha256sum.txt temporary file cleanup could not be synced",
            ],
        )
        self.assertFalse(output.exists())

    def test_signer_write_text_preserves_existing_output_on_replace_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "sha256sum.txt"
            write_text(output, "existing manifest\n")

            with mock.patch.object(
                evidence_signer.os,
                "replace",
                side_effect=OSError("simulated sha256sum replace failure"),
            ):
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )
            output_text = output.read_text(encoding="utf-8")
            temp_files = list(output.parent.glob(".sha256sum.txt.*.tmp"))

        self.assertEqual(errors, ["sha256sum.txt could not be written"])
        self.assertEqual(output_text, "existing manifest\n")
        self.assertEqual(temp_files, [])

    def test_signer_write_text_rejects_symlink_swap_before_replace(self) -> None:
        original_validate = evidence_signer._validate_json_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                output = root / "slot" / "sha256sum.txt"
                target = root / "external-sha256sum.txt"
                output.parent.mkdir(parents=True)
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == output and calls == 2:
                        write_text(target, "external\n")
                        try:
                            path.symlink_to(target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                evidence_signer._validate_json_output_path = validate_then_alias
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )
                target_text = target.read_text(encoding="utf-8")
                temp_files = list(output.parent.glob(".sha256sum.txt.*.tmp"))
        finally:
            evidence_signer._validate_json_output_path = original_validate

        self.assertEqual(errors, ["sha256sum.txt must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertEqual(temp_files, [])

    def test_signer_write_text_rejects_readback_mismatch(self) -> None:
        original_read_output_text = evidence_signer._read_existing_output_text

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "sha256sum.txt"

                def mismatching_read_output_text(
                    path: Path,
                    expected_stat: os.stat_result,
                    label: str,
                    *,
                    max_bytes: int | None = None,
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return "mismatched manifest\n", []
                    return original_read_output_text(
                        path,
                        expected_stat,
                        label,
                        max_bytes=max_bytes,
                    )

                evidence_signer._read_existing_output_text = mismatching_read_output_text
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            evidence_signer._read_existing_output_text = original_read_output_text

        self.assertEqual(errors, ["sha256sum.txt write verification failed"])
        self.assertEqual(output_text, "replacement\n")

    def test_signer_write_text_rejects_oversized_readback_after_replace(self) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "sha256sum.txt"
            text = "replacement\n"
            limit = len(text.encode("utf-8")) + 4
            mutated = False

            def appending_open(path: Path, *args, **kwargs):
                nonlocal mutated
                mode = args[0] if args else kwargs.get("mode", "r")
                if path == output and "r" in str(mode) and not mutated:
                    with original_open(path, "a", encoding="utf-8") as handle:
                        handle.write("X" * 16)
                    mutated = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", appending_open):
                errors = evidence_signer._write_text(
                    output,
                    text,
                    "sha256sum.txt",
                    max_bytes=limit,
                )
            output_text = output.read_text(encoding="utf-8")

        self.assertTrue(mutated)
        self.assertGreater(
            len(output_text.encode("utf-8")),
            len(text.encode("utf-8")),
        )
        self.assertEqual(errors, [f"sha256sum.txt must be no more than {limit} bytes"])

    def test_signer_write_text_rejects_readback_failure(self) -> None:
        original_read_output_text = evidence_signer._read_existing_output_text

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "sha256sum.txt"

                def failing_read_output_text(
                    path: Path,
                    _expected_stat: os.stat_result,
                    label: str,
                    *,
                    max_bytes: int | None = None,
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return None, [f"{label} write verification failed"]
                    return original_read_output_text(
                        path,
                        _expected_stat,
                        label,
                        max_bytes=max_bytes,
                    )

                evidence_signer._read_existing_output_text = failing_read_output_text
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )
                output_text = output.read_text(encoding="utf-8")
                temp_files = list(output.parent.glob(".sha256sum.txt.*.tmp"))
        finally:
            evidence_signer._read_existing_output_text = original_read_output_text

        self.assertEqual(errors, ["sha256sum.txt write verification failed"])
        self.assertEqual(output_text, "replacement\n")
        self.assertEqual(temp_files, [])

    def test_signer_write_text_rejects_symlink_swap_after_replace(self) -> None:
        original_validate = evidence_signer._validate_existing_json_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                output = root / "slot" / "sha256sum.txt"
                target = root / "external-sha256sum.txt"
                write_text(target, "external\n")
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == output and calls == 1:
                        path.unlink(missing_ok=True)
                        try:
                            path.symlink_to(target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                evidence_signer._validate_existing_json_output_path = validate_then_alias
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )
                target_text = target.read_text(encoding="utf-8")
                output_is_symlink = output.is_symlink()
                temp_files = list(output.parent.glob(".sha256sum.txt.*.tmp"))
        finally:
            evidence_signer._validate_existing_json_output_path = original_validate

        self.assertEqual(errors, ["sha256sum.txt must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertTrue(output_is_symlink)
        self.assertEqual(temp_files, [])

    def test_rewrite_sha256_manifest_rejects_oversized_manifest_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")

            with mock.patch.object(
                evidence_signer.device_lab,
                "MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES",
                1,
            ):
                errors = evidence_signer.rewrite_sha256_manifest(slot)
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertEqual(errors, ["sha256sum.txt must be no more than 1 bytes"])
        self.assertEqual(manifest_after, original_manifest)

    def test_rewrite_sha256_manifest_rejects_symlinked_artifact_when_called_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            errors = evidence_signer.rewrite_sha256_manifest(slot)
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertIn("slot artifact logs/runtime.log must not be a symlink", errors)
        self.assertEqual(manifest_after, original_manifest)

    def test_rewrite_sha256_manifest_rejects_hardlinked_manifest_when_called_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            external_manifest = root / "external-sha256sum.txt"
            external_manifest.write_text("do not overwrite\n", encoding="utf-8")
            replace_with_hardlink(self, slot / "sha256sum.txt", external_manifest)

            errors = evidence_signer.rewrite_sha256_manifest(slot)
            target_text = external_manifest.read_text(encoding="utf-8")

        self.assertEqual(errors, ["sha256sum.txt must not be hardlinked"])
        self.assertEqual(target_text, "do not overwrite\n")

    def test_rewrite_sha256_manifest_rejects_secret_looking_artifact_when_called_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")
            secret_artifact = "logs/token=supersecret.log"
            write_text(slot / secret_artifact, "must not be persisted\n")

            errors = evidence_signer.rewrite_sha256_manifest(slot)
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot artifacts must not contain secret-looking material"])
        self.assertEqual(manifest_after, original_manifest)
        self.assertNotIn(secret_artifact, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_rewrite_sha256_manifest_rejects_secret_slot_path_directly_without_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "token=supersecret-slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")

            errors = evidence_signer.rewrite_sha256_manifest(slot)
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertEqual(manifest_after, original_manifest)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_rewrite_sha256_manifest_rejects_slot_directory_metadata_failure_without_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp) / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot:
                    raise OSError("simulated slot directory lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors = evidence_signer.rewrite_sha256_manifest(slot)
            finally:
                path_type.lstat = original_lstat
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertEqual(errors, ["slot directory metadata could not be read"])
        self.assertEqual(manifest_after, original_manifest)

    def test_rewrite_sha256_manifest_rejects_slot_parent_metadata_failure_without_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp) / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot.parent:
                    raise OSError("simulated slot parent lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors = evidence_signer.rewrite_sha256_manifest(slot)
            finally:
                path_type.lstat = original_lstat
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertEqual(errors, ["slot parent directory metadata could not be read"])
        self.assertEqual(manifest_after, original_manifest)

    def test_signer_slot_artifact_digest_rejects_secret_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            secret_relative = "logs/token=supersecret.log"
            write_text(slot / secret_relative, "must not be hashed\n")

            digest, errors = evidence_signer._slot_artifact_sha256(
                slot,
                secret_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot artifacts must not contain secret-looking material"])
        self.assertNotIn(secret_relative, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_signer_slot_artifact_digest_rejects_control_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            control_relative = "logs/runtime\x1b[31m.log"

            digest, errors = evidence_signer._slot_artifact_sha256(
                slot,
                control_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["slot artifact path: unsafe path contains control characters"],
        )
        self.assertNotIn(control_relative, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_signer_slot_artifact_digest_rejects_symlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "pixel8")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = evidence_signer._slot_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot artifact logs/runtime.log must not be a symlink"])

    def test_signer_slot_artifact_digest_rejects_hardlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "pixel8")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = evidence_signer._slot_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot artifact logs/runtime.log must not be hardlinked"])

    def test_signer_slot_artifact_digest_rejects_oversized_artifact_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            runtime_log = slot / "logs" / "runtime.log"
            with runtime_log.open("wb") as handle:
                handle.seek(device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES)
                handle.write(b"x")

            digest, errors = evidence_signer._slot_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "slot artifact logs/runtime.log must be no more than "
                f"{device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES} bytes"
            ],
        )

    def test_signer_slot_artifact_digest_uses_release_apk_specific_limit(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            apk_path = slot / "evidence" / "kagemusha-wallet-release.apk"
            apk_path.write_bytes(b"x" * 16)
            old_base_limit = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
            old_apk_limit = device_lab.MAX_KAGEMUSHA_WALLET_APK_BYTES
            try:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 8
                device_lab.MAX_KAGEMUSHA_WALLET_APK_BYTES = 32
                digest, errors = evidence_signer._slot_artifact_sha256(
                    slot,
                    "evidence/kagemusha-wallet-release.apk",
                )
            finally:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = old_base_limit
                device_lab.MAX_KAGEMUSHA_WALLET_APK_BYTES = old_apk_limit

        self.assertEqual(errors, [])
        self.assertEqual(digest, hashlib.sha256(b"x" * 16).hexdigest())

    def test_signer_slot_artifact_digest_rejects_hardlink_metadata_failure_after_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            target = slot / "logs" / "runtime.log"

            def failing_stat(path: Path, *args, **kwargs):
                if path == target and kwargs.get("follow_symlinks", True):
                    raise OSError("simulated slot artifact stat failure")
                return original_stat(path, *args, **kwargs)

            try:
                path_type.stat = failing_stat
                digest, errors = evidence_signer._slot_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
            finally:
                path_type.stat = original_stat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["slot artifact logs/runtime.log hardlink metadata could not be read"],
        )

    def test_signer_slot_artifact_digest_rejects_file_metadata_failure_after_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            target = slot / "logs" / "runtime.log"

            def failing_lstat(path: Path, *args, **kwargs):
                if path == target:
                    raise OSError("simulated slot artifact lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                digest, errors = evidence_signer._slot_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
            finally:
                path_type.lstat = original_lstat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["slot artifact logs/runtime.log file metadata could not be read"],
        )

    def test_signer_slot_artifact_digest_rejects_read_failure_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            target = slot / "logs" / "runtime.log"

            digest, errors = with_open_failure(
                target,
                lambda: evidence_signer._slot_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                ),
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot artifact logs/runtime.log could not be read"])

    def test_signer_slot_artifact_digest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        original_validate = evidence_signer._validate_slot_artifact_for_digest

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "pixel8")
                artifact_path = slot / "logs" / "runtime.log"
                swapped = False

                def swapping_validate(slot_path: Path, relative: str):
                    nonlocal swapped
                    artifact, artifact_stat, errors = original_validate(
                        slot_path,
                        relative,
                    )
                    if artifact == artifact_path and not errors and not swapped:
                        artifact_path.unlink()
                        write_text(artifact_path, "replacement runtime log\n")
                        swapped = True
                    return artifact, artifact_stat, errors

                evidence_signer._validate_slot_artifact_for_digest = swapping_validate

                digest, errors = evidence_signer._slot_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
                replacement_bytes = artifact_path.read_bytes()
        finally:
            evidence_signer._validate_slot_artifact_for_digest = original_validate

        self.assertTrue(swapped)
        self.assertIsNone(digest)
        self.assertEqual(replacement_bytes, b"replacement runtime log\n")
        self.assertEqual(
            errors,
            ["slot artifact logs/runtime.log changed while being read"],
        )

    def test_rewrite_sha256_manifest_revalidates_artifact_before_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            original_validate = evidence_signer._validate_slot_for_manifest_rewrite

            def validate_then_alias(slot_path: Path) -> list[str]:
                errors = original_validate(slot_path)
                if not errors:
                    replace_with_symlink(self, slot_path / "logs" / "runtime.log", target)
                return errors

            try:
                evidence_signer._validate_slot_for_manifest_rewrite = validate_then_alias
                errors = evidence_signer.rewrite_sha256_manifest(slot)
            finally:
                evidence_signer._validate_slot_for_manifest_rewrite = original_validate
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertEqual(errors, ["slot artifact logs/runtime.log must not be a symlink"])
        self.assertEqual(manifest_after, original_manifest)

    def test_signer_metadata_loader_rejects_secret_slot_path_directly_without_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            slot.mkdir(parents=True)
            (slot / "slot.json").write_text("{not-json", encoding="utf-8")

            metadata, errors = evidence_signer._require_slot_metadata(slot)
            rendered = "\n".join(errors)

        self.assertIsNone(metadata)
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_signer_artifact_digests_include_release_apk_attestation_chain_and_report(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            errors: list[str] = []

            digests = evidence_signer._artifact_digests(  # type: ignore[attr-defined]
                slot,
                errors,
                metadata,
            )

        self.assertEqual(errors, [])
        self.assertIsNotNone(digests)
        assert digests is not None
        self.assertIn(metadata["kagemusha_wallet_apk_path"], digests)
        self.assertIn(metadata["attestation_certificate_chain_path"], digests)
        self.assertIn("attestation/report.json", digests)

    def test_signer_artifact_digests_rejects_secret_slot_path_directly_before_hash(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "token=supersecret-slot")
            errors: list[str] = []

            digests = evidence_signer._artifact_digests(slot, errors)  # type: ignore[attr-defined]
            rendered = "\n".join(errors)

        self.assertIsNone(digests)
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_signer_artifact_digests_rejects_symlinked_slot_ancestor_before_hash(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-parent"
            create_slot(real_parent / "device_lab", "slot-a")
            linked_parent = root / "linked-parent"
            create_dir_symlink(self, linked_parent, real_parent)
            errors: list[str] = []

            digests = evidence_signer._artifact_digests(  # type: ignore[attr-defined]
                linked_parent / "device_lab" / "slot-a",
                errors,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digests)
        self.assertEqual(errors, ["slot ancestor directory must not be a symlink"])
        self.assertNotIn("missing", rendered)
        self.assertNotIn("digest", rendered)

    def test_signer_artifact_digests_rejects_symlinked_artifact_directory_before_hash(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            external_logs = root / "external-logs"
            write_text(external_logs / "runtime.log", "external log\n")
            for entry in (slot / "logs").iterdir():
                entry.unlink()
            (slot / "logs").rmdir()
            create_dir_symlink(self, slot / "logs", external_logs)
            errors: list[str] = []

            digests = evidence_signer._artifact_digests(slot, errors)  # type: ignore[attr-defined]

        self.assertIsNone(digests)
        self.assertEqual(errors, ["logs/ must not be a symlink"])

    def test_signer_helper_rejects_symlinked_slot_json_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            external_slot_json = root / "external-slot.json"
            original_metadata = (slot / "slot.json").read_text(encoding="utf-8")
            write_text(external_slot_json, original_metadata)
            replace_with_symlink(self, slot / "slot.json", external_slot_json)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            target_text = external_slot_json.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertIn("slot.json must not be a symlink", stderr.getvalue())
        self.assertEqual(target_text, original_metadata)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_hardlinked_slot_json_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            external_slot_json = root / "external-slot.json"
            original_metadata = (slot / "slot.json").read_text(encoding="utf-8")
            write_text(external_slot_json, original_metadata)
            replace_with_hardlink(self, slot / "slot.json", external_slot_json)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            target_text = external_slot_json.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertIn("slot.json must not be hardlinked", stderr.getvalue())
        self.assertEqual(target_text, original_metadata)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_metadata_loader_preflights_symlinked_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            metadata, errors = evidence_signer._require_slot_metadata(slot)

        self.assertIsNone(metadata)
        self.assertIn("slot artifact logs/runtime.log must not be a symlink", errors)

    def test_signer_metadata_loader_preflights_hardlinked_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)

            metadata, errors = evidence_signer._require_slot_metadata(slot)

        self.assertIsNone(metadata)
        self.assertIn("slot artifact logs/runtime.log must not be hardlinked", errors)

    def test_signer_helper_rejects_symlinked_required_artifact_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot artifact logs/runtime.log must not be a symlink",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_symlinked_slot_directory_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            slot_link = root / "slot-link"
            create_dir_symlink(self, slot_link, slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot_link),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("slot directory must not be a symlink", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_symlinked_slot_parent_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            real_root = root / "slots"
            slot = create_slot(real_root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            linked_root = root / "linked-slots"
            create_dir_symlink(self, linked_root, real_root)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(linked_root / "pixel8"),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("slot parent directory must not be a symlink", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_symlinked_slot_ancestor_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            external_parent = root / "external-parent"
            real_root = external_parent / "device_lab"
            slot = create_slot(real_root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            linked_parent = root / "linked-parent"
            create_dir_symlink(self, linked_parent, external_parent)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(linked_parent / "device_lab" / "pixel8"),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("slot ancestor directory must not be a symlink", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_hardlinked_required_artifact_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot artifact logs/runtime.log must not be hardlinked",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_non_regular_required_artifact_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            replace_with_fifo(self, slot / "logs" / "runtime.log")

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot artifact logs/runtime.log must be a regular file",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_noncanonical_signed_at_utc(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00+00:00",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "signed evidence artifact signed_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_unexpected_slot_metadata_field(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["operator_notes"] = "unexpected metadata"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json contains unexpected field operator_notes",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_duplicate_slot_json_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            inject_duplicate_json_key(slot / "slot.json", "schema", "shadow")
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("slot.json contains duplicate JSON object key schema", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_duplicate_attestation_json_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            inject_duplicate_json_key(slot / "attestation" / "result.json", "slot", "shadow")
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/result.json contains duplicate JSON object key slot",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_duplicate_d2d_transcript_json_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            inject_duplicate_json_key(slot / "handoff" / "d2d-payment.json", "schema", "shadow")
            refresh_d2d_payment_transcript_hash(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "d2d payment transcript contains duplicate JSON object key schema",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_duplicate_wallet_integrity_json_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            inject_duplicate_json_key(slot / "wallet" / "integrity.json", "schema", "shadow")
            refresh_wallet_integrity_transcript_hash(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "wallet integrity transcript contains duplicate JSON object key schema",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_padded_slot_string_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["device_family"] = (
                f" {device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1]} "
            )
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json device_family must not contain surrounding whitespace",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_control_slot_string_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["app_package_name"] = "org.hyperledger.iroha\x1b[31m"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json app_package_name must not contain control characters",
            rendered,
        )
        self.assertNotIn("\x1b", rendered)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_padded_attestation_chain_path_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["attestation_certificate_chain_path"] = (
                " attestation/keymint-certificate-chain.pem "
            )
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json attestation_certificate_chain_path must not contain surrounding whitespace",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_harness_chain_helper_rejects_root_only_attestation_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel7")
            errors: list[str] = []

            chain_bytes = evidence_signer._attestation_certificate_chain_bytes_for_harness(  # type: ignore[attr-defined]
                slot,
                {"attestation_certificate_chain_path": "attestation"},
                errors,
            )

        self.assertIsNone(chain_bytes)
        self.assertEqual(
            errors,
            [
                "slot.json attestation_certificate_chain_path must stay under attestation/"
            ],
        )

    def test_signer_helper_rejects_irrelevant_raw_test_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = ["./gradlew test --tests unrelated.HealthCheck"]
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        for marker in device_lab.RAW_TEST_COMMAND_REQUIRED_MARKERS:
            self.assertIn(
                f"slot.json raw_test_commands must include {marker}",
                stderr.getvalue(),
            )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_padded_raw_test_command_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            commands = list(device_lab.KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS)
            commands[0] = f" {commands[0]} "
            metadata["raw_test_commands"] = commands
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json raw_test_commands[0] must not contain surrounding whitespace",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_marker_stuffed_raw_test_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = [
                "echo " + " ".join(device_lab.RAW_TEST_COMMAND_REQUIRED_MARKERS)
            ]
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json raw_test_commands must exactly match the Kagemusha Android production raw test command",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_missing_native_bridge_abi_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("native_bridge_abi_version")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json native_bridge_abi_version must be an integer",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_attestation_result_mismatch_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["kagemusha_wallet_policy_sha256"] = "33" * 32
            write_json(attestation_path, attestation)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/result.json kagemusha_wallet_policy_sha256 must match slot.json kagemusha_wallet_policy_sha256",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_attestation_report_mismatch_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            report_path = slot / "attestation" / "report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["verification"]["strongbox_attestation"] = False
            write_json(report_path, report)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/report.json verification.strongbox_attestation must be true",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_missing_attestation_report_level_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            report_path = slot / "attestation" / "report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            del report["verification"]["keymaster_security_level"]
            write_json(report_path, report)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/report.json verification.keymaster_security_level "
            "must be a non-empty string",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_harness_challenge_mismatch_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            harness_path = slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["challenge_hex"] = "00"
            write_json(harness_path, harness)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/harness-result.json challenge_hex digest must match "
            "slot.json attestation_challenge_sha256",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_harness_chain_length_mismatch_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            harness_path = slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["chain_length"] = 3
            write_json(harness_path, harness)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/harness-result.json chain_length must match "
            "attestation certificate-chain certificate count",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_d2d_transcript_mismatch_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["queue_after_sha256"] = "22" * 32
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "d2d payment transcript queue_after_sha256 must match queue/pending_queue.json",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_wallet_integrity_transcript_mismatch_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            transcript_path = slot / "wallet" / "integrity.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["wallet_state_after_rotation_sha256"] = transcript[
                "wallet_state_before_sha256"
            ]
            write_json(transcript_path, transcript)
            refresh_wallet_integrity_transcript_hash(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "wallet integrity transcript wallet_state_before_sha256 must differ from wallet_state_after_rotation_sha256",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_secret_looking_artifact_paths_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            secret_path = "logs/token=supersecret.log"
            write_text(slot / secret_path, "must not be persisted\n")

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

            rendered = stdout.getvalue() + stderr.getvalue()
            manifest_text = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertIn("slot artifacts must not contain secret-looking material", rendered)
        self.assertNotIn(secret_path, rendered)
        self.assertNotIn(secret_path, manifest_text)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_missing_required_slot_artifact_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            (slot / "logs" / "runtime.log").unlink()
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("slot artifact logs/runtime.log is missing", stderr.getvalue())
        self.assertNotIn("Traceback", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_empty_required_slot_artifact_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            write_text(slot / "logs" / "runtime.log", "")
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "required slot artifact logs/runtime.log must be non-empty",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_failed_status_ndjson_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            write_text(slot / "telemetry" / "status.ndjson", '{"status":"failed"}\n')
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "telemetry/status.ndjson line 1 status must not be 'failed'",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_does_not_leak_secret_looking_private_key_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            secret_private_key = Path(temp) / "private_key=supersecret.pem"
            secret_private_key.write_text("not an ed25519 private key\n", encoding="utf-8")

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(secret_private_key),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertNotIn(str(secret_private_key), rendered)
        self.assertNotIn("private_key=supersecret", rendered)
        self.assertIn(
            "private key path must not contain secret-looking material",
            rendered,
        )

    def test_sign_ed25519_rejects_secret_private_key_path_before_openssl_lookup(
        self,
    ) -> None:
        with mock.patch.object(
            device_lab,
            "_require_openssl",
            side_effect=AssertionError("OpenSSL lookup must not run"),
        ):
            with tempfile.TemporaryDirectory() as temp:
                secret_private_key = Path(temp) / "private_key=supersecret.pem"
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    secret_private_key,
                    b"payload",
                    errors,
                )
                rendered = "\n".join(errors)

        self.assertIsNone(signature)
        self.assertEqual(
            errors,
            ["private key path must not contain secret-looking material"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(secret_private_key), rendered)
        self.assertNotIn("private_key=supersecret", rendered)

    def test_sign_ed25519_rejects_control_private_key_path_before_openssl_lookup(
        self,
    ) -> None:
        with mock.patch.object(
            device_lab,
            "_require_openssl",
            side_effect=AssertionError("OpenSSL lookup must not run"),
        ):
            with tempfile.TemporaryDirectory() as temp:
                control_private_key = Path(temp) / "control\nprivate.pem"
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    control_private_key,
                    b"payload",
                    errors,
                )
                rendered = "\n".join(errors)

        self.assertIsNone(signature)
        self.assertEqual(
            errors,
            ["private key path must not contain control characters"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(control_private_key), rendered)

    def test_sign_ed25519_rejects_whitespace_private_key_path_before_openssl_lookup(
        self,
    ) -> None:
        path_type = type(Path("."))
        with tempfile.TemporaryDirectory() as temp:
            whitespace_private_key = Path(temp) / " private.pem"
            with (
                mock.patch.object(
                    path_type,
                    "lstat",
                    side_effect=AssertionError("private key metadata must not be read"),
                ) as lstat,
                mock.patch.object(
                    device_lab,
                    "_require_openssl",
                    side_effect=AssertionError("OpenSSL lookup must not run"),
                ) as require_openssl,
            ):
                errors: list[str] = []
                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    whitespace_private_key,
                    b"payload",
                    errors,
                )

        self.assertIsNone(signature)
        self.assertEqual(
            errors,
            ["private key path must not contain surrounding whitespace"],
        )
        self.assertNotIn(str(whitespace_private_key), "\n".join(errors))
        lstat.assert_not_called()
        require_openssl.assert_not_called()

    def test_sign_ed25519_rejects_private_key_aliases_before_metadata_or_openssl(
        self,
    ) -> None:
        path_type = type(Path("."))
        cases = (
            (
                Path(" private.pem"),
                "private key path must not contain surrounding whitespace",
            ),
            (
                Path("keys") / ".." / "private.pem",
                "private key path must be canonical",
            ),
            (
                Path("keys\\private.pem"),
                "private key path must not contain backslashes",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for private_key, expected_error in cases:
                private_key = root / private_key
                with self.subTest(private_key=private_key):
                    with (
                        mock.patch.object(
                            path_type,
                            "lstat",
                            side_effect=AssertionError(
                                "private key metadata must not be read"
                            ),
                        ) as lstat,
                        mock.patch.object(
                            device_lab,
                            "_require_openssl",
                            side_effect=AssertionError("OpenSSL lookup must not run"),
                        ) as require_openssl,
                    ):
                        errors: list[str] = []
                        signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                            private_key,
                            b"payload",
                            errors,
                        )
                    rendered = "\n".join(errors)

                    self.assertIsNone(signature)
                    self.assertEqual(errors, [expected_error])
                    self.assertNotIn("openssl is required", rendered)
                    self.assertNotIn(str(private_key), rendered)
                    lstat.assert_not_called()
                    require_openssl.assert_not_called()

    def test_sign_ed25519_rejects_missing_private_key_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for a missing private key")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                missing_private_key = Path(temp) / "missing-signer.pem"
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    missing_private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(signature)
        self.assertEqual(errors, ["private key must point to an existing file"])

    def test_sign_ed25519_rejects_non_regular_private_key_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for a private key directory")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                private_key_directory = Path(temp) / "signer-directory.pem"
                private_key_directory.mkdir()
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key_directory,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(signature)
        self.assertEqual(errors, ["private key must be a regular file"])

    def test_sign_ed25519_rejects_oversized_private_key_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for an oversized private key")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_bytes(
                    b"x" * (device_lab.MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES + 1)
                )
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(signature)
        self.assertEqual(
            errors,
            [
                "private key must be no more than "
                f"{device_lab.MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES} bytes"
            ],
        )

    def test_sign_ed25519_rejects_private_key_file_metadata_failure_before_openssl(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for unreadable key metadata")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == private_key:
                        raise OSError("simulated private key lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            path_type.lstat = original_lstat
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(signature)
        self.assertEqual(errors, ["private key file metadata could not be read"])
        self.assertNotIn(str(private_key), "\n".join(errors))

    def test_sign_ed25519_rejects_private_key_hardlink_metadata_failure_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        path_type = type(Path("."))
        original_stat = path_type.stat

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up after metadata failure")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                private_key_stat_calls = 0

                def failing_private_key_stat(path: Path, *args, **kwargs):
                    nonlocal private_key_stat_calls
                    if path == private_key and kwargs.get("follow_symlinks", True):
                        private_key_stat_calls += 1
                        if private_key_stat_calls > 0:
                            raise OSError("simulated private key stat failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_private_key_stat
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            path_type.stat = original_stat

        self.assertIsNone(signature)
        self.assertEqual(errors, ["private key hardlink metadata could not be read"])
        self.assertNotIn(str(private_key), "\n".join(errors))

    def test_sign_ed25519_rejects_signature_read_failure_after_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_open = Path.open

        def fake_run(command, *args, **kwargs):
            out_path = Path(command[command.index("-out") + 1])
            out_path.write_bytes(b"x" * device_lab.ED25519_SIGNATURE_BYTES)
            return subprocess.CompletedProcess(args=args, returncode=0)

        def failing_signature_open(path: Path, *args, **kwargs):
            mode = args[0] if args else kwargs.get("mode", "r")
            if path.name == "signature.bin" and "r" in mode:
                raise OSError("simulated signature read failure")
            return original_open(path, *args, **kwargs)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            Path.open = failing_signature_open
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            Path.open = original_open

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature output could not be read"])

    def test_sign_ed25519_rejects_signature_output_swap_after_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_open = Path.open
        replacement_signature = b"y" * device_lab.ED25519_SIGNATURE_BYTES

        def fake_run(command, *args, **kwargs):
            out_path = Path(command[command.index("-out") + 1])
            out_path.write_bytes(b"x" * device_lab.ED25519_SIGNATURE_BYTES)
            replacement_path = out_path.with_name("replacement-signature.bin")
            replacement_path.write_bytes(replacement_signature)
            return subprocess.CompletedProcess(args=command, returncode=0)

        swapped = False

        def swapping_signature_open(path: Path, *args, **kwargs):
            nonlocal swapped
            mode = args[0] if args else kwargs.get("mode", "r")
            if path.name == "signature.bin" and "r" in mode and not swapped:
                path.with_name("replacement-signature.bin").replace(path)
                swapped = True
            return original_open(path, *args, **kwargs)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            Path.open = swapping_signature_open
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            Path.open = original_open

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature output could not be read"])

    def test_sign_ed25519_rejects_signature_output_hardlink_after_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_open = Path.open

        def fake_run(command, *args, **kwargs):
            out_path = Path(command[command.index("-out") + 1])
            out_path.write_bytes(b"x" * device_lab.ED25519_SIGNATURE_BYTES)
            return subprocess.CompletedProcess(args=command, returncode=0)

        linked = False

        def hardlinking_signature_open(path: Path, *args, **kwargs):
            nonlocal linked
            mode = args[0] if args else kwargs.get("mode", "r")
            if path.name == "signature.bin" and "r" in mode and not linked:
                try:
                    os.link(path, path.with_name("signature-output-hardlink.bin"))
                except (AttributeError, NotImplementedError, OSError) as exc:
                    self.skipTest(
                        f"hardlinks are not available in this test environment: {exc}"
                    )
                linked = True
            return original_open(path, *args, **kwargs)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            Path.open = hardlinking_signature_open
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            Path.open = original_open

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature output could not be read"])

    def test_sign_ed25519_reads_only_shape_bound_signature_output_after_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_open = Path.open
        read_sizes: list[int] = []
        read_limit = device_lab.ED25519_SIGNATURE_BYTES + 1

        class SignatureOutputReader:
            def __init__(self, handle):
                self._handle = handle

            def __enter__(self):
                self._handle.__enter__()
                return self

            def __exit__(self, exc_type, exc, traceback):
                return self._handle.__exit__(exc_type, exc, traceback)

            def fileno(self):
                return self._handle.fileno()

            def read(self, size=-1):
                read_sizes.append(size)
                if size > read_limit:
                    raise AssertionError("signature output read exceeded shape bound")
                return self._handle.read(size)

        def fake_run(command, *args, **kwargs):
            out_path = Path(command[command.index("-out") + 1])
            out_path.write_bytes(b"x" * read_limit)
            return subprocess.CompletedProcess(args=command, returncode=0)

        def bounded_signature_open(path: Path, *args, **kwargs):
            mode = args[0] if args else kwargs.get("mode", "r")
            handle = original_open(path, *args, **kwargs)
            if path.name == "signature.bin" and "r" in mode:
                return SignatureOutputReader(handle)
            return handle

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            Path.open = bounded_signature_open
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            Path.open = original_open

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature output must be 64 bytes"])
        self.assertEqual(read_sizes, [read_limit])

    def test_sign_ed25519_rejects_short_signature_output_after_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run

        def fake_run(command, *args, **kwargs):
            out_path = Path(command[command.index("-out") + 1])
            out_path.write_bytes(b"short signature")
            return subprocess.CompletedProcess(args=command, returncode=0)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature output must be 64 bytes"])

    def test_sign_ed25519_rejects_tempdir_failure_before_payload_staging(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_tempdir = evidence_signer.tempfile.TemporaryDirectory

        def failing_tempdir(*args, **kwargs):
            raise OSError("simulated temporary directory failure")

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after tempdir failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = unexpected_run
            evidence_signer.tempfile.TemporaryDirectory = failing_tempdir
            with original_tempdir() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            evidence_signer.tempfile.TemporaryDirectory = original_tempdir

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature temporary directory could not be created"])

    def test_sign_ed25519_rejects_spawn_failure_after_payload_staging(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run

        def failing_run(*args, **kwargs):
            raise OSError("simulated OpenSSL spawn failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = failing_run
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature command could not be run"])

    def test_sign_ed25519_scrubs_operator_openssl_env(self) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        captured_env: dict[str, str] = {}

        def fake_run(command, **kwargs):
            captured_env.update(kwargs["env"])
            signature_path = Path(command[command.index("-out") + 1])
            signature_path.write_bytes(b"s" * device_lab.ED25519_SIGNATURE_BYTES)
            return subprocess.CompletedProcess(command, 0, stdout=b"", stderr=b"")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []
                with mock.patch.dict(
                    os.environ,
                    {
                        "PATH": "/usr/bin",
                        **{
                            key: f"/tmp/unsafe-{key.lower()}"
                            for key in device_lab.FORBIDDEN_OPENSSL_CHILD_ENV_KEYS  # type: ignore[attr-defined]
                        },
                    },
                    clear=True,
                ):
                    signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                        private_key,
                        b"payload",
                        errors,
                    )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run

        self.assertEqual(errors, [])
        self.assertEqual(signature, b"s" * device_lab.ED25519_SIGNATURE_BYTES)
        self.assertEqual(captured_env["PATH"], "/usr/bin")
        for key in device_lab.FORBIDDEN_OPENSSL_CHILD_ENV_KEYS:  # type: ignore[attr-defined]
            self.assertNotIn(key, captured_env)

    def test_sign_ed25519_rejects_invalid_private_key_after_openssl_failure(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run

        def failing_run(*args, **kwargs):
            raise subprocess.CalledProcessError(1, args[0])

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = failing_run
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("invalid key data\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run

        self.assertIsNone(signature)
        self.assertEqual(
            errors,
            ["private key must be a valid OpenSSL Ed25519 private key"],
        )

    def test_sign_ed25519_rejects_payload_staging_write_failure_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging write failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = unexpected_run
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                with mock.patch.object(
                    device_lab.os,
                    "fsync",
                    side_effect=OSError("simulated payload staging fsync failure"),
                ):
                    signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                        private_key,
                        b"payload",
                        errors,
                    )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature payload could not be staged"])

    def test_sign_ed25519_rejects_payload_staging_readback_mismatch_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_read_staged_bytes = evidence_signer.device_lab._read_staged_bytes  # type: ignore[attr-defined]

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging readback drift")

        def drifting_payload_read(
            path: Path,
            expected_stat: os.stat_result,
            verification_error: str,
        ) -> tuple[bytes | None, list[str]]:
            if path.name == "payload.bin":
                return b"mutated payload", []
            return original_read_staged_bytes(path, expected_stat, verification_error)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = unexpected_run
            evidence_signer.device_lab._read_staged_bytes = drifting_payload_read  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            evidence_signer.device_lab._read_staged_bytes = original_read_staged_bytes  # type: ignore[attr-defined]

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature payload staging verification failed"])

    def test_standard_matrix_accepts_all_kagemusha_device_families(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            transports = sorted(device_lab.D2D_PAYMENT_TRANSPORTS)
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
                create_slot(
                    root,
                    f"slot-{index}",
                    family,
                    signer,
                    d2d_payment_transport=transports[index % len(transports)],
                    d2d_payment_transports=tuple(transports),
                )
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-standard-matrix",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                    ]
                )

        self.assertEqual(status, 0)

    def test_standard_matrix_rejects_missing_d2d_payment_transport_matrix(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
                create_slot(root, f"slot-{index}", family, signer)

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-standard-matrix",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        expected_missing_pairs = [
            {"device_family": family, "transport": transport}
            for family in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
            for transport in sorted(device_lab.D2D_PAYMENT_TRANSPORTS)
            if transport != "nfc_hce"
        ]
        self.assertEqual(status, 1)
        self.assertIn(
            "missing Kagemusha production evidence for standard-family D2D payment transports:",
            rendered,
        )
        self.assertIn("Google Pixel 6 / 6a=nearby_offline", rendered)
        self.assertEqual(
            summary["kagemusha"]["required_d2d_payment_transports"],
            sorted(device_lab.D2D_PAYMENT_TRANSPORTS),
        )
        self.assertEqual(summary["kagemusha"]["covered_d2d_payment_transports"], ["nfc_hce"])
        self.assertEqual(
            summary["kagemusha"]["missing_d2d_payment_transports"],
            ["nearby_offline", "qr"],
        )
        self.assertEqual(
            summary["kagemusha"]["missing_d2d_payment_transport_pairs"],
            expected_missing_pairs,
        )

    def test_standard_matrix_rejects_aggregate_d2d_transport_without_family_pairs(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
                create_slot(
                    root,
                    f"slot-{index}",
                    family,
                    signer,
                    d2d_payment_transport=transports[index % len(transports)],
                )

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-standard-matrix",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        first_family = device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]
        expected_missing_pairs = [
            {"device_family": family, "transport": transport}
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES)
            for transport in transports
            if transport != transports[index % len(transports)]
        ]
        self.assertEqual(status, 1)
        self.assertIn(
            "missing Kagemusha production evidence for standard-family D2D payment transports:",
            rendered,
        )
        self.assertIn(f"{first_family}={transports[1]}", rendered)
        self.assertEqual(
            summary["kagemusha"]["covered_device_families"],
            sorted(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
        )
        self.assertEqual(
            summary["kagemusha"]["covered_d2d_payment_transports"],
            list(transports),
        )
        self.assertEqual(summary["kagemusha"]["missing_d2d_payment_transports"], [])
        self.assertEqual(
            summary["kagemusha"]["missing_d2d_payment_transport_pairs"],
            expected_missing_pairs,
        )

    def test_standard_matrix_rejects_duplicate_device_fingerprint(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
                create_slot(root, f"slot-{index}", family, signer)
            copy_slot_binding(
                source=root / "slot-0",
                target=root / "slot-1",
                signer=signer,
                key="device_fingerprint",
            )

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-standard-matrix",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary_text = summary_path.read_text(encoding="utf-8")
            summary = json.loads(summary_text)

        self.assertEqual(status, 1)
        self.assertIn(
            "duplicate Kagemusha device_fingerprint_sha256 across slots: slot-0, slot-1",
            rendered,
        )
        duplicate = summary["kagemusha"]["duplicate_bindings"][
            "device_fingerprint_sha256"
        ][0]
        self.assertEqual(duplicate["slots"], ["slot-0", "slot-1"])
        self.assertEqual(
            duplicate["value_sha256"],
            hashlib.sha256(b"slot-0/fingerprint").hexdigest(),
        )
        self.assertNotIn("slot-0/fingerprint", rendered)
        self.assertNotIn("slot-0/fingerprint", summary_text)

    def test_build_summary_reports_duplicate_attestation_challenge(self) -> None:
        duplicate_challenge = hashlib.sha256(
            b"duplicate-kagemusha-attestation-challenge"
        ).hexdigest()
        reports = [
            summary_release_report(
                "slot-0",
                device_fingerprint_sha256="a" * 64,
                attestation_challenge_sha256=duplicate_challenge,
                d2d_payment_transcript_sha256="1" * 64,
            ),
            summary_release_report(
                "slot-1",
                device_fingerprint_sha256="b" * 64,
                attestation_challenge_sha256=duplicate_challenge,
                d2d_payment_transcript_sha256="2" * 64,
            ),
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        duplicate = summary["kagemusha"]["duplicate_bindings"][
            "attestation_challenge_sha256"
        ][0]
        self.assertEqual(duplicate["slots"], ["slot-0", "slot-1"])
        self.assertEqual(duplicate["value_sha256"], duplicate_challenge)

    def test_build_summary_reports_duplicate_d2d_transcript_digest(self) -> None:
        duplicate_digest = "7" * 64
        reports = [
            summary_release_report(
                "slot-0",
                d2d_payment_transport="nfc_hce",
                d2d_payment_transcript_sha256=duplicate_digest,
            ),
            summary_release_report(
                "slot-1",
                device_fingerprint_sha256="c" * 64,
                attestation_challenge_sha256="d" * 64,
                d2d_payment_transport="qr",
                d2d_payment_transcript_sha256=duplicate_digest,
            ),
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        duplicate = summary["kagemusha"]["duplicate_bindings"][
            "d2d_payment_transcript_sha256"
        ][0]
        self.assertEqual(duplicate["slots"], ["slot-0", "slot-1"])
        self.assertEqual(duplicate["value_sha256"], duplicate_digest)

    def test_build_summary_reports_duplicate_d2d_transcript_map_digest(self) -> None:
        transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
        primary_transport = transports[0]
        copied_transport = transports[-1]
        duplicate_digest = "9" * 64
        bindings = summary_d2d_transcript_bindings(
            transports,
            primary_transport=primary_transport,
        )
        bindings[copied_transport]["sha256"] = duplicate_digest
        reports = [
            summary_release_report(
                "slot-0",
                d2d_payment_transport=primary_transport,
                d2d_payment_transports=list(transports),
                d2d_payment_transcripts=bindings,
            ),
            summary_release_report(
                "slot-1",
                device_fingerprint_sha256="c" * 64,
                attestation_challenge_sha256="d" * 64,
                d2d_payment_transport=copied_transport,
                d2d_payment_transcript_sha256=duplicate_digest,
            ),
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        duplicate = summary["kagemusha"]["duplicate_bindings"][
            "d2d_payment_transcript_sha256"
        ][0]
        self.assertEqual(duplicate["slots"], ["slot-0", "slot-1"])
        self.assertEqual(duplicate["value_sha256"], duplicate_digest)

    def test_duplicate_matrix_bindings_can_require_complete_signed_evidence(
        self,
    ) -> None:
        duplicate_fingerprint = "a" * 64
        reports = [
            summary_release_report(
                "slot-0",
                device_fingerprint_sha256=duplicate_fingerprint,
            ),
            summary_release_report(
                "slot-1",
                device_fingerprint_sha256=duplicate_fingerprint,
            ),
        ]
        partial = summary_release_report(
            "slot-2",
            device_fingerprint_sha256=duplicate_fingerprint,
        )
        del partial["kagemusha"]["signed_evidence_signer_public_key_sha256"]
        reports.append(partial)

        duplicates = device_lab.kagemusha_duplicate_matrix_bindings(
            reports,
            require_complete_signed_evidence=True,
        )

        duplicate = duplicates["device_fingerprint_sha256"][0]
        self.assertEqual(duplicate["slots"], ["slot-0", "slot-1"])
        self.assertEqual(duplicate["value_sha256"], duplicate_fingerprint)

    def test_duplicate_matrix_bindings_redacts_unsafe_direct_report_slots(self) -> None:
        duplicate_fingerprint = "a" * 64
        reports = [
            {
                "slot": "token=supersecret-slot",
                "status": "ok",
                "kagemusha": {
                    "device_fingerprint_sha256": duplicate_fingerprint,
                },
            },
            {
                "slot": "slot-\x1b[31m",
                "status": "ok",
                "kagemusha": {
                    "device_fingerprint_sha256": duplicate_fingerprint,
                },
            },
        ]

        duplicates = device_lab.kagemusha_duplicate_matrix_bindings(reports)
        rendered = json.dumps(duplicates, allow_nan=False)

        duplicate = duplicates["device_fingerprint_sha256"][0]
        self.assertEqual(
            duplicate["slots"],
            [device_lab.SECRET_PATH_REDACTION, "<unsafe-slot-name>"],
        )
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("token=supersecret-slot", rendered)
        self.assertNotIn("slot-\\u001b[31m", rendered)
        self.assertNotIn("\\u001b", rendered)

    def test_duplicate_matrix_bindings_ignores_non_sha256_direct_values(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "ok",
                "kagemusha": {
                    "device_fingerprint_sha256": "token=supersecret-fingerprint",
                },
            },
            {
                "slot": "slot-1",
                "status": "ok",
                "kagemusha": {
                    "device_fingerprint_sha256": "token=supersecret-fingerprint",
                },
            },
        ]

        duplicates = device_lab.kagemusha_duplicate_matrix_bindings(reports)
        rendered = json.dumps(duplicates, allow_nan=False)

        self.assertEqual(duplicates, {})
        self.assertNotIn("supersecret", rendered)

    def test_duplicate_matrix_bindings_ignores_zero_direct_values(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "ok",
                "kagemusha": {
                    "device_fingerprint_sha256": "0" * 64,
                    "attestation_challenge_sha256": "0" * 64,
                },
            },
            {
                "slot": "slot-1",
                "status": "ok",
                "kagemusha": {
                    "device_fingerprint_sha256": "0" * 64,
                    "attestation_challenge_sha256": "0" * 64,
                },
            },
        ]

        duplicates = device_lab.kagemusha_duplicate_matrix_bindings(reports)

        self.assertEqual(duplicates, {})

    def test_build_summary_redacts_unsafe_direct_report_strings(self) -> None:
        duplicate_fingerprint = "b" * 64
        duplicate_challenge = "c" * 64
        family = device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]
        reports = [
            {
                "slot": "token=supersecret-slot",
                "status": "ok",
                "errors": ["bearer supersecret-token"],
                "kagemusha": {
                    "device_family": family,
                    "device_fingerprint_sha256": duplicate_fingerprint,
                    "attestation_challenge_sha256": duplicate_challenge,
                    "unsafe_note": "x-iroha-signature supersecret-signature",
                },
            },
            {
                "slot": "slot-\x1b[31m",
                "status": "ok",
                "errors": ["plain error"],
                "kagemusha": {
                    "device_family": family,
                    "device_fingerprint_sha256": duplicate_fingerprint,
                    "attestation_challenge_sha256": duplicate_challenge,
                },
            },
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                require_kagemusha_standard_matrix=True,
            )
        rendered = json.dumps(summary, allow_nan=False)

        self.assertEqual(summary["slots"][0]["slot"], device_lab.SECRET_PATH_REDACTION)
        self.assertEqual(summary["slots"][1]["slot"], "<unsafe-slot-name>")
        self.assertEqual(
            summary["slots"][0]["errors"],
            [device_lab.SECRET_PATH_REDACTION],
        )
        self.assertEqual(
            summary["slots"][0]["kagemusha"]["unsafe_note"],
            device_lab.SECRET_PATH_REDACTION,
        )
        self.assertEqual(summary["kagemusha"]["covered_device_families"], [])
        self.assertEqual(summary["kagemusha"]["duplicate_bindings"], {})
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("token=supersecret-slot", rendered)
        self.assertNotIn("slot-\\u001b[31m", rendered)
        self.assertNotIn("\\u001b", rendered)

    def test_build_summary_marks_redacted_key_collision_without_overwrite(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                "token=supersecret-first-key": "first-value",
                "token=supersecret-second-key": "second-value",
                "kagemusha": {
                    "token=supersecret-nested-first": "nested-first",
                    "token=supersecret-nested-second": "nested-second",
                },
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)

        slot_summary = summary["slots"][0]
        self.assertTrue(slot_summary[device_lab.SUMMARY_REDACTION_KEY_COLLISION_FIELD])
        self.assertEqual(slot_summary[device_lab.SECRET_PATH_REDACTION], "first-value")
        self.assertEqual(
            slot_summary["kagemusha"][device_lab.SECRET_PATH_REDACTION],
            "nested-first",
        )
        self.assertNotIn("second-value", rendered)
        self.assertNotIn("nested-second", rendered)
        self.assertNotIn("supersecret", rendered)

    def test_build_summary_normalizes_malformed_direct_report_status(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "errors": ["scan aborted"],
                "kagemusha": {"required": True},
            },
            {
                "slot": "slot-1",
                "status": "OK",
                "errors": ["status was uppercase"],
                "kagemusha": {"required": True},
            },
            "not-a-report",
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)

        self.assertEqual(summary["ok"], 0)
        self.assertEqual(summary["failed"], 3)
        self.assertEqual(summary["slots"][0]["status"], "error")
        self.assertEqual(summary["slots"][1]["status"], "error")
        self.assertEqual(summary["slots"][2]["status"], "error")
        self.assertTrue(summary["slots"][0][device_lab.SUMMARY_STATUS_NORMALIZED_FIELD])
        self.assertTrue(summary["slots"][1][device_lab.SUMMARY_STATUS_NORMALIZED_FIELD])
        self.assertTrue(summary["slots"][2][device_lab.SUMMARY_STATUS_NORMALIZED_FIELD])
        self.assertNotIn("Traceback", rendered)

    def test_build_summary_normalizes_malformed_direct_report_errors(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                "errors": "token=supersecret-error",
            },
            {
                "slot": "slot-1",
                "status": "error",
                "errors": [
                    "plain error",
                    {"token=supersecret-error-key": "secret-value"},
                    7,
                ],
            },
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)

        self.assertEqual(
            summary["slots"][0]["errors"],
            [device_lab.SUMMARY_ERROR_REDACTION],
        )
        self.assertEqual(
            summary["slots"][1]["errors"],
            [
                "plain error",
                device_lab.SUMMARY_ERROR_REDACTION,
                device_lab.SUMMARY_ERROR_REDACTION,
            ],
        )
        self.assertTrue(summary["slots"][0][device_lab.SUMMARY_ERRORS_NORMALIZED_FIELD])
        self.assertTrue(summary["slots"][1][device_lab.SUMMARY_ERRORS_NORMALIZED_FIELD])
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("secret-value", rendered)

    def test_build_summary_normalizes_non_string_direct_report_keys(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                7: "first-value",
                ("tuple-key",): "second-value",
                "kagemusha": {11: "nested-first", 12: "nested-second"},
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)
        slot_summary = summary["slots"][0]

        self.assertTrue(
            slot_summary[device_lab.SUMMARY_NON_STRING_KEY_NORMALIZED_FIELD]
        )
        self.assertTrue(slot_summary[device_lab.SUMMARY_REDACTION_KEY_COLLISION_FIELD])
        self.assertEqual(
            slot_summary[device_lab.SUMMARY_NON_STRING_KEY_REDACTION],
            "first-value",
        )
        self.assertEqual(
            slot_summary["kagemusha"][device_lab.SUMMARY_NON_STRING_KEY_REDACTION],
            "nested-first",
        )
        self.assertNotIn("second-value", rendered)
        self.assertNotIn("nested-second", rendered)
        self.assertNotIn("tuple-key", rendered)

    def test_build_summary_redacts_nonfinite_direct_report_values(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                "duration_seconds": float("nan"),
                "kagemusha": {
                    "required": True,
                    "latency_seconds": float("inf"),
                },
                "samples": [1.0, float("-inf")],
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)
        slot_summary = summary["slots"][0]

        self.assertTrue(
            slot_summary[device_lab.SUMMARY_NONFINITE_NUMBER_NORMALIZED_FIELD]
        )
        self.assertEqual(
            slot_summary["duration_seconds"],
            device_lab.SUMMARY_NONFINITE_NUMBER_REDACTION,
        )
        self.assertEqual(
            slot_summary["kagemusha"]["latency_seconds"],
            device_lab.SUMMARY_NONFINITE_NUMBER_REDACTION,
        )
        self.assertEqual(
            slot_summary["samples"][1],
            device_lab.SUMMARY_NONFINITE_NUMBER_REDACTION,
        )
        self.assertNotIn("NaN", rendered)
        self.assertNotIn("Infinity", rendered)

    def test_build_summary_normalizes_finite_float_direct_report_values(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                "duration_seconds": 0.25,
                "kagemusha": {
                    "required": True,
                    "latency_seconds": 1.5,
                },
                "samples": [1.0, {"nested": 2.5}],
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)
        slot_summary = summary["slots"][0]

        self.assertTrue(
            slot_summary[device_lab.SUMMARY_UNSUPPORTED_VALUE_NORMALIZED_FIELD]
        )
        self.assertEqual(
            slot_summary["duration_seconds"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["kagemusha"]["latency_seconds"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["samples"][0],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["samples"][1]["nested"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertNotIn("0.25", rendered)
        self.assertNotIn("1.5", rendered)
        self.assertNotIn("2.5", rendered)

    def test_build_summary_normalizes_unsupported_direct_report_values(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                "raw_bytes": b"token=supersecret-bytes",
                "path_value": Path("token=supersecret-path"),
                "kagemusha": {
                    "required": True,
                    "object_value": object(),
                },
                "samples": [{"nested_set"}],
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)
        slot_summary = summary["slots"][0]

        self.assertTrue(
            slot_summary[device_lab.SUMMARY_UNSUPPORTED_VALUE_NORMALIZED_FIELD]
        )
        self.assertEqual(
            slot_summary["raw_bytes"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["path_value"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["kagemusha"]["object_value"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["samples"][0],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("nested_set", rendered)

    def test_build_summary_normalizes_malformed_kagemusha_report_shape(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "ok",
                "kagemusha": "token=supersecret-kagemusha",
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                require_kagemusha_standard_matrix=True,
            )
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)
        slot_summary = summary["slots"][0]

        self.assertTrue(slot_summary[device_lab.SUMMARY_KAGEMUSHA_SHAPE_NORMALIZED_FIELD])
        self.assertEqual(slot_summary["kagemusha"], {})
        self.assertEqual(summary["kagemusha"]["covered_device_families"], [])
        self.assertEqual(summary["kagemusha"]["duplicate_bindings"], {})
        self.assertEqual(
            summary["kagemusha"]["missing_device_families"],
            list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
        )
        self.assertNotIn("supersecret", rendered)

    def test_build_summary_ignores_malformed_direct_device_family_values(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "ok",
                "kagemusha": {
                    "device_family": [
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]
                    ],
                },
            },
            {
                "slot": "slot-1",
                "status": "ok",
                "kagemusha": {
                    "device_family": "token=supersecret-family",
                },
            },
            {
                "slot": "slot-2",
                "status": "ok",
                "kagemusha": {
                    "device_family": "Unreviewed Device",
                },
            },
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                require_kagemusha_standard_matrix=True,
            )
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)

        self.assertEqual(summary["kagemusha"]["covered_device_families"], [])
        self.assertEqual(
            summary["kagemusha"]["missing_device_families"],
            list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
        )
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("Unreviewed Device", summary["kagemusha"]["covered_device_families"])

    def test_build_summary_requires_complete_signed_evidence_for_kagemusha_rollup(
        self,
    ) -> None:
        duplicate_fingerprint = "a" * 64
        reports = [
            {
                "slot": "slot-0",
                "status": "ok",
                "kagemusha": {
                    "required": True,
                    "device_family": device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                    "device_fingerprint_sha256": duplicate_fingerprint,
                    "attestation_challenge_sha256": "b" * 64,
                },
            },
            {
                "slot": "slot-1",
                "status": "ok",
                "kagemusha": {
                    "required": True,
                    "device_family": device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
                    "device_fingerprint_sha256": duplicate_fingerprint,
                    "attestation_challenge_sha256": "c" * 64,
                },
            },
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        self.assertEqual(summary["kagemusha"]["covered_device_families"], [])
        self.assertEqual(summary["kagemusha"]["duplicate_bindings"], {})
        self.assertEqual(
            summary["kagemusha"]["missing_device_families"],
            list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
        )

    def test_build_summary_preserves_complete_signed_evidence_for_kagemusha_rollup(
        self,
    ) -> None:
        duplicate_fingerprint = "a" * 64
        first_family = device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]
        second_family = device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1]
        reports = [
            summary_release_report(
                "slot-0",
                first_family,
                device_fingerprint_sha256=duplicate_fingerprint,
                attestation_challenge_sha256="b" * 64,
            ),
            summary_release_report(
                "slot-1",
                second_family,
                device_fingerprint_sha256=duplicate_fingerprint,
                attestation_challenge_sha256="c" * 64,
            ),
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        self.assertEqual(
            summary["kagemusha"]["covered_device_families"],
            sorted([first_family, second_family]),
        )
        duplicate = summary["kagemusha"]["duplicate_bindings"][
            "device_fingerprint_sha256"
        ][0]
        self.assertEqual(duplicate["slots"], ["slot-0", "slot-1"])
        self.assertEqual(duplicate["value_sha256"], duplicate_fingerprint)

    def test_build_summary_requires_trusted_signer_for_kagemusha_rollup(
        self,
    ) -> None:
        family = device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]
        report = summary_release_report("slot-0", family)

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                [report],
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"9" * 64: Path(temp) / "other.pem"},
            )

        self.assertEqual(summary["kagemusha"]["covered_device_families"], [])
        self.assertEqual(summary["kagemusha"]["duplicate_bindings"], {})
        self.assertEqual(
            summary["kagemusha"]["missing_device_families"],
            list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
        )

    def test_build_summary_prunes_untrusted_release_kagemusha_slot_fields(
        self,
    ) -> None:
        report = summary_release_report(
            "slot-0",
            operator_note="retained diagnostic",
        )

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                [report],
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"9" * 64: Path(temp) / "other.pem"},
            )

        slot_kagemusha = summary["slots"][0]["kagemusha"]

        self.assertEqual(summary["kagemusha"]["covered_device_families"], [])
        self.assertEqual(summary["kagemusha"]["covered_d2d_payment_transports"], [])
        self.assertEqual(slot_kagemusha["operator_note"], "retained diagnostic")
        for field in device_lab.KAGEMUSHA_SUMMARY_RELEASE_SLOT_FIELDS:
            self.assertNotIn(field, slot_kagemusha)

    def test_build_summary_prunes_incomplete_release_kagemusha_slot_fields(
        self,
    ) -> None:
        report = summary_release_report(
            "slot-0",
            operator_note="retained diagnostic",
        )
        del report["kagemusha"]["d2d_payment_transcript_sha256"]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                [report],
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        slot_kagemusha = summary["slots"][0]["kagemusha"]

        self.assertEqual(summary["kagemusha"]["covered_device_families"], [])
        self.assertEqual(summary["kagemusha"]["covered_d2d_payment_transports"], [])
        self.assertEqual(slot_kagemusha["operator_note"], "retained diagnostic")
        for field in device_lab.KAGEMUSHA_SUMMARY_RELEASE_SLOT_FIELDS:
            self.assertNotIn(field, slot_kagemusha)

    def test_build_summary_reports_release_d2d_transport_matrix_coverage(self) -> None:
        transports = sorted(device_lab.D2D_PAYMENT_TRANSPORTS)
        reports = [
            summary_release_report(
                f"slot-{index}",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[index],
                d2d_payment_transport=transport,
            )
            for index, transport in enumerate(transports)
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        self.assertEqual(
            summary["kagemusha"]["required_d2d_payment_transports"],
            transports,
        )
        self.assertEqual(
            summary["kagemusha"]["covered_d2d_payment_transports"],
            transports,
        )
        self.assertEqual(summary["kagemusha"]["missing_d2d_payment_transports"], [])

    def test_build_summary_requires_d2d_transcript_map_for_declared_transport_list(
        self,
    ) -> None:
        transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
        reports = [
            summary_release_report(
                "slot-0",
                d2d_payment_transport=transports[0],
                d2d_payment_transports=list(transports),
            )
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        self.assertEqual(summary["kagemusha"]["covered_d2d_payment_transports"], [])
        self.assertEqual(
            summary["kagemusha"]["missing_d2d_payment_transports"],
            list(transports),
        )

    def test_build_summary_requires_exact_d2d_transcript_map_bindings(self) -> None:
        cases = (
            ("missing-transcript", lambda bindings, transports: bindings.pop(transports[-1])),
            (
                "bad-transcript-digest",
                lambda bindings, transports: bindings.__setitem__(
                    transports[-1],
                    {"path": "handoff/d2d-payment-forged.json", "sha256": "0" * 64},
                ),
            ),
            (
                "non-handoff-transcript-path",
                lambda bindings, transports: bindings.__setitem__(
                    transports[-1],
                    {
                        "path": "telemetry/d2d-payment-forged.json",
                        "sha256": hashlib.sha256(b"safe-non-handoff").hexdigest(),
                    },
                ),
            ),
            (
                "primary-binding-drift",
                lambda bindings, transports: bindings.__setitem__(
                    transports[0],
                    {"path": "handoff/forged-primary.json", "sha256": "7" * 64},
                ),
            ),
        )
        for name, mutate in cases:
            with self.subTest(name=name):
                transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
                bindings = summary_d2d_transcript_bindings(
                    transports,
                    primary_transport=transports[0],
                )
                mutate(bindings, transports)
                reports = [
                    summary_release_report(
                        "slot-0",
                        d2d_payment_transport=transports[0],
                        d2d_payment_transports=list(transports),
                        d2d_payment_transcripts=bindings,
                    )
                ]

                with tempfile.TemporaryDirectory() as temp:
                    summary = device_lab.build_summary(
                        Path(temp),
                        reports,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
                    )

                self.assertEqual(summary["kagemusha"]["covered_d2d_payment_transports"], [])
                self.assertEqual(
                    summary["kagemusha"]["missing_d2d_payment_transports"],
                    list(transports),
                )

    def test_build_summary_rejects_reused_d2d_transcript_path(self) -> None:
        transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
        primary_transport = transports[0]
        reused_transport = transports[-1]
        bindings = summary_d2d_transcript_bindings(
            transports,
            primary_transport=primary_transport,
        )
        bindings[reused_transport]["path"] = bindings[primary_transport]["path"]
        bindings[reused_transport]["sha256"] = hashlib.sha256(
            b"kagemusha-summary-reused-d2d-path"
        ).hexdigest()
        reports = [
            summary_release_report(
                "slot-0",
                d2d_payment_transport=primary_transport,
                d2d_payment_transcript_path=bindings[primary_transport]["path"],
                d2d_payment_transcript_sha256=bindings[primary_transport]["sha256"],
                d2d_payment_transports=list(transports),
                d2d_payment_transcripts=bindings,
            )
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        self.assertEqual(summary["kagemusha"]["covered_d2d_payment_transports"], [])
        self.assertEqual(
            summary["kagemusha"]["missing_d2d_payment_transports"],
            list(transports),
        )

    def test_build_summary_rejects_reused_d2d_transcript_digest(self) -> None:
        transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
        primary_transport = transports[0]
        reused_transport = transports[-1]
        bindings = summary_d2d_transcript_bindings(
            transports,
            primary_transport=primary_transport,
        )
        bindings[reused_transport]["sha256"] = bindings[primary_transport]["sha256"]
        reports = [
            summary_release_report(
                "slot-0",
                d2d_payment_transport=primary_transport,
                d2d_payment_transcript_path=bindings[primary_transport]["path"],
                d2d_payment_transcript_sha256=bindings[primary_transport]["sha256"],
                d2d_payment_transports=list(transports),
                d2d_payment_transcripts=bindings,
            )
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        self.assertEqual(summary["kagemusha"]["covered_d2d_payment_transports"], [])
        self.assertEqual(
            summary["kagemusha"]["missing_d2d_payment_transports"],
            list(transports),
        )

    def test_build_summary_requires_canonical_d2d_transport_list(self) -> None:
        transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
        cases = (
            ("duplicate-transport", [transports[0], transports[0], transports[1]]),
            ("unsorted-transport", list(reversed(transports))),
        )
        for name, declared in cases:
            with self.subTest(name=name):
                bindings = summary_d2d_transcript_bindings(
                    transports,
                    primary_transport=transports[0],
                )
                reports = [
                    summary_release_report(
                        "slot-0",
                        d2d_payment_transport=transports[0],
                        d2d_payment_transcript_path=bindings[transports[0]]["path"],
                        d2d_payment_transcript_sha256=bindings[transports[0]]["sha256"],
                        d2d_payment_transports=declared,
                        d2d_payment_transcripts=bindings,
                    )
                ]

                with tempfile.TemporaryDirectory() as temp:
                    summary = device_lab.build_summary(
                        Path(temp),
                        reports,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
                    )

                self.assertEqual(summary["kagemusha"]["covered_d2d_payment_transports"], [])
                self.assertEqual(
                    summary["kagemusha"]["missing_d2d_payment_transports"],
                    list(transports),
                )

    def test_build_summary_accepts_bound_d2d_transcript_map(self) -> None:
        transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
        bindings = summary_d2d_transcript_bindings(
            transports,
            primary_transport=transports[0],
        )
        reports = [
            summary_release_report(
                "slot-0",
                d2d_payment_transport=transports[0],
                d2d_payment_transcript_path=bindings[transports[0]]["path"],
                d2d_payment_transcript_sha256=bindings[transports[0]]["sha256"],
                d2d_payment_transports=list(transports),
                d2d_payment_transcripts=bindings,
            )
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        self.assertEqual(
            summary["kagemusha"]["covered_d2d_payment_transports"],
            list(transports),
        )
        self.assertEqual(summary["kagemusha"]["missing_d2d_payment_transports"], [])

    def test_build_summary_ignores_malformed_release_d2d_transport_values(self) -> None:
        reports = [
            summary_release_report("slot-0", d2d_payment_transport="nfc_hce "),
            summary_release_report("slot-1", d2d_payment_transport="bluetooth"),
            summary_release_report("slot-2", d2d_payment_transport=["qr"]),
            summary_release_report(
                "slot-3",
                d2d_payment_transcript_path="telemetry/d2d-payment.json",
            ),
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        self.assertEqual(summary["kagemusha"]["covered_d2d_payment_transports"], [])
        self.assertEqual(
            summary["kagemusha"]["missing_d2d_payment_transports"],
            sorted(device_lab.D2D_PAYMENT_TRANSPORTS),
        )

    def test_build_summary_rejects_release_d2d_transcript_root_handoff_path(
        self,
    ) -> None:
        reports = [
            summary_release_report(
                "slot-0",
                d2d_payment_transcript_path="handoff",
            )
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
            )

        self.assertEqual(summary["kagemusha"]["covered_d2d_payment_transports"], [])
        self.assertEqual(
            summary["kagemusha"]["missing_d2d_payment_transports"],
            sorted(device_lab.D2D_PAYMENT_TRANSPORTS),
        )

    def test_build_summary_requires_release_artifact_paths_under_expected_roots(
        self,
    ) -> None:
        cases = (
            ("attestation_certificate_chain_path", "evidence/keymint-chain.pem"),
            ("kagemusha_wallet_apk_path", "logs/runtime.log"),
            ("wallet_integrity_transcript_path", "handoff/integrity.json"),
        )
        for field, value in cases:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temp:
                report = summary_release_report(**{field: value})
                summary = device_lab.build_summary(
                    Path(temp),
                    [report],
                    require_kagemusha_production_evidence=True,
                    trusted_signer_public_keys={"4" * 64: Path(temp) / "safe.pem"},
                )

                self.assertEqual(summary["kagemusha"]["covered_device_families"], [])
                self.assertEqual(
                    summary["kagemusha"]["covered_d2d_payment_transports"],
                    [],
                )
                self.assertEqual(
                    summary["kagemusha"]["missing_d2d_payment_transports"],
                    sorted(device_lab.D2D_PAYMENT_TRANSPORTS),
                )

    def test_build_summary_rejects_malformed_complete_signed_evidence_rollup_fields(
        self,
    ) -> None:
        cases = (
            ("slot", "token=supersecret-slot"),
            ("device_model", "Unreviewed Model"),
            ("native_bridge_abi_version", 18),
            ("signed_at_utc", "2026-06-06T00:00:00+00:00"),
            ("kagemusha_wallet_apk_path", "../evidence/kagemusha-wallet-release.apk"),
            ("kagemusha_wallet_apk_sha256", "0" * 64),
        )
        for field, value in cases:
            with self.subTest(field=field):
                report = summary_release_report()
                if field == "slot":
                    report["slot"] = value
                else:
                    report["kagemusha"][field] = value

                with tempfile.TemporaryDirectory() as temp:
                    summary = device_lab.build_summary(
                        Path(temp),
                        [report],
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys={
                            "4" * 64: Path(temp) / "safe.pem"
                        },
                    )

                self.assertEqual(summary["kagemusha"]["covered_device_families"], [])
                self.assertEqual(summary["kagemusha"]["duplicate_bindings"], {})

    def test_build_summary_ignores_non_sha256_direct_trusted_signer_keys(self) -> None:
        signer_digest = "d" * 64
        reports = [
            {
                "slot": "slot-0",
                "status": "ok",
                "kagemusha": {
                    "device_family": device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                },
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={
                    signer_digest: Path(temp) / "safe.pem",
                    "token=supersecret-signer": Path(temp) / "unsafe.pem",
                    "E" * 64: Path(temp) / "uppercase.pem",
                },
            )
        rendered = json.dumps(summary, allow_nan=False)

        self.assertEqual(
            summary["kagemusha"]["trusted_signer_public_key_sha256"],
            [signer_digest],
        )
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("E" * 64, rendered)

    def test_build_summary_ignores_zero_direct_trusted_signer_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                [],
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={
                    "0" * 64: Path(temp) / "zero.pem",
                    "d" * 64: Path(temp) / "safe.pem",
                },
            )

        self.assertEqual(
            summary["kagemusha"]["trusted_signer_public_key_sha256"],
            ["d" * 64],
        )

    def test_build_summary_ignores_non_mapping_direct_trusted_signer_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                [],
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=[("d" * 64, Path(temp) / "safe.pem")],  # type: ignore[arg-type]
            )

        self.assertEqual(summary["kagemusha"]["trusted_signer_public_key_sha256"], [])

    def test_build_summary_ignores_mixed_direct_trusted_signer_key_types(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                [],
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={
                    7: Path(temp) / "integer.pem",
                    "d" * 64: Path(temp) / "safe.pem",
                },  # type: ignore[arg-type]
            )

        self.assertEqual(
            summary["kagemusha"]["trusted_signer_public_key_sha256"],
            ["d" * 64],
        )

    def test_json_summary_reports_kagemusha_matrix_and_signer_pins(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            transports = sorted(device_lab.D2D_PAYMENT_TRANSPORTS)
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
                create_slot(
                    root,
                    f"slot-{index}",
                    family,
                    signer,
                    d2d_payment_transport=transports[index % len(transports)],
                    d2d_payment_transports=tuple(transports),
                )
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-standard-matrix",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        self.assertEqual(status, 0)
        self.assertEqual(summary["ok"], len(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES))
        self.assertEqual(summary["failed"], 0)
        self.assertEqual(summary["root"], device_lab.DEVICE_LAB_ROOT_SUMMARY_LABEL)
        self.assertTrue(summary["kagemusha"]["production_evidence_required"])
        self.assertTrue(summary["kagemusha"]["standard_matrix_required"])
        self.assertEqual(
            summary["kagemusha"]["required_device_families"],
            list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
        )
        self.assertEqual(
            sorted(summary["kagemusha"]["covered_device_families"]),
            sorted(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
        )
        self.assertEqual(summary["kagemusha"]["missing_device_families"], [])
        self.assertEqual(
            summary["kagemusha"]["required_d2d_payment_transports"],
            transports,
        )
        self.assertEqual(
            summary["kagemusha"]["covered_d2d_payment_transports"],
            transports,
        )
        self.assertEqual(summary["kagemusha"]["missing_d2d_payment_transports"], [])
        self.assertEqual(
            summary["kagemusha"]["covered_d2d_payment_transports_by_family"],
            {
                family: transports
                for family in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
            },
        )
        self.assertEqual(
            summary["kagemusha"]["missing_d2d_payment_transport_pairs"],
            [],
        )
        self.assertEqual(
            summary["kagemusha"]["trusted_signer_public_key_sha256"],
            [signer["public_key_sha256"]],
        )
        first_slot = summary["slots"][0]["kagemusha"]
        self.assertEqual(
            first_slot["kagemusha_wallet_apk_path"],
            "evidence/kagemusha-wallet-release.apk",
        )
        self.assertRegex(first_slot["kagemusha_wallet_apk_sha256"], r"^[0-9a-f]{64}$")
        self.assertEqual(
            first_slot["native_bridge_abi_version"],
            device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
        )

    def test_json_summary_reports_d2d_gaps_without_standard_matrix_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(
                root,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
                d2d_payment_transport="nearby_offline",
            )
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        self.assertEqual(status, 0)
        self.assertFalse(summary["kagemusha"]["standard_matrix_required"])
        self.assertEqual(
            summary["kagemusha"]["covered_d2d_payment_transports"],
            ["nearby_offline"],
        )
        self.assertEqual(
            summary["kagemusha"]["missing_d2d_payment_transports"],
            ["nfc_hce", "qr"],
        )

    def test_json_summary_counts_multi_transport_d2d_transcripts(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            transports = tuple(sorted(device_lab.D2D_PAYMENT_TRANSPORTS))
            create_slot(
                root,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
                d2d_payment_transport=transports[0],
                d2d_payment_transports=transports,
            )
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        self.assertEqual(status, 0)
        self.assertEqual(summary["kagemusha"]["covered_d2d_payment_transports"], list(transports))
        self.assertEqual(summary["kagemusha"]["missing_d2d_payment_transports"], [])

    def test_json_summary_does_not_leak_trusted_signer_key_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary_text = summary_path.read_text(encoding="utf-8")

        self.assertEqual(status, 0)
        self.assertNotIn(str(signer["public_key"]), summary_text)
        self.assertNotIn(str(signer["private_key"]), summary_text)
        self.assertIn(str(signer["public_key_sha256"]), summary_text)

    def test_json_summary_does_not_leak_device_lab_root_or_summary_output_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            stdout = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary_text = summary_path.read_text(encoding="utf-8")
            stdout_text = stdout.getvalue()

        self.assertEqual(status, 0)
        self.assertNotIn(str(root), summary_text)
        self.assertNotIn(str(summary_path), stdout_text)
        self.assertIn(device_lab.DEVICE_LAB_ROOT_SUMMARY_LABEL, summary_text)
        self.assertIn("[device-lab] wrote summary", stdout_text)

    def test_main_rejects_secret_looking_root_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_root = Path(temp) / "token=supersecret-slots"
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(secret_root),
                        "--allow-missing-root",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("--root must not contain secret-looking material", rendered)
        self.assertNotIn(str(secret_root), rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("root missing; skipping", rendered)

    def test_main_rejects_control_root_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            control_root = Path(temp) / "control\nslots"
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(control_root),
                        "--allow-missing-root",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("--root must not contain control characters", rendered)
        self.assertNotIn(str(control_root), rendered)
        self.assertNotIn("root missing; skipping", rendered)

    def test_device_lab_root_rejects_surrounding_whitespace_before_metadata(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        def unexpected_lstat(*args, **kwargs):
            raise AssertionError("root metadata must not be read")

        try:
            path_type.lstat = unexpected_lstat
            root_exists, errors = device_lab.classify_device_lab_root_path(
                Path(" slots ")
            )
        finally:
            path_type.lstat = original_lstat

        self.assertFalse(root_exists)
        self.assertEqual(
            errors,
            ["device-lab root path must not contain surrounding whitespace"],
        )

    def test_main_rejects_path_surrounding_whitespace_before_root_classify(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer_path = Path(temp) / "trusted-public.pem"
            cases = (
                (
                    ["--root", f" {root}", "--allow-missing-root"],
                    "--root must not contain surrounding whitespace",
                    f" {root}",
                ),
                (
                    [
                        "--root",
                        str(root),
                        "--allow-missing-root",
                        "--json-out",
                        f"{summary_path} ",
                    ],
                    "--json-out must not contain surrounding whitespace",
                    f"{summary_path} ",
                ),
                (
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        f" {signer_path}",
                    ],
                    "--trusted-signer-public-key[0] must not contain surrounding whitespace",
                    f" {signer_path}",
                ),
            )
            for argv, expected_message, unsafe_path in cases:
                with self.subTest(expected_message=expected_message):
                    stdout = io.StringIO()
                    stderr = io.StringIO()
                    with (
                        mock.patch.object(
                            device_lab,
                            "classify_device_lab_root_path",
                            side_effect=AssertionError(
                                "root classification must not run"
                            ),
                        ) as classify_root,
                        mock.patch.object(
                            device_lab,
                            "load_trusted_signer_public_keys",
                            side_effect=AssertionError(
                                "trusted signer loading must not run"
                            ),
                        ) as load_signers,
                        redirect_stdout(stdout),
                        redirect_stderr(stderr),
                    ):
                        status = device_lab.main(argv)
                    rendered = stdout.getvalue() + stderr.getvalue()

                    self.assertEqual(status, 1)
                    classify_root.assert_not_called()
                    load_signers.assert_not_called()
                    self.assertIn(expected_message, rendered)
                    self.assertNotIn(unsafe_path, rendered)

    def test_main_rejects_control_slot_before_root_classify_without_leak(
        self,
    ) -> None:
        unsafe_slot = "slot-a\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            stdout = io.StringIO()
            stderr = io.StringIO()

            with (
                mock.patch.object(
                    device_lab,
                    "classify_device_lab_root_path",
                    side_effect=AssertionError("root classification must not run"),
                ) as classify_root,
                redirect_stdout(stdout),
                redirect_stderr(stderr),
            ):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        unsafe_slot,
                        "--require-slot",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        classify_root.assert_not_called()
        self.assertIn("slot id 0 must not contain control characters", rendered)
        self.assertNotIn(unsafe_slot, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_main_rejects_control_trusted_signer_before_slot_discovery_without_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            root.mkdir()
            unsafe_key = Path(temp) / "control\nsigner.pem"
            stdout = io.StringIO()
            stderr = io.StringIO()

            with (
                mock.patch.object(
                    device_lab,
                    "discover_slots",
                    side_effect=AssertionError("slot discovery must not run"),
                ) as discover_slots,
                mock.patch.object(
                    device_lab,
                    "load_trusted_signer_public_keys",
                    side_effect=AssertionError("trusted signer loading must not run"),
                ) as load_signers,
                redirect_stdout(stdout),
                redirect_stderr(stderr),
            ):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(unsafe_key),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        discover_slots.assert_not_called()
        load_signers.assert_not_called()
        self.assertIn(
            "--trusted-signer-public-key[0] must not contain control characters",
            rendered,
        )
        self.assertNotIn(str(unsafe_key), rendered)

    def test_main_rejects_trusted_signer_aliases_before_root_classify_without_leak(
        self,
    ) -> None:
        cases = (
            (
                Path("keys") / ".." / "trusted-public.pem",
                "--trusted-signer-public-key[0] must be a canonical path",
            ),
            (
                Path("keys\\trusted-public.pem"),
                "--trusted-signer-public-key[0] must not contain backslashes",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            for key_path, expected_message in cases:
                unsafe_key = Path(temp) / key_path
                with self.subTest(unsafe_key=unsafe_key):
                    stdout = io.StringIO()
                    stderr = io.StringIO()

                    with (
                        mock.patch.object(
                            device_lab,
                            "classify_device_lab_root_path",
                            side_effect=AssertionError(
                                "root classification must not run"
                            ),
                        ) as classify_root,
                        mock.patch.object(
                            device_lab,
                            "load_trusted_signer_public_keys",
                            side_effect=AssertionError(
                                "trusted signer loading must not run"
                            ),
                        ) as load_signers,
                        redirect_stdout(stdout),
                        redirect_stderr(stderr),
                    ):
                        status = device_lab.main(
                            [
                                "--root",
                                str(root),
                                "--require-slot",
                                "--require-kagemusha-production-evidence",
                                "--trusted-signer-public-key",
                                str(unsafe_key),
                            ]
                        )
                    rendered = stdout.getvalue() + stderr.getvalue()

                    self.assertEqual(status, 1)
                    classify_root.assert_not_called()
                    load_signers.assert_not_called()
                    self.assertIn(expected_message, rendered)
                    self.assertNotIn(str(unsafe_key), rendered)

    def test_json_summary_rejects_secret_looking_output_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            secret_summary_path = Path(temp) / "token=supersecret-summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(secret_summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertFalse(secret_summary_path.exists())
        self.assertIn("--json-out must not contain secret-looking material", rendered)
        self.assertNotIn(str(secret_summary_path), rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("[device-lab] wrote summary", rendered)

    def test_json_summary_rejects_control_output_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            control_summary_path = Path(temp) / "control\nsummary" / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(control_summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertFalse(control_summary_path.exists())
        self.assertFalse(control_summary_path.parent.exists())
        self.assertIn("--json-out must not contain control characters", rendered)
        self.assertNotIn(str(control_summary_path), rendered)
        self.assertNotIn("[device-lab] wrote summary", rendered)

    def test_main_rejects_json_output_aliases_before_root_classify_without_leak(
        self,
    ) -> None:
        cases = (
            (
                Path("summary\\out.json"),
                "--json-out must not contain backslashes",
            ),
            (
                Path("summary") / ".." / "out.json",
                "--json-out must be a canonical path",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            for summary_path, expected_message in cases:
                unsafe_summary = Path(temp) / summary_path
                with self.subTest(unsafe_summary=unsafe_summary):
                    stdout = io.StringIO()
                    stderr = io.StringIO()

                    with (
                        mock.patch.object(
                            device_lab,
                            "classify_device_lab_root_path",
                            side_effect=AssertionError(
                                "root classification must not run"
                            ),
                        ) as classify_root,
                        redirect_stdout(stdout),
                        redirect_stderr(stderr),
                    ):
                        status = device_lab.main(
                            [
                                "--root",
                                str(root),
                                "--require-slot",
                                "--json-out",
                                str(unsafe_summary),
                            ]
                        )
                    rendered = stdout.getvalue() + stderr.getvalue()

                    self.assertEqual(status, 1)
                    classify_root.assert_not_called()
                    self.assertFalse(unsafe_summary.exists())
                    self.assertIn(expected_message, rendered)
                    self.assertNotIn(str(unsafe_summary), rendered)
                    self.assertNotIn("[device-lab] wrote summary", rendered)

    def test_write_summary_rejects_secret_output_path_directly_without_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_summary_path = Path(temp) / "token=supersecret-summary.json"

            errors = device_lab.write_summary(secret_summary_path, {"ok": False})
            rendered = json.dumps(errors)

        self.assertEqual(errors, ["--json-out must not contain secret-looking material"])
        self.assertFalse(secret_summary_path.exists())
        self.assertNotIn(str(secret_summary_path), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_write_summary_rejects_control_output_path_directly_without_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            control_summary_path = Path(temp) / "control\nsummary" / "summary.json"

            errors = device_lab.write_summary(control_summary_path, {"ok": False})
            rendered = json.dumps(errors)

        self.assertEqual(errors, ["--json-out must not contain control characters"])
        self.assertFalse(control_summary_path.exists())
        self.assertFalse(control_summary_path.parent.exists())
        self.assertNotIn(str(control_summary_path), rendered)

    def test_write_summary_rejects_nonfinite_json_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"

            errors = device_lab.write_summary(summary_path, {"value": float("inf")})
            temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))

        self.assertEqual(errors, ["--json-out summary is not strict JSON"])
        self.assertFalse(summary_path.exists())
        self.assertEqual(temp_files, [])

    def test_write_summary_rejects_oversized_json_before_write(self) -> None:
        summary = {"ok": False}
        summary_text = json.dumps(summary, indent=2, allow_nan=False) + "\n"
        test_limit = len(summary_text) - 1
        old_limit = device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES
        try:
            device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES = test_limit
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"

                errors = device_lab.write_summary(summary_path, summary)
                temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))
        finally:
            device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES = old_limit

        self.assertEqual(
            errors,
            ["--json-out must be no more than " f"{test_limit} bytes"],
        )
        self.assertFalse(summary_path.exists())
        self.assertEqual(temp_files, [])

    def test_validate_summary_output_path_uses_lstat_before_parent_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated scanner summary parent is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                errors = device_lab.validate_summary_output_path(
                    summary_path,
                    "--json-out",
                )
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertFalse(summary_path.exists())

    def test_validate_summary_output_path_rejects_parent_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated scanner summary parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.validate_summary_output_path(
                    summary_path,
                    "--json-out",
                )
                output_exists = summary_path.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["--json-out parent directory metadata could not be read"])
        self.assertFalse(output_exists)

    def test_validate_summary_output_path_rejects_aliases_before_parent_metadata(
        self,
    ) -> None:
        cases = (
            (
                lambda base: base / "summary" / " out.json",
                "--json-out must not contain surrounding whitespace",
            ),
            (
                lambda base: base / "summary\\out.json",
                "--json-out must not contain backslashes",
            ),
            (
                lambda base: base / "summary" / ".." / "out.json",
                "--json-out must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    summary_path = path_factory(Path(temp))

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias summary output should fail before metadata"
                        ),
                    ):
                        errors = device_lab.validate_summary_output_path(
                            summary_path,
                            "--json-out",
                        )

                    output_exists = summary_path.exists()

                self.assertEqual(errors, [expected_error])
                self.assertFalse(output_exists)

    def test_write_summary_uses_lstat_before_parent_is_dir_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated scanner summary writer parent is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                errors = device_lab.write_summary(summary_path, {"ok": False})
                summary_text = summary_path.read_text(encoding="utf-8")
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertIn('"ok": false', summary_text)

    def test_write_summary_rejects_parent_metadata_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated scanner summary writer parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.write_summary(summary_path, {"ok": False})
                output_exists = summary_path.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["--json-out parent directory metadata could not be read"])
        self.assertFalse(output_exists)

    def test_write_summary_rejects_parent_create_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "missing-parent" / "summary.json"

            def failing_mkdir(path: Path, *args, **kwargs):
                if path == summary_path.parent:
                    raise OSError("simulated summary parent mkdir failure")
                return original_mkdir(path, *args, **kwargs)

            with mock.patch.object(path_type, "mkdir", failing_mkdir):
                errors = device_lab.write_summary(summary_path, {"ok": False})

        self.assertEqual(errors, ["--json-out parent directory could not be created"])
        self.assertFalse(summary_path.exists())

    def test_write_summary_rejects_file_metadata_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"
                summary_path.write_text("existing summary\n", encoding="utf-8")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == summary_path:
                        raise OSError("simulated summary lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.write_summary(summary_path, {"ok": False})
                summary_text = summary_path.read_text(encoding="utf-8")
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["--json-out file metadata could not be read"])
        self.assertEqual(summary_text, "existing summary\n")

    def test_write_summary_rejects_hardlink_metadata_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"
                summary_path.write_text("existing summary\n", encoding="utf-8")

                def failing_stat(path: Path, *args, **kwargs):
                    if path == summary_path and kwargs.get("follow_symlinks", True):
                        raise OSError("simulated summary stat failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                errors = device_lab.write_summary(summary_path, {"ok": False})
                summary_text = summary_path.read_text(encoding="utf-8")
        finally:
            path_type.stat = original_stat

        self.assertEqual(errors, ["--json-out hardlink metadata could not be read"])
        self.assertEqual(summary_text, "existing summary\n")

    def test_write_summary_rejects_write_failure_after_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"

            with mock.patch.object(
                device_lab.os,
                "fsync",
                side_effect=OSError("simulated summary fsync failure"),
            ):
                errors = device_lab.write_summary(summary_path, {"ok": False})

        self.assertEqual(
            errors,
            [
                "--json-out could not be written",
                "--json-out temporary file cleanup could not be synced",
            ],
        )
        self.assertFalse(summary_path.exists())

    def test_write_summary_preserves_existing_output_on_replace_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            summary_path.write_text("existing summary\n", encoding="utf-8")

            with mock.patch.object(
                device_lab.os,
                "replace",
                side_effect=OSError("simulated summary replace failure"),
            ):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            summary_text = summary_path.read_text(encoding="utf-8")
            temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))

        self.assertEqual(errors, ["--json-out could not be written"])
        self.assertEqual(summary_text, "existing summary\n")
        self.assertEqual(temp_files, [])

    def test_write_summary_reports_temp_cleanup_failure_after_write_failure(
        self,
    ) -> None:
        original_unlink = device_lab.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"

            def failing_replace(src, dst, *args, **kwargs) -> None:
                raise OSError("simulated summary replace failure")

            def failing_temp_unlink(path: str, *args, **kwargs):
                if (
                    Path(path).name.startswith(f".{summary_path.name}.")
                    and Path(path).suffix == ".tmp"
                ):
                    raise OSError("simulated summary temp cleanup failure")
                return original_unlink(path, *args, **kwargs)

            with (
                mock.patch.object(device_lab.os, "replace", failing_replace),
                mock.patch.object(device_lab.os, "unlink", failing_temp_unlink),
            ):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))

        self.assertEqual(
            errors,
            [
                "--json-out could not be written",
                "--json-out temporary file could not be removed",
            ],
        )
        self.assertEqual(len(temp_files), 1)

    def test_write_summary_reports_temp_cleanup_failure_after_post_stage_validation_failure(
        self,
    ) -> None:
        original_validate = device_lab.validate_summary_output_path
        original_unlink = device_lab.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            validation_calls = 0

            def racing_validate(path: Path, label: str) -> list[str]:
                nonlocal validation_calls
                if path == summary_path and label == "--json-out":
                    validation_calls += 1
                    if validation_calls == 2:
                        return ["--json-out changed after staging"]
                return original_validate(path, label)

            def failing_temp_unlink(path: str, *args, **kwargs):
                if (
                    Path(path).name.startswith(f".{summary_path.name}.")
                    and Path(path).suffix == ".tmp"
                ):
                    raise OSError("simulated summary temp cleanup failure")
                return original_unlink(path, *args, **kwargs)

            with (
                mock.patch.object(
                    device_lab,
                    "validate_summary_output_path",
                    racing_validate,
                ),
                mock.patch.object(device_lab.os, "unlink", failing_temp_unlink),
            ):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))
            output_exists = summary_path.exists()

        self.assertEqual(
            errors,
            [
                "--json-out changed after staging",
                "--json-out temporary file could not be removed",
            ],
        )
        self.assertEqual(validation_calls, 2)
        self.assertFalse(output_exists)
        self.assertEqual(len(temp_files), 1)

    def test_write_summary_temp_cleanup_rejects_swapped_temp_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp) / ".summary.json.swap.tmp"
            temp_path.write_text("original\n", encoding="utf-8")
            temp_identity = device_lab._file_identity(temp_path.lstat())
            swapped_temp = Path(temp) / "original-temp-file"
            temp_path.rename(swapped_temp)
            temp_path.write_text("do not remove\n", encoding="utf-8")

            errors = device_lab._cleanup_summary_output(
                temp_path,
                temp_identity,
            )
            victim_survived = temp_path.read_text(encoding="utf-8")
            original_survived = swapped_temp.read_text(encoding="utf-8")

        self.assertEqual(errors, ["--json-out temporary file changed before cleanup"])
        self.assertEqual(victim_survived, "do not remove\n")
        self.assertEqual(original_survived, "original\n")

    def test_write_summary_temp_cleanup_reports_sync_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp) / ".summary.json.sync.tmp"
            temp_path.write_text("original\n", encoding="utf-8")
            temp_identity = device_lab._file_identity(temp_path.lstat())

            with mock.patch.object(
                device_lab.os,
                "fsync",
                side_effect=OSError("simulated summary cleanup sync failure"),
            ):
                errors = device_lab._cleanup_summary_output(
                    temp_path,
                    temp_identity,
                )

            temp_exists = temp_path.exists()

        self.assertEqual(
            errors,
            ["--json-out temporary file cleanup could not be synced"],
        )
        self.assertFalse(temp_exists)

    def test_write_summary_rejects_parent_directory_sync_failure_after_replace(
        self,
    ) -> None:
        original_fsync = device_lab.os.fsync

        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            sync_calls = 0

            def failing_parent_fsync(fd: int) -> None:
                nonlocal sync_calls
                sync_calls += 1
                if sync_calls == 2:
                    raise OSError("simulated summary parent sync failure")
                original_fsync(fd)

            with mock.patch.object(device_lab.os, "fsync", failing_parent_fsync):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            output_exists = summary_path.exists()
            temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))

        self.assertEqual(sync_calls, 3)
        self.assertEqual(errors, ["--json-out parent directory could not be synced"])
        self.assertFalse(output_exists)
        self.assertEqual(temp_files, [])

    def test_write_summary_parent_sync_cleanup_reports_failure(self) -> None:
        original_fsync = device_lab.os.fsync
        original_unlink = device_lab.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            sync_calls = 0

            def failing_parent_fsync(fd: int) -> None:
                nonlocal sync_calls
                sync_calls += 1
                if sync_calls == 2:
                    raise OSError("simulated summary parent sync failure")
                original_fsync(fd)

            def failing_summary_unlink(path: str, *args, **kwargs):
                if path == summary_path.name:
                    raise OSError("simulated summary rollback failure")
                return original_unlink(path, *args, **kwargs)

            with (
                mock.patch.object(device_lab.os, "fsync", failing_parent_fsync),
                mock.patch.object(device_lab.os, "unlink", failing_summary_unlink),
            ):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            output_exists = summary_path.exists()

        self.assertEqual(sync_calls, 2)
        self.assertEqual(
            errors,
            [
                "--json-out parent directory could not be synced",
                "--json-out could not be removed after parent sync failure",
            ],
        )
        self.assertTrue(output_exists)

    def test_write_summary_published_cleanup_preserves_swap(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            summary_path = root / "summary.json"
            summary_path.write_text("original\n", encoding="utf-8")
            summary_identity = device_lab._file_identity(summary_path.lstat())
            swapped_summary = root / "original-summary.json"
            summary_path.rename(swapped_summary)
            summary_path.write_text("do not remove\n", encoding="utf-8")
            parent_fd = os.open(root, device_lab._directory_open_flags())
            try:
                errors = device_lab._unlink_summary_output_if_identity_at(
                    parent_fd,
                    summary_path.name,
                    summary_identity,
                )
            finally:
                os.close(parent_fd)
            replacement = summary_path.read_text(encoding="utf-8")
            original = swapped_summary.read_text(encoding="utf-8")

        self.assertEqual(errors, [])
        self.assertEqual(replacement, "do not remove\n")
        self.assertEqual(original, "original\n")

    def test_write_summary_published_cleanup_reports_sync_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            summary_path = root / "summary.json"
            summary_path.write_text("original\n", encoding="utf-8")
            summary_identity = device_lab._file_identity(summary_path.lstat())
            parent_fd = os.open(root, device_lab._directory_open_flags())
            try:
                with mock.patch.object(
                    device_lab.os,
                    "fsync",
                    side_effect=OSError("simulated summary cleanup sync failure"),
                ):
                    errors = device_lab._unlink_summary_output_if_identity_at(
                        parent_fd,
                        summary_path.name,
                        summary_identity,
                    )
            finally:
                os.close(parent_fd)
            summary_exists = summary_path.exists()

        self.assertEqual(
            errors,
            ["--json-out cleanup could not be synced after parent sync failure"],
        )
        self.assertFalse(summary_exists)

    def test_write_summary_rejects_parent_directory_identity_swap_before_sync(
        self,
    ) -> None:
        original_open = device_lab.os.open

        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            root = wrapper / "summary-output-root"
            root.mkdir()
            summary_path = root / "summary.json"
            swapped_root = wrapper / "summary-output-root-swapped"
            swapped = False

            def swapping_parent_open(path: Path, flags: int, *args, **kwargs):
                nonlocal swapped
                if Path(path) == summary_path.parent and not swapped:
                    summary_path.parent.rename(swapped_root)
                    summary_path.parent.mkdir()
                    swapped = True
                return original_open(path, flags, *args, **kwargs)

            with mock.patch.object(device_lab.os, "open", swapping_parent_open):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            output_exists = summary_path.exists()
            swapped_output_exists = (swapped_root / summary_path.name).exists()

        self.assertTrue(swapped)
        self.assertEqual(errors, ["--json-out parent directory changed before sync"])
        self.assertFalse(output_exists)
        self.assertFalse(swapped_output_exists)

    def test_write_summary_rejects_parent_symlink_swap_before_sync_with_cleanup(
        self,
    ) -> None:
        original_replace = device_lab.os.replace

        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            root = wrapper / "summary-output-root"
            root.mkdir()
            summary_path = root / "summary.json"
            swapped_root = wrapper / "summary-output-root-swapped"
            symlink_target = wrapper / "summary-output-root-link-target"
            symlink_target.mkdir()
            swapped = False

            def swapping_replace(src, dst, *args, **kwargs):
                nonlocal swapped
                original_replace(src, dst, *args, **kwargs)
                summary_path.parent.rename(swapped_root)
                try:
                    summary_path.parent.symlink_to(
                        symlink_target,
                        target_is_directory=True,
                    )
                except (NotImplementedError, OSError) as exc:
                    self.skipTest(
                        f"symlinks are not available in this test environment: {exc}"
                    )
                swapped = True

            with mock.patch.object(device_lab.os, "replace", swapping_replace):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            original_output_exists = (swapped_root / summary_path.name).exists()
            symlink_output_exists = summary_path.exists()

        self.assertTrue(swapped)
        self.assertEqual(errors, ["--json-out parent directory changed before sync"])
        self.assertFalse(original_output_exists)
        self.assertFalse(symlink_output_exists)

    def test_write_summary_rejects_symlink_swap_before_replace(self) -> None:
        original_validate = device_lab.validate_summary_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                summary_path = root / "summary.json"
                alias_target = root / "external-summary.json"
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == summary_path and calls == 2:
                        alias_target.write_text("external\n", encoding="utf-8")
                        try:
                            path.symlink_to(alias_target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                device_lab.validate_summary_output_path = validate_then_alias
                errors = device_lab.write_summary(summary_path, {"ok": False})
                target_text = alias_target.read_text(encoding="utf-8")
                temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))
        finally:
            device_lab.validate_summary_output_path = original_validate

        self.assertEqual(errors, ["--json-out must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertEqual(temp_files, [])

    def test_write_summary_rejects_readback_mismatch(self) -> None:
        original_read_summary_output_text = device_lab._read_summary_output_text  # type: ignore[attr-defined]

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"

                def mismatching_read_summary_output_text(
                    path: Path,
                    expected_stat: os.stat_result,
                ) -> tuple[str | None, list[str]]:
                    if path == summary_path:
                        return '{"ok": true}\n', []
                    return original_read_summary_output_text(path, expected_stat)

                device_lab._read_summary_output_text = mismatching_read_summary_output_text  # type: ignore[attr-defined]
                errors = device_lab.write_summary(summary_path, {"ok": False})
                summary_text = summary_path.read_text(encoding="utf-8")
        finally:
            device_lab._read_summary_output_text = original_read_summary_output_text  # type: ignore[attr-defined]

        self.assertEqual(errors, ["--json-out write verification failed"])
        self.assertEqual(summary_text, '{\n  "ok": false\n}\n')

    def test_write_summary_rejects_readback_failure(self) -> None:
        original_read_summary_output_text = device_lab._read_summary_output_text  # type: ignore[attr-defined]

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"

                def failing_read_summary_output_text(
                    path: Path,
                    expected_stat: os.stat_result,
                ) -> tuple[str | None, list[str]]:
                    if path == summary_path:
                        return None, ["--json-out write verification failed"]
                    return original_read_summary_output_text(path, expected_stat)

                device_lab._read_summary_output_text = failing_read_summary_output_text  # type: ignore[attr-defined]
                errors = device_lab.write_summary(summary_path, {"ok": False})
                summary_text = summary_path.read_text(encoding="utf-8")
                temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))
        finally:
            device_lab._read_summary_output_text = original_read_summary_output_text  # type: ignore[attr-defined]

        self.assertEqual(errors, ["--json-out write verification failed"])
        self.assertEqual(summary_text, '{\n  "ok": false\n}\n')
        self.assertEqual(temp_files, [])

    def test_read_summary_output_rejects_oversized_readback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            with summary_path.open("wb") as handle:
                handle.seek(device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES)
                handle.write(b"x")
            expected_stat = summary_path.lstat()

            text, errors = device_lab._read_summary_output_text(  # type: ignore[attr-defined]
                summary_path,
                expected_stat,
            )

        self.assertIsNone(text)
        self.assertEqual(
            errors,
            [
                "--json-out must be no more than "
                f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
            ],
        )

    def test_write_summary_rejects_regular_file_swap_before_readback(self) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            summary_path = root / "summary.json"
            replacement = root / "replacement-summary.json"
            replacement.write_text('{"ok":true}\n', encoding="utf-8")
            swapped = False

            def swapping_open(path: Path, *args, **kwargs):
                nonlocal swapped
                mode = args[0] if args else kwargs.get("mode", "r")
                if path == summary_path and "r" in mode and not swapped:
                    replacement.replace(summary_path)
                    swapped = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", swapping_open):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            summary_text = summary_path.read_text(encoding="utf-8")

        self.assertEqual(errors, ["--json-out changed while being read"])
        self.assertEqual(summary_text, '{"ok":true}\n')

    def test_write_summary_rejects_symlink_swap_after_replace(self) -> None:
        original_validate = device_lab.validate_summary_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                summary_path = root / "summary.json"
                alias_target = root / "external-summary.json"
                alias_target.write_text("external\n", encoding="utf-8")
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == summary_path and calls == 3:
                        path.unlink(missing_ok=True)
                        try:
                            path.symlink_to(alias_target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                device_lab.validate_summary_output_path = validate_then_alias
                errors = device_lab.write_summary(summary_path, {"ok": False})
                target_text = alias_target.read_text(encoding="utf-8")
                output_is_symlink = summary_path.is_symlink()
                temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))
        finally:
            device_lab.validate_summary_output_path = original_validate

        self.assertEqual(errors, ["--json-out must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertTrue(output_is_symlink)
        self.assertEqual(temp_files, [])

    def test_write_summary_rechecks_parent_after_create_before_write(self) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            summary_path = root / "late-linked-summary" / "summary.json"
            alias_target = root / "external-summary"
            alias_target.mkdir()

            def replacing_mkdir(path: Path, *args, **kwargs):
                if path == summary_path.parent:
                    create_dir_symlink(self, path, alias_target)
                    return None
                return original_mkdir(path, *args, **kwargs)

            with mock.patch.object(path_type, "mkdir", replacing_mkdir):
                errors = device_lab.write_summary(summary_path, {"ok": False})

        self.assertEqual(errors, ["--json-out parent directory must not be a symlink"])
        self.assertFalse((alias_target / "summary.json").exists())

    def test_json_summary_rejects_symlinked_output_without_following_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            alias_target = Path(temp) / "external-summary.json"
            alias_target.write_text("do not overwrite\n", encoding="utf-8")
            try:
                summary_path.symlink_to(alias_target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            rendered = stdout.getvalue() + stderr.getvalue()

            self.assertEqual(status, 1)
            self.assertEqual(alias_target.read_text(encoding="utf-8"), "do not overwrite\n")
            self.assertIn("--json-out must not be a symlink", rendered)
            self.assertNotIn(str(alias_target), rendered)
            self.assertNotIn("[device-lab] wrote summary", rendered)

    def test_json_summary_rejects_hardlinked_output_without_overwriting_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            alias_target = Path(temp) / "external-summary.json"
            alias_target.write_text("do not overwrite\n", encoding="utf-8")
            summary_path.write_text("placeholder\n", encoding="utf-8")
            replace_with_hardlink(self, summary_path, alias_target)
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            rendered = stdout.getvalue() + stderr.getvalue()

            self.assertEqual(status, 1)
            self.assertEqual(alias_target.read_text(encoding="utf-8"), "do not overwrite\n")
            self.assertIn("--json-out must not be hardlinked", rendered)
            self.assertNotIn(str(alias_target), rendered)
            self.assertNotIn("[device-lab] wrote summary", rendered)



if __name__ == "__main__":  # pragma: no cover
    unittest.main()
