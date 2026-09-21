//! Consuming expansion of authenticated compact comparator values.
//!
//! The source retains its replay evidence, seal and snapshot through 32 value
//! chunks and the original admitted opening tail. Its original session computes
//! and retains each matching point and blinding before emission. An attached
//! ordered writer advances only after every successful write; no proof or
//! native40 source qualification is claimed. See `ordered_storage_handoff_v1.md`.
use super::*;
use crate::vega::{VegaT256ScalarV1, bulletproof_t256::ZeroizingT256ScalarVecV1};
use std::sync::OnceLock;

const COMPARATOR_PLANE_COUNT_V1: u16 = 7_224;
const VALUE_CHUNKS_V1: u8 = 32;
const SCALARS_PER_CHUNK_V1: usize = 512;
const SCALAR_BYTES_V1: usize = 32;
const VALUE_CHUNK_BYTES_V1: u64 = (SCALARS_PER_CHUNK_V1 * SCALAR_BYTES_V1) as u64;
const _: () = {
    assert!(RADIX_GROUP_COUNT_V2 == 344);
    assert!(COMPARATOR_PLANE_COUNT_V1 as usize == RADIX_GROUP_COUNT_V2 * 21);
    assert!(VALUE_CHUNKS_V1 as usize * SCALARS_PER_CHUNK_V1 == RADIX_COEFFICIENTS_PER_GROUP_V2);
    assert!(VALUE_CHUNK_BYTES_V1 == 16_384);
    assert!(core::mem::size_of::<VegaT256ScalarV1>() == SCALAR_BYTES_V1);
};

/// Public coordinates of one exact comparator plane; never witness data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ComparatorCoordinateV1 {
    ordinal: u16,
    slot: u64,
    lane: u8,
    bit: u8,
}

fn comparator_coordinate_v1(ordinal: u16) -> Result<ComparatorCoordinateV1, ZkAmsMkheErrorV1> {
    let ordinal_usize = usize::from(ordinal);
    let groups = RADIX_GROUP_COUNT_V2;
    let (group, lane, bit) = if ordinal_usize < groups {
        (ordinal_usize, 0, 0)
    } else if ordinal_usize < 2 * groups {
        (ordinal_usize - groups, 0, 1)
    } else if ordinal_usize < 20 * groups {
        let beta_ordinal = ordinal_usize - 2 * groups;
        let group = beta_ordinal / RADIX_COMPARATOR_BITS_V2;
        let beta = beta_ordinal % RADIX_COMPARATOR_BITS_V2;
        if beta < 6 {
            (group, 0, beta + 2)
        } else if beta < 14 {
            (group, 1, beta - 6)
        } else {
            (group, 2, beta - 14)
        }
    } else if ordinal < COMPARATOR_PLANE_COUNT_V1 {
        (ordinal_usize - 20 * groups, 2, 4)
    } else {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    };
    let slot = radix_witness_slot_v2(
        group / RADIX_GROUPS_PER_RECORD_V2,
        group % RADIX_GROUPS_PER_RECORD_V2,
        lane,
    )?;
    Ok(ComparatorCoordinateV1 {
        ordinal,
        slot,
        lane: u8::try_from(lane).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        bit: u8::try_from(bit).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    })
}

// Only the existing immutable public mapping computation is memoized. No
// caller-provided digest, witness, source identity or validation result is cached.
fn canonical_radix_mapping_v1() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    static MAPPING: OnceLock<Result<[u8; 32], ZkAmsMkheErrorV1>> = OnceLock::new();
    *MAPPING.get_or_init(exact_radix_witness_mapping_digest_v2)
}

