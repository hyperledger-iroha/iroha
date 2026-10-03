//! Genuine funded ordinary Send State qualification, after the actual Mint transition.
//! Known-public fixture keys/clock/reservation data cannot create Native custody or DATA effects.
use super::super::{
    KagemushaPreparedIntentCommitmentsV1, composite::RecursiveStateConstructionV1,
    ordinary_guard_circuit::OrdinaryGuardWitnessV1,
};
use super::ordinary_mint_genuine_qualification_tests::{
    active_mint_state::{
        OrdinaryFundedStateForTestingV1, inactive_wrapper_column, require_full_relation_rejection,
        require_public_mutations_rejected,
    },
    sign_message_with_counter,
};
use super::*;
use crate::kagemusha_v1_recursion::real_handoff_qualification_tests::terminally_verify_state_proof;
use crate::kagemusha_v1_state::DigestV1;
use crate::kagemusha_v1_state::{
    OrdinarySendPreviewForQualificationV1, ordinary_send_preview_for_qualification_v1,
};
use iroha_data_model::kagemusha::*;

/// Actual funded→Send proof and full originals; no authenticated Native capability is constructed.
pub(super) struct OrdinarySendStateForTestingV1 {
    pub(super) funded: OrdinaryFundedStateForTestingV1,
    pub(super) state: KagemushaStateV1,
    pub(super) state_relation: KagemushaStateRelationWitnessV1,
    pub(super) generated: KagemushaGeneratedRecursiveStateProofV1,
    // Retain the complete original through the Send fixture lifetime.
    pub(super) _public_original: Vec<u8>,
    pub(super) preparation_relation: KagemushaGuardBundleRelationWitnessV1,
    pub(super) guard: super::ordinary_guard_generation::GeneratedOrdinaryGuardPairV1,
    pub(super) approval: KagemushaAppOperationApprovalV1,
    pub(super) prepared: KagemushaOrdinaryPreparedOutgoingV1,
    pub(super) transition_stream: Vec<u8>,
    pub(super) recovery_stream: Vec<u8>,
    pub(super) preview: OrdinarySendPreviewForQualificationV1,
    pub(super) request: KagemushaOrdinaryPaymentRequestV1,
    pub(super) receiver_credential: KagemushaOrdinaryAppCredentialV1,
    pub(super) previous_receiver_counter: Option<u32>,
    // Retain the complete original through the Send fixture lifetime.
    pub(super) _reservation: KagemushaOutboxReservationV1,
    pub(super) preparation_clock: KagemushaOrdinaryCashClockContextV1,
    pub(super) candidate_digest: DigestV1,
}
/// Prove full funded Send with actual subtraction, exact receiver signature, real maintained
/// AEAD, genuine W2 Guard, both State parities, complete current/history folds and whole SHA.
/// Final Wrapper descriptors must come from the held full-cycle topology planner.
pub(super) fn prove_ordinary_send_state_for_testing_v1(
    funded: OrdinaryFundedStateForTestingV1,
    receiver_credential: KagemushaOrdinaryAppCredentialV1,
    wrapper_eq: &PlonkProtocol<EqAffine>,
    wrapper_ep: &PlonkProtocol<EpAffine>,
    apple: bool,
) -> OrdinarySendStateForTestingV1 {
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let seed = KagemushaRecoverySeedV1::from_unsealed([44; 32]).unwrap();
    let c = &funded.bootstrap.credential;
    let source = &funded.mint_source;
    let mut artifacts = crate::kagemusha_v1_recursion::tests::artifacts();
    artifacts.release_id = funded.state.release_id;
    artifacts.artifact_manifest_digest = source
        .authorization
        .statement
        .context
        .artifact_manifest_digest;
    artifacts.eq_protocol_digest = native_parent_protocol_digest_v1(
        &funded.bootstrap.keys.eq_protocol,
        KagemushaPastaParityV1::Eq,
    )
    .unwrap();
    artifacts.ep_protocol_digest = native_parent_protocol_digest_v1(
        &funded.bootstrap.keys.ep_protocol,
        KagemushaPastaParityV1::Ep,
    )
    .unwrap();
    artifacts.guard_bundle_eq_protocol_digest = native_parent_protocol_digest_v1(
        &funded.bootstrap.guard_eq_protocol,
        KagemushaPastaParityV1::Eq,
    )
    .unwrap();
    artifacts.guard_bundle_ep_protocol_digest = native_parent_protocol_digest_v1(
        &funded.bootstrap.guard_ep_protocol,
        KagemushaPastaParityV1::Ep,
    )
    .unwrap();
    artifacts.mint_authorization_eq_protocol_digest = source.pair.eq.protocol_digest;
    artifacts.mint_authorization_ep_protocol_digest = source.pair.ep.protocol_digest;
    artifacts.mint_finality_eq_protocol_digest = source.source.credit.proof.eq_protocol_digest;
    artifacts.mint_finality_ep_protocol_digest = source.source.credit.proof.ep_protocol_digest;
    artifacts.commit_wrapper_eq_protocol_digest =
        native_parent_protocol_digest_v1(wrapper_eq, KagemushaPastaParityV1::Eq).unwrap();
    artifacts.commit_wrapper_ep_protocol_digest =
        native_parent_protocol_digest_v1(wrapper_ep, KagemushaPastaParityV1::Ep).unwrap();
    artifacts.canonical_empty_effect_digest = funded
        .bootstrap
        .original_relation
        .canonical_empty_effect_digest;
    let account = iroha_data_model::account::AccountId::new(
        iroha_crypto::KeyPair::from_seed(vec![62; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let request_clock = KagemushaOrdinaryCashClockContextV1 {
        version: 1,
        request_nonce: [80; 32],
        signed_observations_original_digest: [81; 32],
        lower_at_ms: 1800,
        upper_at_ms: 1801,
    };
    let secret = [11; 32];
    let request_body = KagemushaOrdinaryPaymentRequestBodyV1 {
        version: 1,
        release_id: funded.state.release_id,
        network_id: *funded.state.lane.network_id.as_bytes(),
        normalized_asset_id: kagemusha_asset_identity_digest_v1(&funded.state.lane.asset).unwrap(),
        asset_incarnation: *funded.state.asset_incarnation.as_bytes(),
        scale: funded.state.lane.scale,
        reserve_pool_id: funded.state.liability_pool_id,
        recipient_account_binding: receiver_credential.subject.account_binding,
        amount: 17,
        recipient_encryption_key: iroha_crypto::kagemusha::kagemusha_x25519_public_key_v1(&secret)
            .unwrap(),
        recipient_credential_digest: receiver_credential.canonical_digest().unwrap(),
        recipient_lane_id: receiver_credential.subject.lane_id,
        request_id: [82; 32],
        clock_context: request_clock,
        issued_at_ms: 1800,
        expires_at_ms: 30_000,
    };
    let request = KagemushaOrdinaryPaymentRequestV1 {
        evidence: sign_message_with_counter(
            &request_body.canonical_signing_bytes().unwrap(),
            apple,
            18,
        ),
        body: request_body,
    };
    let previous_receiver_counter = apple.then_some(17);
    let preparation_clock = KagemushaOrdinaryCashClockContextV1 {
        version: 1,
        request_nonce: [83; 32],
        signed_observations_original_digest: [84; 32],
        lower_at_ms: 1900,
        upper_at_ms: 1901,
    };
    let reservation = KagemushaOutboxReservationV1 {
        reservation_id: [85; 32],
        operation_kind: KagemushaOperationKindV1::SendSplit,
        reserved_outbox_bytes: 150_000_000,
        issued_at_ms: 1900,
        expires_at_ms: 30_000,
    };
    let preview = ordinary_send_preview_for_qualification_v1(
        &funded.state,
        &request,
        &preparation_clock,
        &reservation,
        1,
        [86; 32],
        [87; 32],
        &[88; 152],
        artifacts,
    )
    .unwrap();
    assert_eq!(preview.successor.balance, funded.state.balance - 17);
    assert_eq!(preview.successor.secure_index, 2);
    let mut subject = funded.bootstrap.original_subject.clone();
    subject.operation_kind = KagemushaOperationKindV1::SendSplit;
    subject.transition_statement_digest = preview.statement.digest().unwrap();
    subject.candidate_envelope_digest = [0; 32];
    subject.terminal_body_commitment = [0; 32];
    subject.secure_index_before = funded.state.secure_index;
    subject.secure_index_after = preview.successor.secure_index;
    let challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::PrepareTransition,
        operation_id: [87; 32],
        nonce: [86; 32],
        account_binding: c.subject.account_binding,
        authority_policy_digest: c.subject.app_authority_policy_digest,
        attested_key_id: c.subject.attested_key_id,
        enrollment_digest: c.canonical_digest().unwrap(),
        subject_signing_digest: Sha256::digest(subject.canonical_prepare_signing_bytes().unwrap())
            .into(),
        normalized_guard_digest: preview.normalized.canonical_digest().unwrap(),
        issued_at_ms: 1901,
        expires_at_ms: 1901 + crate::kagemusha_v1_state::ORDINARY_PREPARATION_LIFETIME_MS,
        subject,
    };
    let approval = KagemushaAppOperationApprovalV1 {
        evidence: sign_message_with_counter(
            &challenge.canonical_signing_bytes().unwrap(),
            apple,
            20,
        ),
        challenge,
    };
    let previous_counter = apple.then_some(19);
    let mut guard_relation = funded.bootstrap.original_relation.clone();
    guard_relation.statement = preview.normalized.clone();
    guard_relation.validate().unwrap();
    let guard = super::ordinary_guard_generation::generate_ordinary_guard_pair_v1(
        OrdinaryGuardWitnessV1 {
            relation: &guard_relation,
            credential: c,
            approval: &approval,
            previous_app_attest_counter: previous_counter,
            integrity_lease: None,
            incoming_terminal_body: None,
        },
        funded.state.device_policy_binding.hardware_policy_id,
        &funded.bootstrap.issuer_table,
        &seed,
    )
    .unwrap();
    assert_eq!(
        [guard.eq.protocol_digest, guard.ep.protocol_digest],
        [
            artifacts.guard_bundle_eq_protocol_digest,
            artifacts.guard_bundle_ep_protocol_digest
        ]
    );
    let transition_stream = vec![91; 32];
    let recovery_stream = vec![92; 32];
    let prepared = KagemushaOrdinaryPreparedOutgoingV1 {
        version: 1,
        operation: 2,
        predecessor_state: funded.state.state_commitment,
        successor_state: preview.successor.state_commitment,
        transition_digest: preview.statement.digest().unwrap(),
        prepared_transition_binding_digest: preview.statement.prepared_transition_binding_digest,
        projection_semantic_digest: preview.output.binding_digest().unwrap(),
        lifecycle_binding_digest: preview.statement.lifecycle_binding_digest,
        request_digest: request.canonical_original_digest().unwrap(),
        artifact_manifest_digest: [0; 32],
        preparation_guard_digest: preview.normalized.canonical_digest().unwrap(),
        reservation_digest: reservation.canonical_commitment().unwrap(),
        preparation_authorization_digest:
            kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
                kagemusha_ordinary_app_approval_proof_binding_digest_v1(&approval).unwrap(),
                None,
            )
            .unwrap(),
        stream_lengths: [transition_stream.len() as u64, recovery_stream.len() as u64],
        stream_digests: [
            kagemusha_ordinary_sealed_transition_inputs_digest_v1(&transition_stream).unwrap(),
            kagemusha_ordinary_sealed_recovery_seeds_digest_v1(&recovery_stream).unwrap(),
        ],
    };
    prepared.validate_shape().unwrap();
    let eq_zero = initial_kagemusha_eq_accumulator_v1(&eq).unwrap();
    let ep_zero = initial_kagemusha_ep_accumulator_v1(&ep).unwrap();
    let eq_fold_padding = KagemushaEqFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(
        EqAffine::generator().to_bytes().as_ref(),
    ))
    .unwrap();
    let ep_fold_padding = KagemushaEpFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(
        EpAffine::generator().to_bytes().as_ref(),
    ))
    .unwrap();
    let eq_incoming_padding = dummy_ordinary_proof_bytes(
        wrapper_eq,
        EqAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Eq,
    )
    .unwrap();
    let ep_incoming_padding = dummy_ordinary_proof_bytes(
        wrapper_ep,
        EpAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Ep,
    )
    .unwrap();
    let eq_wrapper_padding_column =
        inactive_wrapper_column::<Fp>(wrapper_eq, wrapper_ep, eq_zero.as_bytes());
    let ep_wrapper_padding_column =
        inactive_wrapper_column::<Fq>(wrapper_eq, wrapper_ep, ep_zero.as_bytes());
    let eq_inner_column = vec![funded.generated.eq_public_instances.clone()];
    let ep_inner_column = vec![funded.generated.ep_public_instances.clone()];
    let eq_outer_column = vec![funded.generated.eq_transport_public_instances.clone()];
    let ep_outer_column = vec![funded.generated.ep_transport_public_instances.clone()];
    let eq_inner_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq,
            &funded.bootstrap.keys.eq_protocol,
            &funded.generated.eq_inner_proof,
            &eq_inner_column[0],
        )
        .unwrap(),
    )
    .unwrap();
    let ep_inner_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep,
            &funded.bootstrap.keys.ep_protocol,
            &funded.generated.ep_inner_proof,
            &ep_inner_column[0],
        )
        .unwrap(),
    )
    .unwrap();
    let eq_parent_complete = fold_kagemusha_eq_accumulators_v1(
        &eq,
        &eq_inner_current,
        &funded.generated.eq_history,
        &seed,
    )
    .unwrap();
    let ep_parent_complete = fold_kagemusha_ep_accumulators_v1(
        &ep,
        &ep_inner_current,
        &funded.generated.ep_history,
        &seed,
    )
    .unwrap();
    let eq_outer_history =
        KagemushaEqAccumulatorV1::try_from_bytes(&funded.generated.proof.eq_history).unwrap();
    let ep_outer_history =
        KagemushaEpAccumulatorV1::try_from_bytes(&funded.generated.proof.ep_history).unwrap();
    let eq_outer_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq,
            &funded.bootstrap.keys.eq_transport_protocol,
            &funded.generated.proof.eq_proof,
            &eq_outer_column[0],
        )
        .unwrap(),
    )
    .unwrap();
    let ep_outer_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep,
            &funded.bootstrap.keys.ep_transport_protocol,
            &funded.generated.proof.ep_proof,
            &ep_outer_column[0],
        )
        .unwrap(),
    )
    .unwrap();
    let eq_outer_complete =
        fold_kagemusha_eq_accumulators_v1(&eq, &eq_outer_current, &eq_outer_history, &seed)
            .unwrap();
    let ep_outer_complete =
        fold_kagemusha_ep_accumulators_v1(&ep, &ep_outer_current, &ep_outer_history, &seed)
            .unwrap();
    let eq_outer_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_parent_complete.successor(),
        eq_outer_complete.successor(),
        &seed,
    )
    .unwrap();
    let ep_outer_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_parent_complete.successor(),
        ep_outer_complete.successor(),
        &seed,
    )
    .unwrap();
    let eq_guard_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq,
            &guard.eq.protocol,
            &guard.eq.proof,
            &guard.eq.instances,
        )
        .unwrap(),
    )
    .unwrap();
    let ep_guard_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep,
            &guard.ep.protocol,
            &guard.ep.proof,
            &guard.ep.instances,
        )
        .unwrap(),
    )
    .unwrap();
    let eq_guard_complete =
        fold_kagemusha_eq_accumulators_v1(&eq, &eq_guard_current, &guard.eq.history, &seed)
            .unwrap();
    let ep_guard_complete =
        fold_kagemusha_ep_accumulators_v1(&ep, &ep_guard_current, &guard.ep.history, &seed)
            .unwrap();
    let eq_guard_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_outer_merge.successor(),
        eq_guard_complete.successor(),
        &seed,
    )
    .unwrap();
    let ep_guard_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_outer_merge.successor(),
        ep_guard_complete.successor(),
        &seed,
    )
    .unwrap();
    let (inactive_authorization, inactive_credit) =
        crate::kagemusha_v1_recursion::generation::production_prover::ordinary_bootstrap_padding_for_testing(
            &funded.state,
            &account,
            artifacts.artifact_manifest_digest,
            [93; 32],
            &source.pair.eq.protocol,
            &source.pair.ep.protocol,
            &source.source.eq_protocol,
            &source.source.ep_protocol,
            &eq_zero,
            &ep_zero,
        )
        .unwrap();
    let eq_auth_columns = vec![
        vec![Fp::ZERO; super::super::ordinary_mint_circuit::ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1],
    ];
    let ep_auth_columns = vec![
        vec![Fq::ZERO; super::super::ordinary_mint_circuit::ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1],
    ];
    let mut eq_auth_columns = eq_auth_columns;
    let mut ep_auth_columns = ep_auth_columns;
    for (x, limb) in eq_auth_columns[0]
        [super::super::ordinary_mint_circuit::ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1 - 34..]
        .iter_mut()
        .zip(eq_zero.as_bytes().chunks_exact(16))
    {
        *x = crate::kagemusha_v1_poseidon::from_u128(u128::from_le_bytes(limb.try_into().unwrap()));
    }
    for (x, limb) in ep_auth_columns[0]
        [super::super::ordinary_mint_circuit::ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1 - 34..]
        .iter_mut()
        .zip(ep_zero.as_bytes().chunks_exact(16))
    {
        *x = crate::kagemusha_v1_poseidon::from_u128(u128::from_le_bytes(limb.try_into().unwrap()));
    }
    let req = super::super::KagemushaMintFinalityHelperVerificationRequestV1 {
        eq_protocol_digest: inactive_credit.proof.eq_protocol_digest,
        ep_protocol_digest: inactive_credit.proof.ep_protocol_digest,
        statement: &inactive_credit.statement,
        semantic_digest: inactive_credit.proof.semantic_digest,
        proof: &inactive_credit.proof,
        finality_certificate_binding: inactive_credit.finality_certificate_binding,
        finality_authority_head: inactive_credit.finality_authority_head,
        finality_genesis_authorization_id: inactive_credit.finality_genesis_authorization_id,
        finality_proof_binding_digest: inactive_credit.finality_proof_binding_digest,
        artifact_manifest_digest: artifacts.artifact_manifest_digest,
    };
    let eq_mint_columns = vec![
        super::super::native_backend::mint_public_instances::<Fp>(&req, eq_zero.as_bytes())
            .unwrap(),
    ];
    let ep_mint_columns = vec![
        super::super::native_backend::mint_public_instances::<Fq>(&req, ep_zero.as_bytes())
            .unwrap(),
    ];
    let eq_wrapper_columns = vec![eq_wrapper_padding_column];
    let ep_wrapper_columns = vec![ep_wrapper_padding_column];
    let t = &preview.statement;
    let mut witness = KagemushaRecursiveStateGenerationWitnessV1 {
        hash_claim: None,
        state: KagemushaStateRelationWitnessV1 {
            operation: KagemushaOperationV1::SendSplit,
            predecessor: Some(funded.state.clone()),
            successor: preview.successor.clone(),
            amount: t.amount,
            journal_revision_before: t.journal_revision_before,
            journal_revision_after: t.journal_revision_after,
            transition_effect_digest: t.effect_digest,
            mint_finality_semantic_digest: t.mint_finality_semantic_digest,
            mint_finality_proof_binding_digest: t.mint_finality_proof_binding_digest,
            peer_credit_id: t.peer_credit_id,
            recipient_encryption_key_binding: t.recipient_encryption_key_binding,
            receive_credit: None,
            receive_credit_binding_digest: t.receive_credit_binding_digest,
            lifecycle_binding_digest: t.lifecycle_binding_digest,
            prepared_transition_binding_digest: t.prepared_transition_binding_digest,
            prepared_intent: Some(KagemushaPreparedIntentCommitmentsV1 {
                preparation_id: prepared.binding_digest().unwrap(),
                sealed_transition_inputs_digest: prepared.stream_digests[0],
                sealed_recovery_seeds_digest: prepared.stream_digests[1],
            }),
            transport_semantic_digest: prepared.projection_semantic_digest,
            guard_statement_digest: preview.normalized.canonical_digest().unwrap(),
            eq_protocol_digest: native_parent_protocol_digest_v1(
                &funded.bootstrap.keys.eq_protocol,
                KagemushaPastaParityV1::Eq,
            )
            .unwrap(),
            ep_protocol_digest: native_parent_protocol_digest_v1(
                &funded.bootstrap.keys.ep_protocol,
                KagemushaPastaParityV1::Ep,
            )
            .unwrap(),
            guard_eq_protocol_digest: guard.eq.protocol_digest,
            guard_ep_protocol_digest: guard.ep.protocol_digest,
            mint_eq_protocol_digest: source.source.credit.proof.eq_protocol_digest,
            mint_ep_protocol_digest: source.source.credit.proof.ep_protocol_digest,
            mint_authorization_eq_protocol_digest: source.pair.eq.protocol_digest,
            mint_authorization_ep_protocol_digest: source.pair.ep.protocol_digest,
            commit_wrapper_eq_protocol_digest: native_parent_protocol_digest_v1(
                wrapper_eq,
                KagemushaPastaParityV1::Eq,
            )
            .unwrap(),
            commit_wrapper_ep_protocol_digest: native_parent_protocol_digest_v1(
                wrapper_ep,
                KagemushaPastaParityV1::Ep,
            )
            .unwrap(),
            guard_eq_credential_audit: funded.bootstrap.keys.eq_protocol_digest,
            guard_ep_credential_audit: funded.bootstrap.keys.ep_protocol_digest,
            eq_deferred_audit: [1; 32],
            ep_deferred_audit: [2; 32],
            replay_insert: None,
        },
        mint_fold_opening: None,
        mint_authorization: &inactive_authorization,
        mint_credit: &inactive_credit,
        guard_relation: guard_relation.clone(),
        hardware_selection: None,
        ordinary_selection: Some(KagemushaOrdinaryAppRecursiveSelectionWitnessV1 {
            credential: c,
            approval: &approval,
            integrity_lease: None,
            previous_app_attest_counter: previous_counter,
            prepared: Some(KagemushaOrdinaryRecursivePreparedOpeningV1 {
                record: &prepared,
                sealed_transition_inputs: &transition_stream,
                sealed_recovery_seeds: &recovery_stream,
            }),
            outer_parent: Some(KagemushaOrdinaryRecursiveOuterParentWitnessV1 {
                public_original: Some(&funded.public_original),
                eq_protocol: &funded.bootstrap.keys.eq_transport_protocol,
                ep_protocol: &funded.bootstrap.keys.ep_transport_protocol,
                eq_instances: &eq_outer_column,
                ep_instances: &ep_outer_column,
                eq_proof: &funded.generated.proof.eq_proof,
                ep_proof: &funded.generated.proof.ep_proof,
                eq_history: &eq_outer_history,
                ep_history: &ep_outer_history,
                eq_history_fold: eq_outer_complete.proof(),
                ep_history_fold: ep_outer_complete.proof(),
                eq_merge_fold: eq_outer_merge.proof(),
                ep_merge_fold: ep_outer_merge.proof(),
            }),
            incoming_mint: None,
            incoming_receive: None,
        }),
        eq_parent_protocol: &funded.bootstrap.keys.eq_protocol,
        ep_parent_protocol: &funded.bootstrap.keys.ep_protocol,
        eq_parent_instances: &eq_inner_column,
        ep_parent_instances: &ep_inner_column,
        eq_parent_proof: &funded.generated.eq_inner_proof,
        ep_parent_proof: &funded.generated.ep_inner_proof,
        eq_predecessor_history: &funded.generated.eq_history,
        ep_predecessor_history: &funded.generated.ep_history,
        eq_parent_fold_proof: eq_parent_complete.proof(),
        ep_parent_fold_proof: ep_parent_complete.proof(),
        eq_incoming_protocol: wrapper_eq,
        ep_incoming_protocol: wrapper_ep,
        eq_incoming_credits: [KagemushaRecursiveIncomingEqGenerationWitnessV1 {
            instances: &eq_wrapper_columns,
            proof: &eq_incoming_padding,
            history: &eq_zero,
            history_fold_proof: &eq_fold_padding,
            merge_fold_proof: &eq_fold_padding,
        }],
        ep_incoming_credits: [KagemushaRecursiveIncomingEpGenerationWitnessV1 {
            instances: &ep_wrapper_columns,
            proof: &ep_incoming_padding,
            history: &ep_zero,
            history_fold_proof: &ep_fold_padding,
            merge_fold_proof: &ep_fold_padding,
        }],
        eq_successor_history: eq_guard_merge.successor(),
        ep_successor_history: ep_guard_merge.successor(),
        eq_guard_protocol: &guard.eq.protocol,
        ep_guard_protocol: &guard.ep.protocol,
        eq_guard_proof: &guard.eq.proof,
        ep_guard_proof: &guard.ep.proof,
        eq_guard_history: &guard.eq.history,
        ep_guard_history: &guard.ep.history,
        eq_guard_history_fold_proof: eq_guard_complete.proof(),
        ep_guard_history_fold_proof: ep_guard_complete.proof(),
        eq_guard_merge_fold_proof: eq_guard_merge.proof(),
        ep_guard_merge_fold_proof: ep_guard_merge.proof(),
        eq_mint_authorization_protocol: &source.pair.eq.protocol,
        ep_mint_authorization_protocol: &source.pair.ep.protocol,
        eq_mint_authorization_instances: &eq_auth_columns,
        ep_mint_authorization_instances: &ep_auth_columns,
        eq_mint_authorization_proof: &inactive_authorization.proof.eq_proof,
        ep_mint_authorization_proof: &inactive_authorization.proof.ep_proof,
        eq_mint_authorization_history: &eq_zero,
        ep_mint_authorization_history: &ep_zero,
        eq_mint_authorization_history_fold_proof: &eq_fold_padding,
        ep_mint_authorization_history_fold_proof: &ep_fold_padding,
        eq_mint_authorization_merge_fold_proof: &eq_fold_padding,
        ep_mint_authorization_merge_fold_proof: &ep_fold_padding,
        eq_mint_protocol: &source.source.eq_protocol,
        ep_mint_protocol: &source.source.ep_protocol,
        eq_mint_instances: &eq_mint_columns,
        ep_mint_instances: &ep_mint_columns,
        eq_mint_proof: &inactive_credit.proof.eq_proof,
        ep_mint_proof: &inactive_credit.proof.ep_proof,
        eq_mint_history: &eq_zero,
        ep_mint_history: &ep_zero,
        eq_mint_history_fold_proof: &eq_fold_padding,
        ep_mint_history_fold_proof: &ep_fold_padding,
        eq_mint_merge_fold_proof: &eq_fold_padding,
        ep_mint_merge_fold_proof: &ep_fold_padding,
    };
    witness.state.validate().unwrap();
    let construction = RecursiveStateConstructionV1::OrdinaryOutgoingQualification;
    let claim = prove_kagemusha_recursive_state_hash_claim_v1_with_construction(
        &source.source.hash_eq,
        &source.source.hash_ep,
        witness.clone(),
        &seed,
        construction,
    )
    .expect("full actual funded Send original SHA and whole history queue");
    let eq_hash_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_guard_merge.successor(),
        &claim.eq_complete_history,
        &seed,
    )
    .unwrap();
    let ep_hash_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_guard_merge.successor(),
        &claim.ep_complete_history,
        &seed,
    )
    .unwrap();
    witness.hash_claim = Some(
        claim
            .consumer_witness(
                &source.source.hash_eq,
                &source.source.hash_ep,
                eq_hash_merge.proof(),
                ep_hash_merge.proof(),
            )
            .unwrap(),
    );
    witness.eq_successor_history = eq_hash_merge.successor();
    witness.ep_successor_history = ep_hash_merge.successor();
    assert!(
        prove_kagemusha_recursive_state_v1(
            &funded.bootstrap.keys.eq,
            &funded.bootstrap.keys.ep,
            witness.clone(),
            &seed
        )
        .is_err()
    );
    let (_, _, eq_audit, ep_audit) =
        build_recursive_generation_pair_v1(&eq, &ep, witness.clone(), construction).unwrap();
    witness.state.eq_deferred_audit = eq_audit;
    witness.state.ep_deferred_audit = ep_audit;
    let generated = prove_kagemusha_recursive_state_v1_with_construction(
        &funded.bootstrap.keys.eq,
        &funded.bootstrap.keys.ep,
        witness.clone(),
        &seed,
        construction,
    )
    .expect("actual funded Send bothparity inner+outer proof under the same retained family");
    terminally_verify_state_proof(&funded.bootstrap.keys, &generated);
    require_public_mutations_rejected(&funded.bootstrap.keys, &generated);
    let mut changed = prepared;
    changed.preparation_authorization_digest[0] ^= 1;
    let mut bad = witness.clone();
    bad.ordinary_selection.as_mut().unwrap().prepared =
        Some(KagemushaOrdinaryRecursivePreparedOpeningV1 {
            record: &changed,
            sealed_transition_inputs: &transition_stream,
            sealed_recovery_seeds: &recovery_stream,
        });
    require_full_relation_rejection(&eq, &ep, bad, construction);
    let width = super::super::state_relation::RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT;
    let public_original = super::super::KagemushaOrdinaryLineageStateOriginalV1 {
        version: 1,
        projection: super::super::KagemushaOrdinaryLineageStateProjectionV1::from_fields(
            generated.eq_transport_public_instances[..width].to_vec(),
            generated.ep_transport_public_instances[..width].to_vec(),
        )
        .unwrap(),
        proof: generated.proof.clone(),
    }
    .canonical_bytes()
    .unwrap();
    let mut state_relation = witness.state.clone();
    state_relation.eq_deferred_audit = generated.proof.eq_deferred_audit;
    state_relation.ep_deferred_audit = generated.proof.ep_deferred_audit;
    state_relation.eq_protocol_digest = generated.proof.eq_protocol_digest;
    state_relation.ep_protocol_digest = generated.proof.ep_protocol_digest;
    let candidate_digest = super::super::kagemusha_candidate_envelope_digest_v1(
        &state_relation.public_inputs_v1().unwrap(),
    )
    .unwrap();
    drop(witness);
    drop(claim);
    OrdinarySendStateForTestingV1 {
        state: preview.successor.clone(),
        state_relation,
        generated,
        _public_original: public_original,
        preparation_relation: guard_relation,
        guard,
        approval,
        prepared,
        transition_stream,
        recovery_stream,
        preview,
        request,
        receiver_credential,
        previous_receiver_counter,
        _reservation: reservation,
        preparation_clock,
        candidate_digest,
        funded,
    }
}
