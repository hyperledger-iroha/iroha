//! Emits authoritative Rust Offline Cash V1 fixtures for Kotlin and Java facade tests.

use iroha_core::zk::kagemusha_finality::build_single_kagemusha_topup_execution_commitment_v2;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey};
use iroha_data_model::NetworkId;
use iroha_data_model::account::AccountId;
use iroha_data_model::asset::{AssetDefinitionId, AssetId};
use iroha_data_model::block::{
    BlockHeader,
    consensus_v2::{
        BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
        ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
        QuorumCertificate, ValidatorPower,
    },
};
use iroha_data_model::domain::DomainId;
use iroha_data_model::offline::{
    KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND, KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5,
    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
    KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4, KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
    KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2, KagemushaAndroidKeyMintHardwareAssertionV1,
    KagemushaDeviceSignatureV2, KagemushaOnlineHardwareAssertionV1, KagemushaPastaCycleParityV1,
    KagemushaPastaCycleProofEnvelopeV4, KagemushaRecursiveSpendArtifactBindingV4,
    KagemushaRecursiveSpendBranchClaimV2, KagemushaRecursiveSpendBundleV4,
    KagemushaRecursiveSpendOperationVectorV4, KagemushaRecursiveSpendProofV4,
    KagemushaRecursiveSpendPublicStatementV4, KagemushaRecursiveSpendRedeemRequestV4,
    KagemushaRecursiveSpendRedeemUnsignedV4, KagemushaRecursiveSpendRedemptionIntentV4,
    KagemushaRecursiveSpendStateBoundaryV5, KagemushaRecursiveSpendTopUpAnchorRefV2,
    KagemushaRecursiveSpendTopUpAnchorV4, KagemushaRecursiveSpendTopUpRequestV4,
    KagemushaRecursiveSpendTopUpUnsignedV4, KagemushaRequestAuthorizationV2,
    KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2, KagemushaTopUpFinalityCompactQcV2,
    KagemushaTopUpFinalityHeightContextV2, KagemushaTopUpFinalityProofV2,
    KagemushaTopUpShieldEvidenceV2, KagemushaUnshieldPublicInputsBindingV2,
    kagemusha_confidential_amount_encoding_v2, kagemusha_recursive_spend_verifier_key_id_v4,
};
use iroha_data_model::peer::PeerId;
use iroha_data_model::proof::{ProofAttachment, ProofBox, VerifyingKeyId};
use iroha_torii_shared::{
    ErrorDetails,
    offline_api::{
        OFFLINE_OPERATION_REJECTION_CODE, OfflineOperationKind, OfflineOperationReference,
        OfflineOperationRejectionError, OfflineOperationResult, OfflineOperationState,
        OfflineOperationStatus, OfflineRedeemResult, OfflineTopUpResult,
    },
};
use norito::derive::NoritoSerialize;

const PARITY_PUBLIC_KEY: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";

#[derive(NoritoSerialize)]
struct UncheckedOfflineOperationRejection {
    code: String,
    message: String,
}

#[derive(NoritoSerialize)]
struct UncheckedOfflineOperationRejectionWithDetails {
    code: String,
    message: String,
    details: Option<ErrorDetails>,
}

#[allow(dead_code)]
#[derive(NoritoSerialize)]
enum UncheckedOfflineOperationStatus<E> {
    Pending,
    Applied,
    Rejected {
        operation_id: String,
        kind: OfflineOperationKind,
        transaction_hash: String,
        error: E,
    },
}

struct ExactOfflineOperationStatusSchema<T>(T);

impl<T: norito::core::NoritoSerialize> norito::core::NoritoSerialize
    for ExactOfflineOperationStatusSchema<T>
{
    fn schema_hash() -> [u8; 16] {
        <OfflineOperationStatus as norito::core::NoritoSerialize>::schema_hash()
    }

    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

fn unchecked_rejection_status<E: norito::core::NoritoSerialize>(
    operation_id: String,
    transaction_hash: String,
    error: E,
) -> Vec<u8> {
    norito::to_bytes(&ExactOfflineOperationStatusSchema(
        UncheckedOfflineOperationStatus::Rejected {
            operation_id,
            kind: OfflineOperationKind::Redeem,
            transaction_hash,
            error,
        },
    ))
    .expect("encode intentionally invalid rejection fixture")
}

fn parity_account_id() -> AccountId {
    let public_key: PublicKey = PARITY_PUBLIC_KEY.parse().expect("parse public key");
    AccountId::new(public_key)
}

fn main() {
    let argument = std::env::args().nth(1);
    if argument.as_deref() != Some("offline-cash-v1") || std::env::args().nth(2).is_some() {
        eprintln!("Usage: kotlin_offline_cash_v1 offline-cash-v1");
        std::process::exit(1);
    }
    emit_offline_cash_v1();
}

fn offline_cash_network_id() -> NetworkId {
    let mut bytes = [0xA0; 32];
    bytes[31] = 0xA1;
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed(bytes),
    ))
}

