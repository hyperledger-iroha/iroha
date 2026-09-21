//! Consuming D/S low-digit preparation from the original authenticated source.
//!
//! This owner rereads each source block once, retains one zeroizing group, and
//! derives the existing group-major D[0..17], S[0..17] candidate value order.
//! Before emission, the retained original session samples rho and computes the
//! matching secret MSM. The original source returns only after all 11,696 planes
//! and the full source schedule. This does not supply stored planes or a proof.
use super::prepared_comparator_plane_v1::{
    PreparedRadixValuesV1, validate_materialized_context_v1,
};
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::ZkAmsPhase23RnsLinkSecretChunkV1;
use crate::vega::{VegaT256ScalarV1, bulletproof_t256::ZeroizingT256ScalarVecV1};

#[path = "prepared_low_digit_plane_v1/low_digit_workspace_v1.rs"]
mod low_digit_workspace_v1;
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) use low_digit_workspace_v1::{LowDigitWorkspaceV1, LowDigitWorkspaceErrorV1};
use low_digit_workspace_v1::LowDigitPreparationRefusalV1;

const LOW_PLANES_PER_GROUP_V1: usize = 2 * RADIX_LOW_LIMBS_V2;
const LOW_PLANE_COUNT_V1: u16 = (RADIX_GROUP_COUNT_V2 * LOW_PLANES_PER_GROUP_V1) as u16;
const _: () = {
    assert!(LOW_PLANE_COUNT_V1 == 11_696);
    assert!(RADIX_LOW_LIMBS_V2 * 15 == 255);
    assert!(
        RADIX_SOURCE_BLOCKS_PER_GROUP_V2 * RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2
            == RADIX_COEFFICIENTS_PER_GROUP_V2
    );
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LowDigitCoordinateV1 {
    group: u16,
    slack: bool,
    digit: u8,
}

fn low_digit_coordinate_v1(ordinal: u16) -> Result<LowDigitCoordinateV1, ZkAmsMkheErrorV1> {
    if ordinal >= LOW_PLANE_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let within_group = usize::from(ordinal) % LOW_PLANES_PER_GROUP_V1;
    Ok(LowDigitCoordinateV1 {
        group: ordinal / LOW_PLANES_PER_GROUP_V1 as u16,
        slack: within_group >= RADIX_LOW_LIMBS_V2,
        digit: (within_group % RADIX_LOW_LIMBS_V2) as u8,
    })
}

struct LowDigitPreparationLiveV1<R, K, P> {
    // Evidence is held solely by cursor until completion, never duplicated.
    source: Phase23RadixWitnessMaterializedV2<R, K, P>,
    cursor: Phase23GlobalLookupRadixSourceCursorV2<R, K, P>,
    group: Option<LowDigitGroupV1>,
    next_plane: u16,
    // Declared after group: secret vector destruction precedes credit release.
    _workspace: LowDigitWorkspaceV1,
}

/// Sole original source and its strict canonical reread, consumed plane by plane.
#[must_use = "dropping this preparation closes the original source and group values"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct LowDigitPreparationV1
<R, K, P> {
    live: Option<LowDigitPreparationLiveV1<R, K, P>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> Phase23RadixWitnessMaterializedV2<R, K, P> {
    /// Reserve both original scalar-vector lifetimes before source reads or
    /// allocation. Only pre-allocation capacity refusal retains a retry owner.
    #[allow(
        clippy::result_large_err,
        reason = "capacity refusal retains the entire original source without allocating"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn into_low_digit_preparation_v1(
        mut self,
    ) -> Result<LowDigitPreparationV1<R, K, P>, LowDigitPreparationRefusalV1<R, K, P>> {
        let admission = (|| {
            if self.next_comparator_plane != 0 {
                return Err(LowDigitWorkspaceErrorV1::Source);
            }
            self.evidence
                .as_ref()
                .ok_or(LowDigitWorkspaceErrorV1::Source)?
                .validate_low_digit_preparation_start_v1(
                    self.record.replay_record_digest,
                    self.record.source_receipt_digest,
                )
                .map_err(|_| LowDigitWorkspaceErrorV1::Source)?;
            validate_materialized_context_v1(&self)
                .map_err(|_| LowDigitWorkspaceErrorV1::Source)?;
            self.evidence
                .as_mut()
                .ok_or(LowDigitWorkspaceErrorV1::Source)?
                .admit_low_digit_workspace_v1()
        })();
        let workspace = match admission {
            Ok(workspace) => workspace,
            Err(reason) => {
                return Err(LowDigitPreparationRefusalV1 {
                    reason,
                    source: if reason == LowDigitWorkspaceErrorV1::Capacity {
                        Some(self)
                    } else {
                        None
                    },
                });
            }
        };
        // Admission is attached before cursor creation; failure cannot detach a
        // source from its funded lifetime. No entropy or ticket is advanced.
        let result = (|| {
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
            Ok(LowDigitPreparationV1 {
                live: Some(LowDigitPreparationLiveV1 {
                    source: self,
                    cursor,
                    group: None,
                    next_plane: 0,
                    _workspace: workspace,
                }),
            })
        })();
        result.map_err(|_| LowDigitPreparationRefusalV1 {
            reason: LowDigitWorkspaceErrorV1::Source,
            source: None,
        })
    }
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> LowDigitPreparationV1<R, K, P> {
    /// Consume this driver into the next complete value plane in canonical order.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn prepare_next_v1(
        mut self,
    ) -> Result<PreparedLowDigitPlaneV1<R, K, P>, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let coordinate = low_digit_coordinate_v1(live.next_plane)?;
        if usize::from(live.next_plane) % LOW_PLANES_PER_GROUP_V1 == 0 {
            if live.group.is_some() {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            live.group = Some(read_low_digit_group_v1(&mut live.cursor, coordinate.group)?);
        }
        let group = live
            .group
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let values = group.prepare_values_v1(coordinate)?;
        let statement = PreparedLowDigitStatementV1 {
            values: &values,
            ordinal: live.next_plane,
            replay_record_digest: live.source.record.replay_record_digest,
            source_receipt_digest: live.source.record.source_receipt_digest,
        };
        // The original cursor/session consumes the actual source-bound values
        // before they can be emitted. It alone samples/adopts the matching rho/C.
        live.cursor.commit_prepared_low_digit_v1(&statement)?;
        Ok(PreparedLowDigitPlaneV1 {
            live: Some(PreparedLowDigitPlaneLiveV1 {
                driver: live,
                values,
            }),
        })
    }

    /// Return the sole original source only after every plane and every source
    /// read. The same session retains the computed D/S points and blindings;
    /// completion does not assert successful plane storage or a proof.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn finish_v1(
        mut self,
    ) -> Result<Phase23RadixWitnessMaterializedV2<R, K, P>, ZkAmsMkheErrorV1> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.next_plane != LOW_PLANE_COUNT_V1 || live.group.is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let (evidence, schedule) = live.cursor.complete_authenticated_source_replay_v1()?;
        let mut source = live.source;
        if source.evidence.is_some()
            || source.next_comparator_plane != 0
            || schedule != source.record.authenticated_read_schedule_root
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        evidence.validate_radix_materialization_source_v1(
            source.record.replay_record_digest,
            source.record.source_receipt_digest,
        )?;
        validate_materialized_context_v1(&source)?;
        source.evidence = Some(evidence);
        Ok(source)
    }
}

