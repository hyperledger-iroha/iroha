//! Minimal ZK attachments store for the app-facing API.
//!
//! Feature-gated behind `app_api`:
//! - Stores attachments (proof envelopes or JSON DTOs) under `./storage/torii/zk_attachments/`.
//!   Base directory is configured via `torii.data_dir`; tests may use `data_dir::OverrideGuard`.
//! - Deterministic id: Blake2b-32 of the sanitized request bytes (lowercase hex).
//! - Endpoints:
//!   - POST `/v1/zk/attachments` – store attachment, returns metadata `{ id, size, content_type, created_ms }`.
//!   - GET  `/v1/zk/attachments` – list metadata for stored attachments.
//!   - GET  `/v1/zk/attachments/{id}` – fetch stored attachment bytes by id.
//!   - DELETE `/v1/zk/attachments/{id}` – delete stored attachment and its metadata.
//! - A background GC task periodically deletes entries older than a TTL;
//!   TTL and size caps are provided via `iroha_config` (Torii).

use std::{
    env, fs,
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{OnceLock, mpsc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{extract::Path as AxumPath, http::StatusCode, response::IntoResponse};
use flate2::read::GzDecoder;
use iroha_config::parameters::actual::AttachmentSanitizerMode;
use iroha_logger::prelude::*;
use norito::{core as norito_core, json};
use sha2::{Digest as _, Sha256};
use tokio::{sync::Mutex, task};
use zstd::stream::read::Decoder as ZstdDecoder;

use crate::{NoritoQuery, routing::MaybeTelemetry, utils::NORITO_MIME_TYPE};

const MAX_ATTACHMENT_BYTES_FALLBACK: usize = 4 * 1024 * 1024; // fallback 4 MiB
const ATTACHMENT_TTL_SECS_FALLBACK: u64 = 7 * 24 * 60 * 60; // fallback 7 days
const GC_INTERVAL_SECS: u64 = 60; // run every minute
const DEFAULT_TENANT: &str = "anon";
const ATTACHMENT_ID_HEX_LEN: usize = 64;
const ZK1_MIME_TYPE: &str = "application/x-zk1";
const OCTET_STREAM_MIME_TYPE: &str = "application/octet-stream";
const JSON_MIME_TYPE: &str = "application/json";
const TEXT_JSON_MIME_TYPE: &str = "text/json";
const ATTACHMENT_SANITIZER_ENV: &str = "IROHA_ATTACHMENT_SANITIZER";
const ATTACHMENT_SANITIZER_MAX_INPUT_ENV: &str = "IROHA_ATTACHMENT_SANITIZER_MAX_INPUT_BYTES";
const SANITIZER_POLL_INTERVAL_MS: u64 = 5;

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
}

pub(crate) fn base_dir() -> PathBuf {
    crate::data_dir::base_dir()
}

fn attachments_dir() -> PathBuf {
    base_dir().join("zk_attachments")
}

fn ensure_dirs() {
    if cfg!(test) {
        let _ = fs::create_dir_all(attachments_dir());
        return;
    }
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = fs::create_dir_all(attachments_dir());
    });
}

fn meta_path(id: &str) -> PathBuf {
    attachments_dir().join(format!("{}.json", id))
}

fn bin_path(id: &str) -> PathBuf {
    attachments_dir().join(format!("{}.bin", id))
}

