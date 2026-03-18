//! JSON filter/selector DSL parser and validator for app-facing endpoints.
//!
//! Scope (phase 1):
//! - Parse a simple JSON AST for boolean composition and field comparisons.
//! - Support field paths for top-level fields and `metadata.<key>`.
//! - Validate operators vs value types and field path form.
//! - Map to an internal validated form; mapping to typed predicates is left for
//!   endpoint-specific adapters.

use norito::{
    Error as NoritoError,
    codec::{Decode, Encode},
    json::{self, FastJsonWrite, JsonDeserialize, JsonSerialize, Map, Value},
};

/// A field path such as `authority`, `timestamp_ms`, or `metadata.display_name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPath(pub String);

impl JsonSerialize for FieldPath {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out);
    }
}

impl JsonDeserialize for FieldPath {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let inner = String::json_deserialize(parser)?;
        Ok(FieldPath(inner))
    }
}

/// Filter expression AST.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterExpr {
    /// Logical conjunction of nested predicates.
    And(Vec<FilterExpr>),
    /// Logical disjunction of nested predicates.
    Or(Vec<FilterExpr>),
    /// Logical negation of a predicate.
    Not(Box<FilterExpr>),
    /// Equality comparison against a field.
    Eq(FieldPath, Value),
    /// Inequality comparison against a field.
    Ne(FieldPath, Value),
    /// Field is strictly less than the provided value.
    Lt(FieldPath, Value),
    /// Field is less than or equal to the provided value.
    Lte(FieldPath, Value),
    /// Field is strictly greater than the provided value.
    Gt(FieldPath, Value),
    /// Field is greater than or equal to the provided value.
    Gte(FieldPath, Value),
    /// Field matches any value in the provided set.
    In(FieldPath, Vec<Value>),
    /// Field does not match any value in the provided set.
    Nin(FieldPath, Vec<Value>),
    /// Field exists predicate.
    Exists(FieldPath),
    /// Field value is null predicate.
    IsNull(FieldPath),
}

impl JsonDeserialize for FilterExpr {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        parse_filter_expr(parser)
    }
}

impl FastJsonWrite for FilterExpr {
    fn write_json(&self, out: &mut String) {
        filter_expr_to_value(self).json_serialize(out);
    }
}

/// Selector (projection) definition as a flat list of field paths.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize, Default)]
pub struct Selector(pub Vec<FieldPath>);

/// Sorting key descriptor.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct SortKey {
    /// Field path to sort by.
    pub key: FieldPath,
    /// Sort direction for the field path.
    #[norito(default = "default_order")]
    pub order: Order,
}

/// Sort direction for a single key.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Order {
    /// Sort values in ascending order.
    Asc,
    /// Sort values in descending order.
    Desc,
}

impl JsonSerialize for Order {
    fn json_serialize(&self, out: &mut String) {
        let value = match self {
            Order::Asc => "asc",
            Order::Desc => "desc",
        };
        norito::json::write_json_string(value, out);
    }
}

impl JsonDeserialize for Order {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let start = parser.position();
        let raw = String::json_deserialize(parser)?;
        match raw.as_str() {
            "asc" => Ok(Order::Asc),
            "desc" => Ok(Order::Desc),
            _ => Err(order_parse_error(parser, start)),
        }
    }
}

fn default_order() -> Order {
    Order::Asc
}

