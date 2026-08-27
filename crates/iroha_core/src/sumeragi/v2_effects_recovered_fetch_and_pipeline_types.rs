/// Closed failure while reserving a dedicated recovered request owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum RecoveredDecisionFetchRequestRegistrationErrorV1 {
    /// The executor is closed or belongs to another height/service requester.
    ForeignExecutor,
    /// Existing ordinary or recovered request indexes are not one exact census.
    InvalidExistingCensus,
    /// Another ordinary or recovered owner has the same exact or logical request.
    ConflictingOwner,
    /// The one recovered Decision Fetch owner position is already occupied.
    Occupied,
}
/// Exclusive vacant registration retained before the coordinator claim.
///
/// Dropping the reservation performs no mutation. Its final commit consumes
/// the registry's claimed-carrier arming token and installs both dedicated
/// indexes in one assertion-only tail.
#[must_use = "dropping a recovered request reservation leaves executor indexes unchanged"]
pub(in crate::sumeragi) struct PreparedRecoveredDecisionFetchRequestRegistrationV1<
    'executor,
    R: EffectRuntime,
> {
    executor: &'executor mut V2EffectExecutor<R>,
    owner: Option<RecoveredDecisionFetchRequestOwnerV1>,
}
impl<R: EffectRuntime> PreparedRecoveredDecisionFetchRequestRegistrationV1<'_, R> {
    /// Return the reserved exact lifecycle key.
    pub(in crate::sumeragi) fn dispatch_key(
        &self,
    ) -> super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1 {
        self.owner
            .as_ref()
            .expect("prepared recovered request retains its owner")
            .dispatch_key()
    }
    /// Hash of the exact signed request retained by this vacant reservation.
    pub(in crate::sumeragi) fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.owner
            .as_ref()
            .expect("prepared recovered request retains its owner")
            .request_hash()
    }
    /// Arm the closed registry carrier and install the request owner indexes.
    pub(in crate::sumeragi) fn commit(
        mut self,
        prepared_registry: super::v2_lifecycle_coordinator::PreparedRecoveredDecisionFetchDispatchV1<'_>,
        wait_source: super::v2_lifecycle_coordinator::WaitSource,
    ) -> super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1 {
        let owner = self
            .owner
            .take()
            .expect("prepared recovered request retains its exact owner");
        let key = owner.dispatch_key();
        assert_eq!(prepared_registry.dispatch_key(), key);
        let request_hash = owner.request_hash();
        assert!(!self.executor.certified_work.contains_key(&request_hash));
        assert!(!self.executor.outstanding_requests.contains(request_hash));
        assert!(!owner.conflicts_with_ordinary_tracker(&self.executor.outstanding_requests));
        assert!(!self.executor.recovered_decision_fetches.contains_key(&key));
        assert!(
            !self
                .executor
                .recovered_decision_fetch_by_request
                .contains_key(&request_hash)
        );
        assert_eq!(prepared_registry.commit_for_executor(wait_source), key);
        let previous_owner = self.executor.recovered_decision_fetches.insert(key, owner);
        assert!(previous_owner.is_none());
        let previous_reverse = self
            .executor
            .recovered_decision_fetch_by_request
            .insert(request_hash, key);
        assert!(previous_reverse.is_none());
        key
    }
}
/// Exact authenticated response candidate owned by one recovered Decision Fetch.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "the recovered response candidate still requires dedicated persistence"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct RecoveredDecisionFetchResponseCandidateV1 {
    context_id: wire::HeightContextId,
    height: wire::Height,
    key: super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    authenticated_responder: PeerId,
    authenticated_response: AuthenticatedCertifiedBodyResponse,
    fetch_tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    canonical_manifest_hash: HashOf<wire::PayloadManifest>,
    body_payload_hash: Hash,
    claim_preflight: CertifiedBodyResponseClaimPreflight,
}
/// Closed reason a revalidated recovered response could not reserve its exact claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum RecoveredDecisionFetchResponseClaimErrorV1 {
    /// The dedicated request/reverse indexes no longer form one exact census.
    InvalidOwnerIndex,
    /// The request owner or its height context no longer matches the task.
    ForeignOwner,
    /// Another physical response acquired the request family first.
    ConflictingClaim,
}
/// Exclusive response-family reservation held until the dedicated queue tail.
#[must_use = "dropping the reservation leaves the recovered response unclaimed"]
pub(in crate::sumeragi) struct PreparedRecoveredDecisionFetchResponseClaimV1<'executor> {
    executor: &'executor mut V2EffectExecutor<SerializedV2Runtime>,
    key: super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    preflight: CertifiedBodyResponseClaimPreflight,
}
/// Read-only exact retirement plan for one recovered request/response owner.
///
/// It contains only copyable dedicated index identities. The authenticated
/// request and response remain private in the installed owner until the
/// post-fsync assertion tail removes both indexes together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "recovered Decision Fetch request owner has not been retired"]
pub(in crate::sumeragi) struct PreparedRecoveredDecisionFetchOwnerRetirementV1 {
    key: super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
}
impl PreparedRecoveredDecisionFetchResponseClaimV1<'_> {
    /// Atomically install/coalesce the exact claim and publish its preflighted
    /// persistence command. The queue commit is assertion-only while its
    /// mutex and fail-stop operation remain held.
    pub(in crate::sumeragi) fn commit_with_queue(
        self,
        queue: super::v2_worker::LifecycleIoCapacityReservation<'_>,
        task: super::v2_lifecycle_coordinator::RecoveredDecisionFetchBodyPersistenceTaskV1,
    ) {
        let Self {
            executor,
            key,
            response_hash,
            preflight,
        } = self;
        assert_eq!(task.dispatch_key(), key);
        assert_eq!(task.response_hash(), response_hash);
        assert_eq!(task.claim_preflight(), preflight);
        let owner = executor
            .recovered_decision_fetches
            .get_mut(&key)
            .expect("exclusive recovered response reservation retains its request owner");
        assert!(owner.matches_response_claim_preflight(response_hash, preflight));
        assert!(owner.commit_exact_response_claim(response_hash));
        queue.commit_recovered_decision_fetch_body_persistence(task);
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredDecisionFetchResponseCandidateV1 {
    /// Return the dedicated lifecycle owner key.
    pub(in crate::sumeragi) const fn dispatch_key(
        &self,
    ) -> super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1 {
        self.key
    }
    /// Hash of the exact signed request family.
    pub(in crate::sumeragi) const fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.request_hash
    }
    /// Hash of the authenticated physical response.
    pub(in crate::sumeragi) const fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        self.response_hash
    }
    /// Return the recovered Fetch round.
    #[allow(
        dead_code,
        reason = "reviewed recovered-response inspection seam retained for selector diagnostics"
    )]
    pub(in crate::sumeragi) const fn round(&self) -> wire::ConsensusRound {
        self.round
    }
    /// Return the recovered Fetch subject.
    #[allow(
        dead_code,
        reason = "reviewed recovered-response inspection seam retained for selector diagnostics"
    )]
    pub(in crate::sumeragi) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }
    /// Return the exact response-family claim state observed at authentication.
    pub(in crate::sumeragi) const fn claim_preflight(&self) -> CertifiedBodyResponseClaimPreflight {
        self.claim_preflight
    }
    /// Recheck the physical response and authenticated outer responder.
    pub(in crate::sumeragi) fn matches_authenticated_response(
        &self,
        response: &wire::CertifiedBodyResponse,
        authenticated_responder: &PeerId,
    ) -> bool {
        self.response_hash == HashOf::new(response)
            && self.authenticated_responder == *authenticated_responder
            && self.authenticated_response.response() == response
    }
    /// Consume the unique authenticated response into dedicated persistence.
    pub(in crate::sumeragi) fn into_authenticated_response(
        self: Box<Self>,
    ) -> AuthenticatedCertifiedBodyResponse {
        self.authenticated_response
    }
}
/// A certified response which cannot own claimed-response selector priority.
#[derive(Debug, PartialEq, Eq)]
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum CertifiedResponsePriorityNonPriority {
    /// No executor-owned exact certified request names this response.
    Unsolicited {
        /// Request hash carried by the response.
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    },
    /// A different authenticated response already owns the request family.
    ConflictingFamilyClaim {
        /// Exact outstanding request whose family is occupied.
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        /// Authenticated response already owning that family.
        claimed_response_hash: HashOf<wire::CertifiedBodyResponse>,
        /// Different authenticated response being classified.
        incoming_response_hash: HashOf<wire::CertifiedBodyResponse>,
    },
}
/// Opaque executor-owned identity of one authenticated response candidate.
///
/// Minting authenticates the response and all live fetch owners without selector debt.
/// The later transaction must still acquire/coalesce the response claim, plan runtime
/// capacity, and reserve the body-fetch service handoff atomically.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "the candidate still requires transactional admission preflight"]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct CertifiedResponsePriorityCandidate {
    context_id: wire::HeightContextId,
    height: wire::Height,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    authenticated_responder: PeerId,
    authenticated_response: AuthenticatedCertifiedBodyResponse,
    work_id: EffectWorkId,
    fetch_tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    proposal_manifest_hash: Option<HashOf<wire::PayloadManifest>>,
    pending_effect_binding: PendingRuntimeEffectBinding,
    canonical_manifest_hash: HashOf<wire::PayloadManifest>,
    body_payload_hash: Hash,
    claim_preflight: CertifiedBodyResponseClaimPreflight,
}
#[cfg_attr(not(test), allow(dead_code))]
impl CertifiedResponsePriorityCandidate {
    /// Frozen height-context identity owning the exact pending fetch.
    pub(crate) const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }
    /// Frozen height owning the exact pending fetch.
    pub(crate) const fn height(&self) -> wire::Height {
        self.height
    }
    /// Work identifier bound by both `certified_work` and `pending_fetches`.
    pub(crate) const fn work_id(&self) -> EffectWorkId {
        self.work_id
    }
    /// Reducer incarnation retained by the executor-owned fetch task.
    pub(crate) const fn fetch_tag(&self) -> EventTag {
        self.fetch_tag
    }
    /// Hash of the exact signed certified-body request.
    pub(crate) const fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.request_hash
    }
    /// Hash of the complete authenticated response occurrence.
    pub(crate) const fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        self.response_hash
    }
    /// Proposal round retained by the executor-owned fetch task.
    pub(crate) const fn round(&self) -> wire::ConsensusRound {
        self.round
    }
    /// Proposal subject retained by the executor-owned fetch task.
    pub(crate) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }
    /// Proposal-supplied manifest authority, if it already joined the fetch.
    pub(crate) const fn proposal_manifest_hash(&self) -> Option<HashOf<wire::PayloadManifest>> {
        self.proposal_manifest_hash
    }
    /// Sealed ordinal-free binding to the complete pending `FetchBody` effect.
    pub(crate) const fn pending_effect_binding(&self) -> &PendingRuntimeEffectBinding {
        &self.pending_effect_binding
    }
    /// Hash of the canonically rederived complete manifest.
    pub(crate) const fn canonical_manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.canonical_manifest_hash
    }
    /// Hash of the authenticated canonical body bytes.
    pub(crate) const fn body_payload_hash(&self) -> Hash {
        self.body_payload_hash
    }
    /// Read-only family state observed after exact response authentication.
    pub(crate) const fn claim_preflight(&self) -> &CertifiedBodyResponseClaimPreflight {
        &self.claim_preflight
    }
    /// Whether this opaque candidate still names the same signed response and
    /// authenticated outer responder from which it was minted.
    ///
    /// This deliberately does not identify a queue occurrence: an exact wire
    /// retransmission has a distinct physical fair-ingress ordinal. The future
    /// composite transaction must additionally bind the queue-minted pending
    /// ingress identity before it can acquire selector authority.
    pub(crate) fn matches_authenticated_response(
        &self,
        response: &wire::CertifiedBodyResponse,
        authenticated_responder: &PeerId,
    ) -> bool {
        self.response_hash == HashOf::new(response)
            && &self.authenticated_responder == authenticated_responder
            && self.authenticated_response.response() == response
    }
    /// Consume the unique response authority retained by this exact probe.
    ///
    /// No clone or detached constructor is exposed. The lifecycle selector
    /// calls this only after its final equality re-probe has consumed the
    /// complete selected family winner.
    pub(in crate::sumeragi) fn into_authenticated_response(
        self: Box<Self>,
    ) -> AuthenticatedCertifiedBodyResponse {
        self.authenticated_response
    }
}
/// Read-only executor classification of one signed certified response carrier.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "this classification does not itself authorize selector debt"]
#[cfg_attr(not(test), allow(dead_code))]
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(in crate::sumeragi) enum CertifiedResponsePriorityProbe {
    /// The response is provably outside claimed-response priority.
    DefinitelyNonPriority(CertifiedResponsePriorityNonPriority),
    /// Exact authentication succeeded; transactional admission is still required.
    PreflightRequired(Box<CertifiedResponsePriorityCandidate>),
    /// A recovered Decision Fetch owns this family outside ordinary indexes.
    RecoveredPreflightRequired(Box<RecoveredDecisionFetchResponseCandidateV1>),
}
#[derive(Clone, Debug)]
struct PendingSignature {
    tag: EventTag,
    request: SignRequest,
    ownership: RuntimeEffectOwnership,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingFetch {
    task: BodyFetchTask,
    request_hash: Option<HashOf<wire::CertifiedBodyRequest>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StorePurpose {
    Reducer,
    LocalProposal,
}
/// Whether one local body pipeline begins from fresh bytes or deliberately
/// replays an exact cold pre-intent body frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalProposalBodyOrigin {
    Fresh,
    RecoveredPreIntent,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum StoreConsumer {
    Reducer {
        tag: EventTag,
        ownership: RuntimeEffectOwnership,
    },
    LocalProposal {
        tag: EventTag,
        ownership: RuntimeEffectOwnership,
        origin: LocalProposalBodyOrigin,
    },
}
impl StoreConsumer {
    fn new(
        tag: EventTag,
        purpose: StorePurpose,
        ownership: RuntimeEffectOwnership,
        local_origin: LocalProposalBodyOrigin,
    ) -> Self {
        match purpose {
            StorePurpose::Reducer => Self::Reducer { tag, ownership },
            StorePurpose::LocalProposal => Self::LocalProposal {
                tag,
                ownership,
                origin: local_origin,
            },
        }
    }
    const fn tag(&self) -> EventTag {
        match self {
            Self::Reducer { tag, .. } | Self::LocalProposal { tag, .. } => *tag,
        }
    }
    fn ownership(&self) -> &RuntimeEffectOwnership {
        match self {
            Self::Reducer { ownership, .. } | Self::LocalProposal { ownership, .. } => ownership,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingStore {
    task: BodyStoreTask,
    consumer: Option<StoreConsumer>,
}
#[derive(Clone, Debug)]
struct PendingApply {
    task: ApplyTask,
    ownership: RuntimeEffectOwnership,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct ReadyBody {
    manifest: wire::PayloadManifest,
    bytes: Arc<[u8]>,
}
impl ReadyBody {
    fn derive(
        context: &wire::HeightContext,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        bytes: impl Into<Arc<[u8]>>,
    ) -> Result<Self, V2ChunkError> {
        let bytes = bytes.into();
        let manifest = encode_payload(context, round, subject, bytes.as_ref())?
            .manifest()
            .clone();
        Ok(Self { manifest, bytes })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BodyPipelineOwner {
    tag: EventTag,
    manifest_hash: Option<HashOf<wire::PayloadManifest>>,
}
/// Preflighted exact-body owner update.
///
/// Planning performs every fallible identity check. The service/runtime
/// admission happens next; installing this value afterwards is an infallible
/// map replacement because the executor is the sole owner of these maps.
#[derive(Debug, PartialEq, Eq)]
struct BodyPipelineOwnerBindingPlan {
    key: (wire::ConsensusRound, wire::BlockSubject),
    owner: BodyPipelineOwner,
    already_owned: bool,
    checked_effective_lock: CheckedProductionTransition<EffectiveLockTraceProjection>,
}
#[derive(Clone, Copy, Debug)]
struct WorkIdPlan {
    id: EffectWorkId,
    next: u64,
}
#[derive(Clone, Debug)]
struct ReadyBodyReleasePlan {
    key: (wire::ConsensusRound, wire::BlockSubject),
    body: ReadyBody,
    remaining_ready_bytes: u64,
}
#[derive(Clone, Debug)]
struct ReadyBodyInstallPlan {
    key: (wire::ConsensusRound, wire::BlockSubject),
    body: ReadyBody,
    ready_body_bytes: u64,
    release: Option<ReadyBodyReleasePlan>,
}
#[derive(Clone, Debug)]
struct RetainedLockedBodyPlan {
    subject: wire::BlockSubject,
    bytes: Arc<[u8]>,
    install: bool,
    ready_body_bytes: u64,
}
#[derive(Clone, Debug)]
struct RetainedBodyUnionEntry {
    bytes: Arc<[u8]>,
    owners: usize,
    manifests: BTreeMap<wire::ConsensusRound, (wire::PayloadManifest, usize)>,
}
#[derive(Clone, Debug, Default)]
struct RetainedBodyUnion {
    entries: BTreeMap<wire::BlockSubject, RetainedBodyUnionEntry>,
}
impl RetainedBodyUnion {
    fn insert(
        &mut self,
        subject: wire::BlockSubject,
        bytes: Arc<[u8]>,
    ) -> Result<(), EffectExecutorError> {
        if Hash::new(bytes.as_ref()) != subject.payload_hash {
            return Err(EffectExecutorError::Contract(
                "retained canonical bytes differ from their subject payload hash".to_owned(),
            ));
        }
        if let Some(existing) = self.entries.get_mut(&subject) {
            if existing.bytes.as_ref() != bytes.as_ref() {
                return Err(EffectExecutorError::Contract(
                    "one canonical subject has conflicting retained bytes".to_owned(),
                ));
            }
            existing.owners = existing.owners.checked_add(1).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "retained canonical-body owner count overflowed".to_owned(),
                )
            })?;
            return Ok(());
        }
        self.entries.insert(
            subject,
            RetainedBodyUnionEntry {
                bytes,
                owners: 1,
                manifests: BTreeMap::new(),
            },
        );
        Ok(())
    }
    fn insert_manifest(
        &mut self,
        manifest: wire::PayloadManifest,
        bytes: Arc<[u8]>,
    ) -> Result<(), EffectExecutorError> {
        if let Some(entry) = self.entries.get(&manifest.subject)
            && let Some((existing, _)) = entry.manifests.get(&manifest.round)
            && existing != &manifest
        {
            return Err(EffectExecutorError::Contract(
                "one exact body round has conflicting retained manifests".to_owned(),
            ));
        }
        let subject = manifest.subject;
        let round = manifest.round;
        self.insert(subject, bytes)?;
        let entry = self.entries.get_mut(&subject).ok_or_else(|| {
            EffectExecutorError::Contract(
                "retained union insertion lost its exact subject".to_owned(),
            )
        })?;
        match entry.manifests.get_mut(&round) {
            Some((_, owners)) => {
                *owners = owners.checked_add(1).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "retained manifest owner count overflowed".to_owned(),
                    )
                })?;
            }
            None => {
                entry.manifests.insert(round, (manifest, 1));
            }
        }
        Ok(())
    }
    fn remove(
        &mut self,
        subject: wire::BlockSubject,
        bytes: &[u8],
    ) -> Result<(), EffectExecutorError> {
        let Some(existing) = self.entries.get_mut(&subject) else {
            return Err(EffectExecutorError::Contract(
                "planned canonical-body release has no deterministic union owner".to_owned(),
            ));
        };
        if existing.bytes.as_ref() != bytes {
            return Err(EffectExecutorError::Contract(
                "planned canonical-body release differs from deterministic union bytes".to_owned(),
            ));
        }
        if existing.owners > 1 {
            existing.owners -= 1;
        } else {
            self.entries.remove(&subject);
        }
        Ok(())
    }
    fn remove_manifest(
        &mut self,
        manifest: &wire::PayloadManifest,
        bytes: &[u8],
    ) -> Result<(), EffectExecutorError> {
        let Some(entry) = self.entries.get_mut(&manifest.subject) else {
            return Err(EffectExecutorError::Contract(
                "planned manifest release has no deterministic union owner".to_owned(),
            ));
        };
        if entry.bytes.as_ref() != bytes {
            return Err(EffectExecutorError::Contract(
                "planned manifest release differs from deterministic union bytes".to_owned(),
            ));
        }
        let Some((existing, manifest_owners)) = entry.manifests.get_mut(&manifest.round) else {
            return Err(EffectExecutorError::Contract(
                "planned manifest release has no exact round owner".to_owned(),
            ));
        };
        if existing != manifest {
            return Err(EffectExecutorError::Contract(
                "planned manifest release differs from exact retained evidence".to_owned(),
            ));
        }
        if *manifest_owners > 1 {
            *manifest_owners -= 1;
        } else {
            entry.manifests.remove(&manifest.round);
        }
        self.remove(manifest.subject, bytes)
    }
    fn total_bytes(&self) -> Result<u64, EffectExecutorError> {
        self.entries.values().try_fold(0u64, |total, entry| {
            let bytes = u64::try_from(entry.bytes.len()).map_err(|_| {
                EffectExecutorError::Contract(
                    "retained canonical-body byte count is not representable".to_owned(),
                )
            })?;
            total.checked_add(bytes).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "retained canonical-body union byte count overflowed".to_owned(),
                )
            })
        })
    }
}
#[derive(Clone, Debug)]
struct CertifiedFetchRequestPlan {
    work_id: EffectWorkId,
    request: wire::CertifiedBodyRequest,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    registration: CertifiedBodyRequestRegistrationPlan,
}
#[derive(Clone, Debug)]
struct CertifiedFetchRetirementPlan {
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    retirement: CertifiedBodyRequestRetirementPlan,
}
#[derive(Clone, Debug)]
struct PendingFetchRetirementPlan {
    /// Exact service/request owner being retired. Commit also asks runtime to
    /// remove any unpublished BodyAvailable token at these same immutable
    /// coordinates before either executor index is released.
    pending: PendingFetch,
    certified: Option<CertifiedFetchRetirementPlan>,
}
#[derive(Clone, Debug)]
enum StaleFetchTransitionPlan {
    Rebind {
        pending: PendingFetch,
        rebound: BodyFetchTask,
        owner: BodyPipelineOwner,
    },
    Retire(PendingFetchRetirementPlan),
}
/// Preflighted byte ownership retired by one certified-view cleanup.
///
/// The exact residual is computed before any cancellation or runtime queue
/// mutation. The executor installs it only after every fallible callback has
/// acknowledged the planned cleanup.
#[derive(Debug)]
struct CertifiedViewBodyCleanupPlan {
    stale_stores: Vec<EffectWorkId>,
    stale_ready: Vec<(wire::ConsensusRound, wire::BlockSubject)>,
    protected_ready_rebinds: Vec<CertifiedViewReadyRebindPlan>,
    accounting: ExactBodyRetirementAccounting,
    checked_effective_lock: CheckedProductionTransition<EffectiveLockTraceProjection>,
}
#[derive(Clone, Debug)]
struct CertifiedViewReadyRebindPlan {
    key: (wire::ConsensusRound, wire::BlockSubject),
    previous_tag: EventTag,
    manifest: wire::PayloadManifest,
    owner: BodyPipelineOwner,
}
#[derive(Clone, Debug)]
enum FetchReadyCommitPlan {
    Reuse {
        release: Option<ReadyBodyReleasePlan>,
    },
    Install(ReadyBodyInstallPlan),
}
#[derive(Debug)]
struct FetchCompletionPlan {
    work_id: EffectWorkId,
    owner: BodyPipelineOwnerBindingPlan,
    ready: FetchReadyCommitPlan,
    certified_retirement: Option<CertifiedFetchRetirementPlan>,
    runtime_reservation: BodyAvailableReservation,
}
/// Closed executor-side retirement prepared for the coordinator-owned
/// certified-Fetch completion path.
/// Ordinary certified-Fetch Phase B consumes this plan through the live lifecycle transaction.
/// This plan reserves no legacy runtime command and mints no lifecycle
/// ordinal. It freezes only existing exact request, Fetch, and body-pipeline
/// indexes so the post-dequeue tail can retire them without another fallible
/// lookup.
#[must_use = "the prepared Fetch owner has not crossed the exact queue dequeue"]
pub(in crate::sumeragi) struct PreparedLifecycleCertifiedFetchCompletion {
    pending: PendingFetch,
    certified: CertifiedFetchRetirementPlan,
    body_pipeline_key: (wire::ConsensusRound, wire::BlockSubject),
    body_pipeline_owner: BodyPipelineOwner,
    manifest: wire::PayloadManifest,
    durable_receipt: DurableBodyReceipt,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    claim_preflight: CertifiedBodyResponseClaimPreflight,
}
impl PreparedLifecycleCertifiedFetchCompletion {
    /// Borrow the exact service task whose owner must be removed after dequeue.
    pub(in crate::sumeragi) const fn task(&self) -> &BodyFetchTask {
        &self.pending.task
    }
}
#[derive(Debug)]
struct FinalityCompletion {
    tag: EventTag,
    receipt: KuraV2CommitReceipt,
    artifact: wire::finality::V2FinalityArtifact,
    ownership: FinalityCompletionOwner,
}
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Debug)]
enum FinalityCompletionOwner {
    Runtime(RuntimeEffectOwnership),
    LifecycleDecisionApply(LifecycleDecisionApplyDispatchKeyV1),
}
/// One-shot permit for moving post-Ledger lifecycle Decision Apply finality into the executor.
pub(in crate::sumeragi) struct LifecycleDecisionApplyExecutorFinalityPermitV1 {
    _linearity: LifecycleDecisionApplyExecutorFinalityLinearityV1,
}
struct LifecycleDecisionApplyExecutorFinalityLinearityV1;
impl Drop for LifecycleDecisionApplyExecutorFinalityLinearityV1 {
    fn drop(&mut self) {}
}
impl LifecycleDecisionApplyExecutorFinalityPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: LifecycleDecisionApplyExecutorFinalityLinearityV1,
        }
    }
}
/// Preflighted executor transition paired with one lifecycle Decision Apply queue cut.
///
/// Ordinary lifecycle Apply dispatches carry an inert instance. During
/// interrupted-tip recovery this token exclusively borrows the exact pending
/// evidence until the worker reservation has installed its command, preventing
/// `ApplicationDispatched` from becoming observable before physical ownership
/// exists.
#[must_use = "the executor Apply-dispatch transition must commit with its worker reservation"]
pub(in crate::sumeragi) struct PreparedLifecycleDecisionApplyExecutorDispatchV1<'executor> {
    pending: Option<PendingKuraApplyDispatchTransitionV1<'executor>>,
    successor_outputs: Option<PendingLifecycleDecisionApplySuccessorOutputsTransitionV1<'executor>>,
}
struct PendingKuraApplyDispatchTransitionV1<'executor> {
    evidence: &'executor mut PendingKuraApplyRecoveryEvidence,
    last_result: &'executor mut Option<PendingTipRecoveryAttemptResult>,
}
struct PendingLifecycleDecisionApplySuccessorOutputsTransitionV1<'executor> {
    installed: &'executor mut Option<AttestedLifecycleDecisionApplySuccessorOutputsV1>,
    retained_effect_batch: &'executor mut Option<RetainedEffectBatch>,
    attestation: AttestedLifecycleDecisionApplySuccessorOutputsV1,
}
impl PreparedLifecycleDecisionApplyExecutorDispatchV1<'_> {
    /// Advance exact pending-Kura evidence after the worker command is installed.
    pub(in crate::sumeragi) fn commit_after_worker_dispatch(self) {
        if let Some(successor_outputs) = self.successor_outputs {
            assert!(
                successor_outputs.installed.is_none(),
                "preflighted post-Apply output proof retains an empty install slot"
            );
            let retained = successor_outputs
                .retained_effect_batch
                .take()
                .expect("preflighted post-Apply output proof retains its exact Apply suffix");
            assert!(
                retained.effects.len() == 1
                    && retained.effects.front().is_some_and(|owned| {
                        successor_outputs
                            .attestation
                            .exactly_matches_retransmit_apply(&owned.effect)
                    }),
                "preflighted post-Apply output proof retains the same exact Apply suffix"
            );
            *successor_outputs.installed = Some(successor_outputs.attestation);
        }
        if let Some(pending) = self.pending {
            assert_eq!(
                pending.evidence.stage,
                PendingKuraApplyRecoveryStage::Apply,
                "preflighted pending-Kura Apply remains at its dispatch boundary"
            );
            pending.evidence.stage = PendingKuraApplyRecoveryStage::ApplicationDispatched;
            *pending.last_result = Some(PendingTipRecoveryAttemptResult::Advanced);
        }
    }
}
/// Executor-authenticated global application-mode debt for lifecycle planning.
///
/// The debt is exactly one until Kura's typed application completion is
/// retained and zero afterwards. Draining unrelated runtime/output queues is
/// intentionally not part of this component.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "the mode observation must be consumed by the composite planner snapshot"]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct LifecycleModeRankSnapshot {
    context_id: wire::HeightContextId,
    height: wire::Height,
    debt: u64,
}
#[cfg_attr(not(test), allow(dead_code))]
impl LifecycleModeRankSnapshot {
    /// Frozen height-context identity owning this mode observation.
    pub(crate) const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }
    /// Frozen height owning this mode observation.
    pub(crate) const fn height(&self) -> wire::Height {
        self.height
    }
    /// Exact application-mode debt (`1` before durable Apply, `0` after).
    pub(crate) const fn debt(&self) -> u64 {
        self.debt
    }
}
impl FinalityCompletion {
    /// Whether this exact durable terminal authorizes the same committed
    /// decision rediscovered before this height rolls over.
    fn matches_apply(
        &self,
        tag: EventTag,
        context: &wire::HeightContext,
        subject: wire::BlockSubject,
        certificate: &wire::QuorumCertificate,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        self.tag == tag
            && matches!(&self.ownership, FinalityCompletionOwner::Runtime(retained) if retained == ownership)
            && self.artifact.validate().is_ok()
            && self.artifact.height_context == *context
            && self.artifact.subject == subject
            && self
                .artifact
                .commit_qc
                .as_ref()
                .same_commit_decision(certificate.as_ref())
            && self.receipt.height() == context.height
            && self.receipt.context_id() == context.id()
            && self.receipt.block_hash() == subject.block_hash
            && self.receipt.subject() == subject
            && self.receipt.certificate() == self.artifact.commit_qc.as_ref()
            && self.receipt.artifact_hash() == HashOf::new(&self.artifact)
    }
}
/// Full immutable identity of one durable reducer Decision.
///
/// The execution commitment is part of consensus identity even when round and
/// subject are unchanged. Keeping the complete tuple in executor ownership
/// prevents a later corrupted runtime observation from being mistaken for an
/// idempotent reconciliation of the first Decision.
type DurableDecision = (
    wire::ConsensusRound,
    wire::ConsensusRound,
    wire::BlockSubject,
    wire::ExecutionCommitment,
);
/// Exact local owner which installed one durable Decision before runner cleanup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingRunnerDecisionCleanup {
    decision: DurableDecision,
    owner_tag: EventTag,
}
/// Exact executor-retained owner for one live lifecycle Apply corridor.
///
/// This is deliberately not generic pending work: the registry and bounded
/// lifecycle queue own physical execution. The executor retains the complete
/// immutable decision only to coalesce reducer retransmits and to prove that
/// the post-Ledger completion consumes the same live carrier.
struct LiveLifecycleDecisionApplyOwnerV1 {
    dispatch_key: LifecycleDecisionApplyDispatchKeyV1,
    tag: EventTag,
    subject: wire::BlockSubject,
    certificate: wire::QuorumCertificate,
    validated_receipt: ValidatedBodyReceipt,
    decision: DurableDecision,
}
/// Preliminary exact owner retained from a published or sidecar-woken
/// Validate successor until that row either publishes a typed non-Apply
/// outcome or upgrades to the full live Apply owner.
struct LiveLifecycleValidateSuccessorOwnerV1 {
    dispatch_key: LifecycleValidateDispatchKeyV1,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    apply_is_authorized: bool,
}
impl LiveLifecycleValidateSuccessorOwnerV1 {
    /// Return whether a freshly attested carrier is the sole legal physical
    /// refinement of this same logical Validate row.
    fn can_refine_to(&self, candidate: &Self) -> bool {
        self.dispatch_key != candidate.dispatch_key
            && self.apply_is_authorized
            && self.dispatch_key.owner() == candidate.dispatch_key.owner()
            && self.dispatch_key.lifecycle_ordinal()
                == candidate.dispatch_key.lifecycle_ordinal()
            && self.dispatch_key.slot() == candidate.dispatch_key.slot()
            && self.round == candidate.round
            && self.subject == candidate.subject
    }

