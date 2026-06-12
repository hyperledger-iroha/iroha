"""ZK-AMS recursive anonymous admission SDK helpers."""

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

ZK_AMS_BACKEND = "stark/fri/sha256-goldilocks"
ZK_AMS_CIRCUIT_ID = "stark/fri/sha256-goldilocks:zk_ams_recursive_admission_v0"
ZK_AMS_DOMAIN_SEPARATOR = "iroha:zk-ams:recursive-admission:v0"
ZK_AMS_DEV_PROOF_PREFIX = b"iroha:zk-ams:dev-fixture:v0:"
ZK_AMS_MAX_ADMISSIONS = 4096
ZK_AMS_MAX_RECURSIVE_PROOF_BYTES = 64 * 1024 * 1024

__all__ = [
    "ZK_AMS_BACKEND",
    "ZK_AMS_CIRCUIT_ID",
    "ZK_AMS_DOMAIN_SEPARATOR",
    "build_zk_ams_admission_batch",
    "build_zk_ams_admission_proof_envelope",
    "build_zk_ams_admission_batch_proof_v0",
    "build_zk_ams_admission_dev_proof_fixture",
    "verify_zk_ams_admission_batch_proof_v0",
    "verify_zk_ams_admission_proof_locally",
    "buildZkAmsAdmissionBatch",
    "buildZkAmsAdmissionProofEnvelope",
    "buildZkAmsAdmissionBatchProofV0",
    "buildZkAmsAdmissionDevProofFixture",
    "verifyZkAmsAdmissionBatchProofV0",
    "verifyZkAmsAdmissionProofLocally",
]


def _normalize_version(value: Any, context: str) -> int:
    version = _positive_u32(1 if value is _MISSING or value is None else value, context)
    if version != 1:
        raise ValueError(f"{context} must be 1")
    return version


def _normalize_zk_ams_backend(value: Any, context: str) -> str:
    _tag, decoded = _normalize_backend(
        ZK_AMS_BACKEND if value is _MISSING or value is None else value,
        context,
    )
    if decoded != "Stark":
        raise ValueError(f"{context} must be {ZK_AMS_BACKEND}")
    return ZK_AMS_BACKEND


def _normalize_circuit_id(value: Any, context: str) -> str:
    circuit_id = _require_non_blank_string(
        ZK_AMS_CIRCUIT_ID if value is _MISSING or value is None else value,
        context,
    )
    if circuit_id not in {ZK_AMS_CIRCUIT_ID, "zk_ams_recursive_admission_v0"}:
        raise ValueError(f"{context} must identify zk_ams_recursive_admission_v0")
    return circuit_id


def _normalize_admission_list(
    value: Any,
    context: str,
    max_items: int,
) -> list[bytes]:
    if not isinstance(value, Sequence) or isinstance(
        value,
        (str, bytes, bytearray, memoryview),
    ):
        raise TypeError(f"{context} must be a sequence")
    if not value:
        raise ValueError(f"{context} must not be empty")
    if len(value) > max_items:
        raise ValueError(f"{context} must contain no more than {max_items} entries")
    entries: list[bytes] = []
    seen: set[bytes] = set()
    for index, entry in enumerate(value):
        normalized = _fixed_bytes(entry, f"{context}[{index}]", 32, nonzero=True)
        if normalized in seen:
            raise ValueError(f"{context} must not contain duplicate entries")
        seen.add(normalized)
        entries.append(normalized)
    return entries


