//! This module contains [`Peer`] structure and related implementations and traits implementations.

use std::str::FromStr;

use derive_more::Constructor;
use iroha_crypto::PublicKey;
use iroha_data_model_derive::model;
use iroha_primitives::addr::SocketAddr;
#[cfg(feature = "json")]
use norito::json::{self, FastJsonWrite, JsonDeserialize};
use norito::literal;

pub use self::model::*;
use crate::{Identifiable, Registered, error::ParseError};

#[model]
mod model {
    use getset::Getters;
    use iroha_data_model_derive::IdEqOrdHash;
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};

    use super::*;

    /// Peer's identification.
    ///
    /// Equality is tested by `public_key` field only.
    /// Each peer should have a unique public key.
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
}

#[cfg(feature = "json")]
impl JsonDeserialize for PeerId {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match value.parse::<PublicKey>() {
            Ok(public_key) => Ok(Self { public_key }),
            Err(err) => Err(json::Error::InvalidField {
                field: "peer_id".into(),
                message: err.to_string(),
            }),
        }
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for Peer {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for Peer {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match value.parse::<Self>() {
            Ok(peer) => Ok(peer),
            Err(err) => Err(json::Error::InvalidField {
                field: "peer".into(),
                message: err.to_string(),
            }),
        }
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use iroha_primitives::addr::SocketAddr;
    use norito::json::{self, FastJsonWrite};

    use super::*;

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
}
