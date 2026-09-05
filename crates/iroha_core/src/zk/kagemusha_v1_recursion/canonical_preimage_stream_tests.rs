//! Byte-stream routing, range, padding, provenance and fixed-topology regressions.

use std::collections::BTreeSet;

use halo2_base::{ContextCell, ContextTag, gates::circuit::builder::BaseCircuitBuilder};
use halo2_proofs::{
    dev::{MockProver, VerifyFailure},
    halo2curves::pasta::{Fp, Fq},
};

use super::*;
use crate::zk::kagemusha_v1_recursion::guard_bundle::assign_bytes;

const STREAM_TEST_K: u32 = 12;
// An exact Range8 lookup has no scaled companion gate that could mask a routing mutation.
const STREAM_TEST_LOOKUP_BITS: usize = 8;

fn patterned_prefix(capacity: usize, actual_len: usize, seed: u8) -> Vec<u8> {
    assert!(actual_len <= capacity);
    (0..capacity)
        .map(|index| {
            if index < actual_len {
                seed.wrapping_add((index as u8).wrapping_mul(37))
            } else {
                0
            }
        })
        .collect()
}

fn expected_concat(
    left: &[u8],
    left_len: usize,
    right: &[u8],
    right_len: usize,
    output_capacity: usize,
) -> Vec<u8> {
    let mut result = left[..left_len].to_vec();
    result.extend_from_slice(&right[..right_len]);
    result.resize(output_capacity, 0);
    result
}

fn stream_builder<F: KagemushaPoseidonFieldV1>(
    left: &[u8],
    left_len: F,
    right: &[u8],
    right_len: F,
    output_capacity: usize,
) -> BaseCircuitBuilder<F> {
    let mut builder = BaseCircuitBuilder::<F>::default()
        .use_k(STREAM_TEST_K as usize)
        .use_lookup_bits(STREAM_TEST_LOOKUP_BITS)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let left_bytes = assign_bytes(ctx, &range, left);
    let left_len = ctx.load_witness(left_len);
    let left = KagemushaBoundedByteStreamV1::constrain(ctx, &range, left_bytes, left_len)
        .expect("left prefix");
    let right_bytes = assign_bytes(ctx, &range, right);
    let right_len = ctx.load_witness(right_len);
    let right = KagemushaBoundedByteStreamV1::constrain(ctx, &range, right_bytes, right_len)
        .expect("right prefix");
    let output = left
        .concat(ctx, &range, &right, output_capacity)
        .expect("concatenation");
    let mut instances = vec![left_len, right_len, output.actual_len()];
    instances.extend(
        output
            .bytes()
            .iter()
            .map(|byte| byte.assigned().expect("assigned output byte")),
    );
    builder.assigned_instances = vec![instances];
    builder.calculate_params(Some(9));
    builder
}

fn stream_case<F: KagemushaPoseidonFieldV1>(
    left: &[u8],
    left_len: F,
    right: &[u8],
    right_len: F,
    output_len: F,
    expected: &[u8],
) -> bool {
    let builder = stream_builder(left, left_len, right, right_len, expected.len());
    let mut instances = vec![left_len, right_len, output_len];
    instances.extend(expected.iter().map(|byte| F::from(u64::from(*byte))));
    MockProver::run(STREAM_TEST_K, &builder, vec![instances])
        .expect("byte-stream mock prover")
        .verify()
        .is_ok()
}

fn assert_concat_boundaries<F: KagemushaPoseidonFieldV1>() {
    // Active zero bytes still contribute to the exact concatenated length.
    assert!(stream_case::<F>(
        &[0; 5],
        F::from(3),
        &[0; 7],
        F::from(4),
        F::from(7),
        &[0; 12],
    ));
    // Every active-length pair for small non-power-of-two buffers, including empty prefixes.
    for left_len in 0..=3 {
        for right_len in 0..=5 {
            let left = patterned_prefix(3, left_len, 0x21);
            let right = patterned_prefix(5, right_len, 0x91);
            let expected = expected_concat(&left, left_len, &right, right_len, 8);
            assert!(stream_case::<F>(
                &left,
                F::from(left_len as u64),
                &right,
                F::from(right_len as u64),
                F::from((left_len + right_len) as u64),
                &expected,
            ));
        }
    }
    for (left_capacity, left_len, right_capacity, right_len, output_capacity) in [
        (0, 0, 0, 0, 0),
        (0, 0, 9, 9, 9),
        (9, 9, 0, 0, 9),
        (9, 0, 9, 0, 0),
        (9, 2, 9, 3, 5),
        (3, 2, 5, 4, 17),
        (16, 16, 9, 9, 25),
        (33, 8, 33, 9, 17),
        (33, 33, 0, 0, 33),
        (17, 7, 9, 9, 26),
        (17, 8, 9, 9, 26),
        (17, 9, 9, 9, 26),
        (17, 15, 9, 9, 26),
        (17, 16, 9, 9, 26),
        (17, 17, 9, 9, 26),
    ] {
        let left = patterned_prefix(left_capacity, left_len, 0x11);
        let right = patterned_prefix(right_capacity, right_len, 0x81);
        let expected = expected_concat(&left, left_len, &right, right_len, output_capacity);
        assert!(stream_case::<F>(
            &left,
            F::from(left_len as u64),
            &right,
            F::from(right_len as u64),
            F::from((left_len + right_len) as u64),
            &expected,
        ));
    }
}

