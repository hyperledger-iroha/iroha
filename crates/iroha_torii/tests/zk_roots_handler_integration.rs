#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration test for /v1/zk/roots using a minimal in-memory state.
#![allow(clippy::too_many_lines)]

use std::{collections::HashSet, sync::Arc};

use axum::{Router, routing::post};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{NewAccount, prelude::*};
use iroha_torii::NoritoJson;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
use tower::ServiceExt as _;

const ACCOUNT_SIGNATORY: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";

#[tokio::test]
async fn zk_roots_endpoint_returns_bounded_recent_roots() {
    // Build state and cap recent roots to 3
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query);
    state.set_zk(iroha_config::parameters::actual::Zk {
        halo2: iroha_config::parameters::actual::Halo2 {
            enabled: false,
            curve: iroha_config::parameters::actual::ZkCurve::Pallas,
            backend: iroha_config::parameters::actual::Halo2Backend::Ipa,
            max_k: iroha_config::parameters::defaults::zk::halo2::MAX_K,
            verifier_budget_ms: iroha_config::parameters::defaults::zk::halo2::VERIFIER_BUDGET_MS,
            verifier_max_batch: iroha_config::parameters::defaults::zk::halo2::VERIFIER_MAX_BATCH,
            ..iroha_config::parameters::actual::Halo2::default()
        },
        fastpq: iroha_config::parameters::actual::Fastpq {
            execution_mode: iroha_config::parameters::actual::FastpqExecutionMode::Auto,
            poseidon_mode: iroha_config::parameters::actual::FastpqPoseidonMode::Auto,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: iroha_config::parameters::defaults::zk::fastpq::METAL_TRACE,
            metal_debug_enum: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_ENUM,
            metal_debug_fused: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_FUSED,
        },
        stark: iroha_config::parameters::actual::Stark::default(),
        root_history_cap: 3,
        ballot_history_cap: iroha_config::parameters::defaults::zk::vote::BALLOT_HISTORY_CAP,
        empty_root_on_empty: iroha_config::parameters::defaults::zk::ledger::EMPTY_ROOT_ON_EMPTY,
        merkle_depth: iroha_config::parameters::defaults::zk::ledger::EMPTY_ROOT_DEPTH,
        preverify_max_bytes: iroha_config::parameters::defaults::zk::preverify::MAX_BYTES,
        preverify_budget_bytes: iroha_config::parameters::defaults::zk::preverify::BUDGET_BYTES,
        proof_history_cap: iroha_config::parameters::defaults::zk::proof::RECORD_HISTORY_CAP,
        proof_retention_grace_blocks:
            iroha_config::parameters::defaults::zk::proof::RETENTION_GRACE_BLOCKS,
        proof_prune_batch: iroha_config::parameters::defaults::zk::proof::PRUNE_BATCH_SIZE,
        bridge_proof_max_range_len:
            iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_RANGE_LEN,
        bridge_proof_max_past_age_blocks:
            iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_PAST_AGE_BLOCKS,
        bridge_proof_max_future_drift_blocks:
            iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_FUTURE_DRIFT_BLOCKS,
        poseidon_params_id: iroha_config::parameters::defaults::confidential::POSEIDON_PARAMS_ID,
        pedersen_params_id: iroha_config::parameters::defaults::confidential::PEDERSEN_PARAMS_ID,
        kaigi_roster_join_vk: None,
        kaigi_roster_leave_vk: None,
        kaigi_usage_vk: None,
        max_proof_size_bytes:
            iroha_config::parameters::defaults::confidential::MAX_PROOF_SIZE_BYTES,
        max_nullifiers_per_tx:
            iroha_config::parameters::defaults::confidential::MAX_NULLIFIERS_PER_TX,
        max_commitments_per_tx:
            iroha_config::parameters::defaults::confidential::MAX_COMMITMENTS_PER_TX,
        max_confidential_ops_per_block:
            iroha_config::parameters::defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
        verify_timeout: iroha_config::parameters::defaults::confidential::VERIFY_TIMEOUT,
        max_anchor_age_blocks:
            iroha_config::parameters::defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
        max_proof_bytes_block:
            iroha_config::parameters::defaults::confidential::MAX_PROOF_BYTES_BLOCK,
        max_verify_calls_per_tx:
            iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
        max_verify_calls_per_block:
            iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
        max_public_inputs: iroha_config::parameters::defaults::confidential::MAX_PUBLIC_INPUTS,
        reorg_depth_bound: iroha_config::parameters::defaults::confidential::REORG_DEPTH_BOUND,
        policy_transition_delay_blocks:
            iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
        policy_transition_window_blocks:
            iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
        tree_roots_history_len:
            iroha_config::parameters::defaults::confidential::TREE_ROOTS_HISTORY_LEN,
        tree_frontier_checkpoint_interval:
            iroha_config::parameters::defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
        registry_max_vk_entries:
            iroha_config::parameters::defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
        registry_max_params_entries:
            iroha_config::parameters::defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
        registry_max_delta_per_block:
            iroha_config::parameters::defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
        gas: iroha_config::parameters::actual::ConfidentialGas {
            proof_base: iroha_config::parameters::defaults::confidential::gas::PROOF_BASE,
            per_public_input:
                iroha_config::parameters::defaults::confidential::gas::PER_PUBLIC_INPUT,
            per_proof_byte: iroha_config::parameters::defaults::confidential::gas::PER_PROOF_BYTE,
            per_nullifier: iroha_config::parameters::defaults::confidential::gas::PER_NULLIFIER,
            per_commitment: iroha_config::parameters::defaults::confidential::gas::PER_COMMITMENT,
        },
    });

    // Seed shielded roots via ISIs
    let domain_id: DomainId = "zkd".parse().unwrap();
    let asset_def_id = AssetDefinitionId::new(
        "zkd".parse().expect("domain id"),
        "rose".parse().expect("asset definition name"),
    );
    let asset_alias = "rose#sbp";
    let owner = AccountId::new(ACCOUNT_SIGNATORY.parse().expect("public key"));
    {
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let definition =
            AssetDefinition::numeric(asset_def_id.clone()).with_name("rose".to_owned());
        let init_instrs: [InstructionBox; 5] = [
            Register::domain(Domain::new(domain_id.clone())).into(),
            Register::account(NewAccount::new(owner.clone()).with_linked_domain(domain_id.clone()))
                .into(),
            Register::asset_definition(definition).into(),
            Mint::asset_numeric(10_000u64, AssetId::of(asset_def_id.clone(), owner.clone())).into(),
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
        ];
        for instr in init_instrs {
            stx.world
                .executor()
                .clone()
                .execute_instruction(&mut stx, &owner, instr)
                .unwrap();
        }
        iroha_data_model::isi::SetAssetDefinitionAlias::bind(
            asset_def_id.clone(),
            asset_alias.parse().expect("asset alias literal"),
            None,
        )
        .execute(&owner, &mut stx)
        .expect("bind asset alias");
        // Push 5 commitments (cap is 3).
        for i in 0..5u8 {
            let mut note = [0u8; 32];
            note[0] = i;
            let ib: InstructionBox = iroha_data_model::isi::zk::Shield::new(
                asset_def_id.clone(),
                owner.clone(),
                1u128,
                note,
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
        block.transactions.insert_block(
            HashSet::<iroha_crypto::HashOf<iroha_data_model::transaction::SignedTransaction>>::new(
            ),
            nonzero!(1_usize),
        );
        let _ = block.commit();
    }

    let state = Arc::new(state);
    // Prepare router with handler
    let app = Router::new().route(
        "/v1/zk/roots",
        post({
            let state = state.clone();
            move |NoritoJson(req): NoritoJson<iroha_torii::ZkRootsGetRequestDto>| async move {
                iroha_torii::handle_v1_zk_roots(state, None, NoritoJson(req)).await
            }
        }),
    );

    // Query with max=0 (bounded by cap=3)
    let req_body = norito::json::to_json(&iroha_torii::json_object(vec![
        iroha_torii::json_entry("asset_id", asset_alias),
        iroha_torii::json_entry("max", 0u64),
    ]))
    .expect("json serialization");
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/roots")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(req_body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    // Collect state roots for comparison
    let view = state.view();
    let roots_all = view
        .world
        .zk_assets()
        .get(&asset_def_id)
        .map(|st| st.root_history.clone())
        .unwrap_or_default();
    let want: Vec<_> = roots_all.iter().rev().take(3).copied().collect();
    let mut want_win = want.clone();
    want_win.reverse();
    let resp_roots: Vec<String> = v
        .get("roots")
        .and_then(|x| x.as_array())
        .unwrap()
        .iter()
        .filter_map(|x| x.as_str().map(ToString::to_string))
        .collect();
    let want_hex: Vec<String> = want_win.iter().map(hex::encode).collect();
    assert_eq!(resp_roots, want_hex);
    // latest must equal last root
    let latest = v.get("latest").and_then(|x| x.as_str()).unwrap_or("");
    assert_eq!(latest, hex::encode(roots_all.last().copied().unwrap()));

    // Query with max=2 (bounded by requested)
    let req_second_body = norito::json::to_json(&iroha_torii::json_object(vec![
        iroha_torii::json_entry("asset_id", asset_alias),
        iroha_torii::json_entry("max", 2u64),
    ]))
    .expect("json serialization");
    let req_second = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/roots")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(req_second_body))
        .unwrap();
    let resp_second = app.clone().oneshot(req_second).await.unwrap();
    assert_eq!(resp_second.status(), http::StatusCode::OK);
    let bytes_second = resp_second.into_body().collect().await.unwrap().to_bytes();
    let v2: norito::json::Value = norito::json::from_slice(&bytes_second).unwrap();
    let second_root_count = v2
        .get("roots")
        .and_then(|x| x.as_array())
        .unwrap()
        .iter()
        .filter_map(|x| x.as_str().map(ToString::to_string))
        .count();
    assert_eq!(second_root_count, 2);
}
