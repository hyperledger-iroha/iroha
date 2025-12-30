//! Capacity declaration tracking and replication order scheduling for the embedded SoraFS node.

use std::collections::HashMap;

use iroha_data_model::{metadata::Metadata, sorafs::capacity::CapacityDeclarationRecord};
use norito::decode_from_bytes;
use sorafs_manifest::capacity::{
    CapacityDeclarationV1, ChunkerCommitmentV1, LaneCommitmentV1, ReplicationAssignmentV1,
    ReplicationOrderSlaV1, ReplicationOrderV1,
};
use thiserror::Error;

/// Manages the active capacity declaration and replication scheduling state for a provider.
#[derive(Debug, Default)]
pub struct CapacityManager {
    state: std::sync::RwLock<CapacityState>,
}

impl CapacityManager {
    /// Construct a new capacity manager with no active declaration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: std::sync::RwLock::new(CapacityState::default()),
        }
    }

    /// Record a capacity declaration captured from the registry.
    pub fn record_declaration(
        &self,
        record: &CapacityDeclarationRecord,
    ) -> Result<(), CapacityError> {
        let declaration: CapacityDeclarationV1 =
            decode_from_bytes(&record.declaration).map_err(CapacityError::DecodeDeclaration)?;
        declaration
            .validate()
            .map_err(CapacityError::ValidateDeclaration)?;

        if declaration.provider_id != record.provider_id.0 {
            return Err(CapacityError::ProviderMismatch);
        }
        if declaration.committed_capacity_gib != record.committed_capacity_gib {
            return Err(CapacityError::CommittedCapacityMismatch {
                declaration: declaration.committed_capacity_gib,
                record: record.committed_capacity_gib,
            });
        }

        let mut state = self.state.write().expect("capacity state poisoned");
        state.active = Some(ActiveCapacity::new(record, declaration));
        Ok(())
    }

    /// Produce a usage snapshot for observability and API responses.
    #[must_use]
    pub fn usage_snapshot(&self) -> CapacityUsageSnapshot {
        let state = self.state.read().expect("capacity state poisoned");
        state.snapshot()
    }

    /// Schedule the assignments from a replication order if it targets the active provider.
    pub fn schedule_order(
        &self,
        order: &ReplicationOrderV1,
    ) -> Result<Option<ReplicationPlan>, CapacityError> {
        let mut state = self.state.write().expect("capacity state poisoned");
        let Some(active) = &mut state.active else {
            return Err(CapacityError::NoActiveDeclaration);
        };

        let assignment = order
            .assignments
            .iter()
            .find(|assignment| assignment.provider_id == active.provider_id);
        let Some(assignment) = assignment else {
            return Ok(None);
        };

        active.ensure_chunker_supported(order)?;
        active.ensure_order_unique(order)?;
        active.reserve_capacity(order, assignment)?;

        Ok(Some(active.record_order(order, assignment)))
    }

    /// Release the reservation for a completed replication order.
    pub fn complete_order(&self, order_id: [u8; 32]) -> Result<ReplicationRelease, CapacityError> {
        let mut state = self.state.write().expect("capacity state poisoned");
        let active = state
            .active
            .as_mut()
            .ok_or(CapacityError::NoActiveDeclaration)?;
        active.complete_order(order_id)
    }
}

#[derive(Debug, Default)]
struct CapacityState {
    active: Option<ActiveCapacity>,
}

impl CapacityState {
    fn snapshot(&self) -> CapacityUsageSnapshot {
        match &self.active {
            Some(active) => active.snapshot(),
            None => CapacityUsageSnapshot::default(),
        }
    }
}

#[derive(Debug, Clone)]
struct ActiveCapacity {
    provider_id: [u8; 32],
    committed_total_gib: u64,
    chunkers: HashMap<String, ChunkerAllocation>,
    lanes: HashMap<String, LaneAllocation>,
    allocated_total_gib: u64,
    outstanding_orders: HashMap<[u8; 32], OrderAllocation>,
    metadata: Metadata,
    declaration_window: DeclarationWindow,
}

