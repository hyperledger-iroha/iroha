//! Deterministic proposal-order execution test.

#![allow(clippy::items_after_statements)]
//! Independent account metadata events expose execution order. Ordinary network transactions
//! follow proposal position, and block results retain those positions after execution.
use iroha_core::sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig};
use iroha_data_model::prelude::*;
use iroha_model_base::chain::ChainId;
use std::num::NonZeroU32;
/// Universal accounts and their signing keys.
type Accounts = Vec<(AccountId, iroha_crypto::KeyPair)>;
fn build_world() -> (CertifiedTestChain, Accounts) {
    let chain_id: ChainId = "chain".parse().unwrap();
    // Each universal account owns a separate metadata key, so the transactions do not conflict.
    let accounts: Accounts = (0..4)
        .map(|_| iroha_test_samples::gen_account_in("wonderland"))
        .collect();
    let (a1, _) = &accounts[0];
    let world = iroha_core::state::World::with(
        [],
        accounts
            .iter()
            .map(|(account, _)| Account::new(account.clone()).build(a1)),
        [],
    );
    let mut config = TestChainConfig::new(world, 1000);
    config.chain_id = chain_id;
    (CertifiedTestChain::start(config).unwrap(), accounts)
}
/// Execute and publish one proposal. A completed execution is immutable, so the execution
/// order is read from the events its publication delivers, which preserve that order.
fn run_block(
    chain: &mut CertifiedTestChain,
    txs: Vec<SignedTransaction>,
) -> (SignedBlock, Vec<AccountId>) {
    let payload_hashes: Vec<_> = txs
        .iter()
        .map(SignedTransaction::hash_as_entrypoint)
        .collect();
    chain
        .take_events()
        .expect("drain events delivered before the observed block");
    let proposal = chain.proposal(None, txs);
    let committed = chain.commit_proposal(
        proposal,
        Signers::Quorum,
        iroha_sumeragi::types::ControlWitness::default(),
    );
    let execution_order = chain
        .take_events()
        .expect("native publication events")
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
    let block = committed.block().as_ref().clone();
    assert_eq!(
        block.network_input_hashes().collect::<Vec<_>>(),
        payload_hashes,
        "entrypoint hashes must preserve proposal order"
    );
    (block, execution_order)
}
/// Distinct signed copy of a round's transaction: a published transaction is never admitted
/// again, so every proposal round signs the same instructions under its own nonce.
fn sign_for_round(
    network_id: NetworkId,
    (authority, key_pair): &(AccountId, iroha_crypto::KeyPair),
    instruction: impl Into<InstructionBox>,
    round: usize,
) -> SignedTransaction {
    let instruction: InstructionBox = instruction.into();
    let mut builder = TransactionBuilder::new(
        network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction]);
    builder.set_nonce(
        u32::try_from(round)
            .ok()
            .and_then(|round| round.checked_add(1))
            .and_then(NonZeroU32::new)
            .expect("round nonce fits"),
    );
    builder.sign(key_pair.private_key())
}
fn independent_transactions(
    network_id: NetworkId,
    accs: &[(AccountId, iroha_crypto::KeyPair)],
    round: usize,
) -> Vec<SignedTransaction> {
    accs.iter()
        .enumerate()
        .map(|(i, signer)| {
            sign_for_round(
                network_id,
                signer,
                SetKeyValue::account(
                    signer.0.clone(),
                    "scheduler_marker".parse().unwrap(),
                    iroha_primitives::json::Json::new(format!("transaction-{i}")),
                ),
                round,
            )
        })
        .collect()
}
fn proposal_order(txs: &[SignedTransaction]) -> Vec<AccountId> {
    txs.iter().map(|tx| tx.authority().clone()).collect()
}
#[test]
fn ordinary_network_execution_follows_proposal_order() {
    let (mut chain, accs) = build_world();
    let network_id = chain.network_id();
    // Each permutation is a distinct proposal order, irrespective of entrypoint hashes.
    let perms: Vec<Vec<usize>> = vec![
        vec![0, 1, 2, 3], // identity
        vec![3, 2, 1, 0], // reverse
        vec![1, 2, 3, 0], // rotate left 1
        vec![2, 3, 0, 1], // rotate left 2
        vec![0, 2, 1, 3], // swap middle
    ];
    for (round, p) in perms.into_iter().enumerate() {
        let txs = independent_transactions(network_id, &accs, round);
        let permuted_txs: Vec<_> = p.iter().map(|&i| txs[i].clone()).collect();
        let expected = proposal_order(&permuted_txs);
        let (vb, got) = run_block(&mut chain, permuted_txs);
        assert_eq!(got, expected, "execution must follow proposal order");
        // All must be approved
        assert!((0..4).all(|index| {
            vb.network_output_at(index)
                .expect("validated transaction has an output")
                .1
                .result
                .as_ref()
                .is_ok()
        }));
    }
}
#[test]
fn ordinary_network_execution_follows_randomized_proposal_orders() {
    // Exercise many distinct proposal orders with a fixed, reproducible shuffle.
    let (mut chain, accs) = build_world();
    let network_id = chain.network_id();
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
    for round in 0..64 {
        let txs = independent_transactions(network_id, &accs, round);
        let permuted = shuffle(&mut rng, &txs);
        let expected = proposal_order(&permuted);
        let (vb, got) = run_block(&mut chain, permuted);
        assert_eq!(
            got, expected,
            "execution must follow each proposal's input order"
        );
        assert!((0..4).all(|index| {
            vb.network_output_at(index)
                .expect("validated transaction has an output")
                .1
                .result
                .as_ref()
                .is_ok()
        }));
    }
}

#[test]
fn scheduler_results_preserve_payload_indices_after_reordering() {
    let (mut chain, accs) = build_world();
    let network_id = chain.network_id();
    for round in 0..accs.len() {
        let mut txs = independent_transactions(network_id, &accs, round);
        let authority = accs[0].0.clone();
        txs[0] = sign_for_round(
            network_id,
            &accs[0],
            RemoveKeyValue::account(authority, "missing".parse().unwrap()),
            round,
        );
        let rejected_hash = txs[0].hash_as_entrypoint();
        // Descending hashes show that ordinary execution follows proposal position,
        // including when hash priority would choose the reverse order; each round
        // rotates that order by one more position.
        txs.sort_by_key(|tx| std::cmp::Reverse(tx.hash_as_entrypoint()));
        txs.rotate_left(round);
        let expected: Vec<_> = txs
            .iter()
            .filter(|tx| tx.hash_as_entrypoint() != rejected_hash)
            .map(|tx| tx.authority().clone())
            .collect();
        let (block, execution_order) = run_block(&mut chain, txs.clone());
        assert_eq!(execution_order, expected);
        assert_eq!(block.execution_outputs().len(), txs.len());
        for (index, tx) in txs.iter().enumerate() {
            let result = &block
                .network_output_at(u32::try_from(index).expect("input index fits u32"))
                .expect("validated transaction has an output")
                .1
                .result;
            assert_eq!(
                result.as_ref().is_err(),
                tx.hash_as_entrypoint() == rejected_hash,
                "a transaction result must remain attached to its payload index"
            );
        }
    }
}
