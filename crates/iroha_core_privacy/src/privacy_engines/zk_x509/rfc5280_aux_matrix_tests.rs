//! Whole-row auxiliary parity, terminal refusal and transactional output controls.

use super::*;

fn der_v1() -> ZkX509DerStarkChallengesV1 {
    ZkX509DerStarkChallengesV1 {
        tuple: core::array::from_fn(|lane| {
            core::array::from_fn(|slot| F(101 + (13 * lane + slot) as u64))
        }),
        byte_lookup: core::array::from_fn(|lane| F(301 + lane as u64)),
    }
}
fn challenges_v1() -> ZkX509Rfc5280StarkChallengesV1 {
    ZkX509Rfc5280StarkChallengesV1 {
        tuple: core::array::from_fn(|lane| {
            core::array::from_fn(|slot| F(7 + (12 * lane + slot) as u64))
        }),
    }
}
fn row_v1() -> RowContextV1 {
    let mut row = RowContextV1 {
        base: [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1],
        fixed: [F::ZERO; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1],
        family: ZkX509Rfc5280StarkFamilyV1::Padding,
    };
    row.base[BASE_DOCUMENT] = F(73);
    row
}

#[test]
fn full_auxiliary_groups_match_original_scalar_all_columns_across_worker_counts() {
    let rows = 19;
    let der = der_v1();
    let challenges = challenges_v1();
    let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
    let mut expected = vec![vec![F(91); rows]; WIDTH];
    for first in (0..WIDTH).step_by(BATCH) {
        super::super::fill_with_v1(
            rows,
            first,
            &mut expected[first..first + BATCH]
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>(),
            &der,
            &challenges,
            &centers,
            |_| Ok(row_v1()),
        )
        .unwrap();
    }
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        let mut actual = vec![vec![F(91); rows]; WIDTH];
        pool.install(|| {
            fill_with_v1(
                rows,
                &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
                &der,
                &challenges,
                &centers,
                |_, _| Ok(row_v1()),
            )
        })
        .unwrap();
        assert_eq!(actual, expected, "workers={workers}");
    }
    assert!(scratch_payload_bytes_v1() >= GROUPS * inverse_window::scratch_payload_bytes_v1());
    assert!(scratch_payload_bytes_v1() < 1024 * 1024);
}

#[test]
fn full_auxiliary_groups_select_earliest_error_after_every_group_finishes() {
    use std::sync::atomic::{AtomicU64, Ordering};
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        let mut actual = vec![vec![F(91); 19]; WIDTH];
        let seen = AtomicU64::new(0);
        let outcome = pool.install(|| {
            fill_groups_v1(
                19,
                &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
                |first, outputs| {
                    let group = first / BATCH;
                    seen.fetch_or(1 << group, Ordering::Relaxed);
                    assert_eq!(outputs.len(), BATCH);
                    for (offset, column) in outputs.iter_mut().enumerate() {
                        column.fill(F((first + offset + 1) as u64));
                    }
                    match group {
                        3 => Err(ZkX509Rfc5280StarkErrorV1::Source),
                        17 => Err(ZkX509Rfc5280StarkErrorV1::Grammar),
                        34 => Err(ZkX509Rfc5280StarkErrorV1::Semantic),
                        _ => Ok(()),
                    }
                },
            )
        });
        assert_eq!(outcome, Err(ZkX509Rfc5280StarkErrorV1::Source));
        assert_eq!(seen.load(Ordering::Relaxed), (1 << GROUPS) - 1);
        assert!(actual.iter().flatten().all(|value| *value == F::ZERO));
    }
}

#[test]
fn bounded_inverse_capacity_preserves_zero_gates_and_noncanonical_refusal() {
    const PAIRS: usize = inverse_window::PAIRS;
    let cases = [
        (F::ZERO, F(u64::MAX)),
        (F::ONE, F::ZERO),
        (F(2), F(19)),
        (F(u64::MAX), F(257)),
    ];
    let mut window = inverse_window::InverseWindowV1::new_v1();
    for index in 0..PAIRS {
        let (gate, factor) = cases[index % cases.len()];
        window.collect_v1(gate, factor);
    }
    window.invert_v1();
    for index in 0..PAIRS {
        let (gate, factor) = cases[index % cases.len()];
        assert_eq!(
            window.replay_v1(gate, factor),
            zero_safe_inverse_v1(gate, factor)
        );
    }
    window.finish_v1();
    assert!(
        std::panic::catch_unwind(|| {
            let mut window = inverse_window::InverseWindowV1::new_v1();
            window.collect_v1(F::ONE, F(u64::MAX));
        })
        .is_err()
    );
}

#[test]
fn full_auxiliary_groups_erase_all_columns_on_error_unwind_and_terminal_failure() {
    let der = der_v1();
    let challenges = challenges_v1();
    let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for mode in 0..3 {
            let mut actual = vec![vec![F(91); 19]; WIDTH];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pool.install(|| {
                    fill_with_v1(
                        19,
                        &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
                        &der,
                        &challenges,
                        &centers,
                        |_, index| {
                            if index == 17 {
                                if mode == 0 {
                                    return Err(ZkX509Rfc5280StarkErrorV1::Shape);
                                }
                                if mode == 1 {
                                    panic!("parallel auxiliary source unwind");
                                }
                            }
                            let mut row = row_v1();
                            if mode == 2 && index == 0 {
                                row.family = ZkX509Rfc5280StarkFamilyV1::SourceByte;
                                row.base[BASE_ACTIVE] = F::ONE;
                                row.base[SERIAL_BYTE_TABLE_MULTIPLICITY] = F::ONE;
                            }
                            Ok(row)
                        },
                    )
                })
            }));
            if mode == 1 {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(actual.iter().flatten().all(|value| *value == F::ZERO));
        }
    }
    for (rows, width, length) in [(0, WIDTH, 0), (19, WIDTH - 1, 19), (19, WIDTH, 18)] {
        let mut invalid = vec![vec![F(91); length]; width];
        assert_eq!(
            fill_groups_v1(
                rows,
                &mut invalid
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>(),
                |_, _| panic!("invalid matrix read source")
            ),
            Err(ZkX509Rfc5280StarkErrorV1::Shape)
        );
        assert!(invalid.iter().flatten().all(|value| *value == F(91)));
    }
}
