//! Integration tests for the Norito Streaming Codec baseline pipeline.
use std::convert::TryFrom;

use norito::streaming::{
    CapabilityFlags, EncryptionSuite, FecScheme, ManifestV1, Multiaddr, PrivacyCapabilities,
    PrivacyRelay, PrivacyRoute, StreamMetadata,
    chunk::{
        BaselineDecoder, BlockCountMismatchInfo, CodecError, chunk_commitments,
        data_availability_root, merkle_proof, merkle_root, storage_commitment, verify_merkle_proof,
    },
    codec::{
        BaselineEncoder, BaselineEncoderConfig, BaselineManifestParams, EncodedSegment,
        FrameDimensions, ManifestError, RawFrame, SegmentError, verify_segment,
    },
};

const SEGMENT_NUMBER: u64 = 41;
const TIMELINE_START_NS: u64 = 1_333_000;
const CONTENT_KEY_ID: u64 = 1_024;
const FRAME_DURATION_NS: u32 = 41_666_667;
const EXPECTED_COMMITMENTS: [[u8; 32]; 3] = [
    [
        142, 28, 112, 76, 39, 75, 137, 66, 101, 250, 158, 239, 241, 78, 192, 181, 93, 49, 41, 183,
        172, 163, 147, 86, 120, 234, 44, 122, 209, 52, 236, 71,
    ],
    [
        1, 153, 89, 109, 181, 166, 60, 243, 234, 142, 136, 238, 172, 84, 37, 2, 20, 122, 30, 46,
        197, 176, 69, 57, 209, 12, 96, 203, 85, 100, 41, 254,
    ],
    [
        122, 210, 17, 248, 186, 96, 224, 9, 59, 18, 17, 14, 236, 200, 174, 250, 84, 54, 71, 66, 80,
        73, 129, 150, 37, 217, 153, 104, 217, 53, 41, 155,
    ],
];
const EXPECTED_CHUNK_ROOT: [u8; 32] = [
    133, 163, 32, 139, 138, 238, 67, 15, 70, 79, 52, 138, 235, 168, 118, 194, 33, 199, 42, 92, 251,
    174, 6, 136, 172, 115, 254, 108, 146, 64, 101, 231,
];
const EXPECTED_NONCE_SALT: [u8; 32] = [
    68, 80, 127, 241, 238, 191, 98, 47, 95, 26, 37, 64, 56, 193, 121, 209, 157, 152, 169, 123, 160,
    111, 179, 74, 147, 199, 37, 3, 229, 99, 29, 141,
];
const EXPECTED_STORAGE_COMMITMENT: [u8; 32] = [
    197, 251, 15, 187, 158, 132, 35, 22, 15, 2, 37, 99, 139, 248, 165, 247, 73, 131, 194, 254, 92,
    96, 167, 188, 104, 226, 148, 203, 251, 63, 44, 176,
];
const EXPECTED_DA_ROOT: [u8; 32] = [
    250, 83, 206, 148, 228, 66, 143, 6, 205, 251, 238, 224, 213, 12, 12, 79, 197, 61, 49, 130, 228,
    91, 131, 42, 62, 158, 245, 200, 200, 159, 203, 146,
];

