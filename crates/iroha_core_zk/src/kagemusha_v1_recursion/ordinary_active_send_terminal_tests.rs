//! Genuine purpose1 Guard, full ordinary Terminal and exact inner/history Wrapper qualification.
//! Known-public mathematical originals grant no Native/FI/clock/DATA authority.
use super::super::{
    KagemushaNormalizedGuardStatementV1,
    composite::ordinary_cash_terminal_math::{
        OrdinaryCashCandidateCompleteProofV1, OrdinaryCashTerminalHalfWitnessV1,
        OrdinaryCashTerminalOutgoingWitnessV1, OrdinaryCashTerminalSemanticWitnessV1,
    },
    ordinary_cash_commit_wrapper::{
        OrdinaryCashCommitWrapperHalfWitnessV1, OrdinaryCashCommitWrapperWitnessV1,
    },
    ordinary_cash_terminal_circuit::OrdinaryCashTerminalCircuitWitnessV1,
    ordinary_cash_terminal_verifier::OrdinaryCashTerminalPublicV1,
    ordinary_guard_circuit::OrdinaryGuardWitnessV1,
    ordinary_guard_recursive_consumer::KagemushaOrdinaryGuardCompleteProofV1,
    ordinary_receiver_request_opening::OrdinaryReceiverRequestWitnessV1,
    typed_sha_consumer::KagemushaRecursiveHashClaimParityWitnessV1,
};
use super::ordinary_active_send_state::OrdinarySendStateForTestingV1;
use super::ordinary_cash_family_qualification::{
    OrdinaryCashProofForTestingV1, generate_ordinary_terminal_for_testing_v1,
    generate_ordinary_wrapper_for_testing_v1,
};
use super::ordinary_mint_genuine_qualification_tests::sign_message_with_counter;
use super::*;
use iroha_data_model::kagemusha::*;

