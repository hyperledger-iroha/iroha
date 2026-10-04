//! Filter expression AST and its canonical JSON form.
//!
//! The JSON form is the structured wire spelling used by SDK builders:
//! `{"op": "<operator>", "args": [...]}`. The same tree also has a human
//! text form (see [`super::text`]); both decode into the same
//! [`FilterExpr`] and are subject to the same structural limits.
use norito::json::{self, FastJsonWrite, JsonDeserialize, JsonSerialize, Map, Value};
use std::fmt;

/// Maximum nesting depth of a filter expression (root is depth 0).
pub const FILTER_MAX_DEPTH: usize = 10;
/// Maximum operator nodes in one filter expression.
pub const FILTER_MAX_NODES: usize = 1_024;
/// Maximum literals accepted by one `in` / `not in` operator.
pub const FILTER_MAX_MEMBERSHIP_VALUES: usize = 1_024;
/// Maximum membership literals across one filter expression.
pub const FILTER_MAX_TOTAL_MEMBERSHIP_VALUES: usize = 4_096;
/// Maximum UTF-8 length of one field path.
pub const FIELD_PATH_MAX_BYTES: usize = 256;

/// A dotted field path such as `owned_by`, `quantity` or `metadata.tier`.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii_shared::list_query::FieldPath")]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldPath(pub String);

impl FieldPath {
    /// Construct a field path from its dotted spelling.
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// The dotted spelling of this path.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Iterate over the dot-separated segments of the path.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('.')
    }

    /// Check the path syntax: non-empty segments, bounded length, no
    /// whitespace or control characters.
    ///
    /// Whether a resource actually exposes the field is decided by the
    /// resource that executes the query, not here.
    ///
    /// # Errors
    /// Returns [`FilterError::InvalidField`] describing the first violation.
    pub fn validate(&self) -> Result<(), FilterError> {
        let invalid = |reason: &'static str| FilterError::InvalidField {
            field: self.0.clone(),
            reason,
        };
        if self.0.is_empty() {
            return Err(invalid("field paths must not be empty"));
        }
        if self.0.len() > FIELD_PATH_MAX_BYTES {
            return Err(invalid("field paths must not exceed 256 bytes"));
        }
        if self
            .0
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
        {
            return Err(invalid(
                "field paths must not contain whitespace or control characters",
            ));
        }
        if self.segments().any(str::is_empty) {
            return Err(invalid("field path segments must not be empty"));
        }
        // The text form quotes segments with backticks and has no escape.
        if self.0.contains('`') {
            return Err(invalid("field paths must not contain backticks"));
        }
        Ok(())
    }
}

impl fmt::Display for FieldPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        super::text::write_field_path(self, f)
    }
}

impl From<&str> for FieldPath {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for FieldPath {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl JsonSerialize for FieldPath {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out);
    }
}

impl JsonDeserialize for FieldPath {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        String::json_deserialize(parser).map(FieldPath)
    }
}

/// Filter expression evaluated against each item of a collection.
///
/// Comparison operators take a field path on the left and a JSON literal on
/// the right. Decimal amounts and integers larger than `u64` are carried as
/// exact decimal strings.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii_shared::list_query::FilterExpr")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterExpr {
    /// All nested expressions match (`a and b`).
    And(Vec<FilterExpr>),
    /// At least one nested expression matches (`a or b`).
    Or(Vec<FilterExpr>),
    /// The nested expression does not match (`not a`).
    Not(Box<FilterExpr>),
    /// Field equals the literal (`field = value`).
    Eq(FieldPath, Value),
    /// Field differs from the literal or is absent (`field != value`).
    Ne(FieldPath, Value),
    /// Field is less than the literal (`field < value`).
    Lt(FieldPath, Value),
    /// Field is less than or equal to the literal (`field <= value`).
    Lte(FieldPath, Value),
    /// Field is greater than the literal (`field > value`).
    Gt(FieldPath, Value),
    /// Field is greater than or equal to the literal (`field >= value`).
    Gte(FieldPath, Value),
    /// Field equals one of the literals (`field in [a, b]`).
    In(FieldPath, Vec<Value>),
    /// Field equals none of the literals or is absent (`field not in [a, b]`).
    Nin(FieldPath, Vec<Value>),
    /// Field is present (`exists(field)`).
    Exists(FieldPath),
    /// Field is absent or null (`field is null`).
    IsNull(FieldPath),
}

