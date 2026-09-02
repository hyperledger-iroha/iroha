//! Router-level authentication regressions for global and viewer-scoped transaction queries.
#![cfg(feature = "app_api")]

use axum::{
    body::Body,
    extract::connect_info::ConnectInfo,
    http::{Method, Request, StatusCode, header},
};
use iroha_core::state::World;
use iroha_data_model::{
    Registrable,
    account::Account,
    domain::{Domain, DomainId},
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use iroha_torii::filter::QueryEnvelope;
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

fn query_request(path: &'static str, body: Vec<u8>) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .extension(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))))
        .body(Body::from(body))
        .expect("transaction query request")
}

#[tokio::test]
async fn global_transaction_query_requires_operator_auth_but_visible_query_remains_account_scoped()
{
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id).build(&ALICE_ID);
    let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let torii = fixtures::StandardToriiHarness::new(&cfg, World::with([domain], [account], []));
    let app = torii.router();
    let body = norito::json::to_vec(&QueryEnvelope::default()).expect("query envelope JSON");

    let account_request = fixtures::app_signed_request(
        &ALICE_ID,
        &ALICE_KEYPAIR,
        query_request("/v1/transactions/query", body.clone()),
        &body,
    );
    let account_response = app
        .clone()
        .oneshot(account_request)
        .await
        .expect("account-authenticated global query response");
    assert_eq!(account_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        account_response.headers().get(header::WWW_AUTHENTICATE),
        Some(&header::HeaderValue::from_static(
            "IrohaOperatorSignature realm=\"torii\""
        )),
        "canonical account authentication must not authorize the global query"
    );

    let operator_request = fixtures::operator_signed_request(
        &cfg.common.key_pair,
        query_request("/v1/transactions/query", body.clone()),
        &body,
    );
    let operator_response = app
        .clone()
        .oneshot(operator_request)
        .await
        .expect("operator-authenticated global query response");
    assert_ne!(operator_response.status(), StatusCode::UNAUTHORIZED);
    assert_ne!(operator_response.status(), StatusCode::FORBIDDEN);

    let visible_response = app
        .router()
        .oneshot(query_request("/v1/transactions/visible/query", body))
        .await
        .expect("missing account-auth visible query response");
    assert_eq!(visible_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        visible_response.headers().get(header::WWW_AUTHENTICATE),
        Some(&header::HeaderValue::from_static("Signature")),
        "viewer-scoped query must retain canonical account authentication"
    );
    app.shutdown().await;
}
