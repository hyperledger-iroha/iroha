//! Direct interpolation controls for the compiled MAIN fixed schedules.
//!
//! The prover interpolates native fixed tables; the verifier evaluates
//! separately compiled schedules at the DEEP point. Each control evaluates the
//! native table's interpolant at random extension points with weights built
//! here, and requires the verifier's value to be identical for every column.

use super::*;
use crate::privacy_engines::zk_x509::der_stark::{
    ZK_X509_DER_STARK_FIXED_WIDTH_V1 as DER_WIDTH, compile_zk_x509_der_stark_fixed_schedule_v1,
};

/// Two extension points off every native subgroup, and their native-root translates.
fn points_v1() -> [E; 2] {
    [
        E::canonical([
            0x6b8b_4567_327b_23c6,
            0x643c_9869_6633_4873,
            0x74b0_dc51_1949_5cff,
            0x2ae8_944a_6255_58ec,
        ])
        .unwrap(),
        E::canonical([
            0x238e_1f29_0000_0003,
            0x46e8_7ccd_3d1b_58ba,
            0x507e_d7ab_2eb1_41f2,
            0x41b7_1efb_0000_0007,
        ])
        .unwrap(),
    ]
}

/// Barycentric weights on the native subgroup: f(x) = sum_r w_r f(omega^r).
fn barycentric_weights_v1(trace_log2: u8, x: E) -> Vec<E> {
    let rows = 1_usize << trace_log2;
    let root = goldilocks_primitive_root_v1(trace_log2).unwrap();
    let scale = x
        .pow(rows as u128)
        .sub(E::ONE)
        .mul_base(F(rows as u64).inv().unwrap());
    let mut weights = Vec::with_capacity(rows);
    let mut native = F::ONE;
    for _ in 0..rows {
        weights.push(x.sub(E::from_base(native)));
        native = native.mul(root);
    }
    aggregate::batch_invert_fp4_nonzero_v1(&mut weights).unwrap();
    native = F::ONE;
    for weight in &mut weights {
        *weight = weight.mul(scale).mul_base(native);
        native = native.mul(root);
    }
    assert_eq!(native, F::ONE);
    weights
}

/// Interpolate every column of a native table at several points in one pass.
fn interpolate_rows_v1<R: AsRef<[F]>>(
    weights: &[Vec<E>],
    width: usize,
    mut row: impl FnMut(usize) -> R,
) -> Vec<Vec<E>> {
    let rows = weights[0].len();
    assert!(weights.iter().all(|point| point.len() == rows));
    let mut result = vec![vec![E::ZERO; width]; weights.len()];
    for index in 0..rows {
        let values = row(index);
        let values = values.as_ref();
        assert_eq!(values.len(), width, "native row {index}");
        for (column, value) in values.iter().enumerate() {
            if *value == F::ZERO {
                continue;
            }
            for (target, point) in result.iter_mut().zip(weights) {
                target[column] = target[column].add(point[index].mul_base(*value));
            }
        }
    }
    result
}

fn first_mismatch_v1(actual: &[E], expected: &[E]) -> Option<usize> {
    assert_eq!(actual.len(), expected.len());
    actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
}

