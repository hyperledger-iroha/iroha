"""ZK-X.509 on-chain identity SDK helpers."""

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
    _require_plain_mapping,
    build_privacy_proof_envelope,
    decode_privacy_proof_envelope,
)
from .zkat import _normalize_account_id

ZK_X509_BACKEND = "stark/fri/sha256-goldilocks"
ZK_X509_CIRCUIT_ID = "stark/fri/sha256-goldilocks:zk_x509_onchain_identity_v0"
ZK_X509_DOMAIN_SEPARATOR = "iroha:zk-x509:onchain-identity:v0"
ZK_X509_DEV_PROOF_PREFIX = b"iroha:zk-x509:dev-fixture:v0:"
ZK_X509_MAX_CA_ROOT_BYTES = 1024 * 1024
ZK_X509_MAX_POLICY_BYTES = 1024 * 1024
ZK_X509_MAX_REVOCATION_BYTES = 1024 * 1024
ZK_X509_MAX_SUBJECT_BYTES = 1024 * 1024

__all__ = [
    "ZK_X509_BACKEND",
    "ZK_X509_CIRCUIT_ID",
    "ZK_X509_DOMAIN_SEPARATOR",
    "build_zk_x509_identity_commitments",
    "build_zk_x509_identity_envelope",
    "build_zk_x509_identity_proof_v0",
    "build_zk_x509_identity_dev_proof_fixture",
    "verify_zk_x509_identity_proof_v0",
    "verify_zk_x509_identity_proof_locally",
    "buildZkX509IdentityCommitments",
    "buildZkX509IdentityEnvelope",
    "buildZkX509IdentityProofV0",
    "buildZkX509IdentityDevProofFixture",
    "verifyZkX509IdentityProofV0",
    "verifyZkX509IdentityProofLocally",
]


def _normalize_version(value: Any, context: str) -> int:
    version = _positive_u32(1 if value is _MISSING or value is None else value, context)
    if version != 1:
        raise ValueError(f"{context} must be 1")
    return version


def _normalize_backend_tag(value: Any, context: str) -> str:
    _tag, decoded = _normalize_backend(
        ZK_X509_BACKEND if value is _MISSING or value is None else value,
        context,
    )
    if decoded != "Stark":
        raise ValueError(f"{context} must be {ZK_X509_BACKEND}")
    return ZK_X509_BACKEND


def _normalize_circuit_id(value: Any, context: str) -> str:
    circuit_id = _require_non_blank_string(
        ZK_X509_CIRCUIT_ID if value is _MISSING or value is None else value,
        context,
    )
    if circuit_id not in {
        ZK_X509_CIRCUIT_ID,
        "zk_x509_onchain_identity_v0",
    }:
        raise ValueError(f"{context} must identify zk_x509_onchain_identity_v0")
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
    digest.update(f"iroha:zk-x509:{label}:v0".encode("utf-8"))
    digest.update(b"\x00")
    digest.update(domain_separator.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(data)
    return digest.digest()


def _address_binding_bytes(*, binding_text: str, domain_separator: str) -> bytes:
    digest = hashlib.sha256()
    digest.update(b"iroha:zk-x509:address-binding:v0")
    digest.update(b"\x00")
    digest.update(domain_separator.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(binding_text.encode("utf-8"))
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
        ZK_X509_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
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


def _ca_root_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxCaRootBytes",
            source.get("max_ca_root_bytes", ZK_X509_MAX_CA_ROOT_BYTES),
        ),
        f"{context}.maxCaRootBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=("caRootCommitment", "ca_root_commitment"),
        data_aliases=(
            "caRoot",
            "ca_root",
            "caRootBytes",
            "ca_root_bytes",
            "caRootJson",
            "ca_root_json",
            "trustRoot",
            "trust_root",
            "trustRootJson",
            "trust_root_json",
        ),
        json_aliases={"caRoot", "ca_root", "trustRoot", "trust_root"},
        field_path="caRootCommitment",
        data_path="caRoot",
        data_json_path="caRootJson",
        data_description="CA root commitment",
        digest_label="ca-root-commitment",
        kind_label="ca-root-digest",
        max_bytes=max_bytes,
    )


def _certificate_policy_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxPolicyBytes",
            source.get("max_policy_bytes", ZK_X509_MAX_POLICY_BYTES),
        ),
        f"{context}.maxPolicyBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=("certificatePolicyHash", "certificate_policy_hash"),
        data_aliases=(
            "certificatePolicy",
            "certificate_policy",
            "certificatePolicyBytes",
            "certificate_policy_bytes",
            "certificatePolicyJson",
            "certificate_policy_json",
        ),
        json_aliases={"certificatePolicy", "certificate_policy"},
        field_path="certificatePolicyHash",
        data_path="certificatePolicy",
        data_json_path="certificatePolicyJson",
        data_description="certificate policy hash",
        digest_label="certificate-policy-hash",
        kind_label="certificate-policy-digest",
        max_bytes=max_bytes,
    )