fn offline_cash_foreign_network_id() -> NetworkId {
    let mut bytes = [0xD5; 32];
    bytes[31] = 0xD7;
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed(bytes),
    ))
}

fn offline_cash_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("cash", "universal").expect("offline cash domain"),
        "xor".parse().expect("offline cash asset name"),
    )
}

fn offline_cash_artifact_binding() -> KagemushaRecursiveSpendArtifactBindingV4 {
    KagemushaRecursiveSpendArtifactBindingV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "kotlin-offline-cash-v1".to_owned(),
        manifest_sha256: [0xA1; 32],
    }
}

fn offline_cash_authorization(
    authority: AccountId,
    asset_definition_id: AssetDefinitionId,
    operation_id: [u8; 32],
    payload_digest: [u8; 32],
    issued_at_ms: u64,
) -> KagemushaRequestAuthorizationV2 {
    KagemushaRequestAuthorizationV2 {
        authority,
        device_id: "kotlin-offline-cash-v1-device".to_owned(),
        asset_definition_id,
        operation_id,
        issued_at_ms,
        expires_at_ms: issued_at_ms + 30_000,
        nonce: [0xA2; 32],
        payload_digest,
        registration_hash: [0xA3; 32],
        hardware_assertion: KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(
            KagemushaAndroidKeyMintHardwareAssertionV1 {
                signature: KagemushaDeviceSignatureV2::from_raw_bytes(&[1; 64])
                    .expect("canonical low-S fixture signature"),
            },
        ),
    }
}

fn offline_cash_top_up_request() -> KagemushaRecursiveSpendTopUpRequestV4 {
    let payer = parity_account_id();
    let asset_definition_id = offline_cash_asset_definition_id();
    let amount = KagemushaScaledAmountV2::new(500, 2).expect("positive top-up amount");
    let operation_id = [0xA4; 32];
    let mut shield_proof = ProofAttachment::new_ref(
        KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.into(),
        ProofBox::new(KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.to_owned(), vec![0xA5]),
        VerifyingKeyId::new(
            KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND,
            "kotlin-offline-cash-topup-shield",
        ),
    );
    shield_proof.vk_commitment = Some([0xA6; 32]);
    let unsigned = KagemushaRecursiveSpendTopUpUnsignedV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        asset: AssetId::new(asset_definition_id.clone(), payer.clone()),
        amount,
        current_note: KagemushaSpendableNoteDescriptorV2 {
            network_id: offline_cash_network_id(),
            asset: asset_definition_id.clone(),
            note_commitment: [0xA7; 32],
            spend_nullifier: [0xA8; 32],
            amount,
        },
        shield_evidence: KagemushaTopUpShieldEvidenceV2 {
            initial_root: [0xA9; 32],
            finalized_root: [0xAA; 32],
            leaf_index: 0,
            proof: shield_proof,
        },
        artifact_binding: offline_cash_artifact_binding(),
        operation_id,
    };
    let payload_digest = unsigned.digest().expect("valid top-up payload");
    unsigned
        .into_request(offline_cash_authorization(
            payer,
            asset_definition_id,
            operation_id,
            payload_digest,
            1_725_000_000_001,
        ))
        .expect("valid top-up request")
}

