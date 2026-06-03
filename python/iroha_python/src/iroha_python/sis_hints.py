"""SIS-with-hints anonymous credential SDK dev-fixture helpers."""

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

SIS_HINTS_BACKEND = "unsupported"
SIS_HINTS_CIRCUIT_ID = "lattice/sis-hints-anoncred-v0:sis_hints_anoncred_pq_v0"
SIS_HINTS_DOMAIN_SEPARATOR = "iroha:sis-hints:anoncred:v0"
SIS_HINTS_DEV_PROOF_PREFIX = b"iroha:sis-hints:dev-fixture:v0:"
SIS_HINTS_MAX_ISSUER_BYTES = 1024 * 1024
SIS_HINTS_MAX_CREDENTIAL_BYTES = 1024 * 1024
SIS_HINTS_MAX_POLICY_BYTES = 1024 * 1024
SIS_HINTS_MAX_PARAMETER_BYTES = 1024 * 1024

__all__ = [
    "SIS_HINTS_BACKEND",
    "SIS_HINTS_CIRCUIT_ID",
    "SIS_HINTS_DOMAIN_SEPARATOR",
    "build_sis_hints_credential_commitments",
    "build_sis_hints_credential_envelope",
    "build_sis_hints_credential_dev_proof_fixture",
    "verify_sis_hints_credential_proof_locally",
    "buildSisHintsCredentialCommitments",
    "buildSisHintsCredentialEnvelope",
    "buildSisHintsCredentialDevProofFixture",
    "verifySisHintsCredentialProofLocally",
]


def _normalize_version(value: Any, context: str) -> int:
    version = _positive_u32(1 if value is _MISSING or value is None else value, context)
    if version != 1:
        raise ValueError(f"{context} must be 1")
    return version


def _normalize_backend_tag(value: Any, context: str) -> str:
    _tag, decoded = _normalize_backend(
        SIS_HINTS_BACKEND if value is _MISSING or value is None else value,
        context,
    )
    if decoded != "Unsupported":
        raise ValueError(
            f"{context} must remain unsupported until a production SIS-with-hints backend is registered"
        )
    return SIS_HINTS_BACKEND


def _normalize_circuit_id(value: Any, context: str) -> str:
    circuit_id = _require_non_blank_string(
        SIS_HINTS_CIRCUIT_ID if value is _MISSING or value is None else value,
        context,
    )
    if circuit_id not in {
        SIS_HINTS_CIRCUIT_ID,
        "sis_hints_anoncred_pq_v0",
    }:
        raise ValueError(f"{context} must identify sis_hints_anoncred_pq_v0")
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
    digest.update(f"iroha:sis-hints:{label}:v0".encode("utf-8"))
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
        SIS_HINTS_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
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


def _issuer_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxIssuerBytes",
            source.get("max_issuer_bytes", SIS_HINTS_MAX_ISSUER_BYTES),
        ),
        f"{context}.maxIssuerBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=(
            "issuerCommitment",
            "issuer_commitment",
            "issuerParameterCommitment",
            "issuer_parameter_commitment",
        ),
        data_aliases=(
            "issuer",
            "issuerBytes",
            "issuer_bytes",
            "issuerJson",
            "issuer_json",
            "issuerParameters",
            "issuer_parameters",
            "issuerParametersJson",
            "issuer_parameters_json",
        ),
        json_aliases={"issuer", "issuerParameters", "issuer_parameters"},
        field_path="issuerCommitment",
        data_path="issuer",
        data_json_path="issuerJson",
        data_description="issuer commitment",
        digest_label="issuer-commitment",
        kind_label="issuer-digest",
        max_bytes=max_bytes,
    )


def _credential_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxCredentialBytes",
            source.get("max_credential_bytes", SIS_HINTS_MAX_CREDENTIAL_BYTES),
        ),
        f"{context}.maxCredentialBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=(
            "credentialCommitment",
            "credential_commitment",
            "credentialShowingCommitment",
            "credential_showing_commitment",
        ),
        data_aliases=(
            "credential",
            "credentialBytes",
            "credential_bytes",
            "credentialJson",
            "credential_json",
            "credentialShowing",
            "credential_showing",
            "credentialShowingJson",
            "credential_showing_json",
            "showing",
            "showingJson",
            "showing_json",
        ),
        json_aliases={
            "credential",
            "credentialShowing",
            "credential_showing",
            "showing",
        },
        field_path="credentialCommitment",
        data_path="credential",
        data_json_path="credentialJson",
        data_description="credential commitment",
        digest_label="credential-commitment",
        kind_label="credential-digest",
        max_bytes=max_bytes,
    )