#[test]
fn bounded_stream_concat_matches_exact_prefixes_in_both_fields() {
    assert_concat_boundaries::<Fp>();
    assert_concat_boundaries::<Fq>();
}

fn assert_invalid_lengths<F: KagemushaPoseidonFieldV1>() {
    for bad_length in [
        F::from(6),
        F::from(7),
        F::from(8),
        F::from(u64::MAX),
        -F::ONE,
    ] {
        assert!(!stream_case::<F>(
            &[0; 5],
            bad_length,
            &[0; 5],
            F::ZERO,
            bad_length,
            &[0; 10]
        ));
        assert!(!stream_case::<F>(
            &[0; 5],
            F::ZERO,
            &[0; 5],
            bad_length,
            bad_length,
            &[0; 10]
        ));
    }
    // A negative left length plus a positive right length cannot cancel into a valid total.
    assert!(!stream_case::<F>(
        &[0; 5],
        -F::ONE,
        &[0; 5],
        F::ONE,
        F::ZERO,
        &[0; 5]
    ));
    for bad_length in [F::ONE, -F::ONE] {
        assert!(!stream_case::<F>(
            &[],
            bad_length,
            &[],
            F::ZERO,
            bad_length,
            &[]
        ));
        assert!(!stream_case::<F>(
            &[],
            F::ZERO,
            &[],
            bad_length,
            bad_length,
            &[]
        ));
    }
}

#[test]
fn bounded_stream_rejects_negative_oversized_and_cancelled_lengths_in_both_fields() {
    assert_invalid_lengths::<Fp>();
    assert_invalid_lengths::<Fq>();
}

fn assert_nonzero_tails<F: KagemushaPoseidonFieldV1>() {
    let left = vec![0; 5];
    let right = vec![0; 5];
    for index in 2..5 {
        let mut corrupt = left.clone();
        corrupt[index] = 1;
        let mut expected = vec![0; 10];
        expected[index] = 1;
        // Match the malformed routing result, so only the input-tail rule rejects this.
        assert!(!stream_case::<F>(
            &corrupt,
            F::from(2),
            &right,
            F::from(3),
            F::from(5),
            &expected
        ));
    }
    for index in 3..5 {
        let mut corrupt = right.clone();
        corrupt[index] = 1;
        let mut expected = vec![0; 10];
        expected[2 + index] = 1;
        assert!(!stream_case::<F>(
            &left,
            F::from(2),
            &corrupt,
            F::from(3),
            F::from(5),
            &expected
        ));
    }
    // The entire zero-length source is a tail, even if every byte would be truncated away.
    assert!(!stream_case::<F>(
        &[1; 5],
        F::ZERO,
        &[],
        F::ZERO,
        F::ZERO,
        &[]
    ));
    assert!(!stream_case::<F>(
        &[],
        F::ZERO,
        &[1; 5],
        F::ZERO,
        F::ZERO,
        &[]
    ));
}

#[test]
fn bounded_stream_rejects_nonzero_tails_including_truncated_sources_in_both_fields() {
    assert_nonzero_tails::<Fp>();
    assert_nonzero_tails::<Fq>();
}

