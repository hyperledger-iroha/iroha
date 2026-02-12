//! Admission-time deterministic batching tests for signature schemes.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::cast_possible_truncation)]
//! These exercise grouping + bisection in `ValidBlock::validate_static` using
//! one intentionally bad signature per batch.

use core::time::Duration;
use std::time::SystemTime;

use iroha_core::{
    block::ValidBlock,
    prelude::*,
    state::State,
    tx::{AcceptTransactionFail, SignatureRejectionCode},
};
use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
use iroha_data_model::prelude::*;
use nonzero_ext::nonzero;

fn setup_world_with_account(algo: Algorithm) -> (State, AccountId, ChainId, KeyPair) {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore};

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let kp = KeyPair::random_with_algorithm(algo);
    let (pubkey, _) = kp.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(domain_id.clone(), pubkey);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    let state = State::new_for_testing(world, kura, query_handle);
    let mut crypto_cfg = iroha_config::parameters::actual::Crypto::default();
    if !crypto_cfg.allowed_signing.contains(&algo) {
        crypto_cfg.allowed_signing.push(algo);
        crypto_cfg.allowed_signing.sort();
        crypto_cfg.allowed_signing.dedup();
    }
    #[cfg(feature = "sm")]
    if matches!(algo, Algorithm::Sm2) {}
    state.set_crypto(crypto_cfg);
    (state, account_id, ChainId::from("chain"), kp)
}

fn set_default_da_policy_hash(header: &mut BlockHeader) {
    let lane_cfg = iroha_config::parameters::actual::LaneConfig::default();
    let hash = iroha_core::da::proof_policy_bundle_hash(&lane_cfg);
    header.set_da_proof_policies_hash(Some(hash));
}

fn build_block_with_txs(
    good_kp: &KeyPair,
    bad_kp: &KeyPair,
    leader_kp: &KeyPair,
    authority: &AccountId,
    chain: &ChainId,
) -> SignedBlock {
    // Build a few transactions where exactly one is signed by a wrong key
    let mk = |msg: &str, kp: &KeyPair| {
        TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, msg.to_string())])
            .sign(kp.private_key())
    };
    let txs = vec![
        mk("ok-1", good_kp),
        mk("ok-2", good_kp),
        mk("bad", bad_kp), // wrong key for same authority
        mk("ok-3", good_kp),
    ];
    // Create header and sign block
    let ct_ms = txs
        .iter()
        .map(|tx| tx.creation_time().as_millis() as u64)
        .max()
        .unwrap_or(0);
    let mut header = BlockHeader::new(nonzero!(1_u64), None, None, None, ct_ms + 1, 0);
    set_default_da_policy_hash(&mut header);
    let leader_sk = leader_kp.private_key();
    let sig = BlockSignature::new(0, SignatureOf::from_hash(leader_sk, header.hash()));
    SignedBlock::presigned(sig, header, txs)
}

#[cfg(feature = "bls")]
fn mk_tx_with_creation_time(
    chain: &ChainId,
    authority: &AccountId,
    msg: &str,
    ct_ms: u64,
    sk: &iroha_crypto::PrivateKey,
) -> SignedTransaction {
    use core::time::Duration;
    let mut b = TransactionBuilder::new(chain.clone(), authority.clone());
    b.set_creation_time(Duration::from_millis(ct_ms));
    b.with_instructions([Log::new(Level::INFO, msg.to_string())])
        .sign(sk)
}

#[cfg(feature = "bls")]
fn presigned_block_with_creation_after_txs(
    leader: &KeyPair,
    txs: Vec<SignedTransaction>,
) -> SignedBlock {
    fn creation_ms(tx: &SignedTransaction) -> u64 {
        let raw = tx.creation_time().as_millis();
        if raw > u128::from(u64::MAX) {
            u64::MAX
        } else {
            raw as u64
        }
    }

    let block_ct = txs
        .iter()
        .map(creation_ms)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let merkle_root = {
        let hashes = txs.iter().map(SignedTransaction::hash_as_entrypoint);
        let tree: iroha_crypto::MerkleTree<TransactionEntrypoint> = hashes.collect();
        tree.root()
    };
    let mut header = BlockHeader::new(nonzero!(1_u64), None, None, None, block_ct, 0);
    header.merkle_root = merkle_root;
    set_default_da_policy_hash(&mut header);
    let sig = BlockSignature::new(
        0,
        SignatureOf::from_hash(leader.private_key(), header.hash()),
    );
    SignedBlock::presigned(sig, header, txs)
}

