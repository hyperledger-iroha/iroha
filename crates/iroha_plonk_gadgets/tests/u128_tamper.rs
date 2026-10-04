//! The running-sum range check and checked unsigned arithmetic:
//!
//! - range checks at every width class, both limb shapes, boundaries
//!   `2^bits - 1` (accepted) and `2^bits` (rejected by a lookup);
//! - the M8 `U128Circuit` chain with the `m8_custom_gate_checks` cases (an
//!   addition giving exactly `2^128`, a subtraction below zero, a wrong
//!   sum);
//! - the M7 `u64` policy-epoch and accepted-time windows with their
//!   rejections;
//! - comparisons, constants and nonzero checks against the native
//!   references;
//! - the per-cell tamper suite and the rows-per-check inventory.

mod common;

use common::{
    Chips, GLUE_COLUMNS, GadgetCircuit, Inputs, RANGE_COLUMN, Shape, accepts, assigned, count,
    extent, report,
};
use ff::Field as _;
use iroha_pasta::{Fp, Fq, poseidon::PoseidonField};
use iroha_plonk::{
    check::{CheckFailure, CheckMode, check_circuit},
    frontend::{Error, Region, synthesize},
};
use iroha_plonk_gadgets::{
    LimbBits, UintChip, Word,
    cells::low_u128,
    range::{RangeShape, checked_add_native, checked_sub_native, lt_native},
    tamper::{Tamper, check_tampered, undetected_tampers},
};

/// `2^bits` in the field.
fn power_of_two<F: PoseidonField>(bits: u64) -> F {
    F::from(2u64).pow_vartime([bits, 0, 0, 0])
}

/// Whether every failure of the strict check is a missing lookup input
/// (the range table rejected a value), and there is at least one.
fn only_range_failures<F: PoseidonField>(circuit: &GadgetCircuit<F>, k: u32, public: &[F]) -> bool {
    let report = check_circuit(circuit, k, &[public.to_vec()], CheckMode::Strict).expect("check");
    !report.is_satisfied()
        && report
            .failures()
            .iter()
            .all(|failure| matches!(failure, CheckFailure::LookupInputMissing { .. }))
}

/// Range-checks every input to `arg 0` bits; even inputs are new
/// witnesses, odd ones are copies of glue witnesses.
fn range_checks<F: PoseidonField>(
    chips: &mut Chips<F>,
    region: &mut Region<'_, F>,
    inputs: &Inputs<F>,
) -> Result<Vec<Word<F>>, Error> {
    let bits = usize::try_from(inputs.arg(0)).map_err(|_| Error::Synthesis)?;
    let mut out = Vec::new();
    for (index, value) in inputs.all().into_iter().enumerate() {
        if index % 2 == 0 {
            out.push(chips.range.witness_range_checked(region, value, bits)?);
        } else {
            let word = chips.glue.witness(region, value)?;
            chips.range.range_check(region, &word, bits)?;
            out.push(word);
        }
    }
    Ok(out)
}

fn range_circuit<F: PoseidonField>(
    limb_bits: usize,
    bits: u64,
    values: Vec<F>,
) -> GadgetCircuit<F> {
    let shape = Shape::new(0, limb_bits, values.len()).with_args(&[bits]);
    GadgetCircuit::new(shape, range_checks::<F>, values)
}

fn range_boundaries<F: PoseidonField>() {
    let k = 9;
    for limb_bits in [4, 7] {
        for bits in [1_u64, 3, 7, 8, 9, 64, 127, 128, 200, 252] {
            let limit = power_of_two::<F>(bits);
            let ok = vec![F::ZERO, limit - F::ONE, F::ONE, limit - F::ONE];
            let circuit = range_circuit(limb_bits, bits, ok.clone());
            assert!(
                accepts(&circuit, k, &ok),
                "b {limb_bits} bits {bits}: {}",
                report(&circuit, k, &ok)
            );
            for bad in [limit, limit + F::ONE, -F::ONE] {
                for position in [0, 1] {
                    let mut values = ok.clone();
                    values[position] = bad;
                    let circuit = range_circuit(limb_bits, bits, values.clone());
                    assert!(
                        only_range_failures(&circuit, k, &values),
                        "b {limb_bits} bits {bits} position {position}"
                    );
                }
            }
        }
    }
}

#[test]
fn range_checks_accept_exactly_the_range() {
    range_boundaries::<Fp>();
    range_boundaries::<Fq>();
}

#[test]
fn range_check_widths_outside_1_to_252_are_errors() {
    for bits in [0, 253] {
        let circuit = range_circuit(4, bits, vec![Fp::ZERO]);
        assert_eq!(
            synthesize(&circuit, 8, Some(&[vec![Fp::ZERO]][..])).map(|_| ()),
            Err(Error::Synthesis)
        );
    }
}