fn validate_materialization_record_context_v1(
    record: &RadixWitnessMaterializationRecordV2,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_radix_witness_record_v2(record)?;
    let mapping = canonical_radix_mapping_v1()?;
    if record.mapping_digest != mapping
        || record.spool_context_digest
            != radix_witness_context_digest_v2(
                record.replay_record_digest,
                record.source_receipt_digest,
                mapping,
            )?
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

pub(super) fn validate_materialized_context_v1<
    R: crate::vega::MaskedRelaxedRandomSourceV1,
    K,
    P,
>(
    source: &Phase23RadixWitnessMaterializedV2<R, K, P>,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_materialization_record_context_v1(&source.record)?;
    if source.snapshot.slot_count_v1() != RADIX_WITNESS_SLOT_COUNT_V2 as u64
        || source.snapshot.plaintext_len_v1() != RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2
        || source.snapshot.file_len_v1() != RADIX_WITNESS_FILE_BYTES_V2
        || *source.snapshot.snapshot_digest_v1() != source.record.snapshot_root
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    source
        .materialization_seal
        .validate_for_materialized_record_v2(&source.record)
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> Phase23RadixWitnessMaterializedV2<R, K, P> {
    /// Consume the source and prepare its next comparator plane in final order.
    /// Failure closes the source; no caller-selected ordinal or retry is possible.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn prepare_next_comparator_plane_v1(
        mut self,
    ) -> Result<PreparedComparatorPlaneV1<R, K, P>, ZkAmsMkheErrorV1> {
        let coordinate = comparator_coordinate_v1(self.next_comparator_plane)?;
        self.evidence
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .validate_comparator_preparation_v1(
                self.record.replay_record_digest,
                self.record.source_receipt_digest,
                self.next_comparator_plane,
            )?;
        validate_materialized_context_v1(&self)?;
        // Every source/context/shape check precedes secret allocation and I/O.
        let values = read_comparator_values_v1(
            &mut self.snapshot,
            coordinate,
            self.record.spool_context_digest,
        )?;
        let statement = PreparedComparatorStatementV1 {
            values: &values,
            ordinal: self.next_comparator_plane,
            replay_record_digest: self.record.replay_record_digest,
            source_receipt_digest: self.record.source_receipt_digest,
        };
        let tail = self
            .evidence
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .commit_prepared_comparator_v1(&statement)?;
        let opening =
            PreparedPlaneOpeningV1::from_committed_v1(values, tail, self.next_comparator_plane)?;
        Ok(PreparedComparatorPlaneV1 {
            live: Some(PreparedComparatorPlaneLiveV1 {
                source: self,
                opening,
            }),
        })
    }
}

struct PreparedComparatorPlaneLiveV1<R, K, P> {
    source: Phase23RadixWitnessMaterializedV2<R, K, P>,
    opening: PreparedPlaneOpeningV1,
}

/// Move-only prepared values retaining the entire authenticated source owner.
#[must_use = "dropping prepared values closes the retained source and secret values"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct PreparedComparatorPlaneV1
<R, K, P> {
    live: Option<PreparedComparatorPlaneLiveV1<R, K, P>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> PreparedComparatorPlaneV1<R, K, P> {
    /// Emit exactly one canonical 512-scalar value chunk. Any error poisons all
    /// retained state, including an incorrect ordinal before I/O or allocation.
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
        // Recheck the exact private ordinal before advancing; never wrap 7,224.
        comparator_coordinate_v1(source.next_comparator_plane)?;
        source.next_comparator_plane = source
            .next_comparator_plane
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(source)
    }
}

/// Borrowed values minted only after the actual private comparator read.
/// Context, ordinal and secret vector cannot be supplied independently.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct PreparedComparatorStatementV1
<'a> {
    values: &'a PreparedRadixValuesV1,
    ordinal: u16,
    replay_record_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
}
impl PreparedComparatorStatementV1<'_> {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn validate_origin_v1(
        &self,
        replay_record_digest: [u8; 32],
        source_receipt_digest: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        comparator_coordinate_v1(self.ordinal)?;
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
        comparator_coordinate_v1(self.ordinal)?;
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

struct PreparedRadixValuesStateV1 {
    values: ZeroizingT256ScalarVecV1,
    next_chunk: u8,
}

// One exact chunk emitter is shared by the consuming comparator and D/S-low
// source owners. It never exposes the vector. Both owners compute the matching
// commitment before emission using the original session's retained blinding.
pub(super) struct PreparedRadixValuesV1 {
    live: Option<PreparedRadixValuesStateV1>,
}

