//! Consuming expansion of the original authenticated small signed source.
//!
//! Signed values and their negative magnitudes use natural source order. Each
//! matching commitment is admitted by the original session before any value
//! chunk escapes. The original admitted tail follows all 32 values; emitting it
//! alone does not construct an authenticated stored snapshot or a proof. The
//! consuming store method joins the actual source-owned ordered writer.
use super::prepared_comparator_plane_v1::{
    PreparedPlaneOpeningV1, PreparedRadixValuesV1, validate_materialized_context_v1,
};
use super::*;
use crate::vega::{
    VegaT256ScalarV1,
    bulletproof_t256::{ZeroizingT256ScalarCopyV1, ZeroizingT256ScalarVecV1},
    zk_ams::mkhe::global_lookup_statement_v1::{
        GlobalLookupCommitmentPhaseV1, GlobalLookupCommitmentPurposeV1,
        comparator_signed_coordinate_v1,
    },
};

const SMALL_SIGNED_FIRST_PLANE_V1: u16 = 7_224;
const SMALL_SIGNED_NEGATIVE_FIRST_PLANE_V1: u16 = 8_256;
const SMALL_SIGNED_AFTER_PLANE_V1: u16 = 9_288;
const SMALL_SIGNED_SOURCE_PLANES_V1: usize = 1_032;
const SMALL_SIGNED_AUTHENTICATED_READ_BYTES_V1: u64 = 2 * 1_032 * (16_384 + 16);
const _: () = {
    assert!(SMALL_SIGNED_SOURCE_PLANES_V1 == PHASE23_RECORD_COUNT_V1 * 3 * 8);
    assert!(SMALL_SIGNED_AFTER_PLANE_V1 - SMALL_SIGNED_FIRST_PLANE_V1 == 2_064);
    assert!(SMALL_SIGNED_AUTHENTICATED_READ_BYTES_V1 == 33_849_600);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SmallSignedPlaneCoordinateV1 {
    ordinal: u16,
    source_slot: u16,
    negative_magnitude: bool,
    bound: u8,
}

fn small_signed_plane_coordinate_v1(
    ordinal: u16,
) -> Result<SmallSignedPlaneCoordinateV1, ZkAmsMkheErrorV1> {
    if !(SMALL_SIGNED_FIRST_PLANE_V1..SMALL_SIGNED_AFTER_PLANE_V1).contains(&ordinal) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let negative_magnitude = ordinal >= SMALL_SIGNED_NEGATIVE_FIRST_PLANE_V1;
    let first = if negative_magnitude {
        SMALL_SIGNED_NEGATIVE_FIRST_PLANE_V1
    } else {
        SMALL_SIGNED_FIRST_PLANE_V1
    };
    let source_slot = ordinal - first;
    let coordinate = comparator_signed_coordinate_v1(u32::from(ordinal))?;
    let purpose = if negative_magnitude {
        GlobalLookupCommitmentPurposeV1::SmallNegativeMagnitude
    } else {
        GlobalLookupCommitmentPurposeV1::SmallSigned
    };
    if coordinate.phase != GlobalLookupCommitmentPhaseV1::ChallengeIndependent
        || coordinate.purpose != purpose
        || coordinate.purpose_ordinal != u32::from(source_slot)
        || coordinate.global_ordinal != 25_112 + u32::from(ordinal - SMALL_SIGNED_FIRST_PLANE_V1)
        || usize::from(source_slot) >= SMALL_SIGNED_SOURCE_PLANES_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(SmallSignedPlaneCoordinateV1 {
        ordinal,
        source_slot,
        negative_magnitude,
        bound: if (source_slot / 8) % 3 == 0 { 1 } else { 2 },
    })
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> Phase23RadixWitnessMaterializedV2<R, K, P> {
    /// Retain exact source/packing openings after every value and tail emission.
    /// This creates neither native40 source authority nor a same-opening proof.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn prepare_source_packing_openings_v1(
        mut self,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if self.next_comparator_plane != SMALL_SIGNED_AFTER_PLANE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        validate_materialized_context_v1(&self)?;
        self.evidence
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .prepare_source_packing_openings_v1(
                self.record.replay_record_digest,
                self.record.source_receipt_digest,
            )?;
        Ok(self)
    }

    /// Consume the source and prepare its next signed or negative-magnitude plane.
    /// The exact completed beta/m stage is required before the first source read.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn prepare_next_small_signed_plane_v1(
        mut self,
    ) -> Result<PreparedSmallSignedPlaneV1<R, K, P>, ZkAmsMkheErrorV1> {
        let coordinate = small_signed_plane_coordinate_v1(self.next_comparator_plane)?;
        validate_materialized_context_v1(&self)?;
        let packed = self
            .evidence
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .read_small_signed_plane_v1(
                self.record.replay_record_digest,
                self.record.source_receipt_digest,
                coordinate.ordinal,
            )?;
        let values = expand_small_signed_values_v1(packed, coordinate)?;
        let statement = PreparedSmallSignedStatementV1 {
            values: &values,
            ordinal: coordinate.ordinal,
            replay_record_digest: self.record.replay_record_digest,
            source_receipt_digest: self.record.source_receipt_digest,
        };
        let tail = self
            .evidence
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .commit_prepared_small_signed_v1(&statement)?;
        let opening =
            PreparedPlaneOpeningV1::from_committed_v1(values, tail, self.next_comparator_plane)?;
        Ok(PreparedSmallSignedPlaneV1 {
            live: Some(PreparedSmallSignedPlaneLiveV1 {
                source: self,
                opening,
            }),
        })
    }
}

