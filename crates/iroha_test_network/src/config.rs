//! Sample configuration builders

use std::path::Path;

use iroha_config::base::toml::WriteExt;
use iroha_data_model::{isi::Instruction, peer::PeerId, ChainId};
use iroha_genesis::{GenesisBlock, RawGenesisTransaction};
use iroha_primitives::unique_vec::UniqueVec;
use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;
use toml::Table;

pub fn chain_id() -> ChainId {
    ChainId::from("00000000-0000-0000-0000-000000000000")
}

pub fn base_iroha_config() -> Table {
    Table::new()
        .write("chain", chain_id())
        .write(
            ["genesis", "public_key"],
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
        )
        // There is no need in persistence in tests.
        .write(["snapshot", "mode"], "disabled")
        .write(["kura", "store_dir"], "./storage")
        .write(["network", "block_gossip_size"], 1)
        .write(["logger", "level"], "DEBUG")
}

pub fn genesis<T: Instruction>(
    extra_isi: impl IntoIterator<Item = T>,
    topology: UniqueVec<PeerId>,
) -> GenesisBlock {
    // TODO: Fix this somehow. Probably we need to make `kagami` a library (#3253).
    let json_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../defaults/genesis.json")
        .canonicalize()
        .unwrap();
    let genesis = RawGenesisTransaction::from_path(&json_path)
        .unwrap_or_else(|err| panic!("failed to parse {}\n{err}", json_path.display()));
    let mut builder = genesis.into_builder();

    for isi in extra_isi {
        builder = builder.append_instruction(isi);
    }

    let genesis_key_pair = SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone();

    builder
        .set_topology(topology.into())
        .build_and_sign(&genesis_key_pair)
        .unwrap_or_else(|err| {
            panic!(
                "\
                failed to build a genesis block from {}\n\
                have you run `scripts/build_wasm.sh` to get wasm blobs?\n\
                {err}",
                json_path.display()
            );
        })
}
