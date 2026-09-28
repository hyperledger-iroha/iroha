//! Shared evaluation of JSON query predicates over data-model entities.
//!
//! Entity query modules resolve their typed alias fields first (for example an
//! identifier rendered in canonical and shorthand form) and fall back to a lazily
//! projected JSON value for every other dotted field path. The projection is
//! computed at most once per evaluated item.

use std::collections::BTreeSet;

use iroha_data_model::query::json::PredicateJson;
use iroha_model_base::domain::DomainId;
use norito::json::{JsonSerialize, Value};

use super::ordinary_predicate_json_value;

/// Resolve a non-empty dotted object path; empty paths and empty segments never match.
#[inline]
pub(crate) fn predicate_value_at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return None;
    }
    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        match current {
            Value::Object(map) => current = map.get(segment)?,
            _ => return None,
        }
    }
    Some(current)
}

/// Whether `value` is the JSON string `expected`.
#[inline]
pub(crate) fn predicate_value_equals_str(value: &Value, expected: &str) -> bool {
    matches!(value, Value::String(raw) if raw == expected)
}

/// Whether `values` contains the JSON string `expected`.
#[inline]
pub(crate) fn predicate_values_contain_str(values: &[Value], expected: &str) -> bool {
    values
        .iter()
        .any(|value| predicate_value_equals_str(value, expected))
}

/// Parse a domain filter value as a fully-qualified domain, or as a name in the universal
/// dataspace.
#[inline]
pub(crate) fn parse_domain_predicate_value(raw: &str) -> Option<DomainId> {
    DomainId::parse_fully_qualified(raw)
        .ok()
        .or_else(|| DomainId::try_new(raw, "universal").ok())
}

/// Narrow the best candidate-id set to its intersection with `candidates`; the first
/// constraint seeds the set.
#[inline]
pub(crate) fn intersect_candidate_ids<T: Ord>(
    best: &mut Option<BTreeSet<T>>,
    candidates: BTreeSet<T>,
) {
    if let Some(current) = best {
        current.retain(|id| candidates.contains(id));
    } else {
        *best = Some(candidates);
    }
}

/// Project `item` to JSON on first use and borrow the cached projection.
#[inline]
pub(crate) fn cached_predicate_json_value<'a, T: JsonSerialize + ?Sized>(
    cache: &'a mut Option<Value>,
    item: &T,
) -> Option<&'a Value> {
    if cache.is_none() {
        *cache = ordinary_predicate_json_value(item);
    }
    cache.as_ref()
}

/// Evaluate `predicate` against `item`.
///
/// `aliases` returns the typed string renderings of a field; a non-empty result
/// decides the condition without projecting the item. Other fields are read from
/// the item's JSON projection; an item without a projection skips those
/// conditions, while a missing path fails `equals`/`in` and `exists`.
pub(crate) fn predicate_matches_with_aliases<T: JsonSerialize + ?Sized>(
    predicate: &PredicateJson,
    item: &T,
    aliases: impl Fn(&T, &str) -> Vec<String>,
) -> bool {
    let mut json = None;
    for cond in &predicate.equals {
        let field_aliases = aliases(item, &cond.field);
        if !field_aliases.is_empty() {
            if !field_aliases
                .iter()
                .any(|alias| predicate_value_equals_str(&cond.value, alias))
            {
                return false;
            }
            continue;
        }
        let Some(value) = cached_predicate_json_value(&mut json, item) else {
            continue;
        };
        let Some(actual) = predicate_value_at_path(value, &cond.field) else {
            return false;
        };
        if actual != &cond.value {
            return false;
        }
    }
    for cond in &predicate.r#in {
        let field_aliases = aliases(item, &cond.field);
        if !field_aliases.is_empty() {
            if !field_aliases
                .iter()
                .any(|alias| predicate_values_contain_str(&cond.values, alias))
            {
                return false;
            }
            continue;
        }
        let Some(value) = cached_predicate_json_value(&mut json, item) else {
            continue;
        };
        let Some(actual) = predicate_value_at_path(value, &cond.field) else {
            return false;
        };
        if !cond.values.iter().any(|candidate| candidate == actual) {
            return false;
        }
    }
    for field in &predicate.exists {
        if !aliases(item, field).is_empty() {
            continue;
        }
        let Some(value) = cached_predicate_json_value(&mut json, item) else {
            continue;
        };
        let Some(actual) = predicate_value_at_path(value, field) else {
            return false;
        };
        if actual.is_null() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_paths_reject_empty_segments() {
        let value = norito::json!({ "a": { "b": 1 } });
        assert_eq!(
            predicate_value_at_path(&value, "a.b"),
            Some(&norito::json!(1))
        );
        assert_eq!(predicate_value_at_path(&value, ""), None);
        assert_eq!(predicate_value_at_path(&value, "a..b"), None);
        assert_eq!(predicate_value_at_path(&value, "a.b.c"), None);
    }

    #[test]
    fn candidate_ids_intersect_after_first_constraint() {
        let mut best = None;
        intersect_candidate_ids(&mut best, BTreeSet::from([1, 2, 3]));
        intersect_candidate_ids(&mut best, BTreeSet::from([2, 3, 4]));
        assert_eq!(best, Some(BTreeSet::from([2, 3])));
    }

    #[test]
    fn string_helpers_match_only_json_strings() {
        assert!(predicate_value_equals_str(&norito::json!("x"), "x"));
        assert!(!predicate_value_equals_str(&norito::json!(1), "1"));
        assert!(predicate_values_contain_str(
            &[norito::json!(1), norito::json!("y")],
            "y"
        ));
    }
}
