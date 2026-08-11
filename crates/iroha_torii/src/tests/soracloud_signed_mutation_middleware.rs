//! Tests for SoraCloud signed-mutation request admission.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    routing::post,
};
use tower::ServiceExt as _;

use super::*;

#[tokio::test]
async fn soracloud_mutation_still_enforces_typed_accept() {
    let handler_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&handler_calls);
    let router = Router::new()
        .route(
            route_catalog::application_api::SORACLOUD_DEPLOY_POST.path(),
            post(move || {
                let calls = Arc::clone(&calls);
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    StatusCode::NO_CONTENT
                }
            }),
        )
        .layer(axum::middleware::from_fn_with_state(
            crate::mk_app_state_for_tests(),
            enforce_soracloud_signed_mutation_request,
        ));

    let response = router
        .oneshot(
            Request::builder()
                .method(axum::http::Method::POST)
                .uri(route_catalog::application_api::SORACLOUD_DEPLOY_POST.path())
                .header(header::ACCEPT, "text/event-stream")
                .body(Body::empty())
                .expect("SoraCloud request"),
        )
        .await
        .expect("SoraCloud response");

    assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
    assert_eq!(handler_calls.load(Ordering::SeqCst), 0);
}
