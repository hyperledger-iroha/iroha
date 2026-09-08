//! Full ordinary Terminal queues from canonical synthetic outbound inputs, without proof authority.
//!
//! The two balances below are explicit arithmetic fixtures. No machine is funded or installed,
//! no proof is fabricated or accepted, and the structurally sealed certificate is never treated
//! as an OEM commit. Redemption retains inactive send transcripts; SendSplit uses the existing
//! signed fixture request and activates the sender, receiver, prepared-transfer and time bindings.

use super::*;
use crate::zk::kagemusha_v1_recursion::{
    canonical_incoming_payment_claims_binding_v1, canonical_sender_state_pair_digest_v1,
    real_handoff_qualification_tests::aggregate_state_with_balance,
    terminal_authorization::{
        canonical_commit_certificate_digest_v1, canonical_prepared_one_use_authorization_digest_v1,
        canonical_terminal_send_output_binding_v1,
    },
};
use halo2_proofs::poly::commitment::ParamsProver as _;
use iroha_data_model::kagemusha::{
    KagemushaHardwareTerminalBodyV1, KagemushaLifecycleBindingV1, KagemushaPaymentOutputV1,
    kagemusha_outbox_min_reserved_bytes_v1, kagemusha_payment_body_digest_from_digests_v1,
    kagemusha_prepared_transfer_digest_v1,
};

fn shape_protocol<C>(width: usize) -> PlonkProtocol<C>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let params = ParamsIPA::<C>::new(8);
    let mut builder = halo2_base::gates::circuit::builder::BaseCircuitBuilder::new(false)
        .use_k(8)
        .use_lookup_bits(7)
        .use_instance_columns(1);
    let cell = builder.main(0).load_witness(C::ScalarExt::ZERO);
    builder.assigned_instances = vec![vec![cell; width]];
    builder.calculate_params(Some(9));
    let vk = halo2_proofs::plonk::keygen_vk(&params, &builder).unwrap();
    snark_verifier::system::halo2::compile(
        &params,
        &vk,
        snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![width]),
    )
}

#[test]
fn terminal_complete_semantic_queue_matches_original_assignment_in_both_fields() {
    assert_complete_semantic_queue_matches_original(KagemushaOperationV1::RedeemSplit, false, 0);
}

#[test]
fn terminal_complete_send_semantic_queue_matches_original_assignment_in_both_fields() {
    assert_complete_semantic_queue_matches_original(KagemushaOperationV1::SendSplit, false, 0);
}

#[test]
fn terminal_redeem_sha_queue_is_independent_of_its_own_proof_outputs() {
    assert_complete_semantic_queue_matches_original(KagemushaOperationV1::RedeemSplit, true, 0);
}

#[test]
fn terminal_send_sha_queue_is_independent_of_its_own_proof_outputs() {
    assert_complete_semantic_queue_matches_original(KagemushaOperationV1::SendSplit, true, 0);
}

#[test]
fn terminal_sha_queue_changes_with_a_consistently_rebound_semantic_transition() {
    for operation in [
        KagemushaOperationV1::SendSplit,
        KagemushaOperationV1::RedeemSplit,
    ] {
        let original = assert_complete_semantic_queue_matches_original(operation, false, 0);
        let changed = assert_complete_semantic_queue_matches_original(operation, false, 1);
        for (original, changed) in original.iter().zip(&changed) {
            assert_ne!(
                original, changed,
                "semantic intent must change its ordered claim"
            );
            assert_ne!(
                original.last(),
                changed.last(),
                "rebound candidate must change"
            );
        }
    }
}

