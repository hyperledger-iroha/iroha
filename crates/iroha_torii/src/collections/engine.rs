//! Validation, evaluation, ordering, keyset paging and aggregation.
use super::{
    CollectionError,
    cursor::{self, CursorError, DIGEST_BYTES},
    specs::{CollectionSpec, FieldType},
};
use iroha_primitives::numeric::{Numeric, RoundingMode};
use iroha_torii_shared::list_query::{
    AggregateFn, AggregateMetric, AggregateSpec, CURSOR_MAX_BYTES, FieldPath, FilterExpr,
    ListQuery, Order, Page, is_decimal_text, sort_to_string,
};
use norito::json::{Map, Value};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, BinaryHeap},
};

/// Execution bounds for one collection read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    /// Page size when the request has no `limit`.
    pub(crate) default_limit: u32,
    /// Largest accepted `limit`.
    pub(crate) max_limit: u32,
    /// Rows one request may examine when it must sort the whole match set
    /// (custom sorts, aggregates, totals).
    pub(crate) max_scanned_rows: usize,
    /// Rows one identity-ordered page may examine before it ends early with
    /// a cursor at the last examined row.
    pub(crate) ordered_page_scan_budget: usize,
    /// Groups (and distinct values) one aggregate may hold.
    pub(crate) max_groups: usize,
}

impl Limits {
    /// Bounds derived from the app-API page limits.
    pub(crate) fn from_page_limits(default_limit: u64, max_limit: u64) -> Self {
        let clamp = |value: u64| u32::try_from(value.max(1)).unwrap_or(u32::MAX);
        let max_limit = clamp(max_limit);
        Self {
            default_limit: clamp(default_limit).min(max_limit),
            max_limit,
            max_scanned_rows: 1 << 20,
            ordered_page_scan_budget: 1 << 16,
            max_groups: 1 << 16,
        }
    }
}

/// One page of rows, before or after projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RowPage {
    /// Rows in the requested order.
    pub(crate) items: Vec<Map>,
    /// Keyset position after the last row, when more rows follow.
    pub(crate) next_cursor: Option<String>,
    /// Exact number of matching rows when requested.
    pub(crate) total: Option<u64>,
}

impl RowPage {
    /// The public page envelope.
    pub(crate) fn into_page(self) -> Page<Value> {
        Page {
            items: self.items.into_iter().map(Value::Object).collect(),
            next_cursor: self.next_cursor,
            total: self.total,
        }
    }
}

#[derive(Clone, Debug)]
struct SortField {
    name: String,
    order: Order,
    ty: FieldType,
}

/// Field resolution for filters and sorting: collection rows or aggregate rows.
enum Schema<'a> {
    Collection(&'static CollectionSpec),
    Aggregate(&'a BTreeMap<String, FieldType>),
}

impl Schema<'_> {
    fn resolve(&self, name: &str) -> Option<(FieldType, bool)> {
        match self {
            Self::Collection(spec) => spec.field(name).map(|field| (field.ty, field.sortable)),
            Self::Aggregate(outputs) => outputs.get(name).map(|ty| (*ty, true)),
        }
    }

    /// Whether `name` holds a list whose elements are compared one by one.
    fn is_list(&self, name: &str) -> bool {
        match self {
            Self::Collection(spec) => spec.field(name).is_some_and(|field| field.list),
            Self::Aggregate(_) => false,
        }
    }

    fn field_list(&self) -> String {
        match self {
            Self::Collection(spec) => spec.field_list(),
            Self::Aggregate(outputs) => outputs.keys().cloned().collect::<Vec<_>>().join(", "),
        }
    }

    fn sortable_list(&self) -> String {
        match self {
            Self::Collection(spec) => spec.sortable_list(),
            Self::Aggregate(outputs) => outputs.keys().cloned().collect::<Vec<_>>().join(", "),
        }
    }

    fn noun(&self) -> String {
        match self {
            Self::Collection(spec) => format!("`{}` rows", spec.id),
            Self::Aggregate(_) => "aggregate rows".to_owned(),
        }
    }

    fn candidates(&self) -> Vec<String> {
        match self {
            Self::Collection(spec) => {
                let mut names: Vec<String> = spec
                    .fields
                    .iter()
                    .map(|field| field.name.to_owned())
                    .collect();
                if spec.metadata {
                    names.push("metadata".to_owned());
                }
                names
            }
            Self::Aggregate(outputs) => outputs.keys().cloned().collect(),
        }
    }
}

/// Pre-parsed literal.
#[derive(Clone, Debug)]
enum Literal {
    Null,
    Bool(bool),
    Number(Numeric),
    Text(String),
    /// A metadata literal and, when it is a decimal string, its value.
    Json(Value, Option<Numeric>),
}

impl Literal {
    /// Object and array literals compare whole values, never elements.
    const fn is_structured(&self) -> bool {
        matches!(self, Self::Json(Value::Array(_) | Value::Object(_), _))
    }
}

#[derive(Clone, Debug)]
struct Path {
    full: String,
    segments: Vec<String>,
}

impl Path {
    fn new(field: &FieldPath) -> Self {
        Self {
            full: field.0.clone(),
            segments: field.0.split('.').map(str::to_owned).collect(),
        }
    }

    fn get<'r>(&self, row: &'r Map) -> Option<&'r Value> {
        if let Some(value) = row.get(&self.full) {
            return Some(value);
        }
        let (first, rest) = self.segments.split_first()?;
        let mut current = row.get(first)?;
        for segment in rest {
            current = current.as_object()?.get(segment)?;
        }
        Some(current)
    }
}

#[derive(Clone, Copy, Debug)]
enum Comparison {
    Lt,
    Lte,
    Gt,
    Gte,
}

/// A filter with resolved field types and parsed literals.
#[derive(Clone, Debug)]
enum Compiled {
    And(Vec<Compiled>),
    Or(Vec<Compiled>),
    Not(Box<Compiled>),
    Eq(Path, FieldType, Literal),
    Ne(Path, FieldType, Literal),
    Range(Path, FieldType, Comparison, Literal),
    In(Path, FieldType, Vec<Literal>),
    Nin(Path, FieldType, Vec<Literal>),
    Exists(Path),
    IsNull(Path),
}

impl Compiled {
    fn matches(&self, row: &Map) -> bool {
        match self {
            Self::And(list) => list.iter().all(|nested| nested.matches(row)),
            Self::Or(list) => list.iter().any(|nested| nested.matches(row)),
            Self::Not(inner) => !inner.matches(row),
            Self::Eq(path, ty, literal) => path
                .get(row)
                .is_some_and(|actual| literal_matches(&path.full, *ty, actual, literal)),
            Self::Ne(path, ty, literal) => path
                .get(row)
                .is_none_or(|actual| !literal_matches(&path.full, *ty, actual, literal)),
            Self::Range(path, ty, comparison, literal) => path
                .get(row)
                .is_some_and(|actual| range_matches(*ty, actual, *comparison, literal)),
            Self::In(path, ty, literals) => path.get(row).is_some_and(|actual| {
                literals
                    .iter()
                    .any(|literal| literal_matches(&path.full, *ty, actual, literal))
            }),
            Self::Nin(path, ty, literals) => path.get(row).is_none_or(|actual| {
                literals
                    .iter()
                    .all(|literal| !literal_matches(&path.full, *ty, actual, literal))
            }),
            Self::Exists(path) => path.get(row).is_some(),
            Self::IsNull(path) => path.get(row).is_none_or(Value::is_null),
        }
    }
}

/// Longest decimal text read as a number. `Numeric` holds at most a 512-bit
/// mantissa and 28 fractional digits; longer digit strings compare as text,
/// so a huge stored value cannot make every comparison parse it.
const MAX_DECIMAL_TEXT_BYTES: usize = 200;

fn numeric(value: &Value) -> Option<Numeric> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .map(Numeric::from)
            .or_else(|| number.as_i64().map(Numeric::from))
            .or_else(|| {
                number
                    .as_u128()
                    .and_then(|wide| Numeric::try_new(wide, 0).ok())
            })
            .or_else(|| {
                // Stored metadata may hold fractional JSON numbers; read them
                // through their shortest round-trip decimal spelling.
                number
                    .as_f64()
                    .filter(|float| float.is_finite())
                    .and_then(|float| decimal_numeric(&float.to_string()))
            }),
        Value::String(text) => decimal_numeric(text),
        _ => None,
    }
}

fn decimal_numeric(text: &str) -> Option<Numeric> {
    if text.len() > MAX_DECIMAL_TEXT_BYTES || !is_decimal_text(text) {
        return None;
    }
    text.parse().ok()
}

fn literal_matches(field: &str, ty: FieldType, actual: &Value, literal: &Literal) -> bool {
    // A scalar literal matches a list when any element does; object and array
    // literals compare whole values.
    if let Value::Array(items) = actual
        && !literal.is_structured()
    {
        return items
            .iter()
            .any(|item| literal_matches(field, ty, item, literal));
    }
    match literal {
        Literal::Null => actual.is_null(),
        Literal::Bool(expected) => actual.as_bool() == Some(*expected),
        Literal::Number(expected) => numeric(actual).is_some_and(|value| value == *expected),
        Literal::Text(expected) => actual.as_str().is_some_and(|value| {
            if field.ends_with("_hex") {
                value.eq_ignore_ascii_case(expected)
            } else {
                value == expected
            }
        }),
        Literal::Json(expected, expected_number) => {
            if ty == FieldType::Json
                && let (Some(left), Some(right)) = (numeric(actual), expected_number)
            {
                return left == *right;
            }
            actual == expected
        }
    }
}

fn range_matches(ty: FieldType, actual: &Value, comparison: Comparison, literal: &Literal) -> bool {
    let ordering = match (ty, literal) {
        (_, Literal::Number(expected)) => match numeric(actual) {
            Some(value) => value.cmp(expected),
            None => return false,
        },
        (_, Literal::Text(expected)) => match actual.as_str() {
            Some(value) => value.cmp(expected.as_str()),
            None => return false,
        },
        _ => return false,
    };
    match comparison {
        Comparison::Lt => ordering == Ordering::Less,
        Comparison::Lte => ordering != Ordering::Greater,
        Comparison::Gt => ordering == Ordering::Greater,
        Comparison::Gte => ordering != Ordering::Less,
    }
}

