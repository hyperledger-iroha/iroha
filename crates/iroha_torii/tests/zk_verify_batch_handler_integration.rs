#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration test for /v1/zk/verify-batch minimal handler.
#![cfg(all(feature = "app_api", feature = "zk-verify-batch"))]

use axum::{Router, routing::post};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

fn sample_pallas_envelope(label: &str) -> iroha_zkp_halo2::OpenVerifyEnvelope {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;
    use iroha_zkp_halo2::backend::pallas::PallasBackend;

    let params = h2::Params::new(8).unwrap();
    let coeffs: Vec<h2::PrimeField64> = (0u64..8).map(|i| h2::PrimeField64::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new(label);
    let p_g = poly.commit(&params).unwrap();
    let z = h2::PrimeField64::from(5u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();
    h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: label.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    }
}

async fn post_json_batch_with_limits(
    body: String,
    open_limits: iroha_zkp_halo2::OpenVerifyLimits,
    max_envelope_bytes: usize,
    enforce_transcript_label_ascii: bool,
) -> norito::json::Value {
    post_batch_with_limits(
        body.into_bytes(),
        "application/json",
        open_limits,
        16,
        max_envelope_bytes,
        enforce_transcript_label_ascii,
    )
    .await
}

async fn post_batch_with_limits(
    body: Vec<u8>,
    content_type: &'static str,
    open_limits: iroha_zkp_halo2::OpenVerifyLimits,
    max_batch: usize,
    max_envelope_bytes: usize,
    enforce_transcript_label_ascii: bool,
) -> norito::json::Value {
    let app = Router::new().route(
        "/v1/zk/verify-batch",
        post(
            move |headers: axum::http::HeaderMap, body: axum::body::Bytes| async move {
                iroha_torii::handle_v1_zk_verify_batch_with_limits(
                    headers,
                    body,
                    open_limits,
                    max_batch,
                    max_envelope_bytes,
                    enforce_transcript_label_ascii,
                )
                .await
            },
        ),
    );

    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/verify-batch")
        .header(http::header::CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    norito::json::from_slice(&bytes).unwrap()
}

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

#[tokio::test]
async fn zk_verify_batch_endpoint_accepts_json_array_and_returns_mixed_statuses() {
    use base64::Engine as _;

    let app = Router::new().route(
        "/v1/zk/verify-batch",
        post(
            |headers: axum::http::HeaderMap, body: axum::body::Bytes| async move {
                iroha_torii::handle_v1_zk_verify_batch(headers, body).await
            },
        ),
    );

    let env_ok = sample_pallas_envelope("torii-json-default");
    let mut env_bad = env_ok.clone();
    env_bad.public.t[0] = env_bad.public.t[0].wrapping_add(1);
    let encoded_ok = base64::engine::general_purpose::STANDARD
        .encode(norito::to_bytes(&env_ok).expect("encode ok envelope"));
    let encoded_bad = base64::engine::general_purpose::STANDARD
        .encode(norito::to_bytes(&env_bad).expect("encode bad envelope"));
    let body = format!(r#"["{encoded_ok}","{encoded_bad}"]"#);

    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/zk/verify-batch")
        .header(
            http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )
        .body(axum::body::Body::from(body))
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
async fn zk_verify_batch_norito_accepts_empty_batch() {
    let v = post_batch_with_limits(
        norito::to_bytes(&Vec::<iroha_zkp_halo2::OpenVerifyEnvelope>::new())
            .expect("encode empty batch"),
        "application/x-norito",
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        16,
        usize::MAX,
        false,
    )
    .await;

    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(
        v.get("statuses").and_then(|x| x.as_array()).map(Vec::len),
        Some(0)
    );
}

#[tokio::test]
async fn zk_verify_batch_endpoint_enforces_diagnostic_limits() {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;
    use iroha_zkp_halo2::backend::pallas::PallasBackend;

    let params = h2::Params::new(8).unwrap();
    let coeffs: Vec<h2::PrimeField64> = (0u64..8).map(|i| h2::PrimeField64::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new("too-long-label");
    let p_g = poly.commit(&params).unwrap();
    let z = h2::PrimeField64::from(5u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();
    let env = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: "too-long-label".to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };

    let norito_vec = norito::to_bytes(&vec![env.clone(), env.clone()]).expect("encode batch");
    let app = Router::new().route(
        "/v1/zk/verify-batch",
        post(
            |headers: axum::http::HeaderMap, body: axum::body::Bytes| async move {
                iroha_torii::handle_v1_zk_verify_batch_with_limits(
                    headers,
                    body,
                    h2::OpenVerifyLimits {
                        max_k: Some(3),
                        max_transcript_label_len: Some(64),
                    },
                    1,
                    usize::MAX,
                    true,
                )
                .await
            },
        ),
    );
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
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));
    assert_eq!(
        v.get("error").and_then(|x| x.as_str()),
        Some("batch_too_large")
    );

    let norito_vec = norito::to_bytes(&vec![env]).expect("encode batch");
    let app = Router::new().route(
        "/v1/zk/verify-batch",
        post(
            |headers: axum::http::HeaderMap, body: axum::body::Bytes| async move {
                iroha_torii::handle_v1_zk_verify_batch_with_limits(
                    headers,
                    body,
                    h2::OpenVerifyLimits {
                        max_k: Some(3),
                        max_transcript_label_len: Some(4),
                    },
                    1,
                    usize::MAX,
                    true,
                )
                .await
            },
        ),
    );
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
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].as_bool(), Some(false));
}

#[tokio::test]
async fn zk_verify_batch_json_preserves_false_statuses_for_bad_entries() {
    use base64::Engine as _;

    let env = sample_pallas_envelope("torii-json");
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(norito::to_bytes(&env).expect("encode envelope"));
    let body = format!(r#"["{encoded}","not base64",7]"#);
    let v = post_json_batch_with_limits(
        body,
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        usize::MAX,
        false,
    )
    .await;

    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    let statuses = v
        .get("statuses")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(statuses.len(), 3);
    assert_eq!(statuses[0].as_bool(), Some(true));
    assert_eq!(statuses[1].as_bool(), Some(false));
    assert_eq!(statuses[2].as_bool(), Some(false));
}

#[tokio::test]
async fn zk_verify_batch_json_applies_per_entry_diagnostic_limits() {
    use base64::Engine as _;

    let env = sample_pallas_envelope("torii-json-limits");
    let encoded_bytes = norito::to_bytes(&env).expect("encode envelope");
    let encoded = base64::engine::general_purpose::STANDARD.encode(&encoded_bytes);
    let body = format!(r#"["{encoded}"]"#);
    let v = post_json_batch_with_limits(
        body,
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        encoded_bytes.len() - 1,
        false,
    )
    .await;
    let statuses = v
        .get("statuses")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].as_bool(), Some(false));

    let label = String::from_utf8(vec![b't', b'o', b'r', b'i', b'i', b'-', 0xc2, 0xb5])
        .expect("valid utf-8 label");
    let env = sample_pallas_envelope(&label);
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(norito::to_bytes(&env).expect("encode envelope"));
    let body = format!(r#"["{encoded}"]"#);
    let v = post_json_batch_with_limits(
        body,
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        usize::MAX,
        true,
    )
    .await;
    let statuses = v
        .get("statuses")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].as_bool(), Some(false));
}

