//! Durable PoTR receipt tracking for the embedded storage node.
//!
//! A receipt is accepted only after both mandatory signatures have been
//! verified against the configured gateway key and a council-verified provider
//! admission. The exact final signed receipt is committed before any repair
//! callback. Missed-deadline callbacks use the signed receipt digest as their
//! exactly-once identity, so a crash after the callback but before the second
//! checkpoint is safe to replay.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write as _},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[cfg(unix)]
use std::os::unix::{
    fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    io::AsRawFd as _,
};

use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::{
    AdmissionRecord,
    potr::{PotrReceiptV1, PotrReceiptValidationError, PotrStatus},
    proof_stream::ProofStreamTier,
    repair::{
        REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, RepairCauseV1, RepairEvidenceV1,
        RepairLatencySlaCauseV1, RepairReportV1, RepairTicketId,
    },
};
use thiserror::Error;

/// Durable PoTR checkpoint schema version.
pub const POTR_TRACKER_CHECKPOINT_VERSION_V1: u8 = 1;
/// File containing the canonical PoTR tracker checkpoint.
pub const POTR_TRACKER_CHECKPOINT_FILE_NAME_V1: &str = "potr-receipts-state.to";
/// Default maximum number of retained signed receipts.
pub const POTR_TRACKER_DEFAULT_MAX_RECORDS_V1: usize = 1_024;
/// Default maximum canonical checkpoint size.
pub const POTR_TRACKER_DEFAULT_CHECKPOINT_MAX_BYTES_V1: u64 = 64 * 1024 * 1024;
/// Maximum canonical size of one signed receipt accepted by the tracker.
pub const POTR_RECEIPT_MAX_CANONICAL_BYTES_V1: usize = 64 * 1024;
/// Maximum number of receipts returned by one export operation.
pub const POTR_EXPORT_MAX_RECORDS_V1: usize = 1_000;

const CHECKPOINT_LOCK_FILE_NAME: &str = "potr-receipts-state.lock";
static CHECKPOINT_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static CHECKPOINT_PROCESS_LOCK: Mutex<()> = Mutex::new(());

#[cfg(unix)]
const LOCK_EXCLUSIVE_NONBLOCKING: std::os::raw::c_int = 2 | 4;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0x0002_0000 | 0x0008_0000;
#[cfg(any(target_os = "macos", target_os = "ios"))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0x0000_0100 | 0x0100_0000;
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))
))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0;

#[cfg(unix)]
unsafe extern "C" {
    fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int;
}

/// Error returned by the authoritative latency-repair scheduler.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{0}")]
pub struct PotrRepairHandoffError(pub String);

/// Exactly-once repair effect required for a missed-deadline PoTR receipt.
pub trait PotrLatencyRepairHandoff: Send + Sync + std::fmt::Debug {
    /// Enqueue the exact governed receipt for authoritative ledger submission.
    fn enqueue_proof_outcome(
        &self,
        source_identity: [u8; 32],
        receipt: &PotrReceiptV1,
        admission_envelope_digest: [u8; 32],
    ) -> Result<[u8; 32], PotrRepairHandoffError>;

    /// Enqueue a latency repair using the final signed receipt digest as identity.
    fn enqueue_latency_repair(
        &self,
        source_identity: [u8; 32],
        report: &RepairReportV1,
    ) -> Result<[u8; 32], PotrRepairHandoffError>;
}

/// Result of recording a final signed receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PotrRecordOutcome {
    /// A new signed receipt was durably inserted.
    Inserted(PotrReceiptStatusV1),
    /// The exact final signed receipt was already retained.
    Existing(PotrReceiptStatusV1),
}

impl PotrRecordOutcome {
    /// Return the durable receipt status.
    #[must_use]
    pub const fn status(self) -> PotrReceiptStatusV1 {
        match self {
            Self::Inserted(status) | Self::Existing(status) => status,
        }
    }
}

/// Compact bounded status for a retained final signed receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PotrReceiptStatusV1 {
    /// Durable insertion sequence.
    pub sequence: u64,
    /// Digest of the exact canonical signed receipt.
    pub receipt_digest: [u8; 32],
    /// Digest identifying the provider/manifest/request scope.
    pub request_scope_digest: [u8; 32],
    /// Receipt result.
    pub status: PotrStatus,
    /// Receipt recording time in milliseconds.
    pub recorded_at_ms: u64,
    /// Repair receipt, present after a missed-deadline handoff completes.
    #[norito(default)]
    pub repair_receipt_digest: Option<[u8; 32]>,
    /// Delivery receipt proving the authoritative ledger outbox accepted this exact receipt.
    #[norito(default)]
    pub proof_outcome_receipt_digest: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredPotrReceiptV1 {
    sequence: u64,
    receipt_digest: [u8; 32],
    request_scope_digest: [u8; 32],
    admission_envelope_digest: [u8; 32],
    gateway_public_key: [u8; 32],
    governed_provider_public_key: Vec<u8>,
    receipt: PotrReceiptV1,
    #[norito(default)]
    repair_report: Option<RepairReportV1>,
    #[norito(default)]
    proof_outcome_receipt_digest: Option<[u8; 32]>,
    #[norito(default)]
    repair_receipt_digest: Option<[u8; 32]>,
}

impl StoredPotrReceiptV1 {
    fn status(&self) -> PotrReceiptStatusV1 {
        PotrReceiptStatusV1 {
            sequence: self.sequence,
            receipt_digest: self.receipt_digest,
            request_scope_digest: self.request_scope_digest,
            status: self.receipt.status,
            recorded_at_ms: self.receipt.recorded_at_ms,
            repair_receipt_digest: self.repair_receipt_digest,
            proof_outcome_receipt_digest: self.proof_outcome_receipt_digest,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PotrTrackerCheckpointV1 {
    version: u8,
    next_sequence: u64,
    records: Vec<StoredPotrReceiptV1>,
}

#[derive(Debug, Clone)]
struct RuntimeState {
    next_sequence: u64,
    records: BTreeMap<[u8; 32], StoredPotrReceiptV1>,
    digest_index: BTreeMap<[u8; 32], [u8; 32]>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            next_sequence: 1,
            records: BTreeMap::new(),
            digest_index: BTreeMap::new(),
        }
    }
}

impl RuntimeState {
    fn checkpoint(&self) -> PotrTrackerCheckpointV1 {
        let mut records = self.records.values().cloned().collect::<Vec<_>>();
        records.sort_by_key(|record| record.sequence);
        PotrTrackerCheckpointV1 {
            version: POTR_TRACKER_CHECKPOINT_VERSION_V1,
            next_sequence: self.next_sequence,
            records,
        }
    }

    fn from_checkpoint(
        checkpoint: PotrTrackerCheckpointV1,
        max_records: usize,
    ) -> Result<Self, PotrTrackerError> {
        validate_checkpoint(&checkpoint, max_records)?;
        let mut records = BTreeMap::new();
        let mut digest_index = BTreeMap::new();
        for record in checkpoint.records {
            let scope = record.request_scope_digest;
            let digest = record.receipt_digest;
            if records.insert(scope, record).is_some()
                || digest_index.insert(digest, scope).is_some()
            {
                return Err(PotrTrackerError::InvalidCheckpoint(
                    "duplicate PoTR request scope or receipt digest".to_owned(),
                ));
            }
        }
        Ok(Self {
            next_sequence: checkpoint.next_sequence,
            records,
            digest_index,
        })
    }
}

#[derive(Debug)]
struct DurableState {
    runtime: RuntimeState,
    fingerprint: Option<[u8; 32]>,
    durability_failure: Option<String>,
}

/// Durable, bounded tracker for final signed PoTR receipts.
#[derive(Debug, Clone)]
pub struct PotrTracker {
    max_records: usize,
    state: Arc<Mutex<DurableState>>,
    checkpoint_store: Option<Arc<PotrCheckpointStore>>,
}

impl Default for PotrTracker {
    fn default() -> Self {
        Self::in_memory(POTR_TRACKER_DEFAULT_MAX_RECORDS_V1)
            .expect("default PoTR tracker policy is valid")
    }
}

impl PotrTracker {
    /// Construct a bounded non-persistent tracker for focused composition tests.
    pub fn in_memory(max_records: usize) -> Result<Self, PotrTrackerError> {
        validate_policy(max_records, POTR_TRACKER_DEFAULT_CHECKPOINT_MAX_BYTES_V1)?;
        Ok(Self {
            max_records,
            state: Arc::new(Mutex::new(DurableState {
                runtime: RuntimeState::default(),
                fingerprint: None,
                durability_failure: None,
            })),
            checkpoint_store: None,
        })
    }

