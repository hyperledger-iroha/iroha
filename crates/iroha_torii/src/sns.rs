//! Ledger-backed Sora Name Service (SNS) HTTP handlers.
use crate::{JsonBody, SharedAppState};
use axum::{
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use iroha_core::{
    sns::{
        SnsError as CoreSnsError, SnsNamespace, get_name_record_by_selector, policy_by_id,
        selector_for_namespace_literal,
    },
    state::{StateReadOnly, StateReadOnlyWithTransactions},
};
use iroha_data_model::sns::{NameRecordV1, NameSelectorV1, NameStatus, SuffixId};
use parking_lot::Mutex;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
const SNS_NAME_CACHE_MAX_ENTRIES: usize = 4096;
/// HTTP-friendly error wrapper for SNS routes.
#[derive(Debug)]
pub enum SnsError {
    /// Entity was not found.
    NotFound(String),
    /// Request failed validation.
    BadRequest(String),
    /// Request conflicts with existing state.
    Conflict(String),
    /// Internal state mutation failed.
    Internal(String),
    /// Torii access or request-rate policy rejected the request.
    Access(crate::Error),
}
impl From<CoreSnsError> for SnsError {
    fn from(error: CoreSnsError) -> Self {
        match error {
            CoreSnsError::NotFound(msg) => Self::NotFound(msg),
            CoreSnsError::BadRequest(msg) => Self::BadRequest(msg),
            CoreSnsError::Conflict(msg) => Self::Conflict(msg),
            CoreSnsError::Internal(msg) => Self::Internal(msg),
        }
    }
}
impl IntoResponse for SnsError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::Conflict(msg) => (StatusCode::CONFLICT, msg),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            Self::Access(error) => return error.into_response(),
        };
        (status, message).into_response()
    }
}
fn current_ledger_time_ms_from_latest_block(latest_block_ms: u64) -> u64 {
    let wall_clock_ms = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(latest_block_ms);
    wall_clock_ms.max(latest_block_ms)
}
fn sns_name_record_cache_valid_until_ms(record: &NameRecordV1) -> Option<u64> {
    match &record.status {
        NameStatus::Active => Some(record.expires_at_ms),
        NameStatus::GracePeriod => Some(record.grace_expires_at_ms),
        NameStatus::Redemption => Some(record.redemption_expires_at_ms),
        NameStatus::Frozen(frozen) => Some(frozen.until_ms),
        NameStatus::Tombstoned(_) => None,
    }
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct SnsNameRecordCacheKey {
    suffix_id: SuffixId,
    label: String,
}
impl SnsNameRecordCacheKey {
    fn from_selector(selector: &NameSelectorV1) -> Self {
        Self {
            suffix_id: selector.suffix_id,
            label: selector.normalized_label().to_owned(),
        }
    }
}
#[derive(Clone, Debug)]
enum CachedSnsNameRecord {
    Found(NameRecordV1),
    NotFound(String),
}
#[derive(Clone, Debug)]
struct SnsNameRecordCacheEntry {
    block_height: u64,
    block_hash: Option<String>,
    valid_until_ms: Option<u64>,
    outcome: CachedSnsNameRecord,
}
/// Per-Torii cache for SNS name reads. Entries are valid only for the latest
/// block identity observed when the lookup was performed. Found records also
/// expire at the next lifecycle deadline.
#[derive(Debug, Default)]
pub(crate) struct SnsNameRecordCache {
    inner: Mutex<SnsNameRecordCacheInner>,
}
#[derive(Debug, Default)]
struct SnsNameRecordCacheInner {
    entries: HashMap<SnsNameRecordCacheKey, SnsNameRecordCacheEntry>,
}
impl SnsNameRecordCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    fn get(
        &self,
        key: &SnsNameRecordCacheKey,
        block_height: u64,
        block_hash: Option<&str>,
        now_ms: u64,
    ) -> Option<Result<NameRecordV1, SnsError>> {
        self.inner.lock().get(key, block_height, block_hash, now_ms)
    }
    fn insert_found(
        &self,
        key: SnsNameRecordCacheKey,
        block_height: u64,
        block_hash: Option<String>,
        valid_until_ms: Option<u64>,
        record: NameRecordV1,
    ) {
        self.insert(
            key,
            block_height,
            block_hash,
            valid_until_ms,
            CachedSnsNameRecord::Found(record),
        );
    }
    fn insert_not_found(
        &self,
        key: SnsNameRecordCacheKey,
        block_height: u64,
        block_hash: Option<String>,
        message: String,
    ) {
        self.insert(
            key,
            block_height,
            block_hash,
            None,
            CachedSnsNameRecord::NotFound(message),
        );
    }
    fn insert(
        &self,
        key: SnsNameRecordCacheKey,
        block_height: u64,
        block_hash: Option<String>,
        valid_until_ms: Option<u64>,
        outcome: CachedSnsNameRecord,
    ) {
        self.inner.lock().insert(
            key,
            SnsNameRecordCacheEntry {
                block_height,
                block_hash,
                valid_until_ms,
                outcome,
            },
        );
    }
    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().entries.len()
    }
}
impl SnsNameRecordCacheInner {
    fn get(
        &mut self,
        key: &SnsNameRecordCacheKey,
        block_height: u64,
        block_hash: Option<&str>,
        now_ms: u64,
    ) -> Option<Result<NameRecordV1, SnsError>> {
        let entry = self.entries.get(key)?;
        if entry.block_height != block_height || entry.block_hash.as_deref() != block_hash {
            self.entries.remove(key);
            return None;
        }
        if matches!(&entry.outcome, CachedSnsNameRecord::Found(_))
            && entry
                .valid_until_ms
                .is_some_and(|valid_until_ms| now_ms >= valid_until_ms)
        {
            self.entries.remove(key);
            return None;
        }
        Some(match entry.outcome.clone() {
            CachedSnsNameRecord::Found(record) => Ok(record),
            CachedSnsNameRecord::NotFound(message) => Err(SnsError::NotFound(message)),
        })
    }
    fn insert(&mut self, key: SnsNameRecordCacheKey, entry: SnsNameRecordCacheEntry) {
        let evicted = if !self.entries.contains_key(&key)
            && self.entries.len() >= SNS_NAME_CACHE_MAX_ENTRIES
        {
            self.entries.keys().next().cloned()
        } else {
            None
        };
        if let Some(evicted) = evicted {
            // Replace one entry instead of flushing the whole cache. The
            // process-local hash seed makes targeted victim selection
            // impractical, and a miss cannot invalidate unrelated hot keys en
            // masse.
            self.entries.remove(&evicted);
        }
        self.entries.insert(key, entry);
    }
}
async fn run_blocking_sns<T>(
    op: &'static str,
    job: impl FnOnce() -> Result<T, SnsError> + Send + 'static,
) -> Result<T, SnsError>
where
    T: Send + 'static,
{
    crate::panic_recovery::join_recoverable(crate::panic_recovery::spawn_blocking_recoverable(job))
        .await
        .map_err(|err| SnsError::Internal(format!("SNS {op} worker failed to join: {err}")))?
}
/// Handle `GET /v1/sns/names/{namespace}/{literal}`.
pub async fn handle_get_name(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
) -> Result<impl IntoResponse, SnsError> {
    crate::check_access(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/sns/names/{namespace}/{literal}",
    )
    .await
    .map_err(SnsError::Access)?;
    let app_for_job = Arc::clone(&app);
    let cache = Arc::clone(&app.sns_name_cache);
    let started_at = std::time::Instant::now();
    let record = run_blocking_sns("get_name", move || {
        let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
        let namespace_path = namespace.as_path().to_owned();
        let view = app_for_job.state.view();
        let latest_block = view.latest_block();
        let block_height = latest_block
            .as_ref()
            .map_or(0, |block| block.header().height().get());
        let block_hash = latest_block.as_ref().map(|block| block.hash().to_string());
        let latest_block_ms = latest_block.as_ref().map_or(0, |block| {
            u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
        });
        let now_ms = current_ledger_time_ms_from_latest_block(latest_block_ms);
        let selector =
            selector_for_namespace_literal(namespace, &literal, &view.nexus.dataspace_catalog)
                .map_err(SnsError::from)?;
        let cache_key = SnsNameRecordCacheKey::from_selector(&selector);
        if let Some(cached) = cache.get(&cache_key, block_height, block_hash.as_deref(), now_ms) {
            iroha_logger::debug!(
                namespace = %namespace_path,
                literal = %literal,
                block_height,
                block_hash = %block_hash.as_deref().unwrap_or("-"),
                elapsed_ms = started_at.elapsed().as_millis(),
                "SNS name lookup cache hit",
            );
            return cached;
        }
        let result =
            get_name_record_by_selector(view.world(), &selector, now_ms).map_err(SnsError::from);
        match &result {
            Ok(record) => cache.insert_found(
                cache_key,
                block_height,
                block_hash.clone(),
                sns_name_record_cache_valid_until_ms(record),
                record.clone(),
            ),
            Err(SnsError::NotFound(message)) => {
                cache.insert_not_found(
                    cache_key,
                    block_height,
                    block_hash.clone(),
                    message.clone(),
                );
            }
            Err(
                SnsError::BadRequest(_)
                | SnsError::Conflict(_)
                | SnsError::Internal(_)
                | SnsError::Access(_),
            ) => {}
        }
        iroha_logger::debug!(
            namespace = %namespace_path,
            literal = %literal,
            block_height,
            block_hash = %block_hash.as_deref().unwrap_or("-"),
            elapsed_ms = started_at.elapsed().as_millis(),
            cache = "miss",
            "SNS name lookup completed",
        );
        result
    })
    .await?;
    Ok(JsonBody(record))
}
/// Handle `GET /v1/sns/policies/{suffix_id}`.
pub async fn handle_get_policy(
    Path(suffix_id): Path<SuffixId>,
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
) -> Result<impl IntoResponse, SnsError> {
    crate::check_access(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/sns/policies/{suffix_id}",
    )
    .await
    .map_err(SnsError::Access)?;
    let app_for_job = Arc::clone(&app);
    let policy = run_blocking_sns("get_policy", move || {
        let view = app_for_job.state.view();
        policy_by_id(view.world(), suffix_id)
            .map_err(SnsError::from)?
            .ok_or_else(|| {
                SnsError::NotFound(format!("suffix policy {suffix_id} is not registered"))
            })
    })
    .await?;
    Ok(JsonBody(policy))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn cache_selector(label: &str) -> NameSelectorV1 {
        NameSelectorV1 {
            version: NameSelectorV1::VERSION,
            suffix_id: iroha_core::sns::ACCOUNT_ALIAS_SUFFIX_ID,
            label: label.to_owned(),
        }
    }
    fn cache_key(label: &str) -> SnsNameRecordCacheKey {
        let selector = cache_selector(label);
        SnsNameRecordCacheKey::from_selector(&selector)
    }
    fn cache_record(label: &str) -> NameRecordV1 {
        let selector = cache_selector(label);
        NameRecordV1::new(
            selector,
            iroha_test_samples::ALICE_ID.clone(),
            Vec::new(),
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            iroha_data_model::metadata::Metadata::default(),
        )
    }
    #[test]
    fn core_error_maps_to_http_status_family() {
        let bad_request: SnsError = CoreSnsError::BadRequest("bad".to_owned()).into();
        let response = bad_request.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let conflict: SnsError = CoreSnsError::Conflict("conflict".to_owned()).into();
        let response = conflict.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let not_found: SnsError = CoreSnsError::NotFound("missing".to_owned()).into();
        let response = not_found.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let internal: SnsError = CoreSnsError::Internal("boom".to_owned()).into();
        let response = internal.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let access = SnsError::Access(crate::Error::AppUnauthorized {
            code: "sns_auth_required",
            message: "authentication required".to_owned(),
        });
        let response = access.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    #[test]
    fn sns_name_cache_returns_found_within_same_block() {
        let cache = SnsNameRecordCache::new();
        let key = cache_key("clear-orbit-3941@hbl.sbp");
        let record = cache_record("clear-orbit-3941@hbl.sbp");
        cache.insert_found(
            key.clone(),
            12,
            Some("block-a".to_owned()),
            Some(u64::MAX),
            record.clone(),
        );
        let cached = cache
            .get(&key, 12, Some("block-a"), 0)
            .expect("cache entry")
            .expect("record");
        assert_eq!(cached, record);
        assert_eq!(cache.len(), 1);
    }
    #[test]
    fn sns_name_cache_returns_not_found_within_same_block() {
        let cache = SnsNameRecordCache::new();
        let key = cache_key("missing-alias@hbl.sbp");
        cache.insert_not_found(
            key.clone(),
            12,
            Some("block-a".to_owned()),
            "registration `missing-alias@hbl.sbp` not found".to_owned(),
        );
        match cache
            .get(&key, 12, Some("block-a"), 0)
            .expect("cache entry")
        {
            Err(SnsError::NotFound(message)) => {
                assert_eq!(message, "registration `missing-alias@hbl.sbp` not found");
            }
            other => panic!("expected cached not found, got {other:?}"),
        }
    }
    #[test]
    fn sns_name_cache_ignores_entries_from_older_block() {
        let cache = SnsNameRecordCache::new();
        let key = cache_key("late-alias@hbl.sbp");
        let record = cache_record("late-alias@hbl.sbp");
        cache.insert_not_found(
            key.clone(),
            12,
            Some("block-a".to_owned()),
            "registration `late-alias@hbl.sbp` not found".to_owned(),
        );
        assert!(cache.get(&key, 13, Some("block-b"), 0).is_none());
        cache.insert_found(
            key.clone(),
            13,
            Some("block-b".to_owned()),
            Some(u64::MAX),
            record.clone(),
        );
        let cached = cache
            .get(&key, 13, Some("block-b"), 0)
            .expect("cache entry")
            .expect("record");
        assert_eq!(cached, record);
    }
    #[test]
    fn sns_name_cache_ignores_entries_from_different_block_hash_at_same_height() {
        let cache = SnsNameRecordCache::new();
        let key = cache_key("same-height-alias@hbl.sbp");
        let record = cache_record("same-height-alias@hbl.sbp");
        cache.insert_not_found(
            key.clone(),
            12,
            Some("old-tip".to_owned()),
            "registration `same-height-alias@hbl.sbp` not found".to_owned(),
        );
        assert!(cache.get(&key, 12, Some("new-tip"), 0).is_none());
        cache.insert_found(
            key.clone(),
            12,
            Some("new-tip".to_owned()),
            Some(u64::MAX),
            record.clone(),
        );
        let cached = cache
            .get(&key, 12, Some("new-tip"), 0)
            .expect("cache entry")
            .expect("record");
        assert_eq!(cached, record);
    }
    #[test]
    fn sns_name_record_cache_deadline_tracks_lifecycle_status() {
        let mut record = cache_record("deadline-alias@hbl.sbp");
        record.expires_at_ms = 50;
        record.grace_expires_at_ms = 75;
        record.redemption_expires_at_ms = 100;
        record.status = NameStatus::Active;
        assert_eq!(sns_name_record_cache_valid_until_ms(&record), Some(50));
        record.status = NameStatus::GracePeriod;
        assert_eq!(sns_name_record_cache_valid_until_ms(&record), Some(75));
        record.status = NameStatus::Redemption;
        assert_eq!(sns_name_record_cache_valid_until_ms(&record), Some(100));
        record.status = NameStatus::Frozen(iroha_data_model::sns::NameFrozenStateV1 {
            reason: "governance".to_owned(),
            until_ms: 125,
        });
        assert_eq!(sns_name_record_cache_valid_until_ms(&record), Some(125));
        record.status = NameStatus::Tombstoned(iroha_data_model::sns::NameTombstoneStateV1 {
            reason: "expired".to_owned(),
        });
        assert_eq!(sns_name_record_cache_valid_until_ms(&record), None);
    }
    #[test]
    fn sns_name_cache_ignores_found_entries_at_lifecycle_deadline() {
        let cache = SnsNameRecordCache::new();
        let key = cache_key("expiring-alias@hbl.sbp");
        let mut record = cache_record("expiring-alias@hbl.sbp");
        record.expires_at_ms = 50;
        cache.insert_found(
            key.clone(),
            12,
            Some("block-a".to_owned()),
            sns_name_record_cache_valid_until_ms(&record),
            record.clone(),
        );
        let cached = cache
            .get(&key, 12, Some("block-a"), 49)
            .expect("cache entry before expiry")
            .expect("record");
        assert_eq!(cached, record);
        assert!(cache.get(&key, 12, Some("block-a"), 50).is_none());
    }
    #[test]
    fn sns_name_cache_replaces_one_entry_at_capacity_without_global_flush() {
        let cache = SnsNameRecordCache::new();
        for index in 0..SNS_NAME_CACHE_MAX_ENTRIES {
            cache.insert_not_found(
                cache_key(&format!("cache-entry-{index}")),
                12,
                Some("block-a".to_owned()),
                "not found".to_owned(),
            );
        }
        assert_eq!(cache.len(), SNS_NAME_CACHE_MAX_ENTRIES);
        let newest = cache_key("newest-cache-entry");
        cache.insert_not_found(
            newest.clone(),
            12,
            Some("block-a".to_owned()),
            "not found".to_owned(),
        );
        assert_eq!(cache.len(), SNS_NAME_CACHE_MAX_ENTRIES);
        assert!(
            cache
                .get(&newest, 12, Some("block-a"), 0)
                .expect("new entry retained")
                .is_err()
        );
    }
    #[test]
    fn sns_name_cache_concurrent_inserts_remain_strictly_bounded() {
        const WORKERS: usize = 8;
        const INSERTS_PER_WORKER: usize = SNS_NAME_CACHE_MAX_ENTRIES / 4;
        let cache = Arc::new(SnsNameRecordCache::new());
        std::thread::scope(|scope| {
            for worker in 0..WORKERS {
                let cache = Arc::clone(&cache);
                scope.spawn(move || {
                    for index in 0..INSERTS_PER_WORKER {
                        cache.insert_not_found(
                            cache_key(&format!("concurrent-cache-entry-{worker}-{index}")),
                            12,
                            Some("block-a".to_owned()),
                            "not found".to_owned(),
                        );
                    }
                });
            }
        });
        assert_eq!(cache.len(), SNS_NAME_CACHE_MAX_ENTRIES);
    }
}
