//! Permission Token and related impls
use std::{collections::BTreeSet, format, string::String, vec::Vec};

use getset::Getters;
use iroha_data_model_derive::model;
use iroha_primitives::json::Json;
use iroha_schema::{Ident, IntoSchema};

pub use self::model::*;

/// Collection of [`Permission`]s
pub type Permissions = BTreeSet<Permission>;

#[model]
mod model {
    use derive_more::Display;
    use norito::codec::{Decode, Encode};

    use super::*;

    /// Stored proof of the account having a permission for a certain action.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Display, Getters,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[display("{name}({payload})")]
    pub struct Permission {
        /// Refers to a type defined in [`crate::executor::ExecutorDataModel`].
        #[getset(skip)]
        pub name: Ident,
        /// Payload containing actual value.
        ///
        /// It is JSON-encoded, and its structure must correspond to the structure of
        /// the type defined in [`crate::executor::ExecutorDataModel`].
        #[getset(get = "pub")]
        pub payload: Json,
    }
}

impl Permission {
    /// Constructor
    pub fn new(name: Ident, payload: impl Into<Json>) -> Self {
        Self {
            name,
            payload: payload.into(),
        }
    }

    /// Refers to a type defined in [`crate::executor::ExecutorDataModel`].
    pub fn name(&self) -> &str {
        &self.name
    }
}

pub mod prelude {
    //! The prelude re-exports most commonly used traits, structs and macros from this crate.
    pub use super::Permission;
}