    /// Open or create a durable PoTR tracker below `state_dir`.
    pub fn open(
        state_dir: &Path,
        max_records: usize,
        checkpoint_max_bytes: u64,
    ) -> Result<Self, PotrTrackerError> {
        validate_policy(max_records, checkpoint_max_bytes)?;
        let store = Arc::new(PotrCheckpointStore::new(state_dir, checkpoint_max_bytes)?);
        let (checkpoint, fingerprint) = store.load(max_records)?;
        let runtime = checkpoint.map_or_else(
            || Ok(RuntimeState::default()),
            |checkpoint| RuntimeState::from_checkpoint(checkpoint, max_records),
        )?;
        Ok(Self {
            max_records,
            state: Arc::new(Mutex::new(DurableState {
                runtime,
                fingerprint,
                durability_failure: None,
            })),
            checkpoint_store: Some(store),
        })
    }

    /// Validate, atomically persist, and complete any authoritative latency repair.
    pub fn record_receipt(
        &self,
        receipt: PotrReceiptV1,
        gateway_public_key: &[u8; 32],
        admission: &AdmissionRecord,
        repair: &dyn PotrLatencyRepairHandoff,
    ) -> Result<PotrRecordOutcome, PotrTrackerError> {
        receipt.validate_with_governed_keys(gateway_public_key, admission)?;
        let canonical_receipt = receipt.signed_receipt_bytes()?;
        if canonical_receipt.len() > POTR_RECEIPT_MAX_CANONICAL_BYTES_V1 {
            return Err(PotrTrackerError::ReceiptTooLarge {
                size: canonical_receipt.len(),
                limit: POTR_RECEIPT_MAX_CANONICAL_BYTES_V1,
            });
        }
        let receipt_digest = receipt.signed_receipt_digest()?;
        let request_scope_digest = receipt.request_scope_digest()?;
        let governed_provider_public_key = admission
            .potr_mldsa_key()
            .ok_or(PotrReceiptValidationError::ProviderKeyUnavailable)?
            .to_vec();
        let repair_report = build_latency_repair_report(&receipt, receipt_digest)?;

        let mut durable = self.lock_state()?;
        let existed = if let Some(existing) = durable.runtime.records.get(&request_scope_digest) {
            if existing.receipt_digest != receipt_digest || existing.receipt != receipt {
                return Err(PotrTrackerError::RequestScopeConflict {
                    request_scope_digest,
                });
            }
            true
        } else {
            if durable.runtime.digest_index.contains_key(&receipt_digest) {
                return Err(PotrTrackerError::ReceiptDigestConflict { receipt_digest });
            }
            if durable.runtime.records.len() >= self.max_records {
                return Err(PotrTrackerError::RetentionExhausted {
                    limit: self.max_records,
                });
            }
            let sequence = durable.runtime.next_sequence;
            let next_sequence = sequence
                .checked_add(1)
                .ok_or(PotrTrackerError::SequenceOverflow)?;
            let record = StoredPotrReceiptV1 {
                sequence,
                receipt_digest,
                request_scope_digest,
                admission_envelope_digest: *admission.envelope_digest(),
                gateway_public_key: *gateway_public_key,
                governed_provider_public_key,
                receipt,
                repair_report,
                proof_outcome_receipt_digest: None,
                repair_receipt_digest: None,
            };
            validate_record(&record)?;
            let mut candidate = durable.runtime.clone();
            candidate.next_sequence = next_sequence;
            candidate.records.insert(request_scope_digest, record);
            candidate
                .digest_index
                .insert(receipt_digest, request_scope_digest);
            self.commit_candidate(&mut durable, candidate)?;
            false
        };

        self.complete_terminal_handoffs_locked(&mut durable, request_scope_digest, repair)?;
        let status = durable
            .runtime
            .records
            .get(&request_scope_digest)
            .ok_or_else(|| {
                PotrTrackerError::InvalidCheckpoint(
                    "PoTR receipt disappeared during terminal handoff".to_owned(),
                )
            })?
            .status();
        Ok(if existed {
            PotrRecordOutcome::Existing(status)
        } else {
            PotrRecordOutcome::Inserted(status)
        })
    }

    /// Retry every persisted ledger or missed-deadline repair handoff.
    ///
    /// This is intended for startup recovery after the repair manager is ready.
    pub fn resume_terminal_handoffs(
        &self,
        repair: &dyn PotrLatencyRepairHandoff,
    ) -> Result<usize, PotrTrackerError> {
        let mut durable = self.lock_state()?;
        let pending = durable
            .runtime
            .records
            .iter()
            .filter_map(|(scope, record)| {
                (record.proof_outcome_receipt_digest.is_none()
                    || record.repair_report.is_some() && record.repair_receipt_digest.is_none())
                .then_some(*scope)
            })
            .collect::<Vec<_>>();
        for scope in &pending {
            self.complete_terminal_handoffs_locked(&mut durable, *scope, repair)?;
        }
        Ok(pending.len())
    }

    /// Return the status for one exact final signed receipt digest.
    pub fn status(
        &self,
        receipt_digest: &[u8; 32],
    ) -> Result<Option<PotrReceiptStatusV1>, PotrTrackerError> {
        let durable = self.lock_state()?;
        Ok(durable
            .runtime
            .digest_index
            .get(receipt_digest)
            .and_then(|scope| durable.runtime.records.get(scope))
            .map(StoredPotrReceiptV1::status))
    }

