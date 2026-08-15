//! Helper types for expressing lightweight JSON predicates.
//!
//! These helpers build canonical JSON payloads that the query DSL can embed in
//! [`CompoundPredicate`] values.  The structures keep field ordering stable so
//! serialised predicates are deterministic and easy to compare in tests or caches.
use crate::query::dsl::CompoundPredicate;
use norito::json::{self, JsonDeserialize, JsonSerialize, Map, Value};
use std::{
    cell::Cell,
    string::{String, ToString},
    vec::Vec,
};
use thiserror::Error;
#[derive(Clone, Copy)]
struct PredicateJsonExecutionBounds {
    body_bytes: usize,
    allocation_bytes: usize,
}
std::thread_local! {
    static PREDICATE_JSON_EXECUTION_BOUNDS: Cell<Option<PredicateJsonExecutionBounds>> = const {
        Cell::new(None)
    };
}
struct PredicateJsonExecutionBoundsGuard(Option<PredicateJsonExecutionBounds>);
impl Drop for PredicateJsonExecutionBoundsGuard {
    fn drop(&mut self) {
        PREDICATE_JSON_EXECUTION_BOUNDS.with(|slot| slot.set(self.0));
    }
}
/// Run predicate JSON work with checked body and allocation ceilings.
///
/// This server-boundary hook leaves ordinary in-process evaluation unchanged
/// when no scope is installed. Nested scopes can only tighten either ceiling.
#[doc(hidden)]
pub fn with_bounded_predicate_json_execution<R>(
    body_bytes: usize,
    allocation_bytes: usize,
    execute: impl FnOnce() -> R,
) -> R {
    let requested = PredicateJsonExecutionBounds {
        body_bytes,
        allocation_bytes,
    };
    let previous = PREDICATE_JSON_EXECUTION_BOUNDS.with(|slot| {
        let previous = slot.get();
        let effective = previous.map_or(requested, |outer| PredicateJsonExecutionBounds {
            body_bytes: outer.body_bytes.min(requested.body_bytes),
            allocation_bytes: outer.allocation_bytes.min(requested.allocation_bytes),
        });
        slot.set(Some(effective));
        previous
    });
    let _guard = PredicateJsonExecutionBoundsGuard(previous);
    execute()
}
fn predicate_json_execution_bounds() -> Option<PredicateJsonExecutionBounds> {
    PREDICATE_JSON_EXECUTION_BOUNDS.with(Cell::get)
}
fn predicate_json_decode_limits(bounds: PredicateJsonExecutionBounds) -> norito::DecodeLimits {
    let total_elements = bounds.allocation_bytes.max(bounds.body_bytes);
    norito::DecodeLimits::new(
        total_elements,
        bounds.body_bytes,
        total_elements,
        bounds.allocation_bytes,
        norito::core::MAX_OWNED_VALUE_DECODE_DEPTH,
    )
}
/// Materialize one predicate candidate through the checked JSON writer when a
/// server execution scope is active.
#[doc(hidden)]
pub fn predicate_json_value_for_execution<T: JsonSerialize + ?Sized>(value: &T) -> Option<Value> {
    let Some(bounds) = predicate_json_execution_bounds() else {
        return json::to_value(value).ok();
    };
    norito::core::with_decode_limits_scope(predicate_json_decode_limits(bounds), || {
        let bytes = json::to_json_bounded_boxed(value, bounds.body_bytes).ok()?;
        let raw = std::str::from_utf8(&bytes).ok()?;
        json::parse_value(raw).ok()
    })
}
/// Parse retained predicate JSON through the owned, allocation-charged
/// conversion when a server execution scope is active.
#[doc(hidden)]
pub fn predicate_json_from_raw_for_execution(raw: &str) -> Option<PredicateJson> {
    let Some(bounds) = predicate_json_execution_bounds() else {
        let value = json::from_json::<Value>(raw).ok()?;
        return PredicateJson::try_from_value(&value).ok();
    };
    if raw.len() > bounds.body_bytes {
        return None;
    }
    norito::core::with_decode_limits_scope(predicate_json_decode_limits(bounds), || {
        let value = json::from_json::<Value>(raw).ok()?;
        PredicateJson::try_from_owned_value(value).ok()
    })
}
/// Parse predicate JSON for an optional producer-local candidate plan.
///
/// The bounded ordinary-query lane deliberately returns no candidate plan:
/// its source proof covers the full world scan, while the predicate itself is
/// still evaluated through [`predicate_json_from_raw_for_execution`]. Legacy
/// callers retain their existing indexed plan outside that execution scope.
#[doc(hidden)]
pub fn predicate_json_candidate_plan_for_execution(raw: &str) -> Option<PredicateJson> {
    if predicate_json_execution_bounds().is_some() {
        // TODO: Restore indexed ordinary plans after every typed identifier
        // parser and candidate container has an allocation-accounted seam.
        return None;
    }
    let value = json::from_json::<Value>(raw).ok()?;
    PredicateJson::try_from_value(&value).ok()
}
/// JSON representation of a lightweight predicate tree.
///
/// Supported operators:
/// - `equals`: matches exact values for the given field path.
/// - `in`: matches values contained in a list.
/// - `exists`: asserts that a field path is present (non-null).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PredicateJson {
    /// List of field/value pairs that must match exactly.
    pub equals: Vec<EqualsCondition>,
    /// List of set membership constraints for fields.
    pub r#in: Vec<InCondition>,
    /// Field paths whose presence is required.
    pub exists: Vec<String>,
}
impl Eq for PredicateJson {}
impl PredicateJson {
    /// Returns true when no conditions are specified.
    pub fn is_empty(&self) -> bool {
        self.equals.is_empty() && self.r#in.is_empty() && self.exists.is_empty()
    }
    /// Convert into a canonical JSON value with deterministic ordering.
    fn to_value(&self) -> Value {
        let mut map = Map::new();
        if !self.equals.is_empty() {
            let mut eqs = self.equals.clone();
            eqs.sort_by(|a, b| a.field.cmp(&b.field));
            let arr = eqs
                .into_iter()
                .map(|cond| {
                    let mut entry = Map::new();
                    entry.insert("field".to_owned(), Value::String(cond.field));
                    entry.insert("value".to_owned(), cond.value);
                    Value::Object(entry)
                })
                .collect();
            map.insert("equals".to_owned(), Value::Array(arr));
        }
        if !self.r#in.is_empty() {
            let mut list = self.r#in.clone();
            list.sort_by(|a, b| a.field.cmp(&b.field));
            let arr = list
                .into_iter()
                .map(|cond| {
                    let mut entry = Map::new();
                    entry.insert("field".to_owned(), Value::String(cond.field));
                    entry.insert("values".to_owned(), Value::Array(cond.values));
                    Value::Object(entry)
                })
                .collect();
            map.insert("in".to_owned(), Value::Array(arr));
        }
        if !self.exists.is_empty() {
            let mut fields = self.exists.clone();
            fields.sort();
            let arr = fields.into_iter().map(Value::String).collect();
            map.insert("exists".to_owned(), Value::Array(arr));
        }
        Value::Object(map)
    }
    fn sort_in_place(&mut self) {
        stable_insertion_sort_by(&mut self.equals, |left, right| left.field.cmp(&right.field));
        stable_insertion_sort_by(&mut self.r#in, |left, right| left.field.cmp(&right.field));
        stable_insertion_sort_by(&mut self.exists, |left, right| left.cmp(right));
    }
    /// Build predicate from JSON value.
    ///
    /// # Errors
    /// Returns an error when the JSON structure does not match the predicate schema.
    pub fn try_from_value(value: &Value) -> Result<Self, PredicateParseError> {
        match value {
            Value::Null => Ok(Self::default()),
            Value::Object(map) => {
                let mut predicate = PredicateJson::default();
                for (key, entry) in map {
                    match key.as_str() {
                        "equals" => {
                            let arr = entry
                                .as_array()
                                .ok_or(PredicateParseError::ExpectedArray("equals"))?;
                            for item in arr {
                                let obj = item
                                    .as_object()
                                    .ok_or(PredicateParseError::ExpectedObjectSection("equals"))?;
                                let field = obj
                                    .get("field")
                                    .ok_or(PredicateParseError::MissingField("equals", "field"))?
                                    .as_string()
                                    .ok_or(PredicateParseError::ExpectedString(
                                        "equals", "field",
                                    ))?;
                                let value = obj
                                    .get("value")
                                    .ok_or(PredicateParseError::MissingField("equals", "value"))?
                                    .clone();
                                predicate.equals.push(EqualsCondition::new(field, value));
                            }
                        }
                        "in" => {
                            let arr = entry
                                .as_array()
                                .ok_or(PredicateParseError::ExpectedArray("in"))?;
                            for item in arr {
                                let obj = item
                                    .as_object()
                                    .ok_or(PredicateParseError::ExpectedObjectSection("in"))?;
                                let field = obj
                                    .get("field")
                                    .ok_or(PredicateParseError::MissingField("in", "field"))?
                                    .as_string()
                                    .ok_or(PredicateParseError::ExpectedString("in", "field"))?;
                                let values = obj
                                    .get("values")
                                    .ok_or(PredicateParseError::MissingField("in", "values"))?
                                    .as_array()
                                    .ok_or(PredicateParseError::ExpectedArray("values"))?
                                    .clone();
                                if values.is_empty() {
                                    return Err(PredicateParseError::EmptyValues(field.to_owned()));
                                }
                                predicate.r#in.push(InCondition::new(field, values));
                            }
                        }
                        "exists" => {
                            let arr = entry
                                .as_array()
                                .ok_or(PredicateParseError::ExpectedArray("exists"))?;
                            for item in arr {
                                let s = item.as_string().ok_or(
                                    PredicateParseError::ExpectedString("exists", "field"),
                                )?;
                                predicate.exists.push(s.to_owned());
                            }
                        }
                        other => return Err(PredicateParseError::UnknownKey(other.to_owned())),
                    }
                }
                predicate.sort_in_place();
                Ok(predicate)
            }
            other => Err(PredicateParseError::ExpectedObjectRoot(other.type_name())),
        }
    }
    /// Build a predicate by consuming an owned JSON value.
    ///
    /// Unlike [`Self::try_from_value`], this path moves condition strings and
    /// nested JSON values out of the parser graph. It is used by bounded Norito
    /// request decoding so retained predicate state does not deep-clone an
    /// attacker-sized `Value` tree after the decode allocation scope ends.
    ///
    /// # Errors
    ///
    /// Returns an error when the JSON structure does not match the predicate schema or its
    /// destination vectors cannot be admitted by the active decode-allocation limit.
    pub fn try_from_owned_value(value: Value) -> Result<Self, PredicateParseError> {
        let map = match value {
            Value::Null => return Ok(Self::default()),
            Value::Object(map) => map,
            other => return Err(PredicateParseError::ExpectedObjectRoot(other.type_name())),
        };
        let mut predicate = PredicateJson::default();
        for (key, entry) in map {
            match key.as_str() {
                "equals" => {
                    let Value::Array(items) = entry else {
                        return Err(PredicateParseError::ExpectedArray("equals"));
                    };
                    predicate.equals = admitted_vec(items.len())?;
                    for item in items {
                        let Value::Object(mut object) = item else {
                            return Err(PredicateParseError::ExpectedObjectSection("equals"));
                        };
                        let field = take_string_field(&mut object, "equals", "field")?;
                        let value = object
                            .remove("value")
                            .ok_or(PredicateParseError::MissingField("equals", "value"))?;
                        predicate.equals.push(EqualsCondition::new(field, value));
                    }
                }
                "in" => {
                    let Value::Array(items) = entry else {
                        return Err(PredicateParseError::ExpectedArray("in"));
                    };
                    predicate.r#in = admitted_vec(items.len())?;
                    for item in items {
                        let Value::Object(mut object) = item else {
                            return Err(PredicateParseError::ExpectedObjectSection("in"));
                        };
                        let field = take_string_field(&mut object, "in", "field")?;
                        let values = object
                            .remove("values")
                            .ok_or(PredicateParseError::MissingField("in", "values"))?;
                        let Value::Array(values) = values else {
                            return Err(PredicateParseError::ExpectedArray("values"));
                        };
                        if values.is_empty() {
                            return Err(PredicateParseError::EmptyValues(field));
                        }
                        predicate.r#in.push(InCondition::new(field, values));
                    }
                }
                "exists" => {
                    let Value::Array(items) = entry else {
                        return Err(PredicateParseError::ExpectedArray("exists"));
                    };
                    predicate.exists = admitted_vec(items.len())?;
                    for item in items {
                        let Value::String(field) = item else {
                            return Err(PredicateParseError::ExpectedString("exists", "field"));
                        };
                        predicate.exists.push(field);
                    }
                }
                other => return Err(PredicateParseError::UnknownKey(other.to_owned())),
            }
        }
        predicate.sort_in_place();
        Ok(predicate)
    }
    /// Convert into a [`CompoundPredicate`] by serialising the canonical JSON representation. The
    /// resulting predicate carries the JSON payload that backends can interpret later.
    ///
    /// # Errors
    /// Returns an error when serialisation of the canonical JSON representation fails.
    pub fn into_compound<T: 'static>(&self) -> Result<CompoundPredicate<T>, json::Error> {
        if self.is_empty() {
            Ok(CompoundPredicate::PASS)
        } else {
            json::from_value(self.to_value())
        }
    }
}
fn stable_insertion_sort_by<T>(values: &mut [T], compare: impl Fn(&T, &T) -> core::cmp::Ordering) {
    for index in 1..values.len() {
        let mut current = index;
        while current > 0 && compare(&values[current], &values[current - 1]).is_lt() {
            values.swap(current, current - 1);
            current -= 1;
        }
    }
}
fn admitted_vec<T>(capacity: usize) -> Result<Vec<T>, PredicateParseError> {
    let requested = capacity
        .checked_mul(core::mem::size_of::<T>())
        .ok_or(PredicateParseError::ResourceLimit)?;
    norito::core::reserve_decode_allocation(requested)
        .map_err(|_| PredicateParseError::ResourceLimit)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| PredicateParseError::ResourceLimit)?;
    let allocated = values
        .capacity()
        .checked_mul(core::mem::size_of::<T>())
        .ok_or(PredicateParseError::ResourceLimit)?;
    if let Some(excess) = allocated.checked_sub(requested)
        && excess != 0
    {
        norito::core::reserve_decode_allocation(excess)
            .map_err(|_| PredicateParseError::ResourceLimit)?;
    }
    Ok(values)
}
fn take_string_field(
    object: &mut Map,
    section: &'static str,
    field: &'static str,
) -> Result<String, PredicateParseError> {
    match object
        .remove(field)
        .ok_or(PredicateParseError::MissingField(section, field))?
    {
        Value::String(value) => Ok(value),
        _ => Err(PredicateParseError::ExpectedString(section, field)),
    }
}
#[cfg(feature = "json")]
fn next_sorted_index_by<T, F>(items: &[T], previous: Option<usize>, key: F) -> Option<usize>
where
    F: for<'a> Fn(&'a T) -> &'a str + Copy,
{
    (0..items.len())
        .filter(|&index| {
            let Some(previous) = previous else {
                return true;
            };
            (key(&items[index]), index) > (key(&items[previous]), previous)
        })
        .min_by(|&left, &right| (key(&items[left]), left).cmp(&(key(&items[right]), right)))
}
impl JsonSerialize for PredicateJson {
    fn json_serialize(&self, out: &mut String) {
        json::write_with_unbounded_sink(out, |sink| self.json_serialize_to(sink));
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        out.begin_container()?;
        out.push('{')?;
        let mut wrote_section = false;
        if !self.equals.is_empty() {
            out.push_str("\"equals\":[")?;
            out.begin_container()?;
            let mut previous = None;
            let mut wrote = false;
            while let Some(index) =
                next_sorted_index_by(&self.equals, previous, |item| item.field.as_str())
            {
                if wrote {
                    out.push(',')?;
                }
                let condition = &self.equals[index];
                out.begin_container()?;
                out.push_str("{\"field\":")?;
                condition.field.json_serialize_to(out)?;
                out.push_str(",\"value\":")?;
                condition.value.json_serialize_to(out)?;
                out.push('}')?;
                out.end_container();
                wrote = true;
                previous = Some(index);
            }
            out.push(']')?;
            out.end_container();
            wrote_section = true;
        }
        if !self.exists.is_empty() {
            if wrote_section {
                out.push(',')?;
            }
            out.push_str("\"exists\":[")?;
            out.begin_container()?;
            let mut previous = None;
            let mut wrote = false;
            while let Some(index) = next_sorted_index_by(&self.exists, previous, String::as_str) {
                if wrote {
                    out.push(',')?;
                }
                self.exists[index].json_serialize_to(out)?;
                wrote = true;
                previous = Some(index);
            }
            out.push(']')?;
            out.end_container();
            wrote_section = true;
        }
        if !self.r#in.is_empty() {
            if wrote_section {
                out.push(',')?;
            }
            out.push_str("\"in\":[")?;
            out.begin_container()?;
            let mut previous = None;
            let mut wrote = false;
            while let Some(index) =
                next_sorted_index_by(&self.r#in, previous, |item| item.field.as_str())
            {
                if wrote {
                    out.push(',')?;
                }
                let condition = &self.r#in[index];
                out.begin_container()?;
                out.push_str("{\"field\":")?;
                condition.field.json_serialize_to(out)?;
                out.push_str(",\"values\":")?;
                condition.values.json_serialize_to(out)?;
                out.push('}')?;
                out.end_container();
                wrote = true;
                previous = Some(index);
            }
            out.push(']')?;
            out.end_container();
        }
        out.push('}')?;
        out.end_container();
        Ok(())
    }
}
impl JsonDeserialize for PredicateJson {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = Value::json_deserialize(parser)?;
        PredicateJson::try_from_value(&value).map_err(|err| json::Error::Message(err.to_string()))
    }
}
/// Equality predicate condition.
#[derive(Debug, Clone, PartialEq)]
pub struct EqualsCondition {
    /// Field path whose value is compared.
    pub field: String,
    /// Value that must match exactly.
    pub value: Value,
}
impl Eq for EqualsCondition {}
impl EqualsCondition {
    /// Construct an equality condition for the given field and value.
    pub fn new(field: impl Into<String>, value: Value) -> Self {
        Self {
            field: field.into(),
            value,
        }
    }
}
/// Set membership predicate condition.
#[derive(Debug, Clone, PartialEq)]
pub struct InCondition {
    /// Field path whose value must be contained in [`values`](Self::values).
    pub field: String,
    /// Allowed values for the field.
    pub values: Vec<Value>,
}
impl Eq for InCondition {}
impl InCondition {
    /// Construct a membership condition for the given field and allowed values.
    pub fn new(field: impl Into<String>, values: Vec<Value>) -> Self {
        Self {
            field: field.into(),
            values,
        }
    }
}
/// Errors produced when parsing predicate JSON structures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PredicateParseError {
    /// The root JSON value must be an object.
    #[error("predicate JSON must be an object, found {0}")]
    ExpectedObjectRoot(&'static str),
    /// A top-level key that is not recognised.
    #[error("unknown predicate section `{0}`")]
    UnknownKey(String),
    /// A predicate section must contain an array payload.
    #[error("`{0}` section must be an array")]
    ExpectedArray(&'static str),
    /// An entry inside a section must be a JSON object.
    #[error("`{0}` section entries must be objects")]
    ExpectedObjectSection(&'static str),
    /// A required field inside the section is missing.
    #[error("missing `{1}` field inside `{0}` section")]
    MissingField(&'static str, &'static str),
    /// A value that must be a string has a different JSON type.
    #[error("`{1}` inside `{0}` section must be a string")]
    ExpectedString(&'static str, &'static str),
    /// Membership conditions must contain at least one value.
    #[error("`in` values for field `{0}` must not be empty")]
    EmptyValues(String),
    /// An active decode-allocation limit or the platform allocator rejected a
    /// destination collection before it was constructed.
    #[error("predicate JSON exceeds the active allocation limit")]
    ResourceLimit,
}
trait ValueExt {
    fn as_string(&self) -> Option<&str>;
    fn type_name(&self) -> &'static str;
}
impl ValueExt for Value {
    fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }
    fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const ALICE_ID_STR: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";
    #[test]
    fn predicate_roundtrip_canonicalises_order() {
        let bob_id = crate::account::AccountId::new(
            "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016"
                .parse()
                .expect("public key"),
        )
        .to_string();
        let json = norito::json!({
            "in": [
                {"field": "authority", "values": [ALICE_ID_STR, bob_id]},
                {"field": "metadata.tier", "values": [1, 2, 3]}
            ],
            "equals": [
                {"value": "wonderland", "field": "domain"},
                {"field": "metadata.display_name", "value": "Alice"}
            ],
            "exists": ["metadata.display_name", "metadata.avatar"]
        });
        let parsed = PredicateJson::try_from_value(&json).expect("parse predicate");
        assert_eq!(parsed.equals.len(), 2);
        assert_eq!(parsed.r#in.len(), 2);
        assert_eq!(parsed.exists.len(), 2);
        let first_encode = norito::json::to_json(&parsed.to_value()).expect("encode");
        let second = parsed
            .into_compound::<()>()
            .expect("compound")
            .json_payload()
            .unwrap()
            .to_owned();
        assert_eq!(first_encode, second);
        // Order is canonical; the equals section must be sorted by field name.
        assert!(second.contains("authority"));
        assert!(second.contains("metadata.display_name"));
        assert!(second.contains("metadata.tier"));
    }
    #[test]
    fn predicate_rejects_empty_in_values() {
        let json = norito::json!({
            "in": [
                {"field": "authority", "values": []}
            ]
        });
        let err = PredicateJson::try_from_value(&json).expect_err("should fail");
        assert!(matches!(err, PredicateParseError::EmptyValues(field) if field == "authority"));
    }
    #[test]
    fn owned_predicate_conversion_moves_values_and_obeys_decode_limits() {
        fn value() -> Value {
            norito::json!({
                "equals": [{"field": "metadata.rank", "value": 7}],
                "in": [{"field": "authority", "values": [ALICE_ID_STR]}],
                "exists": ["metadata.rank"]
            })
        }
        let input = value();
        let expected = PredicateJson::try_from_value(&input).expect("borrowed conversion");
        let limits = norito::DecodeLimits::new(64, 4 * 1_024, 256, 4 * 1_024, 16);
        let (actual, usage) = norito::core::with_decode_limits_measured(limits, || {
            PredicateJson::try_from_owned_value(input.clone())
        });
        assert_eq!(actual.expect("owned conversion"), expected);
        assert!(usage.total_allocated_bytes() > 0);
        let denied = norito::core::with_decode_limits(
            norito::DecodeLimits::new(64, 4 * 1_024, 256, 1, 16),
            || {
                PredicateJson::try_from_owned_value(input)
                    .map_err(|error| norito::core::Error::Message(error.to_string()))
            },
        );
        assert!(denied.is_err());
    }
    #[test]
    fn execution_scope_checks_candidate_body_and_restores_legacy_path() {
        let value = norito::json!({"id": "alice", "metadata": {"rank": 7}});
        let canonical = json::to_json(&value).expect("canonical JSON");
        let admitted = with_bounded_predicate_json_execution(canonical.len(), 8 * 1_024, || {
            predicate_json_value_for_execution(&value)
        });
        assert_eq!(admitted, Some(value.clone()));
        let denied = with_bounded_predicate_json_execution(canonical.len() - 1, 8 * 1_024, || {
            predicate_json_value_for_execution(&value)
        });
        assert!(denied.is_none());
        assert_eq!(predicate_json_value_for_execution(&value), Some(value));
    }
    #[test]
    fn execution_scope_uses_owned_predicate_conversion_under_allocation_limit() {
        let predicate = PredicateJson {
            equals: vec![EqualsCondition::new("metadata.rank", Value::from(7_u64))],
            r#in: Vec::new(),
            exists: vec!["metadata.rank".to_owned()],
        };
        let raw = json::to_json(&predicate).expect("canonical predicate JSON");
        let admitted = with_bounded_predicate_json_execution(raw.len(), 8 * 1_024, || {
            predicate_json_from_raw_for_execution(&raw)
        });
        assert_eq!(admitted, Some(predicate));
        let denied = with_bounded_predicate_json_execution(raw.len(), 1, || {
            predicate_json_from_raw_for_execution(&raw)
        });
        assert!(denied.is_none());
    }
    #[test]
    fn predicate_wire_decoders_have_no_borrowed_deep_clone_path() {
        let source = include_str!("../dsl_fast.rs");
        assert!(!source.contains("PredicateJson::try_from_value(&value)"));
        assert!(!source.contains(
            "PredicateJsonPayload::from_predicate(&predicate)\n                    .as_str()",
        ));
    }
    #[test]
    fn predicate_empty_defaults_to_pass() {
        let predicate = PredicateJson::default();
        let compound = predicate.into_compound::<()>().expect("compound predicate");
        assert!(compound.json_payload().is_none());
    }
    #[test]
    fn direct_predicate_json_matches_legacy_value_and_exact_bound() {
        let predicate = PredicateJson {
            equals: vec![
                EqualsCondition::new("z", Value::Bool(true)),
                EqualsCondition::new("a", Value::Number(1_u64.into())),
                EqualsCondition::new("a", Value::Number(2_u64.into())),
            ],
            r#in: vec![
                InCondition::new("z", vec![Value::String("last".to_owned())]),
                InCondition::new("a", vec![Value::String("first".to_owned())]),
            ],
            exists: vec!["z".to_owned(), "a".to_owned(), "a".to_owned()],
        };
        let legacy = json::to_json(&predicate.to_value()).expect("serialize legacy value");
        let direct = json::to_json(&predicate).expect("serialize direct predicate");
        assert_eq!(
            direct, legacy,
            "stable sorting and object-key order changed"
        );
        assert_eq!(
            json::to_json_bounded(&predicate, legacy.len()).expect("serialize at exact bound"),
            legacy
        );
        assert_eq!(
            json::to_json_bounded(&predicate, direct.len() - 1),
            Err(json::BoundedJsonError::BodyTooLarge)
        );
    }
}
