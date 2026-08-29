#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactFanoutOwnership {
    /// Every post was admitted or the exact unadmitted suffix entered the corridor.
    Owned,
    /// The bounded corridor was full; the semantic producer must retain its source.
    SourceRetained,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExactOutputDriveOutcome {
    Drained,
    ReceiptBackpressured,
    Backpressured {
        closest_rank: usize,
    },
    BudgetExhausted {
        closest_backpressure_rank: Option<usize>,
    },
}
enum ExactOutputAttemptOutcome {
    Admitted,
    ReplyFlush(NetworkReplyFlushAck),
    SidecarFlush(NetworkReplyFlushAck),
    #[cfg(test)]
    TestReplyFlushed,
    Unavailable,
    Retired,
}

/// Scalar-only view of exact network-output ownership for scheduler-stall
/// diagnostics. Peer identities, payloads, hashes, and reply capabilities do
/// not cross this boundary.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(in crate::sumeragi) struct ExactOutputSchedulerSnapshotV1 {
    sealed: bool,
    pending: bool,
    fanouts: usize,
    messages: usize,
    targets: usize,
    remaining_message_occurrences: usize,
    dispatchable_fanouts: usize,
    current_posts: usize,
    ticketless_current_posts: usize,
    admission_tickets: usize,
    ticket_rank_one: usize,
    ticket_rank_later: usize,
    ticket_rank_unavailable: usize,
    minimum_ticket_rank: Option<usize>,
    maximum_ticket_rank: Option<usize>,
    pending_flushes: usize,
    parked_targets: usize,
    completed_targets: usize,
    source_owner_classes: usize,
    source_owner_edges: usize,
    reservation_classes: usize,
    reserved_target_classes: usize,
    reliable_units: usize,
    pacemaker_units: usize,
    sidecar_topology_units: usize,
    sidecar_reply_units: usize,
    ownership_units: usize,
    ownership_unit_capacity: usize,
    shared_ownership_units: usize,
    shared_ownership_unit_capacity: usize,
    admitted_sidecar_receipts: usize,
    sidecar_control_units: usize,
    sidecar_admission_capacity: usize,
    next_fanout_index: usize,
    drive_attempt_budget: usize,
}

/// Process-local corridor/transport owner whose live endpoint identity binds a
/// handoff without entering wire, durable, or consensus state.
struct DurableExactOutputOwnerNonce {
    sealed: AtomicBool,
}
/// Exact-output endpoint retained by one [`ProductionV2Services`] instance.
pub(crate) struct DurableExactOutputServiceOwner(Arc<DurableExactOutputOwnerNonce>);
/// Paired endpoint retained beside one exact [`crate::merge_sidecar::MergeSidecarTransport`].
pub(crate) struct DurableExactOutputTransportOwner(Arc<DurableExactOutputOwnerNonce>);
/// Mint the unique service/transport owner pair for one height-local stack.
pub(crate) fn durable_exact_output_handoff_owner_pair() -> (
    DurableExactOutputServiceOwner,
    DurableExactOutputTransportOwner,
) {
    let owner = Arc::new(DurableExactOutputOwnerNonce {
        sealed: AtomicBool::new(false),
    });
    (
        DurableExactOutputServiceOwner(Arc::clone(&owner)),
        DurableExactOutputTransportOwner(owner),
    )
}
impl DurableExactOutputServiceOwner {
    /// Return whether this service endpoint was minted with one transport endpoint.
    pub(in crate::sumeragi) fn is_bound_to_transport_owner(
        &self,
        owner: &DurableExactOutputTransportOwner,
    ) -> bool {
        Arc::ptr_eq(&self.0, &owner.0)
    }
    fn is_sealed(&self) -> bool {
        self.0.sealed.load(AtomicOrdering::Acquire)
    }
    fn seal(&self) -> Result<(), String> {
        self.0
            .sealed
            .compare_exchange(false, true, AtomicOrdering::AcqRel, AtomicOrdering::Acquire)
            .map(|_| ())
            .map_err(|_| "Sumeragi v2 durable exact-output handoff was already sealed".to_owned())
    }
}
#[cfg(test)]
impl DurableExactOutputTransportOwner {
    /// Reconstruct the paired test endpoint without exposing the owner nonce.
    pub(in crate::sumeragi) fn paired_service_for_test(&self) -> DurableExactOutputServiceOwner {
        DurableExactOutputServiceOwner(Arc::clone(&self.0))
    }
}
/// Move-only durable-supersession proof binding canonical hashes to the private
/// process-local service endpoint, excluding independently created services.
#[must_use]
pub(crate) struct DurableExactOutputHandoffReceipt {
    owner: Arc<DurableExactOutputOwnerNonce>,
    predecessor_context_hash: HashOf<wire::HeightContext>,
    predecessor_context_id: wire::HeightContextId,
    predecessor_height: u64,
    predecessor_network_id: iroha_data_model::NetworkId,
    finality_artifact_hash: HashOf<wire::finality::V2FinalityArtifact>,
    finality_commit_qc: wire::QuorumCertificate,
}
impl DurableExactOutputHandoffReceipt {
    /// Return whether this receipt names the transport endpoint paired with its service.
    pub(crate) fn is_bound_to_transport_owner(
        &self,
        owner: &DurableExactOutputTransportOwner,
    ) -> bool {
        Arc::ptr_eq(&self.owner, &owner.0)
    }
    /// Match the receipt's full canonical predecessor context identity.
    pub(crate) fn matches_predecessor_context(&self, context: &wire::HeightContext) -> bool {
        self.predecessor_context_hash == HashOf::new(context)
            && self.predecessor_context_id == context.id()
            && self.predecessor_height == context.height
            && self.predecessor_network_id == context.network_id
    }
    /// Match the exact durable finality artifact that authorized the seal.
    pub(crate) fn matches_finality_artifact(
        &self,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> bool {
        self.finality_artifact_hash == HashOf::new(artifact)
            && self.predecessor_context_hash == HashOf::new(&artifact.height_context)
            && self.predecessor_context_id == artifact.context_id()
            && self.predecessor_height == artifact.height
            && self.predecessor_network_id == artifact.height_context.network_id
            && self.finality_commit_qc == artifact.commit_qc
    }
    /// Verify the exact parent QC and height relation for one immediate successor.
    pub(crate) fn authorizes_immediate_successor(&self, successor: &wire::HeightContext) -> bool {
        self.predecessor_height.checked_add(1) == Some(successor.height)
            && self.predecessor_network_id == successor.network_id
            && successor.parent_commit_qc.as_ref() == Some(&self.finality_commit_qc)
            && self.finality_commit_qc.round.context_id == self.predecessor_context_id
            && self.finality_commit_qc.round.height == self.predecessor_height
    }
}
fn certified_sidecar_prefix_covers_occurrence(
    prefix: &CertifiedMergeSidecarClosedPrefix,
    requester: &PeerId,
    service_generation: crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
) -> bool {
    requester == &prefix.requester
        && (service_generation < prefix.service_generation
            || (service_generation == prefix.service_generation
                && (stream_epoch < prefix.stream_epoch
                    || (stream_epoch == prefix.stream_epoch
                        && semantic_sequence.get() <= prefix.closed_through))))
}
/// Bounded per-target FIFO owner for semantic network output awaiting actor admission.
#[derive(Debug)]
struct PendingExactOutput {
    fanouts: VecDeque<PendingExactFanout>,
    /// Kura-verified authority observed only after State committed this height.
    ///
    /// This never relaxes ordinary exact-output ownership. It only proves that
    /// a ticketless topology target is superseded when the existing applied-height
    /// handoff contract can authenticate its typed claim from the exact durable
    /// finality artifact, with read-only Kura evidence where the claim requires it.
    /// This does not infer a topology delta.
    applied_height_finality: Option<wire::finality::V2FinalityArtifact>,
    /// Writer-flushed sidecar cursor receipts not yet applied by lane work.
    admitted_sidecar_chunks: VecDeque<CertifiedMergeSidecarChunkAdmission>,
    /// Separate byte-free control-queue bound for sidecar admission receipts.
    sidecar_admission_capacity: usize,
    next_fanout_index: usize,
    /// Next stable enqueue sequence between deterministic overflow rebases.
    next_fanout_fifo_id: ExactFanoutFifoId,
    /// Every outstanding authenticated source mapped to its FIFO-ordered owners.
    source_fifo_owners: BTreeMap<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>,
    /// Ownership-unit bound: shared units, one unit for every frozen
    /// target/class pair, one pacemaker unit, one sidecar topology-progress
    /// unit, and one reproducible exact-reply control unit per frozen target.
    ownership_unit_capacity: usize,
    /// Units available to duplicate or non-frozen target/class ownership.
    shared_ownership_unit_capacity: usize,
    /// Per-target reliable, pacemaker, topology-progress, and reply-control
    /// reservation geometry frozen for this height.
    reserved_target_classes: BTreeSet<ExactTargetReservation>,
    /// Aggregate outstanding multiplicity for each semantic target/class/kind unit.
    reservation_owner_counts: BTreeMap<ExactTargetReservation, usize>,
    /// Total outstanding target/class/kind ownership units in retained fanouts.
    ownership_units: usize,
    /// Outstanding units not covered by the first frozen target/class/kind credit.
    shared_ownership_units: usize,
    /// Deterministic actor-admission attempts before yielding to the runner.
    ///
    /// Atomic Proposal admission retains the two pre-atomic child slices: one
    /// for control and one for chunks.
    drive_attempt_budget: usize,
    max_messages_per_fanout: usize,
    max_peers_per_fanout: usize,
}
/// Precomputed topology-batch mutation held under one mutex after all fallible
/// validation, capacity, FIFO, and index projection.
struct PendingExactOutputBatchPlan {
    existing_fanout_count: usize,
    rebased_existing_fifo_ids: Option<Vec<ExactFanoutFifoId>>,
    fanouts: Vec<PendingExactFanout>,
    source_fifo_owners: BTreeMap<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>,
    reservation_owner_counts: BTreeMap<ExactTargetReservation, usize>,
    ownership_units: usize,
    shared_ownership_units: usize,
    next_fanout_fifo_id: ExactFanoutFifoId,
}
/// Fully validated exact-output removal which can be committed without a
/// fallible step after a surrounding persistence transaction crosses its
/// irreversible boundary.
struct PendingExactOutputRemovalPlan {
    existing_fifo_ids: Vec<ExactFanoutFifoId>,
    removed_fifo_ids: BTreeSet<ExactFanoutFifoId>,
    retained_source_fifo_owners: BTreeMap<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>,
    retained_reservation_owner_counts: BTreeMap<ExactTargetReservation, usize>,
    retained_ownership_units: usize,
    retained_shared_ownership_units: usize,
    retained_next_fanout_index: usize,
}
impl PendingExactOutput {
    fn new(
        shared_ownership_unit_capacity: usize,
        max_messages_per_fanout: usize,
        max_peers_per_fanout: usize,
        frozen_semantic_targets: &[PeerId],
    ) -> Result<Self, String> {
        if shared_ownership_unit_capacity == 0
            || max_messages_per_fanout == 0
            || max_peers_per_fanout == 0
        {
            return Err("Sumeragi v2 outbound corridor bounds must be non-zero".to_owned());
        }
        let reserved_target_classes = frozen_semantic_targets
            .iter()
            .flat_map(|semantic_target| {
                EXACT_OUTPUT_CLASSES
                    .map(|class| ExactTargetReservation {
                        semantic_target: semantic_target.clone(),
                        class,
                        kind: ExactTargetReservationKind::Reliable,
                    })
                    .into_iter()
                    .chain([ExactTargetReservation {
                        semantic_target: semantic_target.clone(),
                        class: ExactOutputClass::Safety,
                        kind: ExactTargetReservationKind::Pacemaker,
                    }])
                    .chain([ExactTargetReservation {
                        semantic_target: semantic_target.clone(),
                        // Topology-routed Request/Close progress is canonical
                        // Consensus traffic and therefore uses the Lane class.
                        class: ExactOutputClass::Lane,
                        kind: ExactTargetReservationKind::SidecarTopologyProgress,
                    }])
                    .chain([ExactTargetReservation {
                        semantic_target: semantic_target.clone(),
                        // Stateless responder controls retain exact reply
                        // authority but cannot be starved by ordinary Lane
                        // output for the same semantic target.
                        class: ExactOutputClass::Lane,
                        kind: ExactTargetReservationKind::SidecarReplyControl,
                    }])
            })
            .collect::<BTreeSet<_>>();
        let sidecar_admission_capacity = shared_ownership_unit_capacity
            .checked_add(
                reserved_target_classes
                    .iter()
                    .filter(|reservation| reservation.kind == ExactTargetReservationKind::Reliable)
                    .count(),
            )
            .ok_or_else(|| "Sumeragi v2 sidecar admission capacity overflowed".to_owned())?;
        let ownership_unit_capacity = shared_ownership_unit_capacity
            .checked_add(reserved_target_classes.len())
            .ok_or_else(|| "Sumeragi v2 outbound corridor capacity overflowed".to_owned())?;
        let drive_attempt_budget = max_peers_per_fanout
            .max(super::v2_core::MAX_EFFECTS_PER_STEP)
            .checked_mul(ATOMIC_PROPOSAL_FANOUT_COUNT)
            .ok_or_else(|| "Sumeragi v2 outbound drive budget overflowed".to_owned())?;
        Ok(Self {
            fanouts: VecDeque::new(),
            applied_height_finality: None,
            admitted_sidecar_chunks: VecDeque::new(),
            sidecar_admission_capacity,
            next_fanout_index: 0,
            next_fanout_fifo_id: 0,
            source_fifo_owners: BTreeMap::new(),
            ownership_unit_capacity,
            shared_ownership_unit_capacity,
            reserved_target_classes,
            reservation_owner_counts: BTreeMap::new(),
            ownership_units: 0,
            shared_ownership_units: 0,
            drive_attempt_budget,
            max_messages_per_fanout,
            max_peers_per_fanout,
        })
    }
    /// Preflight an all-or-nothing fresh topology batch, aggregating Proposal
    /// control/chunk multiplicities once and excluding stateful replacements.
    #[allow(clippy::too_many_lines)]
    fn prepare_atomic_fanout_batch(
        &self,
        mut fanouts: Vec<PendingExactFanout>,
    ) -> Result<Option<PendingExactOutputBatchPlan>, String> {
        let existing_fanout_count = self.fanouts.len();
        let mut additions = BTreeMap::<ExactTargetReservation, usize>::new();
        let mut incumbent_component = false;
        for fanout in &fanouts {
            self.validate_fanout_bounds(fanout)?;
            if fanout.is_complete()
                || fanout.reply_routes.is_some()
                || fanout.ingress_ownership.is_some()
                || fanout
                    .targets
                    .iter()
                    .any(|target| !matches!(target.route, ExactTargetRoute::Topology))
                || self
                    .stranded_responder_control_replacement_index(fanout)
                    .is_some()
                || self.retains_retryable_sidecar_responder_control_for(fanout)
            {
                return Err(
                    "Sumeragi v2 atomic Proposal output changed fresh topology geometry".to_owned(),
                );
            }
            incumbent_component |= self.fanouts.iter().any(|retained| {
                retained.rollover_claim == fanout.rollover_claim
                    && retained.message_hashes == fanout.message_hashes
                    && retained.reply_routes.is_none()
                    && retained.ingress_ownership.is_none()
                    && retained
                        .targets
                        .iter()
                        .all(|target| matches!(target.route, ExactTargetRoute::Topology))
            });
            for (reservation, count) in fanout.outstanding_reservation_counts()? {
                let aggregate = additions.entry(reservation).or_default();
                *aggregate = aggregate.checked_add(count).ok_or_else(|| {
                    "Sumeragi v2 atomic Proposal output ownership overflowed".to_owned()
                })?;
            }
        }
        if incumbent_component {
            // Proposal control and chunks are one inseparable source
            // occurrence. A periodic retry may find either component still
            // actor-backpressured, and the chunk target set may expand from
            // Set A to all voters. Leave the whole retry with that reducer
            // source until the incumbent drains; admitting a duplicate
            // component here would multiply bounded corridor ownership once
            // per retransmission interval. Validate every batch child above
            // before classifying this as temporary source retention.
            return Ok(None);
        }
        if !self.ownership_capacity_available(&additions)? {
            return Ok(None);
        }
        let (reservation_owner_counts, ownership_units, shared_ownership_units) =
            self.ownership_state_after_additions(&additions)?;
        let project_ids = |first: ExactFanoutFifoId| {
            let mut cursor = first;
            let mut ids = Vec::with_capacity(fanouts.len());
            for _ in &fanouts {
                if cursor == ExactFanoutFifoId::MAX {
                    return None;
                }
                ids.push(cursor);
                cursor = cursor.checked_add(1)?;
            }
            Some((ids, cursor))
        };
        let (
            rebased_existing_fifo_ids,
            fanout_fifo_ids,
            next_fanout_fifo_id,
            mut source_fifo_owners,
        ) = if let Some((ids, next)) = project_ids(self.next_fanout_fifo_id) {
            (None, ids, next, self.source_fifo_owners.clone())
        } else {
            let mut rebuilt = BTreeMap::<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>::new();
            let mut existing_ids = Vec::with_capacity(self.fanouts.len());
            for (index, retained) in self.fanouts.iter().enumerate() {
                let fifo_id = ExactFanoutFifoId::try_from(index).map_err(|_| {
                    "Sumeragi v2 atomic Proposal FIFO rebase is not representable".to_owned()
                })?;
                existing_ids.push(fifo_id);
                for source in retained.outstanding_sources()? {
                    rebuilt.entry(source).or_default().insert(fifo_id);
                }
            }
            let first = ExactFanoutFifoId::try_from(self.fanouts.len()).map_err(|_| {
                "Sumeragi v2 atomic Proposal FIFO sequence is not representable".to_owned()
            })?;
            let (ids, next) = project_ids(first)
                .ok_or_else(|| "Sumeragi v2 atomic Proposal FIFO sequence exhausted".to_owned())?;
            (Some(existing_ids), ids, next, rebuilt)
        };
        if fanout_fifo_ids.iter().any(|fifo_id| {
            source_fifo_owners
                .values()
                .any(|owners| owners.contains(fifo_id))
        }) {
            return Err("Sumeragi v2 atomic Proposal FIFO reused a live identity".to_owned());
        }
        for (fanout, fifo_id) in fanouts.iter_mut().zip(fanout_fifo_ids) {
            for source in fanout.outstanding_sources()? {
                source_fifo_owners
                    .entry(source)
                    .or_default()
                    .insert(fifo_id);
            }
            fanout.fifo_id = Some(fifo_id);
        }
        Ok(Some(PendingExactOutputBatchPlan {
            existing_fanout_count,
            rebased_existing_fifo_ids,
            fanouts,
            source_fifo_owners,
            reservation_owner_counts,
            ownership_units,
            shared_ownership_units,
            next_fanout_fifo_id,
        }))
    }
    /// Commit a batch prepared while this exact mutex guard remained held.
    fn commit_atomic_fanout_batch(&mut self, plan: PendingExactOutputBatchPlan) {
        let PendingExactOutputBatchPlan {
            existing_fanout_count,
            rebased_existing_fifo_ids,
            fanouts,
            source_fifo_owners,
            reservation_owner_counts,
            ownership_units,
            shared_ownership_units,
            next_fanout_fifo_id,
        } = plan;
        assert_eq!(
            self.fanouts.len(),
            existing_fanout_count,
            "atomic Proposal output retained the corridor mutex"
        );
        if let Some(rebased) = rebased_existing_fifo_ids {
            assert_eq!(rebased.len(), self.fanouts.len());
            for (fanout, fifo_id) in self.fanouts.iter_mut().zip(rebased) {
                fanout.fifo_id = Some(fifo_id);
            }
        }
        self.fanouts.extend(fanouts);
        self.source_fifo_owners = source_fifo_owners;
        self.reservation_owner_counts = reservation_owner_counts;
        self.ownership_units = ownership_units;
        self.shared_ownership_units = shared_ownership_units;
        self.next_fanout_fifo_id = next_fanout_fifo_id;
    }
    fn is_pending(&self) -> bool {
        self.fanouts.iter().any(|fanout| {
            fanout.has_dispatchable_target()
                || fanout
                    .targets
                    .iter()
                    .any(|target| target.pending_flush.is_some())
        }) || !self.admitted_sidecar_chunks.is_empty()
    }

