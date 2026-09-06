//! Qualified rollback-resistant durability for the Musubi attestation journal.
//!
//! The clock samples only the host UNIX clock and returns a value only after a small, payload-free
//! high-water record is authoritative in a deployment- supplied monotonic compare-and-swap seal.
//! Initialization and restart open are separate: ordinary open never recreates missing state. Every
//! sample fences the configured adapter qualification and exact authoritative record before and
//! after advancement, so rollback, substitution, and unresolved compare-and-swap outcomes fail
//! closed.
//!
//! The same qualified provider also exposes an independent journal-checkpoint namespace. Exact
//! canonical checkpoint bytes are stored as immutable, content-addressed blobs before a small
//! predecessor-linked head is advanced. This separation makes a lost provider response safely
//! recoverable without putting private journal DTOs in the public provider contract.
use crate::provider_attestation_journal::{
    MUSUBI_PROVIDER_ATTESTATION_JOURNAL_CHECKPOINT_MAX_BYTES_V1,
    MusubiProviderAttestationJournalPolicyV1,
    musubi_provider_attestation_journal_checkpoint_revision_v1,
    validate_musubi_provider_attestation_journal_checkpoint_metadata_v1,
};
use crate::provider_ingest_runtime::ProviderIngestFutureV1;
use iroha_config::parameters::is_production_runtime_handle;
use iroha_data_model::{NetworkId, sorafs::capacity::ProviderId};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use std::{
    fmt,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::sync::Mutex;
const CLOCK_SCOPE_DOMAIN_V1: &[u8] = b"sorafs.musubi.provider-attestation.clock-scope.v1\0";
const CLOCK_RECORD_DOMAIN_V1: &[u8] = b"sorafs.musubi.provider-attestation.clock-record.v1\0";
const JOURNAL_CHECKPOINT_SCOPE_DOMAIN_V1: &[u8] =
    b"sorafs.musubi.provider-attestation.journal-checkpoint-scope.v1\0";
const JOURNAL_CHECKPOINT_HEAD_RECORD_DOMAIN_V1: &[u8] =
    b"sorafs.musubi.provider-attestation.journal-checkpoint-head-record.v1\0";
const CLOCK_RECORD_VERSION_V1: u8 = 1;
const JOURNAL_CHECKPOINT_HEAD_RECORD_VERSION_V1: u8 = 1;
const CLOCK_SEAL_QUALIFICATION_VERSION_V1: u8 = 1;
/// Hard deadline for one external monotonic-seal call.
pub const MUSUBI_PROVIDER_ATTESTATION_CLOCK_SEAL_TIMEOUT_MS_V1: u64 = 5_000;
/// Maximum unreferenced candidate checkpoint blobs retained by a V1 seal.
pub const MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_COUNT_MAX_V1: u32 = 16;
/// Maximum aggregate canonical bytes retained across V1 orphan blobs.
pub const MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_BYTES_MAX_V1: u64 =
    2 * MUSUBI_PROVIDER_ATTESTATION_JOURNAL_CHECKPOINT_MAX_BYTES_V1 as u64;
/// Maximum age of a V1 orphan blob before authenticated collection is required.
pub const MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_AGE_MAX_MS_V1: u64 = 24 * 60 * 60 * 1_000;
/// Exact chain incarnation and provider whose journal consumes the clock.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct MusubiProviderAttestationClockScopeV1 {
    network_id: NetworkId,
    provider_id: ProviderId,
}
impl MusubiProviderAttestationClockScopeV1 {
    /// Construct a non-inert journal clock scope.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid network or zero provider identity.
    pub fn try_new(
        network_id: NetworkId,
        provider_id: ProviderId,
    ) -> Result<Self, MusubiProviderAttestationClockErrorV1> {
        let scope = Self {
            network_id,
            provider_id,
        };
        scope.validate()?;
        Ok(scope)
    }
    /// Borrow the exact deployment identity.
    #[must_use]
    pub const fn network_id(&self) -> &NetworkId {
        &self.network_id
    }
    /// Return the exact provider identity.
    #[must_use]
    pub const fn provider_id(&self) -> ProviderId {
        self.provider_id
    }
    fn validate(&self) -> Result<(), MusubiProviderAttestationClockErrorV1> {
        if self.network_id.as_bytes()[31] & 1 != 1 || *self.provider_id.as_bytes() == [0; 32] {
            return Err(MusubiProviderAttestationClockErrorV1::InvalidScope);
        }
        Ok(())
    }
    /// Return the non-secret canonical scope commitment.
    ///
    /// # Errors
    ///
    /// Returns an error if a decoded scope is invalid or canonical encoding unexpectedly fails.
    pub fn scope_digest(&self) -> Result<[u8; 32], MusubiProviderAttestationClockErrorV1> {
        self.validate()?;
        domain_hash_norito(CLOCK_SCOPE_DOMAIN_V1, self)
            .ok_or(MusubiProviderAttestationClockErrorV1::InvalidScope)
    }
}
/// Exact deployment and journal-policy scope of one sealed checkpoint chain.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct MusubiProviderAttestationJournalCheckpointScopeV1 {
    network_id: NetworkId,
    provider_id: ProviderId,
    journal_policy_digest: [u8; 32],
}
impl MusubiProviderAttestationJournalCheckpointScopeV1 {
    /// Construct a non-inert checkpoint-seal scope.
    ///
    /// `journal_policy_digest` must be the result of
    /// [`MusubiProviderAttestationJournalPolicyV1::digest`].
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid network, zero provider identity, or zero journal-policy
    /// digest.
    pub fn try_new(
        network_id: NetworkId,
        provider_id: ProviderId,
        journal_policy_digest: [u8; 32],
    ) -> Result<Self, MusubiProviderAttestationJournalCheckpointSealErrorV1> {
        let scope = Self {
            network_id,
            provider_id,
            journal_policy_digest,
        };
        scope.validate()?;
        Ok(scope)
    }
    /// Borrow the exact deployment identity.
    #[must_use]
    pub const fn network_id(&self) -> &NetworkId {
        &self.network_id
    }
    /// Return the exact provider identity.
    #[must_use]
    pub const fn provider_id(&self) -> ProviderId {
        self.provider_id
    }
    /// Return the canonical journal-policy digest.
    #[must_use]
    pub const fn journal_policy_digest(&self) -> [u8; 32] {
        self.journal_policy_digest
    }
    /// Validate the decoded scope shape.
    ///
    /// # Errors
    ///
    /// Returns an error when any deployment identity or policy digest is
    /// inert or outside its V1 bound.
    pub fn validate(&self) -> Result<(), MusubiProviderAttestationJournalCheckpointSealErrorV1> {
        if self.network_id.as_bytes()[31] & 1 != 1
            || *self.provider_id.as_bytes() == [0; 32]
            || self.journal_policy_digest == [0; 32]
        {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidScope);
        }
        Ok(())
    }
    /// Return the canonical domain-separated scope commitment.
    ///
    /// # Errors
    ///
    /// Returns an error when a decoded scope is invalid or cannot be encoded canonically.
    pub fn scope_digest(
        &self,
    ) -> Result<[u8; 32], MusubiProviderAttestationJournalCheckpointSealErrorV1> {
        self.validate()?;
        domain_hash_norito(JOURNAL_CHECKPOINT_SCOPE_DOMAIN_V1, self)
            .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidScope)
    }
}
/// Exact private-journal checkpoint identity committed by one sealed head.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct MusubiProviderAttestationJournalCheckpointHeadV1 {
    checkpoint_sequence: u64,
    checkpoint_revision: [u8; 32],
    last_observed_unix_ms: u64,
}
impl MusubiProviderAttestationJournalCheckpointHeadV1 {
    /// Construct one non-inert checkpoint head.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero sequence or revision. An untimed sequence-1
    /// enqueue checkpoint may legitimately retain a zero UNIX-time floor.
    pub(crate) fn try_new(
        checkpoint_sequence: u64,
        checkpoint_revision: [u8; 32],
        last_observed_unix_ms: u64,
    ) -> Result<Self, MusubiProviderAttestationJournalCheckpointSealErrorV1> {
        let head = Self {
            checkpoint_sequence,
            checkpoint_revision,
            last_observed_unix_ms,
        };
        head.validate()?;
        Ok(head)
    }
    /// Return the journal checkpoint sequence.
    #[must_use]
    pub const fn checkpoint_sequence(self) -> u64 {
        self.checkpoint_sequence
    }
    /// Return the content-addressed checkpoint revision.
    #[must_use]
    pub const fn checkpoint_revision(self) -> [u8; 32] {
        self.checkpoint_revision
    }
    /// Return the greatest UNIX millisecond observed by the journal.
    #[must_use]
    pub const fn last_observed_unix_ms(self) -> u64 {
        self.last_observed_unix_ms
    }
    /// Validate the decoded V1 head shape.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero sequence or revision.
    pub fn validate(self) -> Result<(), MusubiProviderAttestationJournalCheckpointSealErrorV1> {
        if self.checkpoint_sequence == 0 || self.checkpoint_revision == [0; 32] {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidHead);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct JournalCheckpointHeadRecordMaterialV1 {
    version: u8,
    scope_digest: [u8; 32],
    generation: u64,
    predecessor_record_digest: Option<[u8; 32]>,
    head: Option<MusubiProviderAttestationJournalCheckpointHeadV1>,
}
/// Canonical predecessor-linked record in the checkpoint-head namespace.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct MusubiProviderAttestationJournalCheckpointHeadRecordV1 {
    material: JournalCheckpointHeadRecordMaterialV1,
    record_digest: [u8; 32],
}
impl MusubiProviderAttestationJournalCheckpointHeadRecordV1 {
    /// Construct the explicit one-time empty H0 record.
    ///
    /// H0 contains no checkpoint identity and is the only valid initial record.
    /// Ordinary restart/open code must reject an absent external H0 rather than
    /// create one from any local checkpoint bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for an inert scope digest.
    pub(crate) fn initial(
        scope_digest: [u8; 32],
    ) -> Result<Self, MusubiProviderAttestationJournalCheckpointSealErrorV1> {
        Self::new(scope_digest, 1, None, None)
    }
    /// Construct the unique next record after `previous`.
    ///
    /// # Errors
    ///
    /// Returns an error unless the checkpoint sequence advances by exactly
    /// one, its revision changes, its UNIX floor does not regress, and both
    /// the record generation and checkpoint sequence can advance.
    pub(crate) fn successor(
        previous: &Self,
        head: MusubiProviderAttestationJournalCheckpointHeadV1,
    ) -> Result<Self, MusubiProviderAttestationJournalCheckpointSealErrorV1> {
        previous.validate(previous.material.scope_digest)?;
        head.validate()?;
        let generation = previous
            .material
            .generation
            .checked_add(1)
            .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::ArithmeticOverflow)?;
        let expected_checkpoint_sequence = match previous.material.head {
            None => 1,
            Some(previous_head) => previous_head
                .checkpoint_sequence
                .checked_add(1)
                .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::ArithmeticOverflow)?,
        };
        let previous_unix_ms = previous.material.head.map_or(
            0,
            MusubiProviderAttestationJournalCheckpointHeadV1::last_observed_unix_ms,
        );
        if head.checkpoint_sequence != expected_checkpoint_sequence
            || previous.material.head.is_some_and(|previous_head| {
                head.checkpoint_revision == previous_head.checkpoint_revision
            })
            || head.last_observed_unix_ms < previous_unix_ms
        {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidLineage);
        }
        let next = Self::new(
            previous.material.scope_digest,
            generation,
            Some(previous.record_digest),
            Some(head),
        )?;
        next.validate_successor_of(previous)?;
        Ok(next)
    }
    fn new(
        scope_digest: [u8; 32],
        generation: u64,
        predecessor_record_digest: Option<[u8; 32]>,
        head: Option<MusubiProviderAttestationJournalCheckpointHeadV1>,
    ) -> Result<Self, MusubiProviderAttestationJournalCheckpointSealErrorV1> {
        let material = JournalCheckpointHeadRecordMaterialV1 {
            version: JOURNAL_CHECKPOINT_HEAD_RECORD_VERSION_V1,
            scope_digest,
            generation,
            predecessor_record_digest,
            head,
        };
        let record_digest = domain_hash_norito(JOURNAL_CHECKPOINT_HEAD_RECORD_DOMAIN_V1, &material)
            .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord)?;
        let record = Self {
            material,
            record_digest,
        };
        record.validate(scope_digest)?;
        Ok(record)
    }
    /// Validate the decoded V1 record against the exact scope commitment.
    ///
    /// # Errors
    ///
    /// Returns an error for an unexpected scope, malformed generation/
    /// predecessor shape, invalid head, or noncanonical record digest.
    pub fn validate(
        &self,
        expected_scope_digest: [u8; 32],
    ) -> Result<(), MusubiProviderAttestationJournalCheckpointSealErrorV1> {
        if let Some(head) = self.material.head {
            head.validate()?;
        }
        let lineage_shape_is_valid = match (
            self.material.generation,
            self.material.predecessor_record_digest,
            self.material.head,
        ) {
            (1, None, None) => true,
            (generation @ 2.., Some(digest), Some(head)) => {
                digest != [0; 32] && head.checkpoint_sequence.checked_add(1) == Some(generation)
            }
            _ => false,
        };
        if self.material.version != JOURNAL_CHECKPOINT_HEAD_RECORD_VERSION_V1
            || expected_scope_digest == [0; 32]
            || self.material.scope_digest != expected_scope_digest
            || !lineage_shape_is_valid
            || self.record_digest == [0; 32]
            || domain_hash_norito(JOURNAL_CHECKPOINT_HEAD_RECORD_DOMAIN_V1, &self.material)
                != Some(self.record_digest)
        {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord);
        }
        Ok(())
    }
    /// Validate this record as the exact next record after `previous`.
    ///
    /// # Errors
    ///
    /// Returns an error for any scope, generation, predecessor, checkpoint
    /// sequence, revision, or UNIX-floor discontinuity.
    pub fn validate_successor_of(
        &self,
        previous: &Self,
    ) -> Result<(), MusubiProviderAttestationJournalCheckpointSealErrorV1> {
        previous.validate(previous.material.scope_digest)?;
        self.validate(previous.material.scope_digest)?;
        let expected_generation = previous
            .material
            .generation
            .checked_add(1)
            .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::ArithmeticOverflow)?;
        let expected_checkpoint_sequence = match previous.material.head {
            None => 1,
            Some(previous_head) => previous_head
                .checkpoint_sequence
                .checked_add(1)
                .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::ArithmeticOverflow)?,
        };
        let Some(head) = self.material.head else {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidLineage);
        };
        let previous_unix_ms = previous.material.head.map_or(
            0,
            MusubiProviderAttestationJournalCheckpointHeadV1::last_observed_unix_ms,
        );
        if self.material.generation != expected_generation
            || self.material.predecessor_record_digest != Some(previous.record_digest)
            || head.checkpoint_sequence != expected_checkpoint_sequence
            || previous.material.head.is_some_and(|previous_head| {
                head.checkpoint_revision == previous_head.checkpoint_revision
            })
            || head.last_observed_unix_ms < previous_unix_ms
        {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidLineage);
        }
        Ok(())
    }
    /// Return the checkpoint-seal record generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.material.generation
    }
    /// Return the fixed checkpoint-head record version.
    #[must_use]
    pub const fn version(&self) -> u8 {
        self.material.version
    }
    /// Return the exact checkpoint-seal scope commitment.
    #[must_use]
    pub const fn scope_digest(&self) -> [u8; 32] {
        self.material.scope_digest
    }
    /// Return the previous record digest, absent only at explicit bootstrap.
    #[must_use]
    pub const fn predecessor_record_digest(&self) -> Option<[u8; 32]> {
        self.material.predecessor_record_digest
    }
    /// Return the exact checkpoint head, absent only for the empty H0 record.
    #[must_use]
    pub const fn head(&self) -> Option<MusubiProviderAttestationJournalCheckpointHeadV1> {
        self.material.head
    }
    /// Return the record's canonical domain-separated digest.
    #[must_use]
    pub const fn record_digest(&self) -> [u8; 32] {
        self.record_digest
    }
}
/// Payload-free public qualification of one external journal durability seal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusubiProviderAttestationClockSealQualificationV1 {
    version: u8,
    adapter_revision: u64,
    policy_digest: [u8; 32],
    orphan_blob_count_max: u32,
    orphan_blob_bytes_max: u64,
    orphan_blob_age_max_ms: u64,
}
impl MusubiProviderAttestationClockSealQualificationV1 {
    /// Construct a V1 combined clock/checkpoint seal qualification.
    #[must_use]
    pub const fn new(adapter_revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            version: CLOCK_SEAL_QUALIFICATION_VERSION_V1,
            adapter_revision,
            policy_digest,
            orphan_blob_count_max: MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_COUNT_MAX_V1,
            orphan_blob_bytes_max: MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_BYTES_MAX_V1,
            orphan_blob_age_max_ms: MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_AGE_MAX_MS_V1,
        }
    }
    /// Return the adapter-local qualification revision.
    #[must_use]
    pub const fn adapter_revision(self) -> u64 {
        self.adapter_revision
    }
    /// Return the configured combined durability-seal policy digest.
    #[must_use]
    pub const fn policy_digest(self) -> [u8; 32] {
        self.policy_digest
    }
    /// Return the fixed maximum count of unreferenced candidate blobs.
    #[must_use]
    pub const fn orphan_blob_count_max(self) -> u32 {
        self.orphan_blob_count_max
    }
    /// Return the fixed maximum aggregate bytes of unreferenced candidate blobs.
    #[must_use]
    pub const fn orphan_blob_bytes_max(self) -> u64 {
        self.orphan_blob_bytes_max
    }
    /// Return the fixed maximum orphan age before authenticated collection.
    #[must_use]
    pub const fn orphan_blob_age_max_ms(self) -> u64 {
        self.orphan_blob_age_max_ms
    }
    fn validate(self) -> Result<(), MusubiProviderAttestationClockErrorV1> {
        if self.version != CLOCK_SEAL_QUALIFICATION_VERSION_V1
            || self.adapter_revision == 0
            || self.policy_digest == [0; 32]
            || self.orphan_blob_count_max != MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_COUNT_MAX_V1
            || self.orphan_blob_bytes_max != MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_BYTES_MAX_V1
            || self.orphan_blob_age_max_ms != MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_AGE_MAX_MS_V1
        {
            return Err(MusubiProviderAttestationClockErrorV1::InvalidSealBinding);
        }
        Ok(())
    }
}
/// Expected stable identity and qualification of the injected durability seal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusubiProviderAttestationClockSealBindingV1 {
    runtime_handle: String,
    qualification: MusubiProviderAttestationClockSealQualificationV1,
}
impl MusubiProviderAttestationClockSealBindingV1 {
    /// Construct a credential-free production durability-seal binding.
    ///
    /// # Errors
    ///
    /// Returns an error for a test/development handle or inert qualification.
    pub fn try_new(
        runtime_handle: impl Into<String>,
        qualification: MusubiProviderAttestationClockSealQualificationV1,
    ) -> Result<Self, MusubiProviderAttestationClockErrorV1> {
        let runtime_handle = runtime_handle.into();
        qualification.validate()?;
        if !is_production_runtime_handle(&runtime_handle) {
            return Err(MusubiProviderAttestationClockErrorV1::InvalidSealBinding);
        }
        Ok(Self {
            runtime_handle,
            qualification,
        })
    }
    /// Borrow the credential-free runtime handle.
    #[must_use]
    pub fn runtime_handle(&self) -> &str {
        &self.runtime_handle
    }
    /// Return the exact expected qualification.
    #[must_use]
    pub const fn qualification(&self) -> MusubiProviderAttestationClockSealQualificationV1 {
        self.qualification
    }
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ClockRecordMaterialV1 {
    version: u8,
    scope_digest: [u8; 32],
    generation: u64,
    predecessor_digest: Option<[u8; 32]>,
    floor_unix_ms: u64,
}
/// Canonical payload-free record held by the external monotonic seal.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct MusubiProviderAttestationClockSealRecordV1 {
    material: ClockRecordMaterialV1,
    record_digest: [u8; 32],
}
impl MusubiProviderAttestationClockSealRecordV1 {
    fn initial(
        scope_digest: [u8; 32],
        floor_unix_ms: u64,
    ) -> Result<Self, MusubiProviderAttestationClockErrorV1> {
        Self::new(scope_digest, 1, None, floor_unix_ms)
    }
    fn successor(
        previous: &Self,
        floor_unix_ms: u64,
    ) -> Result<Self, MusubiProviderAttestationClockErrorV1> {
        if floor_unix_ms < previous.material.floor_unix_ms {
            return Err(MusubiProviderAttestationClockErrorV1::ClockRollback);
        }
        let generation = previous
            .material
            .generation
            .checked_add(1)
            .ok_or(MusubiProviderAttestationClockErrorV1::ArithmeticOverflow)?;
        Self::new(
            previous.material.scope_digest,
            generation,
            Some(previous.record_digest),
            floor_unix_ms,
        )
    }
    fn new(
        scope_digest: [u8; 32],
        generation: u64,
        predecessor_digest: Option<[u8; 32]>,
        floor_unix_ms: u64,
    ) -> Result<Self, MusubiProviderAttestationClockErrorV1> {
        let material = ClockRecordMaterialV1 {
            version: CLOCK_RECORD_VERSION_V1,
            scope_digest,
            generation,
            predecessor_digest,
            floor_unix_ms,
        };
        let record_digest = domain_hash_norito(CLOCK_RECORD_DOMAIN_V1, &material)
            .ok_or(MusubiProviderAttestationClockErrorV1::InvalidSealRecord)?;
        let record = Self {
            material,
            record_digest,
        };
        record.validate(scope_digest)?;
        Ok(record)
    }
    fn validate(
        &self,
        expected_scope_digest: [u8; 32],
    ) -> Result<(), MusubiProviderAttestationClockErrorV1> {
        let predecessor_is_valid =
            match (self.material.generation, self.material.predecessor_digest) {
                (1, None) => true,
                (2.., Some(digest)) => digest != [0; 32],
                _ => false,
            };
        if self.material.version != CLOCK_RECORD_VERSION_V1
            || self.material.scope_digest != expected_scope_digest
            || expected_scope_digest == [0; 32]
            || !predecessor_is_valid
            || self.material.floor_unix_ms == 0
            || self.record_digest == [0; 32]
            || domain_hash_norito(CLOCK_RECORD_DOMAIN_V1, &self.material)
                != Some(self.record_digest)
        {
            return Err(MusubiProviderAttestationClockErrorV1::InvalidSealRecord);
        }
        Ok(())
    }
    /// Return the monotonic record generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.material.generation
    }
    /// Return the previous record digest, if this is not the initial record.
    #[must_use]
    pub const fn predecessor_digest(&self) -> Option<[u8; 32]> {
        self.material.predecessor_digest
    }
    /// Return the durably sealed UNIX-millisecond floor.
    #[must_use]
    pub const fn floor_unix_ms(&self) -> u64 {
        self.material.floor_unix_ms
    }
    /// Return the record's canonical domain-separated digest.
    #[must_use]
    pub const fn record_digest(&self) -> [u8; 32] {
        self.record_digest
    }
}
/// Bounded external failure from the deployment monotonic seal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MusubiProviderAttestationClockSealErrorV1 {
    /// The seal could not complete the operation.
    #[error("Musubi provider-attestation clock seal is unavailable")]
    Unavailable,
    /// The seal rejected an invalid or stale operation.
    #[error("Musubi provider-attestation clock seal rejected the operation")]
    Rejected,
    /// The caller cannot tell whether the compare-and-swap committed.
    #[error("Musubi provider-attestation clock seal outcome is ambiguous")]
    Ambiguous,
}
/// Stable failure from the sealed journal-checkpoint protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MusubiProviderAttestationJournalCheckpointSealErrorV1 {
    /// The network/provider/policy scope is inert or malformed.
    #[error("Musubi provider-attestation journal checkpoint scope is invalid")]
    InvalidScope,
    /// A checkpoint head contains an inert or malformed field.
    #[error("Musubi provider-attestation journal checkpoint head is invalid")]
    InvalidHead,
    /// A predecessor-linked head record is malformed or substituted.
    #[error("Musubi provider-attestation journal checkpoint-head record is invalid")]
    InvalidRecord,
    /// A successor does not exactly continue its predecessor.
    #[error("Musubi provider-attestation journal checkpoint-head lineage is invalid")]
    InvalidLineage,
    /// Checkpoint bytes are malformed, oversized, noncanonical, or substituted.
    #[error("Musubi provider-attestation journal checkpoint blob is invalid")]
    InvalidBlob,
    /// An authoritative head names an absent content-addressed blob.
    #[error("Musubi provider-attestation journal checkpoint blob is missing")]
    MissingBlob,
    /// Explicit initialization found an existing external H0/head record.
    #[error("Musubi provider-attestation journal checkpoint seal is already initialized")]
    AlreadyInitialized,
    /// Ordinary open found no externally initialized H0/head record.
    #[error("Musubi provider-attestation journal checkpoint seal is uninitialized")]
    Uninitialized,
    /// The configured or live durability-seal identity/qualification is invalid.
    #[error("Musubi provider-attestation journal durability-seal binding is invalid")]
    InvalidSealBinding,
    /// The durability seal was unavailable or timed out.
    #[error("Musubi provider-attestation journal durability seal is unavailable")]
    SealUnavailable,
    /// The durability seal rejected or contradicted the requested operation.
    #[error("Musubi provider-attestation journal durability seal rejected the operation")]
    SealRejected,
    /// A provider response was lost and exact readback did not resolve it.
    #[error("Musubi provider-attestation journal durability-seal outcome is ambiguous")]
    SealAmbiguous,
    /// Authoritative checkpoint state regressed or disappeared.
    #[error("Musubi provider-attestation journal checkpoint head rolled back")]
    Rollback,
    /// Another head record occupies the same predecessor/generation branch.
    #[error("Musubi provider-attestation journal checkpoint head forked")]
    Fork,
    /// A head generation or checkpoint sequence overflowed.
    #[error("Musubi provider-attestation journal checkpoint counter overflowed")]
    ArithmeticOverflow,
}
/// Deployment boundary for rollback-resistant journal clock and checkpoint state.
///
/// Implementations must provide authenticated, linearizable compare-and-swap storage in two
/// independent small-record namespaces: the clock high-water record and the journal checkpoint
/// head. They must also provide authenticated immutable content-addressed blob storage for exact
/// checkpoint bytes. The qualification covers all three namespaces as one deployment policy.
///
/// Blob operations must independently recompute
/// [`musubi_provider_attestation_journal_checkpoint_blob_revision_v1`] and reject a mismatched
/// identity. A successful put is durable and exact-current retries are idempotent. Head
/// compare-and-swap is linearizable and durable; an exact-current `next` is an idempotent success
/// even with a stale expected digest, while a differing head at the same predecessor is never
/// overwritten. Before acknowledging a head CAS, the provider must bind the named exact blob to
/// durable retention so it cannot disappear while that head or a retained descendant can reference
/// it. Orphaned candidate blobs may be collected only under authenticated rules that prove no
/// retained head can reference them. None of these operations may persist credentials, paths,
/// nonces, or provider URLs.
///
/// Returning a valid qualification attests to the exact fixed orphan count,
/// byte, and age ceilings exported above. Deployment qualification must cover
/// the provider's head-CAS/garbage-collection concurrency matrix and demonstrate
/// that collection retains the latest head blob and its direct predecessor.
pub trait MusubiProviderAttestationClockSealV1: Send + Sync + fmt::Debug + 'static {
    /// Return the stable credential-free runtime identity.
    ///
    /// This getter must be a bounded, non-blocking local snapshot; external
    /// I/O belongs only in the timed load/CAS futures below.
    fn runtime_handle(&self) -> &str;
    /// Return the live payload-free adapter qualification.
    ///
    /// This getter must be a bounded, non-blocking local snapshot; callers
    /// fence it around every timed external seal operation.
    fn qualification(
        &self,
    ) -> Result<
        MusubiProviderAttestationClockSealQualificationV1,
        MusubiProviderAttestationClockSealErrorV1,
    >;
    /// Load the exact authoritative record for one scope, if initialized.
    fn load_latest<'a>(
        &'a self,
        scope_digest: [u8; 32],
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            Option<MusubiProviderAttestationClockSealRecordV1>,
            MusubiProviderAttestationClockSealErrorV1,
        >,
    >;
    /// Install `next` only when the authoritative digest equals `expected`.
    fn compare_and_swap<'a>(
        &'a self,
        scope_digest: [u8; 32],
        expected: Option<[u8; 32]>,
        next: &'a MusubiProviderAttestationClockSealRecordV1,
    ) -> ProviderIngestFutureV1<'a, Result<(), MusubiProviderAttestationClockSealErrorV1>>;
    /// Durably store one immutable content-addressed journal checkpoint blob.
    ///
    /// The default rejects the operation so existing clock-only providers fail
    /// closed until their combined durability implementation is qualified.
    fn put_journal_checkpoint_blob<'a>(
        &'a self,
        _scope_digest: [u8; 32],
        _checkpoint_revision: [u8; 32],
        _checkpoint_blob: &'a [u8],
    ) -> ProviderIngestFutureV1<'a, Result<(), MusubiProviderAttestationClockSealErrorV1>> {
        Box::pin(async { Err(MusubiProviderAttestationClockSealErrorV1::Rejected) })
    }
    /// Load one exact immutable journal checkpoint blob by its revision.
    ///
    /// The default rejects the operation so existing clock-only providers fail
    /// closed until their combined durability implementation is qualified.
    fn load_journal_checkpoint_blob<'a>(
        &'a self,
        _scope_digest: [u8; 32],
        _checkpoint_revision: [u8; 32],
    ) -> ProviderIngestFutureV1<
        'a,
        Result<Option<Vec<u8>>, MusubiProviderAttestationClockSealErrorV1>,
    > {
        Box::pin(async { Err(MusubiProviderAttestationClockSealErrorV1::Rejected) })
    }
    /// Load the exact authoritative checkpoint-head record for one scope.
    ///
    /// This is a separate namespace from [`Self::load_latest`]. The default
    /// rejects the operation so clock-only providers remain fail-closed.
    fn load_journal_checkpoint_head<'a>(
        &'a self,
        _scope_digest: [u8; 32],
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            Option<MusubiProviderAttestationJournalCheckpointHeadRecordV1>,
            MusubiProviderAttestationClockSealErrorV1,
        >,
    > {
        Box::pin(async { Err(MusubiProviderAttestationClockSealErrorV1::Rejected) })
    }
    /// Load one retained checkpoint-head record by its exact record digest.
    ///
    /// The provider must retain the latest record and its exact direct predecessor. Older records
    /// may be collected once they are no longer the direct predecessor of the authoritative latest
    /// head. The default rejects the operation so clock-only providers remain fail-closed.
    fn load_journal_checkpoint_head_record<'a>(
        &'a self,
        _scope_digest: [u8; 32],
        _record_digest: [u8; 32],
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            Option<MusubiProviderAttestationJournalCheckpointHeadRecordV1>,
            MusubiProviderAttestationClockSealErrorV1,
        >,
    > {
        Box::pin(async { Err(MusubiProviderAttestationClockSealErrorV1::Rejected) })
    }
    /// Install `next` only when the checkpoint-head digest equals `expected`.
    ///
    /// This is a separate namespace from [`Self::compare_and_swap`]. The default rejects the
    /// operation so clock-only providers remain fail-closed. A successful CAS must make both `next`
    /// and the exact record named by its predecessor digest available through
    /// [`Self::load_journal_checkpoint_head_record`].
    fn compare_and_swap_journal_checkpoint_head<'a>(
        &'a self,
        _scope_digest: [u8; 32],
        _expected: Option<[u8; 32]>,
        _next: &'a MusubiProviderAttestationJournalCheckpointHeadRecordV1,
    ) -> ProviderIngestFutureV1<'a, Result<(), MusubiProviderAttestationClockSealErrorV1>> {
        Box::pin(async { Err(MusubiProviderAttestationClockSealErrorV1::Rejected) })
    }
}
/// Derive the existing journal checkpoint revision for bounded blob bytes.
///
/// This exposes only the content-addressed identity already used by the private journal store; it
/// does not decode or expose the private checkpoint DTO.
///
/// # Errors
///
/// Returns an error for an empty or hard-oversized blob.
pub fn musubi_provider_attestation_journal_checkpoint_blob_revision_v1(
    checkpoint_blob: &[u8],
) -> Result<[u8; 32], MusubiProviderAttestationJournalCheckpointSealErrorV1> {
    if checkpoint_blob.is_empty()
        || checkpoint_blob.len() > MUSUBI_PROVIDER_ATTESTATION_JOURNAL_CHECKPOINT_MAX_BYTES_V1
    {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob);
    }
    let revision = musubi_provider_attestation_journal_checkpoint_revision_v1(checkpoint_blob);
    if revision == [0; 32] {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob);
    }
    Ok(revision)
}
/// Explicitly initialize the absent checkpoint-head namespace with empty H0.
///
/// Exact H0 readback resolves a lost compare-and-swap response and makes an
/// identical initialization retry idempotent. A different existing record is
/// rejected. The independently sealed clock must already be initialized.
///
/// # Errors
///
/// Fails closed for invalid scope/binding, an uninitialized clock, a different
/// existing head, timeout, qualification drift, or unresolved CAS outcome.
pub(crate) async fn initialize_musubi_provider_attestation_journal_checkpoint_seal_v1(
    scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
    seal_binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<
    MusubiProviderAttestationJournalCheckpointHeadRecordV1,
    MusubiProviderAttestationJournalCheckpointSealErrorV1,
> {
    let scope_digest = scope.scope_digest()?;
    let _ = load_checkpoint_clock_floor(scope, seal_binding, seal).await?;
    let initial = MusubiProviderAttestationJournalCheckpointHeadRecordV1::initial(scope_digest)?;
    let current = load_checkpoint_head_authoritative(scope_digest, seal_binding, seal).await?;
    match current {
        Some(record) if record == initial => {
            ensure_checkpoint_head_unchanged(scope_digest, &record, seal_binding, seal).await?;
            return Ok(record);
        }
        Some(_) => {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::AlreadyInitialized);
        }
        None => {}
    }
    let committed =
        commit_checkpoint_head_and_readback(scope_digest, seal_binding, seal, None, &initial)
            .await?;
    let _ = load_checkpoint_clock_floor(scope, seal_binding, seal).await?;
    Ok(committed)
}
/// Seal one exact canonical journal checkpoint after `expected`.
///
/// The immutable blob is durably installed and read back before the separate monotonic head CAS.
/// Exact blob/head readback resolves lost provider responses, so a retry with the same predecessor
/// and checkpoint is idempotent. The head sequence must advance by exactly one and its observed
/// time must not exceed the authoritative sealed clock floor.
///
/// # Errors
///
/// Fails closed for invalid policy/scope/blob/head/lineage, absent H0, clock
/// disagreement, qualification drift, provider timeout, rollback, or fork.
pub(crate) async fn seal_musubi_provider_attestation_journal_checkpoint_v1(
    scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
    policy: MusubiProviderAttestationJournalPolicyV1,
    seal_binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
    expected: &MusubiProviderAttestationJournalCheckpointHeadRecordV1,
    head: MusubiProviderAttestationJournalCheckpointHeadV1,
    checkpoint_blob: &[u8],
) -> Result<
    MusubiProviderAttestationJournalCheckpointHeadRecordV1,
    MusubiProviderAttestationJournalCheckpointSealErrorV1,
