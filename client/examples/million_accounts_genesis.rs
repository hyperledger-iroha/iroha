#![allow(missing_docs, clippy::pedantic, clippy::restriction)]

use iroha_core::{
    genesis::{GenesisNetwork, GenesisNetworkTrait, GenesisTransaction, RawGenesisBlock},
    prelude::*,
    samples::get_config,
};
use iroha_data_model::prelude::*;
use test_network::{get_key_pair, Peer as TestPeer, TestRuntime};
use tokio::runtime::Runtime;

fn main() {
    fn generate_accounts(num: u32) -> Vec<GenesisTransaction> {
        use iroha_data_model::*;

        let mut ret = Vec::with_capacity(usize::try_from(num).expect("panic"));
        for i in 0_u32..num {
            ret.push(
                GenesisTransaction::new(
                    &format!("Alice-{}", i),
                    &format!("wonderland-{}", i),
                    &PublicKey::default(),
                )
                .expect("Failed to create Genesis"),
            );
            let asset_definition_id =
                AssetDefinitionId::test(&format!("xor-{}", num), &format!("wonderland-{}", num));
            let create_asset = RegisterBox::new(IdentifiableBox::from(
                AssetDefinition::new_quantity(asset_definition_id.clone()),
            ));
            ret.push(GenesisTransaction {
                isi: vec![create_asset.into()],
            });
        }
        ret
    }

    fn generate_genesis(num: u32) -> RawGenesisBlock {
        let transactions = generate_accounts(num);
        RawGenesisBlock { transactions }
    }
    let mut peer = <TestPeer>::new().expect("Failed to create peer");
    let configuration = get_config(
        std::iter::once(peer.id.clone()).collect(),
        Some(get_key_pair()),
    );
    let rt = Runtime::test();
    let genesis = GenesisNetwork::from_configuration(
        true,
        generate_genesis(1_000_000_u32),
        &configuration.genesis,
        configuration.sumeragi.max_instruction_number,
    )
    .expect("genesis creation failed");

    // This only submits the genesis. It doesn't check if the accounts
    // are created, because that check is 1) not needed for what the
    // test is actually for, 2) incredibly slow, making this sort of
    // test impractical, 3) very likely to overflow memory on systems
    // with less than 16GiB of free memory.
    rt.block_on(peer.start_with_config(genesis, configuration));
}
