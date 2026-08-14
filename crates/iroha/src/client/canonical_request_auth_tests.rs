#[test]
fn canonical_query_string_sorts_and_encodes() {
    let canonical = Client::canonical_query_string(Some("b=2&a=3&b=1&space=a+b"))
        .expect("query is within canonical V1 bounds");
    assert_eq!(canonical, "a=3&b=1&b=2&space=a+b");
}
#[test]
fn canonical_query_string_matches_v1_lossy_form_safe_set() {
    let canonical = Client::canonical_query_string(Some(
        "b=%FF&a=%E2%82%AC&literal=%GG&space=a+b&safe=AZaz09*-._~&&empty",
    ))
    .expect("query is within canonical V1 bounds");
    assert_eq!(
        canonical,
        "a=%E2%82%AC&b=%EF%BF%BD&empty=&literal=%25GG&safe=AZaz09*-._%7E&space=a+b"
    );
}
#[test]
fn canonical_request_target_v1_limits_accept_exact_and_reject_plus_one() {
    let network_id = client_with_base_url(base_url()).network_id;
    let exact_method = HttpMethod::from_bytes(&vec![b'A'; CANONICAL_REQUEST_MAX_METHOD_BYTES_V1])
        .expect("exact method token");
    let excessive_method =
        HttpMethod::from_bytes(&vec![b'A'; CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 + 1])
            .expect("plus-one method token");
    let base_url = Url::parse("http://127.0.0.1/").expect("base URL");
    canonical_network_request_message(&network_id, &exact_method, &base_url, &[])
        .expect("exact method bound");
    assert!(
        canonical_network_request_message(&network_id, &excessive_method, &base_url, &[]).is_err()
    );

    let exact_path = format!(
        "http://127.0.0.1/{}",
        "x".repeat(CANONICAL_REQUEST_MAX_PATH_BYTES_V1 - 1)
    );
    let exact_path = Url::parse(&exact_path).expect("exact path URL");
    canonical_network_request_message(&network_id, &HttpMethod::GET, &exact_path, &[])
        .expect("exact path bound");
    let excessive_path = format!(
        "http://127.0.0.1/{}",
        "x".repeat(CANONICAL_REQUEST_MAX_PATH_BYTES_V1)
    );
    let excessive_path = Url::parse(&excessive_path).expect("plus-one path URL");
    assert!(
        canonical_network_request_message(&network_id, &HttpMethod::GET, &excessive_path, &[],)
            .is_err()
    );
}
#[test]
fn canonical_request_query_v1_limits_accept_exact_and_reject_plus_one() {
    let network_id = client_with_base_url(base_url()).network_id;
    let exact_pairs = std::iter::repeat_n("key=value", CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1)
        .collect::<Vec<_>>()
        .join("&");
    let exact_pairs = Url::parse(&format!("http://127.0.0.1/v1/test?{exact_pairs}"))
        .expect("exact query-pair URL");
    canonical_network_request_message(&network_id, &HttpMethod::GET, &exact_pairs, &[])
        .expect("exact query-pair bound");
    let excessive_pairs =
        std::iter::repeat_n("key=value", CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 + 1)
            .collect::<Vec<_>>()
            .join("&");
    let excessive_pairs = Url::parse(&format!("http://127.0.0.1/v1/test?{excessive_pairs}"))
        .expect("plus-one query-pair URL");
    assert!(
        canonical_network_request_message(&network_id, &HttpMethod::GET, &excessive_pairs, &[],)
            .is_err()
    );

    let exact_raw = "x".repeat(CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1);
    let exact_raw =
        Url::parse(&format!("http://127.0.0.1/v1/test?{exact_raw}")).expect("exact raw-query URL");
    canonical_network_request_message(&network_id, &HttpMethod::GET, &exact_raw, &[])
        .expect("exact raw-query bound");
    let excessive_raw = "x".repeat(CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 + 1);
    let excessive_raw = Url::parse(&format!("http://127.0.0.1/v1/test?{excessive_raw}"))
        .expect("plus-one raw-query URL");
    assert!(
        canonical_network_request_message(&network_id, &HttpMethod::GET, &excessive_raw, &[],)
            .is_err()
    );
}
#[test]
fn canonical_request_nonce_and_account_v1_limits_accept_exact_and_reject_plus_one() {
    let client = client_with_base_url(base_url());
    let url = Url::parse("http://127.0.0.1/v1/test").expect("test URL");
    canonical_network_request_signature_message(
        &client.network_id,
        &HttpMethod::GET,
        &url,
        &[],
        42,
        &"!".repeat(CANONICAL_REQUEST_MAX_NONCE_BYTES_V1),
    )
    .expect("exact nonce bound");
    assert!(
        canonical_network_request_signature_message(
            &client.network_id,
            &HttpMethod::GET,
            &url,
            &[],
            42,
            &"!".repeat(CANONICAL_REQUEST_MAX_NONCE_BYTES_V1 + 1),
        )
        .is_err()
    );
    for invalid in ["", "embedded space", "non-ascii-λ"] {
        assert!(
            canonical_network_request_signature_message(
                &client.network_id,
                &HttpMethod::GET,
                &url,
                &[],
                42,
                invalid,
            )
            .is_err()
        );
    }

    let exact_account = format!(
        "0x{}",
        "a".repeat(CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 - 2)
    );
    validate_canonical_request_account_literal(&exact_account).expect("exact account bound");
    let excessive_account = format!(
        "0x{}",
        "a".repeat(CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 - 1)
    );
    assert!(validate_canonical_request_account_literal(&excessive_account).is_err());
}
#[test]
fn canonical_request_signature_and_witness_v1_limits_are_checked() {
    let exact_signature =
        Signature::from_bytes(&vec![0x11; CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1]);
    canonical_request_signature_header_value(&exact_signature)
        .expect("exact detached-signature payload bound");
    let excessive_signature =
        Signature::from_bytes(&vec![0x11; CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1 + 1]);
    assert!(canonical_request_signature_header_value(&excessive_signature).is_err());

    let client = client_with_base_url(base_url());
    let entry = iroha_data_model::soracloud::CanonicalRequestSignatureWitnessV1 {
        signer: client.key_pair.public_key().clone(),
        signature: Signature::from_bytes(&[0x11; 64]),
    };
    let witness = |signature_count| CanonicalRequestWitnessV1 {
        schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
        subject_account: client.account.clone(),
        timestamp_ms: 42,
        nonce: "bounded-witness".to_owned(),
        canonical_request_hash: Hash::new(b"bounded witness request"),
        signatures: vec![entry.clone(); signature_count],
    };
    canonical_request_witness_header_value(&witness(CANONICAL_REQUEST_WITNESS_MAX_SIGNATURES_V1))
        .expect("exact witness signature-count bound");
    assert!(
        canonical_request_witness_header_value(&witness(
            CANONICAL_REQUEST_WITNESS_MAX_SIGNATURES_V1 + 1,
        ))
        .is_err()
    );

    let exact_witness_bytes = vec![0x11; CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1];
    let exact_witness_header = encode_bounded_canonical_base64_value(
        &exact_witness_bytes,
        CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1,
        "test witness",
    )
    .expect("exact witness byte bound");
    assert_eq!(
        exact_witness_header.len(),
        CANONICAL_REQUEST_WITNESS_MAX_HEADER_BYTES_V1
    );
    let excessive_witness_bytes = vec![0x11; CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1 + 1];
    assert!(
        encode_bounded_canonical_base64_value(
            &excessive_witness_bytes,
            CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1,
            "test witness",
        )
        .is_err()
    );
}
