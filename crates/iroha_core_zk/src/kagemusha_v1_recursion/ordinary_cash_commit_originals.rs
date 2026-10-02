//! Proof-verification-only ordinary Commit originals and durable re-admission.
//! Available to the Native WAL under zk-halo2-ipa, without production proving features.
//! No key resolver, proving key, key generation or randomized proof producer is imported.
//! Neither these bytes nor a successful proof admission is a global DATA/current FI effect.
use super::{
    KagemushaArtifactGenerationErrorV1, KagemushaAuthenticatedOrdinaryCashTerminalV1,
    KagemushaAuthenticatedOrdinaryTerminalGuardV1, KagemushaAuthenticatedRecursiveVerifierV1,
    verify_ordinary_cash_terminal_v1,
};
use crate::kagemusha_v1_recursion::{
    KagemushaOrdinaryCashOutgoingOriginalV1, KagemushaOrdinaryLineageCommitProofBundleV1,
    KagemushaOrdinaryLineageOutgoingOriginalsV1, KagemushaOrdinaryLineageStateOriginalV1,
    KagemushaOrdinaryLineageStateProofBundleV1, KagemushaOrdinaryLineageStatementOriginalV1,
    KagemushaVerifiedOrdinaryLineageCommitProofV1, ordinary_cash_carrier_budget_v1,
    verify_ordinary_lineage_commit_v1, verify_ordinary_lineage_reservation_v1,
};
use crate::kagemusha_v1_state::{
    DigestV1, KagemushaAuthenticatedOrdinaryLineageReservationReceiptV1,
};
use crate::kagemusha_v1_state::{
    KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1, KagemushaStateErrorV1,
};
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryLineageCommitV1, kagemusha_ordinary_transition_nullifier_v1,
};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroize as _;

/// Exact admitted mathematical proof and data frames. Only the subsequent genuine acknowledged
/// Core DATA Commit and fresh current FI/clock owner may advance State or expose an outbox.
/// The pre-receipt outgoing frame deliberately cannot contain its own future global receipt.
pub(crate) struct GeneratedOrdinaryCashCommitOriginalsV1 {
    proof: KagemushaVerifiedOrdinaryLineageCommitProofV1,
    selected_successor_state: crate::kagemusha_v1_state::KagemushaStateV1,
    successor_public_state_original: Vec<u8>,
    successor_private_checkpoint_original: Vec<u8>,
    successor_public_inputs: super::KagemushaStateRelationPublicInputsV1,
    private_service_original: Vec<u8>,
    pre_receipt_outgoing_original: Vec<u8>,
}
impl Drop for GeneratedOrdinaryCashCommitOriginalsV1 {
    fn drop(&mut self) {
        if let Some(before) = self.successor_public_inputs.predecessor.as_mut() {
            before.balance.zeroize();
            before.state_nonce_commitment.zeroize();
        }
        self.successor_public_inputs.successor.balance.zeroize();
        self.successor_public_inputs
            .successor
            .state_nonce_commitment
            .zeroize();
        self.successor_private_checkpoint_original.zeroize();
        self.selected_successor_state.balance.zeroize();
        self.selected_successor_state
            .state_nonce_commitment
            .zeroize();
    }
}

