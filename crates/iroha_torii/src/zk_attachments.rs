//! Minimal ZK attachments store for the app-facing API.
//!
//! Feature-gated behind `app_api`:
//! - Stores attachments (proof envelopes or JSON DTOs) under `./storage/torii/zk_attachments/`.
//!   Base directory is configured via `torii.data_dir`; tests may use `data_dir::OverrideGuard`.
//! - Deterministic id: Blake2b-32 of the sanitized request bytes (lowercase hex).
//! - Multi-tenant: attachments are isolated per signed Iroha account. API tokens, when enabled,
//!   are an additional access-control requirement but do not define tenant identity.
//! - Endpoints:
//!   - POST `/v1/zk/attachments` – store attachment, returns metadata `{ id, size, content_type, created_ms }`.
//!   - GET  `/v1/zk/attachments` – list metadata for stored attachments.
//!   - GET  `/v1/zk/attachments/{id}` – fetch stored attachment bytes by id.
//!   - DELETE `/v1/zk/attachments/{id}` – delete stored attachment and its metadata.
//! - A background GC task periodically deletes entries older than a TTL;
//!   TTL and size caps are provided via `iroha_config` (Torii).
//! - The prover keeps a versioned, content-ID processing receipt separately
//!   from evictable reports. Per-tenant live-reference shards retain that
//!   receipt only while at least one matching attachment remains stored.
use crate::{
    NoritoQuery,
    routing::MaybeTelemetry,
    utils::NORITO_MIME_TYPE,
    zk1::{MAX_TLV_COUNT as ZK1_MAX_TLV_COUNT, parse_tags as parse_zk1_tags},
};
use axum::{extract::Path as AxumPath, http::StatusCode, response::IntoResponse};
use flate2::read::GzDecoder;
use iroha_config::parameters::actual::AttachmentSanitizerMode;
use iroha_data_model::{account::AccountId, proof::PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1};
use iroha_logger::prelude::*;
use norito::json;
use parking_lot::{Mutex as SyncMutex, RwLock};
use sha2::{Digest as _, Sha256};
#[cfg(test)]
use std::sync::Arc;
use std::{
    collections::BTreeSet,
    env,
    ffi::OsStr,
    fs,
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{OnceLock, mpsc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{sync::Mutex, task};
use zstd::stream::read::Decoder as ZstdDecoder;
const MAX_ATTACHMENT_BYTES_FALLBACK: usize = 4 * 1024 * 1024; // fallback 4 MiB
const ATTACHMENT_TTL_SECS_FALLBACK: u64 = 7 * 24 * 60 * 60; // fallback 7 days
const GC_INTERVAL_SECS: u64 = 60; // run every minute
const ATTACHMENT_ID_HEX_LEN: usize = 64;
const TENANT_KEY_HEX_LEN: usize = 64;
const ZK1_MIME_TYPE: &str = "application/x-zk1";
const OCTET_STREAM_MIME_TYPE: &str = "application/octet-stream";
const JSON_MIME_TYPE: &str = "application/json";
const ATTACHMENT_SANITIZER_ENV: &str = "IROHA_ATTACHMENT_SANITIZER";
const ATTACHMENT_SANITIZER_MAX_INPUT_ENV: &str = "IROHA_ATTACHMENT_SANITIZER_MAX_INPUT_BYTES";
const ATTACHMENT_SANITIZER_SANDBOXED_ENV: &str = "IROHA_ATTACHMENT_SANITIZER_OS_SANDBOXED";
const ATTACHMENT_SANITIZER_BINARY_STEM: &str = "attachment_sanitizer";
const SANITIZER_POLL_INTERVAL_MS: u64 = 5;
const SANITIZER_RESPONSE_OVERHEAD_BYTES: usize = 64 * 1024;
const ATTACHMENT_META_SCAN_MAX_FILES: u64 =
    iroha_config::parameters::defaults::torii::ATTACHMENTS_GLOBAL_MAX_COUNT_MAX;
const ATTACHMENT_TENANT_SCAN_MAX_ENTRIES: u64 = ATTACHMENT_META_SCAN_MAX_FILES * 2;
const ATTACHMENT_QUOTA_TRANSACTION_VERSION: u16 = 1;
const ATTACHMENT_QUOTA_TRANSACTION_MAX_BYTES: u64 = 2 * 1024 * 1024;
const ATTACHMENT_QUOTA_TRANSACTION_DIR: &str = "zk_attachment_quota";
const ATTACHMENT_QUOTA_TRANSACTION_FILE: &str = "pending_v1.json";
const ATTACHMENT_QUOTA_TRANSACTION_TEMP_PREFIX: &str = ".tmp";
pub(super) const ZK_PROVER_PROCESSING_STATE_VERSION: u16 = 1;
const ZK_PROVER_PROCESSING_REFERENCE_MARKER: &[u8] = b"iroha-torii-zk-prover-live-reference-v1\n";
const ZK_PROVER_PROCESSING_RECEIPT_MAX_BYTES: u64 = 16 * 1024;
const ZK_PROVER_PROCESSING_TEMP_PREFIX: &str = ".tmp";
static ZK_PROVER_PROCESSING_STATE_LOCK: OnceLock<SyncMutex<()>> = OnceLock::new();
/// Maximum encoded size of one persisted attachment metadata record.
pub(super) const ATTACHMENT_META_FILE_MAX_BYTES: u64 = 64 * 1024;
/// Tenant namespace for the attachments store.
///
/// This is a stable, opaque identifier (64-hex) derived from a signed Iroha account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentTenant(String);
impl AttachmentTenant {
    /// Derive a tenant key from a signed account id.
    pub fn from_account(account: &AccountId) -> Self {
        Self(hash_identity_hex("account", &account.to_string()))
    }
    /// Tenant used when neither token nor remote address is available.
    pub fn anonymous() -> Self {
        Self(hash_identity_hex("anon", "anon"))
    }
    /// Return the stable tenant key (lowercase 64-hex).
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
)]
/// Attachment digest metadata (hex-encoded).
pub struct AttachmentHashes {
    /// Blake2b-256 digest of the stored (sanitized) attachment bytes.
    pub blake2b_256: String,
    /// SHA-256 digest of the stored (sanitized) attachment bytes.
    pub sha256: String,
}
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
)]
/// Sanitizer outcome recorded for a stored attachment.
pub struct AttachmentSanitizerVerdict {
    /// Sanitizer verdict (e.g., "accepted").
    pub verdict: String,
    /// Expanded size in bytes after decompression (if any).
    pub expanded_bytes: u64,
    /// Archive depth encountered while expanding payloads.
    pub archive_depth: u32,
    /// Whether the sanitizer executed in an isolated subprocess.
    pub sandboxed: bool,
}
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
)]
/// Provenance metadata recorded alongside an attachment.
pub struct AttachmentProvenance {
    /// Declared MIME type from the request header (normalized).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub declared_type: Option<String>,
    /// Sniffed MIME type derived from magic bytes (normalized).
    pub sniffed_type: String,
    /// Attachment digests of stored bytes.
    pub hashes: AttachmentHashes,
    /// Sanitizer summary for the stored attachment.
    pub sanitizer: AttachmentSanitizerVerdict,
}
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
)]
/// Metadata for a stored attachment.
pub struct AttachmentMeta {
    /// Deterministic id (hex of Blake2b-32 over sanitized body bytes).
    pub id: String,
    /// Content type derived from sniffing (e.g., application/json).
    pub content_type: String,
    /// Size of the stored attachment bytes.
    pub size: u64,
    /// Unix time in milliseconds when the attachment was created.
    pub created_ms: u64,
    /// Hashed tenant identity used for quota enforcement.
    pub tenant: Option<String>,
    /// Provenance metadata for the stored attachment.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<AttachmentProvenance>,
    /// ZK1 TLV tags extracted at ingest time (when content is `application/x-zk1`).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub zk1_tags: Option<Vec<String>>,
}
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
struct AttachmentQuotaTransaction {
    version: u16,
    tenant: String,
    incoming_meta: AttachmentMeta,
    previous_meta: Option<AttachmentMeta>,
    victim_ids: Vec<String>,
}
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub(super) struct ProverProcessingReceipt {
    pub(super) version: u16,
    pub(super) id: String,
    pub(super) processed_ms: u64,
    pub(super) terminal: bool,
    pub(super) retry_not_before_ms: Option<u64>,
    pub(super) retry_count: u32,
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub(super) completed_proof_indices: Vec<u16>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub(super) processing_context_hash: Option<String>,
}
impl ProverProcessingReceipt {
    pub(super) fn disposition_is_valid(&self) -> bool {
        let completed_indices_valid = self.completed_proof_indices.len()
            <= PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1
            && self
                .completed_proof_indices
                .iter()
                .all(|index| usize::from(*index) < PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1)
            && self
                .completed_proof_indices
                .windows(2)
                .all(|pair| pair[0] < pair[1]);
        let context_hash_valid = self.processing_context_hash.as_deref().is_some_and(|hash| {
            hash.len() == ATTACHMENT_ID_HEX_LEN
                && hash
                    .as_bytes()
                    .iter()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        });
        let cache_valid = if self.completed_proof_indices.is_empty() {
            self.processing_context_hash.is_none()
        } else {
            context_hash_valid
        };
        if self.terminal {
            self.retry_not_before_ms.is_none()
                && self.retry_count == 0
                && self.completed_proof_indices.is_empty()
                && self.processing_context_hash.is_none()
        } else {
            self.retry_not_before_ms.is_some()
                && self.retry_count > 0
                && completed_indices_valid
                && cache_valid
        }
    }
    pub(super) fn reconcile_committed(durable: Option<Self>, committed: Self) -> Self {
        if committed.terminal {
            return committed;
        }
        let Some(durable) = durable else {
            return committed;
        };
        if durable.terminal {
            return durable;
        }
        let committed_is_newer = (committed.retry_count, committed.processed_ms)
            >= (durable.retry_count, durable.processed_ms);
        if durable.processing_context_hash != committed.processing_context_hash {
            return if committed_is_newer {
                committed
            } else {
                durable
            };
        }
        let (mut selected, other) = if committed_is_newer {
            (committed, durable)
        } else {
            (durable, committed)
        };
        selected
            .completed_proof_indices
            .extend(other.completed_proof_indices);
        selected.completed_proof_indices.sort_unstable();
        selected.completed_proof_indices.dedup();
        selected
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProverProcessingDecision {
    Missing,
    Suppress,
    Due { retry_count: u32 },
}
pub(crate) fn base_dir() -> PathBuf {
    crate::data_dir::base_dir()
}
fn attachments_root_dir() -> PathBuf {
    base_dir().join("zk_attachments")
}
fn attachments_dir(tenant: &AttachmentTenant) -> PathBuf {
    attachments_root_dir().join(tenant.as_str())
}
fn ensure_root_dir() {
    if cfg!(test) {
        let _ = fs::create_dir_all(attachments_root_dir());
        return;
    }
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = fs::create_dir_all(attachments_root_dir());
    });
}
fn ensure_dirs(tenant: &AttachmentTenant) {
    ensure_root_dir();
    let _ = fs::create_dir_all(attachments_dir(tenant));
}
fn meta_path(tenant: &AttachmentTenant, id: &str) -> PathBuf {
    attachments_dir(tenant).join(format!("{}.json", id))
}
fn bin_path(tenant: &AttachmentTenant, id: &str) -> PathBuf {
    attachments_dir(tenant).join(format!("{}.bin", id))
}
fn attachment_quota_transaction_dir() -> PathBuf {
    base_dir().join(ATTACHMENT_QUOTA_TRANSACTION_DIR)
}
fn attachment_quota_transaction_path() -> PathBuf {
    attachment_quota_transaction_dir().join(ATTACHMENT_QUOTA_TRANSACTION_FILE)
}
#[cfg(unix)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    fs::File::open(path)?.sync_all()
}
#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> std::io::Result<()> {
    // Rust does not expose a portable directory-sync primitive. File data is
    // still synchronized before rename on these targets.
    Ok(())
}
fn sync_parent_directory(path: &Path) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_attachment_file("persisted path has no containing directory"))?;
    sync_directory(parent)
}
fn persist_bytes_atomically(path: &Path, body: &[u8], prefix: &str) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_attachment_file("persisted path has no containing directory"))?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(prefix)
        .tempfile_in(parent)?;
    temporary.write_all(body)?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    sync_directory(parent)
}
fn ensure_attachment_dirs_durable(tenant: &AttachmentTenant) -> std::io::Result<()> {
    let root = attachments_root_dir();
    fs::create_dir_all(&root)?;
    sync_directory(&base_dir())?;
    fs::create_dir_all(attachments_dir(tenant))?;
    sync_directory(&root)
}
fn ensure_attachment_quota_transaction_dir_durable() -> std::io::Result<()> {
    fs::create_dir_all(attachment_quota_transaction_dir())?;
    sync_directory(&base_dir())
}
fn prover_processing_state_dir() -> PathBuf {
    base_dir()
        .join("zk_prover")
        .join(format!("processing_{ZK_PROVER_PROCESSING_STATE_VERSION}"))
}
fn prover_processing_receipt_path(id: &str) -> PathBuf {
    prover_processing_state_dir().join(id).join("receipt.json")
}
fn prover_processing_reference_dir(id: &str) -> PathBuf {
    prover_processing_state_dir().join(id).join("live")
}
fn prover_processing_reference_path(tenant_key: &str, id: &str) -> PathBuf {
    prover_processing_reference_dir(id).join(format!("{tenant_key}.ref"))
}
fn prover_processing_state_lock() -> &'static SyncMutex<()> {
    ZK_PROVER_PROCESSING_STATE_LOCK.get_or_init(|| SyncMutex::new(()))
}
fn persist_processing_marker(path: &Path) -> std::io::Result<()> {
    persist_bytes_atomically(
        path,
        ZK_PROVER_PROCESSING_REFERENCE_MARKER,
        ZK_PROVER_PROCESSING_TEMP_PREFIX,
    )
}
fn is_regular_processing_marker(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
}
/// Ensure the global processing receipt for `id` remains referenced by one live tenant copy.
pub(super) fn ensure_prover_processing_reference(
    tenant_key: &str,
    id: &str,
) -> std::io::Result<()> {
    let tenant_key = sanitize_tenant_key(tenant_key)
        .ok_or_else(|| invalid_attachment_file("invalid processing-reference tenant"))?;
    let id = sanitize_attachment_id(id)
        .ok_or_else(|| invalid_attachment_file("invalid processing-reference attachment id"))?;
    let _guard = prover_processing_state_lock().lock();
    let tenant = AttachmentTenant(tenant_key.clone());
    if fs::symlink_metadata(meta_path(&tenant, &id)).is_err()
        || fs::symlink_metadata(bin_path(&tenant, &id)).is_err()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "processing-reference attachment files are not both live",
        ));
    }
    let path = prover_processing_reference_path(&tenant_key, &id);
    if is_regular_processing_marker(&path) {
        return Ok(());
    }
    persist_processing_marker(&path)
}
fn load_prover_processing_receipt_locked(id: &str) -> Option<ProverProcessingReceipt> {
    let path = prover_processing_receipt_path(id);
    let bytes =
        read_bounded_attachment_regular_file(&path, ZK_PROVER_PROCESSING_RECEIPT_MAX_BYTES).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let receipt = json::from_json::<ProverProcessingReceipt>(text).ok()?;
    (receipt.version == ZK_PROVER_PROCESSING_STATE_VERSION
        && receipt.id == id
        && receipt.disposition_is_valid())
    .then_some(receipt)
}
/// Load a validated processing receipt for one content ID.
pub(super) fn load_prover_processing_receipt(id: &str) -> Option<ProverProcessingReceipt> {
    let id = sanitize_attachment_id(id)?;
    let _guard = prover_processing_state_lock().lock();
    load_prover_processing_receipt_locked(&id)
}
/// Resolve whether a durable content-ID receipt suppresses work at `now_ms`.
pub(super) fn prover_processing_decision(id: &str, now_ms: u64) -> ProverProcessingDecision {
    let Some(id) = sanitize_attachment_id(id) else {
        return ProverProcessingDecision::Missing;
    };
    let _guard = prover_processing_state_lock().lock();
    let Some(receipt) = load_prover_processing_receipt_locked(&id) else {
        return ProverProcessingDecision::Missing;
    };
    if receipt.terminal
        || receipt
            .retry_not_before_ms
            .is_some_and(|retry_at| now_ms < retry_at)
    {
        ProverProcessingDecision::Suppress
    } else {
        ProverProcessingDecision::Due {
            retry_count: receipt.retry_count,
        }
    }
}
fn processing_reference_dir_has_shards(id: &str) -> std::io::Result<bool> {
    let entries = match fs::read_dir(prover_processing_reference_dir(id)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_suffix(".ref"))
                .and_then(sanitize_tenant_key)
                .is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}
