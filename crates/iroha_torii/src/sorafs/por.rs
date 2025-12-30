//! In-memory Proof-of-Retrievability coordinator used by Torii.
//!
//! This module collects governance-issued PoR challenges, provider proofs, and
//! audit verdicts so that operators can query historical outcomes and generate
//! weekly reports. When constructed with [`PorCoordinator::with_persistence`],
//! state is snapshotted to disk using Norito so operators can recover history
//! across restarts.

use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
#[cfg(feature = "app_api")]
use std::{
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    time::Duration as StdDuration,
};

#[cfg(feature = "app_api")]
use blake3::Hasher;
use dashmap::DashMap;
#[cfg(feature = "app_api")]
use iroha_futures::supervisor::ShutdownSignal;
#[cfg(feature = "app_api")]
use norito::json::{self, Map as JsonMap, Value as JsonValue};
use norito::{
    codec::{Decode, Encode},
    decode_from_bytes,
    derive::{NoritoDeserialize, NoritoSerialize},
    to_bytes,
};
use parking_lot::RwLock;
use sorafs_manifest::por::{
    AuditOutcomeV1, AuditVerdictV1, ManualPorChallengeV1, ManualPorChallengeValidationError,
    POR_CHALLENGE_STATUS_VERSION_V1, POR_WEEKLY_REPORT_VERSION_V1, PorChallengeOutcome,
    PorChallengeStatusV1, PorChallengeV1, PorChallengeValidationError, PorProviderSummaryV1,
    PorProviderSummaryValidationError, PorReportIsoWeek, PorReportIsoWeekValidationError,
    PorWeeklyReportV1, PorWeeklyReportValidationError,
};
use sorafs_node::PorVerdictOutcome;
#[cfg(feature = "app_api")]
use sorafs_node::{ManifestVrfBundle, PlannedChallenge, PorChallengePlannerError, PorRandomness};
use thiserror::Error;
use time::{Date, Duration, OffsetDateTime, Weekday};
#[cfg(feature = "app_api")]
use tokio::time::{MissedTickBehavior, interval};

const POR_STATUS_EXPORT_VERSION_V1: u8 = 1;
const POR_COORDINATOR_SNAPSHOT_VERSION_V1: u8 = 1;

#[derive(Debug, Clone)]
struct RecordedVerdict {
    outcome: AuditOutcomeV1,
    failure_reason: Option<String>,
    decided_at: u64,
    proof_digest: Option<[u8; 32]>,
}

