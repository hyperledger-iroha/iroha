//! Shared row/aggregate executor for app-facing `QueryEnvelope` routes.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use axum::{
    body::Body,
    http::header,
    response::{IntoResponse, Response},
};
use iroha_primitives::numeric::{Numeric, NumericSpec};
use norito::json::{Map, Value};

use crate::{
    Error, Result,
    filter::{
        AggregateFn, AggregateMetric, AggregateSpec, FieldPath, FilterExpr, Order, QueryEnvelope,
    },
};

/// Stable resource identifier for account inventory rows.
pub(crate) const RESOURCE_ACCOUNTS: &str = "accounts";
/// Stable resource identifier for account transaction rows.
pub(crate) const RESOURCE_ACCOUNT_TRANSACTIONS: &str = "account_transactions";
/// Stable resource identifier for account asset rows.
pub(crate) const RESOURCE_ACCOUNT_ASSETS: &str = "account_assets";
/// Stable resource identifier for repo agreement rows.
pub(crate) const RESOURCE_REPO_AGREEMENTS: &str = "repo_agreements";
/// Stable resource identifier for domain rows.
pub(crate) const RESOURCE_DOMAINS: &str = "domains";
/// Stable resource identifier for asset definition rows.
pub(crate) const RESOURCE_ASSET_DEFINITIONS: &str = "asset_definitions";
/// Stable resource identifier for offline allowance rows.
pub(crate) const RESOURCE_OFFLINE_ALLOWANCES: &str = "offline_allowances";
/// Stable resource identifier for offline revocation rows.
pub(crate) const RESOURCE_OFFLINE_REVOCATIONS: &str = "offline_revocations";
/// Stable resource identifier for offline transfer rows.
pub(crate) const RESOURCE_OFFLINE_TRANSFERS: &str = "offline_transfers";
/// Stable resource identifier for NFT rows.
pub(crate) const RESOURCE_NFTS: &str = "nfts";
/// Stable resource identifier for RWA rows.
pub(crate) const RESOURCE_RWAS: &str = "rwas";
/// Stable resource identifier for asset holder rows.
pub(crate) const RESOURCE_ASSET_HOLDERS: &str = "asset_holders";

/// Field value capability exposed to the generic DSL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QueryFieldType {
    /// String scalar.
    String,
    /// Numeric scalar.
    Number,
    /// Boolean scalar.
    Bool,
    /// JSON value that may be non-scalar.
    Json,
}

impl QueryFieldType {
    const fn is_scalar(self) -> bool {
        matches!(self, Self::String | Self::Number | Self::Bool)
    }
}

/// One flat field in a resource DSL namespace.
#[derive(Clone, Copy, Debug)]
pub(crate) struct QueryFieldSpec {
    /// Stable DSL field name.
    pub(crate) name: &'static str,
    /// Type/capability class.
    pub(crate) field_type: QueryFieldType,
}

/// A registered query resource.
#[derive(Debug)]
pub(crate) struct QueryResourceSpec {
    /// Stable resource id.
    pub(crate) id: &'static str,
    /// Registered flat fields.
    pub(crate) fields: &'static [QueryFieldSpec],
    /// Whether `metadata.<key>` is accepted as a dynamic scalar field.
    pub(crate) allow_metadata: bool,
    /// Deterministic default sort fields.
    pub(crate) default_sort: &'static [&'static str],
    /// Deterministic tie-break fields appended when absent from user sort.
    pub(crate) tie_breakers: &'static [&'static str],
}

/// Indexed snapshot metadata attached to row/aggregate envelopes.
#[derive(Clone, Debug)]
pub(crate) struct QuerySnapshot {
    /// Indexed height for the rows.
    pub(crate) indexed_height: u64,
    /// Indexed block hash, hex encoded when available.
    pub(crate) indexed_block_hash: Option<String>,
    /// Backend source label.
    pub(crate) query_source: &'static str,
}

impl QuerySnapshot {
    /// Construct a snapshot descriptor.
    pub(crate) const fn new(
        indexed_height: u64,
        indexed_block_hash: Option<String>,
        query_source: &'static str,
    ) -> Self {
        Self {
            indexed_height,
            indexed_block_hash,
            query_source,
        }
    }
}

const fn field(name: &'static str, field_type: QueryFieldType) -> QueryFieldSpec {
    QueryFieldSpec { name, field_type }
}

const ACCOUNT_FIELDS: &[QueryFieldSpec] = &[
    field("id", QueryFieldType::String),
    field("primary_alias", QueryFieldType::String),
    field("primary_alias_name", QueryFieldType::String),
    field("primary_alias_dataspace", QueryFieldType::String),
    field("primary_alias_domain", QueryFieldType::String),
    field("has_primary_alias", QueryFieldType::Bool),
];

const ACCOUNT_TRANSACTION_FIELDS: &[QueryFieldSpec] = &[
    field("authority", QueryFieldType::String),
    field("timestamp_ms", QueryFieldType::Number),
    field("entrypoint_kind", QueryFieldType::String),
    field("entrypoint_hash", QueryFieldType::String),
    field("result_ok", QueryFieldType::Bool),
    field("asset_id", QueryFieldType::Json),
];

const ACCOUNT_ASSET_FIELDS: &[QueryFieldSpec] = &[
    field("account_id", QueryFieldType::String),
    field("asset", QueryFieldType::String),
    field("asset_name", QueryFieldType::String),
    field("asset_alias", QueryFieldType::String),
    field("scope", QueryFieldType::String),
    field("quantity", QueryFieldType::Number),
    field("primary_alias", QueryFieldType::String),
    field("primary_alias_name", QueryFieldType::String),
    field("primary_alias_dataspace", QueryFieldType::String),
    field("primary_alias_domain", QueryFieldType::String),
    field("has_primary_alias", QueryFieldType::Bool),
];

