//! Whole original S leaf seal and bounded authenticated sequential block replay.
//! This storage owner grants no source, opening, qPCS or production authority.
use super::*;

impl WrittenQMaskSBlockFileV1 {
    pub(in crate::vega::zk_ams::mkhe) fn block_ordinal_v1(
        &self,
    ) -> Result<usize, OrderedSnapshotErrorV1> {
        self.require_block_binding_v1()?;
        Ok((self.file.next_slot / S_FIRST_BLOCK_SLOTS_V1 - 1) as usize)
    }
    /// The consuming source caller has already completed the current four
    /// original tickets. This leaf transition grants only physical write custody.
    pub(in crate::vega::zk_ams::mkhe) fn resume_next_block_v1(
        mut self,
    ) -> Result<QMaskSFileV1, OrderedSnapshotErrorV1> {
        self.require_block_binding_v1()?;
        if self.file.block_end_slot == S_SLOTS_V1 {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        self.file.block_end_slot += S_FIRST_BLOCK_SLOTS_V1;
        Ok(self.file)
    }
    pub(in crate::vega::zk_ams::mkhe) fn seal_v1(
        self,
    ) -> Result<SealedQMaskSFileV1, OrderedSnapshotErrorV1> {
        self.require_block_binding_v1()?;
        if self.file.next_slot != S_SLOTS_V1 {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        let live = self.file.live.ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        // All failure/unwind destruction closes the leaf before releasing file
        // credits. Named operation memory remains live through the actual seal.
        let memory = live.memory;
        let mut reservation = live.reservation;
        let writer = live.writer;
        reservation
            .charge_reserved_io_v1(S_FILE_BYTES_V1)
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?;
        let snapshot = writer
            .seal_v1()
            .map_err(|_| OrderedSnapshotErrorV1::Storage)?;
        reservation
            .require_sealed_v1()
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?;
        let digest = *snapshot.snapshot_digest_v1();
        let result = SealedQMaskSFileV1 {
            live: Some(SealedQMaskSFileLiveV1 {
                snapshot,
                reservation,
                memory,
            }),
            context: self.file.context,
            digest,
            next_block: 0,
        };
        result.validate_v1()?;
        Ok(result)
    }
}

struct SealedQMaskSFileLiveV1 {
    snapshot: ConfidentialSpoolSnapshotV1,
    reservation: OrderedStorageReservationV1,
    memory: QMaskSFileMemoryV1,
}
/// The same actual unlinked S file/key, original memory and storage ledgers.
/// There is no replacement snapshot, caller root, reset or generic read API.
pub(in crate::vega::zk_ams::mkhe) struct SealedQMaskSFileV1 {
    live: Option<SealedQMaskSFileLiveV1>,
    context: [u8; 32],
    digest: [u8; 32],
    next_block: usize,
}
impl SealedQMaskSFileV1 {
    fn validate_v1(&self) -> Result<(), OrderedSnapshotErrorV1> {
        let live = self.live.as_ref().ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        live.reservation
            .require_sealed_v1()
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?;
        if self.context != live.memory.binding
            || self.digest == [0; 32]
            || self.digest != *live.snapshot.snapshot_digest_v1()
            || live.snapshot.slot_count_v1() != S_SLOTS_V1
            || live.snapshot.plaintext_len_v1() != S_SLOT_BYTES_V1
            || live.snapshot.ciphertext_record_len_v1() != S_SLOT_BYTES_V1 + 16
            || live.snapshot.file_len_v1() != S_FILE_BYTES_V1
            || self.next_block > (S_SLOTS_V1 / S_FIRST_BLOCK_SLOTS_V1) as usize
        {
            return Err(OrderedSnapshotErrorV1::Context);
        }
        Ok(())
    }
    pub(in crate::vega::zk_ams::mkhe) fn require_original_v1(
        &self,
        binding: [u8; 32],
        budget: &RnsNativeProofResourceBudgetV1,
    ) -> Result<(), OrderedSnapshotErrorV1> {
        self.validate_v1()?;
        let live = self.live.as_ref().ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        if self.context != binding || !live.memory.reservation.belongs_to_v1(budget) {
            return Err(OrderedSnapshotErrorV1::Context);
        }
        Ok(())
    }
    /// Inspect only the exact original sequential-read frontier before funding.
    pub(in crate::vega::zk_ams::mkhe) fn require_next_block_v1(
        &self,
        block: usize,
    ) -> Result<(), OrderedSnapshotErrorV1> {
        self.validate_v1()?;
        if self.next_block != block {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        Ok(())
    }
    /// Charge all eight real record reads before allocating or touching any
    /// record. Temporary capacity pressure preserves the exact sealed owner.
    pub(in crate::vega::zk_ams::mkhe) fn begin_block_read_v1(
        &mut self,
        block: usize,
    ) -> Result<QMaskSBlockReadV1<'_>, OrderedSnapshotErrorV1> {
        if let Err(error) = self.validate_v1() {
            self.live = None;
            return Err(error);
        }
        let mut live = self.live.take().ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        if block != self.next_block || block >= (S_SLOTS_V1 / S_FIRST_BLOCK_SLOTS_V1) as usize {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        match live
            .reservation
            .charge_read_io_v1(S_FIRST_BLOCK_SLOTS_V1 * (S_SLOT_BYTES_V1 + 16))
        {
            Ok(()) => {}
            Err(StorageBudgetErrorV1::IoLimit) => {
                self.live = Some(live);
                return Err(OrderedSnapshotErrorV1::Capacity);
            }
            Err(_) => return Err(OrderedSnapshotErrorV1::Resource),
        }
        self.live = Some(live);
        Ok(QMaskSBlockReadV1 {
            file: self,
            first_slot: block as u64 * S_FIRST_BLOCK_SLOTS_V1,
            next_slot: 0,
            finished: false,
        })
    }
    pub(in crate::vega::zk_ams::mkhe) fn require_replayed_v1(
        &self,
    ) -> Result<(), OrderedSnapshotErrorV1> {
        self.validate_v1()?;
        if self.next_block != (S_SLOTS_V1 / S_FIRST_BLOCK_SLOTS_V1) as usize {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        Ok(())
    }
}
/// Borrowed actual eight-read lifetime; incomplete or failed consumption closes
/// the file/key. No serializable permit or returned raw handle exists.
pub(in crate::vega::zk_ams::mkhe) struct QMaskSBlockReadV1<'a> {
    file: &'a mut SealedQMaskSFileV1,
    first_slot: u64,
    next_slot: u64,
    finished: bool,
}
/// One actual zeroizing plaintext allocation borrowed from its funded read.
/// The exclusive borrow forbids another slot or parent disposal until this
/// chunk is destroyed. No owning plaintext, clone or raw-handle escape exists.
pub(in crate::vega::zk_ams::mkhe) struct QMaskSReadChunkV1<'a> {
    chunk: ConfidentialSpoolChunkV1,
    _parent_read: core::marker::PhantomData<&'a mut ()>,
}
// The existing named operation charge already funds this exact chunk owner;
// the lifetime marker/destructor introduce neither allocation nor layout bytes.
const _: () = assert!(
    core::mem::size_of::<QMaskSReadChunkV1<'static>>()
        == core::mem::size_of::<ConfidentialSpoolChunkV1>()
);
impl QMaskSReadChunkV1<'_> {
    pub(in crate::vega::zk_ams::mkhe) fn len_v1(&self) -> u64 {
        self.chunk.len_v1()
    }
    pub(in crate::vega::zk_ams::mkhe) fn as_slice_v1(&self) -> &[u8] {
        self.chunk.as_slice_v1()
    }
}
impl Drop for QMaskSReadChunkV1<'_> {
    fn drop(&mut self) {
        // This destructor intentionally retains drop-check's exclusive parent
        // borrow through scope-end destruction, even after the last plaintext
        // use. PhantomData alone permits that borrow to end early under NLL.
        // The real chunk field then erases/deallocates through its existing Drop
        // before the parent read/file/memory owner can be released.
    }
}
impl QMaskSBlockReadV1<'_> {
    pub(in crate::vega::zk_ams::mkhe) fn read_next_slot_v1(
        &mut self,
    ) -> Result<QMaskSReadChunkV1<'_>, OrderedSnapshotErrorV1> {
        if self.next_slot >= S_FIRST_BLOCK_SLOTS_V1 {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        let mut live = self
            .file
            .live
            .take()
            .ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        let chunk = live
            .snapshot
            .read_slot_v1(self.first_slot + self.next_slot, self.file.context)
            .map_err(|_| OrderedSnapshotErrorV1::Storage)?;
        self.file.live = Some(live);
        self.next_slot += 1;
        Ok(QMaskSReadChunkV1 {
            chunk,
            _parent_read: core::marker::PhantomData,
        })
    }
    pub(in crate::vega::zk_ams::mkhe) fn finish_v1(mut self) -> Result<(), OrderedSnapshotErrorV1> {
        if self.next_slot != S_FIRST_BLOCK_SLOTS_V1 || self.file.live.is_none() {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        self.file.next_block += 1;
        self.finished = true;
        Ok(())
    }
}
impl Drop for QMaskSBlockReadV1<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.file.live = None;
        }
    }
}

#[cfg(test)]
#[path = "stream_v1_tests.rs"]
mod tests;
