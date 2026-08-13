// Routed-read execution under one shared fanout reservation. Source-bounded
// local producer seams remain intentionally incremental and inventoried.
#[cfg(feature = "app_api")]
fn merge_with_torii_fanout_headers<F>(diagnostics: ToriiFanoutDiagnostics, merge: F) -> Response
where
    F: FnOnce() -> Result<Response, Response>,
{
    match merge() {
        Ok(response) | Err(response) => with_torii_fanout_headers(response, diagnostics),
    }
}
#[cfg(feature = "app_api")]
fn torii_read_fanout_request(
    endpoint: ToriiReadEndpointV1,
    route_scope: ToriiFanoutRouteScopeV1,
    merge: ToriiReadFanoutMergeV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
    response_format: ToriiProxyResponseFormatV1,
) -> ToriiReadFanoutProxyRequestV1 {
    ToriiReadFanoutProxyRequestV1 {
        endpoint,
        route_scope,
        merge,
        path_args,
        query_string,
        body,
        response_format,
    }
}
#[cfg(feature = "app_api")]
async fn resolve_torii_proof_record_for_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    proof_id: String,
) -> Result<
    (
        ProofRecord,
        ToriiFanoutDiagnostics,
        &'static str,
        QueryFanoutMemoryReservation,
    ),
    Response,