impl ActiveCapacity {
    fn new(record: &CapacityDeclarationRecord, declaration: CapacityDeclarationV1) -> Self {
        let chunkers = declaration
            .chunker_commitments
            .iter()
            .map(|commitment| {
                (
                    commitment.profile_id.clone(),
                    ChunkerAllocation::from_commitment(commitment),
                )
            })
            .collect::<HashMap<_, _>>();
        let lanes = declaration
            .lane_commitments
            .iter()
            .map(|lane| (lane.lane_id.clone(), LaneAllocation::from_commitment(lane)))
            .collect::<HashMap<_, _>>();

        Self {
            provider_id: declaration.provider_id,
            committed_total_gib: declaration.committed_capacity_gib,
            chunkers,
            lanes,
            allocated_total_gib: 0,
            outstanding_orders: HashMap::new(),
            metadata: record.metadata.clone(),
            declaration_window: DeclarationWindow {
                registered_epoch: record.registered_epoch,
                valid_from_epoch: record.valid_from_epoch,
                valid_until_epoch: record.valid_until_epoch,
            },
        }
    }

    fn ensure_chunker_supported(&self, order: &ReplicationOrderV1) -> Result<(), CapacityError> {
        if self.chunkers.contains_key(&order.chunking_profile) {
            return Ok(());
        }
        Err(CapacityError::UnsupportedChunker {
            handle: order.chunking_profile.clone(),
        })
    }

    fn ensure_order_unique(&self, order: &ReplicationOrderV1) -> Result<(), CapacityError> {
        if self.outstanding_orders.contains_key(&order.order_id) {
            return Err(CapacityError::OrderAlreadyScheduled {
                order_id: order.order_id,
            });
        }
        Ok(())
    }

    fn reserve_capacity(
        &mut self,
        order: &ReplicationOrderV1,
        assignment: &ReplicationAssignmentV1,
    ) -> Result<(), CapacityError> {
        let slice_gib = assignment.slice_gib;
        if slice_gib == 0 {
            return Err(CapacityError::ZeroSlice);
        }

        let available_total = self
            .committed_total_gib
            .saturating_sub(self.allocated_total_gib);
        if slice_gib > available_total {
            return Err(CapacityError::InsufficientTotalCapacity {
                requested: slice_gib,
                available: available_total,
            });
        }

        let chunker = self
            .chunkers
            .get_mut(&order.chunking_profile)
            .expect("ensure_chunker_supported must be called first");
        chunker.reserve(slice_gib)?;

        if let Some(lane_id) = assignment.lane.as_ref() {
            let lane = self
                .lanes
                .get_mut(lane_id)
                .ok_or_else(|| CapacityError::UnknownLane {
                    lane: lane_id.clone(),
                })?;
            lane.reserve(slice_gib)?;
        }

        self.allocated_total_gib = self
            .allocated_total_gib
            .checked_add(slice_gib)
            .ok_or(CapacityError::AllocationOverflow)?;

        Ok(())
    }

    fn record_order(
        &mut self,
        order: &ReplicationOrderV1,
        assignment: &ReplicationAssignmentV1,
    ) -> ReplicationPlan {
        let slice_gib = assignment.slice_gib;
        let chunker = self
            .chunkers
            .get(&order.chunking_profile)
            .expect("chunker checked during reservation");
        let lane_remaining = assignment
            .lane
            .as_ref()
            .and_then(|lane| self.lanes.get(lane).map(|entry| entry.available()));

        let plan = ReplicationPlan {
            order_id: order.order_id,
            provider_id: self.provider_id,
            manifest_cid: order.manifest_cid.clone(),
            manifest_digest: order.manifest_digest,
            chunker_handle: order.chunking_profile.clone(),
            assigned_slice_gib: slice_gib,
            remaining_total_gib: self.committed_total_gib - self.allocated_total_gib,
            remaining_chunker_gib: chunker.available(),
            lane: assignment.lane.clone(),
            remaining_lane_gib: lane_remaining,
            deadline_at: order.deadline_at,
            issued_at: order.issued_at,
            sla: order.sla,
            metadata: order.metadata.clone(),
        };

        self.outstanding_orders.insert(
            order.order_id,
            OrderAllocation {
                slice_gib,
                chunker_handle: order.chunking_profile.clone(),
                lane: assignment.lane.clone(),
                deadline_at: order.deadline_at,
            },
        );

        plan
    }