struct PreparedLowDigitPlaneLiveV1<R, K, P> {
    // This outer plane must drop before its driver releases the reservation.
    values: PreparedRadixValuesV1,
    driver: LowDigitPreparationLiveV1<R, K, P>,
}

/// Move-only prepared D/S-low values; no point, blinding or value-vector accessor.
#[must_use = "dropping prepared low digits closes all retained source and values"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct PreparedLowDigitPlaneV1
<R, K, P> {
    live: Option<PreparedLowDigitPlaneLiveV1<R, K, P>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> PreparedLowDigitPlaneV1<R, K, P> {
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
    ) -> Result<LowDigitPreparationV1<R, K, P>, ZkAmsMkheErrorV1> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        live.values.finish_v1()?;
        let mut driver = live.driver;
        let coordinate = low_digit_coordinate_v1(driver.next_plane)?;
        if driver.group.as_ref().map(|group| group.group) != Some(coordinate.group) {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        driver.next_plane = driver
            .next_plane
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if usize::from(driver.next_plane) % LOW_PLANES_PER_GROUP_V1 == 0 {
            drop(driver.group.take());
        }
        Ok(LowDigitPreparationV1 { live: Some(driver) })
    }
}

/// Borrowed statement created only from the real source driver's private values.
/// It exposes no vector, scalar, point, session or context-splitting getter.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct PreparedLowDigitStatementV1
<'a> {
    values: &'a PreparedRadixValuesV1,
    ordinal: u16,
    replay_record_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
}