impl GeneratedOrdinaryCashCommitOriginalsV1 {
    pub(crate) fn proof(&self) -> &KagemushaVerifiedOrdinaryLineageCommitProofV1 {
        &self.proof
    }
    pub(crate) fn commit(&self) -> &KagemushaOrdinaryLineageCommitV1 {
        self.proof.commit()
    }
    /// Actual privately retained selected successor. No decoder or offered State creates this
    /// borrow; the assembler joins it to the same actual candidate/whole proof before custody ends.
    pub(crate) fn selected_successor_state(&self) -> &crate::kagemusha_v1_state::KagemushaStateV1 {
        &self.selected_successor_state
    }
    /// Actual full PUBLIC checkpoint generated from the same proof-admitted candidate.
    /// Its complete SHA is the exact globally committed successor Head selector.
    pub(crate) fn successor_public_state_original(&self) -> &[u8] {
        &self.successor_public_state_original
    }
    /// Data-only decoded complete bundle from this already genuinely admitted immutable cap.
    /// No offered/decoded bundle can construct the cap or substitute for its retained full original.
    pub(crate) fn proof_bundle(
        &self,
    ) -> Result<KagemushaOrdinaryLineageCommitProofBundleV1, KagemushaArtifactGenerationErrorV1>
    {
        KagemushaOrdinaryLineageCommitProofBundleV1::decode_original(&self.private_service_original)
            .map_err(proving_error)
    }
    pub(crate) fn successor_private_checkpoint_original(&self) -> &[u8] {
        &self.successor_private_checkpoint_original
    }
    pub(crate) fn with_successor_checkpoint(
        &self,
        verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
        consume: &mut dyn for<'a> FnMut(
            &'a super::KagemushaGeneratedRecursiveStateProofV1,
        ) -> core::result::Result<(), KagemushaStateErrorV1>,
    ) -> core::result::Result<(), KagemushaStateErrorV1> {
        let restored = super::KagemushaRecursiveStateCheckpointV1::decode_canonical_exact(
            &self.successor_private_checkpoint_original,
            verifier,
        )
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
        .restore(verifier, &self.successor_public_inputs)
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        consume(&restored)
    }
    pub(crate) fn private_service_original(&self) -> &[u8] {
        &self.private_service_original
    }
    pub(crate) fn pre_receipt_outgoing_original(&self) -> &[u8] {
        &self.pre_receipt_outgoing_original
    }
}

/// Re-admit durable pending Commit originals using only the actual held authenticated verifier.
/// Native W1/W2/FI/clock/Reserve custody and every exact proof original remain mandatory.
/// This kernel owns no resolver, profile constructor, proving keys or proof-generation call.
pub(crate) fn readmit_ordinary_cash_commit_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
    reservation: &KagemushaAuthenticatedOrdinaryLineageReservationReceiptV1<'_>,
    persisted_commit: &KagemushaOrdinaryLineageCommitV1,
    persisted_service_original: &[u8],
    persisted_pre_receipt_outgoing_original: &[u8],
    persisted_successor_public_state_original: &[u8],
) -> Result<GeneratedOrdinaryCashCommitOriginalsV1, KagemushaArtifactGenerationErrorV1> {
    selection
        .recheck_selected_originals_and_current_custody()
        .map_err(owner_error)?;
    selection
        .recheck_lineage_reservation(reservation)
        .map_err(owner_error)?;
    require_selected_verifier_release(verifier, selection)?;
    let budget = ordinary_cash_carrier_budget_v1(verifier).map_err(proving_error)?;
    require_pre_receipt_capacity(
        persisted_service_original.len(),
        persisted_pre_receipt_outgoing_original.len(),
        &budget,
        selection
            .outbox_reservation_original()
            .reserved_outbox_bytes,
    )?;
    persisted_commit.validate_shape().map_err(proving_error)?;
    let persisted =
        KagemushaOrdinaryLineageCommitProofBundleV1::decode_original(persisted_service_original)
            .map_err(proving_error)?;
    if persisted.outgoing_original() != persisted_pre_receipt_outgoing_original {
        return Err(proving_error(
            "persisted complete outgoing bytes differ from the service bundle",
        ));
    }
    let expected_successor =
        KagemushaOrdinaryLineageStateOriginalV1::from_admitted_candidate(selection.candidate())
            .map_err(proving_error)?
            .canonical_bytes()
            .map_err(proving_error)?;
    if expected_successor != persisted_successor_public_state_original {
        return Err(proving_error(
            "persisted complete successor differs from actual selected candidate",
        ));
    }
    let guard =
        crate::kagemusha_v1_recursion::ordinary_guard_verifier::verify_ordinary_terminal_guard_v1(
            selection,
            persisted.terminal_guard_original(),
        )
        .map_err(owner_error)?;
    let outgoing = persisted.outgoing().map_err(proving_error)?;
    let whole = verify_ordinary_cash_terminal_v1(
        selection,
        &guard,
        persisted.inner_terminal_original(),
        outgoing.wrapper_original(),
        persisted.wrapper_history_fold_originals(),
    )
    .map_err(owner_error)?;
    // This assembler only encodes and genuinely re-verifies retained proof bytes; it calls
    // neither a proof producer nor key generation. Exact historical clock/FI originals are
    // borrowed again from their same Native custody, not substituted by current replacements.
    let recovered =
        assemble_ordinary_cash_commit_v1(verifier, selection, &guard, &whole, reservation)?;
    if recovered.commit() != persisted_commit
        || recovered.private_service_original() != persisted_service_original
        || recovered.pre_receipt_outgoing_original() != persisted_pre_receipt_outgoing_original
        || recovered.successor_public_state_original() != persisted_successor_public_state_original
    {
        return Err(proving_error(
            "persisted Commit/full proof originals do not match the genuine recovered owner",
        ));
    }
    whole
        .recheck_terminal_selection(selection, &guard)
        .map_err(owner_error)?;
    selection
        .recheck_lineage_reservation(reservation)
        .map_err(owner_error)?;
    selection
        .recheck_selected_originals_and_current_custody()
        .map_err(owner_error)?;
    Ok(recovered)
}

