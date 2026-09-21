//! Canonical complement digits from the same sealed original S preimage.
//!
//! This producer fills only the existing challenge-independent complement range.
//! It neither proves the linear/lookup relations nor admits native40/qPCS source.
use super::*;

const COMPLEMENT_FIRST_V1: u32 = S_AFTER_INVENTORY_V1;
const COMPLEMENT_AFTER_V1: u32 = COMPLEMENT_FIRST_V1 + MASK_DIGITS_V1 as u32;
const _: () = {
    assert!(COMPLEMENT_FIRST_V1 == 33_576);
    assert!(COMPLEMENT_AFTER_V1 == 39_976);
};

fn workspace_error_v1(
    error: crate::vega::zk_ams::mkhe::rns_native_resource_budget::RnsNativeResourceErrorV1,
) -> QMaskSErrorV1 {
    match error {
        crate::vega::zk_ams::mkhe::rns_native_resource_budget::RnsNativeResourceErrorV1::WorkspaceLimit => QMaskSErrorV1::Capacity,
        _ => QMaskSErrorV1::Resource,
    }
}

fn complement_coordinate_v1(
    block: QMaskSBlockCoordinateV1,
    digit: usize,
) -> Result<GlobalLookupCommitmentCoordinateV1, QMaskSErrorV1> {
    if digit >= 4 || QMaskSBlockCoordinateV1::from_ordinal_v1(block.ordinal)? != block {
        return Err(QMaskSErrorV1::Source);
    }
    let local = block.ordinal as u32 * 4 + digit as u32;
    let coordinate =
        commitment_coordinate_v1(COMPLEMENT_FIRST_V1 + local).map_err(|_| QMaskSErrorV1::Source)?;
    if coordinate.phase != GlobalLookupCommitmentPhaseV1::ChallengeIndependent
        || coordinate.purpose != GlobalLookupCommitmentPurposeV1::QMaskComplementDigit
        || coordinate.purpose_ordinal != local
    {
        return Err(QMaskSErrorV1::Source);
    }
    Ok(coordinate)
}

fn complement_digit_v1(
    value: u64,
    modulus: u64,
    digit: usize,
) -> Result<u16, crate::generalized_bulletproof::GeneralizedBulletproofErrorV1> {
    let value = Zeroizing::new(value);
    if modulus == 0 || modulus >= 1 << 60 || *value >= modulus || digit >= 4 {
        return Err(
            crate::generalized_bulletproof::GeneralizedBulletproofErrorV1::ArithmeticInvariant,
        );
    }
    // Integer subtraction precedes radix extraction. Per-digit subtraction from
    // 32767 would describe 2^60-1-S rather than the specified q-1-S.
    let complement = Zeroizing::new(modulus - 1 - *value);
    Ok(((*complement >> (15 * digit)) & RADIX_MASK_V1) as u16)
}

/// Only the added original complement masks and phase counters live here.
/// Scalar payload destruction precedes release of their original reservation.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct QMaskComplementOpeningsV1
{
    blindings: ZeroizingT256ScalarVecV1,
    commitment_entropy_before: u64,
    mask_entropy_before: u64,
    reservation: RnsNativeResourceReservationV1,
}

struct ComplementBlockAdmissionV1 {
    evaluations: [Option<RnsNativeU15EvaluationAdmissionV1>; 4],
    _reservation: RnsNativeResourceReservationV1,
}
impl ComplementBlockAdmissionV1 {
    fn new_v1<R>(
        session: &mut GlobalLookupCommitmentSessionLiveV1<R>,
        table: &RnsNativeU15MsmTableV1,
    ) -> Result<Self, QMaskSErrorV1> {
        // The existing leaf owner already funds the serial borrowed record and
        // decrypt scratch. These are the new rho/point/integer temporary owners;
        // the u15 owner independently funds each actual digit and MSM lifetime.
        let scratch = SOURCE_OPENING_BLINDING_SLOT_BYTES_V1 as usize
            + core::mem::size_of::<ConfidentialSpoolChunkV1>()
            + 2 * core::mem::size_of::<Scalar>()
            + core::mem::size_of::<ZeroizingT256ScalarCopyV1>()
            + core::mem::size_of::<[u8; 32]>()
            + core::mem::size_of::<[u8; 33]>()
            + core::mem::size_of::<GlobalLookupCommitmentTicketV1>()
            + 2 * core::mem::size_of::<Zeroizing<u64>>();
        let reservation = session
            .proof_resources
            .reserve_workspace_v1(core::mem::size_of::<Self>() as u64, scratch as u64)
            .map_err(workspace_error_v1)?;
        let mut evaluations = core::array::from_fn(|_| None);
        for evaluation in &mut evaluations {
            *evaluation = Some(
                table
                    .admit_evaluation_v1(&session.proof_resources)
                    .map_err(kernel_error_v1)?,
            );
        }
        Ok(Self {
            evaluations,
            _reservation: reservation,
        })
    }
}

