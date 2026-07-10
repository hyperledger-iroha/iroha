//! In-memory Proof-of-Retrievability coordinator used by Torii.
//!
//! This module collects governance-issued PoR challenges, provider proofs, and
//! audit verdicts so that operators can query historical outcomes and generate
//! weekly reports. When constructed with [`PorCoordinator::with_persistence`],
//! state is snapshotted to disk using Norito so operators can recover history
//! across restarts.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{Read as _, Write as _},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
#[cfg(feature = "app_api")]
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs as _},
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    time::Duration as StdDuration,
};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _};

#[cfg(feature = "app_api")]
use async_trait::async_trait;
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
use parking_lot::{Mutex, RwLock};
use sorafs_manifest::por::{
    AuditOutcomeV1, AuditVerdictV1, ManualPorChallengeV1, ManualPorChallengeValidationError,
    POR_CHALLENGE_STATUS_VERSION_V1, POR_WEEKLY_REPORT_VERSION_V1, PorChallengeOutcome,
    PorChallengeStatusV1, PorChallengeV1, PorChallengeValidationError, PorProviderSummaryV1,
    PorProviderSummaryValidationError, PorReportIsoWeek, PorReportIsoWeekValidationError,
    PorWeeklyReportV1, PorWeeklyReportValidationError, ProviderVrfSubmissionV1,
    ProviderVrfSubmissionValidationError, provider_vrf_input,
};
use sorafs_node::PorVerdictOutcome;
#[cfg(feature = "app_api")]
use sorafs_node::{
    ManifestVrfBundle, ManifestVrfKey, PlannedChallenge, PorChallengePlannerError, PorRandomness,
};
use thiserror::Error;
use time::{Date, Duration, OffsetDateTime, Weekday};
#[cfg(feature = "app_api")]
use tokio::time::{MissedTickBehavior, interval};

const POR_STATUS_EXPORT_VERSION_V1: u8 = 1;
const POR_COORDINATOR_SNAPSHOT_VERSION_V1: u8 = 1;
const MAX_POR_COORDINATOR_RECORDS: usize = 65_536;

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
    mutation_lock: Arc<Mutex<()>>,
    pipeline_lock: Arc<tokio::sync::Mutex<()>>,
}

impl PorCoordinator {
    /// Construct an empty coordinator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Arc::new(DashMap::new()),
            forced_providers: Arc::new(RwLock::new(HashMap::new())),
            persistence: None,
            mutation_lock: Arc::new(Mutex::new(())),
            pipeline_lock: Arc::new(tokio::sync::Mutex::new(())),
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
            mutation_lock: Arc::new(Mutex::new(())),
            pipeline_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    /// Serialize the Torii/node dual-state PoR submission pipeline.
    pub(crate) async fn lock_pipeline(&self) -> tokio::sync::OwnedMutexGuard<()> {
        Arc::clone(&self.pipeline_lock).lock_owned().await
    }

