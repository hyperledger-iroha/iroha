//! Background, non-consensus ZK prover worker tied to attachments.
//!
//! - Periodically scans `zk_attachments` for new items and produces a report
//!   under `zk_prover/reports/<id>.json` with
//!   `{ id, ok, error, content_type, size, created_ms, processed_ms, latency_ms }`.
//! - This module is strictly app-facing and non-forking. It must not affect consensus.
//! - Enabled and paced via `iroha_config` (torii.zk_prover_enabled, torii.zk_prover_scan_period_secs).
//!
//! The worker verifies `ProofAttachment` payloads (single or list, Norito or JSON)
//! using core backend verifiers and records per-proof metadata. It never mutates WSV.

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Read as _,
    io::{Error as IoError, ErrorKind as IoErrorKind},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{extract::Path as AxumPath, http::StatusCode, response::IntoResponse};
use iroha_core::{
    state::{State as CoreState, WorldReadOnly},
    zk::{
        hash_proof, hash_vk, is_developer_only_backend_label, is_trusted_setup_backend_label,
        verify_backend, verify_backend_with_timing_checked,
    },
};
use iroha_data_model::proof::{
    ProofAttachment, ProofAttachmentList, VerifyingKeyBox, VerifyingKeyId,
};
use mv::storage::StorageReadOnly;
use norito::json;
use tokio::{
    runtime::{Handle, RuntimeFlavor},
    sync::Semaphore,
    task::{self, JoinSet},
};

use crate::{NoritoQuery, routing::MaybeTelemetry};

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
    /// For Norito ZK1 envelopes, discovered TLV tags.
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
static MAX_INFLIGHT_OBSERVED: AtomicUsize = AtomicUsize::new(0);

/// Configure prover enable, scan period (seconds), and reports TTL (seconds) from Torii config.
#[allow(clippy::too_many_arguments)]
pub fn configure(
    enabled: bool,
    scan_period_secs: u64,
    reports_ttl_secs: u64,
    max_inflight: usize,
    max_scan_bytes: u64,
    max_scan_millis: u64,
    keys_dir: PathBuf,
    allowed_backends: Vec<String>,
    allowed_circuits: Vec<String>,
    state: Option<Arc<CoreState>>,
    telemetry: MaybeTelemetry,
) {
    let cfg = ProverCfg {
        enabled,
        scan_period_secs,
        reports_ttl_secs,
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
        let mut guard = lock.write().expect("prover cfg lock poisoned");
        *guard = cfg;
        return;
    }
    if PROVER_CFG.set(RwLock::new(cfg.clone())).is_err() {
        if let Some(lock) = PROVER_CFG.get() {
            let mut guard = lock.write().expect("prover cfg lock poisoned");
            *guard = cfg;
        }
    }
}

