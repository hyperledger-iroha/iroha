// Collection reads through the routed-read fan-out (`specs/torii/collection_queries.md`).
//
// The ingress handler decodes one `ListQuery` (GET parameters or POST body),
// validates it, canonicalises account literals and forwards it as the JSON
// body of the collection's `*Query` endpoint. The fan-out coordinator sends
// every route the same query without `select`, merges the routes' pages
// (by keyset, or by block coordinates for transaction history) and applies
// `select` last, so cursors compose across dataspaces.
#[cfg(feature = "app_api")]
use iroha_torii_shared::list_query::ListQuery;

/// Largest accepted `GET` collection query string.
#[cfg(feature = "app_api")]
const COLLECTION_QUERY_MAX_RAW_BYTES: usize = 64 * 1024;

/// Collection behind a routed read endpoint.
#[cfg(feature = "app_api")]
fn collection_target_for_read(
    endpoint: ToriiReadEndpointV1,
    path_args: &[String],
) -> Option<routing::collection_sources::CollectionTarget> {
    use routing::collection_sources::CollectionTarget as T;
    let first = || path_args.first().cloned().unwrap_or_default();
    Some(match endpoint {
        ToriiReadEndpointV1::DomainsList | ToriiReadEndpointV1::DomainsQuery => T::Domains,
        ToriiReadEndpointV1::AccountsList | ToriiReadEndpointV1::AccountsQuery => T::Accounts,
        ToriiReadEndpointV1::AssetDefinitionsList | ToriiReadEndpointV1::AssetDefinitionsQuery => {
            T::AssetDefinitions
        }
        ToriiReadEndpointV1::NftsList | ToriiReadEndpointV1::NftsQuery => T::Nfts,
        ToriiReadEndpointV1::RwasList | ToriiReadEndpointV1::RwasQuery => T::Rwas,
        ToriiReadEndpointV1::AccountAssetsGet | ToriiReadEndpointV1::AccountAssetsQuery => {
            T::AccountAssets(first())
        }
        ToriiReadEndpointV1::AssetHoldersGet | ToriiReadEndpointV1::AssetHoldersQuery => {
            T::AssetHolders(first())
        }
        ToriiReadEndpointV1::AccountTransactionsGet
        | ToriiReadEndpointV1::AccountTransactionsQuery => T::AccountTransactions(first()),
        ToriiReadEndpointV1::TransactionsQuery => T::Transactions,
        _ => return None,
    })
}

/// Decode `GET` collection parameters with ordinary RFC 3986 percent-decoding.
#[cfg(feature = "app_api")]
fn list_query_from_query_string(query: Option<&str>) -> Result<ListQuery, Error> {
    let raw = query.unwrap_or_default();
    if raw.len() > COLLECTION_QUERY_MAX_RAW_BYTES {
        return Err(collections::CollectionError::new(
            "invalid_query",
            "query",
            format!(
                "query strings must not exceed {} bytes; use POST …/query for large filters",
                COLLECTION_QUERY_MAX_RAW_BYTES
            ),
        )
        .into());
    }
    let pairs: Vec<(String, String)> = url::form_urlencoded::parse(raw.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    // Values may span lines (multi-line filters); other control characters
    // are rejected.
    if let Some((key, _)) = pairs.iter().find(|(key, value)| {
        key.chars().any(char::is_control)
            || value
                .chars()
                .any(|ch| ch.is_control() && !matches!(ch, '\t' | '\n' | '\r'))
    }) {
        return Err(collections::CollectionError::new(
            "invalid_query",
            "query",
            format!(
                "parameter `{key}` contains control characters other than tabs and line breaks"
            ),
        )
        .into());
    }
    ListQuery::from_query_pairs(pairs).map_err(|err| collections::CollectionError::from(err).into())
}

/// Decode a `POST …/query` body.
#[cfg(feature = "app_api")]
fn list_query_from_json(body: norito::json::Value) -> Result<ListQuery, Error> {
    ListQuery::from_json_value(body).map_err(|err| collections::CollectionError::from(err).into())
}

/// Canonical forwarded body for a validated query.
#[cfg(feature = "app_api")]
fn collection_query_body(query: &ListQuery) -> Result<Vec<u8>, Error> {
    norito::json::to_vec(&query.to_json_value()).map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(format!(
            "failed to encode collection query: {err}"
        )))
    })
}

