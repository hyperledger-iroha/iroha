"""Public receipt fixtures for foundational promotion-envelope tests."""

from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path
from typing import Any

import sorafs_software_signer_evidence as SIGNER_EVIDENCE
from sorafs_software_signer_receipt import canonical_json_bytes


OPERATION_ID = hashlib.sha256(b"test-only-foundational-receipt-operation").hexdigest()
BINDING_FILENAME = "test-foundational-promotion.binding.norito"
VERIFIER_FILENAME = "test-foundational-receipt-verifier"


def _settings(signature: dict[str, Any]) -> dict[str, Any]:
    return {
        "service_id": signature["service_id"],
        "administrator_id": signature["administrator_id"],
        "key_revision": signature["key_revision"],
        "policy_revision": signature["policy_revision"],
        "policy_digest_sha256": signature["policy_digest_sha256"],
    }


def _validation(
    payload: bytes,
    operation_id: str,
    settings: dict[str, Any],
) -> bytes:
    value = {
        "schema": "sorafs.external_software_signer.signature_receipt_validation.v1",
        "status": "valid",
        "operation_id_hex": operation_id,
        "payload_digest_blake3_hex": "11" * 32,
        "payload_length": len(payload),
        "signature_digest_blake3_hex": "22" * 32,
        "binding_digest_blake3_hex": "33" * 32,
        "backend": "software",
        **settings,
        "role": "promotion",
        "domain": "sorafs.production-readiness.foundational-prerequisites.v1",
        "signature_algorithm": "ed25519",
        "public_key_digest_blake3_hex": "44" * 32,
        "commit_sequence": 7,
        "commit_audit_head_blake3_hex": "55" * 32,
        "audit_sequence": 7,
        "audit_head_blake3_hex": "55" * 32,
        "replayed": False,
        "revoked": False,
        "payload_signature_valid": True,
        "provenance_attestation_valid": True,
        "response_attestation_valid": True,
    }
    return canonical_json_bytes(value)


def _receipt(
    binding: bytes,
    payload: bytes,
    signature: bytes,
    operation_id: str,
) -> bytes:
    return canonical_json_bytes(
        {
            "schema": "test.external_software_signer.signature_receipt.v1",
            "operation_id_hex": operation_id,
            "binding_sha256": hashlib.sha256(binding).hexdigest(),
            "payload_sha256": hashlib.sha256(payload).hexdigest(),
            "signature_sha256": hashlib.sha256(signature).hexdigest(),
        }
    )


def write_verifier(root: Path, signature: dict[str, Any]) -> Path:
    """Write one pinned offline verifier that checks all exact fixture bytes."""

    path = root / VERIFIER_FILENAME
    settings = _settings(signature)
    source = f'''#!/usr/bin/env python3
import argparse
import hashlib
import json
import sys
from pathlib import Path

SETTINGS = {settings!r}
parser = argparse.ArgumentParser()
if len(sys.argv) < 2 or sys.argv[1] != "verify-receipt":
    raise SystemExit(2)
for flag in ("binding", "payload", "signature", "receipt", "expected-operation-id", "validation-out"):
    parser.add_argument("--" + flag, required=True)
args = parser.parse_args(sys.argv[2:])
binding = Path(args.binding).read_bytes()
payload = Path(args.payload).read_bytes()
signature = Path(args.signature).read_bytes()
receipt_raw = Path(args.receipt).read_bytes()
receipt = json.loads(receipt_raw)
expected = {{
    "schema": "test.external_software_signer.signature_receipt.v1",
    "operation_id_hex": args.expected_operation_id,
    "binding_sha256": hashlib.sha256(binding).hexdigest(),
    "payload_sha256": hashlib.sha256(payload).hexdigest(),
    "signature_sha256": hashlib.sha256(signature).hexdigest(),
}}
if receipt != expected or receipt_raw != json.dumps(receipt, sort_keys=True, separators=(",", ":")).encode("ascii"):
    raise SystemExit(1)
validation = {{
    "schema": "sorafs.external_software_signer.signature_receipt_validation.v1",
    "status": "valid", "operation_id_hex": args.expected_operation_id,
    "payload_digest_blake3_hex": "11" * 32, "payload_length": len(payload),
    "signature_digest_blake3_hex": "22" * 32, "binding_digest_blake3_hex": "33" * 32,
    "backend": "software", **SETTINGS, "role": "promotion",
    "domain": "sorafs.production-readiness.foundational-prerequisites.v1",
    "signature_algorithm": "ed25519", "public_key_digest_blake3_hex": "44" * 32,
    "commit_sequence": 7, "commit_audit_head_blake3_hex": "55" * 32,
    "audit_sequence": 7, "audit_head_blake3_hex": "55" * 32,
    "replayed": False, "revoked": False, "payload_signature_valid": True,
    "provenance_attestation_valid": True, "response_attestation_valid": True,
}}
Path(args.validation_out).write_bytes(json.dumps(validation, sort_keys=True, separators=(",", ":")).encode("ascii"))
'''
    if not path.exists():
        path.write_text(source, encoding="utf-8")
        path.chmod(0o500)
    elif path.read_text(encoding="utf-8") != source:
        raise AssertionError("fixture verifier settings changed below one evidence root")
    return path


def refresh_bundle(payload: dict[str, Any]) -> None:
    """Refresh embedded exact-byte evidence after a fixture is re-signed."""

    bundle = payload["signer_receipt_bundle"]
    binding = base64.b64decode(bundle["binding_base64"], validate=True)
    settings = json.loads(binding)
    signing_payload = SIGNER_EVIDENCE.foundational_signing_payload(payload)
    signature = bytes.fromhex(payload["signature"]["signature_hex"])
    receipt = _receipt(binding, signing_payload, signature, bundle["operation_id_hex"])
    payload["signer_receipt_bundle"] = SIGNER_EVIDENCE.build_foundational_receipt_bundle(
        verifier_sha256=bundle["verifier_sha256"],
        operation_id_hex=bundle["operation_id_hex"],
        binding=binding,
        receipt=receipt,
        validation=_validation(signing_payload, bundle["operation_id_hex"], settings),
    )


def attach_bundle(payload: dict[str, Any], root: Path) -> Path:
    """Attach valid post-sign evidence and return its pinned verifier path."""

    verifier = write_verifier(root, payload["signature"])
    binding = canonical_json_bytes(_settings(payload["signature"]))
    signing_payload = SIGNER_EVIDENCE.foundational_signing_payload(payload)
    signature = bytes.fromhex(payload["signature"]["signature_hex"])
    receipt = _receipt(binding, signing_payload, signature, OPERATION_ID)
    payload["signer_receipt_bundle"] = SIGNER_EVIDENCE.build_foundational_receipt_bundle(
        verifier_sha256=hashlib.sha256(verifier.read_bytes()).hexdigest(),
        operation_id_hex=OPERATION_ID,
        binding=binding,
        receipt=receipt,
        validation=_validation(signing_payload, OPERATION_ID, _settings(payload["signature"])),
    )
    return verifier