#[cfg(feature = "bls")]
fn enable_bls_batching(state: &mut iroha_core::state::State) {
    let cfg = iroha_config::parameters::actual::Pipeline {
        ivm_proved: iroha_config::parameters::actual::IvmProvedExecution {
            enabled: iroha_config::parameters::defaults::pipeline::ivm_proved::ENABLED,
            allowed_circuits: Vec::new(),
        },
        dynamic_prepass: iroha_config::parameters::defaults::pipeline::DYNAMIC_PREPASS,
        access_set_cache_enabled:
            iroha_config::parameters::defaults::pipeline::ACCESS_SET_CACHE_ENABLED,
        parallel_overlay: iroha_config::parameters::defaults::pipeline::PARALLEL_OVERLAY,
        workers: iroha_config::parameters::defaults::pipeline::WORKERS,
        stateless_cache_cap: iroha_config::parameters::defaults::pipeline::STATELESS_CACHE_CAP,
        parallel_apply: iroha_config::parameters::defaults::pipeline::PARALLEL_APPLY,
        ready_queue_heap: iroha_config::parameters::defaults::pipeline::READY_QUEUE_HEAP,
        gpu_key_bucket: iroha_config::parameters::defaults::pipeline::GPU_KEY_BUCKET,
        debug_trace_scheduler_inputs:
            iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_SCHEDULER_INPUTS,
        debug_trace_tx_eval: iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_TX_EVAL,
        signature_batch_max: iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX,
        signature_batch_max_ed25519:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_ED25519,
        signature_batch_max_secp256k1:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_SECP256K1,
        signature_batch_max_pqc:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_PQC,
        signature_batch_max_bls: 16, // enable BLS micro-batching
        cache_size: iroha_config::parameters::defaults::pipeline::CACHE_SIZE,
        ivm_cache_max_decoded_ops:
            iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_DECODED_OPS,
        ivm_cache_max_bytes: iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_BYTES,
        ivm_prover_threads: iroha_config::parameters::defaults::pipeline::IVM_PROVER_THREADS,
        overlay_max_instructions:
            iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_INSTRUCTIONS,
        overlay_max_bytes: iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_BYTES,
        overlay_chunk_instructions:
            iroha_config::parameters::defaults::pipeline::OVERLAY_CHUNK_INSTRUCTIONS,
        gas: iroha_config::parameters::actual::Gas {
            tech_account_id: iroha_config::parameters::defaults::pipeline::GAS_TECH_ACCOUNT_ID
                .to_string(),
            accepted_assets: Vec::new(),
            units_per_gas: Vec::new(),
        },
        ivm_max_cycles_upper_bound:
            iroha_config::parameters::defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND,
        ivm_max_decoded_instructions:
            iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_INSTRUCTIONS,
        ivm_max_decoded_bytes: iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_BYTES,
        quarantine_max_txs_per_block:
            iroha_config::parameters::defaults::pipeline::QUARANTINE_MAX_TXS_PER_BLOCK,
        quarantine_tx_max_cycles:
            iroha_config::parameters::defaults::pipeline::QUARANTINE_TX_MAX_CYCLES,
        quarantine_tx_max_millis:
            iroha_config::parameters::defaults::pipeline::QUARANTINE_TX_MAX_MILLIS,
        query_default_cursor_mode: iroha_config::parameters::actual::QueryCursorMode::Ephemeral,
        query_max_fetch_size: iroha_config::parameters::defaults::pipeline::QUERY_MAX_FETCH_SIZE,
        query_stored_min_gas_units:
            iroha_config::parameters::defaults::pipeline::QUERY_STORED_MIN_GAS_UNITS,
        amx_per_dataspace_budget_ms:
            iroha_config::parameters::defaults::pipeline::AMX_PER_DATASPACE_BUDGET_MS,
        amx_group_budget_ms: iroha_config::parameters::defaults::pipeline::AMX_GROUP_BUDGET_MS,
        amx_per_instruction_ns:
            iroha_config::parameters::defaults::pipeline::AMX_PER_INSTRUCTION_NS,
        amx_per_memory_access_ns:
            iroha_config::parameters::defaults::pipeline::AMX_PER_MEMORY_ACCESS_NS,
        amx_per_syscall_ns: iroha_config::parameters::defaults::pipeline::AMX_PER_SYSCALL_NS,
    };
    state.set_pipeline(cfg);
}

