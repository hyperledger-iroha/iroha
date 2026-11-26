//! Regression tests for the baseline streaming codec and bundled entropy.
use super::*;
use crate::streaming::{
    Hash,
    chunk::{
        BaselineDecoder, read_chroma_len, read_frame_header_u16_le, read_frame_header_u32_le,
        read_frame_header_u64_le,
    },
};
use std::{str::FromStr, sync::Arc};
fn hash_seed(seed: u8) -> Hash {
    let mut bytes = [0u8; 32];
    bytes.fill(seed);
    bytes
}
const JPEG_SAMPLE_BLOCK: [i16; BLOCK_PIXELS] = [
    52, 55, 61, 66, 70, 61, 64, 73, //
    63, 59, 55, 90, 109, 85, 69, 72, //
    62, 59, 68, 113, 144, 104, 66, 73, //
    63, 58, 71, 122, 154, 106, 70, 69, //
    67, 61, 68, 104, 126, 88, 68, 70, //
    79, 65, 60, 70, 77, 68, 58, 75, //
    85, 71, 64, 59, 55, 61, 65, 83, //
    87, 79, 69, 68, 65, 76, 78, 94,
];
#[cfg(not(feature = "streaming-fixed-point-dct"))]
const JPEG_SAMPLE_DCT_Q4: [i32; BLOCK_PIXELS] = [
    609, -30, -61, 27, 56, -20, -2, 0, 4, -22, -61, 10, 13, -7, -9, 5, -47, 7, 77, -25, -29, 10, 5,
    -6, -49, 12, 34, -15, -10, 6, 2, 2, 12, -7, -13, -4, -2, 2, -3, 3, -8, 3, 2, -6, -2, 1, 4, 2,
    -1, 0, 0, -2, -1, -3, 4, -1, 0, 0, -1, -4, -1, 0, 1, 2,
];
#[cfg(feature = "streaming-fixed-point-dct")]
const JPEG_SAMPLE_DCT_Q4: [i32; BLOCK_PIXELS] = [
    609, -31, -62, 27, 56, -21, -3, 0, 4, -23, -62, 10, 13, -8, -10, 5, -48, 7, 77, -26, -30, 10,
    5, -7, -50, 12, 34, -16, -11, 6, 2, 2, 12, -8, -14, -5, -3, 2, -4, 3, -9, 3, 2, -7, -3, 1, 4,
    2, -2, 0, 0, -3, -2, -4, 4, -2, -1, 0, -2, -5, -2, -1, 1, 2,
];
const JPEG_SAMPLE_QUANT_Q4: [i16; BLOCK_PIXELS] = [
    10, -1, -2, 0, 1, 0, 0, 0, //
    0, 0, -1, 0, 0, 0, 0, 0, //
    -1, 0, 1, 0, 0, 0, 0, 0, //
    -1, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0,
];
const JPEG_SAMPLE_DEQUANT_Q4: [i32; BLOCK_PIXELS] = [
    640, -44, -80, 0, 96, 0, 0, 0, //
    0, 0, -56, 0, 0, 0, 0, 0, //
    -56, 0, 64, 0, 0, 0, 0, 0, //
    -56, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0,
];
const JPEG_SAMPLE_RECON_Q4: [i32; BLOCK_PIXELS] = [
    55, 39, 51, 85, 88, 60, 52, 70, //
    64, 52, 69, 107, 110, 78, 65, 80, //
    72, 64, 88, 130, 133, 97, 77, 87, //
    70, 64, 90, 134, 137, 99, 77, 85, //
    64, 55, 77, 118, 121, 86, 68, 79, //
    67, 51, 63, 96, 99, 71, 64, 82, //
    82, 57, 57, 81, 84, 65, 70, 97, //
    97, 66, 57, 76, 79, 66, 79, 112,
];
const GOLDEN_Q4_CHUNK: [u8; 30] = [
    0, 0, 0, 0, 232, 3, 0, 0, 0, 0, 0, 0, 0, 4, 1, 0, 14, 0, 0, 255, 255, 0, 247, 255, 6, 255, 255,
    255, 0, 0,
];
#[test]
fn encode_and_verify_roundtrip() {
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
    let dims = encoder.config.frame_dimensions;
    let frames = vec![
        RawFrame::new(dims, vec![1u8; dims.pixel_count()]).expect("frame"),
        RawFrame::new(dims, vec![2u8; dims.pixel_count()]).expect("frame"),
    ];
    let encoded = encoder
        .encode_segment(5, 1_000, 3, &frames, None)
        .expect("encode");
    assert_eq!(encoded.header.segment_number, 5);
    assert_eq!(encoded.header.chunk_count, 2);
    assert_eq!(encoded.descriptors.len(), 2);
    assert_eq!(encoded.descriptors[0].offset, 0);
    assert_eq!(
        encoded.descriptors[1].offset,
        encoded.chunks[0].len() as u32
    );
    verify_segment(
        &encoded.header,
        &encoded.descriptors,
        &encoded.chunks,
        encoded.audio.as_ref(),
    )
    .expect("verification");
}
#[test]
fn encode_segment_with_chroma_roundtrips_payload() {
    let dims = FrameDimensions::new(16, 16);
    let frame = RawFrame::new(dims, vec![0xAA; dims.pixel_count()]).expect("frame");
    let chroma_len = dims.pixel_count() / 4;
    let chroma_frame =
        Chroma420Frame::new(dims, vec![0x10; chroma_len], vec![0xF0; chroma_len]).expect("chroma");
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: 1,
        frame_duration_ns: 10_000,
        quantizer: 12,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let segment = encoder
        .encode_segment_with_chroma(
            5,
            0,
            3,
            &[frame],
            Some(std::slice::from_ref(&chroma_frame)),
            None,
        )
        .expect("encode with chroma");
    let chunk = segment.chunks.first().expect("chunk");
    assert!(
        chunk.len() > FRAME_HEADER_LEN,
        "chunk should include chroma payload bytes"
    );
    let decoder = BaselineDecoder::new(dims, config.frame_duration_ns);
    let decoded = decoder.decode_segment(&segment).expect("decode");
    assert_eq!(decoded.len(), 1, "single frame should decode");
    let decoded_chroma = decoded[0]
        .chroma
        .as_ref()
        .expect("chroma should be present in decoded frame");
    assert_eq!(decoded_chroma.u.len(), chroma_len);
    assert_eq!(decoded_chroma.v.len(), chroma_len);
    assert!(
        decoded_chroma.u.iter().any(|&value| value != 128),
        "chroma decode should not fall back to neutral fill"
    );
    assert!(
        decoded_chroma.v.iter().any(|&value| value != 128),
        "chroma decode should not fall back to neutral fill"
    );
}
#[test]
fn encode_segment_honors_explicit_duration() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(4, 4),
        duration_ns: 999_000,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let frame = RawFrame::new(config.frame_dimensions, vec![0xEE; 16]).unwrap();
    let encoded = encoder
        .encode_segment(6, 10_000, 2, &[frame], None)
        .expect("encode");
    assert_eq!(encoded.header.duration_ns, config.duration_ns);
}
#[test]
fn verify_detects_tampered_chunk() {
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
    let dims = encoder.config.frame_dimensions;
    let frames = vec![
        RawFrame::new(dims, vec![10u8; dims.pixel_count()]).expect("frame"),
        RawFrame::new(dims, vec![11u8; dims.pixel_count()]).expect("frame"),
    ];
    let mut encoded = encoder
        .encode_segment(7, 2_000, 4, &frames, None)
        .expect("encode");
    encoded.chunks[0][0] ^= 0xFF;
    let err = verify_segment(
        &encoded.header,
        &encoded.descriptors,
        &encoded.chunks,
        encoded.audio.as_ref(),
    )
    .expect_err("should fail");
    assert!(matches!(err, SegmentError::CommitmentMismatch(0)));
}
#[test]
fn verify_rejects_unsorted_chunk_ids() {
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
    let dims = encoder.config.frame_dimensions;
    let frames = vec![
        RawFrame::new(dims, vec![3u8; dims.pixel_count()]).expect("frame"),
        RawFrame::new(dims, vec![4u8; dims.pixel_count()]).expect("frame"),
    ];
    let mut encoded = encoder
        .encode_segment(9, 3_000, 7, &frames, None)
        .expect("encode");
    encoded.descriptors[0].chunk_id = encoded.descriptors[1].chunk_id;
    let err = verify_segment(
        &encoded.header,
        &encoded.descriptors,
        &encoded.chunks,
        encoded.audio.as_ref(),
    )
    .expect_err("unsorted ids must fail");
    assert!(matches!(err, SegmentError::UnsortedChunkIds));
}
#[test]
fn verify_rejects_header_count_mismatch() {
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
    let dims = encoder.config.frame_dimensions;
    let frames = vec![RawFrame::new(dims, vec![0xAB; dims.pixel_count()]).expect("frame")];
    let mut encoded = encoder
        .encode_segment(11, 5_000, 8, &frames, None)
        .expect("encode");
    encoded.header.chunk_count = 0;
    let err = verify_segment(
        &encoded.header,
        &encoded.descriptors,
        &encoded.chunks,
        encoded.audio.as_ref(),
    )
    .expect_err("header mismatch must fail");
    assert!(matches!(
        err,
        SegmentError::HeaderCountMismatch(details)
            if details.header == 0 && details.actual == 1
    ));
}
#[test]
fn verify_rejects_descriptor_offset_mismatch() {
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
    let dims = encoder.config.frame_dimensions;
    let frames = vec![
        RawFrame::new(dims, vec![0x01; dims.pixel_count()]).expect("frame"),
        RawFrame::new(dims, vec![0x02; dims.pixel_count()]).expect("frame"),
    ];
    let mut encoded = encoder
        .encode_segment(13, 7_000, 9, &frames, None)
        .expect("encode");
    encoded.descriptors[1].offset += 1;
    let err = verify_segment(
        &encoded.header,
        &encoded.descriptors,
        &encoded.chunks,
        encoded.audio.as_ref(),
    )
    .expect_err("offset mismatch must fail");
    assert!(matches!(
        err,
        SegmentError::DescriptorOffsetMismatch(details) if details.index == 1
    ));
}
#[test]
fn verify_rejects_descriptor_length_mismatch() {
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
    let dims = encoder.config.frame_dimensions;
    let frames = vec![
        RawFrame::new(dims, vec![0x05; dims.pixel_count()]).expect("frame"),
        RawFrame::new(dims, vec![0x06; dims.pixel_count()]).expect("frame"),
    ];
    let mut encoded = encoder
        .encode_segment(17, 8_000, 10, &frames, None)
        .expect("encode");
    encoded.descriptors[0].length -= 1;
    let err = verify_segment(
        &encoded.header,
        &encoded.descriptors,
        &encoded.chunks,
        encoded.audio.as_ref(),
    )
    .expect_err("length mismatch must fail");
    assert!(matches!(
        err,
        SegmentError::DescriptorLengthMismatch(details) if details.index == 0
    ));
}
#[test]
fn decode_rejects_pts_mismatch() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(4, 4),
        frame_duration_ns: 40_000_000,
        frames_per_segment: 2,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let frames = vec![
        RawFrame::new(config.frame_dimensions, vec![0x11; 16]).unwrap(),
        RawFrame::new(config.frame_dimensions, vec![0x22; 16]).unwrap(),
    ];
    let mut segment = encoder
        .encode_segment(19, 1_000, 6, &frames, None)
        .expect("encode");
    segment.header.timeline_start_ns += 1;
    let decoder = BaselineDecoder::new(config.frame_dimensions, config.frame_duration_ns);
    let err = decoder
        .decode_segment(&segment)
        .expect_err("timeline mismatch should be detected");
    assert!(matches!(
        err,
        CodecError::FramePtsMismatch(details) if details.index == 0
    ));
}
#[test]
fn encode_segment_detects_pts_overflow() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(2, 2),
        frame_duration_ns: u32::MAX,
        frames_per_segment: 2,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let frames: Vec<RawFrame> = (0..2)
        .map(|_| {
            RawFrame::new(
                config.frame_dimensions,
                vec![0x7Fu8; config.frame_dimensions.pixel_count()],
            )
            .unwrap()
        })
        .collect();
    let timeline_start_ns = u64::MAX - u64::from(config.frame_duration_ns) + 1;
    let err = encoder
        .encode_segment(23, timeline_start_ns, 4, &frames, None)
        .expect_err("overflow must be rejected");
    assert!(matches!(err, CodecError::FramePtsOverflow(1)));
}
#[test]
fn merkle_root_changes_on_chunk_mutation() {
    let dims = FrameDimensions::new(4, 4);
    let frames = vec![
        RawFrame::new(dims, vec![1u8; 16]).unwrap(),
        RawFrame::new(dims, vec![2u8; 16]).unwrap(),
    ];
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig {
        frame_dimensions: dims,
        ..BaselineEncoderConfig::default()
    });
    let encoded = encoder
        .encode_segment(1, 0, 1, &frames, None)
        .expect("encode");
    let payload_refs: Vec<(u16, &[u8])> = encoded
        .chunks
        .iter()
        .enumerate()
        .map(|(idx, chunk)| (idx as u16, chunk.as_slice()))
        .collect();
    let commitments_a = chunk_commitments(1, &payload_refs);
    let root_a = merkle_root(&commitments_a).expect("root");
    let mut chunks_b = encoded.chunks.clone();
    chunks_b[0][FRAME_HEADER_LEN] ^= 0x1;
    let payload_refs_b: Vec<(u16, &[u8])> = chunks_b
        .iter()
        .enumerate()
        .map(|(idx, chunk)| (idx as u16, chunk.as_slice()))
        .collect();
    let commitments_b = chunk_commitments(1, &payload_refs_b);
    let root_b = merkle_root(&commitments_b).expect("root");
    assert_ne!(root_a, root_b);
}
#[test]
fn manifest_build_and_verify() {
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
    let dims = encoder.config.frame_dimensions;
    let frames = vec![
        RawFrame::new(dims, vec![1u8; dims.pixel_count()]).unwrap(),
        RawFrame::new(dims, vec![2u8; dims.pixel_count()]).unwrap(),
    ];
    let encoded = encoder
        .encode_segment(11, 5_000, 9, &frames, None)
        .expect("encode");
    let params = BaselineManifestParams {
        stream_id: hash_seed(3),
        protocol_version: 1,
        published_at: 1_700_000_000,
        da_endpoint: "/dns/publisher.example/quic".into(),
        privacy_routes: Vec::new(),
        public_metadata: StreamMetadata {
            title: "Demo Stream".into(),
            description: Some("Baseline manifest build test".into()),
            access_policy_id: None,
            tags: vec!["demo".into()],
        },
        capabilities: CapabilityFlags::from_bits(0b101),
        signature: [0u8; 64],
        fec_suite: FecScheme::Rs12_10,
        neural_bundle: None,
        transport_capabilities_hash: hash_seed(9),
    };
    let manifest = encoded.build_manifest(params);
    assert_eq!(manifest.segment_number, encoded.header.segment_number);
    assert_eq!(manifest.chunk_descriptors.len(), encoded.descriptors.len());
    encoded
        .verify_manifest(&manifest)
        .expect("manifest verification");
}
#[test]
fn manifest_verify_detects_descriptor_mismatch() {
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
    let dims = encoder.config.frame_dimensions;
    let frame = RawFrame::new(dims, vec![0xAA; dims.pixel_count()]).unwrap();
    let encoded = encoder
        .encode_segment(21, 10_000, 5, &[frame], None)
        .expect("encode");
    let mut manifest = encoded.build_manifest(BaselineManifestParams {
        stream_id: hash_seed(1),
        da_endpoint: "/dns/test/quic".into(),
        ..BaselineManifestParams::default()
    });
    manifest.chunk_descriptors[0].length += 1;
    let err = encoded
        .verify_manifest(&manifest)
        .expect_err("should detect descriptor mismatch");
    assert!(matches!(err, ManifestError::DescriptorMismatch(0)));
}
#[test]
fn manifest_verify_detects_chunk_root_mismatch() {
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
    let dims = encoder.config.frame_dimensions;
    let frames = vec![
        RawFrame::new(dims, vec![0x01; dims.pixel_count()]).unwrap(),
        RawFrame::new(dims, vec![0x02; dims.pixel_count()]).unwrap(),
    ];
    let encoded = encoder
        .encode_segment(4, 2_500, 3, &frames, None)
        .expect("encode");
    let mut manifest = encoded.build_manifest(BaselineManifestParams {
        stream_id: hash_seed(8),
        da_endpoint: "/dns/test/quic".into(),
        ..BaselineManifestParams::default()
    });
    manifest.chunk_root[0] ^= 0xFF;
    let err = encoded
        .verify_manifest(&manifest)
        .expect_err("should detect chunk root mismatch");
    assert!(matches!(err, ManifestError::ChunkRootMismatch));
}
#[test]
fn manifest_verify_detects_audio_summary_mismatch() {
    let dims = FrameDimensions::new(4, 4);
    let audio_cfg = AudioEncoderConfig::default();
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: 1,
        frame_duration_ns: 5_000_000,
        audio: Some(audio_cfg),
        ..BaselineEncoderConfig::default()
    });
    let frame = RawFrame::new(dims, vec![0x55; dims.pixel_count()]).unwrap();
    let pcm = vec![0i16; audio_cfg.channel_count() * audio_cfg.frame_samples as usize];
    let segment = encoder
        .encode_segment(5, 10_000, 11, &[frame], Some(&pcm))
        .expect("encode segment");
    let mut manifest = segment.build_manifest(BaselineManifestParams {
        stream_id: hash_seed(7),
        da_endpoint: "/dns/audio/quic".into(),
        ..BaselineManifestParams::default()
    });
    manifest.audio_summary.as_mut().unwrap().sample_rate += 1;
    let err = segment
        .verify_manifest(&manifest)
        .expect_err("audio summary mismatch must fail");
    assert!(matches!(err, ManifestError::AudioSummaryMismatch));
}
#[test]
fn encode_manifest_decode_roundtrip() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(4, 4),
        frame_duration_ns: 40_000_000,
        quantizer: 1,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let frames = vec![
        RawFrame::new(config.frame_dimensions, vec![0x10; 16]).unwrap(),
        RawFrame::new(config.frame_dimensions, vec![0x20; 16]).unwrap(),
    ];
    let segment = encoder
        .encode_segment(33, 500, 9, &frames, None)
        .expect("encode");
    let manifest = segment.build_manifest(BaselineManifestParams {
        stream_id: hash_seed(9),
        da_endpoint: "/dns/publisher/quic".into(),
        public_metadata: StreamMetadata {
            title: "Roundtrip".into(),
            description: None,
            access_policy_id: None,
            tags: vec!["rt".into()],
        },
        capabilities: CapabilityFlags::from_bits(0b1010),
        signature: [0u8; 64],
        ..BaselineManifestParams::default()
    });
    segment.verify_manifest(&manifest).expect("manifest verify");
    let decoder = BaselineDecoder::new(config.frame_dimensions, config.frame_duration_ns);
    let decoded = decoder.decode_segment(&segment).expect("decode");
    assert_eq!(decoded.len(), frames.len());
    assert_eq!(decoded[0].index, 0);
    assert_eq!(decoded[0].pts_ns, 500);
    assert_eq!(decoded[0].luma, frames[0].luma);
    assert_eq!(decoded[1].index, 1);
    assert_eq!(decoded[1].pts_ns, 500 + config.frame_duration_ns as u64);
    assert_eq!(decoded[1].luma, frames[1].luma);
}
#[test]
fn audio_adpcm_roundtrip() {
    let mut encoder = AudioEncoder::new(AudioEncoderConfig {
        frame_samples: 64,
        ..AudioEncoderConfig::default()
    })
    .expect("encoder");
    let mut decoder = AudioDecoder::new(AudioEncoderConfig {
        frame_samples: 64,
        ..AudioEncoderConfig::default()
    })
    .expect("decoder");
    let channels = layout_channel_count(AudioLayout::Stereo);
    let mut pcm = Vec::with_capacity(channels * 64);
    for i in 0..64 {
        let angle = (i as f32) * std::f32::consts::PI * 2.0 / 32.0;
        let sample = (angle.sin() * 12_000.0) as i16;
        pcm.push(sample);
        pcm.push(sample.wrapping_mul(-1));
    }
    let frame = encoder.encode_frame(1, 1000, &pcm).expect("encode audio");
    let decoded = decoder.decode_frame(&frame).expect("decode audio");
    assert_eq!(decoded.len(), pcm.len());
    let max_err = decoded
        .iter()
        .zip(pcm.iter())
        .map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs())
        .max()
        .unwrap();
    assert!(max_err <= 11000);
}
#[test]
fn audio_encoder_errors_on_length() {
    let mut encoder = AudioEncoder::new(AudioEncoderConfig {
        frame_samples: 32,
        ..AudioEncoderConfig::default()
    })
    .expect("encoder");
    let pcm = vec![0i16; 10];
    let err = encoder
        .encode_frame(0, 0, &pcm)
        .expect_err("length mismatch must fail");
    assert!(matches!(err, AudioCodecError::InvalidPcmLength(_)));
}
#[test]
fn baseline_encoder_rejects_zero_frame_samples() {
    let dims = FrameDimensions::new(4, 4);
    let audio_cfg = AudioEncoderConfig {
        frame_samples: 0,
        ..AudioEncoderConfig::default()
    };
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        audio: Some(audio_cfg),
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config);
    let frame = RawFrame::new(dims, vec![0u8; dims.pixel_count()]).expect("frame");
    let err = encoder
        .encode_segment(1, 0, 0, &[frame], Some(&[]))
        .expect_err("zero frame_samples should error");
    assert!(matches!(
        err,
        CodecError::Audio(AudioCodecError::InvalidSampleCount(_))
    ));
}
#[test]
fn baseline_encoder_produces_audio_summary() {
    let dims = FrameDimensions::new(4, 4);
    let audio_cfg = AudioEncoderConfig {
        frame_samples: 240,
        fec_level: 1,
        target_bitrate: Some(96_000),
        ..AudioEncoderConfig::default()
    };
    let frame_duration_ns = 5_000_000;
    let mut encoder = BaselineEncoder::new(BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: 2,
        frame_duration_ns,
        audio: Some(audio_cfg),
        ..BaselineEncoderConfig::default()
    });
    let frames = vec![
        RawFrame::new(dims, vec![0x10; dims.pixel_count()]).expect("frame"),
        RawFrame::new(dims, vec![0x20; dims.pixel_count()]).expect("frame"),
    ];
    let channels = audio_cfg.channel_count();
    let samples_per_frame = audio_cfg.frame_samples as usize * channels;
    let mut pcm = Vec::with_capacity(samples_per_frame * frames.len());
    pcm.extend(vec![100i16; samples_per_frame]);
    pcm.extend(vec![-100i16; samples_per_frame]);
    let segment = encoder
        .encode_segment(31, 2_000, 7, &frames, Some(&pcm))
        .expect("encode with audio");
    let audio = segment.audio.as_ref().expect("audio track present");
    assert_eq!(audio.summary.sample_rate, audio_cfg.sample_rate);
    assert_eq!(audio.summary.frame_samples, audio_cfg.frame_samples);
    assert_eq!(audio.summary.frame_duration_ns, frame_duration_ns);
    assert_eq!(audio.summary.frames_per_segment, 2);
    assert_eq!(audio.summary.layout, audio_cfg.layout);
    assert_eq!(audio.summary.fec_level, audio_cfg.fec_level);
    assert_eq!(segment.header.audio_summary, Some(audio.summary));
}
#[test]
fn chroma_rejects_odd_dimensions() {
    let dims = FrameDimensions::new(7, 8);
    let err = Chroma420Frame::new(dims, Vec::new(), Vec::new())
        .expect_err("odd dimensions must be rejected for chroma");
    assert!(matches!(
        err,
        CodecError::ChromaDimensionsNotEven(info)
            if info.width == 7 && info.height == 8
    ));
}
#[test]
fn checked_frame_count_rejects_overflow() {
    let err =
        checked_frame_count(u16::MAX as usize + 1).expect_err("frame count above u16 should fail");
    assert!(matches!(
        err,
        CodecError::FrameCountOverflow(info)
            if info.found == u32::from(u16::MAX) + 1
    ));
}
#[test]
fn checked_chunk_count_rejects_overflow() {
    let err =
        checked_chunk_count(u16::MAX as usize + 1).expect_err("chunk count above u16 should fail");
    assert!(matches!(
        err,
        SegmentError::ChunkCountOverflow(info)
            if info.found == u32::from(u16::MAX) + 1
    ));
}
#[test]
fn audio_frame_cadence_mismatch_rejected() {
    let dims = FrameDimensions::new(8, 8);
    let audio_cfg = AudioEncoderConfig {
        sample_rate: 48_000,
        frame_samples: 240,
        layout: AudioLayout::Stereo,
        ..AudioEncoderConfig::default()
    };
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frame_duration_ns: 33_333_333,
        frames_per_segment: 1,
        audio: Some(audio_cfg),
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config);
    let frame = RawFrame::new(dims, vec![0x22; dims.pixel_count()]).expect("frame");
    let channels = audio_cfg.channel_count();
    let pcm = vec![0i16; audio_cfg.frame_samples as usize * channels];
    let err = encoder
        .encode_segment(1, 1_000, 1, &[frame], Some(&pcm))
        .expect_err("cadence mismatch must fail");
    assert!(matches!(
        err,
        CodecError::AudioFrameCadenceMismatch(info)
            if info.expected == 1600 && info.found == 240
    ));
}
#[test]
fn audio_frame_cadence_rounding_accepts_30fps() {
    let dims = FrameDimensions::new(8, 8);
    let audio_cfg = AudioEncoderConfig {
        sample_rate: 48_000,
        frame_samples: 1600,
        layout: AudioLayout::Stereo,
        ..AudioEncoderConfig::default()
    };
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frame_duration_ns: 33_333_333,
        frames_per_segment: 1,
        audio: Some(audio_cfg),
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config);
    let frame = RawFrame::new(dims, vec![0x33; dims.pixel_count()]).expect("frame");
    let channels = audio_cfg.channel_count();
    let pcm = vec![0i16; audio_cfg.frame_samples as usize * channels];
    encoder
        .encode_segment(2, 2_000, 2, &[frame], Some(&pcm))
        .expect("rounded cadence should pass");
}
#[test]
fn bundled_entropy_stats_recorded() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(8, 8),
        frames_per_segment: 1,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 2,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let frame = RawFrame::new(
        config.frame_dimensions,
        vec![0x55; config.frame_dimensions.pixel_count()],
    )
    .expect("frame");
    let segment = encoder
        .encode_segment(1, 0, 7, &[frame], None)
        .expect("encode");
    assert_eq!(segment.header.entropy_mode, EntropyMode::RansBundled);
    let telemetry = encoder
        .bundled_telemetry()
        .expect("bundled telemetry recorded");
    assert_eq!(telemetry.stats.bundle_width, config.bundle_width);
    assert!(telemetry.stats.blocks_encoded > 0);
    assert!(telemetry.stats.dc_events >= telemetry.stats.blocks_encoded);
    assert!(telemetry.stats.flush_type_complete > 0);
    assert_eq!(
        telemetry.stats.flush_end_of_block,
        telemetry.stats.blocks_encoded
    );
    assert!(!telemetry.bundles.is_empty());
    assert!(!telemetry.ans_stream.is_empty());
    let tables = default_bundle_tables();
    assert_eq!(telemetry.tables_checksum, tables.checksum());
    let decoded = decode_bundle_stream(&telemetry.ans_stream, &telemetry.bundles, tables.as_ref())
        .expect("bundled stream decodes");
    for (record, &decoded_bits) in telemetry.bundles.iter().zip(decoded.iter()) {
        let mask = (1u8 << record.bit_len) - 1;
        assert_eq!(record.bits & mask, decoded_bits & mask);
    }
    assert!(!telemetry.tokens.is_empty());
}
#[test]
fn bundled_context_stats_cover_flushes() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(8, 8),
        frames_per_segment: 1,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 3,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let frame = RawFrame::new(
        config.frame_dimensions,
        vec![0x11; config.frame_dimensions.pixel_count()],
    )
    .expect("frame");
    encoder
        .encode_segment(2, 10, 9, &[frame], None)
        .expect("bundled encode");
    let telemetry = encoder
        .bundled_telemetry()
        .expect("bundled telemetry recorded");
    assert!(
        !telemetry.context_stats.is_empty(),
        "bundled telemetry should expose per-context stats"
    );
    assert!(
        telemetry
            .context_stats
            .iter()
            .any(|ctx| ctx.flush_end_of_block > 0),
        "context stats should record at least one end-of-block flush"
    );
    assert!(
        telemetry
            .context_stats
            .iter()
            .any(|ctx| ctx.flush_context_switch > 0),
        "context switches should be tracked when contexts change"
    );
    for stats in &telemetry.context_stats {
        let type_sum = stats.type_significance_only
            + stats.type_sign_and_magnitude
            + stats.type_sign_parity
            + stats.type_sign_parity_level
            + stats.type_significance_rle;
        assert_eq!(
            stats.bundles_total, type_sum,
            "per-context bundle totals must match recorded type counters"
        );
        assert_eq!(
            stats.flush_type_complete, stats.bundles_total,
            "type-complete flushes should track per-context bundle totals"
        );
    }
}
#[test]
fn bundle_stream_recorder_counts_zero_runs_and_tokens() {
    let dims = FrameDimensions::new(8, 8);
    let tables = default_bundle_tables();
    let mut recorder = BundleStreamRecorder::new(
        2,
        dims,
        4,
        Arc::clone(&tables),
        BundleAcceleration::None,
        0,
        None,
    );
    let mut coeffs = [0i16; BLOCK_PIXELS];
    coeffs[ZIG_ZAG[1]] = 5;
    coeffs[ZIG_ZAG[4]] = -2;
    recorder.record_block(0, &coeffs, FrameType::Predicted);
    recorder.record_dc(3);
    recorder.record_ac(0, 5);
    recorder.record_ac(2, -2);
    recorder.record_eob();
    let telemetry = recorder.finish(None);
    assert_eq!(telemetry.stats.blocks_encoded, 1);
    assert_eq!(telemetry.stats.ac_events, 2);
    assert_eq!(telemetry.stats.zero_run_total, 2);
    assert_eq!(telemetry.stats.max_zero_run, 2);
    assert_eq!(telemetry.stats.nonzero_levels, 2);
    assert_eq!(telemetry.tokens.len(), 4);
    assert!(matches!(telemetry.tokens[0], BundledToken::DcDiff(3)));
    assert!(matches!(
        telemetry.tokens[1],
        BundledToken::Ac { run: 0, value } if value == 5
    ));
    assert!(matches!(
        telemetry.tokens[2],
        BundledToken::Ac { run: 2, value } if value == -2
    ));
    assert!(matches!(telemetry.tokens[3], BundledToken::EndOfBlock));
    assert!(
        telemetry.stats.significance_rle > 0,
        "zero runs should emit RLE bundles"
    );
}
#[test]
fn repeated_zero_ac_blocks_match_individual_bundle_accounting() {
    let escape = BundleContextId(u16::MAX);
    for width in 1..=4 {
        for remap in [
            None,
            Some(Arc::new(BundleContextRemap::new(
                BTreeMap::new(),
                escape,
                0,
                0,
            ))),
        ] {
            let recorder = || {
                BundleStreamRecorder::new(
                    width,
                    FrameDimensions::new(128, 128),
                    16,
                    default_bundle_tables(),
                    BundleAcceleration::None,
                    0,
                    remap.clone(),
                )
            };
            let (mut cached, mut individual) = (recorder(), recorder());
            for pass in 0..3 {
                for frame_type in [FrameType::Intra, FrameType::Predicted] {
                    for index in [0, 1, 16, 119, 136] {
                        for dc in [0, -2, 3] {
                            let mut levels = [0; BLOCK_PIXELS];
                            levels[0] = dc;
                            if pass == 1 && index == 1 {
                                levels[ZIG_ZAG[8]] = -1;
                            }
                            cached.record_block(index, &levels, frame_type);
                            let tile = ((index % 16) & 7) | (((index / 16) & 7) << 3);
                            individual.record_ac_slots(tile as u8, &levels, frame_type);
                            for recorder in [&mut cached, &mut individual] {
                                recorder.record_dc(dc);
                                if levels[ZIG_ZAG[8]] != 0 {
                                    recorder.record_ac(7, -1);
                                }
                                recorder.record_eob();
                            }
                        }
                    }
                }
            }
            assert!(
                cached
                    .zero_ac_patterns
                    .iter()
                    .any(|pattern| pattern.repeats > 1)
            );
            assert!(cached.bundles.len() < individual.bundles.len());
            let reference = individual.finish(None);
            let mut previous = reference.clone();
            previous.ans_stream.fill(0xff);
            previous.bundles.reverse();
            let buffers = (previous.bundles.as_ptr(), previous.ans_stream.as_ptr());
            let expanded = cached.finish(Some(previous));
            assert_eq!(
                buffers,
                (expanded.bundles.as_ptr(), expanded.ans_stream.as_ptr())
            );
            assert_eq!(
                crate::core::to_bytes(&expanded).unwrap(),
                crate::core::to_bytes(&reference).unwrap(),
                "width {width}, remap {}",
                remap.is_some()
            );
        }
    }
}

