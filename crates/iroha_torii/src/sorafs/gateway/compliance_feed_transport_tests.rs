//! Governed compliance-feed transport boundary tests.

use super::*;

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

// Unknown-content-size raw frames exercise the streaming history-window check instead
// of the decoder's known-content-size single-pass shortcut. No encoder default is assumed.
fn raw_zstd_window_frame(window_log: u8, payload: &[u8]) -> Vec<u8> {
    assert!((10..=27).contains(&window_log));
    assert!(payload.len() <= (1_usize << window_log).min(128 * 1024));
    let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0, (window_log - 10) << 3];
    let block_header = (u32::try_from(payload.len()).expect("bounded raw block") << 3) | 1;
    frame.extend_from_slice(&block_header.to_le_bytes()[..3]);
    frame.extend_from_slice(payload);
    frame
}

#[test]
fn zstd_window_and_decoded_limit_accept_exact_boundaries() {
    for (window_log, maximum) in [(10, 1024), (10, 2048), (11, 2048)] {
        let payload = vec![0x41; 1_usize << window_log];
        let frame = raw_zstd_window_frame(window_log, &payload);
        assert_eq!(
            decompress_bounded(&frame, GatewayComplianceContentEncoding::Zstd, maximum)
                .expect("window at or below policy and output at its exact bound"),
            payload
        );
    }
}

#[test]
fn zstd_oversized_window_is_rejected_even_for_tiny_output() {
    let frame = raw_zstd_window_frame(11, b"{}");
    assert_eq!(frame.len(), 11);
    for maximum in [1024, 1536] {
        assert!(matches!(
            decompress_bounded(&frame, GatewayComplianceContentEncoding::Zstd, maximum),
            Err(GatewayComplianceError::Decompression(_))
        ));
    }
}

#[test]
fn zstd_minimum_window_keeps_smaller_decoded_byte_limits_exact() {
    for maximum in [1, 128, 1023] {
        let payload = vec![0x42; maximum];
        assert_eq!(
            decompress_bounded(
                &raw_zstd_window_frame(10, &payload),
                GatewayComplianceContentEncoding::Zstd,
                maximum,
            )
            .expect("minimum window with exact sub-1-KiB output"),
            payload
        );
        assert!(matches!(
            decompress_bounded(
                &raw_zstd_window_frame(10, &vec![0x42; maximum + 1]),
                GatewayComplianceContentEncoding::Zstd,
                maximum,
            ),
            Err(GatewayComplianceError::ResourceLimit {
                resource: "decoded feed bytes",
                found,
                maximum: observed_maximum,
            }) if found == maximum + 1 && observed_maximum == maximum
        ));
    }
}

#[test]
fn zstd_window_limit_applies_to_later_concatenated_frames() {
    let first = raw_zstd_window_frame(10, b"first");
    let mut allowed = first.clone();
    allowed.extend_from_slice(&raw_zstd_window_frame(10, b"next"));
    assert_eq!(
        decompress_bounded(&allowed, GatewayComplianceContentEncoding::Zstd, 1024)
            .expect("two allowed frames"),
        b"firstnext"
    );
    let mut oversized_later = first;
    oversized_later.extend_from_slice(&raw_zstd_window_frame(11, b"next"));
    assert!(matches!(
        decompress_bounded(
            &oversized_later,
            GatewayComplianceContentEncoding::Zstd,
            1024,
        ),
        Err(GatewayComplianceError::Decompression(_))
    ));
}
