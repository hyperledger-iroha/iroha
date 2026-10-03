//! Genuine ordinary incoming State candidate under the distinct captured Main W2 loan.
//! Complete public proof originals never substitute for that loan, its selected source/opening,
//! the future purpose1 approval or the actual global reservation/commit receipt.
use super::{
    DigestV1, KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    KagemushaAuthenticatedRecursiveVerifierV1, KagemushaOperationV1,
    KagemushaOrdinaryLineageStateOriginalV1, KagemushaPairedProofV1, KagemushaPastaParityV1,
    KagemushaRecursionArtifactsV1, KagemushaStateRelationPublicInputsV1,
    ordinary_state_reserved::kagemusha_ordinary_state_outer_protocol_positions_v1,
    verify_kagemusha_state_proof_v1,
};
use crate::kagemusha_v1_state::{
    KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1, KagemushaStateErrorV1,
    KagemushaStateV1,
};
use halo2_proofs::halo2curves::pasta::{Fp, Fq};
use iroha_data_model::kagemusha::KagemushaOrdinaryIncomingPreparationV1;
use sha2::{Digest as _, Sha256};
use zeroize::Zeroize as _;
type Result<T> = core::result::Result<T, KagemushaStateErrorV1>;

/// Closed actual State admission from the same immutable purpose2 incoming capture.
/// It cannot be decoded, cloned, or converted into terminal approval or a mutable State owner.
pub(crate) struct KagemushaAuthenticatedOrdinaryIncomingCandidateV1 {
    operation_id: DigestV1,
    nonce: DigestV1,
    admission_interval: (u64, u64),
    guard_digests: [DigestV1; 5],
    guard_original_sha256: DigestV1,
    preparation: KagemushaOrdinaryIncomingPreparationV1,
    reservation_digest: DigestV1,
    statement_digest: DigestV1,
    original: Vec<u8>,
    original_sha256: DigestV1,
    public_inputs: KagemushaStateRelationPublicInputsV1,
    proof: KagemushaPairedProofV1,
    private_checkpoint_original: Vec<u8>,
}
impl Drop for KagemushaAuthenticatedOrdinaryIncomingCandidateV1 {
    fn drop(&mut self) {
        self.private_checkpoint_original.zeroize();
        if let Some(s) = self.public_inputs.predecessor.as_mut() {
            s.balance.zeroize();
            s.state_nonce_commitment.zeroize();
        }
        self.public_inputs.successor.balance.zeroize();
        self.public_inputs
            .successor
            .state_nonce_commitment
            .zeroize();
    }
}
impl KagemushaAuthenticatedOrdinaryIncomingCandidateV1 {
    pub(crate) fn successor_state(&self) -> &KagemushaStateV1 {
        &self.public_inputs.successor
    }
    pub(crate) fn public_state_original(&self) -> &[u8] {
        &self.original
    }
    /// SHA of the complete public State original, including both exact proofs and whole histories.
    /// It is the first-release incoming candidate identity, never a hash of a private balance DTO.
    pub(crate) fn candidate_original_sha256(&self) -> DigestV1 {
        self.original_sha256
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
    pub(crate) fn preparation(&self) -> &KagemushaOrdinaryIncomingPreparationV1 {
        &self.preparation
    }
    pub(crate) fn guard_original_sha256(&self) -> DigestV1 {
        self.guard_original_sha256
    }
    /// Reverify the complete immutable candidate and its same actual source/W2 selection.
    /// No latest clock or financial decision replaces the original capture in a slow proof.
    pub(crate) fn recheck_incoming_selection(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    ) -> Result<()> {
        selection.recheck_selected_originals_and_current_custody()?;
        guard.recheck_incoming_selection(selection)?;
        let expected = reconstruct_public_inputs(selection, guard, &self.proof)?;
        if self.operation_id != selection.challenge()?.operation_id
            || self.nonce != selection.challenge()?.nonce
            || self.admission_interval != selection.approval_admission_interval_ms()?
            || self.guard_digests != guard.original_digests()
            || self.guard_original_sha256 != <[u8; 32]>::from(Sha256::digest(guard.original()))
            || self.preparation != *selection.preparation()?
            || self.reservation_digest != selection.reservation()?.digest().map_err(material)?
            || self.statement_digest != selection.transition_statement()?.digest()?
            || self.original_sha256 != <[u8; 32]>::from(Sha256::digest(&self.original))
            || self.public_inputs != expected
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        verify_complete_original(
            selection.recursive_verifier(),
            selection,
            &expected,
            &self.proof,
            &self.original,
        )?;
        selection.recheck_selected_originals_and_current_custody()
    }
}

/// Capture complete actual generated private State under the same W2 and exact public original.
/// Returns private persistence data only; candidate admission below remains independently mandatory.
pub(crate) fn capture_ordinary_incoming_state_checkpoint_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
    full_public_state_original: &[u8],
    guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    generated: &super::KagemushaGeneratedRecursiveStateProofV1,
) -> Result<Vec<u8>> {
    selection.recheck_selected_originals_and_current_custody()?;
    guard.recheck_incoming_selection(selection)?;
    if !core::ptr::eq(verifier, selection.recursive_verifier()) {
        return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
    }
    let expected = reconstruct_public_inputs(selection, guard, &generated.proof)?;
    verify_complete_original(
        verifier,
        selection,
        &expected,
        &generated.proof,
        full_public_state_original,
    )?;
    let original =
        super::KagemushaRecursiveStateCheckpointV1::capture(generated, verifier, &expected)
            .map_err(material)?
            .encode_canonical(verifier)
            .map_err(material)?;
    selection.recheck_selected_originals_and_current_custody()?;
    Ok(original)
}

