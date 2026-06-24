//! Local SoraFS moderation ballot lifecycle runtime.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::sorafs::moderation::{
    AdversarialCorpusManifestV1, ModerationReproManifestV1, SoraFsModerationBallotCommitV1,
    SoraFsModerationBallotContextV1, SoraFsModerationBallotError, SoraFsModerationBallotRevealV1,
    SoraFsModerationVoteChoice,
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::{
    SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
    SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
    SoraFsModerationBallotGovernanceTallyV1, SoraFsModerationVoteChoiceV1,
    SoraFsModerationVoteCountsV1,
};
use thiserror::Error;

const MODERATION_ROSTER_HASH_DOMAIN_V1: &[u8] = b"sorafs.moderation.local.panel-roster-hash.v1";
const MODERATION_SCREENING_RECORD_DOMAIN_V1: &[u8] = b"sorafs.moderation.local.screening-record.v1";
const MODERATION_QUARANTINE_RECORD_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local.quarantine-record.v1";

/// Derive the local moderation panel roster hash for an ordered juror roster.
///
/// The local runtime includes the quorum and ordered juror identifiers so
/// replayed announcements cannot silently swap failover order or quorum policy.
#[must_use]
pub fn local_moderation_panel_roster_hash(juror_ids: &[String], quorum: u16) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_ROSTER_HASH_DOMAIN_V1);
    hasher.update(&quorum.to_le_bytes());
    hasher.update(&(juror_ids.len() as u64).to_le_bytes());
    for juror_id in juror_ids {
        hasher.update(&(juror_id.len() as u64).to_le_bytes());
        hasher.update(juror_id.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

/// Local moderation ballot announcement accepted by the lifecycle runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotAnnouncement {
    /// Immutable case context bound by every commit and reveal.
    pub context: SoraFsModerationBallotContextV1,
    /// Confirmed appeal deposit asset-lock id that admitted this ballot.
    pub appeal_deposit_escrow_id_hex: Option<String>,
    /// Confirmed appeal deposit metadata used to publish tally finance reports.
    pub appeal_deposit: Option<ModerationAppealDeposit>,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Ordered juror identifiers eligible to participate in this ballot.
    pub juror_ids: Vec<String>,
    /// Minimum number of valid reveals required before a tally can finalize.
    pub quorum: u16,
    /// UTC timestamp (milliseconds) when the ballot was announced.
    pub announced_at_unix_ms: u64,
    /// Last UTC timestamp (milliseconds) at which commits are accepted.
    pub commit_deadline_unix_ms: u64,
    /// Last UTC timestamp (milliseconds) for the post-commit challenge buffer.
    pub challenge_deadline_unix_ms: u64,
    /// Last UTC timestamp (milliseconds) at which reveals are accepted.
    pub reveal_deadline_unix_ms: u64,
}

/// Confirmed appeal deposit metadata captured at moderation intake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationAppealDeposit {
    /// Confirmed native asset-lock escrow id.
    pub escrow_id_hex: String,
    /// Canonical account that funded the deposit lock.
    pub payer_account: String,
    /// Canonical account receiving non-refunded deposit drawdowns.
    pub destination_account: String,
    /// Optional canonical account required to approve drawdowns.
    pub release_authority_account: Option<String>,
    /// Canonical asset definition held by the lock.
    pub asset_definition_id: String,
    /// Deterministic native asset-lock custody account.
    pub custody_account: String,
    /// Exact non-negative deposited XOR decimal amount.
    pub deposit_xor: String,
    /// Optional lock expiry timestamp in Unix milliseconds.
    pub expires_at_ms: Option<u64>,
    /// Client-supplied idempotency key used to derive the escrow id.
    pub idempotency_key: String,
    /// Canonical lowercase evidence hashes used to derive the escrow id.
    pub evidence_hashes_hex: Vec<String>,
}

/// Local registry record for an admitted moderation reproducibility manifest.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationReproRegistryRecord {
    /// Referenced moderation committee manifest id.
    pub manifest_id: [u8; 16],
    /// BLAKE3 digest of the manifest payload recorded in the manifest body.
    pub manifest_digest: [u8; 32],
    /// BLAKE3 digest of the compiled deterministic runner binary.
    pub runner_hash: [u8; 32],
    /// Runner version string recorded in the manifest body.
    pub runtime_version: String,
    /// Unix timestamp (seconds) when the manifest was issued.
    pub issued_at_unix: u64,
    /// Number of model fingerprint entries covered by the manifest.
    pub model_count: u32,
    /// Number of validated signer entries on the manifest.
    pub signer_count: u32,
}

/// Local registry record for an admitted adversarial corpus manifest.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationCorpusRegistryRecord {
    /// BLAKE3 digest of the canonical Norito corpus manifest bytes.
    pub corpus_digest: [u8; 32],
    /// Unix timestamp (seconds) when the corpus manifest was assembled.
    pub issued_at_unix: u64,
    /// Optional cohort label for the calibration window.
    pub cohort_label: Option<String>,
    /// Number of perceptual families in the corpus.
    pub family_count: u32,
    /// Number of variants across all corpus families.
    pub variant_count: u32,
}

/// Snapshot of the local SoraFS moderation model registry.
#[derive(Debug, Clone, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationModelRegistrySnapshot {
    /// Admitted reproducibility manifests sorted by manifest id.
    pub reproducibility_manifests: Vec<ModerationReproRegistryRecord>,
    /// Admitted adversarial corpus manifests sorted by corpus digest.
    pub adversarial_corpora: Vec<ModerationCorpusRegistryRecord>,
}

/// Local SFM-4a screening verdict.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
#[repr(u8)]
pub enum ModerationScreeningVerdict {
    /// Screening passed without additional moderation action.
    Pass = 1,
    /// Screening passed with a warning or low-severity policy hit.
    Warn = 2,
    /// Content must be quarantined for operator review.
    Quarantine = 3,
    /// Content must be escalated to a moderation panel or appeal workflow.
    Escalate = 4,
    /// Content must be blocked outright.
    Block = 5,
}

impl ModerationScreeningVerdict {
    /// Stable lower-case label used in JSON and digest material.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Quarantine => "quarantine",
            Self::Escalate => "escalate",
            Self::Block => "block",
        }
    }

    fn requires_quarantine_record(self) -> bool {
        matches!(self, Self::Quarantine | Self::Escalate)
    }
}

/// Candidate local screening result to record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationScreeningInput {
    /// Gateway or content subject identifier.
    pub subject: String,
    /// BLAKE3 digest of the screened payload or stable content reference.
    pub subject_digest: [u8; 32],
    /// Governance-approved reproducibility manifest id used for screening.
    pub manifest_id: [u8; 16],
    /// BLAKE3 digest of the deterministic runner binary.
    pub runner_hash: [u8; 32],
    /// Combined moderation score in basis points.
    pub combined_score_bps: u16,
    /// Screening verdict.
    pub verdict: ModerationScreeningVerdict,
    /// Unix timestamp (seconds) when screening completed.
    pub screened_at_unix: u64,
    /// Optional digest of evidence material retained outside this local index.
    pub evidence_digest: Option<[u8; 32]>,
    /// Optional digest of the policy/configuration used for the run.
    pub policy_digest: Option<[u8; 32]>,
    /// Optional operator note.
    pub notes: Option<String>,
}

