// Test body included from the parent module to keep its production source budget bounded.
use super::*;
use crate::tests_runtime_handlers::{
    app_auth_test_guard, mk_app_state_for_tests_with_world, world_with_account,
};
use axum::{body::to_bytes, response::IntoResponse};
use iroha_core::state::World;
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::{
    Registrable,
    account::{Account, AccountId},
    domain::{Domain, DomainId},
    soranet::vpn::VpnUsageVoucherBodyV1,
    transaction::{
        FeePaymentIntent, TransactionBuilder,
        signed::{SealedTransactionReveal, compute_sealed_transaction_commitment},
    },
};
use norito::codec::Encode;
use std::{collections::BTreeSet, sync::Arc};
fn signed_app_headers_for_network(
    network_id: &iroha_data_model::NetworkId,
    account: &AccountId,
    key_pair: &KeyPair,
    method: &Method,
    uri: &Uri,
    body: &[u8],
) -> HeaderMap {
    static TEST_NONCE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_millis() as u64;
    let nonce_seq = TEST_NONCE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nonce = format!("vpn-test-{timestamp_ms}-{nonce_seq}");
    let message = crate::app_auth::canonical_network_request_signature_message(
        network_id,
        method,
        uri,
        body,
        timestamp_ms,
        &nonce,
    )
    .expect("canonical VPN test request is within V1 limits");
    let signature = Signature::try_new(key_pair.private_key(), &message)
        .expect("sign exact-network VPN request fixture");
    let mut headers = HeaderMap::new();
    headers.insert(
        crate::HEADER_ACCOUNT,
        account
            .to_canonical_hex()
            .expect("canonical account header")
            .parse()
            .expect("account header"),
    );
    headers.insert(
        crate::HEADER_SIGNATURE,
        crate::app_auth::signature_header_value(&signature)
            .expect("encode valid signature header")
            .parse()
            .expect("signature header"),
    );
    headers.insert(
        crate::HEADER_TIMESTAMP_MS,
        timestamp_ms.to_string().parse().expect("timestamp header"),
    );
    headers.insert(crate::HEADER_NONCE, nonce.parse().expect("nonce header"));
    headers
}
fn signed_app_headers(
    account: &AccountId,
    key_pair: &KeyPair,
    method: &Method,
    uri: &Uri,
    body: &[u8],
) -> HeaderMap {
    signed_app_headers_for_network(&vpn_test_network_id(), account, key_pair, method, uri, body)
}
fn account_id_for(key_pair: &KeyPair) -> AccountId {
    AccountId::new(key_pair.public_key().clone())
}
fn checked_vpn_ed25519_keypair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("test VPN fixture key derivation should succeed")
}
fn checked_vpn_account(seed: u8) -> AccountId {
    account_id_for(&checked_vpn_ed25519_keypair(seed))
}
fn vpn_test_network_id() -> iroha_data_model::NetworkId {
    let mut bytes = [0_u8; Hash::LENGTH];
    bytes[Hash::LENGTH - 1] = 1;
    iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(Hash::prehashed(
            bytes,
        )),
    )
}
fn test_vpn_relay_trust() -> VpnRelayTrust {
    let relay_keypair = test_vpn_relay_keypair();
    let (_, relay_id) = relay_keypair
        .public_key()
        .try_to_bytes()
        .expect("test relay identity");
    VpnRelayTrust {
        relay_id: relay_id.try_into().expect("32-byte Ed25519 identity"),
        relay_mldsa65_public_key: [0x5A; VPN_RELAY_MLDSA65_PUBLIC_KEY_BYTES_V1],
        relay_endpoint: "/dns/relay.example/udp/9443/quic".to_owned(),
        tls_server_name: "relay.example".to_owned(),
        relay_tls_spki_sha256: [0xAB; 32],
        descriptor_commit: [0xCD; 32],
        relay_certificate_sha256: [0xEF; 32],
        directory_snapshot_digest: [0x42; 32],
        valid_until_ms: u64::MAX,
    }
}
fn test_vpn_relay_keypair() -> KeyPair {
    checked_vpn_ed25519_keypair(0x55)
}
fn sign_test_relay_receipt(receipt: VpnSessionReceiptV1) -> VpnSignedSessionReceiptV1 {
    VpnSignedSessionReceiptV1::try_sign(receipt, test_vpn_relay_keypair().private_key())
        .expect("test VPN relay receipt should sign")
}
fn resign_test_relay_receipt(receipt: &mut VpnSignedSessionReceiptV1) {
    receipt.relay_signature = sign_test_relay_receipt(receipt.receipt.clone()).relay_signature;
}
#[test]
fn vpn_quote_id_commits_to_exact_network_identity_and_metering_key() {
    let network_id = vpn_test_network_id();
    let account_id = checked_vpn_account(0x31);
    let first_metering = checked_vpn_ed25519_keypair(0x32);
    let second_metering = checked_vpn_ed25519_keypair(0x33);
    let first = build_quote_id(
        &network_id,
        &account_id,
        first_metering.public_key(),
        "standard",
        "nonce:with:delimiters",
        1_700_000_000_000,
    );
    assert_eq!(
        first,
        build_quote_id(
            &network_id,
            &account_id,
            first_metering.public_key(),
            "standard",
            "nonce:with:delimiters",
            1_700_000_000_000,
        )
    );
    assert_ne!(
        first,
        build_quote_id(
            &network_id,
            &account_id,
            second_metering.public_key(),
            "standard",
            "nonce:with:delimiters",
            1_700_000_000_000,
        )
    );
    let foreign_network = iroha_data_model::NetworkId::from_genesis_hash(HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        Hash::prehashed([0x44; Hash::LENGTH]),
    ));
    assert_ne!(
        first,
        build_quote_id(
            &foreign_network,
            &account_id,
            first_metering.public_key(),
            "standard",
            "nonce:with:delimiters",
            1_700_000_000_000,
        )
    );
    let second_discriminant = {
        let _guard = iroha_data_model::account::address::ChainDiscriminantGuard::enter(0x7A7A);
        build_quote_id(
            &network_id,
            &account_id,
            first_metering.public_key(),
            "standard",
            "nonce:with:delimiters",
            1_700_000_000_000,
        )
    };
    assert_eq!(first, second_discriminant);
}

#[test]
fn vpn_payment_identity_is_the_inner_signed_hash_for_direct_and_sealed_entrypoints() {
    let network_id = vpn_test_network_id();
    let key_pair = checked_vpn_ed25519_keypair(0x34);
    let account_id = account_id_for(&key_pair);
    let signed = TransactionBuilder::new(
        network_id,
        account_id,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(key_pair.private_key());
    let canonical = canonical_signed_transaction_hash(&signed);
    let direct = TransactionEntrypoint::External(signed.clone());
    assert_eq!(direct.hash().as_ref(), &canonical);

    let salt = [0xA5; 32];
    let commitment = compute_sealed_transaction_commitment(&network_id, &signed, salt, 5);
    let sealed =
        TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(commitment, signed, salt));
    let TransactionEntrypoint::SealedReveal(reveal) = &sealed else {
        unreachable!("fixture is a sealed reveal")
    };
    assert_eq!(
        canonical_signed_transaction_hash(reveal.signed_transaction()),
        canonical
    );
    assert_ne!(
        sealed.hash().as_ref(),
        &canonical,
        "the outer sealed-reveal lookup hash must never become receipt identity"
    );
}

#[test]
fn vpn_receipt_rejects_outer_sealed_reveal_hash_substitution() {
    let network_id = vpn_test_network_id();
    let client_keys = checked_vpn_ed25519_keypair(0x35);
    let account_id = account_id_for(&client_keys);
    let signed = TransactionBuilder::new(
        network_id,
        account_id.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(client_keys.private_key());
    let canonical = canonical_signed_transaction_hash(&signed);
    let salt = [0x5A; 32];
    let commitment = compute_sealed_transaction_commitment(&network_id, &signed, salt, 5);
    let outer_hash =
        TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(commitment, signed, salt))
            .hash();
    assert_ne!(outer_hash.as_ref(), &canonical);

    let mut record = sample_session_record(&account_id);
    record.payment_tx_hash = hex::encode(canonical);
    let session_id = parse_vpn_session_id_hex(&record.session_id).expect("fixture session id");
    let quote_id = decode_hex_32(&record.quote_id, "quote_id").expect("fixture quote id");
    let metering_keys = checked_vpn_ed25519_keypair(0x54);
    let voucher = VpnUsageVoucherV1::try_sign(
        VpnUsageVoucherBodyV1 {
            session_id,
            quote_id,
            relay_id: record.relay_id,
            sequence: 1,
            ingress_bytes: 0,
            egress_bytes: 0,
            active_ms: 0,
            issued_at_ms: record.connected_at_ms,
        },
        metering_keys.private_key(),
    )
    .expect("fixture voucher");
    let receipt = VpnSessionReceiptV1 {
        session_id,
        quote_id,
        payment_tx_hash: *outer_hash.as_ref(),
        account_hash: account_hash(&account_id),
        relay_id: record.relay_id,
        ingress_bytes: 0,
        egress_bytes: 0,
        cover_bytes: 0,
        uptime_secs: 0,
        started_at_ms: record.connected_at_ms,
        ended_at_ms: record.connected_at_ms,
        exit_class: VpnExitClassV1::Standard,
        meter_hash: vpn_tariff_meter_hash_v1(&record.tariff),
        earned_fee: Quantity::zero(),
        highest_voucher_sequence: voucher.body.sequence,
        client_voucher_hash: voucher.hash(),
    };
    let mut receipt = sign_test_relay_receipt(receipt);
    let error = verify_relay_receipt_for_session(&record, &receipt, &voucher)
        .expect_err("outer sealed-reveal hash must not substitute for signed payment identity");
    assert!(format!("{error:?}").contains("payment hash does not match"));

    receipt.receipt.payment_tx_hash = canonical;
    resign_test_relay_receipt(&mut receipt);
    verify_relay_receipt_for_session(&record, &receipt, &voucher)
        .expect("canonical inner signed hash is valid receipt identity");
}
#[test]
fn vpn_relay_trust_rejects_unauthenticated_snapshot_bytes() {
    let error = VpnRelayTrust::from_guard_directory_at(
        b"attacker-controlled directory",
        [0xAA; 32],
        test_vpn_relay_trust().relay_id,
        1,
    )
    .expect_err("directory bytes without the provisioned digest must fail");
    assert!(
        error.contains("snapshot digest mismatch"),
        "unexpected trust error: {error}"
    );
}
#[test]
fn checked_vpn_ed25519_keypair_uses_fallible_seed_derivation() {
    assert_eq!(
        checked_vpn_ed25519_keypair(0x50).algorithm(),
        Algorithm::Ed25519
    );
    assert!(
        KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
        "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
    );
    assert_eq!(
        checked_vpn_account(0x51),
        account_id_for(&checked_vpn_ed25519_keypair(0x51))
    );
}
#[test]
fn vpn_operator_ticket_signer_requires_ed25519() {
    let ed25519 = checked_vpn_ed25519_keypair(0x63);
    ensure_vpn_operator_ticket_signer(ed25519.private_key())
        .expect("Ed25519 VPN operator key should be accepted");

    let secp256k1 = KeyPair::try_from_seed(vec![0x64; 32], Algorithm::Secp256k1)
        .expect("fixture seed derives secp256k1 keypair");
    let error = ensure_vpn_operator_ticket_signer(secp256k1.private_key())
        .expect_err("non-Ed25519 VPN operator key must fail before quote issuance");
    assert!(format!("{error:?}").contains("must use Ed25519"));
}
#[test]
fn active_fee_bounds_only_the_final_minute_ratio() {
    let maximum: Quantity = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
            .parse()
            .expect("signed 512-bit maximum quantity");
    assert_eq!(
        active_fee_per_minute(&maximum, 60)
            .expect("equal minute numerator and lease divisor cancel"),
        maximum
    );
}
fn world_with_accounts(accounts: &[AccountId]) -> World {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(
        accounts
            .first()
            .expect("at least one account is required for test world"),
    );
    let accounts = accounts
        .iter()
        .cloned()
        .map(|account_id| Account::new(account_id.clone()).build(&account_id))
        .collect::<Vec<_>>();
    World::with([domain], accounts, [])
}
fn vpn_enabled_app_with_operator_unchecked(
    world: World,
    operator_account_id: &AccountId,
) -> SharedAppState {
    let app = mk_app_state_for_tests_with_world(world);
    let quote_signer = (1_u16..=u16::from(u8::MAX))
        .map(|seed| checked_vpn_ed25519_keypair(seed as u8))
        .find(|key_pair| account_id_for(key_pair) == *operator_account_id)
        .expect("VPN fixture operator must come from a checked one-byte seed");
    let mut cfg = crate::test_utils::mk_minimal_root_cfg();
    cfg.network.soranet_vpn.enabled = true;
    cfg.network.soranet_vpn.operator_account_id = operator_account_id.clone();
    let mut inner = match Arc::try_unwrap(app) {
        Ok(inner) => inner,
        Err(_) => panic!("test app should be uniquely owned before VPN reconfiguration"),
    };
    inner.kiso = KisoHandle::mock(&cfg);
    inner.vpn_operator_signer = Some(quote_signer);
    inner.vpn_relay_trust = Some(Arc::new(test_vpn_relay_trust()));
    Arc::new(inner)
}
fn vpn_enabled_app_with_operator(
    mut world: World,
    operator_account_id: &AccountId,
) -> SharedAppState {
    let permission: Permission = CanIssueSoranetVpnQuote.into();
    let mut operator_permissions = {
        let permissions = world.account_permissions_mut_for_testing().view();
        permissions
            .get(operator_account_id)
            .cloned()
            .unwrap_or_default()
    };
    operator_permissions.insert(permission);
    world
        .account_permissions_mut_for_testing()
        .insert(operator_account_id.clone(), operator_permissions);
    vpn_enabled_app_with_operator_unchecked(world, operator_account_id)
}

