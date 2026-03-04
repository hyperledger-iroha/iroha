//! Asset-related types and instructions.

use std::collections::btree_map;

use iroha_primitives::numeric::Numeric;

pub mod definition;
pub mod id;
pub mod instructions;
pub mod value;

pub use definition::{AssetDefinition, Mintable, NewAssetDefinition};
pub use id::{AssetDefinitionId, AssetId};
pub use value::{Asset, AssetEntry, AssetValue};

/// [`AssetTotalQuantityMap`] provides an API to work with collection of key([`AssetDefinitionId`])-value([`Numeric`]) pairs.
pub type AssetTotalQuantityMap = btree_map::BTreeMap<AssetDefinitionId, Numeric>;

/// The prelude re-exports most commonly used traits, structs and macros from this module.
pub mod prelude {
    pub use super::{
        definition::{AssetDefinition, Mintable, NewAssetDefinition},
        id::{AssetDefinitionId, AssetId},
        value::Asset,
    };
}