/// Persisted local SFM-4a screening result.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationScreeningRecord {
    /// Stable record id derived from the deterministic record digest.
    pub record_id: [u8; 16],
    /// BLAKE3 digest over canonical screening record fields.
    pub record_digest: [u8; 32],
    /// Gateway or content subject identifier.
    pub subject: String,
    /// BLAKE3 digest of the screened payload or stable content reference.
    pub subject_digest: [u8; 32],
    /// Governance-approved reproducibility manifest id used for screening.
    pub manifest_id: [u8; 16],
    /// BLAKE3 digest of the deterministic runner binary.
    pub runner_hash: [u8; 32],
    /// Combined moderation score in basis points.
    pub combined_score_bps: u16,
    /// Screening verdict.
    pub verdict: ModerationScreeningVerdict,
    /// Unix timestamp (seconds) when screening completed.
    pub screened_at_unix: u64,
    /// Optional digest of evidence material retained outside this local index.
    pub evidence_digest: Option<[u8; 32]>,
    /// Optional digest of the policy/configuration used for the run.
    pub policy_digest: Option<[u8; 32]>,
    /// Optional operator note.
    pub notes: Option<String>,
}

/// Local state of a quarantined screening record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[repr(u8)]
pub enum ModerationQuarantineState {
    /// Awaiting operator or panel review.
    PendingReview = 1,
    /// Reviewed by an operator and ready for a release decision.
    Reviewed = 2,
    /// Released by an authorized operator after review.
    Released = 3,
}

impl ModerationQuarantineState {
    /// Stable lower-case label used in JSON.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PendingReview => "pending_review",
            Self::Reviewed => "reviewed",
            Self::Released => "released",
        }
    }
}

/// Local review action for a quarantined screening record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationQuarantineReviewInput {
    /// Stable quarantine id to review.
    pub quarantine_id: [u8; 16],
    /// Canonical operator or panel member that reviewed the record.
    pub reviewed_by: String,
    /// Unix timestamp (seconds) when review completed.
    pub reviewed_at_unix: u64,
    /// Optional review note.
    pub notes: Option<String>,
}

/// Local release action for a reviewed quarantine record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationQuarantineReleaseInput {
    /// Stable quarantine id to release.
    pub quarantine_id: [u8; 16],
    /// Canonical operator or release authority approving release.
    pub release_authority: String,
    /// Unix timestamp (seconds) when release completed.
    pub released_at_unix: u64,
    /// Optional release note.
    pub notes: Option<String>,
}

/// Persisted local quarantine queue record derived from a screening result.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationQuarantineRecord {
    /// Stable quarantine id derived from screening id and subject digest.
    pub quarantine_id: [u8; 16],
    /// Screening record that produced this queue entry.
    pub screening_record_id: [u8; 16],
    /// Gateway or content subject identifier.
    pub subject: String,
    /// BLAKE3 digest of the quarantined payload or stable content reference.
    pub subject_digest: [u8; 32],
    /// Triggering verdict.
    pub verdict: ModerationScreeningVerdict,
    /// Unix timestamp (seconds) when the record entered local quarantine.
    pub queued_at_unix: u64,
    /// Local review state.
    pub state: ModerationQuarantineState,
    /// Unix timestamp (seconds) when local review completed.
    pub reviewed_at_unix: Option<u64>,
    /// Canonical operator or panel member that reviewed the record.
    pub reviewed_by: Option<String>,
    /// Optional local review note.
    pub review_notes: Option<String>,
    /// Unix timestamp (seconds) when the local record was released.
    pub released_at_unix: Option<u64>,
    /// Canonical operator or authority that approved release.
    pub release_authority: Option<String>,
    /// Optional local release note.
    pub release_notes: Option<String>,
}

/// Outcome of recording one local screening result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationScreeningOutcome {
    /// Persisted screening record.
    pub record: ModerationScreeningRecord,
    /// Quarantine queue record when the verdict requires review.
    pub quarantine: Option<ModerationQuarantineRecord>,
}

/// Snapshot of local SFM-4a screening and quarantine state.
#[derive(Debug, Clone, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationScreeningSnapshot {
    /// Screening records sorted by record id.
    pub screening_records: Vec<ModerationScreeningRecord>,
    /// Quarantine records sorted by quarantine id.
    pub quarantine_records: Vec<ModerationQuarantineRecord>,
}

/// Error raised by the local screening/quarantine runtime.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModerationScreeningError {
    /// Screening input is invalid.
    #[error("invalid moderation screening input: {message}")]
    InvalidInput {
        /// Validation detail.
        message: String,
    },
    /// A screening record id collided with different content.
    #[error("moderation screening record `{record_id_hex}` conflicts with local state")]
    ConflictingRecord {
        /// Conflicting screening record id as lowercase hex.
        record_id_hex: String,
    },
    /// A requested quarantine queue record does not exist.
    #[error("moderation quarantine record `{quarantine_id_hex}` is unknown")]
    UnknownQuarantine {
        /// Unknown quarantine id as lowercase hex.
        quarantine_id_hex: String,
    },
    /// A quarantine state transition was invalid.
    #[error("moderation quarantine record `{quarantine_id_hex}` transition is invalid: {message}")]
    InvalidTransition {
        /// Quarantine id as lowercase hex.
        quarantine_id_hex: String,
        /// Transition validation detail.
        message: String,
    },
    /// A restored snapshot is internally inconsistent.
    #[error("moderation screening snapshot is invalid: {message}")]
    InvalidSnapshot {
        /// Validation detail.
        message: String,
    },
    /// The local screening runtime lock was poisoned.
    #[error("moderation screening state lock poisoned")]
    StateLockPoisoned,
}

/// Error raised while admitting moderation model registry material.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModerationModelRegistryError {
    /// A reproducibility manifest failed canonical validation.
    #[error("moderation reproducibility manifest validation failed: {message}")]
    InvalidReproManifest {
        /// Validation detail.
        message: String,
    },
    /// An adversarial corpus manifest failed canonical validation.
    #[error("moderation adversarial corpus manifest validation failed: {message}")]
    InvalidCorpusManifest {
        /// Validation detail.
        message: String,
    },
    /// A reproducibility manifest conflicts with a previously admitted manifest id.
    #[error("moderation reproducibility manifest `{manifest_id_hex}` conflicts with registry")]
    ConflictingReproManifest {
        /// Conflicting manifest id as lowercase hex.
        manifest_id_hex: String,
    },
    /// A persisted registry snapshot is not internally consistent.
    #[error("moderation model registry snapshot is invalid: {message}")]
    InvalidRegistrySnapshot {
        /// Validation detail.
        message: String,
    },
    /// Corpus manifest canonical encoding failed.
    #[error("failed to encode moderation adversarial corpus manifest: {message}")]
    CorpusEncoding {
        /// Encoding detail.
        message: String,
    },
    /// The local moderation model registry lock was poisoned.
    #[error("moderation model registry state lock poisoned")]
    StateLockPoisoned,
}

/// Local in-memory registry for moderation model release artifacts.
#[derive(Debug, Default)]
pub(crate) struct ModerationModelRegistry {
    repro_manifests: BTreeMap<[u8; 16], ModerationReproRegistryRecord>,
    corpora: BTreeMap<[u8; 32], ModerationCorpusRegistryRecord>,
}

