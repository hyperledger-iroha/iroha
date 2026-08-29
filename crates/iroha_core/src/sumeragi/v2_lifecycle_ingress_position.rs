//! Sealed pre-dequeue fair-ingress geometry for lifecycle planning.
use super::super::{
    FairV2Ingress, FairV2IngressClass, FairV2IngressDequeueDisposition, FairV2IngressEntry,
    FairV2IngressLeaderWirePhase, FairV2IngressLeaderWireSelectorProjection,
    FairV2IngressLeaderWireToken, FairV2IngressOwnershipEvidence, FairV2IngressQueueGateVerdict,
    FairV2IngressSelectorAttemptDiagnosticV1, FairV2IngressSource, FairV2IngressSourceClass,
    FairV2IngressState, FairV2IngressWireKey, InboundBlockMessage,
    fair_v2_ingress_leader_wire_selector_projection, fair_v2_ingress_queue_gate_verdict,
    message::BlockMessage, select_fair_v2_ingress_candidate,
};
use super::schema::{LifecycleContext, LifecycleDigest};
use iroha_crypto::Hash;
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::Encode as _;
use parking_lot::MutexGuard;
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
    sync::Arc,
    time::Instant,
};
const PENDING_INGRESS_IDENTITY_DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:pending-ingress:v2";
/// Exact non-zero fair-ingress positions frozen before a dequeue predicate runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FairIngressQueuePositions {
    lane: NonZeroU64,
    source: NonZeroU64,
}
impl FairIngressQueuePositions {
    /// Return `(lane, source)` in lifecycle-rank field order.
    pub(super) const fn components(self) -> [u64; 2] {
        [self.lane.get(), self.source.get()]
    }
}
/// Queue-minted identity for one exact physical ingress occurrence.
///
/// The digest binds the authenticated wire carrier, its exact source owner and
/// ownership history, the queue-bound lifecycle context, and the receiver-local
/// physical admission ordinal. The selected target additionally proves that
/// its carrier-derived context equals this bound context. The identity contains
/// no superseded runtime lifecycle or scheduler ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct PendingFairIngressIdentity {
    context: LifecycleContext,
    digest: LifecycleDigest,
    physical_admission_ordinal: u64,
}
impl PendingFairIngressIdentity {
    /// Construct one exact queue identity for coordinator-child tests.
    #[cfg(test)]
    pub(super) const fn for_test(
        context: LifecycleContext,
        digest: LifecycleDigest,
        physical_admission_ordinal: u64,
    ) -> Self {
        Self {
            context,
            digest,
            physical_admission_ordinal,
        }
    }
    /// Return the exact queue-bound lifecycle context.
    pub(super) const fn context(&self) -> LifecycleContext {
        self.context
    }
    /// Return the canonical queue-local join digest.
    pub(super) const fn digest(&self) -> LifecycleDigest {
        self.digest
    }
    /// Return the unique physical fair-ingress admission ordinal.
    pub(super) const fn physical_admission_ordinal(&self) -> u64 {
        self.physical_admission_ordinal
    }
    /// Whether two snapshots name the same queue-bound physical coordinates.
    ///
    /// Exact retransmission coalescence deliberately changes the ownership-history
    /// digest without replacing the physical row. Callers must still compare the
    /// authenticated carrier and its semantic owner before treating such snapshots
    /// as one persistence handoff.
    pub(super) fn shares_physical_coordinates(&self, other: &Self) -> bool {
        self.context == other.context
            && self.physical_admission_ordinal == other.physical_admission_ordinal
    }
}
/// Failure to freeze or revalidate one exact pre-dequeue fair-ingress cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum FairIngressQueueCutError {
    /// The selected physical ordinal is the reserved zero value.
    ZeroTargetOrdinal,
    /// The selected physical ordinal is absent from this queue cut.
    MissingTarget,
    /// A ready source was repeated.
    DuplicateReadySource,
    /// A ready source had no corresponding non-empty lane.
    MissingReadyLane,
    /// A non-empty lane was absent from the ready-source sequence.
    ForeignReadyLane,
    /// Two physical occurrences reused one admission ordinal.
    DuplicateAdmissionOrdinal,
    /// A position or frozen physical cutoff was not representable.
    PositionOverflow,
    /// An explicit physical cutoff was zero or ahead of the live admission frontier.
    InvalidPhysicalCut,
    /// An ingress occurrence lost or changed its exact ownership carrier.
    InvalidOccurrenceIdentity,
    /// The target wire carrier does not expose one exact lifecycle context.
    MissingTargetContext,
    /// The target wire context disagrees with the bound fair-ingress height.
    ForeignTargetContext,
    /// The bound leader-wire selector authority is absent or inconsistent.
    InvalidLeaderWireAuthority,
    /// A prepared witness was presented to a different queue instance.
    ForeignQueue,
    /// A prepared witness disagreed with its enclosing lifecycle context.
    ForeignCommitContext,
    /// A prepared witness disagreed with the selected planned ordinal.
    ForeignCommitOrdinal,
    /// A selected occurrence cannot cross its frozen queue-local barrier.
    BlockedTarget,
    /// The prepared queue cut changed before the final atomic commit.
    QueueCutChanged,
    /// The ordinary durable dequeue handoff could not complete.
    DequeueFailed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct FrozenQueueOccurrence<V> {
    physical_admission_ordinal: u64,
    value: V,
}
#[derive(Clone, Debug)]
struct FrozenFairIngressOccurrence {
    wire_key: FairV2IngressWireKey,
    encoded_hash: Hash,
    encoded_len: usize,
    ownership_snapshot: Arc<FairV2IngressOwnershipEvidence>,
    source_class: FairV2IngressSourceClass,
    class: FairV2IngressClass,
    leader_wire_token: Option<FairV2IngressLeaderWireToken>,
    queue_gate: FairV2IngressQueueGateVerdict,
    obsolete: bool,
}
impl PartialEq for FrozenFairIngressOccurrence {
    fn eq(&self, other: &Self) -> bool {
        self.wire_key == other.wire_key
            && self.encoded_hash == other.encoded_hash
            && self.encoded_len == other.encoded_len
            && Arc::ptr_eq(&self.ownership_snapshot, &other.ownership_snapshot)
            && self.source_class == other.source_class
            && self.class == other.class
            && self.leader_wire_token == other.leader_wire_token
            && self.queue_gate == other.queue_gate
            && self.obsolete == other.obsolete
    }
}
impl Eq for FrozenFairIngressOccurrence {}
impl FrozenFairIngressOccurrence {
    /// Compare the immutable queue carrier while deliberately excluding the
    /// monotonic ownership/reply-route history refreshed by exact
    /// retransmission coalescence.
    fn same_carrier(&self, other: &Self) -> bool {
        self.wire_key == other.wire_key
            && self.encoded_hash == other.encoded_hash
            && self.encoded_len == other.encoded_len
            && self
                .ownership_snapshot
                .same_semantic_request(&other.ownership_snapshot)
            && self.source_class == other.source_class
            && self.class == other.class
            && self.leader_wire_token == other.leader_wire_token
            && self.queue_gate == other.queue_gate
            && self.obsolete == other.obsolete
    }
}
/// Opaque exact carrier retained for executor-owned selector classification.
///
/// The queue cut is the sole mint. Fields remain hidden so a sibling cannot
/// claim drainability, source class, or a physical ordinal independently of
/// the state-locked queue projection.
#[derive(Debug)]
pub(in crate::sumeragi) struct FairIngressSelectorOccurrence {
    physical_admission_ordinal: u64,
    context: Option<LifecycleContext>,
    source_class: FairV2IngressSourceClass,
    class: FairV2IngressClass,
    queue_gate: FairV2IngressQueueGateVerdict,
    obsolete: bool,
    inbound: Arc<InboundBlockMessage>,
}
impl FairIngressSelectorOccurrence {
    /// Return the exact queue-local physical ordinal.
    pub(in crate::sumeragi) const fn physical_admission_ordinal(&self) -> u64 {
        self.physical_admission_ordinal
    }
    /// Return the carrier-derived lifecycle context when the wire exposes one.
    pub(super) const fn context(&self) -> Option<LifecycleContext> {
        self.context
    }
    /// Return the authenticated fair-ingress source class.
    pub(super) const fn source_class(&self) -> FairV2IngressSourceClass {
        self.source_class
    }
    /// Return the exact physical queue class.
    pub(super) const fn class(&self) -> FairV2IngressClass {
        self.class
    }
    /// Return the queue-local barrier verdict frozen by the production helper.
    pub(super) const fn queue_gate(&self) -> FairV2IngressQueueGateVerdict {
        self.queue_gate
    }
    /// Return whether durable recovery already made this carrier obsolete.
    pub(super) const fn is_obsolete(&self) -> bool {
        self.obsolete
    }
    /// Borrow the exact immutable inbound carrier retained by the queue cut.
    pub(in crate::sumeragi) fn inbound(&self) -> &InboundBlockMessage {
        &self.inbound
    }
    /// Clone the immutable carrier owner without copying its encoded body.
    pub(super) fn clone_inbound(&self) -> Arc<InboundBlockMessage> {
        Arc::clone(&self.inbound)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct FrozenQueueGeometry<S, V> {
    ready_prefix: Vec<S>,
    lanes: BTreeMap<S, Vec<FrozenQueueOccurrence<V>>>,
    positions: BTreeMap<u64, FairIngressQueuePositions>,
}
fn same_fair_ingress_carrier_geometry(
    retained: &FrozenQueueGeometry<FairV2IngressSource, FrozenFairIngressOccurrence>,
    refreshed: &FrozenQueueGeometry<FairV2IngressSource, FrozenFairIngressOccurrence>,
) -> bool {
    retained.ready_prefix == refreshed.ready_prefix
        && retained.positions == refreshed.positions
        && retained.lanes.len() == refreshed.lanes.len()
        && retained.lanes.iter().all(|(source, retained_lane)| {
            refreshed.lanes.get(source).is_some_and(|refreshed_lane| {
                retained_lane.len() == refreshed_lane.len()
                    && retained_lane.iter().zip(refreshed_lane).all(
                        |(retained_occurrence, refreshed_occurrence)| {
                            retained_occurrence.physical_admission_ordinal
                                == refreshed_occurrence.physical_admission_ordinal
                                && retained_occurrence
                                    .value
                                    .same_carrier(&refreshed_occurrence.value)
                        },
                    )
            })
        })
}
/// Move-only owner of one exact pre-predicate fair-ingress service cut.
///
/// Lock order is always `service_lock`, optionally
/// `producer_publication_lock`, then `state`. The state guard is released
/// before this value is returned, while the service guard and an installed
/// producer fence may span composite coordinator snapshot validation.
/// Unfenced generic cuts still permit producers to append at or after
/// `physical_cut`; they cannot change the frozen prefix. No network, crypto,
/// effect execution, or coordinator mutation may run while a future dequeue
/// commit holds the state guard.
#[must_use = "the queue cut must be validated or consumed by the composite planner"]
pub(super) struct FairIngressQueueCut<'a> {
    queue: &'a FairV2Ingress,
    _service_guard: MutexGuard<'a, ()>,
    producer_publication_guard: Option<MutexGuard<'a, ()>>,
    physical_cut: u128,
    bound_context: (wire::HeightContextId, wire::Height),
    geometry: FrozenQueueGeometry<FairV2IngressSource, FrozenFairIngressOccurrence>,
    selector_occurrences: BTreeMap<u64, FairIngressSelectorOccurrence>,
    pending_identities: BTreeMap<u64, PendingFairIngressIdentity>,
    leader_wire_projection: FairV2IngressLeaderWireSelectorProjection,
    selected_identity: PendingFairIngressIdentity,
    selected_positions: FairIngressQueuePositions,
    selected_disposition: FairV2IngressDequeueDisposition,
}
/// Move-only owner of the exact ordinary-or-lifecycle fair-ingress winner.
///
/// Unlike [`FairIngressQueueCut`], this first-stage cut deliberately accepts a
/// carrier with no lifecycle context or with a foreign context.  It retains the
/// queue's one consumer service episode while the caller performs the same
/// stateful Certified-Serve predicate used by ordinary dequeue.  The cut can
/// then either narrow to a lifecycle census or physically remove this exact
/// occurrence; it exposes no free-standing ordinal or dequeue primitive.
#[must_use = "the selected ingress cut must narrow or dequeue its exact winner"]
pub(in crate::sumeragi) struct FairIngressTurnCut<'a> {
    queue: &'a FairV2Ingress,
    _service_guard: MutexGuard<'a, ()>,
    producer_publication_guard: Option<MutexGuard<'a, ()>>,
    physical_cut: u128,
    geometry: FrozenQueueGeometry<FairV2IngressSource, FrozenFairIngressOccurrence>,
    selector_occurrences: BTreeMap<u64, FairIngressSelectorOccurrence>,
    leader_wire_projection: FairV2IngressLeaderWireSelectorProjection,
    bound_context: Option<(wire::HeightContextId, wire::Height)>,
    selected_source_index: usize,
    selected_physical_ordinal: u64,
    selected_positions: FairIngressQueuePositions,
    selected_disposition: FairV2IngressDequeueDisposition,
}

