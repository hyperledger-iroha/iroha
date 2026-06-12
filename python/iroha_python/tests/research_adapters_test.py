from __future__ import annotations

import pytest

from iroha_python import (
    buildAztecPrivateKernelProofV1,
    buildAztecPrivateRollupTransactionInstruction,
    buildFcmpPlusPlusMembershipProofV1,
    buildFcmpPlusPlusTransferInstruction,
    buildMidenNoteTransactionInstruction,
    buildMidenStarkTransactionProofV1,
    buildOrchardActionBundleInstruction,
    buildOrchardActionBundleProofV1,
    buildPenumbraOutputProofV1,
    buildPenumbraShieldedPoolTransaction,
    buildPenumbraSpendProofV1,
    buildPqMaspStarkRegisterPoolInstruction,
    buildPqMaspStarkTransferInstruction,
    buildPqMaspStarkTransferProofV0,
    build_aztec_private_kernel_proof_v1,
    build_aztec_private_rollup_transaction_instruction,
    build_fcmp_plus_plus_membership_proof_v1,
    build_fcmp_plus_plus_transfer_instruction,
    build_miden_note_transaction_instruction,
    build_miden_stark_transaction_proof_v1,
    build_orchard_action_bundle_instruction,
    build_orchard_action_bundle_proof_v1,
    build_penumbra_output_proof_v1,
    build_penumbra_shielded_pool_transaction,
    build_penumbra_spend_proof_v1,
    build_pq_masp_stark_register_pool_instruction,
    build_pq_masp_stark_transfer_instruction,
    build_pq_masp_stark_transfer_proof_v0,
    decode_privacy_proof_envelope,
)


def _proof_options() -> dict[str, object]:
    return {
        "vkHash": bytes([0x42]) * 32,
        "publicInputs": b"production-research-public-inputs",
        "proofBytes": b"production-research-proof",
    }


def test_research_adapter_proof_helpers_reject_non_plain_mapping_inputs() -> None:
    class ResearchDict(dict):
        pass

    options = _proof_options()
    helpers = (
        (
            build_orchard_action_bundle_proof_v1,
            buildOrchardActionBundleProofV1,
            "Halo2IpaOrchard",
        ),
        (
            build_penumbra_spend_proof_v1,
            buildPenumbraSpendProofV1,
            "Groth16Bls12377",
        ),
        (
            build_penumbra_output_proof_v1,
            buildPenumbraOutputProofV1,
            "Groth16Bls12377",
        ),
        (
            build_fcmp_plus_plus_membership_proof_v1,
            buildFcmpPlusPlusMembershipProofV1,
            "FcmpPlusPlusCurveTree",
        ),
        (
            build_miden_stark_transaction_proof_v1,
            buildMidenStarkTransactionProofV1,
            "MidenStark",
        ),
        (
            build_aztec_private_kernel_proof_v1,
            buildAztecPrivateKernelProofV1,
            "AztecPlonkishPrivateKernel",
        ),
        (
            build_pq_masp_stark_transfer_proof_v0,
            buildPqMaspStarkTransferProofV0,
            "PqMaspStarkFri",
        ),
    )

    for snake_case_helper, camel_case_helper, expected_backend in helpers:
        envelope = snake_case_helper(options)
        decoded = decode_privacy_proof_envelope(envelope)
        assert decoded["backend"] == expected_backend
        assert decoded["proof_bytes"] == b"production-research-proof"
        for helper in (snake_case_helper, camel_case_helper):
            with pytest.raises(TypeError, match="plain dict"):
                helper(ResearchDict(options))


def test_research_adapter_instruction_helpers_reject_non_plain_mapping_inputs() -> None:
    class ResearchDict(dict):
        pass

    options = _proof_options()
    helpers = (
        (
            build_orchard_action_bundle_instruction,
            buildOrchardActionBundleInstruction,
            "zk::SubmitOrchardActionBundle",
        ),
        (
            build_penumbra_shielded_pool_transaction,
            buildPenumbraShieldedPoolTransaction,
            "zk::SubmitPenumbraShieldedPoolTransaction",
        ),
        (
            build_fcmp_plus_plus_transfer_instruction,
            buildFcmpPlusPlusTransferInstruction,
            "zk::SubmitFcmpPlusPlusTransfer",
        ),
        (
            build_miden_note_transaction_instruction,
            buildMidenNoteTransactionInstruction,
            "zk::SubmitMidenNoteTransaction",
        ),
        (
            build_aztec_private_rollup_transaction_instruction,
            buildAztecPrivateRollupTransactionInstruction,
            "zk::SubmitAztecPrivateRollupTransaction",
        ),
        (
            build_pq_masp_stark_register_pool_instruction,
            buildPqMaspStarkRegisterPoolInstruction,
            "zk::SubmitPqMaspStarkTransfer",
        ),
        (
            build_pq_masp_stark_transfer_instruction,
            buildPqMaspStarkTransferInstruction,
            "zk::SubmitPqMaspStarkTransfer",
        ),
    )

    for snake_case_helper, camel_case_helper, instruction_kind in helpers:
        instruction = snake_case_helper(options)
        assert instruction["instruction"] == instruction_kind
        assert instruction["proof_envelope_sha256"]
        for helper in (snake_case_helper, camel_case_helper):
            with pytest.raises(TypeError, match="plain dict"):
                helper(ResearchDict(options))


def test_research_adapter_instruction_metadata_rejects_non_plain_mapping() -> None:
    class ResearchDict(dict):
        pass

    instruction = build_orchard_action_bundle_instruction(
        {**_proof_options(), "metadata": {"purpose": "boundary-test"}}
    )
    assert instruction["metadata"] == {"purpose": "boundary-test"}

    with pytest.raises(TypeError, match="orchard.metadata"):
        build_orchard_action_bundle_instruction(
            {**_proof_options(), "metadata": ResearchDict({"purpose": "test"})}
        )