    fn scheduler_snapshot(&self, sealed: bool) -> ExactOutputSchedulerSnapshotV1 {
        let mut messages = 0usize;
        let mut targets = 0usize;
        let mut remaining_message_occurrences = 0usize;
        let mut dispatchable_fanouts = 0usize;
        let mut current_posts = 0usize;
        let mut ticketless_current_posts = 0usize;
        let mut admission_tickets = 0usize;
        let mut ticket_rank_one = 0usize;
        let mut ticket_rank_later = 0usize;
        let mut ticket_rank_unavailable = 0usize;
        let mut minimum_ticket_rank = None;
        let mut maximum_ticket_rank = None;
        let mut pending_flushes = 0usize;
        let mut parked_targets = 0usize;
        let mut completed_targets = 0usize;
        for fanout in &self.fanouts {
            messages = messages.saturating_add(fanout.messages.len());
            targets = targets.saturating_add(fanout.targets.len());
            dispatchable_fanouts =
                dispatchable_fanouts.saturating_add(usize::from(fanout.has_dispatchable_target()));
            for target in &fanout.targets {
                remaining_message_occurrences = remaining_message_occurrences
                    .saturating_add(fanout.messages.len().saturating_sub(target.message_index));
                current_posts = current_posts.saturating_add(usize::from(target.current.is_some()));
                ticketless_current_posts = ticketless_current_posts.saturating_add(usize::from(
                    target.current.is_some() && target.ticket.is_none(),
                ));
                if let Some(ticket) = target.ticket.as_ref() {
                    admission_tickets = admission_tickets.saturating_add(1);
                    let rank = ticket.rank();
                    match rank {
                        Some(1) => ticket_rank_one = ticket_rank_one.saturating_add(1),
                        Some(_) => {
                            ticket_rank_later = ticket_rank_later.saturating_add(1);
                        }
                        None => {
                            ticket_rank_unavailable = ticket_rank_unavailable.saturating_add(1);
                        }
                    }
                    if let Some(rank) = rank {
                        minimum_ticket_rank = Some(
                            minimum_ticket_rank.map_or(rank, |minimum: usize| minimum.min(rank)),
                        );
                        maximum_ticket_rank = Some(
                            maximum_ticket_rank.map_or(rank, |maximum: usize| maximum.max(rank)),
                        );
                    }
                }
                pending_flushes =
                    pending_flushes.saturating_add(usize::from(target.pending_flush.is_some()));
                parked_targets = parked_targets.saturating_add(usize::from(target.parked));
                completed_targets = completed_targets
                    .saturating_add(usize::from(target.message_index == fanout.messages.len()));
            }
        }
        let mut reliable_units = 0usize;
        let mut pacemaker_units = 0usize;
        let mut sidecar_topology_units = 0usize;
        let mut sidecar_reply_units = 0usize;
        for (reservation, count) in &self.reservation_owner_counts {
            let aggregate = match reservation.kind {
                ExactTargetReservationKind::Reliable => &mut reliable_units,
                ExactTargetReservationKind::Pacemaker => &mut pacemaker_units,
                ExactTargetReservationKind::SidecarTopologyProgress => &mut sidecar_topology_units,
                ExactTargetReservationKind::SidecarReplyControl => &mut sidecar_reply_units,
            };
            *aggregate = aggregate.saturating_add(*count);
        }
        ExactOutputSchedulerSnapshotV1 {
            sealed,
            pending: self.is_pending(),
            fanouts: self.fanouts.len(),
            messages,
            targets,
            remaining_message_occurrences,
            dispatchable_fanouts,
            current_posts,
            ticketless_current_posts,
            admission_tickets,
            ticket_rank_one,
            ticket_rank_later,
            ticket_rank_unavailable,
            minimum_ticket_rank,
            maximum_ticket_rank,
            pending_flushes,
            parked_targets,
            completed_targets,
            source_owner_classes: self.source_fifo_owners.len(),
            source_owner_edges: self
                .source_fifo_owners
                .values()
                .fold(0usize, |total, owners| total.saturating_add(owners.len())),
            reservation_classes: self.reservation_owner_counts.len(),
            reserved_target_classes: self.reserved_target_classes.len(),
            reliable_units,
            pacemaker_units,
            sidecar_topology_units,
            sidecar_reply_units,
            ownership_units: self.ownership_units,
            ownership_unit_capacity: self.ownership_unit_capacity,
            shared_ownership_units: self.shared_ownership_units,
            shared_ownership_unit_capacity: self.shared_ownership_unit_capacity,
            admitted_sidecar_receipts: self.admitted_sidecar_chunks.len(),
            sidecar_control_units: self.sidecar_control_units(),
            sidecar_admission_capacity: self.sidecar_admission_capacity,
            next_fanout_index: self.next_fanout_index,
            drive_attempt_budget: self.drive_attempt_budget,
        }
    }
    fn pending_kura_replica_advert_heights(&self) -> Result<BTreeSet<u64>, String> {
        let mut heights = BTreeSet::new();
        for fanout in &self.fanouts {
            let ExactOutputRolloverClaim::DurableKuraReplicaAdvert { source_height, .. } =
                &fanout.rollover_claim
            else {
                continue;
            };
            fanout
                .rollover_claim
                .validate_fanout(&fanout.messages, &fanout.semantic_peers())?;
            if *source_height == 0 {
                return Err(
                    "pending Kura replica advert lost its non-zero durable source height"
                        .to_owned(),
                );
            }
            heights.insert(*source_height);
        }
        Ok(heights)
    }
    fn plan_fanout_removal(
        &self,
        covered: impl Fn(&PendingExactFanout) -> bool,
        validate_removed: impl Fn(&PendingExactFanout) -> Result<(), String>,
        operation: &'static str,
    ) -> Result<PendingExactOutputRemovalPlan, String> {
        let mut current_sources = BTreeMap::<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>::new();
        let mut current_reservations = BTreeMap::<ExactTargetReservation, usize>::new();
        let mut retained_sources =
            BTreeMap::<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>::new();
        let mut retained_reservations = BTreeMap::<ExactTargetReservation, usize>::new();
        let mut existing_fifo_ids = Vec::with_capacity(self.fanouts.len());
        let mut removed_fifo_ids = BTreeSet::new();
        for fanout in &self.fanouts {
            if fanout.message_hashes.len() != fanout.messages.len()
                || fanout
                    .messages
                    .iter()
                    .zip(&fanout.message_hashes)
                    .any(|(message, expected)| HashOf::new(message) != *expected)
            {
                return Err(format!(
                    "Sumeragi v2 {operation} found altered exact-output payload"
                ));
            }
            let fifo_id = fanout.fifo_id.ok_or_else(|| {
                format!("Sumeragi v2 {operation} found an unowned exact-output fanout")
            })?;
            existing_fifo_ids.push(fifo_id);
            let sources = fanout.outstanding_sources()?;
            let reservations = fanout.outstanding_reservation_counts()?;
            for source in &sources {
                current_sources
                    .entry(source.clone())
                    .or_default()
                    .insert(fifo_id);
            }
            for (reservation, count) in &reservations {
                let aggregate = current_reservations.entry(reservation.clone()).or_default();
                *aggregate = aggregate
                    .checked_add(*count)
                    .ok_or_else(|| format!("Sumeragi v2 {operation} ownership count overflowed"))?;
            }
            if covered(fanout) {
                validate_removed(fanout)?;
                if !removed_fifo_ids.insert(fifo_id) {
                    return Err(format!(
                        "Sumeragi v2 {operation} found duplicate exact-output FIFO ownership"
                    ));
                }
                continue;
            }
            for source in sources {
                retained_sources.entry(source).or_default().insert(fifo_id);
            }
            for (reservation, count) in reservations {
                let aggregate = retained_reservations.entry(reservation).or_default();
                *aggregate = aggregate.checked_add(count).ok_or_else(|| {
                    format!("Sumeragi v2 retained {operation} ownership count overflowed")
                })?;
            }
        }
        if current_sources != self.source_fifo_owners
            || current_reservations != self.reservation_owner_counts
        {
            return Err(format!(
                "Sumeragi v2 {operation} found inconsistent exact-output ownership"
            ));
        }
        let mut retained_units = 0usize;
        let mut retained_shared_units = 0usize;
        for (reservation, count) in &retained_reservations {
            retained_units = retained_units
                .checked_add(*count)
                .ok_or_else(|| format!("Sumeragi v2 retained {operation} units overflowed"))?;
            let frozen_credit = usize::from(self.reserved_target_classes.contains(reservation));
            retained_shared_units = retained_shared_units
                .checked_add(count.checked_sub(frozen_credit).ok_or_else(|| {
                    format!("Sumeragi v2 retained {operation} frozen credit exceeded ownership")
                })?)
                .ok_or_else(|| {
                    format!("Sumeragi v2 retained {operation} shared units overflowed")
                })?;
        }
        let retained_fanout_count = self
            .fanouts
            .len()
            .checked_sub(removed_fifo_ids.len())
            .ok_or_else(|| format!("Sumeragi v2 {operation} count underflowed"))?;
        let retained_next_fanout_index = if retained_fanout_count == 0 {
            0
        } else {
            self.next_fanout_index % retained_fanout_count
        };
        Ok(PendingExactOutputRemovalPlan {
            existing_fifo_ids,
            removed_fifo_ids,
            retained_source_fifo_owners: retained_sources,
            retained_reservation_owner_counts: retained_reservations,
            retained_ownership_units: retained_units,
            retained_shared_ownership_units: retained_shared_units,
            retained_next_fanout_index,
        })
    }
    fn commit_fanout_removal(&mut self, plan: PendingExactOutputRemovalPlan) -> usize {
        let PendingExactOutputRemovalPlan {
            existing_fifo_ids,
            removed_fifo_ids,
            retained_source_fifo_owners,
            retained_reservation_owner_counts,
            retained_ownership_units,
            retained_shared_ownership_units,
            retained_next_fanout_index,
        } = plan;
        debug_assert_eq!(
            self.fanouts
                .iter()
                .map(|fanout| fanout.fifo_id.expect("preflighted exact-output FIFO owner"))
                .collect::<Vec<_>>(),
            existing_fifo_ids,
            "an exclusive exact-output removal plan cannot observe intervening fanout mutation"
        );
        self.fanouts.retain(|fanout| {
            !removed_fifo_ids.contains(
                &fanout
                    .fifo_id
                    .expect("preflighted exact-output fanout remains FIFO-owned"),
            )
        });
        self.source_fifo_owners = retained_source_fifo_owners;
        self.reservation_owner_counts = retained_reservation_owner_counts;
        self.ownership_units = retained_ownership_units;
        self.shared_ownership_units = retained_shared_ownership_units;
        self.next_fanout_index = retained_next_fanout_index;
        removed_fifo_ids.len()
    }
    fn remove_fanouts_matching(
        &mut self,
        covered: impl Fn(&PendingExactFanout) -> bool,
        validate_removed: impl Fn(&PendingExactFanout) -> Result<(), String>,
        operation: &'static str,
    ) -> Result<usize, String> {
        let plan = self.plan_fanout_removal(covered, validate_removed, operation)?;
        Ok(self.commit_fanout_removal(plan))
    }
    fn close_certified_sidecar_prefix(
        &mut self,
        prefix: &CertifiedMergeSidecarClosedPrefix,
    ) -> Result<usize, String> {
        let covered = |fanout: &PendingExactFanout| {
            matches!(
                &fanout.rollover_claim,
                ExactOutputRolloverClaim::CertifiedSidecarChunk { transfer, .. }
                    if certified_sidecar_prefix_covers_occurrence(
                        prefix,
                        &transfer.requester,
                        transfer.service_generation,
                        transfer.stream_epoch,
                        transfer.semantic_sequence,
                )
            )
        };
        let removed = self.remove_fanouts_matching(
            covered,
            |fanout| {
                fanout
                    .is_certified_sidecar_chunk_fanout()
                    .then_some(())
                    .ok_or_else(|| {
                        "Sumeragi v2 sidecar close claim covers a different output kind".to_owned()
                    })
            },
            "sidecar close",
        )?;
        self.admitted_sidecar_chunks.retain(|admission| {
            let projection = admission.projection();
            !certified_sidecar_prefix_covers_occurrence(
                prefix,
                &projection.requester,
                projection.service_generation,
                projection.stream_epoch,
                projection.semantic_sequence,
            )
        });
        debug_assert!(self.sidecar_control_units() <= self.sidecar_admission_capacity);
        Ok(removed)
    }
    fn cancel_historical_lane_recovery_requests(
        &mut self,
        request_hashes: &BTreeSet<HashOf<LaneHistoricalRecoveryRequestV1>>,
    ) -> Result<usize, String> {
        if request_hashes.is_empty() {
            return Ok(0);
        }
        self.remove_fanouts_matching(
            |fanout| {
                matches!(
                    &fanout.rollover_claim,
                    ExactOutputRolloverClaim::HistoricalLaneRecoveryRequest {
                        request_hash,
                        ..
                    } if request_hashes.contains(request_hash)
                )
            },
            |fanout| {
                fanout
                    .rollover_claim
                    .validate_fanout(&fanout.messages, &fanout.semantic_peers())
            },
            "historical recovery cancellation",
        )
    }
    fn cancel_certified_body_request(
        &mut self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) -> Result<usize, String> {
        let plan = self.plan_certified_body_request_cancellation(request_hash)?;
        Ok(self.commit_fanout_removal(plan))
    }
    fn plan_certified_body_request_cancellation(
        &self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) -> Result<PendingExactOutputRemovalPlan, String> {
        self.plan_fanout_removal(
            |fanout| {
                matches!(
                    fanout.messages.as_slice(),
                    [NetworkMessage::SumeragiBlock(envelope)]
                        if matches!(
                            envelope.as_message(),
                            BlockMessage::V2(message)
                                if matches!(
                                    &message.payload,
                                    wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request)
                                        if HashOf::new(request) == request_hash
                                )
                        )
                )
            },
            |fanout| {
                fanout
                    .rollover_claim
                    .validate_fanout(&fanout.messages, &fanout.semantic_peers())
            },
            "certified body-request cancellation",
        )
    }
    fn cancel_commit_certificate_request(
        &mut self,
        request_hash: HashOf<wire::CommitCertificateRequest>,
    ) -> Result<usize, String> {
        self.remove_fanouts_matching(
            |fanout| {
                matches!(
                    fanout.messages.as_slice(),
                    [NetworkMessage::SumeragiBlock(envelope)]
                        if matches!(
                            envelope.as_message(),
                            BlockMessage::V2(message)
                                if matches!(
                                    &message.payload,
                                    wire::ConsensusMessageV2Payload::CommitCertificateRequest(request)
                                        if HashOf::new(request) == request_hash
                                )
                        )
                )
            },
            |fanout| {
                fanout
                    .rollover_claim
                    .validate_fanout(&fanout.messages, &fanout.semantic_peers())
            },
            "commit-certificate request cancellation",
        )
    }
    fn cancel_certified_merge_sidecar_requests(
        &mut self,
        request_hashes: &BTreeSet<HashOf<CertifiedMergeSidecarRequestV1>>,
    ) -> Result<usize, String> {
        if request_hashes.is_empty() {
            return Ok(0);
        }
        self.remove_fanouts_matching(
            |fanout| {
                matches!(
                    &fanout.rollover_claim,
                    ExactOutputRolloverClaim::CertifiedSidecarRequest {
                        request_hash,
                        ..
                    } if request_hashes.contains(request_hash)
                )
            },
            |fanout| {
                fanout
                    .rollover_claim
                    .validate_fanout(&fanout.messages, &fanout.semantic_peers())
            },
            "certified merge-sidecar request cancellation",
        )
    }
    fn cancel_obsolete_certified_merge_sidecar_generation_hints(
        &mut self,
        hints: &[CertifiedMergeSidecarGenerationHintV1],
    ) -> Result<usize, String> {
        if hints.is_empty() {
            return Ok(0);
        }
        if hints.iter().any(|hint| {
            hint.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1
                || hint.current_generation <= hint.observed_generation
                || hint.hint_id != hint.canonical_hint_id()
        }) {
            return Err(
                "Sumeragi v2 generation-fence cancellation has an invalid authenticated hint"
                    .to_owned(),
            );
        }
        self.remove_fanouts_matching(
            |fanout| {
                let [NetworkMessage::CertifiedMergeSidecar(message)] = fanout.messages.as_slice()
                else {
                    return false;
                };
                hints
                    .iter()
                    .any(|hint| match (&fanout.rollover_claim, message.as_ref()) {
                        (
                            ExactOutputRolloverClaim::CertifiedSidecarRequest { .. },
                            CertifiedMergeSidecarMessage::Request(request),
                        ) => {
                            request.version == hint.version
                                && request.request_id == request.canonical_request_id()
                                && request.requester == hint.requester
                                && request.responder == hint.responder
                                && request.service_generation < hint.current_generation
                        }
                        (
                            ExactOutputRolloverClaim::CertifiedSidecarControl { .. },
                            CertifiedMergeSidecarMessage::Close(close),
                        ) => {
                            close.version == hint.version
                                && close.closed_through != 0
                                && close.close_id == close.canonical_close_id()
                                && close.requester == hint.requester
                                && close.responder == hint.responder
                                && close.service_generation < hint.current_generation
                        }
                        _ => false,
                    })
            },
            |fanout| {
                fanout
                    .rollover_claim
                    .validate_fanout(&fanout.messages, &fanout.semantic_peers())
            },
            "certified merge-sidecar generation-fence cancellation",
        )
    }
    fn cancel_acknowledged_certified_merge_sidecar_closes(
        &mut self,
        acknowledgements: &[CertifiedMergeSidecarCloseAckV1],
    ) -> Result<usize, String> {
        if acknowledgements.is_empty() {
            return Ok(0);
        }
        if acknowledgements.iter().any(|acknowledgement| {
            acknowledgement.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1
                || acknowledgement.closed_through == 0
                || acknowledgement.close_id != acknowledgement.canonical_close_id()
        }) {
            return Err(
                "Sumeragi v2 requester Close cancellation has an invalid acknowledgement prefix"
                    .to_owned(),
            );
        }
        self.remove_fanouts_matching(
            |fanout| {
                matches!(
                    fanout.messages.as_slice(),
                    [NetworkMessage::CertifiedMergeSidecar(message)]
                        if matches!(
                            message.as_ref(),
                            CertifiedMergeSidecarMessage::Close(close)
                                if acknowledgements.iter().any(|acknowledgement| {
                                    acknowledgement.covers_requester_close(close)
                                })
                        )
                )
            },
            |fanout| {
                fanout
                    .rollover_claim
                    .validate_fanout(&fanout.messages, &fanout.semantic_peers())
            },
            "acknowledged certified merge-sidecar Close cancellation",
        )
    }
    fn pending_sidecar_flushes(&self) -> usize {
        self.fanouts
            .iter()
            .flat_map(|fanout| &fanout.targets)
            .filter(|target| {
                target
                    .pending_flush
                    .as_ref()
                    .is_some_and(|pending| pending.sidecar_admission.is_some())
            })
            .count()
    }
    fn sidecar_control_units(&self) -> usize {
        self.pending_sidecar_flushes()
            .saturating_add(self.admitted_sidecar_chunks.len())
    }
    fn restore_pending_flush(
        &mut self,
        fanout_index: usize,
        target_index: usize,
        pending_flush: PendingExactReplyFlush,
    ) -> Result<(), String> {
        let target = self
            .fanouts
            .get_mut(fanout_index)
            .and_then(|fanout| fanout.targets.get_mut(target_index))
            .ok_or_else(|| {
                "Sumeragi v2 reply flush target disappeared during validation".to_owned()
            })?;
        if target.pending_flush.replace(pending_flush).is_some() {
            return Err("Sumeragi v2 reply target acquired two writer flushes".to_owned());
        }
        Ok(())
    }
    fn poll_reply_flushes(&mut self) -> Result<(), String> {
        loop {
            let mut terminal = None;
            'scan: for (fanout_index, fanout) in self.fanouts.iter_mut().enumerate() {
                for (target_index, target) in fanout.targets.iter_mut().enumerate() {
                    let Some(pending_flush) = target.pending_flush.as_mut() else {
                        continue;
                    };
                    let status = pending_flush.flush_ack.poll();
                    if !matches!(status, NetworkReplyFlushAckStatus::Pending) {
                        terminal = Some((fanout_index, target_index, status));
                        break 'scan;
                    }
                }
            }
            let Some((fanout_index, target_index, status)) = terminal else {
                return Ok(());
            };
            let (
                canonical_post,
                attempted_source,
                current_route,
                current_timeout_attempt,
                was_parked,
            ) = {
                let fanout = self
                    .fanouts
                    .get(fanout_index)
                    .ok_or_else(|| "Sumeragi v2 flushing reply fanout disappeared".to_owned())?;
                let target = fanout
                    .targets
                    .get(target_index)
                    .ok_or_else(|| "Sumeragi v2 flushing reply target disappeared".to_owned())?;
                let ExactTargetRoute::Reply(route) = &target.route else {
                    return Err("Sumeragi v2 topology target retained a reply flush".to_owned());
                };
                let data = fanout
                    .messages
                    .get(target.message_index)
                    .ok_or_else(|| {
                        "Sumeragi v2 reply flush advanced beyond its immutable payload".to_owned()
                    })?
                    .clone();
                let peer_id = fanout
                    .peers
                    .get(target_index)
                    .ok_or_else(|| "Sumeragi v2 reply flush lost its target".to_owned())?
                    .clone();
                let class = exact_output_class(&data)?;
                (
                    Post {
                        data,
                        peer_id: peer_id.clone(),
                        priority: Priority::High,
                    },
                    target.route.source(&peer_id, class),
                    route.clone(),
                    target.reply_writer_timeout_attempt,
                    target.parked,
                )
            };
            let sidecar_flushing_before = self.pending_sidecar_flushes();
            let checked_flush_trace = {
                let pending_flush = self
                    .fanouts
                    .get(fanout_index)
                    .and_then(|fanout| fanout.targets.get(target_index))
                    .and_then(|target| target.pending_flush.as_ref())
                    .ok_or_else(|| "Sumeragi v2 terminal reply flush lost ownership".to_owned())?;
                if !pending_flush
                    .flush_ack
                    .identity()
                    .is_bound_to_canonical_reply(&canonical_post)
                    || pending_flush.flush_ack.identity().source_key() != current_route.source_key()
                    || pending_flush.reply_writer_timeout_attempt != current_timeout_attempt
                    || pending_flush
                        .flush_ack
                        .identity()
                        .reply_writer_timeout_attempt()
                        != pending_flush.reply_writer_timeout_attempt
                {
                    return Err(
                        "Sumeragi v2 terminal reply flush changed payload, source, or timeout-attempt identity"
                            .to_owned(),
                    );
                }
                if let Some(admission) = pending_flush.sidecar_admission.as_ref() {
                    if !admission.matches_ack_identity(pending_flush.flush_ack.identity()) {
                        return Err(
                            MergeSidecarError::FlushIdentityMismatch(
                                "queued admission and writer acknowledgement identify different actor output",
                            )
                            .to_string(),
                        );
                    }
                    let flushing_before = u64::try_from(sidecar_flushing_before)
                        .expect("bounded sidecar flush count is representable as u64");
                    let flushing_after = flushing_before.checked_sub(1).ok_or_else(|| {
                        MergeSidecarError::FlushIdentityMismatch(
                            "sidecar flushing-owner count underflowed",
                        )
                        .to_string()
                    })?;
                    let admitted_before = u64::try_from(self.admitted_sidecar_chunks.len())
                        .expect("bounded sidecar admission count is representable as u64");
                    let admitted_after = if matches!(status, NetworkReplyFlushAckStatus::Flushed) {
                        admitted_before.checked_add(1).ok_or_else(|| {
                            MergeSidecarError::FlushIdentityMismatch(
                                "sidecar admitted-owner count overflowed",
                            )
                            .to_string()
                        })?
                    } else {
                        admitted_before
                    };
                    let flush_trace = reliable_flush_trace_projection(
                        admission,
                        status,
                        flushing_before,
                        flushing_after,
                        admitted_before,
                        admitted_after,
                        self.sidecar_admission_capacity,
                    )
                    .map_err(|error| error.to_string())?;
                    Some(
                        check_production_reliable_flush_worker_transition(flush_trace)
                            .ok_or_else(|| {
                                MergeSidecarError::FlushIdentityMismatch(
                                    "sidecar flush transition failed its exact ownership kernel",
                                )
                                .to_string()
                            })?
                            .into_projection(),
                    )
                } else {
                    None
                }
            };
            let mut pending_flush = self
                .fanouts
                .get_mut(fanout_index)
                .and_then(|fanout| fanout.targets.get_mut(target_index))
                .and_then(|target| target.pending_flush.take())
                .ok_or_else(|| "Sumeragi v2 terminal reply flush lost ownership".to_owned())?;
            if let Some(admission) = pending_flush.sidecar_admission.as_mut() {
                let flush_trace = checked_flush_trace.ok_or_else(|| {
                    MergeSidecarError::FlushIdentityMismatch(
                        "sidecar admission lost its pre-authorized worker transition",
                    )
                    .to_string()
                })?;
                if matches!(status, NetworkReplyFlushAckStatus::Flushed)
                    && let Err(error) = admission.bind_confirmed_worker_trace(flush_trace)
                {
                    let error = error.to_string();
                    self.restore_pending_flush(fanout_index, target_index, pending_flush)?;
                    return Err(error);
                }
            }
            match status {
                NetworkReplyFlushAckStatus::Pending => {
                    unreachable!("terminal scan excludes pending")
                }
                NetworkReplyFlushAckStatus::Flushed => {
                    if pending_flush.sidecar_admission.is_none()
                        && !pending_flush.flush_ack.identity().claim_writer_flush_once()
                    {
                        self.restore_pending_flush(fanout_index, target_index, pending_flush)?;
                        return Err(
                            "Sumeragi v2 reply writer flush was consumed more than once".to_owned()
                        );
                    }
                    if was_parked {
                        self.fanouts
                            .get_mut(fanout_index)
                            .and_then(|fanout| fanout.targets.get_mut(target_index))
                            .expect("flushing parked target must remain present")
                            .parked = false;
                    }
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("flushed reply fanout must remain present")
                        .mark_admitted(target_index)?;
                    if was_parked {
                        let fanout = self
                            .fanouts
                            .get_mut(fanout_index)
                            .expect("flushed parked fanout must remain present");
                        let target_complete = fanout.target_is_complete(target_index);
                        let target = fanout
                            .targets
                            .get_mut(target_index)
                            .expect("flushed parked target must remain present");
                        let writable = matches!(&target.route, ExactTargetRoute::Topology)
                            || matches!(&target.route,
                                ExactTargetRoute::Reply(route) if route.is_reply_writable());
                        if !target_complete && !writable {
                            target.parked = true;
                        }
                    }
                    self.advance_after_attempt(
                        fanout_index,
                        target_index,
                        Some(&attempted_source),
                    )?;
                    if let Some(admission) = pending_flush.sidecar_admission.take() {
                        self.admitted_sidecar_chunks.push_back(admission);
                    }
                }
                NetworkReplyFlushAckStatus::TimedOut | NetworkReplyFlushAckStatus::Closed => {
                    if matches!(status, NetworkReplyFlushAckStatus::TimedOut) {
                        let target = self
                            .fanouts
                            .get_mut(fanout_index)
                            .and_then(|fanout| fanout.targets.get_mut(target_index))
                            .ok_or_else(|| {
                                "Sumeragi v2 timed-out reply flush lost its target".to_owned()
                            })?;
                        target.reply_writer_timeout_attempt =
                            target.reply_writer_timeout_attempt.saturating_add(1);
                    }
                    let route_state = self
                        .fanouts
                        .get(fanout_index)
                        .and_then(|fanout| fanout.targets.get(target_index))
                        .and_then(|target| match &target.route {
                            ExactTargetRoute::Reply(route) => {
                                Some((route.is_active(), route.is_reply_writable(), target.parked))
                            }
                            ExactTargetRoute::Topology => None,
                        })
                        .ok_or_else(|| {
                            "Sumeragi v2 terminal reply flush lost its route".to_owned()
                        })?;
                    if !route_state.2 {
                        if !route_state.0 {
                            self.retire_inactive_reply_target(fanout_index, target_index)?;
                        } else if !route_state.1 {
                            self.park_unwritable_reply_target(fanout_index, target_index)?;
                        }
                    }
                }
            }
            debug_assert!(self.sidecar_control_units() <= self.sidecar_admission_capacity);
        }
    }
    fn rebase_source_fifo(&mut self) -> Result<(), String> {
        let mut rebuilt = BTreeMap::<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>::new();
        let mut rebased_ids = Vec::with_capacity(self.fanouts.len());
        for (fanout_index, fanout) in self.fanouts.iter().enumerate() {
            let fifo_id = ExactFanoutFifoId::try_from(fanout_index)
                .map_err(|_| "Sumeragi v2 outbound FIFO index is not representable".to_owned())?;
            rebased_ids.push(fifo_id);
            for source in fanout.outstanding_sources()? {
                rebuilt.entry(source).or_default().insert(fifo_id);
            }
        }
        let next_fanout_fifo_id = ExactFanoutFifoId::try_from(self.fanouts.len())
            .map_err(|_| "Sumeragi v2 outbound FIFO sequence is not representable".to_owned())?;
        if next_fanout_fifo_id == ExactFanoutFifoId::MAX {
            return Err("Sumeragi v2 outbound FIFO sequence exhausted".to_owned());
        }
        for (fanout, fifo_id) in self.fanouts.iter_mut().zip(rebased_ids) {
            fanout.fifo_id = Some(fifo_id);
        }
        self.next_fanout_fifo_id = next_fanout_fifo_id;
        self.source_fifo_owners = rebuilt;
        Ok(())
    }
    fn allocate_fanout_fifo_id(&mut self) -> Result<ExactFanoutFifoId, String> {
        if self.next_fanout_fifo_id == ExactFanoutFifoId::MAX {
            self.rebase_source_fifo()?;
        }
        let fifo_id = self.next_fanout_fifo_id;
        if self
            .source_fifo_owners
            .values()
            .any(|owners| owners.contains(&fifo_id))
        {
            return Err("Sumeragi v2 outbound FIFO sequence reused a live identity".to_owned());
        }
        self.next_fanout_fifo_id = fifo_id
            .checked_add(1)
            .ok_or_else(|| "Sumeragi v2 outbound FIFO sequence exhausted".to_owned())?;
        Ok(fifo_id)
    }
    fn unregister_source_fifo_owner(
        &mut self,
        fifo_id: ExactFanoutFifoId,
        source: &ExactTargetSource,
    ) -> Result<(), String> {
        let remove_source = {
            let owners = self
                .source_fifo_owners
                .get_mut(source)
                .ok_or_else(|| "Sumeragi v2 outbound FIFO lost a registered source".to_owned())?;
            if !owners.remove(&fifo_id) {
                return Err("Sumeragi v2 outbound FIFO lost a registered owner".to_owned());
            }
            owners.is_empty()
        };
        if remove_source {
            self.source_fifo_owners.remove(source);
        }
        Ok(())
    }
    fn source_fifo_owners_after_fanout_replacement(
        &self,
        fifo_id: ExactFanoutFifoId,
        prior_sources: &BTreeSet<ExactTargetSource>,
        updated_sources: &BTreeSet<ExactTargetSource>,
    ) -> Result<BTreeMap<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>, String> {
        let indexed_sources = self
            .source_fifo_owners
            .iter()
            .filter_map(|(source, owners)| owners.contains(&fifo_id).then_some(source.clone()))
            .collect::<BTreeSet<_>>();
        if indexed_sources != *prior_sources {
            return Err("Sumeragi v2 outbound FIFO index changed before fanout update".to_owned());
        }
        let mut next = self.source_fifo_owners.clone();
        for source in prior_sources {
            let remove_source = {
                let owners = next
                    .get_mut(source)
                    .expect("preflighted exact-output source owner must remain present");
                let removed = owners.remove(&fifo_id);
                debug_assert!(removed);
                owners.is_empty()
            };
            if remove_source {
                next.remove(source);
            }
        }
        if updated_sources.iter().any(|source| {
            next.get(source)
                .is_some_and(|owners| owners.contains(&fifo_id))
        }) {
            return Err("Sumeragi v2 outbound FIFO registered one owner twice".to_owned());
        }
        for source in updated_sources {
            next.entry(source.clone()).or_default().insert(fifo_id);
        }
        Ok(next)
    }
    fn ownership_addition_load(
        &self,
        additions: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<(usize, usize), String> {
        let mut added_units = 0usize;
        let mut added_shared_units = 0usize;
        for (reservation, added) in additions {
            if *added == 0 {
                return Err("Sumeragi v2 outbound ownership added an empty unit".to_owned());
            }
            added_units = added_units
                .checked_add(*added)
                .ok_or_else(|| "Sumeragi v2 outbound ownership units overflowed".to_owned())?;
            let current = self
                .reservation_owner_counts
                .get(reservation)
                .copied()
                .unwrap_or(0);
            let frozen_credit =
                usize::from(current == 0 && self.reserved_target_classes.contains(reservation));
            added_shared_units = added_shared_units
                .checked_add(added.checked_sub(frozen_credit).ok_or_else(|| {
                    "Sumeragi v2 outbound frozen credit exceeded its ownership".to_owned()
                })?)
                .ok_or_else(|| {
                    "Sumeragi v2 outbound shared ownership units overflowed".to_owned()
                })?;
        }
        Ok((added_units, added_shared_units))
    }
    fn ownership_capacity_available(
        &self,
        additions: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<bool, String> {
        let (added_units, added_shared_units) = self.ownership_addition_load(additions)?;
        Ok(self
            .ownership_units
            .checked_add(added_units)
            .is_some_and(|units| units <= self.ownership_unit_capacity)
            && self
                .shared_ownership_units
                .checked_add(added_shared_units)
                .is_some_and(|units| units <= self.shared_ownership_unit_capacity))
    }
    fn ownership_state_after_additions(
        &self,
        additions: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<(BTreeMap<ExactTargetReservation, usize>, usize, usize), String> {
        let (added_units, added_shared_units) = self.ownership_addition_load(additions)?;
        let next_ownership_units = self
            .ownership_units
            .checked_add(added_units)
            .filter(|units| *units <= self.ownership_unit_capacity)
            .ok_or_else(|| {
                "Sumeragi v2 outbound ownership exceeded its reserved geometry".to_owned()
            })?;
        let next_shared_ownership_units = self
            .shared_ownership_units
            .checked_add(added_shared_units)
            .filter(|units| *units <= self.shared_ownership_unit_capacity)
            .ok_or_else(|| {
                "Sumeragi v2 outbound ownership exceeded its reserved geometry".to_owned()
            })?;
        let mut next_reservation_owner_counts = self.reservation_owner_counts.clone();
        for (reservation, added) in additions {
            let count = next_reservation_owner_counts
                .entry(reservation.clone())
                .or_default();
            *count = count.checked_add(*added).ok_or_else(|| {
                "Sumeragi v2 outbound target/class multiplicity overflowed".to_owned()
            })?;
        }
        Ok((
            next_reservation_owner_counts,
            next_ownership_units,
            next_shared_ownership_units,
        ))
    }
    fn ownership_state_after_removals(
        &self,
        removals: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<(BTreeMap<ExactTargetReservation, usize>, usize, usize), String> {
        let mut next_reservation_owner_counts = self.reservation_owner_counts.clone();
        let mut removed_units = 0usize;
        let mut removed_shared_units = 0usize;
        for (reservation, removed) in removals {
            if *removed == 0 {
                return Err("Sumeragi v2 outbound ownership removed an empty unit".to_owned());
            }
            let current = next_reservation_owner_counts
                .get(reservation)
                .copied()
                .ok_or_else(|| "Sumeragi v2 outbound ownership lost its target/class".to_owned())?;
            let remaining = current.checked_sub(*removed).ok_or_else(|| {
                "Sumeragi v2 outbound ownership removed too many target/class units".to_owned()
            })?;
            removed_units = removed_units
                .checked_add(*removed)
                .ok_or_else(|| "Sumeragi v2 outbound ownership removal overflowed".to_owned())?;
            let frozen_credit_removed =
                usize::from(remaining == 0 && self.reserved_target_classes.contains(reservation));
            removed_shared_units = removed_shared_units
                .checked_add(removed.checked_sub(frozen_credit_removed).ok_or_else(|| {
                    "Sumeragi v2 outbound frozen credit exceeded its removal".to_owned()
                })?)
                .ok_or_else(|| {
                    "Sumeragi v2 outbound shared ownership removal overflowed".to_owned()
                })?;
            if remaining == 0 {
                next_reservation_owner_counts.remove(reservation);
            } else {
                next_reservation_owner_counts.insert(reservation.clone(), remaining);
            }
        }
        let next_ownership_units = self
            .ownership_units
            .checked_sub(removed_units)
            .ok_or_else(|| "Sumeragi v2 outbound ownership total underflowed".to_owned())?;
        let next_shared_ownership_units = self
            .shared_ownership_units
            .checked_sub(removed_shared_units)
            .ok_or_else(|| "Sumeragi v2 outbound shared ownership underflowed".to_owned())?;
        Ok((
            next_reservation_owner_counts,
            next_ownership_units,
            next_shared_ownership_units,
        ))
    }
    fn ownership_state_after_replacement(
        &self,
        removals: &BTreeMap<ExactTargetReservation, usize>,
        additions: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<Option<(BTreeMap<ExactTargetReservation, usize>, usize, usize)>, String> {
        let mut current_units = 0usize;
        let mut current_shared_units = 0usize;
        for (reservation, count) in &self.reservation_owner_counts {
            current_units = current_units.checked_add(*count).ok_or_else(|| {
                "Sumeragi v2 responder-control current ownership overflowed".to_owned()
            })?;
            let frozen_credit = usize::from(self.reserved_target_classes.contains(reservation));
            current_shared_units = current_shared_units
                .checked_add(count.checked_sub(frozen_credit).ok_or_else(|| {
                    "Sumeragi v2 responder-control current ownership lost its frozen credit"
                        .to_owned()
                })?)
                .ok_or_else(|| {
                    "Sumeragi v2 responder-control current shared ownership overflowed".to_owned()
                })?;
        }
        if current_units != self.ownership_units
            || current_shared_units != self.shared_ownership_units
        {
            return Err(
                "Sumeragi v2 responder-control replacement found inconsistent ownership".to_owned(),
            );
        }
        let mut next_reservation_owner_counts = self.reservation_owner_counts.clone();
        for (reservation, removed) in removals {
            if *removed == 0 {
                return Err("Sumeragi v2 outbound ownership replaced an empty unit".to_owned());
            }
            let current = next_reservation_owner_counts
                .get(reservation)
                .copied()
                .ok_or_else(|| {
                    "Sumeragi v2 responder-control replacement lost its target/class".to_owned()
                })?;
            let remaining = current.checked_sub(*removed).ok_or_else(|| {
                "Sumeragi v2 responder-control replacement removed too many units".to_owned()
            })?;
            if remaining == 0 {
                next_reservation_owner_counts.remove(reservation);
            } else {
                next_reservation_owner_counts.insert(reservation.clone(), remaining);
            }
        }
        for (reservation, added) in additions {
            if *added == 0 {
                return Err("Sumeragi v2 outbound ownership replaced with an empty unit".to_owned());
            }
            let count = next_reservation_owner_counts
                .entry(reservation.clone())
                .or_default();
            *count = count.checked_add(*added).ok_or_else(|| {
                "Sumeragi v2 responder-control replacement multiplicity overflowed".to_owned()
            })?;
        }
        let mut next_ownership_units = 0usize;
        let mut next_shared_ownership_units = 0usize;
        for (reservation, count) in &next_reservation_owner_counts {
            next_ownership_units = next_ownership_units.checked_add(*count).ok_or_else(|| {
                "Sumeragi v2 responder-control replacement ownership overflowed".to_owned()
            })?;
            let frozen_credit = usize::from(self.reserved_target_classes.contains(reservation));
            next_shared_ownership_units = next_shared_ownership_units
                .checked_add(count.checked_sub(frozen_credit).ok_or_else(|| {
                    "Sumeragi v2 responder-control replacement lost its frozen credit".to_owned()
                })?)
                .ok_or_else(|| {
                    "Sumeragi v2 responder-control replacement shared ownership overflowed"
                        .to_owned()
                })?;
        }
        if next_ownership_units > self.ownership_unit_capacity
            || next_shared_ownership_units > self.shared_ownership_unit_capacity
        {
            return Ok(None);
        }
        Ok(Some((
            next_reservation_owner_counts,
            next_ownership_units,
            next_shared_ownership_units,
        )))
    }
    fn remove_ownership_units(
        &mut self,
        removals: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<(), String> {
        let (counts, units, shared_units) = self.ownership_state_after_removals(removals)?;
        self.reservation_owner_counts = counts;
        self.ownership_units = units;
        self.shared_ownership_units = shared_units;
        Ok(())
    }
    fn validate_fanout_bounds(&self, fanout: &PendingExactFanout) -> Result<(), String> {
        if fanout.fifo_id.is_some() {
            return Err("Sumeragi v2 outbound fanout already owns a FIFO identity".to_owned());
        }
        if fanout.messages.len() > self.max_messages_per_fanout
            || fanout.peers.len() > self.max_peers_per_fanout
        {
            return Err("Sumeragi v2 outbound fanout exceeds its protocol bound".to_owned());
        }
        if fanout.targets.iter().any(|target| target.parked) {
            return Err("Sumeragi v2 new outbound fanout contains a parked source".to_owned());
        }
        let reply_routes = fanout
            .targets
            .iter()
            .filter_map(|target| match &target.route {
                ExactTargetRoute::Reply(route) => Some(route),
                ExactTargetRoute::Topology => None,
            })
            .collect::<Vec<_>>();
        if !reply_routes.is_empty() {
            if reply_routes.len() != fanout.targets.len() {
                return Err(
                    "Sumeragi v2 outbound fanout mixed topology and reply routes".to_owned(),
                );
            }
            let mut authority = None;
            let mut sources = BTreeSet::new();
            for (route, peer) in reply_routes.iter().copied().zip(&fanout.peers) {
                if !route.is_active() {
                    return Err(
                        "Sumeragi v2 outbound reply fanout contains an inactive capability"
                            .to_owned(),
                    );
                }
                if route.semantic_target() != peer
                    || authority.is_some_and(|prior| !route.same_request_authority(prior))
                {
                    return Err(
                        "Sumeragi v2 outbound reply fanout changed actor or semantic target"
                            .to_owned(),
                    );
                }
                authority.get_or_insert(route);
                if !sources.insert(route.source_key()) {
                    return Err(
                        "Sumeragi v2 outbound reply fanout duplicated an authenticated source"
                            .to_owned(),
                    );
                }
            }
            let history = fanout.reply_routes.as_ref().ok_or_else(|| {
                "Sumeragi v2 outbound reply fanout lost its bounded route history".to_owned()
            })?;
            if history.semantic_target()
                != authority
                    .expect("reply routes established authority")
                    .semantic_target()
                || history.len() != reply_routes.len()
                || history.iter().any(|historical| {
                    !reply_routes
                        .iter()
                        .any(|target| target.same_delivery(historical))
                })
            {
                return Err(
                    "Sumeragi v2 outbound reply fanout route history changed live targets"
                        .to_owned(),
                );
            }
            if let Some(ownership) = &fanout.ingress_ownership
                && (!ownership.validate_exact() || !ownership.matches_reply_routes(Some(history)))
            {
                return Err(
                    "Sumeragi v2 outbound reply fanout changed fair-ingress ownership".to_owned(),
                );
            }
        } else if fanout.reply_routes.is_some() {
            return Err("Sumeragi v2 topology fanout retained reply-route history".to_owned());
        } else if fanout.ingress_ownership.is_some() {
            return Err("Sumeragi v2 topology fanout retained ingress ownership".to_owned());
        }
        if fanout.message_hashes.len() != fanout.messages.len()
            || fanout.message_classes.len() != fanout.messages.len()
            || fanout.message_class_suffixes.len().checked_sub(1) != Some(fanout.messages.len())
        {
            return Err("Sumeragi v2 outbound fanout lost its immutable message index".to_owned());
        }
        if fanout
            .messages
            .iter()
            .zip(&fanout.message_hashes)
            .zip(&fanout.message_classes)
            .any(|((message, expected_hash), expected_class)| {
                HashOf::new(message) != *expected_hash
                    || exact_output_class(message).as_ref() != Ok(expected_class)
            })
        {
            return Err("Sumeragi v2 outbound fanout changed its immutable messages".to_owned());
        }
        if fanout
            .message_class_suffixes
            .last()
            .is_none_or(|suffix| *suffix != 0)
            || fanout
                .message_classes
                .iter()
                .enumerate()
                .any(|(message_index, class)| {
                    let Some(expected_tail) = fanout.message_class_suffixes.get(message_index + 1)
                    else {
                        return true;
                    };
                    let expected_suffix = *expected_tail | exact_output_class_bit(*class);
                    fanout.message_class_suffixes.get(message_index) != Some(&expected_suffix)
                })
        {
            return Err(
                "Sumeragi v2 outbound fanout changed its reliable-class suffixes".to_owned(),
            );
        }
        if fanout.current_source_targets != fanout.expected_current_source_targets()? {
            return Err("Sumeragi v2 outbound fanout changed its local FIFO index".to_owned());
        }
        // Validate every future message class before consulting capacity. An
        // invalid route must never be disguised as temporary backpressure by
        // an already-full corridor.
        let _ = fanout.outstanding_sources()?;
        Ok(())
    }
    fn capacity_available_for(&self, fanout: &PendingExactFanout) -> Result<bool, String> {
        if self
            .fanouts
            .iter()
            .any(|pending| pending.can_coalesce_exact_topology_retry(fanout))
        {
            return Ok(true);
        }
        if self
            .fanouts
            .iter()
            .any(|pending| pending.is_same_acquisition_topology_retry(fanout))
        {
            return Ok(false);
        }
        if let Some(pending) = self
            .fanouts
            .iter()
            .find(|pending| pending.can_coalesce_retry(fanout))
        {
            let plan = pending.reply_target_merge_plan(fanout)?;
            if !self.coalesced_target_geometry_available(pending, &plan)? {
                return Ok(false);
            }
            let additions =
                pending.coalesce_reservation_additions_for_plan(fanout, &plan.targets)?;
            return self.ownership_capacity_available(&additions);
        }
        self.ownership_capacity_available(&fanout.admission_reservation_counts()?)
    }
    fn coalesced_target_geometry_available(
        &self,
        pending: &PendingExactFanout,
        plan: &ReplyTargetMergePlan,
    ) -> Result<bool, String> {
        let appended = plan
            .targets
            .iter()
            .filter(|merge| matches!(merge, ReplyTargetMerge::Append { .. }))
            .count();
        let target_count = pending
            .targets
            .len()
            .checked_add(appended)
            .ok_or_else(|| "Sumeragi v2 reply target geometry overflowed".to_owned())?;
        Ok(target_count <= self.max_peers_per_fanout
            && target_count <= plan.reply_routes.source_capacity())
    }
    fn retains_retryable_sidecar_responder_control_for(
        &self,
        candidate: &PendingExactFanout,
    ) -> bool {
        candidate
            .retryable_certified_sidecar_responder_control_target()
            .is_some_and(|candidate_target| {
                self.fanouts.iter().any(|retained| {
                    retained.retryable_certified_sidecar_responder_control_target()
                        == Some(candidate_target)
                })
            })
    }
    fn stranded_responder_control_replacement_index(
        &self,
        candidate: &PendingExactFanout,
    ) -> Option<usize> {
        let candidate_target = candidate.retryable_certified_sidecar_responder_control_target()?;
        if !candidate.has_writable_reply_target() {
            return None;
        }
        self.fanouts.iter().position(|retained| {
            retained.retryable_certified_sidecar_responder_control_target()
                == Some(candidate_target)
                && retained.is_stranded_retryable_certified_sidecar_responder_control()
        })
    }
    fn responder_control_replacement_ownership(
        &self,
        retained_index: usize,
        candidate: &PendingExactFanout,
    ) -> Result<Option<(BTreeMap<ExactTargetReservation, usize>, usize, usize)>, String> {
        let retained = self
            .fanouts
            .get(retained_index)
            .ok_or_else(|| "Sumeragi v2 stranded responder control disappeared".to_owned())?;
        let retained_fifo_id = retained.fifo_id.ok_or_else(|| {
            "Sumeragi v2 stranded responder control lost its FIFO identity".to_owned()
        })?;
        let retained_sources = retained.outstanding_sources()?;
        let indexed_sources = self
            .source_fifo_owners
            .iter()
            .filter_map(|(source, owners)| {
                owners.contains(&retained_fifo_id).then_some(source.clone())
            })
            .collect::<BTreeSet<_>>();
        if indexed_sources != retained_sources {
            return Err(
                "Sumeragi v2 stranded responder control changed its FIFO ownership".to_owned(),
            );
        }
        self.ownership_state_after_replacement(
            &retained.outstanding_reservation_counts()?,
            &candidate.outstanding_reservation_counts()?,
        )
    }
    fn responder_control_replacement_available(
        &self,
        candidate: &PendingExactFanout,
    ) -> Result<bool, String> {
        let Some(retained_index) = self.stranded_responder_control_replacement_index(candidate)
        else {
            return Ok(false);
        };
        Ok(self
            .responder_control_replacement_ownership(retained_index, candidate)?
            .is_some())
    }
    fn responder_control_replacement_plan(
        &self,
        retained_index: usize,
        candidate: &PendingExactFanout,
    ) -> Result<Option<ResponderControlReplacementPlan>, String> {
        let Some((reservation_owner_counts, ownership_units, shared_ownership_units)) =
            self.responder_control_replacement_ownership(retained_index, candidate)?
        else {
            return Ok(None);
        };
        let retained = self
            .fanouts
            .get(retained_index)
            .expect("located stranded responder control must remain present");
        let retained_fifo_id = retained
            .fifo_id
            .expect("preflighted responder control retains its FIFO identity");
        let replacement_fifo_id = self.next_fanout_fifo_id;
        let next_fanout_fifo_id = replacement_fifo_id.checked_add(1).ok_or_else(|| {
            "Sumeragi v2 outbound FIFO must rebase before responder-control replacement".to_owned()
        })?;
        if self
            .source_fifo_owners
            .values()
            .any(|owners| owners.contains(&replacement_fifo_id))
        {
            return Err(
                "Sumeragi v2 responder-control replacement reused a live FIFO identity".to_owned(),
            );
        }
        let fanout_count = self.fanouts.len();
        if fanout_count == 0 || self.next_fanout_index >= fanout_count {
            return Err(
                "Sumeragi v2 responder-control replacement found an invalid scheduler cursor"
                    .to_owned(),
            );
        }
        let next_fanout_index = if fanout_count == 1 {
            0
        } else if self.next_fanout_index == retained_index {
            if retained_index + 1 < fanout_count {
                // Removing the retained slot shifts its successor into the
                // same index. The fresh replacement rejoins at the tail.
                retained_index
            } else {
                // The retired slot was last, so continue at the old wrap
                // point instead of granting the replacement that position.
                0
            }
        } else if self.next_fanout_index > retained_index {
            self.next_fanout_index - 1
        } else {
            self.next_fanout_index
        };
        let retained_sources = retained.outstanding_sources()?;
        let replacement_sources = candidate.outstanding_sources()?;
        let mut source_fifo_owners = self.source_fifo_owners.clone();
        for source in &retained_sources {
            let remove_source = {
                let owners = source_fifo_owners.get_mut(source).ok_or_else(|| {
                    "Sumeragi v2 responder-control replacement lost a registered source".to_owned()
                })?;
                if !owners.remove(&retained_fifo_id) {
                    return Err(
                        "Sumeragi v2 responder-control replacement lost its registered owner"
                            .to_owned(),
                    );
                }
                owners.is_empty()
            };
            if remove_source {
                source_fifo_owners.remove(source);
            }
        }
        for source in replacement_sources {
            if !source_fifo_owners
                .entry(source)
                .or_default()
                .insert(replacement_fifo_id)
            {
                return Err(
                    "Sumeragi v2 responder-control replacement registered one source twice"
                        .to_owned(),
                );
            }
        }
        Ok(Some(ResponderControlReplacementPlan {
            retained_index,
            replacement_fifo_id,
            next_fanout_fifo_id,
            next_fanout_index,
            source_fifo_owners,
            reservation_owner_counts,
            ownership_units,
            shared_ownership_units,
        }))
    }
    fn commit_stranded_responder_control_replacement(
        &mut self,
        mut candidate: PendingExactFanout,
    ) -> Result<Option<PendingExactFanout>, String> {
        let Some(retained_index) = self.stranded_responder_control_replacement_index(&candidate)
        else {
            return Ok(None);
        };
        // Capacity failure must not rebase live FIFO identities. Establish
        // that the replacement fits at the same liveness snapshot before the
        // only preparatory mutation. Reply writability is monotonic within a
        // tenure, so the plan below deliberately reuses this retained index
        // instead of rereading external route state after a FIFO rebase.
        if self
            .responder_control_replacement_ownership(retained_index, &candidate)?
            .is_none()
        {
            return Ok(None);
        }
        if self.fanouts.is_empty() || self.next_fanout_index >= self.fanouts.len() {
            return Err(
                "Sumeragi v2 responder-control replacement found an invalid scheduler cursor"
                    .to_owned(),
            );
        }
        if self.next_fanout_fifo_id == ExactFanoutFifoId::MAX {
            self.rebase_source_fifo()?;
        }
        let Some(plan) = self.responder_control_replacement_plan(retained_index, &candidate)?
        else {
            return Ok(None);
        };
        candidate.fifo_id = Some(plan.replacement_fifo_id);
        let retired = self
            .fanouts
            .remove(plan.retained_index)
            .expect("planned stranded responder control must remain present");
        // This is new authenticated-source work. Appending it keeps deque
        // round-robin age aligned with the fresh source FIFO identity, even
        // after a later FIFO rebase.
        self.fanouts.push_back(candidate);
        self.next_fanout_fifo_id = plan.next_fanout_fifo_id;
        self.next_fanout_index = plan.next_fanout_index;
        self.source_fifo_owners = plan.source_fifo_owners;
        self.reservation_owner_counts = plan.reservation_owner_counts;
        self.ownership_units = plan.ownership_units;
        self.shared_ownership_units = plan.shared_ownership_units;
        Ok(Some(retired))
    }
    fn replace_stranded_responder_control(
        &mut self,
        candidate: PendingExactFanout,
    ) -> Result<bool, String> {
        let Some(retired) = self.commit_stranded_responder_control_replacement(candidate)? else {
            return Ok(false);
        };
        // Actor-ticket destruction can emit cancellation. Keep that external
        // side effect strictly after every worker-owned index is committed.
        drop(retired);
        Ok(true)
    }
    fn can_enqueue(&self, fanout: &PendingExactFanout) -> Result<bool, String> {
        self.validate_fanout_bounds(fanout)?;
        if self
            .stranded_responder_control_replacement_index(fanout)
            .is_some()
        {
            return self.responder_control_replacement_available(fanout);
        }
        if self
            .fanouts
            .iter()
            .any(|pending| pending.can_coalesce_exact_topology_retry(fanout))
        {
            return Ok(true);
        }
        if self
            .fanouts
            .iter()
            .any(|pending| pending.is_same_acquisition_topology_retry(fanout))
        {
            return Ok(false);
        }
        if self
            .fanouts
            .iter()
            .any(|pending| pending.can_coalesce_retry(fanout))
        {
            return self.capacity_available_for(fanout);
        }
        if self.retains_retryable_sidecar_responder_control_for(fanout) {
            // Preserve one bounded successor in lane work while the incumbent
            // still has a writer or a pending flush. Consuming a distinct
            // control here would lose the newest cumulative CloseAck or the
            // GenerationHint for the request hash the client actually retains.
            return Ok(false);
        }
        self.capacity_available_for(fanout)
    }
    fn validate_owned_reply_transfer(
        &self,
        fanout: &mut PendingExactFanout,
    ) -> Result<bool, String> {
        loop {
            if fanout.retain_active_unowned_reply_targets()? == 0 {
                return Ok(false);
            }
            match self.validate_fanout_bounds(fanout) {
                Ok(()) => return Ok(true),
                Err(error)
                    if fanout.targets.iter().any(
                        |target| matches!(&target.route, ExactTargetRoute::Reply(route) if !route.is_active()),
                    ) =>
                {
                    // A tenure retired between pruning and validation. Active
                    // is monotonic, so each retry removes at least one route.
                    drop(error);
                }
                Err(error) => return Err(error),
            }
        }
    }
    fn can_enqueue_owned_reply_transfer(
        &self,
        mut fanout: PendingExactFanout,
    ) -> Result<bool, String> {
        if !self.validate_owned_reply_transfer(&mut fanout)? {
            return Ok(true);
        }
        self.project_sidecar_receipt_completions(&mut fanout)?;
        if self
            .stranded_responder_control_replacement_index(&fanout)
            .is_some()
        {
            return self.responder_control_replacement_available(&fanout);
        }
        if self
            .fanouts
            .iter()
            .any(|pending| pending.can_coalesce_retry(&fanout))
        {
            return self.capacity_available_for(&fanout);
        }
        if self.retains_retryable_sidecar_responder_control_for(&fanout) {
            return Ok(false);
        }
        self.capacity_available_for(&fanout)
    }
    fn enqueue(&mut self, fanout: PendingExactFanout) -> Result<ExactFanoutOwnership, String> {
        self.validate_fanout_bounds(&fanout)?;
        self.enqueue_validated(fanout)
    }
    fn enqueue_owned_reply_transfer(
        &mut self,
        mut fanout: PendingExactFanout,
    ) -> Result<ExactFanoutOwnership, String> {
        if !self.validate_owned_reply_transfer(&mut fanout)? {
            return Ok(ExactFanoutOwnership::Owned);
        }
        self.project_sidecar_receipt_completions(&mut fanout)?;
        self.enqueue_validated(fanout)
    }
    /// Coalesce post-flush reply redelivery while ordinary fanout ownership and
    /// cursor stay on the target; only the receipt needs terminal projection.
    fn project_sidecar_receipt_completions(
        &self,
        fanout: &mut PendingExactFanout,
    ) -> Result<(), String> {
        let [message] = fanout.messages.as_slice() else {
            return Ok(());
        };
        let NetworkMessage::CertifiedMergeSidecar(message) = message else {
            return Ok(());
        };
        let CertifiedMergeSidecarMessage::Chunk(_) = message.as_ref() else {
            return Ok(());
        };
        let completed_cursor = fanout.messages.len();
        let completed_message_cursor = u64::try_from(completed_cursor)
            .map_err(|_| "Sumeragi v2 sidecar replay cursor exceeded u64".to_owned())?;
        let mut completed_routes = Vec::new();
        let mut projected_completion = false;
        for target in &mut fanout.targets {
            if target.message_index == completed_cursor {
                continue;
            }
            if target.message_index != 0 || target.current.is_some() || target.ticket.is_some() {
                return Err(
                    "Sumeragi v2 sidecar replay carried pre-existing exact-output state".to_owned(),
                );
            }
            let ExactTargetRoute::Reply(route) = &target.route else {
                continue;
            };
            let source_terminal = self.admitted_sidecar_chunks.iter().any(|admission| {
                admission.matches_materialized_chunk(message) && admission.is_bound_to_source(route)
            });
            if source_terminal {
                target.message_index = completed_cursor;
                completed_routes.push(route.clone());
                projected_completion = true;
            }
        }
        if !completed_routes.is_empty() {
            if let Some(ownership) = fanout.ingress_ownership.as_mut() {
                for route in &completed_routes {
                    if !ownership.advance_reply_cursors(route, completed_message_cursor, 0) {
                        return Err(
                            "Sumeragi v2 retained sidecar flush lost fair-ingress ownership"
                                .to_owned(),
                        );
                    }
                }
            }
        }
        if projected_completion {
            fanout.rebuild_current_source_targets()?;
        }
        Ok(())
    }
    fn enqueue_validated(
        &mut self,
        mut fanout: PendingExactFanout,
    ) -> Result<ExactFanoutOwnership, String> {
        if fanout.is_complete() {
            return Ok(ExactFanoutOwnership::Owned);
        }
        if self
            .stranded_responder_control_replacement_index(&fanout)
            .is_some()
        {
            return self
                .replace_stranded_responder_control(fanout)
                .map(|replaced| {
                    if replaced {
                        ExactFanoutOwnership::Owned
                    } else {
                        ExactFanoutOwnership::SourceRetained
                    }
                });
        }
        if self
            .fanouts
            .iter()
            .any(|pending| pending.can_coalesce_exact_topology_retry(&fanout))
        {
            return Ok(ExactFanoutOwnership::Owned);
        }
        if self
            .fanouts
            .iter()
            .any(|pending| pending.is_same_acquisition_topology_retry(&fanout))
        {
            // The task/discovery source retains rotated target batches while
            // the incumbent owns actor rank. Once that fanout drains, a later
            // source retry may install the next bounded batch.
            return Ok(ExactFanoutOwnership::SourceRetained);
        }
        if let Some(index) = self
            .fanouts
            .iter()
            .position(|pending| pending.can_coalesce_retry(&fanout))
        {
            let (fifo_id, prior_sources, plan, preview, ownership_additions) = {
                let pending = self
                    .fanouts
                    .get(index)
                    .expect("located exact-output retry must remain present");
                if pending.current_source_targets != pending.expected_current_source_targets()? {
                    return Err(
                        "Sumeragi v2 retained fanout changed its local FIFO index".to_owned()
                    );
                }
                let fifo_id = pending.fifo_id.ok_or_else(|| {
                    "Sumeragi v2 retained fanout lost its FIFO identity".to_owned()
                })?;
                let plan = pending.reply_target_merge_plan(&fanout)?;
                let preview = pending.preview_coalesce_plan(&fanout, &plan)?;
                let ownership_additions =
                    pending.coalesce_reservation_additions_for_plan(&fanout, &plan.targets)?;
                (
                    fifo_id,
                    pending.outstanding_sources()?,
                    plan,
                    preview,
                    ownership_additions,
                )
            };
            let next_source_fifo_owners = self.source_fifo_owners_after_fanout_replacement(
                fifo_id,
                &prior_sources,
                &preview.outstanding_sources,
            )?;
            if plan.targets.is_empty() {
                self.fanouts
                    .get_mut(index)
                    .expect("located exact-output retry must remain present")
                    .commit_coalesce_plan(&fanout, &plan, preview.current_source_targets);
                self.source_fifo_owners = next_source_fifo_owners;
                return Ok(ExactFanoutOwnership::Owned);
            }
            if !self.coalesced_target_geometry_available(
                self.fanouts
                    .get(index)
                    .expect("located exact-output retry must remain present"),
                &plan,
            )? || !self.ownership_capacity_available(&ownership_additions)?
            {
                return Ok(ExactFanoutOwnership::SourceRetained);
            }
            let (next_reservation_owner_counts, next_ownership_units, next_shared_ownership_units) =
                self.ownership_state_after_additions(&ownership_additions)?;
            self.fanouts
                .get_mut(index)
                .expect("located exact-output retry must remain present")
                .commit_coalesce_plan(&fanout, &plan, preview.current_source_targets);
            self.source_fifo_owners = next_source_fifo_owners;
            self.reservation_owner_counts = next_reservation_owner_counts;
            self.ownership_units = next_ownership_units;
            self.shared_ownership_units = next_shared_ownership_units;
            return Ok(ExactFanoutOwnership::Owned);
        }
        if self.retains_retryable_sidecar_responder_control_for(&fanout) {
            // At most one responder control per semantic target owns this
            // corridor. Lane work retains the distinct successor until the
            // incumbent drains or becomes safely replaceable.
            return Ok(ExactFanoutOwnership::SourceRetained);
        }
        let ownership_additions = fanout.outstanding_reservation_counts()?;
        if !self.ownership_capacity_available(&ownership_additions)? {
            return Ok(ExactFanoutOwnership::SourceRetained);
        }
        let (next_reservation_owner_counts, next_ownership_units, next_shared_ownership_units) =
            self.ownership_state_after_additions(&ownership_additions)?;
        let sources = fanout.outstanding_sources()?;
        let fifo_id = self.allocate_fanout_fifo_id()?;
        let mut next_source_fifo_owners = self.source_fifo_owners.clone();
        debug_assert!(
            next_source_fifo_owners
                .values()
                .all(|owners| !owners.contains(&fifo_id))
        );
        for source in sources {
            next_source_fifo_owners
                .entry(source)
                .or_default()
                .insert(fifo_id);
        }
        fanout.fifo_id = Some(fifo_id);
        self.source_fifo_owners = next_source_fifo_owners;
        self.reservation_owner_counts = next_reservation_owner_counts;
        self.ownership_units = next_ownership_units;
        self.shared_ownership_units = next_shared_ownership_units;
        self.fanouts.push_back(fanout);
        Ok(ExactFanoutOwnership::Owned)
    }
    fn handoff_applied_height_to_durable_reconstruction(
        &mut self,
        artifact: &wire::finality::V2FinalityArtifact,
        durable_lane_authority: Option<&DurableLaneRolloverAuthority>,
        durable_history: Option<&Kura>,
    ) -> Result<usize, String> {
        let mut remaining_posts = 0usize;
        let mut expected_source_fifo_owners =
            BTreeMap::<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>::new();
        let mut expected_reservation_owner_counts =
            BTreeMap::<ExactTargetReservation, usize>::new();
        for fanout in &self.fanouts {
            if let Some(ownership) = &fanout.ingress_ownership
                && (fanout
                    .reply_routes
                    .as_ref()
                    .is_none_or(|routes| !ownership.matches_reply_routes(Some(routes)))
                    || !ownership.validate_exact())
            {
                return Err(
                    "Sumeragi v2 finalized output changed fair-ingress ownership".to_owned(),
                );
            }
            if fanout.message_hashes.len() != fanout.messages.len()
                || fanout
                    .messages
                    .iter()
                    .zip(&fanout.message_hashes)
                    .any(|(message, expected_hash)| HashOf::new(message) != *expected_hash)
            {
                return Err(
                    "Sumeragi v2 retained output changed before finality handoff".to_owned(),
                );
            }
            let fifo_id = fanout.fifo_id.ok_or_else(|| {
                "Sumeragi v2 retained fanout lost its FIFO identity before finality handoff"
                    .to_owned()
            })?;
            for source in fanout.outstanding_sources()? {
                expected_source_fifo_owners
                    .entry(source)
                    .or_default()
                    .insert(fifo_id);
            }
            for (reservation, count) in fanout.outstanding_reservation_counts()? {
                let aggregate = expected_reservation_owner_counts
                    .entry(reservation)
                    .or_default();
                *aggregate = aggregate.checked_add(count).ok_or_else(|| {
                    "Sumeragi v2 outbound handoff ownership count overflowed".to_owned()
                })?;
            }
            applied_height_reconstruction_covers(
                &fanout.messages,
                &fanout.semantic_peers(),
                &fanout.rollover_claim,
                artifact,
                durable_lane_authority,
                durable_history,
            )?;
            for (target_index, target) in fanout.targets.iter().enumerate() {
                if target.message_index > fanout.messages.len() {
                    return Err(
                        "Sumeragi v2 exact-output target advanced beyond its fanout".to_owned()
                    );
                }
                if target.ticket.is_some() && target.current.is_none() {
                    return Err("Sumeragi v2 exact-output ticket lost its returned post".to_owned());
                }
                if target.parked
                    && (!matches!(
                        &target.route,
                        ExactTargetRoute::Reply(route)
                            if !route.is_active() || !route.is_reply_writable()
                    ) || target.current.is_some()
                        || target.ticket.is_some()
                        || fanout.target_is_complete(target_index))
                {
                    return Err(
                        "Sumeragi v2 parked reply source changed before finality handoff"
                            .to_owned(),
                    );
                }
                if let Some(current) = &target.current {
                    if fanout.peers.get(target_index) != Some(&current.peer_id) {
                        return Err(
                            "Sumeragi v2 exact-output target changed before finality handoff"
                                .to_owned(),
                        );
                    }
                    let expected_hash = fanout
                        .message_hashes
                        .get(target.message_index)
                        .ok_or_else(|| {
                            "Sumeragi v2 exact-output target has no expected payload identity"
                                .to_owned()
                        })?;
                    if HashOf::new(&current.data) != *expected_hash {
                        return Err(
                            "Sumeragi v2 returned output changed before finality handoff"
                                .to_owned(),
                        );
                    }
                }
                if let Some(pending_flush) = &target.pending_flush {
                    if target.current.is_some() || target.ticket.is_some() {
                        return Err(
                            "Sumeragi v2 writer flush shared tenure-bound actor ownership"
                                .to_owned(),
                        );
                    }
                    let data = fanout.messages.get(target.message_index).ok_or_else(|| {
                        "Sumeragi v2 writer flush advanced beyond its immutable payload".to_owned()
                    })?;
                    let peer_id = fanout.peers.get(target_index).ok_or_else(|| {
                        "Sumeragi v2 writer flush lost its semantic target".to_owned()
                    })?;
                    let canonical_post = Post {
                        data: data.clone(),
                        peer_id: peer_id.clone(),
                        priority: Priority::High,
                    };
                    let ExactTargetRoute::Reply(route) = &target.route else {
                        return Err(
                            "Sumeragi v2 topology target retained a reply writer flush".to_owned()
                        );
                    };
                    if !pending_flush
                        .flush_ack
                        .identity()
                        .is_bound_to_canonical_reply(&canonical_post)
                        || pending_flush.flush_ack.identity().source_key() != route.source_key()
                        || pending_flush.reply_writer_timeout_attempt
                            != target.reply_writer_timeout_attempt
                        || pending_flush
                            .flush_ack
                            .identity()
                            .reply_writer_timeout_attempt()
                            != pending_flush.reply_writer_timeout_attempt
                        || pending_flush
                            .sidecar_admission
                            .as_ref()
                            .is_some_and(|admission| {
                                !admission.matches_ack_identity(pending_flush.flush_ack.identity())
                            })
                    {
                        return Err(
                            "Sumeragi v2 writer flush changed before finality handoff".to_owned()
                        );
                    }
                }
                for _message in &fanout.messages[target.message_index..] {
                    remaining_posts = remaining_posts.checked_add(1).ok_or_else(|| {
                        "Sumeragi v2 applied-height output count overflowed".to_owned()
                    })?;
                }
            }
        }
        if self.source_fifo_owners != expected_source_fifo_owners {
            return Err(
                "Sumeragi v2 outbound FIFO index changed before finality handoff".to_owned(),
            );
        }
        if self.reservation_owner_counts != expected_reservation_owner_counts {
            return Err(
                "Sumeragi v2 outbound ownership index changed before finality handoff".to_owned(),
            );
        }
        let mut expected_ownership_units = 0usize;
        let mut expected_shared_ownership_units = 0usize;
        for (reservation, count) in &expected_reservation_owner_counts {
            expected_ownership_units = expected_ownership_units
                .checked_add(*count)
                .ok_or_else(|| "Sumeragi v2 outbound handoff units overflowed".to_owned())?;
            let frozen_credit = usize::from(self.reserved_target_classes.contains(reservation));
            expected_shared_ownership_units = expected_shared_ownership_units
                .checked_add(count.checked_sub(frozen_credit).ok_or_else(|| {
                    "Sumeragi v2 outbound handoff lost its frozen ownership credit".to_owned()
                })?)
                .ok_or_else(|| "Sumeragi v2 outbound handoff shared units overflowed".to_owned())?;
        }
        if self.ownership_units != expected_ownership_units
            || self.shared_ownership_units != expected_shared_ownership_units
        {
            return Err(
                "Sumeragi v2 outbound ownership totals changed before finality handoff".to_owned(),
            );
        }
        // Pending sidecar writer occurrences remain in their target's suffix
        // and were counted above. Only flushed receipts live beyond a fanout.
        let sidecar_completions = self.admitted_sidecar_chunks.len();
        remaining_posts = remaining_posts
            .checked_add(sidecar_completions)
            .ok_or_else(|| "Sumeragi v2 applied-height output count overflowed".to_owned())?;
        self.fanouts.clear();
        // The per-height lane transport and worker are dropped together.
        // Pending target acknowledgements and flushed-but-unapplied receipts
        // are atomically superseded by the typed Kura reconstruction claim;
        // retaining either here would let an unresponsive requester block the
        // decided height's successor activation.
        self.admitted_sidecar_chunks.clear();
        self.next_fanout_index = 0;
        self.next_fanout_fifo_id = 0;
        self.source_fifo_owners.clear();
        self.reservation_owner_counts.clear();
        self.ownership_units = 0;
        self.shared_ownership_units = 0;
        Ok(remaining_posts)
    }
    fn target_is_global_head(
        &self,
        fanout_index: usize,
        target_index: usize,
    ) -> Result<bool, String> {
        let fanout = self
            .fanouts
            .get(fanout_index)
            .ok_or_else(|| "Sumeragi v2 exact-output fanout disappeared".to_owned())?;
        if !fanout.target_is_local_head(target_index)? {
            return Ok(false);
        }
        let source = fanout.current_target_source(target_index)?;
        let fifo_id = fanout
            .fifo_id
            .ok_or_else(|| "Sumeragi v2 retained fanout lost its FIFO identity".to_owned())?;
        let owners = self
            .source_fifo_owners
            .get(&source)
            .ok_or_else(|| "Sumeragi v2 outbound FIFO lost its current source".to_owned())?;
        if !owners.contains(&fifo_id) {
            return Err("Sumeragi v2 outbound FIFO lost its current owner".to_owned());
        }
        let oldest_owner = owners
            .first()
            .expect("non-empty exact-output source owner set has a first entry");
        Ok(*oldest_owner == fifo_id)
    }
    fn next_schedulable_target(
        &self,
        blocked_sources: &BTreeSet<ExactTargetSource>,
    ) -> Result<Option<(usize, usize)>, String> {
        let fanout_count = self.fanouts.len();
        for fanout_offset in 0..fanout_count {
            let fanout_index = (self.next_fanout_index + fanout_offset) % fanout_count;
            let fanout = self
                .fanouts
                .get(fanout_index)
                .expect("round-robin exact fanout index must be present");
            for target_offset in 0..fanout.targets.len() {
                let target_index =
                    (fanout.next_target_index + target_offset) % fanout.targets.len();
                if fanout.target_is_complete(target_index) {
                    continue;
                }
                if fanout.targets[target_index].parked
                    || fanout.targets[target_index].pending_flush.is_some()
                {
                    continue;
                }
                let source = fanout.current_target_source(target_index)?;
                if !blocked_sources.contains(&source)
                    && self.target_is_global_head(fanout_index, target_index)?
                {
                    return Ok(Some((fanout_index, target_index)));
                }
            }
        }
        Ok(None)
    }
    /// Whether a FIFO head awaits reply-route activity; later local fanouts may
    /// proceed while it waits for reconnect or flush acknowledgement.
    fn has_quiescent_fifo_head(&self) -> Result<bool, String> {
        for (fanout_index, fanout) in self.fanouts.iter().enumerate() {
            for (target_index, target) in fanout.targets.iter().enumerate() {
                if fanout.target_is_complete(target_index)
                    || (!target.parked && target.pending_flush.is_none())
                {
                    continue;
                }
                if self.target_is_global_head(fanout_index, target_index)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    fn next_inactive_reply_target(&self) -> Option<(usize, usize)> {
        let fanout_count = self.fanouts.len();
        for fanout_offset in 0..fanout_count {
            let fanout_index = (self.next_fanout_index + fanout_offset) % fanout_count;
            let fanout = self
                .fanouts
                .get(fanout_index)
                .expect("round-robin exact fanout index must be present");
            for target_offset in 0..fanout.targets.len() {
                let target_index =
                    (fanout.next_target_index + target_offset) % fanout.targets.len();
                if fanout.target_is_complete(target_index) || fanout.targets[target_index].parked {
                    continue;
                }
                if matches!(
                    &fanout.targets[target_index].route,
                    ExactTargetRoute::Reply(route) if !route.is_active()
                ) {
                    return Some((fanout_index, target_index));
                }
            }
        }
        None
    }
    fn advance_after_attempt(
        &mut self,
        fanout_index: usize,
        target_index: usize,
        admitted_source: Option<&ExactTargetSource>,
    ) -> Result<(), String> {
        let (fanout_complete, released_reservation, released_source_owner) = {
            let fanout = self
                .fanouts
                .get_mut(fanout_index)
                .expect("attempted exact fanout must remain present");
            fanout.advance_target_cursor(target_index);
            let fanout_complete = fanout.is_complete();
            let released_reservation = if let Some(source) = admitted_source {
                let target = fanout
                    .targets
                    .get(target_index)
                    .ok_or_else(|| "Sumeragi v2 exact-output target disappeared".to_owned())?;
                let remaining_mask = *fanout
                    .message_class_suffixes
                    .get(target.message_index)
                    .ok_or_else(|| {
                        "Sumeragi v2 exact-output target advanced beyond its class suffix"
                            .to_owned()
                    })?;
                if remaining_mask & exact_output_class_bit(source.class) != 0 {
                    None
                } else {
                    let semantic_target = fanout
                        .peers
                        .get(target_index)
                        .expect("selected exact-output target must retain its peer");
                    let reservation = fanout.target_reservation(semantic_target, source.class);
                    if reservation.kind == ExactTargetReservationKind::SidecarReplyControl
                        && fanout
                            .outstanding_reservation_counts()?
                            .contains_key(&reservation)
                    {
                        None
                    } else {
                        Some(reservation)
                    }
                }
            } else {
                None
            };
            let released_source_owner = if let Some(source) = admitted_source {
                if fanout.owns_source(source)? {
                    None
                } else {
                    Some(fanout.fifo_id.ok_or_else(|| {
                        "Sumeragi v2 retained fanout lost its FIFO identity".to_owned()
                    })?)
                }
            } else {
                None
            };
            Ok::<_, String>((fanout_complete, released_reservation, released_source_owner))
        }?;
        if let Some(reservation) = released_reservation {
            self.remove_ownership_units(&BTreeMap::from([(reservation, 1)]))?;
        }
        if let (Some(fifo_id), Some(source)) = (released_source_owner, admitted_source) {
            self.unregister_source_fifo_owner(fifo_id, source)?;
        }
        if fanout_complete {
            let fifo_id = self
                .fanouts
                .get(fanout_index)
                .and_then(|fanout| fanout.fifo_id)
                .ok_or_else(|| "Sumeragi v2 completed fanout lost its FIFO identity".to_owned())?;
            if self
                .source_fifo_owners
                .values()
                .any(|owners| owners.contains(&fifo_id))
            {
                return Err("Sumeragi v2 completed fanout retained a FIFO source".to_owned());
            }
            self.fanouts
                .remove(fanout_index)
                .expect("completed exact fanout must remain present");
            self.next_fanout_index = if self.fanouts.is_empty() {
                0
            } else {
                fanout_index % self.fanouts.len()
            };
        } else {
            self.next_fanout_index = (fanout_index + 1) % self.fanouts.len();
        }
        Ok(())
    }
    fn park_unwritable_reply_target(
        &mut self,
        fanout_index: usize,
        target_index: usize,
    ) -> Result<(), String> {
        {
            let fanout = self
                .fanouts
                .get(fanout_index)
                .ok_or_else(|| "Sumeragi v2 draining fanout disappeared".to_owned())?;
            let target = fanout
                .targets
                .get(target_index)
                .ok_or_else(|| "Sumeragi v2 draining reply target disappeared".to_owned())?;
            match &target.route {
                // Reply writability is monotone within one tenure. The final
                // receiver may retire after the actor reports Unavailable or
                // closes a flush acknowledgement, so an inactive route is a
                // valid later observation of the same draining occurrence.
                ExactTargetRoute::Reply(route) if !route.is_reply_writable() => {}
                ExactTargetRoute::Reply(_) => {
                    return Err("Sumeragi v2 attempted to park a writable reply route".to_owned());
                }
                ExactTargetRoute::Topology => {
                    return Err("Sumeragi v2 attempted to park a topology target".to_owned());
                }
            }
            if target.parked || target.pending_flush.is_some() {
                return Err(
                    "Sumeragi v2 attempted to park an owned or already parked reply target"
                        .to_owned(),
                );
            }
            if fanout.target_is_complete(target_index) || fanout.fifo_id.is_none() {
                return Err("Sumeragi v2 draining reply target lost cursor ownership".to_owned());
            }
            let _ = fanout.outstanding_sources()?;
            let _ = fanout.outstanding_reservation_counts()?;
        }
        let fanout = self
            .fanouts
            .get_mut(fanout_index)
            .expect("preflighted draining fanout must remain present");
        let target = fanout
            .targets
            .get_mut(target_index)
            .expect("preflighted draining target must remain present");
        target.current = None;
        target.ticket = None;
        target.parked = true;
        // Preserve route history, immutable payload, message cursor, FIFO age,
        // and reservation ownership. A newer same-source tenure updates this
        // exact target and retries its current item.
        fanout.advance_target_cursor(target_index);
        self.next_fanout_index = (fanout_index + 1) % self.fanouts.len();
        Ok(())
    }
    fn retire_inactive_reply_target(
        &mut self,
        fanout_index: usize,
        target_index: usize,
    ) -> Result<(), String> {
        {
            let fanout = self
                .fanouts
                .get(fanout_index)
                .ok_or_else(|| "Sumeragi v2 retired fanout disappeared".to_owned())?;
            let target = fanout
                .targets
                .get(target_index)
                .ok_or_else(|| "Sumeragi v2 retired reply target disappeared".to_owned())?;
            match &target.route {
                ExactTargetRoute::Reply(route) if !route.is_active() => {}
                ExactTargetRoute::Reply(_) => {
                    return Err("Sumeragi v2 attempted to retire an active reply route".to_owned());
                }
                ExactTargetRoute::Topology => {
                    return Err("Sumeragi v2 attempted to retire a topology target".to_owned());
                }
            }
            if target.parked {
                return Err("Sumeragi v2 attempted to park one reply target twice".to_owned());
            }
            if fanout.reply_routes.is_none() {
                return Err(
                    "Sumeragi v2 retired reply fanout lost its bounded route history".to_owned(),
                );
            }
            if fanout.current_source_targets != fanout.expected_current_source_targets()? {
                return Err(
                    "Sumeragi v2 retired reply fanout changed its local FIFO index".to_owned(),
                );
            }
            if fanout.target_is_complete(target_index) {
                return Err("Sumeragi v2 attempted to park a completed reply source".to_owned());
            }
            if fanout.fifo_id.is_none() {
                return Err("Sumeragi v2 retired fanout lost its FIFO identity".to_owned());
            }
            // Validate the retained source and reservation projections before
            // changing tenure-bound state. Parking preserves both projections.
            let _ = fanout.outstanding_sources()?;
            let _ = fanout.outstanding_reservation_counts()?;
        }
        let fanout = self
            .fanouts
            .get_mut(fanout_index)
            .expect("retired exact fanout must remain present");
        let (_, prune_receipt) = fanout
            .reply_routes
            .as_mut()
            .expect("preflighted reply fanout must retain its route history")
            .retain_active_with_receipt();
        if let Some(ownership) = fanout.ingress_ownership.as_mut() {
            let Some(projected_routes) = ownership.project_retained_reply_routes(prune_receipt)
            else {
                return Err(
                    "Sumeragi v2 retired reply target lost fair-ingress ownership".to_owned(),
                );
            };
            fanout.reply_routes = Some(projected_routes);
        }
        let target = fanout
            .targets
            .get_mut(target_index)
            .expect("retired exact target must remain present");
        target.current = None;
        target.ticket = None;
        target.parked = true;
        // Only the scheduling cursor advances. The message cursor, local/global
        // source FIFO ownership, and reservation ownership stay unchanged so a
        // reconnect retries this exact current item.
        fanout.advance_target_cursor(target_index);
        self.next_fanout_index = (fanout_index + 1) % self.fanouts.len();
        Ok(())
    }
    /// Drive exact output fairly until drained, blocked, or the deterministic budget is spent.
    fn drive_with_budget_ack_and_durable_history<Attempt>(
        &mut self,
        attempt_budget: usize,
        durable_history: Option<&Kura>,
        released_kura_replica_advert_heights: &mut BTreeSet<u64>,
        mut attempt: Attempt,
    ) -> Result<ExactOutputDriveOutcome, String>
    where
        Attempt: FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
            &ExactTargetRoute,
            u8,
        ) -> Result<
            ExactOutputAttemptOutcome,
            NetworkActorAdmissionError<Post<NetworkMessage>>,
        >,
    {
        if attempt_budget == 0 {
            return Err("Sumeragi v2 exact-output drive budget must be non-zero".to_owned());
        }
        let mut blocked_sources = BTreeSet::new();
        let mut closest_backpressure_rank: Option<usize> = None;
        let mut attempts = 0usize;
        while !self.fanouts.is_empty() {
            if attempts == attempt_budget {
                return Ok(ExactOutputDriveOutcome::BudgetExhausted {
                    closest_backpressure_rank,
                });
            }
            if let Some((fanout_index, target_index)) = self.next_inactive_reply_target() {
                attempts = attempts
                    .checked_add(1)
                    .expect("bounded exact-output retirement count cannot overflow");
                self.retire_inactive_reply_target(fanout_index, target_index)?;
                continue;
            }
            let Some((fanout_index, target_index)) =
                self.next_schedulable_target(&blocked_sources)?
            else {
                if !self
                    .fanouts
                    .iter()
                    .any(PendingExactFanout::has_dispatchable_target)
                {
                    return Ok(ExactOutputDriveOutcome::Drained);
                }
                if let Some(closest_rank) = closest_backpressure_rank {
                    return Ok(ExactOutputDriveOutcome::Backpressured { closest_rank });
                }
                if self.has_quiescent_fifo_head()? {
                    return Ok(ExactOutputDriveOutcome::Drained);
                }
                return Err(
                    "Sumeragi v2 exact-output scheduler found no per-target FIFO head".to_owned(),
                );
            };
            let inactive_reply = self
                .fanouts
                .get(fanout_index)
                .and_then(|fanout| fanout.targets.get(target_index))
                .is_some_and(|target| {
                    matches!(&target.route, ExactTargetRoute::Reply(route) if !route.is_active())
                });
            if inactive_reply {
                attempts = attempts
                    .checked_add(1)
                    .expect("bounded exact-output retirement count cannot overflow");
                self.retire_inactive_reply_target(fanout_index, target_index)?;
                continue;
            }
            let message_cursor_before = self
                .fanouts
                .get(fanout_index)
                .and_then(|fanout| fanout.targets.get(target_index))
                .ok_or_else(|| "Sumeragi v2 selected sidecar output target disappeared".to_owned())?
                .message_index;
            let message_cursor_after = message_cursor_before
                .checked_add(1)
                .ok_or_else(|| "Sumeragi v2 exact-output message cursor overflowed".to_owned())?;
            let (post, ticket, route, reply_writer_timeout_attempt) = self
                .fanouts
                .get_mut(fanout_index)
                .expect("selected exact fanout must remain present")
                .take_attempt(target_index)
                .expect("selected exact-output target must own an attempt");
            if matches!(&route, ExactTargetRoute::Reply(reply_route) if !reply_route.is_active()) {
                drop(post);
                drop(ticket);
                attempts = attempts
                    .checked_add(1)
                    .expect("bounded exact-output retirement count cannot overflow");
                self.retire_inactive_reply_target(fanout_index, target_index)?;
                continue;
            }
            let attempted_peer = post.peer_id.clone();
            let attempted_source = route.source(&attempted_peer, exact_output_class(&post.data)?);
            let reply_attempt = match &route {
                ExactTargetRoute::Reply(reply_route) => Some((post.clone(), reply_route.clone())),
                ExactTargetRoute::Topology => None,
            };
            let sidecar_reply = match (&post.data, &route) {
                (
                    NetworkMessage::CertifiedMergeSidecar(message),
                    ExactTargetRoute::Reply(reply_route),
                ) => match message.as_ref() {
                    CertifiedMergeSidecarMessage::Chunk(_) => Some((
                        post.clone(),
                        reply_route.clone(),
                        message_cursor_before,
                        message_cursor_after,
                    )),
                    CertifiedMergeSidecarMessage::Request(_)
                    | CertifiedMergeSidecarMessage::Close(_)
                    | CertifiedMergeSidecarMessage::CloseAck(_)
                    | CertifiedMergeSidecarMessage::GenerationHint(_) => None,
                },
                _ => None,
            };
            if sidecar_reply.is_some()
                && self.sidecar_control_units() >= self.sidecar_admission_capacity
            {
                self.fanouts
                    .get_mut(fanout_index)
                    .expect("receipt-backpressured exact fanout must remain present")
                    .retain_returned(target_index, post, ticket)?;
                return Ok(ExactOutputDriveOutcome::ReceiptBackpressured);
            }
            attempts = attempts
                .checked_add(1)
                .expect("bounded exact-output attempt count cannot overflow");
            match attempt(post, ticket, &route, reply_writer_timeout_attempt) {
                Ok(ExactOutputAttemptOutcome::Admitted) => {
                    if reply_attempt.is_some() {
                        return Err(
                            "Sumeragi v2 admitted a reply without its exact writer-flush witness"
                                .to_owned(),
                        );
                    }
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("admitted exact fanout must remain present")
                        .mark_admitted(target_index)?;
                    self.advance_after_attempt(
                        fanout_index,
                        target_index,
                        Some(&attempted_source),
                    )?;
                }
                Ok(ExactOutputAttemptOutcome::ReplyFlush(flush_ack)) => {
                    if sidecar_reply.is_some() {
                        return Err(
                            "Sumeragi v2 attached an ordinary flush witness to sidecar output"
                                .to_owned(),
                        );
                    }
                    let (canonical_post, reply_route) = reply_attempt.ok_or_else(|| {
                        "Sumeragi v2 attached a reply flush witness to topology output".to_owned()
                    })?;
                    if !flush_ack
                        .identity()
                        .is_bound_to_canonical_reply(&canonical_post)
                        || !flush_ack.identity().is_bound_to_delivery(&reply_route)
                        || flush_ack.identity().reply_writer_timeout_attempt()
                            != reply_writer_timeout_attempt
                    {
                        return Err(
                            "Sumeragi v2 ordinary reply flush changed route, payload, or timeout-attempt identity"
                                .to_owned(),
                        );
                    }
                    let fanout = self
                        .fanouts
                        .get_mut(fanout_index)
                        .expect("flushing exact fanout must remain present");
                    let target = fanout
                        .targets
                        .get_mut(target_index)
                        .expect("flushing exact target must remain present");
                    if target
                        .pending_flush
                        .replace(PendingExactReplyFlush {
                            flush_ack,
                            reply_writer_timeout_attempt,
                            sidecar_admission: None,
                        })
                        .is_some()
                    {
                        return Err(
                            "Sumeragi v2 reply target acquired two writer flushes".to_owned()
                        );
                    }
                    fanout.advance_target_cursor(target_index);
                    self.next_fanout_index = (fanout_index + 1) % self.fanouts.len();
                }
                Ok(ExactOutputAttemptOutcome::SidecarFlush(flush_ack)) => {
                    let (canonical_post, reply_route, message_cursor_before, message_cursor_after) =
                        sidecar_reply.ok_or_else(|| {
                            "Sumeragi v2 attached a sidecar flush witness to non-sidecar output"
                                .to_owned()
                        })?;
                    if flush_ack.identity().reply_writer_timeout_attempt()
                        != reply_writer_timeout_attempt
                    {
                        return Err(
                            "Sumeragi v2 sidecar reply flush changed timeout-attempt identity"
                                .to_owned(),
                        );
                    }
                    let admission = CertifiedMergeSidecarChunkAdmission::from_admitted_reply(
                        &canonical_post,
                        &reply_route,
                        message_cursor_before,
                        message_cursor_after,
                        flush_ack.identity(),
                    )
                    .map_err(|error| error.to_string())?;
                    let fanout = self
                        .fanouts
                        .get_mut(fanout_index)
                        .expect("flushing exact fanout must remain present");
                    let target = fanout
                        .targets
                        .get_mut(target_index)
                        .expect("flushing exact target must remain present");
                    if target
                        .pending_flush
                        .replace(PendingExactReplyFlush {
                            flush_ack,
                            reply_writer_timeout_attempt,
                            sidecar_admission: Some(admission),
                        })
                        .is_some()
                    {
                        return Err(
                            "Sumeragi v2 sidecar target acquired two writer flushes".to_owned()
                        );
                    }
                    if target.message_index != message_cursor_before {
                        return Err(
                            "Sumeragi v2 sidecar cursor advanced before writer flush".to_owned()
                        );
                    }
                    fanout.advance_target_cursor(target_index);
                    self.next_fanout_index = (fanout_index + 1) % self.fanouts.len();
                }
                #[cfg(test)]
                Ok(ExactOutputAttemptOutcome::TestReplyFlushed) => {
                    if reply_attempt.is_none() {
                        return Err(
                            "Sumeragi v2 test attached a synthetic reply flush to topology output"
                                .to_owned(),
                        );
                    }
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("synthetically flushed exact fanout must remain present")
                        .mark_admitted(target_index)?;
                    self.advance_after_attempt(
                        fanout_index,
                        target_index,
                        Some(&attempted_source),
                    )?;
                }
                Ok(ExactOutputAttemptOutcome::Unavailable) => {
                    if !matches!(&route, ExactTargetRoute::Reply(reply_route)
                        if !reply_route.is_reply_writable())
                    {
                        return Err(
                            "Sumeragi v2 network actor reported an unavailable writable route"
                                .to_owned(),
                        );
                    }
                    self.park_unwritable_reply_target(fanout_index, target_index)?;
                }
                Ok(ExactOutputAttemptOutcome::Retired) => {
                    if !matches!(&route, ExactTargetRoute::Reply(reply_route) if !reply_route.is_active())
                    {
                        return Err(
                            "Sumeragi v2 network actor retired a live exact output route"
                                .to_owned(),
                        );
                    }
                    self.retire_inactive_reply_target(fanout_index, target_index)?;
                }
                Err(NetworkActorAdmissionError::Backpressured {
                    message,
                    ticket,
                    rank,
                }) => {
                    if message.peer_id != attempted_peer {
                        self.fanouts
                            .get_mut(fanout_index)
                            .expect("backpressured exact fanout must remain present")
                            .retain_returned(target_index, message, ticket)?;
                        return Err(
                            "Sumeragi v2 network actor changed an exact output target".to_owned()
                        );
                    }
                    let ticketless_topology_target =
                        ticket.is_none() && matches!(&route, ExactTargetRoute::Topology);
                    let release_to_reconstruction_source = ticketless_topology_target
                        && self
                            .fanouts
                            .get(fanout_index)
                            .is_some_and(PendingExactFanout::is_reconstructible_topology_fanout);
                    let release_to_applied_height_finality = ticketless_topology_target
                        && self
                            .applied_height_finality
                            .as_ref()
                            .is_some_and(|artifact| {
                                self.fanouts.get(fanout_index).is_some_and(|fanout| {
                                    let claim_durable_history = if matches!(
                                        &fanout.rollover_claim,
                                        ExactOutputRolloverClaim::DurableKuraReplicaAdvert { .. }
                                            | ExactOutputRolloverClaim::QueuePlanAdmission { .. }
                                    ) {
                                        durable_history
                                    } else {
                                        None
                                    };
                                    applied_height_reconstruction_covers(
                                        &fanout.messages,
                                        &fanout.semantic_peers(),
                                        &fanout.rollover_claim,
                                        artifact,
                                        None,
                                        claim_durable_history,
                                    )
                                    .is_ok()
                                })
                            });
                    if release_to_reconstruction_source || release_to_applied_height_finality {
                        // No actor ticket means this target owns no FIFO rank:
                        // its live-topology membership may have disappeared, or
                        // the bounded waiter table may be full. The fetch,
                        // discovery, autonomous/historical lane, or certified-
                        // sidecar owner can reconstruct the occurrence;
                        // historical responses are rebuilt when the requester
                        // retries and the durable sidecar stream retries
                        // cumulative Close.
                        // Retaining this ticketless worker copy could consume
                        // the only shared non-roster slot forever. Exact durable
                        // finality also supersedes a same-height occurrence once
                        // State commits that height, whether actor rank was lost
                        // to a topology change or waiter exhaustion. Reusing the
                        // applied-height handoff verifier admits only claims it can
                        // authenticate from finality alone or from the exact
                        // read-only Kura source supplied by production services;
                        // typed scope prevents unrelated ticketless topology
                        // traffic from taking that release path.
                        if let Some(ExactOutputRolloverClaim::DurableKuraReplicaAdvert {
                            source_height,
                            ..
                        }) = self
                            .fanouts
                            .get(fanout_index)
                            .map(|fanout| &fanout.rollover_claim)
                        {
                            released_kura_replica_advert_heights.insert(*source_height);
                        }
                        drop(message);
                        self.fanouts
                            .get_mut(fanout_index)
                            .expect("released reconstructible fanout must remain present")
                            .mark_admitted(target_index)?;
                        self.advance_after_attempt(
                            fanout_index,
                            target_index,
                            Some(&attempted_source),
                        )?;
                        continue;
                    }
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("backpressured exact fanout must remain present")
                        .retain_returned(target_index, message, ticket)?;
                    blocked_sources.insert(attempted_source);
                    closest_backpressure_rank =
                        Some(closest_backpressure_rank.map_or(rank, |current| current.min(rank)));
                    self.advance_after_attempt(fanout_index, target_index, None)?;
                }
                Err(NetworkActorAdmissionError::Closed { message }) => {
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("closed exact fanout must remain present")
                        .retain_returned(target_index, message, None)?;
                    return Err(
                        "Sumeragi v2 network actor closed during output admission".to_owned()
                    );
                }
                Err(NetworkActorAdmissionError::Rejected {
                    message,
                    reason: NetworkActorAdmissionRejection::InactiveReplyRoute,
                }) => {
                    drop(message);
                    self.retire_inactive_reply_target(fanout_index, target_index)?;
                }
                Err(NetworkActorAdmissionError::Rejected { message, reason }) => {
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("rejected exact fanout must remain present")
                        .retain_returned(target_index, message, None)?;
                    return Err(format!(
                        "Sumeragi v2 network actor permanently rejected output: {reason:?}"
                    ));
                }
            }
        }
        Ok(ExactOutputDriveOutcome::Drained)
    }
    #[cfg(test)]
    fn drive_with_budget_ack<Attempt>(
        &mut self,
        attempt_budget: usize,
        attempt: Attempt,
    ) -> Result<ExactOutputDriveOutcome, String>
    where
        Attempt: FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
            &ExactTargetRoute,
            u8,
        ) -> Result<
            ExactOutputAttemptOutcome,
            NetworkActorAdmissionError<Post<NetworkMessage>>,
        >,
    {
        let mut released_kura_replica_advert_heights = BTreeSet::new();
        let outcome = self.drive_with_budget_ack_and_durable_history(
            attempt_budget,
            None,
            &mut released_kura_replica_advert_heights,
            attempt,
        )?;
        debug_assert!(released_kura_replica_advert_heights.is_empty());
        Ok(outcome)
    }
    #[cfg(test)]
    fn drive_with_budget<Attempt>(
        &mut self,
        attempt_budget: usize,
        mut attempt: Attempt,
    ) -> Result<ExactOutputDriveOutcome, String>
    where
        Attempt: FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
            &ExactTargetRoute,
        ) -> Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>>,
    {
        self.drive_with_budget_ack(attempt_budget, |post, ticket, route, _timeout_attempt| {
            attempt(post, ticket, route).map(|()| match route {
                ExactTargetRoute::Topology => ExactOutputAttemptOutcome::Admitted,
                ExactTargetRoute::Reply(_) => ExactOutputAttemptOutcome::TestReplyFlushed,
            })
        })
    }
    fn drive_bounded_with_ack<Attempt>(
        &mut self,
        durable_history: &Kura,
        released_kura_replica_advert_heights: &mut BTreeSet<u64>,
        attempt: Attempt,
    ) -> Result<ExactOutputDriveOutcome, String>
    where
        Attempt: FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
            &ExactTargetRoute,
            u8,
        ) -> Result<
            ExactOutputAttemptOutcome,
            NetworkActorAdmissionError<Post<NetworkMessage>>,
        >,
    {
        self.drive_with_budget_ack_and_durable_history(
            self.drive_attempt_budget,
            Some(durable_history),
            released_kura_replica_advert_heights,
            attempt,
        )
    }
    #[cfg(test)]
    fn drive_with<Attempt>(&mut self, attempt: Attempt) -> Result<Option<usize>, String>
    where
        Attempt: FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
            &ExactTargetRoute,
        ) -> Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>>,
    {
        match self.drive_with_budget(usize::MAX, attempt)? {
            ExactOutputDriveOutcome::Drained => Ok(None),
            ExactOutputDriveOutcome::ReceiptBackpressured => Err(
                "unbounded exact-output test drive requires sidecar receipt drainage".to_owned(),
            ),
            ExactOutputDriveOutcome::Backpressured { closest_rank } => Ok(Some(closest_rank)),
            ExactOutputDriveOutcome::BudgetExhausted { .. } => Err(
                "unbounded exact-output test drive unexpectedly exhausted its budget".to_owned(),
            ),
        }
    }
}
fn durable_history_source_covers(
    messages: &[NetworkMessage],
    rollover_claim: &ExactOutputRolloverClaim,
    source_network_id: &iroha_data_model::NetworkId,
    maximum_source_height: wire::Height,
    kura: &Kura,
) -> Result<(), String> {
    let [message] = messages else {
        return Err("Sumeragi v2 durable response claim is not a singleton".to_owned());
    };
    if message.progress_reconstruction() != ProgressReconstruction::Retransmit {
        return Err("Sumeragi v2 durable response is not reconstructible traffic".to_owned());
    }
    let NetworkMessage::SumeragiBlock(envelope) = message else {
        return Err("Sumeragi v2 durable response is not block traffic".to_owned());
    };
    match (rollover_claim, envelope.as_message()) {
        (
            ExactOutputRolloverClaim::DurableCommitCertificateResponse {
                responder: claimed_responder,
                source_height,
                source_context_id,
                ..
            },
            BlockMessage::V2(message),
        ) => {
            let wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) =
                &message.payload
            else {
                return Err("durable CommitQC response changed payload kind".to_owned());
            };
            if *source_height > maximum_source_height {
                return Err("durable CommitQC response belongs to a future height".to_owned());
            }
            let source = kura
                .v2_finality_artifact(*source_height)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    "durable CommitQC response lost its Kura finality source".to_owned()
                })?;
            if &source.height_context.network_id != source_network_id
                || source.context_id() != *source_context_id
                || response.certificate != source.commit_qc
                || &response.responder != claimed_responder
            {
                return Err(
                    "durable CommitQC response differs from its Kura finality source".to_owned(),
                );
            }
            response
                .validate(&source.height_context)
                .map_err(|error| error.to_string())?;
            Signature::try_from_bytes(&response.signature)
                .map_err(|error| error.to_string())?
                .verify(
                    response.responder.public_key(),
                    &response.signature_preimage(),
                )
                .map_err(|error| error.to_string())
        }
        (
            ExactOutputRolloverClaim::DurableCertifiedBodyResponse {
                responder: claimed_responder,
                source_round,
                source_subject,
                ..
            },
            BlockMessage::V2(message),
        ) => {
            let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload
            else {
                return Err("durable body response changed payload kind".to_owned());
            };
            if source_round.height > maximum_source_height {
                return Err("durable body response belongs to a future height".to_owned());
            }
            let source = kura
                .v2_finality_artifact(source_round.height)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "durable body response lost its Kura finality source".to_owned())?;
            if &source.height_context.network_id != source_network_id
                || source.context_id() != source_round.context_id
                || source.subject != *source_subject
            {
                return Err(
                    "durable body response differs from its Kura finality source".to_owned(),
                );
            }
            response
                .validate(&source.height_context)
                .map_err(|error| error.to_string())?;
            if &response.responder != claimed_responder {
                return Err(
                    "durable body response is not bound to the serving network identity".to_owned(),
                );
            }
            Signature::try_from_bytes(&response.signature)
                .map_err(|error| error.to_string())?
                .verify(
                    response.responder.public_key(),
                    &response.signature_preimage(),
                )
                .map_err(|error| error.to_string())?;
            let block_height = usize::try_from(source_round.height)
                .ok()
                .and_then(NonZeroUsize::new)
                .ok_or_else(|| "durable body source height is not representable".to_owned())?;
            let block = kura
                .get_block(block_height)
                .ok_or_else(|| "durable body response lost its canonical Kura block".to_owned())?;
            let proposal = block.canonical_resultless_proposal();
            let canonical_wire = proposal.encode_wire().map_err(|error| error.to_string())?;
            if block.hash() != source_subject.block_hash
                || canonical_wire != response.body
                || Hash::new(&canonical_wire) != source_subject.payload_hash
            {
                return Err("durable body response differs from its canonical Kura body".to_owned());
            }
            let (manifest, _) = encode_payload(
                &source.height_context,
                *source_round,
                *source_subject,
                &canonical_wire,
            )
            .map_err(|error| error.to_string())?
            .into_parts();
            if manifest != response.manifest {
                return Err("durable body response manifest is not Kura-reconstructible".to_owned());
            }
            Ok(())
        }
        (
            ExactOutputRolloverClaim::DurableLaneCertificateResponse {
                lane_id,
                lane_block_height,
                proposal_height,
                proposal_hash,
                ..
            },
            BlockMessage::LaneBlockCertificate(certificate),
        ) => {
            if *proposal_height > maximum_source_height {
                return Err("durable lane certificate belongs to a future height".to_owned());
            }
            let source = kura
                .read_certified_lane_block_artifact(*lane_id, *lane_block_height)
                .ok_or_else(|| {
                    "durable lane certificate lost its certified Kura source".to_owned()
                })?;
            if source.proposal.descriptor.proposal_height != *proposal_height
                || source.proposal.proposal_hash != *proposal_hash
                || certificate.proposal != source.proposal
                || certificate.prepare_qc != source.prepare_qc
                || certificate.commit_qc != source.commit_qc
            {
                return Err(
                    "durable lane certificate differs from its certified Kura source".to_owned(),
                );
            }
            Ok(())
        }
        (
            ExactOutputRolloverClaim::HistoricalLaneCertification {
                source_height,
                lane_id,
                lane_block_height,
                proposal_hash,
                message_hash,
                ..
            },
            message,
        ) => {
            if *source_height >= maximum_source_height || HashOf::new(message) != *message_hash {
                return Err(
                    "historical lane certification has an invalid source height or hash".to_owned(),
                );
            }
            let records = kura
                .historical_autonomous_lane_recovery_records_bounded(
                    crate::kura::HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
                )
                .map_err(|error| error.to_string())?;
            let record = records.into_iter().find(|record| {
                let proposal = &record.payload.origin_proposal;
                proposal.descriptor.proposal_height == *source_height
                    && proposal.descriptor.lane_id == *lane_id
                    && proposal.descriptor.lane_block_height == *lane_block_height
                    && proposal.proposal_hash == *proposal_hash
            });
            let Some(record) = record else {
                return durable_historical_lane_output_source_hash(kura, message).and_then(
                    |source| {
                        source.map(|_| ()).ok_or_else(|| {
                            "historical lane certification lost its exact Kura source".to_owned()
                        })
                    },
                );
            };
            kura.validate_historical_autonomous_lane_recovery_record_dependencies(&record)
                .map_err(|error| error.to_string())?;
            let proposal = &record.payload.origin_proposal;
            let validator_pops = proposal
                .descriptor
                .validator_set
                .iter()
                .zip(&record.validator_pops)
                .map(|(validator, pop)| (validator.public_key().clone(), pop.clone()))
                .collect();
            validate_winning_lane_output(message, proposal, &validator_pops)
        }
        (
            ExactOutputRolloverClaim::HistoricalLaneRecoveryResponse {
                request_hash,
                response_hash,
                ..
            },
            BlockMessage::LaneHistoricalRecoveryResponse(response),
        ) => {
            if response.request_hash != *request_hash
                || HashOf::new(response.as_ref()) != *response_hash
                || response.version != super::message::LANE_HISTORICAL_RECOVERY_VERSION_V1
            {
                return Err(
                    "historical lane recovery response changed its exact request binding"
                        .to_owned(),
                );
            }
            match &response.payload {
                LaneHistoricalRecoveryPayloadV1::CanonicalBlock {
                    block,
                    finality_artifact,
                } => {
                    let height = block.header().height().get();
                    if height > maximum_source_height {
                        return Err(
                            "historical canonical-body response belongs to a future height"
                                .to_owned(),
                        );
                    }
                    let source = kura
                        .v2_finality_artifact(height)
                        .map_err(|error| error.to_string())?
                        .ok_or_else(|| {
                            "historical canonical-body response lost its finality source".to_owned()
                        })?;
                    if &source.height_context.network_id != source_network_id
                        || source != *finality_artifact
                        || source.validate_for_header(&block.header()).is_err()
                        || source.verify().is_err()
                    {
                        return Err(
                            "historical canonical-body response differs from Kura finality"
                                .to_owned(),
                        );
                    }
                    let height = usize::try_from(height)
                        .ok()
                        .and_then(NonZeroUsize::new)
                        .ok_or_else(|| {
                            "historical canonical-body height is not representable".to_owned()
                        })?;
                    if kura.get_block(height).as_deref() != Some(block) {
                        return Err(
                            "historical canonical-body response differs from Kura body".to_owned()
                        );
                    }
                    Ok(())
                }
                LaneHistoricalRecoveryPayloadV1::AutonomousPayload {
                    payload,
                    prepare_qc,
                    commit_qc,
                } => {
                    let descriptor = &payload.origin_proposal.descriptor;
                    if descriptor.proposal_height > maximum_source_height {
                        return Err(
                            "historical autonomous response belongs to a future height".to_owned()
                        );
                    }
                    let certified = kura
                        .read_certified_lane_block_artifact(
                            descriptor.lane_id,
                            descriptor.lane_block_height,
                        )
                        .ok_or_else(|| {
                            "historical autonomous response lost its certified Kura source"
                                .to_owned()
                        })?;
                    if certified.proposal != payload.origin_proposal
                        || certified.prepare_qc != *prepare_qc
                        || certified.commit_qc != *commit_qc
                    {
                        return Err(
                            "historical autonomous response differs from certified Kura evidence"
                                .to_owned(),
                        );
                    }
                    let expected_epoch = payload.epoch;
                    let (durable_payload, _) = kura
                        .current_autonomous_lane_payload(
                            descriptor.lane_id,
                            descriptor.lane_block_height,
                            payload.network_id,
                            expected_epoch,
                        )
                        .ok_or_else(|| {
                            "historical autonomous response lost its payload sidecar".to_owned()
                        })?;
                    let durable_availability = kura
                        .read_autonomous_lane_block_artifact(
                            descriptor.lane_id,
                            descriptor.lane_block_height,
                            payload.network_id,
                            expected_epoch,
                        )
                        .and_then(|artifact| artifact.availability_certificate);
                    if durable_payload != *payload
                        || durable_availability
                            .is_none_or(|certificate| certificate.certificate != *prepare_qc)
                    {
                        return Err(
                            "historical autonomous response differs from its READY sidecar"
                                .to_owned(),
                        );
                    }
                    Ok(())
                }
                LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk {
                    finality_artifact,
                    wire_len,
                    chunk_index,
                    chunk_count,
                    bytes,
                } => {
                    let height = finality_artifact.height;
                    if height == 0 || height > maximum_source_height {
                        return Err(
                            "historical canonical executed-block chunk belongs to an invalid or future height"
                                .to_owned(),
                        );
                    }
                    let source = kura
                        .v2_finality_artifact(height)
                        .map_err(|error| error.to_string())?
                        .ok_or_else(|| {
                            "historical canonical executed-block chunk lost its finality source"
                                .to_owned()
                        })?;
                    let height_index = usize::try_from(height)
                        .ok()
                        .and_then(NonZeroUsize::new)
                        .ok_or_else(|| {
                            "historical canonical executed-block height is not representable"
                                .to_owned()
                        })?;
                    let block = kura
                        .get_block_without_merge_sidecar(height_index)
                        .ok_or_else(|| {
                            "historical canonical executed-block chunk lost its Kura body"
                                .to_owned()
                        })?;
                    if &source.height_context.network_id != source_network_id
                        || source != *finality_artifact
                        || source.verify().is_err()
                        || source.validate_for_header(&block.header()).is_err()
                        || block.header().height().get() != height
                        || block.hash() != source.block_hash
                        || source.commit_qc.execution_commitment.validate().is_err()
                        || !block.executed_block_wire_hash().is_ok_and(|hash| {
                            hash == source
                                .commit_qc
                                .execution_commitment
                                .executed_block_wire_hash
                        })
                    {
                        return Err(
                            "historical canonical executed-block chunk differs from Kura finality"
                                .to_owned(),
                        );
                    }
                    let canonical_wire = block.encode_wire().map_err(|error| error.to_string())?;
                    let expected_wire_len =
                        u64::try_from(canonical_wire.len()).map_err(|error| error.to_string())?;
                    let expected_chunk_count = canonical_wire
                        .len()
                        .div_ceil(crate::merge_sidecar::MAX_CERTIFIED_MERGE_CHUNK_BYTES);
                    let expected_chunk_count_u32 =
                        u32::try_from(expected_chunk_count).map_err(|error| error.to_string())?;
                    let chunk_index_usize =
                        usize::try_from(*chunk_index).map_err(|error| error.to_string())?;
                    let start = chunk_index_usize
                        .checked_mul(crate::merge_sidecar::MAX_CERTIFIED_MERGE_CHUNK_BYTES)
                        .ok_or_else(|| {
                            "historical canonical executed-block chunk offset overflow".to_owned()
                        })?;
                    let end = start
                        .saturating_add(crate::merge_sidecar::MAX_CERTIFIED_MERGE_CHUNK_BYTES)
                        .min(canonical_wire.len());
                    if canonical_wire.is_empty()
                        || expected_wire_len > crate::kura::STRICT_INIT_MAX_BLOCK_BYTES
                        || *wire_len != expected_wire_len
                        || expected_chunk_count == 0
                        || *chunk_count != expected_chunk_count_u32
                        || chunk_index_usize >= expected_chunk_count
                        || bytes.as_slice() != &canonical_wire[start..end]
                    {
                        return Err(
                            "historical canonical executed-block chunk differs from its exact Kura wire"
                                .to_owned(),
                        );
                    }
                    Ok(())
                }
            }
        }
        _ => Err("Sumeragi v2 durable response claim changed output kind".to_owned()),
    }
}
include!("v2_worker/autonomous_lane_output_reconstruction.rs");
include!("v2_worker/kura_replica_advert_refresh.rs");