fn order_parse_error(parser: &norito::json::Parser<'_>, start: usize) -> norito::json::Error {
    const MSG: &str = "expected \"asc\" or \"desc\"";
    let input = parser.input();
    let clamped = start.min(input.len());
    let mut line = 1usize;
    let mut col = 1usize;
    for ch in input[..clamped].chars() {
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    norito::json::Error::WithPos {
        msg: MSG,
        byte: clamped,
        line,
        col,
    }
}

fn parse_filter_expr(
    parser: &mut norito::json::Parser<'_>,
) -> Result<FilterExpr, norito::json::Error> {
    let val = Value::json_deserialize(parser)?;
    filter_expr_from_value(val)
}

fn parse_filter_expr_option(
    parser: &mut norito::json::Parser<'_>,
) -> Result<Option<FilterExpr>, norito::json::Error> {
    let opt_val = Option::<Value>::json_deserialize(parser)?;
    opt_val.map(filter_expr_from_value).transpose()
}

fn filter_expr_from_value(val: Value) -> Result<FilterExpr, norito::json::Error> {
    match val {
        Value::Object(mut obj) => {
            let op_value = obj
                .remove("op")
                .ok_or_else(|| filter_expr_error("filter expression missing op"))?;
            let op = match op_value {
                Value::String(s) => s,
                _ => return Err(filter_expr_error("filter expression op must be string")),
            };
            let args = obj.remove("args").unwrap_or(Value::Null);
            match op.as_str() {
                "and" => match args {
                    Value::Array(values) => {
                        let mut out = Vec::with_capacity(values.len());
                        for value in values {
                            out.push(filter_expr_from_value(value)?);
                        }
                        Ok(FilterExpr::And(out))
                    }
                    _ => Err(filter_expr_error("and expects array args")),
                },
                "or" => match args {
                    Value::Array(values) => {
                        let mut out = Vec::with_capacity(values.len());
                        for value in values {
                            out.push(filter_expr_from_value(value)?);
                        }
                        Ok(FilterExpr::Or(out))
                    }
                    _ => Err(filter_expr_error("or expects array args")),
                },
                "not" => match args {
                    Value::Array(mut values) if values.len() == 1 => {
                        let inner = filter_expr_from_value(values.remove(0))?;
                        Ok(FilterExpr::Not(Box::new(inner)))
                    }
                    _ => Err(filter_expr_error("not expects array args")),
                },
                "eq" => {
                    let (field, value) = parse_binop_args(args)?;
                    Ok(FilterExpr::Eq(field, value))
                }
                "ne" => {
                    let (field, value) = parse_binop_args(args)?;
                    Ok(FilterExpr::Ne(field, value))
                }
                "lt" => {
                    let (field, value) = parse_binop_args(args)?;
                    Ok(FilterExpr::Lt(field, value))
                }
                "lte" => {
                    let (field, value) = parse_binop_args(args)?;
                    Ok(FilterExpr::Lte(field, value))
                }
                "gt" => {
                    let (field, value) = parse_binop_args(args)?;
                    Ok(FilterExpr::Gt(field, value))
                }
                "gte" => {
                    let (field, value) = parse_binop_args(args)?;
                    Ok(FilterExpr::Gte(field, value))
                }
                "in" => {
                    let (field, values) = parse_membership_args(args)?;
                    Ok(FilterExpr::In(field, values))
                }
                "nin" => {
                    let (field, values) = parse_membership_args(args)?;
                    Ok(FilterExpr::Nin(field, values))
                }
                "exists" => {
                    let field = parse_field_arg(args)?;
                    Ok(FilterExpr::Exists(field))
                }
                "is_null" => {
                    let field = parse_field_arg(args)?;
                    Ok(FilterExpr::IsNull(field))
                }
                _ => Err(filter_expr_error("unsupported filter operator")),
            }
        }
        _ => Err(filter_expr_error("filter expression must be an object")),
    }
}

fn parse_field_arg(arg: Value) -> Result<FieldPath, norito::json::Error> {
    match arg {
        Value::String(s) => Ok(FieldPath(s)),
        _ => Err(filter_expr_error("filter field must be string")),
    }
}

fn parse_binop_args(args: Value) -> Result<(FieldPath, Value), norito::json::Error> {
    match args {
        Value::Array(values) if values.len() == 2 => {
            let mut iter = values.into_iter();
            let field_value = iter.next().expect("len checked");
            let rhs = iter.next().expect("len checked");
            let field = match field_value {
                Value::String(s) => FieldPath(s),
                _ => return Err(filter_expr_error("filter field must be string")),
            };
            Ok((field, rhs))
        }
        _ => Err(filter_expr_error("binary operator expects [field, value]")),
    }
}

fn parse_membership_args(args: Value) -> Result<(FieldPath, Vec<Value>), norito::json::Error> {
    let (field, values) = parse_binop_args(args)?;
    match values {
        Value::Array(items) => Ok((field, items)),
        _ => Err(filter_expr_error(
            "membership operator expects array values",
        )),
    }
}

fn filter_expr_error(msg: &'static str) -> norito::json::Error {
    norito::json::Error::WithPos {
        msg,
        byte: 0,
        line: 1,
        col: 1,
    }
}

/// Convert a filter expression into its JSON representation used on the wire.
pub fn filter_expr_to_value(expr: &FilterExpr) -> Value {
    fn binop(op: &str, field: &FieldPath, rhs: &Value) -> Value {
        let mut m = Map::new();
        m.insert("op".into(), Value::from(op));
        m.insert(
            "args".into(),
            Value::Array(vec![Value::from(field.0.clone()), rhs.clone()]),
        );
        Value::Object(m)
    }

    fn listop(op: &str, items: &[FilterExpr]) -> Value {
        let mut m = Map::new();
        m.insert("op".into(), Value::from(op));
        m.insert(
            "args".into(),
            Value::Array(items.iter().map(filter_expr_to_value).collect()),
        );
        Value::Object(m)
    }

    match expr {
        FilterExpr::And(list) => listop("and", list),
        FilterExpr::Or(list) => listop("or", list),
        FilterExpr::Not(inner) => {
            let mut m = Map::new();
            m.insert("op".into(), Value::from("not"));
            m.insert(
                "args".into(),
                Value::Array(vec![filter_expr_to_value(inner)]),
            );
            Value::Object(m)
        }
        FilterExpr::Eq(field, rhs) => binop("eq", field, rhs),
        FilterExpr::Ne(field, rhs) => binop("ne", field, rhs),
        FilterExpr::Lt(field, rhs) => binop("lt", field, rhs),
        FilterExpr::Lte(field, rhs) => binop("lte", field, rhs),
        FilterExpr::Gt(field, rhs) => binop("gt", field, rhs),
        FilterExpr::Gte(field, rhs) => binop("gte", field, rhs),
        FilterExpr::In(field, values) => {
            let mut m = Map::new();
            m.insert("op".into(), Value::from("in"));
            m.insert(
                "args".into(),
                Value::Array(vec![
                    Value::from(field.0.clone()),
                    Value::Array(values.clone()),
                ]),
            );
            Value::Object(m)
        }
        FilterExpr::Nin(field, values) => {
            let mut m = Map::new();
            m.insert("op".into(), Value::from("nin"));
            m.insert(
                "args".into(),
                Value::Array(vec![
                    Value::from(field.0.clone()),
                    Value::Array(values.clone()),
                ]),
            );
            Value::Object(m)
        }
        FilterExpr::Exists(field) => {
            let mut m = Map::new();
            m.insert("op".into(), Value::from("exists"));
            m.insert(
                "args".into(),
                Value::Array(vec![Value::from(field.0.clone())]),
            );
            Value::Object(m)
        }
        FilterExpr::IsNull(field) => {
            let mut m = Map::new();
            m.insert("op".into(), Value::from("is_null"));
            m.insert(
                "args".into(),
                Value::Array(vec![Value::from(field.0.clone())]),
            );
            Value::Object(m)
        }
    }
}

#[cfg(test)]
mod tests {
    use norito::json;

    use super::*;
    use crate::{json_array, json_object, json_value};

    fn obj(pairs: Vec<(&'static str, Value)>) -> Value {
        json_object(pairs)
    }

    fn arr(values: Vec<Value>) -> Value {
        json_array(values)
    }

    fn val<T: JsonSerialize + ?Sized>(value: &T) -> Value {
        json_value(value)
    }

    #[test]
    fn order_serializes_as_lowercase() {
        let asc = norito::json::to_json(&Order::Asc).unwrap();
        let desc = norito::json::to_json(&Order::Desc).unwrap();
        assert_eq!(asc, "\"asc\"");
        assert_eq!(desc, "\"desc\"");
        assert_eq!(Order::Asc, norito::json::from_str(&asc).unwrap());
        assert_eq!(Order::Desc, norito::json::from_str(&desc).unwrap());
    }

    #[test]
    fn filter_expr_serialization_matches_expected_value() {
        let expr = FilterExpr::Eq(
            FieldPath("id".into()),
            Value::from("6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw"),
        );
        let value = filter_expr_to_value(&expr);
        let expected = obj(vec![
            ("op", val("eq")),
            (
                "args",
                arr(vec![
                    val("id"),
                    val("6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw"),
                ]),
            ),
        ]);
        assert_eq!(value, expected);

        let roundtrip: FilterExpr = norito::json::from_value(value).unwrap();
        assert_eq!(roundtrip, expr);
    }

    #[test]
    fn parse_and_validate_simple_filter() {
        let json = obj(vec![
            ("op", val("and")),
            (
                "args",
                arr(vec![
                    obj(vec![
                        ("op", val("eq")),
                        (
                            "args",
                            arr(vec![
                                val("authority"),
                                val("6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw"),
                            ]),
                        ),
                    ]),
                    obj(vec![
                        ("op", val("gte")),
                        (
                            "args",
                            arr(vec![val("timestamp_ms"), val(&1710000000000u64)]),
                        ),
                    ]),
                    obj(vec![
                        ("op", val("eq")),
                        (
                            "args",
                            arr(vec![val("metadata.display_name"), val("Alice")]),
                        ),
                    ]),
                ]),
            ),
        ]);
        let expr: FilterExpr = json::from_value(json).expect("parse");
        validate_filter(&expr).expect("validate");
    }

    #[test]
    fn reject_unsupported_field_path() {
        let json = obj(vec![
            ("op", val("eq")),
            ("args", arr(vec![val("nested.unsupported"), val(&1u64)])),
        ]);
        let expr: FilterExpr = json::from_value(json).expect("parse");
        let err = validate_filter(&expr).unwrap_err();
        assert!(matches!(err, ValidateError::UnsupportedField(_)));
    }

    #[test]
    fn reject_type_mismatch_for_numeric_ops() {
        // lt with a non-number should fail
        let json = obj(vec![
            ("op", val("lt")),
            ("args", arr(vec![val("timestamp_ms"), val("not-a-number")])),
        ]);
        let expr: FilterExpr = json::from_value(json).expect("parse");
        let err = validate_filter(&expr).unwrap_err();
        assert!(matches!(err, ValidateError::TypeMismatch(_)));

        // in with mixed types should fail
        let json2 = obj(vec![
            ("op", val("in")),
            (
                "args",
                arr(vec![val("tx_status"), arr(vec![val("Queued"), val(&1u64)])]),
            ),
        ]);
        let expr2: FilterExpr = json::from_value(json2).expect("parse");
        let err2 = validate_filter(&expr2).unwrap_err();
        assert!(matches!(err2, ValidateError::TypeMismatch(_)));
    }
}

mod filter_expr_option {
    use super::*;

    #[allow(clippy::ref_option)]
    pub fn serialize(value: &Option<FilterExpr>, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(value, out);
    }

    pub fn deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Option<FilterExpr>, norito::json::Error> {
        parse_filter_expr_option(parser)
    }
}

/// Query parameters supplied alongside filter/selector.
#[derive(Debug, Clone, PartialEq, JsonSerialize, JsonDeserialize, Default)]
pub struct QueryEnvelope {
    /// Optional human-readable name (used only for JSON envelopes; not required by convenience endpoints).
    #[norito(default)]
    pub query: Option<String>,
    /// Optional predicate tree applied before projection.
    #[norito(default)]
    #[norito(with = "filter_expr_option")]
    pub filter: Option<FilterExpr>,
    /// Optional projection limiting the fields included in each result.
    #[norito(default)]
    pub select: Option<Selector>,
    /// Sort specification evaluated ahead of pagination.
    #[norito(default)]
    pub sort: Vec<SortKey>,
    /// Pagination controls for offset/limit style queries.
    #[norito(default)]
    pub pagination: Pagination,
    /// Optional batch fetch size for iterable queries.
    pub fetch_size: Option<u64>,
}

const _: () = {
    fn assert_send_sync<T: Send + Sync>() {}
    fn check() {
        assert_send_sync::<QueryEnvelope>();
    }
    let _ = check;
};

impl crate::utils::extractors::SupportsNoritoDecode for QueryEnvelope {
    fn decode_norito(bytes: &[u8]) -> Result<Self, NoritoError> {
        norito::json::from_slice::<Self>(bytes)
            .map_err(|e| NoritoError::Message(format!("invalid QueryEnvelope: {e}")))
    }
}

/// Pagination controls for list and query endpoints.
#[derive(Debug, Copy, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize, Default)]
pub struct Pagination {
    /// Maximum number of items to return.
    pub limit: Option<u64>,
    /// Zero-based offset into the result set.
    #[norito(default)]
    pub offset: u64,
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
/// Ensures field paths use supported prefixes and nesting depth stays within
/// the allowed maximum.
///
/// # Errors
///
/// Returns `ValidateError::UnsupportedField` when a predicate targets an
/// unsupported field path, or `ValidateError::TypeMismatch` when the expression
/// tree exceeds the maximum depth or uses an unsupported comparison target.
pub fn validate_filter(expr: &FilterExpr) -> Result<(), ValidateError> {
    const MAX_DEPTH: usize = 10;
    fn validate_rec(expr: &FilterExpr, depth: usize) -> Result<(), ValidateError> {
        if depth > MAX_DEPTH {
            return Err(ValidateError::TypeMismatch("depth".into()));
        }
        match expr {
            FilterExpr::And(list) | FilterExpr::Or(list) => {
                for e in list {
                    validate_rec(e, depth + 1)?;
                }
                Ok(())
            }
            FilterExpr::Not(inner) => validate_rec(inner, depth + 1),
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
                // For membership checks, require homogeneous primitive types (strings or numbers).
                // Empty list is allowed and simply matches nothing/everything depending on op semantics downstream.
                if !vals.is_empty() {
                    let all_strings = vals.iter().all(norito::json::Value::is_string);
                    let all_numbers = vals.iter().all(norito::json::Value::is_number);
                    if !(all_strings || all_numbers) {
                        return Err(ValidateError::TypeMismatch(f.0.clone()));
                    }
                }
                Ok(())
            }
        }
    }
    validate_rec(expr, 0)
}

impl norito::core::NoritoSerialize for FieldPath {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        <String as norito::core::NoritoSerialize>::serialize(&self.0, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for FieldPath {
    fn try_deserialize(
        archived: &'de norito::core::Archived<FieldPath>,
    ) -> Result<Self, norito::core::Error> {
        let archived_str: &norito::core::Archived<String> = archived.cast();
        let inner = <String as norito::core::NoritoDeserialize>::try_deserialize(archived_str)?;
        Ok(FieldPath(inner))
    }

    fn deserialize(archived: &'de norito::core::Archived<FieldPath>) -> Self {
        Self::try_deserialize(archived)
            .expect("FieldPath should deserialize from a valid Norito string")
    }
}

impl norito::core::NoritoSerialize for Selector {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        <Vec<FieldPath> as norito::core::NoritoSerialize>::serialize(&self.0, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for Selector {
    fn try_deserialize(
        archived: &'de norito::core::Archived<Selector>,
    ) -> Result<Self, norito::core::Error> {
        let archived_inner: &norito::core::Archived<Vec<FieldPath>> = archived.cast();
        let inner =
            <Vec<FieldPath> as norito::core::NoritoDeserialize>::try_deserialize(archived_inner)?;
        Ok(Selector(inner))
    }

    fn deserialize(archived: &'de norito::core::Archived<Selector>) -> Self {
        Self::try_deserialize(archived).expect("Selector should decode from a Norito sequence")
    }
}

impl norito::core::NoritoSerialize for Order {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let tag = match self {
            Order::Asc => 0u8,
            Order::Desc => 1u8,
        };
        <u8 as norito::core::NoritoSerialize>::serialize(&tag, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for Order {
    fn try_deserialize(
        archived: &'de norito::core::Archived<Order>,
    ) -> Result<Self, norito::core::Error> {
        let archived_tag: &norito::core::Archived<u8> = archived.cast();
        let tag = <u8 as norito::core::NoritoDeserialize>::try_deserialize(archived_tag)?;
        match tag {
            0 => Ok(Order::Asc),
            1 => Ok(Order::Desc),
            other => Err(norito::core::Error::Message(format!(
                "invalid Order tag: {other}"
            ))),
        }
    }

    fn deserialize(archived: &'de norito::core::Archived<Order>) -> Self {
        Self::try_deserialize(archived).expect("Order should decode from variant tag")
    }
}

impl norito::core::NoritoSerialize for SortKey {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let payload = (self.key.clone(), self.order);
        <(FieldPath, Order) as norito::core::NoritoSerialize>::serialize(&payload, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for SortKey {
    fn try_deserialize(
        archived: &'de norito::core::Archived<SortKey>,
    ) -> Result<Self, norito::core::Error> {
        let archived_pair: &norito::core::Archived<(FieldPath, Order)> = archived.cast();
        let (key, order) =
            <(FieldPath, Order) as norito::core::NoritoDeserialize>::try_deserialize(
                archived_pair,
            )?;
        Ok(SortKey { key, order })
    }

    fn deserialize(archived: &'de norito::core::Archived<SortKey>) -> Self {
        Self::try_deserialize(archived).expect("SortKey should decode from (FieldPath, Order)")
    }
}

impl norito::core::NoritoSerialize for FilterExpr {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let json = norito::json::to_string(&filter_expr_to_value(self))
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        <String as norito::core::NoritoSerialize>::serialize(&json, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for FilterExpr {
    fn try_deserialize(
        archived: &'de norito::core::Archived<FilterExpr>,
    ) -> Result<Self, norito::core::Error> {
        let archived_str: &norito::core::Archived<String> = archived.cast();
        let json = <String as norito::core::NoritoDeserialize>::try_deserialize(archived_str)?;
        norito::json::from_str::<FilterExpr>(&json)
            .map_err(|err| norito::core::Error::Message(err.to_string()))
    }

    fn deserialize(archived: &'de norito::core::Archived<FilterExpr>) -> Self {
        Self::try_deserialize(archived)
            .expect("FilterExpr should deserialize from canonical JSON form")
    }
}
