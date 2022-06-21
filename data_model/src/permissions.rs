//! Structures, traits and impls related to `Permission`s.

#[cfg(not(feature = "std"))]
use alloc::{
    collections::{btree_map, btree_set},
    format,
    string::String,
    vec::Vec,
};
#[cfg(feature = "std")]
use std::collections::{btree_map, btree_set};

use derive_more::Display;
use getset::Getters;
use iroha_schema::IntoSchema;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{Name, Value};

/// Collection of [`PermissionToken`]s
pub type Permissions<const HASH_LENGTH: usize> = btree_set::BTreeSet<PermissionToken<HASH_LENGTH>>;

/// Stored proof of the account having a permission for a certain action.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    Getters,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
)]
#[getset(get = "pub")]
#[cfg_attr(feature = "ffi_api", iroha_ffi::ffi_bindgen)]
#[display(fmt = "{name}")]
pub struct PermissionToken<const HASH_LENGTH: usize> {
    /// Name of the permission rule given to account.
    name: Name,
    /// Params identifying how this rule applies.
    #[getset(skip)]
    params: btree_map::BTreeMap<Name, Value<HASH_LENGTH>>,
}

impl<const HASH_LENGTH: usize> PartialOrd for PermissionToken<HASH_LENGTH> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<const HASH_LENGTH: usize> Ord for PermissionToken<HASH_LENGTH> {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.name().cmp(other.name())
    }
}

#[cfg_attr(feature = "ffi_api", iroha_ffi::ffi_bindgen)]
impl<const HASH_LENGTH: usize> PermissionToken<HASH_LENGTH> {
    /// Constructor.
    #[inline]
    pub fn new(name: Name) -> Self {
        Self {
            name,
            params: btree_map::BTreeMap::default(),
        }
    }

    /// Add parameters to the `PermissionToken` replacing any previously defined
    #[inline]
    #[must_use]
    pub fn with_params(
        mut self,
        params: impl IntoIterator<Item = (Name, Value<HASH_LENGTH>)>,
    ) -> Self {
        self.params = params.into_iter().collect();
        self
    }

    /// Return a reference to the parameter corresponding to the given name
    #[inline]
    pub fn get_param(&self, name: &Name) -> Option<&Value<HASH_LENGTH>> {
        self.params.get(name)
    }

    /// Get an iterator over parameters of the `PermissionToken`
    #[inline]
    pub fn params(&self) -> impl ExactSizeIterator<Item = (&Name, &Value<HASH_LENGTH>)> {
        self.params.iter()
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this module.
pub mod prelude {
    pub use super::{PermissionToken, Permissions};
}