#[test]
fn baseline_segment_manifest_roundtrip() {
    let dims = FrameDimensions::new(8, 8);
    let encoder_config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frame_duration_ns: FRAME_DURATION_NS,
        frames_per_segment: 3,
        layer_bitmap: 0b101,
        encryption_suite: EncryptionSuite::X25519ChaCha20Poly1305([0xAB; 32]),
        duration_ns: 0,
        quantizer: 0,
        ..BaselineEncoderConfig::default()
    };

    let mut encoder = BaselineEncoder::new(encoder_config.clone());
    let frames = [
        pattern_frame(dims, 0x11),
        pattern_frame(dims, 0x37),
        pattern_frame(dims, 0x59),
    ];
    let raw_frames: Vec<_> = frames
        .iter()
        .map(|bytes| RawFrame::new(dims, bytes.clone()).expect("frame"))
        .collect();

    let segment = encoder
        .encode_segment(
            SEGMENT_NUMBER,
            TIMELINE_START_NS,
            CONTENT_KEY_ID,
            &raw_frames,
            None,
        )
        .expect("encode segment");
    assert_eq!(segment.header.segment_number, SEGMENT_NUMBER);
    assert_eq!(segment.header.content_key_id, CONTENT_KEY_ID);
    assert_eq!(segment.header.layer_bitmap, 0b101);
    assert_eq!(segment.header.timeline_start_ns, TIMELINE_START_NS);
    assert_eq!(
        segment.header.duration_ns,
        FRAME_DURATION_NS.saturating_mul(raw_frames.len() as u32)
    );

    verify_segment(
        &segment.header,
        &segment.descriptors,
        &segment.chunks,
        segment.audio.as_ref(),
    )
    .expect("segment verification");

    let payload_refs: Vec<(u16, &[u8])> = segment
        .descriptors
        .iter()
        .zip(segment.chunks.iter())
        .map(|(descriptor, chunk)| (descriptor.chunk_id, chunk.as_slice()))
        .collect();

    let commitments = chunk_commitments(SEGMENT_NUMBER, &payload_refs);
    assert_eq!(commitments, EXPECTED_COMMITMENTS);

    for (descriptor, expected) in segment.descriptors.iter().zip(commitments.iter()) {
        assert_eq!(&descriptor.commitment, expected);
    }

    let root = merkle_root(&commitments).expect("merkle root");
    assert_eq!(root, EXPECTED_CHUNK_ROOT);
    assert_eq!(segment.header.chunk_merkle_root, EXPECTED_CHUNK_ROOT);

    let chunk_ids: Vec<u16> = segment
        .descriptors
        .iter()
        .map(|descriptor| descriptor.chunk_id)
        .collect();
    let storage = storage_commitment(
        SEGMENT_NUMBER,
        CONTENT_KEY_ID,
        &segment.header.chunk_merkle_root,
        &chunk_ids,
    )
    .expect("storage commitment");
    assert_eq!(storage, EXPECTED_STORAGE_COMMITMENT);

    let da_root = data_availability_root(
        SEGMENT_NUMBER,
        CONTENT_KEY_ID,
        &segment.header.chunk_merkle_root,
        &chunk_ids,
    )
    .expect("da root");
    assert_eq!(da_root, EXPECTED_DA_ROOT);

    assert_eq!(segment.header.nonce_salt, EXPECTED_NONCE_SALT);

    let proof = merkle_proof(&commitments, 1, segment.descriptors[1].chunk_id).expect("proof");
    assert!(verify_merkle_proof(
        &commitments[1],
        &proof,
        &segment.header.chunk_merkle_root
    ));

    let manifest_params = BaselineManifestParams {
        stream_id: [0x21; 32],
        protocol_version: 1,
        published_at: 1_701_234_567,
        da_endpoint: Multiaddr::from("/dns/publisher.nsc/quic"),
        privacy_routes: vec![sample_privacy_route()],
        public_metadata: StreamMetadata {
            title: "Baseline Integration".into(),
            description: Some("integration test manifest".into()),
            access_policy_id: Some([0x44; 32]),
            tags: vec!["nsc".into(), "roundtrip".into()],
        },
        capabilities: CapabilityFlags::from_bits(0b0101),
        signature: [0xCD; 64],
        fec_suite: FecScheme::Rs12_10,
        neural_bundle: None,
        transport_capabilities_hash: [0x33; 32],
    };
    let manifest = segment.build_manifest(manifest_params.clone());
    segment
        .verify_manifest(&manifest)
        .expect("manifest verification");
    assert_eq!(
        manifest.transport_capabilities_hash,
        manifest_params.transport_capabilities_hash
    );

    assert_roundtrip_metadata(&segment, &manifest);

    let decoder = BaselineDecoder::new(dims, FRAME_DURATION_NS);
    let decoded = decoder.decode_segment(&segment).expect("decode");
    assert_eq!(decoded.len(), raw_frames.len());
    for (index, frame) in decoded.iter().enumerate() {
        assert_eq!(frame.index as usize, index);
        assert_eq!(
            frame.pts_ns,
            TIMELINE_START_NS + (FRAME_DURATION_NS as u64 * index as u64)
        );
        for (decoded_px, expected_px) in frame.luma.iter().zip(frames[index].iter()) {
            let decoded_val = *decoded_px;
            let expected_val = *expected_px;
            let diff = (decoded_val as i16 - expected_val as i16).unsigned_abs();
            assert!(
                diff <= 1,
                "pixel mismatch at frame {index}: decoded {decoded_val} expected {expected_val} (diff {diff})"
            );
        }
    }
}