/// Persist a completed content-ID receipt only while at least one attachment copy is live.
pub(super) fn persist_prover_processing_receipt_if_referenced(
    receipt: &ProverProcessingReceipt,
) -> std::io::Result<bool> {
    let id = sanitize_attachment_id(&receipt.id)
        .ok_or_else(|| invalid_attachment_file("invalid processing-receipt attachment id"))?;
    if receipt.version != ZK_PROVER_PROCESSING_STATE_VERSION
        || receipt.id != id
        || !receipt.disposition_is_valid()
    {
        return Err(invalid_attachment_file(
            "invalid ZK prover processing receipt",
        ));
    }
    let _guard = prover_processing_state_lock().lock();
    persist_prover_processing_receipt_if_referenced_locked(receipt, &id)
}
fn persist_prover_processing_receipt_if_referenced_locked(
    receipt: &ProverProcessingReceipt,
    id: &str,
) -> std::io::Result<bool> {
    if !processing_reference_dir_has_shards(id)? {
        return Ok(false);
    }
    let body = json::to_json(receipt).map_err(|error| {
        invalid_attachment_file(format!("encode ZK prover processing receipt: {error}"))
    })?;
    if body.len() as u64 > ZK_PROVER_PROCESSING_RECEIPT_MAX_BYTES {
        return Err(invalid_attachment_file(
            "ZK prover processing receipt exceeds its persistence limit",
        ));
    }
    persist_bytes_atomically(
        &prover_processing_receipt_path(id),
        body.as_bytes(),
        ZK_PROVER_PROCESSING_TEMP_PREFIX,
    )?;
    Ok(true)
}
/// Atomically reconcile a committed report disposition with its durable receipt.
pub(super) fn reconcile_prover_processing_receipt_if_referenced(
    committed: &ProverProcessingReceipt,
) -> std::io::Result<ProverProcessingReceipt> {
    let id = sanitize_attachment_id(&committed.id)
        .ok_or_else(|| invalid_attachment_file("invalid processing-receipt attachment id"))?;
    if committed.version != ZK_PROVER_PROCESSING_STATE_VERSION
        || committed.id != id
        || !committed.disposition_is_valid()
    {
        return Err(invalid_attachment_file(
            "invalid ZK prover processing receipt",
        ));
    }
    let _guard = prover_processing_state_lock().lock();
    let durable = load_prover_processing_receipt_locked(&id);
    let selected = ProverProcessingReceipt::reconcile_committed(durable.clone(), committed.clone());
    if durable.as_ref() != Some(&selected) {
        let _ = persist_prover_processing_receipt_if_referenced_locked(&selected, &id)?;
    }
    Ok(selected)
}
fn remove_file_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => sync_parent_directory(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
fn remove_dir_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_dir(path) {
        Ok(()) => sync_parent_directory(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
fn remove_processing_temp_files(dir: &Path) -> std::io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(ZK_PROVER_PROCESSING_TEMP_PREFIX))
        {
            remove_file_if_present(&entry.path())?;
        }
    }
    Ok(())
}
fn remove_prover_processing_reference_locked(tenant_key: &str, id: &str) -> std::io::Result<()> {
    remove_file_if_present(&prover_processing_reference_path(tenant_key, id))?;
    if processing_reference_dir_has_shards(id)? {
        return Ok(());
    }
    let state_entry_dir = prover_processing_state_dir().join(id);
    let reference_dir = prover_processing_reference_dir(id);
    // Atomic persistence can leave its named temporary file behind if the
    // process stops before rename. Those files are not live-reference shards
    // and must not permanently pin the attachment or its processing state.
    remove_processing_temp_files(&reference_dir)?;
    remove_processing_temp_files(&state_entry_dir)?;
    remove_dir_if_present(&reference_dir)?;
    remove_file_if_present(&prover_processing_receipt_path(id))?;
    remove_dir_if_present(&state_entry_dir)
}
fn invalid_attachment_file(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
}
/// Open one attachment-store entry without following its final path component.
///
/// Unix pins the containing directory and uses one non-blocking `openat`, so a
/// concurrent substitution with a symlink, FIFO, device, or directory cannot
/// redirect or indefinitely block a reader. Windows opens the reparse point
/// itself and requires a stable single-link file identity before accepting the
/// handle. Other targets fail closed until they expose equivalent primitives.
///
/// # Errors
///
/// Returns an error when the path cannot be anchored or the opened entry is not
/// a direct, single-link regular file with a stable identity.
pub(super) fn open_attachment_regular_file(
    path: &Path,
) -> std::io::Result<(fs::File, fs::Metadata)> {
    open_attachment_regular_file_platform(path)
}
#[cfg(unix)]
fn open_attachment_regular_file_platform(path: &Path) -> std::io::Result<(fs::File, fs::Metadata)> {
    use std::os::unix::fs::MetadataExt as _;
    let parent_path = path
        .parent()
        .ok_or_else(|| invalid_attachment_file("attachment path has no containing directory"))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| invalid_attachment_file("attachment path has no file name"))?;
    let parent = fs::File::from(
        rustix::fs::open(
            parent_path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(std::io::Error::from)?,
    );
    let file = fs::File::from(
        rustix::fs::openat(
            &parent,
            file_name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::NOCTTY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(std::io::Error::from)?,
    );
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(invalid_attachment_file(
            "attachment entry is not a direct single-link regular file",
        ));
    }
    Ok((file, metadata))
}
#[cfg(windows)]
fn open_attachment_regular_file_platform(path: &Path) -> std::io::Result<(fs::File, fs::Metadata)> {
    use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let before = fs::symlink_metadata(path)?;
    let before_identity = (before.volume_serial_number(), before.file_index());
    if !before.is_file()
        || before.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || before.number_of_links() != Some(1)
        || before_identity.0.is_none()
        || before_identity.1.is_none()
    {
        return Err(invalid_attachment_file(
            "attachment entry is not a direct single-link regular file",
        ));
    }
    let mut options = fs::OpenOptions::new();
    let file = options
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let opened = file.metadata()?;
    let after = fs::symlink_metadata(path)?;
    let opened_identity = (opened.volume_serial_number(), opened.file_index());
    let after_identity = (after.volume_serial_number(), after.file_index());
    if !opened.is_file()
        || !after.is_file()
        || opened.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || after.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || opened.number_of_links() != Some(1)
        || after.number_of_links() != Some(1)
        || opened_identity != before_identity
        || after_identity != before_identity
    {
        return Err(invalid_attachment_file(
            "attachment entry changed identity while being opened",
        ));
    }
    Ok((file, opened))
}
#[cfg(not(any(unix, windows)))]
fn open_attachment_regular_file_platform(
    _path: &Path,
) -> std::io::Result<(fs::File, fs::Metadata)> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "this platform does not expose a secure direct-file attachment primitive",
    ))
}
/// Read one direct regular attachment entry under a hard byte ceiling.
///
/// # Errors
///
/// Returns an error when secure open fails, the entry exceeds `max_bytes`, or
/// its size or type changes during the read.
pub(super) fn read_bounded_attachment_regular_file(
    path: &Path,
    max_bytes: u64,
) -> std::io::Result<Vec<u8>> {
    let (file, opened_metadata) = open_attachment_regular_file(path)?;
    if opened_metadata.len() > max_bytes {
        return Err(invalid_attachment_file(format!(
            "attachment entry exceeds the {max_bytes}-byte read limit"
        )));
    }
    let mut reader = file.take(max_bytes.saturating_add(1));
    let mut bytes = Vec::with_capacity(
        usize::try_from(opened_metadata.len())
            .map_err(|_| invalid_attachment_file("attachment entry length is not addressable"))?,
    );
    reader.read_to_end(&mut bytes)?;
    let read_size = u64::try_from(bytes.len())
        .map_err(|_| invalid_attachment_file("attachment read length does not fit in u64"))?;
    let final_metadata = reader.get_ref().metadata()?;
    if read_size != opened_metadata.len()
        || final_metadata.len() != opened_metadata.len()
        || !final_metadata.is_file()
    {
        return Err(invalid_attachment_file(
            "attachment entry changed while being read",
        ));
    }
    Ok(bytes)
}
/// Initialize on-disk directories and recover any pending quota transaction.
///
/// Returns `true` when recovery completed or no transaction was pending. A
/// `false` result must prevent consumers such as the prover from observing the
/// attachment store until a later recovery attempt succeeds.
pub fn init_persistence() -> bool {
    ensure_root_dir();
    match recover_attachment_quota_transaction() {
        Ok(_) => true,
        Err(error) => {
            warn!(%error, "failed to recover a pending attachment quota transaction at startup");
            false
        }
    }
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn list_all_ids(tenant: &AttachmentTenant) -> Vec<String> {
    let mut ids = Vec::new();
    if let Ok(rd) = fs::read_dir(attachments_dir(tenant)) {
        for e in rd.flatten() {
            if let Some(name) = e.file_name().to_str() {
                if let Some(id) = name.strip_suffix(".json") {
                    if let Some(sanitized) = sanitize_attachment_id(id) {
                        ids.push(sanitized);
                    }
                }
            }
        }
    }
    ids
}
fn load_meta(tenant: &AttachmentTenant, id: &str) -> Option<AttachmentMeta> {
    let id = sanitize_attachment_id(id)?;
    let path = meta_path(tenant, &id);
    let buf = read_bounded_attachment_regular_file(&path, ATTACHMENT_META_FILE_MAX_BYTES).ok()?;
    let s = std::str::from_utf8(&buf).ok()?;
    let meta = json::from_json::<AttachmentMeta>(s).ok()?;
    validate_attachment_metadata_contract(&meta, tenant.as_str(), &id).ok()?;
    Some(meta)
}
fn save_meta(tenant: &AttachmentTenant, meta: &AttachmentMeta) -> std::io::Result<()> {
    let id = sanitize_attachment_id(&meta.id).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid attachment metadata id",
        )
    })?;
    let body = json::to_json_pretty(meta)
        .map_err(|error| invalid_attachment_file(format!("encode attachment metadata: {error}")))?;
    if body.len() as u64 > ATTACHMENT_META_FILE_MAX_BYTES {
        return Err(invalid_attachment_file(format!(
            "attachment metadata exceeds the {ATTACHMENT_META_FILE_MAX_BYTES}-byte persistence limit"
        )));
    }
    validate_attachment_metadata_contract(meta, tenant.as_str(), &id)
        .map_err(invalid_attachment_file)?;
    let path = meta_path(tenant, &id);
    ensure_attachment_dirs_durable(tenant)?;
    persist_bytes_atomically(&path, body.as_bytes(), ".tmp")?;
    ensure_prover_processing_reference(tenant.as_str(), &id)
}
fn persist_body(tenant: &AttachmentTenant, id: &str, body: &[u8]) -> std::io::Result<()> {
    let id = sanitize_attachment_id(id).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid attachment body id",
        )
    })?;
    let path = bin_path(tenant, &id);
    ensure_attachment_dirs_durable(tenant)?;
    persist_bytes_atomically(&path, body, ".tmp")
}
fn remove_empty_tenant_dir_if_present(tenant: &AttachmentTenant) -> std::io::Result<()> {
    match fs::remove_dir(attachments_dir(tenant)) {
        Ok(()) => sync_directory(&attachments_root_dir()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}
fn delete_attachment_files(tenant: &AttachmentTenant, id: &str) -> std::io::Result<()> {
    if let Some(clean) = sanitize_attachment_id(id) {
        let _guard = prover_processing_state_lock().lock();
        // Drop the live reference first. If the process stops before deleting
        // the attachment files, discovery can safely recreate the reference;
        // the inverse order could leave a ghost reference after a crash.
        remove_prover_processing_reference_locked(tenant.as_str(), &clean)?;
        let meta = meta_path(tenant, &clean);
        let body = bin_path(tenant, &clean);
        remove_file_if_present(&meta)?;
        remove_file_if_present(&body)?;
        remove_empty_tenant_dir_if_present(tenant)?;
    }
    Ok(())
}
fn validate_attachment_quota_transaction(
    transaction: &AttachmentQuotaTransaction,
) -> std::io::Result<()> {
    if transaction.version != ATTACHMENT_QUOTA_TRANSACTION_VERSION {
        return Err(invalid_attachment_file(
            "unsupported attachment quota transaction version",
        ));
    }
    let tenant = sanitize_tenant_key(&transaction.tenant)
        .filter(|tenant| tenant == &transaction.tenant)
        .ok_or_else(|| invalid_attachment_file("invalid attachment quota transaction tenant"))?;
    let incoming_id = sanitize_attachment_id(&transaction.incoming_meta.id)
        .filter(|id| id == &transaction.incoming_meta.id)
        .ok_or_else(|| {
            invalid_attachment_file("invalid attachment quota transaction incoming id")
        })?;
    validate_attachment_metadata_contract(&transaction.incoming_meta, &tenant, &incoming_id)
        .map_err(invalid_attachment_file)?;
    if let Some(previous) = &transaction.previous_meta {
        validate_attachment_metadata_contract(previous, &tenant, &incoming_id)
            .map_err(invalid_attachment_file)?;
    }
    let victim_count = u64::try_from(transaction.victim_ids.len()).unwrap_or(u64::MAX);
    if victim_count == 0 || victim_count > ATTACHMENT_META_SCAN_MAX_FILES {
        return Err(invalid_attachment_file(format!(
            "attachment quota transaction must contain 1..={ATTACHMENT_META_SCAN_MAX_FILES} victims"
        )));
    }
    let mut unique_victims = BTreeSet::new();
    for victim_id in &transaction.victim_ids {
        let victim_id = sanitize_attachment_id(victim_id)
            .filter(|id| id == victim_id)
            .ok_or_else(|| {
                invalid_attachment_file("invalid attachment quota transaction victim id")
            })?;
        if victim_id == incoming_id {
            return Err(invalid_attachment_file(
                "attachment quota transaction cannot evict its incoming attachment",
            ));
        }
        if !unique_victims.insert(victim_id) {
            return Err(invalid_attachment_file(
                "attachment quota transaction contains a duplicate victim",
            ));
        }
    }
    Ok(())
}
fn persist_attachment_quota_transaction(
    transaction: &AttachmentQuotaTransaction,
) -> std::io::Result<()> {
    validate_attachment_quota_transaction(transaction)?;
    let body = json::to_json_pretty(transaction).map_err(|error| {
        invalid_attachment_file(format!("encode attachment quota transaction: {error}"))
    })?;
    if u64::try_from(body.len()).unwrap_or(u64::MAX) > ATTACHMENT_QUOTA_TRANSACTION_MAX_BYTES {
        return Err(invalid_attachment_file(format!(
            "attachment quota transaction exceeds the {ATTACHMENT_QUOTA_TRANSACTION_MAX_BYTES}-byte persistence limit"
        )));
    }
    let path = attachment_quota_transaction_path();
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "an attachment quota transaction is already pending",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    ensure_attachment_quota_transaction_dir_durable()?;
    persist_bytes_atomically(
        &path,
        body.as_bytes(),
        ATTACHMENT_QUOTA_TRANSACTION_TEMP_PREFIX,
    )
}
fn load_attachment_quota_transaction() -> std::io::Result<Option<AttachmentQuotaTransaction>> {
    let path = attachment_quota_transaction_path();
    let body =
        match read_bounded_attachment_regular_file(&path, ATTACHMENT_QUOTA_TRANSACTION_MAX_BYTES) {
            Ok(body) => body,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
    let text = std::str::from_utf8(&body).map_err(|error| {
        invalid_attachment_file(format!("decode quota transaction UTF-8: {error}"))
    })?;
    let transaction = json::from_json::<AttachmentQuotaTransaction>(text).map_err(|error| {
        invalid_attachment_file(format!("decode attachment quota transaction: {error}"))
    })?;
    validate_attachment_quota_transaction(&transaction)?;
    Ok(Some(transaction))
}
fn clear_attachment_quota_transaction() -> std::io::Result<()> {
    remove_file_if_present(&attachment_quota_transaction_path())
}
fn attachment_quota_incoming_is_complete(
    transaction: &AttachmentQuotaTransaction,
) -> std::io::Result<bool> {
    let tenant = AttachmentTenant(transaction.tenant.clone());
    let Some(current_meta) = load_meta(&tenant, &transaction.incoming_meta.id) else {
        return Ok(false);
    };
    if current_meta != transaction.incoming_meta {
        return Ok(false);
    }
    let body = match read_bounded_attachment_regular_file(
        &bin_path(&tenant, &transaction.incoming_meta.id),
        transaction.incoming_meta.size,
    ) {
        Ok(body) => body,
        Err(_) => return Ok(false),
    };
    if validate_attachment_body_contract(&transaction.incoming_meta, &body).is_err() {
        return Ok(false);
    }
    // A metadata rename can complete before its live-reference marker. Repair
    // that final durable side effect before any quota victim is removed.
    ensure_prover_processing_reference(&transaction.tenant, &transaction.incoming_meta.id)?;
    Ok(true)
}
fn rollback_attachment_quota_transaction(
    transaction: &AttachmentQuotaTransaction,
) -> std::io::Result<()> {
    let tenant = AttachmentTenant(transaction.tenant.clone());
    if let Some(previous_meta) = &transaction.previous_meta {
        let body = read_bounded_attachment_regular_file(
            &bin_path(&tenant, &previous_meta.id),
            previous_meta.size,
        )?;
        validate_attachment_body_contract(previous_meta, &body).map_err(invalid_attachment_file)?;
        save_meta(&tenant, previous_meta)?;
    } else {
        delete_attachment_files(&tenant, &transaction.incoming_meta.id)?;
    }
    clear_attachment_quota_transaction()
}
fn finalize_attachment_quota_transaction(
    transaction: &AttachmentQuotaTransaction,
) -> std::io::Result<()> {
    let tenant = AttachmentTenant(transaction.tenant.clone());
    for victim_id in &transaction.victim_ids {
        delete_attachment_files(&tenant, victim_id)?;
    }
    clear_attachment_quota_transaction()
}
/// Recover one quota transaction while the caller holds the attachment quota mutex.
fn recover_attachment_quota_transaction() -> std::io::Result<bool> {
    let Some(transaction) = load_attachment_quota_transaction()? else {
        return Ok(false);
    };
    if attachment_quota_incoming_is_complete(&transaction)? {
        finalize_attachment_quota_transaction(&transaction)?;
    } else {
        rollback_attachment_quota_transaction(&transaction)?;
    }
    Ok(true)
}
fn hash_identity_hex(label: &str, value: &str) -> String {
    let mut buf = Vec::with_capacity(label.len() + 1 + value.len());
    buf.extend_from_slice(label.as_bytes());
    buf.push(b'|');
    buf.extend_from_slice(value.as_bytes());
    let hash = iroha_crypto::Hash::new(&buf);
    let digest: [u8; 32] = hash.into();
    hex::encode::<[u8; 32]>(digest)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SanitizeRejectReason {
    Type,
    Expansion,
    Sandbox,
    Checksum,
}
impl SanitizeRejectReason {
    fn label(self) -> &'static str {
        match self {
            SanitizeRejectReason::Type => "type",
            SanitizeRejectReason::Expansion => "expansion",
            SanitizeRejectReason::Sandbox => "sandbox",
            SanitizeRejectReason::Checksum => "checksum",
        }
    }
    fn from_label(label: &str) -> Option<Self> {
        match label {
            "type" => Some(SanitizeRejectReason::Type),
            "expansion" => Some(SanitizeRejectReason::Expansion),
            "sandbox" => Some(SanitizeRejectReason::Sandbox),
            "checksum" => Some(SanitizeRejectReason::Checksum),
            _ => None,
        }
    }
    fn status_code(self) -> StatusCode {
        match self {
            SanitizeRejectReason::Type => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            SanitizeRejectReason::Expansion => StatusCode::PAYLOAD_TOO_LARGE,
            SanitizeRejectReason::Sandbox | SanitizeRejectReason::Checksum => {
                StatusCode::BAD_REQUEST
            }
        }
    }
}
#[derive(Debug)]
struct SanitizeError {
    reason: SanitizeRejectReason,
    message: String,
}
impl SanitizeError {
    fn new(reason: SanitizeRejectReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
    fn into_wire(self) -> SanitizeErrorWire {
        SanitizeErrorWire {
            reason: self.reason.label().to_string(),
            message: self.message,
        }
    }
    fn from_wire(wire: SanitizeErrorWire) -> Self {
        let reason =
            SanitizeRejectReason::from_label(&wire.reason).unwrap_or(SanitizeRejectReason::Sandbox);
        Self {
            reason,
            message: wire.message,
        }
    }
}
#[derive(Debug, Clone)]
struct SanitizerConfig {
    allowed_mime_types: Vec<String>,
    max_expanded_bytes: u64,
    max_archive_depth: u32,
    timeout: Duration,
    mode: AttachmentSanitizerMode,
}
#[derive(Debug, Clone, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
struct SanitizerSummary {
    sniffed_type: String,
    expanded_bytes: u64,
    archive_depth: u32,
    sandboxed: bool,
}
#[derive(Debug, Clone)]
struct SanitizerOutcome {
    summary: SanitizerSummary,
    sanitized_body: Vec<u8>,
}
#[derive(Debug, Clone, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
struct SanitizeErrorWire {
    reason: String,
    message: String,
}
#[derive(Debug, Clone, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
struct SanitizerRequest {
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    declared_type: Option<String>,
    body: Vec<u8>,
    allowed_mime_types: Vec<String>,
    max_expanded_bytes: u64,
    max_archive_depth: u32,
    timeout_ms: u64,
}
#[derive(Debug, Clone, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
enum SanitizerResponse {
    /// Sanitization succeeded with one summary and exact replacement body.
    Accepted {
        summary: SanitizerSummary,
        sanitized_body: Vec<u8>,
    },
    /// Sanitization rejected the request.
    Rejected { error: SanitizeErrorWire },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SniffedFormat {
    Norito,
    Json,
    Zk1,
    Gzip,
    Zstd,
    Unknown,
}
fn normalize_mime(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mime = trimmed.split(';').next().unwrap_or("").trim();
    if mime.is_empty() {
        return None;
    }
    let mut normalized = mime.to_ascii_lowercase();
    if normalized.starts_with("application/") && normalized.ends_with("+json") {
        normalized = JSON_MIME_TYPE.to_string();
    }
    Some(normalized)
}
fn sniff_format(bytes: &[u8]) -> SniffedFormat {
    if bytes.starts_with(&norito::core::MAGIC) {
        return SniffedFormat::Norito;
    }
    if bytes.len() >= 4 && &bytes[..4] == b"ZK1\0" {
        return SniffedFormat::Zk1;
    }
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        return SniffedFormat::Gzip;
    }
    if bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        return SniffedFormat::Zstd;
    }
    if bytes
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|b| matches!(b, b'{' | b'['))
    {
        return SniffedFormat::Json;
    }
    SniffedFormat::Unknown
}
/// Return the canonical media type for an already-expanded attachment body.
pub(super) fn sniffed_attachment_media_type(bytes: &[u8]) -> Option<&'static str> {
    match sniff_format(bytes) {
        SniffedFormat::Norito => Some(NORITO_MIME_TYPE),
        SniffedFormat::Json => Some(JSON_MIME_TYPE),
        SniffedFormat::Zk1 => Some(ZK1_MIME_TYPE),
        SniffedFormat::Unknown => Some(OCTET_STREAM_MIME_TYPE),
        SniffedFormat::Gzip | SniffedFormat::Zstd => None,
    }
}
fn is_canonical_attachment_digest(value: &str) -> bool {
    value.len() == ATTACHMENT_ID_HEX_LEN
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}
/// Validate invariants that can be checked from persisted metadata alone.
///
/// # Errors
///
/// Returns a descriptive error when identity, tenant, provenance, sanitizer,
/// digest-shape, size, or media-type invariants are violated.
pub(super) fn validate_attachment_metadata_contract(
    meta: &AttachmentMeta,
    expected_tenant: &str,
    expected_id: &str,
) -> Result<(), String> {
    if meta.id != expected_id {
        return Err(format!(
            "proof attachment metadata id {} does not match storage id {expected_id}",
            meta.id
        ));
    }
    if meta.tenant.as_deref() != Some(expected_tenant) {
        return Err("proof attachment metadata tenant does not match its storage namespace".into());
    }
    let provenance = meta.provenance.as_ref().ok_or_else(|| {
        "proof attachment provenance is required by the first-release storage contract".to_owned()
    })?;
    if provenance.sanitizer.verdict != "accepted" {
        return Err("proof attachment provenance sanitizer verdict is not accepted".into());
    }
    if provenance.sanitizer.expanded_bytes != meta.size {
        return Err(format!(
            "proof attachment provenance expanded size {} does not match metadata size {}",
            provenance.sanitizer.expanded_bytes, meta.size
        ));
    }
    if provenance.hashes.blake2b_256 != meta.id
        || !is_canonical_attachment_digest(&provenance.hashes.blake2b_256)
    {
        return Err(
            "proof attachment provenance Blake2b-256 digest does not match metadata id".into(),
        );
    }
    if !is_canonical_attachment_digest(&provenance.hashes.sha256) {
        return Err("proof attachment provenance SHA-256 digest is not canonical".into());
    }
    if provenance.sniffed_type != meta.content_type {
        return Err(
            "proof attachment provenance media type does not match attachment metadata".into(),
        );
    }
    Ok(())
}
/// Validate persisted attachment bytes against their required provenance.
///
/// # Errors
///
/// Returns a descriptive error when size, digest, sanitizer, or media-type
/// provenance does not match `body`.
pub(super) fn validate_attachment_body_contract(
    meta: &AttachmentMeta,
    body: &[u8],
) -> Result<(), String> {
    let actual_size = body.len() as u64;
    if meta.size != actual_size {
        return Err(format!(
            "proof attachment metadata size {} does not match the actual {actual_size}-byte body",
            meta.size
        ));
    }
    let provenance = meta.provenance.as_ref().ok_or_else(|| {
        "proof attachment provenance is required by the first-release storage contract".to_owned()
    })?;
    if provenance.sanitizer.verdict != "accepted" {
        return Err("proof attachment provenance sanitizer verdict is not accepted".into());
    }
    if provenance.sanitizer.expanded_bytes != actual_size {
        return Err(format!(
            "proof attachment provenance expanded size {} does not match the actual {actual_size}-byte body",
            provenance.sanitizer.expanded_bytes
        ));
    }
    let actual_id = hex::encode::<[u8; 32]>(iroha_crypto::Hash::new(body).into());
    if actual_id != meta.id {
        return Err(format!(
            "proof attachment body digest {actual_id} does not match storage id {}",
            meta.id
        ));
    }
    if provenance.hashes.blake2b_256 != actual_id {
        return Err("proof attachment provenance Blake2b-256 digest does not match body".into());
    }
    let actual_sha256 = hex::encode(Sha256::digest(body));
    if provenance.hashes.sha256 != actual_sha256 {
        return Err("proof attachment provenance SHA-256 digest does not match body".into());
    }
    let actual_media_type = sniffed_attachment_media_type(body).ok_or_else(|| {
        "proof attachment body does not have a supported canonical media type".to_owned()
    })?;
    if provenance.sniffed_type != actual_media_type || meta.content_type != actual_media_type {
        return Err(
            "proof attachment provenance and metadata media type do not match the body".into(),
        );
    }
    Ok(())
}
fn read_limited<R: std::io::Read>(
    mut reader: R,
    max_bytes: u64,
    deadline: Instant,
) -> Result<Vec<u8>, SanitizeError> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8 * 1024];
    loop {
        if Instant::now() > deadline {
            return Err(SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                "attachment sanitize timeout exceeded",
            ));
        }
        let read = reader.read(&mut buf).map_err(|err| {
            SanitizeError::new(
                SanitizeRejectReason::Checksum,
                format!("attachment decompress failed: {err}"),
            )
        })?;
        if read == 0 {
            break;
        }
        let next_len = out.len().saturating_add(read);
        if next_len as u64 > max_bytes {
            return Err(SanitizeError::new(
                SanitizeRejectReason::Expansion,
                format!(
                    "attachment expanded beyond max bytes (>{} bytes)",
                    max_bytes
                ),
            ));
        }
        out.extend_from_slice(&buf[..read]);
    }
    Ok(out)
}
fn inspect_bytes(
    bytes: &[u8],
    depth: u32,
    cfg: &SanitizerConfig,
    deadline: Instant,
) -> Result<SanitizerOutcome, SanitizeError> {
    match sniff_format(bytes) {
        SniffedFormat::Norito => Ok(SanitizerOutcome {
            summary: SanitizerSummary {
                sniffed_type: NORITO_MIME_TYPE.to_string(),
                expanded_bytes: bytes.len() as u64,
                archive_depth: depth,
                sandboxed: false,
            },
            sanitized_body: bytes.to_vec(),
        }),
        SniffedFormat::Json => Ok(SanitizerOutcome {
            summary: SanitizerSummary {
                sniffed_type: JSON_MIME_TYPE.to_string(),
                expanded_bytes: bytes.len() as u64,
                archive_depth: depth,
                sandboxed: false,
            },
            sanitized_body: bytes.to_vec(),
        }),
        SniffedFormat::Zk1 => Ok(SanitizerOutcome {
            summary: SanitizerSummary {
                sniffed_type: ZK1_MIME_TYPE.to_string(),
                expanded_bytes: bytes.len() as u64,
                archive_depth: depth,
                sandboxed: false,
            },
            sanitized_body: bytes.to_vec(),
        }),
        SniffedFormat::Gzip => {
            if depth >= cfg.max_archive_depth {
                return Err(SanitizeError::new(
                    SanitizeRejectReason::Expansion,
                    format!(
                        "attachment archive depth exceeds limit ({})",
                        cfg.max_archive_depth
                    ),
                ));
            }
            let mut decoder = GzDecoder::new(bytes);
            let expanded = read_limited(&mut decoder, cfg.max_expanded_bytes, deadline)?;
            let mut inner = inspect_bytes(&expanded, depth + 1, cfg, deadline)?;
            inner.summary.archive_depth = inner.summary.archive_depth.max(depth + 1);
            inner.summary.expanded_bytes = inner.sanitized_body.len() as u64;
            Ok(inner)
        }
        SniffedFormat::Zstd => {
            if depth >= cfg.max_archive_depth {
                return Err(SanitizeError::new(
                    SanitizeRejectReason::Expansion,
                    format!(
                        "attachment archive depth exceeds limit ({})",
                        cfg.max_archive_depth
                    ),
                ));
            }
            let mut decoder = ZstdDecoder::new(bytes).map_err(|err| {
                SanitizeError::new(
                    SanitizeRejectReason::Checksum,
                    format!("attachment decompress failed: {err}"),
                )
            })?;
            let expanded = read_limited(&mut decoder, cfg.max_expanded_bytes, deadline)?;
            let mut inner = inspect_bytes(&expanded, depth + 1, cfg, deadline)?;
            inner.summary.archive_depth = inner.summary.archive_depth.max(depth + 1);
            inner.summary.expanded_bytes = inner.sanitized_body.len() as u64;
            Ok(inner)
        }
        SniffedFormat::Unknown => Err(SanitizeError::new(
            SanitizeRejectReason::Type,
            "unsupported attachment format",
        )),
    }
}
fn sanitizer_config() -> SanitizerConfig {
    SanitizerConfig {
        allowed_mime_types: allowed_mime_types_cfg(),
        max_expanded_bytes: max_expanded_bytes_cfg(),
        max_archive_depth: max_archive_depth_cfg(),
        timeout: sanitize_timeout_cfg(),
        mode: sanitizer_mode_cfg(),
    }
}
fn sanitize_attachment_sync(
    declared_type: Option<&str>,
    body: &[u8],
    cfg: &SanitizerConfig,
) -> Result<SanitizerOutcome, SanitizeError> {
    let deadline = Instant::now() + cfg.timeout;
    let mut outcome = match inspect_bytes(body, 0, cfg, deadline) {
        Ok(outcome) => outcome,
        Err(err) if err.reason == SanitizeRejectReason::Type => SanitizerOutcome {
            summary: SanitizerSummary {
                sniffed_type: OCTET_STREAM_MIME_TYPE.to_string(),
                expanded_bytes: body.len() as u64,
                archive_depth: 0,
                sandboxed: false,
            },
            sanitized_body: body.to_vec(),
        },
        Err(err) => return Err(err),
    };
    outcome.summary.expanded_bytes = outcome.sanitized_body.len() as u64;
    if outcome.summary.expanded_bytes > cfg.max_expanded_bytes {
        return Err(SanitizeError::new(
            SanitizeRejectReason::Expansion,
            format!(
                "attachment expanded beyond max bytes (>{} bytes)",
                cfg.max_expanded_bytes
            ),
        ));
    }
    let declared_norm = declared_type.and_then(normalize_mime);
    if let Some(ref declared) = declared_norm {
        if declared != OCTET_STREAM_MIME_TYPE && declared != &outcome.summary.sniffed_type {
            return Err(SanitizeError::new(
                SanitizeRejectReason::Type,
                format!(
                    "declared content-type `{declared}` does not match sniffed `{}`",
                    outcome.summary.sniffed_type
                ),
            ));
        }
    }
    if !cfg.allowed_mime_types.is_empty()
        && !cfg
            .allowed_mime_types
            .iter()
            .any(|allowed| allowed == &outcome.summary.sniffed_type)
    {
        return Err(SanitizeError::new(
            SanitizeRejectReason::Type,
            format!(
                "attachment type `{}` is not allowlisted",
                outcome.summary.sniffed_type
            ),
        ));
    }
    Ok(outcome)
}
async fn sanitize_attachment(
    declared_type: Option<String>,
    body: axum::body::Bytes,
    admission: Option<crate::ProofBodyAdmissionLease>,
) -> Result<SanitizerOutcome, SanitizeError> {
    let cfg = sanitizer_config();
    match cfg.mode {
        AttachmentSanitizerMode::InProcess => {
            sanitize_attachment_in_process(declared_type, body, cfg, admission).await
        }
        AttachmentSanitizerMode::Subprocess => {
            sanitize_attachment_subprocess(declared_type, body, cfg, admission).await
        }
    }
}
#[cfg(test)]
struct SanitizerWorkerTestGate {
    entered: Arc<tokio::sync::Notify>,
    release: SyncMutex<mpsc::Receiver<()>>,
}
#[cfg(test)]
fn sanitizer_worker_test_gate() -> &'static SyncMutex<Option<Arc<SanitizerWorkerTestGate>>> {
    static GATE: OnceLock<SyncMutex<Option<Arc<SanitizerWorkerTestGate>>>> = OnceLock::new();
    GATE.get_or_init(|| SyncMutex::new(None))
}
#[cfg(test)]
pub(crate) struct SanitizerWorkerTestGateGuard;
#[cfg(test)]
impl Drop for SanitizerWorkerTestGateGuard {
    fn drop(&mut self) {
        *sanitizer_worker_test_gate().lock() = None;
    }
}
#[cfg(test)]
pub(crate) fn install_sanitizer_worker_test_gate(
    entered: Arc<tokio::sync::Notify>,
    release: mpsc::Receiver<()>,
) -> SanitizerWorkerTestGateGuard {
    let mut slot = sanitizer_worker_test_gate().lock();
    assert!(
        slot.is_none(),
        "sanitizer worker test gate already installed"
    );
    *slot = Some(Arc::new(SanitizerWorkerTestGate {
        entered,
        release: SyncMutex::new(release),
    }));
    SanitizerWorkerTestGateGuard
}
#[cfg(test)]
fn wait_for_sanitizer_worker_test_gate() {
    let gate = sanitizer_worker_test_gate().lock().clone();
    if let Some(gate) = gate {
        gate.entered.notify_one();
        gate.release
            .lock()
            .recv()
            .expect("release sanitizer worker test gate");
    }
}
async fn sanitize_attachment_in_process(
    declared_type: Option<String>,
    body: axum::body::Bytes,
    cfg: SanitizerConfig,
    admission: Option<crate::ProofBodyAdmissionLease>,
) -> Result<SanitizerOutcome, SanitizeError> {
    let declared = declared_type.clone();
    let mut outcome = task::spawn_blocking(move || {
        #[cfg(test)]
        wait_for_sanitizer_worker_test_gate();
        let outcome = sanitize_attachment_sync(declared.as_deref(), body.as_ref(), &cfg);
        drop(admission);
        outcome
    })
    .await
    .map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitize task failed: {err}"),
        )
    })??;
    outcome.summary.sandboxed = false;
    Ok(outcome)
}
async fn sanitize_attachment_subprocess(
    declared_type: Option<String>,
    body: axum::body::Bytes,
    cfg: SanitizerConfig,
    admission: Option<crate::ProofBodyAdmissionLease>,
) -> Result<SanitizerOutcome, SanitizeError> {
    let request = SanitizerRequest {
        declared_type,
        body: body.to_vec(),
        allowed_mime_types: cfg.allowed_mime_types.clone(),
        max_expanded_bytes: cfg.max_expanded_bytes,
        max_archive_depth: cfg.max_archive_depth,
        timeout_ms: cfg.timeout.as_millis().max(1) as u64,
    };
    let outcome = task::spawn_blocking(move || {
        #[cfg(test)]
        wait_for_sanitizer_worker_test_gate();
        let outcome = run_sanitizer_subprocess(request, cfg.timeout, admission.clone());
        drop(admission);
        outcome
    })
    .await
    .map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitize task failed: {err}"),
        )
    })??;
    Ok(outcome)
}
fn run_sanitizer_subprocess(
    request: SanitizerRequest,
    timeout: Duration,
    admission: Option<crate::ProofBodyAdmissionLease>,
) -> Result<SanitizerOutcome, SanitizeError> {
    let exe = sanitizer_executable()?;
    let exe = validate_sanitizer_executable(&exe)?;
    let request_bytes = norito::encode_canonical(&request).map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer request encode failed: {err}"),
        )
    })?;
    let max_input_bytes = request_bytes
        .len()
        .saturating_add(1024)
        .max(1024)
        .to_string();
    let max_output_bytes = usize::try_from(request.max_expanded_bytes)
        .unwrap_or(usize::MAX)
        .saturating_add(SANITIZER_RESPONSE_OVERHEAD_BYTES);
    let mut cmd = sandboxed_sanitizer_command(&exe, &max_input_bytes)?;
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer spawn failed: {err}"),
        )
    })?;
    let result = (|| -> Result<SanitizerOutcome, SanitizeError> {
        {
            let mut stdin = child.stdin.take().ok_or_else(|| {
                SanitizeError::new(
                    SanitizeRejectReason::Sandbox,
                    "attachment sanitizer stdin unavailable",
                )
            })?;
            stdin.write_all(&request_bytes).map_err(|err| {
                SanitizeError::new(
                    SanitizeRejectReason::Sandbox,
                    format!("attachment sanitizer write failed: {err}"),
                )
            })?;
        }
        let stdout = child.stdout.take().ok_or_else(|| {
            SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                "attachment sanitizer stdout unavailable",
            )
        })?;
        let rx = spawn_sanitizer_stdout_reader(stdout, max_output_bytes, admission);
        let deadline = Instant::now() + timeout;
        loop {
            let Some(status) = child.try_wait().map_err(|err| {
                SanitizeError::new(
                    SanitizeRejectReason::Sandbox,
                    format!("attachment sanitizer wait failed: {err}"),
                )
            })?
            else {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    return Err(SanitizeError::new(
                        SanitizeRejectReason::Sandbox,
                        "attachment sanitize timeout exceeded",
                    ));
                }
                thread::sleep(Duration::from_millis(SANITIZER_POLL_INTERVAL_MS));
                continue;
            };
            if !status.success() {
                return Err(SanitizeError::new(
                    SanitizeRejectReason::Sandbox,
                    format!("attachment sanitizer exited with {status}"),
                ));
            }
            break;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let stdout_bytes = rx
            .recv_timeout(remaining)
            .map_err(|_| {
                SanitizeError::new(
                    SanitizeRejectReason::Sandbox,
                    "attachment sanitizer output timeout exceeded",
                )
            })?
            .map_err(|err| SanitizeError::new(SanitizeRejectReason::Sandbox, err))?;
        decode_sanitizer_response_bytes(&stdout_bytes)
    })();
    if result.is_err() {
        // Best-effort cleanup. If the sanitizer is still running (e.g. timeout),
        // ensure we kill and reap it to avoid leaking a zombie process.
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}
fn spawn_sanitizer_stdout_reader(
    mut stdout: impl std::io::Read + Send + 'static,
    max_output_bytes: usize,
    admission: Option<crate::ProofBodyAdmissionLease>,
) -> mpsc::Receiver<Result<Vec<u8>, String>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        // A sandbox descendant can inherit stdout after the direct child exits.
        // Keep physical admission for as long as this reader can remain blocked
        // so detached pipe readers cannot accumulate outside the semaphore.
        let admission = admission;
        let result = read_sanitizer_stdout_limited(&mut stdout, max_output_bytes);
        drop(admission);
        let _ = tx.send(result);
    });
    rx
}
fn read_sanitizer_stdout_limited(
    stdout: &mut impl std::io::Read,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let read = stdout
            .read(&mut chunk)
            .map_err(|err| format!("attachment sanitizer stdout read failed: {err}"))?;
        if read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(read) > max_bytes {
            return Err(format!(
                "attachment sanitizer output exceeds {max_bytes} bytes"
            ));
        }
        output.extend_from_slice(&chunk[..read]);
    }
}
fn validate_sanitizer_executable(exe: &Path) -> Result<PathBuf, SanitizeError> {
    let canonical = fs::canonicalize(exe).map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer spawn failed: {err}"),
        )
    })?;
    let metadata = fs::metadata(&canonical).map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer spawn failed: {err}"),
        )
    })?;
    if !metadata.is_file() {
        return Err(SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!(
                "attachment sanitizer spawn failed: {} is not a file",
                canonical.display()
            ),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                format!(
                    "attachment sanitizer spawn failed: {} is not executable",
                    canonical.display()
                ),
            ));
        }
    }
    let current_exe = env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|err| {
            SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                format!("current node executable unavailable: {err}"),
            )
        })?;
    if canonical == current_exe {
        return Err(SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            "attachment sanitizer must be a dedicated executable, not the node binary",
        ));
    }
    let expected_name = format!(
        "{ATTACHMENT_SANITIZER_BINARY_STEM}{}",
        env::consts::EXE_SUFFIX
    );
    if canonical.file_name() != Some(OsStr::new(&expected_name)) {
        return Err(SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer executable must be named {expected_name}"),
        ));
    }
    Ok(canonical)
}
fn decode_sanitizer_response_bytes(stdout_bytes: &[u8]) -> Result<SanitizerOutcome, SanitizeError> {
    let response = norito::decode_canonical::<SanitizerResponse>(stdout_bytes).map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer response decode failed: {err}"),
        )
    })?;
    match response {
        SanitizerResponse::Accepted {
            summary,
            sanitized_body,
        } => Ok(SanitizerOutcome {
            summary,
            sanitized_body,
        }),
        SanitizerResponse::Rejected { error } => Err(SanitizeError::from_wire(error)),
    }
}
fn sanitizer_executable() -> Result<PathBuf, SanitizeError> {
    let override_path = attach_cfg().read().sanitizer_exe_override.clone();
    sanitizer_executable_with_override(override_path)
}
fn sanitizer_executable_with_override(
    override_path: Option<PathBuf>,
) -> Result<PathBuf, SanitizeError> {
    if let Some(path) = override_path {
        return Ok(path);
    }
    let current_exe = env::current_exe().map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer executable unavailable: {err}"),
        )
    })?;
    let directory = current_exe.parent().ok_or_else(|| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!(
                "attachment sanitizer executable unavailable: {} has no parent directory",
                current_exe.display()
            ),
        )
    })?;
    Ok(directory.join(format!(
        "{ATTACHMENT_SANITIZER_BINARY_STEM}{}",
        env::consts::EXE_SUFFIX
    )))
}
fn sandboxed_sanitizer_command(
    exe: &Path,
    max_input_bytes: &str,
) -> Result<Command, SanitizeError> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let search_path = Some(OsStr::new("/usr/bin:/bin"));
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let search_path = None;
    sandboxed_sanitizer_command_for_search_path(exe, max_input_bytes, search_path)
}
fn sandboxed_sanitizer_command_for_search_path(
    exe: &Path,
    max_input_bytes: &str,
    search_path: Option<&OsStr>,
) -> Result<Command, SanitizeError> {
    #[cfg(target_os = "linux")]
    {
        if let Some(bubblewrap) = find_executable_in_search_path(search_path, "bwrap") {
            let mut cmd = Command::new(bubblewrap);
            set_clean_sanitizer_environment(&mut cmd, max_input_bytes);
            cmd.args([
                "--die-with-parent",
                "--new-session",
                "--unshare-user",
                "--unshare-pid",
                "--unshare-net",
                "--unshare-uts",
                "--unshare-ipc",
                "--clearenv",
                "--tmpfs",
                "/",
                "--dev",
                "/dev",
                "--proc",
                "/proc",
                "--tmpfs",
                "/tmp",
                "--chdir",
                "/tmp",
                "--setenv",
                ATTACHMENT_SANITIZER_ENV,
                "1",
                "--setenv",
                ATTACHMENT_SANITIZER_MAX_INPUT_ENV,
                max_input_bytes,
                "--setenv",
                ATTACHMENT_SANITIZER_SANDBOXED_ENV,
                "1",
            ]);
            add_bwrap_runtime_path(&mut cmd, Path::new("/usr"))?;
            add_bwrap_runtime_path(&mut cmd, Path::new("/lib"))?;
            add_bwrap_runtime_path(&mut cmd, Path::new("/lib64"))?;
            cmd.arg("--ro-bind")
                .arg(exe)
                .arg("/attachment_sanitizer")
                .arg("/attachment_sanitizer");
            return Ok(cmd);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(sandbox_exec) = find_executable_in_search_path(search_path, "sandbox-exec") {
            let executable_literal = macos_sandbox_literal(exe)?;
            let profile = format!(
                r#"(version 1)
(deny default)
(allow process-exec (literal "{executable_literal}"))
(allow file-read-metadata)
(allow file-read-data
    (literal "{executable_literal}")
    (subpath "/System/Library")
    (subpath "/usr/lib")
    (subpath "/private/var/db/dyld"))
(allow sysctl-read)
(deny network*)"#
            );
            let mut cmd = Command::new(sandbox_exec);
            set_clean_sanitizer_environment(&mut cmd, max_input_bytes);
            cmd.arg("-p").arg(profile).arg(exe);
            return Ok(cmd);
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let _ = search_path;
    Err(SanitizeError::new(
        SanitizeRejectReason::Sandbox,
        "attachment sanitizer OS sandbox unavailable",
    ))
}
#[cfg(target_os = "linux")]
fn add_bwrap_runtime_path(cmd: &mut Command, path: &Path) -> Result<(), SanitizeError> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(path).map_err(|err| {
            SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                format!(
                    "attachment sanitizer sandbox cannot resolve {}: {err}",
                    path.display()
                ),
            )
        })?;
        cmd.arg("--symlink").arg(target).arg(path);
    } else if metadata.is_dir() {
        cmd.arg("--ro-bind").arg(path).arg(path);
    }
    Ok(())
}
fn set_clean_sanitizer_environment(cmd: &mut Command, max_input_bytes: &str) {
    cmd.env_clear()
        .env(ATTACHMENT_SANITIZER_ENV, "1")
        .env(ATTACHMENT_SANITIZER_MAX_INPUT_ENV, max_input_bytes)
        .env(ATTACHMENT_SANITIZER_SANDBOXED_ENV, "1");
}
#[cfg(target_os = "macos")]
fn macos_sandbox_literal(path: &Path) -> Result<String, SanitizeError> {
    let raw = path.to_str().ok_or_else(|| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            "attachment sanitizer path is not valid UTF-8",
        )
    })?;
    Ok(raw.replace('\\', "\\\\").replace('"', "\\\""))
}
fn find_executable_in_search_path(search_path: Option<&OsStr>, name: &str) -> Option<PathBuf> {
    let path = search_path?;
    for dir in env::split_paths(path) {
        let candidate = dir.join(name);
        if executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}
fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        return metadata.permissions().mode() & 0o111 != 0;
    }
    #[cfg(not(unix))]
    true
}
#[derive(Debug, Clone, Copy)]
struct AttachmentQuotaLimits {
    per_tenant_max_count: u64,
    per_tenant_max_bytes: u64,
    global_max_count: u64,
    global_max_bytes: u64,
}
#[derive(Debug)]
struct AttachmentQuotaScanBudget {
    metadata_records: u64,
    child_entries: u64,
    child_entry_limit: u64,
}
impl AttachmentQuotaScanBudget {
    fn new(child_entry_limit: u64) -> Self {
        Self {
            metadata_records: 0,
            child_entries: 0,
            child_entry_limit,
        }
    }
}
fn quota_scan_entry_within_limit(scanned: &mut u64, limit: u64) -> bool {
    *scanned = (*scanned).saturating_add(1);
    *scanned <= limit
}
fn quota_metas_for_tenant(
    tenant: &AttachmentTenant,
    scan: &mut AttachmentQuotaScanBudget,
) -> std::io::Result<Vec<AttachmentMeta>> {
    let entries = match fs::read_dir(attachments_dir(tenant)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut metas = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !quota_scan_entry_within_limit(&mut scan.child_entries, scan.child_entry_limit) {
            return Err(invalid_attachment_file(format!(
                "attachment quota scan exceeds {} aggregate tenant child entries",
                scan.child_entry_limit
            )));
        }
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(id) = file_name
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
        else {
            continue;
        };
        let Some(id) = sanitize_attachment_id(id) else {
            continue;
        };
        scan.metadata_records = scan.metadata_records.saturating_add(1);
        if scan.metadata_records > ATTACHMENT_META_SCAN_MAX_FILES {
            return Err(invalid_attachment_file(format!(
                "attachment quota scan exceeds {ATTACHMENT_META_SCAN_MAX_FILES} metadata records"
            )));
        }
        let meta = load_meta(tenant, &id).ok_or_else(|| {
            invalid_attachment_file(format!(
                "attachment quota scan found invalid metadata for {id}"
            ))
        })?;
        metas.push(meta);
    }
    Ok(metas)
}
fn other_tenants_quota_usage(
    submitting_tenant: &AttachmentTenant,
    scan: &mut AttachmentQuotaScanBudget,
) -> std::io::Result<(u64, u64)> {
    let entries = match fs::read_dir(attachments_root_dir()) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((0, 0)),
        Err(error) => return Err(error),
    };
    let mut count = 0_u64;
    let mut bytes = 0_u64;
    let mut root_entries_scanned = 0_u64;
    for entry in entries {
        let entry = entry?;
        if !quota_scan_entry_within_limit(&mut root_entries_scanned, ATTACHMENT_META_SCAN_MAX_FILES)
        {
            return Err(invalid_attachment_file(format!(
                "attachment quota root scan exceeds {ATTACHMENT_META_SCAN_MAX_FILES} entries"
            )));
        }
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(tenant_key) = file_name.to_str().and_then(sanitize_tenant_key) else {
            continue;
        };
        if tenant_key == submitting_tenant.as_str() {
            continue;
        }
        let tenant = AttachmentTenant(tenant_key);
        for meta in quota_metas_for_tenant(&tenant, scan)? {
            count = count.saturating_add(1);
            bytes = bytes.saturating_add(meta.size);
        }
    }
    Ok((count, bytes))
}
fn plan_attachment_quota_admission(
    tenant: &AttachmentTenant,
    incoming_id: &str,
    incoming_size: u64,
) -> std::io::Result<Option<Vec<String>>> {
    let limits = quota_limits_cfg();
    let mut scan = AttachmentQuotaScanBudget::new(ATTACHMENT_TENANT_SCAN_MAX_ENTRIES);
    let mut metas = quota_metas_for_tenant(tenant, &mut scan)?;
    let current_count = metas.len();
    // A content-addressed repost replaces the same tenant-local entry. Do not
    // count or evict it as if another attachment were being added: doing so
    // would also discard the durable prover-processing reference for content
    // that remains live after this request.
    metas.retain(|meta| meta.id != incoming_id);
    metas.sort_by(|a, b| {
        a.created_ms
            .cmp(&b.created_ms)
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut total_bytes = metas
        .iter()
        .fold(0_u64, |total, meta| total.saturating_add(meta.size));
    let mut count_after_add = u64::try_from(metas.len())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let per_tenant_max_count = if limits.per_tenant_max_count == 0 {
        u64::MAX
    } else {
        limits.per_tenant_max_count
    };
    let per_tenant_max_bytes = if limits.per_tenant_max_bytes == 0 {
        u64::MAX
    } else {
        limits.per_tenant_max_bytes
    };
    let (other_tenants_count, other_tenants_bytes) = other_tenants_quota_usage(tenant, &mut scan)?;
    let mut idx = 0usize;
    let mut removed_ids: Vec<String> = Vec::new();
    while (count_after_add > per_tenant_max_count
        || total_bytes.saturating_add(incoming_size) > per_tenant_max_bytes
        || other_tenants_count.saturating_add(count_after_add) > limits.global_max_count
        || other_tenants_bytes
            .saturating_add(total_bytes)
            .saturating_add(incoming_size)
            > limits.global_max_bytes)
        && idx < metas.len()
    {
        let victim = &metas[idx];
        removed_ids.push(victim.id.clone());
        total_bytes = total_bytes.saturating_sub(victim.size);
        count_after_add = count_after_add.saturating_sub(1);
        idx += 1;
    }
    let exceeds_per_tenant = count_after_add > per_tenant_max_count
        || total_bytes.saturating_add(incoming_size) > per_tenant_max_bytes;
    let exceeds_global = other_tenants_count.saturating_add(count_after_add)
        > limits.global_max_count
        || other_tenants_bytes
            .saturating_add(total_bytes)
            .saturating_add(incoming_size)
            > limits.global_max_bytes;
    if exceeds_per_tenant || exceeds_global {
        warn!(
            tenant = tenant.as_str(),
            per_tenant_max_count,
            per_tenant_max_bytes,
            global_max_count = limits.global_max_count,
            global_max_bytes = limits.global_max_bytes,
            current_count,
            current_bytes = total_bytes,
            other_tenants_count,
            other_tenants_bytes,
            incoming_bytes = incoming_size,
            "rejecting attachment: unable to make room within attachment quotas"
        );
        return Ok(None);
    }
    // Feasibility is established without mutating retained state. The caller
    // must durably commit the incoming attachment before deleting these
    // tenant-local victims, so a later write failure or process stop cannot
    // discard old data while leaving no replacement.
    if !removed_ids.is_empty() {
        debug!(
            tenant = tenant.as_str(),
            planned_evictions = removed_ids.len(),
            per_tenant_max_count,
            per_tenant_max_bytes,
            global_max_count = limits.global_max_count,
            global_max_bytes = limits.global_max_bytes,
            count_after_add,
            bytes_after_removal = total_bytes,
            incoming_bytes = incoming_size,
            "planned tenant-local attachment evictions after durable commit"
        );
    }
    Ok(Some(removed_ids))
}
/// POST /v1/zk/attachments — store an attachment and return its metadata.
pub async fn handle_post_attachment(
    tenant: AttachmentTenant,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    handle_post_attachment_inner(tenant, headers, body, None).await
}
pub(crate) async fn handle_post_attachment_with_admission(
    tenant: AttachmentTenant,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
    admission: crate::ProofBodyAdmissionLease,
) -> axum::response::Response {
    handle_post_attachment_inner(tenant, headers, body, Some(admission)).await
}
async fn handle_post_attachment_inner(
    tenant: AttachmentTenant,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
    admission: Option<crate::ProofBodyAdmissionLease>,
) -> axum::response::Response {
    // Enforce size cap
    if body.len() > max_bytes_cfg() {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("attachment too large (>{} bytes)", max_bytes_cfg()),
        )
            .into_response();
    }
    let raw_hash = {
        let h = iroha_crypto::Hash::new(&body);
        hex::encode::<[u8; 32]>(h.into())
    };
    let declared_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(normalize_mime);
    let sanitize_start = Instant::now();
    let sanitize_result = sanitize_attachment(declared_type.clone(), body.clone(), admission).await;
    let sanitize_ms = sanitize_start.elapsed().as_millis() as u64;
    let telemetry = telemetry_handle();
    telemetry.with_metrics(|tel| tel.observe_torii_attachment_sanitize_ms(sanitize_ms));
    let sanitized = match sanitize_result {
        Ok(outcome) => outcome,
        Err(err) => {
            telemetry.with_metrics(|tel| tel.inc_torii_attachment_reject(err.reason.label()));
            info!(
                attachment_raw_hash = %raw_hash,
                reason = err.reason.label(),
                "rejecting attachment after sanitization"
            );
            debug!(
                attachment_raw_hash = %raw_hash,
                error = %err.message,
                "attachment sanitize detail"
            );
            return (err.reason.status_code(), err.message).into_response();
        }
    };
    let SanitizerOutcome {
        summary: sanitized_summary,
        sanitized_body,
    } = sanitized;
    let stored_size = sanitized_body.len() as u64;
    let max_bytes = max_bytes_u64_cfg();
    if stored_size > max_bytes {
        warn!(
            tenant = tenant.as_str(),
            limit_bytes = max_bytes,
            body_bytes = stored_size,
            "rejecting attachment: sanitized body exceeds per-item byte cap"
        );
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("sanitized attachment exceeds max bytes (>{max_bytes} bytes)"),
        )
            .into_response();
    }
    let per_tenant_max_bytes = per_tenant_max_bytes_cfg();
    if per_tenant_max_bytes > 0 && stored_size > per_tenant_max_bytes {
        warn!(
            tenant = tenant.as_str(),
            limit_bytes = per_tenant_max_bytes,
            body_bytes = stored_size,
            "rejecting attachment: exceeds per-tenant byte cap"
        );
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "attachment exceeds per-tenant max bytes (>{} bytes)",
                per_tenant_max_bytes
            ),
        )
            .into_response();
    }
    let global_max_bytes = global_max_bytes_cfg();
    if stored_size > global_max_bytes {
        warn!(
            tenant = tenant.as_str(),
            limit_bytes = global_max_bytes,
            body_bytes = stored_size,
            "rejecting attachment: exceeds node-global byte cap"
        );
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("attachment exceeds node-global max bytes (>{global_max_bytes} bytes)"),
        )
            .into_response();
    }
    let zk1_tags = if sanitized_body.starts_with(b"ZK1\0") {
        match parse_zk1_tags(&sanitized_body) {
            Ok(tags) => (!tags.is_empty()).then_some(tags),
            Err(error) => {
                telemetry.with_metrics(|tel| tel.inc_torii_attachment_reject("format"));
                return (
                    StatusCode::BAD_REQUEST,
                    format!("invalid ZK1 envelope: {error}"),
                )
                    .into_response();
            }
        }
    } else {
        None
    };
    let id = {
        let h = iroha_crypto::Hash::new(&sanitized_body);
        hex::encode::<[u8; 32]>(h.into())
    };
    let _guard = quota_lock().lock().await;
    if let Err(error) = recover_attachment_quota_transaction() {
        warn!(
            tenant = tenant.as_str(),
            %error,
            "rejecting attachment: pending quota transaction recovery failed"
        );
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "attachment quota recovery failed".to_string(),
        )
            .into_response();
    }
    let previous_meta = load_meta(&tenant, &id);
    let replacing_existing = previous_meta.is_some();
    let eviction_ids = match plan_attachment_quota_admission(&tenant, &id, stored_size) {
        Ok(Some(eviction_ids)) => eviction_ids,
        Ok(None) => {
            warn!(
                tenant = tenant.as_str(),
                body_bytes = stored_size,
                "rejecting attachment: node-global quota exceeded"
            );
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                "node-global attachment quota exceeded".to_string(),
            )
                .into_response();
        }
        Err(error) => {
            warn!(
                tenant = tenant.as_str(),
                body_bytes = stored_size,
                %error,
                "rejecting attachment: quota accounting failed"
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "attachment quota accounting failed".to_string(),
            )
                .into_response();
        }
    };
    let sha256 = Sha256::digest(&sanitized_body);
    let hashes = AttachmentHashes {
        blake2b_256: id.clone(),
        sha256: hex::encode(sha256),
    };
    let meta = AttachmentMeta {
        id: id.clone(),
        content_type: sanitized_summary.sniffed_type.clone(),
        size: stored_size,
        created_ms: now_ms(),
        tenant: Some(tenant.as_str().to_string()),
        provenance: Some(AttachmentProvenance {
            declared_type,
            sniffed_type: sanitized_summary.sniffed_type,
            hashes,
            sanitizer: AttachmentSanitizerVerdict {
                verdict: "accepted".to_string(),
                expanded_bytes: sanitized_summary.expanded_bytes,
                archive_depth: sanitized_summary.archive_depth,
                sandboxed: sanitized_summary.sandboxed,
            },
        }),
        zk1_tags,
    };
    let quota_transaction = if eviction_ids.is_empty() {
        None
    } else {
        if let Some(previous_meta) = &previous_meta {
            let previous_body = match read_bounded_attachment_regular_file(
                &bin_path(&tenant, &id),
                previous_meta.size,
            ) {
                Ok(body) => body,
                Err(error) => {
                    warn!(
                        tenant = tenant.as_str(),
                        attachment_id = %id,
                        %error,
                        "rejecting attachment: existing replacement body is not recoverable"
                    );
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "existing attachment is not recoverable".to_string(),
                    )
                        .into_response();
                }
            };
            if let Err(error) = validate_attachment_body_contract(previous_meta, &previous_body) {
                warn!(
                    tenant = tenant.as_str(),
                    attachment_id = %id,
                    %error,
                    "rejecting attachment: existing replacement body failed validation"
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "existing attachment is not recoverable".to_string(),
                )
                    .into_response();
            }
        }
        let transaction = AttachmentQuotaTransaction {
            version: ATTACHMENT_QUOTA_TRANSACTION_VERSION,
            tenant: tenant.as_str().to_owned(),
            incoming_meta: meta.clone(),
            previous_meta: previous_meta.clone(),
            victim_ids: eviction_ids.clone(),
        };
        if let Err(error) = persist_attachment_quota_transaction(&transaction) {
            warn!(
                tenant = tenant.as_str(),
                attachment_id = %id,
                %error,
                "rejecting attachment: failed to persist quota transaction"
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to persist attachment quota transaction".to_string(),
            )
                .into_response();
        }
        Some(transaction)
    };
    if let Err(e) = persist_body(&tenant, &id, &sanitized_body) {
        let rollback = if quota_transaction.is_some() {
            recover_attachment_quota_transaction().map(|_| ())
        } else if replacing_existing {
            Ok(())
        } else {
            delete_attachment_files(&tenant, &id)
        };
        if let Err(error) = rollback {
            warn!(
                tenant = tenant.as_str(),
                attachment_id = %id,
                %error,
                "failed to roll back attachment quota transaction after body persistence failure"
            );
        }
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to persist body: {e}"),
        )
            .into_response();
    }
    if let Err(e) = save_meta(&tenant, &meta) {
        let rollback = if quota_transaction.is_some() {
            recover_attachment_quota_transaction().map(|_| ())
        } else if replacing_existing {
            Ok(())
        } else {
            delete_attachment_files(&tenant, &id)
        };
        if let Err(error) = rollback {
            warn!(
                tenant = tenant.as_str(),
                attachment_id = %id,
                %error,
                "failed to roll back attachment quota transaction after metadata persistence failure"
            );
        }
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to persist metadata: {e}"),
        )
            .into_response();
    }
    if quota_transaction.is_some() {
        match recover_attachment_quota_transaction() {
            Ok(true) => {
                info!(
                    tenant = tenant.as_str(),
                    planned = eviction_ids.len(),
                    "completed durable tenant-local attachment quota transaction"
                );
            }
            Ok(false) => {
                error!(
                    tenant = tenant.as_str(),
                    attachment_id = %id,
                    "durable quota transaction disappeared before eviction"
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "attachment quota transaction was lost".to_string(),
                )
                    .into_response();
            }
            Err(error) => {
                warn!(
                    tenant = tenant.as_str(),
                    attachment_id = %id,
                    %error,
                    "durable attachment committed but quota eviction recovery failed"
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "attachment quota eviction failed".to_string(),
                )
                    .into_response();
            }
        }
    }
    let body = json::to_json_pretty(&meta).unwrap_or_else(|_| "{}".into());
    (
        StatusCode::CREATED,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response()
}
/// GET /v1/zk/attachments — list stored attachments metadata.
pub async fn handle_list_attachments(tenant: AttachmentTenant) -> impl IntoResponse {
    handle_list_attachments_filtered(tenant, NoritoQuery(AttachmentListQuery::default())).await
}
#[derive(
    Debug, Default, Clone, crate::json_macros::JsonDeserialize, norito::derive::NoritoDeserialize,
)]
/// Optional filters and projection for attachments listing
pub struct AttachmentListQuery {
    /// Exact id match (64-hex). If provided, only this id is returned if present.
    pub id: Option<String>,
    /// Substring match on content type (case-sensitive).
    pub content_type: Option<String>,
    /// Return only attachments with created_ms >= since_ms
    pub since_ms: Option<u64>,
    /// Return only attachments with created_ms <= before_ms
    pub before_ms: Option<u64>,
    /// Require a ZK1 tag to be present (e.g., "PROF").
    pub has_tag: Option<String>,
    /// Result limit (max 1000)
    pub limit: Option<u32>,
    /// Result offset (applied after sort)
    pub offset: Option<u32>,
    /// Sort order: asc|desc (by created_ms)
    pub order: Option<String>,
    /// If true, return only ids (array of strings)
    pub ids_only: Option<bool>,
}
/// GET /v1/zk/attachments with filters
pub async fn handle_list_attachments_filtered(
    tenant: AttachmentTenant,
    NoritoQuery(q): NoritoQuery<AttachmentListQuery>,
) -> impl IntoResponse {
    let mut metas: Vec<AttachmentMeta> = Vec::new();
    let mut scanned = 0_u64;
    let ids = if let Some(id) = q.id.as_deref() {
        let Some(clean) = sanitize_attachment_id(id) else {
            return (
                StatusCode::BAD_REQUEST,
                "invalid attachment id (expected 64 hex characters)",
            )
                .into_response();
        };
        vec![clean]
    } else {
        list_all_ids(&tenant)
    };
    for id in ids {
        scanned = scanned.saturating_add(1);
        if scanned > ATTACHMENT_META_SCAN_MAX_FILES {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                format!(
                    "too many attachment metadata records to scan (>{ATTACHMENT_META_SCAN_MAX_FILES}); narrow filters"
                ),
            )
                .into_response();
        }
        let Some(meta) = load_meta(&tenant, &id) else {
            continue;
        };
        if let Some(ct) = q.content_type.as_deref() {
            if !meta.content_type.contains(ct) {
                continue;
            }
        }
        if !q.since_ms.map_or(true, |since| meta.created_ms >= since) {
            continue;
        }
        if !q.before_ms.map_or(true, |before| meta.created_ms <= before) {
            continue;
        }
        if let Some(tag) = q.has_tag.as_deref() {
            if !attachment_meta_has_tag(&meta, tag) {
                continue;
            }
        }
        metas.push(meta);
    }
    // Sort by created_ms asc (default)
    metas.sort_by_key(|m| m.created_ms);
    if matches!(q.order.as_deref(), Some("desc" | "DESC" | "Desc")) {
        metas.reverse();
    }
    // Offset/limit
    let start = (q.offset.unwrap_or(0) as usize).min(metas.len());
    let end = q.limit.map_or(metas.len(), |lim| {
        let cap = lim.min(1000) as usize;
        (start + cap).min(metas.len())
    });
    let slice = &metas[start..end];
    let body = if q.ids_only.unwrap_or(false) {
        let ids: Vec<String> = slice.iter().map(|m| m.id.clone()).collect();
        json::to_json_pretty(&ids).unwrap_or_else(|_| "[]".into())
    } else {
        // norito::json requires a sized type; serialize a Vec copy of the slice
        let owned: Vec<AttachmentMeta> = slice.to_vec();
        json::to_json_pretty(&owned).unwrap_or_else(|_| "[]".into())
    };
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap()
}
/// GET /v1/zk/attachments/count — return number of attachments matching filters
pub async fn handle_count_attachments(
    tenant: AttachmentTenant,
    NoritoQuery(q): NoritoQuery<AttachmentListQuery>,
) -> impl IntoResponse {
    let mut count = 0u64;
    let mut scanned = 0_u64;
    let ids = if let Some(id) = q.id.as_deref() {
        let Some(clean) = sanitize_attachment_id(id) else {
            return (
                StatusCode::BAD_REQUEST,
                "invalid attachment id (expected 64 hex characters)",
            )
                .into_response();
        };
        vec![clean]
    } else {
        list_all_ids(&tenant)
    };
    for id in ids {
        scanned = scanned.saturating_add(1);
        if scanned > ATTACHMENT_META_SCAN_MAX_FILES {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                format!(
                    "too many attachment metadata records to scan (>{ATTACHMENT_META_SCAN_MAX_FILES}); narrow filters"
                ),
            )
                .into_response();
        }
        let Some(meta) = load_meta(&tenant, &id) else {
            continue;
        };
        if let Some(ct) = q.content_type.as_deref() {
            if !meta.content_type.contains(ct) {
                continue;
            }
        }
        if !q.since_ms.map_or(true, |since| meta.created_ms >= since) {
            continue;
        }
        if !q.before_ms.map_or(true, |before| meta.created_ms <= before) {
            continue;
        }
        if let Some(tag) = q.has_tag.as_deref() {
            if !attachment_meta_has_tag(&meta, tag) {
                continue;
            }
        }
        count = count.saturating_add(1);
    }
    let s = norito::json::to_json_pretty(&crate::json_object(vec![("count", count)]))
        .unwrap_or_else(|_| "{}".into());
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(s))
        .unwrap()
}
fn attachment_meta_has_tag(meta: &AttachmentMeta, tag: &str) -> bool {
    meta.zk1_tags
        .as_ref()
        .is_some_and(|tags| tags.iter().any(|existing| existing == tag))
}
fn needs_export_sanitization(meta: &AttachmentMeta) -> bool {
    meta.provenance
        .as_ref()
        .map_or(true, |prov| prov.sanitizer.archive_depth > 0)
}
/// GET /v1/zk/attachments/{id} — return the stored attachment bytes.
pub async fn handle_get_attachment(
    tenant: AttachmentTenant,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    handle_get_attachment_inner(tenant, id, None).await
}
pub(crate) async fn handle_get_attachment_with_admission(
    tenant: AttachmentTenant,
    AxumPath(id): AxumPath<String>,
    admission: crate::ProofBodyAdmissionLease,
) -> axum::response::Response {
    handle_get_attachment_inner(tenant, id, Some(admission)).await
}
async fn handle_get_attachment_inner(
    tenant: AttachmentTenant,
    id: String,
    admission: Option<crate::ProofBodyAdmissionLease>,
) -> axum::response::Response {
    let Some(clean) = sanitize_attachment_id(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            "invalid attachment id (expected 64 hex characters)",
        )
            .into_response();
    };
    let Some(meta) = load_meta(&tenant, &clean) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let max_bytes = u64::try_from(max_bytes_cfg()).unwrap_or(u64::MAX);
    let Ok(bytes) = read_bounded_attachment_regular_file(&bin_path(&tenant, &clean), max_bytes)
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Err(error) = validate_attachment_body_contract(&meta, &bytes) {
        warn!(attachment_id = %clean, %error, "rejecting attachment export with invalid persisted provenance");
        return StatusCode::NOT_FOUND.into_response();
    }
    if !needs_export_sanitization(&meta) {
        return axum::response::Response::builder()
            .header(axum::http::header::CONTENT_TYPE, meta.content_type)
            .body(axum::body::Body::from(bytes))
            .unwrap();
    }
    let sanitize_result = sanitize_attachment(
        Some(meta.content_type.clone()),
        axum::body::Bytes::from(bytes),
        admission,
    )
    .await;
    let sanitized = match sanitize_result {
        Ok(outcome) => outcome,
        Err(err) => {
            warn!(
                attachment_id = %clean,
                reason = err.reason.label(),
                "rejecting attachment export after sanitization"
            );
            return (err.reason.status_code(), err.message).into_response();
        }
    };
    if sanitized.summary.sniffed_type != meta.content_type {
        warn!(
            attachment_id = %clean,
            declared = %meta.content_type,
            sniffed = %sanitized.summary.sniffed_type,
            "attachment export content-type mismatch"
        );
        return (
            StatusCode::BAD_REQUEST,
            "attachment export content-type mismatch".to_string(),
        )
            .into_response();
    }
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, meta.content_type)
        .body(axum::body::Body::from(sanitized.sanitized_body))
        .unwrap()
}
/// DELETE /v1/zk/attachments/{id} — delete an attachment and its metadata.
pub async fn handle_delete_attachment(
    tenant: AttachmentTenant,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let Some(clean) = sanitize_attachment_id(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            "invalid attachment id (expected 64 hex characters)",
        )
            .into_response();
    };
    let _guard = quota_lock().lock().await;
    if let Err(error) = recover_attachment_quota_transaction() {
        warn!(
            tenant = tenant.as_str(),
            attachment_id = %clean,
            %error,
            "failed to recover attachment quota transaction before deletion"
        );
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "attachment quota recovery failed".to_string(),
        )
            .into_response();
    }
    let existed = meta_path(&tenant, &clean).exists() || bin_path(&tenant, &clean).exists();
    if let Err(error) = delete_attachment_files(&tenant, &clean) {
        warn!(
            tenant = tenant.as_str(),
            attachment_id = %clean,
            %error,
            "failed to delete attachment safely"
        );
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to delete attachment".to_string(),
        )
            .into_response();
    }
    if existed {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
async fn delete_attachment_if_expired(
    tenant: &AttachmentTenant,
    id: &str,
    ttl: Duration,
) -> std::io::Result<bool> {
    delete_attachment_if_expired_with_before_lock(tenant, id, ttl, || {}).await
}
async fn delete_attachment_if_expired_with_before_lock(
    tenant: &AttachmentTenant,
    id: &str,
    ttl: Duration,
    before_lock: impl FnOnce(),
) -> std::io::Result<bool> {
    let Some(id) = sanitize_attachment_id(id) else {
        return Ok(false);
    };
    // POST, explicit DELETE, quota eviction, and TTL collection all mutate the
    // same content-addressed entry. Re-read its timestamp after acquiring the
    // common lock so an old GC observation cannot delete a successful repost.
    before_lock();
    let _guard = quota_lock().lock().await;
    recover_attachment_quota_transaction()?;
    let Some(meta) = load_meta(tenant, &id) else {
        return Ok(false);
    };
    let meta_time = UNIX_EPOCH + Duration::from_millis(meta.created_ms);
    if SystemTime::now()
        .duration_since(meta_time)
        .unwrap_or_default()
        <= ttl
    {
        return Ok(false);
    }
    delete_attachment_files(tenant, &id)?;
    Ok(true)
}
/// Start a background GC worker that removes expired attachments.
pub fn start_gc_worker() {
    ensure_root_dir();
    tokio::spawn(async move {
        let ttl = Duration::from_secs(ttl_secs_cfg());
        let interval = Duration::from_secs(GC_INTERVAL_SECS);
        loop {
            if let Ok(rd) = fs::read_dir(attachments_root_dir()) {
                for e in rd.flatten() {
                    let Ok(file_type) = e.file_type() else {
                        continue;
                    };
                    if !file_type.is_dir() {
                        continue;
                    }
                    let file_name = e.file_name();
                    let Some(name) = file_name.to_str() else {
                        continue;
                    };
                    let Some(tenant_key) = sanitize_tenant_key(name) else {
                        continue;
                    };
                    let tenant = AttachmentTenant(tenant_key);
                    if let Ok(trd) = fs::read_dir(attachments_dir(&tenant)) {
                        for te in trd.flatten() {
                            let te_file_name = te.file_name();
                            let Some(tname) = te_file_name.to_str() else {
                                continue;
                            };
                            let Some(id) = tname.strip_suffix(".json") else {
                                continue;
                            };
                            if let Err(error) = delete_attachment_if_expired(&tenant, id, ttl).await
                            {
                                warn!(
                                    tenant = tenant.as_str(),
                                    attachment_id = %id,
                                    %error,
                                    "failed to garbage-collect an expired attachment safely"
                                );
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(interval).await;
        }
    });
}
#[derive(Debug, Clone)]
struct AttachConfig {
    ttl_secs: u64,
    max_bytes: u64,
    per_tenant_max_count: u64,
    per_tenant_max_bytes: u64,
    global_max_count: u64,
    global_max_bytes: u64,
    allowed_mime_types: Vec<String>,
    max_expanded_bytes: u64,
    max_archive_depth: u32,
    sanitizer_mode: AttachmentSanitizerMode,
    sanitize_timeout_ms: u64,
    sanitizer_exe_override: Option<PathBuf>,
    telemetry: MaybeTelemetry,
}
impl Default for AttachConfig {
    fn default() -> Self {
        Self {
            ttl_secs: ATTACHMENT_TTL_SECS_FALLBACK,
            max_bytes: MAX_ATTACHMENT_BYTES_FALLBACK as u64,
            per_tenant_max_count:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_PER_TENANT_MAX_COUNT,
            per_tenant_max_bytes:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_PER_TENANT_MAX_BYTES,
            global_max_count:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_GLOBAL_MAX_COUNT,
            global_max_bytes:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_GLOBAL_MAX_BYTES,
            allowed_mime_types:
                iroha_config::parameters::defaults::torii::attachments_allowed_mime_types()
                    .into_iter()
                    .filter_map(|entry| normalize_mime(&entry))
                    .collect(),
            max_expanded_bytes:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_EXPANDED_BYTES,
            max_archive_depth:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_ARCHIVE_DEPTH,
            sanitizer_mode: AttachmentSanitizerMode::Subprocess,
            sanitize_timeout_ms:
                iroha_config::parameters::defaults::torii::ATTACHMENTS_SANITIZE_TIMEOUT_MS,
            sanitizer_exe_override: None,
            telemetry: MaybeTelemetry::disabled(),
        }
    }
}
static ATTACH_CFG: OnceLock<RwLock<AttachConfig>> = OnceLock::new();
static ATTACH_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
fn attach_cfg() -> &'static RwLock<AttachConfig> {
    ATTACH_CFG.get_or_init(|| RwLock::new(AttachConfig::default()))
}
#[cfg(test)]
pub(crate) struct SanitizerModeTestGuard {
    previous_mode: AttachmentSanitizerMode,
    previous_executable: Option<PathBuf>,
}
#[cfg(test)]
impl Drop for SanitizerModeTestGuard {
    fn drop(&mut self) {
        let mut config = attach_cfg().write();
        config.sanitizer_mode = self.previous_mode;
        config.sanitizer_exe_override = self.previous_executable.take();
    }
}
#[cfg(test)]
pub(crate) fn set_sanitizer_mode_for_test(
    mode: AttachmentSanitizerMode,
    executable: Option<PathBuf>,
) -> SanitizerModeTestGuard {
    let mut config = attach_cfg().write();
    let guard = SanitizerModeTestGuard {
        previous_mode: config.sanitizer_mode,
        previous_executable: config.sanitizer_exe_override.clone(),
    };
    config.sanitizer_mode = mode;
    config.sanitizer_exe_override = executable;
    guard
}
/// Configure attachment retention and sanitization from Torii config.
/// The sanitizer executable override is intended for tests and tooling.
#[allow(clippy::too_many_arguments)]
pub fn configure(
    ttl_secs: u64,
    max_bytes: u64,
    per_tenant_max_count: u64,
    per_tenant_max_bytes: u64,
    global_max_count: u64,
    global_max_bytes: u64,
    allowed_mime_types: Vec<String>,
    max_expanded_bytes: u64,
    max_archive_depth: u32,
    sanitizer_mode: AttachmentSanitizerMode,
    sanitize_timeout_ms: u64,
    sanitizer_exe_override: Option<PathBuf>,
    telemetry: MaybeTelemetry,
) {
    let allowed_mime_types = allowed_mime_types
        .into_iter()
        .filter_map(|entry| normalize_mime(&entry))
        .collect();
    *attach_cfg().write() = AttachConfig {
        ttl_secs,
        max_bytes,
        per_tenant_max_count,
        per_tenant_max_bytes,
        global_max_count,
        global_max_bytes,
        allowed_mime_types,
        max_expanded_bytes,
        max_archive_depth,
        sanitizer_mode,
        sanitize_timeout_ms,
        sanitizer_exe_override,
        telemetry,
    };
}
pub(super) fn max_bytes_cfg() -> usize {
    let max_bytes = max_bytes_u64_cfg();
    usize::try_from(max_bytes).unwrap_or(usize::MAX)
}
fn max_bytes_u64_cfg() -> u64 {
    attach_cfg().read().max_bytes
}
fn ttl_secs_cfg() -> u64 {
    attach_cfg().read().ttl_secs
}
fn per_tenant_max_count_cfg() -> u64 {
    attach_cfg().read().per_tenant_max_count
}
fn per_tenant_max_bytes_cfg() -> u64 {
    attach_cfg().read().per_tenant_max_bytes
}
fn global_max_bytes_cfg() -> u64 {
    attach_cfg().read().global_max_bytes
}
fn quota_limits_cfg() -> AttachmentQuotaLimits {
    let config = attach_cfg().read();
    AttachmentQuotaLimits {
        per_tenant_max_count: config.per_tenant_max_count,
        per_tenant_max_bytes: config.per_tenant_max_bytes,
        global_max_count: config.global_max_count,
        global_max_bytes: config.global_max_bytes,
    }
}
fn allowed_mime_types_cfg() -> Vec<String> {
    attach_cfg().read().allowed_mime_types.clone()
}
fn max_expanded_bytes_cfg() -> u64 {
    attach_cfg().read().max_expanded_bytes
}
fn max_archive_depth_cfg() -> u32 {
    attach_cfg().read().max_archive_depth
}
fn sanitizer_mode_cfg() -> AttachmentSanitizerMode {
    attach_cfg().read().sanitizer_mode
}
fn sanitize_timeout_cfg() -> Duration {
    let ms = attach_cfg().read().sanitize_timeout_ms.max(1);
    Duration::from_millis(ms)
}
/// Run the attachment sanitizer process if requested via environment.
pub fn sanitizer_process_exit_code_from_env() -> Option<i32> {
    if env::var_os(ATTACHMENT_SANITIZER_ENV).as_deref() != Some(OsStr::new("1")) {
        return None;
    }
    let exit_code = match run_sanitizer_process() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("attachment sanitizer failed: {}", err.message);
            1
        }
    };
    Some(exit_code)
}
fn run_sanitizer_process() -> Result<(), SanitizeError> {
    if env::var_os(ATTACHMENT_SANITIZER_SANDBOXED_ENV).as_deref() != Some(OsStr::new("1")) {
        return Err(SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            "attachment sanitizer refuses to run outside its OS sandbox",
        ));
    }
    let max_input = env::var(ATTACHMENT_SANITIZER_MAX_INPUT_ENV)
        .map_err(|_| {
            SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                "attachment sanitizer max input limit is missing",
            )
        })?
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                "attachment sanitizer max input limit is invalid",
            )
        })?;
    let payload = read_stdin_limited(max_input)?;
    let request = match decode_sanitizer_request_bytes(&payload) {
        Ok(request) => request,
        Err(err) => {
            let response = SanitizerResponse::Rejected {
                error: err.into_wire(),
            };
            return write_sanitizer_response(&response);
        }
    };
    let cfg = SanitizerConfig {
        allowed_mime_types: request
            .allowed_mime_types
            .into_iter()
            .filter_map(|entry| normalize_mime(&entry))
            .collect(),
        max_expanded_bytes: request.max_expanded_bytes,
        max_archive_depth: request.max_archive_depth,
        timeout: Duration::from_millis(request.timeout_ms.max(1)),
        mode: AttachmentSanitizerMode::InProcess,
    };
    apply_sanitizer_limits(cfg.max_expanded_bytes, cfg.timeout)?;
    let response =
        match sanitize_attachment_sync(request.declared_type.as_deref(), &request.body, &cfg) {
            Ok(mut outcome) => {
                outcome.summary.sandboxed = true;
                SanitizerResponse::Accepted {
                    summary: outcome.summary,
                    sanitized_body: outcome.sanitized_body,
                }
            }
            Err(err) => SanitizerResponse::Rejected {
                error: err.into_wire(),
            },
        };
    write_sanitizer_response(&response)
}
fn decode_sanitizer_request_bytes(payload: &[u8]) -> Result<SanitizerRequest, SanitizeError> {
    norito::decode_canonical(payload).map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer request decode failed: {err}"),
        )
    })
}
fn sanitizer_cpu_limit_secs(timeout: Duration) -> u64 {
    let millis = timeout.as_millis().max(1) as u64;
    (millis.saturating_add(999) / 1000).max(1)
}
fn sanitizer_memory_limit_bytes(max_expanded_bytes: u64) -> u64 {
    const BASE_OVERHEAD_BYTES: u64 = 64 * 1024 * 1024;
    let scaled = max_expanded_bytes.saturating_mul(4);
    scaled
        .saturating_add(BASE_OVERHEAD_BYTES)
        .max(BASE_OVERHEAD_BYTES)
}
fn apply_sanitizer_limits(max_expanded_bytes: u64, timeout: Duration) -> Result<(), SanitizeError> {
    #[cfg(unix)]
    {
        let cpu_limit = sanitizer_cpu_limit_secs(timeout);
        let mem_limit = sanitizer_memory_limit_bytes(max_expanded_bytes);
        set_rlimit(libc::RLIMIT_CPU, cpu_limit)?;
        set_rlimit(libc::RLIMIT_AS, mem_limit)?;
    }
    Ok(())
}
#[cfg(unix)]
#[cfg(any(target_env = "gnu", target_env = "uclibc"))]
type RlimitResource = libc::__rlimit_resource_t;
#[cfg(unix)]
#[cfg(not(any(target_env = "gnu", target_env = "uclibc")))]
type RlimitResource = libc::c_int;
#[cfg(unix)]
#[allow(unsafe_code)]
fn set_rlimit(resource: RlimitResource, value: u64) -> Result<(), SanitizeError> {
    let limit = libc::rlimit {
        rlim_cur: value,
        rlim_max: value,
    };
    let result = unsafe { libc::setrlimit(resource, &raw const limit) };
    if result != 0 {
        return Err(SanitizeError {
            reason: SanitizeRejectReason::Sandbox,
            message: format!(
                "setrlimit failed for resource {:?}: {}",
                resource,
                std::io::Error::last_os_error()
            ),
        });
    }
    Ok(())
}
fn write_sanitizer_response(response: &SanitizerResponse) -> Result<(), SanitizeError> {
    let bytes = norito::encode_canonical(response).map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer response encode failed: {err}"),
        )
    })?;
    let mut stdout = std::io::stdout();
    stdout.write_all(&bytes).map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer response write failed: {err}"),
        )
    })?;
    Ok(())
}
fn read_stdin_limited(max_bytes: usize) -> Result<Vec<u8>, SanitizeError> {
    let mut reader = std::io::stdin().lock();
    let mut buf = [0u8; 8 * 1024];
    let mut out = Vec::new();
    loop {
        let read = reader.read(&mut buf).map_err(|err| {
            SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                format!("attachment sanitizer stdin read failed: {err}"),
            )
        })?;
        if read == 0 {
            break;
        }
        let next_len = out.len().saturating_add(read);
        if next_len > max_bytes {
            return Err(SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                "attachment sanitizer request exceeds max bytes",
            ));
        }
        out.extend_from_slice(&buf[..read]);
    }
    Ok(out)
}
fn telemetry_handle() -> MaybeTelemetry {
    attach_cfg().read().telemetry.clone()
}
fn quota_lock() -> &'static Mutex<()> {
    ATTACH_MUTEX.get_or_init(|| Mutex::new(()))
}
#[cfg(test)]
mod tests {
    use super::{
        AttachmentHashes, AttachmentMeta, AttachmentProvenance, AttachmentSanitizerMode,
        AttachmentSanitizerVerdict, SanitizeRejectReason, SanitizerConfig, ZK1_MAX_TLV_COUNT, json,
        parse_zk1_tags, sanitize_attachment_id, sanitize_attachment_sync,
    };
    use axum::http::HeaderMap;
    use axum::{http::StatusCode, response::IntoResponse};
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use flate2::{Compression, write::GzEncoder};
    use http_body_util::BodyExt as _;
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::account::AccountId;
    use sha2::{Digest as _, Sha256};
    use std::{collections::BTreeSet, ffi::OsStr, fs, path::PathBuf, process::Command};
    use std::{
        io,
        io::Write as _,
        sync::{Arc, Once},
        time::{Duration, Instant},
    };
    #[test]
    fn attachment_config_lock_remains_usable_after_writer_panic() {
        let panic = std::thread::spawn(|| {
            let _guard = super::attach_cfg().write();
            panic!("intentional attachment-config writer panic");
        })
        .join();
        assert!(panic.is_err());
        let _guard = super::attach_cfg().read();
    }
    #[test]
    fn quota_scan_entry_bound_counts_every_raw_entry() {
        let mut scanned = 0_u64;
        assert!(super::quota_scan_entry_within_limit(&mut scanned, 2));
        assert!(super::quota_scan_entry_within_limit(&mut scanned, 2));
        assert!(!super::quota_scan_entry_within_limit(&mut scanned, 2));
        assert_eq!(scanned, 3);
    }
    #[test]
    fn attachment_sanitizer_stdout_reader_retains_admission_until_eof() {
        struct BlockingReader {
            entered: std::sync::mpsc::Sender<()>,
            release: std::sync::mpsc::Receiver<()>,
        }
        impl std::io::Read for BlockingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                self.entered
                    .send(())
                    .expect("signal blocked sanitizer stdout read");
                self.release
                    .recv()
                    .expect("release blocked sanitizer stdout read");
                Ok(0)
            }
        }

        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = semaphore
            .clone()
            .try_acquire_owned()
            .expect("acquire sanitizer admission");
        let admission = crate::ProofBodyAdmissionLease::new(permit);
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let result = super::spawn_sanitizer_stdout_reader(
            BlockingReader {
                entered: entered_tx,
                release: release_rx,
            },
            16,
            Some(admission),
        );
        entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("sanitizer stdout reader must block");
        assert_eq!(
            semaphore.available_permits(),
            0,
            "a detached stdout reader must retain physical admission"
        );
        release_tx
            .send(())
            .expect("release sanitizer stdout reader");
        assert_eq!(
            result
                .recv_timeout(Duration::from_secs(1))
                .expect("sanitizer stdout result")
                .expect("sanitizer stdout read"),
            Vec::<u8>::new()
        );
        assert_eq!(semaphore.available_permits(), 1);
    }
    #[test]
    fn quota_child_entry_budget_is_aggregate_across_tenants() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        let first = super::AttachmentTenant("a".repeat(super::TENANT_KEY_HEX_LEN));
        let second = super::AttachmentTenant("b".repeat(super::TENANT_KEY_HEX_LEN));
        super::ensure_dirs(&first);
        super::ensure_dirs(&second);
        for (tenant, prefix) in [(&first, "first"), (&second, "second")] {
            for index in 0..2 {
                fs::write(
                    super::attachments_dir(tenant).join(format!("{prefix}-{index}.noise")),
                    b"noise",
                )
                .expect("write raw tenant entry");
            }
        }

        let mut scan = super::AttachmentQuotaScanBudget::new(3);
        assert!(
            super::quota_metas_for_tenant(&first, &mut scan)
                .expect("first tenant fits aggregate child-entry budget")
                .is_empty()
        );
        let error = super::quota_metas_for_tenant(&second, &mut scan)
            .expect_err("second tenant must share the first tenant's scan budget");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(scan.child_entries, 4);
    }
    fn test_sanitizer_config(max_expanded_bytes: u64, max_archive_depth: u32) -> SanitizerConfig {
        SanitizerConfig {
            allowed_mime_types: vec![
                super::NORITO_MIME_TYPE.to_string(),
                super::JSON_MIME_TYPE.to_string(),
                super::ZK1_MIME_TYPE.to_string(),
            ],
            max_expanded_bytes,
            max_archive_depth,
            timeout: std::time::Duration::from_millis(100),
            mode: AttachmentSanitizerMode::InProcess,
        }
    }
    fn gzip_compress(input: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(input).expect("write gzip input");
        encoder.finish().expect("finish gzip")
    }
    fn canonical_test_meta(tenant: &super::AttachmentTenant, body: &[u8]) -> AttachmentMeta {
        let id = hex::encode::<[u8; 32]>(Hash::new(body).into());
        AttachmentMeta {
            id: id.clone(),
            content_type: super::JSON_MIME_TYPE.to_owned(),
            size: body.len() as u64,
            created_ms: 1_700_000_000_000,
            tenant: Some(tenant.as_str().to_owned()),
            provenance: Some(AttachmentProvenance {
                declared_type: Some(super::JSON_MIME_TYPE.to_owned()),
                sniffed_type: super::JSON_MIME_TYPE.to_owned(),
                hashes: AttachmentHashes {
                    blake2b_256: id,
                    sha256: hex::encode(Sha256::digest(body)),
                },
                sanitizer: AttachmentSanitizerVerdict {
                    verdict: "accepted".to_owned(),
                    expanded_bytes: body.len() as u64,
                    archive_depth: 0,
                    sandboxed: false,
                },
            }),
            zk1_tags: None,
        }
    }
    fn quota_transaction(
        tenant: &super::AttachmentTenant,
        incoming_meta: AttachmentMeta,
        previous_meta: Option<AttachmentMeta>,
        victim_ids: Vec<String>,
    ) -> super::AttachmentQuotaTransaction {
        super::AttachmentQuotaTransaction {
            version: super::ATTACHMENT_QUOTA_TRANSACTION_VERSION,
            tenant: tenant.as_str().to_owned(),
            incoming_meta,
            previous_meta,
            victim_ids,
        }
    }
    fn persist_canonical_test_attachment(
        tenant: &super::AttachmentTenant,
        body: &[u8],
        created_ms: u64,
    ) -> AttachmentMeta {
        let mut meta = canonical_test_meta(tenant, body);
        meta.created_ms = created_ms;
        super::persist_body(tenant, &meta.id, body).expect("persist canonical attachment body");
        super::save_meta(tenant, &meta).expect("persist canonical attachment metadata");
        meta
    }
    fn padded_json_body(tenant: &str, index: usize, target_len: usize) -> Vec<u8> {
        let prefix = format!(r#"{{"tenant":"{tenant}","index":{index},"padding":""#);
        let suffix = r#""}"#;
        assert!(prefix.len() + suffix.len() <= target_len);
        let mut body = prefix;
        body.push_str(&"x".repeat(target_len - body.len() - suffix.len()));
        body.push_str(suffix);
        assert_eq!(body.len(), target_len);
        body.into_bytes()
    }
    fn load_fixture_base64(name: &str) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("attachments")
            .join(name);
        let encoded = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()));
        let mut joined = String::new();
        for line in encoded.lines() {
            joined.push_str(line.trim());
        }
        BASE64_STANDARD
            .decode(joined.as_bytes())
            .unwrap_or_else(|err| panic!("failed to decode fixture {}: {err}", path.display()))
    }
    fn ensure_test_config() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            super::configure(
                60,
                1024,
                10,
                4096,
                20,
                8192,
                vec![
                    super::NORITO_MIME_TYPE.to_string(),
                    super::JSON_MIME_TYPE.to_string(),
                    super::ZK1_MIME_TYPE.to_string(),
                ],
                4096,
                1,
                AttachmentSanitizerMode::InProcess,
                500,
                None,
                crate::routing::MaybeTelemetry::disabled(),
            );
        });
    }
    fn checked_attachment_ed25519_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("test attachment fixture key derivation should succeed")
    }
    fn checked_attachment_account(seed: u8) -> AccountId {
        AccountId::new(
            checked_attachment_ed25519_keypair(seed)
                .public_key()
                .clone(),
        )
    }
    #[test]
    fn checked_attachment_ed25519_keypair_uses_fallible_seed_derivation() {
        assert_eq!(
            checked_attachment_ed25519_keypair(0x40).algorithm(),
            Algorithm::Ed25519
        );
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
        assert_ne!(
            checked_attachment_account(0x41),
            checked_attachment_account(0x42)
        );
    }
    #[test]
    fn attachment_meta_norito_roundtrip() {
        let meta = AttachmentMeta {
            id: "deadbeef".repeat(4),
            content_type: "application/json".to_string(),
            size: 512,
            created_ms: 1_700_000_000_000,
            tenant: Some("a".repeat(64)),
            provenance: None,
            zk1_tags: None,
        };
        let encoded = json::to_json_pretty(&meta).expect("serialize metadata");
        let decoded: AttachmentMeta = json::from_json(&encoded).expect("deserialize metadata");
        assert_eq!(meta, decoded);
    }
    #[test]
    fn load_meta_rejects_oversized_persisted_metadata() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        let tenant = super::AttachmentTenant::anonymous();
        super::ensure_dirs(&tenant);
        let id = "a".repeat(super::ATTACHMENT_ID_HEX_LEN);
        fs::write(
            super::meta_path(&tenant, &id),
            vec![b' '; super::ATTACHMENT_META_FILE_MAX_BYTES as usize + 1],
        )
        .expect("write oversized persisted metadata");
        assert!(
            super::load_meta(&tenant, &id).is_none(),
            "metadata beyond the 64-KiB persistence contract must not be read or parsed"
        );
    }
    #[test]
    fn save_meta_rejects_records_beyond_the_persistence_contract() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        let tenant = super::AttachmentTenant::anonymous();
        let id = "c".repeat(super::ATTACHMENT_ID_HEX_LEN);
        let meta = AttachmentMeta {
            id: id.clone(),
            content_type: "x".repeat(super::ATTACHMENT_META_FILE_MAX_BYTES as usize),
            size: 0,
            created_ms: 0,
            tenant: Some(tenant.as_str().to_owned()),
            provenance: None,
            zk1_tags: None,
        };
        let error =
            super::save_meta(&tenant, &meta).expect_err("oversized metadata must not be persisted");
        assert!(error.to_string().contains("persistence limit"));
        assert!(!super::meta_path(&tenant, &id).exists());
    }
    #[test]
    fn save_and_load_meta_require_canonical_provenance() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        let tenant = super::AttachmentTenant::anonymous();
        let body = br#"{"valid":true}"#;
        let meta = canonical_test_meta(&tenant, body);
        super::persist_body(&tenant, &meta.id, body).expect("persist canonical body");
        super::save_meta(&tenant, &meta).expect("persist canonical metadata");
        assert_eq!(
            super::load_meta(&tenant, &meta.id),
            Some(meta.clone()),
            "canonical metadata must round-trip"
        );
        let mut missing = meta.clone();
        missing.provenance = None;
        assert!(
            super::save_meta(&tenant, &missing)
                .expect_err("missing provenance must reject")
                .to_string()
                .contains("provenance is required")
        );
        let mut rejected = meta.clone();
        rejected
            .provenance
            .as_mut()
            .expect("canonical provenance")
            .sanitizer
            .verdict = "rejected".to_owned();
        assert!(
            super::save_meta(&tenant, &rejected)
                .expect_err("non-accepted sanitizer verdict must reject")
                .to_string()
                .contains("verdict")
        );
        let mut wrong_expanded_size = meta;
        wrong_expanded_size
            .provenance
            .as_mut()
            .expect("canonical provenance")
            .sanitizer
            .expanded_bytes += 1;
        assert!(
            super::save_meta(&tenant, &wrong_expanded_size)
                .expect_err("expanded-size mismatch must reject")
                .to_string()
                .contains("expanded size")
        );
    }
    #[cfg(unix)]
    #[test]
    fn load_meta_rejects_a_symlink_entry() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        let tenant = super::AttachmentTenant::anonymous();
        super::ensure_dirs(&tenant);
        let id = "b".repeat(super::ATTACHMENT_ID_HEX_LEN);
        let target = tmp.path().join("outside-metadata.json");
        fs::write(&target, b"{}").expect("write symlink target");
        symlink(&target, super::meta_path(&tenant, &id)).expect("create metadata symlink");
        assert!(
            super::load_meta(&tenant, &id).is_none(),
            "metadata readers must not follow attachment-store symlinks"
        );
    }
    #[test]
    fn attachment_tenant_is_derived_from_signed_account() {
        let alice = checked_attachment_account(0x43);
        let bob = checked_attachment_account(0x44);
        assert_eq!(
            super::AttachmentTenant::from_account(&alice),
            super::AttachmentTenant::from_account(&alice)
        );
        assert_ne!(
            super::AttachmentTenant::from_account(&alice),
            super::AttachmentTenant::from_account(&bob)
        );
    }
    #[test]
    fn prover_processing_receipt_lives_until_the_last_attachment_reference_is_deleted() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        let first = super::AttachmentTenant("1".repeat(super::TENANT_KEY_HEX_LEN));
        let second = super::AttachmentTenant("2".repeat(super::TENANT_KEY_HEX_LEN));
        let id = "a".repeat(super::ATTACHMENT_ID_HEX_LEN);
        for tenant in [&first, &second] {
            super::ensure_dirs(tenant);
            fs::write(super::meta_path(tenant, &id), b"metadata").expect("write metadata marker");
            fs::write(super::bin_path(tenant, &id), b"body").expect("write body marker");
            super::ensure_prover_processing_reference(tenant.as_str(), &id)
                .expect("register live attachment reference");
        }
        let receipt = super::ProverProcessingReceipt {
            version: super::ZK_PROVER_PROCESSING_STATE_VERSION,
            id: id.clone(),
            processed_ms: 1,
            terminal: true,
            retry_not_before_ms: None,
            retry_count: 0,
            completed_proof_indices: Vec::new(),
            processing_context_hash: None,
        };
        assert!(
            super::persist_prover_processing_receipt_if_referenced(&receipt)
                .expect("persist terminal receipt")
        );
        assert_eq!(
            super::prover_processing_decision(&id, 1),
            super::ProverProcessingDecision::Suppress
        );
        super::delete_attachment_files(&first, &id).expect("delete first attachment copy");
        assert_eq!(
            super::prover_processing_decision(&id, 1),
            super::ProverProcessingDecision::Suppress,
            "one live duplicate must retain the global receipt"
        );
        super::delete_attachment_files(&second, &id).expect("delete final attachment copy");
        assert_eq!(
            super::prover_processing_decision(&id, 1),
            super::ProverProcessingDecision::Missing,
            "the last attachment deletion must reclaim the receipt"
        );
        assert!(
            super::ensure_prover_processing_reference(second.as_str(), &id).is_err(),
            "a stale discovery entry must not recreate a reference after deletion"
        );
    }
    #[test]
    fn attachment_deletion_preserves_files_when_reference_unlink_fails() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        let tenant = super::AttachmentTenant("4".repeat(super::TENANT_KEY_HEX_LEN));
        let id = "c".repeat(super::ATTACHMENT_ID_HEX_LEN);
        super::ensure_dirs(&tenant);
        let meta_path = super::meta_path(&tenant, &id);
        let body_path = super::bin_path(&tenant, &id);
        fs::write(&meta_path, b"metadata").expect("write metadata marker");
        fs::write(&body_path, b"body").expect("write body marker");
        super::ensure_prover_processing_reference(tenant.as_str(), &id)
            .expect("register live attachment reference");
        let receipt = super::ProverProcessingReceipt {
            version: super::ZK_PROVER_PROCESSING_STATE_VERSION,
            id: id.clone(),
            processed_ms: 1,
            terminal: true,
            retry_not_before_ms: None,
            retry_count: 0,
            completed_proof_indices: Vec::new(),
            processing_context_hash: None,
        };
        assert!(
            super::persist_prover_processing_receipt_if_referenced(&receipt)
                .expect("persist terminal receipt")
        );
        let reference_path = super::prover_processing_reference_path(tenant.as_str(), &id);
        fs::remove_file(&reference_path).expect("remove reference fixture");
        fs::create_dir(&reference_path).expect("replace reference with an undeletable directory");

        super::delete_attachment_files(&tenant, &id)
            .expect_err("reference transition failure must abort attachment deletion");
        assert!(meta_path.is_file());
        assert!(body_path.is_file());
        assert_eq!(
            super::prover_processing_decision(&id, 1),
            super::ProverProcessingDecision::Suppress,
            "the receipt must survive an incomplete reference transition"
        );
    }
    #[test]
    fn attachment_deletion_preserves_files_when_reference_enumeration_fails() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        let tenant = super::AttachmentTenant("5".repeat(super::TENANT_KEY_HEX_LEN));
        let id = "d".repeat(super::ATTACHMENT_ID_HEX_LEN);
        super::ensure_dirs(&tenant);
        let meta_path = super::meta_path(&tenant, &id);
        let body_path = super::bin_path(&tenant, &id);
        fs::write(&meta_path, b"metadata").expect("write metadata marker");
        fs::write(&body_path, b"body").expect("write body marker");
        super::ensure_prover_processing_reference(tenant.as_str(), &id)
            .expect("register live attachment reference");
        let receipt = super::ProverProcessingReceipt {
            version: super::ZK_PROVER_PROCESSING_STATE_VERSION,
            id: id.clone(),
            processed_ms: 1,
            terminal: true,
            retry_not_before_ms: None,
            retry_count: 0,
            completed_proof_indices: Vec::new(),
            processing_context_hash: None,
        };
        assert!(
            super::persist_prover_processing_receipt_if_referenced(&receipt)
                .expect("persist terminal receipt")
        );
        let reference_dir = super::prover_processing_reference_dir(&id);
        fs::remove_dir_all(&reference_dir).expect("remove reference-directory fixture");
        fs::write(&reference_dir, b"not a directory")
            .expect("replace reference directory with a file");
        assert!(
            super::processing_reference_dir_has_shards(&id).is_err(),
            "reference enumeration errors must remain distinguishable from an empty directory"
        );

        super::delete_attachment_files(&tenant, &id)
            .expect_err("reference enumeration failure must abort attachment deletion");
        assert!(meta_path.is_file());
        assert!(body_path.is_file());
        assert_eq!(
            super::prover_processing_decision(&id, 1),
            super::ProverProcessingDecision::Suppress,
            "the receipt must survive an unreadable reference directory"
        );
    }
    #[test]
    fn attachment_deletion_cleans_crash_left_processing_temp_files() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        let tenant = super::AttachmentTenant("6".repeat(super::TENANT_KEY_HEX_LEN));
        let id = "e".repeat(super::ATTACHMENT_ID_HEX_LEN);
        super::ensure_dirs(&tenant);
        let meta_path = super::meta_path(&tenant, &id);
        let body_path = super::bin_path(&tenant, &id);
        fs::write(&meta_path, b"metadata").expect("write metadata marker");
        fs::write(&body_path, b"body").expect("write body marker");
        super::ensure_prover_processing_reference(tenant.as_str(), &id)
            .expect("register live attachment reference");
        let receipt = super::ProverProcessingReceipt {
            version: super::ZK_PROVER_PROCESSING_STATE_VERSION,
            id: id.clone(),
            processed_ms: 1,
            terminal: true,
            retry_not_before_ms: None,
            retry_count: 0,
            completed_proof_indices: Vec::new(),
            processing_context_hash: None,
        };
        assert!(
            super::persist_prover_processing_receipt_if_referenced(&receipt)
                .expect("persist terminal receipt")
        );
        let state_entry_dir = super::prover_processing_state_dir().join(&id);
        let reference_dir = super::prover_processing_reference_dir(&id);
        fs::write(
            reference_dir.join(format!(
                "{}crashed-reference",
                super::ZK_PROVER_PROCESSING_TEMP_PREFIX
            )),
            b"partial reference",
        )
        .expect("write crash-left reference temporary file");
        fs::write(
            state_entry_dir.join(format!(
                "{}crashed-receipt",
                super::ZK_PROVER_PROCESSING_TEMP_PREFIX
            )),
            b"partial receipt",
        )
        .expect("write crash-left receipt temporary file");

        super::delete_attachment_files(&tenant, &id)
            .expect("crash-left processing temporary files must not wedge deletion");
        assert!(!meta_path.exists());
        assert!(!body_path.exists());
        assert!(!state_entry_dir.exists());
        assert_eq!(
            super::prover_processing_decision(&id, 1),
            super::ProverProcessingDecision::Missing
        );
    }
    #[test]
    fn prover_processing_retry_receipt_suppresses_until_its_deadline() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        let tenant = super::AttachmentTenant("3".repeat(super::TENANT_KEY_HEX_LEN));
        let id = "b".repeat(super::ATTACHMENT_ID_HEX_LEN);
        super::ensure_dirs(&tenant);
        fs::write(super::meta_path(&tenant, &id), b"metadata").expect("write metadata marker");
        fs::write(super::bin_path(&tenant, &id), b"body").expect("write body marker");
        super::ensure_prover_processing_reference(tenant.as_str(), &id)
            .expect("register live attachment reference");
        let receipt = super::ProverProcessingReceipt {
            version: super::ZK_PROVER_PROCESSING_STATE_VERSION,
            id: id.clone(),
            processed_ms: 10,
            terminal: false,
            retry_not_before_ms: Some(1_000),
            retry_count: 3,
            completed_proof_indices: Vec::new(),
            processing_context_hash: None,
        };
        assert!(
            super::persist_prover_processing_receipt_if_referenced(&receipt)
                .expect("persist retry receipt")
        );
        assert_eq!(
            super::prover_processing_decision(&id, 999),
            super::ProverProcessingDecision::Suppress
        );
        assert_eq!(
            super::prover_processing_decision(&id, 1_000),
            super::ProverProcessingDecision::Due { retry_count: 3 }
        );
    }
    #[test]
    fn sanitize_attachment_id_rejects_bad_inputs() {
        assert!(sanitize_attachment_id("../etc/passwd").is_none());
        assert!(sanitize_attachment_id("not-hex").is_none());
        assert!(sanitize_attachment_id(&"g".repeat(super::ATTACHMENT_ID_HEX_LEN)).is_none());
        let upper = "A".repeat(super::ATTACHMENT_ID_HEX_LEN);
        assert_eq!(
            sanitize_attachment_id(&upper),
            Some("a".repeat(super::ATTACHMENT_ID_HEX_LEN))
        );
    }
    #[tokio::test]
    async fn get_attachment_rejects_invalid_id() {
        let response = super::handle_get_attachment(
            super::AttachmentTenant::anonymous(),
            axum::extract::Path("../bad".to_string()),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    #[tokio::test]
    async fn get_attachment_rejects_same_size_body_substitution() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        ensure_test_config();
        let tenant = super::AttachmentTenant::anonymous();
        let body = br#"{"valid":true}"#;
        let substituted = br#"{"valid":null}"#;
        assert_eq!(body.len(), substituted.len());
        let meta = canonical_test_meta(&tenant, body);
        super::persist_body(&tenant, &meta.id, body).expect("persist canonical body");
        super::save_meta(&tenant, &meta).expect("persist canonical metadata");
        fs::write(super::bin_path(&tenant, &meta.id), substituted)
            .expect("substitute same-size body");
        let response = super::handle_get_attachment(tenant, axum::extract::Path(meta.id))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    #[test]
    fn sanitizer_accepts_norito_magic() {
        let cfg = test_sanitizer_config(1024, 1);
        let body = b"NRT0test";
        let outcome = sanitize_attachment_sync(None, body, &cfg).expect("sanitized");
        assert_eq!(outcome.summary.sniffed_type, super::NORITO_MIME_TYPE);
        assert_eq!(outcome.summary.expanded_bytes, body.len() as u64);
    }
    #[test]
    fn sanitizer_rejects_declared_mismatch() {
        let cfg = test_sanitizer_config(1024, 1);
        let body = b"NRT0test";
        let err = sanitize_attachment_sync(Some(super::JSON_MIME_TYPE), body, &cfg)
            .expect_err("mismatch rejected");
        assert_eq!(err.reason, SanitizeRejectReason::Type);
    }
    #[test]
    fn sanitizer_accepts_plus_json_declared_type() {
        let cfg = test_sanitizer_config(1024, 1);
        let body = br#"{"hello":"world"}"#;
        let outcome = sanitize_attachment_sync(Some("application/ld+json"), body, &cfg)
            .expect("plus-json should be accepted");
        assert_eq!(outcome.summary.sniffed_type, super::JSON_MIME_TYPE);
    }
    #[test]
    fn sanitizer_rejects_expansion_limit() {
        let cfg = test_sanitizer_config(8, 2);
        let body = b"{\"hello\":\"world\"}";
        let gz = gzip_compress(body);
        let err = sanitize_attachment_sync(None, &gz, &cfg).expect_err("expansion rejected");
        assert_eq!(err.reason, SanitizeRejectReason::Expansion);
    }
    #[test]
    fn sanitizer_rejects_archive_depth() {
        let cfg = test_sanitizer_config(1024, 1);
        let body = b"{\"hello\":\"world\"}";
        let once = gzip_compress(body);
        let twice = gzip_compress(&once);
        let err = sanitize_attachment_sync(None, &twice, &cfg).expect_err("depth rejected");
        assert_eq!(err.reason, SanitizeRejectReason::Expansion);
    }
    #[test]
    fn sanitizer_limit_helpers_round_up() {
        assert_eq!(
            super::sanitizer_cpu_limit_secs(std::time::Duration::from_millis(1)),
            1
        );
        assert_eq!(
            super::sanitizer_cpu_limit_secs(std::time::Duration::from_millis(1001)),
            2
        );
        let min_limit = super::sanitizer_memory_limit_bytes(0);
        assert!(min_limit >= 64 * 1024 * 1024);
        let scaled_limit = super::sanitizer_memory_limit_bytes(16 * 1024 * 1024);
        assert!(scaled_limit > min_limit);
    }
    #[test]
    fn sanitizer_executable_override_prefers_explicit_path() {
        let override_path = PathBuf::from("attachment_sanitizer_stub");
        let resolved = super::sanitizer_executable_with_override(Some(override_path.clone()))
            .expect("override path");
        assert_eq!(resolved, override_path);
    }
    #[test]
    fn sanitizer_executable_defaults_to_dedicated_sibling() {
        let resolved = super::sanitizer_executable_with_override(None).expect("sanitizer path");
        let current = std::env::current_exe().expect("current exe");
        assert_ne!(resolved, current);
        assert_eq!(
            resolved,
            current.parent().expect("binary directory").join(format!(
                "{}{}",
                super::ATTACHMENT_SANITIZER_BINARY_STEM,
                std::env::consts::EXE_SUFFIX
            ))
        );
    }
    #[test]
    fn validate_sanitizer_executable_rejects_missing_non_file_or_node_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let missing = temp.path().join("missing-sanitizer");
        let err =
            super::validate_sanitizer_executable(&missing).expect_err("missing path rejected");
        assert_eq!(err.reason, SanitizeRejectReason::Sandbox);
        assert!(err.message.contains("attachment sanitizer spawn failed"));
        let err =
            super::validate_sanitizer_executable(temp.path()).expect_err("directory path rejected");
        assert_eq!(err.reason, SanitizeRejectReason::Sandbox);
        assert!(err.message.contains("is not a file"));
        let current = std::env::current_exe().expect("current executable");
        let err = super::validate_sanitizer_executable(&current)
            .expect_err("node executable must be rejected");
        assert_eq!(err.reason, SanitizeRejectReason::Sandbox);
        assert!(err.message.contains("dedicated executable"));
    }
    #[test]
    fn sanitizer_command_environment_is_an_explicit_allowlist() {
        let mut cmd = Command::new("attachment_sanitizer");
        cmd.env("PRIVATE_KEY", "must-not-survive");
        super::set_clean_sanitizer_environment(&mut cmd, "4096");
        let envs: Vec<_> = cmd.get_envs().collect();
        assert_eq!(envs.len(), 3);
        assert!(
            envs.iter()
                .all(|(key, _)| *key != OsStr::new("PRIVATE_KEY"))
        );
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new(super::ATTACHMENT_SANITIZER_ENV) && *value == Some(OsStr::new("1"))
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new(super::ATTACHMENT_SANITIZER_MAX_INPUT_ENV)
                && *value == Some(OsStr::new("4096"))
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new(super::ATTACHMENT_SANITIZER_SANDBOXED_ENV)
                && *value == Some(OsStr::new("1"))
        }));
    }
    #[test]
    fn sandboxed_sanitizer_command_fails_closed_without_wrapper_in_search_path() {
        let exe = PathBuf::from("attachment_sanitizer");
        let err = super::sandboxed_sanitizer_command_for_search_path(
            &exe,
            "4096",
            Some(OsStr::new("/definitely/missing")),
        )
        .expect_err("missing sandbox wrapper must reject");
        assert_eq!(err.reason, SanitizeRejectReason::Sandbox);
        assert!(err.message.contains("OS sandbox unavailable"));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn sandboxed_sanitizer_command_uses_sandbox_exec_on_macos() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_exec = temp.path().join("sandbox-exec");
        fs::write(&sandbox_exec, "").expect("write fake sandbox-exec");
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mut permissions = fs::metadata(&sandbox_exec)
                .expect("sandbox-exec metadata")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&sandbox_exec, permissions).expect("make sandbox-exec executable");
        }
        let exe = PathBuf::from("attachment_sanitizer");
        let cmd = super::sandboxed_sanitizer_command_for_search_path(
            &exe,
            "4096",
            Some(temp.path().as_os_str()),
        )
        .expect("sandbox command");
        assert_eq!(cmd.get_program(), sandbox_exec.as_os_str());
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args.first().copied(), Some(OsStr::new("-p")));
        assert!(
            args.get(1)
                .and_then(|arg| arg.to_str())
                .is_some_and(|profile| profile.contains("(deny network*)")
                    && profile.contains("(allow sysctl-read)")
                    && !profile.contains("(allow file-read*)"))
        );
        assert_eq!(args.last().copied(), Some(exe.as_os_str()));
        let envs: Vec<_> = cmd.get_envs().collect();
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new(super::ATTACHMENT_SANITIZER_ENV) && *value == Some(OsStr::new("1"))
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new(super::ATTACHMENT_SANITIZER_SANDBOXED_ENV)
                && *value == Some(OsStr::new("1"))
        }));
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn sandboxed_sanitizer_command_uses_bwrap_from_search_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bubblewrap = temp.path().join("bwrap");
        fs::write(&bubblewrap, "").expect("write fake bwrap");
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mut permissions = fs::metadata(&bubblewrap)
                .expect("bwrap metadata")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&bubblewrap, permissions).expect("make bwrap executable");
        }
        let exe = PathBuf::from("attachment_sanitizer");
        let cmd = super::sandboxed_sanitizer_command_for_search_path(
            &exe,
            "4096",
            Some(temp.path().as_os_str()),
        )
        .expect("sandbox command");
        assert_eq!(cmd.get_program(), bubblewrap.as_os_str());
        let args: Vec<_> = cmd.get_args().collect();
        assert!(
            args.iter()
                .any(|arg| *arg == OsStr::new("--die-with-parent"))
        );
        assert!(args.iter().any(|arg| *arg == OsStr::new("--clearenv")));
        assert!(args.iter().any(|arg| *arg == OsStr::new("--setenv")));
        assert_eq!(
            args.last().copied(),
            Some(OsStr::new("/attachment_sanitizer"))
        );
        assert!(
            !args.windows(3).any(|window| {
                window == [OsStr::new("--ro-bind"), OsStr::new("/"), OsStr::new("/")]
            }),
            "the sandbox must not expose the host root"
        );
    }
    #[test]
    fn sanitizer_stdout_reader_rejects_oversized_response() {
        let mut within_limit = io::Cursor::new(b"1234".as_slice());
        assert_eq!(
            super::read_sanitizer_stdout_limited(&mut within_limit, 4).expect("bounded output"),
            b"1234"
        );
        let mut oversized = io::Cursor::new(b"12345".as_slice());
        let err = super::read_sanitizer_stdout_limited(&mut oversized, 4)
            .expect_err("oversized output rejected");
        assert!(err.contains("output exceeds 4 bytes"));
    }
    struct AlwaysErrReader;
    impl io::Read for AlwaysErrReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("boom"))
        }
    }
    #[test]
    fn read_limited_rejects_expired_deadline_before_read() {
        let err = super::read_limited(
            io::Cursor::new(b"hello".as_slice()),
            16,
            Instant::now() - Duration::from_millis(1),
        )
        .expect_err("expired deadline");
        assert_eq!(err.reason, SanitizeRejectReason::Sandbox);
        assert_eq!(err.message, "attachment sanitize timeout exceeded");
    }
    #[test]
    fn read_limited_wraps_reader_error_as_checksum() {
        let err = super::read_limited(
            AlwaysErrReader,
            16,
            Instant::now() + Duration::from_millis(100),
        )
        .expect_err("reader error");
        assert_eq!(err.reason, SanitizeRejectReason::Checksum);
        assert!(err.message.contains("attachment decompress failed"));
    }
    #[test]
    fn read_limited_rejects_oversized_output() {
        let err = super::read_limited(
            io::Cursor::new(b"hello".as_slice()),
            4,
            Instant::now() + Duration::from_millis(100),
        )
        .expect_err("oversized output");
        assert_eq!(err.reason, SanitizeRejectReason::Expansion);
        assert_eq!(
            err.message,
            "attachment expanded beyond max bytes (>4 bytes)"
        );
    }
    fn encode_sanitizer_response(response: &super::SanitizerResponse) -> Vec<u8> {
        norito::encode_canonical(response).expect("encode canonical sanitizer response")
    }
    fn canonical_sanitizer_request() -> super::SanitizerRequest {
        super::SanitizerRequest {
            declared_type: Some(super::JSON_MIME_TYPE.to_owned()),
            body: br#"{"hello":"world"}"#.to_vec(),
            allowed_mime_types: vec![super::JSON_MIME_TYPE.to_owned()],
            max_expanded_bytes: 1024,
            max_archive_depth: 1,
            timeout_ms: 500,
        }
    }
    #[test]
    fn decode_sanitizer_request_bytes_accepts_exact_canonical_frame() {
        let expected = canonical_sanitizer_request();
        let bytes = norito::encode_canonical(&expected).expect("encode canonical request");
        let decoded = super::decode_sanitizer_request_bytes(&bytes).expect("decode request");
        assert_eq!(decoded.declared_type, expected.declared_type);
        assert_eq!(decoded.body, expected.body);
        assert_eq!(decoded.allowed_mime_types, expected.allowed_mime_types);
        assert_eq!(decoded.max_expanded_bytes, expected.max_expanded_bytes);
        assert_eq!(decoded.max_archive_depth, expected.max_archive_depth);
        assert_eq!(decoded.timeout_ms, expected.timeout_ms);
    }
    #[test]
    fn decode_sanitizer_request_bytes_rejects_truncated_frame() {
        let mut bytes = norito::encode_canonical(&canonical_sanitizer_request())
            .expect("encode canonical request");
        bytes.pop().expect("request frame is non-empty");
        let err = super::decode_sanitizer_request_bytes(&bytes)
            .expect_err("truncated request must fail closed");
        assert_eq!(err.reason, SanitizeRejectReason::Sandbox);
        assert!(err.message.contains("request decode failed"));
    }
    #[test]
    fn decode_sanitizer_response_bytes_propagates_type_error() {
        let err = super::decode_sanitizer_response_bytes(&encode_sanitizer_response(
            &super::SanitizerResponse::Rejected {
                error: super::SanitizeErrorWire {
                    reason: "type".to_string(),
                    message: "unsupported attachment format".to_string(),
                },
            },
        ))
        .expect_err("type reject");
        assert_eq!(err.reason, SanitizeRejectReason::Type);
        assert_eq!(err.message, "unsupported attachment format");
    }
    #[test]
    fn decode_sanitizer_response_bytes_accepts_success_response() {
        let outcome = super::decode_sanitizer_response_bytes(&encode_sanitizer_response(
            &super::SanitizerResponse::Accepted {
                summary: super::SanitizerSummary {
                    sniffed_type: super::JSON_MIME_TYPE.to_string(),
                    expanded_bytes: 17,
                    archive_depth: 1,
                    sandboxed: true,
                },
                sanitized_body: br#"{"hello":"world"}"#.to_vec(),
            },
        ))
        .expect("successful decode");
        assert_eq!(outcome.summary.sniffed_type, super::JSON_MIME_TYPE);
        assert_eq!(outcome.summary.expanded_bytes, 17);
        assert_eq!(outcome.summary.archive_depth, 1);
        assert!(outcome.summary.sandboxed);
        assert_eq!(outcome.sanitized_body, br#"{"hello":"world"}"#.to_vec());
    }
    #[test]
    fn decode_sanitizer_response_bytes_maps_unknown_reason_to_sandbox() {
        let err = super::decode_sanitizer_response_bytes(&encode_sanitizer_response(
            &super::SanitizerResponse::Rejected {
                error: super::SanitizeErrorWire {
                    reason: "mystery".to_string(),
                    message: "unexpected failure".to_string(),
                },
            },
        ))
        .expect_err("unknown reject");
        assert_eq!(err.reason, SanitizeRejectReason::Sandbox);
        assert_eq!(err.message, "unexpected failure");
    }
    #[test]
    fn decode_sanitizer_response_bytes_rejects_truncated_frame() {
        let mut bytes = encode_sanitizer_response(&super::SanitizerResponse::Rejected {
            error: super::SanitizeErrorWire {
                reason: "sandbox".to_owned(),
                message: "rejected".to_owned(),
            },
        });
        bytes.pop().expect("response frame is non-empty");
        let err = super::decode_sanitizer_response_bytes(&bytes)
            .expect_err("truncated response must fail closed");
        assert_eq!(err.reason, SanitizeRejectReason::Sandbox);
        assert!(err.message.contains("response decode failed"));
    }
    #[test]
    fn decode_sanitizer_response_bytes_rejects_well_framed_unknown_variant() {
        let frame = encode_sanitizer_response(&super::SanitizerResponse::Rejected {
            error: super::SanitizeErrorWire {
                reason: "sandbox".to_owned(),
                message: "rejected".to_owned(),
            },
        });
        let view = norito::core::from_bytes_view(&frame).expect("inspect canonical response");
        let flags = view.flags();
        let mut payload = view.as_bytes().to_vec();
        payload[..core::mem::size_of::<u32>()].copy_from_slice(&u32::MAX.to_le_bytes());
        let forged =
            norito::core::frame_bare_with_header_flags::<super::SanitizerResponse>(&payload, flags)
                .expect("frame response with an unknown variant");
        let err = super::decode_sanitizer_response_bytes(&forged)
            .expect_err("unknown response variants must fail closed");
        assert_eq!(err.reason, SanitizeRejectReason::Sandbox);
        assert!(err.message.contains("response decode failed"));
    }
    #[test]
    fn sanitizer_rejects_fixture_gzip_bomb() {
        let cfg = test_sanitizer_config(64 * 1024, 2);
        let gz = load_fixture_base64("gzip_bomb_1m.b64");
        let err = sanitize_attachment_sync(None, &gz, &cfg).expect_err("expansion rejected");
        assert_eq!(err.reason, SanitizeRejectReason::Expansion);
    }
    #[test]
    fn sanitizer_rejects_fixture_zstd_nested_depth() {
        let cfg = test_sanitizer_config(4 * 1024 * 1024, 1);
        let payload = load_fixture_base64("zstd_nested_depth2.b64");
        let err = sanitize_attachment_sync(None, &payload, &cfg).expect_err("depth rejected");
        assert_eq!(err.reason, SanitizeRejectReason::Expansion);
    }
    #[test]
    fn zk1_extract_tags_collects_tlv_tags() {
        let mut bytes = b"ZK1\0".to_vec();
        bytes.extend_from_slice(b"PROF");
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(b"IPAK");
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 2, 3, 4]);
        let tags = parse_zk1_tags(&bytes).expect("zk1 tags");
        assert_eq!(tags, vec!["PROF".to_string(), "IPAK".to_string()]);
    }
    #[test]
    fn zk1_attachment_tag_extraction_rejects_excess_tlvs_without_partial_metadata() {
        let mut bytes = b"ZK1\0".to_vec();
        for _ in 0..ZK1_MAX_TLV_COUNT {
            bytes.extend_from_slice(b"PROF");
            bytes.extend_from_slice(&0u32.to_le_bytes());
        }
        assert_eq!(parse_zk1_tags(&bytes), Ok(vec!["PROF".to_owned()]));
        bytes.extend_from_slice(b"IPAK");
        bytes.extend_from_slice(&0u32.to_le_bytes());
        assert!(parse_zk1_tags(&bytes).is_err());
    }
    #[test]
    fn attachment_meta_tag_filter_requires_the_ingest_index() {
        let tenant = super::AttachmentTenant::anonymous();
        let mut meta = AttachmentMeta {
            id: "deadbeef".repeat(8),
            content_type: super::ZK1_MIME_TYPE.to_string(),
            size: 8,
            created_ms: 1_700_000_000_000,
            tenant: Some(tenant.as_str().to_string()),
            provenance: None,
            zk1_tags: None,
        };
        assert!(!super::attachment_meta_has_tag(&meta, "PROF"));
        meta.zk1_tags = Some(vec!["PROF".to_string()]);
        assert!(super::attachment_meta_has_tag(&meta, "PROF"));
        assert!(!super::attachment_meta_has_tag(&meta, "IPAK"));
    }
    #[test]
    fn needs_export_sanitization_flags_missing_or_nested() {
        let base = AttachmentMeta {
            id: "deadbeef".repeat(4),
            content_type: super::JSON_MIME_TYPE.to_string(),
            size: 8,
            created_ms: 1_700_000_000_000,
            tenant: None,
            provenance: None,
            zk1_tags: None,
        };
        assert!(super::needs_export_sanitization(&base));
        let mut meta = base;
        meta.provenance = Some(AttachmentProvenance {
            declared_type: Some(super::JSON_MIME_TYPE.to_string()),
            sniffed_type: super::JSON_MIME_TYPE.to_string(),
            hashes: AttachmentHashes {
                blake2b_256: "a".repeat(64),
                sha256: "b".repeat(64),
            },
            sanitizer: AttachmentSanitizerVerdict {
                verdict: "accepted".to_string(),
                expanded_bytes: 8,
                archive_depth: 1,
                sandboxed: false,
            },
        });
        assert!(super::needs_export_sanitization(&meta));
        if let Some(provenance) = meta.provenance.as_mut() {
            provenance.sanitizer.archive_depth = 0;
        }
        assert!(!super::needs_export_sanitization(&meta));
    }
    #[tokio::test]
    async fn post_attachment_records_provenance() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        ensure_test_config();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/problem+json"),
        );
        let body = axum::body::Bytes::from_static(br#"{"hello":"world"}"#);
        let response =
            super::handle_post_attachment(super::AttachmentTenant::anonymous(), headers, body)
                .await
                .into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        let meta_bytes = response
            .into_body()
            .collect()
            .await
            .expect("response body")
            .to_bytes();
        let meta_text = std::str::from_utf8(&meta_bytes).expect("utf8");
        let meta: AttachmentMeta = json::from_json(meta_text).expect("meta");
        assert_eq!(meta.content_type, super::JSON_MIME_TYPE);
        let provenance = meta.provenance.expect("provenance");
        assert_eq!(provenance.sniffed_type, super::JSON_MIME_TYPE);
        assert_eq!(
            provenance.declared_type.as_deref(),
            Some(super::JSON_MIME_TYPE)
        );
        assert_eq!(provenance.sanitizer.verdict, "accepted");
        assert_eq!(provenance.sanitizer.archive_depth, 0);
    }
    #[tokio::test]
    async fn compressed_attachment_cannot_expand_beyond_per_item_storage_cap() {
        let _data_dir = crate::test_utils::TestDataDirGuard::new();
        ensure_test_config();
        let tenant = super::AttachmentTenant::anonymous();
        let expanded = format!(r#"{{"padding":"{}"}}"#, "x".repeat(2_000)).into_bytes();
        assert!(expanded.len() > super::max_bytes_cfg());
        assert!(expanded.len() as u64 <= super::max_expanded_bytes_cfg());
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&expanded)
            .expect("compress oversized canonical attachment");
        let compressed = encoder.finish().expect("finish attachment compression");
        assert!(compressed.len() <= super::max_bytes_cfg());

        let response = super::handle_post_attachment(
            tenant.clone(),
            HeaderMap::new(),
            axum::body::Bytes::from(compressed),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(
            super::list_all_ids(&tenant).is_empty(),
            "an expanded body above the per-item cap must not be persisted"
        );
        assert!(
            !super::attachments_dir(&tenant).exists(),
            "a rejected expanded body must not leave a tenant directory"
        );
    }
    #[test]
    fn quota_transaction_validation_rejects_duplicate_and_unbounded_victim_sets() {
        let tenant = super::AttachmentTenant("a".repeat(super::TENANT_KEY_HEX_LEN));
        let incoming = canonical_test_meta(&tenant, br#"{"incoming":true}"#);
        let victim = "b".repeat(super::ATTACHMENT_ID_HEX_LEN);
        let duplicate = quota_transaction(
            &tenant,
            incoming.clone(),
            None,
            vec![victim.clone(), victim.clone()],
        );
        assert_eq!(
            super::validate_attachment_quota_transaction(&duplicate)
                .expect_err("duplicate victims must be rejected")
                .kind(),
            io::ErrorKind::InvalidData
        );

        let too_many = quota_transaction(
            &tenant,
            incoming,
            None,
            vec![
                victim;
                usize::try_from(super::ATTACHMENT_META_SCAN_MAX_FILES)
                    .expect("test scan bound fits usize")
                    + 1
            ],
        );
        assert_eq!(
            super::validate_attachment_quota_transaction(&too_many)
                .expect_err("unbounded victim sets must be rejected")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[test]
    fn quota_recovery_rejects_oversized_journal_without_mutating_attachments() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        ensure_test_config();
        let tenant = super::AttachmentTenant::from_account(&checked_attachment_account(0x57));
        let victim = persist_canonical_test_attachment(&tenant, br#"{"victim":"untouched"}"#, 1);
        super::ensure_attachment_quota_transaction_dir_durable()
            .expect("create durable transaction directory");
        fs::write(
            super::attachment_quota_transaction_path(),
            vec![
                b'x';
                usize::try_from(super::ATTACHMENT_QUOTA_TRANSACTION_MAX_BYTES)
                    .expect("test journal bound fits usize")
                    + 1
            ],
        )
        .expect("write oversized quota journal");

        assert!(super::recover_attachment_quota_transaction().is_err());
        assert!(
            !super::init_persistence(),
            "startup recovery failure must be visible to worker orchestration"
        );
        assert!(super::meta_path(&tenant, &victim.id).is_file());
        assert!(super::bin_path(&tenant, &victim.id).is_file());
        assert!(super::attachment_quota_transaction_path().is_file());
    }
    #[test]
    fn quota_recovery_rolls_back_partial_incoming_and_preserves_victims() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        ensure_test_config();
        let tenant = super::AttachmentTenant::from_account(&checked_attachment_account(0x54));
        let victim_body = br#"{"victim":"preserved"}"#;
        let victim = persist_canonical_test_attachment(&tenant, victim_body, 1);
        let incoming_body = br#"{"incoming":"partial"}"#;
        let incoming = canonical_test_meta(&tenant, incoming_body);
        let transaction =
            quota_transaction(&tenant, incoming.clone(), None, vec![victim.id.clone()]);
        super::persist_attachment_quota_transaction(&transaction)
            .expect("persist quota transaction intent");
        super::persist_body(&tenant, &incoming.id, incoming_body)
            .expect("persist only the incoming body phase");

        assert!(
            super::recover_attachment_quota_transaction().expect("recover partial transaction")
        );
        assert!(super::meta_path(&tenant, &victim.id).is_file());
        assert!(super::bin_path(&tenant, &victim.id).is_file());
        assert!(!super::meta_path(&tenant, &incoming.id).exists());
        assert!(!super::bin_path(&tenant, &incoming.id).exists());
        assert!(!super::attachment_quota_transaction_path().exists());
    }
    #[test]
    fn quota_recovery_restores_previous_metadata_for_partial_repost() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        ensure_test_config();
        let tenant = super::AttachmentTenant::from_account(&checked_attachment_account(0x55));
        let victim =
            persist_canonical_test_attachment(&tenant, br#"{"victim":"preserved-on-repost"}"#, 1);
        let incoming_body = br#"{"incoming":"same-id-repost"}"#;
        let previous = persist_canonical_test_attachment(&tenant, incoming_body, 2);
        let mut replacement = previous.clone();
        replacement.created_ms = 3;
        let transaction = quota_transaction(
            &tenant,
            replacement,
            Some(previous.clone()),
            vec![victim.id.clone()],
        );
        super::persist_attachment_quota_transaction(&transaction)
            .expect("persist repost quota transaction intent");
        super::persist_body(&tenant, &previous.id, incoming_body)
            .expect("persist repost body phase only");

        assert!(super::recover_attachment_quota_transaction().expect("recover partial repost"));
        assert_eq!(super::load_meta(&tenant, &previous.id), Some(previous));
        assert!(super::meta_path(&tenant, &victim.id).is_file());
        assert!(super::bin_path(&tenant, &victim.id).is_file());
        assert!(!super::attachment_quota_transaction_path().exists());
    }
    #[test]
    fn quota_recovery_finalizes_evictions_after_complete_incoming_commit() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        ensure_test_config();
        let tenant = super::AttachmentTenant::from_account(&checked_attachment_account(0x56));
        let victim = persist_canonical_test_attachment(&tenant, br#"{"victim":"evicted"}"#, 1);
        let incoming_body = br#"{"incoming":"complete"}"#;
        let incoming = canonical_test_meta(&tenant, incoming_body);
        let transaction =
            quota_transaction(&tenant, incoming.clone(), None, vec![victim.id.clone()]);
        super::persist_attachment_quota_transaction(&transaction)
            .expect("persist quota transaction intent");
        super::persist_body(&tenant, &incoming.id, incoming_body).expect("persist incoming body");
        super::save_meta(&tenant, &incoming).expect("persist incoming metadata");

        assert!(
            super::recover_attachment_quota_transaction().expect("recover complete transaction")
        );
        assert!(super::meta_path(&tenant, &incoming.id).is_file());
        assert!(super::bin_path(&tenant, &incoming.id).is_file());
        assert!(!super::meta_path(&tenant, &victim.id).exists());
        assert!(!super::bin_path(&tenant, &victim.id).exists());
        assert!(!super::attachment_quota_transaction_path().exists());
    }
    #[tokio::test]
    async fn global_count_quota_evicts_only_the_submitting_tenant() {
        let _data_dir = crate::test_utils::TestDataDirGuard::new();
        ensure_test_config();
        let first = super::AttachmentTenant::from_account(&checked_attachment_account(0x51));
        let second = super::AttachmentTenant::from_account(&checked_attachment_account(0x52));
        let third = super::AttachmentTenant::from_account(&checked_attachment_account(0x53));

        let mut first_oldest = None;
        for index in 0..10 {
            let body = format!(r#"{{"tenant":"first","index":{index}}}"#).into_bytes();
            let meta = persist_canonical_test_attachment(
                &first,
                &body,
                u64::try_from(index).expect("test index fits u64") + 1,
            );
            if index == 0 {
                first_oldest = Some(meta.id);
            }
        }
        for index in 0..10 {
            let body = format!(r#"{{"tenant":"second","index":{index}}}"#).into_bytes();
            persist_canonical_test_attachment(
                &second,
                &body,
                u64::try_from(index).expect("test index fits u64") + 100,
            );
        }
        let second_before = super::list_all_ids(&second)
            .into_iter()
            .collect::<BTreeSet<_>>();

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        let response = super::handle_post_attachment(
            first.clone(),
            headers.clone(),
            axum::body::Bytes::from_static(br#"{"tenant":"first","index":10}"#),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(super::list_all_ids(&first).len(), 10);
        assert!(
            !super::meta_path(&first, &first_oldest.expect("oldest first-tenant id")).exists(),
            "the submitting tenant's oldest entry should make room"
        );
        assert_eq!(
            super::list_all_ids(&second)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            second_before,
            "same-tenant replacement must not evict another tenant"
        );
        let first_before_rejection = super::list_all_ids(&first)
            .into_iter()
            .collect::<BTreeSet<_>>();

        let response = super::handle_post_attachment(
            third.clone(),
            headers,
            axum::body::Bytes::from_static(br#"{"tenant":"third","index":0}"#),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(super::list_all_ids(&third).is_empty());
        assert_eq!(
            super::list_all_ids(&first)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            first_before_rejection,
            "global exhaustion must reject before deleting another tenant's data"
        );
        assert_eq!(
            super::list_all_ids(&second)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            second_before,
            "global exhaustion must preserve all non-submitting tenants"
        );
    }
    #[tokio::test]
    async fn quota_victims_survive_incoming_persistence_failure() {
        let _data_dir = crate::test_utils::TestDataDirGuard::new();
        ensure_test_config();
        let tenant = super::AttachmentTenant::from_account(&checked_attachment_account(0x5a));
        for index in 0..10 {
            let body = format!(r#"{{"retained":{index}}}"#).into_bytes();
            persist_canonical_test_attachment(
                &tenant,
                &body,
                u64::try_from(index).expect("test index fits u64") + 1,
            );
        }
        let retained_before = super::list_all_ids(&tenant)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let incoming = br#"{"incoming":"must-fail-before-eviction"}"#;
        let incoming_id = canonical_test_meta(&tenant, incoming).id;
        fs::create_dir(super::bin_path(&tenant, &incoming_id))
            .expect("install a real filesystem persistence failure");
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );

        let response = super::handle_post_attachment(
            tenant.clone(),
            headers,
            axum::body::Bytes::from_static(incoming),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            super::list_all_ids(&tenant)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            retained_before,
            "a failed incoming write must not delete any planned quota victim"
        );
        assert!(!super::meta_path(&tenant, &incoming_id).exists());
        assert!(
            super::bin_path(&tenant, &incoming_id).is_dir(),
            "the injected directory should be the only incoming-path artifact"
        );
    }
    #[tokio::test]
    async fn quota_eviction_failure_returns_error_and_retains_recovery_intent() {
        let _data_dir = crate::test_utils::TestDataDirGuard::new();
        ensure_test_config();
        let tenant = super::AttachmentTenant::from_account(&checked_attachment_account(0x5b));
        let mut oldest = None;
        for index in 0..10 {
            let body = format!(r#"{{"retained":{index}}}"#).into_bytes();
            let meta = persist_canonical_test_attachment(
                &tenant,
                &body,
                u64::try_from(index).expect("test index fits u64") + 1,
            );
            if index == 0 {
                oldest = Some(meta.id);
            }
        }
        let oldest = oldest.expect("oldest quota victim");
        let victim_body = super::bin_path(&tenant, &oldest);
        fs::remove_file(&victim_body).expect("replace victim body with deletion fault");
        fs::create_dir(&victim_body).expect("create victim body obstruction");
        let obstruction = victim_body.join("still-present");
        fs::write(&obstruction, b"block deletion").expect("write deletion obstruction");
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        let incoming = br#"{"incoming":"durable-before-eviction"}"#;
        let incoming_id = canonical_test_meta(&tenant, incoming).id;

        let response = super::handle_post_attachment(
            tenant.clone(),
            headers,
            axum::body::Bytes::from_static(incoming),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(super::meta_path(&tenant, &incoming_id).is_file());
        assert!(super::bin_path(&tenant, &incoming_id).is_file());
        assert!(
            super::attachment_quota_transaction_path().is_file(),
            "failed eviction must retain its durable recovery intent"
        );

        fs::remove_file(&obstruction).expect("remove deletion obstruction file");
        fs::remove_dir(&victim_body).expect("remove deletion obstruction directory");
        assert!(
            super::recover_attachment_quota_transaction()
                .expect("retry durable quota eviction after fault repair")
        );
        assert!(!super::attachment_quota_transaction_path().exists());
        assert!(!super::meta_path(&tenant, &oldest).exists());
        assert!(!super::bin_path(&tenant, &oldest).exists());
        assert_eq!(super::list_all_ids(&tenant).len(), 10);
    }
    #[tokio::test]
    async fn global_byte_quota_rejects_cross_tenant_growth_without_eviction() {
        let _data_dir = crate::test_utils::TestDataDirGuard::new();
        ensure_test_config();
        let first = super::AttachmentTenant::from_account(&checked_attachment_account(0x61));
        let second = super::AttachmentTenant::from_account(&checked_attachment_account(0x62));
        let third = super::AttachmentTenant::from_account(&checked_attachment_account(0x63));
        for index in 0..4 {
            let body = padded_json_body("first", index, 1_000);
            persist_canonical_test_attachment(
                &first,
                &body,
                u64::try_from(index).expect("test index fits u64") + 1,
            );
            let body = padded_json_body("second", index, 1_000);
            persist_canonical_test_attachment(
                &second,
                &body,
                u64::try_from(index).expect("test index fits u64") + 100,
            );
        }
        let first_before = super::list_all_ids(&first)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let second_before = super::list_all_ids(&second)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        let response = super::handle_post_attachment(
            third.clone(),
            headers,
            axum::body::Bytes::from(padded_json_body("third", 0, 300)),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(super::list_all_ids(&third).is_empty());
        assert_eq!(
            super::list_all_ids(&first)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            first_before
        );
        assert_eq!(
            super::list_all_ids(&second)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            second_before
        );
    }
    #[tokio::test]
    async fn attachment_tenant_churn_does_not_leave_empty_quota_directories() {
        let _data_dir = crate::test_utils::TestDataDirGuard::new();
        ensure_test_config();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        for index in 0_u8..32 {
            let tenant = super::AttachmentTenant::from_account(&checked_attachment_account(
                index.saturating_add(0x80),
            ));
            let body = format!(r#"{{"tenant_churn":{index}}}"#).into_bytes();
            let id = canonical_test_meta(&tenant, &body).id;
            let response = super::handle_post_attachment(
                tenant.clone(),
                headers.clone(),
                axum::body::Bytes::from(body),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::CREATED);
            assert!(super::attachments_dir(&tenant).is_dir());

            let response = super::handle_delete_attachment(tenant.clone(), axum::extract::Path(id))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            assert!(
                !super::attachments_dir(&tenant).exists(),
                "deleted tenant {index} left an empty quota directory"
            );
        }
        assert_eq!(
            fs::read_dir(super::attachments_root_dir())
                .expect("read attachment root after churn")
                .count(),
            0,
            "tenant churn must not accumulate unaccounted root entries"
        );
    }
    #[tokio::test]
    async fn same_id_repost_at_quota_preserves_processing_receipt() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        ensure_test_config();
        let tenant = super::AttachmentTenant::anonymous();
        let mut target: Option<(AttachmentMeta, Vec<u8>)> = None;
        for index in 0..10 {
            let body = format!(r#"{{"attachment":{index}}}"#).into_bytes();
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("application/json"),
            );
            let response = super::handle_post_attachment(
                tenant.clone(),
                headers,
                axum::body::Bytes::from(body.clone()),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::CREATED);
            let meta_bytes = response
                .into_body()
                .collect()
                .await
                .expect("attachment response")
                .to_bytes();
            let meta: AttachmentMeta = json::from_json(
                std::str::from_utf8(&meta_bytes).expect("attachment metadata UTF-8"),
            )
            .expect("attachment metadata JSON");
            if index == 0 {
                target = Some((meta, body));
            }
        }
        let (mut target_meta, target_body) = target.expect("target attachment");
        target_meta.created_ms = 0;
        super::save_meta(&tenant, &target_meta).expect("make target the oldest quota entry");
        assert!(
            super::persist_prover_processing_receipt_if_referenced(
                &super::ProverProcessingReceipt {
                    version: super::ZK_PROVER_PROCESSING_STATE_VERSION,
                    id: target_meta.id.clone(),
                    processed_ms: 1,
                    terminal: true,
                    retry_not_before_ms: None,
                    retry_count: 0,
                    completed_proof_indices: Vec::new(),
                    processing_context_hash: None,
                }
            )
            .expect("persist terminal processing receipt")
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        let response = super::handle_post_attachment(
            tenant.clone(),
            headers,
            axum::body::Bytes::from(target_body),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(super::list_all_ids(&tenant).len(), 10);
        assert_eq!(
            super::prover_processing_decision(&target_meta.id, u64::MAX),
            super::ProverProcessingDecision::Suppress,
            "reposting identical content must not evict its durable processing state"
        );
    }
    #[tokio::test]
    async fn gc_rechecks_expiry_after_waiting_for_a_repost() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        ensure_test_config();
        let tenant = super::AttachmentTenant::anonymous();
        let body = br#"{"attachment":"gc-race"}"#.to_vec();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        let response =
            super::handle_post_attachment(tenant.clone(), headers, axum::body::Bytes::from(body))
                .await
                .into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        let meta_bytes = response
            .into_body()
            .collect()
            .await
            .expect("attachment response")
            .to_bytes();
        let mut meta: AttachmentMeta =
            json::from_json(std::str::from_utf8(&meta_bytes).expect("attachment metadata UTF-8"))
                .expect("attachment metadata JSON");
        meta.created_ms = 0;
        super::save_meta(&tenant, &meta).expect("mark attachment as initially expired");
        assert!(
            super::persist_prover_processing_receipt_if_referenced(
                &super::ProverProcessingReceipt {
                    version: super::ZK_PROVER_PROCESSING_STATE_VERSION,
                    id: meta.id.clone(),
                    processed_ms: 1,
                    terminal: true,
                    retry_not_before_ms: None,
                    retry_count: 0,
                    completed_proof_indices: Vec::new(),
                    processing_context_hash: None,
                }
            )
            .expect("persist terminal processing receipt")
        );

        let mutation_guard = super::quota_lock().lock().await;
        let gc_tenant = tenant.clone();
        let gc_id = meta.id.clone();
        let (before_lock_tx, before_lock_rx) = tokio::sync::oneshot::channel();
        let gc = tokio::spawn(async move {
            super::delete_attachment_if_expired_with_before_lock(
                &gc_tenant,
                &gc_id,
                Duration::from_secs(60),
                move || {
                    let _ = before_lock_tx.send(());
                },
            )
            .await
        });
        before_lock_rx
            .await
            .expect("GC task reaches the shared mutation lock");
        meta.created_ms = super::now_ms();
        super::save_meta(&tenant, &meta).expect("refresh attachment metadata during repost");
        drop(mutation_guard);

        assert!(
            !gc.await
                .expect("GC task joins")
                .expect("GC recheck succeeds"),
            "GC must not delete content refreshed while it waited for the mutation lock"
        );
        assert!(super::meta_path(&tenant, &meta.id).is_file());
        assert!(super::bin_path(&tenant, &meta.id).is_file());
        assert_eq!(
            super::prover_processing_decision(&meta.id, u64::MAX),
            super::ProverProcessingDecision::Suppress
        );
    }
    #[tokio::test]
    async fn post_attachment_rejects_over_cardinality_zk1_before_persistence() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        ensure_test_config();
        let mut envelope = b"ZK1\0".to_vec();
        for _ in 0..=ZK1_MAX_TLV_COUNT {
            envelope.extend_from_slice(b"PROF");
            envelope.extend_from_slice(&0u32.to_le_bytes());
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/x-zk1"),
        );
        let response = super::handle_post_attachment(
            super::AttachmentTenant::anonymous(),
            headers,
            axum::body::Bytes::from(envelope),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            super::list_all_ids(&super::AttachmentTenant::anonymous()).is_empty(),
            "a rejected envelope must not create attachment state"
        );
    }
    #[tokio::test]
    async fn get_attachment_resanitizes_compressed_exports() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let _guard = crate::data_dir::OverrideGuard::new(tmp.path());
        ensure_test_config();
        super::init_persistence();
        let payload = br#"{"hello":"world"}"#;
        let compressed = gzip_compress(payload);
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        let response = super::handle_post_attachment(
            super::AttachmentTenant::anonymous(),
            headers,
            axum::body::Bytes::from(compressed),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        let meta_bytes = response
            .into_body()
            .collect()
            .await
            .expect("meta body")
            .to_bytes();
        let meta_text = std::str::from_utf8(&meta_bytes).expect("utf8 meta");
        let meta: AttachmentMeta = json::from_json(meta_text).expect("meta");
        let expected_id = hex::encode::<[u8; 32]>(Hash::new(payload).into());
        assert_eq!(meta.id, expected_id);
        assert_eq!(meta.size, payload.len() as u64);
        let provenance = meta.provenance.expect("provenance");
        assert!(provenance.sanitizer.archive_depth > 0);
        let response = super::handle_get_attachment(
            super::AttachmentTenant::anonymous(),
            axum::extract::Path(meta.id.clone()),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = response
            .into_body()
            .collect()
            .await
            .expect("body bytes")
            .to_bytes();
        assert_eq!(body_bytes.as_ref(), payload);
    }
}
