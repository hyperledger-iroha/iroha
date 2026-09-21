//! One original-session S block producer, reused through all 6,400 S tickets.
//! The stream retains one original coefficient allocation and one rho inventory.
//! Its named child consumes original S into complement openings; neither phase
//! grants qPCS, native40 source or composite authority.
use super::*;
use crate::vega::{
    bulletproof_t256::ZeroizingT256ScalarVecV1,
    zk_ams::mkhe::{
        global_lookup_statement_v1::{SealedQMaskSFileV1, WrittenQMaskSBlockFileV1},
        rns_native_u15_msm::{
            RnsNativeU15EvaluationAdmissionV1, RnsNativeU15MsmErrorV1, RnsNativeU15MsmTableV1,
        },
    },
};
const S_AFTER_INVENTORY_V1: u32 = QMASK_FIRST_INVENTORY_V1 + MASK_DIGITS_V1 as u32;
const RADIX_MASK_V1: u64 = (1 << 15) - 1;
fn kernel_error_v1(error: RnsNativeU15MsmErrorV1) -> QMaskSErrorV1 {
    match error {
        RnsNativeU15MsmErrorV1::Capacity => QMaskSErrorV1::Capacity,
        RnsNativeU15MsmErrorV1::Source => QMaskSErrorV1::Source,
        RnsNativeU15MsmErrorV1::Allocation | RnsNativeU15MsmErrorV1::Ledger => {
            QMaskSErrorV1::Resource
        }
    }
}
fn terminal_kernel_error_v1(error: RnsNativeU15MsmErrorV1) -> QMaskSErrorV1 {
    match kernel_error_v1(error) {
        QMaskSErrorV1::Capacity => QMaskSErrorV1::Resource,
        other => other,
    }
}
fn entropy_counters_v1<R>(
    session: &GlobalLookupCommitmentSessionLiveV1<R>,
) -> Result<(u64, u64), QMaskSErrorV1> {
    match &session.entropy {
        GlobalLookupProofSessionEntropySourceV1::Production {
            commitment_entropy_bytes,
            q_mask_entropy_bytes,
            ..
        } => Ok((*commitment_entropy_bytes, *q_mask_entropy_bytes)),
        #[cfg(test)]
        GlobalLookupProofSessionEntropySourceV1::TestOnly(_) => Err(QMaskSErrorV1::Entropy),
    }
}
fn require_original_block_v1<R>(
    session: &GlobalLookupCommitmentSessionLiveV1<R>,
    table: &RnsNativeU15MsmTableV1,
    block: &SampledQMaskSBlockV1,
) -> Result<(), QMaskSErrorV1> {
    table
        .require_original_budget_v1(&session.proof_resources)
        .map_err(kernel_error_v1)?;
    if !block
        .memory
        .reservation
        .belongs_to_v1(&session.proof_resources)
        || block.coefficients.values.len() != BLOCK_COEFFICIENTS_V1
        || block.coefficients.values.capacity() != BLOCK_COEFFICIENTS_V1
        || QMaskSBlockCoordinateV1::from_ordinal_v1(block.coordinate.ordinal)? != block.coordinate
        || session.inventory.slots.len() != GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1 as usize
        || session.pending_source.is_some()
        || session.source_opening_context_digest.is_none()
    {
        return Err(QMaskSErrorV1::Source);
    }
    Ok(())
}
fn require_next_coordinates_v1<R>(
    session: &GlobalLookupCommitmentSessionLiveV1<R>,
    coordinate: QMaskSBlockCoordinateV1,
) -> Result<(), QMaskSErrorV1> {
    let first = coordinate.first_ticket_v1();
    if session.next_global_ordinal != first
        || session.next_purpose != GlobalLookupCommitmentPurposeV1::QMaskDigit
        || session.next_purpose_ordinal != coordinate.ordinal as u32 * 4
        || session
            .inventory
            .slots
            .get(first as usize..first as usize + 4)
            .is_none_or(|x| x.iter().any(Option::is_some))
    {
        return Err(QMaskSErrorV1::Source);
    }
    Ok(())
}
fn require_sampled_entropy_v1(
    coordinate: QMaskSBlockCoordinateV1,
    mask: u64,
) -> Result<(), QMaskSErrorV1> {
    let min = coordinate.sampled_values_through_v1() * 8;
    if mask < min || mask > min * MAX_RANDOM_REJECTION_ATTEMPTS_V1 as u64 || mask % 8 != 0 {
        return Err(QMaskSErrorV1::Source);
    }
    Ok(())
}
fn require_file_v1<R>(
    session: &GlobalLookupCommitmentSessionLiveV1<R>,
    block: &SampledQMaskSBlockV1,
    file: &WrittenQMaskSBlockFileV1,
) -> Result<(), QMaskSErrorV1> {
    if file
        .require_block_binding_v1()
        .map_err(QMaskSErrorV1::Storage)?
        != block.binding
        || file.block_ordinal_v1().map_err(QMaskSErrorV1::Storage)? != block.coordinate.ordinal
    {
        return Err(QMaskSErrorV1::Source);
    }
    file.require_original_budget_v1(&session.proof_resources)
        .map_err(|_| QMaskSErrorV1::Source)
}
/// Actual same-ledger allocations admitted before the block's next entropy.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct QMaskSBlockAdmissionV1
{
    evaluations: [Option<RnsNativeU15EvaluationAdmissionV1>; 4],
    reservation: RnsNativeResourceReservationV1,
    rho_owner: Option<RnsNativeResourceReservationV1>,
    coordinate: QMaskSBlockCoordinateV1,
    binding: [u8; 32],
    commitment_entropy_before: u64,
    mask_entropy_before: u64,
}
impl QMaskSBlockAdmissionV1 {
    pub(in super::super) fn new_v1<R>(
        session: &mut GlobalLookupCommitmentSessionLiveV1<R>,
        table: &RnsNativeU15MsmTableV1,
        block: &SampledQMaskSBlockV1,
        file: &WrittenQMaskSBlockFileV1,
    ) -> Result<Self, QMaskSErrorV1> {
        require_original_block_v1(session, table, block)?;
        if block.coordinate.ordinal != 0 {
            return Err(QMaskSErrorV1::Source);
        }
        require_next_coordinates_v1(session, block.coordinate)?;
        require_sampled_entropy_v1(block.coordinate, entropy_counters_v1(session)?.1)?;
        require_file_v1(session, block, file)?;
        Self::reserve_v1(session, table, block.coordinate, block.binding, true)
    }
    pub(in super::super) fn next_v1<R>(
        session: &mut GlobalLookupCommitmentSessionLiveV1<R>,
        table: &RnsNativeU15MsmTableV1,
        stream: &QMaskSOpeningStreamV1,
        file: &WrittenQMaskSBlockFileV1,
    ) -> Result<Self, QMaskSErrorV1> {
        stream.require_progress_v1(session, table)?;
        require_file_v1(session, &stream.block, file)?;
        let coordinate =
            QMaskSBlockCoordinateV1::from_ordinal_v1(stream.block.coordinate.ordinal + 1)?;
        require_next_coordinates_v1(session, coordinate)?;
        Self::reserve_v1(session, table, coordinate, stream.block.binding, false)
    }
    fn reserve_v1<R>(
        session: &mut GlobalLookupCommitmentSessionLiveV1<R>,
        table: &RnsNativeU15MsmTableV1,
        coordinate: QMaskSBlockCoordinateV1,
        binding: [u8; 32],
        first: bool,
    ) -> Result<Self, QMaskSErrorV1> {
        let counters = entropy_counters_v1(session)?;
        let scratch = SOURCE_OPENING_BLINDING_SLOT_BYTES_V1 as usize
            + core::mem::size_of::<ConfidentialSpoolChunkV1>()
            + 2 * core::mem::size_of::<Scalar>()
            + core::mem::size_of::<ZeroizingT256ScalarCopyV1>()
            + core::mem::size_of::<[u8; 32]>()
            + core::mem::size_of::<[u8; 33]>()
            + core::mem::size_of::<GlobalLookupCommitmentTicketV1>()
            + core::mem::size_of::<Zeroizing<u64>>();
        let map = |e| {
            match e {
            crate::vega::zk_ams::mkhe::rns_native_resource_budget::RnsNativeResourceErrorV1::WorkspaceLimit=>QMaskSErrorV1::Capacity,
            _=>QMaskSErrorV1::Resource,
        }
        };
        // One exact complete rho allocation, reserved before the first scalar.
        // Subsequent blocks initialize its remaining slots without reallocation.
        let rho_owner = if first {
            Some(
                session
                    .proof_resources
                    .reserve_workspace_v1(
                        (MASK_DIGITS_V1 * core::mem::size_of::<Scalar>()
                            + core::mem::size_of::<QMaskSOpeningStreamV1>()
                            + core::mem::size_of::<CompleteQMaskSOpeningsV1>())
                            as u64,
                        0,
                    )
                    .map_err(map)?,
            )
        } else {
            None
        };
        let reservation = session
            .proof_resources
            .reserve_workspace_v1(core::mem::size_of::<Self>() as u64, scratch as u64)
            .map_err(map)?;
        let mut evaluations = core::array::from_fn(|_| None);
        for e in &mut evaluations {
            *e = Some(
                table
                    .admit_evaluation_v1(&session.proof_resources)
                    .map_err(kernel_error_v1)?,
            );
        }
        Ok(Self {
            evaluations,
            reservation,
            rho_owner,
            coordinate,
            binding,
            commitment_entropy_before: counters.0,
            mask_entropy_before: counters.1,
        })
    }
    fn require_v1<R>(
        &self,
        session: &GlobalLookupCommitmentSessionLiveV1<R>,
        binding: [u8; 32],
    ) -> Result<(), QMaskSErrorV1> {
        if !self.reservation.belongs_to_v1(&session.proof_resources)
            || self.binding != binding
            || entropy_counters_v1(session)?
                != (self.commitment_entropy_before, self.mask_entropy_before)
            || self.evaluations.iter().any(Option::is_none)
        {
            return Err(QMaskSErrorV1::Source);
        }
        require_next_coordinates_v1(session, self.coordinate)
    }
}
/// Sole initialized original S rho inventory and one reusable secret block.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct QMaskSOpeningStreamV1
{
    block: SampledQMaskSBlockV1,
    blindings: ZeroizingT256ScalarVecV1,
    reservation: RnsNativeResourceReservationV1,
}
impl QMaskSOpeningStreamV1 {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn at_full_s_v1(
        &self,
    ) -> bool {
        self.block.coordinate.ordinal == MASK_BLOCKS_V1 - 1
            && self.blindings.len() == MASK_DIGITS_V1
    }

