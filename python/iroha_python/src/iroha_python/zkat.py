"""zkAt policy-private authenticator SDK helpers."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from typing import Any

from .address import AccountAddress, AccountAddressError
from .verange import (
    DEFAULT_PRIVACY_MAX_PROOF_BYTES,
    DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES,
    VERANGE_MAX_PAYLOAD_BYTES,
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

ZKAT_BACKEND = "stark/fri/sha256-goldilocks"
ZKAT_CIRCUIT_ID = "stark/fri/sha256-goldilocks:zkat_policy_private_auth_v1"
ZKAT_DOMAIN_SEPARATOR = "iroha:zkat:policy-private-auth:v1"
ZKAT_DEV_PROOF_PREFIX = b"iroha:zkat:dev-fixture:v1:"
ZKAT_MAX_POLICY_BYTES = 1024 * 1024

__all__ = [
    "ZKAT_BACKEND",
    "ZKAT_CIRCUIT_ID",
    "ZKAT_DOMAIN_SEPARATOR",
    "build_zkat_policy_commitment",
    "build_zkat_authenticator_envelope",
    "build_zkat_policy_proof_v1",
    "build_zkat_dev_proof_fixture",
    "verify_zkat_policy_proof_v1",
    "verify_zkat_authenticator_locally",
    "buildZkAtPolicyCommitment",
    "buildZkAtAuthenticatorEnvelope",
    "buildZkAtPolicyProofV1",
    "buildZkAtDevProofFixture",
    "verifyZkAtPolicyProofV1",
    "verifyZkAtAuthenticatorLocally",
]

_HEX_64 = re.compile(r"^[0-9a-fA-F]{64}$")


def _normalize_version(value: Any, context: str) -> int:
    version = _positive_u32(1 if value is _MISSING or value is None else value, context)
    if version != 1:
        raise ValueError(f"{context} must be 1")
    return version


def _normalize_zkat_backend(value: Any, context: str) -> str:
    _tag, decoded = _normalize_backend(
        ZKAT_BACKEND if value is _MISSING or value is None else value,
        context,
    )
    if decoded != "Stark":
        raise ValueError(f"{context} must be {ZKAT_BACKEND}")
    return ZKAT_BACKEND


def _normalize_circuit_id(value: Any, context: str) -> str:
    circuit_id = _require_non_blank_string(
        ZKAT_CIRCUIT_ID if value is _MISSING or value is None else value,
        context,
    )
    if circuit_id not in {ZKAT_CIRCUIT_ID, "zkat_policy_private_auth_v1"}:
        raise ValueError(f"{context} must identify zkat_policy_private_auth_v1")
    return circuit_id


def _normalize_policy_epoch(value: Any, context: str) -> int:
    return _positive_u32(value, context)


def _normalize_policy_bytes(
    value: Any,
    alias_key: str | None,
    context: str,
    max_policy_bytes: int,
) -> bytes:
    if alias_key in {"policy", "policyJson", "policy_json"}:
        policy_bytes = _canonical_json_bytes(value, f"{context}.policyJson")
    else:
        policy_bytes = _bounded_bytes(
            value,
            f"{context}.policy",
            max_bytes=max_policy_bytes,
        )
    if len(policy_bytes) > max_policy_bytes:
        raise ValueError(f"{context}.policy must be no larger than {max_policy_bytes} bytes")
    return policy_bytes


def _policy_commitment_bytes(
    *,
    policy_bytes: bytes,
    policy_epoch: int,
    domain_separator: str,
    policy_schema: str,
) -> bytes:
    digest = hashlib.sha256()
    digest.update(b"iroha:zkat:policy-commitment:v1")
    digest.update(b"\x00")
    digest.update(str(policy_epoch).encode("utf-8"))
    digest.update(b"\x00")
    digest.update(domain_separator.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(policy_schema.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(policy_bytes)
    return digest.digest()


def _normalize_payload_digest(source: Mapping[str, Any], context: str) -> bytes:
    digest_key, digest_value = _read_single_alias(
        source,
        ("txDigest", "tx_digest", "payloadDigest", "payload_digest"),
        f"{context}.txDigest",
        "transaction digest",
    )
    payload_key, payload_value = _read_single_alias(
        source,
        ("payload", "payloadBytes", "payload_bytes", "payloadJson", "payload_json"),
        f"{context}.payload",
        "payload",
    )
    explicit_digest = (
        None
        if digest_key is None
        else _fixed_bytes(digest_value, f"{context}.txDigest", 32, nonzero=True)
    )
    if payload_key is None:
        payload_digest = None
    elif payload_key in {"payloadJson", "payload_json"}:
        payload_digest = hashlib.sha256(
            _canonical_json_bytes(payload_value, f"{context}.payloadJson")
        ).digest()
    else:
        max_payload_bytes = _positive_u32(
            source.get(
                "maxPayloadBytes",
                source.get("max_payload_bytes", VERANGE_MAX_PAYLOAD_BYTES),
            ),
            f"{context}.maxPayloadBytes",
        )
        payload_digest = hashlib.sha256(
            _bounded_bytes(
                payload_value,
                f"{context}.payload",
                max_bytes=max_payload_bytes,
            )
        ).digest()
    if explicit_digest is None and payload_digest is None:
        raise ValueError(f"{context}.txDigest or {context}.payload is required")
    if explicit_digest is not None and payload_digest is not None:
        if explicit_digest != payload_digest:
            raise ValueError(
                f"{context}.txDigest must match the SHA-256 digest of {context}.payload"
            )
    return explicit_digest if explicit_digest is not None else payload_digest  # type: ignore[return-value]


def _normalize_account_id(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    raw = value.strip()
    if not raw:
        raise ValueError(f"{context} must be non-empty")
    if "@" in raw:
        raise ValueError(f"{context} must not include '@domain'; use an encoded i105 account id")
    if raw.lower().startswith(("uaid:", "opaque:")) or _HEX_64.fullmatch(raw):
        raise ValueError(f"{context} must be a canonical I105 account id")
    try:
        AccountAddress.parse_encoded(raw)
    except AccountAddressError as exc:
        raise ValueError(f"{context} must be a canonical I105 account id") from exc
    return raw


def build_zkat_policy_commitment(
    options: Mapping[str, Any],
    context: str = "zkAtPolicyCommitment",
) -> dict[str, Any]:
    """Normalize or derive a zkAt policy commitment descriptor."""

    source = _require_plain_mapping(options, context)
    _reject_unknown_fields(
        source,
        {
            "version",
            "policyCommitment",
            "policy_commitment",
            "commitment",
            "policy",
            "policyBytes",
            "policy_bytes",
            "policyJson",
            "policy_json",
            "policyEpoch",
            "policy_epoch",
            "domainSeparator",
            "domain_separator",
            "policySchema",
            "policy_schema",
            "maxPolicyBytes",
            "max_policy_bytes",
        },
        context,
    )
    _commitment_key, commitment_value = _read_single_alias(
        source,
        ("policyCommitment", "policy_commitment", "commitment"),
        f"{context}.policyCommitment",
        "policy commitment",
    )
    policy_key, policy_value = _read_single_alias(
        source,
        ("policy", "policyBytes", "policy_bytes", "policyJson", "policy_json"),
        f"{context}.policy",
        "policy",
    )
    _epoch_key, epoch_value = _read_single_alias(
        source,
        ("policyEpoch", "policy_epoch"),
        f"{context}.policyEpoch",
        "policy epoch",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    _schema_key, schema_value = _read_single_alias(
        source,
        ("policySchema", "policy_schema"),
        f"{context}.policySchema",
        "policy schema",
    )
    policy_epoch = _normalize_policy_epoch(epoch_value, f"{context}.policyEpoch")
    domain_separator = _require_non_blank_string(
        ZKAT_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    policy_schema = _require_non_blank_string(
        "zkat-policy-json-v1" if schema_value is _MISSING else schema_value,
        f"{context}.policySchema",
    )
    explicit_commitment = (
        None
        if commitment_value is _MISSING
        else _fixed_bytes(
            commitment_value,
            f"{context}.policyCommitment",
            32,
            nonzero=True,
        )
    )
    if policy_key is None:
        policy_bytes = None
    else:
        max_policy_bytes = _positive_u32(
            source.get(
                "maxPolicyBytes",
                source.get("max_policy_bytes", ZKAT_MAX_POLICY_BYTES),
            ),
            f"{context}.maxPolicyBytes",
        )
        policy_bytes = _normalize_policy_bytes(
            policy_value,
            policy_key,
            context,
            max_policy_bytes,
        )
    if explicit_commitment is None and policy_bytes is None:
        raise ValueError(f"{context}.policyCommitment or {context}.policy is required")
    derived_commitment = (
        None
        if policy_bytes is None
        else _policy_commitment_bytes(
            policy_bytes=policy_bytes,
            policy_epoch=policy_epoch,
            domain_separator=domain_separator,
            policy_schema=policy_schema,
        )
    )
    if explicit_commitment is not None and derived_commitment is not None:
        if explicit_commitment != derived_commitment:
            raise ValueError(
                f"{context}.policyCommitment must match the derived policy commitment"
            )
    policy_commitment = (
        explicit_commitment if explicit_commitment is not None else derived_commitment
    )
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "policy_commitment": policy_commitment,
        "policy_epoch": policy_epoch,
        "domain_separator": domain_separator,
        "policy_schema": policy_schema,
        "commitment_kind": (
            "external" if policy_bytes is None else "dev-sha256-policy-digest"
        ),
        "policy_digest": None
        if policy_bytes is None
        else hashlib.sha256(policy_bytes).digest(),
    }


def _normalize_policy_commitment_from_source(
    source: Mapping[str, Any],
    context: str,
) -> bytes:
    _commitment_key, commitment_value = _read_single_alias(
        source,
        ("policyCommitment", "policy_commitment", "commitment"),
        f"{context}.policyCommitment",
        "policy commitment",
    )
    policy_key, _policy_value = _read_single_alias(
        source,
        ("policy", "policyBytes", "policy_bytes", "policyJson", "policy_json"),
        f"{context}.policy",
        "policy",
    )
    if commitment_value is not _MISSING and policy_key is None:
        return _fixed_bytes(
            commitment_value,
            f"{context}.policyCommitment",
            32,
            nonzero=True,
        )
    commitment = build_zkat_policy_commitment(
        {
            key: source[key]
            for key in (
                "version",
                "policyCommitment",
                "policy_commitment",
                "commitment",
                "policy",
                "policyBytes",
                "policy_bytes",
                "policyJson",
                "policy_json",
                "policyEpoch",
                "policy_epoch",
                "domainSeparator",
                "domain_separator",
                "policySchema",
                "policy_schema",
                "maxPolicyBytes",
                "max_policy_bytes",
            )
            if key in source
        },
        f"{context}.policyCommitment",
    )
    return commitment["policy_commitment"]


def _normalize_public_inputs(value: Any, context: str) -> dict[str, Any]:
    source = _require_mapping(value, context)
    _reject_unknown_fields(
        source,
        {
            "version",
            "policy_commitment",
            "policyCommitment",
            "tx_digest",
            "txDigest",
            "account_id",
            "accountId",
            "action_class",
            "actionClass",
            "domain_separator",
            "domainSeparator",
            "policy_epoch",
            "policyEpoch",
        },
        context,
    )
    _policy_key, policy_value = _read_single_alias(
        source,
        ("policy_commitment", "policyCommitment"),
        f"{context}.policyCommitment",
        "policy commitment",
    )
    _tx_key, tx_value = _read_single_alias(
        source,
        ("tx_digest", "txDigest"),
        f"{context}.txDigest",
        "transaction digest",
    )
    _account_key, account_value = _read_single_alias(
        source,
        ("account_id", "accountId"),
        f"{context}.accountId",
        "account id",
    )
    _action_key, action_value = _read_single_alias(
        source,
        ("action_class", "actionClass"),
        f"{context}.actionClass",
        "action class",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domain_separator", "domainSeparator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    _epoch_key, epoch_value = _read_single_alias(
        source,
        ("policy_epoch", "policyEpoch"),
        f"{context}.policyEpoch",
        "policy epoch",
    )
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "policy_commitment": _fixed_bytes(
            policy_value,
            f"{context}.policyCommitment",
            32,
            nonzero=True,
        ).hex(),
        "tx_digest": _fixed_bytes(
            tx_value,
            f"{context}.txDigest",
            32,
            nonzero=True,
        ).hex(),
        "account_id": _normalize_account_id(account_value, f"{context}.accountId"),
        "action_class": _require_non_blank_string(
            action_value,
            f"{context}.actionClass",
        ),
        "domain_separator": _require_non_blank_string(
            domain_value,
            f"{context}.domainSeparator",
        ),
        "policy_epoch": _normalize_policy_epoch(
            epoch_value,
            f"{context}.policyEpoch",
        ),
    }


def _normalize_authenticator_parts(
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
    _epoch_key, epoch_value = _read_single_alias(
        source,
        ("policyEpoch", "policy_epoch"),
        f"{context}.policyEpoch",
        "policy epoch",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    _account_key, account_value = _read_single_alias(
        source,
        ("accountId", "account_id"),
        f"{context}.accountId",
        "account id",
    )
    _action_key, action_value = _read_single_alias(
        source,
        ("actionClass", "action_class"),
        f"{context}.actionClass",
        "action class",
    )
    policy_epoch = _normalize_policy_epoch(epoch_value, f"{context}.policyEpoch")
    domain_separator = _require_non_blank_string(
        ZKAT_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    policy_source = {
        key: value
        for key, value in source.items()
        if key
        not in {
            "policyEpoch",
            "policy_epoch",
            "domainSeparator",
            "domain_separator",
        }
    }
    policy_source["policyEpoch"] = policy_epoch
    policy_source["domainSeparator"] = domain_separator
    public_inputs = {
        "version": 1,
        "policy_commitment": _normalize_policy_commitment_from_source(
            policy_source,
            context,
        ).hex(),
        "tx_digest": _normalize_payload_digest(source, context).hex(),
        "account_id": _normalize_account_id(account_value, f"{context}.accountId"),
        "action_class": _require_non_blank_string(
            action_value,
            f"{context}.actionClass",
        ),
        "domain_separator": domain_separator,
        "policy_epoch": policy_epoch,
    }
    max_proof_bytes = _positive_u32(
        source.get("maxProofBytes", source.get("max_proof_bytes", DEFAULT_PRIVACY_MAX_PROOF_BYTES)),
        f"{context}.maxProofBytes",
    )
    return {
        "backend": _normalize_zkat_backend(backend_value, f"{context}.backendTag"),
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


_AUTHENTICATOR_FIELDS = {
    "backend",
    "backendTag",
    "backend_tag",
    "circuitId",
    "circuit_id",
    "vkHash",
    "vk_hash",
    "verifierKeyHash",
    "verifyingKeyHash",
    "policyCommitment",
    "policy_commitment",
    "commitment",
    "policy",
    "policyBytes",
    "policy_bytes",
    "policyJson",
    "policy_json",
    "policyEpoch",
    "policy_epoch",
    "policySchema",
    "policy_schema",
    "txDigest",
    "tx_digest",
    "payloadDigest",
    "payload_digest",
    "payload",
    "payloadBytes",
    "payload_bytes",
    "payloadJson",
    "payload_json",
    "accountId",
    "account_id",
    "actionClass",
    "action_class",
    "domainSeparator",
    "domain_separator",
    "proofBytes",
    "proof_bytes",
    "proof",
    "aux",
    "maxProofBytes",
    "max_proof_bytes",
    "maxPublicInputBytes",
    "max_public_input_bytes",
    "maxPayloadBytes",
    "max_payload_bytes",
    "maxPolicyBytes",
    "max_policy_bytes",
    "version",
}


def build_zkat_authenticator_envelope(options: Mapping[str, Any]) -> bytes:
    """Build canonical OpenVerifyEnvelope bytes for a prepared zkAt authenticator."""

    source = _require_plain_mapping(options, "zkAtAuthenticatorEnvelope")
    _reject_unknown_fields(source, _AUTHENTICATOR_FIELDS, "zkAtAuthenticatorEnvelope")
    parts = _normalize_authenticator_parts(
        source,
        "zkAtAuthenticatorEnvelope",
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


def build_zkat_policy_proof_v1(options: Mapping[str, Any]) -> bytes:
    """Build canonical production zkAt policy proof envelope bytes."""

    source = _require_plain_mapping(options, "zkAtPolicyProofV1")
    _reject_unknown_fields(source, _AUTHENTICATOR_FIELDS, "zkAtPolicyProofV1")
    parts = _normalize_authenticator_parts(
        source,
        "zkAtPolicyProofV1",
        require_proof_bytes=True,
    )
    if parts["proof_bytes"].startswith(ZKAT_DEV_PROOF_PREFIX):
        raise ValueError("zkAtPolicyProofV1.proofBytes must not contain a dev fixture proof")
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
    digest.update(b"iroha:zkat:dev-fixture:v1")
    digest.update(b"\x00")
    digest.update(circuit_id.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(vk_hash)
    digest.update(b"\x00")
    digest.update(public_input_bytes)
    return ZKAT_DEV_PROOF_PREFIX + digest.digest()


def build_zkat_dev_proof_fixture(options: Mapping[str, Any]) -> dict[str, Any]:
    """Build a deterministic zkAt dev proof fixture."""

    source = _require_plain_mapping(options, "zkAtDevProofFixture")
    _reject_unknown_fields(
        source,
        _AUTHENTICATOR_FIELDS - {"proofBytes", "proof_bytes", "proof"},
        "zkAtDevProofFixture",
    )
    parts = _normalize_authenticator_parts(
        source,
        "zkAtDevProofFixture",
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
        "kind": "zkat-dev-fixture-v1",
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


def _ensure_verification_expectations(
    source: Mapping[str, Any],
    public_inputs: Mapping[str, Any],
    context: str,
) -> None:
    if any(
        key in source
        for key in (
            "policyCommitment",
            "policy_commitment",
            "commitment",
            "policy",
            "policyBytes",
            "policy_bytes",
            "policyJson",
            "policy_json",
        )
    ):
        policy_source = {
            key: value
            for key, value in source.items()
            if key
            not in {
                "policyEpoch",
                "policy_epoch",
                "domainSeparator",
                "domain_separator",
            }
        }
        policy_source["policyEpoch"] = public_inputs["policy_epoch"]
        policy_source["domainSeparator"] = public_inputs["domain_separator"]
        expected_policy = _normalize_policy_commitment_from_source(
            policy_source,
            context,
        ).hex()
        if expected_policy != public_inputs["policy_commitment"]:
            raise ValueError(
                f"{context}.policyCommitment must match the envelope public inputs"
            )
    if any(
        key in source
        for key in (
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "txDigest",
            "tx_digest",
            "payloadDigest",
            "payload_digest",
        )
    ):
        expected_digest = _normalize_payload_digest(source, context).hex()
        if expected_digest != public_inputs["tx_digest"]:
            raise ValueError(f"{context}.txDigest must match the envelope public inputs")
    for fields, path, normalize, actual in (
        (
            ("accountId", "account_id"),
            "accountId",
            lambda value: _normalize_account_id(value, f"{context}.accountId"),
            public_inputs["account_id"],
        ),
        (
            ("actionClass", "action_class"),
            "actionClass",
            lambda value: _require_non_blank_string(
                value,
                f"{context}.actionClass",
            ),
            public_inputs["action_class"],
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
        (
            ("policyEpoch", "policy_epoch"),
            "policyEpoch",
            lambda value: _normalize_policy_epoch(value, f"{context}.policyEpoch"),
            public_inputs["policy_epoch"],
        ),
    ):
        _key, value = _read_single_alias(source, fields, f"{context}.{path}", path)
        if value is not _MISSING and normalize(value) != actual:
            raise ValueError(f"{context}.{path} must match the envelope public inputs")


def verify_zkat_authenticator_locally(options: Any) -> dict[str, Any]:
    """Verify a deterministic zkAt dev fixture through an OpenVerify envelope."""

    if isinstance(options, Mapping):
        source = _require_plain_mapping(options, "zkAtAuthenticatorLocalVerification")
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        (
            _AUTHENTICATOR_FIELDS
            | {
                "envelope",
                "proofEnvelope",
                "proof_envelope",
                "bytes",
            }
        )
        - {"backend", "backendTag", "backend_tag", "circuitId", "circuit_id", "vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash", "proofBytes", "proof_bytes", "proof", "aux", "maxProofBytes", "max_proof_bytes", "maxPublicInputBytes", "max_public_input_bytes", "version"},
        "zkAtAuthenticatorLocalVerification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "zkAtAuthenticatorLocalVerification.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError("zkAtAuthenticatorLocalVerification.envelope.backend must be Stark")
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "zkAtAuthenticatorLocalVerification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "zkAtAuthenticatorLocalVerification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "zkAtAuthenticatorLocalVerification.publicInputs",
    )
    _ensure_verification_expectations(
        source,
        public_inputs,
        "zkAtAuthenticatorLocalVerification",
    )
    expected_proof = _dev_proof_bytes(
        circuit_id=circuit_id,
        vk_hash=vk_hash,
        public_input_bytes=decoded["public_inputs"],
    )
    if decoded["proof_bytes"] != expected_proof:
        raise ValueError(
            "zkAtAuthenticatorLocalVerification proof bytes are not a valid zkAt dev fixture"
        )
    return {
        "ok": True,
        "production": False,
        "kind": "zkat-dev-fixture-v1",
        "backend": ZKAT_BACKEND,
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "account_id": public_inputs["account_id"],
        "action_class": public_inputs["action_class"],
        "policy_epoch": public_inputs["policy_epoch"],
    }


def verify_zkat_policy_proof_v1(options: Any) -> dict[str, Any]:
    """Validate a production zkAt policy proof envelope binding."""

    if isinstance(options, Mapping):
        source = _require_plain_mapping(options, "zkAtPolicyProofV1Verification")
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        (
            _AUTHENTICATOR_FIELDS
            | {
                "envelope",
                "proofEnvelope",
                "proof_envelope",
                "bytes",
            }
        )
        - {"backend", "backendTag", "backend_tag", "circuitId", "circuit_id", "vkHash", "vk_hash", "verifierKeyHash", "verifyingKeyHash", "proofBytes", "proof_bytes", "proof", "aux", "maxProofBytes", "max_proof_bytes", "maxPublicInputBytes", "max_public_input_bytes", "version"},
        "zkAtPolicyProofV1Verification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "zkAtPolicyProofV1Verification.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError("zkAtPolicyProofV1Verification.envelope.backend must be Stark")
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "zkAtPolicyProofV1Verification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "zkAtPolicyProofV1Verification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "zkAtPolicyProofV1Verification.publicInputs",
    )
    _ensure_verification_expectations(
        source,
        public_inputs,
        "zkAtPolicyProofV1Verification",
    )
    if decoded["proof_bytes"].startswith(ZKAT_DEV_PROOF_PREFIX):
        raise ValueError(
            "zkAtPolicyProofV1Verification proof bytes must not contain a zkAt dev fixture"
        )
    return {
        "ok": True,
        "production": True,
        "kind": "zkat-policy-private-auth-v1",
        "backend": "Stark",
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "account_id": public_inputs["account_id"],
        "action_class": public_inputs["action_class"],
        "policy_epoch": public_inputs["policy_epoch"],
    }


buildZkAtPolicyCommitment = build_zkat_policy_commitment
buildZkAtAuthenticatorEnvelope = build_zkat_authenticator_envelope
buildZkAtPolicyProofV1 = build_zkat_policy_proof_v1
buildZkAtDevProofFixture = build_zkat_dev_proof_fixture
verifyZkAtPolicyProofV1 = verify_zkat_policy_proof_v1
verifyZkAtAuthenticatorLocally = verify_zkat_authenticator_locally