fn assert_output_overflow<F: KagemushaPoseidonFieldV1>() {
    for (left_len, right_len, output_capacity) in
        [(4, 4, 7), (5, 0, 4), (0, 5, 4), (1, 0, 0), (0, 1, 0)]
    {
        let left = patterned_prefix(5, left_len, 0x21);
        let right = patterned_prefix(5, right_len, 0x91);
        // Give the truncated bytes the routing algorithm would produce. Rejection must come
        // from the exact length bound, even when no visible byte exposes the discarded suffix.
        let expected = expected_concat(&left, left_len, &right, right_len, output_capacity);
        assert!(!stream_case::<F>(
            &left,
            F::from(left_len as u64),
            &right,
            F::from(right_len as u64),
            F::from((left_len + right_len) as u64),
            &expected,
        ));
    }
    assert!(!stream_case::<F>(
        &[0; 5],
        F::from(5),
        &[0; 5],
        F::from(5),
        F::from(10),
        &[0; 9]
    ));
}

#[test]
fn bounded_stream_rejects_total_length_overflow_before_truncation_in_both_fields() {
    assert_output_overflow::<Fp>();
    assert_output_overflow::<Fq>();
}

fn assert_output_substitution<F: KagemushaPoseidonFieldV1>() {
    let left = [0x21, 0, 0x43, 0, 0];
    let right = [0x91, 0, 0xb3, 0, 0];
    let expected = expected_concat(&left, 3, &right, 3, 10);
    assert!(stream_case::<F>(
        &left,
        F::from(3),
        &right,
        F::from(3),
        F::from(6),
        &expected
    ));
    for index in 0..expected.len() {
        let mut corrupt = expected.clone();
        corrupt[index] ^= 0x80;
        assert!(!stream_case::<F>(
            &left,
            F::from(3),
            &right,
            F::from(3),
            F::from(6),
            &corrupt
        ));
    }
    // Byte content cannot substitute a different boundary or a relabelled active length.
    let mut relocated = expected.clone();
    relocated.swap(3, 6);
    assert!(!stream_case::<F>(
        &left,
        F::from(3),
        &right,
        F::from(3),
        F::from(6),
        &relocated
    ));
    for wrong_length in [5, 7] {
        assert!(!stream_case::<F>(
            &left,
            F::from(3),
            &right,
            F::from(3),
            F::from(wrong_length),
            &expected
        ));
    }
}

#[test]
fn bounded_stream_rejects_substituted_bytes_boundaries_and_lengths_in_both_fields() {
    assert_output_substitution::<Fp>();
    assert_output_substitution::<Fq>();
}

/// Change every Base and cached lookup copy so only the output's defining arithmetic rejects it.
fn replace_output_equivalence_class<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    target: AssignedValue<F>,
    value: F,
) {
    assert_eq!(
        builder.lookup_bits(),
        Some(8),
        "mutations require direct Range8 lookups"
    );
    let target = target.cell.expect("mutation target");
    let equalities = builder
        .core()
        .copy_manager
        .lock()
        .expect("copy manager")
        .advice_equalities
        .clone();
    let mut cells = BTreeSet::from([target]);
    loop {
        let previous_len = cells.len();
        for (left, right) in &equalities {
            if cells.contains(left) || cells.contains(right) {
                cells.insert(*left);
                cells.insert(*right);
            }
        }
        if cells.len() == previous_len {
            break;
        }
    }
    for cell in &cells {
        assert_eq!(cell.type_id(), target.type_id());
        assert_eq!(cell.context_id(), 0);
        builder
            .main(0)
            .replace_advice_with_trivial(cell.offset(), value);
    }
    let replacement = builder
        .main(0)
        .get(isize::try_from(target.offset()).expect("mutation target offset"))
        .value;
    let mut cached_copies = 0;
    for manager in builder.lookup_manager() {
        let mut lookups = manager.cells_to_lookup.lock().expect("lookup cells");
        for row in lookups.values_mut().flatten() {
            for lookup in row {
                if lookup.cell.is_some_and(|cell| cells.contains(&cell)) {
                    lookup.value = replacement;
                    cached_copies += 1;
                }
            }
        }
    }
    assert!(
        cached_copies > 0,
        "the output must have a direct Range8 lookup"
    );
}

