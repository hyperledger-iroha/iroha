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
use norito::codec::{Decode, Encode};

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
        let ih58 = self
            .account
            .canonical_ih58()
            .expect("AssetId account should encode to IH58 for JSON");
        let account_literal = format!("{ih58}@{}", self.account.domain());
        let literal = if self.definition.domain == self.account.domain {
            format!("{}##{account_literal}", self.definition.name)
        } else {
            format!("{}#{account_literal}", self.definition)
        };
        norito::json::JsonSerialize::json_serialize(&literal, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for AssetId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        value
            .parse::<AssetId>()
            .map_err(|err| norito::json::Error::Message(err.to_string()))
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
        if self.definition.domain == self.account.domain {
            write!(f, "{}##{}", self.definition.name, self.account)
        } else {
            write!(f, "{}#{}", self.definition, self.account)
        }
    }
}

impl fmt::Debug for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Avoid relying on the Debug formatter for `self` to prevent any
        // accidental recursive formatting. Mirror the Display output explicitly.
        if self.definition.domain == self.account.domain {
            write!(f, "{}##{}", self.definition.name, self.account)
        } else {
            write!(f, "{}#{}", self.definition, self.account)
        }
    }
}

impl FromStr for AssetId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (definition_id_candidate, account_id_candidate) =
            s.rsplit_once('#').ok_or(ParseError {
            reason: "Asset ID should have format `asset#domain#account`, or `asset##account` for the same domains",
        })?;
        let account_id = account_id_candidate.parse::<AccountId>().map_err(|_| ParseError {
            reason: "Failed to parse `account` part in `asset#domain#account` or `asset##account`",
        })?;
        let domain_complement = if definition_id_candidate.ends_with('#') {
            account_id.domain.name.as_ref()
        } else {
            ""
        };
        let definition_id = format!("{definition_id_candidate}{domain_complement}").parse().map_err(|_| ParseError {
            reason: "Failed to parse `asset#domain` (or `asset#`) part in `asset#domain#account` (or `asset##account`)",
        })?;
        Ok(Self::new(definition_id, account_id))
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
        // Should contain the account and asset parts and not crash
        assert!(s.contains("xor"));
        assert!(s.contains('#'));
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
}
