// Included by `da::ingest::tests`; keeping this test here preserves its
// original module path while separating error-envelope negotiation coverage.

#[tokio::test]
async fn da_ingest_error_response_negotiates_error_envelopes() {
    let (parts, body) = build_error_response(
        StatusCode::BAD_REQUEST,
        "bad payload",
        ResponseFormat::Norito,
    )
    .into_parts();
    assert_eq!(parts.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        parts.headers.get(axum::http::header::CONTENT_TYPE),
        Some(&axum::http::HeaderValue::from_static(
            crate::utils::NORITO_MIME_TYPE
        ))
    );
    let bytes = body
        .collect()
        .await
        .expect("collect Norito body")
        .to_bytes();
    let envelope: iroha_torii_shared::ErrorEnvelope =
        norito::decode_from_bytes(&bytes).expect("decode Norito error envelope");
    assert_eq!(envelope.code, "bad_request");
    assert_eq!(envelope.message, "bad payload");

    let (parts, body) =
        build_error_response(StatusCode::CONFLICT, "duplicate", ResponseFormat::Json).into_parts();
    assert_eq!(parts.status, StatusCode::CONFLICT);
    assert_eq!(
        parts.headers.get(axum::http::header::CONTENT_TYPE),
        Some(&axum::http::HeaderValue::from_static("application/json"))
    );
    let bytes = body.collect().await.expect("collect JSON body").to_bytes();
    let envelope: iroha_torii_shared::ErrorEnvelope =
        json::from_slice(&bytes).expect("decode JSON error envelope");
    assert_eq!(envelope.code, "conflict");
    assert_eq!(envelope.message, "duplicate");
}
