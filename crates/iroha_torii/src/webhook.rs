//! Minimal webhook registry for the app-facing API with disk persistence and
//! a background delivery worker.
//!
//! Feature-gated behind `app_api`:
//! - Stores webhooks in-memory, persisted to `./storage/torii/webhooks.json` by default.
//!   Base directory is configured via `torii.data_dir`; tests may use `data_dir::OverrideGuard`.
//! - Exposes CRUD endpoints to create/list/delete webhooks.
//! - Background worker scans a disk-backed queue and delivers payloads with
//!   optional HMAC-SHA256 signature and exponential backoff retries. Queue
//!   admission, spool records, decoded bodies, and each scan batch have hard
//!   bounds so a large or adversarial queue cannot grow worker memory without
//!   limit.
//! - HTTPS delivery is supported when the `app_api_https` feature is enabled,
//!   using `reqwest` + `rustls` with native roots. Otherwise, only `http://` is allowed.
//!
//! Endpoints (wired in `lib.rs` when `app_api` is enabled):
//! - POST `/v1/webhooks` – Create a webhook.
//! - GET  `/v1/webhooks` – List webhooks.
//! - DELETE `/v1/webhooks/{id}` – Delete a webhook by id.
use crate::filter::filter_expr_to_value;
use axum::{extract::Path as AxumPath, http::StatusCode, response::IntoResponse};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use core::{convert::TryFrom, str::FromStr};
use iroha_config::parameters::defaults;
use iroha_data_model::{
    events::data::prelude as df,
    nexus::{DataSpaceId, LaneId},
    prelude::DataEvent,
};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use std::{
    collections::HashMap,
    fs,
    io::{Read as _, Write as _},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    num::{NonZeroU32, NonZeroUsize},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::fs as tokio_fs;
use url::{Host, Url};
const WEBHOOK_REGISTRY_MAX_ENTRIES: usize = 1_024;
const WEBHOOK_REGISTRY_MAX_BYTES: usize = 8 * 1024 * 1024;
const WEBHOOK_ENTRY_MAX_BYTES: usize = 64 * 1024;
const WEBHOOK_HTTP_RESPONSE_MAX_BYTES: u64 = 64 * 1024;
// The configured capacity may be lowered, but never raises this process-level
// safety ceiling. This intentionally matches the shipped default.
const WEBHOOK_QUEUE_HARD_CAPACITY: usize = 10_000;
const WEBHOOK_DELIVERY_MAX_BYTES: usize = 1024 * 1024;
const WEBHOOK_DELIVERY_METADATA_MAX_BYTES: usize = 64 * 1024;
const WEBHOOK_DELIVERY_MAX_BASE64_BYTES: usize = WEBHOOK_DELIVERY_MAX_BYTES.div_ceil(3) * 4;
// A 1 MiB body expands to about 1.34 MiB in base64; leave bounded room for the
// delivery metadata while rejecting unexpectedly large on-disk records.
const WEBHOOK_QUEUE_FILE_MAX_BYTES: usize = 2 * 1024 * 1024;
const WEBHOOK_QUEUE_SCAN_BATCH_SIZE: usize = 128;
const WEBHOOK_QUEUE_SCAN_WORK_ITEMS: usize = 1024;
const WEBHOOK_QUEUE_ADMISSION_SCAN_WORK_ITEMS: usize = WEBHOOK_QUEUE_HARD_CAPACITY * 2;
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
fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
fn lock_registry() -> std::sync::MutexGuard<'static, RegistryInner> {
    lock_unpoisoned(registry())
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
fn effective_queue_capacity(policy: WebhookPolicy) -> usize {
    policy.queue_capacity.get().min(WEBHOOK_QUEUE_HARD_CAPACITY)
}
fn queue_depth_bounded(maximum: usize) -> std::io::Result<usize> {
    queue_depth_bounded_at(
        &queue_dir(),
        maximum,
        WEBHOOK_QUEUE_ADMISSION_SCAN_WORK_ITEMS,
    )
}
fn queue_depth_bounded_at(
    root: &Path,
    maximum: usize,
    work_limit: usize,
) -> std::io::Result<usize> {
    let mut count = 0_usize;
    for (index, entry) in fs::read_dir(root)?.enumerate() {
        if index >= work_limit {
            return Err(std::io::Error::other(
                "webhook queue admission scan work limit reached",
            ));
        }
        let entry = entry?;
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        count = count.saturating_add(1);
        if count >= maximum {
            return Ok(maximum);
        }
    }
    Ok(count)
}
#[cfg(test)]
fn queue_depth() -> usize {
    match queue_depth_bounded(usize::MAX) {
        Ok(depth) => depth,
        Err(err) => {
            iroha_logger::warn!(%err, "failed to read webhook queue directory");
            0
        }
    }
}
fn queue_write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
struct QueueAdmission {
    _guard: std::sync::MutexGuard<'static, ()>,
    remaining: usize,
}
impl QueueAdmission {
    fn begin(policy: WebhookPolicy) -> std::io::Result<Self> {
        ensure_dirs();
        let guard = lock_unpoisoned(queue_write_lock());
        let capacity = effective_queue_capacity(policy);
        let used = queue_depth_bounded(capacity)?;
        Ok(Self {
            _guard: guard,
            remaining: capacity.saturating_sub(used),
        })
    }
    fn is_full(&self) -> bool {
        self.remaining == 0
    }
    fn persist(&mut self, pd: &PendingDelivery) -> std::io::Result<()> {
        if self.is_full() {
            return Err(std::io::Error::other("webhook queue hard capacity reached"));
        }
        let encoded = encode_pending_delivery(pd)?;
        let path = queue_dir().join(format!("{}.json", pd.id));
        let mut tmp = tempfile::NamedTempFile::new_in(queue_dir())?;
        tmp.write_all(encoded.as_bytes())?;
        tmp.flush()?;
        tmp.persist_noclobber(path)?;
        self.remaining = self.remaining.saturating_sub(1);
        Ok(())
    }
}
fn encode_pending_delivery(pd: &PendingDelivery) -> std::io::Result<String> {
    if pd.body.len() > WEBHOOK_DELIVERY_MAX_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "webhook delivery exceeds hard byte limit",
        ));
    }
    if !delivery_metadata_is_bounded(&pd.id, &pd.url, &pd.content_type) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "webhook delivery metadata exceeds hard byte limit",
        ));
    }
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
    let encoded = norito::json::to_json_pretty(&payload).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to encode webhook delivery: {err}"),
        )
    })?;
    if encoded.len() > WEBHOOK_QUEUE_FILE_MAX_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "webhook spool record exceeds hard byte limit",
        ));
    }
    Ok(encoded)
}
fn delivery_metadata_is_bounded(id: &str, url: &str, content_type: &str) -> bool {
    id.len()
        .checked_add(url.len())
        .and_then(|length| length.checked_add(content_type.len()))
        .is_some_and(|length| length <= WEBHOOK_DELIVERY_METADATA_MAX_BYTES)
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
fn parse_account_id_literal(input: &str) -> Option<iroha_data_model::account::AccountId> {
    iroha_data_model::account::AccountId::parse_encoded(input).ok()
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
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
pub fn set_http_timeout_config(config: HttpTimeoutConfig) {
    *http_timeout_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = config;
}
#[derive(Clone, Copy, Debug)]
pub struct WebhookPolicy {
    /// Configured queue capacity, capped by the source-level hard ceiling.
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
#[cfg(test)]
fn webhook_policy_writer_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
fn webhook_policy() -> WebhookPolicy {
    *webhook_policy_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
fn apply_webhook_policy(policy: WebhookPolicy) {
    *webhook_policy_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = policy;
    set_http_timeout_config(HttpTimeoutConfig {
        connect: policy.connect_timeout,
        write: policy.write_timeout,
        read: policy.read_timeout,
    });
}
pub fn set_webhook_policy(policy: WebhookPolicy) {
    #[cfg(test)]
    let _writer_guard = webhook_policy_writer_lock()
        .lock()
        .expect("webhook policy writer lock");
    apply_webhook_policy(policy);
}
/// Webhook destination security policy (SSRF guard rails).
#[derive(Clone, Debug)]
pub struct WebhookSecurityPolicy {
    /// Enable webhook destination guard rails.
    pub enabled: bool,
    /// CIDR allow-list for webhook destination IPs.
    pub allow_nets: Vec<crate::limits::IpNet>,
}
impl Default for WebhookSecurityPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            allow_nets: Vec::new(),
        }
    }
}
fn webhook_security_policy_state() -> &'static Mutex<WebhookSecurityPolicy> {
    static STATE: OnceLock<Mutex<WebhookSecurityPolicy>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(WebhookSecurityPolicy::default()))
}
fn webhook_security_policy() -> WebhookSecurityPolicy {
    webhook_security_policy_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}
