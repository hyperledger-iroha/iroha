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
    let telemetry = recorder.finish();
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
    let telemetry = recorder.finish();
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
    let telemetry = recorder.finish();
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
    let telemetry = recorder.finish();
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
    let telemetry = recorder.finish();
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
fn rdo_perceptual_uses_softer_lambda() {
    let default = rdo_lambda_for_mode(RdoMode::DynamicProgramming, 20);
    let perceptual = rdo_lambda_for_mode(RdoMode::Perceptual, 20);
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
