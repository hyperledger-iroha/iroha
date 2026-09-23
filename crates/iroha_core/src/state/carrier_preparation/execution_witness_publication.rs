//! Persist final witness proofs from the original validated execution custody.
//!
//! Staging and promotion use the existing bounded Kura owners before physical
//! State acquisition. Refusal retains every original journal and reservation;
//! partial durable work is completed by the same exact idempotent continuation.

use super::{super::DetachedCarrierComponents, DecisionBoundCarrierJournals};
use crate::kura::{KuraPublicationPreparationError, KuraWsvCheckpointReceipt};

/// Local durability failure, never a verdict on the decided proposal.
#[derive(Debug)]
pub(crate) enum CarrierExecutionWitnessPublicationError {
    /// Original Kura contention or storage repair prevents checkpoint admission.
    Kura(KuraPublicationPreparationError),
    /// Exact checkpoint, finality or original Kura authentication failed.
    Checkpoint(crate::kura::Error),
    /// Actual witness derivation, staging or exact finality promotion failed.
    Witness(crate::kura::Error),
}

impl<Admission>
    DecisionBoundCarrierJournals<Admission, DetachedCarrierComponents, KuraWsvCheckpointReceipt>
{
    /// Materialize the original witness's finalized proof projection.
    ///
    /// Installation admission and original target-Kura identity must precede
    /// this continuation. It retains no physical guard between the checkpoint
    /// probe and the existing staging/promotion APIs, which acquire their own
    /// locks. No State is read, reexecuted or reconstructed, and no substitute
    /// witness or casting bindings are accepted from the caller.
    ///
    /// The final publisher must reauthenticate the resulting final sidecar
    /// under its actual Kura lease before acquiring any State writer.
    ///
    /// TODO: Avoid regenerating a matching stage on later physical Busy retries
    /// once Kura exposes an authenticated, typed final-proof absence probe.
    /// Until then, retain the exact existing stage/promote retry protocol; a
    /// corrupt or foreign final proof must never be classified as missing.
    pub(crate) fn publish_execution_witness(
        &mut self,
    ) -> Result<(), CarrierExecutionWitnessPublicationError> {
        let lease = self
            .journals
            .kura
            .try_publication_lease()
            .map_err(CarrierExecutionWitnessPublicationError::Kura)?;
        lease
            .reauthenticate_checkpoint(
                &self.checkpoint,
                self.finality.artifact(),
                self.journals.checkpoint,
            )
            .map_err(CarrierExecutionWitnessPublicationError::Checkpoint)?;
        drop(lease);

        self.journals
            .kura
            .stage_kagemusha_finality_sidecar(
                self.finality.artifact().height,
                self.finality.artifact().block_hash,
                self.journals.source_prefix.witness(),
                self.journals.execution_prefix,
                self.journals
                    .source_prefix
                    .parliament_timed_ovn_casting_bindings()
                    .unwrap_or(&[]),
            )
            .map_err(CarrierExecutionWitnessPublicationError::Witness)?;
        self.journals
            .kura
            .promote_kagemusha_finality_sidecar(
                self.finality.artifact(),
                self.checkpoint.finality_receipt(),
            )
            .map_err(CarrierExecutionWitnessPublicationError::Witness)
    }
}

#[cfg(test)]
#[path = "execution_witness_publication_tests.rs"]
mod tests;
