//! Capacity declaration tracking and replication order scheduling for the embedded SoraFS node.

use std::collections::HashMap;

use iroha_data_model::{
    metadata::Metadata,
    sorafs::capacity::{CapacityDeclarationRecord, ProviderId},
};
use norito::{
    decode_from_bytes,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use sorafs_manifest::capacity::{
    CapacityDeclarationV1, ChunkerCommitmentV1, LaneCommitmentV1, ReplicationAssignmentV1,
    ReplicationOrderSlaV1, ReplicationOrderV1,
};
use thiserror::Error;

/// Manages the active capacity declaration and replication scheduling state for a provider.
#[derive(Debug)]
pub struct CapacityManager {
    state: std::sync::RwLock<CapacityState>,
    entry_limit: usize,
}

impl Default for CapacityManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CapacityManager {
    /// Construct a new capacity manager with no active declaration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_entry_limit(65_536)
    }

    /// Construct a capacity manager with a ceiling for declaration indexes and outstanding
    /// replication orders.
    #[must_use]
    pub fn with_entry_limit(entry_limit: usize) -> Self {
        Self {
            state: std::sync::RwLock::new(CapacityState::default()),
            entry_limit: entry_limit.max(1),
        }
    }

    /// Record a capacity declaration captured from the registry.
    pub(crate) fn record_declaration(
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
        for (resource, count) in [
            ("chunker_commitments", declaration.chunker_commitments.len()),
            ("lane_commitments", declaration.lane_commitments.len()),
        ] {
            if count > self.entry_limit {
                return Err(CapacityError::ResourceExhausted {
                    resource,
                    limit: self.entry_limit,
                });
            }
        }

        let mut state = self
            .state
            .write()
            .map_err(|_| CapacityError::StateLockPoisoned)?;
        if let Some(active) = &state.active
            && !active.outstanding_orders.is_empty()
        {
            return Err(
                CapacityError::DeclarationReplacementWhileOrdersOutstanding {
                    count: active.outstanding_orders.len(),
                },
            );
        }
        state.active = Some(ActiveCapacity::new(record, declaration));
        Ok(())
    }

    /// Produce a usage snapshot for observability and API responses.
    #[must_use]
    pub fn usage_snapshot(&self) -> CapacityUsageSnapshot {
        let state = self.state.read().expect("capacity state poisoned");
        state.snapshot()
    }

    /// Reconstruct the active registry record for restart-time telemetry seeding.
    pub(crate) fn active_declaration_record(
        &self,
    ) -> Result<Option<CapacityDeclarationRecord>, CapacityError> {
        let state = self
            .state
            .read()
            .map_err(|_| CapacityError::StateLockPoisoned)?;
        Ok(state.active.as_ref().map(|active| {
            CapacityDeclarationRecord::new(
                ProviderId::new(active.provider_id),
                active.declaration_payload.clone(),
                active.committed_total_gib,
                active.declaration_window.registered_epoch,
                active.declaration_window.valid_from_epoch,
                active.declaration_window.valid_until_epoch,
                active.metadata.clone(),
            )
        }))
    }

    /// Schedule the assignments from a replication order if it targets the active provider.
    pub(crate) fn schedule_order(
        &self,
        order: &ReplicationOrderV1,
    ) -> Result<Option<ReplicationPlan>, CapacityError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| CapacityError::StateLockPoisoned)?;
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
        if active.outstanding_orders.len() >= self.entry_limit {
            return Err(CapacityError::ResourceExhausted {
                resource: "outstanding_orders",
                limit: self.entry_limit,
            });
        }
        let mut candidate = active.clone();
        candidate.reserve_capacity(order, assignment)?;
        let plan = candidate.record_order(order, assignment);
        *active = candidate;

        Ok(Some(plan))
    }

    /// Release the reservation for a completed replication order.
    pub(crate) fn complete_order(
        &self,
        order_id: [u8; 32],
    ) -> Result<ReplicationRelease, CapacityError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| CapacityError::StateLockPoisoned)?;
        let active = state
            .active
            .as_mut()
            .ok_or(CapacityError::NoActiveDeclaration)?;
        let mut candidate = active.clone();
        let release = candidate.complete_order(order_id)?;
        *active = candidate;
        Ok(release)
    }

    /// Export the active declaration and every outstanding capacity reservation.
    pub(crate) fn checkpoint(&self) -> Result<CapacityRuntimeCheckpointV1, CapacityError> {
        let state = self
            .state
            .read()
            .map_err(|_| CapacityError::StateLockPoisoned)?;
        Ok(CapacityRuntimeCheckpointV1 {
            active: state.active.as_ref().map(ActiveCapacity::checkpoint),
        })
    }

    /// Restore an authoritative capacity checkpoint after recomputing all allocation totals.
    pub(crate) fn restore_checkpoint(
        &self,
        checkpoint: CapacityRuntimeCheckpointV1,
    ) -> Result<(), CapacityError> {
        let active = checkpoint
            .active
            .map(|active| ActiveCapacity::restore_checkpoint(active, self.entry_limit))
            .transpose()?;
        let mut state = self
            .state
            .write()
            .map_err(|_| CapacityError::StateLockPoisoned)?;
        state.active = active;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct CapacityState {
    active: Option<ActiveCapacity>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct CapacityChunkerCheckpointV1 {
    handle: String,
    committed: u64,
    allocated: u64,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct CapacityLaneCheckpointV1 {
    lane_id: String,
    max: u64,
    allocated: u64,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct CapacityOrderCheckpointV1 {
    order_id: [u8; 32],
    slice_gib: u64,
    chunker_handle: String,
    lane: Option<String>,
    issued_at: u64,
    deadline_at: u64,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct ActiveCapacityCheckpointV1 {
    declaration_payload: Vec<u8>,
    provider_id: [u8; 32],
    committed_total_gib: u64,
    chunkers: Vec<CapacityChunkerCheckpointV1>,
    lanes: Vec<CapacityLaneCheckpointV1>,
    allocated_total_gib: u64,
    outstanding_orders: Vec<CapacityOrderCheckpointV1>,
    metadata: Metadata,
    declaration_window: DeclarationWindow,
}

/// Canonical restart snapshot for capacity declarations and outstanding reservations.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct CapacityRuntimeCheckpointV1 {
    active: Option<ActiveCapacityCheckpointV1>,
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
    declaration_payload: Vec<u8>,
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
            declaration_payload: record.declaration.clone(),
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

    fn checkpoint(&self) -> ActiveCapacityCheckpointV1 {
        let mut chunkers = self
            .chunkers
            .iter()
            .map(|(handle, allocation)| CapacityChunkerCheckpointV1 {
                handle: handle.clone(),
                committed: allocation.committed,
                allocated: allocation.allocated,
            })
            .collect::<Vec<_>>();
        chunkers.sort_by(|left, right| left.handle.cmp(&right.handle));
        let mut lanes = self
            .lanes
            .iter()
            .map(|(lane_id, allocation)| CapacityLaneCheckpointV1 {
                lane_id: lane_id.clone(),
                max: allocation.max,
                allocated: allocation.allocated,
            })
            .collect::<Vec<_>>();
        lanes.sort_by(|left, right| left.lane_id.cmp(&right.lane_id));
        let mut outstanding_orders = self
            .outstanding_orders
            .iter()
            .map(|(order_id, allocation)| CapacityOrderCheckpointV1 {
                order_id: *order_id,
                slice_gib: allocation.slice_gib,
                chunker_handle: allocation.chunker_handle.clone(),
                lane: allocation.lane.clone(),
                issued_at: allocation.issued_at,
                deadline_at: allocation.deadline_at,
            })
            .collect::<Vec<_>>();
        outstanding_orders.sort_by_key(|order| order.order_id);
        ActiveCapacityCheckpointV1 {
            declaration_payload: self.declaration_payload.clone(),
            provider_id: self.provider_id,
            committed_total_gib: self.committed_total_gib,
            chunkers,
            lanes,
            allocated_total_gib: self.allocated_total_gib,
            outstanding_orders,
            metadata: self.metadata.clone(),
            declaration_window: self.declaration_window,
        }
    }

    fn restore_checkpoint(
        checkpoint: ActiveCapacityCheckpointV1,
        entry_limit: usize,
    ) -> Result<Self, CapacityError> {
        for (resource, count) in [
            ("chunker_commitments", checkpoint.chunkers.len()),
            ("lane_commitments", checkpoint.lanes.len()),
            ("outstanding_orders", checkpoint.outstanding_orders.len()),
        ] {
            if count > entry_limit {
                return Err(CapacityError::InvalidCheckpoint(format!(
                    "{resource} count {count} exceeds configured limit {entry_limit}"
                )));
            }
        }
        if checkpoint.committed_total_gib == 0
            || checkpoint.declaration_window.valid_from_epoch
                > checkpoint.declaration_window.valid_until_epoch
        {
            return Err(CapacityError::InvalidCheckpoint(
                "capacity declaration totals or validity window are invalid".to_owned(),
            ));
        }
        let declaration: CapacityDeclarationV1 = decode_from_bytes(&checkpoint.declaration_payload)
            .map_err(|err| {
                CapacityError::InvalidCheckpoint(format!(
                    "capacity declaration payload cannot be decoded: {err}"
                ))
            })?;
        declaration.validate().map_err(|err| {
            CapacityError::InvalidCheckpoint(format!(
                "capacity declaration payload is invalid: {err}"
            ))
        })?;
        if declaration.provider_id != checkpoint.provider_id
            || declaration.committed_capacity_gib != checkpoint.committed_total_gib
            || declaration.valid_from != checkpoint.declaration_window.valid_from_epoch
            || declaration.valid_until != checkpoint.declaration_window.valid_until_epoch
        {
            return Err(CapacityError::InvalidCheckpoint(
                "capacity declaration payload disagrees with checkpoint summary".to_owned(),
            ));
        }

        let mut chunkers = HashMap::with_capacity(checkpoint.chunkers.len());
        let mut previous_chunker: Option<&str> = None;
        let mut committed_by_chunkers = 0u64;
        for chunker in &checkpoint.chunkers {
            if chunker.handle.trim().is_empty()
                || previous_chunker.is_some_and(|previous| previous >= chunker.handle.as_str())
                || chunker.allocated > chunker.committed
            {
                return Err(CapacityError::InvalidCheckpoint(
                    "capacity chunker index is invalid or non-canonical".to_owned(),
                ));
            }
            previous_chunker = Some(&chunker.handle);
            committed_by_chunkers = committed_by_chunkers
                .checked_add(chunker.committed)
                .ok_or_else(|| {
                    CapacityError::InvalidCheckpoint(
                        "chunker committed capacity overflow".to_owned(),
                    )
                })?;
            chunkers.insert(
                chunker.handle.clone(),
                ChunkerAllocation {
                    committed: chunker.committed,
                    allocated: chunker.allocated,
                },
            );
        }
        if chunkers.is_empty() || committed_by_chunkers != checkpoint.committed_total_gib {
            return Err(CapacityError::InvalidCheckpoint(
                "chunker commitments do not sum to total committed capacity".to_owned(),
            ));
        }
        if declaration.chunker_commitments.len() != chunkers.len()
            || declaration.chunker_commitments.iter().any(|commitment| {
                chunkers
                    .get(&commitment.profile_id)
                    .is_none_or(|allocation| allocation.committed != commitment.committed_gib)
            })
        {
            return Err(CapacityError::InvalidCheckpoint(
                "capacity declaration chunkers disagree with checkpoint allocations".to_owned(),
            ));
        }

        let mut lanes = HashMap::with_capacity(checkpoint.lanes.len());
        let mut previous_lane: Option<&str> = None;
        for lane in &checkpoint.lanes {
            if lane.lane_id.trim().is_empty()
                || previous_lane.is_some_and(|previous| previous >= lane.lane_id.as_str())
                || lane.allocated > lane.max
            {
                return Err(CapacityError::InvalidCheckpoint(
                    "capacity lane index is invalid or non-canonical".to_owned(),
                ));
            }
            previous_lane = Some(&lane.lane_id);
            lanes.insert(
                lane.lane_id.clone(),
                LaneAllocation {
                    max: lane.max,
                    allocated: lane.allocated,
                },
            );
        }
        if declaration.lane_commitments.len() != lanes.len()
            || declaration.lane_commitments.iter().any(|commitment| {
                lanes
                    .get(&commitment.lane_id)
                    .is_none_or(|allocation| allocation.max != commitment.max_gib)
            })
        {
            return Err(CapacityError::InvalidCheckpoint(
                "capacity declaration lanes disagree with checkpoint allocations".to_owned(),
            ));
        }

        let mut outstanding_orders = HashMap::with_capacity(checkpoint.outstanding_orders.len());
        let mut previous_order = None;
        let mut allocated_total_gib = 0u64;
        let mut allocated_by_chunker = HashMap::<String, u64>::new();
        let mut allocated_by_lane = HashMap::<String, u64>::new();
        for order in checkpoint.outstanding_orders {
            if order.slice_gib == 0
                || previous_order.is_some_and(|previous| previous >= order.order_id)
                || !chunkers.contains_key(&order.chunker_handle)
                || order
                    .lane
                    .as_ref()
                    .is_some_and(|lane| !lanes.contains_key(lane))
            {
                return Err(CapacityError::InvalidCheckpoint(
                    "outstanding capacity order is invalid or non-canonical".to_owned(),
                ));
            }
            previous_order = Some(order.order_id);
            allocated_total_gib = allocated_total_gib
                .checked_add(order.slice_gib)
                .ok_or_else(|| {
                    CapacityError::InvalidCheckpoint(
                        "outstanding capacity allocation overflow".to_owned(),
                    )
                })?;
            let chunker_total = allocated_by_chunker
                .entry(order.chunker_handle.clone())
                .or_default();
            *chunker_total = chunker_total.checked_add(order.slice_gib).ok_or_else(|| {
                CapacityError::InvalidCheckpoint("chunker capacity allocation overflow".to_owned())
            })?;
            if let Some(lane) = &order.lane {
                let lane_total = allocated_by_lane.entry(lane.clone()).or_default();
                *lane_total = lane_total.checked_add(order.slice_gib).ok_or_else(|| {
                    CapacityError::InvalidCheckpoint("lane capacity allocation overflow".to_owned())
                })?;
            }
            outstanding_orders.insert(
                order.order_id,
                OrderAllocation {
                    slice_gib: order.slice_gib,
                    chunker_handle: order.chunker_handle,
                    lane: order.lane,
                    issued_at: order.issued_at,
                    deadline_at: order.deadline_at,
                },
            );
        }
        if allocated_total_gib != checkpoint.allocated_total_gib
            || allocated_total_gib > checkpoint.committed_total_gib
        {
            return Err(CapacityError::InvalidCheckpoint(
                "total capacity allocation disagrees with outstanding orders".to_owned(),
            ));
        }
        for (handle, allocation) in &chunkers {
            if allocation.allocated
                != allocated_by_chunker
                    .get(handle)
                    .copied()
                    .unwrap_or_default()
            {
                return Err(CapacityError::InvalidCheckpoint(format!(
                    "chunker `{handle}` allocation disagrees with outstanding orders"
                )));
            }
        }
        for (lane_id, allocation) in &lanes {
            if allocation.allocated != allocated_by_lane.get(lane_id).copied().unwrap_or_default() {
                return Err(CapacityError::InvalidCheckpoint(format!(
                    "lane `{lane_id}` allocation disagrees with outstanding orders"
                )));
            }
        }

        Ok(Self {
            declaration_payload: checkpoint.declaration_payload,
            provider_id: checkpoint.provider_id,
            committed_total_gib: checkpoint.committed_total_gib,
            chunkers,
            lanes,
            allocated_total_gib,
            outstanding_orders,
            metadata: checkpoint.metadata,
            declaration_window: checkpoint.declaration_window,
        })
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
                issued_at: order.issued_at,
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
        let mut chunkers = self
            .chunkers
            .iter()
            .map(|(handle, allocation)| ChunkerUsage {
                handle: handle.clone(),
                committed_gib: allocation.committed,
                allocated_gib: allocation.allocated,
                available_gib: allocation.available(),
            })
            .collect::<Vec<_>>();
        chunkers.sort_by(|left, right| left.handle.cmp(&right.handle));

        let mut lanes = self
            .lanes
            .iter()
            .map(|(lane, allocation)| LaneUsage {
                lane_id: lane.clone(),
                max_gib: allocation.max,
                allocated_gib: allocation.allocated,
                available_gib: allocation.available(),
            })
            .collect::<Vec<_>>();
        lanes.sort_by(|left, right| left.lane_id.cmp(&right.lane_id));

        let mut outstanding_orders = self
            .outstanding_orders
            .iter()
            .map(|(order_id, allocation)| OutstandingOrder {
                order_id: *order_id,
                slice_gib: allocation.slice_gib,
                chunker_handle: allocation.chunker_handle.clone(),
                lane: allocation.lane.clone(),
                issued_at: allocation.issued_at,
                deadline_at: allocation.deadline_at,
            })
            .collect::<Vec<_>>();
        outstanding_orders.sort_by_key(|order| order.order_id);

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
    issued_at: u64,
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
    /// Timestamp when governance issued the order.
    pub issued_at: u64,
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
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
    /// A configured authoritative-state ceiling was reached.
    #[error("capacity resource `{resource}` exhausted (limit {limit})")]
    ResourceExhausted {
        /// Bounded resource label.
        resource: &'static str,
        /// Configured entry ceiling.
        limit: usize,
    },
    /// Replacing a declaration would discard live reservations.
    #[error("cannot replace capacity declaration while {count} orders remain outstanding")]
    DeclarationReplacementWhileOrdersOutstanding {
        /// Outstanding reservation count.
        count: usize,
    },
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
    /// A durable checkpoint failed structural or accounting validation.
    #[error("invalid capacity runtime checkpoint: {0}")]
    InvalidCheckpoint(String),
    /// A durable checkpoint could not be committed.
    #[error("capacity runtime checkpoint failed: {0}")]
    Checkpoint(String),
    /// The in-memory capacity state lock was poisoned.
    #[error("capacity state lock poisoned")]
    StateLockPoisoned,
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
                stake_amount: sorafs_manifest::deal::XorQuantity::try_from_micro(5_000)
                    .expect("legacy micro-XOR stake is representable"),
            },
            committed_capacity_gib: 500,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: Some(vec!["sorafs.sf1@1.0.0".into(), "sorafs-sf1".into()]),
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

    #[test]
    fn failed_lane_reservation_is_transactional() {
        let (manager, record) = make_record_and_manager();
        manager
            .record_declaration(&record)
            .expect("record declaration");
        let mut order = make_order(100);
        order.assignments[0].lane = Some("missing".to_owned());
        assert!(matches!(
            manager
                .schedule_order(&order)
                .expect_err("unknown lane must fail"),
            CapacityError::UnknownLane { .. }
        ));
        let snapshot = manager.usage_snapshot();
        assert_eq!(snapshot.allocated_total_gib, 0);
        assert_eq!(snapshot.chunkers[0].allocated_gib, 0);
        assert_eq!(snapshot.lanes[0].allocated_gib, 0);
        assert!(snapshot.outstanding_orders.is_empty());
    }

    #[test]
    fn configured_limit_refuses_excess_orders_and_live_declaration_replacement() {
        let (_, record) = make_record_and_manager();
        let manager = CapacityManager::with_entry_limit(1);
        manager
            .record_declaration(&record)
            .expect("record declaration");
        let first = make_order(100);
        manager
            .schedule_order(&first)
            .expect("schedule first order")
            .expect("targeted plan");

        let mut second = make_order(100);
        second.order_id = [0x34; 32];
        assert!(matches!(
            manager
                .schedule_order(&second)
                .expect_err("second order must be refused"),
            CapacityError::ResourceExhausted {
                resource: "outstanding_orders",
                limit: 1
            }
        ));
        assert!(matches!(
            manager
                .record_declaration(&record)
                .expect_err("live reservations must block declaration replacement"),
            CapacityError::DeclarationReplacementWhileOrdersOutstanding { count: 1 }
        ));
        assert_eq!(manager.usage_snapshot().allocated_total_gib, 100);
    }

    #[test]
    fn checkpoint_roundtrip_preserves_orders_and_rejects_forged_accounting() {
        let (manager, record) = make_record_and_manager();
        manager
            .record_declaration(&record)
            .expect("record declaration");
        manager
            .schedule_order(&make_order(100))
            .expect("schedule order")
            .expect("targeted plan");
        let checkpoint = manager.checkpoint().expect("checkpoint");
        let expected = norito::to_bytes(&checkpoint).expect("encode checkpoint");

        let restored = CapacityManager::with_entry_limit(8);
        restored
            .restore_checkpoint(checkpoint.clone())
            .expect("restore checkpoint");
        assert_eq!(
            norito::to_bytes(&restored.checkpoint().expect("restored checkpoint"))
                .expect("encode restored checkpoint"),
            expected
        );
        assert_eq!(restored.usage_snapshot().allocated_total_gib, 100);

        let mut forged_total = checkpoint.clone();
        forged_total
            .active
            .as_mut()
            .expect("active checkpoint")
            .allocated_total_gib = 99;
        assert!(matches!(
            CapacityManager::with_entry_limit(8)
                .restore_checkpoint(forged_total)
                .expect_err("forged total must fail"),
            CapacityError::InvalidCheckpoint(_)
        ));

        let mut corrupt_declaration = checkpoint;
        corrupt_declaration
            .active
            .as_mut()
            .expect("active checkpoint")
            .declaration_payload = b"not-norito".to_vec();
        assert!(matches!(
            CapacityManager::with_entry_limit(8)
                .restore_checkpoint(corrupt_declaration)
                .expect_err("corrupt declaration must fail"),
            CapacityError::InvalidCheckpoint(_)
        ));
    }
}