#[test]
fn recorder_orders_contexts_and_attributes_flushes_to_the_previous_context() {
    let mut recorder = BundleStreamRecorder::new(
        2,
        FrameDimensions::new(8, 8),
        16,
        default_bundle_tables(),
        BundleAcceleration::None,
        0,
        None,
    );
    for id in [u16::MAX, 0, 0, u16::MAX] {
        recorder.record_bundle(
            BundleType::SignificanceOnly,
            BundleContextId(id),
            1,
            1,
            BundleFlushReason::TypeComplete,
        );
    }
    recorder.record_eob();
    let report = recorder.finish(None);
    assert_eq!(report.stats.flush_context_switch, 2);
    assert_eq!(report.context_stats.len(), 2);
    for (i, id) in [0, u16::MAX].into_iter().enumerate() {
        let stats = &report.context_stats[i];
        let freq = &report.context_frequencies[i];
        assert_eq!(stats.context, BundleContextId(id));
        assert_eq!(freq.context, stats.context);
        assert_eq!(stats.flush_context_switch, 1);
        assert_eq!(stats.flush_end_of_block, i as u64);
        assert_eq!(stats.bundles_total, 2);
        assert_eq!(freq.symbol_counts[1], 2);
    }
}

#[test]
fn recorder_splits_remapped_zero_runs_at_nonzero_coefficients_and_end_of_block() {
    let escape = BundleContextId(u16::MAX);
    let remap = BundleContextRemap::new(BTreeMap::new(), escape, 0, 0);
    let mut recorder = BundleStreamRecorder::new(
        2,
        FrameDimensions::new(8, 8),
        16,
        default_bundle_tables(),
        BundleAcceleration::None,
        0,
        Some(Arc::new(remap)),
    );
    let mut coeffs = [0; BLOCK_PIXELS];
    coeffs[ZIG_ZAG[6]] = -1;
    recorder.record_block(0, &coeffs, FrameType::Predicted);
    recorder.record_eob();
    let report = recorder.finish(None);
    let (first, rest) = report.bundles.split_at(3);
    assert_eq!(first[0].bits, 3);
    assert_eq!(first[1].bits, 2);
    assert!(matches!(first[2].bundle_type, BundleType::SignAndMagnitude));
    assert_eq!(first[2].bits, 3);
    assert_eq!(rest.len(), 19);
    assert!(
        rest.iter()
            .all(|b| b.bits == 3 && matches!(b.bundle_type, BundleType::SignificanceRle))
    );
    assert!(report.bundles.iter().all(|b| b.context == escape));
    assert_eq!(report.stats.flush_context_switch, 0);
}

