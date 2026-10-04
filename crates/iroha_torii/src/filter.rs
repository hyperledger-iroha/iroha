//! Filter, sort and aggregate types for app-facing collection endpoints and
//! webhook subscriptions.
//!
//! The language (AST, text grammar, JSON form and limits) is
//! [`iroha_torii_shared::list_query`]; this module re-exports it for Torii and
//! keeps the structural check webhook filters use.
pub use iroha_torii_shared::list_query::{
    AggregateFn, AggregateMetric, AggregateSpec, FieldPath, FilterExpr, ListQuery, Order, SortKey,
};
use norito::json::Value;

/// Maximum nesting accepted in a filter expression.
pub(crate) const FILTER_EXPR_MAX_DEPTH: usize = iroha_torii_shared::list_query::FILTER_MAX_DEPTH;
/// Maximum operator nodes accepted in a filter expression.
pub(crate) const FILTER_EXPR_MAX_NODES: usize = iroha_torii_shared::list_query::FILTER_MAX_NODES;
/// Maximum literals accepted by one membership operator.
pub(crate) const FILTER_EXPR_MAX_MEMBERSHIP_VALUES: usize =
    iroha_torii_shared::list_query::FILTER_MAX_MEMBERSHIP_VALUES;
/// Maximum membership literals accepted across one expression tree.
pub(crate) const FILTER_EXPR_MAX_TOTAL_MEMBERSHIP_VALUES: usize =
    iroha_torii_shared::list_query::FILTER_MAX_TOTAL_MEMBERSHIP_VALUES;

/// Canonical JSON form of a filter expression.
pub fn filter_expr_to_value(expr: &FilterExpr) -> Value {
    expr.to_json_value()
}
fn membership_values_are_unique(values: &[Value]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[index + 1..].contains(value))
}
/// Errors produced during filter validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, displaydoc::Display)]
pub enum ValidateError {
    /// unsupported field path: {0}
    UnsupportedField(String),
    /// type mismatch at field: {0}
    TypeMismatch(String),
}
/// The set of allowed field prefixes: top-level and `metadata.<key>`.
fn is_supported_field(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    // Allow metadata.<key>
    if let Some(rest) = path.strip_prefix("metadata.") {
        return !rest.is_empty();
    }
    // Allow simple top-level; further item-type specific checks are endpoint-specific
    !path.contains('.')
}
/// Validate a filter expression structurally.
///
/// Ensures field paths use supported prefixes; logical and membership operands are non-empty;
/// membership values are unique; and depth, node, and set sizes stay within deterministic bounds.
///
/// # Errors
///
/// Returns `ValidateError::UnsupportedField` for unsupported paths and
/// `ValidateError::TypeMismatch` for invalid operand types, malformed
/// programmatic trees, resource exhaustion, or duplicate/empty membership.
pub fn validate_filter(expr: &FilterExpr) -> Result<(), ValidateError> {
    fn validate_rec(
        expr: &FilterExpr,
        depth: usize,
        nodes: &mut usize,
        membership_values: &mut usize,
    ) -> Result<(), ValidateError> {
        if depth > FILTER_EXPR_MAX_DEPTH {
            return Err(ValidateError::TypeMismatch("depth limit".into()));
        }
        *nodes = nodes.saturating_add(1);
        if *nodes > FILTER_EXPR_MAX_NODES {
            return Err(ValidateError::TypeMismatch("node limit".into()));
        }
        match expr {
            FilterExpr::And(list) | FilterExpr::Or(list) => {
                if list.is_empty() {
                    return Err(ValidateError::TypeMismatch(
                        "logical operators require at least one child".into(),
                    ));
                }
                for e in list {
                    validate_rec(e, depth + 1, nodes, membership_values)?;
                }
                Ok(())
            }
            FilterExpr::Not(inner) => validate_rec(inner, depth + 1, nodes, membership_values),
            FilterExpr::Eq(f, _)
            | FilterExpr::Ne(f, _)
            | FilterExpr::Exists(f)
            | FilterExpr::IsNull(f) => {
                if !is_supported_field(&f.0) {
                    return Err(ValidateError::UnsupportedField(f.0.clone()));
                }
                Ok(())
            }
            FilterExpr::Lt(f, v)
            | FilterExpr::Lte(f, v)
            | FilterExpr::Gt(f, v)
            | FilterExpr::Gte(f, v) => {
                if !is_supported_field(&f.0) {
                    return Err(ValidateError::UnsupportedField(f.0.clone()));
                }
                if !v.is_number() {
                    return Err(ValidateError::TypeMismatch(f.0.clone()));
                }
                Ok(())
            }
            FilterExpr::In(f, vals) | FilterExpr::Nin(f, vals) => {
                if !is_supported_field(&f.0) {
                    return Err(ValidateError::UnsupportedField(f.0.clone()));
                }
                if vals.is_empty() {
                    return Err(ValidateError::TypeMismatch(f.0.clone()));
                }
                if vals.len() > FILTER_EXPR_MAX_MEMBERSHIP_VALUES {
                    return Err(ValidateError::TypeMismatch(format!(
                        "membership values for {}",
                        f.0
                    )));
                }
                if !membership_values_are_unique(vals) {
                    return Err(ValidateError::TypeMismatch(f.0.clone()));
                }
                *membership_values = membership_values.saturating_add(vals.len());
                if *membership_values > FILTER_EXPR_MAX_TOTAL_MEMBERSHIP_VALUES {
                    return Err(ValidateError::TypeMismatch(
                        "total membership values".into(),
                    ));
                }
                // For membership checks, require homogeneous primitive types (strings or numbers).
                let all_strings = vals.iter().all(norito::json::Value::is_string);
                let all_numbers = vals.iter().all(norito::json::Value::is_number);
                let all_bools = vals.iter().all(norito::json::Value::is_bool);
                let metadata_values = f.0.starts_with("metadata.");
                if !metadata_values && !(all_strings || all_numbers || all_bools) {
                    return Err(ValidateError::TypeMismatch(f.0.clone()));
                }
                Ok(())
            }
        }
    }
    let mut nodes = 0;
    let mut membership_values = 0;
    validate_rec(expr, 0, &mut nodes, &mut membership_values)
}
