//! Compact authenticated-ledger race transition events.
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
/// A native race revision; query the race for its complete bounded record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RaceEventV1 {
    /// Exact race identifier.
    pub race_id: Hash,
    /// Monotonic native state revision.
    pub revision: u64,
    /// Stable phase index in the RacePhaseV1 declaration order.
    pub phase: u8,
    /// Committed checkpoint and forced-input history root.
    pub dispute_root: Hash,
}
