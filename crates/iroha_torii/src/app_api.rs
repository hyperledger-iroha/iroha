#![cfg(feature = "app_api")]

use std::net::SocketAddr;

use axum::{
    body::Bytes,
    extract::{FromRequestParts, Path, State, connect_info::ConnectInfo},
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use iroha_core::state::{StateReadOnly, WorldReadOnly};
use iroha_data_model::soracloud::SoraRouteVisibilityV1;
use mv::storage::StorageReadOnly;
use norito::json::{self, Map, Value};

use crate::{
    JsonBody, SharedAppState,
    sorafs::site::{decode_content_cid, encode_content_cid},
};

pub(crate) const APP_API_BINDING_CONFIG_NAME: &str = "torii/app_api_binding";
pub(crate) const APP_API_MANIFEST_SCHEMA_VERSION_V1: u16 = 1;

const ADAPTER_CONTRACT_VIEW_BATCH_V1: &str = "contract.view_batch.v1";
const ADAPTER_SWAPS_FILLS_V1: &str = "contract.rollups.swaps_fills.v1";
const ADAPTER_SWAPS_CANDLES_V1: &str = "contract.rollups.swaps_candles.v1";
const ADAPTER_TRADER_ACTIVITY_V1: &str = "contract.rollups.trader_activity.v1";
const ADAPTER_TRADER_ACCOUNT_V1: &str = "contract.rollups.trader_account.v1";
const ADAPTER_INTENTS_V1: &str = "contract.rollups.intents.v1";
const ADAPTER_VAULT_POSITIONS_V1: &str = "contract.rollups.vault_positions.v1";
const ADAPTER_OPERATORS_STATUS_V1: &str = "contract.rollups.operators_status.v1";
const ADAPTER_MARGIN_HEALTH_V1: &str = "contract.rollups.margin_health.v1";
const ADAPTER_RWA_LOTS_V1: &str = "contract.rollups.rwa_lots.v1";
const ADAPTER_DLMM_HOOKS_V1: &str = "contract.rollups.dlmm_hooks.v1";
const API_MANIFEST_MAX_BYTES: u64 = 1024 * 1024;
const API_MANIFEST_FILE_NAMES: &[&str] = &[
    "app-api.json",
    "torii-app-api.json",
    "soraswap-trader-api.json",
    "manifest.json",
];

#[derive(
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
pub(crate) struct ToriiAppApiRouteV1 {
    pub method: String,
    pub path: String,
    pub adapter: String,
    #[norito(default)]
    pub cache_ttl_ms: Option<u64>,
}

#[derive(
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
pub(crate) struct ToriiAppApiManifestV1 {
    pub schema_version: u16,
    pub app_id: String,
    #[norito(default)]
    pub content_cid: Option<String>,
    #[norito(default)]
    pub manifest_digest_hex: Option<String>,
    pub routes: Vec<ToriiAppApiRouteV1>,
}

#[derive(crate::json_macros::JsonDeserialize, Debug, Clone)]
struct ToriiAppApiBindingV1 {
    pub schema_version: u16,
    pub app_id: String,
    #[norito(default)]
    pub content_cid: Option<String>,
    #[norito(default)]
    pub manifest_digest_hex: Option<String>,
    #[norito(default)]
    pub routes: Vec<ToriiAppApiRouteV1>,
}

#[derive(Debug, Clone)]
struct ResolvedBinding {
    service_id: String,
    last_update_sequence: u64,
    binding: ToriiAppApiBindingV1,
}

#[derive(Debug, Clone)]
struct AppApiRouteSource {
    app_id: String,
    content_cid: Option<String>,
    service_id: Option<String>,
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    let mut body = Map::new();
    body.insert("ok".into(), Value::Bool(false));
    body.insert("error".into(), Value::from(message.into()));
    (status, JsonBody(Value::Object(body))).into_response()
}

fn normalize_route_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = if trimmed.starts_with('/') {
        trimmed.to_owned()
    } else {
        format!("/{trimmed}")
    };
    if normalized.contains("//") || normalized.contains('\0') {
        return None;
    }
    Some(normalized)
}

