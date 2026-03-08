//! Asset identifiers.

use std::{
    cmp::Ordering,
    fmt, format,
    hash::{Hash, Hasher},
    str::FromStr,
    string::String,
};

use derive_more::Constructor;
use getset::Getters;
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};

pub use self::model::*;
use crate::{
    Name, account::prelude::*, common::split_nonempty, domain::prelude::*, error::ParseError,
};

#[model]
mod model {
    use super::*;

    /// Identification of an Asset Definition. Consists of asset name and domain name.
    ///
    /// Asset names are compared case-insensitively (ASCII) for equality, ordering and hashing.
    /// This ensures asset definition IDs are unique per domain regardless of name casing (e.g.
    /// `usd#acme` and `USD#acme` refer to the same asset definition).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use iroha_data_model::asset::id::AssetDefinitionId;
    ///
    /// let definition_id = "xor#soramitsu".parse::<AssetDefinitionId>().expect("Valid");
    /// ```
    #[derive(
        derive_more::Debug,
        Clone,
        derive_more::Display,
        Constructor,
        Getters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display("{name}#{domain}")]
    #[debug("{name}#{domain}")]
    #[getset(get = "pub")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AssetDefinitionId {
        /// Domain id.
        pub domain: DomainId,
        /// Asset name.
        pub name: Name,
    }

    /// Identification of an asset combines the entity identifier ([`AssetId`]) with the owner [`AccountId`].
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Getters, Decode, Encode, IntoSchema)]
    #[getset(get = "pub")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AssetId {
        /// Account Identification.
        pub account: AccountId,
        /// Entity Identification.
        pub definition: AssetDefinitionId,
    }
}

string_id!(AssetDefinitionId);

fn cmp_ignore_ascii_case(a: &str, b: &str) -> Ordering {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let shared = core::cmp::min(a_bytes.len(), b_bytes.len());
    for i in 0..shared {
        let a_fold = a_bytes[i].to_ascii_lowercase();
        let b_fold = b_bytes[i].to_ascii_lowercase();
        match a_fold.cmp(&b_fold) {
            Ordering::Equal => {}
            non_eq => return non_eq,
        }
    }
    a_bytes.len().cmp(&b_bytes.len())
}

impl PartialEq for AssetDefinitionId {
    fn eq(&self, other: &Self) -> bool {
        self.domain == other.domain && self.name.as_ref().eq_ignore_ascii_case(other.name.as_ref())
    }
}

impl Eq for AssetDefinitionId {}

impl PartialOrd for AssetDefinitionId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AssetDefinitionId {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.domain.cmp(&other.domain) {
            Ordering::Equal => cmp_ignore_ascii_case(self.name.as_ref(), other.name.as_ref()),
            non_eq => non_eq,
        }
    }
}

impl Hash for AssetDefinitionId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.domain.hash(state);
        let name = self.name.as_ref();
        // Preserve structural hashing (like `str`) without allocating a lowercase String.
        name.len().hash(state);
        for byte in name.as_bytes() {
            state.write_u8(byte.to_ascii_lowercase());
        }
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for AssetId {
    fn write_json(&self, out: &mut String) {
        let literal = self.canonical_encoded();
        norito::json::JsonSerialize::json_serialize(&literal, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for AssetId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        AssetId::parse_encoded(&value).map_err(|err| norito::json::Error::Message(err.to_string()))
    }
}

impl AssetId {
    /// Create a new [`AssetId`]
    pub fn new(definition: AssetDefinitionId, account: AccountId) -> Self {
        Self {
            account,
            definition,
        }
    }

    /// Convenience alias for [`Self::new`]
    pub fn of(definition: AssetDefinitionId, account: AccountId) -> Self {
        Self::new(definition, account)
    }

    /// Parse an encoded asset identifier from `norito:<hex>`.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the value is empty, not prefixed with
    /// `norito:`, contains invalid hex, or does not decode into [`AssetId`].
    pub fn parse_encoded(input: &str) -> Result<Self, ParseError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ParseError {
                reason: "Asset ID must not be empty",
            });
        }
        if trimmed.contains('#') {
            return Err(ParseError {
                reason: "Asset ID textual forms are not supported; use encoded `norito:<hex>`",
            });
        }

        let prefix = "norito:";
        let Some(payload_hex) = trimmed
            .get(..prefix.len())
            .filter(|head| head.eq_ignore_ascii_case(prefix))
            .map(|_| &trimmed[prefix.len()..])
        else {
            return Err(ParseError {
                reason: "Asset ID must use encoded `norito:<hex>` format",
            });
        };
        if payload_hex.is_empty() {
            return Err(ParseError {
                reason: "Asset ID must include hex payload after `norito:`",
            });
        }

        let payload = hex::decode(payload_hex).map_err(|_| ParseError {
            reason: "Asset ID `norito:` payload must be valid hex",
        })?;
        norito::decode_from_bytes::<Self>(&payload).map_err(|_| ParseError {
            reason: "Asset ID `norito:` payload is invalid",
        })
    }

    /// Render this identifier in the canonical encoded `norito:<hex>` form.
    #[must_use]
    pub fn canonical_encoded(&self) -> String {
        // `parse_encoded` expects header-framed Norito bytes.
        let payload = to_bytes(self).expect("asset id encoding should not fail");
        format!("norito:{}", hex::encode(payload))
    }
}