fn expand_small_signed_values_v1(
    packed: ConfidentialSpoolChunkV1,
    coordinate: SmallSignedPlaneCoordinateV1,
) -> Result<PreparedRadixValuesV1, ZkAmsMkheErrorV1> {
    if coordinate != small_signed_plane_coordinate_v1(coordinate.ordinal)?
        || packed.len_v1() != RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    // Check every source byte, including the last coefficient, before scalar
    // allocation. The authenticated original compactor used the same r/e bounds.
    for byte in packed.as_slice_v1() {
        let magnitude = RadixSecretCopyV2::new(if byte & 0x80 == 0 {
            *byte
        } else {
            (!*byte).wrapping_add(1)
        });
        if *magnitude.as_ref_v2() > coordinate.bound {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
    }
    let mut values =
        ZeroizingT256ScalarVecV1::try_with_exact_capacity(RADIX_COEFFICIENTS_PER_GROUP_V2)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    // Compact slots already use k=1024*local_block+i. No D-plane transpose is
    // applied, and no field reduction substitutes for the signed integer check.
    for byte in packed.as_slice_v1() {
        let negative = RadixSecretCopyV2::new(byte >> 7);
        let magnitude = RadixSecretCopyV2::new(if *negative.as_ref_v2() == 0 {
            *byte
        } else {
            (!*byte).wrapping_add(1)
        });
        let scalar = ZeroizingT256ScalarCopyV1::new(VegaT256ScalarV1::from_u64(u64::from(
            *magnitude.as_ref_v2(),
        )));
        values.push(if coordinate.negative_magnitude {
            if *negative.as_ref_v2() == 0 {
                VegaT256ScalarV1::zero()
            } else {
                scalar.get()
            }
        } else if *negative.as_ref_v2() == 0 {
            scalar.get()
        } else {
            -scalar.get()
        });
    }
    PreparedRadixValuesV1::from_exact_values_v1(values)
}

struct PreparedSmallSignedPlaneLiveV1<R, K, P> {
    source: Phase23RadixWitnessMaterializedV2<R, K, P>,
    opening: PreparedPlaneOpeningV1,
}

/// Prepared values retaining the only original source, session and snapshots.
#[must_use = "dropping signed values closes their original source and secret values"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct PreparedSmallSignedPlaneV1
<R, K, P> {
    live: Option<PreparedSmallSignedPlaneLiveV1<R, K, P>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> PreparedSmallSignedPlaneV1<R, K, P> {
    /// Emit the next exact 512-scalar chunk; errors consume the retained owner.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn emit_next_value_chunk_v1(
        &mut self,
        expected_chunk: u8,
    ) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let chunk = live.opening.emit_next_value_chunk_v1(expected_chunk)?;
        self.live = Some(live);
        Ok(chunk)
    }

    /// Emit the original admitted tail only after all 32 ordered value chunks.
    /// Every error consumes the sole original source, values and retained masks.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn emit_opening_tail_v1(
        &mut self,
    ) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let tail = live.opening.emit_tail_v1()?;
        self.live = Some(live);
        Ok(tail)
    }

    /// Consume the actual prepared opening into its source-owned ordered writer.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn store_v1(
        mut self,
    ) -> Result<Phase23RadixWitnessMaterializedV2<R, K, P>, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let ordinal = live.source.next_comparator_plane;
        let writer = live
            .source
            .ordered_writer
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        live.opening.store_v1(writer, ordinal)?;
        self.live = Some(live);
        self.finish_v1()
    }

    /// Return the original source only after all 33 canonical slots were emitted.
    /// Emission does not assert ordered storage, authenticated reopen or a proof.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn finish_v1(
        mut self,
    ) -> Result<Phase23RadixWitnessMaterializedV2<R, K, P>, ZkAmsMkheErrorV1> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        live.opening.finish_v1()?;
        let mut source = live.source;
        if let Some(writer) = source.ordered_writer.as_ref() {
            writer
                .require_next_slot_v1((u64::from(source.next_comparator_plane) + 1) * 33)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        }
        small_signed_plane_coordinate_v1(source.next_comparator_plane)?;
        source.next_comparator_plane = source
            .next_comparator_plane
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(source)
    }
}

/// Borrowed statement minted only from the original authenticated compact read.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct PreparedSmallSignedStatementV1
<'a> {
    values: &'a PreparedRadixValuesV1,
    ordinal: u16,
    replay_record_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
}

impl PreparedSmallSignedStatementV1<'_> {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn validate_origin_v1(
        &self,
        replay_record_digest: [u8; 32],
        source_receipt_digest: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        small_signed_plane_coordinate_v1(self.ordinal)?;
        if replay_record_digest == [0; 32]
            || source_receipt_digest == [0; 32]
            || self.replay_record_digest != replay_record_digest
            || self.source_receipt_digest != source_receipt_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn require_ordinal_v1(
        &self,
        expected: u16,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        small_signed_plane_coordinate_v1(self.ordinal)?;
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

// TODO: join the consuming 33-slot sequence to the canonical ordered writer.
// Emission alone does not establish stored snapshot or opening-proof authority.
#[cfg(test)]
#[path = "prepared_small_signed_plane_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) use tests::TestPreparedSmallSignedV1;
