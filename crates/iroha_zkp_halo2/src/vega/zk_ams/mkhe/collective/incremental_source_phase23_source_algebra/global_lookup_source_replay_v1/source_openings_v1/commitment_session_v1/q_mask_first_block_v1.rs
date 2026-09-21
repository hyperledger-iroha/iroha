//! First canonical private-uniform S block from the sole original entropy owner.
//!
//! No caller chooses S, its RNG, modulus, coordinate, byte budget or file. This
//! phase ends before the four same-block opening rhos; completing those actual
//! tickets in the consuming child is required before any next block, complement
//! or complete-file seal.
use super::*;
use crate::vega::zk_ams::mkhe::{
    global_lookup_statement_v1::{
        OrderedSnapshotErrorV1, QMaskSFileMemoryV1, QMaskSFilePlanV1, QMaskSFileV1,
    },
    rns_native_profile::ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1,
    rns_native_resource_budget::RnsNativeResourceReservationV1,
    sample_below,
};
use crate::vega::{MaskedRelaxedRandomErrorV1, MaskedRelaxedRandomSourceV1};
use zeroize::{Zeroize as _, Zeroizing};

const BLOCK_COEFFICIENTS_V1: usize = 16_384;
const RING_COEFFICIENTS_V1: usize = 131_072;
const COEFFICIENTS_PER_SLOT_V1: usize = 2_048;
const QMASK_FIRST_INVENTORY_V1: u32 = 27_176;
const MASK_BLOCKS_V1: usize = 40 * 5 * 8;
const MASK_DIGITS_V1: usize = MASK_BLOCKS_V1 * 4;

/// One private coordinate owns the only producer traversal; no caller chooses
/// limb, repetition, local block or a different digit/ticket layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QMaskSBlockCoordinateV1 {
    ordinal: usize,
    limb: usize,
    repetition: usize,
    block: usize,
}
impl QMaskSBlockCoordinateV1 {
    fn from_ordinal_v1(ordinal: usize) -> Result<Self, QMaskSErrorV1> {
        if ordinal >= MASK_BLOCKS_V1 {
            return Err(QMaskSErrorV1::Source);
        }
        Ok(Self {
            ordinal,
            limb: ordinal / 40,
            repetition: ordinal / 8 % 5,
            block: ordinal % 8,
        })
    }
    fn first_ticket_v1(self) -> u32 {
        QMASK_FIRST_INVENTORY_V1 + self.ordinal as u32 * 4
    }
    fn first_slot_v1(self) -> u64 {
        self.ordinal as u64 * 8
    }
    fn sampled_values_through_v1(self) -> u64 {
        (self.ordinal as u64 + 1) * BLOCK_COEFFICIENTS_V1 as u64 - (self.ordinal as u64 + 1) / 8
    }
}
const MASK_ENTROPY_MAX_BYTES_V1: u64 =
    40 * 5 * (RING_COEFFICIENTS_V1 as u64 - 1) * MAX_RANDOM_REJECTION_ATTEMPTS_V1 as u64 * 8;
const _: () = assert!(MASK_ENTROPY_MAX_BYTES_V1 == 26_843_340_800);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) enum QMaskSErrorV1
{
    Capacity,
    Source,
    Resource,
    Entropy,
    Storage(OrderedSnapshotErrorV1),
}

/// Owns the actual original-ledger memory reservation until S/file destruction.
/// The private constructor takes the original session, never a fresh budget.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct QMaskFirstBlockMemoryV1
{
    binding: [u8; 32],
    reservation: RnsNativeResourceReservationV1,
}
impl QMaskFirstBlockMemoryV1 {
    pub(super) fn new_v1<R>(
        session: &mut GlobalLookupCommitmentSessionLiveV1<R>,
        plan: &QMaskSFilePlanV1,
    ) -> Result<(Self, QMaskSFileMemoryV1), QMaskSErrorV1> {
        require_first_coordinate_v1(session)?;
        let file_memory = plan
            .reserve_memory_v1(&mut session.proof_resources)
            .map_err(|error| match error {
                OrderedSnapshotErrorV1::Capacity => QMaskSErrorV1::Capacity,
                _ => QMaskSErrorV1::Resource,
            })?;
        let retained = (BLOCK_COEFFICIENTS_V1 * core::mem::size_of::<u64>())
            .checked_add(core::mem::size_of::<SampledQMaskSBlockV1>())
            .ok_or(QMaskSErrorV1::Resource)?;
        let scratch = 2 * core::mem::size_of::<Zeroizing<u64>>();
        let reservation = file_memory
            .reserve_block_workspace_v1(retained as u64, scratch as u64)
            .map_err(|error| match error {
                OrderedSnapshotErrorV1::Capacity => QMaskSErrorV1::Capacity,
                _ => QMaskSErrorV1::Resource,
            })?;
        Ok((
            Self {
                binding: plan.binding_v1(),
                reservation,
            },
            file_memory,
        ))
    }
}

