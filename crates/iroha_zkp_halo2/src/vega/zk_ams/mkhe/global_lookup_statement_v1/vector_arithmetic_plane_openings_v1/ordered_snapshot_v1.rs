//! One move-only local storage owner for two ordered global-plane spools.
//!
//! The parent registers this local storage owner and binds its canonical plan.
//! Storage possession and slot syntax do not verify commitments or supply the missing
//! authenticated materializer, replay permits, proof, or release authority.
//! Caller-supplied chunks move here and are zeroized by the canonical crypto
//! owner on success/error/unwind. Successful reads return that zeroizing owner;
//! callers own its subsequent use and drop. No plaintext copies or secrets are
//! retained in this module beyond the leaf handles and one in-flight chunk.
//! A shared session ledger reserves both file lengths plus their write/seal I/O
//! before file creation and retains that reservation through snapshot reads.
//! Cancellation closes the detached files before releasing live-file credits;
//! attempted authenticated record I/O is never refunded. All source/qPCS owners
//! still need to share the same ledger in the future production integration.
//! This does not extend the leaf's erasure, page-cache, fork, or RSS guarantees.

use std::path::Path;

use iroha_crypto::confidential_spool::{
    CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1, ConfidentialSpoolChunkV1, ConfidentialSpoolErrorV1,
    ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1, ConfidentialSpoolWriterV1,
};

use crate::vega::{
    VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
    bulletproof_t256::ZeroizingT256ScalarCopyV1, sponge::Keccak256,
};

use super::{
    PLANE_COUNT_V1, SNAPSHOT_SLOT_COUNT_V1, SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1,
    SNAPSHOT_SLOT_TAG_BYTES_V1, SNAPSHOT_SLOTS_PER_PLANE_V1, VALUE_SLOTS_PER_PLANE_V1,
    plane_mapping_digest_v1,
};

#[path = "ordered_snapshot_v1/resource_budget_v1.rs"]
mod resource_budget_v1;
pub(in crate::vega::zk_ams::mkhe) use resource_budget_v1::OrderedStorageSessionBudgetV1;
use resource_budget_v1::{OrderedStorageReservationV1, StorageBudgetErrorV1};

const STORAGE_VERSION_V1: u64 = 1;
const SEGMENTS_V1: usize = 2;
const FIRST_PLANES_V1: u64 = 7_075;
const SECOND_PLANES_V1: u64 = 2_213;
const PLAN_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.global-plane.ordered-two-spool.plan\0";
const SEGMENT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.global-plane.ordered-two-spool.segment\0";
const SNAPSHOT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.global-plane.ordered-two-spool.snapshot\0";

const _: () = {
    assert!(FIRST_PLANES_V1 + SECOND_PLANES_V1 == PLANE_COUNT_V1 as u64);
    assert!(SNAPSHOT_SLOT_COUNT_V1 == (FIRST_PLANES_V1 + SECOND_PLANES_V1) * 33);
    assert!(SNAPSHOT_SLOTS_PER_PLANE_V1 == 33);
    assert!(VALUE_SLOTS_PER_PLANE_V1 == 32);
    assert!(SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1 == 16_384);
    assert!(SNAPSHOT_SLOT_TAG_BYTES_V1 == 16);
    assert!(FIRST_PLANES_V1 * 33 * 16_400 == 3_828_990_000);
    assert!(SECOND_PLANES_V1 * 33 * 16_400 == 1_197_675_600);
    assert!(FIRST_PLANES_V1 * 33 * 16_400 <= CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1);
    assert!((FIRST_PLANES_V1 + 1) * 33 * 16_400 > CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1);
    assert!(SECOND_PLANES_V1 * 33 * 16_400 <= CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1);
};

