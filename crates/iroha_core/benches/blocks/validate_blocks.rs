#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#[path = "./common.rs"]
mod common;

use std::sync::{Arc, OnceLock};

use common::*;
use iroha_core::{prelude::*, state::State, sumeragi::network_topology::Topology};
use iroha_crypto::Algorithm;
use iroha_data_model::{isi::InstructionBox, prelude::*};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

type InstructionBatch = Arc<[InstructionBox]>;

const BENCH_DOMAINS: usize = 4;
const BENCH_ACCOUNTS_PER_DOMAIN: usize = 25;
const BENCH_ASSETS_PER_DOMAIN: usize = 25;
const BENCH_DELETE_EVERY_NTH: usize = 5;

/// Lazily prepared instruction batches shared across benchmark iterations to
/// keep setup work bounded while still exercising meaningful block validation.
fn instruction_batches() -> &'static [InstructionBatch; 3] {
    static BATCHES: OnceLock<[InstructionBatch; 3]> = OnceLock::new();
    BATCHES.get_or_init(|| {
        let (domain_ids, account_ids, asset_definition_ids) = generate_ids(
            BENCH_DOMAINS,
            BENCH_ACCOUNTS_PER_DOMAIN,
            BENCH_ASSETS_PER_DOMAIN,
        );
        let owner_id = (*ALICE_ID).clone();
        [
            Arc::from(
                populate_state(&domain_ids, &account_ids, &asset_definition_ids, &owner_id)
                    .into_boxed_slice(),
            ),
            Arc::from(
                delete_every_nth(
                    &domain_ids,
                    &account_ids,
                    &asset_definition_ids,
                    BENCH_DELETE_EVERY_NTH,
                )
                .into_boxed_slice(),
            ),
            Arc::from(
                restore_every_nth(
                    &domain_ids,
                    &account_ids,
                    &asset_definition_ids,
                    BENCH_DELETE_EVERY_NTH,
                )
                .into_boxed_slice(),
            ),
        ]
    })
}

pub struct StateValidateBlocks {
    state: State,
    instructions: Vec<InstructionBatch>,
    account_private_key: PrivateKey,
    account_id: AccountId,
    topology: Topology,
    peer_private_key: PrivateKey,
}

impl StateValidateBlocks {
    /// Create [`State`] and blocks for benchmarking
    ///
    /// # Panics
    ///
    /// - Failed to parse [`AccountId`]
    /// - Failed to generate [`KeyPair`]
    /// - Failed to create instructions for block
    pub fn setup(rt: &tokio::runtime::Handle) -> Self {
        let (peer_public_key, peer_private_key) =
            KeyPair::random_with_algorithm(Algorithm::BlsNormal).into_parts();
        let topology = Topology::new(vec![PeerId::new(peer_public_key)]);
        let alice_id = (*ALICE_ID).clone();
        let alice_keypair = (*ALICE_KEYPAIR).clone();
        let state = build_state(rt, &alice_id, alice_keypair.private_key());
        let instructions = instruction_batches().to_vec();

        Self {
            state,
            instructions,
            account_private_key: alice_keypair.private_key().clone(),
            account_id: alice_id,
            topology,
            peer_private_key,
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
            instructions,
            account_private_key,
            account_id,
            topology,
            peer_private_key,
        }: Self,
    ) {
        let base_height = {
            let view = state.view();
            view.height()
        };
        for (instruction_batch, i) in instructions.into_iter().zip(1..) {
            let (block, mut state_block) = create_block(
                &state,
                instruction_batch.iter().cloned(),
                account_id.clone(),
                &account_private_key,
                &topology,
                &peer_private_key,
            );
            let _events = state_block.apply_without_execution(&block, topology.as_ref().to_owned());
            assert_eq!(state_block.height(), base_height + i);
            state_block.commit().unwrap();

            let block_arc = Arc::new(block.into());
            let state_view = state.view();
            state_view
                .kura()
                .store_block(block_arc)
                .expect("store block in bench setup");
        }
    }
}
