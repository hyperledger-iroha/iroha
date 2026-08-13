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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Selector(pub Vec<FieldPath>);
impl JsonSerialize for Selector {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out);
    }
}
impl JsonDeserialize for Selector {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        Vec::<FieldPath>::json_deserialize(parser).map(Self)
    }
}
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
/// Aggregate function applied in aggregate query mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AggregateFn {
    /// Count rows in each group.
    Count,
    /// Sum the referenced field.
    Sum,
    /// Compute the minimum referenced field.
    Min,
    /// Compute the maximum referenced field.
    Max,
    /// Compute the average referenced field.
    Avg,
    /// Count distinct values for the referenced field.
    DistinctCount,
}
impl JsonSerialize for AggregateFn {
    fn json_serialize(&self, out: &mut String) {
        let value = match self {
            AggregateFn::Count => "count",
            AggregateFn::Sum => "sum",
            AggregateFn::Min => "min",
            AggregateFn::Max => "max",
            AggregateFn::Avg => "avg",
            AggregateFn::DistinctCount => "distinct_count",
        };
        norito::json::write_json_string(value, out);
    }
}
impl JsonDeserialize for AggregateFn {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let start = parser.position();
        let raw = String::json_deserialize(parser)?;
        match raw.as_str() {
            "count" => Ok(AggregateFn::Count),
            "sum" => Ok(AggregateFn::Sum),
            "min" => Ok(AggregateFn::Min),
            "max" => Ok(AggregateFn::Max),
            "avg" => Ok(AggregateFn::Avg),
            "distinct_count" => Ok(AggregateFn::DistinctCount),
            _ => Err(order_parse_error(parser, start)),
        }
    }
}
/// One metric definition in aggregate query mode.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct AggregateMetric {
    /// Output alias used in aggregate rows and `having` clauses.
    pub alias: String,
    /// Aggregate function name.
    #[norito(rename = "fn")]
    pub r#fn: AggregateFn,
    /// Optional field path consumed by the aggregate function.
    #[norito(default)]
    pub field: Option<FieldPath>,
}
/// Aggregate grouping and metric specification.
#[derive(Debug, Clone, PartialEq, JsonSerialize, JsonDeserialize, Default)]
pub struct AggregateSpec {
    /// Grouping dimensions evaluated before metric reduction.
    #[norito(default)]
    pub group_by: Vec<FieldPath>,
    /// Metrics computed per group.
    #[norito(default)]
    pub metrics: Vec<AggregateMetric>,
    /// Optional predicate tree applied after aggregation.
    #[norito(default)]
    #[norito(with = "filter_expr_option")]
    pub having: Option<FilterExpr>,
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
#[allow(unsafe_code)]
fn admitted_filter_vec<T>(capacity: usize) -> Result<Vec<T>, norito::json::Error> {
    let requested = capacity
        .checked_mul(core::mem::size_of::<T>())
        .ok_or(norito::json::Error::DecodeResourceLimit)?;
    norito::core::reserve_decode_allocation(requested)
        .map_err(norito::json::Error::from_decode_resource)?;
    if requested == 0 {
        return Ok(Vec::new());
    }
    let layout = std::alloc::Layout::array::<T>(capacity)
        .map_err(|_| norito::json::Error::AllocationFailed)?;
    // SAFETY: the complete exact layout was admitted before allocation. Null
    // is rejected before ownership, and the returned vector starts empty so
    // only later successful pushes create initialized elements for `Drop`.
    let allocation = unsafe { std::alloc::alloc(layout) };
    let allocation =
        core::ptr::NonNull::new(allocation).ok_or(norito::json::Error::AllocationFailed)?;
    Ok(unsafe { Vec::from_raw_parts(allocation.as_ptr().cast::<T>(), 0, capacity) })
}
fn parse_filter_expr_option(
    parser: &mut norito::json::Parser<'_>,
) -> Result<Option<FilterExpr>, norito::json::Error> {
    let opt_val = Option::<Value>::json_deserialize(parser)?;
    opt_val.map(filter_expr_from_value).transpose()
}
/// Maximum nesting accepted in an app-facing filter expression.
pub(crate) const FILTER_EXPR_MAX_DEPTH: usize = 10;
/// Maximum operator nodes accepted in an app-facing filter expression.
pub(crate) const FILTER_EXPR_MAX_NODES: usize = 1_024;
/// Maximum literals accepted by one membership operator.
pub(crate) const FILTER_EXPR_MAX_MEMBERSHIP_VALUES: usize = 1_024;
/// Maximum membership literals accepted across one expression tree.
pub(crate) const FILTER_EXPR_MAX_TOTAL_MEMBERSHIP_VALUES: usize = 4_096;
#[derive(Default)]
struct FilterExprBudget {
    nodes: usize,
    membership_values: usize,
}
impl FilterExprBudget {
    fn enter(&mut self, depth: usize) -> Result<(), norito::json::Error> {
        if depth > FILTER_EXPR_MAX_DEPTH {
            return Err(filter_expr_error("filter expression exceeds depth limit"));
        }
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > FILTER_EXPR_MAX_NODES {
            return Err(filter_expr_error("filter expression exceeds node limit"));
        }
        Ok(())
    }
    fn add_membership(&mut self, count: usize) -> Result<(), norito::json::Error> {
        if count == 0 {
            return Err(filter_expr_error(
                "membership operator values must not be empty",
            ));
        }
        if count > FILTER_EXPR_MAX_MEMBERSHIP_VALUES {
            return Err(filter_expr_error(
                "membership operator exceeds per-set limit",
            ));
        }
        self.membership_values = self.membership_values.saturating_add(count);
        if self.membership_values > FILTER_EXPR_MAX_TOTAL_MEMBERSHIP_VALUES {
            return Err(filter_expr_error(
                "filter expression exceeds total membership limit",
            ));
        }
        Ok(())
    }
}
fn filter_expr_from_value(val: Value) -> Result<FilterExpr, norito::json::Error> {
    filter_expr_from_value_inner(val, 0, &mut FilterExprBudget::default())
}
fn filter_expr_from_value_inner(
    val: Value,
    depth: usize,
    budget: &mut FilterExprBudget,
) -> Result<FilterExpr, norito::json::Error> {
    budget.enter(depth)?;
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
            if !obj.is_empty() {
                return Err(filter_expr_error(
                    "filter expression contains unknown fields",
                ));
            }
            match op.as_str() {
                "and" => match args {
                    Value::Array(values) if !values.is_empty() => {
                        if values.len() > FILTER_EXPR_MAX_NODES.saturating_sub(budget.nodes) {
                            return Err(filter_expr_error("filter expression exceeds node limit"));
                        }
                        let mut out = admitted_filter_vec(values.len())?;
                        for value in values {
                            out.push(filter_expr_from_value_inner(value, depth + 1, budget)?);
                        }
                        Ok(FilterExpr::And(out))
                    }
                    _ => Err(filter_expr_error("and expects non-empty array args")),
                },
                "or" => match args {
                    Value::Array(values) if !values.is_empty() => {
                        if values.len() > FILTER_EXPR_MAX_NODES.saturating_sub(budget.nodes) {
                            return Err(filter_expr_error("filter expression exceeds node limit"));
                        }
                        let mut out = admitted_filter_vec(values.len())?;
                        for value in values {
                            out.push(filter_expr_from_value_inner(value, depth + 1, budget)?);
                        }
                        Ok(FilterExpr::Or(out))
                    }
                    _ => Err(filter_expr_error("or expects non-empty array args")),
                },
                "not" => match args {
                    Value::Array(mut values) if values.len() == 1 => {
                        let inner =
                            filter_expr_from_value_inner(values.remove(0), depth + 1, budget)?;
                        norito::core::reserve_decode_box_allocation::<FilterExpr>()
                            .map_err(norito::json::Error::from_decode_resource)?;
                        Ok(FilterExpr::Not(Box::new(inner)))
                    }
                    _ => Err(filter_expr_error(
                        "not expects exactly one predicate in array args",
                    )),
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
                    budget.add_membership(values.len())?;
                    Ok(FilterExpr::In(field, values))
                }
                "nin" => {
                    let (field, values) = parse_membership_args(args)?;
                    budget.add_membership(values.len())?;
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
        Value::Array(mut values) if values.len() == 1 => match values.remove(0) {
            Value::String(s) => Ok(FieldPath(s)),
            _ => Err(filter_expr_error("filter field must be string")),
        },
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
        Value::Array(items) => {
            if items.is_empty() {
                return Err(filter_expr_error(
                    "membership operator values must not be empty",
                ));
            }
            if items.len() > FILTER_EXPR_MAX_MEMBERSHIP_VALUES {
                return Err(filter_expr_error(
                    "membership operator exceeds per-set limit",
                ));
            }
            if !membership_values_are_unique(&items) {
                return Err(filter_expr_error(
                    "membership operator values must be unique canonical JSON literals",
                ));
            }
            Ok((field, items))
        }
        _ => Err(filter_expr_error(
            "membership operator expects array values",
        )),
    }
}
fn membership_values_are_unique(values: &[Value]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[index + 1..].contains(value))
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
    fn aggregate_function_serializes_as_lowercase() {
        let count = norito::json::to_json(&AggregateFn::Count).unwrap();
        let distinct = norito::json::to_json(&AggregateFn::DistinctCount).unwrap();
        assert_eq!(count, "\"count\"");
        assert_eq!(distinct, "\"distinct_count\"");
        assert_eq!(AggregateFn::Count, norito::json::from_str(&count).unwrap());
        assert_eq!(
            AggregateFn::DistinctCount,
            norito::json::from_str(&distinct).unwrap()
        );
    }
    #[test]
    fn filter_expr_serialization_matches_expected_value() {
        let expr = FilterExpr::Eq(
            FieldPath("id".into()),
            Value::from("sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"),
        );
        let value = filter_expr_to_value(&expr);
        let expected = obj(vec![
            ("op", val("eq")),
            (
                "args",
                arr(vec![
                    val("id"),
                    val("sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"),
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
                                val("sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"),
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
    fn filter_expr_rejects_unknown_fields_empty_sets_and_malformed_logical_arity() {
        let invalid = [
            norito::json!({"op": "eq", "args": ["result_ok", true], "extra": 1}),
            norito::json!({"op": "and", "args": []}),
            norito::json!({"op": "or", "args": []}),
            norito::json!({"op": "not", "args": []}),
            norito::json!({"op": "in", "args": ["result_ok", []]}),
            norito::json!({"op": "nin", "args": ["result_ok", [true, true]]}),
            norito::json!({"op": "exists", "args": "result_ok"}),
        ];
        for value in invalid {
            assert!(
                json::from_value::<FilterExpr>(value).is_err(),
                "malformed filter expression must be rejected"
            );
        }
    }
    #[test]
    fn filter_expr_presence_roundtrip_uses_canonical_single_argument_array() {
        for expr in [
            FilterExpr::Exists(FieldPath("result_ok".into())),
            FilterExpr::IsNull(FieldPath("metadata.note".into())),
        ] {
            let value = filter_expr_to_value(&expr);
            let decoded: FilterExpr = json::from_value(value).expect("presence roundtrip");
            assert_eq!(decoded, expr);
        }
    }
    #[test]
    fn filter_expr_binary_wire_roundtrips_and_rejects_noncanonical_json_replay() {
        let expr = FilterExpr::Eq(FieldPath("result_ok".into()), Value::Bool(true));
        let bytes = norito::to_bytes(&expr).expect("encode valid FilterExpr");
        assert_eq!(
            norito::decode_from_bytes::<FilterExpr>(&bytes).expect("decode valid FilterExpr"),
            expr
        );
        let canonical = json::to_string(&filter_expr_to_value(&expr)).expect("canonical filter");
        let replay = format!(" {canonical}");
        let (payload, flags) = norito::codec::encode_with_header_flags(&replay);
        let framed = norito::core::frame_bare_with_header_flags::<FilterExpr>(&payload, flags)
            .expect("frame replayed FilterExpr string");
        assert!(norito::decode_from_bytes::<FilterExpr>(&framed).is_err());
    }
    #[test]
    fn filter_expr_parser_enforces_depth_node_and_membership_budgets() {
        let mut deep = norito::json!({"op": "eq", "args": ["result_ok", true]});
        for _ in 0..=FILTER_EXPR_MAX_DEPTH {
            deep = norito::json!({"op": "not", "args": [deep]});
        }
        assert!(json::from_value::<FilterExpr>(deep).is_err());
        let nodes = Value::Array(
            (0..FILTER_EXPR_MAX_NODES)
                .map(|_| norito::json!({"op": "eq", "args": ["result_ok", true]}))
                .collect(),
        );
        let mut root = Map::new();
        root.insert("op".into(), Value::String("and".into()));
        root.insert("args".into(), nodes);
        assert!(json::from_value::<FilterExpr>(Value::Object(root)).is_err());
        let oversized = Value::Array(
            (0..=FILTER_EXPR_MAX_MEMBERSHIP_VALUES)
                .map(|value| Value::from(value as u64))
                .collect(),
        );
        let mut membership = Map::new();
        membership.insert("op".into(), Value::String("in".into()));
        membership.insert(
            "args".into(),
            Value::Array(vec![Value::String("timestamp_ms".into()), oversized]),
        );
        assert!(json::from_value::<FilterExpr>(Value::Object(membership)).is_err());
    }
    #[test]
    fn filter_expr_destination_is_charged_before_exact_allocation() {
        const ENTRIES: usize = 17;
        let bytes = ENTRIES * core::mem::size_of::<u64>();
        let limits = |allocated| {
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, allocated, usize::MAX)
        };
        let (values, usage) = norito::core::with_decode_limits_measured(limits(bytes), || {
            admitted_filter_vec::<u64>(ENTRIES)
        });
        let values = values.expect("exact filter destination budget");
        assert_eq!(values.capacity(), ENTRIES);
        assert_eq!(usage.total_allocated_bytes(), bytes);
        let (rejected, usage) =
            norito::core::with_decode_limits_measured(limits(bytes - 1), || {
                admitted_filter_vec::<u64>(ENTRIES)
            });
        assert!(matches!(
            rejected,
            Err(norito::json::Error::DecodeResourceLimit)
        ));
        assert_eq!(usage.total_allocated_bytes(), 0);
    }
    #[test]
    fn query_envelope_rejects_empty_negative_membership_instead_of_passing() {
        let envelope = norito::json!({
            "filter": {"op": "nin", "args": ["result_ok", []]}
        });
        assert!(json::from_value::<QueryEnvelope>(envelope).is_err());
        let programmatic = FilterExpr::Nin(FieldPath("result_ok".into()), Vec::new());
        assert!(matches!(
            validate_filter(&programmatic),
            Err(ValidateError::TypeMismatch(field)) if field == "result_ok"
        ));
        assert!(norito::to_bytes(&programmatic).is_err());
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
    #[test]
    fn query_envelope_parses_aggregate_mode() {
        let json = obj(vec![
            (
                "aggregate",
                obj(vec![
                    ("group_by", arr(vec![val("primary_alias_domain")])),
                    (
                        "metrics",
                        arr(vec![
                            obj(vec![
                                ("alias", val("user_count")),
                                ("fn", val("distinct_count")),
                                ("field", val("account_id")),
                            ]),
                            obj(vec![
                                ("alias", val("pkr_total")),
                                ("fn", val("sum")),
                                ("field", val("quantity")),
                            ]),
                        ]),
                    ),
                    (
                        "having",
                        obj(vec![
                            ("op", val("gt")),
                            ("args", arr(vec![val("pkr_total"), val("0")])),
                        ]),
                    ),
                ]),
            ),
            (
                "sort",
                arr(vec![obj(vec![
                    ("key", val("primary_alias_domain")),
                    ("order", val("asc")),
                ])]),
            ),
        ]);
        let envelope: QueryEnvelope = json::from_value(json).expect("parse aggregate envelope");
        let aggregate = envelope.aggregate.expect("aggregate spec");
        assert_eq!(
            aggregate.group_by,
            vec![FieldPath("primary_alias_domain".into())]
        );
        assert_eq!(aggregate.metrics.len(), 2);
        assert_eq!(aggregate.metrics[0].alias, "user_count");
        assert_eq!(aggregate.metrics[0].r#fn, AggregateFn::DistinctCount);
        assert_eq!(
            aggregate.metrics[0].field,
            Some(FieldPath("account_id".into()))
        );
        assert_eq!(envelope.sort.len(), 1);
        assert_eq!(envelope.sort[0].key.0, "primary_alias_domain");
        assert_eq!(envelope.sort[0].order, Order::Asc);
    }
    #[test]
    fn query_envelope_parses_nested_sort_keys() {
        let json = obj(vec![(
            "sort",
            arr(vec![obj(vec![
                ("key", val("alias_binding.bound_at_ms")),
                ("order", val("desc")),
            ])]),
        )]);
        let envelope: QueryEnvelope = json::from_value(json).expect("parse sort envelope");
        assert_eq!(envelope.sort.len(), 1);
        assert_eq!(envelope.sort[0].key.0, "alias_binding.bound_at_ms");
        assert_eq!(envelope.sort[0].order, Order::Desc);
    }
    #[test]
    fn query_envelope_parses_and_serializes_selector_array() {
        let raw = r#"{"select":["authority","metadata.amount"]}"#;
        let envelope: QueryEnvelope = json::from_str(raw).expect("parse selector array");
        assert_eq!(
            envelope.select,
            Some(Selector(vec![
                FieldPath("authority".into()),
                FieldPath("metadata.amount".into()),
            ]))
        );
        let encoded = json::to_json(&envelope).expect("serialize query envelope");
        let value: Value = json::from_str(&encoded).expect("parse serialized query envelope");
        assert_eq!(
            value.as_object().and_then(|object| object.get("select")),
            Some(&arr(vec![val("authority"), val("metadata.amount")]))
        );
    }
    #[test]
    fn query_envelope_json_roundtrip_preserves_nested_sort_keys() {
        let envelope = QueryEnvelope {
            query: None,
            filter: None,
            select: None,
            aggregate: None,
            sort: vec![SortKey {
                key: FieldPath("alias_binding.bound_at_ms".into()),
                order: Order::Desc,
            }],
            pagination: Pagination {
                limit: Some(8),
                offset: 0,
            },
            fetch_size: None,
            count_mode: None,
        };
        let bytes = norito::json::to_vec(&envelope).expect("serialize envelope");
        let decoded: QueryEnvelope = norito::json::from_slice(&bytes).expect("decode envelope");
        assert_eq!(decoded.sort, envelope.sort);
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
    /// Optional aggregate mode evaluated after filtering and before pagination.
    #[norito(default)]
    pub aggregate: Option<AggregateSpec>,
    /// Sort specification evaluated ahead of pagination.
    #[norito(default)]
    pub sort: Vec<SortKey>,
    /// Pagination controls for offset/limit style queries.
    #[norito(default)]
    pub pagination: Pagination,
    /// Optional batch fetch size for iterable queries.
    #[norito(default)]
    pub fetch_size: Option<u64>,
    /// Count mode: "bounded" omits exact totals; "exact" preserves total counts.
    #[norito(default)]
    pub count_mode: Option<String>,
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
/// Ensures field paths use supported prefixes; logical and membership operands
/// are non-empty; membership values are unique; and depth, node, and set sizes
/// stay within deterministic bounds.
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
impl norito::core::NoritoSerialize for FieldPath {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
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
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
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
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
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
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
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
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        validate_filter(self)
            .map_err(|err| norito::core::Error::Message(format!("invalid FilterExpr: {err}")))?;
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
        let expr = norito::json::from_str::<FilterExpr>(&json)
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        validate_filter(&expr)
            .map_err(|err| norito::core::Error::Message(format!("invalid FilterExpr: {err}")))?;
        let canonical = norito::json::to_string(&filter_expr_to_value(&expr))
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        if json != canonical {
            return Err(norito::core::Error::Message(
                "FilterExpr binary payload must contain canonical JSON".into(),
            ));
        }
        Ok(expr)
    }
    fn deserialize(archived: &'de norito::core::Archived<FilterExpr>) -> Self {
        Self::try_deserialize(archived)
            .expect("FilterExpr should deserialize from canonical JSON form")
    }
}
