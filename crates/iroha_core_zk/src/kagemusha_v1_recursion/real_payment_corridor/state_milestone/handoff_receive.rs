//! Original peer-credit staging and genuine receiver `ReceiveFold` for the handoff fixture.
//!
//! The sender wrapper supplies its decided current claims and exact retained history. The
//! receiver keeps its original machine, credential, replay insertion, history selection and
//! proof owner. Diagnostic signatures describe a test provider, not physical qualification.

use super::*;
use crate::kagemusha_v1_state::{
    DurableAcknowledgementV1, PeerCreditFoldInputV1, ReceiveFoldCreditV1, ReceiveFoldV1,
    StagePaymentOutcomeV1,
};
use device_owner::DiagnosticDeviceV1;
use handoff_send::DiagnosticSentPaymentV1;

/// Actual installed handoff evidence and the byte-identical ACK retained by receiver staging.
pub(super) struct ReceiveOutcomeV1 {
    pub(super) evidence: GeneratedHandoffEvidenceV1,
    pub(super) acknowledgement: DurableAcknowledgementV1,
}

fn relation_credit(original: &PeerCreditFoldInputV1) -> KagemushaReceiveFoldCreditV1 {
    KagemushaReceiveFoldCreditV1 {
        amount: original.amount,
        credit_id: original.credit_id.0,
        recipient_lane_id: original.recipient_lane_id,
        incoming_proof_binding_digest: original.incoming_proof_binding_digest,
        request_digest: original.request_digest,
        prepared_transfer_digest: original.prepared_transfer_digest,
        transition_nullifier: original.transition_nullifier,
        recipient_encryption_key: original.recipient_encryption_key,
        ciphertext_commitment: original.ciphertext_commitment,
        credit_opening: original.credit_opening,
        receiver_binding_digest: original.receiver_binding_digest,
        payment_output_digest: original.payment_output_digest,
        replay_insert: KagemushaReplayInsertWitnessV1::from(&original.replay_insert_witness),
    }
}

fn transcript_credit(original: &PeerCreditFoldInputV1) -> ReceiveFoldCreditV1 {
    ReceiveFoldCreditV1 {
        amount: original.amount,
        credit_id: original.credit_id,
        recipient_lane_id: original.recipient_lane_id,
        incoming_proof_binding_digest: original.incoming_proof_binding_digest,
        receiver_binding_digest: original.receiver_binding_digest,
        payment_output_digest: original.payment_output_digest,
        envelope_digest: original.envelope_digest,
    }
}