    fn complete_order(&mut self, order_id: [u8; 32]) -> Result<ReplicationRelease, CapacityError> {
        let allocation = self
            .outstanding_orders
            .remove(&order_id)
            .ok_or(CapacityError::OrderNotScheduled { order_id })?;

        self.allocated_total_gib = self
            .allocated_total_gib
            .checked_sub(allocation.slice_gib)
            .ok_or(CapacityError::AllocationUnderflow)?;

        let chunker = self
            .chunkers
            .get_mut(&allocation.chunker_handle)
            .ok_or_else(|| CapacityError::UnsupportedChunker {
                handle: allocation.chunker_handle.clone(),
            })?;
        chunker.release(allocation.slice_gib)?;
        let remaining_chunker_gib = chunker.available();

        let remaining_lane_gib = if let Some(lane_id) = allocation.lane.as_ref() {
            let lane = self
                .lanes
                .get_mut(lane_id)
                .ok_or_else(|| CapacityError::UnknownLane {
                    lane: lane_id.clone(),
                })?;
            lane.release(allocation.slice_gib)?;
            Some(lane.available())
        } else {
            None
        };

        let remaining_total_gib = self
            .committed_total_gib
            .saturating_sub(self.allocated_total_gib);

        Ok(ReplicationRelease {
            order_id,
            provider_id: self.provider_id,
            released_gib: allocation.slice_gib,
            remaining_total_gib,
            remaining_chunker_gib,
            lane: allocation.lane,
            remaining_lane_gib,
        })
    }

    fn snapshot(&self) -> CapacityUsageSnapshot {
        let chunkers = self
            .chunkers
            .iter()
            .map(|(handle, allocation)| ChunkerUsage {
                handle: handle.clone(),
                committed_gib: allocation.committed,
                allocated_gib: allocation.allocated,
                available_gib: allocation.available(),
            })
            .collect::<Vec<_>>();

        let lanes = self
            .lanes
            .iter()
            .map(|(lane, allocation)| LaneUsage {
                lane_id: lane.clone(),
                max_gib: allocation.max,
                allocated_gib: allocation.allocated,
                available_gib: allocation.available(),
            })
            .collect::<Vec<_>>();

        let outstanding_orders = self
            .outstanding_orders
            .iter()
            .map(|(order_id, allocation)| OutstandingOrder {
                order_id: *order_id,
                slice_gib: allocation.slice_gib,
                chunker_handle: allocation.chunker_handle.clone(),
                lane: allocation.lane.clone(),
                deadline_at: allocation.deadline_at,
            })
            .collect::<Vec<_>>();

        CapacityUsageSnapshot {
            provider_id: Some(self.provider_id),
            committed_total_gib: self.committed_total_gib,
            allocated_total_gib: self.allocated_total_gib,
            available_total_gib: self.committed_total_gib - self.allocated_total_gib,
            chunkers,
            lanes,
            outstanding_orders,
            metadata: self.metadata.clone(),
            declaration_window: self.declaration_window,
        }
    }
}

#[derive(Debug, Clone)]
struct ChunkerAllocation {
    committed: u64,
    allocated: u64,
}

impl ChunkerAllocation {
    fn from_commitment(commitment: &ChunkerCommitmentV1) -> Self {
        Self {
            committed: commitment.committed_gib,
            allocated: 0,
        }
    }

