//! Integer-difference, source/borrow linkage, canonical order and erasure tests.
use super::*;
use crate::generalized_bulletproof::ProofSuite;
use crate::vega::{VegaT256PointV1, bulletproof_t256::ZkAmsT256BulletproofSuiteV1};
use core::ops::Add;

fn scalar_from_hex_v1(hex: &str) -> VegaT256ScalarV1 {
    let bytes = core::array::from_fn(|j| u8::from_str_radix(&hex[2 * j..2 * j + 2], 16).unwrap());
    VegaT256ScalarV1::from_be_bytes_exact_ref(&bytes).unwrap()
}

fn scalar_and_packed_v1(scalar: &VegaT256ScalarV1) -> RadixPackedComparatorV2 {
    let mut bytes = [0; 32];
    scalar.write_le_bytes_ref(&mut bytes);
    bytes.reverse();
    pack_comparator_lanes_v2(&radix_coefficient_witness_v2(&bytes).unwrap()).unwrap()
}

fn decode_digit_v1(scalar: &VegaT256ScalarV1) -> u16 {
    let mut bytes = [0; 32];
    scalar.write_le_bytes_ref(&mut bytes);
    assert!(bytes[2..].iter().all(|byte| *byte == 0));
    u16::from_le_bytes(bytes[..2].try_into().unwrap())
}

#[test]
fn all_5848_difference_coordinates_are_group_major_and_exactly_bounded() {
    for ordinal in 0..5_848 {
        let coordinate = difference_digit_coordinate_v1(ordinal).unwrap();
        assert_eq!(
            (coordinate.group, coordinate.digit),
            (ordinal / 17, (ordinal % 17) as u8)
        );
    }
    for ordinal in [5_848, 5_849, u16::MAX] {
        assert!(difference_digit_coordinate_v1(ordinal).is_err());
    }
}

#[test]
fn independent_integer_vectors_cover_threshold_top_bit_and_radix_boundaries() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    // Independent Python big-integer subtraction modulo 2^255, then repeated
    // base-32768 division. The production path instead checks each borrow.
    let vectors: [(&str, [u16; 17]); 10] = [
        (
            "0000000000000000000000000000000000000000000000000000000000000000",
            [
                0, 0, 0, 0, 0, 0, 32736, 32767, 32767, 32767, 32767, 32767, 30719, 32767, 8191, 0,
                0,
            ],
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000000001",
            [
                1, 0, 0, 0, 0, 0, 32736, 32767, 32767, 32767, 32767, 32767, 30719, 32767, 8191, 0,
                0,
            ],
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000007fff",
            [
                32767, 0, 0, 0, 0, 0, 32736, 32767, 32767, 32767, 32767, 32767, 30719, 32767, 8191,
                0, 0,
            ],
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000008000",
            [
                0, 1, 0, 0, 0, 0, 32736, 32767, 32767, 32767, 32767, 32767, 30719, 32767, 8191, 0,
                0,
            ],
        ),
        (
            "7fffffff800000008000000000000000000000007fffffffffffffffffffffff",
            [
                32767, 32767, 32767, 32767, 32767, 32767, 32767, 32767, 32767, 32767, 32767, 32767,
                32767, 32767, 32767, 32767, 32767,
            ],
        ),
        (
            "7fffffff80000000800000000000000000000000800000000000000000000000",
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            "7fffffff80000000800000000000000000000000800000000000000000000001",
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            "7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            [
                32767, 32767, 32767, 32767, 32767, 32767, 32735, 32767, 32767, 32767, 32767, 32767,
                30719, 32767, 8191, 0, 0,
            ],
        ),
        (
            "8000000000000000000000000000000000000000000000000000000000000000",
            [
                0, 0, 0, 0, 0, 0, 32736, 32767, 32767, 32767, 32767, 32767, 30719, 32767, 8191, 0,
                0,
            ],
        ),
        (
            "ffffffff00000001000000000000000000000000fffffffffffffffffffffffe",
            [
                32766, 32767, 32767, 32767, 32767, 32767, 31, 0, 0, 0, 0, 0, 2048, 0, 24576, 32767,
                32767,
            ],
        ),
    ];
    for (hex, expected) in vectors {
        let scalar = scalar_from_hex_v1(hex);
        let packed = scalar_and_packed_v1(&scalar);
        for digit in 0..17 {
            assert_eq!(
                decode_digit_v1(&difference_digit_scalar_v1(&scalar, &packed, digit).unwrap()),
                expected[usize::from(digit)]
            );
            // Toggling the current borrow adds or removes exactly B; reject
            // integer underflow/overflow even though it has a field residue.
            let mut wrong = RadixPackedComparatorV2(packed.0);
            let (lane, bit) = if digit < 6 {
                (0, digit + 2)
            } else if digit < 14 {
                (1, digit - 6)
            } else {
                (2, digit - 14)
            };
            wrong.0[lane] ^= 1 << bit;
            assert!(difference_digit_scalar_v1(&scalar, &wrong, digit).is_err());
        }
        for digit in [17, 18, u8::MAX] {
            assert!(difference_digit_scalar_v1(&scalar, &packed, digit).is_err());
        }
        let mut reserved = RadixPackedComparatorV2(packed.0);
        reserved.0[2] |= 0x80;
        assert!(difference_digit_scalar_v1(&scalar, &reserved, 0).is_err());
    }
}

