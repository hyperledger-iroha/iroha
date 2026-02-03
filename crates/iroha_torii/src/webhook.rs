//! Minimal webhook registry for the app-facing API with disk persistence and
//! a background delivery worker.
//!
//! Feature-gated behind `app_api`:
//! - Stores webhooks in-memory, persisted to `./storage/torii/webhooks.json` by default.
//!   Base directory is configured via `torii.data_dir`; tests may use `data_dir::OverrideGuard`.
//! - Exposes CRUD endpoints to create/list/delete webhooks.
//! - Background worker scans a disk-backed queue and delivers payloads with
//!   optional HMAC-SHA256 signature and exponential backoff retries.
//! - HTTPS delivery is supported when the `app_api_https` feature is enabled,
//!   using `hyper` + `rustls` with WebPKI roots. Otherwise, only `http://` is allowed.
//!
//! Endpoints (wired in `lib.rs` when `app_api` is enabled):
//! - POST `/v1/webhooks` – Create a webhook.
//! - GET  `/v1/webhooks` – List webhooks.
//! - DELETE `/v1/webhooks/{id}` – Delete a webhook by id.

use core::{convert::TryFrom, str::FromStr};
#[cfg(test)]
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use std::{
    collections::HashMap,
    fs,
    io::{Read as _, Write as _},
    num::{NonZeroU32, NonZeroUsize},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{extract::Path as AxumPath, http::StatusCode, response::IntoResponse};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_config::parameters::defaults;
use iroha_data_model::{
    events::data::prelude as df,
    nexus::{DataSpaceId, LaneId},
    prelude::DataEvent,
};
use sha2::{Digest, Sha256};
use tokio::fs as tokio_fs;

use crate::filter::filter_expr_to_value;

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
)]
pub struct WebhookCreate {
    pub url: String,
    pub secret: Option<String>,
    pub active: bool,
    /// Optional filter to match events for this webhook.
    /// Uses the same JSON DSL as app-facing APIs (see `crate::filter::FilterExpr`).
    pub filter: Option<crate::filter::FilterExpr>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
)]
pub struct WebhookEntry {
    pub id: u64,
    pub url: String,
    pub active: bool,
    pub secret: Option<String>,
    pub filter: Option<crate::filter::FilterExpr>,
}

#[allow(dead_code, unused)]
fn default_active() -> bool {
    true
}

#[derive(Default)]
struct RegistryInner {
    next_id: u64,
    items: HashMap<u64, WebhookEntry>,
}

fn registry() -> &'static Mutex<RegistryInner> {
    static REG: OnceLock<Mutex<RegistryInner>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(RegistryInner::default()))
}

fn data_dir() -> PathBuf {
    crate::data_dir::base_dir()
}

fn registry_path() -> PathBuf {
    data_dir().join("webhooks.json")
}

fn queue_dir() -> PathBuf {
    data_dir().join("queue")
}

fn queue_depth() -> usize {
    match fs::read_dir(queue_dir()) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext == "json")
            })
            .count(),
        Err(err) => {
            iroha_logger::warn!(%err, "failed to read webhook queue directory");
            0
        }
    }
}

fn available_queue_slots(policy: WebhookPolicy) -> usize {
    let used = queue_depth();
    policy.queue_capacity.get().saturating_sub(used)
}

fn persist_pending_delivery(pd: &PendingDelivery) -> std::io::Result<()> {
    let path = queue_dir().join(format!("{}.json", pd.id));
    let mut payload = norito::json::Map::new();
    payload.insert("id".into(), norito::json::Value::from(pd.id.clone()));
    payload.insert(
        "webhook_id".into(),
        norito::json::Value::from(pd.webhook_id),
    );
    payload.insert("url".into(), norito::json::Value::from(pd.url.clone()));
    payload.insert(
        "content_type".into(),
        norito::json::Value::from(pd.content_type.clone()),
    );
    payload.insert(
        "body".into(),
        norito::json::Value::from(STANDARD.encode(&pd.body)),
    );
    payload.insert(
        "attempts".into(),
        norito::json::Value::from(pd.attempts as u64),
    );
    payload.insert(
        "next_attempt_ms".into(),
        norito::json::Value::from(pd.next_attempt_ms),
    );
    let s = norito::json::to_json_pretty(&payload).unwrap_or_else(|_| "{}".into());
    let mut tmp = tempfile::NamedTempFile::new_in(queue_dir())?;
    tmp.write_all(s.as_bytes())?;
    tmp.flush()?;
    tmp.persist(path)?;
    Ok(())
}

fn proof_id_from_json(value: &norito::json::Value) -> Option<iroha_data_model::proof::ProofId> {
    use iroha_data_model::proof::ProofId;

    match value {
        norito::json::Value::String(s) => ProofId::from_str(s).ok(),
        norito::json::Value::Object(map) => {
            let backend = map.get("backend")?.as_str()?;
            let hash_value = map.get("proof_hash")?;
            if let Some(hex) = hash_value.as_str() {
                let combined = format!("{backend}:{hex}");
                ProofId::from_str(&combined).ok()
            } else if let Some(arr) = hash_value.as_array() {
                if arr.len() != 32 {
                    return None;
                }
                let mut bytes = [0u8; 32];
                for (idx, item) in arr.iter().enumerate() {
                    let raw = item.as_u64()?;
                    let byte = u8::try_from(raw).ok()?;
                    bytes[idx] = byte;
                }
                Some(ProofId {
                    backend: backend.into(),
                    proof_hash: bytes,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HttpTimeoutConfig {
    pub connect: Duration,
    pub write: Duration,
    pub read: Duration,
}

impl Default for HttpTimeoutConfig {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            write: Duration::from_secs(10),
            read: Duration::from_secs(10),
        }
    }
}

fn http_timeout_state() -> &'static Mutex<HttpTimeoutConfig> {
    static STATE: OnceLock<Mutex<HttpTimeoutConfig>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(HttpTimeoutConfig::default()))
}

pub fn http_timeout_config() -> HttpTimeoutConfig {
    *http_timeout_state()
        .lock()
        .expect("http timeout config lock")
}

pub fn set_http_timeout_config(config: HttpTimeoutConfig) {
    *http_timeout_state()
        .lock()
        .expect("http timeout config lock") = config;
}

#[derive(Clone, Copy, Debug)]
pub struct WebhookPolicy {
    pub queue_capacity: NonZeroUsize,
    pub max_attempts: NonZeroU32,
    pub backoff_initial: Duration,
    pub backoff_max: Duration,
    pub connect_timeout: Duration,
    pub write_timeout: Duration,
    pub read_timeout: Duration,
}

impl Default for WebhookPolicy {
    fn default() -> Self {
        Self {
            queue_capacity: NonZeroUsize::new(defaults::torii::WEBHOOK_QUEUE_CAPACITY)
                .expect("default webhook queue capacity is non-zero"),
            max_attempts: NonZeroU32::new(defaults::torii::WEBHOOK_MAX_ATTEMPTS)
                .expect("default webhook max attempts is non-zero"),
            backoff_initial: Duration::from_millis(defaults::torii::WEBHOOK_BACKOFF_INITIAL_MS),
            backoff_max: Duration::from_millis(defaults::torii::WEBHOOK_BACKOFF_MAX_MS),
            connect_timeout: Duration::from_millis(defaults::torii::WEBHOOK_CONNECT_TIMEOUT_MS),
            write_timeout: Duration::from_millis(defaults::torii::WEBHOOK_WRITE_TIMEOUT_MS),
            read_timeout: Duration::from_millis(defaults::torii::WEBHOOK_READ_TIMEOUT_MS),
        }
    }
}

fn webhook_policy_state() -> &'static Mutex<WebhookPolicy> {
    static STATE: OnceLock<Mutex<WebhookPolicy>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(WebhookPolicy::default()))
}

fn webhook_policy() -> WebhookPolicy {
    *webhook_policy_state().lock().expect("webhook policy lock")
}

pub fn set_webhook_policy(policy: WebhookPolicy) {
    *webhook_policy_state().lock().expect("webhook policy lock") = policy;
    set_http_timeout_config(HttpTimeoutConfig {
        connect: policy.connect_timeout,
        write: policy.write_timeout,
        read: policy.read_timeout,
    });
}

#[cfg(test)]
type HttpPostOverrideFn =
    dyn Fn(&str, &[(&str, String)], &[u8]) -> std::io::Result<u16> + Send + Sync;

#[cfg(test)]
fn http_post_override_slot() -> &'static Mutex<Option<Arc<HttpPostOverrideFn>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<HttpPostOverrideFn>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
fn http_post_override_handler() -> Option<Arc<HttpPostOverrideFn>> {
    http_post_override_slot()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().cloned())
}

#[cfg(test)]
#[must_use]
pub struct HttpPostOverrideGuard;

#[cfg(test)]
impl Drop for HttpPostOverrideGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = http_post_override_slot().lock() {
            *guard = None;
        }
    }
}

#[cfg(test)]
pub fn install_http_post_override<F>(handler: F) -> HttpPostOverrideGuard
where
    F: Fn(&str, &[(&str, String)], &[u8]) -> std::io::Result<u16> + Send + Sync + 'static,
{
    let mut guard = http_post_override_slot()
        .lock()
        .expect("http post override lock");
    assert!(guard.is_none(), "test http post override already installed");
    *guard = Some(Arc::new(handler));
    HttpPostOverrideGuard
}

fn ensure_dirs() {
    if cfg!(test) {
        let _ = fs::create_dir_all(queue_dir());
        return;
    }
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = fs::create_dir_all(queue_dir());
    });
}

fn persist_registry() {
    let path = registry_path();
    ensure_dirs();
    if let Ok(mut tmp) =
        tempfile::NamedTempFile::new_in(path.parent().unwrap_or_else(|| Path::new(".")))
    {
        if let Ok(guard) = registry().lock() {
            let mut arr = Vec::new();
            for (_, e) in guard.items.iter() {
                let mut m = norito::json::Map::new();
                m.insert("id".into(), norito::json::Value::from(e.id));
                m.insert("url".into(), norito::json::Value::from(e.url.clone()));
                m.insert("active".into(), norito::json::Value::from(e.active));
                if let Some(ref s) = e.secret {
                    m.insert("secret".into(), norito::json::Value::from(s.clone()));
                } else {
                    m.insert("secret".into(), norito::json::Value::Null);
                }
                if let Some(ref expr) = e.filter {
                    m.insert("filter".into(), filter_expr_to_value(expr));
                } else {
                    m.insert("filter".into(), norito::json::Value::Null);
                }
                arr.push(norito::json::Value::Object(m));
            }
            let body = norito::json::to_json_pretty(&norito::json::Value::Array(arr))
                .unwrap_or_else(|_| "[]".into());
            let _ = tmp.write_all(body.as_bytes());
            let _ = tmp.flush();
            if let Err(e) = tmp.persist(&path) {
                iroha_logger::warn!(%e, "failed to persist webhook registry");
            }
        }
    }
}

