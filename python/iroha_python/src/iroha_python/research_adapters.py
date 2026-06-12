"""Production privacy adapters for externally-proved protocol rows."""

from __future__ import annotations

import hashlib
from collections.abc import Mapping
from typing import Any

from .verange import (
    DEFAULT_PRIVACY_MAX_PROOF_BYTES,
    DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES,
    _MISSING,
    _bounded_bytes,
    _read_single_alias,
    _reject_unknown_fields,
    _require_plain_mapping,
    build_privacy_proof_envelope,
)

_DEV_MARKERS = (b"dev-fixture", b"mock", b"fixture", b"local-only")

_SPECS: dict[str, dict[str, str]] = {
    "orchard": {
        "algorithm_id": "orchard-halo2-actions-v1",
        "backend": "halo2-pasta-action-bundle",
        "circuit_id": "orchard_halo2_action_bundle_v1",
        "instruction": "zk::SubmitOrchardActionBundle",
    },
    "penumbra_spend": {
        "algorithm_id": "penumbra-masp-v1",
        "backend": "groth16-bls12-377-decaf377",
        "circuit_id": "penumbra_masp_spend_v1",
        "instruction": "zk::SubmitPenumbraShieldedPoolTransaction",
    },
    "penumbra_output": {
        "algorithm_id": "penumbra-masp-v1",
        "backend": "groth16-bls12-377-decaf377",
        "circuit_id": "penumbra_masp_output_v1",
        "instruction": "zk::SubmitPenumbraShieldedPoolTransaction",
    },
    "fcmp": {
        "algorithm_id": "monero-fcmp-plus-plus-v1",
        "backend": "fcmp-plus-plus-curve-trees-bulletproofs",
        "circuit_id": "monero_fcmp_plus_plus_v1",
        "instruction": "zk::SubmitFcmpPlusPlusTransfer",
    },
    "miden": {
        "algorithm_id": "miden-stark-note-v1",
        "backend": "stark-vm-note-transaction",
        "circuit_id": "miden_stark_note_v1",
        "instruction": "zk::SubmitMidenNoteTransaction",
    },
    "aztec": {
        "algorithm_id": "aztec-private-rollup-v1",
        "backend": "plonkish-private-kernel-rollup",
        "circuit_id": "aztec_private_kernel_v1",
        "instruction": "zk::SubmitAztecPrivateRollupTransaction",
    },
    "pq_masp": {
        "algorithm_id": "pq-masp-stark-v0",
        "backend": "pq-masp-stark-fri",
        "circuit_id": "pq_masp_stark_v0",
        "instruction": "zk::SubmitPqMaspStarkTransfer",
    },
}


def _reject_dev_markers(value: bytes, context: str) -> None:
    lowered = value.lower()
    if any(marker in lowered for marker in _DEV_MARKERS):
        raise ValueError(f"{context} must not contain dev fixture or mock bytes")


def _proof_bytes(source: Mapping[str, Any], context: str) -> bytes:
    _key, value = _read_single_alias(
        source,
        ("proofBytes", "proof_bytes", "proof"),
        f"{context}.proofBytes",
        "proof bytes",
    )
    proof = _bounded_bytes(
        value,
        f"{context}.proofBytes",
        max_bytes=int(source.get("maxProofBytes", DEFAULT_PRIVACY_MAX_PROOF_BYTES)),
        allow_empty=False,
    )
    _reject_dev_markers(proof, f"{context}.proofBytes")
    return proof


def _public_inputs(source: Mapping[str, Any], context: str) -> bytes:
    _key, value = _read_single_alias(
        source,
        ("publicInputs", "public_inputs"),
        f"{context}.publicInputs",
        "public inputs",
    )
    return _bounded_bytes(
        value,
        f"{context}.publicInputs",
        max_bytes=int(
            source.get("maxPublicInputBytes", DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES)
        ),
        allow_empty=False,
    )


def _metadata(source: Mapping[str, Any], context: str) -> dict[str, Any]:
    value = source.get("metadata")
    if value is None:
        return {}
    return dict(_require_plain_mapping(value, f"{context}.metadata"))


def _build_research_proof(options: Mapping[str, Any], spec_name: str) -> bytes:
    spec = _SPECS[spec_name]
    source = _require_plain_mapping(options, spec_name)
    _reject_unknown_fields(
        source,
        {
            "vkHash",
            "vk_hash",
            "verifierKeyHash",
            "verifyingKeyHash",
            "publicInputs",
            "public_inputs",
            "proofBytes",
            "proof_bytes",
            "proof",
            "aux",
            "maxProofBytes",
            "max_proof_bytes",
            "maxPublicInputBytes",
            "max_public_input_bytes",
        },
        spec_name,
    )
    return build_privacy_proof_envelope(
        {
            "backend": spec["backend"],
            "circuitId": spec["circuit_id"],
            "vkHash": source.get(
                "vkHash",
                source.get("vk_hash", source.get("verifierKeyHash", source.get("verifyingKeyHash"))),
            ),
            "publicInputs": _public_inputs(source, spec_name),
            "proofBytes": _proof_bytes(source, spec_name),
            "aux": source.get("aux", b""),
            "maxProofBytes": source.get(
                "maxProofBytes",
                source.get("max_proof_bytes", DEFAULT_PRIVACY_MAX_PROOF_BYTES),
            ),
            "maxPublicInputBytes": source.get(
                "maxPublicInputBytes",
                source.get(
                    "max_public_input_bytes",
                    DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES,
                ),
            ),
        }
    )