/// Result of narrowing one exact fair-ingress winner to the active lifecycle.
pub(super) enum FairIngressTurnContextCut<'a> {
    /// The selected carrier belongs to the active lifecycle context.
    Lifecycle(FairIngressQueueCut<'a>),
    /// The selected carrier is ordinary, context-free, or foreign-context work.
    Ordinary(FairIngressTurnCut<'a>),
}
/// Borrow-free opaque witness of one fully revalidated pre-cut queue.
///
/// The witness deliberately retains the complete comparable geometry and both
/// transitional leader-wire barrier projection. A future synchronous transaction can
/// consume it under fresh service/state locks and reject any pre-cut reorder,
/// removal, coalescence, gate change, or ownership mutation. It exposes no
/// constructor, clone, or general mutation surface; its sole dequeue is the
/// sealed move-consuming exact commit below.
#[must_use = "the prepared queue witness must be consumed by the atomic planner"]
pub(super) struct PreparedFairIngressQueueWitness {
    queue_identity: Arc<()>,
    physical_cut: u128,
    geometry: FrozenQueueGeometry<FairV2IngressSource, FrozenFairIngressOccurrence>,
    pending_identities: BTreeMap<u64, PendingFairIngressIdentity>,
    leader_wire_projection: FairV2IngressLeaderWireSelectorProjection,
    selected_identity: PendingFairIngressIdentity,
    selected_positions: FairIngressQueuePositions,
    selected_disposition: FairV2IngressDequeueDisposition,
}
struct PreparedFairIngressQueueSelection {
    ready_sources: Vec<FairV2IngressSource>,
    selected_source_index: usize,
    physical_admission_ordinal: u64,
    disposition: FairV2IngressDequeueDisposition,
}
/// Prevalidated exact dequeue retaining the queue's exclusive service lock.
///
/// The producer-publication fence excludes every queue-state producer
/// mutation between this final preflight and the post-LedgerV1 dequeue. No
/// competing consumer can change or remove the selected prefix either.
/// Consequently the post-fsync dequeue is assertion-only.
#[must_use = "locked fair-ingress dequeue must commit or retain its witness"]
pub(super) struct LockedPreparedFairIngressExactDequeue<'a> {
    queue: &'a FairV2Ingress,
    _service_guard: MutexGuard<'a, ()>,
    _producer_publication_guard: MutexGuard<'a, ()>,
    witness: PreparedFairIngressQueueWitness,
    selection: PreparedFairIngressQueueSelection,
}
impl PreparedFairIngressQueueWitness {
    /// Return the first physical ordinal excluded from this exact witness.
    pub(super) const fn physical_cut(&self) -> u128 {
        self.physical_cut
    }
    /// Return the queue-minted identity of the selected occurrence.
    pub(super) const fn selected_identity(&self) -> &PendingFairIngressIdentity {
        &self.selected_identity
    }
    /// Return the selected occurrence's exact lane/source positions.
    pub(super) const fn selected_positions(&self) -> FairIngressQueuePositions {
        self.selected_positions
    }
    /// Look up one exact pre-cut queue-minted occurrence identity.
    pub(super) fn identity_for_ordinal(
        &self,
        physical_admission_ordinal: u64,
    ) -> Option<&PendingFairIngressIdentity> {
        self.pending_identities.get(&physical_admission_ordinal)
    }
    /// Validate the witness's complete self-contained census before it leaves
    /// the sealed selector preparation boundary.
    pub(super) fn is_internally_exact(&self) -> bool {
        let selected_ordinal = self.selected_identity.physical_admission_ordinal;
        let _retained_barrier_projection = &self.leader_wire_projection;
        self.physical_cut > u128::from(selected_ordinal)
            && self.geometry.positions.len() == self.pending_identities.len()
            && self
                .geometry
                .positions
                .keys()
                .eq(self.pending_identities.keys())
            && self.geometry.positions.get(&selected_ordinal) == Some(&self.selected_positions)
            && self.pending_identities.get(&selected_ordinal) == Some(&self.selected_identity)
    }
    /// Return the frozen ordinary dequeue disposition for the selected row.
    pub(super) const fn selected_disposition(&self) -> FairV2IngressDequeueDisposition {
        self.selected_disposition
    }
    /// Pre-lock and revalidate the exact dequeue before any durable publication.
    #[allow(clippy::result_large_err)]
    pub(super) fn lock_exact_dequeue_retaining<'a>(
        self,
        queue: &'a FairV2Ingress,
        expected_context: LifecycleContext,
        expected_physical_ordinal: u64,
    ) -> Result<LockedPreparedFairIngressExactDequeue<'a>, (FairIngressQueueCutError, Self)> {
        if !Arc::ptr_eq(&self.queue_identity, &queue.queue_identity) {
            return Err((FairIngressQueueCutError::ForeignQueue, self));
        }
        if self.selected_identity.context != expected_context {
            return Err((FairIngressQueueCutError::ForeignCommitContext, self));
        }
        if self.selected_identity.physical_admission_ordinal != expected_physical_ordinal {
            return Err((FairIngressQueueCutError::ForeignCommitOrdinal, self));
        }
        if !self.is_internally_exact() {
            return Err((FairIngressQueueCutError::QueueCutChanged, self));
        }
        let service_guard = queue.service_lock.lock();
        let producer_publication_guard = queue.producer_publication_lock.lock();
        let selection = match self.revalidate_for_commit(queue) {
            Ok(selection) if selection.disposition == self.selected_disposition => selection,
            Ok(_) => return Err((FairIngressQueueCutError::QueueCutChanged, self)),
            Err(error) => return Err((error, self)),
        };
        let state = queue.state.lock();
        if !self.metadata_matches_locked(&state) {
            return Err((FairIngressQueueCutError::QueueCutChanged, self));
        }
        drop(state);
        Ok(LockedPreparedFairIngressExactDequeue {
            queue,
            _service_guard: service_guard,
            _producer_publication_guard: producer_publication_guard,
            witness: self,
            selection,
        })
    }
    /// Atomically remove the exact selected occurrence while retaining this
    /// witness on every rejected comparison.
    ///
    /// This retained comparison remains a queue-level diagnostic and test seam.
    /// Durable lifecycle transactions must instead acquire
    /// [`Self::lock_exact_dequeue_retaining`] before LedgerV1 so producer
    /// coalescence cannot create a fallible post-publication CAS.
    #[allow(clippy::result_large_err)]
    pub(super) fn commit_exact_dequeue_retaining(
        self,
        queue: &FairV2Ingress,
        expected_context: LifecycleContext,
        expected_physical_ordinal: u64,
    ) -> Result<
        (InboundBlockMessage, FairV2IngressDequeueDisposition),
        (FairIngressQueueCutError, Self),
    > {
        macro_rules! retain {
            ($result:expr) => {
                match $result {
                    Ok(value) => value,
                    Err(error) => return Err((error, self)),
                }
            };
        }
        if !Arc::ptr_eq(&self.queue_identity, &queue.queue_identity) {
            return Err((FairIngressQueueCutError::ForeignQueue, self));
        }
        if self.selected_identity.context != expected_context {
            return Err((FairIngressQueueCutError::ForeignCommitContext, self));
        }
        if self.selected_identity.physical_admission_ordinal != expected_physical_ordinal {
            return Err((FairIngressQueueCutError::ForeignCommitOrdinal, self));
        }
        if !self.is_internally_exact() {
            return Err((FairIngressQueueCutError::QueueCutChanged, self));
        }
        let _service_guard = queue.service_lock.lock();
        let selection = retain!(self.revalidate_for_commit(queue));
        if selection.disposition != self.selected_disposition {
            return Err((FairIngressQueueCutError::QueueCutChanged, self));
        }
        let mut state = queue.state.lock();
        if !self.metadata_matches_locked(&state) {
            return Err((FairIngressQueueCutError::QueueCutChanged, self));
        }
        let dequeued = retain!(
            queue
                .dequeue_selected_locked(
                    &mut state,
                    &selection.ready_sources,
                    selection.selected_source_index,
                    selection.physical_admission_ordinal,
                    selection.disposition,
                    true,
                    false,
                    Instant::now(),
                )
                .map_err(|_| FairIngressQueueCutError::DequeueFailed)
        );
        Ok(dequeued)
    }
    /// Atomically remove the exact selected occurrence after revalidating this
    /// complete prepared cut.
    ///
    /// This primitive remains sealed inside the lifecycle coordinator. Its
    /// redundant context and ordinal inputs must eventually come from the
    /// enclosing prepared selector and selected turn plan; neither can replace
    /// the queue-minted witness. Expensive carrier validation runs outside the
    /// queue-state lock while exclusive dequeue service remains held. The
    /// final state-locked comparison performs no network, crypto, or body work
    /// before entering the ordinary mutation tail. A caller must already have
    /// dropped its retained inbound `Arc` clones; the shared tail refuses
    /// non-exclusive envelopes. Production durability paths use the locked
    /// surface above, while this wrapper retains direct queue regression
    /// coverage.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn commit_exact_dequeue(
        self,
        queue: &FairV2Ingress,
        expected_context: LifecycleContext,
        expected_physical_ordinal: u64,
    ) -> Result<(InboundBlockMessage, FairV2IngressDequeueDisposition), FairIngressQueueCutError>
    {
        self.commit_exact_dequeue_retaining(queue, expected_context, expected_physical_ordinal)
            .map_err(|(error, _witness)| error)
    }
    fn revalidate_for_commit(
        &self,
        queue: &FairV2Ingress,
    ) -> Result<PreparedFairIngressQueueSelection, FairIngressQueueCutError> {
        let state = queue.state.lock();
        validate_live_queue_structure(&state)?;
        let leader_wire_projection =
            fair_v2_ingress_leader_wire_selector_projection(&state, true, Some(self.physical_cut))
                .map_err(|_| FairIngressQueueCutError::InvalidLeaderWireAuthority)?;
        let (current, selector_occurrences) =
            freeze_live_geometry(&state, self.physical_cut, &leader_wire_projection)?;
        if !state
            .ready
            .iter()
            .take(self.geometry.ready_prefix.len())
            .eq(self.geometry.ready_prefix.iter())
            || current != self.geometry
            || leader_wire_projection != self.leader_wire_projection
        {
            return Err(FairIngressQueueCutError::QueueCutChanged);
        }
        let selected_ordinal = self.selected_identity.physical_admission_ordinal;
        let selected = find_entry_by_physical_ordinal(&state, selected_ordinal)
            .ok_or(FairIngressQueueCutError::QueueCutChanged)?;
        let selected_source = source_for_physical_ordinal(&state, selected_ordinal)
            .ok_or(FairIngressQueueCutError::QueueCutChanged)?
            .clone();
        let selected_projection = frozen_projection_for_ordinal(&current, selected_ordinal)
            .ok_or(FairIngressQueueCutError::QueueCutChanged)?
            .clone();
        let selected_context =
            target_lifecycle_context(selected).ok_or(FairIngressQueueCutError::QueueCutChanged)?;
        let bound_context = state
            .leader_wire_context
            .ok_or(FairIngressQueueCutError::QueueCutChanged)?;
        if selected_context != bound_context
            || lifecycle_context_from_wire(bound_context) != self.selected_identity.context
        {
            return Err(FairIngressQueueCutError::QueueCutChanged);
        }
        drop(state);
        // Message re-encoding and process-local ownership hashing stay out of
        // the state critical section. `service_guard` still excludes every
        // competing dequeue, while producer changes are caught by the final
        // cached projection comparison below.
        validate_frozen_ownership_outside_state(&current, &selector_occurrences)?;
        let pending_identities = mint_pending_identities(bound_context, &current)?;
        let selected_identity = pending_identity(
            selected_context,
            &selected_source,
            &selected_projection,
            selected_ordinal,
        );
        if selected_identity != self.selected_identity
            || pending_identities != self.pending_identities
        {
            return Err(FairIngressQueueCutError::QueueCutChanged);
        }
        let positions = current
            .positions
            .get(&selected_ordinal)
            .copied()
            .ok_or(FairIngressQueueCutError::QueueCutChanged)?;
        if positions != self.selected_positions {
            return Err(FairIngressQueueCutError::QueueCutChanged);
        }
        let selected_source_index = self
            .geometry
            .ready_prefix
            .iter()
            .position(|source| source == &selected_source)
            .ok_or(FairIngressQueueCutError::QueueCutChanged)?;
        let source_position = u64::try_from(selected_source_index)
            .ok()
            .and_then(|position| position.checked_add(1))
            .ok_or(FairIngressQueueCutError::PositionOverflow)?;
        if source_position != self.selected_positions.source.get() {
            return Err(FairIngressQueueCutError::QueueCutChanged);
        }
        let selected_lane = self
            .geometry
            .lanes
            .get(&selected_source)
            .ok_or(FairIngressQueueCutError::QueueCutChanged)?;
        let lane_index = selected_lane
            .iter()
            .position(|occurrence| occurrence.physical_admission_ordinal == selected_ordinal)
            .ok_or(FairIngressQueueCutError::QueueCutChanged)?;
        let lane_position = u64::try_from(lane_index)
            .ok()
            .and_then(|position| position.checked_add(1))
            .ok_or(FairIngressQueueCutError::PositionOverflow)?;
        if lane_position != self.selected_positions.lane.get() {
            return Err(FairIngressQueueCutError::QueueCutChanged);
        }
        if selected_projection.queue_gate == FairV2IngressQueueGateVerdict::Blocked {
            return Err(FairIngressQueueCutError::BlockedTarget);
        }
        let disposition = if selected_projection.obsolete {
            FairV2IngressDequeueDisposition::RetireObsolete
        } else {
            FairV2IngressDequeueDisposition::Admit
        };
        Ok(PreparedFairIngressQueueSelection {
            ready_sources: self.geometry.ready_prefix.clone(),
            selected_source_index,
            physical_admission_ordinal: selected_ordinal,
            disposition,
        })
    }
    fn metadata_matches_locked(&self, state: &FairV2IngressState) -> bool {
        if validate_live_queue_structure(state).is_err() {
            return false;
        }
        let Ok(leader_wire_projection) =
            fair_v2_ingress_leader_wire_selector_projection(state, true, Some(self.physical_cut))
        else {
            return false;
        };
        let Ok((current, selector_occurrences)) =
            freeze_live_geometry(state, self.physical_cut, &leader_wire_projection)
        else {
            return false;
        };
        drop(selector_occurrences);
        let selected_context_is_exact = find_entry_by_physical_ordinal(
            state,
            self.selected_identity.physical_admission_ordinal,
        )
        .and_then(target_lifecycle_context)
        .zip(state.leader_wire_context)
        .is_some_and(|(carrier, bound)| {
            carrier == bound && lifecycle_context_from_wire(bound) == self.selected_identity.context
        });
        selected_context_is_exact
            && state
                .ready
                .iter()
                .take(self.geometry.ready_prefix.len())
                .eq(self.geometry.ready_prefix.iter())
            && current == self.geometry
            && leader_wire_projection == self.leader_wire_projection
    }
}
impl LockedPreparedFairIngressExactDequeue<'_> {
    /// Release the pre-publication locks while retaining the exact witness.
    ///
    /// Restart-only callers use this after a durable publication attempt has
    /// failed. The witness remains diagnostic authority for the same physical
    /// row, but cannot be committed without passing a fresh lock/revalidation.
    pub(super) fn unlock_retaining(self) -> PreparedFairIngressQueueWitness {
        let Self {
            queue: _,
            _service_guard,
            _producer_publication_guard,
            witness,
            selection: _,
        } = self;
        drop(_producer_publication_guard);
        drop(_service_guard);
        witness
    }

    /// Assertion-remove the prevalidated occurrence after LedgerV1 fsync.
    pub(super) fn commit(self) -> (InboundBlockMessage, FairV2IngressDequeueDisposition) {
        let Self {
            queue,
            _service_guard,
            _producer_publication_guard,
            witness,
            selection,
        } = self;
        let mut state = queue.state.lock();
        assert!(
            witness.metadata_matches_locked(&state),
            "locked lifecycle ingress prefix changed after LedgerV1 publication"
        );
        let dequeued = queue
            .dequeue_selected_locked(
                &mut state,
                &selection.ready_sources,
                selection.selected_source_index,
                selection.physical_admission_ordinal,
                selection.disposition,
                true,
                false,
                Instant::now(),
            )
            .expect("prevalidated lifecycle dequeue is infallible after publication");
        drop(state);
        drop(_producer_publication_guard);
        drop(_service_guard);
        dequeued
    }
}
impl FairIngressQueueCut<'_> {
    /// Return the first physical ordinal excluded from this frozen cut.
    pub(super) const fn physical_cut(&self) -> u128 {
        self.physical_cut
    }
    /// Return the queue-minted identity of the uniquely selected occurrence.
    pub(super) const fn selected_identity(&self) -> &PendingFairIngressIdentity {
        &self.selected_identity
    }
    /// Look up the queue-minted identity of any exact pre-cut occurrence.
    pub(super) fn identity_for_ordinal(
        &self,
        physical_admission_ordinal: u64,
    ) -> Option<&PendingFairIngressIdentity> {
        self.pending_identities.get(&physical_admission_ordinal)
    }
    /// Return the selected occurrence's exact lane and source positions.
    pub(super) const fn selected_positions(&self) -> FairIngressQueuePositions {
        self.selected_positions
    }
    /// Return the exact queue-frozen disposition for the selected occurrence.
    pub(super) const fn selected_disposition(&self) -> FairV2IngressDequeueDisposition {
        self.selected_disposition
    }
    /// Borrow the complete exact pre-cut occurrence census in ordinal order.
    pub(super) fn selector_occurrences(
        &self,
    ) -> impl ExactSizeIterator<Item = &FairIngressSelectorOccurrence> {
        self.selector_occurrences.values()
    }
    /// Consume a cut after its final read-only revalidation and retain every
    /// field needed by the future exact queue commit.
    pub(super) fn into_prepared_witness(self) -> PreparedFairIngressQueueWitness {
        let Self {
            queue,
            _service_guard,
            producer_publication_guard,
            physical_cut,
            bound_context: _,
            geometry,
            selector_occurrences: _,
            pending_identities,
            leader_wire_projection,
            selected_identity,
            selected_positions,
            selected_disposition,
        } = self;
        let queue_identity = Arc::clone(&queue.queue_identity);
        drop(producer_publication_guard);
        drop(_service_guard);
        PreparedFairIngressQueueWitness {
            queue_identity,
            physical_cut,
            geometry,
            pending_identities,
            leader_wire_projection,
            selected_identity,
            selected_positions,
            selected_disposition,
        }
    }
    /// Revalidate the frozen prefix while retaining exclusive dequeue service.
    ///
    /// Appends at or after the physical cut are intentionally ignored, but a
    /// changed ready prefix, removed/reordered pre-cut occurrence, ownership
    /// mutation, or target-context mutation fails the comparison. Structurally
    /// invalid live ownership also fails closed.
    pub(super) fn pre_cut_is_intact(&self) -> bool {
        let state = self.queue.state.lock();
        if validate_live_queue_structure(&state).is_err() {
            return false;
        }
        let Ok(leader_wire_projection) =
            fair_v2_ingress_leader_wire_selector_projection(&state, true, Some(self.physical_cut))
        else {
            return false;
        };
        let Ok((current, selector_occurrences)) =
            freeze_live_geometry(&state, self.physical_cut, &leader_wire_projection)
        else {
            return false;
        };
        if !state
            .ready
            .iter()
            .take(self.geometry.ready_prefix.len())
            .eq(self.geometry.ready_prefix.iter())
            || current != self.geometry
            || leader_wire_projection != self.leader_wire_projection
        {
            return false;
        }
        let Some(selected) = find_entry_by_physical_ordinal(
            &state,
            self.selected_identity.physical_admission_ordinal,
        ) else {
            return false;
        };
        let Some(context) = target_lifecycle_context(selected) else {
            return false;
        };
        let Some(bound_context) = state.leader_wire_context else {
            return false;
        };
        if context != bound_context || bound_context != self.bound_context {
            return false;
        }
        let Some(selected_source) =
            source_for_physical_ordinal(&state, self.selected_identity.physical_admission_ordinal)
        else {
            return false;
        };
        let Some(selected_projection) = frozen_projection_for_ordinal(
            &current,
            self.selected_identity.physical_admission_ordinal,
        ) else {
            return false;
        };
        let selected_source = selected_source.clone();
        let selected_projection = selected_projection.clone();
        let selected_ordinal = selected.admission_ordinal;
        drop(state);
        if validate_frozen_ownership_outside_state(&current, &selector_occurrences).is_err() {
            return false;
        }
        let Ok(pending_identities) = mint_pending_identities(bound_context, &current) else {
            return false;
        };
        let identity = pending_identity(
            context,
            &selected_source,
            &selected_projection,
            selected_ordinal,
        );
        identity == self.selected_identity
            && pending_identities == self.pending_identities
            && self.metadata_is_current()
    }
    fn metadata_is_current(&self) -> bool {
        let state = self.queue.state.lock();
        if validate_live_queue_structure(&state).is_err() {
            return false;
        }
        let Ok(leader_wire_projection) =
            fair_v2_ingress_leader_wire_selector_projection(&state, true, Some(self.physical_cut))
        else {
            return false;
        };
        let Ok((current, _)) =
            freeze_live_geometry(&state, self.physical_cut, &leader_wire_projection)
        else {
            return false;
        };
        state.leader_wire_context == Some(self.bound_context)
            && state
                .ready
                .iter()
                .take(self.geometry.ready_prefix.len())
                .eq(self.geometry.ready_prefix.iter())
            && current == self.geometry
            && leader_wire_projection == self.leader_wire_projection
    }
}
impl<'a> FairIngressQueueCut<'a> {
    /// Convert a still-service-owned lifecycle cut directly into its final lock.
    ///
    /// A producer fence retained by the originating turn cut is moved into the
    /// returned dequeue. An unfenced caller acquires that fence here and must
    /// still pass the exact frozen-prefix comparison. This direct surface avoids
    /// the borrow-free witness's intentional service-lock release/reacquire gap.
    #[allow(clippy::result_large_err)]
    pub(super) fn into_locked_exact_dequeue_retaining(
        mut self,
        expected_context: LifecycleContext,
        expected_physical_ordinal: u64,
    ) -> Result<LockedPreparedFairIngressExactDequeue<'a>, (FairIngressQueueCutError, Self)> {
        if self.selected_identity.context != expected_context {
            return Err((FairIngressQueueCutError::ForeignCommitContext, self));
        }
        if self.selected_identity.physical_admission_ordinal != expected_physical_ordinal {
            return Err((FairIngressQueueCutError::ForeignCommitOrdinal, self));
        }
        if self.producer_publication_guard.is_none() {
            self.producer_publication_guard = Some(self.queue.producer_publication_lock.lock());
        }
        if !self.pre_cut_is_intact() {
            return Err((FairIngressQueueCutError::QueueCutChanged, self));
        }

