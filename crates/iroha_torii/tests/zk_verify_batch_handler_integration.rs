#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration test for /v1/zk/verify-batch minimal handler.
#![cfg(all(feature = "app_api", feature = "zk-verify-batch"))]

use axum::{Router, routing::post};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

#[tokio::test]
async fn zk_verify_batch_endpoint_accepts_norito_vec_and_returns_statuses() {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;
    use iroha_zkp_halo2::backend::pallas::PallasBackend;

    // Router with verify-batch handler
    let app = Router::new().route(
        "/v1/zk/verify-batch",
        post(
            |headers: axum::http::HeaderMap, body: axum::body::Bytes| async move {
                iroha_torii::handle_v1_zk_verify_batch(headers, body).await
            },
        ),
    );

    // Build two envelopes: ok and bad (flip t)
    let params = h2::Params::new(8).unwrap();
    let coeffs: Vec<h2::PrimeField64> = (0u64..8).map(|i| h2::PrimeField64::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new("torii-batch");
    let p_g = poly.commit(&params).unwrap();
    let z = h2::PrimeField64::from(5u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();
    let env_ok = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: "torii-batch".to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let mut bad_pub = env_ok.public.clone();
    bad_pub.t[0] = bad_pub.t[0].wrapping_add(1);
    let env_bad = h2::OpenVerifyEnvelope {
        public: bad_pub,
        ..env_ok.clone()
    };
    let norito_vec = norito::to_bytes(&vec![env_ok, env_bad]).expect("encode batch");

    // Norito request
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/verify-batch")
        .header(http::header::CONTENT_TYPE, "application/x-norito")
        .body(axum::body::Body::from(norito_vec))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    let statuses = v
        .get("statuses")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].as_bool(), Some(true));
    assert_eq!(statuses[1].as_bool(), Some(false));
}

#[cfg(feature = "goldilocks_backend")]
#[tokio::test]
async fn zk_verify_batch_endpoint_accepts_goldilocks_payload() {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;
    use iroha_zkp_halo2::{
        GoldilocksParams, GoldilocksPolynomial, GoldilocksScalar, Transcript,
        backend::goldilocks::GoldilocksBackend,
    };

    let app = Router::new().route(
        "/v1/zk/verify-batch",
        post(
            |headers: axum::http::HeaderMap, body: axum::body::Bytes| async move {
                iroha_torii::handle_v1_zk_verify_batch(headers, body).await
            },
        ),
    );

    let params = GoldilocksParams::new(8).unwrap();
    let coeffs: Vec<GoldilocksScalar> = (0u64..8).map(|i| GoldilocksScalar::from(i + 1)).collect();
    let poly = GoldilocksPolynomial::from_coeffs(coeffs);
    let mut tr = Transcript::new("torii-gold");
    let p_g = poly.commit(&params).unwrap();
    let z = GoldilocksScalar::from(6u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();
    let env_ok = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<GoldilocksBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: "torii-gold".to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let mut bad_public = env_ok.public.clone();
    bad_public.t[0] = bad_public.t[0].wrapping_add(1);
    let env_bad = h2::OpenVerifyEnvelope {
        public: bad_public,
        ..env_ok.clone()
    };
    let norito_vec = norito::to_bytes(&vec![env_ok, env_bad]).expect("encode batch");

    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/verify-batch")
        .header(http::header::CONTENT_TYPE, "application/x-norito")
        .body(axum::body::Body::from(norito_vec))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    let statuses = v
        .get("statuses")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].as_bool(), Some(true));
    assert_eq!(statuses[1].as_bool(), Some(false));
}

#[tokio::test]
async fn zk_verify_batch_endpoint_rejects_bound_metadata_tampering() {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;
    use iroha_zkp_halo2::{PolyOpenTranscriptMetadata, backend::pallas::PallasBackend};

    let app = Router::new().route(
        "/v1/zk/verify-batch",
        post(
            |headers: axum::http::HeaderMap, body: axum::body::Bytes| async move {
                iroha_torii::handle_v1_zk_verify_batch(headers, body).await
            },
        ),
    );

    let params = h2::Params::new(8).unwrap();
    let coeffs: Vec<h2::PrimeField64> = (0u64..8).map(|i| h2::PrimeField64::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let metadata = PolyOpenTranscriptMetadata {
        vk_commitment: Some([0x11; 32]),
        public_inputs_schema_hash: Some([0x22; 32]),
        domain_tag: Some([0x33; 32]),
    };
    let mut tr = h2::Transcript::new("torii-batch-bound");
    let p_g = poly.commit(&params).unwrap();
    let z = h2::PrimeField64::from(6u64);
    let (proof, t) = poly
        .open_with_metadata(&params, &mut tr, z, p_g, metadata)
        .unwrap();
    let env_ok = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: "torii-batch-bound".to_string(),
        vk_commitment: metadata.vk_commitment,
        public_inputs_schema_hash: metadata.public_inputs_schema_hash,
        domain_tag: metadata.domain_tag,
    };
    let mut env_bad = env_ok.clone();
    env_bad.domain_tag = Some([0x44; 32]);
    let norito_vec = norito::to_bytes(&vec![env_ok, env_bad]).expect("encode batch");

    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/verify-batch")
        .header(http::header::CONTENT_TYPE, "application/x-norito")
        .body(axum::body::Body::from(norito_vec))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let statuses = v
        .get("statuses")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].as_bool(), Some(true));
    assert_eq!(statuses[1].as_bool(), Some(false));
}
