#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Basic shape test for `NEW_VIEW` SSE endpoint
#![allow(unexpected_cfgs)]

#[tokio::test]
async fn new_view_sse_content_type() {
    use axum::{Router, routing::get};
    use tower::ServiceExt;

    // Router with SSE endpoint, using minimal poll interval
    let app = Router::new().route(
        "/v2/sumeragi/new_view/sse",
        get(|| async move { iroha_torii::handle_v1_new_view_sse(200) }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v2/sumeragi/new_view/sse")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let ct = resp
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("text/event-stream"));
}
