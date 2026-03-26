//! Helper types for expressing lightweight JSON predicates.
//!
//! These helpers build canonical JSON payloads that the query DSL can embed in
//! [`CompoundPredicate`] values.  The structures keep field ordering stable so
//! serialised predicates are deterministic and easy to compare in tests or
//! caches.

use std::{
    string::{String, ToString},
    vec::Vec,
};

use norito::json::{self, JsonDeserialize, JsonSerialize, Map, Value};
use thiserror::Error;

use crate::query::dsl::CompoundPredicate;

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
        self.equals.sort_by(|a, b| a.field.cmp(&b.field));
        self.r#in.sort_by(|a, b| a.field.cmp(&b.field));
        self.exists.sort();
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

    /// Convert into a [`CompoundPredicate`] by serialising the canonical JSON
    /// representation. The resulting predicate carries the JSON payload that
    /// backends can interpret later.
    ///
    /// # Errors
    /// Returns an error when serialisation of the canonical JSON representation fails.
    pub fn into_compound<T>(&self) -> Result<CompoundPredicate<T>, json::Error> {
        if self.is_empty() {
            Ok(CompoundPredicate::PASS)
        } else {
            json::from_value(self.to_value())
        }
    }
}

impl JsonSerialize for PredicateJson {
    fn json_serialize(&self, out: &mut String) {
        self.to_value().json_serialize(out);
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

    const ALICE_ID_STR: &str = "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ";
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
    fn predicate_empty_defaults_to_pass() {
        let predicate = PredicateJson::default();
        let compound = predicate.into_compound::<()>().expect("compound predicate");
        assert!(compound.json_payload().is_none());
    }
}
