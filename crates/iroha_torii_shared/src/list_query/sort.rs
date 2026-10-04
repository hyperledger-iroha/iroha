//! Sort keys: `field` sorts ascending and `-field` sorts descending.
use super::filter::FieldPath;
use norito::json::{self, JsonDeserialize, JsonSerialize};
use std::fmt;

/// Sort direction.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii_shared::list_query::Order")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Order {
    /// Smallest values first.
    #[default]
    Asc,
    /// Largest values first.
    Desc,
}

impl Order {
    /// Whether this is [`Order::Asc`].
    pub const fn is_ascending(self) -> bool {
        matches!(self, Self::Asc)
    }
}

/// One sort key such as `-quantity`.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii_shared::list_query::SortKey")]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SortKey {
    /// Field to sort by.
    pub key: FieldPath,
    /// Sort direction.
    pub order: Order,
}

impl SortKey {
    /// Ascending key.
    pub fn asc(key: impl Into<FieldPath>) -> Self {
        Self {
            key: key.into(),
            order: Order::Asc,
        }
    }

    /// Descending key.
    pub fn desc(key: impl Into<FieldPath>) -> Self {
        Self {
            key: key.into(),
            order: Order::Desc,
        }
    }

    /// Parse one key such as `-quantity`.
    ///
    /// # Errors
    /// Returns the syntax error from [`super::text::parse_sort`], or an error
    /// when the text holds more than one key.
    pub fn parse(text: &str) -> Result<Self, super::text::FilterSyntaxError> {
        let mut keys = super::text::parse_sort(text)?;
        if keys.len() == 1 {
            Ok(keys.remove(0))
        } else {
            Err(super::text::FilterSyntaxError::at(
                text,
                0,
                "expected exactly one sort key; pass each key as its own array element",
            ))
        }
    }
}

impl fmt::Display for SortKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.order == Order::Desc {
            f.write_str("-")?;
        }
        self.key.fmt(f)
    }
}

/// Render keys as a comma-separated specification such as `-quantity,id`.
pub fn sort_to_string(keys: &[SortKey]) -> String {
    keys.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

impl JsonSerialize for SortKey {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl JsonDeserialize for SortKey {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let text = String::json_deserialize(parser)?;
        Self::parse(&text)
            .map_err(|err| json::Error::Message(format!("invalid sort key `{text}`: {err}")))
    }
}

impl norito::core::SerializePayload for Order {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let tag = match self {
            Self::Asc => 0u8,
            Self::Desc => 1u8,
        };
        <u8 as norito::core::SerializePayload>::serialize(&tag, writer)
    }
}

impl<'de> norito::core::DeserializePayload<'de> for Order {
    fn try_deserialize(
        archived: &'de norito::core::Archived<Order>,
    ) -> Result<Self, norito::core::Error> {
        let archived_tag: &norito::core::Archived<u8> = archived.cast();
        match <u8 as norito::core::DeserializePayload>::try_deserialize(archived_tag)? {
            0 => Ok(Self::Asc),
            1 => Ok(Self::Desc),
            other => Err(norito::core::Error::Message(format!(
                "invalid Order tag: {other}"
            ))),
        }
    }

    fn deserialize(archived: &'de norito::core::Archived<Order>) -> Self {
        Self::try_deserialize(archived).expect("Order should decode from variant tag")
    }
}

impl norito::core::SerializePayload for SortKey {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let payload = (self.key.clone(), self.order);
        <(FieldPath, Order) as norito::core::SerializePayload>::serialize(&payload, writer)
    }
}

impl<'de> norito::core::DeserializePayload<'de> for SortKey {
    fn try_deserialize(
        archived: &'de norito::core::Archived<SortKey>,
    ) -> Result<Self, norito::core::Error> {
        let archived_pair: &norito::core::Archived<(FieldPath, Order)> = archived.cast();
        let (key, order) =
            <(FieldPath, Order) as norito::core::DeserializePayload>::try_deserialize(
                archived_pair,
            )?;
        Ok(Self { key, order })
    }

    fn deserialize(archived: &'de norito::core::Archived<SortKey>) -> Self {
        Self::try_deserialize(archived).expect("SortKey should decode from (FieldPath, Order)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_form_is_the_text_key() {
        let key = SortKey::desc("alias_binding.bound_at_ms");
        let encoded = json::to_json(&key).expect("serialize");
        assert_eq!(encoded, "\"-alias_binding.bound_at_ms\"");
        assert_eq!(json::from_str::<SortKey>(&encoded).expect("decode"), key);
        let err = json::from_str::<SortKey>("\"id:desc\"").expect_err("legacy spelling");
        assert!(err.to_string().contains("`-field`"), "{err}");
    }

    #[test]
    fn display_and_spec_rendering() {
        let keys = [SortKey::desc("quantity"), SortKey::asc("metadata.ui-order")];
        assert_eq!(sort_to_string(&keys), "-quantity,metadata.`ui-order`");
        assert_eq!(
            super::super::text::parse_sort(&sort_to_string(&keys)).expect("roundtrip"),
            keys
        );
    }

    #[test]
    fn binary_payload_roundtrips() {
        let key = SortKey::desc("timestamp_ms");
        let bytes = norito::codec::encode_adaptive(&key);
        assert_eq!(
            norito::codec::decode_adaptive::<SortKey>(&bytes).expect("decode"),
            key
        );
        assert!(norito::codec::decode_adaptive::<Order>(&[2]).is_err());
    }
}
