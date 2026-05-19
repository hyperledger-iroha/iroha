#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests covering single-mint asset semantics.

use std::time::{Duration, Instant};

use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{isi::InstructionBox, prelude::*},
};
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;

fn wait_for_asset_value(
    client: &Client,
    asset_id: &AssetId,
    expected_value: &Numeric,
    context: &str,
) -> Result<Asset> {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);

    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "asset was not queried".to_owned();

    while Instant::now() < deadline {
        match client.query_single(FindAssetById::new(asset_id.clone())) {
            Ok(asset) => {
                last_observed = format!("value={:?}", asset.value());
                if asset.value() == expected_value {
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
        "timed out waiting for asset after {context}; asset_id={asset_id}; expected_value={expected_value:?}; last_observed={last_observed}"
    ))
}

#[test]
fn non_mintable_asset_minting_rules() -> Result<()> {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new().with_pipeline_time(std::time::Duration::from_secs(2)),
        stringify!(non_mintable_asset_minting_rules),
    )?
    else {
        return Ok(());
    };
    let test_client = network.client();
    let account_id = ALICE_ID.clone();

    // Case 1: mintable once can be minted once, but not twice.
    {
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("Valid"),
            "xor_once".parse().expect("Valid"),
        );
        let create_asset = Register::asset_definition(
            {
                let __asset_definition_id = asset_definition_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .mintable_once(),
        );

        let metadata = Metadata::default();
        let asset_id = AssetId::new(asset_definition_id.clone(), account_id.clone());
        let mint = Mint::asset_numeric(200_u32, asset_id.clone());
        let instructions: [InstructionBox; 2] = [create_asset.into(), mint.clone().into()];
        let tx = test_client.build_transaction(instructions, metadata);

        test_client.submit_transaction_blocking(&tx)?;
        wait_for_asset_value(&test_client, &asset_id, &numeric!(200), "first mint")?;

        assert!(test_client.submit_all_blocking([mint]).is_err());
    }

    // Case 2: if registered with non-zero value, it cannot be minted again.
    {
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("Valid"),
            "xor_seeded".parse().expect("Valid"),
        );
        let create_asset = Register::asset_definition(
            {
                let __asset_definition_id = asset_definition_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .mintable_once(),
        );

        let asset_id = AssetId::new(asset_definition_id.clone(), account_id.clone());
        let register_asset = Mint::asset_numeric(1_u32, asset_id.clone());

        test_client.submit_all_blocking::<InstructionBox>([
            create_asset.into(),
            register_asset.clone().into(),
        ])?;
        wait_for_asset_value(&test_client, &asset_id, &numeric!(1), "seeded mint")?;

        assert!(test_client.submit_blocking(register_asset).is_err());

        let mint = Mint::asset_numeric(1u32, asset_id);
        assert!(test_client.submit_blocking(mint).is_err());
    }

    Ok(())
}
