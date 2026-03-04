//! Runtime upgrade lifecycle events.

use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use getset::Getters;

    use super::*;

    /// Runtime upgrade lifecycle events (proposal/activation/cancellation).
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        iroha_data_model_derive::EventSet,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum RuntimeUpgradeEvent {
        Proposed(RuntimeUpgradeProposed),
        Activated(RuntimeUpgradeActivated),
        Canceled(RuntimeUpgradeCanceled),
    }

    /// Emitted when a runtime upgrade is proposed.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[getset(get = "pub")]
    pub struct RuntimeUpgradeProposed {
        pub id: crate::runtime::RuntimeUpgradeId,
        pub abi_version: u16,
        pub start_height: u64,
        pub end_height: u64,
    }

    /// Emitted when a runtime upgrade is activated at `at_height`.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[getset(get = "pub")]
    pub struct RuntimeUpgradeActivated {
        pub id: crate::runtime::RuntimeUpgradeId,
        pub abi_version: u16,
        pub at_height: u64,
    }

    /// Emitted when a runtime upgrade is canceled.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[getset(get = "pub")]
    pub struct RuntimeUpgradeCanceled {
        pub id: crate::runtime::RuntimeUpgradeId,
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    RuntimeUpgradeEvent,
    RuntimeUpgradeProposed,
    RuntimeUpgradeActivated,
    RuntimeUpgradeCanceled,
);

/// Prelude exports for runtime upgrade events
pub mod prelude {
    pub use super::{
        RuntimeUpgradeActivated, RuntimeUpgradeCanceled, RuntimeUpgradeEvent,
        RuntimeUpgradeProposed,
    };
}
