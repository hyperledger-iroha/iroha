#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Native asset escrow integration scenarios.

use std::time::{Duration, Instant};

use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        isi::escrow::{
            AcceptAssetEscrow, MarkEscrowPaymentSent, OpenAssetEscrow, ReleaseAssetEscrow,
        },
        prelude::*,
    },
};
use iroha_crypto::Hash;
use iroha_executor_data_model::permission::asset::CanTransferAsset;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, gen_account_in};

fn wait_for_escrow_status(
    client: &Client,
    escrow_id: EscrowId,
    expected: AssetEscrowStatus,
    context: &str,
) -> Result<AssetEscrowRecord> {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);

    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "escrow was not queried".to_owned();

    while Instant::now() < deadline {
        match client.query_single(FindAssetEscrowById::new(escrow_id)) {
            Ok(record) => {
                last_observed = format!("{:?}", record.status);
                if record.status == expected {
                    return Ok(record);
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }

    Err(eyre!(
        "timed out waiting for escrow {escrow_id:?} to become {expected:?} after {context}; last_observed={last_observed}"
    ))
}

fn wait_for_asset_value(
    client: &Client,
    asset_id: &AssetId,
    expected: &Numeric,
    context: &str,
) -> Result<Asset> {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);

    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "asset was not queried".to_owned();

    while Instant::now() < deadline {
        match client.query_single(FindAssetById::new(asset_id.clone())) {
            Ok(asset) => {
                last_observed = format!("{}", asset.value());
                if asset.value() == expected {
                    return Ok(asset);
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }

    Err(eyre!(
        "timed out waiting for asset {asset_id} to equal {expected} after {context}; last_observed={last_observed}"
    ))
}

fn wait_for_buyer_escrow(
    client: &Client,
    buyer: &AccountId,
    escrow_id: EscrowId,
    context: &str,
) -> Result<Vec<AssetEscrowRecord>> {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);

    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "buyer escrow index was not queried".to_owned();

    while Instant::now() < deadline {
        match client
            .query(FindAssetEscrowsByBuyer::new(buyer.clone()))
            .execute_all()
        {
            Ok(records) => {
                last_observed = format!(
                    "{:?}",
                    records.iter().map(|record| record.id).collect::<Vec<_>>()
                );
                if records.iter().any(|record| record.id == escrow_id) {
                    return Ok(records);
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }

    Err(eyre!(
        "timed out waiting for buyer escrow index to include {escrow_id:?} after {context}; last_observed={last_observed}"
    ))
}

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

        let opened =
            wait_for_escrow_status(&client, escrow_id, AssetEscrowStatus::Open, "open escrow")?;
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

        let released = wait_for_escrow_status(
            &client,
            escrow_id,
            AssetEscrowStatus::Released,
            "release escrow",
        )?;
        assert_eq!(released.buyer, Some(buyer.clone()));
        let buyer_asset_id = AssetId::of(asset_definition_id.clone(), buyer.clone());
        let buyer_asset = wait_for_asset_value(
            &client,
            &buyer_asset_id,
            &Numeric::from(40_u64),
            "release escrow buyer balance",
        )?;
        let seller_asset = wait_for_asset_value(
            &client,
            &seller_asset_id,
            &Numeric::from(60_u64),
            "release escrow seller balance",
        )?;
        assert_eq!(*buyer_asset.value(), Numeric::from(40_u64));
        assert_eq!(*seller_asset.value(), Numeric::from(60_u64));

        let buyer_escrows = wait_for_buyer_escrow(
            &client,
            released.buyer.as_ref().expect("buyer recorded"),
            escrow_id,
            "release escrow buyer index",
        )?;
        assert!(buyer_escrows.iter().any(|record| record.id == escrow_id));

        Ok(())
    })();

    if sandbox::handle_result(result, context)?.is_none() {
        return Ok(());
    }
    Ok(())
}
