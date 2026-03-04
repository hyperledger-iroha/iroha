#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#[path = "./common.rs"]
mod common;

use std::sync::Arc;

use common::*;
use iroha_core::{
    block::CommittedBlock, prelude::*, state::State, sumeragi::network_topology::Topology,
};
use iroha_data_model::peer::PeerId;
use iroha_test_samples::gen_account_in;

pub struct StateApplyBlocks {
    state: State,
    blocks: Vec<CommittedBlock>,
    topology: Topology,
}

impl StateApplyBlocks {
    /// Create [`State`] and blocks for benchmarking
    ///
    /// # Errors
    /// - Failed to parse [`AccountId`]
    /// - Failed to generate [`KeyPair`]
    /// - Failed to create instructions for block
    pub fn setup(rt: &tokio::runtime::Handle) -> Self {
        let domains = 10;
        let accounts_per_domain = 100;
        let assets_per_domain = 100;
        let (domain_ids, account_ids, asset_definition_ids) =
            generate_ids(domains, accounts_per_domain, assets_per_domain);
        let (peer_public_key, peer_private_key) = KeyPair::random().into_parts();
        let peer_id = PeerId::new(peer_public_key);
        let topology = Topology::new(vec![peer_id]);
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let state = build_state(rt, &alice_id, alice_keypair.private_key());

        let nth = 10;
        let instructions = [
            populate_state(&domain_ids, &account_ids, &asset_definition_ids, &alice_id),
            delete_every_nth(&domain_ids, &account_ids, &asset_definition_ids, nth),
            restore_every_nth(&domain_ids, &account_ids, &asset_definition_ids, nth),
        ];

        let blocks = {
            // Create empty state because it will be changed during creation of block
            let state = build_state(rt, &alice_id, alice_keypair.private_key());
            instructions
                .into_iter()
                .map(|instructions| {
                    let (block, mut state_block) = create_block(
                        &state,
                        instructions,
                        alice_id.clone(),
                        alice_keypair.private_key(),
                        &topology,
                        &peer_private_key,
                    );
                    let _events =
                        state_block.apply_without_execution(&block, topology.as_ref().to_owned());
                    state_block.commit().unwrap();
                    let block_arc = Arc::new(block.clone().into());
                    let state_view = state.view();
                    state_view
                        .kura()
                        .store_block(block_arc)
                        .expect("store block in bench setup");
                    block
                })
                .collect::<Vec<_>>()
        };

        Self {
            state,
            blocks,
            topology,
        }
    }

    /// Run benchmark body.
    ///
    /// # Errors
    /// - Not enough blocks
    /// - Failed to apply block
    ///
    /// # Panics
    /// If state height isn't updated after applying block
    pub fn measure(
        Self {
            state,
            blocks,
            topology,
        }: &Self,
    ) {
        let base_height = {
            let view = state.view();
            view.height()
        };
        for (block, i) in blocks.iter().zip(1..) {
            let mut state_block = state.block(block.as_ref().header());
            let _events = state_block.apply(block, topology.as_ref().to_owned());
            state_block.commit().unwrap();
            assert_eq!(state.view().height(), base_height + i);
        }
    }
}
