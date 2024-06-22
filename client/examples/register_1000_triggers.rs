//! Example of registering multiple triggers
//! Used to show Iroha's trigger deduplication capabilities

use iroha::{
    client::Client,
    data_model::{prelude::*, trigger::TriggerId},
};
use iroha_genesis::{GenesisBlock, GenesisBuilder};
use iroha_primitives::unique_vec;
use irohad::samples::{construct_executor, get_config};
use test_network::{
    get_chain_id, get_key_pair, wait_for_genesis_committed_with_max_retries, Peer as TestPeer,
    PeerBuilder, TestClient, TestRuntime,
};
use test_samples::gen_account_in;
use tokio::runtime::Runtime;

fn generate_genesis(
    num_triggers: u32,
    chain_id: ChainId,
    genesis_key_pair: &iroha_crypto::KeyPair,
    topology: Vec<PeerId>,
) -> Result<GenesisBlock, Box<dyn std::error::Error>> {
    let builder = GenesisBuilder::default();

    let wasm =
        iroha_wasm_builder::Builder::new("tests/integration/smartcontracts/mint_rose_trigger")
            .show_output()
            .build()?
            .optimize()?
            .into_bytes()?;
    let wasm = WasmSmartContract::from_compiled(wasm);
    let (account_id, _account_keypair) = gen_account_in("wonderland");

    let build_trigger = |trigger_id: TriggerId| {
        Trigger::new(
            trigger_id.clone(),
            Action::new(
                wasm.clone(),
                Repeats::Indefinitely,
                account_id.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id)
                    .under_authority(account_id.clone()),
            ),
        )
    };

    let builder = (0..num_triggers)
        .map(|i| {
            let trigger_id = i.to_string().parse::<TriggerId>().unwrap();
            let trigger = build_trigger(trigger_id);
            Register::trigger(trigger)
        })
        .fold(builder, GenesisBuilder::append_instruction);

    let executor = construct_executor("../default_executor").expect("Failed to construct executor");
    Ok(builder.build_and_sign(executor, chain_id, genesis_key_pair, topology))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut peer: TestPeer = <TestPeer>::new().expect("Failed to create peer");

    let chain_id = get_chain_id();
    let genesis_key_pair = get_key_pair(test_network::Signatory::Genesis);
    let topology = vec![peer.id.clone()];
    let mut configuration = get_config(
        unique_vec![peer.id.clone()],
        chain_id.clone(),
        get_key_pair(test_network::Signatory::Peer),
        genesis_key_pair.public_key(),
    );

    // Increase executor limits for large genesis
    configuration.chain_wide.executor_runtime.fuel_limit = u64::MAX;
    configuration.chain_wide.executor_runtime.max_memory = u32::MAX.into();

    let genesis = generate_genesis(1_000_u32, chain_id, &genesis_key_pair, topology)?;

    let builder = PeerBuilder::new()
        .with_genesis(genesis)
        .with_config(configuration);

    let rt = Runtime::test();
    let test_client = Client::test(&peer.api_address);
    rt.block_on(builder.start_with_peer(&mut peer));

    wait_for_genesis_committed_with_max_retries(&vec![test_client.clone()], 0, 600);

    Ok(())
}
