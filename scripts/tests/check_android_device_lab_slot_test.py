"""Tests for scripts/check_android_device_lab_slot.py."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
import subprocess
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
import tempfile
import unittest
from unittest import mock


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_android_device_lab_slot.py"
SPEC = importlib.util.spec_from_file_location("check_android_device_lab_slot", MODULE_PATH)
assert SPEC and SPEC.loader  # pragma: no cover - import guard
device_lab = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(device_lab)  # type: ignore[misc]

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

KAGEMUSHA_ANDROID_RAW_TEST_COMMAND = (
    device_lab.KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMAND
)


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


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
    for relative in device_lab._required_signed_evidence_digest_paths(slot):  # type: ignore[attr-defined]
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
    device_fingerprint: str,
    os_build_id: str,
    app_package_name: str,
    app_signing_certificate_sha256: str,
    attestation_challenge_sha256: str,
    offline_wallet_policy_sha256: str,
    offline_wallet_apk_sha256: str,
) -> tuple[str, str]:
    transcript_path = "handoff/d2d-payment.json"
    queue_after_sha256 = hashlib.sha256(
        (slot / "queue" / "pending_queue.json").read_bytes()
    ).hexdigest()
    queue_before_sha256 = hashlib.sha256(
        f"{name}:queue-before-d2d-payment".encode("utf-8")
    ).hexdigest()
    payload_sha256 = hashlib.sha256(
        f"{name}:reserved-lineage-d2d-payload".encode("utf-8")
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
            "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
            "offline_wallet_apk_sha256": offline_wallet_apk_sha256,
            "transport": "nfc_hce",
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
    offline_wallet_policy_sha256: str,
    offline_wallet_apk_sha256: str,
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
            "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
            "offline_wallet_apk_sha256": offline_wallet_apk_sha256,
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


def create_slot(
    root: Path,
    name: str,
    family: str | None = None,
    signer: dict[str, Path | str] | None = None,
) -> Path:
    slot = root / name
    device_fingerprint = f"{name}/fingerprint"
    os_build_id = f"{name}-build"
    app_package_name = "org.hyperledger.iroha.android.offlinewallet"
    app_signing_certificate_sha256 = hashlib.sha256(
        f"{name}:app-signing-certificate".encode("utf-8")
    ).hexdigest()
    attestation_challenge_sha256 = hashlib.sha256(
        f"{name}:attestation-challenge".encode("utf-8")
    ).hexdigest()
    attestation_certificate_chain_path = "attestation/keymint-certificate-chain.pem"
    write_text(
        slot / attestation_certificate_chain_path,
        "-----BEGIN CERTIFICATE-----\n"
        f"{name}-strongbox-keymint-certificate-chain\n"
        "-----END CERTIFICATE-----\n",
    )
    attestation_certificate_chain_sha256 = hashlib.sha256(
        (slot / attestation_certificate_chain_path).read_bytes()
    ).hexdigest()
    offline_wallet_policy_sha256 = hashlib.sha256(
        b"kagemusha-offline-wallet-policy-v1"
    ).hexdigest()
    offline_wallet_apk_path = "evidence/offline-wallet-release.apk"
    write_text(
        slot / offline_wallet_apk_path,
        f"{name}:offline-wallet-release-apk\n",
    )
    offline_wallet_apk_sha256 = hashlib.sha256(
        (slot / offline_wallet_apk_path).read_bytes()
    ).hexdigest()
    raw_test_commands = [KAGEMUSHA_ANDROID_RAW_TEST_COMMAND]
    write_json(
        slot / "telemetry" / "telemetry.json",
        {"schema_version": 1, "slot_id": name, "suite": "kagemusha-device-lab"},
    )
    write_text(slot / "telemetry" / "status.ndjson", '{"status":"ok"}\n')
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
            "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
            "attestation_security_level": "STRONGBOX",
            "keymaster_security_level": "STRONGBOX",
            "keymint_security_level": "STRONGBOX",
            "strongbox_attestation": True,
            "physical_device_attestation": True,
        },
    )
    write_json(slot / "attestation" / "report.json", {"verification": {"status": "ok"}})
    write_json(slot / "queue" / "pending_queue.json", {"slot_id": name, "pending_transactions": []})
    write_text(slot / "queue" / "pending.queue", "")
    write_text(slot / "logs" / "runtime.log", "kagemusha device-lab run complete\n")
    d2d_payment_transcript_path, d2d_payment_transcript_sha256 = (
        write_d2d_payment_transcript(
            slot,
            name,
            family or device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
            device_fingerprint=device_fingerprint,
            os_build_id=os_build_id,
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=attestation_challenge_sha256,
            offline_wallet_policy_sha256=offline_wallet_policy_sha256,
            offline_wallet_apk_sha256=offline_wallet_apk_sha256,
        )
    )
    wallet_integrity_transcript_path, wallet_integrity_transcript_sha256 = (
        write_wallet_integrity_transcript(
            slot,
            name,
            family or device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
            device_fingerprint=device_fingerprint,
            os_build_id=os_build_id,
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=attestation_challenge_sha256,
            attestation_certificate_chain_sha256=attestation_certificate_chain_sha256,
            offline_wallet_policy_sha256=offline_wallet_policy_sha256,
            offline_wallet_apk_sha256=offline_wallet_apk_sha256,
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
                    "device_fingerprint": device_fingerprint,
                    "os_build_id": os_build_id,
                    "minimum_os": minimum_os,
                    "app_package_name": app_package_name,
                    "attestation_certificate_chain_path": attestation_certificate_chain_path,
                    "offline_wallet_apk_path": offline_wallet_apk_path,
                    "d2d_payment_transcript_path": d2d_payment_transcript_path,
                    "wallet_integrity_transcript_path": wallet_integrity_transcript_path,
                    "app_signing_certificate_sha256": app_signing_certificate_sha256,
                    "attestation_challenge_sha256": attestation_challenge_sha256,
                    "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
                    "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
                    "offline_wallet_apk_sha256": offline_wallet_apk_sha256,
                    "d2d_payment_transcript_sha256": d2d_payment_transcript_sha256,
                    "wallet_integrity_transcript_sha256": wallet_integrity_transcript_sha256,
                    "native_bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
                    "strongbox_attestation": True,
                    "physical_device_attestation": True,
                    "keymint_security_level": "STRONGBOX",
                    "one_use_key_rotation_passed": True,
                    "rollback_rejection_passed": True,
                    "abi6_recursive_spend_jni_probe": "passed",
                    "abi7_recursive_compact_jni_probe": "one_hop_verified",
                    "abi7_recursive_compact_prover_state": "multi_hop_proof_composed",
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
                "schema": "iroha.android.device_lab.kagemusha.v1",
                "slot_id": name,
                "device_family": family,
                "device_fingerprint": device_fingerprint,
                "os_build_id": os_build_id,
                "minimum_os": minimum_os,
                "app_package_name": app_package_name,
                "attestation_certificate_chain_path": attestation_certificate_chain_path,
                "offline_wallet_apk_path": offline_wallet_apk_path,
                "d2d_payment_transcript_path": d2d_payment_transcript_path,
                "wallet_integrity_transcript_path": wallet_integrity_transcript_path,
                "app_signing_certificate_sha256": app_signing_certificate_sha256,
                "attestation_challenge_sha256": attestation_challenge_sha256,
                "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
                "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
                "offline_wallet_apk_sha256": offline_wallet_apk_sha256,
                "d2d_payment_transcript_sha256": d2d_payment_transcript_sha256,
                "wallet_integrity_transcript_sha256": wallet_integrity_transcript_sha256,
                "native_bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
                "strongbox_attestation": True,
                "physical_device_attestation": True,
                "keymint_security_level": "STRONGBOX",
                "one_use_key_rotation_passed": True,
                "rollback_rejection_passed": True,
                "abi6_recursive_spend_jni_probe": "passed",
                "abi7_recursive_compact_jni_probe": "one_hop_verified",
                "abi7_recursive_compact_prover_state": "multi_hop_proof_composed",
                "signed_evidence_artifact_path": "evidence/signed-evidence.json",
                "signed_evidence_artifact_sha256": evidence_digest,
                "raw_test_commands": raw_test_commands,
            },
        )

    rewrite_sha256sum(slot)
    return slot


def write_unsigned_production_slot_metadata(slot: Path, name: str, family: str) -> None:
    raw_test_commands = [KAGEMUSHA_ANDROID_RAW_TEST_COMMAND]
    offline_wallet_apk_path = "evidence/offline-wallet-release.apk"
    app_package_name = "org.hyperledger.iroha.android.offlinewallet"
    app_signing_certificate_sha256 = hashlib.sha256(
        f"{name}:app-signing-certificate".encode("utf-8")
    ).hexdigest()
    offline_wallet_policy_sha256 = hashlib.sha256(
        b"kagemusha-offline-wallet-policy-v1"
    ).hexdigest()
    attestation_certificate_chain_path = "attestation/keymint-certificate-chain.pem"
    attestation_certificate_chain_sha256 = hashlib.sha256(
        (slot / attestation_certificate_chain_path).read_bytes()
    ).hexdigest()
    offline_wallet_apk_sha256 = hashlib.sha256(
        (slot / offline_wallet_apk_path).read_bytes()
    ).hexdigest()
    d2d_payment_transcript_path, d2d_payment_transcript_sha256 = (
        write_d2d_payment_transcript(
            slot,
            name,
            family,
            device_fingerprint=f"{name}/fingerprint",
            os_build_id=f"{name}-build",
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=hashlib.sha256(
                f"{name}:attestation-challenge".encode("utf-8")
            ).hexdigest(),
            offline_wallet_policy_sha256=offline_wallet_policy_sha256,
            offline_wallet_apk_sha256=offline_wallet_apk_sha256,
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
            attestation_challenge_sha256=hashlib.sha256(
                f"{name}:attestation-challenge".encode("utf-8")
            ).hexdigest(),
            attestation_certificate_chain_sha256=attestation_certificate_chain_sha256,
            offline_wallet_policy_sha256=offline_wallet_policy_sha256,
            offline_wallet_apk_sha256=offline_wallet_apk_sha256,
        )
    )
    write_json(
        slot / "slot.json",
        {
            "schema": "iroha.android.device_lab.kagemusha.v1",
            "slot_id": name,
            "device_family": family,
            "device_fingerprint": f"{name}/fingerprint",
            "os_build_id": f"{name}-build",
            "minimum_os": device_lab.KAGEMUSHA_STANDARD_DEVICE_MINIMUM_OS[family],
            "app_package_name": app_package_name,
            "attestation_certificate_chain_path": attestation_certificate_chain_path,
            "offline_wallet_apk_path": offline_wallet_apk_path,
            "d2d_payment_transcript_path": d2d_payment_transcript_path,
            "wallet_integrity_transcript_path": wallet_integrity_transcript_path,
            "app_signing_certificate_sha256": app_signing_certificate_sha256,
            "attestation_challenge_sha256": hashlib.sha256(
                f"{name}:attestation-challenge".encode("utf-8")
            ).hexdigest(),
            "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
            "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
            "offline_wallet_apk_sha256": offline_wallet_apk_sha256,
            "d2d_payment_transcript_sha256": d2d_payment_transcript_sha256,
            "wallet_integrity_transcript_sha256": wallet_integrity_transcript_sha256,
            "native_bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
            "strongbox_attestation": True,
            "physical_device_attestation": True,
            "keymint_security_level": "STRONGBOX",
            "one_use_key_rotation_passed": True,
            "rollback_rejection_passed": True,
            "abi6_recursive_spend_jni_probe": "passed",
            "abi7_recursive_compact_jni_probe": "one_hop_verified",
            "abi7_recursive_compact_prover_state": "multi_hop_proof_composed",
            "signed_evidence_artifact_path": "evidence/signed-evidence.json",
            "signed_evidence_artifact_sha256": "0" * 64,
            "raw_test_commands": raw_test_commands,
        },
    )


class AndroidDeviceLabSlotTest(unittest.TestCase):
    def test_checked_in_sample_slot_passes_default_validation(self) -> None:
        root = Path(__file__).resolve().parents[2] / "fixtures" / "android" / "device_lab"
        report = device_lab.scan_slot(root / "slot-sample")
        self.assertEqual(report["status"], "ok", report["errors"])

    def test_scan_slot_rejects_sha256_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            write_text(slot / "logs" / "runtime.log", "tampered after manifest\n")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertIn("sha256sum.txt digest mismatch for logs/runtime.log", report["errors"])

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
        original_rglob = Path.rglob

        def failing_rglob(path: Path, pattern: str):
            if path.name == "logs":
                raise OSError("simulated directory traversal failure")
            return original_rglob(path, pattern)

        try:
            Path.rglob = failing_rglob
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")

                report = device_lab.scan_slot(slot)
        finally:
            Path.rglob = original_rglob

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

    def test_scan_slot_redacts_secret_looking_manifest_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            secret_path = "logs/token=supersecret.log"
            write_text(slot / secret_path, "must not leak\n")
            write_text(
                slot / "sha256sum.txt",
                f"{'00' * 32}  {secret_path}\n",
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
                    if path == manifest_path:
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

            relative, digest = device_lab.validate_d2d_payment_transcript_binding(
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

            relative, digest = device_lab.validate_d2d_payment_transcript_binding(
                slot,
                metadata,
                errors,
            )

        self.assertEqual(relative, "handoff/d2d-payment.json")
        self.assertIsNone(digest)
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

    def test_production_metadata_rejects_unavailable_recursive_compact_one_hop_probe(
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
            metadata["abi7_recursive_compact_jni_probe"] = "unavailable"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json abi7_recursive_compact_jni_probe must be one of ['one_hop_verified']",
            report["errors"],
        )

    def test_production_metadata_rejects_generic_recursive_compact_prover_state(
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
            metadata["abi7_recursive_compact_prover_state"] = (
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
            "slot.json abi7_recursive_compact_prover_state must be one of ['multi_hop_proof_composed']",
            report["errors"],
        )

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
            metadata["signed_evidence_artifact_sha256"] = "00" * 32
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
                "slot.json offline_wallet_apk_path",
                "slot.json offline_wallet_apk_path must point to an existing file",
            )
            rendered = "\n".join(errors)

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["slot.json offline_wallet_apk_path must not contain secret-looking material"],
        )
        self.assertNotIn(secret_relative, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_metadata_artifact_digest_rejects_file_metadata_failure(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                target = slot / "evidence" / "offline-wallet-release.apk"

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == target:
                        raise OSError("simulated metadata artifact metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/offline-wallet-release.apk",
                    "slot.json offline_wallet_apk_path",
                    "slot.json offline_wallet_apk_path must point to an existing file",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "slot.json offline_wallet_apk_path references artifact file metadata "
                "could not be read evidence/offline-wallet-release.apk"
            ],
        )

    def test_metadata_artifact_digest_rejects_read_failure_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            target = slot / "evidence" / "offline-wallet-release.apk"

            payload, digest, errors = with_open_failure(
                target,
                lambda: device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/offline-wallet-release.apk",
                    "slot.json offline_wallet_apk_path",
                    "slot.json offline_wallet_apk_path must point to an existing file",
                ),
            )

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot.json offline_wallet_apk_path could not be read"])

    def test_metadata_artifact_digest_rejects_symlink_swap_after_preflight(
        self,
    ) -> None:
        original_validate = device_lab._validate_metadata_artifact_for_read

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                slot = create_slot(root, "slot-a")
                artifact_path = slot / "evidence" / "offline-wallet-release.apk"
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
                    "evidence/offline-wallet-release.apk",
                    "slot.json offline_wallet_apk_path",
                    "slot.json offline_wallet_apk_path must point to an existing file",
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
                "slot.json offline_wallet_apk_path references symlink artifact "
                "evidence/offline-wallet-release.apk"
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
                artifact_path = slot / "evidence" / "offline-wallet-release.apk"
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
                    "evidence/offline-wallet-release.apk",
                    "slot.json offline_wallet_apk_path",
                    "slot.json offline_wallet_apk_path must point to an existing file",
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
                "slot.json offline_wallet_apk_path references artifact changed "
                "while being read evidence/offline-wallet-release.apk"
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
                slot / "evidence" / "offline-wallet-release.apk",
                target,
            )

            errors, _details = device_lab.validate_kagemusha_production_metadata(
                slot,
                trusted,
            )

        self.assertIn(
            "slot.json offline_wallet_apk_path references hardlinked artifact "
            "evidence/offline-wallet-release.apk",
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
            attestation["offline_wallet_policy_sha256"] = "AA" * 32
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json offline_wallet_policy_sha256 must be lowercase sha256 hex",
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
            metadata.pop("offline_wallet_apk_sha256")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json offline_wallet_apk_sha256 must be lowercase sha256 hex",
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
            write_text(slot / "evidence" / "offline-wallet-release.apk", "tampered apk\n")
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json offline_wallet_apk_sha256 does not match offline_wallet_apk_path",
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

    def test_production_metadata_rejects_missing_d2d_handoff_raw_command_marker(self) -> None:
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
                command.replace(",org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest", "")
                for command in [KAGEMUSHA_ANDROID_RAW_TEST_COMMAND]
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
            "slot.json raw_test_commands must include OfflineNoteTransferHandoff",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include OfflineNoteTransferHandoff",
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
                lambda evidence: evidence.__setitem__("offline_wallet_apk_sha256", "11" * 32),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact offline_wallet_apk_sha256 must match slot.json offline_wallet_apk_sha256",
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
            metadata["abi6_recursive_spend_jni_probe"] = "ok"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact abi6_recursive_spend_jni_probe must match slot.json abi6_recursive_spend_jni_probe",
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
            metadata["raw_test_commands"] = ["./gradlew connectedAndroidTest --rerun"]
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
        self.assertIn(
            "slot.json raw_test_commands must include connectedAndroidTest",
            report["errors"],
        )
        self.assertIn(
            "slot.json raw_test_commands must include :client-android:assembleRelease",
            report["errors"],
        )
        self.assertIn(
            "slot.json raw_test_commands must include :offline-wallet-android:assembleRelease",
            report["errors"],
        )
        self.assertIn(
            "slot.json raw_test_commands must include KagemushaRecursiveSpendProverTest",
            report["errors"],
        )
        self.assertIn(
            "slot.json raw_test_commands must include OfflineNoteTransferHandoff",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include connectedAndroidTest",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include :client-android:assembleRelease",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include :offline-wallet-android:assembleRelease",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include KagemushaRecursiveSpendProverTest",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include OfflineNoteTransferHandoff",
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
                (
                    "echo :client-android:assembleRelease "
                    ":offline-wallet-android:assembleRelease connectedAndroidTest "
                    "KagemushaRecursiveSpendProverTest OfflineNoteTransferHandoff"
                )
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
        for timestamp in (
            "2026-06-06T00:00:00+00:00",
            " 2026-06-06T00:00:00Z ",
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
                self.assertIn(
                    "signed evidence artifact signed_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                    report["errors"],
                )

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
                evidence["artifact_digests"]["logs/runtime.log"] = "00" * 32

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

            def validate_then_alias(slot_path: Path, errors: list[str]) -> None:
                original_validate(slot_path, errors)
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
            apk_path = metadata["offline_wallet_apk_path"]

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
        self.assertIn(
            "required slot artifact logs/runtime.log must be no more than 8 bytes",
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
                lambda evidence: evidence.__setitem__("signature_payload_sha256", "00" * 32),
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
        original_which = device_lab.shutil.which
        try:
            device_lab.shutil.which = lambda _command: None
            with tempfile.TemporaryDirectory() as temp:
                secret_public_key = Path(temp) / "token=supersecret-public.pem"

                trusted, errors = device_lab.load_trusted_signer_public_keys(
                    [secret_public_key]
                )
                rendered = "\n".join(errors)
        finally:
            device_lab.shutil.which = original_which

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must not contain secret-looking material"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(secret_public_key), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_verify_signature_rejects_secret_public_key_path_before_openssl_lookup(
        self,
    ) -> None:
        original_which = device_lab.shutil.which
        try:
            device_lab.shutil.which = lambda _command: None
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
        finally:
            device_lab.shutil.which = original_which

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

    def test_openssl_public_key_der_rejects_invalid_public_key_after_openssl_failure(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run

        def failing_run(*args, **kwargs):
            raise subprocess.CalledProcessError(1, args[0])

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
        original_read_bytes = Path.read_bytes

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging readback drift")

        def drifting_payload_read(path: Path) -> bytes:
            if path.name == "payload.bin":
                return b"mutated payload"
            return original_read_bytes(path)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = unexpected_run
            Path.read_bytes = drifting_payload_read
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
            Path.read_bytes = original_read_bytes

        self.assertEqual(
            errors,
            ["signature verification staged payload did not match input"],
        )

    def test_verify_signature_rejects_signature_staging_readback_mismatch_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run
        original_read_bytes = Path.read_bytes

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging readback drift")

        def drifting_signature_read(path: Path) -> bytes:
            if path.name == "signature.bin":
                return b"mutated signature"
            return original_read_bytes(path)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = unexpected_run
            Path.read_bytes = drifting_signature_read
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
            Path.read_bytes = original_read_bytes

        self.assertEqual(
            errors,
            ["signature verification staged signature did not match input"],
        )

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

    def test_private_public_pair_preserves_public_key_path_error_before_mismatch(
        self,
    ) -> None:
        original_which = device_lab.shutil.which
        try:
            device_lab.shutil.which = lambda _command: None
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
            device_lab.shutil.which = original_which

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
                    if path == public_key:
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
                    if path == output:
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

        self.assertEqual(errors, ["signed evidence output path could not be written"])
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
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return "mismatched signed evidence\n", []
                    return original_read_output_text(path, expected_stat, label)

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
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return None, [f"{label} write verification failed"]
                    return original_read_output_text(path, _expected_stat, label)

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

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

                def failing_mkdir(path: Path, *args, **kwargs):
                    if path == output.parent:
                        raise OSError("simulated signed evidence parent mkdir failure")
                    return original_mkdir(path, *args, **kwargs)

                path_type.mkdir = failing_mkdir

                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
        finally:
            path_type.mkdir = original_mkdir

        self.assertEqual(
            errors,
            ["signed evidence output path parent directory could not be created"],
        )
        self.assertFalse(output.exists())

    def test_signer_write_json_rechecks_parent_after_create_before_write(self) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        try:
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

                path_type.mkdir = replacing_mkdir

                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
        finally:
            path_type.mkdir = original_mkdir

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
                    if path == output:
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

        self.assertEqual(errors, ["sha256sum.txt could not be written"])
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
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return "mismatched manifest\n", []
                    return original_read_output_text(path, expected_stat, label)

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

    def test_signer_write_text_rejects_readback_failure(self) -> None:
        original_read_output_text = evidence_signer._read_existing_output_text

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "sha256sum.txt"

                def failing_read_output_text(
                    path: Path,
                    _expected_stat: os.stat_result,
                    label: str,
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return None, [f"{label} write verification failed"]
                    return original_read_output_text(path, _expected_stat, label)

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

    def test_signer_slot_artifact_digest_rejects_hardlink_metadata_failure_after_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            target = slot / "logs" / "runtime.log"

            def failing_stat(path: Path, *args, **kwargs):
                if path == target:
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

    def test_signer_artifact_digests_include_release_apk_and_attestation_chain(
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
        self.assertIn(metadata["offline_wallet_apk_path"], digests)
        self.assertIn(metadata["attestation_certificate_chain_path"], digests)

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
        self.assertIn(
            "slot.json raw_test_commands must include connectedAndroidTest",
            stderr.getvalue(),
        )
        self.assertIn(
            "slot.json raw_test_commands must include :client-android:assembleRelease",
            stderr.getvalue(),
        )
        self.assertIn(
            "slot.json raw_test_commands must include :offline-wallet-android:assembleRelease",
            stderr.getvalue(),
        )
        self.assertIn(
            "slot.json raw_test_commands must include KagemushaRecursiveSpendProverTest",
            stderr.getvalue(),
        )
        self.assertIn(
            "slot.json raw_test_commands must include OfflineNoteTransferHandoff",
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
                (
                    "echo :client-android:assembleRelease "
                    ":offline-wallet-android:assembleRelease connectedAndroidTest "
                    "KagemushaRecursiveSpendProverTest OfflineNoteTransferHandoff"
                )
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
            attestation["offline_wallet_policy_sha256"] = "33" * 32
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
            "attestation/result.json offline_wallet_policy_sha256 must match slot.json offline_wallet_policy_sha256",
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
        original_which = device_lab.shutil.which
        try:
            device_lab.shutil.which = lambda _command: None
            with tempfile.TemporaryDirectory() as temp:
                secret_private_key = Path(temp) / "private_key=supersecret.pem"
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    secret_private_key,
                    b"payload",
                    errors,
                )
                rendered = "\n".join(errors)
        finally:
            device_lab.shutil.which = original_which

        self.assertIsNone(signature)
        self.assertEqual(
            errors,
            ["private key path must not contain secret-looking material"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(secret_private_key), rendered)
        self.assertNotIn("private_key=supersecret", rendered)

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
                    if path == private_key:
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
        original_read_bytes = Path.read_bytes

        def fake_run(*args, **kwargs):
            return subprocess.CompletedProcess(args=args, returncode=0)

        def failing_signature_read(path: Path) -> bytes:
            if path.name == "signature.bin":
                raise OSError("simulated signature read failure")
            return original_read_bytes(path)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            Path.read_bytes = failing_signature_read
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
            Path.read_bytes = original_read_bytes

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature output could not be read"])

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
        original_read_bytes = Path.read_bytes

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging readback drift")

        def drifting_payload_read(path: Path) -> bytes:
            if path.name == "payload.bin":
                return b"mutated payload"
            return original_read_bytes(path)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = unexpected_run
            Path.read_bytes = drifting_payload_read
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
            Path.read_bytes = original_read_bytes

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature payload staging verification failed"])

    def test_standard_matrix_accepts_all_kagemusha_device_families(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
                create_slot(root, f"slot-{index}", family, signer)
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

    def test_json_summary_reports_kagemusha_matrix_and_signer_pins(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
                create_slot(root, f"slot-{index}", family, signer)
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
            summary["kagemusha"]["trusted_signer_public_key_sha256"],
            [signer["public_key_sha256"]],
        )
        first_slot = summary["slots"][0]["kagemusha"]
        self.assertEqual(
            first_slot["offline_wallet_apk_path"],
            "evidence/offline-wallet-release.apk",
        )
        self.assertRegex(first_slot["offline_wallet_apk_sha256"], r"^[0-9a-f]{64}$")
        self.assertEqual(
            first_slot["native_bridge_abi_version"],
            device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
        )

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

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "missing-parent" / "summary.json"

                def failing_mkdir(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated summary parent mkdir failure")
                    return original_mkdir(path, *args, **kwargs)

                path_type.mkdir = failing_mkdir

                errors = device_lab.write_summary(summary_path, {"ok": False})
        finally:
            path_type.mkdir = original_mkdir

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
                    if path == summary_path:
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

        self.assertEqual(errors, ["--json-out could not be written"])
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
        original_read_text = Path.read_text

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"

                def mismatching_read_text(path: Path, *args, **kwargs) -> str:
                    if path == summary_path:
                        return '{"ok": true}\n'
                    return original_read_text(path, *args, **kwargs)

                Path.read_text = mismatching_read_text
                errors = device_lab.write_summary(summary_path, {"ok": False})
                summary_text = original_read_text(summary_path, encoding="utf-8")
        finally:
            Path.read_text = original_read_text

        self.assertEqual(errors, ["--json-out write verification failed"])
        self.assertEqual(summary_text, '{\n  "ok": false\n}\n')

    def test_write_summary_rejects_readback_failure(self) -> None:
        original_read_text = Path.read_text

        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"

            def failing_read_text(path: Path, *args, **kwargs) -> str:
                if path == summary_path:
                    raise OSError("simulated summary readback failure")
                return original_read_text(path, *args, **kwargs)

            with mock.patch.object(Path, "read_text", failing_read_text):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            summary_text = original_read_text(summary_path, encoding="utf-8")
            temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))

        self.assertEqual(errors, ["--json-out write verification failed"])
        self.assertEqual(summary_text, '{\n  "ok": false\n}\n')
        self.assertEqual(temp_files, [])

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

        try:
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

                path_type.mkdir = replacing_mkdir

                errors = device_lab.write_summary(summary_path, {"ok": False})
        finally:
            path_type.mkdir = original_mkdir

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
