"""Silent-threshold anonymous credential SDK dev-fixture helpers."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
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
    build_privacy_proof_envelope,
    decode_privacy_proof_envelope,
)

SILENT_THRESHOLD_BACKEND = "stark/fri/sha256-goldilocks"
SILENT_THRESHOLD_CIRCUIT_ID = (
    "stark/fri/sha256-goldilocks:silent_threshold_anoncred_v0"
)
SILENT_THRESHOLD_DOMAIN_SEPARATOR = "iroha:silent-threshold:anoncred:v0"
SILENT_THRESHOLD_DEV_PROOF_PREFIX = b"iroha:silent-threshold:dev-fixture:v0:"
SILENT_THRESHOLD_MAX_ISSUER_SET_BYTES = 1024 * 1024
SILENT_THRESHOLD_MAX_POLICY_BYTES = 1024 * 1024
SILENT_THRESHOLD_MAX_SHOWING_BYTES = 1024 * 1024

__all__ = [
    "SILENT_THRESHOLD_BACKEND",
    "SILENT_THRESHOLD_CIRCUIT_ID",
    "SILENT_THRESHOLD_DOMAIN_SEPARATOR",
    "build_silent_threshold_credential_commitments",
    "build_silent_threshold_credential_envelope",
    "build_silent_threshold_credential_dev_proof_fixture",
    "verify_silent_threshold_credential_proof_locally",
    "buildSilentThresholdCredentialCommitments",
    "buildSilentThresholdCredentialEnvelope",
    "buildSilentThresholdCredentialDevProofFixture",
    "verifySilentThresholdCredentialProofLocally",
]


def _normalize_version(value: Any, context: str) -> int:
    version = _positive_u32(1 if value is _MISSING or value is None else value, context)
    if version != 1:
        raise ValueError(f"{context} must be 1")
    return version


def _normalize_backend_tag(value: Any, context: str) -> str:
    _tag, decoded = _normalize_backend(
        SILENT_THRESHOLD_BACKEND if value is _MISSING or value is None else value,
        context,
    )
    if decoded != "Stark":
        raise ValueError(f"{context} must be {SILENT_THRESHOLD_BACKEND}")
    return SILENT_THRESHOLD_BACKEND


def _normalize_circuit_id(value: Any, context: str) -> str:
    circuit_id = _require_non_blank_string(
        SILENT_THRESHOLD_CIRCUIT_ID if value is _MISSING or value is None else value,
        context,
    )
    if circuit_id not in {
        SILENT_THRESHOLD_CIRCUIT_ID,
        "silent_threshold_anoncred_v0",
    }:
        raise ValueError(f"{context} must identify silent_threshold_anoncred_v0")
    return circuit_id


def _structured_bytes(
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


def _digest_bytes(label: str, data: bytes, domain_separator: str) -> bytes:
    digest = hashlib.sha256()
    digest.update(f"iroha:silent-threshold:{label}:v0".encode("utf-8"))
    digest.update(b"\x00")
    digest.update(domain_separator.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(data)
    return digest.digest()


def _derived_field(
    source: Mapping[str, Any],
    context: str,
    *,
    explicit_aliases: Sequence[str],
    data_aliases: Sequence[str],
    json_aliases: set[str],
    field_path: str,
    data_path: str,
    data_json_path: str,
    data_description: str,
    digest_label: str,
    kind_label: str,
    max_bytes: int,
) -> dict[str, Any]:
    _explicit_key, explicit_value = _read_single_alias(
        source,
        explicit_aliases,
        f"{context}.{field_path}",
        data_description,
    )
    data_key, data_value = _read_single_alias(
        source,
        data_aliases,
        f"{context}.{data_path}",
        data_description,
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    domain_separator = _require_non_blank_string(
        SILENT_THRESHOLD_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    explicit = (
        None
        if explicit_value is _MISSING
        else _fixed_bytes(
            explicit_value,
            f"{context}.{field_path}",
            32,
            nonzero=True,
        )
    )
    data_bytes = (
        None
        if data_key is None
        else _structured_bytes(
            data_value,
            data_key,
            json_aliases=json_aliases,
            json_context=f"{context}.{data_json_path}",
            bytes_context=f"{context}.{data_path}",
            max_bytes=max_bytes,
        )
    )
    if explicit is None and data_bytes is None:
        raise ValueError(f"{context}.{field_path} or {context}.{data_path} is required")
    derived = (
        None
        if data_bytes is None
        else _digest_bytes(digest_label, data_bytes, domain_separator)
    )
    if explicit is not None and derived is not None and explicit != derived:
        raise ValueError(
            f"{context}.{field_path} must match the derived {data_description}"
        )
    return {
        "value": explicit if explicit is not None else derived,
        "kind": "external" if data_bytes is None else f"dev-sha256-{kind_label}",
        "digest": None if data_bytes is None else hashlib.sha256(data_bytes).digest(),
    }


def _issuer_set_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxIssuerSetBytes",
            source.get("max_issuer_set_bytes", SILENT_THRESHOLD_MAX_ISSUER_SET_BYTES),
        ),
        f"{context}.maxIssuerSetBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=("issuerSetCommitment", "issuer_set_commitment"),
        data_aliases=(
            "issuerSet",
            "issuer_set",
            "issuerSetBytes",
            "issuer_set_bytes",
            "issuerSetJson",
            "issuer_set_json",
        ),
        json_aliases={"issuerSet", "issuer_set"},
        field_path="issuerSetCommitment",
        data_path="issuerSet",
        data_json_path="issuerSetJson",
        data_description="issuer-set commitment",
        digest_label="issuer-set-commitment",
        kind_label="issuer-set-digest",
        max_bytes=max_bytes,
    )


def _threshold_policy_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxPolicyBytes",
            source.get("max_policy_bytes", SILENT_THRESHOLD_MAX_POLICY_BYTES),
        ),
        f"{context}.maxPolicyBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=("thresholdPolicyHash", "threshold_policy_hash"),
        data_aliases=(
            "thresholdPolicy",
            "threshold_policy",
            "thresholdPolicyBytes",
            "threshold_policy_bytes",
            "thresholdPolicyJson",
            "threshold_policy_json",
        ),
        json_aliases={"thresholdPolicy", "threshold_policy"},
        field_path="thresholdPolicyHash",
        data_path="thresholdPolicy",
        data_json_path="thresholdPolicyJson",
        data_description="threshold policy hash",
        digest_label="threshold-policy-hash",
        kind_label="threshold-policy-digest",
        max_bytes=max_bytes,
    )


def _showing_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxShowingBytes",
            source.get("max_showing_bytes", SILENT_THRESHOLD_MAX_SHOWING_BYTES),
        ),
        f"{context}.maxShowingBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=(
            "credentialShowingCommitment",
            "credential_showing_commitment",
            "showingCommitment",
            "showing_commitment",
        ),
        data_aliases=(
            "credentialShowing",
            "credential_showing",
            "credentialShowingBytes",
            "credential_showing_bytes",
            "credentialShowingJson",
            "credential_showing_json",
            "showing",
            "showingBytes",
            "showing_bytes",
            "showingJson",
            "showing_json",
        ),
        json_aliases={"credentialShowing", "credential_showing", "showing"},
        field_path="credentialShowingCommitment",
        data_path="credentialShowing",
        data_json_path="credentialShowingJson",
        data_description="credential showing commitment",
        digest_label="credential-showing-commitment",
        kind_label="credential-showing-digest",
        max_bytes=max_bytes,
    )


def _showing_nullifier_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxShowingBytes",
            source.get("max_showing_bytes", SILENT_THRESHOLD_MAX_SHOWING_BYTES),
        ),
        f"{context}.maxShowingBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=(
            "showingNullifier",
            "showing_nullifier",
            "credentialShowingNullifier",
            "credential_showing_nullifier",
            "nullifier",
        ),
        data_aliases=(
            "credentialShowing",
            "credential_showing",
            "credentialShowingBytes",
            "credential_showing_bytes",
            "credentialShowingJson",
            "credential_showing_json",
            "showing",
            "showingBytes",
            "showing_bytes",
            "showingJson",
            "showing_json",
        ),
        json_aliases={"credentialShowing", "credential_showing", "showing"},
        field_path="showingNullifier",
        data_path="credentialShowing",
        data_json_path="credentialShowingJson",
        data_description="credential showing nullifier",
        digest_label="credential-showing-nullifier",
        kind_label="credential-showing-nullifier",
        max_bytes=max_bytes,
    )


def _verifier_policy_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxPolicyBytes",
            source.get("max_policy_bytes", SILENT_THRESHOLD_MAX_POLICY_BYTES),
        ),
        f"{context}.maxPolicyBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=("verifierPolicyHash", "verifier_policy_hash"),
        data_aliases=(
            "verifierPolicy",
            "verifier_policy",
            "verifierPolicyBytes",
            "verifier_policy_bytes",
            "verifierPolicyJson",
            "verifier_policy_json",
        ),
        json_aliases={"verifierPolicy", "verifier_policy"},
        field_path="verifierPolicyHash",
        data_path="verifierPolicy",
        data_json_path="verifierPolicyJson",
        data_description="verifier policy hash",
        digest_label="verifier-policy-hash",
        kind_label="verifier-policy-digest",
        max_bytes=max_bytes,
    )


def _source_with_domain(source: Mapping[str, Any], domain_separator: str) -> dict[str, Any]:
    normalized = {
        key: value
        for key, value in source.items()
        if key not in {"domainSeparator", "domain_separator"}
    }
    normalized["domainSeparator"] = domain_separator
    return normalized


def _commitment_parts(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    domain_separator = _require_non_blank_string(
        SILENT_THRESHOLD_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    normalized = _source_with_domain(source, domain_separator)
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "issuer_set": _issuer_set_field(normalized, context),
        "threshold_policy": _threshold_policy_field(normalized, context),
        "showing": _showing_field(normalized, context),
        "showing_nullifier": _showing_nullifier_field(normalized, context),
        "verifier_policy": _verifier_policy_field(normalized, context),
        "domain_separator": domain_separator,
    }


_COMMON_FIELDS = {
    "version",
    "issuerSetCommitment",
    "issuer_set_commitment",
    "issuerSet",
    "issuer_set",
    "issuerSetBytes",
    "issuer_set_bytes",
    "issuerSetJson",
    "issuer_set_json",
    "thresholdPolicyHash",
    "threshold_policy_hash",
    "thresholdPolicy",
    "threshold_policy",
    "thresholdPolicyBytes",
    "threshold_policy_bytes",
    "thresholdPolicyJson",
    "threshold_policy_json",
    "credentialShowingCommitment",
    "credential_showing_commitment",
    "showingCommitment",
    "showing_commitment",
    "credentialShowing",
    "credential_showing",
    "credentialShowingBytes",
    "credential_showing_bytes",
    "credentialShowingJson",
    "credential_showing_json",
    "showing",
    "showingBytes",
    "showing_bytes",
    "showingJson",
    "showing_json",
    "showingNullifier",
    "showing_nullifier",
    "credentialShowingNullifier",
    "credential_showing_nullifier",
    "nullifier",
    "verifierPolicyHash",
    "verifier_policy_hash",
    "verifierPolicy",
    "verifier_policy",
    "verifierPolicyBytes",
    "verifier_policy_bytes",
    "verifierPolicyJson",
    "verifier_policy_json",
    "domainSeparator",
    "domain_separator",
    "maxIssuerSetBytes",
    "max_issuer_set_bytes",
    "maxPolicyBytes",
    "max_policy_bytes",
    "maxShowingBytes",
    "max_showing_bytes",
}


def build_silent_threshold_credential_commitments(
    options: Mapping[str, Any],
) -> dict[str, Any]:
    """Normalize silent-threshold credential public-input commitments."""

    source = _require_mapping(options, "silentThresholdCredentialCommitments")
    _reject_unknown_fields(
        source,
        _COMMON_FIELDS,
        "silentThresholdCredentialCommitments",
    )
    parts = _commitment_parts(source, "silentThresholdCredentialCommitments")
    return {
        "version": parts["version"],
        "issuer_set_commitment": parts["issuer_set"]["value"],
        "threshold_policy_hash": parts["threshold_policy"]["value"],
        "credential_showing_commitment": parts["showing"]["value"],
        "showing_nullifier": parts["showing_nullifier"]["value"],
        "verifier_policy_hash": parts["verifier_policy"]["value"],
        "domain_separator": parts["domain_separator"],
        "commitment_kinds": {
            "issuer_set_commitment": parts["issuer_set"]["kind"],
            "threshold_policy_hash": parts["threshold_policy"]["kind"],
            "credential_showing_commitment": parts["showing"]["kind"],
            "showing_nullifier": parts["showing_nullifier"]["kind"],
            "verifier_policy_hash": parts["verifier_policy"]["kind"],
        },
        "source_digests": {
            "issuer_set": parts["issuer_set"]["digest"],
            "threshold_policy": parts["threshold_policy"]["digest"],
            "credential_showing": parts["showing"]["digest"],
            "showing_nullifier": parts["showing_nullifier"]["digest"],
            "verifier_policy": parts["verifier_policy"]["digest"],
        },
    }


def _normalize_public_inputs(value: Any, context: str) -> dict[str, Any]:
    source = _require_mapping(value, context)
    _reject_unknown_fields(
        source,
        {
            "version",
            "issuer_set_commitment",
            "issuerSetCommitment",
            "threshold_policy_hash",
            "thresholdPolicyHash",
            "credential_showing_commitment",
            "credentialShowingCommitment",
            "showing_nullifier",
            "showingNullifier",
            "credential_showing_nullifier",
            "credentialShowingNullifier",
            "verifier_policy_hash",
            "verifierPolicyHash",
            "domain_separator",
            "domainSeparator",
        },
        context,
    )
    _issuer_key, issuer_value = _read_single_alias(
        source,
        ("issuer_set_commitment", "issuerSetCommitment"),
        f"{context}.issuerSetCommitment",
        "issuer-set commitment",
    )
    _threshold_key, threshold_value = _read_single_alias(
        source,
        ("threshold_policy_hash", "thresholdPolicyHash"),
        f"{context}.thresholdPolicyHash",
        "threshold policy hash",
    )
    _showing_key, showing_value = _read_single_alias(
        source,
        ("credential_showing_commitment", "credentialShowingCommitment"),
        f"{context}.credentialShowingCommitment",
        "credential showing commitment",
    )
    _nullifier_key, nullifier_value = _read_single_alias(
        source,
        (
            "showing_nullifier",
            "showingNullifier",
            "credential_showing_nullifier",
            "credentialShowingNullifier",
        ),
        f"{context}.showingNullifier",
        "showing nullifier",
    )
    _verifier_key, verifier_value = _read_single_alias(
        source,
        ("verifier_policy_hash", "verifierPolicyHash"),
        f"{context}.verifierPolicyHash",
        "verifier policy hash",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domain_separator", "domainSeparator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "issuer_set_commitment": _fixed_bytes(
            issuer_value,
            f"{context}.issuerSetCommitment",
            32,
            nonzero=True,
        ).hex(),
        "threshold_policy_hash": _fixed_bytes(
            threshold_value,
            f"{context}.thresholdPolicyHash",
            32,
            nonzero=True,
        ).hex(),
        "credential_showing_commitment": _fixed_bytes(
            showing_value,
            f"{context}.credentialShowingCommitment",
            32,
            nonzero=True,
        ).hex(),
        "showing_nullifier": _fixed_bytes(
            nullifier_value,
            f"{context}.showingNullifier",
            32,
            nonzero=True,
        ).hex(),
        "verifier_policy_hash": _fixed_bytes(
            verifier_value,
            f"{context}.verifierPolicyHash",
            32,
            nonzero=True,
        ).hex(),
        "domain_separator": _require_non_blank_string(
            domain_value,
            f"{context}.domainSeparator",
        ),
    }


def _proof_parts(
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
    parts = _commitment_parts(source, context)
    public_inputs = {
        "version": parts["version"],
        "issuer_set_commitment": parts["issuer_set"]["value"].hex(),
        "threshold_policy_hash": parts["threshold_policy"]["value"].hex(),
        "credential_showing_commitment": parts["showing"]["value"].hex(),
        "showing_nullifier": parts["showing_nullifier"]["value"].hex(),
        "verifier_policy_hash": parts["verifier_policy"]["value"].hex(),
        "domain_separator": parts["domain_separator"],
    }
    max_proof_bytes = _positive_u32(
        source.get("maxProofBytes", source.get("max_proof_bytes", DEFAULT_PRIVACY_MAX_PROOF_BYTES)),
        f"{context}.maxProofBytes",
    )
    return {
        "backend": _normalize_backend_tag(backend_value, f"{context}.backendTag"),
        "circuit_id": _normalize_circuit_id(circuit_value, f"{context}.circuitId"),
        "vk_hash": _fixed_bytes(vk_hash_value, f"{context}.vkHash", 32, nonzero=True),
        "commitments": parts,
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


_ENVELOPE_FIELDS = {
    *_COMMON_FIELDS,
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
}


def build_silent_threshold_credential_envelope(options: Mapping[str, Any]) -> bytes:
    """Build canonical OpenVerifyEnvelope bytes for a prepared silent-threshold proof."""

    source = _require_mapping(options, "silentThresholdCredentialEnvelope")
    _reject_unknown_fields(source, _ENVELOPE_FIELDS, "silentThresholdCredentialEnvelope")
    parts = _proof_parts(
        source,
        "silentThresholdCredentialEnvelope",
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


def _dev_proof_bytes(
    *,
    circuit_id: str,
    vk_hash: bytes,
    public_input_bytes: bytes,
) -> bytes:
    digest = hashlib.sha256()
    digest.update(b"iroha:silent-threshold:dev-fixture:v0")
    digest.update(b"\x00")
    digest.update(circuit_id.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(vk_hash)
    digest.update(b"\x00")
    digest.update(public_input_bytes)
    return SILENT_THRESHOLD_DEV_PROOF_PREFIX + digest.digest()


def build_silent_threshold_credential_dev_proof_fixture(
    options: Mapping[str, Any],
) -> dict[str, Any]:
    """Build a deterministic silent-threshold dev proof fixture."""

    source = _require_mapping(options, "silentThresholdCredentialDevProofFixture")
    _reject_unknown_fields(
        source,
        _ENVELOPE_FIELDS - {"proofBytes", "proof_bytes", "proof"},
        "silentThresholdCredentialDevProofFixture",
    )
    parts = _proof_parts(
        source,
        "silentThresholdCredentialDevProofFixture",
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
    commitments = parts["commitments"]
    return {
        "kind": "silent-threshold-dev-fixture-v0",
        "production": False,
        "proof_bytes": proof_bytes,
        "proofBytes": proof_bytes,
        "commitments": {
            "issuer_set_commitment": commitments["issuer_set"]["value"],
            "threshold_policy_hash": commitments["threshold_policy"]["value"],
            "credential_showing_commitment": commitments["showing"]["value"],
            "showing_nullifier": commitments["showing_nullifier"]["value"],
            "verifier_policy_hash": commitments["verifier_policy"]["value"],
            "domain_separator": commitments["domain_separator"],
        },
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


def _expectation_source(
    source: Mapping[str, Any],
    public_inputs: Mapping[str, Any],
) -> dict[str, Any]:
    if any(key in source for key in ("domainSeparator", "domain_separator")):
        return dict(source)
    result = dict(source)
    result["domainSeparator"] = public_inputs["domain_separator"]
    return result


def _ensure_expectations(
    source: Mapping[str, Any],
    public_inputs: Mapping[str, Any],
    context: str,
) -> None:
    expectation_source = _expectation_source(source, public_inputs)
    checks = (
        (
            (
                "issuerSetCommitment",
                "issuer_set_commitment",
                "issuerSet",
                "issuer_set",
                "issuerSetBytes",
                "issuer_set_bytes",
                "issuerSetJson",
                "issuer_set_json",
            ),
            "issuerSetCommitment",
            lambda: _issuer_set_field(expectation_source, context)["value"].hex(),
            public_inputs["issuer_set_commitment"],
        ),
        (
            (
                "thresholdPolicyHash",
                "threshold_policy_hash",
                "thresholdPolicy",
                "threshold_policy",
                "thresholdPolicyBytes",
                "threshold_policy_bytes",
                "thresholdPolicyJson",
                "threshold_policy_json",
            ),
            "thresholdPolicyHash",
            lambda: _threshold_policy_field(expectation_source, context)["value"].hex(),
            public_inputs["threshold_policy_hash"],
        ),
        (
            (
                "credentialShowingCommitment",
                "credential_showing_commitment",
                "showingCommitment",
                "showing_commitment",
                "credentialShowing",
                "credential_showing",
                "credentialShowingBytes",
                "credential_showing_bytes",
                "credentialShowingJson",
                "credential_showing_json",
                "showing",
                "showingBytes",
                "showing_bytes",
                "showingJson",
                "showing_json",
            ),
            "credentialShowingCommitment",
            lambda: _showing_field(expectation_source, context)["value"].hex(),
            public_inputs["credential_showing_commitment"],
        ),
        (
            (
                "showingNullifier",
                "showing_nullifier",
                "credentialShowingNullifier",
                "credential_showing_nullifier",
                "nullifier",
                "credentialShowing",
                "credential_showing",
                "credentialShowingBytes",
                "credential_showing_bytes",
                "credentialShowingJson",
                "credential_showing_json",
                "showing",
                "showingBytes",
                "showing_bytes",
                "showingJson",
                "showing_json",
            ),
            "showingNullifier",
            lambda: _showing_nullifier_field(expectation_source, context)["value"].hex(),
            public_inputs["showing_nullifier"],
        ),
        (
            (
                "verifierPolicyHash",
                "verifier_policy_hash",
                "verifierPolicy",
                "verifier_policy",
                "verifierPolicyBytes",
                "verifier_policy_bytes",
                "verifierPolicyJson",
                "verifier_policy_json",
            ),
            "verifierPolicyHash",
            lambda: _verifier_policy_field(expectation_source, context)["value"].hex(),
            public_inputs["verifier_policy_hash"],
        ),
    )
    for fields, path, normalize, actual in checks:
        if any(key in source for key in fields) and normalize() != actual:
            raise ValueError(f"{context}.{path} must match the envelope public inputs")
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    if domain_value is not _MISSING:
        if (
            _require_non_blank_string(domain_value, f"{context}.domainSeparator")
            != public_inputs["domain_separator"]
        ):
            raise ValueError(
                f"{context}.domainSeparator must match the envelope public inputs"
            )


def verify_silent_threshold_credential_proof_locally(options: Any) -> dict[str, Any]:
    """Verify a deterministic silent-threshold dev fixture."""

    if isinstance(options, Mapping):
        source = options
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        _COMMON_FIELDS | {"envelope", "proofEnvelope", "proof_envelope", "bytes"},
        "silentThresholdCredentialLocalVerification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "silentThresholdCredentialLocalVerification.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError(
            "silentThresholdCredentialLocalVerification.envelope.backend must be Stark"
        )
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "silentThresholdCredentialLocalVerification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "silentThresholdCredentialLocalVerification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "silentThresholdCredentialLocalVerification.publicInputs",
    )
    _ensure_expectations(
        source,
        public_inputs,
        "silentThresholdCredentialLocalVerification",
    )
    expected_proof = _dev_proof_bytes(
        circuit_id=circuit_id,
        vk_hash=vk_hash,
        public_input_bytes=decoded["public_inputs"],
    )
    if decoded["proof_bytes"] != expected_proof:
        raise ValueError(
            "silentThresholdCredentialLocalVerification proof bytes are not a valid silent-threshold dev fixture"
        )
    return {
        "ok": True,
        "production": False,
        "kind": "silent-threshold-dev-fixture-v0",
        "backend": SILENT_THRESHOLD_BACKEND,
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "showing_nullifier": public_inputs["showing_nullifier"],
    }


buildSilentThresholdCredentialCommitments = build_silent_threshold_credential_commitments
buildSilentThresholdCredentialEnvelope = build_silent_threshold_credential_envelope
buildSilentThresholdCredentialDevProofFixture = (
    build_silent_threshold_credential_dev_proof_fixture
)
verifySilentThresholdCredentialProofLocally = (
    verify_silent_threshold_credential_proof_locally
)