    pub(in super::super) fn produce_v1<R: MaskedRelaxedRandomSourceV1>(
        session: &mut GlobalLookupCommitmentSessionLiveV1<R>,
        table: &mut RnsNativeU15MsmTableV1,
        block: SampledQMaskSBlockV1,
        mut admission: QMaskSBlockAdmissionV1,
    ) -> Result<Self, QMaskSErrorV1> {
        require_original_block_v1(session, table, &block)?;
        admission.require_v1(session, block.binding)?;
        if block.coordinate.ordinal != 0 || admission.coordinate != block.coordinate {
            return Err(QMaskSErrorV1::Source);
        }
        let reservation = admission.rho_owner.take().ok_or(QMaskSErrorV1::Source)?;
        if !reservation.belongs_to_v1(&session.proof_resources) {
            return Err(QMaskSErrorV1::Source);
        }
        let blindings = ZeroizingT256ScalarVecV1::try_with_exact_capacity(MASK_DIGITS_V1)
            .map_err(|_| QMaskSErrorV1::Resource)?;
        let mut stream = Self {
            block,
            blindings,
            reservation,
        };
        stream.commit_block_v1(session, table, &mut admission)?;
        Ok(stream)
    }
    fn require_progress_v1<R>(
        &self,
        session: &GlobalLookupCommitmentSessionLiveV1<R>,
        table: &RnsNativeU15MsmTableV1,
    ) -> Result<(), QMaskSErrorV1> {
        require_original_block_v1(session, table, &self.block)?;
        let count = (self.block.coordinate.ordinal + 1) * 4;
        if !self.reservation.belongs_to_v1(&session.proof_resources)
            || self.blindings.len() != count
            || session.next_global_ordinal != QMASK_FIRST_INVENTORY_V1 + count as u32
        {
            return Err(QMaskSErrorV1::Source);
        }
        let next = commitment_coordinate_v1(session.next_global_ordinal)
            .map_err(|_| QMaskSErrorV1::Source)?;
        if session.next_purpose != next.purpose
            || session.next_purpose_ordinal != next.purpose_ordinal
        {
            return Err(QMaskSErrorV1::Source);
        }
        require_sampled_entropy_v1(self.block.coordinate, entropy_counters_v1(session)?.1)
    }
    pub(in super::super) fn continue_v1<R: MaskedRelaxedRandomSourceV1>(
        mut self,
        session: &mut GlobalLookupCommitmentSessionLiveV1<R>,
        table: &mut RnsNativeU15MsmTableV1,
        file: &mut QMaskSFileV1,
        mut admission: QMaskSBlockAdmissionV1,
    ) -> Result<Self, QMaskSErrorV1> {
        self.require_progress_v1(session, table)?;
        admission.require_v1(session, self.block.binding)?;
        if admission.rho_owner.is_some()
            || admission.coordinate.ordinal != self.block.coordinate.ordinal + 1
            || file.binding_v1() != self.block.binding
        {
            return Err(QMaskSErrorV1::Source);
        }
        file.require_block_write_v1(
            admission.coordinate.ordinal,
            self.block.binding,
            &session.proof_resources,
        )
        .map_err(QMaskSErrorV1::Storage)?;
        let (random, attempted_bytes) = match &mut session.entropy {
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
        let mut random = MaskEntropyBorrowV1 {
            random,
            attempted_bytes,
        };
        sample_block_into_v1(
            &mut random,
            admission.coordinate.limb,
            admission.coordinate.block,
            &mut self.block.coefficients,
        )?;
        self.block.coordinate = admission.coordinate;
        self.block.write_slots_v1(file)?;
        self.commit_block_v1(session, table, &mut admission)?;
        Ok(self)
    }
    fn commit_block_v1<R: MaskedRelaxedRandomSourceV1>(
        &mut self,
        session: &mut GlobalLookupCommitmentSessionLiveV1<R>,
        table: &mut RnsNativeU15MsmTableV1,
        admission: &mut QMaskSBlockAdmissionV1,
    ) -> Result<(), QMaskSErrorV1> {
        require_original_block_v1(session, table, &self.block)?;
        require_next_coordinates_v1(session, self.block.coordinate)?;
        if self.block.coordinate != admission.coordinate
            || self.blindings.len() != self.block.coordinate.ordinal * 4
        {
            return Err(QMaskSErrorV1::Source);
        }
        let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[self.block.coordinate.limb];
        if self.block.coefficients.values.iter().any(|x| *x >= modulus)
            || (self.block.coordinate.block == 7
                && self.block.coefficients.values[BLOCK_COEFFICIENTS_V1 - 1] != 0)
        {
            return Err(QMaskSErrorV1::Source);
        }
        require_sampled_entropy_v1(self.block.coordinate, entropy_counters_v1(session)?.1)?;
        for digit in 0..4 {
            let coordinate = digit_coordinate_v1(self.block.coordinate, digit)?;
            if session.next_global_ordinal != coordinate.global_ordinal
                || session.next_purpose != coordinate.purpose
                || session.next_purpose_ordinal != coordinate.purpose_ordinal
                || session.inventory.slots[coordinate.global_ordinal as usize].is_some()
                || self.blindings.len() != self.block.coordinate.ordinal * 4 + digit
            {
                return Err(QMaskSErrorV1::Source);
            }
            let (encoded, rho) =
                sample_blinding_v1(&mut session.entropy, coordinate.global_ordinal).map_err(
                    |e| match e {
                        ZkAmsMkheErrorV1::RandomUnavailable => QMaskSErrorV1::Entropy,
                        _ => QMaskSErrorV1::Resource,
                    },
                )?;
            drop(encoded);
            let evaluation = admission.evaluations[digit]
                .take()
                .ok_or(QMaskSErrorV1::Source)?;
            let point = table
                .commitment_with_admission_v1(
                    &session.proof_resources,
                    evaluation,
                    BLOCK_COEFFICIENTS_V1,
                    |index| {
                        let value = Zeroizing::new(self.block.coefficients.values[index]);
                        Ok(((*value >> (15 * digit)) & RADIX_MASK_V1) as u16)
                    },
                    rho.as_ref(),
                )
                .map_err(terminal_kernel_error_v1)?;
            point
                .require_original_budget_v1(&session.proof_resources)
                .map_err(terminal_kernel_error_v1)?;
            let point_wire = point
                .expose_ref_v1()
                .to_non_identity_wire_bytes()
                .map_err(|_| QMaskSErrorV1::Source)?;
            session.inventory.slots[coordinate.global_ordinal as usize] =
                Some(GlobalLookupCommitmentTicketV1 {
                    coordinate,
                    point_wire,
                });
            self.blindings.push(rho.get());
            let next = commitment_coordinate_v1(coordinate.global_ordinal + 1)
                .map_err(|_| QMaskSErrorV1::Source)?;
            session.next_global_ordinal = next.global_ordinal;
            session.next_purpose = next.purpose;
            session.next_purpose_ordinal = next.purpose_ordinal;
        }
        self.require_progress_v1(session, table)
    }
    pub(in super::super) fn finish_v1<R>(
        mut self,
        session: &GlobalLookupCommitmentSessionLiveV1<R>,
        table: &RnsNativeU15MsmTableV1,
    ) -> Result<CompleteQMaskSOpeningsV1, QMaskSErrorV1> {
        self.require_progress_v1(session, table)?;
        if self.block.coordinate.ordinal != MASK_BLOCKS_V1 - 1
            || self.blindings.len() != MASK_DIGITS_V1
            || session.next_purpose != GlobalLookupCommitmentPurposeV1::QMaskComplementDigit
            || session.next_purpose_ordinal != 0
        {
            return Err(QMaskSErrorV1::Source);
        }
        for (i, rho) in self.blindings.as_slice().iter().enumerate() {
            let ticket = session.inventory.slots[QMASK_FIRST_INVENTORY_V1 as usize + i]
                .as_ref()
                .ok_or(QMaskSErrorV1::Source)?;
            if ticket.coordinate
                != digit_coordinate_v1(QMaskSBlockCoordinateV1::from_ordinal_v1(i / 4)?, i % 4)?
                || rho.is_zero()
            {
                return Err(QMaskSErrorV1::Source);
            }
        }
        self.block.coefficients.values.as_mut_slice().zeroize();
        Ok(CompleteQMaskSOpeningsV1 {
            stream: self,
            next_block: 0,
            loaded: false,
        })
    }
}
/// Original full rho allocation and erased reusable S buffer, bound to the same
/// original inventory. It is not a provider or a completed same-opening proof.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct CompleteQMaskSOpeningsV1
{
    stream: QMaskSOpeningStreamV1,
    next_block: usize,
    loaded: bool,
}
impl CompleteQMaskSOpeningsV1 {
    fn load_next_original_block_v1<R>(
        &mut self,
        session: &GlobalLookupCommitmentSessionLiveV1<R>,
        file: &mut SealedQMaskSFileV1,
    ) -> Result<(), QMaskSErrorV1> {
        if self.loaded
            || self.next_block >= MASK_BLOCKS_V1
            || self.stream.blindings.len() != MASK_DIGITS_V1
            || !self
                .stream
                .reservation
                .belongs_to_v1(&session.proof_resources)
        {
            return Err(QMaskSErrorV1::Source);
        }
        file.require_original_v1(self.stream.block.binding, &session.proof_resources)
            .map_err(QMaskSErrorV1::Storage)?;
        let coordinate = QMaskSBlockCoordinateV1::from_ordinal_v1(self.next_block)?;
        // This preflight charges all eight reads and can refuse before any
        // allocation or mutation of the original erased block buffer.
        let mut read = file
            .begin_block_read_v1(self.next_block)
            .map_err(|e| match e {
                OrderedSnapshotErrorV1::Capacity => QMaskSErrorV1::Capacity,
                other => QMaskSErrorV1::Storage(other),
            })?;
        for slot in 0..8 {
            let chunk = read.read_next_slot_v1().map_err(QMaskSErrorV1::Storage)?;
            if chunk.len_v1() != 16_384 {
                return Err(QMaskSErrorV1::Source);
            }
            for (i, bytes) in chunk.as_slice_v1().chunks_exact(8).enumerate() {
                let value = Zeroizing::new(u64::from_le_bytes(
                    bytes.try_into().map_err(|_| QMaskSErrorV1::Source)?,
                ));
                let index = slot * COEFFICIENTS_PER_SLOT_V1 + i;
                if *value >= ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[coordinate.limb]
                    || (coordinate.block == 7 && index == BLOCK_COEFFICIENTS_V1 - 1 && *value != 0)
                {
                    return Err(QMaskSErrorV1::Source);
                }
                self.stream.block.coefficients.values[index] = *value;
            }
        }
        for digit in 0..4 {
            let expected = digit_coordinate_v1(coordinate, digit)?;
            let ticket = session
                .inventory
                .slots
                .get(expected.global_ordinal as usize)
                .and_then(Option::as_ref)
                .ok_or(QMaskSErrorV1::Source)?;
            if ticket.coordinate != expected
                || self.stream.blindings.as_slice()[coordinate.ordinal * 4 + digit].is_zero()
            {
                return Err(QMaskSErrorV1::Source);
            }
        }
        read.finish_v1().map_err(QMaskSErrorV1::Storage)?;
        self.stream.block.coordinate = coordinate;
        self.loaded = true;
        Ok(())
    }
    // Future named complement/P~/H~/qPCS consumers must use this exact private
    // block and original rhos. No raw scalar slice or caller callback exists.
    fn erase_loaded_block_v1(&mut self) -> Result<(), QMaskSErrorV1> {
        if !self.loaded {
            return Err(QMaskSErrorV1::Source);
        }
        self.stream
            .block
            .coefficients
            .values
            .as_mut_slice()
            .zeroize();
        self.loaded = false;
        self.next_block += 1;
        Ok(())
    }
    fn require_replayed_v1(&self, file: &SealedQMaskSFileV1) -> Result<(), QMaskSErrorV1> {
        if self.loaded || self.next_block != MASK_BLOCKS_V1 {
            return Err(QMaskSErrorV1::Source);
        }
        file.require_replayed_v1().map_err(QMaskSErrorV1::Storage)
    }
}
fn digit_coordinate_v1(
    block: QMaskSBlockCoordinateV1,
    digit: usize,
) -> Result<GlobalLookupCommitmentCoordinateV1, QMaskSErrorV1> {
    if digit >= 4 {
        return Err(QMaskSErrorV1::Source);
    }
    let coordinate = commitment_coordinate_v1(block.first_ticket_v1() + digit as u32)
        .map_err(|_| QMaskSErrorV1::Source)?;
    if coordinate.phase != GlobalLookupCommitmentPhaseV1::ChallengeIndependent
        || coordinate.purpose != GlobalLookupCommitmentPurposeV1::QMaskDigit
        || coordinate.purpose_ordinal != block.ordinal as u32 * 4 + digit as u32
    {
        return Err(QMaskSErrorV1::Source);
    }
    Ok(coordinate)
}
#[cfg(test)]
fn first_digit_coordinate_v1(
    digit: usize,
) -> Result<GlobalLookupCommitmentCoordinateV1, QMaskSErrorV1> {
    digit_coordinate_v1(QMaskSBlockCoordinateV1::from_ordinal_v1(0)?, digit)
}
#[cfg(test)]
#[path = "first_openings_v1_tests.rs"]
pub(super) mod tests;

#[path = "first_openings_v1/complement_v1.rs"]
mod complement_v1;
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) use complement_v1::QMaskComplementOpeningsV1;

#[cfg(all(test, unix))]
pub(in super::super) use complement_v1::with_complement_for_retained_refusal_v1;
