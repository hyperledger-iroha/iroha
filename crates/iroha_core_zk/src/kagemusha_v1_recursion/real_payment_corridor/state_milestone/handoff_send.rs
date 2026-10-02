//! Genuine SendSplit, indexed journal, terminal and finalized payment ownership.
//!
//! The private fixture uses signed simulated provider inputs. Every State/Guard/Terminal/
//! CommitWrapper proof is real and verified, but this does not qualify physical hardware.

use super::device_owner::DiagnosticDeviceV1;
use super::*;

pub(super) struct DiagnosticSentPaymentV1 {
    pub(super) state: Rc<KagemushaGeneratedRecursiveStateProofV1>,
    pub(super) public: KagemushaStateRelationPublicInputsV1,
    pub(super) wrapper: Rc<wrapper::ProvenSenderWrapperV1>,
    pub(super) request: KagemushaPaymentRequestV1,
    pub(super) payment: KagemushaPaymentV1,
}

/// Install only the original proved candidate and its exact committed payment.
#[allow(clippy::too_many_arguments)]
pub(super) fn send(
    sender: &mut DiagnosticDeviceV1<'_>,
    receiver_material: &MintRecipientMaterial,
    receiver_credit: &DiagnosticReceiverCreditV1,
    receiver_key: &SigningKey,
    guard_keys: &mut Option<GuardKeys>,
    terminal_keys: &mut Option<Rc<terminal::DiagnosticTerminalKeysV1>>,
    wrapper_keys: &mut Option<Rc<wrapper::DiagnosticWrapperKeysV1>>,
    authorization_counter_before: u128,
    handoff_index: u64,
    amount: u128,
    check_refusals: bool,
) -> Result<DiagnosticSentPaymentV1, String> {
    let context = sender.context;
    let funded = context.funded;
    let (mut preparation, openings) = handoff_inputs::preparation(
        sender.machine.state(),
        sender.machine.journal_revision(),
        authorization_counter_before,
        receiver_material,
        receiver_credit,
        receiver_key,
        handoff_index,
        amount,
    )?;
    let provisional = sender
        .machine
        .prepare_send_split(preparation.clone())
        .map_err(|error| error.to_string())?;
    let output = *provisional
        .send_output()
        .ok_or("original Send output missing")?;
    preparation.encrypted_credit = receiver_credit.seal_for(&output, &preparation.request);
    let candidate = sender
        .machine
        .prepare_send_split(preparation.clone())
        .map_err(|error| error.to_string())?;
    ensure(
        candidate.send_output() == Some(&output),
        "receiver sealing changed original output",
    )?;
    let preview = sender
        .machine
        .diagnostic_send_split_preview(
            &candidate,
            openings.commit_evidence_opening.trusted_commit_time_ms,
        )
        .map_err(|error| error.to_string())?;
    ensure(
        candidate.predecessor_state == openings.predecessor
            && preview.proof_statement.journal_revision_before == openings.journal_revision_before
            && preview.proof_statement.journal_revision_after == openings.journal_revision_after,
        "original sender state or journal changed during preparation",
    )?;
    let operation_id = digest(b"handoff-original-operation", handoff_index);
    let credential_id = sender
        .machine
        .accepted_credential_floor()
        .original_digest()
        .map_err(|error| error.to_string())?;
    let core_key = sender
        .machine
        .enrollment_binding()
        .core_authorization_key_reference;
    let (_, _, capability) = sender
        .machine
        .prepare_indexed_outgoing_candidate(
            operation_id,
            credential_id,
            core_key,
            candidate.clone(),
        )
        .map_err(|error| error.to_string())?;
    if check_refusals {
        let before = sender
            .machine
            .snapshot()
            .map_err(|error| error.to_string())?;
        // A predecessor proof cannot replace the candidate.
        assert!(
            sender
                .machine
                .diagnostic_persist_outgoing_send_candidate(
                    &capability,
                    sender.current.proof.clone(),
                )
                .is_err()
        );
        assert_eq!(
            sender
                .machine
                .snapshot()
                .map_err(|error| error.to_string())?,
            before
        );
    }
    let guard = Rc::new(prove_guard(
        &funded.eq,
        &funded.ep,
        &funded.credential_keys,
        guard_keys,
        guard_relation(sender.material, preview.normalized_guard_statement),
        sender.credential,
        sender.credential,
    ));
    let keys = guard_keys.as_ref().ok_or("original Guard keys missing")?;
    let frame = sender.guard_verifier.retain(Rc::clone(&guard));
    sender
        .guard_verifier
        .verify_transition(
            &preview.hardware_statement,
            &preview.proof_statement,
            &preview.normalized_guard_statement,
            &frame,
        )
        .map_err(|error| error.to_string())?;
    let mut relation = transition_relation_for_corridor(
        sender.machine.state().clone(),
        &preview,
        &guard,
        RecursiveStateProtocolBindings::new(
            context.keys.eq_protocol_digest,
            context.keys.ep_protocol_digest,
            keys,
            funded,
            context.incoming,
        ),
        None,
        None,
        Some(candidate.prepared_intent_commitments()),
    );
    relation.transport_semantic_digest = candidate
        .semantic_digest()
        .map_err(|error| error.to_string())?;
    let parent = parent_from_generated((*sender.current).clone());
    let state = Rc::new(prove_recursive_state_step(
        funded,
        context.keys,
        &guard,
        keys,
        &parent,
        context.incoming,
        relation,
        None,
    ));
    sender.recursive_verifier.retain_state(Rc::clone(&state));
    let public = candidate
        .candidate_public_inputs(context.artifacts, &state.proof)
        .map_err(|error| error.to_string())?;
    crate::kagemusha_v1_recursion::verify_kagemusha_state_proof_v1(
        &sender.recursive_verifier,
        context.artifacts,
        &public,
        &state.proof,
    )
    .map_err(|error| error.to_string())?;
    if check_refusals {
        let before = sender
            .machine
            .snapshot()
            .map_err(|error| error.to_string())?;
        for parity in [KagemushaPastaParityV1::Eq, KagemushaPastaParityV1::Ep] {
            let mut changed = state.proof.clone();
            match parity {
                KagemushaPastaParityV1::Eq => changed.eq_proof[0] ^= 1,
                KagemushaPastaParityV1::Ep => changed.ep_proof[0] ^= 1,
            }
            assert!(
                sender
                    .machine
                    .diagnostic_persist_outgoing_send_candidate(&capability, changed,)
                    .is_err()
            );
            assert_eq!(
                sender
                    .machine
                    .snapshot()
                    .map_err(|error| error.to_string())?,
                before
            );
        }
    }
    let persisted = sender
        .machine
        .diagnostic_persist_outgoing_send_candidate(&capability, state.proof.clone())
        .map_err(|error| error.to_string())?;
    ensure(
        sender.machine.state() == &openings.predecessor,
        "persisting a proof cannot consume the monetary predecessor",
    )?;
    let terminal = terminal::prove_outgoing_terminal(
        funded,
        sender.material,
        sender.credential,
        terminal_keys,
        context.keys,
        guard_keys,
        &sender.recursive_verifier,
        context.artifacts,
        candidate,
        &state,
        &guard,
        terminal::DiagnosticTerminalPreparationV1::Send(&preparation),
        &openings,
    );
    ensure(
        terminal.committed.candidate == persisted,
        "terminal changed the original installed candidate",
    )?;
    let wrapped = Rc::new(wrapper::prove_sender_wrapper(
        funded,
        wrapper_keys,
        context.artifacts,
        context.incoming,
        terminal,
    ));
    wrapper::require_release_pinned_incoming_identity(
        [
            context.artifacts.commit_wrapper_eq_protocol_digest,
            context.artifacts.commit_wrapper_ep_protocol_digest,
        ],
        [
            wrapped.payment.proof.eq_protocol_digest,
            wrapped.payment.proof.ep_protocol_digest,
        ],
    )?;
    let request = preparation.request;
    let payment = KagemushaPaymentV1 {
        version: output.version,
        output,
        encrypted_credit: preparation.encrypted_credit,
        commit_certificate: wrapped.committed.commit_certificate.clone(),
        proof: wrapped.payment.proof.clone(),
    };
    payment
        .validate_shape_against(&request)
        .map_err(|error| error.to_string())?;
    if check_refusals {
        let before = sender
            .machine
            .snapshot()
            .map_err(|error| error.to_string())?;
        assert!(
            sender
                .machine
                .diagnostic_finalize_outgoing_payment(&request, payment.clone(), Vec::new(),)
                .is_err()
        );
        let mut changed = payment.commit_certificate.clone();
        changed.candidate_envelope_digest[0] ^= 1;
        let changed = changed
            .seal_certificate_id()
            .map_err(|error| error.to_string())?;
        let wrong = sender
            .machine
            .recover_indexed_outgoing_commit_capability(operation_id)
            .map_err(|error| error.to_string())?;
        assert!(
            sender
                .machine
                .commit_outgoing_candidate(wrong, changed)
                .is_err()
        );
        assert_eq!(
            sender
                .machine
                .snapshot()
                .map_err(|error| error.to_string())?,
            before
        );
    }
    let committed = sender
        .machine
        .commit_outgoing_candidate(capability, payment.commit_certificate.clone())
        .map_err(|error| error.to_string())?;
    ensure(
        committed == wrapped.committed && sender.machine.state() == &preview.successor,
        "actual committed successor differs from original proved candidate",
    )?;
    sender.current = Rc::clone(&state);
    if check_refusals {
        let before = sender
            .machine
            .snapshot()
            .map_err(|error| error.to_string())?;
        assert!(
            sender
                .machine
                .diagnostic_finalize_outgoing_payment(&request, payment.clone(), Vec::new(),)
                .is_err(),
            "a wrapper must be retained and independently decided before finalization"
        );
        assert_eq!(
            sender
                .machine
                .snapshot()
                .map_err(|error| error.to_string())?,
            before
        );
    }
    sender
        .recursive_verifier
        .retain_payment(&request, &payment, Rc::clone(&wrapped))?;
    if check_refusals {
        let before = sender
            .machine
            .snapshot()
            .map_err(|error| error.to_string())?;
        let mut changed = request.clone();
        changed.amount += 1;
        assert!(
            sender
                .machine
                .diagnostic_finalize_outgoing_payment(&changed, payment.clone(), Vec::new(),)
                .is_err()
        );
        for parity in [KagemushaPastaParityV1::Eq, KagemushaPastaParityV1::Ep] {
            let mut changed = payment.clone();
            match parity {
                KagemushaPastaParityV1::Eq => changed.proof.eq_proof[0] ^= 1,
                KagemushaPastaParityV1::Ep => changed.proof.ep_proof[0] ^= 1,
            }
            assert!(
                sender
                    .machine
                    .diagnostic_finalize_outgoing_payment(&request, changed, Vec::new(),)
                    .is_err()
            );
            assert_eq!(
                sender
                    .machine
                    .snapshot()
                    .map_err(|error| error.to_string())?,
                before
            );
        }
    }
    let finalized = sender
        .machine
        .diagnostic_finalize_outgoing_payment(&request, payment.clone(), Vec::new())
        .map_err(|error| error.to_string())?;
    ensure(
        finalized.canonical_envelope_bytes
            == norito::encode_canonical(&payment).map_err(|error| error.to_string())?,
        "finalized original payment bytes changed",
    )?;
    Ok(DiagnosticSentPaymentV1 {
        state,
        public,
        wrapper: wrapped,
        request,
        payment,
    })
}