#[test]
fn bundle_stream_recorder_flushes_zero_only_blocks() {
    let dims = FrameDimensions::new(8, 8);
    let tables = default_bundle_tables();
    let mut recorder = BundleStreamRecorder::new(
        2,
        dims,
        4,
        Arc::clone(&tables),
        BundleAcceleration::None,
        0,
        None,
    );
    let coeffs = [0i16; BLOCK_PIXELS];
    recorder.record_block(0, &coeffs, FrameType::Intra);
    recorder.record_dc(0);
    recorder.record_eob();
    let telemetry = recorder.finish(None);
    assert_eq!(telemetry.stats.blocks_encoded, 1);
    let expected_slots = u16::try_from(BLOCK_PIXELS - 1).expect("block pixel count fits u16");
    let consumed: u16 = telemetry
        .bundles
        .iter()
        .filter(|record| matches!(record.bundle_type, BundleType::SignificanceRle))
        .map(|record| u16::from(record.bits))
        .sum();
    assert_eq!(
        consumed, expected_slots,
        "run-length bundles must cover every AC slot"
    );
    assert_eq!(
        telemetry.stats.flush_type_complete,
        telemetry.bundles.len() as u64
    );
    assert_eq!(telemetry.stats.flush_end_of_block, 1);
    assert_eq!(
        telemetry.stats.significance_rle,
        telemetry.bundles.len() as u64
    );
}
#[test]
fn bundle_stream_recorder_accumulates_significance_stats() {
    let dims = FrameDimensions::new(8, 8);
    let tables = default_bundle_tables();
    // Use width four so both parity and geq2 counters are active.
    let mut recorder = BundleStreamRecorder::new(
        4,
        dims,
        4,
        Arc::clone(&tables),
        BundleAcceleration::None,
        0,
        None,
    );
    let mut coeffs = [0i16; BLOCK_PIXELS];
    // Populate the first three AC slots with deterministic values.
    coeffs[ZIG_ZAG[1]] = 3; // odd positive
    coeffs[ZIG_ZAG[2]] = -4; // even negative (>= 2)
    coeffs[ZIG_ZAG[3]] = 1; // odd positive
    recorder.record_block(0, &coeffs, FrameType::Intra);
    recorder.record_dc(7);
    recorder.record_ac(0, 3);
    recorder.record_ac(0, -4);
    recorder.record_ac(0, 1);
    recorder.record_eob();
    let telemetry = recorder.finish(None);
    let total_ac = u64::try_from(BLOCK_PIXELS - 1).expect("block size fits u64");
    assert_eq!(telemetry.stats.significance_one, 3);
    assert_eq!(telemetry.stats.significance_zero, total_ac - 3);
    assert_eq!(telemetry.stats.sign_positive, total_ac - 1);
    assert_eq!(telemetry.stats.sign_negative, 1);
    assert_eq!(telemetry.stats.parity_one, 2);
    assert_eq!(telemetry.stats.geq2_one, 2);
}
#[test]
fn bundle_prefetch_keeps_stream_deterministic() {
    let dims = FrameDimensions::new(8, 8);
    let tables = default_bundle_tables();
    let mut recorder = BundleStreamRecorder::new(
        2,
        dims,
        6,
        Arc::clone(&tables),
        BundleAcceleration::None,
        3,
        None,
    );
    for block_idx in 0..6 {
        let mut coeffs = [0i16; BLOCK_PIXELS];
        coeffs[ZIG_ZAG[1]] = (block_idx as i16) + 1;
        recorder.record_block(block_idx, &coeffs, FrameType::Predicted);
        recorder.record_dc(block_idx as i16);
        recorder.record_ac(0, coeffs[ZIG_ZAG[1]]);
        recorder.record_eob();
    }
    let telemetry = recorder.finish(None);
    let baseline_stream = encode_bundle_stream_scalar(tables.as_ref(), &telemetry.bundles, 0);
    let prefetch_stream = encode_bundle_stream_scalar(
        tables.as_ref(),
        &telemetry.bundles,
        telemetry.prefetch_distance,
    );
    let decoded_prefetch =
        decode_bundle_stream(&telemetry.ans_stream, &telemetry.bundles, tables.as_ref())
            .expect("prefetch stream decodes");
    let decoded_prefetch_stream =
        decode_bundle_stream(&prefetch_stream, &telemetry.bundles, tables.as_ref())
            .expect("prefetch stream decodes");
    let decoded_baseline =
        decode_bundle_stream(&baseline_stream, &telemetry.bundles, tables.as_ref())
            .expect("baseline stream decodes");
    assert_eq!(decoded_prefetch, decoded_baseline);
    assert_eq!(decoded_prefetch_stream, decoded_baseline);
    assert_eq!(telemetry.prefetch_distance, 3);
    assert_eq!(telemetry.acceleration, BundleAcceleration::None);
}
fn sample_bundles() -> Vec<BundleRecord> {
    vec![
        BundleRecord {
            bundle_type: BundleType::SignificanceOnly,
            context: BundleContextId::new(1),
            bits: 0b1,
            bit_len: 1,
            flush: BundleFlushReason::TypeComplete,
        },
        BundleRecord {
            bundle_type: BundleType::SignAndMagnitude,
            context: BundleContextId::new(2),
            bits: 0b10,
            bit_len: 2,
            flush: BundleFlushReason::TypeComplete,
        },
        BundleRecord {
            bundle_type: BundleType::SignParity,
            context: BundleContextId::new(3),
            bits: 0b101,
            bit_len: 3,
            flush: BundleFlushReason::TypeComplete,
        },
    ]
}
#[test]
fn bundle_stream_simd_roundtrips() {
    let tables = default_bundle_tables();
    let bundles = sample_bundles();
    let stream = encode_bundle_stream_simd(tables.as_ref(), &bundles, 0);
    assert!(stream.starts_with(&SIMD_BUNDLE_MAGIC));
    let decoded = decode_bundle_stream(&stream, &bundles, tables.as_ref()).expect("decode");
    let expected: Vec<u8> = bundles
        .iter()
        .map(|record| record.bits & ((1u8 << record.bit_len) - 1))
        .collect();
    assert_eq!(decoded, expected);
}
#[test]
fn simd_bundle_lane_len_reader_rejects_truncated_or_overflowed_offsets() {
    let bytes = 13u32.to_le_bytes();
    let mut cursor = 0usize;
    assert_eq!(read_simd_bundle_lane_len(&bytes, &mut cursor).unwrap(), 13);
    assert_eq!(cursor, 4);
    for len in 0..4 {
        let mut cursor = 0usize;
        let err = read_simd_bundle_lane_len(&bytes[..len], &mut cursor)
            .expect_err("truncated lane length should fail closed");
        assert!(matches!(err, BundleDecodeError::InvalidSimdHeader));
        assert_eq!(cursor, 0);
    }
    let mut overflow_cursor = usize::MAX;
    let err = read_simd_bundle_lane_len(&[], &mut overflow_cursor)
        .expect_err("cursor overflow should fail closed");
    assert!(matches!(err, BundleDecodeError::InvalidSimdHeader));
    assert_eq!(overflow_cursor, usize::MAX);
}
#[test]
fn bundle_stream_acceleration_matches_header() {
    let tables = default_bundle_tables();
    let bundles = sample_bundles();
    let (stream, accel) =
        encode_bundle_stream_with_opts(tables.as_ref(), &bundles, BundleAcceleration::CpuSimd, 0);
    let expected = if stream.starts_with(&SIMD_BUNDLE_MAGIC) {
        BundleAcceleration::CpuSimd
    } else {
        BundleAcceleration::None
    };
    assert_eq!(accel, expected);
}
#[test]
fn rle_bundles_roundtrip_via_ans() {
    let tables = default_bundle_tables();
    let bundles = vec![
        BundleRecord {
            bundle_type: BundleType::SignificanceRle,
            context: BundleContextId::new(1),
            bits: 1,
            bit_len: 3,
            flush: BundleFlushReason::TypeComplete,
        },
        BundleRecord {
            bundle_type: BundleType::SignificanceRle,
            context: BundleContextId::new(2),
            bits: 3,
            bit_len: 3,
            flush: BundleFlushReason::TypeComplete,
        },
    ];
    let stream = encode_bundle_stream_scalar(tables.as_ref(), &bundles, 0);
    let decoded = decode_bundle_stream(&stream, &bundles, tables.as_ref()).expect("decode rle");
    for (record, decoded_bits) in bundles.iter().zip(decoded.iter()) {
        let mask = (1u8 << record.bit_len) - 1;
        assert_eq!(record.bits & mask, decoded_bits & mask);
    }
}
#[test]
fn rle_all_one_symbols_roundtrip() {
    let tables = default_bundle_tables();
    let mut bundles = Vec::with_capacity(BLOCK_PIXELS - 1);
    for idx in 0..(BLOCK_PIXELS - 1) {
        bundles.push(BundleRecord {
            bundle_type: BundleType::SignificanceRle,
            context: BundleContextId::new(idx as u16),
            bits: 1,
            bit_len: 3,
            flush: BundleFlushReason::TypeComplete,
        });
    }
    let stream = encode_bundle_stream_scalar(tables.as_ref(), &bundles, 0);
    let decoded = decode_bundle_stream(&stream, &bundles, tables.as_ref()).expect("rle decode");
    assert_eq!(decoded.len(), bundles.len());
    assert!(decoded.iter().all(|&value| value == 1));
}
#[test]
fn simd_bundle_stream_matches_scalar_output() {
    if !cpu_simd_supported() {
        return;
    }
    let dims = FrameDimensions::new(8, 8);
    let tables = default_bundle_tables();
    let mut recorder = BundleStreamRecorder::new(
        3,
        dims,
        5,
        Arc::clone(&tables),
        BundleAcceleration::CpuSimd,
        2,
        None,
    );
    let mut coeffs = [0i16; BLOCK_PIXELS];
    coeffs[ZIG_ZAG[1]] = 5;
    coeffs[ZIG_ZAG[2]] = -3;
    coeffs[ZIG_ZAG[4]] = 4;
    recorder.record_block(0, &coeffs, FrameType::Intra);
    recorder.record_dc(1);
    recorder.record_ac(0, coeffs[ZIG_ZAG[1]]);
    recorder.record_ac(0, coeffs[ZIG_ZAG[2]]);
    recorder.record_ac(1, coeffs[ZIG_ZAG[4]]);
    recorder.record_eob();
    let telemetry = recorder.finish(None);
    let scalar_stream = encode_bundle_stream_scalar(tables.as_ref(), &telemetry.bundles, 0);
    let simd_decoded =
        decode_bundle_stream(&telemetry.ans_stream, &telemetry.bundles, tables.as_ref())
            .expect("simd decode");
    let scalar_decoded = decode_bundle_stream(&scalar_stream, &telemetry.bundles, tables.as_ref())
        .expect("scalar decode");
    assert_eq!(telemetry.acceleration, BundleAcceleration::None);
    assert_eq!(simd_decoded, scalar_decoded);
    assert_eq!(telemetry.prefetch_distance, 2);
}
#[test]
fn bundled_manifest_requires_checksum() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(8, 8),
        frames_per_segment: 1,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 2,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let frame = RawFrame::new(
        config.frame_dimensions,
        vec![0x33; config.frame_dimensions.pixel_count()],
    )
    .expect("frame");
    let segment = encoder
        .encode_segment(2, 0, 5, &[frame], None)
        .expect("bundled segment");
    let mut manifest = segment.build_manifest(BaselineManifestParams {
        stream_id: [0x20; 32],
        ..BaselineManifestParams::default()
    });
    assert!(
        manifest.entropy_tables_checksum.is_some(),
        "bundled manifest must carry checksum"
    );
    segment
        .verify_manifest(&manifest)
        .expect("checksum matches");
    manifest.entropy_tables_checksum = None;
    let err = segment
        .verify_manifest(&manifest)
        .expect_err("missing checksum must fail");
    assert!(matches!(err, ManifestError::EntropyTablesMismatch));
}
#[test]
fn manifest_capabilities_follow_entropy_mode() {
    let bundled_config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(8, 8),
        frames_per_segment: 1,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 2,
        ..BaselineEncoderConfig::default()
    };
    let mut bundled_encoder = BaselineEncoder::new(bundled_config.clone());
    let bundled_frame = RawFrame::new(
        bundled_config.frame_dimensions,
        vec![0x21; bundled_config.frame_dimensions.pixel_count()],
    )
    .expect("frame");
    let bundled_segment = bundled_encoder
        .encode_segment(3, 0, 5, &[bundled_frame], None)
        .expect("bundled segment");
    let manifest = bundled_segment.build_manifest(BaselineManifestParams {
        capabilities: CapabilityFlags::from_bits(0),
        ..BaselineManifestParams::default()
    });
    assert!(
        manifest
            .capabilities
            .contains(CapabilityFlags::FEATURE_ENTROPY_BUNDLED),
        "bundled manifests must set FEATURE_ENTROPY_BUNDLED"
    );
}
#[test]
fn bundled_manifest_missing_entropy_feature_bit_rejected() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(8, 8),
        frames_per_segment: 1,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 2,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let frame = RawFrame::new(
        config.frame_dimensions,
        vec![0x33; config.frame_dimensions.pixel_count()],
    )
    .expect("frame");
    let segment = encoder
        .encode_segment(6, 0, 5, &[frame], None)
        .expect("bundled segment");
    let mut manifest = segment.build_manifest(BaselineManifestParams::default());
    manifest.capabilities = manifest
        .capabilities
        .remove(CapabilityFlags::FEATURE_ENTROPY_BUNDLED);
    let err = segment
        .verify_manifest(&manifest)
        .expect_err("missing bundled bit must fail manifest verification");
    assert!(matches!(
        err,
        ManifestError::CapabilityEntropyFlagMismatch {
            required_bundled: true,
            found_bundled: false
        }
    ));
}
#[test]
fn bundled_manifest_missing_required_acceleration_bit_rejected() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(8, 8),
        frames_per_segment: 1,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 2,
        bundle_acceleration: BundleAcceleration::CpuSimd,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config);
    let frame = RawFrame::new(
        FrameDimensions::new(8, 8),
        vec![0x66; FrameDimensions::new(8, 8).pixel_count()],
    )
    .expect("frame");
    let segment = encoder
        .encode_segment(9, 0, 5, &[frame], None)
        .expect("bundled segment");
    let mut manifest = segment.build_manifest(BaselineManifestParams::default());
    manifest.capabilities = manifest
        .capabilities
        .remove(CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD);
    let err = segment
        .verify_manifest(&manifest)
        .expect_err("missing acceleration bit must fail verification");
    assert!(matches!(
        err,
        ManifestError::CapabilityAccelerationFlagMismatch {
            required_mask,
            found_mask,
            ..
        }
        if required_mask == CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD
            && found_mask == 0
    ));
}
#[test]
fn bundled_manifest_with_wrong_acceleration_bit_rejected() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(8, 8),
        frames_per_segment: 1,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 2,
        bundle_acceleration: BundleAcceleration::Gpu,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config);
    let frame = RawFrame::new(
        FrameDimensions::new(8, 8),
        vec![0x77; FrameDimensions::new(8, 8).pixel_count()],
    )
    .expect("frame");
    let segment = encoder
        .encode_segment(10, 0, 5, &[frame], None)
        .expect("bundled segment");
    let mut manifest = segment.build_manifest(BaselineManifestParams::default());
    manifest.capabilities = manifest
        .capabilities
        .remove(CapabilityFlags::FEATURE_BUNDLE_ACCEL_GPU)
        .insert(CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD);
    let err = segment
        .verify_manifest(&manifest)
        .expect_err("wrong acceleration bit must fail verification");
    assert!(matches!(
        err,
        ManifestError::CapabilityAccelerationFlagMismatch {
            required_mask,
            found_mask,
            ..
        }
        if required_mask == CapabilityFlags::FEATURE_BUNDLE_ACCEL_GPU
            && found_mask == CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD
    ));
}
#[test]
fn bundled_telemetry_roundtrips_tokens_via_ans_stream() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(8, 8),
        frames_per_segment: 1,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 3,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config);
    let frame = RawFrame::new(
        FrameDimensions::new(8, 8),
        vec![0x22; FrameDimensions::new(8, 8).pixel_count()],
    )
    .expect("frame");
    encoder
        .encode_segment(10, 0, 7, &[frame], None)
        .expect("bundled segment");
    let telemetry = encoder
        .bundled_telemetry()
        .expect("bundled telemetry recorded")
        .clone();
    let tables = default_bundle_tables();
    telemetry
        .verify_stream(tables.as_ref())
        .expect("bundled ANS stream verifies");
    let decoded = telemetry
        .decode_symbols(tables.as_ref())
        .expect("decoded symbols");
    assert_eq!(decoded.len(), telemetry.bundles.len());
}
#[test]
fn bundled_telemetry_checksum_mismatch_signalled() {
    let config = BaselineEncoderConfig {
        frame_dimensions: FrameDimensions::new(8, 8),
        frames_per_segment: 1,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 2,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config);
    let frame = RawFrame::new(
        FrameDimensions::new(8, 8),
        vec![0x33; FrameDimensions::new(8, 8).pixel_count()],
    )
    .expect("frame");
    encoder
        .encode_segment(11, 0, 7, &[frame], None)
        .expect("bundled segment");
    let mut telemetry = encoder
        .bundled_telemetry()
        .expect("bundled telemetry recorded")
        .clone();
    telemetry.tables_checksum = [0xAA; 32];
    let tables = default_bundle_tables();
    let err = telemetry
        .verify_stream(tables.as_ref())
        .expect_err("checksum mismatch must be reported");
    assert!(matches!(err, BundleDecodeError::ChecksumMismatch { .. }));
}
#[test]
fn segment_bundle_roundtrip_validates_payloads() {
    let dims = FrameDimensions::new(8, 8);
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: 2,
        frame_duration_ns: 20_000_000,
        quantizer: 4,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config.clone());
    let frame0 = RawFrame::new(dims, vec![0x55; dims.pixel_count()]).expect("frame zero");
    let frame1 = RawFrame::new(dims, vec![0x77; dims.pixel_count()]).expect("frame one");
    let uv_len = usize::from(dims.width / 2 * dims.height / 2);
    let chroma0 =
        Chroma420Frame::new(dims, vec![0x11; uv_len], vec![0x22; uv_len]).expect("chroma0");
    let chroma1 =
        Chroma420Frame::new(dims, vec![0x33; uv_len], vec![0x44; uv_len]).expect("chroma1");
    let segment = encoder
        .encode_segment(12, 1_000_000, 5, &[frame0, frame1], None)
        .expect("encode");
    let bundle = segment.to_bundle_with_chroma(
        config.frame_dimensions,
        config.frame_duration_ns,
        vec![chroma0.clone(), chroma1.clone()],
    );
    let (roundtrip, bundle_dims, bundle_duration, chroma_roundtrip) =
        bundle.into_segment_with_chroma().expect("bundle decodes");
    assert_eq!(bundle_dims, config.frame_dimensions);
    assert_eq!(bundle_duration, config.frame_duration_ns);
    assert_eq!(roundtrip.header, segment.header);
    assert_eq!(roundtrip.descriptors, segment.descriptors);
    assert_eq!(roundtrip.chunks, segment.chunks);
    assert_eq!(roundtrip.audio, segment.audio);
    assert_eq!(chroma_roundtrip, vec![chroma0, chroma1]);
}
#[test]
fn sampled_ac_energy_excludes_dc_and_handles_full_range_residuals() {
    let dims = FrameDimensions::new(8, 8);
    let source: Vec<_> = (0..64).map(|i| if i % 2 == 0 { 255 } else { 0 }).collect();
    let opposite: Vec<_> = source.iter().map(|v| 255 - v).collect();
    let mut costs = PredictionEnergy::default();
    costs.add_plane(&source, &opposite, dims);
    assert_eq!(costs.intra, 32 * 32 * 255 * 255);
    assert_eq!(costs.predicted, 4 * costs.intra);
    assert_eq!(costs.frame_type(), FrameType::Intra);

    let mut costs = PredictionEnergy::default();
    costs.add_plane(&[255; 64], &[0; 64], dims);
    assert_eq!((costs.intra, costs.predicted), (0, 0));
    assert_eq!(costs.frame_type(), FrameType::Predicted);
    costs.add_plane(&source, &source, dims);
    assert_eq!(costs.frame_type(), FrameType::Predicted);
}