    /// Return a bounded sequence-ordered status export.
    pub fn export_statuses(
        &self,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<PotrReceiptStatusV1>, PotrTrackerError> {
        if limit == 0 || limit > POTR_EXPORT_MAX_RECORDS_V1 {
            return Err(PotrTrackerError::InvalidExportLimit {
                limit,
                max: POTR_EXPORT_MAX_RECORDS_V1,
            });
        }
        let durable = self.lock_state()?;
        let mut statuses = durable
            .runtime
            .records
            .values()
            .filter(|record| record.sequence > after_sequence)
            .map(StoredPotrReceiptV1::status)
            .collect::<Vec<_>>();
        statuses.sort_by_key(|status| status.sequence);
        statuses.truncate(limit);
        Ok(statuses)
    }

    /// Return a bounded sequence-ordered export of exact signed receipts.
    pub fn export_receipts(
        &self,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<PotrReceiptV1>, PotrTrackerError> {
        if limit == 0 || limit > POTR_EXPORT_MAX_RECORDS_V1 {
            return Err(PotrTrackerError::InvalidExportLimit {
                limit,
                max: POTR_EXPORT_MAX_RECORDS_V1,
            });
        }
        let durable = self.lock_state()?;
        let mut records = durable
            .runtime
            .records
            .values()
            .filter(|record| record.sequence > after_sequence)
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.sequence);
        Ok(records
            .into_iter()
            .take(limit)
            .map(|record| record.receipt.clone())
            .collect())
    }

    /// Return receipts matching manifest, provider, and optional storage tier.
    pub fn receipts_for(
        &self,
        manifest_digest: &[u8; 32],
        provider_id: &[u8; 32],
        tier: Option<ProofStreamTier>,
    ) -> Result<Vec<PotrReceiptV1>, PotrTrackerError> {
        let durable = self.lock_state()?;
        let mut records = durable
            .runtime
            .records
            .values()
            .filter(|record| {
                record.receipt.manifest_digest == *manifest_digest
                    && record.receipt.provider_id == *provider_id
                    && tier.is_none_or(|filter| record.receipt.tier == filter)
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.sequence);
        Ok(records
            .into_iter()
            .map(|record| record.receipt.clone())
            .collect())
    }

    fn complete_terminal_handoffs_locked(
        &self,
        durable: &mut DurableState,
        request_scope_digest: [u8; 32],
        repair: &dyn PotrLatencyRepairHandoff,
    ) -> Result<(), PotrTrackerError> {
        let Some(record) = durable.runtime.records.get(&request_scope_digest) else {
            return Err(PotrTrackerError::InvalidCheckpoint(
                "missing PoTR receipt during repair handoff".to_owned(),
            ));
        };
        if record.proof_outcome_receipt_digest.is_none() {
            let receipt_digest = record.receipt_digest;
            let proof_outcome_receipt = repair
                .enqueue_proof_outcome(
                    receipt_digest,
                    &record.receipt,
                    record.admission_envelope_digest,
                )
                .map_err(PotrTrackerError::ProofOutcomeHandoff)?;
            if proof_outcome_receipt == [0; 32] {
                return Err(PotrTrackerError::ZeroProofOutcomeReceipt);
            }
            let mut candidate = durable.runtime.clone();
            candidate
                .records
                .get_mut(&request_scope_digest)
                .ok_or_else(|| {
                    PotrTrackerError::InvalidCheckpoint(
                        "missing PoTR receipt while committing proof-outcome handoff".to_owned(),
                    )
                })?
                .proof_outcome_receipt_digest = Some(proof_outcome_receipt);
            self.commit_candidate(durable, candidate)?;
        }
        let record = durable
            .runtime
            .records
            .get(&request_scope_digest)
            .ok_or_else(|| {
                PotrTrackerError::InvalidCheckpoint(
                    "missing PoTR receipt after proof-outcome handoff".to_owned(),
                )
            })?;
        let Some(report) = record.repair_report.clone() else {
            return Ok(());
        };
        if record.repair_receipt_digest.is_some() {
            return Ok(());
        }
        let receipt_digest = record.receipt_digest;
        let repair_receipt = repair
            .enqueue_latency_repair(receipt_digest, &report)
            .map_err(PotrTrackerError::RepairHandoff)?;
        if repair_receipt == [0; 32] {
            return Err(PotrTrackerError::ZeroRepairReceipt);
        }
        let mut candidate = durable.runtime.clone();
        let candidate_record = candidate
            .records
            .get_mut(&request_scope_digest)
            .ok_or_else(|| {
                PotrTrackerError::InvalidCheckpoint(
                    "missing PoTR receipt while committing repair receipt".to_owned(),
                )
            })?;
        candidate_record.repair_receipt_digest = Some(repair_receipt);
        self.commit_candidate(durable, candidate)
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, DurableState>, PotrTrackerError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| PotrTrackerError::RuntimePoisoned)?;
        if let Some(reason) = guard.durability_failure.as_ref() {
            return Err(PotrTrackerError::DurabilityPoisoned(reason.clone()));
        }
        Ok(guard)
    }

    fn commit_candidate(
        &self,
        durable: &mut DurableState,
        candidate: RuntimeState,
    ) -> Result<(), PotrTrackerError> {
        validate_checkpoint(&candidate.checkpoint(), self.max_records)?;
        if let Some(store) = self.checkpoint_store.as_ref() {
            match store.commit(&candidate.checkpoint(), durable.fingerprint) {
                Ok(fingerprint) => durable.fingerprint = Some(fingerprint),
                Err(error) => {
                    if matches!(&error, PotrTrackerError::CheckpointDurabilityUncertain(_)) {
                        durable.durability_failure = Some(error.to_string());
                    }
                    return Err(error);
                }
            }
        }
        durable.runtime = candidate;
        Ok(())
    }
}

fn build_latency_repair_report(
    receipt: &PotrReceiptV1,
    receipt_digest: [u8; 32],
) -> Result<Option<RepairReportV1>, PotrTrackerError> {
    if receipt.status != PotrStatus::MissedDeadline {
        return Ok(None);
    }
    let submitted_at_unix = receipt.recorded_at_ms / 1_000;
    let report = RepairReportV1 {
        version: REPAIR_REPORT_VERSION_V1,
        ticket_id: RepairTicketId(format!("POTR-{}", hex::encode_upper(receipt_digest))),
        auditor_account: "sorafs-potr-runtime".to_owned(),
        submitted_at_unix,
        evidence: RepairEvidenceV1 {
            version: REPAIR_EVIDENCE_VERSION_V1,
            manifest_digest: receipt.manifest_digest,
            provider_id: receipt.provider_id,
            por_history_id: None,
            cause: RepairCauseV1::LatencySla(RepairLatencySlaCauseV1 {
                observed_latency_ms: receipt.latency_ms,
                receipt_digest: Some(receipt_digest),
            }),
            evidence_json: None,
            notes: Some("potr_missed_deadline".to_owned()),
        },
        notes: None,
    };
    report
        .validate()
        .map_err(|error| PotrTrackerError::RepairReport(error.to_string()))?;
    Ok(Some(report))
}

fn validate_policy(max_records: usize, checkpoint_max_bytes: u64) -> Result<(), PotrTrackerError> {
    if max_records == 0 {
        return Err(PotrTrackerError::InvalidPolicy(
            "PoTR receipt retention limit must be positive".to_owned(),
        ));
    }
    if checkpoint_max_bytes < POTR_RECEIPT_MAX_CANONICAL_BYTES_V1 as u64 {
        return Err(PotrTrackerError::InvalidPolicy(
            "PoTR checkpoint ceiling must fit one maximum receipt".to_owned(),
        ));
    }
    Ok(())
}

fn validate_checkpoint(
    checkpoint: &PotrTrackerCheckpointV1,
    max_records: usize,
) -> Result<(), PotrTrackerError> {
    if checkpoint.version != POTR_TRACKER_CHECKPOINT_VERSION_V1 || checkpoint.next_sequence == 0 {
        return Err(PotrTrackerError::InvalidCheckpoint(
            "unsupported PoTR checkpoint version or zero next sequence".to_owned(),
        ));
    }
    if checkpoint.records.len() > max_records {
        return Err(PotrTrackerError::InvalidCheckpoint(
            "PoTR checkpoint exceeds its configured retention bound".to_owned(),
        ));
    }
    let mut scopes = BTreeSet::new();
    let mut digests = BTreeSet::new();
    let mut previous_sequence = None;
    for record in &checkpoint.records {
        validate_record(record)?;
        if !scopes.insert(record.request_scope_digest)
            || !digests.insert(record.receipt_digest)
            || previous_sequence.is_some_and(|sequence| sequence >= record.sequence)
        {
            return Err(PotrTrackerError::InvalidCheckpoint(
                "PoTR records must have unique identities and increasing sequences".to_owned(),
            ));
        }
        previous_sequence = Some(record.sequence);
    }
    if previous_sequence.is_some_and(|sequence| sequence >= checkpoint.next_sequence) {
        return Err(PotrTrackerError::InvalidCheckpoint(
            "PoTR next sequence does not follow retained records".to_owned(),
        ));
    }
    Ok(())
}

fn validate_record(record: &StoredPotrReceiptV1) -> Result<(), PotrTrackerError> {
    record.receipt.validate()?;
    if record.sequence == 0
        || record.receipt_digest == [0; 32]
        || record.request_scope_digest == [0; 32]
        || record.admission_envelope_digest == [0; 32]
        || record.gateway_public_key == [0; 32]
        || record.governed_provider_public_key.is_empty()
        || record.receipt.signed_receipt_digest()? != record.receipt_digest
        || record.receipt.request_scope_digest()? != record.request_scope_digest
        || record
            .receipt
            .gateway_signature
            .as_ref()
            .is_none_or(|signature| signature.public_key.as_slice() != record.gateway_public_key)
        || record
            .receipt
            .provider_signature
            .as_ref()
            .is_none_or(|signature| {
                signature.public_key.as_slice() != record.governed_provider_public_key
            })
        || (record.receipt.status == PotrStatus::MissedDeadline) != record.repair_report.is_some()
        || record.repair_report.is_none() && record.repair_receipt_digest.is_some()
        || record.proof_outcome_receipt_digest == Some([0; 32])
        || record.repair_receipt_digest.is_some() && record.proof_outcome_receipt_digest.is_none()
    {
        return Err(PotrTrackerError::InvalidCheckpoint(
            "persisted PoTR receipt binding is inconsistent".to_owned(),
        ));
    }
    if let Some(report) = record.repair_report.as_ref() {
        report
            .validate()
            .map_err(|error| PotrTrackerError::RepairReport(error.to_string()))?;
        let RepairCauseV1::LatencySla(cause) = &report.evidence.cause else {
            return Err(PotrTrackerError::InvalidCheckpoint(
                "PoTR repair report is not a latency-SLA cause".to_owned(),
            ));
        };
        if report.evidence.manifest_digest != record.receipt.manifest_digest
            || report.evidence.provider_id != record.receipt.provider_id
            || cause.observed_latency_ms != record.receipt.latency_ms
            || cause.receipt_digest != Some(record.receipt_digest)
            || record.repair_receipt_digest == Some([0; 32])
        {
            return Err(PotrTrackerError::InvalidCheckpoint(
                "PoTR latency repair does not bind the signed receipt".to_owned(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
struct PotrCheckpointStore {
    root: PathBuf,
    checkpoint_path: PathBuf,
    lock_path: PathBuf,
    checkpoint_max_bytes: u64,
}

impl PotrCheckpointStore {
    fn new(root: &Path, checkpoint_max_bytes: u64) -> Result<Self, PotrTrackerError> {
        ensure_private_state_directory(root)?;
        Ok(Self {
            root: root.to_path_buf(),
            checkpoint_path: root.join(POTR_TRACKER_CHECKPOINT_FILE_NAME_V1),
            lock_path: root.join(CHECKPOINT_LOCK_FILE_NAME),
            checkpoint_max_bytes,
        })
    }

    fn load(
        &self,
        max_records: usize,
    ) -> Result<(Option<PotrTrackerCheckpointV1>, Option<[u8; 32]>), PotrTrackerError> {
        let _writer = CheckpointWriterGuard::acquire(&self.lock_path)?;
        let Some(bytes) = read_checkpoint_bytes(&self.checkpoint_path, self.checkpoint_max_bytes)?
        else {
            return Ok((None, None));
        };
        let fingerprint = *blake3::hash(&bytes).as_bytes();
        let checkpoint: PotrTrackerCheckpointV1 = norito::decode_from_bytes_with_limits(
            &bytes,
            checkpoint_decode_limits(self.checkpoint_max_bytes),
        )
        .map_err(|error| {
            PotrTrackerError::InvalidCheckpoint(format!(
                "decode canonical PoTR checkpoint: {error}"
            ))
        })?;
        let canonical = norito::to_bytes(&checkpoint).map_err(|error| {
            PotrTrackerError::CanonicalEncoding(format!("re-encode PoTR checkpoint: {error}"))
        })?;
        if canonical != bytes {
            return Err(PotrTrackerError::InvalidCheckpoint(
                "PoTR checkpoint is not canonically encoded".to_owned(),
            ));
        }
        validate_checkpoint(&checkpoint, max_records)?;
        Ok((Some(checkpoint), Some(fingerprint)))
    }

    fn commit(
        &self,
        checkpoint: &PotrTrackerCheckpointV1,
        expected_fingerprint: Option<[u8; 32]>,
    ) -> Result<[u8; 32], PotrTrackerError> {
        let bytes = norito::to_bytes(checkpoint).map_err(|error| {
            PotrTrackerError::CanonicalEncoding(format!("encode PoTR checkpoint: {error}"))
        })?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.checkpoint_max_bytes {
            return Err(PotrTrackerError::CheckpointTooLarge {
                size: bytes.len(),
                limit: usize::try_from(self.checkpoint_max_bytes).unwrap_or(usize::MAX),
            });
        }
        let _writer = CheckpointWriterGuard::acquire(&self.lock_path)?;
        let current = read_checkpoint_bytes(&self.checkpoint_path, self.checkpoint_max_bytes)?;
        let current_fingerprint = current
            .as_deref()
            .map(blake3::hash)
            .map(|digest| *digest.as_bytes());
        if current_fingerprint != expected_fingerprint {
            return Err(PotrTrackerError::StaleCheckpoint);
        }
        let temp_path = self.root.join(format!(
            ".{POTR_TRACKER_CHECKPOINT_FILE_NAME_V1}.{}.{}.tmp",
            std::process::id(),
            CHECKPOINT_TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let write_result = write_checkpoint_temp(&temp_path, &bytes).and_then(|()| {
            let latest = read_checkpoint_bytes(&self.checkpoint_path, self.checkpoint_max_bytes)?;
            let latest_fingerprint = latest
                .as_deref()
                .map(blake3::hash)
                .map(|digest| *digest.as_bytes());
            if latest_fingerprint != expected_fingerprint {
                return Err(PotrTrackerError::StaleCheckpoint);
            }
            fs::rename(&temp_path, &self.checkpoint_path).map_err(|error| {
                PotrTrackerError::CheckpointIo(format!(
                    "rename PoTR checkpoint into place: {error}"
                ))
            })?;
            sync_directory(&self.root)
                .map_err(|error| PotrTrackerError::CheckpointDurabilityUncertain(error.to_string()))
        });
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result?;
        let persisted = read_checkpoint_bytes(&self.checkpoint_path, self.checkpoint_max_bytes)
            .map_err(|error| {
                PotrTrackerError::CheckpointDurabilityUncertain(format!(
                    "could not verify PoTR checkpoint after atomic rename: {error}"
                ))
            })?
            .ok_or_else(|| {
                PotrTrackerError::CheckpointDurabilityUncertain(
                    "PoTR checkpoint disappeared after atomic rename".to_owned(),
                )
            })?;
        if persisted != bytes {
            return Err(PotrTrackerError::CheckpointDurabilityUncertain(
                "PoTR checkpoint bytes changed after atomic rename".to_owned(),
            ));
        }
        Ok(*blake3::hash(&bytes).as_bytes())
    }
}

struct CheckpointWriterGuard {
    _process_guard: std::sync::MutexGuard<'static, ()>,
    _file: File,
}

impl CheckpointWriterGuard {
    fn acquire(path: &Path) -> Result<Self, PotrTrackerError> {
        let process_guard = CHECKPOINT_PROCESS_LOCK
            .try_lock()
            .map_err(|_| PotrTrackerError::CheckpointBusy)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            options.mode(0o600);
            options.custom_flags(SAFE_OPEN_FLAGS);
        }
        let file = options.open(path).map_err(|error| {
            PotrTrackerError::CheckpointIo(format!("open PoTR checkpoint writer lock: {error}"))
        })?;
        validate_open_regular_file(path, &file, 0, true)?;
        #[cfg(unix)]
        {
            // SAFETY: `flock` borrows the live lock-file descriptor. The guard
            // retains ownership of the descriptor for its entire lifetime.
            if unsafe { flock(file.as_raw_fd(), LOCK_EXCLUSIVE_NONBLOCKING) } != 0 {
                return Err(PotrTrackerError::CheckpointBusy);
            }
        }
        Ok(Self {
            _process_guard: process_guard,
            _file: file,
        })
    }
}

fn ensure_private_state_directory(path: &Path) -> Result<(), PotrTrackerError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(PotrTrackerError::CheckpointIo(format!(
                    "PoTR state root {path:?} must be a real directory"
                )));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            builder.mode(0o700);
            builder.create(path).map_err(|error| {
                PotrTrackerError::CheckpointIo(format!("create PoTR state root {path:?}: {error}"))
            })?;
        }
        Err(error) => {
            return Err(PotrTrackerError::CheckpointIo(format!(
                "inspect PoTR state root {path:?}: {error}"
            )));
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        PotrTrackerError::CheckpointIo(format!("reinspect PoTR state root {path:?}: {error}"))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PotrTrackerError::CheckpointIo(format!(
            "PoTR state root {path:?} changed during initialization"
        )));
    }
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
        PotrTrackerError::CheckpointIo(format!("set private PoTR state-root permissions: {error}"))
    })?;
    Ok(())
}

fn read_checkpoint_bytes(path: &Path, max_bytes: u64) -> Result<Option<Vec<u8>>, PotrTrackerError> {
    let path_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(PotrTrackerError::CheckpointIo(format!(
                "inspect PoTR checkpoint: {error}"
            )));
        }
    };
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(PotrTrackerError::CheckpointIo(
            "PoTR checkpoint must be a non-symlink regular file".to_owned(),
        ));
    }
    #[cfg(unix)]
    if path_metadata.nlink() != 1 {
        return Err(PotrTrackerError::CheckpointIo(
            "PoTR checkpoint must not have hard links".to_owned(),
        ));
    }
    if path_metadata.len() > max_bytes {
        return Err(PotrTrackerError::CheckpointTooLarge {
            size: usize::try_from(path_metadata.len()).unwrap_or(usize::MAX),
            limit: usize::try_from(max_bytes).unwrap_or(usize::MAX),
        });
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(SAFE_OPEN_FLAGS);
    let mut file = options.open(path).map_err(|error| {
        PotrTrackerError::CheckpointIo(format!("open PoTR checkpoint: {error}"))
    })?;
    validate_open_regular_file(path, &file, max_bytes, false)?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(path_metadata.len())
            .unwrap_or(usize::MAX)
            .min(usize::try_from(max_bytes).unwrap_or(usize::MAX)),
    );
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            PotrTrackerError::CheckpointIo(format!("read PoTR checkpoint: {error}"))
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(PotrTrackerError::CheckpointTooLarge {
            size: bytes.len(),
            limit: usize::try_from(max_bytes).unwrap_or(usize::MAX),
        });
    }
    let reopened = fs::symlink_metadata(path).map_err(|error| {
        PotrTrackerError::CheckpointIo(format!("reinspect PoTR checkpoint after read: {error}"))
    })?;
    #[cfg(unix)]
    if reopened.dev() != path_metadata.dev()
        || reopened.ino() != path_metadata.ino()
        || reopened.nlink() != 1
    {
        return Err(PotrTrackerError::CheckpointIo(
            "PoTR checkpoint changed during bounded read".to_owned(),
        ));
    }
    Ok(Some(bytes))
}

