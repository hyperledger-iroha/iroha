//! Sparse public group oracles for actual compact-bit expansion and secret MSM.
use super::*;
use crate::generalized_bulletproof::ProofSuite;
use crate::vega::{VegaT256PointV1, bulletproof_t256::ZkAmsT256BulletproofSuiteV1};
use core::ops::{Add, Sub};

const SPARSE_POSITIONS_V1: [usize; 7] = [0, 63, 64, 255, 256, 8_191, 16_383];
// Arithmetic fixture only: it mints no source, replay or production authority.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct TestPreparedComparatorV1
{
    values: PreparedRadixValuesV1,
}
impl TestPreparedComparatorV1 {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn new_v1()
    -> Self {
        Self::for_ordinal_v1(0)
    }
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn for_ordinal_v1(
        ordinal: u16,
    ) -> Self {
        assert!(ordinal < COMPARATOR_PLANE_COUNT_V1);
        let coordinate = comparator_coordinate_v1(ordinal).unwrap();
        let mut packed = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
        for position in SPARSE_POSITIONS_V1 {
            packed.as_mut_slice_v1()[position] = 1 << coordinate.bit;
        }
        Self {
            values: expand_comparator_values_v1(packed, coordinate).unwrap(),
        }
    }
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn statement_v1(
        &self,
        ordinal: u16,
    ) -> PreparedComparatorStatementV1<'_> {
        PreparedComparatorStatementV1 {
            values: &self.values,
            ordinal,
            replay_record_digest: [0x61; 32],
            source_receipt_digest: [0x62; 32],
        }
    }
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn expected_v1(
        rho: VegaT256ScalarV1,
    ) -> VegaT256PointV1 {
        let generators = ZkAmsT256BulletproofSuiteV1::generators()
            .reduce(16_384)
            .unwrap();
        let mut point = generators.h.mul_scalar(rho);
        for position in SPARSE_POSITIONS_V1 {
            point = point.add(generators.g_bold[position]);
        }
        point
    }
}

#[test]
fn actual_top_bit_values_match_independent_sparse_group_equation_and_late_coordinate() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let fixture = TestPreparedComparatorV1::new_v1();
    let rho = VegaT256ScalarV1::from_u64(19);
    let commitment = fixture.statement_v1(0).commitment_v1(&rho).unwrap();
    let expected = TestPreparedComparatorV1::expected_v1(rho);
    assert!(commitment.equals(&expected));
    assert!(!commitment.equals(&TestPreparedComparatorV1::expected_v1(
        VegaT256ScalarV1::from_u64(20)
    )));
    let generators = ZkAmsT256BulletproofSuiteV1::generators()
        .reduce(16_384)
        .unwrap();
    assert!(!commitment.equals(&expected.sub(generators.g_bold[16_383])));
    assert!(
        !commitment.equals(
            &expected
                .sub(generators.g_bold[63])
                .add(generators.g_bold[1])
        )
    );
}

#[test]
fn statement_origin_order_zero_blinding_and_emission_fail_before_commitment() {
    let _guard = super::super::tests::radix_witness_test_guard_v2();
    let mut fixture = TestPreparedComparatorV1::new_v1();
    for ordinal in [0, 343, 344, 687, 688, 6_879, 6_880, 7_223] {
        let statement = fixture.statement_v1(ordinal);
        statement
            .validate_origin_v1([0x61; 32], [0x62; 32])
            .unwrap();
        statement.require_ordinal_v1(ordinal).unwrap();
        assert!(statement.require_ordinal_v1(ordinal + 1).is_err());
        assert!(
            statement
                .validate_origin_v1([0x63; 32], [0x62; 32])
                .is_err()
        );
        assert!(
            statement
                .validate_origin_v1([0x61; 32], [0x63; 32])
                .is_err()
        );
        assert!(statement.validate_origin_v1([0; 32], [0x62; 32]).is_err());
        assert!(statement.validate_origin_v1([0x61; 32], [0; 32]).is_err());
    }
    assert!(
        fixture
            .statement_v1(7_224)
            .require_ordinal_v1(7_224)
            .is_err()
    );
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
            .commitment_v1(&VegaT256ScalarV1::from_u64(19))
            .is_err()
    );
    assert!(fixture.values.emit_next_v1(0).is_err());
    assert!(
        fixture
            .statement_v1(0)
            .commitment_v1(&VegaT256ScalarV1::from_u64(19))
            .is_err()
    );
}
