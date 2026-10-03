//! Inert claim-first persistence and exact readback through the borrowed Unix handle owner.

use super::{
    format::{ClaimDescriptorV1, ClaimFrameV1, decode_claim},
    *,
};
use iroha_fs::BorrowedSealedPrivateFile;
use std::{
    ffi::OsStr,
    io::{Read as _, Seek as _, Write as _},
};

impl AdmittedControlSlotV1<'_> {
    /// Retain C unchanged on every result. A complete existing pair is only an exact local
    /// retry; no result here means the original transaction has or has not entered Queue.
    pub(super) fn persist<C>(
        &self,
        original: CapturedExactControlWireV1<C>,
    ) -> Result<StoredControlCustodyV1<C>, ControlStoreFailureV1<C>> {
        let result = (|| {
            self.owner.limits.validate()?;
            if original.binding != self.owner.binding
                || original.slot != self.slot
                || !original.bytes.belongs_to(&self.owner.budget)
            {
                return Err(ControlJournalErrorV1::Invalid);
            }
            let descriptor =
                ClaimDescriptorV1::new(self.owner.binding, self.slot, original.bytes.as_slice())?;
            let frame = ClaimFrameV1::encode(&descriptor, &self.owner.budget)?;
            let record_bytes = (frame.0.as_slice().len() as u64)
                .checked_add(original.bytes.as_slice().len() as u64)
                .ok_or(ControlJournalErrorV1::Capacity)?;
            if record_bytes > self.admitted_record_bytes
                || self.admitted_record_bytes > self.owner.limits.max_total_bytes
            {
                return Err(ControlJournalErrorV1::Capacity);
            }
            self.persist_exact(&frame, original.bytes.as_slice())
        })();
        match result {
            Ok(()) => Ok(StoredControlCustodyV1 { original }),
            Err(error) => Err(ControlStoreFailureV1 { original, error }),
        }
    }

    fn persist_exact(
        &self,
        frame: &ClaimFrameV1,
        wire: &[u8],
    ) -> Result<(), ControlJournalErrorV1> {
        let directory = self.owner.directory;
        directory
            .revalidate()
            .map_err(ControlJournalErrorV1::Storage)?;
        let claim = open_optional(directory, self.names.claim(), MAX_CLAIM_BYTES_V1)?;
        let existing_wire = open_optional(directory, self.names.wire(), MAX_WIRE_BYTES_V1)?;
        match (claim, existing_wire) {
            (Some(mut claim), Some(mut existing_wire)) => {
                compare_exact(&mut claim, frame.0.as_slice())?;
                compare_exact(&mut existing_wire, wire)?;
                claim.revalidate().map_err(ControlJournalErrorV1::Storage)?;
                existing_wire
                    .revalidate()
                    .map_err(ControlJournalErrorV1::Storage)?;
                return Ok(());
            }
            (None, None) => {}
            _ => return Err(ControlJournalErrorV1::OccupiedIncomplete),
        }

        // Claim the semantic slot before writing any signed wire. No failure path below
        // removes either name. A crash, short write or uncertain fsync occupies the slot.
        let mut claim = directory
            .create_borrowed_private(self.names.claim(), MAX_CLAIM_BYTES_V1)
            .map_err(ControlJournalErrorV1::Storage)?;
        claim
            .write_all(frame.0.as_slice())
            .map_err(ControlJournalErrorV1::Storage)?;
        let mut claim = claim
            .seal_read_only()
            .map_err(ControlJournalErrorV1::Storage)?;
        let mut stored_wire = directory
            .create_borrowed_private(self.names.wire(), MAX_WIRE_BYTES_V1)
            .map_err(ControlJournalErrorV1::Storage)?;
        stored_wire
            .write_all(wire)
            .map_err(ControlJournalErrorV1::Storage)?;
        let mut stored_wire = stored_wire
            .seal_read_only()
            .map_err(ControlJournalErrorV1::Storage)?;
        compare_exact(&mut claim, frame.0.as_slice())?;
        compare_exact(&mut stored_wire, wire)?;
        claim.revalidate().map_err(ControlJournalErrorV1::Storage)?;
        stored_wire
            .revalidate()
            .map_err(ControlJournalErrorV1::Storage)?;
        Ok(())
    }

    /// Return opaque original bytes, never a recreated signed transaction or live deadline.
    pub(super) fn recover_inert(
        &self,
    ) -> Result<RecoveredControlBytesV1<'_>, ControlJournalErrorV1> {
        self.owner.limits.validate()?;
        let directory = self.owner.directory;
        let mut claim = open_optional(directory, self.names.claim(), MAX_CLAIM_BYTES_V1)?
            .ok_or(ControlJournalErrorV1::OccupiedIncomplete)?;
        let mut stored_wire = open_optional(directory, self.names.wire(), MAX_WIRE_BYTES_V1)?
            .ok_or(ControlJournalErrorV1::OccupiedIncomplete)?;
        let claim_length = claim.len().map_err(ControlJournalErrorV1::Storage)?;
        let wire_length = stored_wire.len().map_err(ControlJournalErrorV1::Storage)?;
        let record_length = claim_length
            .checked_add(wire_length)
            .ok_or(ControlJournalErrorV1::Capacity)?;
        if record_length > self.admitted_record_bytes
            || self.admitted_record_bytes > self.owner.limits.max_total_bytes
        {
            return Err(ControlJournalErrorV1::Capacity);
        }
        // Prepay both actual byte backings before constructing either recovery buffer. There
        // is no parent/child reacquisition or unrelated per-operation replacement pool.
        let claim_length =
            usize::try_from(claim_length).map_err(|_| ControlJournalErrorV1::Invalid)?;
        let wire_length =
            usize::try_from(wire_length).map_err(|_| ControlJournalErrorV1::Invalid)?;
        let claim_layout = std::alloc::Layout::array::<u8>(claim_length)
            .map_err(|_| ControlJournalErrorV1::Invalid)?;
        let wire_layout = std::alloc::Layout::array::<u8>(wire_length)
            .map_err(|_| ControlJournalErrorV1::Invalid)?;
        let mut reservation = self
            .owner
            .budget
            .try_reserve_layouts([claim_layout, wire_layout])
            .map_err(|original| ControlJournalErrorV1::Deferred(original.into()))?;
        let mut claim_bytes = prepaid_bytes(claim_length, &mut reservation)?;
        let mut wire_bytes = prepaid_bytes(wire_length, &mut reservation)?;
        read_exact_into(&mut claim, &mut claim_bytes)?;
        let descriptor = decode_claim(claim_bytes.as_slice())?;
        if descriptor.binding != self.owner.binding
            || descriptor.slot != self.slot
            || descriptor.wire_length as usize != wire_length
        {
            return Err(ControlJournalErrorV1::Conflict);
        }
        read_exact_into(&mut stored_wire, &mut wire_bytes)?;
        if !descriptor.matches_wire(wire_bytes.as_slice()) {
            return Err(ControlJournalErrorV1::Conflict);
        }
        claim.revalidate().map_err(ControlJournalErrorV1::Storage)?;
        stored_wire
            .revalidate()
            .map_err(ControlJournalErrorV1::Storage)?;
        // Explicitly close both native children before releasing recovery scratch credits.
        drop(stored_wire);
        drop(claim);
        drop(claim_bytes);
        Ok(RecoveredControlBytesV1 {
            state: self.owner.state,
            bytes: wire_bytes,
        })
    }
}

