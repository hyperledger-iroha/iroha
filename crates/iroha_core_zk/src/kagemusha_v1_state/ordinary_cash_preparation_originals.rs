//! Exact pre-W1 data selected by a genuine captured W2 under the Main cash journal.
use super::*;
use crate::kagemusha_v1_recursion::KagemushaOrdinaryLineageOutgoingOriginalsV1;

impl KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_> {
    /// Return only the PUBLIC paired predecessor already retained by this actual owner.
    pub(crate) fn selected_predecessor_public_state_original(
        &self,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let original = self.owner.public_state_original.clone();
        if original.is_empty() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.recheck_selected_originals_and_current_custody()?;
        Ok(original)
    }
    /// Actual native-held physical reservation; caller integers cannot select this value.
    pub(crate) fn outbox_reservation_original(
        &self,
    ) -> Result<&KagemushaOutboxReservationV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let pending = self
            .owner
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        self.owner
            .carrier_budget
            .require_reserved_bytes(pending.reservation.reserved_outbox_bytes)
            .map_err(material)?;
        pending.reservation.validate().map_err(material)?;
        Ok(&pending.reservation)
    }
    /// Copy exact retained Send or Redeem transport after independently checking its originals.
    /// This is public proof data, never a secret or an independently constructed owner.
    pub(crate) fn outgoing_transport_originals(
        &self,
    ) -> Result<KagemushaOrdinaryLineageOutgoingOriginalsV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let pending = self
            .owner
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let selected = self.selected();
        let original = match (&pending.send_credit, &pending.redeem_credit) {
            (Some(send), None) => {
                send.originals.recheck_at_replay_position(
                    self.owner,
                    pending.operation,
                    &selected.successor,
                    send.receiver.as_ref(),
                    send.receiver_lease.as_deref(),
                )?;
                send.originals
                    .require_statement(&selected.statement, &selected.successor)?;
                KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
                    request: Box::new(send.originals.request()?),
                    output: *send.originals.output(),
                    encrypted_credit: send.originals.encrypted_credit().to_vec(),
                    preparation_clock: pending.preparation_clock,
                }
            }
            (None, Some((successor, redeem))) => {
                if successor != &selected.successor {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                redeem.recheck_original_data(self.owner, pending.operation, successor)?;
                redeem.require_statement(&selected.statement, successor)?;
                KagemushaOrdinaryLineageOutgoingOriginalsV1::Redeem {
                    output: *redeem.output(),
                    beneficiary: redeem.beneficiary().clone(),
                    manifest_original: redeem.manifest_original().to_vec(),
                    preparation_clock: pending.preparation_clock,
                }
            }
            _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
        };
        self.recheck_selected_originals_and_current_custody()?;
        Ok(original)
    }
}