/// Stage the original sender payment and install one genuinely proved receiver transition.
///
/// The caller must retain the generated payment in the shared diagnostic verifier before this
/// invocation. Seed padding, a foreign wrapper, consumed credit or substituted sender proof is
/// refused. All times and nonce commitments remain explicit caller-owned original inputs.
#[allow(clippy::too_many_arguments)]
pub(super) fn receive(
    receiver: &mut DiagnosticDeviceV1<'_>,
    receiver_credit: &DiagnosticReceiverCreditV1,
    sender: &DiagnosticSentPaymentV1,
    guard_keys: &mut Option<GuardKeys>,
    successor_nonce: DigestV1,
    staged_at_ms: u64,
    folded_at_ms: u64,
    check_refusals: bool,
) -> Result<ReceiveOutcomeV1, String> {
    let request = &sender.request;
    let payment = &sender.payment;
    let sender_wrapper = &sender.wrapper;
    receiver
        .recursive_verifier
        .verify_retained_payment(request, payment, sender_wrapper)?;
    let retained_sender = sender_wrapper
        .committed
        .candidate
        .recovery_view()
        .map_err(|error| error.to_string())?;
    ensure(
        retained_sender.candidate_proof == &sender.state.proof,
        "handoff sender State proof differs from the original committed candidate",
    )?;
    let incoming = &sender_wrapper.incoming;
    ensure(
        incoming.eq_proof == payment.proof.eq_proof
            && incoming.ep_proof == payment.proof.ep_proof
            && incoming.eq_current.as_ref() == Some(&sender_wrapper.payment.eq_current_accumulator)
            && incoming.ep_current.as_ref() == Some(&sender_wrapper.payment.ep_current_accumulator),
        "receiver requires the original proved incoming wrapper, never seeded padding",
    )?;
    wrapper::require_release_pinned_incoming_identity(
        [
            receiver.context.artifacts.commit_wrapper_eq_protocol_digest,
            receiver.context.artifacts.commit_wrapper_ep_protocol_digest,
        ],
        [
            native_parent_protocol_digest_v1(&incoming.eq_protocol, KagemushaPastaParityV1::Eq)?,
            native_parent_protocol_digest_v1(&incoming.ep_protocol, KagemushaPastaParityV1::Ep)?,
        ],
    )?;
    let sender_public_inputs = sender_wrapper
        .committed
        .candidate
        .prepared
        .candidate_public_inputs(receiver.context.artifacts, &sender.state.proof)?;
    ensure(
        sender_public_inputs == sender.public,
        "sender record public projection differs from its original committed candidate",
    )?;
    if check_refusals {
        let original_snapshot = receiver
            .machine
            .snapshot()
            .map_err(|error| error.to_string())?;
        let original_state = receiver.machine.state().clone();
        let original_journal_revision = receiver.machine.journal_revision();
        let original_inbox_revision = receiver.machine.inbox_revision();
        let original_pending_count = receiver.machine.pending_credit_count();
        let original_capacity = *receiver.machine.receiver_inbox_capacity();
        let mut substituted_request = request.clone();
        substituted_request.recipient_encryption_key[0] ^= 1;
        let mut substituted_payment = payment.clone();
        let ciphertext = substituted_payment
            .encrypted_credit
            .last_mut()
            .ok_or_else(|| "original peer-credit ciphertext is empty".to_owned())?;
        *ciphertext ^= 1;
        for (candidate_request, candidate_payment) in [
            (&substituted_request, payment),
            (request, &substituted_payment),
        ] {
            assert!(matches!(
                peer_stage::stage_peer_payment(
                    &mut receiver.machine,
                    receiver_credit,
                    candidate_request,
                    candidate_payment,
                    staged_at_ms,
                    &receiver.journal_key,
                    &receiver.device_key,
                ),
                Err(KagemushaStateErrorV1::InvalidPeerCredit)
            ));
            assert_eq!(
                receiver
                    .machine
                    .snapshot()
                    .map_err(|error| error.to_string())?,
                original_snapshot,
            );
            assert_eq!(receiver.machine.state(), &original_state);
            assert_eq!(
                receiver.machine.journal_revision(),
                original_journal_revision
            );
            assert_eq!(receiver.machine.inbox_revision(), original_inbox_revision);
            assert_eq!(
                receiver.machine.pending_credit_count(),
                original_pending_count
            );
            assert_eq!(
                receiver.machine.receiver_inbox_capacity(),
                &original_capacity
            );
        }
        let mut substituted_opening = DiagnosticReceiverCreditV1 {
            private_key: Zeroizing::new(*receiver_credit.private_key),
            credit_commitment_opening: receiver_credit.credit_commitment_opening,
            recipient_binding_opening: receiver_credit.recipient_binding_opening,
            recovery_nonce: receiver_credit.recovery_nonce,
        };
        substituted_opening.recovery_nonce[0] ^= 1;
        assert!(matches!(
            peer_stage::stage_peer_payment(
                &mut receiver.machine,
                &substituted_opening,
                request,
                payment,
                staged_at_ms,
                &receiver.journal_key,
                &receiver.device_key,
            ),
            Err(KagemushaStateErrorV1::InvalidPeerCredit)
        ));
        assert_eq!(
            receiver
                .machine
                .snapshot()
                .map_err(|error| error.to_string())?,
            original_snapshot,
        );
        assert_eq!(receiver.machine.state(), &original_state);
        assert_eq!(
            receiver.machine.journal_revision(),
            original_journal_revision
        );
        assert_eq!(receiver.machine.inbox_revision(), original_inbox_revision);
        assert_eq!(
            receiver.machine.pending_credit_count(),
            original_pending_count
        );
        assert_eq!(
            receiver.machine.receiver_inbox_capacity(),
            &original_capacity
        );
    }
    let predecessor = receiver.machine.state().clone();
    let stage = peer_stage::stage_peer_payment(
        &mut receiver.machine,
        receiver_credit,
        request,
        payment,
        staged_at_ms,
        &receiver.journal_key,
        &receiver.device_key,
    )
    .map_err(|error| error.to_string())?;
    let acknowledgement = match stage {
        StagePaymentOutcomeV1::Staged {
            acknowledgement, ..
        }
        | StagePaymentOutcomeV1::DuplicatePending {
            acknowledgement, ..
        } => acknowledgement,
        StagePaymentOutcomeV1::DuplicateConsumed { .. } => {
            return Err("handoff credit was already folded by the original receiver".to_owned());
        }
    };
    ensure(
        receiver.machine.state() == &predecessor,
        "durable peer staging alone cannot increase the original receiver balance",
    )?;
    let preview = receiver
        .machine
        .preview_receive_fold(
            CreditIdV1(payment.output.credit_id),
            successor_nonce,
            folded_at_ms,
        )
        .map_err(|error| error.to_string())?;
    let expected = preview.transition.successor.clone();
    let receive_credit = transcript_credit(&preview.credit);
    let transcript = ReceiveFoldV1::try_new(receive_credit).map_err(|error| error.to_string())?;
    ensure(
        transcript.canonical_transcript_digest() == preview.receive_credit_binding_digest
            && predecessor.balance.checked_add(request.amount) == Some(expected.balance),
        "receiver preview must bind its exact staged credit and original positive amount",
    )?;
    let funded = receiver.context.funded;
    let guard = Rc::new(prove_guard(
        &funded.eq,
        &funded.ep,
        &funded.credential_keys,
        guard_keys,
        guard_relation(
            receiver.material,
            preview.transition.normalized_guard_statement,
        ),
        receiver.credential,
        receiver.credential,
    ));
    let keys = guard_keys
        .as_ref()
        .ok_or_else(|| "genuine ReceiveFold did not retain Guard keys".to_owned())?;
    ensure(
        keys.eq_protocol_digest == receiver.context.artifacts.guard_bundle_eq_protocol_digest
            && keys.ep_protocol_digest
                == receiver.context.artifacts.guard_bundle_ep_protocol_digest,
        "receiver Guard keys differ from the original artifact context",
    )?;
    let guard_bundle = receiver.guard_verifier.retain(Rc::clone(&guard));
    let relation = transition_relation_for_corridor(
        predecessor,
        &preview.transition,
        &guard,
        RecursiveStateProtocolBindings::new(
            receiver.context.keys.eq_protocol_digest,
            receiver.context.keys.ep_protocol_digest,
            keys,
            funded,
            incoming,
        ),
        None,
        Some(relation_credit(&preview.credit)),
        None,
    );
    let mut receiver_public_inputs = relation.public_inputs_v1()?;
    let parent = parent_from_generated((*receiver.current).clone());
    let next = Rc::new(prove_recursive_state_step(
        funded,
        receiver.context.keys,
        &guard,
        keys,
        &parent,
        incoming,
        relation,
        None,
    ));
    // Generation derives the actual circuit-layout audits, replacing only discovery carriers.
    receiver_public_inputs.eq_deferred_audit = next.proof.eq_deferred_audit;
    receiver_public_inputs.ep_deferred_audit = next.proof.ep_deferred_audit;
    receiver.recursive_verifier.retain_state(Rc::clone(&next));
    let authorization = TransitionAuthorizationV1::new(
        HardwareTransitionCertificateV1 {
            statement: preview.transition.hardware_statement.clone(),
            guard_bundle,
        },
        next.proof.clone(),
    );
    let root_message = receiver
        .machine
        .receive_fold_history_root_selection_signing_bytes(&preview)
        .map_err(|error| error.to_string())?;
    let authorization = receiver
        .machine
        .authorize_receive_fold_history(
            &preview,
            authorization,
            &device_public_key(&receiver.device_key),
            device_signature(&receiver.device_key, &root_message),
        )
        .map_err(|error| error.to_string())?;
    receiver
        .machine
        .receive_fold_prepared(preview, authorization)
        .map_err(|error| error.to_string())?;
    ensure(
        receiver.machine.state() == &expected,
        "actual successful ReceiveFold must equal its original Core successor",
    )?;
    if check_refusals {
        let folded_snapshot = receiver
            .machine
            .snapshot()
            .map_err(|error| error.to_string())?;
        let folded_revision = receiver.machine.journal_revision();
        let folded_inbox_revision = receiver.machine.inbox_revision();
        let folded_pending_count = receiver.machine.pending_credit_count();
        let folded_capacity = *receiver.machine.receiver_inbox_capacity();
        let duplicate = peer_stage::stage_peer_payment(
            &mut receiver.machine,
            receiver_credit,
            request,
            payment,
            staged_at_ms,
            &receiver.journal_key,
            &receiver.device_key,
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(
            duplicate,
            StagePaymentOutcomeV1::DuplicateConsumed {
                acknowledgement: acknowledgement.clone(),
            }
        );
        assert_eq!(
            receiver
                .machine
                .snapshot()
                .map_err(|error| error.to_string())?,
            folded_snapshot,
        );
        assert!(matches!(
            receiver.machine.preview_receive_fold(
                CreditIdV1(payment.output.credit_id),
                successor_nonce,
                folded_at_ms,
            ),
            Err(KagemushaStateErrorV1::CreditNotStaged(id)) if id.0 == payment.output.credit_id
        ));
        assert_eq!(
            receiver
                .machine
                .snapshot()
                .map_err(|error| error.to_string())?,
            folded_snapshot,
        );
        assert_eq!(receiver.machine.state(), &expected);
        assert_eq!(receiver.machine.journal_revision(), folded_revision);
        assert_eq!(receiver.machine.inbox_revision(), folded_inbox_revision);
        assert_eq!(
            receiver.machine.pending_credit_count(),
            folded_pending_count
        );
        assert_eq!(receiver.machine.receiver_inbox_capacity(), &folded_capacity);
    }
    // Never advance the device proof from a preview, rejected authorization or failed install.
    receiver.current = Rc::clone(&next);
    Ok(ReceiveOutcomeV1 {
        evidence: GeneratedHandoffEvidenceV1 {
            sender_public_inputs,
            sender_state_proof: sender.state.proof.clone(),
            payment_request: request.clone(),
            payment: payment.clone(),
            receive_credit,
            receiver_public_inputs,
            receiver_state_proof: next.proof.clone(),
        },
        acknowledgement,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_v1_state::ConsumedCreditInsertWitnessV1;

    #[test]
    fn receive_credit_projections_retain_every_original_opening_and_replay_field() {
        // A structural mapping fixture only. Zero roots are never admitted as machine/proof state.
        let original = PeerCreditFoldInputV1 {
            amount: 37,
            credit_id: CreditIdV1([1; 32]),
            recipient_lane_id: [2; 32],
            incoming_proof_binding_digest: [3; 32],
            request_digest: [4; 32],
            prepared_transfer_digest: [5; 32],
            transition_nullifier: [6; 32],
            recipient_encryption_key: [7; 32],
            ciphertext_commitment: [8; 32],
            credit_opening: KagemushaCreditOpeningV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                credit_id: [1; 32],
                amount: 37,
                credit_commitment_opening: [9; 32],
                recipient_binding_opening: [10; 32],
                recovery_nonce: [11; 32],
            },
            receiver_binding_digest: [12; 32],
            payment_output_digest: [13; 32],
            envelope_digest: [14; 32],
            replay_insert_witness: ConsumedCreditInsertWitnessV1 {
                credit_id: CreditIdV1([1; 32]),
                envelope_digest: [14; 32],
                predecessor_root: KagemushaPastaStateCommitmentV1::ZERO,
                successor_root: KagemushaPastaStateCommitmentV1::ZERO,
                siblings_root_to_leaf: [KagemushaPastaStateCommitmentV1::ZERO; 256],
            },
        };
        let relation = relation_credit(&original);
        assert_eq!(relation.amount, original.amount);
        assert_eq!(relation.credit_id, original.credit_id.0);
        assert_eq!(relation.recipient_lane_id, original.recipient_lane_id);
        assert_eq!(
            relation.incoming_proof_binding_digest,
            original.incoming_proof_binding_digest
        );
        assert_eq!(relation.request_digest, original.request_digest);
        assert_eq!(
            relation.prepared_transfer_digest,
            original.prepared_transfer_digest
        );
        assert_eq!(relation.transition_nullifier, original.transition_nullifier);
        assert_eq!(
            relation.recipient_encryption_key,
            original.recipient_encryption_key
        );
        assert_eq!(
            relation.ciphertext_commitment,
            original.ciphertext_commitment
        );
        assert_eq!(relation.credit_opening, original.credit_opening);
        assert_eq!(
            relation.receiver_binding_digest,
            original.receiver_binding_digest
        );
        assert_eq!(
            relation.payment_output_digest,
            original.payment_output_digest
        );
        assert_eq!(
            relation.replay_insert,
            KagemushaReplayInsertWitnessV1::from(&original.replay_insert_witness)
        );
        assert_eq!(
            transcript_credit(&original),
            ReceiveFoldCreditV1 {
                amount: original.amount,
                credit_id: original.credit_id,
                recipient_lane_id: original.recipient_lane_id,
                incoming_proof_binding_digest: original.incoming_proof_binding_digest,
                receiver_binding_digest: original.receiver_binding_digest,
                payment_output_digest: original.payment_output_digest,
                envelope_digest: original.envelope_digest,
            }
        );
        let mut substituted = original.clone();
        substituted.credit_opening.recovery_nonce[0] ^= 1;
        assert_ne!(relation_credit(&substituted), relation);
        let mut substituted = original.clone();
        substituted.replay_insert_witness.envelope_digest[0] ^= 1;
        assert_ne!(relation_credit(&substituted), relation);
        let mut substituted = original.clone();
        substituted.incoming_proof_binding_digest[0] ^= 1;
        assert_ne!(relation_credit(&substituted), relation);
        assert_ne!(
            transcript_credit(&substituted),
            transcript_credit(&original)
        );
        assert_ne!(
            ReceiveFoldV1::try_new(transcript_credit(&substituted))
                .unwrap()
                .canonical_transcript_digest(),
            ReceiveFoldV1::try_new(transcript_credit(&original))
                .unwrap()
                .canonical_transcript_digest(),
        );
    }
}