/// BLS same-message group duplicate payloads: constructing two identical transactions
/// is rejected by admission (duplicate payload). This still exercises same-message
/// grouping path in signature preverification before the duplicate check.
#[cfg(feature = "bls")]
#[test]
fn bls_same_message_group_duplicate_rejected() {
    // World and keys
    let (mut state, authority, chain, good) = setup_world_with_account(Algorithm::BlsNormal);
    enable_bls_batching(&mut state);
    let leader = good.clone();

    // Two transactions with identical payloads (same creation_time and instructions), both signed correctly
    let ct = 1_651_234_567u64;
    let tx1 = mk_tx_with_creation_time(&chain, &authority, "same", ct, good.private_key());
    let tx2 = mk_tx_with_creation_time(&chain, &authority, "same", ct, good.private_key());

    let ct_ms = u64::max(
        tx1.creation_time().as_millis() as u64,
        tx2.creation_time().as_millis() as u64,
    );
    let mut header = BlockHeader::new(nonzero!(1_u64), None, None, None, ct_ms + 1, 0);
    set_default_da_policy_hash(&mut header);
    let sig = BlockSignature::new(
        0,
        SignatureOf::from_hash(leader.private_key(), header.hash()),
    );
    let block = SignedBlock::presigned(sig, header, vec![tx1, tx2]);
    let peer = PeerId::from(leader.public_key().clone());
    let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);

    let result = ValidBlock::validate(
        block,
        &topology,
        &chain,
        &authority,
        &iroha_primitives::time::TimeSource::new_system(),
        &mut state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)),
    )
    .unpack(|_| {});
    assert!(
        result.is_err(),
        "duplicate identical BLS transactions should be rejected"
    );
    #[cfg(feature = "telemetry")]
    {
        let (same, multi, det) = state.view().metrics().pipeline_sig_bls_counts();
        assert!(same >= 1, "expected same-message aggregate to be used");
        assert_eq!(multi, 0, "unexpected multi-message aggregate count");
        assert_eq!(det, 0, "unexpected deterministic count");
    }
}

/// BLS mixed micro-batch: includes a same-message group (two identical valid txs)
/// and two distinct singletons. Signature preverification should run the fast-aggregate
/// for the group and the multi-message aggregate for the singletons. The overall
/// block is rejected due to duplicate payloads.
#[cfg(feature = "bls")]
#[test]
fn bls_mixed_group_and_singletons_duplicate_rejected() {
    // World and keys
    let (mut state, authority, chain, good) = setup_world_with_account(Algorithm::BlsNormal);
    enable_bls_batching(&mut state);
    let leader = good.clone();

    // Two identical valid transactions (same payload) form the same-message group
    let ct = 1_651_234_567u64;
    let tx_same1 = mk_tx_with_creation_time(&chain, &authority, "same", ct, good.private_key());
    let tx_same2 = mk_tx_with_creation_time(&chain, &authority, "same", ct, good.private_key());

    // Two distinct valid transactions (different payloads) serve as singletons
    let tx_s1 = mk_tx_with_creation_time(&chain, &authority, "s1", ct + 1, good.private_key());
    let tx_s2 = mk_tx_with_creation_time(&chain, &authority, "s2", ct + 2, good.private_key());

    let block =
        presigned_block_with_creation_after_txs(&leader, vec![tx_same1, tx_same2, tx_s1, tx_s2]);
    let peer = PeerId::from(leader.public_key().clone());
    let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);

    let result = ValidBlock::validate(
        block,
        &topology,
        &chain,
        &authority,
        &iroha_primitives::time::TimeSource::new_system(),
        &mut state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)),
    )
    .unpack(|_| {});
    assert!(
        result.is_err(),
        "block with duplicate BLS txs should be rejected, got {result:?}"
    );
    #[cfg(feature = "telemetry")]
    {
        let (same, multi, det) = state.view().metrics().pipeline_sig_bls_counts();
        assert!(same >= 1, "expected same-message aggregate to be used");
        assert!(multi >= 1, "expected multi-message aggregate to be used");
        assert_eq!(det, 0, "unexpected deterministic count");
    }
}

