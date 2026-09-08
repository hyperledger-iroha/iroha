//! Generic native NFT custody and one-shot exact-price sale records.
use crate::{NetworkId, account::AccountId, asset::AssetDefinitionId, nft::NftId};
use iroha_crypto::Hash;
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

macro_rules! record {
    ($(#[$meta:meta])* pub struct $name:ident { $($(#[$field_meta:meta])* pub $field:ident: $ty:ty,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(feature = "json", derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize))]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        pub struct $name { $($(#[$field_meta])* pub $field: $ty,)* }
    };
}
/// Closed native custody namespaces, with independent account derivation domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "kind",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum NftCustodyPurposeV1 {
    /// Exact-price sale offer.
    #[codec(index = 0)]
    Sale,
    /// Generic proved game session item stake.
    #[codec(index = 1)]
    GameWager,
    /// Returnable equipment; never awarded to a winner.
    #[codec(index = 2)]
    GameResource,
}
record! {
    /// Permanent custody identity and bounded reservation history for one NFT.
    pub struct NftCustodyRecordV1 {
        /// Version of the retained native format.
        pub version: u16,
        /// Exact network derived from signed genesis.
        pub network_id: NetworkId,
        /// Offer or generic session identifier.
        pub reservation_id: Hash,
        /// Native protocol authorized to release this NFT.
        pub purpose: NftCustodyPurposeV1,
        /// Exact unique NFT identifier.
        pub nft_id: NftId,
        /// Non-signable custody account; never receives a signing scalar.
        pub custody: AccountId,
        /// Wallet that explicitly deposited this NFT.
        pub original_owner: AccountId,
        /// Exact content commitment frozen during custody.
        pub metadata_hash: Hash,
        /// Final recipient; absent only while the NFT remains reserved.
        pub released_to: Option<AccountId>,
    }
}
record! {
    /// Complete immutable terms approved by both the listing seller and purchasing wallet.
    pub struct NftSaleOfferV1 {
        /// Exact network binding.
        pub network_id: NetworkId,
        /// One-shot globally unique offer identity.
        pub offer_id: Hash,
        /// NFT held by the offer's non-signable custody.
        pub nft_id: NftId,
        /// Wallet that receives the exact payment.
        pub seller: AccountId,
        /// Fungible payment denomination; XOR is an application choice.
        pub payment_asset: AssetDefinitionId,
        /// Positive exact payment, excluding transaction fees.
        pub price: Quantity,
        /// Last consensus height at which a purchase is accepted.
        pub expires_at_height: u64,
        /// Optional sole authorized purchaser.
        pub reserved_buyer: Option<AccountId>,
        /// Content commitment preventing metadata substitution between listing and purchase.
        pub metadata_hash: Hash,
    }
}
/// Terminal offer decisions remain in consensus state to reject all replay.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum NftSaleStatusV1 {
    /// NFT reserved for these immutable sale terms.
    Open,
    /// Atomic payment and NFT delivery completed to this account.
    Purchased(AccountId),
    /// Original owner reclaimed the NFT or anyone expired the offer.
    Cancelled,
}
record! {
    /// Exact offer terms and permanent lifecycle decision.
    pub struct NftSaleRecordV1 {
        /// Version of the retained format.
        pub version: u16,
        /// Immutable complete authorization terms.
        pub offer: NftSaleOfferV1,
        /// Commitment to all terms in their native execution domain.
        pub offer_hash: Hash,
        /// Native reservation identity.
        pub custody: AccountId,
        /// Permanent lifecycle state.
        pub status: NftSaleStatusV1,
        /// Height that admitted the listing.
        pub created_at_height: u64,
        /// Height of the terminal decision, absent while open.
        pub closed_at_height: Option<u64>,
    }
}
impl NftSaleOfferV1 {
    /// Domain-separated commitment to the complete canonical native sale terms.
    pub fn commitment(&self) -> Hash {
        Hash::new_from_chunks(&[b"iroha:nft:exact-price-offer:v1\0", &self.encode()])
    }
}
