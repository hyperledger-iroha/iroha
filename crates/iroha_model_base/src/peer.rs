//! Canonical peer identities, public-key codecs and bounded JSON.

pub use self::model::PeerId;
use derive_more::Constructor;
use iroha_crypto::PublicKey;
use iroha_data_model_derive::model;
use norito::json::{self, FastJsonWrite, JsonDeserialize};
use std::str::FromStr;

#[model]
mod model {
    use super::*;
    use getset::Getters;
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};
    /// Peer's identification.
    ///
    /// Equality is tested by `public_key` field only. Each peer should have a unique public key.
    #[derive(
        derive_more::Debug,
        derive_more::Display,
        Clone,
        Constructor,
        Ord,
        PartialOrd,
        Eq,
        PartialEq,
        Hash,
        Decode,
        Encode,
        IntoSchema,
        Getters,
    )]
    #[display("{public_key}")]
    #[debug("{public_key}")]
    #[getset(get = "pub")]
    #[repr(transparent)]
    #[cfg_attr(
        any(feature = "ffi_export", feature = "ffi_import"),
        ffi_type(unsafe {robust})
    )]
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::peer::model::PeerId")]
    pub struct PeerId {
        /// Public key identifying this peer.
        pub public_key: PublicKey,
    }
}

impl FromStr for PeerId {
    type Err = iroha_crypto::error::ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        PublicKey::from_str(s).map(Self::new)
    }
}

impl From<PublicKey> for PeerId {
    fn from(public_key: PublicKey) -> Self {
        Self { public_key }
    }
}

impl FastJsonWrite for PeerId {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&self.public_key.to_string(), out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        norito::json::JsonSerialize::json_serialize_to(&self.public_key, out)
    }
}

impl JsonDeserialize for PeerId {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        peer_id_from_json_str(&value)
    }

    fn json_from_value(value: &json::Value) -> Result<Self, json::Error> {
        let json::Value::String(value) = value else {
            return Err(invalid_peer_id_json());
        };
        peer_id_from_json_str(value)
    }
}

impl norito::json::JsonObjectKey for PeerId {
    fn visit_json_key_text<E>(&self, visitor: impl FnMut(&str) -> Result<(), E>) -> Result<(), E> {
        norito::json::JsonObjectKey::visit_json_key_text(&self.public_key, visitor)
    }
    fn visit_json_key_text_checked(
        &self,
        visitor: impl FnMut(&str) -> Result<(), json::BoundedJsonError>,
    ) -> Result<(), json::BoundedJsonError> {
        norito::json::JsonObjectKey::visit_json_key_text_checked(&self.public_key, visitor)
    }
}

impl norito::json::JsonObjectKeyOwned for PeerId {
    fn from_json_key_text(key: &str) -> Result<Self, json::Error> {
        peer_id_from_json_str(key)
    }
}

fn peer_id_from_json_str(value: &str) -> Result<PeerId, json::Error> {
    PublicKey::from_canonical_str_for_decode(value)
        .map(PeerId::new)
        .map_err(|error| {
            if error.is_decode_resource_limit() {
                json::Error::from_decode_resource(error)
            } else {
                invalid_peer_id_json()
            }
        })
}

fn invalid_peer_id_json() -> json::Error {
    json::Error::InvalidField {
        field: "peer_id".into(),
        message: "invalid public key".to_owned(),
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for PeerId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        norito::core::decode_field_canonical::<Self>(bytes)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod wire_identity_tests;