> {
    let scope_digest = validate_checkpoint_scope_policy(scope, policy)?;
    expected.validate(scope_digest)?;
    head.validate()?;
    validate_checkpoint_blob(scope, policy, head, checkpoint_blob)?;
    let next = MusubiProviderAttestationJournalCheckpointHeadRecordV1::successor(expected, head)?;
    let clock_floor = load_checkpoint_clock_floor(scope, seal_binding, seal).await?;
    if head.last_observed_unix_ms() > clock_floor {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidHead);
    }
    let current = load_checkpoint_head_authoritative(scope_digest, seal_binding, seal)
        .await?
        .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::Uninitialized)?;
    if current == next {
        let loaded = load_checkpoint_blob_authoritative(
            scope_digest,
            head.checkpoint_revision(),
            seal_binding,
            seal,
        )
        .await?
        .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::MissingBlob)?;
        if loaded != checkpoint_blob {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob);
        }
        ensure_checkpoint_head_unchanged(scope_digest, &current, seal_binding, seal).await?;
        return Ok(current);
    }
    if current != *expected {
        return Err(classify_checkpoint_head_change(expected, &current));
    }
    validate_authoritative_checkpoint_record_blob(scope, policy, &current, seal_binding, seal)
        .await?;
    put_checkpoint_blob_and_readback(
        scope_digest,
        head.checkpoint_revision(),
        checkpoint_blob,
        seal_binding,
        seal,
    )
    .await?;
    let committed = commit_checkpoint_head_and_readback(
        scope_digest,
        seal_binding,
        seal,
        Some(expected),
        &next,
    )
    .await?;
    let clock_floor = load_checkpoint_clock_floor(scope, seal_binding, seal).await?;
    if head.last_observed_unix_ms() > clock_floor {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidHead);
    }
    Ok(committed)
}
/// Open the externally authoritative H0/head and its exact immutable blob.
///
/// Empty H0 returns `None` bytes. A nonempty head returns the canonical bytes
/// named by its revision. The external head is always authoritative; an absent
/// head, missing blob, substituted blob, or local fallback is never accepted.
///
/// # Errors
///
/// Fails closed for absent H0, malformed or noncanonical state, policy/scope disagreement, an
/// observed time above the sealed clock floor, provider timeout, or qualification drift.
pub(crate) async fn load_musubi_provider_attestation_journal_checkpoint_v1(
    scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
    policy: MusubiProviderAttestationJournalPolicyV1,
    seal_binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<
    (
        MusubiProviderAttestationJournalCheckpointHeadRecordV1,
        Option<Vec<u8>>,
    ),
    MusubiProviderAttestationJournalCheckpointSealErrorV1,
> {
    let scope_digest = validate_checkpoint_scope_policy(scope, policy)?;
    let clock_floor = load_checkpoint_clock_floor(scope, seal_binding, seal).await?;
    let record = load_checkpoint_head_authoritative(scope_digest, seal_binding, seal)
        .await?
        .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::Uninitialized)?;
    let Some(head) = record.head() else {
        ensure_checkpoint_head_unchanged(scope_digest, &record, seal_binding, seal).await?;
        return Ok((record, None));
    };
    if head.last_observed_unix_ms() > clock_floor {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidHead);
    }
    let checkpoint_blob = load_checkpoint_blob_authoritative(
        scope_digest,
        head.checkpoint_revision(),
        seal_binding,
        seal,
    )
    .await?
    .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::MissingBlob)?;
    validate_checkpoint_blob(scope, policy, head, &checkpoint_blob)?;
    let post_clock_floor = load_checkpoint_clock_floor(scope, seal_binding, seal).await?;
    if head.last_observed_unix_ms() > post_clock_floor {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidHead);
    }
    ensure_checkpoint_head_unchanged(scope_digest, &record, seal_binding, seal).await?;
    Ok((record, Some(checkpoint_blob)))
}
async fn validate_authoritative_checkpoint_record_blob(
    scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
    policy: MusubiProviderAttestationJournalPolicyV1,
    record: &MusubiProviderAttestationJournalCheckpointHeadRecordV1,
    seal_binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<(), MusubiProviderAttestationJournalCheckpointSealErrorV1> {
    let Some(head) = record.head() else {
        return Ok(());
    };
    let blob = load_checkpoint_blob_authoritative(
        scope.scope_digest()?,
        head.checkpoint_revision(),
        seal_binding,
        seal,
    )
    .await?
    .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::MissingBlob)?;
    validate_checkpoint_blob(scope, policy, head, &blob)
}
async fn ensure_checkpoint_head_unchanged(
    scope_digest: [u8; 32],
    expected: &MusubiProviderAttestationJournalCheckpointHeadRecordV1,
    seal_binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<(), MusubiProviderAttestationJournalCheckpointSealErrorV1> {
    let observed = load_checkpoint_head_authoritative(scope_digest, seal_binding, seal)
        .await?
        .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::Rollback)?;
    if observed != *expected {
        return Err(if observed.generation() > expected.generation() {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable
        } else if observed.generation() == expected.generation() {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::Fork
        } else {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::Rollback
        });
    }
    Ok(())
}
fn validate_checkpoint_scope_policy(
    scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
    policy: MusubiProviderAttestationJournalPolicyV1,
) -> Result<[u8; 32], MusubiProviderAttestationJournalCheckpointSealErrorV1> {
    scope.validate()?;
    let policy_digest = policy
        .digest()
        .map_err(|_| MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidScope)?;
    if policy_digest != scope.journal_policy_digest {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidScope);
    }
    scope.scope_digest()
}
fn validate_checkpoint_blob(
    scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
    policy: MusubiProviderAttestationJournalPolicyV1,
    head: MusubiProviderAttestationJournalCheckpointHeadV1,
    checkpoint_blob: &[u8],
) -> Result<(), MusubiProviderAttestationJournalCheckpointSealErrorV1> {
    if checkpoint_blob.len() > policy.checkpoint_max_bytes
        || musubi_provider_attestation_journal_checkpoint_blob_revision_v1(checkpoint_blob)?
            != head.checkpoint_revision
    {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob);
    }
    let (checkpoint_sequence, last_observed_unix_ms) =
        validate_musubi_provider_attestation_journal_checkpoint_metadata_v1(
            checkpoint_blob,
            policy,
            &scope.network_id,
            scope.provider_id,
        )
        .map_err(|_| MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob)?;
    if checkpoint_sequence != head.checkpoint_sequence
        || last_observed_unix_ms != head.last_observed_unix_ms
    {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob);
    }
    Ok(())
}
async fn load_checkpoint_clock_floor(
    scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
    seal_binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<u64, MusubiProviderAttestationJournalCheckpointSealErrorV1> {
    let clock_scope =
        MusubiProviderAttestationClockScopeV1::try_new(scope.network_id, scope.provider_id)
            .map_err(map_clock_checkpoint_error)?;
    let clock_scope_digest = clock_scope
        .scope_digest()
        .map_err(map_clock_checkpoint_error)?;
    let record = load_authoritative(clock_scope_digest, seal_binding, seal)
        .await
        .map_err(map_clock_checkpoint_error)?
        .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::Uninitialized)?;
    Ok(record.floor_unix_ms())
}
async fn put_checkpoint_blob_and_readback(
    scope_digest: [u8; 32],
    checkpoint_revision: [u8; 32],
    checkpoint_blob: &[u8],
    seal_binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<(), MusubiProviderAttestationJournalCheckpointSealErrorV1> {
    if musubi_provider_attestation_journal_checkpoint_blob_revision_v1(checkpoint_blob)?
        != checkpoint_revision
    {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob);
    }
    qualify_checkpoint_seal(seal_binding, seal)?;
    let put_result = tokio::time::timeout(
        Duration::from_millis(MUSUBI_PROVIDER_ATTESTATION_CLOCK_SEAL_TIMEOUT_MS_V1),
        seal.put_journal_checkpoint_blob(scope_digest, checkpoint_revision, checkpoint_blob),
    )
    .await
    .map_err(|_| MusubiProviderAttestationClockSealErrorV1::Ambiguous)
    .and_then(|result| result);
    let readback_result =
        load_checkpoint_blob_authoritative(scope_digest, checkpoint_revision, seal_binding, seal)
            .await;
    let readback = match readback_result {
        Ok(readback) => readback,
        Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable)
            if mutation_outcome_is_ambiguous(&put_result) =>
        {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealAmbiguous);
        }
        Err(error) => return Err(error),
    };
    if readback.as_deref() == Some(checkpoint_blob) {
        return Ok(());
    }
    match (put_result, readback) {
        (_, Some(_)) => Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob),
        (Err(MusubiProviderAttestationClockSealErrorV1::Ambiguous), None) => {
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealAmbiguous)
        }
        (Err(MusubiProviderAttestationClockSealErrorV1::Unavailable), None) => {
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable)
        }
        (Err(MusubiProviderAttestationClockSealErrorV1::Rejected), None) | (Ok(()), None) => {
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealRejected)
        }
    }
}
async fn load_checkpoint_blob_authoritative(
    scope_digest: [u8; 32],
    checkpoint_revision: [u8; 32],
    seal_binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<Option<Vec<u8>>, MusubiProviderAttestationJournalCheckpointSealErrorV1> {
    if scope_digest == [0; 32] || checkpoint_revision == [0; 32] {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob);
    }
    qualify_checkpoint_seal(seal_binding, seal)?;
    let loaded = tokio::time::timeout(
        Duration::from_millis(MUSUBI_PROVIDER_ATTESTATION_CLOCK_SEAL_TIMEOUT_MS_V1),
        seal.load_journal_checkpoint_blob(scope_digest, checkpoint_revision),
    )
    .await
    .map_err(|_| MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable)?
    .map_err(map_checkpoint_seal_error)?;
    qualify_checkpoint_seal(seal_binding, seal)?;
    if let Some(blob) = &loaded {
        if musubi_provider_attestation_journal_checkpoint_blob_revision_v1(blob)?
            != checkpoint_revision
        {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob);
        }
    }
    Ok(loaded)
}
async fn load_checkpoint_head_authoritative(
    scope_digest: [u8; 32],
    seal_binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<
    Option<MusubiProviderAttestationJournalCheckpointHeadRecordV1>,
    MusubiProviderAttestationJournalCheckpointSealErrorV1,
> {
    if scope_digest == [0; 32] {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidScope);
    }
    qualify_checkpoint_seal(seal_binding, seal)?;
    let loaded = tokio::time::timeout(
        Duration::from_millis(MUSUBI_PROVIDER_ATTESTATION_CLOCK_SEAL_TIMEOUT_MS_V1),
        seal.load_journal_checkpoint_head(scope_digest),
    )
    .await
    .map_err(|_| MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable)?
    .map_err(map_checkpoint_seal_error)?;
    qualify_checkpoint_seal(seal_binding, seal)?;
    if let Some(record) = &loaded {
        record.validate(scope_digest)?;
    }
    Ok(loaded)
}
async fn load_checkpoint_head_record_authoritative(
    scope_digest: [u8; 32],
    record_digest: [u8; 32],
    seal_binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<
    Option<MusubiProviderAttestationJournalCheckpointHeadRecordV1>,
    MusubiProviderAttestationJournalCheckpointSealErrorV1,
> {
    if scope_digest == [0; 32] || record_digest == [0; 32] {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord);
    }
    qualify_checkpoint_seal(seal_binding, seal)?;
    let loaded = tokio::time::timeout(
        Duration::from_millis(MUSUBI_PROVIDER_ATTESTATION_CLOCK_SEAL_TIMEOUT_MS_V1),
        seal.load_journal_checkpoint_head_record(scope_digest, record_digest),
    )
    .await
    .map_err(|_| MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable)?
    .map_err(map_checkpoint_seal_error)?;
    qualify_checkpoint_seal(seal_binding, seal)?;
    if let Some(record) = &loaded {
        record.validate(scope_digest)?;
        if record.record_digest() != record_digest {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord);
        }
    }
    Ok(loaded)
}
async fn commit_checkpoint_head_and_readback(
    scope_digest: [u8; 32],
    seal_binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
    expected: Option<&MusubiProviderAttestationJournalCheckpointHeadRecordV1>,
    next: &MusubiProviderAttestationJournalCheckpointHeadRecordV1,
) -> Result<
    MusubiProviderAttestationJournalCheckpointHeadRecordV1,
    MusubiProviderAttestationJournalCheckpointSealErrorV1,
