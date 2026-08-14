use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    routing::get,
};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;
use super::*;
fn test_router(counter: Arc<AtomicUsize>) -> Router {
    Router::new()
        .route(
            "/v1/files/{*tail}",
            get(move || {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    StatusCode::NO_CONTENT
                }
            }),
        )
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(axum::middleware::from_fn(enforce_strict_request_target))
}
fn sorafs_test_router(counter: Arc<AtomicUsize>) -> Router {
    let mount = |counter: Arc<AtomicUsize>| {
        get(move || {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                StatusCode::NO_CONTENT
            }
        })
    };
    Router::new()
        .route(
            route_catalog::sorafs::CID_ROOT.path(),
            mount(Arc::clone(&counter)),
        )
        .route(
            route_catalog::sorafs::CID_PATH.path(),
            mount(Arc::clone(&counter)),
        )
        .route(
            route_catalog::sorafs::REPUTATION_EVENTS_WEBSOCKET.path(),
            mount(counter),
        )
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(axum::middleware::from_fn(enforce_strict_request_target))
}
fn offline_operation_test_router(counter: Arc<AtomicUsize>) -> Router {
    Router::new()
        .route(
            route_catalog::offline::OPERATION.path(),
            get(move || {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    StatusCode::NO_CONTENT
                }
            }),
        )
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(axum::middleware::from_fn(enforce_strict_request_target))
}
fn governance_selector_test_router(counter: Arc<AtomicUsize>) -> Router {
    let mount = |counter: Arc<AtomicUsize>| {
        get(
            move |axum::extract::Path(selector): axum::extract::Path<String>| {
                let counter = Arc::clone(&counter);
                async move {
                    if !iroha_data_model::governance::is_valid_governance_selector_v1(&selector) {
                        return StatusCode::BAD_REQUEST;
                    }
                    counter.fetch_add(1, Ordering::SeqCst);
                    StatusCode::NO_CONTENT
                }
            },
        )
    };
    Router::new()
        .route("/v1/gov/locks/{id}", mount(Arc::clone(&counter)))
        .route("/v1/gov/referenda/{id}", mount(Arc::clone(&counter)))
        .route("/v1/gov/tally/{id}", mount(counter))
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(axum::middleware::from_fn(enforce_strict_request_target))
}
fn catalog_cutover_test_router(counter: Arc<AtomicUsize>) -> Router {
    let mount = |counter: Arc<AtomicUsize>| {
        axum::routing::post(move || {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                StatusCode::NO_CONTENT
            }
        })
    };
    Router::new()
        .route(
            route_catalog::contracts_and_verification_keys::MULTISIG_PROPOSALS_QUERY_POST.path(),
            mount(Arc::clone(&counter)),
        )
        .route(
            route_catalog::contracts_and_verification_keys::MULTISIG_PROPOSALS_RESOLVE_POST.path(),
            mount(Arc::clone(&counter)),
        )
        .route(
            route_catalog::contracts_and_verification_keys::CONTROLS_ASSET_TRANSFER_QUERY_POST
                .path(),
            mount(counter),
        )
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(axum::middleware::from_fn(enforce_strict_request_target))
}
#[tokio::test]
async fn normalization_sequences_are_typed_bad_requests_before_handler_execution() {
    let counter = Arc::new(AtomicUsize::new(0));
    let router = test_router(Arc::clone(&counter));
    for path in [
        "/v1/files//secret",
        "/v1/files/%2fadmin",
        "/v1/files/%2Fadmin",
        "/v1/files/%5cadmin",
        "/v1/files/%5Cadmin",
        "/v1/files/./admin",
        "/v1/files/%2e%2E/admin",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::ACCEPT, "application/json")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "path={path}");
        assert_eq!(
            response.headers().get(header::VARY),
            Some(&HeaderValue::from_static("Accept")),
            "path={path}"
        );
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect response")
            .to_bytes();
        let envelope: ErrorEnvelope = norito::json::from_slice(&body).expect("typed JSON error");
        assert_eq!(envelope.code(), "request_path_invalid", "path={path}");
    }
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn trailing_slash_and_empty_wildcard_tail_do_not_alias_resources() {
    let counter = Arc::new(AtomicUsize::new(0));
    let router = test_router(Arc::clone(&counter));
    for path in ["/v1/files/", "/v1/files/bundle/"] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::ACCEPT, "application/x-norito")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "path={path}");
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect response")
            .to_bytes();
        let envelope: ErrorEnvelope = norito::decode_from_bytes(&body).expect("typed Norito error");
        assert_eq!(envelope.code(), "route_not_found", "path={path}");
    }
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    let response = router
        .oneshot(
            Request::builder()
                .uri("/v1/files/bundle/object")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn offline_operation_id_rejects_percent_encoded_alias_before_handler_execution() {
    let counter = Arc::new(AtomicUsize::new(0));
    let router = offline_operation_test_router(Arc::clone(&counter));
    let canonical_id = "11".repeat(32);
    let encoded_id = format!("%31{}", &canonical_id[1..]);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/offline/operations/{encoded_id}"))
                .header(header::ACCEPT, "application/json")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect response")
        .to_bytes();
    let envelope: ErrorEnvelope = norito::json::from_slice(&body).expect("typed JSON error");
    assert_eq!(envelope.code(), "request_path_invalid");
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    let response = router
        .oneshot(
            Request::builder()
                .uri(format!("/v1/offline/operations/{canonical_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn governance_selectors_are_canonical_mounted_path_segments_before_lookup() {
    let counter = Arc::new(AtomicUsize::new(0));
    let router = governance_selector_test_router(Arc::clone(&counter));
    let overlong = "a"
        .repeat(iroha_data_model::governance::GOVERNANCE_SELECTOR_V1_MAX_BYTES.saturating_add(1));
    let invalid_paths = [
        "/v1/gov/referenda/a/b".to_owned(),
        "/v1/gov/referenda/a%2Fb".to_owned(),
        "/v1/gov/referenda/.".to_owned(),
        "/v1/gov/referenda/..".to_owned(),
        "/v1/gov/referenda/.hidden".to_owned(),
        "/v1/gov/referenda/%72eferendum".to_owned(),
        "/v1/gov/referenda/%E6%8A%95%E7%A5%A8".to_owned(),
        "/v1/gov/referenda/ref%201".to_owned(),
        format!("/v1/gov/referenda/{overlong}"),
    ];
    for path in invalid_paths {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&path)
                    .header(header::ACCEPT, "application/json")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert!(!response.status().is_success(), "path={path}");
    }
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(axum::http::Method::HEAD)
                .uri("/v1/gov/referenda/%72eferendum")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        counter.load(Ordering::SeqCst),
        0,
        "noncanonical selectors must not reach state lookup"
    );
    for path in [
        "/v1/gov/referenda/ref-1",
        "/v1/gov/tally/A9_selector~with.dots",
        "/v1/gov/locks/lock_1",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NO_CONTENT, "path={path}");
    }
    assert_eq!(
        counter.load(Ordering::SeqCst),
        3,
        "canonical selectors must reach state lookup"
    );
}
#[tokio::test]
async fn sorafs_root_and_stream_paths_reject_retired_and_normalized_aliases() {
    let counter = Arc::new(AtomicUsize::new(0));
    let router = sorafs_test_router(Arc::clone(&counter));
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/sorafs/cid/bafyroot")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    for (path, expected) in [
        ("/ws/reputation", StatusCode::NOT_FOUND),
        ("/sorafs/cid/bafyroot/", StatusCode::NOT_FOUND),
        ("/sorafs//cid/bafyroot", StatusCode::BAD_REQUEST),
        ("/sorafs/cid/bafyroot/%2fsecret", StatusCode::BAD_REQUEST),
        ("/sorafs/cid/bafyroot/%5Csecret", StatusCode::BAD_REQUEST),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::ACCEPT, "application/json")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), expected, "path={path}");
    }
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "rejected aliases must not execute a SoraFS handler"
    );
    for retired_path in [
        "/v1/sorafs/deal/fund-provider",
        "/v1/sorafs/deal/fund-client",
        "/v1/sorafs/deal/open",
        "/v1/sorafs/deal/cancel",
        "/v1/sorafs/deal/usage",
        "/v1/sorafs/deal/settle",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(retired_path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "retired process-local deal path={retired_path}"
        );
    }
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "retired deal requests must not execute any SoraFS handler"
    );
    let response = router
        .oneshot(
            Request::builder()
                .uri("/v1/sorafs/reputation/events/ws")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn canonical_multisig_read_routes_reject_only_retired_spellings() {
    let counter = Arc::new(AtomicUsize::new(0));
    let router = catalog_cutover_test_router(Arc::clone(&counter));
    for retired_path in [
        "/v1/multisig/proposals/lookup",
        "/v1/multisig/proposals/list",
        "/v1/multisig/proposals/get",
        "/v1/multisig/proposals/search",
        "/v1/multisig/approvals/list",
        "/v1/multisig/approvals/get",
        "/v1/multisig/approvals/list_for_authority",
        "/v1/multisig/approvals/get_for_authority",
        "/v1/multisig/approvals/query",
        "/v1/multisig/approvals/lookup",
        "/v1/multisig/approvals/query-for-authority",
        "/v1/multisig/approvals/lookup-for-authority",
        "/v1/controls/asset-transfer/get",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(retired_path)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{retired_path}");
    }
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    for canonical_path in [
        "/v1/multisig/proposals/query",
        "/v1/multisig/proposals/resolve",
        "/v1/controls/asset-transfer/query",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(canonical_path)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(
            response.status(),
            StatusCode::NO_CONTENT,
            "{canonical_path}"
        );
    }
    assert_eq!(counter.load(Ordering::SeqCst), 3);
    for adversarial_path in [
        "/v1/multisig/proposals//query",
        "/v1/multisig/proposals/%2fquery",
        "/v1/multisig/proposals//resolve",
        "/v1/multisig/proposals/%2fresolve",
        "/v1/multisig/proposals/query/",
        "/v1/multisig/proposals/resolve/",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(adversarial_path)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert!(
            matches!(
                response.status(),
                StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
            ),
            "{adversarial_path}"
        );
    }
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}
