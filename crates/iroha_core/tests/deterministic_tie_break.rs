//! Deterministic scheduler tie-break test.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::items_after_statements)]
//! Build several independent transactions (no conflicts) and assert that the
//! execution order is a stable sort by (`call_hash`, index) regardless of the
//! input order.

use std::borrow::Cow;

use iroha_core::block::{BlockBuilder, ValidBlock};
use iroha_data_model::prelude::*;

fn build_world() -> (
    iroha_core::state::State,
    ChainId,
    Vec<(AccountId, iroha_crypto::KeyPair)>,
) {
    let chain_id: ChainId = "chain".parse().unwrap();
    // Create 4 accounts in one domain and seed their assets
    let (a1, k1) = iroha_test_samples::gen_account_in("wonderland");
    let (a2, k2) = iroha_test_samples::gen_account_in("wonderland");
    let (a3, k3) = iroha_test_samples::gen_account_in("wonderland");
    let (a4, k4) = iroha_test_samples::gen_account_in("wonderland");
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&a1);
    let ad: AssetDefinition = AssetDefinition::new(
        iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "coin".parse().unwrap(),
        ),
        NumericSpec::default(),
    )
    .build(&a1);
    let acc1 = Account::new_in_domain(a1.clone(), domain_id.clone()).build(&a1);
    let acc2 = Account::new_in_domain(a2.clone(), domain_id.clone()).build(&a1);
    let acc3 = Account::new_in_domain(a3.clone(), domain_id.clone()).build(&a1);
    let acc4 = Account::new_in_domain(a4.clone(), domain_id).build(&a1);
    // Initialize zero balances for determinism
    let a1_coin = AssetId::of(ad.id().clone(), a1.clone());
    let a2_coin = AssetId::of(ad.id().clone(), a2.clone());
    let a3_coin = AssetId::of(ad.id().clone(), a3.clone());
    let a4_coin = AssetId::of(ad.id().clone(), a4.clone());
    let z = iroha_primitives::numeric::Numeric::new(0, 0);
    let a0 = Asset::new(a1_coin, z.clone());
    let b0 = Asset::new(a2_coin, z.clone());
    let c0 = Asset::new(a3_coin, z.clone());
    let d0 = Asset::new(a4_coin, z);

    let world = iroha_core::state::World::with_assets(
        [domain],
        [acc1, acc2, acc3, acc4],
        [ad],
        [a0, b0, c0, d0],
        [],
    );
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = iroha_core::query::store::LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = iroha_core::state::State::new(
        world,
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let state = iroha_core::state::State::new(world, kura, query);
    (
        state,
        chain_id,
        vec![(a1, k1), (a2, k2), (a3, k3), (a4, k4)],
    )
}

fn run_block(state: &mut iroha_core::state::State, txs: Vec<SignedTransaction>) -> ValidBlock {
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
    let _ = sb.commit();
    vb
}

#[test]
fn scheduler_tie_break_stable_by_call_hash_then_index() {
    let (mut state, chain_id, accs) = build_world();
    // Build 4 independent log transactions
    let txs: Vec<SignedTransaction> = accs
        .iter()
        .enumerate()
        .map(|(i, (aid, kp))| {
            let msg = format!("log-{i}");
            TransactionBuilder::new(chain_id.clone(), aid.clone())
                .with_instructions([Log::new(Level::INFO, msg)])
                .sign(kp.private_key())
        })
        .collect();

    // Canonical execution order captured by running the identity ordering once.
    let expected_block = run_block(&mut state, txs.clone());
    let expected: Vec<_> = expected_block
        .as_ref()
        .entrypoint_hashes()
        .take(4)
        .collect();

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
        let vb = run_block(&mut state, permuted_txs);
        // Extract execution order hashes (entrypoints)
        let got: Vec<_> = vb
            .as_ref()
            .entrypoint_hashes()
            .take(4) // external txs only
            .collect();
        assert_eq!(got, expected, "execution order must be stable");
        // All must be approved
        assert!(vb.as_ref().results().take(4).all(|r| r.as_ref().is_ok()));
    }
}

#[test]
fn scheduler_tie_break_randomized_input_orders() {
    // Same setup as the basic test, but exercise many randomized permutations
    let (mut state, chain_id, accs) = build_world();
    // Build 4 independent log transactions
    let txs: Vec<SignedTransaction> = accs
        .iter()
        .enumerate()
        .map(|(i, (aid, kp))| {
            let msg = format!("log-{i}");
            TransactionBuilder::new(chain_id.clone(), aid.clone())
                .with_instructions([Log::new(Level::INFO, msg)])
                .sign(kp.private_key())
        })
        .collect();

    // Canonical execution order from the initial run (identity permutation).
    let expected_block = run_block(&mut state, txs.clone());
    let expected: Vec<_> = expected_block
        .as_ref()
        .entrypoint_hashes()
        .take(4)
        .collect();

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
        let vb = run_block(&mut state, permuted);
        let got: Vec<_> = vb.as_ref().entrypoint_hashes().take(4).collect();
        assert_eq!(
            got, expected,
            "execution order must be stable across permutations"
        );
        assert!(vb.as_ref().results().take(4).all(|r| r.as_ref().is_ok()));
    }
}
