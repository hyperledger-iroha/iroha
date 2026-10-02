//! Live-cell cleanup before private composition material reaches its final owner.

use super::*;
use crate::privacy_engines::zk_x509::private_table::inspection;

fn private(index: u64) -> E {
    E::from_coefficients([F(index), F(index + 1), F(index + 2), F(index + 3)]).unwrap()
}
fn chunks() -> Vec<Vec<Vec<E>>> {
    vec![
        (0..COMPOSITION_DEGREE_CHUNKS)
            .map(|index| vec![private(index as u64 + 3); index + 1])
            .collect(),
    ]
}

#[test]
fn composition_precursors_clear_populated_lanes_on_success_error_and_unwind() {
    for outcome in 0..3 {
        let (result, records) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| -> Result<(), ZkX509StarkErrorV1> {
                let mut owner =
                    ZeroizingExtensionLanesV1::new(Vec::new(), zeroize_extension_lanes_v1);
                owner.push(vec![vec![private(7); 3]]);
                owner.push(vec![vec![private(11); 5], vec![private(13); 7]]);
                match outcome {
                    0 => Ok(()),
                    1 => Err(ZkX509StarkErrorV1::ProfileMismatch),
                    _ => panic!("synthetic composition construction unwind"),
                }
            })
        });
        assert_eq!(result.is_err(), outcome == 2);
        if let Ok(inner) = result {
            assert_eq!(inner.is_err(), outcome == 1);
        }
        assert_eq!(records.iter().map(|record| record.cells).sum::<usize>(), 15);
        assert!(
            records
                .iter()
                .all(|record| record.nonzero_before == record.cells)
        );
        assert!(records.iter().all(|record| record.nonzero_after == 0));
    }
}

#[test]
fn composition_growth_clears_displaced_allocation_and_reserved_additions_keep_addresses() {
    let mut accumulator = ZeroizingExtensionLanesV1::new(chunks(), zeroize_extension_lanes_v1);
    let contribution = vec![vec![vec![private(19); 13]; COMPOSITION_DEGREE_CHUNKS]];
    let before = accumulator.to_vec();
    let (result, records) = inspection::observe_v1(|| {
        add_main_composition_coefficient_chunks_v1(&mut accumulator, &contribution, 32)
    });
    result.unwrap();
    assert!(records.iter().any(|record| record.nonzero_before > 0));
    assert!(records.iter().all(|record| record.nonzero_after == 0));
    for (column, old) in accumulator[0].iter().zip(&before[0]) {
        for (index, actual) in column.iter().enumerate() {
            assert_eq!(
                *actual,
                old.get(index).copied().unwrap_or(E::ZERO).add(private(19))
            );
        }
    }
    let addresses = accumulator[0].iter().map(Vec::as_ptr).collect::<Vec<_>>();
    add_main_composition_coefficient_chunks_v1(&mut accumulator, &contribution, 32).unwrap();
    assert_eq!(
        addresses,
        accumulator[0].iter().map(Vec::as_ptr).collect::<Vec<_>>()
    );
    let before = accumulator.to_vec();
    let mut malformed = contribution.clone();
    malformed[0][COMPOSITION_DEGREE_CHUNKS - 1].resize(33, private(23));
    assert!(add_main_composition_coefficient_chunks_v1(&mut accumulator, &malformed, 32).is_err());
    assert_eq!(&*accumulator, &before);
}

#[test]
fn composition_cancellation_retains_extent_and_clears_on_drop() {
    let mut accumulator = ZeroizingExtensionLanesV1::new(chunks(), zeroize_extension_lanes_v1);
    let mut contribution = chunks();
    for chunk in &mut contribution[0] {
        for coefficient in chunk {
            *coefficient = E::ZERO.sub(*coefficient);
        }
    }
    let extents = accumulator[0].iter().map(Vec::len).collect::<Vec<_>>();
    let initialized = extents.iter().sum::<usize>();
    let (result, records) = inspection::observe_v1(|| {
        let result =
            add_main_composition_coefficient_chunks_v1(&mut accumulator, &contribution, 32);
        assert_eq!(
            accumulator[0].iter().map(Vec::len).collect::<Vec<_>>(),
            extents
        );
        assert!(
            accumulator[0]
                .iter()
                .flatten()
                .all(|value| *value == E::ZERO)
        );
        // The original clearing owner must visit the full public extent even
        // though exact cancellation has already made every coefficient zero.
        drop(accumulator);
        result
    });
    result.unwrap();
    assert_eq!(
        records.iter().map(|record| record.cells).sum::<usize>(),
        initialized
    );
    assert!(records.iter().all(|record| record.nonzero_after == 0));
}

#[test]
fn composition_evaluation_clears_prior_results_on_late_error_and_matches_horner() {
    let mut source = chunks();
    let root = goldilocks_primitive_root_v1(5).unwrap();
    let output = ZeroizingExtensionLanesV1::new(
        evaluate_main_composition_columns_v1(&source, 32, root, 16).unwrap(),
        zeroize_extension_lanes_v1,
    );
    for (coefficients, evaluated) in source[0].iter().zip(&output[0]) {
        for (row, actual) in evaluated.iter().enumerate() {
            let x = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128));
            let expected = coefficients.iter().rev().fold(E::ZERO, |sum, coefficient| {
                sum.mul_base(x).add(*coefficient)
            });
            assert_eq!(*actual, expected);
        }
    }
    source[0][2].resize(17, private(29));
    let (result, records) =
        inspection::observe_v1(|| evaluate_main_composition_columns_v1(&source, 32, root, 16));
    assert!(result.is_err());
    assert_eq!(records.iter().map(|record| record.cells).sum::<usize>(), 64);
    assert!(records.iter().any(|record| record.nonzero_before > 0));
    assert!(records.iter().all(|record| record.nonzero_after == 0));
    assert!(goldilocks_fp4_evaluate_coset_v1(&[private(31)], 32, F::ZERO, F(7)).is_err());
}
