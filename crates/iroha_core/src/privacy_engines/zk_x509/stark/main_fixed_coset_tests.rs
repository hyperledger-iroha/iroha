//! Independent coefficient, coset-order and owner-failure controls.

use super::*;
use crate::privacy_engines::zk_x509::private_table::inspection;

fn stripe(
    native_log: u8,
    stripe_log: u8,
    full_log: u8,
    ordinal: usize,
) -> main_quotient_stripes::MainQuotientStripeV1 {
    let full_root = goldilocks_primitive_root_v1(full_log).unwrap();
    main_quotient_stripes::MainQuotientStripeV1 {
        rows: 1 << stripe_log,
        count: 1 << (full_log - stripe_log),
        ordinal,
        next_stride: 1 << (stripe_log - native_log),
        root: goldilocks_primitive_root_v1(stripe_log).unwrap(),
        shift: F(GOLDILOCKS_GENERATOR_V1).mul(full_root.pow(ordinal as u128)),
    }
}

#[test]
fn every_stripe_matches_original_coefficients_coset_order_and_recovered_padding() {
    for (native_log, stripe_log, full_log) in [(2, 2, 5), (3, 4, 7), (4, 5, 5)] {
        for width in [1, 8, 11] {
            let coefficients = (0..width)
                .map(|column| {
                    (0..1_usize << native_log)
                        .map(|index| F((index * index + 7 * column + 3) as u64))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut owner = MainFixedCosetV1::new_v1(native_log, coefficients.clone()).unwrap();
            let original_full = coefficients
                .iter()
                .map(|column| {
                    goldilocks_evaluate_coset_v1(
                        column,
                        1 << full_log,
                        goldilocks_primitive_root_v1(full_log).unwrap(),
                        F(GOLDILOCKS_GENERATOR_V1),
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            let mut capacities = None;
            for ordinal in 0..1 << (full_log - stripe_log) {
                let stripe = stripe(native_log, stripe_log, full_log, ordinal);
                let actual = owner.evaluate_v1(stripe).unwrap();
                let current_capacities = actual.iter().map(Vec::capacity).collect::<Vec<_>>();
                if let Some(previous) = &capacities {
                    assert_eq!(&current_capacities, previous);
                }
                capacities = Some(current_capacities);
                for (column, values) in actual.iter().enumerate() {
                    let direct = stripe.evaluate_v1(&coefficients[column]).unwrap();
                    assert_eq!(values, &*direct);
                    for (row, value) in values.iter().enumerate() {
                        let x = stripe.shift.mul(stripe.root.pow(row as u128));
                        let horner = coefficients[column]
                            .iter()
                            .rev()
                            .fold(F::ZERO, |sum, coefficient| sum.mul(x).add(*coefficient));
                        assert_eq!(*value, horner);
                        assert_eq!(*value, original_full[column][ordinal + stripe.count * row]);
                        let next = (row + stripe.next_stride) % stripe.rows;
                        let translated = x.mul(goldilocks_primitive_root_v1(native_log).unwrap());
                        assert_eq!(
                            values[next],
                            coefficients[column]
                                .iter()
                                .rev()
                                .fold(F::ZERO, |sum, coefficient| sum
                                    .mul(translated)
                                    .add(*coefficient))
                        );
                        // Exact canonical field bytes feeding the quotient/PCS
                        // are unchanged, including natural/coset ordering.
                        assert_eq!(
                            value.value().to_be_bytes(),
                            direct[row].value().to_be_bytes()
                        );
                    }
                    let mut recovered = ZeroizingMainTraceColumnV1(values.clone());
                    goldilocks_ifft_v1(&mut recovered, stripe.root).unwrap();
                    let inverse_shift = stripe.shift.inv().unwrap();
                    let mut power = F::ONE;
                    for value in &mut *recovered {
                        *value = value.mul(power);
                        power = power.mul(inverse_shift);
                    }
                    assert_eq!(
                        &recovered[..coefficients[column].len()],
                        &coefficients[column]
                    );
                    assert!(
                        recovered[coefficients[column].len()..]
                            .iter()
                            .all(|value| *value == F::ZERO)
                    );
                }
            }
        }
    }
}

#[test]
fn fixed_matrix_rejects_malformed_coordinates_and_poisoned_reuse_before_publication() {
    for (log, columns) in [
        (20, vec![vec![F(3)]]),
        (2, vec![]),
        (2, vec![vec![F(3); 3]]),
        (2, vec![vec![F(3); 4], vec![F(u64::MAX); 4]]),
    ] {
        assert!(MainFixedCosetV1::new_v1(log, columns).is_err());
    }
    for mutation in 0..7 {
        let mut owner = MainFixedCosetV1::new_v1(2, vec![vec![F(3); 4]]).unwrap();
        let first = stripe(2, 3, 5, 0);
        owner.evaluate_v1(first).unwrap();
        let mut changed = stripe(2, 3, 5, 1);
        match mutation {
            0 => changed.ordinal = 0,
            1 => changed.ordinal = 2,
            2 => changed.root = F::ONE,
            3 => changed.shift = F::ZERO,
            4 => changed.next_stride += 1,
            5 => changed.rows = 7,
            _ => changed.count = 3,
        }
        let before = owner.columns.0.clone();
        assert!(owner.evaluate_v1(changed).is_err());
        assert_eq!(owner.columns.0, before);
        assert!(owner.evaluate_v1(stripe(2, 3, 5, 1)).is_err());
    }
}

#[test]
fn fixed_matrix_clears_owned_cells_after_success_error_and_unwind() {
    for outcome in 0..3 {
        let (result, observations) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| {
                let mut owner = MainFixedCosetV1::new_v1(2, vec![vec![F(3); 4]; 3]).unwrap();
                owner.evaluate_v1(stripe(2, 3, 5, 0)).unwrap();
                match outcome {
                    0 => {
                        owner.evaluate_v1(stripe(2, 3, 5, 1)).unwrap();
                    }
                    1 => {
                        assert!(owner.evaluate_v1(stripe(2, 3, 5, 0)).is_err());
                    }
                    _ => panic!("synthetic fixed matrix unwind"),
                }
            })
        });
        assert_eq!(result.is_err(), outcome == 2);
        assert!(
            observations
                .iter()
                .any(|record| record.cells == 8 && record.nonzero_before != 0)
        );
        assert!(observations.iter().all(|record| record.nonzero_after == 0));
    }
}

#[test]
fn in_place_fixed_transform_observes_all_forward_and_additional_inverse_work() {
    use crate::privacy_engines::zk_x509::prover_observation::ObservationV1;
    let observation = ObservationV1::begin_v1();
    let mut owner = MainFixedCosetV1::new_v1(2, vec![vec![F(3); 4]; 11]).unwrap();
    for ordinal in 0..4 {
        owner.evaluate_v1(stripe(2, 3, 5, ordinal)).unwrap();
    }
    let receipt = observation.finish_v1().public_text_v1();
    assert!(receipt.contains("fixed_coset_forward_columns=44"));
    assert!(receipt.contains("fixed_coset_recovery_inverse_columns=33"));
    assert!(receipt.contains("fixed_coset_forward_butterflies=528"));
    assert!(receipt.contains("fixed_coset_recovery_inverse_butterflies=396"));
}

#[test]
fn independent_full_quotient_coefficients_and_chunk_bytes_survive_fixed_matrix_reuse() {
    // q(X)=b(X)*f(X)+a(X)^2 is evaluated through the same native vanishing
    // division and interleaved quotient placement as the producer. Construct
    // the numerator as Z_H*q so exact division has an independent oracle.
    let native_log = 3;
    let stripe_log = 4;
    let full_log = 7;
    let native_rows = 1 << native_log;
    let full_rows = 1 << full_log;
    let fixed = (0..11)
        .map(|column| {
            (0..native_rows)
                .map(|index| F((column * 17 + index * 3 + 1) as u64))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut owner = MainFixedCosetV1::new_v1(native_log, fixed.clone()).unwrap();
    let mut actual = vec![F::ZERO; full_rows];
    let mut expected_coefficients = vec![F::ZERO; native_rows + 4];
    for (column, coefficients) in fixed.iter().enumerate() {
        let scale = F((column + 1) as u64);
        for (degree, coefficient) in coefficients.iter().enumerate() {
            expected_coefficients[degree + 4] =
                expected_coefficients[degree + 4].add(coefficient.mul(scale));
        }
    }
    expected_coefficients[6] = expected_coefficients[6].add(F(9));
    for ordinal in 0..1 << (full_log - stripe_log) {
        let stripe = stripe(native_log, stripe_log, full_log, ordinal);
        let values = owner.evaluate_v1(stripe).unwrap();
        for row in 0..stripe.rows {
            let x = stripe.shift.mul(stripe.root.pow(row as u128));
            let q = values
                .iter()
                .enumerate()
                .fold(F::ZERO, |sum, (column, values)| {
                    sum.add(values[row].mul(F((column + 1) as u64)).mul(x.pow(4)))
                })
                .add(F(9).mul(x.pow(6)));
            let vanishing = x.pow(native_rows as u128).sub(F::ONE);
            actual[ordinal + stripe.count * row] = q.mul(vanishing).mul(vanishing.inv().unwrap());
        }
    }
    let root = goldilocks_primitive_root_v1(full_log).unwrap();
    assert_eq!(
        actual,
        goldilocks_evaluate_coset_v1(
            &expected_coefficients,
            full_rows,
            root,
            F(GOLDILOCKS_GENERATOR_V1),
        )
        .unwrap()
    );
    goldilocks_ifft_v1(&mut actual, root).unwrap();
    let inverse_shift = F(GOLDILOCKS_GENERATOR_V1).inv().unwrap();
    let mut power = F::ONE;
    for value in &mut actual {
        *value = value.mul(power);
        power = power.mul(inverse_shift);
    }
    expected_coefficients.resize(full_rows, F::ZERO);
    assert_eq!(actual, expected_coefficients);
    for (chunk, expected) in actual.chunks(24).zip(expected_coefficients.chunks(24)) {
        assert_eq!(
            chunk
                .iter()
                .flat_map(|value| value.0.to_be_bytes())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .flat_map(|value| value.0.to_be_bytes())
                .collect::<Vec<_>>(),
        );
    }
}
