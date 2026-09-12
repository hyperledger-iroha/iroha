//! Deterministic scheduler tie-break test.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::items_after_statements)]
//! Independent account metadata events expose scheduler execution order; block entrypoint hashes
//! and results retain payload order so their indices identify the serialized transactions.
use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    governance::manifest::LaneManifestRegistry,
};
use iroha_data_model::prelude::*;
use iroha_model_base::chain::ChainId;
use std::{borrow::Cow, sync::Arc};
fn build_world() -> (
    iroha_core::state::State,
    NetworkId,
    Vec<(AccountId, iroha_crypto::KeyPair)>,
) {
    let chain_id: ChainId = "chain".parse().unwrap();
    // Each universal account owns a separate metadata key, so the transactions do not conflict.
    let (a1, k1) = iroha_test_samples::gen_account_in("wonderland");
    let (a2, k2) = iroha_test_samples::gen_account_in("wonderland");
    let (a3, k3) = iroha_test_samples::gen_account_in("wonderland");
    let (a4, k4) = iroha_test_samples::gen_account_in("wonderland");
    let acc1 = Account::new(a1.clone()).build(&a1);
    let acc2 = Account::new(a2.clone()).build(&a1);
    let acc3 = Account::new(a3.clone()).build(&a1);
    let acc4 = Account::new(a4.clone()).build(&a1);
    let world = iroha_core::state::World::with([], [acc1, acc2, acc3, acc4], []);
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    let state =
        iroha_core::state::State::new_with_chain_for_testing(world, kura, query, chain_id.clone());
    let network_id = *state.network_id_ref();
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
    (
        state,
        network_id,
        vec![(a1, k1), (a2, k2), (a3, k3), (a4, k4)],
    )
}
fn run_block(
    state: &iroha_core::state::State,
    txs: Vec<SignedTransaction>,
) -> (ValidBlock, Vec<AccountId>) {
    let payload_hashes: Vec<_> = txs
        .iter()
        .map(SignedTransaction::hash_as_entrypoint)
        .collect();
    // Build block
    let acc: Vec<_> = txs
        .into_iter()
        .map(|t| iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(t)))
        .collect();
    let new_block = BlockBuilder::new(acc)
        .chain(0, None)
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    let mut sb = state.block(new_block.header());
    let vb = new_block
        .validate_and_record_transactions(&mut sb)
        .unpack(|_| {});
    let execution_order = sb
        .world
        .take_external_events()
        .into_iter()
        .filter_map(|event| {
            let EventBox::Data(event) = event else {
                return None;
            };
            match event.as_ref() {
                DataEvent::Account(AccountEvent::MetadataInserted(change)) => {
                    Some(change.target().clone())
                }
                _ => None,
            }
        })
        .collect();
    assert_eq!(
        vb.as_ref().entrypoint_hashes().collect::<Vec<_>>(),
        payload_hashes,
        "entrypoint hashes must preserve payload order independently of execution order"
    );
    // Drop the overlay so every permutation starts from identical account state.
    (vb, execution_order)
}
fn independent_transactions(
    network_id: NetworkId,
    accs: &[(AccountId, iroha_crypto::KeyPair)],
) -> Vec<SignedTransaction> {
    accs.iter()
        .enumerate()
        .map(|(i, (aid, kp))| {
            TransactionBuilder::new(
                network_id,
                aid.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([SetKeyValue::account(
                aid.clone(),
                "scheduler_marker".parse().unwrap(),
                iroha_primitives::json::Json::new(format!("transaction-{i}")),
            )])
            .sign(kp.private_key())
        })
        .collect()
}
fn expected_execution_order(txs: &[SignedTransaction]) -> Vec<AccountId> {
    let mut order: Vec<_> = txs.iter().enumerate().collect();
    order.sort_by_key(|(index, tx)| (tx.hash_as_entrypoint(), *index));
    order
        .into_iter()
        .map(|(_, tx)| tx.authority().clone())
        .collect()
}
#[test]
fn scheduler_tie_break_stable_by_call_hash_then_index() {
    let (state, network_id, accs) = build_world();
    let txs = independent_transactions(network_id, &accs);
    let expected = expected_execution_order(&txs);
    // Define a few deterministic permutations
    let perms: Vec<Vec<usize>> = vec![
        vec![0, 1, 2, 3], // identity
        vec![3, 2, 1, 0], // reverse
        vec![1, 2, 3, 0], // rotate left 1
        vec![2, 3, 0, 1], // rotate left 2
        vec![0, 2, 1, 3], // swap middle
    ];
    for p in perms {
        let permuted_txs: Vec<_> = p.iter().map(|&i| txs[i].clone()).collect();
        let (vb, got) = run_block(&state, permuted_txs);
        assert_eq!(got, expected, "execution order must be stable");
        // All must be approved
        assert!(vb.as_ref().results().take(4).all(|r| r.as_ref().is_ok()));
    }
}
#[test]
fn scheduler_tie_break_randomized_input_orders() {
    // Same setup as the basic test, but exercise many randomized permutations
    let (state, network_id, accs) = build_world();
    let txs = independent_transactions(network_id, &accs);
    let expected = expected_execution_order(&txs);
    // Deterministic LCG for shuffling
    #[derive(Clone)]
    struct Lcg(u64);
    impl Lcg {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next(&mut self) -> u64 {
            // Constants from Numerical Recipes
            self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            self.0
        }
    }
    fn shuffle<T: Clone>(rng: &mut Lcg, v: &[T]) -> Vec<T> {
        let mut out = v.to_vec();
        // Fisher–Yates using our LCG
        let n = out.len();
        for i in (1..n).rev() {
            let j = usize::try_from(rng.next()).unwrap() % (i + 1);
            out.swap(i, j);
        }
        out
    }
    // Exercise many randomized permutations with a fixed seed for reproducibility
    let mut rng = Lcg::new(0x00C0_FFEE);
    for _ in 0..64 {
        let permuted = shuffle(&mut rng, &txs);
        let (vb, got) = run_block(&state, permuted);
        assert_eq!(
            got, expected,
            "execution order must be stable across permutations"
        );
        assert!(vb.as_ref().results().take(4).all(|r| r.as_ref().is_ok()));
    }
}

#[test]
fn scheduler_results_preserve_payload_indices_after_reordering() {
    let (state, network_id, accs) = build_world();
    let mut txs = independent_transactions(network_id, &accs);
    let (authority, keypair) = &accs[0];
    txs[0] = TransactionBuilder::new(
        network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([RemoveKeyValue::account(
        authority.clone(),
        "missing".parse().unwrap(),
    )])
    .sign(keypair.private_key());
    let rejected_hash = txs[0].hash_as_entrypoint();
    let expected: Vec<_> = expected_execution_order(&txs)
        .into_iter()
        .filter(|account| account != authority)
        .collect();

    // Descending payload hashes guarantee a different order from the scheduler.
    txs.sort_by_key(|tx| std::cmp::Reverse(tx.hash_as_entrypoint()));
    for _ in 0..txs.len() {
        let (block, execution_order) = run_block(&state, txs.clone());
        assert_eq!(execution_order, expected);
        let results: Vec<_> = block.as_ref().results().collect();
        assert_eq!(results.len(), txs.len());
        for (tx, result) in txs.iter().zip(results) {
            assert_eq!(
                result.as_ref().is_err(),
                tx.hash_as_entrypoint() == rejected_hash,
                "a transaction result must remain attached to its payload index"
            );
        }
        txs.rotate_left(1);
    }
}