#[test]
fn sampled_cost_weights_uneven_regions_and_requires_a_strict_margin() {
    let dims = FrameDimensions::new(7 * 8, 5 * 8);
    let mut frame = vec![0; dims.pixel_count()];
    let previous = frame.clone();
    // The last 2×2-block region is represented by the block at (6, 4).
    for y in 4 * 8..5 * 8 {
        for x in 6 * 8..7 * 8 {
            frame[y * usize::from(dims.width) + x] = if x % 2 == 0 { 255 } else { 0 };
        }
    }
    let mut costs = PredictionEnergy::default();
    costs.add_plane(&frame, &previous, dims);
    assert_eq!(costs.intra, 4 * 32 * 32 * 255 * 255);
    assert_eq!(costs.predicted, costs.intra);

    let mut boundary = PredictionEnergy {
        intra: 80,
        predicted: 100,
    };
    assert_eq!(boundary.frame_type(), FrameType::Predicted);
    boundary.intra -= 1;
    assert_eq!(boundary.frame_type(), FrameType::Intra);
}

#[test]
fn chroma_only_cut_refreshes_all_planes_and_the_next_frame_predicts_from_it() {
    let dims = FrameDimensions::new(18, 14);
    let cfg = BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: 3,
        quantizer: 1,
        ..Default::default()
    };
    let decoder = BaselineDecoder::new(dims, cfg.frame_duration_ns);
    let luma = vec![RawFrame::new(dims, vec![0; dims.pixel_count()]).unwrap(); 3];
    let uv_len = dims.pixel_count() / 4;
    let black = Chroma420Frame::new(dims, vec![0; uv_len], vec![0; uv_len]).unwrap();
    let mut pattern = black.clone();
    for (i, sample) in pattern.v.iter_mut().enumerate() {
        *sample = if i % 2 == 0 { 255 } else { 0 };
    }
    let segment = BaselineEncoder::new(cfg)
        .encode_segment_with_chroma(
            0,
            0,
            0,
            &luma,
            Some(&[pattern, black.clone(), black.clone()]),
            None,
        )
        .unwrap();
    let types: Vec<_> = segment
        .chunks
        .iter()
        .map(|c| FrameType::from_byte(c[12]).unwrap())
        .collect();
    assert_eq!(
        types,
        [FrameType::Intra, FrameType::Intra, FrameType::Predicted]
    );
    // Six luma blocks and two blocks on each padded chroma plane, all DC-only.
    assert_eq!(segment.chunks[1].len(), FRAME_HEADER_LEN + 8 + 10 * 5);
    let decoded = decoder.decode_segment(&segment).unwrap();
    for frame in &decoded[1..] {
        assert_eq!(frame.luma, luma[0].luma);
        assert_eq!(frame.chroma.as_ref(), Some(&black));
    }
}

