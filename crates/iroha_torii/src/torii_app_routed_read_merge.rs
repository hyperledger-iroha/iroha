// Bounded merge helpers for application routed reads.
#[cfg(feature = "app_api")]
fn parse_asset_definition_item_literal(literal: &str) -> Option<AssetDefinitionId> {
    literal
        .parse::<AssetDefinitionId>()
        .ok()
        .or_else(|| AssetDefinitionId::parse_address_literal(literal).ok())
}
#[cfg(feature = "app_api")]
fn asset_item_home_dataspace_id(app: &AppState, item: &Value) -> Option<DataSpaceId> {
    let object = item.as_object()?;
    if let Some(alias_literal) = object
        .get("asset_alias")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|literal| !literal.is_empty())
        && let Ok(alias) = alias_literal.parse::<AssetDefinitionAlias>()
    {
        return dataspace_id_for_alias_segment(app, alias.dataspace_segment());
    }
    for key in ["asset_definition_id", "asset"] {
        if let Some(definition_id) = object
            .get(key)
            .and_then(Value::as_str)
            .and_then(parse_asset_definition_item_literal)
        {
            return asset_definition_home_dataspace_id(app, &definition_id);
        }
    }
    for key in ["asset_id", "asset"] {
        if let Some(asset_id) = object
            .get(key)
            .and_then(Value::as_str)
            .and_then(|literal| AssetId::parse_literal(literal).ok())
        {
            return asset_definition_home_dataspace_id(app, asset_id.definition());
        }
    }
    None
}
#[cfg(feature = "app_api")]
fn asset_item_has_global_scope(item: &Value) -> bool {
    let Some(object) = item.as_object() else {
        return false;
    };
    if let Some(scope) = object.get("scope").and_then(Value::as_str) {
        return scope == "global";
    }
    for key in ["asset_id", "asset"] {
        if let Some(asset_id) = object
            .get(key)
            .and_then(Value::as_str)
            .and_then(|literal| AssetId::parse_literal(literal).ok())
        {
            return matches!(asset_id.scope(), AssetBalanceScope::Global);
        }
    }
    false
}
#[cfg(feature = "app_api")]
fn route_is_public_or_universal(app: &AppState, route: RoutingDecision) -> bool {
    if route.dataspace_id == DataSpaceId::UNIVERSAL {
        return true;
    }
    app.state
        .view()
        .nexus()
        .lane_catalog
        .lanes()
        .iter()
        .any(|lane| {
            lane.dataspace_id == route.dataspace_id
                && lane.visibility == iroha_data_model::nexus::LaneVisibility::Public
        })
}
#[cfg(feature = "app_api")]
fn should_keep_authoritative_global_item(
    app: &AppState,
    route: RoutingDecision,
    item: &Value,
) -> bool {
    if !asset_item_has_global_scope(item) {
        return true;
    }
    if let Some(home_dataspace_id) = asset_item_home_dataspace_id(app, item) {
        return home_dataspace_id == route.dataspace_id;
    }
    route_is_public_or_universal(app, route)
}
#[cfg(feature = "app_api")]
fn filter_non_authoritative_global_list_rows(
    app: &AppState,
    endpoint: ToriiReadEndpointV1,
    payloads: Vec<(RoutingDecision, Value)>,
) -> Result<Vec<(RoutingDecision, Value)>, Response> {
    if !matches!(
        endpoint,
        ToriiReadEndpointV1::AccountAssetsGet
            | ToriiReadEndpointV1::AccountAssetsQuery
            | ToriiReadEndpointV1::AssetHoldersGet
            | ToriiReadEndpointV1::AssetHoldersQuery
    ) {
        return Ok(payloads);
    }
    let mut payloads = payloads;
    for (route, payload) in &mut payloads {
        let Some(object) = payload.as_object_mut() else {
            return Err(torii_internal_json_error(
                "expected JSON object payload while filtering routed list response",
            ));
        };
        let Some(items) = object.get_mut("items").and_then(Value::as_array_mut) else {
            return Err(torii_internal_json_error(
                "expected `items` array while filtering routed list response",
            ));
        };
        items.retain(|item| {
            let keep = should_keep_authoritative_global_item(app, *route, item);
            if !keep {
                let asset = item
                    .as_object()
                    .and_then(|object| object.get("asset"))
                    .and_then(Value::as_str)
                    .unwrap_or("<unknown>");
                iroha_logger::debug!(
                    dataspace_id = %route.dataspace_id,
                    asset,
                    "suppressing non-authoritative global asset row from routed Torii merge"
                );
            }
            keep
        });
        let total = u64::try_from(items.len()).unwrap_or(u64::MAX);
        let Some(total_value) = object.get_mut("total") else {
            return Err(torii_internal_json_error(
                "expected `total` while filtering routed list response",
            ));
        };
        *total_value = Value::from(total);
    }
    Ok(payloads)
}
#[cfg(feature = "app_api")]
fn filter_non_authoritative_global_portfolio_rows(
    app: &AppState,
    endpoint: ToriiReadEndpointV1,
    payloads: Vec<(RoutingDecision, Value)>,
) -> Result<Vec<(RoutingDecision, Value)>, Response> {
    if !matches!(endpoint, ToriiReadEndpointV1::AccountsPortfolio) {
        return Ok(payloads);
    }
    let mut payloads = payloads;
    for (route, payload) in &mut payloads {
        let Some(object) = payload.as_object_mut() else {
            return Err(torii_internal_json_error(
                "expected JSON object payload while filtering portfolio response",
            ));
        };
        let Some(dataspaces) = object.get_mut("dataspaces").and_then(Value::as_array_mut) else {
            return Err(torii_internal_json_error(
                "expected `dataspaces` array while filtering portfolio response",
            ));
        };
        let mut total_accounts = 0_u64;
        let mut total_positions = 0_u64;
        for dataspace in dataspaces {
            let Some(dataspace_object) = dataspace.as_object_mut() else {
                return Err(torii_internal_json_error(
                    "portfolio dataspace rows must be JSON objects",
                ));
            };
            let Some(accounts) = dataspace_object
                .get_mut("accounts")
                .and_then(Value::as_array_mut)
            else {
                return Err(torii_internal_json_error(
                    "portfolio dataspace rows must include `accounts`",
                ));
            };
            for account in accounts {
                let Some(account_object) = account.as_object_mut() else {
                    return Err(torii_internal_json_error(
                        "portfolio account rows must be JSON objects",
                    ));
                };
                let Some(assets) = account_object
                    .get_mut("assets")
                    .and_then(Value::as_array_mut)
                else {
                    return Err(torii_internal_json_error(
                        "portfolio account rows must include `assets`",
                    ));
                };
                assets.retain(|asset| {
                    let keep = should_keep_authoritative_global_item(app, *route, asset);
                    if !keep {
                        let asset_literal = asset
                            .as_object()
                            .and_then(|object| {
                                object
                                    .get("asset_id")
                                    .or_else(|| object.get("asset"))
                                    .or_else(|| object.get("asset_definition_id"))
                            })
                            .and_then(Value::as_str)
                            .unwrap_or("<unknown>");
                        iroha_logger::debug!(
                            dataspace_id = %route.dataspace_id,
                            asset = asset_literal,
                            "suppressing non-authoritative global asset row from portfolio merge"
                        );
                    }
                    keep
                });
                total_accounts = total_accounts.saturating_add(1);
                total_positions =
                    total_positions.saturating_add(u64::try_from(assets.len()).unwrap_or(u64::MAX));
            }
        }
        let Some(totals) = object.get_mut("totals").and_then(Value::as_object_mut) else {
            return Err(torii_internal_json_error(
                "expected `totals` object while filtering portfolio response",
            ));
        };
        let Some(accounts_total) = totals.get_mut("accounts") else {
            return Err(torii_internal_json_error(
                "portfolio totals must include `accounts`",
            ));
        };
        *accounts_total = Value::from(total_accounts);
        let Some(positions_total) = totals.get_mut("positions") else {
            return Err(torii_internal_json_error(
                "portfolio totals must include `positions`",
            ));
        };
        *positions_total = Value::from(total_positions);
    }
    Ok(payloads)
}
#[cfg(feature = "app_api")]
fn list_items_from_payload<'a>(
    payload: &'a Value,
    context: &'static str,
) -> Result<&'a [Value], Response> {
    if let Some(items) = payload.as_array() {
        return Ok(items);
    }
    if let Some(items) = payload
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(Value::as_array)
    {
        return Ok(items);
    }
    Err(torii_internal_json_error(context))
}
#[cfg(feature = "app_api")]
fn list_items_from_owned_payload(
    payload: Value,
    context: &'static str,
) -> Result<Vec<Value>, Response> {
    match payload {
        Value::Array(items) => Ok(items),
        Value::Object(mut object) => match object.remove("items") {
            Some(Value::Array(items)) => Ok(items),
            _ => Err(torii_internal_json_error(context)),
        },
        _ => Err(torii_internal_json_error(context)),
    }
}
#[cfg(feature = "app_api")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ToriiExactListPage {
    item_count: u64,
    total: u64,
    has_more: bool,
}
#[cfg(feature = "app_api")]
fn validate_torii_exact_list_page(
    payload: &Value,
    page_offset: u64,
    page_limit: u64,
    expected_total: Option<u64>,
) -> Result<ToriiExactListPage, Response> {
    let object = payload.as_object().ok_or_else(|| {
        torii_internal_json_error("routed account-list page must be a JSON object")
    })?;
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            torii_internal_json_error("routed account-list page must include an `items` array")
        })?;
    let total = object.get("total").and_then(Value::as_u64).ok_or_else(|| {
        torii_internal_json_error("routed account-list page must include an exact `total`")
    })?;
    let has_more = object
        .get("has_more")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            torii_internal_json_error("routed account-list page must include boolean `has_more`")
        })?;
    if object.get("count_mode").and_then(Value::as_str) != Some("exact") {
        return Err(torii_internal_json_error(
            "routed account-list page must report `count_mode` as `exact`",
        ));
    }
    let item_count = u64::try_from(items.len()).unwrap_or(u64::MAX);
    if item_count > page_limit {
        return Err(torii_internal_json_error(format!(
            "routed account-list page returned {item_count} items for limit {page_limit}"
        )));
    }
    if expected_total.is_some_and(|expected| expected != total) {
        return Err(torii_internal_json_error(
            "routed account-list total changed while draining pages",
        ));
    }
    let page_end = page_offset
        .checked_add(item_count)
        .ok_or_else(|| torii_internal_json_error("routed account-list page offset overflowed"))?;
    if page_end > total {
        return Err(torii_internal_json_error(
            "routed account-list page extends beyond its exact total",
        ));
    }
    let expected_has_more = page_end < total;
    if has_more != expected_has_more || (has_more && item_count == 0) {
        return Err(torii_internal_json_error(
            "routed account-list page has inconsistent pagination metadata",
        ));
    }
    Ok(ToriiExactListPage {
        item_count,
        total,
        has_more,
    })
}
#[cfg(feature = "app_api")]
fn merged_list_response(
    payloads: Vec<Value>,
    routed_by: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_json_merge();
    let item_count = payloads.iter().try_fold(0_usize, |count, payload| {
        count.checked_add(
            list_items_from_payload(
                payload,
                "expected JSON object with `items` or JSON array payload while merging list response",
            )?
            .len(),
        )
        .ok_or_else(torii_routed_read_accounting_response)
    })?;
    budget.admit_merge_btree::<Vec<u8>, ()>(1, item_count)?;
    let mut seen = BTreeSet::<Vec<u8>>::new();
    let mut merged_items = budget.try_merge_vec(item_count)?;
    for payload in payloads {
        let payload_items = list_items_from_owned_payload(
            payload,
            "expected JSON object with `items` or JSON array payload while merging list response",
        )?;
        for item in payload_items {
            let key = budget.canonical_json_candidate(&item)?;
            if seen.contains(&key) {
                continue;
            }
            budget.retain_canonical_capacity(key.capacity())?;
            seen.insert(key);
            merged_items.push(item);
        }
    }
    drop(seen);
    budget.admit_merge_btree::<String, Value>(1, 2)?;
    budget.admit_merge_allocation("total".len() + "items".len())?;
    let mut root = norito::json::Map::new();
    root.insert("total".into(), Value::from(merged_items.len() as u64));
    root.insert("items".into(), Value::Array(merged_items));
    let root = Value::Object(root);
    let mut response = budget.json_response(&root)?;
    insert_routed_by_header(&mut response, routed_by);
    Ok(response)
}
#[cfg(feature = "app_api")]
fn merged_paginated_list_response(
    payloads: Vec<Value>,
    page_offset: u64,
    page_limit: u64,
    count_mode_label: &'static str,
    routed_by: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_json_merge();
    let item_count = payloads.iter().try_fold(0_usize, |count, payload| {
        count
            .checked_add(
                list_items_from_payload(
                    payload,
                    "expected JSON object with `items` while merging routed account list",
                )?
                .len(),
            )
            .ok_or_else(torii_routed_read_accounting_response)
    })?;
    budget.admit_merge_btree::<Vec<u8>, ()>(1, item_count)?;
    let mut seen = BTreeSet::<Vec<u8>>::new();
    let mut merged_items = budget.try_merge_vec(item_count)?;
    for payload in payloads {
        let payload_items = list_items_from_owned_payload(
            payload,
            "expected JSON object with `items` while merging routed account list",
        )?;
        for item in payload_items {
            let key = budget.canonical_json_candidate(&item)?;
            if seen.contains(&key) {
                continue;
            }
            budget.retain_canonical_capacity(key.capacity())?;
            seen.insert(key);
            merged_items.push(item);
        }
    }
    let total = merged_items.len();
    let start = usize::try_from(page_offset)
        .unwrap_or(usize::MAX)
        .min(total);
    let limit = usize::try_from(page_limit).unwrap_or(usize::MAX);
    let end = start.saturating_add(limit).min(total);
    let has_more = end < total;
    if start > 0 {
        merged_items.drain(..start);
    }
    merged_items.truncate(end.saturating_sub(start));
    drop(seen);
    let root_entries = if count_mode_label == "exact" { 4 } else { 3 };
    let root_key_bytes = "items".len()
        + "has_more".len()
        + "count_mode".len()
        + if count_mode_label == "exact" {
            "total".len()
        } else {
            0
        };
    budget.admit_merge_btree::<String, Value>(1, root_entries)?;
    budget.admit_merge_allocation(root_key_bytes + count_mode_label.len())?;
    let mut root = norito::json::Map::new();
    root.insert("items".into(), Value::Array(merged_items));
    if count_mode_label == "exact" {
        root.insert("total".into(), Value::from(total as u64));
    }
    root.insert("has_more".into(), Value::from(has_more));
    root.insert("count_mode".into(), Value::from(count_mode_label));
    let root = Value::Object(root);
    let mut response = budget.json_response(&root)?;
    insert_routed_by_header(&mut response, routed_by);
    Ok(response)
}
#[cfg(feature = "app_api")]
#[derive(Debug)]
struct AccountHistoryMergeItem {
    timestamp_ms: u64,
    canonical: Vec<u8>,
    value: Value,
}
#[cfg(feature = "app_api")]
impl AccountHistoryMergeItem {
    fn id(&self) -> &str {
        self.value
            .as_object()
            .and_then(|object| object.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
    }
}
#[cfg(feature = "app_api")]
fn account_history_merge_item(
    payload: Value,
    budget: &mut ToriiRoutedReadMemoryBudget,
) -> Result<AccountHistoryMergeItem, Response> {
    let canonical = budget.canonical_json_candidate(&payload)?;
    let object = payload
        .as_object()
        .ok_or_else(|| torii_internal_json_error("account history item must be a JSON object"))?;
    let timestamp_ms = object
        .get("timestamp_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Ok(AccountHistoryMergeItem {
        timestamp_ms,
        canonical,
        value: payload,
    })
}
#[cfg(feature = "app_api")]
fn account_history_count_mode_label(raw: Option<&str>) -> &'static str {
    match raw {
        Some("bounded") => "bounded",
        Some("exact") | None => "exact",
        Some(_) => "bounded",
    }
}
#[cfg(feature = "app_api")]
fn merged_account_history_response(
    payloads: Vec<Value>,
    page_offset: u64,
    page_limit: u64,
    count_mode_label: &'static str,
    routed_by: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_json_merge();
    let item_count = payloads.iter().try_fold(0_usize, |count, payload| {
        count
            .checked_add(
                list_items_from_payload(
                    payload,
                    "expected JSON object with `items` while merging account history response",
                )?
                .len(),
            )
            .ok_or_else(torii_routed_read_accounting_response)
    })?;
    budget.admit_merge_btree::<Vec<u8>, (u64, Value)>(1, item_count)?;
    let mut unique = BTreeMap::<Vec<u8>, (u64, Value)>::new();
    for payload in payloads {
        let payload_items = list_items_from_owned_payload(
            payload,
            "expected JSON object with `items` while merging account history response",
        )?;
        for item in payload_items {
            let merge_item = account_history_merge_item(item, &mut budget)?;
            if unique.contains_key(&merge_item.canonical) {
                continue;
            }
            budget.retain_canonical_capacity(merge_item.canonical.capacity())?;
            unique.insert(
                merge_item.canonical,
                (merge_item.timestamp_ms, merge_item.value),
            );
        }
    }
    let mut merged_items = budget.try_merge_vec(unique.len())?;
    merged_items.extend(
        unique.into_iter().map(
            |(canonical, (timestamp_ms, value))| AccountHistoryMergeItem {
                timestamp_ms,
                canonical,
                value,
            },
        ),
    );
    merged_items.sort_by(|left, right| {
        right
            .timestamp_ms
            .cmp(&left.timestamp_ms)
            .then_with(|| left.id().cmp(right.id()))
            .then_with(|| left.canonical.cmp(&right.canonical))
    });
    let total = merged_items.len();
    let start = usize::try_from(page_offset)
        .unwrap_or(usize::MAX)
        .min(total);
    let limit = usize::try_from(page_limit).unwrap_or(usize::MAX);
    let end = start.saturating_add(limit).min(total);
    let has_more = end < total;
    if start > 0 {
        merged_items.drain(..start);
    }
    merged_items.truncate(end.saturating_sub(start));
    let mut items = budget.try_merge_vec(merged_items.len())?;
    items.extend(merged_items.into_iter().map(|item| item.value));
    let root_entries = if count_mode_label == "exact" { 5 } else { 4 };
    let root_key_bytes = "items".len()
        + "has_more".len()
        + "count_mode".len()
        + "query_source".len()
        + if count_mode_label == "exact" {
            "total".len()
        } else {
            0
        };
    budget.admit_merge_btree::<String, Value>(1, root_entries)?;
    budget.admit_merge_allocation(
        root_key_bytes + count_mode_label.len() + "account_history_fanout".len(),
    )?;
    let mut root = norito::json::Map::new();
    root.insert("items".into(), Value::Array(items));
    if count_mode_label == "exact" {
        root.insert("total".into(), Value::from(total as u64));
    }
    root.insert("has_more".into(), Value::from(has_more));
    root.insert("count_mode".into(), Value::from(count_mode_label));
    root.insert("query_source".into(), Value::from("account_history_fanout"));
    let root = Value::Object(root);
    let mut response = budget.json_response(&root)?;
    insert_routed_by_header(&mut response, routed_by);
    Ok(response)
}
#[cfg(feature = "app_api")]
fn merged_account_read_response(
    payloads: Vec<ToriiBoundedNoritoPayload<AccountReadResponse>>,
    format: ResponseFormat,
    routed_by: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_typed_merge();
    budget.admit_merge_btree::<Vec<u8>, AccountReadResponse>(1, payloads.len())?;
    let mut unique_payloads = BTreeMap::<Vec<u8>, AccountReadResponse>::new();
    for payload in payloads {
        unique_payloads
            .entry(payload.canonical_bytes)
            .or_insert(payload.value);
    }
    match unique_payloads.len() {
        0 => Err(torii_proxy_error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "no dataspace returned a matching result",
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
                    .expect("build preflighted account-read response"),
                ResponseFormat::Json => {
                    drop(canonical_bytes);
                    budget.json_response(&payload)?
                }
            };
            insert_routed_by_header(&mut response, routed_by);
            Ok(response)
        }
        _ => Err(torii_proxy_error_response(
            StatusCode::CONFLICT,
            "route_conflict",
            "multiple dataspaces returned conflicting singleton results",
        )),
    }
}
#[cfg(feature = "app_api")]
fn merged_singleton_response(
    payloads: Vec<Value>,
    routed_by: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_json_merge();
    budget.admit_merge_btree::<Vec<u8>, Value>(1, payloads.len())?;
    let mut unique_payloads = BTreeMap::<Vec<u8>, Value>::new();
    for payload in payloads {
        let canonical = budget.canonical_json_candidate(&payload)?;
        if unique_payloads.contains_key(&canonical) {
            continue;
        }
        budget.retain_canonical_capacity(canonical.capacity())?;
        unique_payloads.insert(canonical, payload);
    }
    match unique_payloads.len() {
        0 => Err(torii_proxy_error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "no dataspace returned a matching result",
        )),
        1 => {
            let payload = unique_payloads
                .into_values()
                .next()
                .expect("singleton map length should be one");
            let mut response = budget.json_response(&payload)?;
            insert_routed_by_header(&mut response, routed_by);
            Ok(response)
        }
        _ => Err(torii_proxy_error_response(
            StatusCode::CONFLICT,
            "route_conflict",
            "multiple dataspaces returned conflicting singleton results",
        )),
    }
}
#[cfg(feature = "app_api")]
fn pipeline_status_payload_rank(payload: &Value) -> Result<u8, Response> {
    let kind = payload
        .as_object()
        .and_then(|object| object.get("status"))
        .and_then(Value::as_object)
        .and_then(|status| status.get("kind"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            torii_internal_json_error("routed pipeline status must include string `status.kind`")
        })?;
    match kind {
        "Queued" => Ok(0),
        "Approved" => Ok(1),
        "Expired" => Ok(2),
        "Rejected" => Ok(3),
        "Committed" => Ok(4),
        "Applied" => Ok(5),
        _ => Err(torii_internal_json_error(
            "routed pipeline status contained an unknown status kind",
        )),
    }
}
#[cfg(feature = "app_api")]
fn pipeline_status_payload_tie_break(payload: &Value) -> Result<(u8, u64), Response> {
    let object = payload.as_object().ok_or_else(|| {
        torii_internal_json_error("routed pipeline status payload must be a JSON object")
    })?;
    let resolved_from = object
        .get("resolved_from")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            torii_internal_json_error("routed pipeline status must include string `resolved_from`")
        })?;
    let source_rank = match resolved_from {
        "state" => 3,
        "cache" => 2,
        "queue" => 1,
        _ => 0,
    };
    let block_height = object
        .get("status")
        .and_then(Value::as_object)
        .and_then(|status| status.get("block_height"))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    Ok((source_rank, block_height))
}
#[cfg(feature = "app_api")]
fn merged_pipeline_status_response(
    payloads: Vec<Value>,
    routed_by: &'static str,
    budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    let mut budget = budget;
    budget.begin_json_merge();
    let mut best: Option<(Value, u8, (u8, u64))> = None;
    for payload in payloads {
        let rank = pipeline_status_payload_rank(&payload)?;
        let tie_break = pipeline_status_payload_tie_break(&payload)?;
        match best.as_ref() {
            None => best = Some((payload, rank, tie_break)),
            Some((current, current_rank, current_tie_break)) => {
                let payload_hash = payload
                    .as_object()
                    .and_then(|object| object.get("hash"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        torii_internal_json_error(
                            "routed pipeline status must include string `hash`",
                        )
                    })?;
                let current_hash = current
                    .as_object()
                    .and_then(|object| object.get("hash"))
                    .and_then(Value::as_str)
                    .expect("selected pipeline status was already validated");
                if payload_hash != current_hash {
                    return Err(torii_proxy_error_response(
                        StatusCode::CONFLICT,
                        "route_conflict",
                        "multiple dataspaces returned pipeline statuses for different hashes",
                    ));
                }
                if rank > *current_rank || (rank == *current_rank && tie_break > *current_tie_break)
                {
                    best = Some((payload, rank, tie_break));
                }
            }
        }
    }
    let Some((payload, _, _)) = best else {
        return Err(torii_proxy_error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "no dataspace returned a matching result",
        ));
    };
    let mut response = budget.json_response(&payload)?;
    insert_routed_by_header(&mut response, routed_by);
    Ok(response)
}
#[cfg(feature = "app_api")]
fn authorize_alias_resolve_index_payloads(
    app: &SharedAppState,
    caller: Option<&AccountId>,
    mut payloads: Vec<Value>,
) -> Result<Vec<Value>, Response> {
    let public_dataspaces = torii_public_dataspace_ids(app.as_ref());
    let mut index = 0;
    while index < payloads.len() {
        let payload = &payloads[index];
        let alias_literal = payload
            .as_object()
            .and_then(|object| object.get("alias"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                torii_internal_json_error("routed alias-index response must include string `alias`")
            })?;
        let alias =
            parse_exact_account_alias_label_with_live_state(app, alias_literal).map_err(|_| {
                torii_proxy_error_response(
                    StatusCode::CONFLICT,
                    "route_conflict",
                    "a routed alias-index response contained a non-canonical alias",
                )
            })?;
        if public_dataspaces.contains(&alias.label.dataspace) {
            index += 1;
            continue;
        }
        let Some(caller) = caller else {
            payloads.remove(index);
            continue;
        };
        if !torii_authority_can_resolve_resolved_account_alias(
            app.state.view().world(),
            caller,
            &alias.resolved,
        ) {
            return Err(torii_alias_permission_denied_response(
                "exact account-alias resolve permission is required for the returned alias-index binding",
            ));
        }
        index += 1;
    }
    Ok(payloads)
}
#[cfg(feature = "app_api")]
fn merged_alias_resolve_index_response(
    payloads: Vec<Value>,
    routed_by: &'static str,
    source: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_json_merge();
    let mut selected: Option<Value> = None;
    for payload in payloads {
        let object = payload.as_object().ok_or_else(|| {
            torii_internal_json_error("routed alias-index response must be a JSON object")
        })?;
        let index = object.get("index").and_then(Value::as_u64).ok_or_else(|| {
            torii_internal_json_error("routed alias-index response must include u64 `index`")
        })?;
        let alias = object.get("alias").and_then(Value::as_str).ok_or_else(|| {
            torii_internal_json_error("routed alias-index response must include string `alias`")
        })?;
        let account_id = object
            .get("account_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                torii_internal_json_error(
                    "routed alias-index response must include string `account_id`",
                )
            })?;
        if let Some(existing) = selected.as_ref() {
            let existing = existing
                .as_object()
                .expect("selected alias-index payload was already validated");
            if existing.get("index").and_then(Value::as_u64) != Some(index)
                || existing.get("alias").and_then(Value::as_str) != Some(alias)
                || existing.get("account_id").and_then(Value::as_str) != Some(account_id)
            {
                return Err(torii_proxy_error_response(
                    StatusCode::CONFLICT,
                    "route_conflict",
                    "multiple dataspaces returned conflicting alias-index bindings",
                ));
            }
        } else {
            selected = Some(payload);
        }
    }
    let Some(mut payload) = selected else {
        return Err(torii_proxy_error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "no dataspace returned a matching result",
        ));
    };
    let object = payload
        .as_object_mut()
        .expect("selected alias-index payload was already validated");
    budget.admit_merge_allocation(source.len())?;
    if let Some(value) = object.get_mut("source") {
        *value = Value::from(source);
    } else {
        // Inserting into a full root can allocate one sibling and a new root.
        budget.admit_merge_btree::<String, Value>(2, 2)?;
        budget.admit_merge_allocation("source".len())?;
        object.insert("source".to_owned(), Value::from(source));
    }
    let mut response = budget.json_response(&payload)?;
    insert_routed_by_header(&mut response, routed_by);
    Ok(response)
}
#[cfg(feature = "app_api")]
fn merged_alias_lookup_by_account_response(
    payloads: Vec<Value>,
    routed_by: &'static str,
    source: &'static str,
    denied_routes: usize,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_json_merge();
    let item_count = payloads.iter().try_fold(0_usize, |count, payload| {
        let items = payload
            .as_object()
            .and_then(|object| object.get("items"))
            .and_then(Value::as_array)
            .ok_or_else(|| {
                torii_internal_json_error(
                    "routed alias by-account response must include an `items` array",
                )
            })?;
        count
            .checked_add(items.len())
            .ok_or_else(torii_routed_read_accounting_response)
    })?;
    budget.admit_merge_btree::<Vec<u8>, ()>(1, item_count)?;
    let mut merged_items = budget.try_merge_vec(item_count)?;
    let mut account_id: Option<Value> = None;
    let mut seen = BTreeSet::<Vec<u8>>::new();
    for payload in payloads {
        let Value::Object(mut object) = payload else {
            return Err(torii_internal_json_error(
                "routed alias by-account response must be a JSON object",
            ));
        };
        let candidate_account_id = object.remove("account_id").ok_or_else(|| {
            torii_internal_json_error("routed alias by-account response must include `account_id`")
        })?;
        if candidate_account_id.as_str().is_none() {
            return Err(torii_internal_json_error(
                "routed alias by-account `account_id` must be a string",
            ));
        }
        match &account_id {
            Some(existing) if existing != &candidate_account_id => {
                return Err(torii_proxy_error_response(
                    StatusCode::CONFLICT,
                    "route_conflict",
                    "multiple dataspaces returned conflicting alias-account roots",
                ));
            }
            None => account_id = Some(candidate_account_id),
            Some(_) => {}
        }
        let items = match object.remove("items") {
            Some(Value::Array(items)) => items,
            _ => {
                return Err(torii_internal_json_error(
                    "routed alias by-account response must include an `items` array",
                ));
            }
        };
        for mut item in items {
            let item_object = item.as_object_mut().ok_or_else(|| {
                torii_internal_json_error("routed alias by-account items must be JSON objects")
            })?;
            for key in item_object.keys() {
                if !matches!(
                    key.as_str(),
                    "alias" | "dataspace" | "domain" | "is_primary"
                ) {
                    return Err(torii_internal_json_error(
                        "routed alias by-account item contained an unknown field",
                    ));
                }
            }
            if item_object.get("alias").and_then(Value::as_str).is_none()
                || item_object
                    .get("dataspace")
                    .and_then(Value::as_str)
                    .is_none()
                || item_object
                    .get("is_primary")
                    .and_then(Value::as_bool)
                    .is_none()
            {
                return Err(torii_internal_json_error(
                    "routed alias by-account item has invalid required fields",
                ));
            }
            match item_object.get("domain") {
                None | Some(Value::Null) => {
                    item_object.remove("domain");
                }
                Some(value) if value.as_str().is_some() => {}
                Some(_) => {
                    return Err(torii_internal_json_error(
                        "routed alias by-account item `domain` must be a string or null",
                    ));
                }
            }
            let key = budget.canonical_json_candidate(&item)?;
            if seen.contains(&key) {
                continue;
            }
            budget.retain_canonical_capacity(key.capacity())?;
            seen.insert(key);
            merged_items.push(item);
        }
    }
    if merged_items.is_empty() && denied_routes > 0 {
        return Err(torii_alias_permission_denied_response(
            "one or more dataspace routes denied the alias-by-account lookup and no allowed route returned aliases",
        ));
    }
    if merged_items.len() > EXACT_ALIAS_LOOKUP_MAX_ITEMS {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        ))
        .into_response());
    }
    merged_items.sort_by(|left, right| {
        let left = left
            .as_object()
            .expect("merged alias item was already validated");
        let right = right
            .as_object()
            .expect("merged alias item was already validated");
        left.get("alias")
            .and_then(Value::as_str)
            .cmp(&right.get("alias").and_then(Value::as_str))
            .then_with(|| {
                left.get("dataspace")
                    .and_then(Value::as_str)
                    .cmp(&right.get("dataspace").and_then(Value::as_str))
            })
            .then_with(|| {
                left.get("domain")
                    .and_then(Value::as_str)
                    .cmp(&right.get("domain").and_then(Value::as_str))
            })
            .then_with(|| {
                right
                    .get("is_primary")
                    .and_then(Value::as_bool)
                    .cmp(&left.get("is_primary").and_then(Value::as_bool))
            })
    });
    drop(seen);
    budget.admit_merge_btree::<String, Value>(1, 4)?;
    budget.admit_merge_allocation(
        "account_id".len() + "total".len() + "items".len() + "source".len() + source.len(),
    )?;
    let mut root = norito::json::Map::new();
    root.insert(
        "account_id".to_owned(),
        account_id.unwrap_or_else(|| Value::String(String::new())),
    );
    root.insert(
        "total".to_owned(),
        Value::from(u64::try_from(merged_items.len()).unwrap_or(u64::MAX)),
    );
    root.insert("items".to_owned(), Value::Array(merged_items));
    root.insert("source".to_owned(), Value::from(source));
    let mut response = budget.json_response(&Value::Object(root))?;
    insert_routed_by_header(&mut response, routed_by);
    Ok(response)
}
#[cfg(feature = "app_api")]
fn merged_space_directory_bindings_response(
    payloads: Vec<Value>,
    routed_by: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_json_merge();
    let (row_count, account_count) =
        payloads
            .iter()
            .try_fold((0_usize, 0_usize), |(rows_seen, accounts_seen), payload| {
                let rows = payload
                    .as_object()
                    .and_then(|object| object.get("dataspaces"))
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        torii_internal_json_error(
                            "expected `dataspaces` array while merging space-directory bindings",
                        )
                    })?;
                let rows_seen = rows_seen
                    .checked_add(rows.len())
                    .ok_or_else(torii_routed_read_accounting_response)?;
                let accounts_seen = rows.iter().try_fold(accounts_seen, |count, row| {
                    let accounts = row
                        .as_object()
                        .and_then(|object| object.get("accounts"))
                        .and_then(Value::as_array)
                        .ok_or_else(|| {
                            torii_internal_json_error(
                                "space-directory binding rows must include `accounts`",
                            )
                        })?;
                    count
                        .checked_add(accounts.len())
                        .ok_or_else(torii_routed_read_accounting_response)
                })?;
                Ok((rows_seen, accounts_seen))
            })?;
    budget.admit_merge_btree::<u64, (Option<String>, BTreeSet<String>)>(1, row_count)?;
    budget.admit_merge_btree::<String, ()>(row_count, account_count)?;
    let mut uaid: Option<String> = None;
    let mut dataspaces = BTreeMap::<u64, (Option<String>, BTreeSet<String>)>::new();
    for payload in payloads {
        let Value::Object(mut object) = payload else {
            return Err(torii_internal_json_error(
                "expected JSON object payload while merging space-directory bindings",
            ));
        };
        if let Some(Value::String(value)) = object.remove("uaid") {
            match &uaid {
                Some(existing) if existing != &value => {
                    return Err(torii_proxy_error_response(
                        StatusCode::CONFLICT,
                        "route_conflict",
                        "multiple dataspaces returned conflicting UAID bindings roots",
                    ));
                }
                None => uaid = Some(value),
                Some(_) => {}
            }
        }
        let rows = match object.remove("dataspaces") {
            Some(Value::Array(rows)) => rows,
            _ => {
                return Err(torii_internal_json_error(
                    "expected `dataspaces` array while merging space-directory bindings",
                ));
            }
        };
        for row in rows {
            let Value::Object(mut row) = row else {
                return Err(torii_internal_json_error(
                    "space-directory binding rows must be JSON objects",
                ));
            };
            let dataspace_id = row
                .remove("dataspace_id")
                .and_then(|value| value.as_u64())
                .ok_or_else(|| {
                    torii_internal_json_error(
                        "space-directory binding rows must include `dataspace_id`",
                    )
                })?;
            let alias = match row.remove("dataspace_alias") {
                Some(Value::String(alias)) => Some(alias),
                None | Some(Value::Null) => None,
                Some(_) => {
                    return Err(torii_internal_json_error(
                        "space-directory binding `dataspace_alias` must be a string or null",
                    ));
                }
            };
            let accounts = match row.remove("accounts") {
                Some(Value::Array(accounts)) => accounts,
                _ => {
                    return Err(torii_internal_json_error(
                        "space-directory binding rows must include `accounts`",
                    ));
                }
            };
            let entry = dataspaces
                .entry(dataspace_id)
                .or_insert_with(|| (None, BTreeSet::new()));
            if entry.0.is_none() {
                entry.0 = alias;
            }
            for account in accounts {
                let Value::String(account_literal) = account else {
                    return Err(torii_internal_json_error(
                        "space-directory binding accounts must be strings",
                    ));
                };
                entry.1.insert(account_literal);
            }
        }
    }
    let dataspace_count = dataspaces.len();
    let output_map_count = dataspace_count
        .checked_add(1)
        .ok_or_else(torii_routed_read_accounting_response)?;
    let output_map_entries = dataspace_count
        .checked_mul(3)
        .and_then(|entries| entries.checked_add(2))
        .ok_or_else(torii_routed_read_accounting_response)?;
    budget.admit_merge_btree::<String, Value>(output_map_count, output_map_entries)?;
    let fixed_key_bytes = dataspace_count
        .checked_mul("dataspace_id".len() + "dataspace_alias".len() + "accounts".len())
        .and_then(|bytes| bytes.checked_add("uaid".len() + "dataspaces".len()))
        .ok_or_else(torii_routed_read_accounting_response)?;
    budget.admit_merge_allocation(fixed_key_bytes)?;
    let mut rows = budget.try_merge_vec(dataspace_count)?;
    for (dataspace_id, (alias, accounts)) in dataspaces {
        let mut account_values = budget.try_merge_vec(accounts.len())?;
        account_values.extend(accounts.into_iter().map(Value::from));
        let mut row = norito::json::Map::new();
        row.insert("dataspace_id".to_owned(), Value::from(dataspace_id));
        row.insert(
            "dataspace_alias".to_owned(),
            alias.map(Value::from).unwrap_or(Value::Null),
        );
        row.insert("accounts".to_owned(), Value::Array(account_values));
        rows.push(Value::Object(row));
    }
    let mut root = norito::json::Map::new();
    root.insert(
        "uaid".to_owned(),
        uaid.map(Value::from)
            .unwrap_or(Value::String(String::new())),
    );
    root.insert("dataspaces".to_owned(), Value::Array(rows));
    let mut response = budget.json_response(&Value::Object(root))?;
    insert_routed_by_header(&mut response, routed_by);
    Ok(response)
}
#[cfg(feature = "app_api")]
fn merged_space_directory_manifests_response(
    payloads: Vec<Value>,
    offset: u64,
    limit: Option<u64>,
    routed_by: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_json_merge();
    let (row_count, hash_bytes) =
        payloads
            .iter()
            .try_fold((0_usize, 0_usize), |(rows_seen, hash_bytes), payload| {
                let rows = payload
                    .as_object()
                    .and_then(|object| object.get("manifests"))
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        torii_internal_json_error(
                            "expected `manifests` array while merging space-directory manifests",
                        )
                    })?;
                let rows_seen = rows_seen
                    .checked_add(rows.len())
                    .ok_or_else(torii_routed_read_accounting_response)?;
                let hash_bytes = rows.iter().try_fold(hash_bytes, |bytes, row| {
                    let hash = row
                        .as_object()
                        .and_then(|object| object.get("manifest_hash"))
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            torii_internal_json_error(
                                "space-directory manifests must include `manifest_hash`",
                            )
                        })?;
                    bytes
                        .checked_add(hash.len())
                        .ok_or_else(torii_routed_read_accounting_response)
                })?;
                Ok((rows_seen, hash_bytes))
            })?;
    budget.admit_merge_btree::<(u64, String), (Vec<u8>, Value)>(1, row_count)?;
    budget.admit_merge_allocation(hash_bytes)?;
    let mut uaid: Option<String> = None;
    let mut explicit_total = 0u64;
    let mut saw_explicit_total = false;
    let mut manifests = BTreeMap::<(u64, String), (Vec<u8>, Value)>::new();
    for payload in payloads {
        let Value::Object(mut object) = payload else {
            return Err(torii_internal_json_error(
                "expected JSON object payload while merging space-directory manifests",
            ));
        };
        if let Some(Value::String(value)) = object.remove("uaid") {
            match &uaid {
                Some(existing) if existing != &value => {
                    return Err(torii_proxy_error_response(
                        StatusCode::CONFLICT,
                        "route_conflict",
                        "multiple dataspaces returned conflicting UAID manifest roots",
                    ));
                }
                None => uaid = Some(value),
                Some(_) => {}
            }
        }
        if let Some(total) = object.remove("total").and_then(|value| value.as_u64()) {
            saw_explicit_total = true;
            explicit_total = explicit_total.saturating_add(total);
        }
        let rows = match object.remove("manifests") {
            Some(Value::Array(rows)) => rows,
            _ => {
                return Err(torii_internal_json_error(
                    "expected `manifests` array while merging space-directory manifests",
                ));
            }
        };
        for row in rows {
            let object = row.as_object().ok_or_else(|| {
                torii_internal_json_error("space-directory manifests must be JSON objects")
            })?;
            let Some(dataspace_id) = object.get("dataspace_id").and_then(Value::as_u64) else {
                return Err(torii_internal_json_error(
                    "space-directory manifests must include `dataspace_id`",
                ));
            };
            let Some(manifest_hash) = object.get("manifest_hash").and_then(Value::as_str) else {
                return Err(torii_internal_json_error(
                    "space-directory manifests must include `manifest_hash`",
                ));
            };
            let key = (dataspace_id, manifest_hash.to_owned());
            let canonical = budget.canonical_json_candidate(&row)?;
            if let Some((existing_canonical, _)) = manifests.get(&key) {
                if existing_canonical != &canonical {
                    return Err(torii_proxy_error_response(
                        StatusCode::CONFLICT,
                        "route_conflict",
                        "multiple dataspaces returned conflicting manifest records",
                    ));
                }
                continue;
            }
            budget.retain_canonical_capacity(canonical.capacity())?;
            manifests.insert(key, (canonical, row));
        }
    }
    let total = if saw_explicit_total {
        explicit_total
    } else {
        manifests.len() as u64
    };
    let mut merged = budget.try_merge_vec(manifests.len())?;
    merged.extend(manifests.into_values().map(|(_, value)| value));
    let offset = usize::try_from(offset).unwrap_or(usize::MAX);
    if offset >= merged.len() {
        merged.clear();
    } else if offset > 0 {
        merged.drain(0..offset);
    }
    if let Some(limit) = limit {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        if merged.len() > limit {
            merged.truncate(limit);
        }
    }
    budget.admit_merge_btree::<String, Value>(1, 3)?;
    budget.admit_merge_allocation("uaid".len() + "total".len() + "manifests".len())?;
    let mut root = norito::json::Map::new();
    root.insert(
        "uaid".to_owned(),
        uaid.map(Value::from)
            .unwrap_or(Value::String(String::new())),
    );
    root.insert("total".to_owned(), Value::from(total));
    root.insert("manifests".to_owned(), Value::Array(merged));
    let mut response = budget.json_response(&Value::Object(root))?;
    insert_routed_by_header(&mut response, routed_by);
    Ok(response)
}
#[cfg(feature = "app_api")]
fn merged_portfolio_response(
    payloads: Vec<Value>,
    routed_by: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_json_merge();
    let row_count = payloads.iter().try_fold(0_usize, |count, payload| {
        let rows = payload
            .as_object()
            .and_then(|object| object.get("dataspaces"))
            .and_then(Value::as_array)
            .ok_or_else(|| {
                torii_internal_json_error(
                    "expected `dataspaces` array while merging portfolio response",
                )
            })?;
        count
            .checked_add(rows.len())
            .ok_or_else(torii_routed_read_accounting_response)
    })?;
    budget.admit_merge_btree::<u64, Value>(1, row_count)?;
    let mut uaid: Option<String> = None;
    let mut dataspaces = BTreeMap::<u64, Value>::new();
    for payload in payloads {
        let Value::Object(mut object) = payload else {
            return Err(torii_internal_json_error(
                "expected JSON object payload while merging portfolio response",
            ));
        };
        if uaid.is_none() {
            uaid = match object.remove("uaid") {
                Some(Value::String(value)) => Some(value),
                _ => None,
            };
        }
        let rows = match object.remove("dataspaces") {
            Some(Value::Array(rows)) => rows,
            _ => {
                return Err(torii_internal_json_error(
                    "expected `dataspaces` array while merging portfolio response",
                ));
            }
        };
        for row in rows {
            let Some(dataspace_id) = row.get("dataspace_id").and_then(Value::as_u64) else {
                return Err(torii_internal_json_error(
                    "portfolio dataspace rows must include `dataspace_id`",
                ));
            };
            dataspaces.entry(dataspace_id).or_insert(row);
        }
    }
    let mut total_accounts = 0u64;
    let mut total_positions = 0u64;
    for row in dataspaces.values() {
        let account_count = row
            .get("accounts")
            .and_then(Value::as_array)
            .map(|accounts| accounts.len() as u64)
            .unwrap_or(0);
        total_accounts = total_accounts.saturating_add(account_count);
        let position_count = row
            .get("accounts")
            .and_then(Value::as_array)
            .map(|accounts| {
                accounts
                    .iter()
                    .map(|account| {
                        account
                            .get("assets")
                            .and_then(Value::as_array)
                            .map(|assets| assets.len() as u64)
                            .unwrap_or(0)
                    })
                    .sum::<u64>()
            })
            .unwrap_or(0);
        total_positions = total_positions.saturating_add(position_count);
    }
    budget.admit_merge_btree::<String, Value>(2, 5)?;
    budget.admit_merge_allocation(
        "accounts".len() + "positions".len() + "uaid".len() + "totals".len() + "dataspaces".len(),
    )?;
    let mut totals = norito::json::Map::new();
    totals.insert("accounts".into(), Value::from(total_accounts));
    totals.insert("positions".into(), Value::from(total_positions));
    let mut merged_dataspaces = budget.try_merge_vec(dataspaces.len())?;
    merged_dataspaces.extend(dataspaces.into_values());
    let mut root = norito::json::Map::new();
    root.insert("uaid".into(), uaid.map(Value::from).unwrap_or(Value::Null));
    root.insert("totals".into(), Value::Object(totals));
    root.insert("dataspaces".into(), Value::Array(merged_dataspaces));
    let mut response = budget.json_response(&Value::Object(root))?;
    insert_routed_by_header(&mut response, routed_by);
    Ok(response)
}
#[cfg(feature = "app_api")]
fn merged_dataspace_summary_response(
    payloads: Vec<Value>,
    routed_by: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_json_merge();
    let (row_count, account_count, account_bytes) = payloads.iter().try_fold(
        (0_usize, 0_usize, 0_usize),
        |(rows_seen, accounts_seen, account_bytes), payload| {
            let rows = payload
                .as_object()
                .and_then(|object| object.get("dataspaces"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    torii_internal_json_error(
                        "expected `dataspaces` array while merging dataspace summary response",
                    )
                })?;
            let rows_seen = rows_seen
                .checked_add(rows.len())
                .ok_or_else(torii_routed_read_accounting_response)?;
            let (accounts_seen, account_bytes) =
                rows.iter()
                    .try_fold((accounts_seen, account_bytes), |(count, bytes), row| {
                        let Some(accounts) = row.get("accounts").and_then(Value::as_array) else {
                            return Ok((count, bytes));
                        };
                        let count = count
                            .checked_add(accounts.len())
                            .ok_or_else(torii_routed_read_accounting_response)?;
                        let bytes = accounts.iter().try_fold(bytes, |bytes, account| {
                            let account = account.as_str().ok_or_else(|| {
                                torii_internal_json_error(
                                    "dataspace summary account bindings must be strings",
                                )
                            })?;
                            bytes
                                .checked_add(account.len())
                                .ok_or_else(torii_routed_read_accounting_response)
                        })?;
                        Ok((count, bytes))
                    })?;
            Ok((rows_seen, accounts_seen, account_bytes))
        },
    )?;
    budget.admit_merge_btree::<u64, Value>(1, row_count)?;
    budget.admit_merge_btree::<String, ()>(1, account_count)?;
    budget.admit_merge_allocation(account_bytes)?;
    let mut account_literal: Option<String> = None;
    let mut account_id: Option<String> = None;
    let mut uaid: Option<Value> = None;
    let mut dataspaces = BTreeMap::<u64, Value>::new();
    let mut unique_accounts = BTreeSet::<String>::new();
    let mut portfolio_accounts_total = 0u64;
    let mut portfolio_positions_total = 0u64;
    let mut manifests_total = 0u64;
    let mut manifests_active = 0u64;
    let mut consensus_entries_total = 0u64;
    let mut consensus_tx_total = 0u64;
    let mut consensus_chunks_total = 0u64;
    let mut consensus_rbc_bytes_total = 0u64;
    let mut consensus_teu_total = 0u64;
    for payload in payloads {
        let Value::Object(mut object) = payload else {
            return Err(torii_internal_json_error(
                "expected JSON object payload while merging dataspace summary response",
            ));
        };
        if account_literal.is_none() {
            account_literal = match object.remove("account") {
                Some(Value::String(value)) => Some(value),
                _ => None,
            };
        }
        if account_id.is_none() {
            account_id = match object.remove("account_id") {
                Some(Value::String(value)) => Some(value),
                _ => None,
            };
        }
        if uaid.is_none() {
            uaid = object.remove("uaid");
        }
        let rows = match object.remove("dataspaces") {
            Some(Value::Array(rows)) => rows,
            _ => {
                return Err(torii_internal_json_error(
                    "expected `dataspaces` array while merging dataspace summary response",
                ));
            }
        };
        for row in rows {
            let Some(dataspace_id) = row.get("dataspace_id").and_then(Value::as_u64) else {
                return Err(torii_internal_json_error(
                    "dataspace summary rows must include `dataspace_id`",
                ));
            };
            if dataspaces.contains_key(&dataspace_id) {
                dataspaces.insert(dataspace_id, row);
                continue;
            }
            if let Some(accounts) = row.get("accounts").and_then(Value::as_array) {
                for account in accounts {
                    if let Some(account) = account.as_str() {
                        unique_accounts.insert(account.to_owned());
                    }
                }
            }
            let portfolio = row.get("portfolio").and_then(Value::as_object);
            portfolio_accounts_total = portfolio_accounts_total.saturating_add(
                portfolio
                    .and_then(|portfolio| portfolio.get("accounts"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            );
            portfolio_positions_total = portfolio_positions_total.saturating_add(
                portfolio
                    .and_then(|portfolio| portfolio.get("positions"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            );
            let manifest = row.get("manifest").and_then(Value::as_object);
            if manifest
                .and_then(|manifest| manifest.get("present"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                manifests_total = manifests_total.saturating_add(1);
            }
            if manifest
                .and_then(|manifest| manifest.get("active"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                manifests_active = manifests_active.saturating_add(1);
            }
            let consensus = row.get("consensus").and_then(Value::as_object);
            consensus_entries_total = consensus_entries_total.saturating_add(
                consensus
                    .and_then(|consensus| consensus.get("entries"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            );
            consensus_tx_total = consensus_tx_total.saturating_add(
                consensus
                    .and_then(|consensus| consensus.get("tx_count"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            );
            consensus_chunks_total = consensus_chunks_total.saturating_add(
                consensus
                    .and_then(|consensus| consensus.get("total_chunks"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            );
            consensus_rbc_bytes_total = consensus_rbc_bytes_total.saturating_add(
                consensus
                    .and_then(|consensus| consensus.get("rbc_bytes_total"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            );
            consensus_teu_total = consensus_teu_total.saturating_add(
                consensus
                    .and_then(|consensus| consensus.get("teu_total"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            );
            dataspaces.insert(dataspace_id, row);
        }
    }
    let accounts_bound = unique_accounts.len();
    drop(unique_accounts);
    let fixed_key_bytes = [
        "dataspaces",
        "accounts_bound",
        "portfolio_accounts",
        "portfolio_positions",
        "manifests_total",
        "manifests_active",
        "consensus_entries",
        "consensus_tx_count",
        "consensus_chunks_total",
        "consensus_rbc_bytes_total",
        "consensus_teu_total",
        "account",
        "account_id",
        "uaid",
        "totals",
        "dataspaces",
    ]
    .into_iter()
    .try_fold(0_usize, |bytes, key| bytes.checked_add(key.len()))
    .ok_or_else(torii_routed_read_accounting_response)?;
    budget.admit_merge_btree::<String, Value>(2, 16)?;
    budget.admit_merge_allocation(fixed_key_bytes)?;
    let mut totals = norito::json::Map::new();
    totals.insert("dataspaces".into(), Value::from(dataspaces.len() as u64));
    totals.insert("accounts_bound".into(), Value::from(accounts_bound as u64));
    totals.insert(
        "portfolio_accounts".into(),
        Value::from(portfolio_accounts_total),
    );
    totals.insert(
        "portfolio_positions".into(),
        Value::from(portfolio_positions_total),
    );
    totals.insert("manifests_total".into(), Value::from(manifests_total));
    totals.insert("manifests_active".into(), Value::from(manifests_active));
    totals.insert(
        "consensus_entries".into(),
        Value::from(consensus_entries_total),
    );
    totals.insert("consensus_tx_count".into(), Value::from(consensus_tx_total));
    totals.insert(
        "consensus_chunks_total".into(),
        Value::from(consensus_chunks_total),
    );
    totals.insert(
        "consensus_rbc_bytes_total".into(),
        Value::from(consensus_rbc_bytes_total),
    );
    totals.insert(
        "consensus_teu_total".into(),
        Value::from(consensus_teu_total),
    );
    let mut merged_dataspaces = budget.try_merge_vec(dataspaces.len())?;
    merged_dataspaces.extend(dataspaces.into_values());
    let mut root = norito::json::Map::new();
    root.insert(
        "account".into(),
        account_literal.map(Value::from).unwrap_or(Value::Null),
    );
    root.insert(
        "account_id".into(),
        account_id.map(Value::from).unwrap_or(Value::Null),
    );
    root.insert("uaid".into(), uaid.unwrap_or(Value::Null));
    root.insert("totals".into(), Value::Object(totals));
    root.insert("dataspaces".into(), Value::Array(merged_dataspaces));
    let mut response = budget.json_response(&Value::Object(root))?;
    insert_routed_by_header(&mut response, routed_by);
    Ok(response)
}
