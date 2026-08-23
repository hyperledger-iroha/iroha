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
fn routed_read_count_mode_label(raw: Option<&str>) -> &'static str {
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
fn pipeline_status_payload_hash(payload: &Value) -> Result<&str, Response> {
    payload
        .as_object()
        .and_then(|object| object.get("hash"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            torii_internal_json_error("routed pipeline status must include string `hash`")
        })
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
        // Validate every candidate before it can become `best`; otherwise a malformed first
        // payload survives until the next comparison and turns the merge into a panic.
        let _ = pipeline_status_payload_hash(&payload)?;
        match best.as_ref() {
            None => best = Some((payload, rank, tie_break)),
            Some((current, current_rank, current_tie_break)) => {
                let payload_hash = pipeline_status_payload_hash(&payload)?;
                let current_hash = pipeline_status_payload_hash(current)?;
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
fn space_directory_manifest_optional_epoch(
    lifecycle: &Map,
    field: &'static str,
) -> Result<Option<u64>, Response> {
    match lifecycle.get(field) {
        Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            torii_internal_json_error(
                "space-directory manifest lifecycle epochs must be unsigned integers or null",
            )
        }),
        None => Err(torii_internal_json_error(
            "space-directory manifest lifecycle is missing a required epoch field",
        )),
    }
}
#[cfg(feature = "app_api")]
fn space_directory_manifest_row_status(row: &Map) -> Result<&'static str, Response> {
    let lifecycle = row
        .get("lifecycle")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            torii_internal_json_error("space-directory manifests must include a lifecycle object")
        })?;
    if lifecycle.keys().any(|field| {
        !matches!(
            field.as_str(),
            "activated_epoch" | "expired_epoch" | "revocation"
        )
    }) {
        return Err(torii_internal_json_error(
            "space-directory manifest lifecycle contains an unknown field",
        ));
    }
    let activated_epoch = space_directory_manifest_optional_epoch(lifecycle, "activated_epoch")?;
    let expired_epoch = space_directory_manifest_optional_epoch(lifecycle, "expired_epoch")?;
    let revoked = match lifecycle.get("revocation") {
        Some(Value::Null) => false,
        Some(Value::Object(revocation)) => {
            if revocation
                .keys()
                .any(|field| !matches!(field.as_str(), "epoch" | "reason"))
            {
                return Err(torii_internal_json_error(
                    "space-directory manifest revocation contains an unknown field",
                ));
            }
            if revocation.get("epoch").and_then(Value::as_u64).is_none()
                || !matches!(
                    revocation.get("reason"),
                    Some(Value::Null | Value::String(_))
                )
            {
                return Err(torii_internal_json_error(
                    "space-directory manifest revocation metadata is malformed",
                ));
            }
            true
        }
        _ => {
            return Err(torii_internal_json_error(
                "space-directory manifest lifecycle must include revocation metadata or null",
            ));
        }
    };
    let derived_status = if revoked {
        "Revoked"
    } else if expired_epoch.is_some() {
        "Expired"
    } else if activated_epoch.is_some() {
        "Active"
    } else {
        "Pending"
    };
    if row.get("status").and_then(Value::as_str) != Some(derived_status) {
        return Err(torii_internal_json_error(
            "space-directory manifest status disagrees with its lifecycle",
        ));
    }
    Ok(derived_status)
}
#[cfg(feature = "app_api")]
fn merged_space_directory_manifests_response(
    payloads: Vec<Value>,
    offset: u64,
    limit: Option<u64>,
    fanout_incomplete: bool,
    requested_count_mode: &'static str,
    requested_dataspace: Option<u64>,
    requested_status: routing::SpaceDirectoryManifestStatus,
    expected_uaid: &str,
    routed_by: &'static str,
    mut budget: ToriiRoutedReadMemoryBudget,
) -> Result<Response, Response> {
    budget.begin_json_merge();
    let expected_uaid_hash = expected_uaid
        .strip_prefix("uaid:")
        .and_then(|value| value.parse::<Hash>().ok())
        .ok_or_else(|| {
            torii_internal_json_error(
                "space-directory manifest merge requires a canonical expected UAID",
            )
        })?;
    let shard_row_limit = match limit {
        Some(limit) => offset.checked_add(limit).ok_or_else(|| {
            torii_internal_json_error("space-directory manifest page window overflows")
        })?,
        None => u64::MAX,
    };
    let mut upstream_has_more = fanout_incomplete;
    let mut shard_count_mode = None;
    let mut row_count = 0_usize;
    for payload in &payloads {
        let object = payload.as_object().ok_or_else(|| {
            torii_internal_json_error(
                "expected JSON object payload while merging space-directory manifests",
            )
        })?;
        if object.keys().any(|field| {
            !matches!(
                field.as_str(),
                "uaid" | "total" | "has_more" | "count_mode" | "manifests"
            )
        }) {
            return Err(torii_internal_json_error(
                "space-directory manifest pages contain an unknown field",
            ));
        }
        let has_more = object
            .get("has_more")
            .and_then(Value::as_bool)
            .ok_or_else(|| {
                torii_internal_json_error(
                    "space-directory manifest pages must include boolean `has_more`",
                )
            })?;
        upstream_has_more |= has_more;
        let count_mode = match object.get("count_mode").and_then(Value::as_str) {
            Some("exact") => "exact",
            Some("bounded") => "bounded",
            _ => {
                return Err(torii_internal_json_error(
                    "space-directory manifest pages must include a supported `count_mode`",
                ));
            }
        };
        if shard_count_mode.is_some_and(|existing| existing != count_mode) {
            return Err(torii_proxy_error_response(
                StatusCode::CONFLICT,
                "route_conflict",
                "multiple dataspaces returned conflicting manifest count modes",
            ));
        }
        shard_count_mode = Some(count_mode);
        let total = object.get("total").and_then(Value::as_u64).ok_or_else(|| {
            torii_internal_json_error(
                "space-directory manifest pages must include unsigned `total`",
            )
        })?;
        let rows = payload
            .as_object()
            .and_then(|object| object.get("manifests"))
            .and_then(Value::as_array)
            .ok_or_else(|| {
                torii_internal_json_error(
                    "expected `manifests` array while merging space-directory manifests",
                )
            })?;
        let returned = u64::try_from(rows.len()).unwrap_or(u64::MAX);
        if returned > shard_row_limit {
            return Err(torii_internal_json_error(
                "space-directory manifest shard returned more rows than the requested prefix",
            ));
        }
        if has_more && returned != shard_row_limit {
            return Err(torii_internal_json_error(
                "a continuing space-directory manifest shard must return the full requested prefix",
            ));
        }
        if requested_dataspace.is_some() && (has_more || total > 1 || returned > 1) {
            return Err(torii_internal_json_error(
                "a dataspace-filtered manifest page cannot contain or advertise multiple rows",
            ));
        }
        if total < returned || (!has_more && total != returned) || (has_more && total <= returned) {
            return Err(torii_internal_json_error(
                "space-directory manifest page metadata is inconsistent with its rows",
            ));
        }
        let mut previous_dataspace = None;
        for row in rows {
            let Some(dataspace_id) = row
                .as_object()
                .and_then(|object| object.get("dataspace_id"))
                .and_then(Value::as_u64)
            else {
                return Err(torii_internal_json_error(
                    "space-directory manifests must include `dataspace_id`",
                ));
            };
            if previous_dataspace.is_some_and(|previous| previous >= dataspace_id) {
                return Err(torii_internal_json_error(
                    "space-directory manifest shard rows must be strictly ordered by dataspace",
                ));
            }
            previous_dataspace = Some(dataspace_id);
        }
        row_count = row_count
            .checked_add(rows.len())
            .ok_or_else(torii_routed_read_accounting_response)?;
    }
    let count_mode = if upstream_has_more || requested_count_mode == "bounded" {
        "bounded"
    } else {
        "exact"
    };
    budget.admit_merge_btree::<u64, (Vec<u8>, Value)>(1, row_count)?;
    let mut uaid: Option<String> = None;
    let mut manifests = BTreeMap::<u64, (Vec<u8>, Value)>::new();
    for payload in payloads {
        let Value::Object(mut object) = payload else {
            return Err(torii_internal_json_error(
                "expected JSON object payload while merging space-directory manifests",
            ));
        };
        let value = match object.remove("uaid") {
            Some(Value::String(value)) => value,
            _ => {
                return Err(torii_internal_json_error(
                    "space-directory manifest pages must include string `uaid`",
                ));
            }
        };
        if value != expected_uaid {
            return Err(torii_proxy_error_response(
                StatusCode::CONFLICT,
                "route_conflict",
                "a dataspace returned manifests for a different UAID",
            ));
        }
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
        let rows = match object.remove("manifests") {
            Some(Value::Array(rows)) => rows,
            _ => {
                return Err(torii_internal_json_error(
                    "expected `manifests` array while merging space-directory manifests",
                ));
            }
        };
        for row in rows {
            let Value::Object(mut object) = row else {
                return Err(torii_internal_json_error(
                    "space-directory manifests must be JSON objects",
                ));
            };
            if object.keys().any(|field| {
                !matches!(
                    field.as_str(),
                    "dataspace_id"
                        | "dataspace_alias"
                        | "manifest"
                        | "manifest_hash"
                        | "status"
                        | "lifecycle"
                        | "accounts"
                )
            }) {
                return Err(torii_internal_json_error(
                    "space-directory manifest rows contain an unknown field",
                ));
            }
            let Some(dataspace_id) = object.get("dataspace_id").and_then(Value::as_u64) else {
                return Err(torii_internal_json_error(
                    "space-directory manifests must include `dataspace_id`",
                ));
            };
            if !matches!(
                object.get("dataspace_alias"),
                Some(Value::Null | Value::String(_))
            ) {
                return Err(torii_internal_json_error(
                    "space-directory manifests must include a string or null `dataspace_alias`",
                ));
            }
            let accounts = object
                .get("accounts")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    torii_internal_json_error(
                        "space-directory manifests must include an `accounts` array",
                    )
                })?;
            if accounts.iter().any(|account| account.as_str().is_none()) {
                return Err(torii_internal_json_error(
                    "space-directory manifest accounts must be string literals",
                ));
            }
            for account in accounts {
                let literal = account
                    .as_str()
                    .expect("account strings were validated above");
                if literal.trim() != literal {
                    return Err(torii_internal_json_error(
                        "space-directory manifest accounts must be canonical I105 literals",
                    ));
                }
                budget.inspect_typed_json_candidate::<
                    iroha_data_model::account::AccountId,
                    _,
                    _,
                >(
                    account,
                    "space-directory manifest accounts must be canonical I105 literals",
                    |value| {
                        <iroha_data_model::account::AccountId as norito::json::JsonDeserialize>::json_from_value(value)
                    },
                    |_| Ok(()),
                )?;
            }
            if requested_dataspace.is_some_and(|requested| requested != dataspace_id) {
                return Err(torii_proxy_error_response(
                    StatusCode::CONFLICT,
                    "route_conflict",
                    "a dataspace returned a manifest outside the requested dataspace filter",
                ));
            }
            let derived_status = space_directory_manifest_row_status(&object)?;
            let status_matches = match requested_status {
                routing::SpaceDirectoryManifestStatus::All => true,
                routing::SpaceDirectoryManifestStatus::Active => derived_status == "Active",
                routing::SpaceDirectoryManifestStatus::Inactive => derived_status != "Active",
            };
            if !status_matches {
                return Err(torii_proxy_error_response(
                    StatusCode::CONFLICT,
                    "route_conflict",
                    "a dataspace returned a manifest outside the requested status filter",
                ));
            }
            let Some(manifest_hash) = object.get("manifest_hash").and_then(Value::as_str) else {
                return Err(torii_internal_json_error(
                    "space-directory manifests must include `manifest_hash`",
                ));
            };
            if manifest_hash.len() != Hash::LENGTH * 2
                || !manifest_hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(torii_internal_json_error(
                    "space-directory manifest hashes must be canonical lowercase hex",
                ));
            }
            let supplied_manifest_hash = manifest_hash.parse::<Hash>().map_err(|_| {
                torii_internal_json_error(
                    "space-directory manifest hashes must be canonical lowercase hex",
                )
            })?;
            let manifest_value = object.remove("manifest").ok_or_else(|| {
                torii_internal_json_error(
                    "space-directory manifests must include a typed `manifest`",
                )
            })?;
            if manifest_value
                .as_object()
                .and_then(|manifest| manifest.get("uaid"))
                .and_then(Value::as_str)
                != Some(expected_uaid)
            {
                return Err(torii_proxy_error_response(
                    StatusCode::CONFLICT,
                    "route_conflict",
                    "a dataspace returned a manifest with a non-canonical nested UAID",
                ));
            }
            budget.inspect_typed_json_candidate::<
                iroha_data_model::nexus::AssetPermissionManifest,
                _,
                _,
            >(
                &manifest_value,
                "space-directory manifests must include a valid typed `manifest`",
                |value| {
                    <iroha_data_model::nexus::AssetPermissionManifest as norito::json::JsonDeserialize>::json_from_value(value)
                },
                |manifest| {
                    if manifest.uaid.as_hash() != &expected_uaid_hash
                        || manifest.dataspace.as_u64() != dataspace_id
                    {
                        return Err(torii_proxy_error_response(
                            StatusCode::CONFLICT,
                            "route_conflict",
                            "a dataspace returned a manifest whose nested identity does not match its row",
                        ));
                    }
                    let expected_manifest_hash: Hash = HashOf::new(manifest).into();
                    if supplied_manifest_hash != expected_manifest_hash {
                        return Err(torii_internal_json_error(
                            "space-directory manifest hash does not match the typed manifest",
                        ));
                    }
                    Ok(())
                },
            )?;
            object.insert("manifest".to_owned(), manifest_value);
            let row = Value::Object(object);
            let canonical = budget.canonical_json_candidate(&row)?;
            if let Some((existing_canonical, _)) = manifests.get(&dataspace_id) {
                if existing_canonical != &canonical {
                    return Err(torii_proxy_error_response(
                        StatusCode::CONFLICT,
                        "route_conflict",
                        "multiple routes returned conflicting manifests for one dataspace",
                    ));
                }
                continue;
            }
            budget.retain_canonical_capacity(canonical.capacity())?;
            manifests.insert(dataspace_id, (canonical, row));
        }
    }
    let total = manifests.len();
    let mut merged = budget.try_merge_vec(manifests.len())?;
    merged.extend(manifests.into_values().map(|(_, value)| value));
    let start = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(merged.len());
    let limit = limit
        .map(|limit| usize::try_from(limit).unwrap_or(usize::MAX))
        .unwrap_or(usize::MAX);
    let end = start.saturating_add(limit).min(merged.len());
    let has_more = upstream_has_more || end < merged.len();
    if start > 0 {
        merged.drain(..start);
    }
    merged.truncate(end.saturating_sub(start));
    budget.admit_merge_btree::<String, Value>(1, 5)?;
    budget.admit_merge_allocation(
        "uaid".len()
            + "total".len()
            + "has_more".len()
            + "count_mode".len()
            + "manifests".len()
            + count_mode.len(),
    )?;
    let Some(uaid) = uaid else {
        return Err(torii_internal_json_error(
            "space-directory manifest fanout returned no payloads",
        ));
    };
    let mut root = norito::json::Map::new();
    root.insert("uaid".to_owned(), Value::from(uaid));
    root.insert(
        "total".to_owned(),
        Value::from(u64::try_from(total).unwrap_or(u64::MAX)),
    );
    root.insert("has_more".to_owned(), Value::from(has_more));
    root.insert("count_mode".to_owned(), Value::from(count_mode));
    root.insert("manifests".to_owned(), Value::Array(merged));
    let mut response = budget.json_response(&Value::Object(root))?;
    insert_routed_by_header(&mut response, routed_by);
    Ok(response)
}