impl FilterExpr {
    /// Operator name used by the JSON form.
    pub const fn op_name(&self) -> &'static str {
        match self {
            Self::And(_) => "and",
            Self::Or(_) => "or",
            Self::Not(_) => "not",
            Self::Eq(..) => "eq",
            Self::Ne(..) => "ne",
            Self::Lt(..) => "lt",
            Self::Lte(..) => "lte",
            Self::Gt(..) => "gt",
            Self::Gte(..) => "gte",
            Self::In(..) => "in",
            Self::Nin(..) => "nin",
            Self::Exists(_) => "exists",
            Self::IsNull(_) => "is_null",
        }
    }

    /// The field referenced by a leaf predicate, or `None` for `and`/`or`/`not`.
    pub const fn field(&self) -> Option<&FieldPath> {
        match self {
            Self::And(_) | Self::Or(_) | Self::Not(_) => None,
            Self::Eq(field, _)
            | Self::Ne(field, _)
            | Self::Lt(field, _)
            | Self::Lte(field, _)
            | Self::Gt(field, _)
            | Self::Gte(field, _)
            | Self::In(field, _)
            | Self::Nin(field, _)
            | Self::Exists(field)
            | Self::IsNull(field) => Some(field),
        }
    }

    /// Visit every leaf predicate in evaluation order.
    pub fn for_each_leaf<'a>(&'a self, visit: &mut impl FnMut(&'a FilterExpr)) {
        match self {
            Self::And(list) | Self::Or(list) => {
                for nested in list {
                    nested.for_each_leaf(visit);
                }
            }
            Self::Not(inner) => inner.for_each_leaf(visit),
            leaf => visit(leaf),
        }
    }

    /// Nesting depth of the tree (a single leaf has depth 0).
    pub fn depth(&self) -> usize {
        match self {
            Self::And(list) | Self::Or(list) => {
                1 + list.iter().map(Self::depth).max().unwrap_or_default()
            }
            Self::Not(inner) => 1 + inner.depth(),
            _ => 0,
        }
    }

    /// Check structural limits and operand shapes.
    ///
    /// This accepts every tree that the JSON and text parsers produce and
    /// rejects programmatically built trees that they would refuse.
    ///
    /// # Errors
    /// Returns the first [`FilterError`] found in evaluation order.
    pub fn validate(&self) -> Result<(), FilterError> {
        let mut budget = FilterBudget::default();
        validate_rec(self, 0, &mut budget)
    }

    /// Canonical JSON form (`{"op": ..., "args": [...]}`).
    pub fn to_json_value(&self) -> Value {
        filter_expr_to_value(self)
    }

    /// Decode the canonical JSON form.
    ///
    /// # Errors
    /// Returns a [`FilterError`] naming the offending node with a
    /// JSON-pointer-like location such as `args[1].args[0]`.
    pub fn from_json_value(value: Value) -> Result<Self, FilterError> {
        let mut budget = FilterBudget::default();
        let mut location = Location::default();
        let expr = from_value_rec(value, 0, &mut budget, &mut location)?;
        Ok(expr)
    }
}