#[test]
fn scene_analysis_validates_both_chroma_planes_before_sampling() {
    let dims = FrameDimensions::new(18, 14);
    let cfg = BaselineEncoderConfig {
        frame_dimensions: dims,
        ..Default::default()
    };
    let frames = vec![RawFrame::new(dims, vec![100; dims.pixel_count()]).unwrap(); 2];
    let valid = Chroma420Frame::neutral(dims);
    for short_u in [false, true] {
        let mut short = valid.clone();
        if short_u {
            short.u.pop();
        } else {
            short.v.pop();
        }
        let constructed = Chroma420Frame::new(dims, short.u.clone(), short.v.clone());
        let result = BaselineEncoder::new(cfg.clone()).encode_segment_with_chroma(
            1,
            0,
            0,
            &frames,
            Some(&[valid.clone(), short]),
            None,
        );
        for error in [constructed.unwrap_err(), result.unwrap_err()] {
            assert!(matches!(
                error,
                CodecError::InvalidFrameLength(FrameLengthMismatch {
                    actual: 62,
                    expected: 63
                })
            ));
        }
    }
}

#[cfg(feature = "streaming-neural-filter")]
#[test]
fn predicted_frame_uses_the_filtered_luma_seen_by_the_decoder() {
    let dims = FrameDimensions::new(16, 16);
    let cfg = BaselineEncoderConfig {
        frame_dimensions: dims,
        quantizer: 0,
        frames_per_segment: 2,
        ..Default::default()
    };
    let frame = RawFrame::new(
        dims,
        (0..dims.pixel_count()).map(|i| (i * 77) as u8).collect(),
    )
    .unwrap();
    let decoder = BaselineDecoder::new(dims, cfg.frame_duration_ns);
    let mut encoder = BaselineEncoder::new(cfg);
    let first = encoder
        .encode_segment(1, 0, 0, std::slice::from_ref(&frame), None)
        .unwrap();
    let displayed = decoder.decode_segment(&first).unwrap().remove(0).luma;
    assert_ne!(frame.luma, displayed);
    let repeat = RawFrame::new(dims, displayed.clone()).unwrap();
    let encoded = encoder
        .encode_segment(1, 0, 0, &[frame, repeat], None)
        .unwrap();
    assert_eq!(encoded.chunks[1][12], FrameType::Predicted.as_byte());
    assert_eq!(
        encoded.chunks[1].len(),
        FRAME_HEADER_LEN + dims.block_count() * 5
    );
    let mut expected = displayed;
    apply_neural_filter(&mut expected, dims);
    assert_eq!(decoder.decode_segment(&encoded).unwrap()[1].luma, expected);
}