    /// Record a governance-issued challenge.
    ///
    /// # Errors
    ///
    /// Returns [`PorCoordinatorError::InvalidChallenge`] when validation fails,
    /// [`PorCoordinatorError::DuplicateChallenge`] for an exact replay,
    /// [`PorCoordinatorError::ChallengeConflict`] if a different challenge is
    /// already recorded under the same identifier, or
    /// [`PorCoordinatorError::Persistence`] when persistence updates fail.
    pub(crate) fn record_challenge(
        &self,
        challenge: &PorChallengeV1,
    ) -> Result<(), PorCoordinatorError> {
        challenge
            .validate()
            .map_err(PorCoordinatorError::InvalidChallenge)?;
        let _mutation = self.mutation_lock.lock();
        match self.records.entry(challenge.challenge_id) {
            dashmap::mapref::entry::Entry::Occupied(existing) => {
                if existing.get().challenge != *challenge {
                    return Err(PorCoordinatorError::ChallengeConflict {
                        challenge_id: challenge.challenge_id,
                        challenge_id_hex: hex::encode(challenge.challenge_id),
                    });
                }
                Err(PorCoordinatorError::DuplicateChallenge {
                    challenge_id: challenge.challenge_id,
                    challenge_id_hex: hex::encode(challenge.challenge_id),
                })
            }
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                if self.records.len() >= MAX_POR_COORDINATOR_RECORDS {
                    return Err(PorCoordinatorError::RetentionExhausted {
                        limit: MAX_POR_COORDINATOR_RECORDS,
                    });
                }
                let record = ChallengeRecord::from_challenge(challenge.clone());
                if record.challenge.forced {
                    self.track_forced(&record.challenge.provider_id, record.challenge.epoch_id);
                }
                vacant.insert(record);
                if let Err(error) = self.persist() {
                    self.records.remove(&challenge.challenge_id);
                    if challenge.forced {
                        self.untrack_forced(&challenge.provider_id, challenge.epoch_id);
                    }
                    return Err(error);
                }
                Ok(())
            }
        }
    }

    /// Record a provider proof submission.
    ///
    /// # Errors
    ///
    /// Returns [`PorCoordinatorError::InvalidProof`] if validation fails,
    /// [`PorCoordinatorError::DuplicateProof`] for any replay,
    /// [`PorCoordinatorError::UnknownChallenge`] when the challenge cannot be
    /// found, or [`PorCoordinatorError::Persistence`] if persisting updates
    /// fails.
    pub(crate) fn record_proof(
        &self,
        proof: &sorafs_manifest::por::PorProofV1,
        admitted_provider_key: &[u8],
    ) -> Result<(), PorCoordinatorError> {
        proof
            .validate()
            .map_err(PorCoordinatorError::InvalidProof)?;
        proof
            .verify_signature_for_provider(admitted_provider_key)
            .map_err(PorCoordinatorError::InvalidProofSignature)?;
        let _mutation = self.mutation_lock.lock();
        let digest = proof.proof_digest();
        let previous = {
            let mut entry = self.records.get_mut(&proof.challenge_id).ok_or_else(|| {
                PorCoordinatorError::UnknownChallenge {
                    challenge_id: proof.challenge_id,
                    challenge_id_hex: hex::encode(proof.challenge_id),
                }
            })?;
            entry.ensure_consistency(proof.manifest_digest, proof.provider_id)?;
            if !proof
                .samples
                .iter()
                .map(|sample| sample.sample_index)
                .eq(entry.challenge.sample_indices.iter().copied())
            {
                return Err(PorCoordinatorError::SampleIndicesMismatch {
                    challenge_id: proof.challenge_id,
                    challenge_id_hex: hex::encode(proof.challenge_id),
                });
            }
            if proof.submitted_at < entry.challenge.issued_at
                || proof.submitted_at > entry.challenge.deadline_at
            {
                return Err(PorCoordinatorError::ProofOutsideChallengeWindow {
                    submitted_at: proof.submitted_at,
                    issued_at: entry.challenge.issued_at,
                    deadline_at: entry.challenge.deadline_at,
                });
            }
            if entry.proof_digest.is_some() {
                return Err(PorCoordinatorError::DuplicateProof {
                    challenge_id: proof.challenge_id,
                    challenge_id_hex: hex::encode(proof.challenge_id),
                });
            }
            let previous = entry.clone();
            entry.proof_digest = Some(digest);
            entry.proof_submitted_at = Some(proof.submitted_at);
            entry.responded_at = Some(proof.submitted_at);
            previous
        };
        if let Err(error) = self.persist() {
            self.records.insert(proof.challenge_id, previous);
            return Err(error);
        }
        Ok(())
    }

    /// Roll back a just-recorded challenge after the node-side commit failed.
    pub(crate) fn rollback_challenge(
        &self,
        challenge: &PorChallengeV1,
    ) -> Result<(), PorCoordinatorError> {
        let _mutation = self.mutation_lock.lock();
        let Some((_, record)) = self.records.remove(&challenge.challenge_id) else {
            return Ok(());
        };
        if record.challenge != *challenge
            || record.proof_digest.is_some()
            || record.verdict.is_some()
        {
            self.records.insert(challenge.challenge_id, record);
            return Err(PorCoordinatorError::RollbackConflict {
                challenge_id: challenge.challenge_id,
                challenge_id_hex: hex::encode(challenge.challenge_id),
            });
        }
        if challenge.forced {
            self.untrack_forced(&challenge.provider_id, challenge.epoch_id);
        }
        if let Err(error) = self.persist() {
            if challenge.forced {
                self.track_forced(&challenge.provider_id, challenge.epoch_id);
            }
            self.records.insert(challenge.challenge_id, record);
            return Err(error);
        }
        Ok(())
    }

    /// Roll back a just-recorded proof after the node-side commit failed.
    pub(crate) fn rollback_proof(
        &self,
        proof: &sorafs_manifest::por::PorProofV1,
    ) -> Result<(), PorCoordinatorError> {
        let _mutation = self.mutation_lock.lock();
        let digest = proof.proof_digest();
        let previous = {
            let mut entry = self.records.get_mut(&proof.challenge_id).ok_or_else(|| {
                PorCoordinatorError::UnknownChallenge {
                    challenge_id: proof.challenge_id,
                    challenge_id_hex: hex::encode(proof.challenge_id),
                }
            })?;
            if entry.proof_digest != Some(digest) || entry.verdict.is_some() {
                return Err(PorCoordinatorError::RollbackConflict {
                    challenge_id: proof.challenge_id,
                    challenge_id_hex: hex::encode(proof.challenge_id),
                });
            }
            let previous = entry.clone();
            entry.proof_digest = None;
            entry.proof_submitted_at = None;
            entry.responded_at = None;
            previous
        };
        if let Err(error) = self.persist() {
            self.records.insert(proof.challenge_id, previous);
            return Err(error);
        }
        Ok(())
    }

    /// Roll back a just-recorded verdict after the node-side commit failed.
    pub(crate) fn rollback_verdict(
        &self,
        verdict: &AuditVerdictV1,
    ) -> Result<(), PorCoordinatorError> {
        let _mutation = self.mutation_lock.lock();
        let previous = {
            let mut entry = self.records.get_mut(&verdict.challenge_id).ok_or_else(|| {
                PorCoordinatorError::UnknownChallenge {
                    challenge_id: verdict.challenge_id,
                    challenge_id_hex: hex::encode(verdict.challenge_id),
                }
            })?;
            if entry.verdict.as_ref().is_none_or(|recorded| {
                recorded.outcome != verdict.outcome
                    || recorded.proof_digest != verdict.proof_digest
                    || recorded.decided_at != verdict.decided_at
            }) {
                return Err(PorCoordinatorError::RollbackConflict {
                    challenge_id: verdict.challenge_id,
                    challenge_id_hex: hex::encode(verdict.challenge_id),
                });
            }
            let previous = entry.clone();
            entry.verdict = None;
            entry.repair_history_id = None;
            entry.responded_at = entry.proof_submitted_at;
            previous
        };
        if let Err(error) = self.persist() {
            self.records.insert(verdict.challenge_id, previous);
            return Err(error);
        }
        Ok(())
    }

    /// Attach node-side repair history after both lifecycle stores committed.
    pub(crate) fn update_verdict_outcome(
        &self,
        challenge_id: [u8; 32],
        outcome: &PorVerdictOutcome,
    ) -> Result<(), PorCoordinatorError> {
        let _mutation = self.mutation_lock.lock();
        let Some(repair_history_id) = outcome.repair_history_id else {
            return Ok(());
        };
        let previous = {
            let mut entry = self.records.get_mut(&challenge_id).ok_or_else(|| {
                PorCoordinatorError::UnknownChallenge {
                    challenge_id,
                    challenge_id_hex: hex::encode(challenge_id),
                }
            })?;
            let previous = entry.clone();
            entry.repair_history_id = Some(repair_history_id);
            previous
        };
        if let Err(error) = self.persist() {
            self.records.insert(challenge_id, previous);
            return Err(error);
        }
        Ok(())
    }

    /// Record an audit verdict emitted by governance.
    ///
    /// # Errors
    ///
    /// Returns [`PorCoordinatorError`] if the verdict is invalid, references an
    /// unknown challenge, or persistence fails.
    pub(crate) fn record_verdict(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
        outcome: PorVerdictOutcome,
    ) -> Result<(), PorCoordinatorError> {
        verdict
            .validate()
            .map_err(PorCoordinatorError::InvalidVerdict)?;
        verdict
            .verify_signatures_with_policy(trusted_auditor_keys, auditor_threshold)
            .map_err(PorCoordinatorError::InvalidVerdictSignature)?;
        let _mutation = self.mutation_lock.lock();
        let previous = {
            let mut entry = self.records.get_mut(&verdict.challenge_id).ok_or_else(|| {
                PorCoordinatorError::UnknownChallenge {
                    challenge_id: verdict.challenge_id,
                    challenge_id_hex: hex::encode(verdict.challenge_id),
                }
            })?;
            entry.ensure_consistency(verdict.manifest_digest, verdict.provider_id)?;
            if entry.verdict.is_some() {
                return Err(PorCoordinatorError::DuplicateVerdict {
                    challenge_id: verdict.challenge_id,
                    challenge_id_hex: hex::encode(verdict.challenge_id),
                });
            }
            entry.validate_verdict_transition(verdict)?;
            let previous = entry.clone();
            entry.verdict = Some(RecordedVerdict::from(verdict));
            entry.repair_history_id = outcome.repair_history_id;
            if entry.proof_digest.is_none() {
                entry.proof_digest = verdict.proof_digest;
            }
            if entry.responded_at.is_none() {
                entry.responded_at = Some(verdict.decided_at);
            }
            previous
        };
        if let Err(error) = self.persist() {
            self.records.insert(verdict.challenge_id, previous);
            return Err(error);
        }
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
        self.weekly_report_at(cycle, unix_now())
    }

    fn weekly_report_at(
        &self,
        cycle: PorReportIsoWeek,
        generated_at: u64,
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

        let mut provider_map: BTreeMap<[u8; 32], ProviderStats> = BTreeMap::new();
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
            .collect::<Vec<_>>();

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
                let success_rate_bps = if challenges == 0 {
                    10_000
                } else {
                    u16::try_from((u64::from(successes) * 10_000_u64) / u64::from(challenges))
                        .unwrap_or(10_000)
                };
                Some(PorProviderSummaryV1 {
                    provider_id: *provider_id,
                    manifest_count: stats.manifests.len() as u32,
                    challenges,
                    successes,
                    failures,
                    forced,
                    success_rate_bps,
                    first_failure_at: stats.first_failure_at,
                    last_success_latency_ms_p95: None,
                    repair_dispatched: failures > 0,
                    pending_repairs: 0,
                    ticket_id: None,
                })
            })
            .collect();

        top_offenders.sort_by(|left, right| match right.failures.cmp(&left.failures) {
            Ordering::Equal => match right.forced.cmp(&left.forced) {
                Ordering::Equal => left.provider_id.cmp(&right.provider_id),
                other => other,
            },
            other => other,
        });
        if top_offenders.len() > 10 {
            top_offenders.truncate(10);
        }

        let report = PorWeeklyReportV1 {
            version: POR_WEEKLY_REPORT_VERSION_V1,
            cycle,
            generated_at,
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

    fn untrack_forced(&self, provider_id: &[u8; 32], epoch: u64) {
        let mut guard = self.forced_providers.write();
        if let Some(epochs) = guard.get_mut(provider_id) {
            epochs.remove(&epoch);
            if epochs.is_empty() {
                guard.remove(provider_id);
            }
        }
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
    proof_submitted_at: Option<u64>,
    responded_at: Option<u64>,
    verdict: Option<RecordedVerdict>,
    repair_history_id: Option<u64>,
}

impl ChallengeRecord {
    fn from_challenge(challenge: PorChallengeV1) -> Self {
        Self {
            challenge,
            proof_digest: None,
            proof_submitted_at: None,
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

    fn validate_verdict_transition(
        &self,
        verdict: &AuditVerdictV1,
    ) -> Result<(), PorCoordinatorError> {
        if verdict.decided_at < self.challenge.issued_at {
            return Err(PorCoordinatorError::VerdictBeforeChallenge {
                decided_at: verdict.decided_at,
                issued_at: self.challenge.issued_at,
            });
        }
        match (self.proof_digest, verdict.proof_digest) {
            (Some(expected), Some(actual)) if expected != actual => {
                return Err(PorCoordinatorError::ProofDigestMismatch {
                    expected,
                    actual,
                    expected_hex: hex::encode(expected),
                    actual_hex: hex::encode(actual),
                });
            }
            (Some(_), None) => return Err(PorCoordinatorError::MissingVerdictProofDigest),
            (None, Some(_)) => return Err(PorCoordinatorError::UnexpectedVerdictProofDigest),
            (None, None)
                if matches!(
                    verdict.outcome,
                    AuditOutcomeV1::Success | AuditOutcomeV1::Repaired
                ) =>
            {
                return Err(PorCoordinatorError::MissingProofForSuccessfulVerdict);
            }
            _ => {}
        }
        if let Some(submitted_at) = self.proof_submitted_at
            && verdict.decided_at < submitted_at
        {
            return Err(PorCoordinatorError::VerdictBeforeProof {
                decided_at: verdict.decided_at,
                submitted_at,
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

    fn validate_persisted(&self) -> Result<(), String> {
        self.challenge
            .validate()
            .map_err(|error| error.to_string())?;
        if self.proof_digest.is_some() != self.proof_submitted_at.is_some() {
            return Err(
                "proof digest and submission timestamp must both be present or absent".to_owned(),
            );
        }
        if let Some(submitted_at) = self.proof_submitted_at
            && (submitted_at < self.challenge.issued_at
                || submitted_at > self.challenge.deadline_at)
        {
            return Err("proof submission timestamp is outside the challenge window".to_owned());
        }

        let expected_responded_at = match &self.verdict {
            None => {
                if self.repair_history_id.is_some() {
                    return Err("repair history cannot exist without a verdict".to_owned());
                }
                self.proof_submitted_at
            }
            Some(verdict) => {
                if verdict.decided_at < self.challenge.issued_at
                    || self
                        .proof_submitted_at
                        .is_some_and(|submitted_at| verdict.decided_at < submitted_at)
                {
                    return Err("verdict timestamp predates its challenge or proof".to_owned());
                }
                if verdict.proof_digest != self.proof_digest {
                    return Err(
                        "verdict proof digest does not match recorded proof state".to_owned()
                    );
                }
                match verdict.outcome {
                    AuditOutcomeV1::Success => {
                        if verdict.failure_reason.is_some()
                            || self.proof_digest.is_none()
                            || self.repair_history_id.is_some()
                        {
                            return Err(
                                "successful verdict has inconsistent proof, reason, or repair state"
                                    .to_owned(),
                            );
                        }
                    }
                    AuditOutcomeV1::Failed => {
                        if verdict
                            .failure_reason
                            .as_deref()
                            .is_none_or(|reason| reason.trim().is_empty())
                        {
                            return Err("failed verdict is missing a reason".to_owned());
                        }
                    }
                    AuditOutcomeV1::Repaired => {
                        if verdict
                            .failure_reason
                            .as_deref()
                            .is_none_or(|reason| reason.trim().is_empty())
                            || self.proof_digest.is_none()
                        {
                            return Err(
                                "repaired verdict is missing a proof or failure reason".to_owned()
                            );
                        }
                    }
                }
                if self.repair_history_id == Some(0) {
                    return Err("repair history id zero is reserved".to_owned());
                }
                self.proof_submitted_at.or(Some(verdict.decided_at))
            }
        };
        if self.responded_at != expected_responded_at {
            return Err("responded_at does not match the persisted lifecycle".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct ChallengeRecordSnapshot {
    challenge: PorChallengeV1,
    proof_digest: Option<[u8; 32]>,
    proof_submitted_at: Option<u64>,
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
            proof_submitted_at: record.proof_submitted_at,
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
            proof_submitted_at: self.proof_submitted_at,
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

const MAX_POR_COORDINATOR_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;
const SECURE_TEMP_RETRIES: usize = 8;

#[derive(Debug, Error)]
enum SecureFileError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsafe persistence path: {0}")]
    UnsafePath(String),
    #[error("persistence payload exceeds {limit} bytes")]
    Oversize { limit: usize },
    #[error("existing immutable artefact conflicts with canonical bytes")]
    Conflict,
}

fn absolute_secure_path(path: &Path) -> Result<PathBuf, SecureFileError> {
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(SecureFileError::UnsafePath(
            "parent-directory components are forbidden".to_owned(),
        ));
    }
    let candidate = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut absolute = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(SecureFileError::UnsafePath(
                    "parent-directory components are forbidden".to_owned(),
                ));
            }
            _ => absolute.push(component.as_os_str()),
        }
    }
    if absolute.file_name().is_none() {
        return Err(SecureFileError::UnsafePath(
            "persistence path must name a file".to_owned(),
        ));
    }
    Ok(absolute)
}

#[allow(unsafe_code)]
fn ensure_secure_parent(path: &Path) -> Result<(PathBuf, PathBuf, fs::Metadata), SecureFileError> {
    let absolute = absolute_secure_path(path)?;
    let parent = absolute
        .parent()
        .ok_or_else(|| SecureFileError::UnsafePath("persistence path has no parent".to_owned()))?;
    let mut cursor = PathBuf::new();
    for component in parent.components() {
        cursor.push(component.as_os_str());
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(SecureFileError::UnsafePath(format!(
                        "ancestor {} is not a regular directory",
                        cursor.display()
                    )));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut builder = fs::DirBuilder::new();
                #[cfg(unix)]
                builder.mode(0o700);
                builder.create(&cursor)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    let parent_path = parent.to_owned();
    let metadata = fs::symlink_metadata(&parent_path)?;
    #[cfg(unix)]
    // SAFETY: `geteuid` has no preconditions and does not dereference pointers.
    let effective_uid = unsafe { libc::geteuid() };
    #[cfg(unix)]
    if metadata.uid() != effective_uid || metadata.mode() & 0o077 != 0 {
        return Err(SecureFileError::UnsafePath(format!(
            "persistence directory {} must be owned by this process user and mode 0700",
            parent_path.display()
        )));
    }
    Ok((absolute, parent_path, metadata))
}

#[allow(unsafe_code)]
fn validate_secure_file_metadata(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), SecureFileError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SecureFileError::UnsafePath(format!(
            "{} is not a regular non-symlink file",
            path.display()
        )));
    }
    #[cfg(unix)]
    // SAFETY: `geteuid` has no preconditions and does not dereference pointers.
    let effective_uid = unsafe { libc::geteuid() };
    #[cfg(unix)]
    if metadata.uid() != effective_uid || metadata.mode() & 0o077 != 0 || metadata.nlink() != 1 {
        return Err(SecureFileError::UnsafePath(format!(
            "{} must be owned by this process user, private, and singly linked",
            path.display()
        )));
    }
    Ok(())
}

fn secure_read_bytes(path: &Path, max_bytes: usize) -> Result<Option<Vec<u8>>, SecureFileError> {
    let (absolute, _, _) = ensure_secure_parent(path)?;
    let metadata = match fs::symlink_metadata(&absolute) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    validate_secure_file_metadata(&absolute, &metadata)?;
    if metadata.len() > max_bytes as u64 {
        return Err(SecureFileError::Oversize { limit: max_bytes });
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let file = options.open(&absolute)?;
    let opened = file.metadata()?;
    validate_secure_file_metadata(&absolute, &opened)?;
    #[cfg(unix)]
    if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
        return Err(SecureFileError::UnsafePath(format!(
            "{} changed while opening",
            absolute.display()
        )));
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.take(max_bytes as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(SecureFileError::Oversize { limit: max_bytes });
    }
    Ok(Some(bytes))
}

fn sync_secure_directory(parent: &Path) -> Result<(), SecureFileError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW);
    options.open(parent)?.sync_all()?;
    Ok(())
}

fn secure_atomic_write(
    path: &Path,
    bytes: &[u8],
    max_bytes: usize,
    replace_existing: bool,
) -> Result<(), SecureFileError> {
    if bytes.len() > max_bytes {
        return Err(SecureFileError::Oversize { limit: max_bytes });
    }
    let (absolute, parent, parent_before) = ensure_secure_parent(path)?;
    if let Some(existing) = secure_read_bytes(&absolute, max_bytes)? {
        if existing == bytes {
            sync_secure_directory(&parent)?;
            return Ok(());
        }
        if !replace_existing {
            return Err(SecureFileError::Conflict);
        }
    }
    let filename = absolute
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            SecureFileError::UnsafePath("persistence filename is not UTF-8".to_owned())
        })?;
    let mut temp_file = None;
    let mut temp_path = PathBuf::new();
    for _ in 0..SECURE_TEMP_RETRIES {
        let nonce: [u8; 16] = rand::random();
        temp_path = parent.join(format!(".{filename}.{}.tmp", hex::encode(nonce)));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        match options.open(&temp_path) {
            Ok(file) => {
                temp_file = Some(file);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    let mut file = temp_file.ok_or_else(|| {
        SecureFileError::UnsafePath("failed to allocate a unique temporary file".to_owned())
    })?;
    let result = (|| {
        validate_secure_file_metadata(&temp_path, &file.metadata()?)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        let parent_now = fs::symlink_metadata(&parent)?;
        #[cfg(unix)]
        if parent_now.dev() != parent_before.dev() || parent_now.ino() != parent_before.ino() {
            return Err(SecureFileError::UnsafePath(
                "persistence parent changed before rename".to_owned(),
            ));
        }
        if let Ok(destination) = fs::symlink_metadata(&absolute) {
            validate_secure_file_metadata(&absolute, &destination)?;
        }
        if replace_existing {
            fs::rename(&temp_path, &absolute)?;
        } else {
            match fs::hard_link(&temp_path, &absolute) {
                Ok(()) => fs::remove_file(&temp_path)?,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    fs::remove_file(&temp_path)?;
                    return match secure_read_bytes(&absolute, max_bytes)? {
                        Some(existing) if existing == bytes => {
                            sync_secure_directory(&parent)?;
                            Ok(())
                        }
                        Some(_) => Err(SecureFileError::Conflict),
                        None => Err(SecureFileError::Io(error)),
                    };
                }
                Err(error) => return Err(error.into()),
            }
        }
        sync_secure_directory(&parent)?;
        let final_metadata = fs::symlink_metadata(&absolute)?;
        validate_secure_file_metadata(&absolute, &final_metadata)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
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
    /// Persistence path, size, ownership, or atomicity policy failed.
    #[error("secure persistence error: {0}")]
    Secure(String),
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
}

impl PorPersistence {
    fn new(path: PathBuf) -> Self {
        Self { path }
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

        let Some(bytes) = secure_read_bytes(&self.path, MAX_POR_COORDINATOR_SNAPSHOT_BYTES)
            .map_err(|error| PorPersistenceError::Secure(error.to_string()))?
        else {
            return Ok((records, forced));
        };

        let snapshot = decode_from_bytes::<PorCoordinatorSnapshot>(&bytes)
            .map_err(|err| PorPersistenceError::Decode(err.to_string()))?;
        let canonical = to_bytes(&snapshot).map_err(PorPersistenceError::Encode)?;
        if canonical != bytes {
            return Err(PorPersistenceError::Decode(
                "snapshot is not canonically encoded".to_owned(),
            ));
        }
        if snapshot.version != POR_COORDINATOR_SNAPSHOT_VERSION_V1 {
            return Err(PorPersistenceError::UnsupportedVersion {
                found: snapshot.version,
            });
        }

        if snapshot.records.len() > 65_536 || snapshot.forced.len() > 4_096 {
            return Err(PorPersistenceError::Decode(
                "snapshot entry count exceeds production bounds".to_owned(),
            ));
        }
        let mut expected_forced = HashMap::<[u8; 32], BTreeSet<u64>>::new();
        let mut previous_challenge_id = None;
        for record in snapshot.records {
            let record = record.into_record()?;
            record
                .validate_persisted()
                .map_err(PorPersistenceError::Decode)?;
            if previous_challenge_id
                .is_some_and(|previous| previous >= record.challenge.challenge_id)
            {
                return Err(PorPersistenceError::Decode(
                    "snapshot challenge records are not strictly ordered".to_owned(),
                ));
            }
            previous_challenge_id = Some(record.challenge.challenge_id);
            if record.challenge.forced {
                expected_forced
                    .entry(record.challenge.provider_id)
                    .or_default()
                    .insert(record.challenge.epoch_id);
            }
            if records
                .insert(record.challenge.challenge_id, record)
                .is_some()
            {
                return Err(PorPersistenceError::Decode(
                    "snapshot contains a duplicate challenge id".to_owned(),
                ));
            }
        }

        let mut forced_guard = forced.write();
        let mut previous_provider_id = None;
        for provider in snapshot.forced {
            if provider.provider_id.iter().all(|byte| *byte == 0)
                || provider.epochs.len() > 65_536
                || previous_provider_id.is_some_and(|previous| previous >= provider.provider_id)
            {
                return Err(PorPersistenceError::Decode(
                    "snapshot contains invalid or unordered forced-provider state".to_owned(),
                ));
            }
            previous_provider_id = Some(provider.provider_id);
            if provider.epochs.is_empty()
                || provider.epochs.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(PorPersistenceError::Decode(
                    "forced-provider epochs must be non-empty and strictly ordered".to_owned(),
                ));
            }
            forced_guard.insert(provider.provider_id, provider.into_set());
        }
        if *forced_guard != expected_forced {
            return Err(PorPersistenceError::Decode(
                "forced-provider index does not match forced challenge records".to_owned(),
            ));
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
        secure_atomic_write(&self.path, &bytes, MAX_POR_COORDINATOR_SNAPSHOT_BYTES, true)
            .map_err(|error| PorPersistenceError::Secure(error.to_string()))
    }
}

#[cfg(feature = "app_api")]
/// Errors produced by the verified drand randomness provider.
#[derive(Debug, Error)]
pub enum RandomnessError {
    /// Pinned trust, transport, or persistence configuration is unsafe.
    #[error("invalid drand configuration: {0}")]
    Configuration(String),
    /// A network endpoint failed before producing a verified beacon.
    #[error("drand endpoint failure: {0}")]
    Endpoint(String),
    /// Fewer agreeing endpoints than the configured strict majority responded.
    #[error("drand quorum unavailable: {agreeing} agreeing responses; {required} required")]
    QuorumUnavailable {
        /// Largest agreeing response group.
        agreeing: usize,
        /// Required agreement threshold.
        required: usize,
    },
    /// Verified beacon timing does not satisfy pinned freshness constraints.
    #[error("drand beacon timing invalid: {0}")]
    Timing(String),
    /// A verified round regressed below durable high-water state.
    #[error("drand round rollback: received {received}, durable high-water is {high_water}")]
    Rollback {
        /// Received round.
        received: u64,
        /// Durable high-water round.
        high_water: u64,
    },
    /// The same round produced different verified bytes.
    #[error("drand equivocation detected at round {round}")]
    Equivocation {
        /// Conflicting round.
        round: u64,
    },
    /// Durable high-water state failed closed.
    #[error("drand state persistence failure: {0}")]
    Persistence(String),
}

#[cfg(feature = "app_api")]
/// Trait supplying randomness used to schedule PoR challenges.
#[async_trait]
pub trait RandomnessProvider: Send + Sync {
    /// Produce randomness for the specified epoch, returning the commitment used to plan challenges.
    ///
    /// # Errors
    ///
    /// Returns [`RandomnessError`] when transport, verification, quorum,
    /// freshness, replay, or durable-state checks fail.
    async fn randomness_for_epoch(
        &self,
        epoch_id: u64,
        now_secs: u64,
        response_window_secs: u64,
    ) -> Result<PorRandomness, RandomnessError>;
}

#[cfg(feature = "app_api")]
const DRAND_STATE_VERSION_V1: u8 = 1;
#[cfg(feature = "app_api")]
const MAX_DRAND_DNS_ADDRESSES: usize = 16;
#[cfg(feature = "app_api")]
const MIN_DRAND_RESPONSE_BYTES: usize = 128;

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct DrandHighWaterStateV1 {
    version: u8,
    round: u64,
    randomness: [u8; 32],
    signature: [u8; iroha_crypto::drand::DRAND_SIGNATURE_BYTES],
}

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct VerifiedDrandBeacon {
    round: u64,
    randomness: [u8; 32],
    signature: [u8; iroha_crypto::drand::DRAND_SIGNATURE_BYTES],
}

#[cfg(feature = "app_api")]
#[derive(Debug)]
struct DrandEndpoint {
    root: url::Url,
    host: String,
    port: u16,
    pinned_addrs: Vec<SocketAddr>,
    client: reqwest::Client,
}

#[cfg(feature = "app_api")]
/// HTTPS drand provider with pinned chain metadata, DNS, quorum, and durable replay state.
#[derive(Debug)]
pub struct DrandHttpRandomnessProvider {
    public_key: [u8; iroha_crypto::drand::DRAND_PUBLIC_KEY_BYTES],
    genesis_time: u64,
    period_secs: u64,
    epoch_interval_secs: u64,
    quorum: usize,
    max_body_bytes: usize,
    max_beacon_age_secs: u64,
    max_future_skew_secs: u64,
    endpoints: Vec<DrandEndpoint>,
    state_path: PathBuf,
    state: Mutex<Option<DrandHighWaterStateV1>>,
    commit_lock: tokio::sync::Mutex<()>,
}

#[cfg(feature = "app_api")]
impl DrandHttpRandomnessProvider {
    /// Construct a provider after validating trust roots, endpoints, DNS, and persisted state.
    pub fn from_config(
        config: &iroha_config::parameters::actual::SorafsPorDrand,
        epoch_interval_secs: u64,
    ) -> Result<Self, RandomnessError> {
        use iroha_crypto::drand::{
            UNCHAINED_G1_RFC9380_SCHEME, is_valid_unchained_g1_rfc9380_public_key,
        };

        if config.scheme != UNCHAINED_G1_RFC9380_SCHEME {
            return Err(RandomnessError::Configuration(format!(
                "scheme must be `{UNCHAINED_G1_RFC9380_SCHEME}`"
            )));
        }
        if config.chain_hash.iter().all(|byte| *byte == 0) {
            return Err(RandomnessError::Configuration(
                "chain hash must be pinned".to_owned(),
            ));
        }
        if !is_valid_unchained_g1_rfc9380_public_key(&config.public_key) {
            return Err(RandomnessError::Configuration(
                "public key is not a canonical non-identity G2 point".to_owned(),
            ));
        }
        if config.genesis_time == 0 || config.period_secs == 0 || epoch_interval_secs == 0 {
            return Err(RandomnessError::Configuration(
                "genesis_time and period_secs must be non-zero".to_owned(),
            ));
        }
        if config.max_endpoints < 3
            || config.endpoints.len() < 3
            || config.endpoints.len() > config.max_endpoints
            || config.max_endpoints
                > iroha_config::parameters::defaults::sorafs::por::DRAND_MAX_ENDPOINTS
        {
            return Err(RandomnessError::Configuration(format!(
                "between 3 and {} drand endpoints are required",
                iroha_config::parameters::defaults::sorafs::por::DRAND_MAX_ENDPOINTS
            )));
        }
        let quorum = usize::from(config.quorum);
        if quorum <= config.endpoints.len() / 2 || quorum >= config.endpoints.len() {
            return Err(RandomnessError::Configuration(
                "drand quorum must be a strict majority and tolerate one endpoint outage"
                    .to_owned(),
            ));
        }
        if config.connect_timeout.is_zero() || config.request_timeout.is_zero() {
            return Err(RandomnessError::Configuration(
                "drand timeouts must be non-zero".to_owned(),
            ));
        }
        if config.max_body_bytes < MIN_DRAND_RESPONSE_BYTES || config.max_body_bytes > 64 * 1024 {
            return Err(RandomnessError::Configuration(format!(
                "max_body_bytes must be between {MIN_DRAND_RESPONSE_BYTES} and 65536"
            )));
        }
        if config.max_beacon_age_secs < config.period_secs
            || config.max_future_skew_secs > config.max_beacon_age_secs
        {
            return Err(RandomnessError::Configuration(
                "drand freshness/skew bounds are inconsistent".to_owned(),
            ));
        }

        let chain_hex = hex::encode(config.chain_hash);
        let expected_path = format!("/v2/chains/{chain_hex}");
        let mut seen_hosts = BTreeSet::new();
        let mut endpoints = Vec::with_capacity(config.endpoints.len());
        for raw_root in &config.endpoints {
            let root = url::Url::parse(raw_root).map_err(|error| {
                RandomnessError::Configuration(format!("invalid drand endpoint: {error}"))
            })?;
            let host = validate_drand_endpoint(raw_root, &root, &expected_path)?;
            if !seen_hosts.insert(host.clone()) {
                return Err(RandomnessError::Configuration(format!(
                    "duplicate drand endpoint host `{host}`"
                )));
            }
            let port = root.port_or_known_default().ok_or_else(|| {
                RandomnessError::Configuration("drand endpoint has no HTTPS port".to_owned())
            })?;
            let pinned_addrs = resolve_public_endpoint(&host, port)?;
            let client = reqwest::Client::builder()
                .https_only(true)
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(config.connect_timeout)
                .timeout(config.request_timeout)
                .resolve_to_addrs(&host, &pinned_addrs)
                .build()
                .map_err(|error| RandomnessError::Configuration(error.to_string()))?;
            endpoints.push(DrandEndpoint {
                root,
                host,
                port,
                pinned_addrs,
                client,
            });
        }

        let loaded = load_drand_state(&config.state_path, &config.public_key)?;
        Ok(Self {
            public_key: config.public_key,
            genesis_time: config.genesis_time,
            period_secs: config.period_secs,
            epoch_interval_secs,
            quorum,
            max_body_bytes: config.max_body_bytes,
            max_beacon_age_secs: config.max_beacon_age_secs,
            max_future_skew_secs: config.max_future_skew_secs,
            endpoints,
            state_path: config.state_path.clone(),
            state: Mutex::new(loaded),
            commit_lock: tokio::sync::Mutex::new(()),
        })
    }

    fn expected_round(&self, epoch_id: u64, now_secs: u64) -> Result<u64, RandomnessError> {
        let target = epoch_id
            .checked_mul(self.epoch_interval_secs)
            .ok_or_else(|| RandomnessError::Timing("epoch target overflow".to_owned()))?;
        if target > now_secs.saturating_add(self.max_future_skew_secs) {
            return Err(RandomnessError::Timing(
                "PoR epoch target is in the future".to_owned(),
            ));
        }
        if target < self.genesis_time {
            return Err(RandomnessError::Timing(
                "PoR epoch target predates pinned drand genesis".to_owned(),
            ));
        }
        let round = target
            .saturating_sub(self.genesis_time)
            .checked_div(self.period_secs)
            .and_then(|offset| offset.checked_add(1))
            .ok_or_else(|| RandomnessError::Timing("round arithmetic overflow".to_owned()))?;
        let timestamp = self
            .genesis_time
            .checked_add(
                round
                    .saturating_sub(1)
                    .checked_mul(self.period_secs)
                    .ok_or_else(|| {
                        RandomnessError::Timing("round timestamp overflow".to_owned())
                    })?,
            )
            .ok_or_else(|| RandomnessError::Timing("round timestamp overflow".to_owned()))?;
        if timestamp > target {
            return Err(RandomnessError::Timing(
                "computed round is in the future".to_owned(),
            ));
        }
        if target.saturating_sub(timestamp) > self.max_beacon_age_secs {
            return Err(RandomnessError::Timing(format!(
                "round {round} exceeds configured freshness"
            )));
        }
        Ok(round)
    }

    async fn fetch_endpoint(
        &self,
        endpoint: &DrandEndpoint,
        round: u64,
    ) -> Result<VerifiedDrandBeacon, RandomnessError> {
        revalidate_pinned_dns(endpoint).await?;
        let url = format!("{}/rounds/{round}", endpoint.root.as_str());
        let mut response = endpoint
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|error| RandomnessError::Endpoint(error.to_string()))?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(RandomnessError::Endpoint(format!(
                "{} returned status {}",
                endpoint.host,
                response.status()
            )));
        }
        if response
            .content_length()
            .is_some_and(|length| length > self.max_body_bytes as u64)
        {
            return Err(RandomnessError::Endpoint(format!(
                "{} response exceeds byte limit",
                endpoint.host
            )));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| RandomnessError::Endpoint(error.to_string()))?
        {
            if body.len().saturating_add(chunk.len()) > self.max_body_bytes {
                return Err(RandomnessError::Endpoint(format!(
                    "{} response exceeds byte limit",
                    endpoint.host
                )));
            }
            body.extend_from_slice(&chunk);
        }
        parse_and_verify_drand_response(&body, round, &self.public_key)
    }

    async fn commit_high_water(&self, beacon: &VerifiedDrandBeacon) -> Result<(), RandomnessError> {
        let _commit = self.commit_lock.lock().await;
        {
            let state = self.state.lock();
            if let Some(previous) = state.as_ref() {
                if beacon.round < previous.round {
                    return Err(RandomnessError::Rollback {
                        received: beacon.round,
                        high_water: previous.round,
                    });
                }
                if beacon.round == previous.round {
                    if beacon.randomness != previous.randomness
                        || beacon.signature != previous.signature
                    {
                        return Err(RandomnessError::Equivocation {
                            round: beacon.round,
                        });
                    }
                    return Ok(());
                }
            }
        }
        let next = DrandHighWaterStateV1 {
            version: DRAND_STATE_VERSION_V1,
            round: beacon.round,
            randomness: beacon.randomness,
            signature: beacon.signature,
        };
        let state_path = self.state_path.clone();
        let persisted = next.clone();
        tokio::task::spawn_blocking(move || store_secure_state(&state_path, &persisted, "drand"))
            .await
            .map_err(|error| RandomnessError::Persistence(error.to_string()))??;
        let mut state = self.state.lock();
        *state = Some(next);
        Ok(())
    }
}

#[cfg(feature = "app_api")]
#[async_trait]
impl RandomnessProvider for DrandHttpRandomnessProvider {
    async fn randomness_for_epoch(
        &self,
        epoch_id: u64,
        now_secs: u64,
        response_window_secs: u64,
    ) -> Result<PorRandomness, RandomnessError> {
        let round = self.expected_round(epoch_id, now_secs)?;
        let results = futures::future::join_all(
            self.endpoints
                .iter()
                .map(|endpoint| self.fetch_endpoint(endpoint, round)),
        )
        .await;
        let beacon = select_drand_quorum(results.into_iter().flatten(), self.quorum)?;
        self.commit_high_water(&beacon).await?;
        Ok(PorRandomness {
            epoch_id,
            issued_at_unix: now_secs,
            response_window_secs,
            drand_round: beacon.round,
            drand_randomness: beacon.randomness,
            drand_signature: beacon.signature,
        })
    }
}

#[cfg(feature = "app_api")]
fn select_drand_quorum(
    beacons: impl IntoIterator<Item = VerifiedDrandBeacon>,
    quorum: usize,
) -> Result<VerifiedDrandBeacon, RandomnessError> {
    let mut groups = BTreeMap::<VerifiedDrandBeacon, usize>::new();
    for beacon in beacons {
        *groups.entry(beacon).or_default() += 1;
    }
    let agreeing = groups.values().copied().max().unwrap_or(0);
    groups
        .into_iter()
        .find_map(|(beacon, count)| (count >= quorum).then_some(beacon))
        .ok_or(RandomnessError::QuorumUnavailable {
            agreeing,
            required: quorum,
        })
}

#[cfg(feature = "app_api")]
fn validate_drand_endpoint(
    raw_endpoint: &str,
    endpoint: &url::Url,
    expected_path: &str,
) -> Result<String, RandomnessError> {
    if endpoint.as_str().len() > 2_048
        || endpoint.scheme() != "https"
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
        || endpoint.path() != expected_path
        || endpoint.port().is_some_and(|port| port != 443)
    {
        return Err(RandomnessError::Configuration(format!(
            "drand endpoint must be canonical `https://<host>{expected_path}`"
        )));
    }
    let host = endpoint.host_str().ok_or_else(|| {
        RandomnessError::Configuration("drand endpoint host is missing".to_owned())
    })?;
    if host.parse::<IpAddr>().is_ok()
        || host != host.to_ascii_lowercase()
        || host.ends_with('.')
        || host == "localhost"
    {
        return Err(RandomnessError::Configuration(
            "drand endpoint must use a canonical lowercase public DNS name".to_owned(),
        ));
    }
    let canonical = format!("https://{host}{expected_path}");
    if raw_endpoint != canonical || endpoint.as_str() != canonical {
        return Err(RandomnessError::Configuration(format!(
            "drand endpoint must use exact canonical spelling `{canonical}`"
        )));
    }
    Ok(host.to_owned())
}

#[cfg(feature = "app_api")]
fn resolve_public_endpoint(host: &str, port: u16) -> Result<Vec<SocketAddr>, RandomnessError> {
    let mut addresses = (host, port)
        .to_socket_addrs()
        .map_err(|error| RandomnessError::Configuration(format!("DNS for `{host}`: {error}")))?
        .collect::<Vec<_>>();
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty() || addresses.len() > MAX_DRAND_DNS_ADDRESSES {
        return Err(RandomnessError::Configuration(format!(
            "DNS for `{host}` must yield 1..={MAX_DRAND_DNS_ADDRESSES} addresses"
        )));
    }
    if addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(RandomnessError::Configuration(format!(
            "DNS for `{host}` resolved to a non-public address"
        )));
    }
    Ok(addresses)
}

#[cfg(feature = "app_api")]
async fn revalidate_pinned_dns(endpoint: &DrandEndpoint) -> Result<(), RandomnessError> {
    let mut current = tokio::net::lookup_host((endpoint.host.as_str(), endpoint.port))
        .await
        .map_err(|error| RandomnessError::Endpoint(format!("DNS revalidation: {error}")))?
        .collect::<Vec<_>>();
    current.sort_unstable();
    current.dedup();
    if current.is_empty()
        || current.len() > MAX_DRAND_DNS_ADDRESSES
        || current.iter().any(|address| !is_public_ip(address.ip()))
        || current != endpoint.pinned_addrs
    {
        return Err(RandomnessError::Endpoint(format!(
            "DNS rebinding or address-set change detected for `{}`",
            endpoint.host
        )));
    }
    Ok(())
}

#[cfg(feature = "app_api")]
fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

#[cfg(feature = "app_api")]
fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || octets[0] == 0
        || octets[0] >= 240
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
        || (octets[0] == 198 && (18..=19).contains(&octets[1])))
}

#[cfg(feature = "app_api")]
fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let documentation = segments[0] == 0x2001 && segments[1] == 0x0db8;
    let documentation_v2 = segments[0] == 0x3fff && (segments[1] & 0xf000) == 0;
    let orchid = segments[0] == 0x2001 && (segments[1] & 0xfff0) == 0x0010;
    let transition = (segments[0] == 0x2001 && segments[1] == 0)
        || segments[0] == 0x2002
        || ip.to_ipv4_mapped().is_some();
    !((segments[0] & 0xe000) != 0x2000
        || ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || documentation
        || documentation_v2
        || orchid
        || transition)
}

