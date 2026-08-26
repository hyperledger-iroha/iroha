// Exact-network Soracloud HTTP authentication regressions included by the parent test module.
use base64::Engine as _;

#[test]
fn post_torii_mutation_rejects_invalid_url() {
    let payload = norito::json!({ "noop": true });
    let err = post_torii_soracloud_mutation("not-a-url", "v1/soracloud/deploy", &payload, None, 5)
        .expect_err("invalid URL must fail");
    assert!(err.to_string().contains("invalid --torii-url"));
}

#[test]
fn build_soracloud_mutation_auth_headers_adds_single_sig_freshness_headers() {
    let config = crate::fallback_config();
    let endpoint =
        reqwest::Url::parse("http://127.0.0.1:8080/v1/soracloud/deploy").expect("endpoint");
    let headers = build_soracloud_mutation_auth_headers(&config, &endpoint, br#"{"noop":true}"#)
        .expect("single-sig headers");
    assert_eq!(
        headers.expected_response_signers,
        vec![config.key_pair.public_key().clone()]
    );
    let header_map: BTreeMap<_, _> = headers.headers.into_iter().collect();
    assert_eq!(
        header_map.get(HEADER_IROHA_ACCOUNT),
        Some(
            &config
                .account
                .to_canonical_hex()
                .expect("fixture account has canonical hexadecimal form")
        )
    );
    let account_header = header_map
        .get(HEADER_IROHA_ACCOUNT)
        .expect("account header");
    assert!(account_header.is_ascii());
    assert!(account_header.starts_with("0x"));
    assert!(
        account_header[2..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
        &reqwest::Method::POST,
        &endpoint,
        body,
        timestamp_ms,
        nonce,
    )
    .expect("bounded canonical signature message");
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
        &reqwest::Method::POST,
        &endpoint,
        body,
        timestamp_ms,
        nonce,
    )
    .expect("bounded foreign-network signature message");
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
    assert!(message.contains("Soracloud request signature nonce OS RNG failed"));
    assert!(message.contains("failing Soracloud signature nonce RNG"));
}
#[test]
fn soracloud_auth_rejects_query_pair_plus_one_before_signing() {
    let config = crate::fallback_config();
    let query = std::iter::repeat_n("key=value", 65)
        .collect::<Vec<_>>()
        .join("&");
    let endpoint = reqwest::Url::parse(&format!(
        "http://127.0.0.1:8080/v1/soracloud/apps/status?{query}"
    ))
    .expect("endpoint");
    let error = build_soracloud_read_auth_headers(&config, &endpoint)
        .expect_err("one query pair beyond canonical V1 must fail");
    assert!(error.to_string().contains("64 pairs"));
}
#[test]
fn load_soracloud_http_witness_rejects_oversized_file_before_reading() {
    let dir = temp_dir("oversized_witness_file");
    let witness_path = dir.join("witness.json");
    let file = fs::File::create(&witness_path).expect("create sparse witness file");
    file.set_len(SORACLOUD_HTTP_WITNESS_FILE_MAX_BYTES_V1 + 1)
        .expect("size sparse witness file");
    drop(file);
    let error = load_soracloud_http_witness(&witness_path)
        .expect_err("oversized witness file must fail before reading");
    assert!(error.to_string().contains("witness file"));
    assert!(error.to_string().contains("exceeds the V1 limit"));
}
#[test]
fn build_soracloud_read_auth_headers_bind_exact_path_query_and_network() {
    let mut config = crate::fallback_config();
    config.soracloud_http_witness_file = Some(PathBuf::from("ignored-for-read-auth.json"));
    let endpoint = reqwest::Url::parse(
        "http://127.0.0.1:8080/v1/soracloud/apps/status?service=zeta&audit_limit=3",
    )
    .expect("endpoint");
    let headers = build_soracloud_read_auth_headers(&config, &endpoint)
        .expect("single-signature read headers");
    let header_map: BTreeMap<_, _> = headers.into_iter().collect();
    assert!(!header_map.contains_key(HEADER_IROHA_WITNESS));
    let signature = Signature::try_from_bytes(
        &base64::engine::general_purpose::STANDARD
            .decode(header_map.get(HEADER_IROHA_SIGNATURE).expect("signature"))
            .expect("base64 signature"),
    )
    .expect("canonical signature");
    let timestamp_ms = header_map[HEADER_IROHA_TIMESTAMP_MS]
        .parse::<u64>()
        .expect("timestamp");
    let nonce = &header_map[HEADER_IROHA_NONCE];
    let message =
        |network_id: &iroha::data_model::NetworkId, endpoint: &reqwest::Url, body: &[u8]| {
            canonical_network_request_signature_message(
                network_id,
                &reqwest::Method::GET,
                endpoint,
                body,
                timestamp_ms,
                nonce,
            )
            .expect("bounded canonical read signature message")
        };
    signature
        .verify(
            config.key_pair.public_key(),
            &message(&config.network_id, &endpoint, &[]),
        )
        .expect("exact GET path and query verify");
    let reordered = reqwest::Url::parse(
        "http://127.0.0.1:8080/v1/soracloud/apps/status?audit_limit=3&service=zeta",
    )
    .expect("reordered endpoint");
    signature
        .verify(
            config.key_pair.public_key(),
            &message(&config.network_id, &reordered, &[]),
        )
        .expect("canonical query ordering must verify");
    for altered in [
        reqwest::Url::parse(
            "http://127.0.0.1:8080/v1/soracloud/apps/other?service=zeta&audit_limit=3",
        )
        .expect("altered path"),
        reqwest::Url::parse(
            "http://127.0.0.1:8080/v1/soracloud/apps/status?service=zeta&audit_limit=4",
        )
        .expect("altered query"),
    ] {
        signature
            .verify(
                config.key_pair.public_key(),
                &message(&config.network_id, &altered, &[]),
            )
            .expect_err("altered request target must fail verification");
    }
    signature
        .verify(
            config.key_pair.public_key(),
            &message(&config.network_id, &endpoint, b"unexpected-body"),
        )
        .expect_err("GET body substitution must fail verification");
    let foreign_network = iroha::data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha::data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        Hash::new(b"same-label-foreign-soracloud-read-genesis"),
    ));
    signature
        .verify(
            config.key_pair.public_key(),
            &message(&foreign_network, &endpoint, &[]),
        )
        .expect_err("same-label foreign genesis must fail verification");
}
#[test]
fn protected_soracloud_get_fails_before_network_without_local_signer() {
    let previous = SORACLOUD_SUBMISSION_CONFIG.with(|slot| slot.borrow_mut().take());
    let error = fetch_torii_soracloud_status("http://127.0.0.1:1", None, Some("token"), 1)
        .expect_err("missing local signer must fail closed");
    SORACLOUD_SUBMISSION_CONFIG.with(|slot| *slot.borrow_mut() = previous);
    let message = format!("{error:#}");
    assert!(message.contains("protected GET requires an initialized local account signer"));
    assert!(message.contains("submission config is not initialized"));
    assert!(!message.contains("failed to fetch"));
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
            &reqwest::Method::POST,
            &endpoint,
            body,
        )
        .expect("bounded canonical witness request hash"),
        signatures: vec![CanonicalRequestSignatureWitnessV1 {
            signer: config.key_pair.public_key().clone(),
            signature: Signature::try_new(config.key_pair.private_key(), b"fixture witness")
                .expect("fixture witness signature"),
        }],
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
    assert_eq!(
        headers.expected_response_signers,
        vec![config.key_pair.public_key().clone()]
    );
    let header_map: BTreeMap<_, _> = headers.headers.into_iter().collect();
    assert!(!header_map.contains_key(HEADER_IROHA_ACCOUNT));
    assert!(header_map.contains_key(HEADER_IROHA_WITNESS));
    assert!(!header_map.contains_key(HEADER_IROHA_SIGNATURE));
    assert!(!header_map.contains_key(HEADER_IROHA_TIMESTAMP_MS));
    assert!(!header_map.contains_key(HEADER_IROHA_NONCE));
}

