#[test]
fn reputation_auth_headers_match_the_exact_canonical_preimage() {
    let auth = fixture_reputation_auth(0x31, 369);
    let endpoint =
        Url::parse("http://127.0.0.1/v1/sorafs/reputation/events?z=%2B&b=two+words&a=%7E&a=first")
            .expect("endpoint");
    let now = UNIX_EPOCH + Duration::from_millis(1_725_000_000_123);
    let mut rng = IncrementingReputationRng { next: 0x11 };
    let headers = reputation_request_headers_with_rng_at(&auth, &endpoint, now, &mut rng)
        .expect("signed headers");
    let expected_nonce = BASE64_URL_SAFE_NO_PAD.encode([0x11_u8; 12]);
    let expected_request = format!(
        "GET\n/v1/sorafs/reputation/events\na=first&a=%7E&b=two+words&z=%2B\n{}\n1725000000123\n{expected_nonce}",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let mut expected_message = b"iroha.app.request.network.v1\0".to_vec();
    expected_message.extend_from_slice(auth.network_id.as_bytes());
    expected_message.extend_from_slice(expected_request.as_bytes());
    assert_eq!(headers.account_header_value, auth.account_header_value);
    assert!(headers.account_header_value.is_ascii());
    assert_eq!(headers.timestamp_ms, 1_725_000_000_123);
    assert_eq!(headers.nonce, expected_nonce);
    assert_eq!(
        canonical_reputation_request_message(
            &auth.network_id,
            &endpoint,
            headers.timestamp_ms,
            &headers.nonce,
        )
        .expect("canonical reputation request"),
        expected_message
    );
    let signature_bytes = BASE64_STANDARD
        .decode(&headers.signature_base64)
        .expect("standard base64 signature");
    assert_eq!(
        BASE64_STANDARD.encode(&signature_bytes),
        headers.signature_base64
    );
    let signature =
        Signature::try_from_bytes(&signature_bytes).expect("admissible signature payload");
    signature
        .verify(auth.key_pair.public_key(), &expected_message)
        .expect("signature verifies exact canonical request");
    let mutated_path =
        Url::parse("http://127.0.0.1/v1/sorafs/reputation/latest").expect("mutated path");
    let mutated_query =
        Url::parse("http://127.0.0.1/v1/sorafs/reputation/events?limit=2").expect("mutated query");
    let foreign_network =
        "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("foreign canonical network identity");
    for mutated in [
        canonical_reputation_request_message(
            &auth.network_id,
            &mutated_path,
            headers.timestamp_ms,
            &headers.nonce,
        )
        .expect("mutated path request"),
        canonical_reputation_request_message(
            &auth.network_id,
            &mutated_query,
            headers.timestamp_ms,
            &headers.nonce,
        )
        .expect("mutated query request"),
        canonical_reputation_request_message(
            &auth.network_id,
            &endpoint,
            headers.timestamp_ms + 1,
            &headers.nonce,
        )
        .expect("mutated timestamp request"),
        canonical_reputation_request_message(
            &auth.network_id,
            &endpoint,
            headers.timestamp_ms,
            "mutated-nonce",
        )
        .expect("mutated nonce request"),
        canonical_reputation_request_message(
            &foreign_network,
            &endpoint,
            headers.timestamp_ms,
            &headers.nonce,
        )
        .expect("foreign network request"),
    ] {
        assert!(
            signature
                .verify(auth.key_pair.public_key(), &mutated)
                .is_err(),
            "signature must bind every canonical request component"
        );
    }
}

#[test]
fn reputation_auth_requires_an_exact_network_identity() {
    let directory = tempdir().expect("tempdir");
    let path = directory.path().join("reputation.key");
    let key_pair = fixture_keypair(0x43);
    write_reputation_private_key(&path, &key_pair);
    let account = AccountId::new(key_pair.public_key().clone())
        .to_i105_for_discriminant(369)
        .expect("canonical account literal");
    let error = load_reputation_request_auth(
        Some(account),
        Some(path),
        None,
        "sorafs_cli reputation snapshot",
    )
    .err()
    .expect("network identity is mandatory");
    assert!(error.contains("--network-id=GENESIS_HASH"));
}

#[test]
fn reputation_canonical_request_enforces_v1_target_and_freshness_bounds() {
    let network_id = fixture_reputation_network_id();
    let exact_nonce = "n".repeat(REPUTATION_CANONICAL_MAX_NONCE_BYTES_V1);
    let endpoint = Url::parse("https://torii.example/v1/sorafs/reputation/events?a=1")
        .expect("bounded reputation endpoint");
    canonical_reputation_request_message(&network_id, &endpoint, 1, &exact_nonce)
        .expect("exact nonce bound");
    assert!(
        canonical_reputation_request_message(&network_id, &endpoint, 1, &(exact_nonce + "n"),)
            .is_err()
    );

    let exact_pairs = (0..REPUTATION_CANONICAL_MAX_QUERY_PAIRS_V1)
        .map(|index| format!("k{index}=v"))
        .collect::<Vec<_>>()
        .join("&");
    ReputationFormPlan::new(&exact_pairs).expect("exact query-pair bound");
    assert!(ReputationFormPlan::new(&(exact_pairs + "&extra=v")).is_err());
}