/// Full actual outgoing proofs and same original data, without installing a financial owner.
pub(super) struct OrdinarySendTerminalForTestingV1 {
    pub(super) send: OrdinarySendStateForTestingV1,
    pub(super) intent: KagemushaOrdinaryCashTerminalIntentV1,
    pub(super) record: KagemushaOrdinaryCashTerminalRecordV1,
    pub(super) approval: KagemushaAppOperationApprovalV1,
    pub(super) admission_clock_original: Vec<u8>,
    // Retain the genuine purpose-1 Guard pair with the complete outgoing proof owner.
    pub(super) terminal_guard: super::ordinary_guard_generation::GeneratedOrdinaryGuardPairV1,
    pub(super) inner: OrdinaryCashProofForTestingV1,
    pub(super) wrapper: OrdinaryCashProofForTestingV1,
}
/// Execute real purpose1 Guard/full Terminal/Wrapper using exactly the funded Send originals.
/// The caller must compare final actual Wrapper descriptor/identity under its frozen family.
pub(super) fn prove_ordinary_send_terminal_for_testing_v1(
    send: OrdinarySendStateForTestingV1,
    apple: bool,
) -> OrdinarySendTerminalForTestingV1 {
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let seed = KagemushaRecoverySeedV1::from_unsealed([44; 32]).unwrap();
    let before = &send.funded.state;
    let after = &send.state;
    let prepared = &send.prepared;
    assert_eq!(
        send.reservation.canonical_commitment().unwrap(),
        prepared.reservation_digest,
        "the retained outgoing reservation must be the one authorized by preparation"
    );
    let original = super::super::KagemushaOrdinaryLineageStateOriginalV1::decode_original(
        &send.public_original,
    )
    .unwrap();
    assert_eq!(original.proof(), &send.generated.proof);
    let (eq_public, ep_public) = original.public_columns().unwrap();
    let width = super::super::state_relation::RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT;
    assert_eq!(
        eq_public,
        send.generated.eq_transport_public_instances[..width]
    );
    assert_eq!(
        ep_public,
        send.generated.ep_transport_public_instances[..width]
    );
    let c = &send.funded.bootstrap.credential;
    let intent = KagemushaOrdinaryCashTerminalIntentV1 {
        version: 1,
        operation: 2,
        native_operation_id: [101; 32],
        native_nonce: [102; 32],
        preparation_id: prepared.binding_digest().unwrap(),
        candidate_digest: send.candidate_digest,
        state_statement_digest: send.preview.statement.digest().unwrap(),
        predecessor_descriptor_prefix_digest: Sha256::digest(&send.funded.public_original).into(),
        sender_credential_digest: c.canonical_digest().unwrap(),
        reservation_digest: prepared.reservation_digest,
        secure_index_before: before.secure_index,
        secure_index_after: after.secure_index,
        logical_journal_sequence_before: 1,
        logical_journal_sequence_after: 2,
        issued_at_ms: 2101,
        expires_at_ms: 121_101.min(c.subject.expires_at_ms),
    };
    let (_, clock) =
        crate::kagemusha_v1_state::ordinary_signed_clock_data_for_qualification_v1([103; 32], 2100);
    let body = KagemushaOrdinaryCashTerminalBodyV1 {
        version: 1,
        operation: 2,
        amount: 17,
        state_statement_digest: intent.state_statement_digest,
        candidate_digest: intent.candidate_digest,
        preparation_id: intent.preparation_id,
        prepared_projection_semantic_digest: prepared.projection_semantic_digest,
        lifecycle_digest: prepared.lifecycle_binding_digest,
        request_digest: prepared.request_digest,
        recipient_credential_digest: send.receiver_credential.canonical_digest().unwrap(),
        send_output_digest: send.preview.output.binding_digest().unwrap(),
        encrypted_credit_digest: send.preview.output.encrypted_credit_digest,
        artifact_manifest_digest: [0; 32],
        reservation_digest: prepared.reservation_digest,
        native_operation_id: intent.native_operation_id,
        terminal_intent_digest: intent.binding_digest().unwrap(),
        predecessor_descriptor_prefix_digest: intent.predecessor_descriptor_prefix_digest,
        stream_lengths: prepared.stream_lengths,
        stream_digests: prepared.stream_digests,
        clock_context: clock,
        secure_index_before: intent.secure_index_before,
        secure_index_after: intent.secure_index_after,
        logical_journal_sequence_before: 1,
        logical_journal_sequence_after: 2,
    };
    body.validate_against_intent(&intent).unwrap();
    body.validate_against_prepared(prepared).unwrap();
    let mut context = send.preview.guard_context;
    context.terminal_commit_binding_digest =
        kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
            body.binding_digest().unwrap(),
            send.candidate_digest,
            intent.state_statement_digest,
            prepared.reservation_digest,
        )
        .unwrap();
    context.sender_one_time_authorization_digest = prepared.preparation_authorization_digest;
    context.transition_intent_digest = body.binding_digest().unwrap();
    context.recovery_record_digest = intent.binding_digest().unwrap();
    let normalized = KagemushaNormalizedGuardStatementV1::derive_from_transition(
        &send.preview.statement,
        context,
    )
    .unwrap();
    let mut subject = send.approval.challenge.subject.clone();
    subject.candidate_envelope_digest = send.candidate_digest;
    subject.terminal_body_commitment = body.binding_digest().unwrap();
    let challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
        operation_id: intent.native_operation_id,
        nonce: intent.native_nonce,
        account_binding: c.subject.account_binding,
        authority_policy_digest: c.subject.app_authority_policy_digest,
        attested_key_id: c.subject.attested_key_id,
        enrollment_digest: c.canonical_digest().unwrap(),
        subject_signing_digest: Sha256::digest(subject.canonical_signing_bytes().unwrap()).into(),
        normalized_guard_digest: normalized.canonical_digest().unwrap(),
        issued_at_ms: intent.issued_at_ms,
        expires_at_ms: intent.expires_at_ms,
        subject,
    };
    let approval = KagemushaAppOperationApprovalV1 {
        evidence: sign_message_with_counter(
            &challenge.canonical_signing_bytes().unwrap(),
            apple,
            21,
        ),
        challenge,
    };
    let terminal_authorization =
        kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
            kagemusha_ordinary_app_approval_proof_binding_digest_v1(&approval).unwrap(),
            None,
        )
        .unwrap();
    let (admission_clock_original, capture_clock) =
        crate::kagemusha_v1_state::ordinary_signed_clock_data_for_qualification_v1([105; 32], 2102);
    let record = KagemushaOrdinaryCashTerminalRecordV1 {
        version: 1,
        body,
        sender_credential_digest: c.canonical_digest().unwrap(),
        preparation_authorization_digest: prepared.preparation_authorization_digest,
        terminal_authorization_digest: terminal_authorization,
        terminal_subject_digest: approval.challenge.subject_signing_digest,
        admission_clock_context: capture_clock,
        approval_issued_at_ms: intent.issued_at_ms,
        approval_expires_at_ms: intent.expires_at_ms,
    };
    record
        .validate_against_originals(&intent, prepared)
        .unwrap();
    let mut terminal_relation = send.preparation_relation.clone();
    terminal_relation.statement = normalized;
    let terminal_guard = super::ordinary_guard_generation::generate_ordinary_guard_pair_v1(
        OrdinaryGuardWitnessV1 {
            relation: &terminal_relation,
            credential: c,
            approval: &approval,
            integrity_lease: None,
            previous_app_attest_counter: apple.then_some(20),
            incoming_terminal_body: None,
        },
        before.device_policy_binding.hardware_policy_id,
        &send.funded.bootstrap.issuer_table,
        &seed,
    )
    .unwrap();
    assert_eq!(
        [
            terminal_guard.eq.protocol_digest,
            terminal_guard.ep.protocol_digest
        ],
        [send.guard.eq.protocol_digest, send.guard.ep.protocol_digest]
    );
    let semantic = OrdinaryCashTerminalSemanticWitnessV1 {
        state: &send.state_relation,
        preparation_relation: &send.preparation_relation,
        terminal_relation: &terminal_relation,
        sender_credential: c,
        preparation_approval: &send.approval,
        preparation_integrity_lease: None,
        terminal_approval: &approval,
        terminal_integrity_lease: None,
        prepared: KagemushaOrdinaryRecursivePreparedOpeningV1 {
            record: prepared,
            sealed_transition_inputs: &send.transition_stream,
            sealed_recovery_seeds: &send.recovery_stream,
        },
        intent: &intent,
        record: &record,
        preparation_clock: &send.preparation_clock,
        issuer_table: &send.funded.bootstrap.issuer_table,
        outgoing: OrdinaryCashTerminalOutgoingWitnessV1::Send {
            receiver: OrdinaryReceiverRequestWitnessV1 {
                request: &send.request,
                credential: &send.receiver_credential,
                integrity_lease: None,
                previous_app_attest_counter: send.previous_receiver_counter,
                enabled: true,
            },
            output: &send.preview.output,
            encrypted_credit: &send.preview.encrypted_credit,
            credit_opening: &send.preview.credit_opening,
        },
    };
    let public = OrdinaryCashTerminalPublicV1 {
        operation: 2,
        suite_id: before.suite_id,
        vk_set_digest: before.vk_digest,
        release_id: before.release_id,
        network_id: *before.lane.network_id.as_bytes(),
        asset_id: kagemusha_asset_identity_digest_v1(&before.lane.asset).unwrap(),
        asset_incarnation: *before.asset_incarnation.as_bytes(),
        asset_scale: before.lane.scale,
        liability_pool_id: before.liability_pool_id,
        app_credential_profile_id: before.hardware_profile_id,
        policy_epoch: before.policy_epoch,
        lifecycle_digest: body.lifecycle_digest,
        body_digest: body.binding_digest().unwrap(),
        candidate_digest: send.candidate_digest,
        terminal_record_digest: record.binding_digest().unwrap(),
        transition_nullifier: send.preview.output.transition_nullifier,
        request_digest: body.request_digest,
        receiver_credential_digest: body.recipient_credential_digest,
        ciphertext_commitment: send.preview.output.ciphertext_commitment,
        amount: 17,
        output_binding_digest: kagemusha_ordinary_output_binding_digest_v1(
            prepared.projection_semantic_digest,
            body.candidate_digest,
            record.binding_digest().unwrap(),
        )
        .unwrap(),
        redemption_manifest_digest: [0; 32],
        eq_deferred_audit: send.generated.proof.eq_deferred_audit,
        ep_deferred_audit: send.generated.proof.ep_deferred_audit,
        eq_protocol_digest: send.generated.proof.eq_protocol_digest,
        ep_protocol_digest: send.generated.proof.ep_protocol_digest,
    };
    let eq_instances = vec![send.generated.eq_transport_public_instances.clone()];
    let ep_instances = vec![send.generated.ep_transport_public_instances.clone()];
    let eq_history =
        KagemushaEqAccumulatorV1::try_from_bytes(&send.generated.proof.eq_history).unwrap();
    let ep_history =
        KagemushaEpAccumulatorV1::try_from_bytes(&send.generated.proof.ep_history).unwrap();
    let g2eq_history = &send.guard.eq.history;
    let g2ep_history = &send.guard.ep.history;
    let g1eq_history = &terminal_guard.eq.history;
    let g1ep_history = &terminal_guard.ep.history;
    let g2eq = &send.guard.eq.instances;
    let g2ep = &send.guard.ep.instances;
    let g1eq = &terminal_guard.eq.instances;
    let g1ep = &terminal_guard.ep.instances;
    macro_rules! complete {
        ($ty:ty, $verify:ident, $fold:ident, $params:expr, $protocol:expr, $proof:expr, $instances:expr, $history:expr) => {{
            let current =
                <$ty>::from_native(&$verify($params, $protocol, $proof, $instances).unwrap())
                    .unwrap();
            $fold($params, &current, $history, &seed).unwrap()
        }};
    }
    let eq_candidate = complete!(
        KagemushaEqAccumulatorV1,
        verify_eq_succinct_protocol,
        fold_kagemusha_eq_accumulators_v1,
        &eq,
        &send.funded.bootstrap.keys.eq_transport_protocol,
        &send.generated.proof.eq_proof,
        &eq_instances[0],
        &eq_history
    );
    let ep_candidate = complete!(
        KagemushaEpAccumulatorV1,
        verify_ep_succinct_protocol,
        fold_kagemusha_ep_accumulators_v1,
        &ep,
        &send.funded.bootstrap.keys.ep_transport_protocol,
        &send.generated.proof.ep_proof,
        &ep_instances[0],
        &ep_history
    );
    let eq_preparation = complete!(
        KagemushaEqAccumulatorV1,
        verify_eq_succinct_protocol,
        fold_kagemusha_eq_accumulators_v1,
        &eq,
        &send.guard.eq.protocol,
        &send.guard.eq.proof,
        &g2eq,
        &g2eq_history
    );
    let ep_preparation = complete!(
        KagemushaEpAccumulatorV1,
        verify_ep_succinct_protocol,
        fold_kagemusha_ep_accumulators_v1,
        &ep,
        &send.guard.ep.protocol,
        &send.guard.ep.proof,
        &g2ep,
        &g2ep_history
    );
    let eq_terminal = complete!(
        KagemushaEqAccumulatorV1,
        verify_eq_succinct_protocol,
        fold_kagemusha_eq_accumulators_v1,
        &eq,
        &send.guard.eq.protocol,
        &terminal_guard.eq.proof,
        &g1eq,
        &g1eq_history
    );
    let ep_terminal = complete!(
        KagemushaEpAccumulatorV1,
        verify_ep_succinct_protocol,
        fold_kagemusha_ep_accumulators_v1,
        &ep,
        &send.guard.ep.protocol,
        &terminal_guard.ep.proof,
        &g1ep,
        &g1ep_history
    );
    let eq_prepare_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_candidate.successor(),
        eq_preparation.successor(),
        &seed,
    )
    .unwrap();
    let ep_prepare_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_candidate.successor(),
        ep_preparation.successor(),
        &seed,
    )
    .unwrap();
    let eq_terminal_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_prepare_merge.successor(),
        eq_terminal.successor(),
        &seed,
    )
    .unwrap();
    let ep_terminal_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_prepare_merge.successor(),
        ep_terminal.successor(),
        &seed,
    )
    .unwrap();

    let eq_hash = &send.funded.mint_source.source.hash_eq;
    let ep_hash = &send.funded.mint_source.source.hash_ep;
    let hash = super::ordinary_cash_terminal_generation::prove_ordinary_cash_terminal_sha_claim_v1(
        eq_hash,
        ep_hash,
        &public,
        &semantic,
        &eq_instances,
        &ep_instances,
        &seed,
    )
    .unwrap();
    let eq_hash_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_terminal_merge.successor(),
        &hash.eq_complete_history,
        &seed,
    )
    .unwrap();
    let ep_hash_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_terminal_merge.successor(),
        &hash.ep_complete_history,
        &seed,
    )
    .unwrap();
    let protocols = [
        hash.eq_claim_protocol_digest,
        hash.ep_claim_protocol_digest,
        hash.eq_shard_protocol_digest,
        hash.ep_shard_protocol_digest,
    ];
    let eq_history_native = eq_history.to_native().unwrap();
    let ep_history_native = ep_history.to_native().unwrap();
    let g2eq_native = g2eq_history.to_native().unwrap();
    let g2ep_native = g2ep_history.to_native().unwrap();
    let g1eq_native = g1eq_history.to_native().unwrap();
    let g1ep_native = g1ep_history.to_native().unwrap();
    let eq_hash_native = hash.eq_history.to_native().unwrap();
    let ep_hash_native = hash.ep_history.to_native().unwrap();
    let witness = OrdinaryCashTerminalCircuitWitnessV1 {
        public: public.clone(),
        semantic: &semantic,
        eq: OrdinaryCashTerminalHalfWitnessV1 {
            candidate: OrdinaryCashCandidateCompleteProofV1 {
                protocol: &send.funded.bootstrap.keys.eq_transport_protocol,
                instances: &eq_instances,
                proof: &send.generated.proof.eq_proof,
                history: &eq_history_native,
                history_fold_proof: eq_candidate.proof().as_bytes(),
            },
            preparation_guard: KagemushaOrdinaryGuardCompleteProofV1 {
                protocol: &send.guard.eq.protocol,
                proof: &send.guard.eq.proof,
                history: &g2eq_native,
                history_bytes: send.guard.eq.history.as_bytes(),
                history_fold_proof: eq_preparation.proof().as_bytes(),
            },
            terminal_guard: KagemushaOrdinaryGuardCompleteProofV1 {
                protocol: &send.guard.eq.protocol,
                proof: &terminal_guard.eq.proof,
                history: &g1eq_native,
                history_bytes: terminal_guard.eq.history.as_bytes(),
                history_fold_proof: eq_terminal.proof().as_bytes(),
            },
            preparation_merge_fold_proof: eq_prepare_merge.proof().as_bytes(),
            terminal_merge_fold_proof: eq_terminal_merge.proof().as_bytes(),
            hash_claim: KagemushaRecursiveHashClaimParityWitnessV1 {
                protocol_digests: protocols,
                protocol: &eq_hash.claim_protocol,
                instances: &hash.eq_inner_instances,
                proof: &hash.eq_proof,
                history: &eq_hash_native,
                history_fold_proof: hash.eq_history_fold_proof.as_bytes(),
                merge_fold_proof: eq_hash_merge.proof().as_bytes(),
            },
            successor_history: eq_hash_merge.successor().as_bytes(),
        },
        ep: OrdinaryCashTerminalHalfWitnessV1 {
            candidate: OrdinaryCashCandidateCompleteProofV1 {
                protocol: &send.funded.bootstrap.keys.ep_transport_protocol,
                instances: &ep_instances,
                proof: &send.generated.proof.ep_proof,
                history: &ep_history_native,
                history_fold_proof: ep_candidate.proof().as_bytes(),
            },
            preparation_guard: KagemushaOrdinaryGuardCompleteProofV1 {
                protocol: &send.guard.ep.protocol,
                proof: &send.guard.ep.proof,
                history: &g2ep_native,
                history_bytes: send.guard.ep.history.as_bytes(),
                history_fold_proof: ep_preparation.proof().as_bytes(),
            },
            terminal_guard: KagemushaOrdinaryGuardCompleteProofV1 {
                protocol: &send.guard.ep.protocol,
                proof: &terminal_guard.ep.proof,
                history: &g1ep_native,
                history_bytes: terminal_guard.ep.history.as_bytes(),
                history_fold_proof: ep_terminal.proof().as_bytes(),
            },
            preparation_merge_fold_proof: ep_prepare_merge.proof().as_bytes(),
            terminal_merge_fold_proof: ep_terminal_merge.proof().as_bytes(),
            hash_claim: KagemushaRecursiveHashClaimParityWitnessV1 {
                protocol_digests: protocols,
                protocol: &ep_hash.claim_protocol,
                instances: &hash.ep_inner_instances,
                proof: &hash.ep_proof,
                history: &ep_hash_native,
                history_fold_proof: hash.ep_history_fold_proof.as_bytes(),
                merge_fold_proof: ep_hash_merge.proof().as_bytes(),
            },
            successor_history: ep_hash_merge.successor().as_bytes(),
        },
    };

    let manifest = send
        .funded
        .mint_source
        .authorization
        .statement
        .context
        .artifact_manifest_digest;
    let inner = generate_ordinary_terminal_for_testing_v1(witness, manifest, &seed);
    let eq_inner_instances = vec![inner.generated.eq_instances.clone()];
    let ep_inner_instances = vec![inner.generated.ep_instances.clone()];
    let eq_wrapper_fold = fold_kagemusha_eq_accumulators_v1(
        &eq,
        &inner.generated.eq_current,
        &inner.generated.eq_history,
        &seed,
    )
    .unwrap();
    let ep_wrapper_fold = fold_kagemusha_ep_accumulators_v1(
        &ep,
        &inner.generated.ep_current,
        &inner.generated.ep_history,
        &seed,
    )
    .unwrap();
    let eq_inner_history = inner.generated.eq_history.to_native().unwrap();
    let ep_inner_history = inner.generated.ep_history.to_native().unwrap();
    let inner_wire: super::super::ordinary_cash_terminal_verifier::OrdinaryCashProofPairWireV1 =
        norito::decode_canonical(&inner.generated.original).unwrap();
    assert_eq!(
        norito::encode_canonical(&inner_wire).unwrap(),
        inner.generated.original
    );
    let wrapper_public = OrdinaryCashTerminalPublicV1 {
        release_id: inner_wire.release_id,
        eq_protocol_digest: inner_wire.eq_protocol_digest,
        ep_protocol_digest: inner_wire.ep_protocol_digest,
        eq_deferred_audit: inner_wire.eq_deferred_audit,
        ep_deferred_audit: inner_wire.ep_deferred_audit,
        ..public
    };
    let witness = OrdinaryCashCommitWrapperWitnessV1 {
        public: wrapper_public.clone(),
        eq: OrdinaryCashCommitWrapperHalfWitnessV1 {
            protocol: &inner.generated.eq_protocol,
            instances: &eq_inner_instances,
            proof: &inner_wire.eq_proof,
            history: &eq_inner_history,
            history_fold_proof: eq_wrapper_fold.proof().as_bytes(),
            successor_history: eq_wrapper_fold.successor().as_bytes(),
        },
        ep: OrdinaryCashCommitWrapperHalfWitnessV1 {
            protocol: &inner.generated.ep_protocol,
            instances: &ep_inner_instances,
            proof: &inner_wire.ep_proof,
            history: &ep_inner_history,
            history_fold_proof: ep_wrapper_fold.proof().as_bytes(),
            successor_history: ep_wrapper_fold.successor().as_bytes(),
        },
    };
    let wrapper = generate_ordinary_wrapper_for_testing_v1(witness, manifest, &seed);
    assert!(wrapper.generated.wrapper_history_fold_originals.is_some());
    let wrapper_wire: super::super::ordinary_cash_terminal_verifier::OrdinaryCashProofPairWireV1 =
        norito::decode_canonical(&wrapper.generated.original).unwrap();
    let wrapper_public = OrdinaryCashTerminalPublicV1 {
        release_id: wrapper_wire.release_id,
        eq_protocol_digest: wrapper_wire.eq_protocol_digest,
        ep_protocol_digest: wrapper_wire.ep_protocol_digest,
        eq_deferred_audit: wrapper_wire.eq_deferred_audit,
        ep_deferred_audit: wrapper_wire.ep_deferred_audit,
        ..wrapper_public
    };
    // Mutate actual public origins, exact inner audit tuples and complete history limbs. These
    // checks verify the already-created proof and terminally decide the resulting IPA claim.
    for slot in [0, 20, 24, 26, 41, 43, 49] {
        let mut changed = wrapper.generated.eq_instances.clone();
        changed[slot] += Fp::ONE;
        let admitted = verify_eq_succinct_protocol(
            &eq,
            &wrapper.generated.eq_protocol,
            &norito::decode_canonical::<
                super::super::ordinary_cash_terminal_verifier::OrdinaryCashProofPairWireV1,
            >(&wrapper.generated.original)
            .unwrap()
            .eq_proof,
            &changed,
        )
        .ok()
        .and_then(|x| KagemushaEqAccumulatorV1::from_native(&x).ok())
        .is_some_and(|x| decide_kagemusha_eq_accumulator_v1(&eq, &x).is_ok());
        assert!(
            !admitted,
            "actual Wrapper Eq substituted public binding {slot}"
        );
        let mut changed = wrapper.generated.ep_instances.clone();
        changed[slot] += Fq::ONE;
        let admitted = verify_ep_succinct_protocol(
            &ep,
            &wrapper.generated.ep_protocol,
            &norito::decode_canonical::<
                super::super::ordinary_cash_terminal_verifier::OrdinaryCashProofPairWireV1,
            >(&wrapper.generated.original)
            .unwrap()
            .ep_proof,
            &changed,
        )
        .ok()
        .and_then(|x| KagemushaEpAccumulatorV1::from_native(&x).ok())
        .is_some_and(|x| decide_kagemusha_ep_accumulator_v1(&ep, &x).is_ok());
        assert!(
            !admitted,
            "actual Wrapper Ep substituted public binding {slot}"
        );
    }
    // Both substituted folds are themselves genuinely generated. They fold a different
    // history and must not be accepted as the retained complete-inner history originals.
    let eq_wrong = fold_kagemusha_eq_accumulators_v1(
        &eq,
        &inner.generated.eq_current,
        &initial_kagemusha_eq_accumulator_v1(&eq).unwrap(),
        &seed,
    )
    .unwrap();
    let ep_wrong = fold_kagemusha_ep_accumulators_v1(
        &ep,
        &inner.generated.ep_current,
        &initial_kagemusha_ep_accumulator_v1(&ep).unwrap(),
        &seed,
    )
    .unwrap();
    let bad = OrdinaryCashCommitWrapperWitnessV1 {
        public: wrapper_public,
        eq: OrdinaryCashCommitWrapperHalfWitnessV1 {
            protocol: &inner.generated.eq_protocol,
            instances: &eq_inner_instances,
            proof: &inner_wire.eq_proof,
            history: &eq_inner_history,
            history_fold_proof: eq_wrong.proof().as_bytes(),
            successor_history: eq_wrapper_fold.successor().as_bytes(),
        },
        ep: OrdinaryCashCommitWrapperHalfWitnessV1 {
            protocol: &inner.generated.ep_protocol,
            instances: &ep_inner_instances,
            proof: &inner_wire.ep_proof,
            history: &ep_inner_history,
            history_fold_proof: ep_wrong.proof().as_bytes(),
            successor_history: ep_wrapper_fold.successor().as_bytes(),
        },
    };
    use super::super::ordinary_cash_commit_wrapper::{
        build_ordinary_cash_commit_wrapper_ep_v1, build_ordinary_cash_commit_wrapper_eq_v1,
        collect_ordinary_cash_commit_wrapper_audits_v1,
    };
    if let Ok(audits) = collect_ordinary_cash_commit_wrapper_audits_v1(&eq, &ep, &bad) {
        if let Ok((c, columns)) = build_ordinary_cash_commit_wrapper_eq_v1(&eq, &bad, &audits) {
            assert!(
                halo2_proofs::dev::MockProver::run(c.params().k as u32, &c, vec![columns])
                    .unwrap()
                    .verify()
                    .is_err()
            );
        }
        if let Ok((c, columns)) = build_ordinary_cash_commit_wrapper_ep_v1(&ep, &bad, &audits) {
            assert!(
                halo2_proofs::dev::MockProver::run(c.params().k as u32, &c, vec![columns])
                    .unwrap()
                    .verify()
                    .is_err()
            );
        }
    }

    OrdinarySendTerminalForTestingV1 {
        send,
        intent,
        record,
        approval,
        admission_clock_original,
        terminal_guard,
        inner,
        wrapper,
    }
}
