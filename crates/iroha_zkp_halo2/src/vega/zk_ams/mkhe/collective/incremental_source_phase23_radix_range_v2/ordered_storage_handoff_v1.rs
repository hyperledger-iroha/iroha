//! Original materialized values joined to their one ordered authenticated pair.
//!
//! Storage completion retains the original source/session and is not native40
//! source qualification, a proof-role witness provider, or a composite seal.
use super::prepared_comparator_plane_v1::validate_materialized_context_v1;
use super::*;
use crate::vega::zk_ams::mkhe::global_lookup_statement_v1::{
    OrderedPlaneSpoolSnapshotV1, OrderedPlaneSpoolWriterV1, OrderedSnapshotErrorV1,
    OrderedStorageSessionBudgetV1, materialized_plane_context_digest_v1,
};
use core::marker::PhantomData;
use std::path::Path;

const PLANES_V1: u16 = 9_288;
const SLOTS_PER_PLANE_V1: u64 = 33;
const SLOTS_V1: u64 = PLANES_V1 as u64 * SLOTS_PER_PLANE_V1;

/// A short borrow of identities validated from the actual original owners.
/// No constructor accepts caller-supplied axes or post-proof records.
pub(in crate::vega::zk_ams::mkhe) struct MaterializedPlaneContextV1<'a> {
    axes: [[u8; 32]; 3],
    original_owner: PhantomData<&'a ()>,
}
impl MaterializedPlaneContextV1<'_> {
    pub(in crate::vega::zk_ams::mkhe) fn axes_v1(&self) -> [[u8; 32]; 3] {
        self.axes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) enum MaterializedStorageErrorV1
{
    Source,
    Storage(OrderedSnapshotErrorV1),
}

/// Only pre-I/O local Capacity retains a retryable whole original source.
#[must_use = "retry the original owner after local capacity becomes available"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct OrderedStorageStartRefusalV1
<R, K, P> {
    reason: MaterializedStorageErrorV1,
    source: Option<Phase23RadixWitnessMaterializedV2<R, K, P>>,
}
impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> OrderedStorageStartRefusalV1<R, K, P> {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn reason_v1(
        &self,
    ) -> MaterializedStorageErrorV1 {
        self.reason
    }

    #[allow(
        clippy::result_large_err,
        reason = "capacity refusal retains the entire original source without allocating or detaching it"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn retry_v1(
        mut self,
        directory: &Path,
        budget: &mut OrderedStorageSessionBudgetV1,
    ) -> Result<Phase23RadixWitnessMaterializedV2<R, K, P>, Self> {
        let Some(source) = self.source.take() else {
            return Err(self);
        };
        source.begin_ordered_storage_v1(directory, budget)
    }
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> Phase23RadixWitnessMaterializedV2<R, K, P> {
    fn validated_plane_context_v1(
        &self,
    ) -> Result<MaterializedPlaneContextV1<'_>, ZkAmsMkheErrorV1> {
        validate_materialized_context_v1(self)?;
        let records = self
            .evidence
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .ordered_storage_records_v1(
                self.record.replay_record_digest,
                self.record.source_receipt_digest,
            )?;
        Ok(MaterializedPlaneContextV1 {
            axes: [records[0], records[1], self.record.record_digest],
            original_owner: PhantomData,
        })
    }

    /// Reserve the existing pair before file, entropy or output-chunk allocation.
    /// The entire source is retained only for a local pre-I/O capacity refusal.
    #[allow(
        clippy::result_large_err,
        reason = "capacity refusal retains the entire original source without allocating or detaching it"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn begin_ordered_storage_v1(
        self,
        directory: &Path,
        budget: &mut OrderedStorageSessionBudgetV1,
    ) -> Result<Self, OrderedStorageStartRefusalV1<R, K, P>> {
        let context = (|| {
            if self.next_comparator_plane != 0 || self.ordered_writer.is_some() {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            materialized_plane_context_digest_v1(&self.validated_plane_context_v1()?)
        })();
        let context = match context {
            Ok(context) => context,
            Err(_) => {
                return Err(OrderedStorageStartRefusalV1 {
                    reason: MaterializedStorageErrorV1::Source,
                    source: None,
                });
            }
        };
        let writer = OrderedPlaneSpoolWriterV1::create_v1(directory, context, budget);
        self.complete_ordered_storage_admission_v1(writer)
    }

    #[allow(
        clippy::result_large_err,
        reason = "capacity refusal retains the entire original source without allocating or detaching it"
    )]
    fn complete_ordered_storage_admission_v1(
        mut self,
        writer: Result<OrderedPlaneSpoolWriterV1, OrderedSnapshotErrorV1>,
    ) -> Result<Self, OrderedStorageStartRefusalV1<R, K, P>> {
        match writer {
            Ok(writer) => {
                self.ordered_writer = Some(writer);
                Ok(self)
            }
            Err(error) => {
                let source = if error == OrderedSnapshotErrorV1::Capacity {
                    Some(self)
                } else {
                    None
                };
                Err(OrderedStorageStartRefusalV1 {
                    reason: MaterializedStorageErrorV1::Storage(error),
                    source,
                })
            }
        }
    }

    /// Consume every successful stored plane plus the unchanged original source.
    /// This does not construct the global-plane production materializer seal.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn seal_ordered_storage_v1(
        mut self,
    ) -> Result<OrderedMaterializedReplayV1<R, K, P>, MaterializedStorageErrorV1> {
        if self.next_comparator_plane != PLANES_V1 {
            return Err(MaterializedStorageErrorV1::Source);
        }
        let context = materialized_plane_context_digest_v1(
            &self
                .validated_plane_context_v1()
                .map_err(|_| MaterializedStorageErrorV1::Source)?,
        )
        .map_err(|_| MaterializedStorageErrorV1::Source)?;
        self.evidence
            .as_mut()
            .ok_or(MaterializedStorageErrorV1::Source)?
            .begin_stored_plane_replay_v1()
            .map_err(|_| MaterializedStorageErrorV1::Source)?;
        let writer = self
            .ordered_writer
            .take()
            .ok_or(MaterializedStorageErrorV1::Source)?;
        writer
            .require_context_v1(context)
            .map_err(MaterializedStorageErrorV1::Storage)?;
        writer
            .require_next_slot_v1(SLOTS_V1)
            .map_err(MaterializedStorageErrorV1::Storage)?;
        let snapshot = writer
            .seal_v1()
            .map_err(MaterializedStorageErrorV1::Storage)?;
        let identity = snapshot
            .snapshot_digest_v1()
            .map_err(MaterializedStorageErrorV1::Storage)?;
        Ok(OrderedMaterializedReplayV1 {
            live: Some(StoredMaterializedLiveV1 {
                source: self,
                snapshot,
                identity,
                next_slot: 0,
            }),
        })
    }
}