#[test]
fn vpn_operator_signer_is_independent_of_proxy_bridge_signer() {
    let operator = checked_vpn_ed25519_keypair(0x56);
    let operator_account_id = account_id_for(&operator);
    let app = vpn_enabled_app_with_operator_unchecked(
        world_with_account(&operator_account_id),
        &operator_account_id,
    );
    let vpn_signer = app
        .vpn_operator_signer
        .as_ref()
        .expect("enabled VPN fixture has a dedicated operator signer");
    assert_eq!(vpn_signer.public_key(), operator.public_key());
    assert_ne!(
        vpn_signer.public_key(),
        app.torii_proxy_bridge_signer.public_key(),
        "VPN issuer and proxy bridge roles must remain distinct"
    );
}
fn metering_public_key_hex(key_pair: &KeyPair) -> String {
    let (_, payload) = key_pair
        .public_key()
        .try_to_bytes()
        .expect("test metering key is valid");
    hex::encode(payload)
}
#[test]
fn public_key_payload_hex_matches_checked_payload() {
    let key_pair = checked_vpn_ed25519_keypair(0x52);
    let (_, payload) = key_pair
        .public_key()
        .try_to_bytes()
        .expect("test key is valid");
    let encoded = public_key_payload_hex(key_pair.public_key()).expect("payload hex");
    assert_eq!(encoded, hex::encode(payload));
}
#[test]
fn parse_metering_public_key_rejects_inert_or_malformed_ed25519_material() {
    const SMALL_ORDER_POINT: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_IDENTITY: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    for (label, public_key_bytes) in [
        ("all-zero", [0_u8; 32]),
        ("small-order", SMALL_ORDER_POINT),
        ("noncanonical", NONCANONICAL_IDENTITY),
    ] {
        let error = parse_metering_public_key(&hex::encode(public_key_bytes))
            .expect_err("malformed metering key material must fail closed");
        assert!(
            format!("{error:?}").contains("metering_public_key_hex"),
            "{label} public key rejection should name the field: {error:?}"
        );
    }
}
async fn create_quote_for_account(
    app: SharedAppState,
    account: &AccountId,
    key_pair: &KeyPair,
    exit_class: &str,
) -> (VpnQuoteResponseDto, KeyPair) {
    let metering_keys = checked_vpn_ed25519_keypair(0x53);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
    let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
        exit_class: exit_class.to_owned(),
        metering_public_key_hex: metering_public_key_hex(&metering_keys),
    })
    .expect("quote body");
    let headers = signed_app_headers(account, key_pair, &method, &uri, body.as_ref());
    let response = handle_create_vpn_quote(app, &method, &uri, &headers, body.as_ref())
        .await
        .expect("quote")
        .into_response();
    assert_eq!(response.status(), StatusCode::CREATED);
    let quote = read_json(response).await;
    (quote, metering_keys)
}
async fn create_session_for_quote(
    app: SharedAppState,
    account: &AccountId,
    key_pair: &KeyPair,
    quote: &VpnQuoteResponseDto,
    metering_keys: &KeyPair,
) -> VpnSessionResponseDto {
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
    let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
        exit_class: quote.exit_class.clone(),
        quote_id: quote.quote_id.clone(),
        payment_tx_hash: quote.quote_id.clone(),
        metering_public_key_hex: metering_public_key_hex(&metering_keys),
    })
    .expect("session body");
    let headers = signed_app_headers(account, key_pair, &method, &uri, body.as_ref());
    let response = handle_create_vpn_session(app, &method, &uri, &headers, body.as_ref())
        .await
        .expect("session")
        .into_response();
    assert_eq!(response.status(), StatusCode::CREATED);
    read_json(response).await
}
fn sample_session_record(account_id: &AccountId) -> VpnSessionRecord {
    let metering_keys = checked_vpn_ed25519_keypair(0x54);
    let lease_fee = Quantity::from(1_000_000_u64);
    let network_id = vpn_test_network_id();
    let quote_id = [0x11; 32];
    let address_slot = derive_vpn_address_slot_v1(quote_id);
    let lease_id = derive_vpn_lease_id_v1(&network_id, quote_id, account_id);
    let session_id = derive_vpn_session_id_v1(&network_id, quote_id, account_id, address_slot);
    let fee_asset_definition = xor_asset_definition_id();
    let fee_asset_id = fee_asset_definition.to_string();
    let escrow_account_id =
        vpn_lease_custody_account_id(&network_id, &lease_id, &fee_asset_definition)
            .expect("fixture protocol custody");
    VpnSessionRecord {
        session_id: hex::encode(session_id),
        lease_id,
        account_id: account_id.clone(),
        exit_class: "standard".to_owned(),
        relay_endpoint: "/dns/relay.example/udp/9443/quic".to_owned(),
        lease_secs: 600,
        expires_at_ms: 601_000,
        connected_at_ms: 1_000,
        meter_family: "soranet.vpn.standard".to_owned(),
        quote_id: hex::encode(quote_id),
        payment_reference: hex::encode(quote_id),
        payment_tx_hash: "22".repeat(32),
        fee_asset_id,
        escrow_account_id,
        operator_account_id: account_id.clone(),
        lease_fee: lease_fee.clone(),
        tariff: vpn_tariff_for_lease(&lease_fee, 600).expect("valid fixture tariff"),
        flow_label_bits: 24,
        padding_budget_ms: 15,
        relay_id: test_vpn_relay_trust().relay_id,
        relay_mldsa65_public_key: test_vpn_relay_trust().relay_mldsa65_public_key,
        descriptor_commit: [0xCD; 32],
        tls_server_name: "relay.example".to_owned(),
        relay_tls_spki_sha256: [0xAB; 32],
        relay_certificate_sha256: [0xEF; 32],
        directory_snapshot_digest: [0x42; 32],
        relay_trust_valid_until_ms: u64::MAX,
        metering_public_key: metering_keys.public_key().clone(),
        route_pushes: vec!["0.0.0.0/0".to_owned()],
        excluded_routes: Vec::new(),
        dns_servers: vec!["1.1.1.1".to_owned()],
        tunnel_addresses: derive_vpn_address_plan_v1(address_slot).client_tunnel_addresses,
        mtu_bytes: u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES),
        helper_ticket_hex: String::new(),
        bytes_in: 0,
        bytes_out: 0,
    }
}
fn sample_quote_record(
    account_id: &AccountId,
    quote_id: String,
    quote_expires_at_ms: u64,
) -> VpnQuoteRecord {
    let mut session = sample_session_record(account_id);
    let operator = checked_vpn_ed25519_keypair(0x7A);
    session.operator_account_id = AccountId::new(operator.public_key().clone());
    let network_id = vpn_test_network_id();
    let quote_id_bytes = decode_hex_32(&quote_id, "quote").expect("quote id");
    let address_slot = derive_vpn_address_slot_v1(quote_id_bytes);
    let lease_id = derive_vpn_lease_id_v1(&network_id, quote_id_bytes, account_id);
    let session_id =
        derive_vpn_session_id_v1(&network_id, quote_id_bytes, account_id, address_slot);
    let asset_definition = xor_asset_definition_id();
    session.escrow_account_id =
        vpn_lease_custody_account_id(&network_id, &lease_id, &asset_definition)
            .expect("fixture custody");
    session.tunnel_addresses = derive_vpn_address_plan_v1(address_slot).client_tunnel_addresses;
    let policy = VpnQuotePolicyV1 {
        exit_class: VpnExitClassV1::try_from_label(&session.exit_class).expect("exit class"),
        relay_endpoint: session.relay_endpoint.clone(),
        relay_id: session.relay_id,
        relay_mldsa65_public_key: session.relay_mldsa65_public_key,
        descriptor_commit: session.descriptor_commit,
        tls_server_name: session.tls_server_name.clone(),
        relay_tls_spki_sha256: session.relay_tls_spki_sha256,
        relay_certificate_sha256: session.relay_certificate_sha256,
        directory_snapshot_digest: session.directory_snapshot_digest,
        relay_trust_valid_until_ms: session.relay_trust_valid_until_ms,
        lease_secs: session.lease_secs,
        meter_family: session.meter_family.clone(),
        fee_asset_id: asset_definition.to_string(),
        escrow_account_id: session.escrow_account_id.clone(),
        route_pushes: session.route_pushes.clone(),
        excluded_routes: session.excluded_routes.clone(),
        dns_servers: session.dns_servers.clone(),
        tunnel_addresses: session.tunnel_addresses.clone(),
        mtu_bytes: session.mtu_bytes,
        flow_label_bits: session.flow_label_bits,
        padding_budget_ms: session.padding_budget_ms,
    };
    let signed_quote = VpnSignedQuoteV1::try_sign(
        VpnQuoteBodyV1 {
            network_id,
            quote_id: quote_id_bytes,
            lease_id,
            session_id,
            address_slot,
            client_account_id: account_id.clone(),
            operator_account_id: session.operator_account_id.clone(),
            metering_public_key: session.metering_public_key.clone(),
            asset_definition,
            tariff: session.tariff.clone(),
            policy,
            valid_after_ms: 0,
            expires_at_ms: quote_expires_at_ms,
            settlement_grace_ms: 60_000,
        },
        operator.private_key(),
    )
    .expect("sign fixture quote");
    VpnQuoteRecord {
        quote_id: quote_id.clone(),
        lease_id,
        session_id,
        signed_quote,
        account_id: account_id.clone(),
        exit_class: session.exit_class,
        relay_endpoint: session.relay_endpoint,
        lease_secs: session.lease_secs,
        quote_expires_at_ms,
        payment_reference: quote_id,
        fee_asset_id: session.fee_asset_id,
        escrow_account_id: session.escrow_account_id,
        operator_account_id: session.operator_account_id,
        lease_fee: session.lease_fee,
        tariff: session.tariff,
        settlement_grace_ms: 60_000,
        metering_public_key: session.metering_public_key,
        route_pushes: session.route_pushes,
        excluded_routes: session.excluded_routes,
        dns_servers: session.dns_servers,
        tunnel_addresses: session.tunnel_addresses,
        mtu_bytes: session.mtu_bytes,
        meter_family: session.meter_family,
        flow_label_bits: session.flow_label_bits,
        padding_budget_ms: session.padding_budget_ms,
        relay_id: session.relay_id,
        relay_mldsa65_public_key: session.relay_mldsa65_public_key,
        descriptor_commit: session.descriptor_commit,
        tls_server_name: session.tls_server_name,
        relay_tls_spki_sha256: session.relay_tls_spki_sha256,
        relay_certificate_sha256: session.relay_certificate_sha256,
        directory_snapshot_digest: session.directory_snapshot_digest,
        relay_trust_valid_until_ms: session.relay_trust_valid_until_ms,
    }
}
fn sample_indexed_session_record(
    account_id: &AccountId,
    ordinal: usize,
    expires_at_ms: u64,
) -> VpnSessionRecord {
    let mut record = sample_session_record(account_id);
    record.session_id = format!("{ordinal:032x}");
    record.quote_id = format!("{ordinal:064x}");
    record.payment_reference = record.quote_id.clone();
    record.lease_id = decode_hex_32(&record.quote_id, "quote").expect("quote id");
    record.payment_tx_hash = format!("{:064x}", ordinal.saturating_add(10_000));
    record.expires_at_ms = expires_at_ms;
    record
}
fn fixture_operator_key(operator_account_id: &AccountId) -> KeyPair {
    (1_u16..=u16::from(u8::MAX))
        .map(|seed| checked_vpn_ed25519_keypair(seed as u8))
        .find(|key_pair| account_id_for(key_pair) == *operator_account_id)
        .expect("fixture VPN operator must come from a checked one-byte seed")
}
fn resign_lease_quote_projection(record: &mut VpnLeaseRecordV1) {
    let operator = fixture_operator_key(&record.operator_account_id);
    let mut body = record.signed_quote.body.clone();
    body.expires_at_ms = record.expires_at_ms;
    body.valid_after_ms = body.expires_at_ms.saturating_sub(
        body.policy
            .lease_secs
            .checked_mul(1_000)
            .expect("fixture lease duration milliseconds"),
    );
    body.settlement_grace_ms = record.settlement_grace_ms;
    record.signed_quote = VpnSignedQuoteV1::try_sign(body, operator.private_key())
        .expect("re-sign mutated VPN lease projection");
}
fn lease_record_from_session_record(
    record: &VpnSessionRecord,
    status: VpnLeaseStatusV1,
    settlement: Option<(VpnSignedSessionReceiptV1, VpnUsageVoucherV1)>,
) -> VpnLeaseRecordV1 {
    assert_eq!(
        settlement.is_some(),
        status == VpnLeaseStatusV1::Settled,
        "only settled VPN fixture records retain relay receipts"
    );
    let (relay_receipt, settled_client_voucher) = settlement.unzip();
    let lease_id = record.lease_id;
    let quote_id = decode_hex_32(&record.quote_id, "quote").expect("quote id");
    let session_id = parse_vpn_session_id_hex(&record.session_id).expect("fixture session id");
    let address_slot = VpnAddressSlotV1::from_session_id(session_id);
    let relay_receipt_hash = relay_receipt.as_ref().map(VpnSignedSessionReceiptV1::hash);
    let client_voucher_hash = relay_receipt
        .as_ref()
        .map(|receipt| receipt.receipt.client_voucher_hash);
    let earned_fee = relay_receipt
        .as_ref()
        .map_or_else(Quantity::zero, |receipt| receipt.receipt.earned_fee.clone());
    let refunded_fee = match status {
        VpnLeaseStatusV1::Active => Quantity::zero(),
        VpnLeaseStatusV1::Settled => record
            .lease_fee
            .checked_sub(&earned_fee)
            .expect("fixture earned fee does not exceed its lease fee"),
        VpnLeaseStatusV1::Refunded => record.lease_fee.clone(),
    };
    let asset_definition = xor_asset_definition_id();
    let quote_policy = VpnQuotePolicyV1 {
        exit_class: VpnExitClassV1::try_from_label(&record.exit_class).expect("exit class"),
        relay_endpoint: record.relay_endpoint.clone(),
        relay_id: record.relay_id,
        relay_mldsa65_public_key: record.relay_mldsa65_public_key,
        descriptor_commit: record.descriptor_commit,
        tls_server_name: record.tls_server_name.clone(),
        relay_tls_spki_sha256: record.relay_tls_spki_sha256,
        relay_certificate_sha256: record.relay_certificate_sha256,
        directory_snapshot_digest: record.directory_snapshot_digest,
        relay_trust_valid_until_ms: record.relay_trust_valid_until_ms,
        lease_secs: record.lease_secs,
        meter_family: record.meter_family.clone(),
        fee_asset_id: record.fee_asset_id.clone(),
        escrow_account_id: record.escrow_account_id.clone(),
        route_pushes: record.route_pushes.clone(),
        excluded_routes: record.excluded_routes.clone(),
        dns_servers: record.dns_servers.clone(),
        tunnel_addresses: record.tunnel_addresses.clone(),
        mtu_bytes: record.mtu_bytes,
        flow_label_bits: record.flow_label_bits,
        padding_budget_ms: record.padding_budget_ms,
    };
    let operator = fixture_operator_key(&record.operator_account_id);
    let signed_quote = VpnSignedQuoteV1::try_sign(
        VpnQuoteBodyV1 {
            network_id: vpn_test_network_id(),
            quote_id,
            lease_id,
            session_id,
            address_slot,
            client_account_id: record.account_id.clone(),
            operator_account_id: record.operator_account_id.clone(),
            metering_public_key: record.metering_public_key.clone(),
            asset_definition: asset_definition.clone(),
            tariff: record.tariff.clone(),
            policy: quote_policy.clone(),
            valid_after_ms: record
                .expires_at_ms
                .saturating_sub(record.lease_secs.saturating_mul(1_000)),
            expires_at_ms: record.expires_at_ms,
            settlement_grace_ms: 60_000,
        },
        operator.private_key(),
    )
    .expect("sign fixture retained VPN quote");
    VpnLeaseRecordV1 {
        lease_id,
        session_id,
        quote_id,
        client_account_id: record.account_id.clone(),
        operator_account_id: record.operator_account_id.clone(),
        metering_public_key: record.metering_public_key.clone(),
        asset_definition,
        lease_fee: record.lease_fee.clone(),
        custody_account_id: record.escrow_account_id.clone(),
        relay_id: record.relay_id,
        tariff: record.tariff.clone(),
        quote_policy,
        address_slot,
        signed_quote,
        open_tx_hash: decode_hex_32(&record.payment_tx_hash, "payment").expect("payment hash"),
        status,
        opened_at_ms: record.connected_at_ms,
        expires_at_ms: record.expires_at_ms,
        settlement_grace_ms: 60_000,
        settled_at_ms: (status == VpnLeaseStatusV1::Settled).then(|| {
            relay_receipt
                .as_ref()
                .map(|receipt| receipt.receipt.ended_at_ms)
                .expect("settled fixture receipt")
        }),
        refunded_at_ms: (status == VpnLeaseStatusV1::Refunded)
            .then(|| record.expires_at_ms.saturating_add(60_000)),
        highest_voucher_sequence: relay_receipt
            .as_ref()
            .map(|receipt| receipt.receipt.highest_voucher_sequence)
            .unwrap_or_default(),
        client_voucher_hash,
        settled_client_voucher,
        relay_receipt_hash,
        settled_relay_receipt: relay_receipt,
        earned_fee,
        refunded_fee,
    }
}
fn settled_lease_for_account(account: &AccountId, ordinal: u16) -> VpnLeaseRecordV1 {
    let mut quote_id = [0_u8; 32];
    quote_id[..2].copy_from_slice(&ordinal.to_be_bytes());
    let network_id = vpn_test_network_id();
    let address_slot = derive_vpn_address_slot_v1(quote_id);
    let lease_id = derive_vpn_lease_id_v1(&network_id, quote_id, account);
    let session_id = derive_vpn_session_id_v1(&network_id, quote_id, account, address_slot);
    let mut session = sample_session_record(account);
    session.session_id = hex::encode(session_id);
    session.lease_id = lease_id;
    session.quote_id = hex::encode(quote_id);
    session.payment_reference = hex::encode(quote_id);
    let asset_definition = xor_asset_definition_id();
    session.escrow_account_id =
        vpn_lease_custody_account_id(&network_id, &lease_id, &asset_definition)
            .expect("fixture protocol custody");
    session.tunnel_addresses = derive_vpn_address_plan_v1(address_slot).client_tunnel_addresses;
    let settled_at_ms = 10_000_u64 + u64::from(ordinal);
    let active_ms = settled_at_ms - session.connected_at_ms;
    let voucher = VpnUsageVoucherV1::try_sign(
        VpnUsageVoucherBodyV1 {
            session_id: parse_vpn_session_id_hex(&session.session_id).expect("fixture session id"),
            quote_id,
            relay_id: session.relay_id,
            sequence: u64::from(ordinal),
            ingress_bytes: u64::from(ordinal),
            egress_bytes: u64::from(ordinal),
            active_ms,
            issued_at_ms: settled_at_ms,
        },
        checked_vpn_ed25519_keypair(0x54).private_key(),
    )
    .expect("sign settled lease fixture voucher");
    let earned_fee = session
        .tariff
        .fee_for_usage(
            voucher.body.ingress_bytes,
            voucher.body.egress_bytes,
            active_ms,
        )
        .expect("settled lease fixture tariff arithmetic");
    let relay_receipt = sign_test_relay_receipt(VpnSessionReceiptV1 {
        session_id: parse_vpn_session_id_hex(&session.session_id).expect("fixture session id"),
        quote_id,
        payment_tx_hash: decode_hex_32(&session.payment_tx_hash, "payment").expect("payment"),
        account_hash: account_hash(account),
        relay_id: session.relay_id,
        ingress_bytes: voucher.body.ingress_bytes,
        egress_bytes: voucher.body.egress_bytes,
        cover_bytes: 0,
        uptime_secs: u32::try_from(active_ms.div_ceil(1_000)).expect("fixture uptime"),
        started_at_ms: session.connected_at_ms,
        ended_at_ms: settled_at_ms,
        exit_class: VpnExitClassV1::Standard,
        meter_hash: vpn_tariff_meter_hash_v1(&session.tariff),
        earned_fee,
        highest_voucher_sequence: voucher.body.sequence,
        client_voucher_hash: voucher.hash(),
    });
    lease_record_from_session_record(
        &session,
        VpnLeaseStatusV1::Settled,
        Some((relay_receipt, voucher)),
    )
}
#[test]
fn persisted_session_rejects_trust_expiring_before_lease() {
    let account = checked_vpn_account(0x5E);
    let session = sample_session_record(&account);
    let mut lease = lease_record_from_session_record(&session, VpnLeaseStatusV1::Active, None);
    lease.quote_policy.relay_trust_valid_until_ms = lease.expires_at_ms - 1;
    let error = session_record_from_lease(&lease)
        .expect_err("persisted lease must remain bounded by authenticated trust");
    assert!(format!("{error:?}").contains("complete persisted lease"));
}
#[test]
fn persisted_session_requires_current_authenticated_trust() {
    let account = checked_vpn_account(0x60);
    let session = sample_session_record(&account);
    let lease = lease_record_from_session_record(&session, VpnLeaseStatusV1::Active, None);
    let trust = test_vpn_relay_trust();
    ensure_lease_matches_authenticated_trust(&lease, &trust)
        .expect("exact authenticated trust must reconstruct the session");
    let mut wrong_trust = trust;
    wrong_trust.directory_snapshot_digest[0] ^= 1;
    let error = ensure_lease_matches_authenticated_trust(&lease, &wrong_trust)
        .expect_err("different authenticated snapshot must not reconstruct the session");
    assert!(format!("{error:?}").contains("authenticated relay trust"));
}
#[test]
fn pending_receipt_projection_reports_the_relay_service_interval() {
    let account = checked_vpn_account(0x61);
    let mut session = sample_session_record(&account);
    session.connected_at_ms = 1_200;
    let session_id = parse_vpn_session_id_hex(&session.session_id).expect("fixture session id");
    let quote_id = decode_hex_32(&session.quote_id, "quote").expect("quote id");
    let metering_keys = checked_vpn_ed25519_keypair(0x62);
    let voucher = VpnUsageVoucherV1::try_sign(
        VpnUsageVoucherBodyV1 {
            session_id,
            quote_id,
            relay_id: session.relay_id,
            sequence: 1,
            ingress_bytes: 0,
            egress_bytes: 0,
            active_ms: 500,
            issued_at_ms: 1_500,
        },
        metering_keys.private_key(),
    )
    .expect("sign projection voucher");
    let relay_receipt = sign_test_relay_receipt(VpnSessionReceiptV1 {
        session_id,
        quote_id,
        payment_tx_hash: decode_hex_32(&session.payment_tx_hash, "payment").expect("payment hash"),
        account_hash: account_hash(&account),
        relay_id: session.relay_id,
        ingress_bytes: 0,
        egress_bytes: 0,
        cover_bytes: 0,
        uptime_secs: 1,
        started_at_ms: 1_000,
        ended_at_ms: 1_500,
        exit_class: VpnExitClassV1::Standard,
        meter_hash: vpn_tariff_meter_hash_v1(&session.tariff),
        earned_fee: Quantity::zero(),
        highest_voucher_sequence: voucher.body.sequence,
        client_voucher_hash: voucher.hash(),
    });
    let pending = build_pending_settlement_receipt_record(
        &session,
        &relay_receipt,
        &voucher,
        session.lease_id,
        hex::encode(session.lease_id),
    )
    .expect("pending receipt projection");

    assert_eq!(pending.connected_at_ms, 1_200);
    assert_eq!(pending.disconnected_at_ms, 1_500);
    assert_eq!(pending.duration_ms, 500);
}
struct ReceiptFixture {
    body: Vec<u8>,
    relay_receipt: VpnSignedSessionReceiptV1,
    voucher: VpnUsageVoucherV1,
    earned_fee: Quantity,
    lease_id: [u8; 32],
}
fn receipt_submit_body(
    relay_receipt: &VpnSignedSessionReceiptV1,
    voucher: &VpnUsageVoucherV1,
) -> Vec<u8> {
    receipt_submit_body_with_lease_id(relay_receipt, voucher, String::new())
}
fn receipt_submit_body_with_lease_id(
    relay_receipt: &VpnSignedSessionReceiptV1,
    voucher: &VpnUsageVoucherV1,
    lease_id_hex: String,
) -> Vec<u8> {
    norito::json::to_vec(&VpnReceiptSubmitRequestDto {
        relay_receipt_hex: hex::encode(relay_receipt.encode()),
        client_voucher_hex: hex::encode(voucher.encode()),
        lease_id_hex,
    })
    .expect("receipt request")
}
fn receipt_fixture_for_session(
    session: &VpnSessionResponseDto,
    record: &VpnSessionRecord,
    account: &AccountId,
    metering_keys: &KeyPair,
) -> ReceiptFixture {
    let relay_session_id =
        parse_vpn_session_id_hex(&session.session_id).expect("fixture session id");
    let quote_id = decode_hex_32(&session.quote_id, "quote").expect("quote id");
    assert_eq!(session.relay_id_hex, hex::encode(record.relay_id));
    let relay_id = record.relay_id;
    let voucher_body = VpnUsageVoucherBodyV1 {
        session_id: relay_session_id,
        quote_id,
        relay_id,
        sequence: 3,
        ingress_bytes: 4_096,
        egress_bytes: 8_192,
        active_ms: 2_000,
        issued_at_ms: now_ms(),
    };
    let voucher = VpnUsageVoucherV1::try_sign(voucher_body, metering_keys.private_key())
        .expect("checked usage voucher fixture");
    let earned_fee = record
        .tariff
        .fee_for_usage(1_024, 2_048, 0)
        .expect("fixture tariff arithmetic");
    let receipt = sign_test_relay_receipt(VpnSessionReceiptV1 {
        session_id: relay_session_id,
        quote_id,
        payment_tx_hash: decode_hex_32(&session.payment_tx_hash, "payment").expect("payment"),
        account_hash: account_hash(account),
        relay_id,
        ingress_bytes: 1_024,
        egress_bytes: 2_048,
        cover_bytes: 0,
        uptime_secs: 0,
        started_at_ms: voucher.body.issued_at_ms,
        ended_at_ms: voucher.body.issued_at_ms,
        exit_class: VpnExitClassV1::Standard,
        meter_hash: vpn_tariff_meter_hash_v1(&record.tariff),
        earned_fee: earned_fee.clone(),
        highest_voucher_sequence: voucher.body.sequence,
        client_voucher_hash: voucher.hash(),
    });
    let body = receipt_submit_body(&receipt, &voucher);
    ReceiptFixture {
        body,
        relay_receipt: receipt,
        voucher,
        earned_fee,
        lease_id: record.lease_id,
    }
}
async fn active_wsv_receipt_fixture() -> (
    SharedAppState,
    AccountId,
    KeyPair,
    AccountId,
    KeyPair,
    KeyPair,
    ReceiptFixture,
) {
    active_wsv_receipt_fixture_with_additional_accounts(&[]).await
}
async fn active_wsv_receipt_fixture_with_additional_accounts(
    additional_accounts: &[AccountId],
) -> (
    SharedAppState,
    AccountId,
    KeyPair,
    AccountId,
    KeyPair,
    KeyPair,
    ReceiptFixture,
) {
    let user_keys = checked_vpn_ed25519_keypair(0x55);
    let operator_keys = checked_vpn_ed25519_keypair(0x56);
    let user = account_id_for(&user_keys);
    let operator = account_id_for(&operator_keys);
    let mut accounts = Vec::with_capacity(2 + additional_accounts.len());
    accounts.push(user.clone());
    accounts.push(operator.clone());
    accounts.extend_from_slice(additional_accounts);
    let app = vpn_enabled_app_with_operator(world_with_accounts(&accounts), &operator);
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
    app.vpn_sessions.clear();
    (
        app,
        user,
        user_keys,
        operator,
        operator_keys,
        metering_keys,
        fixture,
    )
}
async fn submit_receipt_expect_error(
    app: SharedAppState,
    operator: &AccountId,
    operator_keys: &KeyPair,
    relay_receipt: &VpnSignedSessionReceiptV1,
    voucher: &VpnUsageVoucherV1,
    expected: &str,
) {
    let body = receipt_submit_body(relay_receipt, voucher);
    submit_receipt_body_expect_error(app, operator, operator_keys, body, expected).await;
}
async fn submit_receipt_body_expect_error(
    app: SharedAppState,
    operator: &AccountId,
    operator_keys: &KeyPair,
    body: Vec<u8>,
    expected: &str,
) {
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(operator, operator_keys, &method, &uri, body.as_ref());
    let error = handle_submit_vpn_receipt(app, &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("adversarial receipt must fail");
    assert!(
        format!("{error:?}").contains(expected),
        "expected `{expected}` in {error:?}"
    );
}
async fn read_json<T>(response: axum::response::Response) -> T
where
    T: norito::json::JsonDeserializeOwned,
{
    let status = response.status();
    assert!(status.is_success() || status == StatusCode::NOT_FOUND);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response bytes");
    if status == StatusCode::NOT_FOUND {
        panic!("expected JSON response, got 404");
    }
    norito::json::from_slice(bytes.as_ref()).expect("json body")
}
#[tokio::test]
async fn vpn_profile_uses_config_summary() {
    let account = checked_vpn_account(0x57);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let response = handle_get_vpn_profile(app)
        .await
        .expect("profile")
        .into_response();
    let body: VpnProfileResponseDto = read_json(response).await;
    assert_eq!(body.default_exit_class, "standard");
    assert!(body.available);
    assert_eq!(
        body.supported_exit_classes,
        vec!["standard", "low-latency", "high-security"]
    );
    assert!(!body.relay_endpoint.trim().is_empty());
    assert_eq!(body.mtu_bytes, u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES));
    assert_eq!(body.tunnel_addresses, default_tunnel_addresses());
    assert_eq!(body.route_pushes, vec!["0.0.0.0/0", "::/0"]);
    assert_eq!(body.dns_servers, vec!["1.1.1.1"]);
    assert_eq!(
        body.relay_id_hex,
        hex::encode(test_vpn_relay_trust().relay_id)
    );
    assert_eq!(
        body.relay_mldsa65_public_key_hex,
        hex::encode(test_vpn_relay_trust().relay_mldsa65_public_key)
    );
    assert_eq!(body.descriptor_commit_hex, "cd".repeat(32));
    assert_eq!(body.tls_server_name, "relay.example");
    assert_eq!(body.relay_tls_spki_sha256_hex, "ab".repeat(32));
    assert_eq!(body.relay_certificate_sha256_hex, "ef".repeat(32));
    assert_eq!(body.directory_snapshot_digest_hex, "42".repeat(32));
}
#[test]
fn vpn_wall_clock_conversion_fails_closed_before_the_unix_epoch() {
    assert_eq!(system_time_ms(UNIX_EPOCH), 0);
    assert_eq!(
        system_time_ms(UNIX_EPOCH - std::time::Duration::from_millis(1)),
        u64::MAX
    );
}
#[tokio::test]
async fn vpn_profile_hides_trust_that_cannot_cover_a_lease() {
    let account = checked_vpn_account(0x5F);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let mut inner = Arc::try_unwrap(app)
        .unwrap_or_else(|_| panic!("test app should be uniquely owned before trust update"));
    let mut trust = test_vpn_relay_trust();
    trust.valid_until_ms = now_ms();
    inner.vpn_relay_trust = Some(Arc::new(trust));
    let app = Arc::new(inner);
    let response = handle_get_vpn_profile(app)
        .await
        .expect("profile")
        .into_response();
    let body: VpnProfileResponseDto = read_json(response).await;
    assert!(!body.available);
    assert!(body.relay_endpoint.is_empty());
    assert!(body.relay_id_hex.is_empty());
    assert!(body.relay_mldsa65_public_key_hex.is_empty());
    assert!(body.descriptor_commit_hex.is_empty());
    assert!(body.tls_server_name.is_empty());
    assert!(body.relay_tls_spki_sha256_hex.is_empty());
    assert!(body.relay_certificate_sha256_hex.is_empty());
    assert!(body.directory_snapshot_digest_hex.is_empty());
}
#[tokio::test]
async fn vpn_profile_is_unavailable_without_the_dedicated_operator_signer() {
    let account = checked_vpn_account(0x6F);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let mut inner = Arc::try_unwrap(app)
        .unwrap_or_else(|_| panic!("test app should be uniquely owned before signer removal"));
    inner.vpn_operator_signer = None;
    let response = handle_get_vpn_profile(Arc::new(inner))
        .await
        .expect("profile")
        .into_response();
    let body: VpnProfileResponseDto = read_json(response).await;
    assert!(!body.available);
    assert!(body.relay_endpoint.is_empty());
    assert!(body.relay_id_hex.is_empty());
}
#[tokio::test]
async fn create_vpn_quote_rejects_an_unapproved_operator_signer() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let client_keys = checked_vpn_ed25519_keypair(0x60);
    let client = account_id_for(&client_keys);
    let operator = checked_vpn_account(0x61);
    let app = vpn_enabled_app_with_operator_unchecked(
        world_with_accounts(&[client.clone(), operator.clone()]),
        &operator,
    );
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
    let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
        exit_class: "standard".to_owned(),
        metering_public_key_hex: metering_public_key_hex(&checked_vpn_ed25519_keypair(0x62)),
    })
    .expect("quote body");
    let headers = signed_app_headers(&client, &client_keys, &method, &uri, body.as_ref());
    let error = handle_create_vpn_quote(app, &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("an unapproved signer must not emit a VPN quote");
    assert!(
        format!("{error:?}").contains("CanIssueSoranetVpnQuote"),
        "unexpected issuer denial: {error:?}"
    );
}
#[tokio::test]
async fn create_vpn_quote_requires_trust_for_complete_lease() {
    let client_keys = checked_vpn_ed25519_keypair(0x5B);
    let client = account_id_for(&client_keys);
    let operator = checked_vpn_account(0x5C);
    let app = vpn_enabled_app_with_operator(
        world_with_accounts(&[client.clone(), operator.clone()]),
        &operator,
    );
    let mut inner = Arc::try_unwrap(app)
        .unwrap_or_else(|_| panic!("test app should be uniquely owned before trust update"));
    let mut trust = test_vpn_relay_trust();
    trust.valid_until_ms = now_ms().saturating_add(1);
    inner.vpn_relay_trust = Some(Arc::new(trust));
    let app = Arc::new(inner);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
    let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
        exit_class: "standard".to_owned(),
        metering_public_key_hex: metering_public_key_hex(&checked_vpn_ed25519_keypair(0x5D)),
    })
    .expect("quote body");
    let headers = signed_app_headers(&client, &client_keys, &method, &uri, body.as_ref());
    let error = match handle_create_vpn_quote(app, &method, &uri, &headers, body.as_ref()).await {
        Ok(_) => panic!("lease extending beyond authenticated trust must fail"),
        Err(error) => format!("{error:?}"),
    };
    assert!(error.contains("complete VPN lease"));
}
#[tokio::test]
async fn create_vpn_quote_derives_protocol_custody() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x58);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
    let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
        exit_class: "standard".to_owned(),
        metering_public_key_hex: metering_public_key_hex(&checked_vpn_ed25519_keypair(0x59)),
    })
    .expect("quote body");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());
    let response = handle_create_vpn_quote(app.clone(), &method, &uri, &headers, body.as_ref())
        .await
        .expect("quote creation must derive protocol custody")
        .into_response();
    let quote: VpnQuoteResponseDto = read_json(response).await;
    let lease_id = decode_hex_32(&quote.lease_id_hex, "lease_id").expect("lease id");
    let expected_custody = vpn_lease_custody_account_id(
        app.state.network_id_ref(),
        &lease_id,
        &xor_asset_definition_id(),
    )
    .expect("deterministic protocol custody");
    assert_eq!(quote.fee_asset_id, xor_asset_definition_id().to_string());
    assert_eq!(quote.escrow_account_id, expected_custody.to_string());
    assert_ne!(quote.escrow_account_id, account.to_string());
}
#[test]
fn helper_ticket_is_signed_by_the_vpn_operator() {
    let issuer = checked_vpn_ed25519_keypair(0x5A);
    let account = checked_vpn_account(0x5A);
    let record = sample_session_record(&account);
    let expires_at_ms = 50_000;
    let encoded =
        build_helper_ticket_hex(&record, expires_at_ms, issuer.private_key()).expect("ticket");
    let parsed = VpnHelperTicketV1::parse_hex(&encoded, issuer.public_key(), 1)
        .expect("ticket should parse");
    assert_eq!(
        parse_vpn_session_id_hex(&record.session_id).expect("fixture session id"),
        parsed.session_id
    );
    assert_eq!(
        decode_hex_32(&record.quote_id, "quote").unwrap(),
        parsed.quote_id
    );
    assert_eq!(
        decode_hex_32(&record.payment_tx_hash, "payment").unwrap(),
        parsed.payment_tx_hash
    );
    assert_eq!(account_hash(&record.account_id), parsed.account_hash);
    assert_eq!(record.relay_id, parsed.relay_id);
    assert_eq!(record.metering_public_key, parsed.metering_public_key);
    assert_eq!(record.tariff, parsed.tariff);
    assert_eq!(
        vpn_helper_network_policy_hash_v1(
            &record.relay_endpoint,
            &record.relay_id,
            &record.relay_mldsa65_public_key,
            &record.descriptor_commit,
            &record.tls_server_name,
            &record.relay_tls_spki_sha256,
            &record.relay_certificate_sha256,
            &record.directory_snapshot_digest,
            record.padding_budget_ms,
            &record.route_pushes,
            &record.excluded_routes,
            &record.dns_servers,
            &record.tunnel_addresses,
            record.mtu_bytes,
        ),
        parsed.network_policy_hash
    );
    assert_eq!(expires_at_ms, parsed.expires_at_ms);
}
#[test]
fn settlement_lease_id_canonicalizes_explicit_prefixed_hex() {
    let request = VpnReceiptSubmitRequestDto {
        relay_receipt_hex: String::new(),
        client_voucher_hex: String::new(),
        lease_id_hex: format!("0X{}", "AB".repeat(32)),
    };
    let (lease_id, normalized_hex) =
        settlement_lease_id_from_request_or_index(&request, [0xAB; 32]).expect("explicit lease id");
    assert_eq!(lease_id, [0xAB; 32]);
    assert_eq!(normalized_hex, "ab".repeat(32));
}
#[test]
fn vpn_settlement_evidence_hex_is_bounded_before_decode() {
    let oversized_receipt = "00".repeat(VPN_MAX_SIGNED_RELAY_RECEIPT_NORITO_BYTES_V1 + 1);
    let receipt_error = decode_norito_hex::<VpnSignedSessionReceiptV1>(
        &oversized_receipt,
        "relay_receipt_hex",
        VPN_MAX_SIGNED_RELAY_RECEIPT_NORITO_BYTES_V1,
    )
    .expect_err("oversized signed relay receipt must fail before Norito decode");
    assert!(format!("{receipt_error:?}").contains("exceeds the V1 limit"));

    let oversized_voucher = "00".repeat(VPN_MAX_CLIENT_VOUCHER_NORITO_BYTES_V1 + 1);
    let voucher_error = decode_norito_hex::<VpnUsageVoucherV1>(
        &oversized_voucher,
        "client_voucher_hex",
        VPN_MAX_CLIENT_VOUCHER_NORITO_BYTES_V1,
    )
    .expect_err("oversized client voucher must fail before Norito decode");
    assert!(format!("{voucher_error:?}").contains("exceeds the V1 limit"));
}
#[test]
fn vpn_settlement_evidence_hex_requires_exact_lowercase_spelling() {
    for noncanonical in ["", "0", "0x00", "0X00", "AA", " 00", "00 "] {
        let error = decode_norito_hex::<VpnUsageVoucherV1>(
            noncanonical,
            "client_voucher_hex",
            VPN_MAX_CLIENT_VOUCHER_NORITO_BYTES_V1,
        )
        .expect_err("non-canonical VPN evidence hex must fail before Norito decode");
        assert!(
            format!("{error:?}").contains(
                "must be non-empty even-length lowercase hexadecimal without a prefix or whitespace"
            ),
            "unexpected error for `{noncanonical}`: {error:?}"
        );
    }
}
#[test]
fn vpn_mutation_routes_pin_the_protocol_specific_body_limit() {
    assert_eq!(VPN_MUTATION_REQUEST_MAX_BYTES_V1, 16 * 1_024);
    let compact_source = include_str!("lib.rs")
        .split_whitespace()
        .collect::<String>();
    for handler in [
        "handler_create_vpn_quote",
        "handler_create_vpn_session",
        "handler_submit_vpn_receipt",
    ] {
        let expected = format!(
            "limited_canonical_signature_post({handler},vpn::VPN_MUTATION_REQUEST_MAX_BYTES_V1)"
        );
        assert!(
            compact_source.contains(&expected),
            "VPN mutation handler {handler} must retain its protocol-specific body limit"
        );
    }
}
#[tokio::test]
async fn submit_vpn_receipt_canonicalizes_explicit_lease_id() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let (app, user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
        active_wsv_receipt_fixture().await;
    let canonical_lease_id = hex::encode(fixture.lease_id);
    let submitted_lease_id = format!("0X{}", canonical_lease_id.to_uppercase());
    let body = receipt_submit_body_with_lease_id(
        &fixture.relay_receipt,
        &fixture.voucher,
        submitted_lease_id.clone(),
    );
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());
    let response = handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, body.as_ref())
        .await
        .expect("uppercase explicit lease id should be accepted")
        .into_response();
    let receipt: VpnReceiptResponseDto = read_json(response).await;
    assert_eq!(receipt.lease_id_hex, canonical_lease_id);
    assert_ne!(receipt.lease_id_hex, submitted_lease_id);
    assert_eq!(receipt.status, "settlement_pending");
    assert!(receipt.settle_lease_instruction.is_some());
    assert!(app.vpn_receipts.get(&user).is_none());
    assert_eq!(
        lease_record_by_id(&app, &fixture.lease_id)
            .expect("active lease remains consensus-owned")
            .status,
        VpnLeaseStatusV1::Active
    );
}
#[tokio::test]
async fn create_vpn_session_canonicalizes_payment_hash() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x8A);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
    let submitted_payment_hash = format!("0X{}", quote.quote_id.to_uppercase());
    let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
        exit_class: quote.exit_class.clone(),
        quote_id: quote.quote_id.clone(),
        payment_tx_hash: submitted_payment_hash.clone(),
        metering_public_key_hex: metering_public_key_hex(&metering_keys),
    })
    .expect("session body");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());
    let response = handle_create_vpn_session(app.clone(), &method, &uri, &headers, body.as_ref())
        .await
        .expect("uppercase payment hash should be accepted")
        .into_response();
    let session: VpnSessionResponseDto = read_json(response).await;
    assert_eq!(session.payment_tx_hash, quote.quote_id);
    assert_ne!(session.payment_tx_hash, submitted_payment_hash);
    let stored = app
        .vpn_sessions
        .get(&session.session_id)
        .expect("stored session");
    assert_eq!(stored.payment_tx_hash, quote.quote_id);
    drop(stored);
    assert!(app.vpn_used_payments.contains_key(&quote.quote_id));
    assert!(!app.vpn_used_payments.contains_key(&submitted_payment_hash));
}
#[tokio::test]
async fn create_vpn_session_requires_signed_headers() {
    let account = checked_vpn_account(0x5B);
    let app = mk_app_state_for_tests_with_world(world_with_account(&account));
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/sessions".parse().expect("uri");
    let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
        exit_class: "standard".to_owned(),
        quote_id: String::new(),
        payment_tx_hash: String::new(),
        metering_public_key_hex: String::new(),
    })
    .expect("body");
    let error =
        match handle_create_vpn_session(app, &method, &uri, &HeaderMap::new(), body.as_ref()).await
        {
            Ok(_) => panic!("missing auth should fail"),
            Err(error) => error,
        };
    assert!(format!("{error:?}").contains("signed account headers are required"));
}
#[tokio::test]
async fn vpn_request_rejects_foreign_network_signature_before_decode() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0xA7);
    let account = account_id_for(&key_pair);
    let app = mk_app_state_for_tests_with_world(world_with_account(&account));
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
    let body = b"{not valid json";
    let foreign_network = iroha_data_model::NetworkId::from_genesis_hash(HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        Hash::new(b"same-label-foreign-vpn-genesis"),
    ));
    let headers =
        signed_app_headers_for_network(&foreign_network, &account, &key_pair, &method, &uri, body);
    let error = handle_create_vpn_session(app, &method, &uri, &headers, body)
        .await
        .expect_err("foreign-network VPN signature must fail closed");
    let message = format!("{error:?}");
    assert!(
        message.contains("query signature failed verification"),
        "unexpected foreign-network rejection: {message}"
    );
    assert!(
        !message.contains("invalid vpn session payload"),
        "VPN authentication must precede semantic decode: {message}"
    );
}
#[tokio::test]
async fn vpn_request_handlers_reject_unknown_json_fields_after_auth() {
    fn assert_unknown_field(error: Error, payload_label: &str) {
        let message = format!("{error:?}");
        assert!(
            message.contains(payload_label),
            "expected {payload_label} context, got {message}"
        );
        assert!(
            message.contains("unknown field") && message.contains("unexpected"),
            "expected the unexpected field to be rejected, got {message}"
        );
    }
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x5C);
    let account = account_id_for(&key_pair);
    let app = mk_app_state_for_tests_with_world(world_with_account(&account));
    let method = Method::POST;
    let quote_uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
    let quote_body = br#"{"metering_public_key_hex":"","unexpected":true}"#;
    let quote_headers = signed_app_headers(&account, &key_pair, &method, &quote_uri, quote_body);
    let quote_error =
        handle_create_vpn_quote(app.clone(), &method, &quote_uri, &quote_headers, quote_body)
            .await
            .expect_err("unknown quote field must fail after auth");
    assert_unknown_field(quote_error, "invalid vpn quote create payload");
    let session_uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
    let session_body =
        br#"{"quote_id":"","payment_tx_hash":"","metering_public_key_hex":"","unexpected":true}"#;
    let session_headers =
        signed_app_headers(&account, &key_pair, &method, &session_uri, session_body);
    let session_error = handle_create_vpn_session(
        app.clone(),
        &method,
        &session_uri,
        &session_headers,
        session_body,
    )
    .await
    .expect_err("unknown session field must fail after auth");
    assert_unknown_field(session_error, "invalid vpn session create payload");
    let receipt_uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let receipt_body = br#"{"relay_receipt_hex":"","client_voucher_hex":"","unexpected":true}"#;
    let receipt_headers =
        signed_app_headers(&account, &key_pair, &method, &receipt_uri, receipt_body);
    let receipt_error =
        handle_submit_vpn_receipt(app, &method, &receipt_uri, &receipt_headers, receipt_body)
            .await
            .expect_err("unknown receipt field must fail after auth");
    assert_unknown_field(receipt_error, "invalid vpn receipt payload");
}
#[tokio::test]
async fn vpn_write_handlers_authenticate_before_parsing_malformed_json() {
    let account = checked_vpn_account(0x5C);
    let app = mk_app_state_for_tests_with_world(world_with_account(&account));
    let method = Method::POST;
    let headers = HeaderMap::new();
    let body = b"{not valid json";
    let quote_uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
    let quote_error = handle_create_vpn_quote(app.clone(), &method, &quote_uri, &headers, body)
        .await
        .expect_err("missing quote authentication must win over malformed JSON");
    let session_uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
    let session_error =
        handle_create_vpn_session(app.clone(), &method, &session_uri, &headers, body)
            .await
            .expect_err("missing session authentication must win over malformed JSON");
    let receipt_uri: Uri = "/v1/vpn/receipts".parse().expect("receipt uri");
    let receipt_error = handle_submit_vpn_receipt(app, &method, &receipt_uri, &headers, body)
        .await
        .expect_err("missing receipt authentication must win over malformed JSON");
    for error in [quote_error, session_error, receipt_error] {
        assert!(
            format!("{error:?}").contains("signed account headers are required"),
            "authentication must precede JSON parsing: {error:?}"
        );
    }
}
#[tokio::test]
async fn create_vpn_quote_rejects_non_hex_metering_key() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x5D);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
    let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
        exit_class: "standard".to_owned(),
        metering_public_key_hex: "not-hex".to_owned(),
    })
    .expect("quote body");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());
    let error = handle_create_vpn_quote(app, &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("bad metering key must fail");
    assert!(format!("{error:?}").contains("metering_public_key_hex"));
}
#[tokio::test]
async fn create_vpn_session_rejects_quote_owned_by_different_account() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let user_keys = checked_vpn_ed25519_keypair(0x5F);
    let other_keys = checked_vpn_ed25519_keypair(0x60);
    let user = account_id_for(&user_keys);
    let other = account_id_for(&other_keys);
    let app =
        vpn_enabled_app_with_operator(world_with_accounts(&[user.clone(), other.clone()]), &user);
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &user, &user_keys, "standard").await;
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
    let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
        exit_class: quote.exit_class.clone(),
        quote_id: quote.quote_id.clone(),
        payment_tx_hash: quote.quote_id.clone(),
        metering_public_key_hex: metering_public_key_hex(&metering_keys),
    })
    .expect("session body");
    let headers = signed_app_headers(&other, &other_keys, &method, &uri, body.as_ref());
    let error = handle_create_vpn_session(app.clone(), &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("wrong account must not consume quote");
    assert!(format!("{error:?}").contains("different account"));
    assert!(app.vpn_quotes.contains_key(&quote.quote_id));
    let runtime = lock_vpn_runtime(&app);
    assert_eq!(
        runtime.quote_ids_by_account.get(&user),
        Some(&quote.quote_id)
    );
}
#[tokio::test]
async fn create_vpn_session_rejects_exit_class_mismatch() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x61);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &account, &key_pair, "low-latency").await;
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
    let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
        exit_class: "standard".to_owned(),
        quote_id: quote.quote_id.clone(),
        payment_tx_hash: quote.quote_id.clone(),
        metering_public_key_hex: metering_public_key_hex(&metering_keys),
    })
    .expect("session body");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());
    let error = handle_create_vpn_session(app, &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("exit class mismatch must fail");
    assert!(format!("{error:?}").contains("exit class does not match"));
}
#[tokio::test]
async fn create_vpn_session_rejects_metering_key_mismatch() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x62);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let (quote, _metering_keys) =
        create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
    let wrong_metering_keys = checked_vpn_ed25519_keypair(0x63);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
    let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
        exit_class: quote.exit_class.clone(),
        quote_id: quote.quote_id.clone(),
        payment_tx_hash: quote.quote_id.clone(),
        metering_public_key_hex: metering_public_key_hex(&wrong_metering_keys),
    })
    .expect("session body");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());
    let error = handle_create_vpn_session(app, &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("metering key mismatch must fail");
    assert!(format!("{error:?}").contains("metering key does not match"));
}
#[tokio::test]
async fn create_vpn_session_rejects_empty_payment_hash() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x64);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
    let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
        exit_class: quote.exit_class.clone(),
        quote_id: quote.quote_id.clone(),
        payment_tx_hash: String::new(),
        metering_public_key_hex: metering_public_key_hex(&metering_keys),
    })
    .expect("session body");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());
    let error = handle_create_vpn_session(app, &method, &uri, &headers, body.as_ref())
        .await
        .expect_err("empty payment hash must fail");
    assert!(format!("{error:?}").contains("payment_tx_hash must not be empty"));
}
#[tokio::test]
async fn create_and_get_vpn_session_preserve_the_active_lease() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x65);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &account, &key_pair, "low-latency").await;
    assert_ne!(quote.lease_id_hex, quote.quote_id);
    assert_eq!(quote.lease_id_hex.len(), 64);
    assert_eq!(quote.session_id_hex.len(), 32);
    let open_payload = hex::decode(&quote.open_lease_instruction.payload_hex).expect("open hex");
    let decoded_open = iroha_data_model::isi::decode_instruction_from_pair(
        &quote.open_lease_instruction.wire_id,
        &open_payload,
    )
    .expect("decode open vpn lease instruction");
    let open = decoded_open
        .as_any()
        .downcast_ref::<OpenVpnLeaseEscrow>()
        .expect("open vpn lease instruction");
    assert_eq!(open.quote.body.asset_definition, xor_asset_definition_id());
    let quote_record = app.vpn_quotes.get(&quote.quote_id).expect("stored quote");
    assert!(open_lease_matches_quote(open, quote_record.value()).expect("open lease shape"));
    drop(quote_record);
    let session =
        create_session_for_quote(app.clone(), &account, &key_pair, &quote, &metering_keys).await;
    assert_eq!(session.account_id, account.to_string());
    assert_eq!(session.exit_class, "low-latency");
    assert_eq!(session.status, "active");
    assert_eq!(session.quote_id, quote.quote_id);
    assert_eq!(session.payment_tx_hash, quote.quote_id);
    assert!(!session.helper_ticket_hex.is_empty());
    assert_eq!(session.tunnel_addresses.len(), 2);
    assert_eq!(app.vpn_sessions.len(), 1);
    let get_method = Method::GET;
    let get_uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
        .parse()
        .expect("get uri");
    let get_headers = signed_app_headers(&account, &key_pair, &get_method, &get_uri, &[]);
    let active = handle_get_vpn_session(
        app.clone(),
        &get_method,
        &get_uri,
        &get_headers,
        &session.session_id,
    )
    .await
    .expect("active")
    .into_response();
    let active_body: VpnSessionResponseDto = read_json(active).await;
    assert_eq!(active_body.session_id, session.session_id);
    assert_eq!(active_body.connected_at_ms, session.connected_at_ms);
    assert_eq!(app.vpn_sessions.len(), 1);
}
#[tokio::test]
async fn vpn_session_routes_reject_noncanonical_identifiers() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0xB7);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let method = Method::GET;
    for invalid in [
        "AB".repeat(16),
        format!("0x{}", "ab".repeat(16)),
        "ab".repeat(15),
        "ab".repeat(32),
    ] {
        let uri: Uri = format!("/v1/vpn/sessions/{invalid}")
            .parse()
            .expect("invalid identifier still forms a URI");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, &[]);
        let error = handle_get_vpn_session(app.clone(), &method, &uri, &headers, &invalid)
            .await
            .expect_err("noncanonical session id must fail at route boundary");
        assert!(
            format!("{error:?}")
                .contains("session_id must be exactly 32 lowercase hexadecimal characters"),
            "unexpected error for {method} {invalid}: {error:?}"
        );
    }
}
#[tokio::test]
async fn get_vpn_session_reconstructs_active_record_from_wsv_after_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x66);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
    let session =
        create_session_for_quote(app.clone(), &account, &key_pair, &quote, &metering_keys).await;
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
    app.vpn_sessions.clear();
    let method = Method::GET;
    let uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
        .parse()
        .expect("get uri");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, &[]);
    let response = handle_get_vpn_session(app, &method, &uri, &headers, &session.session_id)
        .await
        .expect("wsv session")
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let body: VpnSessionResponseDto = read_json(response).await;
    assert_eq!(body.session_id, session.session_id);
    assert_eq!(body.account_id, account.to_string());
    assert_eq!(body.payment_tx_hash, session.payment_tx_hash);
    assert_eq!(body.helper_ticket_hex, session.helper_ticket_hex);
}
#[tokio::test]
async fn get_vpn_session_reconstruction_rejects_a_non_operator_node_signer() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0xB8);
    let account = account_id_for(&key_pair);
    let mut app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
    let session =
        create_session_for_quote(app.clone(), &account, &key_pair, &quote, &metering_keys).await;
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
    app.vpn_sessions.clear();
    Arc::get_mut(&mut app)
        .expect("temporary test app has no remaining external owners")
        .vpn_operator_signer = Some(checked_vpn_ed25519_keypair(0xB9));

    let method = Method::GET;
    let uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
        .parse()
        .expect("get uri");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, &[]);
    let error = handle_get_vpn_session(app, &method, &uri, &headers, &session.session_id)
        .await
        .expect_err("a non-operator Torii must not re-sign a helper ticket");
    assert!(format!("{error:?}").contains(
        "vpn operator account must match this Torii node's dedicated quote and helper-ticket signing key"
    ));
}
#[tokio::test]
async fn get_vpn_session_does_not_reconstruct_expired_wsv_lease() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x67);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
    let session =
        create_session_for_quote(app.clone(), &account, &key_pair, &quote, &metering_keys).await;
    let active_record = app
        .vpn_sessions
        .get(&session.session_id)
        .expect("active session")
        .clone();
    let mut lease_record =
        lease_record_from_session_record(&active_record, VpnLeaseStatusV1::Active, None);
    lease_record.expires_at_ms = now_ms().saturating_sub(1);
    lease_record.opened_at_ms = lease_record.expires_at_ms.saturating_sub(10_000);
    resign_lease_quote_projection(&mut lease_record);
    app.state.insert_vpn_lease_for_testing(lease_record);
    app.vpn_sessions.clear();
    let method = Method::GET;
    let uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
        .parse()
        .expect("get uri");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, &[]);
    let response = handle_get_vpn_session(app, &method, &uri, &headers, &session.session_id)
        .await
        .expect("expired wsv session")
        .into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn get_vpn_session_does_not_reconstruct_non_active_wsv_lease() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x68);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
    let session =
        create_session_for_quote(app.clone(), &account, &key_pair, &quote, &metering_keys).await;
    let active_record = app
        .vpn_sessions
        .get(&session.session_id)
        .expect("active session")
        .clone();
    app.state
        .insert_vpn_lease_for_testing(lease_record_from_session_record(
            &active_record,
            VpnLeaseStatusV1::Refunded,
            None,
        ));
    app.vpn_sessions.clear();
    let method = Method::GET;
    let uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
        .parse()
        .expect("get uri");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, &[]);
    let response = handle_get_vpn_session(app, &method, &uri, &headers, &session.session_id)
        .await
        .expect("terminal wsv session")
        .into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn get_vpn_session_rejects_wrong_account_after_wsv_cache_loss() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let owner_keys = checked_vpn_ed25519_keypair(0x69);
    let intruder_keys = checked_vpn_ed25519_keypair(0x6A);
    let owner = account_id_for(&owner_keys);
    let intruder = account_id_for(&intruder_keys);
    let app = vpn_enabled_app_with_operator(
        world_with_accounts(&[owner.clone(), intruder.clone()]),
        &owner,
    );
    let (quote, metering_keys) =
        create_quote_for_account(app.clone(), &owner, &owner_keys, "standard").await;
    let session =
        create_session_for_quote(app.clone(), &owner, &owner_keys, &quote, &metering_keys).await;
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
    app.vpn_sessions.clear();
    let method = Method::GET;
    let uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
        .parse()
        .expect("get uri");
    let headers = signed_app_headers(&intruder, &intruder_keys, &method, &uri, &[]);
    let error = handle_get_vpn_session(app, &method, &uri, &headers, &session.session_id)
        .await
        .expect_err("wrong account must fail");
    assert!(format!("{error:?}").contains("different account"));
}
#[tokio::test]
async fn vpn_quote_create_rejects_replayed_nonce() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x6B);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let method = Method::POST;
    let uri: Uri = "/v1/vpn/quotes".parse().expect("uri");
    let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
        exit_class: "standard".to_owned(),
        metering_public_key_hex: metering_public_key_hex(&checked_vpn_ed25519_keypair(0x6C)),
    })
    .expect("body");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());
    let first = handle_create_vpn_quote(app.clone(), &method, &uri, &headers, body.as_ref())
        .await
        .expect("first")
        .into_response();
    assert_eq!(first.status(), StatusCode::CREATED);
    let error = match handle_create_vpn_quote(app, &method, &uri, &headers, body.as_ref()).await {
        Ok(_) => panic!("replayed request should fail"),
        Err(error) => error,
    };
    assert!(format!("{error:?}").contains("nonce already used"));
}
#[tokio::test]
async fn recreating_session_does_not_fabricate_a_financial_receipt() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x6F);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let (first_quote, first_metering_keys) =
        create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
    let _ = create_session_for_quote(
        app.clone(),
        &account,
        &key_pair,
        &first_quote,
        &first_metering_keys,
    )
    .await;
    let (second_quote, second_metering_keys) =
        create_quote_for_account(app.clone(), &account, &key_pair, "low-latency").await;
    let _ = create_session_for_quote(
        app.clone(),
        &account,
        &key_pair,
        &second_quote,
        &second_metering_keys,
    )
    .await;
    let receipts_method = Method::GET;
    let receipts_uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let receipts_headers =
        signed_app_headers(&account, &key_pair, &receipts_method, &receipts_uri, &[]);
    let receipts = handle_list_vpn_receipts(
        app.clone(),
        &receipts_method,
        &receipts_uri,
        &receipts_headers,
    )
    .await
    .expect("receipts")
    .into_response();
    let receipts_body: VpnReceiptListResponseDto = read_json(receipts).await;
    assert_eq!(receipts_body.total, 0);
    assert_eq!(app.vpn_sessions.len(), 1);
}
#[tokio::test]
async fn list_vpn_receipts_reconstructs_settled_records_from_wsv() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let key_pair = checked_vpn_ed25519_keypair(0x70);
    let account = account_id_for(&key_pair);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    let record = sample_session_record(&account);
    let ended_at_ms = record.connected_at_ms + 10_000;
    let voucher = VpnUsageVoucherV1::try_sign(
        VpnUsageVoucherBodyV1 {
            session_id: parse_vpn_session_id_hex(&record.session_id).expect("fixture session id"),
            quote_id: decode_hex_32(&record.quote_id, "quote").expect("quote"),
            relay_id: record.relay_id,
            sequence: 7,
            ingress_bytes: 128,
            egress_bytes: 256,
            active_ms: 10_000,
            issued_at_ms: ended_at_ms,
        },
        checked_vpn_ed25519_keypair(0x54).private_key(),
    )
    .expect("sign settled projection voucher");
    let earned_fee = record
        .tariff
        .fee_for_usage(128, 256, 10_000)
        .expect("settled projection tariff arithmetic");
    let relay_receipt = sign_test_relay_receipt(VpnSessionReceiptV1 {
        session_id: parse_vpn_session_id_hex(&record.session_id).expect("fixture session id"),
        quote_id: decode_hex_32(&record.quote_id, "quote").expect("quote"),
        payment_tx_hash: decode_hex_32(&record.payment_tx_hash, "payment").expect("payment"),
        account_hash: account_hash(&account),
        relay_id: record.relay_id,
        ingress_bytes: 128,
        egress_bytes: 256,
        cover_bytes: 0,
        uptime_secs: 10,
        started_at_ms: record.connected_at_ms,
        ended_at_ms,
        exit_class: VpnExitClassV1::Standard,
        meter_hash: vpn_tariff_meter_hash_v1(&record.tariff),
        earned_fee: earned_fee.clone(),
        highest_voucher_sequence: voucher.body.sequence,
        client_voucher_hash: voucher.hash(),
    });
    let lease = lease_record_from_session_record(
        &record,
        VpnLeaseStatusV1::Settled,
        Some((relay_receipt, voucher)),
    );
    let mut stale_local = receipt_record_from_settled_lease(&lease)
        .expect("settled fixture projection")
        .expect("settled fixture receipt");
    stale_local.status = "settlement_pending".to_owned();
    stale_local.receipt_source = "relay".to_owned();
    stale_local.earned_fee = Quantity::zero();
    stale_local.refunded_fee = stale_local.lease_fee.clone();
    store_receipt(&app, stale_local);
    app.state.insert_vpn_lease_for_testing(lease);
    let method = Method::GET;
    let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
    let headers = signed_app_headers(&account, &key_pair, &method, &uri, &[]);
    let response = handle_list_vpn_receipts(app, &method, &uri, &headers)
        .await
        .expect("receipts")
        .into_response();
    let body: VpnReceiptListResponseDto = read_json(response).await;
    assert_eq!(body.total, 1);
    assert_eq!(body.items[0].receipt_source, "wsv");
    assert_eq!(body.items[0].status, "settled");
    assert_eq!(body.items[0].earned_fee, earned_fee);
}
#[test]
fn settled_receipt_projection_rejects_tampered_client_evidence() {
    let account = checked_vpn_account(0x71);
    let mut lease = settled_lease_for_account(&account, 1);
    lease
        .settled_client_voucher
        .as_mut()
        .expect("settled fixture retains its client voucher")
        .body
        .ingress_bytes += 1;
    let error = receipt_record_from_settled_lease(&lease)
        .expect_err("receipt projection must reauthenticate the retained client evidence");
    assert!(matches!(
        error,
        Error::AppServiceUnavailable {
            code: "vpn_state_inconsistent",
            ..
        }
    ));
}
#[test]
fn list_vpn_receipts_uses_bounded_account_projection() {
    let account = checked_vpn_account(0x74);
    let unrelated_account = checked_vpn_account(0x75);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    for ordinal in 1..=30_u16 {
        app.state
            .insert_vpn_lease_for_testing(settled_lease_for_account(&account, ordinal));
    }
    for ordinal in 100..200_u16 {
        app.state
            .insert_vpn_lease_for_testing(settled_lease_for_account(&unrelated_account, ordinal));
    }
    let world = app.state.world_view();
    assert_eq!(
        world
            .vpn_settled_leases_by_account()
            .get(&account)
            .map(BTreeSet::len),
        Some(MAX_RECEIPTS_PER_ACCOUNT)
    );
    assert_eq!(
        world
            .vpn_settled_leases_by_account()
            .get(&unrelated_account)
            .map(BTreeSet::len),
        Some(MAX_RECEIPTS_PER_ACCOUNT)
    );
    drop(world);
    let receipts = list_receipts_for_account(&app, &account).expect("bounded receipt page");
    assert_eq!(receipts.len(), MAX_RECEIPTS_PER_ACCOUNT);
    assert_eq!(receipts[0].disconnected_at_ms, 10_030);
    assert_eq!(
        receipts[MAX_RECEIPTS_PER_ACCOUNT - 1].disconnected_at_ms,
        10_007
    );
    assert!(
        receipts
            .iter()
            .all(|receipt| receipt.account_id == account.to_string())
    );
}
#[test]
fn list_vpn_receipts_fails_closed_on_stale_projection() {
    let account = checked_vpn_account(0x76);
    let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
    app.state
        .insert_vpn_settled_lease_index_entry_for_testing(account.clone(), 1, [0xA5; 32]);
    let error = list_receipts_for_account(&app, &account)
        .expect_err("missing indexed lease must fail closed");
    assert!(matches!(
        error,
        Error::AppServiceUnavailable {
            code: "vpn_state_inconsistent",
            ..
        }
    ));
}
#[test]
fn list_vpn_receipts_cannot_reintroduce_a_global_lease_scan() {
    let source = include_str!("vpn.rs");
    let start = source
        .find("fn list_receipts_for_account(")
        .expect("receipt projection function");
    let tail = &source[start..];
    let end = tail
        .find("fn external_signed_transaction_results(")
        .expect("receipt projection terminator");
    let implementation = &tail[..end];
    assert!(implementation.contains("vpn_settled_leases_by_account()"));
    assert!(!implementation.contains("vpn_leases().iter()"));
}
#[test]
fn vpn_runtime_rejects_unsigned_quote_record_projections() {
    let account = checked_vpn_account(0xDF);
    let app = mk_app_state_for_tests_with_world(world_with_account(&account));
    let mut state = VpnRuntimeState::with_capacities(1, 1);
    let mut quote = sample_quote_record(&account, "df".repeat(32), u64::MAX);
    validate_quote_record_projection(&quote, app.state.network_id_ref())
        .expect("exact signed projection");
    quote.relay_tls_spki_sha256[0] ^= 1;
    let error = insert_quote_locked(&app, &mut state, quote, 0)
        .expect_err("an unsigned flat-field substitution must fail before caching");
    assert!(format!("{error:?}").contains("TLS SPKI"));
    assert!(app.vpn_quotes.is_empty());
    assert!(state.quote_ids_by_account.is_empty());
}
#[test]
fn vpn_runtime_rejects_a_valid_quote_from_a_different_exact_network() {
    let account = checked_vpn_account(0xE1);
    let app = mk_app_state_for_tests_with_world(world_with_account(&account));
    let mut quote = sample_quote_record(&account, "e1".repeat(32), u64::MAX);
    let mut body = quote.signed_quote.body.clone();
    let foreign_hash = HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0xE1; Hash::LENGTH]),
    );
    body.network_id = iroha_data_model::NetworkId::from_genesis_hash(foreign_hash);
    quote.signed_quote =
        VpnSignedQuoteV1::try_sign(body, checked_vpn_ed25519_keypair(0x7A).private_key())
            .expect("re-sign foreign-network VPN quote");
    let error = validate_quote_record_projection(&quote, app.state.network_id_ref())
        .expect_err("foreign-network VPN quote must fail before runtime caching");
    assert!(format!("{error:?}").contains("different exact network"));
}
#[test]
fn vpn_runtime_account_expiry_is_constant_and_isolated() {
    let target = checked_vpn_account(0xE0);
    let app = mk_app_state_for_tests_with_world(world_with_account(&target));
    let mut state = VpnRuntimeState::with_capacities(128, 128);
    let mut unrelated_quote_ids = Vec::new();
    let mut unrelated_sessions = Vec::new();
    for ordinal in 1..=64_usize {
        let account = checked_vpn_account(u8::try_from(ordinal).expect("fixture seed"));
        let quote_id = format!("{:064x}", ordinal.saturating_add(20_000));
        insert_quote_locked(
            &app,
            &mut state,
            sample_quote_record(&account, quote_id.clone(), u64::MAX),
            100,
        )
        .expect("unrelated quote");
        let session = sample_indexed_session_record(&account, ordinal, u64::MAX);
        insert_session_locked(&app, &mut state, session.clone(), 100).expect("unrelated session");
        unrelated_quote_ids.push(quote_id);
        unrelated_sessions.push((session.session_id, session.payment_tx_hash));
    }
    let target_quote_id = "aa".repeat(32);
    insert_quote_locked(
        &app,
        &mut state,
        sample_quote_record(&target, target_quote_id.clone(), 100),
        99,
    )
    .expect("target quote");
    let target_session = sample_indexed_session_record(&target, 1_000, 100);
    insert_session_locked(&app, &mut state, target_session.clone(), 99).expect("target session");
    state.quote_account_lookups = 0;
    state.session_account_lookups = 0;
    expire_quote_for_account_locked(&app, &mut state, &target, 100);
    expire_session_for_account_locked(&app, &mut state, &target, 100);
    assert_eq!(state.quote_account_lookups, 1);
    assert_eq!(state.session_account_lookups, 1);
    assert_eq!(app.vpn_quotes.len(), unrelated_quote_ids.len());
    assert_eq!(app.vpn_sessions.len(), unrelated_sessions.len());
    assert!(!app.vpn_quotes.contains_key(&target_quote_id));
    assert!(!app.vpn_sessions.contains_key(&target_session.session_id));
    for quote_id in unrelated_quote_ids {
        assert!(app.vpn_quotes.contains_key(&quote_id));
    }
    for (session_id, payment_hash) in unrelated_sessions {
        assert!(app.vpn_sessions.contains_key(&session_id));
        assert!(app.vpn_used_payments.contains_key(&payment_hash));
    }
    assert!(
        app.vpn_receipts.is_empty(),
        "cache expiry must not fabricate a financial receipt before an on-chain refund"
    );
}
#[test]
fn vpn_runtime_replacement_and_exact_remove_keep_indexes_consistent() {
    let account = checked_vpn_account(0xE1);
    let app = mk_app_state_for_tests_with_world(world_with_account(&account));
    let mut state = VpnRuntimeState::with_capacities(1, 1);
    let first_quote = sample_quote_record(&account, "11".repeat(32), u64::MAX);
    let second_quote = sample_quote_record(&account, "22".repeat(32), u64::MAX);
    insert_quote_locked(&app, &mut state, first_quote.clone(), 100).expect("first quote");
    insert_quote_locked(&app, &mut state, second_quote.clone(), 100).expect("replacement quote");
    assert_eq!(app.vpn_quotes.len(), 1);
    assert!(!app.vpn_quotes.contains_key(&first_quote.quote_id));
    assert_eq!(
        state.quote_ids_by_account.get(&account),
        Some(&second_quote.quote_id)
    );
    let removed_quote = remove_quote_by_id_locked(&app, &mut state, &second_quote.quote_id)
        .expect("exact quote remove");
    assert_eq!(removed_quote.quote_id, second_quote.quote_id);
    assert!(!state.quote_ids_by_account.contains_key(&account));
    let first_session = sample_indexed_session_record(&account, 1, u64::MAX);
    let second_session = sample_indexed_session_record(&account, 2, u64::MAX);
    insert_session_locked(&app, &mut state, first_session.clone(), 100).expect("first session");
    insert_session_locked(&app, &mut state, second_session.clone(), 200)
        .expect("replacement session");
    assert_eq!(app.vpn_sessions.len(), 1);
    assert!(!app.vpn_sessions.contains_key(&first_session.session_id));
    assert!(
        !app.vpn_used_payments
            .contains_key(&first_session.payment_tx_hash)
    );
    assert!(
        app.vpn_used_payments
            .contains_key(&second_session.payment_tx_hash)
    );
    assert_eq!(
        state.session_ids_by_account.get(&account),
        Some(&second_session.session_id)
    );
    assert!(
        app.vpn_receipts.is_empty(),
        "cache replacement must not fabricate a financial receipt"
    );
    let removed = remove_session_by_id_locked(&app, &mut state, &second_session.session_id)
        .expect("exact session remove");
    assert_eq!(removed.session_id, second_session.session_id);
    assert!(!state.session_ids_by_account.contains_key(&account));
    assert!(
        !app.vpn_used_payments
            .contains_key(&second_session.payment_tx_hash)
    );
}
#[test]
fn vpn_runtime_caps_fail_closed_without_evicting_unrelated_accounts() {
    let first = checked_vpn_account(0xE2);
    let second = checked_vpn_account(0xE3);
    let app = mk_app_state_for_tests_with_world(world_with_account(&first));
    let mut state = VpnRuntimeState::with_capacities(1, 1);
    let first_quote = sample_quote_record(&first, "31".repeat(32), u64::MAX);
    let second_quote = sample_quote_record(&second, "32".repeat(32), u64::MAX);
    insert_quote_locked(&app, &mut state, first_quote.clone(), 100).expect("first quote");
    let quote_error = insert_quote_locked(&app, &mut state, second_quote, 100)
        .expect_err("full quote cache must reject another account");
    assert!(format!("{quote_error:?}").contains("quote capacity"));
    assert_eq!(app.vpn_quotes.len(), 1);
    assert!(app.vpn_quotes.contains_key(&first_quote.quote_id));
    let first_session = sample_indexed_session_record(&first, 31, u64::MAX);
    let second_session = sample_indexed_session_record(&second, 32, u64::MAX);
    insert_session_locked(&app, &mut state, first_session.clone(), 100).expect("first session");
    let session_error = insert_session_locked(&app, &mut state, second_session, 100)
        .expect_err("full session cache must reject another account");
    assert!(format!("{session_error:?}").contains("session capacity"));
    assert_eq!(app.vpn_sessions.len(), 1);
    assert!(app.vpn_sessions.contains_key(&first_session.session_id));
    assert!(
        app.vpn_used_payments
            .contains_key(&first_session.payment_tx_hash)
    );
}