fn with_cfg<R>(f: impl FnOnce(&ProverCfg) -> R) -> Option<R> {
    PROVER_CFG.get().map(|lock| {
        let guard = lock.read().expect("prover cfg lock poisoned");
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
    let mut guard = slot.lock().expect("reports dir lock poisoned");
    if guard.as_ref() != Some(&dir) {
        let _ = fs::create_dir_all(&dir);
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
const REPORT_FILE_MAX_BYTES: u64 = 8 * 1024 * 1024;

static REPORT_INDEX_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone)]
struct AttachmentLocation {
    tenant_key: Option<String>,
    id: String,
}

fn sanitize_attachment_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.len() != ATTACHMENT_ID_HEX_LEN {
        return None;
    }
    if trimmed.bytes().any(|b| !b.is_ascii_hexdigit()) {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}

fn sanitize_report_id(raw: &str) -> Option<String> {
    sanitize_attachment_id(raw)
}

fn sanitize_tenant_key(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.len() != TENANT_KEY_HEX_LEN {
        return None;
    }
    if trimmed.bytes().any(|b| !b.is_ascii_hexdigit()) {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}

fn attachments_root_dir() -> PathBuf {
    super::zk_attachments::base_dir().join("zk_attachments")
}

fn attachment_meta_path(tenant_key: Option<&str>, id: &str) -> PathBuf {
    match tenant_key {
        Some(key) => attachments_root_dir()
            .join(key)
            .join(format!("{}.json", id)),
        None => attachments_root_dir().join(format!("{}.json", id)),
    }
}

fn attachment_bin_path(tenant_key: Option<&str>, id: &str) -> PathBuf {
    match tenant_key {
        Some(key) => attachments_root_dir().join(key).join(format!("{}.bin", id)),
        None => attachments_root_dir().join(format!("{}.bin", id)),
    }
}

fn report_path_from_sanitized(id: &str) -> PathBuf {
    reports_dir().join(format!("{}.json", id))
}

fn report_index_path() -> PathBuf {
    prover_dir().join("reports_index.json")
}

fn report_summary_lock() -> &'static Mutex<()> {
    REPORT_INDEX_LOCK.get_or_init(|| Mutex::new(()))
}

fn report_summary_from_report(report: &ProverReport) -> ProverReportSummary {
    ProverReportSummary {
        id: report.id.clone(),
        ok: report.ok,
        error: report.error.clone(),
        content_type: report.content_type.clone(),
        processed_ms: report.processed_ms,
        zk1_tags: report.zk1_tags.clone(),
    }
}

fn normalize_report_summaries(raw: Vec<ProverReportSummary>) -> Vec<ProverReportSummary> {
    let mut by_id: BTreeMap<String, ProverReportSummary> = BTreeMap::new();
    for mut summary in raw {
        let Some(clean) = sanitize_report_id(&summary.id) else {
            continue;
        };
        summary.id = clean.clone();
        by_id.insert(clean, summary);
    }
    by_id.into_values().collect()
}

fn persist_report_summaries_locked(summaries: &[ProverReportSummary]) -> std::io::Result<()> {
    ensure_dirs();
    let path = report_index_path();
    let tmp_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(tmp_dir)?;
    let body = norito::json::to_json_pretty(&summaries.to_vec()).unwrap_or_else(|_| "[]".into());
    use std::io::Write as _;
    tmp.write_all(body.as_bytes())?;
    tmp.flush()?;
    tmp.persist(&path).map(|_| ()).map_err(|e| e.error)
}

fn read_report_summaries_locked() -> Option<Vec<ProverReportSummary>> {
    let mut f = fs::File::open(report_index_path()).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let s = std::str::from_utf8(&buf).ok()?;
    let parsed = norito::json::from_json::<Vec<ProverReportSummary>>(s).ok()?;
    Some(normalize_report_summaries(parsed))
}

fn rebuild_report_summaries_locked() -> Vec<ProverReportSummary> {
    let mut summaries = Vec::new();
    for id in list_report_ids() {
        if let Some(report) = load_report(&id) {
            summaries.push(report_summary_from_report(&report));
        }
    }
    let _ = persist_report_summaries_locked(&summaries);
    summaries
}

fn load_report_summaries() -> Vec<ProverReportSummary> {
    let _guard = report_summary_lock()
        .lock()
        .expect("report summary lock poisoned");
    let mut summaries =
        read_report_summaries_locked().unwrap_or_else(rebuild_report_summaries_locked);
    let before = summaries.len();
    summaries.retain(|summary| report_path_from_sanitized(&summary.id).exists());
    if summaries.len() != before {
        let _ = persist_report_summaries_locked(&summaries);
    }
    summaries
}

fn upsert_report_summary(report: &ProverReport) {
    let _guard = report_summary_lock()
        .lock()
        .expect("report summary lock poisoned");
    let mut summaries =
        read_report_summaries_locked().unwrap_or_else(rebuild_report_summaries_locked);
    let summary = report_summary_from_report(report);
    if let Some(existing) = summaries.iter_mut().find(|entry| entry.id == summary.id) {
        *existing = summary;
    } else {
        summaries.push(summary);
    }
    let _ = persist_report_summaries_locked(&summaries);
}

fn remove_report_summary(id: &str) {
    let Some(clean) = sanitize_report_id(id) else {
        return;
    };
    let _guard = report_summary_lock()
        .lock()
        .expect("report summary lock poisoned");
    let mut summaries =
        read_report_summaries_locked().unwrap_or_else(rebuild_report_summaries_locked);
    let before = summaries.len();
    summaries.retain(|entry| entry.id != clean);
    if summaries.len() != before {
        let _ = persist_report_summaries_locked(&summaries);
    }
}

fn filter_report_summary(
    summary: &ProverReportSummary,
    q: &ProverListQuery,
    requested_id: Option<&str>,
    ok_req: bool,
    failed_req: bool,
) -> bool {
    if let Some(req_id) = requested_id {
        if summary.id != req_id {
            return false;
        }
    }
    if let Some(ct) = q.content_type.as_deref() {
        if !summary.content_type.contains(ct) {
            return false;
        }
    }
    if let Some(tag) = q.has_tag.as_deref() {
        let has_tag = summary
            .zk1_tags
            .as_ref()
            .map(|tags| tags.iter().any(|existing| existing == tag))
            .unwrap_or(false);
        if !has_tag {
            return false;
        }
    }
    if !q.since_ms.map_or(true, |th| summary.processed_ms >= th) {
        return false;
    }
    if !q.before_ms.map_or(true, |th| summary.processed_ms <= th) {
        return false;
    }
    match (ok_req, failed_req) {
        (true, false) => summary.ok,
        (false, true) => !summary.ok,
        _ => true,
    }
}

fn list_attachment_locations() -> Vec<AttachmentLocation> {
    let mut locs = Vec::new();
    if let Ok(rd) = fs::read_dir(attachments_root_dir()) {
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let file_name = e.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            if ft.is_dir() {
                let Some(tenant_key) = sanitize_tenant_key(name) else {
                    continue;
                };
                if let Ok(trd) = fs::read_dir(attachments_root_dir().join(&tenant_key)) {
                    for te in trd.flatten() {
                        let file_name = te.file_name();
                        let Some(tname) = file_name.to_str() else {
                            continue;
                        };
                        let Some(id) = tname.strip_suffix(".json") else {
                            continue;
                        };
                        let Some(clean) = sanitize_attachment_id(id) else {
                            continue;
                        };
                        locs.push(AttachmentLocation {
                            tenant_key: Some(tenant_key.clone()),
                            id: clean,
                        });
                    }
                }
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            // Legacy layout: `<root>/<id>.json`
            if let Some(id) = name.strip_suffix(".json") {
                if let Some(clean) = sanitize_attachment_id(id) {
                    locs.push(AttachmentLocation {
                        tenant_key: None,
                        id: clean,
                    });
                }
            }
        }
    }
    locs
}

fn find_attachment_location(id: &str) -> Option<AttachmentLocation> {
    let clean = sanitize_attachment_id(id)?;
    // Legacy layout first.
    if attachment_meta_path(None, &clean).exists() {
        return Some(AttachmentLocation {
            tenant_key: None,
            id: clean,
        });
    }
    // Tenant layout.
    if let Ok(rd) = fs::read_dir(attachments_root_dir()) {
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let file_name = e.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            let Some(tenant_key) = sanitize_tenant_key(name) else {
                continue;
            };
            if attachment_meta_path(Some(&tenant_key), &clean).exists() {
                return Some(AttachmentLocation {
                    tenant_key: Some(tenant_key),
                    id: clean,
                });
            }
        }
    }
    None
}

fn load_attachment_meta(loc: &AttachmentLocation) -> Option<super::zk_attachments::AttachmentMeta> {
    let mut f = fs::File::open(attachment_meta_path(loc.tenant_key.as_deref(), &loc.id)).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let s = std::str::from_utf8(&buf).ok()?;
    norito::json::from_json::<super::zk_attachments::AttachmentMeta>(s).ok()
}

fn load_attachment_body(loc: &AttachmentLocation) -> Option<Vec<u8>> {
    fs::read(attachment_bin_path(loc.tenant_key.as_deref(), &loc.id)).ok()
}

fn save_report(rep: &ProverReport) -> std::io::Result<()> {
    let Some(id) = sanitize_report_id(&rep.id) else {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "invalid prover report id",
        ));
    };
    ensure_dirs();
    let path = report_path_from_sanitized(&id);
    let tmp_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(tmp_dir)?;
    let s = norito::json::to_json_pretty(rep).unwrap_or_else(|_| "{}".into());
    use std::io::Write as _;
    tmp.write_all(s.as_bytes())?;
    tmp.flush()?;
    tmp.persist(&path).map(|_| ()).map_err(|e| e.error)?;
    upsert_report_summary(rep);
    Ok(())
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