> {
    next.validate(scope_digest)?;
    if let Some(expected) = expected {
        next.validate_successor_of(expected)?;
    } else if next.generation() != 1
        || next.predecessor_record_digest().is_some()
        || next.head().is_some()
    {
        return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidLineage);
    }
    qualify_checkpoint_seal(seal_binding, seal)?;
    let expected_digest =
        expected.map(MusubiProviderAttestationJournalCheckpointHeadRecordV1::record_digest);
    let cas_result = tokio::time::timeout(
        Duration::from_millis(MUSUBI_PROVIDER_ATTESTATION_CLOCK_SEAL_TIMEOUT_MS_V1),
        seal.compare_and_swap_journal_checkpoint_head(scope_digest, expected_digest, next),
    )
    .await
    .map_err(|_| MusubiProviderAttestationClockSealErrorV1::Ambiguous)
    .and_then(|result| result);
    let readback_result =
        load_checkpoint_head_authoritative(scope_digest, seal_binding, seal).await;
    let readback = match readback_result {
        Ok(readback) => readback,
        Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable)
            if mutation_outcome_is_ambiguous(&cas_result) =>
        {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealAmbiguous);
        }
        Err(error) => return Err(error),
    };
    if readback.as_ref() == Some(next) {
        let retained_next = load_checkpoint_head_record_authoritative(
            scope_digest,
            next.record_digest(),
            seal_binding,
            seal,
        )
        .await?
        .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord)?;
        if retained_next != *next {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord);
        }
        if let Some(expected) = expected {
            let retained_predecessor = load_checkpoint_head_record_authoritative(
                scope_digest,
                expected.record_digest(),
                seal_binding,
                seal,
            )
            .await?
            .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord)?;
            if retained_predecessor != *expected {
                return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord);
            }
        }
        return Ok(next.clone());
    }
    if let Some(authoritative) = &readback {
        if authoritative.generation() > next.generation() {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable);
        }
        if authoritative.generation() == next.generation() {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::Fork);
        }
    }
    match (cas_result, readback) {
        (Err(MusubiProviderAttestationClockSealErrorV1::Ambiguous), _) => {
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealAmbiguous)
        }
        (_, None) if expected.is_some() => {
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::Rollback)
        }
        (Err(MusubiProviderAttestationClockSealErrorV1::Unavailable), _) => {
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable)
        }
        (Err(MusubiProviderAttestationClockSealErrorV1::Rejected), _) | (Ok(()), _) => {
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealRejected)
        }
    }
}
fn classify_checkpoint_head_change(
    expected: &MusubiProviderAttestationJournalCheckpointHeadRecordV1,
    authoritative: &MusubiProviderAttestationJournalCheckpointHeadRecordV1,
) -> MusubiProviderAttestationJournalCheckpointSealErrorV1 {
    if authoritative.generation() > expected.generation() {
        MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable
    } else if authoritative.generation() == expected.generation() {
        MusubiProviderAttestationJournalCheckpointSealErrorV1::Fork
    } else {
        MusubiProviderAttestationJournalCheckpointSealErrorV1::Rollback
    }
}
fn mutation_outcome_is_ambiguous(
    outcome: &Result<(), MusubiProviderAttestationClockSealErrorV1>,
) -> bool {
    matches!(
        outcome,
        Err(MusubiProviderAttestationClockSealErrorV1::Ambiguous)
    )
}
fn qualify_checkpoint_seal(
    expected: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<(), MusubiProviderAttestationJournalCheckpointSealErrorV1> {
    qualify_seal(expected, seal).map_err(map_clock_checkpoint_error)
}
fn map_checkpoint_seal_error(
    error: MusubiProviderAttestationClockSealErrorV1,
) -> MusubiProviderAttestationJournalCheckpointSealErrorV1 {
    match error {
        MusubiProviderAttestationClockSealErrorV1::Unavailable => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable
        }
        MusubiProviderAttestationClockSealErrorV1::Rejected => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::SealRejected
        }
        MusubiProviderAttestationClockSealErrorV1::Ambiguous => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::SealAmbiguous
        }
    }
}
fn map_clock_checkpoint_error(
    error: MusubiProviderAttestationClockErrorV1,
) -> MusubiProviderAttestationJournalCheckpointSealErrorV1 {
    match error {
        MusubiProviderAttestationClockErrorV1::InvalidScope => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidScope
        }
        MusubiProviderAttestationClockErrorV1::InvalidSealBinding => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidSealBinding
        }
        MusubiProviderAttestationClockErrorV1::Uninitialized => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::Uninitialized
        }
        MusubiProviderAttestationClockErrorV1::AlreadyInitialized
        | MusubiProviderAttestationClockErrorV1::SealRejected => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::SealRejected
        }
        MusubiProviderAttestationClockErrorV1::InvalidSealRecord => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord
        }
        MusubiProviderAttestationClockErrorV1::SealUnavailable => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable
        }
        MusubiProviderAttestationClockErrorV1::SealAmbiguous => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::SealAmbiguous
        }
        MusubiProviderAttestationClockErrorV1::ClockRollback => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::Rollback
        }
        MusubiProviderAttestationClockErrorV1::ArithmeticOverflow => {
            MusubiProviderAttestationJournalCheckpointSealErrorV1::ArithmeticOverflow
        }
    }
}
/// Stable failure from the qualified journal clock boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MusubiProviderAttestationClockErrorV1 {
    /// The network/provider scope is inert or malformed.
    #[error("Musubi provider-attestation clock scope is invalid")]
    InvalidScope,
    /// The configured or live seal identity/qualification is invalid.
    #[error("Musubi provider-attestation clock seal binding is invalid")]
    InvalidSealBinding,
    /// Ordinary restart found no previously initialized seal record.
    #[error("Musubi provider-attestation clock seal is uninitialized")]
    Uninitialized,
    /// One-time initialization found an existing seal record.
    #[error("Musubi provider-attestation clock seal is already initialized")]
    AlreadyInitialized,
    /// The authoritative seal record is malformed or substituted.
    #[error("Musubi provider-attestation clock seal record is invalid")]
    InvalidSealRecord,
    /// The seal was unavailable or timed out.
    #[error("Musubi provider-attestation clock seal is unavailable")]
    SealUnavailable,
    /// The seal rejected or contradicted the requested transition.
    #[error("Musubi provider-attestation clock seal rejected the transition")]
    SealRejected,
    /// The seal outcome remained ambiguous after authoritative readback.
    #[error("Musubi provider-attestation clock seal outcome is ambiguous")]
    SealAmbiguous,
    /// The host clock or authoritative record regressed.
    #[error("Musubi provider-attestation UNIX clock moved backwards")]
    ClockRollback,
    /// A record generation or system-time conversion overflowed.
    #[error("Musubi provider-attestation clock counter overflowed")]
    ArithmeticOverflow,
}
/// UNIX clock whose every returned value is committed by a qualified seal.
///
/// Construction samples no caller-provided timestamp. Tests exercise the same
/// state machine through crate-private sampled helpers; production callers can
/// only initialize/open and request a fresh host UNIX-time sample.
// Daemon activation remains closed until the supervised provider-ingest layer
// constructs the sealed file-store runtime through its explicit initialize/open
// paths with a production-qualified combined durability provider and accepted
// deployment crash/corruption evidence.
pub struct MusubiProviderAttestationSealedUnixClockV1 {
    scope_digest: [u8; 32],
    seal_binding: MusubiProviderAttestationClockSealBindingV1,
    seal: Arc<dyn MusubiProviderAttestationClockSealV1>,
    state: Mutex<MusubiProviderAttestationClockSealRecordV1>,
}
impl fmt::Debug for MusubiProviderAttestationSealedUnixClockV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MusubiProviderAttestationSealedUnixClockV1")
            .field("scope_digest", &self.scope_digest)
            .field("seal_binding", &self.seal_binding)
            .finish_non_exhaustive()
    }
}
impl MusubiProviderAttestationSealedUnixClockV1 {
    /// Explicitly initialize an absent external monotonic seal.
    ///
    /// # Errors
    ///
    /// Fails closed when the seal is already initialized, unavailable,
    /// substituted, ambiguous after readback, or contradicts host UNIX time.
    pub async fn initialize(
        scope: MusubiProviderAttestationClockScopeV1,
        seal_binding: MusubiProviderAttestationClockSealBindingV1,
        seal: Arc<dyn MusubiProviderAttestationClockSealV1>,
    ) -> Result<Self, MusubiProviderAttestationClockErrorV1> {
        let sampled = system_unix_ms()?;
        Self::open_inner(scope, seal_binding, seal, true, sampled).await
    }
    /// Open an existing external monotonic seal and advance it to host time.
    ///
    /// # Errors
    ///
    /// Fails closed for missing or invalid restart state, identity changes,
    /// rollback, timeout, or an unresolved compare-and-swap outcome.
    pub async fn open(
        scope: MusubiProviderAttestationClockScopeV1,
        seal_binding: MusubiProviderAttestationClockSealBindingV1,
        seal: Arc<dyn MusubiProviderAttestationClockSealV1>,
    ) -> Result<Self, MusubiProviderAttestationClockErrorV1> {
        let sampled = system_unix_ms()?;
        Self::open_inner(scope, seal_binding, seal, false, sampled).await
    }
    async fn open_inner(
        scope: MusubiProviderAttestationClockScopeV1,
        seal_binding: MusubiProviderAttestationClockSealBindingV1,
        seal: Arc<dyn MusubiProviderAttestationClockSealV1>,
        initialize: bool,
        sampled: u64,
    ) -> Result<Self, MusubiProviderAttestationClockErrorV1> {
        scope.validate()?;
        seal_binding.qualification.validate()?;
        let scope_digest = scope.scope_digest()?;
        qualify_seal(&seal_binding, seal.as_ref())?;
        let loaded = load_authoritative(scope_digest, &seal_binding, seal.as_ref()).await?;
        let current = match (initialize, loaded) {
            (true, Some(_)) => {
                return Err(MusubiProviderAttestationClockErrorV1::AlreadyInitialized);
            }
            (false, None) => {
                return Err(MusubiProviderAttestationClockErrorV1::Uninitialized);
            }
            (true, None) => None,
            (false, Some(record)) => Some(record),
        };
        if sampled == 0
            || current
                .as_ref()
                .is_some_and(|record| sampled < record.floor_unix_ms())
        {
            return Err(MusubiProviderAttestationClockErrorV1::ClockRollback);
        }
        let record = match current {
            None => {
                let next =
                    MusubiProviderAttestationClockSealRecordV1::initial(scope_digest, sampled)?;
                commit_and_readback(scope_digest, &seal_binding, seal.as_ref(), None, &next).await?
            }
            Some(record) if sampled == record.floor_unix_ms() => record,
            Some(record) => {
                let next = MusubiProviderAttestationClockSealRecordV1::successor(&record, sampled)?;
                commit_and_readback(
                    scope_digest,
                    &seal_binding,
                    seal.as_ref(),
                    Some(&record),
                    &next,
                )
                .await?
            }
        };
        qualify_seal(&seal_binding, seal.as_ref())?;
        Ok(Self {
            scope_digest,
            seal_binding,
            seal,
            state: Mutex::new(record),
        })
    }
    /// Sample host UNIX time and return it only after exact sealed readback.
    ///
    /// # Errors
    ///
    /// Fails closed on wall-clock rollback, missing/changed seal state,
    /// qualification changes, timeouts, or ambiguous advancement.
    pub async fn now_unix_ms(&self) -> Result<u64, MusubiProviderAttestationClockErrorV1> {
        let sampled = system_unix_ms()?;
        self.advance_to(sampled).await
    }
    async fn advance_to(&self, sampled: u64) -> Result<u64, MusubiProviderAttestationClockErrorV1> {
        let mut retained = self.state.lock().await;
        qualify_seal(&self.seal_binding, self.seal.as_ref())?;
        let authoritative =
            load_authoritative(self.scope_digest, &self.seal_binding, self.seal.as_ref())
                .await?
                .ok_or(MusubiProviderAttestationClockErrorV1::ClockRollback)?;
        if authoritative != *retained {
            if is_exact_successor(&retained, &authoritative) {
                // A caller may be cancelled after the external CAS commits but
                // before `commit_and_readback` can publish the new local floor.
                // Recover only that exact predecessor-linked successor; every
                // other divergence remains a closed rollback/substitution
                // failure and requires an ordinary restart audit.
                *retained = authoritative;
            } else {
                return Err(classify_record_change(&retained, &authoritative));
            }
        }
        if sampled == 0 || sampled < retained.floor_unix_ms() {
            return Err(MusubiProviderAttestationClockErrorV1::ClockRollback);
        }
        if sampled > retained.floor_unix_ms() {
            let next = MusubiProviderAttestationClockSealRecordV1::successor(&retained, sampled)?;
            *retained = commit_and_readback(
                self.scope_digest,
                &self.seal_binding,
                self.seal.as_ref(),
                Some(&*retained),
                &next,
            )
            .await?;
        }
        qualify_seal(&self.seal_binding, self.seal.as_ref())?;
        Ok(sampled)
    }
    /// Return the currently retained sealed floor after serializing with a sample.
    pub async fn durable_floor_unix_ms(&self) -> u64 {
        self.state.lock().await.floor_unix_ms()
    }
    /// Return the non-secret network/provider scope commitment.
    ///
    /// A deployment constructor can compare this value with the independently
    /// derived file-store binding before exposing the combined journal runtime.
    #[must_use]
    pub const fn scope_digest(&self) -> [u8; 32] {
        self.scope_digest
    }
    pub(crate) async fn initialize_journal_checkpoint_seal(
        &self,
        scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
    ) -> Result<
        MusubiProviderAttestationJournalCheckpointHeadRecordV1,
        MusubiProviderAttestationJournalCheckpointSealErrorV1,
    > {
        self.validate_journal_checkpoint_scope_binding(scope)?;
        initialize_musubi_provider_attestation_journal_checkpoint_seal_v1(
            scope,
            &self.seal_binding,
            self.seal.as_ref(),
        )
        .await
    }
    pub(crate) async fn load_journal_checkpoint(
        &self,
        scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
        policy: MusubiProviderAttestationJournalPolicyV1,
    ) -> Result<
        (
            MusubiProviderAttestationJournalCheckpointHeadRecordV1,
            Option<Vec<u8>>,
        ),
        MusubiProviderAttestationJournalCheckpointSealErrorV1,
    > {
        self.validate_journal_checkpoint_scope_binding(scope)?;
        load_musubi_provider_attestation_journal_checkpoint_v1(
            scope,
            policy,
            &self.seal_binding,
            self.seal.as_ref(),
        )
        .await
    }
    pub(crate) async fn seal_journal_checkpoint(
        &self,
        scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
        policy: MusubiProviderAttestationJournalPolicyV1,
        expected: &MusubiProviderAttestationJournalCheckpointHeadRecordV1,
        head: MusubiProviderAttestationJournalCheckpointHeadV1,
        checkpoint_blob: &[u8],
    ) -> Result<
        MusubiProviderAttestationJournalCheckpointHeadRecordV1,
        MusubiProviderAttestationJournalCheckpointSealErrorV1,
    > {
        self.validate_journal_checkpoint_scope_binding(scope)?;
        seal_musubi_provider_attestation_journal_checkpoint_v1(
            scope,
            policy,
            &self.seal_binding,
            self.seal.as_ref(),
            expected,
            head,
            checkpoint_blob,
        )
        .await
    }
    pub(crate) async fn load_journal_checkpoint_direct_predecessor(
        &self,
        scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
        policy: MusubiProviderAttestationJournalPolicyV1,
        current: &MusubiProviderAttestationJournalCheckpointHeadRecordV1,
    ) -> Result<
        (
            MusubiProviderAttestationJournalCheckpointHeadRecordV1,
            Option<Vec<u8>>,
        ),
        MusubiProviderAttestationJournalCheckpointSealErrorV1,
    > {
        self.validate_journal_checkpoint_scope_binding(scope)?;
        let scope_digest = validate_checkpoint_scope_policy(scope, policy)?;
        current.validate(scope_digest)?;
        let predecessor_digest = current
            .predecessor_record_digest()
            .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidLineage)?;
        let predecessor = load_checkpoint_head_record_authoritative(
            scope_digest,
            predecessor_digest,
            &self.seal_binding,
            self.seal.as_ref(),
        )
        .await?
        .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord)?;
        current.validate_successor_of(&predecessor)?;
        let Some(head) = predecessor.head() else {
            return Ok((predecessor, None));
        };
        let blob = load_checkpoint_blob_authoritative(
            scope_digest,
            head.checkpoint_revision(),
            &self.seal_binding,
            self.seal.as_ref(),
        )
        .await?
        .ok_or(MusubiProviderAttestationJournalCheckpointSealErrorV1::MissingBlob)?;
        validate_checkpoint_blob(scope, policy, head, &blob)?;
        let clock_floor =
            load_checkpoint_clock_floor(scope, &self.seal_binding, self.seal.as_ref()).await?;
        if head.last_observed_unix_ms() > clock_floor {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidHead);
        }
        Ok((predecessor, Some(blob)))
    }
    fn validate_journal_checkpoint_scope_binding(
        &self,
        scope: &MusubiProviderAttestationJournalCheckpointScopeV1,
    ) -> Result<(), MusubiProviderAttestationJournalCheckpointSealErrorV1> {
        scope.validate()?;
        let clock_scope = MusubiProviderAttestationClockScopeV1::try_new(
            *scope.network_id(),
            scope.provider_id(),
        )
        .map_err(map_clock_checkpoint_error)?;
        let expected = clock_scope
            .scope_digest()
            .map_err(map_clock_checkpoint_error)?;
        if expected != self.scope_digest {
            return Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidScope);
        }
        Ok(())
    }
    #[cfg(test)]
    async fn initialize_at(
        scope: MusubiProviderAttestationClockScopeV1,
        seal_binding: MusubiProviderAttestationClockSealBindingV1,
        seal: Arc<dyn MusubiProviderAttestationClockSealV1>,
        sampled: u64,
    ) -> Result<Self, MusubiProviderAttestationClockErrorV1> {
        Self::open_inner(scope, seal_binding, seal, true, sampled).await
    }
    #[cfg(test)]
    async fn open_at(
        scope: MusubiProviderAttestationClockScopeV1,
        seal_binding: MusubiProviderAttestationClockSealBindingV1,
        seal: Arc<dyn MusubiProviderAttestationClockSealV1>,
        sampled: u64,
    ) -> Result<Self, MusubiProviderAttestationClockErrorV1> {
        Self::open_inner(scope, seal_binding, seal, false, sampled).await
    }
    #[cfg(test)]
    async fn now_at(&self, sampled: u64) -> Result<u64, MusubiProviderAttestationClockErrorV1> {
        self.advance_to(sampled).await
    }
}
fn qualify_seal(
    expected: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<(), MusubiProviderAttestationClockErrorV1> {
    let handle_before = seal.runtime_handle().to_owned();
    if handle_before != expected.runtime_handle || !is_production_runtime_handle(&handle_before) {
        return Err(MusubiProviderAttestationClockErrorV1::InvalidSealBinding);
    }
    let qualification = seal.qualification().map_err(map_seal_error)?;
    qualification.validate()?;
    let handle_after = seal.runtime_handle();
    if handle_after != handle_before
        || !is_production_runtime_handle(handle_after)
        || qualification != expected.qualification
    {
        return Err(MusubiProviderAttestationClockErrorV1::InvalidSealBinding);
    }
    Ok(())
}
async fn load_authoritative(
    scope_digest: [u8; 32],
    binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
) -> Result<Option<MusubiProviderAttestationClockSealRecordV1>, MusubiProviderAttestationClockErrorV1>
{
    qualify_seal(binding, seal)?;
    let loaded = tokio::time::timeout(
        Duration::from_millis(MUSUBI_PROVIDER_ATTESTATION_CLOCK_SEAL_TIMEOUT_MS_V1),
        seal.load_latest(scope_digest),
    )
    .await
    .map_err(|_| MusubiProviderAttestationClockErrorV1::SealUnavailable)?
    .map_err(map_seal_error)?;
    qualify_seal(binding, seal)?;
    if let Some(record) = &loaded {
        record.validate(scope_digest)?;
    }
    Ok(loaded)
}
async fn commit_and_readback(
    scope_digest: [u8; 32],
    binding: &MusubiProviderAttestationClockSealBindingV1,
    seal: &dyn MusubiProviderAttestationClockSealV1,
    expected: Option<&MusubiProviderAttestationClockSealRecordV1>,
    next: &MusubiProviderAttestationClockSealRecordV1,
) -> Result<MusubiProviderAttestationClockSealRecordV1, MusubiProviderAttestationClockErrorV1> {
    next.validate(scope_digest)?;
    qualify_seal(binding, seal)?;
    let expected_digest = expected.map(MusubiProviderAttestationClockSealRecordV1::record_digest);
    let cas_result = tokio::time::timeout(
        Duration::from_millis(MUSUBI_PROVIDER_ATTESTATION_CLOCK_SEAL_TIMEOUT_MS_V1),
        seal.compare_and_swap(scope_digest, expected_digest, next),
    )
    .await
    .map_err(|_| MusubiProviderAttestationClockSealErrorV1::Ambiguous)
    .and_then(|result| result);
    let readback_result = load_authoritative(scope_digest, binding, seal).await;
    let readback = match readback_result {
        Ok(readback) => readback,
        Err(MusubiProviderAttestationClockErrorV1::SealUnavailable)
            if mutation_outcome_is_ambiguous(&cas_result) =>
        {
            return Err(MusubiProviderAttestationClockErrorV1::SealAmbiguous);
        }
        Err(error) => return Err(error),
    };
    if readback.as_ref() == Some(next) {
        return Ok(next.clone());
    }
    if let Some(authoritative) = &readback {
        if is_exact_successor(next, authoritative) {
            return Ok(authoritative.clone());
        }
        if expected.is_some_and(|expected| {
            authoritative.generation() < expected.generation()
                || authoritative.floor_unix_ms() < expected.floor_unix_ms()
        }) {
            return Err(MusubiProviderAttestationClockErrorV1::ClockRollback);
        }
        if authoritative.generation() > next.generation() {
            return Err(MusubiProviderAttestationClockErrorV1::SealUnavailable);
        }
    }
    match (cas_result, readback) {
        (Err(MusubiProviderAttestationClockSealErrorV1::Ambiguous), _) => {
            Err(MusubiProviderAttestationClockErrorV1::SealAmbiguous)
        }
        (_, None) if expected.is_some() => {
            Err(MusubiProviderAttestationClockErrorV1::ClockRollback)
        }
        (Err(MusubiProviderAttestationClockSealErrorV1::Unavailable), _) => {
            Err(MusubiProviderAttestationClockErrorV1::SealUnavailable)
        }
        (Err(MusubiProviderAttestationClockSealErrorV1::Rejected), _) | (Ok(()), _) => {
            Err(MusubiProviderAttestationClockErrorV1::SealRejected)
        }
    }
}
fn classify_record_change(
    retained: &MusubiProviderAttestationClockSealRecordV1,
    authoritative: &MusubiProviderAttestationClockSealRecordV1,
) -> MusubiProviderAttestationClockErrorV1 {
    if authoritative.generation() < retained.generation()
        || authoritative.floor_unix_ms() < retained.floor_unix_ms()
    {
        MusubiProviderAttestationClockErrorV1::ClockRollback
    } else if authoritative.generation() > retained.generation() {
        MusubiProviderAttestationClockErrorV1::SealUnavailable
    } else {
        MusubiProviderAttestationClockErrorV1::SealRejected
    }
}
fn is_exact_successor(
    retained: &MusubiProviderAttestationClockSealRecordV1,
    authoritative: &MusubiProviderAttestationClockSealRecordV1,
) -> bool {
    retained.generation().checked_add(1) == Some(authoritative.generation())
        && authoritative.predecessor_digest() == Some(retained.record_digest())
        && authoritative.floor_unix_ms() >= retained.floor_unix_ms()
}
fn map_seal_error(
    error: MusubiProviderAttestationClockSealErrorV1,
) -> MusubiProviderAttestationClockErrorV1 {
    match error {
        MusubiProviderAttestationClockSealErrorV1::Unavailable => {
            MusubiProviderAttestationClockErrorV1::SealUnavailable
        }
        MusubiProviderAttestationClockSealErrorV1::Rejected => {
            MusubiProviderAttestationClockErrorV1::SealRejected
        }
        MusubiProviderAttestationClockSealErrorV1::Ambiguous => {
            MusubiProviderAttestationClockErrorV1::SealAmbiguous
        }
    }
}
fn system_unix_ms() -> Result<u64, MusubiProviderAttestationClockErrorV1> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| MusubiProviderAttestationClockErrorV1::ClockRollback)?;
    u64::try_from(elapsed.as_millis())
        .ok()
        .filter(|value| *value != 0)
        .ok_or(MusubiProviderAttestationClockErrorV1::ArithmeticOverflow)
}
fn domain_hash_norito<T: norito::core::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Option<[u8; 32]> {
    let canonical = norito::encode_canonical(value).ok()?;
    let domain_len = u64::try_from(domain.len()).ok()?;
    let canonical_len = u64::try_from(canonical.len()).ok()?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&domain_len.to_be_bytes());
    hasher.update(domain);
    hasher.update(&canonical_len.to_be_bytes());
    hasher.update(&canonical);
    let digest = *hasher.finalize().as_bytes();
    (digest != [0; 32]).then_some(digest)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_attestation_journal::musubi_provider_attestation_journal_test_checkpoint_bytes_v1;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
    use std::{
        collections::BTreeMap,
        sync::{
            Mutex as StdMutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
    };
    const HANDLE: &str = "provider://musubi/provider-attestation-clock/seal";
    type CheckpointKey = ([u8; 32], [u8; 32]);
    type CheckpointBlobs = BTreeMap<CheckpointKey, Vec<u8>>;
    type CheckpointHeadRecords =
        BTreeMap<CheckpointKey, MusubiProviderAttestationJournalCheckpointHeadRecordV1>;
    #[derive(Debug)]
    struct TestSeal {
        qualification: StdMutex<MusubiProviderAttestationClockSealQualificationV1>,
        record: StdMutex<Option<MusubiProviderAttestationClockSealRecordV1>>,
        next_cas_error: StdMutex<Option<MusubiProviderAttestationClockSealErrorV1>>,
        next_load_error: StdMutex<Option<MusubiProviderAttestationClockSealErrorV1>>,
        clock_cas_readback_unavailable: AtomicBool,
        pause_after_clock_cas: AtomicBool,
        clock_cas_reached: tokio::sync::Notify,
        resume_after_clock_cas: tokio::sync::Notify,
        checkpoint_blobs: StdMutex<CheckpointBlobs>,
        next_checkpoint_blob_put_error: StdMutex<Option<MusubiProviderAttestationClockSealErrorV1>>,
        next_checkpoint_blob_load_error:
            StdMutex<Option<MusubiProviderAttestationClockSealErrorV1>>,
        checkpoint_blob_put_readback_unavailable: AtomicBool,
        checkpoint_heads:
            StdMutex<BTreeMap<[u8; 32], MusubiProviderAttestationJournalCheckpointHeadRecordV1>>,
        checkpoint_head_records: StdMutex<CheckpointHeadRecords>,
        next_checkpoint_head_cas_error: StdMutex<Option<MusubiProviderAttestationClockSealErrorV1>>,
        next_checkpoint_head_load_error:
            StdMutex<Option<MusubiProviderAttestationClockSealErrorV1>>,
        checkpoint_head_cas_readback_unavailable: AtomicBool,
        next_checkpoint_head_substitute:
            StdMutex<Option<MusubiProviderAttestationJournalCheckpointHeadRecordV1>>,
        drift_after_checkpoint_blob_put: AtomicBool,
    }
    impl TestSeal {
        fn new() -> Self {
            Self {
                qualification: StdMutex::new(qualification()),
                record: StdMutex::new(None),
                next_cas_error: StdMutex::new(None),
                next_load_error: StdMutex::new(None),
                clock_cas_readback_unavailable: AtomicBool::new(false),
                pause_after_clock_cas: AtomicBool::new(false),
                clock_cas_reached: tokio::sync::Notify::new(),
                resume_after_clock_cas: tokio::sync::Notify::new(),
                checkpoint_blobs: StdMutex::new(BTreeMap::new()),
                next_checkpoint_blob_put_error: StdMutex::new(None),
                next_checkpoint_blob_load_error: StdMutex::new(None),
                checkpoint_blob_put_readback_unavailable: AtomicBool::new(false),
                checkpoint_heads: StdMutex::new(BTreeMap::new()),
                checkpoint_head_records: StdMutex::new(BTreeMap::new()),
                next_checkpoint_head_cas_error: StdMutex::new(None),
                next_checkpoint_head_load_error: StdMutex::new(None),
                checkpoint_head_cas_readback_unavailable: AtomicBool::new(false),
                next_checkpoint_head_substitute: StdMutex::new(None),
                drift_after_checkpoint_blob_put: AtomicBool::new(false),
            }
        }
        fn binding(&self) -> MusubiProviderAttestationClockSealBindingV1 {
            MusubiProviderAttestationClockSealBindingV1::try_new(
                HANDLE,
                *self.qualification.lock().expect("qualification lock"),
            )
            .expect("test binding")
        }
        fn replace_record(&self, record: Option<MusubiProviderAttestationClockSealRecordV1>) {
            *self.record.lock().expect("record lock") = record;
        }
        fn fail_next_cas(&self, error: MusubiProviderAttestationClockSealErrorV1) {
            *self.next_cas_error.lock().expect("CAS error lock") = Some(error);
        }
        fn make_next_clock_cas_readback_unavailable(&self) {
            self.clock_cas_readback_unavailable
                .store(true, Ordering::SeqCst);
        }
        fn pause_after_next_clock_cas(&self) {
            self.pause_after_clock_cas.store(true, Ordering::SeqCst);
        }
        fn fail_next_checkpoint_blob_put(&self, error: MusubiProviderAttestationClockSealErrorV1) {
            *self
                .next_checkpoint_blob_put_error
                .lock()
                .expect("checkpoint blob put error lock") = Some(error);
        }
        fn make_next_checkpoint_blob_put_readback_unavailable(&self) {
            self.checkpoint_blob_put_readback_unavailable
                .store(true, Ordering::SeqCst);
        }
        fn fail_next_checkpoint_head_cas(&self, error: MusubiProviderAttestationClockSealErrorV1) {
            *self
                .next_checkpoint_head_cas_error
                .lock()
                .expect("checkpoint-head CAS error lock") = Some(error);
        }
        fn make_next_checkpoint_head_cas_readback_unavailable(&self) {
            self.checkpoint_head_cas_readback_unavailable
                .store(true, Ordering::SeqCst);
        }
        fn substitute_next_checkpoint_head(
            &self,
            record: MusubiProviderAttestationJournalCheckpointHeadRecordV1,
        ) {
            *self
                .next_checkpoint_head_substitute
                .lock()
                .expect("checkpoint-head substitute lock") = Some(record);
        }
        fn remove_checkpoint_blob(&self, scope_digest: [u8; 32], revision: [u8; 32]) {
            self.checkpoint_blobs
                .lock()
                .expect("checkpoint blob lock")
                .remove(&(scope_digest, revision));
        }
    }
    impl MusubiProviderAttestationClockSealV1 for TestSeal {
        fn runtime_handle(&self) -> &str {
            HANDLE
        }
        fn qualification(
            &self,
        ) -> Result<
            MusubiProviderAttestationClockSealQualificationV1,
            MusubiProviderAttestationClockSealErrorV1,
        > {
            Ok(*self.qualification.lock().expect("qualification lock"))
        }
        fn load_latest<'a>(
            &'a self,
            _scope_digest: [u8; 32],
        ) -> ProviderIngestFutureV1<
            'a,
            Result<
                Option<MusubiProviderAttestationClockSealRecordV1>,
                MusubiProviderAttestationClockSealErrorV1,
            >,
        > {
            Box::pin(async move {
                if let Some(error) = self.next_load_error.lock().expect("load error lock").take() {
                    return Err(error);
                }
                Ok(self.record.lock().expect("record lock").clone())
            })
        }
        fn compare_and_swap<'a>(
            &'a self,
            _scope_digest: [u8; 32],
            expected: Option<[u8; 32]>,
            next: &'a MusubiProviderAttestationClockSealRecordV1,
        ) -> ProviderIngestFutureV1<'a, Result<(), MusubiProviderAttestationClockSealErrorV1>>
        {
            Box::pin(async move {
                let error = self.next_cas_error.lock().expect("CAS error lock").take();
                {
                    let mut record = self.record.lock().expect("record lock");
                    if record.as_ref().map(|value| value.record_digest()) != expected {
                        return Err(MusubiProviderAttestationClockSealErrorV1::Rejected);
                    }
                    if error != Some(MusubiProviderAttestationClockSealErrorV1::Rejected) {
                        *record = Some(next.clone());
                    }
                }
                if self
                    .clock_cas_readback_unavailable
                    .swap(false, Ordering::SeqCst)
                {
                    *self.next_load_error.lock().expect("load error lock") =
                        Some(MusubiProviderAttestationClockSealErrorV1::Unavailable);
                }
                if self.pause_after_clock_cas.swap(false, Ordering::SeqCst) {
                    self.clock_cas_reached.notify_one();
                    self.resume_after_clock_cas.notified().await;
                }
                error.map_or(Ok(()), Err)
            })
        }
        fn put_journal_checkpoint_blob<'a>(
            &'a self,
            scope_digest: [u8; 32],
            checkpoint_revision: [u8; 32],
            checkpoint_blob: &'a [u8],
        ) -> ProviderIngestFutureV1<'a, Result<(), MusubiProviderAttestationClockSealErrorV1>>
        {
            Box::pin(async move {
                let error = self
                    .next_checkpoint_blob_put_error
                    .lock()
                    .expect("checkpoint blob put error lock")
                    .take();
                if scope_digest == [0; 32]
                    || musubi_provider_attestation_journal_checkpoint_blob_revision_v1(
                        checkpoint_blob,
                    )
                    .ok()
                        != Some(checkpoint_revision)
                {
                    return Err(MusubiProviderAttestationClockSealErrorV1::Rejected);
                }
                if error != Some(MusubiProviderAttestationClockSealErrorV1::Rejected) {
                    let mut blobs = self.checkpoint_blobs.lock().expect("checkpoint blob lock");
                    match blobs.get(&(scope_digest, checkpoint_revision)) {
                        Some(retained) if retained.as_slice() != checkpoint_blob => {
                            return Err(MusubiProviderAttestationClockSealErrorV1::Rejected);
                        }
                        Some(_) => {}
                        None => {
                            blobs.insert(
                                (scope_digest, checkpoint_revision),
                                checkpoint_blob.to_vec(),
                            );
                        }
                    }
                }
                if self
                    .checkpoint_blob_put_readback_unavailable
                    .swap(false, Ordering::SeqCst)
                {
                    *self
                        .next_checkpoint_blob_load_error
                        .lock()
                        .expect("checkpoint blob load error lock") =
                        Some(MusubiProviderAttestationClockSealErrorV1::Unavailable);
                }
                if self
                    .drift_after_checkpoint_blob_put
                    .swap(false, Ordering::SeqCst)
                {
                    *self.qualification.lock().expect("qualification lock") =
                        MusubiProviderAttestationClockSealQualificationV1::new(2, [0xA5; 32]);
                }
                error.map_or(Ok(()), Err)
            })
        }
        fn load_journal_checkpoint_blob<'a>(
            &'a self,
            scope_digest: [u8; 32],
            checkpoint_revision: [u8; 32],
        ) -> ProviderIngestFutureV1<
            'a,
            Result<Option<Vec<u8>>, MusubiProviderAttestationClockSealErrorV1>,
        > {
            Box::pin(async move {
                if let Some(error) = self
                    .next_checkpoint_blob_load_error
                    .lock()
                    .expect("checkpoint blob load error lock")
                    .take()
                {
                    return Err(error);
                }
                Ok(self
                    .checkpoint_blobs
                    .lock()
                    .expect("checkpoint blob lock")
                    .get(&(scope_digest, checkpoint_revision))
                    .cloned())
            })
        }
        fn load_journal_checkpoint_head<'a>(
            &'a self,
            scope_digest: [u8; 32],
        ) -> ProviderIngestFutureV1<
            'a,
            Result<
                Option<MusubiProviderAttestationJournalCheckpointHeadRecordV1>,
                MusubiProviderAttestationClockSealErrorV1,
            >,
        > {
            Box::pin(async move {
                if let Some(error) = self
                    .next_checkpoint_head_load_error
                    .lock()
                    .expect("checkpoint head load error lock")
                    .take()
                {
                    return Err(error);
                }
                Ok(self
                    .checkpoint_heads
                    .lock()
                    .expect("checkpoint-head lock")
                    .get(&scope_digest)
                    .cloned())
            })
        }
        fn load_journal_checkpoint_head_record<'a>(
            &'a self,
            scope_digest: [u8; 32],
            record_digest: [u8; 32],
        ) -> ProviderIngestFutureV1<
            'a,
            Result<
                Option<MusubiProviderAttestationJournalCheckpointHeadRecordV1>,
                MusubiProviderAttestationClockSealErrorV1,
            >,
        > {
            Box::pin(async move {
                Ok(self
                    .checkpoint_head_records
                    .lock()
                    .expect("checkpoint-head record lock")
                    .get(&(scope_digest, record_digest))
                    .cloned())
            })
        }
        fn compare_and_swap_journal_checkpoint_head<'a>(
            &'a self,
            scope_digest: [u8; 32],
            expected: Option<[u8; 32]>,
            next: &'a MusubiProviderAttestationJournalCheckpointHeadRecordV1,
        ) -> ProviderIngestFutureV1<'a, Result<(), MusubiProviderAttestationClockSealErrorV1>>
        {
            Box::pin(async move {
                let error = self
                    .next_checkpoint_head_cas_error
                    .lock()
                    .expect("checkpoint-head CAS error lock")
                    .take();
                let substitute = self
                    .next_checkpoint_head_substitute
                    .lock()
                    .expect("checkpoint-head substitute lock")
                    .take();
                let mut heads = self.checkpoint_heads.lock().expect("checkpoint-head lock");
                if next.scope_digest() != scope_digest {
                    return Err(MusubiProviderAttestationClockSealErrorV1::Rejected);
                }
                if heads.get(&scope_digest) == Some(next) {
                    return Ok(());
                }
                if heads
                    .get(&scope_digest)
                    .map(MusubiProviderAttestationJournalCheckpointHeadRecordV1::record_digest)
                    != expected
                {
                    return Err(MusubiProviderAttestationClockSealErrorV1::Rejected);
                }
                if substitute.is_none()
                    && next.head().is_some_and(|head| {
                        !self
                            .checkpoint_blobs
                            .lock()
                            .expect("checkpoint blob lock")
                            .contains_key(&(scope_digest, head.checkpoint_revision()))
                    })
                {
                    return Err(MusubiProviderAttestationClockSealErrorV1::Rejected);
                }
                if error != Some(MusubiProviderAttestationClockSealErrorV1::Rejected) {
                    let retained = substitute.unwrap_or_else(|| next.clone());
                    heads.insert(scope_digest, retained.clone());
                    self.checkpoint_head_records
                        .lock()
                        .expect("checkpoint-head record lock")
                        .insert((scope_digest, retained.record_digest()), retained);
                }
                if self
                    .checkpoint_head_cas_readback_unavailable
                    .swap(false, Ordering::SeqCst)
                {
                    *self
                        .next_checkpoint_head_load_error
                        .lock()
                        .expect("checkpoint head load error lock") =
                        Some(MusubiProviderAttestationClockSealErrorV1::Unavailable);
                }
                error.map_or(Ok(()), Err)
            })
        }
    }
    #[derive(Debug)]
    struct DriftingHandleSeal {
        inner: TestSeal,
        handle_calls: AtomicUsize,
    }
    impl MusubiProviderAttestationClockSealV1 for DriftingHandleSeal {
        fn runtime_handle(&self) -> &str {
            if self.handle_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                HANDLE
            } else {
                "provider://musubi/provider-attestation-clock/substituted"
            }
        }
        fn qualification(
            &self,
        ) -> Result<
            MusubiProviderAttestationClockSealQualificationV1,
            MusubiProviderAttestationClockSealErrorV1,
        > {
            self.inner.qualification()
        }
        fn load_latest<'a>(
            &'a self,
            scope_digest: [u8; 32],
        ) -> ProviderIngestFutureV1<
            'a,
            Result<
                Option<MusubiProviderAttestationClockSealRecordV1>,
                MusubiProviderAttestationClockSealErrorV1,
            >,
        > {
            self.inner.load_latest(scope_digest)
        }
        fn compare_and_swap<'a>(
            &'a self,
            scope_digest: [u8; 32],
            expected: Option<[u8; 32]>,
            next: &'a MusubiProviderAttestationClockSealRecordV1,
        ) -> ProviderIngestFutureV1<'a, Result<(), MusubiProviderAttestationClockSealErrorV1>>
        {
            self.inner.compare_and_swap(scope_digest, expected, next)
        }
    }
    fn qualification() -> MusubiProviderAttestationClockSealQualificationV1 {
        MusubiProviderAttestationClockSealQualificationV1::new(1, [0xA5; 32])
    }
    #[test]
    fn qualification_binds_fixed_orphan_retention_limits() {
        let exact = qualification();
        assert_eq!(
            exact.orphan_blob_count_max(),
            MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_COUNT_MAX_V1
        );
        assert_eq!(
            exact.orphan_blob_bytes_max(),
            MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_BYTES_MAX_V1
        );
        assert_eq!(
            exact.orphan_blob_age_max_ms(),
            MUSUBI_PROVIDER_ATTESTATION_ORPHAN_BLOB_AGE_MAX_MS_V1
        );
        assert!(exact.validate().is_ok());
        let mut substituted = exact;
        substituted.orphan_blob_count_max = substituted.orphan_blob_count_max.saturating_add(1);
        assert_eq!(
            substituted.validate(),
            Err(MusubiProviderAttestationClockErrorV1::InvalidSealBinding)
        );
        let mut substituted = exact;
        substituted.orphan_blob_bytes_max = substituted.orphan_blob_bytes_max.saturating_add(1);
        assert_eq!(
            substituted.validate(),
            Err(MusubiProviderAttestationClockErrorV1::InvalidSealBinding)
        );
        let mut substituted = exact;
        substituted.orphan_blob_age_max_ms = substituted.orphan_blob_age_max_ms.saturating_add(1);
        assert_eq!(
            substituted.validate(),
            Err(MusubiProviderAttestationClockErrorV1::InvalidSealBinding)
        );
    }
    fn scope(seed: u8) -> MusubiProviderAttestationClockScopeV1 {
        MusubiProviderAttestationClockScopeV1::try_new(
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                [seed; 32],
            ))),
            ProviderId::new([seed.wrapping_add(1); 32]),
        )
        .expect("valid test scope")
    }
    fn checkpoint_scope(seed: u8) -> MusubiProviderAttestationJournalCheckpointScopeV1 {
        let clock_scope = scope(seed);
        MusubiProviderAttestationJournalCheckpointScopeV1::try_new(
            *clock_scope.network_id(),
            clock_scope.provider_id(),
            MusubiProviderAttestationJournalPolicyV1::default()
                .digest()
                .expect("default journal policy digest"),
        )
        .expect("valid checkpoint scope")
    }
    async fn initialized_checkpoint_seal(
        seed: u8,
    ) -> (
        Arc<TestSeal>,
        MusubiProviderAttestationClockSealBindingV1,
        MusubiProviderAttestationJournalCheckpointScopeV1,
        MusubiProviderAttestationJournalCheckpointHeadRecordV1,
    ) {
        let seal = Arc::new(TestSeal::new());
        let binding = seal.binding();
        let _clock = MusubiProviderAttestationSealedUnixClockV1::initialize_at(
            scope(seed),
            binding.clone(),
            seal.clone(),
            1_000,
        )
        .await
        .expect("initialize clock namespace");
        let checkpoint_scope = checkpoint_scope(seed);
        let h0 = initialize_musubi_provider_attestation_journal_checkpoint_seal_v1(
            &checkpoint_scope,
            &binding,
            seal.as_ref(),
        )
        .await
        .expect("initialize empty checkpoint H0");
        (seal, binding, checkpoint_scope, h0)
    }
    fn checkpoint_head(
        checkpoint_sequence: u64,
        last_observed_unix_ms: u64,
        checkpoint_blob: &[u8],
    ) -> MusubiProviderAttestationJournalCheckpointHeadV1 {
        MusubiProviderAttestationJournalCheckpointHeadV1::try_new(
            checkpoint_sequence,
            musubi_provider_attestation_journal_checkpoint_blob_revision_v1(checkpoint_blob)
                .expect("checkpoint revision"),
            last_observed_unix_ms,
        )
        .expect("checkpoint head")
    }
    #[test]
    fn checkpoint_commitments_and_bytes_ignore_ambient_norito_flags() {
        let policy = MusubiProviderAttestationJournalPolicyV1::default();
        let checkpoint_scope = checkpoint_scope(0x30);
        let expected_scope_digest = checkpoint_scope
            .scope_digest()
            .expect("canonical checkpoint scope digest");
        let expected_policy_digest = policy.digest().expect("canonical policy digest");
        let expected_checkpoint =
            musubi_provider_attestation_journal_test_checkpoint_bytes_v1(1, 0);
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(
            checkpoint_scope
                .scope_digest()
                .expect("ambient-independent scope digest"),
            expected_scope_digest
        );
        assert_eq!(
            policy.digest().expect("ambient-independent policy digest"),
            expected_policy_digest
        );
        assert_eq!(
            musubi_provider_attestation_journal_test_checkpoint_bytes_v1(1, 0),
            expected_checkpoint
        );
    }
    #[test]
    fn checkpoint_h0_and_successors_enforce_exact_lineage() {
        let scope_digest = checkpoint_scope(0x31)
            .scope_digest()
            .expect("checkpoint scope digest");
        let h0 = MusubiProviderAttestationJournalCheckpointHeadRecordV1::initial(scope_digest)
            .expect("empty H0");
        assert_eq!(h0.version(), 1);
        assert_eq!(h0.generation(), 1);
        assert_eq!(h0.scope_digest(), scope_digest);
        assert_eq!(h0.predecessor_record_digest(), None);
        assert_eq!(h0.head(), None);
        let first = MusubiProviderAttestationJournalCheckpointHeadRecordV1::successor(
            &h0,
            MusubiProviderAttestationJournalCheckpointHeadV1::try_new(1, [0x11; 32], 0)
                .expect("untimed first head"),
        )
        .expect("first checkpoint successor");
        assert_eq!(first.generation(), 2);
        assert_eq!(first.predecessor_record_digest(), Some(h0.record_digest()));
        first
            .validate_successor_of(&h0)
            .expect("exact H0 successor");
        let second = MusubiProviderAttestationJournalCheckpointHeadRecordV1::successor(
            &first,
            MusubiProviderAttestationJournalCheckpointHeadV1::try_new(2, [0x22; 32], 10)
                .expect("second head"),
        )
        .expect("second checkpoint successor");
        assert_eq!(second.generation(), 3);
        second
            .validate_successor_of(&first)
            .expect("exact second successor");
        for invalid_head in [
            MusubiProviderAttestationJournalCheckpointHeadV1::try_new(3, [0x33; 32], 10)
                .expect("skipped-sequence head"),
            MusubiProviderAttestationJournalCheckpointHeadV1::try_new(2, [0x11; 32], 10)
                .expect("reused-revision head"),
        ] {
            assert_eq!(
                MusubiProviderAttestationJournalCheckpointHeadRecordV1::successor(
                    &first,
                    invalid_head,
                ),
                Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidLineage)
            );
        }
        let regressing = MusubiProviderAttestationJournalCheckpointHeadRecordV1::successor(
            &second,
            MusubiProviderAttestationJournalCheckpointHeadV1::try_new(3, [0x44; 32], 9)
                .expect("regressing-time head shape"),
        );
        assert_eq!(
            regressing,
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidLineage)
        );
        assert_eq!(
            MusubiProviderAttestationJournalCheckpointHeadRecordV1::new(
                scope_digest,
                100,
                Some([0x55; 32]),
                Some(
                    MusubiProviderAttestationJournalCheckpointHeadV1::try_new(1, [0x66; 32], 10,)
                        .expect("structural head"),
                ),
            ),
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord)
        );
        let terminal = MusubiProviderAttestationJournalCheckpointHeadRecordV1::new(
            scope_digest,
            u64::MAX,
            Some([0x77; 32]),
            Some(
                MusubiProviderAttestationJournalCheckpointHeadV1::try_new(
                    u64::MAX - 1,
                    [0x88; 32],
                    10,
                )
                .expect("terminal head"),
            ),
        )
        .expect("terminal structurally valid record");
        assert_eq!(
            MusubiProviderAttestationJournalCheckpointHeadRecordV1::successor(
                &terminal,
                MusubiProviderAttestationJournalCheckpointHeadV1::try_new(
                    u64::MAX,
                    [0x99; 32],
                    10,
                )
                .expect("overflow successor head"),
            ),
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::ArithmeticOverflow)
        );
    }
    #[test]
    fn checkpoint_head_change_classification_distinguishes_concurrency_and_integrity() {
        let scope_digest = checkpoint_scope(0x3A)
            .scope_digest()
            .expect("checkpoint scope digest");
        let h0 = MusubiProviderAttestationJournalCheckpointHeadRecordV1::initial(scope_digest)
            .expect("empty H0");
        let first = MusubiProviderAttestationJournalCheckpointHeadRecordV1::successor(
            &h0,
            MusubiProviderAttestationJournalCheckpointHeadV1::try_new(1, [0xA1; 32], 0)
                .expect("first head"),
        )
        .expect("first record");
        let competing_first = MusubiProviderAttestationJournalCheckpointHeadRecordV1::successor(
            &h0,
            MusubiProviderAttestationJournalCheckpointHeadV1::try_new(1, [0xA2; 32], 0)
                .expect("competing head"),
        )
        .expect("competing first record");
        let second = MusubiProviderAttestationJournalCheckpointHeadRecordV1::successor(
            &first,
            MusubiProviderAttestationJournalCheckpointHeadV1::try_new(2, [0xA3; 32], 1)
                .expect("second head"),
        )
        .expect("second record");
        assert_eq!(
            classify_checkpoint_head_change(&h0, &second),
            MusubiProviderAttestationJournalCheckpointSealErrorV1::SealUnavailable,
            "any qualified later generation is concurrent progress"
        );
        assert_eq!(
            classify_checkpoint_head_change(&first, &competing_first),
            MusubiProviderAttestationJournalCheckpointSealErrorV1::Fork
        );
        assert_eq!(
            classify_checkpoint_head_change(&first, &h0),
            MusubiProviderAttestationJournalCheckpointSealErrorV1::Rollback
        );
    }
    #[tokio::test]
    async fn checkpoint_scope_and_policy_substitution_fail_closed() {
        let (seal, binding, checkpoint_scope, h0) = initialized_checkpoint_seal(0x32).await;
        let mut foreign_policy = MusubiProviderAttestationJournalPolicyV1::default();
        foreign_policy.max_attempts += 1;
        let blob = musubi_provider_attestation_journal_test_checkpoint_bytes_v1(1, 0);
        let head = checkpoint_head(1, 0, &blob);
        assert_eq!(
            seal_musubi_provider_attestation_journal_checkpoint_v1(
                &checkpoint_scope,
                foreign_policy,
                &binding,
                seal.as_ref(),
                &h0,
                head,
                &blob,
            )
            .await,
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidScope)
        );
        let foreign_scope = MusubiProviderAttestationJournalCheckpointScopeV1::try_new(
            *checkpoint_scope.network_id(),
            checkpoint_scope.provider_id(),
            [0xFE; 32],
        )
        .expect("foreign policy scope");
        assert_eq!(
            h0.validate(foreign_scope.scope_digest().expect("foreign scope digest")),
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidRecord)
        );
    }
    #[tokio::test]
    async fn checkpoint_blob_digest_and_metadata_mismatch_fail_before_storage() {
        let (seal, binding, checkpoint_scope, h0) = initialized_checkpoint_seal(0x33).await;
        let blob = musubi_provider_attestation_journal_test_checkpoint_bytes_v1(1, 0);
        let substituted_revision =
            MusubiProviderAttestationJournalCheckpointHeadV1::try_new(1, [0xEE; 32], 0)
                .expect("substituted revision head");
        assert_eq!(
            seal_musubi_provider_attestation_journal_checkpoint_v1(
                &checkpoint_scope,
                MusubiProviderAttestationJournalPolicyV1::default(),
                &binding,
                seal.as_ref(),
                &h0,
                substituted_revision,
                &blob,
            )
            .await,
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob)
        );
        assert!(seal.checkpoint_blobs.lock().expect("blob lock").is_empty());
        let wrong_metadata = checkpoint_head(1, 1, &blob);
        assert_eq!(
            seal_musubi_provider_attestation_journal_checkpoint_v1(
                &checkpoint_scope,
                MusubiProviderAttestationJournalPolicyV1::default(),
                &binding,
                seal.as_ref(),
                &h0,
                wrong_metadata,
                &blob,
            )
            .await,
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidBlob)
        );
    }
    #[tokio::test]
    async fn checkpoint_head_lost_cas_response_resolves_by_exact_readback() {
        let (seal, binding, checkpoint_scope, h0) = initialized_checkpoint_seal(0x34).await;
        let blob = musubi_provider_attestation_journal_test_checkpoint_bytes_v1(1, 100);
        let head = checkpoint_head(1, 100, &blob);
        seal.fail_next_checkpoint_head_cas(MusubiProviderAttestationClockSealErrorV1::Ambiguous);
        let committed = seal_musubi_provider_attestation_journal_checkpoint_v1(
            &checkpoint_scope,
            MusubiProviderAttestationJournalPolicyV1::default(),
            &binding,
            seal.as_ref(),
            &h0,
            head,
            &blob,
        )
        .await
        .expect("exact readback resolves lost head-CAS response");
        assert_eq!(committed.generation(), 2);
        assert_eq!(committed.head(), Some(head));
        let retried = seal_musubi_provider_attestation_journal_checkpoint_v1(
            &checkpoint_scope,
            MusubiProviderAttestationJournalPolicyV1::default(),
            &binding,
            seal.as_ref(),
            &h0,
            head,
            &blob,
        )
        .await
        .expect("identical stale-predecessor retry is idempotent");
        assert_eq!(retried, committed);
    }
    #[tokio::test]
    async fn checkpoint_mutation_ambiguity_survives_unavailable_resolution_readback() {
        let (blob_seal, blob_binding, blob_scope, blob_h0) =
            initialized_checkpoint_seal(0x3B).await;
        let blob = musubi_provider_attestation_journal_test_checkpoint_bytes_v1(1, 100);
        let blob_head = checkpoint_head(1, 100, &blob);
        blob_seal
            .fail_next_checkpoint_blob_put(MusubiProviderAttestationClockSealErrorV1::Ambiguous);
        blob_seal.make_next_checkpoint_blob_put_readback_unavailable();
        assert_eq!(
            seal_musubi_provider_attestation_journal_checkpoint_v1(
                &blob_scope,
                MusubiProviderAttestationJournalPolicyV1::default(),
                &blob_binding,
                blob_seal.as_ref(),
                &blob_h0,
                blob_head,
                &blob,
            )
            .await,
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealAmbiguous)
        );
        let (head_seal, head_binding, head_scope, head_h0) =
            initialized_checkpoint_seal(0x3C).await;
        let head_blob = musubi_provider_attestation_journal_test_checkpoint_bytes_v1(1, 100);
        let head = checkpoint_head(1, 100, &head_blob);
        head_seal
            .fail_next_checkpoint_head_cas(MusubiProviderAttestationClockSealErrorV1::Ambiguous);
        head_seal.make_next_checkpoint_head_cas_readback_unavailable();
        assert_eq!(
            seal_musubi_provider_attestation_journal_checkpoint_v1(
                &head_scope,
                MusubiProviderAttestationJournalPolicyV1::default(),
                &head_binding,
                head_seal.as_ref(),
                &head_h0,
                head,
                &head_blob,
            )
            .await,
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::SealAmbiguous)
        );
    }
    #[tokio::test]
    async fn checkpoint_head_same_predecessor_fork_is_rejected() {
        let (seal, binding, checkpoint_scope, h0) = initialized_checkpoint_seal(0x35).await;
        let blob = musubi_provider_attestation_journal_test_checkpoint_bytes_v1(1, 100);
        let head = checkpoint_head(1, 100, &blob);
        let fork = MusubiProviderAttestationJournalCheckpointHeadRecordV1::successor(
            &h0,
            MusubiProviderAttestationJournalCheckpointHeadV1::try_new(1, [0xFD; 32], 100)
                .expect("fork head"),
        )
        .expect("structural same-predecessor fork");
        seal.substitute_next_checkpoint_head(fork);
        seal.fail_next_checkpoint_head_cas(MusubiProviderAttestationClockSealErrorV1::Ambiguous);
        assert_eq!(
            seal_musubi_provider_attestation_journal_checkpoint_v1(
                &checkpoint_scope,
                MusubiProviderAttestationJournalPolicyV1::default(),
                &binding,
                seal.as_ref(),
                &h0,
                head,
                &blob,
            )
            .await,
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::Fork)
        );
    }
    #[tokio::test]
    async fn checkpoint_open_rejects_missing_current_blob() {
        let (seal, binding, checkpoint_scope, h0) = initialized_checkpoint_seal(0x36).await;
        let blob = musubi_provider_attestation_journal_test_checkpoint_bytes_v1(1, 100);
        let head = checkpoint_head(1, 100, &blob);
        let committed = seal_musubi_provider_attestation_journal_checkpoint_v1(
            &checkpoint_scope,
            MusubiProviderAttestationJournalPolicyV1::default(),
            &binding,
            seal.as_ref(),
            &h0,
            head,
            &blob,
        )
        .await
        .expect("seal checkpoint");
        assert_eq!(committed.head(), Some(head));
        seal.remove_checkpoint_blob(
            checkpoint_scope
                .scope_digest()
                .expect("checkpoint scope digest"),
            head.checkpoint_revision(),
        );
        assert_eq!(
            load_musubi_provider_attestation_journal_checkpoint_v1(
                &checkpoint_scope,
                MusubiProviderAttestationJournalPolicyV1::default(),
                &binding,
                seal.as_ref(),
            )
            .await,
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::MissingBlob)
        );
    }
    #[tokio::test]
    async fn checkpoint_effect_qualification_drift_fails_closed() {
        let (seal, binding, checkpoint_scope, h0) = initialized_checkpoint_seal(0x37).await;
        let blob = musubi_provider_attestation_journal_test_checkpoint_bytes_v1(1, 100);
        let head = checkpoint_head(1, 100, &blob);
        seal.drift_after_checkpoint_blob_put
            .store(true, Ordering::SeqCst);
        assert_eq!(
            seal_musubi_provider_attestation_journal_checkpoint_v1(
                &checkpoint_scope,
                MusubiProviderAttestationJournalPolicyV1::default(),
                &binding,
                seal.as_ref(),
                &h0,
                head,
                &blob,
            )
            .await,
            Err(MusubiProviderAttestationJournalCheckpointSealErrorV1::InvalidSealBinding)
        );
    }
    #[tokio::test]
    async fn initialization_restart_and_advancement_are_exactly_sealed() {
        let seal = Arc::new(TestSeal::new());
        let binding = seal.binding();
        let clock = MusubiProviderAttestationSealedUnixClockV1::initialize_at(
            scope(1),
            binding.clone(),
            seal.clone(),
            100,
        )
        .await
        .expect("initialize sealed clock");
        assert_eq!(clock.durable_floor_unix_ms().await, 100);
        assert_eq!(clock.now_at(125).await.expect("advance clock"), 125);
        let record = seal
            .record
            .lock()
            .expect("record lock")
            .clone()
            .expect("sealed record");
        assert_eq!(record.generation(), 2);
        assert_eq!(record.floor_unix_ms(), 125);
        drop(clock);
        let reopened =
            MusubiProviderAttestationSealedUnixClockV1::open_at(scope(1), binding, seal, 130)
                .await
                .expect("open sealed clock after restart");
        assert_eq!(reopened.durable_floor_unix_ms().await, 130);
    }
    #[tokio::test]
    async fn public_clock_path_samples_only_system_unix_time() {
        let seal = Arc::new(TestSeal::new());
        let binding = seal.binding();
        let clock = MusubiProviderAttestationSealedUnixClockV1::initialize(
            scope(6),
            binding.clone(),
            seal.clone(),
        )
        .await
        .expect("initialize from system UNIX time");
        let initialized_floor = clock.durable_floor_unix_ms().await;
        assert_ne!(initialized_floor, 0);
        assert!(clock.now_unix_ms().await.expect("sample system time") >= initialized_floor);
        drop(clock);
        let reopened = MusubiProviderAttestationSealedUnixClockV1::open(scope(6), binding, seal)
            .await
            .expect("restart from system UNIX time");
        assert!(reopened.durable_floor_unix_ms().await >= initialized_floor);
    }
    #[tokio::test]
    async fn qualification_rejects_a_handle_that_drifts_within_one_snapshot() {
        let seal = Arc::new(DriftingHandleSeal {
            inner: TestSeal::new(),
            handle_calls: AtomicUsize::new(0),
        });
        let binding = MusubiProviderAttestationClockSealBindingV1::try_new(HANDLE, qualification())
            .expect("expected binding");
        assert_eq!(
            MusubiProviderAttestationSealedUnixClockV1::initialize_at(
                scope(9),
                binding,
                seal,
                100,
            )
            .await
            .expect_err("intra-snapshot handle drift must fail closed"),
            MusubiProviderAttestationClockErrorV1::InvalidSealBinding
        );
    }
    #[tokio::test]
    async fn restart_never_reinitializes_missing_or_regressing_state() {
        let seal = Arc::new(TestSeal::new());
        let binding = seal.binding();
        assert_eq!(
            MusubiProviderAttestationSealedUnixClockV1::open_at(
                scope(2),
                binding.clone(),
                seal.clone(),
                100,
            )
            .await
            .expect_err("ordinary open must reject absent state"),
            MusubiProviderAttestationClockErrorV1::Uninitialized
        );
        let clock = MusubiProviderAttestationSealedUnixClockV1::initialize_at(
            scope(2),
            binding.clone(),
            seal.clone(),
            100,
        )
        .await
        .expect("initialize clock");
        assert_eq!(
            MusubiProviderAttestationSealedUnixClockV1::initialize_at(
                scope(2),
                binding.clone(),
                seal.clone(),
                101,
            )
            .await
            .expect_err("reinitialization must fail"),
            MusubiProviderAttestationClockErrorV1::AlreadyInitialized
        );
        assert_eq!(
            MusubiProviderAttestationSealedUnixClockV1::open_at(
                scope(2),
                binding,
                seal.clone(),
                99,
            )
            .await
            .expect_err("restart clock rollback must fail"),
            MusubiProviderAttestationClockErrorV1::ClockRollback
        );
        seal.replace_record(None);
        assert_eq!(
            clock.now_at(101).await,
            Err(MusubiProviderAttestationClockErrorV1::ClockRollback)
        );
    }
    #[tokio::test]
    async fn ambiguous_commit_succeeds_only_after_exact_authoritative_readback() {
        let seal = Arc::new(TestSeal::new());
        let binding = seal.binding();
        seal.fail_next_cas(MusubiProviderAttestationClockSealErrorV1::Ambiguous);
        let clock = MusubiProviderAttestationSealedUnixClockV1::initialize_at(
            scope(3),
            binding,
            seal.clone(),
            100,
        )
        .await
        .expect("exact readback resolves lost CAS response");
        assert_eq!(clock.durable_floor_unix_ms().await, 100);
        seal.fail_next_cas(MusubiProviderAttestationClockSealErrorV1::Rejected);
        assert_eq!(
            clock.now_at(101).await,
            Err(MusubiProviderAttestationClockErrorV1::SealRejected)
        );
        assert_eq!(clock.durable_floor_unix_ms().await, 100);
    }
    #[tokio::test]
    async fn clock_mutation_ambiguity_survives_unavailable_resolution_readback() {
        let seal = Arc::new(TestSeal::new());
        let binding = seal.binding();
        let clock = MusubiProviderAttestationSealedUnixClockV1::initialize_at(
            scope(0x3D),
            binding,
            seal.clone(),
            100,
        )
        .await
        .expect("initialize clock");
        seal.fail_next_cas(MusubiProviderAttestationClockSealErrorV1::Ambiguous);
        seal.make_next_clock_cas_readback_unavailable();
        assert_eq!(
            clock.now_at(101).await,
            Err(MusubiProviderAttestationClockErrorV1::SealAmbiguous)
        );
    }
    #[tokio::test]
    async fn overlapping_clock_writers_adopt_an_exact_successor_of_their_commit() {
        let seal = Arc::new(TestSeal::new());
        let binding = seal.binding();
        let first = Arc::new(
            MusubiProviderAttestationSealedUnixClockV1::initialize_at(
                scope(0x3E),
                binding.clone(),
                seal.clone(),
                100,
            )
            .await
            .expect("initialize first clock"),
        );
        let second = MusubiProviderAttestationSealedUnixClockV1::open_at(
            scope(0x3E),
            binding,
            seal.clone(),
            100,
        )
        .await
        .expect("open second clock instance");
        seal.pause_after_next_clock_cas();
        let first_writer = tokio::spawn({
            let first = Arc::clone(&first);
            async move { first.now_at(101).await }
        });
        seal.clock_cas_reached.notified().await;
        assert_eq!(second.now_at(102).await.expect("overtaking writer"), 102);
        seal.resume_after_clock_cas.notify_one();
        assert_eq!(
            first_writer.await.expect("first clock writer task"),
            Ok(101),
            "the first writer adopts the exact successor that covers its floor"
        );
        assert_eq!(first.durable_floor_unix_ms().await, 102);
        assert_eq!(second.durable_floor_unix_ms().await, 102);
    }
    #[tokio::test]
    async fn farther_ahead_clock_readback_with_regressed_floor_is_rollback() {
        let seal = Arc::new(TestSeal::new());
        let binding = seal.binding();
        let clock = Arc::new(
            MusubiProviderAttestationSealedUnixClockV1::initialize_at(
                scope(0x3F),
                binding,
                seal.clone(),
                100,
            )
            .await
            .expect("initialize clock"),
        );
        let substituted = MusubiProviderAttestationClockSealRecordV1::new(
            clock.scope_digest(),
            4,
            Some([0xF1; 32]),
            99,
        )
        .expect("structurally valid farther-ahead record");
        seal.pause_after_next_clock_cas();
        let writer = tokio::spawn({
            let clock = Arc::clone(&clock);
            async move { clock.now_at(101).await }
        });
        seal.clock_cas_reached.notified().await;
        seal.replace_record(Some(substituted));
        seal.resume_after_clock_cas.notify_one();
        assert_eq!(
            writer.await.expect("clock writer task"),
            Err(MusubiProviderAttestationClockErrorV1::ClockRollback)
        );
        assert_eq!(clock.durable_floor_unix_ms().await, 100);
    }
    #[tokio::test]
    async fn advancement_recovers_exact_successor_committed_before_cancellation() {
        let seal = Arc::new(TestSeal::new());
        let binding = seal.binding();
        let clock = MusubiProviderAttestationSealedUnixClockV1::initialize_at(
            scope(7),
            binding,
            seal.clone(),
            100,
        )
        .await
        .expect("initialize clock");
        let retained = seal
            .record
            .lock()
            .expect("record lock")
            .clone()
            .expect("initial record");
        let committed = MusubiProviderAttestationClockSealRecordV1::successor(&retained, 101)
            .expect("successor committed before cancellation");
        // Model cancellation after the external CAS became authoritative but
        // before the clock future could assign the returned record locally.
        seal.replace_record(Some(committed));
        assert_eq!(clock.durable_floor_unix_ms().await, 100);
        assert_eq!(clock.now_at(102).await.expect("recover and advance"), 102);
        assert_eq!(clock.durable_floor_unix_ms().await, 102);
        let authoritative = seal
            .record
            .lock()
            .expect("record lock")
            .clone()
            .expect("advanced record");
        assert_eq!(authoritative.generation(), 3);
        assert_eq!(authoritative.floor_unix_ms(), 102);
    }
    #[tokio::test]
    async fn advancement_treats_unproven_forward_record_as_unavailable() {
        let seal = Arc::new(TestSeal::new());
        let binding = seal.binding();
        let clock = MusubiProviderAttestationSealedUnixClockV1::initialize_at(
            scope(8),
            binding,
            seal.clone(),
            100,
        )
        .await
        .expect("initialize clock");
        let retained = seal
            .record
            .lock()
            .expect("record lock")
            .clone()
            .expect("initial record");
        let substituted = MusubiProviderAttestationClockSealRecordV1::new(
            retained.material.scope_digest,
            2,
            Some([0xFF; 32]),
            101,
        )
        .expect("structurally valid substituted record");
        seal.replace_record(Some(substituted));
        assert_eq!(
            clock.now_at(102).await,
            Err(MusubiProviderAttestationClockErrorV1::SealUnavailable),
            "an unproven forward record may be a concurrent advancement and must remain retryable"
        );
        assert_eq!(clock.durable_floor_unix_ms().await, 100);
    }
    #[tokio::test]
    async fn qualification_and_scope_substitution_fail_closed() {
        let seal = Arc::new(TestSeal::new());
        let binding = seal.binding();
        let clock = MusubiProviderAttestationSealedUnixClockV1::initialize_at(
            scope(4),
            binding,
            seal.clone(),
            100,
        )
        .await
        .expect("initialize clock");
        *seal.qualification.lock().expect("qualification lock") =
            MusubiProviderAttestationClockSealQualificationV1::new(2, [0xA5; 32]);
        assert_eq!(
            clock.now_at(101).await,
            Err(MusubiProviderAttestationClockErrorV1::InvalidSealBinding)
        );
        *seal.qualification.lock().expect("qualification lock") = qualification();
        let foreign = MusubiProviderAttestationClockSealRecordV1::initial(
            scope(5).scope_digest().expect("scope digest"),
            100,
        )
        .expect("foreign record");
        seal.replace_record(Some(foreign));
        assert_eq!(
            clock.now_at(101).await,
            Err(MusubiProviderAttestationClockErrorV1::InvalidSealRecord)
        );
    }
    #[test]
    fn public_scope_digest_revalidates_decoded_shape() {
        let mut raw_fixture = scope(9);
        // Model a structurally decoded value that bypassed `try_new` with an unmarked hash.
        raw_fixture.network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0; 32])),
        );
        let bytes = norito::encode_canonical(&raw_fixture).expect("encode raw invalid fixture");
        let invalid = norito::decode_canonical::<MusubiProviderAttestationClockScopeV1>(&bytes)
            .expect("decode raw invalid fixture");
        assert_eq!(
            invalid.scope_digest(),
            Err(MusubiProviderAttestationClockErrorV1::InvalidScope)
        );
    }
}