const REPO_AGREEMENT_FIELDS: &[QueryFieldSpec] = &[
    field("id", QueryFieldType::String),
    field("initiator", QueryFieldType::String),
    field("counterparty", QueryFieldType::String),
    field("custodian", QueryFieldType::String),
    field("cash_leg.asset_definition_id", QueryFieldType::String),
    field("cash_leg.quantity", QueryFieldType::Number),
    field("collateral_leg.asset_definition_id", QueryFieldType::String),
    field("collateral_leg.quantity", QueryFieldType::Number),
    field("rate_bps", QueryFieldType::Number),
    field("maturity_timestamp_ms", QueryFieldType::Number),
    field("initiated_timestamp_ms", QueryFieldType::Number),
    field("last_margin_check_timestamp_ms", QueryFieldType::Number),
    field("governance.haircut_bps", QueryFieldType::Number),
    field("governance.margin_frequency_secs", QueryFieldType::Number),
];

const DOMAIN_FIELDS: &[QueryFieldSpec] = &[field("id", QueryFieldType::String)];

const ASSET_DEFINITION_FIELDS: &[QueryFieldSpec] = &[
    field("id", QueryFieldType::String),
    field("name", QueryFieldType::String),
    field("alias", QueryFieldType::String),
    field("alias_binding.alias", QueryFieldType::String),
    field("alias_binding.status", QueryFieldType::String),
    field("alias_binding.lease_expiry_ms", QueryFieldType::Number),
    field("alias_binding.grace_until_ms", QueryFieldType::Number),
    field("alias_binding.bound_at_ms", QueryFieldType::Number),
];

const OFFLINE_ALLOWANCE_FIELDS: &[QueryFieldSpec] = &[
    field("certificate_id_hex", QueryFieldType::String),
    field("controller_id", QueryFieldType::String),
    field("controller_display", QueryFieldType::String),
    field("asset_id", QueryFieldType::String),
    field("asset_definition_id", QueryFieldType::String),
    field("asset_definition_name", QueryFieldType::String),
    field("asset_definition_alias", QueryFieldType::String),
    field("registered_at_ms", QueryFieldType::Number),
    field("expires_at_ms", QueryFieldType::Number),
    field("policy_expires_at_ms", QueryFieldType::Number),
    field("refresh_at_ms", QueryFieldType::Number),
    field("verdict_id_hex", QueryFieldType::String),
    field("attestation_nonce_hex", QueryFieldType::String),
    field("remaining_amount", QueryFieldType::Number),
    field("deadline_kind", QueryFieldType::String),
    field("deadline_state", QueryFieldType::String),
    field("deadline_ms", QueryFieldType::Number),
    field("deadline_ms_remaining", QueryFieldType::Number),
];

const OFFLINE_REVOCATION_FIELDS: &[QueryFieldSpec] = &[
    field("verdict_id_hex", QueryFieldType::String),
    field("issuer_id", QueryFieldType::String),
    field("issuer_display", QueryFieldType::String),
    field("revoked_at_ms", QueryFieldType::Number),
    field("reason", QueryFieldType::String),
    field("note", QueryFieldType::String),
];

const OFFLINE_TRANSFER_FIELDS: &[QueryFieldSpec] = &[
    field("bundle_id_hex", QueryFieldType::String),
    field("controller_id", QueryFieldType::String),
    field("receiver_id", QueryFieldType::String),
    field("deposit_account_id", QueryFieldType::String),
    field("status", QueryFieldType::String),
    field("rejection_reason", QueryFieldType::String),
    field("asset_id", QueryFieldType::String),
    field("certificate_id_hex", QueryFieldType::String),
    field("verdict_id_hex", QueryFieldType::String),
    field("attestation_nonce_hex", QueryFieldType::String),
    field("platform_policy", QueryFieldType::String),
    field("receipt_count", QueryFieldType::Number),
    field("recorded_at_ms", QueryFieldType::Number),
    field("recorded_at_height", QueryFieldType::Number),
    field("archived_at_height", QueryFieldType::Number),
    field("certificate_expires_at_ms", QueryFieldType::Number),
    field("policy_expires_at_ms", QueryFieldType::Number),
    field("refresh_at_ms", QueryFieldType::Number),
];

const NFT_FIELDS: &[QueryFieldSpec] = &[field("id", QueryFieldType::String)];
const RWA_FIELDS: &[QueryFieldSpec] = &[field("id", QueryFieldType::String)];

const ASSET_HOLDER_FIELDS: &[QueryFieldSpec] = &[
    field("account_id", QueryFieldType::String),
    field("asset", QueryFieldType::String),
    field("asset_alias", QueryFieldType::String),
    field("scope", QueryFieldType::String),
    field("quantity", QueryFieldType::Number),
    field("primary_alias", QueryFieldType::String),
    field("primary_alias_name", QueryFieldType::String),
    field("primary_alias_dataspace", QueryFieldType::String),
    field("primary_alias_domain", QueryFieldType::String),
    field("has_primary_alias", QueryFieldType::Bool),
];

