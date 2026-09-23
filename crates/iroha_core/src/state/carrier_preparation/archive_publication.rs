//! Publish the original archive captures before acquiring State publication writers.
//!
//! Each archive retains its admitted immutable projection and retry progress. A
//! completed provider capture survives a later reputation failure; retry never
//! recaptures State or treats an archive-local error as a consensus rejection.

use super::{super::DetachedCarrierComponents, DecisionBoundCarrierJournals};
use crate::{
    kura::{KuraPublicationPreparationError, KuraWsvCheckpointReceipt},
    query::{
        provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1,
        reputation_finalized::ReputationFinalizedArchiveError,
    },
};

/// Local archive continuation refusal with every original owner still retained.
pub(crate) enum CarrierArchivePublicationError {
    /// Original Kura contention or storage repair prevents the receipt join.
    Kura(KuraPublicationPreparationError),
    /// The exact retained checkpoint/finality no longer authenticates.
    Checkpoint(crate::kura::Error),
    /// Provider publication failed; reputation publication has not been attempted.
    Provider(ProviderIngestFinalizedArchiveErrorV1),
    /// Reputation publication failed; completed provider work remains retained.
    Reputation(ReputationFinalizedArchiveError),
}

impl std::fmt::Debug for CarrierArchivePublicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kura(error) => f.debug_tuple("Kura").field(error).finish(),
            Self::Checkpoint(error) => f.debug_tuple("Checkpoint").field(error).finish(),
            Self::Provider(error) => f.debug_tuple("Provider").field(error).finish(),
            Self::Reputation(error) => f.debug_tuple("Reputation").field(error).finish(),
        }
    }
}

impl<Admission>
    DecisionBoundCarrierJournals<Admission, DetachedCarrierComponents, KuraWsvCheckpointReceipt>
{
    /// Authenticate and publish the original captures in provider/reputation order.
    ///
    /// The caller admits installation work and joins the original target Kura
    /// before calling this method. It must hold no State or Kura publication
    /// lease. The temporary nonblocking lease below protects checkpoint readback
    /// and both archive writes. Each capture authenticates through the held Kura
    /// boundary and probes its archive writer without waiting on its readers.
    /// Every refusal releases this lease before returning the exact retry owner.
    /// The final State publisher must rejoin the same receipt under its final
    /// Kura lease; this intermediate success grants no State publication right.
    ///
    /// Refusal leaves this complete owner in place, including partial immutable
    /// archive progress and all capture/resource reservations. Retrying invokes
    /// the existing exact-replay checks on any already completed capture.
    pub(crate) fn publish_archives(&mut self) -> Result<(), CarrierArchivePublicationError> {
        let lease = self
            .journals
            .kura
            .try_publication_lease()
            .map_err(CarrierArchivePublicationError::Kura)?;
        lease
            .reauthenticate_checkpoint(
                &self.checkpoint,
                self.finality.artifact(),
                self.journals.checkpoint,
            )
            .map_err(CarrierArchivePublicationError::Checkpoint)?;
        let receipt = self.checkpoint.finality_receipt();
        if let Some(provider) = self.journals.provider_capture.as_mut() {
            provider
                .publish_under_publication_lease(&lease, receipt)
                .map_err(CarrierArchivePublicationError::Provider)?;
        }
        if let Some(reputation) = self.journals.reputation_capture.as_mut() {
            reputation
                .publish_under_publication_lease(&lease, receipt)
                .map_err(CarrierArchivePublicationError::Reputation)?;
        }
        drop(lease);
        Ok(())
    }
}

#[cfg(test)]
#[path = "archive_publication_tests.rs"]
mod tests;
