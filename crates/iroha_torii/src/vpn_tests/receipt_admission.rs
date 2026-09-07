//! Receipt admission and cache-loss adversarial tests.

use super::*;

#[tokio::test]
async fn submit_vpn_receipt_allows_expired_session_within_wsv_grace() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let user_keys = checked_vpn_ed25519_keypair(0x74);
    let operator_keys = checked_vpn_ed25519_keypair(0x75);
    let user = account_id_for(&user_keys);
    let operator = account_id_for(&operator_keys);
    let app = vpn_enabled_app_with_operator(
        world_with_accounts(&[user.clone(), operator.clone()]),
        &operator,
    );
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &user, &user_keys, "standard").await;
    let mut lease_record = active_lease_record_from_quote(&quote);
    let expires_at_ms = now_ms().saturating_sub(1_000);
    let issued_at_ms = expires_at_ms.saturating_sub(1);
    let opened_at_ms = issued_at_ms;
    lease_record.opened_at_ms = opened_at_ms;
    lease_record.expires_at_ms = expires_at_ms;
    lease_record.settlement_grace_ms = 60_000;
    resign_lease_quote_projection(&mut lease_record);
    let active_record = session_record_from_lease(&lease_record).expect("expired lease projection");
    let session = response_from_record(&active_record);
    let mut fixture = receipt_fixture_for_session(&session, &active_record, &user, &metering_keys);
    let mut voucher_body = fixture.voucher.body;
    voucher_body.issued_at_ms = issued_at_ms;
    fixture.voucher = VpnUsageVoucherV1::try_sign(voucher_body, metering_keys.private_key())
        .expect("re-sign within-grace fixture voucher");
    fixture.relay_receipt.receipt.started_at_ms = opened_at_ms;
    fixture.relay_receipt.receipt.ended_at_ms = issued_at_ms;
    fixture.relay_receipt.receipt.client_voucher_hash = fixture.voucher.hash();
    resign_test_relay_receipt(&mut fixture.relay_receipt);
    fixture.body = receipt_submit_body(&fixture.relay_receipt, &fixture.voucher);
    app.state.insert_vpn_lease_for_testing(lease_record);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(
        &operator,
        &operator_keys,
        &method,
        &uri,
        fixture.body.as_ref(),
    );
    let response =
        handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, fixture.body.as_ref())
            .await
            .expect("settled within grace")
            .into_response();
    assert_eq!(response.status(), StatusCode::CREATED);
    let pending: VpnReceiptResponseDto = read_json(response).await;
    assert_eq!(pending.status, "settlement_pending");
    assert_eq!(pending.earned_fee, fixture.earned_fee);
    assert_eq!(pending.lease_id_hex, hex::encode(fixture.lease_id));
    assert!(pending.settle_lease_instruction.is_some());
    assert!(app.vpn_receipts.is_empty());
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_after_wsv_grace() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let user_keys = checked_vpn_ed25519_keypair(0x76);
    let operator_keys = checked_vpn_ed25519_keypair(0x77);
    let user = account_id_for(&user_keys);
    let operator = account_id_for(&operator_keys);
    let app = vpn_enabled_app_with_operator(
        world_with_accounts(&[user.clone(), operator.clone()]),
        &operator,
    );
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &user, &user_keys, "standard").await;
    let mut lease_record = active_lease_record_from_quote(&quote);
    lease_record.expires_at_ms = now_ms().saturating_sub(10_000);
    lease_record.opened_at_ms = lease_record.expires_at_ms.saturating_sub(10_000);
    lease_record.settlement_grace_ms = 1;
    resign_lease_quote_projection(&mut lease_record);
    let active_record = session_record_from_lease(&lease_record).expect("expired lease projection");
    let session = response_from_record(&active_record);
    let fixture = receipt_fixture_for_session(&session, &active_record, &user, &metering_keys);
    app.state.insert_vpn_lease_for_testing(lease_record);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(
        &operator,
        &operator_keys,
        &method,
        &uri,
        fixture.body.as_ref(),
    );
    let error =
        handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, fixture.body.as_ref())
            .await
            .expect_err("settlement after grace must fail");
    assert!(format!("{error:?}").contains("grace window expired"));
}
#[tokio::test]
async fn unrelated_account_cannot_reserve_vpn_settlement_capacity_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let unrelated_keys = checked_vpn_ed25519_keypair(0xA4);
    let unrelated = account_id_for(&unrelated_keys);
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture_with_additional_accounts(std::slice::from_ref(&unrelated)).await;
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(
        &unrelated,
        &unrelated_keys,
        &method,
        &uri,
        fixture.body.as_ref(),
    );
    let error =
        handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, fixture.body.as_ref())
            .await
            .expect_err("unrelated receipt signer must fail");
    assert!(format!("{error:?}").contains("configured operator account"));
    {
        let state = app
            .vpn_state_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(state.settlement_reservations, 0);
        assert!(state.settling_session_ids.is_empty());
    }

    let headers = signed_app_headers(
        &operator,
        &operator_keys,
        &method,
        &uri,
        fixture.body.as_ref(),
    );
    let response =
        handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, fixture.body.as_ref())
            .await
            .expect("configured operator can reserve and prepare settlement")
            .into_response();
    assert_eq!(response.status(), StatusCode::CREATED);
    let state = app
        .vpn_state_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(state.settlement_reservations, 1);
    assert!(state.settling_session_ids.is_empty());
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_tampered_relay_signature_before_admission() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.ingress_bytes = relay_receipt.receipt.ingress_bytes.saturating_add(1);
    let body = receipt_submit_body(&relay_receipt, &fixture.voucher);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());
    let error = handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("tampered relay receipt signature must fail");
    assert!(format!("{error:?}").contains("relay receipt signature verification failed"));
    let state = app
        .vpn_state_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(state.settlement_reservations, 0);
    assert!(
        state.settling_session_ids.is_empty(),
        "invalid relay signatures must fail before reserving settlement state"
    );
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_exact_signed_request_replay() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(
        &operator,
        &operator_keys,
        &method,
        &uri,
        fixture.body.as_ref(),
    );
    let first =
        handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, fixture.body.as_ref())
            .await
            .expect("first settlement")
            .into_response();
    assert_eq!(first.status(), StatusCode::CREATED);
    let replay = handle_submit_vpn_receipt(app, &method, &uri, &headers, fixture.body.as_ref())
        .await
        .expect_err("exact request replay must fail");
    assert!(format!("{replay:?}").contains("nonce already used"));
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_explicit_lease_id_for_different_active_lease() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let user_keys = checked_vpn_ed25519_keypair(0x78);
    let other_user_keys = checked_vpn_ed25519_keypair(0x79);
    let operator_keys = checked_vpn_ed25519_keypair(0x7A);
    let user = account_id_for(&user_keys);
    let other_user = account_id_for(&other_user_keys);
    let operator = account_id_for(&operator_keys);
    let app = vpn_enabled_app_with_operator(
        world_with_accounts(&[user.clone(), other_user.clone(), operator.clone()]),
        &operator,
    );
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &user, &user_keys, "standard").await;
    let session =
        create_session_for_quote(app.clone(), &user, &user_keys, &quote, &metering_keys).await;
    let active_record = app
        .vpn_sessions
        .get(&session.session_id)
        .expect("active session")
        .clone();
    let fixture = receipt_fixture_for_session(&session, &active_record, &user, &metering_keys);
    app.state
        .insert_vpn_lease_for_testing(lease_record_from_session_record(
            &active_record,
            VpnLeaseStatusV1::Active,
            None,
        ));
    let (other_quote, other_metering_keys) =
        create_quote_for_account(app.clone(), &other_user, &other_user_keys, "standard").await;
    let other_session = create_session_for_quote(
        app.clone(),
        &other_user,
        &other_user_keys,
        &other_quote,
        &other_metering_keys,
    )
    .await;
    let other_record = app
        .vpn_sessions
        .get(&other_session.session_id)
        .expect("other active session")
        .clone();
    app.state
        .insert_vpn_lease_for_testing(lease_record_from_session_record(
            &other_record,
            VpnLeaseStatusV1::Active,
            None,
        ));
    app.vpn_sessions.clear();
    let body = receipt_submit_body_with_lease_id(
        &fixture.relay_receipt,
        &fixture.voucher,
        hex::encode(other_record.lease_id),
    );
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());
    let error = handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("explicit mismatched lease id must fail");
    assert!(format!("{error:?}").contains("consensus-indexed VPN session"));
    assert!(app.vpn_receipts.is_empty());
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_wrong_metering_key_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let wrong_metering_keys = checked_vpn_ed25519_keypair(0x7B);
    let voucher =
        VpnUsageVoucherV1::try_sign(fixture.voucher.body, wrong_metering_keys.private_key())
            .expect("checked wrong-metering-key voucher");
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.client_voucher_hash = voucher.hash();
    resign_test_relay_receipt(&mut relay_receipt);
    let body = receipt_submit_body(&relay_receipt, &voucher);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());
    let error = handle_submit_vpn_receipt(app, &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("wrong metering key must fail");
    assert!(format!("{error:?}").contains("public key does not match"));
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_relay_earned_fee_inflation_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.earned_fee = fixture
        .earned_fee
        .checked_add(&Quantity::one())
        .expect("tampered earned fee remains representable");
    resign_test_relay_receipt(&mut relay_receipt);
    let body = receipt_submit_body(&relay_receipt, &fixture.voucher);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());
    let error = handle_submit_vpn_receipt(app, &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("inflated earned fee must fail");
    assert!(format!("{error:?}").contains("earned fee does not match"));
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_voucher_hash_substitution_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut voucher = fixture.voucher.clone();
    voucher.body.sequence = voucher.body.sequence.saturating_add(1);
    voucher = VpnUsageVoucherV1::try_sign(voucher.body, metering_keys.private_key())
        .expect("checked changed voucher");
    let body = receipt_submit_body(&fixture.relay_receipt, &voucher);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());
    let error = handle_submit_vpn_receipt(app, &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("voucher substitution must fail");
    assert!(format!("{error:?}").contains("does not commit"));
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_payment_hash_substitution_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.payment_tx_hash[0] ^= 0x01;
    resign_test_relay_receipt(&mut relay_receipt);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &fixture.voucher,
        "payment hash does not match",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_account_hash_substitution_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.account_hash[0] ^= 0x01;
    resign_test_relay_receipt(&mut relay_receipt);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &fixture.voucher,
        "account hash does not match",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_relay_id_substitution_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let wrong_relay_key = checked_vpn_ed25519_keypair(0x54);
    let (_, wrong_relay_public_key) = wrong_relay_key
        .public_key()
        .try_to_bytes()
        .expect("wrong relay fixture public key");
    let mut receipt_body = fixture.relay_receipt.receipt;
    receipt_body
        .relay_id
        .copy_from_slice(wrong_relay_public_key);
    let relay_receipt =
        VpnSignedSessionReceiptV1::try_sign(receipt_body, wrong_relay_key.private_key())
            .expect("wrong relay fixture signs its own receipt identity");
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &fixture.voucher,
        "relay id does not match",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_usage_beyond_prepaid_ceiling_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let authorized_ingress_bytes = fixture.voucher.body.ingress_bytes;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.ingress_bytes = authorized_ingress_bytes.saturating_add(1);
    resign_test_relay_receipt(&mut relay_receipt);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &fixture.voucher,
        "exceeds the submitted prepaid voucher ceilings",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_noncanonical_uptime_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.uptime_secs = 1;
    resign_test_relay_receipt(&mut relay_receipt);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &fixture.voucher,
        "uptime must equal",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_inverted_interval_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.started_at_ms = 10_000;
    relay_receipt.receipt.ended_at_ms = 9_999;
    resign_test_relay_receipt(&mut relay_receipt);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &fixture.voucher,
        "service interval is inverted",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_cover_telemetry_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.cover_bytes = 1;
    resign_test_relay_receipt(&mut relay_receipt);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &fixture.voucher,
        "must not carry unauthenticated cover telemetry",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_uncommitted_meter_hash_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.meter_hash[0] ^= 1;
    resign_test_relay_receipt(&mut relay_receipt);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &fixture.voucher,
        "meter hash does not match",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_voucher_signature_tamper_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut voucher = fixture.voucher.clone();
    voucher.body.issued_at_ms = voucher.body.issued_at_ms.saturating_add(1);
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.client_voucher_hash = voucher.hash();
    resign_test_relay_receipt(&mut relay_receipt);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &voucher,
        "signature failed",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_voucher_sequence_mismatch_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.highest_voucher_sequence = relay_receipt
        .receipt
        .highest_voucher_sequence
        .saturating_add(1);
    resign_test_relay_receipt(&mut relay_receipt);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &fixture.voucher,
        "voucher sequence does not match",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_receipt_session_id_mismatch_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.session_id[0] ^= 0x01;
    resign_test_relay_receipt(&mut relay_receipt);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &fixture.voucher,
        "active consensus-indexed VPN session",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_voucher_session_id_mismatch_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut voucher = fixture.voucher.clone();
    voucher.body.session_id[0] ^= 0x01;
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &fixture.relay_receipt,
        &voucher,
        "session id does not match",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_receipt_quote_id_mismatch_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.quote_id[0] ^= 0x01;
    resign_test_relay_receipt(&mut relay_receipt);
    let body = receipt_submit_body_with_lease_id(
        &relay_receipt,
        &fixture.voucher,
        hex::encode(fixture.lease_id),
    );
    submit_receipt_body_expect_error(
        app,
        &operator,
        &operator_keys,
        body,
        "quote id does not match",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_voucher_quote_id_mismatch_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut voucher = fixture.voucher.clone();
    voucher.body.quote_id[0] ^= 0x01;
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &fixture.relay_receipt,
        &voucher,
        "quote id does not match",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_voucher_relay_id_mismatch_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut voucher = fixture.voucher.clone();
    voucher.body.relay_id[0] ^= 0x01;
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &fixture.relay_receipt,
        &voucher,
        "relay id does not match",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_malformed_relay_receipt_hex() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let body = norito::json::to_vec(&VpnReceiptSubmitRequestDto {
        relay_receipt_hex: "not-hex".to_owned(),
        client_voucher_hex: hex::encode(fixture.voucher.encode()),
        lease_id_hex: String::new(),
    })
    .expect("receipt request");
    submit_receipt_body_expect_error(app, &operator, &operator_keys, body, "relay_receipt_hex")
        .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_malformed_client_voucher_hex() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let body = norito::json::to_vec(&VpnReceiptSubmitRequestDto {
        relay_receipt_hex: hex::encode(fixture.relay_receipt.encode()),
        client_voucher_hex: "not-hex".to_owned(),
        lease_id_hex: String::new(),
    })
    .expect("receipt request");
    submit_receipt_body_expect_error(app, &operator, &operator_keys, body, "client_voucher_hex")
        .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_client_voucher_trailing_norito_bytes() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut encoded_voucher = fixture.voucher.encode();
    encoded_voucher.push(0);
    let body = norito::json::to_vec(&VpnReceiptSubmitRequestDto {
        relay_receipt_hex: hex::encode(fixture.relay_receipt.encode()),
        client_voucher_hex: hex::encode(encoded_voucher),
        lease_id_hex: String::new(),
    })
    .expect("receipt request");
    submit_receipt_body_expect_error(
        app,
        &operator,
        &operator_keys,
        body,
        "client_voucher_hex is not valid Norito",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_explicit_lease_id_wrong_length() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let body = receipt_submit_body_with_lease_id(
        &fixture.relay_receipt,
        &fixture.voucher,
        "aa".to_owned(),
    );
    submit_receipt_body_expect_error(
        app,
        &operator,
        &operator_keys,
        body,
        "lease_id_hex must decode to 32 bytes",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_explicit_lease_id_non_hex() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let body = receipt_submit_body_with_lease_id(
        &fixture.relay_receipt,
        &fixture.voucher,
        "not-hex".to_owned(),
    );
    submit_receipt_body_expect_error(app, &operator, &operator_keys, body, "lease_id_hex").await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_unknown_receipt_lease_id_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut relay_receipt = fixture.relay_receipt;
    relay_receipt.receipt.quote_id[0] ^= 0x01;
    resign_test_relay_receipt(&mut relay_receipt);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &relay_receipt,
        &fixture.voucher,
        "quote id does not match",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_settled_lease_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut lease = wsv_lease_record_by_id(&app, &fixture.lease_id).expect("active lease");
    lease.status = VpnLeaseStatusV1::Settled;
    lease.settled_at_ms = Some(fixture.relay_receipt.receipt.ended_at_ms);
    lease.highest_voucher_sequence = fixture.relay_receipt.receipt.highest_voucher_sequence;
    lease.client_voucher_hash = Some(fixture.voucher.hash());
    lease.settled_client_voucher = Some(fixture.voucher.clone());
    lease.relay_receipt_hash = Some(fixture.relay_receipt.hash());
    lease.settled_relay_receipt = Some(fixture.relay_receipt.clone());
    lease.earned_fee = fixture.earned_fee.clone();
    lease.refunded_fee = lease
        .lease_fee
        .checked_sub(&fixture.earned_fee)
        .expect("fixture earned fee does not exceed lease fee");
    app.state.insert_vpn_lease_for_testing(lease);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &fixture.relay_receipt,
        &fixture.voucher,
        "active consensus-indexed VPN session",
    )
    .await;
}
#[tokio::test]
async fn submit_vpn_receipt_rejects_refunded_lease_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let mut lease = wsv_lease_record_by_id(&app, &fixture.lease_id).expect("active lease");
    lease.status = VpnLeaseStatusV1::Refunded;
    lease.refunded_at_ms = Some(lease.refund_available_at_ms());
    lease.refunded_fee = lease.lease_fee.clone();
    app.state.insert_vpn_lease_for_testing(lease);
    submit_receipt_expect_error(
        app,
        &operator,
        &operator_keys,
        &fixture.relay_receipt,
        &fixture.voucher,
        "active consensus-indexed VPN session",
    )
    .await;
}

#[tokio::test]
async fn submit_vpn_receipt_requires_operator_and_client_voucher() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let user_keys = checked_vpn_ed25519_keypair(0x7D);
    let operator_keys = checked_vpn_ed25519_keypair(0x7E);
    let user = account_id_for(&user_keys);
    let operator = account_id_for(&operator_keys);
    let app = vpn_enabled_app_with_operator(
        world_with_accounts(&[user.clone(), operator.clone()]),
        &operator,
    );
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &user, &user_keys, "standard").await;
    let session =
        create_session_for_quote(app.clone(), &user, &user_keys, &quote, &metering_keys).await;
    let active_record = app
        .vpn_sessions
        .get(&session.session_id)
        .expect("active session")
        .clone();
    app.state
        .insert_vpn_lease_for_testing(lease_record_from_session_record(
            &active_record,
            VpnLeaseStatusV1::Active,
            None,
        ));
    let relay_session_id =
        parse_vpn_session_id_hex(&session.session_id).expect("fixture session id");
    let quote_id = decode_hex_32(&session.quote_id, "quote").expect("quote id");
    assert_eq!(session.relay_id_hex, hex::encode(active_record.relay_id));
    let relay_id = active_record.relay_id;
    let issued_at_ms = now_ms();
    let voucher_body = VpnUsageVoucherBodyV1 {
        session_id: relay_session_id,
        quote_id,
        relay_id,
        sequence: 3,
        ingress_bytes: 1_024,
        egress_bytes: 2_048,
        active_ms: 10_000,
        issued_at_ms,
    };
    let voucher = VpnUsageVoucherV1::try_sign(voucher_body, metering_keys.private_key())
        .expect("checked usage voucher fixture");
    let active_ms = issued_at_ms.saturating_sub(session.connected_at_ms);
    let earned_fee = active_record
        .tariff
        .fee_for_usage(1_024, 2_048, active_ms)
        .expect("fixture tariff arithmetic");
    let receipt = sign_test_relay_receipt(VpnSessionReceiptV1 {
        session_id: relay_session_id,
        quote_id,
        payment_tx_hash: decode_hex_32(&session.payment_tx_hash, "payment").expect("payment"),
        account_hash: account_hash(&user),
        relay_id,
        ingress_bytes: 1_024,
        egress_bytes: 2_048,
        cover_bytes: 0,
        uptime_secs: u32::try_from(active_ms.div_ceil(1_000)).expect("fixture uptime"),
        started_at_ms: session.connected_at_ms,
        ended_at_ms: issued_at_ms,
        exit_class: VpnExitClassV1::Standard,
        meter_hash: vpn_tariff_meter_hash_v1(&active_record.tariff),
        earned_fee: earned_fee.clone(),
        highest_voucher_sequence: voucher.body.sequence,
        client_voucher_hash: voucher.hash(),
    });
    let body = norito::json::to_vec(&VpnReceiptSubmitRequestDto {
        relay_receipt_hex: hex::encode(receipt.encode()),
        client_voucher_hex: hex::encode(voucher.encode()),
        lease_id_hex: String::new(),
    })
    .expect("receipt request");
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());
    app.vpn_sessions.clear();
    let response = handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, body.as_ref())
        .await
        .expect("settlement prepared")
        .into_response();
    assert_eq!(response.status(), StatusCode::CREATED);
    let pending: VpnReceiptResponseDto = read_json(response).await;
    assert_eq!(pending.status, "settlement_pending");
    assert_eq!(pending.receipt_source, "relay");
    assert_eq!(pending.earned_fee, earned_fee);
    assert_eq!(
        pending.refunded_fee,
        session
            .lease_fee
            .checked_sub(&earned_fee)
            .expect("fixture earned fee does not exceed lease fee")
    );
    assert_eq!(pending.lease_id_hex, hex::encode(active_record.lease_id));
    let settle_instruction = pending
        .settle_lease_instruction
        .as_ref()
        .expect("native settle instruction");
    let settle_payload = hex::decode(&settle_instruction.payload_hex).expect("payload hex");
    let decoded_settle = iroha_data_model::isi::decode_instruction_from_pair(
        &settle_instruction.wire_id,
        &settle_payload,
    )
    .expect("decode native settle instruction");
    let settle = decoded_settle
        .as_any()
        .downcast_ref::<SettleVpnLease>()
        .expect("settle vpn lease instruction");
    assert_eq!(settle.lease_id, active_record.lease_id);
    assert_eq!(settle.relay_receipt, receipt);
    assert_eq!(settle.client_voucher, voucher);
    assert_eq!(app.vpn_sessions.len(), 0);
    assert!(app.vpn_receipts.get(&user).is_none());
    assert_eq!(
        wsv_lease_record_by_id(&app, &active_record.lease_id)
            .expect("active lease remains consensus-owned")
            .status,
        VpnLeaseStatusV1::Active
    );
    let runtime = lock_vpn_runtime(&app);
    assert!(runtime.settling_session_ids.is_empty());
}
