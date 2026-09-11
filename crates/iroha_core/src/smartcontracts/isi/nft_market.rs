//! Generic exact-price NFT sales, independent of any game or marketplace operator.
use super::{
    Error, Execute,
    nft_custody::{prepare_nft_release_v1, reserve_nft_v1},
};
use crate::state::{StateReadOnly, StateTransaction, WorldReadOnly};
use iroha_data_model::{
    account::AccountId,
    asset::AssetId,
    isi::{Transfer, nft_market::*},
    nft_market::*,
};
use mv::storage::StorageReadOnly;

/// Release-pinned native marketplace relation and custody policy identifier.
pub fn nft_market_profile_id_v1() -> iroha_crypto::Hash {
    iroha_crypto::Hash::new(b"iroha:nft-market:v1:owner-reservation:exact-terms:atomic-payment:immutable-content:height-expiry:one-shot")
}
/// Activation remains closed until native parity, adversarial tests and rollout review complete.
pub fn nft_market_qualified_v1() -> bool {
    false
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvariantViolation(message.into().into())
}
impl Execute for OfferNftV1 {
    fn execute(
        self,
        authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if st.world.nft_sale_offers.get(&self.offer_id).is_some() {
            return Err(invalid("NFT offer id has already been used"));
        }
        let height = st.block_height();
        if self.price.is_zero()
            || self.expires_at_height <= height
            || self.expires_at_height > height.saturating_add(1_000_000)
            || self.reserved_buyer.as_ref() == Some(authority)
        {
            return Err(invalid(
                "NFT offer requires a positive price, bounded future expiry and distinct buyer",
            ));
        }
        st.world.asset_definition(&self.payment_asset)?;
        let spec = st.numeric_spec_for(&self.payment_asset)?;
        super::asset::isi::assert_numeric_spec_with(self.price.as_numeric(), spec)?;
        if let Some(buyer) = &self.reserved_buyer {
            st.world.account(buyer)?;
        }
        let reservation = reserve_nft_v1(
            st,
            authority,
            self.offer_id,
            NftCustodyPurposeV1::Sale,
            &self.nft_id,
        )?;
        let offer = NftSaleOfferV1 {
            network_id: *st.network_id(),
            offer_id: self.offer_id,
            nft_id: self.nft_id,
            seller: authority.clone(),
            payment_asset: self.payment_asset,
            price: self.price,
            expires_at_height: self.expires_at_height,
            reserved_buyer: self.reserved_buyer,
            metadata_hash: reservation.metadata_hash,
        };
        let record = NftSaleRecordV1 {
            version: 1,
            offer_hash: offer.commitment(),
            offer,
            custody: reservation.custody,
            status: NftSaleStatusV1::Open,
            created_at_height: height,
            closed_at_height: None,
        };
        st.world
            .nft_sale_offers
            .insert(record.offer.offer_id, record);
        Ok(())
    }
}
impl Execute for BuyNftV1 {
    fn execute(
        self,
        authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut record = st
            .world
            .nft_sale_offers
            .get(&self.offer.offer_id)
            .cloned()
            .ok_or_else(|| invalid("NFT offer does not exist"))?;
        let height = st.block_height();
        if record.status != NftSaleStatusV1::Open
            || record.offer != self.offer
            || record.offer_hash != self.offer.commitment()
            || self.offer.network_id != *st.network_id()
            || height > self.offer.expires_at_height
            || authority == &self.offer.seller
            || self
                .offer
                .reserved_buyer
                .as_ref()
                .is_some_and(|buyer| buyer != authority)
        {
            return Err(invalid(
                "NFT purchase differs from the exact open, unexpired seller-approved offer",
            ));
        }
        let release = prepare_nft_release_v1(
            st,
            self.offer.offer_id,
            NftCustodyPurposeV1::Sale,
            &self.offer.nft_id,
            authority,
        )?;
        // Buyer authority authorizes only its own exact payment. Ordinary issuer controls and
        // reserve guards remain in force. The transaction overlay rolls back both legs on error.
        Transfer::asset_quantity(
            AssetId::new(self.offer.payment_asset, authority.clone()),
            self.offer.price,
            self.offer.seller,
        )
        .execute(authority, st)?;
        release.apply(st);
        record.status = NftSaleStatusV1::Purchased(authority.clone());
        record.closed_at_height = Some(height);
        st.world
            .nft_sale_offers
            .insert(record.offer.offer_id, record);
        Ok(())
    }
}
impl Execute for CancelNftOfferV1 {
    fn execute(
        self,
        authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut record = st
            .world
            .nft_sale_offers
            .get(&self.offer_id)
            .cloned()
            .ok_or_else(|| invalid("NFT offer does not exist"))?;
        let height = st.block_height();
        if record.status != NftSaleStatusV1::Open
            || record.offer_hash != self.expected_offer_hash
            || record.offer.network_id != *st.network_id()
            || (authority != &record.offer.seller && height <= record.offer.expires_at_height)
        {
            return Err(invalid(
                "only the seller can cancel an exact open NFT offer before expiry",
            ));
        }
        prepare_nft_release_v1(
            st,
            self.offer_id,
            NftCustodyPurposeV1::Sale,
            &record.offer.nft_id,
            &record.offer.seller,
        )?
        .apply(st);
        record.status = NftSaleStatusV1::Cancelled;
        record.closed_at_height = Some(height);
        st.world.nft_sale_offers.insert(self.offer_id, record);
        Ok(())
    }
}