/// Structural or operand error in a filter expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterError {
    /// The JSON form is malformed at `location`.
    Malformed {
        /// JSON-pointer-like location of the offending node (`""` for the root).
        location: String,
        /// What was wrong.
        reason: String,
    },
    /// A field path is syntactically invalid.
    InvalidField {
        /// The rejected field path.
        field: String,
        /// What was wrong.
        reason: &'static str,
    },
    /// An operand does not fit its operator.
    InvalidOperand {
        /// Field the operand belongs to.
        field: String,
        /// What was wrong.
        reason: &'static str,
    },
    /// A deterministic size bound was exceeded.
    LimitExceeded {
        /// Which bound was exceeded.
        limit: &'static str,
        /// The configured maximum.
        max: usize,
    },
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { location, reason } if location.is_empty() => f.write_str(reason),
            Self::Malformed { location, reason } => write!(f, "{reason} (at `{location}`)"),
            Self::InvalidField { field, reason } => write!(f, "invalid field `{field}`: {reason}"),
            Self::InvalidOperand { field, reason } => {
                write!(f, "invalid operand for `{field}`: {reason}")
            }
            Self::LimitExceeded { limit, max } => {
                write!(f, "filter exceeds the {limit} limit of {max}")
            }
        }
    }
}

impl std::error::Error for FilterError {}

impl FilterError {
    /// Field associated with the error, when there is one.
    pub fn field(&self) -> Option<&str> {
        match self {
            Self::InvalidField { field, .. } | Self::InvalidOperand { field, .. } => Some(field),
            Self::Malformed { .. } | Self::LimitExceeded { .. } => None,
        }
    }
}

#[derive(Default)]
struct FilterBudget {
    nodes: usize,
    membership_values: usize,
}

impl FilterBudget {
    fn enter(&mut self, depth: usize) -> Result<(), FilterError> {
        if depth > FILTER_MAX_DEPTH {
            return Err(FilterError::LimitExceeded {
                limit: "nesting depth",
                max: FILTER_MAX_DEPTH,
            });
        }
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > FILTER_MAX_NODES {
            return Err(FilterError::LimitExceeded {
                limit: "node count",
                max: FILTER_MAX_NODES,
            });
        }
        Ok(())
    }

    fn membership(&mut self, field: &FieldPath, values: &[Value]) -> Result<(), FilterError> {
        if values.is_empty() {
            return Err(FilterError::InvalidOperand {
                field: field.0.clone(),
                reason: "membership lists must not be empty",
            });
        }
        if values.len() > FILTER_MAX_MEMBERSHIP_VALUES {
            return Err(FilterError::LimitExceeded {
                limit: "membership list size",
                max: FILTER_MAX_MEMBERSHIP_VALUES,
            });
        }
        self.membership_values = self.membership_values.saturating_add(values.len());
        if self.membership_values > FILTER_MAX_TOTAL_MEMBERSHIP_VALUES {
            return Err(FilterError::LimitExceeded {
                limit: "total membership values",
                max: FILTER_MAX_TOTAL_MEMBERSHIP_VALUES,
            });
        }
        if values
            .iter()
            .enumerate()
            .any(|(index, value)| values[index + 1..].contains(value))
        {
            return Err(FilterError::InvalidOperand {
                field: field.0.clone(),
                reason: "membership list values must be unique",
            });
        }
        let homogeneous = values.iter().all(Value::is_string)
            || values.iter().all(is_numeric_literal)
            || values.iter().all(Value::is_bool);
        if !homogeneous && !field.0.starts_with("metadata.") {
            return Err(FilterError::InvalidOperand {
                field: field.0.clone(),
                reason: "membership list values must all be strings, numbers or booleans",
            });
        }
        Ok(())
    }
}

/// Whether a literal can take part in a numeric range comparison.
///
/// Integers are JSON numbers; decimals and integers wider than `u64` are
/// exact decimal strings.
pub fn is_numeric_literal(value: &Value) -> bool {
    match value {
        Value::Number(number) => number.as_f64().is_some_and(f64::is_finite),
        Value::String(text) => is_decimal_text(text),
        _ => false,
    }
}