impl AssetDefinitionId {
    /// Convenience alias for [`Self::new`]
    pub fn of(domain: DomainId, name: Name) -> Self {
        Self::new(domain, name)
    }
}

/// Asset Definition Identification is represented by `name#domain_name` string.
impl FromStr for AssetDefinitionId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name_candidate, domain_id_candidate) = split_nonempty(
            s,
            '#',
            "Asset Definition ID should have format `name#domain`",
            "Empty `name` part in `name#domain`",
            "Empty `domain` part in `name#domain`",
        )?;
        let name = name_candidate.parse().map_err(|_| ParseError {
            reason: "Failed to parse `name` part in `name#domain`",
        })?;
        let domain_id = domain_id_candidate.parse().map_err(|_| ParseError {
            reason: "Failed to parse `domain` part in `name#domain`",
        })?;
        Ok(Self::new(domain_id, name))
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical_encoded())
    }
}

impl fmt::Debug for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical_encoded())
    }
}

impl FromStr for AssetId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_encoded(s)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cmp::Ordering,
        collections::BTreeSet,
        hash::{Hash as _, Hasher as _},
    };

    use iroha_crypto::KeyPair;

    use super::*;
    use crate::account::AccountId;

    #[test]
    fn debug_formats_without_recursion() {
        let kp = KeyPair::random();
        let domain: DomainId = "domain".parse().unwrap();
        let account: AccountId = AccountId::new(domain.clone(), kp.public_key().clone());
        let def: AssetDefinitionId = "xor#domain".parse().unwrap();
        let id = AssetId::new(def, account);
        let s = format!("{id:?}");
        // Should be the canonical encoded literal and not recurse.
        assert!(s.starts_with("norito:"));
    }

    #[test]
    fn asset_definition_id_name_is_case_insensitive() {
        let lower: AssetDefinitionId = "usd#acme".parse().unwrap();
        let upper: AssetDefinitionId = "USD#acme".parse().unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower.cmp(&upper), Ordering::Equal);

        let mut hasher_lower = std::collections::hash_map::DefaultHasher::new();
        lower.hash(&mut hasher_lower);
        let mut hasher_upper = std::collections::hash_map::DefaultHasher::new();
        upper.hash(&mut hasher_upper);
        assert_eq!(hasher_lower.finish(), hasher_upper.finish());

        let mut set = BTreeSet::new();
        set.insert(lower);
        set.insert(upper);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn asset_id_parse_encoded_roundtrips() {
        let kp = KeyPair::random();
        let account_domain: DomainId = "wonderland".parse().expect("domain");
        let account = AccountId::new(account_domain.clone(), kp.public_key().clone());
        let definition: AssetDefinitionId = "xor#wonderland".parse().expect("definition");
        let id = AssetId::new(definition, account);

        let encoded = id.canonical_encoded();
        let parsed = AssetId::parse_encoded(&encoded).expect("encoded asset id parses");
        assert_eq!(parsed, id);
    }

    #[test]
    fn asset_id_parse_encoded_rejects_textual_literal() {
        let kp = KeyPair::random();
        let account_domain: DomainId = "wonderland".parse().expect("domain");
        let account = AccountId::new(account_domain.clone(), kp.public_key().clone());
        let literal = format!("xor#wonderland#{account}");

        let err = AssetId::parse_encoded(&literal).expect_err("textual literal must fail");
        assert!(
            err.reason().contains("textual forms are not supported"),
            "unexpected error: {err}"
        );
    }
}
