//! Full active Receive State with the actual funded Send Terminal/Wrapper and all histories.
//! Mathematical known-public FI/clock/original data cannot create Native or service authority.
use super::super::composite::RecursiveStateConstructionV1;
use super::super::ordinary_guard_circuit::OrdinaryGuardWitnessV1;
use super::ordinary_active_send_terminal::OrdinarySendTerminalForTestingV1;
use super::ordinary_active_state_bootstrap::OrdinaryBootstrapStateForTestingV1;
use super::ordinary_mint_genuine_qualification_tests::{
    active_mint_state::{require_full_relation_rejection, require_public_mutations_rejected},
    sign_message_with_counter,
};
use super::*;
use crate::kagemusha_v1_recursion::KagemushaReplayInsertWitnessV1;
use crate::kagemusha_v1_recursion::real_handoff_qualification_tests::terminally_verify_state_proof;
use crate::kagemusha_v1_state::{
    CreditIdV1, KagemushaTransitionKindV1, OrdinaryConsumedCreditsForQualificationV1,
    ordinary_incoming_preview_for_qualification_v1,
};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature, SignatureOf};
use iroha_data_model::kagemusha::*;

/// Actual receiver proof and its complete public original; no owner or effect capability.
pub(super) struct OrdinaryReceiveStateForTestingV1 {
    pub(super) receiver: OrdinaryBootstrapStateForTestingV1,
    pub(super) state: KagemushaStateV1,
    pub(super) generated: KagemushaGeneratedRecursiveStateProofV1,
    pub(super) public_original: Vec<u8>,
}
/// Execute genuine both-parity Receive from the same real sender Wrapper, exact original
/// request, actual maintained AEAD plaintext, original key/counter scope, fresh W2 and replay tree.
pub(super) fn prove_ordinary_receive_state_for_testing_v1(
    sender: &OrdinarySendTerminalForTestingV1,
    receiver: OrdinaryBootstrapStateForTestingV1,
    apple: bool,
) -> OrdinaryReceiveStateForTestingV1 {
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let seed = KagemushaRecoverySeedV1::from_unsealed([44; 32]).unwrap();
    let c = &receiver.credential;
    assert_eq!(&sender.send.receiver_credential, c);
    let source = &sender.send.funded.mint_source;
    let wrapper_eq = &sender.wrapper.generated.eq_protocol;
    let wrapper_ep = &sender.wrapper.generated.ep_protocol;
    // Genuine mathematical family equality, independent of account and active operation.
    assert_eq!(
        receiver
            .keys
            .eq
            .inner_verifying_key
            .to_bytes(SerdeFormat::Processed),
        sender
            .send
            .funded
            .bootstrap
            .keys
            .eq
            .inner_verifying_key
            .to_bytes(SerdeFormat::Processed)
    );
    assert_eq!(
        receiver
            .keys
            .ep
            .inner_verifying_key
            .to_bytes(SerdeFormat::Processed),
        sender
            .send
            .funded
            .bootstrap
            .keys
            .ep
            .inner_verifying_key
            .to_bytes(SerdeFormat::Processed)
    );
    assert_eq!(
        receiver
            .keys
            .eq
            .verifying_key
            .to_bytes(SerdeFormat::Processed),
        sender
            .send
            .funded
            .bootstrap
            .keys
            .eq
            .verifying_key
            .to_bytes(SerdeFormat::Processed)
    );
    assert_eq!(
        receiver
            .keys
            .ep
            .verifying_key
            .to_bytes(SerdeFormat::Processed),
        sender
            .send
            .funded
            .bootstrap
            .keys
            .ep
            .verifying_key
            .to_bytes(SerdeFormat::Processed)
    );
    let outgoing = outgoing_data(sender);
    let outgoing_original = outgoing.canonical_bytes().unwrap();
    let request = &sender.send.request;
    let output = &sender.send.preview.output;
    let encrypted = &sender.send.preview.encrypted_credit;
    let envelope =
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            encrypted,
            request.body.recipient_encryption_key,
        )
        .unwrap();
    let aad = output
        .encrypted_credit_aad_against(request, &sender.send.preparation_clock)
        .unwrap();
    let opened = crate::kagemusha_v1_crypto::open_kagemusha_credit_v1(
        &envelope,
        &aad,
        request.body.recipient_encryption_key,
        &[11; 32],
    )
    .unwrap();
    assert_eq!(opened, sender.send.preview.credit_opening);
    let mut artifacts = crate::kagemusha_v1_recursion::tests::artifacts();
    artifacts.release_id = receiver.state.release_id;
    artifacts.artifact_manifest_digest = source
        .authorization
        .statement
        .context
        .artifact_manifest_digest;
    artifacts.eq_protocol_digest =
        native_parent_protocol_digest_v1(&receiver.keys.eq_protocol, KagemushaPastaParityV1::Eq)
            .unwrap();
    artifacts.ep_protocol_digest =
        native_parent_protocol_digest_v1(&receiver.keys.ep_protocol, KagemushaPastaParityV1::Ep)
            .unwrap();
    artifacts.guard_bundle_eq_protocol_digest =
        native_parent_protocol_digest_v1(&receiver.guard_eq_protocol, KagemushaPastaParityV1::Eq)
            .unwrap();
    artifacts.guard_bundle_ep_protocol_digest =
        native_parent_protocol_digest_v1(&receiver.guard_ep_protocol, KagemushaPastaParityV1::Ep)
            .unwrap();
    artifacts.mint_authorization_eq_protocol_digest = source.pair.eq.protocol_digest;
    artifacts.mint_authorization_ep_protocol_digest = source.pair.ep.protocol_digest;
    artifacts.mint_finality_eq_protocol_digest = source.source.credit.proof.eq_protocol_digest;
    artifacts.mint_finality_ep_protocol_digest = source.source.credit.proof.ep_protocol_digest;
    artifacts.commit_wrapper_eq_protocol_digest =
        native_parent_protocol_digest_v1(wrapper_eq, KagemushaPastaParityV1::Eq).unwrap();
    artifacts.commit_wrapper_ep_protocol_digest =
        native_parent_protocol_digest_v1(wrapper_ep, KagemushaPastaParityV1::Ep).unwrap();
    artifacts.canonical_empty_effect_digest =
        receiver.original_relation.canonical_empty_effect_digest;
    let account = ordinary_qualification_wallet_account_v1(63);
    let owner = owner_data(&receiver.state, account.clone());
    assert_eq!(
        kagemusha_ordinary_app_account_binding_v1(&account),
        c.subject.account_binding
    );
    let fresh_clock = KagemushaOrdinaryCashClockContextV1 {
        version: 1,
        request_nonce: [111; 32],
        signed_observations_original_digest: [112; 32],
        lower_at_ms: 2500,
        upper_at_ms: 2501,
    };
    // The external immutable Core assertion is deliberately data only in this mathematical test.
    // No service/Native assertion capability or finalized money effect is manufactured.
    let external_commit_sha =
        Sha256::digest(b"known-public external finalized Core assertion DATA").into();
    let reservation = KagemushaOrdinaryIncomingReservationV1 {
        selection: KagemushaOrdinaryIncomingSelectionV1 {
            version: 1,
            lineage: KagemushaOrdinaryFinancialLineageV1 {
                version: 1,
                owner,
                financial_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(&c.subject).unwrap(),
                financial_authority_commitment: c.subject.financial_authority_commitment,
            },
            operation_id: [113; 32],
            predecessor: KagemushaOrdinaryFinancialHeadV1 {
                state_commitment: receiver.state.state_commitment,
                logical_sequence: receiver.state.logical_sequence,
                state_original_sha256: Sha256::digest(&receiver.public_original).into(),
            },
            source: KagemushaOrdinaryIncomingSourceSelectionV1::Receive {
                sender_commit_transport_original_sha256: external_commit_sha,
                sender_outgoing_original_sha256: Sha256::digest(&outgoing_original).into(),
                recipient_request_original_digest: request.canonical_original_digest().unwrap(),
                encrypted_credit_original_sha256: Sha256::digest(encrypted).into(),
            },
            credit_id: output.credit_id,
            amount: output.amount,
            scale: receiver.state.lane.scale,
            recipient_app_credential_digest: c.canonical_digest().unwrap(),
            financial_control_original_sha256: [114; 32],
            clock_context_digest: fresh_clock.binding_digest().unwrap(),
        },
        finalized_source_original_sha256: external_commit_sha,
        source_proof_original_sha256: Sha256::digest(&outgoing_original).into(),
        source_semantic_digest: output.binding_digest().unwrap(),
    };
    reservation.validate_shape().unwrap();
    let mut consumed = OrdinaryConsumedCreditsForQualificationV1::empty();
    assert_eq!(consumed.root(), receiver.state.consumed_credit_root);
    let replay = consumed
        .preview(CreditIdV1(output.credit_id), reservation.digest().unwrap())
        .unwrap();
    let preview = ordinary_incoming_preview_for_qualification_v1(
        &receiver.state,
        &reservation,
        KagemushaTransitionKindV1::ReceiveFold,
        [0; 32],
        [0; 32],
        &replay,
        [115; 32],
        0,
        [114; 32],
        &fresh_clock,
        [116; 32],
        artifacts,
    )
    .unwrap();
    assert_eq!(preview.successor.balance, 17);
    assert_eq!(preview.successor.secure_index, 1);
    let mut subject = receiver.original_subject.clone();
    subject.operation_kind = KagemushaOperationKindV1::ReceiveFold;
    subject.transition_statement_digest = preview.statement.digest().unwrap();
    subject.candidate_envelope_digest = [0; 32];
    subject.terminal_body_commitment = [0; 32];
    subject.secure_index_before = receiver.state.secure_index;
    subject.secure_index_after = preview.successor.secure_index;
    let challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::PrepareTransition,
        operation_id: preview.preparation.operation_id,
        nonce: preview.preparation.nonce,
        account_binding: c.subject.account_binding,
        authority_policy_digest: c.subject.app_authority_policy_digest,
        attested_key_id: c.subject.attested_key_id,
        enrollment_digest: c.canonical_digest().unwrap(),
        subject_signing_digest: Sha256::digest(subject.canonical_prepare_signing_bytes().unwrap())
            .into(),
        normalized_guard_digest: preview.normalized.canonical_digest().unwrap(),
        issued_at_ms: 2501,
        expires_at_ms: 2501 + crate::kagemusha_v1_state::ORDINARY_PREPARATION_LIFETIME_MS,
        subject,
    };
    let approval = KagemushaAppOperationApprovalV1 {
        evidence: sign_message_with_counter(
            &challenge.canonical_signing_bytes().unwrap(),
            apple,
            19,
        ),
        challenge,
    };
    let previous_counter = apple.then_some(18);
    let mut guard_relation = receiver.original_relation.clone();
    guard_relation.statement = preview.normalized.clone();
    let guard = super::ordinary_guard_generation::generate_ordinary_guard_pair_v1(
        OrdinaryGuardWitnessV1 {
            relation: &guard_relation,
            credential: c,
            approval: &approval,
            previous_app_attest_counter: previous_counter,
            integrity_lease: None,
            incoming_terminal_body: None,
        },
        receiver.state.device_policy_binding.hardware_policy_id,
        &receiver.issuer_table,
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
    let wire: super::super::ordinary_cash_terminal_verifier::OrdinaryCashProofPairWireV1 =
        norito::decode_canonical(&sender.wrapper.generated.original).unwrap();
    assert_eq!(
        norito::encode_canonical(&wire).unwrap(),
        sender.wrapper.generated.original
    );
    let eq_wrapper_columns = vec![sender.wrapper.generated.eq_instances.clone()];
    let ep_wrapper_columns = vec![sender.wrapper.generated.ep_instances.clone()];
    let eq_incoming_history = sender.wrapper.generated.eq_history.clone();
    let ep_incoming_history = sender.wrapper.generated.ep_history.clone();
    let eq_inner_column = vec![receiver.generated.eq_public_instances.clone()];
    let ep_inner_column = vec![receiver.generated.ep_public_instances.clone()];
    let eq_outer_column = vec![receiver.generated.eq_transport_public_instances.clone()];
    let ep_outer_column = vec![receiver.generated.ep_transport_public_instances.clone()];
    let eq_inner_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq,
            &receiver.keys.eq_protocol,
            &receiver.generated.eq_inner_proof,
            &eq_inner_column[0],
        )
        .unwrap(),
    )
    .unwrap();
    let ep_inner_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep,
            &receiver.keys.ep_protocol,
            &receiver.generated.ep_inner_proof,
            &ep_inner_column[0],
        )
        .unwrap(),
    )
    .unwrap();
    let eq_parent_complete = fold_kagemusha_eq_accumulators_v1(
        &eq,
        &eq_inner_current,
        &receiver.generated.eq_history,
        &seed,
    )
    .unwrap();
    let ep_parent_complete = fold_kagemusha_ep_accumulators_v1(
        &ep,
        &ep_inner_current,
        &receiver.generated.ep_history,
        &seed,
    )
    .unwrap();
    let eq_outer_history =
        KagemushaEqAccumulatorV1::try_from_bytes(&receiver.generated.proof.eq_history).unwrap();
    let ep_outer_history =
        KagemushaEpAccumulatorV1::try_from_bytes(&receiver.generated.proof.ep_history).unwrap();
    let eq_outer_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq,
            &receiver.keys.eq_transport_protocol,
            &receiver.generated.proof.eq_proof,
            &eq_outer_column[0],
        )
        .unwrap(),
    )
    .unwrap();
    let ep_outer_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep,
            &receiver.keys.ep_transport_protocol,
            &receiver.generated.proof.ep_proof,
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

    let eq_incoming_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(&eq, wrapper_eq, &wire.eq_proof, &eq_wrapper_columns[0])
            .unwrap(),
    )
    .unwrap();
    let ep_incoming_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(&ep, wrapper_ep, &wire.ep_proof, &ep_wrapper_columns[0])
            .unwrap(),
    )
    .unwrap();
    let eq_incoming_complete =
        fold_kagemusha_eq_accumulators_v1(&eq, &eq_incoming_current, &eq_incoming_history, &seed)
            .unwrap();
    let ep_incoming_complete =
        fold_kagemusha_ep_accumulators_v1(&ep, &ep_incoming_current, &ep_incoming_history, &seed)
            .unwrap();
    let eq_incoming_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        eq_outer_merge.successor(),
        eq_incoming_complete.successor(),
        &seed,
    )
    .unwrap();
    let ep_incoming_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_outer_merge.successor(),
        ep_incoming_complete.successor(),
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
        eq_incoming_merge.successor(),
        eq_guard_complete.successor(),
        &seed,
    )
    .unwrap();
    let ep_guard_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        ep_incoming_merge.successor(),
        ep_guard_complete.successor(),
        &seed,
    )
    .unwrap();
    let (inactive_authorization, inactive_credit) =
        crate::kagemusha_v1_recursion::generation::production_prover::ordinary_bootstrap_padding_for_testing(
            &receiver.state,
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

    let receive_credit = super::super::KagemushaReceiveFoldCreditV1 {
        amount: output.amount,
        credit_id: output.credit_id,
        recipient_lane_id: request.body.recipient_lane_id,
        incoming_proof_binding_digest: sender.record.body.candidate_digest,
        request_digest: request.canonical_original_digest().unwrap(),
        prepared_transfer_digest: sender.record.body.binding_digest().unwrap(),
        transition_nullifier: output.transition_nullifier,
        recipient_encryption_key: request.body.recipient_encryption_key,
        ciphertext_commitment: output.ciphertext_commitment,
        credit_opening: opened,
        receiver_binding_digest: c.canonical_digest().unwrap(),
        payment_output_digest: output.binding_digest().unwrap(),
        replay_insert: KagemushaReplayInsertWitnessV1::from(&replay),
    };
    let t = &preview.statement;
    let mut witness = KagemushaRecursiveStateGenerationWitnessV1 {
        hash_claim: None,
        state: KagemushaStateRelationWitnessV1 {
            operation: KagemushaOperationV1::ReceiveFold,
            predecessor: Some(receiver.state.clone()),
            successor: preview.successor.clone(),
            amount: t.amount,
            journal_revision_before: t.journal_revision_before,
            journal_revision_after: t.journal_revision_after,
            transition_effect_digest: t.effect_digest,
            mint_finality_semantic_digest: t.mint_finality_semantic_digest,
            mint_finality_proof_binding_digest: t.mint_finality_proof_binding_digest,
            peer_credit_id: t.peer_credit_id,
            recipient_encryption_key_binding: t.recipient_encryption_key_binding,
            receive_credit: Some(receive_credit.clone()),
            receive_credit_binding_digest: t.receive_credit_binding_digest,
            lifecycle_binding_digest: t.lifecycle_binding_digest,
            prepared_transition_binding_digest: t.prepared_transition_binding_digest,
            prepared_intent: None,
            transport_semantic_digest: preview.transport_semantic_digest,
            guard_statement_digest: preview.normalized.canonical_digest().unwrap(),
            eq_protocol_digest: native_parent_protocol_digest_v1(
                &receiver.keys.eq_protocol,
                KagemushaPastaParityV1::Eq,
            )
            .unwrap(),
            ep_protocol_digest: native_parent_protocol_digest_v1(
                &receiver.keys.ep_protocol,
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
            guard_eq_credential_audit: receiver.keys.eq_protocol_digest,
            guard_ep_credential_audit: receiver.keys.ep_protocol_digest,
            eq_deferred_audit: [1; 32],
            ep_deferred_audit: [2; 32],
            replay_insert: None,
        },
        mint_fold_opening: None,
        mint_authorization: &inactive_authorization,
        mint_credit: &inactive_credit,
        guard_relation,
        hardware_selection: None,
        ordinary_selection: Some(KagemushaOrdinaryAppRecursiveSelectionWitnessV1 {
            credential: c,
            approval: &approval,
            integrity_lease: None,
            previous_app_attest_counter: previous_counter,
            prepared: None,
            outer_parent: Some(KagemushaOrdinaryRecursiveOuterParentWitnessV1 {
                public_original: Some(&receiver.public_original),
                eq_protocol: &receiver.keys.eq_transport_protocol,
                ep_protocol: &receiver.keys.ep_transport_protocol,
                eq_instances: &eq_outer_column,
                ep_instances: &ep_outer_column,
                eq_proof: &receiver.generated.proof.eq_proof,
                ep_proof: &receiver.generated.proof.ep_proof,
                eq_history: &eq_outer_history,
                ep_history: &ep_outer_history,
                eq_history_fold: eq_outer_complete.proof(),
                ep_history_fold: ep_outer_complete.proof(),
                eq_merge_fold: eq_outer_merge.proof(),
                ep_merge_fold: ep_outer_merge.proof(),
            }),
            incoming_mint: None,
            incoming_receive: Some(KagemushaOrdinaryRecursiveReceiveIncomingOpeningV1 {
                outgoing: &outgoing,
                reservation: &reservation,
                preparation: &preview.preparation,
                receiver_credential: c,
                receiver_integrity_lease: None,
                previous_receiver_app_attest_counter: sender.send.previous_receiver_counter,
                credit_opening: &opened,
            }),
        }),
        eq_parent_protocol: &receiver.keys.eq_protocol,
        ep_parent_protocol: &receiver.keys.ep_protocol,
        eq_parent_instances: &eq_inner_column,
        ep_parent_instances: &ep_inner_column,
        eq_parent_proof: &receiver.generated.eq_inner_proof,
        ep_parent_proof: &receiver.generated.ep_inner_proof,
        eq_predecessor_history: &receiver.generated.eq_history,
        ep_predecessor_history: &receiver.generated.ep_history,
        eq_parent_fold_proof: eq_parent_complete.proof(),
        ep_parent_fold_proof: ep_parent_complete.proof(),
        eq_incoming_protocol: wrapper_eq,
        ep_incoming_protocol: wrapper_ep,
        eq_incoming_credits: [KagemushaRecursiveIncomingEqGenerationWitnessV1 {
            instances: &eq_wrapper_columns,
            proof: &wire.eq_proof,
            history: &eq_incoming_history,
            history_fold_proof: eq_incoming_complete.proof(),
            merge_fold_proof: eq_incoming_merge.proof(),
        }],
        ep_incoming_credits: [KagemushaRecursiveIncomingEpGenerationWitnessV1 {
            instances: &ep_wrapper_columns,
            proof: &wire.ep_proof,
            history: &ep_incoming_history,
            history_fold_proof: ep_incoming_complete.proof(),
            merge_fold_proof: ep_incoming_merge.proof(),
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
    let construction = RecursiveStateConstructionV1::OrdinaryReceiveIncomingQualification;
    let claim = prove_kagemusha_recursive_state_hash_claim_v1_with_construction(
        &source.source.hash_eq,
        &source.source.hash_ep,
        witness.clone(),
        &seed,
        construction,
    )
    .expect("full actual Receive original SHA and whole history queue");
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
            &receiver.keys.eq,
            &receiver.keys.ep,
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
        &receiver.keys.eq,
        &receiver.keys.ep,
        witness.clone(),
        &seed,
        construction,
    )
    .expect("actual Receive bothparity inner+outer proof under the same retained family");
    terminally_verify_state_proof(&receiver.keys, &generated);
    require_public_mutations_rejected(&receiver.keys, &generated);

    let mut bad = witness.clone();
    bad.state
        .receive_credit
        .as_mut()
        .unwrap()
        .credit_opening
        .recipient_binding_opening[0] ^= 1;
    require_full_relation_rejection(&eq, &ep, bad, construction);
    let mut changed_reservation = reservation.clone();
    changed_reservation.source_proof_original_sha256[0] ^= 1;
    let mut bad = witness.clone();
    bad.ordinary_selection
        .as_mut()
        .unwrap()
        .incoming_receive
        .as_mut()
        .unwrap()
        .reservation = &changed_reservation;
    require_full_relation_rejection(&eq, &ep, bad, construction);
    let mut changed_parent = receiver.public_original.clone();
    let last = changed_parent.len() - 1;
    changed_parent[last] ^= 1;
    let mut bad = witness.clone();
    bad.ordinary_selection
        .as_mut()
        .unwrap()
        .outer_parent
        .as_mut()
        .unwrap()
        .public_original = Some(&changed_parent);
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
    consumed.install(&replay).unwrap();
    assert_eq!(consumed.root(), preview.successor.consumed_credit_root);
    assert!(
        consumed
            .preview(CreditIdV1(output.credit_id), [119; 32])
            .is_err(),
        "same consumed identity cannot be re-keyed by another reservation/FI/head/operation"
    );
    drop(witness);
    drop(claim);
    OrdinaryReceiveStateForTestingV1 {
        state: preview.successor,
        generated,
        public_original,
        receiver,
    }
}

// Complete known-public signed DATA only. These helpers never authenticate a current World,
// grant an FI/Clock/Native owner, or stand in for the independently admitted Core assertion.
fn owner_data(
    state: &KagemushaStateV1,
    account: iroha_data_model::account::AccountId,
) -> KagemushaRetailEnrollmentOwnerV1 {
    KagemushaRetailEnrollmentOwnerV1 {
        account_id: account,
        runtime: KagemushaRetailEnrollmentRuntimeV1 {
            fi_id: "qualification-fi".parse().unwrap(),
            ledger_dataspace_id: iroha_model_base::topology::DataSpaceId::new(1),
            authentication_namespace: "qualification-auth".parse().unwrap(),
            network_id: state.lane.network_id,
            asset: state.lane.asset.clone(),
            asset_incarnation: state.asset_incarnation,
            scale: state.lane.scale,
        },
        lane_id: state.lane.device_lane_id,
    }
}
fn outgoing_data(
    sender: &OrdinarySendTerminalForTestingV1,
) -> super::super::KagemushaOrdinaryCashOutgoingOriginalV1 {
    let state = &sender.send.funded.state;
    let c = &sender.send.funded.bootstrap.credential;
    let owner = owner_data(state, ordinary_qualification_wallet_account_v1(62));
    assert_eq!(
        kagemusha_ordinary_app_account_binding_v1(&owner.account_id),
        c.subject.account_binding
    );
    let issuer = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
    let policy = KagemushaRetailEnrollmentIssuerPolicyV1 {
        version: 1,
        issuer_policy_id: [121; 32],
        issuer_public_key: issuer.public_key().clone(),
        issuer_audience: "qualification-fi-audience".parse().unwrap(),
        runtime: owner.runtime.clone(),
        valid_from_ms: 1,
        expires_at_ms: 50_000,
        maximum_certificate_lifetime_ms: 49_999,
    };
    let subject = KagemushaOrdinaryRetailEnrollmentSubjectV1 {
        version: 1,
        enrollment_id: owner.enrollment_id().unwrap(),
        issuer_policy_id: policy.issuer_policy_id,
        issuer_audience: policy.issuer_audience.clone(),
        owner: owner.clone(),
        issuance: KagemushaOrdinaryRetailEnrollmentIssuanceV1 {
            release_id: state.release_id,
            hardware_policy_digest: state.device_policy_binding.hardware_policy_id,
            core_authorization_key_reference: [122; 32],
            credential: c.clone(),
        },
        challenge_evidence_digest: [123; 32],
        ordinary_app_credential_digest: c.canonical_digest().unwrap(),
        issued_at_ms: 100,
        expires_at_ms: 30_000,
    };
    let signature =
        SignatureOf::try_new(issuer.private_key(), &subject.approval_payload().unwrap()).unwrap();
    let certificate = KagemushaOrdinaryRetailEnrollmentCertificateV1 { subject, signature };
    // Actual Ed equation over the whole exact original, with no test accepting authority.
    certificate
        .signature
        .verify(
            issuer.public_key(),
            &certificate.subject.approval_payload().unwrap(),
        )
        .unwrap();
    let certificate_original = certificate.canonical_bytes().unwrap();
    let request = KagemushaOrdinaryCurrentControlRequestV1 {
        version: 1,
        request_nonce: [124; 32],
        owner,
        enrollment_original_sha256: Sha256::digest(&certificate_original).into(),
        credential_original_sha256: Sha256::digest(c.canonical_bytes().unwrap()).into(),
        issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(&policy).unwrap(),
    };
    let subject = KagemushaOrdinaryCurrentControlSubjectV1 {
        request: request.clone(),
        release_id: state.release_id,
        hardware_profile_id: c.subject.hardware_profile_id,
        profile_policy_epoch: state.policy_epoch,
        ordinary_trust_policy_digest: [125; 32],
        app_authority_policy_digest: c.subject.app_authority_policy_digest,
        authority_height: 2,
        authority_context_id: Hash::new(b"known-public current-control context DATA"),
        world_root: Hash::new(b"known-public World root DATA; no membership capability"),
        world_schema_hash: Hash::new(b"known-public World schema DATA"),
        asset_definition_original_sha256: [126; 32],
        verifier_registry_original_sha256: [127; 32],
        data_incarnation_digest: [128; 32],
        data_revision: 1,
        data_policy_epoch: 1,
        data_schema_epoch: 1,
        latest_integrity_lease_original: None,
        issued_at_ms: 2000,
        expires_at_ms: 12_000,
    };
    let signature = Signature::new(
        issuer.private_key(),
        &subject.issuer_signing_message().unwrap(),
    );
    let control = KagemushaSignedOrdinaryCurrentControlV1 { subject, signature };
    control.verify_for_request(&request, &policy).unwrap();
    super::super::KagemushaOrdinaryCashOutgoingOriginalV1::from_public_parts(
        sender.send.preview.normalized.clone(),
        sender.send.preview.statement.clone(),
        super::super::KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
            request: Box::new(sender.send.request.clone()),
            output: sender.send.preview.output,
            encrypted_credit: sender.send.preview.encrypted_credit.clone(),
            preparation_clock: sender.send.preparation_clock,
        },
        sender.send.prepared,
        sender.intent,
        sender.record.clone(),
        c.canonical_bytes().unwrap(),
        norito::encode_canonical(&sender.approval).unwrap(),
        None,
        certificate_original,
        control.canonical_bytes().unwrap(),
        sender.admission_clock_original.clone(),
        sender.wrapper.generated.original.clone(),
    )
    .unwrap()
}