const ACCOUNTS_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_ACCOUNTS,
    fields: ACCOUNT_FIELDS,
    allow_metadata: false,
    default_sort: &["id"],
    tie_breakers: &["id"],
};
const ACCOUNT_TRANSACTIONS_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_ACCOUNT_TRANSACTIONS,
    fields: ACCOUNT_TRANSACTION_FIELDS,
    allow_metadata: true,
    default_sort: &["timestamp_ms", "entrypoint_hash"],
    tie_breakers: &["entrypoint_hash"],
};
const ACCOUNT_ASSETS_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_ACCOUNT_ASSETS,
    fields: ACCOUNT_ASSET_FIELDS,
    allow_metadata: false,
    default_sort: &["asset", "scope"],
    tie_breakers: &["account_id", "asset", "scope"],
};
const REPO_AGREEMENTS_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_REPO_AGREEMENTS,
    fields: REPO_AGREEMENT_FIELDS,
    allow_metadata: false,
    default_sort: &["id"],
    tie_breakers: &["id"],
};
const DOMAINS_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_DOMAINS,
    fields: DOMAIN_FIELDS,
    allow_metadata: false,
    default_sort: &["id"],
    tie_breakers: &["id"],
};
const ASSET_DEFINITIONS_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_ASSET_DEFINITIONS,
    fields: ASSET_DEFINITION_FIELDS,
    allow_metadata: true,
    default_sort: &["id"],
    tie_breakers: &["id"],
};
const OFFLINE_ALLOWANCES_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_OFFLINE_ALLOWANCES,
    fields: OFFLINE_ALLOWANCE_FIELDS,
    allow_metadata: false,
    default_sort: &["registered_at_ms", "certificate_id_hex"],
    tie_breakers: &["certificate_id_hex"],
};
const OFFLINE_REVOCATIONS_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_OFFLINE_REVOCATIONS,
    fields: OFFLINE_REVOCATION_FIELDS,
    allow_metadata: true,
    default_sort: &["revoked_at_ms", "verdict_id_hex"],
    tie_breakers: &["verdict_id_hex"],
};
const OFFLINE_TRANSFERS_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_OFFLINE_TRANSFERS,
    fields: OFFLINE_TRANSFER_FIELDS,
    allow_metadata: false,
    default_sort: &["recorded_at_ms", "bundle_id_hex"],
    tie_breakers: &["bundle_id_hex"],
};
const NFTS_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_NFTS,
    fields: NFT_FIELDS,
    allow_metadata: true,
    default_sort: &["id"],
    tie_breakers: &["id"],
};
const RWAS_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_RWAS,
    fields: RWA_FIELDS,
    allow_metadata: false,
    default_sort: &["id"],
    tie_breakers: &["id"],
};
const ASSET_HOLDERS_SPEC: QueryResourceSpec = QueryResourceSpec {
    id: RESOURCE_ASSET_HOLDERS,
    fields: ASSET_HOLDER_FIELDS,
    allow_metadata: false,
    default_sort: &["account_id", "scope"],
    tie_breakers: &["account_id", "scope"],
};

/// Return the registry descriptor for a stable resource id.
pub(crate) fn registered_resource(id: &str) -> Option<&'static QueryResourceSpec> {
    match id {
        RESOURCE_ACCOUNTS => Some(&ACCOUNTS_SPEC),
        RESOURCE_ACCOUNT_TRANSACTIONS => Some(&ACCOUNT_TRANSACTIONS_SPEC),
        RESOURCE_ACCOUNT_ASSETS => Some(&ACCOUNT_ASSETS_SPEC),
        RESOURCE_REPO_AGREEMENTS => Some(&REPO_AGREEMENTS_SPEC),
        RESOURCE_DOMAINS => Some(&DOMAINS_SPEC),
        RESOURCE_ASSET_DEFINITIONS => Some(&ASSET_DEFINITIONS_SPEC),
        RESOURCE_OFFLINE_ALLOWANCES => Some(&OFFLINE_ALLOWANCES_SPEC),
        RESOURCE_OFFLINE_REVOCATIONS => Some(&OFFLINE_REVOCATIONS_SPEC),
        RESOURCE_OFFLINE_TRANSFERS => Some(&OFFLINE_TRANSFERS_SPEC),
        RESOURCE_NFTS => Some(&NFTS_SPEC),
        RESOURCE_RWAS => Some(&RWAS_SPEC),
        RESOURCE_ASSET_HOLDERS => Some(&ASSET_HOLDERS_SPEC),
        _ => None,
    }
}

/// Stable resource ids that support aggregate mode.
pub(crate) const fn aggregate_supported_resources() -> &'static [&'static str] {
    &[
        RESOURCE_ACCOUNTS,
        RESOURCE_ACCOUNT_TRANSACTIONS,
        RESOURCE_ACCOUNT_ASSETS,
        RESOURCE_REPO_AGREEMENTS,
        RESOURCE_DOMAINS,
        RESOURCE_ASSET_DEFINITIONS,
        RESOURCE_OFFLINE_ALLOWANCES,
        RESOURCE_OFFLINE_REVOCATIONS,
        RESOURCE_OFFLINE_TRANSFERS,
        RESOURCE_NFTS,
        RESOURCE_RWAS,
        RESOURCE_ASSET_HOLDERS,
    ]
}

/// Stable resource ids that have DA projection export support.
pub(crate) const fn projection_export_supported_resources() -> &'static [&'static str] {
    &[
        RESOURCE_ACCOUNTS,
        RESOURCE_ACCOUNT_ASSETS,
        RESOURCE_ASSET_HOLDERS,
        RESOURCE_ASSET_DEFINITIONS,
        RESOURCE_DOMAINS,
    ]
}