def _normalize_recursive_proof_digest(
    source: Mapping[str, Any],
    context: str,
) -> bytes:
    _digest_key, digest_value = _read_single_alias(
        source,
        ("recursiveProofDigest", "recursive_proof_digest"),
        f"{context}.recursiveProofDigest",
        "recursive proof digest",
    )
    _proof_key, proof_value = _read_single_alias(
        source,
        (
            "recursiveProof",
            "recursiveProofBytes",
            "recursive_proof",
            "recursive_proof_bytes",
        ),
        f"{context}.recursiveProof",
        "recursive proof bytes",
    )
    explicit_digest = (
        None
        if digest_value is _MISSING
        else _fixed_bytes(
            digest_value,
            f"{context}.recursiveProofDigest",
            32,
            nonzero=True,
        )
    )
    if proof_value is _MISSING:
        proof_digest = None
    else:
        max_recursive_proof_bytes = _positive_u32(
            source.get(
                "maxRecursiveProofBytes",
                source.get(
                    "max_recursive_proof_bytes",
                    ZK_AMS_MAX_RECURSIVE_PROOF_BYTES,
                ),
            ),
            f"{context}.maxRecursiveProofBytes",
        )
        proof_digest = hashlib.sha256(
            _bounded_bytes(
                proof_value,
                f"{context}.recursiveProof",
                max_bytes=max_recursive_proof_bytes,
            )
        ).digest()
    if explicit_digest is None and proof_digest is None:
        raise ValueError(
            f"{context}.recursiveProofDigest or {context}.recursiveProof is required"
        )
    if explicit_digest is not None and proof_digest is not None:
        if explicit_digest != proof_digest:
            raise ValueError(
                f"{context}.recursiveProofDigest must match the SHA-256 digest of {context}.recursiveProof"
            )
    return explicit_digest if explicit_digest is not None else proof_digest  # type: ignore[return-value]


def _admission_batch_root_bytes(
    *,
    issuer_root: bytes,
    admission_nullifiers: Sequence[bytes],
    anonymous_account_commitments: Sequence[bytes],
    recursive_proof_digest: bytes,
    domain_separator: str,
) -> bytes:
    payload = {
        "issuer_root": issuer_root.hex(),
        "admission_nullifiers": [entry.hex() for entry in admission_nullifiers],
        "anonymous_account_commitments": [
            entry.hex() for entry in anonymous_account_commitments
        ],
        "recursive_proof_digest": recursive_proof_digest.hex(),
        "domain_separator": domain_separator,
    }
    digest = hashlib.sha256()
    digest.update(b"iroha:zk-ams:admission-batch-root:v0")
    digest.update(b"\x00")
    digest.update(_canonical_json_bytes(payload, "zkAmsAdmissionBatch.root"))
    return digest.digest()


def _normalize_admission_batch_parts(
    source: Mapping[str, Any],
    context: str,
) -> dict[str, Any]:
    _issuer_key, issuer_root_value = _read_single_alias(
        source,
        ("issuerRoot", "issuer_root"),
        f"{context}.issuerRoot",
        "issuer root",
    )
    _batch_key, batch_root_value = _read_single_alias(
        source,
        ("admissionBatchRoot", "admission_batch_root", "batchRoot", "batch_root"),
        f"{context}.admissionBatchRoot",
        "admission batch root",
    )
    _nullifier_key, nullifier_value = _read_single_alias(
        source,
        ("admissionNullifiers", "admission_nullifiers", "nullifiers"),
        f"{context}.admissionNullifiers",
        "admission nullifiers",
    )
    _account_key, account_value = _read_single_alias(
        source,
        (
            "anonymousAccountCommitments",
            "anonymous_account_commitments",
            "accountCommitments",
            "account_commitments",
        ),
        f"{context}.anonymousAccountCommitments",
        "anonymous account commitments",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    max_batch_size = _positive_u32(
        source.get("maxBatchSize", source.get("max_batch_size", ZK_AMS_MAX_ADMISSIONS)),
        f"{context}.maxBatchSize",
    )
    if max_batch_size > ZK_AMS_MAX_ADMISSIONS:
        raise ValueError(
            f"{context}.maxBatchSize must be no greater than {ZK_AMS_MAX_ADMISSIONS}"
        )
    issuer_root = _fixed_bytes(
        issuer_root_value,
        f"{context}.issuerRoot",
        32,
        nonzero=True,
    )
    admission_nullifiers = _normalize_admission_list(
        nullifier_value,
        f"{context}.admissionNullifiers",
        max_batch_size,
    )
    anonymous_account_commitments = _normalize_admission_list(
        account_value,
        f"{context}.anonymousAccountCommitments",
        max_batch_size,
    )
    if len(admission_nullifiers) != len(anonymous_account_commitments):
        raise ValueError(
            f"{context}.admissionNullifiers length must match anonymousAccountCommitments length"
        )
    nullifier_set = set(admission_nullifiers)
    for index, commitment in enumerate(anonymous_account_commitments):
        if commitment in nullifier_set:
            raise ValueError(
                f"{context}.anonymousAccountCommitments[{index}] must not overlap admissionNullifiers"
            )
    recursive_proof_digest = _normalize_recursive_proof_digest(source, context)
    domain_separator = _require_non_blank_string(
        ZK_AMS_DOMAIN_SEPARATOR if domain_value is _MISSING else domain_value,
        f"{context}.domainSeparator",
    )
    derived_batch_root = _admission_batch_root_bytes(
        issuer_root=issuer_root,
        admission_nullifiers=admission_nullifiers,
        anonymous_account_commitments=anonymous_account_commitments,
        recursive_proof_digest=recursive_proof_digest,
        domain_separator=domain_separator,
    )
    explicit_batch_root = (
        None
        if batch_root_value is _MISSING
        else _fixed_bytes(
            batch_root_value,
            f"{context}.admissionBatchRoot",
            32,
            nonzero=True,
        )
    )
    if explicit_batch_root is not None and explicit_batch_root != derived_batch_root:
        raise ValueError(
            f"{context}.admissionBatchRoot must match the derived admission batch root"
        )
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "issuer_root": issuer_root,
        "admission_batch_root": (
            explicit_batch_root if explicit_batch_root is not None else derived_batch_root
        ),
        "admission_nullifiers": admission_nullifiers,
        "anonymous_account_commitments": anonymous_account_commitments,
        "recursive_proof_digest": recursive_proof_digest,
        "domain_separator": domain_separator,
        "batch_size": len(admission_nullifiers),
    }