fn patterned_group_parts_v1(ordinal: u16) -> (LowDigitGroupV1, [ConfidentialSpoolChunkV1; 3]) {
    let coordinate = difference_digit_coordinate_v1(ordinal).unwrap();
    let mut values = ZeroizingT256ScalarVecV1::try_with_exact_capacity(16_384).unwrap();
    let mut lanes =
        core::array::from_fn(|_| ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap());
    for j in 0..16_384 {
        let value = match j {
            0 => 7_u16,
            256 => 9,
            1 => 11,
            16_383 => 13,
            _ => 0,
        };
        let mut offset = [0_u8; 32];
        for bit in 0..15 {
            let position = usize::from(coordinate.digit) * 15 + bit;
            offset[31 - position / 8] |= (((value >> bit) & 1) as u8) << (position % 8);
        }
        let mut encoded = [0_u8; 32];
        assert_eq!(
            *fixed_add_be_v2(&RADIX_CENTERING_THRESHOLD_BE_V2, &offset, &mut encoded).as_ref_v2(),
            0
        );
        let scalar = VegaT256ScalarV1::from_be_bytes_exact_ref(&encoded).unwrap();
        let packed = scalar_and_packed_v1(&scalar);
        let v = (j % 256) * 64 + j / 256;
        for (lane, byte) in lanes.iter_mut().zip(packed.0) {
            lane.as_mut_slice_v1()[v] = byte;
        }
        values.push(scalar);
    }
    (
        LowDigitGroupV1 {
            group: coordinate.group,
            values,
        },
        lanes,
    )
}

#[test]
fn every_sealed_lane_bit_and_last_coefficient_are_bound_to_original_source() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    for (lane, mask, index) in [
        (0, 1, 0),
        (0, 4, 1),
        (1, 1, 64),
        (2, 1, 16_383),
        (2, 0x20, 16_383),
    ] {
        let (source, mut lanes) = patterned_group_parts_v1(0);
        lanes[lane].as_mut_slice_v1()[index] ^= mask;
        assert!(DifferenceDigitGroupV1::from_authenticated_group_v1(source, lanes).is_err());
    }
    let (mut source, lanes) = patterned_group_parts_v1(0);
    source.group = 344;
    assert!(DifferenceDigitGroupV1::from_authenticated_group_v1(source, lanes).is_err());
    let (source, mut lanes) = patterned_group_parts_v1(0);
    lanes[1] = ConfidentialSpoolChunkV1::new_zeroed_v1(32).unwrap();
    assert!(DifferenceDigitGroupV1::from_authenticated_group_v1(source, lanes).is_err());
}

// This is an isolated arithmetic/MSM fixture, never a replay authority or a
// complete source proof. Four source coefficients are K + value * B^digit.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct TestPreparedDifferenceDigitV1
{
    values: PreparedRadixValuesV1,
}
impl TestPreparedDifferenceDigitV1 {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn new_v1(
        ordinal: u16,
    ) -> Self {
        let (source, lanes) = patterned_group_parts_v1(ordinal);
        let group = DifferenceDigitGroupV1::from_authenticated_group_v1(source, lanes).unwrap();
        Self {
            values: group
                .prepare_values_v1(difference_digit_coordinate_v1(ordinal).unwrap())
                .unwrap(),
        }
    }
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn statement_v1(
        &self,
        ordinal: u16,
    ) -> PreparedDifferenceDigitStatementV1<'_> {
        PreparedDifferenceDigitStatementV1 {
            values: &self.values,
            ordinal,
            replay_record_digest: [0x61; 32],
            source_receipt_digest: [0x62; 32],
        }
    }
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn expected_commitment_v1(
        rho: VegaT256ScalarV1,
    ) -> VegaT256PointV1 {
        let generators = ZkAmsT256BulletproofSuiteV1::generators()
            .reduce(16_384)
            .unwrap();
        let mut point = generators.h.mul_scalar(rho);
        for (v, value) in [(0, 7), (1, 9), (64, 11), (16_383, 13)] {
            point = point.add(generators.g_bold[v].mul_scalar(VegaT256ScalarV1::from_u64(value)));
        }
        point
    }
}