def _showing_policy_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxPolicyBytes",
            source.get("max_policy_bytes", SIS_HINTS_MAX_POLICY_BYTES),
        ),
        f"{context}.maxPolicyBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=(
            "showingPolicyHash",
            "showing_policy_hash",
            "policyHash",
            "policy_hash",
            "verifierPolicyHash",
            "verifier_policy_hash",
        ),
        data_aliases=(
            "showingPolicy",
            "showing_policy",
            "showingPolicyBytes",
            "showing_policy_bytes",
            "showingPolicyJson",
            "showing_policy_json",
            "policy",
            "policyBytes",
            "policy_bytes",
            "policyJson",
            "policy_json",
            "verifierPolicy",
            "verifier_policy",
            "verifierPolicyJson",
            "verifier_policy_json",
        ),
        json_aliases={
            "showingPolicy",
            "showing_policy",
            "policy",
            "verifierPolicy",
            "verifier_policy",
        },
        field_path="showingPolicyHash",
        data_path="showingPolicy",
        data_json_path="showingPolicyJson",
        data_description="showing policy hash",
        digest_label="showing-policy-hash",
        kind_label="showing-policy-hash",
        max_bytes=max_bytes,
    )


def _parameter_hash_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxParameterBytes",
            source.get("max_parameter_bytes", SIS_HINTS_MAX_PARAMETER_BYTES),
        ),
        f"{context}.maxParameterBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=(
            "parameterHash",
            "parameter_hash",
            "paramsHash",
            "params_hash",
        ),
        data_aliases=(
            "parameters",
            "parametersBytes",
            "parameters_bytes",
            "parametersJson",
            "parameters_json",
            "parameterSet",
            "parameter_set",
            "parameterSetJson",
            "parameter_set_json",
            "sisParameters",
            "sis_parameters",
            "sisParametersJson",
            "sis_parameters_json",
            "params",
            "paramsJson",
            "params_json",
        ),
        json_aliases={
            "parameters",
            "parameterSet",
            "parameter_set",
            "sisParameters",
            "sis_parameters",
            "params",
        },
        field_path="parameterHash",
        data_path="parameters",
        data_json_path="parametersJson",
        data_description="parameter hash",
        digest_label="parameter-hash",
        kind_label="parameter-hash",
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
        SIS_HINTS_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    normalized = _source_with_domain(source, domain_separator)
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "issuer": _issuer_field(normalized, context),
        "credential": _credential_field(normalized, context),
        "showing_policy": _showing_policy_field(normalized, context),
        "parameter_hash": _parameter_hash_field(normalized, context),
        "domain_separator": domain_separator,
    }


_COMMON_FIELDS = {
    "version",
    "issuerCommitment",
    "issuer_commitment",
    "issuerParameterCommitment",
    "issuer_parameter_commitment",
    "issuer",
    "issuerBytes",
    "issuer_bytes",
    "issuerJson",
    "issuer_json",
    "issuerParameters",
    "issuer_parameters",
    "issuerParametersJson",
    "issuer_parameters_json",
    "credentialCommitment",
    "credential_commitment",
    "credentialShowingCommitment",
    "credential_showing_commitment",
    "credential",
    "credentialBytes",
    "credential_bytes",
    "credentialJson",
    "credential_json",
    "credentialShowing",
    "credential_showing",
    "credentialShowingJson",
    "credential_showing_json",
    "showing",
    "showingJson",
    "showing_json",
    "showingPolicyHash",
    "showing_policy_hash",
    "policyHash",
    "policy_hash",
    "verifierPolicyHash",
    "verifier_policy_hash",
    "showingPolicy",
    "showing_policy",
    "showingPolicyBytes",
    "showing_policy_bytes",
    "showingPolicyJson",
    "showing_policy_json",
    "policy",
    "policyBytes",
    "policy_bytes",
    "policyJson",
    "policy_json",
    "verifierPolicy",
    "verifier_policy",
    "verifierPolicyJson",
    "verifier_policy_json",
    "parameterHash",
    "parameter_hash",
    "paramsHash",
    "params_hash",
    "parameters",
    "parametersBytes",
    "parameters_bytes",
    "parametersJson",
    "parameters_json",
    "parameterSet",
    "parameter_set",
    "parameterSetJson",
    "parameter_set_json",
    "sisParameters",
    "sis_parameters",
    "sisParametersJson",
    "sis_parameters_json",
    "params",
    "paramsJson",
    "params_json",
    "domainSeparator",
    "domain_separator",
    "maxIssuerBytes",
    "max_issuer_bytes",
    "maxCredentialBytes",
    "max_credential_bytes",
    "maxPolicyBytes",
    "max_policy_bytes",
    "maxParameterBytes",
    "max_parameter_bytes",
}