impl ModerationModelRegistry {
    pub(crate) fn admit_repro_manifest(
        &mut self,
        manifest: ModerationReproManifestV1,
    ) -> Result<ModerationReproRegistryRecord, ModerationModelRegistryError> {
        let summary = manifest.validate().map_err(|err| {
            ModerationModelRegistryError::InvalidReproManifest {
                message: err.to_string(),
            }
        })?;
        let record = ModerationReproRegistryRecord {
            manifest_id: summary.manifest_id,
            manifest_digest: manifest.body.manifest_digest,
            runner_hash: manifest.body.runner_hash,
            runtime_version: manifest.body.runtime_version,
            issued_at_unix: summary.issued_at_unix,
            model_count: summary.model_count,
            signer_count: summary.signer_count,
        };
        match self.repro_manifests.get(&record.manifest_id) {
            Some(existing) if existing != &record => {
                Err(ModerationModelRegistryError::ConflictingReproManifest {
                    manifest_id_hex: hex::encode(record.manifest_id),
                })
            }
            Some(existing) => Ok(existing.clone()),
            None => {
                self.repro_manifests
                    .insert(record.manifest_id, record.clone());
                Ok(record)
            }
        }
    }

    pub(crate) fn admit_corpus_manifest(
        &mut self,
        manifest: AdversarialCorpusManifestV1,
    ) -> Result<ModerationCorpusRegistryRecord, ModerationModelRegistryError> {
        manifest
            .validate()
            .map_err(|err| ModerationModelRegistryError::InvalidCorpusManifest {
                message: err.to_string(),
            })?;
        let encoded = norito::to_bytes(&manifest).map_err(|err| {
            ModerationModelRegistryError::CorpusEncoding {
                message: err.to_string(),
            }
        })?;
        let corpus_digest = *blake3::hash(&encoded).as_bytes();
        let family_count = manifest.families.len().min(u32::MAX as usize) as u32;
        let variant_count = manifest
            .families
            .iter()
            .map(|family| family.variants.len())
            .sum::<usize>()
            .min(u32::MAX as usize) as u32;
        let record = ModerationCorpusRegistryRecord {
            corpus_digest,
            issued_at_unix: manifest.issued_at_unix,
            cohort_label: manifest.cohort_label,
            family_count,
            variant_count,
        };
        Ok(self
            .corpora
            .entry(record.corpus_digest)
            .or_insert_with(|| record.clone())
            .clone())
    }

    pub(crate) fn snapshot(&self) -> ModerationModelRegistrySnapshot {
        ModerationModelRegistrySnapshot {
            reproducibility_manifests: self.repro_manifests.values().cloned().collect(),
            adversarial_corpora: self.corpora.values().cloned().collect(),
        }
    }

    pub(crate) fn restore_snapshot(
        &mut self,
        snapshot: ModerationModelRegistrySnapshot,
    ) -> Result<(), ModerationModelRegistryError> {
        let mut repro_manifests = BTreeMap::new();
        for record in snapshot.reproducibility_manifests {
            if record.runtime_version.trim().is_empty() {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: format!(
                        "reproducibility manifest `{}` has an empty runtime version",
                        hex::encode(record.manifest_id)
                    ),
                });
            }
            if record.model_count == 0 {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: format!(
                        "reproducibility manifest `{}` has no model fingerprints",
                        hex::encode(record.manifest_id)
                    ),
                });
            }
            if record.signer_count == 0 {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: format!(
                        "reproducibility manifest `{}` has no governance signers",
                        hex::encode(record.manifest_id)
                    ),
                });
            }
            if repro_manifests.insert(record.manifest_id, record).is_some() {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: "duplicate reproducibility manifest id".to_string(),
                });
            }
        }

        let mut corpora = BTreeMap::new();
        for record in snapshot.adversarial_corpora {
            if record.family_count == 0 {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: format!(
                        "adversarial corpus `{}` has no perceptual families",
                        hex::encode(record.corpus_digest)
                    ),
                });
            }
            if record.variant_count == 0 {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: format!(
                        "adversarial corpus `{}` has no variants",
                        hex::encode(record.corpus_digest)
                    ),
                });
            }
            if corpora.insert(record.corpus_digest, record).is_some() {
                return Err(ModerationModelRegistryError::InvalidRegistrySnapshot {
                    message: "duplicate adversarial corpus digest".to_string(),
                });
            }
        }

        self.repro_manifests = repro_manifests;
        self.corpora = corpora;
        Ok(())
    }
}

/// Local in-memory runtime for SFM-4a screening and quarantine evidence.
#[derive(Debug, Default)]
pub(crate) struct ModerationScreeningRuntime {
    screening_records: BTreeMap<[u8; 16], ModerationScreeningRecord>,
    quarantine_records: BTreeMap<[u8; 16], ModerationQuarantineRecord>,
}

impl ModerationScreeningRuntime {
    pub(crate) fn record_screening(
        &mut self,
        input: ModerationScreeningInput,
    ) -> Result<ModerationScreeningOutcome, ModerationScreeningError> {
        let record = screening_record_from_input(input)?;
        let quarantine = record
            .verdict
            .requires_quarantine_record()
            .then(|| quarantine_record_from_screening(&record));

        match self.screening_records.get(&record.record_id) {
            Some(existing) if existing != &record => {
                return Err(ModerationScreeningError::ConflictingRecord {
                    record_id_hex: hex::encode(record.record_id),
                });
            }
            Some(existing) => {
                let quarantine = quarantine
                    .as_ref()
                    .and_then(|record| self.quarantine_records.get(&record.quarantine_id))
                    .cloned();
                return Ok(ModerationScreeningOutcome {
                    record: existing.clone(),
                    quarantine,
                });
            }
            None => {
                self.screening_records
                    .insert(record.record_id, record.clone());
            }
        }

        if let Some(quarantine) = quarantine.clone() {
            self.quarantine_records
                .insert(quarantine.quarantine_id, quarantine);
        }

        Ok(ModerationScreeningOutcome { record, quarantine })
    }

    pub(crate) fn snapshot(&self) -> ModerationScreeningSnapshot {
        ModerationScreeningSnapshot {
            screening_records: self.screening_records.values().cloned().collect(),
            quarantine_records: self.quarantine_records.values().cloned().collect(),
        }
    }

    pub(crate) fn review_quarantine(
        &mut self,
        input: ModerationQuarantineReviewInput,
    ) -> Result<ModerationQuarantineRecord, ModerationScreeningError> {
        let reviewed_by =
            clean_required_text(input.reviewed_by, "reviewed_by", input.quarantine_id, true)?;
        if input.reviewed_at_unix == 0 {
            return Err(ModerationScreeningError::InvalidTransition {
                quarantine_id_hex: hex::encode(input.quarantine_id),
                message: "reviewed_at_unix must be non-zero".to_string(),
            });
        }
        let review_notes = clean_optional_text(input.notes);
        let record = self
            .quarantine_records
            .get_mut(&input.quarantine_id)
            .ok_or(ModerationScreeningError::UnknownQuarantine {
                quarantine_id_hex: hex::encode(input.quarantine_id),
            })?;

        match record.state {
            ModerationQuarantineState::PendingReview => {
                record.state = ModerationQuarantineState::Reviewed;
                record.reviewed_at_unix = Some(input.reviewed_at_unix);
                record.reviewed_by = Some(reviewed_by);
                record.review_notes = review_notes;
                Ok(record.clone())
            }
            ModerationQuarantineState::Reviewed
                if record.reviewed_at_unix == Some(input.reviewed_at_unix)
                    && record.reviewed_by.as_deref() == Some(reviewed_by.as_str())
                    && record.review_notes == review_notes =>
            {
                Ok(record.clone())
            }
            ModerationQuarantineState::Reviewed => {
                Err(ModerationScreeningError::InvalidTransition {
                    quarantine_id_hex: hex::encode(input.quarantine_id),
                    message: "record is already reviewed with different metadata".to_string(),
                })
            }
            ModerationQuarantineState::Released => {
                Err(ModerationScreeningError::InvalidTransition {
                    quarantine_id_hex: hex::encode(input.quarantine_id),
                    message: "released records cannot be reviewed again".to_string(),
                })
            }
        }
    }

