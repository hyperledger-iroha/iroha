//! Consuming centering-difference preparation from the original source.
//!
//! The exact completed top-bit stage enters one canonical source reread. Each
//! group binds the sealed comparator lanes to the original coefficients before
//! deriving its 17 base-2^15 subtraction digits. The original session samples
//! and retains rho and adopts the actual MSM for inventory 12,728..18,576.
//! Value chunks alone do not supply stored opening tails or a proof.
use super::prepared_comparator_plane_v1::{
    PreparedRadixValuesV1, validate_materialized_context_v1,
};
use super::prepared_low_digit_plane_v1::{LowDigitGroupV1, read_low_digit_group_v1};
use super::*;
use crate::vega::{VegaT256ScalarV1, bulletproof_t256::ZeroizingT256ScalarVecV1};

const DIFFERENCE_PLANES_PER_GROUP_V1: usize = RADIX_LOW_LIMBS_V2;
const DIFFERENCE_PLANE_COUNT_V1: u16 =
    (RADIX_GROUP_COUNT_V2 * DIFFERENCE_PLANES_PER_GROUP_V1) as u16;
const _: () = {
    assert!(DIFFERENCE_PLANE_COUNT_V1 == 5_848);
    assert!(RADIX_LOW_LIMBS_V2 * 15 == 255);
    assert!(
        RADIX_SOURCE_BLOCKS_PER_GROUP_V2 * RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2
            == RADIX_COEFFICIENTS_PER_GROUP_V2
    );
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DifferenceDigitCoordinateV1 {
    group: u16,
    digit: u8,
}

fn difference_digit_coordinate_v1(
    ordinal: u16,
) -> Result<DifferenceDigitCoordinateV1, ZkAmsMkheErrorV1> {
    if ordinal >= DIFFERENCE_PLANE_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let within_group = usize::from(ordinal) % DIFFERENCE_PLANES_PER_GROUP_V1;
    Ok(DifferenceDigitCoordinateV1 {
        group: ordinal / DIFFERENCE_PLANES_PER_GROUP_V1 as u16,
        digit: (within_group % RADIX_LOW_LIMBS_V2) as u8,
    })
}

struct DifferenceDigitPreparationLiveV1<R, K, P> {
    // Evidence is held solely by cursor until completion, never duplicated.
    source: Phase23RadixWitnessMaterializedV2<R, K, P>,
    cursor: Phase23GlobalLookupRadixSourceCursorV2<R, K, P>,
    group: Option<DifferenceDigitGroupV1>,
    next_plane: u16,
}

/// Sole original source and its strict canonical reread, consumed plane by plane.
#[must_use = "dropping this preparation closes the original source and group values"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct DifferenceDigitPreparationV1
<R, K, P> {
    live: Option<DifferenceDigitPreparationLiveV1<R, K, P>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> Phase23RadixWitnessMaterializedV2<R, K, P> {
    /// Begin delta only after all 688 original top-bit commitments.
    /// Preparation samples/adopts each point before its values can be emitted.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn into_difference_digit_preparation_v1(
        mut self,
    ) -> Result<DifferenceDigitPreparationV1<R, K, P>, ZkAmsMkheErrorV1> {
        if usize::from(self.next_comparator_plane) != 2 * RADIX_GROUP_COUNT_V2 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        self.evidence
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .validate_difference_digit_preparation_start_v1(
                self.record.replay_record_digest,
                self.record.source_receipt_digest,
            )?;
        validate_materialized_context_v1(&self)?;
        // Validate record, original lineage, seal and snapshot before allocation
        // or source I/O. The strict cursor takes the original evidence exactly once.
        let evidence = self
            .evidence
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let (cursor, axes) = Phase23GlobalLookupRadixSourceCursorV2::begin_v2(evidence)?;
        if axes.replay_record_digest != self.record.replay_record_digest
            || axes.source_receipt_digest != self.record.source_receipt_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(DifferenceDigitPreparationV1 {
            live: Some(DifferenceDigitPreparationLiveV1 {
                source: self,
                cursor,
                group: None,
                next_plane: 0,
            }),
        })
    }
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> DifferenceDigitPreparationV1<R, K, P> {
    /// Consume this driver into the next complete value plane in canonical order.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn prepare_next_v1(
        mut self,
    ) -> Result<PreparedDifferenceDigitPlaneV1<R, K, P>, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let coordinate = difference_digit_coordinate_v1(live.next_plane)?;
        if usize::from(live.next_plane) % DIFFERENCE_PLANES_PER_GROUP_V1 == 0 {
            if live.group.is_some() {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            let source_group = read_low_digit_group_v1(&mut live.cursor, coordinate.group)?;
            let record = usize::from(coordinate.group) / RADIX_GROUPS_PER_RECORD_V2;
            let group = usize::from(coordinate.group) % RADIX_GROUPS_PER_RECORD_V2;
            let mut read_lane = |lane| -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
                live.source
                    .snapshot
                    .read_slot_v1(
                        radix_witness_slot_v2(record, group, lane)?,
                        live.source.record.spool_context_digest,
                    )
                    .map_err(map_spool_error_v2)
            };
            let lanes = [read_lane(0)?, read_lane(1)?, read_lane(2)?];
            live.group = Some(DifferenceDigitGroupV1::from_authenticated_group_v1(
                source_group,
                lanes,
            )?);
        }
        let group = live
            .group
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let values = group.prepare_values_v1(coordinate)?;
        let statement = PreparedDifferenceDigitStatementV1 {
            values: &values,
            ordinal: live.next_plane,
            replay_record_digest: live.source.record.replay_record_digest,
            source_receipt_digest: live.source.record.source_receipt_digest,
        };
        // The original cursor/session consumes the actual source-bound values
        // before they can be emitted. It alone samples/adopts the matching rho/C.
        live.cursor
            .commit_prepared_difference_digit_v1(&statement)?;
        Ok(PreparedDifferenceDigitPlaneV1 {
            live: Some(PreparedDifferenceDigitPlaneLiveV1 {
                driver: live,
                values,
            }),
        })
    }

    /// Return the sole original source only after every plane and every source
    /// read. The same session retains the computed delta points and blindings;
    /// completion does not assert successful plane storage or a proof.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn finish_v1(
        mut self,
    ) -> Result<Phase23RadixWitnessMaterializedV2<R, K, P>, ZkAmsMkheErrorV1> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.next_plane != DIFFERENCE_PLANE_COUNT_V1 || live.group.is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let (evidence, schedule) = live.cursor.complete_authenticated_source_replay_v1()?;
        let mut source = live.source;
        if source.evidence.is_some()
            || usize::from(source.next_comparator_plane) != 2 * RADIX_GROUP_COUNT_V2
            || schedule != source.record.authenticated_read_schedule_root
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        evidence.validate_radix_materialization_source_v1(
            source.record.replay_record_digest,
            source.record.source_receipt_digest,
        )?;
        evidence.validate_difference_digit_preparation_complete_v1()?;
        validate_materialized_context_v1(&source)?;
        source.evidence = Some(evidence);
        Ok(source)
    }
}

