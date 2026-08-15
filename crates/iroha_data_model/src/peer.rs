//! This module contains [`Peer`] structure and related implementations and traits implementations.
pub use self::model::*;
use crate::{Identifiable, Registered, error::ParseError};
use derive_more::Constructor;
use iroha_crypto::PublicKey;
use iroha_data_model_derive::model;
use iroha_primitives::addr::SocketAddr;
#[cfg(feature = "json")]
use norito::json::{self, FastJsonWrite, JsonDeserialize};
use norito::literal;
use std::str::FromStr;
#[model]
mod model {
    use super::*;
    use getset::Getters;
    use iroha_data_model_derive::IdEqOrdHash;
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
    pub struct PeerId {
        /// Public Key of the [`Peer`].
        pub public_key: PublicKey,
    }
    /// Representation of other Iroha Peer instances running in separate processes.
    #[derive(
        derive_more::Debug,
        derive_more::Display,
        Clone,
        IdEqOrdHash,
        Decode,
        Encode,
        IntoSchema,
        Getters,
    )]
    #[display("{id}@{address}")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct Peer {
        /// Address of the [`Peer`]'s entrypoint.
        #[getset(get = "pub")]
        pub address: SocketAddr,
        /// Peer Identification.
        #[getset(get = "pub")]
        pub id: PeerId,
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
impl Peer {
    /// Construct `Peer` given `id` and `address`.
    #[inline]
    pub fn new(address: SocketAddr, id: impl Into<PeerId>) -> Self {
        Self {
            address,
            id: id.into(),
        }
    }
}
impl FromStr for Peer {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (public_key_candidate, address_candidate) = match s.rsplit_once('@') {
            None => (s, None),
            Some(("", _)) => {
                return Err(ParseError {
                    reason: "Empty `public_key` part in `public_key@address`",
                });
            }
            Some((_, "")) => {
                return Err(ParseError {
                    reason: "Empty `address` part in `public_key@address`",
                });
            }
            Some((public, addr)) => (public, Some(addr)),
        };
        let public_key: PublicKey = public_key_candidate.parse().map_err(|_| ParseError {
            reason: r#"Failed to parse `public_key` part in `public_key@address`. `public_key` should have multihash format e.g. "ed0120...""#,
        })?;
        let address = if let Some(address_candidate) = address_candidate {
            if let Ok(address) = address_candidate.parse() {
                address
            } else {
                let body = literal::parse("addr", address_candidate).map_err(|_| ParseError {
                    reason: "Failed to parse `address` part in `public_key@address`",
                })?;
                body.parse().map_err(|_| ParseError {
                    reason: "Failed to parse `address` part in `public_key@address`",
                })?
            }
        } else {
            // Allow configs to omit the address and rely on gossip/relay to refresh it later.
            // Default to an inert placeholder that will be replaced once a real connection is seen.
            "0.0.0.0:0".parse().expect("static socket address parses")
        };
        Ok(Self::new(address, public_key))
    }
}
#[cfg(test)]
mod parse_tests {
    use super::*;
    #[test]
    fn peer_from_str_accepts_addr_literal() {
        let key = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
        let literal = literal::format("addr", "127.0.0.1:1337");
        let candidate = format!("{key}@{literal}");
        let peer = Peer::from_str(&candidate).expect("peer parses from addr literal");
        assert_eq!(peer.address().to_string(), "127.0.0.1:1337");
        assert_eq!(peer.id().public_key.to_string(), key);
    }
    #[test]
    fn peer_from_str_rejects_malformed_addr_literal() {
        let key = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
        let mut literal = literal::format("addr", "127.0.0.1:1337");
        literal.pop();
        literal.push('0');
        let candidate = format!("{key}@{literal}");
        assert!(Peer::from_str(&candidate).is_err());
    }
    #[test]
    fn peer_from_str_accepts_bare_public_key() {
        let key = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
        let peer = Peer::from_str(key).expect("peer parses from bare public key");
        assert_eq!(peer.id().public_key.to_string(), key);
        assert_eq!(peer.address().to_string(), "0.0.0.0:0");
    }
}
impl Registered for Peer {
    type With = PeerId;
}
/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::{Peer, PeerId};
}
#[cfg(feature = "json")]
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
#[cfg(feature = "json")]
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

    fn json_from_map_key(key: &str) -> Result<Self, json::Error> {
        peer_id_from_json_str(key)
    }
}

#[cfg(feature = "json")]
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