#[test]
fn vpn_runtime_capacity_reclaims_expired_accounts_from_expiry_indexes() {
    let expired_account = checked_vpn_account(0xE4);
    let fresh_account = checked_vpn_account(0xE5);
    let app = mk_app_state_for_tests_with_world(world_with_account(&expired_account));
    let mut state = VpnRuntimeState::with_capacities(1, 1);

    let expired_quote = sample_quote_record(&expired_account, "41".repeat(32), 100);
    insert_quote_locked(&app, &mut state, expired_quote.clone(), 99)
        .expect("unexpired first quote");
    let fresh_quote = sample_quote_record(&fresh_account, "42".repeat(32), u64::MAX);
    insert_quote_locked(&app, &mut state, fresh_quote.clone(), 100)
        .expect("expired quote capacity must be reclaimed");
    assert!(!app.vpn_quotes.contains_key(&expired_quote.quote_id));
    assert!(app.vpn_quotes.contains_key(&fresh_quote.quote_id));
    assert_eq!(state.quote_expirations.len(), 1);

    let expired_session = sample_indexed_session_record(&expired_account, 41, 100);
    insert_session_locked(&app, &mut state, expired_session.clone(), 99)
        .expect("unexpired first session");
    let fresh_session = sample_indexed_session_record(&fresh_account, 42, u64::MAX);
    insert_session_locked(&app, &mut state, fresh_session.clone(), 100)
        .expect("expired session capacity must be reclaimed");
    assert!(!app.vpn_sessions.contains_key(&expired_session.session_id));
    assert!(app.vpn_sessions.contains_key(&fresh_session.session_id));
    assert!(
        !app.vpn_used_payments
            .contains_key(&expired_session.payment_tx_hash)
    );
    assert!(
        app.vpn_used_payments
            .contains_key(&fresh_session.payment_tx_hash)
    );
    assert_eq!(state.session_expirations.len(), 1);
}