/// The only incoming candidate constructor: exact Main capture, admitted Guard and genuine State.
/// Restored WAL originals enter here again; decoding a carrier cannot manufacture this result.
pub(crate) fn verify_ordinary_incoming_candidate_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
    full_public_state_original: &[u8],
    guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    private_checkpoint_original: &[u8],
) -> Result<KagemushaAuthenticatedOrdinaryIncomingCandidateV1> {
    selection.recheck_selected_originals_and_current_custody()?;
    guard.recheck_incoming_selection(selection)?;
    if !core::ptr::eq(verifier, selection.recursive_verifier()) {
        return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
    }
    let carrier =
        KagemushaOrdinaryLineageStateOriginalV1::decode_original(full_public_state_original)
            .map_err(material)?;
    let proof = carrier.proof().clone();
    let public_inputs = reconstruct_public_inputs(selection, guard, &proof)?;
    verify_complete_original(
        verifier,
        selection,
        &public_inputs,
        &proof,
        full_public_state_original,
    )?;
    let restored = super::KagemushaRecursiveStateCheckpointV1::decode_canonical_exact(
        private_checkpoint_original,
        verifier,
    )
    .map_err(material)?
    .restore(verifier, &public_inputs)
    .map_err(material)?;
    if restored.proof != proof {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let candidate = KagemushaAuthenticatedOrdinaryIncomingCandidateV1 {
        operation_id: selection.challenge()?.operation_id,
        nonce: selection.challenge()?.nonce,
        admission_interval: selection.approval_admission_interval_ms()?,
        guard_digests: guard.original_digests(),
        guard_original_sha256: Sha256::digest(guard.original()).into(),
        preparation: *selection.preparation()?,
        reservation_digest: selection.reservation()?.digest().map_err(material)?,
        statement_digest: selection.transition_statement()?.digest()?,
        original: full_public_state_original.to_vec(),
        original_sha256: Sha256::digest(full_public_state_original).into(),
        public_inputs,
        proof,
        private_checkpoint_original: private_checkpoint_original.to_vec(),
    };
    candidate.recheck_incoming_selection(selection, guard)?;
    Ok(candidate)
}
fn selected_artifacts(
    selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
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
fn verify_complete_original(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
    expected: &KagemushaStateRelationPublicInputsV1,
    proof: &KagemushaPairedProofV1,
    raw: &[u8],
) -> Result<()> {
    let carrier =
        KagemushaOrdinaryLineageStateOriginalV1::decode_original(raw).map_err(material)?;
    let (eq, ep) = carrier.public_columns().map_err(material)?;
    if carrier.proof() != proof
        || eq
            != expected
                .recursive_semantic_public_instances::<Fp>()
                .map_err(material)?
        || ep
            != expected
                .recursive_semantic_public_instances::<Fq>()
                .map_err(material)?
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    verify_kagemusha_state_proof_v1(verifier, selected_artifacts(selection)?, expected, proof)
        .map_err(|e| KagemushaStateErrorV1::ProofRejected(e.to_string()))
}
fn require_preparation_identity(
    preparation: &KagemushaOrdinaryIncomingPreparationV1,
    reservation: DigestV1,
    operation: DigestV1,
    nonce: DigestV1,
    statement: DigestV1,
    before: DigestV1,
    after: DigestV1,
    indexes: [u128; 2],
    journal: [u128; 2],
    intent: DigestV1,
    recovery: DigestV1,
) -> Result<()> {
    preparation.validate_shape().map_err(material)?;
    if preparation.reservation_digest != reservation
        || preparation.operation_id != operation
        || preparation.nonce != nonce
        || preparation.transition_statement_digest != statement
        || preparation.predecessor_state_commitment != before
        || preparation.successor_state_commitment != after
        || preparation.financial_index_before != indexes[0]
        || preparation.financial_index_after != indexes[1]
        || u128::from(preparation.logical_journal_sequence_before) != journal[0]
        || u128::from(preparation.logical_journal_sequence_after) != journal[1]
        || preparation.binding_digest().map_err(material)? != intent
        || preparation.recovery_binding_digest().map_err(material)? != recovery
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}
fn reconstruct_public_inputs(
    selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    proof: &KagemushaPairedProofV1,
) -> Result<KagemushaStateRelationPublicInputsV1> {
    let before = selection.selected_predecessor_state()?;
    let after = selection.selected_successor_state()?;
    let statement = selection.transition_statement()?;
    let normalized = selection.normalized_guard_statement()?;
    // These two operation branches consume distinct complete first-release source graphs.
    // The authentic released ordinary State key must contain both Mint113 and Receive83.
    let operation = KagemushaOperationV1::from(statement.kind);
    if !matches!(
        operation,
        KagemushaOperationV1::MintFold | KagemushaOperationV1::ReceiveFold
    ) {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    let digests = guard.original_digests();
    let artifacts = selected_artifacts(selection)?;
    let envelope = selection.reservation()?.digest().map_err(material)?;
    require_preparation_identity(
        selection.preparation()?,
        envelope,
        selection.challenge()?.operation_id,
        selection.challenge()?.nonce,
        statement.digest()?,
        before.state_commitment,
        after.state_commitment,
        [before.secure_index, after.secure_index],
        [
            statement.journal_revision_before,
            statement.journal_revision_after,
        ],
        normalized.transition_intent_digest,
        normalized.recovery_record_digest,
    )?;
    if normalized.operation != operation
        || statement.effect_digest != envelope
        || statement.amount != selection.reservation()?.selection.amount
        || statement.prepared_transition_binding_digest != [0; 32]
        || statement.peer_credit_id != [0; 32]
        || statement.recipient_encryption_key_binding != [0; 32]
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    match operation {
        KagemushaOperationV1::MintFold => {
            if statement.receive_credit_binding_digest != [0; 32]
                || statement.mint_finality_semantic_digest
                    != selection.reservation()?.source_semantic_digest
                || statement.mint_finality_proof_binding_digest == [0; 32]
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        KagemushaOperationV1::ReceiveFold => {
            if statement.receive_credit_binding_digest != envelope
                || statement.mint_finality_semantic_digest != [0; 32]
                || statement.mint_finality_proof_binding_digest != [0; 32]
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            let mut entered = false;
            selection.with_received_source(&mut |source| {
                if entered {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                entered = true;
                let reservation = selection.reservation()?;
                if source.amount() != statement.amount
                    || source.credit_id() != reservation.selection.credit_id
                    || source.receiver_credential_digest() != selection.credential()?.digest()
                    || reservation.source_proof_original_sha256
                        != <DigestV1>::from(Sha256::digest(source.outgoing_original()))
                    || reservation.finalized_source_original_sha256
                        != source.received_assertion_original_sha256()
                    || reservation.source_semantic_digest
                        != source.output().binding_digest().map_err(material)?
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                Ok(())
            })?;
            if !entered {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
    }
    let (eq_outer, ep_outer) =
        kagemusha_ordinary_state_outer_protocol_positions_v1(selection.recursive_verifier());
    if proof.guard_eq_credential_audit != eq_outer || proof.guard_ep_credential_audit != ep_outer {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(KagemushaStateRelationPublicInputsV1 {
        operation,
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
        prepared_intent: None,
        transport_semantic_digest: selection.transport_semantic_digest()?,
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
        guard_eq_credential_audit: eq_outer,
        guard_ep_credential_audit: ep_outer,
        eq_deferred_audit: proof.eq_deferred_audit,
        ep_deferred_audit: proof.ep_deferred_audit,
    })
}
fn material(_: impl core::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::SnapshotIntegrity
}
#[cfg(test)]
mod tests {
    use super::*;
    fn preparation() -> KagemushaOrdinaryIncomingPreparationV1 {
        KagemushaOrdinaryIncomingPreparationV1 {
            version: 1,
            reservation_digest: [1; 32],
            operation_id: [2; 32],
            nonce: [3; 32],
            transition_statement_digest: [4; 32],
            predecessor_state_commitment: [5; 32],
            successor_state_commitment: [6; 32],
            financial_control_original_sha256: [7; 32],
            clock_context_digest: [8; 32],
            financial_index_before: (1_u128 << 100) + 9,
            financial_index_after: (1_u128 << 100) + 10,
            logical_journal_sequence_before: 11,
            logical_journal_sequence_after: 12,
        }
    }
    #[test]
    fn incoming_candidate_scope_preserves_full_financial_indexes_source_and_fresh_capture_originals()
     {
        // Transcript-only data: no fixture creates a candidate, Native loan or accepting proof.
        let p = preparation();
        let intent = p.binding_digest().unwrap();
        let recovery = p.recovery_binding_digest().unwrap();
        let check = |p: &KagemushaOrdinaryIncomingPreparationV1| {
            require_preparation_identity(
                p,
                [1; 32],
                [2; 32],
                [3; 32],
                [4; 32],
                [5; 32],
                [6; 32],
                [(1_u128 << 100) + 9, (1_u128 << 100) + 10],
                [11, 12],
                intent,
                recovery,
            )
        };
        check(&p).unwrap();
        let changes: [fn(&mut KagemushaOrdinaryIncomingPreparationV1); 10] = [
            |p| p.reservation_digest[0] ^= 1,
            |p| p.operation_id[0] ^= 1,
            |p| p.nonce[0] ^= 1,
            |p| p.transition_statement_digest[0] ^= 1,
            |p| p.predecessor_state_commitment[0] ^= 1,
            |p| p.successor_state_commitment[0] ^= 1,
            |p| p.financial_index_before = 9,
            |p| p.financial_index_after = 10,
            |p| p.financial_control_original_sha256[0] ^= 1,
            |p| p.clock_context_digest[0] ^= 1,
        ];
        for change in changes {
            let mut changed = p;
            change(&mut changed);
            assert!(check(&changed).is_err());
        }
        assert!(
            require_preparation_identity(
                &p,
                [1; 32],
                [2; 32],
                [3; 32],
                [4; 32],
                [5; 32],
                [6; 32],
                [(1_u128 << 100) + 9, (1_u128 << 100) + 10],
                [(1_u128 << 100) + 11, (1_u128 << 100) + 12],
                intent,
                recovery
            )
            .is_err()
        );
    }
}