#[cfg(feature = "app_api")]
fn parse_and_verify_drand_response(
    body: &[u8],
    expected_round: u64,
    public_key: &[u8; iroha_crypto::drand::DRAND_PUBLIC_KEY_BYTES],
) -> Result<VerifiedDrandBeacon, RandomnessError> {
    let value: JsonValue = json::from_slice(body)
        .map_err(|error| RandomnessError::Endpoint(format!("invalid drand JSON: {error}")))?;
    let object = value.as_object().ok_or_else(|| {
        RandomnessError::Endpoint("drand response must be a JSON object".to_owned())
    })?;
    if object.len() != 2 || !object.contains_key("round") || !object.contains_key("signature") {
        return Err(RandomnessError::Endpoint(
            "drand v2 response must contain exactly round and signature".to_owned(),
        ));
    }
    let round = object
        .get("round")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| RandomnessError::Endpoint("drand round must be a u64".to_owned()))?;
    if round != expected_round {
        return Err(RandomnessError::Endpoint(format!(
            "drand returned round {round}; expected {expected_round}"
        )));
    }
    fn decode_canonical_hex<const N: usize>(
        value: Option<&JsonValue>,
        field: &str,
    ) -> Result<[u8; N], RandomnessError> {
        let text = value.and_then(JsonValue::as_str).ok_or_else(|| {
            RandomnessError::Endpoint(format!("drand {field} must be a hex string"))
        })?;
        if text.len() != N * 2
            || !text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(RandomnessError::Endpoint(format!(
                "drand {field} must be canonical lowercase {N}-byte hex"
            )));
        }
        let bytes = hex::decode(text)
            .map_err(|error| RandomnessError::Endpoint(format!("invalid {field}: {error}")))?;
        bytes
            .try_into()
            .map_err(|_| RandomnessError::Endpoint(format!("drand {field} has invalid length")))
    }
    let signature = decode_canonical_hex(object.get("signature"), "signature")?;
    let randomness =
        iroha_crypto::drand::verify_unchained_g1_rfc9380(public_key, round, &signature, None)
            .map_err(|error| RandomnessError::Endpoint(error.to_string()))?;
    Ok(VerifiedDrandBeacon {
        round,
        randomness,
        signature,
    })
}

