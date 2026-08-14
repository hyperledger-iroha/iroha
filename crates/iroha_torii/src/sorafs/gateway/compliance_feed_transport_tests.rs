#[test]
fn feed_transport_response_debug_is_payload_free() {
    let mut response = fetch_response(b"PRIVATE-FEED-BODY".to_vec());
    response.redirect_location = Some("https://feed.example/PRIVATE-REDIRECT".into());
    let debug = format!("{response:?}");
    assert!(debug.contains("body_bytes"));
    assert!(debug.contains("redirect_location_bytes"));
    assert!(!debug.contains("PRIVATE-FEED-BODY"));
    assert!(!debug.contains("PRIVATE-REDIRECT"));
}
#[test]
fn feed_fetch_rejects_private_dns_and_rebinding() {
    let policy = feed_policy();
    let private = ScriptedTransport {
        resolutions: Mutex::new(VecDeque::from([vec![
            "127.0.0.1".parse().expect("private IP"),
        ]])),
        response: fetch_response(Vec::new()),
    };
    assert!(matches!(
        fetch_feed_bytes(
            &policy,
            GatewayComplianceFetchLimits::default(),
            &test_feed_transport_identity(),
            &private,
        ),
        Err(GatewayComplianceError::NonPublicAddress)
    ));
    let rebinding = ScriptedTransport {
        resolutions: Mutex::new(VecDeque::from([
            vec!["93.184.216.34".parse().expect("public IP")],
            vec!["93.184.216.35".parse().expect("public IP")],
        ])),
        response: fetch_response(Vec::new()),
    };
    assert!(matches!(
        fetch_feed_bytes(
            &policy,
            GatewayComplianceFetchLimits::default(),
            &test_feed_transport_identity(),
            &rebinding,
        ),
        Err(GatewayComplianceError::DnsRebinding)
    ));
}
#[test]
fn feed_fetch_rejects_wrong_trust_pin_and_decompression_bomb() {
    let policy = feed_policy();
    let mut wrong_pin_response = fetch_response(Vec::new());
    wrong_pin_response.peer_spki_sha256 = [0x99; 32];
    let wrong_pin = ScriptedTransport {
        resolutions: Mutex::new(VecDeque::from([vec![
            "93.184.216.34".parse().expect("public IP"),
        ]])),
        response: wrong_pin_response,
    };
    assert!(matches!(
        fetch_feed_bytes(
            &policy,
            GatewayComplianceFetchLimits::default(),
            &test_feed_transport_identity(),
            &wrong_pin,
        ),
        Err(GatewayComplianceError::TrustPinMismatch)
    ));
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&vec![0x41; 4_096]).expect("gzip write");
    let compressed = encoder.finish().expect("gzip finish");
    assert!(matches!(
        decompress_bounded(&compressed, GatewayComplianceContentEncoding::Gzip, 128),
        Err(GatewayComplianceError::ResourceLimit { .. })
    ));
}
#[test]
fn feed_fetch_rejects_redirect_outside_exact_allowlist() {
    let policy = feed_policy();
    let mut response = fetch_response(Vec::new());
    response.status = 302;
    response.redirect_location = Some("https://mirror.example/catalog".into());
    let redirect = ScriptedTransport {
        resolutions: Mutex::new(VecDeque::from([
            vec!["93.184.216.34".parse().expect("public IP")],
            vec!["93.184.216.34".parse().expect("public IP")],
        ])),
        response,
    };
    assert!(matches!(
        fetch_feed_bytes(
            &policy,
            GatewayComplianceFetchLimits::default(),
            &test_feed_transport_identity(),
            &redirect,
        ),
        Err(GatewayComplianceError::UnsafeUrl(_))
    ));
}
