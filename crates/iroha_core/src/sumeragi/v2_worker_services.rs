/// Concrete effect services used by the live v2 height runner.
pub(crate) struct ProductionV2Services {
    context: wire::HeightContext,
    validator_set_pops: Vec<Vec<u8>>,
    state: Arc<crate::state::State>,
    local_peer: PeerId,
    local_validator: Option<wire::ValidatorIndex>,
    key_pair: KeyPair,
    network: IrohaNetwork,
    /// Rotating start for bounded live-topology archive discovery.
    archive_peer_cursor: AtomicUsize,
    kura: Arc<Kura>,
    io: Option<V2IoHandle>,
    lifecycle_body_store_identity: Option<V2BodyStoreInstanceIdentity>,
    lifecycle_payload_store_identity: Option<CertifiedServePayloadStoreInstanceIdentity>,
    fetches: BTreeMap<EffectWorkId, FetchSession>,
    fetch_by_manifest: BTreeMap<HashOf<wire::PayloadManifest>, EffectWorkId>,
    orphan_chunks: BTreeMap<HashOf<wire::PayloadManifest>, VecDeque<BufferedPayloadChunk>>,
    orphan_chunk_count: usize,
    orphan_chunk_bytes: u64,
    orphan_lifecycle_sweep_cursor: Option<OrphanPayloadLifecycleSweepCursor>,
    max_orphan_chunks: usize,
    max_orphan_chunk_bytes: u64,
    max_merge_sidecar_deferrals: usize,
    local_completions: VecDeque<LocalCompletion>,
    held_io_completion: Option<V2IoCompletion>,
    next_completion_source: CompletionSource,
    locked_candidate_acquisition: Option<LockedCandidateAcquisition>,
    next_locked_candidate_acquisition_id: u64,
    proposal_work_retired: bool,
    prepared_candidates: VecDeque<PreparedCandidateBody>,
    merge_sidecar_deferrals: VecDeque<DeferredMergeSidecarWork>,
    outbound_chunks: BTreeMap<HashOf<wire::PayloadManifest>, RetainedOutboundPayload>,
    fast_path_proposals: BTreeSet<wire::ConsensusRound>,
    pending_exact_output: Mutex<PendingExactOutput>,
    /// Process-lifetime proactive refresh owner shared across immutable height
    /// services. Its retained Kura token is not pending exact output and never
    /// participates in finality sealing.
    kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
    exact_output_handoff_owner: DurableExactOutputServiceOwner,
    #[cfg(test)]
    exact_output_admission_hook: Option<Mutex<ExactOutputAdmissionHook>>,
    #[cfg(test)]
    consensus_broadcasts: Vec<wire::ConsensusMessageV2>,
    active_tag: EventTag,
    last_status: Option<EffectExecutorStatus>,
    fatal_reason: Option<String>,
    output_guard: Arc<ConsensusOutputGuard>,
    leader_wire_ingress: Arc<FairV2Ingress>,
    leader_wire_recovery_authority: super::serviced_candidate_store::LeaderWireRecoveryAuthority,
    clean_teardown: bool,
}

/// Result of linearizing Runtime behind the physical Completion prefix.
#[must_use = "the cut decision must be consumed before another Runtime turn"]
pub(in crate::sumeragi) enum V2CompletionRuntimeCutDecisionV1 {
    /// A physical completion owns the next outer turn.
    RetryCompletion,
    /// Runtime owns the cut because the physical completion lane was empty.
    Runtime(V2CompletionRuntimeCutV1),
    /// A full runtime FIFO must retire one exact Completion-class owner before
    /// the blocked physical completion can cross into the reducer.
    CapacityRelief(V2CompletionCapacityReliefCutV1),
}

struct V2CompletionRuntimeCutBindingV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    context_id: wire::HeightContextId,
    height: u64,
    cut_at: Instant,
    _linearity: V2CompletionRuntimeCutLinearityV1,
}