def _revocation_root_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxRevocationBytes",
            source.get("max_revocation_bytes", ZK_X509_MAX_REVOCATION_BYTES),
        ),
        f"{context}.maxRevocationBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=("revocationRoot", "revocation_root"),
        data_aliases=(
            "revocationData",
            "revocation_data",
            "revocationBytes",
            "revocation_bytes",
            "revocationJson",
            "revocation_json",
            "revocationSet",
            "revocation_set",
            "revocationSetJson",
            "revocation_set_json",
            "revocationList",
            "revocation_list",
            "revocationListJson",
            "revocation_list_json",
        ),
        json_aliases={
            "revocationData",
            "revocation_data",
            "revocationSet",
            "revocation_set",
            "revocationList",
            "revocation_list",
        },
        field_path="revocationRoot",
        data_path="revocationData",
        data_json_path="revocationJson",
        data_description="revocation root",
        digest_label="revocation-root",
        kind_label="revocation-root-digest",
        max_bytes=max_bytes,
    )


def _subject_commitment_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    max_bytes = _positive_u32(
        source.get(
            "maxSubjectBytes",
            source.get("max_subject_bytes", ZK_X509_MAX_SUBJECT_BYTES),
        ),
        f"{context}.maxSubjectBytes",
    )
    return _derived_field(
        source,
        context,
        explicit_aliases=("subjectCommitment", "subject_commitment"),
        data_aliases=(
            "subject",
            "subjectBytes",
            "subject_bytes",
            "subjectJson",
            "subject_json",
            "certificateSubject",
            "certificate_subject",
            "certificateSubjectJson",
            "certificate_subject_json",
        ),
        json_aliases={"subject", "certificateSubject", "certificate_subject"},
        field_path="subjectCommitment",
        data_path="subject",
        data_json_path="subjectJson",
        data_description="subject commitment",
        digest_label="subject-commitment",
        kind_label="subject-digest",
        max_bytes=max_bytes,
    )


