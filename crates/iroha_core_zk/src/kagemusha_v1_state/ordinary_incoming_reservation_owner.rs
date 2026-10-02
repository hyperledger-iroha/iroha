//! Actual Main-owned pre-Reserve State/Guard/checkpoint and portable proof retention.
//! This row precedes global dispatch. Its data is independently re-admitted under the same
//! captured W2 on every recovery; no decoded row or profile creates an incoming capability.
use super::*;
use crate::kagemusha_v1_recursion::{
    KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1,
    KagemushaAuthenticatedOrdinaryIncomingCandidateV1,
    KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1, KagemushaRecursiveStateCheckpointV1,
    KagemushaVerifiedOrdinaryIncomingReservationProofV1, assemble_ordinary_incoming_reservation_v1,
    verify_ordinary_incoming_candidate_v1, verify_ordinary_incoming_preparation_guard_v1,
};
use zeroize::Zeroize as _;

/// Complete exact originals retained before any global account-sign/transport fence.
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::OrdinaryIncomingReservationCandidateV1"
)]
pub(super) struct IncomingReservationCandidateOriginals {
    operation: DigestV1,
    nonce: DigestV1,
    guard_original: Vec<u8>,
    public_state_original: Vec<u8>,
    private_checkpoint_original: Vec<u8>,
    reservation_bundle_original: Vec<u8>,
}
impl core::fmt::Debug for IncomingReservationCandidateOriginals {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IncomingReservationCandidateOriginals")
            .field("operation", &self.operation)
            .field("private_originals", &"redacted")
            .finish()
    }
}
impl Drop for IncomingReservationCandidateOriginals {
    fn drop(&mut self) {
        self.private_checkpoint_original.zeroize();
    }
}
struct ReAdmitted {
    candidate: KagemushaAuthenticatedOrdinaryIncomingCandidateV1,
    guard: KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    proof: KagemushaVerifiedOrdinaryIncomingReservationProofV1,
}

fn require_lengths(
    lengths: [usize; 4],
    maximum_checkpoint: usize,
) -> Result<(), KagemushaStateErrorV1> {
    let maxima = [
        KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1,
        32 * 1024,
        maximum_checkpoint,
        KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1,
    ];
    if lengths
        .into_iter()
        .zip(maxima)
        .any(|(n, max)| n == 0 || n > max)
    {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    }
    Ok(())
}

/// Conservative finite whole-row bound from actual released checkpoint geometry and sole
/// complete public protocol maxima. It grants no authority and is checked before encoding.
pub(super) fn maximum_candidate_record_bytes(
    maximum_checkpoint: usize,
) -> Result<u64, KagemushaStateErrorV1> {
    if maximum_checkpoint == 0 {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    }
    [
        KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1,
        32 * 1024,
        maximum_checkpoint,
        KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1,
    ]
    .into_iter()
    .try_fold(128 * 1024u64, |sum, n| {
        sum.checked_add(u64::try_from(n).map_err(material)?)
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
    })
}

