// Outermost HTTP request admission for application-API routed reads.
use http_body_util::BodyExt as _;
#[derive(Clone)]
struct AppRoutedReadHttpAdmission {
    reservation: QueryFanoutMemoryReservation,
    decode_plan: ToriiRoutedReadRequestDecodePlan,
}
tokio::task_local! {
    static APP_ROUTED_READ_HTTP_ADMISSION: AppRoutedReadHttpAdmission;
}
fn current_app_routed_read_fanout_reservation() -> Option<QueryFanoutMemoryReservation> {
    APP_ROUTED_READ_HTTP_ADMISSION
        .try_with(|admission| admission.reservation.clone())
        .ok()
}
fn current_app_routed_read_decode_plan() -> Option<ToriiRoutedReadRequestDecodePlan> {
    APP_ROUTED_READ_HTTP_ADMISSION
        .try_with(|admission| admission.decode_plan)
        .ok()
}
fn app_routed_read_http_admission_is_active() -> bool {
    APP_ROUTED_READ_HTTP_ADMISSION.try_with(|_| ()).is_ok()
}
#[derive(Clone, Debug)]
struct AdmittedAppRoutedReadBody {
    bytes: Bytes,
    destination_bytes: usize,
}
fn admitted_app_routed_read_body(request: &Request<Body>) -> Option<Bytes> {
    if !app_routed_read_http_admission_is_active() {
        return None;
    }
    request
        .extensions()
        .get::<AdmittedAppRoutedReadBody>()
        .map(|body| {
            debug_assert!(body.bytes.len() <= body.destination_bytes);
            body.bytes.clone()
        })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppRoutedReadHttpDecoder {
    None,
    Query(&'static str),
    StringQuery(&'static str),
    JsonOrNorito(&'static str),
    Json(&'static str),
    ExactInternalAssetScope,
}
impl AppRoutedReadHttpDecoder {
    fn body_type_name(self) -> Option<&'static str> {
        match self {
            Self::JsonOrNorito(name) | Self::Json(name) => Some(name),
            Self::None | Self::Query(_) | Self::StringQuery(_) | Self::ExactInternalAssetScope => {
                None
            }
        }
    }
    fn typed_request_name(self) -> Option<&'static str> {
        match self {
            Self::Query(name)
            | Self::StringQuery(name)
            | Self::JsonOrNorito(name)
            | Self::Json(name) => Some(name),
            Self::None | Self::ExactInternalAssetScope => None,
        }
    }
    fn preflight_media(self, headers: &HeaderMap) -> Result<(), Response> {
        match self {
            Self::JsonOrNorito(_) => crate::utils::typed_request_content_format(headers).map(drop),
            Self::Json(_) => crate::utils::canonical_json_request_content_type(headers),
            Self::None | Self::Query(_) | Self::StringQuery(_) | Self::ExactInternalAssetScope => {
                Ok(())
            }
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct AppRoutedReadHttpEndpoint {
    endpoint: ToriiReadEndpointV1,
    route: iroha_torii_shared::route_catalog::RouteDescriptor,
    decoder: AppRoutedReadHttpDecoder,
}
macro_rules! app_routed_read_endpoint {
    ($endpoint:ident, $route:path, $decoder:expr) => {
        AppRoutedReadHttpEndpoint {
            endpoint: ToriiReadEndpointV1::$endpoint,
            route: $route,
            decoder: $decoder,
        }
    };
}
const APP_ROUTED_READ_HTTP_ENDPOINTS_V1: [AppRoutedReadHttpEndpoint; 46] = [
    app_routed_read_endpoint!(AccountGet, route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_GET, AppRoutedReadHttpDecoder::None),
    app_routed_read_endpoint!(ExplorerAccountDetail, route_catalog::application_api::EXPLORER_ACCOUNTS_BY_ACCOUNT_ID_GET, AppRoutedReadHttpDecoder::None),
    app_routed_read_endpoint!(AccountAssetsGet, route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_ASSETS_GET, AppRoutedReadHttpDecoder::Query("AccountAssetsGetParams")),
    app_routed_read_endpoint!(AccountAssetsQuery, route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_ASSETS_QUERY_POST, AppRoutedReadHttpDecoder::JsonOrNorito("QueryEnvelope")),
    app_routed_read_endpoint!(AccountPermissionsGet, route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_PERMISSIONS_GET, AppRoutedReadHttpDecoder::Query("Pagination")),
    app_routed_read_endpoint!(AccountTransactionsGet, route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_GET, AppRoutedReadHttpDecoder::Query("AccountTransactionsGetParams")),
    app_routed_read_endpoint!(AccountTransactionsQuery, route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_QUERY_POST, AppRoutedReadHttpDecoder::JsonOrNorito("QueryEnvelope")),
    app_routed_read_endpoint!(TransactionsQuery, route_catalog::application_api::TRANSACTIONS_QUERY_POST, AppRoutedReadHttpDecoder::JsonOrNorito("QueryEnvelope")),
    app_routed_read_endpoint!(PipelineTransactionStatusGet, route_catalog::pipeline::TRANSACTION_STATUS, AppRoutedReadHttpDecoder::StringQuery("PipelineStatusQuery")),
    app_routed_read_endpoint!(ProofRecordGet, route_catalog::pipeline::PROOF, AppRoutedReadHttpDecoder::None),
    app_routed_read_endpoint!(AccountsList, route_catalog::application_api::ACCOUNTS_GET, AppRoutedReadHttpDecoder::Query("ListFilterParams")),
    app_routed_read_endpoint!(AccountsQuery, route_catalog::application_api::ACCOUNTS_QUERY_POST, AppRoutedReadHttpDecoder::JsonOrNorito("QueryEnvelope")),
    app_routed_read_endpoint!(AccountsPortfolio, route_catalog::application_api::ACCOUNTS_BY_UAID_PORTFOLIO_GET, AppRoutedReadHttpDecoder::Query("AccountsPortfolioQuery")),
    app_routed_read_endpoint!(AssetDefinitionsList, route_catalog::application_api::ASSETS_DEFINITIONS_GET, AppRoutedReadHttpDecoder::Query("ListFilterParams")),
    app_routed_read_endpoint!(AssetDefinitionGet, route_catalog::application_api::ASSETS_DEFINITIONS_BY_ASSET_GET, AppRoutedReadHttpDecoder::None),
    app_routed_read_endpoint!(AssetDefinitionsQuery, route_catalog::application_api::ASSETS_DEFINITIONS_QUERY_POST, AppRoutedReadHttpDecoder::JsonOrNorito("QueryEnvelope")),
    app_routed_read_endpoint!(AssetHoldersGet, route_catalog::telemetry::ASSET_HOLDERS, AppRoutedReadHttpDecoder::Query("AssetHolderGetParams")),
    app_routed_read_endpoint!(AssetHoldersQuery, route_catalog::telemetry::ASSET_HOLDERS_QUERY, AppRoutedReadHttpDecoder::JsonOrNorito("QueryEnvelope")),
    app_routed_read_endpoint!(DomainsList, route_catalog::application_api::DOMAINS_GET, AppRoutedReadHttpDecoder::Query("Pagination")),
    app_routed_read_endpoint!(DomainsQuery, route_catalog::application_api::DOMAINS_QUERY_POST, AppRoutedReadHttpDecoder::JsonOrNorito("QueryEnvelope")),
    app_routed_read_endpoint!(NftsList, route_catalog::application_api::NFTS_GET, AppRoutedReadHttpDecoder::Query("ListFilterParams")),
    app_routed_read_endpoint!(NftsQuery, route_catalog::application_api::NFTS_QUERY_POST, AppRoutedReadHttpDecoder::JsonOrNorito("QueryEnvelope")),
    app_routed_read_endpoint!(NexusPublicLaneValidators, route_catalog::application_api::NEXUS_PUBLIC_LANES_BY_LANE_ID_VALIDATORS_GET, AppRoutedReadHttpDecoder::Query("PublicLaneValidatorsQueryParams")),
    app_routed_read_endpoint!(NexusPublicLaneStake, route_catalog::application_api::NEXUS_PUBLIC_LANES_BY_LANE_ID_STAKE_GET, AppRoutedReadHttpDecoder::Query("PublicLaneStakeQueryParams")),
    app_routed_read_endpoint!(NexusPublicLaneRewards, route_catalog::application_api::NEXUS_PUBLIC_LANES_BY_LANE_ID_REWARDS_PENDING_GET, AppRoutedReadHttpDecoder::Query("PublicLaneRewardsQueryParams")),
    app_routed_read_endpoint!(NexusDataspacesAccountSummary, route_catalog::application_api::NEXUS_DATASPACES_ACCOUNTS_BY_LITERAL_SUMMARY_GET, AppRoutedReadHttpDecoder::Query("NexusDataspacesAccountSummaryQueryParams")),
    app_routed_read_endpoint!(SpaceDirectoryBindingsGet, route_catalog::application_api::SPACE_DIRECTORY_UAIDS_BY_UAID_GET, AppRoutedReadHttpDecoder::Query("SpaceDirectoryBindingsQuery")),
    app_routed_read_endpoint!(SpaceDirectoryManifestsGet, route_catalog::application_api::SPACE_DIRECTORY_UAIDS_BY_UAID_MANIFESTS_GET, AppRoutedReadHttpDecoder::Query("SpaceDirectoryManifestQuery")),
    app_routed_read_endpoint!(RwasList, route_catalog::application_api::RWAS_GET, AppRoutedReadHttpDecoder::Query("ListFilterParams")),
    app_routed_read_endpoint!(RwasQuery, route_catalog::application_api::RWAS_QUERY_POST, AppRoutedReadHttpDecoder::JsonOrNorito("QueryEnvelope")),
    app_routed_read_endpoint!(AliasResolve, route_catalog::aliases::RESOLVE, AppRoutedReadHttpDecoder::Json("AliasResolveRequestDto")),
    app_routed_read_endpoint!(AliasResolveIndex, route_catalog::aliases::RESOLVE_INDEX, AppRoutedReadHttpDecoder::Json("AliasResolveIndexRequestDto")),
    app_routed_read_endpoint!(AliasLookupByAccount, route_catalog::aliases::BY_ACCOUNT, AppRoutedReadHttpDecoder::Json("AliasLookupByAccountRequestDto")),
    app_routed_read_endpoint!(ExplorerAssetDefinitionDetail, route_catalog::application_api::EXPLORER_ASSET_DEFINITIONS_BY_DEFINITION_ID_GET, AppRoutedReadHttpDecoder::None),
    app_routed_read_endpoint!(ExplorerAssetDefinitionEconometrics, route_catalog::application_api::EXPLORER_ASSET_DEFINITIONS_BY_DEFINITION_ID_ECONOMETRICS_GET, AppRoutedReadHttpDecoder::None),
    app_routed_read_endpoint!(ExplorerAssetDefinitionSnapshot, route_catalog::application_api::EXPLORER_ASSET_DEFINITIONS_BY_DEFINITION_ID_SNAPSHOT_GET, AppRoutedReadHttpDecoder::None),
    app_routed_read_endpoint!(ContractAliasResolve, route_catalog::contracts_and_verification_keys::CONTRACTS_ALIASES_RESOLVE_POST, AppRoutedReadHttpDecoder::Json("ContractAliasResolveRequestDto")),
    app_routed_read_endpoint!(ContractStateGet, route_catalog::contracts_and_verification_keys::CONTRACTS_STATE_GET, AppRoutedReadHttpDecoder::Query("ContractStateQuery")),
    app_routed_read_endpoint!(ContractViewPost, route_catalog::contracts_and_verification_keys::CONTRACTS_VIEW_POST, AppRoutedReadHttpDecoder::JsonOrNorito("ContractViewDto")),
    app_routed_read_endpoint!(ContractViewBatchPost, route_catalog::contracts_and_verification_keys::CONTRACTS_VIEW_BATCH_POST, AppRoutedReadHttpDecoder::JsonOrNorito("ContractViewBatchDto")),
    app_routed_read_endpoint!(AccountHistoryGet, route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_HISTORY_GET, AppRoutedReadHttpDecoder::Query("AccountHistoryGetParams")),
    app_routed_read_endpoint!(InternalAccountGet, route_catalog::application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_GET, AppRoutedReadHttpDecoder::None),
    app_routed_read_endpoint!(InternalAccountTransactionGet, route_catalog::application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_BY_ENTRYPOINT_HASH_GET, AppRoutedReadHttpDecoder::None),
    app_routed_read_endpoint!(InternalAccountAssetGet, route_catalog::application_api::INTERNAL_ACCOUNTS_BY_ACCOUNT_ID_ASSETS_BY_ASSET_DEFINITION_ID_GET, AppRoutedReadHttpDecoder::ExactInternalAssetScope),
    app_routed_read_endpoint!(ContractDeploymentState, route_catalog::contracts_and_verification_keys::CONTRACTS_DEPLOYMENT_STATE_POST, AppRoutedReadHttpDecoder::Json("ContractDeploymentStateRequestDto")),
    app_routed_read_endpoint!(AccountOnboardingCurrentState, route_catalog::application_api::ACCOUNTS_ONBOARDING_CURRENT_STATE_POST, AppRoutedReadHttpDecoder::Json("AccountOnboardingCurrentStateRequestV1")),
];
fn app_routed_read_http_endpoint(route_id: &str) -> Option<AppRoutedReadHttpEndpoint> {
    APP_ROUTED_READ_HTTP_ENDPOINTS_V1
        .iter()
        .copied()
        .find(|entry| entry.route.stable_route_id() == route_id)
}
async fn enforce_app_routed_read_http_admission(
    State(app): State<SharedAppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let Some(endpoint) = request
        .extensions()
        .get::<MatchedRouteMetadata>()
        .and_then(|metadata| app_routed_read_http_endpoint(metadata.stable_route_id()))
    else {
        return next.run(request).await;
    };
    if let Err(response) = endpoint.decoder.preflight_media(request.headers()) {
        return response;
    }
    let decode_plan = match torii_routed_read_request_decode_plan(&app) {
        Ok(plan) => plan,
        Err(response) => return response,
    };
    // Hyper has already retained the complete request target. Charge its URI
    // representation once, including an absolute-form scheme/authority, fixed
    // route literals, dynamic percent-encoded path segments, and query bytes.
    // Axum's path percent decoder can only shrink this representation before
    // bounded typed/canonical parsing starts.
    let target_bytes = app_routed_read_raw_target_bytes(request.uri());
    if let Err(response) = decode_plan.admit_raw_input(target_bytes) {
        return map_app_routed_read_request_response(response);
    }
    let accepts_body = endpoint.decoder.body_type_name().is_some();
    let body_limit = if accepts_body {
        decode_plan
            .raw_input_limit_bytes
            .saturating_sub(target_bytes)
    } else {
        0
    };
    let declared_body_bytes = match if accepts_body {
        preflight_app_routed_read_content_length(request.headers(), body_limit)
    } else {
        preflight_bodyless_app_routed_read(request.headers())
    } {
        Ok(declared) => declared,
        Err(response) => return response,
    };
    // Axum has already installed its decoded URL-parameter Arcs before an
    // endpoint middleware runs. That pre-route allocation is covered by the
    // listener's bounded HTTP-head envelope. Reject an over-P target before a
    // fanout permit can overlap it; for an accepted target, the decoded values
    // only shrink and share the outer P phase with the exact body destination.
    let reservation = match try_acquire_new_query_fanout_memory(&app) {
        Ok(reservation) => reservation,
        Err(response) => return response,
    };
    let (parts, body) = request.into_parts();
    let body = match tokio::time::timeout(
        app.app_api_routed_read_body_read_timeout,
        collect_app_routed_read_body(body, body_limit, declared_body_bytes),
    )
    .await
    {
        Ok(Ok(body)) => body,
        Ok(Err(response)) => {
            return hold_query_fanout_memory_in_response_body(response, reservation);
        }
        Err(_) => {
            let response = torii_proxy_error_response(
                StatusCode::REQUEST_TIMEOUT,
                "request_body_timeout",
                "The admitted application routed-read body did not complete before its absolute deadline.",
            );
            return hold_query_fanout_memory_in_response_body(response, reservation);
        }
    };
    let mut request = Request::from_parts(parts, Body::from(body.bytes.clone()));
    if accepts_body {
        request.extensions_mut().insert(body);
    }
    let admission = AppRoutedReadHttpAdmission {
        reservation: reservation.clone(),
        decode_plan,
    };
    let response = APP_ROUTED_READ_HTTP_ADMISSION
        .scope(admission, next.run(request))
        .await;
    let _inventory_identity = (endpoint.endpoint, endpoint.decoder.typed_request_name());
    hold_app_routed_read_reservation_if_needed(response, reservation)
}
fn app_routed_read_raw_target_bytes(uri: &axum::http::Uri) -> usize {
    let path_and_query = uri
        .path_and_query()
        .map_or_else(|| uri.path().len(), |target| target.as_str().len());
    let mut total = Some(path_and_query);
    if let Some(scheme) = uri.scheme_str() {
        total = total
            .and_then(|bytes| bytes.checked_add(scheme.len()))
            .and_then(|bytes| bytes.checked_add(1)); // `:`
    }
    if let Some(authority) = uri.authority() {
        let prefix = if uri.scheme().is_some() { 2 } else { 0 }; // `//`
        total = total
            .and_then(|bytes| bytes.checked_add(prefix))
            .and_then(|bytes| bytes.checked_add(authority.as_str().len()));
    }
    // A parsed in-memory URI cannot realistically overflow this sum. Mapping
    // overflow above every valid admission limit keeps the boundary fail-closed.
    total.unwrap_or(usize::MAX)
}
fn hold_app_routed_read_reservation_if_needed(
    response: Response,
    reservation: QueryFanoutMemoryReservation,
) -> Response {
    if response
        .extensions()
        .get::<QueryFanoutMemoryReservation>()
        .is_some()
    {
        response
    } else {
        hold_query_fanout_memory_in_response_body(response, reservation)
    }
}
fn preflight_app_routed_read_content_length(
    headers: &HeaderMap,
    body_limit: usize,
) -> Result<Option<usize>, Response> {
    if headers.contains_key(axum::http::header::CONTENT_LENGTH)
        && headers.contains_key(axum::http::header::TRANSFER_ENCODING)
    {
        return Err(torii_proxy_error_response(
            StatusCode::BAD_REQUEST,
            "request_body_invalid",
            "Content-Length and Transfer-Encoding cannot both frame one request body.",
        ));
    }
    let mut values = headers.get_all(axum::http::header::CONTENT_LENGTH).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(torii_proxy_error_response(
            StatusCode::BAD_REQUEST,
            "request_body_invalid",
            "The request contains multiple Content-Length values.",
        ));
    }
    let encoded = value.as_bytes();
    if encoded.is_empty()
        || !encoded.iter().all(u8::is_ascii_digit)
        || (encoded.len() > 1 && encoded[0] == b'0')
    {
        return Err(torii_proxy_error_response(
            StatusCode::BAD_REQUEST,
            "request_body_invalid",
            "The request Content-Length is not a canonical unsigned decimal.",
        ));
    }
    let declared = value
        .to_str()
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            app_routed_read_request_capacity_response(
                "declared body bytes",
                body_limit.saturating_add(1),
                body_limit,
            )
        })?;
    if declared > body_limit {
        return Err(app_routed_read_request_capacity_response(
            "declared body bytes",
            declared,
            body_limit,
        ));
    }
    Ok(Some(declared))
}
fn preflight_bodyless_app_routed_read(headers: &HeaderMap) -> Result<Option<usize>, Response> {
    if headers.contains_key(axum::http::header::TRANSFER_ENCODING) {
        return Err(torii_proxy_error_response(
            StatusCode::BAD_REQUEST,
            "request_body_invalid",
            "This application routed-read endpoint does not accept a framed request body.",
        ));
    }
    preflight_app_routed_read_content_length(headers, 0)
}
async fn collect_app_routed_read_body(
    body: Body,
    limit: usize,
    declared_bytes: Option<usize>,
) -> Result<AdmittedAppRoutedReadBody, Response> {
    // A missing Content-Length owns the entire P-sized destination while the
    // stream is collected. A non-empty exact owner moves into the request
    // extension and replacement body; a zero-length owner is dropped only
    // after EOF and replaced by the canonical static empty `Bytes` value.
    let destination_bytes = declared_bytes.unwrap_or(limit);
    let storage = torii_allocate_exact_bytes(destination_bytes).map_err(|error| match error {
        norito::json::BoundedJsonError::AllocationFailed => torii_proxy_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "route_unavailable",
            "The admitted application request destination could not be allocated.",
        ),
        _ => app_routed_read_request_capacity_response(
            "raw body bytes",
            destination_bytes,
            destination_bytes,
        ),
    })?;
    let mut owner = ExactAppRoutedReadBody { storage, len: 0 };
    let mut body = body;
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| app_routed_read_body_error_response())?;
        let data = frame.into_data().map_err(|_| {
            torii_proxy_error_response(
                StatusCode::BAD_REQUEST,
                "request_body_invalid",
                "Application routed-read request trailers are not accepted.",
            )
        })?;
        if data.is_empty() {
            continue;
        }
        let next = owner.len.checked_add(data.len()).ok_or_else(|| {
            app_routed_read_request_capacity_response(
                "raw body bytes",
                limit.saturating_add(1),
                limit,
            )
        })?;
        if next > destination_bytes {
            return Err(app_routed_read_request_capacity_response(
                "raw body bytes",
                next,
                destination_bytes,
            ));
        }
        for (slot, byte) in owner.storage[owner.len..next].iter_mut().zip(data.iter()) {
            slot.write(*byte);
        }
        owner.len = next;
    }
    if let Some(declared_bytes) = declared_bytes
        && owner.len != declared_bytes
    {
        return Err(torii_proxy_error_response(
            StatusCode::BAD_REQUEST,
            "request_body_invalid",
            format!(
                "The request body length differs from Content-Length (actual {}, declared {declared_bytes}).",
                owner.len
            ),
        ));
    }
    let bytes = if owner.len == 0 {
        Bytes::new()
    } else {
        Bytes::from_owner(owner)
    };
    Ok(AdmittedAppRoutedReadBody {
        bytes,
        destination_bytes,
    })
}
fn app_routed_read_body_error_response() -> Response {
    torii_proxy_error_response(
        StatusCode::BAD_REQUEST,
        "request_body_invalid",
        "The application routed-read request body stream failed.",
    )
}
struct ExactAppRoutedReadBody {
    storage: Box<[std::mem::MaybeUninit<u8>]>,
    len: usize,
}
impl AsRef<[u8]> for ExactAppRoutedReadBody {
    #[allow(unsafe_code)]
    fn as_ref(&self) -> &[u8] {
        // SAFETY: the collector initializes exactly `len` leading bytes before
        // constructing the owner-backed `Bytes`, and never mutates it later.
        unsafe { std::slice::from_raw_parts(self.storage.as_ptr().cast::<u8>(), self.len) }
    }
}
#[cfg(test)]
include!("tests/lib_routed_reads/app_routed_read_http_admission.rs");
