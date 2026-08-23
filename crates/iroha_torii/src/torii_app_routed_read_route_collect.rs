#[cfg(feature = "app_api")]
async fn execute_torii_fanout_json_payloads_resolved_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
    endpoint: ToriiReadEndpointV1,
    path_args: Vec<String>,
    query_string: Option<String>,
    body: Vec<u8>,
    proxy_memory: Option<ToriiProxyMemoryReservation>,
) -> Result<
    (
        Vec<Value>,
        ToriiFanoutDiagnostics,
        &'static str,
        ToriiRoutedReadMemoryBudget,
    ),
    Response,
> {
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
    let collected = collect_torii_routed_list_json_payloads(
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
    .await?;
    let ToriiFanoutRoutedJsonPayloads {
        payloads,
        diagnostics,
        mut budget,
    } = collected;
    let payloads = filter_non_authoritative_global_list_rows(app.as_ref(), endpoint, payloads)?;
    let payloads =
        filter_non_authoritative_global_portfolio_rows(app.as_ref(), endpoint, payloads)?;
    let mut values = budget.try_retained_vec(payloads.len())?;
    for (_, payload) in payloads {
        budget.push_retained(&mut values, payload)?;
    }
    Ok((values, diagnostics, routed_by, budget))
}
#[cfg(feature = "app_api")]
async fn execute_torii_accounts_list_fanout_for_resolved_routes(
    app: &SharedAppState,
    routes: Vec<RoutingDecision>,
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
    let request_decode_plan = match torii_routed_read_request_decode_plan(app) {
        Ok(plan) => plan,
        Err(response) => return response,
    };
    let mut params = match decode_torii_proxy_query::<routing::ListFilterParams>(
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
                    "limit must be between 1 and {} for /v1/accounts",
                    limits.max_page_limit
                ),
            }
            .into_response();
        }
        Ok(limit) => limit,
        Err(error) => return error.into_response(),
    };
    let page_offset = params.offset;
    let count_mode_label = routed_read_count_mode_label(params.count_mode.as_deref());
    let routed_by = routed_by_for_routes(app, &routes);
    params.offset = 0;
    params.limit = Some(limits.max_page_limit.max(1));
    params.count_mode = Some("exact".to_owned());
    let collected = match collect_torii_paginated_list_json_payloads(
        &routes,
        limits.max_page_limit.max(1),
        app.query_fanout_working_set_bytes,
        app.torii_proxy_max_response_bytes,
        |route, route_offset, route_limit| {
            let mut page_params = params.clone();
            page_params.offset = route_offset;
            page_params.limit = Some(route_limit);
            let proxy_memory = proxy_memory.clone();
            async move {
                let query_string = match encode_torii_proxy_query(&page_params) {
                    Ok(query_string) => query_string,
                    Err(error) => return error.into_response(),
                };
                execute_torii_read_for_route(
                    app,
                    route,
                    torii_read_request(
                        ToriiReadEndpointV1::AccountsList,
                        route,
                        Vec::new(),
                        query_string,
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
    let ToriiFanoutJsonPayloads {
        payloads,
        diagnostics,
        budget,
    } = collected;
    merge_with_torii_fanout_headers(diagnostics, || {
        merged_paginated_list_response(
            payloads,
            page_offset,
            page_limit,
            count_mode_label,
            routed_by,
            budget,
        )
    })
}