fn normalize_dispatch_path(path: &str) -> Option<String> {
    normalize_route_path(path.trim_start_matches('/'))
}

fn is_supported_method(method: &str) -> bool {
    method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("POST")
}

fn adapter_is_supported(adapter: &str) -> bool {
    matches!(
        adapter,
        ADAPTER_CONTRACT_VIEW_BATCH_V1
            | ADAPTER_SWAPS_FILLS_V1
            | ADAPTER_SWAPS_CANDLES_V1
            | ADAPTER_TRADER_ACTIVITY_V1
            | ADAPTER_TRADER_ACCOUNT_V1
            | ADAPTER_INTENTS_V1
            | ADAPTER_VAULT_POSITIONS_V1
            | ADAPTER_OPERATORS_STATUS_V1
            | ADAPTER_MARGIN_HEALTH_V1
            | ADAPTER_RWA_LOTS_V1
            | ADAPTER_DLMM_HOOKS_V1
    )
}

fn validate_route(route: &ToriiAppApiRouteV1) -> Result<(), Response> {
    if !is_supported_method(&route.method) {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            format!("unsupported app API route method `{}`", route.method),
        ));
    }
    if normalize_route_path(&route.path).is_none() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            format!("invalid app API route path `{}`", route.path),
        ));
    }
    if !adapter_is_supported(&route.adapter) {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            format!("unsupported app API adapter `{}`", route.adapter),
        ));
    }
    Ok(())
}

fn validate_manifest(
    manifest: &ToriiAppApiManifestV1,
    expected_cid: Option<&str>,
    expected_manifest_digest_hex: Option<&str>,
) -> Result<(), Response> {
    if manifest.schema_version != APP_API_MANIFEST_SCHEMA_VERSION_V1 {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            format!(
                "unsupported app API manifest schema_version `{}`",
                manifest.schema_version
            ),
        ));
    }
    if manifest.app_id.trim().is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "app API manifest app_id must not be empty",
        ));
    }
    if let (Some(expected), Some(actual)) = (expected_cid, manifest.content_cid.as_deref()) {
        if actual != expected {
            return Err(json_error(
                StatusCode::CONFLICT,
                format!("app API manifest content_cid `{actual}` does not match request CID"),
            ));
        }
    }
    if let (Some(expected), Some(actual)) = (
        expected_manifest_digest_hex,
        manifest.manifest_digest_hex.as_deref(),
    ) {
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(json_error(
                StatusCode::CONFLICT,
                format!(
                    "app API manifest manifest_digest_hex `{actual}` does not match stored manifest"
                ),
            ));
        }
    }
    if manifest.routes.is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "app API manifest must define at least one route",
        ));
    }
    for route in &manifest.routes {
        validate_route(route)?;
    }
    Ok(())
}

fn validate_binding(binding: &ToriiAppApiBindingV1) -> Result<(), Response> {
    if binding.schema_version != APP_API_MANIFEST_SCHEMA_VERSION_V1 {
        return Err(json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "unsupported app API binding schema_version `{}`",
                binding.schema_version
            ),
        ));
    }
    if binding.app_id.trim().is_empty() {
        return Err(json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "app API binding app_id must not be empty",
        ));
    }
    if binding.routes.is_empty() && binding.content_cid.is_none() {
        return Err(json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "app API binding must define routes or content_cid",
        ));
    }
    for route in &binding.routes {
        validate_route(route)?;
    }
    Ok(())
}

fn binding_to_manifest(binding: &ToriiAppApiBindingV1) -> ToriiAppApiManifestV1 {
    ToriiAppApiManifestV1 {
        schema_version: binding.schema_version,
        app_id: binding.app_id.clone(),
        content_cid: binding.content_cid.clone(),
        manifest_digest_hex: binding.manifest_digest_hex.clone(),
        routes: binding.routes.clone(),
    }
}

