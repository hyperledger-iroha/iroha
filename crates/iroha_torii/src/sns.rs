//! Ledger-backed Sora Name Service (SNS) HTTP handlers.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use iroha_core::{
    sns::{
        SnsError as CoreSnsError, SnsNamespace, get_name_record, policy_by_id,
        selector_for_namespace_literal,
    },
    state::{StateReadOnly, StateReadOnlyWithTransactions},
};
use iroha_data_model::sns::{
    FreezeNameRequestV1, GovernanceHookV1, NameRecordV1, NameSelectorV1, RegisterNameRequestV1,
    RenewNameRequestV1, SuffixId, TransferNameRequestV1, UpdateControllersRequestV1,
};
use std::sync::Arc;

use crate::{JsonBody, SharedAppState};

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
        };
        (status, message).into_response()
    }
}

fn metric_namespace_from_suffix_id(suffix_id: SuffixId) -> String {
    SnsNamespace::from_suffix_id(suffix_id)
        .map(|namespace| namespace.as_path().to_owned())
        .unwrap_or_else(|_| suffix_id.to_string())
}

fn record_registrar_status<T>(
    app: &SharedAppState,
    namespace: &str,
    outcome: &Result<T, SnsError>,
) {
    let result = if outcome.is_ok() { "ok" } else { "error" };
    app.telemetry.with_metrics(|telemetry| {
        telemetry.inc_sns_registrar_status(result, namespace);
    });
}

fn current_ledger_time_ms_from_latest_block(latest_block_ms: u64) -> u64 {
    let wall_clock_ms = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(latest_block_ms);
    wall_clock_ms.max(latest_block_ms)
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
    outcome: CachedSnsNameRecord,
}

/// Per-Torii cache for SNS name reads. Entries are valid only for the latest
/// block height observed when the lookup was performed.
#[derive(Debug, Default)]
pub(crate) struct SnsNameRecordCache {
    entries: DashMap<SnsNameRecordCacheKey, SnsNameRecordCacheEntry>,
}

impl SnsNameRecordCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn get(
        &self,
        key: &SnsNameRecordCacheKey,
        block_height: u64,
    ) -> Option<Result<NameRecordV1, SnsError>> {
        let entry = self.entries.get(key)?;
        if entry.block_height != block_height {
            return None;
        }
        Some(match &entry.outcome {
            CachedSnsNameRecord::Found(record) => Ok(record.clone()),
            CachedSnsNameRecord::NotFound(message) => Err(SnsError::NotFound(message.clone())),
        })
    }

    fn insert_found(&self, key: SnsNameRecordCacheKey, block_height: u64, record: NameRecordV1) {
        self.insert(key, block_height, CachedSnsNameRecord::Found(record));
    }

    fn insert_not_found(&self, key: SnsNameRecordCacheKey, block_height: u64, message: String) {
        self.insert(key, block_height, CachedSnsNameRecord::NotFound(message));
    }

    fn insert(&self, key: SnsNameRecordCacheKey, block_height: u64, outcome: CachedSnsNameRecord) {
        if self.entries.len() >= SNS_NAME_CACHE_MAX_ENTRIES {
            self.entries.clear();
        }
        self.entries.insert(
            key,
            SnsNameRecordCacheEntry {
                block_height,
                outcome,
            },
        );
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

async fn run_blocking_sns<T>(
    op: &'static str,
    job: impl FnOnce() -> Result<T, SnsError> + Send + 'static,
) -> Result<T, SnsError>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|err| SnsError::Internal(format!("SNS {op} worker failed to join: {err}")))?
}

async fn run_serialized_blocking<T>(
    gate: Arc<tokio::sync::Mutex<()>>,
    op: &'static str,
    job: impl FnOnce() -> Result<T, SnsError> + Send + 'static,
) -> Result<T, SnsError>
where
    T: Send + 'static,
{
    let _guard = gate.lock().await;
    run_blocking_sns(op, job).await
}

fn direct_sns_mutation_disabled(op: &str) -> SnsError {
    SnsError::BadRequest(format!(
        "SNS {op} must be submitted as a consensus transaction; direct Torii WSV mutation is disabled"
    ))
}

