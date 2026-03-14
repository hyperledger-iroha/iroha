//! Enforce that provided `root_hint` values reference a recent Merkle root within
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! the configured window; reject stale/unknown roots.
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
    zk::test_utils::halo2_fixture_envelope,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{account::NewAccount, prelude::*};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

#[test]
fn unshield_rejects_stale_root_hint_and_accepts_recent() {
    use iroha_config::parameters::{actual as cfg, defaults};

    // Create state and cap recent roots to 3
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let mut state = State::new(
        World::new(),
        kura,
        query,
        iroha_core::telemetry::StateTelemetry::default(),
    );
    #[cfg(not(feature = "telemetry"))]
    let mut state = State::new(World::new(), kura, query);
    state.set_zk(cfg::Zk {
        halo2: cfg::Halo2 {
            enabled: defaults::zk::halo2::ENABLED,
            curve: cfg::ZkCurve::Pallas,
            backend: cfg::Halo2Backend::Ipa,
            max_k: defaults::zk::halo2::MAX_K,
            verifier_budget_ms: defaults::zk::halo2::VERIFIER_BUDGET_MS,
            verifier_max_batch: defaults::zk::halo2::VERIFIER_MAX_BATCH,
            max_envelope_bytes: defaults::zk::halo2::MAX_ENVELOPE_BYTES,
            max_proof_bytes: defaults::zk::halo2::MAX_PROOF_BYTES,
            max_transcript_label_len: defaults::zk::halo2::MAX_TRANSCRIPT_LABEL_LEN,
            enforce_transcript_label_ascii: defaults::zk::halo2::ENFORCE_TRANSCRIPT_LABEL_ASCII,
            ..cfg::Halo2::default()
        },
        fastpq: cfg::Fastpq {
            execution_mode: cfg::FastpqExecutionMode::Auto,
            poseidon_mode: cfg::FastpqPoseidonMode::Auto,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: defaults::zk::fastpq::METAL_TRACE,
            metal_debug_enum: defaults::zk::fastpq::METAL_DEBUG_ENUM,
            metal_debug_fused: defaults::zk::fastpq::METAL_DEBUG_FUSED,
        },
        stark: cfg::Stark::default(),
        root_history_cap: 3,
        ballot_history_cap: defaults::zk::vote::BALLOT_HISTORY_CAP,
        empty_root_on_empty: defaults::zk::ledger::EMPTY_ROOT_ON_EMPTY,
        merkle_depth: defaults::zk::ledger::EMPTY_ROOT_DEPTH,
        preverify_max_bytes: defaults::zk::preverify::MAX_BYTES,
        preverify_budget_bytes: defaults::zk::preverify::BUDGET_BYTES,
        proof_history_cap: defaults::zk::proof::RECORD_HISTORY_CAP,
        proof_retention_grace_blocks: defaults::zk::proof::RETENTION_GRACE_BLOCKS,
        proof_prune_batch: defaults::zk::proof::PRUNE_BATCH_SIZE,
        bridge_proof_max_range_len: defaults::zk::proof::BRIDGE_MAX_RANGE_LEN,
        bridge_proof_max_past_age_blocks: defaults::zk::proof::BRIDGE_MAX_PAST_AGE_BLOCKS,
        bridge_proof_max_future_drift_blocks: defaults::zk::proof::BRIDGE_MAX_FUTURE_DRIFT_BLOCKS,
        poseidon_params_id: defaults::confidential::POSEIDON_PARAMS_ID,
        pedersen_params_id: defaults::confidential::PEDERSEN_PARAMS_ID,
        kaigi_roster_join_vk: None,
        kaigi_roster_leave_vk: None,
        kaigi_usage_vk: None,
        max_proof_size_bytes: defaults::confidential::MAX_PROOF_SIZE_BYTES,
        max_nullifiers_per_tx: defaults::confidential::MAX_NULLIFIERS_PER_TX,
        max_commitments_per_tx: defaults::confidential::MAX_COMMITMENTS_PER_TX,
        max_confidential_ops_per_block: defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
        verify_timeout: defaults::confidential::VERIFY_TIMEOUT,
        max_anchor_age_blocks: defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
        max_proof_bytes_block: defaults::confidential::MAX_PROOF_BYTES_BLOCK,
        max_verify_calls_per_tx: defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
        max_verify_calls_per_block: defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
        max_public_inputs: defaults::confidential::MAX_PUBLIC_INPUTS,
        reorg_depth_bound: defaults::confidential::REORG_DEPTH_BOUND,
        policy_transition_delay_blocks: defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
        policy_transition_window_blocks: defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
        tree_roots_history_len: defaults::confidential::TREE_ROOTS_HISTORY_LEN,
        tree_frontier_checkpoint_interval:
            defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
        registry_max_vk_entries: defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
        registry_max_params_entries: defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
        registry_max_delta_per_block: defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
        gas: cfg::ConfidentialGas {
            proof_base: defaults::confidential::gas::PROOF_BASE,
            per_public_input: defaults::confidential::gas::PER_PUBLIC_INPUT,
            per_proof_byte: defaults::confidential::gas::PER_PROOF_BYTE,
            per_nullifier: defaults::confidential::gas::PER_NULLIFIER,
            per_commitment: defaults::confidential::gas::PER_COMMITMENT,
        },
    });

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let domain_id: DomainId = "zkd".parse().unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "zkd".parse().unwrap(),
        "rose".parse().unwrap(),
    );
    let alice = AccountId::new(KeyPair::random().public_key().clone());

    // Bootstrap domain/account/asset and mint, then enable ZK (Hybrid)
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new_in_domain(alice.clone(), domain_id.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
        Mint::asset_numeric(10_000u64, AssetId::of(asset_def_id.clone(), alice.clone())).into(),
        iroha_data_model::isi::zk::RegisterZkAsset::new(
            asset_def_id.clone(),
            iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
            true,
            true,
            None,
            None,
            None,
        )
        .into(),
    ] {
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &alice, instr)
            .unwrap();
    }

    // Produce several roots; remember a soon-to-be-stale root
    #[allow(unused_assignments)]
    let mut stale_root = [0u8; 32];
    for i in 0..3u8 {
        let mut note = [0u8; 32];
        note[0] = i;
        let ib: InstructionBox = iroha_data_model::isi::zk::Shield::new(
            asset_def_id.clone(),
            alice.clone(),
            1u128,
            note,
            iroha_data_model::confidential::ConfidentialEncryptedPayload::default(),
        )
        .into();
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &alice, ib)
            .unwrap();
    }
    // Observe current latest root and keep a copy as stale after more updates
    stale_root = *stx
        .world
        .zk_assets()
        .get(&asset_def_id)
        .unwrap()
        .root_history
        .last()
        .unwrap();
    // More shields to push the window forward and evict `stale_root`
    for i in 3..6u8 {
        let mut note = [0u8; 32];
        note[0] = i;
        let ib: InstructionBox = iroha_data_model::isi::zk::Shield::new(
            asset_def_id.clone(),
            alice.clone(),
            1u128,
            note,
            iroha_data_model::confidential::ConfidentialEncryptedPayload::default(),
        )
        .into();
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &alice, ib)
            .unwrap();
    }
    stx.apply();

    // Negative: Unshield with stale root_hint must be rejected
    let mut block2 = state.block(iroha_data_model::block::BlockHeader::new(
        nonzero!(2_u64),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut stx2 = block2.transaction();
    let bad_fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let bad_vk = bad_fixture
        .vk_box("halo2/ipa")
        .expect("fixture verifying key");
    let bad_unshield = iroha_data_model::isi::zk::Unshield::new(
        asset_def_id.clone(),
        alice.clone(),
        1u128,
        vec![[9u8; 32]],
        iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            bad_fixture.proof_box("halo2/ipa"),
            bad_vk,
        ),
        Some(stale_root),
    );
    let res =
        stx2.world
            .executor()
            .clone()
            .execute_instruction(&mut stx2, &alice, bad_unshield.into());
    assert!(res.is_err(), "stale root_hint must be rejected");

    // Positive: ZkTransfer with recent root_hint is accepted
    let mut block3 = state.block(iroha_data_model::block::BlockHeader::new(
        nonzero!(3_u64),
        None,
        None,
        None,
        0,
        0,
    ));
    let mut stx3 = block3.transaction();
    let current_root = {
        *stx3
            .world
            .zk_assets()
            .get(&asset_def_id)
            .unwrap()
            .root_history
            .last()
            .unwrap()
    };
    let ok_fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let ok_vk = ok_fixture
        .vk_box("halo2/ipa")
        .expect("fixture verifying key");
    let ok_transfer = iroha_data_model::isi::zk::ZkTransfer::new(
        asset_def_id.clone(),
        Vec::new(),
        vec![[3u8; 32]],
        iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ok_fixture.proof_box("halo2/ipa"),
            ok_vk,
        ),
        Some(current_root),
    );
    stx3.world
        .executor()
        .clone()
        .execute_instruction(&mut stx3, &alice, ok_transfer.into())
        .expect("recent root_hint accepted");
}
