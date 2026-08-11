//! Background, non-consensus ZK prover worker tied to attachments.
//!
//! - Periodically scans `zk_attachments` for new items and produces a report
//!   under `zk_prover/reports/<id>.json` with
//!   `{ id, ok, error, content_type, size, created_ms, processed_ms, latency_ms }`.
//!   Bounded query metadata is persisted independently under
//!   `zk_prover/report_index/<id>.json`, so saving one report never rewrites
//!   metadata for every other report. Report maintenance streams those shards
//!   one at a time; configured count and aggregate-byte retention limits evict
//!   the oldest reports deterministically before a new report is committed.
//!   Attachment discovery likewise retains only a scan-budget-derived window,
//!   resumes its directory cursor across cycles, and canonically orders each
//!   window instead of collecting and sorting the complete tenant population;
//!   unscheduled locations remain in a retry queue with the same hard cap.
//! - This module is strictly app-facing and non-forking. It must not affect consensus.
//! - Enabled and paced via `iroha_config` (torii.zk_prover_enabled, torii.zk_prover_scan_period_secs).
//!
//! The worker verifies `ProofAttachment` payloads (single or list, Norito or JSON)
//! using core backend verifiers and records per-proof metadata. It never mutates WSV.

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

#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
use axum::{extract::Path as AxumPath, http::StatusCode, response::IntoResponse};
use iroha_core::{
    state::{State as CoreState, WorldReadOnly},
    zk::{
        hash_proof, hash_vk, is_developer_only_backend_label, is_trusted_setup_backend_label,
        is_verifier_backend_registry_label_v1, is_verifier_readiness_claim_label, verify_backend,
        verify_backend_with_timing_checked,
    },
};
#[cfg(test)]
use iroha_crypto::Hash;
use iroha_data_model::proof::{
    ProofAttachment, ProofAttachmentList, VerifyingKeyBox, VerifyingKeyId,
};
use mv::storage::StorageReadOnly;
use norito::json;
use parking_lot::{Mutex, RwLock};
#[cfg(test)]
use sha2::{Digest as _, Sha256};
use tokio::{
    runtime::{Handle, RuntimeFlavor},
    sync::Semaphore,
    task::{self, JoinSet},
};

