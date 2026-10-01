//! Alternating-device payments through the original proved monetary and journal owners.
//!
//! Local storage is provisioned explicitly for this run; the canonical protocol capacities,
//! compact proof bounds, original receipt checks and release identity remain enforced.

use super::device_owner::{DiagnosticDeviceProofContextV1, DiagnosticDeviceV1};
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    funded: &RealFundedPrerequisite,
    state_keys: &StateKeys,
    guard_keys: &mut Option<GuardKeys>,
    terminal_keys: &mut Option<Rc<terminal::DiagnosticTerminalKeysV1>>,
    wrapper_keys: &mut Option<Rc<wrapper::DiagnosticWrapperKeysV1>>,
    artifacts: KagemushaRecursionArtifactsV1,
    incoming: &IncomingStateProofMaterial,
    count: usize,
) -> Result<KagemushaHandoffSequenceVerificationV1, String> {
    ensure(
        matches!(count, 1 | 1_024),
        "fixture requires one original handoff or all1024",
    )?;
    let material = core_bound_mint_recipient_material(
        1,
        artifacts.release_id,
        funded
            .material
            .authorization_relation
            .statement
            .context
            .vk_digest,
        artifacts.artifact_manifest_digest,
        funded.mint_credit.statement.amount,
    );
    let credential = funded.credential_keys.prove(
        &funded.eq,
        &funded.ep,
        &funded.hash_eq,
        &funded.hash_ep,
        material.platform_credential.clone(),
    );
    let context = DiagnosticDeviceProofContextV1 {
        funded,
        keys: state_keys,
        artifacts,
        incoming,
    };
    // Graph convergence already proved the same device-zero Bootstrap statement. Each new
    // journal owner needs independent proof custody: reusing that map would replace a retained
    // original proof with a freshly randomized proof having the same semantic digest.
    let verifier = DiagnosticVerifier {
        funded,
        keys: state_keys,
        artifacts,
        states: Rc::new(RefCell::new(BTreeMap::new())),
        payments: Rc::new(RefCell::new(BTreeMap::new())),
    };
    // Provision physical bytes for exact original retained receipts, not a protocol ancestry
    // limit. Every Core insertion still meters its actual canonical bytes and refuses overflow.
    let count_bytes = u64::try_from(count).map_err(|_| "fixture count overflow")?;
    let outbox_bytes = count_bytes
        .checked_mul(KagemushaDurableCapacityV1::MINIMUM_OUTBOX_BYTES)
        .and_then(|bytes| bytes.checked_add(32 * 1024 * 1024))
        .ok_or("fixture outbox overflow")?;
    let inbox_bytes = count_bytes
        .checked_mul(KagemushaDurableCapacityV1::MINIMUM_INBOX_BYTES)
        .and_then(|bytes| bytes.checked_add(32 * 1024 * 1024))
        .ok_or("fixture inbox overflow")?;
    let history_bytes = count_bytes
        .checked_mul(1024 * 1024)
        .and_then(|bytes| bytes.checked_add(8 * 1024 * 1024))
        .ok_or("fixture history overflow")?;
    let capacity = KagemushaDurableCapacityV1 {
        inbox_bytes,
        outbox_bytes,
    };
    let mut first = DiagnosticDeviceV1::bootstrap(
        &context,
        0,
        &funded.material,
        &funded.credential,
        guard_keys,
        verifier.clone(),
        capacity,
        history_bytes,
    )?;
    let mut second = DiagnosticDeviceV1::bootstrap(
        &context,
        1,
        &material,
        &credential,
        guard_keys,
        verifier.clone(),
        capacity,
        history_bytes,
    )?;
    first.apply_funded_mint(
        guard_keys,
        digest(b"handoff-original-mint-successor", 0),
        MINT_TIME,
    )?;
    ensure(
        first.machine.state().balance == 1_000 && second.machine.state().balance == 0,
        "both original admitted balances must follow sole finalized MintFold",
    )?;
    let receiver_credits = [
        DiagnosticReceiverCreditV1::for_device(0),
        DiagnosticReceiverCreditV1::for_device(1),
    ];
    let mut authorization_counters = [0_u128; 2];
    let mut generated = Vec::with_capacity(count);
    for index in 0..count {
        let sender_index = index % 2;
        let receiver_index = 1 - sender_index;
        let (sender, receiver) = if sender_index == 0 {
            (&mut first, &mut second)
        } else {
            (&mut second, &mut first)
        };
        let original_total = sender
            .machine
            .state()
            .balance
            .checked_add(receiver.machine.state().balance)
            .ok_or("balance overflow")?;
        ensure(
            original_total == 1_000,
            "pre-handoff conserved balance changed",
        )?;
        let original_sender = sender.machine.state().clone();
        let original_receiver = receiver.machine.state().clone();
        let index = u64::try_from(index).map_err(|_| "handoff index overflow")?;
        let sent = handoff_send::send(
            sender,
            receiver.material,
            &receiver_credits[receiver_index],
            &receiver.device_key,
            guard_keys,
            terminal_keys,
            wrapper_keys,
            authorization_counters[sender_index],
            index,
            400,
            index == 0,
        )?;
        authorization_counters[sender_index] = authorization_counters[sender_index]
            .checked_add(1)
            .ok_or("original authorization counter overflow")?;
        let staged_at_ms = sent
            .payment
            .output
            .committed_at_ms
            .checked_add(1)
            .ok_or("stage time overflow")?;
        let folded_at_ms = staged_at_ms.checked_add(1).ok_or("fold time overflow")?;
        let received = handoff_receive::receive(
            receiver,
            &receiver_credits[receiver_index],
            &sent,
            guard_keys,
            digest(b"handoff-original-receiver-successor", index),
            staged_at_ms,
            folded_at_ms,
            index == 0,
        )?;
        ensure(
            sender.machine.state().balance == original_sender.balance - 400
                && receiver.machine.state().balance == original_receiver.balance + 400
                && sender.machine.state().balance + receiver.machine.state().balance
                    == original_total,
            "actual SendSplit and ReceiveFold did not conserve the positive payment",
        )?;
        handoff_release::release(
            sender,
            digest(b"handoff-original-operation", index),
            &sent,
            &received.acknowledgement,
            index == 0,
        )?;
        generated.push(received.evidence);
        // Keep the complete original proof evidence; final acceptance always goes through the
        // production sequence verifier and cannot be inferred from progress or model arithmetic.
        eprintln!(
            "KAGEMUSHA genuine payment handoff {}/{} installed",
            index + 1,
            count
        );
    }
    let verified = if count == 1_024 {
        verify_real_handoff_qualification_v1(&verifier, artifacts, &generated)
    } else {
        let evidence = generated
            .iter()
            .map(GeneratedHandoffEvidenceV1::evidence)
            .collect::<Vec<_>>();
        verify_kagemusha_handoff_evidence_sequence_v1(&verifier, artifacts, &evidence)
            .map_err(|error| error.to_string())?
    };
    ensure(
        verified.verified_handoffs == count,
        "production sequence omitted a real handoff",
    )?;
    Ok(verified)
}