struct V2CompletionRuntimeCutLinearityV1;

impl V2CompletionRuntimeCutBindingV1 {
    fn new(
        output_guard: Arc<ConsensusOutputGuard>,
        context_id: wire::HeightContextId,
        height: u64,
        cut_at: Instant,
    ) -> Self {
        Self {
            output_guard,
            context_id,
            height,
            cut_at,
            _linearity: V2CompletionRuntimeCutLinearityV1,
        }
    }

    fn consume_for_executor(
        self,
        output_guard: &Arc<ConsensusOutputGuard>,
        context: &wire::HeightContext,
    ) -> Option<Instant> {
        (Arc::ptr_eq(&self.output_guard, output_guard)
            && self.context_id == context.id()
            && self.height == context.height)
            .then_some(self.cut_at)
    }
}

/// Move-only proof that an empty physical Completion lane precedes Runtime.
///
/// The worker records completion ownership and mints this cut under the same
/// mutex. A worker result retained after this empty observation therefore
/// cannot claim a timestamp before `cut_at`.
#[must_use = "the completion cut must be consumed by the matching executor step"]
pub(in crate::sumeragi) struct V2CompletionRuntimeCutV1 {
    binding: V2CompletionRuntimeCutBindingV1,
}

impl V2CompletionRuntimeCutV1 {
    fn new(
        output_guard: Arc<ConsensusOutputGuard>,
        context_id: wire::HeightContextId,
        height: u64,
        cut_at: Instant,
    ) -> Self {
        Self {
            binding: V2CompletionRuntimeCutBindingV1::new(
                output_guard,
                context_id,
                height,
                cut_at,
            ),
        }
    }

    /// Consume the cut only against the exact height executor and fail-stop owner.
    pub(in crate::sumeragi) fn consume_for_executor(
        self,
        output_guard: &Arc<ConsensusOutputGuard>,
        context: &wire::HeightContext,
    ) -> Option<Instant> {
        self.binding.consume_for_executor(output_guard, context)
    }
}

/// Move-only proof that a physical completion is blocked by a full runtime FIFO.
///
/// This token authorizes only one Completion-class capacity-relief turn. It
/// carries the blocked worker/local lifecycle ordinal so the runtime cannot
/// retire a later reducer owner to make room for an earlier physical result.
#[must_use = "the relief cut must be consumed by the matching Completion-only step"]
pub(in crate::sumeragi) struct V2CompletionCapacityReliefCutV1 {
    binding: V2CompletionRuntimeCutBindingV1,
    blocked_completion_lifecycle_ordinal: u128,
}

impl V2CompletionCapacityReliefCutV1 {
    fn new(
        output_guard: Arc<ConsensusOutputGuard>,
        context_id: wire::HeightContextId,
        height: u64,
        cut_at: Instant,
        blocked_completion_lifecycle_ordinal: u128,
    ) -> Option<Self> {
        (blocked_completion_lifecycle_ordinal != 0).then(|| Self {
            binding: V2CompletionRuntimeCutBindingV1::new(
                output_guard,
                context_id,
                height,
                cut_at,
            ),
            blocked_completion_lifecycle_ordinal,
        })
    }

