//! This module contains Fees structures, their implementation and related traits and
//! instructions implementations.
#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};

use derive_more::Display;
use iroha_data_model_derive::{model, IdEqOrdHash};
use iroha_schema::IntoSchema;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

pub use self::model::*;
use crate::{account::prelude::*, asset::prelude::*, Identifiable};

#[model]
mod model {
    use getset::{CopyGetters, Getters};

    use super::*;

    /// Builder which can be used in a transaction to create a new [`FeeReceiverDefinition`]
    #[derive(
        Debug,
        Display,
        Clone,
        CopyGetters,
        Getters,
        Decode,
        Encode,
        Deserialize,
        Serialize,
        IntoSchema,
        IdEqOrdHash,
    )]
    #[display(fmt = "{account}${asset}")]
    #[serde(rename = "FeeReceiverDefinition")]
    #[ffi_type]
    pub struct FeeReceiverDefinition {
        #[id]
        /// Accound that receives fees.
        pub account: AccountId,
        /// Asset identification that is used as a fee currency.
        pub asset: AssetDefinitionId,
    }

    impl FeeReceiverDefinition {
        /// Create a new [`FeeReceiverDefinition`]
        pub fn new(account: AccountId, asset: AssetDefinitionId) -> Self {
            Self { account, asset }
        }
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::FeeReceiverDefinition;
}