#[test]
fn vpn_runtime_reserved_expiry_survives_reclaim_and_later_frees_capacity() {
    let account = checked_vpn_account(0xE6);
    let replacement_account = checked_vpn_account(0xE7);
    let app = mk_app_state_for_tests_with_world(world_with_account(&account));
    let expires_at_ms = u64::MAX - 1;
    let session = sample_indexed_session_record(&account, 43, expires_at_ms);
    {
        let mut state = lock_vpn_runtime(&app);
        state.session_capacity = 1;
        insert_session_locked(
            &app,
            &mut state,
            session.clone(),
            expires_at_ms.saturating_sub(1),
        )
        .expect("session exists before its fixture expiry");
    }
    let reservation =
        VpnSettlementReservation::reserve(&app, session.session_id.clone()).expect("reservation");
    {
        let mut state = lock_vpn_runtime(&app);
        reclaim_expired_sessions_locked(&app, &mut state, expires_at_ms);
        assert!(app.vpn_sessions.contains_key(&session.session_id));
        assert!(
            state
                .session_expirations
                .contains(&(expires_at_ms, session.session_id.clone()))
        );
    }
    assert!(
        now_ms() < expires_at_ms,
        "fixture requires a simulated wall-clock rollback before reservation release"
    );
    drop(reservation);
    let mut state = lock_vpn_runtime(&app);
    assert!(app.vpn_sessions.contains_key(&session.session_id));
    assert!(
        state
            .session_expirations
            .contains(&(expires_at_ms, session.session_id.clone()))
    );

    let replacement = sample_indexed_session_record(&replacement_account, 44, u64::MAX);
    insert_session_locked(&app, &mut state, replacement.clone(), expires_at_ms)
        .expect("released expired reservation must free session capacity");
    assert!(!app.vpn_sessions.contains_key(&session.session_id));
    assert!(!state.session_ids_by_account.contains_key(&account));
    assert!(!app.vpn_used_payments.contains_key(&session.payment_tx_hash));
    assert_eq!(app.vpn_sessions.len(), 1);
    assert!(app.vpn_sessions.contains_key(&replacement.session_id));
}