fn load_registry() {
    let path = registry_path();
    if let Ok(mut f) = fs::File::open(&path) {
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_ok() {
            if let Ok(s) = String::from_utf8(buf) {
                if let Ok(norito::json::Value::Array(arr)) =
                    norito::json::from_str::<norito::json::Value>(&s)
                {
                    let mut guard = registry().lock().expect("poisoned");
                    guard.items.clear();
                    let mut max_id = 0u64;
                    for v in arr {
                        if let norito::json::Value::Object(m) = v {
                            if let (Some(idv), Some(urlv), Some(activev)) = (
                                m.get("id").and_then(norito::json::Value::as_u64),
                                m.get("url")
                                    .and_then(norito::json::Value::as_str)
                                    .map(ToString::to_string),
                                m.get("active").and_then(|v| match v {
                                    norito::json::Value::Bool(b) => Some(*b),
                                    _ => None,
                                }),
                            ) {
                                let secret = m
                                    .get("secret")
                                    .and_then(norito::json::Value::as_str)
                                    .map(ToString::to_string);
                                let entry = WebhookEntry {
                                    id: idv,
                                    url: urlv,
                                    active: activev,
                                    secret,
                                    filter: m.get("filter").and_then(value_to_filter_expr),
                                };
                                max_id = max_id.max(idv);
                                guard.items.insert(idv, entry);
                            }
                        }
                    }
                    guard.next_id = max_id;
                }
            }
        }
    }
}

/// Initialize persistence: create data dir and load registry from disk.
pub fn init_persistence() {
    ensure_dirs();
    load_registry();
}

fn webhook_entry_to_public_json(entry: &WebhookEntry) -> norito::json::Value {
    let mut m = norito::json::Map::new();
    m.insert("id".into(), norito::json::Value::from(entry.id));
    m.insert("url".into(), norito::json::Value::from(entry.url.clone()));
    m.insert("active".into(), norito::json::Value::from(entry.active));
    m.insert(
        "has_secret".into(),
        norito::json::Value::from(entry.secret.is_some()),
    );
    if let Some(ref expr) = entry.filter {
        m.insert("filter".into(), filter_expr_to_value(expr));
    } else {
        m.insert("filter".into(), norito::json::Value::Null);
    }
    norito::json::Value::Object(m)
}

/// POST /v1/webhooks – create a webhook entry.
pub async fn handle_create_webhook(
    crate::utils::extractors::JsonOnly(req): crate::utils::extractors::JsonOnly<WebhookCreate>,
) -> axum::response::Response {
    if let Some(ref expr) = req.filter {
        if let Err(e) = crate::filter::validate_filter(expr) {
            return (StatusCode::BAD_REQUEST, format!("invalid filter: {e}")).into_response();
        }
    }
    let mut guard = registry().lock().expect("poisoned");
    guard.next_id += 1;
    let id = guard.next_id;
    let entry = WebhookEntry {
        id,
        url: req.url,
        active: req.active,
        secret: req.secret,
        filter: req.filter,
    };
    guard.items.insert(id, entry.clone());
    drop(guard);
    persist_registry();
    // Build Norito JSON response
    let body = norito::json::to_json_pretty(&webhook_entry_to_public_json(&entry))
        .unwrap_or_else(|_| "{}".into());
    (StatusCode::CREATED, body).into_response()
}

/// GET /v1/webhooks – list current webhook entries.
pub async fn handle_list_webhooks() -> impl IntoResponse {
    let guard = registry().lock().expect("poisoned");
    let mut entries: Vec<_> = guard.items.values().cloned().collect();
    entries.sort_by_key(|w| w.id);
    let mut arr = Vec::with_capacity(entries.len());
    for e in entries {
        arr.push(webhook_entry_to_public_json(&e));
    }
    let body = norito::json::to_json_pretty(&norito::json::Value::Array(arr))
        .unwrap_or_else(|_| "[]".into());
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap()
}

