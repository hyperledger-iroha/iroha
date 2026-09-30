//! Independent polynomial and generic-kernel checks for fixed-column Fp4 evaluation.

use super::*;
use crate::privacy_engines::transparent_stark::goldilocks_ifft_v1;

fn domain(log: u8) -> ZkX509FixedAlgebraicDomainV1 {
    ZkX509FixedAlgebraicDomainV1::new_v1(log, log + 2, F(7)).unwrap()
}

fn mixed_schedule(log: u8) -> ZkX509FixedAlgebraicScheduleV1 {
    let size = 1_u64 << log;
    let mut atoms = vec![
        ZkX509FixedAlgebraicAtomV1::affine_v1(0, 0, size, F(19), F(5)).unwrap(),
        ZkX509FixedAlgebraicAtomV1::sparse_v1(0, size - 1, F(23)).unwrap(),
        ZkX509FixedAlgebraicAtomV1::sparse_v1(1, size - 1, F(31)).unwrap(),
    ];
    if size >= 4 {
        for stride in [2, 3, 6, 10, 24] {
            let count = (size - 1) / stride + 1;
            if count >= 2 {
                for column in 0..3 {
                    atoms.push(
                        ZkX509FixedAlgebraicAtomV1::repeated_affine_v1(
                            column,
                            0,
                            count,
                            stride,
                            F(37 + u64::from(column)),
                            F(41),
                        )
                        .unwrap(),
                    );
                }
            }
        }
        // Enough occurrences sharing stride two to select the cyclic table,
        // while the other strides still exercise direct repeated sums.
        for column in 0..3 {
            atoms.push(
                ZkX509FixedAlgebraicAtomV1::repeated_affine_v1(
                    column,
                    1,
                    size / 2,
                    2,
                    F(43 + u64::from(column)),
                    F(47),
                )
                .unwrap(),
            );
        }
    }
    ZkX509FixedAlgebraicScheduleV1::new_v1(domain(log), 4, atoms).unwrap()
}

fn interpolation_reference(schedule: &ZkX509FixedAlgebraicScheduleV1, point: E) -> Vec<E> {
    let native_size = schedule.domain.native_size_v1().unwrap() as usize;
    let root = goldilocks_primitive_root_v1(schedule.domain.native_log2).unwrap();
    (0..schedule.width)
        .map(|column| {
            let mut coefficients = vec![F::ZERO; native_size];
            schedule
                .fill_native_column_v1(column, &mut coefficients)
                .unwrap();
            goldilocks_ifft_v1(&mut coefficients, root).unwrap();
            coefficients
                .iter()
                .rev()
                .fold(E::ZERO, |value, &coefficient| {
                    value.mul(point).add(E::from_base(coefficient))
                })
        })
        .collect()
}

#[test]
fn fixed_fp4_schedule_matches_native_ifft_and_horner_and_base_embeddings() {
    let mut saw_table = false;
    let mut saw_direct = false;
    for log in [1, 2, 4, 6] {
        let schedule = mixed_schedule(log);
        let digest_before = schedule.descriptor_digest;
        let (_, runs) = repeated_stride_plan_v1(&schedule.atoms).unwrap();
        for run in runs {
            if repeated_stride_uses_table_v1(run, 1, 1_u64 << log).unwrap() {
                saw_table = true;
            } else {
                saw_direct = true;
            }
        }
        for point in [
            E::ZERO,
            E::canonical([7, 11, 13, 17]).unwrap(),
            E::canonical([0, 1, 0, 0]).unwrap(),
        ] {
            assert_eq!(
                schedule.evaluate_extension_point_v1(point).unwrap(),
                interpolation_reference(&schedule, point)
            );
        }
        let indices = [0, 1, schedule.domain.lde_size_v1().unwrap() - 1];
        let base = schedule.evaluate_query_indices_v1(&indices).unwrap();
        for (slot, index) in indices.into_iter().enumerate() {
            let point = E::from_base(schedule.domain.query_point_v1(index).unwrap());
            assert_eq!(
                schedule.evaluate_extension_point_v1(point).unwrap(),
                base.row_v1(slot)
                    .unwrap()
                    .iter()
                    .copied()
                    .map(E::from_base)
                    .collect::<Vec<_>>()
            );
        }
        assert_eq!(schedule.descriptor_digest, digest_before);
    }
    assert!(saw_table && saw_direct);
}