def _address_binding_field(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    _binding_key, binding_value = _read_single_alias(
        source,
        ("addressBinding", "address_binding", "walletBinding", "wallet_binding"),
        f"{context}.addressBinding",
        "address binding",
    )
    _account_key, account_value = _read_single_alias(
        source,
        ("accountId", "account_id", "walletAccountId", "wallet_account_id"),
        f"{context}.accountId",
        "account id",
    )
    _wallet_key, wallet_value = _read_single_alias(
        source,
        ("walletAddress", "wallet_address"),
        f"{context}.walletAddress",
        "wallet address",
    )
    if account_value is not _MISSING and wallet_value is not _MISSING:
        raise ValueError(f"{context}.accountId and {context}.walletAddress must not both be provided")
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    domain_separator = _require_non_blank_string(
        ZK_X509_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    explicit = (
        None
        if binding_value is _MISSING
        else _fixed_bytes(
            binding_value,
            f"{context}.addressBinding",
            32,
            nonzero=True,
        )
    )
    if account_value is not _MISSING:
        binding_text = _normalize_account_id(account_value, f"{context}.accountId")
    elif wallet_value is not _MISSING:
        binding_text = _require_non_blank_string(
            wallet_value,
            f"{context}.walletAddress",
        )
    else:
        binding_text = None
    derived = (
        None
        if binding_text is None
        else _address_binding_bytes(
            binding_text=binding_text,
            domain_separator=domain_separator,
        )
    )
    if explicit is None and derived is None:
        raise ValueError(f"{context}.addressBinding or {context}.accountId is required")
    if explicit is not None and derived is not None and explicit != derived:
        raise ValueError(f"{context}.addressBinding must match the derived account binding")
    return {
        "value": explicit if explicit is not None else derived,
        "kind": "external" if derived is None else "dev-sha256-account-binding",
        "digest": None,
    }


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
        ZK_X509_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    normalized = _source_with_domain(source, domain_separator)
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "ca_root": _ca_root_field(normalized, context),
        "certificate_policy": _certificate_policy_field(normalized, context),
        "revocation_root": _revocation_root_field(normalized, context),
        "subject": _subject_commitment_field(normalized, context),
        "address_binding": _address_binding_field(normalized, context),
        "domain_separator": domain_separator,
    }


_COMMON_FIELDS = {
    "version",
    "caRootCommitment",
    "ca_root_commitment",
    "caRoot",
    "ca_root",
    "caRootBytes",
    "ca_root_bytes",
    "caRootJson",
    "ca_root_json",
    "trustRoot",
    "trust_root",
    "trustRootJson",
    "trust_root_json",
    "certificatePolicyHash",
    "certificate_policy_hash",
    "certificatePolicy",
    "certificate_policy",
    "certificatePolicyBytes",
    "certificate_policy_bytes",
    "certificatePolicyJson",
    "certificate_policy_json",
    "revocationRoot",
    "revocation_root",
    "revocationData",
    "revocation_data",
    "revocationBytes",
    "revocation_bytes",
    "revocationJson",
    "revocation_json",
    "revocationSet",
    "revocation_set",
    "revocationSetJson",
    "revocation_set_json",
    "revocationList",
    "revocation_list",
    "revocationListJson",
    "revocation_list_json",
    "subjectCommitment",
    "subject_commitment",
    "subject",
    "subjectBytes",
    "subject_bytes",
    "subjectJson",
    "subject_json",
    "certificateSubject",
    "certificate_subject",
    "certificateSubjectJson",
    "certificate_subject_json",
    "addressBinding",
    "address_binding",
    "walletBinding",
    "wallet_binding",
    "accountId",
    "account_id",
    "walletAccountId",
    "wallet_account_id",
    "walletAddress",
    "wallet_address",
    "domainSeparator",
    "domain_separator",
    "maxCaRootBytes",
    "max_ca_root_bytes",
    "maxPolicyBytes",
    "max_policy_bytes",
    "maxRevocationBytes",
    "max_revocation_bytes",
    "maxSubjectBytes",
    "max_subject_bytes",
}


def build_zk_x509_identity_commitments(options: Mapping[str, Any]) -> dict[str, Any]:
    """Normalize ZK-X.509 identity proof public-input commitments."""

    source = _require_plain_mapping(options, "zkX509IdentityCommitments")
    _reject_unknown_fields(source, _COMMON_FIELDS, "zkX509IdentityCommitments")
    parts = _commitment_parts(source, "zkX509IdentityCommitments")
    return {
        "version": parts["version"],
        "ca_root_commitment": parts["ca_root"]["value"],
        "certificate_policy_hash": parts["certificate_policy"]["value"],
        "revocation_root": parts["revocation_root"]["value"],
        "subject_commitment": parts["subject"]["value"],
        "address_binding": parts["address_binding"]["value"],
        "domain_separator": parts["domain_separator"],
        "commitment_kinds": {
            "ca_root_commitment": parts["ca_root"]["kind"],
            "certificate_policy_hash": parts["certificate_policy"]["kind"],
            "revocation_root": parts["revocation_root"]["kind"],
            "subject_commitment": parts["subject"]["kind"],
            "address_binding": parts["address_binding"]["kind"],
        },
        "source_digests": {
            "ca_root": parts["ca_root"]["digest"],
            "certificate_policy": parts["certificate_policy"]["digest"],
            "revocation": parts["revocation_root"]["digest"],
            "subject": parts["subject"]["digest"],
            "address_binding": parts["address_binding"]["digest"],
        },
    }


def _normalize_public_inputs(value: Any, context: str) -> dict[str, Any]:
    source = _require_mapping(value, context)
    _reject_unknown_fields(
        source,
        {
            "version",
            "ca_root_commitment",
            "caRootCommitment",
            "certificate_policy_hash",
            "certificatePolicyHash",
            "revocation_root",
            "revocationRoot",
            "subject_commitment",
            "subjectCommitment",
            "address_binding",
            "addressBinding",
            "domain_separator",
            "domainSeparator",
        },
        context,
    )
    _ca_key, ca_value = _read_single_alias(
        source,
        ("ca_root_commitment", "caRootCommitment"),
        f"{context}.caRootCommitment",
        "CA root commitment",
    )
    _policy_key, policy_value = _read_single_alias(
        source,
        ("certificate_policy_hash", "certificatePolicyHash"),
        f"{context}.certificatePolicyHash",
        "certificate policy hash",
    )
    _revocation_key, revocation_value = _read_single_alias(
        source,
        ("revocation_root", "revocationRoot"),
        f"{context}.revocationRoot",
        "revocation root",
    )
    _subject_key, subject_value = _read_single_alias(
        source,
        ("subject_commitment", "subjectCommitment"),
        f"{context}.subjectCommitment",
        "subject commitment",
    )
    _address_key, address_value = _read_single_alias(
        source,
        ("address_binding", "addressBinding"),
        f"{context}.addressBinding",
        "address binding",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domain_separator", "domainSeparator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "ca_root_commitment": _fixed_bytes(
            ca_value,
            f"{context}.caRootCommitment",
            32,
            nonzero=True,
        ).hex(),
        "certificate_policy_hash": _fixed_bytes(
            policy_value,
            f"{context}.certificatePolicyHash",
            32,
            nonzero=True,
        ).hex(),
        "revocation_root": _fixed_bytes(
            revocation_value,
            f"{context}.revocationRoot",
            32,
            nonzero=True,
        ).hex(),
        "subject_commitment": _fixed_bytes(
            subject_value,
            f"{context}.subjectCommitment",
            32,
            nonzero=True,
        ).hex(),
        "address_binding": _fixed_bytes(
            address_value,
            f"{context}.addressBinding",
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
        "ca_root_commitment": parts["ca_root"]["value"].hex(),
        "certificate_policy_hash": parts["certificate_policy"]["value"].hex(),
        "revocation_root": parts["revocation_root"]["value"].hex(),
        "subject_commitment": parts["subject"]["value"].hex(),
        "address_binding": parts["address_binding"]["value"].hex(),
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


def build_zk_x509_identity_envelope(options: Mapping[str, Any]) -> bytes:
    """Build canonical OpenVerifyEnvelope bytes for a prepared ZK-X.509 proof."""

    source = _require_plain_mapping(options, "zkX509IdentityEnvelope")
    _reject_unknown_fields(source, _ENVELOPE_FIELDS, "zkX509IdentityEnvelope")
    parts = _proof_parts(source, "zkX509IdentityEnvelope", require_proof_bytes=True)
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


def build_zk_x509_identity_proof_v0(options: Mapping[str, Any]) -> bytes:
    """Build canonical production ZK-X.509 identity proof envelope bytes."""

    source = _require_plain_mapping(options, "zkX509IdentityProofV0")
    _reject_unknown_fields(source, _ENVELOPE_FIELDS, "zkX509IdentityProofV0")
    parts = _proof_parts(source, "zkX509IdentityProofV0", require_proof_bytes=True)
    if parts["proof_bytes"].startswith(ZK_X509_DEV_PROOF_PREFIX):
        raise ValueError(
            "zkX509IdentityProofV0.proofBytes must not contain a dev fixture proof"
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
    digest.update(b"iroha:zk-x509:dev-fixture:v0")
    digest.update(b"\x00")
    digest.update(circuit_id.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(vk_hash)
    digest.update(b"\x00")
    digest.update(public_input_bytes)
    return ZK_X509_DEV_PROOF_PREFIX + digest.digest()


def build_zk_x509_identity_dev_proof_fixture(options: Mapping[str, Any]) -> dict[str, Any]:
    """Build a deterministic ZK-X.509 identity dev proof fixture."""

    source = _require_plain_mapping(options, "zkX509IdentityDevProofFixture")
    _reject_unknown_fields(
        source,
        _ENVELOPE_FIELDS - {"proofBytes", "proof_bytes", "proof"},
        "zkX509IdentityDevProofFixture",
    )
    parts = _proof_parts(
        source,
        "zkX509IdentityDevProofFixture",
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
        "kind": "zk-x509-dev-fixture-v0",
        "production": False,
        "proof_bytes": proof_bytes,
        "proofBytes": proof_bytes,
        "commitments": {
            "ca_root_commitment": commitments["ca_root"]["value"],
            "certificate_policy_hash": commitments["certificate_policy"]["value"],
            "revocation_root": commitments["revocation_root"]["value"],
            "subject_commitment": commitments["subject"]["value"],
            "address_binding": commitments["address_binding"]["value"],
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
                "caRootCommitment",
                "ca_root_commitment",
                "caRoot",
                "ca_root",
                "caRootBytes",
                "ca_root_bytes",
                "caRootJson",
                "ca_root_json",
                "trustRoot",
                "trust_root",
                "trustRootJson",
                "trust_root_json",
            ),
            "caRootCommitment",
            lambda: _ca_root_field(expectation_source, context)["value"].hex(),
            public_inputs["ca_root_commitment"],
        ),
        (
            (
                "certificatePolicyHash",
                "certificate_policy_hash",
                "certificatePolicy",
                "certificate_policy",
                "certificatePolicyBytes",
                "certificate_policy_bytes",
                "certificatePolicyJson",
                "certificate_policy_json",
            ),
            "certificatePolicyHash",
            lambda: _certificate_policy_field(expectation_source, context)["value"].hex(),
            public_inputs["certificate_policy_hash"],
        ),
        (
            (
                "revocationRoot",
                "revocation_root",
                "revocationData",
                "revocation_data",
                "revocationBytes",
                "revocation_bytes",
                "revocationJson",
                "revocation_json",
                "revocationSet",
                "revocation_set",
                "revocationSetJson",
                "revocation_set_json",
                "revocationList",
                "revocation_list",
                "revocationListJson",
                "revocation_list_json",
            ),
            "revocationRoot",
            lambda: _revocation_root_field(expectation_source, context)["value"].hex(),
            public_inputs["revocation_root"],
        ),
        (
            (
                "subjectCommitment",
                "subject_commitment",
                "subject",
                "subjectBytes",
                "subject_bytes",
                "subjectJson",
                "subject_json",
                "certificateSubject",
                "certificate_subject",
                "certificateSubjectJson",
                "certificate_subject_json",
            ),
            "subjectCommitment",
            lambda: _subject_commitment_field(expectation_source, context)["value"].hex(),
            public_inputs["subject_commitment"],
        ),
        (
            (
                "addressBinding",
                "address_binding",
                "walletBinding",
                "wallet_binding",
                "accountId",
                "account_id",
                "walletAccountId",
                "wallet_account_id",
                "walletAddress",
                "wallet_address",
            ),
            "addressBinding",
            lambda: _address_binding_field(expectation_source, context)["value"].hex(),
            public_inputs["address_binding"],
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


def verify_zk_x509_identity_proof_locally(options: Any) -> dict[str, Any]:
    """Verify a deterministic ZK-X.509 identity dev fixture."""

    if isinstance(options, Mapping):
        source = _require_plain_mapping(options, "zkX509IdentityLocalVerification")
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        _COMMON_FIELDS | {"envelope", "proofEnvelope", "proof_envelope", "bytes"},
        "zkX509IdentityLocalVerification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "zkX509IdentityLocalVerification.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError("zkX509IdentityLocalVerification.envelope.backend must be Stark")
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "zkX509IdentityLocalVerification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "zkX509IdentityLocalVerification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "zkX509IdentityLocalVerification.publicInputs",
    )
    _ensure_expectations(source, public_inputs, "zkX509IdentityLocalVerification")
    expected_proof = _dev_proof_bytes(
        circuit_id=circuit_id,
        vk_hash=vk_hash,
        public_input_bytes=decoded["public_inputs"],
    )
    if decoded["proof_bytes"] != expected_proof:
        raise ValueError(
            "zkX509IdentityLocalVerification proof bytes are not a valid ZK-X.509 dev fixture"
        )
    return {
        "ok": True,
        "production": False,
        "kind": "zk-x509-dev-fixture-v0",
        "backend": ZK_X509_BACKEND,
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "address_binding": public_inputs["address_binding"],
    }


def verify_zk_x509_identity_proof_v0(options: Any) -> dict[str, Any]:
    """Validate a production ZK-X.509 identity proof envelope binding."""

    if isinstance(options, Mapping):
        source = _require_plain_mapping(options, "zkX509IdentityProofV0")
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        _COMMON_FIELDS | {"envelope", "proofEnvelope", "proof_envelope", "bytes"},
        "zkX509IdentityProofV0",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "zkX509IdentityProofV0.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError("zkX509IdentityProofV0.envelope.backend must be Stark")
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "zkX509IdentityProofV0.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "zkX509IdentityProofV0.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "zkX509IdentityProofV0.publicInputs",
    )
    _ensure_expectations(source, public_inputs, "zkX509IdentityProofV0")
    if decoded["proof_bytes"].startswith(ZK_X509_DEV_PROOF_PREFIX):
        raise ValueError(
            "zkX509IdentityProofV0 proof bytes must not contain a ZK-X.509 dev fixture"
        )
    return {
        "ok": True,
        "production": True,
        "kind": "zk-x509-onchain-identity-v0",
        "backend": "Stark",
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "address_binding": public_inputs["address_binding"],
    }


buildZkX509IdentityCommitments = build_zk_x509_identity_commitments
buildZkX509IdentityEnvelope = build_zk_x509_identity_envelope
buildZkX509IdentityProofV0 = build_zk_x509_identity_proof_v0
buildZkX509IdentityDevProofFixture = build_zk_x509_identity_dev_proof_fixture
verifyZkX509IdentityProofV0 = verify_zk_x509_identity_proof_v0
verifyZkX509IdentityProofLocally = verify_zk_x509_identity_proof_locally