#[test]
fn range_inventory_matches_the_shapes() {
    let k = 9;
    for (limb_bits, bits) in [(4_usize, 128_u64), (7, 128), (7, 64), (4, 9), (7, 252)] {
        let shape = RangeShape::new(
            usize::try_from(bits).expect("bits"),
            LimbBits::new(limb_bits).expect("limb bits"),
        )
        .expect("shape");
        let values = vec![Fp::from(5u64); 3];
        let circuit = range_circuit(limb_bits, bits, values.clone());
        let flags = assigned(&circuit, k, &values);
        assert_eq!(
            extent(&flags[RANGE_COLUMN]),
            3 * shape.rows,
            "b {limb_bits} bits {bits}"
        );
        assert_eq!(count(&flags[RANGE_COLUMN]), 3 * shape.rows);
    }
    // M8 inventory: 128 bits in 10 rows at b = 15 and 16 rows at b = 8.
    for (limb_bits, rows) in [(15, 10), (12, 12), (10, 14), (8, 16)] {
        let shape = RangeShape::new(128, LimbBits::new(limb_bits).expect("b")).expect("shape");
        assert_eq!(shape.rows, rows);
    }
}

#[test]
fn every_range_cell_is_pinned() {
    for bits in [9_u64, 128] {
        let values = vec![
            Fq::from(300u64),
            Fq::from(77u64),
            Fq::ZERO,
            Fq::from(511u64),
        ];
        let circuit = range_circuit(4, bits, values.clone());
        assert_eq!(undetected_tampers(&circuit, 9, &[values]), Ok(Vec::new()));
    }
}

/// The M8 lane: `acc + x0 - x1 + x2 - x3`, every value a range-checked
/// `u128`.
fn m8_chain<F: PoseidonField>(
    chips: &mut Chips<F>,
    region: &mut Region<'_, F>,
    inputs: &Inputs<F>,
) -> Result<Vec<Word<F>>, Error> {
    let mut uint = UintChip::new(&mut chips.glue, &mut chips.range);
    let mut acc = uint.assign_u128(region, inputs.get(0).map(|v| low_u128(&v)))?;
    for index in 1..inputs.len() {
        let x = uint.assign_u128(region, inputs.get(index).map(|v| low_u128(&v)))?;
        acc = if index % 2 == 1 {
            uint.checked_add(region, &acc, &x)?
        } else {
            uint.checked_sub(region, &acc, &x)?
        };
    }
    Ok(vec![acc.word().clone()])
}

/// The field value of the chain (equal to the integer chain when no step
/// leaves `[0, 2^128)`).
fn chain_value<F: PoseidonField>(values: &[u128]) -> F {
    values
        .iter()
        .enumerate()
        .fold(F::ZERO, |acc, (index, value)| {
            let value = F::from_u128(*value);
            if index == 0 || index % 2 == 1 {
                acc + value
            } else {
                acc - value
            }
        })
}

fn chain_circuit<F: PoseidonField>(values: &[u128]) -> (GadgetCircuit<F>, Vec<F>) {
    let inputs = values.iter().map(|value| F::from_u128(*value)).collect();
    let circuit = GadgetCircuit::new(Shape::new(0, 7, 1), m8_chain::<F>, inputs);
    (circuit, vec![chain_value(values)])
}

const CHAIN_K: u32 = 9;

fn m8_cases<F: PoseidonField>() {
    let start = (1_u128 << 127) + 0x1234_5678;
    let honest = [start, 1 << 99, 3 << 98, 77, 1 << 90];
    let (circuit, public) = chain_circuit::<F>(&honest);
    assert!(
        accepts(&circuit, CHAIN_K, &public),
        "{}",
        report(&circuit, CHAIN_K, &public)
    );
    // Native references agree with the honest chain.
    let mut acc = start;
    for (index, value) in honest.iter().enumerate().skip(1) {
        acc = if index % 2 == 1 {
            checked_add_native(128, acc, *value)
        } else {
            checked_sub_native(128, acc, *value)
        }
        .expect("honest step");
    }
    assert_eq!(public[0], F::from_u128(acc));
    // An addition giving exactly 2^128.
    let mut overflow = honest;
    overflow[1] = u128::MAX - start + 1;
    assert_eq!(checked_add_native(128, start, overflow[1]), None);
    let (circuit, public) = chain_circuit::<F>(&overflow);
    assert!(only_range_failures(&circuit, CHAIN_K, &public), "overflow");
    // A subtraction below zero.
    let mut underflow = honest;
    underflow[2] = start + honest[1] + 1;
    assert_eq!(
        checked_sub_native(128, start + honest[1], underflow[2]),
        None
    );
    let (circuit, public) = chain_circuit::<F>(&underflow);
    assert!(only_range_failures(&circuit, CHAIN_K, &public), "underflow");
    // A wrong sum: the first addition's output (glue column c, row 0) off
    // by one.
    let (circuit, public) = chain_circuit::<F>(&honest);
    let tamper = Tamper {
        column: GLUE_COLUMNS[2],
        row: 0,
        delta: F::ONE,
    };
    let report = check_tampered(&circuit, CHAIN_K, &[public], Some(tamper)).expect("check");
    assert!(!report.is_satisfied(), "wrong sum");
}