        let selection = (|| {
            let selected_source = source_for_frozen_ordinal(
                &self.geometry,
                self.selected_identity.physical_admission_ordinal,
            )
            .ok_or(FairIngressQueueCutError::MissingTarget)?;
            let selected_source_index = self
                .geometry
                .ready_prefix
                .iter()
                .position(|source| source == selected_source)
                .ok_or(FairIngressQueueCutError::MissingReadyLane)?;
            Ok(PreparedFairIngressQueueSelection {
                ready_sources: self.geometry.ready_prefix.clone(),
                selected_source_index,
                physical_admission_ordinal: self.selected_identity.physical_admission_ordinal,
                disposition: self.selected_disposition,
            })
        })();
        let selection = match selection {
            Ok(selection) => selection,
            Err(error) => return Err((error, self)),
        };

        let Self {
            queue,
            _service_guard,
            producer_publication_guard,
            physical_cut,
            bound_context: _,
            geometry,
            selector_occurrences,
            pending_identities,
            leader_wire_projection,
            selected_identity,
            selected_positions,
            selected_disposition,
        } = self;
        drop(selector_occurrences);
        let producer_publication_guard = producer_publication_guard
            .expect("direct exact dequeue installed one producer-publication fence");
        let witness = PreparedFairIngressQueueWitness {
            queue_identity: Arc::clone(&queue.queue_identity),
            physical_cut,
            geometry,
            pending_identities,
            leader_wire_projection,
            selected_identity,
            selected_positions,
            selected_disposition,
        };
        debug_assert!(witness.is_internally_exact());
        Ok(LockedPreparedFairIngressExactDequeue {
            queue,
            _service_guard,
            _producer_publication_guard: producer_publication_guard,
            witness,
            selection,
        })
    }

    /// Widen a current-context cut back to its exact queue-owned turn.
    ///
    /// This is used only when a complete executor census proves that a
    /// current-context CertifiedBodyResponse belongs to the ordinary owner.
    /// The same service guard remains held throughout the conversion.
    pub(super) fn into_ordinary_turn_cut(self) -> FairIngressTurnCut<'a> {
        let Self {
            queue,
            _service_guard,
            producer_publication_guard,
            physical_cut,
            bound_context,
            geometry,
            selector_occurrences,
            pending_identities: _,
            leader_wire_projection,
            selected_identity,
            selected_positions,
            selected_disposition,
        } = self;
        let selected_physical_ordinal = selected_identity.physical_admission_ordinal;
        let selected_source = source_for_frozen_ordinal(&geometry, selected_physical_ordinal)
            .expect("lifecycle-selected ingress remains in its frozen source");
        let selected_source_index = geometry
            .ready_prefix
            .iter()
            .position(|source| source == selected_source)
            .expect("lifecycle-selected ingress source remains ready");
        FairIngressTurnCut {
            queue,
            _service_guard,
            producer_publication_guard,
            physical_cut,
            geometry,
            selector_occurrences,
            leader_wire_projection,
            bound_context: Some(bound_context),
            selected_source_index,
            selected_physical_ordinal,
            selected_positions,
            selected_disposition,
        }
    }
}

impl<'a> FairIngressTurnCut<'a> {
    /// Borrow the exact selected immutable carrier while dequeue service stays held.
    pub(in crate::sumeragi) fn selected_occurrence(&self) -> &FairIngressSelectorOccurrence {
        self.selector_occurrences
            .get(&self.selected_physical_ordinal)
            .expect("selected ingress occurrence remains in its frozen census")
    }

    /// Return the queue-frozen ordinary dequeue disposition.
    pub(in crate::sumeragi) const fn selected_disposition(
        &self,
    ) -> FairV2IngressDequeueDisposition {
        self.selected_disposition
    }

    /// Fence every producer mutation and refresh monotonic ownership coalescence.
    ///
    /// The cut already owns `service_lock`, so acquiring the publication fence
    /// here follows the queue's global `service -> producer -> state` order.
    /// Exact retransmission may have replaced reply-route/ownership history
    /// before this call; that history is refreshed only when every immutable
    /// carrier, position, gate, and leader-wire projection remains identical.
    /// The returned cut retains both guards through lifecycle narrowing.
    #[allow(clippy::result_large_err)]
    pub(super) fn fence_producer_publication_retaining(
        mut self,
    ) -> Result<Self, (FairIngressQueueCutError, Self)> {
        if self.producer_publication_guard.is_none() {
            self.producer_publication_guard = Some(self.queue.producer_publication_lock.lock());
        }

        let refreshed = (|| {
            let state = self.queue.state.lock();
            validate_live_queue_structure(&state)?;
            let leader_wire_projection = fair_v2_ingress_leader_wire_selector_projection(
                &state,
                true,
                Some(self.physical_cut),
            )
            .map_err(|_| FairIngressQueueCutError::InvalidLeaderWireAuthority)?;
            let (geometry, selector_occurrences) =
                freeze_live_geometry(&state, self.physical_cut, &leader_wire_projection)?;
            let bound_context = state.leader_wire_context;
            if bound_context != self.bound_context
                || leader_wire_projection != self.leader_wire_projection
                || !same_fair_ingress_carrier_geometry(&self.geometry, &geometry)
            {
                return Err(FairIngressQueueCutError::QueueCutChanged);
            }
            let selected_positions = geometry
                .positions
                .get(&self.selected_physical_ordinal)
                .copied()
                .ok_or(FairIngressQueueCutError::MissingTarget)?;
            let selected_source =
                source_for_frozen_ordinal(&geometry, self.selected_physical_ordinal)
                    .ok_or(FairIngressQueueCutError::MissingTarget)?;
            let selected_source_index = geometry
                .ready_prefix
                .iter()
                .position(|source| source == selected_source)
                .ok_or(FairIngressQueueCutError::MissingReadyLane)?;
            let selected_projection =
                frozen_projection_for_ordinal(&geometry, self.selected_physical_ordinal)
                    .ok_or(FairIngressQueueCutError::MissingTarget)?;
            let selected_disposition = if selected_projection.obsolete {
                FairV2IngressDequeueDisposition::RetireObsolete
            } else {
                FairV2IngressDequeueDisposition::Admit
            };
            if selected_positions != self.selected_positions
                || selected_source_index != self.selected_source_index
                || selected_disposition != self.selected_disposition
            {
                return Err(FairIngressQueueCutError::QueueCutChanged);
            }
            drop(state);
            validate_frozen_ownership_outside_state(&geometry, &selector_occurrences)?;
            Ok((
                geometry,
                selector_occurrences,
                leader_wire_projection,
                bound_context,
            ))
        })();

        match refreshed {
            Ok((geometry, selector_occurrences, leader_wire_projection, bound_context)) => {
                self.geometry = geometry;
                self.selector_occurrences = selector_occurrences;
                self.leader_wire_projection = leader_wire_projection;
                self.bound_context = bound_context;
                Ok(self)
            }
            Err(error) => Err((error, self)),
        }
    }

    /// Narrow this exact winner to the active lifecycle when its carrier agrees.
    ///
    /// Context-free and foreign-context carriers remain ordinary without
    /// releasing the service guard. Structural inconsistency fails closed at
    /// the caller while that guard is still retained by the returned error cut.
    #[allow(clippy::result_large_err)]
    pub(super) fn narrow_to_lifecycle(
        self,
        expected_context: LifecycleContext,
    ) -> Result<FairIngressTurnContextCut<'a>, (FairIngressQueueCutError, Self)> {
        let selected = self.selected_occurrence();
        if selected.context() != Some(expected_context) {
            return Ok(FairIngressTurnContextCut::Ordinary(self));
        }
        let Some(bound_context) = self.bound_context else {
            return Err((FairIngressQueueCutError::MissingTargetContext, self));
        };
        if lifecycle_context_from_wire(bound_context) != expected_context {
            return Err((FairIngressQueueCutError::ForeignTargetContext, self));
        }
        let pending_identities = match mint_pending_identities(bound_context, &self.geometry) {
            Ok(identities) => identities,
            Err(error) => return Err((error, self)),
        };
        let Some(selected_source) =
            source_for_frozen_ordinal(&self.geometry, self.selected_physical_ordinal)
        else {
            return Err((FairIngressQueueCutError::MissingTarget, self));
        };
        let Some(selected_projection) =
            frozen_projection_for_ordinal(&self.geometry, self.selected_physical_ordinal)
        else {
            return Err((FairIngressQueueCutError::MissingTarget, self));
        };
        let selected_identity = pending_identity(
            bound_context,
            selected_source,
            selected_projection,
            self.selected_physical_ordinal,
        );
        if pending_identities.get(&self.selected_physical_ordinal) != Some(&selected_identity) {
            return Err((FairIngressQueueCutError::InvalidOccurrenceIdentity, self));
        }
        let Self {
            queue,
            _service_guard,
            producer_publication_guard,
            physical_cut,
            geometry,
            selector_occurrences,
            leader_wire_projection,
            bound_context: _,
            selected_source_index: _,
            selected_physical_ordinal: _,
            selected_positions,
            selected_disposition,
        } = self;
        let cut = FairIngressQueueCut {
            queue,
            _service_guard,
            producer_publication_guard,
            physical_cut,
            bound_context,
            geometry,
            selector_occurrences,
            pending_identities,
            leader_wire_projection,
            selected_identity,
            selected_positions,
            selected_disposition,
        };
        if !cut.metadata_is_current() {
            return Err((
                FairIngressQueueCutError::QueueCutChanged,
                cut.into_ordinary_turn_cut(),
            ));
        }
        Ok(FairIngressTurnContextCut::Lifecycle(cut))
    }

    /// Physically remove exactly this selected winner under the retained service episode.
    ///
    /// Every frozen inbound `Arc` is released before the ordinary durable
    /// mutation tail checks exclusive ownership. On rejection the cut is
    /// returned with its service guard still held so the caller can close
    /// consensus output before releasing queue ownership.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn dequeue_exact_retaining(
        mut self,
    ) -> Result<
        (InboundBlockMessage, FairV2IngressDequeueDisposition),
        (FairIngressQueueCutError, Self),
    > {
        drop(std::mem::take(&mut self.selector_occurrences));
        let ready_sources = self.geometry.ready_prefix.clone();
        let mut state = self.queue.state.lock();
        match self.queue.dequeue_selected_locked(
            &mut state,
            &ready_sources,
            self.selected_source_index,
            self.selected_physical_ordinal,
            self.selected_disposition,
            true,
            false,
            Instant::now(),
        ) {
            Ok(dequeued) => Ok(dequeued),
            Err(_) => {
                drop(state);
                Err((FairIngressQueueCutError::DequeueFailed, self))
            }
        }
    }
}
#[derive(Clone, Copy)]
enum LifecycleQueueCutTarget {
    Exact(u64),
    NextAdmissible,
}
#[derive(Clone, Copy)]
enum FairIngressTurnSelectionPolicy {
    OrdinaryRetireObsolete,
    PredicateOnly,
}
impl FairV2Ingress {
    /// Select the exact next ordinary-or-lifecycle winner under one service episode.
    ///
    /// This is the queue-owned replacement for a read-only lifecycle probe
    /// followed by ordinary reselection. It uses the production
    /// strict-before-dependency selector, keeps obsolete false-predicate
    /// retirement, and retains the service guard across the caller's stateful
    /// lifecycle preparation. A false predicate never removes or rotates an
    /// occurrence.
    pub(super) fn capture_next_ingress_turn_cut(
        &self,
        predicate: impl FnMut(&FairIngressSelectorOccurrence) -> bool,
    ) -> Result<Option<FairIngressTurnCut<'_>>, FairIngressQueueCutError> {
        self.capture_next_ingress_turn_cut_at(
            None,
            FairIngressTurnSelectionPolicy::OrdinaryRetireObsolete,
            predicate,
        )
    }

    /// Select the next ordinary-or-lifecycle winner strictly before an external cut.
    ///
    /// `physical_cut` is the first physical admission ordinal excluded from
    /// this service episode. It must be non-zero and no later than the queue's
    /// live next-admission ordinal while the service/state locks are held.
    /// Later producer appends therefore cannot enter the frozen geometry or
    /// renew a bounded pre-timeout scan. Selection and exact dequeue retain the
    /// ordinary queue gate, source rotation, ownership, and durable handoff.
    /// Unlike the ordinary turn, this bounded scan cannot select an unrelated
    /// WAL-obsolete carrier when the supplied exact predicate rejects it.
    pub(in crate::sumeragi) fn capture_next_ingress_turn_cut_before(
        &self,
        physical_cut: u128,
        predicate: impl FnMut(&FairIngressSelectorOccurrence) -> bool,
    ) -> Result<Option<FairIngressTurnCut<'_>>, FairIngressQueueCutError> {
        self.capture_next_ingress_turn_cut_at(
            Some(physical_cut),
            FairIngressTurnSelectionPolicy::PredicateOnly,
            predicate,
        )
    }

    /// Select before a fixed cut while allowing ordinary obsolete retirement.
    ///
    /// This is the bounded dependency companion to
    /// [`Self::capture_next_ingress_turn_cut_before`]. It preserves the same
    /// immutable physical prefix but lets an already WAL-obsolete predecessor
    /// cross the ordinary durable retirement tail even when `predicate`
    /// accepts only the exact work blocked behind that predecessor. It grants
    /// no gate bypass: a non-obsolete row must still satisfy `predicate` and a
    /// blocked row remains ineligible.
    pub(in crate::sumeragi) fn capture_next_ingress_turn_cut_before_with_obsolete_retirement(
        &self,
        physical_cut: u128,
        predicate: impl FnMut(&FairIngressSelectorOccurrence) -> bool,
    ) -> Result<Option<FairIngressTurnCut<'_>>, FairIngressQueueCutError> {
        self.capture_next_ingress_turn_cut_at(
            Some(physical_cut),
            FairIngressTurnSelectionPolicy::OrdinaryRetireObsolete,
            predicate,
        )
    }

    fn capture_next_ingress_turn_cut_at(
        &self,
        requested_physical_cut: Option<u128>,
        selection_policy: FairIngressTurnSelectionPolicy,
        mut predicate: impl FnMut(&FairIngressSelectorOccurrence) -> bool,
    ) -> Result<Option<FairIngressTurnCut<'_>>, FairIngressQueueCutError> {
        let service_guard = self.service_lock.lock();
        let service_attempt_at = Instant::now();
        let mut state = self.state.lock();
        validate_live_queue_structure(&state)?;
        let live_physical_cut = u128::from(state.last_admission_ordinal)
            .checked_add(1)
            .ok_or(FairIngressQueueCutError::PositionOverflow)?;
        let physical_cut = requested_physical_cut.unwrap_or(live_physical_cut);
        if physical_cut == 0 || physical_cut > live_physical_cut {
            return Err(FairIngressQueueCutError::InvalidPhysicalCut);
        }
        let leader_wire_projection =
            fair_v2_ingress_leader_wire_selector_projection(&state, true, Some(physical_cut))
                .map_err(|_| FairIngressQueueCutError::InvalidLeaderWireAuthority)?;
        let (geometry, selector_occurrences) =
            freeze_live_geometry(&state, physical_cut, &leader_wire_projection)?;
        let records_service_attempt = match selection_policy {
            FairIngressTurnSelectionPolicy::OrdinaryRetireObsolete => state.len != 0,
            FairIngressTurnSelectionPolicy::PredicateOnly => !selector_occurrences.is_empty(),
        };
        if records_service_attempt {
            state.last_service_attempt_at = Some(service_attempt_at);
        }
        let bound_context = state.leader_wire_context;
        drop(state);
        validate_frozen_ownership_outside_state(&geometry, &selector_occurrences)?;
        let candidates = geometry
            .ready_prefix
            .iter()
            .map(|source| {
                geometry
                    .lanes
                    .get(source)
                    .ok_or(FairIngressQueueCutError::MissingReadyLane)?
                    .iter()
                    .map(|occurrence| {
                        let selector = selector_occurrences
                            .get(&occurrence.physical_admission_ordinal)
                            .ok_or(FairIngressQueueCutError::InvalidOccurrenceIdentity)?;
                        if selector.queue_gate() != occurrence.value.queue_gate
                            || selector.is_obsolete() != occurrence.value.obsolete
                        {
                            return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
                        }
                        Ok(selector)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (gate_blocked, gate_strict, gate_dependency) = selector_occurrences.values().fold(
            (0usize, 0usize, 0usize),
            |(blocked, strict, dependency), occurrence| match occurrence.queue_gate() {
                FairV2IngressQueueGateVerdict::Blocked => {
                    (blocked.saturating_add(1), strict, dependency)
                }
                FairV2IngressQueueGateVerdict::Strict => {
                    (blocked, strict.saturating_add(1), dependency)
                }
                FairV2IngressQueueGateVerdict::Dependency => {
                    (blocked, strict, dependency.saturating_add(1))
                }
            },
        );
        let mut predicate_tested = 0usize;
        let mut predicate_rejected = 0usize;
        let selected = select_fair_v2_ingress_candidate(
            &candidates,
            |occurrence| {
                (
                    occurrence.physical_admission_ordinal(),
                    occurrence.queue_gate(),
                    matches!(
                        selection_policy,
                        FairIngressTurnSelectionPolicy::OrdinaryRetireObsolete
                    ) && occurrence.is_obsolete(),
                )
            },
            |occurrence| {
                predicate_tested = predicate_tested.saturating_add(1);
                let accepted = predicate(occurrence);
                if !accepted {
                    predicate_rejected = predicate_rejected.saturating_add(1);
                }
                accepted
            },
        );
        *self.last_selector_attempt.lock() = Some(FairV2IngressSelectorAttemptDiagnosticV1 {
            observed_at: Instant::now(),
            physical_cut,
            depth: selector_occurrences.len(),
            gate_blocked,
            gate_strict,
            gate_dependency,
            predicate_tested,
            predicate_rejected,
            selected: selected.is_some(),
        });
        let Some((selected_source_index, selected_physical_ordinal, _)) = selected else {
            return Ok(None);
        };
        drop(candidates);
        let selected_disposition = if selector_occurrences
            .get(&selected_physical_ordinal)
            .expect("shared fair selector returns one frozen occurrence")
            .is_obsolete()
        {
            FairV2IngressDequeueDisposition::RetireObsolete
        } else {
            FairV2IngressDequeueDisposition::Admit
        };
        let selected_positions = geometry
            .positions
            .get(&selected_physical_ordinal)
            .copied()
            .expect("shared fair selector returns one frozen occurrence");
        Ok(Some(FairIngressTurnCut {
            queue: self,
            _service_guard: service_guard,
            producer_publication_guard: None,
            physical_cut,
            geometry,
            selector_occurrences,
            leader_wire_projection,
            bound_context,
            selected_source_index,
            selected_physical_ordinal,
            selected_positions,
            selected_disposition,
        }))
    }

    /// Freeze one exact target's pre-predicate fair-ingress queue geometry.
    ///
    /// This is the sole mint for lifecycle lane/source positions. It acquires
    /// the same service lock as checked dequeue, then snapshots state at the
    /// next physical admission cutoff. The selected ordinal must name exactly
    /// one authenticated occurrence in the frozen ready prefix.
    // TODO: Consume this cut only from the future composite planner factory,
    // together with an executor-authenticated complete per-occurrence verdict
    // set for selector debt and the existing mode/capacity/runner snapshots.
    pub(super) fn capture_lifecycle_queue_cut(
        &self,
        target_physical_ordinal: u64,
    ) -> Result<FairIngressQueueCut<'_>, FairIngressQueueCutError> {
        if target_physical_ordinal == 0 {
            return Err(FairIngressQueueCutError::ZeroTargetOrdinal);
        }
        self.capture_lifecycle_queue_cut_for(
            LifecycleQueueCutTarget::Exact(target_physical_ordinal),
            |_| false,
        )?
        .ok_or(FairIngressQueueCutError::MissingTarget)
    }

    /// Freeze one complete queue census around the next fair admissible occurrence.
    ///
    /// Selection follows the ordinary dequeue's exact ready-source/lane order:
    /// strict candidates are considered first, then dependency-bypass
    /// candidates only when no strict candidate satisfies `predicate`.
    /// Nothing is removed or rotated. A selected non-lifecycle or foreign
    /// context is returned as `None`, leaving that occurrence to the ordinary
    /// runner without exposing its physical ordinal.
    pub(super) fn capture_next_lifecycle_queue_cut(
        &self,
        predicate: impl FnMut(&FairIngressSelectorOccurrence) -> bool,
    ) -> Result<Option<FairIngressQueueCut<'_>>, FairIngressQueueCutError> {
        self.capture_lifecycle_queue_cut_for(LifecycleQueueCutTarget::NextAdmissible, predicate)
    }

    fn capture_lifecycle_queue_cut_for(
        &self,
        target: LifecycleQueueCutTarget,
        mut predicate: impl FnMut(&FairIngressSelectorOccurrence) -> bool,
    ) -> Result<Option<FairIngressQueueCut<'_>>, FairIngressQueueCutError> {
        let service_guard = self.service_lock.lock();
        let state = self.state.lock();
        validate_live_queue_structure(&state)?;
        let physical_cut = u128::from(state.last_admission_ordinal)
            .checked_add(1)
            .ok_or(FairIngressQueueCutError::PositionOverflow)?;
        let leader_wire_projection =
            fair_v2_ingress_leader_wire_selector_projection(&state, true, Some(physical_cut))
                .map_err(|_| FairIngressQueueCutError::InvalidLeaderWireAuthority)?;
        let (geometry, selector_occurrences) =
            freeze_live_geometry(&state, physical_cut, &leader_wire_projection)?;
        let bound_context = state.leader_wire_context;
        drop(state);
        validate_frozen_ownership_outside_state(&geometry, &selector_occurrences)?;
        let next_admissible = matches!(target, LifecycleQueueCutTarget::NextAdmissible);
        let target_physical_ordinal = match target {
            LifecycleQueueCutTarget::Exact(ordinal) => ordinal,
            LifecycleQueueCutTarget::NextAdmissible => {
                let Some(ordinal) = select_next_admissible_ordinal(
                    &geometry,
                    &selector_occurrences,
                    &mut predicate,
                )?
                else {
                    return Ok(None);
                };
                ordinal
            }
        };
        let selected_positions = select_positions(&geometry, target_physical_ordinal)?;
        let selected = selector_occurrences
            .get(&target_physical_ordinal)
            .ok_or(FairIngressQueueCutError::MissingTarget)?;
        let selected_source = source_for_frozen_ordinal(&geometry, target_physical_ordinal)
            .ok_or(FairIngressQueueCutError::MissingTarget)?;
        let selected_projection = frozen_projection_for_ordinal(&geometry, target_physical_ordinal)
            .ok_or(FairIngressQueueCutError::MissingTarget)?;
        let Some(context) = selected.context() else {
            if next_admissible {
                return Ok(None);
            }
            return Err(FairIngressQueueCutError::MissingTargetContext);
        };
        let Some(bound_context) = bound_context else {
            if next_admissible {
                return Ok(None);
            }
            return Err(FairIngressQueueCutError::MissingTargetContext);
        };
        let bound_lifecycle_context = lifecycle_context_from_wire(bound_context);
        if context != bound_lifecycle_context {
            if next_admissible {
                return Ok(None);
            }
            return Err(FairIngressQueueCutError::ForeignTargetContext);
        }
        let selected_source = selected_source.clone();
        let selected_projection = selected_projection.clone();
        let pending_identities = mint_pending_identities(bound_context, &geometry)?;
        let selected_identity = pending_identity(
            bound_context,
            &selected_source,
            &selected_projection,
            target_physical_ordinal,
        );
        if pending_identities.get(&target_physical_ordinal) != Some(&selected_identity) {
            return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
        }
        let selected_disposition = if selected_projection.obsolete {
            FairV2IngressDequeueDisposition::RetireObsolete
        } else {
            FairV2IngressDequeueDisposition::Admit
        };
        let cut = FairIngressQueueCut {
            queue: self,
            _service_guard: service_guard,
            producer_publication_guard: None,
            physical_cut,
            bound_context,
            geometry,
            selector_occurrences,
            pending_identities,
            leader_wire_projection,
            selected_identity,
            selected_positions,
            selected_disposition,
        };
        if !cut.metadata_is_current() {
            return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
        }
        Ok(Some(cut))
    }
}
fn source_for_frozen_ordinal<'a>(
    geometry: &'a FrozenQueueGeometry<FairV2IngressSource, FrozenFairIngressOccurrence>,
    ordinal: u64,
) -> Option<&'a FairV2IngressSource> {
    geometry.lanes.iter().find_map(|(source, lane)| {
        lane.iter()
            .any(|occurrence| occurrence.physical_admission_ordinal == ordinal)
            .then_some(source)
    })
}