fn list_report_ids() -> Vec<String> {
    let mut ids = Vec::new();
    if let Ok(rd) = fs::read_dir(reports_dir()) {
        for e in rd.flatten() {
            if let Some(name) = e.file_name().to_str() {
                if let Some(id) = name.strip_suffix(".json") {
                    if let Some(clean) = sanitize_report_id(id) {
                        ids.push(clean);
                    }
                }
            }
        }
    }
    ids
}

fn delete_report_files(id: &str) {
    if let Some(clean) = sanitize_report_id(id) {
        let _ = fs::remove_file(report_path_from_sanitized(&clean));
        remove_report_summary(&clean);
    }
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
    let _guard = report_summary_lock()
        .lock()
        .expect("report summary lock poisoned");
    let mut summaries =
        read_report_summaries_locked().unwrap_or_else(rebuild_report_summaries_locked);
    let mut retained = Vec::with_capacity(summaries.len());
    for summary in summaries.drain(..) {
        let age_ms = now.saturating_sub(summary.processed_ms);
        if age_ms > ttl_ms {
            let _ = fs::remove_file(report_path_from_sanitized(&summary.id));
            deleted += 1;
        } else {
            retained.push(summary);
        }
    }
    let _ = persist_report_summaries_locked(&retained);
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
        && (allowlist.is_empty() || allowlist.iter().any(|allowed| backend.starts_with(allowed)))
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
    let list_err = match norito::decode_from_bytes::<ProofAttachmentList>(body) {
        Ok(list) => return Ok(list.0),
        Err(err) => err.to_string(),
    };
    let single_err = match norito::decode_from_bytes::<ProofAttachment>(body) {
        Ok(single) => return Ok(vec![single]),
        Err(err) => err.to_string(),
    };
    Err(format!(
        "norito decode failed (list: {list_err}, single: {single_err})"
    ))
}

fn decode_json_attachments(body: &[u8]) -> Result<Vec<ProofAttachment>, String> {
    let list_err = match norito::json::from_slice::<ProofAttachmentList>(body) {
        Ok(list) => return Ok(list.0),
        Err(err) => err.to_string(),
    };
    let single_err = match norito::json::from_slice::<ProofAttachment>(body) {
        Ok(single) => return Ok(vec![single]),
        Err(err) => err.to_string(),
    };
    let vec_err = match norito::json::from_slice::<Vec<ProofAttachment>>(body) {
        Ok(list) => return Ok(list),
        Err(err) => err.to_string(),
    };
    Err(format!(
        "json decode failed (list: {list_err}, single: {single_err}, vec: {vec_err})"
    ))
}