/// Execute row or aggregate mode for one registered resource.
pub(crate) fn execute_query_envelope(
    resource: &QueryResourceSpec,
    envelope: QueryEnvelope,
    rows: Vec<Map>,
    limit_cap: u64,
    snapshot: QuerySnapshot,
) -> Result<Response> {
    if envelope.select.is_some() && envelope.aggregate.is_some() {
        return Err(validation_error(
            "select and aggregate are mutually exclusive",
        ));
    }
    validate_filter(resource, envelope.filter.as_ref())?;
    validate_sort(resource, &envelope.sort)?;
    validate_select(resource, envelope.select.as_ref())?;

    let filtered = rows
        .into_iter()
        .filter(|row| {
            envelope
                .filter
                .as_ref()
                .map_or(true, |expr| evaluate_filter(expr, row))
        })
        .collect::<Vec<_>>();

    if let Some(aggregate) = envelope.aggregate.as_ref() {
        return execute_aggregate_mode(
            resource,
            filtered,
            aggregate,
            &envelope.sort,
            envelope.pagination.limit,
            envelope.pagination.offset,
            limit_cap,
            snapshot,
        );
    }

    let select = envelope
        .select
        .as_ref()
        .ok_or_else(|| validation_error("select or aggregate is required for generic mode"))?;
    let sorted = sort_rows(filtered, resource, &envelope.sort);
    let total = sorted.len();
    let items = paginate(
        sorted,
        envelope.pagination.limit,
        envelope.pagination.offset,
        limit_cap,
    )
    .into_iter()
    .map(|row| project_row(&row, select))
    .collect::<Vec<_>>();
    build_common_response(items, total, snapshot)
}

fn execute_aggregate_mode(
    resource: &QueryResourceSpec,
    rows: Vec<Map>,
    aggregate: &AggregateSpec,
    sort: &[crate::filter::SortKey],
    limit: Option<u64>,
    offset: u64,
    limit_cap: u64,
    snapshot: QuerySnapshot,
) -> Result<Response> {
    validate_aggregate(resource, aggregate, sort)?;
    let mut rows = aggregate_rows(resource, rows, aggregate)?;
    if let Some(having) = aggregate.having.as_ref() {
        rows.retain(|row| evaluate_filter(having, row));
    }
    rows = sort_rows(rows, resource, sort);
    let total = rows.len();
    let items = paginate(rows, limit, offset, limit_cap);
    build_common_response(items, total, snapshot)
}

fn build_common_response(
    items: Vec<Map>,
    total: usize,
    snapshot: QuerySnapshot,
) -> Result<Response> {
    let mut top = Map::new();
    top.insert(
        "items".into(),
        Value::Array(items.into_iter().map(Value::Object).collect()),
    );
    top.insert("total".into(), Value::from(total as u64));
    top.insert(
        "indexed_height".into(),
        Value::from(snapshot.indexed_height),
    );
    top.insert(
        "indexed_block_hash".into(),
        snapshot.indexed_block_hash.map_or(Value::Null, Value::from),
    );
    top.insert("query_source".into(), Value::from(snapshot.query_source));
    let body = norito::json::to_json_pretty(&top).map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            err.to_string(),
        ))
    })?;
    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(response.into_response())
}

fn validation_error(message: impl Into<String>) -> Error {
    Error::AppQueryValidation {
        code: "unsupported_query_shape",
        message: message.into(),
    }
}

fn field_spec(resource: &QueryResourceSpec, name: &str) -> Option<QueryFieldSpec> {
    if resource.allow_metadata
        && name
            .strip_prefix("metadata.")
            .is_some_and(|suffix| !suffix.is_empty())
    {
        return Some(QueryFieldSpec {
            name: "metadata.*",
            field_type: QueryFieldType::Json,
        });
    }
    resource
        .fields
        .iter()
        .copied()
        .find(|field| field.name == name)
}

fn validate_field(resource: &QueryResourceSpec, field: &FieldPath) -> Result<QueryFieldSpec> {
    field_spec(resource, &field.0).ok_or_else(|| {
        validation_error(format!(
            "resource `{}` does not support field `{}`",
            resource.id, field.0
        ))
    })
}

fn validate_filter(resource: &QueryResourceSpec, filter: Option<&FilterExpr>) -> Result<()> {
    let Some(filter) = filter else {
        return Ok(());
    };
    fn validate_rec(resource: &QueryResourceSpec, expr: &FilterExpr, depth: usize) -> Result<()> {
        if depth > 10 {
            return Err(validation_error("filter depth exceeds 10"));
        }
        match expr {
            FilterExpr::And(list) | FilterExpr::Or(list) => {
                for nested in list {
                    validate_rec(resource, nested, depth + 1)?;
                }
                Ok(())
            }
            FilterExpr::Not(inner) => validate_rec(resource, inner, depth + 1),
            FilterExpr::Eq(field, value) | FilterExpr::Ne(field, value) => {
                validate_filter_literal(resource, field, value).map(|_| ())
            }
            FilterExpr::Lt(field, value)
            | FilterExpr::Lte(field, value)
            | FilterExpr::Gt(field, value)
            | FilterExpr::Gte(field, value) => {
                let spec = validate_filter_literal(resource, field, value)?;
                if spec.field_type != QueryFieldType::Number {
                    return Err(validation_error(format!(
                        "field `{}` does not support range comparisons",
                        field.0
                    )));
                }
                Ok(())
            }
            FilterExpr::In(field, values) | FilterExpr::Nin(field, values) => {
                let spec = validate_field(resource, field)?;
                for value in values {
                    validate_value_type(field, spec.field_type, value)?;
                }
                Ok(())
            }
            FilterExpr::Exists(field) | FilterExpr::IsNull(field) => {
                validate_field(resource, field).map(|_| ())
            }
        }
    }
    validate_rec(resource, filter, 0)
}