#[test]
#[ignore = "interpolates every log19 native RFC, SHA and DER column at four points for five shapes"]
fn compiled_log19_public_sha_and_der_schedules_equal_native_interpolants() {
    let root = goldilocks_primitive_root_v1(ZK_X509_MAX_NATIVE_TRACE_LOG2_V1).unwrap();
    // Current and next-row points: z and omega19 * z for both base points.
    let points = points_v1()
        .into_iter()
        .flat_map(|point| [point, point.mul_base(root)])
        .collect::<Vec<_>>();
    let weights = points
        .iter()
        .map(|point| barycentric_weights_v1(ZK_X509_MAX_NATIVE_TRACE_LOG2_V1, *point))
        .collect::<Vec<_>>();
    // DER is shape independent; its closed forms cover rows 0 and 2^19 - 1.
    let der_schedule = compile_zk_x509_der_stark_fixed_schedule_v1(
        crate::privacy_engines::zk_x509::der_stark::ZkX509DerStarkShapeV1,
    );
    let der_expected = interpolate_rows_v1(&weights, DER_WIDTH, |row| {
        der_schedule.fixed_row(row).unwrap()
    });
    for disclosures in 0..=4_usize {
        let mut statement =
            crate::privacy_engines::zk_x509::stark::tests::main_log19_statement_fixture_v1();
        statement.disclosed_attribute_indices = (0..u8::try_from(disclosures).unwrap()).collect();
        let rfc_fixed = compile_zk_x509_rfc5280_stark_fixed_schedule_v1(
            ZkX509Rfc5280StarkShapeV1::from_statement(&statement).unwrap(),
        )
        .unwrap();
        let sha_shape = ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: disclosures,
        };
        let sha_fixed = ZkX509ShaBatchFixedProviderV1::new_v1(sha_shape).unwrap();
        let public =
            MainLog19PublicFixedAffineScheduleV1::compile_v1(&rfc_fixed, &sha_fixed).unwrap();
        let rfc_expected =
            interpolate_rows_v1(&weights, ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1, |row| {
                rfc_fixed.fixed_row(row).unwrap()
            });
        let sha_expected = (0..ZK_X509_SHA_SEGMENT_COUNT_V1)
            .map(|segment| {
                interpolate_rows_v1(&weights, ZK_X509_SHA_BATCH_FIXED_WIDTH_V1, |row| {
                    sha_fixed.fixed_row_v1(segment, row).unwrap()
                })
            })
            .collect::<Vec<_>>();
        let sha_schedule = zk_x509_sha_fixed_algebraic_schedule_v1(sha_shape).unwrap();
        for (base, point) in points_v1().into_iter().enumerate() {
            let opened = log19_public_at_point_v1(&public, point).unwrap();
            for (shift, opening) in opened.iter().enumerate() {
                let slot = 2 * base + shift;
                assert_eq!(
                    first_mismatch_v1(&opening.rfc, &rfc_expected[slot]),
                    None,
                    "RFC column, {disclosures} disclosures, point {base}, shift {shift}"
                );
                assert_eq!(
                    first_mismatch_v1(&opening.der, &der_expected[slot]),
                    None,
                    "DER column, point {base}, shift {shift}"
                );
                for (segment, expected) in sha_expected.iter().enumerate() {
                    assert_eq!(
                        first_mismatch_v1(
                            &opening.sha_public[segment],
                            &expected[slot][MAIN_LOG19_SHA_PUBLIC_FIXED_START_V1..]
                        ),
                        None,
                        "SHA public column, segment {segment}, {disclosures} disclosures, point {base}, shift {shift}"
                    );
                }
                // The algebraic SHA schedule, at the same current or next point.
                let algebraic = sha_schedule
                    .evaluate_extension_point_v1(points[slot])
                    .unwrap();
                assert_eq!(algebraic.len(), ZK_X509_SHA_FIXED_ALGEBRAIC_WIDTH_V1);
                for (segment, expected) in sha_expected.iter().enumerate() {
                    let start = segment * ZK_X509_SHA_BATCH_FIXED_WIDTH_V1;
                    assert_eq!(
                        first_mismatch_v1(
                            &algebraic[start..start + ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
                            &expected[slot]
                        ),
                        None,
                        "SHA column, segment {segment}, {disclosures} disclosures, point {base}, shift {shift}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "interpolates the native fixed table of all fifteen log19 P-256 registrations at two points"]
fn compiled_p256_log19_role_schedules_equal_every_registrations_native_interpolant() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let native = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let schedule = zk_x509_p256_fixed_algebraic_schedule_v1().unwrap();
    let points = points_v1();
    let weights = points
        .iter()
        .map(|point| barycentric_weights_v1(ZK_X509_MAX_NATIVE_TRACE_LOG2_V1, *point))
        .collect::<Vec<_>>();
    let actual = points
        .iter()
        .map(|point| schedule.evaluate_extension_point_v1(*point).unwrap())
        .collect::<Vec<_>>();
    let mut registrations = 0;
    let mut covered = vec![false; ZK_X509_P256_FIXED_ALGEBRAIC_WIDTH_V1];
    for registration in layout.registered_segments.iter().copied() {
        let Ok(identity) = p256_main_registration_from_main_layout_v1(registration) else {
            continue;
        };
        if registration.segment.trace_log2 != ZK_X509_MAX_NATIVE_TRACE_LOG2_V1 {
            continue;
        }
        registrations += 1;
        let kind = crate::privacy_engines::zk_x509::fixed_algebraic_p256::zk_x509_p256_fixed_algebraic_schedule_for_registration_v1(identity)
            .unwrap();
        let (start, width) = kind.start_width_v1();
        assert_eq!(width, registration.segment.fixed_width);
        covered[start..start + width].fill(true);
        let expected = interpolate_rows_v1(&weights, width, |row| {
            native.fixed_row_v1(identity, row).unwrap()
        });
        for (point, expected) in expected.iter().enumerate() {
            assert_eq!(
                first_mismatch_v1(&actual[point][start..start + width], expected),
                None,
                "P-256 registration {identity:?}, point {point}"
            );
        }
    }
    assert_eq!(registrations, 15);
    assert!(covered.iter().all(|covered| *covered));
}
