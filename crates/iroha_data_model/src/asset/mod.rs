//! Asset-related types and instructions.

use std::collections::btree_map;

use iroha_primitives::numeric::Numeric;

pub mod alias;
pub mod definition;
pub mod id;
pub mod instructions;
pub mod policy;
pub mod value;

pub use alias::AssetDefinitionAlias;
pub use definition::{AssetBalancePolicy, AssetDefinition, Mintable, NewAssetDefinition};
pub use id::{AssetBalanceScope, AssetDefinitionId, AssetId};
pub use policy::{
    ASSET_ISSUER_USAGE_POLICY_METADATA_KEY, AssetIssuerUsagePolicyV1, AssetSubjectBindingV1,
    DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY, DomainAssetUsagePolicyV1,
};
pub use value::{Asset, AssetEntry, AssetValue};

/// [`AssetTotalQuantityMap`] provides an API to work with collection of key([`AssetDefinitionId`])-value([`Numeric`]) pairs.
pub type AssetTotalQuantityMap = btree_map::BTreeMap<AssetDefinitionId, Numeric>;

/// The prelude re-exports most commonly used traits, structs and macros from this module.
pub mod prelude {
    pub use super::{
        alias::AssetDefinitionAlias,
        definition::{AssetBalancePolicy, AssetDefinition, Mintable, NewAssetDefinition},
        id::{AssetBalanceScope, AssetDefinitionId, AssetId},
        policy::{
            ASSET_ISSUER_USAGE_POLICY_METADATA_KEY, AssetIssuerUsagePolicyV1,
            AssetSubjectBindingV1, DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY,
            DomainAssetUsagePolicyV1,
        },
        value::Asset,
    };
}