fn read_comparator_values_v1(
    snapshot: &mut ConfidentialSpoolSnapshotV1,
    coordinate: ComparatorCoordinateV1,
    context: [u8; 32],
) -> Result<PreparedRadixValuesV1, ZkAmsMkheErrorV1> {
    if coordinate != comparator_coordinate_v1(coordinate.ordinal)? {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let packed = snapshot
        .read_slot_v1(coordinate.slot, context)
        .map_err(map_spool_error_v2)?;
    expand_comparator_values_v1(packed, coordinate)
}

fn expand_comparator_values_v1(
    packed: ConfidentialSpoolChunkV1,
    coordinate: ComparatorCoordinateV1,
) -> Result<PreparedRadixValuesV1, ZkAmsMkheErrorV1> {
    if coordinate != comparator_coordinate_v1(coordinate.ordinal)?
        || packed.len_v1() != RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    // The original materializer's lane2 high bits are reserved zeros. Inspect
    // every polynomial-coefficient coordinate before any emission. Logical-slot
    // padding is checked separately by the authenticated source packing owner.
    if coordinate.lane == 2 && packed.as_slice_v1().iter().any(|byte| byte & 0xe0 != 0) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    // The final exact capacity is fallibly reserved while plaintext remains in
    // its crypto-owned zeroizing chunk. Exactly N pushes cannot grow this buffer.
    let mut values =
        ZeroizingT256ScalarVecV1::try_with_exact_capacity(RADIX_COEFFICIENTS_PER_GROUP_V2)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for byte in packed.as_slice_v1() {
        let bit = RadixSecretCopyV2::new((byte >> coordinate.bit) & 1);
        values.push(VegaT256ScalarV1::from_u64(u64::from(*bit.as_ref_v2())));
    }
    if values.len() != RADIX_COEFFICIENTS_PER_GROUP_V2 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    PreparedRadixValuesV1::from_exact_values_v1(values)
}

impl PreparedRadixValuesV1 {
    pub(super) fn commitment_v1(
        &self,
        blinding: &VegaT256ScalarV1,
    ) -> Result<
        crate::generalized_bulletproof::SecretPoint<crate::vega::VegaT256PointV1>,
        ZkAmsMkheErrorV1,
    > {
        let live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.next_chunk != 0 || live.values.len() != RADIX_COEFFICIENTS_PER_GROUP_V2 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        super::super::source_algebra::source_opening_commitment_for_suite_v1::<
            crate::vega::bulletproof_t256::ZkAmsT256BulletproofSuiteV1,
        >(
            live.values.as_slice(),
            blinding,
            RADIX_COEFFICIENTS_PER_GROUP_V2,
        )
    }

    pub(super) fn from_exact_values_v1(
        values: ZeroizingT256ScalarVecV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if values.len() != RADIX_COEFFICIENTS_PER_GROUP_V2 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(Self {
            live: Some(PreparedRadixValuesStateV1 {
                values,
                next_chunk: 0,
            }),
        })
    }

    pub(super) fn emit_next_v1(
        &mut self,
        expected_chunk: u8,
    ) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if expected_chunk != live.next_chunk
            || expected_chunk >= VALUE_CHUNKS_V1
            || live.values.len() != RADIX_COEFFICIENTS_PER_GROUP_V2
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let start = usize::from(expected_chunk)
            .checked_mul(SCALARS_PER_CHUNK_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(SCALARS_PER_CHUNK_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let scalars = live
            .values
            .as_slice()
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(VALUE_CHUNK_BYTES_V1)
            .map_err(map_spool_error_v2)?;
        for (scalar, slot) in scalars
            .iter()
            .zip(chunk.as_mut_slice_v1().chunks_exact_mut(SCALAR_BYTES_V1))
        {
            let destination: &mut [u8; SCALAR_BYTES_V1] = slot
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            scalar.write_le_bytes_ref(destination);
            destination.reverse();
        }
        live.next_chunk = live
            .next_chunk
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.live = Some(live);
        Ok(chunk)
    }

    pub(super) fn finish_v1(mut self) -> Result<(), ZkAmsMkheErrorV1> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.next_chunk != VALUE_CHUNKS_V1
            || live.values.len() != RADIX_COEFFICIENTS_PER_GROUP_V2
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "prepared_comparator_plane_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "prepared_comparator_statement_v1_tests.rs"]
mod statement_tests;
#[cfg(test)]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
use statement_tests::TestPreparedComparatorV1;

#[path = "prepared_plane_opening_v1.rs"]
mod prepared_plane_opening_v1;
pub(super) use prepared_plane_opening_v1::PreparedPlaneOpeningV1;
