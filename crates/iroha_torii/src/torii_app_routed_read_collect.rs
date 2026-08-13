// Bounded collection and decode helpers for application routed reads.
#[cfg(feature = "app_api")]
#[derive(Debug)]
struct ToriiFanoutJsonPayloads {
    payloads: Vec<Value>,
    diagnostics: ToriiFanoutDiagnostics,
    budget: ToriiRoutedReadMemoryBudget,
}
#[cfg(feature = "app_api")]
#[derive(Debug)]
struct ToriiFanoutRoutedJsonPayloads {
    payloads: Vec<(RoutingDecision, Value)>,
    diagnostics: ToriiFanoutDiagnostics,
    budget: ToriiRoutedReadMemoryBudget,
}
#[cfg(feature = "app_api")]
async fn collect_torii_singleton_json_payloads<F, Fut>(
    routes: &[RoutingDecision],
    working_set_bytes: usize,
    max_body_bytes: usize,
    mut fetch: F,
) -> Result<ToriiFanoutJsonPayloads, Response>
where
    F: FnMut(RoutingDecision) -> Fut,
    Fut: std::future::Future<Output = Response>,
{
    let mut diagnostics = ToriiFanoutDiagnostics::default();
    let mut last_not_found = None;
    let mut last_route_unavailable = None;
    let mut budget = ToriiRoutedReadMemoryBudget::new(working_set_bytes, max_body_bytes)?;
    let mut payloads = budget.try_retained_vec(routes.len())?;
    for route in routes {
        diagnostics.record_attempt();
        let response = fetch(*route).await;
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
        match torii_json_body_value(response, &mut budget).await {
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
    Ok(ToriiFanoutJsonPayloads {
        payloads,
        diagnostics,
        budget,
    })
}
#[cfg(feature = "app_api")]
async fn collect_torii_list_json_payloads<F, Fut>(
    routes: &[RoutingDecision],
    working_set_bytes: usize,
    max_body_bytes: usize,
    mut fetch: F,
) -> Result<ToriiFanoutJsonPayloads, Response>
where
    F: FnMut(RoutingDecision) -> Fut,
    Fut: std::future::Future<Output = Response>,
{
    let mut diagnostics = ToriiFanoutDiagnostics::default();
    let mut last_not_found = None;
    let mut last_route_unavailable = None;
    let mut budget = ToriiRoutedReadMemoryBudget::new(working_set_bytes, max_body_bytes)?;
    let mut payloads = budget.try_retained_vec(routes.len())?;
    for route in routes {
        diagnostics.record_attempt();
        let response = fetch(*route).await;
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
        match torii_json_body_value(response, &mut budget).await {
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
    Ok(ToriiFanoutJsonPayloads {
        payloads,
        diagnostics,
        budget,
    })
}
#[cfg(feature = "app_api")]
async fn collect_torii_routed_list_json_payloads<F, Fut>(
    routes: &[RoutingDecision],
    working_set_bytes: usize,
    max_body_bytes: usize,
    mut fetch: F,
) -> Result<ToriiFanoutRoutedJsonPayloads, Response>
where
    F: FnMut(RoutingDecision) -> Fut,
    Fut: std::future::Future<Output = Response>,
{
    let mut diagnostics = ToriiFanoutDiagnostics::default();
    let mut last_not_found = None;
    let mut last_route_unavailable = None;
    let mut budget = ToriiRoutedReadMemoryBudget::new(working_set_bytes, max_body_bytes)?;
    let mut payloads = budget.try_retained_vec(routes.len())?;
    for route in routes {
        diagnostics.record_attempt();
        let response = fetch(*route).await;
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
        match torii_json_body_value(response, &mut budget).await {
            Ok(payload) => {
                diagnostics.record_success();
                budget.push_retained(&mut payloads, (*route, payload))?;
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
    Ok(ToriiFanoutRoutedJsonPayloads {
        payloads,
        diagnostics,
        budget,
    })
}
#[cfg(feature = "app_api")]
async fn collect_torii_paginated_list_json_payloads<F, Fut>(
    routes: &[RoutingDecision],
    page_limit: u64,
    working_set_bytes: usize,
    max_body_bytes: usize,
    mut fetch: F,
) -> Result<ToriiFanoutJsonPayloads, Response>
where
    F: FnMut(RoutingDecision, u64, u64) -> Fut,
    Fut: std::future::Future<Output = Response>,
{
    let mut diagnostics = ToriiFanoutDiagnostics::default();
    let mut last_not_found = None;
    let mut last_route_unavailable = None;
    let mut budget = ToriiRoutedReadMemoryBudget::new(working_set_bytes, max_body_bytes)?;
    let mut payloads = budget.try_retained_vec(routes.len())?;
    for route in routes {
        diagnostics.record_attempt();
        let mut route_offset = 0_u64;
        let mut route_total = None;
        let mut route_succeeded = false;
        loop {
            let response = fetch(*route, route_offset, page_limit).await;
            if response.status() == StatusCode::NOT_FOUND {
                diagnostics.record_skipped_response(&response);
                if route_succeeded {
                    return Err(with_torii_fanout_headers(response, diagnostics));
                }
                last_not_found = Some(response);
                break;
            }
            if torii_response_has_reject_code(&response, "route_unavailable") {
                diagnostics.record_skipped_response(&response);
                if route_succeeded {
                    return Err(with_torii_fanout_headers(response, diagnostics));
                }
                last_route_unavailable = Some(response);
                break;
            }
            let payload = match torii_json_body_value(response, &mut budget).await {
                Ok(payload) => payload,
                Err(response) => {
                    diagnostics.record_skipped_response(&response);
                    return Err(with_torii_fanout_headers(response, diagnostics));
                }
            };
            let page = match validate_torii_exact_list_page(
                &payload,
                route_offset,
                page_limit,
                route_total,
            ) {
                Ok(page) => page,
                Err(response) => {
                    diagnostics.record_skipped_response(&response);
                    return Err(with_torii_fanout_headers(response, diagnostics));
                }
            };
            route_total = Some(page.total);
            route_succeeded = true;
            budget.push_retained(&mut payloads, payload)?;
            if !page.has_more {
                break;
            }
            route_offset = route_offset.saturating_add(page.item_count);
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
    Ok(ToriiFanoutJsonPayloads {
        payloads,
        diagnostics,
        budget,
    })
}
#[cfg(feature = "app_api")]
async fn collect_torii_alias_json_payloads<F, Fut>(
    routes: &[RoutingDecision],
    denied_routes: usize,
    permission_denied_message: &'static str,
    working_set_bytes: usize,
    max_body_bytes: usize,
    mut fetch: F,
) -> Result<ToriiFanoutJsonPayloads, Response>
where
    F: FnMut(RoutingDecision) -> Fut,
    Fut: std::future::Future<Output = Response>,
{
    let mut diagnostics = ToriiFanoutDiagnostics::default();
    let mut last_not_found = None;
    let mut last_route_unavailable = None;
    let mut last_permission_denied = None;
    let mut budget = ToriiRoutedReadMemoryBudget::new(working_set_bytes, max_body_bytes)?;
    let mut payloads = budget.try_retained_vec(routes.len())?;
    for _ in 0..denied_routes {
        diagnostics.record_denied();
    }
    if routes.is_empty() {
        let response = if diagnostics.denied_routes > 0 {
            torii_alias_permission_denied_response(permission_denied_message)
        } else {
            torii_proxy_error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                "no visible dataspace returned a matching alias",
            )
        };
        return Err(with_torii_fanout_headers(response, diagnostics));
    }
    for route in routes {
        diagnostics.record_attempt();
        let response = fetch(*route).await;
        if torii_response_has_reject_code(&response, "permission_denied") {
            diagnostics.record_denied();
            last_permission_denied = Some(response);
            continue;
        }
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
        match torii_json_body_value(response, &mut budget).await {
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
        let response = last_permission_denied.unwrap_or_else(|| {
            if diagnostics.denied_routes > 0 {
                torii_alias_permission_denied_response(permission_denied_message)
            } else {
                last_not_found.unwrap_or_else(|| {
                    last_route_unavailable.unwrap_or_else(|| {
                        torii_proxy_error_response(
                            StatusCode::NOT_FOUND,
                            "not_found",
                            "no dataspace returned a matching result",
                        )
                    })
                })
            }
        });
        return Err(with_torii_fanout_headers(response, diagnostics));
    }
    Ok(ToriiFanoutJsonPayloads {
        payloads,
        diagnostics,
        budget,
    })
}
#[cfg(feature = "app_api")]
fn filter_permission_opened_alias_lookup_payload(
    app: &SharedAppState,
    caller: &AccountId,
    mut payload: Value,
) -> Result<Value, Response> {
    let state_view = app.state.view();
    let world = state_view.world();
    let object = payload.as_object_mut().ok_or_else(|| {
        torii_internal_json_error("alias by-account response must be a JSON object")
    })?;
    let items = object
        .get_mut("items")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            torii_internal_json_error("alias by-account response must include an `items` array")
        })?;
    let mut malformed_item = false;
    items.retain(|item| {
        let Some(alias_literal) = item
            .as_object()
            .and_then(|object| object.get("alias"))
            .and_then(Value::as_str)
        else {
            malformed_item = true;
            return false;
        };
        match parse_exact_account_alias_label_with_live_state(app, alias_literal) {
            Ok(alias)
                if torii_authority_can_resolve_resolved_account_alias(
                    world,
                    caller,
                    &alias.resolved,
                ) =>
            {
                true
            }
            Ok(_) => false,
            Err(error) => {
                iroha_logger::warn!(
                    alias = %alias_literal,
                    ?error,
                    "Torii alias-by-account permission filter dropped malformed alias literal"
                );
                false
            }
        }
    });
    if malformed_item {
        return Err(torii_internal_json_error(
            "alias by-account items must include string `alias`",
        ));
    }
    let total = u64::try_from(items.len()).map_err(|_| {
        torii_internal_json_error(
            "permission-filtered alias lookup result count does not fit in u64",
        )
    })?;
    let total_value = object.get_mut("total").ok_or_else(|| {
        torii_internal_json_error("alias by-account response must include `total`")
    })?;
    *total_value = Value::from(total);
    Ok(payload)
}
#[cfg(feature = "app_api")]
async fn collect_torii_alias_lookup_json_payloads<F, Fut>(
    app: &SharedAppState,
    routes: &[ToriiAliasLookupRouteAccess],
    denied_routes: usize,
    permission_denied_message: &'static str,
    caller: Option<&AccountId>,
    working_set_bytes: usize,
    max_body_bytes: usize,
    mut fetch: F,
) -> Result<ToriiFanoutJsonPayloads, Response>
where
    F: FnMut(RoutingDecision) -> Fut,
    Fut: std::future::Future<Output = Response>,
{
    let mut diagnostics = ToriiFanoutDiagnostics::default();
    let mut last_not_found = None;
    let mut last_route_unavailable = None;
    let mut last_permission_denied = None;
    let mut budget = ToriiRoutedReadMemoryBudget::new(working_set_bytes, max_body_bytes)?;
    let mut payloads = budget.try_retained_vec(routes.len())?;
    for _ in 0..denied_routes {
        diagnostics.record_denied();
    }
    if routes.is_empty() {
        let response = if diagnostics.denied_routes > 0 {
            torii_alias_permission_denied_response(permission_denied_message)
        } else {
            torii_proxy_error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                "no visible dataspace returned a matching alias",
            )
        };
        return Err(with_torii_fanout_headers(response, diagnostics));
    }
    for route in routes {
        diagnostics.record_attempt();
        let response = fetch(route.route).await;
        if torii_response_has_reject_code(&response, "permission_denied") {
            diagnostics.record_denied();
            last_permission_denied = Some(response);
            continue;
        }
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
        match torii_json_body_value(response, &mut budget).await {
            Ok(payload) => {
                let payload = if route.filter_by_permission {
                    match caller {
                        Some(caller) => {
                            filter_permission_opened_alias_lookup_payload(app, caller, payload)?
                        }
                        None => payload,
                    }
                } else {
                    payload
                };
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
        let response = last_permission_denied.unwrap_or_else(|| {
            if diagnostics.denied_routes > 0 {
                torii_alias_permission_denied_response(permission_denied_message)
            } else {
                last_not_found.unwrap_or_else(|| {
                    last_route_unavailable.unwrap_or_else(|| {
                        torii_proxy_error_response(
                            StatusCode::NOT_FOUND,
                            "not_found",
                            "no dataspace returned a matching result",
                        )
                    })
                })
            }
        });
        return Err(with_torii_fanout_headers(response, diagnostics));
    }
    Ok(ToriiFanoutJsonPayloads {
        payloads,
        diagnostics,
        budget,
    })
}
#[cfg(feature = "app_api")]
async fn bound_torii_single_route_response(
    response: Response,
    response_format: ToriiProxyResponseFormatV1,
    budget: &mut ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    let status = response.status();
    let (mut parts, body) = response.into_parts();
    let body_limit = if status.is_success() {
        budget.route_body_limit()
    } else {
        budget.final_body_limit()
    };
    let body = axum::body::to_bytes(body, body_limit)
        .await
        .map_err(|_| torii_routed_read_body_response())?;
    if !status.is_success() {
        parts.headers.remove(axum::http::header::CONTENT_LENGTH);
        return Ok(Response::from_parts(parts, Body::from(body)));
    }
    match response_format {
        ToriiProxyResponseFormatV1::Json => {
            let plan = budget.decode_plan(body.len())?;
            let profile = budget.json_profile(&body, plan)?;
            let (value, usage) = norito::core::with_decode_limits_measured(plan.limits, || {
                norito::json::from_slice::<Value>(&body)
            });
            let value = value.map_err(torii_routed_read_json_decode_response)?;
            budget.verify_json_value_usage(profile, usage)?;
            budget.retain_decode_usage(usage)?;
            drop(body);
            let canonical = budget.json_body(&value)?;
            parts.headers.remove(axum::http::header::CONTENT_LENGTH);
            parts.headers.insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            Ok(Response::from_parts(
                parts,
                Body::from(Bytes::from(canonical)),
            ))
        }
        ToriiProxyResponseFormatV1::Norito => {
            let header = norito::core::Header::read(std::io::Cursor::new(body.as_ref()))
                .map_err(torii_routed_read_norito_decode_response)?;
            if header.compression != norito::Compression::None
                || header.flags != norito::default_encode_flags()
            {
                return Err(torii_proxy_error_response(
                    StatusCode::BAD_GATEWAY,
                    "route_unavailable",
                    "proxied Norito response used a non-canonical layout",
                ));
            }
            torii_routed_read_ensure(
                "single-route Norito response",
                body.len(),
                budget.final_body_limit(),
            )?;
            parts.headers.remove(axum::http::header::CONTENT_LENGTH);
            Ok(Response::from_parts(parts, Body::from(body)))
        }
    }
}
#[cfg(feature = "app_api")]
async fn torii_json_body_value(
    response: Response,
    budget: &mut ToriiRoutedReadMemoryBudget,
) -> Result<Value, Response> {
    let status = response.status();
    if !status.is_success() {
        return Err(response);
    }
    let body = axum::body::to_bytes(response.into_body(), budget.route_body_limit())
        .await
        .map_err(|_| torii_routed_read_body_response())?;
    let plan = budget.decode_plan(body.len())?;
    let profile = budget.json_profile(&body, plan)?;
    let (value, usage) = norito::core::with_decode_limits_measured(plan.limits, || {
        norito::json::from_slice::<Value>(&body)
    });
    let value = value.map_err(torii_routed_read_json_decode_response)?;
    budget.verify_json_value_usage(profile, usage)?;
    budget.retain_decode_usage(usage)?;
    drop(body);
    let canonical = norito::json::to_json_bounded_boxed(&value, plan.canonical_limit_bytes)
        .map_err(|_| torii_routed_read_json_encode_response())?;
    drop(canonical);
    Ok(value)
}
#[cfg(feature = "app_api")]
async fn torii_json_body<T>(
    response: Response,
    _label: &str,
    budget: &mut ToriiRoutedReadMemoryBudget,
) -> Result<ToriiBoundedNoritoPayload<T>, Response>
where
    T: norito::json::JsonDeserializeOwned + norito::core::NoritoSerialize,
{
    let status = response.status();
    if !status.is_success() {
        return Err(response);
    }
    let body = axum::body::to_bytes(response.into_body(), budget.route_body_limit())
        .await
        .map_err(|_| torii_routed_read_body_response())?;
    let plan = budget.decode_plan(body.len())?;
    budget.json_profile(&body, plan)?;
    let (value, usage) = norito::core::with_decode_limits_measured(plan.limits, || {
        norito::json::from_slice::<T>(&body)
    });
    let value = value.map_err(torii_routed_read_json_decode_response)?;
    budget.retain_decode_usage(usage)?;
    drop(body);
    let canonical_bytes = norito::core::to_bytes_bounded(&value, plan.canonical_limit_bytes)
        .map_err(|_| torii_routed_read_norito_encode_response())?;
    budget.retain_canonical_capacity(canonical_bytes.capacity())?;
    Ok(ToriiBoundedNoritoPayload {
        value,
        canonical_bytes,
    })
}
#[cfg(feature = "app_api")]
async fn torii_norito_body<T>(
    response: Response,
    _label: &str,
    budget: &mut ToriiRoutedReadMemoryBudget,
) -> Result<ToriiBoundedNoritoPayload<T>, Response>
where
    T: norito::codec::Decode + norito::core::NoritoSerialize,
{
    let status = response.status();
    if !status.is_success() {
        return Err(response);
    }
    let body = axum::body::to_bytes(response.into_body(), budget.route_body_limit())
        .await
        .map_err(|_| torii_routed_read_body_response())?;
    let plan = budget.decode_plan(body.len())?;
    let (value, usage) = norito::core::with_decode_limits_measured(plan.limits, || {
        norito::decode_from_bytes_with_limits::<T>(&body, plan.limits)
    });
    let value = value.map_err(torii_routed_read_norito_decode_response)?;
    budget.retain_decode_usage(usage)?;
    drop(body);
    let canonical_bytes = norito::core::to_bytes_bounded(&value, plan.canonical_limit_bytes)
        .map_err(|_| torii_routed_read_norito_encode_response())?;
    budget.retain_canonical_capacity(canonical_bytes.capacity())?;
    Ok(ToriiBoundedNoritoPayload {
        value,
        canonical_bytes,
    })
}