    pub(crate) fn release_quarantine(
        &mut self,
        input: ModerationQuarantineReleaseInput,
    ) -> Result<ModerationQuarantineRecord, ModerationScreeningError> {
        let release_authority = clean_required_text(
            input.release_authority,
            "release_authority",
            input.quarantine_id,
            true,
        )?;
        if input.released_at_unix == 0 {
            return Err(ModerationScreeningError::InvalidTransition {
                quarantine_id_hex: hex::encode(input.quarantine_id),
                message: "released_at_unix must be non-zero".to_string(),
            });
        }
        let release_notes = clean_optional_text(input.notes);
        let record = self
            .quarantine_records
            .get_mut(&input.quarantine_id)
            .ok_or(ModerationScreeningError::UnknownQuarantine {
                quarantine_id_hex: hex::encode(input.quarantine_id),
            })?;

        match record.state {
            ModerationQuarantineState::PendingReview => {
                Err(ModerationScreeningError::InvalidTransition {
                    quarantine_id_hex: hex::encode(input.quarantine_id),
                    message: "record must be reviewed before release".to_string(),
                })
            }
            ModerationQuarantineState::Reviewed => {
                if record
                    .reviewed_at_unix
                    .is_some_and(|reviewed_at| input.released_at_unix < reviewed_at)
                {
                    return Err(ModerationScreeningError::InvalidTransition {
                        quarantine_id_hex: hex::encode(input.quarantine_id),
                        message: "released_at_unix must be >= reviewed_at_unix".to_string(),
                    });
                }
                record.state = ModerationQuarantineState::Released;
                record.released_at_unix = Some(input.released_at_unix);
                record.release_authority = Some(release_authority);
                record.release_notes = release_notes;
                Ok(record.clone())
            }
            ModerationQuarantineState::Released
                if record.released_at_unix == Some(input.released_at_unix)
                    && record.release_authority.as_deref() == Some(release_authority.as_str())
                    && record.release_notes == release_notes =>
            {
                Ok(record.clone())
            }
            ModerationQuarantineState::Released => {
                Err(ModerationScreeningError::InvalidTransition {
                    quarantine_id_hex: hex::encode(input.quarantine_id),
                    message: "record is already released with different metadata".to_string(),
                })
            }
        }
    }

    pub(crate) fn restore_snapshot(
        &mut self,
        snapshot: ModerationScreeningSnapshot,
    ) -> Result<(), ModerationScreeningError> {
        let mut screening_records = BTreeMap::new();
        for record in snapshot.screening_records {
            validate_screening_record(&record)?;
            if screening_records.insert(record.record_id, record).is_some() {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: "duplicate moderation screening record id".to_string(),
                });
            }
        }

        let mut quarantine_records = BTreeMap::new();
        for quarantine in snapshot.quarantine_records {
            validate_quarantine_record(&quarantine)?;
            let Some(screening_record) = screening_records.get(&quarantine.screening_record_id)
            else {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: format!(
                        "quarantine record `{}` references unknown screening record `{}`",
                        hex::encode(quarantine.quarantine_id),
                        hex::encode(quarantine.screening_record_id)
                    ),
                });
            };
            validate_quarantine_record_matches_screening(&quarantine, screening_record)?;
            if quarantine_records
                .insert(quarantine.quarantine_id, quarantine)
                .is_some()
            {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: "duplicate moderation quarantine record id".to_string(),
                });
            }
        }

        self.screening_records = screening_records;
        self.quarantine_records = quarantine_records;
        Ok(())
    }
}

fn screening_record_from_input(
    input: ModerationScreeningInput,
) -> Result<ModerationScreeningRecord, ModerationScreeningError> {
    if input.subject.trim().is_empty() {
        return Err(ModerationScreeningError::InvalidInput {
            message: "subject must not be blank".to_string(),
        });
    }
    if input.combined_score_bps > 10_000 {
        return Err(ModerationScreeningError::InvalidInput {
            message: "combined_score_bps must be <= 10000".to_string(),
        });
    }
    if input.screened_at_unix == 0 {
        return Err(ModerationScreeningError::InvalidInput {
            message: "screened_at_unix must be non-zero".to_string(),
        });
    }

    let record_digest = screening_record_digest(
        &input.subject,
        input.subject_digest,
        input.manifest_id,
        input.runner_hash,
        input.combined_score_bps,
        input.verdict,
        input.screened_at_unix,
        input.evidence_digest,
        input.policy_digest,
        input.notes.as_deref(),
    );
    let mut record_id = [0u8; 16];
    record_id.copy_from_slice(&record_digest[..16]);
    Ok(ModerationScreeningRecord {
        record_id,
        record_digest,
        subject: input.subject,
        subject_digest: input.subject_digest,
        manifest_id: input.manifest_id,
        runner_hash: input.runner_hash,
        combined_score_bps: input.combined_score_bps,
        verdict: input.verdict,
        screened_at_unix: input.screened_at_unix,
        evidence_digest: input.evidence_digest,
        policy_digest: input.policy_digest,
        notes: input.notes,
    })
}

fn validate_screening_record(
    record: &ModerationScreeningRecord,
) -> Result<(), ModerationScreeningError> {
    if record.subject.trim().is_empty() {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: "screening record subject must not be blank".to_string(),
        });
    }
    if record.combined_score_bps > 10_000 {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "screening record `{}` has combined_score_bps > 10000",
                hex::encode(record.record_id)
            ),
        });
    }
    if record.screened_at_unix == 0 {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "screening record `{}` has zero screened_at_unix",
                hex::encode(record.record_id)
            ),
        });
    }
    let expected_digest = screening_record_digest(
        &record.subject,
        record.subject_digest,
        record.manifest_id,
        record.runner_hash,
        record.combined_score_bps,
        record.verdict,
        record.screened_at_unix,
        record.evidence_digest,
        record.policy_digest,
        record.notes.as_deref(),
    );
    let mut expected_id = [0u8; 16];
    expected_id.copy_from_slice(&expected_digest[..16]);
    if record.record_digest != expected_digest || record.record_id != expected_id {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "screening record `{}` digest/id mismatch",
                hex::encode(record.record_id)
            ),
        });
    }
    Ok(())
}

fn quarantine_record_from_screening(
    record: &ModerationScreeningRecord,
) -> ModerationQuarantineRecord {
    let quarantine_digest = quarantine_record_digest(
        record.record_id,
        record.subject_digest,
        record.verdict,
        record.screened_at_unix,
    );
    let mut quarantine_id = [0u8; 16];
    quarantine_id.copy_from_slice(&quarantine_digest[..16]);
    ModerationQuarantineRecord {
        quarantine_id,
        screening_record_id: record.record_id,
        subject: record.subject.clone(),
        subject_digest: record.subject_digest,
        verdict: record.verdict,
        queued_at_unix: record.screened_at_unix,
        state: ModerationQuarantineState::PendingReview,
        reviewed_at_unix: None,
        reviewed_by: None,
        review_notes: None,
        released_at_unix: None,
        release_authority: None,
        release_notes: None,
    }
}

