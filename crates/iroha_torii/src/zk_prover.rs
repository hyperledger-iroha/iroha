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
    zk::{hash_proof, hash_vk, verify_backend, verify_backend_with_timing_checked},
};
use iroha_data_model::proof::{
    ProofAttachment, ProofAttachmentList, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord,
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
    allowlist.is_empty() || allowlist.iter().any(|allowed| backend.starts_with(allowed))
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

fn find_vk_record_by_commitment(
    state: &CoreState,
    commitment: [u8; 32],
) -> Option<(VerifyingKeyId, VerifyingKeyRecord)> {
    let world = state.world_view();
    for (id, record) in world.verifying_keys().iter() {
        if record.commitment == commitment {
            return Some((id.clone(), record.clone()));
        }
    }
    None
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

    if attachment.proof.backend.as_str() != backend_str {
        errors.push("proof backend does not match attachment backend".into());
    }
    if attachment.proof.bytes.is_empty() {
        errors.push("proof bytes are empty".into());
    }
    if !backend_allowed(backend_str, &ctx.allowed_backends) {
        errors.push(format!("backend `{backend_str}` not allowed"));
    }
    if backend_str == "stark/fri-v1" || backend_str.starts_with("stark/fri-v1/") {
        if let Some(state) = ctx.state.as_ref() {
            if !state.zk_snapshot().stark.enabled {
                errors.push("stark verification is disabled in node configuration".into());
            }
        }
    }

    let mut vk_box: Option<VerifyingKeyBox> = None;
    let mut resolved_vk_ref = attachment.vk_ref.clone();
    let mut circuit_id: Option<String> = None;

    match (&attachment.vk_ref, &attachment.vk_inline) {
        (Some(_), Some(_)) => {
            errors.push("attachment must include exactly one of vk_ref or vk_inline".into());
        }
        (None, None) => {
            errors.push("attachment missing vk_ref/vk_inline".into());
        }
        (Some(vk_id), None) => {
            if vk_id.backend.as_str() != backend_str {
                errors.push(format!(
                    "vk_ref backend `{}` does not match proof backend `{backend_str}`",
                    vk_id.backend
                ));
            }
            let state = match ctx.state.as_ref() {
                Some(state) => state,
                None => {
                    errors.push("verifying key lookup requires core state".into());
                    return ProofReportEntry {
                        backend,
                        ok: false,
                        error: Some(errors.join("; ")),
                        proof_hash,
                        vk_ref: resolved_vk_ref,
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
                        vk_ref: resolved_vk_ref,
                        circuit_id,
                    };
                }
            };
            if !record.is_active() {
                errors.push("verifying key is not active".into());
            }
            if record.max_proof_bytes > 0
                && attachment.proof.bytes.len() > record.max_proof_bytes as usize
            {
                errors.push(format!(
                    "proof exceeds max_proof_bytes {}",
                    record.max_proof_bytes
                ));
            }
            if let Some(commitment) = attachment.vk_commitment {
                if commitment != record.commitment {
                    errors.push("vk_commitment does not match registry commitment".into());
                }
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
        }
        (None, Some(vk_inline)) => {
            if vk_inline.backend.as_str() != backend_str {
                errors.push("verifying key backend does not match proof backend".into());
            }
            let vk_hash = hash_vk(vk_inline);
            if let Some(commitment) = attachment.vk_commitment {
                if commitment != vk_hash {
                    errors.push("vk_commitment does not match inline verifying key".into());
                }
            }
            vk_box = Some(vk_inline.clone());
            if let Some(state) = ctx.state.as_ref() {
                if let Some((vk_id, record)) = find_vk_record_by_commitment(state, vk_hash) {
                    if !record.is_active() {
                        errors.push("verifying key is not active".into());
                    }
                    if record.max_proof_bytes > 0
                        && attachment.proof.bytes.len() > record.max_proof_bytes as usize
                    {
                        errors.push(format!(
                            "proof exceeds max_proof_bytes {}",
                            record.max_proof_bytes
                        ));
                    }
                    if record.vk_len > 0 && vk_inline.bytes.len() != record.vk_len as usize {
                        errors.push(format!(
                            "verifying key length {} does not match registry vk_len {}",
                            vk_inline.bytes.len(),
                            record.vk_len
                        ));
                    }
                    resolved_vk_ref = Some(vk_id);
                    circuit_id = Some(record.circuit_id);
                }
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
        vk_ref: resolved_vk_ref,
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
/// GET /v2/zk/prover/reports — list prover reports with optional filters.
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
/// GET /v2/zk/prover/reports/count — return number of matching prover reports.
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
/// DELETE /v2/zk/prover/reports — bulk delete reports matching filters.
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
/// GET /v2/zk/prover/reports/{id} — get a single report by id.
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
/// DELETE /v2/zk/prover/reports/{id} — delete a single report by id.
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
    use iroha_core::zk::test_utils::halo2_fixture_envelope;
    use iroha_data_model::proof::ProofAttachment;

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
            None,
            MaybeTelemetry::disabled(),
        );
        super::TEST_PROCESSING_DELAY_MS.store(0, AtomicOrdering::SeqCst);
        super::MAX_INFLIGHT_OBSERVED.store(0, AtomicOrdering::SeqCst);
    }

    fn init_test_cfg() {
        configure_test_cfg(iroha_config::parameters::defaults::torii::zk_prover_allowed_circuits());
    }

    fn fixture_attachment_bytes() -> Vec<u8> {
        let fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
        let proof = fixture.proof_box("halo2/ipa");
        let vk = fixture.vk_box("halo2/ipa").expect("fixture vk bytes");
        let attachment = ProofAttachment::new_inline("halo2/ipa".into(), proof, vk);
        norito::to_bytes(&attachment).expect("proof attachment bytes")
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

    #[tokio::test(flavor = "current_thread")]
    async fn get_report_rejects_invalid_id() {
        let response = axum::response::IntoResponse::into_response(
            super::handle_get_report(axum::extract::Path("../bad".to_string())).await,
        );
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_report_rejects_invalid_id() {
        let response = axum::response::IntoResponse::into_response(
            super::handle_delete_report(axum::extract::Path("../bad".to_string())).await,
        );
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
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

    #[tokio::test(flavor = "current_thread")]
    async fn count_reports_filters_using_report_summaries() {
        use http_body_util::BodyExt as _;

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
