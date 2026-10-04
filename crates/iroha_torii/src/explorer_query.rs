//! Shared list-query ingress over Explorer's bounded, visibility-scoped scanners.
//!
//! Native scan positions remain internal. The public cursor binds the collection,
//! complete filter and current authorization scope; history positions also retain
//! their committed snapshot. Filtering an entire bounded candidate page may yield
//! an empty page with a continuation, which clients must follow.

use super::*;
use crate::collections::specs::{FieldSpec, FieldType};
use crate::collections::{CollectionError, CollectionSpec, Limits, RowPage, prepare};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use iroha_torii_shared::list_query::{CURSOR_MAX_BYTES, FilterExpr, ListQuery, Page};
use norito::json::{Map, Value};

const fn field(name: &'static str, ty: FieldType) -> FieldSpec {
    FieldSpec {
        name,
        ty,
        sortable: false,
        account: false,
        list: false,
    }
}
use FieldType::{Bool, Json, Number, String as Text};

macro_rules! spec {
    ($name:ident, $id:literal, $tag:literal, [$($key:literal),+], $metadata:literal, [$($field:literal => $ty:ident),* $(,)?]) => {
        static $name: CollectionSpec = CollectionSpec {
            id: $id, tag: $tag, fields: &[$(field($field, $ty)),*], metadata: $metadata,
            default_sort: &[], identity: &[$($key),+], positioned: 0, ordered: false,
        };
    };
}
spec!(ACCOUNTS, "explorer_accounts", 40, ["id"], true, [
    "id" => Text, "network_prefix" => Number, "owned_domains" => Number,
    "owned_assets" => Number, "owned_nfts" => Number,
]);
spec!(DOMAINS, "explorer_domains", 41, ["id"], true, [
    "id" => Text, "logo" => Text, "owned_by" => Text, "accounts" => Number,
    "assets" => Number, "nfts" => Number,
]);
spec!(DEFINITIONS, "explorer_asset_definitions", 42, ["id"], true, [
    "id" => Text, "owning_domain" => Text, "mintable" => Text, "logo" => Text,
    "owned_by" => Text, "assets" => Number, "total_quantity" => Number,
    "locked_quantity" => Number, "circulating_quantity" => Number,
]);
spec!(ASSETS, "explorer_assets", 43, ["id"], false, [
    "id" => Text, "definition_id" => Text, "account_id" => Text, "value" => Number,
]);
spec!(NFTS, "explorer_nfts", 44, ["id"], true, ["id" => Text, "owned_by" => Text]);
spec!(RWAS, "explorer_rwas", 45, ["id"], true, [
    "id" => Text, "owned_by" => Text, "quantity" => Number, "held_quantity" => Number,
    "primary_reference" => Text, "status" => Text, "is_frozen" => Bool, "parents" => Json,
]);
spec!(BLOCKS, "explorer_blocks", 46, ["height"], false, [
    "hash" => Text, "height" => Number, "created_at" => Text, "prev_block_hash" => Text,
    "transactions_hash" => Text, "transactions_rejected" => Number, "transactions_total" => Number,
]);
spec!(TRANSACTIONS, "explorer_transactions", 47, ["hash"], false, [
    "authority" => Text, "hash" => Text, "block" => Number, "created_at" => Text,
    "executable" => Text, "status" => Text,
]);
spec!(LATEST_TRANSACTIONS, "explorer_transactions_latest", 48, ["hash"], false, [
    "authority" => Text, "hash" => Text, "block" => Number, "created_at" => Text,
    "executable" => Text, "status" => Text,
]);
spec!(INSTRUCTIONS, "explorer_instructions", 49, ["transaction_hash", "index"], false, [
    "authority" => Text, "created_at" => Text, "kind" => Text, "box" => Json,
    "box.encoded" => Text, "box.framed_sha256" => Text, "box.json" => Json,
    "transaction_hash" => Text, "transaction_status" => Text, "block" => Number, "index" => Number,
]);
spec!(LATEST_INSTRUCTIONS, "explorer_instructions_latest", 50, ["transaction_hash", "index"], false, [
    "authority" => Text, "created_at" => Text, "kind" => Text, "box" => Json,
    "box.encoded" => Text, "box.framed_sha256" => Text, "box.json" => Json,
    "transaction_hash" => Text, "transaction_status" => Text, "block" => Number, "index" => Number,
]);

