//! Lifecycle events of governed runtime change sets and emergency containment
//! (`specs/live_runtime_upgrades.md` §6 and §7.1).
pub use self::model::*;
use super::*;
use iroha_data_model_derive::model;
#[model]
mod model {
    use super::*;
    use getset::Getters;
    /// Lifecycle of a change set: scheduled by governance, attested by validators, then
    /// activated, lapsed or canceled; and of emergency containment: contained, then expired.
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
    #[event_set(
        schema_name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeEventSet"
    )]
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeEvent"
    )]
    pub enum RuntimeUpgradeEvent {
        Scheduled(RuntimeUpgradeScheduled),
        Attested(RuntimeUpgradeAttested),
        Activated(RuntimeUpgradeActivated),
        Lapsed(RuntimeUpgradeLapsed),
        Canceled(RuntimeUpgradeCanceled),
        Contained(RuntimeUpgradeContained),
        ContainmentExpired(RuntimeUpgradeContainmentExpired),
        ContainmentReviewed(RuntimeUpgradeContainmentReviewed),
        ContainmentResolved(RuntimeUpgradeContainmentResolved),
    }
    /// Emitted when governance approves a change set for its effective height.
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
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeScheduled"
    )]
    pub struct RuntimeUpgradeScheduled {
        pub id: crate::runtime::lifecycle::ChangeSetId,
        pub effective_height: u64,
    }
    /// Emitted when a validator's support attestation is recorded.
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
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeAttested"
    )]
    pub struct RuntimeUpgradeAttested {
        pub id: crate::runtime::lifecycle::ChangeSetId,
        pub validator: iroha_crypto::PublicKey,
    }
    /// Emitted when a change set takes effect at the start of `height`.
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
        pub id: crate::runtime::lifecycle::ChangeSetId,
        pub height: u64,
        pub successor: crate::runtime::definition::SelectionId,
    }
    /// Emitted when a change set reaches its effective height without complete support,
    /// elapsed notice or a selection it still applies to; it never applies.
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
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeLapsed"
    )]
    pub struct RuntimeUpgradeLapsed {
        pub id: crate::runtime::lifecycle::ChangeSetId,
        pub height: u64,
    }
    /// Emitted when governance withdraws a scheduled change set.
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
        pub id: crate::runtime::lifecycle::ChangeSetId,
        pub height: u64,
    }
    /// Emitted when the runtime emergency panel's containment is authorized; its restrictions
    /// apply from `effective_height`.
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
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeContained"
    )]
    pub struct RuntimeUpgradeContained {
        pub containment: [u8; 32],
        pub incident: [u8; 32],
        pub effective_height: u64,
    }
    /// Emitted when a containment's lease ends at `height` without a renewal.
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
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeContainmentExpired"
    )]
    pub struct RuntimeUpgradeContainmentExpired {
        pub containment: [u8; 32],
        pub height: u64,
    }
    /// Emitted when governance proposes or approves a change set that addresses a definition an
    /// active containment holds: progress of its ratification case.
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
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeContainmentReviewed"
    )]
    pub struct RuntimeUpgradeContainmentReviewed {
        pub containment: [u8; 32],
        pub change_set: crate::runtime::lifecycle::ChangeSetId,
    }
    /// Emitted when change sets that activate at `height` take over every restriction a
    /// containment held, closing its ratification case.
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
        name = "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeContainmentResolved"
    )]
    pub struct RuntimeUpgradeContainmentResolved {
        pub containment: [u8; 32],
        pub height: u64,
    }
}

impl RuntimeUpgradeEvent {
    /// The change set the event belongs to; containment events belong to none.
    #[must_use]
    pub const fn change_set(&self) -> Option<&crate::runtime::lifecycle::ChangeSetId> {
        match self {
            Self::Scheduled(event) => Some(&event.id),
            Self::Attested(event) => Some(&event.id),
            Self::Activated(event) => Some(&event.id),
            Self::Lapsed(event) => Some(&event.id),
            Self::Canceled(event) => Some(&event.id),
            Self::ContainmentReviewed(event) => Some(&event.change_set),
            Self::Contained(_) | Self::ContainmentExpired(_) | Self::ContainmentResolved(_) => None,
        }
    }
}

impl_json_via_norito_bytes!(
    RuntimeUpgradeEvent,
    RuntimeUpgradeScheduled,
    RuntimeUpgradeAttested,
    RuntimeUpgradeActivated,
    RuntimeUpgradeLapsed,
    RuntimeUpgradeCanceled,
    RuntimeUpgradeContained,
    RuntimeUpgradeContainmentExpired,
    RuntimeUpgradeContainmentReviewed,
    RuntimeUpgradeContainmentResolved,
);
/// Prelude exports for runtime upgrade events
pub mod prelude {
    pub use super::{
        RuntimeUpgradeActivated, RuntimeUpgradeAttested, RuntimeUpgradeCanceled,
        RuntimeUpgradeContained, RuntimeUpgradeContainmentExpired,
        RuntimeUpgradeContainmentResolved, RuntimeUpgradeContainmentReviewed, RuntimeUpgradeEvent,
        RuntimeUpgradeLapsed, RuntimeUpgradeScheduled,
    };
}

#[cfg(test)]
mod captured_event_boundary_identity_tests;