fn validate_open_regular_file(
    path: &Path,
    file: &File,
    max_bytes: u64,
    allow_empty_lock: bool,
) -> Result<(), PotrTrackerError> {
    let metadata = file.metadata().map_err(|error| {
        PotrTrackerError::CheckpointIo(format!("inspect opened PoTR state file {path:?}: {error}"))
    })?;
    if !metadata.is_file() || (!allow_empty_lock && metadata.len() > max_bytes) {
        return Err(PotrTrackerError::CheckpointIo(format!(
            "opened PoTR state path {path:?} is unsafe or oversized"
        )));
    }
    #[cfg(unix)]
    {
        let path_metadata = fs::symlink_metadata(path).map_err(|error| {
            PotrTrackerError::CheckpointIo(format!("inspect PoTR state path {path:?}: {error}"))
        })?;
        if path_metadata.file_type().is_symlink()
            || path_metadata.dev() != metadata.dev()
            || path_metadata.ino() != metadata.ino()
            || metadata.nlink() != 1
        {
            return Err(PotrTrackerError::CheckpointIo(format!(
                "PoTR state path {path:?} changed or has hard links"
            )));
        }
    }
    Ok(())
}

fn write_checkpoint_temp(path: &Path, bytes: &[u8]) -> Result<(), PotrTrackerError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
        options.custom_flags(SAFE_OPEN_FLAGS);
    }
    let mut file = options.open(path).map_err(|error| {
        PotrTrackerError::CheckpointIo(format!(
            "create private PoTR checkpoint temporary file: {error}"
        ))
    })?;
    validate_open_regular_file(path, &file, u64::MAX, true)?;
    file.write_all(bytes).map_err(|error| {
        PotrTrackerError::CheckpointIo(format!("write PoTR checkpoint temporary file: {error}"))
    })?;
    file.sync_all().map_err(|error| {
        PotrTrackerError::CheckpointIo(format!("sync PoTR checkpoint temporary file: {error}"))
    })?;
    #[cfg(unix)]
    if file
        .metadata()
        .map_err(|error| {
            PotrTrackerError::CheckpointIo(format!(
                "reinspect PoTR checkpoint temporary file: {error}"
            ))
        })?
        .nlink()
        != 1
    {
        return Err(PotrTrackerError::CheckpointIo(
            "PoTR checkpoint temporary file acquired a hard link".to_owned(),
        ));
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), PotrTrackerError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            PotrTrackerError::CheckpointIo(format!("sync PoTR checkpoint directory: {error}"))
        })
}

