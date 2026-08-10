#!/usr/bin/env python3
"""Schema-closed public provenance checks for external software signers."""

from __future__ import annotations

import json
import re
from typing import Any

from sorafs_production_readiness_contract import (
    FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN,
)


MAX_SIGNER_ID_BYTES = 128
MAX_SIGNER_REVISION = (1 << 63) - 1
LOWER_SHA256_RE = re.compile(r"[0-9a-f]{64}")


def _canonical_identifier(value: Any) -> str | None:
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        return None
    try:
        encoded = value.encode("utf-8")
    except UnicodeEncodeError:
        return None
    forbidden = {"dev", "fixture", "local", "mock", "sample", "test"}
    tokens = set(re.split(r"[^a-z0-9]+", value.lower()))
    return value if len(encoded) <= MAX_SIGNER_ID_BYTES and not tokens & forbidden else None


def _positive_revision(value: Any) -> int | None:
    return (
        value
        if isinstance(value, int)
        and not isinstance(value, bool)
        and 0 < value <= MAX_SIGNER_REVISION
        else None
    )


def parse_foundational_signer_public_key(
    value: Any,
    errors: list[str],
    *,
    path: str,
) -> bytes | None:
    """Decode one exact, non-zero Ed25519 public key without echoing it."""

    if not isinstance(value, str) or LOWER_SHA256_RE.fullmatch(value) is None:
        errors.append(f"{path} must be exactly 32 bytes of lowercase hex")
        return None
    public_key = bytes.fromhex(value)
    if not any(public_key):
        errors.append(f"{path} must not be the all-zero key")
        return None
    return public_key


def foundational_signing_payload(payload: dict[str, Any]) -> bytes:
    """Return the canonical, domain-separated prerequisite signature payload."""

    unsigned = dict(payload)
    signature = unsigned.get("signature")
    if isinstance(signature, dict):
        unsigned_signature = dict(signature)
        unsigned_signature.pop("signature_hex", None)
        unsigned["signature"] = unsigned_signature
    body = json.dumps(
        unsigned,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")
    return FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN + body


def validate_foundational_software_signer(
    signature: dict[str, Any] | None,
    errors: list[str],
) -> dict[str, Any]:
    """Validate and return payload-free promotion signer provenance."""

    result = {
        "signer_backend": None,
        "signer_service_id": None,
        "signer_administrator_id": None,
        "signer_key_revision": None,
        "signer_policy_revision": None,
        "signer_policy_digest_sha256": None,
    }
    if signature is None:
        return result
    if signature.get("backend") != "software":
        errors.append("foundational prerequisite signer backend must be `software`")
    else:
        result["signer_backend"] = "software"
    for field, output in (
        ("service_id", "signer_service_id"),
        ("administrator_id", "signer_administrator_id"),
    ):
        value = _canonical_identifier(signature.get(field))
        if value is None:
            errors.append(
                f"foundational prerequisite signer {field} must be canonical and at most {MAX_SIGNER_ID_BYTES} UTF-8 bytes"
            )
        else:
            result[output] = value
    if result["signer_service_id"] is not None and (
        result["signer_service_id"] == result["signer_administrator_id"]
    ):
        errors.append(
            "foundational prerequisite signer service_id and administrator_id must differ"
        )
    for field, output in (
        ("key_revision", "signer_key_revision"),
        ("policy_revision", "signer_policy_revision"),
    ):
        value = _positive_revision(signature.get(field))
        if value is None:
            errors.append(
                f"foundational prerequisite signer {field} must be in 1..2^63-1"
            )
        else:
            result[output] = value
    digest = signature.get("policy_digest_sha256")
    if (
        not isinstance(digest, str)
        or LOWER_SHA256_RE.fullmatch(digest) is None
        or not any(bytes.fromhex(digest))
    ):
        errors.append(
            "foundational prerequisite signer policy_digest_sha256 must be non-zero canonical lowercase SHA-256"
        )
    else:
        result["signer_policy_digest_sha256"] = digest
    return result


def validate_aggregate_software_signer(
    row: dict[str, Any],
    errors: list[str],
) -> None:
    """Revalidate the payload-free signer provenance copied to an aggregate."""

    signature = {
        "backend": row.get("signer_backend"),
        "service_id": row.get("signer_service_id"),
        "administrator_id": row.get("signer_administrator_id"),
        "key_revision": row.get("signer_key_revision"),
        "policy_revision": row.get("signer_policy_revision"),
        "policy_digest_sha256": row.get("signer_policy_digest_sha256"),
    }
    validate_foundational_software_signer(signature, errors)


__all__ = [
    "foundational_signing_payload",
    "parse_foundational_signer_public_key",
    "validate_aggregate_software_signer",
    "validate_foundational_software_signer",
]
