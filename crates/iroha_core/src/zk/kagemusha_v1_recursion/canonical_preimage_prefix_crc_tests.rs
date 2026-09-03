//! Exact variable-prefix CRC64-XZ algebra, assigned constraints, and fixed-topology regressions.

use halo2_base::{ContextCell, ContextTag, gates::circuit::builder::BaseCircuitBuilder};
use halo2_proofs::{
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
};

use super::*;
use crate::zk::kagemusha_v1_recursion::guard_bundle::assign_bytes;

const PREFIX_CRC_TEST_K: u32 = 12;
// The independent oracle shifts in the opposite direction with the normal ECMA polynomial.
const NORMAL_CRC64_POLYNOMIAL: u64 = 0x42F0_E1EB_A9EA_3693;

fn normal_zero_byte_transition(reflected_state: u64) -> u64 {
    let mut state = reflected_state.reverse_bits();
    for _ in 0..8 {
        let high_bit = state & (1_u64 << 63) != 0;
        state <<= 1;
        if high_bit {
            state ^= NORMAL_CRC64_POLYNOMIAL;
        }
    }
    state.reverse_bits()
}

fn independent_crc64_xz(payload: &[u8]) -> u64 {
    let mut state = u64::MAX;
    for byte in payload {
        state ^= u64::from(byte.reverse_bits()) << 56;
        for _ in 0..8 {
            let high_bit = state & (1_u64 << 63) != 0;
            state <<= 1;
            if high_bit {
                state ^= NORMAL_CRC64_POLYNOMIAL;
            }
        }
    }
    !state.reverse_bits()
}

fn unpad_checksum_native(checksum: u64, mut padding_len: usize) -> u64 {
    let mut state = !checksum;
    let mut inverse_power = crc64_inverse_zero_byte_matrix_v1();
    while padding_len != 0 {
        if padding_len & 1 != 0 {
            state = crc64_apply_matrix_native_v1(&inverse_power, state);
        }
        inverse_power = crc64_square_matrix_v1(&inverse_power);
        padding_len >>= 1;
    }
    !state
}

fn patterned_payload(capacity: usize, actual_len: usize) -> Vec<u8> {
    assert!(actual_len <= capacity);
    (0..capacity)
        .map(|index| {
            if index < actual_len {
                (index.wrapping_mul(37).wrapping_add(11) & 0xff) as u8
            } else {
                0
            }
        })
        .collect()
}

fn prefix_crc_builder<F: KagemushaPoseidonFieldV1>(
    payload: &[u8],
    actual_len: F,
) -> BaseCircuitBuilder<F> {
    let mut builder = BaseCircuitBuilder::<F>::default()
        .use_k(PREFIX_CRC_TEST_K as usize)
        .use_lookup_bits(PREFIX_CRC_TEST_K as usize - 1)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let bytes = assign_bytes(ctx, &range, payload);
    let assigned_len = ctx.load_witness(actual_len);
    let checksum =
        crc64_xz_prefix_bytes_v1(ctx, &range, &bytes, assigned_len).expect("prefix CRC relation");
    let mut instances = vec![assigned_len];
    instances.extend(checksum.map(|byte| byte.assigned().expect("constrained checksum byte")));
    builder.assigned_instances = vec![instances];
    builder.calculate_params(Some(9));
    builder
}

fn prefix_crc_case<F: KagemushaPoseidonFieldV1>(
    payload: &[u8],
    actual_len: F,
    public_len: F,
    expected_crc: u64,
) -> bool {
    let builder = prefix_crc_builder(payload, actual_len);
    let mut instances = vec![public_len];
    instances.extend(
        expected_crc
            .to_le_bytes()
            .into_iter()
            .map(|byte| F::from(u64::from(byte))),
    );
    MockProver::run(PREFIX_CRC_TEST_K, &builder, vec![instances])
        .expect("prefix CRC mock prover")
        .verify()
        .is_ok()
}

#[test]
fn prefix_crc_inverse_matrix_is_two_sided_on_every_basis_vector() {
    assert_eq!(
        NORMAL_CRC64_POLYNOMIAL.reverse_bits(),
        REFLECTED_CRC64_POLYNOMIAL
    );
    assert_eq!(independent_crc64_xz(b"123456789"), 0x995d_c9bb_df19_39fa);
    assert_eq!(independent_crc64_xz(&[]), 0);
    let inverse = crc64_inverse_zero_byte_matrix_v1();
    // Equality on the entire GF(2) basis proves the inverse identities for all 64-bit states,
    // not merely states reachable from the selected CRC test messages.
    for bit in 0..64 {
        let state = 1_u64 << bit;
        assert_eq!(
            crc64_apply_matrix_native_v1(&inverse, normal_zero_byte_transition(state)),
            state,
            "inverse after forward, basis {bit}",
        );
        assert_eq!(
            normal_zero_byte_transition(crc64_apply_matrix_native_v1(&inverse, state)),
            state,
            "forward after inverse, basis {bit}",
        );
    }
}

