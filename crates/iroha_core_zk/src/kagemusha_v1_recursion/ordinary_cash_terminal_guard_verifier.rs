//! Purpose1 ordinary Guard admission under an actual captured Native terminal loan.
//!
//! Purpose2 Guard and candidate proofs remain separately verified. This closed proof result
//! authorizes no State mutation or outbox publication until the complete ordinary Terminal and
//! CommitWrapper proofs and their whole histories have also been verified against this selection.

use super::*;
use crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1;
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalChallengeV1, KagemushaAppOperationApprovalPurposeV1,
    KagemushaOperationKindV1, KagemushaOrdinaryCashClockContextV1,
    kagemusha_ordinary_terminal_guard_commit_binding_digest_v1,
};

/// Real paired purpose1 Guard admitted under one immutable Native capture.
/// Only genuine released proof verification and the actual closed terminal selection construct it.
pub(crate) struct KagemushaAuthenticatedOrdinaryTerminalGuardV1 {
    operation_id: DigestV1,
    nonce: DigestV1,
    admission_clock: KagemushaOrdinaryCashClockContextV1,
    terminal_record_digest: DigestV1,
    candidate_digest: DigestV1,
    digests: [DigestV1; 5],
    eq_history: History,
    ep_history: History,
    original: Vec<u8>,
}
impl KagemushaAuthenticatedOrdinaryTerminalGuardV1 {
    pub(crate) fn operation_id(&self) -> DigestV1 {
        self.operation_id
    }
    pub(crate) fn nonce(&self) -> DigestV1 {
        self.nonce
    }
    pub(crate) fn admission_clock_context(&self) -> &KagemushaOrdinaryCashClockContextV1 {
        &self.admission_clock
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
    pub(crate) fn original(&self) -> &[u8] {
        &self.original
    }
    pub(crate) fn recheck_terminal_selection(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
    ) -> Result<()> {
        selection.recheck_selected_originals_and_current_custody()?;
        let expected = terminal_digests(selection)?;
        if self.operation_id != selection.challenge().operation_id
            || self.nonce != selection.challenge().nonce
            || self.admission_clock != *selection.admission_clock_context()
            || self.terminal_record_digest
                != selection
                    .terminal_record()
                    .binding_digest()
                    .map_err(integrity)?
            || self.candidate_digest != selection.candidate().candidate_envelope_digest()
            || self.digests != expected
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let material = selection
            .recursive_verifier()
            .ordinary_guard_verifier_material();
        let wire = decode_exact(&self.original, &material)?;
        require_wire_digests(&wire, expected)?;
        if wire.eq_history != self.eq_history || wire.ep_history != self.ep_history {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        verify_wire(&wire, &material)?;
        selection.recheck_selected_originals_and_current_custody()
    }
}
pub(crate) fn verify_ordinary_terminal_guard_v1(
    selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
    paired_guard: &[u8],
) -> Result<KagemushaAuthenticatedOrdinaryTerminalGuardV1> {
    selection.recheck_selected_originals_and_current_custody()?;
    let expected = terminal_digests(selection)?;
    let material = selection
        .recursive_verifier()
        .ordinary_guard_verifier_material();
    let wire = decode_exact(paired_guard, &material)?;
    require_wire_digests(&wire, expected)?;
    verify_wire(&wire, &material)?;
    selection.recheck_selected_originals_and_current_custody()?;
    Ok(KagemushaAuthenticatedOrdinaryTerminalGuardV1 {
        operation_id: selection.challenge().operation_id,
        nonce: selection.challenge().nonce,
        admission_clock: *selection.admission_clock_context(),
        terminal_record_digest: selection
            .terminal_record()
            .binding_digest()
            .map_err(integrity)?,
        candidate_digest: selection.candidate().candidate_envelope_digest(),
        digests: expected,
        eq_history: wire.eq_history,
        ep_history: wire.ep_history,
        original: paired_guard.to_vec(),
    })
}

pub(in crate::kagemusha_v1_recursion) fn terminal_digests(
    selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
) -> Result<[DigestV1; 5]> {
    selection.recheck_selected_originals_and_current_custody()?;
    let preparation = selection.preparation_selection()?;
    let candidate = selection.candidate();
    candidate.recheck_preparation_selection(&preparation, selection.preparation_guard())?;
    let release = selection.authenticated_release()?;
    if release.purpose() != KagemushaReleasePurposeV1::Production {
        return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
    }
    let credential = selection.enrollment().app_credential();
    let before = selection.selected_predecessor_state();
    let after = selection.selected_successor_state();
    let intent = selection.terminal_intent();
    let body = selection.terminal_body();
    let record = selection.terminal_record();
    let prepared = candidate.prepared_record();
    record
        .validate_against_originals(intent, prepared)
        .map_err(integrity)?;
    let body_digest = body.binding_digest().map_err(integrity)?;
    let intent_digest = intent.binding_digest().map_err(integrity)?;
    let challenge = selection.challenge();
    let normalized = selection.normalized_guard_statement();
    let mut expected_normalized = *preparation.normalized_guard_statement();
    expected_normalized.terminal_commit_binding_digest =
        kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
            body_digest,
            candidate.candidate_envelope_digest(),
            candidate.full_state_sha256(),
            prepared.reservation_digest,
        )
        .map_err(integrity)?;
    // The shared position commits earlier genuine ordinary W2. It carries no OEM one-use grant.
    expected_normalized.sender_one_time_authorization_digest = if prepared.operation == 2 {
        prepared.preparation_authorization_digest
    } else {
        [0; 32]
    };
    expected_normalized.transition_intent_digest = body_digest;
    expected_normalized.recovery_record_digest = intent_digest;
    let authorization = selection.authorization_binding_digest()?;
    if normalized != &expected_normalized
        || normalized.predecessor_state_commitment != before.state_commitment
        || normalized.successor_state_commitment != after.state_commitment
        || normalized.predecessor_logical_sequence != before.logical_sequence
        || normalized.successor_logical_sequence != after.logical_sequence
        || normalized.amount != selection.transition_statement().amount
        || body.amount != selection.transition_statement().amount
        || intent.secure_index_before != before.secure_index
        || intent.secure_index_after != after.secure_index
        || u128::from(intent.logical_journal_sequence_before)
            != candidate.public_inputs().journal_revision_before
        || u128::from(intent.logical_journal_sequence_after)
            != candidate.public_inputs().journal_revision_after
        || intent.candidate_digest != candidate.candidate_envelope_digest()
        || intent.state_statement_digest != candidate.full_state_sha256()
        || intent.preparation_id != candidate.preparation_id()?
        || intent.sender_credential_digest != credential.digest()
        || intent.native_operation_id != challenge.operation_id
        || intent.native_nonce != challenge.nonce
        || intent.native_operation_id == preparation.challenge().operation_id
        || intent.native_nonce == preparation.challenge().nonce
        || record.body != *body
        || record.sender_credential_digest != credential.digest()
        || record.preparation_authorization_digest != prepared.preparation_authorization_digest
        || record.terminal_authorization_digest != authorization
        || record.terminal_subject_digest != challenge.subject_signing_digest
        || record.admission_clock_context != *selection.admission_clock_context()
        || record.approval_issued_at_ms != challenge.issued_at_ms
        || record.approval_expires_at_ms != challenge.expires_at_ms
        || challenge.issued_at_ms != intent.issued_at_ms
        || challenge.expires_at_ms != intent.expires_at_ms
        || challenge.account_binding != credential.subject().account_binding
        || challenge.authority_policy_digest != credential.subject().app_authority_policy_digest
        || challenge.attested_key_id != credential.subject().attested_key_id
        || challenge.subject.release_id != release.release_id()
        || challenge.subject.provider_policy_root != release.provider_policy_root()
        || challenge.subject.app_policy_digest != credential.static_binding_digest()
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let mut expected_subject = preparation.challenge().subject;
    expected_subject.candidate_envelope_digest = candidate.candidate_envelope_digest();
    expected_subject.terminal_body_commitment = body_digest;
    if challenge.subject != expected_subject {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let operation = match selection.transition_statement().kind {
        crate::kagemusha_v1_state::KagemushaTransitionKindV1::SendSplit => {
            KagemushaOperationKindV1::SendSplit
        }
        crate::kagemusha_v1_state::KagemushaTransitionKindV1::RedeemSplit => {
            KagemushaOperationKindV1::RedeemSplit
        }
        _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
    };
    let subject_digest = require_terminal_challenge(
        challenge,
        credential.digest(),
        normalized.canonical_digest().map_err(integrity)?,
        selection.transition_statement().digest()?,
        candidate.candidate_envelope_digest(),
        body_digest,
        before.secure_index,
        after.secure_index,
        operation,
    )?;
    let expected = [
        challenge.normalized_guard_digest,
        credential.digest(),
        authorization,
        subject_digest,
        release.provider_policy_root(),
    ];
    if expected.contains(&[0; 32]) {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(expected)
}
#[allow(clippy::too_many_arguments)]
fn require_terminal_challenge(
    challenge: &KagemushaAppOperationApprovalChallengeV1,
    credential: DigestV1,
    normalized: DigestV1,
    statement: DigestV1,
    candidate: DigestV1,
    body: DigestV1,
    secure_before: u128,
    secure_after: u128,
    operation: KagemushaOperationKindV1,
) -> Result<DigestV1> {
    challenge.canonical_signing_bytes().map_err(integrity)?;
    if challenge.purpose != KagemushaAppOperationApprovalPurposeV1::MonetaryTransition
        || !matches!(
            operation,
            KagemushaOperationKindV1::SendSplit | KagemushaOperationKindV1::RedeemSplit
        )
        || candidate == [0; 32]
        || body == [0; 32]
        || challenge.subject.operation_kind != operation
        || challenge.enrollment_digest != credential
        || challenge.subject.credential_id != credential
        || challenge.normalized_guard_digest != normalized
        || challenge.subject.transition_statement_digest != statement
        || challenge.subject.candidate_envelope_digest != candidate
        || challenge.subject.terminal_body_commitment != body
        || challenge.subject.secure_index_before != secure_before
        || challenge.subject.secure_index_after != secure_after
        || secure_before.checked_add(1) != Some(secure_after)
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let digest: DigestV1 = Sha256::digest(
        challenge
            .canonical_subject_signing_bytes()
            .map_err(integrity)?,
    )
    .into();
    if digest != challenge.subject_signing_digest {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(digest)
}
fn require_wire_digests(wire: &OrdinaryGuardProofWireV1, expected: [DigestV1; 5]) -> Result<()> {
    if [
        wire.normalized_guard_digest,
        wire.credential_digest,
        wire.authorization_transcript_digest,
        wire.subject_signing_digest,
        wire.provider_policy_root,
    ] != expected
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}
fn integrity(_: impl core::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::SnapshotIntegrity
}

#[cfg(test)]
#[path = "ordinary_cash_terminal_guard_verifier_tests.rs"]
mod tests;
