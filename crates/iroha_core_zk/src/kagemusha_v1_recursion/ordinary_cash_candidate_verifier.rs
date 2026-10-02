//! Actual ordinary outgoing State admission after a genuine purpose2 Guard.
//!
//! The closed result can select the later Native purpose1 attempt only while the same cash owner
//! remains held. Shared prepared records and candidate digests are data. This constructor instead
//! rebuilds every State input from that owner and requires actual paired proofs and whole histories.

use super::{
    DigestV1, KagemushaAuthenticatedRecursiveVerifierV1, KagemushaOperationV1,
    KagemushaPairedProofV1, KagemushaPastaParityV1, KagemushaPreparedIntentCommitmentsV1,
    KagemushaRecursionArtifactsV1, KagemushaStateRelationPublicInputsV1,
    ordinary_guard_verifier::KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    ordinary_state_reserved::kagemusha_ordinary_state_outer_protocol_positions_v1,
    terminal_authorization::kagemusha_candidate_envelope_digest_v1,
    verify_kagemusha_state_proof_v1,
};
use crate::kagemusha_v1_state::{
    KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1, KagemushaStateErrorV1, KagemushaStateV1,
    KagemushaTransitionKindV1,
};
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryPreparedOutgoingV1, KagemushaOrdinaryPreparedTransitionV1,
};
use sha2::{Digest as _, Sha256};

type Result<T> = core::result::Result<T, KagemushaStateErrorV1>;

/// Actual paired ordinary State proof admitted against one exact purpose2 preparation.
/// There is no decoder, accepting verifier overload or conversion to an approval/mutable owner.
pub(crate) struct KagemushaAuthenticatedOrdinaryCashCandidateV1 {
    operation_id: DigestV1,
    nonce: DigestV1,
    admission_time_ms: u64,
    guard_digests: [DigestV1; 5],
    guard_original_sha256: DigestV1,
    prepared: KagemushaOrdinaryPreparedOutgoingV1,
    candidate_digest: DigestV1,
    state_sha256: DigestV1,
    public_inputs: KagemushaStateRelationPublicInputsV1,
    proof: KagemushaPairedProofV1,
    private_checkpoint_original: Vec<u8>,
}
impl Drop for KagemushaAuthenticatedOrdinaryCashCandidateV1 {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.private_checkpoint_original.zeroize();
        if let Some(before) = self.public_inputs.predecessor.as_mut() {
            before.balance.zeroize();
            before.state_nonce_commitment.zeroize();
        }
        self.public_inputs.successor.balance.zeroize();
        self.public_inputs
            .successor
            .state_nonce_commitment
            .zeroize();
    }
}
impl KagemushaAuthenticatedOrdinaryCashCandidateV1 {
    pub(crate) fn prepared_record(&self) -> &KagemushaOrdinaryPreparedOutgoingV1 {
        &self.prepared
    }
    pub(crate) fn preparation_id(&self) -> Result<DigestV1> {
        self.prepared.binding_digest().map_err(material)
    }
    pub(crate) fn candidate_envelope_digest(&self) -> DigestV1 {
        self.candidate_digest
    }
    pub(crate) fn full_state_sha256(&self) -> DigestV1 {
        self.state_sha256
    }
    pub(crate) fn predecessor_state(&self) -> &KagemushaStateV1 {
        self.public_inputs
            .predecessor
            .as_ref()
            .expect("closed outgoing candidate predecessor")
    }
    pub(crate) fn successor_state(&self) -> &KagemushaStateV1 {
        &self.public_inputs.successor
    }
    pub(crate) fn public_inputs(&self) -> &KagemushaStateRelationPublicInputsV1 {
        &self.public_inputs
    }
    pub(crate) fn private_checkpoint_original(&self) -> &[u8] {
        &self.private_checkpoint_original
    }
    pub(crate) fn with_retained_checkpoint(
        &self,
        verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
        consume: &mut dyn for<'a> FnMut(
            &'a super::KagemushaGeneratedRecursiveStateProofV1,
        ) -> core::result::Result<(), KagemushaStateErrorV1>,
    ) -> Result<()> {
        let restored = super::KagemushaRecursiveStateCheckpointV1::decode_canonical_exact(
            &self.private_checkpoint_original,
            verifier,
        )
        .map_err(material)?
        .restore(verifier, &self.public_inputs)
        .map_err(material)?;
        if restored.proof != self.proof {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        consume(&restored)
    }
    pub(crate) fn proof(&self) -> &KagemushaPairedProofV1 {
        &self.proof
    }
    pub(crate) fn preparation_operation_id(&self) -> DigestV1 {
        self.operation_id
    }

    /// Recheck the same immutable candidate selection before the Native terminal intent is made.
    /// This result never substitutes for a fresh purpose1 approval or an actual financial CAS.
    pub(crate) fn recheck_preparation_selection(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    ) -> Result<()> {
        selection.recheck_selected_originals_and_current_custody()?;
        guard.recheck_preparation_selection(selection)?;
        let expected = reconstruct_public_inputs(selection, guard, &self.prepared, &self.proof)?;
        if self.operation_id != selection.challenge().operation_id
            || self.nonce != selection.challenge().nonce
            || self.admission_time_ms != selection.approval_admission_time_ms()
            || self.guard_digests != guard.original_digests()
            || self.guard_original_sha256 != <DigestV1>::from(Sha256::digest(guard.original()))
            || self.state_sha256 != selection.transition_statement().digest()?
            || self.public_inputs != expected
            || self.candidate_digest
                != kagemusha_candidate_envelope_digest_v1(&expected).map_err(material)?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        // Only this module can construct or mutate the result, but restored data must enter through
        // the real verifier again. A decoded candidate cannot recover this private constructor.
        let artifacts = selected_artifacts(selection)?;
        verify_kagemusha_state_proof_v1(
            selection.recursive_verifier(),
            artifacts,
            &expected,
            &self.proof,
        )
        .map_err(|e| KagemushaStateErrorV1::ProofRejected(e.to_string()))?;
        selection.recheck_selected_originals_and_current_custody()
    }
}