#[cfg(all(test, feature = "app_api"))]
mod routed_read_merge_regression_tests {
    use super::*;

    const TEST_BODY_BYTES: usize = 1024 * 1024;

    fn test_budget() -> ToriiRoutedReadMemoryBudget {
        ToriiRoutedReadMemoryBudget::new(
            routed_read_working_set_for_phase(TEST_BODY_BYTES),
            TEST_BODY_BYTES,
        )
        .expect("routed-read merge test memory envelope should fit")
    }

    fn test_manifest_uaid() -> iroha_data_model::nexus::UniversalAccountId {
        iroha_data_model::nexus::UniversalAccountId::from_hash(Hash::new(
            b"torii-space-directory-merge-test-uaid",
        ))
    }

    fn test_manifest_uaid_literal() -> String {
        test_manifest_uaid().to_string()
    }

    fn test_manifest_row(dataspace_id: u64, status: &'static str, issued_ms: u64) -> Value {
        let manifest = iroha_data_model::nexus::AssetPermissionManifest {
            version: iroha_data_model::nexus::ManifestVersion::V1,
            uaid: test_manifest_uaid(),
            dataspace: DataSpaceId::new(dataspace_id),
            issued_ms,
            activation_epoch: 1,
            expiry_epoch: None,
            entries: Vec::new(),
        };
        let manifest_hash: Hash = HashOf::new(&manifest).into();
        let lifecycle = match status {
            "Pending" => norito::json!({
                "activated_epoch": null,
                "expired_epoch": null,
                "revocation": null
            }),
            "Active" => norito::json!({
                "activated_epoch": 1,
                "expired_epoch": null,
                "revocation": null
            }),
            "Expired" => norito::json!({
                "activated_epoch": 1,
                "expired_epoch": 2,
                "revocation": null
            }),
            "Revoked" => norito::json!({
                "activated_epoch": 1,
                "expired_epoch": null,
                "revocation": {"epoch": 2, "reason": null}
            }),
            _ => panic!("unsupported test manifest status"),
        };
        let mut row = Map::new();
        row.insert("dataspace_id".to_owned(), Value::from(dataspace_id));
        row.insert(
            "manifest".to_owned(),
            norito::json::to_value(&manifest).expect("encode test manifest"),
        );
        row.insert(
            "manifest_hash".to_owned(),
            Value::from(hex::encode(manifest_hash.as_ref())),
        );
        row.insert("status".to_owned(), Value::from(status));
        row.insert("lifecycle".to_owned(), lifecycle);
        row.insert("dataspace_alias".to_owned(), Value::Null);
        row.insert("accounts".to_owned(), Value::Array(Vec::new()));
        Value::Object(row)
    }

