//! Structures, traits and impls related to `Role`s.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};

use iroha_data_model_derive::model;

pub use self::model::*;
use crate::{
    account::AccountId,
    permission::{Permission, Permissions},
    Identifiable, Name, Registered, Registrable,
};

#[model]
mod model {
    use derive_more::{Constructor, Display, FromStr};
    use getset::Getters;
    use iroha_data_model_derive::IdEqOrdHash;
    use iroha_schema::IntoSchema;
    use parity_scale_codec::{Decode, Encode};
    use serde::{Deserialize, Serialize};

    use super::*;

    /// Identification of a role.
    #[derive(
        Debug,
        Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Constructor,
        FromStr,
        Getters,
        Decode,
        Encode,
        Deserialize,
        Serialize,
        IntoSchema,
    )]
    #[getset(get = "pub")]
    #[serde(transparent)]
    #[repr(transparent)]
    #[ffi_type(opaque)]
    pub struct RoleId {
        /// Role name, should be unique .
        pub name: Name,
    }

    /// Role is a tag for a set of permission tokens.
    #[derive(
        Debug, Display, Clone, IdEqOrdHash, Decode, Encode, Deserialize, Serialize, IntoSchema,
    )]
    #[display(fmt = "{id}")]
    #[ffi_type]
    pub struct Role {
        /// Unique name of the role.
        pub id: RoleId,
        /// Permission tokens.
        pub permissions: Permissions,
    }

    /// Builder for [`Role`]
    #[derive(
        Debug,
        Display,
        Clone,
        Getters,
        IdEqOrdHash,
        Decode,
        Encode,
        Deserialize,
        Serialize,
        IntoSchema,
    )]
    #[ffi_type]
    #[getset(get = "pub")]
    #[display(fmt = "{grant_to}: {inner}")]
    pub struct NewRole {
        #[allow(missing_docs)]
        #[id(transparent)]
        #[serde(flatten)]
        pub inner: Role,
        /// First owner
        pub grant_to: AccountId,
    }
}

impl Role {
    /// Constructor.
    #[inline]
    pub fn new(id: RoleId, grant_to: AccountId) -> <Self as Registered>::With {
        NewRole::new(id, grant_to)
    }

    /// Get an iterator over [`permissions`](Permission) of the `Role`
    #[inline]
    pub fn permissions(&self) -> impl ExactSizeIterator<Item = &Permission> {
        self.permissions.iter()
    }
}

impl NewRole {
    /// Constructor
    #[must_use]
    #[inline]
    fn new(id: RoleId, grant_to: AccountId) -> Self {
        Self {
            grant_to,
            inner: Role {
                id,
                permissions: Permissions::new(),
            },
        }
    }

    /// Add permission to the [`Role`]
    #[must_use]
    #[inline]
    pub fn add_permission(mut self, perm: impl Into<Permission>) -> Self {
        self.inner.permissions.insert(perm.into());
        self
    }
}

impl Registered for Role {
    type With = NewRole;
}

impl Registrable for NewRole {
    type Target = Role;

    #[inline]
    fn build(self, _authority: &AccountId) -> Self::Target {
        self.inner
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this module.
pub mod prelude {
    pub use super::{NewRole, Role, RoleId};
}