#[cfg(feature = "app_api")]
fn load_drand_state(
    path: &Path,
    public_key: &[u8; iroha_crypto::drand::DRAND_PUBLIC_KEY_BYTES],
) -> Result<Option<DrandHighWaterStateV1>, RandomnessError> {
    let Some(bytes) = read_secure_state(path, 4 * 1024, "drand")? else {
        return Ok(None);
    };
    let state: DrandHighWaterStateV1 = decode_from_bytes(&bytes)
        .map_err(|error| RandomnessError::Persistence(error.to_string()))?;
    let canonical =
        to_bytes(&state).map_err(|error| RandomnessError::Persistence(error.to_string()))?;
    if canonical != bytes || state.version != DRAND_STATE_VERSION_V1 || state.round == 0 {
        return Err(RandomnessError::Persistence(
            "drand state is non-canonical or has an unsupported version".to_owned(),
        ));
    }
    iroha_crypto::drand::verify_unchained_g1_rfc9380(
        public_key,
        state.round,
        &state.signature,
        Some(&state.randomness),
    )
    .map_err(|error| RandomnessError::Persistence(error.to_string()))?;
    Ok(Some(state))
}

#[cfg(feature = "app_api")]
fn read_secure_state(
    path: &Path,
    max_bytes: usize,
    label: &str,
) -> Result<Option<Vec<u8>>, RandomnessError> {
    secure_read_bytes(path, max_bytes)
        .map_err(|error| RandomnessError::Persistence(format!("{label} state: {error}")))
}

#[cfg(feature = "app_api")]
fn store_secure_state<T: norito::core::NoritoSerialize>(
    path: &Path,
    value: &T,
    label: &str,
) -> Result<(), RandomnessError> {
    let bytes = to_bytes(value).map_err(|error| RandomnessError::Persistence(error.to_string()))?;
    secure_atomic_write(path, &bytes, 64 * 1024 * 1024, true)
        .map_err(|error| RandomnessError::Persistence(format!("{label} state: {error}")))
}

#[cfg(feature = "app_api")]
/// Errors collecting VRF materials required for PoR challenge planning.
#[derive(Debug, Error)]
pub enum VrfError {
    /// Submission fields are structurally invalid.
    #[error("invalid provider VRF submission: {0}")]
    InvalidSubmission(#[from] ProviderVrfSubmissionValidationError),
    /// Provider is not in the current council-approved admission registry.
    #[error("provider is not admitted for PoR VRF submissions")]
    UnadmittedProvider,
    /// Ed25519 submission authentication failed or used a stale advert key.
    #[error("provider VRF submission signature is invalid: {0}")]
    InvalidSignature(String),
    /// Target is not an active local manifest for the submitted provider.
    #[error("provider VRF submission targets an unknown, unpinned, or expired manifest")]
    UnknownManifest,
    /// BLS variant/key/proof/output or canonical input binding failed.
    #[error("provider VRF proof verification failed")]
    InvalidProof,
    /// Submission timestamp exceeds the configured skew window.
    #[error("provider VRF submission timestamp is outside the accepted clock window")]
    InvalidTimestamp,
    /// Submission epoch is too old or too far in the future.
    #[error("provider VRF submission epoch is outside the retained window")]
    InvalidEpoch,
    /// Provider sequence did not advance durable replay high-water state.
    #[error("provider VRF sequence replay: received {received}, high-water {high_water}")]
    Replay {
        /// Sequence number carried by the rejected submission.
        received: u64,
        /// Highest sequence number already persisted for the provider.
        high_water: u64,
    },
    /// The exact manifest/epoch/round submission was already accepted.
    #[error("provider VRF submission is a duplicate")]
    Duplicate,
    /// Conflicting evidence was submitted for the same manifest/epoch.
    #[error("provider VRF equivocation detected")]
    Equivocation,
    /// Durable entry limit is exhausted after safe pruning.
    #[error("provider VRF state entry limit {limit} reached")]
    Limit {
        /// Maximum number of durable entries allowed for the provider state.
        limit: usize,
    },
    /// Durable state failed closed.
    #[error("provider VRF state persistence failure: {0}")]
    Persistence(String),
}

#[cfg(feature = "app_api")]
/// Supplies VRF bundles required to plan PoR challenges.
pub trait VrfProvider: Send + Sync {
    /// Return verified VRF bundles matching the exact drand randomness record.
    ///
    /// # Errors
    ///
    /// Returns [`VrfError`] when durable state cannot be safely queried.
    fn vrf_bundles_for_epoch(
        &self,
        randomness: &PorRandomness,
    ) -> Result<HashMap<ManifestVrfKey, ManifestVrfBundle>, VrfError>;
}

#[cfg(feature = "app_api")]
const VRF_STATE_VERSION_V1: u8 = 1;

#[cfg(feature = "app_api")]
#[derive(
    Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, PartialOrd, Ord,
)]
struct VrfStateKeyV1 {
    epoch_id: u64,
    provider_id: [u8; 32],
    manifest_digest: [u8; 32],
}

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct VrfStateEntryV1 {
    key: VrfStateKeyV1,
    submission: ProviderVrfSubmissionV1,
}

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct VrfProviderSequenceV1 {
    provider_id: [u8; 32],
    high_water: u64,
}

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct VrfStateSnapshotV1 {
    version: u8,
    entries: Vec<VrfStateEntryV1>,
    sequences: Vec<VrfProviderSequenceV1>,
}

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, Default)]
struct VrfState {
    entries: BTreeMap<VrfStateKeyV1, ProviderVrfSubmissionV1>,
    sequences: BTreeMap<[u8; 32], u64>,
}

#[cfg(feature = "app_api")]
/// Admission-bound, authenticated, bounded, durable provider VRF store.
#[derive(Debug)]
pub struct VerifiedVrfProvider {
    admission: Arc<super::AdmissionRegistry>,
    chain_id: Vec<u8>,
    state_path: PathBuf,
    max_entries: usize,
    retention_epochs: u64,
    max_clock_skew_secs: u64,
    state: Mutex<VrfState>,
}