    fn test_manifest_payload(
        rows: Vec<Value>,
        total: u64,
        has_more: bool,
        count_mode: &'static str,
    ) -> Value {
        let mut payload = Map::new();
        payload.insert("uaid".to_owned(), Value::from(test_manifest_uaid_literal()));
        payload.insert("total".to_owned(), Value::from(total));
        payload.insert("has_more".to_owned(), Value::from(has_more));
        payload.insert("count_mode".to_owned(), Value::from(count_mode));
        payload.insert("manifests".to_owned(), Value::Array(rows));
        Value::Object(payload)
    }

    fn assert_invalid_manifest_proxy_response(response: &Response) {
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_response")
        );
    }

    #[test]
    fn manifest_fanout_forces_bounded_shard_counting_without_losing_client_intent() {
        for (raw, expected) in [
            (None, "exact"),
            (Some("exact"), "exact"),
            (Some("bounded"), "bounded"),
            (Some("unsupported"), "bounded"),
        ] {
            let query = routing::SpaceDirectoryManifestQuery {
                count_mode: raw.map(str::to_owned),
                ..routing::SpaceDirectoryManifestQuery::default()
            };
            let (shard_query, requested) =
                bounded_space_directory_manifest_shard_query(query, 2, 3)
                    .expect("valid fanout window");
            assert_eq!(requested, expected);
            assert_eq!(shard_query.count_mode.as_deref(), Some("bounded"));
            assert_eq!(shard_query.offset, Some(0));
            assert_eq!(shard_query.limit, Some(5));
        }

        let query = routing::SpaceDirectoryManifestQuery {
            dataspace: Some(7),
            limit: Some(u64::MAX),
            offset: Some(u64::MAX),
            ..routing::SpaceDirectoryManifestQuery::default()
        };
        let (shard_query, _) = bounded_space_directory_manifest_shard_query(query, 2, 3)
            .expect("a filtered manifest fanout has a one-row shard prefix");
        assert_eq!(shard_query.offset, Some(0));
        assert_eq!(shard_query.limit, Some(1));
    }

    #[test]
    fn pipeline_status_merge_rejects_a_missing_hash_in_every_position() {
        let missing_hash = norito::json!({
            "status": {"kind": "Queued"},
            "resolved_from": "queue"
        });
        let valid = norito::json!({
            "hash": "abc",
            "status": {"kind": "Applied", "block_height": 7},
            "resolved_from": "state"
        });

        for payloads in [
            vec![missing_hash.clone(), valid.clone()],
            vec![missing_hash.clone()],
            vec![valid, missing_hash],
        ] {
            let response = merged_pipeline_status_response(payloads, "proxy", test_budget())
                .expect_err("a pipeline status without a hash must be rejected");
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(
                response
                    .headers()
                    .get("x-iroha-reject-code")
                    .and_then(|value| value.to_str().ok()),
                Some("invalid_proxy_response")
            );
        }
    }

    #[tokio::test]
    async fn space_directory_manifest_merge_counts_deduplicated_entries() {
        let manifest = test_manifest_row(7, "Active", 1);
        let payload = test_manifest_payload(vec![manifest], 1, false, "bounded");
        let expected_uaid = test_manifest_uaid_literal();
        let response = merged_space_directory_manifests_response(
            vec![payload.clone(), payload],
            0,
            None,
            false,
            "exact",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect("identical manifests should merge successfully");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("manifest merge response body");
        let payload: Value = norito::json::from_slice(&body).expect("manifest merge response JSON");
        assert_eq!(payload["total"].as_u64(), Some(1));
        assert_eq!(payload["has_more"].as_bool(), Some(false));
        assert_eq!(payload["count_mode"].as_str(), Some("exact"));
        assert_eq!(payload["manifests"].as_array().map(Vec::len), Some(1));
    }

    #[tokio::test]
    async fn space_directory_manifest_merge_marks_partial_prefix_as_bounded() {
        let payload =
            test_manifest_payload(vec![test_manifest_row(7, "Active", 1)], 2, true, "bounded");
        let expected_uaid = test_manifest_uaid_literal();
        let response = merged_space_directory_manifests_response(
            vec![payload],
            0,
            Some(1),
            false,
            "exact",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect("a partial shard prefix should merge successfully");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("manifest merge response body");
        let payload: Value = norito::json::from_slice(&body).expect("manifest merge response JSON");
        assert_eq!(payload["total"].as_u64(), Some(1));
        assert_eq!(payload["has_more"].as_bool(), Some(true));
        assert_eq!(payload["count_mode"].as_str(), Some("bounded"));
    }

    #[tokio::test]
    async fn space_directory_manifest_merge_marks_incomplete_fanout_as_bounded() {
        let payload =
            test_manifest_payload(vec![test_manifest_row(7, "Active", 1)], 1, false, "bounded");
        let expected_uaid = test_manifest_uaid_literal();
        let response = merged_space_directory_manifests_response(
            vec![payload],
            0,
            Some(1),
            true,
            "exact",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect("a partial fanout should return an explicitly bounded page");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("manifest merge response body");
        let payload: Value = norito::json::from_slice(&body).expect("manifest merge response JSON");
        assert_eq!(payload["total"].as_u64(), Some(1));
        assert_eq!(payload["has_more"].as_bool(), Some(true));
        assert_eq!(payload["count_mode"].as_str(), Some("bounded"));
    }

    #[tokio::test]
    async fn space_directory_manifest_merge_preserves_bounded_client_intent() {
        let payload =
            test_manifest_payload(vec![test_manifest_row(7, "Active", 1)], 1, false, "bounded");
        let expected_uaid = test_manifest_uaid_literal();
        let response = merged_space_directory_manifests_response(
            vec![payload],
            0,
            Some(1),
            false,
            "bounded",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect("a bounded client request should merge successfully");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("manifest merge response body");
        let payload: Value = norito::json::from_slice(&body).expect("manifest merge response JSON");
        assert_eq!(payload["has_more"].as_bool(), Some(false));
        assert_eq!(payload["count_mode"].as_str(), Some("bounded"));
    }

    #[test]
    fn space_directory_manifest_merge_rejects_conflicting_dataspace_records() {
        let expected_uaid = test_manifest_uaid_literal();
        let response = merged_space_directory_manifests_response(
            vec![
                test_manifest_payload(vec![test_manifest_row(7, "Active", 1)], 1, false, "bounded"),
                test_manifest_payload(vec![test_manifest_row(7, "Active", 2)], 1, false, "bounded"),
            ],
            0,
            Some(1),
            false,
            "exact",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect_err("one dataspace cannot resolve to two different manifests");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_conflict")
        );
    }

    #[test]
    fn space_directory_manifest_merge_rejects_the_wrong_uaid() {
        let mut payload =
            test_manifest_payload(vec![test_manifest_row(7, "Active", 1)], 1, false, "bounded");
        payload
            .as_object_mut()
            .expect("payload object")
            .insert("uaid".to_owned(), Value::from("uaid:wrong"));
        let expected_uaid = test_manifest_uaid_literal();
        let response = merged_space_directory_manifests_response(
            vec![payload],
            0,
            Some(1),
            false,
            "exact",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect_err("a response for another UAID must not be merged");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_conflict")
        );
    }

    #[test]
    fn space_directory_manifest_merge_rejects_incoherent_page_metadata() {
        let row = test_manifest_row(7, "Active", 1);
        let mut missing_total = test_manifest_payload(vec![row.clone()], 1, false, "bounded");
        missing_total
            .as_object_mut()
            .expect("payload object")
            .remove("total");
        let false_terminal = test_manifest_payload(vec![row.clone()], 100, false, "bounded");
        let false_continuation = test_manifest_payload(vec![row.clone()], 1, true, "bounded");
        let mut unknown_page_field = test_manifest_payload(vec![row], 1, false, "bounded");
        unknown_page_field
            .as_object_mut()
            .expect("payload object")
            .insert("untrusted".to_owned(), Value::Bool(true));
        let expected_uaid = test_manifest_uaid_literal();
        for payload in [
            missing_total,
            false_terminal,
            false_continuation,
            unknown_page_field,
        ] {
            let response = merged_space_directory_manifests_response(
                vec![payload],
                0,
                Some(1),
                false,
                "bounded",
                None,
                routing::SpaceDirectoryManifestStatus::All,
                &expected_uaid,
                "proxy",
                test_budget(),
            )
            .expect_err("malformed page metadata must be rejected");
            assert_invalid_manifest_proxy_response(&response);
        }

        let undersized_continuation =
            test_manifest_payload(vec![test_manifest_row(7, "Active", 1)], 2, true, "bounded");
        let response = merged_space_directory_manifests_response(
            vec![undersized_continuation],
            0,
            Some(2),
            false,
            "bounded",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect_err("a continuing shard must provide the complete requested prefix");
        assert_invalid_manifest_proxy_response(&response);

        let impossible_filtered_continuation =
            test_manifest_payload(vec![test_manifest_row(7, "Active", 1)], 2, true, "bounded");
        let response = merged_space_directory_manifests_response(
            vec![impossible_filtered_continuation],
            0,
            Some(1),
            false,
            "bounded",
            Some(7),
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect_err("a dataspace-filtered page cannot advertise another row");
        assert_invalid_manifest_proxy_response(&response);
    }

    #[test]
    fn space_directory_manifest_merge_rejects_filter_and_identity_violations() {
        let expected_uaid = test_manifest_uaid_literal();
        let cases = [
            (
                test_manifest_payload(vec![test_manifest_row(8, "Active", 1)], 1, false, "bounded"),
                Some(7),
                routing::SpaceDirectoryManifestStatus::All,
            ),
            (
                test_manifest_payload(
                    vec![test_manifest_row(7, "Pending", 1)],
                    1,
                    false,
                    "bounded",
                ),
                None,
                routing::SpaceDirectoryManifestStatus::Active,
            ),
        ];
        for (payload, dataspace, status) in cases {
            let response = merged_space_directory_manifests_response(
                vec![payload],
                0,
                Some(1),
                false,
                "exact",
                dataspace,
                status,
                &expected_uaid,
                "proxy",
                test_budget(),
            )
            .expect_err("a row outside the requested filter must be rejected");
            assert_eq!(response.status(), StatusCode::CONFLICT);
        }

        let mut nested_mismatch = test_manifest_row(8, "Active", 1);
        nested_mismatch
            .as_object_mut()
            .expect("manifest row")
            .insert("dataspace_id".to_owned(), Value::from(7_u64));
        let response = merged_space_directory_manifests_response(
            vec![test_manifest_payload(
                vec![nested_mismatch],
                1,
                false,
                "bounded",
            )],
            0,
            Some(1),
            false,
            "exact",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect_err("the nested manifest identity must match its row");
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let mut nested_uaid_mismatch = test_manifest_row(7, "Active", 1);
        let object = nested_uaid_mismatch.as_object_mut().expect("manifest row");
        let mut manifest: iroha_data_model::nexus::AssetPermissionManifest =
            norito::json::from_value(object["manifest"].clone()).expect("decode test manifest");
        manifest.uaid = iroha_data_model::nexus::UniversalAccountId::from_hash(Hash::new(
            b"different-space-directory-merge-test-uaid",
        ));
        let manifest_hash: Hash = HashOf::new(&manifest).into();
        object.insert(
            "manifest".to_owned(),
            norito::json::to_value(&manifest).expect("encode mismatched test manifest"),
        );
        object.insert(
            "manifest_hash".to_owned(),
            Value::from(hex::encode(manifest_hash.as_ref())),
        );
        let response = merged_space_directory_manifests_response(
            vec![test_manifest_payload(
                vec![nested_uaid_mismatch],
                1,
                false,
                "bounded",
            )],
            0,
            Some(1),
            false,
            "exact",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect_err("the nested manifest UAID must match the response root");
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let mut noncanonical_nested_uaid = test_manifest_row(7, "Active", 1);
        noncanonical_nested_uaid
            .as_object_mut()
            .and_then(|row| row.get_mut("manifest"))
            .and_then(Value::as_object_mut)
            .expect("typed manifest object")
            .insert(
                "uaid".to_owned(),
                Value::from(expected_uaid.to_ascii_uppercase()),
            );
        let response = merged_space_directory_manifests_response(
            vec![test_manifest_payload(
                vec![noncanonical_nested_uaid],
                1,
                false,
                "bounded",
            )],
            0,
            Some(1),
            false,
            "exact",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect_err("the nested manifest UAID must use the canonical literal");
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn space_directory_manifest_merge_rejects_lifecycle_hash_and_prefix_violations() {
        let expected_uaid = test_manifest_uaid_literal();
        let mut wrong_status = test_manifest_row(7, "Pending", 1);
        wrong_status
            .as_object_mut()
            .expect("manifest row")
            .insert("status".to_owned(), Value::from("Active"));
        let mut wrong_hash = test_manifest_row(7, "Active", 1);
        wrong_hash
            .as_object_mut()
            .expect("manifest row")
            .insert("manifest_hash".to_owned(), Value::from("00".repeat(32)));
        let mut malformed_hash = test_manifest_row(7, "Active", 1);
        malformed_hash
            .as_object_mut()
            .expect("manifest row")
            .insert("manifest_hash".to_owned(), Value::from("not-a-hash"));
        let mut missing_manifest = test_manifest_row(7, "Active", 1);
        missing_manifest
            .as_object_mut()
            .expect("manifest row")
            .remove("manifest");
        let mut missing_accounts = test_manifest_row(7, "Active", 1);
        missing_accounts
            .as_object_mut()
            .expect("manifest row")
            .remove("accounts");
        let mut invalid_alias = test_manifest_row(7, "Active", 1);
        invalid_alias
            .as_object_mut()
            .expect("manifest row")
            .insert("dataspace_alias".to_owned(), Value::from(7_u64));
        let mut invalid_account = test_manifest_row(7, "Active", 1);
        invalid_account
            .as_object_mut()
            .expect("manifest row")
            .insert(
                "accounts".to_owned(),
                Value::Array(vec![Value::from("not-an-i105-account")]),
            );
        let mut unknown_row_field = test_manifest_row(7, "Active", 1);
        unknown_row_field
            .as_object_mut()
            .expect("manifest row")
            .insert("untrusted".to_owned(), Value::Bool(true));
        let mut unknown_lifecycle_field = test_manifest_row(7, "Active", 1);
        unknown_lifecycle_field
            .as_object_mut()
            .and_then(|row| row.get_mut("lifecycle"))
            .and_then(Value::as_object_mut)
            .expect("manifest lifecycle")
            .insert("untrusted".to_owned(), Value::Bool(true));
        let mut unknown_revocation_field = test_manifest_row(7, "Revoked", 1);
        unknown_revocation_field
            .as_object_mut()
            .and_then(|row| row.get_mut("lifecycle"))
            .and_then(Value::as_object_mut)
            .and_then(|lifecycle| lifecycle.get_mut("revocation"))
            .and_then(Value::as_object_mut)
            .expect("manifest revocation")
            .insert("untrusted".to_owned(), Value::Bool(true));
        for row in [
            wrong_status,
            wrong_hash,
            malformed_hash,
            missing_manifest,
            missing_accounts,
            invalid_alias,
            invalid_account,
            unknown_row_field,
            unknown_lifecycle_field,
            unknown_revocation_field,
        ] {
            let response = merged_space_directory_manifests_response(
                vec![test_manifest_payload(vec![row], 1, false, "bounded")],
                0,
                Some(1),
                false,
                "exact",
                None,
                routing::SpaceDirectoryManifestStatus::All,
                &expected_uaid,
                "proxy",
                test_budget(),
            )
            .expect_err("a malformed manifest row must be rejected");
            assert_invalid_manifest_proxy_response(&response);
        }

        let oversized = test_manifest_payload(
            vec![
                test_manifest_row(1, "Active", 1),
                test_manifest_row(2, "Active", 1),
            ],
            2,
            false,
            "bounded",
        );
        let response = merged_space_directory_manifests_response(
            vec![oversized],
            0,
            Some(1),
            false,
            "exact",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect_err("a shard page cannot exceed its requested prefix");
        assert_invalid_manifest_proxy_response(&response);

        let unordered = test_manifest_payload(
            vec![
                test_manifest_row(2, "Active", 1),
                test_manifest_row(1, "Active", 1),
            ],
            2,
            false,
            "bounded",
        );
        let response = merged_space_directory_manifests_response(
            vec![unordered],
            0,
            Some(2),
            false,
            "exact",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect_err("a shard page must be canonically ordered");
        assert_invalid_manifest_proxy_response(&response);
    }

    #[tokio::test]
    async fn space_directory_manifest_merge_applies_nonzero_offset_after_global_ordering() {
        let expected_uaid = test_manifest_uaid_literal();
        let response = merged_space_directory_manifests_response(
            vec![
                test_manifest_payload(
                    vec![
                        test_manifest_row(1, "Active", 1),
                        test_manifest_row(3, "Active", 1),
                    ],
                    2,
                    false,
                    "bounded",
                ),
                test_manifest_payload(
                    vec![
                        test_manifest_row(2, "Active", 1),
                        test_manifest_row(4, "Active", 1),
                    ],
                    2,
                    false,
                    "bounded",
                ),
            ],
            1,
            Some(2),
            false,
            "exact",
            None,
            routing::SpaceDirectoryManifestStatus::All,
            &expected_uaid,
            "proxy",
            test_budget(),
        )
        .expect("interleaved shard rows should merge globally");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("manifest merge response body");
        let payload: Value = norito::json::from_slice(&body).expect("manifest merge response JSON");
        let ids: Vec<_> = payload["manifests"]
            .as_array()
            .expect("manifests array")
            .iter()
            .map(|row| row["dataspace_id"].as_u64().expect("dataspace id"))
            .collect();
        assert_eq!(ids, vec![2, 3]);
        assert_eq!(payload["total"].as_u64(), Some(4));
        assert_eq!(payload["has_more"].as_bool(), Some(true));
        assert_eq!(payload["count_mode"].as_str(), Some("exact"));
    }
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