fn open_optional<'a>(
    directory: &'a PrivateDirectory,
    name: &'a OsStr,
    maximum: usize,
) -> Result<Option<BorrowedSealedPrivateFile<'a>>, ControlJournalErrorV1> {
    match directory.open_borrowed_read_only(name, maximum) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            directory
                .revalidate()
                .map_err(ControlJournalErrorV1::Storage)?;
            Ok(None)
        }
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            Err(ControlJournalErrorV1::OccupiedIncomplete)
        }
        Err(error) => Err(ControlJournalErrorV1::Storage(error)),
    }
}

fn compare_exact(
    file: &mut BorrowedSealedPrivateFile<'_>,
    expected: &[u8],
) -> Result<(), ControlJournalErrorV1> {
    if file.len().map_err(ControlJournalErrorV1::Storage)? != expected.len() as u64 {
        return Err(ControlJournalErrorV1::Conflict);
    }
    let before = file.snapshot().map_err(ControlJournalErrorV1::Storage)?;
    file.rewind().map_err(ControlJournalErrorV1::Storage)?;
    let mut scratch = [0; 1024];
    for piece in expected.chunks(scratch.len()) {
        file.read_exact(&mut scratch[..piece.len()])
            .map_err(ControlJournalErrorV1::Storage)?;
        if &scratch[..piece.len()] != piece {
            return Err(ControlJournalErrorV1::Conflict);
        }
    }
    if file.snapshot().map_err(ControlJournalErrorV1::Storage)? != before {
        return Err(ControlJournalErrorV1::Conflict);
    }
    Ok(())
}

fn read_exact_into(
    file: &mut BorrowedSealedPrivateFile<'_>,
    bytes: &mut ChargedBuffer<u8>,
) -> Result<(), ControlJournalErrorV1> {
    if !bytes.as_slice().is_empty()
        || file.len().map_err(ControlJournalErrorV1::Storage)? != bytes.capacity() as u64
    {
        return Err(ControlJournalErrorV1::Invalid);
    }
    let before = file.snapshot().map_err(ControlJournalErrorV1::Storage)?;
    file.rewind().map_err(ControlJournalErrorV1::Storage)?;
    let mut scratch = [0; 1024];
    while bytes.as_slice().len() < bytes.capacity() {
        let remaining = (bytes.capacity() - bytes.as_slice().len()).min(scratch.len());
        file.read_exact(&mut scratch[..remaining])
            .map_err(ControlJournalErrorV1::Storage)?;
        bytes
            .append(&scratch[..remaining])
            .map_err(|_| ControlJournalErrorV1::Invalid)?;
    }
    if file.snapshot().map_err(ControlJournalErrorV1::Storage)? != before {
        return Err(ControlJournalErrorV1::Conflict);
    }
    Ok(())
}

fn prepaid_bytes(
    length: usize,
    reservation: &mut iroha_allocation::AllocationReservation,
) -> Result<ChargedBuffer<u8>, ControlJournalErrorV1> {
    ChargedBuffer::from_reservation(length, reservation).map_err(|error| match error {
        iroha_allocation::PrepaidBufferError::Allocation(error) => error.into(),
        // A split mismatch is a violated local layout plan, not a new capacity refusal. Never
        // manufacture a wake source or reacquire a different pool in this branch.
        iroha_allocation::PrepaidBufferError::Reservation(_) => ControlJournalErrorV1::Invalid,
    })
}