#[cfg(feature = "app_api")]
impl VerifiedVrfProvider {
    /// Load and fully reverify durable provider VRF state.
    pub fn with_persistence(
        admission: Arc<super::AdmissionRegistry>,
        chain_id: Vec<u8>,
        state_path: PathBuf,
        max_entries: usize,
        retention_epochs: u64,
        max_clock_skew_secs: u64,
    ) -> Result<Self, VrfError> {
        if admission.is_empty() || chain_id.is_empty() || chain_id.len() > 255 {
            return Err(VrfError::Persistence(
                "admission registry and bounded chain id are required".to_owned(),
            ));
        }
        if max_entries == 0 || max_entries > 65_536 || retention_epochs == 0 {
            return Err(VrfError::Persistence(
                "VRF bounds must be non-zero and max_entries <= 65536".to_owned(),
            ));
        }
        let state = load_vrf_state(&state_path, max_entries, &admission, &chain_id)?;
        Ok(Self {
            admission,
            chain_id,
            state_path,
            max_entries,
            retention_epochs,
            max_clock_skew_secs,
            state: Mutex::new(state),
        })
    }

    fn verify_submission(
        &self,
        submission: &ProviderVrfSubmissionV1,
        now_secs: u64,
        current_epoch: u64,
    ) -> Result<(), VrfError> {
        submission.validate()?;
        if submission.issued_at > now_secs.saturating_add(self.max_clock_skew_secs)
            || now_secs.saturating_sub(submission.issued_at) > self.max_clock_skew_secs
        {
            return Err(VrfError::InvalidTimestamp);
        }
        let oldest = current_epoch.saturating_sub(self.retention_epochs);
        if submission.epoch_id < oldest || submission.epoch_id > current_epoch.saturating_add(1) {
            return Err(VrfError::InvalidEpoch);
        }
        let record = self
            .admission
            .entry(&submission.provider_id)
            .ok_or(VrfError::UnadmittedProvider)?;
        submission
            .verify_signature_for_provider(record.advert_key())
            .map_err(|error| VrfError::InvalidSignature(error.to_string()))?;
        verify_provider_vrf(submission, record.por_vrf_key(), &self.chain_id)
    }