fn specification(path: &str) -> Option<&'static CollectionSpec> {
    Some(match path.strip_suffix("/query").unwrap_or(path) {
        "/v1/explorer/accounts" => &ACCOUNTS,
        "/v1/explorer/domains" => &DOMAINS,
        "/v1/explorer/asset-definitions" => &DEFINITIONS,
        "/v1/explorer/assets" => &ASSETS,
        "/v1/explorer/nfts" => &NFTS,
        "/v1/explorer/rwas" => &RWAS,
        "/v1/explorer/blocks" => &BLOCKS,
        "/v1/explorer/transactions" => &TRANSACTIONS,
        "/v1/explorer/transactions/latest" => &LATEST_TRANSACTIONS,
        "/v1/explorer/instructions" => &INSTRUCTIONS,
        "/v1/explorer/instructions/latest" => &LATEST_INSTRUCTIONS,
        _ => return None,
    })
}

fn invalid(control: &'static str, message: impl Into<String>) -> Error {
    CollectionError::from(iroha_torii_shared::list_query::ListQueryError::new(
        control, message,
    ))
    .into()
}

fn limits() -> Limits {
    Limits::from_page_limits(
        u64::from(explorer::EXPLORER_CURSOR_DEFAULT_LIMIT),
        u64::from(explorer::EXPLORER_CURSOR_MAX_LIMIT),
    )
}

/// A conjunctive equality can narrow the native scanner without changing the
/// full row predicate. Disjunctions and negations never become scan bounds.
fn conjunctive_equality<'a>(filter: Option<&'a FilterExpr>, name: &str) -> Option<&'a Value> {
    match filter? {
        FilterExpr::Eq(field, value) if field.as_str() == name => Some(value),
        FilterExpr::And(children) => children
            .iter()
            .find_map(|child| conjunctive_equality(Some(child), name)),
        _ => None,
    }
}

/// Synthetic selectors that use maintained indexes or committed instruction semantics.
/// They are equality-only conjuncts; ordinary DTO fields accept the complete filter AST.
fn synthetic_fields(spec: &CollectionSpec) -> &'static [&'static str] {
    match spec.id {
        "explorer_accounts" => &["domain", "with_asset"],
        "explorer_nfts" | "explorer_rwas" => &["domain"],
        "explorer_transactions" | "explorer_transactions_latest" => &["asset_id"],
        "explorer_instructions" | "explorer_instructions_latest" => &["account", "asset_id"],
        _ => &[],
    }
}

fn extract_selectors(
    expr: FilterExpr,
    names: &[&str],
    selectors: &mut BTreeMap<String, String>,
) -> Result<Option<FilterExpr>, Error> {
    if let FilterExpr::And(children) = expr {
        let children = children
            .into_iter()
            .map(|expr| extract_selectors(expr, names, selectors))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        return Ok((!children.is_empty()).then_some(FilterExpr::And(children)));
    }
    if let FilterExpr::Eq(ref field, Value::String(ref value)) = expr
        && names.contains(&field.as_str())
    {
        if selectors.insert(field.0.clone(), value.clone()).is_some() {
            return Err(invalid(
                "filter",
                format!("synthetic selector `{field}` may appear only once"),
            ));
        }
        return Ok(None);
    }
    fn contains(expr: &FilterExpr, names: &[&str]) -> bool {
        match expr {
            FilterExpr::And(children) | FilterExpr::Or(children) => {
                children.iter().any(|expr| contains(expr, names))
            }
            FilterExpr::Not(expr) => contains(expr, names),
            expr => expr
                .field()
                .is_some_and(|field| names.contains(&field.as_str())),
        }
    }
    if contains(&expr, names) {
        return Err(invalid(
            "filter",
            format!(
                "synthetic selectors {} require one string equality in the top-level conjunction",
                names.join(", ")
            ),
        ));
    }
    Ok(Some(expr))
}