/// Initialize on-disk directories for attachments storage.
pub fn init_persistence() {
    ensure_dirs();
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn list_all_ids() -> Vec<String> {
    let mut ids = Vec::new();
    if let Ok(rd) = fs::read_dir(attachments_dir()) {
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

fn load_meta(id: &str) -> Option<AttachmentMeta> {
    let id = sanitize_attachment_id(id)?;
    let path = meta_path(&id);
    let mut f = fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let s = std::str::from_utf8(&buf).ok()?;
    json::from_json(s).ok()
}

fn save_meta(meta: &AttachmentMeta) -> std::io::Result<()> {
    let path = meta_path(&meta.id);
    ensure_dirs();
    let tmp_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(tmp_dir)?;
    let body = json::to_json_pretty(meta).unwrap_or_else(|_| "{}".into());
    tmp.write_all(body.as_bytes())?;
    tmp.flush()?;
    tmp.persist(&path).map(|_| ()).map_err(|e| e.error)
}

fn persist_body(id: &str, body: &[u8]) -> std::io::Result<()> {
    let path = bin_path(id);
    ensure_dirs();
    let tmp_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(tmp_dir)?;
    tmp.write_all(body)?;
    tmp.flush()?;
    tmp.persist(&path).map(|_| ()).map_err(|e| e.error)
}

fn delete_attachment_files(id: &str) {
    if let Some(clean) = sanitize_attachment_id(id) {
        let _ = fs::remove_file(meta_path(&clean));
        let _ = fs::remove_file(bin_path(&clean));
    }
}

fn tenant_key_from_headers(headers: &axum::http::HeaderMap) -> String {
    if let Some(token) = headers.get("x-api-token").and_then(|v| v.to_str().ok()) {
        return hash_identity("token", token);
    }
    if let Some(auth) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        return hash_identity("auth", auth);
    }
    DEFAULT_TENANT.to_string()
}

fn hash_identity(label: &str, value: &str) -> String {
    let mut buf = Vec::with_capacity(label.len() + 1 + value.len());
    buf.extend_from_slice(label.as_bytes());
    buf.push(b'|');
    buf.extend_from_slice(value.as_bytes());
    let hash = iroha_crypto::Hash::new(&buf);
    let digest: [u8; 32] = hash.into();
    format!("{label}:{}", hex::encode::<[u8; 32]>(digest))
}

fn meta_tenant(meta: &AttachmentMeta) -> &str {
    meta.tenant.as_deref().unwrap_or(DEFAULT_TENANT)
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
struct SanitizerResponse {
    ok: bool,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    summary: Option<SanitizerSummary>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    sanitized_body: Option<Vec<u8>>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    error: Option<SanitizeErrorWire>,
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
    if normalized == TEXT_JSON_MIME_TYPE || normalized.ends_with("+json") {
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
) -> Result<SanitizerOutcome, SanitizeError> {
    let cfg = sanitizer_config();
    match cfg.mode {
        AttachmentSanitizerMode::InProcess => {
            sanitize_attachment_in_process(declared_type, body, cfg).await
        }
        AttachmentSanitizerMode::Subprocess => {
            sanitize_attachment_subprocess(declared_type, body, cfg).await
        }
    }
}

async fn sanitize_attachment_in_process(
    declared_type: Option<String>,
    body: axum::body::Bytes,
    cfg: SanitizerConfig,
) -> Result<SanitizerOutcome, SanitizeError> {
    let declared = declared_type.clone();
    let mut outcome = task::spawn_blocking(move || {
        sanitize_attachment_sync(declared.as_deref(), body.as_ref(), &cfg)
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
) -> Result<SanitizerOutcome, SanitizeError> {
    let request = SanitizerRequest {
        declared_type,
        body: body.to_vec(),
        allowed_mime_types: cfg.allowed_mime_types.clone(),
        max_expanded_bytes: cfg.max_expanded_bytes,
        max_archive_depth: cfg.max_archive_depth,
        timeout_ms: cfg.timeout.as_millis().max(1) as u64,
    };
    let mut outcome = task::spawn_blocking(move || run_sanitizer_subprocess(request, cfg.timeout))
        .await
        .map_err(|err| {
            SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                format!("attachment sanitize task failed: {err}"),
            )
        })??;
    outcome.summary.sandboxed = true;
    Ok(outcome)
}

fn run_sanitizer_subprocess(
    request: SanitizerRequest,
    timeout: Duration,
) -> Result<SanitizerOutcome, SanitizeError> {
    let exe = sanitizer_executable()?;
    let request_bytes = norito::to_bytes(&request).map_err(|err| {
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
    let mut cmd = Command::new(exe);
    cmd.env(ATTACHMENT_SANITIZER_ENV, "1")
        .env(ATTACHMENT_SANITIZER_MAX_INPUT_ENV, max_input_bytes)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer spawn failed: {err}"),
        )
    })?;
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
    let mut stdout = child.stdout.take().ok_or_else(|| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            "attachment sanitizer stdout unavailable",
        )
    })?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let result = stdout.read_to_end(&mut buf).map(|_| buf);
        let _ = tx.send(result);
    });

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
        .map_err(|err| {
            SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                format!("attachment sanitizer stdout read failed: {err}"),
            )
        })?;
    let archived = norito::from_bytes::<SanitizerResponse>(&stdout_bytes).map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer response decode failed: {err}"),
        )
    })?;
    let response: SanitizerResponse = norito_core::NoritoDeserialize::deserialize(archived);
    if response.ok {
        let summary = response.summary.ok_or_else(|| {
            SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                "attachment sanitizer response missing summary",
            )
        })?;
        let sanitized_body = response.sanitized_body.ok_or_else(|| {
            SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                "attachment sanitizer response missing body",
            )
        })?;
        Ok(SanitizerOutcome {
            summary,
            sanitized_body,
        })
    } else {
        let wire = response.error.ok_or_else(|| {
            SanitizeError::new(
                SanitizeRejectReason::Sandbox,
                "attachment sanitizer response missing error",
            )
        })?;
        Err(SanitizeError::from_wire(wire))
    }
}