struct PreparedDifferenceDigitPlaneLiveV1<R, K, P> {
    driver: DifferenceDigitPreparationLiveV1<R, K, P>,
    values: PreparedRadixValuesV1,
}

/// Move-only prepared centering-difference values; no point, blinding or value-vector accessor.
#[must_use = "dropping prepared low digits closes all retained source and values"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct PreparedDifferenceDigitPlaneV1
<R, K, P> {
    live: Option<PreparedDifferenceDigitPlaneLiveV1<R, K, P>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> PreparedDifferenceDigitPlaneV1<R, K, P> {
    /// Emit the next of exactly 32 canonical 512-scalar chunks. Wrong order,
    /// overrun, allocation failure or unwind drops source, group and plane.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn emit_next_value_chunk_v1(
        &mut self,
        expected_chunk: u8,
    ) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let chunk = live.values.emit_next_v1(expected_chunk)?;
        self.live = Some(live);
        Ok(chunk)
    }

    /// Advance only after all chunks; an incomplete finish consumes everything.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn finish_v1(
        mut self,
    ) -> Result<DifferenceDigitPreparationV1<R, K, P>, ZkAmsMkheErrorV1> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        live.values.finish_v1()?;
        let mut driver = live.driver;
        let coordinate = difference_digit_coordinate_v1(driver.next_plane)?;
        if driver.group.as_ref().map(|group| group.source.group) != Some(coordinate.group) {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        driver.next_plane = driver
            .next_plane
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if usize::from(driver.next_plane) % DIFFERENCE_PLANES_PER_GROUP_V1 == 0 {
            drop(driver.group.take());
        }
        Ok(DifferenceDigitPreparationV1 { live: Some(driver) })
    }
}

