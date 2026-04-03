//! Determinism tests for signature preverification batching under input permutations.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::items_after_statements)]
//! Ensures that, for a block containing one bad signature among valid ones,
//! the offending transaction identified by batch verification is stable across
//! different input orders.

use std::sync::Arc;

use iroha_core::{
    block::{BlockValidationError as BErr, ValidBlock},
    prelude::*,
    state::{State, StateReadOnly},
    tx::AcceptTransactionFail as AF,
};
use iroha_crypto::{Algorithm, HashOf, KeyPair, SignatureOf};
use iroha_data_model::{block::builder::BlockBuilder, prelude::*};
use nonzero_ext::nonzero;

fn setup_world_with_account(algo: Algorithm) -> (State, AccountId, ChainId, KeyPair) {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let kp = KeyPair::random_with_algorithm(algo);
    let (pubkey, _) = kp.clone().into_parts();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let account_id = AccountId::of(pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let mut state = State::new_for_testing(world, kura, query_handle);
    let chain = ChainId::from("chain");
    state.chain_id = chain.clone();
    let mut crypto_cfg = iroha_config::parameters::actual::Crypto::default();
    if !crypto_cfg.allowed_signing.contains(&algo) {
        crypto_cfg.allowed_signing.push(algo);
        crypto_cfg.allowed_signing.sort();
        crypto_cfg.allowed_signing.dedup();
    }
    state.set_crypto(crypto_cfg);
    (state, account_id, chain, kp)
}

fn enable_batch_caps(state: &mut iroha_core::state::State) {
    let mut cfg = state.view().pipeline().clone();
    // Enable modest caps for all schemes (harmless for unused ones)
    cfg.signature_batch_max_ed25519 = 8;
    cfg.signature_batch_max_secp256k1 = 8;
    #[cfg(feature = "bls")]
    {
        cfg.signature_batch_max_bls = 16;
    }
    state.set_pipeline(cfg);
}

#[derive(Clone)]
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        // Numerical Recipes LCG constants
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0
    }
}

fn shuffle<T: Clone>(rng: &mut Lcg, v: &[T]) -> Vec<T> {
    let mut out = v.to_vec();
    let n = out.len();
    for i in (1..n).rev() {
        let j = usize::try_from(rng.next()).unwrap_or(0) % (i + 1);
        out.swap(i, j);
    }
    out
}

fn mk_block_with_permuted_txs(
    txs: Vec<SignedTransaction>,
    height: std::num::NonZeroU64,
    prev_block_hash: Option<HashOf<BlockHeader>>,
    leader: &KeyPair,
    proof_policy_bundle: &iroha_data_model::da::commitment::DaProofPolicyBundle,
) -> SignedBlock {
    // Build header with creation time just after max tx creation_time
    let ct_ms = txs
        .iter()
        .map(|tx| u64::try_from(tx.creation_time().as_millis()).unwrap_or(0))
        .max()
        .unwrap_or(0);
    let header = BlockHeader::new(height, prev_block_hash, None, None, ct_ms + 1, 0);
    let mut builder = BlockBuilder::new(header);
    builder.set_da_proof_policies(Some(proof_policy_bundle.clone()));
    for tx in txs {
        builder.push_transaction(tx);
    }
    builder.build_with_signature(0, leader.private_key())
}

fn seed_genesis_block(state: &State) -> HashOf<BlockHeader> {
    if let Some(hash) = state.view().latest_block_hash() {
        return hash;
    }

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let genesis = BlockBuilder::new(header).build_with_signature(0, leader.private_key());
    let genesis_hash = genesis.hash();
    let mut state_block = state.block(genesis.header());
    let peer = PeerId::from(leader.public_key().clone());
    let valid = ValidBlock::validate_unchecked(genesis, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let _ = state_block.apply_without_execution(&committed, vec![peer]);
    state_block
        .kura()
        .store_block(Arc::new(committed.clone().into()))
        .expect("store genesis block");
    state_block.commit().expect("genesis commit must succeed");
    genesis_hash
}

fn run_validate(
    state: &mut iroha_core::state::State,
    block: SignedBlock,
    chain: &ChainId,
    authority: &AccountId,
    leader: &KeyPair,
) -> Result<ValidBlock, Box<iroha_core::block::BlockValidationError>> {
    let peer = PeerId::from(leader.public_key().clone());
    let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);
    let header = block.header();
    ValidBlock::validate(
        block,
        &topology,
        chain,
        authority,
        &iroha_primitives::time::TimeSource::new_system(),
        &mut state.block(header),
    )
    .unpack(|_| {})
    .map_err(|(_, e)| Box::new(*e))
}

