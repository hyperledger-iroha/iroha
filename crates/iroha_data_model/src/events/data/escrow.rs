//! Events emitted by native asset escrow flows.
use crate::{account::AccountId, escrow::AssetEscrowRecord, name::Name};
use iroha_crypto::Hash;
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
/// Native escrow lifecycle events.
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
    IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "event", content = "payload"))]
pub enum EscrowEvent {
    /// Escrow opened and funded.
    Opened(AssetEscrowRecord),
    /// Escrow accepted by a buyer.
    Accepted(AssetEscrowRecord),
    /// Buyer marked payment as sent.
    PaymentSent(AssetEscrowRecord),
    /// Seller released funds to the buyer.
    Released(AssetEscrowRecord),
    /// Escrow cancelled and refunded to the seller.
    Cancelled(AssetEscrowRecord),
    /// Generic lock expired and refunded remaining custody.
    Expired(AssetEscrowRecord),
    /// One ordered conditional-escrow predicate was attested on-chain.
    Attested(ConditionalEscrowAttested),
    /// Dispute opened for court moderation.
    Disputed(AssetEscrowDisputed),
    /// Court resolved a disputed escrow.
    Resolved(AssetEscrowResolved),
}
impl EscrowEvent {
    /// Return the escrow record carried by this event.
    #[must_use]
    pub const fn escrow(&self) -> &AssetEscrowRecord {
        match self {
            Self::Opened(escrow)
            | Self::Accepted(escrow)
            | Self::PaymentSent(escrow)
            | Self::Released(escrow)
            | Self::Cancelled(escrow)
            | Self::Expired(escrow) => escrow,
            Self::Disputed(payload) => &payload.escrow,
            Self::Attested(payload) => &payload.escrow,
            Self::Resolved(payload) => &payload.escrow,
        }
    }
}
/// Conditional-escrow attestation event.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ConditionalEscrowAttested {
    /// Full query-visible record after applying the attestation.
    pub escrow: AssetEscrowRecord,
    /// Exact immutable condition identifier.
    pub condition_id: Name,
    /// Account that authorized the attestation.
    pub attestor: AccountId,
    /// Whether this attestation atomically released all remaining custody.
    pub automatically_released: bool,
}
/// Dispute opening event.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AssetEscrowDisputed {
    /// Escrow record after entering disputed state.
    pub escrow: AssetEscrowRecord,
    /// Account that opened the dispute.
    pub opened_by: AccountId,
    /// Evidence hashes attached by the disputing party.
    pub evidence_hashes: Vec<Hash>,
}
/// Court resolution event.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AssetEscrowResolved {
    /// Escrow record after resolution.
    pub escrow: AssetEscrowRecord,
    /// Court account that resolved the dispute.
    pub resolver: AccountId,
    /// Amount released to the buyer.
    pub buyer_amount: Quantity,
    /// Amount refunded to the seller.
    pub seller_amount: Quantity,
}
/// Prelude exports for native escrow events.
pub mod prelude {
    pub use super::{
        AssetEscrowDisputed, AssetEscrowResolved, ConditionalEscrowAttested, EscrowEvent,
        EscrowEventSet,
    };
    pub use crate::escrow::{AssetEscrowRecord, EscrowId};
}
