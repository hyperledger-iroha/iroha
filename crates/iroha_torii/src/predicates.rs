//! Predicate builders for server-side transaction queries.
//!
//! This module is part of the app API and converts its JSON filter DSL into a
//! `CompoundPredicate<CommittedTransaction>`.
//!
//! Typed atom mapping
//! ==================
//! When the data model exposes predicate atoms for `CommittedTransaction`, the
//! following atom accessors and operators are expected (names indicative):
//!
//! - `CommittedTransaction::authority() -> Atom<AccountId>`
//!   - Comparators: `.eq(AccountId)`
//! - `CommittedTransaction::timestamp_ms() -> Atom<u64>`
//!   - Comparators: `.gte(u64)`, `.lte(u64)`, `.gt(u64)`, `.lt(u64)`, `.eq(u64)`
//! - `CommittedTransaction::entrypoint_hash() -> Atom<HashOf<TransactionEntrypoint>>`
//!   - Comparators: `.eq(Hash)`, `.in(&[Hash])`
//! - `CommittedTransaction::result_ok() -> Atom<bool>`
//!   - Comparators: `.eq(bool)`
//!
//! Composition is expected via `CompoundPredicate::build(|p| { ... })` returning
//! a chained predicate:
//!
//! ```ignore
//! CP::build(|mut p| {
//!   p = p.and(CommittedTransaction::authority().eq(auth));
//!   p = p.and(CommittedTransaction::timestamp_ms().gte(ts_ge));
//!   p = p.and(CommittedTransaction::timestamp_ms().lte(ts_le));
//!   p = p.and(CommittedTransaction::entrypoint_hash().eq(h));
//!   p = p.and(CommittedTransaction::result_ok().eq(true));
//!   p
//! })
//! ```
//!
//! Mapping table (JSON DSL → predicate atom)
//! - `Eq("authority", AccountId)` → `authority().eq(AccountId)`
//! - `Gte/Lte("timestamp_ms", u64)` → `timestamp_ms().gte(u64)/lte(u64)`
//! - `Eq/In("entrypoint_hash", Hash)` → `entrypoint_hash().eq(hash)/in([..])`
//! - `Eq("result_ok", bool)` → `result_ok().eq(bool)`
//! - `And/Or/Not` → combine via `.and()` / `.or()` / `.not()` helpers as provided by the DSL
//!
//! Implementation notes:
//! - This module now builds a typed predicate tree (`CommittedTxPredicate`) and
//!   embeds it into the `CompoundPredicate`. The executor evaluates it server-side
//!   via `EvaluatePredicate`; endpoint-only fields such as `asset_id` remain
//!   under the authoritative local transaction filter.
use crate::filter::{FilterExpr, validate_filter};
use iroha_data_model::{
    name::Name,
    query::{
        CommittedTransaction, CommittedTxFilters,
        dsl::{CommittedTxPredicate as TP, CompoundPredicate as CP},
    },
};
use iroha_primitives::json::Json;
use norito::json::Value;
/// Build a server-side predicate for `CommittedTransaction` from the JSON DSL.
pub fn build_tx_predicate(expr: &FilterExpr) -> CP<CommittedTransaction> {
    if validate_filter(expr).is_err() {
        return CP::from_committed_tx_predicate(TP::Const(false));
    }
    // Map a JSON DSL filter into a typed predicate tree for committed transactions
    fn parse_acc(s: &str) -> Option<iroha_data_model::account::AccountId> {
        iroha_data_model::account::AccountId::parse_encoded(s)
            .ok()
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .filter(|account| account.to_string() == s)
    }
    fn parse_hash(
        s: &str,
    ) -> Option<iroha_crypto::HashOf<iroha_data_model::transaction::signed::TransactionEntrypoint>>
    {
        let hash = s
            .parse::<
                iroha_crypto::HashOf<
                    iroha_data_model::transaction::signed::TransactionEntrypoint,
                >,
            >()
            .ok()?;
        (hash.to_string() == s).then_some(hash)
    }
    fn map(expr: &FilterExpr) -> TP {
        use crate::filter::FieldPath;
        use FilterExpr as F;
        fn metadata_key(field: &str) -> Option<Name> {
            field.strip_prefix("metadata.").and_then(|rest| {
                rest.parse::<Name>()
                    .ok()
                    .filter(|name| name.to_string() == rest)
            })
        }
        fn json_from_value(value: &Value) -> Option<Json> {
            Json::from_norito_value_ref(value).ok()
        }
        fn json_vec_from_values(values: &[Value]) -> Option<Vec<Json>> {
            let mut out = Vec::with_capacity(values.len());
            for v in values {
                out.push(json_from_value(v)?);
            }
            Some(out)
        }
        match expr {
            F::And(list) => TP::And(list.iter().map(map).collect()),
            F::Or(list) => TP::Or(list.iter().map(map).collect()),
            F::Not(inner) => TP::Not(Box::new(map(inner))),
            F::Eq(FieldPath(f), v) => {
                if let Some(name) = metadata_key(f) {
                    json_from_value(v)
                        .map(|json| TP::MetadataEq {
                            key: name,
                            value: json,
                        })
                        .unwrap_or(TP::Const(false))
                } else if f == "authority" {
                    v.as_str()
                        .and_then(parse_acc)
                        .map(TP::AuthorityEq)
                        .unwrap_or(TP::Const(false))
                } else if f == "entrypoint_hash" {
                    v.as_str()
                        .and_then(parse_hash)
                        .map(TP::EntryEq)
                        .unwrap_or(TP::Const(false))
                } else if f == "result_ok" {
                    v.as_bool().map(TP::ResultEq).unwrap_or(TP::Const(false))
                } else if f == "timestamp_ms" {
                    v.as_u64().map(TP::TsEq).unwrap_or(TP::Const(false))
                } else {
                    TP::Const(false)
                }
            }
            F::Ne(FieldPath(f), v) => {
                if let Some(name) = metadata_key(f) {
                    json_from_value(v)
                        .map(|json| TP::MetadataNe {
                            key: name,
                            value: json,
                        })
                        .unwrap_or(TP::Const(false))
                } else if f == "authority" {
                    v.as_str()
                        .and_then(parse_acc)
                        .map(TP::AuthorityNe)
                        .unwrap_or(TP::Const(false))
                } else if f == "entrypoint_hash" {
                    v.as_str()
                        .and_then(parse_hash)
                        .map(TP::EntryNe)
                        .unwrap_or(TP::Const(false))
                } else if f == "result_ok" {
                    v.as_bool().map(TP::ResultNe).unwrap_or(TP::Const(false))
                } else if f == "timestamp_ms" {
                    v.as_u64()
                        .map(|n| TP::Not(Box::new(TP::TsEq(n))))
                        .unwrap_or(TP::Const(false))
                } else {
                    TP::Const(false)
                }
            }
            F::In(FieldPath(f), vals) => {
                if let Some(name) = metadata_key(f) {
                    match json_vec_from_values(vals) {
                        Some(set) if !set.is_empty() => TP::MetadataIn {
                            key: name,
                            values: set,
                        },
                        Some(_) => TP::Const(false),
                        None => TP::Const(false),
                    }
                } else if f == "authority" {
                    let set: Option<Vec<_>> = vals
                        .iter()
                        .map(|v| v.as_str().and_then(parse_acc))
                        .collect();
                    match set {
                        Some(set) if !set.is_empty() => TP::AuthorityIn(set),
                        _ => TP::Const(false),
                    }
                } else if f == "entrypoint_hash" {
                    let set: Option<Vec<_>> = vals
                        .iter()
                        .map(|v| v.as_str().and_then(parse_hash))
                        .collect();
                    match set {
                        Some(set) if !set.is_empty() => TP::EntryIn(set),
                        _ => TP::Const(false),
                    }
                } else if f == "result_ok" {
                    let set: Option<Vec<_>> = vals.iter().map(|v| v.as_bool()).collect();
                    match set {
                        Some(set) if !set.is_empty() => TP::ResultIn(set),
                        _ => TP::Const(false),
                    }
                } else if f == "timestamp_ms" {
                    let set: Option<Vec<_>> =
                        vals.iter().map(norito::json::Value::as_u64).collect();
                    match set {
                        Some(set) if !set.is_empty() => TP::TsIn(set),
                        _ => TP::Const(false),
                    }
                } else {
                    TP::Const(false)
                }
            }
            F::Nin(FieldPath(f), vals) => {
                if let Some(name) = metadata_key(f) {
                    match json_vec_from_values(vals) {
                        Some(set) if !set.is_empty() => TP::MetadataNin {
                            key: name,
                            values: set,
                        },
                        Some(_) => TP::Const(false),
                        None => TP::Const(false),
                    }
                } else if f == "authority" {
                    let set: Option<Vec<_>> = vals
                        .iter()
                        .map(|v| v.as_str().and_then(parse_acc))
                        .collect();
                    match set {
                        Some(set) if !set.is_empty() => TP::AuthorityNin(set),
                        _ => TP::Const(false),
                    }
                } else if f == "entrypoint_hash" {
                    let set: Option<Vec<_>> = vals
                        .iter()
                        .map(|v| v.as_str().and_then(parse_hash))
                        .collect();
                    match set {
                        Some(set) if !set.is_empty() => TP::EntryNin(set),
                        _ => TP::Const(false),
                    }
                } else if f == "result_ok" {
                    let set: Option<Vec<_>> = vals.iter().map(|v| v.as_bool()).collect();
                    match set {
                        Some(set) if !set.is_empty() => TP::ResultNin(set),
                        _ => TP::Const(false),
                    }
                } else if f == "timestamp_ms" {
                    let set: Option<Vec<_>> =
                        vals.iter().map(norito::json::Value::as_u64).collect();
                    match set {
                        Some(set) if !set.is_empty() => TP::TsNin(set),
                        _ => TP::Const(false),
                    }
                } else {
                    TP::Const(false)
                }
            }
            F::Exists(FieldPath(f)) => {
                if let Some(name) = metadata_key(f) {
                    TP::MetadataExists {
                        key: name,
                        exists: true,
                    }
                } else if f == "authority" {
                    TP::AuthorityExists(true)
                } else if f == "entrypoint_hash" {
                    TP::EntryExists(true)
                } else if f == "result_ok" {
                    TP::ResultExists(true)
                } else if f == "timestamp_ms" {
                    TP::TsExists(true)
                } else {
                    TP::Const(false)
                }
            }
            F::IsNull(FieldPath(f)) => {
                if let Some(name) = metadata_key(f) {
                    TP::MetadataIsNull {
                        key: name,
                        is_null: true,
                    }
                } else if f == "authority" {
                    TP::AuthorityExists(false)
                } else if f == "entrypoint_hash" {
                    TP::EntryExists(false)
                } else if f == "result_ok" {
                    TP::ResultExists(false)
                } else if f == "timestamp_ms" {
                    TP::TsExists(false)
                } else {
                    TP::Const(false)
                }
            }
            F::Lt(FieldPath(f), v) if f == "timestamp_ms" => {
                v.as_u64().map(TP::TsLt).unwrap_or(TP::Const(false))
            }
            F::Lte(FieldPath(f), v) if f == "timestamp_ms" => {
                v.as_u64().map(TP::TsLte).unwrap_or(TP::Const(false))
            }
            F::Gt(FieldPath(f), v) if f == "timestamp_ms" => {
                v.as_u64().map(TP::TsGt).unwrap_or(TP::Const(false))
            }
            F::Gte(FieldPath(f), v) if f == "timestamp_ms" => {
                v.as_u64().map(TP::TsGte).unwrap_or(TP::Const(false))
            }
            // Safety default: reject invalid/unknown fields by returning a false leaf
            _ => TP::Const(false),
        }
    }
    let tree = map(expr);
    let tree = norito::json::to_value(&tree)
        .ok()
        .and_then(|value| norito::json::from_value::<TP>(value).ok())
        .unwrap_or(TP::Const(false));
    CP::from_committed_tx_predicate(tree)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{FieldPath, QueryEnvelope};
    fn serialized_tree(expr: &FilterExpr) -> TP {
        let predicate = build_tx_predicate(expr);
        let raw = norito::json::to_json(&predicate).expect("serialize compound predicate");
        assert_ne!(raw, "{}", "committed filters must never serialize as pass");
        norito::json::from_json(&raw).expect("decode typed predicate tree")
    }
    #[test]
    fn query_envelope_filter_reaches_lossless_typed_predicate_codec() {
        let envelope: QueryEnvelope = norito::json::from_value(norito::json!({
            "filter": {
                "op": "or",
                "args": [
                    {"op": "eq", "args": ["result_ok", true]},
                    {"op": "eq", "args": ["metadata.tier", "gold"]},
                    {"op": "is_null", "args": ["metadata.note"]}
                ]
            }
        }))
        .expect("query envelope");
        let expr = envelope.filter.expect("filter");
        validate_filter(&expr).expect("validated envelope filter");
        assert_eq!(
            serialized_tree(&expr),
            TP::Or(vec![
                TP::ResultEq(true),
                TP::MetadataEq {
                    key: "tier".parse().expect("metadata key"),
                    value: Json::new("gold"),
                },
                TP::MetadataIsNull {
                    key: "note".parse().expect("metadata key"),
                    is_null: true,
                },
            ])
        );
    }
    #[test]
    fn malformed_or_unsafe_envelope_filters_fail_closed_not_pass() {
        let invalid = vec![
            FilterExpr::And(Vec::new()),
            FilterExpr::Or(Vec::new()),
            FilterExpr::In(FieldPath("timestamp_ms".into()), Vec::new()),
            FilterExpr::Nin(FieldPath("result_ok".into()), Vec::new()),
            FilterExpr::In(
                FieldPath("timestamp_ms".into()),
                vec![Value::from(1_u64), Value::String("bad".into())],
            ),
            FilterExpr::Nin(
                FieldPath("result_ok".into()),
                vec![Value::Bool(true), Value::Bool(true)],
            ),
            FilterExpr::Eq(
                FieldPath("entrypoint_hash".into()),
                Value::String("AA".repeat(iroha_crypto::Hash::LENGTH)),
            ),
        ];
        for expr in invalid {
            assert_eq!(serialized_tree(&expr), TP::Const(false));
        }
    }
    #[test]
    fn overdeep_programmatic_envelope_filter_fails_closed() {
        let mut expr = FilterExpr::Eq(FieldPath("result_ok".into()), Value::Bool(true));
        for _ in 0..=crate::filter::FILTER_EXPR_MAX_DEPTH {
            expr = FilterExpr::Not(Box::new(expr));
        }
        assert_eq!(serialized_tree(&expr), TP::Const(false));
    }
}