#[test]
fn m8_u128_cases() {
    m8_cases::<Fp>();
    m8_cases::<Fq>();
}

#[test]
fn every_u128_chain_cell_is_pinned() {
    let (circuit, public) = chain_circuit::<Fp>(&[1 << 127, 5, 3, u128::MAX >> 2, 9]);
    assert_eq!(
        undetected_tampers(&circuit, CHAIN_K, &[public]),
        Ok(Vec::new())
    );
}

#[test]
fn u128_inventory() {
    // b = 7: a 128-bit check is 19 limbs + 1 shifted row = 20 rows; a
    // checked add/sub is one glue row plus one check.
    let shape = RangeShape::new(128, LimbBits::new(7).expect("b")).expect("shape");
    assert_eq!(shape.rows, 20);
    let (circuit, public) = chain_circuit::<Fp>(&[1 << 127, 5, 3, 7, 9]);
    let flags = assigned(&circuit, CHAIN_K, &public);
    assert_eq!(extent(&flags[RANGE_COLUMN]), (5 + 4) * shape.rows);
    assert_eq!(extent(&flags[GLUE_COLUMNS[0]]), 4);
    assert_eq!(count(&flags[GLUE_COLUMNS[3]]), 0);
}

/// The M7 `sigma_send` windows on `u64`s: Request epoch <= policy epoch,
/// floor <= lower, Request time <= lower, lower <= upper; then `[Request
/// time < lower]` and `sequence + 1` (u128).
fn m7_windows<F: PoseidonField>(
    chips: &mut Chips<F>,
    region: &mut Region<'_, F>,
    inputs: &Inputs<F>,
) -> Result<Vec<Word<F>>, Error> {
    let mut uint = UintChip::new(&mut chips.glue, &mut chips.range);
    // The policy epoch through the generic width (so a value of 2^64 can be
    // laid out and rejected), the others as `u64`s.
    let mut words = vec![uint.assign::<64>(region, inputs.get(0).map(|v| low_u128(&v)))?];
    for index in 1..6 {
        let value = inputs
            .get(index)
            .map(|v| u64::try_from(low_u128(&v)).unwrap_or(u64::MAX));
        words.push(uint.assign_u64(region, value)?);
    }
    let [policy, request, floor, time, lower, upper] = [0, 1, 2, 3, 4, 5].map(|i| &words[i]);
    uint.assert_le(region, request, policy)?;
    uint.assert_le(region, floor, lower)?;
    uint.assert_le(region, time, lower)?;
    uint.assert_le(region, lower, upper)?;
    let early = uint.lt(region, time, lower)?;
    let sequence = uint.assign_u128(region, inputs.get(6).map(|v| low_u128(&v)))?;
    let next = uint.checked_add_constant(region, &sequence, 1)?;
    Ok(vec![early.word().clone(), next.word().clone()])
}

fn windows_circuit<F: PoseidonField>(values: [u128; 7]) -> (GadgetCircuit<F>, Vec<F>) {
    let inputs = values.iter().map(|value| F::from_u128(*value)).collect();
    let early = lt_native(64, values[3], values[4]).unwrap_or(false);
    let public = vec![
        if early { F::ONE } else { F::ZERO },
        F::from_u128(values[6]) + F::ONE,
    ];
    (
        GadgetCircuit::new(Shape::new(0, 7, 2), m7_windows::<F>, inputs),
        public,
    )
}