fn checkpoint_decode_limits(max_bytes: u64) -> norito::DecodeLimits {
    let max_bytes = usize::try_from(max_bytes).unwrap_or(usize::MAX);
    norito::DecodeLimits::new(
        max_bytes.max(1),
        max_bytes,
        max_bytes,
        max_bytes.saturating_mul(2),
        64,
    )
}

/// Errors returned by the durable PoTR receipt protocol.
#[derive(Debug, Error)]
pub enum PotrTrackerError {
    /// Receipt shape, signature, or governed key binding failed.
    #[error("invalid final signed PoTR receipt: {0}")]
    Receipt(#[from] PotrReceiptValidationError),
    /// Tracker bounds are inconsistent.
    #[error("invalid PoTR tracker policy: {0}")]
    InvalidPolicy(String),
    /// Final signed receipt exceeded the protocol ceiling.
    #[error("PoTR signed receipt is {size} bytes, exceeding limit {limit}")]
    ReceiptTooLarge {
        /// Canonically encoded receipt size.
        size: usize,
        /// Maximum accepted receipt size.
        limit: usize,
    },
    /// The checkpoint exceeded its configured ceiling.
    #[error("PoTR checkpoint is {size} bytes, exceeding limit {limit}")]
    CheckpointTooLarge {
        /// Observed checkpoint size.
        size: usize,
        /// Maximum accepted checkpoint size.
        limit: usize,
    },
    /// The request identity is already occupied by a different signed receipt.
    #[error("PoTR request scope {request_scope_digest:02x?} already has a different receipt")]
    RequestScopeConflict {
        /// Digest of the already-occupied request scope.
        request_scope_digest: [u8; 32],
    },
    /// A receipt digest unexpectedly mapped to a different request scope.
    #[error("PoTR receipt digest {receipt_digest:02x?} conflicts with retained state")]
    ReceiptDigestConflict {
        /// Digest that collided with retained state.
        receipt_digest: [u8; 32],
    },
    /// Receipt retention is full; signed audit evidence is never silently evicted.
    #[error("PoTR receipt retention exhausted at {limit} records")]
    RetentionExhausted {
        /// Configured maximum number of retained receipts.
        limit: usize,
    },
    /// Durable insertion sequence overflowed.
    #[error("PoTR receipt insertion sequence overflowed")]
    SequenceOverflow,
    /// Export limit is zero or above the protocol cap.
    #[error("PoTR export limit {limit} must be between 1 and {max}")]
    InvalidExportLimit {
        /// Requested export limit.
        limit: usize,
        /// Protocol maximum export limit.
        max: usize,
    },
    /// Latency repair report construction failed.
    #[error("invalid PoTR latency repair report: {0}")]
    RepairReport(String),
    /// The authoritative repair callback failed. The pending state remains durable.
    #[error("PoTR latency repair handoff failed: {0}")]
    RepairHandoff(#[source] PotrRepairHandoffError),
    /// The authoritative ledger-delivery callback failed. The signed receipt remains durable.
    #[error("PoTR proof-outcome ledger handoff failed: {0}")]
    ProofOutcomeHandoff(#[source] PotrRepairHandoffError),
    /// A repair callback returned an inert receipt.
    #[error("PoTR latency repair handoff returned an all-zero receipt")]
    ZeroRepairReceipt,
    /// A ledger-delivery callback returned an inert receipt.
    #[error("PoTR proof-outcome ledger handoff returned an all-zero receipt")]
    ZeroProofOutcomeReceipt,
    /// Canonical checkpoint encoding failed.
    #[error("PoTR canonical encoding failed: {0}")]
    CanonicalEncoding(String),
    /// Persisted state is corrupt or internally inconsistent.
    #[error("invalid PoTR tracker checkpoint: {0}")]
    InvalidCheckpoint(String),
    /// Durable checkpoint path is unsafe or inaccessible.
    #[error("PoTR tracker checkpoint I/O failed: {0}")]
    CheckpointIo(String),
    /// Another runtime changed the checkpoint after this instance loaded it.
    #[error("PoTR tracker checkpoint changed concurrently; stale writer rejected")]
    StaleCheckpoint,
    /// Another process currently owns the checkpoint writer lock.
    #[error("PoTR tracker checkpoint writer is busy")]
    CheckpointBusy,
    /// Atomic rename became visible but parent-directory durability is uncertain.
    #[error("PoTR tracker checkpoint durability is uncertain: {0}")]
    CheckpointDurabilityUncertain(String),
    /// The runtime is poisoned after an uncertain durable commit.
    #[error("PoTR tracker durability is poisoned: {0}")]
    DurabilityPoisoned(String),
    /// The in-process runtime lock was poisoned.
    #[error("PoTR tracker runtime lock is poisoned")]
    RuntimePoisoned,
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{
            Arc, Barrier,
            atomic::{AtomicU64, Ordering},
        },
        thread,
    };

    use ed25519_dalek::{Signer as _, SigningKey};
    use iroha_crypto::{Algorithm, KeyPair};
    use sorafs_manifest::{
        AdvertEndpoint, AvailabilityTier, CapabilityTlv, CapabilityType, CouncilSignature,
        EndpointAdmissionV1, EndpointAttestationKind, EndpointAttestationV1, EndpointKind,
        PathDiversityPolicy, ProviderAdmissionCouncilPolicy, ProviderAdmissionEnvelopeV1,
        ProviderAdmissionProposalV1, ProviderAdvertBodyV1, ProviderVrfPublicKeyV1, QosHints,
        StakePointer, compute_advert_body_digest, compute_envelope_authorization_digest,
        compute_proposal_digest,
        potr::{POTR_RECEIPT_VERSION_V1, PotrStatus, sign_potr_receipt_v1},
    };
    use tempfile::TempDir;

    use crate::{NodeHandle, config::StorageConfig};

    use super::*;

    const PROVIDER_ID: [u8; 32] = [0x22; 32];
    const MANIFEST_DIGEST: [u8; 32] = [0x11; 32];

    #[derive(Debug, Default)]
    struct RecordingRepair {
        proof_failures_remaining: AtomicU64,
        failures_remaining: AtomicU64,
        proof_outcomes: Mutex<BTreeMap<[u8; 32], (PotrReceiptV1, [u8; 32], [u8; 32])>>,
        reports: Mutex<BTreeMap<[u8; 32], (RepairReportV1, [u8; 32])>>,
    }

    impl RecordingRepair {
        fn failing(count: u64) -> Self {
            Self {
                proof_failures_remaining: AtomicU64::new(0),
                failures_remaining: AtomicU64::new(count),
                proof_outcomes: Mutex::new(BTreeMap::new()),
                reports: Mutex::new(BTreeMap::new()),
            }
        }

        fn proof_failing(count: u64) -> Self {
            Self {
                proof_failures_remaining: AtomicU64::new(count),
                ..Self::default()
            }
        }

        fn count(&self) -> usize {
            self.reports.lock().expect("repair lock").len()
        }

        fn contains(&self, identity: &[u8; 32]) -> bool {
            self.reports
                .lock()
                .expect("repair lock")
                .contains_key(identity)
        }
    }

    impl PotrLatencyRepairHandoff for RecordingRepair {
        fn enqueue_proof_outcome(
            &self,
            source_identity: [u8; 32],
            receipt: &PotrReceiptV1,
            admission_envelope_digest: [u8; 32],
        ) -> Result<[u8; 32], PotrRepairHandoffError> {
            if self
                .proof_failures_remaining
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                return Err(PotrRepairHandoffError(
                    "injected proof-outcome outage".to_owned(),
                ));
            }
            if receipt
                .signed_receipt_digest()
                .map_err(|error| PotrRepairHandoffError(error.to_string()))?
                != source_identity
            {
                return Err(PotrRepairHandoffError(
                    "proof-outcome source identity conflict".to_owned(),
                ));
            }
            let bytes = norito::to_bytes(receipt)
                .map_err(|error| PotrRepairHandoffError(error.to_string()))?;
            let mut hasher = blake3::Hasher::new();
            hasher.update(b"test.potr.proof-outcome.v1\0");
            hasher.update(&source_identity);
            hasher.update(&admission_envelope_digest);
            hasher.update(&bytes);
            let operation_id = *hasher.finalize().as_bytes();
            let mut proof_outcomes = self.proof_outcomes.lock().expect("proof-outcome lock");
            match proof_outcomes.get(&source_identity) {
                Some((existing, existing_admission, existing_operation))
                    if existing == receipt && *existing_admission == admission_envelope_digest =>
                {
                    Ok(*existing_operation)
                }
                Some(_) => Err(PotrRepairHandoffError(
                    "proof-outcome source identity conflict".to_owned(),
                )),
                None => {
                    proof_outcomes.insert(
                        source_identity,
                        (receipt.clone(), admission_envelope_digest, operation_id),
                    );
                    Ok(operation_id)
                }
            }
        }

        fn enqueue_latency_repair(
            &self,
            source_identity: [u8; 32],
            report: &RepairReportV1,
        ) -> Result<[u8; 32], PotrRepairHandoffError> {
            if self
                .failures_remaining
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                return Err(PotrRepairHandoffError("injected repair outage".to_owned()));
            }
            let bytes = norito::to_bytes(report)
                .map_err(|error| PotrRepairHandoffError(error.to_string()))?;
            let mut hasher = blake3::Hasher::new();
            hasher.update(b"test.potr.repair.v1\0");
            hasher.update(&source_identity);
            hasher.update(&bytes);
            let receipt = *hasher.finalize().as_bytes();
            let mut reports = self.reports.lock().expect("repair lock");
            match reports.get(&source_identity) {
                Some((existing, existing_receipt)) if existing == report => Ok(*existing_receipt),
                Some(_) => Err(PotrRepairHandoffError(
                    "repair source identity conflict".to_owned(),
                )),
                None => {
                    reports.insert(source_identity, (report.clone(), receipt));
                    Ok(receipt)
                }
            }
        }
    }