/// Handle `POST /v1/sns/names`.
#[axum::debug_handler(state = SharedAppState)]
pub async fn handle_register_name(
    State(app): State<SharedAppState>,
    crate::JsonOnly(request): crate::JsonOnly<RegisterNameRequestV1>,
) -> Result<Response, SnsError> {
    let namespace = metric_namespace_from_suffix_id(request.selector.suffix_id);
    let outcome = Err(direct_sns_mutation_disabled("register_name"));
    record_registrar_status(&app, &namespace, &outcome);
    outcome
}

/// Handle `GET /v1/sns/names/{namespace}/{literal}`.
pub async fn handle_get_name(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
) -> Result<impl IntoResponse, SnsError> {
    let app_for_job = Arc::clone(&app);
    let cache = Arc::clone(&app.sns_name_cache);
    let record = run_blocking_sns("get_name", move || {
        let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
        let view = app_for_job.state.view();
        let latest_block = view.latest_block();
        let block_height = latest_block
            .as_ref()
            .map_or(0, |block| block.header().height().get());
        let latest_block_ms = latest_block.as_ref().map_or(0, |block| {
            u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
        });
        let selector =
            selector_for_namespace_literal(namespace, &literal, &view.nexus.dataspace_catalog)
                .map_err(SnsError::from)?;
        let cache_key = SnsNameRecordCacheKey::from_selector(&selector);
        if let Some(cached) = cache.get(&cache_key, block_height) {
            return cached;
        }
        let result = get_name_record(
            view.world(),
            &view.nexus.dataspace_catalog,
            namespace,
            &literal,
            current_ledger_time_ms_from_latest_block(latest_block_ms),
        )
        .map_err(SnsError::from);
        match &result {
            Ok(record) => cache.insert_found(cache_key, block_height, record.clone()),
            Err(SnsError::NotFound(message)) => {
                cache.insert_not_found(cache_key, block_height, message.clone());
            }
            Err(SnsError::BadRequest(_) | SnsError::Conflict(_) | SnsError::Internal(_)) => {}
        }
        result
    })
    .await?;
    Ok(JsonBody(record))
}

/// Handle `POST /v1/sns/names/{namespace}/{literal}/renew`.
pub async fn handle_renew_name(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(_request): crate::JsonOnly<RenewNameRequestV1>,
) -> Result<Response, SnsError> {
    let _ = literal;
    let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
    let namespace_label = namespace.as_path().to_owned();
    let outcome = Err(direct_sns_mutation_disabled("renew_name"));
    record_registrar_status(&app, &namespace_label, &outcome);
    outcome
}

/// Handle `POST /v1/sns/names/{namespace}/{literal}/transfer`.
pub async fn handle_transfer_name(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(_request): crate::JsonOnly<TransferNameRequestV1>,
) -> Result<Response, SnsError> {
    let _ = literal;
    let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
    let namespace_label = namespace.as_path().to_owned();
    let outcome = Err(direct_sns_mutation_disabled("transfer_name"));
    record_registrar_status(&app, &namespace_label, &outcome);
    outcome
}

/// Handle `POST /v1/sns/names/{namespace}/{literal}/controllers`.
pub async fn handle_update_name_controllers(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(_request): crate::JsonOnly<UpdateControllersRequestV1>,
) -> Result<Response, SnsError> {
    let _ = literal;
    let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
    let namespace_label = namespace.as_path().to_owned();
    let outcome = Err(direct_sns_mutation_disabled("update_name_controllers"));
    record_registrar_status(&app, &namespace_label, &outcome);
    outcome
}

/// Handle `POST /v1/sns/names/{namespace}/{literal}/freeze`.
pub async fn handle_freeze_name(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(_request): crate::JsonOnly<FreezeNameRequestV1>,
) -> Result<Response, SnsError> {
    let _ = literal;
    let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
    let namespace_label = namespace.as_path().to_owned();
    let outcome = Err(direct_sns_mutation_disabled("freeze_name"));
    record_registrar_status(&app, &namespace_label, &outcome);
    outcome
}