#[tokio::test]
async fn zk_verify_batch_json_applies_open_verify_limits() {
    use base64::Engine as _;

    let env = sample_pallas_envelope("torii-json-open-limits");
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(norito::to_bytes(&env).expect("encode envelope"));
    let body = format!(r#"["{encoded}"]"#);
    let v = post_json_batch_with_limits(
        body,
        iroha_zkp_halo2::OpenVerifyLimits {
            max_k: Some(2),
            max_transcript_label_len: Some(64),
        },
        usize::MAX,
        false,
    )
    .await;

    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    let statuses = v
        .get("statuses")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].as_bool(), Some(false));
}

#[tokio::test]
async fn zk_verify_batch_text_json_content_type_is_accepted() {
    use base64::Engine as _;

    let env = sample_pallas_envelope("torii-text-json");
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(norito::to_bytes(&env).expect("encode envelope"));
    let body = format!(r#"["{encoded}"]"#);
    let v = post_batch_with_limits(
        body.into_bytes(),
        "text/json",
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        16,
        usize::MAX,
        false,
    )
    .await;

    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    let statuses = v
        .get("statuses")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].as_bool(), Some(true));
}

#[tokio::test]
async fn zk_verify_batch_json_rejects_oversized_batch_before_decode() {
    let v = post_batch_with_limits(
        br#"["not base64","also not base64"]"#.to_vec(),
        "application/json",
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        1,
        usize::MAX,
        false,
    )
    .await;

    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));
    assert_eq!(
        v.get("error").and_then(|x| x.as_str()),
        Some("batch_too_large")
    );
    assert_eq!(v.get("max").and_then(|x| x.as_u64()), Some(1));
    assert_eq!(v.get("actual").and_then(|x| x.as_u64()), Some(2));
}