#[test]
fn ac_zero_midpoints_save_tokens_for_both_signs() {
    for sign in [1, -1] {
        let mut transform = [0; BLOCK_PIXELS];
        transform[0] = 8 * sign; // DC delta always occupies two bytes.
        transform[2] = 5 * sign; // AC step 10, exact midpoint.
        transform[3] = 9 * sign; // AC step 16, past the midpoint.
        transform[1] = 6 * sign; // AC step 11, odd steps have no tie.
        let levels = quantize_coeffs(&transform, 1);
        assert_eq!(&levels[..4], &[sign as i16, sign as i16, 0, sign as i16]);
        assert_eq!(quantize_coeffs(&transform, 0)[2], (5 * sign) as i16);
    }
}

#[test]
fn rdo_minimizes_transform_error_for_the_available_tokens() {
    // Exhaustively compare all subsets on a small set of coefficients. Vary the
    // source magnitude to exercise empty, partial, and full budgets.
    let slots = [1, 4, 8, 15, 40, 63];
    for amplitude in [0, 1, 3, 8, 32] {
        let mut transform = [0; BLOCK_PIXELS];
        for (&slot, c) in slots.iter().zip([18, -22, 45, -91, 15, 133]) {
            transform[slot] = amplitude * c;
        }
        let original = quantize_coeffs(&transform, 1);
        let budget = slots
            .iter()
            .filter(|&&i| original[i].unsigned_abs() >= 4)
            .count() as u32;
        let mut selected = original;
        let mut optimizer = BlockRdo::new(RdoMode::DynamicProgramming, 1, 2, None).unwrap();
        optimizer.select_levels(&transform, &mut selected, FrameType::Intra, 0);
        let error = |levels: &[i16; BLOCK_PIXELS]| -> i64 {
            let decoded = dequantize_coeffs(levels, 1);
            slots
                .iter()
                .map(|&i| i64::from(transform[i] - decoded[i]).pow(2))
                .sum()
        };
        let selected_error = error(&selected);
        assert_eq!(
            selected.iter().filter(|&&x| x != 0).count(),
            budget as usize
        );
        for mask in 0u32..(1 << slots.len()) {
            if mask.count_ones() <= budget {
                let mut alternative = [0; BLOCK_PIXELS];
                for (i, &slot) in slots.iter().enumerate() {
                    if mask & (1 << i) != 0 {
                        alternative[slot] = original[slot];
                    }
                }
                assert!(
                    selected_error <= error(&alternative),
                    "amplitude={amplitude}, mask={mask}"
                );
            }
        }
    }
}

#[test]
fn rdo_breaks_equal_reconstruction_gains_by_raster_position() {
    let mut transform = [0; BLOCK_PIXELS];
    transform[2] = 40; // The one level of magnitude 4 sets a one-token budget.
    transform[3] = -48;
    transform[18] = 48; // Same step and reconstruction gain as slot 3.
    let mut levels = quantize_coeffs(&transform, 1);
    let mut optimizer = BlockRdo::new(RdoMode::DynamicProgramming, 1, 2, None).unwrap();
    optimizer.select_levels(&transform, &mut levels, FrameType::Predicted, 3);
    assert_eq!((levels[2], levels[3], levels[18]), (0, -3, 0));
}

#[test]
fn rdo_reports_luma_record_bits_before_frame_compression() {
    let dims = FrameDimensions::new(15, 11);
    let frames: Vec<_> = (0..3)
        .map(|n| {
            RawFrame::new(
                dims,
                (0..dims.pixel_count())
                    .map(|i| ((i * 13 + n * 23) % 256) as u8)
                    .collect(),
            )
            .unwrap()
        })
        .collect();
    for mode in [
        RdoMode::DynamicProgramming,
        RdoMode::Perceptual,
        RdoMode::Neural,
    ] {
        let cfg = BaselineEncoderConfig {
            frame_dimensions: dims,
            frames_per_segment: 3,
            quantizer: 1,
            rdo_mode: mode,
            ..Default::default()
        };
        let decoder = BaselineDecoder::new(dims, cfg.frame_duration_ns);
        let mut encoder = BaselineEncoder::new(cfg);
        let segment = encoder.encode_segment(1, 0, 0, &frames, None).unwrap();
        let decoded = decoder.decode_segment(&segment).unwrap();
        assert_eq!(decoded.len(), frames.len());
        assert!(decoded.iter().all(|f| f.luma.len() == dims.pixel_count()));
        let report = encoder.bundled_telemetry().unwrap().rdo.as_ref().unwrap();
        let block_bytes: usize = segment
            .chunks
            .iter()
            .map(|chunk| {
                let body = &chunk[FRAME_HEADER_LEN..];
                if chunk[12] & FRAME_ZSTD == 0 {
                    body.len()
                } else {
                    zstd::decompressed_size(body).unwrap() as usize
                }
            })
            .sum();
        assert_eq!(report.after_rate_bits, block_bytes as u64 * 8);
        assert!(report.after_rate_bits < report.before_rate_bits);
        assert_eq!(report.blocks_optimized, 12);
        if mode == RdoMode::Neural {
            assert_eq!(
                report.neural_class_histogram.iter().sum::<u64>(),
                report.blocks_optimized
            );
        }
        let repeat = encoder.encode_segment(1, 0, 0, &frames, None).unwrap();
        assert_eq!(repeat.chunks, segment.chunks);
    }
}

