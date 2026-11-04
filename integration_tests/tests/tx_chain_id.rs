//! Validates that an exact genesis-network mismatch causes transaction rejection.
use integration_tests::sandbox;
use iroha::crypto::{Hash, HashOf};
use iroha::data_model::prelude::*;
use iroha_model_base::domain::DomainId;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, gen_account_in};
#[test]
fn send_tx_with_same_chain_name_but_different_genesis_network() {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new().with_peers(4),
        stringify!(send_tx_with_same_chain_name_but_different_genesis_network),
    )
    .unwrap() else {
        return;
    };
    let test_client = network.client();
    // Given
    let (sender_id, sender_keypair) = gen_account_in("wonderland");
    let (receiver_id, _receiver_keypair) = gen_account_in("wonderland");
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "test_asset".parse().unwrap(),
    );
    let to_transfer = Quantity::from(1_u32);
    let create_sender_account = Register::account(Account::new(sender_id.clone()));
    let create_receiver_account = Register::account(Account::new(receiver_id.clone()));
    let register_asset_definition = Register::asset_definition({
        let __asset_definition_id = asset_definition_id.clone();
        AssetDefinition::numeric(
            __asset_definition_id.clone(),
            "test_asset".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(DomainId::try_new("wonderland", "universal").unwrap()),
        )
    });
    let register_asset = Mint::asset_quantity(
        10_u32,
        AssetId::new(asset_definition_id.clone(), sender_id.clone()),
    );
    let fee_asset_definition = iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
        .parse::<AssetDefinitionId>()
        .unwrap();
    let fund_sender = Transfer::asset_quantity(
        AssetId::new(fee_asset_definition, ALICE_ID.clone()),
        100u32,
        sender_id.clone(),
    );
    test_client
        .submit_all::<InstructionBox>(
            [
                create_sender_account.into(),
                create_receiver_account.into(),
                register_asset_definition.into(),
                register_asset.into(),
                fund_sender.into(),
            ],
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .unwrap();
    let network_id = network.network_id();
    let foreign_network_id =
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"same-chain-name-different-genesis",
        )));
    assert_ne!(network_id, foreign_network_id);
    let transfer_instruction = Transfer::asset_quantity(
        AssetId::new(
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "test_asset".parse().unwrap(),
            ),
            sender_id.clone(),
        ),
        to_transfer,
        receiver_id.clone(),
    );
    let asset_transfer_builder = TransactionBuilder::new(
        network_id,
        sender_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([transfer_instruction.clone()]);
    let sender_client = integration_tests::sync::rebind_blocking_client(&test_client, |client| {
        client.account = sender_id.clone();
        client.key_pair = sender_keypair.clone();
    });
    let quote = sender_client
        .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature {
            payload: asset_transfer_builder.payload(),
        })
        .expect("quote the valid-network transfer before signing");
    assert!(
        asset_transfer_builder
            .payload()
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "fee quote must preserve the selected payer and gas bound"
    );
    let asset_transfer_tx_0 = asset_transfer_builder
        .with_fee_payment_intent(quote.intent.clone())
        .sign(sender_keypair.private_key());
    let asset_transfer_tx_1 =
        TransactionBuilder::new(foreign_network_id, sender_id.clone(), quote.intent)
            .with_instructions([transfer_instruction])
            .sign(sender_keypair.private_key());
    test_client
        .submit_transaction_and_wait(&asset_transfer_tx_0)
        .unwrap();
    let _err = test_client
        // no need for "blocking" - it must be rejected synchronously
        .submit_transaction(&asset_transfer_tx_1)
        .unwrap_err();
}