/// Handle `DELETE /v1/sns/names/{namespace}/{literal}/freeze`.
pub async fn handle_unfreeze_name(
    Path((namespace, literal)): Path<(String, String)>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(_governance): crate::JsonOnly<GovernanceHookV1>,
) -> Result<Response, SnsError> {
    let _ = literal;
    let namespace = SnsNamespace::from_path(&namespace).map_err(SnsError::from)?;
    let namespace_label = namespace.as_path().to_owned();
    let outcome = Err(direct_sns_mutation_disabled("unfreeze_name"));
    record_registrar_status(&app, &namespace_label, &outcome);
    outcome
}

/// Handle `GET /v1/sns/policies/{suffix_id}`.
pub async fn handle_get_policy(
    Path(suffix_id): Path<SuffixId>,
    State(app): State<SharedAppState>,
) -> Result<impl IntoResponse, SnsError> {
    let app_for_job = Arc::clone(&app);
    let policy = run_blocking_sns("get_policy", move || {
        let view = app_for_job.state.view();
        policy_by_id(view.world(), suffix_id).ok_or_else(|| {
            SnsError::NotFound(format!("suffix policy {suffix_id} is not registered"))
        })
    })
    .await?;
    Ok(JsonBody(policy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

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
    fn metric_namespace_uses_fixed_namespace_paths() {
        assert_eq!(
            metric_namespace_from_suffix_id(iroha_core::sns::ACCOUNT_ALIAS_SUFFIX_ID),
            "account-alias"
        );
        assert_eq!(
            metric_namespace_from_suffix_id(iroha_core::sns::DOMAIN_NAME_SUFFIX_ID),
            "domain"
        );
        assert_eq!(
            metric_namespace_from_suffix_id(iroha_core::sns::DATASPACE_ALIAS_SUFFIX_ID),
            "dataspace"
        );
    }

    #[test]
    fn metric_namespace_uses_numeric_fallback_for_unknown_suffix_id() {
        assert_eq!(metric_namespace_from_suffix_id(0xFFFF), "65535");
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
    }

    #[test]
    fn direct_sns_mutation_disabled_returns_client_error() {
        let response = direct_sns_mutation_disabled("register_name").into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn sns_name_cache_returns_found_within_same_block() {
        let cache = SnsNameRecordCache::new();
        let key = cache_key("clear-orbit-3941@hbl.sbp");
        let record = cache_record("clear-orbit-3941@hbl.sbp");

        cache.insert_found(key.clone(), 12, record.clone());

        let cached = cache.get(&key, 12).expect("cache entry").expect("record");
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
            "registration `missing-alias@hbl.sbp` not found".to_owned(),
        );

        match cache.get(&key, 12).expect("cache entry") {
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
            "registration `late-alias@hbl.sbp` not found".to_owned(),
        );
        assert!(cache.get(&key, 13).is_none());

        cache.insert_found(key.clone(), 13, record.clone());
        let cached = cache.get(&key, 13).expect("cache entry").expect("record");
        assert_eq!(cached, record);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn serialized_mutations_run_one_blocking_job_at_a_time() {
        let gate = Arc::new(tokio::sync::Mutex::new(()));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let first = {
            let first_gate = Arc::clone(&gate);
            let first_active = Arc::clone(&active);
            let first_peak = Arc::clone(&peak);
            tokio::spawn(async move {
                run_serialized_blocking(first_gate, "serialize-test-1", move || {
                    let concurrent = first_active.fetch_add(1, Ordering::SeqCst) + 1;
                    first_peak.fetch_max(concurrent, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(50));
                    first_active.fetch_sub(1, Ordering::SeqCst);
                    Ok::<_, SnsError>(())
                })
                .await
            })
        };

        let second = {
            let second_gate = Arc::clone(&gate);
            let second_active = Arc::clone(&active);
            let second_peak = Arc::clone(&peak);
            tokio::spawn(async move {
                run_serialized_blocking(second_gate, "serialize-test-2", move || {
                    let concurrent = second_active.fetch_add(1, Ordering::SeqCst) + 1;
                    second_peak.fetch_max(concurrent, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(50));
                    second_active.fetch_sub(1, Ordering::SeqCst);
                    Ok::<_, SnsError>(())
                })
                .await
            })
        };

        first.await.unwrap().unwrap();
        second.await.unwrap().unwrap();
        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }
}
