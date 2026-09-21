//! Same-ledger S storage in exact eight-record block lifetimes.
//!
//! The whole canonical file and write/seal I/O are admitted before creation. This
//! writer closes after every eight slots. Its original consuming source owner
//! completes four same-block opening tickets before it resumes the next block.
use super::*;
use crate::vega::zk_ams::mkhe::rns_native_resource_budget::{
    RnsNativeProofResourceBudgetV1, RnsNativeResourceErrorV1, RnsNativeResourceReservationV1,
};
use crate::vega::{
    bulletproof_t256::ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
    zk_ams::mkhe::rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1, ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1,
    },
};

const S_CONTEXT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.q-mask.original-S.local-spool\0";
const S_LAYOUT_V1: &[u8] = b"limb,repetition,coefficient;40,5,131072;u64le;2048/slot;top-zero;private-uniform-S;pre-initial-root";
const S_SLOTS_V1: u64 = 12_800;
const S_SLOT_BYTES_V1: u64 = 16_384;
const S_FIRST_BLOCK_SLOTS_V1: u64 = 8;
const S_FILE_BYTES_V1: u64 = S_SLOTS_V1 * (S_SLOT_BYTES_V1 + 16);
const _: () = assert!(S_FILE_BYTES_V1 == 209_920_000);

/// Private geometry/context made only by the original sealed pair. It grants
/// neither file access nor proof/source authority and has no public constructor.
pub(in crate::vega::zk_ams::mkhe) struct QMaskSFilePlanV1 {
    layout: ConfidentialSpoolLayoutV1,
    original_snapshot: [u8; 32],
    context: [u8; 32],
}
impl QMaskSFilePlanV1 {
    pub(in crate::vega::zk_ams::mkhe) fn binding_v1(&self) -> [u8; 32] {
        self.context
    }
    pub(in crate::vega::zk_ams::mkhe) fn named_memory_v1(&self) -> (usize, usize) {
        // Counting both the wrapper and leaf handle deliberately overcounts the
        // in-place leaf. Private key and operation payloads remain leaf-owned.
        (
            (core::mem::size_of::<QMaskSFileV1>()
                + core::mem::size_of::<SealedQMaskSFileV1>()
                + core::mem::size_of::<QMaskSBlockReadV1<'_>>())
                + self.layout.named_retained_bytes_v1(),
            self.layout.named_operation_workspace_bytes_v1(),
        )
    }
    pub(in crate::vega::zk_ams::mkhe) fn reserve_memory_v1(
        &self,
        budget: &mut RnsNativeProofResourceBudgetV1,
    ) -> Result<QMaskSFileMemoryV1, OrderedSnapshotErrorV1> {
        let (retained, scratch) = self.named_memory_v1();
        let reservation = budget
            .reserve_workspace_v1(retained as u64, scratch as u64)
            .map_err(|e| match e {
                RnsNativeResourceErrorV1::WorkspaceLimit => OrderedSnapshotErrorV1::Capacity,
                _ => OrderedSnapshotErrorV1::Resource,
            })?;
        Ok(QMaskSFileMemoryV1 {
            binding: self.context,
            reservation,
        })
    }
}

/// Actual leaf-memory custody; its fields and underfunded construction are private.
pub(in crate::vega::zk_ams::mkhe) struct QMaskSFileMemoryV1 {
    binding: [u8; 32],
    reservation: RnsNativeResourceReservationV1,
}
impl QMaskSFileMemoryV1 {
    pub(in crate::vega::zk_ams::mkhe) fn reserve_block_workspace_v1(
        &self,
        retained: u64,
        scratch: u64,
    ) -> Result<RnsNativeResourceReservationV1, OrderedSnapshotErrorV1> {
        self.reservation
            .reserve_child_workspace_v1(retained, scratch)
            .map_err(|e| match e {
                RnsNativeResourceErrorV1::WorkspaceLimit => OrderedSnapshotErrorV1::Capacity,
                _ => OrderedSnapshotErrorV1::Resource,
            })
    }
}

impl OrderedPlaneSpoolSnapshotV1 {
    pub(in crate::vega::zk_ams::mkhe) fn q_mask_s_file_plan_v1(
        &self,
    ) -> Result<QMaskSFilePlanV1, OrderedSnapshotErrorV1> {
        self.validate_live_v1()?;
        self.live
            .as_ref()
            .ok_or(OrderedSnapshotErrorV1::Poisoned)?
            .reservation
            .require_sealed_v1()
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?;
        let mut hash = Keccak256::new();
        hash.update(S_CONTEXT_DOMAIN_V1);
        hash.update(&[1]);
        // These are the actual early original source/materialization context
        // and encrypted pair identity. No qPCS root or verifier weight exists.
        hash.update(&self.plan.plane_context);
        hash.update(&self.digest);
        hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
        hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1]);
        for modulus in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1 {
            hash.update(&modulus.to_be_bytes());
        }
        hash.update(&S_SLOTS_V1.to_be_bytes());
        hash.update(&S_SLOT_BYTES_V1.to_be_bytes());
        hash.update(S_LAYOUT_V1);
        let context = nonzero_v1(hash.finalize())?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(S_SLOTS_V1, S_SLOT_BYTES_V1, context)
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?;
        if layout.file_len_v1() != S_FILE_BYTES_V1 {
            return Err(OrderedSnapshotErrorV1::Resource);
        }
        Ok(QMaskSFilePlanV1 {
            layout,
            original_snapshot: self.digest,
            context,
        })
    }

    /// The caller has already reserved the plan's named memory on the original
    /// proof ledger. This method admits actual file/I/O on this pair's ledger.
    pub(in crate::vega::zk_ams::mkhe) fn create_q_mask_s_file_v1(
        &self,
        directory: &Path,
        plan: QMaskSFilePlanV1,
        memory: QMaskSFileMemoryV1,
    ) -> Result<QMaskSFileV1, OrderedSnapshotErrorV1> {
        self.create_q_mask_s_file_with_v1(directory, plan, memory, |path, layout| {
            ConfidentialSpoolWriterV1::create_in_v1(path, layout)
        })
    }

    fn create_q_mask_s_file_with_v1(
        &self,
        directory: &Path,
        plan: QMaskSFilePlanV1,
        memory: QMaskSFileMemoryV1,
        create: impl FnOnce(
            &Path,
            ConfidentialSpoolLayoutV1,
        ) -> Result<ConfidentialSpoolWriterV1, ConfidentialSpoolErrorV1>,
    ) -> Result<QMaskSFileV1, OrderedSnapshotErrorV1> {
        let expected = self.q_mask_s_file_plan_v1()?;
        if memory.binding != plan.context
            || plan.layout != expected.layout
            || plan.context != expected.context
            || plan.original_snapshot != self.digest
        {
            return Err(OrderedSnapshotErrorV1::Context);
        }
        let original = self.live.as_ref().ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        let reservation = original
            .reservation
            .reserve_sibling_file_v1(plan.layout.file_len_v1())
            .map_err(|error| match error {
                StorageBudgetErrorV1::SpoolLimit | StorageBudgetErrorV1::IoLimit => {
                    OrderedSnapshotErrorV1::Capacity
                }
                _ => OrderedSnapshotErrorV1::Resource,
            })?;
        // Reverse local destruction and field order both close the real file
        // before its same-Arc reservation releases live file bytes.
        let writer = create(directory, plan.layout).map_err(|_| OrderedSnapshotErrorV1::Storage)?;
        Ok(QMaskSFileV1 {
            live: Some(QMaskSFileLiveV1 {
                writer,
                reservation,
                memory,
            }),
            context: plan.context,
            next_slot: 0,
            block_end_slot: S_FIRST_BLOCK_SLOTS_V1,
        })
    }
}