fn resolve_binding_candidates(state: &SharedAppState) -> Result<Vec<ResolvedBinding>, Response> {
    let state_view = state.state.view();
    let world = state_view.world();
    let mut candidates = Vec::new();

    for (service_id, deployment) in world.soracloud_service_deployments().iter() {
        let service_name = service_id.to_string();
        let Some(bundle) = world.soracloud_service_revisions().get(&(
            service_name.clone(),
            deployment.current_service_version.clone(),
        )) else {
            continue;
        };
        let Some(route) = bundle.service.route.as_ref() else {
            continue;
        };
        if route.visibility != SoraRouteVisibilityV1::Public {
            continue;
        }
        let Some(config_entry) = deployment.service_configs.get(APP_API_BINDING_CONFIG_NAME) else {
            continue;
        };
        let binding = config_entry
            .value_json
            .try_into_any_norito::<ToriiAppApiBindingV1>()
            .map_err(|err| {
                json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!(
                        "app API binding for service `{service_name}` could not be decoded: {err}"
                    ),
                )
            })?;
        validate_binding(&binding)?;
        candidates.push(ResolvedBinding {
            service_id: service_name,
            last_update_sequence: config_entry.last_update_sequence,
            binding,
        });
    }

    candidates.sort_by(|left, right| {
        right
            .last_update_sequence
            .cmp(&left.last_update_sequence)
            .then_with(|| left.service_id.cmp(&right.service_id))
    });
    Ok(candidates)
}

fn route_to_json(route: &ToriiAppApiRouteV1) -> Value {
    let mut object = Map::new();
    object.insert("method".into(), Value::from(route.method.clone()));
    object.insert("path".into(), Value::from(route.path.clone()));
    object.insert("adapter".into(), Value::from(route.adapter.clone()));
    object.insert(
        "cache_ttl_ms".into(),
        route.cache_ttl_ms.map_or(Value::Null, Value::from),
    );
    Value::Object(object)
}

fn manifest_to_json(manifest: &ToriiAppApiManifestV1) -> Value {
    let mut object = Map::new();
    object.insert(
        "schema_version".into(),
        Value::from(manifest.schema_version as u64),
    );
    object.insert("app_id".into(), Value::from(manifest.app_id.clone()));
    object.insert(
        "content_cid".into(),
        manifest
            .content_cid
            .clone()
            .map_or(Value::Null, Value::from),
    );
    object.insert(
        "manifest_digest_hex".into(),
        manifest
            .manifest_digest_hex
            .clone()
            .map_or(Value::Null, Value::from),
    );
    object.insert(
        "routes".into(),
        Value::Array(manifest.routes.iter().map(route_to_json).collect()),
    );
    Value::Object(object)
}

fn binding_to_json(candidate: &ResolvedBinding) -> Value {
    let manifest = binding_to_manifest(&candidate.binding);
    let mut object = match manifest_to_json(&manifest) {
        Value::Object(object) => object,
        _ => Map::new(),
    };
    object.insert(
        "service_id".into(),
        Value::from(candidate.service_id.clone()),
    );
    object.insert(
        "last_update_sequence".into(),
        Value::from(candidate.last_update_sequence),
    );
    Value::Object(object)
}

fn read_api_manifest_payload(
    state: &SharedAppState,
    stored: &sorafs_node::store::StoredManifest,
) -> Result<Vec<u8>, Response> {
    let file = API_MANIFEST_FILE_NAMES
        .iter()
        .find_map(|name| {
            let path = vec![(*name).to_owned()];
            stored.file_by_path(&path)
        })
        .or_else(|| {
            if stored.files().len() == 1 {
                stored.files().first()
            } else {
                None
            }
        })
        .ok_or_else(|| {
            json_error(
                StatusCode::NOT_FOUND,
                "SoraFS CID does not contain an app API manifest file",
            )
        })?;

    if file.size > API_MANIFEST_MAX_BYTES {
        return Err(json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "app API manifest exceeds the maximum supported size",
        ));
    }
    let length = usize::try_from(file.size).map_err(|_| {
        json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "app API manifest exceeds host usize limits",
        )
    })?;
    state
        .sorafs_node
        .read_payload_range(stored.manifest_id(), file.offset, length)
        .map_err(|err| {
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read app API manifest payload: {err}"),
            )
        })
}