    fn reserve(&mut self, slice_gib: u64) -> Result<(), CapacityError> {
        let available = self.available();
        if slice_gib > available {
            return Err(CapacityError::InsufficientChunkerCapacity {
                requested: slice_gib,
                available,
            });
        }
        self.allocated = self
            .allocated
            .checked_add(slice_gib)
            .ok_or(CapacityError::AllocationOverflow)?;
        Ok(())
    }

    fn release(&mut self, slice_gib: u64) -> Result<(), CapacityError> {
        self.allocated = self
            .allocated
            .checked_sub(slice_gib)
            .ok_or(CapacityError::AllocationUnderflow)?;
        Ok(())
    }

    fn available(&self) -> u64 {
        self.committed.saturating_sub(self.allocated)
    }
}

#[derive(Debug, Clone)]
struct LaneAllocation {
    max: u64,
    allocated: u64,
}

impl LaneAllocation {
    fn from_commitment(commitment: &LaneCommitmentV1) -> Self {
        Self {
            max: commitment.max_gib,
            allocated: 0,
        }
    }

    fn reserve(&mut self, slice_gib: u64) -> Result<(), CapacityError> {
        let available = self.available();
        if slice_gib > available {
            return Err(CapacityError::InsufficientLaneCapacity {
                requested: slice_gib,
                available,
            });
        }
        self.allocated = self
            .allocated
            .checked_add(slice_gib)
            .ok_or(CapacityError::AllocationOverflow)?;
        Ok(())
    }

    fn release(&mut self, slice_gib: u64) -> Result<(), CapacityError> {
        self.allocated = self
            .allocated
            .checked_sub(slice_gib)
            .ok_or(CapacityError::AllocationUnderflow)?;
        Ok(())
    }

    fn available(&self) -> u64 {
        self.max.saturating_sub(self.allocated)
    }
}

#[derive(Debug, Clone)]
struct OrderAllocation {
    slice_gib: u64,
    chunker_handle: String,
    lane: Option<String>,
    deadline_at: u64,
}

/// Summary of the currently active capacity declaration.
#[derive(Debug, Clone, Default)]
pub struct CapacityUsageSnapshot {
    /// Active provider identifier, if a declaration has been recorded.
    pub provider_id: Option<[u8; 32]>,
    /// Total GiB committed in the active declaration.
    pub committed_total_gib: u64,
    /// GiB currently reserved for outstanding replication orders.
    pub allocated_total_gib: u64,
    /// Remaining GiB available for new assignments.
    pub available_total_gib: u64,
    /// Per-profile capacity usage.
    pub chunkers: Vec<ChunkerUsage>,
    /// Per-lane capacity usage.
    pub lanes: Vec<LaneUsage>,
    /// Outstanding replication orders currently tracked by the scheduler.
    pub outstanding_orders: Vec<OutstandingOrder>,
    /// Metadata entries persisted alongside the declaration.
    pub metadata: Metadata,
    /// Record window associated with the declaration.
    pub declaration_window: DeclarationWindow,
}

/// Usage entry for a chunker profile.
#[derive(Debug, Clone)]
pub struct ChunkerUsage {
    /// Canonical chunker handle (`namespace.name@semver`).
    pub handle: String,
    /// GiB committed for the profile.
    pub committed_gib: u64,
    /// GiB reserved for outstanding orders.
    pub allocated_gib: u64,
    /// Remaining GiB available for new assignments.
    pub available_gib: u64,
}

/// Usage entry for a capacity lane.
#[derive(Debug, Clone)]
pub struct LaneUsage {
    /// Lane identifier (e.g., `global`, `hot`).
    pub lane_id: String,
    /// Maximum GiB allocatable to the lane.
    pub max_gib: u64,
    /// GiB currently reserved.
    pub allocated_gib: u64,
    /// Remaining GiB available within the lane.
    pub available_gib: u64,
}

