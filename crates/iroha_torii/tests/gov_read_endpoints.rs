#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii handler tests for governance read endpoints: proposal/referendum/locks/tally.
#![allow(unexpected_cfgs, clippy::similar_names, clippy::too_many_lines)]

use std::sync::Arc;

use axum::{extract::Path as AxPath, response::IntoResponse};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    account::AccountId as DMAccountId,
    asset::prelude::{AssetDefinitionId, AssetId},
    common::Owned,
    domain::prelude::DomainId,
    governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ProposalKind,
    },
    peer::PeerId,
};
use iroha_primitives::{numeric::Numeric, unique_vec::UniqueVec};
use mv::storage::StorageReadOnly;

#[tokio::test]
async fn gov_proposal_get_returns_record() {
    let mut st_raw = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );

    // Insert a proposal record under a known id
    let id_bytes = [0xAAu8; 32];
    let id_hex = hex::encode(id_bytes);
    // Build a minimal proposer id
    let kp = iroha_crypto::KeyPair::random();
    let proposer: iroha_data_model::account::AccountId = iroha_data_model::account::AccountId::of(
        "wonderland".parse().unwrap(),
        kp.public_key().clone(),
    );
    let rec = iroha_core::state::GovernanceProposalRecord {
        proposer,
        kind: ProposalKind::DeployContract(DeployContractProposal {
            namespace: "apps".to_string(),
            contract_id: "calc.v1".to_string(),
            code_hash_hex: ContractCodeHash::from_hex_str(&"11".repeat(32)).expect("code hash"),
            abi_hash_hex: ContractAbiHash::from_hex_str(&"22".repeat(32)).expect("abi hash"),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        }),
        created_height: 0,
        status: iroha_core::state::GovernanceProposalStatus::Proposed,
        pipeline: iroha_core::state::GovernancePipeline::default(),
    };
    iroha_core::query::insert_gov_proposal_for_test(&mut st_raw, id_bytes, rec);
    {
        let view = st_raw.view();
        assert!(
            view.world().governance_proposals().get(&id_bytes).is_some(),
            "proposal record should be present in state"
        );
    }

    let state = Arc::new(st_raw);
    let resp = iroha_torii::handle_gov_get_proposal(state.clone(), AxPath(id_hex))
        .await
        .expect("handler ok")
        .into_response();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(
        v.get("found").and_then(norito::json::Value::as_bool),
        Some(true)
    );
    let proposal_obj = v
        .get("proposal")
        .and_then(norito::json::Value::as_object)
        .expect("proposal present");
    let kind_obj = proposal_obj
        .get("kind")
        .and_then(norito::json::Value::as_object)
        .expect("kind present");
    assert_eq!(
        kind_obj.get("kind").and_then(norito::json::Value::as_str),
        Some("DeployContract")
    );
    let deploy_payload = kind_obj
        .get("payload")
        .and_then(norito::json::Value::as_object)
        .expect("deploy payload present");
    assert_eq!(
        deploy_payload
            .get("contract_id")
            .and_then(norito::json::Value::as_str),
        Some("calc.v1")
    );
}

#[tokio::test]
async fn gov_proposal_get_invalid_id_and_missing_entry() {
    let state = Arc::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));

    // Case 1: invalid hex (odd length)
    let err = iroha_torii::handle_gov_get_proposal(state.clone(), AxPath("abc".to_string()))
        .await
        .expect_err("expected error for invalid id hex");
    match err {
        iroha_torii::Error::Query(vf) => match vf {
            iroha_data_model::ValidationFail::QueryFailed(qf) => match qf {
                iroha_data_model::query::error::QueryExecutionFail::Conversion(msg) => {
                    assert_eq!(msg, "invalid id");
                }
                other => panic!("unexpected query fail variant: {other:?}"),
            },
            other => panic!("unexpected validation fail: {other:?}"),
        },
        other => panic!("unexpected error type: {other:?}"),
    }

    // Case 2: valid hex but wrong length (31 bytes)
    let bad_len = "aa".repeat(31);
    let err = iroha_torii::handle_gov_get_proposal(state.clone(), AxPath(bad_len))
        .await
        .expect_err("expected error for invalid id length");
    match err {
        iroha_torii::Error::Query(vf) => match vf {
            iroha_data_model::ValidationFail::QueryFailed(qf) => match qf {
                iroha_data_model::query::error::QueryExecutionFail::Conversion(msg) => {
                    assert_eq!(msg, "invalid id length");
                }
                other => panic!("unexpected query fail variant: {other:?}"),
            },
            other => panic!("unexpected validation fail: {other:?}"),
        },
        other => panic!("unexpected error type: {other:?}"),
    }

    // Case 3: well-formed (32-byte) id that does not exist → found=false
    let missing = "bb".repeat(32);
    let resp = iroha_torii::handle_gov_get_proposal(state, AxPath(missing))
        .await
        .expect("handler ok for missing entry")
        .into_response();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(
        v.get("found").and_then(norito::json::Value::as_bool),
        Some(false)
    );
}

