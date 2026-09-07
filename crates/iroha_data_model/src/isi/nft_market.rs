//! Native NFT listing, exact-price purchase and cancellation instructions.
use super::*;
use crate::{nft::NftId, nft_market::NftSaleOfferV1};
use iroha_crypto::Hash;
use iroha_primitives::numeric::Quantity;

macro_rules! native_isi {
    ($(#[$meta:meta])* pub struct $name:ident { $($(#[$fm:meta])* pub $field:ident: $ty:ty,)* }) => {
        isi! {
            $(#[$meta])*
            #[cfg_attr(feature = "json", derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize))]
            #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
            pub struct $name { $($(#[$fm])* pub $field: $ty,)* }
        }
        impl crate::seal::Instruction for $name {}
        impl $name {
            /// Construct the exact typed native instruction.
            pub fn new($($field: $ty,)*) -> Self { Self { $($field,)* } }
        }
        impl<'a> norito::core::DecodeFromSlice<'a> for $name {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags);
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }
                let mut offset = 0;
                $(let $field = super::decode_aos_canonical_field::<$ty>(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;)*
                if offset != bytes.len() { return Err(norito::core::Error::LengthMismatch); }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field,)* }, offset))
            }
        }
    };
}
native_isi! {
    /// Reserve an authenticated owner's NFT for immutable exact-price terms.
    pub struct OfferNftV1 {
        /// One-shot offer identifier.
        pub offer_id: Hash,
        /// Exact NFT currently owned by the authenticated seller.
        pub nft_id: NftId,
        /// Fungible payment denomination.
        pub payment_asset: AssetDefinitionId,
        /// Positive exact price, separate from fees.
        pub price: Quantity,
        /// Last block height admitting purchase.
        pub expires_at_height: u64,
        /// Optional sole purchaser.
        pub reserved_buyer: Option<AccountId>,
    }
}
native_isi! {
    /// Pay the exact retained price and receive its NFT atomically.
    pub struct BuyNftV1 {
        /// Complete expected terms, including network, seller, denomination, price and content.
        pub offer: NftSaleOfferV1,
    }
}
native_isi! {
    /// Cancel as the seller or expire after the consensus-height deadline.
    pub struct CancelNftOfferV1 {
        /// Exact permanent offer identifier.
        pub offer_id: Hash,
        /// Exact terms being cancelled; prevents stale or foreign intent substitution.
        pub expected_offer_hash: Hash,
    }
}