fn suggestion(name: &str, candidates: &[String]) -> Option<String> {
    fn distance(left: &str, right: &str) -> usize {
        let right: Vec<char> = right.chars().collect();
        let mut previous: Vec<usize> = (0..=right.len()).collect();
        for (i, lc) in left.chars().enumerate() {
            let mut current = vec![i + 1];
            for (j, rc) in right.iter().enumerate() {
                let cost = usize::from(lc != *rc);
                current.push(
                    (previous[j] + cost)
                        .min(previous[j + 1] + 1)
                        .min(current[j] + 1),
                );
            }
            previous = current;
        }
        previous[right.len()]
    }
    candidates
        .iter()
        .map(|candidate| (distance(name, candidate), candidate))
        .filter(|(distance, candidate)| *distance <= 2.max(candidate.len() / 4))
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate.clone())
}

fn unknown_field(control: &'static str, schema: &Schema<'_>, name: &str) -> CollectionError {
    let code = match control {
        "sort" => "invalid_sort",
        "select" => "invalid_select",
        "aggregate" => "invalid_aggregate",
        _ => "invalid_filter",
    };
    let mut err = CollectionError::new(
        code,
        control,
        format!("`{name}` is not a field of {}", schema.noun()),
    )
    .with_actual(name)
    .with_expected(schema.field_list());
    if let Some(candidate) = suggestion(name, &schema.candidates()) {
        err = err.with_hint(format!("did you mean `{candidate}`?"));
    } else if let Schema::Collection(spec) = schema
        && spec.metadata
        && !name.starts_with("metadata")
    {
        err = err.with_hint(format!(
            "metadata entries are addressed as `metadata.{name}`"
        ));
    }
    err
}

fn literal_for(
    control: &'static str,
    field: &FieldPath,
    ty: FieldType,
    value: &Value,
) -> Result<Literal, CollectionError> {
    let mismatch = || {
        CollectionError::new(
            if control == "filter" {
                "invalid_filter"
            } else {
                "invalid_aggregate"
            },
            control,
            format!(
                "`{}` holds {} values, so it cannot be compared with {}",
                field.0,
                ty.label(),
                norito::json::to_json(value).unwrap_or_default()
            ),
        )
        .with_actual(field.0.clone())
        .with_expected(ty.label())
    };
    if value.is_null() {
        return Ok(Literal::Null);
    }
    match ty {
        FieldType::String => value
            .as_str()
            .map(|text| Literal::Text(text.to_owned()))
            .ok_or_else(mismatch),
        FieldType::Number => numeric(value).map(Literal::Number).ok_or_else(mismatch),
        FieldType::Bool => value.as_bool().map(Literal::Bool).ok_or_else(mismatch),
        FieldType::Json => Ok(match numeric(value) {
            Some(number) if !value.is_string() => Literal::Number(number),
            number => Literal::Json(value.clone(), number),
        }),
    }
}