/// Validate and canonicalise a collection query at the ingress node.
#[cfg(feature = "app_api")]
fn prepare_collection_forward(
    app: &SharedAppState,
    target: &routing::collection_sources::CollectionTarget,
    mut query: ListQuery,
) -> Result<(ListQuery, Vec<u8>), Error> {
    let telemetry = app.telemetry_handle();
    routing::collection_sources::canonicalize_collection_query(
        app.state.as_ref(),
        target,
        &mut query,
        &telemetry,
    )?;
    let limits = routing::collection_sources::collection_limits();
    collections::prepare(target.spec(), target.scope(), &query, &limits)?;
    let body = collection_query_body(&query)?;
    Ok((query, body))
}

/// Forward a validated collection read through the visible-account fan-out.
#[cfg(feature = "app_api")]
async fn forward_visible_collection_read(
    app: &SharedAppState,
    caller: Option<&AccountId>,
    endpoint: ToriiReadEndpointV1,
    target: routing::collection_sources::CollectionTarget,
    path_args: Vec<String>,
    query: ListQuery,
) -> Result<Response, Error> {
    let (_, body) = prepare_collection_forward(app, &target, query)?;
    Ok(execute_torii_visible_fanout_list_read(app, caller, endpoint, path_args, None, body).await)
}

/// Page size used to price a collection read for rate limiting.
#[cfg(feature = "app_api")]
fn collection_page_limit(query: &ListQuery) -> u64 {
    let limits = routing::collection_sources::collection_limits();
    u64::from(
        query
            .limit
            .unwrap_or(limits.default_limit)
            .clamp(1, limits.max_limit),
    )
}

/// An empty page for reads with no authoritative route.
#[cfg(feature = "app_api")]
fn empty_collection_page_response(routed_by: &'static str) -> Response {
    let body = norito::json::to_json(
        &iroha_torii_shared::list_query::Page::<norito::json::Value>::last(Vec::new()),
    )
    .unwrap_or_else(|_| "{\"items\":[],\"next_cursor\":null}".to_owned());
    let mut response = Response::new(axum::body::Body::from(body));
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    insert_routed_by_header(&mut response, routed_by);
    response
}

/// Decode the query carried by a routed collection request.
#[cfg(feature = "app_api")]
fn decode_routed_collection_query(
    plan: ToriiRoutedReadRequestDecodePlan,
    endpoint: ToriiReadEndpointV1,
    query_string: Option<&str>,
    body: &[u8],
) -> Result<ListQuery, Response> {
    let is_get = matches!(
        endpoint,
        ToriiReadEndpointV1::DomainsList
            | ToriiReadEndpointV1::AccountsList
            | ToriiReadEndpointV1::AssetDefinitionsList
            | ToriiReadEndpointV1::NftsList
            | ToriiReadEndpointV1::RwasList
            | ToriiReadEndpointV1::AccountAssetsGet
            | ToriiReadEndpointV1::AssetHoldersGet
            | ToriiReadEndpointV1::AccountTransactionsGet
    );
    if is_get {
        return list_query_from_query_string(query_string).map_err(IntoResponse::into_response);
    }
    let value =
        decode_torii_proxy_json_body::<norito::json::Value>(plan, body, "collection query body")?;
    list_query_from_json(value).map_err(IntoResponse::into_response)
}

/// Execute one route's share of a collection read on this node (full rows).
#[cfg(feature = "app_api")]
async fn execute_routed_collection_read(
    app: &SharedAppState,
    request: &ToriiReadProxyRequestV1,
    plan: ToriiRoutedReadRequestDecodePlan,
    routing_decision: RoutingDecision,
    routed_by: &'static str,
    target: routing::collection_sources::CollectionTarget,
    visibility: &routing::DataspaceReadVisibility,
) -> Response {
    let query = match decode_routed_collection_query(
        plan,
        request.endpoint,
        request.query_string.as_deref(),
        &request.body,
    ) {
        Ok(query) => query,
        Err(response) => return response,
    };
    let telemetry = app.telemetry_handle();
    let result = routing::collection_sources::execute_collection_local(
        Some(app),
        &app.state,
        &target,
        query,
        &telemetry,
        visibility,
        Some(routing_decision),
    )
    .await
    .and_then(routing::collection_sources::row_page_response);
    finish_torii_read_result(result, routing_decision, routed_by)
}

