"""Jindo lattice PCS SDK helpers."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from typing import Any

from .verange import (
    DEFAULT_PRIVACY_MAX_PROOF_BYTES,
    DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES,
    _MISSING,
    _build_privacy_proof_envelope_internal,
    _bounded_bytes,
    _canonical_json_bytes,
    _decode_privacy_proof_envelope_internal,
    _fixed_bytes,
    _normalize_backend,
    _normalize_backend_allowing_unsupported,
    _positive_u32,
    _read_single_alias,
    _reject_unknown_fields,
    _require_mapping,
    _require_non_blank_string,
)

JINDO_BACKEND = "unsupported"
JINDO_PRODUCTION_BACKEND = "lattice-pcs-sis"
JINDO_CIRCUIT_ID = "lattice/jindo-pcs-v0:jindo_lattice_pcs_zk_v0"
JINDO_DOMAIN_SEPARATOR = "iroha:jindo:lattice-pcs:v0"
JINDO_DEV_PROOF_PREFIX = b"iroha:jindo:dev-fixture:v0:"
JINDO_MAX_POLYNOMIAL_BYTES = 16 * 1024 * 1024
JINDO_MAX_OPENING_CLAIM_BYTES = 1024 * 1024
JINDO_MAX_QUERY_SET_BYTES = 1024 * 1024
JINDO_MAX_PARAMETER_BYTES = 1024 * 1024

__all__ = [
    "JINDO_BACKEND",
    "JINDO_PRODUCTION_BACKEND",
    "JINDO_CIRCUIT_ID",
    "JINDO_DOMAIN_SEPARATOR",
    "build_jindo_lattice_public_inputs",
    "build_jindo_lattice_proof_envelope",
    "build_jindo_lattice_proof_v0",
    "build_jindo_lattice_dev_proof_fixture",
    "verify_jindo_polynomial_commitment_v0",
    "verify_jindo_lattice_proof_locally",
    "buildJindoLatticePublicInputs",
    "buildJindoLatticeProofEnvelope",
    "buildJindoLatticeProofV0",
    "buildJindoLatticeDevProofFixture",
    "verifyJindoPolynomialCommitmentV0",
    "verifyJindoLatticeProofLocally",
]


def _normalize_version(value: Any, context: str) -> int:
    version = _positive_u32(1 if value is _MISSING or value is None else value, context)
    if version != 1:
        raise ValueError(f"{context} must be 1")
    return version


def _normalize_backend_tag(value: Any, context: str) -> str:
    _tag, decoded = _normalize_backend_allowing_unsupported(
        JINDO_BACKEND if value is _MISSING or value is None else value,
        context,
    )
    if decoded != "Unsupported":
        raise ValueError(
            f"{context} must remain unsupported until a production Jindo backend is registered"
        )
    return JINDO_BACKEND


def _normalize_production_backend_tag(value: Any, context: str) -> str:
    _tag, decoded = _normalize_backend(
        JINDO_PRODUCTION_BACKEND if value is _MISSING or value is None else value,
        context,
    )
    if decoded != "LatticePcsSis":
        raise ValueError(f"{context} must identify the production Jindo LatticePcsSis backend")
    return JINDO_PRODUCTION_BACKEND


def _normalize_circuit_id(value: Any, context: str) -> str:
    circuit_id = _require_non_blank_string(
        JINDO_CIRCUIT_ID if value is _MISSING or value is None else value,
        context,
    )
    if circuit_id not in {
        JINDO_CIRCUIT_ID,
        "jindo_lattice_pcs_zk_v0",
    }:
        raise ValueError(f"{context} must identify jindo_lattice_pcs_zk_v0")
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
    digest.update(f"iroha:jindo:{label}:v0".encode("utf-8"))
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
        JINDO_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
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


def _commitment_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxPolynomialBytes",
            source.get("max_polynomial_bytes", JINDO_MAX_POLYNOMIAL_BYTES),
        ),
        f"{context}.maxPolynomialBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=(
            "commitment",
            "polynomialCommitment",
            "polynomial_commitment",
        ),
        data_aliases=(
            "polynomial",
            "polynomialBytes",
            "polynomial_bytes",
            "polynomialJson",
            "polynomial_json",
            "commitmentMaterial",
            "commitment_material",
            "commitmentMaterialJson",
            "commitment_material_json",
        ),
        json_aliases={"polynomial", "commitmentMaterial", "commitment_material"},
        field_path="commitment",
        data_path="polynomial",
        data_json_path="polynomialJson",
        data_description="polynomial commitment",
        digest_label="commitment",
        kind_label="commitment-digest",
        max_bytes=max_bytes,
    )


def _opening_claim_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxOpeningClaimBytes",
            source.get("max_opening_claim_bytes", JINDO_MAX_OPENING_CLAIM_BYTES),
        ),
        f"{context}.maxOpeningClaimBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=(
            "openingClaimCommitment",
            "opening_claim_commitment",
            "openingClaimHash",
            "opening_claim_hash",
            "openingClaimDigest",
            "opening_claim_digest",
            "opening_claim",
            "openingClaim",
        ),
        data_aliases=(
            "claim",
            "claimBytes",
            "claim_bytes",
            "claimJson",
            "claim_json",
            "openingClaimBytes",
            "opening_claim_bytes",
            "openingClaimJson",
            "opening_claim_json",
            "evaluationClaim",
            "evaluation_claim",
            "evaluationClaimJson",
            "evaluation_claim_json",
        ),
        json_aliases={"claim", "evaluationClaim", "evaluation_claim"},
        field_path="openingClaim",
        data_path="openingClaim",
        data_json_path="openingClaimJson",
        data_description="opening claim",
        digest_label="opening-claim",
        kind_label="opening-claim-digest",
        max_bytes=max_bytes,
    )


def _query_set_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxQuerySetBytes",
            source.get("max_query_set_bytes", JINDO_MAX_QUERY_SET_BYTES),
        ),
        f"{context}.maxQuerySetBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=(
            "querySetHash",
            "query_set_hash",
            "querySetRoot",
            "query_set_root",
        ),
        data_aliases=(
            "querySet",
            "query_set",
            "querySetBytes",
            "query_set_bytes",
            "querySetJson",
            "query_set_json",
            "queries",
            "queriesJson",
            "queries_json",
        ),
        json_aliases={"querySet", "query_set", "queries"},
        field_path="querySet",
        data_path="querySet",
        data_json_path="querySetJson",
        data_description="query set",
        digest_label="query-set",
        kind_label="query-set-digest",
        max_bytes=max_bytes,
    )


def _parameter_hash_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxParameterBytes",
            source.get("max_parameter_bytes", JINDO_MAX_PARAMETER_BYTES),
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
            "params",
            "paramsBytes",
            "params_bytes",
            "paramsJson",
            "params_json",
        ),
        json_aliases={"parameters", "parameterSet", "parameter_set", "params"},
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


def _public_input_parts(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    domain_separator = _require_non_blank_string(
        JINDO_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    normalized = _source_with_domain(source, domain_separator)
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "commitment": _commitment_field(normalized, context),
        "opening_claim": _opening_claim_field(normalized, context),
        "query_set": _query_set_field(normalized, context),
        "parameter_hash": _parameter_hash_field(normalized, context),
        "domain_separator": domain_separator,
    }


_COMMON_FIELDS = {
    "version",
    "commitment",
    "polynomialCommitment",
    "polynomial_commitment",
    "polynomial",
    "polynomialBytes",
    "polynomial_bytes",
    "polynomialJson",
    "polynomial_json",
    "commitmentMaterial",
    "commitment_material",
    "commitmentMaterialJson",
    "commitment_material_json",
    "openingClaimCommitment",
    "opening_claim_commitment",
    "openingClaimHash",
    "opening_claim_hash",
    "openingClaimDigest",
    "opening_claim_digest",
    "opening_claim",
    "openingClaim",
    "claim",
    "claimBytes",
    "claim_bytes",
    "claimJson",
    "claim_json",
    "openingClaimBytes",
    "opening_claim_bytes",
    "openingClaimJson",
    "opening_claim_json",
    "evaluationClaim",
    "evaluation_claim",
    "evaluationClaimJson",
    "evaluation_claim_json",
    "querySetHash",
    "query_set_hash",
    "querySetRoot",
    "query_set_root",
    "querySet",
    "query_set",
    "querySetBytes",
    "query_set_bytes",
    "querySetJson",
    "query_set_json",
    "queries",
    "queriesJson",
    "queries_json",
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
    "params",
    "paramsBytes",
    "params_bytes",
    "paramsJson",
    "params_json",
    "domainSeparator",
    "domain_separator",
    "maxPolynomialBytes",
    "max_polynomial_bytes",
    "maxOpeningClaimBytes",
    "max_opening_claim_bytes",
    "maxQuerySetBytes",
    "max_query_set_bytes",
    "maxParameterBytes",
    "max_parameter_bytes",
}


def build_jindo_lattice_public_inputs(options: Mapping[str, Any]) -> dict[str, Any]:
    """Normalize Jindo lattice PCS public inputs for SDK/dev-fixture use."""

    source = _require_mapping(options, "jindoLatticePublicInputs")
    _reject_unknown_fields(source, _COMMON_FIELDS, "jindoLatticePublicInputs")
    parts = _public_input_parts(source, "jindoLatticePublicInputs")
    return {
        "version": parts["version"],
        "commitment": parts["commitment"]["value"],
        "opening_claim": parts["opening_claim"]["value"],
        "query_set": parts["query_set"]["value"],
        "parameter_hash": parts["parameter_hash"]["value"],
        "domain_separator": parts["domain_separator"],
        "commitment_kinds": {
            "commitment": parts["commitment"]["kind"],
            "opening_claim": parts["opening_claim"]["kind"],
            "query_set": parts["query_set"]["kind"],
            "parameter_hash": parts["parameter_hash"]["kind"],
        },
        "source_digests": {
            "polynomial": parts["commitment"]["digest"],
            "opening_claim": parts["opening_claim"]["digest"],
            "query_set": parts["query_set"]["digest"],
            "parameters": parts["parameter_hash"]["digest"],
        },
    }


def _normalize_public_inputs(value: Any, context: str) -> dict[str, Any]:
    source = _require_mapping(value, context)
    _reject_unknown_fields(
        source,
        {
            "version",
            "commitment",
            "opening_claim",
            "openingClaim",
            "query_set",
            "querySet",
            "parameter_hash",
            "parameterHash",
            "domain_separator",
            "domainSeparator",
        },
        context,
    )
    _commitment_key, commitment_value = _read_single_alias(
        source,
        ("commitment",),
        f"{context}.commitment",
        "commitment",
    )
    _opening_key, opening_value = _read_single_alias(
        source,
        ("opening_claim", "openingClaim"),
        f"{context}.openingClaim",
        "opening claim",
    )
    _query_key, query_value = _read_single_alias(
        source,
        ("query_set", "querySet"),
        f"{context}.querySet",
        "query set",
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
        "commitment": _fixed_bytes(
            commitment_value,
            f"{context}.commitment",
            32,
            nonzero=True,
        ).hex(),
        "opening_claim": _fixed_bytes(
            opening_value,
            f"{context}.openingClaim",
            32,
            nonzero=True,
        ).hex(),
        "query_set": _fixed_bytes(
            query_value,
            f"{context}.querySet",
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
    production_backend: bool = False,
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
    parts = _public_input_parts(source, context)
    public_inputs = {
        "version": parts["version"],
        "commitment": parts["commitment"]["value"].hex(),
        "opening_claim": parts["opening_claim"]["value"].hex(),
        "query_set": parts["query_set"]["value"].hex(),
        "parameter_hash": parts["parameter_hash"]["value"].hex(),
        "domain_separator": parts["domain_separator"],
    }
    max_proof_bytes = _positive_u32(
        source.get("maxProofBytes", source.get("max_proof_bytes", DEFAULT_PRIVACY_MAX_PROOF_BYTES)),
        f"{context}.maxProofBytes",
    )
    return {
        "backend": (
            _normalize_production_backend_tag(backend_value, f"{context}.backendTag")
            if production_backend
            else _normalize_backend_tag(backend_value, f"{context}.backendTag")
        ),
        "circuit_id": _normalize_circuit_id(circuit_value, f"{context}.circuitId"),
        "vk_hash": _fixed_bytes(vk_hash_value, f"{context}.vkHash", 32, nonzero=True),
        "inputs": parts,
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


def build_jindo_lattice_proof_envelope(options: Mapping[str, Any]) -> bytes:
    """Build canonical OpenVerifyEnvelope bytes for a prepared Jindo proof."""

    source = _require_mapping(options, "jindoLatticeProofEnvelope")
    _reject_unknown_fields(source, _ENVELOPE_FIELDS, "jindoLatticeProofEnvelope")
    parts = _proof_parts(source, "jindoLatticeProofEnvelope", require_proof_bytes=True)
    return _build_privacy_proof_envelope_internal(
        {
            "backend": parts["backend"],
            "circuitId": parts["circuit_id"],
            "vkHash": parts["vk_hash"],
            "publicInputs": parts["public_input_bytes"],
            "proofBytes": parts["proof_bytes"],
            "aux": source.get("aux", b""),
            "maxProofBytes": parts["max_proof_bytes"],
            "maxPublicInputBytes": parts["max_public_input_bytes"],
        },
        allow_unsupported_backend=True,
    )


def build_jindo_lattice_proof_v0(options: Mapping[str, Any]) -> bytes:
    """Build canonical production Jindo lattice PCS proof envelope bytes."""

    source = _require_mapping(options, "jindoLatticeProofV0")
    _reject_unknown_fields(source, _ENVELOPE_FIELDS, "jindoLatticeProofV0")
    parts = _proof_parts(
        source,
        "jindoLatticeProofV0",
        require_proof_bytes=True,
        production_backend=True,
    )
    if parts["proof_bytes"].startswith(JINDO_DEV_PROOF_PREFIX):
        raise ValueError("jindoLatticeProofV0.proofBytes must not contain a dev fixture proof")
    return _build_privacy_proof_envelope_internal(
        {
            "backend": parts["backend"],
            "circuitId": parts["circuit_id"],
            "vkHash": parts["vk_hash"],
            "publicInputs": parts["public_input_bytes"],
            "proofBytes": parts["proof_bytes"],
            "aux": source.get("aux", b""),
            "maxProofBytes": parts["max_proof_bytes"],
            "maxPublicInputBytes": parts["max_public_input_bytes"],
        },
    )


def _dev_proof_bytes(
    *,
    circuit_id: str,
    vk_hash: bytes,
    public_input_bytes: bytes,
) -> bytes:
    digest = hashlib.sha256()
    digest.update(b"iroha:jindo:dev-fixture:v0")
    digest.update(b"\x00")
    digest.update(circuit_id.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(vk_hash)
    digest.update(b"\x00")
    digest.update(public_input_bytes)
    return JINDO_DEV_PROOF_PREFIX + digest.digest()


def build_jindo_lattice_dev_proof_fixture(options: Mapping[str, Any]) -> dict[str, Any]:
    """Build a deterministic Jindo lattice PCS dev proof fixture."""

    source = _require_mapping(options, "jindoLatticeDevProofFixture")
    _reject_unknown_fields(
        source,
        _ENVELOPE_FIELDS - {"proofBytes", "proof_bytes", "proof"},
        "jindoLatticeDevProofFixture",
    )
    parts = _proof_parts(
        source,
        "jindoLatticeDevProofFixture",
        require_proof_bytes=False,
    )
    proof_bytes = _dev_proof_bytes(
        circuit_id=parts["circuit_id"],
        vk_hash=parts["vk_hash"],
        public_input_bytes=parts["public_input_bytes"],
    )
    envelope = _build_privacy_proof_envelope_internal(
        {
            "backend": parts["backend"],
            "circuitId": parts["circuit_id"],
            "vkHash": parts["vk_hash"],
            "publicInputs": parts["public_input_bytes"],
            "proofBytes": proof_bytes,
            "aux": source.get("aux", b""),
            "maxProofBytes": parts["max_proof_bytes"],
            "maxPublicInputBytes": parts["max_public_input_bytes"],
        },
        allow_unsupported_backend=True,
    )
    return {
        "kind": "jindo-lattice-dev-fixture-v0",
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
                "commitment",
                "polynomialCommitment",
                "polynomial_commitment",
                "polynomial",
                "polynomialBytes",
                "polynomial_bytes",
                "polynomialJson",
                "polynomial_json",
                "commitmentMaterial",
                "commitment_material",
                "commitmentMaterialJson",
                "commitment_material_json",
            ),
            "commitment",
            lambda: _commitment_field(expectation_source, context)["value"].hex(),
            public_inputs["commitment"],
        ),
        (
            (
                "openingClaimCommitment",
                "opening_claim_commitment",
                "openingClaimHash",
                "opening_claim_hash",
                "openingClaimDigest",
                "opening_claim_digest",
                "opening_claim",
                "openingClaim",
                "claim",
                "claimBytes",
                "claim_bytes",
                "claimJson",
                "claim_json",
                "openingClaimBytes",
                "opening_claim_bytes",
                "openingClaimJson",
                "opening_claim_json",
                "evaluationClaim",
                "evaluation_claim",
                "evaluationClaimJson",
                "evaluation_claim_json",
            ),
            "openingClaim",
            lambda: _opening_claim_field(expectation_source, context)["value"].hex(),
            public_inputs["opening_claim"],
        ),
        (
            (
                "querySetHash",
                "query_set_hash",
                "querySetRoot",
                "query_set_root",
                "querySet",
                "query_set",
                "querySetBytes",
                "query_set_bytes",
                "querySetJson",
                "query_set_json",
                "queries",
                "queriesJson",
                "queries_json",
            ),
            "querySet",
            lambda: _query_set_field(expectation_source, context)["value"].hex(),
            public_inputs["query_set"],
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
                "params",
                "paramsBytes",
                "params_bytes",
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


def verify_jindo_lattice_proof_locally(options: Any) -> dict[str, Any]:
    """Verify a deterministic Jindo lattice PCS dev fixture."""

    if isinstance(options, Mapping):
        source = options
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        _COMMON_FIELDS | {"envelope", "proofEnvelope", "proof_envelope", "bytes"},
        "jindoLatticeLocalVerification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "jindoLatticeLocalVerification.envelope",
        "proof envelope",
    )
    decoded = _decode_privacy_proof_envelope_internal(
        envelope_value,
        allow_unsupported_backend=True,
    )
    if decoded["backend"] != "Unsupported":
        raise ValueError(
            "jindoLatticeLocalVerification.envelope.backend must be Unsupported until a production Jindo backend is registered"
        )
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "jindoLatticeLocalVerification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "jindoLatticeLocalVerification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "jindoLatticeLocalVerification.publicInputs",
    )
    _ensure_expectations(source, public_inputs, "jindoLatticeLocalVerification")
    expected_proof = _dev_proof_bytes(
        circuit_id=circuit_id,
        vk_hash=vk_hash,
        public_input_bytes=decoded["public_inputs"],
    )
    if decoded["proof_bytes"] != expected_proof:
        raise ValueError(
            "jindoLatticeLocalVerification proof bytes are not a valid Jindo dev fixture"
        )
    return {
        "ok": True,
        "production": False,
        "kind": "jindo-lattice-dev-fixture-v0",
        "backend": JINDO_BACKEND,
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "parameter_hash": public_inputs["parameter_hash"],
    }


def verify_jindo_polynomial_commitment_v0(options: Any) -> dict[str, Any]:
    """Validate a production Jindo lattice PCS proof envelope binding."""

    if isinstance(options, Mapping):
        source = options
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        _COMMON_FIELDS | {"envelope", "proofEnvelope", "proof_envelope", "bytes"},
        "jindoPolynomialCommitmentV0",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "jindoPolynomialCommitmentV0.envelope",
        "proof envelope",
    )
    decoded = _decode_privacy_proof_envelope_internal(envelope_value)
    if decoded["backend"] != "LatticePcsSis":
        raise ValueError(
            "jindoPolynomialCommitmentV0.envelope.backend must be LatticePcsSis"
        )
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "jindoPolynomialCommitmentV0.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "jindoPolynomialCommitmentV0.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "jindoPolynomialCommitmentV0.publicInputs",
    )
    _ensure_expectations(source, public_inputs, "jindoPolynomialCommitmentV0")
    if decoded["proof_bytes"].startswith(JINDO_DEV_PROOF_PREFIX):
        raise ValueError(
            "jindoPolynomialCommitmentV0 proof bytes must not contain a Jindo dev fixture"
        )
    return {
        "ok": True,
        "production": True,
        "kind": "jindo-lattice-pcs-zk-v0",
        "backend": "LatticePcsSis",
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "parameter_hash": public_inputs["parameter_hash"],
    }


buildJindoLatticePublicInputs = build_jindo_lattice_public_inputs
buildJindoLatticeProofEnvelope = build_jindo_lattice_proof_envelope
buildJindoLatticeProofV0 = build_jindo_lattice_proof_v0
buildJindoLatticeDevProofFixture = build_jindo_lattice_dev_proof_fixture
verifyJindoPolynomialCommitmentV0 = verify_jindo_polynomial_commitment_v0
verifyJindoLatticeProofLocally = verify_jindo_lattice_proof_locally