fn offline_cash_redeem_request() -> KagemushaRecursiveSpendRedeemRequestV4 {
    let recipient = parity_account_id();
    let network_id = offline_cash_network_id();
    let asset_definition_id = offline_cash_asset_definition_id();
    let amount = KagemushaScaledAmountV2::new(500, 2).expect("positive redemption amount");
    let operation_id = [0xB1; 32];
    let topup_anchor_ref = KagemushaRecursiveSpendTopUpAnchorRefV2 {
        topup_operation_id: [0xB2; 32],
        anchor_digest: [0xB3; 32],
    };
    let branch_claim = KagemushaRecursiveSpendBranchClaimV2::root(topup_anchor_ref.anchor_digest)
        .expect("canonical root branch claim");
    let note = KagemushaSpendableNoteDescriptorV2 {
        network_id,
        asset: asset_definition_id.clone(),
        note_commitment: [0xB4; 32],
        spend_nullifier: [0xB5; 32],
        amount,
    };
    let binding = offline_cash_artifact_binding();
    let verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
        KagemushaPastaCycleParityV1::StepEq,
        binding.manifest_sha256,
    );
    let statement = KagemushaRecursiveSpendPublicStatementV4 {
        network_id,
        asset: asset_definition_id.clone(),
        asset_scale: 2,
        final_root: [0xB6; 32],
        next_zero_leaf_index: 1,
        topup_anchor_refs: vec![topup_anchor_ref.clone()],
        proof_step_count: 1,
        peer_hop_count: 0,
        current_note: note.clone(),
        branch_claims: vec![branch_claim.clone()],
        transition: None,
        artifact_binding: binding.clone(),
        verifier_key_id: verifier_key_id.clone(),
    };
    let public_statement_digest = statement.digest().expect("valid public statement");
    let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5];
    state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
    let mut operation_limbs = [0; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];
    operation_limbs[0] = 1;
    let bundle = KagemushaRecursiveSpendBundleV4 {
        statement,
        operation: KagemushaRecursiveSpendOperationVectorV4 {
            limbs: operation_limbs,
        },
        recursive_proof: KagemushaRecursiveSpendProofV4 {
            verifier_key_id,
            public_statement_digest,
            proof_envelope: KagemushaPastaCycleProofEnvelopeV4 {
                version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
                proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
                transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
                step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
                step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
                artifact_generation: binding.generation.clone(),
                manifest_sha256: binding.manifest_sha256,
                step_eq_parameter_generation: "kotlin-offline-cash-eq-params".to_owned(),
                step_ep_parameter_generation: "kotlin-offline-cash-ep-params".to_owned(),
                step_eq_circuit_params_sha256: [0xB7; 32],
                step_ep_circuit_params_sha256: [0xB8; 32],
                step_eq_verifier_key_sha256: [0xB9; 32],
                step_ep_verifier_key_sha256: [0xBA; 32],
                state_boundary: KagemushaRecursiveSpendStateBoundaryV5::new(state_limbs)
                    .expect("valid state boundary"),
                proof: ProofBox::new(
                    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
                    vec![0xBB],
                ),
            },
        },
    };
    let bundle_digest = bundle.digest().expect("valid recursive bundle");
    let unshield_public_inputs = KagemushaUnshieldPublicInputsBindingV2 {
        input_commitment_0: note.note_commitment,
        input_commitment_1: [0; 32],
        nullifier_0: note.spend_nullifier,
        nullifier_1: [0; 32],
        change_output_commitment: [0; 32],
        root: [0xB6; 32],
        public_amount: kagemusha_confidential_amount_encoding_v2(amount.atomic_units),
        asset_tag: [0xBC; 32],
        network_tag: [0xBD; 32],
    };
    let redemption = KagemushaRecursiveSpendRedemptionIntentV4 {
        network_id,
        asset: asset_definition_id.clone(),
        input_note: note,
        parent_branch_claims: vec![branch_claim],
        parent_topup_anchor_refs: vec![topup_anchor_ref],
        parent_proof_step_count: 1,
        parent_peer_hop_count: 0,
        parent_bundle_digest: bundle_digest,
        input_root: [0xB6; 32],
        recipient: recipient.clone(),
        public_amount: amount,
        change_output: None,
        change_artifact_binding: None,
        unshield_public_inputs_digest: unshield_public_inputs
            .digest()
            .expect("valid unshield public inputs"),
        unshield_public_inputs,
        operation_id,
    };
    let mut redeem_proof = ProofAttachment::new_ref(
        KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.into(),
        ProofBox::new(KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.to_owned(), vec![0xBE]),
        VerifyingKeyId::new(
            KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND,
            "kotlin-offline-cash-unshield",
        ),
    );
    redeem_proof.vk_commitment = Some([0xBF; 32]);
    let unsigned = KagemushaRecursiveSpendRedeemUnsignedV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        bundle,
        recipient: recipient.clone(),
        amount,
        redeem_proof,
        redemption,
        offline_change: None,
        block_height: 10,
        operation_id,
    };
    let payload_digest = unsigned.digest().expect("valid redemption payload");
    unsigned
        .into_request(offline_cash_authorization(
            recipient,
            asset_definition_id,
            operation_id,
            payload_digest,
            1_725_000_000_002,
        ))
        .expect("valid redemption request")
}