#[test]
fn ed25519_batch_permutation_finds_same_bad_sig() {
    let (mut state, authority, chain, good) = setup_world_with_account(Algorithm::Ed25519);
    enable_batch_caps(&mut state);
    let bad = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let genesis_hash = seed_genesis_block(&state);
    let height = nonzero!(2_u64);
    let proof_policy_bundle =
        iroha_core::da::proof_policy_bundle(&state.view().nexus().lane_config);

    // Build a few transactions where exactly one is signed by a wrong key
    let mk = |msg: &str, mismatched_sig: bool| {
        let mut tx = TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, msg.to_string())])
            .sign(good.private_key());
        if mismatched_sig {
            let sig = TransactionSignature(SignatureOf::new(bad.private_key(), tx.payload()));
            tx.set_signature(sig);
        }
        tx
    };
    let tx_ok1 = mk("ok-1", false);
    let tx_ok2 = mk("ok-2", false);
    let tx_bad = mk("bad", true);
    let tx_ok3 = mk("ok-3", false);
    let tx_ok4 = mk("ok-4", false);
    let baseline = vec![tx_ok1, tx_ok2, tx_bad.clone(), tx_ok3, tx_ok4];
    let bad_sig = tx_bad.signature().clone();

    // Deterministic permutations
    let mut rng = Lcg::new(0xED_25_51_9D);
    for _ in 0..32 {
        let perm = shuffle(&mut rng, &baseline);
        let block = mk_block_with_permuted_txs(
            perm,
            height,
            Some(genesis_hash),
            &leader,
            &proof_policy_bundle,
        );
        let err = run_validate(&mut state, block, &chain, &authority, &leader)
            .expect_err("block must be rejected due to bad signature");

        match *err {
            BErr::TransactionAccept(AF::SignatureVerification(fail)) => {
                assert_eq!(
                    fail.signature, bad_sig,
                    "offending signature must match the known bad tx signature"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}

#[test]
fn secp256k1_batch_permutation_finds_same_bad_sig() {
    let (mut state, authority, chain, good) = setup_world_with_account(Algorithm::Secp256k1);
    enable_batch_caps(&mut state);
    let bad = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
    let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let genesis_hash = seed_genesis_block(&state);
    let height = nonzero!(2_u64);
    let proof_policy_bundle =
        iroha_core::da::proof_policy_bundle(&state.view().nexus().lane_config);

    let mk = |msg: &str, mismatched_sig: bool| {
        let mut tx = TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, msg.to_string())])
            .sign(good.private_key());
        if mismatched_sig {
            let sig = TransactionSignature(SignatureOf::new(bad.private_key(), tx.payload()));
            tx.set_signature(sig);
        }
        tx
    };
    let tx_ok1 = mk("ok-1", false);
    let tx_ok2 = mk("ok-2", false);
    let tx_bad = mk("bad", true);
    let tx_ok3 = mk("ok-3", false);
    let tx_ok4 = mk("ok-4", false);
    let baseline = vec![tx_ok1, tx_ok2, tx_bad.clone(), tx_ok3, tx_ok4];
    let bad_sig = tx_bad.signature().clone();

    let mut rng = Lcg::new(0x53_45_43_50);
    for _ in 0..32 {
        let perm = shuffle(&mut rng, &baseline);
        let block = mk_block_with_permuted_txs(
            perm,
            height,
            Some(genesis_hash),
            &leader,
            &proof_policy_bundle,
        );
        let err = run_validate(&mut state, block, &chain, &authority, &leader)
            .expect_err("block must be rejected due to bad signature");
        use iroha_core::{block::BlockValidationError as BErr, tx::AcceptTransactionFail as AF};
        match *err {
            BErr::TransactionAccept(AF::SignatureVerification(fail)) => {
                assert_eq!(
                    fail.signature, bad_sig,
                    "offending signature must match the known bad tx signature"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}

#[test]
#[cfg(feature = "bls")]
fn bls_multimessage_batch_passes() {
    let (mut state, authority, chain, signer) = setup_world_with_account(Algorithm::BlsNormal);
    enable_batch_caps(&mut state);
    let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let genesis_hash = seed_genesis_block(&state);
    let height = nonzero!(2_u64);
    let proof_policy_bundle =
        iroha_core::da::proof_policy_bundle(&state.view().nexus().lane_config);

    let mk = |msg: &str| {
        TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, msg.to_string())])
            .sign(signer.private_key())
    };
    let txs = vec![mk("m1"), mk("m2"), mk("m3"), mk("m4"), mk("m5")];

    let block = mk_block_with_permuted_txs(
        txs,
        height,
        Some(genesis_hash),
        &leader,
        &proof_policy_bundle,
    );
    run_validate(&mut state, block, &chain, &authority, &leader)
        .expect("valid BLS multi-message batch must pass");
}

#[test]
#[cfg(feature = "bls")]
fn bls_multimessage_batch_finds_same_bad_sig() {
    let (mut state, authority, chain, good) = setup_world_with_account(Algorithm::BlsNormal);
    enable_batch_caps(&mut state);
    let bad = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let genesis_hash = seed_genesis_block(&state);
    let height = nonzero!(2_u64);
    let proof_policy_bundle =
        iroha_core::da::proof_policy_bundle(&state.view().nexus().lane_config);

    let mk = |msg: &str, mismatched_sig: bool| {
        let mut tx = TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, msg.to_string())])
            .sign(good.private_key());
        if mismatched_sig {
            let sig = TransactionSignature(SignatureOf::new(bad.private_key(), tx.payload()));
            tx.set_signature(sig);
        }
        tx
    };
    let tx_ok1 = mk("ok-1", false);
    let tx_ok2 = mk("ok-2", false);
    let tx_bad = mk("bad", true);
    let tx_ok3 = mk("ok-3", false);
    let tx_ok4 = mk("ok-4", false);
    let baseline = vec![tx_ok1, tx_ok2, tx_bad.clone(), tx_ok3, tx_ok4];
    let bad_sig = tx_bad.signature().clone();

    let mut rng = Lcg::new(0xB150_0BAD);
    for _ in 0..32 {
        let perm = shuffle(&mut rng, &baseline);
        let block = mk_block_with_permuted_txs(
            perm,
            height,
            Some(genesis_hash),
            &leader,
            &proof_policy_bundle,
        );
        let err = run_validate(&mut state, block, &chain, &authority, &leader)
            .expect_err("block must be rejected due to bad BLS signature");

        match *err {
            BErr::TransactionAccept(AF::SignatureVerification(fail)) => {
                assert_eq!(
                    fail.signature, bad_sig,
                    "offending signature must match the known bad tx signature"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}

#[cfg(feature = "bls")]
#[test]
fn bls_batch_permutation_finds_same_bad_sig() {
    let (mut state, authority, chain, good) = setup_world_with_account(Algorithm::BlsNormal);
    enable_batch_caps(&mut state);
    let bad = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let genesis_hash = seed_genesis_block(&state);
    let height = nonzero!(2_u64);
    let proof_policy_bundle =
        iroha_core::da::proof_policy_bundle(&state.view().nexus().lane_config);

    // Use distinct messages to exercise multi-message aggregation path
    let mk = |msg: &str, mismatched_sig: bool| {
        let mut tx = TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, msg.to_string())])
            .sign(good.private_key());
        if mismatched_sig {
            let sig = TransactionSignature(SignatureOf::new(bad.private_key(), tx.payload()));
            tx.set_signature(sig);
        }
        tx
    };
    let tx_ok1 = mk("m1", false);
    let tx_ok2 = mk("m2", false);
    let tx_bad = mk("m3-bad", true);
    let tx_ok3 = mk("m4", false);
    let tx_ok4 = mk("m5", false);
    let baseline = vec![tx_ok1, tx_ok2, tx_bad.clone(), tx_ok3, tx_ok4];
    let bad_sig = tx_bad.signature().clone();

    let mut rng = Lcg::new(0xB1_5B_4D);
    for _ in 0..16 {
        let perm = shuffle(&mut rng, &baseline);
        let block = mk_block_with_permuted_txs(
            perm,
            height,
            Some(genesis_hash),
            &leader,
            &proof_policy_bundle,
        );
        let err = run_validate(&mut state, block, &chain, &authority, &leader)
            .expect_err("block must be rejected due to bad signature");
        use iroha_core::{block::BlockValidationError as BErr, tx::AcceptTransactionFail as AF};
        match *err {
            BErr::TransactionAccept(AF::SignatureVerification(fail)) => {
                assert_eq!(
                    fail.signature, bad_sig,
                    "offending signature must match the known bad tx signature"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