fn pattern_frame(dimensions: FrameDimensions, seed: u8) -> Vec<u8> {
    let count = dimensions.pixel_count();
    (0..count)
        .map(|idx| seed.wrapping_add((idx as u8).wrapping_mul(7)))
        .collect()
}

fn sample_privacy_route() -> PrivacyRoute {
    PrivacyRoute {
        route_id: [0x31; 32],
        entry: PrivacyRelay {
            relay_id: [0x51; 32],
            endpoint: Multiaddr::from("/dns/entry.nsc/quic"),
            key_fingerprint: [0x61; 32],
            capabilities: PrivacyCapabilities::from_bits(0b001),
        },
        exit: PrivacyRelay {
            relay_id: [0x71; 32],
            endpoint: Multiaddr::from("/dns/exit.nsc/quic"),
            key_fingerprint: [0x81; 32],
            capabilities: PrivacyCapabilities::from_bits(0b010),
        },
        ticket_entry: vec![0xA0, 0xA1],
        ticket_exit: vec![0xB0, 0xB1],
        expiry_segment: SEGMENT_NUMBER + 16,
        soranet: None,
    }
}

fn assert_roundtrip_metadata(segment: &EncodedSegment, manifest: &ManifestV1) {
    assert_eq!(segment.header.segment_number, manifest.segment_number);
    assert_eq!(segment.header.profile, manifest.profile);
    assert_eq!(segment.header.encryption_suite, manifest.encryption_suite);
    assert_eq!(segment.header.chunk_merkle_root, manifest.chunk_root);
    assert_eq!(segment.header.content_key_id, manifest.content_key_id);
    assert_eq!(segment.header.nonce_salt, manifest.nonce_salt);
    assert_eq!(segment.descriptors.len(), manifest.chunk_descriptors.len());
}

#[test]
fn baseline_encoder_handles_unaligned_dimensions() {
    let dims = FrameDimensions::new(10, 8);
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: 1,
        ..BaselineEncoderConfig::default()
    };
    let frame_duration = config.frame_duration_ns;
    let mut encoder = BaselineEncoder::new(config);
    let raw = RawFrame::new(dims, vec![0x33; dims.pixel_count()]).expect("frame");
    let segment = encoder
        .encode_segment(7, TIMELINE_START_NS, CONTENT_KEY_ID, &[raw], None)
        .expect("unaligned dimensions must encode");

    assert_eq!(segment.header.chunk_count, 1);
    let decoder = BaselineDecoder::new(dims, frame_duration);
    let frames = decoder
        .decode_segment(&segment)
        .expect("decode unaligned frame");
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].luma.len(), dims.pixel_count());
    let mut max_diff = 0u16;
    for &decoded_px in &frames[0].luma {
        let diff = (decoded_px as i16 - 0x33).unsigned_abs();
        max_diff = max_diff.max(diff);
    }
    assert!(
        max_diff <= 16,
        "unaligned frame exceeded tolerance with diff {max_diff}"
    );
}

#[test]
fn baseline_quantized_roundtrip_within_tolerance() {
    let dims = FrameDimensions::new(16, 16);
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frame_duration_ns: FRAME_DURATION_NS / 2,
        frames_per_segment: 2,
        quantizer: 6,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let frames = [pattern_frame(dims, 0x22), pattern_frame(dims, 0x9F)];
    let raw_frames: Vec<_> = frames
        .iter()
        .map(|bytes| RawFrame::new(dims, bytes.clone()).expect("frame"))
        .collect();

    let segment = encoder
        .encode_segment(
            SEGMENT_NUMBER + 1,
            TIMELINE_START_NS + 99,
            CONTENT_KEY_ID + 1,
            &raw_frames,
            None,
        )
        .expect("encode segment");

    verify_segment(
        &segment.header,
        &segment.descriptors,
        &segment.chunks,
        segment.audio.as_ref(),
    )
    .expect("segment verification");

    let decoder = BaselineDecoder::new(dims, config.frame_duration_ns);
    let decoded = decoder.decode_segment(&segment).expect("decode");
    assert_eq!(decoded.len(), raw_frames.len());

    let mut max_diff = 0u16;
    for (frame_idx, decoded_frame) in decoded.iter().enumerate() {
        for (decoded_px, expected_px) in decoded_frame.luma.iter().zip(frames[frame_idx].iter()) {
            let diff = (*decoded_px as i16 - *expected_px as i16).unsigned_abs();
            max_diff = max_diff.max(diff);
        }
    }
    assert!(
        max_diff <= 200,
        "maximum pixel delta {max_diff} exceeded tolerance"
    );
}