fn sanitizer_executable() -> Result<PathBuf, SanitizeError> {
    let override_path = ATTACH_CFG
        .get()
        .and_then(|cfg| cfg.sanitizer_exe_override.clone());
    sanitizer_executable_with_override(override_path)
}

fn sanitizer_executable_with_override(
    override_path: Option<PathBuf>,
) -> Result<PathBuf, SanitizeError> {
    if let Some(path) = override_path {
        return Ok(path);
    }
    env::current_exe().map_err(|err| {
        SanitizeError::new(
            SanitizeRejectReason::Sandbox,
            format!("attachment sanitizer executable unavailable: {err}"),
        )
    })
}

fn enforce_per_tenant_quota(tenant: &str, incoming_size: u64) -> bool {
    let max_count_raw = per_tenant_max_count_cfg();
    let max_bytes_raw = per_tenant_max_bytes_cfg();
    if max_count_raw == 0 && max_bytes_raw == 0 {
        return true;
    }

    let mut metas: Vec<AttachmentMeta> = list_all_ids()
        .into_iter()
        .filter_map(|id| load_meta(&id))
        .filter(|meta| meta_tenant(meta) == tenant)
        .collect();
    metas.sort_by(|a, b| {
        a.created_ms
            .cmp(&b.created_ms)
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut total_bytes: u64 = metas.iter().map(|m| m.size).sum();
    let mut count_after_add = metas.len() as u64 + 1;
    let max_count = if max_count_raw == 0 {
        u64::MAX
    } else {
        max_count_raw
    };
    let max_bytes = if max_bytes_raw == 0 {
        u64::MAX
    } else {
        max_bytes_raw
    };

    let mut idx = 0usize;
    let mut removed_ids: Vec<String> = Vec::new();
    while (count_after_add > max_count || total_bytes.saturating_add(incoming_size) > max_bytes)
        && idx < metas.len()
    {
        let victim = &metas[idx];
        removed_ids.push(victim.id.clone());
        total_bytes = total_bytes.saturating_sub(victim.size);
        count_after_add = count_after_add.saturating_sub(1);
        idx += 1;
    }

    if count_after_add > max_count || total_bytes.saturating_add(incoming_size) > max_bytes {
        warn!(
            %tenant,
            max_count,
            max_bytes,
            current_count = metas.len(),
            current_bytes = total_bytes,
            incoming_bytes = incoming_size,
            "rejecting attachment: unable to make room within tenant quota"
        );
        return false;
    }

    for id in removed_ids.iter() {
        delete_attachment_files(id);
    }
    if !removed_ids.is_empty() {
        info!(
            %tenant,
            removed = removed_ids.len(),
            max_count,
            max_bytes,
            count_after_add,
            bytes_after_removal = total_bytes,
            incoming_bytes = incoming_size,
            "evicted attachments to satisfy tenant quota"
        );
    }
    true
}

/// POST /v1/zk/attachments — store an attachment and return its metadata.
pub async fn handle_post_attachment(
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    // Enforce size cap
    if body.len() > max_bytes_cfg() {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("attachment too large (>{} bytes)", max_bytes_cfg()),
        )
            .into_response();
    }
    let tenant_key = tenant_key_from_headers(&headers);
    let raw_hash = {
        let h = iroha_crypto::Hash::new(&body);
        hex::encode::<[u8; 32]>(h.into())
    };
    let declared_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(normalize_mime);
    let sanitize_start = Instant::now();
    let sanitize_result = sanitize_attachment(declared_type.clone(), body.clone()).await;
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
    let per_tenant_max_bytes = per_tenant_max_bytes_cfg();
    if per_tenant_max_bytes > 0 && stored_size > per_tenant_max_bytes {
        warn!(
            tenant = %tenant_key,
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
    let id = {
        let h = iroha_crypto::Hash::new(&sanitized_body);
        hex::encode::<[u8; 32]>(h.into())
    };
    let _guard = quota_lock().lock().await;
    if !enforce_per_tenant_quota(&tenant_key, stored_size) {
        warn!(
            tenant = %tenant_key,
            body_bytes = stored_size,
            "rejecting attachment: per-tenant quota exceeded"
        );
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            "per-tenant attachment quota exceeded".to_string(),
        )
            .into_response();
    }
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
        tenant: Some(tenant_key),
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
    };
    if let Err(e) = persist_body(&id, &sanitized_body) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to persist body: {e}"),
        )
            .into_response();
    }
    if let Err(e) = save_meta(&meta) {
        // Rollback body if meta fails
        delete_attachment_files(&id);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to persist metadata: {e}"),
        )
            .into_response();
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
pub async fn handle_list_attachments() -> impl IntoResponse {
    // Unfiltered list for the default endpoint.
    handle_list_attachments_filtered(NoritoQuery(AttachmentListQuery::default())).await
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
    NoritoQuery(q): NoritoQuery<AttachmentListQuery>,
) -> impl IntoResponse {
    let mut metas: Vec<AttachmentMeta> = Vec::new();
    if let Some(id) = q.id.as_deref() {
        let Some(clean) = sanitize_attachment_id(id) else {
            return (
                StatusCode::BAD_REQUEST,
                "invalid attachment id (expected 64 hex characters)",
            )
                .into_response();
        };
        if let Some(m) = load_meta(&clean) {
            metas.push(m);
        }
    } else {
        for id in list_all_ids() {
            if let Some(m) = load_meta(&id) {
                metas.push(m);
            }
        }
    }
    // Filter by content-type and time bounds
    if let Some(ref ct) = q.content_type {
        metas.retain(|m| m.content_type.contains(ct));
    }
    if let Some(since) = q.since_ms {
        metas.retain(|m| m.created_ms >= since);
    }
    if let Some(before) = q.before_ms {
        metas.retain(|m| m.created_ms <= before);
    }
    // Optional ZK1 tag filter: requires scanning bodies and parsing TLVs for candidates
    if let Some(ref tag) = q.has_tag {
        metas.retain(|m| zk1_attachment_has_tag(&m.id, tag));
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
    NoritoQuery(q): NoritoQuery<AttachmentListQuery>,
) -> impl IntoResponse {
    // Reuse listing path but early-count
    let mut metas: Vec<AttachmentMeta> = Vec::new();
    if let Some(id) = q.id.as_deref() {
        let Some(clean) = sanitize_attachment_id(id) else {
            return (
                StatusCode::BAD_REQUEST,
                "invalid attachment id (expected 64 hex characters)",
            )
                .into_response();
        };
        if let Some(m) = load_meta(&clean) {
            metas.push(m);
        }
    } else {
        for id in list_all_ids() {
            if let Some(m) = load_meta(&id) {
                metas.push(m);
            }
        }
    }
    if let Some(ref ct) = q.content_type {
        metas.retain(|m| m.content_type.contains(ct));
    }
    if let Some(since) = q.since_ms {
        metas.retain(|m| m.created_ms >= since);
    }
    if let Some(before) = q.before_ms {
        metas.retain(|m| m.created_ms <= before);
    }
    if let Some(ref tag) = q.has_tag {
        metas.retain(|m| zk1_attachment_has_tag(&m.id, tag));
    }
    let count = metas.len() as u64;
    let s = norito::json::to_json_pretty(&crate::json_object(vec![("count", count)]))
        .unwrap_or_else(|_| "{}".into());
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(s))
        .unwrap()
}