struct ScanQuery {
    spec: &'static CollectionSpec,
    rows: ListQuery,
    selectors: BTreeMap<String, String>,
    native: explorer::ExplorerCursorQuery,
    digest: [u8; 32],
}

impl ScanQuery {
    fn new(
        spec: &'static CollectionSpec,
        query: ListQuery,
        visibility: [u8; 32],
    ) -> Result<Self, Error> {
        query.validate().map_err(CollectionError::from)?;
        for (unsupported, present) in [
            ("sort", !query.sort.is_empty()),
            ("aggregate", query.aggregate.is_some()),
            ("include_total", query.include_total),
        ] {
            if present {
                return Err(invalid(
                    unsupported,
                    "bounded Explorer feeds support filter, select, limit and cursor; full-scan controls are unavailable",
                ));
            }
        }
        let filter = query
            .filter
            .as_ref()
            .map(|filter| filter.to_string())
            .unwrap_or_default();
        let mut scope = Vec::new();
        scope.extend_from_slice(b"iroha-explorer-list-query-v1\0");
        scope.extend_from_slice(spec.id.as_bytes());
        scope.push(0);
        scope.extend_from_slice(&visibility);
        scope.extend_from_slice(filter.as_bytes());
        let digest = *iroha_crypto::Hash::new(scope).as_ref();
        let native_cursor = query
            .cursor
            .as_deref()
            .map(|token| Self::unwrap_cursor(spec.tag, &digest, token))
            .transpose()?;
        let mut rows = query;
        rows.cursor = None;
        let mut selectors = BTreeMap::new();
        rows.filter = rows
            .filter
            .take()
            .map(|expr| extract_selectors(expr, synthetic_fields(spec), &mut selectors))
            .transpose()?
            .flatten();
        let plan = prepare(spec, "", &rows, &limits())?;
        let native = explorer::ExplorerCursorQuery {
            cursor: native_cursor,
            limit: u32::try_from(plan.limit()).expect("bounded limit"),
        };
        Ok(Self {
            spec,
            rows,
            selectors,
            native,
            digest,
        })
    }

    fn unwrap_cursor(tag: u8, digest: &[u8; 32], token: &str) -> Result<String, Error> {
        let fail = || {
            invalid(
                "cursor",
                "cursor must come from this Explorer collection with the same filter and authorization scope",
            )
        };
        if token.len() > CURSOR_MAX_BYTES {
            return Err(fail());
        }
        let bytes = URL_SAFE_NO_PAD.decode(token).map_err(|_| fail())?;
        if bytes.len() <= 37
            || &bytes[..4] != b"IEL1"
            || bytes[4] != tag
            || &bytes[5..37] != digest
            || URL_SAFE_NO_PAD.encode(&bytes) != token
        {
            return Err(fail());
        }
        String::from_utf8(bytes[37..].to_vec()).map_err(|_| fail())
    }

    fn wrap_cursor(&self, native: &str) -> Result<String, Error> {
        let mut bytes = b"IEL1".to_vec();
        bytes.push(self.spec.tag);
        bytes.extend_from_slice(&self.digest);
        bytes.extend_from_slice(native.as_bytes());
        let token = URL_SAFE_NO_PAD.encode(bytes);
        if token.len() > CURSOR_MAX_BYTES {
            return Err(invalid(
                "cursor",
                "Explorer scan position exceeds the cursor bound",
            ));
        }
        Ok(token)
    }

