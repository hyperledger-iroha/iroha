//! Public operator faucet policy is discoverable before account registration and leaks no custody.

use super::*;
use iroha_torii_shared::account_faucet_policy::{
    ACCOUNT_FAUCET_POLICY_MAX_BYTES, AccountFaucetAdvertisement,
};

#[tokio::test]
async fn faucet_policy_discovery_is_unsigned_exact_and_read_only() {
    let context = build_faucet_test_context_with_registration(false, None, false);
    assert!(
        context
            .state
            .world_view()
            .account(&context.user_id)
            .is_err()
    );
    let height = context.state.committed_height();
    let mut request = Request::builder()
        .method("GET")
        .uri("/v1/accounts/faucet/policy")
        .body(axum::body::Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            40000,
        ))));
    let response = context.app.clone().oneshot(request).await.unwrap();
    let response = expect_status(response, StatusCode::OK).await;
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json; charset=utf-8"
    );
    let bytes = to_bytes(response.into_body(), ACCOUNT_FAUCET_POLICY_MAX_BYTES)
        .await
        .unwrap();
    let policy: AccountFaucetAdvertisement = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(policy.schema_version, 1);
    assert_eq!(policy.network_id, *context.state.network_id_ref());
    assert_eq!(
        policy.network_prefix,
        iroha_data_model::account::address::chain_discriminant()
    );
    assert_eq!(policy.authority, context.authority_id);
    assert_eq!(policy.asset_definition_id, context.asset_definition_id);
    assert_eq!(policy.amount.to_string(), "25000");
    let json: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 6);
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!text.contains("private_key"));
    assert!(!text.contains("token"));
    assert_eq!(context.state.committed_height(), height);
    assert!(
        context
            .state
            .world_view()
            .account(&context.user_id)
            .is_err()
    );
    context.app.shutdown().await;
}

#[tokio::test]
async fn faucet_policy_discovery_rejects_query_and_body() {
    let context = build_faucet_test_context(false);
    for (path, body) in [
        ("/v1/accounts/faucet/policy?authority=other", ""),
        ("/v1/accounts/faucet/policy", "{}"),
    ] {
        let mut request = Request::builder()
            .method("GET")
            .uri(path)
            .body(axum::body::Body::from(body))
            .unwrap();
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                40000,
            ))));
        let response = context.app.clone().oneshot(request).await.unwrap();
        assert!(
            response.status().is_client_error(),
            "{path}: {}",
            response.status()
        );
    }
    context.app.shutdown().await;
}