#[test]
fn verify_segment_detects_unsorted_chunk_ids() {
    let dims = FrameDimensions::new(8, 8);
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: 2,
        ..BaselineEncoderConfig::default()
    });
    let frames = [pattern_frame(dims, 0x10), pattern_frame(dims, 0x11)];
    let raw_frames: Vec<_> = frames
        .iter()
        .map(|bytes| RawFrame::new(dims, bytes.clone()).expect("frame"))
        .collect();
    let segment = encoder
        .encode_segment(
            SEGMENT_NUMBER + 2,
            TIMELINE_START_NS,
            CONTENT_KEY_ID,
            &raw_frames,
            None,
        )
        .expect("encode segment");

    let mut tampered_descriptors = segment.descriptors.clone();
    tampered_descriptors[0].chunk_id = 1;
    tampered_descriptors[1].chunk_id = 0;
    let err = verify_segment(
        &segment.header,
        &tampered_descriptors,
        &segment.chunks,
        segment.audio.as_ref(),
    )
    .expect_err("unsorted chunk ids must fail");
    assert!(matches!(err, SegmentError::UnsortedChunkIds));
}

#[test]
fn manifest_verification_rejects_nonce_salt_change() {
    let dims = FrameDimensions::new(8, 8);
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: 1,
        ..BaselineEncoderConfig::default()
    });
    let frame = RawFrame::new(dims, pattern_frame(dims, 0x7D)).expect("frame");
    let segment = encoder
        .encode_segment(
            SEGMENT_NUMBER + 3,
            TIMELINE_START_NS,
            CONTENT_KEY_ID,
            &[frame],
            None,
        )
        .expect("encode segment");

    let mut manifest = segment.build_manifest(BaselineManifestParams {
        stream_id: [0x99; 32],
        da_endpoint: Multiaddr::from("/dns/nonce-mismatch/quic"),
        ..BaselineManifestParams::default()
    });
    manifest.nonce_salt[0] ^= 0xFF;
    let err = segment
        .verify_manifest(&manifest)
        .expect_err("nonce mismatch must fail");
    assert!(matches!(err, ManifestError::NonceSaltMismatch));
}

fn build_segment_for_decoder_tests(
    dims: FrameDimensions,
    frame_count: usize,
) -> (BaselineEncoderConfig, EncodedSegment) {
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: frame_count as u16,
        frame_duration_ns: FRAME_DURATION_NS,
        quantizer: 0,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let frames: Vec<_> = (0..frame_count)
        .map(|idx| {
            let seed = 0x30u8.wrapping_add((idx as u8).wrapping_mul(0x11));
            RawFrame::new(dims, pattern_frame(dims, seed)).expect("frame")
        })
        .collect();
    let segment_number = SEGMENT_NUMBER + frame_count as u64 + 10;
    let timeline = TIMELINE_START_NS + 250;
    let content_key = CONTENT_KEY_ID + 5;
    let segment = encoder
        .encode_segment(segment_number, timeline, content_key, &frames, None)
        .expect("encode segment");
    (config, segment)
}