impl crate::smartcontracts::ValidSingularQuery
    for iroha_data_model::query::nft_market::FindNftSaleOfferById
{
    fn execute(
        &self,
        state: &impl StateReadOnly,
    ) -> Result<NftSaleRecordV1, iroha_data_model::query::error::QueryExecutionFail> {
        state
            .world()
            .nft_sale_offers()
            .get(&self.offer_id)
            .ok_or(iroha_data_model::query::error::QueryExecutionFail::NotFound)
            .and_then(crate::smartcontracts::isi::query::own_singular_query_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{Registrable, prelude::*};
    use iroha_model_base::domain::DomainId;
    use iroha_model_base::metadata::Metadata;
    use iroha_primitives::numeric::Quantity;
    use std::num::NonZeroU64;

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        )
    }
    fn fixture() -> (State, OfferNftV1, AccountId, AccountId, AccountId) {
        let seller = account(81);
        let buyer = account(82);
        let outsider = account(83);
        let domain = DomainId::try_new("nft_market", "universal").unwrap();
        let asset =
            AssetDefinitionId::derive_from_components(domain.clone(), "xor".parse().unwrap());
        let world = World::with_assets(
            [Domain::new(domain).build(&seller)],
            [&seller, &buyer, &outsider].map(|id| Account::new(id.clone()).build(&seller)),
            [AssetDefinition::numeric(
                asset.clone(),
                "NFT purchase test",
                AssetBalancePolicy::Global,
                None,
            )
            .build(&seller)],
            [Asset::new(
                AssetId::of(asset.clone(), buyer.clone()),
                Quantity::from(10_u32),
            )],
            [],
        );
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let offer = OfferNftV1::new(
            Hash::new(b"one-shot NFT sale"),
            "skin$nft_market.universal".parse().unwrap(),
            asset,
            Quantity::from(3_u32),
            100,
            None,
        );
        (state, offer, seller, buyer, outsider)
    }
    fn header(height: u64) -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(height).unwrap(),
            None,
            None,
            None,
            height * 1_000,
            0,
        )
    }
    fn mint(st: &mut StateTransaction<'_, '_>, offer: &OfferNftV1, seller: &AccountId) {
        Register::nft(Nft::new(offer.nft_id.clone(), Metadata::default()))
            .execute(seller, st)
            .unwrap();
    }
    fn sale(st: &StateTransaction<'_, '_>, offer: &OfferNftV1) -> NftSaleRecordV1 {
        st.world
            .nft_sale_offers
            .get(&offer.offer_id)
            .unwrap()
            .clone()
    }
    fn owner(st: &StateTransaction<'_, '_>, nft: &NftId) -> AccountId {
        st.world.nft(nft).unwrap().value().owned_by.clone()
    }
    fn balance(
        st: &StateTransaction<'_, '_>,
        asset: &AssetDefinitionId,
        account: &AccountId,
    ) -> Quantity {
        st.world
            .asset(&AssetId::of(asset.clone(), account.clone()))
            .map(|asset| (**asset.value()).clone())
            .unwrap_or_else(|_| Quantity::zero())
    }

    #[test]
    fn exact_price_purchase_moves_both_legs_and_rejects_replayed_or_changed_terms() {
        let (state, offer, seller, buyer, outsider) = fixture();
        let mut block = state.block(header(1));
        let mut st = block.transaction();
        st.tx_call_hash = Some(Hash::new(b"native-nft-purchase-test-call"));
        mint(&mut st, &offer, &seller);
        offer.clone().execute(&seller, &mut st).unwrap();
        let retained = sale(&st, &offer);
        crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1(
            &retained,
            "iroha_data_model::nft_market::NftSaleRecordV1",
        );
        assert_eq!(owner(&st, &offer.nft_id), retained.custody);
        for altered in [
            NftSaleOfferV1 {
                price: Quantity::from(1_u32),
                ..retained.offer.clone()
            },
            NftSaleOfferV1 {
                seller: outsider.clone(),
                ..retained.offer.clone()
            },
            NftSaleOfferV1 {
                metadata_hash: Hash::new(b"forged skin"),
                ..retained.offer.clone()
            },
            NftSaleOfferV1 {
                expires_at_height: 101,
                ..retained.offer.clone()
            },
        ] {
            assert!(BuyNftV1::new(altered).execute(&buyer, &mut st).is_err());
        }
        assert_eq!(
            balance(&st, &offer.payment_asset, &buyer),
            Quantity::from(10_u32)
        );
        BuyNftV1::new(retained.offer.clone())
            .execute(&buyer, &mut st)
            .unwrap();
        assert_eq!(owner(&st, &offer.nft_id), buyer);
        assert_eq!(
            balance(&st, &offer.payment_asset, &buyer),
            Quantity::from(7_u32)
        );
        assert_eq!(
            balance(&st, &offer.payment_asset, &seller),
            Quantity::from(3_u32)
        );
        assert_eq!(
            sale(&st, &offer).status,
            NftSaleStatusV1::Purchased(buyer.clone())
        );
        assert!(
            BuyNftV1::new(retained.offer)
                .execute(&buyer, &mut st)
                .is_err()
        );
        assert!(offer.clone().execute(&seller, &mut st).is_err());
        assert!(
            CancelNftOfferV1::new(offer.offer_id, retained.offer_hash)
                .execute(&seller, &mut st)
                .is_err()
        );
        assert_eq!(
            balance(&st, &offer.payment_asset, &seller),
            Quantity::from(3_u32)
        );
    }

    #[test]
    fn insufficient_payment_and_wrong_reserved_buyer_preserve_the_reserved_nft() {
        let (state, mut offer, seller, buyer, outsider) = fixture();
        offer.reserved_buyer = Some(buyer.clone());
        offer.price = Quantity::from(11_u32);
        let mut block = state.block(header(1));
        let mut st = block.transaction();
        mint(&mut st, &offer, &seller);
        offer.clone().execute(&seller, &mut st).unwrap();
        let retained = sale(&st, &offer);
        assert!(
            BuyNftV1::new(retained.offer.clone())
                .execute(&outsider, &mut st)
                .is_err()
        );
        assert!(
            BuyNftV1::new(retained.offer)
                .execute(&buyer, &mut st)
                .is_err()
        );
        assert_eq!(owner(&st, &offer.nft_id), retained.custody);
        assert_eq!(sale(&st, &offer).status, NftSaleStatusV1::Open);
        assert_eq!(
            balance(&st, &offer.payment_asset, &buyer),
            Quantity::from(10_u32)
        );
        assert!(balance(&st, &offer.payment_asset, &seller).is_zero());
    }

    #[test]
    fn native_custody_blocks_domain_owner_transfers_metadata_deletion_and_cross_protocol_release() {
        let (state, offer, seller, buyer, _) = fixture();
        let mut block = state.block(header(1));
        let mut st = block.transaction();
        mint(&mut st, &offer, &seller);
        offer.clone().execute(&seller, &mut st).unwrap();
        let retained = sale(&st, &offer);
        assert!(
            Transfer::nft(
                retained.custody.clone(),
                offer.nft_id.clone(),
                buyer.clone()
            )
            .execute(&seller, &mut st)
            .is_err()
        );
        assert!(
            SetKeyValue::nft(offer.nft_id.clone(), "skin".parse().unwrap(), "forged")
                .execute(&seller, &mut st)
                .is_err()
        );
        assert!(
            RemoveKeyValue::nft(offer.nft_id.clone(), "skin".parse().unwrap())
                .execute(&seller, &mut st)
                .is_err()
        );
        assert!(
            Unregister::nft(offer.nft_id.clone())
                .execute(&seller, &mut st)
                .is_err()
        );
        assert!(
            Unregister::domain(offer.nft_id.domain().clone())
                .execute(&seller, &mut st)
                .is_err()
        );
        assert!(
            Unregister::account(seller.clone())
                .execute(&seller, &mut st)
                .is_err()
        );
        assert!(
            Unregister::account(retained.custody.clone())
                .execute(&seller, &mut st)
                .is_err()
        );
        assert!(
            prepare_nft_release_v1(
                &st,
                offer.offer_id,
                NftCustodyPurposeV1::GameWager,
                &offer.nft_id,
                &buyer
            )
            .is_err()
        );
        assert_eq!(owner(&st, &offer.nft_id), retained.custody);
        assert_eq!(sale(&st, &offer), retained);
    }

    #[test]
    fn only_owner_can_list_and_only_seller_can_cancel_before_height_expiry() {
        let (state, offer, seller, buyer, outsider) = fixture();
        let mut block = state.block(header(1));
        let mut st = block.transaction();
        mint(&mut st, &offer, &seller);
        assert!(offer.clone().execute(&outsider, &mut st).is_err());
        let mut zero = offer.clone();
        zero.price = Quantity::zero();
        assert!(zero.execute(&seller, &mut st).is_err());
        let mut expired = offer.clone();
        expired.expires_at_height = 1;
        assert!(expired.execute(&seller, &mut st).is_err());
        assert_eq!(owner(&st, &offer.nft_id), seller);
        offer.clone().execute(&seller, &mut st).unwrap();
        let retained = sale(&st, &offer);
        assert!(
            CancelNftOfferV1::new(offer.offer_id, retained.offer_hash)
                .execute(&buyer, &mut st)
                .is_err()
        );
        CancelNftOfferV1::new(offer.offer_id, retained.offer_hash)
            .execute(&seller, &mut st)
            .unwrap();
        assert_eq!(owner(&st, &offer.nft_id), seller);
        assert_eq!(sale(&st, &offer).status, NftSaleStatusV1::Cancelled);
        assert!(offer.clone().execute(&seller, &mut st).is_err());
        assert!(!super::super::nft_custody::retained_nft_account(
            &st.world, &seller
        ));
        assert!(super::super::nft_custody::retained_nft_account(
            &st.world,
            &retained.custody
        ));
    }

    #[test]
    fn anyone_can_expire_at_a_later_consensus_height_and_snapshot_guards_rebuild() {
        let (mut state, offer, seller, buyer, outsider) = fixture();
        {
            let mut block = state.block(header(1));
            let mut st = block.transaction();
            mint(&mut st, &offer, &seller);
            offer.clone().execute(&seller, &mut st).unwrap();
            st.apply();
            block.commit_world_overlay_for_testing().unwrap();
        }
        state.world.rebuild_nft_custody_indexes().unwrap();
        let retained = state
            .world
            .nft_sale_offers
            .view()
            .get(&offer.offer_id)
            .unwrap()
            .clone();
        let mut block = state.block(header(101));
        let mut st = block.transaction();
        assert!(
            BuyNftV1::new(retained.offer.clone())
                .execute(&buyer, &mut st)
                .is_err()
        );
        CancelNftOfferV1::new(offer.offer_id, retained.offer_hash)
            .execute(&outsider, &mut st)
            .unwrap();
        assert_eq!(owner(&st, &offer.nft_id), seller);
        st.apply();
        block.commit_world_overlay_for_testing().unwrap();
        state.world.rebuild_nft_custody_indexes().unwrap();
        assert_eq!(
            state
                .world
                .nft_sale_offers
                .view()
                .get(&offer.offer_id)
                .unwrap()
                .closed_at_height,
            Some(101)
        );
    }

    #[test]
    fn snapshot_rejects_an_extra_sale_reservation_reusing_another_offers_id() {
        let (mut state, offer, seller, _, _) = fixture();
        {
            let mut block = state.block(header(1));
            let mut st = block.transaction();
            mint(&mut st, &offer, &seller);
            offer.clone().execute(&seller, &mut st).unwrap();
            let mut orphan = offer.clone();
            orphan.nft_id = "orphan$nft_market.universal".parse().unwrap();
            mint(&mut st, &orphan, &seller);
            // Simulate a corrupted snapshot: the public Offer ISI rejects this duplicate ID.
            reserve_nft_v1(
                &mut st,
                &seller,
                offer.offer_id,
                NftCustodyPurposeV1::Sale,
                &orphan.nft_id,
            )
            .unwrap();
            st.apply();
            block.commit_world_overlay_for_testing().unwrap();
        }
        assert!(
            state
                .world
                .rebuild_nft_custody_indexes()
                .unwrap_err()
                .contains("exact permanent offer")
        );
    }

    #[test]
    fn shared_batch_release_preflights_all_items_and_decrements_shared_references_exactly() {
        let (state, first, seller, buyer, _) = fixture();
        let mut second = first.clone();
        second.offer_id = Hash::new(b"second NFT reservation");
        second.nft_id = "second$nft_market.universal".parse().unwrap();
        let mut block = state.block(header(1));
        let mut st = block.transaction();
        for offer in [&first, &second] {
            mint(&mut st, offer, &seller);
            offer.clone().execute(&seller, &mut st).unwrap();
        }
        let release = |offer: &OfferNftV1| {
            (
                offer.offer_id,
                NftCustodyPurposeV1::Sale,
                offer.nft_id.clone(),
                buyer.clone(),
            )
        };
        assert!(
            super::super::nft_custody::prepare_nft_releases_v1(
                &st,
                vec![release(&first), release(&first)]
            )
            .is_err()
        );
        assert_eq!(st.world.nft_custody_owner_refs.get(&seller), Some(&2));
        let prepared = super::super::nft_custody::prepare_nft_releases_v1(
            &st,
            vec![release(&first), release(&second)],
        )
        .unwrap();
        assert_eq!(st.world.nft_custody_owner_refs.get(&seller), Some(&2));
        prepared.apply(&mut st);
        assert_eq!(st.world.nft_custody_owner_refs.get(&seller), None);
        assert_eq!(
            st.world.nft_custody_domain_refs.get(first.nft_id.domain()),
            None
        );
        assert_eq!(owner(&st, &first.nft_id), buyer);
        assert_eq!(owner(&st, &second.nft_id), buyer);
    }
}