struct StoredMaterializedLiveV1<R, K, P> {
    source: Phase23RadixWitnessMaterializedV2<R, K, P>,
    snapshot: OrderedPlaneSpoolSnapshotV1,
    identity: [u8; 32],
    next_slot: u64,
}

/// Reopens the retained pair in its canonical order; no path, key, raw record or
/// independent snapshot authority can enter or leave this owner.
#[must_use = "the exact source and sealed pair remain owned through full replay"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct OrderedMaterializedReplayV1
<R, K, P> {
    live: Option<StoredMaterializedLiveV1<R, K, P>>,
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> OrderedMaterializedReplayV1<R, K, P> {
    /// Authenticate one exact next slot and compare every tail with its admitted
    /// original point/rho. Capacity restores this same pair/source/cursor before
    /// any leaf read; every other failure or unwind destroys them together.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn verify_next_slot_v1(
        &mut self,
    ) -> Result<bool, MaterializedStorageErrorV1> {
        let mut live = self.live.take().ok_or(MaterializedStorageErrorV1::Source)?;
        if live.next_slot >= SLOTS_V1
            || live.source.next_comparator_plane != PLANES_V1
            || live.source.ordered_writer.is_some()
            || live
                .snapshot
                .snapshot_digest_v1()
                .map_err(MaterializedStorageErrorV1::Storage)?
                != live.identity
        {
            return Err(MaterializedStorageErrorV1::Source);
        }
        let chunk = match live.snapshot.read_slot_v1(live.next_slot) {
            Ok(chunk) => chunk,
            Err(OrderedSnapshotErrorV1::Capacity) => {
                self.live = Some(live);
                return Err(MaterializedStorageErrorV1::Storage(
                    OrderedSnapshotErrorV1::Capacity,
                ));
            }
            Err(error) => return Err(MaterializedStorageErrorV1::Storage(error)),
        };
        if live.next_slot % SLOTS_PER_PLANE_V1 == 32 {
            let ordinal = u16::try_from(live.next_slot / SLOTS_PER_PLANE_V1)
                .map_err(|_| MaterializedStorageErrorV1::Source)?;
            live.source
                .evidence
                .as_ref()
                .ok_or(MaterializedStorageErrorV1::Source)?
                .validate_stored_plane_tail_v1(ordinal, chunk.as_slice_v1())
                .map_err(|_| MaterializedStorageErrorV1::Source)?;
        }
        // The canonical crypto owner erases plaintext before restoring custody.
        drop(chunk);
        live.next_slot += 1;
        let complete = live.next_slot == SLOTS_V1;
        self.live = Some(live);
        Ok(complete)
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn finish_v1(
        mut self,
    ) -> Result<VerifiedStoredMaterializedSourceV1<R, K, P>, MaterializedStorageErrorV1> {
        let live = self.live.take().ok_or(MaterializedStorageErrorV1::Source)?;
        if live.next_slot != SLOTS_V1
            || live
                .snapshot
                .snapshot_digest_v1()
                .map_err(MaterializedStorageErrorV1::Storage)?
                != live.identity
        {
            return Err(MaterializedStorageErrorV1::Source);
        }
        Ok(VerifiedStoredMaterializedSourceV1 { live })
    }
}

/// Local storage linkage only. There is deliberately no conversion to a
/// native40 source authority, proof-role witness provider, or composite seal.
/// TODO: consume this same source/pair in the genuine proof-role driver after
/// the governed native40 source and all remaining opening producers exist.
#[must_use = "retains the only original source and authenticated stored pair"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct VerifiedStoredMaterializedSourceV1
<R, K, P> {
    live: StoredMaterializedLiveV1<R, K, P>,
}

use crate::vega::zk_ams::mkhe::rns_native_u15_msm::RnsNativeU15MsmErrorV1;

/// A pre-allocation local Capacity refusal retains the whole verified source.
#[must_use = "retry only this original verified source after capacity is released"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct QMaskKernelStartRefusalV1
<R, K, P> {
    reason: RnsNativeU15MsmErrorV1,
    original: Option<VerifiedStoredMaterializedSourceV1<R, K, P>>,
}
impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> QMaskKernelStartRefusalV1<R, K, P> {
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn reason_v1(
        &self,
    ) -> RnsNativeU15MsmErrorV1 {
        self.reason
    }
    #[allow(
        clippy::result_large_err,
        reason = "capacity refusal retains the original source without allocating a replacement owner"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn retry_v1(
        mut self,
    ) -> Result<PreparedStoredQMaskKernelV1<R, K, P>, Self> {
        let Some(original) = self.original.take() else {
            return Err(self);
        };
        original.prepare_q_mask_kernel_v1()
    }
}

/// The original verified source/pair with its same-ledger immutable digit table.
/// This prepares arithmetic only: no Q-mask samples, opening tickets, native40
/// authority, production provider or composite admission is constructed.
#[must_use = "retains the sole original session, stored pair and admitted public table"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct PreparedStoredQMaskKernelV1
<R, K, P> {
    live: Option<VerifiedStoredMaterializedSourceV1<R, K, P>>,
}
impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P>
    VerifiedStoredMaterializedSourceV1<R, K, P>
{
    #[allow(
        clippy::result_large_err,
        reason = "capacity refusal retains the original source without allocating a replacement owner"
    )]
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn prepare_q_mask_kernel_v1(
        mut self,
    ) -> Result<PreparedStoredQMaskKernelV1<R, K, P>, QMaskKernelStartRefusalV1<R, K, P>> {
        let result = (|| {
            if self.live.next_slot != SLOTS_V1
                || self
                    .live
                    .snapshot
                    .snapshot_digest_v1()
                    .map_err(|_| RnsNativeU15MsmErrorV1::Source)?
                    != self.live.identity
            {
                return Err(RnsNativeU15MsmErrorV1::Source);
            }
            self.live
                .source
                .evidence
                .as_mut()
                .ok_or(RnsNativeU15MsmErrorV1::Source)?
                .begin_q_mask_kernel_v1()
        })();
        match result {
            Ok(()) => Ok(PreparedStoredQMaskKernelV1 { live: Some(self) }),
            Err(reason) => Err(QMaskKernelStartRefusalV1 {
                reason,
                original: if reason == RnsNativeU15MsmErrorV1::Capacity {
                    Some(self)
                } else {
                    None
                },
            }),
        }
    }
}

#[cfg(test)]
#[path = "ordered_storage_handoff_v1_tests.rs"]
mod tests;

#[path = "ordered_storage_handoff_v1/first_q_mask_block_v1.rs"]
mod first_q_mask_block_v1;