fn refresh_segment_commitments(segment: &mut EncodedSegment) {
    segment.header.chunk_count = segment.descriptors.len() as u16;
    let mut payload_refs = Vec::with_capacity(segment.chunks.len());
    let mut offset = 0u32;
    for (descriptor, chunk) in segment.descriptors.iter_mut().zip(segment.chunks.iter()) {
        descriptor.offset = offset;
        let length = u32::try_from(chunk.len()).expect("chunk length fits u32");
        descriptor.length = length;
        offset = offset.saturating_add(length);
        payload_refs.push((descriptor.chunk_id, chunk.as_slice()));
    }
    let commitments = chunk_commitments(segment.header.segment_number, &payload_refs);
    for (descriptor, commitment) in segment.descriptors.iter_mut().zip(commitments.iter()) {
        descriptor.commitment = *commitment;
    }
    segment.header.chunk_merkle_root = merkle_root(&commitments).expect("chunk root");
}

#[test]
fn baseline_decoder_rejects_frame_index_mismatch() {
    let dims = FrameDimensions::new(8, 8);
    let (config, mut segment) = build_segment_for_decoder_tests(dims, 2);
    let tampered_index = 9u32;
    segment.chunks[0][0..4].copy_from_slice(&tampered_index.to_le_bytes());
    refresh_segment_commitments(&mut segment);
    verify_segment(
        &segment.header,
        &segment.descriptors,
        &segment.chunks,
        segment.audio.as_ref(),
    )
    .expect("segment verification");

    let decoder = BaselineDecoder::new(dims, config.frame_duration_ns);
    let err = decoder
        .decode_segment(&segment)
        .expect_err("frame index mismatch must fail");
    match err {
        CodecError::FrameIndexMismatch(info) => {
            assert_eq!(info.expected, 0);
            assert_eq!(info.found, tampered_index);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn baseline_decoder_rejects_frame_pts_mismatch() {
    let dims = FrameDimensions::new(8, 8);
    let (config, mut segment) = build_segment_for_decoder_tests(dims, 2);
    let mut first_pts_bytes = [0u8; 8];
    first_pts_bytes.copy_from_slice(&segment.chunks[0][4..12]);
    let first_frame_pts = u64::from_le_bytes(first_pts_bytes);
    segment.header.timeline_start_ns = segment.header.timeline_start_ns.saturating_add(1);
    verify_segment(
        &segment.header,
        &segment.descriptors,
        &segment.chunks,
        segment.audio.as_ref(),
    )
    .expect("segment verification");

    let decoder = BaselineDecoder::new(dims, config.frame_duration_ns);
    let err = decoder
        .decode_segment(&segment)
        .expect_err("frame pts mismatch must fail");
    match err {
        CodecError::FramePtsMismatch(info) => {
            assert_eq!(info.index, 0);
            assert_eq!(info.expected, segment.header.timeline_start_ns);
            assert_eq!(info.found, first_frame_pts);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn baseline_decoder_rejects_block_count_mismatch() {
    let dims = FrameDimensions::new(8, 8);
    let (config, mut segment) = build_segment_for_decoder_tests(dims, 2);
    segment.chunks[1][14..16].copy_from_slice(&0u16.to_le_bytes());
    refresh_segment_commitments(&mut segment);
    verify_segment(
        &segment.header,
        &segment.descriptors,
        &segment.chunks,
        segment.audio.as_ref(),
    )
    .expect("segment verification");

    let decoder = BaselineDecoder::new(dims, config.frame_duration_ns);
    let err = decoder
        .decode_segment(&segment)
        .expect_err("block count mismatch must fail");
    match err {
        CodecError::BlockCountMismatch(BlockCountMismatchInfo { expected, found }) => {
            let expected_blocks = dims.align_to_block().block_count() as u32;
            assert_eq!(expected, expected_blocks);
            assert_eq!(found, 0);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn baseline_decoder_rejects_truncated_chunk() {
    let dims = FrameDimensions::new(8, 8);
    let (config, mut segment) = build_segment_for_decoder_tests(dims, 1);
    segment.chunks[0].truncate(8);
    refresh_segment_commitments(&mut segment);
    verify_segment(
        &segment.header,
        &segment.descriptors,
        &segment.chunks,
        segment.audio.as_ref(),
    )
    .expect("segment verification");

    let decoder = BaselineDecoder::new(dims, config.frame_duration_ns);
    let err = decoder
        .decode_segment(&segment)
        .expect_err("truncated chunk must fail");
    assert!(matches!(err, CodecError::ChunkTooShort));
}