impl QMaskComplementOpeningsV1 {
    pub(in super::super::super) fn new_v1<R>(
        session: &mut GlobalLookupCommitmentSessionLiveV1<R>,
        table: &RnsNativeU15MsmTableV1,
        source: &CompleteQMaskSOpeningsV1,
        file: &SealedQMaskSFileV1,
    ) -> Result<Self, QMaskSErrorV1> {
        require_original_block_v1(session, table, &source.stream.block)?;
        if source.loaded
            || source.next_block != 0
            || source.stream.blindings.len() != MASK_DIGITS_V1
            || !source
                .stream
                .reservation
                .belongs_to_v1(&session.proof_resources)
            || !source
                .stream
                .block
                .coefficients
                .values
                .iter()
                .all(|value| *value == 0)
            || session.next_global_ordinal != COMPLEMENT_FIRST_V1
            || session.next_purpose != GlobalLookupCommitmentPurposeV1::QMaskComplementDigit
            || session.next_purpose_ordinal != 0
        {
            return Err(QMaskSErrorV1::Source);
        }
        file.require_original_v1(source.stream.block.binding, &session.proof_resources)
            .map_err(QMaskSErrorV1::Storage)?;
        file.require_next_block_v1(0)
            .map_err(QMaskSErrorV1::Storage)?;
        let counters = entropy_counters_v1(session)?;
        require_sampled_entropy_v1(
            QMaskSBlockCoordinateV1::from_ordinal_v1(MASK_BLOCKS_V1 - 1)?,
            counters.1,
        )?;
        let reservation = session
            .proof_resources
            .reserve_workspace_v1(
                (MASK_DIGITS_V1 * core::mem::size_of::<Scalar>() + core::mem::size_of::<Self>())
                    as u64,
                0,
            )
            .map_err(workspace_error_v1)?;
        let blindings = ZeroizingT256ScalarVecV1::try_with_exact_capacity(MASK_DIGITS_V1)
            .map_err(|_| QMaskSErrorV1::Resource)?;
        let result = Self {
            blindings,
            commitment_entropy_before: counters.0,
            mask_entropy_before: counters.1,
            reservation,
        };
        result.require_progress_v1(session, table, source, file)?;
        Ok(result)
    }

    fn require_progress_v1<R>(
        &self,
        session: &GlobalLookupCommitmentSessionLiveV1<R>,
        table: &RnsNativeU15MsmTableV1,
        source: &CompleteQMaskSOpeningsV1,
        file: &SealedQMaskSFileV1,
    ) -> Result<(), QMaskSErrorV1> {
        require_original_block_v1(session, table, &source.stream.block)?;
        let count = self.blindings.len();
        if count > MASK_DIGITS_V1
            || count % 4 != 0
            || source.loaded
            || source.next_block != count / 4
            || source.stream.blindings.len() != MASK_DIGITS_V1
            || !source
                .stream
                .reservation
                .belongs_to_v1(&session.proof_resources)
            || !self.reservation.belongs_to_v1(&session.proof_resources)
            || session.next_global_ordinal != COMPLEMENT_FIRST_V1 + count as u32
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
        let counters = entropy_counters_v1(session)?;
        let used = counters
            .0
            .checked_sub(self.commitment_entropy_before)
            .ok_or(QMaskSErrorV1::Source)?;
        let minimum = count as u64 * 32;
        if counters.1 != self.mask_entropy_before
            || used < minimum
            || used > minimum * MAX_RANDOM_REJECTION_ATTEMPTS_V1 as u64
            || used % 32 != 0
        {
            return Err(QMaskSErrorV1::Source);
        }
        file.require_original_v1(source.stream.block.binding, &session.proof_resources)
            .map_err(QMaskSErrorV1::Storage)?;
        file.require_next_block_v1(source.next_block)
            .map_err(QMaskSErrorV1::Storage)
    }