// Minimal ZK1 tag scan for an attachment id. Returns true if the attachment
// body starts with ZK1 magic and contains a TLV with the given ASCII tag.
fn zk1_attachment_has_tag(id: &str, tag: &str) -> bool {
    let Some(clean) = sanitize_attachment_id(id) else {
        return false;
    };
    let Ok(bytes) = std::fs::read(bin_path(&clean)) else {
        return false;
    };
    zk1_bytes_has_tag(&bytes, tag)
}

fn zk1_bytes_has_tag(bytes: &[u8], tag: &str) -> bool {
    if bytes.len() < 8 {
        return false;
    }
    if &bytes[..4] != b"ZK1\0" {
        return false;
    }
    let mut pos = 4usize;
    while pos + 8 <= bytes.len() {
        let tag_bytes = &bytes[pos..pos + 4];
        let len = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        pos += 8;
        if pos + len > bytes.len() {
            return false;
        }
        // Compare tag (ASCII). Unknowns ignored.
        if let Ok(tag_str) = core::str::from_utf8(tag_bytes) {
            if tag_str == tag {
                return true;
            }
        }
        pos += len;
    }
    false
}

fn needs_export_sanitization(meta: &AttachmentMeta) -> bool {
    meta.provenance
        .as_ref()
        .map_or(true, |prov| prov.sanitizer.archive_depth > 0)
}

