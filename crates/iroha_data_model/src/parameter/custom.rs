//! Custom parameter definitions.
use std::collections::BTreeMap;

use getset::Getters;
use iroha_primitives::json::Json;
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};

use crate::name::Name;

#[cfg(feature = "json")]
pub mod json_helpers {
    //! JSON helper utilities for serializing and deserializing custom parameters.
    use norito::json::{self, JsonDeserialize, JsonSerialize, Parser, Value};

    use super::*;

    /// Serialize a `CustomParameters` map into a JSON object written to `out`.
    pub fn serialize(parameters: &CustomParameters, out: &mut String) {
        out.push('{');
        let mut first = true;
        for (id, parameter) in parameters {
            if first {
                first = false;
            } else {
                out.push(',');
            }
            json::write_json_string(&id.to_string(), out);
            out.push(':');
            JsonSerialize::json_serialize(parameter, out);
        }
        out.push('}');
    }

    /// Deserialize a `CustomParameters` map from a JSON stream.
    ///
    /// # Errors
    ///
    /// Returns [`json::Error`] when the input is not an object or contains
    /// invalid parameter identifiers/payloads.
    pub fn deserialize(parser: &mut Parser<'_>) -> Result<CustomParameters, json::Error> {
        let value = Value::json_deserialize(parser)?;
        from_value(value)
    }

    /// Convert a JSON [`Value`] into `CustomParameters`.
    ///
    /// # Errors
    ///
    /// Returns [`json::Error`] when the value is not an object or individual
    /// parameter entries fail to parse.
    pub fn from_value(value: Value) -> Result<CustomParameters, json::Error> {
        let map = match value {
            Value::Object(map) => map,
            other => {
                return Err(json::Error::Message(format!(
                    "expected object for CustomParameters, got {other:?}"
                )));
            }
        };

        let mut out: CustomParameters = CustomParameters::default();
        for (key, entry) in map {
            let id: CustomParameterId =
                key.parse()
                    .map_err(|err: <CustomParameterId as core::str::FromStr>::Err| {
                        json::Error::InvalidField {
                            field: key.clone(),
                            message: err.to_string(),
                        }
                    })?;

            let entry_json = norito::json::to_json(&entry)
                .map_err(|err| json::Error::Message(err.to_string()))?;
            let mut entry_parser = Parser::new(&entry_json);
            let mut parameter = CustomParameter::json_deserialize(&mut entry_parser)?;
            parameter.id = id.clone();
            out.insert(id, parameter);
        }

        Ok(out)
    }
}

/// Collection of [`CustomParameter`]s
pub(crate) type CustomParameters = BTreeMap<CustomParameterId, CustomParameter>;

pub use self::model::*;

#[iroha_data_model_derive::model]
mod model {
    use derive_more::{Constructor, Display, FromStr};
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};

    use super::*;

    /// Id of a custom parameter
    #[derive(
        Debug,
        Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        FromStr,
        Constructor,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct CustomParameterId(pub Name);

    /// A custom blockchain parameter
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Getters)]
    #[getset(get = "pub")]
    pub struct CustomParameter {
        /// Unique id of the parameter.
        pub id: CustomParameterId,
        /// Payload containing actual value.
        ///
        /// It is JSON-encoded, and its structure must correspond to the structure of
        /// the type defined in [`crate::executor::ExecutorDataModel`].
        pub payload: Json,
    }
}

impl CustomParameterId {
    /// Getter for name
    pub fn name(&self) -> &Name {
        &self.0
    }
}

impl CustomParameter {
    /// Constructor
    pub fn new(id: CustomParameterId, payload: impl Into<Json>) -> Self {
        Self {
            id,
            payload: payload.into(),
        }
    }
}

impl crate::Identifiable for CustomParameter {
    type Id = CustomParameterId;

    fn id(&self) -> &Self::Id {
        &self.id
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for CustomParameterId {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for CustomParameterId {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        Name::json_deserialize(parser).map(Self)
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for CustomParameter {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        json::write_json_string("id", out);
        out.push(':');
        self.id.json_serialize(out);
        out.push(',');
        json::write_json_string("payload", out);
        out.push(':');
        self.payload.json_serialize(out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for CustomParameter {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let mut map = match value {
            json::Value::Object(map) => map,
            _ => {
                return Err(json::Error::InvalidField {
                    field: String::from("CustomParameter"),
                    message: String::from("expected object"),
                });
            }
        };

        let id_value = map.remove("id").ok_or_else(|| json::Error::MissingField {
            field: String::from("id"),
        })?;
        let payload_value = map
            .remove("payload")
            .ok_or_else(|| json::Error::MissingField {
                field: String::from("payload"),
            })?;

        if let Some((field, _)) = map.into_iter().next() {
            return Err(json::Error::UnknownField { field });
        }

        let id = {
            let json_id = norito::json::to_json(&id_value)
                .map_err(|err| json::Error::Message(err.to_string()))?;
            let mut parser = norito::json::Parser::new(&json_id);
            CustomParameterId::json_deserialize(&mut parser)?
        };

        let payload = {
            let json_payload = norito::json::to_json(&payload_value)
                .map_err(|err| json::Error::Message(err.to_string()))?;
            let mut parser = norito::json::Parser::new(&json_payload);
            Json::json_deserialize(&mut parser)?
        };

        Ok(Self { id, payload })
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr as _;

    use super::*;
    use crate::name::Name;

    #[test]
    fn id_name_returns_inner() {
        let id = CustomParameterId::new(Name::from_str("param").expect("Valid"));
        assert_eq!(id.name(), &Name::from_str("param").expect("Valid"));
    }

    #[test]
    fn new_parameter_stores_payload() {
        let id = CustomParameterId::new(Name::from_str("test").expect("Valid"));
        let json = Json::new(10u32);
        let param = CustomParameter::new(id.clone(), json.clone());
        assert_eq!(param.id, id);
        assert_eq!(param.payload(), &json);
    }
}