_BATCH_FIELDS = {
    "version",
    "issuerRoot",
    "issuer_root",
    "admissionBatchRoot",
    "admission_batch_root",
    "batchRoot",
    "batch_root",
    "admissionNullifiers",
    "admission_nullifiers",
    "nullifiers",
    "anonymousAccountCommitments",
    "anonymous_account_commitments",
    "accountCommitments",
    "account_commitments",
    "recursiveProofDigest",
    "recursive_proof_digest",
    "recursiveProof",
    "recursiveProofBytes",
    "recursive_proof",
    "recursive_proof_bytes",
    "domainSeparator",
    "domain_separator",
    "maxBatchSize",
    "max_batch_size",
    "maxRecursiveProofBytes",
    "max_recursive_proof_bytes",
}


def build_zk_ams_admission_batch(options: Mapping[str, Any]) -> dict[str, Any]:
    """Normalize a ZK-AMS recursive admission batch and derive its root."""

    source = _require_plain_mapping(options, "zkAmsAdmissionBatch")
    _reject_unknown_fields(source, _BATCH_FIELDS, "zkAmsAdmissionBatch")
    batch = _normalize_admission_batch_parts(source, "zkAmsAdmissionBatch")
    return {
        "version": batch["version"],
        "issuer_root": batch["issuer_root"],
        "admission_batch_root": batch["admission_batch_root"],
        "admission_nullifiers": batch["admission_nullifiers"],
        "anonymous_account_commitments": batch["anonymous_account_commitments"],
        "recursive_proof_digest": batch["recursive_proof_digest"],
        "domain_separator": batch["domain_separator"],
        "batch_size": batch["batch_size"],
        "root_kind": "dev-sha256-admission-batch-root",
    }