fn require_first_coordinate_v1<R>(
    session: &GlobalLookupCommitmentSessionLiveV1<R>,
) -> Result<(), QMaskSErrorV1> {
    let coordinate =
        commitment_coordinate_v1(QMASK_FIRST_INVENTORY_V1).map_err(|_| QMaskSErrorV1::Source)?;
    if session.next_global_ordinal != coordinate.global_ordinal
        || session.next_purpose != GlobalLookupCommitmentPurposeV1::QMaskDigit
        || session.next_purpose_ordinal != 0
        || session.pending_source.is_some()
        || session.source_opening_context_digest.is_none()
        || session
            .inventory
            .slots
            .get(QMASK_FIRST_INVENTORY_V1 as usize)
            .is_none_or(Option::is_some)
    {
        return Err(QMaskSErrorV1::Source);
    }
    Ok(())
}

/// Private borrowed adapter charges every real eight-byte request before fill.
/// It cannot reseed, rewind, replace or extract the original random source.
struct MaskEntropyBorrowV1<'a, R> {
    random: &'a mut R,
    attempted_bytes: &'a mut u64,
}
impl<R: MaskedRelaxedRandomSourceV1> MaskedRelaxedRandomSourceV1 for MaskEntropyBorrowV1<'_, R> {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        let next = self
            .attempted_bytes
            .checked_add(8)
            .filter(|n| *n <= MASK_ENTROPY_MAX_BYTES_V1);
        if destination.len() != 8 {
            destination.zeroize();
            return Err(MaskedRelaxedRandomErrorV1::Unavailable);
        }
        let Some(next) = next else {
            destination.zeroize();
            return Err(MaskedRelaxedRandomErrorV1::Unavailable);
        };
        *self.attempted_bytes = next;
        self.random.fill_bytes(destination).map_err(|error| {
            destination.zeroize();
            error
        })
    }
}

/// Closed original coefficient owner; no raw vector, mutable slice or caller-S
/// constructor is exposed. The original file preserves every sampled preimage.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct SampledQMaskSBlockV1
{
    coordinate: QMaskSBlockCoordinateV1,
    coefficients: QMaskCoefficientsV1,
    binding: [u8; 32],
    memory: QMaskFirstBlockMemoryV1,
}
struct QMaskCoefficientsV1 {
    values: Vec<u64>,
}
impl Drop for QMaskCoefficientsV1 {
    fn drop(&mut self) {
        self.values.as_mut_slice().zeroize();
        #[cfg(test)]
        {
            assert!(self.values.iter().all(|value| *value == 0));
            ZEROIZED_MASK_VALUES_V1.with(|count| count.set(count.get() + self.values.len()));
        }
    }
}
impl SampledQMaskSBlockV1 {
    pub(super) fn sample_v1<R: MaskedRelaxedRandomSourceV1>(
        session: &mut GlobalLookupCommitmentSessionLiveV1<R>,
        memory: QMaskFirstBlockMemoryV1,
    ) -> Result<Self, QMaskSErrorV1> {
        require_first_coordinate_v1(session)?;
        if !memory.reservation.belongs_to_v1(&session.proof_resources) {
            return Err(QMaskSErrorV1::Source);
        }
        let (original_random, q_mask_entropy_bytes) = match &mut session.entropy {
            GlobalLookupProofSessionEntropySourceV1::Production {
                original_random,
                q_mask_entropy_bytes,
                ..
            } => (original_random, q_mask_entropy_bytes),
            #[cfg(test)]
            GlobalLookupProofSessionEntropySourceV1::TestOnly(_) => {
                return Err(QMaskSErrorV1::Entropy);
            }
        };
        if *q_mask_entropy_bytes != 0 {
            return Err(QMaskSErrorV1::Source);
        }
        let mut random = MaskEntropyBorrowV1 {
            random: original_random,
            attempted_bytes: q_mask_entropy_bytes,
        };
        let coefficients = sample_block_v1(&mut random, 0, 0)?;
        Ok(Self {
            coordinate: QMaskSBlockCoordinateV1::from_ordinal_v1(0)?,
            coefficients,
            binding: memory.binding,
            memory,
        })
    }
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn write_slots_v1(
        &self,
        file: &mut QMaskSFileV1,
    ) -> Result<(), QMaskSErrorV1> {
        if file.binding_v1() != self.binding
            || self.coefficients.values.len() != BLOCK_COEFFICIENTS_V1
        {
            return Err(QMaskSErrorV1::Source);
        }
        for slot in 0..8 {
            let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384)
                .map_err(|_| QMaskSErrorV1::Resource)?;
            let first = slot * COEFFICIENTS_PER_SLOT_V1;
            for (index, coefficient) in self.coefficients.values
                [first..first + COEFFICIENTS_PER_SLOT_V1]
                .iter()
                .enumerate()
            {
                let encoded = Zeroizing::new(coefficient.to_le_bytes());
                chunk.as_mut_slice_v1()[index * 8..index * 8 + 8].copy_from_slice(&encoded[..]);
            }
            file.write_slot_v1(self.coordinate.first_slot_v1() + slot as u64, chunk)
                .map_err(QMaskSErrorV1::Storage)?;
        }
        Ok(())
    }
}

