#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration test for /v1/proofs/query (signed core query wrapper).
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::{Router, routing::post};
use http_body_util::BodyExt as _;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{
    Registrable,
    proof::{ProofId, ProofRecord, ProofStatus, VerifyingKeyId},
};
use iroha_torii::QueryOptions;
use mv::storage::StorageReadOnly;
use norito::json;
use tower::ServiceExt as _;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn proofs_query_find_by_id_returns_norito() {
    let backend = "halo2/ipa";
    let proof_hash = [0xAA; 32];

    // Authority registered in state so query validation succeeds
    let key_pair = iroha_crypto::KeyPair::random();
    let domain_name = "wonderland";
    let domain_id: iroha_data_model::domain::DomainId = domain_name.parse().unwrap();
    let authority = iroha_data_model::account::AccountId::new(key_pair.public_key().clone());
    let domain = iroha_data_model::domain::Domain::new(domain_id.clone()).build(&authority);
    let account = iroha_data_model::account::Account::new(authority.to_account_id(domain_id))
        .build(&authority);
    let world = World::with(
        [domain],
        [account],
        std::iter::empty::<iroha_data_model::asset::definition::AssetDefinition>(),
    );

    // Minimal state and live query store
    let kura = Kura::blank_kura_for_testing();
    let live = LiveQueryStore::start_test();
    let live_for_route = live.clone();
    let state = State::new_for_testing(world, kura, live);
    let mut state = state;

    // Seed one proof record
    let id = ProofId {
        backend: backend.into(),
        proof_hash,
    };
    let proof_id = id.clone();
    let rec = ProofRecord {
        id: id.clone(),
        vk_ref: Some(VerifyingKeyId::new("halo2/ipa", "vk_test")),
        vk_commitment: None,
        status: ProofStatus::Verified,
        verified_at_height: Some(1),
        bridge: None,
    };
    iroha_core::query::insert_proof_record_for_test(&mut state, id, rec);
    {
        let view = state.view();
        assert!(
            view.world().proofs().get(&proof_id).is_some(),
            "proof record not inserted"
        );
    }

    let state = Arc::new(state);
    #[cfg(feature = "telemetry")]
    let tel = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let tel = iroha_torii::MaybeTelemetry::for_tests();

    let app = Router::new().route(
        "/v1/proofs/query",
        post({
            let state = state.clone();
            move |iroha_torii::NoritoJson(dto): iroha_torii::NoritoJson<
                iroha_torii::ProofFindByIdQueryDto,
            >| async move {
                let signed = iroha_torii::signed_find_proof_by_id(&dto)?;
                iroha_torii::handle_queries_with_opts(
                    live_for_route.clone(),
                    state,
                    signed,
                    tel,
                    iroha_torii::NoritoQuery(QueryOptions::default()),
                    iroha_torii::ResponseFormat::Norito,
                )
                .await
            }
        }),
    );

    let private_key = iroha_crypto::ExposedPrivateKey(key_pair.private_key().clone()).to_string();
    let dto = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", authority.to_string()),
        iroha_torii::json_entry("private_key", private_key),
        iroha_torii::json_entry("backend", backend),
        iroha_torii::json_entry("hash_hex", hex::encode(proof_hash)),
    ]);
    let body = json::to_vec(&dto).unwrap();
    let request = http::Request::builder()
        .method("POST")
        .uri("/v1/proofs/query")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(request).await.unwrap();
    let status = resp.status();
    let ct = resp
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        http::StatusCode::OK,
        "unexpected status {status} body={}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(ct.contains("application/x-norito"), "content-type: {ct}");
    // Body should be non-empty Norito payload
    assert!(!bytes.is_empty());
}