/// Outstanding replication order tracked by the scheduler.
#[derive(Debug, Clone)]
pub struct OutstandingOrder {
    /// Replication order identifier.
    pub order_id: [u8; 32],
    /// GiB assigned to this provider.
    pub slice_gib: u64,
    /// Chunker profile handle for the assignment.
    pub chunker_handle: String,
    /// Optional lane identifier associated with the order.
    pub lane: Option<String>,
    /// Deadline (seconds) when ingestion must be complete.
    pub deadline_at: u64,
}

/// Result of scheduling a replication order for the active provider.
#[derive(Debug, Clone)]
pub struct ReplicationPlan {
    /// Order identifier issued by governance.
    pub order_id: [u8; 32],
    /// Provider identifier targeted by the order.
    pub provider_id: [u8; 32],
    /// Manifest CID to replicate.
    pub manifest_cid: Vec<u8>,
    /// Canonical manifest digest (BLAKE3-256).
    pub manifest_digest: [u8; 32],
    /// Chunker profile to be used when ingesting the manifest.
    pub chunker_handle: String,
    /// GiB assigned to this provider.
    pub assigned_slice_gib: u64,
    /// Remaining GiB across the total commitment.
    pub remaining_total_gib: u64,
    /// Remaining GiB for the chunker profile.
    pub remaining_chunker_gib: u64,
    /// Optional lane for the assignment.
    pub lane: Option<String>,
    /// Remaining GiB within the lane, if applicable.
    pub remaining_lane_gib: Option<u64>,
    /// Deadline for completing ingestion.
    pub deadline_at: u64,
    /// Timestamp when the order was issued.
    pub issued_at: u64,
    /// SLA constraints attached to the order.
    pub sla: ReplicationOrderSlaV1,
    /// Metadata entries attached to the order.
    pub metadata: Vec<sorafs_manifest::capacity::CapacityMetadataEntry>,
}

/// Result of completing a replication order and releasing its reservation.
#[derive(Debug, Clone)]
pub struct ReplicationRelease {
    /// Order identifier issued by governance.
    pub order_id: [u8; 32],
    /// Provider identifier targeted by the order.
    pub provider_id: [u8; 32],
    /// GiB released back into the capacity pool.
    pub released_gib: u64,
    /// Remaining GiB across the total commitment.
    pub remaining_total_gib: u64,
    /// Remaining GiB for the chunker profile.
    pub remaining_chunker_gib: u64,
    /// Optional lane for the assignment.
    pub lane: Option<String>,
    /// Remaining GiB within the lane, if applicable.
    pub remaining_lane_gib: Option<u64>,
}

/// Declaration record window used to expose registry metadata.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeclarationWindow {
    /// Epoch (inclusive) when the declaration was registered.
    pub registered_epoch: u64,
    /// Epoch (inclusive) when the declaration becomes active.
    pub valid_from_epoch: u64,
    /// Epoch (inclusive) when the declaration expires.
    pub valid_until_epoch: u64,
}