    pub(in super::super::super) fn produce_next_v1<R: MaskedRelaxedRandomSourceV1>(
        &mut self,
        session: &mut GlobalLookupCommitmentSessionLiveV1<R>,
        table: &mut RnsNativeU15MsmTableV1,
        source: &mut CompleteQMaskSOpeningsV1,
        file: &mut SealedQMaskSFileV1,
    ) -> Result<(), QMaskSErrorV1> {
        self.require_progress_v1(session, table, source, file)?;
        let block = QMaskSBlockCoordinateV1::from_ordinal_v1(source.next_block)?;
        let first = complement_coordinate_v1(block, 0)?.global_ordinal as usize;
        if session
            .inventory
            .slots
            .get(first..first + 4)
            .is_none_or(|tickets| tickets.iter().any(Option::is_some))
        {
            return Err(QMaskSErrorV1::Source);
        }
        // Both possible Capacity returns precede any read, entropy, slot write
        // or input mutation. Unused evaluator reservations drop on read refusal.
        let mut admission = ComplementBlockAdmissionV1::new_v1(session, table)?;
        source.load_next_original_block_v1(session, file)?;
        if source.stream.block.coordinate != block || !source.loaded {
            return Err(QMaskSErrorV1::Source);
        }
        let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[block.limb];
        for digit in 0..4 {
            let coordinate = complement_coordinate_v1(block, digit)?;
            if session.next_global_ordinal != coordinate.global_ordinal
                || session.next_purpose != coordinate.purpose
                || session.next_purpose_ordinal != coordinate.purpose_ordinal
                || session.inventory.slots[coordinate.global_ordinal as usize].is_some()
                || self.blindings.len() != block.ordinal * 4 + digit
            {
                return Err(QMaskSErrorV1::Source);
            }
            let (encoded, rho) =
                sample_blinding_v1(&mut session.entropy, coordinate.global_ordinal).map_err(
                    |error| match error {
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
                        complement_digit_v1(
                            source.stream.block.coefficients.values[index],
                            modulus,
                            digit,
                        )
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
        source.erase_loaded_block_v1()?;
        self.require_progress_v1(session, table, source, file)
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn at_complete_v1(
        &self,
    ) -> bool {
        self.blindings.len() == MASK_DIGITS_V1
    }

    pub(in super::super::super) fn require_complete_v1<R>(
        &self,
        session: &GlobalLookupCommitmentSessionLiveV1<R>,
        table: &RnsNativeU15MsmTableV1,
        source: &CompleteQMaskSOpeningsV1,
        file: &SealedQMaskSFileV1,
    ) -> Result<(), QMaskSErrorV1> {
        self.require_progress_v1(session, table, source, file)?;
        if !self.at_complete_v1()
            || session.next_global_ordinal != COMPLEMENT_AFTER_V1
            || session.next_purpose != GlobalLookupCommitmentPurposeV1::Multiplicity
            || session.next_purpose_ordinal != 0
        {
            return Err(QMaskSErrorV1::Source);
        }
        source.require_replayed_v1(file)?;
        for (index, rho) in self.blindings.as_slice().iter().enumerate() {
            let expected = complement_coordinate_v1(
                QMaskSBlockCoordinateV1::from_ordinal_v1(index / 4)?,
                index % 4,
            )?;
            let ticket = session.inventory.slots[expected.global_ordinal as usize]
                .as_ref()
                .ok_or(QMaskSErrorV1::Source)?;
            if ticket.coordinate != expected || rho.is_zero() {
                return Err(QMaskSErrorV1::Source);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "complement_v1_tests.rs"]
pub(super) mod tests;

#[cfg(all(test, unix))]
pub(in super::super::super) use tests::with_complement_for_retained_refusal_v1;
