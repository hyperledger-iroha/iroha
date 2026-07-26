"""Vega existing-credential ZK SDK helpers."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping
from typing import Any

from .verange import (
    DEFAULT_PRIVACY_MAX_PROOF_BYTES,
    DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES,
    _MISSING,
    _bounded_bytes,
    _canonical_json_bytes,
    _fixed_bytes,
    _normalize_backend,
    _positive_u32,
    _read_single_alias,
    _reject_unknown_fields,
    _require_mapping,
    _require_non_blank_string,
    _require_plain_mapping,
    build_privacy_proof_envelope,
    decode_privacy_proof_envelope,
)
from .zkat import _normalize_account_id

VEGA_BACKEND = "stark/fri/sha256-goldilocks"
VEGA_CIRCUIT_ID = "stark/fri/sha256-goldilocks:vega_existing_credential_zk_v0"
VEGA_DOMAIN_SEPARATOR = "iroha:vega:existing-credential-zk:v0"
VEGA_DEV_PROOF_PREFIX = b"iroha:vega:dev-fixture:v0:"
VEGA_MAX_PREDICATE_BYTES = 1024 * 1024
VEGA_MAX_ISSUER_BYTES = 1024 * 1024

__all__ = [
    "VEGA_BACKEND",
    "VEGA_CIRCUIT_ID",
    "VEGA_DOMAIN_SEPARATOR",
    "build_vega_credential_predicate_commitment",
    "build_vega_credential_proof_envelope",
    "build_vega_credential_predicate_proof_v0",
    "build_vega_credential_dev_proof_fixture",
    "verify_vega_credential_predicate_proof_v0",
    "verify_vega_credential_proof_locally",
    "buildVegaCredentialPredicateCommitment",
    "buildVegaCredentialProofEnvelope",
    "buildVegaCredentialPredicateProofV0",
    "buildVegaCredentialDevProofFixture",
    "verifyVegaCredentialPredicateProofV0",
    "verifyVegaCredentialProofLocally",
]


def _normalize_version(value: Any, context: str) -> int:
    version = _positive_u32(1 if value is _MISSING or value is None else value, context)
    if version != 1:
        raise ValueError(f"{context} must be 1")
    return version


def _normalize_vega_backend(value: Any, context: str) -> str:
    _tag, decoded = _normalize_backend(
        VEGA_BACKEND if value is _MISSING or value is None else value,
        context,
    )
    if decoded != "Stark":
        raise ValueError(f"{context} must be {VEGA_BACKEND}")
    return VEGA_BACKEND


def _normalize_circuit_id(value: Any, context: str) -> str:
    circuit_id = _require_non_blank_string(
        VEGA_CIRCUIT_ID if value is _MISSING or value is None else value,
        context,
    )
    if circuit_id not in {VEGA_CIRCUIT_ID, "vega_existing_credential_zk_v0"}:
        raise ValueError(f"{context} must identify vega_existing_credential_zk_v0")
    return circuit_id


def _normalize_u32(value: Any, context: str) -> int:
    if isinstance(value, bool):
        raise TypeError(f"{context} must be a non-negative integer")
    if isinstance(value, int):
        parsed = value
    elif isinstance(value, str) and value.strip().isdigit():
        parsed = int(value.strip(), 10)
    else:
        raise TypeError(f"{context} must be a non-negative integer")
    if parsed < 0 or parsed > 0xFFFF_FFFF:
        raise ValueError(f"{context} must be between 0 and 4294967295")
    return parsed


def _normalize_credential_schema(value: Any, context: str) -> str:
    schema = _require_non_blank_string(value, context)
    if len(schema) > 256:
        raise ValueError(f"{context} must be no longer than 256 characters")
    return schema


def _normalize_structured_bytes(
    value: Any,
    alias_key: str | None,
    *,
    json_aliases: set[str],
    json_context: str,
    bytes_context: str,
    max_bytes: int,
) -> bytes:
    if (
        alias_key is not None
        and (
            alias_key.endswith("Json")
            or alias_key.endswith("_json")
            or alias_key in json_aliases
        )
    ):
        data = _canonical_json_bytes(value, json_context)
    else:
        data = _bounded_bytes(value, bytes_context, max_bytes=max_bytes)
    if len(data) > max_bytes:
        raise ValueError(f"{bytes_context} must be no larger than {max_bytes} bytes")
    return data


def _predicate_commitment_bytes(
    *,
    predicate_bytes: bytes,
    credential_schema: str,
    domain_separator: str,
) -> bytes:
    digest = hashlib.sha256()
    digest.update(b"iroha:vega:predicate-commitment:v0")
    digest.update(b"\x00")
    digest.update(credential_schema.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(domain_separator.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(predicate_bytes)
    return digest.digest()


def _issuer_commitment_bytes(*, issuer_bytes: bytes, domain_separator: str) -> bytes:
    digest = hashlib.sha256()
    digest.update(b"iroha:vega:issuer-commitment:v0")
    digest.update(b"\x00")
    digest.update(domain_separator.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(issuer_bytes)
    return digest.digest()


def _subject_binding_bytes(*, account_id: str, domain_separator: str) -> bytes:
    digest = hashlib.sha256()
    digest.update(b"iroha:vega:subject-binding:v0")
    digest.update(b"\x00")
    digest.update(domain_separator.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(account_id.encode("utf-8"))
    return digest.digest()


def _normalize_predicate_commitment_from_source(
    source: Mapping[str, Any],
    context: str,
) -> dict[str, Any]:
    _commitment_key, commitment_value = _read_single_alias(
        source,
        ("predicateCommitment", "predicate_commitment", "commitment"),
        f"{context}.predicateCommitment",
        "predicate commitment",
    )
    predicate_key, predicate_value = _read_single_alias(
        source,
        ("predicate", "predicateBytes", "predicate_bytes", "predicateJson", "predicate_json"),
        f"{context}.predicate",
        "predicate",
    )
    _schema_key, schema_value = _read_single_alias(
        source,
        ("credentialSchema", "credential_schema"),
        f"{context}.credentialSchema",
        "credential schema",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    credential_schema = _normalize_credential_schema(
        schema_value,
        f"{context}.credentialSchema",
    )
    domain_separator = _require_non_blank_string(
        VEGA_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    explicit_commitment = (
        None
        if commitment_value is _MISSING
        else _fixed_bytes(
            commitment_value,
            f"{context}.predicateCommitment",
            32,
            nonzero=True,
        )
    )
    if predicate_key is None:
        predicate_bytes = None
    else:
        max_predicate_bytes = _positive_u32(
            source.get(
                "maxPredicateBytes",
                source.get("max_predicate_bytes", VEGA_MAX_PREDICATE_BYTES),
            ),
            f"{context}.maxPredicateBytes",
        )
        predicate_bytes = _normalize_structured_bytes(
            predicate_value,
            predicate_key,
            json_aliases={"predicate"},
            json_context=f"{context}.predicateJson",
            bytes_context=f"{context}.predicate",
            max_bytes=max_predicate_bytes,
        )
    if explicit_commitment is None and predicate_bytes is None:
        raise ValueError(
            f"{context}.predicateCommitment or {context}.predicate is required"
        )
    derived_commitment = (
        None
        if predicate_bytes is None
        else _predicate_commitment_bytes(
            predicate_bytes=predicate_bytes,
            credential_schema=credential_schema,
            domain_separator=domain_separator,
        )
    )
    if explicit_commitment is not None and derived_commitment is not None:
        if explicit_commitment != derived_commitment:
            raise ValueError(
                f"{context}.predicateCommitment must match the derived predicate commitment"
            )
    predicate_commitment = (
        explicit_commitment if explicit_commitment is not None else derived_commitment
    )
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "predicate_commitment": predicate_commitment,
        "credential_schema": credential_schema,
        "domain_separator": domain_separator,
        "commitment_kind": (
            "external" if predicate_bytes is None else "dev-sha256-predicate-digest"
        ),
        "predicate_digest": None
        if predicate_bytes is None
        else hashlib.sha256(predicate_bytes).digest(),
    }


def _normalize_issuer_commitment_from_source(
    source: Mapping[str, Any],
    context: str,
) -> bytes:
    _commitment_key, commitment_value = _read_single_alias(
        source,
        ("issuerCommitment", "issuer_commitment"),
        f"{context}.issuerCommitment",
        "issuer commitment",
    )
    issuer_key, issuer_value = _read_single_alias(
        source,
        ("issuer", "issuerBytes", "issuer_bytes", "issuerJson", "issuer_json"),
        f"{context}.issuer",
        "issuer",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    domain_separator = _require_non_blank_string(
        VEGA_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    explicit_commitment = (
        None
        if commitment_value is _MISSING
        else _fixed_bytes(
            commitment_value,
            f"{context}.issuerCommitment",
            32,
            nonzero=True,
        )
    )
    if issuer_key is None:
        issuer_bytes = None
    else:
        max_issuer_bytes = _positive_u32(
            source.get(
                "maxIssuerBytes",
                source.get("max_issuer_bytes", VEGA_MAX_ISSUER_BYTES),
            ),
            f"{context}.maxIssuerBytes",
        )
        issuer_bytes = _normalize_structured_bytes(
            issuer_value,
            issuer_key,
            json_aliases={"issuer"},
            json_context=f"{context}.issuerJson",
            bytes_context=f"{context}.issuer",
            max_bytes=max_issuer_bytes,
        )
    if explicit_commitment is None and issuer_bytes is None:
        raise ValueError(f"{context}.issuerCommitment or {context}.issuer is required")
    derived_commitment = (
        None
        if issuer_bytes is None
        else _issuer_commitment_bytes(
            issuer_bytes=issuer_bytes,
            domain_separator=domain_separator,
        )
    )
    if explicit_commitment is not None and derived_commitment is not None:
        if explicit_commitment != derived_commitment:
            raise ValueError(
                f"{context}.issuerCommitment must match the derived issuer commitment"
            )
    return explicit_commitment if explicit_commitment is not None else derived_commitment  # type: ignore[return-value]


def _normalize_subject_binding_from_source(
    source: Mapping[str, Any],
    context: str,
) -> bytes:
    _binding_key, binding_value = _read_single_alias(
        source,
        (
            "subjectBinding",
            "subject_binding",
            "identityCommitment",
            "identity_commitment",
            "accountCommitment",
            "account_commitment",
        ),
        f"{context}.subjectBinding",
        "subject binding",
    )
    _account_key, account_value = _read_single_alias(
        source,
        ("accountId", "account_id", "subjectAccountId", "subject_account_id"),
        f"{context}.accountId",
        "account id",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    domain_separator = _require_non_blank_string(
        VEGA_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    explicit_binding = (
        None
        if binding_value is _MISSING
        else _fixed_bytes(
            binding_value,
            f"{context}.subjectBinding",
            32,
            nonzero=True,
        )
    )
    account_binding = (
        None
        if account_value is _MISSING
        else _subject_binding_bytes(
            account_id=_normalize_account_id(account_value, f"{context}.accountId"),
            domain_separator=domain_separator,
        )
    )
    if explicit_binding is None and account_binding is None:
        raise ValueError(f"{context}.subjectBinding or {context}.accountId is required")
    if explicit_binding is not None and account_binding is not None:
        if explicit_binding != account_binding:
            raise ValueError(
                f"{context}.subjectBinding must match the derived account subject binding"
            )
    return explicit_binding if explicit_binding is not None else account_binding  # type: ignore[return-value]


def _normalize_public_inputs(value: Any, context: str) -> dict[str, Any]:
    source = _require_mapping(value, context)
    _reject_unknown_fields(
        source,
        {
            "version",
            "issuer_commitment",
            "issuerCommitment",
            "credential_schema",
            "credentialSchema",
            "predicate_commitment",
            "predicateCommitment",
            "subject_binding",
            "subjectBinding",
            "expiration_epoch",
            "expirationEpoch",
            "domain_separator",
            "domainSeparator",
        },
        context,
    )
    _issuer_key, issuer_value = _read_single_alias(
        source,
        ("issuer_commitment", "issuerCommitment"),
        f"{context}.issuerCommitment",
        "issuer commitment",
    )
    _schema_key, schema_value = _read_single_alias(
        source,
        ("credential_schema", "credentialSchema"),
        f"{context}.credentialSchema",
        "credential schema",
    )
    _predicate_key, predicate_value = _read_single_alias(
        source,
        ("predicate_commitment", "predicateCommitment"),
        f"{context}.predicateCommitment",
        "predicate commitment",
    )
    _subject_key, subject_value = _read_single_alias(
        source,
        ("subject_binding", "subjectBinding"),
        f"{context}.subjectBinding",
        "subject binding",
    )
    _expiration_key, expiration_value = _read_single_alias(
        source,
        ("expiration_epoch", "expirationEpoch"),
        f"{context}.expirationEpoch",
        "expiration epoch",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domain_separator", "domainSeparator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "issuer_commitment": _fixed_bytes(
            issuer_value,
            f"{context}.issuerCommitment",
            32,
            nonzero=True,
        ).hex(),
        "credential_schema": _normalize_credential_schema(
            schema_value,
            f"{context}.credentialSchema",
        ),
        "predicate_commitment": _fixed_bytes(
            predicate_value,
            f"{context}.predicateCommitment",
            32,
            nonzero=True,
        ).hex(),
        "subject_binding": _fixed_bytes(
            subject_value,
            f"{context}.subjectBinding",
            32,
            nonzero=True,
        ).hex(),
        "expiration_epoch": _normalize_u32(
            expiration_value,
            f"{context}.expirationEpoch",
        ),
        "domain_separator": _require_non_blank_string(
            domain_value,
            f"{context}.domainSeparator",
        ),
    }


def _normalize_proof_parts(
    source: Mapping[str, Any],
    context: str,
    *,
    require_proof_bytes: bool,
) -> dict[str, Any]:
    _backend_key, backend_value = _read_single_alias(
        source,
        ("backendTag", "backend_tag", "backend"),
        f"{context}.backendTag",
        "backend tag",
    )
    _circuit_key, circuit_value = _read_single_alias(
        source,
        ("circuitId", "circuit_id"),
        f"{context}.circuitId",
        "circuit id",
    )
    _vk_key, vk_hash_value = _read_single_alias(
        source,
        ("vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash"),
        f"{context}.vkHash",
        "verifying key hash",
    )
    _proof_key, proof_value = _read_single_alias(
        source,
        ("proofBytes", "proof_bytes", "proof"),
        f"{context}.proofBytes",
        "proof bytes",
    )
    if require_proof_bytes and proof_value is _MISSING:
        raise TypeError(f"{context}.proofBytes is required")
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    _schema_key, schema_value = _read_single_alias(
        source,
        ("credentialSchema", "credential_schema"),
        f"{context}.credentialSchema",
        "credential schema",
    )
    _expiration_key, expiration_value = _read_single_alias(
        source,
        ("expirationEpoch", "expiration_epoch"),
        f"{context}.expirationEpoch",
        "expiration epoch",
    )
    domain_separator = _require_non_blank_string(
        VEGA_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    credential_schema = _normalize_credential_schema(
        schema_value,
        f"{context}.credentialSchema",
    )
    public_inputs = {
        "version": 1,
        "issuer_commitment": _normalize_issuer_commitment_from_source(
            source,
            context,
        ).hex(),
        "credential_schema": credential_schema,
        "predicate_commitment": _normalize_predicate_commitment_from_source(
            source,
            context,
        )["predicate_commitment"].hex(),
        "subject_binding": _normalize_subject_binding_from_source(
            source,
            context,
        ).hex(),
        "expiration_epoch": _normalize_u32(
            expiration_value,
            f"{context}.expirationEpoch",
        ),
        "domain_separator": domain_separator,
    }
    max_proof_bytes = _positive_u32(
        source.get("maxProofBytes", source.get("max_proof_bytes", DEFAULT_PRIVACY_MAX_PROOF_BYTES)),
        f"{context}.maxProofBytes",
    )
    return {
        "backend": _normalize_vega_backend(backend_value, f"{context}.backendTag"),
        "circuit_id": _normalize_circuit_id(circuit_value, f"{context}.circuitId"),
        "vk_hash": _fixed_bytes(vk_hash_value, f"{context}.vkHash", 32, nonzero=True),
        "public_inputs": public_inputs,
        "public_input_bytes": _canonical_json_bytes(
            public_inputs,
            f"{context}.publicInputs",
        ),
        "proof_bytes": (
            None
            if proof_value is _MISSING
            else _bounded_bytes(
                proof_value,
                f"{context}.proofBytes",
                max_bytes=max_proof_bytes,
            )
        ),
        "max_proof_bytes": max_proof_bytes,
        "max_public_input_bytes": source.get(
            "maxPublicInputBytes",
            source.get("max_public_input_bytes", DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES),
        ),
    }


_PREDICATE_FIELDS = {
    "version",
    "predicateCommitment",
    "predicate_commitment",
    "commitment",
    "predicate",
    "predicateBytes",
    "predicate_bytes",
    "predicateJson",
    "predicate_json",
    "credentialSchema",
    "credential_schema",
    "domainSeparator",
    "domain_separator",
    "maxPredicateBytes",
    "max_predicate_bytes",
}


def build_vega_credential_predicate_commitment(
    options: Mapping[str, Any],
    context: str = "vegaCredentialPredicateCommitment",
) -> dict[str, Any]:
    """Normalize or derive a Vega credential predicate commitment."""

    source = _require_plain_mapping(options, context)
    _reject_unknown_fields(source, _PREDICATE_FIELDS, context)
    return _normalize_predicate_commitment_from_source(source, context)


_PROOF_FIELDS = {
    "version",
    "backend",
    "backendTag",
    "backend_tag",
    "circuitId",
    "circuit_id",
    "vkHash",
    "vk_hash",
    "verifierKeyHash",
    "verifyingKeyHash",
    "issuerCommitment",
    "issuer_commitment",
    "issuer",
    "issuerBytes",
    "issuer_bytes",
    "issuerJson",
    "issuer_json",
    "predicateCommitment",
    "predicate_commitment",
    "commitment",
    "predicate",
    "predicateBytes",
    "predicate_bytes",
    "predicateJson",
    "predicate_json",
    "credentialSchema",
    "credential_schema",
    "subjectBinding",
    "subject_binding",
    "identityCommitment",
    "identity_commitment",
    "accountCommitment",
    "account_commitment",
    "accountId",
    "account_id",
    "subjectAccountId",
    "subject_account_id",
    "expirationEpoch",
    "expiration_epoch",
    "domainSeparator",
    "domain_separator",
    "proofBytes",
    "proof_bytes",
    "proof",
    "aux",
    "maxIssuerBytes",
    "max_issuer_bytes",
    "maxPredicateBytes",
    "max_predicate_bytes",
    "maxProofBytes",
    "max_proof_bytes",
    "maxPublicInputBytes",
    "max_public_input_bytes",
}


def build_vega_credential_proof_envelope(options: Mapping[str, Any]) -> bytes:
    """Build canonical OpenVerifyEnvelope bytes for a prepared Vega proof."""

    source = _require_plain_mapping(options, "vegaCredentialProofEnvelope")
    _reject_unknown_fields(source, _PROOF_FIELDS, "vegaCredentialProofEnvelope")
    parts = _normalize_proof_parts(
        source,
        "vegaCredentialProofEnvelope",
        require_proof_bytes=True,
    )
    return build_privacy_proof_envelope(
        {
            "backend": parts["backend"],
            "circuitId": parts["circuit_id"],
            "vkHash": parts["vk_hash"],
            "publicInputs": parts["public_input_bytes"],
            "proofBytes": parts["proof_bytes"],
            "aux": source.get("aux", b""),
            "maxProofBytes": parts["max_proof_bytes"],
            "maxPublicInputBytes": parts["max_public_input_bytes"],
        }
    )


def build_vega_credential_predicate_proof_v0(
    options: Mapping[str, Any],
) -> bytes:
    """Build canonical production Vega credential predicate proof bytes."""

    source = _require_plain_mapping(options, "vegaCredentialPredicateProofV0")
    _reject_unknown_fields(source, _PROOF_FIELDS, "vegaCredentialPredicateProofV0")
    parts = _normalize_proof_parts(
        source,
        "vegaCredentialPredicateProofV0",
        require_proof_bytes=True,
    )
    if parts["proof_bytes"].startswith(VEGA_DEV_PROOF_PREFIX):
        raise ValueError(
            "vegaCredentialPredicateProofV0.proofBytes must not contain a dev fixture proof"
        )
    return build_privacy_proof_envelope(
        {
            "backend": parts["backend"],
            "circuitId": parts["circuit_id"],
            "vkHash": parts["vk_hash"],
            "publicInputs": parts["public_input_bytes"],
            "proofBytes": parts["proof_bytes"],
            "aux": source.get("aux", b""),
            "maxProofBytes": parts["max_proof_bytes"],
            "maxPublicInputBytes": parts["max_public_input_bytes"],
        }
    )


def _dev_proof_bytes(
    *,
    circuit_id: str,
    vk_hash: bytes,
    public_input_bytes: bytes,
) -> bytes:
    digest = hashlib.sha256()
    digest.update(b"iroha:vega:dev-fixture:v0")
    digest.update(b"\x00")
    digest.update(circuit_id.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(vk_hash)
    digest.update(b"\x00")
    digest.update(public_input_bytes)
    return VEGA_DEV_PROOF_PREFIX + digest.digest()


def build_vega_credential_dev_proof_fixture(options: Mapping[str, Any]) -> dict[str, Any]:
    """Build a deterministic Vega credential dev proof fixture."""

    source = _require_plain_mapping(options, "vegaCredentialDevProofFixture")
    _reject_unknown_fields(
        source,
        _PROOF_FIELDS - {"proofBytes", "proof_bytes", "proof"},
        "vegaCredentialDevProofFixture",
    )
    parts = _normalize_proof_parts(
        source,
        "vegaCredentialDevProofFixture",
        require_proof_bytes=False,
    )
    proof_bytes = _dev_proof_bytes(
        circuit_id=parts["circuit_id"],
        vk_hash=parts["vk_hash"],
        public_input_bytes=parts["public_input_bytes"],
    )
    envelope = build_privacy_proof_envelope(
        {
            "backend": parts["backend"],
            "circuitId": parts["circuit_id"],
            "vkHash": parts["vk_hash"],
            "publicInputs": parts["public_input_bytes"],
            "proofBytes": proof_bytes,
            "aux": source.get("aux", b""),
            "maxProofBytes": parts["max_proof_bytes"],
            "maxPublicInputBytes": parts["max_public_input_bytes"],
        }
    )
    return {
        "kind": "vega-dev-fixture-v0",
        "production": False,
        "proof_bytes": proof_bytes,
        "proofBytes": proof_bytes,
        "public_inputs": parts["public_inputs"],
        "public_input_bytes": parts["public_input_bytes"],
        "publicInputBytes": parts["public_input_bytes"],
        "envelope": envelope,
    }


def _parse_public_inputs(value: bytes, context: str) -> dict[str, Any]:
    try:
        parsed = json.loads(value.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError(f"{context} must contain valid JSON public inputs") from exc
    normalized = _normalize_public_inputs(parsed, context)
    if value != _canonical_json_bytes(normalized, context):
        raise ValueError(f"{context} must use canonical JSON encoding")
    return normalized


def _expectation_source_with_public_defaults(
    source: Mapping[str, Any],
    public_inputs: Mapping[str, Any],
) -> dict[str, Any]:
    result = dict(source)
    if not any(key in result for key in ("domainSeparator", "domain_separator")):
        result["domainSeparator"] = public_inputs["domain_separator"]
    if not any(key in result for key in ("credentialSchema", "credential_schema")):
        result["credentialSchema"] = public_inputs["credential_schema"]
    return result


def _ensure_verification_expectations(
    source: Mapping[str, Any],
    public_inputs: Mapping[str, Any],
    context: str,
) -> None:
    expectation_source = _expectation_source_with_public_defaults(source, public_inputs)
    if any(
        key in source
        for key in (
            "issuerCommitment",
            "issuer_commitment",
            "issuer",
            "issuerBytes",
            "issuer_bytes",
            "issuerJson",
            "issuer_json",
        )
    ):
        expected = _normalize_issuer_commitment_from_source(
            expectation_source,
            context,
        ).hex()
        if expected != public_inputs["issuer_commitment"]:
            raise ValueError(
                f"{context}.issuerCommitment must match the envelope public inputs"
            )
    if any(
        key in source
        for key in (
            "predicateCommitment",
            "predicate_commitment",
            "commitment",
            "predicate",
            "predicateBytes",
            "predicate_bytes",
            "predicateJson",
            "predicate_json",
        )
    ):
        expected = _normalize_predicate_commitment_from_source(
            expectation_source,
            context,
        )["predicate_commitment"].hex()
        if expected != public_inputs["predicate_commitment"]:
            raise ValueError(
                f"{context}.predicateCommitment must match the envelope public inputs"
            )
    if any(
        key in source
        for key in (
            "subjectBinding",
            "subject_binding",
            "identityCommitment",
            "identity_commitment",
            "accountCommitment",
            "account_commitment",
            "accountId",
            "account_id",
            "subjectAccountId",
            "subject_account_id",
        )
    ):
        expected = _normalize_subject_binding_from_source(
            expectation_source,
            context,
        ).hex()
        if expected != public_inputs["subject_binding"]:
            raise ValueError(
                f"{context}.subjectBinding must match the envelope public inputs"
            )
    for fields, path, normalize, actual in (
        (
            ("credentialSchema", "credential_schema"),
            "credentialSchema",
            lambda value: _normalize_credential_schema(
                value,
                f"{context}.credentialSchema",
            ),
            public_inputs["credential_schema"],
        ),
        (
            ("expirationEpoch", "expiration_epoch"),
            "expirationEpoch",
            lambda value: _normalize_u32(value, f"{context}.expirationEpoch"),
            public_inputs["expiration_epoch"],
        ),
        (
            ("domainSeparator", "domain_separator"),
            "domainSeparator",
            lambda value: _require_non_blank_string(
                value,
                f"{context}.domainSeparator",
            ),
            public_inputs["domain_separator"],
        ),
    ):
        _key, value = _read_single_alias(source, fields, f"{context}.{path}", path)
        if value is not _MISSING and normalize(value) != actual:
            raise ValueError(f"{context}.{path} must match the envelope public inputs")


def verify_vega_credential_proof_locally(options: Any) -> dict[str, Any]:
    """Verify a deterministic Vega dev fixture through an OpenVerify envelope."""

    if isinstance(options, Mapping):
        source = _require_plain_mapping(options, "vegaCredentialLocalVerification")
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        (
            _PROOF_FIELDS
            | {"envelope", "proofEnvelope", "proof_envelope", "bytes"}
        )
        - {
            "backend",
            "backendTag",
            "backend_tag",
            "circuitId",
            "circuit_id",
            "vkHash",
            "vk_hash",
            "verifierKeyHash",
            "verifyingKeyHash",
            "proofBytes",
            "proof_bytes",
            "proof",
            "aux",
            "maxProofBytes",
            "max_proof_bytes",
            "maxPublicInputBytes",
            "max_public_input_bytes",
            "version",
        },
        "vegaCredentialLocalVerification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "vegaCredentialLocalVerification.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError("vegaCredentialLocalVerification.envelope.backend must be Stark")
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "vegaCredentialLocalVerification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "vegaCredentialLocalVerification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "vegaCredentialLocalVerification.publicInputs",
    )
    _ensure_verification_expectations(
        source,
        public_inputs,
        "vegaCredentialLocalVerification",
    )
    expected_proof = _dev_proof_bytes(
        circuit_id=circuit_id,
        vk_hash=vk_hash,
        public_input_bytes=decoded["public_inputs"],
    )
    if decoded["proof_bytes"] != expected_proof:
        raise ValueError(
            "vegaCredentialLocalVerification proof bytes are not a valid Vega dev fixture"
        )
    return {
        "ok": True,
        "production": False,
        "kind": "vega-dev-fixture-v0",
        "backend": VEGA_BACKEND,
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "credential_schema": public_inputs["credential_schema"],
        "expiration_epoch": public_inputs["expiration_epoch"],
    }


def verify_vega_credential_predicate_proof_v0(options: Any) -> dict[str, Any]:
    """Validate a production Vega credential predicate proof envelope."""

    if isinstance(options, Mapping):
        source = _require_plain_mapping(options, "vegaCredentialPredicateProofV0")
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        (
            _PROOF_FIELDS
            | {"envelope", "proofEnvelope", "proof_envelope", "bytes"}
        )
        - {
            "backend",
            "backendTag",
            "backend_tag",
            "circuitId",
            "circuit_id",
            "vkHash",
            "vk_hash",
            "verifierKeyHash",
            "verifyingKeyHash",
            "proofBytes",
            "proof_bytes",
            "proof",
            "aux",
            "maxProofBytes",
            "max_proof_bytes",
            "maxPublicInputBytes",
            "max_public_input_bytes",
            "version",
        },
        "vegaCredentialPredicateProofV0",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "vegaCredentialPredicateProofV0.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError("vegaCredentialPredicateProofV0.envelope.backend must be Stark")
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "vegaCredentialPredicateProofV0.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "vegaCredentialPredicateProofV0.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "vegaCredentialPredicateProofV0.publicInputs",
    )
    _ensure_verification_expectations(
        source,
        public_inputs,
        "vegaCredentialPredicateProofV0",
    )
    if decoded["proof_bytes"].startswith(VEGA_DEV_PROOF_PREFIX):
        raise ValueError(
            "vegaCredentialPredicateProofV0 proof bytes must not contain a Vega dev fixture"
        )
    return {
        "ok": True,
        "production": True,
        "kind": "vega-existing-credential-zk-v0",
        "backend": "Stark",
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "credential_schema": public_inputs["credential_schema"],
        "expiration_epoch": public_inputs["expiration_epoch"],
    }


buildVegaCredentialPredicateCommitment = build_vega_credential_predicate_commitment
buildVegaCredentialProofEnvelope = build_vega_credential_proof_envelope
buildVegaCredentialPredicateProofV0 = build_vega_credential_predicate_proof_v0
buildVegaCredentialDevProofFixture = build_vega_credential_dev_proof_fixture
verifyVegaCredentialPredicateProofV0 = verify_vega_credential_predicate_proof_v0
verifyVegaCredentialProofLocally = verify_vega_credential_proof_locally
