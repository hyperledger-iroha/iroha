//! Compact authenticated-ledger game session transition events.
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
/// A native game session revision; query the session for its complete bounded record.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::events::data::game::GameSessionEventV1")]
pub struct GameSessionEventV1 {
    /// Exact session identifier.
    pub session_id: Hash,
    /// Monotonic native state revision.
    pub revision: u64,
    /// Stable phase index in the GamePhaseV1 declaration order.
    pub phase: u8,
    /// Committed checkpoint and forced-input history root.
    pub dispute_root: Hash,
    /// Exact immutable awards and unpaid amounts, ordered by permanent roster slot.
    pub payout_claims: Vec<crate::game::GamePayoutClaimV1>,
    /// Explicit NFT stakes and their completed native recipients.
    pub item_stakes: Vec<crate::game::GameItemStakeV1>,
    /// Exact retained returnable equipment state.
    pub resources: Vec<crate::game::GameResourceReservationRecordV1>,
    /// Legal terminal transition height, absent before settlement/cancellation.
    pub terminal_at_height: Option<u64>,
}