fn sample_block_v1<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
    limb: usize,
    block: usize,
) -> Result<QMaskCoefficientsV1, QMaskSErrorV1> {
    if limb >= ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.len() || block >= 8 {
        return Err(QMaskSErrorV1::Source);
    }
    let mut coefficients = QMaskCoefficientsV1 { values: Vec::new() };
    coefficients
        .values
        .try_reserve_exact(BLOCK_COEFFICIENTS_V1)
        .map_err(|_| QMaskSErrorV1::Resource)?;
    if coefficients.values.capacity() != BLOCK_COEFFICIENTS_V1 {
        return Err(QMaskSErrorV1::Resource);
    }
    sample_block_into_v1(random, limb, block, &mut coefficients)?;
    Ok(coefficients)
}

// The first allocation and every later in-place refill use this one sampler.
fn sample_block_into_v1<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
    limb: usize,
    block: usize,
    coefficients: &mut QMaskCoefficientsV1,
) -> Result<(), QMaskSErrorV1> {
    let modulus = *ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
        .get(limb)
        .ok_or(QMaskSErrorV1::Source)?;
    if block >= 8 || coefficients.values.capacity() != BLOCK_COEFFICIENTS_V1 {
        return Err(QMaskSErrorV1::Source);
    }
    coefficients.values.as_mut_slice().zeroize();
    coefficients.values.clear();
    for index in 0..BLOCK_COEFFICIENTS_V1 {
        if block * BLOCK_COEFFICIENTS_V1 + index == RING_COEFFICIENTS_V1 - 1 {
            coefficients.values.push(0);
        } else {
            let value =
                Zeroizing::new(sample_below(modulus, random).map_err(|_| QMaskSErrorV1::Entropy)?);
            coefficients.values.push(*value);
        }
    }
    Ok(())
}

#[cfg(test)]
std::thread_local! { static ZEROIZED_MASK_VALUES_V1:core::cell::Cell<usize>=const {core::cell::Cell::new(0)}; }
#[cfg(test)]
#[path = "q_mask_first_block_v1_tests.rs"]
mod tests;

#[path = "q_mask_first_block_v1/first_openings_v1.rs"]
mod first_openings_v1;
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) use first_openings_v1::{
    QMaskComplementOpeningsV1,CompleteQMaskSOpeningsV1, QMaskSBlockAdmissionV1, QMaskSOpeningStreamV1};

#[cfg(all(test, unix))]
pub(super) use first_openings_v1::tests::with_first_openings_for_retained_refusal_v1;

#[cfg(all(test, unix))]
pub(super) use first_openings_v1::tests::with_s_stream_for_retained_refusal_v1;

#[cfg(all(test, unix))]
pub(super) use first_openings_v1::with_complement_for_retained_refusal_v1;