fn validate_quarantine_record(
    record: &ModerationQuarantineRecord,
) -> Result<(), ModerationScreeningError> {
    if !record.verdict.requires_quarantine_record() {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` has non-quarantine verdict `{}`",
                hex::encode(record.quarantine_id),
                record.verdict.as_str()
            ),
        });
    }
    if record.subject.trim().is_empty() {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` has blank subject",
                hex::encode(record.quarantine_id)
            ),
        });
    }
    if record.queued_at_unix == 0 {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` has zero queued_at_unix",
                hex::encode(record.quarantine_id)
            ),
        });
    }
    validate_quarantine_state_fields(record)?;
    Ok(())
}

fn validate_quarantine_record_matches_screening(
    record: &ModerationQuarantineRecord,
    screening_record: &ModerationScreeningRecord,
) -> Result<(), ModerationScreeningError> {
    let expected = quarantine_record_from_screening(screening_record);
    if record.quarantine_id != expected.quarantine_id
        || record.screening_record_id != expected.screening_record_id
        || record.subject != expected.subject
        || record.subject_digest != expected.subject_digest
        || record.verdict != expected.verdict
        || record.queued_at_unix != expected.queued_at_unix
    {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` does not match its screening record",
                hex::encode(record.quarantine_id)
            ),
        });
    }
    Ok(())
}

fn validate_quarantine_state_fields(
    record: &ModerationQuarantineRecord,
) -> Result<(), ModerationScreeningError> {
    match record.state {
        ModerationQuarantineState::PendingReview => {
            if record.reviewed_at_unix.is_some()
                || record.reviewed_by.is_some()
                || record.review_notes.is_some()
                || record.released_at_unix.is_some()
                || record.release_authority.is_some()
                || record.release_notes.is_some()
            {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: format!(
                        "pending quarantine record `{}` must not carry review/release metadata",
                        hex::encode(record.quarantine_id)
                    ),
                });
            }
        }
        ModerationQuarantineState::Reviewed => {
            validate_nonzero_optional_timestamp(
                record.quarantine_id,
                "reviewed_at_unix",
                record.reviewed_at_unix,
            )?;
            validate_nonblank_optional_text(
                record.quarantine_id,
                "reviewed_by",
                &record.reviewed_by,
            )?;
            if record.released_at_unix.is_some()
                || record.release_authority.is_some()
                || record.release_notes.is_some()
            {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: format!(
                        "reviewed quarantine record `{}` must not carry release metadata",
                        hex::encode(record.quarantine_id)
                    ),
                });
            }
        }
        ModerationQuarantineState::Released => {
            validate_nonzero_optional_timestamp(
                record.quarantine_id,
                "reviewed_at_unix",
                record.reviewed_at_unix,
            )?;
            validate_nonblank_optional_text(
                record.quarantine_id,
                "reviewed_by",
                &record.reviewed_by,
            )?;
            validate_nonzero_optional_timestamp(
                record.quarantine_id,
                "released_at_unix",
                record.released_at_unix,
            )?;
            validate_nonblank_optional_text(
                record.quarantine_id,
                "release_authority",
                &record.release_authority,
            )?;
            if let (Some(reviewed_at), Some(released_at)) =
                (record.reviewed_at_unix, record.released_at_unix)
            {
                if released_at < reviewed_at {
                    return Err(ModerationScreeningError::InvalidSnapshot {
                        message: format!(
                            "released quarantine record `{}` has released_at_unix < reviewed_at_unix",
                            hex::encode(record.quarantine_id)
                        ),
                    });
                }
            }
        }
    }
    validate_optional_note(record.quarantine_id, "review_notes", &record.review_notes)?;
    validate_optional_note(record.quarantine_id, "release_notes", &record.release_notes)
}

fn validate_nonzero_optional_timestamp(
    quarantine_id: [u8; 16],
    field: &str,
    value: Option<u64>,
) -> Result<(), ModerationScreeningError> {
    match value {
        Some(0) | None => Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` requires non-zero {field}",
                hex::encode(quarantine_id)
            ),
        }),
        Some(_) => Ok(()),
    }
}

fn validate_nonblank_optional_text(
    quarantine_id: [u8; 16],
    field: &str,
    value: &Option<String>,
) -> Result<(), ModerationScreeningError> {
    match value.as_deref() {
        Some(value) if !value.trim().is_empty() => Ok(()),
        _ => Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` requires non-blank {field}",
                hex::encode(quarantine_id)
            ),
        }),
    }
}

fn validate_optional_note(
    quarantine_id: [u8; 16],
    field: &str,
    value: &Option<String>,
) -> Result<(), ModerationScreeningError> {
    if value
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(ModerationScreeningError::InvalidSnapshot {
            message: format!(
                "quarantine record `{}` has blank {field}",
                hex::encode(quarantine_id)
            ),
        });
    }
    Ok(())
}

fn clean_required_text(
    value: String,
    field: &str,
    quarantine_id: [u8; 16],
    transition_error: bool,
) -> Result<String, ModerationScreeningError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        let message = format!("{field} must not be blank");
        if transition_error {
            return Err(ModerationScreeningError::InvalidTransition {
                quarantine_id_hex: hex::encode(quarantine_id),
                message,
            });
        }
        return Err(ModerationScreeningError::InvalidInput { message });
    }
    Ok(value)
}

fn clean_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

#[allow(clippy::too_many_arguments)]
fn screening_record_digest(
    subject: &str,
    subject_digest: [u8; 32],
    manifest_id: [u8; 16],
    runner_hash: [u8; 32],
    combined_score_bps: u16,
    verdict: ModerationScreeningVerdict,
    screened_at_unix: u64,
    evidence_digest: Option<[u8; 32]>,
    policy_digest: Option<[u8; 32]>,
    notes: Option<&str>,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_SCREENING_RECORD_DOMAIN_V1);
    update_string(&mut hasher, subject);
    hasher.update(&subject_digest);
    hasher.update(&manifest_id);
    hasher.update(&runner_hash);
    hasher.update(&combined_score_bps.to_le_bytes());
    update_string(&mut hasher, verdict.as_str());
    hasher.update(&screened_at_unix.to_le_bytes());
    update_optional_digest(&mut hasher, evidence_digest);
    update_optional_digest(&mut hasher, policy_digest);
    update_optional_string(&mut hasher, notes);
    *hasher.finalize().as_bytes()
}

fn quarantine_record_digest(
    screening_record_id: [u8; 16],
    subject_digest: [u8; 32],
    verdict: ModerationScreeningVerdict,
    queued_at_unix: u64,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_QUARANTINE_RECORD_DOMAIN_V1);
    hasher.update(&screening_record_id);
    hasher.update(&subject_digest);
    update_string(&mut hasher, verdict.as_str());
    hasher.update(&queued_at_unix.to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn update_string(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn update_optional_digest(hasher: &mut blake3::Hasher, value: Option<[u8; 32]>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hasher.update(&value);
        }
        None => {
            hasher.update(&[0]);
        }
    };
}

fn update_optional_string(hasher: &mut blake3::Hasher, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            update_string(hasher, value);
        }
        None => {
            hasher.update(&[0]);
        }
    };
}

/// Sequenced local moderation ballot event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationBallotEventKind {
    /// A ballot was announced.
    BallotAnnounced,
    /// A juror commitment was accepted.
    CommitAccepted,
    /// A juror reveal was accepted.
    RevealAccepted,
    /// The ballot was tallied.
    BallotTallied,
}

impl From<ModerationBallotEventKind> for SoraFsModerationBallotGovernanceEventKindV1 {
    fn from(kind: ModerationBallotEventKind) -> Self {
        match kind {
            ModerationBallotEventKind::BallotAnnounced => Self::BallotAnnounced,
            ModerationBallotEventKind::CommitAccepted => Self::CommitAccepted,
            ModerationBallotEventKind::RevealAccepted => Self::RevealAccepted,
            ModerationBallotEventKind::BallotTallied => Self::BallotTallied,
        }
    }
}

