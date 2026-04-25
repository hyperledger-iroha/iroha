//! Generic asset escrow records and identifiers.

use iroha_crypto::Hash;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{account::AccountId, asset::AssetDefinitionId, name::Name};

/// Domain-separation prefix for escrow ids derived from Kotodama escrow names.
pub const KOTODAMA_ESCROW_ID_PREFIX: &str = "kotodama-native-escrow:";

/// Stable identifier for a native asset escrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct EscrowId(pub Hash);

impl EscrowId {
    /// Construct an escrow identifier from a hash.
    #[must_use]
    pub const fn new(hash: Hash) -> Self {
        Self(hash)
    }

    /// Return the inner hash.
    #[must_use]
    pub const fn as_hash(&self) -> &Hash {
        &self.0
    }

    /// Derive the native escrow id used by Kotodama escrow builtins.
    #[must_use]
    pub fn from_kotodama_name(name: &Name) -> Self {
        Self(Hash::new(format!("{KOTODAMA_ESCROW_ID_PREFIX}{name}")))
    }
}

/// Lifecycle state for a native asset escrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[repr(u8)]
pub enum AssetEscrowStatus {
    /// Seller has locked funds, but no buyer has accepted the offer.
    Open,
    /// Buyer accepted the offer.
    Accepted,
    /// Buyer marked the off-chain payment as sent.
    PaymentSent,
    /// Buyer or seller opened a dispute for court moderation.
    Disputed,
    /// Seller released the escrow to the buyer.
    Released,
    /// Seller cancelled and refunded the escrow before payment was marked.
    Cancelled,
    /// Court resolved the disputed escrow.
    Resolved,
}

/// Court resolution details for a disputed escrow.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AssetEscrowResolution {
    /// Account that resolved the dispute.
    pub resolver: AccountId,
    /// Amount released to the buyer.
    pub buyer_amount: Numeric,
    /// Amount refunded to the seller.
    pub seller_amount: Numeric,
    /// Evidence or judgement hashes attached to the resolution.
    pub evidence_hashes: Vec<Hash>,
    /// Unix timestamp (milliseconds) when the resolution was recorded.
    pub resolved_at_ms: u64,
}

/// Ledger-managed numeric asset escrow.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AssetEscrowRecord {
    /// Escrow identifier.
    pub id: EscrowId,
    /// Seller that funded the escrow.
    pub seller: AccountId,
    /// Buyer that accepted the offer, if any.
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub buyer: Option<AccountId>,
    /// Escrowed asset definition.
    pub asset_definition: AssetDefinitionId,
    /// Total amount held by the escrow.
    pub amount: Numeric,
    /// Deterministic protocol custody account holding the locked balance.
    pub custody: AccountId,
    /// Current lifecycle status.
    pub status: AssetEscrowStatus,
    /// Evidence hashes attached by the parties.
    pub evidence_hashes: Vec<Hash>,
    /// Unix timestamp (milliseconds) when the escrow was opened.
    pub created_at_ms: u64,
    /// Unix timestamp (milliseconds) when a buyer accepted.
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub accepted_at_ms: Option<u64>,
    /// Unix timestamp (milliseconds) when the buyer marked payment as sent.
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub payment_sent_at_ms: Option<u64>,
    /// Unix timestamp (milliseconds) when a dispute was opened.
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub disputed_at_ms: Option<u64>,
    /// Unix timestamp (milliseconds) when the escrow closed.
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub closed_at_ms: Option<u64>,
    /// Optional court resolution details for resolved disputed escrows.
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub resolution: Option<AssetEscrowResolution>,
}

/// Prelude exports for native escrow records.
pub mod prelude {
    pub use super::{
        AssetEscrowRecord, AssetEscrowResolution, AssetEscrowStatus, EscrowId,
        KOTODAMA_ESCROW_ID_PREFIX,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    #[test]
    fn asset_escrow_record_roundtrips_norito() {
        let seller_keypair = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
        let buyer_keypair = KeyPair::from_seed(vec![0x52; 32], Algorithm::Ed25519);
        let seller = AccountId::new(seller_keypair.public_key().clone());
        let buyer = AccountId::new(buyer_keypair.public_key().clone());
        let asset_definition: AssetDefinitionId =
            "61CtjvNd9T3THAR65GsMVHr82Bjc".parse().expect("asset id");
        let id = EscrowId::new(Hash::new("escrow-roundtrip"));
        let record = AssetEscrowRecord {
            id,
            seller: seller.clone(),
            buyer: Some(buyer.clone()),
            asset_definition,
            amount: Numeric::new(42_u32, 0),
            custody: seller,
            status: AssetEscrowStatus::PaymentSent,
            evidence_hashes: vec![Hash::new("evidence")],
            created_at_ms: 1,
            accepted_at_ms: Some(2),
            payment_sent_at_ms: Some(3),
            disputed_at_ms: None,
            closed_at_ms: None,
            resolution: None,
        };

        let bytes = norito::to_bytes(&record).expect("encode");
        let decoded: AssetEscrowRecord = norito::decode_from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, record);
        assert_eq!(decoded.buyer, Some(buyer));
    }

    #[test]
    fn kotodama_escrow_id_derivation_is_stable() {
        let name: Name = "aitai_offer".parse().expect("valid name");
        assert_eq!(
            EscrowId::from_kotodama_name(&name),
            EscrowId::new(Hash::new("kotodama-native-escrow:aitai_offer"))
        );
    }
}