/// Whether `text` is a canonical decimal literal: `-?(0|[1-9][0-9]*)(\.[0-9]+)?`.
pub fn is_decimal_text(text: &str) -> bool {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (integer, fraction) = match unsigned.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (unsigned, None),
    };
    let integer_ok = integer == "0"
        || integer
            .as_bytes()
            .split_first()
            .is_some_and(|(first, rest)| {
                matches!(first, b'1'..=b'9') && rest.iter().all(u8::is_ascii_digit)
            });
    let fraction_ok = fraction.is_none_or(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    });
    integer_ok && fraction_ok
}

fn validate_rec(
    expr: &FilterExpr,
    depth: usize,
    budget: &mut FilterBudget,
) -> Result<(), FilterError> {
    budget.enter(depth)?;
    match expr {
        FilterExpr::And(list) | FilterExpr::Or(list) => {
            if list.is_empty() {
                return Err(FilterError::Malformed {
                    location: String::new(),
                    reason: format!("`{}` needs at least one operand", expr.op_name()),
                });
            }
            for nested in list {
                validate_rec(nested, depth + 1, budget)?;
            }
            Ok(())
        }
        FilterExpr::Not(inner) => validate_rec(inner, depth + 1, budget),
        FilterExpr::Eq(field, value) | FilterExpr::Ne(field, value) => {
            field.validate()?;
            validate_scalar_operand(field, value)
        }
        FilterExpr::Lt(field, value)
        | FilterExpr::Lte(field, value)
        | FilterExpr::Gt(field, value)
        | FilterExpr::Gte(field, value) => {
            field.validate()?;
            reject_inexact_numbers(field, value)?;
            if is_numeric_literal(value) || value.is_string() {
                Ok(())
            } else {
                Err(FilterError::InvalidOperand {
                    field: field.0.clone(),
                    reason: "range comparisons need a number, decimal or string literal",
                })
            }
        }
        FilterExpr::In(field, values) | FilterExpr::Nin(field, values) => {
            field.validate()?;
            values
                .iter()
                .try_for_each(|value| reject_inexact_numbers(field, value))?;
            budget.membership(field, values)
        }
        FilterExpr::Exists(field) | FilterExpr::IsNull(field) => field.validate(),
    }
}

fn reject_inexact_numbers(field: &FieldPath, value: &Value) -> Result<(), FilterError> {
    let inexact = match value {
        Value::Number(number) => {
            number.as_u64().is_none() && number.as_i64().is_none() && number.as_u128().is_none()
        }
        Value::Array(items) => {
            return items
                .iter()
                .try_for_each(|item| reject_inexact_numbers(field, item));
        }
        Value::Object(members) => {
            return members
                .values()
                .try_for_each(|member| reject_inexact_numbers(field, member));
        }
        _ => false,
    };
    if inexact {
        return Err(FilterError::InvalidOperand {
            field: field.0.clone(),
            reason: "fractional JSON numbers are not exact; write decimals as strings such as \"1.5\"",
        });
    }
    Ok(())
}

fn validate_scalar_operand(field: &FieldPath, value: &Value) -> Result<(), FilterError> {
    reject_inexact_numbers(field, value)?;
    // Structured literals are allowed only for dynamic metadata values, whose
    // shape is application-defined.
    if matches!(value, Value::Array(_) | Value::Object(_)) && !field.0.starts_with("metadata.") {
        return Err(FilterError::InvalidOperand {
            field: field.0.clone(),
            reason: "comparison literals must be strings, numbers, booleans or null",
        });
    }
    Ok(())
}

/// Location breadcrumb used only to build error messages.
#[derive(Default)]
struct Location {
    segments: Vec<LocationSegment>,
}

enum LocationSegment {
    Key(&'static str),
    Index(usize),
}

impl Location {
    fn render(&self) -> String {
        let mut out = String::new();
        for segment in &self.segments {
            match segment {
                LocationSegment::Key(key) => {
                    if !out.is_empty() {
                        out.push('.');
                    }
                    out.push_str(key);
                }
                LocationSegment::Index(index) => {
                    out.push('[');
                    out.push_str(&index.to_string());
                    out.push(']');
                }
            }
        }
        out
    }