/// DELETE /v1/webhooks/{id} – delete a webhook.
pub async fn handle_delete_webhook(AxumPath(id): AxumPath<u64>) -> impl IntoResponse {
    let mut guard = registry().lock().expect("poisoned");
    let removed = guard.items.remove(&id).is_some();
    drop(guard);
    if removed {
        persist_registry();
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

/// Compute HMAC-SHA256 of `body` with `secret` and return lowercase hex string.
fn hmac_sha256_hex(secret: &[u8], body: &[u8]) -> String {
    const BLOCK: usize = 64; // Sha256 block size
    let mut key = [0u8; BLOCK];
    if secret.len() > BLOCK {
        let digest = Sha256::digest(secret);
        key[..32].copy_from_slice(&digest);
    } else {
        key[..secret.len()].copy_from_slice(secret);
    }
    let mut o_key_pad = [0u8; BLOCK];
    let mut i_key_pad = [0u8; BLOCK];
    for i in 0..BLOCK {
        o_key_pad[i] = key[i] ^ 0x5c;
        i_key_pad[i] = key[i] ^ 0x36;
    }
    let mut inner = Sha256::new();
    inner.update(&i_key_pad);
    inner.update(body);
    let inner_sum = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(&o_key_pad);
    outer.update(&inner_sum);
    let mac = outer.finalize();
    hex::encode(mac)
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
)]
struct PendingDelivery {
    id: String,
    webhook_id: u64,
    url: String,
    content_type: String,
    body: Vec<u8>,
    attempts: u32,
    next_attempt_ms: u64,
}

#[allow(dead_code, unused)]
pub fn enqueue_delivery_for_all(body: Vec<u8>, content_type: &str) {
    ensure_dirs();
    let policy = webhook_policy();
    let mut remaining = available_queue_slots(policy);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let guard = match registry().lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    for (id, w) in guard.items.iter() {
        if !w.active {
            continue;
        }
        if remaining == 0 {
            iroha_logger::warn!(
                capacity = policy.queue_capacity.get(),
                "webhook queue at capacity; dropping new deliveries"
            );
            break;
        }
        let pd = PendingDelivery {
            id: format!("{}-{}", id, now),
            webhook_id: *id,
            url: w.url.clone(),
            content_type: content_type.to_string(),
            body: body.clone(),
            attempts: 0,
            next_attempt_ms: now,
        };
        if let Err(err) = persist_pending_delivery(&pd) {
            iroha_logger::warn!(%err, "failed to persist webhook payload");
            continue;
        }
        remaining = remaining.saturating_sub(1);
    }
}

pub fn enqueue_event_for_matching_webhooks(
    event: &iroha_data_model::events::EventBox,
    content_type: &str,
) {
    ensure_dirs();
    let policy = webhook_policy();
    let mut remaining = available_queue_slots(policy);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Snapshot registry to minimize lock duration
    let entries: Vec<(u64, WebhookEntry)> = match registry().lock() {
        Ok(g) => g.items.iter().map(|(k, v)| (*k, v.clone())).collect(),
        Err(_) => return,
    };

    let json_val = crate::routing::event_to_json_value(event);
    let body = match norito::json::to_json(&json_val) {
        Ok(s) => s.into_bytes(),
        Err(e) => {
            iroha_logger::warn!(%e, "failed to serialize event for webhook");
            return;
        }
    };

    for (id, w) in entries {
        if !w.active {
            continue;
        }
        if remaining == 0 {
            iroha_logger::warn!(
                capacity = policy.queue_capacity.get(),
                "webhook queue at capacity; dropping new deliveries"
            );
            break;
        }
        if let Some(ref expr) = w.filter {
            let (proof_backend, proof_call_hash, proof_envelope_hash) = parse_proof_filters(expr);
            let has_proof_filters = crate::proof_filters::has_any_proof_filters(
                proof_backend.as_ref(),
                proof_call_hash.as_ref(),
                proof_envelope_hash.as_ref(),
            );
            let only_proof_filters = has_proof_filters && expr_contains_only_proof_filters(expr);
            if !event_matches_filter(event, expr) && !only_proof_filters {
                continue;
            }
            if has_proof_filters
                && !crate::proof_filters::event_matches_proof_filters(
                    event,
                    proof_backend.as_ref(),
                    proof_call_hash.as_ref(),
                    proof_envelope_hash.as_ref(),
                    only_proof_filters,
                )
            {
                continue;
            }
        }
        let pd = PendingDelivery {
            id: format!("{}-{}", id, now),
            webhook_id: id,
            url: w.url.clone(),
            content_type: content_type.to_string(),
            body: body.clone(),
            attempts: 0,
            next_attempt_ms: now,
        };
        if let Err(err) = persist_pending_delivery(&pd) {
            iroha_logger::warn!(%err, "failed to persist webhook payload");
            continue;
        }
        remaining = remaining.saturating_sub(1);
    }
}

fn parse_proof_filters(
    expr: &crate::filter::FilterExpr,
) -> (
    Option<Vec<String>>,   // proof_backend
    Option<Vec<[u8; 32]>>, // proof_call_hash
    Option<Vec<[u8; 32]>>, // proof_envelope_hash
) {
    use crate::filter::FilterExpr as F;
    let mut proof_backend: Option<Vec<String>> = None;
    let mut proof_call_hash: Option<Vec<[u8; 32]>> = None;
    let mut proof_envelope_hash: Option<Vec<[u8; 32]>> = None;

    fn walk(
        e: &crate::filter::FilterExpr,
        proof_backend: &mut Option<Vec<String>>,
        proof_call_hash: &mut Option<Vec<[u8; 32]>>,
        proof_envelope_hash: &mut Option<Vec<[u8; 32]>>,
    ) {
        match e {
            F::And(list) | F::Or(list) => {
                for sub in list {
                    walk(sub, proof_backend, proof_call_hash, proof_envelope_hash);
                }
            }
            F::Not(inner) => walk(inner, proof_backend, proof_call_hash, proof_envelope_hash),
            F::Eq(field, val) => {
                if field.0 == "proof_backend" {
                    if let Some(s) = val.as_str() {
                        let v = proof_backend.get_or_insert_with(Vec::new);
                        v.push(s.to_string());
                    }
                } else if field.0 == "proof_call_hash" || field.0 == "proof_envelope_hash" {
                    if let Some(s) = val.as_str() {
                        if s.len() == 64 {
                            if let Ok(bytes) = hex::decode(s) {
                                if bytes.len() == 32 {
                                    let mut arr = [0u8; 32];
                                    arr.copy_from_slice(&bytes);
                                    if field.0 == "proof_call_hash" {
                                        let v = proof_call_hash.get_or_insert_with(Vec::new);
                                        v.push(arr);
                                    } else {
                                        let v = proof_envelope_hash.get_or_insert_with(Vec::new);
                                        v.push(arr);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            F::In(field, vals) => {
                if field.0 == "proof_backend" {
                    for val in vals {
                        if let Some(s) = val.as_str() {
                            let v = proof_backend.get_or_insert_with(Vec::new);
                            v.push(s.to_string());
                        }
                    }
                } else if field.0 == "proof_call_hash" || field.0 == "proof_envelope_hash" {
                    for val in vals {
                        if let Some(s) = val.as_str() {
                            if s.len() == 64 {
                                if let Ok(bytes) = hex::decode(s) {
                                    if bytes.len() == 32 {
                                        let mut arr = [0u8; 32];
                                        arr.copy_from_slice(&bytes);
                                        if field.0 == "proof_call_hash" {
                                            let v = proof_call_hash.get_or_insert_with(Vec::new);
                                            v.push(arr);
                                        } else {
                                            let v =
                                                proof_envelope_hash.get_or_insert_with(Vec::new);
                                            v.push(arr);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    walk(
        expr,
        &mut proof_backend,
        &mut proof_call_hash,
        &mut proof_envelope_hash,
    );
    (proof_backend, proof_call_hash, proof_envelope_hash)
}

fn is_proof_field(name: &str) -> bool {
    matches!(
        name,
        "proof_backend" | "proof_call_hash" | "proof_envelope_hash"
    )
}

fn expr_contains_only_proof_filters(expr: &crate::filter::FilterExpr) -> bool {
    use crate::filter::FilterExpr as F;
    match expr {
        F::And(list) | F::Or(list) => list.iter().all(expr_contains_only_proof_filters),
        F::Not(inner) => expr_contains_only_proof_filters(inner),
        F::Eq(field, _)
        | F::Ne(field, _)
        | F::Lt(field, _)
        | F::Lte(field, _)
        | F::Gt(field, _)
        | F::Gte(field, _)
        | F::Exists(field)
        | F::IsNull(field)
        | F::In(field, _)
        | F::Nin(field, _) => is_proof_field(&field.0),
    }
}

fn event_filter_boxes_from_expr(
    expr: &crate::filter::FilterExpr,
) -> Vec<iroha_data_model::events::EventFilterBox> {
    use std::num::NonZeroU64;

    use iroha_data_model::events::{
        EventFilterBox,
        execute_trigger::prelude::ExecuteTriggerEventFilter,
        pipeline::{BlockEventFilter, BlockStatus, TransactionEventFilter, TransactionStatus},
        time::{ExecutionTime, TimeEventFilter},
        trigger_completed::prelude::{TriggerCompletedEventFilter, TriggerCompletedOutcomeType},
    };

    use crate::filter::FilterExpr as F;

    #[derive(Clone)]
    enum PF {
        Tx(TransactionEventFilter),
        Block(BlockEventFilter),
    }

    fn merge(a: PF, b: PF) -> Option<PF> {
        match (a, b) {
            (PF::Tx(mut x), PF::Tx(y)) => {
                if let Some(st) = y.status() {
                    x = x.for_status(st.clone());
                }
                if let Some(h) = y.block_height {
                    x = x.for_block_height(h);
                }
                if let Some(hash) = y.hash() {
                    x = x.for_hash(hash.clone());
                }
                Some(PF::Tx(x))
            }
            (PF::Block(mut x), PF::Block(y)) => {
                if let Some(st) = y.status() {
                    x = x.for_status(st.clone());
                }
                if let Some(h) = y.height() {
                    x = x.for_height(h);
                }
                Some(PF::Block(x))
            }
            _ => None,
        }
    }

    fn to_event_boxes(pfs: Vec<PF>) -> Vec<EventFilterBox> {
        pfs.into_iter()
            .map(|pf| match pf {
                PF::Tx(f) => EventFilterBox::Pipeline(f.into()),
                PF::Block(f) => EventFilterBox::Pipeline(f.into()),
            })
            .collect()
    }

    fn parse_tx_status(s: &str) -> Option<TransactionStatus> {
        match s {
            "Queued" => Some(TransactionStatus::Queued),
            "Expired" => Some(TransactionStatus::Expired),
            "Approved" => Some(TransactionStatus::Approved),
            "Rejected" => Some(TransactionStatus::Rejected(Box::new(
                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::InternalError("filtered".into()),
                ),
            ))),
            _ => None,
        }
    }

    fn parse_block_status(s: &str) -> Option<BlockStatus> {
        match s {
            "Created" => Some(BlockStatus::Created),
            "Approved" => Some(BlockStatus::Approved),
            "Rejected" => Some(BlockStatus::Rejected(
                iroha_data_model::block::error::BlockRejectionReason::ConsensusBlockRejection,
            )),
            "Committed" => Some(BlockStatus::Committed),
            "Applied" => Some(BlockStatus::Applied),
            _ => None,
        }
    }

    fn build(expr: &crate::filter::FilterExpr) -> Vec<PF> {
        match expr {
            F::Eq(field, value) => match field.0.as_str() {
                // Transaction fields
                "tx_status" => value
                    .as_str()
                    .and_then(parse_tx_status)
                    .map(|st| vec![PF::Tx(TransactionEventFilter::new().for_status(st))])
                    .unwrap_or_default(),
                "tx_hash" => value
                    .as_str()
                    .and_then(|s| {
                        s.parse::<iroha_crypto::HashOf<
                            iroha_data_model::transaction::signed::SignedTransaction,
                        >>()
                        .ok()
                    })
                    .map(|h| vec![PF::Tx(TransactionEventFilter::new().for_hash(h))])
                    .unwrap_or_default(),
                "tx_block_height" => value
                    .as_u64()
                    .and_then(NonZeroU64::new)
                    .map(|h| {
                        vec![PF::Tx(
                            TransactionEventFilter::new().for_block_height(Some(h)),
                        )]
                    })
                    .unwrap_or_default(),
                // Block fields
                "block_status" => value
                    .as_str()
                    .and_then(parse_block_status)
                    .map(|st| vec![PF::Block(BlockEventFilter::new().for_status(st))])
                    .unwrap_or_default(),
                "block_height" => value
                    .as_u64()
                    .and_then(NonZeroU64::new)
                    .map(|h| vec![PF::Block(BlockEventFilter::new().for_height(h))])
                    .unwrap_or_default(),
                _ => Vec::new(),
            },
            F::IsNull(field) if field.0.as_str() == "tx_block_height" => {
                vec![PF::Tx(TransactionEventFilter::new().for_block_height(None))]
            }
            F::In(field, list) if field.0.as_str() == "tx_status" => {
                let mut acc = Vec::new();
                for v in list {
                    if let Some(st) = v.as_str().and_then(parse_tx_status) {
                        acc.push(PF::Tx(TransactionEventFilter::new().for_status(st)));
                    }
                }
                acc
            }
            F::And(children) => {
                let mut acc: Vec<PF> = vec![];
                for c in children {
                    let next = build(c);
                    if acc.is_empty() {
                        acc = next;
                    } else {
                        let mut merged = Vec::new();
                        for a in &acc {
                            for b in &next {
                                if let Some(m) = merge(a.clone(), b.clone()) {
                                    merged.push(m);
                                }
                            }
                        }
                        acc = merged;
                    }
                }
                acc
            }
            F::Or(children) => {
                let mut acc = Vec::new();
                for c in children {
                    acc.extend(build(c));
                }
                acc
            }
            F::Not(inner) => match &**inner {
                F::Eq(f, v) if f.0.as_str() == "tx_status" => {
                    let mut acc = Vec::new();
                    if let Some(target) = v.as_str().and_then(parse_tx_status) {
                        use iroha_data_model::events::pipeline::TransactionStatus as TS;
                        let rejected = TS::Rejected(Box::new(
                            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                iroha_data_model::ValidationFail::InternalError("filtered".into()),
                            ),
                        ));
                        let all = [TS::Queued, TS::Expired, TS::Approved, rejected];
                        for st in all.into_iter() {
                            if core::mem::discriminant(&st) != core::mem::discriminant(&target) {
                                acc.push(PF::Tx(TransactionEventFilter::new().for_status(st)));
                            }
                        }
                    }
                    acc
                }
                F::Eq(f, v) if f.0.as_str() == "block_status" => {
                    let mut acc = Vec::new();
                    if let Some(target) = v.as_str().and_then(parse_block_status) {
                        use iroha_data_model::events::pipeline::BlockStatus as BS;
                        let all = [
                            BS::Created,
                            BS::Approved,
                            BS::Rejected(
                                iroha_data_model::block::error::BlockRejectionReason::ConsensusBlockRejection,
                            ),
                            BS::Committed,
                            BS::Applied,
                        ];
                        for st in all.into_iter() {
                            if core::mem::discriminant(&st) != core::mem::discriminant(&target) {
                                acc.push(PF::Block(BlockEventFilter::new().for_status(st)));
                            }
                        }
                    }
                    acc
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }
    // Map non-pipeline fields to event filters. For AND, merge compatible
    // constraints (id + event set + coarse kind) into a single typed filter.
    // For OR, return a union of child mappings.
    fn map_non_pipeline(expr: &crate::filter::FilterExpr) -> Vec<EventFilterBox> {
        match expr {
            F::Eq(field, value) => match field.0.as_str() {
                // Coarse event kind
                "event_kind" => match value.as_str() {
                    Some("Data" | "AnyData") => {
                        vec![EventFilterBox::Data(df::DataEventFilter::Any)]
                    }
                    Some("ExecuteTrigger") => vec![EventFilterBox::ExecuteTrigger(
                        ExecuteTriggerEventFilter::new(),
                    )],
                    Some("TriggerCompleted") => vec![EventFilterBox::TriggerCompleted(
                        TriggerCompletedEventFilter::new(),
                    )],
                    Some("Time") => vec![EventFilterBox::Time(TimeEventFilter(
                        ExecutionTime::PreCommit,
                    ))],
                    _ => Vec::new(),
                },
                // Data origins
                "peer_id" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|id| {
                        EventFilterBox::Data(df::DataEventFilter::Peer(
                            df::PeerEventFilter::new().for_peer(id),
                        ))
                    })
                    .into_iter()
                    .collect(),
                "domain_id" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|id| {
                        EventFilterBox::Data(df::DataEventFilter::Domain(
                            df::DomainEventFilter::new().for_domain(id),
                        ))
                    })
                    .into_iter()
                    .collect(),
                "account_id" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|id| {
                        EventFilterBox::Data(df::DataEventFilter::Account(
                            df::AccountEventFilter::new().for_account(id),
                        ))
                    })
                    .into_iter()
                    .collect(),
                "asset_id" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|id| {
                        EventFilterBox::Data(df::DataEventFilter::Asset(
                            df::AssetEventFilter::new().for_asset(id),
                        ))
                    })
                    .into_iter()
                    .collect(),
                "asset_definition_id" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|id| {
                        EventFilterBox::Data(df::DataEventFilter::AssetDefinition(
                            df::AssetDefinitionEventFilter::new().for_asset_definition(id),
                        ))
                    })
                    .into_iter()
                    .collect(),
                "nft_id" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|id| {
                        EventFilterBox::Data(df::DataEventFilter::Nft(
                            df::NftEventFilter::new().for_nft(id),
                        ))
                    })
                    .into_iter()
                    .collect(),
                "data_trigger_id" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|id| {
                        EventFilterBox::Data(df::DataEventFilter::Trigger(
                            df::TriggerEventFilter::new().for_trigger(id),
                        ))
                    })
                    .into_iter()
                    .collect(),
                "role_id" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|id| {
                        EventFilterBox::Data(df::DataEventFilter::Role(
                            df::RoleEventFilter::new().for_role(id),
                        ))
                    })
                    .into_iter()
                    .collect(),
                "proof_id" => proof_id_from_json(value)
                    .map(|id| {
                        EventFilterBox::Data(df::DataEventFilter::Proof(
                            df::ProofEventFilter::new().for_proof(id),
                        ))
                    })
                    .into_iter()
                    .collect(),
                // Time events
                "time_precommit" if value.as_bool() == Some(true) => vec![EventFilterBox::Time(
                    TimeEventFilter(ExecutionTime::PreCommit),
                )],
                // Trigger execution
                "execute_trigger_id" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|id| {
                        EventFilterBox::ExecuteTrigger(
                            ExecuteTriggerEventFilter::new().for_trigger(id),
                        )
                    })
                    .into_iter()
                    .collect(),
                "execute_trigger_authority" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|acc: iroha_data_model::account::AccountId| {
                        EventFilterBox::ExecuteTrigger(
                            ExecuteTriggerEventFilter::new().under_authority(acc),
                        )
                    })
                    .into_iter()
                    .collect(),
                // Trigger completed
                "trigger_completed_id" | "trigger_id" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|id| {
                        EventFilterBox::TriggerCompleted(
                            TriggerCompletedEventFilter::new().for_trigger(id),
                        )
                    })
                    .into_iter()
                    .collect(),
                "trigger_completed_outcome" => value
                    .as_str()
                    .and_then(|s| match s {
                        "Success" => Some(TriggerCompletedOutcomeType::Success),
                        "Failure" => Some(TriggerCompletedOutcomeType::Failure),
                        _ => None,
                    })
                    .map(|ty| {
                        EventFilterBox::TriggerCompleted(
                            TriggerCompletedEventFilter::new().for_outcome(ty),
                        )
                    })
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            },
            F::And(children) => {
                // Collect constraints per category and synthesize merged filters
                #[derive(Default)]
                struct C {
                    // Data categories
                    peer_id: Option<iroha_data_model::peer::PeerId>,
                    peer_set: Option<df::PeerEventSet>,
                    domain_id: Option<iroha_data_model::domain::DomainId>,
                    domain_set: Option<df::DomainEventSet>,
                    account_id: Option<iroha_data_model::account::AccountId>,
                    account_set: Option<df::AccountEventSet>,
                    asset_id: Option<iroha_data_model::asset::AssetId>,
                    asset_set: Option<df::AssetEventSet>,
                    asset_def_id: Option<iroha_data_model::asset::AssetDefinitionId>,
                    asset_def_set: Option<df::AssetDefinitionEventSet>,
                    nft_id: Option<iroha_data_model::nft::NftId>,
                    nft_set: Option<df::NftEventSet>,
                    role_id: Option<iroha_data_model::role::RoleId>,
                    role_set: Option<df::RoleEventSet>,
                    proof_id: Option<iroha_data_model::proof::ProofId>,
                    proof_set: Option<iroha_data_model::events::data::proof::ProofEventSet>,
                    cfg_set: Option<df::ConfigurationEventSet>,
                    exec_set: Option<df::ExecutorEventSet>,
                    // Time
                    time_precommit: bool,
                    // ExecuteTrigger / TriggerCompleted
                    exec_trig_id: Option<iroha_data_model::trigger::TriggerId>,
                    exec_trig_auth: Option<iroha_data_model::account::AccountId>,
                    trigc_id: Option<iroha_data_model::trigger::TriggerId>,
                    trigc_outcome: Option<TriggerCompletedOutcomeType>,
                    // coarse kinds
                    want_data_any: bool,
                }

                fn parse_event_list<T>(
                    vals: &norito::json::Value,
                    from_str: &dyn Fn(&str) -> Option<T>,
                ) -> Option<T>
                where
                    T: core::ops::BitOr<Output = T> + Copy,
                {
                    if let Some(s) = vals.as_str() {
                        return from_str(s);
                    }
                    if let Some(arr) = vals.as_array() {
                        let mut acc: Option<T> = None;
                        for v in arr {
                            if let Some(s) = v.as_str() {
                                if let Some(one) = from_str(s) {
                                    acc = Some(acc.map_or(one, |a| a | one));
                                }
                            }
                        }
                        return acc;
                    }
                    None
                }

                fn apply_constraint(c: &mut C, f: &str, v: &norito::json::Value) {
                    match f {
                        // coarse kinds
                        "event_kind" => {
                            if v.as_str().is_some_and(|s| matches!(s, "Data" | "AnyData")) {
                                c.want_data_any = true;
                            } else if v.as_str() == Some("Time") {
                                c.time_precommit = true;
                            }
                        }
                        // data ids
                        "peer_id" => c.peer_id = v.as_str().and_then(|s| s.parse().ok()),
                        "domain_id" => c.domain_id = v.as_str().and_then(|s| s.parse().ok()),
                        "account_id" => c.account_id = v.as_str().and_then(|s| s.parse().ok()),
                        "asset_id" => c.asset_id = v.as_str().and_then(|s| s.parse().ok()),
                        "asset_definition_id" => {
                            c.asset_def_id = v.as_str().and_then(|s| s.parse().ok())
                        }
                        "nft_id" => c.nft_id = v.as_str().and_then(|s| s.parse().ok()),
                        "role_id" => c.role_id = v.as_str().and_then(|s| s.parse().ok()),
                        "proof_id" => {
                            c.proof_id = proof_id_from_json(v);
                        }
                        // data event sets
                        "peer_event" => {
                            c.peer_set = parse_event_list(v, &|s| match s {
                                "Added" => Some(df::PeerEventSet::Added),
                                "Removed" => Some(df::PeerEventSet::Removed),
                                _ => None,
                            });
                        }
                        "domain_event" => {
                            c.domain_set = parse_event_list(v, &|s| match s {
                                "Created" => Some(df::DomainEventSet::Created),
                                "Deleted" => Some(df::DomainEventSet::Deleted),
                                "AssetDefinition" => Some(df::DomainEventSet::AnyAssetDefinition),
                                "Nft" => Some(df::DomainEventSet::AnyNft),
                                "Account" => Some(df::DomainEventSet::AnyAccount),
                                "MetadataInserted" => Some(df::DomainEventSet::MetadataInserted),
                                "MetadataRemoved" => Some(df::DomainEventSet::MetadataRemoved),
                                "OwnerChanged" => Some(df::DomainEventSet::OwnerChanged),
                                _ => None,
                            });
                        }
                        "account_event" => {
                            c.account_set = parse_event_list(v, &|s| match s {
                                "Created" => Some(df::AccountEventSet::Created),
                                "Deleted" => Some(df::AccountEventSet::Deleted),
                                "Asset" => Some(df::AccountEventSet::AnyAsset),
                                "PermissionAdded" => Some(df::AccountEventSet::PermissionAdded),
                                "PermissionRemoved" => Some(df::AccountEventSet::PermissionRemoved),
                                "RoleGranted" => Some(df::AccountEventSet::RoleGranted),
                                "RoleRevoked" => Some(df::AccountEventSet::RoleRevoked),
                                "MetadataInserted" => Some(df::AccountEventSet::MetadataInserted),
                                "MetadataRemoved" => Some(df::AccountEventSet::MetadataRemoved),
                                _ => None,
                            });
                        }
                        "asset_event" => {
                            c.asset_set = parse_event_list(v, &|s| match s {
                                "Created" => Some(df::AssetEventSet::Created),
                                "Deleted" => Some(df::AssetEventSet::Deleted),
                                "Added" => Some(df::AssetEventSet::Added),
                                "Removed" => Some(df::AssetEventSet::Removed),
                                "MetadataInserted" => Some(df::AssetEventSet::MetadataInserted),
                                "MetadataRemoved" => Some(df::AssetEventSet::MetadataRemoved),
                                _ => None,
                            });
                        }
                        "asset_definition_event" => {
                            c.asset_def_set = parse_event_list(v, &|s| match s {
                                "Created" => Some(df::AssetDefinitionEventSet::Created),
                                "Deleted" => Some(df::AssetDefinitionEventSet::Deleted),
                                "MetadataInserted" => {
                                    Some(df::AssetDefinitionEventSet::MetadataInserted)
                                }
                                "MetadataRemoved" => {
                                    Some(df::AssetDefinitionEventSet::MetadataRemoved)
                                }
                                "MintabilityChanged" => {
                                    Some(df::AssetDefinitionEventSet::MintabilityChanged)
                                }
                                "MintabilityChangedDetailed" => {
                                    Some(df::AssetDefinitionEventSet::MintabilityChangedDetailed)
                                }
                                "TotalQuantityChanged" => {
                                    Some(df::AssetDefinitionEventSet::TotalQuantityChanged)
                                }
                                "OwnerChanged" => Some(df::AssetDefinitionEventSet::OwnerChanged),
                                _ => None,
                            });
                        }
                        "nft_event" => {
                            c.nft_set = parse_event_list(v, &|s| match s {
                                "Created" => Some(df::NftEventSet::Created),
                                "Deleted" => Some(df::NftEventSet::Deleted),
                                "OwnerChanged" => Some(df::NftEventSet::OwnerChanged),
                                _ => None,
                            });
                        }
                        "role_event" => {
                            c.role_set = parse_event_list(v, &|s| match s {
                                "Created" => Some(df::RoleEventSet::Created),
                                "Deleted" => Some(df::RoleEventSet::Deleted),
                                "PermissionAdded" => Some(df::RoleEventSet::PermissionAdded),
                                "PermissionRemoved" => Some(df::RoleEventSet::PermissionRemoved),
                                _ => None,
                            });
                        }
                        "configuration_event" => {
                            c.cfg_set = parse_event_list(v, &|s| match s {
                                "Changed" => Some(df::ConfigurationEventSet::Changed),
                                _ => None,
                            });
                        }
                        "executor_event" => {
                            c.exec_set = parse_event_list(v, &|s| match s {
                                "Upgraded" => Some(df::ExecutorEventSet::Upgraded),
                                _ => None,
                            });
                        }
                        // time
                        "time_precommit" => c.time_precommit |= v.as_bool().unwrap_or(false),
                        // execute trigger / trigger completed
                        "execute_trigger_id" => {
                            c.exec_trig_id = v.as_str().and_then(|s| s.parse().ok());
                        }
                        "execute_trigger_authority" => {
                            c.exec_trig_auth = v.as_str().and_then(|s| s.parse().ok());
                        }
                        "trigger_completed_id" | "trigger_id" => {
                            c.trigc_id = v.as_str().and_then(|s| s.parse().ok());
                        }
                        "trigger_completed_outcome" => {
                            c.trigc_outcome = v.as_str().and_then(|s| match s {
                                "Success" => Some(TriggerCompletedOutcomeType::Success),
                                "Failure" => Some(TriggerCompletedOutcomeType::Failure),
                                _ => None,
                            });
                        }
                        _ => {}
                    }
                }

                let mut c = C::default();
                for child in children {
                    match child {
                        F::Eq(f, v) => apply_constraint(&mut c, &f.0, v),
                        F::And(grand) | F::Or(grand) => {
                            // Flatten: apply constraints from nested groups as union for sets
                            for g in grand {
                                if let F::Eq(f, v) = g {
                                    apply_constraint(&mut c, &f.0, v);
                                }
                            }
                        }
                        F::Not(inner) => {
                            if let F::Eq(f, v) = inner.as_ref() {
                                apply_constraint(&mut c, &f.0, v);
                            }
                        }
                        _ => {}
                    }
                }

                let mut out: Vec<EventFilterBox> = Vec::new();
                // Synthesize merged typed filters per category
                if c.want_data_any {
                    out.push(EventFilterBox::Data(df::DataEventFilter::Any));
                }
                if c.peer_id.is_some() || c.peer_set.is_some() {
                    let mut f = df::PeerEventFilter::new();
                    if let Some(id) = c.peer_id {
                        f = f.for_peer(id);
                    }
                    if let Some(set) = c.peer_set {
                        f = f.for_events(set);
                    }
                    out.push(EventFilterBox::Data(df::DataEventFilter::Peer(f)));
                }
                if c.domain_id.is_some() || c.domain_set.is_some() {
                    let mut f = df::DomainEventFilter::new();
                    if let Some(id) = c.domain_id {
                        f = f.for_domain(id);
                    }
                    if let Some(set) = c.domain_set {
                        f = f.for_events(set);
                    }
                    out.push(EventFilterBox::Data(df::DataEventFilter::Domain(f)));
                }
                if c.account_id.is_some() || c.account_set.is_some() {
                    let mut f = df::AccountEventFilter::new();
                    if let Some(id) = c.account_id {
                        f = f.for_account(id);
                    }
                    if let Some(set) = c.account_set {
                        f = f.for_events(set);
                    }
                    out.push(EventFilterBox::Data(df::DataEventFilter::Account(f)));
                }
                if c.asset_id.is_some() || c.asset_set.is_some() {
                    let mut f = df::AssetEventFilter::new();
                    if let Some(id) = c.asset_id {
                        f = f.for_asset(id);
                    }
                    if let Some(set) = c.asset_set {
                        f = f.for_events(set);
                    }
                    out.push(EventFilterBox::Data(df::DataEventFilter::Asset(f)));
                }
                if c.asset_def_id.is_some() || c.asset_def_set.is_some() {
                    let mut f = df::AssetDefinitionEventFilter::new();
                    if let Some(id) = c.asset_def_id {
                        f = f.for_asset_definition(id);
                    }
                    if let Some(set) = c.asset_def_set {
                        f = f.for_events(set);
                    }
                    out.push(EventFilterBox::Data(df::DataEventFilter::AssetDefinition(
                        f,
                    )));
                }
                if c.nft_id.is_some() || c.nft_set.is_some() {
                    let mut f = df::NftEventFilter::new();
                    if let Some(id) = c.nft_id {
                        f = f.for_nft(id);
                    }
                    if let Some(set) = c.nft_set {
                        f = f.for_events(set);
                    }
                    out.push(EventFilterBox::Data(df::DataEventFilter::Nft(f)));
                }
                if c.role_id.is_some() || c.role_set.is_some() {
                    let mut f = df::RoleEventFilter::new();
                    if let Some(id) = c.role_id {
                        f = f.for_role(id);
                    }
                    if let Some(set) = c.role_set {
                        f = f.for_events(set);
                    }
                    out.push(EventFilterBox::Data(df::DataEventFilter::Role(f)));
                }
                if c.proof_id.is_some() || c.proof_set.is_some() {
                    let mut f = df::ProofEventFilter::new();
                    if let Some(id) = c.proof_id {
                        f = f.for_proof(id);
                    }
                    if let Some(set) = c.proof_set {
                        f = f.for_events(set);
                    }
                    out.push(EventFilterBox::Data(df::DataEventFilter::Proof(f)));
                }
                if let Some(set) = c.cfg_set {
                    out.push(EventFilterBox::Data(df::DataEventFilter::Configuration(
                        df::ConfigurationEventFilter::new().for_events(set),
                    )));
                }
                if let Some(set) = c.exec_set {
                    out.push(EventFilterBox::Data(df::DataEventFilter::Executor(
                        df::ExecutorEventFilter::new().for_events(set),
                    )));
                }
                if c.time_precommit {
                    out.push(EventFilterBox::Time(TimeEventFilter(
                        ExecutionTime::PreCommit,
                    )));
                }
                if c.exec_trig_id.is_some() || c.exec_trig_auth.is_some() {
                    let mut f = ExecuteTriggerEventFilter::new();
                    if let Some(id) = c.exec_trig_id {
                        f = f.for_trigger(id);
                    }
                    if let Some(a) = c.exec_trig_auth {
                        f = f.under_authority(a);
                    }
                    out.push(EventFilterBox::ExecuteTrigger(f));
                }
                if c.trigc_id.is_some() || c.trigc_outcome.is_some() {
                    let mut f = TriggerCompletedEventFilter::new();
                    if let Some(id) = c.trigc_id {
                        f = f.for_trigger(id);
                    }
                    if let Some(o) = c.trigc_outcome {
                        f = f.for_outcome(o);
                    }
                    out.push(EventFilterBox::TriggerCompleted(f));
                }
                out
            }
            F::Or(children) => {
                let mut out = Vec::new();
                for c in children {
                    out.extend(map_non_pipeline(c));
                }
                out
            }
            F::Not(inner) => map_non_pipeline(inner),
            _ => Vec::new(),
        }
    }

    let mut out = to_event_boxes(build(expr));
    out.extend(map_non_pipeline(expr));
    out
}

fn event_matches_filter(
    event: &iroha_data_model::events::EventBox,
    expr: &crate::filter::FilterExpr,
) -> bool {
    #[cfg(feature = "transparent_api")]
    {
        use iroha_data_model::events::EventFilter as _;
        let filters = event_filter_boxes_from_expr(expr);
        return filters.iter().any(|f| f.matches(event));
    }
    #[allow(unreachable_code)]
    false
}

fn value_to_filter_expr(v: &norito::json::Value) -> Option<crate::filter::FilterExpr> {
    let s = norito::json::to_json(v).ok()?;
    norito::json::from_str::<crate::filter::FilterExpr>(&s).ok()
}

fn io_timeout_error(operation: &str, duration: Duration) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        format!("{operation} timed out after {:?}", duration),
    )
}

async fn http_post_plain(
    url: &str,
    headers: &[(&str, String)],
    body: &[u8],
) -> std::io::Result<u16> {
    // Very small plain HTTP/1.1 client for http:// (no TLS). For https, fail fast.
    if let Some(rest) = url.strip_prefix("http://") {
        let (host_port, path) = match rest.split_once('/') {
            Some((hp, p)) => (hp, format!("/{}", p)),
            None => (rest, "/".to_string()),
        };
        let (host, port) = match host_port.split_once(':') {
            Some((h, p)) => (h, p.parse::<u16>().unwrap_or(80)),
            None => (host_port, 80),
        };
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpStream,
        };
        let timeouts = http_timeout_config();
        let mut stream =
            match tokio::time::timeout(timeouts.connect, TcpStream::connect((host, port))).await {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err(io_timeout_error("tcp connect", timeouts.connect)),
            };
        let mut req = Vec::new();
        req.extend_from_slice(format!("POST {} HTTP/1.1\r\n", path).as_bytes());
        req.extend_from_slice(format!("Host: {}\r\n", host).as_bytes());
        req.extend_from_slice(b"Connection: close\r\n");
        req.extend_from_slice(b"User-Agent: iroha-torii-webhook/1\r\n");
        for (k, v) in headers {
            req.extend_from_slice(format!("{}: {}\r\n", k, v).as_bytes());
        }
        req.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
        req.extend_from_slice(b"\r\n");
        req.extend_from_slice(body);
        let write_result = tokio::time::timeout(timeouts.write, async {
            stream.write_all(&req).await?;
            stream.flush().await
        })
        .await
        .map_err(|_| io_timeout_error("tcp write", timeouts.write))?;
        write_result?;
        let mut buf = Vec::new();
        let read_result = tokio::time::timeout(timeouts.read, stream.read_to_end(&mut buf))
            .await
            .map_err(|_| io_timeout_error("tcp read", timeouts.read))?;
        read_result?;
        // Parse status code
        if let Some(line) = buf.split(|&b| b == b'\n').next() {
            let line = String::from_utf8_lossy(line);
            if let Some(code_str) = line.split_whitespace().nth(1) {
                if let Ok(code) = code_str.parse::<u16>() {
                    return Ok(code);
                }
            }
        }
        Ok(0)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "HTTPS not supported without TLS client",
        ))
    }
}

#[cfg(feature = "app_api_https")]
async fn http_post_https(
    url: &str,
    headers: &[(&str, String)],
    body: &[u8],
) -> std::io::Result<u16> {
    use std::str::FromStr as _;

    use hyper::{Request, body::Body, http::HeaderName};
    use hyper_rustls::HttpsConnectorBuilder;

    let uri = hyper::Uri::from_str(url).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("bad url: {e}"))
    })?;
    let https = HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_or_http()
        .enable_http1()
        .build();
    let client: hyper::Client<_, Body> = hyper::Client::builder().http1_only(true).build(https);

    let mut req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("User-Agent", "iroha-torii-webhook/1")
        .header("Connection", "close")
        .body(Body::from(body.to_vec()))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("req build: {e}")))?;

    let headers_mut = req.headers_mut();
    for (k, v) in headers {
        if let Ok(name) = HeaderName::from_str(k) {
            headers_mut.insert(name, v.parse().unwrap_or_default());
        }
    }

    let resp = client
        .request(req)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("https req: {e}")))?;
    Ok(resp.status().as_u16())
}

