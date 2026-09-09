//! Populated codec bundle values for immutable streaming wire fixtures.
use super::wire_value_fixtures::record;
use crate::streaming::{codec::*, *};

pub(super) fn codec_values(rows: &mut Vec<crate::json::Value>) {
    let dimensions = FrameDimensions::new(8, 8);
    record(rows, "codec::FrameDimensions", dimensions);
    let chroma = Chroma420Frame::new(dimensions, vec![17; 16], vec![29; 16])
        .expect("valid populated 4:2:0 chroma planes");
    record(rows, "codec::Chroma420Frame", chroma.clone());
    let chunk = vec![1, 3, 5, 7, 11, 13];
    let commitment = blake3_hash(&chunk);
    let header = SegmentHeader {
        segment_number: 71,
        profile: ProfileId::UHD_MAIN,
        entropy_mode: EntropyMode::RansBundled,
        entropy_tables_checksum: Some([19; 32]),
        encryption_suite: EncryptionSuite::Kyber768XChaCha20Poly1305([23; 32]),
        layer_bitmap: 3,
        chunk_merkle_root: commitment,
        chunk_count: 1,
        timeline_start_ns: 1_234_567_890,
        duration_ns: 33_333_333,
        feedback_hint: FeedbackHint {
            layer_hints: vec![LayerFeedback {
                layer_id: 1,
                min_target_kbps: 200,
                max_target_kbps: 400,
                storage_hint: Some(StorageClass::Permanent),
            }],
            report_interval_ms: Some(500),
            fec: Some(FecParameters {
                scheme: FecScheme::Rs12_10,
                window_step: Some(1),
                parity_symbols: Some(2),
            }),
        },
        content_key_id: 31,
        nonce_salt: [37; 32],
        storage_class: StorageClass::Permanent,
        audio_summary: None,
        bundle_acceleration: BundleAcceleration::None,
    };
    record(
        rows,
        "codec::SegmentBundle",
        SegmentBundle {
            header,
            descriptors: vec![ChunkDescriptor {
                chunk_id: 0,
                offset: 0,
                length: u32::try_from(chunk.len()).unwrap(),
                commitment,
                parity: false,
            }],
            chunks: vec![chunk],
            audio: None,
            frame_dimensions: dimensions,
            frame_duration_ns: 33_333_333,
            chroma: vec![chroma],
        },
    );
    let stats = BundledStats {
        bundle_width: 4,
        bundles_total: 43,
        blocks_encoded: 47,
        significance_zero: 53,
        significance_one: 59,
        sign_negative: 61,
        sign_positive: 67,
        parity_one: 71,
        geq2_one: 73,
        dc_events: 79,
        ac_events: 83,
        nonzero_levels: 89,
        zero_run_total: 97,
        max_zero_run: 7,
        significance_rle: 101,
        flush_type_complete: 103,
        flush_context_switch: 107,
        flush_end_of_block: 109,
    };
    record(rows, "codec::BundledStats", stats);
    let rdo = RdoTelemetry {
        mode: RdoMode::DynamicProgramming,
        lambda_bits: 0.75,
        blocks_optimized: 13,
        energy_histogram: [1, 2, 3, 5, 7],
        before_rate_bits: 1_009,
        after_rate_bits: 809,
        distortion_penalty: 17,
        neural_class_histogram: vec![19, 23, 29],
    };
    record(rows, "codec::RdoTelemetry", rdo.clone());
    let context = BundleContextId::new(113);
    record(rows, "codec::BundleContextId", context);
    let mut symbol_counts = [0; RDO_MAX_SYMBOLS];
    symbol_counts[0] = 127;
    symbol_counts[RDO_MAX_SYMBOLS - 1] = 131;
    let frequency = ContextFrequency {
        context,
        bundles: 137,
        total_bits: 139,
        symbol_counts,
    };
    record(rows, "codec::ContextFrequency", frequency);
    let remap = ContextRemapSummary {
        escape_context: BundleContextId::new(149),
        remapped: 151,
        dropped: 157,
    };
    record(rows, "codec::ContextRemapSummary", remap);
    let context_stats = BundleContextStats {
        context,
        bundles_total: 163,
        type_significance_only: 167,
        type_sign_and_magnitude: 173,
        type_sign_parity: 179,
        type_sign_parity_level: 181,
        type_significance_rle: 191,
        flush_type_complete: 193,
        flush_context_switch: 197,
        flush_end_of_block: 199,
    };
    record(rows, "codec::BundleContextStats", context_stats);
    let bundle = BundleRecord {
        bundle_type: BundleType::SignParityLevel,
        context,
        bits: 0b101,
        bit_len: 3,
        flush: BundleFlushReason::ContextSwitch,
    };
    record(rows, "codec::BundleRecord", bundle);
    let tokens = vec![
        BundledToken::DcDiff(-211),
        BundledToken::Ac {
            run: 7,
            value: -223,
        },
        BundledToken::EndOfBlock,
    ];
    for (index, value) in tokens.iter().copied().enumerate() {
        record(rows, &format!("codec::BundledToken::{index}"), value);
    }
    for (index, value) in [
        BundleType::SignificanceOnly,
        BundleType::SignAndMagnitude,
        BundleType::SignParity,
        BundleType::SignParityLevel,
        BundleType::SignificanceRle,
    ]
    .into_iter()
    .enumerate()
    {
        record(rows, &format!("codec::BundleType::{index}"), value);
    }
    for (index, value) in [
        BundleFlushReason::TypeComplete,
        BundleFlushReason::ContextSwitch,
        BundleFlushReason::EndOfBlock,
    ]
    .into_iter()
    .enumerate()
    {
        record(rows, &format!("codec::BundleFlushReason::{index}"), value);
    }
    let mut telemetry = BundledTelemetry {
        stats,
        tokens,
        bundles: vec![bundle],
        context_stats: vec![context_stats],
        context_frequencies: vec![frequency],
        ans_stream: vec![227, 229, 233, 239],
        ans_precision_bits: 8,
        tables_checksum: [241; 32],
        acceleration: BundleAcceleration::None,
        prefetch_distance: 251,
        context_remap: Some(remap),
        rdo: Some(rdo),
    };
    record(
        rows,
        "codec::BundledTelemetry::populated",
        telemetry.clone(),
    );
    telemetry.context_remap = None;
    telemetry.rdo = None;
    record(
        rows,
        "codec::BundledTelemetry::without_optional_records",
        telemetry,
    );
}