fn validate_filter_literal(
    resource: &QueryResourceSpec,
    field: &FieldPath,
    value: &Value,
) -> Result<QueryFieldSpec> {
    let spec = validate_field(resource, field)?;
    validate_value_type(field, spec.field_type, value)?;
    Ok(spec)
}

fn validate_value_type(field: &FieldPath, field_type: QueryFieldType, value: &Value) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    let valid = match field_type {
        QueryFieldType::String => value.is_string(),
        QueryFieldType::Number => numeric_from_value(value).is_some(),
        QueryFieldType::Bool => value.is_bool(),
        QueryFieldType::Json => true,
    };
    if valid {
        Ok(())
    } else {
        Err(validation_error(format!(
            "field `{}` expects {:?} values",
            field.0, field_type
        )))
    }
}

fn validate_sort(resource: &QueryResourceSpec, sort: &[crate::filter::SortKey]) -> Result<()> {
    for key in sort {
        validate_field(resource, &key.key)?;
    }
    Ok(())
}

fn validate_select(
    resource: &QueryResourceSpec,
    select: Option<&crate::filter::Selector>,
) -> Result<()> {
    let Some(select) = select else {
        return Ok(());
    };
    for field in &select.0 {
        validate_field(resource, field)?;
    }
    Ok(())
}

fn validate_aggregate(
    resource: &QueryResourceSpec,
    aggregate: &AggregateSpec,
    sort: &[crate::filter::SortKey],
) -> Result<()> {
    let mut output_fields = BTreeSet::new();
    for group in &aggregate.group_by {
        let spec = validate_field(resource, group)?;
        if !spec.field_type.is_scalar() && !group.0.starts_with("metadata.") {
            return Err(validation_error(format!(
                "group_by field `{}` is not scalar",
                group.0
            )));
        }
        output_fields.insert(group.0.clone());
    }
    if aggregate.metrics.is_empty() {
        return Err(validation_error("aggregate metrics must not be empty"));
    }
    for metric in &aggregate.metrics {
        validate_metric(resource, metric)?;
        if !is_valid_alias(&metric.alias) {
            return Err(validation_error(format!(
                "aggregate alias `{}` is invalid",
                metric.alias
            )));
        }
        if !output_fields.insert(metric.alias.clone()) {
            return Err(validation_error(format!(
                "aggregate output field `{}` is duplicated",
                metric.alias
            )));
        }
    }
    for key in sort {
        if !output_fields.contains(&key.key.0) {
            return Err(validation_error(format!(
                "aggregate sort key `{}` is not produced by group_by or metrics",
                key.key.0
            )));
        }
    }
    if let Some(having) = aggregate.having.as_ref() {
        validate_having(having, &output_fields)?;
    }
    Ok(())
}

fn validate_metric(resource: &QueryResourceSpec, metric: &AggregateMetric) -> Result<()> {
    match metric.r#fn {
        AggregateFn::Count => {
            if metric.field.is_some() {
                return Err(validation_error("count must not declare a field"));
            }
            Ok(())
        }
        AggregateFn::DistinctCount => {
            let field = metric
                .field
                .as_ref()
                .ok_or_else(|| validation_error("distinct_count requires a field"))?;
            let spec = validate_field(resource, field)?;
            if spec.field_type.is_scalar() || field.0.starts_with("metadata.") {
                Ok(())
            } else {
                Err(validation_error(format!(
                    "distinct_count field `{}` is not scalar",
                    field.0
                )))
            }
        }
        AggregateFn::Sum | AggregateFn::Min | AggregateFn::Max | AggregateFn::Avg => {
            let field = metric
                .field
                .as_ref()
                .ok_or_else(|| validation_error("numeric aggregate requires a field"))?;
            let spec = validate_field(resource, field)?;
            if spec.field_type == QueryFieldType::Number {
                Ok(())
            } else {
                Err(validation_error(format!(
                    "numeric aggregate field `{}` is not numeric",
                    field.0
                )))
            }
        }
    }
}

fn validate_having(expr: &FilterExpr, output_fields: &BTreeSet<String>) -> Result<()> {
    match expr {
        FilterExpr::And(list) | FilterExpr::Or(list) => {
            for nested in list {
                validate_having(nested, output_fields)?;
            }
            Ok(())
        }
        FilterExpr::Not(inner) => validate_having(inner, output_fields),
        FilterExpr::Eq(field, _)
        | FilterExpr::Ne(field, _)
        | FilterExpr::Lt(field, _)
        | FilterExpr::Lte(field, _)
        | FilterExpr::Gt(field, _)
        | FilterExpr::Gte(field, _)
        | FilterExpr::In(field, _)
        | FilterExpr::Nin(field, _)
        | FilterExpr::Exists(field)
        | FilterExpr::IsNull(field) => {
            if output_fields.contains(&field.0) {
                Ok(())
            } else {
                Err(validation_error(format!(
                    "having field `{}` is not produced by aggregate",
                    field.0
                )))
            }
        }
    }
}

