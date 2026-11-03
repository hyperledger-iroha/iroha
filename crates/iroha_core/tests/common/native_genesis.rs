//! Genuine signed genesis for native proof admission and contract-binding fixtures.
use iroha_core::{
    state::{State, World},
    sumeragi::{
        startup,
        test_chain::{CertifiedTestChain, TestChainConfig},
    },
};
use iroha_data_model::prelude::AccountId;

/// Apply original signed genesis before any `StateExecutor` retains the fixture State.
pub fn certified_state(world: World) -> State {
    certified_state_with(TestChainConfig::new(world, 0))
}

/// Apply the original signed genesis of an explicitly configured fixture chain.
///
/// Instructions that only genesis may execute belong in `config.genesis_instructions`, and
/// node configuration bound by the signed genesis (ZK, governance, Nexus, pipeline) belongs in
/// `config` before signing. Ordinary fixture blocks start at the height after the genesis.
pub fn certified_state_with(config: TestChainConfig) -> State {
    let account = AccountId::new(config.genesis_key.public_key().clone());
    let mode = config.consensus_mode;
    let prepared = CertifiedTestChain::prepare(config).expect("prepare authenticated genesis");
    let state = std::sync::Arc::try_unwrap(prepared.state)
        .unwrap_or_else(|_| panic!("unpublished fixture State is unique"));
    startup::apply_genesis(
        &state,
        prepared.genesis.block().clone(),
        &account,
        mode.into(),
        None,
    )
    .expect("apply authenticated genesis");
    state
}
