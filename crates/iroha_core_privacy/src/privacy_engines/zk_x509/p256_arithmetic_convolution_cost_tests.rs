//! Complete original integrated arithmetic parity and balanced cost.

use super::*;

fn original_integrated_v1<A: PolynomialAirFieldV1>(
    current: &[A; P256_ARITHMETIC_BASE_WIDTH_V1],
    next: &[A; P256_ARITHMETIC_BASE_WIDTH_V1],
    current_aux: &[A; P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1],
    next_aux: &[A; P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1],
    fixed: &[A; P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1],
    scalar_challenges: P256ScalarBitBusChallengesV1,
    arithmetic_copy_challenges: P256ArithmeticCopyChallengesV1,
) -> Result<Vec<A>, P256AggregateAdapterErrorV1> {
    let current_native: &[A; P256_ARITHMETIC_STARK_AUX_WIDTH_V1] = current_aux
        [..P256_ARITHMETIC_STARK_AUX_WIDTH_V1]
        .try_into()
        .map_err(|_| P256AggregateAdapterErrorV1::Topology)?;
    let next_native: &[A; P256_ARITHMETIC_STARK_AUX_WIDTH_V1] = next_aux
        [..P256_ARITHMETIC_STARK_AUX_WIDTH_V1]
        .try_into()
        .map_err(|_| P256AggregateAdapterErrorV1::Topology)?;
    let native_fixed: &[A; P256_ARITHMETIC_STARK_FIXED_WIDTH_V1] = fixed
        [..P256_ARITHMETIC_STARK_FIXED_WIDTH_V1]
        .try_into()
        .map_err(|_| P256AggregateAdapterErrorV1::Topology)?;
    let mut residues = super::super::p256_air::arithmetic_residues_original_for_test_v1(
        current,
        next,
        current_native,
        next_native,
        native_fixed,
    )?;
    let events: [P256ScalarSourceEventFixedV1<A>; 8] = core::array::from_fn(|slot| {
        let start = ARITHMETIC_SCALAR_FIXED + slot * SCALAR_EVENT_FIXED_WIDTH;
        decode_scalar_event_v1(&fixed[start..start + SCALAR_EVENT_FIXED_WIDTH])
    });
    let boundary = decode_boundary_v1(
        &fixed[ARITHMETIC_BOUNDARY_FIXED..ARITHMETIC_BOUNDARY_FIXED + CROSS_BOUNDARY_FIXED_WIDTH],
    );
    let sources = p256_arithmetic_opened_scalar_source_bits_v1(current, native_fixed);
    residues.extend(evaluate_compact_scalar_residues_v1(
        &events,
        &sources,
        boundary,
        &current_aux[ARITHMETIC_SCALAR_AUX..ARITHMETIC_VALUE_COPY_AUX],
        &next_aux[ARITHMETIC_SCALAR_AUX..ARITHMETIC_VALUE_COPY_AUX],
        scalar_challenges,
    )?);
    let copy_events: [P256ArithmeticCopyEventFixedV1<A>; 3] = core::array::from_fn(|slot| {
        let start = ARITHMETIC_VALUE_COPY_FIXED + slot * ARITHMETIC_COPY_EVENT_FIXED_WIDTH;
        decode_arithmetic_copy_event_v1(&fixed[start..start + ARITHMETIC_COPY_EVENT_FIXED_WIDTH])
    });
    let copy_boundary = decode_boundary_v1(&fixed[ARITHMETIC_VALUE_COPY_BOUNDARY_FIXED..]);
    let copy_sources = p256_arithmetic_opened_operand_limbs_v1(current, native_fixed);
    residues.extend(evaluate_compact_arithmetic_copy_residues_v1(
        &copy_events,
        &copy_sources,
        copy_boundary,
        &current_aux[ARITHMETIC_VALUE_COPY_AUX..],
        &next_aux[ARITHMETIC_VALUE_COPY_AUX..],
        arithmetic_copy_challenges,
    )?);
    if residues.len() != P256_ARITHMETIC_AGGREGATE_CONSTRAINT_COUNT_V1 {
        return Err(P256AggregateAdapterErrorV1::Topology);
    }
    Ok(residues)
}

use crate::privacy_engines::transparent_stark::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1 as E};
use std::time::Instant;
use zeroize::Zeroize as _;