fn select_next_admissible_ordinal(
    geometry: &FrozenQueueGeometry<FairV2IngressSource, FrozenFairIngressOccurrence>,
    selector_occurrences: &BTreeMap<u64, FairIngressSelectorOccurrence>,
    predicate: &mut impl FnMut(&FairIngressSelectorOccurrence) -> bool,
) -> Result<Option<u64>, FairIngressQueueCutError> {
    let candidates = geometry
        .ready_prefix
        .iter()
        .map(|source| {
            geometry
                .lanes
                .get(source)
                .ok_or(FairIngressQueueCutError::MissingReadyLane)?
                .iter()
                .map(|occurrence| {
                    let selector = selector_occurrences
                        .get(&occurrence.physical_admission_ordinal)
                        .ok_or(FairIngressQueueCutError::InvalidOccurrenceIdentity)?;
                    if selector.queue_gate() != occurrence.value.queue_gate
                        || selector.is_obsolete() != occurrence.value.obsolete
                    {
                        return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
                    }
                    Ok(selector)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(select_fair_v2_ingress_candidate(
        &candidates,
        |occurrence| {
            (
                occurrence.physical_admission_ordinal(),
                occurrence.queue_gate(),
                occurrence.is_obsolete(),
            )
        },
        |occurrence| predicate(occurrence),
    )
    .map(|(_, ordinal, _)| ordinal))
}
fn mint_pending_identities(
    context: (wire::HeightContextId, wire::Height),
    geometry: &FrozenQueueGeometry<FairV2IngressSource, FrozenFairIngressOccurrence>,
) -> Result<BTreeMap<u64, PendingFairIngressIdentity>, FairIngressQueueCutError> {
    let mut identities = BTreeMap::new();
    for (source, lane) in &geometry.lanes {
        for occurrence in lane {
            let ordinal = occurrence.physical_admission_ordinal;
            let identity = pending_identity(context, source, &occurrence.value, ordinal);
            if identities.insert(ordinal, identity).is_some() {
                return Err(FairIngressQueueCutError::DuplicateAdmissionOrdinal);
            }
        }
    }
    if identities.len() != geometry.positions.len()
        || identities
            .keys()
            .any(|ordinal| !geometry.positions.contains_key(ordinal))
    {
        return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
    }
    Ok(identities)
}
fn freeze_live_geometry(
    state: &FairV2IngressState,
    physical_cut: u128,
    leader_wire_projection: &FairV2IngressLeaderWireSelectorProjection,
) -> Result<
    (
        FrozenQueueGeometry<FairV2IngressSource, FrozenFairIngressOccurrence>,
        BTreeMap<u64, FairIngressSelectorOccurrence>,
    ),
    FairIngressQueueCutError,
> {
    let mut lanes = BTreeMap::new();
    let mut selector_occurrences = BTreeMap::new();
    for (source, lane) in &state.lanes {
        let mut frozen = Vec::new();
        for (index, entry) in lane.entries.iter().enumerate() {
            if u128::from(entry.admission_ordinal) >= physical_cut {
                continue;
            }
            let queue_gate =
                fair_v2_ingress_queue_gate_verdict(source, lane, index, leader_wire_projection);
            let obsolete = entry
                .leader_wire_token
                .as_ref()
                .is_some_and(|token| leader_wire_projection.obsolete_tokens.contains(token));
            frozen.push(FrozenQueueOccurrence {
                physical_admission_ordinal: entry.admission_ordinal,
                value: exact_occurrence_projection(source, entry, queue_gate, obsolete)?,
            });
            if selector_occurrences
                .insert(
                    entry.admission_ordinal,
                    FairIngressSelectorOccurrence {
                        physical_admission_ordinal: entry.admission_ordinal,
                        context: target_lifecycle_context(entry).map(lifecycle_context_from_wire),
                        source_class: source.class(),
                        class: entry.class,
                        queue_gate,
                        obsolete,
                        inbound: Arc::clone(&entry.inbound),
                    },
                )
                .is_some()
            {
                return Err(FairIngressQueueCutError::DuplicateAdmissionOrdinal);
            }
        }
        if !frozen.is_empty() {
            lanes.insert(source.clone(), frozen);
        }
    }
    let ready_prefix = state
        .ready
        .iter()
        .filter(|source| lanes.contains_key(*source))
        .cloned()
        .collect::<Vec<_>>();
    let geometry = freeze_geometry(&ready_prefix, lanes, physical_cut)?;
    if selector_occurrences.len() != geometry.positions.len()
        || selector_occurrences
            .keys()
            .any(|ordinal| !geometry.positions.contains_key(ordinal))
    {
        return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
    }
    Ok((geometry, selector_occurrences))
}
fn validate_live_queue_structure(
    state: &FairV2IngressState,
) -> Result<(), FairIngressQueueCutError> {
    let mut ready_sources = BTreeSet::new();
    if state
        .ready
        .iter()
        .any(|source| !ready_sources.insert(source.clone()))
    {
        return Err(FairIngressQueueCutError::DuplicateReadySource);
    }
    let nonempty_sources = state
        .lanes
        .iter()
        .filter(|(_, lane)| !lane.entries.is_empty())
        .map(|(source, _)| source.clone())
        .collect::<BTreeSet<_>>();
    if ready_sources != nonempty_sources {
        return Err(FairIngressQueueCutError::ForeignReadyLane);
    }
    let mut admission_ordinals = BTreeSet::new();
    let mut pending_wire_owners = BTreeMap::new();
    let mut total_len = 0_usize;
    let mut total_bytes = 0_usize;
    for (source, lane) in &state.lanes {
        let mut pending_wire = BTreeSet::new();
        let mut progress_len = 0_usize;
        let mut certified_fence_escape_len = 0_usize;
        let mut timeout_vote_len = 0_usize;
        let mut transport_completion_len = 0_usize;
        let mut lane_bytes = 0_usize;
        let mut certified_fence_escape_bytes = 0_usize;
        let mut timeout_vote_bytes = 0_usize;
        let mut transport_completion_bytes = 0_usize;
        for entry in &lane.entries {
            if !entry_storage_is_exact(state, source, entry)
                || !admission_ordinals.insert(entry.admission_ordinal)
            {
                return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
            }
            let key = entry
                .wire_key
                .as_ref()
                .ok_or(FairIngressQueueCutError::InvalidOccurrenceIdentity)?;
            if !pending_wire.insert(key.clone())
                || pending_wire_owners
                    .insert(key.clone(), source.clone())
                    .is_some()
            {
                return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
            }
            total_len = total_len
                .checked_add(1)
                .ok_or(FairIngressQueueCutError::PositionOverflow)?;
            lane_bytes = lane_bytes
                .checked_add(entry.encoded_len)
                .ok_or(FairIngressQueueCutError::PositionOverflow)?;
            if entry.class == FairV2IngressClass::Progress {
                progress_len = progress_len
                    .checked_add(1)
                    .ok_or(FairIngressQueueCutError::PositionOverflow)?;
            }
            if super::super::fair_v2_ingress_is_certified_fence_escape(&entry.inbound) {
                certified_fence_escape_len = certified_fence_escape_len
                    .checked_add(1)
                    .ok_or(FairIngressQueueCutError::PositionOverflow)?;
                certified_fence_escape_bytes = certified_fence_escape_bytes
                    .checked_add(entry.encoded_len)
                    .ok_or(FairIngressQueueCutError::PositionOverflow)?;
            }
            if super::super::fair_v2_ingress_is_timeout_vote(&entry.inbound) {
                timeout_vote_len = timeout_vote_len
                    .checked_add(1)
                    .ok_or(FairIngressQueueCutError::PositionOverflow)?;
                if matches!(source, FairV2IngressSource::Validator(_)) {
                    timeout_vote_bytes = timeout_vote_bytes
                        .checked_add(entry.encoded_len)
                        .ok_or(FairIngressQueueCutError::PositionOverflow)?;
                }
            }
            if entry.class == FairV2IngressClass::TransportCompletion {
                transport_completion_len = transport_completion_len
                    .checked_add(1)
                    .ok_or(FairIngressQueueCutError::PositionOverflow)?;
                transport_completion_bytes = transport_completion_bytes
                    .checked_add(entry.encoded_len)
                    .ok_or(FairIngressQueueCutError::PositionOverflow)?;
            }
        }
        total_bytes = total_bytes
            .checked_add(lane_bytes)
            .ok_or(FairIngressQueueCutError::PositionOverflow)?;
        if lane.pending_wire != pending_wire
            || lane.progress_len != progress_len
            || lane.certified_fence_escape_len != certified_fence_escape_len
            || lane.timeout_vote_len != timeout_vote_len
            || lane.transport_completion_len != transport_completion_len
            || lane.bytes != lane_bytes
            || lane.certified_fence_escape_bytes != certified_fence_escape_bytes
            || lane.timeout_vote_bytes != timeout_vote_bytes
            || lane.transport_completion_bytes != transport_completion_bytes
        {
            return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
        }
    }
    if state.pending_wire_owners != pending_wire_owners
        || state.len != total_len
        || state.bytes != total_bytes
        || state.nonempty_since.is_some() != (total_len != 0)
        || admission_ordinals
            .last()
            .is_some_and(|ordinal| *ordinal > state.last_admission_ordinal)
    {
        return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
    }
    Ok(())
}
fn validate_frozen_ownership_outside_state(
    geometry: &FrozenQueueGeometry<FairV2IngressSource, FrozenFairIngressOccurrence>,
    selector_occurrences: &BTreeMap<u64, FairIngressSelectorOccurrence>,
) -> Result<(), FairIngressQueueCutError> {
    for occurrence in geometry.lanes.values().flat_map(|lane| lane.iter()) {
        let ordinal = occurrence.physical_admission_ordinal;
        let frozen = &occurrence.value;
        let inbound = selector_occurrences
            .get(&ordinal)
            .ok_or(FairIngressQueueCutError::InvalidOccurrenceIdentity)?
            .inbound();
        let live = inbound
            .ingress_ownership()
            .ok_or(FairIngressQueueCutError::InvalidOccurrenceIdentity)?;
        let snapshot = frozen.ownership_snapshot.as_ref();
        if !snapshot.validate_exact()
            || !live.validate_exact()
            || snapshot.process_local_projection_hash() != live.process_local_projection_hash()
            || !live.matches_message(inbound.message())
            || !live.matches_semantic_origin(inbound.sender())
            || !live.matches_reply_routes(inbound.reply_routes())
        {
            return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
        }
    }
    Ok(())
}
fn entry_storage_is_exact(
    state: &FairV2IngressState,
    source: &FairV2IngressSource,
    entry: &FairV2IngressEntry,
) -> bool {
    let Some(key) = entry.wire_key.as_ref() else {
        return false;
    };
    let expected_source = if state.roster.contains(entry.inbound.via()) {
        FairV2IngressSource::Validator(entry.inbound.via().clone())
    } else {
        FairV2IngressSource::Authenticated(entry.inbound.via().clone())
    };
    let Some(ownership) = entry.inbound.ingress_ownership() else {
        return false;
    };
    let snapshot = entry.ownership_snapshot.as_ref();
    entry.admission_ordinal != 0
        && entry.admission_ordinal <= state.last_admission_ordinal
        && *source == expected_source
        && entry.class == FairV2IngressClass::classify(&entry.inbound)
        && entry.encoded_len == entry.encoded_bytes.len()
        && &key.origin == entry.inbound.sender()
        && ownership.first.physical_admission_ordinal == entry.admission_ordinal
        && ownership.runtime_physical_cut.is_none()
        && ownership.leader_wire_runtime_receipt.is_none()
        && ownership.leader_wire_token.as_ref() == entry.leader_wire_token.as_ref()
        && ownership.first.wire_key == *key
        && Arc::ptr_eq(&ownership.first.encoded_bytes, &entry.encoded_bytes)
        && ownership.first.encoded_len == entry.encoded_len
        && ownership.first.class == entry.class
        && ownership.first.semantic_owner_source == *source
        && &ownership.first.semantic_origin == entry.inbound.sender()
        && snapshot.first.physical_admission_ordinal == entry.admission_ordinal
        && snapshot.runtime_physical_cut.is_none()
        && snapshot.leader_wire_runtime_receipt.is_none()
        && snapshot.leader_wire_token.as_ref() == entry.leader_wire_token.as_ref()
        && snapshot.first.wire_key == *key
        && Arc::ptr_eq(&snapshot.first.encoded_bytes, &entry.encoded_bytes)
        && snapshot.first.encoded_len == entry.encoded_len
        && snapshot.first.class == entry.class
        && snapshot.first.semantic_owner_source == *source
        && &snapshot.first.semantic_origin == entry.inbound.sender()
}
fn select_positions<S, V>(
    geometry: &FrozenQueueGeometry<S, V>,
    target_physical_ordinal: u64,
) -> Result<FairIngressQueuePositions, FairIngressQueueCutError> {
    if target_physical_ordinal == 0 {
        return Err(FairIngressQueueCutError::ZeroTargetOrdinal);
    }
    geometry
        .positions
        .get(&target_physical_ordinal)
        .copied()
        .ok_or(FairIngressQueueCutError::MissingTarget)
}
fn freeze_geometry<S, V>(
    ready: &[S],
    lanes: BTreeMap<S, Vec<FrozenQueueOccurrence<V>>>,
    physical_cut: u128,
) -> Result<FrozenQueueGeometry<S, V>, FairIngressQueueCutError>
where
    S: Clone + Ord,
    V: Clone + Eq,
{
    let mut unique_ready = BTreeSet::new();
    for source in ready {
        if !unique_ready.insert(source.clone()) {
            return Err(FairIngressQueueCutError::DuplicateReadySource);
        }
    }
    let mut ready_prefix = Vec::new();
    let mut frozen_lanes = BTreeMap::new();
    let mut positions = BTreeMap::new();
    let mut source_position = 0_u64;
    for source in ready {
        let lane = lanes
            .get(source)
            .ok_or(FairIngressQueueCutError::MissingReadyLane)?;
        if lane.is_empty() {
            return Err(FairIngressQueueCutError::MissingReadyLane);
        }
        source_position = source_position
            .checked_add(1)
            .ok_or(FairIngressQueueCutError::PositionOverflow)?;
        let source_position =
            NonZeroU64::new(source_position).ok_or(FairIngressQueueCutError::PositionOverflow)?;
        let mut frozen_lane = Vec::with_capacity(lane.len());
        for (lane_index, occurrence) in lane.iter().enumerate() {
            if occurrence.physical_admission_ordinal == 0
                || u128::from(occurrence.physical_admission_ordinal) >= physical_cut
            {
                return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
            }
            let lane_position = u64::try_from(lane_index)
                .ok()
                .and_then(|position| position.checked_add(1))
                .and_then(NonZeroU64::new)
                .ok_or(FairIngressQueueCutError::PositionOverflow)?;
            if positions
                .insert(
                    occurrence.physical_admission_ordinal,
                    FairIngressQueuePositions {
                        lane: lane_position,
                        source: source_position,
                    },
                )
                .is_some()
            {
                return Err(FairIngressQueueCutError::DuplicateAdmissionOrdinal);
            }
            frozen_lane.push(occurrence.clone());
        }
        ready_prefix.push(source.clone());
        frozen_lanes.insert(source.clone(), frozen_lane);
    }
    if lanes.keys().any(|source| !unique_ready.contains(source)) {
        return Err(FairIngressQueueCutError::ForeignReadyLane);
    }
    Ok(FrozenQueueGeometry {
        ready_prefix,
        lanes: frozen_lanes,
        positions,
    })
}
fn exact_occurrence_projection(
    source: &FairV2IngressSource,
    entry: &FairV2IngressEntry,
    queue_gate: FairV2IngressQueueGateVerdict,
    obsolete: bool,
) -> Result<FrozenFairIngressOccurrence, FairIngressQueueCutError> {
    let key = entry
        .wire_key
        .as_ref()
        .ok_or(FairIngressQueueCutError::InvalidOccurrenceIdentity)?;
    let ownership = entry
        .inbound
        .ingress_ownership()
        .ok_or(FairIngressQueueCutError::InvalidOccurrenceIdentity)?;
    if entry.admission_ordinal == 0
        || entry.encoded_len != entry.encoded_bytes.len()
        || ownership.first.physical_admission_ordinal != entry.admission_ordinal
        || ownership.runtime_physical_cut.is_some()
        || ownership.leader_wire_runtime_receipt.is_some()
        || ownership.leader_wire_token.as_ref() != entry.leader_wire_token.as_ref()
        || ownership.first.wire_key != *key
        || !Arc::ptr_eq(&ownership.first.encoded_bytes, &entry.encoded_bytes)
        || ownership.first.encoded_len != entry.encoded_len
        || ownership.first.semantic_owner_source != *source
    {
        return Err(FairIngressQueueCutError::InvalidOccurrenceIdentity);
    }
    Ok(FrozenFairIngressOccurrence {
        wire_key: key.clone(),
        encoded_hash: key.hash,
        encoded_len: entry.encoded_len,
        ownership_snapshot: Arc::clone(&entry.ownership_snapshot),
        source_class: source.class(),
        class: entry.class,
        leader_wire_token: entry.leader_wire_token.clone(),
        queue_gate,
        obsolete,
    })
}
fn frozen_projection_for_ordinal<S>(
    geometry: &FrozenQueueGeometry<S, FrozenFairIngressOccurrence>,
    physical_admission_ordinal: u64,
) -> Option<&FrozenFairIngressOccurrence>
where
    S: Ord,
{
    geometry
        .lanes
        .values()
        .flat_map(|lane| lane.iter())
        .find(|occurrence| occurrence.physical_admission_ordinal == physical_admission_ordinal)
        .map(|occurrence| &occurrence.value)
}
fn find_entry_by_physical_ordinal(
    state: &FairV2IngressState,
    physical_admission_ordinal: u64,
) -> Option<&FairV2IngressEntry> {
    state
        .lanes
        .values()
        .flat_map(|lane| lane.entries.iter())
        .find(|entry| entry.admission_ordinal == physical_admission_ordinal)
}
fn source_for_physical_ordinal(
    state: &FairV2IngressState,
    physical_admission_ordinal: u64,
) -> Option<&FairV2IngressSource> {
    state.lanes.iter().find_map(|(source, lane)| {
        lane.entries
            .iter()
            .any(|entry| entry.admission_ordinal == physical_admission_ordinal)
            .then_some(source)
    })
}
fn target_lifecycle_context(
    entry: &FairV2IngressEntry,
) -> Option<(wire::HeightContextId, wire::Height)> {
    let token_context = entry
        .leader_wire_token
        .as_ref()
        .map(|token| (token.identity.context_id, token.identity.height));
    let carrier_context = match entry.inbound.message() {
        BlockMessage::V2(message) if message.validate_version().is_ok() => match &message.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                Some((proposal.round.context_id, proposal.round.height))
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                Some((vote.round.context_id, vote.round.height))
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                Some((certificate.round.context_id, certificate.round.height))
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                Some((vote.round.context_id, vote.round.height))
            }
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
                Some((certificate.round.context_id, certificate.round.height))
            }
            wire::ConsensusMessageV2Payload::PayloadManifest(manifest) => {
                Some((manifest.round.context_id, manifest.round.height))
            }
            wire::ConsensusMessageV2Payload::PayloadChunk(_) => {
                let token = entry.leader_wire_token.as_ref()?;
                return (token.identity.phase == FairV2IngressLeaderWirePhase::Chunk)
                    .then_some((token.identity.context_id, token.identity.height));
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) => {
                Some((request.round.context_id, request.round.height))
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) => Some((
                response.manifest.round.context_id,
                response.manifest.round.height,
            )),
            wire::ConsensusMessageV2Payload::CommitCertificateRequest(request) => {
                Some((request.context_id, request.height))
            }
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => Some((
                response.certificate.round.context_id,
                response.certificate.round.height,
            )),
            wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => return None,
        },
        BlockMessage::V2(_) => return None,
        BlockMessage::KuraReplicaAdvert(_)
        | BlockMessage::LaneBlockProposal(_)
        | BlockMessage::LaneExecutablePayload(_)
        | BlockMessage::LaneBlockNewViewVote(_)
        | BlockMessage::LaneBlockNewViewCertificate(_)
        | BlockMessage::LaneBlockVote(_)
        | BlockMessage::LaneBlockQc(_)
        | BlockMessage::LaneBlockCertificate(_)
        | BlockMessage::LaneHistoricalRecoveryRequest(_)
        | BlockMessage::LaneHistoricalRecoveryResponse(_) => return None,
    };
    match (carrier_context, token_context) {
        (Some(carrier), Some(token)) if carrier != token => None,
        (Some(carrier), Some(_) | None) => Some(carrier),
        (None, _) => None,
    }
}
fn pending_identity(
    (context_id, height): (wire::HeightContextId, wire::Height),
    source: &FairV2IngressSource,
    occurrence: &FrozenFairIngressOccurrence,
    physical_admission_ordinal: u64,
) -> PendingFairIngressIdentity {
    let mut projection = Vec::new();
    projection.extend_from_slice(PENDING_INGRESS_IDENTITY_DOMAIN);
    append_field(&mut projection, &context_id.encode());
    projection.extend_from_slice(&height.to_le_bytes());
    super::super::fair_v2_ingress_append_source_identity(&mut projection, source);
    super::super::fair_v2_ingress_append_peer_identity(
        &mut projection,
        &occurrence.wire_key.origin,
    );
    projection.extend_from_slice(occurrence.wire_key.hash.as_ref());
    projection.extend_from_slice(occurrence.encoded_hash.as_ref());
    projection.extend_from_slice(
        &u64::try_from(occurrence.encoded_len)
            .expect("bounded ingress wire length fits u64")
            .to_le_bytes(),
    );
    projection.extend_from_slice(
        occurrence
            .ownership_snapshot
            .process_local_projection_hash()
            .as_ref(),
    );
    projection.push(match occurrence.source_class {
        FairV2IngressSourceClass::Validator => 0,
        FairV2IngressSourceClass::Authenticated => 1,
    });
    projection.push(match occurrence.class {
        FairV2IngressClass::Auxiliary => 0,
        FairV2IngressClass::Progress => 1,
        FairV2IngressClass::TransportCompletion => 2,
    });
    match occurrence.leader_wire_token.as_ref() {
        None => projection.push(0),
        Some(token) => {
            projection.push(1);
            append_field(&mut projection, &token.encode());
        }
    }
    projection.push(match occurrence.queue_gate {
        FairV2IngressQueueGateVerdict::Blocked => 0,
        FairV2IngressQueueGateVerdict::Strict => 1,
        FairV2IngressQueueGateVerdict::Dependency => 2,
    });
    projection.push(u8::from(occurrence.obsolete));
    projection.extend_from_slice(&physical_admission_ordinal.to_le_bytes());
    let hash = Hash::new(projection);
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(hash.as_ref());
    PendingFairIngressIdentity {
        context: lifecycle_context_from_wire((context_id, height)),
        digest: LifecycleDigest::new(digest),
        physical_admission_ordinal,
    }
}
fn lifecycle_context_from_wire(
    (context_id, height): (wire::HeightContextId, wire::Height),
) -> LifecycleContext {
    let mut context_digest = [0_u8; 32];
    context_digest.copy_from_slice(context_id.0.as_ref());
    LifecycleContext::new(LifecycleDigest::new(context_digest), height)
}
fn append_field(projection: &mut Vec<u8>, field: &[u8]) {
    projection.extend_from_slice(
        &u64::try_from(field.len())
            .expect("bounded pending-ingress identity field fits u64")
            .to_le_bytes(),
    );
    projection.extend_from_slice(field);
}
#[cfg(test)]
mod tests {
    use super::super::super::{FairV2IngressPushDisposition, InboundBlockMessage};
    use super::*;
    use iroha_crypto::{HashOf, KeyPair};
    use iroha_data_model::peer::PeerId;
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum Source {
        First,
        Second,
        Third,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Value(u8);
    fn occurrence(physical_admission_ordinal: u64, value: u8) -> FrozenQueueOccurrence<Value> {
        FrozenQueueOccurrence {
            physical_admission_ordinal,
            value: Value(value),
        }
    }
    fn freeze_before_cut(
        ready: &[Source],
        lanes: BTreeMap<Source, Vec<FrozenQueueOccurrence<Value>>>,
        physical_cut: u128,
    ) -> Result<FrozenQueueGeometry<Source, Value>, FairIngressQueueCutError> {
        let lanes = lanes
            .into_iter()
            .filter_map(|(source, lane)| {
                let lane = lane
                    .into_iter()
                    .filter(|entry| u128::from(entry.physical_admission_ordinal) < physical_cut)
                    .collect::<Vec<_>>();
                (!lane.is_empty()).then_some((source, lane))
            })
            .collect::<BTreeMap<_, _>>();
        let ready_prefix = ready
            .iter()
            .filter(|source| lanes.contains_key(*source))
            .copied()
            .collect::<Vec<_>>();
        freeze_geometry(&ready_prefix, lanes, physical_cut)
    }
    fn commit_certificate_request(
        context_id: wire::HeightContextId,
        height: wire::Height,
        requester: &PeerId,
        signature_byte: u8,
    ) -> BlockMessage {
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CommitCertificateRequest(
                wire::CommitCertificateRequest {
                    protocol_version: wire::PROTOCOL_VERSION,
                    network_id: crate::sumeragi::synthetic_network_id(
                        "lifecycle-ingress-position-test",
                    ),
                    context_id,
                    height,
                    requester: requester.clone(),
                    signature: vec![signature_byte],
                },
            ),
        ))
    }
    fn certified_body_response(
        context_id: wire::HeightContextId,
        height: wire::Height,
        responder: &PeerId,
    ) -> BlockMessage {
        let body = vec![0xA5];
        let payload_hash = Hash::new(&body);
        let manifest = wire::PayloadManifest {
            round: wire::ConsensusRound {
                context_id,
                height,
                view: 0,
            },
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"lifecycle-ingress-equal-response-block",
                )),
                payload_hash,
            },
            payload_size_bytes: 1,
            layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 2,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 1,
                max_chunk_count: 2,
            },
            chunk_hashes: vec![payload_hash; 2],
            chunk_root: Hash::new(payload_hash.as_ref()),
        };
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(wire::CertifiedBodyResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"lifecycle-ingress-equal-response-request",
                )),
                manifest,
                body,
                responder: responder.clone(),
                signature: vec![0x5A],
            }),
        ))
    }
    fn assert_same_v2_message(actual: &BlockMessage, expected: &BlockMessage) {
        let (BlockMessage::V2(actual), BlockMessage::V2(expected)) = (actual, expected) else {
            panic!("expected two Sumeragi V2 messages: {actual:?}, {expected:?}")
        };
        assert_eq!(actual, expected);
    }
    fn single_commit_request_ingress(
        signature_byte: u8,
    ) -> (FairV2Ingress, LifecycleContext, PeerId, BlockMessage, u64) {
        const HEIGHT: wire::Height = 17;
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle-ingress-atomic-commit-context",
        )));
        let peer = PeerId::from(KeyPair::random().public_key().clone());
        let ingress = FairV2Ingress::new(16, 1024 * 1024, 512 * 1024, 0, 0);
        ingress
            .configure_roster([peer.clone()])
            .expect("one validator lane fits the atomic commit fixture");
        ingress.state.lock().leader_wire_context = Some((context_id, HEIGHT));
        ingress.open().expect("open atomic commit fixture");
        let message = commit_certificate_request(context_id, HEIGHT, &peer, signature_byte);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                message.clone(),
                peer.clone(),
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let ordinal = ingress.state.lock().last_admission_ordinal;
        (
            ingress,
            lifecycle_context_from_wire((context_id, HEIGHT)),
            peer,
            message,
            ordinal,
        )
    }
    fn two_commit_request_ingress() -> (FairV2Ingress, u64, u64) {
        const HEIGHT: wire::Height = 19;
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle-ingress-explicit-physical-cut-context",
        )));
        let peer = PeerId::from(KeyPair::random().public_key().clone());
        let ingress = FairV2Ingress::new(16, 1024 * 1024, 512 * 1024, 0, 0);
        ingress
            .configure_roster([peer.clone()])
            .expect("one validator lane fits the explicit-cut fixture");
        ingress.state.lock().leader_wire_context = Some((context_id, HEIGHT));
        ingress.open().expect("open explicit-cut fixture");

        let mut ordinals = Vec::new();
        for signature_byte in [41, 42] {
            let message = commit_certificate_request(context_id, HEIGHT, &peer, signature_byte);
            assert!(matches!(
                ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                    message,
                    peer.clone(),
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
            ordinals.push(ingress.state.lock().last_admission_ordinal);
        }
        (ingress, ordinals[0], ordinals[1])
    }
    #[test]
    fn freezes_exact_one_based_lane_and_source_positions() {
        let ready = [Source::First, Source::Second, Source::Third];
        let lanes = BTreeMap::from([
            (Source::First, vec![occurrence(1, 10), occurrence(2, 11)]),
            (Source::Second, vec![occurrence(3, 12)]),
            (Source::Third, vec![occurrence(4, 13), occurrence(5, 14)]),
        ]);
        let frozen = freeze_geometry(&ready, lanes, 6).expect("valid exact queue cut");
        assert_eq!(frozen.positions[&1].components(), [1, 1]);
        assert_eq!(frozen.positions[&2].components(), [2, 1]);
        assert_eq!(frozen.positions[&3].components(), [1, 2]);
        assert_eq!(frozen.positions[&4].components(), [1, 3]);
        assert_eq!(frozen.positions[&5].components(), [2, 3]);
        assert_eq!(
            select_positions(&frozen, 0),
            Err(FairIngressQueueCutError::ZeroTargetOrdinal)
        );
        assert_eq!(
            select_positions(&frozen, 6),
            Err(FairIngressQueueCutError::MissingTarget)
        );
    }
    #[test]
    fn rejects_missing_foreign_and_duplicate_queue_rows() {
        let missing = freeze_geometry(
            &[Source::First, Source::Second],
            BTreeMap::from([(Source::First, vec![occurrence(1, 1)])]),
            2,
        );
        assert_eq!(missing, Err(FairIngressQueueCutError::MissingReadyLane));
        let foreign = freeze_geometry(
            &[Source::First],
            BTreeMap::from([
                (Source::First, vec![occurrence(1, 1)]),
                (Source::Second, vec![occurrence(2, 2)]),
            ]),
            3,
        );
        assert_eq!(foreign, Err(FairIngressQueueCutError::ForeignReadyLane));
        let duplicate_source = freeze_geometry(
            &[Source::First, Source::First],
            BTreeMap::from([(Source::First, vec![occurrence(1, 1)])]),
            2,
        );
        assert_eq!(
            duplicate_source,
            Err(FairIngressQueueCutError::DuplicateReadySource)
        );
        let duplicate_ordinal = freeze_geometry(
            &[Source::First, Source::Second],
            BTreeMap::from([
                (Source::First, vec![occurrence(1, 1)]),
                (Source::Second, vec![occurrence(1, 2)]),
            ]),
            2,
        );
        assert_eq!(
            duplicate_ordinal,
            Err(FairIngressQueueCutError::DuplicateAdmissionOrdinal)
        );
    }
    #[test]
    fn duplicate_carrier_values_remain_distinct_by_physical_ordinal() {
        let frozen = freeze_geometry(
            &[Source::First, Source::Second],
            BTreeMap::from([
                (Source::First, vec![occurrence(1, 7)]),
                (Source::Second, vec![occurrence(2, 7)]),
            ]),
            3,
        )
        .expect("physical ordinals distinguish equal carrier values");
        assert_eq!(frozen.positions[&1].components(), [1, 1]);
        assert_eq!(frozen.positions[&2].components(), [1, 2]);
    }
    #[test]
    fn shared_selector_keeps_strict_dependency_blocked_and_obsolete_ordering() {
        #[derive(Clone, Copy)]
        struct Candidate {
            ordinal: u64,
            gate: FairV2IngressQueueGateVerdict,
            obsolete: bool,
            allowed: bool,
        }

        let candidates = vec![
            vec![
                Candidate {
                    ordinal: 1,
                    gate: FairV2IngressQueueGateVerdict::Dependency,
                    obsolete: false,
                    allowed: true,
                },
                Candidate {
                    ordinal: 2,
                    gate: FairV2IngressQueueGateVerdict::Blocked,
                    obsolete: true,
                    allowed: true,
                },
            ],
            vec![Candidate {
                ordinal: 3,
                gate: FairV2IngressQueueGateVerdict::Strict,
                obsolete: false,
                allowed: true,
            }],
        ];
        assert_eq!(
            select_fair_v2_ingress_candidate(
                &candidates,
                |candidate| (candidate.ordinal, candidate.gate, candidate.obsolete),
                |candidate| candidate.allowed,
            ),
            Some((1, 3, FairV2IngressDequeueDisposition::Admit)),
            "a later strict candidate wins before an earlier dependency"
        );

        let dependency_only = vec![
            vec![candidates[0][0]],
            vec![Candidate {
                allowed: false,
                ..candidates[1][0]
            }],
        ];
        assert_eq!(
            select_fair_v2_ingress_candidate(
                &dependency_only,
                |candidate| (candidate.ordinal, candidate.gate, candidate.obsolete),
                |candidate| candidate.allowed,
            ),
            Some((0, 1, FairV2IngressDequeueDisposition::Admit)),
            "dependency is considered only after the strict set rejects"
        );

        let obsolete = vec![vec![Candidate {
            ordinal: 4,
            gate: FairV2IngressQueueGateVerdict::Strict,
            obsolete: true,
            allowed: false,
        }]];
        assert_eq!(
            select_fair_v2_ingress_candidate(
                &obsolete,
                |candidate| (candidate.ordinal, candidate.gate, candidate.obsolete),
                |_| false,
            ),
            Some((0, 4, FairV2IngressDequeueDisposition::RetireObsolete)),
            "obsolete retirement bypasses a false downstream predicate"
        );

        let blocked = vec![vec![candidates[0][1]]];
        assert_eq!(
            select_fair_v2_ingress_candidate(
                &blocked,
                |candidate| (candidate.ordinal, candidate.gate, candidate.obsolete),
                |_| true,
            ),
            None,
            "a blocked control or Serve carrier never crosses its queue gate"
        );
    }

    #[test]
    fn explicit_turn_cut_selects_an_exact_pre_cut_occurrence() {
        let (ingress, first_ordinal, second_ordinal) = two_commit_request_ingress();
        let physical_cut = ingress.next_physical_admission_ordinal();

        let cut = ingress
            .capture_next_ingress_turn_cut_before(physical_cut, |occurrence| {
                occurrence.physical_admission_ordinal() == second_ordinal
            })
            .expect("capture a valid explicit physical cut")
            .expect("the exact pre-cut occurrence is eligible");

        assert_eq!(cut.physical_cut, physical_cut);
        assert_eq!(
            cut.selected_occurrence().physical_admission_ordinal(),
            second_ordinal,
            "an unrelated earlier row cannot consume the exact-predicate scan"
        );
        drop(cut);
        assert_eq!(ingress.len(), 2, "cut capture is read-only");
        let ordinary = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("ordinary recapture remains structurally valid")
            .expect("the earlier generic row remains queued");
        assert_eq!(
            ordinary.selected_occurrence().physical_admission_ordinal(),
            first_ordinal
        );
    }

    #[test]
    fn explicit_turn_cut_excludes_post_cut_occurrences_from_the_predicate() {
        let (ingress, _, peer, first_message, first_ordinal) = single_commit_request_ingress(40);
        let physical_cut = ingress.next_physical_admission_ordinal();
        let (context_id, height) = match &first_message {
            BlockMessage::V2(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::CommitCertificateRequest(request),
                ..
            }) => (request.context_id, request.height),
            _ => panic!("explicit-cut fixture must retain a commit-certificate request"),
        };
        let post_cut_message = commit_certificate_request(context_id, height, &peer, 43);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                post_cut_message,
                peer,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let second_ordinal = ingress.state.lock().last_admission_ordinal;
        assert_eq!(u128::from(second_ordinal), physical_cut);
        let mut tested = Vec::new();

        let selected = ingress
            .capture_next_ingress_turn_cut_before(physical_cut, |occurrence| {
                let ordinal = occurrence.physical_admission_ordinal();
                tested.push(ordinal);
                ordinal == second_ordinal
            })
            .expect("capture a valid explicit physical cut");

        assert!(selected.is_none(), "the post-cut match remains invisible");
        assert_eq!(tested, vec![first_ordinal]);
        assert_eq!(ingress.len(), 2);

        let (post_cut_only, _, _) = two_commit_request_ingress();
        assert!(
            post_cut_only
                .capture_next_ingress_turn_cut_before(1, |_| true)
                .expect("the initial exclusion boundary is a valid cut")
                .is_none()
        );
        assert!(
            post_cut_only.state.lock().last_service_attempt_at.is_none(),
            "traffic wholly after the frozen cut cannot claim a service attempt"
        );
    }

    #[test]
    fn explicit_turn_cut_rejects_zero_and_future_cuts() {
        let (ingress, _, _) = two_commit_request_ingress();
        let live_physical_cut = ingress.next_physical_admission_ordinal();

        assert!(matches!(
            ingress.capture_next_ingress_turn_cut_before(0, |_| true),
            Err(FairIngressQueueCutError::InvalidPhysicalCut)
        ));
        assert!(matches!(
            ingress.capture_next_ingress_turn_cut_before(live_physical_cut + 1, |_| true),
            Err(FairIngressQueueCutError::InvalidPhysicalCut)
        ));
        assert_eq!(ingress.len(), 2);
    }

    #[test]
    fn explicit_turn_cut_false_predicate_preserves_queue_geometry() {
        let (ingress, first_ordinal, second_ordinal) = two_commit_request_ingress();
        let physical_cut = u128::from(second_ordinal);
        let before = {
            let state = ingress.state.lock();
            (state.len, state.ready.iter().cloned().collect::<Vec<_>>())
        };

        assert!(
            ingress
                .capture_next_ingress_turn_cut_before(physical_cut, |_| false)
                .expect("a false predicate is a valid explicit-cut scan")
                .is_none()
        );
        let after = {
            let state = ingress.state.lock();
            (state.len, state.ready.iter().cloned().collect::<Vec<_>>())
        };
        assert_eq!(
            after, before,
            "a rejected scan cannot remove or rotate work"
        );

        let retained = ingress
            .capture_next_ingress_turn_cut_before(physical_cut, |_| true)
            .expect("recapture the same frozen prefix")
            .expect("the original fair winner remains queued");
        assert_eq!(
            retained.selected_occurrence().physical_admission_ordinal(),
            first_ordinal
        );
    }

    #[test]
    fn turn_cut_dequeues_exact_winner_once_and_preserves_ready_rotation() {
        const HEIGHT: wire::Height = 13;
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle-ingress-turn-cut-rotation",
        )));
        let first = PeerId::from(KeyPair::random().public_key().clone());
        let second = PeerId::from(KeyPair::random().public_key().clone());
        let ingress = FairV2Ingress::new(16, 3 * 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster([first.clone(), second.clone()])
            .expect("configure two exact validator lanes");
        ingress.state.lock().leader_wire_context = Some((context_id, HEIGHT));
        ingress.open().expect("open turn-cut rotation ingress");
        let first_winner = commit_certificate_request(context_id, HEIGHT, &first, 1);
        for (message, source) in [
            (first_winner.clone(), first.clone()),
            (
                commit_certificate_request(context_id, HEIGHT, &first, 2),
                first.clone(),
            ),
            (
                commit_certificate_request(context_id, HEIGHT, &second, 3),
                second.clone(),
            ),
        ] {
            assert!(matches!(
                ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                    message, source
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
        }

        let first_cut = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("capture exact first fair winner")
            .expect("first fair winner exists");
        assert_eq!(
            first_cut.selected_occurrence().physical_admission_ordinal(),
            1
        );
        let first_cut = match first_cut
            .narrow_to_lifecycle(lifecycle_context_from_wire((context_id, HEIGHT)))
            .unwrap_or_else(|_| panic!("narrow the exact current-context winner"))
        {
            FairIngressTurnContextCut::Lifecycle(cut) => cut.into_ordinary_turn_cut(),
            FairIngressTurnContextCut::Ordinary(_) => {
                panic!("matching commit request must narrow to this lifecycle")
            }
        };
        let (drained, disposition) = first_cut
            .dequeue_exact_retaining()
            .unwrap_or_else(|_| panic!("dequeue exact first winner"));
        assert_same_v2_message(drained.message(), &first_winner);
        assert_eq!(disposition, FairV2IngressDequeueDisposition::Admit);
        assert_eq!(
            drained
                .ingress_ownership()
                .and_then(FairV2IngressOwnershipEvidence::runtime_physical_cut),
            Some(4)
        );
        let (depth, ready) = {
            let state = ingress.state.lock();
            (state.len, state.ready.iter().cloned().collect::<Vec<_>>())
        };
        assert_eq!(depth, 2);
        assert_eq!(
            ready,
            vec![
                FairV2IngressSource::Validator(second.clone()),
                FairV2IngressSource::Validator(first.clone()),
            ]
        );

        assert!(
            ingress
                .capture_next_ingress_turn_cut(|_| false)
                .expect("a false pure predicate is a valid retained turn")
                .is_none()
        );
        let rejected_attempt = ingress
            .last_selector_attempt
            .lock()
            .expect("the composite selector publishes its scalar attempt");
        assert_eq!(rejected_attempt.depth, depth);
        assert!(rejected_attempt.gate_strict > 0);
        assert_eq!(
            rejected_attempt.predicate_rejected,
            rejected_attempt.predicate_tested
        );
        assert!(rejected_attempt.predicate_tested > 0);
        assert!(!rejected_attempt.selected);
        let state = ingress.state.lock();
        assert_eq!(state.len, depth);
        assert_eq!(state.ready.iter().cloned().collect::<Vec<_>>(), ready);
        drop(state);

        let rotated = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("capture winner after ready-source rotation")
            .expect("rotated fair winner exists");
        assert_eq!(
            rotated.selected_occurrence().physical_admission_ordinal(),
            3
        );
        let (rotated, _) = rotated
            .dequeue_exact_retaining()
            .unwrap_or_else(|_| panic!("dequeue rotated exact winner"));
        assert_eq!(
            rotated
                .ingress_ownership()
                .and_then(FairV2IngressOwnershipEvidence::physical_admission_ordinal),
            Some(3)
        );
        assert_eq!(ingress.len(), 1);
    }

    #[test]
    fn turn_cut_reports_retryable_change_when_selected_carrier_coalesces_before_narrowing() {
        let (ingress, context, peer, message, ordinal) = single_commit_request_ingress(27);
        let cut = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("capture exact current-context turn")
            .expect("current-context winner exists");
        assert_eq!(
            cut.selected_occurrence().physical_admission_ordinal(),
            ordinal
        );

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(message, peer,)),
            Ok(FairV2IngressPushDisposition::Coalesced)
        ));
        let retained = match cut.narrow_to_lifecycle(context) {
            Err((FairIngressQueueCutError::QueueCutChanged, retained)) => retained,
            Err((_error, _retained)) => {
                panic!("coalesced ownership must report a retryable queue-cut change")
            }
            Ok(_) => panic!("coalesced ownership cannot retain the stale frozen cut"),
        };
        drop(retained);
        assert_eq!(ingress.len(), 1, "retry cannot consume the selected row");

        let recaptured = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("recapture coalesced current-context turn")
            .expect("coalesced winner remains available");
        assert!(matches!(
            recaptured.narrow_to_lifecycle(context),
            Ok(FairIngressTurnContextCut::Lifecycle(_))
        ));
        assert_eq!(ingress.len(), 1, "narrowing alone cannot dequeue the row");
    }

    #[test]
    fn fenced_turn_refreshes_coalescence_and_transfers_guards_to_exact_dequeue() {
        let (ingress, context, peer, message, ordinal) = single_commit_request_ingress(29);
        let cut = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("capture current-context turn before ownership refresh")
            .expect("current-context winner exists");
        let ownership_before = cut
            .selected_occurrence()
            .inbound()
            .ingress_ownership()
            .expect("selected ingress retains ownership")
            .process_local_projection_hash();

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                message.clone(),
                peer.clone(),
            )),
            Ok(FairV2IngressPushDisposition::Coalesced)
        ));
        let cut = cut
            .fence_producer_publication_retaining()
            .unwrap_or_else(|(error, _)| panic!("refresh coalesced turn under fence: {error:?}"));
        let ownership_after = cut
            .selected_occurrence()
            .inbound()
            .ingress_ownership()
            .expect("refreshed ingress retains ownership")
            .process_local_projection_hash();
        assert_ne!(
            ownership_before, ownership_after,
            "the fenced cut must retain the latest coalesced ownership"
        );
        assert!(ingress.service_lock.try_lock().is_none());
        assert!(ingress.producer_publication_lock.try_lock().is_none());

        let cut = match cut.narrow_to_lifecycle(context) {
            Ok(FairIngressTurnContextCut::Lifecycle(cut)) => cut,
            Ok(FairIngressTurnContextCut::Ordinary(_)) => {
                panic!("refreshed current-context turn must remain lifecycle work")
            }
            Err((error, _)) => panic!("fenced refreshed turn must narrow exactly: {error:?}"),
        };
        assert_eq!(
            cut.selected_disposition(),
            FairV2IngressDequeueDisposition::Admit
        );
        let locked = cut
            .into_locked_exact_dequeue_retaining(context, ordinal)
            .unwrap_or_else(|(error, _)| panic!("direct fenced dequeue must lock: {error:?}"));
        assert!(ingress.service_lock.try_lock().is_none());
        assert!(ingress.producer_publication_lock.try_lock().is_none());

        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let ingress = &ingress;
            scope.spawn(move || {
                started_tx
                    .send(())
                    .expect("fenced retransmission start receiver remains live");
                let result =
                    ingress.try_push(InboundBlockMessage::from_authenticated_peer(message, peer));
                result_tx
                    .send(result)
                    .expect("fenced retransmission result receiver remains live");
            });
            started_rx
                .recv()
                .expect("retransmission reaches the retained producer fence");
            std::thread::yield_now();
            assert!(matches!(
                result_rx.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ));

            let (dequeued, disposition) = locked.commit();
            assert_eq!(disposition, FairV2IngressDequeueDisposition::Admit);
            drop(dequeued);
            assert!(matches!(
                result_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("retransmission resumes after direct exact dequeue"),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
        });
        assert_eq!(ingress.len(), 1);
        assert!(ingress.state.lock().last_admission_ordinal > ordinal);
    }

    #[test]
    fn lifecycle_cut_detects_selected_carrier_coalescence_during_selector_capture() {
        let (ingress, context, peer, message, ordinal) = single_commit_request_ingress(28);
        let turn = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("capture exact current-context turn")
            .expect("current-context winner exists");
        assert_eq!(
            turn.selected_occurrence().physical_admission_ordinal(),
            ordinal
        );
        let cut = match turn.narrow_to_lifecycle(context) {
            Ok(FairIngressTurnContextCut::Lifecycle(cut)) => cut,
            Ok(FairIngressTurnContextCut::Ordinary(_)) => {
                panic!("current-context winner must narrow to lifecycle ownership")
            }
            Err((_error, _retained)) => panic!("unchanged turn cut must narrow exactly"),
        };
        assert!(cut.pre_cut_is_intact());

        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(message, peer,)),
            Ok(FairV2IngressPushDisposition::Coalesced)
        ));
        assert!(
            !cut.pre_cut_is_intact(),
            "selector capture must reject a pre-cut ownership refresh"
        );
        drop(cut);
        assert_eq!(ingress.len(), 1, "selector retry cannot consume the row");

        let recaptured = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("recapture coalesced current-context turn")
            .expect("coalesced winner remains available");
        assert!(matches!(
            recaptured.narrow_to_lifecycle(context),
            Ok(FairIngressTurnContextCut::Lifecycle(_))
        ));
        assert_eq!(ingress.len(), 1, "stable recapture cannot dequeue the row");
    }

    #[test]
    fn direct_locked_conversion_retains_unfenced_cut_after_coalescence() {
        let (ingress, context, peer, message, ordinal) = single_commit_request_ingress(30);
        let turn = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("capture unfenced current-context turn")
            .expect("current-context winner exists");
        let cut = match turn.narrow_to_lifecycle(context) {
            Ok(FairIngressTurnContextCut::Lifecycle(cut)) => cut,
            Ok(FairIngressTurnContextCut::Ordinary(_)) => {
                panic!("current-context winner must narrow to lifecycle ownership")
            }
            Err((error, _)) => panic!("unchanged turn must narrow exactly: {error:?}"),
        };
        let foreign_context =
            LifecycleContext::new(LifecycleDigest::new([0xA5; 32]), context.height());
        let cut = match cut.into_locked_exact_dequeue_retaining(foreign_context, ordinal) {
            Err((FairIngressQueueCutError::ForeignCommitContext, retained)) => retained,
            Err((error, _)) => panic!("wrong direct-lock context returned {error:?}"),
            Ok(_) => panic!("wrong context cannot enter the direct exact dequeue"),
        };
        let cut = match cut.into_locked_exact_dequeue_retaining(context, ordinal + 1) {
            Err((FairIngressQueueCutError::ForeignCommitOrdinal, retained)) => retained,
            Err((error, _)) => panic!("wrong direct-lock ordinal returned {error:?}"),
            Ok(_) => panic!("wrong ordinal cannot enter the direct exact dequeue"),
        };
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(message, peer)),
            Ok(FairV2IngressPushDisposition::Coalesced)
        ));

        let retained = match cut.into_locked_exact_dequeue_retaining(context, ordinal) {
            Err((FairIngressQueueCutError::QueueCutChanged, retained)) => retained,
            Err((error, _)) => panic!("coalesced direct lock returned structural error: {error:?}"),
            Ok(_) => panic!("unfenced stale cut cannot enter the direct exact dequeue"),
        };
        drop(retained);
        assert_eq!(
            ingress.len(),
            1,
            "rejected direct lock cannot dequeue the row"
        );
        assert!(ingress.service_lock.try_lock().is_some());
        assert!(ingress.producer_publication_lock.try_lock().is_some());
    }

    #[test]
    fn foreign_winner_dequeues_as_ordinary_without_reselection() {
        const HEIGHT: wire::Height = 31;
        let bound_context = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle-ingress-bound-context",
        )));
        let foreign_context = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle-ingress-foreign-context",
        )));
        let peer = PeerId::from(KeyPair::random().public_key().clone());
        let ingress = FairV2Ingress::new(7, 1024 * 1024, 512 * 1024, 0, 0);
        ingress
            .configure_roster([peer.clone()])
            .expect("configure foreign-winner lane");
        ingress.state.lock().leader_wire_context = Some((bound_context, HEIGHT));
        ingress.open().expect("open foreign-winner ingress");
        let message = commit_certificate_request(foreign_context, HEIGHT, &peer, 9);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                message.clone(),
                peer
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));

        let cut = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("capture exact foreign winner")
            .expect("foreign winner exists");
        let cut = match cut
            .narrow_to_lifecycle(lifecycle_context_from_wire((bound_context, HEIGHT)))
            .unwrap_or_else(|_| panic!("foreign winner remains a valid ordinary cut"))
        {
            FairIngressTurnContextCut::Ordinary(cut) => cut,
            FairIngressTurnContextCut::Lifecycle(_) => {
                panic!("foreign carrier cannot enter the bound lifecycle census")
            }
        };
        let (drained, _) = cut
            .dequeue_exact_retaining()
            .unwrap_or_else(|_| panic!("dequeue exact foreign winner"));
        assert_same_v2_message(drained.message(), &message);
        assert_eq!(ingress.len(), 0);
    }

    #[test]
    fn ordinary_head_ignores_later_unowned_invalid_response() {
        const HEIGHT: wire::Height = 37;
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle-ingress-ordinary-before-invalid-response",
        )));
        let peer = PeerId::from(KeyPair::random().public_key().clone());
        let ingress = FairV2Ingress::new(8, 2 * 1024 * 1024, 1024 * 1024, 0, 512 * 1024);
        ingress
            .configure_roster([peer.clone()])
            .expect("configure ordinary-head lane");
        ingress.state.lock().leader_wire_context = Some((context_id, HEIGHT));
        ingress.open().expect("open ordinary-head ingress");
        let ordinary = commit_certificate_request(context_id, HEIGHT, &peer, 1);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                ordinary.clone(),
                peer.clone(),
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let mut invalid_response = certified_body_response(context_id, HEIGHT, &peer);
        let BlockMessage::V2(response_message) = &mut invalid_response else {
            unreachable!("response fixture uses the v2 carrier")
        };
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) =
            &mut response_message.payload
        else {
            unreachable!("response fixture retains its payload class")
        };
        response.signature.clear();
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                invalid_response.clone(),
                peer.clone(),
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));

        let cut = ingress
            .capture_next_ingress_turn_cut(|_| true)
            .expect("later malformed family cannot poison queue selection")
            .expect("ordinary head remains selected");
        assert_eq!(cut.selected_occurrence().physical_admission_ordinal(), 1);
        let (drained, _) = cut
            .dequeue_exact_retaining()
            .unwrap_or_else(|_| panic!("dequeue exact ordinary head"));
        assert_same_v2_message(drained.message(), &ordinary);
        assert_eq!(ingress.len(), 1);
        let retained = ingress
            .try_recv()
            .expect("later invalid response remains queued");
        assert_same_v2_message(retained.message(), &invalid_response);
    }

    #[test]
    fn equal_response_hashes_receive_distinct_queue_minted_identities() {
        const HEIGHT: wire::Height = 11;
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle-ingress-equal-response-context",
        )));
        let first = PeerId::from(KeyPair::random().public_key().clone());
        let second = PeerId::from(KeyPair::random().public_key().clone());
        let ingress = FairV2Ingress::new(16, 2 * 1024 * 1024, 512 * 1024, 0, 512 * 1024);
        ingress
            .configure_roster([first.clone(), second.clone()])
            .expect("two validator lanes fit the identity test queue");
        ingress.state.lock().leader_wire_context = Some((context_id, HEIGHT));
        ingress.open().expect("open identity test ingress");
        let response = certified_body_response(context_id, HEIGHT, &first);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                response.clone(),
                first.clone(),
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let first_ordinal = ingress.state.lock().last_admission_ordinal;
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                response.clone(),
                second.clone(),
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let second_ordinal = ingress.state.lock().last_admission_ordinal;
        let cut = ingress
            .capture_lifecycle_queue_cut(first_ordinal)
            .expect("capture both equal-response occurrences");
        let first_identity = cut
            .identity_for_ordinal(first_ordinal)
            .expect("first response has a queue identity");
        let second_identity = cut
            .identity_for_ordinal(second_ordinal)
            .expect("second response has a queue identity");
        assert_ne!(first_ordinal, second_ordinal);
        assert_ne!(first_identity, second_identity);
        assert_ne!(first_identity.digest(), second_identity.digest());
        assert_eq!(first_identity, cut.selected_identity());
        let response_hashes = cut
            .selector_occurrences()
            .map(|occurrence| match occurrence.inbound().message() {
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
                    ..
                }) => HashOf::new(response),
                _ => panic!("identity fixture must retain certified responses"),
            })
            .collect::<Vec<_>>();
        assert_eq!(response_hashes.len(), 2);
        assert_eq!(response_hashes[0], response_hashes[1]);
        assert!(cut.pre_cut_is_intact());
        drop(cut);

        let rotated = FairV2Ingress::new(16, 3 * 1024 * 1024, 1024 * 1024, 0, 512 * 1024);
        rotated
            .configure_roster([first.clone(), second.clone()])
            .expect("two validator lanes fit the fair-selection rotation queue");
        rotated.state.lock().leader_wire_context = Some((context_id, HEIGHT));
        rotated.open().expect("open fair-selection rotation queue");
        let first_message = commit_certificate_request(context_id, HEIGHT, &first, 1);
        let second_same_source = commit_certificate_request(context_id, HEIGHT, &first, 2);
        for (message, source) in [
            (first_message, first.clone()),
            (second_same_source, first.clone()),
            (
                commit_certificate_request(context_id, HEIGHT, &second, 3),
                second.clone(),
            ),
        ] {
            assert!(matches!(
                rotated.try_push(InboundBlockMessage::from_authenticated_peer(
                    message, source
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
        }
        let initial = rotated
            .capture_next_lifecycle_queue_cut(|_| true)
            .expect("read-only fair selection freezes the initial queue")
            .expect("initial fair winner exists");
        assert_eq!(initial.selected_identity().physical_admission_ordinal(), 1);
        drop(initial);
        let drained = rotated
            .try_recv_if_checked(|_| true)
            .expect("ordinary checked dequeue uses the same initial winner")
            .expect("initial winner remains queued");
        assert_eq!(
            drained
                .ingress_ownership()
                .expect("dequeued carrier retains ingress ownership")
                .first
                .physical_admission_ordinal,
            1,
        );
        let after_rotation = rotated
            .capture_next_lifecycle_queue_cut(|_| true)
            .expect("read-only fair selection freezes the rotated queue")
            .expect("rotated fair winner exists");
        assert_eq!(
            after_rotation
                .selected_identity()
                .physical_admission_ordinal(),
            3,
            "ready-source rotation wins over the lower remaining physical ordinal",
        );
        assert_eq!(rotated.len(), 2, "selection cannot dequeue either survivor");
    }
    #[test]
    fn post_cut_append_preserves_geometry_but_pre_cut_mutation_fails_cas() {
        let ready = [Source::First, Source::Second];
        let initial = BTreeMap::from([
            (Source::First, vec![occurrence(1, 1), occurrence(2, 2)]),
            (Source::Second, vec![occurrence(3, 3)]),
        ]);
        let frozen = freeze_geometry(&ready, initial, 4).expect("valid initial cut");
        let appended_ready = [Source::First, Source::Second, Source::Third];
        let appended = BTreeMap::from([
            (
                Source::First,
                vec![occurrence(1, 1), occurrence(2, 2), occurrence(4, 4)],
            ),
            (Source::Second, vec![occurrence(3, 3)]),
            (Source::Third, vec![occurrence(5, 5)]),
        ]);
        assert_eq!(
            freeze_before_cut(&appended_ready, appended, 4).expect("appends are beyond cut"),
            frozen
        );
        let mutated = BTreeMap::from([
            (Source::First, vec![occurrence(2, 2), occurrence(1, 1)]),
            (Source::Second, vec![occurrence(3, 3)]),
        ]);
        assert_ne!(
            freeze_geometry(&ready, mutated, 4).expect("mutation is structurally representable"),
            frozen
        );
    }
    #[test]
    fn live_cut_binds_context_owner_and_projection_across_append_and_coalescence() {
        const SOURCE_BYTES: usize = 1024 * 1024;
        const HEIGHT: wire::Height = 9;
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle-ingress-position-context",
        )));
        let first = PeerId::from(KeyPair::random().public_key().clone());
        let second = PeerId::from(KeyPair::random().public_key().clone());
        let ingress = FairV2Ingress::new(16, 3 * SOURCE_BYTES, SOURCE_BYTES, 0, 0);
        ingress
            .configure_roster([first.clone(), second.clone()])
            .expect("two validator lanes fit the test queue");
        ingress.state.lock().leader_wire_context = Some((context_id, HEIGHT));
        ingress.open().expect("open test ingress");
        let target_message = commit_certificate_request(context_id, HEIGHT, &first, 1);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                target_message.clone(),
                first.clone(),
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let target_ordinal = ingress.state.lock().last_admission_ordinal;
        let cut = ingress
            .capture_lifecycle_queue_cut(target_ordinal)
            .expect("capture exact live queue cut");
        let initial_digest = cut.selected_identity().digest();
        let mut expected_context_digest = [0_u8; 32];
        expected_context_digest.copy_from_slice(context_id.0.as_ref());
        assert_eq!(
            cut.selected_identity().context(),
            LifecycleContext::new(LifecycleDigest::new(expected_context_digest), HEIGHT)
        );
        assert_eq!(
            cut.selected_identity().physical_admission_ordinal(),
            target_ordinal
        );
        assert_eq!(cut.selected_positions().components(), [1, 1]);
        assert_eq!(cut.physical_cut(), u128::from(target_ordinal) + 1);
        let selector_rows = cut.selector_occurrences().collect::<Vec<_>>();
        assert_eq!(selector_rows.len(), 1);
        let selector_row = selector_rows[0];
        assert_eq!(selector_row.physical_admission_ordinal(), target_ordinal);
        assert_eq!(
            selector_row.context(),
            Some(cut.selected_identity().context())
        );
        assert_eq!(
            selector_row.source_class(),
            FairV2IngressSourceClass::Validator
        );
        assert_eq!(selector_row.class(), FairV2IngressClass::Progress);
        assert_eq!(
            selector_row.queue_gate(),
            FairV2IngressQueueGateVerdict::Strict
        );
        assert!(!selector_row.is_obsolete());
        assert_same_v2_message(selector_row.inbound().message(), &target_message);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                commit_certificate_request(context_id, HEIGHT, &second, 2),
                second,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(
            cut.pre_cut_is_intact(),
            "a newly ready source at the physical cut cannot change the frozen prefix"
        );
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                target_message,
                first
            )),
            Ok(FairV2IngressPushDisposition::Coalesced)
        ));
        assert!(
            !cut.pre_cut_is_intact(),
            "same-wire ownership history mutation must fail the queue-cut CAS"
        );
        drop(cut);
        let recaptured = ingress
            .capture_lifecycle_queue_cut(target_ordinal)
            .expect("recapture mutated ownership projection");
        assert_ne!(
            recaptured.selected_identity().digest(),
            initial_digest,
            "the selected join digest binds the complete ownership projection"
        );
        let original_class = {
            let mut state = ingress.state.lock();
            let entry = state
                .lanes
                .values_mut()
                .flat_map(|lane| lane.entries.iter_mut())
                .find(|entry| entry.admission_ordinal == target_ordinal)
                .expect("target remains queued while the service cut is held");
            let original = entry.class;
            entry.class = match original {
                FairV2IngressClass::Auxiliary => FairV2IngressClass::Progress,
                FairV2IngressClass::Progress | FairV2IngressClass::TransportCompletion => {
                    FairV2IngressClass::Auxiliary
                }
            };
            original
        };
        assert!(
            !recaptured.pre_cut_is_intact(),
            "changing a pre-cut occurrence class must invalidate the cut"
        );
        ingress
            .state
            .lock()
            .lanes
            .values_mut()
            .flat_map(|lane| lane.entries.iter_mut())
            .find(|entry| entry.admission_ordinal == target_ordinal)
            .expect("target remains queued while restoring its test class")
            .class = original_class;
        assert!(recaptured.pre_cut_is_intact());
        let original_encoded_len = {
            let mut state = ingress.state.lock();
            let entry = state
                .lanes
                .values_mut()
                .flat_map(|lane| lane.entries.iter_mut())
                .find(|entry| entry.admission_ordinal == target_ordinal)
                .expect("target remains queued while mutating its stored length");
            let original = entry.encoded_len;
            entry.encoded_len = original
                .checked_add(1)
                .expect("test wire length increment fits usize");
            original
        };
        assert!(
            !recaptured.pre_cut_is_intact(),
            "stored wire length mutation must fail release-mode structure validation"
        );
        ingress
            .state
            .lock()
            .lanes
            .values_mut()
            .flat_map(|lane| lane.entries.iter_mut())
            .find(|entry| entry.admission_ordinal == target_ordinal)
            .expect("target remains queued while restoring its stored length")
            .encoded_len = original_encoded_len;
        assert!(recaptured.pre_cut_is_intact());
        let removed_key = {
            let mut state = ingress.state.lock();
            let lane = state
                .lanes
                .values_mut()
                .find(|lane| {
                    lane.entries
                        .iter()
                        .any(|entry| entry.admission_ordinal == target_ordinal)
                })
                .expect("target lane remains queued while mutating its wire index");
            let key = lane
                .entries
                .iter()
                .find(|entry| entry.admission_ordinal == target_ordinal)
                .and_then(|entry| entry.wire_key.clone())
                .expect("target retains one wire key");
            assert!(lane.pending_wire.remove(&key));
            key
        };
        assert!(
            !recaptured.pre_cut_is_intact(),
            "pending-wire index mutation must fail release-mode structure validation"
        );
        {
            let mut state = ingress.state.lock();
            let lane = state
                .lanes
                .values_mut()
                .find(|lane| {
                    lane.entries
                        .iter()
                        .any(|entry| entry.admission_ordinal == target_ordinal)
                })
                .expect("target lane remains queued while restoring its wire index");
            assert!(lane.pending_wire.insert(removed_key));
        }
        assert!(recaptured.pre_cut_is_intact());
        ingress.state.lock().requires_leader_wire_lifecycle_gate = true;
        assert!(
            !recaptured.pre_cut_is_intact(),
            "a newly required but unbound leader-wire authority must invalidate the cut"
        );
        ingress.state.lock().requires_leader_wire_lifecycle_gate = false;
    }
    #[test]
    fn prepared_commit_uses_production_accounting_rotation_and_physical_cut() {
        const HEIGHT: wire::Height = 19;
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle-ingress-prepared-commit-success",
        )));
        let first = PeerId::from(KeyPair::random().public_key().clone());
        let second = PeerId::from(KeyPair::random().public_key().clone());
        let ingress = FairV2Ingress::new(16, 3 * 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster([first.clone(), second.clone()])
            .expect("two validator lanes fit the prepared commit fixture");
        ingress.state.lock().leader_wire_context = Some((context_id, HEIGHT));
        ingress.open().expect("open prepared commit fixture");
        let selected = commit_certificate_request(context_id, HEIGHT, &first, 1);
        let retained_first = commit_certificate_request(context_id, HEIGHT, &first, 2);
        let retained_second = commit_certificate_request(context_id, HEIGHT, &second, 3);
        for (message, via) in [
            (selected.clone(), first.clone()),
            (retained_first, first.clone()),
            (retained_second, second.clone()),
        ] {
            assert!(matches!(
                ingress.try_push(InboundBlockMessage::from_authenticated_peer(message, via)),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
        }
        let selected_ordinal = 1;
        let physical_cut = ingress.next_physical_admission_ordinal();
        let witness = ingress
            .capture_lifecycle_queue_cut(selected_ordinal)
            .expect("capture exact selected occurrence")
            .into_prepared_witness();
        assert_eq!(
            witness.selected_disposition(),
            FairV2IngressDequeueDisposition::Admit,
            "the borrow-free witness retains the exact frozen disposition",
        );
        let (inbound, disposition) = witness
            .commit_exact_dequeue(
                &ingress,
                lifecycle_context_from_wire((context_id, HEIGHT)),
                selected_ordinal,
            )
            .expect("unchanged prepared cut commits exactly once");
        assert_same_v2_message(inbound.message(), &selected);
        assert_eq!(disposition, FairV2IngressDequeueDisposition::Admit);
        assert_eq!(
            inbound
                .ingress_ownership()
                .and_then(FairV2IngressOwnershipEvidence::runtime_physical_cut),
            Some(physical_cut)
        );
        let state = ingress.state.lock();
        assert_eq!(state.len, 2);
        assert_eq!(state.pending_wire_owners.len(), 2);
        assert!(
            state.last_service_attempt_at.is_some(),
            "the sealed commit records the same scheduler-service evidence as ordinary dequeue"
        );
        assert_eq!(
            state.ready.iter().cloned().collect::<Vec<_>>(),
            vec![
                FairV2IngressSource::Validator(second),
                FairV2IngressSource::Validator(first),
            ]
        );
        validate_live_queue_structure(&state).expect("shared dequeue tail keeps exact accounting");
    }
    #[test]
    fn prepared_commit_preserves_unrelated_post_cut_append() {
        const HEIGHT: wire::Height = 23;
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle-ingress-prepared-commit-append",
        )));
        let first = PeerId::from(KeyPair::random().public_key().clone());
        let second = PeerId::from(KeyPair::random().public_key().clone());
        let ingress = FairV2Ingress::new(16, 3 * 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster([first.clone(), second.clone()])
            .expect("two validator lanes fit the append fixture");
        ingress.state.lock().leader_wire_context = Some((context_id, HEIGHT));
        ingress.open().expect("open append fixture");
        let selected = commit_certificate_request(context_id, HEIGHT, &first, 1);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                selected, first
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let selected_ordinal = ingress.state.lock().last_admission_ordinal;
        let witness = ingress
            .capture_lifecycle_queue_cut(selected_ordinal)
            .expect("capture target before unrelated append")
            .into_prepared_witness();
        let appended = commit_certificate_request(context_id, HEIGHT, &second, 2);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                appended.clone(),
                second,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        let append_ordinal = ingress.state.lock().last_admission_ordinal;
        let (drained, _) = witness
            .commit_exact_dequeue(
                &ingress,
                lifecycle_context_from_wire((context_id, HEIGHT)),
                selected_ordinal,
            )
            .expect("post-cut append does not change the prepared prefix");
        assert_eq!(
            drained
                .ingress_ownership()
                .and_then(FairV2IngressOwnershipEvidence::runtime_physical_cut),
            Some(u128::from(append_ordinal) + 1)
        );
        assert_eq!(ingress.len(), 1);
        let retained = ingress.try_recv().expect("post-cut append remains queued");
        assert_same_v2_message(retained.message(), &appended);
    }
    #[test]
    fn prepared_commit_rejects_pre_cut_coalescence_without_dequeue() {
        let (ingress, context, peer, message, ordinal) = single_commit_request_ingress(7);
        let witness = ingress
            .capture_lifecycle_queue_cut(ordinal)
            .expect("capture target before same-wire coalescence")
            .into_prepared_witness();
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                message.clone(),
                peer
            )),
            Ok(FairV2IngressPushDisposition::Coalesced)
        ));
        let before = {
            let state = ingress.state.lock();
            (
                state.len,
                state.bytes,
                state.ready.clone(),
                state.pending_wire_owners.clone(),
            )
        };
        assert!(matches!(
            witness.commit_exact_dequeue(&ingress, context, ordinal),
            Err(FairIngressQueueCutError::QueueCutChanged)
        ));
        let state = ingress.state.lock();
        assert_eq!(
            (
                state.len,
                state.bytes,
                state.ready.clone(),
                state.pending_wire_owners.clone(),
            ),
            before
        );
        assert!(find_entry_by_physical_ordinal(&state, ordinal).is_some());
    }
    #[test]
    fn prepared_lock_reports_retryable_change_for_pre_lock_coalescence() {
        let (ingress, context, peer, message, ordinal) = single_commit_request_ingress(33);
        let witness = ingress
            .capture_lifecycle_queue_cut(ordinal)
            .expect("capture target before final lock")
            .into_prepared_witness();
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(message, peer)),
            Ok(FairV2IngressPushDisposition::Coalesced)
        ));
        let retained = match witness.lock_exact_dequeue_retaining(&ingress, context, ordinal) {
            Err((FairIngressQueueCutError::QueueCutChanged, retained)) => retained,
            Err((error, _retained)) => {
                panic!("coalescence returned structural error: {error:?}")
            }
            Ok(_) => panic!("stale witness cannot acquire the final dequeue lock"),
        };
        drop(retained);
        assert_eq!(ingress.len(), 1, "failed pre-lock cannot dequeue the row");
        assert!(ingress.producer_publication_lock.try_lock().is_some());

        let recaptured = ingress
            .capture_lifecycle_queue_cut(ordinal)
            .expect("recapture coalesced target")
            .into_prepared_witness();
        let locked = recaptured
            .lock_exact_dequeue_retaining(&ingress, context, ordinal)
            .unwrap_or_else(|(error, _)| panic!("stable recapture must lock: {error:?}"));
        drop(locked);
        assert_eq!(
            ingress.len(),
            1,
            "dropping unpublished lock retains the row"
        );
    }
    #[test]
    fn locked_publication_fence_serializes_same_wire_and_reenqueues_after_commit() {
        let (ingress, context, peer, message, ordinal) = single_commit_request_ingress(8);
        let expected = message.clone();
        let witness = ingress
            .capture_lifecycle_queue_cut(ordinal)
            .expect("capture target before final publication lock")
            .into_prepared_witness();
        let locked = match witness.lock_exact_dequeue_retaining(&ingress, context, ordinal) {
            Ok(locked) => locked,
            Err((error, _witness)) => panic!("final publication lock failed: {error:?}"),
        };
        assert!(
            ingress.producer_publication_lock.try_lock().is_none(),
            "the exact dequeue must retain the producer fence before publication"
        );

        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let ingress = &ingress;
            scope.spawn(move || {
                started_tx
                    .send(())
                    .expect("same-wire producer start receiver remains live");
                let result =
                    ingress.try_push(InboundBlockMessage::from_authenticated_peer(message, peer));
                result_tx
                    .send(result)
                    .expect("same-wire producer result receiver remains live");
            });
            started_rx
                .recv()
                .expect("same-wire producer reaches the fenced operation");
            std::thread::yield_now();
            assert!(matches!(
                result_rx.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ));

            let (dequeued, disposition) = locked.commit();
            assert_eq!(disposition, FairV2IngressDequeueDisposition::Admit);
            drop(dequeued);
            assert!(matches!(
                result_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("same-wire producer resumes after exact dequeue"),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
        });

        let state = ingress.state.lock();
        assert_eq!(state.len, 1);
        assert!(state.last_admission_ordinal > ordinal);
        let retained = find_entry_by_physical_ordinal(&state, state.last_admission_ordinal)
            .expect("retransmission owns one new physical occurrence");
        assert_same_v2_message(retained.inbound.message(), &expected);
    }
    #[test]
    fn locked_publication_fence_serializes_unrelated_append_and_preserves_it() {
        let (ingress, context, peer, _selected, ordinal) = single_commit_request_ingress(9);
        let (context_id, height) = ingress
            .state
            .lock()
            .leader_wire_context
            .expect("fixture binds one exact wire context");
        let appended = commit_certificate_request(context_id, height, &peer, 10);
        let expected = appended.clone();
        let witness = ingress
            .capture_lifecycle_queue_cut(ordinal)
            .expect("capture target before unrelated producer")
            .into_prepared_witness();
        let locked = match witness.lock_exact_dequeue_retaining(&ingress, context, ordinal) {
            Ok(locked) => locked,
            Err((error, _witness)) => panic!("final publication lock failed: {error:?}"),
        };
        assert!(ingress.producer_publication_lock.try_lock().is_none());

        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let ingress = &ingress;
            scope.spawn(move || {
                started_tx
                    .send(())
                    .expect("append producer start receiver remains live");
                let result =
                    ingress.try_push(InboundBlockMessage::from_authenticated_peer(appended, peer));
                result_tx
                    .send(result)
                    .expect("append producer result receiver remains live");
            });
            started_rx
                .recv()
                .expect("append producer reaches the fenced operation");
            std::thread::yield_now();
            assert!(matches!(
                result_rx.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ));

            let (dequeued, disposition) = locked.commit();
            assert_eq!(disposition, FairV2IngressDequeueDisposition::Admit);
            drop(dequeued);
            assert!(matches!(
                result_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("append producer resumes after exact dequeue"),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
        });

        assert_eq!(ingress.len(), 1);
        let retained = ingress
            .try_recv()
            .expect("unrelated post-publication append remains queued");
        assert_same_v2_message(retained.message(), &expected);
    }
    #[test]
    fn dropping_locked_publication_fence_releases_producer_without_dequeue() {
        let (ingress, context, peer, selected, ordinal) = single_commit_request_ingress(11);
        let (context_id, height) = ingress
            .state
            .lock()
            .leader_wire_context
            .expect("fixture binds one exact wire context");
        let appended = commit_certificate_request(context_id, height, &peer, 12);
        let witness = ingress
            .capture_lifecycle_queue_cut(ordinal)
            .expect("capture target before droppable publication fence")
            .into_prepared_witness();
        let locked = match witness.lock_exact_dequeue_retaining(&ingress, context, ordinal) {
            Ok(locked) => locked,
            Err((error, _witness)) => panic!("final publication lock failed: {error:?}"),
        };
        assert!(ingress.producer_publication_lock.try_lock().is_none());
        drop(locked);
        assert!(
            ingress.producer_publication_lock.try_lock().is_some(),
            "dropping an unpublished exact dequeue releases its producer fence"
        );
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                appended.clone(),
                peer,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));

        assert_eq!(ingress.len(), 2);
        let retained_selected = ingress
            .try_recv()
            .expect("dropping the locked preparation leaves the target queued");
        assert_same_v2_message(retained_selected.message(), &selected);
        let retained_append = ingress
            .try_recv()
            .expect("producer resumes after the unpublished preparation drops");
        assert_same_v2_message(retained_append.message(), &appended);
    }
    #[test]
    fn prepared_commit_rejects_pre_cut_reorder_without_dequeue() {
        const HEIGHT: wire::Height = 29;
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle-ingress-prepared-commit-reorder",
        )));
        let first = PeerId::from(KeyPair::random().public_key().clone());
        let second = PeerId::from(KeyPair::random().public_key().clone());
        let ingress = FairV2Ingress::new(16, 3 * 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster([first.clone(), second.clone()])
            .expect("two validator lanes fit the reorder fixture");
        ingress.state.lock().leader_wire_context = Some((context_id, HEIGHT));
        ingress.open().expect("open reorder fixture");
        for (signature, peer) in [(1, first), (2, second)] {
            assert!(matches!(
                ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                    commit_certificate_request(context_id, HEIGHT, &peer, signature),
                    peer,
                )),
                Ok(FairV2IngressPushDisposition::Enqueued)
            ));
        }
        let ordinal = 1;
        let witness = ingress
            .capture_lifecycle_queue_cut(ordinal)
            .expect("capture target before ready-prefix reorder")
            .into_prepared_witness();
        let before = {
            let mut state = ingress.state.lock();
            state.ready.swap(0, 1);
            (state.len, state.bytes, state.ready.clone())
        };
        assert!(matches!(
            witness.commit_exact_dequeue(
                &ingress,
                lifecycle_context_from_wire((context_id, HEIGHT)),
                ordinal,
            ),
            Err(FairIngressQueueCutError::QueueCutChanged)
        ));
        let state = ingress.state.lock();
        assert_eq!((state.len, state.bytes, state.ready.clone()), before);
        assert!(find_entry_by_physical_ordinal(&state, ordinal).is_some());
    }
    #[test]
    fn prepared_commit_rejects_wrong_queue_context_and_ordinal() {
        let (first_ingress, first_context, _, _, first_ordinal) = single_commit_request_ingress(11);
        let (foreign_ingress, _, _, _, _) = single_commit_request_ingress(12);
        let foreign_queue_witness = first_ingress
            .capture_lifecycle_queue_cut(first_ordinal)
            .expect("capture witness for exact queue")
            .into_prepared_witness();
        assert!(matches!(
            foreign_queue_witness.commit_exact_dequeue(
                &foreign_ingress,
                first_context,
                first_ordinal,
            ),
            Err(FairIngressQueueCutError::ForeignQueue)
        ));
        assert_eq!(first_ingress.len(), 1);
        assert_eq!(foreign_ingress.len(), 1);
        let (context_ingress, context, _, _, context_ordinal) = single_commit_request_ingress(13);
        let context_witness = context_ingress
            .capture_lifecycle_queue_cut(context_ordinal)
            .expect("capture witness for exact context")
            .into_prepared_witness();
        assert!(matches!(
            context_witness.commit_exact_dequeue(
                &context_ingress,
                LifecycleContext::new(LifecycleDigest::new([0xFF; 32]), context.height()),
                context_ordinal,
            ),
            Err(FairIngressQueueCutError::ForeignCommitContext)
        ));
        assert_eq!(context_ingress.len(), 1);
        let (ordinal_ingress, ordinal_context, _, _, ordinal) = single_commit_request_ingress(14);
        let ordinal_witness = ordinal_ingress
            .capture_lifecycle_queue_cut(ordinal)
            .expect("capture witness for exact ordinal")
            .into_prepared_witness();
        assert!(matches!(
            ordinal_witness.commit_exact_dequeue(&ordinal_ingress, ordinal_context, ordinal + 1,),
            Err(FairIngressQueueCutError::ForeignCommitOrdinal)
        ));
        assert_eq!(ordinal_ingress.len(), 1);
    }
    #[test]
    fn prepared_witness_is_consumed_by_one_exact_dequeue() {
        let (ingress, context, _, _, ordinal) = single_commit_request_ingress(21);
        let mut witness = Some(
            ingress
                .capture_lifecycle_queue_cut(ordinal)
                .expect("capture one-shot queue witness")
                .into_prepared_witness(),
        );
        witness
            .take()
            .expect("one-shot witness is initially present")
            .commit_exact_dequeue(&ingress, context, ordinal)
            .expect("one-shot witness commits its exact row");
        assert!(witness.take().is_none());
        assert!(matches!(
            ingress.capture_lifecycle_queue_cut(ordinal),
            Err(FairIngressQueueCutError::MissingTarget)
        ));
    }
    #[test]
    fn prepared_commit_revalidates_the_frozen_disposition_before_dequeue() {
        let (ingress, context, _, _, ordinal) = single_commit_request_ingress(23);
        let mut witness = ingress
            .capture_lifecycle_queue_cut(ordinal)
            .expect("capture exact ordinary-admission disposition")
            .into_prepared_witness();
        assert_eq!(
            witness.selected_disposition(),
            FairV2IngressDequeueDisposition::Admit
        );
        witness.selected_disposition = FairV2IngressDequeueDisposition::RetireObsolete;
        assert!(matches!(
            witness.commit_exact_dequeue(&ingress, context, ordinal),
            Err(FairIngressQueueCutError::QueueCutChanged)
        ));
        assert_eq!(ingress.len(), 1, "disposition drift cannot consume the row");
    }
    #[test]
    fn prepared_commit_refuses_a_retained_inbound_arc_without_dequeue() {
        let (ingress, context, _, _, ordinal) = single_commit_request_ingress(22);
        let cut = ingress
            .capture_lifecycle_queue_cut(ordinal)
            .expect("capture exact row before retaining a classifier carrier");
        let retained = cut
            .selector_occurrences()
            .next()
            .expect("captured census contains the selected row")
            .clone_inbound();
        let witness = cut.into_prepared_witness();
        assert!(matches!(
            witness.commit_exact_dequeue(&ingress, context, ordinal),
            Err(FairIngressQueueCutError::DequeueFailed)
        ));
        assert_eq!(ingress.len(), 1);
        assert_eq!(
            retained
                .ingress_ownership()
                .and_then(FairV2IngressOwnershipEvidence::physical_admission_ordinal),
            Some(ordinal)
        );
        drop(retained);
    }
}