    fn malformed(&self, reason: impl Into<String>) -> FilterError {
        FilterError::Malformed {
            location: self.render(),
            reason: reason.into(),
        }
    }
}

const OPERATORS: &str = "and, or, not, eq, ne, lt, lte, gt, gte, in, nin, exists, is_null";

fn from_value_rec(
    value: Value,
    depth: usize,
    budget: &mut FilterBudget,
    location: &mut Location,
) -> Result<FilterExpr, FilterError> {
    budget.enter(depth)?;
    let Value::Object(mut object) = value else {
        return Err(location.malformed(
            "a filter node must be an object such as {\"op\": \"eq\", \"args\": [\"field\", value]}",
        ));
    };
    let op = match object.remove("op") {
        Some(Value::String(op)) => op,
        Some(_) => return Err(location.malformed("`op` must be a string")),
        None => return Err(location.malformed("a filter node needs an `op` member")),
    };
    let args = object.remove("args").unwrap_or(Value::Null);
    if let Some(unknown) = object.keys().next() {
        return Err(location.malformed(format!(
            "unknown member `{unknown}`; a filter node has only `op` and `args`"
        )));
    }
    location.segments.push(LocationSegment::Key("args"));
    let parsed = match op.as_str() {
        "and" | "or" => {
            let Value::Array(values) = args else {
                return Err(location.malformed(format!("`{op}` takes an array of filter nodes")));
            };
            if values.is_empty() {
                return Err(location.malformed(format!("`{op}` needs at least one operand")));
            }
            if values.len() > FILTER_MAX_NODES.saturating_sub(budget.nodes) {
                return Err(FilterError::LimitExceeded {
                    limit: "node count",
                    max: FILTER_MAX_NODES,
                });
            }
            let mut out = Vec::with_capacity(values.len());
            for (index, nested) in values.into_iter().enumerate() {
                location.segments.push(LocationSegment::Index(index));
                out.push(from_value_rec(nested, depth + 1, budget, location)?);
                location.segments.pop();
            }
            // A one-operand `and`/`or` is its operand: the text form cannot
            // spell it, and both forms must decode to the same tree.
            match (out.len(), op.as_str()) {
                (1, _) => out.remove(0),
                (_, "and") => FilterExpr::And(out),
                _ => FilterExpr::Or(out),
            }
        }
        "not" => match args {
            Value::Array(mut values) if values.len() == 1 => {
                location.segments.push(LocationSegment::Index(0));
                let inner = from_value_rec(values.remove(0), depth + 1, budget, location)?;
                location.segments.pop();
                FilterExpr::Not(Box::new(inner))
            }
            _ => {
                return Err(location.malformed("`not` takes an array with exactly one filter node"));
            }
        },
        "eq" | "ne" | "lt" | "lte" | "gt" | "gte" => {
            let (field, operand) = binary_args(args, &op, location)?;
            let expr = match op.as_str() {
                "eq" => FilterExpr::Eq(field, operand),
                "ne" => FilterExpr::Ne(field, operand),
                "lt" => FilterExpr::Lt(field, operand),
                "lte" => FilterExpr::Lte(field, operand),
                "gt" => FilterExpr::Gt(field, operand),
                _ => FilterExpr::Gte(field, operand),
            };
            validate_rec_leaf(&expr)?;
            expr
        }
        "in" | "nin" => {
            let (field, operand) = binary_args(args, &op, location)?;
            let Value::Array(values) = operand else {
                return Err(location.malformed(format!("`{op}` takes [\"field\", [value, ...]]")));
            };
            field.validate()?;
            values
                .iter()
                .try_for_each(|value| reject_inexact_numbers(&field, value))?;
            budget.membership(&field, &values)?;
            if op == "in" {
                FilterExpr::In(field, values)
            } else {
                FilterExpr::Nin(field, values)
            }
        }
        "exists" | "is_null" => {
            let field = match args {
                Value::Array(mut values) if values.len() == 1 => match values.remove(0) {
                    Value::String(field) => FieldPath(field),
                    _ => return Err(location.malformed("the field must be a string")),
                },
                _ => return Err(location.malformed(format!("`{op}` takes [\"field\"]"))),
            };
            field.validate()?;
            if op == "exists" {
                FilterExpr::Exists(field)
            } else {
                FilterExpr::IsNull(field)
            }
        }
        other => {
            location.segments.pop();
            return Err(location.malformed(format!(
                "unknown operator `{other}`; expected one of: {OPERATORS}"
            )));
        }
    };
    location.segments.pop();
    Ok(parsed)
}

fn validate_rec_leaf(expr: &FilterExpr) -> Result<(), FilterError> {
    // Leaves never recurse, so a fresh budget only re-checks operand shapes.
    validate_rec(expr, 0, &mut FilterBudget::default())
}

fn binary_args(
    args: Value,
    op: &str,
    location: &Location,
) -> Result<(FieldPath, Value), FilterError> {
    match args {
        Value::Array(values) if values.len() == 2 => {
            let mut iter = values.into_iter();
            let field = iter.next().expect("length checked");
            let operand = iter.next().expect("length checked");
            match field {
                Value::String(field) => Ok((FieldPath(field), operand)),
                _ => Err(location.malformed("the first argument must be the field name")),
            }
        }
        _ => Err(location.malformed(format!("`{op}` takes [\"field\", value]"))),
    }
}

fn filter_expr_to_value(expr: &FilterExpr) -> Value {
    let args = match expr {
        FilterExpr::And(list) | FilterExpr::Or(list) => {
            Value::Array(list.iter().map(filter_expr_to_value).collect())
        }
        FilterExpr::Not(inner) => Value::Array(vec![filter_expr_to_value(inner)]),
        FilterExpr::Eq(field, operand)
        | FilterExpr::Ne(field, operand)
        | FilterExpr::Lt(field, operand)
        | FilterExpr::Lte(field, operand)
        | FilterExpr::Gt(field, operand)
        | FilterExpr::Gte(field, operand) => {
            Value::Array(vec![Value::from(field.0.clone()), operand.clone()])
        }
        FilterExpr::In(field, values) | FilterExpr::Nin(field, values) => Value::Array(vec![
            Value::from(field.0.clone()),
            Value::Array(values.clone()),
        ]),
        FilterExpr::Exists(field) | FilterExpr::IsNull(field) => {
            Value::Array(vec![Value::from(field.0.clone())])
        }
    };
    let mut map = Map::new();
    map.insert("op".into(), Value::from(expr.op_name()));
    map.insert("args".into(), args);
    Value::Object(map)
}

// `JsonSerialize` comes from norito's blanket impl over `FastJsonWrite`.
impl FastJsonWrite for FilterExpr {
    fn write_json(&self, out: &mut String) {
        filter_expr_to_value(self).json_serialize(out);
    }
}

impl JsonDeserialize for FilterExpr {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = Value::json_deserialize(parser)?;
        filter_from_json_or_text(value).map_err(|err| json::Error::Message(err.to_string()))
    }
}