/// BLS same-message group bisection: one of two identical payloads is signed by a wrong key.
#[cfg(feature = "bls")]
#[test]
fn bls_same_message_group_bisect_bad() {
    let (mut state, authority, chain, good) = setup_world_with_account(Algorithm::BlsNormal);
    enable_bls_batching(&mut state);
    let bad = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let leader = good.clone();

    let ct = 1_651_234_567u64;
    let tx_good = mk_tx_with_creation_time(&chain, &authority, "same", ct, good.private_key());
    // Same payload but signed with a key that does not match the authority
    let tx_bad = mk_tx_with_creation_time(&chain, &authority, "same", ct, bad.private_key());

    let block = presigned_block_with_creation_after_txs(&leader, vec![tx_good, tx_bad]);
    let peer = PeerId::from(leader.public_key().clone());
    let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);

    let result = ValidBlock::validate(
        block,
        &topology,
        &chain,
        &authority,
        &iroha_primitives::time::TimeSource::new_system(),
        &mut state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)),
    )
    .unpack(|_| {});
    assert!(
        result.is_err(),
        "block must be rejected when one same-message sig is invalid"
    );
    #[cfg(feature = "telemetry")]
    {
        let (same, multi, det) = state.view().metrics().pipeline_sig_bls_counts();
        assert!(
            same >= 1,
            "expected same-message aggregate attempt to be recorded"
        );
        assert_eq!(multi, 0, "unexpected multi-message aggregate count");
        assert_eq!(det, 0, "unexpected deterministic count");
    }
}

