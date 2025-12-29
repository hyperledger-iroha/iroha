//! Minimal ZK attachments store for the app-facing API.
//!
//! Feature-gated behind `app_api`:
//! - Stores attachments (proof envelopes or JSON DTOs) under `./storage/torii/zk_attachments/`.
//!   Base directory is configured via `torii.data_dir`; tests may use `data_dir::OverrideGuard`.
//! - Deterministic id: Blake2b-32 of the raw request body bytes (lowercase hex).
//! - Endpoints:
//!   - POST `/v1/zk/attachments` – store attachment, returns metadata `{ id, size, content_type, created_ms }`.
//!   - GET  `/v1/zk/attachments` – list metadata for stored attachments.
//!   - GET  `/v1/zk/attachments/{id}` – fetch raw attachment by id.
//!   - DELETE `/v1/zk/attachments/{id}` – delete stored attachment and its metadata.
//! - A background GC task periodically deletes entries older than a TTL;
//!   TTL and size caps are provided via `iroha_config` (Torii).

use std::{
    fs,
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{extract::Path as AxumPath, http::StatusCode, response::IntoResponse};
use std::sync::OnceLock;
use tokio::sync::Mutex;

use crate::{NoritoJson, NoritoQuery};
use iroha_logger::prelude::*;
use norito::json;

const MAX_ATTACHMENT_BYTES_FALLBACK: usize = 4 * 1024 * 1024; // fallback 4 MiB
const ATTACHMENT_TTL_SECS_FALLBACK: u64 = 7 * 24 * 60 * 60; // fallback 7 days
const GC_INTERVAL_SECS: u64 = 60; // run every minute
const DEFAULT_TENANT: &str = "anon";
const ATTACHMENT_ID_HEX_LEN: usize = 64;

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
    /// Deterministic id (hex of Blake2b-32 over raw body bytes).
    pub id: String,
    /// Content type as provided by the client (e.g., application/json).
    pub content_type: String,
    /// Size of the attachment in bytes.
    pub size: u64,
    /// Unix time in milliseconds when the attachment was created.
    pub created_ms: u64,
    /// Hashed tenant identity used for quota enforcement.
    pub tenant: Option<String>,
}

pub(crate) fn base_dir() -> PathBuf {
    crate::data_dir::base_dir()
}

fn attachments_dir() -> PathBuf {
    base_dir().join("zk_attachments")
}

fn ensure_dirs() {
    let _ = fs::create_dir_all(attachments_dir());
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
    let per_tenant_max_bytes = per_tenant_max_bytes_cfg();
    if per_tenant_max_bytes > 0 && (body.len() as u64) > per_tenant_max_bytes {
        warn!(
            tenant = %tenant_key,
            limit_bytes = per_tenant_max_bytes,
            body_bytes = body.len(),
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
    let _guard = quota_lock().lock().await;
    if !enforce_per_tenant_quota(&tenant_key, body.len() as u64) {
        warn!(
            tenant = %tenant_key,
            body_bytes = body.len(),
            "rejecting attachment: per-tenant quota exceeded"
        );
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            "per-tenant attachment quota exceeded".to_string(),
        )
            .into_response();
    }
    // Compute deterministic id
    let id = {
        let h = iroha_crypto::Hash::new(&body);
        hex::encode::<[u8; 32]>(h.into())
    };
    let ct = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let meta = AttachmentMeta {
        id: id.clone(),
        content_type: ct,
        size: body.len() as u64,
        created_ms: now_ms(),
        tenant: Some(tenant_key),
    };
    if let Err(e) = persist_body(&id, &body) {
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
    // Back-compat: no filters, full metadata list
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

/// GET /v1/zk/attachments/{id} — return the raw attachment bytes.
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
    match fs::read(bin_path(&clean)) {
        Ok(bytes) => axum::response::Response::builder()
            .header(axum::http::header::CONTENT_TYPE, meta.content_type)
            .body(axum::body::Body::from(bytes))
            .unwrap(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
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
#[derive(Debug, Clone, Copy)]
struct AttachConfig {
    ttl_secs: u64,
    max_bytes: u64,
    per_tenant_max_count: u64,
    per_tenant_max_bytes: u64,
}

static ATTACH_CFG: OnceLock<AttachConfig> = OnceLock::new();
static ATTACH_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

/// Configure attachments TTL, per-item size cap, and per-tenant quotas from Torii config.
pub fn configure(
    ttl_secs: u64,
    max_bytes: u64,
    per_tenant_max_count: u64,
    per_tenant_max_bytes: u64,
) {
    let _ = ATTACH_CFG.set(AttachConfig {
        ttl_secs,
        max_bytes,
        per_tenant_max_count,
        per_tenant_max_bytes,
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

fn quota_lock() -> &'static Mutex<()> {
    ATTACH_MUTEX.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use super::{AttachmentMeta, json, sanitize_attachment_id};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    #[test]
    fn attachment_meta_norito_roundtrip() {
        let meta = AttachmentMeta {
            id: "deadbeef".repeat(4),
            content_type: "application/json".to_string(),
            size: 512,
            created_ms: 1_700_000_000_000,
            tenant: Some("auth:tenant".to_string()),
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
}
