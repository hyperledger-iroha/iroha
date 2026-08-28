// Tail extracted from `vpn_tests.rs` to keep both test sources within the hard limit.
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