#[test]
fn vpn_runtime_expiry_uses_indexes_without_global_cache_scans() {
    let source = include_str!("vpn.rs");
    let implementation = &source[..source
        .find("#[cfg(all(test, feature = \"app_api\"))]")
        .expect("VPN test module")];
    let compact = implementation
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(!implementation.contains("remove_existing_sessions_for_account"));
    assert!(!implementation.contains("allocate_session_id_and_address_plan"));
    assert!(!compact.contains("vpn_quotes.iter()"));
    assert!(!compact.contains("vpn_sessions.iter()"));
    assert!(implementation.contains("quote_expirations"));
    assert!(implementation.contains("session_expirations"));
}
#[tokio::test]
async fn vpn_address_derivation_separates_active_session_fixtures() {
    let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
    let first_keys = checked_vpn_ed25519_keypair(0x71);
    let second_keys = checked_vpn_ed25519_keypair(0x72);
    let third_keys = checked_vpn_ed25519_keypair(0x73);
    let first_account = account_id_for(&first_keys);
    let second_account = account_id_for(&second_keys);
    let third_account = account_id_for(&third_keys);
    let world = world_with_accounts(&[
        first_account.clone(),
        second_account.clone(),
        third_account.clone(),
    ]);
    let app = vpn_enabled_app_with_operator(world, &first_account);
    let (first_quote, first_metering_keys) =
        create_quote_for_account(app.clone(), &first_account, &first_keys, "standard").await;
    let first_session = create_session_for_quote(
        app.clone(),
        &first_account,
        &first_keys,
        &first_quote,
        &first_metering_keys,
    )
    .await;
    let (second_quote, second_metering_keys) =
        create_quote_for_account(app.clone(), &second_account, &second_keys, "standard").await;
    let second_session = create_session_for_quote(
        app.clone(),
        &second_account,
        &second_keys,
        &second_quote,
        &second_metering_keys,
    )
    .await;
    assert_ne!(
        first_session.tunnel_addresses,
        second_session.tunnel_addresses
    );
    let (third_quote, third_metering_keys) =
        create_quote_for_account(app.clone(), &third_account, &third_keys, "standard").await;
    let third_session = create_session_for_quote(
        app.clone(),
        &third_account,
        &third_keys,
        &third_quote,
        &third_metering_keys,
    )
    .await;
    assert_ne!(
        third_session.tunnel_addresses,
        second_session.tunnel_addresses
    );
    assert_eq!(app.vpn_sessions.len(), 3);
}
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
    let session =
        create_session_for_quote(app.clone(), &user, &user_keys, &quote, &metering_keys).await;
    let active_record = app
        .vpn_sessions
        .get(&session.session_id)
        .expect("active session")
        .clone();
    let mut fixture = receipt_fixture_for_session(&session, &active_record, &user, &metering_keys);
    let mut lease_record =
        lease_record_from_session_record(&active_record, VpnLeaseStatusV1::Active, None);
    let expires_at_ms = now_ms().saturating_sub(1_000);
    let issued_at_ms = expires_at_ms.saturating_sub(1);
    let opened_at_ms = issued_at_ms;
    lease_record.opened_at_ms = opened_at_ms;
    lease_record.expires_at_ms = expires_at_ms;
    lease_record.settlement_grace_ms = 60_000;
    resign_lease_quote_projection(&mut lease_record);
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
    app.vpn_sessions.clear();
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
    let session =
        create_session_for_quote(app.clone(), &user, &user_keys, &quote, &metering_keys).await;
    let active_record = app
        .vpn_sessions
        .get(&session.session_id)
        .expect("active session")
        .clone();
    let fixture = receipt_fixture_for_session(&session, &active_record, &user, &metering_keys);
    let mut lease_record =
        lease_record_from_session_record(&active_record, VpnLeaseStatusV1::Active, None);
    lease_record.expires_at_ms = now_ms().saturating_sub(10_000);
    lease_record.opened_at_ms = lease_record.expires_at_ms.saturating_sub(10_000);
    lease_record.settlement_grace_ms = 1;
    resign_lease_quote_projection(&mut lease_record);
    app.state.insert_vpn_lease_for_testing(lease_record);
    app.vpn_sessions.clear();
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
    let mut lease = lease_record_by_id(&app, &fixture.lease_id).expect("active lease");
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
    let mut lease = lease_record_by_id(&app, &fixture.lease_id).expect("active lease");
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
include!("vpn_tests_tail.rs");