/// Decode a filter given either as text (a JSON string) or as the JSON form.
///
/// # Errors
/// Returns [`FilterParseError`] carrying the text position or JSON location.
pub fn filter_from_json_or_text(value: Value) -> Result<FilterExpr, FilterParseError> {
    match value {
        Value::String(text) => FilterExpr::parse(&text).map_err(FilterParseError::Syntax),
        other => FilterExpr::from_json_value(other).map_err(FilterParseError::Structure),
    }
}

/// Error produced while decoding a filter from text or JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterParseError {
    /// The text form did not parse.
    Syntax(super::text::FilterSyntaxError),
    /// The JSON form or the parsed tree is structurally invalid.
    Structure(FilterError),
}

impl fmt::Display for FilterParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(err) => err.fmt(f),
            Self::Structure(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for FilterParseError {}

impl norito::core::SerializePayload for FieldPath {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        <String as norito::core::SerializePayload>::serialize(&self.0, writer)
    }
}

impl<'de> norito::core::DeserializePayload<'de> for FieldPath {
    fn try_deserialize(
        archived: &'de norito::core::Archived<FieldPath>,
    ) -> Result<Self, norito::core::Error> {
        let archived_str: &norito::core::Archived<String> = archived.cast();
        <String as norito::core::DeserializePayload>::try_deserialize(archived_str).map(FieldPath)
    }