fn is_valid_alias(alias: &str) -> bool {
    let mut chars = alias.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn row_field_value<'a>(row: &'a Map, field: &str) -> Option<&'a Value> {
    if let Some(value) = row.get(field) {
        return Some(value);
    }
    let mut parts = field.split('.');
    let first = parts.next()?;
    let mut current = row.get(first)?;
    for part in parts {
        let Value::Object(map) = current else {
            return None;
        };
        current = map.get(part)?;
    }
    Some(current)
}

fn evaluate_filter(expr: &FilterExpr, row: &Map) -> bool {
    match expr {
        FilterExpr::And(list) => list.iter().all(|nested| evaluate_filter(nested, row)),
        FilterExpr::Or(list) => list.iter().any(|nested| evaluate_filter(nested, row)),
        FilterExpr::Not(inner) => !evaluate_filter(inner, row),
        FilterExpr::Eq(field, expected) => row_field_value(row, &field.0)
            .is_some_and(|actual| values_equal(&field.0, actual, expected)),
        FilterExpr::Ne(field, expected) => row_field_value(row, &field.0)
            .is_none_or(|actual| !values_equal(&field.0, actual, expected)),
        FilterExpr::Lt(field, expected) => row_field_value(row, &field.0)
            .is_some_and(|actual| compare_values(actual, expected) == Ordering::Less),
        FilterExpr::Lte(field, expected) => row_field_value(row, &field.0).is_some_and(|actual| {
            matches!(
                compare_values(actual, expected),
                Ordering::Less | Ordering::Equal
            )
        }),
        FilterExpr::Gt(field, expected) => row_field_value(row, &field.0)
            .is_some_and(|actual| compare_values(actual, expected) == Ordering::Greater),
        FilterExpr::Gte(field, expected) => row_field_value(row, &field.0).is_some_and(|actual| {
            matches!(
                compare_values(actual, expected),
                Ordering::Greater | Ordering::Equal
            )
        }),
        FilterExpr::In(field, values) => row_field_value(row, &field.0).is_some_and(|actual| {
            values
                .iter()
                .any(|expected| values_equal(&field.0, actual, expected))
        }),
        FilterExpr::Nin(field, values) => row_field_value(row, &field.0).is_none_or(|actual| {
            values
                .iter()
                .all(|expected| !values_equal(&field.0, actual, expected))
        }),
        FilterExpr::Exists(field) => row_field_value(row, &field.0).is_some(),
        FilterExpr::IsNull(field) => row_field_value(row, &field.0).is_none_or(Value::is_null),
    }
}

fn values_equal(field: &str, left: &Value, right: &Value) -> bool {
    if let Value::Array(items) = left {
        return items.iter().any(|item| values_equal(field, item, right));
    }
    if let Value::Array(items) = right {
        return items.iter().any(|item| values_equal(field, left, item));
    }
    if let (Some(lhs), Some(rhs)) = (left.as_str(), right.as_str()) {
        return if field.ends_with("_hex") {
            lhs.eq_ignore_ascii_case(rhs)
        } else {
            lhs == rhs
        };
    }
    compare_values(left, right) == Ordering::Equal
}

fn compare_values(left: &Value, right: &Value) -> Ordering {
    if let (Some(lhs), Some(rhs)) = (numeric_from_value(left), numeric_from_value(right)) {
        return lhs.cmp(&rhs);
    }
    if let (Some(lhs), Some(rhs)) = (left.as_bool(), right.as_bool()) {
        return lhs.cmp(&rhs);
    }
    if let (Some(lhs), Some(rhs)) = (left.as_str(), right.as_str()) {
        return lhs.cmp(rhs);
    }
    value_rank(left)
        .cmp(&value_rank(right))
        .then_with(|| json_sort_string(left).cmp(&json_sort_string(right)))
}

fn value_rank(value: &Value) -> u8 {
    if value.is_null() {
        0
    } else if value.is_bool() {
        1
    } else if numeric_from_value(value).is_some() {
        2
    } else if value.is_string() {
        3
    } else {
        4
    }
}

fn numeric_from_value(value: &Value) -> Option<Numeric> {
    value
        .as_u64()
        .map(Numeric::from)
        .or_else(|| {
            let signed = value.as_i64()?;
            u64::try_from(signed).ok().map(Numeric::from)
        })
        .or_else(|| value.as_str()?.parse().ok())
}

fn json_sort_string(value: &Value) -> String {
    norito::json::to_json(value).unwrap_or_else(|_| "null".to_owned())
}

fn sort_rows(
    mut rows: Vec<Map>,
    resource: &QueryResourceSpec,
    sort: &[crate::filter::SortKey],
) -> Vec<Map> {
    let mut sort_fields = if sort.is_empty() {
        resource
            .default_sort
            .iter()
            .map(|field| ((*field).to_owned(), Order::Asc))
            .collect::<Vec<_>>()
    } else {
        sort.iter()
            .map(|key| (key.key.0.clone(), key.order))
            .collect::<Vec<_>>()
    };
    for tie_breaker in resource.tie_breakers {
        if !sort_fields.iter().any(|(field, _)| field == tie_breaker) {
            sort_fields.push(((*tie_breaker).to_owned(), Order::Asc));
        }
    }
    rows.sort_by(|left, right| compare_rows(left, right, &sort_fields));
    rows
}