pub(super) fn assemble_ordinary_cash_commit_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
    terminal_guard: &KagemushaAuthenticatedOrdinaryTerminalGuardV1,
    whole: &KagemushaAuthenticatedOrdinaryCashTerminalV1,
    reservation: &KagemushaAuthenticatedOrdinaryLineageReservationReceiptV1<'_>,
) -> Result<GeneratedOrdinaryCashCommitOriginalsV1, KagemushaArtifactGenerationErrorV1> {
    selection
        .recheck_selected_originals_and_current_custody()
        .map_err(owner_error)?;
    selection
        .recheck_lineage_reservation(reservation)
        .map_err(owner_error)?;
    terminal_guard
        .recheck_terminal_selection(selection)
        .map_err(owner_error)?;
    whole
        .recheck_terminal_selection(selection, terminal_guard)
        .map_err(owner_error)?;
    require_selected_verifier_release(verifier, selection)?;
    let budget = ordinary_cash_carrier_budget_v1(verifier).map_err(proving_error)?;
    let neutral = selection.outbox_reservation_original();
    budget
        .require_reserved_bytes(neutral.reserved_outbox_bytes)
        .map_err(proving_error)?;
    let preparation = selection.preparation_selection().map_err(owner_error)?;
    selection
        .candidate()
        .recheck_preparation_selection(&preparation, selection.preparation_guard())
        .map_err(owner_error)?;
    let candidate_original =
        KagemushaOrdinaryLineageStateOriginalV1::from_admitted_candidate(selection.candidate())
            .map_err(proving_error)?
            .canonical_bytes()
            .map_err(proving_error)?;
    let predecessor_original = selection
        .selected_predecessor_public_state_original()
        .map_err(owner_error)?;
    let neutral_original =
        norito::encode_canonical(neutral).map_err(|e| proving_error(e.to_string()))?;
    let outgoing = actual_outgoing_originals(selection)?;
    let preparation_bundle = KagemushaOrdinaryLineageStateProofBundleV1::from_public_parts(
        preparation.normalized_guard_statement().clone(),
        KagemushaOrdinaryLineageStatementOriginalV1::Outgoing(Box::new(
            preparation.transition_statement().clone(),
        )),
        candidate_original.clone(),
        preparation
            .enrollment()
            .app_credential()
            .original()
            .to_vec(),
        preparation.original().to_vec(),
        preparation
            .original_approval_integrity_lease()
            .map(|lease| lease.original().to_vec()),
        selection.preparation_guard().original().to_vec(),
        Some(selection.candidate().prepared_record().clone()),
        Some(outgoing.clone()),
    )
    .map_err(proving_error)?
    .canonical_bytes()
    .map_err(proving_error)?;
    let reserved = reservation.reservation().map_err(|error| {
        proving_error(format!("original native proving custody rejected: {error}"))
    })?;
    let admitted_reservation = verify_ordinary_lineage_reservation_v1(
        verifier,
        reserved,
        &preparation_bundle,
        &predecessor_original,
        &neutral_original,
        preparation.enrollment().app_credential(),
        preparation
            .original_approval_integrity_lease()
            .map(|lease| lease.as_ref()),
    )
    .map_err(proving_error)?;
    let [preparation_clock, intent_clock, admission_clock] = selection
        .retained_signed_clock_originals()
        .map_err(owner_error)?;
    let financial_control_original = selection
        .financial_control_original()
        .map_err(owner_error)?;
    let outgoing_original = KagemushaOrdinaryCashOutgoingOriginalV1::from_public_parts(
        preparation.normalized_guard_statement().clone(),
        preparation.transition_statement().clone(),
        outgoing,
        selection.candidate().prepared_record().clone(),
        selection.terminal_intent().clone(),
        *selection.terminal_record(),
        selection.enrollment().app_credential().original().to_vec(),
        selection.original().to_vec(),
        selection
            .original_approval_integrity_lease()
            .map(|lease| lease.original().to_vec()),
        selection
            .enrollment()
            .certificate()
            .canonical_bytes()
            .map_err(proving_error)?,
        financial_control_original.clone(),
        admission_clock,
        whole.commit_wrapper_original().to_vec(),
    )
    .map_err(proving_error)?
    .canonical_bytes()
    .map_err(proving_error)?;
    let service_original = KagemushaOrdinaryLineageCommitProofBundleV1::from_public_parts(
        preparation_bundle,
        predecessor_original,
        neutral_original,
        outgoing_original.clone(),
        terminal_guard.original().to_vec(),
        whole.inner_terminal_original().to_vec(),
        preparation_clock,
        intent_clock,
        whole.wrapper_history_fold_originals().map(<[u8]>::to_vec),
    )
    .map_err(proving_error)?
    .canonical_bytes()
    .map_err(proving_error)?;
    // The receiver frame is incomplete until the separately governed immutable DATA receipt
    // exists. This bounds the actual pre-receipt operands; the Native effect owner must call
    // require_assembled_frames again on the full completed receiver frame before exposure.
    require_pre_receipt_capacity(
        service_original.len(),
        outgoing_original.len(),
        &budget,
        neutral.reserved_outbox_bytes,
    )?;
    let before = selection.selected_predecessor_state();
    let transition_nullifier = kagemusha_ordinary_transition_nullifier_v1(
        before.state_commitment,
        before.secure_index,
        before.hardware_epoch.epoch_id,
        *before.lane.network_id.as_bytes(),
        before.lane.device_lane_id,
        before.liability_pool_id,
    )
    .map_err(proving_error)?;
    let commit = KagemushaOrdinaryLineageCommitV1 {
        reservation: reserved.clone(),
        transition_nullifier,
        purpose1_approval_original_sha256: Sha256::digest(selection.original()).into(),
        terminal_record_original_sha256: Sha256::digest(
            norito::encode_canonical(selection.terminal_record())
                .map_err(|e| proving_error(e.to_string()))?,
        )
        .into(),
        terminal_proofs_original_sha256: Sha256::digest(whole.inner_terminal_original()).into(),
        wrapper_proofs_original_sha256: Sha256::digest(whole.commit_wrapper_original()).into(),
        outgoing_original_sha256: Sha256::digest(&outgoing_original).into(),
        purpose1_financial_control_original_sha256: Sha256::digest(&financial_control_original)
            .into(),
        purpose1_clock_context_original_sha256: Sha256::digest(
            norito::encode_canonical(selection.admission_clock_context())
                .map_err(|e| proving_error(e.to_string()))?,
        )
        .into(),
    };
    commit.validate_shape().map_err(proving_error)?;
    let mut entered = false;
    let mut admitted = None;
    selection
        .with_retained_verified_signed_clock_originals(&mut |clocks| {
            if entered {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            entered = true;
            admitted = Some(verify_ordinary_lineage_commit_v1(
                verifier,
                &commit,
                &service_original,
                &admitted_reservation,
                selection.enrollment().app_credential(),
                selection
                    .original_approval_integrity_lease()
                    .map(|lease| lease.as_ref()),
                clocks,
            ));
            Ok(())
        })
        .map_err(owner_error)?;
    let proof = admitted
        .ok_or_else(|| proving_error("the actual retained clock capabilities were not lent"))?
        .map_err(proving_error)?;
    whole
        .recheck_terminal_selection(selection, terminal_guard)
        .map_err(owner_error)?;
    selection
        .recheck_lineage_reservation(reservation)
        .map_err(owner_error)?;
    selection
        .recheck_selected_originals_and_current_custody()
        .map_err(owner_error)?;
    let selected_successor_state = selection.selected_successor_state();
    if selected_successor_state != selection.candidate().successor_state() {
        return Err(proving_error(
            "the actual selected successor differs from the admitted candidate",
        ));
    }
    require_successor_checkpoint_join(
        selected_successor_state.state_commitment,
        selected_successor_state.logical_sequence,
        &candidate_original,
        &proof.commit().reservation.successor,
    )
    .map_err(proving_error)?;
    Ok(GeneratedOrdinaryCashCommitOriginalsV1 {
        proof,
        selected_successor_state: selected_successor_state.clone(),
        successor_public_state_original: candidate_original,
        successor_private_checkpoint_original: selection
            .candidate()
            .private_checkpoint_original()
            .to_vec(),
        successor_public_inputs: selection.candidate().public_inputs().clone(),
        private_service_original: service_original,
        pre_receipt_outgoing_original: outgoing_original,
    })
}

fn require_selected_verifier_release(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    let installed = verifier.monetary_release().map_err(proving_error)?;
    let selected = selection.authenticated_release().map_err(owner_error)?;
    if installed.release_id() != selected.release_id()
        || installed.attestation_digest() != selected.attestation_digest()
        || installed.authority_policy_digest() != selected.authority_policy_digest()
        || installed.native_profile_digest() != selected.native_profile_digest()
        || installed.vk_set_digest() != selected.vk_set_digest()
    {
        return Err(proving_error(
            "recovery verifier release differs from actual selected Native release",
        ));
    }
    Ok(())
}

fn actual_outgoing_originals(
    selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
) -> Result<KagemushaOrdinaryLineageOutgoingOriginalsV1, KagemushaArtifactGenerationErrorV1> {
    match (
        selection.send_transport_originals(),
        selection.redeem_transport_originals(),
    ) {
        (Some((request, output, encrypted_credit, _, _)), None) => {
            Ok(KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
                request: Box::new(request.clone()),
                output: *output,
                encrypted_credit: encrypted_credit.to_vec(),
                preparation_clock: *selection.preparation_clock_context(),
            })
        }
        (None, Some((output, beneficiary, manifest_original))) => {
            Ok(KagemushaOrdinaryLineageOutgoingOriginalsV1::Redeem {
                output: *output,
                beneficiary: beneficiary.clone(),
                manifest_original: manifest_original.to_vec(),
                preparation_clock: *selection.preparation_clock_context(),
            })
        }
        _ => Err(proving_error(
            "the actual Native outgoing originals are absent or aliased",
        )),
    }
}

