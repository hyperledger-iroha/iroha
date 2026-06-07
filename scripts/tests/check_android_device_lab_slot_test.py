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
                    "abi7_recursive_compact_jni_probe": "unavailable",
                    "abi7_recursive_compact_prover_state": "proof_composition_unavailable",
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
                "abi7_recursive_compact_jni_probe": "unavailable",
                "abi7_recursive_compact_prover_state": "proof_composition_unavailable",
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
            "abi7_recursive_compact_jni_probe": "unavailable",
            "abi7_recursive_compact_prover_state": "proof_composition_unavailable",
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

    def test_production_metadata_rejects_available_recursive_compact_probe(self) -> None:
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
            metadata["abi7_recursive_compact_jni_probe"] = "available"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json abi7_recursive_compact_jni_probe must be one of ['fail_closed', 'unavailable']",
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