fn decode_proof_attachments(
    content_type: &str,
    body: &[u8],
) -> Result<Vec<ProofAttachment>, String> {
    const ZK1_MIME_TYPE: &str = "application/x-zk1";

    if content_type.contains(super::utils::NORITO_MIME_TYPE) {
        if body.len() >= 4 && &body[..4] == b"ZK1\0" {
            return match zk1_minimal_validate(body) {
                Ok(()) => Err("unsupported ZK1 envelope (expected ProofAttachment payload)".into()),
                Err(err) => Err(err),
            };
        }
        return decode_norito_attachments(body)
            .map_err(|err| format!("norito decode error: {err}"));
    }
    if content_type.contains(ZK1_MIME_TYPE) {
        return match zk1_minimal_validate(body) {
            Ok(()) => Err("unsupported ZK1 envelope (expected ProofAttachment payload)".into()),
            Err(err) => Err(err),
        };
    }
    if content_type.contains("application/json") || content_type.contains("text/json") {
        return decode_json_attachments(body).map_err(|err| format!("json decode error: {err}"));
    }
    let json_attempt = decode_json_attachments(body);
    if let Ok(decoded) = json_attempt {
        return Ok(decoded);
    }
    let norito_attempt = decode_norito_attachments(body);
    if let Ok(decoded) = norito_attempt {
        return Ok(decoded);
    }
    let json_err = json_attempt
        .err()
        .unwrap_or_else(|| "unknown json error".into());
    let norito_err = norito_attempt
        .err()
        .unwrap_or_else(|| "unknown norito error".into());
    Err(format!(
        "unsupported payload (json: {json_err}; norito: {norito_err})"
    ))
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

// Minimal ZK1 structural validation: accept bare magic or well-formed TLVs.
// Recognized tags are advisory; unknown tags are allowed as long as TLVs are well-formed.
fn zk1_minimal_validate(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 4 || &bytes[..4] != b"ZK1\0" {
        return Err("missing ZK1 magic".into());
    }
    if bytes.len() == 4 {
        return Ok(()); // bare envelope is allowed
    }
    let mut pos = 4usize;
    const MAX_TLV_PAYLOAD: usize = 8 * 1024 * 1024; // 8 MiB safety bound
    while pos < bytes.len() {
        if pos + 8 > bytes.len() {
            return Err("truncated TLV header".into());
        }
        let tag = &bytes[pos..pos + 4];
        let len_le = &bytes[pos + 4..pos + 8];
        let len = u32::from_le_bytes([len_le[0], len_le[1], len_le[2], len_le[3]]) as usize;
        pos += 8;
        if len > MAX_TLV_PAYLOAD {
            return Err("TLV payload too large".into());
        }
        if pos + len > bytes.len() {
            return Err("truncated TLV payload".into());
        }
        // Optionally note recognized tags (no-op in stub)
        let _recognized = matches!(tag, b"PROF" | b"IPAK" | b"H2VK" | b"I10P");
        pos += len;
    }
    Ok(())
}

fn zk1_extract_tags(bytes: &[u8]) -> Vec<String> {
    let mut tags = Vec::new();
    if bytes.len() < 4 || &bytes[..4] != b"ZK1\0" {
        return tags;
    }
    let mut pos = 4usize;
    while pos + 8 <= bytes.len() {
        let tag_bytes = &bytes[pos..pos + 4];
        let tag = core::str::from_utf8(tag_bytes)
            .ok()
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("{:02X?}", tag_bytes));
        tags.push(tag);
        let len = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        pos += 8;
        if pos + len > bytes.len() {
            break;
        }
        pos += len;
    }
    tags
}

/// Process a single attachment id, emitting a report if not present yet.
pub fn process_attachment_once(id: &str) -> Option<ProverReport> {
    let clean = sanitize_attachment_id(id)?;
    let loc = find_attachment_location(&clean)?;
    process_attachment_once_at(&loc)
}