#[tokio::test]
async fn gov_council_current_uses_configured_fallback() {
    let kura = Kura::blank_kura_for_testing();
    let query_store = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query_store);

    state.gov.parliament_committee_size = 1;
    state.gov.parliament_term_blocks = 5;
    state.gov.parliament_min_stake = 200;
    state.gov.parliament_eligibility_asset_id = "stake#wonderland"
        .parse::<AssetDefinitionId>()
        .expect("asset id");

    let mut wb = state.world.block();
    let domain: DomainId = "wonderland".parse().expect("domain");
    let stake_def = state.gov.parliament_eligibility_asset_id.clone();

    let eligible_kp = KeyPair::random();
    let ineligible_kp = KeyPair::random();
    let eligible_account = DMAccountId::of(domain.clone(), eligible_kp.public_key().clone());
    let ineligible_account = DMAccountId::of(domain.clone(), ineligible_kp.public_key().clone());

    let mut peers: UniqueVec<_> = UniqueVec::new();
    let _ = peers.push(PeerId::from(eligible_kp.public_key().clone()));
    let _ = peers.push(PeerId::from(ineligible_kp.public_key().clone()));
    *wb.peers_mut_for_testing().get_mut() = peers;

    let eligible_asset = AssetId::of(stake_def.clone(), eligible_account.clone());
    let ineligible_asset = AssetId::of(stake_def.clone(), ineligible_account.clone());
    wb.assets_mut_for_testing()
        .insert(eligible_asset, Owned::new(Numeric::from(500_u64)));
    wb.assets_mut_for_testing()
        .insert(ineligible_asset, Owned::new(Numeric::from(50_u64)));
    wb.commit();

    let state = Arc::new(state);
    let resp = iroha_torii::handle_gov_council_current(state)
        .await
        .expect("handler ok")
        .0;

    assert_eq!(resp.members.len(), 1);
    let member = &resp.members[0].account_id;
    assert_eq!(member, &eligible_account.to_string());
}