fn compile_filter(
    control: &'static str,
    schema: &Schema<'_>,
    expr: &FilterExpr,
) -> Result<Compiled, CollectionError> {
    let resolve = |field: &FieldPath| {
        schema
            .resolve(&field.0)
            .map(|(ty, _)| ty)
            .ok_or_else(|| unknown_field(control, schema, &field.0))
    };
    let leaf_literal =
        |field: &FieldPath, ty, value: &Value| literal_for(control, field, ty, value);
    Ok(match expr {
        FilterExpr::And(list) => Compiled::And(
            list.iter()
                .map(|nested| compile_filter(control, schema, nested))
                .collect::<Result<_, _>>()?,
        ),
        FilterExpr::Or(list) => Compiled::Or(
            list.iter()
                .map(|nested| compile_filter(control, schema, nested))
                .collect::<Result<_, _>>()?,
        ),
        FilterExpr::Not(inner) => Compiled::Not(Box::new(compile_filter(control, schema, inner)?)),
        FilterExpr::Eq(field, value) => {
            let ty = resolve(field)?;
            Compiled::Eq(Path::new(field), ty, leaf_literal(field, ty, value)?)
        }
        FilterExpr::Ne(field, value) => {
            let ty = resolve(field)?;
            Compiled::Ne(Path::new(field), ty, leaf_literal(field, ty, value)?)
        }
        FilterExpr::Lt(field, value)
        | FilterExpr::Lte(field, value)
        | FilterExpr::Gt(field, value)
        | FilterExpr::Gte(field, value) => {
            let ty = resolve(field)?;
            let code = if control == "filter" {
                "invalid_filter"
            } else {
                "invalid_aggregate"
            };
            if schema.is_list(&field.0) {
                return Err(CollectionError::new(
                    code,
                    control,
                    format!("`{}` holds a list, which cannot be range-compared", field.0),
                )
                .with_actual(field.0.clone())
                .with_hint("match list elements with `=` or `in [..]`"));
            }
            let comparison = match expr {
                FilterExpr::Lt(..) => Comparison::Lt,
                FilterExpr::Lte(..) => Comparison::Lte,
                FilterExpr::Gt(..) => Comparison::Gt,
                _ => Comparison::Gte,
            };
            let literal = match ty {
                FieldType::Number | FieldType::Json => numeric(value).map(Literal::Number),
                FieldType::String => value.as_str().map(|text| Literal::Text(text.to_owned())),
                FieldType::Bool => None,
            }
            .ok_or_else(|| {
                CollectionError::new(
                    code,
                    control,
                    format!(
                        "`{}` holds {} values; range comparisons need a {} literal",
                        field.0,
                        ty.label(),
                        if ty == FieldType::String {
                            "string"
                        } else {
                            "number"
                        }
                    ),
                )
                .with_actual(field.0.clone())
            })?;
            Compiled::Range(Path::new(field), ty, comparison, literal)
        }
        FilterExpr::In(field, values) | FilterExpr::Nin(field, values) => {
            let ty = resolve(field)?;
            let literals = values
                .iter()
                .map(|value| leaf_literal(field, ty, value))
                .collect::<Result<Vec<_>, _>>()?;
            if matches!(expr, FilterExpr::In(..)) {
                Compiled::In(Path::new(field), ty, literals)
            } else {
                Compiled::Nin(Path::new(field), ty, literals)
            }
        }
        FilterExpr::Exists(field) => {
            resolve(field)?;
            Compiled::Exists(Path::new(field))
        }
        FilterExpr::IsNull(field) => {
            resolve(field)?;
            Compiled::IsNull(Path::new(field))
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SortValue {
    Null,
    Bool(bool),
    Number(Numeric),
    Text(String),
    Other(String),
}

impl SortValue {
    fn from_value(value: &Value, ty: FieldType) -> Self {
        if value.is_null() {
            return Self::Null;
        }
        match ty {
            FieldType::Number => numeric(value).map_or_else(|| Self::other(value), Self::Number),
            FieldType::String => value
                .as_str()
                .map_or_else(|| Self::other(value), |text| Self::Text(text.to_owned())),
            FieldType::Bool => value
                .as_bool()
                .map_or_else(|| Self::other(value), Self::Bool),
            FieldType::Json => {
                if let Some(flag) = value.as_bool() {
                    Self::Bool(flag)
                } else if let Some(number) = numeric(value) {
                    Self::Number(number)
                } else if let Some(text) = value.as_str() {
                    Self::Text(text.to_owned())
                } else {
                    Self::other(value)
                }
            }
        }
    }

    fn other(value: &Value) -> Self {
        Self::Other(norito::json::to_json(value).unwrap_or_default())
    }

    const fn rank(&self) -> u8 {
        match self {
            Self::Null => 0,
            Self::Bool(_) => 1,
            Self::Number(_) => 2,
            Self::Text(_) => 3,
            Self::Other(_) => 4,
        }
    }
}

impl Ord for SortValue {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Bool(left), Self::Bool(right)) => left.cmp(right),
            (Self::Number(left), Self::Number(right)) => left.cmp(right),
            (Self::Text(left), Self::Text(right)) | (Self::Other(left), Self::Other(right)) => {
                left.cmp(right)
            }
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

impl PartialOrd for SortValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A row's position in the requested order.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RowKey {
    values: Vec<SortValue>,
    orders: Vec<Order>,
}

impl Ord for RowKey {
    fn cmp(&self, other: &Self) -> Ordering {
        for ((left, right), order) in self.values.iter().zip(&other.values).zip(&self.orders) {
            let ordering = left.cmp(right);
            if ordering != Ordering::Equal {
                return if order.is_ascending() {
                    ordering
                } else {
                    ordering.reverse()
                };
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for RowKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct Entry {
    key: RowKey,
    seq: u64,
    row: Map,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Entry {}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key).then(self.seq.cmp(&other.seq))
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Aggregate plan with resolved output columns.
struct AggregatePlan {
    spec: AggregateSpec,
    outputs: BTreeMap<String, FieldType>,
    /// Type of each metric's field (`Json` for `count`), in metric order.
    metric_types: Vec<FieldType>,
    having: Option<Compiled>,
}

/// A validated query bound to one collection.
pub(crate) struct Prepared<'q> {
    spec: &'static CollectionSpec,
    query: &'q ListQuery,
    filter: Option<Compiled>,
    aggregate: Option<AggregatePlan>,
    sort: Vec<SortField>,
    limit: usize,
    after: Option<RowKey>,
    after_position: Option<Vec<u64>>,
    /// Identity-ordered read: descending or not.
    ordered: Option<bool>,
    /// The cursor's `id` for an identity-ordered read.
    after_id: Option<String>,
    digest: [u8; DIGEST_BYTES],
}

/// Where an identity-ordered page starts: strictly after `after` in
/// canonical identifier order (strictly before it when `descending`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OrderedScan<'a> {
    /// The previous page's last examined `id`.
    pub(crate) after: Option<&'a str>,
    /// Whether rows are read in descending identifier order.
    pub(crate) descending: bool,
}

/// Block coordinates `(block_height, block_index)` of a positioned row.
pub(crate) type Position = (u64, u64);

/// Row fields holding a positioned row's coordinates.
const POSITION_FIELDS: [&str; 2] = ["block_height", "block_index"];

/// Validate `query` against `spec` and bind its cursor to the collection,
/// its `scope` (the path's account or asset definition, empty for top-level
/// collections), the filter, the sort and the aggregate.
///
/// # Errors
/// Returns the first rejected control.
pub(crate) fn prepare<'q>(
    spec: &'static CollectionSpec,
    scope: &str,
    query: &'q ListQuery,
    limits: &Limits,
) -> Result<Prepared<'q>, CollectionError> {
    query.validate()?;
    let collection = Schema::Collection(spec);
    let filter = query
        .filter
        .as_ref()
        .map(|expr| compile_filter("filter", &collection, expr))
        .transpose()?;
    if let Some(select) = &query.select {
        for field in select {
            if collection.resolve(&field.0).is_none() {
                return Err(unknown_field("select", &collection, &field.0));
            }
        }
    }
    if spec.positioned > 0 {
        reject_unpositioned_controls(spec, query)?;
    }
    let aggregate = query
        .aggregate
        .as_ref()
        .map(|aggregate| plan_aggregate(spec, aggregate))
        .transpose()?;
    let sort = effective_sort(spec, query, aggregate.as_ref())?;
    let limit = query.limit.unwrap_or(limits.default_limit);
    if limit == 0 || limit > limits.max_limit {
        return Err(CollectionError::new(
            "invalid_limit",
            "limit",
            format!("`limit` must be between 1 and {}", limits.max_limit),
        )
        .with_actual(limit.to_string())
        .with_expected(format!("1..={}", limits.max_limit)));
    }
    let filter_text = query
        .filter
        .as_ref()
        .map(|filter| norito::json::to_json(&filter.to_json_value()).unwrap_or_default())
        .unwrap_or_default();
    let aggregate_text = query
        .aggregate
        .as_ref()
        .map(|aggregate| norito::json::to_json(aggregate).unwrap_or_default())
        .unwrap_or_default();
    let digest = cursor::digest(&[
        spec.id,
        scope,
        &filter_text,
        &sort_to_string(&query.sort),
        &aggregate_text,
    ]);
    let ordered = (spec.ordered && aggregate.is_none())
        .then(|| match query.sort.as_slice() {
            [] => Some(false),
            [key] if key.key.0 == "id" => Some(!key.order.is_ascending()),
            _ => None,
        })
        .flatten();
    let mut after_id = None;
    let (after, after_position) = match query.cursor.as_deref() {
        None => (None, None),
        Some(token) if spec.positioned > 0 => (
            None,
            Some(decode_position(spec, &digest, token).map_err(cursor_error)?),
        ),
        Some(token) => {
            let values =
                cursor::decode(token, spec.tag, &digest, sort.len()).map_err(cursor_error)?;
            if ordered.is_some() {
                after_id = Some(
                    values[0]
                        .as_str()
                        .ok_or_else(|| cursor_error(CursorError::Malformed))?
                        .to_owned(),
                );
            }
            (Some(key_from_values(&sort, &values)), None)
        }
    };
    Ok(Prepared {
        spec,
        query,
        filter,
        aggregate,
        sort,
        limit: usize::try_from(limit).unwrap_or(usize::MAX),
        after,
        after_position,
        ordered,
        after_id,
        digest,
    })
}

fn decode_position(
    spec: &CollectionSpec,
    digest: &[u8; DIGEST_BYTES],
    token: &str,
) -> Result<Vec<u64>, CursorError> {
    let position: Vec<u64> = cursor::decode(token, spec.tag, digest, spec.positioned)?
        .iter()
        .map(|value| value.as_u64().ok_or(CursorError::Malformed))
        .collect::<Result<_, _>>()?;
    if position.first() == Some(&0) {
        return Err(CursorError::Malformed);
    }
    Ok(position)
}

fn narrow_height_range(expr: &FilterExpr, range: &mut (u64, u64)) {
    let bound = |field: &FieldPath, value: &Value| {
        (field.0 == POSITION_FIELDS[0])
            .then(|| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
            .flatten()
    };
    match expr {
        FilterExpr::And(list) => list
            .iter()
            .for_each(|nested| narrow_height_range(nested, range)),
        FilterExpr::Eq(field, value) => {
            if let Some(height) = bound(field, value) {
                range.0 = range.0.max(height);
                range.1 = range.1.min(height);
            }
        }
        FilterExpr::Gte(field, value) => {
            if let Some(height) = bound(field, value) {
                range.0 = range.0.max(height);
            }
        }
        FilterExpr::Gt(field, value) => {
            if let Some(height) = bound(field, value) {
                range.0 = range.0.max(height.saturating_add(1));
            }
        }
        FilterExpr::Lte(field, value) => {
            if let Some(height) = bound(field, value) {
                range.1 = range.1.min(height);
            }
        }
        FilterExpr::Lt(field, value) => {
            if let Some(height) = bound(field, value) {
                range.1 = range.1.min(height.saturating_sub(1));
            }
        }
        _ => {}
    }
}

fn reject_unpositioned_controls(
    spec: &CollectionSpec,
    query: &ListQuery,
) -> Result<(), CollectionError> {
    if !query.sort.is_empty() {
        return Err(CollectionError::new(
            "invalid_sort",
            "sort",
            format!(
                "`{}` rows are returned newest first and cannot be re-sorted",
                spec.id
            ),
        )
        .with_hint("omit `sort`; filter on `block_height` or `timestamp_ms` to select a range"));
    }
    if query.include_total {
        return Err(CollectionError::new(
            "invalid_include_total",
            "include_total",
            format!(
                "totals are not available for `{}`: counting would scan the whole history",
                spec.id
            ),
        ));
    }
    if query.aggregate.is_some() {
        return Err(CollectionError::new(
            "invalid_aggregate",
            "aggregate",
            format!(
                "aggregates are not available for `{}`: they would scan the whole history",
                spec.id
            ),
        ));
    }
    Ok(())
}

fn cursor_error(err: CursorError) -> CollectionError {
    CollectionError::new("invalid_cursor", "cursor", err.message())
}

fn effective_sort(
    spec: &'static CollectionSpec,
    query: &ListQuery,
    aggregate: Option<&AggregatePlan>,
) -> Result<Vec<SortField>, CollectionError> {
    let schema = match aggregate {
        Some(plan) => Schema::Aggregate(&plan.outputs),
        None => Schema::Collection(spec),
    };
    let mut fields = Vec::new();
    for key in &query.sort {
        let Some((ty, sortable)) = schema.resolve(&key.key.0) else {
            return Err(unknown_field("sort", &schema, &key.key.0));
        };
        if !sortable {
            return Err(CollectionError::new(
                "invalid_sort",
                "sort",
                format!("`{}` cannot be sorted on", key.key.0),
            )
            .with_actual(key.key.0.clone())
            .with_expected(schema.sortable_list()));
        }
        fields.push(SortField {
            name: key.key.0.clone(),
            order: key.order,
            ty,
        });
    }
    let (defaults, identity): (Vec<(String, Order)>, Vec<String>) = match aggregate {
        Some(plan) => {
            let groups: Vec<String> = plan
                .spec
                .group_by
                .iter()
                .map(|field| field.0.clone())
                .collect();
            (
                groups
                    .iter()
                    .map(|name| (name.clone(), Order::Asc))
                    .collect(),
                groups,
            )
        }
        None => (
            spec.default_sort
                .iter()
                .map(|(name, order)| ((*name).to_owned(), *order))
                .collect(),
            spec.identity
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        ),
    };
    if fields.is_empty() {
        for (name, order) in defaults {
            let ty = schema.resolve(&name).map_or(FieldType::Json, |(ty, _)| ty);
            fields.push(SortField { name, order, ty });
        }
    }
    for name in identity {
        if !fields.iter().any(|field| field.name == name) {
            let ty = schema.resolve(&name).map_or(FieldType::Json, |(ty, _)| ty);
            fields.push(SortField {
                name,
                order: Order::Asc,
                ty,
            });
        }
    }
    Ok(fields)
}

fn plan_aggregate(
    spec: &'static CollectionSpec,
    aggregate: &AggregateSpec,
) -> Result<AggregatePlan, CollectionError> {
    let collection = Schema::Collection(spec);
    let invalid = |message: String| CollectionError::new("invalid_aggregate", "aggregate", message);
    let mut outputs = BTreeMap::new();
    let mut metric_types = Vec::with_capacity(aggregate.metrics.len());
    for group in &aggregate.group_by {
        let Some((ty, _)) = collection.resolve(&group.0) else {
            return Err(unknown_field("aggregate", &collection, &group.0));
        };
        if !ty.is_scalar() && !group.0.starts_with("metadata.") {
            return Err(invalid(format!(
                "`group_by` field `{}` is not a scalar",
                group.0
            )));
        }
        if outputs.insert(group.0.clone(), ty).is_some() {
            return Err(invalid(format!("`group_by` lists `{}` twice", group.0)));
        }
    }
    for metric in &aggregate.metrics {
        let alias_ok = metric
            .alias
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
            && metric
                .alias
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        if !alias_ok {
            return Err(invalid(format!(
                "metric alias `{}` must be an identifier such as `total_supply`",
                metric.alias
            )));
        }
        // Group values are written at their paths; an alias naming the first
        // segment of one would overwrite it.
        if let Some(group) = aggregate.group_by.iter().find(|group| {
            group
                .0
                .split('.')
                .next()
                .is_some_and(|segment| segment == metric.alias)
                && group.0 != metric.alias
        }) {
            return Err(invalid(format!(
                "metric alias `{}` collides with the group field `{}`",
                metric.alias, group.0
            )));
        }
        match (metric.r#fn, &metric.field) {
            (AggregateFn::Count, None) => metric_types.push(FieldType::Json),
            (AggregateFn::Count, Some(_)) => {
                return Err(invalid(
                    "`count` counts rows and takes no `field`".to_owned(),
                ));
            }
            (_, None) => {
                return Err(invalid(format!(
                    "`{}` needs a `field`",
                    metric.r#fn.as_str()
                )));
            }
            (function, Some(field)) => {
                let Some((ty, _)) = collection.resolve(&field.0) else {
                    return Err(unknown_field("aggregate", &collection, &field.0));
                };
                let numeric_fn = !matches!(function, AggregateFn::DistinctCount);
                if numeric_fn && !matches!(ty, FieldType::Number | FieldType::Json) {
                    return Err(invalid(format!(
                        "`{}` needs a numeric field; `{}` holds {} values",
                        function.as_str(),
                        field.0,
                        ty.label()
                    )));
                }
                metric_types.push(ty);
            }
        }
        if outputs
            .insert(metric.alias.clone(), FieldType::Number)
            .is_some()
        {
            return Err(invalid(format!(
                "output column `{}` is defined twice",
                metric.alias
            )));
        }
    }
    let having = aggregate
        .having
        .as_ref()
        .map(|expr| compile_filter("aggregate", &Schema::Aggregate(&outputs), expr))
        .transpose()?;
    Ok(AggregatePlan {
        spec: aggregate.clone(),
        outputs,
        metric_types,
        having,
    })
}

fn key_from_values(sort: &[SortField], values: &[Value]) -> RowKey {
    RowKey {
        values: sort
            .iter()
            .zip(values)
            .map(|(field, value)| SortValue::from_value(value, field.ty))
            .collect(),
        orders: sort.iter().map(|field| field.order).collect(),
    }
}

fn field_value<'r>(row: &'r Map, name: &str) -> Option<&'r Value> {
    if let Some(value) = row.get(name) {
        return Some(value);
    }
    let mut segments = name.split('.');
    let mut current = row.get(segments.next()?)?;
    for segment in segments {
        current = current.as_object()?.get(segment)?;
    }
    Some(current)
}

fn insert_path(target: &mut Map, name: &str, value: Value) {
    let mut segments = name.split('.').peekable();
    let mut current = target;
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            current.insert(segment.to_owned(), value);
            return;
        }
        let slot = current
            .entry(segment.to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        if !slot.is_object() {
            *slot = Value::Object(Map::new());
        }
        let Value::Object(next) = slot else {
            unreachable!("slot was just made an object")
        };
        current = next;
    }
}

impl Prepared<'_> {
    /// Rows per page.
    pub(crate) fn limit(&self) -> usize {
        self.limit
    }

    /// Coordinates a positioned read resumes strictly before.
    pub(crate) fn resume_position(&self) -> Option<Position> {
        self.after_position
            .as_ref()
            .map(|position| (position[0], position[1]))
    }

    /// Movement coordinates a multi-row transaction read resumes strictly before.
    pub(crate) fn resume_movement_position(&self) -> Option<(u64, u64, u64)> {
        self.after_position
            .as_ref()
            .map(|position| (position[0], position[1], position[2]))
    }

    /// Encode the last examined account movement without skipping its transaction's remaining rows.
    pub(crate) fn movement_page(
        &self,
        items: Vec<Map>,
        resume: Option<(u64, u64, u64)>,
    ) -> RowPage {
        RowPage {
            items,
            next_cursor: resume.map(|(height, index, movement)| {
                cursor::encode(
                    self.spec.tag,
                    &self.digest,
                    &[
                        Value::from(height),
                        Value::from(index),
                        Value::from(movement),
                    ],
                )
            }),
            total: None,
        }
    }

    /// Inclusive `block_height` range implied by the filter's top-level
    /// conjuncts; history walks start at its top and stop below its bottom.
    pub(crate) fn height_range(&self) -> (u64, u64) {
        let mut range = (0, u64::MAX);
        if let Some(filter) = &self.query.filter {
            narrow_height_range(filter, &mut range);
        }
        range
    }

    /// The cursor after `row`.
    ///
    /// # Errors
    /// Rejects sort values too large to fit in a cursor: issuing one would
    /// only make the next request fail.
    fn cursor_after(&self, row: &Map) -> Result<String, CollectionError> {
        self.cursor_from_values(&self.key_values(row))
    }

    /// Encode a cursor from sort values, refusing ones too large to accept.
    fn cursor_from_values(&self, values: &[Value]) -> Result<String, CollectionError> {
        let token = cursor::encode(self.spec.tag, &self.digest, values);
        if token.len() > CURSOR_MAX_BYTES {
            return Err(CollectionError::new(
                "invalid_sort",
                "sort",
                format!(
                    "a row's sort values need a {}-byte cursor, more than the {CURSOR_MAX_BYTES} bytes a cursor may hold",
                    token.len()
                ),
            )
            .with_hint("sort by fields with shorter values"));
        }
        Ok(token)
    }

    /// A page of a positioned read; `resume` is where the next page starts
    /// (exclusive), `None` once history is exhausted.
    pub(crate) fn positioned_page(&self, items: Vec<Map>, resume: Option<Position>) -> RowPage {
        RowPage {
            items,
            next_cursor: resume.map(|(height, index)| {
                cursor::encode(
                    self.spec.tag,
                    &self.digest,
                    &[Value::from(height), Value::from(index)],
                )
            }),
            total: None,
        }
    }

    /// Whether `row` passes the filter.
    pub(crate) fn matches(&self, row: &Map) -> bool {
        self.filter
            .as_ref()
            .is_none_or(|filter| filter.matches(row))
    }

    fn key_values(&self, row: &Map) -> Vec<Value> {
        self.sort
            .iter()
            .map(|field| {
                field_value(row, &field.name)
                    .cloned()
                    .unwrap_or(Value::Null)
            })
            .collect()
    }

    fn key_of(&self, row: &Map) -> RowKey {
        RowKey {
            values: self
                .sort
                .iter()
                .map(|field| {
                    field_value(row, &field.name).map_or(SortValue::Null, |value| {
                        SortValue::from_value(value, field.ty)
                    })
                })
                .collect(),
            orders: self.sort.iter().map(|field| field.order).collect(),
        }
    }

    fn after_cursor(&self, key: &RowKey) -> bool {
        self.after
            .as_ref()
            .is_none_or(|after| key.cmp(after) == Ordering::Greater)
    }

    /// The seek of an identity-ordered read, when this read is one: the
    /// producer then streams rows in canonical identifier order from the
    /// cursor into [`Self::execute_ordered`].
    pub(crate) fn ordered_scan(&self) -> Option<OrderedScan<'_>> {
        self.ordered.map(|descending| OrderedScan {
            after: self.after_id.as_deref(),
            descending,
        })
    }

    /// Execute an identity-ordered read over storage entries in the requested
    /// order. Each entry carries whether its key is past the cursor and its
    /// visible row, if any. Hidden entries still consume the scan budget.
    ///
    /// The page stops after `limit` matches, or once it has examined
    /// [`Limits::ordered_page_scan_budget`] rows; a short page then carries a
    /// cursor at the last examined visible row, so sparse matches stay reachable
    /// page by page without revealing hidden keys. Totals require entries from
    /// the start of the collection, including those before the cursor, within
    /// [`Limits::max_scanned_rows`].
    ///
    /// # Errors
    /// Fails when a total exceeds the scan limit, no visible continuation can
    /// be established within the page budget, or a visible row lacks its `id`.
    pub(crate) fn execute_ordered<I>(
        &self,
        rows: I,
        limits: &Limits,
    ) -> Result<RowPage, CollectionError>
    where
        I: IntoIterator<Item = (bool, Option<Map>)>,
    {
        debug_assert!(self.ordered.is_some(), "only identity-ordered reads stream");
        let mut items = Vec::new();
        let mut scanned = 0usize;
        let mut total = 0u64;
        let mut has_more = false;
        let mut last_examined = None;
        let mut stopped_early = false;
        for (after_cursor, row) in rows {
            scanned += 1;
            if self.query.include_total && scanned > limits.max_scanned_rows {
                return Err(CollectionError::new(
                    "query_scan_limit_exceeded",
                    "include_total",
                    format!(
                        "counting would examine more than {} rows; add a more selective filter or omit `include_total`",
                        limits.max_scanned_rows
                    ),
                ));
            }
            if let Some(row) = row {
                let id = row
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        CollectionError::new("invalid_query", "query", "a row has no `id`")
                    })?
                    .to_owned();
                if self.matches(&row) {
                    total += 1;
                    if after_cursor {
                        if items.len() < self.limit {
                            items.push(row);
                        } else {
                            has_more = true;
                            if !self.query.include_total {
                                break;
                            }
                        }
                    }
                }
                if after_cursor {
                    last_examined = Some(id);
                }
            }
            if !self.query.include_total && scanned >= limits.ordered_page_scan_budget {
                if last_examined.is_none() {
                    return Err(CollectionError::new(
                        "query_scan_limit_exceeded",
                        "filter",
                        "the page scan budget was exhausted before a visible continuation; narrow the query",
                    ));
                }
                stopped_early = true;
                break;
            }
        }
        let resume_id = if has_more {
            items
                .last()
                .and_then(|row| row.get("id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        } else if stopped_early {
            last_examined
        } else {
            None
        };
        let next_cursor = resume_id
            .map(|id| self.cursor_from_values(&[Value::from(id)]))
            .transpose()?;
        Ok(RowPage {
            items,
            next_cursor,
            total: self.query.include_total.then_some(total),
        })
    }

    /// Execute against the collection's candidate rows (unordered).
    ///
    /// # Errors
    /// Fails when the scan or aggregate budgets are exceeded.
    pub(crate) fn execute<I>(&self, rows: I, limits: &Limits) -> Result<RowPage, CollectionError>
    where
        I: IntoIterator<Item = Map>,
    {
        self.execute_entries(rows.into_iter().map(Some), limits)
    }

    /// Execute an unordered storage scan, charging skipped and hidden entries
    /// to the same budget as visible rows.
    pub(crate) fn execute_entries<I>(
        &self,
        rows: I,
        limits: &Limits,
    ) -> Result<RowPage, CollectionError>
    where
        I: IntoIterator<Item = Option<Map>>,
    {
        debug_assert!(
            self.spec.positioned == 0,
            "positioned rows are paged by their producer"
        );
        let mut scanned = 0usize;
        let mut admit = |row: &Option<Map>| -> Result<bool, CollectionError> {
            scanned += 1;
            if scanned > limits.max_scanned_rows {
                return Err(CollectionError::new(
                    "query_scan_limit_exceeded",
                    "filter",
                    format!(
                        "the query would examine more than {} rows; add a more selective filter",
                        limits.max_scanned_rows
                    ),
                ));
            }
            Ok(row.as_ref().is_some_and(|row| self.matches(row)))
        };
        match &self.aggregate {
            None => {
                let mut kept = BinaryHeap::with_capacity(self.limit.saturating_add(2));
                let mut total = 0u64;
                let mut seq = 0u64;
                for row in rows {
                    if !admit(&row)? {
                        continue;
                    }
                    if let Some(row) = row {
                        total += 1;
                        self.keep(&mut kept, &mut seq, row);
                    }
                }
                self.finish(kept, self.query.include_total.then_some(total))
            }
            Some(plan) => {
                let mut groups = Vec::new();
                let grouped = aggregate_rows(
                    plan,
                    rows.into_iter()
                        .map(|row| admit(&row).map(|keep| if keep { row } else { None })),
                    limits.max_groups,
                    &mut groups,
                );
                grouped?;
                let mut kept = BinaryHeap::with_capacity(self.limit.saturating_add(2));
                let mut total = 0u64;
                let mut seq = 0u64;
                for row in groups {
                    if plan
                        .having
                        .as_ref()
                        .is_some_and(|having| !having.matches(&row))
                    {
                        continue;
                    }
                    total += 1;
                    self.keep(&mut kept, &mut seq, row);
                }
                self.finish(kept, self.query.include_total.then_some(total))
            }
        }
    }

    fn keep(&self, kept: &mut BinaryHeap<Entry>, seq: &mut u64, row: Map) {
        let key = self.key_of(&row);
        if !self.after_cursor(&key) {
            return;
        }
        if kept.len() > self.limit
            && kept
                .peek()
                .is_some_and(|largest| key.cmp(&largest.key) != Ordering::Less)
        {
            return;
        }
        kept.push(Entry {
            key,
            seq: *seq,
            row,
        });
        *seq += 1;
        if kept.len() > self.limit + 1 {
            kept.pop();
        }
    }

    fn finish(
        &self,
        kept: BinaryHeap<Entry>,
        total: Option<u64>,
    ) -> Result<RowPage, CollectionError> {
        let mut entries = kept.into_sorted_vec();
        let has_more = entries.len() > self.limit;
        entries.truncate(self.limit);
        let next_cursor = has_more
            .then(|| entries.last())
            .flatten()
            .map(|last| self.cursor_after(&last.row))
            .transpose()?;
        Ok(RowPage {
            items: entries.into_iter().map(|entry| entry.row).collect(),
            next_cursor,
            total,
        })
    }

    /// Apply `select` to a page of full rows.
    pub(crate) fn project(&self, mut page: RowPage) -> RowPage {
        if let Some(select) = &self.query.select {
            page.items = page
                .items
                .into_iter()
                .map(|row| {
                    let mut projected = Map::new();
                    for field in select {
                        let value = field_value(&row, &field.0).cloned().unwrap_or(Value::Null);
                        insert_path(&mut projected, &field.0, value);
                    }
                    projected
                })
                .collect();
        }
        page
    }
}