#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
use crate::NoritoQuery;
use crate::{
    routing::MaybeTelemetry,
    zk_attachments::{
        ATTACHMENT_META_FILE_MAX_BYTES, open_attachment_regular_file,
        read_bounded_attachment_regular_file, validate_attachment_body_contract,
        validate_attachment_metadata_contract,
    },
    zk1::{MAX_TLV_COUNT as ZK1_MAX_TLV_COUNT, parse_tags as parse_zk1_tags},
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

fn cfg_enabled() -> bool {
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
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
const REPORT_QUERY_DEFAULT_LIMIT: usize = 100;
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
const REPORT_QUERY_MAX_LIMIT: usize = 1_000;
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
const REPORT_QUERY_MAX_OFFSET: usize = 10_000;
#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
const REPORT_QUERY_MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
static REPORT_SUMMARY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static ATTACHMENT_DISCOVERY_STATE: OnceLock<Mutex<Option<AttachmentDiscoveryState>>> =
    OnceLock::new();

include!("zk_prover/attachment_discovery_and_report_storage.rs");

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

fn prune_stale_report_summaries_locked() {
    let Ok(entries) = fs::read_dir(report_index_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let Some(id) = report_id_from_entry(&entry) else {
            continue;
        };
        if !report_path_from_sanitized(&id).is_file() {
            let _ = fs::remove_file(entry.path());
        }
    }
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
    prune_stale_report_summaries_locked();
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

include!("zk_prover/report_query.rs");

fn scan_deadline_reached(start: std::time::Instant, max_millis: u64) -> bool {
    start.elapsed() >= Duration::from_millis(max_millis)
}

fn canonicalize_attachment_locations(locations: &mut Vec<AttachmentLocation>) {
    locations.sort_unstable();
    let mut seen_ids = HashSet::with_capacity(locations.len());
    locations.retain(|location| seen_ids.insert(location.id.clone()));
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
            if !report_path_from_sanitized(&location.id).exists() {
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
        |location| !report_path_from_sanitized(&location.id).exists(),
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
    let removed = remove_file_if_present(&report_path_from_sanitized(&clean))?;
    let _ = remove_file_if_present(&report_summary_path_from_sanitized(&clean))?;
    Ok(removed)
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
    prune_stale_report_summaries_locked();
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

/// Garbage collect reports older than configured TTL. Returns number of deleted reports.
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
    prune_stale_report_summaries_locked();
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
}

fn backend_allowed(backend: &str, allowlist: &[String]) -> bool {
    !is_trusted_setup_backend_label(backend)
        && !is_developer_only_backend_label(backend)
        && !is_verifier_readiness_claim_label(backend)
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
    fs::read(&path).map_err(|err| {
        format!(
            "failed to read verifying key bytes at {}: {err}",
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

fn process_proof_attachment(ctx: &ProverContext, attachment: &ProofAttachment) -> ProofReportEntry {
    let backend = attachment.backend.clone();
    let backend_str = backend.as_str();
    let proof_hash = Some(hex::encode(hash_proof(&attachment.proof)));
    let mut errors = Vec::new();
    let resolved_vk_ref = attachment.vk_ref.clone();
    let mut circuit_id: Option<String> = None;

    if attachment.proof.backend.as_str() != backend_str {
        errors.push("proof backend does not match attachment backend".into());
    }
    if attachment.proof.bytes.is_empty() {
        errors.push("proof bytes are empty".into());
    }
    if is_trusted_setup_backend_label(backend_str) {
        errors.push(format!(
            "trusted-setup backend `{backend_str}` is not supported"
        ));
    } else if is_developer_only_backend_label(backend_str) {
        errors.push(format!(
            "developer-only backend `{backend_str}` is not supported"
        ));
    } else if !backend_allowed(backend_str, &ctx.allowed_backends) {
        errors.push(format!("backend `{backend_str}` not allowed"));
    }
    if crate::is_stark_fri_v1_backend(backend_str) {
        if let Some(state) = ctx.state.as_ref() {
            if !state.zk_snapshot().stark.enabled {
                errors.push("stark verification is disabled in node configuration".into());
            }
        }
    }

    let vk_id = &attachment.vk_ref;
    if vk_id.backend.as_str() != backend_str {
        errors.push(format!(
            "vk_ref backend `{}` does not match proof backend `{backend_str}`",
            vk_id.backend
        ));
    }
    if !errors.is_empty() {
        return ProofReportEntry {
            backend,
            ok: false,
            error: Some(errors.join("; ")),
            proof_hash,
            vk_ref: Some(resolved_vk_ref),
            circuit_id,
        };
    }

    let mut vk_box: Option<VerifyingKeyBox> = None;
    let state = match ctx.state.as_ref() {
        Some(state) => state,
        None => {
            errors.push("verifying key lookup requires core state".into());
            return ProofReportEntry {
                backend,
                ok: false,
                error: Some(errors.join("; ")),
                proof_hash,
                vk_ref: Some(resolved_vk_ref),
                circuit_id,
            };
        }
    };
    let world = state.world_view();
    let record = match world.verifying_keys().get(vk_id) {
        Some(record) => record.clone(),
        None => {
            errors.push("verifying key not found in registry".into());
            return ProofReportEntry {
                backend,
                ok: false,
                error: Some(errors.join("; ")),
                proof_hash,
                vk_ref: Some(resolved_vk_ref),
                circuit_id,
            };
        }
    };
    if !record.is_active() {
        errors.push("verifying key is not active".into());
    }
    if record.max_proof_bytes > 0 && attachment.proof.bytes.len() > record.max_proof_bytes as usize
    {
        errors.push(format!(
            "proof exceeds max_proof_bytes {}",
            record.max_proof_bytes
        ));
    }
    if let Some(commitment) = attachment.vk_commitment
        && commitment != record.commitment
    {
        errors.push("vk_commitment does not match registry commitment".into());
    }
    circuit_id = Some(record.circuit_id.clone());
    if let Some(key) = record.key.clone() {
        if key.backend.as_str() != backend_str {
            errors.push("verifying key backend does not match proof backend".into());
        } else {
            vk_box = Some(key);
        }
    } else {
        match load_vk_bytes(&ctx.keys_dir, vk_id) {
            Ok(bytes) => {
                if record.vk_len > 0 && bytes.len() != record.vk_len as usize {
                    errors.push(format!(
                        "verifying key length {} does not match registry vk_len {}",
                        bytes.len(),
                        record.vk_len
                    ));
                }
                vk_box = Some(VerifyingKeyBox::new(backend.clone(), bytes));
            }
            Err(err) => errors.push(err),
        }
    }
    if let Some(vk_box) = vk_box.as_ref() {
        if vk_box.bytes.is_empty() {
            errors.push("verifying key bytes are empty".into());
        } else {
            let vk_hash = hash_vk(vk_box);
            if vk_hash != record.commitment {
                errors.push("verifying key bytes do not match registry commitment".into());
            }
        }
    }

    if !ctx.allowed_circuits.is_empty() {
        match circuit_id.as_deref() {
            Some(circuit) if circuit_allowed(circuit, &ctx.allowed_circuits) => {}
            Some(circuit) => errors.push(format!("circuit `{circuit}` not allowed")),
            None => errors.push("circuit_id unavailable for allowlist".into()),
        }
    }

    if errors.is_empty() {
        match vk_box.as_ref() {
            Some(vk_box) => {
                let verified = if let Some(state) = ctx.state.as_ref() {
                    let zk = state.zk_snapshot();
                    verify_backend_with_timing_checked(
                        backend_str,
                        &attachment.proof,
                        Some(vk_box),
                        &zk,
                    )
                    .ok
                } else {
                    verify_backend(backend_str, &attachment.proof, Some(vk_box))
                };
                if !verified {
                    errors.push("verification failed".into());
                }
            }
            None => errors.push("verifying key bytes missing".into()),
        }
    }

    let ok = errors.is_empty();
    ProofReportEntry {
        backend,
        ok,
        error: if ok { None } else { Some(errors.join("; ")) },
        proof_hash,
        vk_ref: Some(resolved_vk_ref),
        circuit_id,
    }
}

/// Process a single attachment id, emitting a report if not present yet.
pub fn process_attachment_once(id: &str) -> Option<ProverReport> {
    let clean = sanitize_attachment_id(id)?;
    let loc = find_attachment_location(&clean)?;
    process_attachment_once_at(&loc)
}

fn process_attachment_once_at(loc: &AttachmentLocation) -> Option<ProverReport> {
    if report_path_from_sanitized(&loc.id).exists() {
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
    // A direct request and the background scan may race. The immutable
    // snapshot is discarded if either side already committed the report.
    if report_path_from_sanitized(&loc.id).exists() {
        return load_report(&loc.id);
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
    };
    let mut proofs: Vec<ProofReportEntry> = Vec::new();
    let (ok, err, backend, vk_ref, proof_hash, circuit_id) =
        match validated_body.and_then(|body| decode_proof_attachments(&meta.content_type, body)) {
            Ok(attachments) => {
                if attachments.is_empty() {
                    (
                        false,
                        Some("empty proof attachment list".into()),
                        None,
                        None,
                        None,
                        None,
                    )
                } else {
                    for attachment in attachments {
                        proofs.push(process_proof_attachment(&ctx, &attachment));
                    }
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
                    (ok, err, backend, vk_ref, proof_hash, circuit_id)
                }
            }
            Err(err) => (false, Some(err), None, None, None, None),
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
    };
    let _ = save_report(&rep);
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

// ---------------- Report-store test adapters ----------------
//
// These helpers used to compile as dormant public HTTP handlers even though
// no production router mounted them. Keep the query/render coverage in unit
// tests without shipping an unauthenticated administrative surface. The
// adapters compile only for unit tests or the explicit integration-test
// feature. Any future report API must be declared in the route catalog with
// one exact, replay-protected authentication policy before it is compiled for
// production.

#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
#[derive(
    Debug, Default, Clone, crate::json_macros::JsonDeserialize, norito::derive::NoritoDeserialize,
)]
/// Optional filters and options for listing prover reports (app-facing API).
pub struct ProverListQuery {
    /// Only successful reports when true.
    pub ok_only: Option<bool>,
    /// Only failed reports when true.
    pub failed_only: Option<bool>,
    /// Exact report id (hex) to match.
    pub id: Option<String>,
    /// Substring match on content type.
    pub content_type: Option<String>,
    /// Require a ZK1 TLV tag to be present. Must be exactly four printable ASCII bytes (e.g., "PROF").
    pub has_tag: Option<String>,
    /// Maximum number of results to return (default 100, hard maximum 1000).
    pub limit: Option<u32>,
    /// Return only reports with processed_ms >= since_ms.
    pub since_ms: Option<u64>,
    /// Return only reports with processed_ms <= before_ms.
    pub before_ms: Option<u64>,
    /// When true, return only report ids (array of strings) instead of full objects.
    pub ids_only: Option<bool>,
    /// Result ordering: "asc" (default) or "desc" by processed_ms.
    pub order: Option<String>,
    /// Offset to apply after ordering and filtering (hard maximum 10000).
    pub offset: Option<u32>,
    /// Convenience: alias for `failed_only=true` (errors are reports with ok=false).
    pub errors_only: Option<bool>,
    /// Projection: when true, return only `{ id, error }` objects for reports with `ok=false`.
    pub messages_only: Option<bool>,
    /// Convenience: when true, return only the latest report (by processed_ms) after filters.
    pub latest: Option<bool>,
}

#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
/// GET /v1/zk/prover/reports — list prover reports with optional filters.
pub async fn handle_list_reports(
    NoritoQuery(q): NoritoQuery<ProverListQuery>,
) -> impl IntoResponse {
    let ok_req = q.ok_only.unwrap_or(false);
    let failed_req = q.failed_only.unwrap_or(false)
        || q.errors_only.unwrap_or(false)
        || q.messages_only.unwrap_or(false);
    if let Err(message) = validate_zk1_tag_filter(&q) {
        return (StatusCode::BAD_REQUEST, message).into_response();
    }
    let requested_id = if let Some(id) = q.id.as_deref() {
        let Some(clean) = sanitize_report_id(id) else {
            return (
                StatusCode::BAD_REQUEST,
                "invalid report id (expected 64 hex characters)",
            )
                .into_response();
        };
        Some(clean)
    } else {
        None
    };

    let filtered = match select_report_summaries(&q, requested_id.as_deref(), ok_req, failed_req) {
        Ok(filtered) => filtered,
        Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
    };
    // If ids_only requested, project to ids only
    let s = if q.ids_only.unwrap_or(false) {
        let ids: Vec<String> = filtered.iter().map(|summary| summary.id.clone()).collect();
        norito::json::to_json_pretty(&ids).unwrap_or_else(|_| "[]".into())
    } else if q.messages_only.unwrap_or(false) {
        // Project to message summaries for failed reports only
        let msgs: Vec<norito::json::Value> = filtered
            .into_iter()
            .filter(|summary| !summary.ok)
            .map(|summary| {
                let mut m = norito::json::Map::new();
                m.insert("id".into(), norito::json::Value::from(summary.id));
                m.insert(
                    "error".into(),
                    summary
                        .error
                        .map(norito::json::Value::from)
                        .unwrap_or(norito::json::Value::Null),
                );
                norito::json::Value::Object(m)
            })
            .collect();
        norito::json::to_json_pretty(&msgs).unwrap_or_else(|_| "[]".into())
    } else {
        match encode_full_report_page(filtered) {
            Ok(body) => body,
            Err(message) => {
                return (StatusCode::PAYLOAD_TOO_LARGE, message).into_response();
            }
        }
    };
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(s))
        .unwrap()
}

#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
/// GET /v1/zk/prover/reports/count — return number of matching prover reports.
pub async fn handle_count_reports(
    NoritoQuery(q): NoritoQuery<ProverListQuery>,
) -> impl IntoResponse {
    let ok_req = q.ok_only.unwrap_or(false);
    let failed_req = q.failed_only.unwrap_or(false) || q.errors_only.unwrap_or(false);
    if let Err(message) = validate_zk1_tag_filter(&q) {
        return (StatusCode::BAD_REQUEST, message).into_response();
    }
    let requested_id = if let Some(id) = q.id.as_deref() {
        let Some(clean) = sanitize_report_id(id) else {
            return (
                StatusCode::BAD_REQUEST,
                "invalid report id (expected 64 hex characters)",
            )
                .into_response();
        };
        Some(clean)
    } else {
        None
    };

    let count = count_report_summaries(&q, requested_id.as_deref(), ok_req, failed_req);
    let body = norito::json::to_json_pretty(&crate::json_object(vec![("count", count)]))
        .unwrap_or_else(|_| "{}".into());
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap()
}

#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
/// DELETE /v1/zk/prover/reports — bulk delete reports matching filters.
pub async fn handle_delete_reports(
    NoritoQuery(q): NoritoQuery<ProverListQuery>,
) -> impl IntoResponse {
    let ok_req = q.ok_only.unwrap_or(false);
    let failed_req = q.failed_only.unwrap_or(false) || q.errors_only.unwrap_or(false);
    if let Err(message) = validate_zk1_tag_filter(&q) {
        return (StatusCode::BAD_REQUEST, message).into_response();
    }
    let requested_id = if let Some(id) = q.id.as_deref() {
        let Some(clean) = sanitize_report_id(id) else {
            return (
                StatusCode::BAD_REQUEST,
                "invalid report id (expected 64 hex characters)",
            )
                .into_response();
        };
        Some(clean)
    } else {
        None
    };
    let mut delete_query = q.clone();
    if delete_query.limit.is_none() {
        delete_query.limit = Some(REPORT_QUERY_MAX_LIMIT as u32);
    }
    let matches =
        match select_report_summaries(&delete_query, requested_id.as_deref(), ok_req, failed_req) {
            Ok(matches) => matches,
            Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
        };

    let mut deleted_ids = Vec::new();
    for summary in matches {
        delete_report_files(&summary.id);
        deleted_ids.push(summary.id);
    }
    let deleted_count = deleted_ids.len() as u64;
    let body = norito::json::to_json_pretty(&crate::json_object(vec![
        crate::json_entry("deleted", deleted_count),
        crate::json_entry("ids", deleted_ids),
    ]))
    .unwrap_or_else(|_| "{}".into());
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap()
}

#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
/// GET /v1/zk/prover/reports/{id} — get a single report by id.
pub async fn handle_get_report(AxumPath(id): AxumPath<String>) -> impl IntoResponse {
    let Some(clean) = sanitize_report_id(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            "invalid report id (expected 64 hex characters)",
        )
            .into_response();
    };
    load_report(&clean).map_or_else(
        || StatusCode::NOT_FOUND.into_response(),
        |r| {
            let s = norito::json::to_json_pretty(&r).unwrap_or_else(|_| "{}".into());
            axum::response::Response::builder()
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(s))
                .unwrap()
        },
    )
}

#[cfg(all(feature = "app_api", any(test, feature = "ws_integration_tests")))]
/// DELETE /v1/zk/prover/reports/{id} — delete a single report by id.
pub async fn handle_delete_report(AxumPath(id): AxumPath<String>) -> impl IntoResponse {
    let Some(clean) = sanitize_report_id(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            "invalid report id (expected 64 hex characters)",
        )
            .into_response();
    };
    let existed = report_path_from_sanitized(&clean).exists();
    delete_report_files(&clean);
    if existed {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[cfg(test)]
mod tests {
    use http_body_util::BodyExt as _;
    use iroha_core::zk::test_utils::{FixtureEnvelope, halo2_ivm_execution_envelope};
    use iroha_data_model::proof::{ProofAttachment, ProofBox};

    use super::*;
    use crate::test_utils::TestDataDirGuard;

    const TEST_SCAN_BUDGET_MARGIN_BYTES: u64 = 1024;

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
        let fixture_len = fixture_attachment_bytes().len() as u64;
        let max_scan_bytes = fixture_len
            .saturating_add(TEST_SCAN_BUDGET_MARGIN_BYTES)
            .max(ATTACHMENT_DISCOVERY_BYTES_PER_LOCATION.saturating_mul(8));
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
            Some(fixture_state()),
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
            "halo2/ipa:ivm-execution-v1",
            "halo2/pasta/ivm-execution-v1",
            "stark/fri",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "stark/fri/sha256_goldilocks.v1",
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

    fn fixture_state() -> Arc<CoreState> {
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
        zk.halo2.enabled = true;
        state
            .set_zk(zk)
            .expect("empty SCCP outbox accepts prover test configuration");
        Arc::new(state)
    }

    #[test]
    fn prover_worker_does_not_classify_profileless_stark_prefix_as_stark() {
        let ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: Vec::new(),
            allowed_circuits: Vec::new(),
            state: Some(fixture_state()),
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
    fn prover_worker_still_reports_missing_registry_for_supported_backend() {
        let ctx = ProverContext {
            keys_dir: PathBuf::new(),
            allowed_backends: Vec::new(),
            allowed_circuits: Vec::new(),
            state: Some(fixture_state()),
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
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn get_report_rejects_invalid_id() {
        let response = axum::response::IntoResponse::into_response(
            super::handle_get_report(axum::extract::Path("../bad".to_string())).await,
        );
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_rejects_invalid_id() {
        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            id: Some("../bad".to_string()),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn count_reports_rejects_invalid_id() {
        let response = super::handle_count_reports(NoritoQuery(ProverListQuery {
            id: Some("../bad".to_string()),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_reports_rejects_invalid_id() {
        let response = super::handle_delete_reports(NoritoQuery(ProverListQuery {
            id: Some("../bad".to_string()),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn get_report_returns_not_found_for_missing_report() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let response = axum::response::IntoResponse::into_response(
            super::handle_get_report(axum::extract::Path("fd".repeat(32))).await,
        );
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn get_report_returns_saved_report_payload() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let report = sample_report(
            "0f".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            now_ms(),
        );
        save_report(&report).expect("save report");

        let response = axum::response::IntoResponse::into_response(
            super::handle_get_report(axum::extract::Path(report.id.to_ascii_uppercase())).await,
        );
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("response body")
            .to_bytes();
        let loaded: ProverReport =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(loaded.id, report.id);
        assert!(!loaded.ok);
        assert_eq!(loaded.error.as_deref(), Some("verification failed"));
        assert_eq!(loaded.content_type, "application/x-zk1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_report_rejects_invalid_id() {
        let response = axum::response::IntoResponse::into_response(
            super::handle_delete_report(axum::extract::Path("../bad".to_string())).await,
        );
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_report_returns_not_found_for_missing_report() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let response = axum::response::IntoResponse::into_response(
            super::handle_delete_report(axum::extract::Path("fe".repeat(32))).await,
        );
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_report_removes_existing_report_and_index() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let report = sample_report("1f".repeat(32), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");

        let response = axum::response::IntoResponse::into_response(
            super::handle_delete_report(axum::extract::Path(report.id.to_ascii_uppercase())).await,
        );
        assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
        assert!(
            load_report(&report.id).is_none(),
            "report should be deleted"
        );
        assert!(
            load_report_summaries()
                .iter()
                .all(|summary| summary.id != report.id),
            "deleted report should be removed from the index"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_report_rebuilds_malformed_index_and_preserves_other_reports() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let first = sample_report("2f".repeat(32), true, None, "application/json", now_ms());
        let second = sample_report(
            "3f".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            first.processed_ms.saturating_add(1),
        );
        save_report(&first).expect("save first report");
        save_report(&second).expect("save second report");
        corrupt_report_summary(&first.id);

        let response = axum::response::IntoResponse::into_response(
            super::handle_delete_report(axum::extract::Path(first.id.clone())).await,
        );
        assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
        assert!(
            load_report(&first.id).is_none(),
            "deleted report should be gone"
        );
        assert!(
            load_report(&second.id).is_some(),
            "other report should remain"
        );

        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, second.id);
    }

    #[test]
    fn report_index_tracks_save_and_delete() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();
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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
    fn bounded_report_key_selection_never_retains_more_than_the_requested_window() {
        let mut keys = BoundedReportKeys::new(false);
        for processed_ms in (0..10_000_u64).rev() {
            keys.consider(
                ReportOrderKey {
                    processed_ms,
                    id: format!("{processed_ms:064x}"),
                },
                3,
            );
        }
        let selected = keys.into_ordered();
        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].processed_ms, 0);
        assert_eq!(selected[2].processed_ms, 2);
    }

    #[test]
    fn report_query_window_caps_limit_and_rejects_offset_overflow() {
        assert_eq!(
            report_query_window(&ProverListQuery::default()).expect("default window"),
            (0, REPORT_QUERY_DEFAULT_LIMIT)
        );
        let capped = report_query_window(&ProverListQuery {
            limit: Some(u32::MAX),
            offset: Some(REPORT_QUERY_MAX_OFFSET as u32),
            ..Default::default()
        })
        .expect("maximum supported offset");
        assert_eq!(capped, (REPORT_QUERY_MAX_OFFSET, REPORT_QUERY_MAX_LIMIT));

        let error = report_query_window(&ProverListQuery {
            offset: Some(REPORT_QUERY_MAX_OFFSET as u32 + 1),
            ..Default::default()
        })
        .expect_err("offset beyond the bounded selection window must fail");
        assert!(error.contains("pagination ceiling"));
    }

    #[test]
    fn delete_report_files_prunes_stale_index_entry_when_file_is_missing() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();
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
        init_test_cfg();
        let _env = TestDataDirGuard::new();
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
        init_test_cfg();
        let _env = TestDataDirGuard::new();
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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let summaries = load_report_summaries();
        assert!(summaries.is_empty());

        let persisted = read_report_summaries_locked();
        assert!(persisted.is_empty());
    }

    #[test]
    fn load_report_summaries_prunes_missing_report_files_from_index() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();
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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();
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
    fn filter_report_summary_applies_requested_id_content_type_tag_and_time_bounds() {
        let id = "db".repeat(32);
        let summary = ProverReportSummary {
            id: id.clone(),
            ok: false,
            error: Some("verification failed".to_string()),
            content_type: "application/x-zk1+json".to_string(),
            processed_ms: 42,
            zk1_tags: Some(vec!["PROF".to_string(), "IPAK".to_string()]),
        };
        let query = ProverListQuery {
            content_type: Some("x-zk1".to_string()),
            has_tag: Some("PROF".to_string()),
            since_ms: Some(42),
            before_ms: Some(42),
            ..Default::default()
        };

        assert!(filter_report_summary(
            &summary,
            &query,
            Some(&id),
            false,
            false,
        ));
        assert!(!filter_report_summary(
            &summary,
            &query,
            Some(&"ef".repeat(32)),
            false,
            false,
        ));
        assert!(!filter_report_summary(
            &summary,
            &ProverListQuery {
                content_type: Some("text/plain".to_string()),
                ..query.clone()
            },
            Some(&id),
            false,
            false,
        ));
        assert!(!filter_report_summary(
            &summary,
            &ProverListQuery {
                has_tag: Some("KIND".to_string()),
                ..query.clone()
            },
            Some(&id),
            false,
            false,
        ));
        assert!(!filter_report_summary(
            &summary,
            &ProverListQuery {
                since_ms: Some(43),
                ..query.clone()
            },
            Some(&id),
            false,
            false,
        ));
        assert!(!filter_report_summary(
            &summary,
            &ProverListQuery {
                before_ms: Some(41),
                ..query
            },
            Some(&id),
            false,
            false,
        ));
    }

    #[test]
    fn validate_zk1_tag_filter_rejects_malformed_tags() {
        assert!(
            validate_zk1_tag_filter(&ProverListQuery {
                has_tag: Some("PROF".to_string()),
                ..Default::default()
            })
            .is_ok()
        );
        for malformed in ["", "ABC", "ABCDE", "AB C", "A\nBC", "éééé"] {
            let err = validate_zk1_tag_filter(&ProverListQuery {
                has_tag: Some(malformed.to_string()),
                ..Default::default()
            })
            .expect_err("malformed tag filter should fail closed");
            assert!(err.contains("invalid ZK1 tag filter"));
        }
    }

    #[test]
    fn filter_report_summary_status_flags_follow_ok_failed_matrix() {
        let ok = ProverReportSummary {
            id: "01".repeat(32),
            ok: true,
            error: None,
            content_type: "application/json".to_string(),
            processed_ms: 10,
            zk1_tags: None,
        };
        let failed = ProverReportSummary {
            id: "02".repeat(32),
            ok: false,
            error: Some("verification failed".to_string()),
            content_type: "application/x-zk1".to_string(),
            processed_ms: 11,
            zk1_tags: Some(vec!["PROF".to_string()]),
        };
        let query = ProverListQuery::default();

        assert!(filter_report_summary(&ok, &query, None, false, false));
        assert!(filter_report_summary(&failed, &query, None, false, false));
        assert!(filter_report_summary(&ok, &query, None, true, false));
        assert!(!filter_report_summary(&failed, &query, None, true, false));
        assert!(!filter_report_summary(&ok, &query, None, false, true));
        assert!(filter_report_summary(&failed, &query, None, false, true));
        assert!(filter_report_summary(&ok, &query, None, true, true));
        assert!(filter_report_summary(&failed, &query, None, true, true));
    }

    #[test]
    fn gc_reports_once_deletes_only_expired_reports_and_retains_fresh_index() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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
        init_test_cfg();
        let _env = TestDataDirGuard::new();

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

    #[tokio::test(flavor = "current_thread")]
    async fn count_reports_filters_using_report_summaries() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();
        let report_with_tag = ProverReport {
            id: "11".repeat(32),
            ok: true,
            error: None,
            content_type: "application/x-zk1".to_string(),
            size: 64,
            created_ms: now_ms(),
            processed_ms: now_ms(),
            latency_ms: 0,
            zk1_tags: Some(vec!["PROF".to_string()]),
            backend: None,
            vk_ref: None,
            proof_hash: None,
            circuit_id: None,
            proofs: Vec::new(),
        };
        let report_without_tag = ProverReport {
            id: "22".repeat(32),
            ok: false,
            error: Some("verification failed".to_string()),
            content_type: "application/x-zk1".to_string(),
            size: 64,
            created_ms: now_ms(),
            processed_ms: now_ms(),
            latency_ms: 0,
            zk1_tags: Some(vec!["IPAK".to_string()]),
            backend: None,
            vk_ref: None,
            proof_hash: None,
            circuit_id: None,
            proofs: Vec::new(),
        };
        save_report(&report_with_tag).expect("save tagged report");
        save_report(&report_without_tag).expect("save untagged report");

        let query = ProverListQuery {
            has_tag: Some("PROF".to_string()),
            ..Default::default()
        };
        let response = super::handle_count_reports(NoritoQuery(query))
            .await
            .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("response body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["count"].as_u64(), Some(1));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn count_reports_failed_only_alias_counts_failed_reports() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let ok = sample_report("14".repeat(32), true, None, "application/json", now_ms());
        let failed = sample_report(
            "15".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            ok.processed_ms.saturating_add(1),
        );
        save_report(&ok).expect("save ok report");
        save_report(&failed).expect("save failed report");

        let response = super::handle_count_reports(NoritoQuery(ProverListQuery {
            failed_only: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["count"].as_u64(), Some(1));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn count_reports_ok_and_errors_filters_together_count_all_reports() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let ok = sample_report("16".repeat(32), true, None, "application/json", now_ms());
        let failed = sample_report(
            "17".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            ok.processed_ms.saturating_add(1),
        );
        save_report(&ok).expect("save ok report");
        save_report(&failed).expect("save failed report");

        let response = super::handle_count_reports(NoritoQuery(ProverListQuery {
            ok_only: Some(true),
            errors_only: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["count"].as_u64(), Some(2));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn count_reports_errors_only_alias_counts_failed_reports() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let ok = sample_report("12".repeat(32), true, None, "application/json", now_ms());
        let failed = sample_report(
            "34".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            ok.processed_ms.saturating_add(1),
        );
        save_report(&ok).expect("save ok report");
        save_report(&failed).expect("save failed report");

        let response = super::handle_count_reports(NoritoQuery(ProverListQuery {
            errors_only: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["count"].as_u64(), Some(1));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn count_reports_id_filter_accepts_uppercase_hex() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let report = sample_report("13".repeat(32), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");

        let response = super::handle_count_reports(NoritoQuery(ProverListQuery {
            id: Some(report.id.to_ascii_uppercase()),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["count"].as_u64(), Some(1));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn count_reports_returns_zero_when_no_reports_match() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let report = sample_report("18".repeat(32), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");

        let response = super::handle_count_reports(NoritoQuery(ProverListQuery {
            content_type: Some("application/x-zk1".to_string()),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["count"].as_u64(), Some(0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn count_reports_filters_by_content_type_tag_and_time_bounds() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let mut target = sample_report(
            "19".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1+json",
            now_ms(),
        );
        target.zk1_tags = Some(vec!["PROF".to_string()]);
        let mut wrong_tag = sample_report(
            "1a".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1+json",
            target.processed_ms.saturating_add(1),
        );
        wrong_tag.zk1_tags = Some(vec!["IPAK".to_string()]);
        let mut wrong_time = sample_report(
            "1b".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1+json",
            target.processed_ms.saturating_add(10),
        );
        wrong_time.zk1_tags = Some(vec!["PROF".to_string()]);
        save_report(&target).expect("save target report");
        save_report(&wrong_tag).expect("save wrong-tag report");
        save_report(&wrong_time).expect("save wrong-time report");

        let response = super::handle_count_reports(NoritoQuery(ProverListQuery {
            content_type: Some("x-zk1".to_string()),
            has_tag: Some("PROF".to_string()),
            since_ms: Some(target.processed_ms),
            before_ms: Some(target.processed_ms),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["count"].as_u64(), Some(1));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_ids_only_takes_precedence_over_messages_only() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let failed = sample_report(
            "55".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            now_ms(),
        );
        let ok = sample_report(
            "66".repeat(32),
            true,
            None,
            "application/json",
            failed.processed_ms.saturating_add(1),
        );
        save_report(&failed).expect("save failed report");
        save_report(&ok).expect("save ok report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            ids_only: Some(true),
            messages_only: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let ids: Vec<String> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(ids, vec![failed.id]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_failed_only_alias_returns_only_failed_ids() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let ok = sample_report("67".repeat(32), true, None, "application/json", now_ms());
        let failed = sample_report(
            "68".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            ok.processed_ms.saturating_add(1),
        );
        save_report(&ok).expect("save ok report");
        save_report(&failed).expect("save failed report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            ids_only: Some(true),
            failed_only: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let ids: Vec<String> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(ids, vec![failed.id]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_id_filter_accepts_uppercase_hex() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let report = sample_report("6c".repeat(32), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            ids_only: Some(true),
            id: Some(report.id.to_ascii_uppercase()),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let ids: Vec<String> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(ids, vec![report.id]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_filters_by_content_type_tag_and_time_bounds() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let mut target = sample_report(
            "69".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1+json",
            now_ms(),
        );
        target.zk1_tags = Some(vec!["PROF".to_string(), "IPAK".to_string()]);
        let mut wrong_tag = sample_report(
            "6a".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1+json",
            target.processed_ms.saturating_add(1),
        );
        wrong_tag.zk1_tags = Some(vec!["IPAK".to_string()]);
        let mut wrong_time = sample_report(
            "6b".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1+json",
            target.processed_ms.saturating_add(10),
        );
        wrong_time.zk1_tags = Some(vec!["PROF".to_string()]);

        save_report(&target).expect("save target report");
        save_report(&wrong_tag).expect("save wrong-tag report");
        save_report(&wrong_time).expect("save wrong-time report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            ids_only: Some(true),
            content_type: Some("x-zk1".to_string()),
            has_tag: Some("PROF".to_string()),
            since_ms: Some(target.processed_ms),
            before_ms: Some(target.processed_ms),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let ids: Vec<String> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(ids, vec![target.id]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_messages_only_preserves_null_error_field() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let report = sample_report("77".repeat(32), false, None, "application/x-zk1", now_ms());
        save_report(&report).expect("save report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            messages_only: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        let arr = parsed.as_array().expect("message array");
        assert_eq!(arr.len(), 1);
        assert_eq!(
            arr[0].get("id").and_then(|v| v.as_str()),
            Some(report.id.as_str())
        );
        assert!(matches!(
            arr[0].get("error"),
            Some(norito::json::Value::Null)
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_messages_only_excludes_successful_reports() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let ok = sample_report("79".repeat(32), true, None, "application/json", now_ms());
        let failed = sample_report(
            "7a".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            ok.processed_ms.saturating_add(1),
        );
        save_report(&ok).expect("save ok report");
        save_report(&failed).expect("save failed report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            ok_only: Some(true),
            messages_only: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        let arr = parsed.as_array().expect("message array");
        assert_eq!(arr.len(), 1);
        assert_eq!(
            arr[0].get("id").and_then(|v| v.as_str()),
            Some(failed.id.as_str())
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_latest_messages_only_returns_latest_failed_message() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let failed_old = sample_report(
            "7b".repeat(32),
            false,
            Some("first failure"),
            "application/x-zk1",
            now_ms(),
        );
        let failed_new = sample_report(
            "7c".repeat(32),
            false,
            Some("second failure"),
            "application/x-zk1",
            failed_old.processed_ms.saturating_add(1),
        );
        let ok_latest = sample_report(
            "7d".repeat(32),
            true,
            None,
            "application/json",
            failed_new.processed_ms.saturating_add(1),
        );
        save_report(&failed_old).expect("save first failed report");
        save_report(&failed_new).expect("save second failed report");
        save_report(&ok_latest).expect("save latest ok report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            messages_only: Some(true),
            latest: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        let arr = parsed.as_array().expect("message array");
        assert_eq!(arr.len(), 1);
        assert_eq!(
            arr[0].get("id").and_then(|v| v.as_str()),
            Some(failed_new.id.as_str())
        );
        assert_eq!(
            arr[0].get("error").and_then(|v| v.as_str()),
            Some("second failure")
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_desc_order_accepts_uppercase_desc() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let first = sample_report("56".repeat(32), true, None, "application/json", now_ms());
        let second = sample_report(
            "78".repeat(32),
            true,
            None,
            "application/json",
            first.processed_ms.saturating_add(1),
        );
        save_report(&first).expect("save first report");
        save_report(&second).expect("save second report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            ids_only: Some(true),
            order: Some("DESC".to_string()),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let ids: Vec<String> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(ids, vec![second.id, first.id]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_desc_order_accepts_mixed_case_desc() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let first = sample_report("59".repeat(32), true, None, "application/json", now_ms());
        let second = sample_report(
            "5d".repeat(32),
            true,
            None,
            "application/json",
            first.processed_ms.saturating_add(1),
        );
        save_report(&first).expect("save first report");
        save_report(&second).expect("save second report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            ids_only: Some(true),
            order: Some("Desc".to_string()),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let ids: Vec<String> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(ids, vec![second.id, first.id]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_latest_returns_latest_full_report() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let first = sample_report("57".repeat(32), true, None, "application/json", now_ms());
        let second = sample_report(
            "58".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            first.processed_ms.saturating_add(1),
        );
        save_report(&first).expect("save first report");
        save_report(&second).expect("save second report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            latest: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let reports: Vec<norito::json::Value> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(reports.len(), 1);
        assert_eq!(
            reports[0].get("id").and_then(|v| v.as_str()),
            Some(second.id.as_str())
        );
        assert_eq!(reports[0].get("ok").and_then(|v| v.as_bool()), Some(false));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_latest_overrides_order_offset_and_limit() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let first = sample_report("5a".repeat(32), true, None, "application/json", now_ms());
        let second = sample_report(
            "5b".repeat(32),
            true,
            None,
            "application/json",
            first.processed_ms.saturating_add(1),
        );
        let third = sample_report(
            "5c".repeat(32),
            true,
            None,
            "application/json",
            second.processed_ms.saturating_add(1),
        );
        save_report(&first).expect("save first report");
        save_report(&second).expect("save second report");
        save_report(&third).expect("save third report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            latest: Some(true),
            ids_only: Some(true),
            order: Some("DESC".to_string()),
            offset: Some(2),
            limit: Some(1),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let ids: Vec<String> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(ids, vec![third.id]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_reports_returns_deleted_ids_and_keeps_non_matching_reports() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let keep = sample_report("88".repeat(32), true, None, "application/json", now_ms());
        let delete = sample_report(
            "99".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            keep.processed_ms.saturating_add(1),
        );
        save_report(&keep).expect("save kept report");
        save_report(&delete).expect("save deleted report");

        let response = super::handle_delete_reports(NoritoQuery(ProverListQuery {
            id: Some(delete.id.clone()),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["deleted"].as_u64(), Some(1));
        assert_eq!(
            parsed["ids"].as_array(),
            Some(&vec![norito::json::Value::from(delete.id.clone())])
        );
        assert!(
            load_report(&delete.id).is_none(),
            "matched report should be deleted"
        );
        assert!(
            load_report(&keep.id).is_some(),
            "non-matching report should remain"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_reports_id_filter_accepts_uppercase_hex() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let report = sample_report(
            "8a".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            now_ms(),
        );
        save_report(&report).expect("save report");

        let response = super::handle_delete_reports(NoritoQuery(ProverListQuery {
            id: Some(report.id.to_ascii_uppercase()),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["deleted"].as_u64(), Some(1));
        assert_eq!(
            parsed["ids"].as_array(),
            Some(&vec![norito::json::Value::from(report.id.clone())])
        );
        assert!(
            load_report(&report.id).is_none(),
            "report should be deleted"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_reports_errors_only_alias_deletes_only_failed_reports() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let ok = sample_report("9a".repeat(32), true, None, "application/json", now_ms());
        let failed = sample_report(
            "bc".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            ok.processed_ms.saturating_add(1),
        );
        save_report(&ok).expect("save ok report");
        save_report(&failed).expect("save failed report");

        let response = super::handle_delete_reports(NoritoQuery(ProverListQuery {
            errors_only: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["deleted"].as_u64(), Some(1));
        assert_eq!(
            parsed["ids"].as_array(),
            Some(&vec![norito::json::Value::from(failed.id.clone())])
        );
        assert!(load_report(&ok.id).is_some(), "ok report should remain");
        assert!(
            load_report(&failed.id).is_none(),
            "failed report should be deleted"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_reports_failed_only_alias_deletes_only_failed_reports() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let ok = sample_report("bf".repeat(32), true, None, "application/json", now_ms());
        let failed = sample_report(
            "c0".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            ok.processed_ms.saturating_add(1),
        );
        save_report(&ok).expect("save ok report");
        save_report(&failed).expect("save failed report");

        let response = super::handle_delete_reports(NoritoQuery(ProverListQuery {
            failed_only: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["deleted"].as_u64(), Some(1));
        assert_eq!(
            parsed["ids"].as_array(),
            Some(&vec![norito::json::Value::from(failed.id.clone())])
        );
        assert!(load_report(&ok.id).is_some(), "ok report should remain");
        assert!(
            load_report(&failed.id).is_none(),
            "failed report should be deleted"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_reports_filters_by_content_type_tag_and_time_bounds() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let mut target = sample_report(
            "c1".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1+json",
            now_ms(),
        );
        target.zk1_tags = Some(vec!["PROF".to_string()]);
        let mut wrong_tag = sample_report(
            "c2".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1+json",
            target.processed_ms.saturating_add(1),
        );
        wrong_tag.zk1_tags = Some(vec!["IPAK".to_string()]);
        let mut wrong_time = sample_report(
            "c3".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1+json",
            target.processed_ms.saturating_add(10),
        );
        wrong_time.zk1_tags = Some(vec!["PROF".to_string()]);
        save_report(&target).expect("save target report");
        save_report(&wrong_tag).expect("save wrong-tag report");
        save_report(&wrong_time).expect("save wrong-time report");

        let response = super::handle_delete_reports(NoritoQuery(ProverListQuery {
            content_type: Some("x-zk1".to_string()),
            has_tag: Some("PROF".to_string()),
            since_ms: Some(target.processed_ms),
            before_ms: Some(target.processed_ms),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["deleted"].as_u64(), Some(1));
        assert_eq!(
            parsed["ids"].as_array(),
            Some(&vec![norito::json::Value::from(target.id.clone())])
        );
        assert!(
            load_report(&target.id).is_none(),
            "target should be deleted"
        );
        assert!(
            load_report(&wrong_tag.id).is_some(),
            "wrong-tag report should remain"
        );
        assert!(
            load_report(&wrong_time.id).is_some(),
            "wrong-time report should remain"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_reports_ok_and_errors_filters_together_delete_all_reports() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let ok = sample_report("bd".repeat(32), true, None, "application/json", now_ms());
        let failed = sample_report(
            "be".repeat(32),
            false,
            Some("verification failed"),
            "application/x-zk1",
            ok.processed_ms.saturating_add(1),
        );
        save_report(&ok).expect("save ok report");
        save_report(&failed).expect("save failed report");

        let response = super::handle_delete_reports(NoritoQuery(ProverListQuery {
            ok_only: Some(true),
            errors_only: Some(true),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["deleted"].as_u64(), Some(2));
        let ids = parsed["ids"].as_array().expect("ids array");
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&norito::json::Value::from(ok.id.clone())));
        assert!(ids.contains(&norito::json::Value::from(failed.id.clone())));
        assert!(load_report(&ok.id).is_none(), "ok report should be deleted");
        assert!(
            load_report(&failed.id).is_none(),
            "failed report should be deleted"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_reports_with_no_matches_returns_zero_and_empty_ids() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let report = sample_report("ce".repeat(32), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");

        let response = super::handle_delete_reports(NoritoQuery(ProverListQuery {
            id: Some("cf".repeat(32)),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let parsed: norito::json::Value =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(parsed["deleted"].as_u64(), Some(0));
        assert_eq!(parsed["ids"].as_array(), Some(&vec![]));
        assert!(
            load_report(&report.id).is_some(),
            "unmatched report should remain"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_offset_past_end_returns_empty_array() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let report = sample_report("aa".repeat(32), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            offset: Some(5),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let reports: Vec<norito::json::Value> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert!(reports.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_offset_and_limit_select_expected_window() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let first = sample_report("ab".repeat(32), true, None, "application/json", now_ms());
        let second = sample_report(
            "ac".repeat(32),
            true,
            None,
            "application/json",
            first.processed_ms.saturating_add(1),
        );
        let third = sample_report(
            "ad".repeat(32),
            true,
            None,
            "application/json",
            second.processed_ms.saturating_add(1),
        );
        let fourth = sample_report(
            "ae".repeat(32),
            true,
            None,
            "application/json",
            third.processed_ms.saturating_add(1),
        );
        save_report(&first).expect("save first report");
        save_report(&second).expect("save second report");
        save_report(&third).expect("save third report");
        save_report(&fourth).expect("save fourth report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            ids_only: Some(true),
            offset: Some(1),
            limit: Some(2),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let ids: Vec<String> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(ids, vec![second.id, third.id]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_skips_unloadable_report_bodies_in_full_projection() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();
        ensure_dirs();

        let id = "ae".repeat(32);
        fs::write(report_path_from_sanitized(&id), "{not json").expect("write report");
        persist_report_summaries_locked(&[ProverReportSummary {
            id: id.clone(),
            ok: true,
            error: None,
            content_type: "application/json".to_string(),
            processed_ms: now_ms(),
            zk1_tags: None,
        }])
        .expect("persist report index");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery::default()))
            .await
            .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let reports: Vec<norito::json::Value> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert!(reports.is_empty(), "unloadable reports should be skipped");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_latest_with_no_matches_returns_empty_ids_array() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let report = sample_report("de".repeat(32), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            latest: Some(true),
            ids_only: Some(true),
            has_tag: Some("PROF".to_string()),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let ids: Vec<String> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert!(ids.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_reports_limit_is_capped_to_one_thousand() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();
        ensure_dirs();

        let mut reports = Vec::with_capacity(1001);
        let base_ms = now_ms();
        for idx in 0..1001u64 {
            let id = format!("{:064x}", idx + 1_000);
            let report = sample_report(
                id,
                true,
                None,
                "application/json",
                base_ms.saturating_add(idx),
            );
            fs::write(
                report_path_from_sanitized(&report.id),
                norito::json::to_json_pretty(&report).expect("report json"),
            )
            .expect("write report file");
            reports.push(report);
        }
        let summaries: Vec<ProverReportSummary> =
            reports.iter().map(report_summary_from_report).collect();
        persist_report_summaries_locked(&summaries).expect("persist report index");

        let response = super::handle_list_reports(NoritoQuery(ProverListQuery {
            ids_only: Some(true),
            limit: Some(5_000),
            ..Default::default()
        }))
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let ids: Vec<String> =
            norito::json::from_json(std::str::from_utf8(&bytes).expect("utf8")).expect("json");
        assert_eq!(ids.len(), 1000);
        assert_eq!(ids.first(), Some(&reports[0].id));
        assert_eq!(ids.last(), Some(&reports[999].id));
        assert!(!ids.contains(&reports[1000].id));
    }

    include!("zk_prover/scanner_tests.rs");
}