/// BLS multi-message aggregate across distinct messages: two valid BLS txs with
/// different payloads should be accepted when batching is enabled.
#[cfg(feature = "bls")]
#[test]
fn bls_multi_message_aggregate_ok() {
    let (mut state, authority, chain, good) = setup_world_with_account(Algorithm::BlsNormal);
    enable_bls_batching(&mut state);
    let leader = good.clone();

    // Two distinct messages with current creation time (avoid TTL rejection)
    let tx1 = TransactionBuilder::new(chain.clone(), authority.clone())
        .with_instructions([Log::new(Level::INFO, "m1".to_string())])
        .sign(good.private_key());
    let tx2 = TransactionBuilder::new(chain.clone(), authority.clone())
        .with_instructions([Log::new(Level::INFO, "m2".to_string())])
        .sign(good.private_key());

    let block = presigned_block_with_creation_after_txs(&leader, vec![tx1, tx2]);
    let peer = PeerId::from(leader.public_key().clone());
    let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);

    let result = ValidBlock::validate(
        block,
        &topology,
        &chain,
        &authority,
        &iroha_primitives::time::TimeSource::new_system(),
        &mut state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)),
    )
    .unpack(|_| {});
    assert!(
        result.is_ok(),
        "block with distinct valid BLS txs must be accepted, got {result:?}"
    );
    #[cfg(feature = "telemetry")]
    {
        let (same, multi, det) = state.view().metrics().pipeline_sig_bls_counts();
        assert_eq!(same, 0, "unexpected same-message aggregate count");
        assert!(multi >= 1, "expected multi-message aggregate to be used");
        assert_eq!(det, 0, "unexpected deterministic count");
    }

    #[cfg(feature = "sm")]
    #[test]
    fn sm2_transactions_rejected_when_sm_disabled() {
        use iroha_crypto::KeyPair;

        let (mut state, authority, chain, signer) = setup_world_with_account(Algorithm::Sm2);
        let leader = KeyPair::random();
        let block = build_block_with_txs(&signer, &signer, &leader, &authority, &chain);
        let peer = PeerId::from(leader.public_key().clone());
        let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);

        let result = ValidBlock::validate(
            block,
            &topology,
            &chain,
            &authority,
            &iroha_primitives::time::TimeSource::new_system(),
            &mut state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)),
        )
        .unpack(|_| {});
        assert!(
            result.is_err(),
            "SM2 block should be rejected when SM is disabled"
        );
    }

    #[cfg(feature = "sm")]
    #[test]
    fn sm2_transactions_accepted() {
        use iroha_crypto::KeyPair;

        let (mut state, authority, chain, signer) = setup_world_with_account(Algorithm::Sm2);
        let leader = KeyPair::random();
        let block = build_block_with_txs(&signer, &signer, &leader, &authority, &chain);
        let peer = PeerId::from(leader.public_key().clone());
        let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);

        let result = ValidBlock::validate(
            block,
            &topology,
            &chain,
            &authority,
            &iroha_primitives::time::TimeSource::new_system(),
            &mut state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)),
        )
        .unpack(|_| {});
        assert!(
            result.is_ok(),
            "SM2 block should be accepted when SM is enabled"
        );
    }
}

/// BLS multi-message aggregate bisection: one of two distinct payloads is signed by a wrong key
/// and should trigger a multi-message aggregate failure plus telemetry bookkeeping.
#[cfg(feature = "bls")]
#[test]
fn bls_multi_message_aggregate_fails_and_counts() {
    let (mut state, authority, chain, good) = setup_world_with_account(Algorithm::BlsNormal);
    enable_bls_batching(&mut state);
    let bad = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let leader = good.clone();

    let ct = 1_751_234_567u64;
    let tx_valid = mk_tx_with_creation_time(&chain, &authority, "m1", ct, good.private_key());
    let tx_bad = mk_tx_with_creation_time(&chain, &authority, "m2", ct + 1, bad.private_key());

    let block = presigned_block_with_creation_after_txs(&leader, vec![tx_valid, tx_bad]);
    let peer = PeerId::from(leader.public_key().clone());
    let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);

    let result = ValidBlock::validate(
        block,
        &topology,
        &chain,
        &authority,
        &iroha_primitives::time::TimeSource::new_system(),
        &mut state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)),
    )
    .unpack(|_| {});
    assert!(
        result.is_err(),
        "block must be rejected when multi-message aggregate contains a bad signature"
    );

    #[cfg(feature = "telemetry")]
    {
        let ((same_success, same_failure), (multi_success, multi_failure)) =
            state.view().metrics().pipeline_sig_bls_result_totals();
        assert_eq!(
            same_success + same_failure,
            0,
            "same-message aggregates should not be exercised in this scenario"
        );
        assert_eq!(
            multi_success, 0,
            "no multi-message aggregate should succeed"
        );
        assert!(
            multi_failure >= 1,
            "multi-message aggregate failures should be counted"
        );
    }
}

#[cfg(feature = "bls")]
#[test]
fn bls_batch_bisection_finds_bad_sig() {
    // Normal BLS variant
    let (state, authority, chain, good) = setup_world_with_account(Algorithm::BlsNormal);
    let bad = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let leader = good.clone();
    let block = build_block_with_txs(&good, &bad, &leader, &authority, &chain);
    let peer = PeerId::from(leader.public_key().clone());
    let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);

    // Validate statically; expect rejection due to bad signature
    let result = ValidBlock::validate(
        block,
        &topology,
        &chain,
        &authority,
        &iroha_primitives::time::TimeSource::new_system(),
        &mut state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)),
    )
    .unpack(|_| {});
    assert!(
        result.is_err(),
        "block with one bad BLS signature must be rejected"
    );
}