struct QMaskSFileLiveV1 {
    writer: ConfidentialSpoolWriterV1,
    reservation: OrderedStorageReservationV1,
    memory: QMaskSFileMemoryV1,
}

/// Actual partial S file and its original storage reservation. No raw writer,
/// reset, new budget, late root or caller file can be installed.
pub(in crate::vega::zk_ams::mkhe) struct QMaskSFileV1 {
    live: Option<QMaskSFileLiveV1>,
    context: [u8; 32],
    next_slot: u64,
    block_end_slot: u64,
}
impl QMaskSFileV1 {
    pub(in crate::vega::zk_ams::mkhe) fn binding_v1(&self) -> [u8; 32] {
        self.context
    }
    pub(in crate::vega::zk_ams::mkhe) fn require_block_write_v1(
        &self,
        block: usize,
        binding: [u8; 32],
        budget: &RnsNativeProofResourceBudgetV1,
    ) -> Result<(), OrderedSnapshotErrorV1> {
        let live = self.live.as_ref().ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        if self.context != binding
            || !live.memory.reservation.belongs_to_v1(budget)
            || block >= 1600
            || self.next_slot != block as u64 * 8
            || self.block_end_slot != (block as u64 + 1) * 8
        {
            return Err(OrderedSnapshotErrorV1::Context);
        }
        Ok(())
    }
    pub(in crate::vega::zk_ams::mkhe) fn write_slot_v1(
        &mut self,
        slot: u64,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), OrderedSnapshotErrorV1> {
        let mut live = self.live.take().ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        if slot != self.next_slot
            || slot >= self.block_end_slot
            || self.block_end_slot > S_SLOTS_V1
            || chunk.len_v1() != S_SLOT_BYTES_V1
        {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        live.reservation
            .charge_reserved_io_v1(S_SLOT_BYTES_V1 + 16)
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?;
        live.writer
            .write_slot_v1(slot, chunk)
            .map_err(|_| OrderedSnapshotErrorV1::Storage)?;
        self.next_slot += 1;
        self.live = Some(live);
        Ok(())
    }
    pub(in crate::vega::zk_ams::mkhe) fn finish_block_v1(
        self,
    ) -> Result<WrittenQMaskSBlockFileV1, OrderedSnapshotErrorV1> {
        if self.live.is_none()
            || self.next_slot != self.block_end_slot
            || self.block_end_slot == 0
            || self.block_end_slot > S_SLOTS_V1
            || self.block_end_slot % S_FIRST_BLOCK_SLOTS_V1 != 0
        {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        Ok(WrittenQMaskSBlockFileV1 { file: self })
    }
}

/// Physical closed-block custody. The original source caller joins this file
/// with its four same-block tickets before continuing; storage alone does not
/// grant source, opening or proof authority.
pub(in crate::vega::zk_ams::mkhe) struct WrittenQMaskSBlockFileV1 {
    file: QMaskSFileV1,
}

impl WrittenQMaskSBlockFileV1 {
    /// The actual closed leaf keeps the same source proof-ledger reservation.
    pub(in crate::vega::zk_ams::mkhe) fn require_original_budget_v1(
        &self,
        budget: &RnsNativeProofResourceBudgetV1,
    ) -> Result<(), OrderedSnapshotErrorV1> {
        let live = self
            .file
            .live
            .as_ref()
            .ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        if !live.memory.reservation.belongs_to_v1(budget) {
            return Err(OrderedSnapshotErrorV1::Context);
        }
        self.require_block_binding_v1().map(|_| ())
    }

    /// Inspect the original closed eight-record owner; no caller digest enters.
    pub(in crate::vega::zk_ams::mkhe) fn require_block_binding_v1(
        &self,
    ) -> Result<[u8; 32], OrderedSnapshotErrorV1> {
        if self.file.live.is_none()
            || self.file.next_slot != self.file.block_end_slot
            || self.file.block_end_slot == 0
            || self.file.block_end_slot > S_SLOTS_V1
            || self.file.block_end_slot % S_FIRST_BLOCK_SLOTS_V1 != 0
        {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        Ok(self.file.context)
    }
}

#[cfg(test)]
#[path = "q_mask_s_file_v1_tests.rs"]
mod tests;

#[path = "q_mask_s_file_v1/stream_v1.rs"]
mod stream_v1;
pub(in crate::vega::zk_ams::mkhe) use stream_v1::{QMaskSBlockReadV1, SealedQMaskSFileV1};