#[test]
fn prefix_crc_inverse_binary_powers_match_independent_serial_transition() {
    let mut inverse_power = crc64_inverse_zero_byte_matrix_v1();
    for exponent in 0..=9 {
        for bit in 0..64 {
            let original = 1_u64 << bit;
            let mut padded = original;
            for _ in 0..(1_usize << exponent) {
                padded = normal_zero_byte_transition(padded);
            }
            assert_eq!(
                crc64_apply_matrix_native_v1(&inverse_power, padded),
                original,
                "binary power {exponent}, basis {bit}",
            );
        }
        inverse_power = crc64_square_matrix_v1(&inverse_power);
    }
}

#[test]
fn prefix_crc_native_unpadding_matches_portable_norito_at_all_selected_lengths() {
    for capacity in [
        0, 1, 2, 3, 7, 8, 9, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257,
    ] {
        for actual_len in 0..=capacity {
            let payload = patterned_payload(capacity, actual_len);
            let checksum = norito::crc64_fallback(&payload);
            let expected = norito::crc64_fallback(&payload[..actual_len]);
            assert_eq!(independent_crc64_xz(&payload), checksum);
            assert_eq!(independent_crc64_xz(&payload[..actual_len]), expected);
            assert_eq!(
                unpad_checksum_native(checksum, capacity - actual_len),
                expected,
                "capacity {capacity}, actual length {actual_len}",
            );
        }
    }
}

fn assert_prefix_crc_boundaries<F: KagemushaPoseidonFieldV1>() {
    for (capacity, lengths) in [
        (0, vec![0]),
        (1, vec![0, 1]),
        (33, vec![0, 1, 7, 8, 9, 15, 16, 17, 31, 32, 33]),
    ] {
        for actual_len in lengths {
            let payload = patterned_payload(capacity, actual_len);
            let expected = norito::crc64_fallback(&payload[..actual_len]);
            let length = F::from(actual_len as u64);
            assert!(
                prefix_crc_case::<F>(&payload, length, length, expected),
                "capacity {capacity}, actual length {actual_len}, Eq parity {}",
                F::IS_EQ_PARITY,
            );
        }
    }
}

#[test]
fn prefix_crc_matches_portable_norito_in_both_pasta_fields() {
    assert_prefix_crc_boundaries::<Fp>();
    assert_prefix_crc_boundaries::<Fq>();
}

fn assert_prefix_crc_mixed_constant_bytes<F: KagemushaPoseidonFieldV1>() {
    for actual_len in [3, 9] {
        let payload = patterned_payload(9, actual_len);
        let mut builder = BaseCircuitBuilder::<F>::default()
            .use_k(PREFIX_CRC_TEST_K as usize)
            .use_lookup_bits(PREFIX_CRC_TEST_K as usize - 1)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let bytes = payload
            .iter()
            .enumerate()
            .map(|(index, byte)| {
                if index % 2 == 0 {
                    PastaSha256ByteV1::constant(*byte)
                } else {
                    let assigned = ctx.load_witness(F::from(u64::from(*byte)));
                    PastaSha256ByteV1::range_checked(ctx, &range, assigned)
                }
            })
            .collect::<Vec<_>>();
        let length = ctx.load_witness(F::from(actual_len as u64));
        let output = crc64_xz_prefix_bytes_v1(ctx, &range, &bytes, length)
            .expect("mixed constant/assigned prefix CRC");
        builder.assigned_instances = vec![
            output
                .map(|byte| byte.assigned().expect("constrained CRC byte"))
                .to_vec(),
        ];
        builder.calculate_params(Some(9));
        let expected = norito::crc64_fallback(&payload[..actual_len])
            .to_le_bytes()
            .into_iter()
            .map(|byte| F::from(u64::from(byte)))
            .collect();
        MockProver::run(PREFIX_CRC_TEST_K, &builder, vec![expected])
            .expect("mixed prefix CRC mock prover")
            .assert_satisfied();
    }
}

#[test]
fn prefix_crc_includes_fixed_byte_affine_terms_in_both_pasta_fields() {
    assert_prefix_crc_mixed_constant_bytes::<Fp>();
    assert_prefix_crc_mixed_constant_bytes::<Fq>();
}