async fn http_post(url: &str, headers: &[(&str, String)], body: &[u8]) -> std::io::Result<u16> {
    #[cfg(test)]
    if let Some(handler) = http_post_override_handler() {
        return handler(url, headers, body);
    }

    if url.starts_with("https://") {
        #[cfg(feature = "app_api_https")]
        {
            return http_post_https(url, headers, body).await;
        }
        #[cfg(not(feature = "app_api_https"))]
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "HTTPS not supported; enable feature app_api_https",
            ));
        }
    }
    #[cfg(feature = "app_api_wss")]
    if url.starts_with("wss://") || url.starts_with("ws://") {
        return ws_send(url, headers, body).await;
    }
    #[cfg(not(feature = "app_api_wss"))]
    if url.starts_with("wss://") || url.starts_with("ws://") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "WS/WSS not supported; enable feature app_api_wss",
        ));
    }
    http_post_plain(url, headers, body).await
}

#[cfg(feature = "app_api_wss")]
async fn ws_send(url: &str, headers: &[(&str, String)], body: &[u8]) -> std::io::Result<u16> {
    use std::str::FromStr;

    use futures::SinkExt as _;
    use tokio_tungstenite::connect_async;
    use tungstenite::{Message, client::IntoClientRequest, http::HeaderName};

    let mut req = url.into_client_request().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("bad url: {e}"))
    })?;
    for (k, v) in headers {
        if let Ok(name) = HeaderName::from_str(k) {
            if let Ok(val) = v.parse() {
                req.headers_mut().insert(name, val);
            }
        }
    }
    let (mut ws, _resp) = connect_async(req)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("ws connect: {e}")))?;
    ws.send(Message::Binary(body.to_vec().into()))
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("ws send: {e}")))?;
    let _ = ws.close(None).await;
    Ok(200)
}

