// SCCP request-authentication fixtures shared by the split runtime-handler tests.

fn sccp_ingress_auth_fixture() -> (AccountId, KeyPair) {
    let key_pair = checked_torii_test_keypair_from_seed_byte(
        0xC7,
        Algorithm::Ed25519,
        "derive SCCP ingress account fixture key",
    );
    let account = AccountId::new(key_pair.public_key().clone());
    (account, key_pair)
}

fn seed_sccp_ingress_auth_account(app: &SharedAppState) {
    let (account, _) = sccp_ingress_auth_fixture();
    if app.state.view().world().account(&account).is_ok() {
        return;
    }
    let next_height = app
        .state
        .latest_block_header_fast()
        .map_or(1, |header| header.height().get().saturating_add(1));
    let header = BlockHeader::new(
        NonZeroU64::new(next_height).expect("non-zero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = app.state.block(header);
    let mut transaction = block.transaction();
    Register::account(Account::new(account))
        .execute(&ALICE_ID, &mut transaction)
        .expect("register SCCP ingress authentication fixture account");
    transaction.apply();
    block.commit().expect("commit SCCP ingress fixture account");
}

fn authenticate_sccp_ingress_request(
    request: &mut axum::http::Request<axum::body::Body>,
    body: &[u8],
) {
    let (account, key_pair) = sccp_ingress_auth_fixture();
    let headers = signed_network_app_headers(
        &signed_query_test_network_id(),
        &account,
        &key_pair,
        request.method(),
        request.uri(),
        body,
    );
    for (name, value) in &headers {
        request.headers_mut().insert(name.clone(), value.clone());
    }
}
