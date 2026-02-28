#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for governance read endpoint wiring (`/v1/gov/proposals/{id}`).
#![allow(clippy::similar_names)]
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::{Router, routing::get};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::governance::types::{
    AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ProposalKind,
};
use tower::ServiceExt as _; // for Router::oneshot

#[tokio::test]
async fn gov_proposal_get_router_mapping() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: gov proposal-get router test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }

    // Minimal in-memory state and a single proposal record
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut raw_state = State::new_for_testing(World::default(), kura, query);
    let id_bytes = [0xCDu8; 32];
    let id_hex = hex::encode(id_bytes);
    let header = iroha_data_model::block::BlockHeader::new(
        nonzero_ext::nonzero!(1_u64),
        None,
        None,
        None,
        0,
        0,
    );
    {
        // Open and immediately drop a state block to set up initial header if required.
        let _ = raw_state.block(header);
    }
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
        parliament_snapshot: None,
    };
    iroha_core::query::insert_gov_proposal_for_test(&mut raw_state, id_bytes, rec);

    // Wire only the GET route under the shared URI constant
    let state = Arc::new(raw_state);
    let app = Router::new().route(
        iroha_torii_shared::uri::GOV_PROPOSAL_GET,
        get({
            let state = state.clone();
            move |axum::extract::Path(id): axum::extract::Path<String>| async move {
                iroha_torii::handle_gov_get_proposal(state, axum::extract::Path(id)).await
            }
        }),
    );

    let request = http::Request::builder()
        .method("GET")
        .uri(format!("/v1/gov/proposals/{id_hex}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(request).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(
        v.get("found").and_then(norito::json::Value::as_bool),
        Some(true)
    );
    let deploy_payload = v
        .get("proposal")
        .and_then(|o| o.get("kind"))
        .and_then(|k| k.get("DeployContract"))
        .expect("deploy payload present");
    assert_eq!(
        deploy_payload
            .get("namespace")
            .and_then(norito::json::Value::as_str),
        Some("apps")
    );
}
