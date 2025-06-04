//! This module contains [`Nft`] structure and it's implementation

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
use core::str::FromStr;

use iroha_data_model_derive::model;

pub use self::model::*;
use crate::{metadata::Metadata, prelude::AccountId, ParseError, Registered};

#[model]
mod model {
    use derive_more::{Constructor, DebugCustom, Display};
    use getset::{CopyGetters, Getters};
    use iroha_data_model_derive::IdEqOrdHash;
    use iroha_schema::IntoSchema;
    use parity_scale_codec::{Decode, Encode};
    use serde::{Deserialize, Serialize};
    use serde_with::{DeserializeFromStr, SerializeDisplay};

    use super::*;
    use crate::{account::prelude::*, domain::prelude::*, Identifiable, Name};

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
        DebugCustom,
        Clone,
        Display,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Constructor,
        Getters,
        Decode,
        Encode,
        DeserializeFromStr,
        SerializeDisplay,
        IntoSchema,
    )]
    #[display(fmt = "{name}${domain}")]
    #[debug(fmt = "{name}${domain}")]
    #[getset(get = "pub")]
    #[ffi_type]
    pub struct NftId {
        /// Domain id.
        pub domain: DomainId,
        /// NFT name.
        pub name: Name,
    }

    /// Non fungible asset, represents some unique value
    #[derive(
        Debug,
        Display,
        Clone,
        IdEqOrdHash,
        CopyGetters,
        Getters,
        Decode,
        Encode,
        Deserialize,
        Serialize,
        IntoSchema,
    )]
    #[display(fmt = "{id}")]
    #[ffi_type]
    pub struct Nft {
        /// An Identification of the [`Nft`].
        pub id: NftId,
        /// Content of the [`Nft`], as a key-value store.
        #[getset(get = "pub")]
        pub content: Metadata,
        /// The account that owns this NFT.
        #[getset(get = "pub")]
        pub owned_by: AccountId,
    }

    /// Read-only reference to [`Nft`].
    /// Used in query filters to avoid copying.
    #[derive(Copy, Clone)]
    pub struct NftEntry<'world> {
        /// An Identification of the [`Nft`].
        pub id: &'world NftId,
        /// Content of the [`Nft`], as a key-value store.
        pub content: &'world Metadata,
        /// The account that owns this NFT.
        pub owned_by: &'world AccountId,
    }

    /// [`Nft`] without `id` field.
    /// Needed only for [`World::nfts`] map to reduce memory usage.
    /// In other places use [`Nft`] directly.
    #[derive(Clone, Deserialize, Serialize)]
    pub struct NftValue {
        /// Content of the [`Nft`], as a key-value store.
        pub content: Metadata,
        /// The account that owns this NFT.
        pub owned_by: AccountId,
    }

    /// Builder which can be submitted in a transaction to create a new [`Nft`]
    #[derive(
        Debug, Display, Clone, IdEqOrdHash, Decode, Encode, Deserialize, Serialize, IntoSchema,
    )]
    #[display(fmt = "{id}")]
    #[serde(rename = "Nft")]
    #[ffi_type]
    pub struct NewNft {
        /// An Identification of the [`Nft`].
        pub id: NftId,
        /// Content of the [`Nft`], as a key-value store.
        pub content: Metadata,
    }
}

impl Nft {
    /// Constructor
    pub fn new(id: NftId, content: Metadata) -> <Self as Registered>::With {
        NewNft { id, content }
    }
}

/// NFT Identification is represented by `name$domain_name` string.
impl FromStr for NftId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.rsplit_once('$') {
            None => Err(ParseError {
                reason: "Non Fungible Asset ID should have format `name$domain`",
            }),
            Some(("", _)) => Err(ParseError {
                reason: "Empty `name` part in `name$domain`",
            }),
            Some((_, "")) => Err(ParseError {
                reason: "Empty `domain` part in `name$domain`",
            }),
            Some((name_candidate, domain_id_candidate)) => {
                let name = name_candidate.parse().map_err(|_| ParseError {
                    reason: "Failed to parse `name` part in `name$domain`",
                })?;
                let domain_id = domain_id_candidate.parse().map_err(|_| ParseError {
                    reason: "Failed to parse `domain` part in `name$domain`",
                })?;
                Ok(Self::new(domain_id, name))
            }
        }
    }
}

impl Registered for Nft {
    type With = NewNft;
}

impl<'world> NftEntry<'world> {
    /// Constructor
    pub fn new(id: &'world NftId, value: &'world NftValue) -> Self {
        Self {
            id,
            content: &value.content,
            owned_by: &value.owned_by,
        }
    }

    /// Getter for `id`
    pub fn id(&self) -> &NftId {
        self.id
    }

    /// Getter for `content`
    pub fn content(&self) -> &Metadata {
        self.content
    }

    /// Getter for `owned_by`
    pub fn owned_by(&self) -> &AccountId {
        self.owned_by
    }
}

impl From<NftEntry<'_>> for Nft {
    fn from(value: NftEntry) -> Self {
        Self {
            id: value.id.clone(),
            content: value.content.clone(),
            owned_by: value.owned_by.clone(),
        }
    }
}

impl From<&Nft> for NftValue {
    fn from(nft: &Nft) -> Self {
        Self {
            content: nft.content.clone(),
            owned_by: nft.owned_by.clone(),
        }
    }
}

impl From<Nft> for NftValue {
    fn from(nft: Nft) -> Self {
        Self {
            content: nft.content,
            owned_by: nft.owned_by,
        }
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::{NewNft, Nft, NftId};
}