/// Local moderation ballot vote counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModerationVoteCounts {
    /// Number of `uphold` reveals.
    pub uphold: u32,
    /// Number of `overturn` reveals.
    pub overturn: u32,
    /// Number of `modify` reveals.
    pub modify: u32,
    /// Number of `escalate` reveals.
    pub escalate: u32,
}

impl ModerationVoteCounts {
    fn increment(&mut self, choice: SoraFsModerationVoteChoice) {
        match choice {
            SoraFsModerationVoteChoice::Uphold => self.uphold = self.uphold.saturating_add(1),
            SoraFsModerationVoteChoice::Overturn => {
                self.overturn = self.overturn.saturating_add(1);
            }
            SoraFsModerationVoteChoice::Modify => self.modify = self.modify.saturating_add(1),
            SoraFsModerationVoteChoice::Escalate => {
                self.escalate = self.escalate.saturating_add(1);
            }
        }
    }

    fn winning_choice(self) -> Option<SoraFsModerationVoteChoice> {
        let choices = [
            (SoraFsModerationVoteChoice::Uphold, self.uphold),
            (SoraFsModerationVoteChoice::Overturn, self.overturn),
            (SoraFsModerationVoteChoice::Modify, self.modify),
            (SoraFsModerationVoteChoice::Escalate, self.escalate),
        ];
        let max_votes = choices.iter().map(|(_, count)| *count).max().unwrap_or(0);
        if max_votes == 0
            || choices
                .iter()
                .filter(|(_, count)| *count == max_votes)
                .count()
                != 1
        {
            return None;
        }
        choices
            .into_iter()
            .find_map(|(choice, count)| (count == max_votes).then_some(choice))
    }
}

impl From<ModerationVoteCounts> for SoraFsModerationVoteCountsV1 {
    fn from(counts: ModerationVoteCounts) -> Self {
        Self {
            uphold: counts.uphold,
            overturn: counts.overturn,
            modify: counts.modify,
            escalate: counts.escalate,
        }
    }
}

/// Final local tally for one moderation ballot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotTally {
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Vote counts by moderation choice.
    pub counts: ModerationVoteCounts,
    /// Number of valid reveals included in the tally.
    pub votes_total: u32,
    /// Required reveal quorum.
    pub quorum: u16,
    /// Winning choice when the tally has exactly one highest vote count.
    pub winning_choice: Option<SoraFsModerationVoteChoice>,
    /// True when the vote reached quorum but no unique winner exists.
    pub contested: bool,
    /// UTC timestamp (milliseconds) when the tally was finalized locally.
    pub tallied_at_unix_ms: u64,
}

impl From<&ModerationBallotTally> for SoraFsModerationBallotGovernanceTallyV1 {
    fn from(tally: &ModerationBallotTally) -> Self {
        Self {
            case_id: tally.case_id.clone(),
            round_id: tally.round_id.clone(),
            counts: tally.counts.into(),
            votes_total: tally.votes_total,
            quorum: tally.quorum,
            winning_choice: tally.winning_choice.map(governance_vote_choice),
            contested: tally.contested,
            tallied_at_unix_ms: tally.tallied_at_unix_ms,
        }
    }
}

/// Snapshot of one local moderation ballot lifecycle record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotRecord {
    /// Ballot announcement.
    pub announcement: ModerationBallotAnnouncement,
    /// Accepted juror commitments sorted by juror id.
    pub commits: Vec<SoraFsModerationBallotCommitV1>,
    /// Accepted juror reveals sorted by juror id.
    pub reveals: Vec<SoraFsModerationBallotRevealV1>,
    /// Final tally when the ballot has been finalized.
    pub tally: Option<ModerationBallotTally>,
}

/// Result of accepting a moderation ballot commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotCommitOutcome {
    /// Accepted commitment payload.
    pub accepted_commit: SoraFsModerationBallotCommitV1,
    /// Number of commitments accepted for the ballot.
    pub committed_count: usize,
    /// Number of reveals accepted for the ballot.
    pub revealed_count: usize,
}

/// Result of accepting a moderation ballot reveal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotRevealOutcome {
    /// Accepted reveal payload.
    pub accepted_reveal: SoraFsModerationBallotRevealV1,
    /// Number of commitments accepted for the ballot.
    pub committed_count: usize,
    /// Number of reveals accepted for the ballot.
    pub revealed_count: usize,
}

/// Sequenced local moderation ballot event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationBallotEvent {
    /// Monotonic local event sequence.
    pub sequence: u64,
    /// Event kind.
    pub kind: ModerationBallotEventKind,
    /// UTC timestamp (milliseconds) when the event was generated.
    pub generated_at_unix_ms: u64,
    /// Moderation or appeal case identifier.
    pub case_id: String,
    /// Moderation ballot round identifier.
    pub round_id: String,
    /// Juror associated with the event, when any.
    pub juror_id: Option<String>,
    /// Accepted commitment count after the event.
    pub committed_count: u64,
    /// Accepted reveal count after the event.
    pub revealed_count: u64,
    /// Tally associated with the event, when finalized.
    pub tally: Option<ModerationBallotTally>,
}

impl ModerationBallotEvent {
    pub(crate) fn to_governance_event_v1(&self) -> SoraFsModerationBallotGovernanceEventV1 {
        SoraFsModerationBallotGovernanceEventV1 {
            version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            sequence: self.sequence,
            kind: self.kind.into(),
            generated_at_unix_ms: self.generated_at_unix_ms,
            case_id: self.case_id.clone(),
            round_id: self.round_id.clone(),
            juror_id: self.juror_id.clone(),
            committed_count: self.committed_count,
            revealed_count: self.revealed_count,
            tally: self.tally.as_ref().map(Into::into),
        }
    }
}

fn governance_vote_choice(choice: SoraFsModerationVoteChoice) -> SoraFsModerationVoteChoiceV1 {
    match choice {
        SoraFsModerationVoteChoice::Uphold => SoraFsModerationVoteChoiceV1::Uphold,
        SoraFsModerationVoteChoice::Overturn => SoraFsModerationVoteChoiceV1::Overturn,
        SoraFsModerationVoteChoice::Modify => SoraFsModerationVoteChoiceV1::Modify,
        SoraFsModerationVoteChoice::Escalate => SoraFsModerationVoteChoiceV1::Escalate,
    }
}