fn assert_complete_semantic_queue_matches_original(
    operation: KagemushaOperationV1,
    check_independence: bool,
    intent_variant: u64,
) -> [Vec<Vec<u8>>; 2] {
    assert!(matches!(
        operation,
        KagemushaOperationV1::RedeemSplit | KagemushaOperationV1::SendSplit
    ));
    let release = digest(b"terminal-shared-semantic-fixture", 0);
    let material = core_bound_mint_recipient_material(
        release,
        digest(b"vk-set", 0),
        digest(b"terminal-semantic-fixture-manifest", 0),
        1_000,
    );
    let credential = &material.platform_credential.statement;
    // These are synthetic arithmetic witnesses, not a funded Bootstrap or authorized state.
    let predecessor = aggregate_state_with_balance(
        release,
        credential,
        digest(b"terminal-semantic-fixture-before", 0),
        1_000,
        5,
    );
    let prepared_send = (operation == KagemushaOperationV1::SendSplit)
        .then(|| send_preparation(&material, &predecessor, 5));
    let successor = aggregate_state_with_balance(
        release,
        credential,
        prepared_send
            .as_ref()
            .map(|(preparation, _)| preparation.successor_state_nonce_commitment)
            .unwrap_or_else(|| digest(b"terminal-semantic-fixture-after", 0)),
        600,
        6,
    );
    predecessor.validate().unwrap();
    successor.validate().unwrap();
    let openings = prepared_send
        .as_ref()
        .map(|(_, openings)| openings.clone())
        .unwrap_or_else(|| DiagnosticSenderOpeningsV1 {
            predecessor: predecessor.clone(),
            journal_revision_before: 5,
            journal_revision_after: 6,
            authorization_counter_before: 0,
            authorization_counter_after: 1,
            one_use_hardware_authorization: digest(b"terminal-semantic-fixture-one-use", 0),
            commit_evidence_opening: KagemushaCommitEvidenceOpeningV1 {
                opening: digest(b"terminal-semantic-fixture-time", 0),
                trusted_commit_time_ms: SEND_TIME,
                lease_id: [0; 32],
                lease_valid_from_ms: 0,
                lease_expires_at_ms: 0,
            },
        });
    let authorization = canonical_prepared_one_use_authorization_digest_v1(
        operation,
        openings.one_use_hardware_authorization,
        &predecessor,
        5,
        0,
    );
    let nullifier = canonical_predecessor_conflict_nullifier_v1(authorization);
    let reservation = prepared_send
        .as_ref()
        .map(|(preparation, _)| preparation.outbox_reservation)
        .unwrap_or(KagemushaOutboxReservationV1 {
            reservation_id: digest(b"terminal-semantic-fixture-reservation", 0),
            operation_kind: KagemushaOperationKindV1::RedeemSplit,
            reserved_outbox_bytes: kagemusha_outbox_min_reserved_bytes_v1(
                KagemushaOperationKindV1::RedeemSplit,
            )
            .expect("redemption has a canonical complete-artifact reservation minimum"),
            issued_at_ms: 250,
            expires_at_ms: 1_000,
        });
    reservation
        .validate()
        .expect("synthetic fixture reserves every recoverable outbound artifact");
    let mut insufficient_reservation = reservation;
    insufficient_reservation.reserved_outbox_bytes =
        kagemusha_outbox_min_reserved_bytes_v1(reservation.operation_kind).unwrap() - 1;
    assert!(insufficient_reservation.validate().is_err());
    let send = prepared_send.map(|(preparation, openings)| {
        openings.validate_preparation(&preparation).unwrap();
        assert_eq!(
            preparation.prepared_one_use_authorization_digest,
            authorization
        );
        assert_eq!(preparation.transition_nullifier, nullifier);
        preparation
            .request
            .validate_against_profile(&material.hardware_profile)
            .unwrap();
        let output = KagemushaPaymentOutputV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            request_digest: preparation.request.canonical_digest().unwrap(),
            amount: preparation.request.amount,
            sender_before_commitment: predecessor.state_commitment,
            sender_after_commitment: successor.state_commitment,
            transition_nullifier: nullifier,
            credit_id: [0; 32],
            ciphertext_commitment: preparation.ciphertext_commitment,
            commit_evidence: preparation.commit_evidence,
            committed_at_ms: preparation.commit_authorization_reference_ms,
        }
        .seal_credit_id_against(&preparation.request)
        .unwrap();
        output
            .canonical_digest_against(&preparation.request)
            .unwrap();
        KagemushaTerminalSendPrivateV1 {
            request: preparation.request,
            output,
            encrypted_credit_digest: kagemusha_ciphertext_digest_v1(&preparation.encrypted_credit),
        }
    });
    let request_digest = send
        .as_ref()
        .map(|send| send.output.request_digest)
        .unwrap_or([0; 32]);
    let lifecycle = KagemushaLifecycleBindingV1 {
        version: 1,
        network_id: predecessor.lane.network_id,
        protocol_version: 1,
        suite_id: predecessor.suite_id,
        vk_digest: predecessor.vk_digest,
        release_id: release,
        asset: predecessor.lane.asset.clone(),
        asset_incarnation: predecessor.asset_incarnation,
        scale: predecessor.lane.scale,
        liability_pool_id: predecessor.liability_pool_id,
        hardware_profile_id: predecessor.hardware_profile_id,
        policy_epoch: predecessor.policy_epoch,
        operation_kind: if send.is_some() {
            KagemushaOperationKindV1::SendSplit
        } else {
            KagemushaOperationKindV1::RedeemSplit
        },
        request_id: send
            .as_ref()
            .map(|send| send.request.request_id)
            .unwrap_or([0; 32]),
        receiver_lane_commitment: send
            .as_ref()
            .map(|send| send.request.hardware_credential.lane_commitment)
            .unwrap_or([0; 32]),
        credit_id: send
            .as_ref()
            .map(|send| send.output.credit_id)
            .unwrap_or([0; 32]),
        ciphertext_digest: send
            .as_ref()
            .map(|send| send.encrypted_credit_digest)
            .unwrap_or([0; 32]),
    };
    lifecycle.validate().unwrap();
    let lifecycle_digest = lifecycle.canonical_digest().unwrap();
    let reservation_digest = canonical_outbox_reservation_commitment_v1(reservation).unwrap();
    let prepared = canonical_prepared_transition_binding_digest_v1(
        lifecycle_digest,
        request_digest,
        send.as_ref()
            .map(|send| send.output.sender_before_commitment)
            .unwrap_or([0; 32]),
        send.as_ref()
            .map(|send| send.output.sender_after_commitment)
            .unwrap_or([0; 32]),
        400,
        reservation_digest,
        authorization,
    );
    let mut normalized = KagemushaNormalizedGuardStatementV1 {
        version: 1,
        protocol_version: credential.protocol_version,
        predecessor_suite_id: predecessor.suite_id,
        predecessor_vk_digest: predecessor.vk_digest,
        successor_suite_id: successor.suite_id,
        successor_vk_digest: successor.vk_digest,
        operation,
        amount: 400,
        peer_credit_id: lifecycle.credit_id,
        recipient_encryption_key_binding: send
            .as_ref()
            .map(|send| send.request.recipient_encryption_key)
            .unwrap_or([0; 32]),
        mint_finality_proof_binding_digest: [0; 32],
        predecessor_release_id: release,
        release_id: release,
        network_id: credential.network_id,
        asset_id: credential.asset_id,
        asset_incarnation: credential.asset_incarnation,
        asset_scale: credential.asset_scale,
        liability_pool_id: credential.liability_pool_id,
        hardware_profile_id: credential.hardware_profile_id,
        policy_epoch: credential.policy_epoch,
        lane_id: credential.lane_id,
        predecessor_state_commitment: predecessor.state_commitment,
        successor_state_commitment: successor.state_commitment,
        predecessor_state_nonce_commitment: predecessor.state_nonce_commitment,
        successor_state_nonce_commitment: successor.state_nonce_commitment,
        predecessor_logical_sequence: 5,
        successor_logical_sequence: 6,
        predecessor_hardware_epoch_generation: credential.hardware_epoch_generation,
        successor_hardware_epoch_generation: credential.hardware_epoch_generation,
        predecessor_hardware_epoch_id: credential.hardware_epoch_id,
        successor_hardware_epoch_id: credential.hardware_epoch_id,
        predecessor_key_reference: credential.key_reference,
        successor_key_reference: credential.key_reference,
        predecessor_hardware_policy_id: credential.hardware_policy_id,
        successor_hardware_policy_id: credential.hardware_policy_id,
        journal_revision_before: 5,
        journal_revision_after: 6,
        lifecycle_binding_digest: lifecycle_digest,
        prepared_transition_binding_digest: prepared,
        terminal_commit_binding_digest: [0; 32],
        sender_one_time_authorization_digest: [0; 32],
        receive_credit_binding_digest: [0; 32],
        transition_intent_digest: digest(b"terminal-semantic-fixture-intent", intent_variant),
        transition_effect_digest: digest(b"terminal-semantic-fixture-effect", 0),
        recovery_record_digest: digest(b"terminal-semantic-fixture-recovery", 0),
        durable_inbox_effect_digest: credential.canonical_empty_effect_digest,
        durable_outbox_effect_digest: digest(b"terminal-semantic-fixture-outbox", 0),
    };
    let before_guard = guard_relation(&material, normalized);
    let eq_candidate_protocol = shape_protocol::<EqAffine>(RECURSIVE_PUBLIC_INSTANCE_COUNT);
    let ep_candidate_protocol = shape_protocol::<EpAffine>(RECURSIVE_PUBLIC_INSTANCE_COUNT);
    let eq_guard_protocol = shape_protocol::<EqAffine>(44);
    let ep_guard_protocol = shape_protocol::<EpAffine>(44);
    let eq_protocol =
        native_parent_protocol_digest_v1(&eq_candidate_protocol, KagemushaPastaParityV1::Eq)
            .unwrap();
    let ep_protocol =
        native_parent_protocol_digest_v1(&ep_candidate_protocol, KagemushaPastaParityV1::Ep)
            .unwrap();
    // Reuse only the typed projection fixture. Its placeholder proof is dropped without use.
    let (mut candidate, _) = crate::zk::kagemusha_v1_recursion::tests::state_verification_fixture();
    candidate.operation = operation;
    candidate.predecessor = Some(predecessor.clone());
    candidate.successor = successor.clone();
    candidate.amount = 400;
    candidate.journal_revision_before = 5;
    candidate.journal_revision_after = 6;
    candidate.transition_effect_digest = normalized.transition_effect_digest;
    candidate.lifecycle_binding_digest = lifecycle_digest;
    candidate.prepared_transition_binding_digest = prepared;
    candidate.peer_credit_id = normalized.peer_credit_id;
    candidate.recipient_encryption_key_binding = normalized.recipient_encryption_key_binding;
    candidate.transport_semantic_digest = send
        .as_ref()
        .map(|send| {
            kagemusha_payment_body_digest_from_digests_v1(
                send.output.canonical_digest_against(&send.request).unwrap(),
                send.encrypted_credit_digest,
            )
        })
        .unwrap_or_else(|| digest(b"terminal-semantic-fixture-payload", 0));
    candidate.guard_statement_digest = before_guard.statement_digest();
    candidate.eq_protocol_digest = eq_protocol;
    candidate.ep_protocol_digest = ep_protocol;
    candidate.guard_eq_protocol_digest =
        native_parent_protocol_digest_v1(&eq_guard_protocol, KagemushaPastaParityV1::Eq).unwrap();
    candidate.guard_ep_protocol_digest =
        native_parent_protocol_digest_v1(&ep_guard_protocol, KagemushaPastaParityV1::Ep).unwrap();
    let eq_semantic = candidate.public_instances::<Fp>().unwrap();
    let ep_semantic = candidate.public_instances::<Fq>().unwrap();
    let candidate_digest =
        canonical_terminal_authorization_candidate_digest_v1(&[eq_semantic.clone()]).unwrap();
    assert_eq!(
        candidate_digest,
        canonical_terminal_authorization_candidate_digest_v1(&[ep_semantic.clone()]).unwrap()
    );
    let body = KagemushaHardwareTerminalBodyV1 {
        version: 1,
        candidate_envelope_digest: candidate_digest,
        lifecycle_binding_digest: lifecycle_digest,
        transition_nullifier: nullifier,
        outbox_reservation_commitment: reservation_digest,
        commit_evidence: openings.commit_evidence().unwrap(),
        hardware_profile_id: predecessor.hardware_profile_id,
        policy_epoch: predecessor.policy_epoch,
        private_successor_commitment: successor.state_commitment,
        private_journal_commitment: digest(b"terminal-semantic-fixture-journal", 0),
        private_recovery_commitment: normalized.recovery_record_digest,
    };
    let certificate = KagemushaCommitCertificateV1 {
        version: 1,
        certificate_id: [0; 32],
        candidate_envelope_digest: candidate_digest,
        lifecycle_binding_digest: lifecycle_digest,
        transition_nullifier: nullifier,
        outbox_reservation_commitment: reservation_digest,
        commit_evidence: body.commit_evidence,
        hardware_profile_id: body.hardware_profile_id,
        policy_epoch: body.policy_epoch,
        hardware_terminal_commitment: [0; 32],
    }
    .seal_with_terminal_body(&body)
    .unwrap();
    let certificate_digest = canonical_commit_certificate_digest_v1(&certificate).unwrap();
    let terminal_output_binding = send
        .as_ref()
        .map(|send| {
            let output_digest = send.output.canonical_digest_against(&send.request).unwrap();
            let prepared_transfer = kagemusha_prepared_transfer_digest_v1(
                &send.request,
                send.output.sender_before_commitment,
                send.output.sender_after_commitment,
                nullifier,
                send.output.ciphertext_commitment,
            )
            .unwrap();
            canonical_terminal_send_output_binding_v1(
                send.output.credit_id,
                send.request.recipient_encryption_key,
                send.request.hardware_credential.lane_commitment,
                prepared_transfer,
                output_digest,
                canonical_incoming_payment_claims_binding_v1([
                    request_digest,
                    send.request.hardware_credential.credential_id,
                    canonical_sender_state_pair_digest_v1(
                        predecessor.state_commitment,
                        successor.state_commitment,
                    ),
                    output_digest,
                    send.encrypted_credit_digest,
                    candidate_digest,
                    certificate_digest,
                ]),
            )
        })
        .unwrap_or_else(|| digest(b"terminal-semantic-fixture-output", 0));
    let public = KagemushaTerminalAuthorizationPublicInputsV1::from_lifecycle(
        &lifecycle,
        candidate.transport_semantic_digest,
        candidate_digest,
        certificate_digest,
        nullifier,
        request_digest,
        send.as_ref()
            .map(|send| send.request.hardware_credential.credential_id)
            .unwrap_or([0; 32]),
        send.as_ref()
            .map(|send| send.output.ciphertext_commitment)
            .unwrap_or([0; 32]),
        400,
        terminal_output_binding,
        encode_pasta(Fp::from(3)),
        encode_pasta(Fq::from(4)),
        encode_pasta(Fp::from(1)),
        encode_pasta(Fq::from(2)),
    )
    .unwrap();
    let private = KagemushaTerminalAuthorizationPrivateTransitionV1 {
        lifecycle,
        predecessor,
        successor,
        outbox_reservation: reservation,
        commit_certificate: certificate,
        commit_evidence_opening: openings.commit_evidence_opening,
        one_use_hardware_authorization: openings.one_use_hardware_authorization,
        terminal_payload_digest: candidate.transport_semantic_digest,
        send,
        journal_revision_before: 5,
        journal_revision_after: 6,
        authorization_counter_before: 0,
        authorization_counter_after: 1,
        hardware_profile: material.hardware_profile.clone(),
        hardware_credential: material.hardware_credential.clone(),
    };
    private
        .validate_against(&public)
        .expect("canonical synthetic private transition");
    normalized.sender_one_time_authorization_digest = if private.send.is_some() {
        authorization
    } else {
        [0; 32]
    };
    normalized.terminal_commit_binding_digest = canonical_terminal_commit_binding_digest_v1(
        &public,
        &private,
        prepared,
        normalized.sender_one_time_authorization_digest,
        normalized.transition_intent_digest,
        normalized.transition_effect_digest,
        normalized.recovery_record_digest,
        normalized.durable_inbox_effect_digest,
        normalized.durable_outbox_effect_digest,
    )
    .unwrap();
    let guard = guard_relation(&material, normalized);
    let mut eq_column = eq_semantic;
    eq_column.resize(RECURSIVE_PUBLIC_INSTANCE_COUNT, Fp::ZERO);
    let mut ep_column = ep_semantic;
    ep_column.resize(RECURSIVE_PUBLIC_INSTANCE_COUNT, Fq::ZERO);
    let eq_candidates = [eq_column];
    let ep_candidates = [ep_column];
    let eq_guards = [vec![Fp::ZERO; 44]];
    let ep_guards = [vec![Fq::ZERO; 44]];
    let history = [0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1];
    let mut enabled = [[0; 32]; TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1];
    enabled[0] = public.hardware_profile_id;
    let inputs = TerminalSemanticPlanInputsV1 {
        public: &public,
        private_transition: &private,
        terminal_guard_relation: &guard,
        enabled_hardware_profiles: &enabled,
        eq: TerminalSemanticPlanParityV1 {
            candidate_protocol: &eq_candidate_protocol,
            candidate_instances: &eq_candidates,
            terminal_guard_protocol: &eq_guard_protocol,
            terminal_guard_instances: &eq_guards,
            successor_history: &history,
        },
        ep: TerminalSemanticPlanParityV1 {
            candidate_protocol: &ep_candidate_protocol,
            candidate_instances: &ep_candidates,
            terminal_guard_protocol: &ep_guard_protocol,
            terminal_guard_instances: &ep_guards,
            successor_history: &history,
        },
    };
    let plan = plan_terminal_semantic_sha_v1(inputs).expect(
        "complete shared semantic assignment equals independently retained original in both fields",
    );
    assert_eq!(plan.eq_messages.len(), 26);
    assert_eq!(plan.ep_messages.len(), 26);
    assert_eq!(plan.job_block_counts.len(), 26);
    assert!(plan.job_block_counts.iter().all(|count| *count > 0));
    if check_independence {
        // Change both u128 limbs of each later digest independently, and every limb of each
        // successor history. These outputs do not yet exist when the typed claim is proved.
        for slot in 0..6 {
            let mut changed_public = public.clone();
            let mut changed_eq_history = history;
            let mut changed_ep_history = history;
            match slot {
                0 => changed_public.eq_deferred_audit = [5; 32],
                1 => changed_public.ep_deferred_audit = [6; 32],
                2 => changed_public.eq_protocol_digest = [7; 32],
                3 => changed_public.ep_protocol_digest = [8; 32],
                4 => changed_eq_history = [9; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
                5 => changed_ep_history = [10; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
                _ => unreachable!("six later proof-output roles"),
            }
            let changed = plan_terminal_semantic_sha_v1(TerminalSemanticPlanInputsV1 {
                public: &changed_public,
                eq: TerminalSemanticPlanParityV1 {
                    successor_history: &changed_eq_history,
                    ..inputs.eq
                },
                ep: TerminalSemanticPlanParityV1 {
                    successor_history: &changed_ep_history,
                    ..inputs.ep
                },
                ..inputs
            })
            .expect("complete queue planning does not depend on later proof outputs");
            assert_eq!(
                changed, plan,
                "later proof-output slot {slot} changed a SHA job"
            );
        }

        // Detached semantic candidates and certificates cannot reuse a bound planning input.
        let mut changed_candidates = eq_candidates.clone();
        changed_candidates[0]
            [crate::zk::kagemusha_v1_recursion::state_relation::public_instance::AMOUNT] += Fp::ONE;
        assert!(
            plan_terminal_semantic_sha_v1(TerminalSemanticPlanInputsV1 {
                eq: TerminalSemanticPlanParityV1 {
                    candidate_instances: &changed_candidates,
                    ..inputs.eq
                },
                ..inputs
            })
            .is_err()
        );
        let mut changed_private = private.clone();
        changed_private.commit_certificate.candidate_envelope_digest[0] ^= 1;
        assert!(
            plan_terminal_semantic_sha_v1(TerminalSemanticPlanInputsV1 {
                private_transition: &changed_private,
                ..inputs
            })
            .is_err()
        );
    }
    for messages in [&plan.eq_messages, &plan.ep_messages] {
        assert!(
            messages
                .last()
                .unwrap()
                .starts_with(b"iroha:kagemusha:v1:terminal-authorization-candidate\0")
        );
        assert_eq!(
            DigestV1::from(Sha256::digest(messages.last().unwrap())),
            candidate_digest
        );
        if let Some(send) = &private.send {
            // Check the active branch's original canonical byte messages as well as the
            // complete assignment fingerprint. An inactive all-zero send template cannot
            // satisfy these request/output/prepared-transfer assertions.
            let mut prepared_bytes = KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes().to_vec();
            prepared_bytes.extend_from_slice(&send.output.request_digest);
            prepared_bytes.extend_from_slice(&send.output.amount.to_le_bytes());
            for digest in [
                send.output.sender_before_commitment,
                send.output.sender_after_commitment,
                send.output.transition_nullifier,
                send.request.recipient_encryption_key,
                send.output.ciphertext_commitment,
            ] {
                prepared_bytes.extend_from_slice(&digest);
            }
            assert_eq!(prepared_bytes.len(), 210);
            for (domain, bytes, expected_digest) in [
                (
                    b"iroha:kagemusha:v1:payment-request".as_slice(),
                    send.request.circuit_transcript_bytes().unwrap(),
                    send.request.canonical_digest().unwrap(),
                ),
                (
                    b"iroha:kagemusha:v1:send-split-statement".as_slice(),
                    send.output.circuit_transcript_bytes().to_vec(),
                    send.output.canonical_digest_against(&send.request).unwrap(),
                ),
                (
                    b"iroha:kagemusha:v1:prepared-transfer".as_slice(),
                    prepared_bytes,
                    kagemusha_prepared_transfer_digest_v1(
                        &send.request,
                        send.output.sender_before_commitment,
                        send.output.sender_after_commitment,
                        send.output.transition_nullifier,
                        send.output.ciphertext_commitment,
                    )
                    .unwrap(),
                ),
            ] {
                let mut expected_message = domain.to_vec();
                expected_message.push(0);
                expected_message.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
                expected_message.extend_from_slice(&bytes);
                assert_eq!(
                    messages
                        .iter()
                        .filter(|message| **message == expected_message)
                        .count(),
                    1,
                    "each active send transcript appears exactly once in the complete queue"
                );
                assert_eq!(
                    DigestV1::from(Sha256::digest(&expected_message)),
                    expected_digest
                );
            }
        }
    }
    // The zero Guard columns and histories were checked only for fixed shape. No nested proof,
    // accumulated decision, public release gate, persistent state or final payment was used.
    [plan.eq_messages, plan.ep_messages]
}
