//! Experimental predicate builders for server-side transaction queries.
//!
//! This module is compiled only when the `tx_predicates` feature is enabled.
//! It provides a single entrypoint `build_tx_predicate` that converts the
//! app-facing JSON filter DSL into a `CompoundPredicate<CommittedTransaction>`.
//!
//! README (expected atom API)
//! ==========================
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
//!   via `EvaluatePredicate` without falling back to local filtering when the
//!   `tx_predicates` feature is enabled.

#![allow(dead_code)]

use iroha_data_model::{
    name::Name,
    query::{
        CommittedTransaction, CommittedTxFilters,
        dsl::{CommittedTxPredicate as TP, CompoundPredicate as CP},
    },
};
use iroha_primitives::json::Json;
use norito::json::Value;

use crate::filter::FilterExpr;

/// Build a server-side predicate for `CommittedTransaction` from the JSON DSL.
pub fn build_tx_predicate(expr: &FilterExpr) -> CP<CommittedTransaction> {
    // Map a JSON DSL filter into a typed predicate tree for committed transactions
    fn parse_acc(s: &str) -> Option<iroha_data_model::account::AccountId> {
        s.parse().ok()
    }
    fn parse_hash(
        s: &str,
    ) -> Option<iroha_crypto::HashOf<iroha_data_model::transaction::signed::TransactionEntrypoint>>
    {
        s.parse().ok()
    }
    fn map(expr: &FilterExpr) -> TP {
        use FilterExpr as F;

        use crate::filter::FieldPath;

        fn metadata_key(field: &str) -> Option<Name> {
            field
                .strip_prefix("metadata.")
                .and_then(|rest| rest.parse::<Name>().ok())
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
                    let set: Vec<_> = vals
                        .iter()
                        .filter_map(|v| v.as_str().and_then(parse_acc))
                        .collect();
                    if set.is_empty() {
                        TP::Const(false)
                    } else {
                        TP::AuthorityIn(set)
                    }
                } else if f == "entrypoint_hash" {
                    let set: Vec<_> = vals
                        .iter()
                        .filter_map(|v| v.as_str().and_then(parse_hash))
                        .collect();
                    if set.is_empty() {
                        TP::Const(false)
                    } else {
                        TP::EntryIn(set)
                    }
                } else if f == "result_ok" {
                    let set: Vec<_> = vals.iter().filter_map(|v| v.as_bool()).collect();
                    if set.is_empty() {
                        TP::Const(false)
                    } else {
                        TP::ResultIn(set)
                    }
                } else if f == "timestamp_ms" {
                    let set: Vec<_> = vals
                        .iter()
                        .filter_map(norito::json::Value::as_u64)
                        .collect();
                    if set.is_empty() {
                        TP::Const(false)
                    } else {
                        TP::TsIn(set)
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
                        Some(_) => TP::Const(true),
                        None => TP::Const(false),
                    }
                } else if f == "authority" {
                    let set: Vec<_> = vals
                        .iter()
                        .filter_map(|v| v.as_str().and_then(parse_acc))
                        .collect();
                    if set.is_empty() {
                        TP::Const(true)
                    } else {
                        TP::AuthorityNin(set)
                    }
                } else if f == "entrypoint_hash" {
                    let set: Vec<_> = vals
                        .iter()
                        .filter_map(|v| v.as_str().and_then(parse_hash))
                        .collect();
                    if set.is_empty() {
                        TP::Const(true)
                    } else {
                        TP::EntryNin(set)
                    }
                } else if f == "result_ok" {
                    let set: Vec<_> = vals.iter().filter_map(|v| v.as_bool()).collect();
                    if set.is_empty() {
                        TP::Const(true)
                    } else {
                        TP::ResultNin(set)
                    }
                } else if f == "timestamp_ms" {
                    let set: Vec<_> = vals
                        .iter()
                        .filter_map(norito::json::Value::as_u64)
                        .collect();
                    if set.is_empty() {
                        TP::Const(true)
                    } else {
                        TP::TsNin(set)
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
    CP::from_committed_tx_predicate(tree)
}