def build_sis_hints_credential_commitments(options: Mapping[str, Any]) -> dict[str, Any]:
    """Normalize SIS-with-hints anonymous credential public-input commitments."""

    source = _require_mapping(options, "sisHintsCredentialCommitments")
    _reject_unknown_fields(source, _COMMON_FIELDS, "sisHintsCredentialCommitments")
    parts = _commitment_parts(source, "sisHintsCredentialCommitments")
    return {
        "version": parts["version"],
        "issuer_commitment": parts["issuer"]["value"],
        "credential_commitment": parts["credential"]["value"],
        "showing_policy_hash": parts["showing_policy"]["value"],
        "parameter_hash": parts["parameter_hash"]["value"],
        "domain_separator": parts["domain_separator"],
        "commitment_kinds": {
            "issuer_commitment": parts["issuer"]["kind"],
            "credential_commitment": parts["credential"]["kind"],
            "showing_policy_hash": parts["showing_policy"]["kind"],
            "parameter_hash": parts["parameter_hash"]["kind"],
        },
        "source_digests": {
            "issuer": parts["issuer"]["digest"],
            "credential": parts["credential"]["digest"],
            "showing_policy": parts["showing_policy"]["digest"],
            "parameters": parts["parameter_hash"]["digest"],
        },
    }