#[cfg(feature = "json")]
fn invalid_peer_id_json() -> json::Error {
    json::Error::InvalidField {
        field: "peer_id".into(),
        message: "invalid public key".to_owned(),
    }
}
#[cfg(feature = "json")]
impl FastJsonWrite for Peer {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_display_to(self, out)
    }
}
#[cfg(feature = "json")]
impl JsonDeserialize for Peer {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        peer_from_json_str(&value)
    }

    fn json_from_value(value: &json::Value) -> Result<Self, json::Error> {
        let json::Value::String(value) = value else {
            return Err(invalid_peer_json());
        };
        peer_from_json_str(value)
    }

    fn json_from_map_key(key: &str) -> Result<Self, json::Error> {
        peer_from_json_str(key)
    }
}

#[cfg(feature = "json")]
fn peer_from_json_str(value: &str) -> Result<Peer, json::Error> {
    let (public_key, address) = match value.rsplit_once('@') {
        None => (value, None),
        Some(("", _)) | Some((_, "")) => return Err(invalid_peer_json()),
        Some((public_key, address)) => (public_key, Some(address)),
    };
    let public_key = PublicKey::from_canonical_str_for_decode(public_key).map_err(|error| {
        if error.is_decode_resource_limit() {
            json::Error::from_decode_resource(error)
        } else {
            invalid_peer_json()
        }
    })?;
    let address = match address {
        None => SocketAddr::from_str_for_decode("0.0.0.0:0"),
        Some(address) => SocketAddr::from_str_for_decode(address).or_else(|error| {
            if error.is_decode_resource_limit() {
                return Err(error);
            }
            let body = literal::parse_without_diagnostics("addr", address)
                .ok_or(norito::core::Error::LengthMismatch)?;
            SocketAddr::from_str_for_decode(body)
        }),
    }
    .map_err(|error| {
        if error.is_decode_resource_limit() {
            json::Error::from_decode_resource(error)
        } else {
            invalid_peer_json()
        }
    })?;
    Ok(Peer::new(address, public_key))
}

#[cfg(feature = "json")]
fn invalid_peer_json() -> json::Error {
    json::Error::InvalidField {
        field: "peer".into(),
        message: "failed to parse peer public key or address".to_owned(),
    }
}
#[cfg(all(test, feature = "json"))]
mod tests {
    use super::*;
    use iroha_primitives::addr::SocketAddr;
    use norito::json::{self, FastJsonWrite};
    #[test]
    fn peer_json_roundtrip() {
        let pk = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
        let addr: SocketAddr = "127.0.0.1:1337".parse().expect("valid address");
        let peer = Peer::new(addr.clone(), pk.parse::<PublicKey>().expect("valid key"));
        let mut json = String::new();
        peer.write_json(&mut json);
        assert_eq!(json, format!("\"{pk}@{addr}\""));
        let decoded: Peer = json::from_json(&json).expect("deserialize peer");
        assert_eq!(decoded.address(), &addr);
        assert_eq!(decoded.id().public_key, peer.id().public_key);
    }

    #[test]
    fn peer_bounded_json_matches_legacy_at_exact_limit() {
        let pk = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
        let public_key = pk.parse::<PublicKey>().expect("valid key");
        let cases = [
            Peer::new(
                "127.0.0.1:1337".parse::<SocketAddr>().expect("IPv4"),
                public_key.clone(),
            ),
            Peer::new(
                SocketAddr::Host(iroha_primitives::addr::SocketAddrHost {
                    host: "quoted\\\"host.example".into(),
                    port: 1337,
                }),
                public_key,
            ),
        ];
        for peer in cases {
            let expected = json::to_json(&peer).expect("legacy peer JSON");
            assert_eq!(
                json::to_json_bounded(&peer, expected.len()).expect("exact bounded peer JSON"),
                expected
            );
            assert!(matches!(
                json::to_json_bounded(&peer, expected.len() - 1),
                Err(json::BoundedJsonError::BodyTooLarge)
            ));
        }
    }

    #[test]
    fn peer_json_decode_preserves_public_key_and_host_resource_limits() {
        fn limits(bytes: usize) -> norito::core::DecodeLimits {
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
        }

        let key = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
        let host = "source-controlled-hostname.example";
        let candidate = format!("\"{key}@{host}:1337\"");

        let (key_rejected, usage) =
            norito::core::with_decode_limits_measured(limits(candidate.len() - 2), || {
                json::from_str::<Peer>(&candidate)
            });
        assert!(matches!(
            key_rejected,
            Err(json::Error::DecodeResourceLimit)
        ));
        assert_eq!(usage.total_allocated_bytes(), candidate.len() - 2);

        let key_bytes = key.len() / 2 - 2;
        let before_host = candidate.len() - 2 + key_bytes;
        let (host_rejected, usage) =
            norito::core::with_decode_limits_measured(limits(before_host), || {
                json::from_str::<Peer>(&candidate)
            });
        assert!(matches!(
            host_rejected,
            Err(json::Error::DecodeResourceLimit)
        ));
        assert_eq!(usage.total_allocated_bytes(), before_host);

        let exact = before_host + host.len();
        let (decoded, usage) = norito::core::with_decode_limits_measured(limits(exact), || {
            json::from_str::<Peer>(&candidate)
        });
        let decoded = decoded.expect("exact peer decode budget");
        assert_eq!(decoded.address().to_string(), format!("{host}:1337"));
        assert_eq!(usage.total_allocated_bytes(), exact);

        let address = SocketAddr::Host(iroha_primitives::addr::SocketAddrHost {
            host: host.into(),
            port: 1337,
        });
        let literal = address.to_literal();
        let literal_candidate = format!("\"{key}@{literal}\"");
        let literal_exact = literal_candidate.len() - 2 + key_bytes + host.len();
        let (decoded, usage) =
            norito::core::with_decode_limits_measured(limits(literal_exact), || {
                json::from_str::<Peer>(&literal_candidate)
            });
        assert_eq!(
            decoded.expect("literal peer address").address().to_string(),
            format!("{host}:1337")
        );
        assert_eq!(usage.total_allocated_bytes(), literal_exact);
    }