impl From<&AuditVerdictV1> for RecordedVerdict {
    fn from(verdict: &AuditVerdictV1) -> Self {
        Self {
            outcome: verdict.outcome,
            failure_reason: verdict.failure_reason.clone(),
            decided_at: verdict.decided_at,
            proof_digest: verdict.proof_digest,
        }
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct RecordedVerdictSnapshot {
    outcome: u8,
    #[norito(default)]
    failure_reason: Option<String>,
    decided_at: u64,
    #[norito(default)]
    proof_digest: Option<[u8; 32]>,
}

impl From<&RecordedVerdict> for RecordedVerdictSnapshot {
    fn from(verdict: &RecordedVerdict) -> Self {
        Self {
            outcome: verdict.outcome as u8,
            failure_reason: verdict.failure_reason.clone(),
            decided_at: verdict.decided_at,
            proof_digest: verdict.proof_digest,
        }
    }
}

impl RecordedVerdictSnapshot {
    fn into_recorded_verdict(self) -> Result<RecordedVerdict, PorPersistenceError> {
        let outcome = match self.outcome {
            1 => AuditOutcomeV1::Success,
            2 => AuditOutcomeV1::Failed,
            3 => AuditOutcomeV1::Repaired,
            value => return Err(PorPersistenceError::InvalidFlag { value }),
        };
        Ok(RecordedVerdict {
            outcome,
            failure_reason: self.failure_reason,
            decided_at: self.decided_at,
            proof_digest: self.proof_digest,
        })
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RecordedVerdictSnapshot {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        norito::core::decode_field_canonical::<RecordedVerdictSnapshot>(bytes)
    }
}

/// Binary export produced for PoR status snapshots.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct PorStatusExportV1 {
    /// Schema version.
    pub version: u8,
    /// Unix timestamp when the export was generated.
    pub generated_at: u64,
    /// Optional epoch filter lower bound (inclusive).
    #[norito(default)]
    pub start_epoch: Option<u64>,
    /// Optional epoch filter upper bound (inclusive).
    #[norito(default)]
    pub end_epoch: Option<u64>,
    /// Challenge status records included in the export.
    pub statuses: Vec<PorChallengeStatusV1>,
}

impl PorStatusExportV1 {
    /// Validate export metadata.
    ///
    /// # Errors
    ///
    /// Returns [`PorStatusExportValidationError`] if the export version,
    /// epoch bounds, or contained challenge statuses are invalid.
    pub fn validate(&self) -> Result<(), PorStatusExportValidationError> {
        if self.version != POR_STATUS_EXPORT_VERSION_V1 {
            return Err(PorStatusExportValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if let (Some(start), Some(end)) = (self.start_epoch, self.end_epoch) {
            if start > end {
                return Err(PorStatusExportValidationError::InvalidEpochRange { start, end });
            }
        }
        for status in &self.statuses {
            status
                .validate()
                .map_err(PorStatusExportValidationError::InvalidStatus)?;
        }
        Ok(())
    }
}

/// Validation errors for [`PorStatusExportV1`].
#[allow(clippy::large_enum_variant, variant_size_differences)]
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PorStatusExportValidationError {
    /// The supplied export version is not supported.
    #[error("unsupported export version {found}")]
    UnsupportedVersion {
        /// Version byte read from the export payload.
        found: u8,
    },
    /// Provided epoch range has the start greater than the end.
    #[error("start_epoch {start} must not exceed end_epoch {end}")]
    InvalidEpochRange {
        /// Inclusive start of the epoch interval.
        start: u64,
        /// Inclusive end of the epoch interval.
        end: u64,
    },
    /// One of the embedded status records failed validation.
    #[error("status record invalid: {0}")]
    InvalidStatus(#[source] sorafs_manifest::por::PorChallengeStatusValidationError),
}

/// Aggregate coordinator for PoR challenge lifecycle.
#[derive(Debug, Clone)]
pub struct PorCoordinator {
    records: Arc<DashMap<[u8; 32], ChallengeRecord>>,
    /// Tracks recent forced challenges so we can flag providers missing VRFs.
    forced_providers: Arc<RwLock<HashMap<[u8; 32], BTreeSet<u64>>>>,
    persistence: Option<Arc<PorPersistence>>,
}

impl PorCoordinator {
    /// Construct an empty coordinator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Arc::new(DashMap::new()),
            forced_providers: Arc::new(RwLock::new(HashMap::new())),
            persistence: None,
        }
    }

    /// Construct a coordinator backed by on-disk persistence.
    ///
    /// # Errors
    ///
    /// Returns [`PorPersistenceError`] if the existing persistence records cannot
    /// be loaded from disk.
    pub fn with_persistence<P: Into<PathBuf>>(path: P) -> Result<Self, PorPersistenceError> {
        let persistence = Arc::new(PorPersistence::new(path.into()));
        let (records, forced) = persistence.load()?;
        Ok(Self {
            records,
            forced_providers: forced,
            persistence: Some(persistence),
        })
    }

    /// Record a governance-issued challenge.
    ///
    /// # Errors
    ///
    /// Returns [`PorCoordinatorError::InvalidChallenge`] when validation fails,
    /// [`PorCoordinatorError::ChallengeConflict`] if a different challenge is
    /// already recorded, or [`PorCoordinatorError::Persistence`] when
    /// persistence updates fail.
    pub fn record_challenge(&self, challenge: &PorChallengeV1) -> Result<(), PorCoordinatorError> {
        challenge
            .validate()
            .map_err(PorCoordinatorError::InvalidChallenge)?;
        match self.records.entry(challenge.challenge_id) {
            dashmap::mapref::entry::Entry::Occupied(existing) => {
                if existing.get().challenge != *challenge {
                    return Err(PorCoordinatorError::ChallengeConflict {
                        challenge_id: challenge.challenge_id,
                        challenge_id_hex: hex::encode(challenge.challenge_id),
                    });
                }
                // Idempotent replay; nothing else to do.
                Ok(())
            }
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                let record = ChallengeRecord::from_challenge(challenge.clone());
                if record.challenge.forced {
                    self.track_forced(&record.challenge.provider_id, record.challenge.epoch_id);
                }
                vacant.insert(record);
                self.persist()?;
                Ok(())
            }
        }
    }

    /// Record a provider proof submission.
    ///
    /// # Errors
    ///
    /// Returns [`PorCoordinatorError::InvalidProof`] if validation fails,
    /// [`PorCoordinatorError::UnknownChallenge`] when the challenge cannot be
    /// found, or [`PorCoordinatorError::Persistence`] if persisting updates
    /// fails.
    pub fn record_proof(
        &self,
        proof: &sorafs_manifest::por::PorProofV1,
    ) -> Result<(), PorCoordinatorError> {
        proof
            .validate()
            .map_err(PorCoordinatorError::InvalidProof)?;
        let digest = proof.proof_digest();
        {
            let mut entry = self.records.get_mut(&proof.challenge_id).ok_or_else(|| {
                PorCoordinatorError::UnknownChallenge {
                    challenge_id: proof.challenge_id,
                    challenge_id_hex: hex::encode(proof.challenge_id),
                }
            })?;
            entry.ensure_consistency(proof.manifest_digest, proof.provider_id)?;
            if let Some(existing) = entry.proof_digest {
                if existing == digest {
                    return Ok(());
                }
                return Err(PorCoordinatorError::DuplicateProof {
                    challenge_id: proof.challenge_id,
                    challenge_id_hex: hex::encode(proof.challenge_id),
                });
            }
            entry.proof_digest = Some(digest);
            entry.responded_at = Some(unix_now());
        }
        self.persist()?;
        Ok(())
    }

    /// Record an audit verdict emitted by governance.
    ///
    /// # Errors
    ///
    /// Returns [`PorCoordinatorError`] if the verdict is invalid, references an
    /// unknown challenge, or persistence fails.
    pub fn record_verdict(
        &self,
        verdict: &AuditVerdictV1,
        outcome: PorVerdictOutcome,
    ) -> Result<(), PorCoordinatorError> {
        verdict
            .validate()
            .map_err(PorCoordinatorError::InvalidVerdict)?;
        {
            let mut entry = self.records.get_mut(&verdict.challenge_id).ok_or_else(|| {
                PorCoordinatorError::UnknownChallenge {
                    challenge_id: verdict.challenge_id,
                    challenge_id_hex: hex::encode(verdict.challenge_id),
                }
            })?;
            entry.ensure_consistency(verdict.manifest_digest, verdict.provider_id)?;
            entry.verdict = Some(RecordedVerdict::from(verdict));
            entry.repair_history_id = outcome.repair_history_id;
            if entry.proof_digest.is_none() {
                entry.proof_digest = verdict.proof_digest;
            }
            if entry.responded_at.is_none() {
                entry.responded_at = Some(verdict.decided_at);
            }
        }
        self.persist()?;
        Ok(())
    }

    /// Snapshot challenge statuses using optional filters.
    #[must_use]
    pub fn query_statuses(
        &self,
        filter: &PorStatusFilter,
        limit: Option<usize>,
        page_token: Option<[u8; 32]>,
    ) -> Vec<PorChallengeStatusV1> {
        let mut statuses: Vec<_> = self
            .records
            .iter()
            .map(|entry| entry.value().to_status())
            .collect();
        statuses.sort_by(|left, right| match left.issued_at.cmp(&right.issued_at) {
            Ordering::Equal => left.challenge_id.cmp(&right.challenge_id),
            other => other,
        });
        if let Some(token) = page_token {
            let pos = statuses
                .iter()
                .position(|status| status.challenge_id == token)
                .map(|idx| idx + 1)
                .unwrap_or(0);
            statuses = statuses.split_off(pos);
        }
        if let Some(limit) = limit {
            statuses.truncate(limit);
        }
        statuses
            .into_iter()
            .filter(|status| filter.matches(status))
            .collect()
    }

    /// Export challenge statuses within an optional epoch range.
    ///
    /// # Errors
    ///
    /// Returns [`PorCoordinatorError`] if the generated export fails validation.
    pub fn export_statuses(
        &self,
        range: Option<(u64, u64)>,
    ) -> Result<PorStatusExportV1, PorCoordinatorError> {
        let filter = PorStatusFilter {
            manifest: None,
            provider: None,
            epoch: None,
            status: None,
        };
        let statuses: Vec<_> = self
            .records
            .iter()
            .map(|entry| entry.value().to_status())
            .filter(|status| match range {
                Some((start, end)) => (status.epoch_id >= start) && (status.epoch_id <= end),
                None => true,
            })
            .filter(|status| filter.matches(status))
            .collect();

        let export = PorStatusExportV1 {
            version: POR_STATUS_EXPORT_VERSION_V1,
            generated_at: unix_now(),
            start_epoch: range.map(|r| r.0),
            end_epoch: range.map(|r| r.1),
            statuses,
        };
        export
            .validate()
            .map_err(PorCoordinatorError::InvalidExport)?;
        Ok(export)
    }

    /// Generate a weekly report for the supplied ISO week.
    ///
    /// # Errors
    ///
    /// Returns [`PorCoordinatorError`] if the week is invalid, data cannot be
    /// aggregated, or the report fails validation.
    pub fn weekly_report(
        &self,
        cycle: PorReportIsoWeek,
    ) -> Result<PorWeeklyReportV1, PorCoordinatorError> {
        cycle
            .validate()
            .map_err(PorCoordinatorError::InvalidIsoWeek)?;
        let (start, end) = iso_week_bounds(cycle)?;

        let mut statuses: Vec<_> = self
            .records
            .iter()
            .map(|entry| entry.value().to_status())
            .filter(|status| {
                let issued =
                    OffsetDateTime::from_unix_timestamp(status.issued_at as i64).unwrap_or(start);
                issued >= start && issued < end
            })
            .collect();
        statuses.sort_by(|left, right| match left.issued_at.cmp(&right.issued_at) {
            Ordering::Equal => left.challenge_id.cmp(&right.challenge_id),
            other => other,
        });

        let challenges_total = statuses.len() as u32;
        let challenges_verified = statuses
            .iter()
            .filter(|s| matches!(s.status, PorChallengeOutcome::Verified))
            .count() as u32;
        let challenges_failed = statuses
            .iter()
            .filter(|s| {
                matches!(
                    s.status,
                    PorChallengeOutcome::Failed | PorChallengeOutcome::Repaired
                )
            })
            .count() as u32;
        let forced_challenges = statuses.iter().filter(|s| s.forced).count() as u32;

        let mut provider_map: HashMap<[u8; 32], ProviderStats> = HashMap::new();
        for status in &statuses {
            let entry = provider_map.entry(status.provider_id).or_default();
            entry.manifests.insert(status.manifest_digest);
            entry.challenges += 1;
            match status.status {
                PorChallengeOutcome::Verified => entry.successes += 1,
                PorChallengeOutcome::Failed | PorChallengeOutcome::Repaired => {
                    entry.failures += 1;
                    if entry.first_failure_at.is_none() {
                        entry.first_failure_at =
                            Some(status.responded_at.unwrap_or(status.issued_at));
                    }
                }
                PorChallengeOutcome::Forced => entry.forced += 1,
                PorChallengeOutcome::Pending => {}
            }
        }

        let providers_missing_vrf = provider_map
            .iter()
            .filter(|(_, stats)| stats.forced > 0)
            .map(|(provider, _)| *provider)
            .collect();

        let mut top_offenders: Vec<PorProviderSummaryV1> = provider_map
            .iter()
            .filter_map(|(provider_id, stats)| {
                if stats.failures == 0 && stats.forced == 0 {
                    return None;
                }
                let challenges = stats.challenges;
                let successes = stats.successes;
                let failures = stats.failures;
                let forced = stats.forced;
                let success_rate = if challenges == 0 {
                    1.0
                } else {
                    successes as f64 / challenges as f64
                };
                Some(PorProviderSummaryV1 {
                    provider_id: *provider_id,
                    manifest_count: stats.manifests.len() as u32,
                    challenges,
                    successes,
                    failures,
                    forced,
                    success_rate,
                    first_failure_at: stats.first_failure_at,
                    last_success_latency_ms_p95: None,
                    repair_dispatched: failures > 0,
                    pending_repairs: 0,
                    ticket_id: None,
                })
            })
            .collect();

        top_offenders.sort_by(|left, right| match right.failures.cmp(&left.failures) {
            Ordering::Equal => right.forced.cmp(&left.forced),
            other => other,
        });
        if top_offenders.len() > 10 {
            top_offenders.truncate(10);
        }

        let report = PorWeeklyReportV1 {
            version: POR_WEEKLY_REPORT_VERSION_V1,
            cycle,
            generated_at: unix_now(),
            challenges_total,
            challenges_verified,
            challenges_failed,
            forced_challenges,
            repairs_enqueued: 0,
            repairs_completed: 0,
            mean_latency_ms: None,
            p95_latency_ms: None,
            slashing_events: Vec::new(),
            providers_missing_vrf,
            top_offenders,
            notes: None,
        };
        report
            .validate()
            .map_err(PorCoordinatorError::InvalidWeeklyReport)?;
        Ok(report)
    }

    /// Construct a manual challenge from an auditor request.
    ///
    /// # Errors
    ///
    /// Returns [`PorCoordinatorError::InvalidManualChallenge`] if the request
    /// payload fails validation or [`PorCoordinatorError::InvalidChallenge`]
    /// when the resulting challenge becomes inconsistent.
    pub fn build_manual_challenge(
        manual: &ManualPorChallengeV1,
        base: &PorChallengeV1,
    ) -> Result<PorChallengeV1, PorCoordinatorError> {
        manual
            .validate()
            .map_err(PorCoordinatorError::InvalidManualChallenge)?;
        let mut challenge = base.clone();
        challenge.sample_count = manual.requested_samples.unwrap_or(challenge.sample_count);
        if let Some(deadline_secs) = manual.requested_deadline_secs {
            challenge.deadline_at = challenge.issued_at.saturating_add(u64::from(deadline_secs));
        }
        challenge
            .validate()
            .map_err(PorCoordinatorError::InvalidChallenge)?;
        Ok(challenge)
    }

    /// Persist coordinator state to the configured backing store, if present.
    ///
    /// # Errors
    ///
    /// Returns [`PorCoordinatorError::Persistence`] when the persistence layer
    /// encounters a failure.
    fn persist(&self) -> Result<(), PorCoordinatorError> {
        if let Some(persistence) = &self.persistence {
            let mut records: Vec<_> = self
                .records
                .iter()
                .map(|entry| entry.value().clone())
                .collect();
            records.sort_by(|left, right| {
                left.challenge
                    .challenge_id
                    .cmp(&right.challenge.challenge_id)
            });

            let forced_guard = self.forced_providers.read();
            let mut forced: Vec<_> = forced_guard
                .iter()
                .map(|(provider, epochs)| (*provider, epochs.iter().copied().collect::<Vec<_>>()))
                .collect();
            forced.sort_by(|left, right| left.0.cmp(&right.0));
            drop(forced_guard);

            persistence.store(&records, &forced)?;
        }
        Ok(())
    }

    fn track_forced(&self, provider_id: &[u8; 32], epoch: u64) {
        let mut guard = self.forced_providers.write();
        guard.entry(*provider_id).or_default().insert(epoch);
    }
}

impl Default for PorCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct ChallengeRecord {
    challenge: PorChallengeV1,
    proof_digest: Option<[u8; 32]>,
    responded_at: Option<u64>,
    verdict: Option<RecordedVerdict>,
    repair_history_id: Option<u64>,
}

impl ChallengeRecord {
    fn from_challenge(challenge: PorChallengeV1) -> Self {
        Self {
            challenge,
            proof_digest: None,
            responded_at: None,
            verdict: None,
            repair_history_id: None,
        }
    }

    fn ensure_consistency(
        &self,
        manifest_digest: [u8; 32],
        provider_id: [u8; 32],
    ) -> Result<(), PorCoordinatorError> {
        if self.challenge.manifest_digest != manifest_digest {
            return Err(PorCoordinatorError::ManifestMismatch {
                expected: self.challenge.manifest_digest,
                actual: manifest_digest,
                expected_hex: hex::encode(self.challenge.manifest_digest),
                actual_hex: hex::encode(manifest_digest),
            });
        }
        if self.challenge.provider_id != provider_id {
            return Err(PorCoordinatorError::ProviderMismatch {
                expected: self.challenge.provider_id,
                actual: provider_id,
                expected_hex: hex::encode(self.challenge.provider_id),
                actual_hex: hex::encode(provider_id),
            });
        }
        Ok(())
    }

    fn to_status(&self) -> PorChallengeStatusV1 {
        let mut status = PorChallengeStatusV1 {
            version: POR_CHALLENGE_STATUS_VERSION_V1,
            challenge_id: self.challenge.challenge_id,
            manifest_digest: self.challenge.manifest_digest,
            provider_id: self.challenge.provider_id,
            epoch_id: self.challenge.epoch_id,
            drand_round: self.challenge.drand_round,
            status: PorChallengeOutcome::Pending,
            sample_count: self.challenge.sample_count,
            forced: self.challenge.forced,
            issued_at: self.challenge.issued_at,
            responded_at: self.responded_at,
            proof_digest: self.proof_digest,
            repair_task_id: self.repair_history_id.map(|id| {
                let mut bytes = [0u8; 16];
                bytes[..8].copy_from_slice(&id.to_le_bytes());
                bytes
            }),
            failure_reason: None,
            verifier_latency_ms: None,
        };

        if let Some(verdict) = &self.verdict {
            status.status = match verdict.outcome {
                AuditOutcomeV1::Success => PorChallengeOutcome::Verified,
                AuditOutcomeV1::Failed => PorChallengeOutcome::Failed,
                AuditOutcomeV1::Repaired => PorChallengeOutcome::Repaired,
            };
            if verdict.outcome != AuditOutcomeV1::Success {
                status.failure_reason.clone_from(&verdict.failure_reason);
            }
            if status.responded_at.is_none() {
                status.responded_at = Some(verdict.decided_at);
            }
            if status.proof_digest.is_none() {
                status.proof_digest = verdict.proof_digest;
            }
        } else if self.challenge.forced {
            status.status = PorChallengeOutcome::Forced;
        }

        status
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct ChallengeRecordSnapshot {
    challenge: PorChallengeV1,
    proof_digest: Option<[u8; 32]>,
    responded_at: Option<u64>,
    verdict: Option<RecordedVerdictSnapshot>,
    repair_history_id: Option<u64>,
}

impl<'a> norito::core::DecodeFromSlice<'a> for ChallengeRecordSnapshot {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        norito::core::decode_field_canonical::<ChallengeRecordSnapshot>(bytes)
    }
}

impl From<&ChallengeRecord> for ChallengeRecordSnapshot {
    fn from(record: &ChallengeRecord) -> Self {
        Self {
            challenge: record.challenge.clone(),
            proof_digest: record.proof_digest,
            responded_at: record.responded_at,
            verdict: record.verdict.as_ref().map(RecordedVerdictSnapshot::from),
            repair_history_id: record.repair_history_id,
        }
    }
}

impl ChallengeRecordSnapshot {
    fn into_record(self) -> Result<ChallengeRecord, PorPersistenceError> {
        let verdict = match self.verdict {
            Some(snapshot) => Some(snapshot.into_recorded_verdict()?),
            None => None,
        };
        Ok(ChallengeRecord {
            challenge: self.challenge,
            proof_digest: self.proof_digest,
            responded_at: self.responded_at,
            verdict,
            repair_history_id: self.repair_history_id,
        })
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct ForcedProviderSnapshot {
    provider_id: [u8; 32],
    epochs: Vec<u64>,
}

impl ForcedProviderSnapshot {
    fn into_set(self) -> BTreeSet<u64> {
        self.epochs.into_iter().collect()
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for ForcedProviderSnapshot {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        norito::core::decode_field_canonical::<ForcedProviderSnapshot>(bytes)
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct PorCoordinatorSnapshot {
    version: u8,
    records: Vec<ChallengeRecordSnapshot>,
    forced: Vec<ForcedProviderSnapshot>,
}

impl<'a> norito::core::DecodeFromSlice<'a> for PorCoordinatorSnapshot {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        norito::core::decode_field_canonical::<PorCoordinatorSnapshot>(bytes)
    }
}

/// Errors that may occur when reading or writing PoR persistence snapshots.
#[derive(Debug, Error)]
pub enum PorPersistenceError {
    /// Underlying filesystem I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Failed to serialize the snapshot payload.
    #[error("encode error: {0}")]
    Encode(#[source] norito::core::Error),
    /// Failed to deserialize persisted state.
    #[error("decode error: {0}")]
    Decode(String),
    /// Snapshot version on disk does not match the supported one.
    #[error("unsupported snapshot version {found}")]
    UnsupportedVersion {
        /// Version byte found in the snapshot file.
        found: u8,
    },
    /// Encountered an unexpected flag while parsing snapshot contents.
    #[error("invalid flag value {value}")]
    InvalidFlag {
        /// Flag value carrying invalid data.
        value: u8,
    },
}

#[derive(Debug)]
struct PorPersistence {
    path: PathBuf,
    temp_path: PathBuf,
}

impl PorPersistence {
    fn new(path: PathBuf) -> Self {
        let temp_path = path.with_added_extension("tmp");
        Self { path, temp_path }
    }

    /// Load persisted coordinator state from disk if present.
    ///
    /// # Errors
    ///
    /// Returns [`PorPersistenceError`] when the snapshot cannot be read or decoded.
    fn load(
        &self,
    ) -> Result<
        (
            Arc<DashMap<[u8; 32], ChallengeRecord>>,
            Arc<RwLock<HashMap<[u8; 32], BTreeSet<u64>>>>,
        ),
        PorPersistenceError,
    > {
        let records = Arc::new(DashMap::new());
        let forced = Arc::new(RwLock::new(HashMap::new()));

        let candidate = if self.path.exists() {
            Some(self.path.clone())
        } else if self.temp_path.exists() {
            Some(self.temp_path.clone())
        } else {
            None
        };

        let Some(path) = candidate else {
            return Ok((records, forced));
        };

        let bytes = fs::read(path)?;
        if bytes.is_empty() {
            return Ok((records, forced));
        }

        let snapshot = decode_from_bytes::<PorCoordinatorSnapshot>(&bytes)
            .map_err(|err| PorPersistenceError::Decode(err.to_string()))?;
        if snapshot.version != POR_COORDINATOR_SNAPSHOT_VERSION_V1 {
            return Err(PorPersistenceError::UnsupportedVersion {
                found: snapshot.version,
            });
        }

        for record in snapshot.records {
            let record = record.into_record()?;
            records.insert(record.challenge.challenge_id, record);
        }

        let mut forced_guard = forced.write();
        for provider in snapshot.forced {
            forced_guard.insert(provider.provider_id, provider.into_set());
        }
        drop(forced_guard);

        Ok((records, forced))
    }

    /// Store the supplied coordinator snapshot to disk.
    ///
    /// # Errors
    ///
    /// Returns [`PorPersistenceError`] when the snapshot cannot be encoded or written.
    fn store(
        &self,
        records: &[ChallengeRecord],
        forced: &[([u8; 32], Vec<u64>)],
    ) -> Result<(), PorPersistenceError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let snapshot = PorCoordinatorSnapshot {
            version: POR_COORDINATOR_SNAPSHOT_VERSION_V1,
            records: records.iter().map(ChallengeRecordSnapshot::from).collect(),
            forced: forced
                .iter()
                .map(|(provider_id, epochs)| ForcedProviderSnapshot {
                    provider_id: *provider_id,
                    epochs: epochs.clone(),
                })
                .collect(),
        };

        let bytes = to_bytes(&snapshot).map_err(PorPersistenceError::Encode)?;

        fs::write(&self.temp_path, &bytes)?;
        fs::rename(&self.temp_path, &self.path)?;
        Ok(())
    }
}

#[cfg(feature = "app_api")]
/// Errors produced by the deterministic randomness provider.
#[derive(Debug, Error, Copy, Clone)]
pub enum RandomnessError {
    /// Failed to derive randomness material for the requested epoch.
    #[error("failed to derive deterministic randomness bytes")]
    Derivation,
}

#[cfg(feature = "app_api")]
/// Trait supplying randomness used to schedule PoR challenges.
pub trait RandomnessProvider: Send + Sync {
    /// Produce randomness for the specified epoch, returning the commitment used to plan challenges.
    ///
    /// # Errors
    ///
    /// Returns [`RandomnessError::Derivation`] when deterministic randomness cannot be derived.
    fn randomness_for_epoch(
        &self,
        epoch_id: u64,
        now_secs: u64,
        response_window_secs: u64,
    ) -> Result<PorRandomness, RandomnessError>;
}

#[cfg(feature = "app_api")]
/// Deterministic randomness provider derived from an optional seed.
#[derive(Debug, Clone, Copy)]
pub struct DeterministicRandomnessProvider {
    /// Optional seed mixed into the derived randomness.
    seed: Option<[u8; 32]>,
}

#[cfg(feature = "app_api")]
impl DeterministicRandomnessProvider {
    #[must_use]
    /// Create a new provider backed by the given seed.
    pub fn new(seed: Option<[u8; 32]>) -> Self {
        Self { seed }
    }

    fn mix_bytes(&self, domain: &[u8], epoch_id: u64, now_secs: u64, counter: u32) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(domain);
        hasher.update(&epoch_id.to_le_bytes());
        hasher.update(&now_secs.to_le_bytes());
        hasher.update(&counter.to_le_bytes());
        if let Some(seed) = self.seed {
            hasher.update(&seed);
        }
        *hasher.finalize().as_bytes()
    }
}

#[cfg(feature = "app_api")]
impl RandomnessProvider for DeterministicRandomnessProvider {
    fn randomness_for_epoch(
        &self,
        epoch_id: u64,
        now_secs: u64,
        response_window_secs: u64,
    ) -> Result<PorRandomness, RandomnessError> {
        let randomness = self.mix_bytes(b"sora:por:drand:v1", epoch_id, now_secs, 0);
        let mut signature = Vec::with_capacity(96);
        let mut counter = 0u32;
        while signature.len() < 96 {
            let chunk = self.mix_bytes(b"sora:por:signature:v1", epoch_id, now_secs, counter);
            signature.extend_from_slice(&chunk);
            counter = counter.saturating_add(1);
        }
        signature.truncate(96);

        Ok(PorRandomness {
            epoch_id,
            issued_at_unix: now_secs,
            response_window_secs,
            drand_round: epoch_id,
            drand_randomness: randomness,
            drand_signature: signature,
        })
    }
}

#[cfg(feature = "app_api")]
/// Errors collecting VRF materials required for PoR challenge planning.
#[derive(Debug, Error, Copy, Clone)]
pub enum VrfError {
    /// Failed to gather VRF bundle metadata from storage or governance.
    #[error("failed to collect provider VRF bundle map")]
    Collection,
}

#[cfg(feature = "app_api")]
/// Supplies VRF bundles required to plan PoR challenges.
pub trait VrfProvider: Send + Sync {
    /// Return the VRF bundles recorded for `epoch_id`.
    ///
    /// # Errors
    ///
    /// Returns [`VrfError::Collection`] when VRF bundles cannot be collected.
    fn vrf_bundles_for_epoch(
        &self,
        epoch_id: u64,
    ) -> Result<HashMap<[u8; 32], ManifestVrfBundle>, VrfError>;
}

#[cfg(feature = "app_api")]
/// No-op VRF provider used in environments without published bundles.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmptyVrfProvider;

#[cfg(feature = "app_api")]
impl VrfProvider for EmptyVrfProvider {
    fn vrf_bundles_for_epoch(
        &self,
        _epoch_id: u64,
    ) -> Result<HashMap<[u8; 32], ManifestVrfBundle>, VrfError> {
        Ok(HashMap::new())
    }
}

#[cfg(feature = "app_api")]
/// Errors emitted when publishing PoR governance artefacts.
#[derive(Debug, Error)]
pub enum GovernancePublishError {
    /// Failed while accessing the filesystem.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Norito or JSON serialisation failed prior to persistence.
    #[error("serialization error: {0}")]
    Serialisation(String),
}

#[cfg(feature = "app_api")]
/// Emits PoR governance artefacts to a backing store.
pub trait GovernancePublisher: Send + Sync {
    /// Persist a challenge payload together with its duplicate sample metadata.
    ///
    /// # Errors
    ///
    /// Returns [`GovernancePublishError`] when the payload cannot be persisted.
    fn publish_challenge(
        &self,
        challenge: &PorChallengeV1,
        duplicate_samples: usize,
    ) -> Result<(), GovernancePublishError>;

    /// Persist the weekly governance report.
    ///
    /// # Errors
    ///
    /// Returns [`GovernancePublishError`] when writing the report fails.
    fn publish_weekly_report(
        &self,
        report: &PorWeeklyReportV1,
    ) -> Result<(), GovernancePublishError>;
}

#[cfg(feature = "app_api")]
/// Governance publisher that materialises artefacts on the filesystem.
#[derive(Debug)]
pub struct FilesystemGovernancePublisher {
    root: PathBuf,
}

#[cfg(feature = "app_api")]
impl FilesystemGovernancePublisher {
    #[must_use]
    /// Create a publisher rooted at the supplied directory.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn write_json(&self, path: PathBuf, value: JsonValue) -> Result<(), GovernancePublishError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        } else {
            fs::create_dir_all(&self.root)?;
        }
        let body = json::to_json_pretty(&value)
            .map_err(|err| GovernancePublishError::Serialisation(err.to_string()))?;
        fs::write(path, body.as_bytes())?;
        Ok(())
    }
}

#[cfg(feature = "app_api")]
impl GovernancePublisher for FilesystemGovernancePublisher {
    fn publish_challenge(
        &self,
        challenge: &PorChallengeV1,
        duplicate_samples: usize,
    ) -> Result<(), GovernancePublishError> {
        let epoch_dir = self
            .root
            .join("challenges")
            .join(format!("{:010}", challenge.epoch_id));
        let mut payload = JsonMap::new();
        payload.insert(
            "challenge".into(),
            json::to_value(challenge)
                .map_err(|err| GovernancePublishError::Serialisation(err.to_string()))?,
        );
        payload.insert(
            "duplicate_samples".into(),
            JsonValue::from(duplicate_samples as u64),
        );
        let filename = format!("{}.json", hex::encode(challenge.challenge_id));
        self.write_json(epoch_dir.join(filename), JsonValue::Object(payload))
    }

    fn publish_weekly_report(
        &self,
        report: &PorWeeklyReportV1,
    ) -> Result<(), GovernancePublishError> {
        let mut payload = JsonMap::new();
        payload.insert(
            "report".into(),
            json::to_value(report)
                .map_err(|err| GovernancePublishError::Serialisation(err.to_string()))?,
        );
        let filename = format!("{}-{:02}.json", report.cycle.year, report.cycle.week);
        self.write_json(
            self.root.join("reports").join(filename),
            JsonValue::Object(payload),
        )
    }
}

#[cfg(feature = "app_api")]
/// Errors that can surface while running the PoR automation workflow.
#[derive(Debug, Error)]
pub enum PorAutomationError {
    /// Randomness provider failed to produce a value.
    #[error("randomness failure: {0}")]
    Randomness(#[from] RandomnessError),
    /// Failed to collect VRF information required for challenge planning.
    #[error("vrf provider failure: {0}")]
    Vrf(#[from] VrfError),
    /// Challenge planner failed to assemble a schedule.
    #[error("challenge planner failure: {0}")]
    Planner(#[from] PorChallengePlannerError),
    /// Storage backend encountered an error.
    #[error("storage error: {0}")]
    Storage(#[from] sorafs_node::PorTrackerError),
    /// Coordinator rejected the requested state change.
    #[error("coordinator error: {0}")]
    Coordinator(#[from] PorCoordinatorError),
    /// Governance publication step failed.
    #[error("governance publish failure: {0}")]
    Governance(#[from] GovernancePublishError),
    /// Timestamp arithmetic overflowed the supported range.
    #[error("timestamp overflow")]
    TimestampOverflow,
}

#[cfg(feature = "app_api")]
/// Runtime wiring PoR challenge scheduling, proof ingestion, and reporting automation.
pub struct PorCoordinatorRuntime {
    /// Storage backend responsible for persisting PoR-related records.
    storage: Arc<dyn PorStorage>,
    /// In-memory coordinator that validates challenges, proofs, and verdicts.
    coordinator: Arc<PorCoordinator>,
    /// Randomness provider used to derive deterministic challenge seeds.
    randomness: Arc<dyn RandomnessProvider>,
    /// Adapter supplying governance/peer VRF bundle metadata.
    vrf_provider: Arc<dyn VrfProvider>,
    /// Publisher invoked to emit governance-facing telemetry (reports, exports).
    publisher: Arc<dyn GovernancePublisher>,
    /// Interval between PoR epochs in seconds.
    epoch_interval_secs: u64,
    /// Response window duration granted to providers (seconds).
    response_window_secs: u64,
    /// Last epoch for which automation was executed successfully.
    last_epoch: AtomicU64,
    /// Marker tracking when weekly reports were last generated.
    last_report_marker: AtomicU64,
}

#[cfg(feature = "app_api")]
impl PorCoordinatorRuntime {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    /// Create a new runtime harness for PoR automation.
    pub fn new(
        storage: Arc<dyn PorStorage>,
        coordinator: Arc<PorCoordinator>,
        randomness: Arc<dyn RandomnessProvider>,
        vrf_provider: Arc<dyn VrfProvider>,
        publisher: Arc<dyn GovernancePublisher>,
        epoch_interval_secs: u64,
        response_window_secs: u64,
    ) -> Self {
        Self {
            storage,
            coordinator,
            randomness,
            vrf_provider,
            publisher,
            epoch_interval_secs: epoch_interval_secs.max(60),
            response_window_secs: response_window_secs.max(60),
            last_epoch: AtomicU64::new(u64::MAX),
            last_report_marker: AtomicU64::new(0),
        }
    }

    fn compute_epoch(&self, now_secs: u64) -> u64 {
        now_secs / self.epoch_interval_secs
    }

    /// Compute the ISO week marker for the supplied timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`PorAutomationError`] when the timestamp cannot be converted
    /// into a valid ISO week.
    fn compute_iso_marker(now_secs: u64) -> Result<(PorReportIsoWeek, u64), PorAutomationError> {
        let ts = i64::try_from(now_secs).map_err(|_| PorAutomationError::TimestampOverflow)?;
        let datetime = OffsetDateTime::from_unix_timestamp(ts)
            .map_err(|_| PorAutomationError::TimestampOverflow)?;
        let (year, week, _) = datetime.to_iso_week_date();
        let year_u16 = u16::try_from(year).map_err(|_| PorAutomationError::TimestampOverflow)?;
        let cycle = PorReportIsoWeek {
            year: year_u16,
            week,
        };
        cycle
            .validate()
            .map_err(PorCoordinatorError::InvalidIsoWeek)
            .map_err(PorAutomationError::Coordinator)?;
        let marker = (u64::from(cycle.year) << 8) | u64::from(cycle.week);
        Ok((cycle, marker))
    }

    /// Publish a weekly report if the ISO week marker has advanced.
    ///
    /// # Errors
    ///
    /// Returns [`PorAutomationError`] when report generation or publishing fails.
    fn publish_weekly_report_if_needed(&self, now_secs: u64) -> Result<(), PorAutomationError> {
        let (cycle, marker) = Self::compute_iso_marker(now_secs)?;
        if self.last_report_marker.load(AtomicOrdering::SeqCst) == marker {
            return Ok(());
        }
        let report = self
            .coordinator
            .weekly_report(cycle.clone())
            .map_err(PorAutomationError::Coordinator)?;
        self.publisher.publish_weekly_report(&report)?;
        self.last_report_marker
            .store(marker, AtomicOrdering::SeqCst);
        Ok(())
    }

    /// Execute automation logic for the specified timestamp (seconds since UNIX epoch).
    ///
    /// # Errors
    ///
    /// Returns [`PorAutomationError`] if randomness, storage, or publishing
    /// backends fail during execution.
    pub fn run_once_at(&self, now_secs: u64) -> Result<bool, PorAutomationError> {
        let epoch = self.compute_epoch(now_secs);
        if self.last_epoch.load(AtomicOrdering::SeqCst) == epoch {
            self.publish_weekly_report_if_needed(now_secs)?;
            return Ok(false);
        }

        let randomness =
            self.randomness
                .randomness_for_epoch(epoch, now_secs, self.response_window_secs)?;
        let vrf_map = self.vrf_provider.vrf_bundles_for_epoch(epoch)?;
        let planned = self.storage.plan_challenges(randomness.clone(), &vrf_map)?;

        if planned.is_empty() {
            self.last_epoch.store(epoch, AtomicOrdering::SeqCst);
            self.publish_weekly_report_if_needed(now_secs)?;
            return Ok(false);
        }

        for PlannedChallenge {
            challenge,
            duplicate_samples,
        } in planned
        {
            self.storage.record_challenge(&challenge)?;
            self.coordinator
                .record_challenge(&challenge)
                .map_err(PorAutomationError::Coordinator)?;
            if let Err(err) = self
                .publisher
                .publish_challenge(&challenge, duplicate_samples)
            {
                iroha_logger::error!(
                    ?err,
                    provider_id = %hex::encode(challenge.provider_id),
                    challenge_id = %hex::encode(challenge.challenge_id),
                    "failed to publish PoR challenge to governance DAG directory"
                );
                return Err(PorAutomationError::Governance(err));
            }
        }

        self.last_epoch.store(epoch, AtomicOrdering::SeqCst);
        self.publish_weekly_report_if_needed(now_secs)?;
        Ok(true)
    }

    /// Execute automation logic using the current system clock.
    ///
    /// # Errors
    ///
    /// Propagates [`PorAutomationError`] from [`Self::run_once_at`].
    pub fn run_once(&self) -> Result<bool, PorAutomationError> {
        self.run_once_at(unix_now())
    }

    /// Spawn a Tokio task that periodically runs [`run_once`](Self::run_once`) until shutdown.
    pub fn spawn(self: Arc<Self>, shutdown: ShutdownSignal) {
        const TICK_INTERVAL_SECS: u64 = 60;
        tokio::spawn(async move {
            let mut ticker = interval(StdDuration::from_secs(TICK_INTERVAL_SECS));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = shutdown.receive() => break,
                    _ = ticker.tick() => {
                        if let Err(err) = self.run_once() {
                            iroha_logger::error!(%err, "PoR coordinator runtime tick failed");
                        }
                    }
                }
            }
        });
    }
}

#[cfg(feature = "app_api")]
/// Storage abstraction required by the PoR automation runtime.
pub trait PorStorage: Send + Sync {
    /// Produce challenge plans for the supplied randomness and VRF dataset.
    ///
    /// # Errors
    ///
    /// Returns [`PorChallengePlannerError`] when planning fails.
    fn plan_challenges(
        &self,
        randomness: PorRandomness,
        vrf_records: &HashMap<[u8; 32], ManifestVrfBundle>,
    ) -> Result<Vec<PlannedChallenge>, PorChallengePlannerError>;

    /// Record the fact that a challenge was issued so providers can submit proofs later.
    ///
    /// # Errors
    ///
    /// Returns [`sorafs_node::PorTrackerError`] when the challenge cannot be persisted.
    fn record_challenge(
        &self,
        challenge: &PorChallengeV1,
    ) -> Result<(), sorafs_node::PorTrackerError>;
}

#[cfg(feature = "app_api")]
impl PorStorage for sorafs_node::NodeHandle {
    fn plan_challenges(
        &self,
        randomness: PorRandomness,
        vrf_records: &HashMap<[u8; 32], ManifestVrfBundle>,
    ) -> Result<Vec<PlannedChallenge>, PorChallengePlannerError> {
        self.plan_por_challenges(randomness, vrf_records)
    }

    fn record_challenge(
        &self,
        challenge: &PorChallengeV1,
    ) -> Result<(), sorafs_node::PorTrackerError> {
        self.record_por_challenge(challenge)
    }
}

#[derive(Default)]
struct ProviderStats {
    manifests: HashSet<[u8; 32]>,
    challenges: u32,
    successes: u32,
    failures: u32,
    forced: u32,
    first_failure_at: Option<u64>,
}

/// Parameters used for filtering status queries.
/// Filter parameters for querying recorded PoR status information.
#[derive(Clone, Copy, Debug, Default)]
pub struct PorStatusFilter {
    /// Restrict results to challenges involving this manifest digest.
    pub manifest: Option<[u8; 32]>,
    /// Restrict results to challenges issued to this provider.
    pub provider: Option<[u8; 32]>,
    /// Restrict results to a specific epoch identifier.
    pub epoch: Option<u64>,
    /// Restrict results to a given challenge outcome.
    pub status: Option<PorChallengeOutcome>,
}

impl PorStatusFilter {
    fn matches(&self, status: &PorChallengeStatusV1) -> bool {
        if let Some(manifest) = self.manifest {
            if status.manifest_digest != manifest {
                return false;
            }
        }
        if let Some(provider) = self.provider {
            if status.provider_id != provider {
                return false;
            }
        }
        if let Some(epoch) = self.epoch {
            if status.epoch_id != epoch {
                return false;
            }
        }
        if let Some(outcome) = self.status {
            if status.status != outcome {
                return false;
            }
        }
        true
    }
}

/// Errors returned by the PoR coordinator while processing challenges, proofs, or reports.
#[derive(Debug, Error)]
pub enum PorCoordinatorError {
    /// Challenge payload failed validation.
    #[error("challenge payload invalid: {0}")]
    InvalidChallenge(#[source] PorChallengeValidationError),
    /// Proof payload failed validation.
    #[error("proof payload invalid: {0}")]
    InvalidProof(#[source] sorafs_manifest::por::PorProofValidationError),
    /// Verdict payload failed validation.
    #[error("verdict payload invalid: {0}")]
    InvalidVerdict(#[source] sorafs_manifest::por::AuditVerdictValidationError),
    /// Manual challenge request failed validation.
    #[error("manual challenge invalid: {0}")]
    InvalidManualChallenge(#[source] sorafs_manifest::por::ManualPorChallengeValidationError),
    /// Weekly report failed validation.
    #[error("weekly report failed validation: {0}")]
    InvalidWeeklyReport(#[source] PorWeeklyReportValidationError),
    /// Export payload failed validation.
    #[error("export payload failed validation: {0}")]
    InvalidExport(#[source] PorStatusExportValidationError),
    /// Challenge already exists with different payload.
    #[error("challenge with id {challenge_id_hex} already recorded with different payload")]
    ChallengeConflict {
        /// Binary challenge identifier that conflicts with existing state.
        challenge_id: [u8; 32],
        /// Hexadecimal representation of the conflicting identifier.
        challenge_id_hex: String,
    },
    /// Proof already recorded for the given challenge.
    #[error("proof already recorded for challenge {challenge_id_hex}")]
    DuplicateProof {
        /// Challenge identifier receiving duplicate proof.
        challenge_id: [u8; 32],
        /// Hex representation of the challenge identifier.
        challenge_id_hex: String,
    },
    /// Challenge identifier not found.
    #[error("unknown challenge id {challenge_id_hex}")]
    UnknownChallenge {
        /// Missing challenge identifier.
        challenge_id: [u8; 32],
        /// Hex representation of the missing identifier.
        challenge_id_hex: String,
    },
    /// Submitted manifest digest does not match the expected digest.
    #[error("manifest digest mismatch (expected {expected_hex}, got {actual_hex})")]
    ManifestMismatch {
        /// Expected manifest digest.
        expected: [u8; 32],
        /// Actual manifest digest supplied in the proof.
        actual: [u8; 32],
        /// Expected digest as hex.
        expected_hex: String,
        /// Actual digest as hex.
        actual_hex: String,
    },
    /// Submitted provider identifier does not match the challenge metadata.
    #[error("provider id mismatch (expected {expected_hex}, got {actual_hex})")]
    ProviderMismatch {
        /// Expected provider identifier.
        expected: [u8; 32],
        /// Actual provider identifier.
        actual: [u8; 32],
        /// Expected identifier rendered as hex.
        expected_hex: String,
        /// Actual identifier rendered as hex.
        actual_hex: String,
    },
    /// ISO week input could not be parsed.
    #[error("invalid ISO week requested: {0}")]
    InvalidIsoWeek(#[source] PorReportIsoWeekValidationError),
    /// Failed to compute ISO week bounds from the supplied data.
    #[error("failed to compute ISO week bounds")]
    IsoWeekComputation,
    /// Underlying persistence failed.
    #[error("persistence failure: {0}")]
    Persistence(#[from] PorPersistenceError),
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn iso_week_bounds(
    cycle: PorReportIsoWeek,
) -> Result<(OffsetDateTime, OffsetDateTime), PorCoordinatorError> {
    let date = Date::from_iso_week_date(i32::from(cycle.year), cycle.week, Weekday::Monday)
        .map_err(|_| PorCoordinatorError::IsoWeekComputation)?;
    let start = date
        .with_hms(0, 0, 0)
        .map_err(|_| PorCoordinatorError::IsoWeekComputation)?
        .assume_utc();
    let end = start + Duration::weeks(1);
    Ok((start, end))
}

// ------------- Tests -------------
#[cfg(test)]
mod tests {
    use sorafs_manifest::{
        por::{
            POR_CHALLENGE_VERSION_V1, POR_PROOF_VERSION_V1, derive_challenge_id,
            derive_challenge_seed,
        },
        provider_advert::{AdvertSignature, SignatureAlgorithm},
    };
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn persistence_temp_path_preserves_suffixes() {
        let base = PathBuf::from("/tmp/por_snapshot.norito.json");
        let persistence = PorPersistence::new(base);
        assert_eq!(
            persistence.temp_path,
            Path::new("/tmp/por_snapshot.norito.json.tmp")
        );
    }

    fn sample_challenge(forced: bool) -> PorChallengeV1 {
        let manifest_digest = [0x22; 32];
        let provider_id = [0x33; 32];
        let epoch_id = 42;
        let drand_round = 77;
        let drand_randomness = [0x44; 32];
        let vrf_output = if forced { None } else { Some([0x66; 32]) };
        let sample_indices: Vec<u64> = (0..64).collect();
        let seed = derive_challenge_seed(
            &drand_randomness,
            vrf_output.as_ref(),
            &manifest_digest,
            epoch_id,
        );
        let challenge_id =
            derive_challenge_id(&seed, &manifest_digest, &provider_id, epoch_id, drand_round);
        PorChallengeV1 {
            version: POR_CHALLENGE_VERSION_V1,
            challenge_id,
            manifest_digest,
            provider_id,
            epoch_id,
            drand_round,
            drand_randomness,
            drand_signature: vec![0x55; 48],
            vrf_output,
            vrf_proof: if forced { None } else { Some(vec![0x77; 80]) },
            forced,
            chunking_profile: "sorafs.sf1@1.0.0".to_string(),
            seed,
            sample_tier: 1,
            sample_count: 64,
            sample_indices,
            issued_at: 1_700_000_000,
            deadline_at: 1_700_000_900,
        }
    }

    fn sample_proof(challenge: &PorChallengeV1) -> sorafs_manifest::por::PorProofV1 {
        sorafs_manifest::por::PorProofV1 {
            version: POR_PROOF_VERSION_V1,
            challenge_id: challenge.challenge_id,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            samples: (0..64)
                .map(|idx| sorafs_manifest::por::PorProofSampleV1 {
                    sample_index: idx,
                    chunk_offset: 0,
                    chunk_size: 4096,
                    chunk_digest: [0x10; 32],
                    leaf_digest: [0x20; 32],
                })
                .collect(),
            auth_path: vec![[0xAA; 32]],
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0xAB; 32],
                signature: vec![0xCD; 64],
            },
            submitted_at: 1_700_000_500,
        }
    }

    fn sample_verdict(challenge: &PorChallengeV1, outcome: AuditOutcomeV1) -> AuditVerdictV1 {
        AuditVerdictV1 {
            version: POR_WEEKLY_REPORT_VERSION_V1,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            challenge_id: challenge.challenge_id,
            proof_digest: Some([0x55; 32]),
            outcome,
            failure_reason: match outcome {
                AuditOutcomeV1::Success => None,
                AuditOutcomeV1::Failed | AuditOutcomeV1::Repaired => {
                    Some("digest mismatch".to_string())
                }
            },
            decided_at: 1_700_000_600,
            auditor_signatures: vec![AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0xAB; 32],
                signature: vec![0xCD; 64],
            }],
            metadata: Vec::new(),
        }
    }

    #[test]
    fn records_challenge_proof_and_verdict() {
        let coordinator = PorCoordinator::new();
        let challenge = sample_challenge(false);
        coordinator.record_challenge(&challenge).expect("challenge");
        let proof = sample_proof(&challenge);
        let proof_digest = proof.proof_digest();
        coordinator.record_proof(&proof).expect("proof");
        let verdict = sample_verdict(&challenge, AuditOutcomeV1::Success);
        coordinator
            .record_verdict(
                &verdict,
                PorVerdictOutcome {
                    stats: sorafs_node::PorVerdictStats {
                        success_samples: 64,
                        failed_samples: 0,
                    },
                    repair_history_id: None,
                    consecutive_failures: 0,
                    slash: None,
                },
            )
            .expect("verdict");
        let statuses = coordinator.query_statuses(&PorStatusFilter::default(), None, None);
        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert_eq!(status.status, PorChallengeOutcome::Verified);
        assert_eq!(status.proof_digest, Some(proof_digest));
    }

    #[test]
    fn weekly_report_compiles() {
        let coordinator = PorCoordinator::new();
        let mut challenge = sample_challenge(true);
        challenge.issued_at = 1_700_000_000;
        challenge.deadline_at = challenge.issued_at + 600;
        coordinator.record_challenge(&challenge).expect("challenge");
        let verdict = sample_verdict(&challenge, AuditOutcomeV1::Failed);
        coordinator
            .record_verdict(
                &verdict,
                PorVerdictOutcome {
                    stats: sorafs_node::PorVerdictStats {
                        success_samples: 0,
                        failed_samples: 64,
                    },
                    repair_history_id: Some(42),
                    consecutive_failures: 1,
                    slash: None,
                },
            )
            .expect("verdict");
        let cycle = PorReportIsoWeek {
            year: 2023,
            week: 46,
        };
        let report = coordinator.weekly_report(cycle).expect("report");
        assert_eq!(report.challenges_total, 1);
        assert_eq!(report.challenges_failed, 1);
        assert_eq!(report.top_offenders.len(), 1);
    }

    #[test]
    fn persistence_round_trip_restores_state() {
        let dir = tempdir().expect("temp dir");
        let snapshot_path = dir.path().join("por_snapshot.to");
        let expected_digest;

        {
            let coordinator =
                PorCoordinator::with_persistence(&snapshot_path).expect("coordinator");
            let challenge = sample_challenge(false);
            coordinator.record_challenge(&challenge).expect("challenge");
            let proof = sample_proof(&challenge);
            let proof_digest = proof.proof_digest();
            coordinator.record_proof(&proof).expect("proof");
            let verdict = sample_verdict(&challenge, AuditOutcomeV1::Repaired);
            coordinator
                .record_verdict(
                    &verdict,
                    PorVerdictOutcome {
                        stats: sorafs_node::PorVerdictStats {
                            success_samples: 48,
                            failed_samples: 16,
                        },
                        repair_history_id: Some(99),
                        consecutive_failures: 0,
                        slash: None,
                    },
                )
                .expect("verdict");
            expected_digest = proof_digest;
        }

        let coordinator =
            PorCoordinator::with_persistence(&snapshot_path).expect("reload coordinator");
        let statuses = coordinator.query_statuses(&PorStatusFilter::default(), None, None);
        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert_eq!(status.status, PorChallengeOutcome::Repaired);
        assert_eq!(status.proof_digest, Some(expected_digest));
        assert!(status.responded_at.is_some());
        let repair_task = status.repair_task_id.expect("repair id");
        let mut repair_bytes = [0u8; 8];
        repair_bytes.copy_from_slice(&repair_task[..8]);
        assert_eq!(u64::from_le_bytes(repair_bytes), 99);
    }

    #[cfg(feature = "app_api")]
    mod runtime {
        use std::{collections::HashMap, fs, path::Path, str::FromStr, sync::Arc};

        use iroha_config::base::util::Bytes;
        use iroha_data_model::{
            metadata::Metadata,
            name::Name,
            sorafs::capacity::{CapacityDeclarationRecord, ProviderId},
        };
        use sorafs_manifest::{
            BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder, PinPolicy,
            capacity::{
                CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, ChunkerCommitmentV1,
                LaneCommitmentV1,
            },
            provider_advert::StakePointer,
        };
        use sorafs_node::{NodeHandle, config::StorageConfig};
        use tempfile::tempdir;

        use super::*;
        use crate::sorafs::por::{RandomnessProvider, VrfProvider};

        #[derive(Clone)]
        struct StaticRandomnessProvider {
            randomness: PorRandomness,
        }

        impl RandomnessProvider for StaticRandomnessProvider {
            fn randomness_for_epoch(
                &self,
                _epoch_id: u64,
                _now_secs: u64,
                _response_window_secs: u64,
            ) -> Result<PorRandomness, RandomnessError> {
                Ok(self.randomness.clone())
            }
        }

        #[derive(Default, Clone)]
        struct StaticVrfProvider {
            map: HashMap<u64, HashMap<[u8; 32], ManifestVrfBundle>>,
        }

        impl StaticVrfProvider {
            fn with_entry(epoch: u64, manifest: [u8; 32], bundle: ManifestVrfBundle) -> Self {
                let mut map = HashMap::new();
                map.insert(epoch, HashMap::from([(manifest, bundle)]));
                Self { map }
            }
        }

        impl VrfProvider for StaticVrfProvider {
            fn vrf_bundles_for_epoch(
                &self,
                epoch_id: u64,
            ) -> Result<HashMap<[u8; 32], ManifestVrfBundle>, VrfError> {
                Ok(self.map.get(&epoch_id).cloned().unwrap_or_default())
            }
        }

        fn storage_config(root: &Path) -> StorageConfig {
            StorageConfig::builder()
                .enabled(true)
                .data_dir(root.join("storage"))
                .max_capacity_bytes(Bytes(1_u64 << 30))
                .build()
        }

        fn declare_capacity(handle: &NodeHandle, provider_id: [u8; 32]) {
            let declaration = CapacityDeclarationV1 {
                version: CAPACITY_DECLARATION_VERSION_V1,
                provider_id,
                stake: StakePointer {
                    pool_id: [0xAA; 32],
                    stake_amount: 1,
                },
                committed_capacity_gib: 128,
                chunker_commitments: vec![ChunkerCommitmentV1 {
                    profile_id: "sorafs.sf1@1.0.0".to_string(),
                    profile_aliases: None,
                    committed_gib: 128,
                    capability_refs: Vec::new(),
                }],
                lane_commitments: vec![LaneCommitmentV1 {
                    lane_id: "default".to_string(),
                    max_gib: 128,
                }],
                pricing: None,
                valid_from: 1,
                valid_until: 2,
                metadata: Vec::new(),
            };
            let payload = norito::to_bytes(&declaration).expect("encode declaration");
            let mut metadata = Metadata::default();
            metadata.insert(
                Name::from_str("profile.sample_multiplier").expect("metadata key"),
                1_u64,
            );
            let record = CapacityDeclarationRecord::new(
                ProviderId::new(provider_id),
                payload,
                declaration.committed_capacity_gib,
                1,
                1,
                2,
                metadata,
            );
            handle
                .record_capacity_declaration(&record)
                .expect("record capacity declaration");
        }

        fn ingest_manifest(
            handle: &NodeHandle,
            payload: &[u8],
        ) -> ([u8; 32], sorafs_manifest::ManifestV1) {
            let plan = sorafs_car::CarBuildPlan::single_file(payload).expect("plan");
            let digest = blake3::hash(payload);
            let manifest = ManifestBuilder::new()
                .root_cid(digest.as_bytes().to_vec())
                .dag_codec(DagCodecId(0x71))
                .chunking_from_profile(
                    sorafs_chunker::ChunkProfile::DEFAULT,
                    BLAKE3_256_MULTIHASH_CODE,
                )
                .content_length(plan.content_length)
                .car_digest(digest.into())
                .car_size(plan.content_length)
                .pin_policy(PinPolicy::default())
                .build()
                .expect("manifest");
            let mut reader = payload;
            handle
                .ingest_manifest(&manifest, &plan, &mut reader)
                .expect("ingest manifest");
            (manifest.digest().expect("digest").into(), manifest)
        }

        fn challenge_paths(root: &Path, epoch: u64) -> Vec<std::path::PathBuf> {
            let epoch_dir = root.join("challenges").join(format!("{epoch:010}"));
            if !epoch_dir.exists() {
                return Vec::new();
            }
            fs::read_dir(epoch_dir)
                .expect("challenge dir")
                .map(|entry| entry.expect("entry").path())
                .collect()
        }

        #[test]
        fn runtime_emits_governance_challenge_with_vrf() {
            let temp_dir = tempdir().expect("temp dir");
            let governance_dir = temp_dir.path().join("governance");
            let handle = NodeHandle::new(storage_config(temp_dir.path()));
            let provider_id = [0x11; 32];
            declare_capacity(&handle, provider_id);
            let payload = vec![0xAB; 512 * 1024];
            let (manifest_digest, _manifest) = ingest_manifest(&handle, &payload);

            let epoch_interval = 3_600;
            let epoch_id = 500_000;
            let now_secs = epoch_id * epoch_interval;
            let randomness = PorRandomness {
                epoch_id,
                issued_at_unix: now_secs,
                response_window_secs: 900,
                drand_round: 42_000,
                drand_randomness: [0x21; 32],
                drand_signature: vec![0xCD; 96],
            };
            let randomness_provider = Arc::new(StaticRandomnessProvider {
                randomness: randomness.clone(),
            });
            let vrf_provider = Arc::new(StaticVrfProvider::with_entry(
                epoch_id,
                manifest_digest,
                ManifestVrfBundle {
                    output: [0x55; 32],
                    proof: vec![0x66; 80],
                },
            ));
            let publisher = Arc::new(FilesystemGovernancePublisher::new(governance_dir.clone()));
            let coordinator = Arc::new(PorCoordinator::new());
            let storage: Arc<dyn PorStorage> = Arc::new(handle.clone());

            let runtime = Arc::new(PorCoordinatorRuntime::new(
                storage,
                coordinator.clone(),
                randomness_provider,
                vrf_provider,
                publisher,
                epoch_interval,
                randomness.response_window_secs,
            ));

            let triggered = runtime.run_once_at(now_secs).expect("runtime tick");
            assert!(triggered, "expected challenge scheduling on new epoch");

            let statuses = coordinator.query_statuses(&PorStatusFilter::default(), None, None);
            assert_eq!(statuses.len(), 1);
            let status = &statuses[0];
            assert_eq!(status.epoch_id, epoch_id);
            assert_eq!(status.drand_round, randomness.drand_round);
            assert!(!status.forced, "VRF should prevent forced flag");
            assert_eq!(status.status, PorChallengeOutcome::Pending);

            let challenge_files = challenge_paths(&governance_dir, epoch_id);
            assert_eq!(challenge_files.len(), 1, "challenge file emitted");
            let challenge_json = fs::read_to_string(&challenge_files[0]).expect("challenge json");
            let parsed: norito::json::Value =
                norito::json::from_str(&challenge_json).expect("parse challenge json");
            let duplicate_samples = parsed
                .as_object()
                .and_then(|map| map.get("duplicate_samples"))
                .and_then(norito::json::Value::as_u64)
                .expect("duplicate_samples");
            assert_eq!(duplicate_samples, 0);
            let forced_flag = parsed
                .as_object()
                .and_then(|map| map.get("challenge"))
                .and_then(|value| value.as_object())
                .and_then(|challenge| challenge.get("forced"))
                .and_then(norito::json::Value::as_bool)
                .expect("forced flag");
            assert!(!forced_flag);

            let reports_dir = governance_dir.join("reports");
            assert!(
                reports_dir.exists(),
                "weekly report directory should be created"
            );
            assert!(
                fs::read_dir(&reports_dir)
                    .expect("reports dir")
                    .next()
                    .is_some(),
                "weekly report emitted"
            );

            let retriggered = runtime.run_once_at(now_secs).expect("second tick");
            assert!(
                !retriggered,
                "re-running same epoch should not schedule new challenges"
            );
        }

        #[test]
        fn runtime_marks_forced_when_vrf_missing() {
            let temp_dir = tempdir().expect("temp dir");
            let governance_dir = temp_dir.path().join("governance");
            let handle = NodeHandle::new(storage_config(temp_dir.path()));
            let provider_id = [0x22; 32];
            declare_capacity(&handle, provider_id);
            let payload = vec![0xBC; 256 * 1024];
            let (_manifest_digest, _manifest) = ingest_manifest(&handle, &payload);

            let epoch_interval = 1_800;
            let epoch_id = 600_000;
            let now_secs = epoch_id * epoch_interval;
            let randomness = PorRandomness {
                epoch_id,
                issued_at_unix: now_secs,
                response_window_secs: 600,
                drand_round: 21_000,
                drand_randomness: [0x31; 32],
                drand_signature: vec![0xAA; 96],
            };
            let randomness_provider = Arc::new(StaticRandomnessProvider {
                randomness: randomness.clone(),
            });
            let vrf_provider = Arc::new(StaticVrfProvider::default());
            let publisher = Arc::new(FilesystemGovernancePublisher::new(governance_dir.clone()));
            let coordinator = Arc::new(PorCoordinator::new());
            let storage: Arc<dyn PorStorage> = Arc::new(handle.clone());

            let runtime = PorCoordinatorRuntime::new(
                storage,
                coordinator.clone(),
                randomness_provider,
                vrf_provider,
                publisher,
                epoch_interval,
                randomness.response_window_secs,
            );

            let triggered = runtime.run_once_at(now_secs).expect("runtime tick");
            assert!(triggered, "forced challenge should be scheduled");

            let statuses = coordinator.query_statuses(&PorStatusFilter::default(), None, None);
            assert_eq!(statuses.len(), 1);
            let status = &statuses[0];
            assert_eq!(status.epoch_id, epoch_id);
            assert!(status.forced, "missing VRF should mark challenge forced");

            let challenge_files = challenge_paths(&governance_dir, epoch_id);
            assert_eq!(challenge_files.len(), 1);
            let challenge_json = fs::read_to_string(&challenge_files[0]).expect("challenge json");
            let forced_flag = norito::json::from_str::<norito::json::Value>(&challenge_json)
                .expect("parse challenge json")
                .as_object()
                .and_then(|map| map.get("challenge"))
                .and_then(|value| value.as_object())
                .and_then(|challenge| challenge.get("forced"))
                .and_then(norito::json::Value::as_bool)
                .expect("forced flag");
            assert!(forced_flag);
        }
    }
}
