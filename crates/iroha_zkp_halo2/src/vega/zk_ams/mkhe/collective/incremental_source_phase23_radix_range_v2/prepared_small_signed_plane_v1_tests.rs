//! Independent signed embeddings, exact coordinates and sparse actual-MSM oracles.
use super::*;
use crate::generalized_bulletproof::ProofSuite;
use crate::vega::{VegaT256PointV1, bulletproof_t256::ZkAmsT256BulletproofSuiteV1};
use core::ops::{Add, Sub};

const SPARSE_SIGNED_POSITIONS_V1: [(usize, i8); 7] = [
    (0, -1),
    (63, 1),
    (64, -1),
    (1_023, 1),
    (1_024, -1),
    (8_191, 1),
    (16_383, -1),
];

// Arithmetic fixture only. It cannot mint authenticated source evidence.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct TestPreparedSmallSignedV1
{
    values: PreparedRadixValuesV1,
}

impl TestPreparedSmallSignedV1 {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn for_ordinal_v1(
        ordinal: u16,
    ) -> Self {
        let coordinate = small_signed_plane_coordinate_v1(ordinal).unwrap();
        let mut packed = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
        for (position, sign) in SPARSE_SIGNED_POSITIONS_V1 {
            packed.as_mut_slice_v1()[position] = (sign * coordinate.bound as i8) as u8;
        }
        Self {
            values: expand_small_signed_values_v1(packed, coordinate).unwrap(),
        }
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn statement_v1(
        &self,
        ordinal: u16,
    ) -> PreparedSmallSignedStatementV1<'_> {
        PreparedSmallSignedStatementV1 {
            values: &self.values,
            ordinal,
            replay_record_digest: [0x61; 32],
            source_receipt_digest: [0x62; 32],
        }
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn expected_v1(
        ordinal: u16,
        rho: VegaT256ScalarV1,
    ) -> VegaT256PointV1 {
        let unit = usize::from(ordinal - 7_224) % 1_032;
        let bound = if unit / 8 % 3 == 0 { 1 } else { 2 };
        let negative_magnitude = ordinal >= 8_256;
        let generators = ZkAmsT256BulletproofSuiteV1::generators()
            .reduce(16_384)
            .unwrap();
        let mut point = generators.h.mul_scalar(rho);
        for (position, sign) in SPARSE_SIGNED_POSITIONS_V1 {
            for _ in 0..bound {
                point = if negative_magnitude {
                    if sign < 0 {
                        point.add(generators.g_bold[position])
                    } else {
                        point
                    }
                } else if sign < 0 {
                    point.sub(generators.g_bold[position])
                } else {
                    point.add(generators.g_bold[position])
                };
            }
        }
        point
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn expected_positive_v1(
        signed_ordinal: u16,
        rho_sum: VegaT256ScalarV1,
    ) -> VegaT256PointV1 {
        let unit = usize::from(signed_ordinal - 7_224);
        assert!(unit < 1_032);
        let bound = if unit / 8 % 3 == 0 { 1 } else { 2 };
        let generators = ZkAmsT256BulletproofSuiteV1::generators()
            .reduce(16_384)
            .unwrap();
        let mut point = generators.h.mul_scalar(rho_sum);
        for (position, sign) in SPARSE_SIGNED_POSITIONS_V1 {
            if sign > 0 {
                for _ in 0..bound {
                    point = point.add(generators.g_bold[position]);
                }
            }
        }
        point
    }
}

fn scalar_be_v1(value: i8) -> [u8; 32] {
    // Independent integer oracle: negative magnitudes are subtracted directly
    // from the public field modulus, rather than using the implementation's negation.
    let mut out = [0_u8; 32];
    if value >= 0 {
        out[31] = value as u8;
    } else {
        out = VEGA_T256_SCALAR_MODULUS_BE_V1;
        let mut borrow = u16::from(value.unsigned_abs());
        for byte in out.iter_mut().rev() {
            let current = u16::from(*byte);
            *byte = current.wrapping_sub(borrow) as u8;
            borrow = u16::from(current < borrow);
        }
        assert_eq!(borrow, 0);
    }
    out
}

#[test]
fn all_2064_signed_selectors_match_natural_source_order_and_role_bounds() {
    for ordinal in 7_224..9_288 {
        let coordinate = small_signed_plane_coordinate_v1(ordinal).unwrap();
        let expected_slot = (ordinal - 7_224) % 1_032;
        assert_eq!(coordinate.source_slot, expected_slot);
        assert_eq!(coordinate.negative_magnitude, ordinal >= 8_256);
        assert_eq!(
            coordinate.bound,
            if expected_slot / 8 % 3 == 0 { 1 } else { 2 }
        );
        let shared = comparator_signed_coordinate_v1(u32::from(ordinal)).unwrap();
        assert_eq!(shared.global_ordinal, 25_112 + u32::from(ordinal - 7_224));
    }
    for ordinal in [0, 7_223, 9_288, 9_289, u16::MAX] {
        assert!(small_signed_plane_coordinate_v1(ordinal).is_err());
    }
    assert_eq!(SMALL_SIGNED_AUTHENTICATED_READ_BYTES_V1, 33_849_600);
}

#[test]
fn signed_and_negative_chunks_match_integer_oracle_without_transpose() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    for ordinal in [7_224, 7_232, 8_255, 8_256, 8_264, 9_287] {
        let coordinate = small_signed_plane_coordinate_v1(ordinal).unwrap();
        let mut packed = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
        for (index, byte) in packed.as_mut_slice_v1().iter_mut().enumerate() {
            *byte = if coordinate.bound == 1 {
                [-1_i8, 0, 1][index % 3]
            } else {
                [-2_i8, -1, 0, 1, 2][index % 5]
            } as u8;
        }
        let mut values = expand_small_signed_values_v1(packed, coordinate).unwrap();
        for chunk in 0..32_u8 {
            let output = values.emit_next_v1(chunk).unwrap();
            for (local, bytes) in output.as_slice_v1().chunks_exact(32).enumerate() {
                let index = usize::from(chunk) * 512 + local;
                let x = if coordinate.bound == 1 {
                    [-1_i8, 0, 1][index % 3]
                } else {
                    [-2_i8, -1, 0, 1, 2][index % 5]
                };
                let expected = if coordinate.negative_magnitude {
                    (-x).max(0)
                } else {
                    x
                };
                assert_eq!(
                    bytes,
                    scalar_be_v1(expected),
                    "plane={ordinal} index={index}"
                );
            }
        }
        values.finish_v1().unwrap();
    }
}

#[test]
fn malformed_signed_bytes_and_coordinates_fail_before_scalar_allocation() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    use crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1;
    for ordinal in [7_224, 7_232, 8_256, 8_264] {
        let coordinate = small_signed_plane_coordinate_v1(ordinal).unwrap();
        for value in 0..=255_u8 {
            let mut packed = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
            packed.as_mut_slice_v1()[16_383] = value;
            if (value as i8).unsigned_abs() <= coordinate.bound {
                continue;
            }
            let before = zeroizing_t256_scalar_vec_drop_count_v1();
            assert!(expand_small_signed_values_v1(packed, coordinate).is_err());
            assert_eq!(zeroizing_t256_scalar_vec_drop_count_v1(), before);
        }
        for axis in 0..4 {
            let mut changed = coordinate;
            match axis {
                0 => changed.ordinal += 1,
                1 => changed.source_slot += 1,
                2 => changed.negative_magnitude = !changed.negative_magnitude,
                _ => changed.bound += 1,
            }
            assert!(
                expand_small_signed_values_v1(
                    ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap(),
                    changed
                )
                .is_err()
            );
        }
        assert!(
            expand_small_signed_values_v1(
                ConfidentialSpoolChunkV1::new_zeroed_v1(16_383).unwrap(),
                coordinate
            )
            .is_err()
        );
    }
}