    fn deserialize(archived: &'de norito::core::Archived<FieldPath>) -> Self {
        Self::try_deserialize(archived)
            .expect("FieldPath should deserialize from a valid Norito string")
    }
}

impl norito::core::SerializePayload for FilterExpr {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.validate()
            .map_err(|err| norito::core::Error::Message(format!("invalid FilterExpr: {err}")))?;
        let json = json::to_string(&filter_expr_to_value(self))
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        <String as norito::core::SerializePayload>::serialize(&json, writer)
    }
}

impl<'de> norito::core::DeserializePayload<'de> for FilterExpr {
    fn try_deserialize(
        archived: &'de norito::core::Archived<FilterExpr>,
    ) -> Result<Self, norito::core::Error> {
        let archived_str: &norito::core::Archived<String> = archived.cast();
        let raw = <String as norito::core::DeserializePayload>::try_deserialize(archived_str)?;
        let value =
            json::parse_value(&raw).map_err(|err| norito::core::Error::Message(err.to_string()))?;
        let expr = FilterExpr::from_json_value(value)
            .map_err(|err| norito::core::Error::Message(format!("invalid FilterExpr: {err}")))?;
        let canonical = json::to_string(&filter_expr_to_value(&expr))
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        if raw != canonical {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(value: Value) -> Result<FilterExpr, FilterError> {
        FilterExpr::from_json_value(value)
    }

    #[test]
    fn json_form_roundtrips() {
        let expr = FilterExpr::And(vec![
            FilterExpr::Eq("owned_by".into(), Value::from("alice")),
            FilterExpr::Gte("quantity".into(), Value::from("10.5")),
            FilterExpr::Not(Box::new(FilterExpr::IsNull("metadata.tier".into()))),
            FilterExpr::In("status".into(), vec![Value::from("A"), Value::from("B")]),
        ]);
        let value = expr.to_json_value();
        assert_eq!(decode(value).expect("roundtrip"), expr);
    }

    #[test]
    fn json_errors_name_the_offending_node() {
        let value = norito::json!({
            "op": "and",
            "args": [
                {"op": "eq", "args": ["a", 1]},
                {"op": "between", "args": ["b", 1, 2]}
            ]
        });
        let err = decode(value).expect_err("unknown operator");
        let message = err.to_string();
        assert!(message.contains("unknown operator `between`"), "{message}");
        assert!(message.contains("args[1]"), "{message}");
    }

    #[test]
    fn json_rejects_malformed_nodes() {
        for (value, needle) in [
            (
                norito::json!({"op": "eq", "args": ["a", true], "extra": 1}),
                "unknown member `extra`",
            ),
            (
                norito::json!({"op": "and", "args": []}),
                "at least one operand",
            ),
            (
                norito::json!({"op": "not", "args": []}),
                "exactly one filter node",
            ),
            (
                norito::json!({"op": "in", "args": ["a", []]}),
                "must not be empty",
            ),
            (
                norito::json!({"op": "nin", "args": ["a", [true, true]]}),
                "must be unique",
            ),
            (
                norito::json!({"op": "exists", "args": "a"}),
                "takes [\"field\"]",
            ),
            (norito::json!({"args": []}), "needs an `op`"),
            (norito::json!(["eq"]), "must be an object"),
            (
                norito::json!({"op": "eq", "args": ["a b", 1]}),
                "whitespace",
            ),
            (
                norito::json!({"op": "lt", "args": ["a", true]}),
                "range comparisons",
            ),
            (
                norito::json!({"op": "eq", "args": ["a", [1]]}),
                "comparison literals",
            ),
        ] {
            let err = decode(value).expect_err("malformed");
            assert!(
                err.to_string().contains(needle),
                "{err} should mention {needle}"
            );
        }
    }

    #[test]
    fn fractional_json_numbers_are_rejected() {
        for value in [
            norito::json!({"op": "eq", "args": ["a", 1.5]}),
            norito::json!({"op": "gte", "args": ["a", 0.25]}),
            norito::json!({"op": "in", "args": ["a", [1, 2.5]]}),
            norito::json!({"op": "eq", "args": ["metadata.x", {"a": [1, {"b": 1.5}]}]}),
        ] {
            let err = decode(value).expect_err("fractional number");
            assert!(
                err.to_string().contains("write decimals as strings"),
                "{err}"
            );
        }
        assert!(decode(norito::json!({"op": "eq", "args": ["a", "1.5"]})).is_ok());
    }

    #[test]
    fn single_operand_connectives_decode_to_their_operand() {
        for op in ["and", "or"] {
            let value = norito::json!({"op": op, "args": [{"op": "eq", "args": ["a", 1]}]});
            let expr = decode(value).expect("single operand");
            assert_eq!(expr, FilterExpr::Eq("a".into(), Value::from(1u64)));
            assert_eq!(FilterExpr::parse(&expr.to_string()).expect("text"), expr);
        }
    }

    #[test]
    fn metadata_fields_accept_structured_literals() {
        let value = norito::json!({"op": "eq", "args": ["metadata.tags", ["a", "b"]]});
        assert!(decode(value).is_ok());
    }

    #[test]
    fn limits_are_enforced() {
        let mut deep = norito::json!({"op": "eq", "args": ["a", true]});
        for _ in 0..=FILTER_MAX_DEPTH {
            deep = norito::json!({"op": "not", "args": [deep]});
        }
        assert!(matches!(
            decode(deep),
            Err(FilterError::LimitExceeded {
                limit: "nesting depth",
                ..
            })
        ));
        let wide = Value::Array(
            (0..FILTER_MAX_NODES)
                .map(|_| norito::json!({"op": "eq", "args": ["a", true]}))
                .collect(),
        );
        let mut root = Map::new();
        root.insert("op".into(), Value::from("and"));
        root.insert("args".into(), wide);
        assert!(matches!(
            decode(Value::Object(root)),
            Err(FilterError::LimitExceeded {
                limit: "node count",
                ..
            })
        ));
        let oversized: Vec<Value> = (0..=FILTER_MAX_MEMBERSHIP_VALUES as u64)
            .map(Value::from)
            .collect();
        let expr = FilterExpr::In("a".into(), oversized);
        assert!(matches!(
            expr.validate(),
            Err(FilterError::LimitExceeded {
                limit: "membership list size",
                ..
            })
        ));
    }

    #[test]
    fn decimal_text_grammar() {
        for good in [
            "0",
            "10",
            "-3",
            "10.5",
            "-0.25",
            "340282366920938463463374607431768211455",
        ] {
            assert!(is_decimal_text(good), "{good}");
        }
        for bad in ["", "-", "01", "1.", ".5", "1e3", "+1", "1.2.3", " 1"] {
            assert!(!is_decimal_text(bad), "{bad}");
        }
    }

    #[test]
    fn binary_payload_carries_canonical_json() {
        let expr = FilterExpr::Eq("result_ok".into(), Value::Bool(true));
        let bytes = norito::codec::encode_adaptive(&expr);
        let decoded: FilterExpr = norito::codec::decode_adaptive(&bytes).expect("decode");
        assert_eq!(decoded, expr);
        let canonical = json::to_string(&expr.to_json_value()).expect("canonical");
        let payload = norito::codec::encode_adaptive(&format!(" {canonical}"));
        assert!(norito::codec::decode_adaptive::<FilterExpr>(&payload).is_err());
    }
}
