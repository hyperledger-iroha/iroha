//! Runtime upgrade lifecycle events.
pub use self::model::*;
use super::*;
use iroha_data_model_derive::model;
#[model]
mod model {
    use super::*;
    use getset::Getters;
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
    #[event_set(
        schema_name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeEventSet"
    )]
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeEvent"
    )]
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
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeProposed"
    )]
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
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeActivated"
    )]
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
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeCanceled"
    )]
    pub struct RuntimeUpgradeCanceled {
        pub id: crate::runtime::RuntimeUpgradeId,
    }
}

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

#[cfg(test)]
mod captured_event_boundary_identity_tests;