    /// Consume the cut only against the exact height executor and fail-stop owner.
    pub(in crate::sumeragi) fn consume_for_executor(
        self,
        output_guard: &Arc<ConsensusOutputGuard>,
        context: &wire::HeightContext,
    ) -> Option<(Instant, u128)> {
        let Self {
            binding,
            blocked_completion_lifecycle_ordinal,
        } = self;
        binding
            .consume_for_executor(output_guard, context)
            .map(|cut_at| (cut_at, blocked_completion_lifecycle_ordinal))
    }
}
/// Private move-only permit for unpacking one WAL/registry signed Broadcast.
pub(in crate::sumeragi) struct RecoveredLifecycleSignBroadcastOutputPermitV1 {
    _linearity: RecoveredLifecycleSignBroadcastOutputPermitLinearityV1,
}
struct RecoveredLifecycleSignBroadcastOutputPermitLinearityV1;
impl Drop for RecoveredLifecycleSignBroadcastOutputPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleSignBroadcastOutputPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleSignBroadcastOutputPermitLinearityV1,
        }
    }
}
/// Private one-shot next-Vote lookup permit enforcing worker/store identity.
pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyExecutorPermitV1 {
    _linearity: RecoveredLifecycleNextVoteBodyExecutorPermitLinearityV1,
    context: wire::HeightContext,
    requester: PeerId,
    output_guard: Arc<ConsensusOutputGuard>,
    body_store_identity: V2BodyStoreInstanceIdentity,
}
struct RecoveredLifecycleNextVoteBodyExecutorPermitLinearityV1;
impl Drop for RecoveredLifecycleNextVoteBodyExecutorPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleNextVoteBodyExecutorPermitV1 {
    fn new(
        context: wire::HeightContext,
        requester: PeerId,
        output_guard: Arc<ConsensusOutputGuard>,
        body_store_identity: V2BodyStoreInstanceIdentity,
    ) -> Self {
        Self {
            _linearity: RecoveredLifecycleNextVoteBodyExecutorPermitLinearityV1,
            context,
            requester,
            output_guard,
            body_store_identity,
        }
    }
    /// Consume only against the same executor/store owner joined by the service.
    pub(in crate::sumeragi) fn consume_for_executor(
        self,
        context: &wire::HeightContext,
        requester: &PeerId,
        output_guard: &Arc<ConsensusOutputGuard>,
        body_store_identity: &V2BodyStoreInstanceIdentity,
    ) -> Option<V2BodyStoreInstanceIdentity> {
        (self.context == *context
            && self.requester == *requester
            && Arc::ptr_eq(&self.output_guard, output_guard)
            && self.body_store_identity.same_instance(body_store_identity))
        .then_some(self.body_store_identity)
    }
}
/// Private permit consuming an adapter-sealed Proposal control/payload pair
/// behind one exact-output reservation.
pub(in crate::sumeragi) struct RecoveredLifecycleProposalExactOutputPermitV1 {
    _linearity: RecoveredLifecycleProposalExactOutputPermitLinearityV1,
}
struct RecoveredLifecycleProposalExactOutputPermitLinearityV1;
impl Drop for RecoveredLifecycleProposalExactOutputPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleProposalExactOutputPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleProposalExactOutputPermitLinearityV1,
        }
    }
}
/// Result of reserving exact output for a recovered signed Broadcast.
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(in crate::sumeragi) enum RecoveredLifecycleSignBroadcastOutputCaptureV1<'service> {
    /// The bounded corridor cannot retain this fanout yet; nothing changed.
    Unavailable,
    /// The exact corridor mutex and fail-stop operation remain retained.
    Reserved(RecoveredLifecycleSignBroadcastOutputReservationV1<'service>),
}
/// Borrow-bound exact-output reservation for one durable recovered Broadcast.
///
/// Dropping the armed reservation fail-stops. The caller must first park the
/// volatile claim while leaving LedgerV1 Ready, then commit the fanout.
#[must_use = "recovered signed Broadcast output must enter its exact corridor"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignBroadcastOutputReservationV1<'service> {
    operation: Option<ConsensusFailStopOperation<'service>>,
    pending: Option<std::sync::MutexGuard<'service, PendingExactOutput>>,
    output: Option<RecoveredLifecycleSignBroadcastPreparedOutputV1>,
}
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum RecoveredLifecycleSignBroadcastPreparedOutputV1 {
    Single(Option<PendingExactFanout>),
    Proposal(PendingExactOutputBatchPlan),
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleSignBroadcastOutputReservationV1<'_> {
    /// Publish the preflighted fanout in the assertion-only post-fsync tail.
    pub(in crate::sumeragi) fn commit_after_publication(mut self) {
        let mut pending = self
            .pending
            .take()
            .expect("recovered Sign output reservation retains its corridor mutex");
        let operation = self
            .operation
            .take()
            .expect("recovered Sign output commit retains its fail-stop operation");
        match self
            .output
            .take()
            .expect("recovered Sign output retains its exact publication")
        {
            RecoveredLifecycleSignBroadcastPreparedOutputV1::Single(fanout) => {
                if let Some(fanout) = fanout {
                    assert_eq!(
                        pending.enqueue(fanout),
                        Ok(ExactFanoutOwnership::Owned),
                        "preflighted recovered Sign fanout must enter exact-output ownership"
                    );
                }
            }
            RecoveredLifecycleSignBroadcastPreparedOutputV1::Proposal(batch) => {
                pending.commit_atomic_fanout_batch(batch);
            }
        }
        drop(pending);
        operation.complete();
    }
}
/// Result of atomically reserving Proposal control and payload fanouts.
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(in crate::sumeragi) enum RecoveredLifecycleProposalExactOutputCaptureV1<'service> {
    /// Aggregate ownership does not fit; the corridor remains unchanged and
    /// the exact authority is returned for a later bounded retry.
    Unavailable(super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1),
    /// Both fanouts remain behind one mutex and fail-stop operation.
    Reserved(RecoveredLifecycleProposalExactOutputReservationV1<'service>),
}
/// Borrow-bound atomic Proposal output reservation.
///
/// Dropping while armed fail-stops output. Every recoverable prepublication
/// path must consume [`Self::abort_before_publication`].
#[must_use = "recovered Proposal output must commit atomically or use its typed abort"]
pub(in crate::sumeragi) struct RecoveredLifecycleProposalExactOutputReservationV1<'service> {
    operation: Option<ConsensusFailStopOperation<'service>>,
    pending: Option<std::sync::MutexGuard<'service, PendingExactOutput>>,
    batch: Option<PendingExactOutputBatchPlan>,
    authority: Option<super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1>,
    wal_append: RecoveredLifecycleProposalPrepareWalAppendSealV1,
}