fn assert_arithmetic_routing_mutations<F: KagemushaPoseidonFieldV1>() {
    let left = [1, 2, 3, 0, 0];
    let right = [0x11, 0x12, 0x13, 0, 0];
    let expected = expected_concat(&left, 3, &right, 3, 10);
    let mut wrong_outputs = Vec::new();
    for index in 0..expected.len() {
        let mut wrong = expected.clone();
        wrong[index] ^= 0x40;
        wrong_outputs.push(wrong);
    }
    wrong_outputs.push(vec![1, 2, 3, 0, 0x11, 0x12, 0x13, 0, 0, 0]);
    wrong_outputs.push(vec![1, 2, 0x11, 0x12, 0x13, 3, 0, 0, 0, 0]);
    for wrong in wrong_outputs {
        let mut builder = stream_builder::<F>(&left, F::from(3), &right, F::from(3), 10);
        let outputs = builder.assigned_instances[0][3..].to_vec();
        for (index, byte) in wrong.iter().copied().enumerate() {
            if byte != expected[index] {
                replace_output_equivalence_class(
                    &mut builder,
                    outputs[index],
                    F::from(u64::from(byte)),
                );
            }
        }
        let mut instances = vec![F::from(3), F::from(3), F::from(6)];
        instances.extend(wrong.into_iter().map(|byte| F::from(u64::from(byte))));
        let failures = MockProver::run(STREAM_TEST_K, &builder, vec![instances])
            .expect("mutated routing mock prover")
            .verify()
            .expect_err("coordinated output mutations must fail routing arithmetic");
        assert!(
            failures
                .iter()
                .any(|failure| matches!(failure, VerifyFailure::ConstraintNotSatisfied { .. })),
            "mutation failed without an arithmetic constraint failure: {failures:?}",
        );
    }
}

#[test]
fn bounded_stream_routing_rejects_coordinated_output_witness_mutations_in_both_fields() {
    assert_arithmetic_routing_mutations::<Fp>();
    assert_arithmetic_routing_mutations::<Fq>();
}

fn assert_missing_length_identity<F: KagemushaPoseidonFieldV1>() {
    let mut builder = BaseCircuitBuilder::<F>::default()
        .use_k(STREAM_TEST_K as usize)
        .use_lookup_bits(STREAM_TEST_LOOKUP_BITS);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let mut length = ctx.load_witness(F::ZERO);
    length.cell = None;
    let before = ctx.advice_len();
    assert!(KagemushaBoundedByteStreamV1::constrain(ctx, &range, Vec::new(), length).is_err());
    assert_eq!(
        ctx.advice_len(),
        before,
        "reject invalid handles before gates"
    );
}

#[test]
fn bounded_stream_rejects_unbound_length_handles_before_gates_in_both_fields() {
    assert_missing_length_identity::<Fp>();
    assert_missing_length_identity::<Fq>();
}

fn assert_constant_fragments_and_crc<F: KagemushaPoseidonFieldV1>() {
    let mut builder = BaseCircuitBuilder::<F>::default()
        .use_k(STREAM_TEST_K as usize)
        .use_lookup_bits(STREAM_TEST_LOOKUP_BITS)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let prefix_len = ctx.load_constant(F::from(3));
    let prefix = KagemushaBoundedByteStreamV1::constrain(
        ctx,
        &range,
        b"kgm"
            .iter()
            .copied()
            .map(PastaSha256ByteV1::constant)
            .collect(),
        prefix_len,
    )
    .expect("constant prefix");
    let field = assign_bytes(ctx, &range, &[0x21, 0, 0x43, 0, 0, 0, 0, 0, 0]);
    let field_len = ctx.load_witness(F::from(3));
    let field = KagemushaBoundedByteStreamV1::constrain(ctx, &range, field, field_len)
        .expect("variable semantic field");
    let suffix_len = ctx.load_constant(F::from(2));
    let suffix = KagemushaBoundedByteStreamV1::constrain(
        ctx,
        &range,
        vec![
            PastaSha256ByteV1::constant(0),
            PastaSha256ByteV1::constant(0x7f),
        ],
        suffix_len,
    )
    .expect("constant suffix");
    let joined = prefix
        .concat(ctx, &range, &field, 12)
        .expect("prefix and field");
    let joined = joined
        .concat(ctx, &range, &suffix, 14)
        .expect("append suffix");
    let crc =
        super::super::crc64_xz_prefix_bytes_v1(ctx, &range, joined.bytes(), joined.actual_len())
            .expect("exact active-prefix CRC");
    let mut instances = vec![joined.actual_len()];
    instances.extend(joined.bytes().iter().map(|byte| {
        range
            .gate()
            .mul(ctx, byte.quantum_cell(), QuantumCell::Constant(F::ONE))
    }));
    instances.extend(crc.map(|byte| byte.assigned().expect("constrained CRC byte")));
    builder.assigned_instances = vec![instances];
    builder.calculate_params(Some(9));
    let active = b"kgm\x21\x00\x43\x00\x7f";
    let mut padded = active.to_vec();
    padded.resize(14, 0);
    let mut expected = vec![F::from(active.len() as u64)];
    expected.extend(padded.into_iter().map(|byte| F::from(u64::from(byte))));
    expected.extend(
        norito::crc64_fallback(active)
            .to_le_bytes()
            .into_iter()
            .map(|byte| F::from(u64::from(byte))),
    );
    MockProver::run(STREAM_TEST_K, &builder, vec![expected])
        .expect("constant/variable concat CRC mock prover")
        .assert_satisfied();
}

