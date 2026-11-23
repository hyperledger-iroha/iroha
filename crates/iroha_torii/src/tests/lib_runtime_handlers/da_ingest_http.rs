/// Exercise signed-account admission and request bounds on the mounted DA routes.
#[cfg(feature = "app_api")]
#[tokio::test]
async fn mounted_da_ingest_auth_and_size_errors_are_private_and_negotiated() {
    use axum::{
        body::{Body, Bytes},
        http::{Request, header},
    };
    use tower::ServiceExt as _;

    const CHAIN: &str = "da-ingest-http-rejections";
    let mut body_limit = 0;
    let (kura, state) = RuntimeApiRouterFixture::native_state(CHAIN, World::default());
    let fixture = RuntimeApiRouterFixture::with_configured_runtime(
        CHAIN,
        kura,
        state,
        ToriiRuntimeDeps::new(
            crate::build_identity_test_fixture::build_identity(),
            routing::MaybeTelemetry::disabled(),
        ),
        |cfg| {
            body_limit =
                usize::try_from(cfg.torii.max_content_len.get()).expect("fixture body limit")
        },
    );
    let router = fixture.router.router();
    let oversized = Bytes::from(vec![b' '; body_limit + 1]);
    for path in ["/v1/da/ingest", "/v1/da/ingest/parts"] {
        for (body, status, code) in [
            (
                Bytes::from_static(b"{}"),
                StatusCode::UNAUTHORIZED,
                "canonical_authentication_required",
            ),
            (
                oversized.clone(),
                StatusCode::PAYLOAD_TOO_LARGE,
                "request_payload_too_large",
            ),
        ] {
            for (accept, content_type) in [
                ("application/json", "application/json; charset=utf-8"),
                (
                    crate::utils::NORITO_MIME_TYPE,
                    crate::utils::NORITO_MIME_TYPE,
                ),
            ] {
                let request = Request::post(path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::ACCEPT, accept)
                    .body(Body::from(body.clone()))
                    .expect("unsigned DA request");
                let response = router.clone().oneshot(request).await.expect("DA response");
                assert_eq!(response.status(), status, "{path} {accept}");
                assert_eq!(
                    torii_response_header(&response, "cache-control"),
                    Some("private, no-store"),
                    "{path} {status} {accept}"
                );
                assert_eq!(
                    torii_response_header(&response, "content-type"),
                    Some(content_type),
                    "{path} {status} {accept}"
                );
                let bytes = axum::body::to_bytes(response.into_body(), 4096)
                    .await
                    .expect("bounded DA rejection");
                let envelope: crate::ErrorEnvelope = if accept == "application/json" {
                    norito::json::from_slice(&bytes).expect("JSON error envelope")
                } else {
                    norito::decode_from_bytes(&bytes).expect("Norito error envelope")
                };
                assert_eq!(envelope.code(), code, "{path} {status} {accept}");
            }
        }
    }
    fixture.shutdown().await;
}
