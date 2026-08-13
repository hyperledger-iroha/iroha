#!/usr/bin/env python3
"""Schema-closed public provenance checks for external software signers."""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import re
from pathlib import Path
from typing import Any

from sorafs_evidence_json import read_evidence_bytes
from sorafs_production_readiness_contract import (
    FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN,
    FOUNDATIONAL_SIGNER_RECEIPT_BUNDLE_FIELDS,
    FOUNDATIONAL_SIGNER_RECEIPT_BUNDLE_SCHEMA,
)
from sorafs_software_signer_receipt import (
    parse_canonical_validation,
    run_offline_receipt_verifier,
    validate_receipt_validation,
)


MAX_SIGNER_ID_BYTES = 128
MAX_SIGNER_REVISION = (1 << 63) - 1
LOWER_SHA256_RE = re.compile(r"[0-9a-f]{64}")
LOWER_SIGNATURE_RE = re.compile(r"[0-9a-f]{128}")
MAX_EXTERNAL_SIGNER_VERIFIER_BYTES = 128 * 1024 * 1024
MAX_EXTERNAL_SIGNER_BINDING_BYTES = 64 * 1024
MAX_EXTERNAL_SIGNER_RECEIPT_BYTES = 64 * 1024


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
    unsigned.pop("signer_receipt_bundle", None)
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


def build_foundational_receipt_bundle(
    *,
    verifier_sha256: str,
    operation_id_hex: str,
    binding: bytes,
    receipt: bytes,
    validation: bytes,
) -> dict[str, str]:
    """Build the public post-sign evidence carried outside the signed payload."""

    return {
        "schema": FOUNDATIONAL_SIGNER_RECEIPT_BUNDLE_SCHEMA,
        "verifier_sha256": verifier_sha256,
        "operation_id_hex": operation_id_hex,
        "binding_base64": base64.b64encode(binding).decode("ascii"),
        "receipt_base64": base64.b64encode(receipt).decode("ascii"),
        "validation_sha256": hashlib.sha256(validation).hexdigest(),
    }


def _decode_bundle_bytes(
    value: Any,
    *,
    field: str,
    maximum: int,
    errors: list[str],
) -> bytes | None:
    if not isinstance(value, str) or not value or not value.isascii():
        errors.append(f"foundational signer receipt bundle {field} must be canonical base64")
        return None
    try:
        decoded = base64.b64decode(value, validate=True)
    except (ValueError, binascii.Error):
        errors.append(f"foundational signer receipt bundle {field} must be canonical base64")
        return None
    if not decoded or len(decoded) > maximum or base64.b64encode(decoded).decode("ascii") != value:
        errors.append(
            f"foundational signer receipt bundle {field} must decode to bounded non-empty bytes"
        )
        return None
    return decoded