def _build_research_instruction(options: Mapping[str, Any], spec_name: str) -> dict[str, Any]:
    spec = _SPECS[spec_name]
    source = _require_plain_mapping(options, spec_name)
    _reject_unknown_fields(
        source,
        {
            "proofEnvelope",
            "proof_envelope",
            "proofBytes",
            "proof_bytes",
            "proof",
            "vkHash",
            "vk_hash",
            "verifierKeyHash",
            "verifyingKeyHash",
            "publicInputs",
            "public_inputs",
            "aux",
            "metadata",
            "maxProofBytes",
            "max_proof_bytes",
            "maxPublicInputBytes",
            "max_public_input_bytes",
        },
        spec_name,
    )
    envelope_key, envelope_value = _read_single_alias(
        source,
        ("proofEnvelope", "proof_envelope"),
        f"{spec_name}.proofEnvelope",
        "proof envelope",
    )
    if envelope_key is None:
        proof_source = {key: value for key, value in source.items() if key != "metadata"}
        envelope = _build_research_proof(proof_source, spec_name)
    else:
        envelope = _bounded_bytes(
            envelope_value,
            f"{spec_name}.proofEnvelope",
            max_bytes=DEFAULT_PRIVACY_MAX_PROOF_BYTES + DEFAULT_PRIVACY_MAX_PUBLIC_INPUT_BYTES,
            allow_empty=False,
        )
        _reject_dev_markers(envelope, f"{spec_name}.proofEnvelope")
    return {
        "algorithm_id": spec["algorithm_id"],
        "instruction": spec["instruction"],
        "proof_envelope": envelope,
        "proof_envelope_sha256": hashlib.sha256(envelope).hexdigest(),
        "metadata": _metadata(source, spec_name),
    }


def build_orchard_action_bundle_proof_v1(options: Mapping[str, Any]) -> bytes:
    return _build_research_proof(options, "orchard")


def build_orchard_action_bundle_instruction(options: Mapping[str, Any]) -> dict[str, Any]:
    return _build_research_instruction(options, "orchard")


def build_penumbra_spend_proof_v1(options: Mapping[str, Any]) -> bytes:
    return _build_research_proof(options, "penumbra_spend")


def build_penumbra_output_proof_v1(options: Mapping[str, Any]) -> bytes:
    return _build_research_proof(options, "penumbra_output")


def build_penumbra_shielded_pool_transaction(options: Mapping[str, Any]) -> dict[str, Any]:
    return _build_research_instruction(options, "penumbra_spend")


def build_fcmp_plus_plus_membership_proof_v1(options: Mapping[str, Any]) -> bytes:
    return _build_research_proof(options, "fcmp")


def build_fcmp_plus_plus_transfer_instruction(options: Mapping[str, Any]) -> dict[str, Any]:
    return _build_research_instruction(options, "fcmp")


def build_miden_stark_transaction_proof_v1(options: Mapping[str, Any]) -> bytes:
    return _build_research_proof(options, "miden")


def build_miden_note_transaction_instruction(options: Mapping[str, Any]) -> dict[str, Any]:
    return _build_research_instruction(options, "miden")


def build_aztec_private_kernel_proof_v1(options: Mapping[str, Any]) -> bytes:
    return _build_research_proof(options, "aztec")


def build_aztec_private_rollup_transaction_instruction(options: Mapping[str, Any]) -> dict[str, Any]:
    return _build_research_instruction(options, "aztec")


def build_pq_masp_stark_transfer_proof_v0(options: Mapping[str, Any]) -> bytes:
    return _build_research_proof(options, "pq_masp")


def build_pq_masp_stark_register_pool_instruction(options: Mapping[str, Any]) -> dict[str, Any]:
    return _build_research_instruction(options, "pq_masp")


def build_pq_masp_stark_transfer_instruction(options: Mapping[str, Any]) -> dict[str, Any]:
    return _build_research_instruction(options, "pq_masp")


def generate_ml_dsa_key_pair(*_args: Any, **_kwargs: Any) -> None:
    raise RuntimeError("ML-DSA key generation requires a native PQ provider")


def encapsulate_ml_kem(*_args: Any, **_kwargs: Any) -> None:
    raise RuntimeError("ML-KEM encapsulation requires a native PQ provider")


buildOrchardActionBundleProofV1 = build_orchard_action_bundle_proof_v1
buildOrchardActionBundleInstruction = build_orchard_action_bundle_instruction
buildPenumbraSpendProofV1 = build_penumbra_spend_proof_v1
buildPenumbraOutputProofV1 = build_penumbra_output_proof_v1
buildPenumbraShieldedPoolTransaction = build_penumbra_shielded_pool_transaction
buildFcmpPlusPlusMembershipProofV1 = build_fcmp_plus_plus_membership_proof_v1
buildFcmpPlusPlusTransferInstruction = build_fcmp_plus_plus_transfer_instruction
buildMidenStarkTransactionProofV1 = build_miden_stark_transaction_proof_v1
buildMidenNoteTransactionInstruction = build_miden_note_transaction_instruction
buildAztecPrivateKernelProofV1 = build_aztec_private_kernel_proof_v1
buildAztecPrivateRollupTransactionInstruction = (
    build_aztec_private_rollup_transaction_instruction
)
buildPqMaspStarkTransferProofV0 = build_pq_masp_stark_transfer_proof_v0
buildPqMaspStarkRegisterPoolInstruction = (
    build_pq_masp_stark_register_pool_instruction
)
buildPqMaspStarkTransferInstruction = build_pq_masp_stark_transfer_instruction
generateMlDsaKeyPair = generate_ml_dsa_key_pair
encapsulateMlKem = encapsulate_ml_kem