def _normalize_public_inputs(value: Any, context: str) -> dict[str, Any]:
    source = _require_mapping(value, context)
    _reject_unknown_fields(
        source,
        {
            "version",
            "issuer_commitment",
            "issuerCommitment",
            "credential_commitment",
            "credentialCommitment",
            "showing_policy_hash",
            "showingPolicyHash",
            "parameter_hash",
            "parameterHash",
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
    _credential_key, credential_value = _read_single_alias(
        source,
        ("credential_commitment", "credentialCommitment"),
        f"{context}.credentialCommitment",
        "credential commitment",
    )
    _policy_key, policy_value = _read_single_alias(
        source,
        ("showing_policy_hash", "showingPolicyHash"),
        f"{context}.showingPolicyHash",
        "showing policy hash",
    )
    _parameter_key, parameter_value = _read_single_alias(
        source,
        ("parameter_hash", "parameterHash"),
        f"{context}.parameterHash",
        "parameter hash",
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
        "credential_commitment": _fixed_bytes(
            credential_value,
            f"{context}.credentialCommitment",
            32,
            nonzero=True,
        ).hex(),
        "showing_policy_hash": _fixed_bytes(
            policy_value,
            f"{context}.showingPolicyHash",
            32,
            nonzero=True,
        ).hex(),
        "parameter_hash": _fixed_bytes(
            parameter_value,
            f"{context}.parameterHash",
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
        "issuer_commitment": parts["issuer"]["value"].hex(),
        "credential_commitment": parts["credential"]["value"].hex(),
        "showing_policy_hash": parts["showing_policy"]["value"].hex(),
        "parameter_hash": parts["parameter_hash"]["value"].hex(),
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


def build_sis_hints_credential_envelope(options: Mapping[str, Any]) -> bytes:
    """Build canonical OpenVerifyEnvelope bytes for a prepared SIS-with-hints proof."""

    source = _require_mapping(options, "sisHintsCredentialEnvelope")
    _reject_unknown_fields(source, _ENVELOPE_FIELDS, "sisHintsCredentialEnvelope")
    parts = _proof_parts(source, "sisHintsCredentialEnvelope", require_proof_bytes=True)
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
    digest.update(b"iroha:sis-hints:dev-fixture:v0")
    digest.update(b"\x00")
    digest.update(circuit_id.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(vk_hash)
    digest.update(b"\x00")
    digest.update(public_input_bytes)
    return SIS_HINTS_DEV_PROOF_PREFIX + digest.digest()


def build_sis_hints_credential_dev_proof_fixture(
    options: Mapping[str, Any],
) -> dict[str, Any]:
    """Build a deterministic SIS-with-hints credential dev proof fixture."""

    source = _require_mapping(options, "sisHintsCredentialDevProofFixture")
    _reject_unknown_fields(
        source,
        _ENVELOPE_FIELDS - {"proofBytes", "proof_bytes", "proof"},
        "sisHintsCredentialDevProofFixture",
    )
    parts = _proof_parts(
        source,
        "sisHintsCredentialDevProofFixture",
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
        "kind": "sis-hints-dev-fixture-v0",
        "production": False,
        "proof_bytes": proof_bytes,
        "proofBytes": proof_bytes,
        "commitments": {
            "issuer_commitment": commitments["issuer"]["value"],
            "credential_commitment": commitments["credential"]["value"],
            "showing_policy_hash": commitments["showing_policy"]["value"],
            "parameter_hash": commitments["parameter_hash"]["value"],
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
                "issuerCommitment",
                "issuer_commitment",
                "issuerParameterCommitment",
                "issuer_parameter_commitment",
                "issuer",
                "issuerBytes",
                "issuer_bytes",
                "issuerJson",
                "issuer_json",
                "issuerParameters",
                "issuer_parameters",
                "issuerParametersJson",
                "issuer_parameters_json",
            ),
            "issuerCommitment",
            lambda: _issuer_field(expectation_source, context)["value"].hex(),
            public_inputs["issuer_commitment"],
        ),
        (
            (
                "credentialCommitment",
                "credential_commitment",
                "credentialShowingCommitment",
                "credential_showing_commitment",
                "credential",
                "credentialBytes",
                "credential_bytes",
                "credentialJson",
                "credential_json",
                "credentialShowing",
                "credential_showing",
                "credentialShowingJson",
                "credential_showing_json",
                "showing",
                "showingJson",
                "showing_json",
            ),
            "credentialCommitment",
            lambda: _credential_field(expectation_source, context)["value"].hex(),
            public_inputs["credential_commitment"],
        ),
        (
            (
                "showingPolicyHash",
                "showing_policy_hash",
                "policyHash",
                "policy_hash",
                "verifierPolicyHash",
                "verifier_policy_hash",
                "showingPolicy",
                "showing_policy",
                "showingPolicyBytes",
                "showing_policy_bytes",
                "showingPolicyJson",
                "showing_policy_json",
                "policy",
                "policyBytes",
                "policy_bytes",
                "policyJson",
                "policy_json",
                "verifierPolicy",
                "verifier_policy",
                "verifierPolicyJson",
                "verifier_policy_json",
            ),
            "showingPolicyHash",
            lambda: _showing_policy_field(expectation_source, context)["value"].hex(),
            public_inputs["showing_policy_hash"],
        ),
        (
            (
                "parameterHash",
                "parameter_hash",
                "paramsHash",
                "params_hash",
                "parameters",
                "parametersBytes",
                "parameters_bytes",
                "parametersJson",
                "parameters_json",
                "parameterSet",
                "parameter_set",
                "parameterSetJson",
                "parameter_set_json",
                "sisParameters",
                "sis_parameters",
                "sisParametersJson",
                "sis_parameters_json",
                "params",
                "paramsJson",
                "params_json",
            ),
            "parameterHash",
            lambda: _parameter_hash_field(expectation_source, context)["value"].hex(),
            public_inputs["parameter_hash"],
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


def verify_sis_hints_credential_proof_locally(options: Any) -> dict[str, Any]:
    """Verify a deterministic SIS-with-hints credential dev fixture."""

    if isinstance(options, Mapping):
        source = options
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        _COMMON_FIELDS | {"envelope", "proofEnvelope", "proof_envelope", "bytes"},
        "sisHintsCredentialLocalVerification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "sisHintsCredentialLocalVerification.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Unsupported":
        raise ValueError(
            "sisHintsCredentialLocalVerification.envelope.backend must be Unsupported until a production SIS-with-hints backend is registered"
        )
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "sisHintsCredentialLocalVerification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "sisHintsCredentialLocalVerification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "sisHintsCredentialLocalVerification.publicInputs",
    )
    _ensure_expectations(source, public_inputs, "sisHintsCredentialLocalVerification")
    expected_proof = _dev_proof_bytes(
        circuit_id=circuit_id,
        vk_hash=vk_hash,
        public_input_bytes=decoded["public_inputs"],
    )
    if decoded["proof_bytes"] != expected_proof:
        raise ValueError(
            "sisHintsCredentialLocalVerification proof bytes are not a valid SIS-with-hints dev fixture"
        )
    return {
        "ok": True,
        "production": False,
        "kind": "sis-hints-dev-fixture-v0",
        "backend": SIS_HINTS_BACKEND,
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "parameter_hash": public_inputs["parameter_hash"],
    }


buildSisHintsCredentialCommitments = build_sis_hints_credential_commitments
buildSisHintsCredentialEnvelope = build_sis_hints_credential_envelope
buildSisHintsCredentialDevProofFixture = build_sis_hints_credential_dev_proof_fixture
verifySisHintsCredentialProofLocally = verify_sis_hints_credential_proof_locally