    fn accept_verified(
        &self,
        submission: ProviderVrfSubmissionV1,
        current_epoch: u64,
    ) -> Result<(), VrfError> {
        let oldest = current_epoch.saturating_sub(self.retention_epochs);

        let key = VrfStateKeyV1 {
            epoch_id: submission.epoch_id,
            provider_id: submission.provider_id,
            manifest_digest: submission.manifest_digest,
        };
        let mut state = self.state.lock();
        if let Some(existing) = state.entries.get(&key) {
            if existing.drand_round == submission.drand_round
                && existing.output == submission.output
                && existing.proof == submission.proof
            {
                return Err(VrfError::Duplicate);
            }
            return Err(VrfError::Equivocation);
        }
        let high_water = state
            .sequences
            .get(&submission.provider_id)
            .copied()
            .unwrap_or(0);
        if submission.sequence <= high_water {
            return Err(VrfError::Replay {
                received: submission.sequence,
                high_water,
            });
        }
        let retained_entries = state
            .entries
            .keys()
            .filter(|key| key.epoch_id >= oldest)
            .count();
        if retained_entries >= self.max_entries {
            return Err(VrfError::Limit {
                limit: self.max_entries,
            });
        }
        let previous = state.clone();
        state.entries.retain(|key, _| key.epoch_id >= oldest);
        state
            .sequences
            .insert(submission.provider_id, submission.sequence);
        state.entries.insert(key, submission);
        if let Err(error) = persist_vrf_state(&self.state_path, &state) {
            *state = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Authenticate, verify, replay-check, and durably accept one provider VRF.
    pub fn submit(
        &self,
        submission: ProviderVrfSubmissionV1,
        now_secs: u64,
        current_epoch: u64,
        target_is_active: bool,
    ) -> Result<(), VrfError> {
        self.verify_submission(&submission, now_secs, current_epoch)?;
        if !target_is_active {
            return Err(VrfError::UnknownManifest);
        }
        self.accept_verified(submission, current_epoch)
    }
}

#[cfg(feature = "app_api")]
impl VrfProvider for VerifiedVrfProvider {
    fn vrf_bundles_for_epoch(
        &self,
        randomness: &PorRandomness,
    ) -> Result<HashMap<ManifestVrfKey, ManifestVrfBundle>, VrfError> {
        let state = self.state.lock();
        let mut bundles = HashMap::new();
        for (key, submission) in state.entries.range(
            VrfStateKeyV1 {
                epoch_id: randomness.epoch_id,
                provider_id: [0; 32],
                manifest_digest: [0; 32],
            }..=VrfStateKeyV1 {
                epoch_id: randomness.epoch_id,
                provider_id: [u8::MAX; 32],
                manifest_digest: [u8::MAX; 32],
            },
        ) {
            if submission.drand_round != randomness.drand_round
                || self.admission.entry(&key.provider_id).is_none()
            {
                continue;
            }
            let lookup = ManifestVrfKey {
                provider_id: key.provider_id,
                manifest_digest: key.manifest_digest,
            };
            bundles.insert(
                lookup,
                ManifestVrfBundle {
                    provider_id: key.provider_id,
                    manifest_digest: key.manifest_digest,
                    epoch_id: key.epoch_id,
                    drand_round: submission.drand_round,
                    output: submission.output,
                    proof: submission.proof,
                },
            );
        }
        Ok(bundles)
    }
}

#[cfg(feature = "app_api")]
fn verify_provider_vrf(
    submission: &ProviderVrfSubmissionV1,
    key: &sorafs_manifest::ProviderVrfPublicKeyV1,
    chain_id: &[u8],
) -> Result<(), VrfError> {
    let input = provider_vrf_input(
        &submission.provider_id,
        &submission.manifest_digest,
        submission.epoch_id,
        submission.drand_round,
    );
    let output = match key {
        sorafs_manifest::ProviderVrfPublicKeyV1::BlsNormal(public_key) => {
            iroha_crypto::vrf::verify_normal_bytes_with_chain(
                public_key,
                chain_id,
                &input,
                &submission.proof,
            )
        }
        sorafs_manifest::ProviderVrfPublicKeyV1::BlsSmall(public_key) => {
            iroha_crypto::vrf::verify_small_bytes_with_chain(
                public_key,
                chain_id,
                &input,
                &submission.proof,
            )
        }
    };
    if output.map(|output| output.0) != Some(submission.output) {
        return Err(VrfError::InvalidProof);
    }
    Ok(())
}

#[cfg(feature = "app_api")]
fn persist_vrf_state(path: &Path, state: &VrfState) -> Result<(), VrfError> {
    let snapshot = VrfStateSnapshotV1 {
        version: VRF_STATE_VERSION_V1,
        entries: state
            .entries
            .iter()
            .map(|(key, submission)| VrfStateEntryV1 {
                key: *key,
                submission: submission.clone(),
            })
            .collect(),
        sequences: state
            .sequences
            .iter()
            .map(|(provider_id, high_water)| VrfProviderSequenceV1 {
                provider_id: *provider_id,
                high_water: *high_water,
            })
            .collect(),
    };
    store_secure_state(path, &snapshot, "provider VRF")
        .map_err(|error| VrfError::Persistence(error.to_string()))
}

#[cfg(feature = "app_api")]
fn load_vrf_state(
    path: &Path,
    max_entries: usize,
    admission: &super::AdmissionRegistry,
    chain_id: &[u8],
) -> Result<VrfState, VrfError> {
    let max_bytes = max_entries
        .checked_mul(768)
        .and_then(|bytes| bytes.checked_add(64 * 1024))
        .ok_or_else(|| VrfError::Persistence("VRF state byte limit overflow".to_owned()))?;
    let Some(bytes) = read_secure_state(path, max_bytes, "provider VRF")
        .map_err(|error| VrfError::Persistence(error.to_string()))?
    else {
        return Ok(VrfState::default());
    };
    let snapshot: VrfStateSnapshotV1 =
        decode_from_bytes(&bytes).map_err(|error| VrfError::Persistence(error.to_string()))?;
    let canonical =
        to_bytes(&snapshot).map_err(|error| VrfError::Persistence(error.to_string()))?;
    if canonical != bytes
        || snapshot.version != VRF_STATE_VERSION_V1
        || snapshot.entries.len() > max_entries
    {
        return Err(VrfError::Persistence(
            "VRF state is non-canonical, unsupported, or over limit".to_owned(),
        ));
    }
    let mut state = VrfState::default();
    let mut previous_key = None;
    for entry in snapshot.entries {
        if previous_key.is_some_and(|previous| previous >= entry.key)
            || entry.key.epoch_id != entry.submission.epoch_id
            || entry.key.provider_id != entry.submission.provider_id
            || entry.key.manifest_digest != entry.submission.manifest_digest
        {
            return Err(VrfError::Persistence(
                "VRF state entries are duplicate, unordered, or misbound".to_owned(),
            ));
        }
        previous_key = Some(entry.key);
        entry.submission.validate()?;
        let Some(record) = admission.entry(&entry.key.provider_id) else {
            // Revocation deliberately removes the provider's verification keys
            // from the active registry. Drop its expired trust-bound payloads,
            // but retain the separately persisted sequence high-water below so
            // re-admission cannot resurrect an old signed submission.
            iroha_logger::warn!(
                provider_id = %hex::encode(entry.key.provider_id),
                epoch_id = entry.key.epoch_id,
                "dropping persisted PoR VRF entry for a no-longer-admitted provider"
            );
            continue;
        };
        entry
            .submission
            .verify_signature_for_provider(record.advert_key())
            .map_err(|error| VrfError::InvalidSignature(error.to_string()))?;
        verify_provider_vrf(&entry.submission, record.por_vrf_key(), chain_id)?;
        state.entries.insert(entry.key, entry.submission);
    }
    let mut previous_provider = None;
    for sequence in snapshot.sequences {
        if sequence.provider_id.iter().all(|byte| *byte == 0)
            || sequence.high_water == 0
            || previous_provider.is_some_and(|previous| previous >= sequence.provider_id)
        {
            return Err(VrfError::Persistence(
                "VRF replay high-water entries are invalid or unordered".to_owned(),
            ));
        }
        let observed = state
            .entries
            .values()
            .filter(|submission| submission.provider_id == sequence.provider_id)
            .map(|submission| submission.sequence)
            .max()
            .unwrap_or(0);
        if observed > sequence.high_water {
            return Err(VrfError::Persistence(
                "VRF replay high-water regresses below an accepted entry".to_owned(),
            ));
        }
        previous_provider = Some(sequence.provider_id);
        state
            .sequences
            .insert(sequence.provider_id, sequence.high_water);
    }
    if state
        .entries
        .values()
        .any(|submission| !state.sequences.contains_key(&submission.provider_id))
    {
        return Err(VrfError::Persistence(
            "VRF state is missing provider replay high-water".to_owned(),
        ));
    }
    Ok(state)
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
    /// Hardened atomic persistence failed.
    #[error("secure persistence error: {0}")]
    Persistence(String),
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
    const MAX_ARTEFACT_BYTES: usize = 16 * 1024 * 1024;

    /// Create a publisher and validate/create its private non-symlink root.
    pub fn try_new(root: PathBuf) -> Result<Self, GovernancePublishError> {
        let (_, root, _) = ensure_secure_parent(&root.join(".por-publisher-probe"))
            .map_err(|error| GovernancePublishError::Persistence(error.to_string()))?;
        Ok(Self { root })
    }

    fn write_json(
        &self,
        path: PathBuf,
        value: JsonValue,
        replace_existing: bool,
    ) -> Result<(), GovernancePublishError> {
        let body = json::to_json_pretty(&value)
            .map_err(|err| GovernancePublishError::Serialisation(err.to_string()))?;
        secure_atomic_write(
            &path,
            body.as_bytes(),
            Self::MAX_ARTEFACT_BYTES,
            replace_existing,
        )
        .map_err(|error| GovernancePublishError::Persistence(error.to_string()))
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
        self.write_json(epoch_dir.join(filename), JsonValue::Object(payload), false)
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
            true,
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
    /// Submission-capable verified provider used by the Torii ingest route.
    verified_vrf_provider: Option<Arc<VerifiedVrfProvider>>,
    /// Publisher invoked to emit governance-facing telemetry (reports, exports).
    publisher: Arc<dyn GovernancePublisher>,
    /// Torii telemetry handle used for scheduler metrics.
    telemetry: crate::routing::MaybeTelemetry,
    /// Interval between PoR epochs in seconds.
    epoch_interval_secs: u64,
    /// Response window duration granted to providers (seconds).
    response_window_secs: u64,
    /// Epoch-relative deadline before the forced challenge path is permitted.
    vrf_submission_deadline_secs: u64,
    /// Last epoch for which automation was executed successfully.
    last_epoch: AtomicU64,
    /// Marker tracking when weekly reports were last generated.
    last_report_marker: AtomicU64,
    /// Serialises scheduler invocations and their durable side effects.
    run_lock: tokio::sync::Mutex<()>,
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
        vrf_submission_deadline_secs: u64,
    ) -> Self {
        Self {
            storage,
            coordinator,
            randomness,
            vrf_provider,
            verified_vrf_provider: None,
            publisher,
            telemetry: crate::routing::MaybeTelemetry::disabled(),
            epoch_interval_secs: epoch_interval_secs.max(60),
            response_window_secs: response_window_secs.max(60),
            vrf_submission_deadline_secs,
            last_epoch: AtomicU64::new(u64::MAX),
            last_report_marker: AtomicU64::new(0),
            run_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Attach the authenticated provider VRF ingest store used by this runtime.
    #[must_use]
    pub fn with_verified_vrf_provider(mut self, provider: Arc<VerifiedVrfProvider>) -> Self {
        self.verified_vrf_provider = Some(provider);
        self
    }

    /// Attach Torii telemetry to the runtime.
    #[must_use]
    pub fn with_telemetry(mut self, telemetry: crate::routing::MaybeTelemetry) -> Self {
        self.telemetry = telemetry;
        self
    }

    fn record_challenge_metric(&self, challenge: &PorChallengeV1, duplicate_samples: usize) {
        self.telemetry.with_metrics(|tel| {
            tel.record_sorafs_por_scheduler_challenge(challenge.forced, duplicate_samples);
        });
    }

    fn record_scheduler_failure(&self) {
        self.telemetry.with_metrics(|tel| {
            tel.record_sorafs_por_scheduler_failure();
        });
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
    pub async fn run_once_at(&self, now_secs: u64) -> Result<bool, PorAutomationError> {
        let _run = self.run_lock.lock().await;
        let epoch = self.compute_epoch(now_secs);
        if self.last_epoch.load(AtomicOrdering::SeqCst) == epoch {
            self.publish_weekly_report_if_needed(now_secs)?;
            return Ok(false);
        }

        let epoch_start = epoch
            .checked_mul(self.epoch_interval_secs)
            .ok_or(PorAutomationError::TimestampOverflow)?;
        let forced_deadline = epoch_start
            .checked_add(self.vrf_submission_deadline_secs)
            .ok_or(PorAutomationError::TimestampOverflow)?;
        if now_secs < forced_deadline {
            self.publish_weekly_report_if_needed(now_secs)?;
            return Ok(false);
        }

        let mut randomness = self
            .randomness
            .randomness_for_epoch(epoch, now_secs, self.response_window_secs)
            .await?;
        randomness.issued_at_unix = forced_deadline;
        let vrf_map = self.vrf_provider.vrf_bundles_for_epoch(&randomness)?;
        let planned = self.storage.plan_challenges(randomness, &vrf_map, true)?;

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
            match self.coordinator.record_challenge(&challenge) {
                Ok(()) | Err(PorCoordinatorError::DuplicateChallenge { .. }) => {}
                Err(error) => return Err(PorAutomationError::Coordinator(error)),
            }
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
            self.record_challenge_metric(&challenge, duplicate_samples);
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
    pub async fn run_once(&self) -> Result<bool, PorAutomationError> {
        self.run_once_at(unix_now()).await
    }

    /// Accept one authenticated provider VRF for an active local manifest.
    pub fn submit_provider_vrf(
        &self,
        submission: ProviderVrfSubmissionV1,
        now_secs: u64,
    ) -> Result<(), VrfError> {
        let provider = self
            .verified_vrf_provider
            .as_ref()
            .ok_or_else(|| VrfError::Persistence("VRF submission store is disabled".to_owned()))?;
        let current_epoch = self.compute_epoch(now_secs);
        provider.verify_submission(&submission, now_secs, current_epoch)?;
        let target_is_active = self.storage.vrf_target_is_active(
            submission.provider_id,
            submission.manifest_digest,
            now_secs,
        );
        if !target_is_active {
            return Err(VrfError::UnknownManifest);
        }
        provider.accept_verified(submission, current_epoch)
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
                        if let Err(err) = self.run_once().await {
                            self.record_scheduler_failure();
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
        vrf_records: &HashMap<ManifestVrfKey, ManifestVrfBundle>,
        allow_forced: bool,
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

    /// Return whether a provider currently owns the active local manifest target.
    fn vrf_target_is_active(
        &self,
        provider_id: [u8; 32],
        manifest_digest: [u8; 32],
        now_secs: u64,
    ) -> bool;
}

#[cfg(feature = "app_api")]
impl PorStorage for sorafs_node::NodeHandle {
    fn plan_challenges(
        &self,
        randomness: PorRandomness,
        vrf_records: &HashMap<ManifestVrfKey, ManifestVrfBundle>,
        allow_forced: bool,
    ) -> Result<Vec<PlannedChallenge>, PorChallengePlannerError> {
        self.plan_por_challenges_with_forced_policy(randomness, vrf_records, allow_forced)
    }

    fn record_challenge(
        &self,
        challenge: &PorChallengeV1,
    ) -> Result<(), sorafs_node::PorTrackerError> {
        self.record_por_challenge(challenge)
    }

    fn vrf_target_is_active(
        &self,
        provider_id: [u8; 32],
        manifest_digest: [u8; 32],
        now_secs: u64,
    ) -> bool {
        if self.capacity_usage().provider_id != Some(provider_id) {
            return false;
        }
        let Some(storage) = self.storage() else {
            return false;
        };
        let grace = self.gc_config().retention_grace_secs();
        storage.manifests().into_iter().any(|manifest| {
            if manifest.manifest_digest() != &manifest_digest {
                return false;
            }
            let retention = manifest.retention_epoch();
            retention == 0 || now_secs < retention.saturating_add(grace)
        })
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
    /// Durable coordinator retention is full.
    #[error("PoR coordinator retention exhausted (limit {limit})")]
    RetentionExhausted {
        /// Configured hard entry limit.
        limit: usize,
    },
    /// Proof payload failed validation.
    #[error("proof payload invalid: {0}")]
    InvalidProof(#[source] sorafs_manifest::por::PorProofValidationError),
    /// Proof signature is invalid or not bound to provider admission.
    #[error("proof signature invalid or unauthorised: {0}")]
    InvalidProofSignature(#[source] sorafs_manifest::por::PorSignatureVerificationError),
    /// Verdict payload failed validation.
    #[error("verdict payload invalid: {0}")]
    InvalidVerdict(#[source] sorafs_manifest::por::AuditVerdictValidationError),
    /// Verdict signatures do not satisfy the configured auditor policy.
    #[error("verdict signatures invalid or unauthorised: {0}")]
    InvalidVerdictSignature(#[source] sorafs_manifest::por::PorSignatureVerificationError),
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
    /// Exact challenge payload was replayed.
    #[error("challenge with id {challenge_id_hex} was already recorded")]
    DuplicateChallenge {
        /// Replayed challenge identifier.
        challenge_id: [u8; 32],
        /// Hexadecimal representation of the replayed identifier.
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
    /// Verdict already finalised the challenge.
    #[error("verdict already recorded for challenge {challenge_id_hex}")]
    DuplicateVerdict {
        /// Challenge identifier receiving a duplicate verdict.
        challenge_id: [u8; 32],
        /// Hex representation of the challenge identifier.
        challenge_id_hex: String,
    },
    /// A compensating rollback encountered a later or different transition.
    #[error("cannot roll back challenge {challenge_id_hex}; lifecycle state changed")]
    RollbackConflict {
        /// Challenge identifier whose state could not be rolled back.
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
    /// Proof sample indices differ from the governed challenge selection.
    #[error("proof sample indices do not match challenge {challenge_id_hex}")]
    SampleIndicesMismatch {
        /// Challenge identifier whose sample coverage was violated.
        challenge_id: [u8; 32],
        /// Hex representation of the challenge identifier.
        challenge_id_hex: String,
    },
    /// Provider timestamp falls outside the challenge response window.
    #[error(
        "proof submitted_at {submitted_at} is outside challenge window {issued_at}..={deadline_at}"
    )]
    ProofOutsideChallengeWindow {
        /// Provider-supplied proof timestamp.
        submitted_at: u64,
        /// Challenge issue timestamp.
        issued_at: u64,
        /// Inclusive challenge deadline.
        deadline_at: u64,
    },
    /// Verdict proof digest differs from the recorded proof.
    #[error("verdict proof digest mismatch (expected {expected_hex}, got {actual_hex})")]
    ProofDigestMismatch {
        /// Recorded proof digest.
        expected: [u8; 32],
        /// Verdict-supplied proof digest.
        actual: [u8; 32],
        /// Recorded digest in hexadecimal.
        expected_hex: String,
        /// Supplied digest in hexadecimal.
        actual_hex: String,
    },
    /// A proof exists, so the verdict must bind its digest.
    #[error("verdict must include the recorded proof digest")]
    MissingVerdictProofDigest,
    /// Verdict claims a proof digest when no proof was recorded.
    #[error("verdict includes a proof digest but no proof was recorded")]
    UnexpectedVerdictProofDigest,
    /// Successful or repaired verdicts cannot be issued without a proof.
    #[error("successful or repaired verdict requires a recorded proof")]
    MissingProofForSuccessfulVerdict,
    /// Verdict timestamp predates the challenge.
    #[error("verdict decided_at {decided_at} predates challenge issued_at {issued_at}")]
    VerdictBeforeChallenge {
        /// Verdict decision timestamp.
        decided_at: u64,
        /// Challenge issue timestamp.
        issued_at: u64,
    },
    /// Verdict timestamp predates the proof.
    #[error("verdict decided_at {decided_at} predates proof submitted_at {submitted_at}")]
    VerdictBeforeProof {
        /// Verdict decision timestamp.
        decided_at: u64,
        /// Proof submission timestamp.
        submitted_at: u64,
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
    use std::sync::{Arc as StdArc, Barrier};

    use ed25519_dalek::{Signer as _, SigningKey};
    use sorafs_manifest::{
        por::{
            POR_CHALLENGE_VERSION_V1, POR_PROOF_VERSION_V1, POR_VRF_SUBMISSION_VERSION_V1,
            derive_challenge_id, derive_challenge_seed,
        },
        provider_advert::{AdvertSignature, SignatureAlgorithm},
    };
    use tempfile::tempdir;

    use super::*;

    fn provider_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[0xAB; 32])
    }

    fn auditor_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[0xAC; 32])
    }

    fn provider_key() -> Vec<u8> {
        provider_signing_key().verifying_key().to_bytes().to_vec()
    }

    fn auditor_keys() -> Vec<Vec<u8>> {
        vec![auditor_signing_key().verifying_key().to_bytes().to_vec()]
    }

    fn resign_proof(proof: &mut sorafs_manifest::por::PorProofV1) {
        let key = provider_signing_key();
        proof.signature.public_key = key.verifying_key().to_bytes().to_vec();
        let payload = proof
            .signature_payload_bytes()
            .expect("encode proof signing payload");
        proof.signature.signature = key.sign(&payload).to_bytes().to_vec();
    }

    fn resign_verdict(verdict: &mut AuditVerdictV1) {
        let key = auditor_signing_key();
        let payload = verdict
            .signature_payload_bytes()
            .expect("encode verdict signing payload");
        verdict.auditor_signatures = vec![AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: key.verifying_key().to_bytes().to_vec(),
            signature: key.sign(&payload).to_bytes().to_vec(),
        }];
    }

    #[test]
    fn persistence_path_preserves_suffixes_without_predictable_temp_name() {
        let base = PathBuf::from("/tmp/por_snapshot.norito.json");
        let persistence = PorPersistence::new(base.clone());
        assert_eq!(persistence.path, base);
    }

    #[cfg(unix)]
    #[test]
    fn immutable_secure_publication_is_exactly_idempotent_and_conflict_safe() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempdir().expect("temp dir");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).expect("private root");
        let path = dir.path().join("challenge.json");
        secure_atomic_write(&path, b"canonical-a", 1_024, false).expect("first publication");
        secure_atomic_write(&path, b"canonical-a", 1_024, false).expect("exact replay");
        assert!(matches!(
            secure_atomic_write(&path, b"canonical-b", 1_024, false),
            Err(SecureFileError::Conflict)
        ));
        assert_eq!(fs::read(&path).expect("published bytes"), b"canonical-a");
        assert_eq!(
            fs::read_dir(dir.path())
                .expect("list publication root")
                .filter_map(Result::ok)
                .count(),
            1,
            "temporary files must not survive publication or conflict"
        );
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_immutable_publication_has_one_canonical_winner() {
        use std::os::unix::fs::PermissionsExt as _;

        const WORKERS: usize = 16;
        let dir = tempdir().expect("temp dir");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).expect("private root");
        let path = StdArc::new(dir.path().join("challenge.json"));
        let barrier = StdArc::new(Barrier::new(WORKERS));
        let results = std::thread::scope(|scope| {
            let mut workers = Vec::with_capacity(WORKERS);
            for index in 0..WORKERS {
                let path = StdArc::clone(&path);
                let barrier = StdArc::clone(&barrier);
                workers.push(scope.spawn(move || {
                    let body = format!("canonical-{index:02}");
                    barrier.wait();
                    (
                        body.clone(),
                        secure_atomic_write(&path, body.as_bytes(), 1_024, false),
                    )
                }));
            }
            workers
                .into_iter()
                .map(|worker| worker.join().expect("publication worker"))
                .collect::<Vec<_>>()
        });
        assert_eq!(
            results.iter().filter(|(_, result)| result.is_ok()).count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|(_, result)| matches!(result, Err(SecureFileError::Conflict)))
                .count(),
            WORKERS - 1
        );
        let winner = results
            .iter()
            .find(|(_, result)| result.is_ok())
            .map(|(body, _)| body.as_bytes())
            .expect("one winner");
        assert_eq!(fs::read(&*path).expect("winner bytes"), winner);
    }

    #[cfg(unix)]
    #[test]
    fn secure_persistence_rejects_parent_traversal_symlinks_and_hardlinks() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let dir = tempdir().expect("temp dir");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).expect("private root");
        assert!(matches!(
            secure_atomic_write(&dir.path().join("nested/../escape"), b"x", 8, true),
            Err(SecureFileError::UnsafePath(_))
        ));

        let real = dir.path().join("real");
        fs::create_dir(&real).expect("real directory");
        fs::set_permissions(&real, fs::Permissions::from_mode(0o700)).expect("private real dir");
        let linked = dir.path().join("linked");
        symlink(&real, &linked).expect("linked ancestor");
        assert!(matches!(
            secure_atomic_write(&linked.join("state.to"), b"x", 8, true),
            Err(SecureFileError::UnsafePath(_))
        ));

        let destination = real.join("state.to");
        secure_atomic_write(&destination, b"state", 8, true).expect("initial state");
        let alias = real.join("state-alias.to");
        fs::hard_link(&destination, &alias).expect("hard link");
        assert!(matches!(
            secure_read_bytes(&destination, 8),
            Err(SecureFileError::UnsafePath(_))
        ));
    }

    #[cfg(feature = "app_api")]
    #[test]
    fn vrf_restart_drops_revoked_provider_entries_but_keeps_replay_high_water() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            let dir = tempdir().expect("temp dir");
            fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700))
                .expect("private state root");
            let path = dir.path().join("vrf-state.to");
            let provider_id = [0x41; 32];
            let manifest_digest = [0x42; 32];
            let submission = ProviderVrfSubmissionV1 {
                version: POR_VRF_SUBMISSION_VERSION_V1,
                provider_id,
                manifest_digest,
                epoch_id: 7,
                drand_round: 9,
                output: [0x43; 32],
                proof: iroha_crypto::vrf::VrfProof::SigInG1([0x44; 48]),
                sequence: 11,
                issued_at: 1_800_000_000,
                signature: AdvertSignature {
                    algorithm: SignatureAlgorithm::Ed25519,
                    public_key: vec![0x45; 32],
                    signature: vec![0x46; 64],
                },
            };
            submission.validate().expect("structural submission");
            let key = VrfStateKeyV1 {
                epoch_id: submission.epoch_id,
                provider_id,
                manifest_digest,
            };
            let mut persisted = VrfState::default();
            persisted.entries.insert(key, submission);
            persisted.sequences.insert(provider_id, 11);
            persist_vrf_state(&path, &persisted).expect("persist admitted-era state");

            let restored = load_vrf_state(
                &path,
                16,
                &crate::sorafs::AdmissionRegistry::empty(),
                b"test-chain",
            )
            .expect("revoked-provider state must not brick restart");
            assert!(restored.entries.is_empty());
            assert_eq!(restored.sequences.get(&provider_id), Some(&11));
        }
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
            drand_signature: [0x55; 48],
            vrf_output,
            vrf_proof: if forced {
                None
            } else {
                Some(iroha_crypto::vrf::VrfProof::SigInG1([0x77; 48]))
            },
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
        let mut proof = sorafs_manifest::por::PorProofV1 {
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
                public_key: Vec::new(),
                signature: Vec::new(),
            },
            submitted_at: 1_700_000_500,
        };
        resign_proof(&mut proof);
        proof
    }

    fn sample_verdict(
        challenge: &PorChallengeV1,
        outcome: AuditOutcomeV1,
        proof_digest: Option<[u8; 32]>,
    ) -> AuditVerdictV1 {
        let mut verdict = AuditVerdictV1 {
            version: sorafs_manifest::por::AUDIT_VERDICT_VERSION_V1,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            challenge_id: challenge.challenge_id,
            proof_digest,
            outcome,
            failure_reason: match outcome {
                AuditOutcomeV1::Success => None,
                AuditOutcomeV1::Failed | AuditOutcomeV1::Repaired => {
                    Some("digest mismatch".to_string())
                }
            },
            decided_at: 1_700_000_600,
            auditor_signatures: Vec::new(),
            metadata: Vec::new(),
        };
        resign_verdict(&mut verdict);
        verdict
    }

    #[test]
    fn records_challenge_proof_and_verdict() {
        let coordinator = PorCoordinator::new();
        let challenge = sample_challenge(false);
        coordinator.record_challenge(&challenge).expect("challenge");
        let proof = sample_proof(&challenge);
        let proof_digest = proof.proof_digest();
        coordinator
            .record_proof(&proof, &provider_key())
            .expect("proof");
        let verdict = sample_verdict(&challenge, AuditOutcomeV1::Success, Some(proof_digest));
        coordinator
            .record_verdict(
                &verdict,
                &auditor_keys(),
                1,
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

    fn verdict_outcome(sample_count: u16, failed: bool) -> PorVerdictOutcome {
        PorVerdictOutcome {
            stats: sorafs_node::PorVerdictStats {
                success_samples: if failed { 0 } else { u64::from(sample_count) },
                failed_samples: if failed { u64::from(sample_count) } else { 0 },
            },
            repair_history_id: failed.then_some(1),
            consecutive_failures: u64::from(failed),
            slash: None,
        }
    }

    #[test]
    fn forged_proofs_and_verdicts_leave_coordinator_state_retryable() {
        let coordinator = PorCoordinator::new();
        let challenge = sample_challenge(false);
        coordinator.record_challenge(&challenge).unwrap();
        let proof = sample_proof(&challenge);

        for mutation in 0..4 {
            let mut forged = proof.clone();
            match mutation {
                0 => forged.provider_id[0] ^= 1,
                1 => forged.manifest_digest[0] ^= 1,
                2 => forged.samples.swap(0, 1),
                3 => forged.submitted_at = challenge.deadline_at + 1,
                _ => unreachable!(),
            }
            resign_proof(&mut forged);
            assert!(coordinator.record_proof(&forged, &provider_key()).is_err());
            let status = coordinator.query_statuses(&PorStatusFilter::default(), None, None);
            assert_eq!(status[0].proof_digest, None);
        }

        coordinator
            .record_proof(&proof, &provider_key())
            .expect("valid proof retry");
        let digest = proof.proof_digest();
        let valid = sample_verdict(&challenge, AuditOutcomeV1::Success, Some(digest));
        for mutation in 0..5 {
            let mut forged = valid.clone();
            match mutation {
                0 => forged.provider_id[0] ^= 1,
                1 => forged.manifest_digest[0] ^= 1,
                2 => forged.proof_digest = Some([0xEE; 32]),
                3 => forged.proof_digest = None,
                4 => forged.decided_at = proof.submitted_at - 1,
                _ => unreachable!(),
            }
            resign_verdict(&mut forged);
            assert!(
                coordinator
                    .record_verdict(
                        &forged,
                        &auditor_keys(),
                        1,
                        verdict_outcome(challenge.sample_count, false),
                    )
                    .is_err()
            );
            let status = coordinator.query_statuses(&PorStatusFilter::default(), None, None);
            assert_eq!(status[0].status, PorChallengeOutcome::Pending);
            assert_eq!(status[0].proof_digest, Some(digest));
        }

        coordinator
            .record_verdict(
                &valid,
                &auditor_keys(),
                1,
                verdict_outcome(challenge.sample_count, false),
            )
            .expect("valid verdict retry");
        assert!(matches!(
            coordinator.record_verdict(
                &valid,
                &auditor_keys(),
                1,
                verdict_outcome(challenge.sample_count, false),
            ),
            Err(PorCoordinatorError::DuplicateVerdict { .. })
        ));
    }

    #[test]
    fn coordinator_enforces_admission_key_and_auditor_policy() {
        let coordinator = PorCoordinator::new();
        let challenge = sample_challenge(false);
        coordinator.record_challenge(&challenge).unwrap();
        let proof = sample_proof(&challenge);

        assert!(matches!(
            coordinator.record_proof(&proof, &[0xEE; 32]),
            Err(PorCoordinatorError::InvalidProofSignature(
                sorafs_manifest::por::PorSignatureVerificationError::ProviderSignerMismatch
            ))
        ));
        coordinator
            .record_proof(&proof, &provider_key())
            .expect("admitted provider proof");

        let verdict = sample_verdict(
            &challenge,
            AuditOutcomeV1::Success,
            Some(proof.proof_digest()),
        );
        assert!(matches!(
            coordinator.record_verdict(
                &verdict,
                &[vec![0xEF; 32]],
                1,
                verdict_outcome(challenge.sample_count, false),
            ),
            Err(PorCoordinatorError::InvalidVerdictSignature(
                sorafs_manifest::por::PorSignatureVerificationError::UntrustedAuditorSigner
            ))
        ));
        let mut threshold_keys = auditor_keys();
        threshold_keys.push(vec![0xF0; 32]);
        assert!(matches!(
            coordinator.record_verdict(
                &verdict,
                &threshold_keys,
                2,
                verdict_outcome(challenge.sample_count, false),
            ),
            Err(PorCoordinatorError::InvalidVerdictSignature(
                sorafs_manifest::por::PorSignatureVerificationError::InsufficientTrustedAuditorSignatures {
                    actual: 1,
                    required: 2,
                }
            ))
        ));
        coordinator
            .record_verdict(
                &verdict,
                &auditor_keys(),
                1,
                verdict_outcome(challenge.sample_count, false),
            )
            .expect("trusted auditor threshold");
    }

    #[test]
    fn coordinator_rejects_replays_and_supports_compensating_rollbacks() {
        let coordinator = PorCoordinator::new();
        let challenge = sample_challenge(false);
        coordinator.record_challenge(&challenge).unwrap();
        assert!(matches!(
            coordinator.record_challenge(&challenge),
            Err(PorCoordinatorError::DuplicateChallenge { .. })
        ));

        let proof = sample_proof(&challenge);
        coordinator.record_proof(&proof, &provider_key()).unwrap();
        assert!(matches!(
            coordinator.record_proof(&proof, &provider_key()),
            Err(PorCoordinatorError::DuplicateProof { .. })
        ));
        coordinator.rollback_proof(&proof).unwrap();
        let status = coordinator.query_statuses(&PorStatusFilter::default(), None, None);
        assert_eq!(status[0].proof_digest, None);

        coordinator.record_proof(&proof, &provider_key()).unwrap();
        let verdict = sample_verdict(
            &challenge,
            AuditOutcomeV1::Success,
            Some(proof.proof_digest()),
        );
        let outcome = verdict_outcome(challenge.sample_count, false);
        coordinator
            .record_verdict(&verdict, &auditor_keys(), 1, outcome.clone())
            .unwrap();
        assert!(matches!(
            coordinator.record_verdict(&verdict, &auditor_keys(), 1, outcome),
            Err(PorCoordinatorError::DuplicateVerdict { .. })
        ));
        coordinator.rollback_verdict(&verdict).unwrap();
        let status = coordinator.query_statuses(&PorStatusFilter::default(), None, None);
        assert_eq!(status[0].status, PorChallengeOutcome::Pending);
        assert_eq!(status[0].proof_digest, Some(proof.proof_digest()));
    }

    #[test]
    fn concurrent_conflicting_proofs_have_exactly_one_winner() {
        const WORKERS: usize = 16;
        let coordinator = StdArc::new(PorCoordinator::new());
        let challenge = sample_challenge(false);
        coordinator.record_challenge(&challenge).unwrap();
        let barrier = StdArc::new(Barrier::new(WORKERS));

        let results = std::thread::scope(|scope| {
            let mut workers = Vec::with_capacity(WORKERS);
            for index in 0..WORKERS {
                let coordinator = StdArc::clone(&coordinator);
                let barrier = StdArc::clone(&barrier);
                let mut proof = sample_proof(&challenge);
                proof.auth_path[0][0] = u8::try_from(index + 1).expect("worker index fits u8");
                resign_proof(&mut proof);
                let provider_key = provider_key();
                workers.push(scope.spawn(move || {
                    barrier.wait();
                    coordinator.record_proof(&proof, &provider_key)
                }));
            }
            workers
                .into_iter()
                .map(|worker| worker.join().expect("proof worker"))
                .collect::<Vec<_>>()
        });

        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(PorCoordinatorError::DuplicateProof { .. })))
                .count(),
            WORKERS - 1
        );
        let statuses = coordinator.query_statuses(&PorStatusFilter::default(), None, None);
        assert!(statuses[0].proof_digest.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn persistence_failures_roll_back_each_coordinator_transition() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempdir().expect("temp dir");
        let blocked_parent = dir.path().join("blocked");
        let snapshot_path = blocked_parent.join("por.to");
        let coordinator = PorCoordinator::with_persistence(&snapshot_path).unwrap();
        fs::set_permissions(&blocked_parent, fs::Permissions::from_mode(0o755)).unwrap();
        let challenge = sample_challenge(true);

        assert!(matches!(
            coordinator.record_challenge(&challenge),
            Err(PorCoordinatorError::Persistence(_))
        ));
        assert!(
            coordinator
                .query_statuses(&PorStatusFilter::default(), None, None)
                .is_empty()
        );

        fs::set_permissions(&blocked_parent, fs::Permissions::from_mode(0o700)).unwrap();
        coordinator
            .record_challenge(&challenge)
            .expect("challenge succeeds after persistence recovery");
        let proof = sample_proof(&challenge);
        fs::set_permissions(&blocked_parent, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            coordinator.record_proof(&proof, &provider_key()),
            Err(PorCoordinatorError::Persistence(_))
        ));
        let status = coordinator.query_statuses(&PorStatusFilter::default(), None, None);
        assert_eq!(status[0].proof_digest, None);

        fs::set_permissions(&blocked_parent, fs::Permissions::from_mode(0o700)).unwrap();
        coordinator
            .record_proof(&proof, &provider_key())
            .expect("proof succeeds after persistence recovery");
        let digest = proof.proof_digest();
        let verdict = sample_verdict(&challenge, AuditOutcomeV1::Success, Some(digest));
        fs::set_permissions(&blocked_parent, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            coordinator.record_verdict(
                &verdict,
                &auditor_keys(),
                1,
                verdict_outcome(challenge.sample_count, false),
            ),
            Err(PorCoordinatorError::Persistence(_))
        ));
        let status = coordinator.query_statuses(&PorStatusFilter::default(), None, None);
        assert_eq!(status[0].status, PorChallengeOutcome::Forced);
        assert_eq!(status[0].proof_digest, Some(digest));

        fs::set_permissions(&blocked_parent, fs::Permissions::from_mode(0o700)).unwrap();
        coordinator
            .record_verdict(
                &verdict,
                &auditor_keys(),
                1,
                verdict_outcome(challenge.sample_count, false),
            )
            .expect("verdict succeeds after persistence recovery");
    }

    #[test]
    fn weekly_report_compiles() {
        let coordinator = PorCoordinator::new();
        let mut challenge = sample_challenge(true);
        challenge.issued_at = 1_700_000_000;
        challenge.deadline_at = challenge.issued_at + 600;
        coordinator.record_challenge(&challenge).expect("challenge");
        let verdict = sample_verdict(&challenge, AuditOutcomeV1::Failed, None);
        coordinator
            .record_verdict(
                &verdict,
                &auditor_keys(),
                1,
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

    fn record_failed_forced_challenge(
        coordinator: &PorCoordinator,
        provider_byte: u8,
        manifest_byte: u8,
        issued_at: u64,
    ) {
        let mut challenge = sample_challenge(true);
        challenge.provider_id = [provider_byte; 32];
        challenge.manifest_digest = [manifest_byte; 32];
        challenge.issued_at = issued_at;
        challenge.deadline_at = issued_at + 600;
        challenge.seed = derive_challenge_seed(
            &challenge.drand_randomness,
            None,
            &challenge.manifest_digest,
            challenge.epoch_id,
        );
        challenge.challenge_id = derive_challenge_id(
            &challenge.seed,
            &challenge.manifest_digest,
            &challenge.provider_id,
            challenge.epoch_id,
            challenge.drand_round,
        );
        coordinator.record_challenge(&challenge).expect("challenge");
        let mut verdict = sample_verdict(&challenge, AuditOutcomeV1::Failed, None);
        verdict.decided_at = issued_at + 500;
        resign_verdict(&mut verdict);
        coordinator
            .record_verdict(
                &verdict,
                &auditor_keys(),
                1,
                PorVerdictOutcome {
                    stats: sorafs_node::PorVerdictStats {
                        success_samples: 0,
                        failed_samples: 64,
                    },
                    repair_history_id: Some(u64::from(provider_byte)),
                    consecutive_failures: 1,
                    slash: None,
                },
            )
            .expect("verdict");
    }

    #[test]
    fn weekly_report_is_byte_stable_across_insertion_orders_and_ties() {
        let entries = [(4_u8, 14_u8), (2, 12), (3, 13), (1, 11)];
        let first = PorCoordinator::new();
        let second = PorCoordinator::new();
        for (index, (provider, manifest)) in entries.iter().copied().enumerate() {
            record_failed_forced_challenge(
                &first,
                provider,
                manifest,
                1_700_000_000 + index as u64,
            );
        }
        for (index, (provider, manifest)) in entries.iter().copied().rev().enumerate() {
            record_failed_forced_challenge(
                &second,
                provider,
                manifest,
                1_700_000_003 - index as u64,
            );
        }

        let cycle = PorReportIsoWeek {
            year: 2023,
            week: 46,
        };
        let generated_at = 1_700_100_000;
        let first_report = first
            .weekly_report_at(cycle, generated_at)
            .expect("first report");
        let second_report = second
            .weekly_report_at(cycle, generated_at)
            .expect("second report");

        assert_eq!(first_report, second_report);
        assert_eq!(
            first_report.providers_missing_vrf,
            vec![[1; 32], [2; 32], [3; 32], [4; 32]]
        );
        assert_eq!(
            first_report
                .top_offenders
                .iter()
                .map(|summary| summary.provider_id)
                .collect::<Vec<_>>(),
            vec![[1; 32], [2; 32], [3; 32], [4; 32]]
        );
        assert_eq!(
            to_bytes(&first_report).expect("encode first report"),
            to_bytes(&second_report).expect("encode second report")
        );
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
            coordinator
                .record_proof(&proof, &provider_key())
                .expect("proof");
            let verdict = sample_verdict(&challenge, AuditOutcomeV1::Repaired, Some(proof_digest));
            coordinator
                .record_verdict(
                    &verdict,
                    &auditor_keys(),
                    1,
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
        use std::{
            collections::HashMap,
            fs,
            path::Path,
            str::FromStr,
            sync::{
                Arc,
                atomic::{AtomicUsize, Ordering as AtomicOrdering},
            },
        };

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

        #[async_trait]
        impl RandomnessProvider for StaticRandomnessProvider {
            async fn randomness_for_epoch(
                &self,
                _epoch_id: u64,
                _now_secs: u64,
                _response_window_secs: u64,
            ) -> Result<PorRandomness, RandomnessError> {
                Ok(self.randomness)
            }
        }

        #[derive(Default, Clone)]
        struct StaticVrfProvider {
            map: HashMap<u64, HashMap<ManifestVrfKey, ManifestVrfBundle>>,
        }

        impl StaticVrfProvider {
            fn with_entry(epoch: u64, manifest: [u8; 32], bundle: ManifestVrfBundle) -> Self {
                let mut map = HashMap::new();
                map.insert(
                    epoch,
                    HashMap::from([(
                        ManifestVrfKey {
                            provider_id: bundle.provider_id,
                            manifest_digest: manifest,
                        },
                        bundle,
                    )]),
                );
                Self { map }
            }
        }

        impl VrfProvider for StaticVrfProvider {
            fn vrf_bundles_for_epoch(
                &self,
                randomness: &PorRandomness,
            ) -> Result<HashMap<ManifestVrfKey, ManifestVrfBundle>, VrfError> {
                Ok(self
                    .map
                    .get(&randomness.epoch_id)
                    .cloned()
                    .unwrap_or_default())
            }
        }

        #[derive(Clone)]
        struct ReplaySafeStorage {
            planned: Vec<PlannedChallenge>,
            recorded: Arc<Mutex<Option<PorChallengeV1>>>,
        }

        impl PorStorage for ReplaySafeStorage {
            fn plan_challenges(
                &self,
                _randomness: PorRandomness,
                _vrf_records: &HashMap<ManifestVrfKey, ManifestVrfBundle>,
                _allow_forced: bool,
            ) -> Result<Vec<PlannedChallenge>, PorChallengePlannerError> {
                Ok(self.planned.clone())
            }

            fn record_challenge(
                &self,
                challenge: &PorChallengeV1,
            ) -> Result<(), sorafs_node::PorTrackerError> {
                let mut recorded = self.recorded.lock();
                match recorded.as_ref() {
                    Some(existing) if existing == challenge => Ok(()),
                    Some(_) => Err(sorafs_node::PorTrackerError::ChallengeConflict),
                    None => {
                        *recorded = Some(challenge.clone());
                        Ok(())
                    }
                }
            }

            fn vrf_target_is_active(
                &self,
                _provider_id: [u8; 32],
                _manifest_digest: [u8; 32],
                _now_secs: u64,
            ) -> bool {
                true
            }
        }

        struct FailOncePublisher {
            attempts: AtomicUsize,
            published: Mutex<Vec<PorChallengeV1>>,
        }

        impl GovernancePublisher for FailOncePublisher {
            fn publish_challenge(
                &self,
                challenge: &PorChallengeV1,
                _duplicate_samples: usize,
            ) -> Result<(), GovernancePublishError> {
                if self.attempts.fetch_add(1, AtomicOrdering::SeqCst) == 0 {
                    return Err(GovernancePublishError::Io(std::io::Error::other(
                        "injected publication failure",
                    )));
                }
                self.published.lock().push(challenge.clone());
                Ok(())
            }

            fn publish_weekly_report(
                &self,
                _report: &PorWeeklyReportV1,
            ) -> Result<(), GovernancePublishError> {
                Ok(())
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

        #[tokio::test]
        async fn runtime_retries_exact_sinks_after_mid_pipeline_failure() {
            let epoch_interval = 3_600;
            let epoch_id = 42;
            let vrf_deadline = 300;
            let now_secs = epoch_id * epoch_interval + vrf_deadline;
            let mut challenge = sample_challenge(true);
            challenge.issued_at = now_secs;
            challenge.deadline_at = now_secs + 900;
            let planned = PlannedChallenge {
                challenge: challenge.clone(),
                duplicate_samples: 0,
            };
            let recorded = Arc::new(Mutex::new(None));
            let storage = Arc::new(ReplaySafeStorage {
                planned: vec![planned],
                recorded: Arc::clone(&recorded),
            });
            let publisher = Arc::new(FailOncePublisher {
                attempts: AtomicUsize::new(0),
                published: Mutex::new(Vec::new()),
            });
            let randomness = PorRandomness {
                epoch_id,
                issued_at_unix: now_secs,
                response_window_secs: 900,
                drand_round: challenge.drand_round,
                drand_randomness: challenge.drand_randomness,
                drand_signature: challenge.drand_signature,
            };
            let coordinator = Arc::new(PorCoordinator::new());
            let runtime = PorCoordinatorRuntime::new(
                storage,
                Arc::clone(&coordinator),
                Arc::new(StaticRandomnessProvider { randomness }),
                Arc::new(StaticVrfProvider::default()),
                publisher.clone(),
                epoch_interval,
                900,
                vrf_deadline,
            );

            assert!(matches!(
                runtime.run_once_at(now_secs).await,
                Err(PorAutomationError::Governance(_))
            ));
            assert_eq!(recorded.lock().as_ref(), Some(&challenge));
            assert_eq!(
                coordinator
                    .query_statuses(&PorStatusFilter::default(), None, None)
                    .len(),
                1
            );

            assert!(
                runtime
                    .run_once_at(now_secs)
                    .await
                    .expect("exact retry succeeds")
            );
            assert_eq!(publisher.published.lock().as_slice(), &[challenge]);
            assert!(!runtime.run_once_at(now_secs).await.expect("epoch complete"));
            assert_eq!(publisher.attempts.load(AtomicOrdering::SeqCst), 2);
        }

        #[tokio::test]
        async fn runtime_emits_governance_challenge_with_vrf() {
            let temp_dir = tempdir().expect("temp dir");
            let governance_dir = temp_dir.path().join("governance");
            let handle = NodeHandle::new(storage_config(temp_dir.path()));
            let provider_id = [0x11; 32];
            declare_capacity(&handle, provider_id);
            let payload = vec![0xAB; 512 * 1024];
            let (manifest_digest, _manifest) = ingest_manifest(&handle, &payload);

            let epoch_interval = 3_600;
            let epoch_id = 500_000;
            let vrf_deadline = 300;
            let now_secs = epoch_id * epoch_interval + vrf_deadline;
            let randomness = PorRandomness {
                epoch_id,
                issued_at_unix: now_secs,
                response_window_secs: 900,
                drand_round: 42_000,
                drand_randomness: [0x21; 32],
                drand_signature: [0xCD; 48],
            };
            let randomness_provider = Arc::new(StaticRandomnessProvider {
                randomness: randomness.clone(),
            });
            let vrf_provider = Arc::new(StaticVrfProvider::with_entry(
                epoch_id,
                manifest_digest,
                ManifestVrfBundle {
                    provider_id,
                    manifest_digest,
                    epoch_id,
                    drand_round: randomness.drand_round,
                    output: [0x55; 32],
                    proof: iroha_crypto::vrf::VrfProof::SigInG1([0x66; 48]),
                },
            ));
            let publisher = Arc::new(
                FilesystemGovernancePublisher::try_new(governance_dir.clone())
                    .expect("governance publisher"),
            );
            let coordinator = Arc::new(PorCoordinator::new());
            let storage: Arc<dyn PorStorage> = Arc::new(handle.clone());
            #[cfg(feature = "telemetry")]
            let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
            #[cfg(feature = "telemetry")]
            let telemetry = crate::routing::MaybeTelemetry::from_profile(
                Some(iroha_core::telemetry::Telemetry::new(metrics.clone(), true)),
                iroha_config::parameters::actual::TelemetryProfile::Full,
            );

            let runtime = PorCoordinatorRuntime::new(
                storage,
                coordinator.clone(),
                randomness_provider,
                vrf_provider,
                publisher,
                epoch_interval,
                randomness.response_window_secs,
                vrf_deadline,
            );
            #[cfg(feature = "telemetry")]
            let runtime = runtime.with_telemetry(telemetry);
            let runtime = Arc::new(runtime);

            let triggered = runtime.run_once_at(now_secs).await.expect("runtime tick");
            assert!(triggered, "expected challenge scheduling on new epoch");
            #[cfg(feature = "telemetry")]
            assert_eq!(
                metrics
                    .torii_sorafs_por_challenges_total
                    .with_label_values(&["scheduled"])
                    .get(),
                1,
                "runtime should emit a scheduler metric for a published challenge"
            );

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

            let retriggered = runtime.run_once_at(now_secs).await.expect("second tick");
            assert!(
                !retriggered,
                "re-running same epoch should not schedule new challenges"
            );
        }

        #[tokio::test]
        async fn runtime_marks_forced_when_vrf_missing() {
            let temp_dir = tempdir().expect("temp dir");
            let governance_dir = temp_dir.path().join("governance");
            let handle = NodeHandle::new(storage_config(temp_dir.path()));
            let provider_id = [0x22; 32];
            declare_capacity(&handle, provider_id);
            let payload = vec![0xBC; 256 * 1024];
            let (_manifest_digest, _manifest) = ingest_manifest(&handle, &payload);

            let epoch_interval = 1_800;
            let epoch_id = 600_000;
            let vrf_deadline = 300;
            let now_secs = epoch_id * epoch_interval + vrf_deadline;
            let randomness = PorRandomness {
                epoch_id,
                issued_at_unix: now_secs,
                response_window_secs: 600,
                drand_round: 21_000,
                drand_randomness: [0x31; 32],
                drand_signature: [0xAA; 48],
            };
            let randomness_provider = Arc::new(StaticRandomnessProvider {
                randomness: randomness.clone(),
            });
            let vrf_provider = Arc::new(StaticVrfProvider::default());
            let publisher = Arc::new(
                FilesystemGovernancePublisher::try_new(governance_dir.clone())
                    .expect("governance publisher"),
            );
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
                vrf_deadline,
            );

            let triggered = runtime.run_once_at(now_secs).await.expect("runtime tick");
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