/// Identity seal created after a Proposal control/chunk batch owns capacity;
/// its borrow prevents WAL append from bypassing the reservation.
struct RecoveredLifecycleProposalPrepareWalAppendSealV1 {
    dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
    body_store_identity: V2BodyStoreInstanceIdentity,
    output_guard: Arc<ConsensusOutputGuard>,
    attempted: bool,
}

/// Borrow-bound proof that one exact Proposal batch remains reserved.
#[must_use = "the Proposal WAL append permit must remain tied to its output reservation"]
pub(in crate::sumeragi) struct RecoveredLifecycleProposalPrepareWalAppendPermitV1<'reservation> {
    seal: &'reservation mut RecoveredLifecycleProposalPrepareWalAppendSealV1,
}

impl RecoveredLifecycleProposalPrepareWalAppendPermitV1<'_> {
    /// Compare the preview owner without exposing any reservation constituent.
    pub(in crate::sumeragi) fn authorizes(
        &self,
        dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
        body_store_identity: &V2BodyStoreInstanceIdentity,
        output_guard: &Arc<ConsensusOutputGuard>,
    ) -> bool {
        !self.seal.attempted
            && self.seal.dispatch_key == dispatch_key
            && self
                .seal
                .body_store_identity
                .same_instance(body_store_identity)
            && Arc::ptr_eq(&self.seal.output_guard, output_guard)
    }

    /// Irreversibly cross the retry boundary before attempting the WAL append.
    pub(in crate::sumeragi) fn cross_wal_attempt_boundary(self) {
        self.seal.attempted = true;
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleProposalExactOutputReservationV1<'_> {
    /// Borrow the sole initial-Proposal WAL permit while this batch is armed.
    pub(in crate::sumeragi) fn prepare_wal_append_permit(
        &mut self,
    ) -> Option<RecoveredLifecycleProposalPrepareWalAppendPermitV1<'_>> {
        (self.operation.is_some()
            && self.pending.is_some()
            && self.batch.is_some()
            && self.authority.is_some()
            && !self.wal_append.attempted)
            .then_some(RecoveredLifecycleProposalPrepareWalAppendPermitV1 {
                seal: &mut self.wal_append,
            })
    }

    /// Release an unchanged aggregate reservation before durable publication.
    pub(in crate::sumeragi) fn abort_before_publication(
        mut self,
    ) -> super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1 {
        assert!(
            !self.wal_append.attempted,
            "an attempted Proposal WAL cut cannot return to the prepublication retry boundary"
        );
        drop(self.pending.take());
        drop(self.batch.take());
        self.operation
            .take()
            .expect("armed recovered Proposal output retains its fail-stop operation")
            .complete();
        self.authority
            .take()
            .expect("armed recovered Proposal output retains its retry authority")
    }
    /// Install both preflighted fanouts in one assertion-only publication tail.
    pub(in crate::sumeragi) fn commit_after_publication(mut self) {
        let mut pending = self
            .pending
            .take()
            .expect("recovered Proposal output retains its corridor mutex");
        // Take this after the mutex guard: reverse local-drop order closes
        // output before unlocking the corridor if any assertion below unwinds.
        let operation = self
            .operation
            .take()
            .expect("recovered Proposal output retains its fail-stop operation");
        let batch = self
            .batch
            .take()
            .expect("recovered Proposal output retains its aggregate batch");
        let authority = self
            .authority
            .take()
            .expect("recovered Proposal output commit retains its exact authority");
        pending.commit_atomic_fanout_batch(batch);
        drop(pending);
        drop(authority);
        operation.complete();
    }
}
/// Borrow-bound exact-output reservation retained before coordinator claim.
///
/// Preencoding, topology construction, rollover validation, and `can_enqueue`
/// all precede scheduler planning. Dropping an armed reservation closes output;
/// recoverable pre-claim failures must consume [`Self::abort_before_claim`].
#[must_use = "exact recovered Fetch output must commit or use its typed pre-claim abort"]
pub(in crate::sumeragi) struct RecoveredDecisionFetchExactOutputReservationV1<'service> {
    operation: Option<ConsensusFailStopOperation<'service>>,
    pending: Option<std::sync::MutexGuard<'service, PendingExactOutput>>,
    fanout: Option<PendingExactFanout>,
}
impl RecoveredDecisionFetchExactOutputReservationV1<'_> {
    /// Test-only release of an unchanged pre-claim reservation.
    #[cfg(test)]
    pub(in crate::sumeragi) fn abort_before_claim(mut self) {
        drop(self.pending.take());
        self.operation
            .take()
            .expect("armed recovered Fetch output retains its fail-stop operation")
            .complete();
    }
    /// Publish the preflighted fanout in the assertion-only post-arming tail.
    pub(in crate::sumeragi) fn commit(mut self) {
        let mut pending = self
            .pending
            .take()
            .expect("recovered Fetch output reservation retains its corridor mutex");
        // Take this after the mutex guard so unwinding closes output before
        // releasing the exact-output corridor.
        let operation = self
            .operation
            .take()
            .expect("recovered Fetch output commit retains its fail-stop operation");
        if let Some(fanout) = self.fanout.take() {
            assert_eq!(
                pending.enqueue(fanout),
                Ok(ExactFanoutOwnership::Owned),
                "preflighted recovered Fetch fanout must enter exact-output ownership"
            );
        }
        drop(pending);
        operation.complete();
    }
}
fn maximum_orphan_chunk_bytes(layout: wire::DataAvailabilityLayout) -> u64 {
    u64::from(layout.max_chunk_count)
        .saturating_mul(u64::from(layout.chunk_size_bytes))
        .min(wire::MAX_DA_ENCODED_PAYLOAD_BYTES)
}