#[test]
fn signed_statement_origin_order_and_chunk_poisoning_are_enforced() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let mut fixture = TestPreparedSmallSignedV1::for_ordinal_v1(7_224);
    let statement = fixture.statement_v1(7_224);
    statement
        .validate_origin_v1([0x61; 32], [0x62; 32])
        .unwrap();
    statement.require_ordinal_v1(7_224).unwrap();
    assert!(statement.require_ordinal_v1(7_225).is_err());
    for (record, receipt) in [
        ([0; 32], [0x62; 32]),
        ([0x61; 32], [0; 32]),
        ([0x63; 32], [0x62; 32]),
        ([0x61; 32], [0x63; 32]),
    ] {
        assert!(statement.validate_origin_v1(record, receipt).is_err());
    }
    assert!(
        fixture
            .statement_v1(9_288)
            .require_ordinal_v1(9_288)
            .is_err()
    );
    assert!(
        fixture
            .statement_v1(7_224)
            .commitment_v1(&VegaT256ScalarV1::zero())
            .is_err()
    );
    drop(fixture.values.emit_next_v1(0).unwrap());
    assert!(
        fixture
            .statement_v1(7_224)
            .commitment_v1(&VegaT256ScalarV1::one())
            .is_err()
    );
    assert!(fixture.values.emit_next_v1(0).is_err());
    assert!(fixture.values.emit_next_v1(1).is_err());
    assert!(fixture.values.finish_v1().is_err());
    for consumed in [0, 31, 32] {
        let mut fixture = TestPreparedSmallSignedV1::for_ordinal_v1(8_256);
        for chunk in 0..consumed {
            drop(fixture.values.emit_next_v1(chunk).unwrap());
        }
        assert_eq!(fixture.values.finish_v1().is_ok(), consumed == 32);
    }
    let mut empty = PreparedSmallSignedPlaneV1::<core::convert::Infallible, (), ()> { live: None };
    assert!(empty.emit_next_value_chunk_v1(0).is_err());
    assert!(empty.finish_v1().is_err());
}

#[test]
fn consuming_signed_driver_keeps_source_validation_ahead_of_read_and_emission() {
    // Structural coverage only: a production context is still uninhabited.
    let source = include_str!("prepared_small_signed_plane_v1.rs");
    let prepare = source
        .split("fn prepare_next_small_signed_plane_v1(")
        .nth(1)
        .unwrap()
        .split("fn expand_small_signed_values_v1(")
        .next()
        .unwrap();
    assert!(
        prepare
            .find("small_signed_plane_coordinate_v1(self.next_comparator_plane)")
            .unwrap()
            < prepare.find(".read_small_signed_plane_v1(").unwrap()
    );
    assert!(
        prepare
            .find("validate_materialized_context_v1(&self)")
            .unwrap()
            < prepare.find(".read_small_signed_plane_v1(").unwrap()
    );
    assert!(
        prepare.find(".read_small_signed_plane_v1(").unwrap()
            < prepare.find("expand_small_signed_values_v1(").unwrap()
    );
    assert!(
        prepare
            .find(".commit_prepared_small_signed_v1(&statement)")
            .unwrap()
            < prepare.find("Ok(PreparedSmallSignedPlaneV1").unwrap()
    );
}