def validate_foundational_receipt_bundle(
    payload: dict[str, Any],
    signature: dict[str, Any] | None,
    *,
    verifier_path: Path | None,
    expected_verifier_sha256: str | None,
    errors: list[str],
) -> None:
    """Replay the pinned verifier before accepting software-key qualification."""

    signature_hex = None if signature is None else signature.get("signature_hex")
    if (
        not isinstance(signature_hex, str)
        or LOWER_SIGNATURE_RE.fullmatch(signature_hex) is None
        or not any(bytes.fromhex(signature_hex))
    ):
        return
    bundle = payload.get("signer_receipt_bundle")
    if not isinstance(bundle, dict):
        errors.append("foundational prerequisite requires a signer receipt bundle")
        return
    if set(bundle) != FOUNDATIONAL_SIGNER_RECEIPT_BUNDLE_FIELDS:
        errors.append("foundational signer receipt bundle fields do not match the contract")
    if bundle.get("schema") != FOUNDATIONAL_SIGNER_RECEIPT_BUNDLE_SCHEMA:
        errors.append("foundational signer receipt bundle schema must match the contract")
    verifier_sha256 = bundle.get("verifier_sha256")
    operation_id_hex = bundle.get("operation_id_hex")
    validation_sha256 = bundle.get("validation_sha256")
    for field, value in (
        ("verifier_sha256", verifier_sha256),
        ("operation_id_hex", operation_id_hex),
        ("validation_sha256", validation_sha256),
    ):
        if (
            not isinstance(value, str)
            or LOWER_SHA256_RE.fullmatch(value) is None
            or not any(bytes.fromhex(value))
        ):
            errors.append(
                f"foundational signer receipt bundle {field} must be a non-zero lowercase digest"
            )
    binding = _decode_bundle_bytes(
        bundle.get("binding_base64"),
        field="binding_base64",
        maximum=MAX_EXTERNAL_SIGNER_BINDING_BYTES,
        errors=errors,
    )
    receipt = _decode_bundle_bytes(
        bundle.get("receipt_base64"),
        field="receipt_base64",
        maximum=MAX_EXTERNAL_SIGNER_RECEIPT_BYTES,
        errors=errors,
    )
    if (
        not isinstance(expected_verifier_sha256, str)
        or LOWER_SHA256_RE.fullmatch(expected_verifier_sha256) is None
        or not any(bytes.fromhex(expected_verifier_sha256))
    ):
        errors.append(
            "foundational prerequisite requires an independently reviewed signer verifier SHA-256"
        )
    elif verifier_sha256 != expected_verifier_sha256:
        errors.append(
            "foundational signer receipt bundle verifier SHA-256 must match the independently reviewed digest"
        )
    verifier: bytes | None = None
    if not isinstance(verifier_path, Path):
        errors.append("foundational prerequisite requires a pinned signer receipt verifier")
    else:
        try:
            verifier = read_evidence_bytes(
                verifier_path, MAX_EXTERNAL_SIGNER_VERIFIER_BYTES
            )
        except (OSError, RuntimeError, ValueError):
            errors.append("foundational signer receipt verifier could not be read securely")
        if verifier == b"":
            errors.append("foundational signer receipt verifier must not be empty")
        elif (
            verifier is not None
            and isinstance(expected_verifier_sha256, str)
            and hashlib.sha256(verifier).hexdigest() != expected_verifier_sha256
        ):
            errors.append(
                "foundational signer receipt verifier SHA-256 does not match the independently reviewed digest"
            )
    if errors or verifier is None or binding is None or receipt is None:
        return
    validation, verifier_errors = run_offline_receipt_verifier(
        verifier=verifier,
        binding=binding,
        payload=foundational_signing_payload(payload),
        signature=bytes.fromhex(signature_hex),
        receipt=receipt,
        operation_id_hex=operation_id_hex,
    )
    errors.extend(verifier_errors)
    if validation is None:
        return
    parsed, validation_errors = parse_canonical_validation(validation)
    errors.extend(validation_errors)
    if parsed is not None:
        errors.extend(
            validate_receipt_validation(
                parsed,
                operation_id_hex=operation_id_hex,
                payload_length=len(foundational_signing_payload(payload)),
                service_id=signature.get("service_id"),
                administrator_id=signature.get("administrator_id"),
                key_revision=signature.get("key_revision"),
                policy_revision=signature.get("policy_revision"),
                policy_digest_sha256=signature.get("policy_digest_sha256"),
            )
        )
    if hashlib.sha256(validation).hexdigest() != validation_sha256:
        errors.append(
            "foundational signer receipt validation SHA-256 does not match the finalized bundle"
        )


def validate_foundational_receipt_from_options(
    payload: dict[str, Any],
    signature: dict[str, Any] | None,
    options: Any,
    errors: list[str],
) -> None:
    """Apply aggregate options to the post-sign receipt replay boundary."""

    if not getattr(options, "replay_foundational_signer_receipt", True):
        return
    validate_foundational_receipt_bundle(
        payload,
        signature,
        verifier_path=getattr(options, "foundational_signer_verifier", None),
        expected_verifier_sha256=getattr(
            options, "foundational_signer_verifier_sha256", None
        ),
        errors=errors,
    )


def add_foundational_receipt_verifier_arguments(parser: Any) -> None:
    """Add runtime-only trust-tool inputs without creating an evidence slot."""

    parser.add_argument(
        "--foundational-prerequisite-signer-verifier",
        dest="foundational_signer_verifier",
        type=Path,
        help="Pinned external software signer binary providing verify-receipt.",
    )
    parser.add_argument(
        "--foundational-prerequisite-signer-verifier-sha256",
        dest="foundational_signer_verifier_sha256",
        help="Independently reviewed SHA-256 of the exact signer verifier binary.",
    )


def parse_reviewed_verifier_sha256(
    value: Any,
    errors: list[str],
    *,
    label: str,
) -> str | None:
    """Normalize one independently reviewed non-zero verifier digest."""

    if (
        not isinstance(value, str)
        or LOWER_SHA256_RE.fullmatch(value) is None
        or not any(bytes.fromhex(value))
    ):
        errors.append(f"{label} must be a non-zero canonical lowercase SHA-256")
        return None
    return value


def verifier_sha256_from_args(args: Any) -> tuple[str | None, list[str]]:
    """Return an optional normalized checker trust-tool digest."""

    value = getattr(args, "foundational_signer_verifier_sha256", None)
    if value is None:
        return None, []
    errors: list[str] = []
    digest = parse_reviewed_verifier_sha256(
        value,
        errors,
        label="--foundational-prerequisite-signer-verifier-sha256",
    )
    return digest, errors


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
    "add_foundational_receipt_verifier_arguments",
    "foundational_signing_payload",
    "build_foundational_receipt_bundle",
    "parse_foundational_signer_public_key",
    "parse_reviewed_verifier_sha256",
    "validate_aggregate_software_signer",
    "validate_foundational_receipt_bundle",
    "validate_foundational_receipt_from_options",
    "validate_foundational_software_signer",
    "verifier_sha256_from_args",
]