fn backoff_delay(policy: &WebhookPolicy, attempts: u32) -> Duration {
    let base_ms = policy.backoff_initial.as_millis().max(1);
    let max_ms = policy.backoff_max.as_millis().max(base_ms);
    let pow = attempts.saturating_sub(1).min(31);
    let delay_ms = base_ms.saturating_mul(1u128 << pow).min(max_ms);
    Duration::from_millis(delay_ms as u64)
}

async fn try_deliver(pd: &mut PendingDelivery, secret: Option<&str>) -> bool {
    let mut headers = vec![("Content-Type", pd.content_type.clone())];
    if let Some(sec) = secret {
        let sig = hmac_sha256_hex(sec.as_bytes(), &pd.body);
        headers.push(("X-Iroha-Webhook-Signature", format!("sha256={sig}")));
    }
    match http_post(&pd.url, &headers, &pd.body).await {
        Ok(code) if (200..300).contains(&code) => true,
        Ok(code) => {
            iroha_logger::warn!(code, url=%pd.url, "webhook delivery returned non-2xx");
            false
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::TimedOut {
                iroha_logger::warn!(%e, url=%pd.url, "webhook delivery timed out");
            } else {
                iroha_logger::warn!(%e, url=%pd.url, "webhook delivery failed");
            }
            false
        }
    }
}