pub fn set_webhook_security_policy(policy: WebhookSecurityPolicy) {
    *webhook_security_policy_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = policy;
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
        {
            let guard = lock_registry();
            let mut arr = Vec::with_capacity(guard.items.len());
            for (_, e) in guard.items.iter() {
                arr.push(webhook_entry_to_storage_json(e));
            }
            let body = norito::json::to_json_pretty(&norito::json::Value::Array(arr))
                .unwrap_or_else(|_| "[]".into());
            if body.len() > WEBHOOK_REGISTRY_MAX_BYTES {
                iroha_logger::error!(
                    actual = body.len(),
                    maximum = WEBHOOK_REGISTRY_MAX_BYTES,
                    "refusing to persist oversized webhook registry"
                );
                return;
            }
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
    if let Ok(f) = fs::File::open(&path) {
        let Ok(metadata) = f.metadata() else {
            return;
        };
        if !metadata.is_file()
            || usize::try_from(metadata.len()).map_or(true, |len| len > WEBHOOK_REGISTRY_MAX_BYTES)
        {
            iroha_logger::warn!(
                path = %path.display(),
                maximum = WEBHOOK_REGISTRY_MAX_BYTES,
                "refusing to load oversized or non-regular webhook registry"
            );
            return;
        }
        let mut buf = Vec::new();
        let mut limited = f.take(
            u64::try_from(WEBHOOK_REGISTRY_MAX_BYTES)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        );
        if limited.read_to_end(&mut buf).is_ok() && buf.len() <= WEBHOOK_REGISTRY_MAX_BYTES {
            if let Ok(norito::json::Value::Array(arr)) =
                norito::json::from_slice::<norito::json::Value>(&buf)
            {
                let mut guard = lock_registry();
                guard.items.clear();
                let mut max_id = 0u64;
                for (index, v) in arr.into_iter().enumerate() {
                    if let norito::json::Value::Object(m) = v {
                        let Some(idv) = m.get("id").and_then(norito::json::Value::as_u64) else {
                            continue;
                        };
                        // IDs are durable identities, including for entries
                        // quarantined below or ignored past the storage cap.
                        // Never recycle one merely because the rest of its
                        // persisted record is corrupt.
                        max_id = max_id.max(idv);
                        if index >= WEBHOOK_REGISTRY_MAX_ENTRIES {
                            continue;
                        }
                        if let (Some(urlv), Some(activev)) = (
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
                            let filter = match m.get("filter") {
                                None | Some(norito::json::Value::Null) => None,
                                Some(value) => {
                                    let Some(filter) = value_to_filter_expr(value) else {
                                        iroha_logger::warn!(
                                            webhook_id = idv,
                                            "skipping persisted webhook with malformed filter"
                                        );
                                        continue;
                                    };
                                    if let Err(error) = crate::filter::validate_filter(&filter) {
                                        iroha_logger::warn!(
                                            webhook_id = idv,
                                            %error,
                                            "skipping persisted webhook with invalid filter"
                                        );
                                        continue;
                                    }
                                    Some(filter)
                                }
                            };
                            let entry = WebhookEntry {
                                id: idv,
                                url: urlv,
                                active: activev,
                                secret,
                                filter,
                            };
                            guard.items.insert(idv, entry);
                        }
                    }
                }
                guard.next_id = max_id;
            }
        }
    }
}
fn webhook_entry_to_storage_json(entry: &WebhookEntry) -> norito::json::Value {
    let mut map = norito::json::Map::new();
    map.insert("id".into(), norito::json::Value::from(entry.id));
    map.insert("url".into(), norito::json::Value::from(entry.url.clone()));
    map.insert("active".into(), norito::json::Value::from(entry.active));
    map.insert(
        "secret".into(),
        entry
            .secret
            .clone()
            .map_or(norito::json::Value::Null, norito::json::Value::from),
    );
    map.insert(
        "filter".into(),
        entry
            .filter
            .as_ref()
            .map_or(norito::json::Value::Null, filter_expr_to_value),
    );
    norito::json::Value::Object(map)
}
fn webhook_entry_encoded_len(entry: &WebhookEntry) -> Result<usize, norito::json::Error> {
    norito::json::to_vec(&webhook_entry_to_storage_json(entry)).map(|bytes| bytes.len())
}
fn registry_can_retain(guard: &RegistryInner, candidate: &WebhookEntry) -> bool {
    if guard.items.len() >= WEBHOOK_REGISTRY_MAX_ENTRIES {
        return false;
    }
    let Ok(candidate_len) = webhook_entry_encoded_len(candidate) else {
        return false;
    };
    if candidate_len > WEBHOOK_ENTRY_MAX_BYTES {
        return false;
    }
    let retained = guard.items.values().try_fold(0_usize, |total, entry| {
        webhook_entry_encoded_len(entry)
            .ok()
            .and_then(|len| total.checked_add(len.saturating_add(1)))
    });
    retained.is_some_and(|retained| {
        retained
            .checked_add(candidate_len.saturating_add(2))
            .is_some_and(|total| total <= WEBHOOK_REGISTRY_MAX_BYTES)
    })
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
fn is_public_ipv4(v4: Ipv4Addr) -> bool {
    if v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_multicast()
        || v4.is_broadcast()
        || v4.is_documentation()
        || v4.is_unspecified()
    {
        return false;
    }
    let [a, b, ..] = v4.octets();
    // 0.0.0.0/8 (\"this network\")
    if a == 0 {
        return false;
    }
    // 100.64.0.0/10 (carrier-grade NAT)
    if a == 100 && (64..=127).contains(&b) {
        return false;
    }
    // 198.18.0.0/15 (benchmarking)
    if a == 198 && (b == 18 || b == 19) {
        return false;
    }
    // 240.0.0.0/4 (reserved)
    if a >= 240 {
        return false;
    }
    true
}
fn is_documentation_ipv6(v6: Ipv6Addr) -> bool {
    // 2001:db8::/32
    let seg = v6.segments();
    seg[0] == 0x2001 && seg[1] == 0x0db8
}
fn is_public_ipv6(v6: Ipv6Addr) -> bool {
    if v6.is_loopback()
        || v6.is_unspecified()
        || v6.is_multicast()
        || v6.is_unicast_link_local()
        || v6.is_unique_local()
        || is_documentation_ipv6(v6)
    {
        return false;
    }
    if let Some(v4) = v6.to_ipv4_mapped() {
        return is_public_ipv4(v4);
    }
    true
}
fn is_public_destination_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_ipv4(v4),
        IpAddr::V6(v6) => is_public_ipv6(v6),
    }
}
fn is_destination_ip_allowed(ip: IpAddr, policy: &WebhookSecurityPolicy) -> bool {
    if crate::limits::cidr_contains(&policy.allow_nets, ip) {
        return true;
    }
    is_public_destination_ip(ip)
}
fn is_localhost_domain(domain: &str) -> bool {
    let domain = domain.trim_end_matches('.');
    domain.eq_ignore_ascii_case("localhost")
}
fn validate_webhook_url_for_create(
    raw: &str,
    policy: &WebhookSecurityPolicy,
) -> Result<(), (StatusCode, String)> {
    let url = Url::parse(raw)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid webhook url: {e}")))?;
    match url.scheme() {
        "http" | "https" | "ws" | "wss" => {}
        other => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("unsupported webhook scheme `{other}`"),
            ));
        }
    }
    let Some(host) = url.host() else {
        return Err((
            StatusCode::BAD_REQUEST,
            "webhook url must include a host".to_string(),
        ));
    };
    if policy.enabled {
        if let Host::Domain(domain) = host {
            if is_localhost_domain(domain) {
                return Err((
                    StatusCode::FORBIDDEN,
                    "webhook url host `localhost` is not allowed".to_string(),
                ));
            }
        }
        match host {
            Host::Ipv4(v4) => {
                if !is_destination_ip_allowed(IpAddr::V4(v4), policy) {
                    return Err((
                        StatusCode::FORBIDDEN,
                        "webhook url host is not allowed".to_string(),
                    ));
                }
            }
            Host::Ipv6(v6) => {
                if !is_destination_ip_allowed(IpAddr::V6(v6), policy) {
                    return Err((
                        StatusCode::FORBIDDEN,
                        "webhook url host is not allowed".to_string(),
                    ));
                }
            }
            Host::Domain(_) => {}
        }
    }
    Ok(())
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
    let policy = webhook_security_policy();
    if let Err((status, message)) = validate_webhook_url_for_create(&req.url, &policy) {
        return (status, message).into_response();
    }
    let mut guard = lock_registry();
    let Some(id) = guard.next_id.checked_add(1) else {
        return (
            StatusCode::INSUFFICIENT_STORAGE,
            "webhook registry identifier space exhausted",
        )
            .into_response();
    };
    let entry = WebhookEntry {
        id,
        url: req.url,
        active: req.active,
        secret: req.secret,
        filter: req.filter,
    };
    if !registry_can_retain(&guard, &entry) {
        return (
            StatusCode::INSUFFICIENT_STORAGE,
            "webhook registry capacity exceeded",
        )
            .into_response();
    }
    guard.next_id = id;
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
    let guard = lock_registry();
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
    let mut guard = lock_registry();
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
    if body.len() > WEBHOOK_DELIVERY_MAX_BYTES {
        iroha_logger::warn!(
            actual = body.len(),
            maximum = WEBHOOK_DELIVERY_MAX_BYTES,
            "dropping oversized webhook delivery"
        );
        return;
    }
    if content_type.len() > WEBHOOK_DELIVERY_METADATA_MAX_BYTES {
        iroha_logger::warn!(
            actual = content_type.len(),
            maximum = WEBHOOK_DELIVERY_METADATA_MAX_BYTES,
            "dropping webhook delivery with oversized content type"
        );
        return;
    }
    let policy = webhook_policy();
    let mut admission = match QueueAdmission::begin(policy) {
        Ok(admission) => admission,
        Err(err) => {
            iroha_logger::warn!(%err, "failed to inspect webhook queue capacity");
            return;
        }
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let guard = lock_registry();
    for (id, w) in guard.items.iter() {
        if !w.active {
            continue;
        }
        if admission.is_full() {
            iroha_logger::warn!(
                capacity = effective_queue_capacity(policy),
                "webhook queue at capacity; dropping new deliveries"
            );
            break;
        }
        let delivery_id = format!("{}-{}", id, now);
        if !delivery_metadata_is_bounded(&delivery_id, &w.url, content_type) {
            iroha_logger::warn!(
                webhook_id = *id,
                maximum = WEBHOOK_DELIVERY_METADATA_MAX_BYTES,
                "dropping webhook delivery with oversized metadata"
            );
            continue;
        }
        let pd = PendingDelivery {
            id: delivery_id,
            webhook_id: *id,
            url: w.url.clone(),
            content_type: content_type.to_string(),
            body: body.clone(),
            attempts: 0,
            next_attempt_ms: now,
        };
        if let Err(err) = admission.persist(&pd) {
            iroha_logger::warn!(%err, "failed to persist webhook payload");
            continue;
        }
    }
}
pub fn enqueue_event_for_matching_webhooks(
    event: &iroha_data_model::events::EventBox,
    content_type: &str,
) {
    ensure_dirs();
    if content_type.len() > WEBHOOK_DELIVERY_METADATA_MAX_BYTES {
        iroha_logger::warn!(
            actual = content_type.len(),
            maximum = WEBHOOK_DELIVERY_METADATA_MAX_BYTES,
            "dropping webhook event with oversized content type"
        );
        return;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    // Snapshot registry to minimize lock duration
    let entries: Vec<(u64, WebhookEntry)> = lock_registry()
        .items
        .iter()
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    let json_val = crate::routing::event_to_json_value(event);
    let body = match norito::json::to_json(&json_val) {
        Ok(s) => s.into_bytes(),
        Err(e) => {
            iroha_logger::warn!(%e, "failed to serialize event for webhook");
            return;
        }
    };
    if body.len() > WEBHOOK_DELIVERY_MAX_BYTES {
        iroha_logger::warn!(
            actual = body.len(),
            maximum = WEBHOOK_DELIVERY_MAX_BYTES,
            "dropping oversized webhook event"
        );
        return;
    }
    let policy = webhook_policy();
    let mut admission = match QueueAdmission::begin(policy) {
        Ok(admission) => admission,
        Err(err) => {
            iroha_logger::warn!(%err, "failed to inspect webhook queue capacity");
            return;
        }
    };
    for (id, w) in entries {
        if !w.active {
            continue;
        }
        if admission.is_full() {
            iroha_logger::warn!(
                capacity = effective_queue_capacity(policy),
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
        let delivery_id = format!("{}-{}", id, now);
        if !delivery_metadata_is_bounded(&delivery_id, &w.url, content_type) {
            iroha_logger::warn!(
                webhook_id = id,
                maximum = WEBHOOK_DELIVERY_METADATA_MAX_BYTES,
                "dropping webhook event with oversized metadata"
            );
            continue;
        }
        let pd = PendingDelivery {
            id: delivery_id,
            webhook_id: id,
            url: w.url.clone(),
            content_type: content_type.to_string(),
            body: body.clone(),
            attempts: 0,
            next_attempt_ms: now,
        };
        if let Err(err) = admission.persist(&pd) {
            iroha_logger::warn!(%err, "failed to persist webhook payload");
            continue;
        }
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
    use crate::filter::FilterExpr as F;
    use iroha_data_model::events::{
        EventFilterBox,
        execute_trigger::prelude::ExecuteTriggerEventFilter,
        pipeline::{BlockEventFilter, BlockStatus, TransactionEventFilter, TransactionStatus},
        time::{ExecutionTime, TimeEventFilter},
        trigger_completed::prelude::{TriggerCompletedEventFilter, TriggerCompletedOutcomeType},
    };
    use std::num::NonZeroU64;
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
                    .and_then(|s| iroha_data_model::domain::DomainId::parse_fully_qualified(s).ok())
                    .map(|id| {
                        EventFilterBox::Data(df::DataEventFilter::Domain(
                            df::DomainEventFilter::new().for_domain(id),
                        ))
                    })
                    .into_iter()
                    .collect(),
                "account_id" => value
                    .as_str()
                    .and_then(parse_account_id_literal)
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
                "rwa_id" => value
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .map(|id| {
                        EventFilterBox::Data(df::DataEventFilter::Rwa(
                            df::RwaEventFilter::new().for_rwa(id),
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
                    .and_then(parse_account_id_literal)
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
                    rwa_id: Option<iroha_data_model::rwa::RwaId>,
                    rwa_set: Option<df::RwaEventSet>,
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
                        "domain_id" => {
                            c.domain_id = v.as_str().and_then(|s| {
                                iroha_data_model::domain::DomainId::parse_fully_qualified(s).ok()
                            })
                        }
                        "account_id" => {
                            c.account_id = v.as_str().and_then(parse_account_id_literal)
                        }
                        "asset_id" => c.asset_id = v.as_str().and_then(|s| s.parse().ok()),
                        "asset_definition_id" => {
                            c.asset_def_id = v.as_str().and_then(|s| s.parse().ok())
                        }
                        "nft_id" => c.nft_id = v.as_str().and_then(|s| s.parse().ok()),
                        "rwa_id" => c.rwa_id = v.as_str().and_then(|s| s.parse().ok()),
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
                                "AssetDefinition" => Some(df::DomainEventSet::AssetDefinition),
                                "Asset" => Some(df::DomainEventSet::Asset),
                                "Nft" => Some(df::DomainEventSet::AnyNft),
                                "Account" => Some(df::DomainEventSet::Account),
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
                        "rwa_event" => {
                            c.rwa_set = parse_event_list(v, &|s| match s {
                                "Created" => Some(df::RwaEventSet::Created),
                                "MetadataInserted" => Some(df::RwaEventSet::MetadataInserted),
                                "MetadataRemoved" => Some(df::RwaEventSet::MetadataRemoved),
                                "OwnerChanged" => Some(df::RwaEventSet::OwnerChanged),
                                "Split" => Some(df::RwaEventSet::Split),
                                "Merged" => Some(df::RwaEventSet::Merged),
                                "Redeemed" => Some(df::RwaEventSet::Redeemed),
                                "Frozen" => Some(df::RwaEventSet::Frozen),
                                "Unfrozen" => Some(df::RwaEventSet::Unfrozen),
                                "Held" => Some(df::RwaEventSet::Held),
                                "Released" => Some(df::RwaEventSet::Released),
                                "ForceTransferred" => Some(df::RwaEventSet::ForceTransferred),
                                "ControlsChanged" => Some(df::RwaEventSet::ControlsChanged),
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
                            c.exec_trig_auth = v.as_str().and_then(parse_account_id_literal);
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
                if c.rwa_id.is_some() || c.rwa_set.is_some() {
                    let mut f = df::RwaEventFilter::new();
                    if let Some(id) = c.rwa_id {
                        f = f.for_rwa(id);
                    }
                    if let Some(set) = c.rwa_set {
                        f = f.for_events(set);
                    }
                    out.push(EventFilterBox::Data(df::DataEventFilter::Rwa(f)));
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
fn io_invalid_input(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into())
}
fn io_permission_denied(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, message.into())
}
async fn resolve_destination_addrs(
    url: &Url,
    policy: &WebhookSecurityPolicy,
) -> std::io::Result<Vec<SocketAddr>> {
    let Some(host) = url.host() else {
        return Err(io_invalid_input("webhook url missing host"));
    };
    let Some(port) = url.port_or_known_default() else {
        return Err(io_invalid_input("webhook url missing port"));
    };
    if policy.enabled {
        if let Host::Domain(domain) = host {
            if is_localhost_domain(domain) {
                return Err(io_permission_denied(
                    "webhook destination host `localhost` is not allowed",
                ));
            }
        }
    }
    match host {
        Host::Ipv4(v4) => {
            let ip = IpAddr::V4(v4);
            if policy.enabled && !is_destination_ip_allowed(ip, policy) {
                return Err(io_permission_denied(
                    "webhook destination IP is not allowed",
                ));
            }
            Ok(vec![SocketAddr::new(ip, port)])
        }
        Host::Ipv6(v6) => {
            let ip = IpAddr::V6(v6);
            if policy.enabled && !is_destination_ip_allowed(ip, policy) {
                return Err(io_permission_denied(
                    "webhook destination IP is not allowed",
                ));
            }
            Ok(vec![SocketAddr::new(ip, port)])
        }
        Host::Domain(domain) => {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((domain, port)).await?.collect();
            if addrs.is_empty() {
                return Err(io_invalid_input(
                    "webhook destination resolved to no addresses",
                ));
            }
            if policy.enabled {
                for addr in &addrs {
                    if !is_destination_ip_allowed(addr.ip(), policy) {
                        return Err(io_permission_denied(
                            "webhook destination resolved to a disallowed IP",
                        ));
                    }
                }
            }
            Ok(addrs)
        }
    }
}
fn host_header_value(url: &Url) -> std::io::Result<String> {
    let Some(host) = url.host() else {
        return Err(io_invalid_input("webhook url missing host"));
    };
    let Some(port) = url.port_or_known_default() else {
        return Err(io_invalid_input("webhook url missing port"));
    };
    let known_default = match url.scheme() {
        "http" | "ws" => Some(80),
        "https" | "wss" => Some(443),
        _ => None,
    };
    let host = match host {
        Host::Domain(domain) => domain.to_string(),
        Host::Ipv4(v4) => v4.to_string(),
        Host::Ipv6(v6) => format!("[{v6}]"),
    };
    let mut out = host;
    if known_default.is_some_and(|d| d != port) {
        out.push(':');
        out.push_str(&port.to_string());
    }
    Ok(out)
}
#[cfg(feature = "app_api_https")]
fn https_delivery_dns_override(
    url: &Url,
    connect_addrs: &[SocketAddr],
) -> Option<(String, Vec<SocketAddr>)> {
    match url.host() {
        // Preserve the original hostname for SNI / certificate verification while
        // pinning the actual connect target to the already-vetted address set.
        Some(Host::Domain(domain)) if !connect_addrs.is_empty() => {
            Some((domain.to_owned(), connect_addrs.to_vec()))
        }
        _ => None,
    }
}
#[cfg(feature = "app_api_wss")]
fn websocket_pinned_connect_addr(
    url: &Url,
    policy: &WebhookSecurityPolicy,
    connect_addrs: &[SocketAddr],
) -> Option<SocketAddr> {
    match url.scheme() {
        "ws" => connect_addrs.first().copied(),
        "wss" if policy.enabled => connect_addrs.first().copied(),
        _ => None,
    }
}
async fn http_post_plain(
    url: &Url,
    connect_addr: SocketAddr,
    host_header: &str,
    headers: &[(&str, String)],
    body: &[u8],
) -> std::io::Result<u16> {
    // Very small plain HTTP/1.1 client for http:// (no TLS).
    if url.scheme() != "http" {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "invalid scheme for plain HTTP client",
        ))
    } else {
        let mut path = url.path().to_string();
        if path.is_empty() {
            path = "/".to_string();
        }
        if let Some(query) = url.query() {
            path.push('?');
            path.push_str(query);
        }
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpStream,
        };
        let timeouts = http_timeout_config();
        let mut stream =
            match tokio::time::timeout(timeouts.connect, TcpStream::connect(connect_addr)).await {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err(io_timeout_error("tcp connect", timeouts.connect)),
            };
        let mut req = Vec::new();
        req.extend_from_slice(format!("POST {} HTTP/1.1\r\n", path).as_bytes());
        req.extend_from_slice(format!("Host: {}\r\n", host_header).as_bytes());
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
        let mut limited = stream.take(WEBHOOK_HTTP_RESPONSE_MAX_BYTES.saturating_add(1));
        let read_result = tokio::time::timeout(timeouts.read, limited.read_to_end(&mut buf))
            .await
            .map_err(|_| io_timeout_error("tcp read", timeouts.read))?;
        read_result?;
        ensure_webhook_http_response_is_bounded(&buf)?;
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
    }
}
fn ensure_webhook_http_response_is_bounded(bytes: &[u8]) -> std::io::Result<()> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > WEBHOOK_HTTP_RESPONSE_MAX_BYTES {
        return Err(std::io::Error::other(format!(
            "webhook response exceeded the {WEBHOOK_HTTP_RESPONSE_MAX_BYTES}-byte protocol limit"
        )));
    }
    Ok(())
}
#[cfg(feature = "app_api_https")]
async fn http_post_https(
    url: &Url,
    connect_addrs: &[SocketAddr],
    headers: &[(&str, String)],
    body: &[u8],
) -> std::io::Result<u16> {
    use reqwest::header::{HeaderName, HeaderValue};
    let mut client_builder = reqwest::Client::builder()
        .timeout(
            http_timeout_config().connect
                + http_timeout_config().write
                + http_timeout_config().read,
        )
        .http1_only();
    if let Some((domain, pinned_addrs)) = https_delivery_dns_override(url, connect_addrs) {
        client_builder = client_builder.resolve_to_addrs(&domain, &pinned_addrs);
    }
    let client = client_builder
        .build()
        .map_err(|e| std::io::Error::other(format!("https client build: {e}")))?;
    let mut req = client
        .post(url.as_str())
        .header("User-Agent", "iroha-torii-webhook/1")
        .header("Connection", "close");
    for (k, v) in headers {
        if let Ok(name) = HeaderName::from_str(k) {
            if let Ok(value) = HeaderValue::from_str(v) {
                req = req.header(name, value);
            }
        }
    }
    let resp = req
        .body(body.to_vec())
        .send()
        .await
        .map_err(|e| std::io::Error::other(format!("https req: {e}")))?;
    Ok(resp.status().as_u16())
}
async fn http_post(url: &str, headers: &[(&str, String)], body: &[u8]) -> std::io::Result<u16> {
    #[cfg(test)]
    if let Some(handler) = http_post_override_handler() {
        return handler(url, headers, body);
    }
    let parsed = Url::parse(url).map_err(|e| io_invalid_input(format!("bad url: {e}")))?;
    let scheme = parsed.scheme();
    let policy = webhook_security_policy();
    if scheme == "https" {
        #[cfg(feature = "app_api_https")]
        {
            let connect_addrs = if policy.enabled {
                resolve_destination_addrs(&parsed, &policy).await?
            } else {
                Vec::new()
            };
            return http_post_https(&parsed, &connect_addrs, headers, body).await;
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
    if scheme == "wss" || scheme == "ws" {
        let connect_addrs = if scheme == "ws" || policy.enabled {
            resolve_destination_addrs(&parsed, &policy).await?
        } else {
            Vec::new()
        };
        let connect_addr = websocket_pinned_connect_addr(&parsed, &policy, &connect_addrs);
        return ws_send(&parsed, connect_addr, headers, body).await;
    }
    #[cfg(not(feature = "app_api_wss"))]
    if scheme == "wss" || scheme == "ws" {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "WS/WSS not supported; enable feature app_api_wss",
        ));
    }
    if scheme != "http" {
        return Err(io_invalid_input(format!(
            "unsupported webhook scheme `{scheme}`"
        )));
    }
    let addrs = resolve_destination_addrs(&parsed, &policy).await?;
    let Some(connect_addr) = addrs.into_iter().next() else {
        return Err(io_invalid_input(
            "webhook destination resolved to no addresses",
        ));
    };
    let host_header = host_header_value(&parsed)?;
    http_post_plain(&parsed, connect_addr, &host_header, headers, body).await
}
#[cfg(feature = "app_api_wss")]
async fn ws_send(
    url: &Url,
    connect_addr: Option<SocketAddr>,
    headers: &[(&str, String)],
    body: &[u8],
) -> std::io::Result<u16> {
    use futures::SinkExt as _;
    use std::str::FromStr;
    use tokio_tungstenite::{client_async_tls_with_config, connect_async};
    use tungstenite::{Message, client::IntoClientRequest, http::HeaderName};
    let mut req = url.as_str().into_client_request().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("bad url: {e}"))
    })?;
    for (k, v) in headers {
        if let Ok(name) = HeaderName::from_str(k) {
            if let Ok(val) = v.parse() {
                req.headers_mut().insert(name, val);
            }
        }
    }
    let (mut ws, _resp) = match connect_addr {
        Some(addr) => {
            use tokio::net::TcpStream;
            let timeouts = http_timeout_config();
            let stream = tokio::time::timeout(timeouts.connect, TcpStream::connect(addr))
                .await
                .map_err(|_| io_timeout_error("tcp connect", timeouts.connect))??;
            client_async_tls_with_config(req, stream, None, None)
                .await
                .map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, format!("ws connect: {e}"))
                })?
        }
        None => connect_async(req).await.map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("ws connect: {e}"))
        })?,
    };
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
            if matches!(
                e.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::InvalidInput
            ) {
                iroha_logger::warn!(
                    %e,
                    url=%pd.url,
                    "dropping webhook payload due to disallowed destination"
                );
                return true;
            }
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
struct QueueScanState {
    root: PathBuf,
    capacity: usize,
    entries: fs::ReadDir,
    retained: usize,
}
#[derive(Default)]
struct QueueScanCursor {
    state: Option<QueueScanState>,
}
struct QueueScanBatch {
    paths: Vec<PathBuf>,
    overflow_paths: Vec<PathBuf>,
    sweep_complete: bool,
}
fn queue_scan_cursor() -> &'static Mutex<QueueScanCursor> {
    static CURSOR: OnceLock<Mutex<QueueScanCursor>> = OnceLock::new();
    CURSOR.get_or_init(|| Mutex::new(QueueScanCursor::default()))
}
fn discover_queue_batch_at(
    cursor: &mut QueueScanCursor,
    root: &Path,
    capacity: usize,
    batch_limit: usize,
    work_limit: usize,
) -> std::io::Result<QueueScanBatch> {
    if cursor
        .state
        .as_ref()
        .is_none_or(|state| state.root != root || state.capacity != capacity)
    {
        cursor.state = Some(QueueScanState {
            root: root.to_path_buf(),
            capacity,
            entries: fs::read_dir(root)?,
            retained: 0,
        });
    }
    let mut paths = Vec::with_capacity(batch_limit);
    let mut overflow_paths = Vec::new();
    let mut work = 0_usize;
    let mut sweep_complete = false;
    while paths.len().saturating_add(overflow_paths.len()) < batch_limit && work < work_limit {
        let next = cursor
            .state
            .as_mut()
            .expect("queue scan state initialized")
            .entries
            .next();
        let Some(entry) = next else {
            cursor.state = None;
            sweep_complete = true;
            break;
        };
        work = work.saturating_add(1);
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                cursor.state = None;
                return Err(err);
            }
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let state = cursor
            .state
            .as_mut()
            .expect("queue scan state remains initialized");
        if state.retained < capacity {
            state.retained = state.retained.saturating_add(1);
            paths.push(path);
        } else {
            // Overflow records are removed without reading or decoding them.
            overflow_paths.push(path);
        }
    }
    paths.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    overflow_paths.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    Ok(QueueScanBatch {
        paths,
        overflow_paths,
        sweep_complete,
    })
}
fn discover_queue_batch(policy: WebhookPolicy) -> std::io::Result<QueueScanBatch> {
    let mut cursor = lock_unpoisoned(queue_scan_cursor());
    discover_queue_batch_at(
        &mut cursor,
        &queue_dir(),
        effective_queue_capacity(policy),
        WEBHOOK_QUEUE_SCAN_BATCH_SIZE,
        WEBHOOK_QUEUE_SCAN_WORK_ITEMS,
    )
}
fn prune_verified_queue_overflow(
    paths: Vec<PathBuf>,
    policy: WebhookPolicy,
) -> std::io::Result<usize> {
    if paths.is_empty() {
        return Ok(0);
    }
    // Hold admission while re-counting and pruning. Files may have been
    // delivered since this streaming cursor classified the paths, so only the
    // currently verified excess is removed.
    let _guard = lock_unpoisoned(queue_write_lock());
    let capacity = effective_queue_capacity(policy);
    let observed = queue_depth_bounded_at(
        &queue_dir(),
        capacity.saturating_add(paths.len()),
        WEBHOOK_QUEUE_ADMISSION_SCAN_WORK_ITEMS,
    )?;
    let mut remaining_excess = observed.saturating_sub(capacity).min(paths.len());
    let mut removed = 0_usize;
    for path in paths {
        if remaining_excess == 0 {
            break;
        }
        match fs::remove_file(&path) {
            Ok(()) => {
                remaining_excess = remaining_excess.saturating_sub(1);
                removed = removed.saturating_add(1);
                iroha_logger::warn!(
                    ?path,
                    capacity,
                    "removed webhook queue record beyond hard capacity"
                );
            }
            Err(err) => {
                iroha_logger::warn!(%err, ?path, "failed to remove excess webhook payload");
            }
        }
    }
    Ok(removed)
}
async fn read_queue_file_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt as _;
    let metadata = tokio_fs::symlink_metadata(path).await?;
    let maximum = u64::try_from(WEBHOOK_QUEUE_FILE_MAX_BYTES).unwrap_or(u64::MAX);
    if !metadata.file_type().is_file() || metadata.len() > maximum {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "webhook spool record is oversized or non-regular",
        ));
    }
    let file = tokio_fs::File::open(path).await?;
    let capacity = usize::try_from(metadata.len())
        .unwrap_or(WEBHOOK_QUEUE_FILE_MAX_BYTES)
        .min(WEBHOOK_QUEUE_FILE_MAX_BYTES);
    let mut bytes = Vec::with_capacity(capacity);
    let mut limited = file.take(maximum.saturating_add(1));
    limited.read_to_end(&mut bytes).await?;
    if bytes.len() > WEBHOOK_QUEUE_FILE_MAX_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "webhook spool record exceeds hard byte limit",
        ));
    }
    Ok(bytes)
}
fn decode_pending_delivery(bytes: &[u8]) -> Option<PendingDelivery> {
    if bytes.len() > WEBHOOK_QUEUE_FILE_MAX_BYTES {
        return None;
    }
    let norito::json::Value::Object(map) =
        norito::json::from_slice::<norito::json::Value>(bytes).ok()?
    else {
        return None;
    };
    let id = map.get("id")?.as_str()?;
    let webhook_id = map.get("webhook_id")?.as_u64()?;
    let url = map.get("url")?.as_str()?;
    let content_type = map.get("content_type")?.as_str()?;
    if !delivery_metadata_is_bounded(id, url, content_type) {
        return None;
    }
    let encoded_body = map.get("body")?.as_str()?;
    if encoded_body.len() > WEBHOOK_DELIVERY_MAX_BASE64_BYTES {
        return None;
    }
    let body = STANDARD.decode(encoded_body).ok()?;
    if body.len() > WEBHOOK_DELIVERY_MAX_BYTES {
        return None;
    }
    let attempts = match map.get("attempts") {
        None => 0,
        Some(value) => u32::try_from(value.as_u64()?).ok()?,
    };
    let next_attempt_ms = map
        .get("next_attempt_ms")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    Some(PendingDelivery {
        id: id.to_string(),
        webhook_id,
        url: url.to_string(),
        content_type: content_type.to_string(),
        body,
        attempts,
        next_attempt_ms,
    })
}
async fn process_queue_once() -> Duration {
    let policy = webhook_policy();
    let batch = match discover_queue_batch(policy) {
        Ok(batch) => batch,
        Err(err) => {
            iroha_logger::warn!(%err, "failed to iterate webhook queue directory");
            return Duration::from_secs(5);
        }
    };
    let batch_had_entries = !batch.paths.is_empty() || !batch.overflow_paths.is_empty();
    if let Err(err) = prune_verified_queue_overflow(batch.overflow_paths, policy) {
        iroha_logger::warn!(%err, "failed to verify webhook queue overflow");
    }
    let mut next_due = None;
    for path in batch.paths {
        let bytes = match read_queue_file_bounded(&path).await {
            Ok(bytes) => bytes,
            Err(e) => {
                iroha_logger::warn!(%e, ?path, "failed to read pending webhook delivery");
                if e.kind() == std::io::ErrorKind::InvalidData {
                    if let Err(remove_err) = tokio_fs::remove_file(&path).await {
                        iroha_logger::warn!(
                            %remove_err,
                            ?path,
                            "failed to remove invalid webhook payload"
                        );
                    }
                }
                continue;
            }
        };
        let mut pd = match decode_pending_delivery(&bytes) {
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
            let delay = Duration::from_millis(pd.next_attempt_ms.saturating_sub(now_ms));
            next_due = Some(next_due.map_or(delay, |current: Duration| current.min(delay)));
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
        let secret = lock_registry()
            .items
            .get(&pd.webhook_id)
            .cloned()
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
            match encode_pending_delivery(&pd) {
                Ok(encoded) => {
                    if let Err(e) = tokio_fs::write(&path, encoded.as_bytes()).await {
                        iroha_logger::warn!(
                            %e,
                            ?path,
                            "failed to persist pending webhook delivery"
                        );
                    }
                }
                Err(err) => {
                    iroha_logger::warn!(
                        %err,
                        ?path,
                        "dropping webhook delivery that exceeded spool bounds"
                    );
                    if let Err(remove_err) = tokio_fs::remove_file(&path).await {
                        iroha_logger::warn!(
                            %remove_err,
                            ?path,
                            "failed to remove oversized webhook payload"
                        );
                    }
                }
            }
        }
    }
    if batch.sweep_complete {
        next_due
            .unwrap_or(Duration::from_secs(1))
            .min(Duration::from_secs(1))
    } else if batch_had_entries {
        Duration::ZERO
    } else {
        // A work-bounded scan containing only unrelated files must yield
        // before continuing the persistent directory cursor.
        Duration::from_millis(1)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestDataDirGuard;
    use http_body_util::BodyExt as _;
    use iroha_crypto::Hash;
    use iroha_data_model::events::EventFilter; // bring .matches()
    use iroha_data_model::events::{
        EventBox,
        pipeline::{TransactionEvent, TransactionStatus},
    };
    use std::{
        convert::TryFrom,
        fs,
        sync::{Arc, Barrier, Mutex, MutexGuard},
    };
    use tokio::{
        runtime::Runtime,
        time::{Duration, sleep},
    };
    fn registry_entry(id: u64, url: String) -> WebhookEntry {
        WebhookEntry {
            id,
            url,
            active: true,
            secret: None,
            filter: None,
        }
    }
    #[test]
    fn webhook_registry_rejects_entry_and_count_overflow() {
        let mut registry = RegistryInner::default();
        let oversized = registry_entry(1, "x".repeat(WEBHOOK_ENTRY_MAX_BYTES));
        assert!(!registry_can_retain(&registry, &oversized));
        let compact = registry_entry(1, "https://example.com/hook".to_string());
        for id in 0..WEBHOOK_REGISTRY_MAX_ENTRIES {
            registry
                .items
                .insert(u64::try_from(id).expect("id fits"), compact.clone());
        }
        assert!(!registry_can_retain(&registry, &compact));
    }
    #[test]
    fn persisted_webhook_with_malformed_filter_is_skipped_instead_of_widened() {
        let _env = TestDataDirGuard::new();
        {
            let mut registry = lock_registry();
            registry.next_id = 0;
            registry.items.clear();
        }
        let mut malformed = webhook_entry_to_storage_json(&registry_entry(
            7,
            "https://filtered.example/hook".to_owned(),
        ));
        let norito::json::Value::Object(ref mut fields) = malformed else {
            panic!("stored webhook entry must be an object");
        };
        fields.insert(
            "filter".into(),
            norito::json::Value::from("not-a-filter-expression"),
        );
        let valid = webhook_entry_to_storage_json(&WebhookEntry {
            id: 2,
            url: "https://valid-filter.example/hook".to_owned(),
            active: true,
            secret: None,
            filter: Some(crate::filter::FilterExpr::Eq(
                crate::filter::FieldPath("status".to_owned()),
                norito::json::Value::from("Approved"),
            )),
        });
        fs::create_dir_all(data_dir()).expect("create webhook data directory");
        let body =
            norito::json::to_json_pretty(&norito::json::Value::Array(vec![malformed, valid]))
                .expect("encode persisted webhook registry");
        fs::write(registry_path(), body).expect("write persisted webhook registry");
        load_registry();
        let mut registry = lock_registry();
        assert!(
            !registry.items.contains_key(&7),
            "a malformed stored filter must not become an unfiltered webhook"
        );
        assert!(
            registry.items.contains_key(&2),
            "a valid neighboring webhook must still load"
        );
        assert!(
            registry
                .items
                .get(&2)
                .is_some_and(|entry| entry.filter.is_some()),
            "the valid neighboring webhook must retain its filter"
        );
        assert_eq!(
            registry.next_id, 7,
            "a quarantined webhook ID must not be recycled",
        );
        registry.next_id = 0;
        registry.items.clear();
    }
    #[test]
    fn webhook_http_response_bound_rejects_limit_plus_one() {
        let maximum = usize::try_from(WEBHOOK_HTTP_RESPONSE_MAX_BYTES).expect("limit fits");
        assert!(ensure_webhook_http_response_is_bounded(&vec![0_u8; maximum]).is_ok());
        let error = ensure_webhook_http_response_is_bounded(&vec![0_u8; maximum + 1])
            .expect_err("limit plus one must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
    }
    #[test]
    fn webhook_delivery_body_bound_accepts_limit_and_rejects_limit_plus_one() {
        let mut pending = PendingDelivery {
            id: "body-boundary".to_string(),
            webhook_id: 1,
            url: "http://example.test/webhook".to_string(),
            content_type: "application/octet-stream".to_string(),
            body: vec![0xA5; WEBHOOK_DELIVERY_MAX_BYTES],
            attempts: 0,
            next_attempt_ms: 0,
        };
        let encoded = encode_pending_delivery(&pending).expect("boundary body must encode");
        let decoded =
            decode_pending_delivery(encoded.as_bytes()).expect("boundary body must decode");
        assert_eq!(decoded.body.len(), WEBHOOK_DELIVERY_MAX_BYTES);
        pending.body.push(0);
        let error = encode_pending_delivery(&pending).expect_err("limit plus one must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        pending.body.clear();
        pending.content_type = "x".repeat(WEBHOOK_DELIVERY_METADATA_MAX_BYTES + 1);
        let error = encode_pending_delivery(&pending).expect_err("metadata overflow must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
    #[test]
    fn webhook_spool_decode_rejects_encoded_body_overflow() {
        let mut payload = norito::json::Map::new();
        payload.insert("id".into(), norito::json::Value::from("encoded-overflow"));
        payload.insert("webhook_id".into(), norito::json::Value::from(1_u64));
        payload.insert(
            "url".into(),
            norito::json::Value::from("http://example.test/webhook"),
        );
        payload.insert(
            "content_type".into(),
            norito::json::Value::from("application/octet-stream"),
        );
        payload.insert(
            "body".into(),
            norito::json::Value::from("A".repeat(WEBHOOK_DELIVERY_MAX_BASE64_BYTES + 4)),
        );
        payload.insert("attempts".into(), norito::json::Value::from(0_u64));
        payload.insert("next_attempt_ms".into(), norito::json::Value::from(0_u64));
        let record = norito::json::to_vec(&payload).expect("encode overflow record");
        assert!(record.len() <= WEBHOOK_QUEUE_FILE_MAX_BYTES);
        assert!(
            decode_pending_delivery(&record).is_none(),
            "encoded body overflow must be rejected before base64 decode"
        );
    }
    #[test]
    fn webhook_queue_capacity_has_a_hard_ceiling() {
        let policy = WebhookPolicy {
            queue_capacity: NonZeroUsize::new(WEBHOOK_QUEUE_HARD_CAPACITY + 1)
                .expect("hard capacity plus one is non-zero"),
            ..WebhookPolicy::default()
        };
        assert_eq!(
            effective_queue_capacity(policy),
            WEBHOOK_QUEUE_HARD_CAPACITY
        );
    }
    #[test]
    fn queue_admission_scan_fails_closed_at_work_limit() {
        let _env = TestDataDirGuard::new();
        let root = queue_dir();
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create queue directory");
        for name in ["noise-1", "noise-2", "noise-3"] {
            fs::write(root.join(name), b"").expect("write queue noise");
        }
        let error = queue_depth_bounded_at(&root, 1, 2)
            .expect_err("work exhaustion must fail queue admission closed");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
    }
    #[test]
    fn queue_discovery_sorts_each_bounded_batch() {
        let _env = TestDataDirGuard::new();
        let root = queue_dir();
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create queue directory");
        for name in ["0003.json", "0001.json", "0002.json"] {
            fs::write(root.join(name), b"{}").expect("write queue entry");
        }
        let mut cursor = QueueScanCursor::default();
        let batch =
            discover_queue_batch_at(&mut cursor, &root, 3, 4, 4).expect("discover queue batch");
        let names: Vec<_> = batch
            .paths
            .iter()
            .map(|path| {
                path.file_name()
                    .expect("file name")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            names,
            ["0001.json", "0002.json", "0003.json"].map(str::to_string)
        );
        assert!(batch.overflow_paths.is_empty());
        assert!(batch.sweep_complete);
    }
    #[test]
    fn queue_discovery_bounds_batches_and_marks_capacity_overflow() {
        let _env = TestDataDirGuard::new();
        let root = queue_dir();
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create queue directory");
        for name in ["0001.json", "0002.json", "0003.json"] {
            fs::write(root.join(name), b"{}").expect("write queue entry");
        }
        let mut cursor = QueueScanCursor::default();
        let first = discover_queue_batch_at(&mut cursor, &root, 2, 2, 3)
            .expect("discover first queue batch");
        assert_eq!(
            first.paths.len() + first.overflow_paths.len(),
            2,
            "a scan batch must not retain more paths than its bound"
        );
        assert!(!first.sweep_complete);
        let second = discover_queue_batch_at(&mut cursor, &root, 2, 2, 3)
            .expect("discover second queue batch");
        assert_eq!(second.paths.len() + second.overflow_paths.len(), 1);
        assert_eq!(
            first.overflow_paths.len() + second.overflow_paths.len(),
            1,
            "records beyond capacity must be marked before replay"
        );
        assert!(second.sweep_complete);
    }
    #[test]
    fn queue_overflow_pruning_rechecks_current_capacity() {
        let _env = TestDataDirGuard::new();
        let root = queue_dir();
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create queue directory");
        let first = root.join("0001.json");
        let second = root.join("0002.json");
        fs::write(&first, b"{}").expect("write first queue entry");
        fs::write(&second, b"{}").expect("write second queue entry");
        let policy = WebhookPolicy {
            queue_capacity: NonZeroUsize::new(2).expect("non-zero capacity"),
            ..WebhookPolicy::default()
        };
        assert_eq!(
            prune_verified_queue_overflow(vec![second.clone()], policy)
                .expect("verify queue at capacity"),
            0
        );
        assert!(second.exists(), "a current in-capacity record must remain");
        let overflow = root.join("0003.json");
        fs::write(&overflow, b"{}").expect("write overflow queue entry");
        assert_eq!(
            prune_verified_queue_overflow(vec![overflow.clone()], policy)
                .expect("prune verified overflow"),
            1
        );
        assert!(!overflow.exists(), "verified overflow must be removed");
        assert_eq!(queue_depth_bounded_at(&root, 3, 3).unwrap(), 2);
    }
    #[test]
    fn delivery_worker_removes_oversized_spool_file_before_decode() {
        let _env = TestDataDirGuard::new();
        let root = queue_dir();
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create queue directory");
        let oversized = root.join("oversized.json");
        let file = fs::File::create(&oversized).expect("create oversized queue file");
        file.set_len(
            u64::try_from(WEBHOOK_QUEUE_FILE_MAX_BYTES)
                .expect("file bound fits u64")
                .saturating_add(1),
        )
        .expect("extend oversized queue file");
        let _ = Runtime::new()
            .expect("tokio runtime")
            .block_on(process_queue_once());
        assert!(!oversized.exists(), "oversized spool file must be removed");
    }
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
    struct WebhookPolicyGuard {
        previous: super::WebhookPolicy,
        _writer_guard: MutexGuard<'static, ()>,
    }
    impl WebhookPolicyGuard {
        fn new(policy: super::WebhookPolicy) -> Self {
            let writer_guard = super::webhook_policy_writer_lock()
                .lock()
                .expect("webhook policy writer lock");
            let previous = super::webhook_policy();
            super::apply_webhook_policy(policy);
            Self {
                previous,
                _writer_guard: writer_guard,
            }
        }
    }
    impl Drop for WebhookPolicyGuard {
        fn drop(&mut self) {
            super::apply_webhook_policy(self.previous);
        }
    }
    fn expect_json_object(value: norito::json::Value, context: &str) -> norito::json::Map {
        match value {
            norito::json::Value::Object(map) => map,
            _ => panic!("expected object for {context}", context = context),
        }
    }
    #[test]
    fn registry_lock_recovers_after_a_guard_unwinds() {
        let mutex = Mutex::new(0_u8);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut guard = mutex.lock().expect("fresh test mutex");
            *guard = 7;
            panic!("poison the local test mutex");
        }));
        assert!(unwind.is_err());
        let mut recovered = super::lock_unpoisoned(&mutex);
        assert_eq!(*recovered, 7);
        *recovered = 8;
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
    fn queue_capacity_check_and_persistence_are_atomic() {
        const WRITERS: usize = 8;
        let _env = TestDataDirGuard::new();
        let _ = fs::remove_dir_all(super::queue_dir());
        super::ensure_dirs();
        let policy = super::WebhookPolicy {
            queue_capacity: NonZeroUsize::new(1).unwrap(),
            max_attempts: NonZeroU32::new(3).unwrap(),
            backoff_initial: Duration::from_secs(1),
            backoff_max: Duration::from_secs(1),
            connect_timeout: Duration::from_secs(1),
            write_timeout: Duration::from_secs(1),
            read_timeout: Duration::from_secs(1),
        };
        let barrier = Arc::new(Barrier::new(WRITERS));
        let handles: Vec<_> = (0..WRITERS)
            .map(|writer| {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    let mut admission = QueueAdmission::begin(policy)?;
                    admission.persist(&PendingDelivery {
                        id: format!("writer-{writer}"),
                        webhook_id: u64::try_from(writer).expect("writer id fits u64"),
                        url: "http://example.test/webhook".to_string(),
                        content_type: "text/plain".to_string(),
                        body: format!("payload-{writer}").into_bytes(),
                        attempts: 0,
                        next_attempt_ms: 0,
                    })
                })
            })
            .collect();
        let mut persisted = 0_usize;
        for handle in handles {
            if handle.join().expect("queue writer thread").is_ok() {
                persisted = persisted.saturating_add(1);
            }
        }
        assert_eq!(persisted, 1, "exactly one writer should reserve capacity");
        assert_eq!(
            super::queue_depth(),
            1,
            "concurrent writers must not overshoot queue capacity"
        );
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
    fn overflowing_persisted_attempts_are_removed_without_delivery() {
        let _env = TestDataDirGuard::new();
        let _ = fs::remove_dir_all(super::queue_dir());
        super::ensure_dirs();
        let pending_path = super::queue_dir().join("overflowing-attempts.json");
        let mut payload = norito::json::Map::new();
        payload.insert(
            "id".into(),
            norito::json::Value::from("overflowing-attempts"),
        );
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
        payload.insert(
            "attempts".into(),
            norito::json::Value::from(u64::from(u32::MAX) + 1),
        );
        payload.insert("next_attempt_ms".into(), norito::json::Value::from(0u64));
        let json = norito::json::to_json_pretty(&payload).expect("serialize pending payload");
        fs::write(&pending_path, json.as_bytes()).expect("write pending payload");
        let delivery_attempts = Arc::new(AtomicU32::new(0));
        let recorded_attempts = Arc::clone(&delivery_attempts);
        let _http_guard = super::install_http_post_override(move |_, _, _| {
            recorded_attempts.fetch_add(1, Ordering::SeqCst);
            Ok(200)
        });
        let rt = Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            super::process_queue_once().await;
        });
        assert!(
            !pending_path.exists(),
            "invalid spool record must be removed"
        );
        assert_eq!(
            delivery_attempts.load(Ordering::SeqCst),
            0,
            "overflow must not reset the retry budget and trigger delivery"
        );
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
            dataspace_id: DataSpaceId::UNIVERSAL,
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
        use crate::filter::{FieldPath, FilterExpr};
        use iroha_data_model::events::data::{
            prelude::DataEvent,
            proof::{ProofEvent, ProofVerified},
        };
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
    #[test]
    fn webhook_url_validation_rejects_localhost_when_enabled() {
        let policy = WebhookSecurityPolicy {
            enabled: true,
            allow_nets: Vec::new(),
        };
        let err = super::validate_webhook_url_for_create("http://localhost/callback", &policy)
            .expect_err("localhost must be rejected");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }
    #[test]
    fn webhook_url_validation_allows_localhost_when_disabled() {
        let policy = WebhookSecurityPolicy {
            enabled: false,
            allow_nets: Vec::new(),
        };
        super::validate_webhook_url_for_create("http://localhost/callback", &policy)
            .expect("localhost allowed when guard rails disabled");
    }
    #[test]
    fn webhook_url_validation_rejects_private_ip_literal_when_enabled() {
        let policy = WebhookSecurityPolicy {
            enabled: true,
            allow_nets: Vec::new(),
        };
        let err = super::validate_webhook_url_for_create("http://127.0.0.1:8080/callback", &policy)
            .expect_err("loopback must be rejected");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }
    #[test]
    fn webhook_url_validation_allows_allowlisted_ip_literal_when_enabled() {
        let allow = crate::limits::parse_cidr("127.0.0.1/32").expect("valid cidr");
        let policy = WebhookSecurityPolicy {
            enabled: true,
            allow_nets: vec![allow],
        };
        super::validate_webhook_url_for_create("http://127.0.0.1:8080/callback", &policy)
            .expect("allow-listed loopback allowed");
    }
    #[test]
    fn webhook_delivery_guard_rejects_private_ip_literal_when_enabled() {
        let policy = WebhookSecurityPolicy {
            enabled: true,
            allow_nets: Vec::new(),
        };
        let url = Url::parse("http://127.0.0.1:1/callback").expect("valid url");
        let rt = Runtime::new().expect("tokio runtime");
        let err = rt
            .block_on(super::resolve_destination_addrs(&url, &policy))
            .expect_err("private destination rejected");
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    }
    #[cfg(feature = "app_api_https")]
    #[test]
    fn https_delivery_dns_override_pins_vetted_domain_addresses() {
        let url = Url::parse("https://example.test/hook").expect("valid url");
        let addrs = vec![
            "203.0.113.10:443".parse().expect("addr"),
            "203.0.113.11:443".parse().expect("addr"),
        ];
        let override_addrs =
            super::https_delivery_dns_override(&url, &addrs).expect("domain override");
        assert_eq!(override_addrs.0, "example.test");
        assert_eq!(override_addrs.1, addrs);
    }
    #[cfg(feature = "app_api_https")]
    #[test]
    fn https_delivery_dns_override_skips_ip_literals() {
        let url = Url::parse("https://203.0.113.10/hook").expect("valid url");
        let addrs = vec!["203.0.113.10:443".parse().expect("addr")];
        assert!(
            super::https_delivery_dns_override(&url, &addrs).is_none(),
            "ip-literal URLs should not install a DNS override"
        );
    }
    #[cfg(feature = "app_api_wss")]
    #[test]
    fn websocket_pinned_connect_addr_pins_secure_delivery_when_guarded() {
        let policy = WebhookSecurityPolicy {
            enabled: true,
            allow_nets: Vec::new(),
        };
        let url = Url::parse("wss://example.test/socket").expect("valid url");
        let addrs = vec!["203.0.113.20:443".parse().expect("addr")];
        assert_eq!(
            super::websocket_pinned_connect_addr(&url, &policy, &addrs),
            addrs.first().copied()
        );
    }
}