#[test]
fn mldsa_batch_bisection_finds_bad_sig() {
    // ML‑DSA (Dilithium3)
    let (state, authority, chain, good) = setup_world_with_account(Algorithm::MlDsa);
    let bad = KeyPair::random_with_algorithm(Algorithm::MlDsa);
    let leader = KeyPair::random();
    let block = build_block_with_txs(&good, &bad, &leader, &authority, &chain);
    let peer = PeerId::from(leader.public_key().clone());
    let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);

    let result = ValidBlock::validate(
        block,
        &topology,
        &chain,
        &authority,
        &iroha_primitives::time::TimeSource::new_system(),
        &mut state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)),
    )
    .unpack(|_| {});
    assert!(
        result.is_err(),
        "block with one bad ML‑DSA signature must be rejected"
    );
}

#[test]
fn ed25519_batch_bisection_finds_bad_sig() {
    let (state, authority, chain, good) = setup_world_with_account(Algorithm::Ed25519);
    let bad = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let leader = KeyPair::random();
    let block = build_block_with_txs(&good, &bad, &leader, &authority, &chain);
    let peer = PeerId::from(leader.public_key().clone());
    let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);

    let result = ValidBlock::validate(
        block,
        &topology,
        &chain,
        &authority,
        &iroha_primitives::time::TimeSource::new_system(),
        &mut state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)),
    )
    .unpack(|_| {});
    assert!(
        result.is_err(),
        "block with one bad Ed25519 signature must be rejected"
    );
}

#[test]
fn secp256k1_batch_bisection_finds_bad_sig() {
    let (state, authority, chain, good) = setup_world_with_account(Algorithm::Secp256k1);
    let bad = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
    let leader = KeyPair::random();
    let block = build_block_with_txs(&good, &bad, &leader, &authority, &chain);
    let peer = PeerId::from(leader.public_key().clone());
    let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![peer]);

    let result = ValidBlock::validate(
        block,
        &topology,
        &chain,
        &authority,
        &iroha_primitives::time::TimeSource::new_system(),
        &mut state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)),
    )
    .unpack(|_| {});
    assert!(
        result.is_err(),
        "block with one bad secp256k1 signature must be rejected"
    );
}

#[test]
fn rejects_transaction_signed_with_disallowed_algorithm() {
    let (state, authority, chain, _) = setup_world_with_account(Algorithm::Ed25519);
    let max_clock_drift = core::time::Duration::from_secs(0);
    let default_limits = TransactionParameters::default();
    let tx_limits = TransactionParameters::with_max_signatures(
        nonzero!(1_u64),
        nonzero!(16_u64),
        nonzero!(2048_u64),
        default_limits.max_tx_bytes(),
        default_limits.max_decompressed_bytes(),
        default_limits.max_metadata_depth(),
    );
    let secp = KeyPair::random_with_algorithm(Algorithm::Secp256k1);

    let tx = TransactionBuilder::new(chain.clone(), authority.clone())
        .with_instructions([Log::new(Level::INFO, "secp attempt".to_owned())])
        .sign(secp.private_key());

    let mut crypto_cfg = (*state.crypto()).clone();
    crypto_cfg
        .allowed_signing
        .retain(|algo| *algo == Algorithm::Ed25519);
    crypto_cfg.allowed_curve_ids =
        iroha_config::parameters::defaults::crypto::derive_curve_ids_from_algorithms(
            &crypto_cfg.allowed_signing,
        );
    state.set_crypto(crypto_cfg.clone());
    match AcceptedTransaction::accept(tx, &chain, max_clock_drift, tx_limits, &crypto_cfg) {
        Err(AcceptTransactionFail::SignatureVerification(fail)) => {
            assert_eq!(fail.code(), SignatureRejectionCode::AlgorithmNotPermitted);
            assert!(
                fail.detail.contains("not permitted"),
                "expected rejection due to disallowed algorithm, got {:?}",
                fail.detail
            );
        }
        other => panic!("expected SignatureVerification failure, got {other:?}"),
    }
}