#[test]
fn bounded_stream_preserves_constants_embedded_zeroes_and_exact_crc_in_both_fields() {
    assert_constant_fragments_and_crc::<Fp>();
    assert_constant_fragments_and_crc::<Fq>();
}

#[derive(Debug, PartialEq, Eq)]
struct ByteStreamShape<F: KagemushaPoseidonFieldV1> {
    k: usize,
    advice_columns: Vec<usize>,
    fixed_columns: usize,
    lookup_columns: Vec<usize>,
    lookup_bits: Option<usize>,
    instance_columns: usize,
    advice_rows: Vec<usize>,
    selectors: Vec<Vec<bool>>,
    advice_equalities: Vec<(ContextCell, ContextCell)>,
    constant_equalities: Vec<(F, ContextCell)>,
    lookup_cells: Vec<(usize, ContextTag, ContextCell)>,
    instance_cells: Vec<Vec<ContextCell>>,
}

fn stream_shape<F: KagemushaPoseidonFieldV1>(
    builder: &BaseCircuitBuilder<F>,
) -> ByteStreamShape<F> {
    let params = &builder.config_params;
    let mut advice_rows = Vec::new();
    let mut selectors = Vec::new();
    for phase in &builder.core().phase_manager {
        for ctx in &phase.threads {
            advice_rows.push(ctx.advice_len());
            selectors.push(ctx.selector.iter().copied().collect());
        }
    }
    let mut lookup_cells = Vec::new();
    for (phase, manager) in builder.lookup_manager().iter().enumerate() {
        let lookups = manager.cells_to_lookup.lock().expect("lookup cells");
        for (tag, rows) in lookups.iter() {
            for row in rows {
                lookup_cells.push((phase, *tag, row[0].cell.expect("lookup position")));
            }
        }
    }
    let copies = builder
        .core()
        .copy_manager
        .lock()
        .expect("copy constraints");
    ByteStreamShape {
        k: params.k,
        advice_columns: params.num_advice_per_phase.clone(),
        fixed_columns: params.num_fixed,
        lookup_columns: params.num_lookup_advice_per_phase.clone(),
        lookup_bits: params.lookup_bits,
        instance_columns: params.num_instance_columns,
        advice_rows,
        selectors,
        advice_equalities: copies.advice_equalities.clone(),
        constant_equalities: copies
            .constant_equalities
            .iter()
            .map(|(value, cell)| (*value, *cell))
            .collect(),
        lookup_cells,
        instance_cells: builder
            .assigned_instances
            .iter()
            .map(|column| {
                column
                    .iter()
                    .map(|value| value.cell.expect("instance position"))
                    .collect()
            })
            .collect(),
    }
}

fn assert_fixed_shape<F: KagemushaPoseidonFieldV1>() {
    let baseline = stream_shape(&stream_builder::<F>(
        &[0; 17],
        F::ZERO,
        &[0; 9],
        F::ZERO,
        26,
    ));
    for left_len in [0, 1, 7, 8, 9, 15, 16, 17] {
        for right_len in [0, 1, 8, 9] {
            let left = patterned_prefix(17, left_len, 0x21);
            let right = patterned_prefix(9, right_len, 0x91);
            let builder = stream_builder::<F>(
                &left,
                F::from(left_len as u64),
                &right,
                F::from(right_len as u64),
                26,
            );
            assert_eq!(
                stream_shape(&builder),
                baseline,
                "left {left_len}, right {right_len}"
            );
            assert_eq!(stream_shape(&builder.deep_clone().unknown(true)), baseline);
        }
    }
    // Invalid witness values must not change fixed columns, selectors, copy/lookup wiring or
    // public mappings either; only constraint satisfaction is allowed to differ.
    let invalid = stream_builder::<F>(&[0xff; 17], -F::ONE, &[0xff; 9], F::from(10), 26);
    assert_eq!(stream_shape(&invalid), baseline);
}

#[test]
fn bounded_stream_shape_is_independent_of_lengths_and_values_in_both_fields() {
    assert_fixed_shape::<Fp>();
    assert_fixed_shape::<Fq>();
}