#[test]
fn rdo_perceptual_uses_softer_lambda() {
    let default = rdo_lambda_hundredths(RdoMode::DynamicProgramming, 20);
    let perceptual = rdo_lambda_hundredths(RdoMode::Perceptual, 20);
    assert!(
        perceptual < default,
        "perceptual lambda should be softer than default for SSIM bias"
    );
}
#[test]
fn entropy_mode_parsing_rejects_unknown_strings() {
    assert_eq!(EntropyMode::from_str("rans"), Err(()));
    assert_eq!(EntropyMode::from_str("rans-unknown"), Err(()));
    assert_eq!(EntropyMode::from_str("cabac"), Err(()));
    assert_eq!(
        EntropyMode::from_str("rans_bundled"),
        Ok(EntropyMode::RansBundled)
    );
}
#[test]
fn bundled_ans_roundtrip_order() {
    let records = vec![
        BundleRecord {
            bundle_type: BundleType::SignificanceOnly,
            context: BundleContextId::new(1),
            bits: 0,
            bit_len: 1,
            flush: BundleFlushReason::TypeComplete,
        },
        BundleRecord {
            bundle_type: BundleType::SignAndMagnitude,
            context: BundleContextId::new(2),
            bits: 0b11,
            bit_len: 2,
            flush: BundleFlushReason::TypeComplete,
        },
        BundleRecord {
            bundle_type: BundleType::SignParity,
            context: BundleContextId::new(3),
            bits: 0b101,
            bit_len: 3,
            flush: BundleFlushReason::TypeComplete,
        },
        BundleRecord {
            bundle_type: BundleType::SignParityLevel,
            context: BundleContextId::new(4),
            bits: 0b1110,
            bit_len: 4,
            flush: BundleFlushReason::TypeComplete,
        },
    ];
    let tables = default_bundle_tables();
    let stream = encode_bundle_stream(tables.as_ref(), &records);
    let decoded =
        decode_bundle_stream(&stream, &records, tables.as_ref()).expect("bundle stream decodes");
    assert_eq!(
        decoded,
        records
            .iter()
            .map(|r| r.bits & ((1u8 << r.bit_len) - 1))
            .collect::<Vec<_>>()
    );
}
#[test]
fn verify_segment_accepts_audio_within_tolerance() {
    let dims = FrameDimensions::new(4, 4);
    let audio_cfg = AudioEncoderConfig::default();
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: 2,
        frame_duration_ns: 5_000_000,
        audio: Some(audio_cfg),
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config);
    let frames = vec![
        RawFrame::new(dims, vec![0x11; dims.pixel_count()]).expect("frame"),
        RawFrame::new(dims, vec![0x22; dims.pixel_count()]).expect("frame"),
    ];
    let channels = audio_cfg.channel_count();
    let samples_per_frame = audio_cfg.frame_samples as usize * channels;
    let pcm = vec![0i16; samples_per_frame * frames.len()];
    let segment = encoder
        .encode_segment(41, 3_000, 5, &frames, Some(&pcm))
        .expect("encode segment");
    let mut skewed = segment.clone();
    if let Some(audio) = skewed.audio.as_mut() {
        audio.frames[0].timestamp_ns += AUDIO_SYNC_TOLERANCE_NS - 1;
    }
    verify_segment(
        &skewed.header,
        &skewed.descriptors,
        &skewed.chunks,
        skewed.audio.as_ref(),
    )
    .expect("skew within tolerance");
}
#[test]
fn verify_segment_rejects_audio_skew_beyond_tolerance() {
    let dims = FrameDimensions::new(4, 4);
    let audio_cfg = AudioEncoderConfig::default();
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: 2,
        frame_duration_ns: 5_000_000,
        audio: Some(audio_cfg),
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config);
    let frames = vec![
        RawFrame::new(dims, vec![0x33; dims.pixel_count()]).expect("frame"),
        RawFrame::new(dims, vec![0x44; dims.pixel_count()]).expect("frame"),
    ];
    let channels = audio_cfg.channel_count();
    let samples_per_frame = audio_cfg.frame_samples as usize * channels;
    let pcm = vec![0i16; samples_per_frame * frames.len()];
    let segment = encoder
        .encode_segment(43, 5_000, 9, &frames, Some(&pcm))
        .expect("encode segment");
    let mut skewed = segment.clone();
    if let Some(audio) = skewed.audio.as_mut() {
        audio.frames[1].timestamp_ns += AUDIO_SYNC_TOLERANCE_NS + 1;
    }
    let err = verify_segment(
        &skewed.header,
        &skewed.descriptors,
        &skewed.chunks,
        skewed.audio.as_ref(),
    )
    .expect_err("skew beyond tolerance must fail");
    assert!(matches!(
        err,
        SegmentError::AudioTimestampMismatch(info) if info.index == 1
    ));
}
#[test]
fn frame_packing_requires_a_size_win_and_preserves_the_header() {
    for len in [MIN_FRAME_COMPRESSION_BYTES - 1, MIN_FRAME_COMPRESSION_BYTES] {
        let mut chunk = vec![0; FRAME_HEADER_LEN + len];
        chunk[12] = FrameType::Predicted.as_byte();
        let original = chunk.clone();
        compress_frame_body(&mut chunk).unwrap();
        if len < MIN_FRAME_COMPRESSION_BYTES {
            assert_eq!(chunk, original);
        } else {
            assert!(chunk.len() < original.len());
            assert_eq!(chunk[12], FRAME_ZSTD | 1);
            let mut header = chunk[..FRAME_HEADER_LEN].to_vec();
            header[12] &= !FRAME_ZSTD;
            assert_eq!(header, original[..FRAME_HEADER_LEN]);
            let records = decompress_frame_body(&chunk[FRAME_HEADER_LEN..], 4, 0).unwrap();
            assert_eq!(records, original[FRAME_HEADER_LEN..]);
        }
    }

    let mut literal = vec![0; FRAME_HEADER_LEN];
    // A sequence of distinct literals is cheaper to transmit directly at this length.
    literal.extend(0..=255);
    let original = literal.clone();
    compress_frame_body(&mut literal).unwrap();
    assert_eq!(literal, original);
}

#[test]
fn compressed_payload_requires_one_sized_frame_within_the_block_budget() {
    let records = [0; 256];
    let compressed = zstd::bulk::compress(&records, 1).unwrap();
    assert!(matches!(
        decompress_frame_body(&compressed, 1, 0),
        Err(CodecError::InvalidCompressedFrame)
    ));
    assert!(matches!(
        decompress_frame_body(&compressed, 64, 0),
        Err(CodecError::InvalidCompressedFrame)
    ));
    let unsized_frame = zstd::stream::encode_all(records.as_slice(), 1).unwrap();
    let concatenated = [compressed.as_slice(), compressed.as_slice()].concat();
    let truncated = &compressed[..compressed.len() - 1];
    for body in [unsized_frame.as_slice(), &concatenated, truncated, &[]] {
        assert!(matches!(
            decompress_frame_body(body, 4, 0),
            Err(CodecError::InvalidCompressedFrame)
        ));
    }
    assert!(matches!(
        FrameType::from_byte(FRAME_ZSTD | 2),
        Err(CodecError::UnknownFrameType(0x82))
    ));
}

#[test]
fn sparse_dct_paths_match_the_full_kernels() {
    let mut residual = [0; BLOCK_PIXELS];
    assert_eq!(forward_dct(&residual), forward_dct_full(&residual));
    residual[BLOCK_PIXELS - 1] = -255;
    assert_eq!(forward_dct(&residual), forward_dct_full(&residual));

    let mut coeffs = [0; BLOCK_PIXELS];
    for dc in (-36..=36).chain([
        i32::MIN,
        -262_145,
        -262_140,
        -255,
        255,
        262_140,
        262_145,
        i32::MAX,
    ]) {
        coeffs[0] = dc;
        assert_eq!(inverse_dct(&coeffs), inverse_dct_full(&coeffs), "DC {dc}");
    }
    coeffs[0] = 23;
    coeffs[BLOCK_PIXELS - 1] = -97;
    assert_eq!(inverse_dct(&coeffs), inverse_dct_full(&coeffs));
}

#[test]
fn sparse_inverse_matches_direct_spatial_sums() {
    let check = |coeffs: &[i32; BLOCK_PIXELS]| {
        let spatial = inverse_dct(coeffs);
        for (index, &sample) in spatial.iter().enumerate() {
            let (x, y) = (index / BLOCK_SIZE, index % BLOCK_SIZE);
            #[cfg(feature = "streaming-fixed-point-dct")]
            let direct = {
                let mut sum = 0_i64;
                for (slot, &c) in coeffs.iter().enumerate() {
                    let u = i64::from(DCT_FACTORS_Q15[slot / BLOCK_SIZE][x]);
                    let v = i64::from(DCT_FACTORS_Q15[slot % BLOCK_SIZE][y]);
                    sum = sum.saturating_add(i64::from(c) * u * v);
                }
                round_q30(sum).clamp(-32768, 32767) as i32
            };
            #[cfg(not(feature = "streaming-fixed-point-dct"))]
            let direct = {
                let mut sum = 0.0_f64;
                for (slot, &c) in coeffs.iter().enumerate() {
                    sum += f64::from(c)
                        * DCT_FACTORS[slot / BLOCK_SIZE][x]
                        * DCT_FACTORS[slot % BLOCK_SIZE][y];
                }
                sum.round().clamp(-32768.0, 32767.0) as i32
            };
            assert_eq!(sample, direct, "pixel ({x}, {y})");
        }
    };
    for slot in 0..BLOCK_PIXELS {
        for value in [i32::MIN, -13, -4, 4, 13, i32::MAX] {
            let mut coeffs = [0; BLOCK_PIXELS];
            coeffs[slot] = value;
            check(&coeffs);
        }
    }
    for stride in [1, 2, 5, 17] {
        let coeffs = std::array::from_fn(|i| {
            if i % stride == 0 {
                (i as i32 * 971) % 4093 - 2046
            } else {
                0
            }
        });
        check(&coeffs);
    }
}

#[test]
fn certified_blocks_match_full_transform_payload_and_reference() {
    let dims = FrameDimensions::new(32, 8);
    let frame: Vec<u8> = (0..dims.pixel_count())
        .map(|i| {
            let x = i % 32;
            [24, 193, 8, 137][x / 8] + (x % 3) as u8
        })
        .collect();
    let previous = vec![100; dims.pixel_count()];
    for (kind, reference) in [
        (FrameType::Intra, None),
        (FrameType::Predicted, Some(previous.as_slice())),
    ] {
        let mut dc = 0;
        let mut expected = Vec::new();
        let mut pixels = vec![0; dims.pixel_count()];
        for i in 0..dims.block_count() {
            let (residual, predictor) = build_residual_block(&frame, reference, dims, i, kind);
            assert!(certified_dc(&residual, 16).is_some());
            let levels = quantize_coeffs(&forward_dct_full(&residual), 16);
            encode_block_rle(
                &levels,
                &mut dc,
                &mut expected,
                &mut NoopBundledHooks,
                i,
                kind,
            );
            let spatial = inverse_dct_full(&dequantize_coeffs(&levels, 16));
            write_reconstructed_block(&mut pixels, &spatial, &predictor, dims, i);
        }
        let encoded = BaselineEncoder::encode_frame_payload(
            &frame,
            kind,
            reference,
            dims,
            16,
            true,
            &mut NoopBundledHooks,
            None,
        );
        assert_eq!(encoded, (expected, Some(pixels)));
    }
}

#[test]
fn terminal_planes_emit_identical_payloads_without_a_prediction_reference() {
    let dims = FrameDimensions::new(16, 8);
    let plane: Vec<_> = (0..dims.pixel_count()).map(|i| (i * 37) as u8).collect();
    let previous: Vec<_> = plane.iter().map(|v| v / 3).collect();
    for (kind, reference) in [
        (FrameType::Intra, None),
        (FrameType::Predicted, Some(previous.as_slice())),
    ] {
        let encode = |retain| {
            BaselineEncoder::encode_frame_payload(
                &plane,
                kind,
                reference,
                dims,
                16,
                retain,
                &mut NoopBundledHooks,
                None,
            )
        };
        let (with_reference, pixels) = encode(true);
        let (terminal, discarded) = encode(false);
        assert_eq!(terminal, with_reference);
        assert_eq!(pixels.unwrap().len(), dims.pixel_count());
        assert!(discarded.is_none());
    }
}

#[test]
fn centered_energy_bound_preserves_quantized_levels() {
    let check = |block: &[i16; BLOCK_PIXELS], qp| {
        let full = forward_dct_full(block);
        if let Some(dc) = certified_dc(block, qp) {
            assert_eq!(dc, full[0]);
            let levels = quantize_coeffs(&full, qp);
            assert_eq!(quantize_level(dc, quantization_step(0, qp)), levels[0]);
            assert!(levels[1..].iter().all(|&v| v == 0));
        }
    };
    // At Q16, alternating +/-9 lies just below the threshold and +/-10 above it.
    for (swing, skipped) in [(9, true), (10, false)] {
        let block = std::array::from_fn(|i| if i % 2 == 0 { 120 + swing } else { 120 - swing });
        let full = forward_dct_full(&block);
        assert!(full[1..].iter().any(|&v| v != 0));
        assert_eq!(certified_dc(&block, 16).is_some(), skipped);
        check(&block, 16);
    }
    // Exercise DC midpoints of both signs, extremes, and both sides of the bound.
    let mut tie = [0; BLOCK_PIXELS];
    tie[10] = 14;
    tie[50] = -10;
    let mut state = 0x9e37_79b9u32;
    for qp in [0, 1, 2, 7, 16, 63, 127, 255] {
        for block in [
            tie,
            tie.map(|v| -v),
            [i16::MIN; BLOCK_PIXELS],
            [i16::MAX; BLOCK_PIXELS],
        ] {
            check(&block, qp);
        }
        for span in [1_i32, 4, 9, 32, 255, 32767] {
            for offset in [-64_i32, 0, 192] {
                let block = std::array::from_fn(|_| {
                    state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                    let x = (state >> 16) as i32 % (2 * span + 1) - span + offset;
                    x.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
                });
                check(&block, qp);
            }
        }
    }
}