#[test]
fn build_soracloud_mutation_auth_headers_rejects_empty_witness_signer_set() {
    let mut config = crate::fallback_config();
    let endpoint =
        reqwest::Url::parse("http://127.0.0.1:8080/v1/soracloud/deploy").expect("endpoint");
    let body = br#"{"noop":true}"#;
    let witness = CanonicalRequestWitnessV1 {
        schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
        subject_account: config.account.clone(),
        timestamp_ms: 42,
        nonce: "empty-witness-signers".to_owned(),
        canonical_request_hash: canonical_network_request_hash(
            &config.network_id,
            &reqwest::Method::POST,
            &endpoint,
            body,
        )
        .expect("bounded canonical witness request hash"),
        signatures: Vec::new(),
    };
    let dir = temp_dir("empty_witness_signers");
    let witness_path = dir.join("witness.json");
    fs::write(
        &witness_path,
        json::to_vec(&witness).expect("encode witness json"),
    )
    .expect("write witness file");
    config.soracloud_http_witness_file = Some(witness_path);
    let error = build_soracloud_mutation_auth_headers(&config, &endpoint, body)
        .expect_err("empty canonical request witness signer set must fail");
    assert!(error.to_string().contains("at least one signature"));
}

#[test]
fn post_torii_mutation_does_not_follow_signed_body_redirects() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("redirect listener");
    let address = listener.local_addr().expect("redirect listener address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("initial mutation request");
        let request = read_mock_http_request(&mut stream);
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/soracloud/deploy");
        write!(
            stream,
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: /redirected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .expect("write redirect response");
        drop(stream);

        listener
            .set_nonblocking(true)
            .expect("nonblocking redirect listener");
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match listener.accept() {
                Ok((mut redirected, _)) => {
                    let request = read_mock_http_request(&mut redirected);
                    let _ = write!(
                        redirected,
                        "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    return Some(request);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return None;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("redirect listener failed: {error}"),
            }
        }
    });

    let previous =
        SORACLOUD_SUBMISSION_CONFIG.with(|slot| slot.replace(Some(crate::fallback_config())));
    let error = post_torii_soracloud_mutation(
        &format!("http://{address}"),
        "v1/soracloud/deploy",
        &norito::json!({ "noop": true }),
        None,
        1,
    )
    .expect_err("redirect response must not be followed");
    SORACLOUD_SUBMISSION_CONFIG.with(|slot| *slot.borrow_mut() = previous);

    assert!(error.to_string().contains("307 Temporary Redirect"));
    assert!(
        server.join().expect("redirect server").is_none(),
        "signed mutation body must not be replayed to a redirect target",
    );
}