/// Borrowed statement created only from the real source driver's private values.
/// It exposes no vector, scalar, point, session or context-splitting getter.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct PreparedDifferenceDigitStatementV1
<'a> {
    values: &'a PreparedRadixValuesV1,
    ordinal: u16,
    replay_record_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
}

impl PreparedDifferenceDigitStatementV1<'_> {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn validate_origin_and_read_position_v1(
        &self,
        replay_record_digest: [u8; 32],
        source_receipt_digest: [u8; 32],
        next_record: u16,
        next_block: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let coordinate = difference_digit_coordinate_v1(self.ordinal)?;
        let after_group = (usize::from(coordinate.group) + 1) * RADIX_SOURCE_BLOCKS_PER_GROUP_V2;
        if self.replay_record_digest != replay_record_digest
            || self.source_receipt_digest != source_receipt_digest
            || replay_record_digest == [0; 32]
            || source_receipt_digest == [0; 32]
            || usize::from(next_record) != after_group / PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1
            || usize::from(next_block) != after_group % PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn require_ordinal_v1(
        &self,
        expected: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        difference_digit_coordinate_v1(self.ordinal)?;
        if self.ordinal != expected {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn commitment_v1(
        &self,
        blinding: &VegaT256ScalarV1,
    ) -> Result<
        crate::generalized_bulletproof::SecretPoint<crate::vega::VegaT256PointV1>,
        ZkAmsMkheErrorV1,
    > {
        self.values.commitment_v1(blinding)
    }
}

// The source vector remains in source (block, coefficient) order. Packed lanes
// already use v = coefficient * 64 + block; only source reads are transposed.
struct DifferenceDigitGroupV1 {
    source: LowDigitGroupV1,
    lanes: [ConfidentialSpoolChunkV1; RADIX_PACKED_LANES_PER_GROUP_V2],
}

impl DifferenceDigitGroupV1 {
    fn from_authenticated_group_v1(
        source: LowDigitGroupV1,
        lanes: [ConfidentialSpoolChunkV1; RADIX_PACKED_LANES_PER_GROUP_V2],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if usize::from(source.group) >= RADIX_GROUP_COUNT_V2
            || source.values.len() != RADIX_COEFFICIENTS_PER_GROUP_V2
            || lanes
                .iter()
                .any(|lane| lane.as_slice_v1().len() != RADIX_COEFFICIENTS_PER_GROUP_V2)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        // Check every packed bit, including the final coefficient and reserved
        // lane-2 bits, against a freshly derived canonical source witness.
        let mut invalid = RadixSecretCopyV2::new(0_u8);
        for coefficient in 0..RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 {
            for block in 0..RADIX_SOURCE_BLOCKS_PER_GROUP_V2 {
                let scalar = &source.values.as_slice()
                    [block * RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 + coefficient];
                let mut encoded = RadixSecretBytesV2::zeroed_v2();
                scalar.write_le_bytes_ref(encoded.as_mut_v2());
                encoded.as_mut_v2().reverse();
                let witness = radix_coefficient_witness_v2(encoded.as_ref_v2())?;
                let packed = pack_comparator_lanes_v2(&witness)?;
                let v = coefficient * RADIX_SOURCE_BLOCKS_PER_GROUP_V2 + block;
                for (lane, expected) in lanes.iter().zip(packed.0.iter()) {
                    invalid.or_assign_v2(lane.as_slice_v1()[v] ^ expected);
                }
            }
        }
        if *invalid.as_ref_v2() != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(Self { source, lanes })
    }

    fn prepare_values_v1(
        &self,
        coordinate: DifferenceDigitCoordinateV1,
    ) -> Result<PreparedRadixValuesV1, ZkAmsMkheErrorV1> {
        if self.source.group != coordinate.group
            || usize::from(coordinate.digit) >= RADIX_LOW_LIMBS_V2
            || self.source.values.len() != RADIX_COEFFICIENTS_PER_GROUP_V2
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut values =
            ZeroizingT256ScalarVecV1::try_with_exact_capacity(RADIX_COEFFICIENTS_PER_GROUP_V2)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for coefficient in 0..RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 {
            for block in 0..RADIX_SOURCE_BLOCKS_PER_GROUP_V2 {
                let scalar = &self.source.values.as_slice()
                    [block * RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 + coefficient];
                let v = coefficient * RADIX_SOURCE_BLOCKS_PER_GROUP_V2 + block;
                let packed = RadixPackedComparatorV2([
                    self.lanes[0].as_slice_v1()[v],
                    self.lanes[1].as_slice_v1()[v],
                    self.lanes[2].as_slice_v1()[v],
                ]);
                values.push(difference_digit_scalar_v1(
                    scalar,
                    &packed,
                    coordinate.digit,
                )?);
            }
        }
        PreparedRadixValuesV1::from_exact_values_v1(values)
    }
}

fn difference_digit_scalar_v1(
    scalar: &VegaT256ScalarV1,
    packed: &RadixPackedComparatorV2,
    digit: u8,
) -> Result<VegaT256ScalarV1, ZkAmsMkheErrorV1> {
    if usize::from(digit) >= RADIX_LOW_LIMBS_V2 || packed.0[2] & 0xe0 != 0 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut encoded = RadixSecretBytesV2::zeroed_v2();
    scalar.write_le_bytes_ref(encoded.as_mut_v2());
    encoded.as_mut_v2().reverse();
    let mut low = RadixSecretCopyV2::new(0_u16);
    let mut threshold = 0_u16;
    for offset in 0..15 {
        let position = usize::from(digit) * 15 + offset;
        let bit = bit_le_from_be_v2(encoded.as_ref_v2(), position);
        low.or_assign_v2(u16::from(*bit.as_ref_v2()) << offset);
        threshold |=
            u16::from(*bit_le_from_be_v2(&RADIX_CENTERING_THRESHOLD_BE_V2, position).as_ref_v2())
                << offset;
    }
    let borrow = |h: u8| {
        let (lane, bit) = if h < 6 {
            (0, h + 2)
        } else if h < 14 {
            (1, h - 6)
        } else {
            (2, h - 14)
        };
        RadixSecretCopyV2::new(u16::from((packed.0[lane] >> bit) & 1))
    };
    let prior = if digit == 0 {
        RadixSecretCopyV2::new(0_u16)
    } else {
        borrow(digit - 1)
    };
    let current = borrow(digit);
    // A 15-bit digit plus B times one bit is at most 65,535. A malformed
    // borrow must reject as an integer, without field or wrapping subtraction.
    let left = RadixSecretCopyV2::new(*low.as_ref_v2() + RADIX_BASE_V2 * *current.as_ref_v2());
    let right = RadixSecretCopyV2::new(threshold + *prior.as_ref_v2());
    let delta = RadixSecretCopyV2::new(
        left.as_ref_v2()
            .checked_sub(*right.as_ref_v2())
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
    );
    if *delta.as_ref_v2() >= RADIX_BASE_V2 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(VegaT256ScalarV1::from_u64(u64::from(*delta.as_ref_v2())))
}

#[cfg(test)]
#[path = "prepared_difference_digit_plane_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
use tests::TestPreparedDifferenceDigitV1;