/// Fan a collection read out to `routes` and merge their keyset pages.
#[cfg(feature = "app_api")]
#[allow(clippy::too_many_arguments)]
async fn execute_collection_fanout(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    route_scope: ToriiFanoutRouteScopeV1,
    endpoint: ToriiReadEndpointV1,
    target: routing::collection_sources::CollectionTarget,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
    proxy_memory: Option<ToriiProxyMemoryReservation>,
) -> Response {
    let plan = match torii_routed_read_request_decode_plan(app) {
        Ok(plan) => plan,
        Err(response) => return response,
    };
    let query = match decode_routed_collection_query(plan, endpoint, query_string.as_deref(), &body)
    {
        Ok(query) => query,
        Err(response) => return response,
    };
    let limits = routing::collection_sources::collection_limits();
    let prepared = match collections::prepare(target.spec(), target.scope(), &query, &limits) {
        Ok(prepared) => prepared,
        Err(err) => return Error::from(err).into_response(),
    };
    let route_body = match collection_query_body(&prepared.route_query()) {
        Ok(body) => body,
        Err(err) => return err.into_response(),
    };
    let route_endpoint = match endpoint {
        ToriiReadEndpointV1::DomainsList => ToriiReadEndpointV1::DomainsQuery,
        ToriiReadEndpointV1::AccountsList => ToriiReadEndpointV1::AccountsQuery,
        ToriiReadEndpointV1::AssetDefinitionsList => ToriiReadEndpointV1::AssetDefinitionsQuery,
        ToriiReadEndpointV1::NftsList => ToriiReadEndpointV1::NftsQuery,
        ToriiReadEndpointV1::RwasList => ToriiReadEndpointV1::RwasQuery,
        ToriiReadEndpointV1::AccountAssetsGet => ToriiReadEndpointV1::AccountAssetsQuery,
        ToriiReadEndpointV1::AssetHoldersGet => ToriiReadEndpointV1::AssetHoldersQuery,
        ToriiReadEndpointV1::AccountTransactionsGet => {
            ToriiReadEndpointV1::AccountTransactionsQuery
        }
        other => other,
    };
    let (payloads, diagnostics, routed_by, _budget) =
        match execute_torii_fanout_json_payloads_resolved_routes(
            app,
            routes,
            route_scope,
            route_endpoint,
            path_args,
            None,
            route_body,
            proxy_memory,
        )
        .await
        {
            Ok(collected) => collected,
            Err(response) => return response,
        };
    merge_with_torii_fanout_headers(diagnostics, || {
        let pages = payloads
            .into_iter()
            .map(collections::RowPage::from_json)
            .collect::<Result<Vec<_>, _>>()
            .map_err(torii_internal_json_error)?;
        let merged = prepared
            .merge(pages)
            .map_err(|err| Error::from(err).into_response())?;
        let mut response = routing::collection_sources::row_page_response(prepared.project(merged))
            .map_err(IntoResponse::into_response)?;
        insert_routed_by_header(&mut response, routed_by);
        Ok(response)
    })
}

/// Execute a collection read served by this node alone (no fan-out).
#[cfg(feature = "app_api")]
async fn execute_direct_collection_read(
    app: &SharedAppState,
    target: routing::collection_sources::CollectionTarget,
    query: ListQuery,
) -> Result<Response, Error> {
    let telemetry = app.telemetry_handle();
    routing::collection_sources::execute_collection_response(
        Some(app),
        &app.state,
        &target,
        query,
        &telemetry,
        // Directly served collections are public and carry no dataspace scoping.
        &routing::DataspaceReadVisibility::new(std::collections::BTreeSet::new(), true),
    )
    .await
}