#[tokio::test]
async fn zk_verify_batch_norito_applies_per_entry_diagnostic_limits() {
    let good = sample_pallas_envelope("torii-norito-good");
    let non_ascii_label = String::from_utf8(vec![b't', b'o', b'r', b'i', b'i', b'-', 0xc2, 0xb5])
        .expect("valid utf-8 label");
    let non_ascii = sample_pallas_envelope(&non_ascii_label);
    let body = norito::to_bytes(&vec![good.clone(), non_ascii]).expect("encode batch");

    let v = post_batch_with_limits(
        body,
        "application/x-norito",
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        16,
        usize::MAX,
        true,
    )
    .await;
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    let statuses = v
        .get("statuses")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].as_bool(), Some(true));
    assert_eq!(statuses[1].as_bool(), Some(false));

    let encoded_good = norito::to_bytes(&good).expect("encode envelope");
    let v = post_batch_with_limits(
        norito::to_bytes(&vec![good]).expect("encode batch"),
        "application/x-norito",
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        16,
        encoded_good.len() - 1,
        false,
    )
    .await;
    let statuses = v
        .get("statuses")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].as_bool(), Some(false));
}

#[tokio::test]
async fn zk_verify_batch_returns_false_for_invalid_or_unknown_bodies() {
    let v = post_batch_with_limits(
        b"not norito".to_vec(),
        "application/x-norito",
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        16,
        usize::MAX,
        false,
    )
    .await;
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));
    assert_eq!(
        v.get("statuses").and_then(|x| x.as_array()).map(Vec::len),
        Some(0)
    );

    let v = post_batch_with_limits(
        br#"{"not":"an array"}"#.to_vec(),
        "application/json",
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        16,
        usize::MAX,
        false,
    )
    .await;
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));

    let v = post_batch_with_limits(
        br#"["unterminated""#.to_vec(),
        "application/json",
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        16,
        usize::MAX,
        false,
    )
    .await;
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));

    let v = post_batch_with_limits(
        b"ignored".to_vec(),
        "text/plain",
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        16,
        usize::MAX,
        false,
    )
    .await;
    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));
}

#[tokio::test]
async fn zk_verify_batch_json_accepts_empty_batch() {
    let v = post_json_batch_with_limits(
        "[]".to_owned(),
        iroha_zkp_halo2::OpenVerifyLimits::unbounded(),
        usize::MAX,
        false,
    )
    .await;

    assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(
        v.get("statuses").and_then(|x| x.as_array()).map(Vec::len),
        Some(0)
    );
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
