//! Actual Main-owned outgoing prepared operands and paired candidate retention before global Reserve.
//! Replay re-admits every exact original under captured W2; no decoded row creates proof authority.
use super::*;
use crate::kagemusha_v1_recursion::{
    GeneratedOrdinaryCashReservationOriginalsV1, KagemushaArtifactByteResolverV1,
    KagemushaAuthenticatedOrdinaryCashCandidateV1,
    KagemushaAuthenticatedOrdinaryPreparationGuardV1, KagemushaOrdinaryLineageStateOriginalV1,
    KagemushaProductionProverV1, KagemushaRecursiveVerifierProfileV1,
    readmit_retained_ordinary_outgoing_candidate_v1, verify_ordinary_preparation_guard_v1,
};
use zeroize::Zeroize as _;
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::OrdinaryOutgoingReservationCandidateV1"
)]
pub(super) struct OutgoingReservationCandidateOriginals {
    operation: DigestV1,
    nonce: DigestV1,
    guard_original: Vec<u8>,
    public_state_original: Vec<u8>,
    private_checkpoint_original: Vec<u8>,
}
impl core::fmt::Debug for OutgoingReservationCandidateOriginals {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OutgoingReservationCandidateOriginals")
            .field("operation", &self.operation)
            .field("private_originals", &"redacted")
            .finish()
    }
}
impl Drop for OutgoingReservationCandidateOriginals {
    fn drop(&mut self) {
        self.private_checkpoint_original.zeroize();
    }
}
pub(super) struct ReAdmittedOutgoing {
    pub(super) candidate: KagemushaAuthenticatedOrdinaryCashCandidateV1,
    pub(super) guard: KagemushaAuthenticatedOrdinaryPreparationGuardV1,
}
impl OutgoingReservationCandidateOriginals {
    pub(super) fn readmit(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
    ) -> Result<ReAdmittedOutgoing, KagemushaStateErrorV1> {
        owner.recheck_proving_history(ProvingHistoryOperation::OutgoingApproval)?;
        let selection = owner.captured_preparation()?;
        let maximum = crate::kagemusha_v1_recursion::KagemushaRecursiveStateCheckpointV1::maximum_encoded_bytes(&owner.verifier).map_err(material)?;
        require_outgoing_candidate_lengths(
            [
                self.guard_original.len(),
                self.public_state_original.len(),
                self.private_checkpoint_original.len(),
            ],
            maximum,
        )?;
        if self.operation != selection.challenge().operation_id
            || self.nonce != selection.challenge().nonce
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let guard = verify_ordinary_preparation_guard_v1(&selection, &self.guard_original)?;
        let (prepared, _, _) = selection.retained_outgoing_proof_operands(&guard)?;
        let candidate = readmit_retained_ordinary_outgoing_candidate_v1(
            &selection,
            &guard,
            prepared,
            &self.public_state_original,
            &self.private_checkpoint_original,
        )?;
        candidate.recheck_preparation_selection(&selection, &guard)?;
        Ok(ReAdmittedOutgoing { candidate, guard })
    }
}
fn require_outgoing_candidate_lengths(
    lengths: [usize; 3],
    checkpoint: usize,
) -> Result<(), KagemushaStateErrorV1> {
    let maxima = [KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1, 32 * 1024, checkpoint];
    if checkpoint == 0
        || lengths
            .into_iter()
            .zip(maxima)
            .any(|(n, max)| n == 0 || n > max)
    {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    }
    Ok(())
}
impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Full-operation physical reservation precedes W2. Every later complete row fits this finite
    /// suffix and the same released payload ceiling, or construction is refused before platform use.
    pub(super) fn outgoing_completion_slot_bytes(&self) -> Result<u32, KagemushaStateErrorV1> {
        outgoing_slot_bytes(
            self.carrier_budget.required_outbox_slot_bytes(),
            self.maximum_record_payload_bytes,
        )
    }
    pub(super) fn require_outgoing_rows(&self, rows: u64) -> Result<(), KagemushaStateErrorV1> {
        if self
            .prefix
            .sequence
            .checked_add(rows)
            .is_none_or(|n| n > MAX_ROWS)
        {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        Ok(())
    }
    fn require_outgoing_record(&self, record: &Record) -> Result<(), KagemushaStateErrorV1> {
        let bytes = u64::try_from(norito::canonical_frame_len(record).map_err(material)?)
            .map_err(material)?;
        if bytes == 0 || bytes > self.maximum_record_payload_bytes {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.reservation.reserved_outbox_bytes != self.outgoing_completion_slot_bytes()? {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.require_outbox_capacity_for_new_slot()
    }
    pub(super) fn retain_outgoing_proof_operands(
        &mut self,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let selection = self.captured_preparation()?;
        if let Some(held) = &self.outgoing_proof_operands {
            return held.recheck(&selection, guard);
        }
        if self.outgoing_reservation_candidate.is_some()
            || self.terminal.as_ref().is_none_or(|t| t.has_pending())
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let originals = OutgoingProofOperandOriginals::create(&selection, guard)?;
        self.require_outgoing_rows(2 + 4 + 3)?;
        self.require_outgoing_record(&Record::OutgoingProofOperands(originals.clone()))?;
        self.persist(&Record::OutgoingProofOperands(originals.clone()))?;
        self.outgoing_proof_operands = Some(originals);
        let selection = self.captured_preparation()?;
        self.outgoing_proof_operands
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .recheck(&selection, guard)
    }
    pub(super) fn replay_outgoing_proof_operands(
        &mut self,
        originals: OutgoingProofOperandOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.outgoing_proof_operands.is_some() || self.outgoing_reservation_candidate.is_some() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let selection = self.captured_preparation()?;
        let guard = verify_ordinary_preparation_guard_v1(&selection, originals.guard_original())?;
        originals.recheck(&selection, &guard)?;
        self.require_outgoing_record(&Record::OutgoingProofOperands(originals.clone()))?;
        self.outgoing_proof_operands = Some(originals);
        Ok(())
    }
    pub(super) fn retained_outgoing_guard(
        &self,
    ) -> Result<KagemushaAuthenticatedOrdinaryPreparationGuardV1, KagemushaStateErrorV1> {
        let selection = self.captured_preparation()?;
        let originals = self
            .outgoing_proof_operands
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let guard = verify_ordinary_preparation_guard_v1(&selection, originals.guard_original())?;
        originals.recheck(&selection, &guard)?;
        Ok(guard)
    }
    pub(super) fn retain_outgoing_reservation_candidate(
        &mut self,
        candidate: KagemushaAuthenticatedOrdinaryCashCandidateV1,
        guard: KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let selection = self.captured_preparation()?;
        candidate.recheck_preparation_selection(&selection, &guard)?;
        selection.retained_outgoing_proof_operands(&guard)?;
        let originals = OutgoingReservationCandidateOriginals {
            operation: selection.challenge().operation_id,
            nonce: selection.challenge().nonce,
            guard_original: guard.original().to_vec(),
            public_state_original:
                KagemushaOrdinaryLineageStateOriginalV1::from_admitted_candidate(&candidate)
                    .map_err(material)?
                    .canonical_bytes()
                    .map_err(material)?,
            private_checkpoint_original: candidate.private_checkpoint_original().to_vec(),
        };
        originals.readmit(self)?;
        let digest = Sha256::digest(&originals.public_state_original).into();
        if let Some(held) = &self.outgoing_reservation_candidate {
            if held != &originals {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            return Ok(digest);
        }
        self.require_outgoing_rows(1 + 4 + 3)?;
        self.require_outgoing_record(&Record::OutgoingReservationCandidate(originals.clone()))?;
        self.persist(&Record::OutgoingReservationCandidate(originals.clone()))?;
        self.outgoing_reservation_candidate = Some(originals);
        self.require_current_financial_control()?;
        Ok(digest)
    }
    pub(super) fn replay_outgoing_reservation_candidate(
        &mut self,
        originals: OutgoingReservationCandidateOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.outgoing_reservation_candidate.is_some() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        originals.readmit(self)?;
        self.require_outgoing_record(&Record::OutgoingReservationCandidate(originals.clone()))?;
        self.outgoing_reservation_candidate = Some(originals);
        Ok(())
    }
    pub(super) fn retained_outgoing_reservation<R: KagemushaArtifactByteResolverV1>(
        &self,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<
        (
            ReAdmittedOutgoing,
            GeneratedOrdinaryCashReservationOriginalsV1,
        ),
        KagemushaStateErrorV1,
    > {
        self.require_current_financial_control()?;
        let row = self
            .outgoing_reservation_candidate
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let admitted = row.readmit(self)?;
        let selection = self.captured_preparation()?;
        let prover = KagemushaProductionProverV1::load_ordinary_cash(&selection, profile, resolver)
            .map_err(material)?;
        let reservation = prover
            .assemble_ordinary_cash_reservation(&selection, &admitted.guard, &admitted.candidate)
            .map_err(material)?;
        self.require_current_financial_control()?;
        Ok((admitted, reservation))
    }
}
fn outgoing_slot_bytes(carrier: u32, maximum_payload: u64) -> Result<u32, KagemushaStateErrorV1> {
    if carrier == 0 || maximum_payload == 0 {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    }
    // Complete outgoing Main chronology: intent, Native credit, selection, W2 fence/capture/ack,
    // sealed operands, candidate, W1 selection/fence/capture/ack, Commit/Advance/Ack. Each full frame
    // charges the released maximum plus exact framing allowance; committed histories retain the slot.
    let suffix = maximum_payload
        .checked_add(256)
        .and_then(|n| n.checked_mul(15))
        .and_then(|n| n.checked_add(u64::from(carrier)))
        .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
    u32::try_from(suffix).map_err(material)
}
impl KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_> {
    pub(crate) fn retained_outgoing_proof_operands(
        &self,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    ) -> Result<
        (
            &iroha_data_model::kagemusha::KagemushaOrdinaryPreparedOutgoingV1,
            &[u8],
            &[u8],
        ),
        KagemushaStateErrorV1,
    > {
        self.recheck_selected_originals_and_current_custody()?;
        let row = self
            .owner
            .outgoing_proof_operands
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        row.recheck(self, guard)?;
        Ok((
            row.prepared(),
            row.transition_stream(),
            row.recovery_stream(),
        ))
    }
    pub(crate) fn with_retained_predecessor_checkpoint(
        &self,
        consume: &mut dyn for<'a> FnMut(
            &'a crate::kagemusha_v1_recursion::KagemushaGeneratedRecursiveStateProofV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        self.owner.with_retained_predecessor_checkpoint(consume)?;
        self.recheck_selected_originals_and_current_custody()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outgoing_candidate_binds_all_bounded_complete_roles() {
        let maxima = [KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1, 32 * 1024, 777];
        assert!(require_outgoing_candidate_lengths(maxima, 777).is_ok());
        for role in 0..3 {
            let mut x = maxima;
            x[role] = 0;
            assert!(require_outgoing_candidate_lengths(x, 777).is_err());
            let mut x = maxima;
            x[role] += 1;
            assert!(require_outgoing_candidate_lengths(x, 777).is_err());
        }
        assert!(require_outgoing_candidate_lengths(maxima, 0).is_err());
    }
    #[test]
    fn outgoing_slot_charges_the_entire_private_wal_suffix_before_w2() {
        assert_eq!(outgoing_slot_bytes(10, 100).unwrap(), 10 + 15 * 356);
        assert!(outgoing_slot_bytes(0, 100).is_err());
        assert!(outgoing_slot_bytes(10, 0).is_err());
        assert!(outgoing_slot_bytes(10, u64::MAX).is_err());
        assert!(outgoing_slot_bytes(u32::MAX, 100).is_err());
    }
}
