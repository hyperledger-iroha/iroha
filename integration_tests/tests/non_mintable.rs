//! Integration tests covering single-mint asset semantics.

use eyre::Result;
use integration_tests::sandbox;
use iroha::data_model::{isi::InstructionBox, prelude::*};
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;

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
        let asset_definition_id = "xor_once#wonderland"
            .parse::<AssetDefinitionId>()
            .expect("Valid");
        let create_asset = Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id.clone()).mintable_once(),
        );

        let metadata = Metadata::default();
        let mint = Mint::asset_numeric(
            200_u32,
            AssetId::new(asset_definition_id.clone(), account_id.clone()),
        );
        let instructions: [InstructionBox; 2] = [create_asset.into(), mint.clone().into()];
        let tx = test_client.build_transaction(instructions, metadata);

        test_client.submit_transaction_blocking(&tx)?;
        assert!(
            test_client
                .query(FindAssets::new())
                .execute_all()?
                .iter()
                .any(|asset| {
                    *asset.id().account() == account_id
                        && *asset.id().definition() == asset_definition_id
                        && *asset.value() == numeric!(200)
                })
        );

        assert!(test_client.submit_all_blocking([mint]).is_err());
    }

    // Case 2: if registered with non-zero value, it cannot be minted again.
    {
        let asset_definition_id = "xor_seeded#wonderland"
            .parse::<AssetDefinitionId>()
            .expect("Valid");
        let create_asset = Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id.clone()).mintable_once(),
        );

        let asset_id = AssetId::new(asset_definition_id.clone(), account_id.clone());
        let register_asset = Mint::asset_numeric(1_u32, asset_id.clone());

        test_client.submit_all_blocking::<InstructionBox>([
            create_asset.into(),
            register_asset.clone().into(),
        ])?;
        assert!(
            test_client
                .query(FindAssets::new())
                .execute_all()?
                .iter()
                .any(|asset| {
                    *asset.id().account() == account_id
                        && *asset.id().definition() == asset_definition_id
                        && *asset.value() == numeric!(1)
                })
        );

        assert!(test_client.submit_blocking(register_asset).is_err());

        let mint = Mint::asset_numeric(1u32, asset_id);
        assert!(test_client.submit_blocking(mint).is_err());
    }

    Ok(())
}
