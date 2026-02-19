//! Offline settlement lifecycle events for the data event stream.

use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use super::*;

    /// Offline settlement lifecycle events emitted by validators.
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
    pub enum OfflineTransferEvent {
        /// A revocation bundle entry was imported and applied to the WSV (POS-facing).
        RevocationImported(crate::offline::OfflineVerdictRevocation),
        /// An offline bundle settled on-ledger.
        Settled(OfflineTransferSettled),
        /// Remaining allowance from an expired certificate was reclaimed to the controller.
        AllowanceReclaimed(OfflineAllowanceReclaimed),
        /// A settled bundle satisfied the retention policy and moved to the archived tier.
        Archived(OfflineTransferArchived),
        /// An archived bundle exceeded the cold-retention window and was pruned.
        Pruned(OfflineTransferPruned),
    }

    /// Payload emitted when an offline transfer bundle settles on-ledger.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct OfflineTransferSettled {
        /// Unique bundle identifier.
        pub bundle_id: iroha_crypto::Hash,
        /// Controller/sender account bound to the allowance certificate.
        pub controller: crate::account::AccountId,
        /// Receiver declared in the bundle.
        pub receiver: crate::account::AccountId,
        /// Online deposit account that credited the funds.
        pub deposit_account: crate::account::AccountId,
        /// Asset definition settled by the bundle.
        pub asset_definition: crate::AssetDefinitionId,
        /// Aggregate amount credited on-ledger.
        pub amount: Numeric,
        /// Number of receipts aggregated by the bundle.
        pub receipt_count: u32,
        /// Unix timestamp (ms) when the bundle settled.
        pub recorded_at_ms: u64,
        /// Snapshot of the platform token captured during settlement (if applicable).
        #[norito(default)]
        pub platform_snapshot: Option<crate::offline::OfflinePlatformTokenSnapshot>,
    }

    /// Payload emitted when an expired allowance remainder is reclaimed by its controller.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct OfflineAllowanceReclaimed {
        /// Certificate identifier reclaimed by this event.
        pub certificate_id: iroha_crypto::Hash,
        /// Controller account receiving the reclaimed amount.
        pub controller: crate::account::AccountId,
        /// Asset definition moved out of escrow.
        pub asset_definition: crate::AssetDefinitionId,
        /// Amount reclaimed back to the controller.
        pub amount: Numeric,
        /// Unix timestamp (ms) when the reclaim operation succeeded.
        pub reclaimed_at_ms: u64,
    }

    /// Payload emitted when a bundle transitions into the archived retention tier.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct OfflineTransferArchived {
        /// Unique bundle identifier.
        pub bundle_id: iroha_crypto::Hash,
        /// Block height when the bundle initially settled.
        pub recorded_at_height: u64,
        /// Block height when the bundle was archived.
        pub archived_at_height: u64,
        /// Unix timestamp (ms) when the archive event was emitted.
        pub archived_at_ms: u64,
    }

    /// Payload emitted when an archived bundle is pruned from hot storage.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct OfflineTransferPruned {
        /// Unique bundle identifier.
        pub bundle_id: iroha_crypto::Hash,
        /// Block height when the bundle was archived.
        pub archived_at_height: u64,
        /// Block height when the bundle was pruned.
        pub pruned_at_height: u64,
        /// Unix timestamp (ms) when the prune event was emitted.
        pub pruned_at_ms: u64,
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    OfflineTransferEvent,
    OfflineTransferSettled,
    OfflineAllowanceReclaimed,
    OfflineTransferArchived,
    OfflineTransferPruned
);

/// Prelude exports for offline settlement events.
pub mod prelude {
    pub use super::{
        OfflineAllowanceReclaimed, OfflineTransferArchived, OfflineTransferEvent,
        OfflineTransferSettled,
    };
}