struct RowV1 {
    current: [F; P256_ARITHMETIC_BASE_WIDTH_V1],
    next: [F; P256_ARITHMETIC_BASE_WIDTH_V1],
    current_aux: [F; P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1],
    next_aux: [F; P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1],
    fixed: [F; P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1],
}
impl Drop for RowV1 {
    fn drop(&mut self) {
        self.current.zeroize();
        self.next.zeroize();
        self.current_aux.zeroize();
        self.next_aux.zeroize();
        self.fixed.zeroize();
    }
}
fn values_v1<const N: usize>(seed: u64) -> [F; N] {
    let mut state = seed + 1;
    core::array::from_fn(|index| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        F(if index % 13 == 0 {
            GOLDILOCKS_MODULUS_V1 - 1
        } else {
            state % GOLDILOCKS_MODULUS_V1
        })
    })
}
fn row_v1(seed: u64) -> RowV1 {
    RowV1 {
        current: values_v1(seed * 5),
        next: values_v1(seed * 5 + 1),
        current_aux: values_v1(seed * 5 + 2),
        next_aux: values_v1(seed * 5 + 3),
        fixed: values_v1(seed * 5 + 4),
    }
}
fn challenges_v1() -> (P256ScalarBitBusChallengesV1, P256ArithmeticCopyChallengesV1) {
    let scalar = P256ScalarBitBusChallengesV1 {
        lanes: core::array::from_fn(|lane| {
            super::super::p256_scalar_bit_bus::P256ScalarBitBusLaneChallengesV1 {
                terms: core::array::from_fn(|term| F((1 + lane * 17 + term) as u64)),
            }
        }),
    };
    let copy = P256ArithmeticCopyChallengesV1 {
        lanes: core::array::from_fn(|lane| P256ArithmeticCopyLaneChallengesV1 {
            terms: core::array::from_fn(|term| F((101 + lane * 17 + term) as u64)),
        }),
    };
    scalar.validate_v1().unwrap();
    copy.validate_v1().unwrap();
    (scalar, copy)
}
fn evaluate_v1(row: &RowV1, wide: bool) -> Result<Vec<F>, P256AggregateAdapterErrorV1> {
    let (scalar, copy) = challenges_v1();
    if wide {
        evaluate_p256_arithmetic_aggregate_residues_over_field_v1(
            &row.current,
            &row.next,
            &row.current_aux,
            &row.next_aux,
            &row.fixed,
            scalar,
            copy,
        )
    } else {
        original_integrated_v1(
            &row.current,
            &row.next,
            &row.current_aux,
            &row.next_aux,
            &row.fixed,
            scalar,
            copy,
        )
    }
}
#[test]
fn arithmetic_convolution_complete_integrated_rows_and_challenge_refusals_match_original() {
    for seed in 0..64 {
        let row = row_v1(seed);
        let expected = zeroize::Zeroizing::new(evaluate_v1(&row, false).unwrap());
        let actual = zeroize::Zeroizing::new(evaluate_v1(&row, true).unwrap());
        assert_eq!(actual.len(), P256_ARITHMETIC_AGGREGATE_CONSTRAINT_COUNT_V1);
        assert_eq!(*actual, *expected, "seed={seed}");
        if seed < 4 {
            let extend =
                |value: F| E::from_coefficients([value, value.add(F::ONE), F(7), F(13)]).unwrap();
            let current = row.current.map(extend);
            let next = row.next.map(extend);
            let current_aux = row.current_aux.map(extend);
            let next_aux = row.next_aux.map(extend);
            let fixed = row.fixed.map(extend);
            let (scalar, copy) = challenges_v1();
            assert_eq!(
                evaluate_p256_arithmetic_aggregate_residues_over_field_v1(
                    &current,
                    &next,
                    &current_aux,
                    &next_aux,
                    &fixed,
                    scalar,
                    copy
                ),
                original_integrated_v1(
                    &current,
                    &next,
                    &current_aux,
                    &next_aux,
                    &fixed,
                    scalar,
                    copy
                )
            );
        }
    }
    let row = row_v1(99);
    for mutation in 0..4 {
        let (mut scalar, mut copy) = challenges_v1();
        match mutation {
            0 => scalar.lanes[0].terms[0] = F::ZERO,
            1 => scalar.lanes[3].terms[4] = scalar.lanes[0].terms[0],
            2 => copy.lanes[0].terms[0] = F(GOLDILOCKS_MODULUS_V1),
            3 => copy.lanes[3].terms[2] = copy.lanes[0].terms[0],
            _ => unreachable!(),
        }
        let expected = original_integrated_v1(
            &row.current,
            &row.next,
            &row.current_aux,
            &row.next_aux,
            &row.fixed,
            scalar,
            copy,
        );
        assert!(expected.is_err());
        assert_eq!(
            evaluate_p256_arithmetic_aggregate_residues_over_field_v1(
                &row.current,
                &row.next,
                &row.current_aux,
                &row.next_aux,
                &row.fixed,
                scalar,
                copy
            ),
            expected
        );
    }
}