    fn page(&self, scan: Value) -> Result<Page<Value>, Error> {
        let plan = prepare(self.spec, "", &self.rows, &limits())?;
        let items = scan
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("query", "Explorer scan did not produce rows"))?;
        let items = items
            .iter()
            .map(|item| {
                item.as_object()
                    .cloned()
                    .ok_or_else(|| invalid("query", "Explorer scan row must be an object"))
            })
            .collect::<Result<Vec<Map>, Error>>()?
            .into_iter()
            .filter(|row| plan.matches(row))
            .collect();
        let cursor = scan
            .get("pagination")
            .and_then(|meta| meta.get("next_cursor"));
        let next_cursor = match cursor {
            Some(Value::Null) => None,
            Some(Value::String(token)) if !token.is_empty() => Some(self.wrap_cursor(token)?),
            _ => {
                return Err(invalid(
                    "query",
                    "Explorer scan did not produce a continuation boundary",
                ));
            }
        };
        Ok(plan
            .project(RowPage {
                items,
                next_cursor,
                total: None,
            })?
            .into_page())
    }
}

pub(super) async fn get(
    State(app): State<SharedAppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<Response, Error> {
    let caller =
        torii_visibility_account_from_headers(&app, &headers, &method, &uri, &[], "v1/explorer")?;
    let query = list_query_from_query_string(uri.query())?;
    execute(&app, &headers, remote.ip(), &uri, caller, query).await
}

pub(super) async fn post(
    State(app): State<SharedAppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    body: axum::body::Bytes,
) -> Result<Response, Error> {
    let caller = torii_visibility_account_from_headers(
        &app,
        &headers,
        &method,
        &uri,
        body.as_ref(),
        "v1/explorer/query",
    )?;
    let plan = match torii_routed_read_request_decode_plan(&app) {
        Ok(plan) => plan,
        Err(response) => return Ok(response),
    };
    let value =
        match decode_torii_proxy_json_body::<Value>(plan, body.as_ref(), "Explorer list query") {
            Ok(value) => value,
            Err(response) => return Ok(response),
        };
    execute(
        &app,
        &headers,
        remote.ip(),
        &uri,
        caller,
        list_query_from_json(value)?,
    )
    .await
}

async fn execute(
    app: &SharedAppState,
    headers: &HeaderMap,
    remote: std::net::IpAddr,
    uri: &Uri,
    caller: ToriiAccountReadVisibility,
    mut query: ListQuery,
) -> Result<Response, Error> {
    let spec =
        specification(uri.path()).ok_or_else(|| invalid("query", "unknown Explorer collection"))?;
    let visibility = caller
        .into_dataspace_context(Arc::clone(app))
        .current_visibility();
    if let Some(filter) = query.filter.as_mut() {
        let fields: &[&str] = match spec.id {
            "explorer_accounts" => &["id"],
            "explorer_domains"
            | "explorer_asset_definitions"
            | "explorer_nfts"
            | "explorer_rwas" => &["owned_by"],
            "explorer_assets" => &["account_id"],
            "explorer_transactions" | "explorer_transactions_latest" => &["authority"],
            "explorer_instructions" | "explorer_instructions_latest" => &["authority", "account"],
            _ => &[],
        };
        for field in fields {
            routing::canonicalize_filter_account_literals(
                filter,
                field,
                Some(&app.state),
                &app.telemetry_handle(),
                "/v1/explorer?filter",
            )
            .map_err(|error| invalid("filter", error.to_string()))?;
        }
    }
    let query = ScanQuery::new(spec, query, visibility.visible_route_set_digest())?;
    if !limits::is_allowed_by_cidr(headers, Some(remote), &app.api_rate_limit_bypass_nets) {
        let cost = routing::app_query_limits().rate_limit_cost(u64::from(query.native.limit));
        check_access_enforced_with_cost(app, headers, Some(remote), spec.id, true, cost).await?;
    }
    let account = |name: &str| {
        query
            .selectors
            .get(name)
            .map(|value| parse_account_id_for_endpoint(app, value, "/v1/explorer?filter"))
            .transpose()
    };
    let domain = query
        .selectors
        .get("domain")
        .map(|value| parse_domain_id(value))
        .transpose()?;
    let asset = query
        .selectors
        .get("asset_id")
        .map(|value| parse_asset_id(value))
        .transpose()?;
    let block = conjunctive_equality(query.rows.filter.as_ref(), "block").and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    });
    let transaction_hash = conjunctive_equality(query.rows.filter.as_ref(), "transaction_hash")
        .and_then(Value::as_str)
        .map(|value| {
            value
                .parse()
                .map_err(|error| invalid("filter", format!("invalid transaction_hash: {error}")))
        })
        .transpose()?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    let state = app.state.clone();
    let telemetry = app.telemetry.clone();
    let native = query.native.clone();
    let response = match spec.id {
        "explorer_accounts" => {
            routing::handle_v1_explorer_accounts_admitted(
                state,
                visibility,
                native,
                domain,
                query
                    .selectors
                    .get("with_asset")
                    .map(|value| parse_asset_definition_id(app, value))
                    .transpose()?,
                admission,
            )
            .await?
        }
        "explorer_domains" => {
            routing::handle_v1_explorer_domains_admitted(state, visibility, native, None, admission)
                .await?
        }
        "explorer_asset_definitions" => {
            routing::handle_v1_explorer_asset_definitions_admitted(
                state, visibility, native, None, None, admission,
            )
            .await?
        }
        "explorer_assets" => {
            routing::handle_v1_explorer_assets_admitted(
                state, visibility, native, None, None, None, admission,
            )
            .await?
        }
        "explorer_nfts" => {
            routing::handle_v1_explorer_nfts_admitted(
                state, visibility, native, None, domain, admission,
            )
            .await?
        }
        "explorer_rwas" => {
            routing::handle_v1_explorer_rwas_admitted(
                state, visibility, native, None, domain, admission,
            )
            .await?
        }
        "explorer_blocks" => {
            routing::handle_v1_explorer_blocks_admitted(
                state, telemetry, visibility, native, admission,
            )
            .await?
        }
        "explorer_transactions" => {
            routing::handle_v1_explorer_transactions_admitted(
                state, telemetry, visibility, native, None, block, None, asset, admission,
            )
            .await?
        }
        "explorer_transactions_latest" => {
            routing::handle_v1_explorer_transactions_latest_admitted(
                state, telemetry, visibility, native, None, block, None, asset, admission,
            )
            .await?
        }
        "explorer_instructions" | "explorer_instructions_latest" => {
            let filters = routing::ExplorerInstructionQuery {
                account: account("account")?,
                authority: None,
                transaction_hash,
                status: None,
                block,
                kind: None,
                asset_id: asset,
            };
            if spec.id == "explorer_instructions" {
                routing::handle_v1_explorer_instructions_admitted(
                    state, telemetry, visibility, native, filters, admission,
                )
                .await?
            } else {
                routing::handle_v1_explorer_instructions_latest_admitted(
                    state, telemetry, visibility, native, filters, admission,
                )
                .await?
            }
        }
        _ => unreachable!("closed Explorer collection table"),
    };
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| invalid("query", error.to_string()))?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    routing::run_admitted_blocking(admission, "Explorer projection worker failed", move || {
        let scan = norito::json::from_slice(&bytes)
            .map_err(|error| invalid("query", error.to_string()))?;
        Ok(JsonBody(query.page(scan)?).into_response())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_torii_shared::list_query::{FieldPath, field};

    #[test]
    fn shared_filter_projection_preserves_counters_and_empty_continuations() {
        let query = ListQuery::new()
            .filter(field("owned_assets").gte(2))
            .select([FieldPath::from("id"), FieldPath::from("owned_assets")]);
        let query = ScanQuery::new(&ACCOUNTS, query, [1; 32]).expect("query");
        let page = query.page(norito::json!({"items":[{"id":"alice","owned_assets":1}],"pagination":{"next_cursor":"native"}})).expect("page");
        assert!(page.items.is_empty());
        let token = page
            .next_cursor
            .expect("continue after an empty filtered page");
        let next = ScanQuery::new(
            &ACCOUNTS,
            ListQuery {
                cursor: Some(token.clone()),
                select: None,
                ..query.rows.clone()
            },
            [1; 32],
        )
        .expect("projection may change");
        assert_eq!(next.native.cursor.as_deref(), Some("native"));
        assert!(
            ScanQuery::new(
                &ACCOUNTS,
                ListQuery {
                    cursor: Some(token),
                    ..ListQuery::new()
                },
                [1; 32]
            )
            .is_err()
        );
        let page = query.page(norito::json!({"items":[{"id":"bob","owned_assets":3,"network_prefix":753}],"pagination":{"next_cursor":null}})).expect("page");
        assert_eq!(
            page.items,
            vec![norito::json!({"id":"bob","owned_assets":3})]
        );
        assert!(page.next_cursor.is_none());
        assert!(page.total.is_none());
    }

    #[test]
    fn cursor_rejects_different_visibility_collection_and_legacy_position() {
        let query = ScanQuery::new(&ACCOUNTS, ListQuery::new(), [1; 32]).expect("query");
        let token = query.wrap_cursor("native").expect("cursor");
        for (spec, scope) in [(&ACCOUNTS, [2; 32]), (&DOMAINS, [1; 32])] {
            assert!(
                ScanQuery::new(
                    spec,
                    ListQuery {
                        cursor: Some(token.clone()),
                        ..ListQuery::new()
                    },
                    scope
                )
                .is_err()
            );
        }
        assert!(
            ScanQuery::new(
                &ACCOUNTS,
                ListQuery {
                    cursor: Some("native".into()),
                    ..ListQuery::new()
                },
                [1; 32]
            )
            .is_err()
        );
    }

    #[test]
    fn synthetic_filters_are_explicit_and_never_silently_reinterpreted() {
        let query = ListQuery::new().filter(
            field("domain")
                .eq("wonderland")
                .and(field("owned_assets").gt(1)),
        );
        let query = ScanQuery::new(&ACCOUNTS, query, [0; 32]).expect("conjunctive selector");
        assert_eq!(
            query.selectors.get("domain").map(String::as_str),
            Some("wonderland")
        );
        for filter in [
            field("domain").ne("wonderland"),
            field("domain").eq("a").or(field("id").eq("b")),
            field("domain").eq("a").and(field("domain").eq("b")),
        ] {
            assert!(ScanQuery::new(&ACCOUNTS, ListQuery::new().filter(filter), [0; 32]).is_err());
        }
        for query in [
            ListQuery {
                include_total: true,
                ..ListQuery::new()
            },
            ListQuery {
                limit: Some(101),
                ..ListQuery::new()
            },
            ListQuery::new().filter(field("unknown").eq(1)),
        ] {
            assert!(ScanQuery::new(&ACCOUNTS, query, [0; 32]).is_err());
        }
    }

    #[test]
    fn every_feed_has_one_get_and_post_schema() {
        for path in [
            "accounts",
            "domains",
            "asset-definitions",
            "assets",
            "nfts",
            "rwas",
            "blocks",
            "transactions",
            "transactions/latest",
            "instructions",
            "instructions/latest",
        ] {
            let path = format!("/v1/explorer/{path}");
            assert_eq!(
                specification(&path).expect("get").id,
                specification(&format!("{path}/query")).expect("post").id
            );
        }
    }

    #[test]
    fn only_conjunctive_equalities_narrow_history_scans() {
        let bound = field("block").eq(7).and(field("status").eq("committed"));
        assert_eq!(
            conjunctive_equality(Some(&bound), "block").and_then(Value::as_u64),
            Some(7)
        );
        let unbounded = field("block").eq(7).or(field("status").eq("committed"));
        assert!(conjunctive_equality(Some(&unbounded), "block").is_none());
    }
}