fn process_attachment_once_at(loc: &AttachmentLocation) -> Option<ProverReport> {
    // Skip if report already exists
    if report_path_from_sanitized(&loc.id).exists() {
        return load_report(&loc.id);
    }
    let meta = load_attachment_meta(loc)?;
    let body = load_attachment_body(loc)?;
    let zk1_tags = if body.len() >= 4 && &body[..4] == b"ZK1\0" {
        zk1_minimal_validate(&body)
            .ok()
            .map(|_| zk1_extract_tags(&body))
    } else {
        None
    };
    let ctx = ProverContext {
        keys_dir: cfg_keys_dir(),
        allowed_backends: cfg_allowed_backends(),
        allowed_circuits: cfg_allowed_circuits(),
        state: cfg_state(),
    };
    let mut proofs: Vec<ProofReportEntry> = Vec::new();
    let (ok, err, backend, vk_ref, proof_hash, circuit_id) =
        match decode_proof_attachments(&meta.content_type, &body) {
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
        id: meta.id.clone(),
        ok,
        error: err,
        content_type: meta.content_type,
        size: meta.size,
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

/// Scan all known attachments once, generating missing reports.
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
    let mut pending: Vec<AttachmentLocation> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
    for loc in list_attachment_locations() {
        if report_path_from_sanitized(&loc.id).exists() {
            continue;
        }
        if seen_ids.insert(loc.id.clone()) {
            pending.push(loc);
        }
    }

    let mut remaining = pending.len() as u64;
    telemetry.with_metrics(|tel| tel.set_torii_zk_prover_pending(remaining));

    let max_bytes = cfg_max_scan_bytes();
    let max_millis = cfg_max_scan_millis();
    let max_inflight = cfg_max_inflight();

    let semaphore = Arc::new(Semaphore::new(max_inflight));
    let inflight = Arc::new(AtomicU64::new(0));
    let start = std::time::Instant::now();
    let mut budget_reason: Option<&'static str> = None;
    let mut bytes_processed = 0u64;
    let mut processed_reports = 0usize;
    let mut join_set = JoinSet::new();

    for loc in pending {
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
            if start.elapsed().as_millis() as u64 >= max_millis {
                budget_reason = Some("time");
                break;
            }
        }
        if budget_reason.is_some() {
            break;
        }
        if start.elapsed().as_millis() as u64 >= max_millis {
            budget_reason = Some("time");
            break;
        }

        let Some(meta) = load_attachment_meta(&loc) else {
            remaining = remaining.saturating_sub(1);
            telemetry.with_metrics(|tel| tel.set_torii_zk_prover_pending(remaining));
            continue;
        };

        if bytes_processed.saturating_add(meta.size) > max_bytes {
            budget_reason = Some("bytes");
            break;
        }

        bytes_processed = bytes_processed.saturating_add(meta.size);
        remaining = remaining.saturating_sub(1);
        telemetry.with_metrics(|tel| tel.set_torii_zk_prover_pending(remaining));

        let permit = match semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => break,
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
            let result = task::spawn_blocking(move || process_attachment_once_at(&loc_owned))
                .await
                .map_err(|err| err.to_string())?;
            drop(permit);
            let after = inflight.fetch_sub(1, Ordering::SeqCst) - 1;
            telemetry_clone.with_metrics(|tel| tel.set_torii_zk_prover_inflight(after));
            Ok::<_, String>(result.is_some())
        });
    }

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

// ---------------- App-facing endpoints (feature-gated) ----------------

#[cfg(feature = "app_api")]
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
    /// Require a ZK1 tag to be present (e.g., "PROF").
    pub has_tag: Option<String>,
    /// Maximum number of results to return.
    pub limit: Option<u32>,
    /// Return only reports with processed_ms >= since_ms.
    pub since_ms: Option<u64>,
    /// Return only reports with processed_ms <= before_ms.
    pub before_ms: Option<u64>,
    /// When true, return only report ids (array of strings) instead of full objects.
    pub ids_only: Option<bool>,
    /// Result ordering: "asc" (default) or "desc" by processed_ms.
    pub order: Option<String>,
    /// Offset to apply after ordering and filtering (server-side paging).
    pub offset: Option<u32>,
    /// Convenience: alias for `failed_only=true` (errors are reports with ok=false).
    pub errors_only: Option<bool>,
    /// Projection: when true, return only `{ id, error }` objects for reports with `ok=false`.
    pub messages_only: Option<bool>,
    /// Convenience: when true, return only the latest report (by processed_ms) after filters.
    pub latest: Option<bool>,
}

#[cfg(feature = "app_api")]
/// GET /v1/zk/prover/reports — list prover reports with optional filters.
pub async fn handle_list_reports(
    NoritoQuery(q): NoritoQuery<ProverListQuery>,
) -> impl IntoResponse {
    let ok_req = q.ok_only.unwrap_or(false);
    let failed_req = q.failed_only.unwrap_or(false)
        || q.errors_only.unwrap_or(false)
        || q.messages_only.unwrap_or(false);
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

    let mut filtered: Vec<ProverReportSummary> = load_report_summaries()
        .into_iter()
        .filter(|summary| {
            filter_report_summary(summary, &q, requested_id.as_deref(), ok_req, failed_req)
        })
        .collect();
    filtered.sort_by_key(|summary| summary.processed_ms);
    // latest=true overrides order/offset/limit: pick the last (max processed_ms)
    if q.latest.unwrap_or(false) {
        if let Some(last) = filtered.pop() {
            filtered = vec![last];
        } else {
            filtered.clear();
        }
    } else {
        // Apply ordering
        if matches!(q.order.as_deref(), Some("desc" | "DESC" | "Desc")) {
            filtered.reverse();
        }
        // Apply offset then limit
        if let Some(off) = q.offset {
            let off = off as usize;
            if off < filtered.len() {
                filtered = filtered.split_off(off);
            } else {
                filtered.clear();
            }
        }
        if let Some(lim) = q.limit {
            let cap = lim.min(1000) as usize; // safety cap
            if filtered.len() > cap {
                filtered.truncate(cap);
            }
        }
    }
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
        let reports: Vec<ProverReport> = filtered
            .into_iter()
            .filter_map(|summary| load_report(&summary.id))
            .collect();
        norito::json::to_json_pretty(&reports).unwrap_or_else(|_| "[]".into())
    };
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(s))
        .unwrap()
}

