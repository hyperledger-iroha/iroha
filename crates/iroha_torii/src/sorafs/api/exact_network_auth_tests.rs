// Exact-network SoraFS HTTP authentication helpers and regression included by the parent tests.

fn signed_app_headers(
    account: &AccountId,
    key_pair: &KeyPair,
    method: &Method,
    uri: &Uri,
    body: &[u8],
) -> HeaderMap {
    signed_network_app_headers(
        &crate::signed_query_test_network_id(),
        account,
        key_pair,
        method,
        uri,
        body,
    )
}

fn reputation_request_keypair() -> KeyPair {
    KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519)
        .expect("derive reputation request authentication key")
}

fn reputation_signed_get_headers(uri: &Uri, body: &[u8]) -> HeaderMap {
    let keypair = reputation_request_keypair();
    let account = AccountId::new(keypair.public_key().clone());
    signed_app_headers(&account, &keypair, &Method::GET, uri, body)
}

fn reputation_auth_test_guard() -> impl Drop {
    crate::tests_runtime_handlers::app_auth_test_guard(
        crate::app_auth::CanonicalRequestAuthConfig::default(),
    )
}

#[test]
fn sorafs_control_auth_helpers_reject_foreign_exact_network() {
    let _guard = crate::tests_runtime_handlers::app_auth_test_guard(
        crate::app_auth::CanonicalRequestAuthConfig::default(),
    );
    let method = Method::GET;
    let uri = Uri::from_static("/v1/sorafs/reputation/latest");
    let (state, headers) = crate::tests_runtime_handlers::foreign_network_signed_app_fixture(
        &method,
        &uri,
        &[],
        0xD5,
        0xE5,
    );

    let reputation = match require_reputation_canonical_auth(&state, &headers, &method, &uri, &[]) {
        Ok(()) => panic!("foreign-network reputation auth must fail closed"),
        Err(response) => response,
    };
    assert_eq!(reputation.status(), StatusCode::UNAUTHORIZED);

    for response in [
        require_moderation_request_auth(&state, &headers, &method, &uri, &[], None),
        require_appeal_finance_request_auth(&state, &headers, &method, &uri, &[]),
        require_transparency_source_request_auth(&state, &headers, &method, &uri, &[]),
    ] {
        let response = match response {
            Ok(_) => panic!("foreign-network SoraFS control auth must fail closed"),
            Err(response) => response,
        };
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
