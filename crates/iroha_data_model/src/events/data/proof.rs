//! Proof verification events (ZK) for the data event stream.

use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use super::*;

    /// Proof verification outcome events.
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
    pub enum ProofEvent {
        /// Proof verified successfully.
        Verified(ProofVerified),
        /// Proof failed verification.
        Rejected(ProofRejected),
        /// Proof records were pruned due to retention policy.
        Pruned(ProofPruned),
    }

    /// A proof verification success event payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ProofVerified {
        /// Proof identifier (backend + proof hash).
        pub id: crate::proof::ProofId,
        /// Optional verifying key reference used for verification.
        pub vk_ref: Option<crate::proof::VerifyingKeyId>,
        /// Commitment of the verifying key used for verification.
        pub vk_commitment: Option<[u8; 32]>,
        /// Hash of the transaction entrypoint (call hash) that produced this event, when available.
        pub call_hash: Option<[u8; 32]>,
        /// Hash of the verification envelope payload (e.g., Norito TLV body) when available.
        /// Bound to the transaction `call_hash` via audit fields and used for operator correlation.
        pub envelope_hash: Option<[u8; 32]>,
    }

    /// A proof verification failure event payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ProofRejected {
        /// Proof identifier (backend + proof hash).
        pub id: crate::proof::ProofId,
        /// Optional verifying key reference used for verification.
        pub vk_ref: Option<crate::proof::VerifyingKeyId>,
        /// Commitment of the verifying key used for verification if available.
        pub vk_commitment: Option<[u8; 32]>,
        /// Hash of the transaction entrypoint (call hash) that produced this event, when available.
        pub call_hash: Option<[u8; 32]>,
        /// Hash of the verification envelope payload (e.g., Norito TLV body) when available.
        pub envelope_hash: Option<[u8; 32]>,
    }

    /// A proof retention event payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct ProofPruned {
        /// Backend identifier whose registry entries were pruned.
        pub backend: String,
        /// Proof identifiers removed in this pruning pass (bounded by `prune_batch`).
        pub removed: Vec<crate::proof::ProofId>,
        /// Remaining proof records for this backend after pruning.
        pub remaining: u64,
        /// Configured per-backend cap at the time pruning executed.
        pub cap: u64,
        /// Grace window (in blocks) honored during pruning.
        pub grace_blocks: u64,
        /// Maximum removals per pass (prune batch) enforced.
        pub prune_batch: u64,
        /// Block height at which pruning ran.
        pub pruned_at_height: u64,
        /// Account that issued the pruning instruction (or triggered the insert that pruned).
        pub pruned_by: crate::account::AccountId,
        /// Origin of the pruning run.
        pub origin: ProofPruneOrigin,
    }

    /// Source of a proof pruning pass.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum ProofPruneOrigin {
        /// Retention enforcement triggered while inserting a new proof record.
        Insert,
        /// Explicit `PruneProofs` instruction.
        Manual,
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    ProofEvent,
    ProofVerified,
    ProofRejected,
    ProofPruned,
    ProofPruneOrigin
);

/// Prelude exports for proof events
pub mod prelude {
    pub use super::{ProofEvent, ProofPruneOrigin, ProofPruned, ProofRejected, ProofVerified};
}

// Convenience constructors for common event-set presets
impl ProofEventSet {
    /// A set that matches only `Verified` events.
    pub const fn only_verified() -> Self {
        Self::Verified
    }
    /// A set that matches only `Rejected` events.
    pub const fn only_rejected() -> Self {
        Self::Rejected
    }
}
