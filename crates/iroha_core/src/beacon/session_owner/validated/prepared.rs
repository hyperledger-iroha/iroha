//! Preclaimed verification and shell custody consuming one exact decoded graph.

use super::*;
use iroha_allocation::ReservedChargedShared;
use iroha_data_model::consensus::GlobalThresholdBeaconDkgSessionV1;

/// Existing signature/transcript verifier and exact shared shell prepared before private work.
///
/// This owner is move-only. Its constructor is exposed through the local prepared
/// seat so the message bound derives from canonical row shapes, not an arbitrary
/// allowance. The final original-pool graph is moved unchanged into the shell.
pub struct PreparedGlobalThresholdBeaconSessionVerificationV1 {
    session: GlobalThresholdBeaconDkgSessionV1,
    budget: AllocationBudget,
    workspace: Workspace,
    shell: ReservedChargedShared<RetainedPayload<Payload>>,
}
impl PreparedGlobalThresholdBeaconSessionVerificationV1 {
    pub(in crate::beacon) fn new(
        session: GlobalThresholdBeaconDkgSessionV1,
        preimage: usize,
        budget: &AllocationBudget,
    ) -> Result<Self, GlobalThresholdBeaconSessionError> {
        beacon::validate_dkg_session(&session)?;
        let dealers = usize::from(session.committee_size);
        let scratch_bytes = Layout::array::<u8>(preimage)
            .map_err(|_| AllocationRefusal::DemandOverflow)?
            .size()
            .checked_add(
                Layout::array::<ValidatedDealerCommitment<BeaconPurpose>>(dealers)
                    .map_err(|_| AllocationRefusal::DemandOverflow)?
                    .size(),
            )
            .ok_or(AllocationRefusal::DemandOverflow)?;
        let total = scratch_bytes
            .checked_add(ChargedShared::<RetainedPayload<Payload>>::allocation_layout().size())
            .ok_or(AllocationRefusal::DemandOverflow)?;
        let mut reservation = budget.try_reserve_bytes(total)?;
        let shell = ChargedShared::reserve_from(&mut reservation)?;
        let workspace = Workspace::new(
            ScratchDemand {
                bytes: scratch_bytes,
                preimage,
                dealers,
            },
            &mut reservation,
        )?;
        if reservation.remaining_bytes() != 0 {
            return Err(GlobalThresholdBeaconSessionError::PlanChanged);
        }
        Ok(Self {
            session,
            budget: budget.clone(),
            workspace,
            shell,
        })
    }

    /// Exact original scratch backing retired only after successful session sealing.
    #[cfg(test)]
    pub(in crate::beacon) fn retired_scratch_layouts(&self) -> [Layout; 2] {
        [
            Layout::array::<u8>(self.workspace.preimage.capacity())
                .expect("the original preimage backing has a valid layout"),
            Layout::array::<ValidatedDealerCommitment<BeaconPurpose>>(
                self.workspace.dealers.capacity(),
            )
            .expect("the original dealer scratch backing has a valid layout"),
        ]
    }

    /// Whether this prepared verifier's exact original pool is the supplied pool.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.workspace.preimage.belongs_to(budget) && self.workspace.dealers.belongs_to(budget)
    }

    /// Verify and consume the original retained graph without copying or late allocation.
    ///
    /// # Errors
    /// Returns this exact prepared owner and the unchanged source graph together
    /// with the original error. Scratch is reset on refusal; no shared session is
    /// published and no input allocation is dropped, repriced or re-admitted.
    pub fn seal(
        mut self,
        source: RetainedPayload<GlobalThresholdBeaconKeySessionV1>,
        expected: &GlobalThresholdBeaconSessionBindingV1,
    ) -> Result<
        ValidatedGlobalThresholdBeaconSessionV1,
        (
            Self,
            RetainedPayload<GlobalThresholdBeaconKeySessionV1>,
            GlobalThresholdBeaconSessionError,
        ),
    > {
        let checked = (|| -> Result<
            (AdaptiveThresholdBlsPublicTranscript<BeaconPurpose>, usize),
            GlobalThresholdBeaconSessionError,
        > {
            if !source.belongs_to(&self.budget) {
                return Err(GlobalThresholdBeaconSessionError::ForeignReservation);
            }
            let record = source.get();
            if record.adaptive_dkg.session != self.session {
                return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
            }
            validate_binding(record, expected)?;
            let demand = Workspace::demand(record)?;
            if demand.preimage > self.workspace.preimage.capacity()
                || demand.dealers > self.workspace.dealers.capacity()
            {
                return Err(GlobalThresholdBeaconSessionError::PlanChanged);
            }
            let retained_bytes = source
                .allocation_bytes()
                .and_then(|bytes| {
                    bytes.checked_add(
                        ChargedShared::<RetainedPayload<Payload>>::allocation_layout().size(),
                    )
                })
                .ok_or(AllocationRefusal::DemandOverflow)?;
            self.workspace.dealers.truncate(0);
            let transcript = verify_prepaid(record, &mut self.workspace)?;
            Ok((transcript, retained_bytes))
        })();
        let (transcript, retained_bytes) = match checked {
            Ok(value) => value,
            Err(error) => {
                self.workspace.dealers.truncate(0);
                return Err((self, source, error));
            }
        };
        // SAFETY: the entire original canonical graph moves unchanged into the
        // private immutable payload. The transcript is initialized inline storage;
        // the same original-pool shared shell was physically allocated preclaim.
        #[allow(unsafe_code)]
        let payload = unsafe {
            source.map_payload(|record| Payload {
                record,
                transcript,
                retained_bytes,
            })
        };
        let owner = self.shell.initialize(payload);
        Ok(ValidatedGlobalThresholdBeaconSessionV1 { owner })
    }
}

#[cfg(test)]
mod tests;