/// Error raised by the local moderation ballot runtime.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModerationBallotRuntimeError {
    /// Canonical moderation ballot payload validation failed.
    #[error(transparent)]
    Validation(#[from] SoraFsModerationBallotError),
    /// Ballot already exists for the case and round.
    #[error("moderation ballot `{case_id}` round `{round_id}` already exists")]
    DuplicateBallot {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
    },
    /// Ballot is unknown for the case and round.
    #[error("moderation ballot `{case_id}` round `{round_id}` is unknown")]
    UnknownBallot {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
    },
    /// Ballot round id is blank.
    #[error("moderation ballot round id is required")]
    MissingRoundId,
    /// Ballot roster is empty.
    #[error("moderation ballot juror roster is required")]
    MissingJurors,
    /// Ballot roster contains a blank juror id.
    #[error("moderation ballot juror id is required")]
    BlankJurorId,
    /// Ballot roster contains a duplicate juror id.
    #[error("moderation ballot juror `{juror_id}` is duplicated")]
    DuplicateJuror {
        /// Duplicated juror identifier.
        juror_id: String,
    },
    /// Ballot quorum is zero or larger than the roster.
    #[error("moderation ballot quorum `{quorum}` is invalid for roster size `{roster_size}`")]
    InvalidQuorum {
        /// Requested quorum.
        quorum: u16,
        /// Roster length.
        roster_size: usize,
    },
    /// Announcement roster hash does not match the local canonical roster hash.
    #[error("moderation ballot roster hash does not match juror roster and quorum")]
    RosterHashMismatch,
    /// Stored appeal deposit metadata is missing its escrow id.
    #[error("moderation ballot appeal deposit escrow id is required")]
    MissingAppealDepositEscrowId,
    /// Stored appeal deposit metadata conflicts with the compatibility escrow id field.
    #[error("moderation ballot appeal deposit escrow id metadata mismatch")]
    AppealDepositEscrowIdMismatch,
    /// Ballot window timestamps are inconsistent.
    #[error("moderation ballot windows must satisfy announced < commit < challenge < reveal")]
    InvalidWindows,
    /// Commit/reveal payload is bound to a different context.
    #[error("moderation ballot payload context mismatch")]
    PayloadContextMismatch,
    /// Commit/reveal payload is bound to a different round.
    #[error("moderation ballot payload round mismatch: expected `{expected}`, found `{found}`")]
    PayloadRoundMismatch {
        /// Expected round identifier.
        expected: String,
        /// Payload round identifier.
        found: String,
    },
    /// Juror is not part of the announced roster.
    #[error(
        "juror `{juror_id}` is not eligible for moderation ballot `{case_id}` round `{round_id}`"
    )]
    IneligibleJuror {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// Commit submission arrived outside the commit window.
    #[error("moderation ballot commit window is closed at `{now_unix_ms}`")]
    CommitWindowClosed {
        /// Acceptance timestamp.
        now_unix_ms: u64,
    },
    /// A commitment already exists for the juror.
    #[error(
        "juror `{juror_id}` already committed for moderation ballot `{case_id}` round `{round_id}`"
    )]
    DuplicateCommit {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// Reveal submission arrived before the reveal window opened.
    #[error("moderation ballot reveal window is not open at `{now_unix_ms}`")]
    RevealWindowNotOpen {
        /// Acceptance timestamp.
        now_unix_ms: u64,
    },
    /// Reveal submission arrived after the reveal window closed.
    #[error("moderation ballot reveal window is closed at `{now_unix_ms}`")]
    RevealWindowClosed {
        /// Acceptance timestamp.
        now_unix_ms: u64,
    },
    /// The juror has no accepted commitment to reveal.
    #[error(
        "juror `{juror_id}` has no commitment for moderation ballot `{case_id}` round `{round_id}`"
    )]
    MissingCommit {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// A reveal already exists for the juror.
    #[error(
        "juror `{juror_id}` already revealed for moderation ballot `{case_id}` round `{round_id}`"
    )]
    DuplicateReveal {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
        /// Juror identifier.
        juror_id: String,
    },
    /// Tally was requested before the reveal window closed and before all jurors revealed.
    #[error("moderation ballot reveal window is still open until `{reveal_deadline_unix_ms}`")]
    TallyWindowOpen {
        /// Reveal deadline.
        reveal_deadline_unix_ms: u64,
    },
    /// Tally was already finalized.
    #[error("moderation ballot `{case_id}` round `{round_id}` was already tallied")]
    AlreadyTallied {
        /// Case identifier.
        case_id: String,
        /// Round identifier.
        round_id: String,
    },
    /// Accepted reveals did not satisfy quorum.
    #[error("moderation ballot quorum `{quorum}` not met by `{reveals}` reveals")]
    QuorumNotMet {
        /// Required quorum.
        quorum: u16,
        /// Accepted reveal count.
        reveals: usize,
    },
    /// The local moderation runtime lock was poisoned.
    #[error("moderation ballot state lock poisoned")]
    StateLockPoisoned,
}

/// Local in-memory moderation ballot lifecycle runtime.
#[derive(Debug, Default)]
pub(crate) struct ModerationBallotRuntime {
    ballots: BTreeMap<ModerationBallotKey, ModerationBallotState>,
}

impl ModerationBallotRuntime {
    pub(crate) fn announce_ballot(
        &mut self,
        announcement: ModerationBallotAnnouncement,
    ) -> Result<ModerationBallotRecord, ModerationBallotRuntimeError> {
        validate_announcement(&announcement)?;
        let key = ModerationBallotKey::from_announcement(&announcement);
        if self.ballots.contains_key(&key) {
            return Err(ModerationBallotRuntimeError::DuplicateBallot {
                case_id: key.case_id,
                round_id: key.round_id,
            });
        }
        self.ballots.insert(
            key.clone(),
            ModerationBallotState {
                announcement,
                commits: BTreeMap::new(),
                reveals: BTreeMap::new(),
                tally: None,
            },
        );
        Ok(self
            .ballots
            .get(&key)
            .expect("inserted moderation ballot exists")
            .to_record())
    }

