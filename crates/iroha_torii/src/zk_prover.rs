//! Background, non-consensus ZK prover worker tied to attachments.
//!
//! - Periodically scans `zk_attachments` for new items and produces a report
//!   under `zk_prover/reports/<id>.json` with
//!   `{ id, ok, error, content_type, size, created_ms, processed_ms, latency_ms }`.
//!   Bounded retention metadata is persisted independently under
//!   `zk_prover/report_index/<id>.json`, so saving one report never rewrites
//!   metadata for every other report. Report maintenance streams those shards
//!   one at a time; configured count and aggregate-byte retention limits evict
//!   the oldest reports deterministically before a new report is committed.
//!   Attachment discovery likewise retains only a scan-budget-derived window,
//!   resumes its directory cursor across cycles, and canonically orders each
//!   window instead of collecting and sorting the complete tenant population;
//!   unscheduled locations remain in a retry queue with the same hard cap.
//!   Versioned content-ID processing receipts are retained separately from
//!   reports and referenced by each live tenant copy, so report eviction does
//!   not cause completed attachments to be verified again. Retryable policy or
//!   registry failures use bounded exponential backoff.
//!   Registry-backed verifying-key files use the data model's 8 MiB V1 payload
//!   ceiling and a stable direct-file read, so a corrupt key file cannot race
//!   metadata admission and make the worker allocate an unbounded buffer;
//!   inline registry keys are verified by reference instead of being cloned.
//! - This module is strictly app-facing and non-forking. It must not affect consensus.
//! - Enabled and paced via `iroha_config` (torii.zk_prover_enabled, torii.zk_prover_scan_period_secs).
//!
//! The worker verifies `ProofAttachment` payloads (single or list, Norito or JSON)
//! using core backend verifiers and records per-proof metadata. It never mutates WSV.
use crate::{
    routing::MaybeTelemetry,
    zk_attachments::{
        ATTACHMENT_META_FILE_MAX_BYTES, ProverProcessingDecision, ProverProcessingReceipt,
        ZK_PROVER_PROCESSING_STATE_VERSION, ensure_prover_processing_reference,
        load_prover_processing_receipt, open_attachment_regular_file,
        persist_prover_processing_receipt_if_referenced, prover_processing_decision,
        read_bounded_attachment_regular_file, reconcile_prover_processing_receipt_if_referenced,
        validate_attachment_body_contract, validate_attachment_metadata_contract,
    },
    zk1::{MAX_TLV_COUNT as ZK1_MAX_TLV_COUNT, parse_tags as parse_zk1_tags},
};
use iroha_core::{
    state::{
        State as CoreState, StateQueryView, StateReadOnly, WorldReadOnly,
        compute_zk_consensus_policy_hash,
    },
    zk::{
        hash_proof, hash_vk, is_developer_only_backend_label, is_production_claim_backend_label,
        is_trusted_setup_backend_label, is_verifier_backend_registry_label_v1,
        production_verify_backend_tag, verify_backend_with_timing_checked,
    },
};
#[cfg(test)]
use iroha_crypto::Hash;
use iroha_data_model::proof::{
    ProofAttachment, ProofAttachmentList, VERIFYING_KEY_BOX_MAX_PAYLOAD_BYTES_V1, VerifyingKeyBox,
    VerifyingKeyId, VerifyingKeyRecord,
};
use iroha_data_model::zk::BackendTag;
use mv::storage::StorageReadOnly;
use norito::json;
use parking_lot::{Mutex, RwLock};
use sha2::{Digest as _, Sha256};
#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::{
    collections::{BinaryHeap, HashSet},
    fs,
    io::Read as _,
    io::{Error as IoError, ErrorKind as IoErrorKind},
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    runtime::{Handle, RuntimeFlavor},
    sync::Semaphore,
    task::{self, JoinSet},
};
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
/// Durable processing disposition embedded in a prover report.
pub struct ProverReportProcessing {
    /// Whether the attachment outcome must not be retried automatically.
    pub terminal: bool,
    /// Earliest retry time for a transient failure, in Unix milliseconds.
    #[norito(required)]
    pub retry_not_before_ms: Option<u64>,
    /// Number of transient attempts already made for this attachment.
    pub retry_count: u32,
    /// Indices whose successful verification can be reused on the next retry.
    pub completed_proof_indices: Vec<u16>,
    /// Hash of the effective verifier context for reusable successful proofs.
    #[norito(required)]
    pub processing_context_hash: Option<String>,
}
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
/// Per-proof result entry for prover reports.
pub struct ProofReportEntry {
    /// Proof backend identifier.
    pub backend: String,
    /// True if verification succeeded.
    pub ok: bool,
    /// Optional error string on failure.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Stable proof hash (hex) when available.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub proof_hash: Option<String>,
    /// Verifying key reference resolved from attachment or registry.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub vk_ref: Option<VerifyingKeyId>,
    /// Circuit identifier if resolved from the verifier registry.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub circuit_id: Option<String>,
}
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
/// Result of processing an attachment by the non-consensus prover worker.
pub struct ProverReport {
    /// Attachment id processed.
    pub id: String,
    /// True if processing succeeded.
    pub ok: bool,
    /// Optional error string on failure.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Attachment content type.
    pub content_type: String,
    /// Attachment size in bytes.
    pub size: u64,
    /// Original creation time (ms) of the attachment.
    pub created_ms: u64,
    /// Time (ms) when this report was produced.
    pub processed_ms: u64,
    /// Wall-clock latency between attachment creation and prover processing (ms).
    #[norito(default)]
    pub latency_ms: u64,
    /// For Norito ZK1 envelopes, discovered unique TLV tags (at most 64).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub zk1_tags: Option<Vec<String>>,
    /// Proof backend (when the attachment holds a single proof).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Verifying key reference (when the attachment holds a single proof).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub vk_ref: Option<VerifyingKeyId>,
    /// Proof hash (hex) for single-proof attachments.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub proof_hash: Option<String>,
    /// Circuit identifier (when resolved from the verifier registry).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub circuit_id: Option<String>,
    /// Per-proof results for attachments containing multiple proofs.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub proofs: Vec<ProofReportEntry>,
    /// Processing disposition used to recover a receipt after a partial commit.
    #[norito(required)]
    pub processing: Option<ProverReportProcessing>,
}
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
struct ProverReportSummary {
    id: String,
    ok: bool,
    #[norito(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    content_type: String,
    processed_ms: u64,
    #[norito(skip_serializing_if = "Option::is_none")]
    zk1_tags: Option<Vec<String>>,
}
#[derive(Clone)]
struct ProverCfg {
    enabled: bool,
    scan_period_secs: u64,
    reports_ttl_secs: u64,
    reports_max_count: u64,
    reports_max_bytes: u64,
    max_inflight: usize,
    max_scan_bytes: u64,
    max_scan_millis: u64,
    keys_dir: PathBuf,
    allowed_backends: Vec<String>,
    allowed_circuits: Vec<String>,
    state: Option<Arc<CoreState>>,
    telemetry: MaybeTelemetry,
    #[cfg(test)]
    verification_attempts: Arc<AtomicUsize>,
}
static PROVER_CFG: OnceLock<RwLock<ProverCfg>> = OnceLock::new();
#[cfg(test)]
static TEST_PROCESSING_DELAY_MS: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static TEST_SNAPSHOT_LOAD_DELAY_MS: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static TEST_MAX_SCAN_MILLIS_OVERRIDE: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static MAX_INFLIGHT_OBSERVED: AtomicUsize = AtomicUsize::new(0);
/// Configure prover scheduling, bounded report retention, verifier scope, and telemetry.
#[allow(clippy::too_many_arguments)]
pub fn configure(
    enabled: bool,
    scan_period_secs: u64,
    reports_ttl_secs: u64,
    reports_max_count: u64,
    reports_max_bytes: u64,
    max_inflight: usize,
    max_scan_bytes: u64,
    max_scan_millis: u64,
    keys_dir: PathBuf,
    allowed_backends: Vec<String>,
    allowed_circuits: Vec<String>,
    state: Option<Arc<CoreState>>,
    telemetry: MaybeTelemetry,
) {
    assert!(
        reports_max_count > 0,
        "prover report retention count must be greater than zero"
    );
    assert!(
        reports_max_bytes >= REPORT_FILE_MAX_BYTES.saturating_add(REPORT_SUMMARY_FILE_MAX_BYTES),
        "prover report retention bytes must fit one maximum-size report and summary"
    );
    let cfg = ProverCfg {
        enabled,
        scan_period_secs,
        reports_ttl_secs,
        reports_max_count,
        reports_max_bytes,
        max_inflight,
        max_scan_bytes,
        max_scan_millis,
        keys_dir,
        allowed_backends,
        allowed_circuits,
        state,
        telemetry,
        #[cfg(test)]
        verification_attempts: Arc::new(AtomicUsize::new(0)),
    };
    if let Some(lock) = PROVER_CFG.get() {
        let mut guard = lock.write();
        *guard = cfg;
        return;
    }
    if PROVER_CFG.set(RwLock::new(cfg.clone())).is_err() {
        if let Some(lock) = PROVER_CFG.get() {
            let mut guard = lock.write();
            *guard = cfg;
        }
    }
}
fn with_cfg<R>(f: impl FnOnce(&ProverCfg) -> R) -> Option<R> {
    PROVER_CFG.get().map(|lock| {
        let guard = lock.read();
        f(&*guard)
    })
}
pub(crate) fn cfg_enabled() -> bool {
    with_cfg(|c| c.enabled).unwrap_or(false)
}
fn cfg_scan_period() -> Duration {
    Duration::from_secs(with_cfg(|c| c.scan_period_secs).unwrap_or(30))
}
fn cfg_reports_ttl_secs() -> u64 {
    with_cfg(|c| c.reports_ttl_secs).unwrap_or(7 * 24 * 60 * 60)
}
fn cfg_reports_max_count() -> u64 {
    with_cfg(|c| c.reports_max_count)
        .unwrap_or(iroha_config::parameters::defaults::torii::ZK_PROVER_REPORTS_MAX_COUNT)
}
fn cfg_reports_max_bytes() -> u64 {
    with_cfg(|c| c.reports_max_bytes)
        .unwrap_or(iroha_config::parameters::defaults::torii::ZK_PROVER_REPORTS_MAX_BYTES)
}
fn cfg_max_inflight() -> usize {
    with_cfg(|c| c.max_inflight)
        .unwrap_or(iroha_config::parameters::defaults::torii::ZK_PROVER_MAX_INFLIGHT)
        .max(1)
}
fn cfg_max_scan_bytes() -> u64 {
    with_cfg(|c| c.max_scan_bytes)
        .unwrap_or(iroha_config::parameters::defaults::torii::ZK_PROVER_MAX_SCAN_BYTES)
}
fn cfg_max_scan_millis() -> u64 {
    #[cfg(test)]
    {
        let override_millis = TEST_MAX_SCAN_MILLIS_OVERRIDE.load(Ordering::Relaxed);
        if override_millis > 0 {
            return override_millis;
        }
    }
    with_cfg(|c| c.max_scan_millis)
        .unwrap_or(iroha_config::parameters::defaults::torii::ZK_PROVER_MAX_SCAN_MILLIS)
}
fn cfg_keys_dir() -> PathBuf {
    with_cfg(|c| c.keys_dir.clone())
        .unwrap_or_else(iroha_config::parameters::defaults::torii::zk_prover_keys_dir)
}
fn cfg_allowed_backends() -> Vec<String> {
    with_cfg(|c| c.allowed_backends.clone())
        .unwrap_or_else(iroha_config::parameters::defaults::torii::zk_prover_allowed_backends)
}
fn cfg_allowed_circuits() -> Vec<String> {
    with_cfg(|c| c.allowed_circuits.clone())
        .unwrap_or_else(iroha_config::parameters::defaults::torii::zk_prover_allowed_circuits)
}
fn cfg_state() -> Option<Arc<CoreState>> {
    with_cfg(|c| c.state.clone()).flatten()
}
#[cfg(test)]
fn cfg_verification_attempts() -> Option<Arc<AtomicUsize>> {
    with_cfg(|cfg| Arc::clone(&cfg.verification_attempts))
}
#[cfg(test)]
fn proof_verification_attempt_count() -> usize {
    with_cfg(|cfg| cfg.verification_attempts.load(AtomicOrdering::SeqCst)).unwrap_or(0)
}
#[cfg(test)]
fn set_proof_verification_attempt_count(count: usize) {
    let _ = with_cfg(|cfg| {
        cfg.verification_attempts
            .store(count, AtomicOrdering::SeqCst);
    });
}
fn telemetry_handle() -> MaybeTelemetry {
    with_cfg(|c| c.telemetry.clone()).unwrap_or_else(MaybeTelemetry::disabled)
}
fn prover_dir() -> PathBuf {
    super::zk_attachments::base_dir().join("zk_prover")
}
fn reports_dir() -> PathBuf {
    prover_dir().join("reports")
}
fn ensure_dirs() {
    // `base_dir()` can be overridden in tests; keep directory creation keyed to the current path.
    static LAST_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    let slot = LAST_DIR.get_or_init(|| Mutex::new(None));
    let dir = reports_dir();
    let mut guard = slot.lock();
    if guard.as_ref() != Some(&dir) {
        let _ = fs::create_dir_all(&dir);
        // The first-release store uses independent summary shards. Remove the
        // obsolete generated aggregate rather than retaining attacker-amplified
        // bytes that no code reads.
        let _ = fs::remove_file(prover_dir().join("reports_index.json"));
        *guard = Some(dir);
    }
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
const ATTACHMENT_ID_HEX_LEN: usize = 64;
const TENANT_KEY_HEX_LEN: usize = 64;
const PROOF_ATTACHMENT_BODY_MAX_BYTES_V1: u64 =
    iroha_config::parameters::defaults::torii::ZK_PROVER_ATTACHMENT_BODY_MAX_BYTES_V1;
const REPORT_FILE_MAX_BYTES: u64 =
    iroha_config::parameters::defaults::torii::ZK_PROVER_REPORT_MAX_BYTES_V1;
const REPORT_SUMMARY_FILE_MAX_BYTES: u64 =
    iroha_config::parameters::defaults::torii::ZK_PROVER_REPORT_SUMMARY_MAX_BYTES_V1;
const REPORT_SUMMARY_ERROR_MAX_BYTES: usize = 4 * 1024;
const REPORT_SUMMARY_CONTENT_TYPE_MAX_BYTES: usize = 256;
const REPORT_SUMMARY_TAG_MAX_BYTES: usize = 32;
const REPORT_RETENTION_EVICTION_BATCH: usize = 128;
// A discovery slot owns two fixed-length path components plus Vec/HashSet and
// allocator overhead. Charging a conservative 512 bytes of the configured scan
// byte geometry per slot keeps discovery memory proportional to the operator's
// existing scan budget without changing the attachment-body byte accounting.
const ATTACHMENT_DISCOVERY_BYTES_PER_LOCATION: u64 = 512;
const ATTACHMENT_DISCOVERY_MAX_LOCATIONS: u64 = 4_096;
// Directory entries include tenant directories, files, and end-of-directory
// transitions. Eight work items per retained location permit ordinary
// one-file tenant layouts to make progress while bounding hostile namespaces.
const ATTACHMENT_DISCOVERY_WORK_PER_LOCATION: u64 = 8;
const ATTACHMENT_DISCOVERY_MAX_WORK_ITEMS: u64 =
    ATTACHMENT_DISCOVERY_MAX_LOCATIONS * ATTACHMENT_DISCOVERY_WORK_PER_LOCATION;
static REPORT_SUMMARY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static ATTACHMENT_DISCOVERY_STATE: OnceLock<Mutex<Option<AttachmentDiscoveryState>>> =
    OnceLock::new();
static ATTACHMENT_PROCESSING_CLAIMS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
include!("zk_prover/attachment_discovery_and_report_storage.rs");
struct AttachmentProcessingClaim {
    id: String,
}
impl AttachmentProcessingClaim {
    fn acquire(id: &str) -> Option<Self> {
        let claims = ATTACHMENT_PROCESSING_CLAIMS.get_or_init(|| Mutex::new(HashSet::new()));
        let mut claims = claims.lock();
        claims
            .insert(id.to_owned())
            .then(|| Self { id: id.to_owned() })
    }
}
impl Drop for AttachmentProcessingClaim {
    fn drop(&mut self) {
        if let Some(claims) = ATTACHMENT_PROCESSING_CLAIMS.get() {
            claims.lock().remove(&self.id);
        }
    }
}
#[cfg(test)]
fn persist_report_summaries_locked(summaries: &[ProverReportSummary]) -> std::io::Result<()> {
    ensure_dirs();
    fs::create_dir_all(report_index_dir())?;
    for summary in summaries {
        let Some(id) = sanitize_report_id(&summary.id) else {
            continue;
        };
        let mut normalized = summary.clone();
        normalized.id = id;
        persist_report_summary_locked(&normalized)?;
    }
    for entry in fs::read_dir(report_index_dir())? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let keep = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .is_some_and(|id| {
                summaries
                    .iter()
                    .filter_map(|summary| sanitize_report_id(&summary.id))
                    .any(|desired| desired == id)
            });
        if !keep {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}
fn read_report_summary_locked(id: &str) -> Option<ProverReportSummary> {
    let clean = sanitize_report_id(id)?;
    let path = report_summary_path_from_sanitized(&clean);
    let metadata = fs::symlink_metadata(&path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > REPORT_SUMMARY_FILE_MAX_BYTES {
        return None;
    }
    let mut reader = fs::File::open(path)
        .ok()?
        .take(REPORT_SUMMARY_FILE_MAX_BYTES.saturating_add(1));
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).ok()?;
    if buf.len() as u64 > REPORT_SUMMARY_FILE_MAX_BYTES {
        return None;
    }
    let s = std::str::from_utf8(&buf).ok()?;
    let mut summary = norito::json::from_json::<ProverReportSummary>(s).ok()?;
    summary.id = clean;
    Some(bound_persisted_report_summary(summary))
}
fn report_id_from_entry(entry: &fs::DirEntry) -> Option<String> {
    if !entry.file_type().ok()?.is_file() {
        return None;
    }
    let name = entry.file_name();
    let raw_id = name.to_str()?.strip_suffix(".json")?;
    let clean = sanitize_report_id(raw_id)?;
    (raw_id == clean).then_some(clean)
}
fn visit_report_ids(mut visitor: impl FnMut(String) -> bool) {
    let Ok(entries) = fs::read_dir(reports_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let Some(id) = report_id_from_entry(&entry) else {
            continue;
        };
        if !visitor(id) {
            break;
        }
    }
}
fn load_or_repair_report_summary_locked(id: &str) -> Option<ProverReportSummary> {
    let clean = sanitize_report_id(id)?;
    if !report_path_from_sanitized(&clean).is_file() {
        let _ = fs::remove_file(report_summary_path_from_sanitized(&clean));
        return None;
    }
    if let Some(summary) = read_report_summary_locked(&clean) {
        return Some(summary);
    }
    let report = load_report(&clean)?;
    let summary = report_summary_from_report(&report);
    let _ = persist_report_summary_locked(&summary);
    Some(summary)
}
fn visit_report_summaries_locked(mut visitor: impl FnMut(ProverReportSummary) -> bool) {
    visit_report_ids(|id| {
        if let Some(summary) = load_or_repair_report_summary_locked(&id) {
            visitor(summary)
        } else {
            true
        }
    });
}
fn prune_stale_report_summaries_locked() -> usize {
    let mut pruned = 0usize;
    let Ok(entries) = fs::read_dir(report_index_dir()) else {
        return pruned;
    };
    for entry in entries.flatten() {
        let Some(id) = report_id_from_entry(&entry) else {
            continue;
        };
        if !report_path_from_sanitized(&id).is_file() && fs::remove_file(entry.path()).is_ok() {
            pruned = pruned.saturating_add(1);
        }
    }
    pruned
}
#[cfg(test)]
fn read_report_summaries_locked() -> Vec<ProverReportSummary> {
    let mut summaries = Vec::new();
    visit_report_summaries_locked(|summary| {
        if summaries.len()
            >= usize::try_from(
                iroha_config::parameters::defaults::torii::ZK_PROVER_REPORTS_MAX_COUNT,
            )
            .unwrap_or(usize::MAX)
        {
            return false;
        }
        summaries.push(summary);
        true
    });
    summaries.sort_by(|left, right| left.id.cmp(&right.id));
    summaries
}
#[cfg(test)]
fn load_report_summaries() -> Vec<ProverReportSummary> {
    let _guard = report_summary_lock().lock();
    let _ = prune_stale_report_summaries_locked();
    read_report_summaries_locked()
}
#[cfg(test)]
fn remove_report_summary_locked(id: &str) {
    let Some(clean) = sanitize_report_id(id) else {
        return;
    };
    let _ = fs::remove_file(report_summary_path_from_sanitized(&clean));
}
#[cfg(test)]
fn remove_report_summary(id: &str) {
    let _guard = report_summary_lock().lock();
    remove_report_summary_locked(id);
}
fn scan_deadline_reached(start: std::time::Instant, max_millis: u64) -> bool {
    start.elapsed() >= Duration::from_millis(max_millis)
}
fn canonicalize_attachment_locations(locations: &mut Vec<AttachmentLocation>) {
    locations.sort_unstable();
    let mut seen_ids = HashSet::with_capacity(locations.len());
    locations.retain(|location| seen_ids.insert(location.id.clone()));
}
fn processing_retry_delay_ms(retry_count: u32) -> u64 {
    const MAX_RETRY_DELAY_MS: u64 = 24 * 60 * 60 * 1_000;
    let base = u64::try_from(cfg_scan_period().as_millis())
        .unwrap_or(u64::MAX)
        .max(1_000);
    let exponent = retry_count.saturating_sub(1).min(16);
    base.saturating_mul(1_u64 << exponent)
        .min(MAX_RETRY_DELAY_MS)
}
fn processing_receipt_from_report(report: &ProverReport) -> Option<ProverProcessingReceipt> {
    let processing = if report.ok {
        ProverReportProcessing {
            terminal: true,
            retry_not_before_ms: None,
            retry_count: 0,
            completed_proof_indices: Vec::new(),
            processing_context_hash: None,
        }
    } else {
        report.processing.clone()?
    };
    let receipt = ProverProcessingReceipt {
        version: ZK_PROVER_PROCESSING_STATE_VERSION,
        id: report.id.clone(),
        processed_ms: report.processed_ms,
        terminal: processing.terminal,
        retry_not_before_ms: processing.retry_not_before_ms,
        retry_count: processing.retry_count,
        completed_proof_indices: processing.completed_proof_indices,
        processing_context_hash: processing.processing_context_hash,
    };
    receipt.disposition_is_valid().then_some(receipt)
}
fn committed_report_processing_decision(id: &str, now_ms: u64) -> Option<ProverProcessingDecision> {
    let report = load_report(id)?;
    let committed = processing_receipt_from_report(&report)?;
    let receipt =
        reconcile_prover_processing_receipt_if_referenced(&committed).unwrap_or_else(|error| {
            iroha_logger::warn!(
                attachment_id = %id,
                %error,
                "Failed to reconcile a committed ZK prover report receipt"
            );
            ProverProcessingReceipt::reconcile_committed(
                load_prover_processing_receipt(id),
                committed,
            )
        });
    if receipt.terminal
        || receipt
            .retry_not_before_ms
            .is_some_and(|retry_at| now_ms < retry_at)
    {
        Some(ProverProcessingDecision::Suppress)
    } else {
        Some(ProverProcessingDecision::Due {
            retry_count: receipt.retry_count,
        })
    }
}
fn attachment_needs_processing(location: &AttachmentLocation) -> bool {
    if let Err(error) = ensure_prover_processing_reference(&location.tenant_key, &location.id) {
        iroha_logger::warn!(
            attachment_id = %location.id,
            tenant = %location.tenant_key,
            %error,
            "Failed to persist ZK prover live-attachment reference"
        );
        return false;
    }
    processing_retry_count(&location.id).is_some()
}
fn discover_attachment_window(
    stream: &mut AttachmentDirectoryStream,
    geometry: AttachmentDiscoveryGeometry,
    start: std::time::Instant,
    max_millis: u64,
    mut include: impl FnMut(&AttachmentLocation) -> bool,
) -> AttachmentDiscovery {
    let mut discovery = AttachmentDiscovery::default();
    loop {
        if discovery.locations.len() >= geometry.max_locations {
            discovery.work_exhausted = true;
            break;
        }
        if discovery.work_items >= geometry.max_work_items {
            discovery.work_exhausted = true;
            break;
        }
        if scan_deadline_reached(start, max_millis) {
            discovery.time_exhausted = true;
            break;
        }
        let step = stream.step();
        discovery.work_items = discovery.work_items.saturating_add(1);
        match step {
            AttachmentDirectoryStep::Advanced => {}
            AttachmentDirectoryStep::Location(location) => {
                if include(&location) {
                    discovery.locations.push(location);
                }
            }
            AttachmentDirectoryStep::Complete => {
                discovery.sweep_complete = true;
                break;
            }
        }
        if scan_deadline_reached(start, max_millis) {
            discovery.time_exhausted = true;
            break;
        }
    }
    // Directory order is platform-local and non-consensus. Canonical ordering
    // inside each bounded window keeps scheduling and tests reproducible without
    // retaining the complete attachment population merely to sort it.
    canonicalize_attachment_locations(&mut discovery.locations);
    discovery
}
fn attachment_discovery_state() -> &'static Mutex<Option<AttachmentDiscoveryState>> {
    ATTACHMENT_DISCOVERY_STATE.get_or_init(|| Mutex::new(None))
}
fn discover_pending_attachment_locations(
    geometry: AttachmentDiscoveryGeometry,
    start: std::time::Instant,
    max_millis: u64,
) -> AttachmentDiscovery {
    let root = attachments_root_dir();
    let mut state_guard = attachment_discovery_state().lock();
    let root_changed = state_guard
        .as_ref()
        .is_some_and(|state| state.root.as_path() != root.as_path());
    if root_changed || state_guard.is_none() {
        *state_guard = Some(AttachmentDiscoveryState {
            root: root.clone(),
            stream: None,
            retry_locations: Vec::new(),
        });
    }
    let state = state_guard.as_mut().expect("initialized above");
    let mut discovery = AttachmentDiscovery::default();
    if !state.retry_locations.is_empty() {
        let mut retry_locations = std::mem::take(&mut state.retry_locations)
            .into_iter()
            .peekable();
        loop {
            if retry_locations.peek().is_none() {
                break;
            }
            if discovery.locations.len() >= geometry.max_locations
                || discovery.work_items >= geometry.max_work_items
            {
                discovery.work_exhausted = true;
                break;
            }
            if scan_deadline_reached(start, max_millis) {
                discovery.time_exhausted = true;
                break;
            }
            let location = retry_locations.next().expect("peeked above");
            discovery.work_items = discovery.work_items.saturating_add(1);
            if attachment_needs_processing(&location) {
                discovery.locations.push(location);
            }
            if scan_deadline_reached(start, max_millis) {
                discovery.time_exhausted = true;
                break;
            }
        }
        state.retry_locations.extend(retry_locations);
        if discovery.work_exhausted || discovery.time_exhausted {
            canonicalize_attachment_locations(&mut discovery.locations);
            return discovery;
        }
    }
    if state.stream.is_none() {
        let Ok(stream) = AttachmentDirectoryStream::open(root) else {
            discovery.sweep_complete = true;
            canonicalize_attachment_locations(&mut discovery.locations);
            return discovery;
        };
        state.stream = Some(stream);
    }
    let remaining_geometry = AttachmentDiscoveryGeometry {
        max_locations: geometry
            .max_locations
            .saturating_sub(discovery.locations.len()),
        max_work_items: geometry.max_work_items.saturating_sub(discovery.work_items),
    };
    let streamed = discover_attachment_window(
        state.stream.as_mut().expect("initialized above"),
        remaining_geometry,
        start,
        max_millis,
        attachment_needs_processing,
    );
    discovery.locations.extend(streamed.locations);
    discovery.work_items = discovery.work_items.saturating_add(streamed.work_items);
    discovery.sweep_complete = streamed.sweep_complete;
    discovery.work_exhausted = streamed.work_exhausted;
    discovery.time_exhausted = streamed.time_exhausted;
    if discovery.sweep_complete {
        state.stream = None;
    }
    canonicalize_attachment_locations(&mut discovery.locations);
    discovery
}
fn retry_pending_attachment_locations(locations: Vec<AttachmentLocation>) {
    if locations.is_empty() {
        return;
    }
    let root = attachments_root_dir();
    let mut state_guard = attachment_discovery_state().lock();
    let root_changed = state_guard
        .as_ref()
        .is_some_and(|state| state.root.as_path() != root.as_path());
    if root_changed || state_guard.is_none() {
        *state_guard = Some(AttachmentDiscoveryState {
            root,
            stream: None,
            retry_locations: Vec::new(),
        });
    }
    let state = state_guard.as_mut().expect("initialized above");
    let hard_cap = usize::try_from(ATTACHMENT_DISCOVERY_MAX_LOCATIONS)
        .expect("the hard attachment discovery cap fits usize");
    // Callers can return at most one bounded discovery window. Merge before
    // truncation so concurrent scans retain the canonical lowest locations,
    // independent of completion order.
    state.retry_locations.extend(locations);
    canonicalize_attachment_locations(&mut state.retry_locations);
    state.retry_locations.truncate(hard_cap);
}
fn find_attachment_location(id: &str) -> Option<AttachmentLocation> {
    let clean = sanitize_attachment_id(id)?;
    let mut stream = AttachmentDirectoryStream::open(attachments_root_dir()).ok()?;
    let mut found: Option<AttachmentLocation> = None;
    loop {
        match stream.step() {
            AttachmentDirectoryStep::Advanced => {}
            AttachmentDirectoryStep::Location(location) => {
                if location.id == clean && found.as_ref().is_none_or(|current| &location < current)
                {
                    found = Some(location);
                }
            }
            AttachmentDirectoryStep::Complete => return found,
        }
    }
}
fn load_attachment_meta(loc: &AttachmentLocation) -> Option<super::zk_attachments::AttachmentMeta> {
    let path = attachment_meta_path(&loc.tenant_key, &loc.id);
    let buf = read_bounded_attachment_regular_file(&path, ATTACHMENT_META_FILE_MAX_BYTES).ok()?;
    let s = std::str::from_utf8(&buf).ok()?;
    norito::json::from_json::<super::zk_attachments::AttachmentMeta>(s).ok()
}
struct AttachmentBodyLoad {
    observed_size: u64,
    bytes_read: u64,
    body: Result<Vec<u8>, String>,
}
enum AttachmentBodyLoadOutcome {
    Loaded(AttachmentBodyLoad),
    DeferredForByteBudget { required_bytes: u64 },
}
struct AttachmentSnapshot {
    meta: super::zk_attachments::AttachmentMeta,
    body_load: AttachmentBodyLoad,
}
enum AttachmentSnapshotLoad {
    Ready(AttachmentSnapshot),
    DeferredForByteBudget { required_bytes: u64 },
}
fn load_attachment_body_with_read_budget(
    loc: &AttachmentLocation,
    read_budget: u64,
) -> Option<AttachmentBodyLoadOutcome> {
    let path = attachment_bin_path(&loc.tenant_key, &loc.id);
    let (file, opened_metadata) = match open_attachment_regular_file(&path) {
        Ok(opened) => opened,
        Err(error) => {
            return Some(AttachmentBodyLoadOutcome::Loaded(AttachmentBodyLoad {
                observed_size: 0,
                bytes_read: 0,
                body: Err(format!(
                    "failed to securely open proof attachment body: {error}"
                )),
            }));
        }
    };
    let observed_size = opened_metadata.len();
    if observed_size > PROOF_ATTACHMENT_BODY_MAX_BYTES_V1 {
        return Some(AttachmentBodyLoadOutcome::Loaded(AttachmentBodyLoad {
            observed_size,
            bytes_read: 0,
            body: Err(format!(
                "proof attachment body is {observed_size} bytes, exceeding the {PROOF_ATTACHMENT_BODY_MAX_BYTES_V1}-byte first-release limit"
            )),
        }));
    }
    if observed_size > read_budget {
        return Some(AttachmentBodyLoadOutcome::DeferredForByteBudget {
            required_bytes: observed_size,
        });
    }
    // Read from this one opened file description and never reopen the path.
    // Limiting the read to the opened size makes aggregate accounting exact;
    // a concurrent grow/shrink is detected from descriptor metadata below.
    let mut reader = file.take(observed_size);
    let mut bytes = Vec::with_capacity(usize::try_from(observed_size).ok()?);
    if let Err(error) = reader.read_to_end(&mut bytes) {
        let bytes_read = u64::try_from(bytes.len()).ok()?;
        return Some(AttachmentBodyLoadOutcome::Loaded(AttachmentBodyLoad {
            observed_size,
            bytes_read,
            body: Err(format!("failed to read proof attachment body: {error}")),
        }));
    }
    let read_size = u64::try_from(bytes.len()).ok()?;
    let final_size = match reader.get_ref().metadata() {
        Ok(metadata) => metadata.len(),
        Err(error) => {
            return Some(AttachmentBodyLoadOutcome::Loaded(AttachmentBodyLoad {
                observed_size: read_size,
                bytes_read: read_size,
                body: Err(format!(
                    "failed to re-inspect opened proof attachment body: {error}"
                )),
            }));
        }
    };
    if read_size != observed_size || final_size != observed_size {
        return Some(AttachmentBodyLoadOutcome::Loaded(AttachmentBodyLoad {
            observed_size: final_size,
            bytes_read: read_size,
            body: Err(format!(
                "proof attachment body changed size while being read: opened {observed_size}, read {read_size}, final {final_size}"
            )),
        }));
    }
    Some(AttachmentBodyLoadOutcome::Loaded(AttachmentBodyLoad {
        observed_size,
        bytes_read: read_size,
        body: Ok(bytes),
    }))
}
#[cfg(test)]
fn load_attachment_body(loc: &AttachmentLocation) -> Option<AttachmentBodyLoad> {
    match load_attachment_body_with_read_budget(loc, PROOF_ATTACHMENT_BODY_MAX_BYTES_V1)? {
        AttachmentBodyLoadOutcome::Loaded(body_load) => Some(body_load),
        AttachmentBodyLoadOutcome::DeferredForByteBudget { .. } => {
            unreachable!("the intrinsic body ceiling is a sufficient direct-load read budget")
        }
    }
}
fn load_attachment_snapshot(
    loc: &AttachmentLocation,
    read_budget: u64,
) -> Option<AttachmentSnapshotLoad> {
    let meta = load_attachment_meta(loc)?;
    let snapshot_load = match load_attachment_body_with_read_budget(loc, read_budget)? {
        AttachmentBodyLoadOutcome::Loaded(body_load) => {
            AttachmentSnapshotLoad::Ready(AttachmentSnapshot { meta, body_load })
        }
        AttachmentBodyLoadOutcome::DeferredForByteBudget { required_bytes } => {
            AttachmentSnapshotLoad::DeferredForByteBudget { required_bytes }
        }
    };
    #[cfg(test)]
    {
        let delay = TEST_SNAPSHOT_LOAD_DELAY_MS.load(AtomicOrdering::Relaxed);
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
    }
    Some(snapshot_load)
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReportRetentionCandidate {
    processed_ms: u64,
    id: String,
    retained_bytes: u64,
}
impl Ord for ReportRetentionCandidate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.processed_ms, &self.id, self.retained_bytes).cmp(&(
            other.processed_ms,
            &other.id,
            other.retained_bytes,
        ))
    }
}
impl PartialOrd for ReportRetentionCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Default)]
struct ReportStoreScan {
    count: u64,
    retained_bytes: u64,
    oldest: BinaryHeap<ReportRetentionCandidate>,
}
fn scan_report_store_locked(exclude_id: &str) -> ReportStoreScan {
    let mut scan = ReportStoreScan::default();
    visit_report_ids(|id| {
        if id == exclude_id {
            return true;
        }
        let report_path = report_path_from_sanitized(&id);
        let Ok(report_metadata) = fs::symlink_metadata(&report_path) else {
            return true;
        };
        if !report_metadata.file_type().is_file() {
            return true;
        }
        let processed_ms =
            load_or_repair_report_summary_locked(&id).map_or(0, |summary| summary.processed_ms);
        let summary_bytes = fs::symlink_metadata(report_summary_path_from_sanitized(&id))
            .ok()
            .filter(|metadata| metadata.file_type().is_file())
            .map_or(0, |metadata| metadata.len());
        let retained_bytes = report_metadata.len().saturating_add(summary_bytes);
        scan.count = scan.count.saturating_add(1);
        scan.retained_bytes = scan.retained_bytes.saturating_add(retained_bytes);
        let candidate = ReportRetentionCandidate {
            processed_ms,
            id,
            retained_bytes,
        };
        if scan.oldest.len() < REPORT_RETENTION_EVICTION_BATCH {
            scan.oldest.push(candidate);
        } else if scan
            .oldest
            .peek()
            .is_some_and(|newest_old| candidate < *newest_old)
        {
            let _ = scan.oldest.pop();
            scan.oldest.push(candidate);
        }
        true
    });
    scan
}
fn report_store_fits(count: u64, retained_bytes: u64, max_count: u64, max_bytes: u64) -> bool {
    count <= max_count && retained_bytes <= max_bytes
}
fn remove_file_if_present(path: &Path) -> std::io::Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == IoErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
fn delete_report_files_locked(id: &str) -> std::io::Result<bool> {
    let Some(clean) = sanitize_report_id(id) else {
        return Ok(false);
    };
    let report_path = report_path_from_sanitized(&clean);
    if report_path.is_file() {
        if let Some(committed) = load_report(&clean)
            .as_ref()
            .and_then(processing_receipt_from_report)
        {
            let _ = reconcile_prover_processing_receipt_if_referenced(&committed)?;
        }
    }
    let removed_report = remove_file_if_present(&report_path)?;
    let removed_summary = remove_file_if_present(&report_summary_path_from_sanitized(&clean))?;
    Ok(removed_report || removed_summary)
}
fn enforce_report_store_capacity_locked(
    exclude_id: &str,
    added_count: u64,
    added_bytes: u64,
    max_count: u64,
    max_bytes: u64,
) -> std::io::Result<usize> {
    if added_count > max_count || added_bytes > max_bytes {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "prover report retention geometry cannot admit the report",
        ));
    }
    let _ = prune_stale_report_summaries_locked();
    let mut evicted = 0usize;
    loop {
        let scan = scan_report_store_locked(exclude_id);
        let mut projected_count = scan.count.saturating_add(added_count);
        let mut projected_bytes = scan.retained_bytes.saturating_add(added_bytes);
        if report_store_fits(projected_count, projected_bytes, max_count, max_bytes) {
            return Ok(evicted);
        }
        let candidates = scan.oldest.into_sorted_vec();
        if candidates.is_empty() {
            return Err(IoError::other(
                "prover report retention geometry cannot admit the report",
            ));
        }
        for candidate in candidates {
            delete_report_files_locked(&candidate.id)?;
            evicted = evicted.saturating_add(1);
            projected_count = projected_count.saturating_sub(1);
            projected_bytes = projected_bytes.saturating_sub(candidate.retained_bytes);
            if report_store_fits(projected_count, projected_bytes, max_count, max_bytes) {
                return Ok(evicted);
            }
        }
    }
}
fn save_report_with_limits(
    rep: &ProverReport,
    max_count: u64,
    max_bytes: u64,
) -> std::io::Result<()> {
    let Some(id) = sanitize_report_id(&rep.id) else {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "invalid prover report id",
        ));
    };
    ensure_dirs();
    let body = norito::json::to_json_pretty(rep)
        .map_err(|error| IoError::new(IoErrorKind::InvalidData, error.to_string()))?;
    if body.len() as u64 > REPORT_FILE_MAX_BYTES {
        return Err(IoError::new(
            IoErrorKind::InvalidData,
            "prover report exceeds the hard size limit",
        ));
    }
    let summary = report_summary_from_report(rep);
    let summary_body = norito::json::to_json(&summary)
        .map_err(|error| IoError::new(IoErrorKind::InvalidData, error.to_string()))?;
    if summary_body.len() as u64 > REPORT_SUMMARY_FILE_MAX_BYTES {
        return Err(IoError::new(
            IoErrorKind::InvalidData,
            "prover report summary exceeds the hard size limit",
        ));
    }
    let incoming_bytes = (body.len() as u64).saturating_add(summary_body.len() as u64);
    let _guard = report_summary_lock().lock();
    enforce_report_store_capacity_locked(&id, 1, incoming_bytes, max_count, max_bytes)?;
    let path = report_path_from_sanitized(&id);
    let tmp_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(tmp_dir)?;
    use std::io::Write as _;
    tmp.write_all(body.as_bytes())?;
    tmp.flush()?;
    tmp.persist(&path).map(|_| ()).map_err(|e| e.error)?;
    persist_report_summary_locked(&summary)?;
    Ok(())
}
fn save_report(rep: &ProverReport) -> std::io::Result<()> {
    save_report_with_limits(rep, cfg_reports_max_count(), cfg_reports_max_bytes())
}
fn load_report(id: &str) -> Option<ProverReport> {
    let clean = sanitize_report_id(id)?;
    let path = report_path_from_sanitized(&clean);
    let file_len = fs::metadata(&path).ok()?.len();
    if file_len > REPORT_FILE_MAX_BYTES {
        iroha_logger::warn!(
            %clean,
            file_len,
            max = REPORT_FILE_MAX_BYTES,
            "Skipping oversized prover report file"
        );
        return None;
    }
    let f = fs::File::open(path).ok()?;
    let mut reader = f.take(REPORT_FILE_MAX_BYTES.saturating_add(1));
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).ok()?;
    if (buf.len() as u64) > REPORT_FILE_MAX_BYTES {
        iroha_logger::warn!(
            %clean,
            read_len = buf.len(),
            max = REPORT_FILE_MAX_BYTES,
            "Skipping oversized prover report payload"
        );
        return None;
    }
    let s = std::str::from_utf8(&buf).ok()?;
    let mut report = norito::json::from_json::<ProverReport>(s).ok()?;
    // Normalize persisted ids defensively so lookups remain canonical.
    report.id = clean;
    Some(report)
}
#[cfg(test)]
fn delete_report_files(id: &str) {
    let _guard = report_summary_lock().lock();
    let _ = delete_report_files_locked(id);
}
fn record_prover_metrics(report: &ProverReport) {
    let telemetry = telemetry_handle();
    let status_label = if report.ok { "ok" } else { "error" };
    telemetry.with_metrics(|tel| {
        tel.observe_torii_zk_prover(
            status_label,
            report.content_type.as_str(),
            report.size,
            report.latency_ms,
        );
    });
}
/// Garbage collect expired report artifacts and stale index entries.
///
/// Returns the number of report records removed by retention GC.
pub fn gc_reports_once() -> usize {
    ensure_dirs();
    let ttl = Duration::from_secs(cfg_reports_ttl_secs());
    let now = now_ms();
    let ttl_ms = ttl.as_millis() as u64;
    let mut deleted = 0usize;
    let _guard = report_summary_lock().lock();
    visit_report_summaries_locked(|summary| {
        let age_ms = now.saturating_sub(summary.processed_ms);
        if age_ms > ttl_ms {
            if delete_report_files_locked(&summary.id).unwrap_or(false) {
                deleted = deleted.saturating_add(1);
            }
        }
        true
    });
    deleted = deleted.saturating_add(prune_stale_report_summaries_locked());
    match enforce_report_store_capacity_locked(
        "",
        0,
        0,
        cfg_reports_max_count(),
        cfg_reports_max_bytes(),
    ) {
        Ok(evicted) => deleted = deleted.saturating_add(evicted),
        Err(error) => {
            iroha_logger::warn!(%error, "Failed to enforce prover report retention geometry");
        }
    }
    if deleted > 0 {
        let telemetry = telemetry_handle();
        telemetry.with_metrics(|tel| tel.inc_torii_zk_prover_gc(deleted as u64));
    }
    deleted
}
#[derive(Clone)]
struct ProverContext {
    keys_dir: PathBuf,
    allowed_backends: Vec<String>,
    allowed_circuits: Vec<String>,
    state: Option<Arc<CoreState>>,
    #[cfg(test)]
    verification_attempts: Option<Arc<AtomicUsize>>,
}
fn backend_allowed(backend: &str, allowlist: &[String]) -> bool {
    !is_trusted_setup_backend_label(backend)
        && !is_developer_only_backend_label(backend)
        && !is_production_claim_backend_label(backend)
        && !is_unsupported_stark_fri_backend_label(backend)
        && is_verifier_backend_registry_label_v1(backend)
        && (allowlist.is_empty() || allowlist.iter().any(|allowed| backend.starts_with(allowed)))
}
fn is_unsupported_stark_fri_backend_label(backend: &str) -> bool {
    backend.starts_with(iroha_data_model::zk::ZK_BACKEND_STARK_FRI_V1)
        && !iroha_data_model::zk::is_stark_fri_v1_backend_label(backend)
}
fn circuit_allowed(circuit_id: &str, allowlist: &[String]) -> bool {
    allowlist.is_empty()
        || allowlist
            .iter()
            .any(|allowed| circuit_id.starts_with(allowed))
}
fn sanitize_vk_component(component: &str) -> String {
    let mut out = String::with_capacity(component.len());
    for ch in component.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "_".to_string() } else { out }
}
fn vk_store_path(keys_dir: &Path, id: &VerifyingKeyId) -> PathBuf {
    let backend = sanitize_vk_component(id.backend.as_ref());
    let name = sanitize_vk_component(&id.name);
    keys_dir.join(format!("{backend}__{name}.vk"))
}
fn load_vk_bytes(keys_dir: &Path, id: &VerifyingKeyId) -> Result<Vec<u8>, String> {
    let path = vk_store_path(keys_dir, id);
    read_bounded_attachment_regular_file(
        &path,
        u64::try_from(VERIFYING_KEY_BOX_MAX_PAYLOAD_BYTES_V1)
            .expect("V1 verifying-key byte ceiling fits u64"),
    )
    .map_err(|err| {
        format!(
            "failed to read bounded verifying key bytes at {}: {err}",
            path.display()
        )
    })
}
fn decode_norito_attachments(body: &[u8]) -> Result<Vec<ProofAttachment>, String> {
    let list_err = match norito::decode_canonical::<ProofAttachmentList>(body) {
        Ok(list) => return Ok(list.into_vec()),
        Err(err) => err.to_string(),
    };
    let single_err = match norito::decode_canonical::<ProofAttachment>(body) {
        Ok(single) => return Ok(vec![single]),
        Err(err) => err.to_string(),
    };
    Err(format!(
        "norito decode failed (list: {list_err}, single: {single_err})"
    ))
}
fn decode_json_attachments(body: &[u8]) -> Result<Vec<ProofAttachment>, String> {
    let list_err = match norito::json::from_slice::<ProofAttachmentList>(body) {
        Ok(list) => return Ok(list.into_vec()),
        Err(err) => err.to_string(),
    };
    let single_err = match norito::json::from_slice::<ProofAttachment>(body) {
        Ok(single) => return Ok(vec![single]),
        Err(err) => err.to_string(),
    };
    Err(format!(
        "json decode failed (canonical list: {list_err}, single: {single_err})"
    ))
}
fn decode_proof_attachments(
    content_type: &str,
    body: &[u8],
) -> Result<Vec<ProofAttachment>, String> {
    if u64::try_from(body.len()).map_or(true, |size| size > PROOF_ATTACHMENT_BODY_MAX_BYTES_V1) {
        return Err(format!(
            "proof attachment body exceeds the {PROOF_ATTACHMENT_BODY_MAX_BYTES_V1}-byte first-release limit"
        ));
    }
    match super::utils::strict_typed_content_format(content_type) {
        Some(super::utils::TypedRequestContentFormat::Norito) => {
            if body.len() >= 4 && &body[..4] == b"ZK1\0" {
                return match parse_zk1_tags(body) {
                    Ok(_) => {
                        Err("unsupported ZK1 envelope (expected ProofAttachment payload)".into())
                    }
                    Err(err) => Err(err),
                };
            }
            decode_norito_attachments(body).map_err(|err| format!("norito decode error: {err}"))
        }
        Some(super::utils::TypedRequestContentFormat::Json) => {
            decode_json_attachments(body).map_err(|err| format!("json decode error: {err}"))
        }
        None if super::utils::is_parameter_free_media_type(
            content_type,
            "application",
            "x-zk1",
        ) =>
        {
            match parse_zk1_tags(body) {
                Ok(_) => Err("unsupported ZK1 envelope (expected ProofAttachment payload)".into()),
                Err(err) => Err(err),
            }
        }
        None => Err(format!(
            "unsupported proof attachment content type: {content_type}"
        )),
    }
}
fn processing_context_put_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}
fn processing_context_put_str(hasher: &mut Sha256, value: &str) {
    processing_context_put_bytes(hasher, value.as_bytes());
}
fn processing_context_put_option_str(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            processing_context_put_str(hasher, value);
        }
        None => hasher.update([0]),
    }
}
fn processing_context_put_option_u64(hasher: &mut Sha256, value: Option<u64>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.to_be_bytes());
        }
        None => hasher.update([0]),
    }
}
fn processing_context_put_record(
    hasher: &mut Sha256,
    record: Option<&VerifyingKeyRecord>,
    verification_height: Option<u64>,
) {
    let Some(record) = record else {
        hasher.update([0]);
        return;
    };
    hasher.update([1]);
    hasher.update(record.version.to_be_bytes());
    processing_context_put_str(hasher, &record.circuit_id);
    processing_context_put_option_str(hasher, record.owner_manifest_id.as_deref());
    processing_context_put_str(hasher, &record.namespace);
    processing_context_put_str(hasher, record.backend.canonical_label());
    processing_context_put_str(hasher, &record.curve);
    processing_context_put_bytes(hasher, &record.public_inputs_schema_hash);
    processing_context_put_bytes(hasher, &record.commitment);
    hasher.update(record.vk_len.to_be_bytes());
    hasher.update(record.max_proof_bytes.to_be_bytes());
    processing_context_put_option_str(hasher, record.gas_schedule_id.as_deref());
    processing_context_put_option_str(hasher, record.metadata_uri_cid.as_deref());
    processing_context_put_option_str(hasher, record.vk_bytes_cid.as_deref());
    processing_context_put_option_u64(hasher, record.activation_height);
    processing_context_put_option_u64(hasher, record.withdraw_height);
    hasher.update([u8::from(record.status)]);
    hasher.update([verification_height.is_some_and(|height| record.is_active_at(height)) as u8]);
    // The commitment, rather than another copy of the potentially large key,
    // defines verifier identity. A prior success remains valid if storage moves
    // between an inline key and the same commitment-backed external key.
}
fn proof_processing_context_hash(
    ctx: &ProverContext,
    verifier_view: Option<&StateQueryView<'_>>,
    attachments: &[ProofAttachment],
) -> String {
    let mut hasher = Sha256::new();
    processing_context_put_bytes(&mut hasher, b"iroha:torii:zk-prover-retry-context:v1");
    processing_context_put_str(&mut hasher, env!("CARGO_PKG_VERSION"));
    processing_context_put_option_str(&mut hasher, option_env!("VERGEN_GIT_SHA"));
    hasher.update([
        cfg!(feature = "zk-halo2") as u8,
        cfg!(feature = "zk-halo2-ipa") as u8,
        cfg!(feature = "zk-stark") as u8,
        cfg!(feature = "goldilocks_backend") as u8,
        cfg!(feature = "circuit-params") as u8,
    ]);
    hasher.update(
        u64::try_from(ctx.allowed_backends.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for backend in &ctx.allowed_backends {
        processing_context_put_str(&mut hasher, backend);
    }
    hasher.update(
        u64::try_from(ctx.allowed_circuits.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for circuit in &ctx.allowed_circuits {
        processing_context_put_str(&mut hasher, circuit);
    }
    hasher.update(
        u64::try_from(attachments.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    match verifier_view {
        Some(view) => {
            hasher.update([1]);
            processing_context_put_bytes(&mut hasher, &compute_zk_consensus_policy_hash(&view.zk));
            let verification_height = u64::try_from(view.height()).unwrap_or(u64::MAX);
            let world = view.world();
            let verifying_keys = world.verifying_keys();
            for attachment in attachments {
                processing_context_put_str(&mut hasher, attachment.backend.as_str());
                processing_context_put_str(&mut hasher, attachment.vk_ref.backend.as_str());
                processing_context_put_str(&mut hasher, &attachment.vk_ref.name);
                processing_context_put_record(
                    &mut hasher,
                    verifying_keys.get(&attachment.vk_ref),
                    Some(verification_height),
                );
            }
        }
        None => {
            hasher.update([0]);
            for attachment in attachments {
                processing_context_put_str(&mut hasher, attachment.backend.as_str());
                processing_context_put_str(&mut hasher, attachment.vk_ref.backend.as_str());
                processing_context_put_str(&mut hasher, &attachment.vk_ref.name);
                processing_context_put_record(&mut hasher, None, None);
            }
        }
    }
    hex::encode(hasher.finalize())
}
struct ProofProcessingResult {
    report: ProofReportEntry,
    retryable: bool,
}
fn cached_successful_proof_report(
    verifier_view: Option<&StateQueryView<'_>>,
    attachment: &ProofAttachment,
) -> ProofReportEntry {
    let circuit_id = verifier_view.and_then(|view| {
        view.world()
            .verifying_keys()
            .get(&attachment.vk_ref)
            .map(|record| record.circuit_id.clone())
    });
    ProofReportEntry {
        backend: attachment.backend.to_string(),
        ok: true,
        error: None,
        proof_hash: Some(hex::encode(hash_proof(&attachment.proof))),
        vk_ref: Some(attachment.vk_ref.clone()),
        circuit_id,
    }
}
#[cfg(test)]
fn process_proof_attachment_with_disposition(
    ctx: &ProverContext,
    attachment: &ProofAttachment,
) -> ProofProcessingResult {
    let verifier_view = ctx.state.as_ref().map(|state| state.query_view());
    process_proof_attachment_in_view(ctx, verifier_view.as_ref(), attachment)
}
fn process_proof_attachment_in_view(
    ctx: &ProverContext,
    verifier_view: Option<&StateQueryView<'_>>,
    attachment: &ProofAttachment,
) -> ProofProcessingResult {
    let backend = attachment.backend.clone();
    let backend_str = backend.as_str();
    let proof_hash = Some(hex::encode(hash_proof(&attachment.proof)));
    let mut errors = Vec::new();
    let resolved_vk_ref = attachment.vk_ref.clone();
    let mut circuit_id: Option<String> = None;
    let mut retryable = false;
    let mut terminal_error = false;
    let result =
        |errors: Vec<String>, circuit_id: Option<String>, retryable: bool, terminal_error: bool| {
            let ok = errors.is_empty();
            ProofProcessingResult {
                report: ProofReportEntry {
                    backend: backend.clone(),
                    ok,
                    error: (!ok).then(|| errors.join("; ")),
                    proof_hash: proof_hash.clone(),
                    vk_ref: Some(resolved_vk_ref.clone()),
                    circuit_id,
                },
                retryable: !ok && retryable && !terminal_error,
            }
        };
    if attachment.proof.backend.as_str() != backend_str {
        errors.push("proof backend does not match attachment backend".into());
        terminal_error = true;
    }
    if attachment.proof.bytes.is_empty() {
        errors.push("proof bytes are empty".into());
        terminal_error = true;
    }
    if is_trusted_setup_backend_label(backend_str) {
        errors.push(format!(
            "trusted-setup backend `{backend_str}` is not supported"
        ));
        terminal_error = true;
    } else if is_developer_only_backend_label(backend_str) {
        errors.push(format!(
            "developer-only backend `{backend_str}` is not supported"
        ));
        terminal_error = true;
    } else if !backend_allowed(backend_str, &ctx.allowed_backends) {
        errors.push(format!("backend `{backend_str}` not allowed"));
        retryable = is_verifier_backend_registry_label_v1(backend_str);
        terminal_error |= !retryable;
    }
    match production_verify_backend_tag(backend_str) {
        Some(BackendTag::Halo2IpaPasta) if !cfg!(feature = "zk-halo2-ipa") => {
            errors.push("halo2 verification is unavailable in this node build".into());
            retryable = true;
        }
        Some(BackendTag::Stark) if !cfg!(feature = "zk-stark") => {
            errors.push("stark verification is unavailable in this node build".into());
            retryable = true;
        }
        _ => {}
    }
    if let Some(view) = verifier_view {
        let zk = &view.zk;
        match production_verify_backend_tag(backend_str) {
            Some(BackendTag::Halo2IpaPasta) if !zk.halo2.enabled => {
                errors.push("halo2 verification is disabled in node configuration".into());
                retryable = true;
            }
            Some(BackendTag::Halo2IpaPasta)
                if attachment.proof.bytes.len() > zk.halo2.max_envelope_bytes =>
            {
                errors.push(format!(
                    "halo2 proof exceeds node-configured max_envelope_bytes {}",
                    zk.halo2.max_envelope_bytes
                ));
                retryable = true;
            }
            Some(BackendTag::Halo2IpaPasta) => {
                if let Ok(envelope) = norito::decode_canonical::<
                    iroha_data_model::zk::OpenVerifyEnvelope,
                >(&attachment.proof.bytes)
                    && envelope.backend == BackendTag::Halo2IpaPasta
                    && envelope.proof_bytes.len() > zk.halo2.max_proof_bytes
                {
                    errors.push(format!(
                        "halo2 proof exceeds node-configured max_proof_bytes {}",
                        zk.halo2.max_proof_bytes
                    ));
                    retryable = true;
                }
            }
            Some(BackendTag::Stark) if !zk.stark.enabled => {
                errors.push("stark verification is disabled in node configuration".into());
                retryable = true;
            }
            Some(BackendTag::Stark)
                if attachment.proof.bytes.len() > zk.stark.max_envelope_bytes =>
            {
                errors.push(format!(
                    "stark proof exceeds node-configured max_envelope_bytes {}",
                    zk.stark.max_envelope_bytes
                ));
                retryable = true;
            }
            Some(BackendTag::Stark) => {
                if let Ok(envelope) = norito::decode_canonical::<
                    iroha_data_model::zk::OpenVerifyEnvelope,
                >(&attachment.proof.bytes)
                    && envelope.backend == BackendTag::Stark
                {
                    if envelope.proof_bytes.len() > zk.stark.max_envelope_bytes {
                        errors.push(format!(
                            "stark proof wrapper exceeds node-configured max_envelope_bytes {}",
                            zk.stark.max_envelope_bytes
                        ));
                        retryable = true;
                    } else if let Ok(open) = norito::decode_canonical::<
                        iroha_data_model::zk::StarkFriOpenProofV1,
                    >(&envelope.proof_bytes)
                        && open.envelope_bytes.len() > zk.stark.max_proof_bytes
                    {
                        errors.push(format!(
                            "stark proof exceeds node-configured max_proof_bytes {}",
                            zk.stark.max_proof_bytes
                        ));
                        retryable = true;
                    }
                }
            }
            None => {}
        }
    }
    let vk_id = &attachment.vk_ref;
    if vk_id.backend.as_str() != backend_str {
        errors.push(format!(
            "vk_ref backend `{}` does not match proof backend `{backend_str}`",
            vk_id.backend
        ));
        terminal_error = true;
    }
    if !errors.is_empty() {
        return result(errors, circuit_id, retryable, terminal_error);
    }
    let view = match verifier_view {
        Some(view) => view,
        None => {
            errors.push("verifying key lookup requires core state".into());
            return result(errors, circuit_id, true, false);
        }
    };
    let verification_height = u64::try_from(view.height()).unwrap_or(u64::MAX);
    let record = match view.world().verifying_keys().get(vk_id) {
        Some(record) => record,
        None => {
            errors.push("verifying key not found in registry".into());
            return result(errors, circuit_id, true, false);
        }
    };
    if !record.is_active_at(verification_height) {
        errors.push("verifying key is not active".into());
        retryable = true;
    }
    if record.max_proof_bytes > 0 && attachment.proof.bytes.len() > record.max_proof_bytes as usize
    {
        errors.push(format!(
            "proof exceeds max_proof_bytes {}",
            record.max_proof_bytes
        ));
        retryable = true;
    }
    if let Some(commitment) = attachment.vk_commitment
        && commitment != record.commitment
    {
        errors.push("vk_commitment does not match registry commitment".into());
        retryable = true;
    }
    circuit_id = Some(record.circuit_id.clone());
    let vk_box = match record.key.as_ref() {
        Some(key) if key.backend.as_str() != backend_str => {
            errors.push("verifying key backend does not match proof backend".into());
            retryable = true;
            None
        }
        Some(key) => Some(std::borrow::Cow::Borrowed(key)),
        None => match load_vk_bytes(&ctx.keys_dir, vk_id) {
            Ok(bytes) => {
                if record.vk_len > 0 && bytes.len() != record.vk_len as usize {
                    errors.push(format!(
                        "verifying key length {} does not match registry vk_len {}",
                        bytes.len(),
                        record.vk_len
                    ));
                    retryable = true;
                }
                Some(std::borrow::Cow::Owned(VerifyingKeyBox::new(
                    backend.clone(),
                    bytes,
                )))
            }
            Err(err) => {
                errors.push(err);
                retryable = true;
                None
            }
        },
    };
    if let Some(vk_box) = vk_box.as_deref() {
        if vk_box.bytes.is_empty() {
            errors.push("verifying key bytes are empty".into());
            retryable = true;
        } else {
            let vk_hash = hash_vk(vk_box);
            if vk_hash != record.commitment {
                errors.push("verifying key bytes do not match registry commitment".into());
                retryable = true;
            }
        }
    }
    if !ctx.allowed_circuits.is_empty() {
        match circuit_id.as_deref() {
            Some(circuit) if circuit_allowed(circuit, &ctx.allowed_circuits) => {}
            Some(circuit) => errors.push(format!("circuit `{circuit}` not allowed")),
            None => errors.push("circuit_id unavailable for allowlist".into()),
        }
        if !errors.is_empty() {
            retryable = true;
        }
    }
    if errors.is_empty() {
        match vk_box.as_deref() {
            Some(vk_box) => {
                #[cfg(test)]
                if let Some(attempts) = &ctx.verification_attempts {
                    attempts.fetch_add(1, AtomicOrdering::SeqCst);
                }
                let verified = verify_backend_with_timing_checked(
                    backend_str,
                    &attachment.proof,
                    Some(vk_box),
                    &view.zk,
                )
                .ok;
                if !verified {
                    errors.push("verification failed".into());
                }
            }
            None => errors.push("verifying key bytes missing".into()),
        }
    }
    result(errors, circuit_id, retryable, false)
}
#[cfg(test)]
fn process_proof_attachment(ctx: &ProverContext, attachment: &ProofAttachment) -> ProofReportEntry {
    process_proof_attachment_with_disposition(ctx, attachment).report
}
/// Process a single attachment id, emitting a report if not present yet.
pub fn process_attachment_once(id: &str) -> Option<ProverReport> {
    let clean = sanitize_attachment_id(id)?;
    let loc = find_attachment_location(&clean)?;
    process_attachment_once_at(&loc)
}
fn processing_retry_count(id: &str) -> Option<u32> {
    let now_ms = now_ms();
    let report_decision = || committed_report_processing_decision(id, now_ms);
    match prover_processing_decision(id, now_ms) {
        ProverProcessingDecision::Suppress => {
            // A report rename may have committed immediately before a crash or
            // failed terminal-receipt write. Reconcile it even while the
            // provisional backoff is active, before retention can evict it.
            // A terminal receipt is already authoritative, so avoid reopening
            // and decoding its potentially large report on every scan.
            let receipt = load_prover_processing_receipt(id);
            reconcile_suppressed_report_if_needed(receipt.as_ref(), || {
                let _ = report_decision();
            });
            None
        }
        ProverProcessingDecision::Due { retry_count } => match report_decision() {
            Some(ProverProcessingDecision::Suppress) => None,
            Some(ProverProcessingDecision::Due {
                retry_count: report_retry_count,
            }) => Some(retry_count.max(report_retry_count)),
            Some(ProverProcessingDecision::Missing) | None => Some(retry_count),
        },
        ProverProcessingDecision::Missing => match report_decision() {
            Some(ProverProcessingDecision::Suppress) => None,
            Some(ProverProcessingDecision::Due { retry_count }) => Some(retry_count),
            Some(ProverProcessingDecision::Missing) | None => Some(0),
        },
    }
}
fn reconcile_suppressed_report_if_needed(
    receipt: Option<&ProverProcessingReceipt>,
    reconcile_report: impl FnOnce(),
) {
    if receipt.is_none_or(|receipt| !receipt.terminal) {
        reconcile_report();
    }
}
#[derive(Clone, Default)]
struct CompletedProofCache {
    indices: Vec<u16>,
    context_hash: Option<String>,
}
fn completed_proof_cache_for_retry(id: &str) -> CompletedProofCache {
    let durable = load_prover_processing_receipt(id).filter(|receipt| !receipt.terminal);
    let committed = load_report(id)
        .as_ref()
        .and_then(processing_receipt_from_report)
        .filter(|receipt| !receipt.terminal);
    let selected = match (durable, committed) {
        (Some(durable), Some(committed)) => {
            ProverProcessingReceipt::reconcile_committed(Some(durable), committed)
        }
        (Some(receipt), None) | (None, Some(receipt)) => receipt,
        (None, None) => return CompletedProofCache::default(),
    };
    CompletedProofCache {
        indices: selected.completed_proof_indices,
        context_hash: selected.processing_context_hash,
    }
}
fn checkpoint_completed_proofs(
    loc: &AttachmentLocation,
    retry_count: u32,
    retry_not_before_ms: u64,
    completed_proof_indices: &[u16],
    processing_context_hash: &str,
) {
    if completed_proof_indices.is_empty() {
        return;
    }
    let receipt = ProverProcessingReceipt {
        version: ZK_PROVER_PROCESSING_STATE_VERSION,
        id: loc.id.clone(),
        processed_ms: now_ms(),
        terminal: false,
        retry_not_before_ms: Some(retry_not_before_ms),
        retry_count,
        completed_proof_indices: completed_proof_indices.to_vec(),
        processing_context_hash: Some(processing_context_hash.to_owned()),
    };
    match persist_prover_processing_receipt_if_referenced(&receipt) {
        Ok(true) => {}
        Ok(false) => iroha_logger::debug!(
            attachment_id = %loc.id,
            "Skipping successful-proof checkpoint because no live attachment reference remains"
        ),
        Err(error) => iroha_logger::warn!(
            attachment_id = %loc.id,
            %error,
            "Failed to checkpoint a successful sibling proof before processing the next proof"
        ),
    }
}
fn process_attachment_once_at(loc: &AttachmentLocation) -> Option<ProverReport> {
    if let Err(error) = ensure_prover_processing_reference(&loc.tenant_key, &loc.id) {
        iroha_logger::warn!(
            attachment_id = %loc.id,
            tenant = %loc.tenant_key,
            %error,
            "Failed to persist ZK prover live-attachment reference"
        );
        return load_report(&loc.id);
    }
    if processing_retry_count(&loc.id).is_none() {
        return load_report(&loc.id);
    }
    match load_attachment_snapshot(loc, PROOF_ATTACHMENT_BODY_MAX_BYTES_V1)? {
        AttachmentSnapshotLoad::Ready(snapshot) => process_attachment_snapshot_at(loc, snapshot),
        AttachmentSnapshotLoad::DeferredForByteBudget { .. } => {
            unreachable!("the intrinsic body ceiling is a sufficient direct-load read budget")
        }
    }
}
fn validate_attachment_snapshot<'a>(
    loc: &AttachmentLocation,
    meta: &super::zk_attachments::AttachmentMeta,
    body_load: &'a AttachmentBodyLoad,
) -> Result<&'a [u8], String> {
    let body = body_load
        .body
        .as_deref()
        .map_err(std::clone::Clone::clone)?;
    if meta.size != body_load.observed_size {
        return Err(format!(
            "proof attachment metadata size {} does not match the actual {}-byte body",
            meta.size, body_load.observed_size
        ));
    }
    validate_attachment_metadata_contract(meta, &loc.tenant_key, &loc.id)?;
    validate_attachment_body_contract(meta, body)?;
    Ok(body)
}
fn process_attachment_snapshot_at(
    loc: &AttachmentLocation,
    snapshot: AttachmentSnapshot,
) -> Option<ProverReport> {
    // A direct request and the background scan may race. Only one claimant may
    // verify a content id; later claimants observe its durable receipt/report.
    let _claim = AttachmentProcessingClaim::acquire(&loc.id)?;
    let Some(previous_retry_count) = processing_retry_count(&loc.id) else {
        return load_report(&loc.id);
    };
    let previous_completed_proofs = completed_proof_cache_for_retry(&loc.id);
    let retry_count = previous_retry_count.saturating_add(1);
    let attempt_started_ms = now_ms();
    let provisional_retry_not_before_ms =
        attempt_started_ms.saturating_add(processing_retry_delay_ms(retry_count));
    let provisional_receipt = ProverProcessingReceipt {
        version: ZK_PROVER_PROCESSING_STATE_VERSION,
        id: loc.id.clone(),
        processed_ms: attempt_started_ms,
        terminal: false,
        retry_not_before_ms: Some(provisional_retry_not_before_ms),
        retry_count,
        completed_proof_indices: previous_completed_proofs.indices.clone(),
        processing_context_hash: previous_completed_proofs.context_hash.clone(),
    };
    match persist_prover_processing_receipt_if_referenced(&provisional_receipt) {
        Ok(true) => {}
        Ok(false) => return None,
        Err(error) => {
            iroha_logger::warn!(
                attachment_id = %loc.id,
                %error,
                "Skipping ZK proof processing because its provisional receipt could not persist"
            );
            return None;
        }
    }
    let AttachmentSnapshot { meta, body_load } = snapshot;
    let validated_body = validate_attachment_snapshot(loc, &meta, &body_load);
    let zk1_tags = validated_body.as_ref().ok().and_then(|body| {
        if body.len() >= 4 && &body[..4] == b"ZK1\0" {
            parse_zk1_tags(body).ok()
        } else {
            None
        }
    });
    let ctx = ProverContext {
        keys_dir: cfg_keys_dir(),
        allowed_backends: cfg_allowed_backends(),
        allowed_circuits: cfg_allowed_circuits(),
        state: cfg_state(),
        #[cfg(test)]
        verification_attempts: cfg_verification_attempts(),
    };
    let mut proofs: Vec<ProofReportEntry> = Vec::new();
    let (
        ok,
        err,
        backend,
        vk_ref,
        proof_hash,
        circuit_id,
        retryable,
        completed_proof_indices,
        processing_context_hash,
    ) = match validated_body.and_then(|body| decode_proof_attachments(&meta.content_type, body)) {
        Ok(attachments) => {
            if attachments.is_empty() {
                (
                    false,
                    Some("empty proof attachment list".into()),
                    None,
                    None,
                    None,
                    None,
                    false,
                    Vec::new(),
                    None,
                )
            } else {
                let mut saw_retryable_failure = false;
                let mut saw_terminal_failure = false;
                let verifier_view = ctx.state.as_ref().map(|state| state.query_view());
                let current_processing_context_hash =
                    proof_processing_context_hash(&ctx, verifier_view.as_ref(), &attachments);
                let cached_successes: HashSet<u16> = previous_completed_proofs
                    .context_hash
                    .as_deref()
                    .filter(|hash| *hash == current_processing_context_hash)
                    .map(|_| previous_completed_proofs.indices.iter().copied().collect())
                    .unwrap_or_default();
                let mut completed_proof_indices = Vec::with_capacity(attachments.len());
                for (index, attachment) in attachments.into_iter().enumerate() {
                    let index = u16::try_from(index).ok();
                    if index.is_some_and(|index| cached_successes.contains(&index)) {
                        completed_proof_indices.extend(index);
                        proofs.push(cached_successful_proof_report(
                            verifier_view.as_ref(),
                            &attachment,
                        ));
                        continue;
                    }
                    let processed =
                        process_proof_attachment_in_view(&ctx, verifier_view.as_ref(), &attachment);
                    if !processed.report.ok {
                        saw_retryable_failure |= processed.retryable;
                        saw_terminal_failure |= !processed.retryable;
                    } else {
                        completed_proof_indices.extend(index);
                        checkpoint_completed_proofs(
                            loc,
                            retry_count,
                            provisional_retry_not_before_ms,
                            &completed_proof_indices,
                            &current_processing_context_hash,
                        );
                    }
                    proofs.push(processed.report);
                }
                completed_proof_indices.sort_unstable();
                completed_proof_indices.dedup();
                let failures: Vec<_> = proofs.iter().filter(|p| !p.ok).collect();
                let ok = failures.is_empty();
                let err = if ok {
                    None
                } else {
                    let first = failures
                        .first()
                        .and_then(|p| p.error.clone())
                        .unwrap_or_else(|| "verification failed".into());
                    Some(format!(
                        "{} of {} proofs failed: {}",
                        failures.len(),
                        proofs.len(),
                        first
                    ))
                };
                let (backend, vk_ref, proof_hash, circuit_id) = if proofs.len() == 1 {
                    let entry = &proofs[0];
                    (
                        Some(entry.backend.clone()),
                        entry.vk_ref.clone(),
                        entry.proof_hash.clone(),
                        entry.circuit_id.clone(),
                    )
                } else {
                    (None, None, None, None)
                };
                if proofs.len() == 1 {
                    proofs.clear();
                }
                let retryable = saw_retryable_failure && !saw_terminal_failure;
                let processing_context_hash = (!completed_proof_indices.is_empty())
                    .then_some(current_processing_context_hash);
                (
                    ok,
                    err,
                    backend,
                    vk_ref,
                    proof_hash,
                    circuit_id,
                    retryable,
                    completed_proof_indices,
                    processing_context_hash,
                )
            }
        }
        Err(err) => (
            false,
            Some(err),
            None,
            None,
            None,
            None,
            false,
            Vec::new(),
            None,
        ),
    };
    #[cfg(test)]
    {
        use std::time::Duration;
        let delay = TEST_PROCESSING_DELAY_MS.load(AtomicOrdering::Relaxed);
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
    }
    let processed_ms = now_ms();
    let latency_ms = processed_ms.saturating_sub(meta.created_ms);
    let processing = ProverReportProcessing {
        terminal: !retryable,
        retry_not_before_ms: retryable
            .then(|| processed_ms.saturating_add(processing_retry_delay_ms(retry_count))),
        retry_count: retryable.then_some(retry_count).unwrap_or(0),
        completed_proof_indices: retryable
            .then_some(completed_proof_indices)
            .unwrap_or_default(),
        processing_context_hash: retryable.then_some(processing_context_hash).flatten(),
    };
    let rep = ProverReport {
        id: loc.id.clone(),
        ok,
        error: err,
        content_type: meta.content_type,
        size: body_load.observed_size,
        created_ms: meta.created_ms,
        processed_ms,
        latency_ms,
        zk1_tags,
        backend,
        vk_ref,
        proof_hash,
        circuit_id,
        proofs,
        processing: Some(processing),
    };
    let receipt = processing_receipt_from_report(&rep)
        .expect("new prover reports always carry a valid processing disposition");
    if !receipt.terminal
        && let Err(error) = persist_prover_processing_receipt_if_referenced(&receipt)
    {
        iroha_logger::warn!(
            attachment_id = %rep.id,
            %error,
            "Failed to checkpoint successful sibling proofs before retry report persistence"
        );
    }
    match save_report(&rep) {
        Ok(()) => match persist_prover_processing_receipt_if_referenced(&receipt) {
            Ok(true) => {}
            Ok(false) => {
                iroha_logger::debug!(
                    attachment_id = %rep.id,
                    "Skipping durable ZK prover receipt because no live attachment reference remains"
                );
            }
            Err(error) => {
                iroha_logger::warn!(
                    attachment_id = %rep.id,
                    %error,
                    "Failed to finalize ZK prover processing receipt"
                );
            }
        },
        Err(error) => {
            iroha_logger::warn!(
                attachment_id = %rep.id,
                %error,
                "Failed to persist ZK prover report; provisional retry receipt remains active"
            );
        }
    }
    record_prover_metrics(&rep);
    Some(rep)
}
/// Scan one bounded attachment-discovery window, generating missing reports.
#[derive(Debug, Clone, Default)]
struct ScanStats {
    processed_reports: usize,
    bytes_processed: u64,
    duration_ms: u64,
    remaining_pending: u64,
    budget_exhausted: Option<&'static str>,
}
async fn run_budgeted_scan() -> ScanStats {
    ensure_dirs();
    let telemetry = telemetry_handle();
    let max_bytes = cfg_max_scan_bytes();
    let max_millis = cfg_max_scan_millis();
    let max_inflight = cfg_max_inflight();
    let start = std::time::Instant::now();
    // Reserve half of the wall-clock budget for loading and scheduling the
    // bounded window. Otherwise a large directory can consume the entire
    // deadline repeatedly without allowing any discovered item to progress.
    let discovery_max_millis = max_millis.div_ceil(2).max(1);
    let discovery = discover_pending_attachment_locations(
        AttachmentDiscoveryGeometry::from_scan_bytes(max_bytes),
        start,
        discovery_max_millis,
    );
    let mut remaining = discovery.pending_estimate();
    let discovery_budget_reason = discovery.budget_reason();
    let discovery_work_exhausted = discovery_budget_reason == Some("work");
    let discovery_time_exhausted = discovery_budget_reason == Some("time");
    let mut budget_reason = scan_deadline_reached(start, max_millis).then_some("time");
    let mut pending = discovery.locations.into_iter();
    telemetry.with_metrics(|tel| tel.set_torii_zk_prover_pending(remaining));
    let semaphore = Arc::new(Semaphore::new(max_inflight));
    let inflight = Arc::new(AtomicU64::new(0));
    let mut byte_deferred = false;
    let mut bytes_processed = 0u64;
    let mut processed_reports = 0usize;
    let mut join_set = JoinSet::new();
    let mut retry_locations = Vec::new();
    while let Some(loc) = pending.next() {
        while join_set.len() >= max_inflight {
            let Some(res) = join_set.join_next().await else {
                break;
            };
            match res {
                Ok(Ok(true)) => processed_reports += 1,
                Ok(Ok(false)) => {}
                Ok(Err(err)) => {
                    iroha_logger::warn!(%err, "Background prover attachment processing failed");
                }
                Err(err) => {
                    iroha_logger::warn!(%err, "Background prover task join failed");
                }
            }
            if scan_deadline_reached(start, max_millis) {
                budget_reason = Some("time");
                break;
            }
        }
        if budget_reason.is_some() {
            retry_locations.push(loc);
            retry_locations.extend(pending);
            break;
        }
        if scan_deadline_reached(start, max_millis) {
            budget_reason = Some("time");
            retry_locations.push(loc);
            retry_locations.extend(pending);
            break;
        }
        let remaining_read_budget = max_bytes.saturating_sub(bytes_processed);
        let snapshot_loc = loc.clone();
        let snapshot_load = match task::spawn_blocking(move || {
            load_attachment_snapshot(&snapshot_loc, remaining_read_budget)
        })
        .await
        {
            Ok(snapshot_load) => snapshot_load,
            Err(error) => {
                iroha_logger::warn!(%error, "Background prover snapshot load failed");
                retry_locations.push(loc);
                continue;
            }
        };
        let crossed_time_budget = scan_deadline_reached(start, max_millis);
        let Some(snapshot_load) = snapshot_load else {
            remaining = remaining.saturating_sub(1);
            telemetry.with_metrics(|tel| tel.set_torii_zk_prover_pending(remaining));
            if crossed_time_budget {
                budget_reason = Some("time");
                retry_locations.extend(pending);
                break;
            }
            continue;
        };
        let snapshot = match snapshot_load {
            AttachmentSnapshotLoad::Ready(snapshot) => snapshot,
            AttachmentSnapshotLoad::DeferredForByteBudget { required_bytes } => {
                let _ = required_bytes;
                byte_deferred = true;
                retry_locations.push(loc);
                if crossed_time_budget {
                    budget_reason = Some("time");
                    retry_locations.extend(pending);
                    break;
                }
                // Do not let one large entry head-of-line block smaller later
                // entries that still fit the aggregate read budget.
                continue;
            }
        };
        let bytes_read = snapshot.body_load.bytes_read;
        if bytes_read > remaining_read_budget {
            iroha_logger::error!(
                bytes_read,
                remaining_read_budget,
                "bounded attachment snapshot exceeded its assigned read budget"
            );
            byte_deferred = true;
            retry_locations.push(loc);
            continue;
        }
        bytes_processed = bytes_processed.saturating_add(bytes_read);
        remaining = remaining.saturating_sub(1);
        telemetry.with_metrics(|tel| tel.set_torii_zk_prover_pending(remaining));
        let permit = match semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                retry_locations.push(loc);
                retry_locations.extend(pending);
                break;
            }
        };
        let inflight = inflight.clone();
        let telemetry_clone = telemetry.clone();
        let loc_owned = loc;
        join_set.spawn(async move {
            let prev = inflight.fetch_add(1, Ordering::SeqCst) + 1;
            telemetry_clone.with_metrics(|tel| tel.set_torii_zk_prover_inflight(prev));
            #[cfg(test)]
            {
                MAX_INFLIGHT_OBSERVED.fetch_max(prev as usize, AtomicOrdering::SeqCst);
            }
            let result =
                task::spawn_blocking(move || process_attachment_snapshot_at(&loc_owned, snapshot))
                    .await
                    .map_err(|err| err.to_string())?;
            drop(permit);
            let after = inflight.fetch_sub(1, Ordering::SeqCst) - 1;
            telemetry_clone.with_metrics(|tel| tel.set_torii_zk_prover_inflight(after));
            Ok::<_, String>(result.is_some())
        });
        if crossed_time_budget {
            // The body bytes have already been read and charged. Complete
            // this immutable snapshot once, then stop scheduling new work.
            budget_reason = Some("time");
            retry_locations.extend(pending);
            break;
        }
    }
    retry_pending_attachment_locations(retry_locations);
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(true)) => processed_reports += 1,
            Ok(Ok(false)) => {}
            Ok(Err(err)) => {
                iroha_logger::warn!(%err, "Background prover attachment processing failed");
            }
            Err(err) => {
                iroha_logger::warn!(%err, "Background prover task join failed");
            }
        }
    }
    if budget_reason.is_none() {
        if byte_deferred {
            budget_reason = Some("bytes");
        } else if discovery_time_exhausted {
            budget_reason = Some("time");
        } else if discovery_work_exhausted {
            budget_reason = Some("work");
        }
    }
    telemetry.with_metrics(|tel| {
        tel.set_torii_zk_prover_inflight(0);
        tel.set_torii_zk_prover_pending(remaining);
        tel.record_torii_zk_prover_scan(bytes_processed, start.elapsed().as_millis() as u64);
    });
    if let Some(reason) = budget_reason {
        telemetry.with_metrics(|tel| tel.inc_torii_zk_prover_budget_exhausted(reason));
    }
    ScanStats {
        processed_reports,
        bytes_processed,
        duration_ms: start.elapsed().as_millis() as u64,
        remaining_pending: remaining,
        budget_exhausted: budget_reason,
    }
}
fn block_on_scan() -> ScanStats {
    Handle::try_current().map_or_else(
        |_| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("create runtime")
                .block_on(run_budgeted_scan())
        },
        |handle| match handle.runtime_flavor() {
            RuntimeFlavor::MultiThread => {
                let handle = handle.clone();
                task::block_in_place(|| handle.block_on(run_budgeted_scan()))
            }
            RuntimeFlavor::CurrentThread => {
                drop(handle);
                thread::spawn(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("create runtime")
                        .block_on(run_budgeted_scan())
                })
                .join()
                .expect("run_budgeted_scan panicked")
            }
            _ => {
                // Future runtime flavors fallback to multi-thread semantics.
                let handle = handle.clone();
                task::block_in_place(|| handle.block_on(run_budgeted_scan()))
            }
        },
    )
}
/// Run a single scan synchronously, returning the number of new reports created.
pub fn scan_once() -> usize {
    block_on_scan().processed_reports
}
/// Start background scan worker when enabled. No-op if disabled.
pub fn start_worker() {
    if !cfg_enabled() {
        return;
    }
    ensure_dirs();
    let period = cfg_scan_period();
    tokio::spawn(async move {
        loop {
            let stats = run_budgeted_scan().await;
            if let Some(reason) = stats.budget_exhausted {
                iroha_logger::warn!(%reason, processed = stats.processed_reports, bytes = stats.bytes_processed, "Background prover scan hit budget");
            }
            let _ = task::spawn_blocking(gc_reports_once).await;
            tokio::time::sleep(period).await;
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestDataDirGuard;
    use iroha_core::zk::test_utils::{FixtureEnvelope, halo2_ivm_execution_envelope};
    use iroha_data_model::proof::{ProofAttachment, ProofBox};
    const TEST_SCAN_BUDGET_MARGIN_BYTES: u64 = 1024;
    #[cfg(any(unix, windows))]
    #[test]
    fn verifying_key_file_read_accepts_v1_limit_and_rejects_first_overflow_byte() {
        let directory = tempfile::tempdir().expect("temporary verifying-key directory");
        let id = VerifyingKeyId::new("halo2/ipa", "bounded-vk-read");
        let path = vk_store_path(directory.path(), &id);
        let limit = u64::try_from(VERIFYING_KEY_BOX_MAX_PAYLOAD_BYTES_V1)
            .expect("V1 verifying-key byte ceiling fits u64");
        let file = fs::File::create(&path).expect("create exact-bound sparse verifying key");
        file.set_len(limit)
            .expect("size exact-bound sparse verifying key");
        drop(file);
        let exact = load_vk_bytes(directory.path(), &id)
            .expect("an exact-bound direct verifying-key file is accepted");
        assert_eq!(u64::try_from(exact.len()).expect("length fits u64"), limit);
        drop(exact);
        let file = fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("reopen verifying key for overflow fixture");
        file.set_len(limit.saturating_add(1))
            .expect("size overflowing sparse verifying key");
        drop(file);
        let error = load_vk_bytes(directory.path(), &id)
            .expect_err("the first byte beyond the V1 ceiling must fail before allocation");
        assert!(
            error.contains("bounded verifying key bytes"),
            "unexpected overflow rejection: {error}"
        );
    }
    #[test]
    fn report_summary_lock_remains_serialized_and_usable_after_writer_panic() {
        let panic = std::thread::spawn(|| {
            let _guard = super::report_summary_lock().lock();
            panic!("intentional report-summary writer panic");
        })
        .join();
        assert!(panic.is_err());
        let _guard = super::report_summary_lock().lock();
    }
    fn configure_test_cfg(allowed_circuits: Vec<String>) {
        configure_test_cfg_with_state(allowed_circuits, fixture_state());
    }
    fn configure_test_cfg_with_state(allowed_circuits: Vec<String>, state: Arc<CoreState>) {
        let fixture_len = fixture_attachment_bytes().len() as u64;
        let max_scan_bytes = fixture_len
            .saturating_add(TEST_SCAN_BUDGET_MARGIN_BYTES)
            .max(ATTACHMENT_DISCOVERY_BYTES_PER_LOCATION.saturating_mul(8));
        configure_test_cfg_with_state_and_scan_bytes(allowed_circuits, state, max_scan_bytes);
    }
    fn configure_test_cfg_with_state_and_scan_bytes(
        allowed_circuits: Vec<String>,
        state: Arc<CoreState>,
        max_scan_bytes: u64,
    ) {
        let _ = super::configure(
            true,
            1,
            7 * 24 * 60 * 60,
            iroha_config::parameters::defaults::torii::ZK_PROVER_REPORTS_MAX_COUNT,
            iroha_config::parameters::defaults::torii::ZK_PROVER_REPORTS_MAX_BYTES,
            2,
            max_scan_bytes,
            5_000,
            iroha_config::parameters::defaults::torii::zk_prover_keys_dir(),
            iroha_config::parameters::defaults::torii::zk_prover_allowed_backends(),
            allowed_circuits,
            Some(state),
            MaybeTelemetry::disabled(),
        );
        super::TEST_PROCESSING_DELAY_MS.store(0, AtomicOrdering::SeqCst);
        super::TEST_SNAPSHOT_LOAD_DELAY_MS.store(0, AtomicOrdering::SeqCst);
        super::MAX_INFLIGHT_OBSERVED.store(0, AtomicOrdering::SeqCst);
    }
    fn init_test_cfg() {
        configure_test_cfg(iroha_config::parameters::defaults::torii::zk_prover_allowed_circuits());
    }
    struct SnapshotLoadDelayReset;
    impl Drop for SnapshotLoadDelayReset {
        fn drop(&mut self) {
            super::TEST_SNAPSHOT_LOAD_DELAY_MS.store(0, AtomicOrdering::SeqCst);
            super::TEST_MAX_SCAN_MILLIS_OVERRIDE.store(0, AtomicOrdering::SeqCst);
        }
    }
    fn attachment_body_id(body: &[u8]) -> String {
        hex::encode::<[u8; 32]>(Hash::new(body).into())
    }
    fn fixture_attachment_provenance(
        body: &[u8],
        content_type: &str,
    ) -> super::super::zk_attachments::AttachmentProvenance {
        super::super::zk_attachments::AttachmentProvenance {
            declared_type: Some(content_type.to_owned()),
            sniffed_type: content_type.to_owned(),
            hashes: super::super::zk_attachments::AttachmentHashes {
                blake2b_256: attachment_body_id(body),
                sha256: hex::encode(Sha256::digest(body)),
            },
            sanitizer: super::super::zk_attachments::AttachmentSanitizerVerdict {
                verdict: "accepted".to_owned(),
                expanded_bytes: body.len() as u64,
                archive_depth: 0,
                sandboxed: false,
            },
        }
    }
    fn corrupt_report_summary(id: &str) {
        fs::create_dir_all(report_index_dir()).expect("create report summary directory");
        fs::write(report_summary_path_from_sanitized(id), "{not json")
            .expect("write malformed report summary");
    }
    #[test]
    fn prover_backend_allowlist_rejects_trusted_setup_labels() {
        let broad_halo2 = ["halo2/".to_owned()];
        for backend in [
            "groth16/bn254",
            "kzg",
            "KZG",
            " kzg ",
            "kzg/ceremony-v1",
            "KZG/ceremony-v1",
            "bn254",
            "BN254",
            "\tBN254\n",
            "bn256",
            "bls12_381",
            "halo2/bn254",
            "halo2/kzg",
            "halo2/ipa:kzg",
            "halo2/ipa:KZG",
            "halo2/ipa: KZG",
        ] {
            assert!(
                !backend_allowed(backend, &broad_halo2),
                "trusted-setup backend {backend} must not pass broad prover allowlists"
            );
        }
        assert!(backend_allowed("halo2/ipa", &broad_halo2));
    }
    #[test]
    fn prover_backend_allowlist_rejects_developer_only_labels() {
        for backend in [
            "debug",
            "Debug",
            "debug-proof",
            "Debug-Proof",
            "debug/ok",
            "halo2/debug",
            "halo2/ipa:debug-proof",
            "halo2/ipa:DEBUG-Proof",
            "stark/fri/debug",
            "stark/fri/Debug",
            "mock",
            "Mock",
            "mock-proof",
            "Mock-Proof",
            "halo2/mock",
            "halo2/ipa:mock-proof",
            "halo2/ipa:Mock-Proof",
            "miden-stark:dev-fixture",
            "zk-trace/mock-proof",
        ] {
            assert!(
                !backend_allowed(backend, &[]),
                "developer-only backend {backend} must not pass even an empty prover allowlist"
            );
        }
        assert!(backend_allowed("halo2/ipa", &[]));
    }
    #[test]
    fn prover_backend_allowlist_rejects_protocol_claimed_and_unregistered_stark_labels() {
        let broad_backends = [
            "halo2/ipa".to_owned(),
            "halo2/pasta".to_owned(),
            "stark/fri".to_owned(),
        ];
        for backend in [
            "halo2/ipa/orchard",
            "halo2-ipa-orchard",
            "groth16/bls12-377",
            "stark/fri/miden",
            "stark/fri/pq-masp-stark-fri",
            "halo2/ipa:release-ready",
            "halo2/ipa:certified-mainnet",
            "halo2/ipa/orchard:production-ready",
            "orchard:mainnet-ready",
            "penumbra-masp:external-security-review",
            "jindo-lattice-pcs-zk:release-ready",
            "sis-hints-anoncred-pq-v0",
            "sis-with-hints",
            "sis-with-hints:s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
            "stark/fri/boi-audited",
            "stark/fri/external-security-review",
            "stark/fri/latest",
            "stark/fri/random-profile",
            "stark/fri/sha512-goldilocks",
            "stark/fri/poseidon2-goldilocks/extra",
            "stark/fri-v2",
            "halo2/unknown-native-v1",
            "halo2/ipa:ivm-execution-v1",
            "halo2/ipa:tiny-add-public",
            "halo2/pasta/tiny-add",
            "halo2/pasta/ivm-execution-v2",
            "halo2/pasta/unknown-native-v1",
        ] {
            assert!(
                !backend_allowed(backend, &[]),
                "unsafe backend {backend} must not pass an empty prover allowlist"
            );
            assert!(
                !backend_allowed(backend, &broad_backends),
                "unsafe backend {backend} must not pass broad prover allowlists"
            );
        }
        for backend in [
            "halo2/ipa",
            "halo2/pasta/ivm-execution-v1",
            "stark/fri/poseidon-x7-goldilocks-6x64-v1",
        ] {
            assert!(
                backend_allowed(backend, &[]),
                "registry backend {backend} should pass an empty prover allowlist"
            );
            assert!(
                backend_allowed(backend, &broad_backends),
                "registry backend {backend} should pass matching broad prover allowlists"
            );
        }
    }
    fn fixture_envelope() -> FixtureEnvelope {
        static FIXTURE: OnceLock<FixtureEnvelope> = OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                halo2_ivm_execution_envelope(
                    Hash::new(b"torii-prover-fixture/code"),
                    Hash::new(b"torii-prover-fixture/overlay"),
                    Hash::new(b"torii-prover-fixture/events"),
                    Hash::new(b"torii-prover-fixture/gas-policy"),
                )
            })
            .clone()
    }
    fn fixture_attachment() -> ProofAttachment {
        let fixture = fixture_envelope();
        let vk = fixture.vk_box("halo2/ipa").expect("fixture vk bytes");
        let vk_commitment = hash_vk(&vk);
        let proof = fixture.proof_box("halo2/ipa");
        let vk_id = VerifyingKeyId::new("halo2/ipa", iroha_core::zk::IVM_EXECUTION_V1_CIRCUIT_ID);
        let mut attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof, vk_id);
        attachment.vk_commitment = Some(vk_commitment);
        attachment
    }
    fn fixture_attachment_bytes() -> Vec<u8> {
        norito::encode_canonical(&fixture_attachment()).expect("canonical proof attachment bytes")
    }
    #[test]
    fn json_attachment_ingress_rejects_legacy_array_surface() {
        let attachment = fixture_attachment();
        let single_json =
            norito::json::to_json(&attachment).expect("serialize single proof attachment");
        assert_eq!(
            decode_proof_attachments("application/json", single_json.as_bytes())
                .expect("single proof attachment object must remain accepted"),
            vec![attachment]
        );
        let array_json = format!("[{single_json}]");
        let error = decode_proof_attachments("application/json", array_json.as_bytes())
            .expect_err("legacy JSON proof-attachment arrays must be rejected");
        assert!(
            error.contains("canonical list") && error.contains("single"),
            "unexpected JSON array rejection: {error}"
        );
        assert!(
            !error.contains("vec:"),
            "legacy Vec decoder leaked into the accepted JSON surface: {error}"
        );
    }
    #[test]
    fn attachment_ingress_uses_exact_media_types_and_canonical_norito() {
        let attachment = fixture_attachment();
        let canonical =
            norito::encode_canonical(&attachment).expect("canonical proof attachment bytes");
        for content_type in ["application/x-norito", " Application/X-Norito\t"] {
            assert_eq!(
                decode_proof_attachments(content_type, &canonical)
                    .expect("canonical Norito attachment must decode"),
                vec![attachment.clone()],
                "content type {content_type}"
            );
        }
        for content_type in [
            "",
            "application/octet-stream",
            "text/application/x-norito",
            "application/x-norito-suffix",
            "application/x-norito; charset=binary",
            "application/json, application/x-norito",
            "application/json;",
            "application/json; charset=utf-16",
            "application/json; charset=utf-8; charset=utf-8",
            "application/json; q=1, text/plain",
            "text/plain",
        ] {
            let error = decode_proof_attachments(content_type, &canonical)
                .expect_err("ambiguous or unsupported media type must fail closed");
            assert!(
                error.contains("unsupported proof attachment content type"),
                "unexpected {content_type} rejection: {error}"
            );
        }
        let mut noncanonical = canonical.clone();
        let last = noncanonical
            .last_mut()
            .expect("canonical attachment frame is non-empty");
        *last ^= 1;
        let error = decode_proof_attachments("application/x-norito", &noncanonical)
            .expect_err("mutated canonical frame must fail closed");
        assert!(
            error.contains("norito decode error"),
            "unexpected error: {error}"
        );
        let list = ProofAttachmentList::try_from(vec![attachment.clone(), attachment.clone()])
            .expect("two attachments are a valid bounded proof list");
        let list_frame =
            norito::encode_canonical(&list).expect("canonical proof attachment list bytes");
        assert_eq!(
            decode_proof_attachments("application/x-norito", &list_frame)
                .expect("canonical binary list must decode"),
            vec![attachment.clone(), attachment.clone()]
        );
        let list_json = norito::json::to_json(&list).expect("canonical list JSON");
        assert_eq!(
            decode_proof_attachments("application/json; charset=UTF-8", list_json.as_bytes())
                .expect("canonical base64 JSON list must decode"),
            vec![attachment.clone(), attachment]
        );
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate_single = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::core::to_bytes(&fixture_attachment())
                .expect("valid alternate-layout single attachment")
        };
        assert_ne!(alternate_single, canonical);
        norito::decode_from_bytes::<ProofAttachment>(&alternate_single)
            .expect("permissive decoder establishes alternate frame validity");
        let error = decode_proof_attachments("application/x-norito", &alternate_single)
            .expect_err("alternate-layout single attachment must be rejected");
        assert!(
            error.contains("norito decode error"),
            "unexpected error: {error}"
        );
        let alternate_list = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::core::to_bytes(&list).expect("valid alternate-layout attachment list")
        };
        assert_ne!(alternate_list, list_frame);
        norito::decode_from_bytes::<ProofAttachmentList>(&alternate_list)
            .expect("permissive decoder establishes alternate list validity");
        let error = decode_proof_attachments("application/x-norito", &alternate_list)
            .expect_err("alternate-layout attachment list must be rejected");
        assert!(
            error.contains("norito decode error"),
            "unexpected error: {error}"
        );
        for (label, compressed) in [
            ("single", {
                let mut bytes = Vec::new();
                norito::serialize_into(
                    &mut bytes,
                    &fixture_attachment(),
                    norito::Compression::Zstd,
                )
                .expect("compress single attachment");
                bytes
            }),
            ("list", {
                let mut bytes = Vec::new();
                norito::serialize_into(&mut bytes, &list, norito::Compression::Zstd)
                    .expect("compress attachment list");
                bytes
            }),
        ] {
            let error = decode_proof_attachments("application/x-norito", &compressed)
                .expect_err("compressed Norito must be rejected as non-canonical");
            assert!(
                error.contains("norito decode error"),
                "unexpected compressed-{label} rejection: {error}"
            );
        }
        let oversized = vec![0_u8; PROOF_ATTACHMENT_BODY_MAX_BYTES_V1 as usize + 1];
        let error = decode_proof_attachments("application/x-norito", &oversized)
            .expect_err("oversized proof body must fail before decoding");
        assert!(
            error.contains("first-release limit"),
            "unexpected oversized-body rejection: {error}"
        );
    }
    fn fixture_state_with_vk_window_and_zk(
        activation_height: Option<u64>,
        withdraw_height: Option<u64>,
        configure_zk: impl FnOnce(&mut iroha_config::parameters::actual::Zk),
    ) -> Arc<CoreState> {
        let fixture = fixture_envelope();
        let vk = fixture.vk_box("halo2/ipa").expect("fixture vk bytes");
        let vk_id = VerifyingKeyId::new("halo2/ipa", iroha_core::zk::IVM_EXECUTION_V1_CIRCUIT_ID);
        let vk_commitment = hash_vk(&vk);
        let mut record = iroha_data_model::proof::VerifyingKeyRecord::new_with_owner(
            1,
            iroha_core::zk::IVM_EXECUTION_V1_CIRCUIT_ID,
            None,
            "test",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            iroha_core::zk::ivm_execution_public_inputs_schema_hash(),
            vk_commitment,
        );
        record.vk_len = u32::try_from(vk.bytes.len()).expect("fixture vk length fits");
        record.max_proof_bytes = 1024 * 1024;
        record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        record.activation_height = activation_height;
        record.withdraw_height = withdraw_height;
        record.key = Some(vk);
        let mut world = iroha_core::state::World::new();
        world
            .verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), record);
        world.verifying_keys_by_circuit_mut_for_testing().insert(
            (iroha_core::zk::IVM_EXECUTION_V1_CIRCUIT_ID.into(), 1),
            vk_id,
        );
        let mut state = iroha_core::state::State::new_for_testing(
            world,
            iroha_core::kura::Kura::blank_kura_for_testing(),
            iroha_core::query::store::LiveQueryStore::start_test(),
        );
        let mut zk = state.zk_snapshot();
        configure_zk(&mut zk);
        state
            .set_zk(zk)
            .expect("empty SCCP outbox accepts prover test configuration");
        Arc::new(state)
    }
    fn fixture_state_with_vk_window(
        activation_height: Option<u64>,
        withdraw_height: Option<u64>,
    ) -> Arc<CoreState> {
        fixture_state_with_vk_window_and_zk(activation_height, withdraw_height, |zk| {
            zk.halo2.enabled = true;
        })
    }
    fn fixture_state() -> Arc<CoreState> {
        fixture_state_with_vk_window(None, None)
    }
    #[test]
    fn prover_worker_rejects_verifier_outside_committed_height_window() {
        let ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: Vec::new(),
            allowed_circuits: Vec::new(),
            state: Some(fixture_state_with_vk_window(Some(1), None)),
            verification_attempts: None,
        };
        let report = process_proof_attachment(&ctx, &fixture_attachment());
        let error = report
            .error
            .expect("future verifier must reject at committed height zero");
        assert!(error.contains("verifying key is not active"), "{error}");
    }
    #[test]
    fn prover_worker_does_not_classify_profileless_stark_prefix_as_stark() {
        let ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: Vec::new(),
            allowed_circuits: Vec::new(),
            state: Some(fixture_state()),
            verification_attempts: None,
        };
        let attachment = ProofAttachment::new_ref(
            "stark/fri/".to_owned(),
            ProofBox::new("stark/fri/".to_owned(), vec![0x42]),
            VerifyingKeyId::new("stark/fri/", "profileless"),
        );
        let report = process_proof_attachment(&ctx, &attachment);
        let error = report
            .error
            .expect("profile-less STARK/Fri prefix must reject");
        assert!(
            !error.contains("stark verification is disabled"),
            "profile-less STARK/Fri prefix must not be classified as a STARK backend"
        );
        assert!(error.contains("backend `stark/fri/` not allowed"));
        assert!(
            !error.contains("verifying key not found"),
            "profile-less STARK/Fri prefix must stop before registry lookup: {error}"
        );
        assert!(report.circuit_id.is_none());
    }
    #[test]
    fn prover_worker_rejects_trusted_setup_backend_before_registry_lookup() {
        let ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: vec!["halo2/".to_owned()],
            allowed_circuits: Vec::new(),
            state: Some(fixture_state()),
            verification_attempts: None,
        };
        for backend in ["halo2/kzg", "halo2/ipa:KZG", "halo2/ipa: KZG"] {
            let attachment = ProofAttachment::new_ref(
                backend.to_owned(),
                ProofBox::new(backend.to_owned(), vec![0x42]),
                VerifyingKeyId::new(backend, "trusted-setup"),
            );
            let report = process_proof_attachment(&ctx, &attachment);
            let error = report
                .error
                .expect("trusted-setup backend must reject before verification");
            assert!(error.contains("trusted-setup backend"), "case {backend}");
            assert!(
                !error.contains("verifying key not found"),
                "trusted-setup backend must stop before registry lookup: {error}"
            );
            assert!(report.circuit_id.is_none(), "case {backend}");
        }
    }
    #[test]
    fn prover_worker_rejects_developer_only_backend_before_registry_lookup() {
        let ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: Vec::new(),
            allowed_circuits: Vec::new(),
            state: Some(fixture_state()),
            verification_attempts: None,
        };
        for backend in ["debug/ok", "halo2/ipa:Mock-Proof"] {
            let attachment = ProofAttachment::new_ref(
                backend.to_owned(),
                ProofBox::new(backend.to_owned(), vec![0x42]),
                VerifyingKeyId::new(backend, "developer-only"),
            );
            let report = process_proof_attachment(&ctx, &attachment);
            let error = report
                .error
                .expect("developer-only backend must reject before verification");
            assert!(error.contains("developer-only backend"), "case {backend}");
            assert!(
                !error.contains("verifying key not found"),
                "developer-only backend must stop before registry lookup: {error}"
            );
            assert!(report.circuit_id.is_none(), "case {backend}");
        }
    }
    #[test]
    fn prover_worker_rejects_attachment_backend_mismatch_before_registry_lookup() {
        let ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: Vec::new(),
            allowed_circuits: Vec::new(),
            state: Some(fixture_state()),
            verification_attempts: None,
        };
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".to_owned(),
            ProofBox::new("stark/fri".to_owned(), vec![0x42]),
            VerifyingKeyId::new("halo2/ipa", "tiny-add"),
        );
        let report = process_proof_attachment(&ctx, &attachment);
        let error = report
            .error
            .expect("backend mismatch must reject before registry lookup");
        assert!(error.contains("proof backend does not match attachment backend"));
        assert!(
            !error.contains("verifying key not found"),
            "backend mismatch must stop before registry lookup: {error}"
        );
        assert!(report.circuit_id.is_none());
    }
    #[test]
    fn terminal_proof_error_overrides_retryable_policy_error() {
        let ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: vec!["stark/fri".to_owned()],
            allowed_circuits: Vec::new(),
            state: Some(fixture_state()),
            verification_attempts: None,
        };
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".to_owned(),
            ProofBox::new("stark/fri".to_owned(), vec![0x42]),
            VerifyingKeyId::new("halo2/ipa", "tiny-add"),
        );
        let processed = process_proof_attachment_with_disposition(&ctx, &attachment);
        assert!(!processed.report.ok);
        assert!(
            processed
                .report
                .error
                .as_deref()
                .is_some_and(|error| error.contains("proof backend does not match"))
        );
        assert!(
            !processed.retryable,
            "a terminally malformed proof must not inherit policy retries"
        );
    }
    #[test]
    fn prover_worker_retries_after_halo2_is_reenabled() {
        let attachment = fixture_attachment();
        let disabled_ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: Vec::new(),
            allowed_circuits: Vec::new(),
            state: Some(fixture_state_with_vk_window_and_zk(None, None, |zk| {
                zk.halo2.enabled = false;
            })),
            verification_attempts: None,
        };
        let disabled = process_proof_attachment_with_disposition(&disabled_ctx, &attachment);
        assert!(!disabled.report.ok);
        assert!(
            disabled
                .report
                .error
                .as_deref()
                .is_some_and(|error| error.contains("halo2 verification is disabled"))
        );
        assert!(
            disabled.retryable,
            "a node-configuration gate may be lifted without changing the attachment"
        );

        let undersized_ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: Vec::new(),
            allowed_circuits: Vec::new(),
            state: Some(fixture_state_with_vk_window_and_zk(None, None, |zk| {
                zk.halo2.enabled = true;
                zk.halo2.max_proof_bytes = 0;
            })),
            verification_attempts: None,
        };
        let undersized = process_proof_attachment_with_disposition(&undersized_ctx, &attachment);
        assert!(!undersized.report.ok);
        assert!(
            undersized
                .report
                .error
                .as_deref()
                .is_some_and(|error| error.contains("max_proof_bytes"))
        );
        assert!(
            undersized.retryable,
            "a mutable node size guardrail must not create a terminal receipt"
        );

        let enabled_ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: Vec::new(),
            allowed_circuits: Vec::new(),
            state: Some(fixture_state()),
            verification_attempts: None,
        };
        let enabled = process_proof_attachment_with_disposition(&enabled_ctx, &attachment);
        assert!(
            enabled.report.ok,
            "the same proof must verify once Halo2 is enabled"
        );
        assert!(!enabled.retryable);
    }
    #[test]
    fn prover_worker_still_reports_missing_registry_for_supported_backend() {
        let ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: Vec::new(),
            allowed_circuits: Vec::new(),
            state: Some(fixture_state()),
            verification_attempts: None,
        };
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".to_owned(),
            ProofBox::new("halo2/ipa".to_owned(), vec![0x42]),
            VerifyingKeyId::new("halo2/ipa", "missing-vk"),
        );
        let report = process_proof_attachment(&ctx, &attachment);
        let error = report
            .error
            .expect("supported missing verifier must report registry miss");
        assert!(error.contains("verifying key not found"));
        assert!(report.circuit_id.is_none());
    }
    fn anon_tenant_key() -> String {
        super::super::zk_attachments::AttachmentTenant::anonymous()
            .as_str()
            .to_string()
    }
    fn ensure_tenant_dir(tenant_key: &str) {
        fs::create_dir_all(attachments_root_dir().join(tenant_key))
            .expect("attachments tenant dir");
    }
    fn sample_report(
        id: String,
        ok: bool,
        error: Option<&str>,
        content_type: &str,
        processed_ms: u64,
    ) -> ProverReport {
        ProverReport {
            id,
            ok,
            error: error.map(str::to_owned),
            content_type: content_type.to_owned(),
            size: 64,
            created_ms: processed_ms.saturating_sub(1),
            processed_ms,
            latency_ms: 1,
            zk1_tags: None,
            backend: None,
            vk_ref: None,
            proof_hash: None,
            circuit_id: None,
            proofs: Vec::new(),
            processing: Some(ProverReportProcessing {
                terminal: true,
                retry_not_before_ms: None,
                retry_count: 0,
                completed_proof_indices: Vec::new(),
                processing_context_hash: None,
            }),
        }
    }
    #[test]
    fn prover_report_processing_json_requires_complete_v1_schema() {
        let processing = ProverReportProcessing {
            terminal: true,
            retry_not_before_ms: None,
            retry_count: 0,
            completed_proof_indices: Vec::new(),
            processing_context_hash: None,
        };
        let canonical = json::to_value(&processing).expect("encode exact report disposition");
        assert!(
            canonical
                .get("retry_not_before_ms")
                .is_some_and(norito::json::Value::is_null),
            "terminal retry deadline must be present as explicit null"
        );
        assert!(
            canonical
                .get("completed_proof_indices")
                .and_then(norito::json::Value::as_array)
                .is_some_and(Vec::is_empty),
            "terminal completed-proof cache must be present as an empty array"
        );
        assert!(
            canonical
                .get("processing_context_hash")
                .is_some_and(norito::json::Value::is_null),
            "terminal processing-context hash must be present as explicit null"
        );
        assert_eq!(
            json::from_value::<ProverReportProcessing>(canonical.clone())
                .expect("decode exact report disposition"),
            processing
        );
        for field in [
            "terminal",
            "retry_not_before_ms",
            "retry_count",
            "completed_proof_indices",
            "processing_context_hash",
        ] {
            let mut omitted = canonical.clone();
            omitted
                .as_object_mut()
                .expect("report disposition object")
                .remove(field);
            assert!(
                json::from_value::<ProverReportProcessing>(omitted).is_err(),
                "omitted report disposition field `{field}` must not default"
            );
        }
        let mut unknown = canonical;
        unknown
            .as_object_mut()
            .expect("report disposition object")
            .insert("retired_cache".to_owned(), true.into());
        assert!(
            json::from_value::<ProverReportProcessing>(unknown).is_err(),
            "unknown report disposition fields must fail closed"
        );
    }
    #[test]
    fn prover_report_json_requires_explicit_processing_disposition() {
        let mut report = sample_report("ac".repeat(32), true, None, "application/x-norito", 10);
        report.processing = None;
        let canonical = json::to_value(&report).expect("encode report with null disposition");
        assert!(
            canonical
                .get("processing")
                .is_some_and(norito::json::Value::is_null),
            "absent processing disposition must be represented by an explicit null key"
        );
        assert_eq!(
            json::from_value::<ProverReport>(canonical.clone())
                .expect("decode report with explicit null disposition"),
            report
        );
        let mut omitted = canonical;
        omitted
            .as_object_mut()
            .expect("prover report object")
            .remove("processing");
        assert!(
            json::from_value::<ProverReport>(omitted).is_err(),
            "omitted processing disposition must not default to null"
        );
    }
    #[test]
    fn terminal_receipt_suppression_skips_report_reconciliation() {
        let terminal = ProverProcessingReceipt {
            version: ZK_PROVER_PROCESSING_STATE_VERSION,
            id: "aa".repeat(32),
            processed_ms: 10,
            terminal: true,
            retry_not_before_ms: None,
            retry_count: 0,
            completed_proof_indices: Vec::new(),
            processing_context_hash: None,
        };
        let reconciliations = std::cell::Cell::new(0_u8);
        reconcile_suppressed_report_if_needed(Some(&terminal), || {
            reconciliations.set(reconciliations.get().saturating_add(1));
        });
        assert_eq!(
            reconciliations.get(),
            0,
            "a terminal receipt must not reopen its committed report on every scan"
        );

        let retry = ProverProcessingReceipt {
            terminal: false,
            retry_not_before_ms: Some(20),
            retry_count: 1,
            ..terminal
        };
        reconcile_suppressed_report_if_needed(Some(&retry), || {
            reconciliations.set(reconciliations.get().saturating_add(1));
        });
        assert_eq!(
            reconciliations.get(),
            1,
            "a provisional backoff receipt must still reconcile a crash-committed report"
        );
    }
    #[test]
    fn retry_report_rejects_unbound_or_malformed_completed_proof_cache() {
        let mut report = sample_report(
            "ab".repeat(32),
            false,
            Some("retryable fixture"),
            "application/x-norito",
            10,
        );
        let processing = report.processing.as_mut().expect("processing fixture");
        processing.terminal = false;
        processing.retry_not_before_ms = Some(20);
        processing.retry_count = 1;
        processing.completed_proof_indices = vec![0];
        assert!(
            processing_receipt_from_report(&report).is_none(),
            "a completed proof index without a verifier-context hash is unsafe"
        );

        report
            .processing
            .as_mut()
            .expect("processing fixture")
            .processing_context_hash = Some("A".repeat(64));
        assert!(
            processing_receipt_from_report(&report).is_none(),
            "context hashes must use canonical lowercase hexadecimal"
        );

        let processing = report.processing.as_mut().expect("processing fixture");
        processing.processing_context_hash = Some("a".repeat(64));
        processing.completed_proof_indices = vec![1, 0];
        assert!(
            processing_receipt_from_report(&report).is_none(),
            "completed proof indices must be strictly ordered"
        );

        report
            .processing
            .as_mut()
            .expect("processing fixture")
            .completed_proof_indices = vec![0];
        assert!(processing_receipt_from_report(&report).is_some());
    }
    #[test]
    fn report_index_tracks_save_and_delete() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let id = "f00df00d".repeat(8);
        let report = ProverReport {
            id: id.clone(),
            ok: true,
            error: None,
            content_type: "application/x-norito".to_string(),
            size: 128,
            created_ms: now_ms(),
            processed_ms: now_ms(),
            latency_ms: 0,
            zk1_tags: Some(vec!["PROF".to_string()]),
            backend: Some("halo2/ipa".to_string()),
            vk_ref: None,
            proof_hash: None,
            circuit_id: None,
            proofs: Vec::new(),
            processing: Some(ProverReportProcessing {
                terminal: true,
                retry_not_before_ms: None,
                retry_count: 0,
                completed_proof_indices: Vec::new(),
                processing_context_hash: None,
            }),
        };
        save_report(&report).expect("save report");
        let summaries = load_report_summaries();
        assert!(
            summaries.iter().any(|summary| summary.id == id),
            "saved report should appear in index"
        );
        delete_report_files(&id);
        let summaries = load_report_summaries();
        assert!(
            summaries.iter().all(|summary| summary.id != id),
            "deleted report should be removed from index"
        );
    }
    #[test]
    fn report_summary_upserts_touch_only_the_matching_shard() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let first = sample_report("a1".repeat(32), true, None, "application/json", now_ms());
        let second = sample_report(
            "a2".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            first.processed_ms.saturating_add(1),
        );
        save_report(&first).expect("save first report");
        let first_path = report_summary_path_from_sanitized(&first.id);
        let first_bytes = fs::read(&first_path).expect("read first summary shard");
        save_report(&second).expect("save second report");
        assert_eq!(
            fs::read(&first_path).expect("read unchanged first summary shard"),
            first_bytes,
            "saving another report must not rewrite existing summary shards"
        );
        assert!(report_summary_path_from_sanitized(&second.id).is_file());
        assert!(
            !prover_dir().join("reports_index.json").exists(),
            "the quadratic monolithic report index must not be recreated"
        );
    }
    #[test]
    fn report_summary_bounds_untrusted_variable_length_fields() {
        let mut report = sample_report("a3".repeat(32), false, None, "application/json", now_ms());
        report.error = Some("é".repeat(REPORT_SUMMARY_ERROR_MAX_BYTES));
        report.content_type = "x".repeat(REPORT_SUMMARY_CONTENT_TYPE_MAX_BYTES + 1);
        report.zk1_tags = Some(
            (0..(ZK1_MAX_TLV_COUNT + 1))
                .map(|index| format!("{index:04}-{}", "x".repeat(REPORT_SUMMARY_TAG_MAX_BYTES)))
                .collect(),
        );
        let summary = report_summary_from_report(&report);
        assert!(
            summary.error.as_deref().expect("bounded error").len()
                <= REPORT_SUMMARY_ERROR_MAX_BYTES
        );
        assert!(summary.content_type.len() <= REPORT_SUMMARY_CONTENT_TYPE_MAX_BYTES);
        let tags = summary.zk1_tags.as_ref().expect("bounded tags");
        assert_eq!(tags.len(), ZK1_MAX_TLV_COUNT);
        assert!(
            tags.iter()
                .all(|tag| tag.len() <= REPORT_SUMMARY_TAG_MAX_BYTES)
        );
        let encoded = norito::json::to_json(&summary).expect("encode bounded summary");
        assert!(encoded.len() as u64 <= REPORT_SUMMARY_FILE_MAX_BYTES);
    }
    #[test]
    fn persisted_report_summary_is_rebounded_after_decode() {
        let summary = bound_persisted_report_summary(ProverReportSummary {
            id: "aa".repeat(32),
            ok: false,
            error: Some("é".repeat(REPORT_SUMMARY_ERROR_MAX_BYTES)),
            content_type: "x".repeat(REPORT_SUMMARY_CONTENT_TYPE_MAX_BYTES + 1),
            processed_ms: 1,
            zk1_tags: Some(
                (0..=ZK1_MAX_TLV_COUNT)
                    .map(|index| format!("{index:04}{}", "x".repeat(REPORT_SUMMARY_TAG_MAX_BYTES)))
                    .collect(),
            ),
        });
        assert!(
            summary.error.as_deref().expect("bounded error").len()
                <= REPORT_SUMMARY_ERROR_MAX_BYTES
        );
        assert!(summary.content_type.len() <= REPORT_SUMMARY_CONTENT_TYPE_MAX_BYTES);
        assert_eq!(
            summary.zk1_tags.as_ref().expect("bounded tags").len(),
            ZK1_MAX_TLV_COUNT
        );
    }
    fn encoded_report_store_bytes(report: &ProverReport) -> u64 {
        let report_bytes = norito::json::to_json_pretty(report)
            .expect("encode report")
            .len() as u64;
        let summary_bytes = norito::json::to_json(&report_summary_from_report(report))
            .expect("encode summary")
            .len() as u64;
        report_bytes.saturating_add(summary_bytes)
    }
    #[test]
    fn report_store_count_limit_evicts_the_oldest_report_on_append() {
        let _env = TestDataDirGuard::new();
        let first = sample_report("a4".repeat(32), true, None, "application/json", 1);
        let second = sample_report("a5".repeat(32), true, None, "application/json", 2);
        let third = sample_report("a6".repeat(32), true, None, "application/json", 3);
        save_report_with_limits(&first, 2, u64::MAX).expect("save first report");
        save_report_with_limits(&second, 2, u64::MAX).expect("save second report");
        save_report_with_limits(&third, 2, u64::MAX).expect("save third report");
        assert!(
            load_report(&first.id).is_none(),
            "oldest report must be evicted"
        );
        assert!(load_report(&second.id).is_some());
        assert!(load_report(&third.id).is_some());
    }
    #[test]
    fn report_store_byte_limit_evicts_before_persisting_the_new_report() {
        let _env = TestDataDirGuard::new();
        let first = sample_report("a7".repeat(32), true, None, "application/json", 1);
        let mut second = sample_report("a8".repeat(32), false, None, "application/json", 2);
        second.error = Some("bounded verifier failure".repeat(16));
        let byte_limit = encoded_report_store_bytes(&first)
            .saturating_add(encoded_report_store_bytes(&second))
            .saturating_sub(1);
        save_report_with_limits(&first, 10, byte_limit).expect("save first report");
        save_report_with_limits(&second, 10, byte_limit).expect("save second report");
        assert!(
            load_report(&first.id).is_none(),
            "old bytes must be reclaimed"
        );
        assert!(load_report(&second.id).is_some());
    }
    #[test]
    fn report_store_rejects_an_item_larger_than_its_byte_geometry() {
        let _env = TestDataDirGuard::new();
        let report = sample_report("a9".repeat(32), true, None, "application/json", 1);
        let required = encoded_report_store_bytes(&report);
        let error = save_report_with_limits(&report, 1, required.saturating_sub(1))
            .expect_err("an individually impossible report must be rejected");
        assert_eq!(error.kind(), IoErrorKind::InvalidInput);
        assert!(!report_path_from_sanitized(&report.id).exists());
    }
    #[test]
    fn delete_report_files_prunes_stale_index_entry_when_file_is_missing() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let id = "f1".repeat(32);
        persist_report_summaries_locked(&[ProverReportSummary {
            id: id.clone(),
            ok: true,
            error: None,
            content_type: "application/json".to_string(),
            processed_ms: now_ms(),
            zk1_tags: None,
        }])
        .expect("persist stale index");
        delete_report_files(&id);
        let persisted = read_report_summaries_locked();
        assert!(persisted.is_empty(), "stale summary should be removed");
    }
    #[test]
    fn delete_report_files_ignores_invalid_id_and_preserves_existing_reports() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let report = sample_report("f2".repeat(32), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");
        delete_report_files("../bad");
        assert!(
            load_report(&report.id).is_some(),
            "invalid delete should not remove valid reports"
        );
        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, report.id);
    }
    #[test]
    fn remove_report_summary_ignores_invalid_and_missing_ids() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let report = sample_report("f3".repeat(32), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");
        remove_report_summary("../bad");
        remove_report_summary(&"f4".repeat(32));
        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, report.id);
    }
    #[test]
    fn load_report_rejects_oversized_report_file() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        ensure_dirs();
        let id = "ab".repeat(32);
        let path = report_path_from_sanitized(&id);
        std::fs::write(&path, vec![b'x'; (REPORT_FILE_MAX_BYTES as usize) + 1])
            .expect("write oversized report");
        assert!(
            load_report(&id).is_none(),
            "oversized report must be rejected"
        );
    }
    #[test]
    fn load_report_rejects_non_utf8_report_file() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        ensure_dirs();
        let id = "ac".repeat(32);
        fs::write(report_path_from_sanitized(&id), [0xff, 0xfe, 0xfd]).expect("write report");
        assert!(
            load_report(&id).is_none(),
            "non-utf8 report payload must be rejected"
        );
    }
    #[test]
    fn load_report_rejects_malformed_report_json() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        ensure_dirs();
        let id = "ad".repeat(32);
        fs::write(report_path_from_sanitized(&id), "{not json").expect("write report");
        assert!(
            load_report(&id).is_none(),
            "malformed report json must be rejected"
        );
    }
    #[test]
    fn load_report_returns_none_for_invalid_id() {
        assert!(load_report("../bad").is_none());
    }
    #[test]
    fn normalize_report_summaries_drops_invalid_ids_and_keeps_last_duplicate() {
        let dup_id = "cd".repeat(32);
        let normalized = normalize_report_summaries(vec![
            ProverReportSummary {
                id: "bad".to_string(),
                ok: false,
                error: Some("invalid".to_string()),
                content_type: "application/json".to_string(),
                processed_ms: 1,
                zk1_tags: None,
            },
            ProverReportSummary {
                id: dup_id.to_ascii_uppercase(),
                ok: true,
                error: None,
                content_type: "application/json".to_string(),
                processed_ms: 2,
                zk1_tags: None,
            },
            ProverReportSummary {
                id: dup_id.clone(),
                ok: false,
                error: Some("latest".to_string()),
                content_type: "application/x-zk1".to_string(),
                processed_ms: 3,
                zk1_tags: Some(vec!["PROF".to_string()]),
            },
        ]);
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].id, dup_id);
        assert!(!normalized[0].ok);
        assert_eq!(normalized[0].error.as_deref(), Some("latest"));
        assert_eq!(normalized[0].content_type, "application/x-zk1");
        assert_eq!(
            normalized[0].zk1_tags.as_deref(),
            Some(&["PROF".to_string()][..])
        );
    }
    #[test]
    fn load_report_summaries_rebuilds_when_report_summary_is_malformed() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let report = sample_report("bb".repeat(32), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");
        corrupt_report_summary(&report.id);
        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, report.id);
        let persisted = read_report_summaries_locked();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].id, report.id);
    }
    #[test]
    fn load_report_summaries_rebuilds_empty_index_when_no_reports_exist() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let summaries = load_report_summaries();
        assert!(summaries.is_empty());
        let persisted = read_report_summaries_locked();
        assert!(persisted.is_empty());
    }
    #[test]
    fn load_report_summaries_prunes_missing_report_files_from_index() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let keep_id = "33".repeat(32);
        let missing_id = "44".repeat(32);
        let keep = sample_report(keep_id.clone(), true, None, "application/json", now_ms());
        save_report(&keep).expect("save kept report");
        persist_report_summaries_locked(&[
            report_summary_from_report(&keep),
            ProverReportSummary {
                id: missing_id,
                ok: false,
                error: Some("missing".to_string()),
                content_type: "application/x-zk1".to_string(),
                processed_ms: keep.processed_ms.saturating_add(1),
                zk1_tags: Some(vec!["PROF".to_string()]),
            },
        ])
        .expect("persist stale index");
        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, keep_id);
        let persisted = read_report_summaries_locked();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].id, keep_id);
    }
    #[test]
    fn load_report_summaries_normalizes_valid_index_entries_and_deduplicates_ids() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let id = "45".repeat(32);
        let report = sample_report(id.clone(), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");
        persist_report_summaries_locked(&[
            ProverReportSummary {
                id: "bad".to_string(),
                ok: false,
                error: Some("invalid".to_string()),
                content_type: "application/x-zk1".to_string(),
                processed_ms: report.processed_ms.saturating_sub(1),
                zk1_tags: Some(vec!["BAD".to_string()]),
            },
            ProverReportSummary {
                id: id.to_ascii_uppercase(),
                ok: true,
                error: None,
                content_type: "application/json".to_string(),
                processed_ms: report.processed_ms,
                zk1_tags: None,
            },
            ProverReportSummary {
                id: id.clone(),
                ok: false,
                error: Some("latest".to_string()),
                content_type: "application/x-zk1".to_string(),
                processed_ms: report.processed_ms.saturating_add(1),
                zk1_tags: Some(vec!["PROF".to_string()]),
            },
        ])
        .expect("persist duplicated report index");
        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, id);
        assert!(!summaries[0].ok);
        assert_eq!(summaries[0].error.as_deref(), Some("latest"));
        assert_eq!(summaries[0].content_type, "application/x-zk1");
        assert_eq!(
            summaries[0].zk1_tags.as_deref(),
            Some(&["PROF".to_string()][..])
        );
        let persisted = read_report_summaries_locked();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].id, id);
    }
    #[test]
    fn save_report_recovers_when_existing_summary_is_malformed() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let first = sample_report("d1".repeat(32), true, None, "application/json", now_ms());
        let second = sample_report(
            "d2".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            first.processed_ms.saturating_add(1),
        );
        save_report(&first).expect("save first report");
        corrupt_report_summary(&first.id);
        save_report(&second).expect("save second report");
        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 2);
        assert!(summaries.iter().any(|summary| summary.id == first.id));
        assert!(summaries.iter().any(|summary| summary.id == second.id));
    }
    #[test]
    fn load_report_normalizes_persisted_uppercase_id() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        ensure_dirs();
        let id = "ef".repeat(32);
        let persisted = sample_report(
            id.to_ascii_uppercase(),
            true,
            None,
            "application/json",
            now_ms(),
        );
        fs::write(
            report_path_from_sanitized(&id),
            norito::json::to_json_pretty(&persisted).expect("report json"),
        )
        .expect("write report");
        let loaded = load_report(&id.to_ascii_uppercase()).expect("load report");
        assert_eq!(loaded.id, id);
    }
    #[test]
    fn save_report_rejects_invalid_id() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let err = save_report(&sample_report(
            "bad".to_string(),
            true,
            None,
            "application/json",
            now_ms(),
        ))
        .expect_err("invalid report id should be rejected");
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
    }
    #[test]
    fn save_report_updates_existing_summary_without_duplicates() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let id = "ca".repeat(32);
        let first = sample_report(id.clone(), true, None, "application/json", now_ms());
        let mut updated = sample_report(
            id.clone(),
            false,
            Some("verification failed"),
            "application/x-zk1",
            first.processed_ms.saturating_add(10),
        );
        updated.zk1_tags = Some(vec!["PROF".to_string()]);
        save_report(&first).expect("save initial report");
        save_report(&updated).expect("save updated report");
        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, id);
        assert!(!summaries[0].ok);
        assert_eq!(summaries[0].error.as_deref(), Some("verification failed"));
        assert_eq!(summaries[0].content_type, "application/x-zk1");
        assert_eq!(summaries[0].processed_ms, updated.processed_ms);
        assert_eq!(
            summaries[0].zk1_tags.as_deref(),
            Some(&["PROF".to_string()][..])
        );
        let loaded = load_report(&id).expect("load updated report");
        assert!(!loaded.ok);
        assert_eq!(loaded.error.as_deref(), Some("verification failed"));
        assert_eq!(loaded.processed_ms, updated.processed_ms);
        assert_eq!(loaded.zk1_tags.as_deref(), Some(&["PROF".to_string()][..]));
    }
    #[test]
    fn report_id_visitor_ignores_invalid_entries() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        ensure_dirs();
        let uppercase_id = "AB".repeat(32);
        let clean_id = uppercase_id.to_ascii_lowercase();
        fs::write(report_path_from_sanitized(&clean_id), b"{}").expect("write report file");
        fs::write(reports_dir().join("bad.json"), b"{}").expect("write invalid report id");
        fs::write(reports_dir().join("not-a-report.txt"), b"{}").expect("write non-report file");
        let mut ids = Vec::new();
        visit_report_ids(|id| {
            ids.push(id);
            true
        });
        assert_eq!(ids, vec![clean_id]);
    }
    #[test]
    fn gc_reports_once_deletes_only_expired_reports_and_retains_fresh_index() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let ttl_ms = Duration::from_secs(cfg_reports_ttl_secs()).as_millis() as u64;
        let now = now_ms();
        let fresh_processed_ms = now.saturating_sub(ttl_ms.saturating_div(2));
        let expired = sample_report(
            "10".repeat(32),
            false,
            Some("old"),
            "application/x-zk1",
            now.saturating_sub(ttl_ms.saturating_add(10)),
        );
        let fresh = sample_report(
            "20".repeat(32),
            true,
            None,
            "application/json",
            fresh_processed_ms,
        );
        save_report(&expired).expect("save expired report");
        save_report(&fresh).expect("save fresh report");
        let deleted = gc_reports_once();
        assert_eq!(deleted, 1);
        assert!(
            load_report(&expired.id).is_none(),
            "expired report should be removed"
        );
        assert!(
            load_report(&fresh.id).is_some(),
            "fresh report should remain"
        );
        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, fresh.id);
    }
    #[test]
    fn gc_reports_once_keeps_reports_when_none_are_expired() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let ttl_ms = Duration::from_secs(cfg_reports_ttl_secs()).as_millis() as u64;
        let fresh = sample_report(
            "21".repeat(32),
            true,
            None,
            "application/json",
            now_ms().saturating_sub(ttl_ms.saturating_div(2)),
        );
        save_report(&fresh).expect("save fresh report");
        let deleted = gc_reports_once();
        assert_eq!(deleted, 0);
        assert!(
            load_report(&fresh.id).is_some(),
            "fresh report should remain after gc"
        );
        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, fresh.id);
    }
    #[test]
    fn gc_reports_once_rebuilds_when_report_summary_is_malformed() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let fresh = sample_report("31".repeat(32), true, None, "application/json", now_ms());
        save_report(&fresh).expect("save fresh report");
        corrupt_report_summary(&fresh.id);
        let deleted = gc_reports_once();
        assert_eq!(deleted, 0);
        assert!(
            load_report(&fresh.id).is_some(),
            "fresh report should remain after gc rebuild"
        );
        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, fresh.id);
    }
    #[test]
    fn gc_reports_once_deletes_expired_index_entries_even_when_report_file_is_missing() {
        let _env = TestDataDirGuard::new();
        init_test_cfg();
        let ttl_ms = Duration::from_secs(cfg_reports_ttl_secs()).as_millis() as u64;
        persist_report_summaries_locked(&[ProverReportSummary {
            id: "32".repeat(32),
            ok: false,
            error: Some("expired".to_string()),
            content_type: "application/x-zk1".to_string(),
            processed_ms: now_ms().saturating_sub(ttl_ms.saturating_add(10)),
            zk1_tags: Some(vec!["PROF".to_string()]),
        }])
        .expect("persist stale index");
        let deleted = gc_reports_once();
        assert_eq!(deleted, 1);
        assert!(load_report_summaries().is_empty());
    }
    include!("zk_prover/scanner_tests.rs");
}
