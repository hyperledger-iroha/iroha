"""Anonymous PGC SDK helper and proof-envelope builders."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from typing import Any

from .verange import (
    DEFAULT_PRIVACY_MAX_PROOF_BYTES,
    DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES,
    VERANGE_MAX_PAYLOAD_BYTES,
    _MISSING,
    _bounded_bytes,
    _canonical_json_bytes,
    _fixed_bytes,
    _normalize_backend,
    _optional_aux_value,
    _positive_u32,
    _read_single_alias,
    _reject_unknown_fields,
    _require_mapping,
    _require_non_blank_string,
    build_privacy_proof_envelope,
    decode_privacy_proof_envelope,
)

ANONYMOUS_PGC_BACKEND = "stark/fri/sha256-goldilocks"
ANONYMOUS_PGC_CIRCUIT_ID = (
    "stark/fri/sha256-goldilocks:anonymous_pgc_k_out_of_n_v1"
)
ANONYMOUS_PGC_DOMAIN_SEPARATOR = "iroha:anonymous-pgc:k-out-of-n:v1"
ANONYMOUS_PGC_DEV_PROOF_PREFIX = b"iroha:anonymous-pgc:dev-fixture:v1:"
ANONYMOUS_PGC_MAX_RECEIVERS = 64
ANONYMOUS_PGC_MAX_BALANCE_COMMITMENTS = 64
ANONYMOUS_PGC_MAX_RANGE_COMMITMENTS = 64
ANONYMOUS_PGC_MAX_CIPHERTEXT_BYTES = 64 * 1024

__all__ = [
    "ANONYMOUS_PGC_BACKEND",
    "ANONYMOUS_PGC_CIRCUIT_ID",
    "ANONYMOUS_PGC_DOMAIN_SEPARATOR",
    "build_anonymous_pgc_receiver_set",
    "build_anonymous_pgc_account_commitment_instruction",
    "build_anonymous_pgc_k_out_of_n_proof_v1",
    "verify_anonymous_pgc_k_out_of_n_proof_v1",
    "build_anonymous_pgc_transfer_instruction",
    "build_anonymous_pgc_dev_proof_fixture",
    "verify_anonymous_pgc_dev_proof_locally",
    "buildAnonymousPgcReceiverSet",
    "buildAnonymousPgcAccountCommitmentInstruction",
    "buildAnonymousPgcKOutOfNProofV1",
    "verifyAnonymousPgcKOutOfNProofV1",
    "buildAnonymousPgcTransferInstruction",
    "buildAnonymousPgcDevProofFixture",
    "verifyAnonymousPgcDevProofLocally",
]


def _normalize_version(value: Any, context: str) -> int:
    version = _positive_u32(1 if value is _MISSING or value is None else value, context)
    if version != 1:
        raise ValueError(f"{context} must be 1")
    return version


def _normalize_anonymous_pgc_backend(value: Any, context: str) -> str:
    _tag, decoded = _normalize_backend(
        ANONYMOUS_PGC_BACKEND if value is _MISSING or value is None else value,
        context,
    )
    if decoded != "Stark":
        raise ValueError(f"{context} must be {ANONYMOUS_PGC_BACKEND}")
    return ANONYMOUS_PGC_BACKEND


def _normalize_circuit_id(value: Any, context: str) -> str:
    circuit_id = _require_non_blank_string(
        ANONYMOUS_PGC_CIRCUIT_ID if value is _MISSING or value is None else value,
        context,
    )
    if circuit_id not in {
        ANONYMOUS_PGC_CIRCUIT_ID,
        "anonymous_pgc_k_out_of_n_v1",
    }:
        raise ValueError(f"{context} must identify anonymous_pgc_k_out_of_n_v1")
    return circuit_id


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


def _normalize_commitment_list(
    value: Any,
    context: str,
    max_items: int,
) -> list[bytes]:
    if not isinstance(value, Sequence) or isinstance(
        value,
        (str, bytes, bytearray, memoryview),
    ):
        raise TypeError(f"{context} must be a non-empty sequence")
    if not value:
        raise ValueError(f"{context} must be non-empty")
    if len(value) > max_items:
        raise ValueError(f"{context} must contain at most {max_items} entries")
    commitments: list[bytes] = []
    for index, entry in enumerate(value):
        raw = entry
        if isinstance(entry, Mapping):
            _commitment_key, raw = _read_single_alias(
                entry,
                (
                    "commitment",
                    "rangeCommitment",
                    "range_commitment",
                    "valueCommitment",
                    "value_commitment",
                ),
                f"{context}[{index}].commitment",
                "commitment",
            )
        commitments.append(_fixed_bytes(raw, f"{context}[{index}]", 32, nonzero=True))
    seen: set[bytes] = set()
    for commitment in commitments:
        if commitment in seen:
            raise ValueError(f"{context} must not contain duplicates")
        seen.add(commitment)
    return commitments


def _normalize_receiver_entry(value: Any, context: str) -> dict[str, bytes]:
    source = _require_mapping(value, context)
    _reject_unknown_fields(
        source,
        {
            "accountCommitment",
            "account_commitment",
            "receiverCommitment",
            "receiver_commitment",
            "ciphertextCommitment",
            "ciphertext_commitment",
            "receiverCiphertextCommitment",
            "receiver_ciphertext_commitment",
            "ciphertext",
            "receiverCiphertext",
            "receiver_ciphertext",
            "encryptedNote",
            "encrypted_note",
            "ciphertextDigest",
            "ciphertext_digest",
        },
        context,
    )
    _account_key, account_value = _read_single_alias(
        source,
        (
            "accountCommitment",
            "account_commitment",
            "receiverCommitment",
            "receiver_commitment",
        ),
        f"{context}.accountCommitment",
        "receiver account commitment",
    )
    _ciphertext_commitment_key, ciphertext_commitment_value = _read_single_alias(
        source,
        (
            "ciphertextCommitment",
            "ciphertext_commitment",
            "receiverCiphertextCommitment",
            "receiver_ciphertext_commitment",
        ),
        f"{context}.ciphertextCommitment",
        "receiver ciphertext commitment",
    )
    _ciphertext_key, ciphertext_value = _read_single_alias(
        source,
        (
            "ciphertext",
            "receiverCiphertext",
            "receiver_ciphertext",
            "encryptedNote",
            "encrypted_note",
        ),
        f"{context}.ciphertext",
        "receiver ciphertext",
    )
    _digest_key, digest_value = _read_single_alias(
        source,
        ("ciphertextDigest", "ciphertext_digest"),
        f"{context}.ciphertextDigest",
        "receiver ciphertext digest",
    )
    receiver = {
        "account_commitment": _fixed_bytes(
            account_value,
            f"{context}.accountCommitment",
            32,
            nonzero=True,
        ),
        "ciphertext_commitment": _fixed_bytes(
            ciphertext_commitment_value,
            f"{context}.ciphertextCommitment",
            32,
            nonzero=True,
        ),
    }
    supplied_digest = (
        None
        if digest_value is _MISSING
        else _fixed_bytes(
            digest_value,
            f"{context}.ciphertextDigest",
            32,
            nonzero=True,
        )
    )
    computed_digest = (
        None
        if ciphertext_value is _MISSING
        else hashlib.sha256(
            _bounded_bytes(
                ciphertext_value,
                f"{context}.ciphertext",
                max_bytes=ANONYMOUS_PGC_MAX_CIPHERTEXT_BYTES,
            )
        ).digest()
    )
    if supplied_digest is not None and computed_digest is not None:
        if supplied_digest != computed_digest:
            raise ValueError(
                f"{context}.ciphertextDigest must match the SHA-256 digest of {context}.ciphertext"
            )
    if supplied_digest is not None or computed_digest is not None:
        receiver["ciphertext_digest"] = (
            supplied_digest if supplied_digest is not None else computed_digest
        )  # type: ignore[assignment]
    return receiver


def _receiver_set_commitment(
    *,
    version: int,
    threshold: int,
    receivers: Sequence[Mapping[str, bytes]],
) -> bytes:
    payload = {
        "version": version,
        "receiver_count": len(receivers),
        "threshold": threshold,
        "receivers": [
            {
                "account_commitment": entry["account_commitment"].hex(),
                "ciphertext_commitment": entry["ciphertext_commitment"].hex(),
            }
            for entry in receivers
        ],
    }
    digest = hashlib.sha256()
    digest.update(b"iroha:anonymous-pgc:receiver-set:v1")
    digest.update(b"\x00")
    digest.update(_canonical_json_bytes(payload, "anonymousPgcReceiverSet.commitment"))
    return digest.digest()


def build_anonymous_pgc_receiver_set(
    options: Mapping[str, Any],
    context: str = "anonymousPgcReceiverSet",
) -> dict[str, Any]:
    """Normalize a prepared Anonymous PGC receiver set descriptor.

    This helper does not generate receiver ciphertexts or a production
    Anonymous PGC proof; callers must provide commitments produced by their
    wallet/prover.
    """

    source = _require_mapping(options, context)
    _reject_unknown_fields(source, {"version", "threshold", "k", "receivers"}, context)
    _threshold_key, threshold_value = _read_single_alias(
        source,
        ("threshold", "k"),
        f"{context}.threshold",
        "receiver threshold",
    )
    receivers_value = source.get("receivers", _MISSING)
    if not isinstance(receivers_value, Sequence) or isinstance(
        receivers_value,
        (str, bytes, bytearray, memoryview),
    ):
        raise TypeError(f"{context}.receivers must be a non-empty sequence")
    if not receivers_value:
        raise ValueError(f"{context}.receivers must be non-empty")
    if len(receivers_value) > ANONYMOUS_PGC_MAX_RECEIVERS:
        raise ValueError(
            f"{context}.receivers must contain at most {ANONYMOUS_PGC_MAX_RECEIVERS} entries"
        )
    version = _normalize_version(source.get("version", _MISSING), f"{context}.version")
    receivers = [
        _normalize_receiver_entry(entry, f"{context}.receivers[{index}]")
        for index, entry in enumerate(receivers_value)
    ]
    threshold = _positive_u32(
        len(receivers) if threshold_value is _MISSING else threshold_value,
        f"{context}.threshold",
    )
    if threshold > len(receivers):
        raise ValueError(f"{context}.threshold must not exceed receivers length")
    seen_accounts: set[bytes] = set()
    seen_ciphertexts: set[bytes] = set()
    for receiver in receivers:
        account_commitment = receiver["account_commitment"]
        ciphertext_commitment = receiver["ciphertext_commitment"]
        if account_commitment in seen_accounts:
            raise ValueError(
                f"{context}.receivers must not contain duplicate account commitments"
            )
        if ciphertext_commitment in seen_ciphertexts:
            raise ValueError(
                f"{context}.receivers must not contain duplicate ciphertext commitments"
            )
        seen_accounts.add(account_commitment)
        seen_ciphertexts.add(ciphertext_commitment)
    receiver_set = {
        "version": version,
        "threshold": threshold,
        "receiver_count": len(receivers),
        "receivers": receivers,
    }
    receiver_set["receiver_set_commitment"] = _receiver_set_commitment(
        version=version,
        threshold=threshold,
        receivers=receivers,
    )
    return receiver_set


def _normalize_receiver_set(value: Any, context: str) -> dict[str, Any]:
    source = _require_mapping(value, context)
    rebuilt = build_anonymous_pgc_receiver_set(
        {
            key: source[key]
            for key in ("version", "threshold", "k", "receivers")
            if key in source
        },
        context,
    )
    _commitment_key, commitment_value = _read_single_alias(
        source,
        ("receiver_set_commitment", "receiverSetCommitment"),
        f"{context}.receiverSetCommitment",
        "receiver-set commitment",
    )
    if commitment_value is not _MISSING:
        supplied = _fixed_bytes(
            commitment_value,
            f"{context}.receiverSetCommitment",
            32,
            nonzero=True,
        )
        if supplied != rebuilt["receiver_set_commitment"]:
            raise ValueError(
                f"{context}.receiverSetCommitment must match receivers and threshold"
            )
    _count_key, count_value = _read_single_alias(
        source,
        ("receiver_count", "receiverCount"),
        f"{context}.receiverCount",
        "receiver count",
    )
    if count_value is not _MISSING:
        if _positive_u32(count_value, f"{context}.receiverCount") != rebuilt["receiver_count"]:
            raise ValueError(f"{context}.receiverCount must match receivers length")
    return rebuilt


def _normalize_receiver_set_from_source(
    source: Mapping[str, Any],
    context: str,
) -> dict[str, Any]:
    _receiver_set_key, receiver_set_value = _read_single_alias(
        source,
        ("receiverSet", "receiver_set"),
        f"{context}.receiverSet",
        "receiver set",
    )
    if receiver_set_value is not _MISSING:
        return _normalize_receiver_set(receiver_set_value, f"{context}.receiverSet")
    return build_anonymous_pgc_receiver_set(
        {
            key: source[key]
            for key in ("version", "threshold", "k", "receivers")
            if key in source
        },
        f"{context}.receiverSet",
    )


def _normalize_public_inputs(value: Any, context: str) -> dict[str, Any]:
    source = _require_mapping(value, context)
    _reject_unknown_fields(
        source,
        {
            "version",
            "anonymity_set_root",
            "anonymitySetRoot",
            "tx_digest",
            "txDigest",
            "balance_commitments",
            "balanceCommitments",
            "receiver_set_commitment",
            "receiverSetCommitment",
            "receiver_ciphertext_commitments",
            "receiverCiphertextCommitments",
            "receiver_threshold",
            "receiverThreshold",
            "receiver_count",
            "receiverCount",
            "link_tag",
            "linkTag",
            "range_commitments",
            "rangeCommitments",
            "chain_id",
            "chainId",
            "domain_separator",
            "domainSeparator",
        },
        context,
    )
    _root_key, root_value = _read_single_alias(
        source,
        ("anonymity_set_root", "anonymitySetRoot"),
        f"{context}.anonymitySetRoot",
        "anonymity set root",
    )
    _tx_key, tx_value = _read_single_alias(
        source,
        ("tx_digest", "txDigest"),
        f"{context}.txDigest",
        "transaction digest",
    )
    _balance_key, balance_value = _read_single_alias(
        source,
        ("balance_commitments", "balanceCommitments"),
        f"{context}.balanceCommitments",
        "balance commitments",
    )
    _receiver_set_key, receiver_set_value = _read_single_alias(
        source,
        ("receiver_set_commitment", "receiverSetCommitment"),
        f"{context}.receiverSetCommitment",
        "receiver-set commitment",
    )
    _ciphertexts_key, ciphertexts_value = _read_single_alias(
        source,
        ("receiver_ciphertext_commitments", "receiverCiphertextCommitments"),
        f"{context}.receiverCiphertextCommitments",
        "receiver ciphertext commitments",
    )
    _threshold_key, threshold_value = _read_single_alias(
        source,
        ("receiver_threshold", "receiverThreshold"),
        f"{context}.receiverThreshold",
        "receiver threshold",
    )
    _count_key, count_value = _read_single_alias(
        source,
        ("receiver_count", "receiverCount"),
        f"{context}.receiverCount",
        "receiver count",
    )
    _link_key, link_value = _read_single_alias(
        source,
        ("link_tag", "linkTag"),
        f"{context}.linkTag",
        "link tag",
    )
    _range_key, range_value = _read_single_alias(
        source,
        ("range_commitments", "rangeCommitments"),
        f"{context}.rangeCommitments",
        "range commitments",
    )
    _chain_key, chain_value = _read_single_alias(
        source,
        ("chain_id", "chainId"),
        f"{context}.chainId",
        "chain id",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domain_separator", "domainSeparator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    receiver_ciphertext_commitments = _normalize_commitment_list(
        ciphertexts_value,
        f"{context}.receiverCiphertextCommitments",
        ANONYMOUS_PGC_MAX_RECEIVERS,
    )
    receiver_threshold = _positive_u32(
        threshold_value,
        f"{context}.receiverThreshold",
    )
    receiver_count = _positive_u32(count_value, f"{context}.receiverCount")
    if receiver_count != len(receiver_ciphertext_commitments):
        raise ValueError(
            f"{context}.receiverCount must match receiverCiphertextCommitments length"
        )
    if receiver_threshold > receiver_count:
        raise ValueError(f"{context}.receiverThreshold must not exceed receiverCount")
    return {
        "version": _normalize_version(source.get("version", _MISSING), f"{context}.version"),
        "anonymity_set_root": _fixed_bytes(
            root_value,
            f"{context}.anonymitySetRoot",
            32,
            nonzero=True,
        ).hex(),
        "tx_digest": _fixed_bytes(
            tx_value,
            f"{context}.txDigest",
            32,
            nonzero=True,
        ).hex(),
        "balance_commitments": [
            entry.hex()
            for entry in _normalize_commitment_list(
                balance_value,
                f"{context}.balanceCommitments",
                ANONYMOUS_PGC_MAX_BALANCE_COMMITMENTS,
            )
        ],
        "receiver_set_commitment": _fixed_bytes(
            receiver_set_value,
            f"{context}.receiverSetCommitment",
            32,
            nonzero=True,
        ).hex(),
        "receiver_ciphertext_commitments": [
            entry.hex() for entry in receiver_ciphertext_commitments
        ],
        "receiver_threshold": receiver_threshold,
        "receiver_count": receiver_count,
        "link_tag": _fixed_bytes(
            link_value,
            f"{context}.linkTag",
            32,
            nonzero=True,
        ).hex(),
        "range_commitments": [
            entry.hex()
            for entry in _normalize_commitment_list(
                range_value,
                f"{context}.rangeCommitments",
                ANONYMOUS_PGC_MAX_RANGE_COMMITMENTS,
            )
        ],
        "chain_id": _require_non_blank_string(chain_value, f"{context}.chainId"),
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
    _root_key, root_value = _read_single_alias(
        source,
        ("anonymitySetRoot", "anonymity_set_root"),
        f"{context}.anonymitySetRoot",
        "anonymity set root",
    )
    _balance_key, balance_value = _read_single_alias(
        source,
        ("balanceCommitments", "balance_commitments"),
        f"{context}.balanceCommitments",
        "balance commitments",
    )
    _link_key, link_value = _read_single_alias(
        source,
        ("linkTag", "link_tag"),
        f"{context}.linkTag",
        "link tag",
    )
    _range_key, range_value = _read_single_alias(
        source,
        ("rangeCommitments", "range_commitments"),
        f"{context}.rangeCommitments",
        "range commitments",
    )
    _chain_key, chain_value = _read_single_alias(
        source,
        ("chainId", "chain_id"),
        f"{context}.chainId",
        "chain id",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        f"{context}.domainSeparator",
        "domain separator",
    )
    receiver_set = _normalize_receiver_set_from_source(source, context)
    balance_commitments = _normalize_commitment_list(
        balance_value,
        f"{context}.balanceCommitments",
        ANONYMOUS_PGC_MAX_BALANCE_COMMITMENTS,
    )
    range_commitments = _normalize_commitment_list(
        range_value,
        f"{context}.rangeCommitments",
        ANONYMOUS_PGC_MAX_RANGE_COMMITMENTS,
    )
    public_inputs = {
        "version": 1,
        "anonymity_set_root": _fixed_bytes(
            root_value,
            f"{context}.anonymitySetRoot",
            32,
            nonzero=True,
        ).hex(),
        "tx_digest": _normalize_payload_digest(source, context).hex(),
        "balance_commitments": [entry.hex() for entry in balance_commitments],
        "receiver_set_commitment": receiver_set["receiver_set_commitment"].hex(),
        "receiver_ciphertext_commitments": [
            entry["ciphertext_commitment"].hex() for entry in receiver_set["receivers"]
        ],
        "receiver_threshold": receiver_set["threshold"],
        "receiver_count": receiver_set["receiver_count"],
        "link_tag": _fixed_bytes(
            link_value,
            f"{context}.linkTag",
            32,
            nonzero=True,
        ).hex(),
        "range_commitments": [entry.hex() for entry in range_commitments],
        "chain_id": _require_non_blank_string(
            chain_value,
            f"{context}.chainId",
        ),
        "domain_separator": _require_non_blank_string(
            ANONYMOUS_PGC_DOMAIN_SEPARATOR
            if domain_value is _MISSING
            else domain_value,
            f"{context}.domainSeparator",
        ),
    }
    max_proof_bytes = _positive_u32(
        source.get("maxProofBytes", source.get("max_proof_bytes", DEFAULT_PRIVACY_MAX_PROOF_BYTES)),
        f"{context}.maxProofBytes",
    )
    return {
        "backend": _normalize_anonymous_pgc_backend(
            backend_value,
            f"{context}.backendTag",
        ),
        "circuit_id": _normalize_circuit_id(circuit_value, f"{context}.circuitId"),
        "vk_hash": _fixed_bytes(vk_hash_value, f"{context}.vkHash", 32, nonzero=True),
        "receiver_set": receiver_set,
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


def _dev_proof_bytes(
    *,
    circuit_id: str,
    vk_hash: bytes,
    public_input_bytes: bytes,
) -> bytes:
    digest = hashlib.sha256()
    digest.update(b"iroha:anonymous-pgc:dev-fixture:v1")
    digest.update(b"\x00")
    digest.update(circuit_id.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(vk_hash)
    digest.update(b"\x00")
    digest.update(public_input_bytes)
    return ANONYMOUS_PGC_DEV_PROOF_PREFIX + digest.digest()


def _anonymous_pgc_proof_allowed_fields() -> set[str]:
    return {
        "backend",
        "backendTag",
        "backend_tag",
        "circuitId",
        "circuit_id",
        "vkHash",
        "vk_hash",
        "verifierKeyHash",
        "verifyingKeyHash",
        "receiverSet",
        "receiver_set",
        "version",
        "threshold",
        "k",
        "receivers",
        "anonymitySetRoot",
        "anonymity_set_root",
        "txDigest",
        "tx_digest",
        "payloadDigest",
        "payload_digest",
        "payload",
        "payloadBytes",
        "payload_bytes",
        "payloadJson",
        "payload_json",
        "balanceCommitments",
        "balance_commitments",
        "linkTag",
        "link_tag",
        "rangeCommitments",
        "range_commitments",
        "chainId",
        "chain_id",
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
    }


def build_anonymous_pgc_k_out_of_n_proof_v1(options: Mapping[str, Any]) -> bytes:
    """Build a production Anonymous PGC proof envelope from prover output."""

    source = _require_mapping(options, "anonymousPgcKOutOfNProofV1")
    _reject_unknown_fields(
        source,
        _anonymous_pgc_proof_allowed_fields(),
        "anonymousPgcKOutOfNProofV1",
    )
    parts = _normalize_proof_parts(
        source,
        "anonymousPgcKOutOfNProofV1",
        require_proof_bytes=True,
    )
    proof_bytes = parts["proof_bytes"]
    if proof_bytes.startswith(ANONYMOUS_PGC_DEV_PROOF_PREFIX):
        raise ValueError(
            "anonymousPgcKOutOfNProofV1.proofBytes must not contain an Anonymous PGC dev fixture"
        )
    aux = _bounded_bytes(
        _optional_aux_value(source, "anonymousPgcKOutOfNProofV1"),
        "anonymousPgcKOutOfNProofV1.aux",
        max_bytes=64 * 1024,
        allow_empty=True,
    )
    return build_privacy_proof_envelope(
        {
            "backend": parts["backend"],
            "circuitId": parts["circuit_id"],
            "vkHash": parts["vk_hash"],
            "publicInputs": parts["public_input_bytes"],
            "proofBytes": proof_bytes,
            "aux": aux,
            "maxProofBytes": parts["max_proof_bytes"],
            "maxPublicInputBytes": parts["max_public_input_bytes"],
        }
    )


def build_anonymous_pgc_dev_proof_fixture(options: Mapping[str, Any]) -> dict[str, Any]:
    """Build a deterministic Anonymous PGC dev fixture.

    The returned envelope verifies binding of public inputs only. It is not a
    production Anonymous PGC proof.
    """

    source = _require_mapping(options, "anonymousPgcDevProofFixture")
    _reject_unknown_fields(
        source,
        {
            "backend",
            "backendTag",
            "backend_tag",
            "circuitId",
            "circuit_id",
            "vkHash",
            "vk_hash",
            "verifierKeyHash",
            "verifyingKeyHash",
            "receiverSet",
            "receiver_set",
            "version",
            "threshold",
            "k",
            "receivers",
            "anonymitySetRoot",
            "anonymity_set_root",
            "txDigest",
            "tx_digest",
            "payloadDigest",
            "payload_digest",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "balanceCommitments",
            "balance_commitments",
            "linkTag",
            "link_tag",
            "rangeCommitments",
            "range_commitments",
            "chainId",
            "chain_id",
            "domainSeparator",
            "domain_separator",
            "aux",
            "maxProofBytes",
            "max_proof_bytes",
            "maxPublicInputBytes",
            "max_public_input_bytes",
            "maxPayloadBytes",
            "max_payload_bytes",
        },
        "anonymousPgcDevProofFixture",
    )
    parts = _normalize_proof_parts(
        source,
        "anonymousPgcDevProofFixture",
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
        "kind": "anonymous-pgc-dev-fixture-v1",
        "production": False,
        "proof_bytes": proof_bytes,
        "proofBytes": proof_bytes,
        "receiver_set": parts["receiver_set"],
        "public_inputs": parts["public_inputs"],
        "public_input_bytes": parts["public_input_bytes"],
        "publicInputBytes": parts["public_input_bytes"],
        "envelope": envelope,
    }


def _instruction_digest(payload: Mapping[str, Any], domain: bytes) -> str:
    digest = hashlib.sha256()
    digest.update(domain)
    digest.update(b"\x00")
    digest.update(_canonical_json_bytes(payload, "anonymousPgcInstruction"))
    return digest.hexdigest()


def build_anonymous_pgc_account_commitment_instruction(
    options: Mapping[str, Any],
) -> dict[str, Any]:
    """Build a typed Anonymous PGC account-commitment instruction model."""

    source = _require_mapping(options, "anonymousPgcAccountCommitmentInstruction")
    _reject_unknown_fields(
        source,
        {
            "accountCommitment",
            "account_commitment",
            "anonymitySetRoot",
            "anonymity_set_root",
            "chainId",
            "chain_id",
            "domainSeparator",
            "domain_separator",
        },
        "anonymousPgcAccountCommitmentInstruction",
    )
    _commitment_key, commitment_value = _read_single_alias(
        source,
        ("accountCommitment", "account_commitment"),
        "anonymousPgcAccountCommitmentInstruction.accountCommitment",
        "account commitment",
    )
    _root_key, root_value = _read_single_alias(
        source,
        ("anonymitySetRoot", "anonymity_set_root"),
        "anonymousPgcAccountCommitmentInstruction.anonymitySetRoot",
        "anonymity-set root",
    )
    _chain_key, chain_value = _read_single_alias(
        source,
        ("chainId", "chain_id"),
        "anonymousPgcAccountCommitmentInstruction.chainId",
        "chain id",
    )
    _domain_key, domain_value = _read_single_alias(
        source,
        ("domainSeparator", "domain_separator"),
        "anonymousPgcAccountCommitmentInstruction.domainSeparator",
        "domain separator",
    )
    payload = {
        "kind": "zk::RegisterAnonymousPgcAccountCommitment",
        "version": 1,
        "account_commitment": _fixed_bytes(
            commitment_value,
            "anonymousPgcAccountCommitmentInstruction.accountCommitment",
            32,
            nonzero=True,
        ).hex(),
        "anonymity_set_root": _fixed_bytes(
            root_value,
            "anonymousPgcAccountCommitmentInstruction.anonymitySetRoot",
            32,
            nonzero=True,
        ).hex(),
        "chain_id": _require_non_blank_string(
            chain_value,
            "anonymousPgcAccountCommitmentInstruction.chainId",
        ),
        "domain_separator": _require_non_blank_string(
            ANONYMOUS_PGC_DOMAIN_SEPARATOR
            if domain_value is _MISSING
            else domain_value,
            "anonymousPgcAccountCommitmentInstruction.domainSeparator",
        ),
    }
    payload["instruction_digest"] = _instruction_digest(
        payload,
        b"iroha:anonymous-pgc:account-commitment-instruction:v1",
    )
    return payload


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
            ("anonymitySetRoot", "anonymity_set_root"),
            "anonymitySetRoot",
            lambda value: _fixed_bytes(
                value,
                f"{context}.anonymitySetRoot",
                32,
                nonzero=True,
            ).hex(),
            public_inputs["anonymity_set_root"],
        ),
        (
            ("linkTag", "link_tag"),
            "linkTag",
            lambda value: _fixed_bytes(
                value,
                f"{context}.linkTag",
                32,
                nonzero=True,
            ).hex(),
            public_inputs["link_tag"],
        ),
    ):
        _key, value = _read_single_alias(source, fields, f"{context}.{path}", path)
        if value is not _MISSING and normalize(value) != actual:
            raise ValueError(f"{context}.{path} must match the envelope public inputs")
    if any(key in source for key in ("receiverSet", "receiver_set", "receivers")):
        receiver_set = _normalize_receiver_set_from_source(source, context)
        if (
            receiver_set["receiver_set_commitment"].hex()
            != public_inputs["receiver_set_commitment"]
            or receiver_set["threshold"] != public_inputs["receiver_threshold"]
            or receiver_set["receiver_count"] != public_inputs["receiver_count"]
        ):
            raise ValueError(f"{context}.receiverSet must match the envelope public inputs")
        ciphertext_commitments = [
            entry["ciphertext_commitment"].hex() for entry in receiver_set["receivers"]
        ]
        if ciphertext_commitments != list(public_inputs["receiver_ciphertext_commitments"]):
            raise ValueError(
                f"{context}.receiverSet ciphertext commitments must match the envelope public inputs"
            )
    for fields, path, max_items, actual in (
        (
            ("balanceCommitments", "balance_commitments"),
            "balanceCommitments",
            ANONYMOUS_PGC_MAX_BALANCE_COMMITMENTS,
            public_inputs["balance_commitments"],
        ),
        (
            ("rangeCommitments", "range_commitments"),
            "rangeCommitments",
            ANONYMOUS_PGC_MAX_RANGE_COMMITMENTS,
            public_inputs["range_commitments"],
        ),
    ):
        _key, value = _read_single_alias(source, fields, f"{context}.{path}", path)
        if value is not _MISSING:
            expected = [
                entry.hex()
                for entry in _normalize_commitment_list(
                    value,
                    f"{context}.{path}",
                    max_items,
                )
            ]
            if expected != list(actual):
                raise ValueError(f"{context}.{path} must match the envelope public inputs")
    _chain_key, chain_value = _read_single_alias(
        source,
        ("chainId", "chain_id"),
        f"{context}.chainId",
        "chain id",
    )
    if chain_value is not _MISSING:
        if (
            _require_non_blank_string(chain_value, f"{context}.chainId")
            != public_inputs["chain_id"]
        ):
            raise ValueError(f"{context}.chainId must match the envelope public inputs")
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


def verify_anonymous_pgc_k_out_of_n_proof_v1(options: Any) -> dict[str, Any]:
    """Validate a production Anonymous PGC proof envelope binding."""

    if isinstance(options, Mapping):
        source = options
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        {
            "envelope",
            "proofEnvelope",
            "proof_envelope",
            "bytes",
            "receiverSet",
            "receiver_set",
            "version",
            "threshold",
            "k",
            "receivers",
            "anonymitySetRoot",
            "anonymity_set_root",
            "txDigest",
            "tx_digest",
            "payloadDigest",
            "payload_digest",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "balanceCommitments",
            "balance_commitments",
            "linkTag",
            "link_tag",
            "rangeCommitments",
            "range_commitments",
            "chainId",
            "chain_id",
            "domainSeparator",
            "domain_separator",
            "maxPayloadBytes",
            "max_payload_bytes",
        },
        "anonymousPgcKOutOfNProofV1Verification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "anonymousPgcKOutOfNProofV1Verification.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError(
            "anonymousPgcKOutOfNProofV1Verification.envelope.backend must be Stark"
        )
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "anonymousPgcKOutOfNProofV1Verification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "anonymousPgcKOutOfNProofV1Verification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "anonymousPgcKOutOfNProofV1Verification.publicInputs",
    )
    _ensure_verification_expectations(
        source,
        public_inputs,
        "anonymousPgcKOutOfNProofV1Verification",
    )
    if decoded["proof_bytes"].startswith(ANONYMOUS_PGC_DEV_PROOF_PREFIX):
        raise ValueError(
            "anonymousPgcKOutOfNProofV1Verification proof bytes must not contain an Anonymous PGC dev fixture"
        )
    return {
        "ok": True,
        "production": True,
        "kind": "anonymous-pgc-k-out-of-n-v1",
        "backend": ANONYMOUS_PGC_BACKEND,
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "receiver_count": public_inputs["receiver_count"],
        "receiver_threshold": public_inputs["receiver_threshold"],
    }


def verify_anonymous_pgc_dev_proof_locally(options: Any) -> dict[str, Any]:
    """Verify a deterministic Anonymous PGC dev fixture envelope locally."""

    if isinstance(options, Mapping):
        source = options
    else:
        source = {"envelope": options}
    _reject_unknown_fields(
        source,
        {
            "envelope",
            "proofEnvelope",
            "proof_envelope",
            "bytes",
            "receiverSet",
            "receiver_set",
            "version",
            "threshold",
            "k",
            "receivers",
            "anonymitySetRoot",
            "anonymity_set_root",
            "txDigest",
            "tx_digest",
            "payloadDigest",
            "payload_digest",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "balanceCommitments",
            "balance_commitments",
            "linkTag",
            "link_tag",
            "rangeCommitments",
            "range_commitments",
            "chainId",
            "chain_id",
            "domainSeparator",
            "domain_separator",
            "maxPayloadBytes",
            "max_payload_bytes",
        },
        "anonymousPgcDevProofLocalVerification",
    )
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("envelope", "proofEnvelope", "proof_envelope", "bytes"),
        "anonymousPgcDevProofLocalVerification.envelope",
        "proof envelope",
    )
    decoded = decode_privacy_proof_envelope(envelope_value)
    if decoded["backend"] != "Stark":
        raise ValueError(
            "anonymousPgcDevProofLocalVerification.envelope.backend must be Stark"
        )
    circuit_id = _normalize_circuit_id(
        decoded["circuit_id"],
        "anonymousPgcDevProofLocalVerification.envelope.circuitId",
    )
    vk_hash = _fixed_bytes(
        decoded["vk_hash"],
        "anonymousPgcDevProofLocalVerification.envelope.vkHash",
        32,
        nonzero=True,
    )
    public_inputs = _parse_public_inputs(
        decoded["public_inputs"],
        "anonymousPgcDevProofLocalVerification.publicInputs",
    )
    _ensure_verification_expectations(
        source,
        public_inputs,
        "anonymousPgcDevProofLocalVerification",
    )
    expected_proof = _dev_proof_bytes(
        circuit_id=circuit_id,
        vk_hash=vk_hash,
        public_input_bytes=decoded["public_inputs"],
    )
    if decoded["proof_bytes"] != expected_proof:
        raise ValueError(
            "anonymousPgcDevProofLocalVerification proof bytes are not a valid Anonymous PGC dev fixture"
        )
    return {
        "ok": True,
        "production": False,
        "kind": "anonymous-pgc-dev-fixture-v1",
        "backend": ANONYMOUS_PGC_BACKEND,
        "circuit_id": circuit_id,
        "verifier_key_hash": vk_hash.hex(),
        "public_inputs": public_inputs,
        "public_input_bytes": len(decoded["public_inputs"]),
        "proof_bytes": len(decoded["proof_bytes"]),
        "aux_bytes": len(decoded["aux"]),
        "receiver_count": public_inputs["receiver_count"],
        "receiver_threshold": public_inputs["receiver_threshold"],
    }


def build_anonymous_pgc_transfer_instruction(options: Mapping[str, Any]) -> dict[str, Any]:
    """Build a typed Anonymous PGC transfer instruction model."""

    source = _require_mapping(options, "anonymousPgcTransferInstruction")
    _reject_unknown_fields(
        source,
        {
            "proofEnvelope",
            "proof_envelope",
            "envelope",
            "bytes",
            "receiverSet",
            "receiver_set",
            "payload",
            "payloadBytes",
            "payload_bytes",
            "payloadJson",
            "payload_json",
            "txDigest",
            "tx_digest",
            "payloadDigest",
            "payload_digest",
            "anonymitySetRoot",
            "anonymity_set_root",
            "balanceCommitments",
            "balance_commitments",
            "linkTag",
            "link_tag",
            "rangeCommitments",
            "range_commitments",
            "chainId",
            "chain_id",
            "domainSeparator",
            "domain_separator",
        },
        "anonymousPgcTransferInstruction",
    )
    verified = verify_anonymous_pgc_k_out_of_n_proof_v1(source)
    public_inputs = verified["public_inputs"]
    _envelope_key, envelope_value = _read_single_alias(
        source,
        ("proofEnvelope", "proof_envelope", "envelope", "bytes"),
        "anonymousPgcTransferInstruction.proofEnvelope",
        "proof envelope",
    )
    envelope = _bounded_bytes(
        envelope_value,
        "anonymousPgcTransferInstruction.proofEnvelope",
        max_bytes=DEFAULT_PRIVACY_MAX_PROOF_BYTES,
    )
    payload = {
        "kind": "zk::SubmitAnonymousPgcTransfer",
        "version": 1,
        "proof_envelope": envelope,
        "anonymity_set_root": public_inputs["anonymity_set_root"],
        "tx_digest": public_inputs["tx_digest"],
        "receiver_set_commitment": public_inputs["receiver_set_commitment"],
        "receiver_threshold": public_inputs["receiver_threshold"],
        "receiver_count": public_inputs["receiver_count"],
        "link_tag": public_inputs["link_tag"],
        "chain_id": public_inputs["chain_id"],
        "domain_separator": public_inputs["domain_separator"],
    }
    digest_payload = {key: value for key, value in payload.items() if key != "proof_envelope"}
    digest_payload["proof_envelope_sha256"] = hashlib.sha256(envelope).hexdigest()
    payload["instruction_digest"] = _instruction_digest(
        digest_payload,
        b"iroha:anonymous-pgc:transfer-instruction:v1",
    )
    return payload


buildAnonymousPgcReceiverSet = build_anonymous_pgc_receiver_set
buildAnonymousPgcAccountCommitmentInstruction = (
    build_anonymous_pgc_account_commitment_instruction
)
buildAnonymousPgcKOutOfNProofV1 = build_anonymous_pgc_k_out_of_n_proof_v1
verifyAnonymousPgcKOutOfNProofV1 = verify_anonymous_pgc_k_out_of_n_proof_v1
buildAnonymousPgcTransferInstruction = build_anonymous_pgc_transfer_instruction
buildAnonymousPgcDevProofFixture = build_anonymous_pgc_dev_proof_fixture
verifyAnonymousPgcDevProofLocally = verify_anonymous_pgc_dev_proof_locally