/// Spawn the background delivery worker. Idempotent.
pub fn start_delivery_worker() {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    ensure_dirs();
    tokio::spawn(async move {
        loop {
            let delay = process_queue_once().await;
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
        }
    });
}

async fn process_queue_once() -> Duration {
    // Scan queue directory
    let mut read_dir = match tokio_fs::read_dir(queue_dir()).await {
        Ok(rd) => rd,
        Err(e) => {
            iroha_logger::warn!(%e, "failed to read webhook queue directory");
            return Duration::from_secs(5);
        }
    };
    let mut entries = Vec::new();
    let mut read_dir_failed = false;
    loop {
        match read_dir.next_entry().await {
            Ok(Some(entry)) => entries.push(entry),
            Ok(None) => break,
            Err(e) => {
                iroha_logger::warn!(%e, "failed to iterate webhook queue directory");
                read_dir_failed = true;
                break;
            }
        }
    }
    if read_dir_failed {
        return Duration::from_secs(5);
    }
    if entries.is_empty() {
        return Duration::from_secs(1);
    }
    entries.sort_by_key(tokio::fs::DirEntry::file_name);
    let policy = webhook_policy();
    for e in entries {
        let path = e.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let bytes = match tokio_fs::read(&path).await {
            Ok(b) => b,
            Err(e) => {
                iroha_logger::warn!(%e, ?path, "failed to read pending webhook delivery");
                continue;
            }
        };
        let mut pd: PendingDelivery = match String::from_utf8(bytes)
            .ok()
            .and_then(|s| norito::json::from_str::<norito::json::Value>(&s).ok())
            .and_then(|v| match v {
                norito::json::Value::Object(m) => {
                    let id = m
                        .get("id")
                        .and_then(norito::json::Value::as_str)
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    let webhook_id = m
                        .get("webhook_id")
                        .and_then(norito::json::Value::as_u64)
                        .unwrap_or(0);
                    let url = m
                        .get("url")
                        .and_then(norito::json::Value::as_str)
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    let content_type = m
                        .get("content_type")
                        .and_then(norito::json::Value::as_str)
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    let body = m
                        .get("body")
                        .and_then(norito::json::Value::as_str)
                        .and_then(|s| STANDARD.decode(s).ok())
                        .unwrap_or_default();
                    let attempts = m
                        .get("attempts")
                        .and_then(norito::json::Value::as_u64)
                        .unwrap_or(0) as u32;
                    let next_attempt_ms = m
                        .get("next_attempt_ms")
                        .and_then(norito::json::Value::as_u64)
                        .unwrap_or(0);
                    Some(PendingDelivery {
                        id,
                        webhook_id,
                        url,
                        content_type,
                        body,
                        attempts,
                        next_attempt_ms,
                    })
                }
                _ => None,
            }) {
            Some(p) => p,
            None => {
                if let Err(e) = tokio_fs::remove_file(&path).await {
                    iroha_logger::warn!(%e, ?path, "failed to remove invalid webhook payload");
                }
                continue;
            }
        };
        // Wait until next_attempt
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if now_ms < pd.next_attempt_ms {
            continue;
        }
        if pd.attempts >= policy.max_attempts.get() {
            iroha_logger::warn!(
                attempts = pd.attempts,
                webhook_id = pd.webhook_id,
                "dropping webhook payload that exceeded max attempts"
            );
            if let Err(e) = tokio_fs::remove_file(&path).await {
                iroha_logger::warn!(%e, ?path, "failed to remove over-attempted webhook payload");
            }
            continue;
        }
        // Lookup secret (if present)
        let secret = registry()
            .lock()
            .ok()
            .and_then(|g| g.items.get(&pd.webhook_id).cloned())
            .and_then(|w| w.secret);
        if try_deliver(&mut pd, secret.as_deref()).await {
            if let Err(e) = tokio_fs::remove_file(&path).await {
                iroha_logger::warn!(%e, ?path, "failed to remove delivered webhook payload");
            }
        } else {
            pd.attempts = pd.attempts.saturating_add(1);
            if pd.attempts >= policy.max_attempts.get() {
                iroha_logger::warn!(
                    attempts = pd.attempts,
                    webhook_id = pd.webhook_id,
                    "dropping webhook payload after max attempts"
                );
                if let Err(e) = tokio_fs::remove_file(&path).await {
                    iroha_logger::warn!(%e, ?path, "failed to remove failed webhook payload");
                }
                continue;
            }
            let delay = backoff_delay(&policy, pd.attempts);
            let next = SystemTime::now()
                .checked_add(delay)
                .unwrap_or_else(SystemTime::now)
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            pd.next_attempt_ms = next;
            let mut m = norito::json::Map::new();
            m.insert("id".into(), norito::json::Value::from(pd.id.clone()));
            m.insert(
                "webhook_id".into(),
                norito::json::Value::from(pd.webhook_id),
            );
            m.insert("url".into(), norito::json::Value::from(pd.url.clone()));
            m.insert(
                "content_type".into(),
                norito::json::Value::from(pd.content_type.clone()),
            );
            m.insert(
                "body".into(),
                norito::json::Value::from(STANDARD.encode(&pd.body)),
            );
            m.insert(
                "attempts".into(),
                norito::json::Value::from(pd.attempts as u64),
            );
            m.insert(
                "next_attempt_ms".into(),
                norito::json::Value::from(pd.next_attempt_ms),
            );
            let s = norito::json::to_json_pretty(&m).unwrap_or_else(|_| "{}".into());
            if let Err(e) = tokio_fs::write(&path, s.as_bytes()).await {
                iroha_logger::warn!(%e, ?path, "failed to persist pending webhook delivery");
            }
        }
    }
    Duration::from_millis(0)
}

#[cfg(test)]
mod tests {
    use std::{
        convert::TryFrom,
        fs,
        sync::{Arc, Mutex},
    };

    use http_body_util::BodyExt as _;
    use iroha_crypto::Hash;
    use iroha_data_model::events::EventFilter; // bring .matches()
    use iroha_data_model::events::{
        EventBox,
        pipeline::{TransactionEvent, TransactionStatus},
    };
    use tokio::{
        runtime::Runtime,
        time::{Duration, sleep},
    };

    use super::*;
    use crate::test_utils::TestDataDirGuard;

    struct TimeoutOverride(super::HttpTimeoutConfig);

    impl TimeoutOverride {
        fn new(config: super::HttpTimeoutConfig) -> Self {
            let previous = super::http_timeout_config();
            super::set_http_timeout_config(config);
            Self(previous)
        }
    }

    impl Drop for TimeoutOverride {
        fn drop(&mut self) {
            super::set_http_timeout_config(self.0);
        }
    }

    struct WebhookPolicyGuard(super::WebhookPolicy);

    impl WebhookPolicyGuard {
        fn new(policy: super::WebhookPolicy) -> Self {
            let previous = super::webhook_policy();
            super::set_webhook_policy(policy);
            Self(previous)
        }
    }

    impl Drop for WebhookPolicyGuard {
        fn drop(&mut self) {
            super::set_webhook_policy(self.0);
        }
    }

    fn expect_json_object(value: norito::json::Value, context: &str) -> norito::json::Map {
        match value {
            norito::json::Value::Object(map) => map,
            _ => panic!("expected object for {context}", context = context),
        }
    }

    #[test]
    fn proof_id_parsing_supports_string_and_object_forms() {
        use hex::encode;
        use iroha_data_model::proof::ProofId;

        let proof = ProofId {
            backend: "halo2/ipa".into(),
            proof_hash: [0xAB; 32],
        };

        let string_value = norito::json::Value::from(proof.to_string());
        assert_eq!(
            super::proof_id_from_json(&string_value),
            Some(proof.clone())
        );

        let mut map = norito::json::Map::new();
        map.insert("backend".into(), norito::json::Value::from("halo2/ipa"));
        map.insert(
            "proof_hash".into(),
            norito::json::Value::from(format!("0x{}", encode(proof.proof_hash))),
        );
        let object_value = norito::json::Value::Object(map);
        assert_eq!(
            super::proof_id_from_json(&object_value),
            Some(proof.clone())
        );

        let mut map_array = norito::json::Map::new();
        map_array.insert("backend".into(), norito::json::Value::from("halo2/ipa"));
        let array = proof
            .proof_hash
            .iter()
            .map(|b| norito::json::Value::from(u64::from(*b)))
            .collect();
        map_array.insert("proof_hash".into(), norito::json::Value::Array(array));
        let array_value = norito::json::Value::Object(map_array);
        assert_eq!(super::proof_id_from_json(&array_value), Some(proof));
    }