#[test]
#[ignore = "balanced complete 455-residue arithmetic AIR cost with exact output erasure; optimized build"]
fn arithmetic_convolution_complete_integrated_air_cost() {
    arithmetic_convolution_complete_integrated_air_cost_with_workers_v1(&[1, 4, 16, 20]);
}

#[test]
#[ignore = "complete arithmetic AIR cost at the current full-prover worker count; optimized build"]
fn arithmetic_convolution_complete_integrated_air_cost_20_workers() {
    arithmetic_convolution_complete_integrated_air_cost_with_workers_v1(&[20]);
}

#[test]
#[ignore = "complete arithmetic AIR cost at sixteen workers; optimized build"]
fn arithmetic_convolution_complete_integrated_air_cost_16_workers() {
    arithmetic_convolution_complete_integrated_air_cost_with_workers_v1(&[16]);
}

fn arithmetic_convolution_complete_integrated_air_cost_with_workers_v1(workers: &[usize]) {
    assert!(!cfg!(debug_assertions));
    const ROWS: usize = 1 << 16;
    let fixtures = (0..512).map(row_v1).collect::<Vec<_>>();
    // Independent full 455-cell parity before timed work for every cached input.
    for row in &fixtures {
        let original = zeroize::Zeroizing::new(evaluate_v1(row, false).unwrap());
        let wide = zeroize::Zeroizing::new(evaluate_v1(row, true).unwrap());
        assert_eq!(*original, *wide);
    }
    let (scalar, copy) = challenges_v1();
    eprintln!(
        "ARITHMETIC_CONVOLUTION_HEADER rows={ROWS} fixture_rows={} fixture_bytes={} constraints={} samples_per_arm=8 timed=complete_integrated_air_plus_full_output_consumption_and_erasure caller=pool_install protocol_unchanged=true full_prover_timing=false",
        fixtures.len(),
        fixtures.len() * core::mem::size_of::<RowV1>(),
        P256_ARITHMETIC_AGGREGATE_CONSTRAINT_COUNT_V1,
    );
    for &workers in workers {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        let run = |wide: bool| {
            let start = Instant::now();
            let results = zeroize::Zeroizing::new(pool.install(|| {
                (0..ROWS)
                    .into_par_iter()
                    .map(|index| {
                        let row = &fixtures[index % fixtures.len()];
                        let result = if wide {
                            evaluate_p256_arithmetic_aggregate_residues_over_field_v1(
                                &row.current,
                                &row.next,
                                &row.current_aux,
                                &row.next_aux,
                                &row.fixed,
                                scalar,
                                copy,
                            )
                        } else {
                            original_integrated_v1(
                                &row.current,
                                &row.next,
                                &row.current_aux,
                                &row.next_aux,
                                &row.fixed,
                                scalar,
                                copy,
                            )
                        }
                        .unwrap();
                        let result = zeroize::Zeroizing::new(result);
                        // Keep all 455 residues live and included in equal cleanup.
                        let value = std::hint::black_box(result.as_slice())
                            .iter()
                            .copied()
                            .fold(F::ZERO, F::add);
                        std::hint::black_box(value)
                    })
                    .collect::<Vec<_>>()
            }));
            let checksum = results.iter().copied().fold(F::ZERO, F::add);
            drop(results);
            (start.elapsed().as_secs_f64(), checksum)
        };
        let expected = run(false).1;
        assert_eq!(run(true).1, expected);
        let mut samples: [Vec<f64>; 2] = core::array::from_fn(|_| Vec::new());
        for round in 0..8 {
            for position in 0..2 {
                let mode = (round + position) % 2;
                let (seconds, checksum) = run(mode == 1);
                assert_eq!(checksum, expected);
                samples[mode].push(seconds);
                eprintln!(
                    "ARITHMETIC_CONVOLUTION_COST workers={workers} rows={ROWS} constraints=455 round={round} mode={mode} seconds={seconds:.9}"
                );
            }
        }
        for (mode, samples) in samples.iter_mut().enumerate() {
            samples.sort_by(f64::total_cmp);
            let median = (samples[3] + samples[4]) / 2.0;
            eprintln!(
                "ARITHMETIC_CONVOLUTION_MEDIAN workers={workers} rows={ROWS} mode={mode} samples=8 seconds={median:.9}"
            );
        }
    }
}