/// Capture actual generated private State for the same outgoing preparation and expected state.
/// Data alone grants no candidate, W1, global Commit or StateAdvance authority.
pub(crate) fn capture_ordinary_cash_state_checkpoint_v1(
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    prepared: &KagemushaOrdinaryPreparedOutgoingV1,
    generated: &super::KagemushaGeneratedRecursiveStateProofV1,
) -> Result<Vec<u8>> {
    selection.recheck_selected_originals_and_current_custody()?;
    guard.recheck_preparation_selection(selection)?;
    let expected = reconstruct_public_inputs(selection, guard, prepared, &generated.proof)?;
    let original = super::KagemushaRecursiveStateCheckpointV1::capture(
        generated,
        selection.recursive_verifier(),
        &expected,
    )
    .map_err(material)?
    .encode_canonical(selection.recursive_verifier())
    .map_err(material)?;
    selection.recheck_selected_originals_and_current_custody()?;
    Ok(original)
}

pub(crate) fn verify_ordinary_cash_candidate_v1(
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    prepared: KagemushaOrdinaryPreparedOutgoingV1,
    proof: KagemushaPairedProofV1,
    private_checkpoint_original: &[u8],
) -> Result<KagemushaAuthenticatedOrdinaryCashCandidateV1> {
    selection.recheck_selected_originals_and_current_custody()?;
    guard.recheck_preparation_selection(selection)?;
    let public_inputs = reconstruct_public_inputs(selection, guard, &prepared, &proof)?;
    let artifacts = selected_artifacts(selection)?;
    verify_kagemusha_state_proof_v1(
        selection.recursive_verifier(),
        artifacts,
        &public_inputs,
        &proof,
    )
    .map_err(|e| KagemushaStateErrorV1::ProofRejected(e.to_string()))?;
    let restored = super::KagemushaRecursiveStateCheckpointV1::decode_canonical_exact(
        private_checkpoint_original,
        selection.recursive_verifier(),
    )
    .map_err(material)?
    .restore(selection.recursive_verifier(), &public_inputs)
    .map_err(material)?;
    if restored.proof != proof {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let candidate_digest =
        kagemusha_candidate_envelope_digest_v1(&public_inputs).map_err(material)?;
    selection.recheck_selected_originals_and_current_custody()?;
    Ok(KagemushaAuthenticatedOrdinaryCashCandidateV1 {
        operation_id: selection.challenge().operation_id,
        nonce: selection.challenge().nonce,
        admission_time_ms: selection.approval_admission_time_ms(),
        guard_digests: guard.original_digests(),
        guard_original_sha256: Sha256::digest(guard.original()).into(),
        prepared,
        candidate_digest,
        state_sha256: selection.transition_statement().digest()?,
        public_inputs,
        proof,
        private_checkpoint_original: private_checkpoint_original.to_vec(),
    })
}

fn selected_artifacts(
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
) -> Result<KagemushaRecursionArtifactsV1> {
    let release = selection.authenticated_release()?;
    let empty =
        crate::kagemusha_v1_state::canonical_empty_durable_effect_digest_v1(release.release_id())?;
    let expected =
        KagemushaRecursionArtifactsV1::from_authenticated_ordinary_release(&release, empty)
            .map_err(material)?;
    let actual = selection
        .recursive_verifier()
        .state_checkpoint_material()
        .artifacts;
    if actual != expected {
        return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
    }
    Ok(actual)
}

fn reconstruct_public_inputs(
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    prepared: &KagemushaOrdinaryPreparedOutgoingV1,
    proof: &KagemushaPairedProofV1,
) -> Result<KagemushaStateRelationPublicInputsV1> {
    let before = selection.selected_predecessor_state();
    let after = selection.selected_successor_state();
    let statement = selection.transition_statement();
    let operation = match statement.kind {
        KagemushaTransitionKindV1::SendSplit => 2,
        KagemushaTransitionKindV1::RedeemSplit => 4,
        _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
    };
    let digests = guard.original_digests();
    let artifacts = selected_artifacts(selection)?;
    require_prepared_originals(
        prepared,
        KagemushaOrdinaryPreparedTransitionV1 {
            version: 1,
            operation,
            lifecycle_digest: statement.lifecycle_binding_digest,
            request_digest: prepared.request_digest,
            predecessor_state: before.state_commitment,
            successor_state: after.state_commitment,
            amount: statement.amount,
            reservation_digest: prepared.reservation_digest,
            native_preparation_operation_id: selection.challenge().operation_id,
        },
        statement.digest()?,
        digests[0],
        digests[2],
        artifacts.artifact_manifest_digest,
    )?;
    if prepared.prepared_transition_binding_digest != statement.prepared_transition_binding_digest
        || KagemushaOperationV1::from(statement.kind)
            != selection.normalized_guard_statement().operation
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let (eq_reserved, ep_reserved) =
        kagemusha_ordinary_state_outer_protocol_positions_v1(selection.recursive_verifier());
    if proof.guard_eq_credential_audit != eq_reserved
        || proof.guard_ep_credential_audit != ep_reserved
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(KagemushaStateRelationPublicInputsV1 {
        operation: if operation == 2 {
            KagemushaOperationV1::SendSplit
        } else {
            KagemushaOperationV1::RedeemSplit
        },
        predecessor: Some(before.clone()),
        successor: after.clone(),
        amount: statement.amount,
        journal_revision_before: statement.journal_revision_before,
        journal_revision_after: statement.journal_revision_after,
        transition_effect_digest: statement.effect_digest,
        mint_finality_semantic_digest: statement.mint_finality_semantic_digest,
        mint_finality_proof_binding_digest: statement.mint_finality_proof_binding_digest,
        peer_credit_id: statement.peer_credit_id,
        recipient_encryption_key_binding: statement.recipient_encryption_key_binding,
        receive_credit_binding_digest: statement.receive_credit_binding_digest,
        lifecycle_binding_digest: statement.lifecycle_binding_digest,
        prepared_transition_binding_digest: statement.prepared_transition_binding_digest,
        prepared_intent: Some(KagemushaPreparedIntentCommitmentsV1 {
            preparation_id: prepared.binding_digest().map_err(material)?,
            sealed_transition_inputs_digest: prepared.stream_digests[0],
            sealed_recovery_seeds_digest: prepared.stream_digests[1],
        }),
        transport_semantic_digest: prepared.projection_semantic_digest,
        guard_statement_digest: digests[0],
        eq_protocol_digest: artifacts.eq_protocol_digest,
        ep_protocol_digest: artifacts.ep_protocol_digest,
        guard_eq_protocol_digest: artifacts
            .guard_bundle_protocol_digest(KagemushaPastaParityV1::Eq)
            .map_err(material)?,
        guard_ep_protocol_digest: artifacts
            .guard_bundle_protocol_digest(KagemushaPastaParityV1::Ep)
            .map_err(material)?,
        mint_eq_protocol_digest: artifacts
            .mint_finality_protocol_digest(KagemushaPastaParityV1::Eq)
            .map_err(material)?,
        mint_ep_protocol_digest: artifacts
            .mint_finality_protocol_digest(KagemushaPastaParityV1::Ep)
            .map_err(material)?,
        mint_authorization_eq_protocol_digest: artifacts.mint_authorization_eq_protocol_digest,
        mint_authorization_ep_protocol_digest: artifacts.mint_authorization_ep_protocol_digest,
        commit_wrapper_eq_protocol_digest: artifacts.commit_wrapper_eq_protocol_digest,
        commit_wrapper_ep_protocol_digest: artifacts.commit_wrapper_ep_protocol_digest,
        guard_eq_credential_audit: eq_reserved,
        guard_ep_credential_audit: ep_reserved,
        eq_deferred_audit: proof.eq_deferred_audit,
        ep_deferred_audit: proof.ep_deferred_audit,
    })
}

fn require_prepared_originals(
    record: &KagemushaOrdinaryPreparedOutgoingV1,
    transition: KagemushaOrdinaryPreparedTransitionV1,
    full_state_sha: DigestV1,
    normalized_guard: DigestV1,
    authorization: DigestV1,
    artifact_manifest: DigestV1,
) -> Result<()> {
    record.validate_shape().map_err(material)?;
    transition.validate_shape().map_err(material)?;
    if record.operation != transition.operation
        || record.predecessor_state != transition.predecessor_state
        || record.successor_state != transition.successor_state
        || record.transition_digest != full_state_sha
        || record.prepared_transition_binding_digest
            != transition.binding_digest().map_err(material)?
        || record.lifecycle_binding_digest != transition.lifecycle_digest
        || record.request_digest != transition.request_digest
        || record.reservation_digest != transition.reservation_digest
        || record.preparation_guard_digest != normalized_guard
        || record.preparation_authorization_digest != authorization
        || (record.operation == 4 && record.artifact_manifest_digest != artifact_manifest)
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}
fn material(_: impl core::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::SnapshotIntegrity
}

#[cfg(test)]
mod tests {
    use super::*;
    // Pure original-field fixtures only. No fixture makes a Native candidate or accepts a proof.
    fn fields(
        operation: u8,
    ) -> (
        KagemushaOrdinaryPreparedTransitionV1,
        KagemushaOrdinaryPreparedOutgoingV1,
    ) {
        let t = KagemushaOrdinaryPreparedTransitionV1 {
            version: 1,
            operation,
            lifecycle_digest: [1; 32],
            request_digest: if operation == 2 { [2; 32] } else { [0; 32] },
            predecessor_state: [3; 32],
            successor_state: [4; 32],
            amount: (1_u128 << 100) + 5,
            reservation_digest: [6; 32],
            native_preparation_operation_id: [7; 32],
        };
        let p = KagemushaOrdinaryPreparedOutgoingV1 {
            version: 1,
            operation,
            predecessor_state: t.predecessor_state,
            successor_state: t.successor_state,
            transition_digest: [8; 32],
            prepared_transition_binding_digest: t.binding_digest().unwrap(),
            projection_semantic_digest: [9; 32],
            lifecycle_binding_digest: t.lifecycle_digest,
            request_digest: t.request_digest,
            artifact_manifest_digest: if operation == 4 { [10; 32] } else { [0; 32] },
            preparation_guard_digest: [11; 32],
            reservation_digest: t.reservation_digest,
            preparation_authorization_digest: [12; 32],
            stream_lengths: [2048, 512],
            stream_digests: [[13; 32], [14; 32]],
        };
        (t, p)
    }
    #[test]
    fn ordinary_candidate_originals_require_exact_acyclic_preparation_before_any_proof_admission() {
        for op in [2, 4] {
            let (t, p) = fields(op);
            let check = |p: &KagemushaOrdinaryPreparedOutgoingV1, t| {
                require_prepared_originals(p, t, [8; 32], [11; 32], [12; 32], [10; 32])
            };
            check(&p, t).unwrap();
            for field in 0..10 {
                let mut q = p;
                match field {
                    0 => q.predecessor_state[0] ^= 1,
                    1 => q.successor_state[0] ^= 1,
                    2 => q.transition_digest[0] ^= 1,
                    3 => q.prepared_transition_binding_digest[0] ^= 1,
                    4 => q.lifecycle_binding_digest[0] ^= 1,
                    5 => q.request_digest[0] ^= 1,
                    6 => q.reservation_digest[0] ^= 1,
                    7 => q.preparation_guard_digest[0] ^= 1,
                    8 => q.preparation_authorization_digest[0] ^= 1,
                    _ => q.artifact_manifest_digest[0] ^= 1,
                }
                assert!(check(&q, t).is_err());
            }
            for field in 0..3 {
                let mut q = t;
                match field {
                    0 => q.amount = 5,
                    1 => q.native_preparation_operation_id[0] ^= 1,
                    _ => q.reservation_digest[0] ^= 1,
                }
                assert!(check(&p, q).is_err());
            }
            let mut q = p;
            q.stream_lengths[0] = 2049;
            assert!(check(&q, t).is_err());
            let mut q = p;
            q.stream_lengths[1] = 513;
            assert!(check(&q, t).is_err());
        }
    }
}
