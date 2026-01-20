//! VRF-backed council derivation tests (feature: `gov_vrf` + `app_api`).
#![cfg(all(feature = "app_api", feature = "gov_vrf"))]

use std::sync::Arc;

use axum::response::IntoResponse;
use http_body_util::BodyExt as _;
use iroha_core::{
    governance::parliament,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{account::AccountId, domain::DomainId};
use norito::json;

#[tokio::test]
async fn vrf_derive_orders_desc_and_tie_breaks_by_account() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: gov VRF derive test gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    // Minimal state; height and beacon default to 0
    let state = Arc::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let chain_id_value = state.view().chain_id().clone();
    let chain_id = chain_id_value.to_string();

    // Compute council seed the same way as the handler
    const TERM_BLOCKS: u64 = 43_200;
    let height = state.view().height() as u64;
    let epoch = height / TERM_BLOCKS;
    let beacon_bytes: [u8; 32] = state
        .view()
        .latest_block_hash()
        .map(|h| *h.as_ref())
        .unwrap_or([0u8; 32]);
    let seed = parliament::compute_seed(&chain_id_value, epoch, &beacon_bytes);

    // Build two distinct candidates with valid VRF proofs (Normal variant)
    let domain: DomainId = "wonderland".parse().unwrap();
    let account_alice = AccountId::new(
        domain.clone(),
        KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let account_bob = AccountId::new(
        domain.clone(),
        KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let account_carol = AccountId::new(
        domain,
        KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );

    let (pk1, sk1) =
        iroha_crypto::BlsNormal::keypair(iroha_crypto::KeyGenOption::UseSeed(vec![1, 2, 3, 4]));
    let (pk2, sk2) =
        iroha_crypto::BlsNormal::keypair(iroha_crypto::KeyGenOption::UseSeed(vec![4, 3, 2, 1]));
    let (pk3, sk3) =
        iroha_crypto::BlsNormal::keypair(iroha_crypto::KeyGenOption::UseSeed(vec![9, 9, 9, 9]));

    let input_alice = parliament::build_input(&seed, &account_alice);
    let input_bob = parliament::build_input(&seed, &account_bob);
    let input_carol = parliament::build_input(&seed, &account_carol);

    let (_y1, pi1) =
        iroha_crypto::vrf::prove_normal_with_chain(&sk1, chain_id.as_bytes(), &input_alice);
    let (_y2, pi2) =
        iroha_crypto::vrf::prove_normal_with_chain(&sk2, chain_id.as_bytes(), &input_bob);
    let (_y3, pi3) =
        iroha_crypto::vrf::prove_normal_with_chain(&sk3, chain_id.as_bytes(), &input_carol);

    let pk1_bytes = pk1.to_bytes();
    let pk2_bytes = pk2.to_bytes();
    let pk3_bytes = pk3.to_bytes();
    let proof1 = match pi1 {
        iroha_crypto::vrf::VrfProof::SigInG2(arr) => arr,
        _ => unreachable!("normal variant uses SigInG2"),
    };
    let proof2 = match pi2 {
        iroha_crypto::vrf::VrfProof::SigInG2(arr) => arr,
        _ => unreachable!("normal variant uses SigInG2"),
    };
    let proof3 = match pi3 {
        iroha_crypto::vrf::VrfProof::SigInG2(arr) => arr,
        _ => unreachable!("normal variant uses SigInG2"),
    };
    let pk1_b64 = base64::engine::general_purpose::STANDARD.encode(pk1_bytes);
    let pk2_b64 = base64::engine::general_purpose::STANDARD.encode(pk2_bytes);
    let pk3_b64 = base64::engine::general_purpose::STANDARD.encode(pk3_bytes);
    let pi1_b64 = base64::engine::general_purpose::STANDARD.encode(proof1);
    let pi2_b64 = base64::engine::general_purpose::STANDARD.encode(proof2);
    let pi3_b64 = base64::engine::general_purpose::STANDARD.encode(proof3);

    let alice = account_alice.to_string();
    let bob = account_bob.to_string();
    let carol = account_carol.to_string();

    // Build request DTO as expected by handler
    let body = iroha_torii::json_object(vec![
        ("committee_size", 3usize),
        ("epoch", epoch),
        (
            "candidates",
            iroha_torii::json_array(vec![
                iroha_torii::json_object(vec![
                    ("account_id", alice.clone()),
                    ("variant", "Normal"),
                    ("pk_b64", pk1_b64.clone()),
                    ("proof_b64", pi1_b64.clone()),
                ]),
                iroha_torii::json_object(vec![
                    ("account_id", bob.clone()),
                    ("variant", "Normal"),
                    ("pk_b64", pk2_b64),
                    ("proof_b64", pi2_b64),
                ]),
                iroha_torii::json_object(vec![
                    ("account_id", carol.clone()),
                    ("variant", "Normal"),
                    ("pk_b64", pk3_b64),
                    ("proof_b64", pi3_b64),
                ]),
            ]),
        ),
    ]);
    // Use the app handler directly
    let req: iroha_torii::CouncilDeriveVrfRequest = norito::json::from_value(body).unwrap();
    let resp = iroha_torii::gov::handle_gov_council_derive_vrf(
        state,
        iroha_torii::utils::extractors::NoritoJson(req),
    )
    .await
    .expect("handler ok")
    .into_response();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let members = v
        .get("members")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(members.len(), 3);
    let candidate_refs = [
        parliament::CandidateRef {
            account_id: &account_alice,
            variant: parliament::CandidateVariant::Normal,
            public_key: pk1_bytes.as_slice(),
            proof: proof1.as_slice(),
        },
        parliament::CandidateRef {
            account_id: &account_bob,
            variant: parliament::CandidateVariant::Normal,
            public_key: pk2_bytes.as_slice(),
            proof: proof2.as_slice(),
        },
        parliament::CandidateRef {
            account_id: &account_carol,
            variant: parliament::CandidateVariant::Normal,
            public_key: pk3_bytes.as_slice(),
            proof: proof3.as_slice(),
        },
    ];
    let expected = parliament::derive_committee(&chain_id_value, &seed, candidate_refs, 3)
        .members
        .into_iter()
        .map(|member| member.account_id.to_string())
        .collect::<Vec<_>>();
    let observed = members
        .iter()
        .map(|member| {
            member
                .get("account_id")
                .and_then(|x| x.as_str())
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(observed, expected, "VRF ordering should be deterministic");
}