fn offline_cash_top_up_result(
    top_up: &KagemushaRecursiveSpendTopUpRequestV4,
) -> OfflineTopUpResult {
    let finalized_block_height = 1;
    let transaction_hash = [0xC1; 32];
    let anchor = KagemushaRecursiveSpendTopUpAnchorV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
        network_id: top_up.current_note.network_id,
        payer: top_up.authorization.authority.clone(),
        asset: top_up.asset.clone(),
        asset_scale: top_up.amount.scale,
        amount: top_up.amount,
        initial_root: top_up.shield_evidence.initial_root,
        finalized_root: top_up.shield_evidence.finalized_root,
        shield_leaf_index: top_up.shield_evidence.leaf_index,
        current_note: top_up.current_note.clone(),
        topup_operation_id: top_up.operation_id,
        shield_verifier_id: top_up.shield_evidence.proof.vk_ref.clone(),
        shield_verifier_commitment: top_up
            .shield_evidence
            .proof
            .vk_commitment
            .expect("top-up fixture verifier commitment"),
        artifact_binding: top_up.artifact_binding.clone(),
        finalized_height: finalized_block_height,
        finalized_tx_hash: transaction_hash,
        anchor_digest: [0; 32],
    }
    .finalize_digest()
    .expect("valid finalized top-up anchor");
    let commitment = build_single_kagemusha_topup_execution_commitment_v2(
        anchor.topup_operation_id,
        anchor.anchor_digest,
    )
    .expect("single top-up execution commitment");
    let mut roster = (0xC0_u8..=0xC3)
        .map(|seed| {
            let validator_key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture validator key");
            ValidatorPower {
                validator: PeerId::new(validator_key.public_key().clone()),
                power: 1,
            }
        })
        .collect::<Vec<_>>();
    roster.sort_by(|left, right| left.validator.cmp(&right.validator));
    let context = HeightContext {
        network_id: anchor.network_id,
        protocol_version: PROTOCOL_VERSION,
        height: finalized_block_height,
        epoch: 0,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"offline-cash-v1-nexus-context"),
        execution_policy_hash: Hash::new(b"offline-cash-v1-execution-policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0xC3; 32],
    };
    context.validate().expect("valid fixture height context");
    let round = ConsensusRound {
        context_id: context.id(),
        height: finalized_block_height,
        view: 0,
    };
    let certificate = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject: BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"offline-cash-v1-finalized-block",
            )),
            payload_hash: Hash::new(b"offline-cash-v1-finalized-payload"),
        },
        execution_commitment: ExecutionCommitment::new_without_merge_carrier(
            Hash::new(b"offline-cash-v1-parent-state"),
            commitment.post_state_root,
            commitment.ordinary_writes_root,
            Some(commitment.topup_anchor_root),
            1,
            1,
            Hash::new(b"offline-cash-v1-executed-block-wire"),
        )
        .expect("valid fixture execution commitment"),
        signers: vec![0],
        aggregate_signature: vec![0xC4; 96],
    };
    let finality_proof = KagemushaTopUpFinalityProofV2 {
        version: KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
        anchor: anchor
            .compact_ref()
            .expect("fixture top-up anchor reference"),
        commit_qc: KagemushaTopUpFinalityCompactQcV2 {
            height_context: KagemushaTopUpFinalityHeightContextV2 {
                context_id: context.id(),
                network_id: context.network_id,
                protocol_version: context.protocol_version,
                height: context.height,
                epoch: context.epoch,
                epoch_end_height: context.epoch_end_height,
                next_epoch_snapshot: context.next_epoch_snapshot,
                mode: context.mode,
                parent_commit_qc: context.parent_commit_qc,
                snapshot_bootstrap: context.snapshot_bootstrap,
                nexus_amx_context_hash: context.nexus_amx_context_hash,
                execution_policy_hash: context.execution_policy_hash,
                da_layout: context.da_layout,
                leader_seed: context.leader_seed,
            },
            certificate,
        },
        anchor_path: commitment.anchor_path,
    };
    finality_proof
        .validate_structure()
        .expect("structurally valid fixture finality proof");
    OfflineTopUpResult {
        transaction_hash: hex::encode(transaction_hash),
        finalized_block_height,
        server_time_ms: 1_725_000_000_101,
        anchor,
        finality_proof,
    }
}

