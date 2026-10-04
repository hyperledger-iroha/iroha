//! Purpose2 incoming Guard admission against the distinct actual Main incoming selection.
//!
//! This proof result has no conversion to purpose1 or a mutable monetary owner. Its constructor
//! verifies both actual released IPA proofs and terminally decides both entire carried histories.
//! The caller must still prove the selected State and obtain a separate terminal approval.

use super::*;
use crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1;
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalChallengeV1, KagemushaAppOperationApprovalPurposeV1,
    KagemushaOperationKindV1,
};

/// One paired ordinary preparation proof admitted under the same immutable Native incoming operation.
/// The private constructor requires real released proofs and the original captured purpose2 loan.
pub(crate) struct KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1 {
    operation_id: DigestV1,
    nonce: DigestV1,
    admission_interval_ms: (u64, u64),
    digests: [DigestV1; 5],
    eq_history: History,
    ep_history: History,
    original: Vec<u8>,
}
impl KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1 {
    pub(crate) fn original_digests(&self) -> [DigestV1; 5] {
        self.digests
    }
    pub(crate) fn original(&self) -> &[u8] {
        &self.original
    }

    /// Recheck this exact proof result against its still-held immutable incoming selection.
    /// No decoded proof, new challenge, Bootstrap capture or current-time reinterpretation is used.
    pub(crate) fn recheck_incoming_selection(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
    ) -> Result<()> {
        selection.recheck_selected_originals_and_current_custody()?;
        let expected = incoming_preparation_digests(selection)?;
        if self.operation_id != selection.challenge()?.operation_id
            || self.nonce != selection.challenge()?.nonce
            || self.admission_interval_ms != selection.approval_admission_interval_ms()?
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

pub(crate) fn verify_ordinary_incoming_preparation_guard_v1(
    selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
    paired_guard: &[u8],
) -> Result<KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1> {
    selection.recheck_selected_originals_and_current_custody()?;
    let expected = incoming_preparation_digests(selection)?;
    let material = selection
        .recursive_verifier()
        .ordinary_guard_verifier_material();
    let wire = decode_exact(paired_guard, &material)?;
    require_wire_digests(&wire, expected)?;
    verify_wire(&wire, &material)?;
    selection.recheck_selected_originals_and_current_custody()?;
    Ok(KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1 {
        operation_id: selection.challenge()?.operation_id,
        nonce: selection.challenge()?.nonce,
        admission_interval_ms: selection.approval_admission_interval_ms()?,
        digests: expected,
        eq_history: wire.eq_history,
        ep_history: wire.ep_history,
        original: paired_guard.to_vec(),
    })
}

pub(in crate::kagemusha_v1_recursion) fn incoming_preparation_digests(
    selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
) -> Result<[DigestV1; 5]> {
    let release = selection.authenticated_release()?;
    if release.purpose() != KagemushaReleasePurposeV1::Production {
        return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
    }
    let normalized = selection.normalized_guard_statement()?;
    let normalized_digest = normalized
        .canonical_digest()
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    let c = selection.credential()?;
    let challenge = selection.challenge()?;
    let before = selection.selected_predecessor_state()?;
    let after = selection.selected_successor_state()?;
    let subject = &challenge.subject;
    if normalized.terminal_commit_binding_digest != [0; 32]
        || normalized.sender_one_time_authorization_digest != [0; 32]
        || normalized.predecessor_state_commitment != before.state_commitment
        || normalized.successor_state_commitment != after.state_commitment
        || normalized.predecessor_logical_sequence != before.logical_sequence
        || normalized.successor_logical_sequence != after.logical_sequence
        || normalized.amount != selection.transition_statement()?.amount
        || subject.release_id != release.release_id()
        || subject.provider_policy_root != release.provider_policy_root()
        || subject.app_policy_digest != c.static_binding_digest()
        || subject.network_id != before.lane.network_id
        || subject.lane_commitment != before.lane.device_lane_id
        || subject.hardware_profile_id != before.hardware_profile_id
        || subject.policy_epoch != before.policy_epoch
        || subject.hardware_epoch_id != before.hardware_epoch.epoch_id
        || u128::from(subject.hardware_epoch_generation) != before.hardware_epoch.generation
        || challenge.account_binding != c.subject().account_binding
        || challenge.authority_policy_digest != c.subject().app_authority_policy_digest
        || challenge.attested_key_id != c.subject().attested_key_id
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let preparation = selection.preparation()?;
    let reservation = selection.reservation()?;
    let statement = selection.transition_statement()?;
    preparation
        .validate_shape()
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    let reservation_digest = reservation
        .digest()
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    if preparation.reservation_digest != reservation_digest
        || preparation.operation_id != challenge.operation_id
        || preparation.nonce != challenge.nonce
        || preparation.transition_statement_digest != statement.digest()?
        || preparation.predecessor_state_commitment != before.state_commitment
        || preparation.successor_state_commitment != after.state_commitment
        || preparation.financial_index_before != before.secure_index
        || preparation.financial_index_after != after.secure_index
        || u128::from(preparation.logical_journal_sequence_before)
            != statement.journal_revision_before
        || u128::from(preparation.logical_journal_sequence_after)
            != statement.journal_revision_after
        || normalized.transition_intent_digest
            != preparation
                .binding_digest()
                .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
        || normalized.recovery_record_digest
            != preparation
                .recovery_binding_digest()
                .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
        || normalized.transition_effect_digest != reservation_digest
        || normalized.operation != super::super::KagemushaOperationV1::from(statement.kind)
        || reservation.selection.amount != statement.amount
        || reservation.selection.operation_id != challenge.operation_id
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let expected_operation = match selection.transition_statement()?.kind {
        crate::kagemusha_v1_state::KagemushaTransitionKindV1::MintFold => {
            KagemushaOperationKindV1::MintFold
        }
        crate::kagemusha_v1_state::KagemushaTransitionKindV1::ReceiveFold => {
            KagemushaOperationKindV1::ReceiveFold
        }
        _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
    };
    let (lower, upper) = selection.approval_admission_interval_ms()?;
    if lower > upper || lower < challenge.issued_at_ms || upper >= challenge.expires_at_ms {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let subject_digest = require_preparation_challenge(
        challenge,
        c.digest(),
        normalized_digest,
        selection.transition_statement()?.digest()?,
        before.secure_index,
        after.secure_index,
        expected_operation,
    )?;
    let expected = [
        normalized_digest,
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

fn require_preparation_challenge(
    challenge: &KagemushaAppOperationApprovalChallengeV1,
    credential: DigestV1,
    normalized: DigestV1,
    statement: DigestV1,
    secure_before: u128,
    secure_after: u128,
    operation: KagemushaOperationKindV1,
) -> Result<DigestV1> {
    challenge
        .canonical_signing_bytes()
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
    if challenge.purpose != KagemushaAppOperationApprovalPurposeV1::PrepareTransition
        || !matches!(
            operation,
            KagemushaOperationKindV1::MintFold | KagemushaOperationKindV1::ReceiveFold
        )
        || challenge.subject.operation_kind != operation
        || challenge.enrollment_digest != credential
        || challenge.subject.credential_id != credential
        || challenge.normalized_guard_digest != normalized
        || challenge.subject.transition_statement_digest != statement
        || challenge.subject.secure_index_before != secure_before
        || challenge.subject.secure_index_after != secure_after
        || secure_before.checked_add(1) != Some(secure_after)
        || challenge.subject.candidate_envelope_digest != [0; 32]
        || challenge.subject.terminal_body_commitment != [0; 32]
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let digest: DigestV1 = Sha256::digest(
        challenge
            .canonical_subject_signing_bytes()
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{id::NetworkId, kagemusha::KagemushaHardwareTransitionSelectionV1};
    // These are transcript-scope fixtures, not Native owners or proof/clock authority.
    fn challenge(operation: KagemushaOperationKindV1) -> KagemushaAppOperationApprovalChallengeV1 {
        let mut c = KagemushaAppOperationApprovalChallengeV1 {
            version: 1,
            purpose: KagemushaAppOperationApprovalPurposeV1::PrepareTransition,
            operation_id: [1; 32],
            nonce: [2; 32],
            account_binding: [3; 32],
            authority_policy_digest: [4; 32],
            attested_key_id: [5; 32],
            enrollment_digest: [6; 32],
            subject_signing_digest: [7; 32],
            normalized_guard_digest: [8; 32],
            issued_at_ms: 100,
            expires_at_ms: 200,
            subject: KagemushaHardwareTransitionSelectionV1 {
                version: 1,
                release_id: [9; 32],
                provider_policy_root: [10; 32],
                app_policy_digest: [11; 32],
                credential_id: [6; 32],
                network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"ordinary incoming challenge network"),
                )),
                lane_commitment: [13; 32],
                hardware_profile_id: [14; 32],
                policy_epoch: 15,
                hardware_epoch_id: [16; 32],
                hardware_epoch_generation: 17,
                operation_kind: operation,
                transition_statement_digest: [18; 32],
                candidate_envelope_digest: [0; 32],
                terminal_body_commitment: [0; 32],
                secure_index_before: (1_u128 << 100) + 9,
                secure_index_after: (1_u128 << 100) + 10,
            },
        };
        c.subject_signing_digest =
            Sha256::digest(c.canonical_subject_signing_bytes().unwrap()).into();
        c
    }
    fn check(
        c: &KagemushaAppOperationApprovalChallengeV1,
        operation: KagemushaOperationKindV1,
    ) -> Result<DigestV1> {
        require_preparation_challenge(
            c,
            [6; 32],
            [8; 32],
            [18; 32],
            (1_u128 << 100) + 9,
            (1_u128 << 100) + 10,
            operation,
        )
    }
    #[test]
    fn ordinary_incoming_guard_scope_requires_purpose2_full_subject_and_actual_u128_secure_indexes()
    {
        for op in [
            KagemushaOperationKindV1::MintFold,
            KagemushaOperationKindV1::ReceiveFold,
        ] {
            let c = challenge(op);
            assert_eq!(check(&c, op).unwrap(), c.subject_signing_digest);
            let mutations: [fn(&mut KagemushaAppOperationApprovalChallengeV1); 9] = [
                |c| c.purpose = KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
                |c| c.enrollment_digest[0] ^= 1,
                |c| c.subject.credential_id[0] ^= 1,
                |c| c.normalized_guard_digest[0] ^= 1,
                |c| c.subject.transition_statement_digest[0] ^= 1,
                |c| c.subject.secure_index_before = 9,
                |c| c.subject.secure_index_after = 10,
                |c| c.subject.candidate_envelope_digest = [19; 32],
                |c| c.subject.terminal_body_commitment = [20; 32],
            ];
            for mutate in mutations {
                let mut altered = c;
                mutate(&mut altered);
                if let Ok(bytes) = altered.canonical_subject_signing_bytes() {
                    altered.subject_signing_digest = Sha256::digest(bytes).into();
                }
                assert!(check(&altered, op).is_err());
            }
            let other = if op == KagemushaOperationKindV1::MintFold {
                KagemushaOperationKindV1::ReceiveFold
            } else {
                KagemushaOperationKindV1::MintFold
            };
            assert!(check(&c, other).is_err());
            assert!(
                require_preparation_challenge(
                    &c,
                    [6; 32],
                    [8; 32],
                    [18; 32],
                    c.subject.secure_index_before,
                    c.subject.secure_index_before,
                    op
                )
                .is_err()
            );
        }
    }
}