#[test]
fn dct_tables_satisfy_centered_projection_bound() {
    #[cfg(feature = "streaming-fixed-point-dct")]
    let basis = DCT_FACTORS_Q15.map(|r| r.map(|v| f64::from(v) / 32768.0));
    #[cfg(not(feature = "streaming-fixed-point-dct"))]
    let basis = DCT_FACTORS;
    for (i, row) in basis.iter().enumerate() {
        assert!(row.iter().map(|x| x * x).sum::<f64>() <= 65.0 / 64.0);
        if i > 0 {
            assert!(row.iter().sum::<f64>().abs() < 1e-12);
        } else {
            assert!(row.iter().all(|&v| v == row[0]));
        }
    }
}

#[test]
fn separable_forward_dct_matches_direct_evaluation() {
    fn check(block: &[i16; BLOCK_PIXELS]) {
        let transformed = forward_dct_full(block);
        for u in 0..BLOCK_SIZE {
            for v in 0..BLOCK_SIZE {
                #[cfg(feature = "streaming-fixed-point-dct")]
                let direct = {
                    let mut sum = 0_i64;
                    for x in 0..BLOCK_SIZE {
                        for y in 0..BLOCK_SIZE {
                            sum += i64::from(block[x * BLOCK_SIZE + y])
                                * i64::from(DCT_FACTORS_Q15[u][x])
                                * i64::from(DCT_FACTORS_Q15[v][y]);
                        }
                    }
                    round_q30(sum) as i32
                };
                #[cfg(not(feature = "streaming-fixed-point-dct"))]
                let direct = direct_dct_coefficient(block, &DCT_FACTORS[u], &DCT_FACTORS[v]);
                assert_eq!(
                    transformed[u * BLOCK_SIZE + v],
                    direct,
                    "coefficient ({u}, {v})"
                );
            }
        }
    }

    // At DC these two samples sum to exactly +0.5 in direct evaluation.
    // Rounding the unguarded two-pass result would choose zero.
    let mut tie = [0; BLOCK_PIXELS];
    tie[10] = 14;
    tie[50] = -10;
    check(&tie);
    check(&tie.map(|v| -v));
    #[cfg(not(feature = "streaming-fixed-point-dct"))]
    assert_eq!(
        (
            forward_dct_full(&tie)[0],
            forward_dct_full(&tie.map(|v| -v))[0]
        ),
        (1, -1)
    );

    for value in [i16::MIN, -255, 0, 255, i16::MAX] {
        check(&[value; BLOCK_PIXELS]);
    }
    let mut state = 0x32d0_37b5u32;
    for i in 0..256 {
        let block = std::array::from_fn(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let value = (state >> 16) as i16;
            if i % 2 == 0 { value % 256 } else { value }
        });
        check(&block);
    }
}

#[test]
fn forward_dct_quantization_matches_golden() {
    let dct = forward_dct(&JPEG_SAMPLE_BLOCK);
    assert_eq!(dct, JPEG_SAMPLE_DCT_Q4);
    let quant = quantize_coeffs(&dct, 4);
    assert_eq!(quant, JPEG_SAMPLE_QUANT_Q4);
    let dequant = dequantize_coeffs(&quant, 4);
    assert_eq!(dequant, JPEG_SAMPLE_DEQUANT_Q4);
    let recon = inverse_dct(&dequant);
    assert_eq!(recon, JPEG_SAMPLE_RECON_Q4);
}
#[test]
fn baseline_encoder_entropy_matches_golden_chunk() {
    let dims = FrameDimensions::new(8, 8);
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frame_duration_ns: 16_666_666,
        duration_ns: 16_666_666,
        quantizer: 4,
        frames_per_segment: 1,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config);
    let frame_bytes: Vec<u8> = (0..dims.pixel_count())
        .map(|idx| (idx as u8).wrapping_mul(3).wrapping_add(17))
        .collect();
    let frame = RawFrame::new(dims, frame_bytes).expect("frame");
    let segment = encoder
        .encode_segment(42, 1_000, 7, &[frame], None)
        .expect("encode");
    assert_eq!(segment.chunks.len(), 1);
    assert_eq!(segment.chunks[0].as_slice(), GOLDEN_Q4_CHUNK);
}
#[test]
fn decode_block_rle_rejects_overflow_run() {
    let dims = FrameDimensions::new(8, 8);
    let config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frame_duration_ns: 16_666_666,
        duration_ns: 16_666_666,
        quantizer: 4,
        frames_per_segment: 1,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(config);
    let frame_bytes: Vec<u8> = (0..dims.pixel_count())
        .map(|idx| (idx as u8).wrapping_mul(3).wrapping_add(17))
        .collect();
    let frame = RawFrame::new(dims, frame_bytes).expect("frame");
    let segment = encoder
        .encode_segment(7, 10_000, 3, &[frame], None)
        .expect("encode");
    let mut corrupted = segment.chunks[0].clone();
    let run_offset = FRAME_HEADER_LEN + 2;
    corrupted[run_offset] = 200;
    let mut offset = FRAME_HEADER_LEN;
    let mut prev_dc = 0i16;
    let err = decode_block_rle(&corrupted, &mut offset, &mut prev_dc, 0).expect_err("overflow");
    assert!(matches!(err, CodecError::RleOverflow(0)));
}
#[test]
fn decode_block_rle_rejects_missing_end_of_block() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0i16.to_le_bytes());
    for _ in 0..(BLOCK_PIXELS - 1) {
        payload.push(0);
        payload.extend_from_slice(&0i16.to_le_bytes());
    }
    let mut offset = 0usize;
    let mut prev_dc = 0i16;
    let err = decode_block_rle(&payload, &mut offset, &mut prev_dc, 0)
        .expect_err("missing end-of-block should reject");
    assert!(matches!(err, CodecError::MissingEndOfBlock(0)));
}
#[test]
fn decode_block_rle_rejects_truncated_stream() {
    let mut offset = 0usize;
    let mut prev_dc = 0i16;
    let err =
        decode_block_rle(&[0xAA], &mut offset, &mut prev_dc, 2).expect_err("too short for dc diff");
    assert!(matches!(err, CodecError::TruncatedBlock(2)));
    let mut payload = Vec::new();
    payload.extend_from_slice(&0i16.to_le_bytes());
    payload.push(0x01);
    // missing coefficient value bytes
    let mut offset2 = 0usize;
    let mut prev_dc2 = 0i16;
    let err = decode_block_rle(&payload, &mut offset2, &mut prev_dc2, 4)
        .expect_err("missing run payload");
    assert!(matches!(err, CodecError::TruncatedBlock(4)));
    let mut overflow_offset = usize::MAX;
    let mut prev_dc3 = 0i16;
    let err = decode_block_rle(&[], &mut overflow_offset, &mut prev_dc3, 6)
        .expect_err("offset arithmetic overflow should fail closed");
    assert!(matches!(err, CodecError::TruncatedBlock(6)));
    assert_eq!(overflow_offset, usize::MAX);
}
#[test]
fn rle_block_readers_reject_offset_overflow_without_advancing() {
    let mut i16_offset = usize::MAX;
    let err = take_block_i16_le(&[], &mut i16_offset, 7)
        .expect_err("i16 reader offset overflow should fail closed");
    assert!(matches!(err, CodecError::TruncatedBlock(7)));
    assert_eq!(i16_offset, usize::MAX);
    let mut record_offset = usize::MAX;
    let err = take_rle_record(&[], &mut record_offset, 8)
        .expect_err("RLE record reader offset overflow should fail closed");
    assert!(matches!(err, CodecError::TruncatedBlock(8)));
    assert_eq!(record_offset, usize::MAX);
}
#[test]
fn frame_header_readers_reject_truncated_or_overflowed_offsets() {
    let mut header = [0u8; FRAME_HEADER_LEN];
    header[..4].copy_from_slice(&42u32.to_le_bytes());
    header[4..12].copy_from_slice(&1234u64.to_le_bytes());
    header[14..16].copy_from_slice(&9u16.to_le_bytes());
    assert_eq!(read_frame_header_u32_le(&header, 0).unwrap(), 42);
    assert_eq!(read_frame_header_u64_le(&header, 4).unwrap(), 1234);
    assert_eq!(read_frame_header_u16_le(&header, 14).unwrap(), 9);
    assert!(matches!(
        read_frame_header_u32_le(&header[..3], 0),
        Err(CodecError::ChunkTooShort)
    ));
    assert!(matches!(
        read_frame_header_u64_le(&header[..11], 4),
        Err(CodecError::ChunkTooShort)
    ));
    assert!(matches!(
        read_frame_header_u16_le(&header, usize::MAX),
        Err(CodecError::ChunkTooShort)
    ));
}
#[test]
fn chroma_len_reader_rejects_truncated_or_overflowed_offsets() {
    let mut metadata = [0u8; 8];
    metadata[..4].copy_from_slice(&5u32.to_le_bytes());
    metadata[4..8].copy_from_slice(&7u32.to_le_bytes());
    assert_eq!(read_chroma_len(&metadata, 0).unwrap(), 5);
    assert_eq!(read_chroma_len(&metadata, 4).unwrap(), 7);
    assert!(matches!(
        read_chroma_len(&metadata[..3], 0),
        Err(CodecError::ChromaPayloadTruncated(_))
    ));
    assert!(matches!(
        read_chroma_len(&metadata, usize::MAX),
        Err(CodecError::ChromaPayloadTruncated(_))
    ));
}
fn deterministic_payloads(seed: u8, count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|chunk_idx| {
            let len = 1 + ((seed as usize + chunk_idx * 7) % 47);
            (0..len)
                .map(|idx| {
                    seed.wrapping_add(chunk_idx as u8)
                        .wrapping_mul(31)
                        .wrapping_add(idx as u8)
                })
                .collect()
        })
        .collect()
}
#[test]
fn merkle_proof_roundtrip_holds() {
    let cases = [
        (0_u64, 0_u64, deterministic_payloads(0, 1)),
        (1, 7, deterministic_payloads(3, 2)),
        (u64::MAX, 42, deterministic_payloads(11, 5)),
    ];
    for (segment_number, content_key_id, payloads) in cases {
        let chunk_ids: Vec<u16> = (0..payloads.len()).map(|idx| idx as u16).collect();
        let payload_refs: Vec<(u16, &[u8])> = chunk_ids
            .iter()
            .zip(payloads.iter())
            .map(|(id, bytes)| (*id, bytes.as_slice()))
            .collect();
        let commitments = chunk_commitments(segment_number, &payload_refs);
        let root = merkle_root(&commitments).expect("non-empty leaves produce root");
        for (idx, chunk_id) in chunk_ids.iter().enumerate() {
            let proof =
                crate::streaming::chunk::merkle_proof(&commitments, idx, *chunk_id).expect("proof");
            let leaf = commitments[idx];
            assert!(crate::streaming::chunk::verify_merkle_proof(
                &leaf, &proof, &root
            ));
        }
        let storage_hash = crate::streaming::chunk::storage_commitment(
            segment_number,
            content_key_id,
            &root,
            &chunk_ids,
        )
        .expect("storage commitment");
        let da_hash = crate::streaming::chunk::data_availability_root(
            segment_number,
            content_key_id,
            &root,
            &chunk_ids,
        )
        .expect("da root");
        assert_ne!(storage_hash, da_hash);
        if chunk_ids.len() >= 2 {
            let mut unsorted = chunk_ids.clone();
            unsorted.swap(0, 1);
            assert!(matches!(
                crate::streaming::chunk::storage_commitment(
                    segment_number,
                    content_key_id,
                    &root,
                    &unsorted
                ),
                Err(ChunkError::UnsortedChunkIds)
            ));
            assert!(matches!(
                crate::streaming::chunk::data_availability_root(
                    segment_number,
                    content_key_id,
                    &root,
                    &unsorted
                ),
                Err(ChunkError::UnsortedChunkIds)
            ));
        }
    }
}