    #[test]
    fn delivery_worker_processes_queue() {
        let _env = TestDataDirGuard::new();
        {
            let mut g = registry().lock().unwrap();
            g.next_id = 0;
            g.items.clear();
        }
        super::init_persistence();
        let rt = Runtime::new().expect("tokio runtime");

        rt.block_on(async {
            let deliveries = Arc::new(Mutex::new(Vec::new()));
            let deliveries_clone = Arc::clone(&deliveries);
            let _http_guard = super::install_http_post_override(move |url, _headers, body| {
                deliveries_clone
                    .lock()
                    .expect("deliveries lock")
                    .push((url.to_string(), body.to_vec()));
                Ok(200)
            });

            let target_url = "http://local.test/webhook";

            let webhook_id = {
                let mut g = registry().lock().unwrap();
                g.next_id = 1;
                g.items.insert(
                    1,
                    WebhookEntry {
                        id: 1,
                        url: target_url.to_string(),
                        active: true,
                        secret: None,
                        filter: None,
                    },
                );
                1
            };

            let queue_file = super::queue_dir().join("pending-delivery.json");
            let mut payload = norito::json::Map::new();
            payload.insert("id".into(), norito::json::Value::from("test-id"));
            payload.insert(
                "webhook_id".into(),
                norito::json::Value::from(
                    u64::try_from(webhook_id).expect("webhook id should be non-negative"),
                ),
            );
            payload.insert("url".into(), norito::json::Value::from(target_url));
            payload.insert(
                "content_type".into(),
                norito::json::Value::from("application/json"),
            );
            payload.insert(
                "body".into(),
                norito::json::Value::from(STANDARD.encode(b"{\"ok\":true}")),
            );
            payload.insert("attempts".into(), norito::json::Value::from(0u64));
            payload.insert("next_attempt_ms".into(), norito::json::Value::from(0u64));
            let payload = norito::json::to_json_pretty(&payload).expect("serialize payload");
            std::fs::write(&queue_file, payload).expect("write queue file");

            let mut delivered = false;
            for _ in 0..50 {
                let _ = super::process_queue_once().await;
                if !queue_file.exists() {
                    delivered = true;
                    break;
                }
                sleep(Duration::from_millis(50)).await;
            }
            assert!(delivered, "queued delivery should be processed and removed");

            let recorded = deliveries.lock().expect("deliveries lock");
            assert_eq!(recorded.len(), 1, "expected exactly one delivery attempt");
            let (url, body) = &recorded[0];
            assert_eq!(url, target_url);
            assert!(
                body.windows(b"\"ok\":true".len())
                    .any(|w| w == b"\"ok\":true")
            );

            let mut g = registry().lock().unwrap();
            g.next_id = 0;
            g.items.clear();
        });
    }

    #[test]
    fn queue_capacity_limits_enqueued_payloads() {
        let _env = TestDataDirGuard::new();
        let _ = fs::remove_dir_all(super::queue_dir());
        super::ensure_dirs();
        let _policy_guard = WebhookPolicyGuard::new(super::WebhookPolicy {
            queue_capacity: NonZeroUsize::new(1).unwrap(),
            max_attempts: NonZeroU32::new(3).unwrap(),
            backoff_initial: Duration::from_secs(1),
            backoff_max: Duration::from_secs(1),
            connect_timeout: Duration::from_secs(1),
            write_timeout: Duration::from_secs(1),
            read_timeout: Duration::from_secs(1),
        });
        {
            let mut g = registry().lock().unwrap();
            g.items.clear();
            g.items.insert(
                1,
                WebhookEntry {
                    id: 1,
                    url: "http://example.test/webhook".to_string(),
                    active: true,
                    secret: None,
                    filter: None,
                },
            );
        }

        super::enqueue_delivery_for_all(b"first".to_vec(), "text/plain");
        super::enqueue_delivery_for_all(b"second".to_vec(), "text/plain");

        assert_eq!(super::queue_depth(), 1);
    }

