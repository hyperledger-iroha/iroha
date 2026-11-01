//! Original signed-genesis State shared by the ISI gas bench and its regression.

use std::{num::NonZeroU64, sync::Arc};

use iroha_core::{
    state::{State, StateBlock, StateTransaction, World},
    sumeragi::{
        startup,
        test_chain::{CertifiedTestChain, TestChainConfig},
    },
};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId, NewAssetDefinition},
    block::BlockHeader,
    isi::InstructionBox,
};
use iroha_model_base::domain::DomainId;

/// Construct the domain-owned asset registered by the post-genesis benchmark.
///
/// # Panics
/// Panics if the benchmark's asset name is invalid.
pub fn domain_owned_asset_definition(domain: DomainId, name: &str) -> NewAssetDefinition {
    let id = AssetDefinitionId::derive_from_components(
        domain.clone(),
        name.parse().expect("valid benchmark asset name"),
    );
    AssetDefinition::numeric(id, name, AssetBalancePolicy::Global, Some(domain))
}

/// Apply the real four-validator fixture genesis to the original unpublished State.
///
/// # Panics
/// Panics if the canonical genesis fixture cannot be prepared or applied.
pub fn state_after_signed_genesis(world: World, instructions: Vec<InstructionBox>) -> State {
    let mut config = TestChainConfig::new(world, 0);
    config.genesis_instructions = instructions;
    let genesis_account = AccountId::new(config.genesis_key.public_key().clone());
    let mode = config.consensus_mode;
    let prepared = CertifiedTestChain::prepare(config).expect("prepare signed benchmark genesis");
    let state = Arc::try_unwrap(prepared.state)
        .unwrap_or_else(|_| panic!("unpublished benchmark State must remain unique"));
    startup::apply_genesis(
        &state,
        prepared.genesis.block().clone(),
        &genesis_account,
        mode.into(),
        None,
    )
    .expect("apply signed benchmark genesis");
    state
}

/// Bind the isolated benchmark overlay to its actual committed genesis parent.
///
/// # Panics
/// Panics if the fixture has lost its one-block committed prefix.
pub fn block_header(state: &State) -> BlockHeader {
    assert_eq!(
        state.committed_height(),
        1,
        "benchmark retains its genesis prefix"
    );
    BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        Some(
            state
                .view()
                .latest_block_hash()
                .expect("original genesis parent"),
        ),
        None,
        1_000,
        0,
    )
}

/// Open a bounded component invocation before executing the measured ISI.
///
/// The fixture identity is reused only across discarded overlays. It supplies
/// callback ownership without authenticating a Network input or publishing a block.
///
/// # Panics
/// Panics if the caller supplies an admitted carrier or exhausts its frozen pool.
pub fn component_transaction<'block, 'state>(
    block: &'block mut StateBlock<'state>,
) -> StateTransaction<'block, 'state> {
    block.transaction_for_fastpq_testing(iroha_crypto::Hash::new(
        b"iroha:isi-gas-calibration:component:v1",
    ))
}
