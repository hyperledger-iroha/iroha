//! Permission Token and related impls
use std::{collections::BTreeSet, format, string::String, vec::Vec};

use getset::Getters;
use iroha_data_model_derive::model;
use iroha_primitives::json::Json;
use iroha_schema::{Ident, IntoSchema};

pub use self::model::*;

/// Collection of [`Permission`]s
pub type Permissions = BTreeSet<Permission>;

#[model]
mod model {
    use derive_more::Display;
    use norito::codec::{Decode, Encode};

    use super::*;

    /// Stored proof of the account having a permission for a certain action.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Display, Getters,
    )]
    #[cfg_attr(feature = "json", derive(crate::DeriveJsonSerialize))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[display("{name}({payload})")]
    pub struct Permission {
        /// Refers to a type defined in [`crate::executor::ExecutorDataModel`].
        #[getset(skip)]
        pub name: Ident,
        /// Payload containing actual value.
        ///
        /// It is JSON-encoded, and its structure must correspond to the structure of
        /// the type defined in [`crate::executor::ExecutorDataModel`].
        #[getset(get = "pub")]
        pub payload: Json,
    }
}

impl Permission {
    /// Constructor
    pub fn new(name: Ident, payload: impl Into<Json>) -> Self {
        Self {
            name,
            payload: payload.into(),
        }
    }

    /// Refers to a type defined in [`crate::executor::ExecutorDataModel`].
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(feature = "json")]
fn minify_json_whitespace(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_string = false;
    let mut escaped = false;

    for ch in raw.chars() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            ' ' | '\n' | '\r' | '\t' => {}
            _ => out.push(ch),
        }
    }

    out
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for Permission {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::{MapVisitor, RawValue};

        let mut visitor = MapVisitor::new(parser)?;
        let mut name: Option<Ident> = None;
        let mut payload: Option<Json> = None;

        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "name" => {
                    if name.is_some() {
                        return Err(norito::json::Error::duplicate_field("name"));
                    }
                    name = Some(visitor.parse_value::<Ident>()?);
                }
                "payload" => {
                    if payload.is_some() {
                        return Err(norito::json::Error::duplicate_field("payload"));
                    }
                    let raw = visitor.parse_value::<RawValue>()?;
                    payload = Some(Json::from_string_unchecked(minify_json_whitespace(
                        raw.as_str(),
                    )));
                }
                _ => visitor.skip_value()?,
            }
        }
        visitor.finish()?;

        let name = name.ok_or_else(|| norito::json::Error::missing_field("name"))?;
        let payload = payload.ok_or_else(|| norito::json::Error::missing_field("payload"))?;

        Ok(Self { name, payload })
    }
}

pub mod prelude {
    //! The prelude re-exports most commonly used traits, structs and macros from this crate.
    pub use super::Permission;
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::json::JsonDeserialize as _;

    fn deserialize_permission_with_parser(raw: &str) -> Result<Permission, norito::json::Error> {
        let mut parser = norito::json::Parser::new(raw);
        let permission = Permission::json_deserialize(&mut parser)?;
        parser.skip_ws();
        assert!(
            parser.eof(),
            "permission parser should consume the full input"
        );
        Ok(permission)
    }

    #[test]
    fn permission_deserialization_minifies_payload_whitespace() {
        let decoded: Permission = norito::json::from_str(
            r#"{
                "name": "CanDoThing",
                "payload": {  "k" : 1 }
            }"#,
        )
        .expect("deserialize permission");
        let canonical = Permission::new(
            "CanDoThing".into(),
            Json::from_string_unchecked("{\"k\":1}".to_owned()),
        );

        assert_eq!(decoded.payload().get(), "{\"k\":1}");
        assert_eq!(decoded, canonical);
    }

    #[test]
    fn permission_deserialization_keeps_duplicate_payload_keys_distinct() {
        let stored =
            deserialize_permission_with_parser(r#"{"name":"CanDoThing","payload":{"k":0,"k":1}}"#)
                .expect("deserialize permission");
        let canonical = Permission::new(
            "CanDoThing".into(),
            Json::from_string_unchecked("{\"k\":1}".to_owned()),
        );

        assert_eq!(stored.payload().get(), "{\"k\":0,\"k\":1}");
        assert_ne!(stored, canonical);
        assert!(!BTreeSet::from([stored]).contains(&canonical));
    }

    #[test]
    fn permission_deserialization_rejects_duplicate_top_level_fields() {
        let duplicate_name = deserialize_permission_with_parser(
            r#"{"name":"CanDoThing","name":"OtherThing","payload":null}"#,
        )
        .expect_err("duplicate name must fail");
        let duplicate_payload = deserialize_permission_with_parser(
            r#"{"name":"CanDoThing","payload":null,"payload":{}}"#,
        )
        .expect_err("duplicate payload must fail");

        assert!(
            duplicate_name
                .to_string()
                .contains("duplicate field `name`")
        );
        assert!(
            duplicate_payload
                .to_string()
                .contains("duplicate field `payload`")
        );
    }
}