    fn governed_fixture() -> (AdmissionRecord, KeyPair, KeyPair) {
        let gateway_key =
            KeyPair::try_from_seed(vec![0x11; 32], Algorithm::Ed25519).expect("gateway key");
        let provider_key =
            KeyPair::try_from_seed(vec![0x31; 32], Algorithm::MlDsa).expect("provider key");
        let (_, gateway_public) = gateway_key.public_key().to_bytes();
        let (provider_algorithm, provider_public) = provider_key
            .public_key()
            .try_to_bytes()
            .expect("provider public key");
        assert_eq!(provider_algorithm, Algorithm::MlDsa);
        let descriptor = sorafs_manifest::chunker_registry::lookup(sorafs_manifest::ProfileId(1))
            .expect("SF1 profile");
        let profile_aliases = Some(
            descriptor
                .aliases
                .iter()
                .map(|alias| (*alias).to_owned())
                .collect(),
        );
        let stake = StakePointer {
            pool_id: [0x91; 32],
            stake_amount: "0.000000001".parse().expect("canonical XOR stake"),
        };
        let capabilities = vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::PotrMlDsa,
                payload: provider_public.to_vec(),
            },
        ];
        let endpoint = AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "potr.example.test".to_owned(),
            metadata: Vec::new(),
        };
        let endpoint_admission = EndpointAdmissionV1 {
            endpoint: endpoint.clone(),
            attestation: EndpointAttestationV1 {
                version: sorafs_manifest::ENDPOINT_ATTESTATION_VERSION_V1,
                kind: EndpointAttestationKind::Mtls,
                attested_at: 1_600_000_000,
                expires_at: 1_800_000_000,
                leaf_certificate: vec![1],
                intermediate_certificates: Vec::new(),
                alpn_ids: vec!["h2".to_owned()],
                report: Vec::new(),
            },
        };
        let (vrf_public, vrf_private) =
            iroha_crypto::BlsNormal::keypair(iroha_crypto::KeyGenOption::UseSeed(vec![0x34; 32]))
                .expect("fixture BLS keypair");
        let vrf_pair: KeyPair = (vrf_public, vrf_private).into();
        let proposal = ProviderAdmissionProposalV1 {
            version: sorafs_manifest::PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
            provider_id: PROVIDER_ID,
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases,
            stake: stake.clone(),
            capabilities: capabilities.clone(),
            endpoints: vec![endpoint_admission],
            advert_key: gateway_public
                .try_into()
                .expect("Ed25519 public key length"),
            por_vrf_key: ProviderVrfPublicKeyV1::BlsNormal(
                vrf_pair
                    .public_key()
                    .to_bytes()
                    .1
                    .try_into()
                    .expect("BLS key length"),
            ),
            jurisdiction_code: "US".to_owned(),
            contact_uri: None,
            stream_budget: None,
            transport_hints: None,
        };
        let advert_body = ProviderAdvertBodyV1 {
            provider_id: PROVIDER_ID,
            profile_id: proposal.profile_id.clone(),
            profile_aliases: proposal.profile_aliases.clone(),
            stake,
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 90_000,
                max_concurrent_streams: 1,
            },
            capabilities,
            endpoints: vec![endpoint],
            rendezvous_topics: Vec::new(),
            path_policy: PathDiversityPolicy {
                min_guard_weight: 1,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: None,
            transport_hints: None,
        };
        let mut envelope = ProviderAdmissionEnvelopeV1 {
            version: sorafs_manifest::PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
            proposal_digest: compute_proposal_digest(&proposal).expect("proposal digest"),
            advert_body_digest: compute_advert_body_digest(&advert_body)
                .expect("advert body digest"),
            proposal,
            advert_body,
            issued_at: 1_600_000_000,
            retention_epoch: 1_800_000_000,
            council_signatures: Vec::new(),
            notes: None,
        };
        let council_key = SigningKey::from_bytes(&[0x61; 32]);
        let authorization_digest = compute_envelope_authorization_digest(&envelope)
            .expect("envelope authorization digest");
        envelope.council_signatures.push(CouncilSignature {
            signer: council_key.verifying_key().to_bytes(),
            signature: council_key.sign(&authorization_digest).to_bytes().to_vec(),
        });
        let policy =
            ProviderAdmissionCouncilPolicy::new([council_key.verifying_key().to_bytes()], 1)
                .expect("council policy");
        (
            AdmissionRecord::new(envelope, &policy).expect("governed admission"),
            gateway_key,
            provider_key,
        )
    }

    fn gateway_public_key(key: &KeyPair) -> [u8; 32] {
        key.public_key()
            .to_bytes()
            .1
            .try_into()
            .expect("Ed25519 gateway key")
    }

    fn signed_receipt(
        gateway_key: &KeyPair,
        provider_key: &KeyPair,
        request_id: [u8; 16],
        status: PotrStatus,
        range_start: u64,
    ) -> PotrReceiptV1 {
        let (deadline_ms, latency_ms) = match status {
            PotrStatus::MissedDeadline => (40_000, 42_000),
            _ => (90_000, 42_000),
        };
        let receipt = PotrReceiptV1 {
            version: POTR_RECEIPT_VERSION_V1,
            manifest_digest: MANIFEST_DIGEST,
            provider_id: PROVIDER_ID,
            tier: ProofStreamTier::Hot,
            deadline_ms,
            latency_ms,
            status,
            requested_at_ms: 1_700_000_000_000,
            responded_at_ms: 1_700_000_042_000,
            recorded_at_ms: 1_700_000_042_100,
            range_start,
            range_end: range_start + 1_023,
            request_id: Some(request_id),
            trace_id: Some([0x33; 16]),
            note: None,
            gateway_signature: None,
            provider_signature: None,
        };
        sign_potr_receipt_v1(receipt, gateway_key, provider_key).expect("sign receipt")
    }

    #[test]
    fn final_signed_receipt_is_atomic_restart_safe_and_idempotent() {
        let (admission, gateway_key, provider_key) = governed_fixture();
        let gateway_public = gateway_public_key(&gateway_key);
        let receipt = signed_receipt(
            &gateway_key,
            &provider_key,
            [0x44; 16],
            PotrStatus::Success,
            0,
        );
        let digest = receipt.signed_receipt_digest().expect("signed digest");
        let dir = TempDir::new().expect("state dir");
        let repair = RecordingRepair::default();
        let tracker =
            PotrTracker::open(dir.path(), 8, POTR_TRACKER_DEFAULT_CHECKPOINT_MAX_BYTES_V1)
                .expect("open tracker");
        assert!(matches!(
            tracker
                .record_receipt(receipt.clone(), &gateway_public, &admission, &repair)
                .expect("insert"),
            PotrRecordOutcome::Inserted(_)
        ));
        assert!(matches!(
            tracker
                .record_receipt(receipt.clone(), &gateway_public, &admission, &repair)
                .expect("replay"),
            PotrRecordOutcome::Existing(_)
        ));
        drop(tracker);

        let restored =
            PotrTracker::open(dir.path(), 8, POTR_TRACKER_DEFAULT_CHECKPOINT_MAX_BYTES_V1)
                .expect("restore tracker");
        assert_eq!(
            restored
                .status(&digest)
                .expect("status")
                .map(|status| status.sequence),
            Some(1)
        );
        assert_eq!(
            restored.export_receipts(0, 8).expect("export"),
            vec![receipt]
        );
        assert_eq!(repair.count(), 0);
    }

    #[test]
    fn wrong_signer_and_overlapping_request_scope_fail_closed() {
        let (admission, gateway_key, provider_key) = governed_fixture();
        let gateway_public = gateway_public_key(&gateway_key);
        let repair = RecordingRepair::default();
        let tracker = PotrTracker::in_memory(8).expect("tracker");
        let baseline = signed_receipt(
            &gateway_key,
            &provider_key,
            [0x45; 16],
            PotrStatus::Success,
            0,
        );
        let wrong_gateway =
            KeyPair::try_from_seed(vec![0x12; 32], Algorithm::Ed25519).expect("wrong gateway");
        let wrong = signed_receipt(
            &wrong_gateway,
            &provider_key,
            [0x46; 16],
            PotrStatus::Success,
            0,
        );
        assert!(matches!(
            tracker.record_receipt(wrong, &gateway_public, &admission, &repair),
            Err(PotrTrackerError::Receipt(
                PotrReceiptValidationError::GatewayKeyMismatch
            ))
        ));
        tracker
            .record_receipt(baseline, &gateway_public, &admission, &repair)
            .expect("baseline");
        let overlapping = signed_receipt(
            &gateway_key,
            &provider_key,
            [0x45; 16],
            PotrStatus::Success,
            512,
        );
        assert!(matches!(
            tracker.record_receipt(overlapping, &gateway_public, &admission, &repair),
            Err(PotrTrackerError::RequestScopeConflict { .. })
        ));
    }

    #[test]
    fn proof_outcome_outage_persists_receipt_and_restart_replays_before_repair() {
        let (admission, gateway_key, provider_key) = governed_fixture();
        let gateway_public = gateway_public_key(&gateway_key);
        let receipt = signed_receipt(
            &gateway_key,
            &provider_key,
            [0x49; 16],
            PotrStatus::Success,
            0,
        );
        let receipt_digest = receipt.signed_receipt_digest().expect("receipt digest");
        let dir = TempDir::new().expect("state dir");
        let failing = RecordingRepair::proof_failing(1);
        let tracker =
            PotrTracker::open(dir.path(), 8, POTR_TRACKER_DEFAULT_CHECKPOINT_MAX_BYTES_V1)
                .expect("tracker");
        assert!(matches!(
            tracker.record_receipt(receipt, &gateway_public, &admission, &failing),
            Err(PotrTrackerError::ProofOutcomeHandoff(_))
        ));
        let persisted = tracker
            .status(&receipt_digest)
            .expect("status")
            .expect("persisted receipt");
        assert_eq!(persisted.proof_outcome_receipt_digest, None);
        assert_eq!(persisted.repair_receipt_digest, None);
        drop(tracker);

        let restored =
            PotrTracker::open(dir.path(), 8, POTR_TRACKER_DEFAULT_CHECKPOINT_MAX_BYTES_V1)
                .expect("restore");
        let handoff = RecordingRepair::default();
        assert_eq!(
            restored
                .resume_terminal_handoffs(&handoff)
                .expect("resume terminal handoff"),
            1
        );
        assert!(
            restored
                .status(&receipt_digest)
                .expect("status")
                .expect("persisted receipt")
                .proof_outcome_receipt_digest
                .is_some()
        );
        assert_eq!(
            restored
                .resume_terminal_handoffs(&handoff)
                .expect("no pending handoff"),
            0
        );
    }

    #[test]
    fn repair_outage_persists_pending_identity_and_restart_replays_exactly_once() {
        let (admission, gateway_key, provider_key) = governed_fixture();
        let gateway_public = gateway_public_key(&gateway_key);
        let receipt = signed_receipt(
            &gateway_key,
            &provider_key,
            [0x47; 16],
            PotrStatus::MissedDeadline,
            0,
        );
        let receipt_digest = receipt.signed_receipt_digest().expect("receipt digest");
        let dir = TempDir::new().expect("state dir");
        let failing = RecordingRepair::failing(1);
        let tracker =
            PotrTracker::open(dir.path(), 8, POTR_TRACKER_DEFAULT_CHECKPOINT_MAX_BYTES_V1)
                .expect("tracker");
        assert!(matches!(
            tracker.record_receipt(receipt, &gateway_public, &admission, &failing),
            Err(PotrTrackerError::RepairHandoff(_))
        ));
        assert_eq!(
            tracker
                .status(&receipt_digest)
                .expect("status")
                .expect("persisted receipt")
                .repair_receipt_digest,
            None
        );
        drop(tracker);

        let restored =
            PotrTracker::open(dir.path(), 8, POTR_TRACKER_DEFAULT_CHECKPOINT_MAX_BYTES_V1)
                .expect("restore");
        let repair = RecordingRepair::default();
        assert_eq!(
            restored
                .resume_terminal_handoffs(&repair)
                .expect("resume repair"),
            1
        );
        assert!(repair.contains(&receipt_digest));
        assert_eq!(repair.count(), 1);
        assert_eq!(
            restored
                .resume_terminal_handoffs(&repair)
                .expect("no pending repair"),
            0
        );
    }

    #[test]
    fn node_startup_defers_repair_required_potr_handoff() {
        let (admission, gateway_key, provider_key) = governed_fixture();
        let gateway_public = gateway_public_key(&gateway_key);
        let receipt = signed_receipt(
            &gateway_key,
            &provider_key,
            [0x48; 16],
            PotrStatus::MissedDeadline,
            0,
        );
        let receipt_digest = receipt.signed_receipt_digest().expect("receipt digest");
        let dir = TempDir::new().expect("state dir");
        let root = dir.path().canonicalize().expect("canonical state dir");
        let config = StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .build();
        let tracker = PotrTracker::open(
            &config.data_dir().join("potr-receipts"),
            config.runtime_retention().state_entry_limit(),
            config.runtime_retention().checkpoint_max_bytes(),
        )
        .expect("tracker");
        let failing = RecordingRepair::failing(1);
        assert!(matches!(
            tracker.record_receipt(receipt, &gateway_public, &admission, &failing),
            Err(PotrTrackerError::RepairHandoff(_))
        ));
        drop(tracker);

        let first_restart = NodeHandle::try_new(config.clone())
            .expect("repair-required PoTR handoff must not brick node startup");
        assert_eq!(
            first_restart
                .potr_receipt_status(&receipt_digest)
                .expect("receipt status")
                .expect("persisted receipt")
                .repair_receipt_digest,
            None
        );
        assert!(matches!(
            first_restart.resume_potr_terminal_handoffs(&first_restart),
            Err(PotrTrackerError::RepairHandoff(_))
        ));
        assert_eq!(
            first_restart
                .potr_receipt_status(&receipt_digest)
                .expect("receipt status")
                .expect("persisted receipt")
                .repair_receipt_digest,
            None,
            "a repair-incapable adapter must not fabricate a receipt"
        );
        drop(first_restart);

        let second_restart = NodeHandle::try_new(config)
            .expect("pending repair-required PoTR handoff must remain restart-safe");
        assert_eq!(
            second_restart
                .potr_receipt_status(&receipt_digest)
                .expect("receipt status")
                .expect("persisted receipt")
                .repair_receipt_digest,
            None
        );
    }

    #[test]
    fn racing_conflicting_receipts_have_one_durable_winner() {
        let (admission, gateway_key, provider_key) = governed_fixture();
        let gateway_public = gateway_public_key(&gateway_key);
        let tracker = Arc::new(PotrTracker::in_memory(8).expect("tracker"));
        let repair = Arc::new(RecordingRepair::default());
        let barrier = Arc::new(Barrier::new(3));
        let receipts = [
            signed_receipt(
                &gateway_key,
                &provider_key,
                [0x48; 16],
                PotrStatus::Success,
                0,
            ),
            signed_receipt(
                &gateway_key,
                &provider_key,
                [0x48; 16],
                PotrStatus::Success,
                512,
            ),
        ];
        let mut workers = Vec::new();
        for receipt in receipts {
            let tracker = Arc::clone(&tracker);
            let repair = Arc::clone(&repair);
            let admission = admission.clone();
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                barrier.wait();
                tracker.record_receipt(receipt, &gateway_public, &admission, repair.as_ref())
            }));
        }
        barrier.wait();
        let outcomes = workers
            .into_iter()
            .map(|worker| worker.join().expect("worker"))
            .collect::<Vec<_>>();
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| {
                    matches!(outcome, Err(PotrTrackerError::RequestScopeConflict { .. }))
                })
                .count(),
            1
        );
        assert_eq!(tracker.export_receipts(0, 8).unwrap().len(), 1);
    }
}