fn assert_prefix_crc_rejects_wrong_length<F: KagemushaPoseidonFieldV1>() {
    let mut payload = vec![0; 17];
    payload[..3].copy_from_slice(b"abc");
    let expected = norito::crc64_fallback(b"abc");
    assert_ne!(expected, norito::crc64_fallback(b"abc\0"));
    // A public length cannot be relabelled without changing the assigned length.
    assert!(!prefix_crc_case(&payload, F::from(3), F::from(4), expected));
    // Even with valid zero padding and matching public/assigned lengths, the fourth zero byte
    // is part of the active message when L=4 and cannot retain CRC("abc").
    assert!(!prefix_crc_case(&payload, F::from(4), F::from(4), expected));
    for length in [
        F::from(18),
        F::from(31),
        F::from(32),
        F::from(u64::MAX),
        -F::ONE,
    ] {
        assert!(!prefix_crc_case(&payload, length, length, expected));
    }
    for length in [F::ONE, -F::ONE] {
        assert!(!prefix_crc_case(&[], length, length, 0));
    }
}

#[test]
fn prefix_crc_rejects_wrong_out_of_range_and_negative_lengths_in_both_fields() {
    assert_prefix_crc_rejects_wrong_length::<Fp>();
    assert_prefix_crc_rejects_wrong_length::<Fq>();
}

fn assert_prefix_crc_rejects_wrong_checksum<F: KagemushaPoseidonFieldV1>() {
    for actual_len in [0, 1, 8, 17] {
        let payload = patterned_payload(17, actual_len);
        let expected = norito::crc64_fallback(&payload[..actual_len]);
        let length = F::from(actual_len as u64);
        assert!(!prefix_crc_case(
            &payload,
            length,
            length,
            expected ^ (1_u64 << (actual_len % 64))
        ));
    }
}

#[test]
fn prefix_crc_rejects_substituted_checksums_in_both_pasta_fields() {
    assert_prefix_crc_rejects_wrong_checksum::<Fp>();
    assert_prefix_crc_rejects_wrong_checksum::<Fq>();
}

fn assert_prefix_crc_rejects_nonzero_tail<F: KagemushaPoseidonFieldV1>() {
    for index in 3..9 {
        let mut payload = patterned_payload(9, 3);
        payload[index] = 0x80;
        // Supply the output of the inverse algorithm for this malformed payload. Rejection
        // must come from the tail constraint, not merely from a mismatching expected checksum.
        let computed = unpad_checksum_native(norito::crc64_fallback(&payload), 6);
        assert!(!prefix_crc_case(&payload, F::from(3), F::from(3), computed));
    }
    let payload = vec![1; 9];
    let computed = unpad_checksum_native(norito::crc64_fallback(&payload), 9);
    assert!(!prefix_crc_case(&payload, F::ZERO, F::ZERO, computed));
}

#[test]
fn prefix_crc_rejects_every_nonzero_tail_position_in_both_pasta_fields() {
    assert_prefix_crc_rejects_nonzero_tail::<Fp>();
    assert_prefix_crc_rejects_nonzero_tail::<Fq>();
}

#[derive(Debug, PartialEq, Eq)]
struct PrefixCrcShape<F: KagemushaPoseidonFieldV1> {
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

fn prefix_crc_shape<F: KagemushaPoseidonFieldV1>(
    builder: &BaseCircuitBuilder<F>,
) -> PrefixCrcShape<F> {
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
    PrefixCrcShape {
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

fn assert_prefix_crc_fixed_shape<F: KagemushaPoseidonFieldV1>() {
    let baseline = prefix_crc_shape(&prefix_crc_builder::<F>(&[0; 33], F::ZERO));
    for actual_len in [1, 7, 8, 9, 15, 16, 17, 31, 32, 33] {
        let payload = patterned_payload(33, actual_len);
        let builder = prefix_crc_builder(&payload, F::from(actual_len as u64));
        assert_eq!(
            prefix_crc_shape(&builder),
            baseline,
            "active length {actual_len}"
        );
    }
    // Include an invalid witness: even a length that fails constraints cannot alter selectors,
    // fixed values, equality wiring, lookup locations, or the public-column mapping.
    let invalid = prefix_crc_builder::<F>(&[0xff; 33], F::from(34));
    assert_eq!(prefix_crc_shape(&invalid), baseline);
}

#[test]
fn prefix_crc_shape_is_independent_of_length_and_values_in_both_pasta_fields() {
    assert_prefix_crc_fixed_shape::<Fp>();
    assert_prefix_crc_fixed_shape::<Fq>();
}
