//! Execution-only conformance and adversarial tests for the shared native digest owner.

use super::*;

const CONTEXT: TransparentStarkDigestContextV1 =
    TransparentStarkDigestContextV1::execution_v1(b"race-stock-proof-v1");
const ROLE: &[u8] = b"execution-race-base-leaf-v1";
const PHASE: &[u8] = b"vector-row";

#[test]
fn execution_digest_matches_independent_six_lane_vector() {
    // Reproduced with scripts/fastpq/reference_digest384.py, using hashlib SHAKE256
    // and Python integer arithmetic. All 48 bytes bind the exact execution domain.
    let fields: &[&[u8]] = &[
        &[0, 255, 1],
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
    ];
    let actual = goldilocks_digest384_frame_v1(CONTEXT, ROLE, PHASE, 2, 17, 3, fields).unwrap();
    assert_eq!(
        actual.to_le_bytes(),
        [
            0xb4, 0x9b, 0x21, 0x42, 0x1b, 0xd0, 0x7f, 0xf6, 0x09, 0x1e, 0x8c, 0x01, 0x04, 0xea,
            0x61, 0x11, 0x38, 0xc2, 0x76, 0x1c, 0x38, 0x3a, 0x01, 0xf9, 0xe4, 0xc5, 0x23, 0x7e,
            0xd0, 0x22, 0xd3, 0x32, 0x58, 0x25, 0xd9, 0x05, 0xfc, 0x77, 0x83, 0x21, 0xf5, 0x88,
            0x55, 0x80, 0x59, 0x6e, 0xe4, 0xbc
        ]
    );
}

#[test]
fn execution_digest_separates_coordinates_fields_and_profiles() {
    let fields: &[&[u8]] = &[b"left", b"right"];
    let canonical = goldilocks_digest384_frame_v1(CONTEXT, ROLE, PHASE, 2, 17, 3, fields).unwrap();
    for (context, role, phase, level, index, counter) in [
        (
            TransparentStarkDigestContextV1::execution_v1(b"classed-touring-s1-proof-v1"),
            ROLE,
            PHASE,
            2,
            17,
            3,
        ),
        (
            CONTEXT,
            b"execution-race-aux-leaf-v1".as_slice(),
            PHASE,
            2,
            17,
            3,
        ),
        (CONTEXT, ROLE, b"binary-merkle-node".as_slice(), 2, 17, 3),
        (CONTEXT, ROLE, PHASE, 3, 17, 3),
        (CONTEXT, ROLE, PHASE, 2, 18, 3),
        (CONTEXT, ROLE, PHASE, 2, 17, 4),
    ] {
        assert_ne!(
            canonical,
            goldilocks_digest384_frame_v1(context, role, phase, level, index, counter, fields)
                .unwrap()
        );
    }
    for altered in [
        vec![b"leftright".as_slice()],
        vec![b"right".as_slice(), b"left".as_slice()],
        vec![b"left".as_slice(), b"right".as_slice(), b"".as_slice()],
    ] {
        assert_ne!(
            canonical,
            goldilocks_digest384_frame_v1(CONTEXT, ROLE, PHASE, 2, 17, 3, &altered).unwrap()
        );
    }
}

#[test]
fn execution_digest_stream_matches_all_chunk_boundaries_and_rejects_wrong_lengths() {
    let payload = (0_u8..=64).collect::<Vec<_>>();
    for len in [0, 1, 6, 7, 8, 13, 14, 15, 64, 65] {
        let fields: &[&[u8]] = &[b"prefix", &payload[..len]];
        let expected =
            goldilocks_digest384_frame_v1(CONTEXT, ROLE, PHASE, 2, 17, 3, fields).unwrap();
        for split in 0..=len {
            let mut stream = goldilocks_digest384_last_field_stream_v1(
                CONTEXT,
                ROLE,
                PHASE,
                2,
                17,
                3,
                &[b"prefix"],
                len,
            )
            .unwrap();
            stream.update(&payload[..split]).unwrap();
            stream.update(&payload[split..len]).unwrap();
            assert_eq!(stream.finalize().unwrap(), expected);
        }
    }
    let mut overrun =
        goldilocks_digest384_last_field_stream_v1(CONTEXT, ROLE, PHASE, 2, 17, 3, &[], 1).unwrap();
    let error = overrun.update(b"ab").unwrap_err();
    assert!(matches!(
        error,
        GoldilocksDigest384LastFieldStreamErrorV1::InputOverrun { .. }
    ));
    assert_eq!(
        map_digest_stream_error_v1(error),
        TransparentStarkErrorV1::FrameLengthOverflow
    );
    let underrun =
        goldilocks_digest384_last_field_stream_v1(CONTEXT, ROLE, PHASE, 2, 17, 3, &[], 1).unwrap();
    let error = underrun.finalize().unwrap_err();
    assert!(matches!(
        error,
        GoldilocksDigest384LastFieldStreamErrorV1::InputUnderrun { .. }
    ));
    assert_eq!(
        map_digest_stream_error_v1(error),
        TransparentStarkErrorV1::FrameLengthOverflow
    );
    assert_eq!(
        map_digest_stream_error_v1(GoldilocksDigest384LastFieldStreamErrorV1::FramingLimitExceeded),
        TransparentStarkErrorV1::FrameLengthOverflow
    );
}

#[test]
fn execution_digest_rejects_empty_and_oversized_domains_before_hashing() {
    const OVERSIZED: &[u8] = &[b'x'; u16::MAX as usize + 1];
    for context in [
        CONTEXT,
        TransparentStarkDigestContextV1::execution_v1(b"classed-touring-s1-proof-v1"),
    ] {
        assert_eq!(context.protocol_label_v1(), b"native-execution-v1");
        assert_eq!(
            context.maximum_proof_bytes_v1(),
            iroha_data_model::execution_proofs::EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1
        );
    }
    for (context, role, phase) in [
        (
            TransparentStarkDigestContextV1::execution_v1(b""),
            ROLE,
            PHASE,
        ),
        (CONTEXT, b"".as_slice(), PHASE),
        (CONTEXT, ROLE, b"".as_slice()),
        (
            TransparentStarkDigestContextV1::execution_v1(OVERSIZED),
            ROLE,
            PHASE,
        ),
        (CONTEXT, OVERSIZED, PHASE),
        (CONTEXT, ROLE, OVERSIZED),
    ] {
        assert_eq!(
            goldilocks_digest384_frame_v1(context, role, phase, 0, 0, 0, &[]),
            Err(TransparentStarkErrorV1::InvalidDigestDomain)
        );
        assert!(matches!(
            goldilocks_digest384_last_field_stream_v1(context, role, phase, 0, 0, 0, &[], 0),
            Err(TransparentStarkErrorV1::InvalidDigestDomain)
        ));
    }
}
