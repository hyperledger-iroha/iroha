// Exact-network Soracloud HTTP authentication regressions included by the parent test module.

#[test]
fn build_soracloud_mutation_auth_headers_adds_single_sig_freshness_headers() {
    let config = crate::fallback_config();
    let endpoint =
        reqwest::Url::parse("http://127.0.0.1:8080/v1/soracloud/deploy").expect("endpoint");
    let headers = build_soracloud_mutation_auth_headers(&config, &endpoint, br#"{"noop":true}"#)
        .expect("single-sig headers");
    let header_map: BTreeMap<_, _> = headers.into_iter().collect();

    assert_eq!(
        header_map.get(HEADER_IROHA_ACCOUNT),
        Some(&config.account.to_string())
    );
    assert!(header_map.contains_key(HEADER_IROHA_SIGNATURE));
    assert!(header_map.contains_key(HEADER_IROHA_TIMESTAMP_MS));
    assert!(header_map.contains_key(HEADER_IROHA_NONCE));
    assert!(!header_map.contains_key(HEADER_IROHA_WITNESS));

    let signature = Signature::try_from_bytes(
        &base64::engine::general_purpose::STANDARD
            .decode(
                header_map
                    .get(HEADER_IROHA_SIGNATURE)
                    .expect("signature header"),
            )
            .expect("base64 signature"),
    )
    .expect("canonical signature");
    let timestamp_ms = header_map
        .get(HEADER_IROHA_TIMESTAMP_MS)
        .expect("timestamp header")
        .parse::<u64>()
        .expect("canonical timestamp");
    let nonce = header_map.get(HEADER_IROHA_NONCE).expect("nonce header");
    let body = br#"{"noop":true}"#;
    let message = canonical_network_request_signature_message(
        &config.network_id,
        "POST",
        &endpoint,
        body,
        timestamp_ms,
        nonce,
    );
    signature
        .verify(config.key_pair.public_key(), &message)
        .expect("exact-network Soracloud signature verifies");
    let foreign_network = iroha::data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha::data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        Hash::new(b"same-label-foreign-soracloud-genesis"),
    ));
    let foreign_message = canonical_network_request_signature_message(
        &foreign_network,
        "POST",
        &endpoint,
        body,
        timestamp_ms,
        nonce,
    );
    signature
        .verify(config.key_pair.public_key(), &foreign_message)
        .expect_err("Soracloud signature must not verify for another genesis network");
}

#[test]
fn build_soracloud_mutation_auth_headers_reports_nonce_rng_failure() {
    let config = crate::fallback_config();
    let endpoint =
        reqwest::Url::parse("http://127.0.0.1:8080/v1/soracloud/deploy").expect("endpoint");
    let mut rng = FailingSoracloudSignatureNonceRng;

    let error = build_soracloud_mutation_auth_headers_with_rng(
        &config,
        &endpoint,
        br#"{"noop":true}"#,
        &mut rng,
    )
    .expect_err("signature nonce RNG failure");
    let message = format!("{error:?}");

    assert!(message.contains("Soracloud mutation signature nonce OS RNG failed"));
    assert!(message.contains("failing Soracloud signature nonce RNG"));
}

#[test]
fn build_soracloud_mutation_auth_headers_uses_witness_file_when_configured() {
    let mut config = crate::fallback_config();
    let endpoint =
        reqwest::Url::parse("http://127.0.0.1:8080/v1/soracloud/deploy").expect("endpoint");
    let body = br#"{"noop":true}"#;
    let witness = CanonicalRequestWitnessV1 {
        schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
        subject_account: config.account.clone(),
        timestamp_ms: 42,
        nonce: "fixture-witness".to_owned(),
        canonical_request_hash: canonical_network_request_hash(
            &config.network_id,
            "POST",
            &endpoint,
            body,
        ),
        signatures: Vec::new(),
    };
    let dir = temp_dir("witness_headers");
    let witness_path = dir.join("witness.json");
    fs::write(
        &witness_path,
        json::to_vec(&witness).expect("encode witness json"),
    )
    .expect("write witness file");
    config.soracloud_http_witness_file = Some(witness_path);

    let headers = build_soracloud_mutation_auth_headers(&config, &endpoint, body).expect("headers");
    let header_map: BTreeMap<_, _> = headers.into_iter().collect();

    assert_eq!(
        header_map.get(HEADER_IROHA_ACCOUNT),
        Some(&config.account.to_string())
    );
    assert!(header_map.contains_key(HEADER_IROHA_WITNESS));
    assert!(!header_map.contains_key(HEADER_IROHA_SIGNATURE));
    assert!(!header_map.contains_key(HEADER_IROHA_TIMESTAMP_MS));
    assert!(!header_map.contains_key(HEADER_IROHA_NONCE));
}
