//! Asset-related types and instructions.

use std::collections::btree_map;

use iroha_primitives::numeric::Quantity;

pub mod alias;
pub mod definition;
pub mod id;
pub mod instructions;
pub mod policy;
pub mod transfer_control;
pub mod value;

pub use alias::{AssetDefinitionAlias, ResolvedAssetDefinitionAliasV1};
pub use definition::{AssetBalancePolicy, AssetDefinition, Mintable, NewAssetDefinition};
pub use id::{AssetBalanceScope, AssetDefinitionId, AssetId};
pub use policy::{
    ASSET_ISSUER_USAGE_POLICY_METADATA_KEY, AssetIssuerUsagePolicyV1, AssetSubjectBindingV1,
    DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY, DomainAssetUsagePolicyV1,
};
pub use transfer_control::{
    ASSET_TRANSFER_AVAILABILITY_MAX_REASON_BYTES_V1, ASSET_TRANSFER_CONTROL_METADATA_KEY,
    AssetTransferAvailability, AssetTransferControlRecord, AssetTransferControlStoreV1,
    AssetTransferControlWindow, AssetTransferLimit, AssetTransferUsageBucket,
    validate_asset_transfer_availability_reason,
};
pub use value::{Asset, AssetEntry, AssetValue};

/// [`AssetTotalQuantityMap`] stores canonical non-negative totals by asset definition.
pub type AssetTotalQuantityMap = btree_map::BTreeMap<AssetDefinitionId, Quantity>;

/// The prelude re-exports most commonly used traits, structs and macros from this module.
pub mod prelude {
    pub use super::{
        alias::{AssetDefinitionAlias, ResolvedAssetDefinitionAliasV1},
        definition::{AssetBalancePolicy, AssetDefinition, Mintable, NewAssetDefinition},
        id::{AssetBalanceScope, AssetDefinitionId, AssetId},
        policy::{
            ASSET_ISSUER_USAGE_POLICY_METADATA_KEY, AssetIssuerUsagePolicyV1,
            AssetSubjectBindingV1, DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY,
            DomainAssetUsagePolicyV1,
        },
        transfer_control::{
            ASSET_TRANSFER_AVAILABILITY_MAX_REASON_BYTES_V1, ASSET_TRANSFER_CONTROL_METADATA_KEY,
            AssetTransferAvailability, AssetTransferControlRecord, AssetTransferControlStoreV1,
            AssetTransferControlWindow, AssetTransferLimit, AssetTransferUsageBucket,
            validate_asset_transfer_availability_reason,
        },
        value::Asset,
    };
}