fn require_pre_receipt_capacity(
    service: usize,
    outgoing: usize,
    budget: &crate::kagemusha_v1_recursion::KagemushaOrdinaryCashCarrierBudgetV1,
    actual_reserved: u32,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    budget
        .require_reserved_bytes(actual_reserved)
        .map_err(proving_error)?;
    require_frame_sizes(
        service,
        outgoing,
        budget.private_service_max_bytes(),
        budget.compact_receiver_max_bytes(),
        actual_reserved,
    )
}

fn require_frame_sizes(
    service: usize,
    outgoing: usize,
    service_max: u32,
    receiver_max: u32,
    actual_reserved: u32,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if service == 0
        || outgoing == 0
        || service > service_max as usize
        || outgoing > receiver_max as usize
        || service
            .checked_add(outgoing)
            .is_none_or(|sum| sum > actual_reserved as usize)
    {
        return Err(proving_error(
            "the actual pre-receipt complete frames exceed Native capacity",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::require_frame_sizes;

    #[test]
    fn actual_service_and_receiver_frames_both_fit_the_same_native_slot() {
        assert!(
            require_frame_sizes(53_000_000, 17_000_000, 54_000_000, 34_000_000, 88_000_000).is_ok()
        );
        assert!(
            require_frame_sizes(53_000_000, 17_000_000, 54_000_000, 34_000_000, 192 * 1024)
                .is_err()
        );
        assert!(
            require_frame_sizes(53_000_000, 17_000_000, 54_000_000, 34_000_000, 69_999_999)
                .is_err()
        );
        assert!(
            require_frame_sizes(53_000_000, 17_000_000, 52_999_999, 34_000_000, 88_000_000)
                .is_err()
        );
        assert!(
            require_frame_sizes(53_000_000, 17_000_000, 54_000_000, 16_999_999, 88_000_000)
                .is_err()
        );
    }

    #[test]
    fn absent_or_overflowing_pre_receipt_frames_are_never_sized_as_complete() {
        assert!(require_frame_sizes(0, 1, 16, 16, 32).is_err());
        assert!(require_frame_sizes(1, 0, 16, 16, 32).is_err());
        assert!(require_frame_sizes(usize::MAX, 1, u32::MAX, u32::MAX, u32::MAX).is_err());
    }
}

fn require_successor_checkpoint_join(
    state_commitment: DigestV1,
    logical_sequence: u128,
    public_checkpoint_original: &[u8],
    committed_head: &iroha_data_model::kagemusha::KagemushaOrdinaryFinancialHeadV1,
) -> Result<(), String> {
    if public_checkpoint_original.is_empty()
        || committed_head.state_commitment != state_commitment
        || committed_head.logical_sequence != logical_sequence
        || committed_head.state_original_sha256
            != <DigestV1>::from(Sha256::digest(public_checkpoint_original))
    {
        return Err("ordinary selected successor/public checkpoint/committed Head differ".into());
    }
    Ok(())
}
#[cfg(test)]
mod selected_successor_tests {
    use super::*;
    #[test]
    fn successor_join_preserves_full_u128_sequence_and_complete_public_checkpoint_sha() {
        // Data-only selector regression. These values create no State proof or Native owner.
        let original =
            norito::encode_canonical(&(1_u16, [23_u8; 32], (1_u128 << 112) + 7)).unwrap();
        let head = iroha_data_model::kagemusha::KagemushaOrdinaryFinancialHeadV1 {
            state_commitment: [23; 32],
            logical_sequence: (1_u128 << 112) + 7,
            state_original_sha256: Sha256::digest(&original).into(),
        };
        assert!(
            require_successor_checkpoint_join(
                head.state_commitment,
                head.logical_sequence,
                &original,
                &head
            )
            .is_ok()
        );
        let mut substituted = head;
        substituted.logical_sequence = 7;
        assert!(
            require_successor_checkpoint_join(
                head.state_commitment,
                head.logical_sequence,
                &original,
                &substituted
            )
            .is_err()
        );
        substituted = head;
        substituted.state_commitment[0] ^= 1;
        assert!(
            require_successor_checkpoint_join(
                head.state_commitment,
                head.logical_sequence,
                &original,
                &substituted
            )
            .is_err()
        );
        let mut changed = original;
        changed.push(0);
        assert!(
            require_successor_checkpoint_join(
                head.state_commitment,
                head.logical_sequence,
                &changed,
                &head
            )
            .is_err()
        );
    }
}

fn owner_error(error: KagemushaStateErrorV1) -> KagemushaArtifactGenerationErrorV1 {
    proving_error(format!("original native proving custody rejected: {error}"))
}
fn proving_error(reason: impl Into<String>) -> KagemushaArtifactGenerationErrorV1 {
    KagemushaArtifactGenerationErrorV1::CircuitBuild(reason.into())
}