fn m7_window_cases<F: PoseidonField>() {
    let honest = [900, 850, 100, 400, 400, 600_400, 41];
    let (circuit, public) = windows_circuit::<F>(honest);
    assert!(
        accepts(&circuit, CHAIN_K, &public),
        "{}",
        report(&circuit, CHAIN_K, &public)
    );
    let mut later = honest;
    later[4] = 401;
    let (circuit, public) = windows_circuit::<F>(later);
    assert_eq!(public[0], F::ONE);
    assert!(accepts(&circuit, CHAIN_K, &public));
    let rejected = [
        ("stale epoch", 1, 901),
        ("early time", 4, 99),
        ("upper below lower", 5, 399),
        ("u64 overflow", 0, 1 << 64),
        ("sequence overflow", 6, u128::MAX),
    ];
    for (label, index, value) in rejected {
        let mut values = honest;
        values[index] = value;
        if index == 4 {
            values[3] = 50;
        }
        let (circuit, public) = windows_circuit::<F>(values);
        assert!(only_range_failures(&circuit, CHAIN_K, &public), "{label}");
    }
}

#[test]
fn m7_u64_windows() {
    m7_window_cases::<Fp>();
    m7_window_cases::<Fq>();
}

#[test]
fn every_window_cell_is_pinned() {
    let (circuit, public) = windows_circuit::<Fq>([900, 850, 100, 400, 401, 600_400, 41]);
    assert_eq!(
        undetected_tampers(&circuit, CHAIN_K, &[public]),
        Ok(Vec::new())
    );
}

/// `assert_lt`, both `lt` outcomes, `assert_nonzero` and constants.
fn comparisons<F: PoseidonField>(
    chips: &mut Chips<F>,
    region: &mut Region<'_, F>,
    inputs: &Inputs<F>,
) -> Result<Vec<Word<F>>, Error> {
    let mut uint = UintChip::new(&mut chips.glue, &mut chips.range);
    let a = uint.assign_u128(region, inputs.get(0).map(|v| low_u128(&v)))?;
    let b = uint.assign_u128(region, inputs.get(1).map(|v| low_u128(&v)))?;
    let limit = uint.constant::<128>(region, u128::MAX)?;
    uint.assert_lt(region, &a, &b)?;
    uint.assert_le(region, &b, &limit)?;
    uint.assert_nonzero(region, &b)?;
    let less = uint.lt(region, &a, &b)?;
    let greater = uint.lt(region, &b, &a)?;
    let equal = uint.lt(region, &a, &a)?;
    let wide = UintChip::<F>::widen::<128, 128>(&a);
    Ok(vec![
        less.word().clone(),
        greater.word().clone(),
        equal.word().clone(),
        wide.word().clone(),
    ])
}

fn comparison_circuit<F: PoseidonField>(a: u128, b: u128) -> (GadgetCircuit<F>, Vec<F>) {
    let bit = |value: Option<bool>| if value == Some(true) { F::ONE } else { F::ZERO };
    let public = vec![
        bit(lt_native(128, a, b)),
        bit(lt_native(128, b, a)),
        F::ZERO,
        F::from_u128(a),
    ];
    let circuit = GadgetCircuit::new(
        Shape::new(0, 7, 4),
        comparisons::<F>,
        vec![F::from_u128(a), F::from_u128(b)],
    );
    (circuit, public)
}

#[test]
fn comparisons_match_the_native_references() {
    let k = 8;
    for (a, b) in [(0, 1), (5, u128::MAX), (u128::MAX - 1, u128::MAX)] {
        let (circuit, public) = comparison_circuit::<Fp>(a, b);
        assert!(
            accepts(&circuit, k, &public),
            "{}",
            report(&circuit, k, &public)
        );
    }
    // a = b fails assert_lt; b = 0 fails assert_nonzero as well.
    let (circuit, public) = comparison_circuit::<Fq>(7, 7);
    assert!(only_range_failures(&circuit, k, &public));
    let (circuit, public) = comparison_circuit::<Fq>(0, 0);
    assert!(!accepts(&circuit, k, &public));
    let (circuit, public) = comparison_circuit::<Fp>(3, 9);
    assert_eq!(undetected_tampers(&circuit, k, &[public]), Ok(Vec::new()));
}

/// A constant wider than its type.
fn wide_constant<F: PoseidonField>(
    chips: &mut Chips<F>,
    region: &mut Region<'_, F>,
    _inputs: &Inputs<F>,
) -> Result<Vec<Word<F>>, Error> {
    let mut uint = UintChip::new(&mut chips.glue, &mut chips.range);
    let constant = uint.constant::<64>(region, 1 << 64)?;
    Ok(vec![constant.word().clone()])
}

#[test]
fn constants_wider_than_their_type_are_errors() {
    let circuit = GadgetCircuit::new(Shape::new(0, 7, 1), wide_constant::<Fp>, Vec::new());
    assert_eq!(
        synthesize(&circuit, 8, Some(&[vec![Fp::ZERO]][..])).map(|_| ()),
        Err(Error::Synthesis)
    );
}
