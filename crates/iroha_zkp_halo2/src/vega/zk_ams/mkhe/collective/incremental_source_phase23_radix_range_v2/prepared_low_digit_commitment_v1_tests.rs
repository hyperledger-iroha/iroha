//! Actual nonzero low-digit/MSM relation and origin/order boundary controls.
use super::*;
use crate::generalized_bulletproof::ProofSuite;
use crate::vega::{VegaT256PointV1, bulletproof_t256::ZkAmsT256BulletproofSuiteV1};
use core::ops::{Add, Sub};

// An isolated arithmetic fixture, never source replay evidence or a provider.
// Values are derived by the real D-low projection from four source coefficients.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct TestPreparedLowDigitV1
{
    values: PreparedRadixValuesV1,
}
impl TestPreparedLowDigitV1 {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn new_v1()
    -> Self {
        let mut values = ZeroizingT256ScalarVecV1::try_with_exact_capacity(16_384).unwrap();
        for j in 0..16_384 {
            values.push(VegaT256ScalarV1::from_u64(match j {
                0 => 7,
                256 => 9,
                1 => 11,
                16_383 => 13,
                _ => 0,
            }));
        }
        let values = LowDigitGroupV1 { group: 0, values }
            .prepare_values_v1(low_digit_coordinate_v1(0).unwrap())
            .unwrap();
        Self { values }
    }
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn statement_v1(
        &self,
        ordinal: u16,
    ) -> PreparedLowDigitStatementV1<'_> {
        PreparedLowDigitStatementV1 {
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
        // Independent sparse public group equation. It does not call the secret
        // MSM, low-bit extractor, transpose loop or its builder.
        let mut expected = generators.h.mul_scalar(rho);
        for (v, scalar) in [(0, 7), (1, 9), (64, 11), (16_383, 13)] {
            expected =
                expected.add(generators.g_bold[v].mul_scalar(VegaT256ScalarV1::from_u64(scalar)));
        }
        expected
    }
}

#[test]
fn actual_nonzero_full_plane_uses_original_ordered_generators_and_one_rho_h() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let fixture = TestPreparedLowDigitV1::new_v1();
    let rho = VegaT256ScalarV1::from_u64(17);
    let commitment = fixture.statement_v1(0).commitment_v1(&rho).unwrap();
    assert!(commitment.equals(&TestPreparedLowDigitV1::expected_commitment_v1(rho)));
    assert!(
        !commitment.equals(&TestPreparedLowDigitV1::expected_commitment_v1(
            VegaT256ScalarV1::from_u64(18)
        ))
    );
    let generators = ZkAmsT256BulletproofSuiteV1::generators()
        .reduce(16_384)
        .unwrap();
    let wrong_order = TestPreparedLowDigitV1::expected_commitment_v1(rho)
        .sub(generators.g_bold[1].mul_scalar(VegaT256ScalarV1::from_u64(9)))
        .add(generators.g_bold[256].mul_scalar(VegaT256ScalarV1::from_u64(9)));
    assert!(!commitment.equals(&wrong_order));
    let changed =
        TestPreparedLowDigitV1::expected_commitment_v1(rho).add(generators.g_bold[16_383]);
    assert!(!commitment.equals(&changed));
}

#[test]
fn malformed_origin_read_position_and_wire_order_are_rejected_before_msm() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let fixture = TestPreparedLowDigitV1::new_v1();
    for (ordinal, record, block) in [
        (0, 0, 64),
        (33, 0, 64),
        (34, 0, 128),
        (271, 1, 0),
        (11_695, 43, 0),
    ] {
        let statement = fixture.statement_v1(ordinal);
        statement.require_ordinal_v1(u32::from(ordinal)).unwrap();
        statement
            .validate_origin_and_read_position_v1([0x61; 32], [0x62; 32], record, block)
            .unwrap();
        assert!(
            statement
                .require_ordinal_v1(u32::from(ordinal) + 1)
                .is_err()
        );
        assert!(
            statement
                .validate_origin_and_read_position_v1([0x63; 32], [0x62; 32], record, block)
                .is_err()
        );
        assert!(
            statement
                .validate_origin_and_read_position_v1([0x61; 32], [0x63; 32], record, block)
                .is_err()
        );
        assert!(
            statement
                .validate_origin_and_read_position_v1([0x61; 32], [0x62; 32], record + 1, block)
                .is_err()
        );
        assert!(
            statement
                .validate_origin_and_read_position_v1([0x61; 32], [0x62; 32], record, block + 1)
                .is_err()
        );
    }
    for ordinal in [11_696, u16::MAX] {
        let statement = fixture.statement_v1(ordinal);
        assert!(statement.require_ordinal_v1(u32::from(ordinal)).is_err());
        assert!(
            statement
                .validate_origin_and_read_position_v1([0x61; 32], [0x62; 32], 43, 0)
                .is_err()
        );
    }
    let mut statement = fixture.statement_v1(0);
    statement.replay_record_digest = [0; 32];
    assert!(
        statement
            .validate_origin_and_read_position_v1([0; 32], [0x62; 32], 0, 64)
            .is_err()
    );
    statement.replay_record_digest = [0x61; 32];
    statement.source_receipt_digest = [0; 32];
    assert!(
        statement
            .validate_origin_and_read_position_v1([0x61; 32], [0; 32], 0, 64)
            .is_err()
    );
}

#[test]
fn zero_blinding_and_already_emitted_or_poisoned_values_cannot_be_committed() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let mut fixture = TestPreparedLowDigitV1::new_v1();
    assert!(
        fixture
            .statement_v1(0)
            .commitment_v1(&VegaT256ScalarV1::zero())
            .is_err()
    );
    drop(fixture.values.emit_next_v1(0).unwrap());
    assert!(
        fixture
            .statement_v1(0)
            .commitment_v1(&VegaT256ScalarV1::from_u64(17))
            .is_err()
    );
    assert!(fixture.values.emit_next_v1(0).is_err());
    assert!(
        fixture
            .statement_v1(0)
            .commitment_v1(&VegaT256ScalarV1::from_u64(17))
            .is_err()
    );
}