#[tokio::test]
async fn gov_referendum_and_locks_and_tally_endpoints() {
    let mut raw_state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );

    // Prepare a referendum record and a locks record under rid = "r1"
    let rid = "r1".to_string();
    // Referendum record
    let rr = iroha_core::state::GovernanceReferendumRecord {
        h_start: 0,
        h_end: 100,
        status: iroha_core::state::GovernanceReferendumStatus::Open,
        mode: iroha_core::state::GovernanceReferendumMode::Plain,
    };
    // Locks: one Aye with amount=10000, duration=conviction_step_blocks (factor=2 before clamp).
    let kp = iroha_crypto::KeyPair::random();
    let owner: iroha_data_model::account::AccountId = iroha_data_model::account::AccountId::of(
        "wonderland".parse().unwrap(),
        kp.public_key().clone(),
    );
    let conviction_step = raw_state.gov.conviction_step_blocks.max(1);
    let duration_blocks = conviction_step;
    let max_conviction = raw_state.gov.max_conviction;
    let mut locks = iroha_core::state::GovernanceLocksForReferendum::default();
    let rec = iroha_core::state::GovernanceLockRecord {
        owner: owner.clone(),
        amount: 10_000u128,
        slashed: 0,
        expiry_height: 1_000,
        direction: 0, // approve
        duration_blocks,
    };
    locks.locks.insert(owner, rec);
    let prev_height = raw_state.view().height();
    assert_eq!(prev_height, 0, "fresh test state should start at height 0");
    iroha_core::query::insert_gov_referendum_for_test(&mut raw_state, rid.clone(), rr);
    iroha_core::query::insert_gov_locks_for_test(&mut raw_state, rid.clone(), locks);

    let state = Arc::new(raw_state);

    // GET referendum
    let resp_r = iroha_torii::handle_gov_get_referendum(state.clone(), AxPath(rid.clone()))
        .await
        .expect("handler ok")
        .into_response();
    assert_eq!(resp_r.status(), http::StatusCode::OK);
    let br = resp_r.into_body().collect().await.unwrap().to_bytes();
    let vr: norito::json::Value = norito::json::from_slice(&br).unwrap();
    assert_eq!(
        vr.get("found").and_then(norito::json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        vr.get("referendum")
            .and_then(|o| o.get("mode"))
            .and_then(norito::json::Value::as_str),
        Some("Plain")
    );

    // GET locks
    let resp_l = iroha_torii::handle_gov_get_locks(state.clone(), AxPath(rid.clone()))
        .await
        .expect("handler ok")
        .into_response();
    assert_eq!(resp_l.status(), http::StatusCode::OK);
    let bl = resp_l.into_body().collect().await.unwrap().to_bytes();
    let vl: norito::json::Value = norito::json::from_slice(&bl).unwrap();
    assert_eq!(
        vl.get("found").and_then(norito::json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        vl.get("referendum_id")
            .and_then(norito::json::Value::as_str),
        Some(&rid[..])
    );

    // GET tally: sqrt(10000) = 100, conviction factor = 1 + duration/step.
    let resp_t = iroha_torii::handle_gov_get_tally(state.clone(), AxPath(rid))
        .await
        .expect("handler ok")
        .into_response();
    assert_eq!(resp_t.status(), http::StatusCode::OK);
    let bt = resp_t.into_body().collect().await.unwrap().to_bytes();
    let vt: norito::json::Value = norito::json::from_slice(&bt).unwrap();
    let mut factor = 1u64 + (duration_blocks / conviction_step);
    if factor > max_conviction {
        factor = max_conviction;
    }
    let expected_approve = 100u128.saturating_mul(u128::from(factor));
    assert_eq!(
        vt.get("approve").and_then(norito::json::Value::as_u64),
        Some(expected_approve as u64)
    );

    // Missing ids: referendum/locks not present → found=false; tally zeros
    let missing_id = "missing-rid".to_string();
    let resp_rm = iroha_torii::handle_gov_get_referendum(state.clone(), AxPath(missing_id.clone()))
        .await
        .expect("handler ok")
        .into_response();
    let brm = resp_rm.into_body().collect().await.unwrap().to_bytes();
    let vrm: norito::json::Value = norito::json::from_slice(&brm).unwrap();
    assert_eq!(
        vrm.get("found").and_then(norito::json::Value::as_bool),
        Some(false)
    );

    let resp_lm = iroha_torii::handle_gov_get_locks(state.clone(), AxPath(missing_id.clone()))
        .await
        .expect("handler ok")
        .into_response();
    let blm = resp_lm.into_body().collect().await.unwrap().to_bytes();
    let vlm: norito::json::Value = norito::json::from_slice(&blm).unwrap();
    assert_eq!(
        vlm.get("found").and_then(norito::json::Value::as_bool),
        Some(false)
    );

    let resp_tm = iroha_torii::handle_gov_get_tally(state, AxPath(missing_id))
        .await
        .expect("handler ok")
        .into_response();
    let btm = resp_tm.into_body().collect().await.unwrap().to_bytes();
    let vtm: norito::json::Value = norito::json::from_slice(&btm).unwrap();
    assert_eq!(
        vtm.get("approve").and_then(norito::json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        vtm.get("reject").and_then(norito::json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        vtm.get("abstain").and_then(norito::json::Value::as_u64),
        Some(0)
    );
}