    pub(crate) fn submit_commit(
        &mut self,
        commit: SoraFsModerationBallotCommitV1,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotCommitOutcome, ModerationBallotRuntimeError> {
        commit.validate()?;
        let key = ModerationBallotKey::from_context_and_round(&commit.context, &commit.round_id);
        let state = self.ballots.get_mut(&key).ok_or_else(|| {
            ModerationBallotRuntimeError::UnknownBallot {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
            }
        })?;
        validate_payload_scope(state, &commit.context, &commit.round_id)?;
        if now_unix_ms < state.announcement.announced_at_unix_ms
            || now_unix_ms > state.announcement.commit_deadline_unix_ms
        {
            return Err(ModerationBallotRuntimeError::CommitWindowClosed { now_unix_ms });
        }
        ensure_eligible_juror(state, &commit.juror_id)?;
        if state.commits.contains_key(&commit.juror_id) {
            return Err(ModerationBallotRuntimeError::DuplicateCommit {
                case_id: key.case_id,
                round_id: key.round_id,
                juror_id: commit.juror_id,
            });
        }
        state
            .commits
            .insert(commit.juror_id.clone(), commit.clone());
        Ok(ModerationBallotCommitOutcome {
            accepted_commit: commit,
            committed_count: state.commits.len(),
            revealed_count: state.reveals.len(),
        })
    }

    pub(crate) fn submit_reveal(
        &mut self,
        reveal: SoraFsModerationBallotRevealV1,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotRevealOutcome, ModerationBallotRuntimeError> {
        reveal.validate()?;
        let key = ModerationBallotKey::from_context_and_round(&reveal.context, &reveal.round_id);
        let state = self.ballots.get_mut(&key).ok_or_else(|| {
            ModerationBallotRuntimeError::UnknownBallot {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
            }
        })?;
        validate_payload_scope(state, &reveal.context, &reveal.round_id)?;
        if now_unix_ms <= state.announcement.challenge_deadline_unix_ms {
            return Err(ModerationBallotRuntimeError::RevealWindowNotOpen { now_unix_ms });
        }
        if now_unix_ms > state.announcement.reveal_deadline_unix_ms {
            return Err(ModerationBallotRuntimeError::RevealWindowClosed { now_unix_ms });
        }
        ensure_eligible_juror(state, &reveal.juror_id)?;
        let commit = state.commits.get(&reveal.juror_id).ok_or_else(|| {
            ModerationBallotRuntimeError::MissingCommit {
                case_id: key.case_id.clone(),
                round_id: key.round_id.clone(),
                juror_id: reveal.juror_id.clone(),
            }
        })?;
        if state.reveals.contains_key(&reveal.juror_id) {
            return Err(ModerationBallotRuntimeError::DuplicateReveal {
                case_id: key.case_id,
                round_id: key.round_id,
                juror_id: reveal.juror_id,
            });
        }
        commit.verify_reveal(&reveal)?;
        state
            .reveals
            .insert(reveal.juror_id.clone(), reveal.clone());
        Ok(ModerationBallotRevealOutcome {
            accepted_reveal: reveal,
            committed_count: state.commits.len(),
            revealed_count: state.reveals.len(),
        })
    }

    pub(crate) fn tally_ballot(
        &mut self,
        case_id: &str,
        round_id: &str,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotTally, ModerationBallotRuntimeError> {
        let key = ModerationBallotKey::new(case_id, round_id);
        let state = self.ballots.get_mut(&key).ok_or_else(|| {
            ModerationBallotRuntimeError::UnknownBallot {
                case_id: case_id.to_owned(),
                round_id: round_id.to_owned(),
            }
        })?;
        if state.tally.is_some() {
            return Err(ModerationBallotRuntimeError::AlreadyTallied {
                case_id: case_id.to_owned(),
                round_id: round_id.to_owned(),
            });
        }
        let all_jurors_revealed = state.reveals.len() == state.announcement.juror_ids.len();
        if now_unix_ms < state.announcement.reveal_deadline_unix_ms && !all_jurors_revealed {
            return Err(ModerationBallotRuntimeError::TallyWindowOpen {
                reveal_deadline_unix_ms: state.announcement.reveal_deadline_unix_ms,
            });
        }
        if state.reveals.len() < usize::from(state.announcement.quorum) {
            return Err(ModerationBallotRuntimeError::QuorumNotMet {
                quorum: state.announcement.quorum,
                reveals: state.reveals.len(),
            });
        }

        let mut counts = ModerationVoteCounts::default();
        for reveal in state.reveals.values() {
            counts.increment(reveal.choice);
        }
        let winning_choice = counts.winning_choice();
        let tally = ModerationBallotTally {
            case_id: case_id.to_owned(),
            round_id: round_id.to_owned(),
            counts,
            votes_total: state.reveals.len() as u32,
            quorum: state.announcement.quorum,
            winning_choice,
            contested: winning_choice.is_none(),
            tallied_at_unix_ms: now_unix_ms,
        };
        state.tally = Some(tally.clone());
        Ok(tally)
    }

    pub(crate) fn ballot(&self, case_id: &str, round_id: &str) -> Option<ModerationBallotRecord> {
        self.ballots
            .get(&ModerationBallotKey::new(case_id, round_id))
            .map(ModerationBallotState::to_record)
    }

    pub(crate) fn ballots(&self) -> Vec<ModerationBallotRecord> {
        self.ballots
            .values()
            .map(ModerationBallotState::to_record)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ModerationBallotKey {
    case_id: String,
    round_id: String,
}

impl ModerationBallotKey {
    fn new(case_id: &str, round_id: &str) -> Self {
        Self {
            case_id: case_id.to_owned(),
            round_id: round_id.to_owned(),
        }
    }

    fn from_announcement(announcement: &ModerationBallotAnnouncement) -> Self {
        Self::new(&announcement.context.case_id, &announcement.round_id)
    }

    fn from_context_and_round(context: &SoraFsModerationBallotContextV1, round_id: &str) -> Self {
        Self::new(&context.case_id, round_id)
    }
}

#[derive(Debug, Clone)]
struct ModerationBallotState {
    announcement: ModerationBallotAnnouncement,
    commits: BTreeMap<String, SoraFsModerationBallotCommitV1>,
    reveals: BTreeMap<String, SoraFsModerationBallotRevealV1>,
    tally: Option<ModerationBallotTally>,
}

impl ModerationBallotState {
    fn to_record(&self) -> ModerationBallotRecord {
        ModerationBallotRecord {
            announcement: self.announcement.clone(),
            commits: self.commits.values().cloned().collect(),
            reveals: self.reveals.values().cloned().collect(),
            tally: self.tally.clone(),
        }
    }
}

fn validate_announcement(
    announcement: &ModerationBallotAnnouncement,
) -> Result<(), ModerationBallotRuntimeError> {
    announcement.context.validate()?;
    if announcement.round_id.trim().is_empty() {
        return Err(ModerationBallotRuntimeError::MissingRoundId);
    }
    if announcement.juror_ids.is_empty() {
        return Err(ModerationBallotRuntimeError::MissingJurors);
    }
    let mut seen = BTreeSet::new();
    for juror_id in &announcement.juror_ids {
        if juror_id.trim().is_empty() {
            return Err(ModerationBallotRuntimeError::BlankJurorId);
        }
        if !seen.insert(juror_id.clone()) {
            return Err(ModerationBallotRuntimeError::DuplicateJuror {
                juror_id: juror_id.clone(),
            });
        }
    }
    if announcement.quorum == 0 || usize::from(announcement.quorum) > announcement.juror_ids.len() {
        return Err(ModerationBallotRuntimeError::InvalidQuorum {
            quorum: announcement.quorum,
            roster_size: announcement.juror_ids.len(),
        });
    }
    let expected_hash =
        local_moderation_panel_roster_hash(&announcement.juror_ids, announcement.quorum);
    if announcement.context.panel_roster_hash != expected_hash {
        return Err(ModerationBallotRuntimeError::RosterHashMismatch);
    }
    if let Some(deposit) = &announcement.appeal_deposit {
        if deposit.escrow_id_hex.trim().is_empty() {
            return Err(ModerationBallotRuntimeError::MissingAppealDepositEscrowId);
        }
        if announcement
            .appeal_deposit_escrow_id_hex
            .as_deref()
            .is_some_and(|escrow_id| escrow_id != deposit.escrow_id_hex)
        {
            return Err(ModerationBallotRuntimeError::AppealDepositEscrowIdMismatch);
        }
    }
    if announcement.announced_at_unix_ms >= announcement.commit_deadline_unix_ms
        || announcement.commit_deadline_unix_ms >= announcement.challenge_deadline_unix_ms
        || announcement.challenge_deadline_unix_ms >= announcement.reveal_deadline_unix_ms
    {
        return Err(ModerationBallotRuntimeError::InvalidWindows);
    }
    Ok(())
}

fn validate_payload_scope(
    state: &ModerationBallotState,
    context: &SoraFsModerationBallotContextV1,
    round_id: &str,
) -> Result<(), ModerationBallotRuntimeError> {
    if state.announcement.context != *context {
        return Err(ModerationBallotRuntimeError::PayloadContextMismatch);
    }
    if state.announcement.round_id != round_id {
        return Err(ModerationBallotRuntimeError::PayloadRoundMismatch {
            expected: state.announcement.round_id.clone(),
            found: round_id.to_owned(),
        });
    }
    Ok(())
}

fn ensure_eligible_juror(
    state: &ModerationBallotState,
    juror_id: &str,
) -> Result<(), ModerationBallotRuntimeError> {
    if state
        .announcement
        .juror_ids
        .iter()
        .any(|eligible| eligible == juror_id)
    {
        return Ok(());
    }
    Err(ModerationBallotRuntimeError::IneligibleJuror {
        case_id: state.announcement.context.case_id.clone(),
        round_id: state.announcement.round_id.clone(),
        juror_id: juror_id.to_owned(),
    })
}
