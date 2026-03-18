//! This module contains [`Nft`] structure and it's implementation

use std::{format, str::FromStr, string::String, vec::Vec};

use iroha_data_model_derive::model;

pub use self::model::*;
use crate::{
    IntoKeyValue, Registered, Registrable,
    common::{Owned, Ref, split_nonempty},
    error::ParseError,
    metadata::Metadata,
    prelude::AccountId,
};

#[model]
mod model {
    use derive_more::Constructor;
    use getset::{CopyGetters, Getters};
    use iroha_data_model_derive::{IdEqOrdHash, RegistrableBuilder};
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};

    use super::*;
    use crate::{Identifiable, Name, account::prelude::*, domain::prelude::*};

    /// Identification of an Non Fungible Asset. Consists of Asset name and Domain name.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use iroha_data_model::nft::NftId;
    ///
    /// let nft_id = "nft_name$soramitsu".parse::<NftId>().expect("Valid");
    /// ```
    #[derive(
        derive_more::Debug,
        Clone,
        derive_more::Display,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Constructor,
        Getters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display("{name}${domain}")]
    #[debug("{name}${domain}")]
    #[getset(get = "pub")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct NftId {
        /// Domain id.
        pub domain: DomainId,
        /// NFT name.
        pub name: Name,
    }

    /// Non fungible asset, represents some unique value
    #[derive(
        derive_more::Debug,
        derive_more::Display,
        Clone,
        IdEqOrdHash,
        CopyGetters,
        Getters,
        Decode,
        Encode,
        IntoSchema,
        RegistrableBuilder,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[display("{id}")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct Nft {
        /// An Identification of the [`Nft`].
        pub id: NftId,
        /// Content of the [`Nft`], as a key-value store.
        #[getset(get = "pub")]
        pub content: Metadata,
        /// The account that owns this NFT.
        #[getset(get = "pub")]
        #[registrable_builder(skip, init = authority.clone())]
        pub owned_by: AccountId,
    }
}

string_id!(NftId);

/// Read-only reference to [`Nft`].
/// Used in query filters to avoid copying.
pub type NftEntry<'world> = Ref<'world, NftId, NftValue>;

/// [`Nft`] without `id` field.
/// Needed only for the world-state NFT map to reduce memory usage.
/// In other places use [`Nft`] directly.
#[derive(Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveFastJson,
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct NftData {
    /// Content of the [`Nft`], as a key-value store.
    pub content: Metadata,
    /// The account that owns this NFT.
    pub owned_by: AccountId,
}

/// Wrapper over [`NftData`] used in storages.
pub type NftValue = Owned<NftData>;

impl Nft {
    /// Constructor
    pub fn new(id: NftId, content: Metadata) -> <Self as Registered>::With {
        <Self as Registered>::With::new(id, content)
    }
}

impl NftId {
    /// Convenience alias for [`Self::new`]
    pub fn of(domain: crate::domain::prelude::DomainId, name: crate::Name) -> Self {
        Self::new(domain, name)
    }
}

/// NFT Identification is represented by `name$domain_name` string.
impl FromStr for NftId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name_candidate, domain_id_candidate) = split_nonempty(
            s,
            '$',
            "Non Fungible Asset ID should have format `name$domain`",
            "Empty `name` part in `name$domain`",
            "Empty `domain` part in `name$domain`",
        )?;
        let name = name_candidate.parse().map_err(|_| ParseError {
            reason: "Failed to parse `name` part in `name$domain`",
        })?;
        let domain_id = domain_id_candidate.parse().map_err(|_| ParseError {
            reason: "Failed to parse `domain` part in `name$domain`",
        })?;
        Ok(Self::new(domain_id, name))
    }
}

impl IntoKeyValue for Nft {
    type Key = NftId;
    type Value = NftValue;
    fn into_key_value(self) -> (Self::Key, Self::Value) {
        (
            self.id,
            Owned::new(NftData {
                content: self.content,
                owned_by: self.owned_by,
            }),
        )
    }
}

#[cfg(all(test, feature = "json"))]
mod json_tests {
    use super::*;
    use crate::{Name, domain::prelude::DomainId, metadata::Metadata};

    #[test]
    fn new_nft_json_roundtrip() {
        let domain: DomainId = "art".parse().expect("domain id");
        let id = NftId::new(domain, Name::from_str("mona_lisa").expect("nft name"));
        let mut content = Metadata::default();
        content.insert("artist".parse().expect("metadata key"), "da_vinci");

        let builder = NewNft {
            id: id.clone(),
            content: content.clone(),
        };

        let json = norito::json::to_json(&builder).expect("serialize NFT builder");
        let decoded: NewNft = norito::json::from_json(&json).expect("deserialize NFT builder");

        assert_eq!(decoded.id, id);
        assert_eq!(decoded.content, content);
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::{NewNft, Nft, NftId};
}