def _normalize_public_inputs(value: Any, context: str) -> dict[str, Any]:
    source = _require_mapping(value, context)
    _reject_unknown_fields(
        source,
        {
            "version",
            "issuer_root",
            "issuerRoot",
            "admission_batch_root",
            "admissionBatchRoot",
            "admission_nullifiers",
            "admissionNullifiers",
            "anonymous_account_commitments",
            "anonymousAccountCommitments",
            "recursive_proof_digest",
            "recursiveProofDigest",
            "domain_separator",
            "domainSeparator",
        },
        context,
    )
    _issuer_key, issuer_root_value = _read_single_alias(
        source,
        ("issuer_root", "issuerRoot"),
        f"{context}.issuerRoot",
        "issuer root",
    )
    _batch_key, batch_root_value = _read_single_alias(
        source,
        ("admission_batch_root", "admissionBatchRoot"),
        f"{context}.admissionBatchRoot",
        "admission batch root",
    )
    _nullifier_key, nullifier_value = _read_single_alias(
        source,
        ("admission_nullifiers", "admissionNullifiers"),
        f"{context}.admissionNullifiers",
        "admission nullifiers",
    )
    _account_key, account_value = _read_single_alias(
        source,
        ("anonymous_account_commitments", "anonymousAccountCommitments"),
        f"{context}.anonymousAccountCommitments",
        "anonymous account commitments",
    )
    _proof_key, proof_digest_value = _read_single_alias(
        source,
        ("recursive_proof_digest", "recursiveProofDigest"),
        f"{context}.recursiveProofDigest",
        "recursive proof digest",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domain_separator", "domainSeparator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    batch = _normalize_admission_batch_parts(
        {
            "version": source.get("version", _MISSING),
            "issuerRoot": issuer_root_value,
            "admissionBatchRoot": batch_root_value,
            "admissionNullifiers": nullifier_value,
            "anonymousAccountCommitments": account_value,
            "recursiveProofDigest": proof_digest_value,
            "domainSeparator": domain_value,
        },
        context,
    )
    return {
        "version": batch["version"],
        "issuer_root": batch["issuer_root"].hex(),
        "admission_batch_root": batch["admission_batch_root"].hex(),
        "admission_nullifiers": [
            entry.hex() for entry in batch["admission_nullifiers"]
        ],
        "anonymous_account_commitments": [
            entry.hex() for entry in batch["anonymous_account_commitments"]
        ],
        "recursive_proof_digest": batch["recursive_proof_digest"].hex(),
        "domain_separator": batch["domain_separator"],
    }


def _normalize_admission_proof_parts(
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
    batch = _normalize_admission_batch_parts(source, context)
    public_inputs = {
        "version": batch["version"],
        "issuer_root": batch["issuer_root"].hex(),
        "admission_batch_root": batch["admission_batch_root"].hex(),
        "admission_nullifiers": [
            entry.hex() for entry in batch["admission_nullifiers"]
        ],
        "anonymous_account_commitments": [
            entry.hex() for entry in batch["anonymous_account_commitments"]
        ],
        "recursive_proof_digest": batch["recursive_proof_digest"].hex(),
        "domain_separator": batch["domain_separator"],
    }
    max_proof_bytes = _positive_u32(
        source.get("maxProofBytes", source.get("max_proof_bytes", DEFAULT_PRIVACY_MAX_PROOF_BYTES)),
        f"{context}.maxProofBytes",
    )
    return {
        "backend": _normalize_zk_ams_backend(backend_value, f"{context}.backendTag"),
        "circuit_id": _normalize_circuit_id(circuit_value, f"{context}.circuitId"),
        "vk_hash": _fixed_bytes(vk_hash_value, f"{context}.vkHash", 32, nonzero=True),
        "batch": batch,
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


_PROOF_FIELDS = {
    *_BATCH_FIELDS,
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


def build_zk_ams_admission_proof_envelope(options: Mapping[str, Any]) -> bytes:
    """Build canonical OpenVerifyEnvelope bytes for a prepared ZK-AMS proof."""

    source = _require_plain_mapping(options, "zkAmsAdmissionProofEnvelope")
    _reject_unknown_fields(source, _PROOF_FIELDS, "zkAmsAdmissionProofEnvelope")
    parts = _normalize_admission_proof_parts(
        source,
        "zkAmsAdmissionProofEnvelope",
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


def build_zk_ams_admission_batch_proof_v0(options: Mapping[str, Any]) -> bytes:
    """Build canonical production ZK-AMS recursive admission proof bytes."""

    source = _require_plain_mapping(options, "zkAmsAdmissionBatchProofV0")
    _reject_unknown_fields(source, _PROOF_FIELDS, "zkAmsAdmissionBatchProofV0")
    parts = _normalize_admission_proof_parts(
        source,
        "zkAmsAdmissionBatchProofV0",
        require_proof_bytes=True,
    )
    if parts["proof_bytes"].startswith(ZK_AMS_DEV_PROOF_PREFIX):
        raise ValueError(
            "zkAmsAdmissionBatchProofV0.proofBytes must not contain a dev fixture proof"
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
    digest.update(b"iroha:zk-ams:dev-fixture:v0")
    digest.update(b"\x00")
    digest.update(circuit_id.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(vk_hash)
    digest.update(b"\x00")
    digest.update(public_input_bytes)
    return ZK_AMS_DEV_PROOF_PREFIX + digest.digest()


def build_zk_ams_admission_dev_proof_fixture(options: Mapping[str, Any]) -> dict[str, Any]:
    """Build a deterministic ZK-AMS dev proof fixture."""

    source = _require_plain_mapping(options, "zkAmsAdmissionDevProofFixture")
    _reject_unknown_fields(
        source,
        _PROOF_FIELDS - {"proofBytes", "proof_bytes", "proof"},
        "zkAmsAdmissionDevProofFixture",
    )
    parts = _normalize_admission_proof_parts(
        source,
        "zkAmsAdmissionDevProofFixture",
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
    batch = parts["batch"]
    return {
        "kind": "zk-ams-dev-fixture-v0",
        "production": False,
        "proof_bytes": proof_bytes,
        "proofBytes": proof_bytes,
        "batch": {
            "version": batch["version"],
            "issuer_root": batch["issuer_root"],
            "admission_batch_root": batch["admission_batch_root"],
            "admission_nullifiers": batch["admission_nullifiers"],
            "anonymous_account_commitments": batch["anonymous_account_commitments"],
            "recursive_proof_digest": batch["recursive_proof_digest"],
            "domain_separator": batch["domain_separator"],
            "batch_size": batch["batch_size"],
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


def _ensure_verification_expectations(
    source: Mapping[str, Any],
    public_inputs: Mapping[str, Any],
    context: str,
) -> None:
    scalar_checks = (
        (
            ("issuerRoot", "issuer_root"),
            "issuerRoot",
            lambda value: _fixed_bytes(
                value,
                f"{context}.issuerRoot",
                32,
                nonzero=True,
            ).hex(),
            public_inputs["issuer_root"],
        ),
        (
            ("admissionBatchRoot", "admission_batch_root", "batchRoot", "batch_root"),
            "admissionBatchRoot",
            lambda value: _fixed_bytes(
                value,
                f"{context}.admissionBatchRoot",
                32,
                nonzero=True,
            ).hex(),
            public_inputs["admission_batch_root"],
        ),
        (
            (
                "recursiveProofDigest",
                "recursive_proof_digest",
                "recursiveProof",
                "recursiveProofBytes",
                "recursive_proof",
                "recursive_proof_bytes",
            ),
            "recursiveProofDigest",
            lambda _value: _normalize_recursive_proof_digest(source, context).hex(),
            public_inputs["recursive_proof_digest"],
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
    )
    for fields, path, normalize, actual in scalar_checks:
        _key, value = _read_single_alias(source, fields, f"{context}.{path}", path)
        if value is not _MISSING and normalize(value) != actual:
            raise ValueError(f"{context}.{path} must match the envelope public inputs")
    max_batch_size = _positive_u32(
        source.get("maxBatchSize", source.get("max_batch_size", ZK_AMS_MAX_ADMISSIONS)),
        f"{context}.maxBatchSize",
    )
    if max_batch_size > ZK_AMS_MAX_ADMISSIONS:
        raise ValueError(
            f"{context}.maxBatchSize must be no greater than {ZK_AMS_MAX_ADMISSIONS}"
        )
    _nullifier_key, nullifier_value = _read_single_alias(
        source,
        ("admissionNullifiers", "admission_nullifiers", "nullifiers"),
        f"{context}.admissionNullifiers",
        "admission nullifiers",
    )
    if nullifier_value is not _MISSING:
        expected = [
            entry.hex()
            for entry in _normalize_admission_list(
                nullifier_value,
                f"{context}.admissionNullifiers",
                max_batch_size,
            )
        ]
        if expected != list(public_inputs["admission_nullifiers"]):
            raise ValueError(
                f"{context}.admissionNullifiers must match the envelope public inputs"
            )
    _account_key, account_value = _read_single_alias(
        source,
        (
            "anonymousAccountCommitments",
            "anonymous_account_commitments",
            "accountCommitments",
            "account_commitments",
        ),
        f"{context}.anonymousAccountCommitments",
        "anonymous account commitments",
    )
    if account_value is not _MISSING:
        expected = [
            entry.hex()
            for entry in _normalize_admission_list(
                account_value,
                f"{context}.anonymousAccountCommitments",
                max_batch_size,
            )
        ]
        if expected != list(public_inputs["anonymous_account_commitments"]):
            raise ValueError(
                f"{context}.anonymousAccountCommitments must match the envelope public inputs"
            )


def verify_zk_ams_admission_proof_locally(options: Any) -> dict[str, Any]:
    """Verify a deterministic ZK-AMS dev fixture through an OpenVerify envelope."""

    if isinstance(options, Mapping):
        source = _require_plain_mapping(options, "zkAmsAdmissionLocalVerification")
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        _BATCH_FIELDS
        | {
            "envelope",
            "proofEnvelope",
            "proof_envelope",
            "bytes",
        },
        "zkAmsAdmissionLocalVerification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "zkAmsAdmissionLocalVerification.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError("zkAmsAdmissionLocalVerification.envelope.backend must be Stark")
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "zkAmsAdmissionLocalVerification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "zkAmsAdmissionLocalVerification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "zkAmsAdmissionLocalVerification.publicInputs",
    )
    _ensure_verification_expectations(
        source,
        public_inputs,
        "zkAmsAdmissionLocalVerification",
    )
    expected_proof = _dev_proof_bytes(
        circuit_id=circuit_id,
        vk_hash=vk_hash,
        public_input_bytes=decoded["public_inputs"],
    )
    if decoded["proof_bytes"] != expected_proof:
        raise ValueError(
            "zkAmsAdmissionLocalVerification proof bytes are not a valid ZK-AMS dev fixture"
        )
    return {
        "ok": True,
        "production": False,
        "kind": "zk-ams-dev-fixture-v0",
        "backend": ZK_AMS_BACKEND,
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "admission_batch_root": public_inputs["admission_batch_root"],
        "batch_size": len(public_inputs["admission_nullifiers"]),
    }


def verify_zk_ams_admission_batch_proof_v0(options: Any) -> dict[str, Any]:
    """Verify production ZK-AMS recursive admission proof envelope structure."""

    if isinstance(options, Mapping):
        source = _require_plain_mapping(options, "zkAmsAdmissionBatchProofV0")
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        _BATCH_FIELDS
        | {
            "envelope",
            "proofEnvelope",
            "proof_envelope",
            "bytes",
            "maxProofBytes",
            "max_proof_bytes",
            "maxPublicInputBytes",
            "max_public_input_bytes",
            "version",
        },
        "zkAmsAdmissionBatchProofV0",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "zkAmsAdmissionBatchProofV0.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError("zkAmsAdmissionBatchProofV0.envelope.backend must be Stark")
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "zkAmsAdmissionBatchProofV0.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "zkAmsAdmissionBatchProofV0.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "zkAmsAdmissionBatchProofV0.publicInputs",
    )
    _ensure_verification_expectations(
        source,
        public_inputs,
        "zkAmsAdmissionBatchProofV0",
    )
    if decoded["proof_bytes"].startswith(ZK_AMS_DEV_PROOF_PREFIX):
        raise ValueError(
            "zkAmsAdmissionBatchProofV0 proof bytes must not contain a ZK-AMS dev fixture"
        )
    return {
        "ok": True,
        "production": True,
        "kind": "zk-ams-recursive-admission-v0",
        "backend": "Stark",
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "admission_batch_root": public_inputs["admission_batch_root"],
        "batch_size": len(public_inputs["admission_nullifiers"]),
    }


buildZkAmsAdmissionBatch = build_zk_ams_admission_batch
buildZkAmsAdmissionProofEnvelope = build_zk_ams_admission_proof_envelope
buildZkAmsAdmissionBatchProofV0 = build_zk_ams_admission_batch_proof_v0
buildZkAmsAdmissionDevProofFixture = build_zk_ams_admission_dev_proof_fixture
verifyZkAmsAdmissionBatchProofV0 = verify_zk_ams_admission_batch_proof_v0
verifyZkAmsAdmissionProofLocally = verify_zk_ams_admission_proof_locally
