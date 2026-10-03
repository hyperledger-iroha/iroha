//! Finite canonical carrier decoder tests. These do not qualify any cryptographic proof.
use super::*;
fn context() -> KagemushaOrdinaryCashClockContextV1 {
    KagemushaOrdinaryCashClockContextV1 {
        version: 1,
        request_nonce: [11; 32],
        signed_observations_original_digest: [12; 32],
        lower_at_ms: 13,
        upper_at_ms: 14,
    }
}
#[test]
fn commit_data_decoder_requires_full_canonical_frame_and_exact_type() {
    let value = context();
    let raw = norito::encode_canonical(&value).unwrap();
    let decoded: KagemushaOrdinaryCashClockContextV1 = decode_data(&raw, raw.len()).unwrap();
    assert_eq!(decoded, value);
    assert!(decode_data::<KagemushaOrdinaryCashClockContextV1>(&raw, raw.len() - 1).is_err());
    assert!(decode_data::<KagemushaOrdinaryCashClockContextV1>(&[], raw.len()).is_err());
    let mut trailing = raw.clone();
    trailing.push(0);
    assert!(decode_data::<KagemushaOrdinaryCashClockContextV1>(&trailing, trailing.len()).is_err());
    assert!(KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(&raw).is_err());
    assert!(KagemushaOrdinaryLineageCommitProofBundleV1::decode_original(&raw).is_err());
}
#[test]
fn compact_and_service_clock_caps_preserve_existing_full_original_maxima() {
    assert_eq!(
        KAGEMUSHA_ORDINARY_CASH_OUTGOING_ORIGINAL_MAX_BYTES_V1,
        KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1 + 256 * 1024
    );
    assert_eq!(
        COMMIT_BUNDLE_MAX,
        3 * KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1 + 3 * 1024 * 1024
    );
    for raw in [&[][..], &[0][..], &[1, 2, 3][..]] {
        assert!(KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(raw).is_err());
        assert!(KagemushaOrdinaryLineageCommitProofBundleV1::decode_original(raw).is_err());
    }
}