/// Errors raised while managing capacity declarations or scheduling replication orders.
#[derive(Debug, Error)]
pub enum CapacityError {
    /// Failure decoding the canonical Norito payload for the declaration.
    #[error("failed to decode capacity declaration payload: {0}")]
    DecodeDeclaration(norito::core::Error),
    /// Validation error encountered while checking the declaration.
    #[error("capacity declaration validation failed: {0}")]
    ValidateDeclaration(sorafs_manifest::capacity::CapacityDeclarationValidationError),
    /// The declaration payload provider did not match the record provider.
    #[error("capacity declaration provider id mismatch between record and payload")]
    ProviderMismatch,
    /// The declaration committed capacity differs from the record summary.
    #[error(
        "capacity declaration committed GiB mismatch (declaration {declaration}, record {record})"
    )]
    CommittedCapacityMismatch {
        /// Committed GiB reported in the declaration payload.
        declaration: u64,
        /// Committed GiB recorded in the registry snapshot.
        record: u64,
    },
    /// A scheduling request was made without an active declaration.
    #[error("no active capacity declaration recorded")]
    NoActiveDeclaration,
    /// The replication order referenced an unsupported chunker profile.
    #[error("replication order chunker `{handle}` is not supported by the active declaration")]
    UnsupportedChunker {
        /// Chunker handle referenced by the replication order.
        handle: String,
    },
    /// The replication order has already been scheduled.
    #[error("replication order {order_id:02x?} has already been scheduled")]
    OrderAlreadyScheduled {
        /// Identifier of the replication order already tracked by the scheduler.
        order_id: [u8; 32],
    },
    /// The requested slice exceeds the remaining global capacity.
    #[error("insufficient total capacity: requested {requested} GiB, available {available} GiB")]
    InsufficientTotalCapacity {
        /// GiB slice requested by the replication order.
        requested: u64,
        /// Remaining GiB across the declaration’s total commitment.
        available: u64,
    },
    /// The requested slice exceeds the remaining chunker-specific capacity.
    #[error("insufficient chunker capacity: requested {requested} GiB, available {available} GiB")]
    InsufficientChunkerCapacity {
        /// GiB slice requested by the replication order.
        requested: u64,
        /// Remaining GiB available for the selected chunker profile.
        available: u64,
    },
    /// The requested slice exceeds the lane-specific capacity.
    #[error("insufficient lane capacity: requested {requested} GiB, available {available} GiB")]
    InsufficientLaneCapacity {
        /// GiB slice requested by the replication order.
        requested: u64,
        /// Remaining GiB in the referenced lane.
        available: u64,
    },
    /// The replication order referenced an unknown lane.
    #[error("replication order references unknown lane `{lane}`")]
    UnknownLane {
        /// Lane identifier present in the replication order.
        lane: String,
    },
    /// The replication order is not currently scheduled.
    #[error("replication order {order_id:02x?} is not currently scheduled")]
    OrderNotScheduled {
        /// Identifier of the replication order missing from the scheduler.
        order_id: [u8; 32],
    },
    /// A scheduling request attempted to reserve zero GiB.
    #[error("replication assignment must reserve a positive GiB slice")]
    ZeroSlice,
    /// Internal allocation tracking overflowed a 64-bit counter.
    #[error("capacity allocation overflowed internal counters")]
    AllocationOverflow,
    /// Internal allocation tracking underflowed while releasing capacity.
    #[error("capacity allocation underflowed internal counters")]
    AllocationUnderflow,
}

#[cfg(test)]
mod tests {
    use iroha_data_model::sorafs::prelude::ProviderId;
    use norito::to_bytes;
    use sorafs_manifest::capacity::{
        CAPACITY_DECLARATION_VERSION_V1, REPLICATION_ORDER_VERSION_V1, ReplicationOrderSlaV1,
    };

    use super::*;

