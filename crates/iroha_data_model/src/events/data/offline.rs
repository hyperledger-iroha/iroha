//! Offline note lifecycle events for the data event stream.

use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use super::*;

    /// Offline note lifecycle events emitted by validators.
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
    pub enum OfflineNoteEvent {
        /// An Offline note was issued and escrow was reserved.
        NoteIssued(OfflineNoteIssued),
        /// An Offline note was redeemed and escrow was credited to the recipient.
        NoteRedeemed(OfflineNoteRedeemed),
        /// An optional Offline audit token was recorded.
        AuditRecorded(OfflineNoteAuditRecorded),
    }

    /// Payload emitted when an Offline note is issued.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct OfflineNoteIssued {
        /// Deterministic note commitment.
        pub note_commitment: iroha_crypto::Hash,
        /// Account bound by the issued one-use key certificate.
        pub account: crate::account::AccountId,
        /// Asset reserved into offline escrow.
        pub asset: crate::AssetId,
        /// Amount reserved into offline escrow.
        pub amount: Numeric,
        /// Unix timestamp (ms) when issuance succeeded.
        pub recorded_at_ms: u64,
    }

    /// Payload emitted when an Offline note is redeemed.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct OfflineNoteRedeemed {
        /// Issued note commitment consumed by this redemption.
        pub source_note_commitment: iroha_crypto::Hash,
        /// Recipient account credited online.
        pub recipient: crate::account::AccountId,
        /// Asset credited from offline escrow.
        pub asset: crate::AssetId,
        /// Amount credited from offline escrow.
        pub amount: Numeric,
        /// Unix timestamp (ms) when redemption succeeded.
        pub recorded_at_ms: u64,
    }

    /// Payload emitted when an optional Offline audit token is recorded.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct OfflineNoteAuditRecorded {
        /// Payment token identifier.
        pub token_id: iroha_crypto::Hash,
        /// Account bound by the sender one-use key certificate.
        pub account: crate::account::AccountId,
        /// Public-input hash verified for the audit proof.
        pub public_inputs_hash: iroha_crypto::Hash,
        /// Unix timestamp (ms) when the audit record succeeded.
        pub recorded_at_ms: u64,
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    OfflineNoteEvent,
    OfflineNoteIssued,
    OfflineNoteRedeemed,
    OfflineNoteAuditRecorded
);

/// Prelude exports for Offline note events.
pub mod prelude {
    pub use super::{
        OfflineNoteAuditRecorded, OfflineNoteEvent, OfflineNoteIssued, OfflineNoteRedeemed,
    };
}
