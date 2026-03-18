//! Tests for `ZK_ROOTS_GET` respecting request max and configured cap.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]

use iroha_config::parameters::{actual as cfg, defaults};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::ivm::host::CoreHost,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::KeyPair;
use iroha_data_model::{account::NewAccount, prelude::*};
use ivm::{IVMHost, Memory, PointerType, syscalls, zk_verify};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

#[test]
fn zk_roots_get_respects_cap_and_max() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: zk roots get cap test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    // Build state with small roots cap
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
        root_history_cap: 4,
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

    // Seed world: domain/account/asset and mint
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let domain_id: DomainId = "zkd".parse().unwrap();
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "zkd".parse().unwrap(),
        "zcoin".parse().unwrap(),
    );
    let owner = AccountId::new(KeyPair::random().public_key().clone());
    for instr in [
        Register::domain(Domain::new(domain_id.clone())).into(),
        Register::account(NewAccount::new_in_domain(owner.clone(), domain_id.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        )
        .into(),
        Mint::asset_numeric(10_000u64, AssetId::of(asset_def_id.clone(), owner.clone())).into(),
        // Register zk policy
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
            .execute_instruction(&mut stx, &owner, instr)
            .unwrap();
    }
    // Multiple shields to exceed cap
    for _ in 0..16 {
        let ib: InstructionBox = iroha_data_model::isi::zk::Shield::new(
            asset_def_id.clone(),
            owner.clone(),
            100u128,
            [7u8; 32],
            iroha_data_model::confidential::ConfidentialEncryptedPayload::default(),
        )
        .into();
        stx.world
            .executor()
            .clone()
            .execute_instruction(&mut stx, &owner, ib)
            .unwrap();
    }
    stx.apply();

    // Build CoreHost and snapshot roots
    let mut vm = ivm::IVM::new(1_000_000);
    let mut host = CoreHost::new(owner.clone());
    host.set_zk_root_history_cap(state.view().zk.root_history_cap);
    // Snapshot
    {
        use std::collections::BTreeMap;
        let mut snap: BTreeMap<AssetDefinitionId, Vec<[u8; 32]>> = BTreeMap::new();
        for (ad, st) in state.view().world.zk_assets().iter() {
            snap.insert(ad.clone(), st.root_history.clone());
        }
        host.set_zk_roots_snapshot(snap);
    }
    let mut host = host;

    // Case 1: max=0 => bounded by cap
    let req = zk_verify::RootsGetRequest {
        asset_id: asset_def_id.to_string(),
        max: 0,
    };
    let payload = norito::to_bytes(&req).expect("encode");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    host.syscall(syscalls::SYSCALL_ZK_ROOTS_GET, &mut vm)
        .expect("syscall ok");
    let resp_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(resp_ptr).expect("valid tlv");
    let resp: zk_verify::RootsGetResponse =
        norito::decode_from_bytes(tlv.payload).expect("decode response");
    assert!(resp.roots.len() <= 4);
    assert_eq!(resp.roots.len(), 4);

    // Case 2: max=2 => bounded by 2
    let req = zk_verify::RootsGetRequest {
        asset_id: asset_def_id.to_string(),
        max: 2,
    };
    let payload = norito::to_bytes(&req).expect("encode");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
    vm.memory.preload_input(128, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START + 128);
    host.syscall(syscalls::SYSCALL_ZK_ROOTS_GET, &mut vm)
        .expect("syscall ok");
    let resp_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(resp_ptr).expect("valid tlv");
    let resp: zk_verify::RootsGetResponse =
        norito::decode_from_bytes(tlv.payload).expect("decode response");
    assert_eq!(resp.roots.len(), 2);
}
