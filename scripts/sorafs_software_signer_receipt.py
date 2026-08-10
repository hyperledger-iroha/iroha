#!/usr/bin/env python3
"""Validate payload-free promotion receipt-verifier output."""

from __future__ import annotations

import json
import re
from typing import Any


PROMOTION_SIGNER_ROLE = "promotion"
PROMOTION_SIGNER_DOMAIN = (
    "sorafs.production-readiness.foundational-prerequisites.v1"
)
RECEIPT_VALIDATION_SCHEMA = (
    "sorafs.external_software_signer.signature_receipt_validation.v1"
)
RECEIPT_VALIDATION_FIELDS = frozenset(
    {
        "schema",
        "status",
        "operation_id_hex",
        "payload_digest_blake3_hex",
        "payload_length",
        "signature_digest_blake3_hex",
        "binding_digest_blake3_hex",
        "backend",
        "service_id",
        "administrator_id",
        "role",
        "domain",
        "signature_algorithm",
        "key_revision",
        "policy_revision",
        "policy_digest_sha256",
        "public_key_digest_blake3_hex",
        "commit_sequence",
        "commit_audit_head_blake3_hex",
        "audit_sequence",
        "audit_head_blake3_hex",
        "replayed",
        "revoked",
        "payload_signature_valid",
        "provenance_attestation_valid",
        "response_attestation_valid",
    }
)
LOWER_DIGEST_RE = re.compile(r"[0-9a-f]{64}")


def canonical_json_bytes(value: Any) -> bytes:
    """Return the exact compact JSON representation required from the verifier."""

    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")


def parse_canonical_validation(
    raw: bytes,
) -> tuple[dict[str, Any] | None, list[str]]:
    """Decode a schema-closed, duplicate-free canonical validation artifact."""

    errors: list[str] = []

    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        value: dict[str, Any] = {}
        for key, item in pairs:
            if key in value:
                raise ValueError("duplicate member")
            value[key] = item
        return value

    try:
        text = raw.decode("ascii")
        value = json.loads(
            text,
            object_pairs_hook=reject_duplicates,
            parse_constant=lambda _value: (_ for _ in ()).throw(
                ValueError("non-finite number")
            ),
        )
    except (RecursionError, UnicodeDecodeError, ValueError, json.JSONDecodeError):
        return None, ["software signer receipt validation must be strict ASCII JSON"]
    if not isinstance(value, dict):
        return None, ["software signer receipt validation must be a JSON object"]
    try:
        canonical = canonical_json_bytes(value)
    except (TypeError, ValueError):
        return None, ["software signer receipt validation contains invalid JSON values"]
    if raw != canonical:
        errors.append("software signer receipt validation must use canonical JSON")
    if set(value) != RECEIPT_VALIDATION_FIELDS:
        errors.append("software signer receipt validation fields do not match the contract")
    return value, errors


def _canonical_nonzero_digest(value: Any) -> bool:
    return (
        isinstance(value, str)
        and LOWER_DIGEST_RE.fullmatch(value) is not None
        and any(bytes.fromhex(value))
    )


def _positive_integer(value: Any) -> bool:
    return (
        isinstance(value, int)
        and not isinstance(value, bool)
        and 0 < value <= (1 << 63) - 1
    )


def validate_receipt_validation(
    value: dict[str, Any],
    *,
    operation_id_hex: str,
    payload_length: int,
    service_id: str,
    administrator_id: str,
    key_revision: int,
    policy_revision: int,
    policy_digest_sha256: str,
) -> list[str]:
    """Require the verifier result to match every reviewed promotion binding."""

    errors: list[str] = []
    exact = {
        "schema": RECEIPT_VALIDATION_SCHEMA,
        "status": "valid",
        "operation_id_hex": operation_id_hex,
        "payload_length": payload_length,
        "backend": "software",
        "service_id": service_id,
        "administrator_id": administrator_id,
        "role": PROMOTION_SIGNER_ROLE,
        "domain": PROMOTION_SIGNER_DOMAIN,
        "signature_algorithm": "ed25519",
        "key_revision": key_revision,
        "policy_revision": policy_revision,
        "policy_digest_sha256": policy_digest_sha256,
        "revoked": False,
        "payload_signature_valid": True,
        "provenance_attestation_valid": True,
        "response_attestation_valid": True,
    }
    for field, expected in exact.items():
        if value.get(field) != expected:
            errors.append(
                f"software signer receipt validation {field} does not match the reviewed promotion binding"
            )
    for field in (
        "payload_digest_blake3_hex",
        "signature_digest_blake3_hex",
        "binding_digest_blake3_hex",
        "public_key_digest_blake3_hex",
        "commit_audit_head_blake3_hex",
        "audit_head_blake3_hex",
    ):
        if not _canonical_nonzero_digest(value.get(field)):
            errors.append(
                f"software signer receipt validation {field} must be a non-zero lowercase digest"
            )
    for field in ("commit_sequence", "audit_sequence"):
        if not _positive_integer(value.get(field)):
            errors.append(
                f"software signer receipt validation {field} must be a positive bounded integer"
            )
    if not isinstance(value.get("replayed"), bool):
        errors.append("software signer receipt validation replayed must be boolean")
    if value.get("commit_sequence") != value.get("audit_sequence"):
        errors.append(
            "software signer receipt validation commit and audit sequences must match"
        )
    if value.get("commit_audit_head_blake3_hex") != value.get(
        "audit_head_blake3_hex"
    ):
        errors.append(
            "software signer receipt validation commit and audit heads must match"
        )
    return errors


__all__ = [
    "PROMOTION_SIGNER_DOMAIN",
    "PROMOTION_SIGNER_ROLE",
    "RECEIPT_VALIDATION_FIELDS",
    "RECEIPT_VALIDATION_SCHEMA",
    "canonical_json_bytes",
    "parse_canonical_validation",
    "validate_receipt_validation",
]