async fn load_api_manifest_by_cid(
    state: &SharedAppState,
    cid: &str,
) -> Result<ToriiAppApiManifestV1, Response> {
    let stored = crate::sorafs::api::resolve_site_manifest_by_cid_unchecked(state, cid).await?;
    let payload = read_api_manifest_payload(state, &stored)?;
    let mut manifest = json::from_slice::<ToriiAppApiManifestV1>(&payload).map_err(|err| {
        json_error(
            StatusCode::BAD_REQUEST,
            format!("failed to decode app API manifest JSON: {err}"),
        )
    })?;
    let stored_cid = encode_content_cid(stored.manifest_cid());
    let stored_manifest_digest_hex = hex::encode(stored.manifest_digest());
    if manifest.content_cid.is_none() {
        manifest.content_cid = Some(stored_cid.clone());
    }
    if manifest.manifest_digest_hex.is_none() {
        manifest.manifest_digest_hex = Some(stored_manifest_digest_hex.clone());
    }
    validate_manifest(
        &manifest,
        Some(&stored_cid),
        Some(&stored_manifest_digest_hex),
    )?;
    Ok(manifest)
}

fn find_matching_route<'a>(
    routes: &'a [ToriiAppApiRouteV1],
    method: &Method,
    path: &str,
) -> Option<&'a ToriiAppApiRouteV1> {
    routes.iter().find(|route| {
        route.method.eq_ignore_ascii_case(method.as_str())
            && normalize_route_path(&route.path).as_deref() == Some(path)
            && adapter_is_supported(&route.adapter)
    })
}

async fn decode_query<T>(uri: &Uri) -> Result<T, Response>
where
    T: norito::json::JsonDeserializeOwned + Send,
{
    let request = Request::builder()
        .uri(uri.clone())
        .body(())
        .expect("valid request from existing URI");
    let (mut parts, _) = request.into_parts();
    crate::NoritoQuery::<T>::from_request_parts(&mut parts, &())
        .await
        .map(|crate::NoritoQuery(value)| value)
}

fn decode_json_body<T>(body: &Bytes) -> Result<T, Response>
where
    T: norito::json::JsonDeserializeOwned,
{
    json::from_slice::<T>(body.as_ref()).map_err(|err| {
        json_error(
            StatusCode::BAD_REQUEST,
            format!("invalid app API JSON body: {err}"),
        )
    })
}

fn attach_dispatch_headers(
    response: &mut Response,
    source: &AppApiRouteSource,
    route: &ToriiAppApiRouteV1,
) {
    if let Ok(value) = HeaderValue::from_str(&source.app_id) {
        response.headers_mut().insert("x-torii-app-api-id", value);
    }
    if let Some(cid) = source.content_cid.as_ref() {
        if let Ok(value) = HeaderValue::from_str(cid) {
            response.headers_mut().insert("x-torii-app-api-cid", value);
        }
    }
    if let Some(service_id) = source.service_id.as_ref() {
        if let Ok(value) = HeaderValue::from_str(service_id) {
            response
                .headers_mut()
                .insert("x-torii-app-api-service", value);
        }
    }
    if let Ok(value) = HeaderValue::from_str(&route.adapter) {
        response
            .headers_mut()
            .insert("x-torii-app-api-adapter", value);
    }
}