impl PreparedLowDigitStatementV1<'_> {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn validate_origin_and_read_position_v1(
        &self,
        replay_record_digest: [u8; 32],
        source_receipt_digest: [u8; 32],
        next_record: u16,
        next_block: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let coordinate = low_digit_coordinate_v1(self.ordinal)?;
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
        expected: u32,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        low_digit_coordinate_v1(self.ordinal)?;
        if u32::from(self.ordinal) != expected {
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

// Values retain authenticated source order (block, coefficient); projection
// applies the existing v=coefficient*64+block permutation exactly once.
pub(super) struct LowDigitGroupV1 {
    pub(super) group: u16,
    pub(super) values: ZeroizingT256ScalarVecV1,
}

pub(super) fn read_low_digit_group_v1<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P>(
    cursor: &mut Phase23GlobalLookupRadixSourceCursorV2<R, K, P>,
    group: u16,
) -> Result<LowDigitGroupV1, ZkAmsMkheErrorV1> {
    if usize::from(group) >= RADIX_GROUP_COUNT_V2 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let record = usize::from(group) / RADIX_GROUPS_PER_RECORD_V2;
    let first_block =
        (usize::from(group) % RADIX_GROUPS_PER_RECORD_V2) * RADIX_SOURCE_BLOCKS_PER_GROUP_V2;
    let mut values =
        ZeroizingT256ScalarVecV1::try_with_exact_capacity(RADIX_COEFFICIENTS_PER_GROUP_V2)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for local_block in 0..RADIX_SOURCE_BLOCKS_PER_GROUP_V2 {
        let chunk = cursor.read_next_canonical_block_v2(record, first_block + local_block)?;
        values = append_canonical_group_block_v1(values, chunk, local_block)?;
    }
    Ok(LowDigitGroupV1 { group, values })
}

fn append_canonical_group_block_v1(
    mut values: ZeroizingT256ScalarVecV1,
    mut chunk: ZkAmsPhase23RnsLinkSecretChunkV1,
    local_block: usize,
) -> Result<ZeroizingT256ScalarVecV1, ZkAmsMkheErrorV1> {
    if local_block >= RADIX_SOURCE_BLOCKS_PER_GROUP_V2
        || values.len() != local_block * RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2
        || chunk.as_mut_bytes_v1().len() != PHASE23_MAIN_BLOCK_BYTES_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    for encoded in chunk.as_mut_bytes_v1().chunks_exact(32) {
        let encoded: &[u8; 32] = encoded
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        values.push(
            VegaT256ScalarV1::from_be_bytes_exact_ref(encoded)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        );
    }
    Ok(values)
}

impl LowDigitGroupV1 {
    fn prepare_values_v1(
        &self,
        coordinate: LowDigitCoordinateV1,
    ) -> Result<PreparedRadixValuesV1, ZkAmsMkheErrorV1> {
        if self.group != coordinate.group
            || usize::from(self.group) >= RADIX_GROUP_COUNT_V2
            || usize::from(coordinate.digit) >= RADIX_LOW_LIMBS_V2
            || self.values.len() != RADIX_COEFFICIENTS_PER_GROUP_V2
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut values =
            ZeroizingT256ScalarVecV1::try_with_exact_capacity(RADIX_COEFFICIENTS_PER_GROUP_V2)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for coefficient in 0..RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 {
            for block in 0..RADIX_SOURCE_BLOCKS_PER_GROUP_V2 {
                let scalar = &self.values.as_slice()
                    [block * RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 + coefficient];
                values.push(low_digit_scalar_v1(
                    scalar,
                    coordinate.slack,
                    coordinate.digit,
                )?);
            }
        }
        PreparedRadixValuesV1::from_exact_values_v1(values)
    }
}

fn low_digit_scalar_v1(
    scalar: &VegaT256ScalarV1,
    slack: bool,
    digit: u8,
) -> Result<VegaT256ScalarV1, ZkAmsMkheErrorV1> {
    if usize::from(digit) >= RADIX_LOW_LIMBS_V2 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut encoded = RadixSecretBytesV2::zeroed_v2();
    scalar.write_le_bytes_ref(encoded.as_mut_v2());
    encoded.as_mut_v2().reverse();
    let mut complement = RadixSecretBytesV2::zeroed_v2();
    let bytes = if slack {
        if *fixed_subtract_be_v2(
            &RADIX_MODULUS_MINUS_ONE_BE_V2,
            encoded.as_ref_v2(),
            complement.as_mut_v2(),
        )
        .as_ref_v2()
            != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        complement.as_ref_v2()
    } else {
        encoded.as_ref_v2()
    };
    let mut low = RadixSecretCopyV2::new(0_u16);
    for offset in 0..15 {
        let bit = bit_le_from_be_v2(bytes, usize::from(digit) * 15 + offset);
        low.or_assign_v2(u16::from(*bit.as_ref_v2()) << offset);
    }
    Ok(VegaT256ScalarV1::from_u64(u64::from(*low.as_ref_v2())))
}

#[cfg(test)]
#[path = "prepared_low_digit_plane_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "prepared_low_digit_commitment_v1_tests.rs"]
mod commitment_tests;
#[cfg(test)]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
use commitment_tests::TestPreparedLowDigitV1;