    #[test]
    fn peer_value_and_map_key_decoders_do_not_stage_json_text() {
        fn limits(bytes: usize) -> norito::core::DecodeLimits {
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
        }

        let key = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
        let key_bytes = key.len() / 2 - 2;
        let peer_id_value = json::Value::String(key.to_owned());
        let (peer_id, usage) = norito::core::with_decode_limits_measured(limits(key_bytes), || {
            <PeerId as JsonDeserialize>::json_from_value(&peer_id_value)
        });
        assert_eq!(peer_id.expect("PeerId value").public_key.to_string(), key);
        assert_eq!(usage.total_allocated_bytes(), key_bytes);
        let (peer_id, usage) = norito::core::with_decode_limits_measured(limits(key_bytes), || {
            <PeerId as JsonDeserialize>::json_from_map_key(key)
        });
        assert_eq!(peer_id.expect("PeerId map key").public_key.to_string(), key);
        assert_eq!(usage.total_allocated_bytes(), key_bytes);

        let host = "source-controlled-hostname.example";
        let peer_text = format!("{key}@{host}:1337");
        let exact = key_bytes + host.len();
        let (peer, usage) = norito::core::with_decode_limits_measured(limits(exact), || {
            <Peer as JsonDeserialize>::json_from_map_key(&peer_text)
        });
        assert_eq!(
            peer.expect("Peer map key").address().to_string(),
            format!("{host}:1337")
        );
        assert_eq!(usage.total_allocated_bytes(), exact);
        let peer_value = json::Value::String(peer_text.clone());
        let (peer, usage) = norito::core::with_decode_limits_measured(limits(exact), || {
            <Peer as JsonDeserialize>::json_from_value(&peer_value)
        });
        assert_eq!(
            peer.expect("Peer value").address().to_string(),
            format!("{host}:1337")
        );
        assert_eq!(usage.total_allocated_bytes(), exact);

        let (rejected, usage) =
            norito::core::with_decode_limits_measured(limits(exact - 1), || {
                <Peer as JsonDeserialize>::json_from_map_key(&peer_text)
            });
        assert!(matches!(rejected, Err(json::Error::DecodeResourceLimit)));
        assert_eq!(usage.total_allocated_bytes(), key_bytes);
    }
    #[test]
    fn peer_id_bounded_json_delegates_to_public_key_without_scratch() {
        let literal = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
        let peer_id = PeerId::new(literal.parse::<PublicKey>().expect("valid key"));
        let expected = format!("\"{literal}\"");
        assert_eq!(
            json::to_json_bounded(&peer_id, expected.len()).expect("exact checked PeerId JSON"),
            expected
        );
        assert!(matches!(
            json::to_json_bounded(&peer_id, expected.len() - 1),
            Err(json::BoundedJsonError::BodyTooLarge)
        ));
    }

    #[test]
    fn peer_id_json_decode_preserves_public_key_resource_errors() {
        let literal = "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
        let encoded = format!("\"{literal}\"");
        let limits = norito::core::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            literal.len(),
            usize::MAX,
        );
        let (decoded, usage) = norito::core::with_decode_limits_measured(limits, || {
            json::from_str::<PeerId>(&encoded)
        });
        assert!(matches!(decoded, Err(json::Error::DecodeResourceLimit)));
        assert_eq!(
            usage.total_allocated_bytes(),
            literal.len(),
            "the JSON string allocation should be admitted before PublicKey retention is denied"
        );
    }

    #[test]
    fn peer_id_json_wraps_only_semantic_public_key_errors() {
        let error = json::from_str::<PeerId>(r#""not-a-public-key""#)
            .expect_err("invalid public key must fail");
        assert!(matches!(
            error,
            json::Error::InvalidField { field, .. } if field == "peer_id"
        ));
    }
}
