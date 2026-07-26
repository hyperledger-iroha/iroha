//! Tests covering transport capability resolution and hashing.
use blake3::Hasher;
use norito::streaming::{
    HpkeSuite, HpkeSuiteMask, PrivacyBucketGranularity, TransportCapabilities,
    TransportCapabilityError, resolve_transport_capabilities,
};

fn caps(
    suites: HpkeSuiteMask,
    datagram: bool,
    datagram_size: u16,
    feedback_ms: u16,
) -> TransportCapabilities {
    TransportCapabilities {
        hpke_suites: suites,
        supports_datagram: datagram,
        max_segment_datagram_size: datagram_size,
        fec_feedback_interval_ms: feedback_ms,
        privacy_bucket_granularity: PrivacyBucketGranularity::StandardV1,
    }
}

#[test]
fn resolves_shared_suite_preferring_lowest_index() {
    let local = caps(
        HpkeSuiteMask::from_bits(HpkeSuiteMask::KYBER768.bits() | HpkeSuiteMask::KYBER1024.bits()),
        true,
        1500,
        200,
    );
    let remote = caps(HpkeSuiteMask::KYBER1024, true, 1200, 250);

    let resolved = resolve_transport_capabilities(&local, &remote).expect("resolution");
    assert_eq!(resolved.hpke_suite, HpkeSuite::Kyber1024AuthPsk);
    assert!(resolved.use_datagram);
    assert_eq!(resolved.max_segment_datagram_size, 1200);
    assert_eq!(resolved.fec_feedback_interval_ms, 250);
    assert_eq!(
        resolved.privacy_bucket_granularity,
        PrivacyBucketGranularity::StandardV1
    );

    // Hash must be stable and match manual calculation.
    let expected_hash = {
        let mut hasher = Hasher::new();
        hasher.update(b"nsc-transport-capabilities");
        hasher.update(&HpkeSuite::Kyber1024AuthPsk.suite_id().to_le_bytes());
        hasher.update(&[1]); // use_datagram = true
        hasher.update(&1200u16.to_le_bytes());
        hasher.update(&250u16.to_le_bytes());
        hasher.update(&[PrivacyBucketGranularity::StandardV1 as u8]);
        hasher.finalize()
    };
    assert_eq!(
        resolved.capabilities_hash(),
        <blake3::Hash as Into<[u8; 32]>>::into(expected_hash),
        "capability hash must follow spec derivation"
    );
}

#[test]
fn resolves_without_datagram_when_peer_disables() {
    let local = caps(HpkeSuiteMask::KYBER768, true, 1500, 180);
    let remote = caps(HpkeSuiteMask::KYBER768, false, 0, 300);

    let resolved = resolve_transport_capabilities(&local, &remote).expect("resolution");
    assert!(!resolved.use_datagram);
    assert_eq!(resolved.max_segment_datagram_size, 0);
    assert_eq!(resolved.fec_feedback_interval_ms, 300);
}

#[test]
fn errors_when_no_shared_suite() {
    let local = caps(HpkeSuiteMask::KYBER768, true, 1500, 200);
    let remote = caps(HpkeSuiteMask::EMPTY, true, 1500, 200);

    let err = resolve_transport_capabilities(&local, &remote).expect_err("missing suite");
    assert!(matches!(err, TransportCapabilityError::NoSharedHpkeSuite));
}

#[test]
fn errors_on_zero_datagram_size() {
    let local = caps(HpkeSuiteMask::KYBER768, true, 1500, 200);
    let remote = caps(HpkeSuiteMask::KYBER768, true, 0, 200);

    let err = resolve_transport_capabilities(&local, &remote).expect_err("zero size");
    assert!(matches!(
        err,
        TransportCapabilityError::InvalidDatagramSize(0)
    ));
}
