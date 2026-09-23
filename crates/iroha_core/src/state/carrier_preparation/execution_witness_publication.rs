//! Persist final witness proofs from the original validated execution custody.
//!
//! Staging and promotion use the existing bounded Kura owners before physical
//! State acquisition. Refusal retains every original journal and reservation;
//! partial durable work is completed by the same exact idempotent continuation.

use super::{super::DetachedCarrierComponents, DecisionBoundCarrierJournals};
use crate::kura::{KuraPublicationLease, KuraWsvCheckpointReceipt};

/// Local durability failure, never a verdict on the decided proposal.
#[derive(Debug)]
pub(crate) enum CarrierExecutionWitnessPublicationError {
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
    /// Original target-Kura identity and source authentication precede this
    /// continuation. The caller retains one lease across source authentication,
    /// witness/archive persistence and State preparation. No substitute witness
    /// or casting bindings are accepted, and no Kura fence is reacquired here.
    ///
    /// The final publisher reauthenticates the final sidecar under this same
    /// lease before acquiring any State writer.
    pub(crate) fn publish_execution_witness(
        &mut self,
        lease: &KuraPublicationLease<'_>,
    ) -> Result<(), CarrierExecutionWitnessPublicationError> {
        lease
            .reauthenticate_checkpoint(
                &self.checkpoint,
                self.finality.artifact(),
                self.journals.checkpoint,
            )
            .map_err(CarrierExecutionWitnessPublicationError::Checkpoint)?;
        lease
            .publish_execution_witness(
                self.finality.artifact(),
                self.checkpoint.finality_receipt(),
                self.journals.source_prefix.witness(),
                self.journals
                    .source_prefix
                    .parliament_timed_ovn_casting_bindings()
                    .unwrap_or(&[]),
            )
            .map_err(CarrierExecutionWitnessPublicationError::Witness)
    }
}

#[cfg(test)]
#[path = "execution_witness_publication_tests.rs"]
mod tests;