#[cfg(feature = "app_api")]
/// GET /v1/zk/prover/reports/count — return number of matching prover reports.
pub async fn handle_count_reports(
    NoritoQuery(q): NoritoQuery<ProverListQuery>,
) -> impl IntoResponse {
    let ok_req = q.ok_only.unwrap_or(false);
    let failed_req = q.failed_only.unwrap_or(false) || q.errors_only.unwrap_or(false);
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

    let count = load_report_summaries()
        .into_iter()
        .filter(|summary| {
            filter_report_summary(summary, &q, requested_id.as_deref(), ok_req, failed_req)
        })
        .count() as u64;
    let body = norito::json::to_json_pretty(&crate::json_object(vec![("count", count)]))
        .unwrap_or_else(|_| "{}".into());
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap()
}

#[cfg(feature = "app_api")]
/// DELETE /v1/zk/prover/reports — bulk delete reports matching filters.
pub async fn handle_delete_reports(
    NoritoQuery(q): NoritoQuery<ProverListQuery>,
) -> impl IntoResponse {
    let ok_req = q.ok_only.unwrap_or(false);
    let failed_req = q.failed_only.unwrap_or(false) || q.errors_only.unwrap_or(false);
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
    let matches: Vec<String> = load_report_summaries()
        .into_iter()
        .filter(|summary| {
            filter_report_summary(summary, &q, requested_id.as_deref(), ok_req, failed_req)
        })
        .map(|summary| summary.id)
        .collect();

    let mut deleted_ids = Vec::new();
    for id in matches {
        delete_report_files(&id);
        deleted_ids.push(id);
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

#[cfg(feature = "app_api")]
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

#[cfg(feature = "app_api")]
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
    use iroha_core::zk::test_utils::halo2_fixture_envelope;
    use iroha_data_model::proof::{ProofAttachment, ProofBox};

    use super::*;
    use crate::test_utils::TestDataDirGuard;

    const TEST_SCAN_BUDGET_MARGIN_BYTES: u64 = 1024;

    fn configure_test_cfg(allowed_circuits: Vec<String>) {
        let fixture_len = fixture_attachment_bytes().len() as u64;
        let max_scan_bytes = fixture_len.saturating_add(TEST_SCAN_BUDGET_MARGIN_BYTES);
        let _ = super::configure(
            true,
            1,
            7 * 24 * 60 * 60,
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
        super::MAX_INFLIGHT_OBSERVED.store(0, AtomicOrdering::SeqCst);
    }

    fn init_test_cfg() {
        configure_test_cfg(iroha_config::parameters::defaults::torii::zk_prover_allowed_circuits());
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
            "zk-trace/mock-proof",
        ] {
            assert!(
                !backend_allowed(backend, &[]),
                "developer-only backend {backend} must not pass even an empty prover allowlist"
            );
        }
        assert!(backend_allowed("halo2/ipa", &[]));
    }

    fn fixture_attachment_bytes() -> Vec<u8> {
        let seed = halo2_fixture_envelope("halo2/ipa:tiny-add", [0u8; 32]);
        let vk = seed.vk_box("halo2/ipa").expect("fixture vk bytes");
        let vk_commitment = hash_vk(&vk);
        let fixture = halo2_fixture_envelope("halo2/ipa:tiny-add", vk_commitment);
        let proof = fixture.proof_box("halo2/ipa");
        let vk_id = VerifyingKeyId::new("halo2/ipa", "tiny-add");
        let mut attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof, vk_id);
        attachment.vk_commitment = Some(vk_commitment);
        norito::to_bytes(&attachment).expect("proof attachment bytes")
    }

    fn fixture_state() -> Arc<CoreState> {
        let seed = halo2_fixture_envelope("halo2/ipa:tiny-add", [0u8; 32]);
        let vk = seed.vk_box("halo2/ipa").expect("fixture vk bytes");
        let vk_id = VerifyingKeyId::new("halo2/ipa", "tiny-add");
        let vk_commitment = hash_vk(&vk);
        let mut record = iroha_data_model::proof::VerifyingKeyRecord::new_with_owner(
            1,
            "tiny-add",
            None,
            "test",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            [0; 32],
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
        world
            .verifying_keys_by_circuit_mut_for_testing()
            .insert(("tiny-add".into(), 1), vk_id);
        let mut state = iroha_core::state::State::new_for_testing(
            world,
            iroha_core::kura::Kura::blank_kura_for_testing(),
            iroha_core::query::store::LiveQueryStore::start_test(),
        );
        let mut zk = state.zk_snapshot();
        zk.halo2.enabled = true;
        state.set_zk(zk);
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
        assert!(error.contains("verifying key not found"));
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
        fs::write(report_index_path(), "{not json").expect("write malformed report index");

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

        let persisted = read_report_summaries_locked().expect("read report index");
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
    fn load_report_summaries_rebuilds_when_reports_index_is_malformed() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let report = sample_report("bb".repeat(32), true, None, "application/json", now_ms());
        save_report(&report).expect("save report");
        fs::write(report_index_path(), "{not json").expect("write malformed report index");

        let summaries = load_report_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, report.id);

        let persisted = read_report_summaries_locked().expect("rebuilt index");
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].id, report.id);
    }

    #[test]
    fn load_report_summaries_rebuilds_empty_index_when_no_reports_exist() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let summaries = load_report_summaries();
        assert!(summaries.is_empty());

        let persisted = read_report_summaries_locked().expect("persisted empty index");
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

        let persisted = read_report_summaries_locked().expect("read cleaned index");
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

        let persisted = read_report_summaries_locked().expect("read normalized index");
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].id, id);
    }

    #[test]
    fn save_report_rebuilds_index_when_existing_index_is_malformed() {
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
        fs::write(report_index_path(), "{not json").expect("write malformed report index");
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
    fn list_report_ids_ignores_invalid_entries_and_normalizes_case() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();
        ensure_dirs();

        let uppercase_id = "AB".repeat(32);
        let clean_id = uppercase_id.to_ascii_lowercase();
        fs::write(report_path_from_sanitized(&clean_id), b"{}").expect("write report file");
        fs::write(reports_dir().join("bad.json"), b"{}").expect("write invalid report id");
        fs::write(reports_dir().join("not-a-report.txt"), b"{}").expect("write non-report file");

        let ids = list_report_ids();
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
    fn gc_reports_once_rebuilds_when_reports_index_is_malformed() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();

        let fresh = sample_report("31".repeat(32), true, None, "application/json", now_ms());
        save_report(&fresh).expect("save fresh report");
        fs::write(report_index_path(), "{not json").expect("write malformed report index");

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

    #[test]
    fn scan_and_report_single_attachment() {
        configure_test_cfg(Vec::new());
        let _env = TestDataDirGuard::new();
        // Create an attachment manually
        let id = "deadbeef".repeat(8);
        let body = fixture_attachment_bytes();
        let tenant_key = anon_tenant_key();
        let meta = super::super::zk_attachments::AttachmentMeta {
            id: id.clone(),
            content_type: "application/x-norito".to_string(),
            size: body.len() as u64,
            created_ms: now_ms(),
            tenant: Some(tenant_key.clone()),
            provenance: None,
            zk1_tags: None,
        };
        ensure_tenant_dir(&tenant_key);
        fs::write(attachment_bin_path(Some(&tenant_key), &id), &body).unwrap();
        fs::write(
            attachment_meta_path(Some(&tenant_key), &id),
            norito::json::to_json_pretty(&meta).unwrap(),
        )
        .unwrap();
        // Run one scan
        let stats = super::block_on_scan();
        assert_eq!(stats.processed_reports, 1, "one report created");
        assert_eq!(stats.budget_exhausted, None);
        let rep = load_report(&id).expect("report exists");
        assert!(rep.ok);
        assert_eq!(rep.content_type, "application/x-norito");
        assert_eq!(rep.size, body.len() as u64);
        assert_eq!(rep.backend.as_deref(), Some("halo2/ipa"));
        assert!(rep.proof_hash.is_some());
        assert!(rep.proofs.is_empty());
        assert_eq!(
            rep.latency_ms,
            rep.processed_ms.saturating_sub(rep.created_ms)
        );
    }

    #[test]
    fn scan_respects_byte_budget() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();
        let budget = super::cfg_max_scan_bytes().max(2);
        let budget = usize::try_from(budget).unwrap_or(usize::MAX);
        let first_size = budget.saturating_sub(1).max(1);
        let sizes = [first_size, 2usize];
        let tenant_key = anon_tenant_key();
        ensure_tenant_dir(&tenant_key);
        // Create two attachments totalling more than the configured byte budget.
        for (idx, size) in sizes.into_iter().enumerate() {
            let id = format!("{:064x}", idx + 1);
            let meta = super::super::zk_attachments::AttachmentMeta {
                id: id.clone(),
                content_type: "application/json".to_string(),
                size: size as u64,
                created_ms: now_ms(),
                tenant: Some(tenant_key.clone()),
                provenance: None,
                zk1_tags: None,
            };
            fs::write(
                attachment_bin_path(Some(&tenant_key), &id),
                vec![b'A'; size],
            )
            .unwrap();
            fs::write(
                attachment_meta_path(Some(&tenant_key), &id),
                norito::json::to_json_pretty(&meta).unwrap(),
            )
            .unwrap();
        }

        let stats = super::block_on_scan();
        assert_eq!(
            stats.processed_reports, 1,
            "only first attachment fits budget"
        );
        assert_eq!(stats.budget_exhausted, Some("bytes"));
        assert_eq!(stats.remaining_pending, 1);
    }

    #[test]
    fn scan_bounds_concurrency() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();
        super::TEST_PROCESSING_DELAY_MS.store(50, AtomicOrdering::SeqCst);
        let tenant_key = anon_tenant_key();
        ensure_tenant_dir(&tenant_key);
        // Create four small attachments to trigger overlapping work.
        for idx in 0..4 {
            let id = format!("{:064x}", idx + 10);
            let meta = super::super::zk_attachments::AttachmentMeta {
                id: id.clone(),
                content_type: "application/json".to_string(),
                size: 16,
                created_ms: now_ms(),
                tenant: Some(tenant_key.clone()),
                provenance: None,
                zk1_tags: None,
            };
            fs::write(attachment_bin_path(Some(&tenant_key), &id), vec![b'B'; 16]).unwrap();
            fs::write(
                attachment_meta_path(Some(&tenant_key), &id),
                norito::json::to_json_pretty(&meta).unwrap(),
            )
            .unwrap();
        }

        let stats = super::block_on_scan();
        assert_eq!(stats.budget_exhausted, None);
        let observed = super::MAX_INFLIGHT_OBSERVED.load(AtomicOrdering::SeqCst);
        assert!(
            observed <= super::cfg_max_inflight(),
            "observed inflight {} exceeds cap",
            observed
        );
        super::TEST_PROCESSING_DELAY_MS.store(0, AtomicOrdering::SeqCst);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn scan_once_handles_current_thread_runtime() {
        init_test_cfg();
        let _env = TestDataDirGuard::new();
        assert_eq!(super::scan_once(), 0);
    }

    #[test]
    fn zk1_extracts_tags_prof_and_ipak() {
        let mut v = b"ZK1\0".to_vec();
        // PROF with 0 payload
        v.extend_from_slice(b"PROF");
        v.extend_from_slice(&0u32.to_le_bytes());
        // IPAK with 4-byte payload
        v.extend_from_slice(b"IPAK");
        v.extend_from_slice(&4u32.to_le_bytes());
        v.extend_from_slice(&5u32.to_le_bytes());
        let tags = zk1_extract_tags(&v);
        assert!(tags.starts_with(&["PROF".to_string(), "IPAK".to_string()]));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn background_worker_processes_pending_attachments() {
        configure_test_cfg(Vec::new());
        let _env = TestDataDirGuard::new();

        // Prepare attachment directory with one valid proof attachment and one malformed ZK1 payload.
        let tenant_key = anon_tenant_key();
        ensure_tenant_dir(&tenant_key);

        let ok_body = fixture_attachment_bytes();
        let ok_id = format!("{:064x}", 0x42u64);
        fs::write(attachment_bin_path(Some(&tenant_key), &ok_id), &ok_body).expect("write ok body");
        let ok_meta = super::super::zk_attachments::AttachmentMeta {
            id: ok_id.clone(),
            content_type: "application/x-norito".to_string(),
            size: ok_body.len() as u64,
            created_ms: super::now_ms(),
            tenant: Some(tenant_key.clone()),
            provenance: None,
            zk1_tags: None,
        };
        fs::write(
            attachment_meta_path(Some(&tenant_key), &ok_id),
            norito::json::to_json_pretty(&ok_meta).expect("ok meta json"),
        )
        .expect("write ok meta");

        let mut err_body = b"ZK1\0".to_vec();
        err_body.extend_from_slice(b"PROF");
        err_body.extend_from_slice(&10u32.to_le_bytes());
        let err_id = format!("{:064x}", 0x43u64);
        fs::write(attachment_bin_path(Some(&tenant_key), &err_id), &err_body)
            .expect("write err body");
        let err_meta = super::super::zk_attachments::AttachmentMeta {
            id: err_id.clone(),
            content_type: "application/x-norito".to_string(),
            size: err_body.len() as u64,
            created_ms: super::now_ms(),
            tenant: Some(tenant_key.clone()),
            provenance: None,
            zk1_tags: None,
        };
        fs::write(
            attachment_meta_path(Some(&tenant_key), &err_id),
            norito::json::to_json_pretty(&err_meta).expect("err meta json"),
        )
        .expect("write err meta");

        super::start_worker();

        use tokio::time::{Duration, Instant, sleep};
        let deadline = Instant::now() + Duration::from_secs(6);
        let mut ok_report_ready = false;
        let mut err_ready = false;
        while Instant::now() < deadline {
            if !ok_report_ready {
                ok_report_ready = super::load_report(&ok_id).is_some();
            }
            if !err_ready {
                err_ready = super::load_report(&err_id)
                    .map(|rep| !rep.ok)
                    .unwrap_or(false);
            }
            if ok_report_ready && err_ready {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert!(ok_report_ready, "Proof attachment should produce a report");
        assert!(
            err_ready,
            "Malformed Norito attachment should produce an error report"
        );

        assert_eq!(
            super::scan_once(),
            0,
            "worker should drain pending attachments"
        );
    }
}