#[test]
fn fixed_fp4_highest_degree_and_constant_columns_have_independent_expected_values() {
    let domain = domain(6);
    let native_size = domain.native_size_v1().unwrap();
    let root = goldilocks_primitive_root_v1(domain.native_log2).unwrap();
    let mut atoms = Vec::new();
    for row in 0..native_size {
        atoms.push(
            ZkX509FixedAlgebraicAtomV1::sparse_v1(
                0,
                row,
                root.pow(u128::from(row * (native_size - 1))),
            )
            .unwrap(),
        );
    }
    atoms.push(ZkX509FixedAlgebraicAtomV1::affine_v1(1, 0, native_size, F(91), F::ZERO).unwrap());
    let schedule = ZkX509FixedAlgebraicScheduleV1::new_v1(domain, 3, atoms).unwrap();
    for point in [
        E::canonical([7, 11, 13, 17]).unwrap(),
        E::canonical([23, 0, 29, 31]).unwrap(),
    ] {
        assert_eq!(
            schedule.evaluate_extension_point_v1(point).unwrap(),
            vec![
                point.pow(u128::from(native_size - 1)),
                E::from_base(F(91)),
                E::ZERO
            ]
        );
    }
}

#[test]
fn fixed_fp4_cyclic_prefixes_match_direct_sums_for_every_shift_and_gcd() {
    let table = LagrangeTableV1::<E>::at_extension_point_v1(
        domain(6),
        E::canonical([7, 11, 13, 17]).unwrap(),
    )
    .unwrap();
    for stride in [1_u64, 2, 3, 6, 10, 24, 32, 40] {
        let cyclic = CyclicStrideTableV1::new_v1(&table.weights, stride).unwrap();
        for first in [0_u64, 1, 5] {
            let count = (63 - first) / stride + 1;
            for shift in 0..64 {
                let expected = (0..count).fold(E::ZERO, |sum, occurrence| {
                    let row = (first + occurrence * stride) as usize;
                    let coefficient = F(7).add(F(5).mul(F(occurrence)));
                    sum.add(table.weights[(row + 64 - shift) % 64].mul_base(coefficient))
                });
                assert_eq!(
                    cyclic
                        .repeated_affine_sum_v1(first, count, stride, F(7), F(5), shift)
                        .unwrap(),
                    expected
                );
            }
        }
    }
}

#[test]
fn fixed_fp4_singular_points_and_excess_work_fail_closed() {
    let schedule = mixed_schedule(4);
    let root = goldilocks_primitive_root_v1(4).unwrap();
    for index in 0..16 {
        assert_eq!(
            schedule.evaluate_extension_point_v1(E::from_base(root.pow(index))),
            Err(ZkX509FixedAlgebraicErrorV1::DivisionByZero)
        );
    }
    for malformed in [vec![], vec![E::ONE], vec![E::ZERO; 2], vec![E::ONE; 3]] {
        assert!(LagrangeTableV1::from_weights_v1(malformed).is_err());
    }
    assert!(LagrangeTableV1::from_weights_v1(vec![F(GOLDILOCKS_MODULUS_V1), F::ONE]).is_err());
    // The compiled cap is enforced before allocating an extension Lagrange
    // table. One legal schedule can still demand too many distinct stride sums.
    let native_size = 1_u64 << 20;
    let mut atoms = Vec::new();
    for stride in 2..=257 {
        for column in 0..256 {
            atoms.push(
                ZkX509FixedAlgebraicAtomV1::repeated_v1(
                    column,
                    0,
                    (native_size - 1) / stride + 1,
                    stride,
                    F::ONE,
                )
                .unwrap(),
            );
        }
    }
    let schedule = ZkX509FixedAlgebraicScheduleV1::new_v1(domain(20), 256, atoms).unwrap();
    assert_eq!(
        schedule.evaluate_extension_point_v1(E::canonical([7, 11, 13, 17]).unwrap()),
        Err(ZkX509FixedAlgebraicErrorV1::LimitExceeded)
    );
}