/// GET /v1/zk/attachments/{id} — return the stored attachment bytes.
pub async fn handle_get_attachment(AxumPath(id): AxumPath<String>) -> impl IntoResponse {
    let Some(clean) = sanitize_attachment_id(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            "invalid attachment id (expected 64 hex characters)",
        )
            .into_response();
    };
    let Some(meta) = load_meta(&clean) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(bytes) = fs::read(bin_path(&clean)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !needs_export_sanitization(&meta) {
        return axum::response::Response::builder()
            .header(axum::http::header::CONTENT_TYPE, meta.content_type)
            .body(axum::body::Body::from(bytes))
            .unwrap();
    }
    let sanitize_result = sanitize_attachment(
        Some(meta.content_type.clone()),
        axum::body::Bytes::from(bytes),
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
pub async fn handle_delete_attachment(AxumPath(id): AxumPath<String>) -> impl IntoResponse {
    let Some(clean) = sanitize_attachment_id(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            "invalid attachment id (expected 64 hex characters)",
        )
            .into_response();
    };
    let existed = meta_path(&clean).exists() || bin_path(&clean).exists();
    delete_attachment_files(&clean);
    if existed {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Start a background GC worker that removes expired attachments.
pub fn start_gc_worker() {
    ensure_dirs();
    tokio::spawn(async move {
        let ttl = Duration::from_secs(ttl_secs_cfg());
        let interval = Duration::from_secs(GC_INTERVAL_SECS);
        loop {
            let now = SystemTime::now();
            if let Ok(rd) = fs::read_dir(attachments_dir()) {
                for e in rd.flatten() {
                    if let Some(name) = e.file_name().to_str() {
                        if let Some(id) = name.strip_suffix(".json") {
                            if let Some(meta) = load_meta(id) {
                                let meta_time = UNIX_EPOCH + Duration::from_millis(meta.created_ms);
                                if now.duration_since(meta_time).unwrap_or_default() > ttl {
                                    delete_attachment_files(id);
                                }
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
    allowed_mime_types: Vec<String>,
    max_expanded_bytes: u64,
    max_archive_depth: u32,
    sanitizer_mode: AttachmentSanitizerMode,
    sanitize_timeout_ms: u64,
    sanitizer_exe_override: Option<PathBuf>,
    telemetry: MaybeTelemetry,
}

static ATTACH_CFG: OnceLock<AttachConfig> = OnceLock::new();
static ATTACH_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

/// Configure attachments TTL, per-item size cap, and per-tenant quotas from Torii config.
/// The sanitizer executable override is intended for tests and tooling.
#[allow(clippy::too_many_arguments)]
pub fn configure(
    ttl_secs: u64,
    max_bytes: u64,
    per_tenant_max_count: u64,
    per_tenant_max_bytes: u64,
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
    let _ = ATTACH_CFG.set(AttachConfig {
        ttl_secs,
        max_bytes,
        per_tenant_max_count,
        per_tenant_max_bytes,
        allowed_mime_types,
        max_expanded_bytes,
        max_archive_depth,
        sanitizer_mode,
        sanitize_timeout_ms,
        sanitizer_exe_override,
        telemetry,
    });
}

fn max_bytes_cfg() -> usize {
    ATTACH_CFG
        .get()
        .map(|c| c.max_bytes as usize)
        .unwrap_or(MAX_ATTACHMENT_BYTES_FALLBACK)
}

fn ttl_secs_cfg() -> u64 {
    ATTACH_CFG
        .get()
        .map(|c| c.ttl_secs)
        .unwrap_or(ATTACHMENT_TTL_SECS_FALLBACK)
}

fn per_tenant_max_count_cfg() -> u64 {
    ATTACH_CFG
        .get()
        .map(|c| c.per_tenant_max_count)
        .unwrap_or(0)
}

fn per_tenant_max_bytes_cfg() -> u64 {
    ATTACH_CFG
        .get()
        .map(|c| c.per_tenant_max_bytes)
        .unwrap_or(0)
}

fn allowed_mime_types_cfg() -> Vec<String> {
    let raw = ATTACH_CFG
        .get()
        .map(|c| c.allowed_mime_types.clone())
        .unwrap_or_else(iroha_config::parameters::defaults::torii::attachments_allowed_mime_types);
    raw.into_iter()
        .filter_map(|entry| normalize_mime(&entry))
        .collect()
}

fn max_expanded_bytes_cfg() -> u64 {
    ATTACH_CFG
        .get()
        .map(|c| c.max_expanded_bytes)
        .unwrap_or(iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_EXPANDED_BYTES)
}

fn max_archive_depth_cfg() -> u32 {
    ATTACH_CFG
        .get()
        .map(|c| c.max_archive_depth)
        .unwrap_or(iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_ARCHIVE_DEPTH)
}

fn sanitizer_mode_cfg() -> AttachmentSanitizerMode {
    ATTACH_CFG
        .get()
        .map(|c| c.sanitizer_mode)
        .unwrap_or(AttachmentSanitizerMode::Subprocess)
}

fn sanitize_timeout_cfg() -> Duration {
    let ms = ATTACH_CFG
        .get()
        .map(|c| c.sanitize_timeout_ms)
        .unwrap_or(iroha_config::parameters::defaults::torii::ATTACHMENTS_SANITIZE_TIMEOUT_MS)
        .max(1);
    Duration::from_millis(ms)
}

/// Run the attachment sanitizer process if requested via environment.
pub fn sanitizer_process_exit_code_from_env() -> Option<i32> {
    env::var_os(ATTACHMENT_SANITIZER_ENV)?;
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
    let max_input = env::var(ATTACHMENT_SANITIZER_MAX_INPUT_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(MAX_ATTACHMENT_BYTES_FALLBACK.saturating_mul(4));
    let payload = read_stdin_limited(max_input)?;
    let archived = match norito::from_bytes::<SanitizerRequest>(&payload) {
        Ok(request) => request,
        Err(err) => {
            let response = SanitizerResponse {
                ok: false,
                summary: None,
                sanitized_body: None,
                error: Some(
                    SanitizeError::new(
                        SanitizeRejectReason::Sandbox,
                        format!("attachment sanitizer request decode failed: {err}"),
                    )
                    .into_wire(),
                ),
            };
            return write_sanitizer_response(&response);
        }
    };
    let request: SanitizerRequest = norito_core::NoritoDeserialize::deserialize(archived);
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
    if let Err(err) = apply_sanitizer_limits(cfg.max_expanded_bytes, cfg.timeout) {
        debug!(
            error = %err.message,
            "attachment sanitizer resource limits unavailable"
        );
    }
    let response =
        match sanitize_attachment_sync(request.declared_type.as_deref(), &request.body, &cfg) {
            Ok(mut outcome) => {
                outcome.summary.sandboxed = true;
                SanitizerResponse {
                    ok: true,
                    summary: Some(outcome.summary),
                    sanitized_body: Some(outcome.sanitized_body),
                    error: None,
                }
            }
            Err(err) => SanitizerResponse {
                ok: false,
                summary: None,
                sanitized_body: None,
                error: Some(err.into_wire()),
            },
        };
    write_sanitizer_response(&response)
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
    let bytes = norito::to_bytes(response).map_err(|err| {
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
    ATTACH_CFG
        .get()
        .map(|c| c.telemetry.clone())
        .unwrap_or_else(MaybeTelemetry::disabled)
}

fn quota_lock() -> &'static Mutex<()> {
    ATTACH_MUTEX.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use axum::http::HeaderMap;
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use flate2::{Compression, write::GzEncoder};
    use http_body_util::BodyExt as _;
    use iroha_crypto::Hash;
    use std::{io::Write as _, sync::Once};

    use axum::{http::StatusCode, response::IntoResponse};

    use super::{
        AttachmentHashes, AttachmentMeta, AttachmentProvenance, AttachmentSanitizerMode,
        AttachmentSanitizerVerdict, SanitizeRejectReason, SanitizerConfig, json,
        sanitize_attachment_id, sanitize_attachment_sync,
    };

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

    #[test]
    fn attachment_meta_norito_roundtrip() {
        let meta = AttachmentMeta {
            id: "deadbeef".repeat(4),
            content_type: "application/json".to_string(),
            size: 512,
            created_ms: 1_700_000_000_000,
            tenant: Some("auth:tenant".to_string()),
            provenance: None,
        };

        let encoded = json::to_json_pretty(&meta).expect("serialize metadata");
        let decoded: AttachmentMeta = json::from_json(&encoded).expect("deserialize metadata");

        assert_eq!(meta, decoded);
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
        let response = super::handle_get_attachment(axum::extract::Path("../bad".to_string()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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
    fn sanitizer_executable_defaults_to_current_exe() {
        let resolved = super::sanitizer_executable_with_override(None).expect("current exe");
        let current = std::env::current_exe().expect("current exe");
        assert_eq!(resolved, current);
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
    fn needs_export_sanitization_flags_missing_or_nested() {
        let base = AttachmentMeta {
            id: "deadbeef".repeat(4),
            content_type: super::JSON_MIME_TYPE.to_string(),
            size: 8,
            created_ms: 1_700_000_000_000,
            tenant: None,
            provenance: None,
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
            axum::http::HeaderValue::from_static("text/json"),
        );
        let body = axum::body::Bytes::from_static(br#"{"hello":"world"}"#);
        let response = super::handle_post_attachment(headers, body)
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
        let response = super::handle_post_attachment(headers, axum::body::Bytes::from(compressed))
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

        let response = super::handle_get_attachment(axum::extract::Path(meta.id.clone()))
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
