//! Asset values and entries.

use derive_more::Display;
use getset::Getters;
use iroha_data_model_derive::{IdEqOrdHash, model};
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

pub use self::model::*;
use super::id::AssetId;
use crate::{Identifiable, IntoKeyValue, Registered};

#[model]
mod model {
    use super::*;

    /// Asset represents some sort of commodity or value.
    /// All possible variants of [`Asset`] entity's components.
    #[derive(Debug, Display, Clone, IdEqOrdHash, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[display("{id}: {value}")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct Asset {
        /// Component Identification.
        pub id: AssetId,
        /// Asset's Quantity.
        #[getset(get = "pub")]
        pub value: Numeric,
    }
}

use crate::common::{Owned, Ref};

/// Read-only reference to [`Asset`].
/// Used in query filters to avoid copying.
pub type AssetEntry<'world> = Ref<'world, AssetId, AssetValue>;

/// [`Asset`] without `id` field.
/// Needed only for the world-state asset map to reduce memory usage.
/// In other places use [`Asset`] directly.
pub type AssetValue = Owned<Numeric>;

impl Asset {
    /// Constructor
    pub fn new(id: AssetId, value: impl Into<Numeric>) -> <Self as Registered>::With {
        Self {
            id,
            value: value.into(),
        }
    }
}

impl Registered for Asset {
    type With = Self;
}

impl IntoKeyValue for Asset {
    type Key = AssetId;
    type Value = AssetValue;
    fn into_key_value(self) -> (Self::Key, Self::Value) {
        (self.id, Owned::new(self.value))
    }
}