#[test]
fn derived_plane_emits_exact_source_transpose_and_rejects_repeated_chunks() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    for ordinal in [0, 5_847] {
        let mut fixture = TestPreparedDifferenceDigitV1::new_v1(ordinal);
        for chunk in 0..32 {
            let encoded = fixture.values.emit_next_v1(chunk).unwrap();
            for (index, bytes) in encoded.as_slice_v1().chunks_exact(32).enumerate() {
                let v = usize::from(chunk) * 512 + index;
                let expected = match v {
                    0 => 7_u8,
                    1 => 9,
                    64 => 11,
                    16_383 => 13,
                    _ => 0,
                };
                assert!(bytes[..31].iter().all(|byte| *byte == 0));
                assert_eq!(bytes[31], expected);
            }
        }
        fixture.values.finish_v1().unwrap();
    }
    let mut fixture = TestPreparedDifferenceDigitV1::new_v1(0);
    fixture.values.emit_next_v1(0).unwrap();
    assert!(fixture.values.emit_next_v1(0).is_err());
    assert!(fixture.values.emit_next_v1(1).is_err());
    assert!(fixture.values.finish_v1().is_err());
}

#[test]
fn statements_bind_exact_origin_read_position_and_unemitted_values() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let mut fixture = TestPreparedDifferenceDigitV1::new_v1(0);
    for ordinal in [0, 16, 17, 135, 5_847] {
        let statement = fixture.statement_v1(ordinal);
        let after = (usize::from(ordinal) / 17 + 1) * 64;
        let (record, block) = ((after / 512) as u16, (after % 512) as u16);
        statement.require_ordinal_v1(ordinal).unwrap();
        statement
            .validate_origin_and_read_position_v1([0x61; 32], [0x62; 32], record, block)
            .unwrap();
        for (r, s, a, b) in [
            ([0x60; 32], [0x62; 32], record, block),
            ([0x61; 32], [0x63; 32], record, block),
            ([0x61; 32], [0x62; 32], record, block + 1),
            ([0x61; 32], [0x62; 32], record + 1, block),
        ] {
            assert!(
                statement
                    .validate_origin_and_read_position_v1(r, s, a, b)
                    .is_err()
            );
        }
        assert!(statement.require_ordinal_v1(ordinal + 1).is_err());
    }
    assert!(
        fixture
            .statement_v1(5_848)
            .require_ordinal_v1(5_848)
            .is_err()
    );
    assert!(
        fixture
            .statement_v1(0)
            .commitment_v1(&VegaT256ScalarV1::zero())
            .is_err()
    );
    let rho = VegaT256ScalarV1::from_u64(17);
    let commitment = fixture.statement_v1(0).commitment_v1(&rho).unwrap();
    assert!(commitment.equals(&TestPreparedDifferenceDigitV1::expected_commitment_v1(rho)));
    fixture.values.emit_next_v1(0).unwrap();
    assert!(fixture.statement_v1(0).commitment_v1(&rho).is_err());
}

#[test]
fn driver_requires_exact_top_stage_and_full_source_schedule_without_public_secrets() {
    let source = include_str!("prepared_difference_digit_plane_v1.rs")
        .split_whitespace()
        .collect::<String>();
    let begin = source
        .split("fninto_difference_digit_preparation_v1(")
        .nth(1)
        .unwrap()
        .split("impl<R:")
        .next()
        .unwrap();
    assert!(
        begin
            .find("validate_difference_digit_preparation_start_v1")
            .unwrap()
            < begin
                .find("Phase23GlobalLookupRadixSourceCursorV2::begin_v2")
                .unwrap()
    );
    assert!(begin.contains("self.evidence.take()"));
    assert!(source.contains("live.cursor.commit_prepared_difference_digit_v1(&statement)?"));
    assert!(source.contains("live.cursor.complete_authenticated_source_replay_v1()?"));
    assert!(source.contains("evidence.validate_difference_digit_preparation_complete_v1()?"));
    assert!(!source.contains("fninto_parts"));
    assert!(!source.contains("pubvalues:"));
}

#[test]
fn poisoned_difference_owners_cannot_prepare_emit_finish_or_recover_source() {
    assert!(
        DifferenceDigitPreparationV1::<core::convert::Infallible, (), ()> { live: None }
            .prepare_next_v1()
            .is_err()
    );
    assert!(
        DifferenceDigitPreparationV1::<core::convert::Infallible, (), ()> { live: None }
            .finish_v1()
            .is_err()
    );
    let mut plane =
        PreparedDifferenceDigitPlaneV1::<core::convert::Infallible, (), ()> { live: None };
    assert!(plane.emit_next_value_chunk_v1(0).is_err());
    assert!(plane.finish_v1().is_err());
}