fn emit_offline_cash_v1() {
    let top_up = offline_cash_top_up_request();
    let redeem = offline_cash_redeem_request();
    top_up
        .validate_public_binding()
        .expect("top-up fixture public binding");
    redeem
        .validate_public_binding()
        .expect("redeem fixture public binding");
    let top_up_id = hex::encode(top_up.operation_id);
    let redeem_id = hex::encode(redeem.operation_id);
    let top_up_reference = OfflineOperationReference {
        operation_id: top_up_id.clone(),
        kind: OfflineOperationKind::TopUp,
        state: OfflineOperationState::Pending,
        transaction_hash: "c1".repeat(32),
        status_uri: format!("/v1/offline/operations/{top_up_id}"),
        submitted_at_ms: top_up.authorization.issued_at_ms,
    };
    let redeem_reference = OfflineOperationReference {
        operation_id: redeem_id.clone(),
        kind: OfflineOperationKind::Redeem,
        state: OfflineOperationState::Pending,
        transaction_hash: "c3".repeat(32),
        status_uri: format!("/v1/offline/operations/{redeem_id}"),
        submitted_at_ms: redeem.authorization.issued_at_ms,
    };
    let mut invalid_transaction_hash_reference = top_up_reference.clone();
    invalid_transaction_hash_reference.transaction_hash = "c0".repeat(32);
    let top_up_pending = OfflineOperationStatus::Pending {
        operation_id: top_up_id.clone(),
        kind: OfflineOperationKind::TopUp,
        transaction_hash: "c1".repeat(32),
        submitted_at_ms: top_up.authorization.issued_at_ms,
    };
    let redeem_pending = OfflineOperationStatus::Pending {
        operation_id: redeem_id.clone(),
        kind: OfflineOperationKind::Redeem,
        transaction_hash: "c3".repeat(32),
        submitted_at_ms: redeem.authorization.issued_at_ms,
    };
    let redeem_applied = OfflineOperationStatus::Applied {
        operation_id: redeem_id.clone(),
        result: OfflineOperationResult::Redeem(OfflineRedeemResult {
            transaction_hash: "c3".repeat(32),
            finalized_block_height: 9,
            server_time_ms: 1_725_000_000_102,
        }),
    };
    let rejected = OfflineOperationStatus::Rejected {
        operation_id: redeem_id.clone(),
        kind: OfflineOperationKind::Redeem,
        transaction_hash: "c3".repeat(32),
        error: OfflineOperationRejectionError::try_new("rejected")
            .expect("canonical rejection fixture"),
    };
    let wrong_rejection_code_status = unchecked_rejection_status(
        redeem_id.clone(),
        "c3".repeat(32),
        UncheckedOfflineOperationRejection {
            code: "another_rejection".to_owned(),
            message: "rejected".to_owned(),
        },
    );
    let rejection_details_status = unchecked_rejection_status(
        redeem_id.clone(),
        "c3".repeat(32),
        UncheckedOfflineOperationRejectionWithDetails {
            code: OFFLINE_OPERATION_REJECTION_CODE.to_owned(),
            message: "rejected".to_owned(),
            details: Some(ErrorDetails {
                layer: Some("torii".to_owned()),
                ..ErrorDetails::default()
            }),
        },
    );
    let oversized_rejection_message_status = unchecked_rejection_status(
        redeem_id.clone(),
        "c3".repeat(32),
        UncheckedOfflineOperationRejection {
            code: OFFLINE_OPERATION_REJECTION_CODE.to_owned(),
            message: "界".repeat(1_025),
        },
    );
    let invalid_transaction_hash_status = OfflineOperationStatus::Pending {
        operation_id: top_up_id.clone(),
        kind: OfflineOperationKind::TopUp,
        transaction_hash: "c0".repeat(32),
        submitted_at_ms: top_up.authorization.issued_at_ms,
    };
    let top_up_applied_result = offline_cash_top_up_result(&top_up);
    let top_up_status = |result| OfflineOperationStatus::Applied {
        operation_id: top_up_id.clone(),
        result: OfflineOperationResult::TopUp(result),
    };
    let top_up_applied = top_up_status(top_up_applied_result.clone());
    let mut invalid_top_up_anchor_result = top_up_applied_result.clone();
    invalid_top_up_anchor_result.anchor.anchor_digest[0] ^= 1;
    let invalid_top_up_anchor_status = top_up_status(invalid_top_up_anchor_result);
    let mut invalid_top_up_proof_result = top_up_applied_result.clone();
    invalid_top_up_proof_result.finality_proof.version = 0;
    let invalid_top_up_proof_status = top_up_status(invalid_top_up_proof_result);
    let wrong_top_up_operation_status = OfflineOperationStatus::Applied {
        operation_id: "d3".repeat(32),
        result: OfflineOperationResult::TopUp(top_up_applied_result.clone()),
    };
    let mut wrong_top_up_transaction_result = top_up_applied_result.clone();
    wrong_top_up_transaction_result.transaction_hash = "d5".repeat(32);
    let wrong_top_up_transaction_status = top_up_status(wrong_top_up_transaction_result);
    let mut wrong_top_up_height_result = top_up_applied_result.clone();
    wrong_top_up_height_result.finalized_block_height += 1;
    let wrong_top_up_height_status = top_up_status(wrong_top_up_height_result);
    let mut wrong_top_up_proof_network_result = top_up_applied_result.clone();
    wrong_top_up_proof_network_result
        .finality_proof
        .commit_qc
        .height_context
        .network_id = offline_cash_foreign_network_id();
    let wrong_top_up_proof_network_status = top_up_status(wrong_top_up_proof_network_result);
    let mut foreign_network_top_up = top_up.clone();
    foreign_network_top_up.current_note.network_id = offline_cash_foreign_network_id();
    let foreign_network_top_up_status =
        top_up_status(offline_cash_top_up_result(&foreign_network_top_up));
    let mut wrong_top_up_proof_anchor_result = top_up_applied_result.clone();
    wrong_top_up_proof_anchor_result
        .finality_proof
        .anchor
        .anchor_digest[0] ^= 1;
    let wrong_top_up_proof_anchor_status = top_up_status(wrong_top_up_proof_anchor_result);
    let mut wrong_top_up_proof_height_result = top_up_applied_result.clone();
    wrong_top_up_proof_height_result
        .finality_proof
        .commit_qc
        .height_context
        .height += 1;
    wrong_top_up_proof_height_result
        .finality_proof
        .commit_qc
        .certificate
        .round
        .height += 1;
    wrong_top_up_proof_height_result
        .finality_proof
        .commit_qc
        .certificate
        .proposal_round
        .height += 1;
    let wrong_top_up_proof_height_status = top_up_status(wrong_top_up_proof_height_result);
    let mut invalid_binding_top_up = top_up.clone();
    invalid_binding_top_up.operation_id = [0xA5; 32];
    let mut wrong_id_reference = top_up_reference.clone();
    wrong_id_reference.operation_id = "d1".repeat(32);
    wrong_id_reference.status_uri =
        format!("/v1/offline/operations/{}", wrong_id_reference.operation_id);
    let mut wrong_kind_reference = top_up_reference.clone();
    wrong_kind_reference.kind = OfflineOperationKind::Redeem;
    let mut wrong_time_reference = top_up_reference.clone();
    wrong_time_reference.submitted_at_ms += 1;
    let mut zero_time_reference = top_up_reference.clone();
    zero_time_reference.submitted_at_ms = 0;
    let mut wrong_uri_reference = top_up_reference.clone();
    wrong_uri_reference.status_uri = format!("/v1/offline/operations/{redeem_id}");
    let wrong_id_status = OfflineOperationStatus::Pending {
        operation_id: redeem_id.clone(),
        kind: OfflineOperationKind::TopUp,
        transaction_hash: "c1".repeat(32),
        submitted_at_ms: top_up.authorization.issued_at_ms,
    };
    let zero_submitted_pending_status = OfflineOperationStatus::Pending {
        operation_id: top_up_id.clone(),
        kind: OfflineOperationKind::TopUp,
        transaction_hash: "c1".repeat(32),
        submitted_at_ms: 0,
    };
    let zero_height_status = OfflineOperationStatus::Applied {
        operation_id: redeem_id.clone(),
        result: OfflineOperationResult::Redeem(OfflineRedeemResult {
            transaction_hash: "c3".repeat(32),
            finalized_block_height: 0,
            server_time_ms: 9,
        }),
    };
    let zero_time_status = OfflineOperationStatus::Applied {
        operation_id: redeem_id,
        result: OfflineOperationResult::Redeem(OfflineRedeemResult {
            transaction_hash: "c3".repeat(32),
            finalized_block_height: 9,
            server_time_ms: 0,
        }),
    };
    let rows = [
        (
            "network_id",
            hex::encode(offline_cash_network_id().as_bytes()),
        ),
        ("top_up_operation_id", top_up_id),
        (
            "top_up_submitted_at_ms",
            top_up.authorization.issued_at_ms.to_string(),
        ),
        (
            "top_up_request",
            hex::encode(norito::to_bytes(&top_up).expect("encode top-up request")),
        ),
        (
            "top_up_reference",
            hex::encode(norito::to_bytes(&top_up_reference).expect("encode top-up reference")),
        ),
        (
            "top_up_pending_status",
            hex::encode(norito::to_bytes(&top_up_pending).expect("encode top-up pending status")),
        ),
        (
            "top_up_finalized_block_height",
            top_up_applied_result.finalized_block_height.to_string(),
        ),
        (
            "top_up_server_time_ms",
            top_up_applied_result.server_time_ms.to_string(),
        ),
        (
            "top_up_applied_status",
            hex::encode(norito::to_bytes(&top_up_applied).expect("encode applied top-up status")),
        ),
        (
            "invalid_top_up_anchor_status",
            hex::encode(
                norito::to_bytes(&invalid_top_up_anchor_status)
                    .expect("encode invalid-anchor top-up status"),
            ),
        ),
        (
            "invalid_top_up_proof_status",
            hex::encode(
                norito::to_bytes(&invalid_top_up_proof_status)
                    .expect("encode invalid-proof top-up status"),
            ),
        ),
        (
            "wrong_top_up_operation_status",
            hex::encode(
                norito::to_bytes(&wrong_top_up_operation_status)
                    .expect("encode wrong-operation top-up status"),
            ),
        ),
        (
            "wrong_top_up_transaction_status",
            hex::encode(
                norito::to_bytes(&wrong_top_up_transaction_status)
                    .expect("encode wrong-transaction top-up status"),
            ),
        ),
        (
            "wrong_top_up_height_status",
            hex::encode(
                norito::to_bytes(&wrong_top_up_height_status)
                    .expect("encode wrong-height top-up status"),
            ),
        ),
        (
            "wrong_top_up_proof_network_status",
            hex::encode(
                norito::to_bytes(&wrong_top_up_proof_network_status)
                    .expect("encode wrong-network top-up status"),
            ),
        ),
        (
            "foreign_network_top_up_status",
            hex::encode(
                norito::to_bytes(&foreign_network_top_up_status)
                    .expect("encode self-consistent foreign-network top-up status"),
            ),
        ),
        (
            "wrong_top_up_proof_anchor_status",
            hex::encode(
                norito::to_bytes(&wrong_top_up_proof_anchor_status)
                    .expect("encode wrong-proof-anchor top-up status"),
            ),
        ),
        (
            "wrong_top_up_proof_height_status",
            hex::encode(
                norito::to_bytes(&wrong_top_up_proof_height_status)
                    .expect("encode wrong-proof-height top-up status"),
            ),
        ),
        ("redeem_operation_id", hex::encode(redeem.operation_id)),
        (
            "redeem_submitted_at_ms",
            redeem.authorization.issued_at_ms.to_string(),
        ),
        (
            "redeem_request",
            hex::encode(norito::to_bytes(&redeem).expect("encode redeem request")),
        ),
        (
            "redeem_reference",
            hex::encode(norito::to_bytes(&redeem_reference).expect("encode redeem reference")),
        ),
        (
            "redeem_pending_status",
            hex::encode(norito::to_bytes(&redeem_pending).expect("encode redeem pending status")),
        ),
        (
            "redeem_applied_status",
            hex::encode(norito::to_bytes(&redeem_applied).expect("encode redeem applied status")),
        ),
        (
            "rejected_status",
            hex::encode(norito::to_bytes(&rejected).expect("encode rejected status")),
        ),
        (
            "invalid_binding_top_up_request",
            hex::encode(
                norito::to_bytes(&invalid_binding_top_up)
                    .expect("encode invalid-binding top-up request"),
            ),
        ),
        (
            "wrong_id_reference",
            hex::encode(norito::to_bytes(&wrong_id_reference).expect("encode wrong-id reference")),
        ),
        (
            "wrong_kind_reference",
            hex::encode(
                norito::to_bytes(&wrong_kind_reference).expect("encode wrong-kind reference"),
            ),
        ),
        (
            "wrong_time_reference",
            hex::encode(
                norito::to_bytes(&wrong_time_reference).expect("encode wrong-time reference"),
            ),
        ),
        (
            "zero_time_reference",
            hex::encode(
                norito::to_bytes(&zero_time_reference).expect("encode zero-time reference"),
            ),
        ),
        (
            "wrong_uri_reference",
            hex::encode(
                norito::to_bytes(&wrong_uri_reference).expect("encode wrong-uri reference"),
            ),
        ),
        (
            "invalid_transaction_hash_reference",
            hex::encode(
                norito::to_bytes(&invalid_transaction_hash_reference)
                    .expect("encode invalid-transaction-hash reference"),
            ),
        ),
        (
            "wrong_id_status",
            hex::encode(norito::to_bytes(&wrong_id_status).expect("encode wrong-id status")),
        ),
        (
            "zero_submitted_pending_status",
            hex::encode(
                norito::to_bytes(&zero_submitted_pending_status)
                    .expect("encode zero-submitted pending status"),
            ),
        ),
        (
            "zero_height_status",
            hex::encode(norito::to_bytes(&zero_height_status).expect("encode zero-height status")),
        ),
        (
            "zero_time_status",
            hex::encode(norito::to_bytes(&zero_time_status).expect("encode zero-time status")),
        ),
        (
            "invalid_transaction_hash_status",
            hex::encode(
                norito::to_bytes(&invalid_transaction_hash_status)
                    .expect("encode invalid-transaction-hash status"),
            ),
        ),
        (
            "wrong_rejection_code_status",
            hex::encode(wrong_rejection_code_status),
        ),
        (
            "rejection_details_status",
            hex::encode(rejection_details_status),
        ),
        (
            "oversized_rejection_message_status",
            hex::encode(oversized_rejection_message_status),
        ),
    ];
    for (name, value) in rows {
        println!("{name}={value}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate_status_through_c_abi(status: &OfflineOperationStatus) -> Result<Vec<u8>, i32> {
        let archive = norito::to_bytes(status).expect("encode operation status");
        let mut output = std::ptr::null_mut();
        let mut output_len = 0;
        let code = unsafe {
            connect_norito_bridge::connect_norito_kagemusha_validate_operation_status_v4(
                archive.as_ptr(),
                archive
                    .len()
                    .try_into()
                    .expect("status length fits c_ulong"),
                &mut output,
                &mut output_len,
            )
        };
        if code != 0 {
            assert!(output.is_null());
            assert_eq!(output_len, 0);
            return Err(code);
        }
        assert!(!output.is_null());
        let output_len: usize = output_len.try_into().expect("output length fits usize");
        let validated = unsafe { std::slice::from_raw_parts(output, output_len) }.to_vec();
        connect_norito_bridge::connect_norito_free(output);
        Ok(validated)
    }

    #[test]
    fn operation_status_c_abi_rejects_mismatched_top_up_finality_bindings() {
        let top_up = offline_cash_top_up_request();
        let operation_id = hex::encode(top_up.operation_id);
        let result = offline_cash_top_up_result(&top_up);
        let status = |result| OfflineOperationStatus::Applied {
            operation_id: operation_id.clone(),
            result: OfflineOperationResult::TopUp(result),
        };
        let valid = status(result.clone());
        assert_eq!(
            validate_status_through_c_abi(&valid).expect("validate canonical applied status"),
            norito::to_bytes(&valid).expect("encode canonical applied status")
        );

        let mut invalid_anchor = result.clone();
        invalid_anchor.anchor.anchor_digest[0] ^= 1;
        let mut invalid_proof = result.clone();
        invalid_proof.finality_proof.version = 0;
        let wrong_operation = OfflineOperationStatus::Applied {
            operation_id: "d3".repeat(32),
            result: OfflineOperationResult::TopUp(result.clone()),
        };
        let mut wrong_transaction = result.clone();
        wrong_transaction.transaction_hash = "d5".repeat(32);
        let mut wrong_height = result.clone();
        wrong_height.finalized_block_height += 1;
        let mut wrong_proof_network = result.clone();
        wrong_proof_network
            .finality_proof
            .commit_qc
            .height_context
            .network_id = offline_cash_foreign_network_id();
        let mut wrong_proof_anchor = result.clone();
        wrong_proof_anchor.finality_proof.anchor.anchor_digest[0] ^= 1;
        let mut wrong_proof_height = result;
        wrong_proof_height
            .finality_proof
            .commit_qc
            .height_context
            .height += 1;
        wrong_proof_height
            .finality_proof
            .commit_qc
            .certificate
            .round
            .height += 1;
        wrong_proof_height
            .finality_proof
            .commit_qc
            .certificate
            .proposal_round
            .height += 1;

        for invalid in [
            status(invalid_anchor),
            status(invalid_proof),
            wrong_operation,
            status(wrong_transaction),
            status(wrong_height),
            status(wrong_proof_network),
            status(wrong_proof_anchor),
            status(wrong_proof_height),
        ] {
            assert!(validate_status_through_c_abi(&invalid).is_err());
        }
    }
}
