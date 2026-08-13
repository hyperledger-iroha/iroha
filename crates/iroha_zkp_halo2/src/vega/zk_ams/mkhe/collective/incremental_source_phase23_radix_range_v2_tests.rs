use super::*;
const PRODUCTION_SOURCE_V2: &str = include_str!("incremental_source_phase23_radix_range_v2.rs");
const TEST_SOURCE_V2: &str = include_str!("incremental_source_phase23_radix_range_v2_tests.rs");
const PARENT_SOURCE_V2: &str = include_str!("incremental_source_phase23.rs");
fn exact_transcript_manifest_v2() -> Vec<u8> {
    encode_transcript_frames_v2(&RADIX_RANGE_TRANSCRIPT_FRAMES_V2)
}
fn encode_transcript_frames_v2(frames: &[&[u8]]) -> Vec<u8> {
    let mut encoded = Vec::new();
    for (ordinal, frame) in frames.iter().enumerate() {
        encoded.extend_from_slice(&(ordinal as u16).to_be_bytes());
        encoded.extend_from_slice(&(frame.len() as u16).to_be_bytes());
        encoded.extend_from_slice(frame);
    }
    encoded
}
#[test]
fn exact_topology_formulas_and_coordinates_are_frozen() {
    assert_eq!(RADIX_RECORDS_V2, 43);
    assert_eq!(RADIX_RECORD_ORDER_V2, b"X1/U16/E16/RE1/W8/RW1");
    assert_eq!(RADIX_GROUPS_PER_RECORD_V2, 8);
    assert_eq!(RADIX_GROUPS_V2, 344);
    assert_eq!(RADIX_COEFFICIENTS_PER_GROUP_V2, 16_384);
    assert_eq!(RADIX_SOURCE_BLOCKS_PER_GROUP_V2, 64);
    assert_eq!(RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2, 256);
    assert_eq!(RADIX_COEFFICIENTS_V2, 5_636_096);
    assert_eq!(RADIX_LOW_DIGITS_PER_COEFFICIENT_V2, 34);
    assert_eq!(RADIX_LOW_DIGITS_V2, 191_627_264);
    assert_eq!(RADIX_TOP_BITS_V2, 11_272_192);
    assert_eq!(RADIX_INVERSE_PLANES_PER_GROUP_V2, 2);
    assert_eq!(RADIX_INVERSE_POINTS_PER_PLANE_V2, 17);
    assert_eq!(RADIX_INVERSE_POINTS_PER_GROUP_V2, 34);
    assert_eq!(RADIX_COMMITMENT_POINTS_PER_GROUP_V2, 70);
    assert_eq!(RADIX_RANGE_COMMITMENT_POINTS_V2, 24_080);
    assert_eq!(RADIX_SOURCE_COEFFICIENT_COMMITMENT_POINTS_V2, 344);
    assert_eq!(
        DECOMPOSITION_D_FORMULA_V2,
        b"D=sum_{h=0}^{16}(2^15)^h*d_h+(2^15)^17*b_d"
    );
    assert_eq!(
        DECOMPOSITION_S_FORMULA_V2,
        b"S=sum_{h=0}^{16}(2^15)^h*s_h+(2^15)^17*b_s"
    );
    assert_eq!(CANONICAL_VALUE_FORMULA_V2, b"v=D mod p;D+S=p-1");
    assert_eq!(DIGIT_TABLE_FORMULA_V2, b"d_h,s_h in [0,32767]");
    assert_eq!(TOP_BIT_FORMULA_V2, b"b_d*(b_d-1)=b_s*(b_s-1)=b_d*b_s=0");
    assert_eq!(
        COMMITMENT_TOPOLOGY_FORMULA_V2,
        b"per-group:source1,D17,S17,Dinv17,Sinv17,Dtop1,Stop1"
    );
    assert_eq!(
        LOOKUP_FORMULA_V2,
        b"reject until z notin [0,32767];then U_D,U_S=(z-A)^-1;absorb Dinv then Sinv"
    );
    let family_boundaries = [(0, 1), (1, 2), (17, 3), (33, 4), (34, 5), (42, 6)];
    for (ordinal, family) in family_boundaries {
        assert_eq!(
            source_coordinate_v2(ordinal, 0, 0, 0).unwrap().family,
            family
        );
    }
    let first = source_coordinate_v2(0, 0, 0, 0).unwrap();
    assert_eq!(first.ordinal, 0);
    assert_eq!(first.group, 0);
    assert_eq!(first.source_block, 0);
    assert_eq!(first.coefficient, 0);
    assert_eq!(first.source_index, 0);
    assert_eq!(first.packing_index, 0);
    let transposed = source_coordinate_v2(0, 0, 1, 2).unwrap();
    assert_eq!(transposed.source_index, 258);
    assert_eq!(transposed.packing_index, 129);
    let last = source_coordinate_v2(42, 7, 63, 255).unwrap();
    assert_eq!(last.source_index, 5_636_095);
    assert_eq!(last.packing_index, 5_636_095);
    assert!(source_coordinate_v2(43, 0, 0, 0).is_err());
    assert!(source_coordinate_v2(0, 8, 0, 0).is_err());
    assert!(source_coordinate_v2(0, 0, 64, 0).is_err());
    assert!(source_coordinate_v2(0, 0, 0, 256).is_err());
    let order = core::array::from_fn(|index| index as u16);
    let exact = topology_digest_for_record_order_v2(&order).unwrap();
    assert_eq!(exact, exact_topology_digest_v2().unwrap());
    let mut swapped = order;
    swapped.swap(0, 1);
    assert_ne!(
        topology_digest_for_record_order_v2(&swapped).unwrap(),
        exact
    );
    swapped[1] = 1;
    assert!(topology_digest_for_record_order_v2(&swapped).is_err());
}
#[test]
fn exact_wire_work_io_heap_and_soundness_equations_are_planning_only() {
    let wire_components = [
        RADIX_WIRE_HEADER_BYTES_V2,
        RADIX_WIRE_TERMINAL_BP_BYTES_V2,
        RADIX_WIRE_CROSS_SCHNORR_BYTES_V2,
        RADIX_WIRE_SOURCE_COEFFICIENT_POINTS_BYTES_V2,
        RADIX_WIRE_DIGIT_SLACK_INVERSE_TOP_POINTS_BYTES_V2,
        RADIX_WIRE_MULTIPLICITY_BYTES_V2,
        RADIX_WIRE_CUBIC_MESSAGES_BYTES_V2,
        RADIX_WIRE_HIDDEN_EVALUATION_COMMITMENTS_BYTES_V2,
        RADIX_WIRE_COEFFICIENT_BASIS_IPAS_BYTES_V2,
        RADIX_WIRE_TABLE_IPA_BYTES_V2,
        RADIX_WIRE_MASK_COMMITMENT_IPA_BYTES_V2,
        RADIX_WIRE_32_GATE_BP_BYTES_V2,
        RADIX_WIRE_PACKING_OPENINGS_BYTES_V2,
    ];
    assert_eq!(wire_components.into_iter().sum::<usize>(), 2_149_717);
    assert_eq!(RADIX_WIRE_CUBIC_MESSAGES_BYTES_V2, 233 * 96);
    assert_eq!(RADIX_WIRE_HIDDEN_EVALUATION_COMMITMENTS_BYTES_V2, 52 * 33);
    assert_eq!(RADIX_WIRE_COEFFICIENT_BASIS_IPAS_BYTES_V2, 16_352);
    assert_eq!(RADIX_WIRE_TABLE_IPA_BYTES_V2, 1_088);
    assert_eq!(RADIX_WIRE_MASK_COMMITMENT_IPA_BYTES_V2, 725);
    assert_eq!(RADIX_WIRE_32_GATE_BP_BYTES_V2, 834);
    assert_eq!(RADIX_WIRE_PACKING_OPENINGS_BYTES_V2, 1_216_031);
    assert_eq!(Q_PCS_WIRE_BYTES_V2, 29_245_792);
    assert_eq!(RADIX_Q_PCS_COMBINED_WIRE_BYTES_V2, 31_395_509);
    assert_eq!(RADIX_Q_PCS_COMBINED_CAP_BYTES_V2, 33_554_432);
    assert_eq!(RADIX_Q_PCS_COMBINED_MARGIN_BYTES_V2, 2_158_923);
    assert_eq!(RADIX_DIGIT_SLACK_EMISSIONS_V2, 191_627_264);
    assert_eq!(RADIX_BATCH_INVERSIONS_MAX_V2, 344 * 17);
    assert_eq!(RADIX_INVERSE_PASS_MULTIPLICATIONS_V2, 574_881_792);
    assert_eq!(RADIX_FIXED_BASE_SOURCE_RANGE_TERMS_V2, 400_187_240);
    assert_eq!(RADIX_FIXED_BASE_TERMINAL_TERMS_V2, 1_574_400);
    assert_eq!(RADIX_SUMCHECK_VISITS_V2, 789_053_396);
    assert_eq!(RADIX_PACKING_TRANSPOSE_STAGES_V2, 95_813_632);
    assert_eq!(RADIX_COMMITTED_IPAS_V2, 1_536);
    assert_eq!(RADIX_COMMITTED_IPA_VECTOR_LENGTH_V2, 2_048);
    assert_eq!(RADIX_TABLE_IPAS_V2, 1);
    assert_eq!(RADIX_TABLE_IPA_VECTOR_LENGTH_V2, 32_768);
    assert_eq!(RADIX_EXTERNAL_IO_BYTES_V2, 26_846_528_789);
    assert_eq!(RADIX_CONFIDENTIAL_SCRATCH_BYTES_V2, 6_836_977_664);
    assert_eq!(RADIX_SOURCE_PUBLICATION_BYTES_V2, 7_152_600_416);
    assert_eq!(Q_PCS_EXTERNAL_PEAK_BYTES_V2, 10_504_241_168);
    assert_eq!(RADIX_LOCAL_HEAP_BYTES_V2, 20_598_361);
    assert_eq!(RADIX_RETAINED_SOURCE_ROOT_BYTES_V2, 83_503_936);
    assert_eq!(RADIX_PHASE_NAMED_HEAP_BYTES_V2, 104_102_297);
    assert_eq!(Q_PCS_CONSERVATIVE_HEAP_BYTES_V2, 120_129_088);
    assert_eq!(RADIX_Q_PCS_OVERLAP_HEAP_BYTES_V2, 140_727_449);
    assert_eq!(RADIX_Q_PCS_HEAP_CEILING_BYTES_V2, 167_772_160);
    assert_eq!(RADIX_Q_PCS_HEAP_MARGIN_BYTES_V2, 27_044_711);
    let modulus = VEGA_T256_SCALAR_MODULUS_BE_V1
        .iter()
        .fold(0.0_f64, |value, byte| value * 256.0 + f64::from(*byte));
    let failure = RADIX_LOOKUP_SOUNDNESS_NUMERATOR_V2 as f64 / (modulus - 32_768.0);
    let bits = -failure.log2();
    assert!((228.48..228.49).contains(&bits));
    assert_eq!(RADIX_LOOKUP_SOUNDNESS_BITS_X100_FLOOR_V2, 22_848);
    assert_eq!(
        LOOKUP_SOUNDNESS_FORMULA_V2,
        b"191679039/(p-32768)<2^-228.48"
    );
    assert_eq!(CROSS_BASIS_STATISTICAL_HVZK_BITS_V2, 245);
    assert_eq!(
        CROSS_BASIS_HVZK_FORMULA_V2,
        b"64-byte-modular-reduction-vector-mask:distance-from-ideal<2^-245"
    );
    assert_eq!(
        STATIC_EVIDENCE_FORMULA_V2,
        b"planning-only:no-proof:no-authority:no-RSS:no-release"
    );
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TinyRadix {
    low: [u8; 3],
    top: u8,
}
fn tiny_decompose_v2(mut value: u16) -> TinyRadix {
    let low = core::array::from_fn(|_| {
        let digit = (value % 4) as u8;
        value /= 4;
        digit
    });
    TinyRadix {
        low,
        top: value as u8,
    }
}
fn tiny_reconstruct_v2(radix: TinyRadix) -> Option<u16> {
    if radix.top > 1 || radix.low.iter().any(|digit| *digit >= 4) {
        return None;
    }
    Some(
        u16::from(radix.low[0])
            + 4 * u16::from(radix.low[1])
            + 16 * u16::from(radix.low[2])
            + 64 * u16::from(radix.top),
    )
}
fn tiny_canonical_relation_v2(value: u16, d: TinyRadix, s: TinyRadix) -> bool {
    match (tiny_reconstruct_v2(d), tiny_reconstruct_v2(s)) {
        (Some(d_value), Some(s_value)) => {
            value < 113 && d_value % 113 == value && d_value + s_value == 112 && d.top * s.top == 0
        }
        _ => false,
    }
}
#[test]
fn independent_tiny_canonical_radix_kat_rejects_overflow_high_bits() {
    let kats = [
        (
            0,
            TinyRadix {
                low: [0, 0, 0],
                top: 0,
            },
            TinyRadix {
                low: [0, 0, 3],
                top: 1,
            },
        ),
        (
            1,
            TinyRadix {
                low: [1, 0, 0],
                top: 0,
            },
            TinyRadix {
                low: [3, 3, 2],
                top: 1,
            },
        ),
        (
            56,
            TinyRadix {
                low: [0, 2, 3],
                top: 0,
            },
            TinyRadix {
                low: [0, 2, 3],
                top: 0,
            },
        ),
        (
            57,
            TinyRadix {
                low: [1, 2, 3],
                top: 0,
            },
            TinyRadix {
                low: [3, 1, 3],
                top: 0,
            },
        ),
        (
            112,
            TinyRadix {
                low: [0, 0, 3],
                top: 1,
            },
            TinyRadix {
                low: [0, 0, 0],
                top: 0,
            },
        ),
    ];
    for (value, expected_d, expected_s) in kats {
        assert_eq!(tiny_decompose_v2(value), expected_d);
        assert_eq!(tiny_decompose_v2(112 - value), expected_s);
        assert!(tiny_canonical_relation_v2(value, expected_d, expected_s));
    }
    assert!(!tiny_canonical_relation_v2(
        0,
        TinyRadix {
            low: [0, 0, 0],
            top: 2
        },
        TinyRadix {
            low: [0, 0, 3],
            top: 1
        }
    ));
    assert!(!tiny_canonical_relation_v2(
        0,
        TinyRadix {
            low: [4, 0, 0],
            top: 0
        },
        TinyRadix {
            low: [0, 0, 3],
            top: 1
        }
    ));
    assert!(!tiny_canonical_relation_v2(
        113,
        tiny_decompose_v2(113),
        tiny_decompose_v2(112)
    ));
}
fn tiny_mod_pow_v2(mut base: u16, mut exponent: u16) -> u16 {
    let mut result = 1_u16;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = (result * base) % 113;
        }
        base = (base * base) % 113;
        exponent >>= 1;
    }
    result
}
fn tiny_lookup_side_v2(values: &[u16], z: u16) -> u16 {
    values.iter().fold(0_u16, |sum, value| {
        (sum + tiny_mod_pow_v2((z + 113 - value) % 113, 111)) % 113
    })
}
fn tiny_table_side_v2(multiplicities: &[u16; 4], z: u16) -> u16 {
    multiplicities
        .iter()
        .enumerate()
        .fold(0_u16, |sum, (table, multiplicity)| {
            let inverse = tiny_mod_pow_v2((z + 113 - table as u16) % 113, 111);
            (sum + multiplicity * inverse) % 113
        })
}
#[test]
fn independent_log_derivative_lookup_kat_detects_a_replacement() {
    let table_side = tiny_table_side_v2(&[1, 2, 0, 1], 7);
    assert_eq!(table_side, 107);
    assert_eq!(tiny_lookup_side_v2(&[0, 3, 1, 1], 7), 107);
    assert_eq!(tiny_lookup_side_v2(&[0, 3, 1, 4], 7), 13);
    assert_ne!(tiny_lookup_side_v2(&[0, 3, 1, 4], 7), table_side);
}
fn tiny_packing_transpose_v2(source: &[u16], blocks: usize, width: usize) -> Vec<u16> {
    let mut packed = Vec::with_capacity(source.len());
    for coefficient in 0..width {
        for block in 0..blocks {
            packed.push(source[block * width + coefficient]);
        }
    }
    packed
}
#[test]
fn independent_packing_transpose_oracle_detects_coordinate_and_order_mutations() {
    let source = [0_u16, 1, 2, 3, 4, 5];
    assert_eq!(tiny_packing_transpose_v2(&source, 2, 3), [0, 3, 1, 4, 2, 5]);
    assert_ne!(tiny_packing_transpose_v2(&source, 3, 2), [0, 3, 1, 4, 2, 5]);
    let exact = source_coordinate_v2(0, 0, 1, 2).unwrap();
    let swapped_coordinate = source_coordinate_v2(0, 0, 2, 1).unwrap();
    assert_ne!(exact.source_index, swapped_coordinate.source_index);
    assert_ne!(exact.packing_index, swapped_coordinate.packing_index);
    let next_group = source_coordinate_v2(0, 1, 1, 2).unwrap();
    assert_eq!(next_group.source_index - exact.source_index, 16_384);
    assert_eq!(next_group.packing_index - exact.packing_index, 16_384);
    let order = core::array::from_fn(|index| index as u16);
    let exact_digest = topology_digest_for_record_order_v2(&order).unwrap();
    let mut reordered = order;
    reordered.swap(16, 17);
    assert_ne!(
        topology_digest_for_record_order_v2(&reordered).unwrap(),
        exact_digest
    );
}
#[test]
fn transcript_manifest_rejects_splice_missing_extra_noncanonical_and_trailing_frames() {
    let exact = exact_transcript_manifest_v2();
    let digest = require_exact_transcript_manifest_v2(&exact).unwrap();
    assert_ne!(digest, [0; 32]);
    let mut spliced_frames = RADIX_RANGE_TRANSCRIPT_FRAMES_V2.to_vec();
    spliced_frames.swap(14, 15);
    assert!(
        require_exact_transcript_manifest_v2(&encode_transcript_frames_v2(&spliced_frames))
            .is_err()
    );
    let missing = encode_transcript_frames_v2(&RADIX_RANGE_TRANSCRIPT_FRAMES_V2[..31]);
    assert!(require_exact_transcript_manifest_v2(&missing).is_err());
    let mut extra_frames = RADIX_RANGE_TRANSCRIPT_FRAMES_V2.to_vec();
    extra_frames.push(b"extra");
    assert!(
        require_exact_transcript_manifest_v2(&encode_transcript_frames_v2(&extra_frames)).is_err()
    );
    let mut noncanonical_ordinal = exact.clone();
    noncanonical_ordinal[1] = 1;
    assert!(require_exact_transcript_manifest_v2(&noncanonical_ordinal).is_err());
    let mut noncanonical_length = exact.clone();
    noncanonical_length[3] += 1;
    assert!(require_exact_transcript_manifest_v2(&noncanonical_length).is_err());
    let mut trailing = exact;
    trailing.push(0);
    assert!(require_exact_transcript_manifest_v2(&trailing).is_err());
    assert_eq!(
        &RADIX_RANGE_TRANSCRIPT_FRAMES_V2[4..12],
        &[
            b"terminal-commitments".as_slice(),
            b"group-source-coefficient-commitments-344".as_slice(),
            b"d-low-digit-commitments".as_slice(),
            b"s-low-digit-commitments".as_slice(),
            b"d-top-bit-commitments".as_slice(),
            b"s-top-bit-commitments".as_slice(),
            b"lookup-multiplicity-commitment".as_slice(),
            b"zero-sum-mask-commitment".as_slice(),
        ]
    );
    let z = RADIX_RANGE_TRANSCRIPT_FRAMES_V2
        .iter()
        .position(|frame| *frame == b"lookup-z-outside-digit-table")
        .unwrap();
    let d_inverse = RADIX_RANGE_TRANSCRIPT_FRAMES_V2
        .iter()
        .position(|frame| *frame == b"d-inverse-commitments")
        .unwrap();
    let s_inverse = RADIX_RANGE_TRANSCRIPT_FRAMES_V2
        .iter()
        .position(|frame| *frame == b"s-inverse-commitments")
        .unwrap();
    let shard = RADIX_RANGE_TRANSCRIPT_FRAMES_V2
        .iter()
        .position(|frame| *frame == b"shard-challenges")
        .unwrap();
    let constraint = RADIX_RANGE_TRANSCRIPT_FRAMES_V2
        .iter()
        .position(|frame| *frame == b"constraint-challenges")
        .unwrap();
    let binding = RADIX_RANGE_TRANSCRIPT_FRAMES_V2
        .iter()
        .position(|frame| *frame == b"binding-digest")
        .unwrap();
    let q_link = RADIX_RANGE_TRANSCRIPT_FRAMES_V2
        .iter()
        .position(|frame| *frame == b"future-q-l-linkage")
        .unwrap();
    let gamma = RADIX_RANGE_TRANSCRIPT_FRAMES_V2
        .iter()
        .position(|frame| *frame == b"gamma")
        .unwrap();
    assert_eq!(
        (z, d_inverse, s_inverse, shard, constraint),
        (19, 20, 21, 22, 23)
    );
    assert!(binding < q_link && q_link < gamma);
    assert!(
        !RADIX_RANGE_TRANSCRIPT_FRAMES_V2
            .iter()
            .any(|frame| *frame == b"lookup-U")
    );
    for inverse in [d_inverse, s_inverse] {
        let mut before_z = RADIX_RANGE_TRANSCRIPT_FRAMES_V2.to_vec();
        before_z.swap(z, inverse);
        assert!(
            require_exact_transcript_manifest_v2(&encode_transcript_frames_v2(&before_z)).is_err()
        );
    }
    let mut swapped_inverses = RADIX_RANGE_TRANSCRIPT_FRAMES_V2.to_vec();
    swapped_inverses.swap(d_inverse, s_inverse);
    assert!(
        require_exact_transcript_manifest_v2(&encode_transcript_frames_v2(&swapped_inverses))
            .is_err()
    );
    for missing_inverse in [d_inverse, s_inverse] {
        let mut missing_inverse_frames = RADIX_RANGE_TRANSCRIPT_FRAMES_V2.to_vec();
        missing_inverse_frames.remove(missing_inverse);
        assert!(
            require_exact_transcript_manifest_v2(&encode_transcript_frames_v2(
                &missing_inverse_frames
            ))
            .is_err()
        );
    }
    let mut extra_inverse = RADIX_RANGE_TRANSCRIPT_FRAMES_V2.to_vec();
    extra_inverse.insert(shard, b"s-inverse-commitments");
    assert!(
        require_exact_transcript_manifest_v2(&encode_transcript_frames_v2(&extra_inverse)).is_err()
    );
    let mut independent_u = RADIX_RANGE_TRANSCRIPT_FRAMES_V2.to_vec();
    independent_u.insert(shard, b"lookup-U");
    assert!(
        require_exact_transcript_manifest_v2(&encode_transcript_frames_v2(&independent_u)).is_err()
    );
}
fn test_ingress_v2() -> RadixRangeIngressV2 {
    RadixRangeIngressV2::begin_v2(
        RadixRangeSourceSealV2::TestOnly,
        RadixRangeReplaySealV2::TestOnly,
        RadixRangePackingSealV2::TestOnly,
        RadixRangeZkSealV2::TestOnly,
    )
}
#[test]
fn typestate_poison_error_unwind_zeroization_privacy_and_source_guards_hold() {
    ZEROIZED_TRANSIENT_DROPS_V2.store(0, Ordering::SeqCst);
    assert!(test_ingress_v2().check_v2(&[]).is_err());
    assert_eq!(ZEROIZED_TRANSIENT_DROPS_V2.load(Ordering::SeqCst), 1);
    let mut poisoned = test_ingress_v2();
    let live = poisoned.live.take().unwrap();
    drop(live);
    assert!(poisoned.check_v2(&exact_transcript_manifest_v2()).is_err());
    assert_eq!(ZEROIZED_TRANSIENT_DROPS_V2.load(Ordering::SeqCst), 2);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        test_ingress_v2().force_unwind_after_take_v2()
    }));
    assert!(unwind.is_err());
    assert_eq!(ZEROIZED_TRANSIENT_DROPS_V2.load(Ordering::SeqCst), 3);
    let prerequisite = consume_phase23_radix_range_static_prerequisite_v2(
        RadixRangeSourceSealV2::TestOnly,
        RadixRangeReplaySealV2::TestOnly,
        RadixRangePackingSealV2::TestOnly,
        RadixRangeZkSealV2::TestOnly,
        &exact_transcript_manifest_v2(),
    )
    .unwrap();
    assert!(prerequisite.live.is_some());
    assert!(!prerequisite.record.source_algebra_actually_verified);
    assert!(!prerequisite.record.radix_decomposition_actually_verified);
    assert!(!prerequisite.record.radix_canonical_range_actually_verified);
    assert!(!prerequisite.record.radix_lookup_actually_verified);
    assert!(!prerequisite.record.zero_knowledge_actually_verified);
    assert!(!prerequisite.record.packing_transpose_actually_verified);
    assert!(!prerequisite.record.packing_equality_actually_verified);
    assert!(!prerequisite.record.hyrax_actually_verified);
    assert!(!prerequisite.record.q_pcs_replay_actually_verified);
    assert!(!prerequisite.record.q_pcs_handoff_actually_complete);
    assert!(!prerequisite.record.operational_qualification_accepted);
    assert!(!prerequisite.record.receipt_accepted);
    assert!(!prerequisite.record.rss_qualified);
    assert!(!prerequisite.record.proof_minted);
    assert!(!prerequisite.record.authority_minted);
    assert!(!prerequisite.record.release_complete);
    drop(prerequisite);
    assert_eq!(ZEROIZED_TRANSIENT_DROPS_V2.load(Ordering::SeqCst), 4);
    for impossible in [
        "source_algebra: Infallible",
        "authenticated_replay: Infallible",
        "packing_transpose: Infallible",
        "packing_equality: Infallible",
        "radix_lookup: Infallible",
        "sumcheck: Infallible",
        "hyrax: Infallible",
        "statistical_hvzk: Infallible",
    ] {
        assert!(PRODUCTION_SOURCE_V2.contains(impossible));
    }
    assert!(
        PRODUCTION_SOURCE_V2
            .contains("let mut live = self\n            .live\n            .take()")
    );
    let check = PRODUCTION_SOURCE_V2.split("fn check_v2").nth(1).unwrap();
    assert!(
        check.find(".take()").unwrap()
            < check.find("require_exact_transcript_manifest_v2").unwrap()
    );
    let freeze = PRODUCTION_SOURCE_V2.split("fn freeze_v2").nth(1).unwrap();
    assert!(freeze.find(".take()").unwrap() < freeze.find("nonzero_digest_v2").unwrap());
    let pre_z = PRODUCTION_SOURCE_V2
        .split("impl<'a> RadixRangePreLookupManifestV2")
        .nth(1)
        .unwrap()
        .split("impl<'a> RadixRangeLookupZDerivedManifestV2")
        .next()
        .unwrap();
    assert!(pre_z.contains("absorb_until_v2(20)"));
    assert!(!pre_z.contains("absorb_z_dependent_inverse_planes_v2"));
    let post_z = PRODUCTION_SOURCE_V2
        .split("impl<'a> RadixRangeLookupZDerivedManifestV2")
        .nth(1)
        .unwrap()
        .split("impl RadixRangeLookupUManifestV2")
        .next()
        .unwrap();
    assert!(post_z.contains("absorb_z_dependent_inverse_planes_v2"));
    assert!(post_z.contains("absorb_until_v2(22)"));
    assert!(!PRODUCTION_SOURCE_V2.contains("pub(crate)"));
    assert!(!PRODUCTION_SOURCE_V2.contains("pub fn"));
    assert!(!PRODUCTION_SOURCE_V2.contains("#[derive"));
    assert!(!PRODUCTION_SOURCE_V2.contains("VegaT256Point"));
    assert!(!PRODUCTION_SOURCE_V2.contains("[u8; 33]"));
    assert!(!PRODUCTION_SOURCE_V2.contains("Encode"));
    assert!(!PRODUCTION_SOURCE_V2.contains("Decode"));
    assert!(!PRODUCTION_SOURCE_V2.contains("into_parts"));
    assert!(!PRODUCTION_SOURCE_V2.contains("as_tuple"));
    assert!(!PRODUCTION_SOURCE_V2.contains("FnOnce"));
    assert!(!PRODUCTION_SOURCE_V2.contains("dyn Fn"));
    assert!(!PRODUCTION_SOURCE_V2.contains("mem::forget"));
    assert!(PARENT_SOURCE_V2.contains("mod radix_range_v2;"));
    assert!(!PARENT_SOURCE_V2.contains("pub mod radix_range_v2;"));
}
#[test]
fn source_and_test_files_remain_within_the_scoped_budgets() {
    assert!(PRODUCTION_SOURCE_V2.lines().count() <= 1_200);
    assert!(TEST_SOURCE_V2.lines().count() <= 700);
    assert!(PRODUCTION_SOURCE_V2.len() <= 52_000);
    assert!(TEST_SOURCE_V2.len() <= 34_000);
}
