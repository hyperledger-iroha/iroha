//! Daemon-owned restart boundary for private Musubi publication custody.
//!
//! This local assembly does not start the private listener. A deployment factory may take the
//! recovered journal and verified seed store only after they match the daemon's live network.
use super::{
    MusubiPublicationFinalizedArchiveRegistrationReaderV1, MusubiPublicationPrivateServiceContextV1,
};
use iroha_musubi_service::{
    DurableMusubiPublicationServiceJournalLimitsV1,
    DurableMusubiPublicationServiceJournalOpenErrorV1, DurableMusubiPublicationServiceJournalV1,
    MusubiPublicationServiceConfigurationV1, MusubiPublicationServiceJournalBindingV1,
    MusubiSeedStagingBackendV1, MusubiSeedStagingErrorV1,
};
use std::path::Path;

/// Failure assembling the exact local publication-custody owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiPublicationPrivateLocalCustodyErrorV1 {
    /// The supplied service configuration is for another genesis-derived network.
    NetworkMismatch,
    /// Existing replay state could not be reopened and durably recovered.
    Journal(DurableMusubiPublicationServiceJournalOpenErrorV1),
    /// The private seed directory could not be pinned under the configured provider.
    Seed(MusubiSeedStagingErrorV1),
}

/// Recovered local owners available to a qualified injected publication factory.
///
/// Ordinary startup never initializes missing journal state or a missing seed ownership marker.
/// Opening the journal first recovers interrupted attempts as durable tombstones; opening the
/// seed store then pins its exact bytes.
/// If either open fails, both exclusive directory leases are released before an error returns.
pub struct MusubiPublicationPrivateLocalCustodyV1 {
    journal: DurableMusubiPublicationServiceJournalV1,
    seed: MusubiSeedStagingBackendV1,
    finalized_reader: MusubiPublicationFinalizedArchiveRegistrationReaderV1,
}
impl std::fmt::Debug for MusubiPublicationPrivateLocalCustodyV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MusubiPublicationPrivateLocalCustodyV1")
            .field("journal_revision", &self.journal.revision())
            .finish_non_exhaustive()
    }
}
impl MusubiPublicationPrivateLocalCustodyV1 {
    /// Transfer exclusive owners to a qualified private-service factory.
    ///
    /// The journal, seed backend and reader remain separate concrete dependencies so the factory
    /// can inject them into the existing service/coordinator interfaces without a second protocol.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        DurableMusubiPublicationServiceJournalV1,
        MusubiSeedStagingBackendV1,
        MusubiPublicationFinalizedArchiveRegistrationReaderV1,
    ) {
        (self.journal, self.seed, self.finalized_reader)
    }
}
impl MusubiPublicationPrivateServiceContextV1 {
    /// Reopen already initialized local publication custody for this daemon's exact network.
    ///
    /// The durable journal performs interrupted-attempt recovery before the caller can receive
    /// the seed backend. Missing journal state or seed ownership is an error, never implicit
    /// first-boot creation.
    /// Provisioning initializes both native owners explicitly; restart never repairs missing history.
    /// This assembly does not enable TLS ingress, provider mutation, or readback routes.
    ///
    /// # Errors
    /// Rejects a foreign network, missing or invalid journal/seed ownership, or unsafe native custody.
    pub fn open_local_publication_custody(
        &self,
        journal_root: &Path,
        seed_root: &Path,
        configuration: &MusubiPublicationServiceConfigurationV1,
        journal_limits: DurableMusubiPublicationServiceJournalLimitsV1,
        max_seed_records: u32,
        max_seed_bytes: u64,
    ) -> Result<MusubiPublicationPrivateLocalCustodyV1, MusubiPublicationPrivateLocalCustodyErrorV1>
    {
        if configuration.network_id != self.network_id {
            return Err(MusubiPublicationPrivateLocalCustodyErrorV1::NetworkMismatch);
        }
        let binding = MusubiPublicationServiceJournalBindingV1::from_configuration(configuration);
        let journal =
            DurableMusubiPublicationServiceJournalV1::open(journal_root, binding, journal_limits)
                .map_err(MusubiPublicationPrivateLocalCustodyErrorV1::Journal)?;
        let seed = MusubiSeedStagingBackendV1::open(
            seed_root,
            configuration.seed_provider,
            max_seed_records,
            max_seed_bytes,
        )
        .map_err(MusubiPublicationPrivateLocalCustodyErrorV1::Seed)?;
        Ok(MusubiPublicationPrivateLocalCustodyV1 {
            journal,
            seed,
            finalized_reader: self.finalized_archive_registration_reader(),
        })
    }
}