/// Coarse storage/shape rejection without paths, secret values, or leaf errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::vega::zk_ams::mkhe) enum OrderedSnapshotErrorV1 {
    Shape,
    Context,
    Order,
    Resource,
    Capacity,
    Storage,
    Semantics,
    Poisoned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeometryV1 {
    Canonical,
    #[cfg(test)]
    Tiny,
}
impl GeometryV1 {
    const fn plane_counts(self) -> [u64; SEGMENTS_V1] {
        match self {
            Self::Canonical => [FIRST_PLANES_V1, SECOND_PLANES_V1],
            #[cfg(test)]
            Self::Tiny => [1, 1],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SegmentV1 {
    ordinal: u64,
    first_plane: u64,
    planes: u64,
    first_slot: u64,
    slots: u64,
    file_bytes: u64,
}
impl SegmentV1 {
    fn new_v1(ordinal: u64, first_plane: u64, planes: u64) -> Result<Self, OrderedSnapshotErrorV1> {
        if ordinal >= SEGMENTS_V1 as u64 || planes == 0 {
            return Err(OrderedSnapshotErrorV1::Shape);
        }
        let first_slot = first_plane
            .checked_mul(SNAPSHOT_SLOTS_PER_PLANE_V1)
            .ok_or(OrderedSnapshotErrorV1::Resource)?;
        let slots = planes
            .checked_mul(SNAPSHOT_SLOTS_PER_PLANE_V1)
            .ok_or(OrderedSnapshotErrorV1::Resource)?;
        first_slot
            .checked_add(slots)
            .ok_or(OrderedSnapshotErrorV1::Resource)?;
        let record_bytes = SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1
            .checked_add(SNAPSHOT_SLOT_TAG_BYTES_V1)
            .ok_or(OrderedSnapshotErrorV1::Resource)?;
        let file_bytes = slots
            .checked_mul(record_bytes)
            .ok_or(OrderedSnapshotErrorV1::Resource)?;
        Ok(Self {
            ordinal,
            first_plane,
            planes,
            first_slot,
            slots,
            file_bytes,
        })
    }
    fn words_v1(self) -> [u64; 6] {
        [
            self.ordinal,
            self.first_plane,
            self.planes,
            self.first_slot,
            self.slots,
            self.file_bytes,
        ]
    }
}

/// Complete checked public layout plan; individual live spools never escape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OrderedPlaneSpoolPlanV1 {
    geometry: GeometryV1,
    plane_context: [u8; 32],
    mapping: [u8; 32],
    total_planes: u64,
    total_slots: u64,
    segments: [SegmentV1; SEGMENTS_V1],
    contexts: [[u8; 32]; SEGMENTS_V1],
    layouts: [ConfidentialSpoolLayoutV1; SEGMENTS_V1],
    digest: [u8; 32],
}
impl OrderedPlaneSpoolPlanV1 {
    /// Check the sole full geometry before any I/O, entropy, or private buffer.
    pub(super) fn canonical_v1(plane_context: [u8; 32]) -> Result<Self, OrderedSnapshotErrorV1> {
        let mapping = plane_mapping_digest_v1().map_err(|_| OrderedSnapshotErrorV1::Context)?;
        Self::build_v1(GeometryV1::Canonical, plane_context, mapping)
    }
    fn build_v1(
        geometry: GeometryV1,
        plane_context: [u8; 32],
        mapping: [u8; 32],
    ) -> Result<Self, OrderedSnapshotErrorV1> {
        if plane_context == [0; 32] || mapping == [0; 32] {
            return Err(OrderedSnapshotErrorV1::Context);
        }
        let counts = geometry.plane_counts();
        let total_planes = counts[0]
            .checked_add(counts[1])
            .ok_or(OrderedSnapshotErrorV1::Resource)?;
        let total_slots = total_planes
            .checked_mul(SNAPSHOT_SLOTS_PER_PLANE_V1)
            .ok_or(OrderedSnapshotErrorV1::Resource)?;
        let segments = [
            SegmentV1::new_v1(0, 0, counts[0])?,
            SegmentV1::new_v1(1, counts[0], counts[1])?,
        ];
        let mut contexts = [[0; 32]; SEGMENTS_V1];
        for (index, segment) in segments.iter().enumerate() {
            let mut hash = context_prefix_v1(
                SEGMENT_DOMAIN_V1,
                plane_context,
                mapping,
                total_planes,
                total_slots,
            );
            for value in segment.words_v1() {
                hash.update(&value.to_be_bytes());
            }
            contexts[index] = nonzero_v1(hash.finalize())?;
        }
        if contexts[0] == contexts[1] {
            return Err(OrderedSnapshotErrorV1::Context);
        }
        // Both constructors complete before any call to create_in_v1.
        let layouts = [
            ConfidentialSpoolLayoutV1::new_v1(
                segments[0].slots,
                SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1,
                contexts[0],
            )
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?,
            ConfidentialSpoolLayoutV1::new_v1(
                segments[1].slots,
                SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1,
                contexts[1],
            )
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?,
        ];
        for (segment, layout) in segments.iter().zip(layouts.iter()) {
            if layout.slot_count_v1() != segment.slots
                || layout.plaintext_len_v1() != SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1
                || layout.file_len_v1() != segment.file_bytes
            {
                return Err(OrderedSnapshotErrorV1::Shape);
            }
        }
        let mut hash = context_prefix_v1(
            PLAN_DOMAIN_V1,
            plane_context,
            mapping,
            total_planes,
            total_slots,
        );
        for index in 0..SEGMENTS_V1 {
            for value in segments[index].words_v1() {
                hash.update(&value.to_be_bytes());
            }
            hash.update(&contexts[index]);
        }
        let digest = nonzero_v1(hash.finalize())?;
        Ok(Self {
            geometry,
            plane_context,
            mapping,
            total_planes,
            total_slots,
            segments,
            contexts,
            layouts,
            digest,
        })
    }
    fn validate_v1(&self) -> Result<(), OrderedSnapshotErrorV1> {
        if *self != Self::build_v1(self.geometry, self.plane_context, self.mapping)? {
            return Err(OrderedSnapshotErrorV1::Shape);
        }
        Ok(())
    }
    fn route_v1(&self, global_slot: u64) -> Result<(usize, u64), OrderedSnapshotErrorV1> {
        if global_slot >= self.total_slots {
            return Err(OrderedSnapshotErrorV1::Shape);
        }
        let index = usize::from(global_slot >= self.segments[1].first_slot);
        let segment = self.segments[index];
        let local = global_slot
            .checked_sub(segment.first_slot)
            .ok_or(OrderedSnapshotErrorV1::Shape)?;
        if local >= segment.slots || segment.first_slot.checked_add(local) != Some(global_slot) {
            return Err(OrderedSnapshotErrorV1::Shape);
        }
        Ok((index, local))
    }
    /// Total logical slot count of the full retained plane snapshot.
    pub(super) const fn slot_count_v1(&self) -> u64 {
        self.total_slots
    }
    /// Complete checked public storage mapping digest, not proof authority.
    pub(super) const fn descriptor_digest_v1(&self) -> [u8; 32] {
        self.digest
    }
}

fn context_prefix_v1(
    domain: &[u8],
    context: [u8; 32],
    mapping: [u8; 32],
    planes: u64,
    slots: u64,
) -> Keccak256 {
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&context);
    hash.update(&mapping);
    for value in [
        STORAGE_VERSION_V1,
        SEGMENTS_V1 as u64,
        planes,
        slots,
        SNAPSHOT_SLOTS_PER_PLANE_V1,
        SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1,
        SNAPSHOT_SLOT_TAG_BYTES_V1,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash
}
fn nonzero_v1(digest: [u8; 32]) -> Result<[u8; 32], OrderedSnapshotErrorV1> {
    if digest == [0; 32] {
        Err(OrderedSnapshotErrorV1::Context)
    } else {
        Ok(digest)
    }
}

// Canonical scalar parsing borrows the owned plaintext and wipes the scalar
// owner immediately. No validation path returns a copied secret scalar.
fn validate_slot_v1(global_slot: u64, bytes: &[u8]) -> Result<(), OrderedSnapshotErrorV1> {
    if bytes.len() != SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1 as usize {
        return Err(OrderedSnapshotErrorV1::Shape);
    }
    if global_slot % SNAPSHOT_SLOTS_PER_PLANE_V1 < VALUE_SLOTS_PER_PLANE_V1 {
        for encoded in bytes.chunks_exact(32) {
            let scalar = ZeroizingT256ScalarCopyV1::new(
                Scalar::from_be_bytes_exact_ref(
                    encoded
                        .try_into()
                        .map_err(|_| OrderedSnapshotErrorV1::Shape)?,
                )
                .map_err(|_| OrderedSnapshotErrorV1::Semantics)?,
            );
            drop(scalar);
        }
    } else {
        let blinding = ZeroizingT256ScalarCopyV1::new(
            Scalar::from_be_bytes_exact_ref(
                bytes[..32]
                    .try_into()
                    .map_err(|_| OrderedSnapshotErrorV1::Shape)?,
            )
            .map_err(|_| OrderedSnapshotErrorV1::Semantics)?,
        );
        if blinding.as_ref() == &Scalar::zero() {
            return Err(OrderedSnapshotErrorV1::Semantics);
        }
        Point::from_non_identity_wire_bytes_exact(&bytes[32..65])
            .map_err(|_| OrderedSnapshotErrorV1::Semantics)?;
        if bytes[65..].iter().any(|byte| *byte != 0) {
            return Err(OrderedSnapshotErrorV1::Semantics);
        }
    }
    Ok(())
}

// Field order is deliberate: detached leaf files/keys drop before their spool
// reservation. The crypto owner unlinks each file while empty before sizing;
// no path/reopen/descriptor escape is available from this pair. This is not a
// fork, kernel-cache or physical secure-deletion guarantee.
struct ReservedSpoolPairV1<T> {
    spools: [T; SEGMENTS_V1],
    reservation: OrderedStorageReservationV1,
}

/// Move-only writer for the complete logical snapshot, never one segment.
#[must_use = "dropping the pair closes both encrypted spools"]
pub(in crate::vega::zk_ams::mkhe) struct OrderedPlaneSpoolWriterV1 {
    live: Option<ReservedSpoolPairV1<ConfidentialSpoolWriterV1>>,
    plan: OrderedPlaneSpoolPlanV1,
    next_slot: u64,
}
impl OrderedPlaneSpoolWriterV1 {
    /// Construct both canonical files only after validating the full plan.
    pub(in crate::vega::zk_ams::mkhe) fn create_v1(
        directory: &Path,
        plane_context: [u8; 32],
        budget: &mut OrderedStorageSessionBudgetV1,
    ) -> Result<Self, OrderedSnapshotErrorV1> {
        Self::create_with_plan_v1(
            directory,
            OrderedPlaneSpoolPlanV1::canonical_v1(plane_context)?,
            budget,
        )
    }
    #[cfg(test)]
    pub(in crate::vega::zk_ams::mkhe) fn create_tiny_for_test_v1(
        directory: &Path,
        context: [u8; 32],
        budget: &mut OrderedStorageSessionBudgetV1,
    ) -> Result<Self, OrderedSnapshotErrorV1> {
        let mapping = plane_mapping_digest_v1().map_err(|_| OrderedSnapshotErrorV1::Context)?;
        Self::create_with_plan_v1(
            directory,
            OrderedPlaneSpoolPlanV1::build_v1(GeometryV1::Tiny, context, mapping)?,
            budget,
        )
    }

    fn create_with_plan_v1(
        directory: &Path,
        plan: OrderedPlaneSpoolPlanV1,
        budget: &mut OrderedStorageSessionBudgetV1,
    ) -> Result<Self, OrderedSnapshotErrorV1> {
        Self::create_with_plan_and_leaf_factory_v1(directory, plan, budget, |path, layout| {
            ConfidentialSpoolWriterV1::create_in_v1(path, layout)
        })
    }
    // One private construction path keeps admission ahead of both real crypto
    // calls. Tests may fail/panic on the second factory call; no alternate leaf
    // representation, source authority or public callback is accepted.
    fn create_with_plan_and_leaf_factory_v1(
        directory: &Path,
        plan: OrderedPlaneSpoolPlanV1,
        budget: &mut OrderedStorageSessionBudgetV1,
        mut create_leaf: impl FnMut(
            &Path,
            ConfidentialSpoolLayoutV1,
        )
            -> Result<ConfidentialSpoolWriterV1, ConfidentialSpoolErrorV1>,
    ) -> Result<Self, OrderedSnapshotErrorV1> {
        plan.validate_v1()?;
        let spool_bytes = plan.segments[0]
            .file_bytes
            .checked_add(plan.segments[1].file_bytes)
            .ok_or(OrderedSnapshotErrorV1::Resource)?;
        // Keep this binding before both leaf bindings: error/unwind destroys
        // every already-created file before returning its reservation credits.
        let reservation = budget
            .reserve_files_v1(spool_bytes)
            .map_err(|error| match error {
                StorageBudgetErrorV1::SpoolLimit | StorageBudgetErrorV1::IoLimit => {
                    OrderedSnapshotErrorV1::Capacity
                }
                _ => OrderedSnapshotErrorV1::Resource,
            })?;
        let first =
            create_leaf(directory, plan.layouts[0]).map_err(|_| OrderedSnapshotErrorV1::Storage)?;
        let second =
            create_leaf(directory, plan.layouts[1]).map_err(|_| OrderedSnapshotErrorV1::Storage)?;
        Ok(Self {
            live: Some(ReservedSpoolPairV1 {
                spools: [first, second],
                reservation,
            }),
            plan,
            next_slot: 0,
        })
    }
    /// Require the context derived from the retained original source records.
    pub(in crate::vega::zk_ams::mkhe) fn require_context_v1(
        &self,
        context: [u8; 32],
    ) -> Result<(), OrderedSnapshotErrorV1> {
        self.live.as_ref().ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        self.plan.validate_v1()?;
        if self.plan.plane_context != context {
            return Err(OrderedSnapshotErrorV1::Context);
        }
        Ok(())
    }

    /// Inspect the existing consuming cursor before extracting any source chunk.
    pub(in crate::vega::zk_ams::mkhe) fn require_next_slot_v1(
        &self,
        expected: u64,
    ) -> Result<(), OrderedSnapshotErrorV1> {
        self.live.as_ref().ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        self.plan.validate_v1()?;
        if self.next_slot != expected || expected > self.plan.total_slots {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        Ok(())
    }

    /// Consume one exact next slot; any failure discards both retained files.
    pub(in crate::vega::zk_ams::mkhe) fn write_slot_v1(
        &mut self,
        global_slot: u64,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), OrderedSnapshotErrorV1> {
        let mut live = self.live.take().ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        if global_slot != self.next_slot {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        let (segment, local) = self.plan.route_v1(global_slot)?;
        validate_slot_v1(global_slot, chunk.as_slice_v1())?;
        // A record writes exactly plaintext bytes plus its authentication tag.
        // Charge the full request even if a leaf fails after a partial write.
        live.reservation
            .charge_reserved_io_v1(SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1 + SNAPSHOT_SLOT_TAG_BYTES_V1)
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?;
        live.spools[segment]
            .write_slot_v1(local, chunk)
            .map_err(|_| OrderedSnapshotErrorV1::Storage)?;
        self.next_slot = self
            .next_slot
            .checked_add(1)
            .ok_or(OrderedSnapshotErrorV1::Resource)?;
        self.live = Some(live);
        Ok(())
    }
    /// Authenticate and consume both complete files; partial success cannot escape.
    pub(in crate::vega::zk_ams::mkhe) fn seal_v1(
        self,
    ) -> Result<OrderedPlaneSpoolSnapshotV1, OrderedSnapshotErrorV1> {
        let live = self.live.ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        if self.next_slot != self.plan.total_slots {
            return Err(OrderedSnapshotErrorV1::Order);
        }
        // Declare the reservation before extracted leaves so every early return
        // closes both remaining leaf/snapshot handles before releasing credits.
        let mut reservation = live.reservation;
        let [first, second] = live.spools;
        // Each leaf seal authenticates/hash-reads its complete ciphertext file.
        reservation
            .charge_reserved_io_v1(self.plan.segments[0].file_bytes)
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?;
        let first = first
            .seal_v1()
            .map_err(|_| OrderedSnapshotErrorV1::Storage)?;
        reservation
            .charge_reserved_io_v1(self.plan.segments[1].file_bytes)
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?;
        let second = second
            .seal_v1()
            .map_err(|_| OrderedSnapshotErrorV1::Storage)?;
        reservation
            .require_sealed_v1()
            .map_err(|_| OrderedSnapshotErrorV1::Resource)?;
        let leaf_digests = [*first.snapshot_digest_v1(), *second.snapshot_digest_v1()];
        let digest = aggregate_digest_v1(&self.plan, leaf_digests)?;
        let snapshot = OrderedPlaneSpoolSnapshotV1 {
            live: Some(ReservedSpoolPairV1 {
                spools: [first, second],
                reservation,
            }),
            plan: self.plan,
            leaf_digests,
            digest,
        };
        snapshot.validate_live_v1()?;
        Ok(snapshot)
    }
}

fn aggregate_digest_v1(
    plan: &OrderedPlaneSpoolPlanV1,
    leaf_digests: [[u8; 32]; SEGMENTS_V1],
) -> Result<[u8; 32], OrderedSnapshotErrorV1> {
    if leaf_digests[0] == [0; 32]
        || leaf_digests[1] == [0; 32]
        || leaf_digests[0] == leaf_digests[1]
    {
        return Err(OrderedSnapshotErrorV1::Context);
    }
    let mut hash = context_prefix_v1(
        SNAPSHOT_DOMAIN_V1,
        plan.plane_context,
        plan.mapping,
        plan.total_planes,
        plan.total_slots,
    );
    hash.update(&plan.digest);
    for index in 0..SEGMENTS_V1 {
        for value in plan.segments[index].words_v1() {
            hash.update(&value.to_be_bytes());
        }
        hash.update(&plan.contexts[index]);
        hash.update(&leaf_digests[index]);
    }
    nonzero_v1(hash.finalize())
}

/// One retained authenticated pair; its digest alone cannot authorize reads.
#[must_use = "dropping the snapshot closes both encrypted files"]
pub(in crate::vega::zk_ams::mkhe) struct OrderedPlaneSpoolSnapshotV1 {
    live: Option<ReservedSpoolPairV1<ConfidentialSpoolSnapshotV1>>,
    plan: OrderedPlaneSpoolPlanV1,
    leaf_digests: [[u8; 32]; SEGMENTS_V1],
    digest: [u8; 32],
}
impl OrderedPlaneSpoolSnapshotV1 {
    fn validate_live_v1(&self) -> Result<(), OrderedSnapshotErrorV1> {
        let live = self.live.as_ref().ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        self.validate_pair_v1(&live.spools)
    }
    fn validate_pair_v1(
        &self,
        live: &[ConfidentialSpoolSnapshotV1; SEGMENTS_V1],
    ) -> Result<(), OrderedSnapshotErrorV1> {
        self.plan.validate_v1()?;
        if aggregate_digest_v1(&self.plan, self.leaf_digests)? != self.digest {
            return Err(OrderedSnapshotErrorV1::Context);
        }
        for index in 0..SEGMENTS_V1 {
            let segment = self.plan.segments[index];
            if live[index].slot_count_v1() != segment.slots
                || live[index].plaintext_len_v1() != SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1
                || live[index].ciphertext_record_len_v1()
                    != SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1 + SNAPSHOT_SLOT_TAG_BYTES_V1
                || live[index].file_len_v1() != segment.file_bytes
                || *live[index].snapshot_digest_v1() != self.leaf_digests[index]
            {
                return Err(OrderedSnapshotErrorV1::Context);
            }
        }
        Ok(())
    }
    /// Return the actual ordered encrypted snapshot identity, never a proof receipt.
    pub(in crate::vega::zk_ams::mkhe) fn snapshot_digest_v1(
        &self,
    ) -> Result<[u8; 32], OrderedSnapshotErrorV1> {
        self.validate_live_v1()?;
        Ok(self.digest)
    }
    /// Authenticate and validate a global slot while retaining one logical owner.
    pub(in crate::vega::zk_ams::mkhe) fn read_slot_v1(
        &mut self,
        global_slot: u64,
    ) -> Result<ConfidentialSpoolChunkV1, OrderedSnapshotErrorV1> {
        let mut live = self.live.take().ok_or(OrderedSnapshotErrorV1::Poisoned)?;
        self.validate_pair_v1(&live.spools)?;
        let (segment, local) = self.plan.route_v1(global_slot)?;
        // Bounds/context preflight is complete before admitting an actual read.
        // Admitted retries are new charges; failed/partial I/O is never refunded.
        match live
            .reservation
            .charge_read_io_v1(SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1 + SNAPSHOT_SLOT_TAG_BYTES_V1)
        {
            Ok(()) => {}
            Err(StorageBudgetErrorV1::IoLimit) => {
                // Another writer may still own unspent I/O reservations. Local
                // capacity pressure is not corrupted evidence: retain both exact
                // files and their identity, without performing or charging a read.
                self.live = Some(live);
                return Err(OrderedSnapshotErrorV1::Capacity);
            }
            Err(_) => return Err(OrderedSnapshotErrorV1::Resource),
        }
        let chunk = live.spools[segment]
            .read_slot_v1(local, self.plan.contexts[segment])
            .map_err(|_| OrderedSnapshotErrorV1::Storage)?;
        validate_slot_v1(global_slot, chunk.as_slice_v1())?;
        self.live = Some(live);
        Ok(chunk)
    }
}

#[cfg(test)]
#[path = "ordered_snapshot_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ordered_snapshot_v1/resource_budget_storage_tests_v1.rs"]
mod resource_budget_storage_tests_v1;

#[path = "ordered_snapshot_v1/q_mask_s_file_v1.rs"]
mod q_mask_s_file_v1;
pub(in crate::vega::zk_ams::mkhe) use q_mask_s_file_v1::{
    QMaskSFileMemoryV1, QMaskSFilePlanV1, QMaskSFileV1, SealedQMaskSFileV1,
    WrittenQMaskSBlockFileV1,
};