> {
    let reservation = try_acquire_query_fanout_memory(app)?;
    match resolve_torii_proof_record_for_supported_routes(app, routes, proof_id).await {
        Ok((record, diagnostics, routed_by)) => Ok((record, diagnostics, routed_by, reservation)),
        Err(response) => Err(hold_query_fanout_memory_in_response_body(
            response,
            reservation,
        )),
    }
}
#[cfg(feature = "app_api")]
async fn resolve_torii_proof_record_for_supported_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    proof_id: String,
) -> Result<(ProofRecord, ToriiFanoutDiagnostics, &'static str), Response> {
    if routes.is_empty() {
        return Err(with_torii_fanout_headers(
            torii_proxy_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "route_unavailable",
                "no Nexus dataspace routes are configured",
            ),
            ToriiFanoutDiagnostics::default(),
        ));
    }
    let routed_by = routed_by_for_routes(app, &routes);
    let mut diagnostics = ToriiFanoutDiagnostics::default();
    let mut last_not_found = None;
    let mut last_route_unavailable = None;
    let mut budget = ToriiRoutedReadMemoryBudget::new(
        app.query_fanout_working_set_bytes,
        app.torii_proxy_max_response_bytes,
    )?;
    let payload_capacity = routes
        .len()
        .checked_add(1)
        .ok_or_else(torii_routed_read_accounting_response)?;
    let mut payloads = budget.try_retained_vec(payload_capacity)?;
    let parsed_proof_id = proof_id
        .parse::<iroha_data_model::proof::ProofId>()
        .map_err(|error| {
            torii_proxy_error_response(
                StatusCode::BAD_REQUEST,
                "invalid_proof_id",
                format!("failed to parse proof id `{proof_id}`: {error}"),
            )
        })?;
    if let Some(payload) =
        torii_bounded_local_proof_record_payload(app, &parsed_proof_id, &mut budget)?
    {
        budget.push_retained(&mut payloads, payload)?;
    }
    for route in &routes {
        diagnostics.record_attempt();
        let response = execute_torii_read_for_route(
            app,
            *route,
            torii_read_request(
                ToriiReadEndpointV1::ProofRecordGet,
                *route,
                vec![proof_id.clone()],
                None,
                Vec::new(),
            ),
            None,
        )
        .await;
        if response.status() == StatusCode::NOT_FOUND {
            diagnostics.record_skipped_response(&response);
            last_not_found = Some(response);
            continue;
        }
        if torii_response_has_reject_code(&response, "route_unavailable") {
            diagnostics.record_skipped_response(&response);
            last_route_unavailable = Some(response);
            continue;
        }
        match torii_norito_body::<ProofRecord>(response, "proof record response", &mut budget).await
        {
            Ok(payload) => {
                diagnostics.record_success();
                budget.push_retained(&mut payloads, payload)?;
            }
            Err(response) => {
                diagnostics.record_skipped_response(&response);
                return Err(with_torii_fanout_headers(response, diagnostics));
            }
        }
    }
    if payloads.is_empty() {
        let response = last_not_found.unwrap_or_else(|| {
            last_route_unavailable.unwrap_or_else(|| {
                torii_proxy_error_response(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "no dataspace returned a matching result",
                )
            })
        });
        return Err(with_torii_fanout_headers(response, diagnostics));
    }
    budget.begin_typed_merge();
    budget.admit_merge_btree::<Vec<u8>, ProofRecord>(1, payloads.len())?;
    let mut unique_payloads = BTreeMap::<Vec<u8>, ProofRecord>::new();
    for payload in payloads {
        unique_payloads
            .entry(payload.canonical_bytes)
            .or_insert(payload.value);
    }
    match unique_payloads.len() {
        1 => Ok((
            unique_payloads
                .into_values()
                .next()
                .expect("singleton map length should be one"),
            diagnostics,
            routed_by,
        )),
        _ => Err(with_torii_fanout_headers(
            torii_proxy_error_response(
                StatusCode::CONFLICT,
                "route_conflict",
                "multiple dataspaces returned conflicting singleton results",
            ),
            diagnostics,
        )),
    }
}
#[cfg(feature = "app_api")]
fn merged_trusted_internal_read_response<T>(
    payloads: Vec<ToriiBoundedNoritoPayload<T>>,
    format: ResponseFormat,
    routed_by: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response>
where
    T: JsonSerialize + norito::core::NoritoSerialize + 'static,
{
    budget.begin_typed_merge();
    budget.admit_merge_btree::<Vec<u8>, T>(1, payloads.len())?;
    let mut unique_payloads = BTreeMap::<Vec<u8>, T>::new();
    for payload in payloads {
        unique_payloads
            .entry(payload.canonical_bytes)
            .or_insert(payload.value);
    }
    match unique_payloads.len() {
        0 => Err(trusted_internal_read_error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "no target-account route returned the exact resource",
            format,
        )),
        1 => {
            let (canonical_bytes, payload) = unique_payloads
                .into_iter()
                .next()
                .expect("singleton map length should be one");
            let mut response = match format {
                ResponseFormat::Norito => Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        axum::http::header::CONTENT_TYPE,
                        HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
                    )
                    .body(Body::from(canonical_bytes))
                    .expect("build preflighted trusted routed-read response"),
                ResponseFormat::Json => {
                    drop(canonical_bytes);
                    budget.json_response(&payload)?
                }
            };
            insert_routed_by_header(&mut response, routed_by);
            Ok(response)
        }
        _ => Err(trusted_internal_read_error_response(
            StatusCode::CONFLICT,
            "route_conflict",
            "target-account routes returned conflicting exact resources",
            format,
        )),
    }
}
#[cfg(feature = "app_api")]
async fn execute_torii_trusted_internal_read_for_resolved_routes<T>(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    format: ResponseFormat,
    response_label: &'static str,
) -> Response
where
    T: JsonSerialize + norito::codec::Decode + norito::core::NoritoSerialize + 'static,
{
    execute_torii_trusted_internal_read_for_supported_routes::<T>(
        app,
        routes,
        endpoint,
        path_args,
        query_string,
        format,
        response_label,
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_trusted_internal_read_for_supported_routes<T>(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    format: ResponseFormat,
    response_label: &'static str,
) -> Response
where
    T: JsonSerialize + norito::codec::Decode + norito::core::NoritoSerialize + 'static,
{
    let reservation = match try_acquire_query_fanout_memory(app) {
        Ok(reservation) => reservation,
        Err(response) => return response,
    };
    let admission = match ToriiRoutedReadMemoryBudget::new(
        app.query_fanout_working_set_bytes,
        app.torii_proxy_max_response_bytes,
    ) {
        Ok(admission) => admission,
        Err(response) => {
            return hold_query_fanout_memory_in_response_body(response, reservation);
        }
    };
    let empty_body: Vec<u8> = Vec::new();
    let request_bytes = match torii_routed_read_request_bytes(
        &path_args,
        path_args.capacity(),
        query_string.as_ref(),
        empty_body.capacity(),
    ) {
        Ok(bytes) => bytes,
        Err(response) => {
            return hold_query_fanout_memory_in_response_body(response, reservation);
        }
    };
    if let Err(response) = admission.admit_request_bytes(request_bytes) {
        return hold_query_fanout_memory_in_response_body(response, reservation);
    }
    let response = execute_torii_trusted_internal_read_for_resolved_routes_admitted::<T>(
        app,
        routes,
        endpoint,
        path_args,
        query_string,
        format,
        response_label,
    )
    .await;
    hold_query_fanout_memory_in_response_body(response, reservation)
}
#[cfg(feature = "app_api")]
async fn execute_torii_trusted_internal_read_for_resolved_routes_admitted<T>(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    format: ResponseFormat,
    response_label: &'static str,
) -> Response
where
    T: JsonSerialize + norito::codec::Decode + norito::core::NoritoSerialize + 'static,
{
    if routes.is_empty() {
        return with_torii_fanout_headers(
            trusted_internal_read_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "route_unavailable",
                "no target-account dataspace routes are configured",
                format,
            ),
            ToriiFanoutDiagnostics::default(),
        );
    }
    let mut diagnostics = ToriiFanoutDiagnostics::default();
    let mut saw_not_found = false;
    let mut saw_route_unavailable = false;
    let mut budget = match ToriiRoutedReadMemoryBudget::new(
        app.query_fanout_working_set_bytes,
        app.torii_proxy_max_response_bytes,
    ) {
        Ok(budget) => budget,
        Err(response) => return response,
    };
    let mut payloads = match budget.try_retained_vec(routes.len()) {
        Ok(payloads) => payloads,
        Err(response) => return response,
    };
    for route in &routes {
        diagnostics.record_attempt();
        let mut request = torii_read_request(
            endpoint,
            *route,
            path_args.clone(),
            query_string.clone(),
            Vec::new(),
        );
        request.response_format = ToriiProxyResponseFormatV1::Norito;
        let response = execute_torii_read_for_route(app, *route, request, None).await;
        if response.status() == StatusCode::NOT_FOUND {
            diagnostics.record_skipped_response(&response);
            saw_not_found = true;
            continue;
        }
        if torii_response_has_reject_code(&response, "route_unavailable") {
            diagnostics.record_skipped_response(&response);
            saw_route_unavailable = true;
            continue;
        }
        match torii_norito_body::<T>(response, response_label, &mut budget).await {
            Ok(payload) => {
                diagnostics.record_success();
                if let Err(response) = budget.push_retained(&mut payloads, payload) {
                    return with_torii_fanout_headers(response, diagnostics);
                }
            }
            Err(response) => {
                diagnostics.record_skipped_response(&response);
                return with_torii_fanout_headers(response, diagnostics);
            }
        }
    }
    if payloads.is_empty() {
        let response = if saw_not_found {
            trusted_internal_read_error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                "no target-account route returned the exact resource",
                format,
            )
        } else if saw_route_unavailable {
            trusted_internal_read_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "route_unavailable",
                "no target-account route was available",
                format,
            )
        } else {
            trusted_internal_read_error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                "no target-account route returned the exact resource",
                format,
            )
        };
        return with_torii_fanout_headers(response, diagnostics);
    }
    merge_with_torii_fanout_headers(diagnostics, || {
        merged_trusted_internal_read_response(
            payloads,
            format,
            routed_by_for_routes(app, &routes),
            budget,
        )
    })
}
#[cfg(feature = "app_api")]
async fn execute_torii_internal_account_read_for_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    canonical_account_id: String,
    format: ResponseFormat,
) -> Response {
    execute_torii_trusted_internal_read_for_resolved_routes::<InternalAccountReadResponse>(
        app,
        routes,
        ToriiReadEndpointV1::InternalAccountGet,
        vec![canonical_account_id],
        None,
        format,
        "trusted internal account response",
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_internal_account_transaction_read_for_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    canonical_account_id: String,
    canonical_entrypoint_hash: String,
    format: ResponseFormat,
) -> Response {
    execute_torii_trusted_internal_read_for_resolved_routes::<
        iroha_data_model::query::CommittedTransaction,
    >(
        app,
        routes,
        ToriiReadEndpointV1::InternalAccountTransactionGet,
        vec![canonical_account_id, canonical_entrypoint_hash],
        None,
        format,
        "trusted internal committed transaction response",
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_internal_account_asset_read_for_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    canonical_account_id: String,
    canonical_asset_definition_id: String,
    canonical_scope: String,
    format: ResponseFormat,
) -> Response {
    let query_string = {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        serializer.append_pair("scope", &canonical_scope);
        serializer.finish()
    };
    execute_torii_trusted_internal_read_for_resolved_routes::<Asset>(
        app,
        routes,
        ToriiReadEndpointV1::InternalAccountAssetGet,
        vec![canonical_account_id, canonical_asset_definition_id],
        Some(query_string),
        format,
        "trusted internal account asset response",
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_account_read_for_resolved_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    canonical_account_id: String,
    format: ResponseFormat,
    proxy_memory: Option<ToriiProxyMemoryReservation>,
) -> Response {
    if routes.is_empty() {
        return with_torii_fanout_headers(
            torii_proxy_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "route_unavailable",
                "no Nexus dataspace routes are configured",
            ),
            ToriiFanoutDiagnostics::default(),
        );
    }
    let mut diagnostics = ToriiFanoutDiagnostics::default();
    let mut last_not_found = None;
    let mut last_route_unavailable = None;
    let mut budget = match ToriiRoutedReadMemoryBudget::new(
        app.query_fanout_working_set_bytes,
        app.torii_proxy_max_response_bytes,
    ) {
        Ok(budget) => budget,
        Err(response) => return response,
    };
    let mut payloads = match budget.try_retained_vec(routes.len()) {
        Ok(payloads) => payloads,
        Err(response) => return response,
    };
    for route in &routes {
        diagnostics.record_attempt();
        let response = execute_torii_read_for_route(
            app,
            *route,
            torii_read_request(
                ToriiReadEndpointV1::AccountGet,
                *route,
                vec![canonical_account_id.clone()],
                None,
                Vec::new(),
            ),
            proxy_memory.clone(),
        )
        .await;
        if response.status() == StatusCode::NOT_FOUND {
            diagnostics.record_skipped_response(&response);
            last_not_found = Some(response);
            continue;
        }
        if torii_response_has_reject_code(&response, "route_unavailable") {
            diagnostics.record_skipped_response(&response);
            last_route_unavailable = Some(response);
            continue;
        }
        match torii_json_body::<AccountReadResponse>(response, "account get response", &mut budget)
            .await
        {
            Ok(payload) => {
                diagnostics.record_success();
                if let Err(response) = budget.push_retained(&mut payloads, payload) {
                    return with_torii_fanout_headers(response, diagnostics);
                }
            }
            Err(response) => {
                diagnostics.record_skipped_response(&response);
                return with_torii_fanout_headers(response, diagnostics);
            }
        }
    }
    if payloads.is_empty() {
        let response = last_not_found.unwrap_or_else(|| {
            last_route_unavailable.unwrap_or_else(|| {
                torii_proxy_error_response(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "no dataspace returned a matching result",
                )
            })
        });
        return with_torii_fanout_headers(response, diagnostics);
    }
    merge_with_torii_fanout_headers(diagnostics, || {
        merged_account_read_response(payloads, format, routed_by_for_routes(app, &routes), budget)
    })
}
#[cfg(feature = "app_api")]
fn account_history_payload_has_more(payload: &Value, item_count: usize, page_limit: u64) -> bool {
    payload
        .as_object()
        .and_then(|object| object.get("has_more"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| u64::try_from(item_count).unwrap_or(u64::MAX) >= page_limit)
}
#[cfg(feature = "app_api")]
async fn execute_torii_account_history_read_for_resolved_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    path_args: Vec<String>,
    query_string: Option<String>,
    proxy_memory: Option<ToriiProxyMemoryReservation>,
) -> Response {
    if routes.is_empty() {
        return with_torii_fanout_headers(
            torii_proxy_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "route_unavailable",
                "no Nexus dataspace routes are configured",
            ),
            ToriiFanoutDiagnostics::default(),
        );
    }
    let Some(account_id) = path_args.get(0).cloned() else {
        return torii_proxy_error_response(
            StatusCode::BAD_REQUEST,
            "invalid_proxy_request",
            "missing proxied path argument `account_id`",
        );
    };
    let request_decode_plan = match torii_routed_read_request_decode_plan(app) {
        Ok(plan) => plan,
        Err(response) => return response,
    };
    let mut params = match decode_torii_proxy_query::<routing::AccountHistoryGetParams>(
        request_decode_plan,
        query_string.as_deref(),
    ) {
        Ok(params) => params,
        Err(response) => return response,
    };
    let limits = routing::app_query_limits();
    let page_limit = match limits.clamp_page_limit(params.limit) {
        Ok(0) => {
            return Error::AppQueryValidation {
                code: "invalid_pagination",
                message: format!(
                    "limit must be between 1 and {} for {}",
                    limits.max_page_limit,
                    routing::ENDPOINT_ACCOUNTS_HISTORY
                ),
            }
            .into_response();
        }
        Ok(limit) => limit,
        Err(error) => return error.into_response(),
    };
    params.limit = Some(page_limit);
    let count_mode_label = account_history_count_mode_label(params.count_mode.as_deref());
    let routed_by = routed_by_for_routes(app, &routes);
    if routes.len() == 1 {
        let query_string = match encode_torii_proxy_query(&params) {
            Ok(query_string) => query_string,
            Err(error) => return error.into_response(),
        };
        let mut diagnostics = ToriiFanoutDiagnostics::default();
        diagnostics.record_attempt();
        let response = execute_torii_read_for_route(
            app,
            routes[0],
            torii_read_request(
                ToriiReadEndpointV1::AccountHistoryGet,
                routes[0],
                vec![account_id],
                query_string,
                Vec::new(),
            ),
            proxy_memory,
        )
        .await;
        if response.status().is_success() {
            diagnostics.record_success();
        } else {
            diagnostics.record_skipped_response(&response);
        }
        return with_torii_fanout_headers(response, diagnostics);
    }
    let (payloads, diagnostics, budget) = match collect_torii_account_history_json_payloads(
        &routes,
        &params,
        page_limit,
        count_mode_label,
        app.query_fanout_working_set_bytes,
        app.torii_proxy_max_response_bytes,
        |route, page_query_string| {
            let account_id = account_id.clone();
            let proxy_memory = proxy_memory.clone();
            async move {
                execute_torii_read_for_route(
                    app,
                    route,
                    torii_read_request(
                        ToriiReadEndpointV1::AccountHistoryGet,
                        route,
                        vec![account_id],
                        page_query_string,
                        Vec::new(),
                    ),
                    proxy_memory,
                )
                .await
            }
        },
    )
    .await
    {
        Ok(collected) => collected,
        Err(response) => return response,
    };
    merge_with_torii_fanout_headers(diagnostics, || {
        merged_account_history_response(
            payloads,
            params.offset,
            page_limit,
            count_mode_label,
            routed_by,
            budget,
        )
    })
}
#[cfg(feature = "app_api")]
async fn collect_torii_account_history_json_payloads<F, Fut>(
    routes: &[RoutingDecision],
    params: &routing::AccountHistoryGetParams,
    page_limit: u64,
    count_mode_label: &'static str,
    working_set_bytes: usize,
    max_body_bytes: usize,
    mut fetch: F,
) -> Result<
    (
        Vec<Value>,
        ToriiFanoutDiagnostics,
        ToriiRoutedReadMemoryBudget,
    ),
    Response,
>
where
    F: FnMut(RoutingDecision, Option<String>) -> Fut,
    Fut: core::future::Future<Output = Response>,
{
    let per_route_target = (count_mode_label == "bounded")
        .then(|| params.offset.saturating_add(page_limit).saturating_add(1));
    let limits = routing::app_query_limits();
    let chunk_limit = limits.max_page_limit.max(1);
    let mut diagnostics = ToriiFanoutDiagnostics::default();
    let mut last_not_found = None;
    let mut last_route_unavailable = None;
    let mut budget = ToriiRoutedReadMemoryBudget::new(working_set_bytes, max_body_bytes)?;
    let mut payloads = budget.try_retained_vec(routes.len())?;
    for route in routes {
        diagnostics.record_attempt();
        let mut route_offset = 0_u64;
        let mut route_items_seen = 0_u64;
        let mut route_succeeded = false;
        loop {
            let mut page_params = params.clone();
            page_params.offset = route_offset;
            let next_limit = per_route_target
                .map(|target| {
                    target
                        .saturating_sub(route_items_seen)
                        .max(1)
                        .min(chunk_limit)
                })
                .unwrap_or(chunk_limit);
            page_params.limit = Some(next_limit);
            let page_query_string = match encode_torii_proxy_query(&page_params) {
                Ok(query_string) => query_string,
                Err(error) => return Err(error.into_response()),
            };
            let response = fetch(*route, page_query_string).await;
            if response.status() == StatusCode::NOT_FOUND {
                diagnostics.record_skipped_response(&response);
                if route_succeeded {
                    return Err(with_torii_fanout_headers(response, diagnostics));
                } else {
                    last_not_found = Some(response);
                }
                break;
            }
            if torii_response_has_reject_code(&response, "route_unavailable") {
                diagnostics.record_skipped_response(&response);
                if route_succeeded {
                    return Err(with_torii_fanout_headers(response, diagnostics));
                } else {
                    last_route_unavailable = Some(response);
                }
                break;
            }
            let payload = match torii_json_body_value(response, &mut budget).await {
                Ok(payload) => payload,
                Err(response) => {
                    diagnostics.record_skipped_response(&response);
                    return Err(with_torii_fanout_headers(response, diagnostics));
                }
            };
            let item_count = match list_items_from_payload(
                &payload,
                "expected account history JSON object with `items`",
            ) {
                Ok(items) => items.len(),
                Err(response) => {
                    diagnostics.record_skipped_response(&response);
                    return Err(with_torii_fanout_headers(response, diagnostics));
                }
            };
            let has_more = account_history_payload_has_more(&payload, item_count, next_limit);
            budget.push_retained(&mut payloads, payload)?;
            route_succeeded = true;
            let item_count_u64 = u64::try_from(item_count).unwrap_or(u64::MAX);
            route_items_seen = route_items_seen.saturating_add(item_count_u64);
            if item_count == 0
                || !has_more
                || per_route_target.is_some_and(|target| route_items_seen >= target)
            {
                break;
            }
            route_offset = route_offset.saturating_add(item_count_u64);
        }
        if route_succeeded {
            diagnostics.record_success();
        }
    }
    if payloads.is_empty() {
        let response = last_not_found.unwrap_or_else(|| {
            last_route_unavailable.unwrap_or_else(|| {
                torii_proxy_error_response(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "no dataspace returned a matching result",
                )
            })
        });
        return Err(with_torii_fanout_headers(response, diagnostics));
    }
    Ok((payloads, diagnostics, budget))
}
#[cfg(feature = "app_api")]
async fn execute_torii_read_fanout_for_resolved_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    merge: ToriiReadFanoutMergeV1,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
    response_format: ToriiProxyResponseFormatV1,
    proxy_memory: Option<ToriiProxyMemoryReservation>,
) -> Response {
    execute_torii_read_for_supported_resolved_routes(
        app,
        routes,
        merge,
        endpoint,
        path_args,
        query_string,
        body,
        response_format,
        proxy_memory,
    )
    .await
}
/// Execute an authenticated incoming Read request inside one complete fanout
/// working-set reservation.
///
/// The request representation is admitted before route execution. The local
/// response is then collected and canonicalized under the same envelope, and
/// the permit moves into the returned body so transport buffering cannot
/// release it early.
#[cfg(feature = "app_api")]
async fn execute_incoming_torii_read_request_locally_bounded(
    app: &SharedAppState,
    read_request: ToriiReadProxyRequestV1,
    routing_decision: RoutingDecision,
) -> Response {
    let reservation = match try_acquire_query_fanout_memory(app) {
        Ok(reservation) => reservation,
        Err(response) => return response,
    };
    let mut budget = match ToriiRoutedReadMemoryBudget::new(
        app.query_fanout_working_set_bytes,
        app.torii_proxy_max_response_bytes,
    ) {
        Ok(budget) => budget,
        Err(response) => {
            return hold_query_fanout_memory_in_response_body(response, reservation);
        }
    };
    let request_bytes = match torii_routed_read_request_bytes(
        &read_request.path_args,
        read_request.path_args.capacity(),
        read_request.query_string.as_ref(),
        read_request.body.capacity(),
    ) {
        Ok(bytes) => bytes,
        Err(response) => {
            return hold_query_fanout_memory_in_response_body(response, reservation);
        }
    };
    if let Err(response) = budget.admit_request_bytes(request_bytes) {
        return hold_query_fanout_memory_in_response_body(response, reservation);
    }
    let response_format = read_request.response_format;
    let response =
        execute_torii_read_request_locally(app, read_request, routing_decision, "proxy").await;
    let response =
        match bound_torii_single_route_response(response, response_format, &mut budget).await {
            Ok(response) | Err(response) => response,
        };
    hold_query_fanout_memory_in_response_body(response, reservation)
}
#[cfg(feature = "app_api")]
async fn execute_torii_read_for_supported_resolved_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    merge: ToriiReadFanoutMergeV1,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
    response_format: ToriiProxyResponseFormatV1,
    proxy_memory: Option<ToriiProxyMemoryReservation>,
) -> Response {
    let reservation = match try_acquire_query_fanout_memory(app) {
        Ok(reservation) => reservation,
        Err(response) => return response,
    };
    let admission = match ToriiRoutedReadMemoryBudget::new(
        app.query_fanout_working_set_bytes,
        app.torii_proxy_max_response_bytes,
    ) {
        Ok(admission) => admission,
        Err(response) => {
            return hold_query_fanout_memory_in_response_body(response, reservation);
        }
    };
    let request_bytes = match torii_routed_read_request_bytes(
        &path_args,
        path_args.capacity(),
        query_string.as_ref(),
        body.capacity(),
    ) {
        Ok(bytes) => bytes,
        Err(response) => {
            return hold_query_fanout_memory_in_response_body(response, reservation);
        }
    };
    if let Err(response) = admission.admit_request_bytes(request_bytes) {
        return hold_query_fanout_memory_in_response_body(response, reservation);
    }
    let response = execute_torii_read_fanout_for_resolved_routes_admitted(
        app,
        routes,
        merge,
        endpoint,
        path_args,
        query_string,
        body,
        response_format,
        proxy_memory,
    )
    .await;
    hold_query_fanout_memory_in_response_body(response, reservation)
}
#[cfg(feature = "app_api")]
async fn execute_torii_read_fanout_for_resolved_routes_admitted(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    merge: ToriiReadFanoutMergeV1,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
    response_format: ToriiProxyResponseFormatV1,
    proxy_memory: Option<ToriiProxyMemoryReservation>,
) -> Response {
    match merge {
        ToriiReadFanoutMergeV1::List => {
            if endpoint == ToriiReadEndpointV1::AccountsList {
                return execute_torii_accounts_list_fanout_for_resolved_routes(
                    app,
                    routes,
                    query_string,
                    proxy_memory,
                )
                .await;
            }
            match execute_torii_fanout_json_payloads_resolved_routes(
                app,
                routes,
                endpoint,
                path_args,
                query_string,
                body,
                proxy_memory,
            )
            .await
            {
                Ok((payloads, diagnostics, routed_by, budget)) => {
                    merge_with_torii_fanout_headers(diagnostics, || {
                        merged_list_response(payloads, routed_by, budget)
                    })
                }
                Err(response) => response,
            }
        }
        ToriiReadFanoutMergeV1::Singleton => {
            if routes.is_empty() {
                return with_torii_fanout_headers(
                    torii_proxy_error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "route_unavailable",
                        "no Nexus dataspace routes are configured",
                    ),
                    ToriiFanoutDiagnostics::default(),
                );
            }
            let collected = match collect_torii_singleton_json_payloads(
                &routes,
                app.query_fanout_working_set_bytes,
                app.torii_proxy_max_response_bytes,
                |route| {
                    execute_torii_read_for_route(
                        app,
                        route,
                        torii_read_request(
                            endpoint,
                            route,
                            path_args.clone(),
                            query_string.clone(),
                            body.clone(),
                        ),
                        proxy_memory.clone(),
                    )
                },
            )
            .await
            {
                Ok(payloads) => payloads,
                Err(response) => return response,
            };
            merge_with_torii_fanout_headers(collected.diagnostics, || {
                if matches!(endpoint, ToriiReadEndpointV1::PipelineTransactionStatusGet) {
                    merged_pipeline_status_response(
                        collected.payloads,
                        routed_by_for_routes(app, &routes),
                        collected.budget,
                    )
                } else {
                    merged_singleton_response(
                        collected.payloads,
                        routed_by_for_routes(app, &routes),
                        collected.budget,
                    )
                }
            })
        }
        ToriiReadFanoutMergeV1::Account => {
            let Some(canonical_account_id) = path_args.get(0).cloned() else {
                return torii_proxy_error_response(
                    StatusCode::BAD_REQUEST,
                    "invalid_proxy_request",
                    "missing proxied path argument `account_id`",
                );
            };
            execute_torii_account_read_for_resolved_routes(
                app,
                routes,
                canonical_account_id,
                response_format_from_torii_proxy(response_format),
                proxy_memory,
            )
            .await
        }
        ToriiReadFanoutMergeV1::AccountHistory => {
            execute_torii_account_history_read_for_resolved_routes(
                app,
                routes,
                path_args,
                query_string,
                proxy_memory,
            )
            .await
        }
        ToriiReadFanoutMergeV1::Portfolio => {
            match execute_torii_fanout_json_payloads_resolved_routes(
                app,
                routes,
                endpoint,
                path_args,
                query_string,
                body,
                proxy_memory,
            )
            .await
            {
                Ok((payloads, diagnostics, routed_by, budget)) => {
                    merge_with_torii_fanout_headers(diagnostics, || {
                        merged_portfolio_response(payloads, routed_by, budget)
                    })
                }
                Err(response) => response,
            }
        }
        ToriiReadFanoutMergeV1::DataspaceSummary => {
            match execute_torii_fanout_json_payloads_resolved_routes(
                app,
                routes,
                endpoint,
                path_args,
                query_string,
                body,
                proxy_memory,
            )
            .await
            {
                Ok((payloads, diagnostics, routed_by, budget)) => {
                    merge_with_torii_fanout_headers(diagnostics, || {
                        merged_dataspace_summary_response(payloads, routed_by, budget)
                    })
                }
                Err(response) => response,
            }
        }
        ToriiReadFanoutMergeV1::SpaceDirectoryBindings => {
            match execute_torii_fanout_json_payloads_resolved_routes(
                app,
                routes,
                endpoint,
                path_args,
                query_string,
                body,
                proxy_memory,
            )
            .await
            {
                Ok((payloads, diagnostics, routed_by, budget)) => {
                    merge_with_torii_fanout_headers(diagnostics, || {
                        merged_space_directory_bindings_response(payloads, routed_by, budget)
                    })
                }
                Err(response) => response,
            }
        }
        ToriiReadFanoutMergeV1::SpaceDirectoryManifests {
            page_offset,
            page_limit,
        } => {
            match execute_torii_fanout_json_payloads_resolved_routes(
                app,
                routes,
                endpoint,
                path_args,
                query_string,
                body,
                proxy_memory,
            )
            .await
            {
                Ok((payloads, diagnostics, routed_by, budget)) => {
                    merge_with_torii_fanout_headers(diagnostics, || {
                        merged_space_directory_manifests_response(
                            payloads,
                            page_offset,
                            page_limit,
                            routed_by,
                            budget,
                        )
                    })
                }
                Err(response) => response,
            }
        }
    }
}
#[cfg(feature = "app_api")]
async fn execute_torii_read_fanout_proxy_request(
    app: &SharedAppState,
    request: ToriiReadFanoutProxyRequestV1,
    proxy_memory: Option<ToriiProxyMemoryReservation>,
) -> Response {
    let routes = match torii_fanout_scope_routes(app.as_ref(), &request.route_scope) {
        Ok(routes) => routes,
        Err(response) => return response,
    };
    execute_torii_read_fanout_for_resolved_routes(
        app,
        routes,
        request.merge,
        request.endpoint,
        request.path_args,
        request.query_string,
        request.body,
        request.response_format,
        proxy_memory,
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_read_fanout_via_nexus(
    app: &SharedAppState,
    route_scope: ToriiFanoutRouteScopeV1,
    merge: ToriiReadFanoutMergeV1,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
    response_format: ToriiProxyResponseFormatV1,
) -> Response {
    let routes = match torii_fanout_scope_routes(app.as_ref(), &route_scope) {
        Ok(routes) => routes,
        Err(response) => return response,
    };
    execute_torii_read_via_nexus_for_supported_routes(
        app,
        routes,
        route_scope,
        merge,
        endpoint,
        path_args,
        query_string,
        body,
        response_format,
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_read_via_nexus_for_supported_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    route_scope: ToriiFanoutRouteScopeV1,
    merge: ToriiReadFanoutMergeV1,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
    response_format: ToriiProxyResponseFormatV1,
) -> Response {
    if routes.len() <= 1 {
        return execute_torii_read_fanout_for_resolved_routes(
            app,
            routes,
            merge,
            endpoint,
            path_args,
            query_string,
            body,
            response_format,
            None,
        )
        .await;
    }
    let request = torii_read_fanout_request(
        endpoint,
        route_scope,
        merge,
        path_args,
        query_string,
        body,
        response_format,
    );
    let nexus_route = match torii_nexus_route(app.as_ref()) {
        Ok(route) => route,
        Err(response) => return response,
    };
    if should_execute_route_locally(app.as_ref(), nexus_route) {
        return execute_torii_read_fanout_proxy_request(app, request, None).await;
    }
    execute_torii_proxy_request_with_fallback(
        app,
        nexus_route,
        ToriiProxyRequestKindV4::ReadFanout(request),
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_fanout_list_read(
    app: &SharedAppState,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
) -> Response {
    execute_torii_read_fanout_via_nexus(
        app,
        ToriiFanoutRouteScopeV1::AllDataspaces,
        ToriiReadFanoutMergeV1::List,
        endpoint,
        path_args,
        query_string,
        body,
        ToriiProxyResponseFormatV1::Json,
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_list_read_for_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    route_scope: ToriiFanoutRouteScopeV1,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
) -> Response {
    if routes.len() > 1 {
        execute_torii_read_fanout_via_nexus(
            app,
            route_scope,
            ToriiReadFanoutMergeV1::List,
            endpoint,
            path_args,
            query_string,
            body,
            ToriiProxyResponseFormatV1::Json,
        )
        .await
    } else {
        execute_torii_read_fanout_for_resolved_routes(
            app,
            routes,
            ToriiReadFanoutMergeV1::List,
            endpoint,
            path_args,
            query_string,
            body,
            ToriiProxyResponseFormatV1::Json,
            None,
        )
        .await
    }
}
#[cfg(feature = "app_api")]
async fn execute_torii_account_history_read_for_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    route_scope: ToriiFanoutRouteScopeV1,
    path_args: Vec<String>,
    query_string: Option<String>,
) -> Response {
    if routes.len() > 1 {
        execute_torii_read_fanout_via_nexus(
            app,
            route_scope,
            ToriiReadFanoutMergeV1::AccountHistory,
            ToriiReadEndpointV1::AccountHistoryGet,
            path_args,
            query_string,
            Vec::new(),
            ToriiProxyResponseFormatV1::Json,
        )
        .await
    } else {
        execute_torii_read_fanout_for_resolved_routes(
            app,
            routes,
            ToriiReadFanoutMergeV1::AccountHistory,
            ToriiReadEndpointV1::AccountHistoryGet,
            path_args,
            query_string,
            Vec::new(),
            ToriiProxyResponseFormatV1::Json,
            None,
        )
        .await
    }
}
#[cfg(feature = "app_api")]
async fn execute_torii_fanout_singleton_read(
    app: &SharedAppState,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
) -> Response {
    execute_torii_read_fanout_via_nexus(
        app,
        ToriiFanoutRouteScopeV1::AllDataspaces,
        ToriiReadFanoutMergeV1::Singleton,
        endpoint,
        path_args,
        query_string,
        body,
        ToriiProxyResponseFormatV1::Json,
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_asset_definition_singleton_read(
    app: &SharedAppState,
    endpoint: ToriiReadEndpointV1,
    definition_id: &AssetDefinitionId,
) -> Response {
    let definition_literal = definition_id.to_string();
    if let Some(route) = torii_asset_definition_read_route(app.as_ref(), definition_id) {
        return execute_torii_singleton_read_for_routes(
            app,
            vec![route],
            ToriiFanoutRouteScopeV1::AllDataspaces,
            endpoint,
            vec![definition_literal],
            None,
            Vec::new(),
        )
        .await;
    }
    execute_torii_fanout_singleton_read(app, endpoint, vec![definition_literal], None, Vec::new())
        .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_singleton_read_for_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    route_scope: ToriiFanoutRouteScopeV1,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
) -> Response {
    if routes.len() > 1 {
        execute_torii_read_fanout_via_nexus(
            app,
            route_scope,
            ToriiReadFanoutMergeV1::Singleton,
            endpoint,
            path_args,
            query_string,
            body,
            ToriiProxyResponseFormatV1::Json,
        )
        .await
    } else {
        execute_torii_read_fanout_for_resolved_routes(
            app,
            routes,
            ToriiReadFanoutMergeV1::Singleton,
            endpoint,
            path_args,
            query_string,
            body,
            ToriiProxyResponseFormatV1::Json,
            None,
        )
        .await
    }
}
#[cfg(feature = "app_api")]
async fn execute_torii_account_read_for_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    canonical_account_id: String,
    format: ResponseFormat,
) -> Response {
    execute_torii_read_fanout_for_resolved_routes(
        app,
        routes,
        ToriiReadFanoutMergeV1::Account,
        ToriiReadEndpointV1::AccountGet,
        vec![canonical_account_id],
        None,
        Vec::new(),
        torii_proxy_response_format(format),
        None,
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_account_read_for_routes_scoped(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    route_scope: ToriiFanoutRouteScopeV1,
    canonical_account_id: String,
    format: ResponseFormat,
) -> Response {
    if routes.len() > 1 {
        execute_torii_read_fanout_via_nexus(
            app,
            route_scope,
            ToriiReadFanoutMergeV1::Account,
            ToriiReadEndpointV1::AccountGet,
            vec![canonical_account_id],
            None,
            Vec::new(),
            torii_proxy_response_format(format),
        )
        .await
    } else {
        execute_torii_read_fanout_for_resolved_routes(
            app,
            routes,
            ToriiReadFanoutMergeV1::Account,
            ToriiReadEndpointV1::AccountGet,
            vec![canonical_account_id],
            None,
            Vec::new(),
            torii_proxy_response_format(format),
            None,
        )
        .await
    }
}
#[cfg(feature = "app_api")]
async fn execute_torii_fanout_portfolio_read(
    app: &SharedAppState,
    uaid_literal: String,
    query_string: Option<String>,
) -> Response {
    execute_torii_read_fanout_via_nexus(
        app,
        ToriiFanoutRouteScopeV1::AllDataspaces,
        ToriiReadFanoutMergeV1::Portfolio,
        ToriiReadEndpointV1::AccountsPortfolio,
        vec![uaid_literal],
        query_string,
        Vec::new(),
        ToriiProxyResponseFormatV1::Json,
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_fanout_dataspace_summary_read(
    app: &SharedAppState,
    account_literal: String,
    query_string: Option<String>,
) -> Response {
    execute_torii_read_fanout_via_nexus(
        app,
        ToriiFanoutRouteScopeV1::AllDataspaces,
        ToriiReadFanoutMergeV1::DataspaceSummary,
        ToriiReadEndpointV1::NexusDataspacesAccountSummary,
        vec![account_literal],
        query_string,
        Vec::new(),
        ToriiProxyResponseFormatV1::Json,
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_fanout_space_directory_bindings_read(
    app: &SharedAppState,
    uaid_literal: String,
    query_string: Option<String>,
) -> Response {
    execute_torii_read_fanout_via_nexus(
        app,
        ToriiFanoutRouteScopeV1::AllDataspaces,
        ToriiReadFanoutMergeV1::SpaceDirectoryBindings,
        ToriiReadEndpointV1::SpaceDirectoryBindingsGet,
        vec![uaid_literal],
        query_string,
        Vec::new(),
        ToriiProxyResponseFormatV1::Json,
    )
    .await
}
#[cfg(feature = "app_api")]
async fn execute_torii_fanout_space_directory_manifests_read(
    app: &SharedAppState,
    uaid_literal: String,
    query_string: Option<String>,
    offset: u64,
    limit: Option<u64>,
) -> Response {
    execute_torii_read_fanout_via_nexus(
        app,
        ToriiFanoutRouteScopeV1::AllDataspaces,
        ToriiReadFanoutMergeV1::SpaceDirectoryManifests {
            page_offset: offset,
            page_limit: limit,
        },
        ToriiReadEndpointV1::SpaceDirectoryManifestsGet,
        vec![uaid_literal],
        query_string,
        Vec::new(),
        ToriiProxyResponseFormatV1::Json,
    )
    .await
}