impl IncomingReservationCandidateOriginals {
    fn readmit(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
    ) -> Result<ReAdmitted, KagemushaStateErrorV1> {
        owner.recheck_proving_history(ProvingHistoryOperation::IncomingApproval)?;
        let maximum = KagemushaRecursiveStateCheckpointV1::maximum_encoded_bytes(&owner.verifier)
            .map_err(material)?;
        require_lengths(
            [
                self.guard_original.len(),
                self.public_state_original.len(),
                self.private_checkpoint_original.len(),
                self.reservation_bundle_original.len(),
            ],
            maximum,
        )?;
        if maximum_candidate_record_bytes(maximum)? > owner.maximum_record_payload_bytes {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        let selection = owner.captured_incoming_approval()?;
        // Portable Receive service custody is a distinct incomplete relation; it cannot be
        // promoted through this existing Mint assembler or an offered decoded source.
        if selection.transition_statement()?.kind != KagemushaTransitionKindV1::MintFold
            || self.operation != selection.challenge()?.operation_id
            || self.nonce != selection.challenge()?.nonce
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let guard =
            verify_ordinary_incoming_preparation_guard_v1(&selection, &self.guard_original)?;
        let candidate = verify_ordinary_incoming_candidate_v1(
            &owner.verifier,
            &selection,
            &self.public_state_original,
            &guard,
            &self.private_checkpoint_original,
        )?;
        let proof = assemble_ordinary_incoming_reservation_v1(&selection, &candidate, &guard)
            .map_err(material)?;
        if proof.original() != self.reservation_bundle_original {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        candidate.recheck_incoming_selection(&selection, &guard)?;
        Ok(ReAdmitted {
            candidate,
            guard,
            proof,
        })
    }
    pub(super) fn capacity_charge(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
    ) -> Result<u64, KagemushaStateErrorV1> {
        let bytes = u64::try_from(
            norito::canonical_frame_len(&Record::IncomingReservationCandidate(self.clone()))
                .map_err(material)?,
        )
        .map_err(material)?;
        if bytes == 0 || bytes > owner.maximum_record_payload_bytes {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        bytes
            .checked_add(256)
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Called only after actual Native proof production, never with managed originals/caps.
    pub(super) fn retain_incoming_reservation_candidate(
        &mut self,
        candidate: KagemushaAuthenticatedOrdinaryIncomingCandidateV1,
        guard: KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let selection = self.captured_incoming_approval()?;
        candidate.recheck_incoming_selection(&selection, &guard)?;
        let proof = assemble_ordinary_incoming_reservation_v1(&selection, &candidate, &guard)
            .map_err(material)?;
        let originals = IncomingReservationCandidateOriginals {
            operation: selection.challenge()?.operation_id,
            nonce: selection.challenge()?.nonce,
            guard_original: guard.original().to_vec(),
            public_state_original: candidate.public_state_original().to_vec(),
            private_checkpoint_original: candidate.private_checkpoint_original().to_vec(),
            reservation_bundle_original: proof.original().to_vec(),
        };
        originals.readmit(self)?;
        let digest = Sha256::digest(&originals.reservation_bundle_original).into();
        if let Some(retained) = &self.incoming_reservation_candidate {
            if retained != &originals {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            return Ok(digest);
        }
        if self
            .pending_incoming
            .as_ref()
            .is_none_or(|p| p.terminal.is_some())
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        // Remaining Main suffix: this row, four W1 rows, Prepared/Advance/Ack.
        self.require_incoming_rows(1 + 4 + 3)?;
        self.require_incoming_candidate_capacity(&originals)?;
        self.persist(&Record::IncomingReservationCandidate(originals.clone()))?;
        self.incoming_reservation_candidate = Some(originals);
        self.require_current_financial_control()?;
        Ok(digest)
    }
    pub(super) fn replay_incoming_reservation_candidate(
        &mut self,
        originals: IncomingReservationCandidateOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.incoming_reservation_candidate.is_some()
            || self
                .pending_incoming
                .as_ref()
                .is_none_or(|p| p.terminal.is_some())
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        originals.readmit(self)?;
        self.require_incoming_candidate_capacity(&originals)?;
        self.incoming_reservation_candidate = Some(originals);
        Ok(())
    }
    fn require_incoming_candidate_capacity(
        &self,
        originals: &IncomingReservationCandidateOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        let pending = self
            .pending_incoming
            .as_ref()
            .and_then(|p| p.approval.as_ref())
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let actual = originals
            .capacity_charge(self)?
            .checked_add(pending.captured_suffix_charge_bytes()?)
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        if actual > pending.completion_capacity_bytes()? {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        self.recheck_receiver_request_storage()
    }
    /// Reserve exact global incoming request only after complete candidate fsync and re-admission.
    /// Account signature and HTTP dispatch remain separately fenced by the actual session owner.
    pub fn reserve_proven_incoming_mint(&mut self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let originals = self
            .incoming_reservation_candidate
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let admitted = originals.readmit(self)?;
        self.require_incoming_candidate_capacity(originals)?;
        let financial = self.publication.cash_financial();
        let current = self.control.loan(financial).map_err(material)?;
        self.lineage_cas
            .reserve_incoming_transition(financial, &current, &admitted.proof)
            .map_err(material)
    }
    /// Exact full portable original; it is transport data, never a decoded money grant.
    pub fn incoming_reservation_proof_original(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let row = self
            .incoming_reservation_candidate
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let admitted = row.readmit(self)?;
        Ok(admitted.proof.original().to_vec())
    }
    /// W1 is selected only after an actual independently authenticated acknowledged Reserve.
    pub fn select_retained_incoming_mint_terminal(
        &mut self,
        reserve_request_original_sha256: DigestV1,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let originals = self
            .incoming_reservation_candidate
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let admitted = originals.readmit(self)?;
        let challenge = self.select_incoming_terminal(
            admitted.candidate,
            admitted.guard,
            reserve_request_original_sha256,
        )?;
        norito::encode_canonical(&challenge).map_err(material)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_carrier_all_four_roles_are_bounded() {
        let checkpoint = 777usize;
        let maxima = [
            KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1,
            32 * 1024,
            checkpoint,
            KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1,
        ];
        assert!(require_lengths(maxima, checkpoint).is_ok());
        for role in 0..4 {
            let mut absent = maxima;
            absent[role] = 0;
            assert!(require_lengths(absent, checkpoint).is_err());
            let mut over = maxima;
            over[role] += 1;
            assert!(require_lengths(over, checkpoint).is_err());
        }
    }
    #[test]
    fn candidate_ceiling_is_finite_and_framed() {
        assert!(maximum_candidate_record_bytes(0).is_err());
        if usize::BITS == 64 {
            assert!(maximum_candidate_record_bytes(usize::MAX).is_err());
        }
        let a = maximum_candidate_record_bytes(10).unwrap();
        assert_eq!(maximum_candidate_record_bytes(11).unwrap(), a + 1);
        assert!(a > KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1 as u64);
        let released = cash_record_payload_limit(
            1,
            1,
            crate::kagemusha_v1_recursion::KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1
                as u64,
            10,
        )
        .unwrap();
        // Actual full49 contains full46 plus the genuine W1 clocks/Guard. The same finite
        // released ceiling must dominate all four variable roles in the pre-Reserve row.
        assert!(a <= released);
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Return only actual Native signed request/proof and a read-only acknowledged disposition.
    /// Existing Ed64 comes from the exact same AccountSigned WAL, never a fresh signature.
    /// # Errors
    /// Rejects an uncertain account fence, changed proof/request or ambiguous acknowledgements.
    pub fn sign_incoming_mint_reservation_transport(
        &mut self,
        sign: impl FnOnce(
            &KagemushaAuthenticatedOrdinaryLineageAccountSigningV1<'_>,
        ) -> Result<[u8; 64], KagemushaStateErrorV1>,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.reserve_proven_incoming_mint()?;
        let row = self
            .incoming_reservation_candidate
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let proof = row.readmit(self)?.proof;
        let financial = self.publication.cash_financial();
        let current = self.control.loan(financial).map_err(material)?;
        let acknowledged = self
            .lineage_cas
            .acknowledged_incoming_reservation_request(financial, &current, &proof)
            .map_err(material)?;
        let (status, original, signature) = if let Some((request, signature)) = acknowledged {
            (2, request, signature.to_vec())
        } else {
            let fields = self.sign_retained_lineage_request(sign)?;
            match fields.as_slice() {
                [request, signature] => (0, request.clone(), signature.clone()),
                _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
            }
        };
        let key: DigestV1 = Sha256::digest(&original).into();
        self.require_current_financial_control()?;
        Ok(vec![
            vec![status],
            original,
            signature,
            proof.original().to_vec(),
            key.to_vec(),
        ])
    }
}
