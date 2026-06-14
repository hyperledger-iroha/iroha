#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Persist VRF-derived council into WSV and verify `current` returns it.
#![cfg(all(feature = "app_api", feature = "gov_vrf"))]

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use base64::Engine as _;
use http_body_util::BodyExt as _;
use iroha_core::{
    governance::parliament,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, StateReadOnly, World},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::account::AccountId;
use tower::ServiceExt as _;

fn fixture_account(seed: u8) -> AccountId {
    let mut material = [0u8; 32];
    let domain = b"torii-gov-vrf-candidate";
    material[..domain.len()].copy_from_slice(domain);
    material[31] = seed;
    let keypair = KeyPair::try_from_seed(material.to_vec(), Algorithm::Ed25519)
        .expect("fixture seed must derive a valid keypair");
    AccountId::new(keypair.public_key().clone())
}

fn fixture_bls_normal_keypair(
    seed: u8,
) -> (
    iroha_crypto::BlsNormalPublicKey,
    iroha_crypto::BlsNormalPrivateKey,
) {
    let mut material = b"torii-gov-vrf-bls-candidate".to_vec();
    material.push(seed);
    iroha_crypto::BlsNormal::keypair(iroha_crypto::KeyGenOption::UseSeed(material))
        .expect("fixture seed must derive a valid BLS normal keypair")
}

#[test]
fn fixture_keys_use_checked_seed_derivation() {
    assert_ne!(fixture_account(0), fixture_account(1));
    let (bls_public, _bls_private) = fixture_bls_normal_keypair(0);
    let (other_bls_public, _other_bls_private) = fixture_bls_normal_keypair(1);
    assert!(bls_public != other_bls_public);
    assert!(
        KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
        "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
    );
    assert!(
        iroha_crypto::BlsNormal::keypair(iroha_crypto::KeyGenOption::UseSeed(vec![0; 4])).is_err(),
        "checked BLS seed derivation must reject weak all-zero fixture seeds"
    );
}

#[tokio::test]
async fn persist_vrf_council_response_matches_derive_vrf() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: gov VRF council persist test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    // Minimal state
    let state = Arc::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events = tokio::sync::broadcast::channel(4).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();

    let chain_id_value = state.view().chain_id().clone();
    let chain_id_arc = Arc::new(chain_id_value.clone());

    // Routes: persist, derive-vrf, current
    let app = Router::new()
        .route(
            "/v1/gov/council/persist",
            post({
                let state = state.clone();
                let queue = queue.clone();
                let chain_id = chain_id_arc.clone();
                let telemetry = telemetry.clone();
                move |body: iroha_torii::NoritoJson<iroha_torii::CouncilPersistRequest>| async move {
                    iroha_torii::handle_gov_council_persist(
                        chain_id.clone(),
                        queue.clone(),
                        state.clone(),
                        telemetry.clone(),
                        body,
                    )
                    .await
                }
            }),
        )
        .route(
            "/v1/gov/council/derive-vrf",
            post({
                let state = state.clone();
                move |body: iroha_torii::NoritoJson<iroha_torii::CouncilDeriveVrfRequest>| async move {
                    iroha_torii::handle_gov_council_derive_vrf(state, body).await
                }
            }),
        )
        .route(
            "/v1/gov/council/current",
            get({
                let state = state.clone();
                move || async move { iroha_torii::handle_gov_council_current(state).await }
            }),
        );

    // Compose seed and inputs as handler does
    let v = state.view();
    let chain_id_value = v.chain_id().clone();
    let chain_id_str = chain_id_value.to_string();
    const TERM_BLOCKS: u64 = 43_200;
    let height = v.height() as u64;
    let epoch = height / TERM_BLOCKS;
    let beacon_bytes: [u8; 32] = v
        .latest_block_hash()
        .map(|h| *h.as_ref())
        .unwrap_or([0u8; 32]);
    let seed = parliament::compute_seed(&chain_id_value, epoch, &beacon_bytes);

    // Build 5 candidates (Normal variant).
    let mut candidates = Vec::new();
    for i in 0..5u8 {
        let (pk, sk) = fixture_bls_normal_keypair(i);
        let pk_bytes = KeyPair::from((pk.clone(), sk.clone()))
            .public_key()
            .to_bytes()
            .1
            .to_vec();
        let pk_b64 = base64::engine::general_purpose::STANDARD.encode(pk_bytes);
        let account = fixture_account(i);
        let account_id = account.to_string();
        let input = parliament::build_input(&seed, &account);
        let (_y, pi) =
            iroha_crypto::vrf::prove_normal_with_chain(&sk, chain_id_str.as_bytes(), &input)
                .expect("normal VRF proof");
        let pr_b64 = base64::engine::general_purpose::STANDARD.encode(match pi {
            iroha_crypto::vrf::VrfProof::SigInG2(arr) => arr.to_vec(),
            _ => unreachable!(),
        });
        candidates.push(iroha_torii::json_object(vec![
            iroha_torii::json_entry("account_id", account_id),
            iroha_torii::json_entry("variant", "Normal"),
            iroha_torii::json_entry("pk_b64", pk_b64),
            iroha_torii::json_entry("proof_b64", pr_b64),
        ]));
    }

    // Persist with committee_size = 3
    let body = norito::json::to_string(&iroha_torii::json_object(vec![
        iroha_torii::json_entry("committee_size", 3usize),
        iroha_torii::json_entry("epoch", epoch),
        iroha_torii::json_entry("candidates", candidates.clone()),
    ]))
    .expect("serialize persist request body");
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/gov/council/persist")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);

    let persist_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let persisted: norito::json::Value = norito::json::from_slice(&persist_bytes).unwrap();
    let persisted_members = persisted
        .get("members")
        .and_then(|x| x.as_array())
        .unwrap()
        .iter()
        .map(|m| {
            m.get("account_id")
                .and_then(|x| x.as_str())
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert!(
        persisted
            .get("tx_instructions")
            .and_then(|x| x.as_array())
            .map_or(false, |items| !items.is_empty()),
        "persist response must return an unsigned council persistence instruction"
    );

    // Derive again via handler to produce a comparable ordering
    let req3 = http::Request::builder()
        .method("POST")
        .uri("/v1/gov/council/derive-vrf")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body({
            let body = iroha_torii::json_object(vec![
                iroha_torii::json_entry("committee_size", 3usize),
                iroha_torii::json_entry("epoch", epoch),
                iroha_torii::json_entry("candidates", candidates),
            ]);
            axum::body::Body::from(
                norito::json::to_string(&body).expect("serialize derive request body"),
            )
        })
        .unwrap();
    let resp3 = app.clone().oneshot(req3).await.unwrap();
    assert_eq!(resp3.status(), http::StatusCode::OK);
    let b3 = resp3.into_body().collect().await.unwrap().to_bytes();
    let dv: norito::json::Value = norito::json::from_slice(&b3).unwrap();
    let derived_members = dv
        .get("members")
        .and_then(|x| x.as_array())
        .unwrap()
        .iter()
        .map(|m| {
            m.get("account_id")
                .and_then(|x| x.as_str())
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        persisted_members, derived_members,
        "persist response and derived members must match"
    );
}