    fn exactly_matches_apply(
        &self,
        subject: wire::BlockSubject,
        certificate: &wire::QuorumCertificate,
    ) -> bool {
        self.apply_is_authorized
            && self.dispatch_key.lifecycle_ordinal() != 0
            && self.round == certificate.proposal_round
            && self.subject == subject
            && certificate.subject == subject
            && certificate.phase == wire::GlobalPhase::Commit
    }

    fn exactly_precedes_live_apply(
        &self,
        authority: &LiveLifecycleDecisionApplyReconciliationAuthorityV1,
    ) -> bool {
        let certificate = authority.certificate();
        let validate_predecessor_ordinal = authority.validate_predecessor_ordinal();
        self.apply_is_authorized
            && self.dispatch_key.owner() == authority.dispatch_key().owner()
            && validate_predecessor_ordinal != 0
            && self.dispatch_key.lifecycle_ordinal() == validate_predecessor_ordinal
            && validate_predecessor_ordinal < authority.dispatch_key().lifecycle_ordinal()
            && self.round == certificate.proposal_round
            && self.subject == authority.subject()
            && certificate.subject == self.subject
    }
}
impl LiveLifecycleDecisionApplyOwnerV1 {
    fn exactly_matches(
        &self,
        dispatch_key: LifecycleDecisionApplyDispatchKeyV1,
        tag: EventTag,
        subject: wire::BlockSubject,
        certificate: &wire::QuorumCertificate,
        validated_receipt: &ValidatedBodyReceipt,
        decision: DurableDecision,
    ) -> bool {
        self.dispatch_key == dispatch_key
            && dispatch_key.lineage() == LifecycleDecisionApplyLineageV1::Live
            && self.tag == tag
            && self.subject == subject
            && self.certificate == *certificate
            && self.validated_receipt == *validated_receipt
            && self.decision == decision
    }
    fn exactly_matches_retransmit(
        &self,
        tag: EventTag,
        subject: wire::BlockSubject,
        certificate: &wire::QuorumCertificate,
    ) -> bool {
        self.tag == tag
            && self.subject == subject
            && self.certificate == *certificate
            && self.decision
                == (
                    certificate.round,
                    certificate.proposal_round,
                    subject,
                    certificate.execution_commitment,
                )
            && self.validated_receipt.execution_commitment() == certificate.execution_commitment
    }
    fn exactly_matches_completion(
        &self,
        dispatch_key: LifecycleDecisionApplyDispatchKeyV1,
        tag: EventTag,
        subject: wire::BlockSubject,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> bool {
        self.dispatch_key == dispatch_key
            && dispatch_key.lineage() == LifecycleDecisionApplyLineageV1::Live
            && self.tag == tag
            && self.subject == subject
            && artifact.subject == subject
            && artifact.commit_qc.as_ref() == self.certificate.as_ref()
            && self.validated_receipt.execution_commitment()
                == self.certificate.execution_commitment
            && receipt.subject() == subject
            && receipt.certificate() == self.certificate.as_ref()
            && receipt.artifact_hash() == HashOf::new(artifact)
    }
}
/// One atomic read of reducer state which can retire executor-owned work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeReconciliationFrontier {
    tag: Option<EventTag>,
    locked_body: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    /// Strongest durable PrepareQC retained for bounded cleanup authority.
    /// This is never a voting lock and cannot drive Fetch/rebind behavior.
    highest_prepare: Option<wire::QuorumCertificateRef>,
    lock_is_authoritative: bool,
    decision: Option<DurableDecision>,
}
/// Closed body-owner retirement predicate shared by pre-dispatch filtering and
/// the service-owning lock reconciliation commit.
fn protected_lock_retires_body_key(
    superseded: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    replacement: (wire::ConsensusRound, wire::BlockSubject),
    key: (wire::ConsensusRound, wire::BlockSubject),
) -> bool {
    let (replacement_round, replacement_subject) = replacement;
    let (round, subject) = key;
    match superseded {
        Some((old_round, old_subject)) if old_subject == replacement_subject => {
            subject == old_subject && round == old_round
        }
        Some((_, old_subject)) => {
            subject == old_subject
                || (subject != replacement_subject
                    && round.context_id == replacement_round.context_id
                    && round.height == replacement_round.height
                    && round.view <= replacement_round.view)
        }
        None => {
            round.context_id == replacement_round.context_id
                && round.height == replacement_round.height
                && if subject == replacement_subject {
                    round.view < replacement_round.view
                } else {
                    round.view <= replacement_round.view
                }
        }
    }
}
fn protected_lock_body(
    protected_lock: Option<&wire::QuorumCertificate>,
) -> Option<(wire::ConsensusRound, wire::BlockSubject)> {
    protected_lock.map(|certificate| (certificate.proposal_round, certificate.subject))
}
fn highest_prepare_body(
    highest_prepare: Option<wire::QuorumCertificateRef>,
) -> Option<(wire::ConsensusRound, wire::BlockSubject)> {
    highest_prepare.map(|certificate| (certificate.proposal_round, certificate.subject))
}
/// One adapter macro-step's causal suffix waiting for bounded dispatch capacity.
///
/// The adapter bounds every serialized invocation by [`MAX_EFFECTS_PER_STEP`],
/// including any synchronous persistence continuation, so retaining the
/// unconsumed suffix preserves exact FIFO order without creating an
/// independently growing queue. This queue is intentionally volatile: after
/// process restart each progress item is reconstructed from the source
/// classified by [`RestartEffectSource`] rather than from this adapter memory.
#[derive(Debug)]
struct RetainedEffectBatch {
    effects: VecDeque<OwnedAdapterEffect>,
    oldest_at: Instant,
}
