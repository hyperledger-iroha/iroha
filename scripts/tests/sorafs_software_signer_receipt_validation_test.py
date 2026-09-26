"""Exact JSON type checks for foundational software-signer receipt replay."""

from __future__ import annotations

import copy

import pytest

from sorafs_software_signer_receipt import (
    PROMOTION_SIGNER_DOMAIN,
    PROMOTION_SIGNER_ROLE,
    RECEIPT_VALIDATION_SCHEMA,
    canonical_json_bytes,
    parse_canonical_validation,
    validate_receipt_validation,
)


OPERATION_ID = "11" * 32
POLICY_DIGEST = "22" * 32
BINDING = {
    "operation_id_hex": OPERATION_ID,
    "payload_length": 64,
    "service_id": "sorafs-promotion-signer-a",
    "administrator_id": "sorafs-promotion-admin-b",
    "key_revision": 5,
    "policy_revision": 8,
    "policy_digest_sha256": POLICY_DIGEST,
}


def valid_receipt_validation() -> dict[str, object]:
    """Return one schema-complete canonical verifier result for the exact role."""

    return {
        "schema": RECEIPT_VALIDATION_SCHEMA,
        "status": "valid",
        "operation_id_hex": OPERATION_ID,
        "payload_digest_blake3_hex": "31" * 32,
        "payload_length": 64,
        "signature_digest_blake3_hex": "32" * 32,
        "binding_digest_blake3_hex": "33" * 32,
        "service_id": BINDING["service_id"],
        "administrator_id": BINDING["administrator_id"],
        "role": PROMOTION_SIGNER_ROLE,
        "domain": PROMOTION_SIGNER_DOMAIN,
        "signature_algorithm": "ed25519",
        "key_revision": 5,
        "policy_revision": 8,
        "policy_digest_sha256": POLICY_DIGEST,
        "public_key_digest_blake3_hex": "34" * 32,
        "commit_sequence": 1,
        "commit_audit_head_blake3_hex": "35" * 32,
        "audit_sequence": 1,
        "audit_head_blake3_hex": "35" * 32,
        "replayed": False,
        "revoked": False,
        "payload_signature_valid": True,
        "provenance_attestation_valid": True,
        "response_attestation_valid": True,
    }


def test_foundational_receipt_validation_accepts_exact_json_types() -> None:
    """The valid pinned-verifier projection remains accepted."""

    value = valid_receipt_validation()
    parsed, parse_errors = parse_canonical_validation(canonical_json_bytes(value))
    assert parse_errors == []
    assert parsed is not None
    assert validate_receipt_validation(parsed, **BINDING) == []


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("payload_length", 64.0),
        ("key_revision", 5.0),
        ("policy_revision", 8.0),
        ("revoked", 0),
        ("payload_signature_valid", 1),
        ("provenance_attestation_valid", 1),
        ("response_attestation_valid", 1),
    ],
)
def test_foundational_receipt_validation_rejects_numeric_type_aliases(
    field: str, replacement: object,
) -> None:
    """Canonical JSON values cannot impersonate other reviewed JSON types."""

    value = copy.deepcopy(valid_receipt_validation())
    value[field] = replacement
    parsed, parse_errors = parse_canonical_validation(canonical_json_bytes(value))
    assert parse_errors == []
    assert parsed is not None
    assert (
        f"software signer receipt validation {field} does not match the reviewed promotion binding"
        in validate_receipt_validation(parsed, **BINDING)
    )