#[test]
fn accepts_transaction_once_algorithm_whitelisted() {
    let (state, authority, chain, _) = setup_world_with_account(Algorithm::Ed25519);
    let max_clock_drift = Duration::from_secs(0);
    let default_limits = TransactionParameters::default();
    let tx_limits = TransactionParameters::with_max_signatures(
        nonzero!(1_u64),
        nonzero!(16_u64),
        nonzero!(2048_u64),
        default_limits.max_tx_bytes(),
        default_limits.max_decompressed_bytes(),
        default_limits.max_metadata_depth(),
    );
    let secp = KeyPair::random_with_algorithm(Algorithm::Secp256k1);

    let tx = TransactionBuilder::new(chain.clone(), authority.clone())
        .with_instructions([Log::new(Level::INFO, "secp allowed".to_owned())])
        .sign(secp.private_key());

    let mut crypto_cfg = (*state.crypto()).clone();
    if !crypto_cfg.allowed_signing.contains(&Algorithm::Secp256k1) {
        crypto_cfg.allowed_signing.push(Algorithm::Secp256k1);
        crypto_cfg.allowed_signing.sort();
        crypto_cfg.allowed_signing.dedup();
    }
    state.set_crypto(crypto_cfg.clone());

    AcceptedTransaction::accept(tx, &chain, max_clock_drift, tx_limits, &crypto_cfg)
        .expect("transaction should be accepted after whitelisting");
}

#[test]
fn rejects_transaction_when_ttl_expired() {
    let (state, authority, chain, kp) = setup_world_with_account(Algorithm::Ed25519);
    let max_clock_drift = Duration::from_secs(0);
    let default_limits = TransactionParameters::default();
    let tx_limits = TransactionParameters::with_max_signatures(
        nonzero!(1_u64),
        nonzero!(16_u64),
        nonzero!(2048_u64),
        default_limits.max_tx_bytes(),
        default_limits.max_decompressed_bytes(),
        default_limits.max_metadata_depth(),
    );

    let mut builder = TransactionBuilder::new(chain.clone(), authority.clone());
    builder = builder.with_instructions([Log::new(Level::INFO, "expired ttl".to_owned())]);
    builder.set_creation_time(Duration::from_secs(0));
    builder.set_ttl(Duration::from_secs(1));
    let tx = builder.sign(kp.private_key());

    let crypto_cfg = (*state.crypto()).clone();
    match AcceptedTransaction::accept(tx, &chain, max_clock_drift, tx_limits, &crypto_cfg) {
        Err(AcceptTransactionFail::TransactionExpired { .. }) => {}
        other => panic!("expected TransactionExpired failure, got {other:?}"),
    }
}

#[test]
fn accepts_transaction_with_valid_ttl() {
    let (state, authority, chain, kp) = setup_world_with_account(Algorithm::Ed25519);
    let max_clock_drift = Duration::from_secs(5);
    let default_limits = TransactionParameters::default();
    let tx_limits = TransactionParameters::with_max_signatures(
        nonzero!(1_u64),
        nonzero!(16_u64),
        nonzero!(2048_u64),
        default_limits.max_tx_bytes(),
        default_limits.max_decompressed_bytes(),
        default_limits.max_metadata_depth(),
    );

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    let creation_time = now.saturating_sub(Duration::from_secs(1));

    let mut builder = TransactionBuilder::new(chain.clone(), authority.clone());
    builder = builder.with_instructions([Log::new(Level::INFO, "valid ttl".to_owned())]);
    builder.set_creation_time(creation_time);
    builder.set_ttl(Duration::from_secs(10));
    let tx = builder.sign(kp.private_key());

    let crypto_cfg = (*state.crypto()).clone();
    AcceptedTransaction::accept(tx, &chain, max_clock_drift, tx_limits, &crypto_cfg)
        .expect("transaction with unexpired TTL should be accepted");
}