#[derive(Debug)]
enum MetricState {
    Count(u64),
    DistinctCount(BTreeSet<String>, FieldType),
    Sum(Option<Numeric>),
    Min(Option<Numeric>),
    Max(Option<Numeric>),
    Avg { sum: Option<Numeric>, count: u64 },
}

impl MetricState {
    const fn new(metric: &AggregateMetric, ty: FieldType) -> Self {
        match metric.r#fn {
            AggregateFn::Count => Self::Count(0),
            AggregateFn::DistinctCount => Self::DistinctCount(BTreeSet::new(), ty),
            AggregateFn::Sum => Self::Sum(None),
            AggregateFn::Min => Self::Min(None),
            AggregateFn::Max => Self::Max(None),
            AggregateFn::Avg => Self::Avg {
                sum: None,
                count: 0,
            },
        }
    }

    fn update(
        &mut self,
        row: &Map,
        metric: &AggregateMetric,
        max_distinct: usize,
    ) -> Result<(), CollectionError> {
        let value = metric
            .field
            .as_ref()
            .and_then(|field| field_value(row, &field.0));
        let overflow = || {
            CollectionError::new(
                "invalid_aggregate",
                "aggregate",
                format!("`{}` overflowed the numeric range", metric.alias),
            )
        };
        match self {
            Self::Count(total) => *total += 1,
            Self::DistinctCount(values, ty) => {
                if let Some(value) = value {
                    let key = group_key_text(value, *ty);
                    if !values.contains(&key) && values.len() >= max_distinct {
                        return Err(CollectionError::new(
                            "query_scan_limit_exceeded",
                            "aggregate",
                            format!(
                                "`{}` would track more than {max_distinct} distinct values",
                                metric.alias
                            ),
                        ));
                    }
                    values.insert(key);
                }
            }
            Self::Sum(total) => {
                if let Some(value) = value.and_then(numeric) {
                    *total = Some(match total.take() {
                        Some(existing) => existing.checked_add(value).ok_or_else(overflow)?,
                        None => value,
                    });
                }
            }
            Self::Min(current) => {
                if let Some(value) = value.and_then(numeric)
                    && current.as_ref().is_none_or(|existing| value < *existing)
                {
                    *current = Some(value);
                }
            }
            Self::Max(current) => {
                if let Some(value) = value.and_then(numeric)
                    && current.as_ref().is_none_or(|existing| value > *existing)
                {
                    *current = Some(value);
                }
            }
            Self::Avg { sum, count } => {
                if let Some(value) = value.and_then(numeric) {
                    *sum = Some(match sum.take() {
                        Some(existing) => existing.checked_add(value).ok_or_else(overflow)?,
                        None => value,
                    });
                    *count += 1;
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> Value {
        match self {
            Self::Count(total) => Value::from(total),
            Self::DistinctCount(values, _) => Value::from(values.len() as u64),
            Self::Sum(value) | Self::Min(value) | Self::Max(value) => {
                value.map_or(Value::Null, |value| Value::from(value.to_string()))
            }
            Self::Avg { sum, count } => match sum {
                Some(sum) if count > 0 => {
                    let scale = sum.scale().max(6);
                    sum.try_decimal_div_round(
                        &Numeric::new(count, 0),
                        scale,
                        RoundingMode::TowardZero,
                    )
                    .map_or(Value::Null, |avg| Value::from(avg.to_string()))
                }
                _ => Value::Null,
            },
        }
    }
}

/// Identity of a group or distinct value. It follows filter equality: on
/// numeric and metadata fields a number and its decimal string are one value.
fn group_key_text(value: &Value, ty: FieldType) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(flag) => flag.to_string(),
        _ => match numeric(value) {
            Some(number)
                if value.is_number() || matches!(ty, FieldType::Number | FieldType::Json) =>
            {
                number.to_string()
            }
            _ => norito::json::to_json(value).unwrap_or_default(),
        },
    }
}

fn aggregate_rows<I>(
    plan: &AggregatePlan,
    rows: I,
    max_groups: usize,
    out: &mut Vec<Map>,
) -> Result<(), CollectionError>
where
    I: Iterator<Item = Result<Option<Map>, CollectionError>>,
{
    struct Group {
        values: Vec<(String, Value)>,
        metrics: Vec<MetricState>,
    }
    let mut groups: BTreeMap<Vec<String>, Group> = BTreeMap::new();
    for row in rows {
        let Some(row) = row? else {
            continue;
        };
        let key: Vec<String> = plan
            .spec
            .group_by
            .iter()
            .map(|field| {
                let ty = plan
                    .outputs
                    .get(&field.0)
                    .copied()
                    .unwrap_or(FieldType::Json);
                field_value(&row, &field.0)
                    .map_or_else(|| "null".to_owned(), |value| group_key_text(value, ty))
            })
            .collect();
        if !groups.contains_key(&key) && groups.len() >= max_groups {
            return Err(CollectionError::new(
                "query_scan_limit_exceeded",
                "aggregate",
                format!("the aggregate would produce more than {max_groups} groups; add a filter"),
            ));
        }
        let group = groups.entry(key).or_insert_with(|| Group {
            values: plan
                .spec
                .group_by
                .iter()
                .map(|field| {
                    (
                        field.0.clone(),
                        field_value(&row, &field.0).cloned().unwrap_or(Value::Null),
                    )
                })
                .collect(),
            metrics: plan
                .spec
                .metrics
                .iter()
                .zip(&plan.metric_types)
                .map(|(metric, ty)| MetricState::new(metric, *ty))
                .collect(),
        });
        for (state, metric) in group.metrics.iter_mut().zip(&plan.spec.metrics) {
            state.update(&row, metric, max_groups)?;
        }
    }
    for group in groups.into_values() {
        let mut row = Map::new();
        for (name, value) in group.values {
            insert_path(&mut row, &name, value);
        }
        for (metric, state) in plan.spec.metrics.iter().zip(group.metrics) {
            row.insert(metric.alias.clone(), state.finish());
        }
        out.push(row);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::specs::{ACCOUNT_ASSETS, ACCOUNT_TRANSACTIONS, DOMAINS};
    use super::*;
    use iroha_torii_shared::list_query::{AggregateMetric, SortKey, field};

    const LIMITS: Limits = Limits {
        default_limit: 2,
        max_limit: 10,
        max_scanned_rows: 1000,
        ordered_page_scan_budget: 1000,
        max_groups: 100,
    };

    fn domain(id: &str, owner: &str, tier: u64) -> Map {
        let mut metadata = Map::new();
        metadata.insert("tier".into(), Value::from(tier));
        let mut row = Map::new();
        row.insert("id".into(), Value::from(id));
        row.insert("owned_by".into(), Value::from(owner));
        row.insert("logo".into(), Value::Null);
        row.insert("metadata".into(), Value::Object(metadata));
        row
    }

    fn domains() -> Vec<Map> {
        vec![
            domain("delta", "bob", 2),
            domain("alpha", "alice", 1),
            domain("echo", "alice", 3),
            domain("charlie", "bob", 1),
            domain("bravo", "alice", 2),
        ]
    }

    fn ids(page: &RowPage) -> Vec<String> {
        page.items
            .iter()
            .map(|row| row["id"].as_str().unwrap().to_owned())
            .collect()
    }

    fn run(query: &ListQuery) -> Result<RowPage, CollectionError> {
        let prepared = prepare(&DOMAINS, "", query, &LIMITS)?;
        let page = prepared.execute(domains(), &LIMITS)?;
        Ok(prepared.project(page))
    }

    #[test]
    fn cursor_walk_visits_every_row_once_in_order() {
        let query = ListQuery::new().sort_by(SortKey::desc("metadata.tier"));
        let mut seen = Vec::new();
        let mut current = query.clone();
        loop {
            let page = run(&current).expect("page");
            assert!(page.items.len() <= 2);
            seen.extend(ids(&page));
            match page.next_cursor {
                Some(cursor) => current = query.clone().cursor(cursor),
                None => break,
            }
        }
        assert_eq!(seen, ["echo", "bravo", "delta", "alpha", "charlie"]);
    }

    #[test]
    fn filters_select_and_totals() {
        let query = ListQuery::new()
            .filter(field("owned_by").eq("alice") & field("metadata.tier").gte(2))
            .select(["id", "metadata.tier"])
            .limit(10)
            .include_total();
        let page = run(&query).expect("page");
        assert_eq!(ids(&page), ["bravo", "echo"]);
        assert_eq!(page.total, Some(2));
        assert_eq!(page.next_cursor, None);
        assert_eq!(page.items[0]["metadata"]["tier"].as_u64(), Some(2));
        assert!(page.items[0].get("owned_by").is_none());
    }

    #[test]
    fn rows_inserted_between_pages_do_not_shift_later_pages() {
        let query = ListQuery::new();
        let first = run(&query).expect("first page");
        assert_eq!(ids(&first), ["alpha", "bravo"]);
        let prepared_next = query.clone().cursor(first.next_cursor.clone().unwrap());
        let prepared = prepare(&DOMAINS, "", &prepared_next, &LIMITS).unwrap();
        let mut rows = domains();
        rows.push(domain("aardvark", "carol", 9));
        let second = prepared.execute(rows, &LIMITS).unwrap();
        assert_eq!(ids(&second), ["charlie", "delta"]);
    }

    #[test]
    fn rejects_unknown_fields_with_suggestions() {
        let err = run(&ListQuery::new().filter(field("ownd_by").eq("alice"))).unwrap_err();
        assert_eq!(err.code, "invalid_filter");
        assert_eq!(err.actual.as_deref(), Some("ownd_by"));
        assert_eq!(err.hint.as_deref(), Some("did you mean `owned_by`?"));
        let err = run(&ListQuery::new().filter(field("tier").eq(1))).unwrap_err();
        assert_eq!(
            err.hint.as_deref(),
            Some("metadata entries are addressed as `metadata.tier`")
        );
        let err = run(&ListQuery::new().sort_by(SortKey::asc("logo"))).unwrap_err();
        assert_eq!(err.code, "invalid_sort");
        let err = run(&ListQuery::new().select(["owner"])).unwrap_err();
        assert_eq!(err.code, "invalid_select");
    }

    #[test]
    fn rejects_type_mismatches_and_bad_limits() {
        let err = run(&ListQuery::new().filter(field("id").gt(3))).unwrap_err();
        assert_eq!(err.code, "invalid_filter");
        assert!(err.message.contains("range comparisons"), "{err}");
        let err = run(&ListQuery::new().filter(field("owned_by").eq(5))).unwrap_err();
        assert!(err.message.contains("string values"), "{err}");
        let err = run(&ListQuery::new().limit(11)).unwrap_err();
        assert_eq!(err.code, "invalid_limit");
        assert_eq!(err.expected.as_deref(), Some("1..=10"));
    }

    #[test]
    fn cursors_are_bound_to_their_query() {
        let first = run(&ListQuery::new()).unwrap();
        let cursor = first.next_cursor.unwrap();
        let err = run(&ListQuery::new()
            .filter(field("owned_by").eq("bob"))
            .cursor(cursor.clone()))
        .unwrap_err();
        assert_eq!(err.code, "invalid_cursor");
        let other = prepare(
            &ACCOUNT_ASSETS,
            "",
            &ListQuery::new().cursor(cursor),
            &LIMITS,
        )
        .err();
        assert_eq!(other.map(|err| err.code), Some("invalid_cursor"));
        let page = run(&ListQuery::new()
            .limit(5)
            .cursor(run(&ListQuery::new()).unwrap().next_cursor.unwrap()));
        assert_eq!(ids(&page.unwrap()), ["charlie", "delta", "echo"]);
    }

    #[test]
    fn numeric_comparisons_are_exact() {
        let mut rows = Vec::new();
        for (asset, quantity) in [("a", "10.50"), ("b", "2"), ("c", "10.5000001")] {
            let mut row = Map::new();
            row.insert("asset".into(), Value::from(asset));
            row.insert("scope".into(), Value::from("global"));
            row.insert("account_id".into(), Value::from("alice"));
            row.insert("quantity".into(), Value::from(quantity));
            rows.push(row);
        }
        let query = ListQuery::new()
            .filter(field("quantity").eq("10.5") | field("quantity").lt(3))
            .sort_by(SortKey::desc("quantity"));
        let prepared = prepare(&ACCOUNT_ASSETS, "", &query, &LIMITS).unwrap();
        let page = prepared.execute(rows, &LIMITS).unwrap();
        let assets: Vec<_> = page
            .items
            .iter()
            .map(|row| row["asset"].as_str().unwrap())
            .collect();
        assert_eq!(assets, ["a", "b"]);
    }

    #[test]
    fn aggregates_group_filter_and_page() {
        let query = ListQuery::new()
            .aggregate(AggregateSpec {
                group_by: vec!["owned_by".into()],
                metrics: vec![
                    AggregateMetric {
                        alias: "domains".into(),
                        r#fn: AggregateFn::Count,
                        field: None,
                    },
                    AggregateMetric {
                        alias: "tiers".into(),
                        r#fn: AggregateFn::Sum,
                        field: Some("metadata.tier".into()),
                    },
                ],
                having: Some(FilterExpr::parse("domains >= 2").unwrap()),
            })
            .sort_by(SortKey::desc("tiers"))
            .limit(1);
        let first = run(&query).unwrap();
        assert_eq!(first.items[0]["owned_by"].as_str(), Some("alice"));
        assert_eq!(first.items[0]["tiers"].as_str(), Some("6"));
        let second = run(&query.clone().cursor(first.next_cursor.unwrap())).unwrap();
        assert_eq!(second.items[0]["owned_by"].as_str(), Some("bob"));
        assert_eq!(second.next_cursor, None);
        let err = run(&ListQuery::new().aggregate(AggregateSpec {
            group_by: vec![],
            metrics: vec![AggregateMetric {
                alias: "x".into(),
                r#fn: AggregateFn::Sum,
                field: Some("id".into()),
            }],
            having: None,
        }))
        .unwrap_err();
        assert!(err.message.contains("numeric field"), "{err}");
    }
    fn transaction(height: u64, index: u64, authority: &str) -> Map {
        let mut row = Map::new();
        row.insert(
            "entrypoint_hash".into(),
            Value::from(format!("{height}:{index}")),
        );
        row.insert("block_height".into(), Value::from(height));
        row.insert("block_index".into(), Value::from(index));
        row.insert("authority".into(), Value::from(authority));
        row
    }

    fn row_position(row: &Map) -> Option<Position> {
        Some((
            row.get(POSITION_FIELDS[0])?.as_u64()?,
            row.get(POSITION_FIELDS[1])?.as_u64()?,
        ))
    }

    fn hashes(rows: &[Map]) -> Vec<String> {
        rows.iter()
            .map(|row| row["entrypoint_hash"].as_str().unwrap().to_owned())
            .collect()
    }

    /// One route's history walk: newest first, strictly before the cursor,
    /// examining at most `budget` rows per page.
    fn walk(prepared: &Prepared<'_>, history: &[Map], budget: usize) -> RowPage {
        let resume = prepared.resume_position();
        let (mut items, mut last_kept, mut last_visited, mut visited) = (Vec::new(), None, None, 0);
        for row in history.iter().rev() {
            let position = row_position(row).unwrap();
            if resume.is_some_and(|resume| position >= resume) {
                continue;
            }
            if visited == budget {
                return prepared.positioned_page(items, last_visited);
            }
            visited += 1;
            last_visited = Some(position);
            if !prepared.matches(row) {
                continue;
            }
            if items.len() == prepared.limit() {
                return prepared.positioned_page(items, last_kept);
            }
            items.push(row.clone());
            last_kept = Some(position);
        }
        prepared.positioned_page(items, None)
    }

    #[test]
    fn positioned_reads_page_by_block_coordinates() {
        let history: Vec<Map> = (1..=6)
            .flat_map(|height| {
                (0..3).map(move |index| {
                    let authority = if (height + index) % 2 == 0 {
                        "alice"
                    } else {
                        "bob"
                    };
                    transaction(height, index, authority)
                })
            })
            .collect();
        let expected: Vec<String> = hashes(&history)
            .into_iter()
            .rev()
            .zip(history.iter().rev())
            .filter(|(_, row)| row["authority"].as_str() == Some("alice"))
            .map(|(hash, _)| hash)
            .collect();
        let query = ListQuery::new()
            .filter(field("authority").eq("alice"))
            .limit(2);
        for budget in [1, 2, 5, 100] {
            let (mut seen, mut current) = (Vec::new(), query.clone());
            for _ in 0..100 {
                let prepared = prepare(&ACCOUNT_TRANSACTIONS, "", &current, &LIMITS).unwrap();
                let page = walk(&prepared, &history, budget);
                assert!(page.items.len() <= 2);
                seen.extend(hashes(&page.items));
                match page.next_cursor {
                    Some(cursor) => current = query.clone().cursor(cursor),
                    None => break,
                }
            }
            assert_eq!(seen, expected, "budget {budget}");
        }
        let single = prepare(&ACCOUNT_TRANSACTIONS, "", &query, &LIMITS).unwrap();
        let page = walk(&single, &history, 100);
        let next = query.clone().cursor(page.next_cursor.unwrap());
        let resumed = prepare(&ACCOUNT_TRANSACTIONS, "", &next, &LIMITS).unwrap();
        assert_eq!(resumed.resume_position(), row_position(&page.items[1]));
    }

    #[test]
    fn movement_cursors_keep_the_intra_transaction_position_and_scope() {
        // Both collections accept the filter, so cross-collection replay reaches
        // cursor validation instead of stopping at an unknown field.
        let query = ListQuery::new().filter(FilterExpr::parse("block_height >= 1").unwrap());
        let prepared = prepare(
            &super::super::specs::ACCOUNT_HISTORY,
            "alice",
            &query,
            &LIMITS,
        )
        .unwrap();
        let page = prepared.movement_page(Vec::new(), Some((70, 2, 8)));
        let next = query.clone().cursor(page.next_cursor.unwrap());
        let resumed = prepare(
            &super::super::specs::ACCOUNT_HISTORY,
            "alice",
            &next,
            &LIMITS,
        )
        .unwrap();
        assert_eq!(resumed.resume_position(), Some((70, 2)));
        assert_eq!(resumed.resume_movement_position(), Some((70, 2, 8)));
        assert_eq!(resumed.movement_page(Vec::new(), None).next_cursor, None);
        assert_eq!(
            prepare(&super::super::specs::ACCOUNT_HISTORY, "bob", &next, &LIMITS)
                .err()
                .unwrap()
                .code,
            "invalid_cursor"
        );
        assert_eq!(
            prepare(&ACCOUNT_TRANSACTIONS, "alice", &next, &LIMITS)
                .err()
                .unwrap()
                .code,
            "invalid_cursor"
        );
        let zero_height = query.clone().cursor(
            prepared
                .movement_page(Vec::new(), Some((0, 2, 8)))
                .next_cursor
                .unwrap(),
        );
        assert_eq!(
            prepare(
                &super::super::specs::ACCOUNT_HISTORY,
                "alice",
                &zero_height,
                &LIMITS,
            )
            .err()
            .unwrap()
            .code,
            "invalid_cursor"
        );
    }

    #[test]
    fn height_range_follows_top_level_conjuncts() {
        let range = |text: &str| {
            let query = ListQuery::new().filter(FilterExpr::parse(text).unwrap());
            prepare(&ACCOUNT_TRANSACTIONS, "", &query, &LIMITS)
                .unwrap()
                .height_range()
        };
        assert_eq!(range("result_ok = true"), (0, u64::MAX));
        assert_eq!(range("block_height >= 10 and block_height < 20"), (10, 19));
        assert_eq!(
            range("block_height > 10 and block_height <= \"15\""),
            (11, 15)
        );
        assert_eq!(range("block_height = 7 and authority = \"a\""), (7, 7));
        assert_eq!(
            range("block_height >= 9 or block_height = 1"),
            (0, u64::MAX)
        );
        assert_eq!(range("not block_height < 5"), (0, u64::MAX));
        let (low, high) = range("block_height > 9 and block_height < 9");
        assert!(low > high, "contradictory bounds select nothing");
    }

    #[test]
    fn positioned_reads_reject_sort_totals_aggregates_and_keyset_cursors() {
        let count = AggregateSpec {
            group_by: vec![],
            metrics: vec![AggregateMetric {
                alias: "n".into(),
                r#fn: AggregateFn::Count,
                field: None,
            }],
            having: None,
        };
        for (query, code) in [
            (
                ListQuery::new().sort_by(SortKey::desc("block_height")),
                "invalid_sort",
            ),
            (ListQuery::new().include_total(), "invalid_include_total"),
            (ListQuery::new().aggregate(count), "invalid_aggregate"),
        ] {
            let err = prepare(&ACCOUNT_TRANSACTIONS, "", &query, &LIMITS)
                .err()
                .expect("rejected");
            assert_eq!(err.code, code);
        }
        let keyset = run(&ListQuery::new()).unwrap().next_cursor.unwrap();
        let err = prepare(
            &ACCOUNT_TRANSACTIONS,
            "",
            &ListQuery::new().cursor(keyset),
            &LIMITS,
        )
        .err();
        assert_eq!(err.map(|err| err.code), Some("invalid_cursor"));
        let page = prepare(&ACCOUNT_TRANSACTIONS, "", &ListQuery::new(), &LIMITS)
            .unwrap()
            .positioned_page(Vec::new(), Some((7, 1)));
        let err = prepare(
            &DOMAINS,
            "",
            &ListQuery::new().cursor(page.next_cursor.unwrap()),
            &LIMITS,
        )
        .err();
        assert_eq!(err.map(|err| err.code), Some("invalid_cursor"));
    }

    #[test]
    fn scan_budget_is_enforced() {
        let limits = Limits {
            max_scanned_rows: 3,
            ..LIMITS
        };
        let query = ListQuery::new();
        let prepared = prepare(&DOMAINS, "", &query, &limits).unwrap();
        let err = prepared.execute(domains(), &limits).unwrap_err();
        assert_eq!(err.code, "query_scan_limit_exceeded");
    }

    #[test]
    fn page_json_roundtrip() {
        let page = run(&ListQuery::new().include_total()).unwrap();
        let json = norito::json::to_value(&page.clone().into_page()).unwrap();
        let decoded: Page<Value> = norito::json::from_value(json).unwrap();
        assert_eq!(decoded.items.len(), page.items.len());
        assert_eq!(decoded.next_cursor, page.next_cursor);
        assert_eq!(decoded.total, page.total);
    }

    fn asset_row(account: &str, asset: &str, scope: &str) -> Map {
        let mut row = Map::new();
        row.insert("account_id".into(), Value::from(account));
        row.insert("asset".into(), Value::from(asset));
        row.insert("scope".into(), Value::from(scope));
        row.insert("quantity".into(), Value::from("1"));
        row
    }

    fn domain_with_metadata(id: &str, metadata: Value) -> Map {
        let mut row = domain(id, "alice", 1);
        row.insert("metadata".into(), metadata);
        row
    }

    fn count_metric(alias: &str) -> AggregateMetric {
        AggregateMetric {
            alias: alias.into(),
            r#fn: AggregateFn::Count,
            field: None,
        }
    }
    #[test]
    fn cursors_bind_the_collection_path() {
        let rows = || {
            vec![
                asset_row("alice", "a", "ds1"),
                asset_row("alice", "b", "ds1"),
            ]
        };
        let query = ListQuery::new().limit(1);
        let prepared = prepare(&ACCOUNT_ASSETS, "alice", &query, &LIMITS).unwrap();
        let cursor = prepared
            .execute(rows(), &LIMITS)
            .unwrap()
            .next_cursor
            .unwrap();
        let next = query.cursor(cursor);
        assert!(prepare(&ACCOUNT_ASSETS, "alice", &next, &LIMITS).is_ok());
        let err = prepare(&ACCOUNT_ASSETS, "bob", &next, &LIMITS)
            .err()
            .unwrap();
        assert_eq!(err.code, "invalid_cursor");
    }

    #[test]
    fn oversized_sort_values_are_refused_instead_of_issuing_unusable_cursors() {
        let mut long = Map::new();
        long.insert("note".into(), Value::from("x".repeat(CURSOR_MAX_BYTES)));
        let rows = vec![
            domain_with_metadata("alpha", Value::Object(long)),
            domain_with_metadata("bravo", norito::json!({"note": "short"})),
        ];
        let query = ListQuery::new()
            .sort_by(SortKey::desc("metadata.note"))
            .limit(1);
        let prepared = prepare(&DOMAINS, "", &query, &LIMITS).unwrap();
        let err = prepared.execute(rows.clone(), &LIMITS).unwrap_err();
        assert_eq!(err.code, "invalid_sort");
        let ascending = ListQuery::new()
            .sort_by(SortKey::asc("metadata.note"))
            .limit(1);
        let prepared = prepare(&DOMAINS, "", &ascending, &LIMITS).unwrap();
        let page = prepared.execute(rows, &LIMITS).unwrap();
        assert_eq!(ids(&page), ["bravo"]);
        assert!(page.next_cursor.is_some(), "short sort values still page");
    }

    #[test]
    fn metadata_matches_list_elements_and_fractional_numbers() {
        let rows = vec![
            domain_with_metadata(
                "alpha",
                norito::json!({"rating": 4.5, "tags": ["vip", "beta"]}),
            ),
            domain_with_metadata("bravo", norito::json!({"rating": 3, "tags": ["beta"]})),
        ];
        let matching = |filter: &str| {
            let query = ListQuery::new().filter(FilterExpr::parse(filter).unwrap());
            let prepared = prepare(&DOMAINS, "", &query, &LIMITS).unwrap();
            ids(&prepared.execute(rows.clone(), &LIMITS).unwrap())
        };
        assert_eq!(matching("metadata.rating >= 4"), ["alpha"]);
        assert_eq!(matching("metadata.rating = 4.5"), ["alpha"]);
        assert_eq!(matching("metadata.tags = \"vip\""), ["alpha"]);
        assert_eq!(matching("metadata.tags in [\"beta\"]"), ["alpha", "bravo"]);
        assert_eq!(matching("metadata.tags != \"vip\""), ["bravo"]);
    }

    #[test]
    fn huge_decimal_text_compares_as_text() {
        let huge = "1".repeat(MAX_DECIMAL_TEXT_BYTES + 1);
        assert!(numeric(&Value::from(huge.clone())).is_none());
        assert!(numeric(&Value::from(huge[..MAX_DECIMAL_TEXT_BYTES / 2].to_owned())).is_some());
    }

    #[test]
    fn aggregate_groups_follow_equality_and_reject_alias_collisions() {
        let rows = vec![
            domain_with_metadata("alpha", norito::json!({"tier": 5})),
            domain_with_metadata("bravo", norito::json!({"tier": "5"})),
            domain_with_metadata("charlie", norito::json!({"tier": 6})),
        ];
        let query = ListQuery::new().aggregate(AggregateSpec {
            group_by: vec!["metadata.tier".into()],
            metrics: vec![count_metric("n")],
            having: None,
        });
        let prepared = prepare(&DOMAINS, "", &query, &LIMITS).unwrap();
        let page = prepared.execute(rows, &LIMITS).unwrap();
        let counts: Vec<u64> = page
            .items
            .iter()
            .map(|row| row["n"].as_u64().unwrap())
            .collect();
        assert_eq!(counts, [2, 1], "5 and \"5\" are one group");
        let colliding = ListQuery::new().aggregate(AggregateSpec {
            group_by: vec!["metadata.tier".into()],
            metrics: vec![count_metric("metadata")],
            having: None,
        });
        let err = prepare(&DOMAINS, "", &colliding, &LIMITS).err().unwrap();
        assert_eq!(err.code, "invalid_aggregate");
        assert!(err.message.contains("collides"), "{err}");
    }

    #[test]
    fn range_comparisons_reject_lists_and_name_the_aggregate() {
        let list = ListQuery::new().filter(field("asset_ids").gt("x"));
        let err = prepare(&ACCOUNT_TRANSACTIONS, "", &list, &LIMITS)
            .err()
            .unwrap();
        assert_eq!(err.code, "invalid_filter");
        assert!(err.message.contains("list"), "{err}");
        let having = ListQuery::new().aggregate(AggregateSpec {
            group_by: vec!["owned_by".into()],
            metrics: vec![count_metric("n")],
            having: Some(FilterExpr::parse("n > \"x\"").unwrap()),
        });
        let err = prepare(&DOMAINS, "", &having, &LIMITS).err().unwrap();
        assert_eq!(err.code, "invalid_aggregate");
    }

    /// Rows in canonical order, strictly after `after` (or before it,
    /// descending): what an identity-ordered producer streams.
    fn seek(rows: &[Map], scan: OrderedScan<'_>) -> Vec<(bool, Option<Map>)> {
        let mut sorted = rows.to_vec();
        sorted.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        if scan.descending {
            sorted.reverse();
        }
        sorted
            .into_iter()
            .filter(|row| {
                scan.after.is_none_or(|after| {
                    let id = row["id"].as_str().unwrap();
                    if scan.descending {
                        id < after
                    } else {
                        id > after
                    }
                })
            })
            .map(|row| (true, Some(row)))
            .collect()
    }

    fn ordered_pages(query: &ListQuery, limits: &Limits) -> Vec<(Vec<String>, bool)> {
        let mut pages = Vec::new();
        let mut current = query.clone();
        for _ in 0..20 {
            let prepared = prepare(&DOMAINS, "", &current, limits).unwrap();
            let scan = prepared.ordered_scan().expect("identity-ordered read");
            let page = prepared
                .execute_ordered(seek(&domains(), scan), limits)
                .unwrap();
            let more = page.next_cursor.is_some();
            pages.push((ids(&page), more));
            match page.next_cursor {
                Some(cursor) => current = query.clone().cursor(cursor),
                None => break,
            }
        }
        pages
    }

    #[test]
    fn identity_ordered_reads_seek_from_the_cursor() {
        let pages = ordered_pages(&ListQuery::new(), &LIMITS);
        let all: Vec<String> = pages.iter().flat_map(|(ids, _)| ids.clone()).collect();
        assert_eq!(all, ["alpha", "bravo", "charlie", "delta", "echo"]);
        assert!(pages.iter().all(|(ids, _)| ids.len() <= 2));
        let descending = ordered_pages(&ListQuery::new().sort_by(SortKey::desc("id")), &LIMITS);
        let all: Vec<String> = descending.iter().flat_map(|(ids, _)| ids.clone()).collect();
        assert_eq!(all, ["echo", "delta", "charlie", "bravo", "alpha"]);
    }

    #[test]
    fn identity_ordered_pages_end_early_at_the_scan_budget() {
        let limits = Limits {
            ordered_page_scan_budget: 2,
            ..LIMITS
        };
        // Only `echo` (the last row in identifier order) matches.
        let query = ListQuery::new().filter(field("metadata.tier").eq(3));
        let pages = ordered_pages(&query, &limits);
        assert_eq!(
            pages,
            [
                (vec![], true),
                (vec![], true),
                (vec!["echo".to_owned()], false)
            ],
            "short pages carry a cursor at the last examined row"
        );
    }

    #[test]
    fn identity_ordered_totals_count_every_match() {
        let query = ListQuery::new()
            .filter(field("owned_by").eq("alice"))
            .include_total()
            .limit(1);
        let prepared = prepare(&DOMAINS, "", &query, &LIMITS).unwrap();
        let scan = prepared.ordered_scan().unwrap();
        let page = prepared
            .execute_ordered(seek(&domains(), scan), &LIMITS)
            .unwrap();
        assert_eq!(ids(&page), ["alpha"]);
        assert_eq!(page.total, Some(3));
        assert!(page.next_cursor.is_some());
    }

    #[test]
    fn ordered_lookahead_stops_after_a_full_page_with_sparse_matches() {
        let limits = Limits {
            ordered_page_scan_budget: 2,
            ..LIMITS
        };
        let query = ListQuery::new().filter(field("id").eq("alpha")).limit(1);
        let prepared = prepare(&DOMAINS, "", &query, &limits).unwrap();
        let examined = std::cell::Cell::new(0);
        let rows = seek(&domains(), prepared.ordered_scan().unwrap())
            .into_iter()
            .inspect(|_| examined.set(examined.get() + 1));
        let page = prepared.execute_ordered(rows, &limits).unwrap();
        assert_eq!(ids(&page), ["alpha"]);
        assert_eq!(
            examined.get(),
            2,
            "a full page must not disable the scan budget"
        );
        assert!(page.next_cursor.is_some());
    }

    #[test]
    fn ordered_scans_charge_hidden_entries_without_exposing_their_keys() {
        let limits = Limits {
            ordered_page_scan_budget: 2,
            ..LIMITS
        };
        let query = ListQuery::new().filter(field("id").eq("echo"));
        let prepared = prepare(&DOMAINS, "", &query, &limits).unwrap();
        let visible = seek(&domains(), prepared.ordered_scan().unwrap())[0].clone();
        let page = prepared
            .execute_ordered([visible, (true, None)], &limits)
            .unwrap();
        assert!(page.items.is_empty());
        let continuation = query.clone().cursor(page.next_cursor.unwrap());
        let resumed = prepare(&DOMAINS, "", &continuation, &limits).unwrap();
        assert_eq!(resumed.ordered_scan().unwrap().after, Some("alpha"));
        let err = prepared
            .execute_ordered([(true, None), (true, None)], &limits)
            .unwrap_err();
        assert_eq!(err.code, "query_scan_limit_exceeded");
    }

    #[test]
    fn ordered_totals_include_matches_before_the_cursor() {
        for descending in [false, true] {
            let query = ListQuery::new()
                .include_total()
                .limit(1)
                .sort_by(if descending {
                    SortKey::desc("id")
                } else {
                    SortKey::asc("id")
                });
            let prepared = prepare(&DOMAINS, "", &query, &LIMITS).unwrap();
            let rows = seek(&domains(), prepared.ordered_scan().unwrap());
            let first = prepared.execute_ordered(rows.clone(), &LIMITS).unwrap();
            let after = ids(&first)[0].clone();
            let query = query.cursor(first.next_cursor.unwrap());
            let prepared = prepare(&DOMAINS, "", &query, &LIMITS).unwrap();
            let second = prepared
                .execute_ordered(
                    rows.into_iter().map(|(_, row)| {
                        let id = row.as_ref().unwrap()["id"].as_str().unwrap();
                        (
                            if descending {
                                id < after.as_str()
                            } else {
                                id > after.as_str()
                            },
                            row,
                        )
                    }),
                    &LIMITS,
                )
                .unwrap();
            assert_eq!(
                second.total,
                Some(5),
                "total describes the entire query on every page"
            );
            assert_eq!(ids(&second), if descending { ["delta"] } else { ["bravo"] });
        }
    }

    #[test]
    fn unordered_and_aggregate_scans_charge_hidden_entries() {
        let limits = Limits {
            max_scanned_rows: 1,
            ..LIMITS
        };
        for query in [
            ListQuery::new(),
            ListQuery::new().aggregate(AggregateSpec {
                group_by: vec![],
                metrics: vec![count_metric("n")],
                having: None,
            }),
        ] {
            let prepared = prepare(&DOMAINS, "", &query, &limits).unwrap();
            let err = prepared.execute_entries([None, None], &limits).unwrap_err();
            assert_eq!(err.code, "query_scan_limit_exceeded");
        }
    }

    #[test]
    fn only_identity_orders_stream() {
        let ordered = |query: ListQuery| {
            prepare(&DOMAINS, "", &query, &LIMITS)
                .unwrap()
                .ordered_scan()
                .is_some()
        };
        assert!(ordered(ListQuery::new()));
        assert!(ordered(ListQuery::new().sort_by(SortKey::asc("id"))));
        assert!(ordered(ListQuery::new().sort_by(SortKey::desc("id"))));
        assert!(!ordered(ListQuery::new().sort_by(SortKey::asc("owned_by"))));
        assert!(!ordered(
            ListQuery::new()
                .sort_by(SortKey::asc("id"))
                .sort_by(SortKey::asc("owned_by"))
        ));
        let everything = ListQuery::new();
        let assets = prepare(&ACCOUNT_ASSETS, "alice", &everything, &LIMITS).unwrap();
        assert!(
            assets.ordered_scan().is_none(),
            "only id-keyed collections stream"
        );
    }
}