async fn dispatch_app_api_route(
    app: SharedAppState,
    headers: HeaderMap,
    remote: SocketAddr,
    method: Method,
    uri: Uri,
    route: &ToriiAppApiRouteV1,
    source: AppApiRouteSource,
    body: Option<Bytes>,
) -> Response {
    let mut response = match route.adapter.as_str() {
        ADAPTER_CONTRACT_VIEW_BATCH_V1 => {
            if method != Method::POST {
                return StatusCode::METHOD_NOT_ALLOWED.into_response();
            }
            let Some(body) = body else {
                return json_error(StatusCode::BAD_REQUEST, "app API POST body is required");
            };
            let request = match decode_json_body::<crate::routing::ContractViewBatchDto>(&body) {
                Ok(value) => crate::NoritoJson(value),
                Err(response) => return response,
            };
            match super::handler_post_contract_view_batch(
                State(app),
                headers,
                ConnectInfo(remote),
                request,
            )
            .await
            {
                Ok(response) => response,
                Err(err) => err.into_response(),
            }
        }
        ADAPTER_SWAPS_FILLS_V1 => {
            if method != Method::GET {
                return StatusCode::METHOD_NOT_ALLOWED.into_response();
            }
            let params =
                match decode_query::<crate::routing::ContractRollupSwapsFillsParams>(&uri).await {
                    Ok(value) => value,
                    Err(response) => return response,
                };
            match super::handler_contracts_rollups_swaps_fills_get(
                State(app),
                headers,
                ConnectInfo(remote),
                crate::NoritoQuery(params),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => err.into_response(),
            }
        }
        ADAPTER_SWAPS_CANDLES_V1 => {
            if method != Method::GET {
                return StatusCode::METHOD_NOT_ALLOWED.into_response();
            }
            let params = match decode_query::<crate::routing::ContractRollupSwapsCandlesParams>(
                &uri,
            )
            .await
            {
                Ok(value) => value,
                Err(response) => return response,
            };
            match super::handler_contracts_rollups_swaps_candles_get(
                State(app),
                headers,
                ConnectInfo(remote),
                crate::NoritoQuery(params),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => err.into_response(),
            }
        }
        ADAPTER_TRADER_ACTIVITY_V1 => {
            if method != Method::GET {
                return StatusCode::METHOD_NOT_ALLOWED.into_response();
            }
            let params = match decode_query::<crate::routing::ContractEventGetParams>(&uri).await {
                Ok(value) => value,
                Err(response) => return response,
            };
            match super::handler_contracts_rollups_trader_activity_get(
                State(app),
                headers,
                ConnectInfo(remote),
                crate::NoritoQuery(params),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => err.into_response(),
            }
        }
        ADAPTER_TRADER_ACCOUNT_V1 => {
            if method != Method::GET {
                return StatusCode::METHOD_NOT_ALLOWED.into_response();
            }
            let params = match decode_query::<crate::routing::TraderRollupAccountParams>(&uri).await
            {
                Ok(value) => value,
                Err(response) => return response,
            };
            match super::handler_contracts_rollups_trader_account_get(
                State(app),
                headers,
                ConnectInfo(remote),
                crate::NoritoQuery(params),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => err.into_response(),
            }
        }
        ADAPTER_INTENTS_V1 => {
            if method != Method::GET {
                return StatusCode::METHOD_NOT_ALLOWED.into_response();
            }
            let params = match decode_query::<crate::routing::ContractEventGetParams>(&uri).await {
                Ok(value) => value,
                Err(response) => return response,
            };
            match super::handler_contracts_rollups_intents_get(
                State(app),
                headers,
                ConnectInfo(remote),
                crate::NoritoQuery(params),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => err.into_response(),
            }
        }
        ADAPTER_VAULT_POSITIONS_V1 => {
            if method != Method::GET {
                return StatusCode::METHOD_NOT_ALLOWED.into_response();
            }
            let params = match decode_query::<crate::routing::ContractEventGetParams>(&uri).await {
                Ok(value) => value,
                Err(response) => return response,
            };
            match super::handler_contracts_rollups_vault_positions_get(
                State(app),
                headers,
                ConnectInfo(remote),
                crate::NoritoQuery(params),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => err.into_response(),
            }
        }
        ADAPTER_OPERATORS_STATUS_V1 => {
            if method != Method::GET {
                return StatusCode::METHOD_NOT_ALLOWED.into_response();
            }
            let params = match decode_query::<crate::routing::ContractEventGetParams>(&uri).await {
                Ok(value) => value,
                Err(response) => return response,
            };
            match super::handler_contracts_rollups_operators_status_get(
                State(app),
                headers,
                ConnectInfo(remote),
                crate::NoritoQuery(params),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => err.into_response(),
            }
        }
        ADAPTER_MARGIN_HEALTH_V1 => {
            if method != Method::GET {
                return StatusCode::METHOD_NOT_ALLOWED.into_response();
            }
            let params = match decode_query::<crate::routing::ContractEventGetParams>(&uri).await {
                Ok(value) => value,
                Err(response) => return response,
            };
            match super::handler_contracts_rollups_margin_health_get(
                State(app),
                headers,
                ConnectInfo(remote),
                crate::NoritoQuery(params),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => err.into_response(),
            }
        }
        ADAPTER_RWA_LOTS_V1 => {
            if method != Method::GET {
                return StatusCode::METHOD_NOT_ALLOWED.into_response();
            }
            let params = match decode_query::<crate::routing::ContractEventGetParams>(&uri).await {
                Ok(value) => value,
                Err(response) => return response,
            };
            match super::handler_contracts_rollups_rwa_lots_get(
                State(app),
                headers,
                ConnectInfo(remote),
                crate::NoritoQuery(params),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => err.into_response(),
            }
        }
        ADAPTER_DLMM_HOOKS_V1 => {
            if method != Method::GET {
                return StatusCode::METHOD_NOT_ALLOWED.into_response();
            }
            let params = match decode_query::<crate::routing::ContractEventGetParams>(&uri).await {
                Ok(value) => value,
                Err(response) => return response,
            };
            match super::handler_contracts_rollups_dlmm_hooks_get(
                State(app),
                headers,
                ConnectInfo(remote),
                crate::NoritoQuery(params),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => err.into_response(),
            }
        }
        _ => {
            return json_error(
                StatusCode::BAD_REQUEST,
                format!("unsupported app API adapter `{}`", route.adapter),
            );
        }
    };
    attach_dispatch_headers(&mut response, &source, route);
    response
}