    fn make_record_and_manager() -> (CapacityManager, CapacityDeclarationRecord) {
        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x11; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: 5_000,
            },
            committed_capacity_gib: 500,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: Some(vec!["sorafs.sf1@1.0.0".into()]),
                committed_gib: 500,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "global".into(),
                max_gib: 500,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 10,
            metadata: vec![],
        };
        let payload = to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            1,
            10,
            Metadata::default(),
        );

        (CapacityManager::new(), record)
    }

    fn make_order(slice_gib: u64) -> ReplicationOrderV1 {
        ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: [0x33; 32],
            manifest_cid: vec![0x44; 32],
            manifest_digest: [0x55; 32],
            chunking_profile: "sorafs.sf1@1.0.0".into(),
            target_replicas: 1,
            assignments: vec![ReplicationAssignmentV1 {
                provider_id: [0x11; 32],
                slice_gib,
                lane: Some("global".into()),
            }],
            issued_at: 5,
            deadline_at: 6,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 600,
                min_availability_percent_milli: 95000,
                min_por_success_percent_milli: 97000,
            },
            metadata: Vec::new(),
        }
    }

    #[test]
    fn records_capacity_declaration_and_produces_snapshot() {
        let (manager, record) = make_record_and_manager();
        manager
            .record_declaration(&record)
            .expect("record declaration");

        let snapshot = manager.usage_snapshot();
        assert_eq!(
            snapshot.provider_id.unwrap(),
            record.provider_id.as_bytes().to_owned()
        );
        assert_eq!(snapshot.committed_total_gib, 500);
        assert_eq!(snapshot.available_total_gib, 500);
        assert_eq!(snapshot.chunkers.len(), 1);
        assert_eq!(snapshot.lanes.len(), 1);
        assert!(snapshot.metadata.iter().next().is_none());
    }

    #[test]
    fn schedules_replication_order_and_updates_usage() {
        let (manager, record) = make_record_and_manager();
        manager
            .record_declaration(&record)
            .expect("record declaration");

        let order = make_order(100);
        let plan = manager
            .schedule_order(&order)
            .expect("schedule order")
            .expect("plan produced");
        assert_eq!(plan.assigned_slice_gib, 100);
        assert_eq!(plan.remaining_total_gib, 400);
        assert_eq!(plan.remaining_chunker_gib, 400);
        assert_eq!(plan.remaining_lane_gib, Some(400));

        let snapshot = manager.usage_snapshot();
        assert_eq!(snapshot.allocated_total_gib, 100);
        assert_eq!(snapshot.available_total_gib, 400);
        assert_eq!(snapshot.chunkers[0].allocated_gib, 100);
        assert_eq!(snapshot.lanes[0].allocated_gib, 100);
        assert_eq!(snapshot.outstanding_orders.len(), 1);
    }

    #[test]
    fn rejects_orders_exceeding_total_capacity() {
        let (manager, record) = make_record_and_manager();
        manager
            .record_declaration(&record)
            .expect("record declaration");

        let order = make_order(600);
        let err = manager.schedule_order(&order).unwrap_err();
        assert!(matches!(
            err,
            CapacityError::InsufficientTotalCapacity { requested: 600, .. }
        ));
    }

    #[test]
    fn rejects_orders_for_unknown_chunkers() {
        let (manager, record) = make_record_and_manager();
        manager
            .record_declaration(&record)
            .expect("record declaration");

        let mut order = make_order(100);
        order.chunking_profile = "sorafs.alt@1.0.0".into();
        let err = manager.schedule_order(&order).unwrap_err();
        assert!(matches!(
            err,
            CapacityError::UnsupportedChunker { handle } if handle == "sorafs.alt@1.0.0"
        ));
    }

    #[test]
    fn completes_replication_order_and_releases_capacity() {
        let (manager, record) = make_record_and_manager();
        manager
            .record_declaration(&record)
            .expect("record declaration");

        let order = make_order(200);
        manager
            .schedule_order(&order)
            .expect("schedule order")
            .expect("plan produced");

        let release = manager
            .complete_order(order.order_id)
            .expect("complete order");
        assert_eq!(release.released_gib, 200);
        assert_eq!(release.remaining_total_gib, 500);
        assert_eq!(release.remaining_chunker_gib, 500);
        assert_eq!(release.remaining_lane_gib, Some(500));

        let snapshot_after = manager.usage_snapshot();
        assert_eq!(snapshot_after.allocated_total_gib, 0);
        assert_eq!(snapshot_after.available_total_gib, 500);
        assert!(snapshot_after.outstanding_orders.is_empty());
    }

    #[test]
    fn completing_unknown_order_returns_error() {
        let (manager, record) = make_record_and_manager();
        manager
            .record_declaration(&record)
            .expect("record declaration");

        let err = manager
            .complete_order([0xAB; 32])
            .expect_err("completion should fail");
        assert!(matches!(
            err,
            CapacityError::OrderNotScheduled { order_id } if order_id == [0xAB; 32]
        ));
    }
}
