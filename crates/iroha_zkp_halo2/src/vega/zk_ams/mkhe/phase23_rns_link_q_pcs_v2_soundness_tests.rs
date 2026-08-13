use super::super::{Fq2ParametersV1, Fq2V1};
use super::*;
use crate::vega::sponge::shake256;
use sha2::{Digest as _, Sha256};
const SOURCE_DIGEST: [u8; 32] = [0x22; 32];
const ALGEBRA_DIGEST: [u8; 32] = [0x33; 32];
const INITIAL_ROOT: [u8; 32] = [0x44; 32];
const QUOTIENT_ROOT: [u8; 32] = [0x55; 32];
const PARAMETER_KAT: [u8; 32] = [
    0xcc, 0x56, 0x91, 0x18, 0x77, 0xef, 0x83, 0xb0, 0x4c, 0x3c, 0xe8, 0x79, 0x64, 0x0f, 0x29, 0x43,
    0xce, 0xab, 0xe1, 0x3c, 0x38, 0xa7, 0x37, 0x2d, 0x5c, 0x4f, 0x69, 0x63, 0x7f, 0xe7, 0x75, 0x66,
];
const INITIAL_TRANSCRIPT_KAT: [u8; 32] = [
    0x60, 0x6d, 0xeb, 0xc3, 0x2d, 0xde, 0x94, 0x6a, 0x2a, 0x4b, 0xc6, 0x6d, 0x93, 0xb3, 0xbf, 0x68,
    0x37, 0xf9, 0x26, 0x81, 0xa6, 0xda, 0x02, 0xc5, 0x05, 0x61, 0x18, 0x8e, 0x09, 0x45, 0xd2, 0x5d,
];
const BATCH_SCHEDULE_KAT: [u8; 32] = [
    0xb3, 0xdf, 0xab, 0x02, 0xbf, 0x2c, 0xd4, 0xcf, 0x5b, 0xb2, 0x3c, 0x77, 0x3c, 0x66, 0x16, 0xba,
    0xa8, 0x67, 0x28, 0xaa, 0x73, 0x00, 0x36, 0x7a, 0x0c, 0x8a, 0x99, 0xd0, 0x98, 0xe6, 0xb1, 0x90,
];
const PRE_FRI_TRANSCRIPT_KAT: [u8; 32] = [
    0xb6, 0xe4, 0x18, 0x1d, 0xb4, 0xde, 0xd2, 0x0e, 0x26, 0x14, 0x71, 0xfe, 0xa9, 0x93, 0x48, 0xdc,
    0x06, 0x50, 0x2c, 0xfe, 0xdd, 0xeb, 0xa0, 0x0d, 0x9a, 0xe2, 0x07, 0x9b, 0x78, 0xe3, 0xba, 0xd3,
];
const POST_FRI0_TRANSCRIPT_KAT: [u8; 32] = [
    0xc1, 0x31, 0x7e, 0x0f, 0x12, 0xb3, 0xe5, 0xba, 0x8d, 0xd8, 0x9c, 0xcd, 0x20, 0xb0, 0xf5, 0x92,
    0x8f, 0x2b, 0xd7, 0xdd, 0x05, 0x3c, 0x0a, 0x7a, 0x32, 0x77, 0xfd, 0x8d, 0x6d, 0x14, 0xa6, 0xd3,
];
const FOLD0_SCHEDULE_KAT: [u8; 32] = [
    0x8e, 0x4a, 0x29, 0x52, 0x37, 0x22, 0x08, 0xf8, 0x5c, 0x7c, 0x56, 0x5d, 0xcf, 0x20, 0x6c, 0xd3,
    0x3c, 0xbb, 0xd1, 0x04, 0x17, 0x62, 0xd8, 0x69, 0x7a, 0x6e, 0x26, 0xf2, 0xe6, 0xb7, 0x3c, 0x3c,
];
const ALPHA0_FIRST_KAT: (u64, u64) = (1_072_159_532_130_022_203, 1_116_667_884_697_309_814);
const ALPHA0_LAST_KAT: (u64, u64) = (212_205_419_376_918_950, 464_094_173_245_421_321);
const FOLD0_LANE0_KAT: (u64, u64) = (262_030_072_022_937_729, 117_015_592_385_837_654);
const FOLD0_LANE1_KAT: (u64, u64) = (950_909_105_422_868_960, 1_055_806_915_739_682_745);
const FOLD_SCHEDULE_KAT: [u8; 32] = [
    0x53, 0xb1, 0x0f, 0x65, 0xbe, 0xc2, 0x68, 0x61, 0x0a, 0x1b, 0xcf, 0x03, 0xde, 0xd9, 0x9d, 0x48,
    0xcc, 0xd1, 0x5f, 0xd4, 0xe2, 0x3b, 0xcc, 0xcd, 0xc1, 0x15, 0xa2, 0xf3, 0x1d, 0x72, 0x8c, 0xc6,
];
const QUERY_ARRAY_KAT: [u8; 32] = [
    0x67, 0x55, 0x6e, 0x20, 0x05, 0xa1, 0x42, 0xda, 0x7c, 0x40, 0xa6, 0x84, 0x9c, 0x29, 0xef, 0xfb,
    0xc7, 0x98, 0x45, 0x41, 0x38, 0xee, 0x91, 0x3d, 0xbe, 0xfa, 0x0f, 0xfa, 0x79, 0xab, 0xb5, 0x2f,
];
fn context() -> ExpectedPublicContextV2 {
    ExpectedPublicContextV2 {
        sealed_source_transcript_digest: SOURCE_DIGEST,
        source_algebra_binding_digest: ALGEBRA_DIGEST,
    }
}
fn manual_parameter_digest() -> [u8; 32] {
    let mut frame = b"iroha.zk-ams.v2.q-pcs.soundness.parameters\0".to_vec();
    frame.extend_from_slice(&[2, 17, 19]);
    frame.extend_from_slice(&131_072_u32.to_be_bytes());
    frame.extend_from_slice(&524_288_u32.to_be_bytes());
    frame.extend_from_slice(&160_u16.to_be_bytes());
    frame.extend_from_slice(&[38, 5, 10, 18]);
    frame.extend_from_slice(b"P:2N/c[2N-1]=0;H:N/c[N-1]=0");
    frame.extend_from_slice(b"column=limb*10+repetition*2+role;P:0;H:1");
    frame.extend_from_slice(b"Bp=aP+bXUP;Bh=aX^NH+bX^(N+1)UH");
    for (limb, modulus) in RELEASE_MODULI_V1.iter().copied().enumerate() {
        frame.push(limb as u8);
        frame.extend_from_slice(&modulus.to_be_bytes());
    }
    keccak256(&frame)
}
fn manual_initial_transcript(root: [u8; 32]) -> [u8; 32] {
    let mut frame = b"iroha.zk-ams.v2.q-pcs.soundness.initial-root\0".to_vec();
    frame.push(2);
    frame.extend_from_slice(&manual_parameter_digest());
    frame.extend_from_slice(&SOURCE_DIGEST);
    frame.extend_from_slice(&ALGEBRA_DIGEST);
    frame.extend_from_slice(&root);
    keccak256(&frame)
}
fn manual_points(transcript: [u8; 32]) -> [u64; RELATION_COUNT_V2] {
    let mut points = [0_u64; RELATION_COUNT_V2];
    for limb in 0..LIMBS_V2 {
        let modulus = RELEASE_MODULI_V1[limb];
        let zone = u64::MAX - u64::MAX % modulus;
        for repetition in 0..REPETITIONS_V2 {
            let coordinate = limb * REPETITIONS_V2 + repetition;
            for attempt in 0_u32..256 {
                let mut frame = b"iroha.zk-ams.v2.q-pcs.soundness.relation-point\0".to_vec();
                frame.push(2);
                frame.extend_from_slice(&transcript);
                frame.extend_from_slice(&[limb as u8, repetition as u8]);
                frame.extend_from_slice(&modulus.to_be_bytes());
                frame.extend_from_slice(&attempt.to_be_bytes());
                let bytes = shake256(&frame, 8);
                let candidate = u64::from_be_bytes(bytes.try_into().expect("eight bytes"));
                if candidate < zone {
                    let point = candidate % modulus;
                    if point != 0
                        && !points[limb * REPETITIONS_V2..coordinate].contains(&point)
                        && mod_add_v1(mod_pow_v1(point, 131_072, modulus), 1, modulus) != 0
                        && mod_pow_v1(point, 524_288, modulus) != 1
                    {
                        points[coordinate] = point;
                        break;
                    }
                }
            }
            assert_ne!(points[coordinate], 0);
        }
    }
    points
}
fn manual_absorb_evaluations(transcript: [u8; 32], encoded: &[u8]) -> [u8; 32] {
    let mut frame = b"iroha.zk-ams.v2.q-pcs.soundness.evaluations\0".to_vec();
    frame.push(2);
    frame.extend_from_slice(&transcript);
    frame.extend_from_slice(&190_u16.to_be_bytes());
    frame.extend_from_slice(encoded);
    keccak256(&frame)
}
fn manual_absorb_root(
    domain: &[u8],
    transcript: [u8; 32],
    ordinal: u8,
    root: [u8; 32],
) -> [u8; 32] {
    let mut frame = domain.to_vec();
    frame.push(2);
    frame.extend_from_slice(&transcript);
    frame.push(ordinal);
    frame.extend_from_slice(&root);
    keccak256(&frame)
}
fn manual_fq2_challenge(
    domain: &[u8],
    transcript: [u8; 32],
    limb: usize,
    row: usize,
    component: usize,
    layer: usize,
) -> [u64; 2] {
    let modulus = RELEASE_MODULI_V1[limb];
    let zone = u64::MAX - u64::MAX % modulus;
    for attempt in 0_u32..256 {
        let mut frame = domain.to_vec();
        frame.push(2);
        frame.extend_from_slice(&transcript);
        frame.extend_from_slice(&[limb as u8, row as u8, component as u8, layer as u8]);
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
    panic!("manual challenge rejection bound exhausted")
}
fn manual_absorb_schedule_value(
    digest: [u8; 32],
    kind: u8,
    limb: usize,
    row: usize,
    component: usize,
    layer: usize,
    value: [u64; 2],
) -> [u8; 32] {
    let mut frame = b"iroha.zk-ams.v2.q-pcs.soundness.schedule\0".to_vec();
    frame.extend_from_slice(&[2, kind]);
    frame.extend_from_slice(&digest);
    frame.extend_from_slice(&[limb as u8, row as u8, component as u8, layer as u8]);
    frame.extend_from_slice(&value[0].to_be_bytes());
    frame.extend_from_slice(&value[1].to_be_bytes());
    keccak256(&frame)
}
fn manual_batch_schedule(transcript: [u8; 32]) -> ([u8; 32], usize) {
    let mut digest = transcript;
    let mut challenge_count = 0_usize;
    for limb in 0..38 {
        for row in 0..10 {
            let (committed_power, quotient_power) = if row % 2 == 0 {
                (0_u32, 1_u32)
            } else {
                (131_072_u32, 131_073_u32)
            };
            let mut formula = b"iroha.zk-ams.v2.q-pcs.soundness.schedule\0".to_vec();
            formula.extend_from_slice(&[2, 2, limb as u8, row as u8]);
            formula.extend_from_slice(&digest);
            formula.extend_from_slice(&committed_power.to_be_bytes());
            formula.extend_from_slice(&quotient_power.to_be_bytes());
            digest = keccak256(&formula);
            for component in 0..2 {
                let value = manual_fq2_challenge(
                    b"iroha.zk-ams.v2.q-pcs.soundness.ten-row-batch\0",
                    transcript,
                    limb,
                    row,
                    component,
                    0,
                );
                digest = manual_absorb_schedule_value(digest, 0, limb, row, component, 0, value);
                challenge_count += 1;
            }
        }
    }
    (digest, challenge_count)
}
fn manual_transcript_through_quotient(wire: &[u8]) -> [u8; 32] {
    let evaluations = &wire[HEADER_BYTES_V2..HEADER_BYTES_V2 + EVALUATION_BYTES_V2];
    let transcript =
        manual_absorb_evaluations(manual_initial_transcript(INITIAL_ROOT), evaluations);
    let quotient_offset = HEADER_BYTES_V2 + EVALUATION_BYTES_V2;
    let quotient_root: [u8; 32] = wire[quotient_offset..quotient_offset + 32]
        .try_into()
        .expect("quotient root");
    manual_absorb_root(
        b"iroha.zk-ams.v2.q-pcs.soundness.quotient-root\0",
        transcript,
        0,
        quotient_root,
    )
}
fn manual_fold_schedule(wire: &[u8]) -> ([u8; 32], usize, usize) {
    let mut transcript = manual_transcript_through_quotient(wire);
    let (mut schedule, batch_count) = manual_batch_schedule(transcript);
    let roots_offset = HEADER_BYTES_V2 + EVALUATION_BYTES_V2 + 32;
    let mut fold_count = 0_usize;
    for layer in 0..18 {
        let root: [u8; 32] = wire[roots_offset + 32 * layer..roots_offset + 32 * (layer + 1)]
            .try_into()
            .expect("FRI root");
        transcript = manual_absorb_root(
            b"iroha.zk-ams.v2.q-pcs.soundness.fri-root\0",
            transcript,
            layer as u8,
            root,
        );
        for limb in 0..38 {
            for row in 0..10 {
                let value = manual_fq2_challenge(
                    b"iroha.zk-ams.v2.q-pcs.soundness.ten-row-fold\0",
                    transcript,
                    limb,
                    row,
                    0,
                    layer,
                );
                schedule = manual_absorb_schedule_value(schedule, 1, limb, row, 0, layer, value);
                fold_count += 1;
            }
        }
    }
    (schedule, batch_count, fold_count)
}
fn manual_query_array_digest(queries: &[u32; QUERY_COUNT_V2]) -> [u8; 32] {
    let mut frame = Vec::with_capacity(QUERY_COUNT_V2 * 4);
    for query in queries {
        frame.extend_from_slice(&query.to_be_bytes());
    }
    keccak256(&frame)
}
fn manual_absorb_terminal(transcript: [u8; 32], terminal: &[u8]) -> [u8; 32] {
    let mut frame = b"iroha.zk-ams.v2.q-pcs.soundness.fri-terminal\0".to_vec();
    frame.push(2);
    frame.extend_from_slice(&transcript);
    frame.extend_from_slice(&380_u16.to_be_bytes());
    frame.extend_from_slice(terminal);
    keccak256(&frame)
}
fn manual_queries(transcript: [u8; 32]) -> [u32; QUERY_COUNT_V2] {
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
            let bytes = shake256(&frame, 8);
            let candidate = u64::from_be_bytes(bytes.try_into().expect("eight bytes"));
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
fn manual_queries_from_wire(wire: &[u8]) -> [u32; QUERY_COUNT_V2] {
    let mut transcript = manual_transcript_through_quotient(wire);
    let roots_offset = HEADER_BYTES_V2 + EVALUATION_BYTES_V2 + 32;
    for layer in 0..FRI_ROUNDS_V2 {
        let root: [u8; 32] = wire[roots_offset + 32 * layer..roots_offset + 32 * (layer + 1)]
            .try_into()
            .expect("FRI root");
        transcript = manual_absorb_root(
            b"iroha.zk-ams.v2.q-pcs.soundness.fri-root\0",
            transcript,
            layer as u8,
            root,
        );
    }
    let terminal_offset = roots_offset + FRI_ROOT_BYTES_V2;
    transcript = manual_absorb_terminal(
        transcript,
        &wire[terminal_offset..terminal_offset + TERMINAL_BYTES_V2],
    );
    manual_queries(transcript)
}
fn manual_indices(queries: &[u32; QUERY_COUNT_V2], length: usize) -> Vec<u32> {
    let half = (length / 2) as u32;
    let mut indices = Vec::with_capacity(2 * QUERY_COUNT_V2);
    for query in queries {
        let base = *query % half;
        indices.extend_from_slice(&[base, base + half]);
    }
    indices.sort_unstable();
    indices.dedup();
    indices
}
fn manual_authentication_count(mut indices: Vec<u32>, mut length: usize) -> usize {
    let mut authentication = 0_usize;
    while length > 1 {
        let mut parents = Vec::with_capacity(indices.len());
        for index in &indices {
            if indices.binary_search(&(*index ^ 1)).is_err() {
                authentication += 1;
            }
            parents.push(*index / 2);
        }
        parents.dedup();
        indices = parents;
        length /= 2;
    }
    authentication
}
fn authentication_cap_witness_queries(auth_only: bool) -> [u32; QUERY_COUNT_V2] {
    let mut states = [0_u8; QUERY_COUNT_V2];
    let mut next = 0_usize;
    for state in 0_u16..=u8::MAX as u16 {
        if (state as u8).count_ones() % 2 == 1 {
            states[next] = state as u8;
            next += 1;
        }
    }
    for state in 0_u16..=u8::MAX as u16 {
        if (state as u8).count_ones() % 2 == 0 {
            states[next] = state as u8;
            next += 1;
            if next == QUERY_COUNT_V2 {
                break;
            }
        }
    }
    assert_eq!(next, QUERY_COUNT_V2);
    let mut queries = [0_u32; QUERY_COUNT_V2];
    for (query, state) in queries.iter_mut().zip(states) {
        let start = if auth_only { 1_usize } else { 0 };
        for bit in start..18_usize {
            let state_bit = (if auth_only { bit - 1 } else { bit }) % 8;
            *query |= u32::from((state >> state_bit) & 1) << bit;
        }
    }
    queries
}
fn sha256_query_array(queries: &[u32; QUERY_COUNT_V2]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for query in queries {
        digest.update(query.to_be_bytes());
    }
    digest.finalize().into()
}
fn manual_fri_geometry(mut queries: [u32; QUERY_COUNT_V2]) -> (usize, usize) {
    let mut opened = 0_usize;
    let mut authentication = 0_usize;
    let mut length = 524_288_usize;
    for _ in 0..18 {
        let indices = manual_indices(&queries, length);
        opened += indices.len();
        authentication += manual_authentication_count(indices, length);
        let half = (length / 2) as u32;
        for query in &mut queries {
            *query %= half;
        }
        length /= 2;
    }
    assert_eq!(length, 2);
    (opened, authentication)
}
fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}
fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}
fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
}
fn append_manual_section(wire: &mut Vec<u8>, queries: &[u32; QUERY_COUNT_V2], length: usize) {
    let indices = manual_indices(queries, length);
    let authentication = manual_authentication_count(indices.clone(), length);
    wire.extend_from_slice(&(indices.len() as u32).to_be_bytes());
    wire.extend_from_slice(&(authentication as u32).to_be_bytes());
    wire.resize(wire.len() + indices.len() * LEAF_BYTES_V2, 0);
    wire.resize(wire.len() + authentication * 32, 0xa5);
}
fn canonical_wire_for_queries(query_override: Option<[u32; QUERY_COUNT_V2]>) -> Vec<u8> {
    let mut wire = vec![0_u8; FIXED_BEFORE_SECTIONS_V2];
    wire[..16].copy_from_slice(b"IROHA-QPCSV2\0\0\0\0");
    wire[16..24].copy_from_slice(&[2, 17, 19, 38, 5, 10, 18, 2]);
    put_u32(&mut wire, 24, 131_072);
    put_u32(&mut wire, 28, 524_288);
    put_u16(&mut wire, 32, 160);
    put_u16(&mut wire, 34, 320);
    put_u32(&mut wire, 36, 4_028);
    put_u32(&mut wire, 40, 3_392);
    put_u32(&mut wire, 44, 20_030);
    put_u32(&mut wire, 48, 6_080);
    put_u32(&mut wire, 52, 16);
    put_u64(&mut wire, 56, 29_245_792);
    wire[64..96].copy_from_slice(&manual_parameter_digest());
    wire[96..128].copy_from_slice(&SOURCE_DIGEST);
    wire[128..160].copy_from_slice(&ALGEBRA_DIGEST);
    wire[160..192].copy_from_slice(&INITIAL_ROOT);
    let initial_transcript = manual_initial_transcript(INITIAL_ROOT);
    let points = manual_points(initial_transcript);
    let evaluation_start = HEADER_BYTES_V2;
    for limb in 0..LIMBS_V2 {
        let modulus = RELEASE_MODULI_V1[limb];
        for repetition in 0..REPETITIONS_V2 {
            let relation = limb * REPETITIONS_V2 + repetition;
            let quotient = (relation as u64 + 1) % modulus;
            let factor = mod_add_v1(mod_pow_v1(points[relation], N_V2, modulus), 1, modulus);
            let product = mod_mul_v1(factor, quotient, modulus);
            let offset = evaluation_start + relation * 16;
            put_u64(&mut wire, offset, product);
            put_u64(&mut wire, offset + 8, quotient);
        }
    }
    let evaluation_end = evaluation_start + EVALUATION_BYTES_V2;
    let mut transcript =
        manual_absorb_evaluations(initial_transcript, &wire[evaluation_start..evaluation_end]);
    wire[evaluation_end..evaluation_end + 32].copy_from_slice(&QUOTIENT_ROOT);
    transcript = manual_absorb_root(
        b"iroha.zk-ams.v2.q-pcs.soundness.quotient-root\0",
        transcript,
        0,
        QUOTIENT_ROOT,
    );
    let roots_start = evaluation_end + 32;
    for layer in 0..FRI_ROUNDS_V2 {
        let root = [0x60 + layer as u8; 32];
        wire[roots_start + 32 * layer..roots_start + 32 * (layer + 1)].copy_from_slice(&root);
        transcript = manual_absorb_root(
            b"iroha.zk-ams.v2.q-pcs.soundness.fri-root\0",
            transcript,
            layer as u8,
            root,
        );
    }
    let terminal_start = roots_start + FRI_ROOT_BYTES_V2;
    let terminal = &wire[terminal_start..terminal_start + TERMINAL_BYTES_V2];
    transcript = manual_absorb_terminal(transcript, terminal);
    let queries = query_override.unwrap_or_else(|| manual_queries(transcript));
    append_manual_section(&mut wire, &queries, DOMAIN_SIZE_V2);
    append_manual_section(&mut wire, &queries, DOMAIN_SIZE_V2);
    let mut layer_queries = queries;
    let mut length = DOMAIN_SIZE_V2;
    for _ in 0..FRI_ROUNDS_V2 {
        append_manual_section(&mut wire, &layer_queries, length);
        let half = (length / 2) as u32;
        for query in &mut layer_queries {
            *query %= half;
        }
        length /= 2;
    }
    assert!(wire.len() <= MAX_PROOF_BYTES_V2);
    wire
}
fn canonical_wire() -> Vec<u8> {
    canonical_wire_for_queries(None)
}
fn through_relations<'a>(wire: &'a [u8]) -> RelationsCheckedV2<'a> {
    let mut header = begin_v2(wire, context(), SourceReplaySealV2::TestOnly).unwrap();
    let mut points = header.derive_points_v2().unwrap();
    points.check_relations_v2().unwrap()
}
fn through_fri<'a>(wire: &'a [u8]) -> FriTranscriptBoundV2<'a> {
    let mut relations = through_relations(wire);
    let mut quotient = relations.bind_quotient_root_v2().unwrap();
    quotient.bind_fri_transcript_v2().unwrap()
}
#[test]
fn independent_authentication_cap_witness_geometry_is_exact_and_correlated() {
    const AUTH_ONLY_QUERY_SHA256: [u8; 32] = [
        0x9e, 0x07, 0xdc, 0xc4, 0xd2, 0x42, 0xe2, 0x75, 0x7b, 0xe1, 0xf2, 0x07, 0xc4, 0x1a, 0xf8,
        0xb2, 0x23, 0x73, 0xa9, 0xc2, 0xe6, 0x26, 0xe3, 0x3c, 0x55, 0x07, 0x44, 0xc3, 0xd0, 0x2c,
        0x63, 0x87,
    ];
    const COMBINED_MAX_QUERY_SHA256: [u8; 32] = [
        0xf9, 0x42, 0x32, 0x31, 0xe4, 0x0b, 0xa5, 0xa6, 0xb1, 0xf1, 0x2c, 0x17, 0x03, 0x3e, 0x89,
        0xa5, 0xd3, 0xbd, 0x12, 0xcb, 0x10, 0x55, 0x78, 0xbd, 0x45, 0x2a, 0x1d, 0xeb, 0xf4, 0x7b,
        0xdf, 0x2c,
    ];
    let auth_only = authentication_cap_witness_queries(true);
    assert_eq!(sha256_query_array(&auth_only), AUTH_ONLY_QUERY_SHA256);
    assert_eq!(manual_fri_geometry(auth_only), (3_710, 20_030));
    let initial_indices = manual_indices(&auth_only, 524_288);
    assert_eq!(initial_indices.len(), 320);
    assert_eq!(manual_authentication_count(initial_indices, 524_288), 3_392);
    let auth_only_fri_bytes = 3_710 * 6_080 + 20_030 * 32;
    assert_eq!(auth_only_fri_bytes, 23_197_760);
    assert_eq!(
        checked_fri_multiproof_bytes_v2(3_710, 20_030).unwrap(),
        auth_only_fri_bytes
    );
    let auth_only_whole_bytes = 16_480 + 2 * (320 * 6_080 + 3_392 * 32) + auth_only_fri_bytes;
    assert_eq!(auth_only_whole_bytes, 27_322_528);
    let auth_only_wire = canonical_wire_for_queries(Some(auth_only));
    assert_eq!(auth_only_wire.len(), auth_only_whole_bytes);
    let mut auth_only_fri = through_fri(&auth_only_wire);
    // Test the bounded parser independently of a transcript-preimage search.
    auth_only_fri.live.as_mut().unwrap().queries = auth_only;
    assert!(auth_only_fri.parse_exact_sections_v2().is_ok());
    drop(auth_only_fri);
    drop(auth_only_wire);
    let combined_max = authentication_cap_witness_queries(false);
    assert_eq!(sha256_query_array(&combined_max), COMBINED_MAX_QUERY_SHA256);
    assert_eq!(manual_fri_geometry(combined_max), (4_028, 19_712));
    let combined_initial_indices = manual_indices(&combined_max, 524_288);
    assert_eq!(combined_initial_indices.len(), 320);
    assert_eq!(
        manual_authentication_count(combined_initial_indices, 524_288),
        3_392
    );
    assert_eq!(
        checked_fri_multiproof_bytes_v2(4_028, 19_712).unwrap(),
        25_121_024
    );
    assert_eq!(
        16_480 + 2 * (320 * 6_080 + 3_392 * 32) + 25_121_024,
        29_245_792
    );
    let mut combined_max_wire = canonical_wire_for_queries(Some(combined_max));
    assert_eq!(combined_max_wire.len(), 29_245_792);
    assert_eq!(combined_max_wire.len(), MAX_PROOF_BYTES_V2);
    assert_eq!(read_u32_v2(&combined_max_wire, 44).unwrap(), 20_030);
    let mut combined_max_fri = through_fri(&combined_max_wire);
    // Test the exact cap independently of a transcript-preimage search.
    combined_max_fri.live.as_mut().unwrap().queries = combined_max;
    assert!(combined_max_fri.parse_exact_sections_v2().is_ok());
    drop(combined_max_fri);
    put_u32(&mut combined_max_wire, 44, 19_712);
    assert!(matches!(
        begin_v2(&combined_max_wire, context(), SourceReplaySealV2::TestOnly),
        Err(SoundnessErrorV2::InvalidHeader)
    ));
    assert!(matches!(
        checked_fri_multiproof_bytes_v2(4_028, 19_713),
        Err(SoundnessErrorV2::InvalidSectionCount)
    ));
    assert_eq!(MAX_MULTIPROOF_AUTH_BYTES_V2, 858_048);
    assert_eq!(MAX_MULTIPROOF_SECTION_BYTES_V2, 29_229_312);
    assert_eq!(GLOBAL_PROOF_CAP_BYTES_V2 - MAX_PROOF_BYTES_V2, 4_308_640);
}
#[test]
fn independent_manual_transcript_oracle_matches_all_points() {
    let wire = canonical_wire();
    let mut header = begin_v2(&wire, context(), SourceReplaySealV2::TestOnly).unwrap();
    assert_eq!(
        header.live.as_ref().unwrap().header.parameter_digest,
        manual_parameter_digest()
    );
    let expected_transcript = manual_initial_transcript(INITIAL_ROOT);
    assert_eq!(manual_parameter_digest(), PARAMETER_KAT);
    assert_eq!(expected_transcript, INITIAL_TRANSCRIPT_KAT);
    let expected_points = manual_points(expected_transcript);
    let points = header.derive_points_v2().unwrap();
    let live = points.live.as_ref().unwrap();
    assert_eq!(live.transcript, expected_transcript);
    assert_eq!(live.relation_points, expected_points);
    let mut relations = through_relations(&wire);
    let quotient = relations.bind_quotient_root_v2().unwrap();
    let quotient_live = quotient.live.as_ref().unwrap();
    let quotient_transcript = manual_transcript_through_quotient(&wire);
    let (batch_schedule, batch_count) = manual_batch_schedule(quotient_transcript);
    assert_eq!(batch_count, 38 * 10 * 2);
    assert_eq!(batch_schedule, BATCH_SCHEDULE_KAT);
    assert_eq!(quotient_live.batch_schedule_digest, batch_schedule);
    let (fold_schedule, manual_batch_count, fold_count) = manual_fold_schedule(&wire);
    assert_eq!(manual_batch_count, 760);
    assert_eq!(fold_count, 18 * 38 * 10);
    assert_eq!(fold_schedule, FOLD_SCHEDULE_KAT);
    let fri = through_fri(&wire);
    let queries = manual_queries_from_wire(&wire);
    assert_eq!(
        fri.live.as_ref().unwrap().fold_schedule_digest,
        fold_schedule
    );
    assert_eq!(fri.live.as_ref().unwrap().queries, queries);
    assert_eq!(manual_query_array_digest(&queries), QUERY_ARRAY_KAT);
}
#[test]
fn prover_post_root_typestate_matches_the_independent_t0_t1_t2_oracle() {
    let wire = canonical_wire();
    let expected_t0 = manual_initial_transcript(INITIAL_ROOT);
    let expected_points = manual_points(expected_t0);
    let points = ProverPostRootPointsV2::derive_v2(
        PARAMETER_KAT,
        SOURCE_DIGEST,
        ALGEBRA_DIGEST,
        INITIAL_ROOT,
    )
    .unwrap();
    for limb in 0..LIMBS_V2 {
        for repetition in 0..REPETITIONS_V2 {
            assert_eq!(
                points.point_v2(limb, repetition).unwrap(),
                expected_points[limb * REPETITIONS_V2 + repetition]
            );
        }
    }
    let evaluations = &wire[HEADER_BYTES_V2..HEADER_BYTES_V2 + EVALUATION_BYTES_V2];
    let expected_t1 = manual_absorb_evaluations(expected_t0, evaluations);
    let evaluations_bound = points.bind_evaluations_v2(evaluations).unwrap();
    assert_eq!(evaluations_bound.transcript_v2().unwrap(), expected_t1);
    let expected_t2 = manual_absorb_root(
        b"iroha.zk-ams.v2.q-pcs.soundness.quotient-root\0",
        expected_t1,
        0,
        QUOTIENT_ROOT,
    );
    let quotient_bound = evaluations_bound
        .bind_quotient_root_v2(QUOTIENT_ROOT)
        .unwrap();
    assert_eq!(quotient_bound.transcript_v2().unwrap(), expected_t2);
    assert_eq!(quotient_bound.quotient_root_v2().unwrap(), QUOTIENT_ROOT);
    assert_eq!(
        quotient_bound.live.as_ref().unwrap().batch_schedule_digest,
        BATCH_SCHEDULE_KAT
    );
}
#[test]
fn move_only_batch_owner_reuses_the_exact_760_value_schedule_and_poison_order() {
    let wire = canonical_wire();
    let evaluations = &wire[HEADER_BYTES_V2..HEADER_BYTES_V2 + EVALUATION_BYTES_V2];
    let expected_t0 = manual_initial_transcript(INITIAL_ROOT);
    let expected_t1 = manual_absorb_evaluations(expected_t0, evaluations);
    let expected_t2 = manual_absorb_root(
        b"iroha.zk-ams.v2.q-pcs.soundness.quotient-root\0",
        expected_t1,
        0,
        QUOTIENT_ROOT,
    );
    let points = ProverPostRootPointsV2::derive_v2(
        PARAMETER_KAT,
        SOURCE_DIGEST,
        ALGEBRA_DIGEST,
        INITIAL_ROOT,
    )
    .unwrap();
    let mut batch = points
        .bind_evaluations_v2(evaluations)
        .unwrap()
        .bind_quotient_root_v2(QUOTIENT_ROOT)
        .unwrap()
        .begin_batch_challenges_v2()
        .unwrap();
    assert_eq!(
        batch.context_v2().unwrap(),
        (expected_t1, expected_t2, BATCH_SCHEDULE_KAT)
    );
    let mut committed = [0_u8; 16_384];
    let mut quotient = [0_u8; 16_384];
    for value in committed.chunks_exact_mut(16) {
        value[..8].copy_from_slice(&3_u64.to_be_bytes());
        value[8..].copy_from_slice(&4_u64.to_be_bytes());
    }
    for value in quotient.chunks_exact_mut(16) {
        value[..8].copy_from_slice(&5_u64.to_be_bytes());
        value[8..].copy_from_slice(&6_u64.to_be_bytes());
    }
    let mut output = [0_u8; 16_384];
    batch
        .mix_next_block_v2(0, 0, &committed, &quotient, &mut output)
        .unwrap();
    let field = Fq2ParametersV1::derive(RELEASE_MODULI_V1[0], 19).unwrap();
    let a = manual_fq2_challenge(
        b"iroha.zk-ams.v2.q-pcs.soundness.ten-row-batch\0",
        expected_t2,
        0,
        0,
        0,
        0,
    );
    let b = manual_fq2_challenge(
        b"iroha.zk-ams.v2.q-pcs.soundness.ten-row-batch\0",
        expected_t2,
        0,
        0,
        1,
        0,
    );
    for lane in [0_usize, 1, 1_023] {
        let x = field.pow(field.domain_root, lane as u128);
        let expected = field.add(
            field.mul(Fq2V1 { c0: a[0], c1: a[1] }, Fq2V1 { c0: 3, c1: 4 }),
            field.mul(
                Fq2V1 { c0: b[0], c1: b[1] },
                field.mul(x, Fq2V1 { c0: 5, c1: 6 }),
            ),
        );
        assert_eq!(
            &output[lane * 16..lane * 16 + 16],
            &[expected.c0.to_be_bytes(), expected.c1.to_be_bytes()].concat()
        );
    }
    batch
        .mix_next_block_v2(0, 1, &committed, &quotient, &mut output)
        .unwrap();
    let a = manual_fq2_challenge(
        b"iroha.zk-ams.v2.q-pcs.soundness.ten-row-batch\0",
        expected_t2,
        0,
        1,
        0,
        0,
    );
    let b = manual_fq2_challenge(
        b"iroha.zk-ams.v2.q-pcs.soundness.ten-row-batch\0",
        expected_t2,
        0,
        1,
        1,
        0,
    );
    for lane in [0_usize, 1, 1_023] {
        let x = field.pow(field.domain_root, lane as u128);
        let x_n = field.pow(x, N_V2 as u128);
        let expected = field.add(
            field.mul(
                Fq2V1 { c0: a[0], c1: a[1] },
                field.mul(x_n, Fq2V1 { c0: 3, c1: 4 }),
            ),
            field.mul(
                Fq2V1 { c0: b[0], c1: b[1] },
                field.mul(field.mul(x_n, x), Fq2V1 { c0: 5, c1: 6 }),
            ),
        );
        assert_eq!(
            &output[lane * 16..lane * 16 + 16],
            &[expected.c0.to_be_bytes(), expected.c1.to_be_bytes()].concat()
        );
    }
    assert!(matches!(
        batch.mix_next_block_v2(0, 3, &committed, &quotient, &mut output),
        Err(SoundnessErrorV2::InvalidBatchEquation)
    ));
    assert!(matches!(
        batch.mix_next_block_v2(0, 2, &committed, &quotient, &mut output),
        Err(SoundnessErrorV2::Poisoned)
    ));
}
#[test]
fn layer0_root_owner_derives_exact_380_alphas_folds_and_poisons_hostile_order() {
    let wire = canonical_wire();
    assert_eq!(
        manual_transcript_through_quotient(&wire),
        PRE_FRI_TRANSCRIPT_KAT
    );
    let rows = ProverBatchRowsCompleteV2 {
        transcript: PRE_FRI_TRANSCRIPT_KAT,
        batch_schedule_digest: BATCH_SCHEDULE_KAT,
    };
    let mut fold = rows.bind_fri_layer0_root_v2([0x60; 32]).unwrap();
    assert_eq!(
        fold.context_v2().unwrap(),
        (
            PRE_FRI_TRANSCRIPT_KAT,
            POST_FRI0_TRANSCRIPT_KAT,
            BATCH_SCHEDULE_KAT,
            FOLD0_SCHEDULE_KAT,
            [0x60; 32],
        )
    );
    let first_alpha = fold.live.as_ref().unwrap().alphas[0];
    assert_eq!((first_alpha.c0, first_alpha.c1), ALPHA0_FIRST_KAT);
    assert_eq!(fold.live.as_ref().unwrap().alphas.len(), 380);
    let last_alpha = fold.live.as_ref().unwrap().alphas[379];
    assert_eq!((last_alpha.c0, last_alpha.c1), ALPHA0_LAST_KAT);
    let mut positive = [0_u8; 16_384];
    let mut negative = [0_u8; 16_384];
    for value in positive.chunks_exact_mut(16) {
        value[..8].copy_from_slice(&3_u64.to_be_bytes());
        value[8..].copy_from_slice(&4_u64.to_be_bytes());
    }
    for value in negative.chunks_exact_mut(16) {
        value[..8].copy_from_slice(&5_u64.to_be_bytes());
        value[8..].copy_from_slice(&6_u64.to_be_bytes());
    }
    let mut output = [0_u8; 16_384];
    fold.fold_next_pair_v2(0, 0, &positive, &negative, &mut output)
        .unwrap();
    let decode = |lane: usize| {
        (
            u64::from_be_bytes(output[lane * 16..lane * 16 + 8].try_into().unwrap()),
            u64::from_be_bytes(output[lane * 16 + 8..lane * 16 + 16].try_into().unwrap()),
        )
    };
    assert_eq!(decode(0), FOLD0_LANE0_KAT);
    assert_eq!(decode(1), FOLD0_LANE1_KAT);
    fold.next_pair_block = 256;
    fold.next_column = 0;
    let complete = fold.complete_v2().unwrap();
    assert_eq!(
        complete.context_v2(),
        (
            PRE_FRI_TRANSCRIPT_KAT,
            POST_FRI0_TRANSCRIPT_KAT,
            BATCH_SCHEDULE_KAT,
            FOLD0_SCHEDULE_KAT,
            [0x60; 32],
        )
    );
    let rows = ProverBatchRowsCompleteV2 {
        transcript: PRE_FRI_TRANSCRIPT_KAT,
        batch_schedule_digest: BATCH_SCHEDULE_KAT,
    };
    let mut hostile = rows.bind_fri_layer0_root_v2([0x60; 32]).unwrap();
    assert!(matches!(
        hostile.fold_next_pair_v2(0, 1, &positive, &negative, &mut output),
        Err(SoundnessErrorV2::InvalidFriEquation)
    ));
    assert!(matches!(
        hostile.fold_next_pair_v2(0, 0, &positive, &negative, &mut output),
        Err(SoundnessErrorV2::Poisoned)
    ));
    positive[..8].copy_from_slice(&RELEASE_MODULI_V1[0].to_be_bytes());
    let rows = ProverBatchRowsCompleteV2 {
        transcript: PRE_FRI_TRANSCRIPT_KAT,
        batch_schedule_digest: BATCH_SCHEDULE_KAT,
    };
    let mut noncanonical = rows.bind_fri_layer0_root_v2([0x60; 32]).unwrap();
    assert!(matches!(
        noncanonical.fold_next_pair_v2(0, 0, &positive, &negative, &mut output),
        Err(SoundnessErrorV2::NonCanonicalResidue)
    ));
    assert!(matches!(
        noncanonical.fold_next_pair_v2(0, 0, &positive, &negative, &mut output),
        Err(SoundnessErrorV2::Poisoned)
    ));
}
#[test]
fn canonical_envelope_reaches_only_non_authorizing_structural_state() {
    let wire = canonical_wire();
    let mut header = begin_v2(&wire, context(), SourceReplaySealV2::TestOnly).unwrap();
    let mut points = header.derive_points_v2().unwrap();
    assert!(matches!(
        header.derive_points_v2(),
        Err(SoundnessErrorV2::Poisoned)
    ));
    let mut relations = points.check_relations_v2().unwrap();
    assert!(matches!(
        points.check_relations_v2(),
        Err(SoundnessErrorV2::Poisoned)
    ));
    let mut quotient = relations.bind_quotient_root_v2().unwrap();
    let mut fri = quotient.bind_fri_transcript_v2().unwrap();
    let parsed = fri.parse_exact_sections_v2().unwrap();
    assert!(parsed.live.is_some());
    assert!(TEN_ROW_MERKLE_PATHS_VERIFIED_V2);
    assert!(OPENING_QUOTIENT_EQUATIONS_VERIFIED_V2);
    assert!(TEN_ROW_BATCHING_EQUATIONS_VERIFIED_V2);
    assert!(TEN_ROW_FRI_EQUATIONS_VERIFIED_V2);
    assert!(!RELEASE_READY_V2);
}
#[test]
fn relation_and_encoding_fail_before_quotient_binding() {
    let wire = canonical_wire();
    let mut changed = wire.clone();
    changed[HEADER_BYTES_V2 + 7] ^= 1;
    let mut header = begin_v2(&changed, context(), SourceReplaySealV2::TestOnly).unwrap();
    let mut points = header.derive_points_v2().unwrap();
    assert!(matches!(
        points.check_relations_v2(),
        Err(SoundnessErrorV2::RelationMismatch)
    ));
    let mut noncanonical = wire;
    noncanonical[HEADER_BYTES_V2..HEADER_BYTES_V2 + 8]
        .copy_from_slice(&RELEASE_MODULI_V1[0].to_be_bytes());
    let mut header = begin_v2(&noncanonical, context(), SourceReplaySealV2::TestOnly).unwrap();
    let mut points = header.derive_points_v2().unwrap();
    assert!(matches!(
        points.check_relations_v2(),
        Err(SoundnessErrorV2::NonCanonicalResidue)
    ));
}
#[test]
fn root_context_order_and_terminal_mutations_fail_closed() {
    let wire = canonical_wire();
    let mut zero_root = wire.clone();
    zero_root[160..192].fill(0);
    assert!(matches!(
        begin_v2(&zero_root, context(), SourceReplaySealV2::TestOnly),
        Err(SoundnessErrorV2::InvalidRoot)
    ));
    let wrong_context = ExpectedPublicContextV2 {
        sealed_source_transcript_digest: [9; 32],
        source_algebra_binding_digest: ALGEBRA_DIGEST,
    };
    assert!(matches!(
        begin_v2(&wire, wrong_context, SourceReplaySealV2::TestOnly),
        Err(SoundnessErrorV2::InvalidPublicContext)
    ));
    let mut reordered = wire.clone();
    reordered[HEADER_BYTES_V2..HEADER_BYTES_V2 + EVALUATION_BYTES_V2 + 32].rotate_right(32);
    let mut header = begin_v2(&reordered, context(), SourceReplaySealV2::TestOnly).unwrap();
    let mut points = header.derive_points_v2().unwrap();
    assert!(matches!(
        points.check_relations_v2(),
        Err(SoundnessErrorV2::NonCanonicalResidue | SoundnessErrorV2::RelationMismatch)
    ));
    let quotient_offset = HEADER_BYTES_V2 + EVALUATION_BYTES_V2;
    let mut zero_quotient = wire.clone();
    zero_quotient[quotient_offset..quotient_offset + 32].fill(0);
    let mut relations = through_relations(&zero_quotient);
    assert!(matches!(
        relations.bind_quotient_root_v2(),
        Err(SoundnessErrorV2::InvalidRoot)
    ));
    let terminal_offset = quotient_offset + 32 + FRI_ROOT_BYTES_V2;
    let mut terminal_mismatch = wire;
    terminal_mismatch[terminal_offset + LEAF_BYTES_V2 + 7] = 1;
    let mut relations = through_relations(&terminal_mismatch);
    let mut quotient = relations.bind_quotient_root_v2().unwrap();
    assert!(matches!(
        quotient.bind_fri_transcript_v2(),
        Err(SoundnessErrorV2::InvalidTerminal)
    ));
}
#[test]
fn every_bound_root_controls_later_challenges() {
    let wire = canonical_wire();
    let mut initial_mutation = wire.clone();
    initial_mutation[191] ^= 1;
    let mut original_header = begin_v2(&wire, context(), SourceReplaySealV2::TestOnly).unwrap();
    let mut changed_header =
        begin_v2(&initial_mutation, context(), SourceReplaySealV2::TestOnly).unwrap();
    let original_points = original_header.derive_points_v2().unwrap();
    let changed_points = changed_header.derive_points_v2().unwrap();
    assert_ne!(
        original_points.live.as_ref().unwrap().relation_points,
        changed_points.live.as_ref().unwrap().relation_points
    );
    let first_fri_root = HEADER_BYTES_V2 + EVALUATION_BYTES_V2 + 32;
    let mut fri_mutation = wire.clone();
    fri_mutation[first_fri_root] ^= 1;
    let original = through_fri(&wire);
    let changed = through_fri(&fri_mutation);
    assert_ne!(
        original.live.as_ref().unwrap().fold_schedule_digest,
        changed.live.as_ref().unwrap().fold_schedule_digest
    );
    assert_ne!(
        original.live.as_ref().unwrap().queries,
        changed.live.as_ref().unwrap().queries
    );
}
#[test]
fn section_counts_canonical_values_caps_and_trailing_bytes_are_strict() {
    let wire = canonical_wire();
    let mut bad_count = wire.clone();
    put_u32(&mut bad_count, FIXED_BEFORE_SECTIONS_V2, 319);
    let mut fri = through_fri(&bad_count);
    assert!(matches!(
        fri.parse_exact_sections_v2(),
        Err(SoundnessErrorV2::InvalidSectionCount)
    ));
    let mut bad_value = wire.clone();
    let first_value = FIXED_BEFORE_SECTIONS_V2 + SECTION_HEADER_BYTES_V2;
    bad_value[first_value..first_value + 8].copy_from_slice(&RELEASE_MODULI_V1[0].to_be_bytes());
    let mut fri = through_fri(&bad_value);
    assert!(matches!(
        fri.parse_exact_sections_v2(),
        Err(SoundnessErrorV2::NonCanonicalResidue)
    ));
    let mut trailing = wire.clone();
    trailing.push(0);
    let mut fri = through_fri(&trailing);
    assert!(matches!(
        fri.parse_exact_sections_v2(),
        Err(SoundnessErrorV2::TrailingBytes)
    ));
    assert!(matches!(
        begin_v2(
            &vec![0; MAX_PROOF_BYTES_V2 + 1],
            context(),
            SourceReplaySealV2::TestOnly
        ),
        Err(SoundnessErrorV2::ProofCapExceeded)
    ));
}
#[test]
fn source_guards_keep_the_slice_private_bounded_and_honest() {
    let source = include_str!("phase23_rns_link_q_pcs_v2_soundness.rs");
    let parent = include_str!("phase23_rns_link_q_pcs.rs");
    assert!(source.lines().count() <= 1_450);
    assert!(source.contains("source_and_algebra: Infallible"));
    assert!(source.contains("authenticated_multipass_replay: Infallible"));
    assert!(source.contains("const ROWS_PER_LIMB_V2: usize = 10;"));
    assert!(source.contains("const BATCH_CHALLENGE_COUNT_V2: usize = COORDINATE_COUNT_V2 * 2;"));
    assert!(source.contains("field.pow(field.domain_root, u128::from(block) * 1_024)"));
    assert!(source.contains("struct ProverFriLayer0ChallengesV2"));
    assert!(source.contains("struct ProverFriLayer0FoldCompleteV2"));
    assert!(source.contains("absorb_root_v2(FRI_ROOT_DOMAIN_V2, self.transcript, 0, root)"));
    assert!(
        source
            .contains("absorb_schedule_value_v2(fold_schedule_digest, 1, limb, row, 0, 0, alpha)")
    );
    assert!(source.contains("let exponent = u128::from(pair_block) * 1_024;"));
    assert!(source.contains("const MAX_PROOF_BYTES_V2: usize = 29_245_792;"));
    assert!(source.contains("const MAX_FRI_AUTH_HASHES_V2: usize = 20_030;"));
    assert!(source.contains("const MAX_FRI_MULTIPROOF_BYTES_V2: usize = 25_121_024;"));
    assert!(source.contains("const MAX_MULTIPROOF_SECTION_BYTES_V2: usize ="));
    assert!(source.matches("checked_fri_multiproof_bytes_v2(").count() >= 2);
    for false_gate in [
        "SOURCE_AGGREGATION_LINKED_V2: bool = false",
        "CROSS_SET_ALGEBRA_VERIFIED_V2: bool = false",
        "HYRAX_LINKED_V2: bool = false",
        "PRODUCTION_SAMPLER_QUALIFIED_V2: bool = false",
        "ZERO_KNOWLEDGE_THEOREM_INSTANTIATED_V2: bool = false",
        "AUTHENTICATED_MULTIPASS_REPLAY_INTEGRATED_V2: bool = false",
        "COEFFICIENT_TOP_ZERO_REPLAY_VERIFIED_V2: bool = false",
        "COMPLETE_WORK_BOUND_DERIVED_V2: bool = false",
        "MEASURED_RSS_WITHIN_CAP_V2: bool = false",
        "OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false",
        "RELEASE_READY_V2: bool = false",
    ] {
        assert!(source.contains(false_gate));
    }
    for true_gate in [
        "TEN_ROW_MERKLE_PATHS_VERIFIED_V2: bool = true",
        "OPENING_QUOTIENT_EQUATIONS_VERIFIED_V2: bool = true",
        "TEN_ROW_BATCHING_EQUATIONS_VERIFIED_V2: bool = true",
        "TEN_ROW_FRI_EQUATIONS_VERIFIED_V2: bool = true",
    ] {
        assert!(source.contains(true_gate));
    }
    assert!(
        parent.contains("#[path = \"phase23_rns_link_q_pcs_v2_soundness.rs\"]\nmod v2_soundness;")
    );
    assert!(!parent.contains(concat!("pub use ", "v2_soundness")));
    assert!(!source.contains("pub struct"));
    assert!(!source.contains("pub enum"));
    assert!(!source.contains("pub fn"));
    assert!(!source.contains("Vec<"));
    assert!(!source.contains(".to_vec()"));
    assert!(!source.contains("shake256("));
    assert!(source.contains("Shake256Reader"));
    assert!(!source.contains("caller_challenge"));
    assert!(!source.contains("impl Clone for HeaderParsedV2"));
    assert!(!source.contains("impl Clone for PointsDerivedV2"));
    assert!(!source.contains("impl Clone for RelationsCheckedV2"));
    assert!(!source.contains("impl Clone for QuotientRootBoundV2"));
    assert!(!source.contains("impl Clone for ProverBatchChallengesV2"));
    assert!(!source.contains("impl Clone for ProverFriLayer0ChallengesV2"));
    assert!(!source.contains("impl Clone for ProverFriLayer0FoldCompleteV2"));
    assert!(!source.contains("impl Clone for FriTranscriptBoundV2"));
    assert!(!source.contains("impl Clone for StructurallyParsedV2"));
}
