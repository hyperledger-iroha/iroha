use super::*;
use crate::vega::sponge::shake256;
const POST_FRI0_TRANSCRIPT_KAT_V2: [u8; 32] = [
    0xc1, 0x31, 0x7e, 0x0f, 0x12, 0xb3, 0xe5, 0xba, 0x8d, 0xd8, 0x9c, 0xcd, 0x20, 0xb0, 0xf5, 0x92,
    0x8f, 0x2b, 0xd7, 0xdd, 0x05, 0x3c, 0x0a, 0x7a, 0x32, 0x77, 0xfd, 0x8d, 0x6d, 0x14, 0xa6, 0xd3,
];
const BATCH_SCHEDULE_KAT_V2: [u8; 32] = [
    0xb3, 0xdf, 0xab, 0x02, 0xbf, 0x2c, 0xd4, 0xcf, 0x5b, 0xb2, 0x3c, 0x77, 0x3c, 0x66, 0x16, 0xba,
    0xa8, 0x67, 0x28, 0xaa, 0x73, 0x00, 0x36, 0x7a, 0x0c, 0x8a, 0x99, 0xd0, 0x98, 0xe6, 0xb1, 0x90,
];
const FOLD0_SCHEDULE_KAT_V2: [u8; 32] = [
    0x8e, 0x4a, 0x29, 0x52, 0x37, 0x22, 0x08, 0xf8, 0x5c, 0x7c, 0x56, 0x5d, 0xcf, 0x20, 0x6c, 0xd3,
    0x3c, 0xbb, 0xd1, 0x04, 0x17, 0x62, 0xd8, 0x69, 0x7a, 0x6e, 0x26, 0xf2, 0xe6, 0xb7, 0x3c, 0x3c,
];
const FOLD_SCHEDULE_KAT_V2: [u8; 32] = [
    0x53, 0xb1, 0x0f, 0x65, 0xbe, 0xc2, 0x68, 0x61, 0x0a, 0x1b, 0xcf, 0x03, 0xde, 0xd9, 0x9d, 0x48,
    0xcc, 0xd1, 0x5f, 0xd4, 0xe2, 0x3b, 0xcc, 0xcd, 0xc1, 0x15, 0xa2, 0xf3, 0x1d, 0x72, 0x8c, 0xc6,
];
const QUERY_ARRAY_KAT_V2: [u8; 32] = [
    0x67, 0x55, 0x6e, 0x20, 0x05, 0xa1, 0x42, 0xda, 0x7c, 0x40, 0xa6, 0x84, 0x9c, 0x29, 0xef, 0xfb,
    0xc7, 0x98, 0x45, 0x41, 0x38, 0xee, 0x91, 0x3d, 0xbe, 0xfa, 0x0f, 0xfa, 0x79, 0xab, 0xb5, 0x2f,
];
fn manual_absorb_root_v2(transcript: [u8; 32], layer: u8, root: [u8; 32]) -> [u8; 32] {
    let mut frame = b"iroha.zk-ams.v2.q-pcs.soundness.fri-root\0".to_vec();
    frame.push(2);
    frame.extend_from_slice(&transcript);
    frame.push(layer);
    frame.extend_from_slice(&root);
    keccak256(&frame)
}
fn manual_challenge_v2(transcript: [u8; 32], limb: usize, row: usize, layer: usize) -> [u64; 2] {
    let modulus = RELEASE_MODULI_V1[limb];
    let zone = u64::MAX - u64::MAX % modulus;
    for attempt in 0_u32..256 {
        let mut frame = b"iroha.zk-ams.v2.q-pcs.soundness.ten-row-fold\0".to_vec();
        frame.push(2);
        frame.extend_from_slice(&transcript);
        frame.extend_from_slice(&[limb as u8, row as u8, 0, layer as u8]);
        frame.extend_from_slice(&modulus.to_be_bytes());
        frame.extend_from_slice(&attempt.to_be_bytes());
        let bytes: [u8; 16] = shake256(&frame, 16).try_into().expect("sixteen bytes");
        let c0 = u64::from_be_bytes(bytes[..8].try_into().expect("first component"));
        let c1 = u64::from_be_bytes(bytes[8..].try_into().expect("second component"));
        if c0 < zone && c1 < zone {
            let value = [c0 % modulus, c1 % modulus];
            if value != [0, 0] {
                return value;
            }
        }
    }
    panic!("manual fold challenge rejection bound exhausted")
}
fn manual_absorb_schedule_v2(
    digest: [u8; 32],
    limb: usize,
    row: usize,
    layer: usize,
    value: [u64; 2],
) -> [u8; 32] {
    let mut frame = b"iroha.zk-ams.v2.q-pcs.soundness.schedule\0".to_vec();
    frame.extend_from_slice(&[2, 1]);
    frame.extend_from_slice(&digest);
    frame.extend_from_slice(&[limb as u8, row as u8, 0, layer as u8]);
    frame.extend_from_slice(&value[0].to_be_bytes());
    frame.extend_from_slice(&value[1].to_be_bytes());
    keccak256(&frame)
}
fn manual_rounds_through_v2(last_layer: usize) -> ([u8; 32], [u8; 32]) {
    let mut transcript = POST_FRI0_TRANSCRIPT_KAT_V2;
    let mut schedule = FOLD0_SCHEDULE_KAT_V2;
    for layer in 1..=last_layer {
        transcript = manual_absorb_root_v2(transcript, layer as u8, [0x60 + layer as u8; 32]);
        for limb in 0..38 {
            for row in 0..10 {
                let value = manual_challenge_v2(transcript, limb, row, layer);
                schedule = manual_absorb_schedule_v2(schedule, limb, row, layer, value);
            }
        }
    }
    (transcript, schedule)
}
fn manual_rounds_v2() -> ([u8; 32], [u8; 32]) {
    manual_rounds_through_v2(17)
}
fn manual_queries_v2(transcript: [u8; 32]) -> [u32; QUERY_COUNT_V2] {
    let mut terminal_frame = b"iroha.zk-ams.v2.q-pcs.soundness.fri-terminal\0".to_vec();
    terminal_frame.push(2);
    terminal_frame.extend_from_slice(&transcript);
    terminal_frame.extend_from_slice(&380_u16.to_be_bytes());
    terminal_frame.resize(terminal_frame.len() + 12_160, 0);
    let transcript = keccak256(&terminal_frame);
    let bound = 262_144_u64;
    let zone = u64::MAX - u64::MAX % bound;
    let mut queries = [0_u32; QUERY_COUNT_V2];
    for ordinal in 0..QUERY_COUNT_V2 {
        for attempt in 0_u32..256 {
            let mut frame = b"iroha.zk-ams.v2.q-pcs.soundness.query\0".to_vec();
            frame.push(2);
            frame.extend_from_slice(&transcript);
            frame.extend_from_slice(&(ordinal as u16).to_be_bytes());
            frame.extend_from_slice(&attempt.to_be_bytes());
            let bytes: [u8; 8] = shake256(&frame, 8).try_into().expect("eight bytes");
            let candidate = u64::from_be_bytes(bytes);
            if candidate < zone {
                let query = (candidate % bound) as u32;
                if !queries[..ordinal].contains(&query) {
                    queries[ordinal] = query;
                    break;
                }
            }
        }
    }
    queries
}
fn query_digest_v2(queries: &[u32; QUERY_COUNT_V2]) -> [u8; 32] {
    let mut frame = Vec::with_capacity(QUERY_COUNT_V2 * 4);
    for query in queries {
        frame.extend_from_slice(&query.to_be_bytes());
    }
    keccak256(&frame)
}
#[test]
fn continuation_transcript_matches_literal_independent_oracle() {
    let (manual_transcript, manual_schedule) = manual_rounds_v2();
    assert_eq!(manual_schedule, FOLD_SCHEDULE_KAT_V2);
    let mut live = ProverFriRoundsLiveV2 {
        transcript: POST_FRI0_TRANSCRIPT_KAT_V2,
        batch_schedule_digest: BATCH_SCHEDULE_KAT_V2,
        fold_schedule_digest: FOLD0_SCHEDULE_KAT_V2,
        next_layer: 1,
    };
    let mut last_context = None;
    for layer in 1..=17_u8 {
        let round = bind_round_live_v2(live, [0x60 + layer; 32]).unwrap();
        assert_eq!(round.context.layer, layer);
        assert_eq!(round.alphas.len(), 380);
        last_context = Some(round.context);
        live = round.continuation;
    }
    assert_eq!(live.transcript, manual_transcript);
    assert_eq!(live.fold_schedule_digest, manual_schedule);
    let complete = ProverFriRoundCompleteV2 {
        live: Some(live),
        context: last_context.unwrap(),
    };
    let queries = complete
        .bind_terminal_v2(&[0_u8; 12_160])
        .unwrap()
        .derive_queries_v2()
        .unwrap();
    let manual_queries = manual_queries_v2(manual_transcript);
    assert_eq!(queries.queries_v2(), &manual_queries);
    assert_eq!(query_digest_v2(&manual_queries), QUERY_ARRAY_KAT_V2);
    assert_eq!(queries.context_v2().2, FOLD_SCHEDULE_KAT_V2);
    let plan = queries.into_canonical_proof_plan_v2().unwrap();
    assert_eq!(plan.query_digest_v2(), QUERY_ARRAY_KAT_V2);
    assert_eq!(plan.exact_wire_bytes_v2(), 27_196_704);
    assert_eq!(
        plan.section_shape_digest_v2(),
        [
            0x03, 0xb8, 0x27, 0x20, 0x89, 0x43, 0xc7, 0x25, 0xf2, 0x02, 0x34, 0x24, 0x09, 0x0c,
            0x5a, 0x1a, 0x9a, 0x1a, 0xd1, 0x75, 0x20, 0x76, 0x43, 0x87, 0xd2, 0x90, 0xf9, 0xc4,
            0x91, 0xa1, 0xe1, 0x5d,
        ]
    );
    let expected_shapes: [(u32, u32); 20] = [
        (320, 3_096),
        (320, 3_096),
        (320, 3_096),
        (320, 2_824),
        (318, 2_484),
        (318, 2_162),
        (316, 1_850),
        (314, 1_532),
        (312, 1_246),
        (298, 934),
        (286, 664),
        (260, 390),
        (230, 194),
        (172, 64),
        (118, 10),
        (64, 0),
        (32, 0),
        (16, 0),
        (8, 0),
        (4, 0),
    ];
    for (ordinal, expected) in expected_shapes.into_iter().enumerate() {
        let section = plan.section_v2(ordinal).unwrap();
        assert_eq!((section.opened_v2(), section.authentication_v2()), expected);
    }
}
#[test]
fn last_round_is_exactly_one_two_by_two_fold_and_rejects_unequal_terminal() {
    let (transcript, schedule) = manual_rounds_v2();
    let (transcript16, schedule16) = manual_rounds_through_v2(16);
    let ready = ProverFriRoundsReadyV2 {
        live: Some(ProverFriRoundsLiveV2 {
            transcript: transcript16,
            batch_schedule_digest: BATCH_SCHEDULE_KAT_V2,
            fold_schedule_digest: schedule16,
            next_layer: 17,
        }),
    };
    assert_eq!(round_shape_v2(17).unwrap(), (1, 2));
    assert_eq!(round_shape_v2(1).unwrap(), (128, 1_024));
    let mut challenges = ready.bind_next_root_v2([0x71; 32]).unwrap();
    let mut reference = ProverFriRoundsReadyV2 {
        live: Some(ProverFriRoundsLiveV2 {
            transcript: transcript16,
            batch_schedule_digest: BATCH_SCHEDULE_KAT_V2,
            fold_schedule_digest: schedule16,
            next_layer: 17,
        }),
    }
    .bind_next_root_v2([0x71; 32])
    .unwrap();
    let mut values = [0_u8; 64];
    for (lane, value) in values.chunks_exact_mut(16).enumerate() {
        value[..8].copy_from_slice(&(lane as u64 + 1).to_be_bytes());
        value[8..].copy_from_slice(&(lane as u64 + 5).to_be_bytes());
    }
    let mut expected = [0_u8; 32];
    reference
        .fold_next_pair_v2(0, 0, &values[..32], &values[32..], &mut expected)
        .unwrap();
    challenges
        .fold_terminal_column_in_place_v2(0, &mut values)
        .unwrap();
    assert_eq!(&values[..32], &expected);
    for column in 1..380_u16 {
        let mut values = [0_u8; 64];
        challenges
            .fold_terminal_column_in_place_v2(column, &mut values)
            .unwrap();
        assert_eq!(&values[..32], &[0; 32]);
    }
    let complete_zero = challenges.complete_v2().unwrap();
    assert_eq!(complete_zero.context_v2().fold_schedule_digest, schedule);
    assert!(complete_zero.bind_terminal_v2(&[0; 12_160]).is_ok());
    let mut unequal = [0_u8; 12_160];
    unequal[6_080] = 1;
    let complete = ProverFriRoundCompleteV2 {
        live: Some(ProverFriRoundsLiveV2 {
            transcript,
            batch_schedule_digest: BATCH_SCHEDULE_KAT_V2,
            fold_schedule_digest: schedule,
            next_layer: 18,
        }),
        context: ProverFriRoundContextV2 {
            layer: 17,
            pre_root_transcript: [1; 32],
            post_root_transcript: transcript,
            batch_schedule_digest: BATCH_SCHEDULE_KAT_V2,
            prior_fold_schedule_digest: [2; 32],
            fold_schedule_digest: schedule,
            root: [0x71; 32],
        },
    };
    assert!(matches!(
        complete.bind_terminal_v2(&unequal),
        Err(SoundnessErrorV2::InvalidTerminal)
    ));
}
#[test]
fn source_guard_pins_move_only_bounded_continuation() {
    let source = include_str!("prover_fri_rounds_v2.rs");
    assert!(source.lines().count() <= 520);
    for required in [
        "struct ProverFriRoundsReadyV2",
        "struct ProverFriRoundChallengesV2",
        "struct ProverFriRoundCompleteV2",
        "struct ProverFriTerminalBoundV2",
        "struct ProverFriQueriesV2",
        "next_layer: FIRST_CONTINUATION_LAYER_V2",
        "usize::from(layer)",
        "derive_queries_v2(self.transcript)",
    ] {
        assert!(
            source.contains(required),
            "missing continuation pin: {required}"
        );
    }
    assert!(!source.contains("pub struct"));
    assert!(!source.contains("pub enum"));
    assert!(!source.contains("Vec<"));
    assert!(!source.contains("layer 18"));
}
