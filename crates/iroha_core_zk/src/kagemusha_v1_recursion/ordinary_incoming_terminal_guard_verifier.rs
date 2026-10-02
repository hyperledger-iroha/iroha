//! Genuine incoming purpose1 Guard admitted under the distinct immutable Native W1 loan.
//! This result has no mutable State/effect authority; complete independent Commit admission and
//! authentic global CAS acknowledgment remain mandatory before funding/credit consumption.
use super::*;
use crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1;
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalPurposeV1, KagemushaOrdinaryCashClockContextV1,
    kagemusha_ordinary_terminal_guard_commit_binding_digest_v1,
};

pub(crate) struct KagemushaAuthenticatedOrdinaryIncomingTerminalGuardV1 {
    operation_id: DigestV1,
    nonce: DigestV1,
    admission_clock: KagemushaOrdinaryCashClockContextV1,
    body_digest: DigestV1,
    candidate_digest: DigestV1,
    digests: [DigestV1; 5],
    eq_history: History,
    ep_history: History,
    original: Vec<u8>,
}
impl KagemushaAuthenticatedOrdinaryIncomingTerminalGuardV1 {
    pub(crate) fn original(&self) -> &[u8] {
        &self.original
    }
    pub(crate) fn original_digests(&self) -> [DigestV1; 5] {
        self.digests
    }
    pub(crate) fn eq_history(&self) -> &History {
        &self.eq_history
    }
    pub(crate) fn ep_history(&self) -> &History {
        &self.ep_history
    }
    pub(crate) fn recheck_terminal_selection(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1<'_>,
    ) -> Result<()> {
        selection.recheck_selected_originals_and_current_custody()?;
        let expected = incoming_terminal_digests(selection)?;
        if self.operation_id != selection.challenge()?.operation_id
            || self.nonce != selection.challenge()?.nonce
            || self.admission_clock != *selection.admission_clock_context()?
            || self.body_digest
                != selection
                    .terminal_body()?
                    .binding_digest()
                    .map_err(integrity)?
            || self.candidate_digest != selection.candidate()?.candidate_original_sha256()
            || self.digests != expected
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let w2 = selection.preparation_selection()?;
        let material = w2.recursive_verifier().ordinary_guard_verifier_material();
        let wire = decode_exact(&self.original, &material)?;
        require_wire_digests(&wire, expected)?;
        if wire.eq_history != self.eq_history || wire.ep_history != self.ep_history {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        verify_wire(&wire, &material)?;
        selection.recheck_selected_originals_and_current_custody()
    }
}
pub(crate) fn verify_ordinary_incoming_terminal_guard_v1(
    selection: &KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1<'_>,
    paired_guard: &[u8],
) -> Result<KagemushaAuthenticatedOrdinaryIncomingTerminalGuardV1> {
    selection.recheck_selected_originals_and_current_custody()?;
    let digests = incoming_terminal_digests(selection)?;
    let w2 = selection.preparation_selection()?;
    let material = w2.recursive_verifier().ordinary_guard_verifier_material();
    let wire = decode_exact(paired_guard, &material)?;
    require_wire_digests(&wire, digests)?;
    verify_wire(&wire, &material)?;
    selection.recheck_selected_originals_and_current_custody()?;
    Ok(KagemushaAuthenticatedOrdinaryIncomingTerminalGuardV1 {
        operation_id: selection.challenge()?.operation_id,
        nonce: selection.challenge()?.nonce,
        admission_clock: *selection.admission_clock_context()?,
        body_digest: selection
            .terminal_body()?
            .binding_digest()
            .map_err(integrity)?,
        candidate_digest: selection.candidate()?.candidate_original_sha256(),
        digests,
        eq_history: wire.eq_history,
        ep_history: wire.ep_history,
        original: paired_guard.to_vec(),
    })
}
pub(in crate::kagemusha_v1_recursion) fn incoming_terminal_digests(
    selection: &KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1<'_>,
) -> Result<[DigestV1; 5]> {
    selection.recheck_selected_originals_and_current_custody()?;
    let w2 = selection.preparation_selection()?;
    let candidate = selection.candidate()?;
    let guard = selection.preparation_guard()?;
    candidate.recheck_incoming_selection(&w2, guard)?;
    guard.recheck_incoming_selection(&w2)?;
    let release = w2.authenticated_release()?;
    if release.purpose() != KagemushaReleasePurposeV1::Production {
        return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
    }
    let body = selection.terminal_body()?;
    let i = selection.terminal_intent()?;
    i.validate_shape().map_err(integrity)?;
    let body_digest = body.binding_digest().map_err(integrity)?;
    let intent_digest = i.binding_digest().map_err(integrity)?;
    let reservation = w2.reservation()?;
    let statement = w2.transition_statement()?;
    let prep = w2.preparation()?;
    let before = w2.selected_predecessor_state()?;
    let after = w2.selected_successor_state()?;
    let c = w2.credential()?;
    let challenge = selection.challenge()?;
    let mut expected_normalized = *w2.normalized_guard_statement()?;
    expected_normalized.terminal_commit_binding_digest =
        kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
            body_digest,
            candidate.candidate_original_sha256(),
            sha(candidate.public_state_original()),
            reservation.digest().map_err(integrity)?,
        )
        .map_err(integrity)?;
    expected_normalized.sender_one_time_authorization_digest = [0; 32];
    expected_normalized.transition_intent_digest = body_digest;
    expected_normalized.recovery_record_digest = intent_digest;
    let mut expected_subject = w2.challenge()?.subject;
    expected_subject.candidate_envelope_digest = candidate.candidate_original_sha256();
    expected_subject.terminal_body_commitment = body_digest;
    let capture = selection.admission_clock_context()?;
    capture
        .validate_within_original_window(i.issued_at_ms, i.expires_at_ms)
        .map_err(integrity)?;
    let canonical_transition = norito::encode_canonical(statement).map_err(integrity)?;
    let fi = selection.financial_control_original()?;
    let reserve = selection.reserve_receipt_original()?;
    let expected_kind = match statement.kind {
        crate::kagemusha_v1_state::KagemushaTransitionKindV1::MintFold => 1,
        crate::kagemusha_v1_state::KagemushaTransitionKindV1::ReceiveFold => 3,
        _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
    };
    if body.intent != *i
        || i.operation != expected_kind
        || selection.normalized_guard_statement()? != &expected_normalized
        || challenge.subject != expected_subject
        || challenge.purpose != KagemushaAppOperationApprovalPurposeV1::MonetaryTransition
        || challenge.operation_id != i.native_operation_id
        || challenge.nonce != i.native_nonce
        || challenge.operation_id == w2.challenge()?.operation_id
        || challenge.nonce == w2.challenge()?.nonce
        || challenge.enrollment_digest != c.digest()
        || challenge.account_binding != c.subject().account_binding
        || challenge.authority_policy_digest != c.subject().app_authority_policy_digest
        || challenge.attested_key_id != c.subject().attested_key_id
        || challenge.normalized_guard_digest
            != expected_normalized.canonical_digest().map_err(integrity)?
        || challenge.issued_at_ms != i.issued_at_ms
        || challenge.expires_at_ms != i.expires_at_ms
        || i.preparation_digest != prep.binding_digest().map_err(integrity)?
        || i.reservation_digest != reservation.digest().map_err(integrity)?
        || i.finalized_source_original_sha256 != reservation.finalized_source_original_sha256
        || i.source_proof_original_sha256 != reservation.source_proof_original_sha256
        || i.state_original_sha256 != sha(candidate.public_state_original())
        || i.candidate_original_sha256 != candidate.candidate_original_sha256()
        || i.transition_statement_original_sha256 != sha(&canonical_transition)
        || i.preparation_guard_original_sha256 != sha(guard.original())
        || i.purpose2_approval_original_sha256 != sha(w2.original()?)
        || i.financial_control_original_sha256 != sha(&fi)
        || fi == w2.financial_control_original()?
        || i.reserve_receipt_original_sha256 != sha(&reserve)
        || i.clock_context.request_nonce == w2.preparation_clock_context()?.request_nonce
        || i.clock_context.lower_at_ms < w2.preparation_clock_context()?.lower_at_ms
        || i.clock_context.upper_at_ms < w2.preparation_clock_context()?.upper_at_ms
        || capture.lower_at_ms < i.clock_context.lower_at_ms
        || capture.upper_at_ms < i.clock_context.upper_at_ms
        || i.financial_index_before != before.secure_index
        || i.financial_index_after != after.secure_index
        || i.financial_sequence_before != before.logical_sequence
        || i.financial_sequence_after != after.logical_sequence
        || u128::from(i.logical_journal_sequence_before) != statement.journal_revision_before
        || u128::from(i.logical_journal_sequence_after) != statement.journal_revision_after
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let subject_digest: DigestV1 = sha(&challenge
        .canonical_subject_signing_bytes()
        .map_err(integrity)?);
    if challenge.subject_signing_digest != subject_digest {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let expected = [
        challenge.normalized_guard_digest,
        c.digest(),
        selection.authorization_binding_digest()?,
        subject_digest,
        release.provider_policy_root(),
    ];
    if expected.contains(&[0; 32]) {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(expected)
}
fn require_wire_digests(w: &OrdinaryGuardProofWireV1, e: [DigestV1; 5]) -> Result<()> {
    if [
        w.normalized_guard_digest,
        w.credential_digest,
        w.authorization_transcript_digest,
        w.subject_signing_digest,
        w.provider_policy_root,
    ] != e
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}
fn integrity(_: impl core::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::SnapshotIntegrity
}

fn sha(raw: &[u8]) -> DigestV1 {
    Sha256::digest(raw).into()
}
