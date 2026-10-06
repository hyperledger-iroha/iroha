//! Genuine signed genesis for native proof admission and contract-binding fixtures.
use iroha_core::{
    state::{State, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};

/// Retain the actual committed genesis/root identity in an exclusively owned test State.
pub(crate) fn certified_state(world: World) -> State {
    let chain =
        CertifiedTestChain::start(TestChainConfig::new(world, 0)).expect("authenticated genesis");
    let state = std::sync::Arc::clone(chain.state());
    drop(chain);
    std::sync::Arc::try_unwrap(state).unwrap_or_else(|_| panic!("exclusive fixture State"))
}