    #[test]
    fn payload_dropped_after_max_attempts() {
        let _env = TestDataDirGuard::new();
        let _ = fs::remove_dir_all(super::queue_dir());
        super::ensure_dirs();
        let _policy_guard = WebhookPolicyGuard::new(super::WebhookPolicy {
            queue_capacity: NonZeroUsize::new(10).unwrap(),
            max_attempts: NonZeroU32::new(2).unwrap(),
            backoff_initial: Duration::from_millis(10),
            backoff_max: Duration::from_millis(20),
            connect_timeout: Duration::from_secs(1),
            write_timeout: Duration::from_secs(1),
            read_timeout: Duration::from_secs(1),
        });
        {
            let mut g = registry().lock().unwrap();
            g.items.clear();
            g.items.insert(
                1,
                WebhookEntry {
                    id: 1,
                    url: "http://local.test/webhook".to_string(),
                    active: true,
                    secret: None,
                    filter: None,
                },
            );
        }
        let pending_path = super::queue_dir().join("pending-drop.json");
        let mut payload = norito::json::Map::new();
        payload.insert("id".into(), norito::json::Value::from("pending-drop"));
        payload.insert("webhook_id".into(), norito::json::Value::from(1u64));
        payload.insert(
            "url".into(),
            norito::json::Value::from("http://local.test/webhook"),
        );
        payload.insert(
            "content_type".into(),
            norito::json::Value::from("application/json"),
        );
        payload.insert(
            "body".into(),
            norito::json::Value::from(STANDARD.encode(b"payload")),
        );
        payload.insert("attempts".into(), norito::json::Value::from(1u64));
        payload.insert("next_attempt_ms".into(), norito::json::Value::from(0u64));
        let json = norito::json::to_json_pretty(&payload).expect("serialize pending payload");
        fs::write(&pending_path, json.as_bytes()).expect("write pending payload");

        let _http_guard = super::install_http_post_override(|_, _, _| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "intentional failure",
            ))
        });
        let rt = Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            super::process_queue_once().await;
        });

        assert_eq!(super::queue_depth(), 0);
    }

    #[test]
    fn delivery_worker_times_out_and_continues() {
        let _env = TestDataDirGuard::new();
        {
            let mut g = registry().lock().unwrap();
            g.next_id = 0;
            g.items.clear();
        }
        super::init_persistence();
        let rt = Runtime::new().expect("tokio runtime");
        let _timeout_guard = TimeoutOverride::new(super::HttpTimeoutConfig {
            connect: Duration::from_millis(200),
            write: Duration::from_millis(200),
            read: Duration::from_millis(200),
        });

        rt.block_on(async {
            let hung_url = "http://local.test/hung/".to_string();
            let success_url = "http://local.test/success/".to_string();
            let hung_attempts = Arc::new(AtomicU32::new(0));
            let success_hits = Arc::new(AtomicU32::new(0));
            let hung_attempts_clone = Arc::clone(&hung_attempts);
            let success_hits_clone = Arc::clone(&success_hits);
            let closure_hung_url = hung_url.clone();
            let closure_success_url = success_url.clone();
            let _http_guard = super::install_http_post_override(move |url, _headers, _body| {
                if url == closure_hung_url {
                    hung_attempts_clone.fetch_add(1, Ordering::SeqCst);
                    Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "simulated timeout",
                    ))
                } else if url == closure_success_url {
                    success_hits_clone.fetch_add(1, Ordering::SeqCst);
                    Ok(200)
                } else {
                    Ok(200)
                }
            });

            {
                let mut g = registry().lock().unwrap();
                g.next_id = 2;
                g.items.insert(
                    1,
                    WebhookEntry {
                        id: 1,
                        url: hung_url.clone(),
                        active: true,
                        secret: None,
                        filter: None,
                    },
                );
                g.items.insert(
                    2,
                    WebhookEntry {
                        id: 2,
                        url: success_url.clone(),
                        active: true,
                        secret: None,
                        filter: None,
                    },
                );
            }

            let queue_dir = super::queue_dir();
            let hung_file = queue_dir.join("0001-timeout.json");
            let success_file = queue_dir.join("0002-success.json");

            let mut hung_payload = norito::json::Map::new();
            hung_payload.insert("id".into(), norito::json::Value::from("timeout-job"));
            hung_payload.insert("webhook_id".into(), norito::json::Value::from(1u64));
            hung_payload.insert("url".into(), norito::json::Value::from(hung_url.clone()));
            hung_payload.insert(
                "content_type".into(),
                norito::json::Value::from("application/json"),
            );
            hung_payload.insert(
                "body".into(),
                norito::json::Value::from(STANDARD.encode(b"{\"timeout\":true}")),
            );
            hung_payload.insert("attempts".into(), norito::json::Value::from(0u64));
            hung_payload.insert("next_attempt_ms".into(), norito::json::Value::from(0u64));
            let hung_payload =
                norito::json::to_json_pretty(&hung_payload).expect("serialize timeout payload");
            std::fs::write(&hung_file, hung_payload).expect("write timeout payload");

            let mut success_payload = norito::json::Map::new();
            success_payload.insert("id".into(), norito::json::Value::from("success-job"));
            success_payload.insert("webhook_id".into(), norito::json::Value::from(2u64));
            success_payload.insert("url".into(), norito::json::Value::from(success_url.clone()));
            success_payload.insert(
                "content_type".into(),
                norito::json::Value::from("application/json"),
            );
            success_payload.insert(
                "body".into(),
                norito::json::Value::from(STANDARD.encode(b"{\"ok\":true}")),
            );
            success_payload.insert("attempts".into(), norito::json::Value::from(0u64));
            success_payload.insert("next_attempt_ms".into(), norito::json::Value::from(0u64));
            let success_payload =
                norito::json::to_json_pretty(&success_payload).expect("serialize success payload");
            std::fs::write(&success_file, success_payload).expect("write success payload");

            let mut success_delivered = false;
            for _ in 0..50 {
                let _ = super::process_queue_once().await;
                if !success_file.exists() {
                    success_delivered = true;
                    break;
                }
                sleep(Duration::from_millis(50)).await;
            }
            assert!(success_delivered, "successful delivery should be removed");

            let mut timeout_recorded = false;
            for _ in 0..50 {
                let _ = super::process_queue_once().await;
                if let Ok(contents) = std::fs::read_to_string(&hung_file) {
                    if contents.contains("\"attempts\": 1") {
                        timeout_recorded = true;
                        break;
                    }
                }
                sleep(Duration::from_millis(50)).await;
            }
            assert!(
                timeout_recorded,
                "timeout job should record a failed attempt"
            );

            let hung_contents =
                std::fs::read_to_string(&hung_file).expect("read timeout payload after retry");
            let hung_value: norito::json::Value =
                norito::json::from_str(&hung_contents).expect("valid timeout payload json");
            let hung_map = expect_json_object(hung_value, "timeout payload");
            assert_eq!(
                hung_map
                    .get("attempts")
                    .and_then(norito::json::Value::as_u64),
                Some(1)
            );
            let next_attempt = hung_map
                .get("next_attempt_ms")
                .and_then(norito::json::Value::as_u64)
                .unwrap_or(0);
            assert!(next_attempt > 0);

            assert!(
                hung_attempts.load(Ordering::SeqCst) >= 1,
                "expected at least one timeout attempt",
            );
            assert!(
                success_hits.load(Ordering::SeqCst) >= 1,
                "expected success webhook to be attempted",
            );

            std::fs::remove_file(&hung_file).expect("cleanup timeout payload");

            let mut g = registry().lock().unwrap();
            g.next_id = 0;
            g.items.clear();
        });
    }

    fn expect_json_array(value: norito::json::Value, context: &str) -> Vec<norito::json::Value> {
        match value {
            norito::json::Value::Array(arr) => arr,
            _ => panic!("expected array for {context}", context = context),
        }
    }

    #[test]
    fn create_list_delete_roundtrip() {
        let _env = TestDataDirGuard::new();
        {
            let mut g = registry().lock().unwrap();
            g.next_id = 0;
            g.items.clear();
        }
        super::init_persistence();
        let data_dir = super::data_dir();
        let rt = Runtime::new().expect("tokio runtime");

        let (entry_id, entry_url) = rt.block_on(async {
            let created_resp =
                super::handle_create_webhook(crate::utils::extractors::JsonOnly(WebhookCreate {
                    url: "https://example.com/hook".into(),
                    secret: Some("s".into()),
                    active: true,
                    filter: None,
                }))
                .await;
            let created_resp = created_resp.into_response();
            assert_eq!(created_resp.status(), StatusCode::CREATED);
            let bytes = created_resp.into_body().collect().await.unwrap().to_bytes();
            let created_value: norito::json::Value =
                norito::json::from_slice(&bytes).expect("valid json body");
            let created_map = expect_json_object(created_value, "created webhook");
            assert!(!created_map.contains_key("secret"));
            assert_eq!(
                created_map
                    .get("has_secret")
                    .and_then(norito::json::Value::as_bool),
                Some(true)
            );
            let id = created_map
                .get("id")
                .and_then(norito::json::Value::as_u64)
                .expect("webhook id in response");
            let url = created_map
                .get("url")
                .and_then(norito::json::Value::as_str)
                .expect("webhook url in response")
                .to_string();

            let list_resp = super::handle_list_webhooks().await.into_response();
            assert_eq!(list_resp.status(), StatusCode::OK);
            let list_bytes = list_resp.into_body().collect().await.unwrap().to_bytes();
            let list_value: norito::json::Value =
                norito::json::from_slice(&list_bytes).expect("valid list json");
            let list_array = expect_json_array(list_value, "webhook list");
            assert_eq!(list_array.len(), 1);
            let list_entry_map = expect_json_object(
                list_array.into_iter().next().expect("one entry"),
                "list entry",
            );
            assert!(!list_entry_map.contains_key("secret"));
            assert_eq!(
                list_entry_map
                    .get("has_secret")
                    .and_then(norito::json::Value::as_bool),
                Some(true)
            );
            (id, url)
        });

        let persisted = std::fs::read_to_string(data_dir.join("webhooks.json")).unwrap();
        assert!(persisted.contains(&entry_url));

        rt.block_on(async {
            let del_status = super::handle_delete_webhook(AxumPath(entry_id)).await;
            assert_eq!(del_status.into_response().status(), StatusCode::NO_CONTENT);
        });

        rt.block_on(async {
            let del_status = super::handle_delete_webhook(AxumPath(entry_id)).await;
            assert_eq!(del_status.into_response().status(), StatusCode::NOT_FOUND);
        });

        {
            let mut g = registry().lock().unwrap();
            g.next_id = 0;
            g.items.clear();
        }
    }

    #[test]
    fn responses_report_secret_presence_without_exposing_value() {
        let _env = TestDataDirGuard::new();
        {
            let mut g = registry().lock().unwrap();
            g.next_id = 0;
            g.items.clear();
        }
        super::init_persistence();
        let rt = Runtime::new().expect("tokio runtime");

        rt.block_on(async {
            let no_secret_resp =
                super::handle_create_webhook(crate::utils::extractors::JsonOnly(WebhookCreate {
                    url: "https://no-secret.example".into(),
                    secret: None,
                    active: true,
                    filter: None,
                }))
                .await
                .into_response();
            let no_secret_bytes = no_secret_resp
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes();
            let no_secret_map = expect_json_object(
                norito::json::from_slice(&no_secret_bytes).expect("valid no-secret json"),
                "create webhook without secret",
            );
            assert!(!no_secret_map.contains_key("secret"));
            assert_eq!(
                no_secret_map
                    .get("has_secret")
                    .and_then(norito::json::Value::as_bool),
                Some(false)
            );

            let with_secret_resp =
                super::handle_create_webhook(crate::utils::extractors::JsonOnly(WebhookCreate {
                    url: "https://with-secret.example".into(),
                    secret: Some("super-secret".into()),
                    active: true,
                    filter: None,
                }))
                .await
                .into_response();
            let with_secret_bytes = with_secret_resp
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes();
            let with_secret_map = expect_json_object(
                norito::json::from_slice(&with_secret_bytes).expect("valid with-secret json"),
                "create webhook with secret",
            );
            assert!(!with_secret_map.contains_key("secret"));
            assert_eq!(
                with_secret_map
                    .get("has_secret")
                    .and_then(norito::json::Value::as_bool),
                Some(true)
            );

            let list_resp = super::handle_list_webhooks().await.into_response();
            assert_eq!(list_resp.status(), StatusCode::OK);
            let list_bytes = list_resp.into_body().collect().await.unwrap().to_bytes();
            let list_entries = expect_json_array(
                norito::json::from_slice(&list_bytes).expect("valid list json"),
                "list after secret variations",
            );
            assert_eq!(list_entries.len(), 2);

            let mut seen = Vec::new();
            for entry in list_entries {
                let map = expect_json_object(entry, "list entry secret check");
                assert!(!map.contains_key("secret"));
                let url = map
                    .get("url")
                    .and_then(norito::json::Value::as_str)
                    .expect("url present")
                    .to_string();
                let has_secret = map
                    .get("has_secret")
                    .and_then(norito::json::Value::as_bool)
                    .expect("has_secret present");
                seen.push((url, has_secret));
            }

            assert!(
                seen.iter()
                    .any(|(url, has)| url == "https://no-secret.example" && !has)
            );
            assert!(
                seen.iter()
                    .any(|(url, has)| url == "https://with-secret.example" && *has)
            );
        });

        {
            let mut g = registry().lock().unwrap();
            g.next_id = 0;
            g.items.clear();
        }
    }

    #[test]
    fn hmac_known_vector() {
        // RFC 4231 Test Case 1
        let key = [0x0b_u8; 20];
        let data = b"Hi There";
        let mac = super::hmac_sha256_hex(&key, data);
        assert_eq!(
            mac,
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn enqueue_respects_filter() {
        let _env = TestDataDirGuard::new();
        super::init_persistence();

        // Insert 2 webhooks: one for Queued, one for Approved
        {
            let mut g = registry().lock().unwrap();
            g.next_id = 0;
            g.items.clear();
            g.next_id += 1;
            let id1 = g.next_id;
            g.items.insert(
                id1,
                WebhookEntry {
                    id: id1,
                    url: "http://127.0.0.1:9/blackhole".into(),
                    active: true,
                    secret: None,
                    filter: Some(crate::filter::FilterExpr::Eq(
                        crate::filter::FieldPath("tx_status".into()),
                        norito::json::Value::String("Queued".into()),
                    )),
                },
            );
            g.next_id += 1;
            let id2 = g.next_id;
            g.items.insert(
                id2,
                WebhookEntry {
                    id: id2,
                    url: "http://127.0.0.1:9/blackhole".into(),
                    active: true,
                    secret: None,
                    filter: Some(crate::filter::FilterExpr::Eq(
                        crate::filter::FieldPath("tx_status".into()),
                        norito::json::Value::String("Approved".into()),
                    )),
                },
            );
        }

        // Event with tx_status = Queued
        let ev = EventBox::from(TransactionEvent {
            hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::prehashed(
                [7u8; Hash::LENGTH],
            )),
            block_height: None,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Queued,
        });

        enqueue_event_for_matching_webhooks(&ev, "application/json");

        let files = std::fs::read_dir(queue_dir()).unwrap();
        let count = files
            .filter(|e| {
                if let Ok(f) = e {
                    if let Some(ext) = f.path().extension() {
                        return ext == "json";
                    }
                }
                false
            })
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn enqueue_respects_proof_envelope_hash_filter() {
        use iroha_data_model::events::data::{
            prelude::DataEvent,
            proof::{ProofEvent, ProofVerified},
        };

        use crate::filter::{FieldPath, FilterExpr};

        let _env = TestDataDirGuard::new();
        super::init_persistence();

        // Two webhooks: one matches specific envelope hash, one with different hash
        let match_id: u64;
        {
            let mut g = registry().lock().unwrap();
            g.next_id = 0;
            g.items.clear();
            // matching: proof_envelope_hash == 0xCC..CC
            g.next_id += 1;
            let id1 = g.next_id;
            match_id = id1;
            g.items.insert(
                id1,
                WebhookEntry {
                    id: id1,
                    url: "http://127.0.0.1:9/blackhole".into(),
                    active: true,
                    secret: None,
                    filter: Some(FilterExpr::Eq(
                        FieldPath("proof_envelope_hash".into()),
                        norito::json::Value::String(hex::encode([0xCCu8; 32])),
                    )),
                },
            );
            // non-matching: proof_envelope_hash == 0xDD..DD
            g.next_id += 1;
            let id2 = g.next_id;
            g.items.insert(
                id2,
                WebhookEntry {
                    id: id2,
                    url: "http://127.0.0.1:9/blackhole".into(),
                    active: true,
                    secret: None,
                    filter: Some(FilterExpr::Eq(
                        FieldPath("proof_envelope_hash".into()),
                        norito::json::Value::String(hex::encode([0xDDu8; 32])),
                    )),
                },
            );
        }

        // Event with envelope_hash = 0xCC..CC
        let ev = iroha_data_model::events::EventBox::Data(
            iroha_data_model::events::SharedDataEvent::from(DataEvent::Proof(
                ProofEvent::Verified(ProofVerified {
                    id: iroha_data_model::proof::ProofId {
                        backend: "halo2/ipa".into(),
                        proof_hash: [0xA1; 32],
                    },
                    vk_ref: None,
                    vk_commitment: None,
                    call_hash: None,
                    envelope_hash: Some([0xCC; 32]),
                }),
            )),
        );

        enqueue_event_for_matching_webhooks(&ev, "application/json");

        // Exactly one delivery (matching id1) should be enqueued; also assert webhook_id matches
        let files: Vec<_> = std::fs::read_dir(queue_dir())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
            .collect();
        assert_eq!(files.len(), 1);
        let content = std::fs::read_to_string(files[0].path()).unwrap();
        let v: norito::json::Value = norito::json::from_str(&content).unwrap();
        let got_id = v
            .as_object()
            .and_then(|m| m.get("webhook_id"))
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        assert_eq!(got_id, match_id);
    }

    #[test]
    fn proof_id_eq_builds_matching_filter() {
        use crate::filter::{FieldPath, FilterExpr};
        // Build a ProofId and event record wrapper
        let id = iroha_data_model::proof::ProofId {
            backend: "halo2/ipa".into(),
            proof_hash: [0xAA; 32],
        };
        let id_str = format!("{}", id);
        use iroha_data_model::events::data::{
            prelude::DataEvent,
            proof::{ProofEvent, ProofVerified},
        };
        let ev: iroha_data_model::events::EventBox = iroha_data_model::events::EventBox::Data(
            iroha_data_model::events::SharedDataEvent::from(DataEvent::Proof(
                ProofEvent::Verified(ProofVerified {
                    id: id.clone(),
                    vk_ref: None,
                    vk_commitment: None,
                    call_hash: None,
                    envelope_hash: None,
                }),
            )),
        );

        let expr = FilterExpr::Eq(
            FieldPath("proof_id".into()),
            norito::json::Value::String(id_str),
        );
        let filters = event_filter_boxes_from_expr(&expr);
        assert!(!filters.is_empty());
        assert!(filters.iter().any(|f| f.matches(&ev)));
    }
}