async fn dispatch_manifest_path(
    app: SharedAppState,
    headers: HeaderMap,
    remote: SocketAddr,
    method: Method,
    uri: Uri,
    raw_path: String,
    manifest: ToriiAppApiManifestV1,
    source: AppApiRouteSource,
    body: Option<Bytes>,
) -> Response {
    let Some(path) = normalize_dispatch_path(&raw_path) else {
        return json_error(StatusCode::BAD_REQUEST, "invalid app API route path");
    };
    let Some(route) = find_matching_route(&manifest.routes, &method, &path) else {
        return json_error(
            StatusCode::NOT_FOUND,
            format!(
                "app API manifest has no {} route for `{path}`",
                method.as_str()
            ),
        );
    };
    dispatch_app_api_route(app, headers, remote, method, uri, route, source, body).await
}

pub(crate) async fn handle_get_app_api_bindings(State(app): State<SharedAppState>) -> Response {
    let candidates = match resolve_binding_candidates(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut body = Map::new();
    body.insert("ok".into(), Value::Bool(true));
    body.insert(
        "config_name".into(),
        Value::from(APP_API_BINDING_CONFIG_NAME.to_owned()),
    );
    body.insert(
        "active_service_id".into(),
        candidates
            .first()
            .map(|candidate| Value::from(candidate.service_id.clone()))
            .unwrap_or(Value::Null),
    );
    body.insert(
        "items".into(),
        Value::Array(candidates.iter().map(binding_to_json).collect()),
    );
    JsonBody(Value::Object(body)).into_response()
}

pub(crate) async fn handle_get_app_api_cid_manifest(
    State(app): State<SharedAppState>,
    Path(cid): Path<String>,
) -> Response {
    let manifest = match load_api_manifest_by_cid(&app, &cid).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    JsonBody(manifest_to_json(&manifest)).into_response()
}

pub(crate) async fn handle_get_app_api_cid_path(
    State(app): State<SharedAppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Path((cid, raw_path)): Path<(String, String)>,
) -> Response {
    let manifest = match load_api_manifest_by_cid(&app, &cid).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let source = AppApiRouteSource {
        app_id: manifest.app_id.clone(),
        content_cid: Some(cid),
        service_id: None,
    };
    dispatch_manifest_path(
        app, headers, remote, method, uri, raw_path, manifest, source, None,
    )
    .await
}

pub(crate) async fn handle_post_app_api_cid_path(
    State(app): State<SharedAppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Path((cid, raw_path)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let manifest = match load_api_manifest_by_cid(&app, &cid).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let source = AppApiRouteSource {
        app_id: manifest.app_id.clone(),
        content_cid: Some(cid),
        service_id: None,
    };
    dispatch_manifest_path(
        app,
        headers,
        remote,
        method,
        uri,
        raw_path,
        manifest,
        source,
        Some(body),
    )
    .await
}

pub(crate) async fn handle_get_app_api_active_path(
    State(app): State<SharedAppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Path(raw_path): Path<String>,
) -> Response {
    let candidates = match resolve_binding_candidates(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(candidate) = candidates.first() else {
        return json_error(StatusCode::NOT_FOUND, "no active app API binding found");
    };
    let manifest = if candidate.binding.routes.is_empty() {
        let Some(cid) = candidate.binding.content_cid.as_deref() else {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "active app API binding has no routes or content_cid",
            );
        };
        match load_api_manifest_by_cid(&app, cid).await {
            Ok(value) => value,
            Err(response) => return response,
        }
    } else {
        binding_to_manifest(&candidate.binding)
    };
    let source = AppApiRouteSource {
        app_id: manifest.app_id.clone(),
        content_cid: manifest.content_cid.clone(),
        service_id: Some(candidate.service_id.clone()),
    };
    dispatch_manifest_path(
        app, headers, remote, method, uri, raw_path, manifest, source, None,
    )
    .await
}

pub(crate) async fn handle_post_app_api_active_path(
    State(app): State<SharedAppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Path(raw_path): Path<String>,
    body: Bytes,
) -> Response {
    let candidates = match resolve_binding_candidates(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(candidate) = candidates.first() else {
        return json_error(StatusCode::NOT_FOUND, "no active app API binding found");
    };
    let manifest = if candidate.binding.routes.is_empty() {
        let Some(cid) = candidate.binding.content_cid.as_deref() else {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "active app API binding has no routes or content_cid",
            );
        };
        match load_api_manifest_by_cid(&app, cid).await {
            Ok(value) => value,
            Err(response) => return response,
        }
    } else {
        binding_to_manifest(&candidate.binding)
    };
    let source = AppApiRouteSource {
        app_id: manifest.app_id.clone(),
        content_cid: manifest.content_cid.clone(),
        service_id: Some(candidate.service_id.clone()),
    };
    dispatch_manifest_path(
        app,
        headers,
        remote,
        method,
        uri,
        raw_path,
        manifest,
        source,
        Some(body),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_route_path_accepts_relative_and_absolute_paths() {
        assert_eq!(
            normalize_route_path("v1/contracts/rollups/trader/account").as_deref(),
            Some("/v1/contracts/rollups/trader/account")
        );
        assert_eq!(
            normalize_route_path("/v1/contracts/view/batch").as_deref(),
            Some("/v1/contracts/view/batch")
        );
        assert_eq!(normalize_route_path(""), None);
        assert_eq!(normalize_route_path("/v1//broken"), None);
    }

    #[test]
    fn find_matching_route_requires_method_path_and_adapter() {
        let routes = vec![ToriiAppApiRouteV1 {
            method: "GET".to_owned(),
            path: "/v1/contracts/rollups/swaps/fills".to_owned(),
            adapter: ADAPTER_SWAPS_FILLS_V1.to_owned(),
            cache_ttl_ms: None,
        }];
        assert!(
            find_matching_route(&routes, &Method::GET, "/v1/contracts/rollups/swaps/fills")
                .is_some()
        );
        assert!(
            find_matching_route(&routes, &Method::POST, "/v1/contracts/rollups/swaps/fills")
                .is_none()
        );
    }
}
