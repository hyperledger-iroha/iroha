#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Native asset escrow integration scenarios.

use eyre::Result;
use integration_tests::sandbox;
use iroha::data_model::{
    isi::escrow::{AcceptAssetEscrow, MarkEscrowPaymentSent, OpenAssetEscrow, ReleaseAssetEscrow},
    prelude::*,
};
use iroha_crypto::Hash;
use iroha_executor_data_model::permission::asset::CanTransferAsset;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, gen_account_in};

#[test]
fn native_asset_escrow_aitai_flow_on_multi_peer_network() -> Result<()> {
    let context = stringify!(native_asset_escrow_aitai_flow_on_multi_peer_network);
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(NetworkBuilder::new().with_min_peers(4), context)
            .unwrap()
    else {
        return Ok(());
    };
    let client = network.client();

    let result: Result<()> = (|| {
        let seller = ALICE_ID.clone();
        let (buyer, buyer_keypair) = gen_account_in("wonderland");
        let buyer_client = network
            .peer()
            .client_for(&buyer, buyer_keypair.private_key().clone());
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal")?,
            "aitai_xor_native".parse()?,
        );
        let seller_asset_id = AssetId::of(asset_definition_id.clone(), seller.clone());
        client.submit_all_blocking([
            InstructionBox::from(Register::account(Account::new(buyer.clone()))),
            Register::asset_definition(
                AssetDefinition::numeric(asset_definition_id.clone())
                    .with_name(asset_definition_id.name().to_string()),
            )
            .into(),
            Mint::asset_numeric(Numeric::from(100_u64), seller_asset_id.clone()).into(),
        ])?;

        let escrow_id = EscrowId::new(Hash::new("native-aitai-flow"));
        client.submit_blocking(OpenAssetEscrow::with_evidence_hashes(
            escrow_id,
            asset_definition_id.clone(),
            Numeric::from(40_u64),
            vec![Hash::new("fiat-invoice")],
        ))?;

        let opened = client.query_single(FindAssetEscrowById::new(escrow_id))?;
        assert_eq!(opened.status, AssetEscrowStatus::Open);
        let custody_asset_id = AssetId::of(asset_definition_id.clone(), opened.custody.clone());
        client.submit_blocking(Grant::account_permission(
            Permission::from(CanTransferAsset {
                asset: custody_asset_id.clone(),
            }),
            seller.clone(),
        ))?;
        assert!(
            client
                .submit_blocking(Transfer::asset_numeric(
                    custody_asset_id,
                    Numeric::from(1_u64),
                    seller.clone(),
                ))
                .is_err(),
            "active native escrow custody must not be drainable through generic transfer"
        );

        buyer_client.submit_blocking(AcceptAssetEscrow::new(escrow_id))?;
        buyer_client.submit_blocking(MarkEscrowPaymentSent::new(escrow_id))?;
        client.submit_blocking(ReleaseAssetEscrow::new(escrow_id))?;

        let released = client.query_single(FindAssetEscrowById::new(escrow_id))?;
        assert_eq!(released.status, AssetEscrowStatus::Released);
        assert_eq!(released.buyer, Some(buyer.clone()));
        let buyer_asset_id = AssetId::of(asset_definition_id.clone(), buyer);
        let buyer_asset = client.query_single(FindAssetById::new(buyer_asset_id))?;
        let seller_asset = client.query_single(FindAssetById::new(seller_asset_id))?;
        assert_eq!(*buyer_asset.value(), Numeric::from(40_u64));
        assert_eq!(*seller_asset.value(), Numeric::from(60_u64));

        let buyer_escrows = client
            .query(FindAssetEscrowsByBuyer::new(
                released.buyer.clone().expect("buyer recorded"),
            ))
            .execute_all()?;
        assert!(buyer_escrows.iter().any(|record| record.id == escrow_id));

        Ok(())
    })();

    if sandbox::handle_result(result, context)?.is_none() {
        return Ok(());
    }
    Ok(())
}