fn compare_rows(left: &Map, right: &Map, sort: &[(String, Order)]) -> Ordering {
    for (field, order) in sort {
        let left_value = row_field_value(left, field).unwrap_or(&Value::Null);
        let right_value = row_field_value(right, field).unwrap_or(&Value::Null);
        let ordering = compare_values(left_value, right_value);
        let ordering = if matches!(order, Order::Asc) {
            ordering
        } else {
            ordering.reverse()
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

fn paginate(rows: Vec<Map>, limit: Option<u64>, offset: u64, cap: u64) -> Vec<Map> {
    let effective_limit = limit.unwrap_or(cap).min(cap);
    let skip = usize::try_from(offset).unwrap_or(usize::MAX);
    let take = usize::try_from(effective_limit).unwrap_or(usize::MAX);
    rows.into_iter().skip(skip).take(take).collect()
}

fn project_row(row: &Map, select: &crate::filter::Selector) -> Map {
    let mut projected = Map::new();
    for field in &select.0 {
        let value = row_field_value(row, &field.0)
            .cloned()
            .unwrap_or(Value::Null);
        projected.insert(field.0.clone(), value);
    }
    projected
}

#[derive(Debug)]
enum MetricState {
    Count(u64),
    DistinctCount(BTreeSet<String>),
    Sum(Option<Numeric>),
    Min(Option<Numeric>),
    Max(Option<Numeric>),
    Avg { sum: Option<Numeric>, count: u64 },
}

impl MetricState {
    fn new(metric: &AggregateMetric) -> Self {
        match metric.r#fn {
            AggregateFn::Count => Self::Count(0),
            AggregateFn::DistinctCount => Self::DistinctCount(BTreeSet::new()),
            AggregateFn::Sum => Self::Sum(None),
            AggregateFn::Min => Self::Min(None),
            AggregateFn::Max => Self::Max(None),
            AggregateFn::Avg => Self::Avg {
                sum: None,
                count: 0,
            },
        }
    }

    fn update(&mut self, row: &Map, metric: &AggregateMetric) -> Result<()> {
        match self {
            Self::Count(total) => {
                *total = total.saturating_add(1);
                Ok(())
            }
            Self::DistinctCount(values) => {
                let field = metric
                    .field
                    .as_ref()
                    .ok_or_else(|| validation_error("distinct_count requires a field"))?;
                if let Some(value) = row_field_value(row, &field.0) {
                    values.insert(json_sort_string(value));
                }
                Ok(())
            }
            Self::Sum(total) => {
                let value = metric_numeric_value(row, metric)?;
                if let Some(value) = value {
                    *total = Some(match total.take() {
                        Some(existing) => existing
                            .checked_add(value)
                            .ok_or_else(|| validation_error("aggregate sum overflowed"))?,
                        None => value,
                    });
                }
                Ok(())
            }
            Self::Min(current) => {
                let value = metric_numeric_value(row, metric)?;
                if let Some(value) = value
                    && current.as_ref().is_none_or(|existing| value < *existing)
                {
                    *current = Some(value);
                }
                Ok(())
            }
            Self::Max(current) => {
                let value = metric_numeric_value(row, metric)?;
                if let Some(value) = value
                    && current.as_ref().is_none_or(|existing| value > *existing)
                {
                    *current = Some(value);
                }
                Ok(())
            }
            Self::Avg { sum, count } => {
                let value = metric_numeric_value(row, metric)?;
                if let Some(value) = value {
                    *sum = Some(match sum.take() {
                        Some(existing) => existing
                            .checked_add(value)
                            .ok_or_else(|| validation_error("aggregate avg overflowed"))?,
                        None => value,
                    });
                    *count = count.saturating_add(1);
                }
                Ok(())
            }
        }
    }

    fn finalize(self) -> Result<Value> {
        match self {
            Self::Count(total) => Ok(Value::from(total)),
            Self::DistinctCount(values) => Ok(Value::from(values.len() as u64)),
            Self::Sum(value) | Self::Min(value) | Self::Max(value) => Ok(value
                .map(|value| Value::from(value.to_string()))
                .unwrap_or(Value::Null)),
            Self::Avg { sum, count } => {
                let Some(sum) = sum else {
                    return Ok(Value::Null);
                };
                if count == 0 {
                    return Ok(Value::Null);
                }
                let divisor = Numeric::new(count, 0);
                let scale = sum.scale().max(6);
                let avg = sum
                    .checked_div(divisor, NumericSpec::fractional(scale))
                    .ok_or_else(|| validation_error("aggregate avg overflowed"))?;
                Ok(Value::from(avg.to_string()))
            }
        }
    }
}

fn metric_numeric_value(row: &Map, metric: &AggregateMetric) -> Result<Option<Numeric>> {
    let field = metric
        .field
        .as_ref()
        .ok_or_else(|| validation_error("numeric aggregate requires a field"))?;
    Ok(row_field_value(row, &field.0).and_then(numeric_from_value))
}

struct GroupState {
    group_values: Vec<(String, Value)>,
    metrics: Vec<MetricState>,
}

fn aggregate_rows(
    resource: &QueryResourceSpec,
    rows: Vec<Map>,
    aggregate: &AggregateSpec,
) -> Result<Vec<Map>> {
    let mut groups: BTreeMap<Vec<String>, GroupState> = BTreeMap::new();
    for row in rows {
        let group_values = aggregate
            .group_by
            .iter()
            .map(|field| {
                let value = row_field_value(&row, &field.0)
                    .cloned()
                    .unwrap_or(Value::Null);
                (field.0.clone(), value)
            })
            .collect::<Vec<_>>();
        let group_key = group_values
            .iter()
            .map(|(_, value)| json_sort_string(value))
            .collect::<Vec<_>>();
        let entry = groups.entry(group_key).or_insert_with(|| GroupState {
            group_values: group_values.clone(),
            metrics: aggregate.metrics.iter().map(MetricState::new).collect(),
        });
        for (state, metric) in entry.metrics.iter_mut().zip(&aggregate.metrics) {
            state.update(&row, metric)?;
        }
    }
    let mut out = Vec::with_capacity(groups.len());
    for state in groups.into_values() {
        let mut row = Map::new();
        for (field, value) in state.group_values {
            row.insert(field, value);
        }
        for (metric, state) in aggregate.metrics.iter().zip(state.metrics) {
            row.insert(metric.alias.clone(), state.finalize()?);
        }
        for field in resource.tie_breakers {
            row.entry((*field).to_owned()).or_insert(Value::Null);
        }
        out.push(row);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{AggregateMetric, Pagination, Selector, SortKey};
    use http_body_util::BodyExt as _;

    fn row(fields: &[(&str, Value)]) -> Map {
        let mut row = Map::new();
        for (key, value) in fields {
            row.insert((*key).to_owned(), value.clone());
        }
        row
    }

    async fn response_json(response: Response) -> Value {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        norito::json::from_slice(&bytes).expect("json")
    }

    #[tokio::test]
    async fn select_projects_only_requested_fields() {
        let envelope = QueryEnvelope {
            select: Some(Selector(vec![FieldPath("id".into())])),
            pagination: Pagination {
                limit: Some(10),
                offset: 0,
            },
            ..Default::default()
        };
        let response = execute_query_envelope(
            &ACCOUNTS_SPEC,
            envelope,
            vec![row(&[
                ("id", Value::from("alice")),
                ("has_primary_alias", Value::from(true)),
            ])],
            100,
            QuerySnapshot::new(7, Some("abcd".into()), "live"),
        )
        .expect("query response");
        let payload = response_json(response).await;
        assert_eq!(payload["items"][0]["id"].as_str(), Some("alice"));
        assert!(payload["items"][0]["has_primary_alias"].is_null());
        assert_eq!(payload["indexed_height"].as_u64(), Some(7));
        assert_eq!(payload["query_source"].as_str(), Some("live"));
    }

    #[tokio::test]
    async fn aggregate_distinct_count_is_exact_without_input_sorting() {
        let envelope = QueryEnvelope {
            aggregate: Some(AggregateSpec {
                group_by: vec![FieldPath("primary_alias_domain".into())],
                metrics: vec![AggregateMetric {
                    alias: "users".into(),
                    r#fn: AggregateFn::DistinctCount,
                    field: Some(FieldPath("id".into())),
                }],
                having: None,
            }),
            pagination: Pagination {
                limit: Some(10),
                offset: 0,
            },
            ..Default::default()
        };
        let response = execute_query_envelope(
            &ACCOUNTS_SPEC,
            envelope,
            vec![
                row(&[
                    ("id", Value::from("a")),
                    ("primary_alias_domain", Value::from("hbl.paynet")),
                ]),
                row(&[
                    ("id", Value::from("b")),
                    ("primary_alias_domain", Value::from("hbl.paynet")),
                ]),
                row(&[
                    ("id", Value::from("a")),
                    ("primary_alias_domain", Value::from("hbl.paynet")),
                ]),
            ],
            100,
            QuerySnapshot::new(7, None, "live"),
        )
        .expect("aggregate response");
        let payload = response_json(response).await;
        assert_eq!(payload["items"][0]["users"].as_u64(), Some(2));
    }

    #[tokio::test]
    async fn having_runs_before_pagination() {
        let envelope = QueryEnvelope {
            aggregate: Some(AggregateSpec {
                group_by: vec![FieldPath("primary_alias_domain".into())],
                metrics: vec![AggregateMetric {
                    alias: "total".into(),
                    r#fn: AggregateFn::Count,
                    field: None,
                }],
                having: Some(FilterExpr::Gt(
                    FieldPath("total".into()),
                    Value::from(1_u64),
                )),
            }),
            sort: vec![SortKey {
                key: FieldPath("primary_alias_domain".into()),
                order: Order::Asc,
            }],
            pagination: Pagination {
                limit: Some(1),
                offset: 0,
            },
            ..Default::default()
        };
        let response = execute_query_envelope(
            &ACCOUNTS_SPEC,
            envelope,
            vec![
                row(&[
                    ("id", Value::from("a")),
                    ("primary_alias_domain", Value::from("hbl.paynet")),
                ]),
                row(&[
                    ("id", Value::from("b")),
                    ("primary_alias_domain", Value::from("hbl.paynet")),
                ]),
                row(&[
                    ("id", Value::from("c")),
                    ("primary_alias_domain", Value::from("ubl.paynet")),
                ]),
            ],
            100,
            QuerySnapshot::new(7, None, "live"),
        )
        .expect("aggregate response");
        let payload = response_json(response).await;
        assert_eq!(payload["total"].as_u64(), Some(1));
        assert_eq!(
            payload["items"][0]["primary_alias_domain"].as_str(),
            Some("hbl.paynet")
        );
    }

    #[test]
    fn select_and_aggregate_are_mutually_exclusive() {
        let envelope = QueryEnvelope {
            select: Some(Selector(vec![FieldPath("id".into())])),
            aggregate: Some(AggregateSpec {
                metrics: vec![AggregateMetric {
                    alias: "total".into(),
                    r#fn: AggregateFn::Count,
                    field: None,
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(
            execute_query_envelope(
                &ACCOUNTS_SPEC,
                envelope,
                Vec::new(),
                100,
                QuerySnapshot::new(0, None, "live"),
            )
            .is_err()
        );
    }
}
