//! Events emitted by native asset escrow flows.

use iroha_crypto::Hash;
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{account::AccountId, escrow::AssetEscrowRecord};

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
    /// Oracle condition was attested; the record may also be automatically released.
    Attested(AssetEscrowAttested),
    /// Escrow cancelled and refunded to the seller.
    Cancelled(AssetEscrowRecord),
    /// Generic lock expired and refunded remaining custody.
    Expired(AssetEscrowRecord),
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

/// Native conditional-lock attestation event.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AssetEscrowAttested {
    /// Escrow record after applying the attestation.
    pub escrow: AssetEscrowRecord,
    /// Exact condition identifier that was satisfied.
    pub condition_id: String,
    /// Account that supplied the attestation.
    pub attestor: AccountId,
    /// Whether this attestation also released the escrow.
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
        AssetEscrowAttested, AssetEscrowDisputed, AssetEscrowResolved, EscrowEvent, EscrowEventSet,
    };
    pub use crate::escrow::{AssetEscrowRecord, EscrowId};
}
