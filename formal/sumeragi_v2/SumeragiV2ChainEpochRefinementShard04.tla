---- MODULE SumeragiV2ChainEpochRefinementShard04 ----
EXTENDS SumeragiV2ChainEpochRefinementShard03

THEOREM IndexedQuorumConfigurationMatchesBase ==
  \A initialContext:
    IndexedAsync(initialContext)!QuorumConfiguration
      <=> QuorumConfiguration
BY DEF IndexedAsync!QuorumConfiguration,
       IndexedAsync!Epochs, IndexedAsync!RosterSequence,
       IndexedAsync!VotingRoster, IndexedAsync!ValidatorIds,
       IndexedAsync!VotingPower, IndexedAsync!PowerUnits,
       IndexedAsync!PowerOf, IndexedAsync!Byzantine,
       IndexedAsync!Cardinality, IndexedAsync!IsFiniteSet,
       QuorumConfiguration, Epochs, RosterSequence, VotingRoster,
       ValidatorIds, VotingPower, PowerUnits, PowerOf, Byzantine,
       Cardinality, IsFiniteSet

THEOREM IndexedBodyHeldByMatchesBase ==
  \A initialContext, durable, node, bodyContext, roundView, subject:
    IndexedAsync(initialContext)!BodyHeldBy(
      durable, node, bodyContext, roundView, subject)
      <=> BodyHeldBy(durable, node, bodyContext, roundView, subject)
BY DEF IndexedAsync!BodyHeldBy, IndexedAsync!BodyRecord,
       BodyHeldBy, BodyRecord

(***************************************************************************
Full temporal proof facts are instantiated over the identical concrete tuple
at one arbitrary free module constant.  `IndexedAsync` remains the
authoritative production-network relation, and `IndexedAsyncSafetyProof`
supplies only the three context-indexed non-temporal aliases above.  This fixed
proof-only instance contributes the complete temporal closure without adding
alternate state or a second step.  A theorem conditional on VerificationContext
membership is semantically valid for every interpretation of that constant.
***************************************************************************)
VerificationCore(component) ==
  IndexedCore(VerificationContext, component)

VerificationScheduler(component) ==
  IndexedScheduler(VerificationContext, component)

VerificationRecovery(component) ==
  IndexedRecovery(VerificationContext, component)

VerificationProducer(component) ==
  IndexedProducer(VerificationContext, component)

VerificationFixedCorridorDeadlines ==
  IndexedFixedCorridorDeadlines(VerificationContext)

VerificationServeProducerTurnDue ==
  IndexedServeProducerTurnDue(VerificationContext)

VerificationAsyncProof ==
  INSTANCE SumeragiV2AsyncTemporalClosureProofs
    WITH
       height <- VerificationCore(1),
       context <- VerificationCore(2),
       contextHistory <- VerificationCore(3),
       nodeView <- VerificationCore(4),
       generation <- VerificationCore(5),
       up <- VerificationCore(6),
       gst <- VerificationCore(7),
       availableBodies <- VerificationCore(8),
       durableBodies <- VerificationCore(9),
       retainedLockedBodies <- VerificationCore(10),
       validatedBodies <- VerificationCore(11),
       invalidBodies <- VerificationCore(12),
       seenProposals <- VerificationCore(13),
       receivedVotes <- VerificationCore(14),
       receivedQCs <- VerificationCore(15),
       receivedTimeoutVotes <- VerificationCore(16),
       receivedTCs <- VerificationCore(17),
       proposalIntents <- VerificationCore(18),
       prepareIntents <- VerificationCore(19),
       commitIntents <- VerificationCore(20),
       timeoutIntents <- VerificationCore(21),
       prepareQCs <- VerificationCore(22),
       commitQCs <- VerificationCore(23),
       formedTCs <- VerificationCore(24),
       installedTCs <- VerificationCore(25),
       lastInstalledTc <- VerificationCore(26),
       lockPrepareQc <- VerificationCore(27),
       highestPrepareQc <- VerificationCore(28),
       lockRank <- VerificationCore(29),
       lockSubject <- VerificationCore(30),
       highestRank <- VerificationCore(31),
       highestSubject <- VerificationCore(32),
       pendingProposal <- VerificationCore(33),
       pendingPrepare <- VerificationCore(34),
       pendingObservePrepare <- VerificationCore(35),
       pendingLockCommit <- VerificationCore(36),
       pendingTimeout <- VerificationCore(37),
       pendingInstallTC <- VerificationCore(38),
       pendingDecision <- VerificationCore(39),
       signProposals <- VerificationCore(40),
       signVotes <- VerificationCore(41),
       signTimeouts <- VerificationCore(42),
       proposalNetwork <- VerificationCore(43),
       voteNetwork <- VerificationCore(44),
       qcNetwork <- VerificationCore(45),
       timeoutNetwork <- VerificationCore(46),
       tcNetwork <- VerificationCore(47),
       decisions <- VerificationCore(48),
       applied <- VerificationCore(49),
       asyncNow <- VerificationScheduler(1),
       asyncCommandQueues <- VerificationScheduler(2),
       asyncNextCommandClass <- VerificationScheduler(3),
       asyncFifoOwed <- VerificationScheduler(4),
       asyncTimeoutEmitted <- VerificationScheduler(5),
       asyncRunnerPhase <- VerificationScheduler(6),
       asyncRunnerBudget <- VerificationScheduler(7),
       asyncCausalAdmissionOwed <- VerificationScheduler(8),
       asyncNextLocalSource <- VerificationScheduler(9),
       asyncIoQueues <- VerificationScheduler(10),
       asyncNextServeAdmissionOrdinal <- VerificationScheduler(11),
       asyncNextServeIngressOrdinal <- VerificationScheduler(12),
       asyncServeIngressAdmissions <- VerificationScheduler(13),
       asyncServeAdmissions <- VerificationScheduler(14),
       asyncServeReservations <- VerificationScheduler(15),
       asyncServeTombstones <- VerificationScheduler(16),
       asyncServeAttempts <- VerificationScheduler(17),
       asyncOutstandingWork <- VerificationScheduler(18),
       asyncIoReadyCompletions <- VerificationScheduler(19),
       asyncLocalReadyCompletions <- VerificationScheduler(20),
       asyncNextCompletionSource <- VerificationScheduler(21),
       asyncIoControlAvailable <- VerificationScheduler(22),
       asyncDeferredCompletionQueues <- VerificationScheduler(23),
       asyncDeferredProgressQueues <- VerificationScheduler(24),
       asyncDeferredNormalQueues <- VerificationScheduler(25),
       asyncDeferredHandoffs <- VerificationScheduler(26),
       asyncNextDeferredClass <- VerificationScheduler(27),
       asyncDeferredDrainOwed <- VerificationScheduler(28),
       asyncCausalQueues <- VerificationScheduler(29),
       asyncOutstandingTags <- VerificationScheduler(30),
       asyncNodeDeadlines <- VerificationScheduler(31),
       asyncRetransmitDeadlines <- VerificationScheduler(32),
       asyncNodeServiceDeadlines <- VerificationScheduler(33),
       asyncIoServiceDeadlines <- VerificationScheduler(34),
       asyncSentItems <- VerificationScheduler(35),
       asyncRetainedControl <- VerificationScheduler(36),
       asyncActiveRequests <- VerificationScheduler(37),
       asyncCertifiedResponseClaim <- VerificationScheduler(38),
       asyncTransport <- VerificationScheduler(39),
       asyncIngressLanes <- VerificationScheduler(40),
       asyncIngressReady <- VerificationScheduler(41),
       asyncLeaderWireLifecycles <- VerificationScheduler(42),
       asyncHeldChunks <- VerificationScheduler(43),
       asyncHistoricalRecoveryTargets <- VerificationScheduler(44),
       asyncControlServiceState <- VerificationScheduler(45),
       asyncServiceActivationState <- VerificationScheduler(46),
       asyncRecoveryPhase <- VerificationRecovery(1),
       asyncRecoveryNode <- VerificationRecovery(2),
       asyncRecoveryGeneration <- VerificationRecovery(3),
       asyncRecoveryReplayQueue <- VerificationRecovery(4),
       asyncHistoricalLockRestartAuthorities <- VerificationRecovery(5),
       asyncProducerKnownObligations <- VerificationProducer(1),
       asyncProducerConsumedEpisodes <- VerificationProducer(2),
       asyncProducerOriginHistory <- VerificationProducer(3),
       asyncFixedCorridorDeadlines <- VerificationFixedCorridorDeadlines,
       asyncServeProducerTurnReady <-
         VerificationServeProducerTurnDue

AdmissibleContextRecords ==
  {initialContext \in ContextRecords:
     FrozenContextAdmissible(initialContext)}

IndexedAsyncStateShape ==
  /\ DOMAIN indexedAsyncState = AdmissibleContextRecords
  /\ \A initialContext \in AdmissibleContextRecords:
       /\ Len(indexedAsyncState[initialContext]) = 7
       /\ DOMAIN indexedAsyncState[initialContext] = 1..7
       /\ indexedAsyncState[initialContext][1]
            = indexedAsyncState[initialContext][2][7]
       /\ Len(indexedAsyncState[initialContext][2]) = 49
       /\ DOMAIN indexedAsyncState[initialContext][2] = 1..49
       /\ Len(indexedAsyncState[initialContext][3]) = 46
       /\ DOMAIN indexedAsyncState[initialContext][3] = 1..46
       /\ Len(indexedAsyncState[initialContext][4]) = 5
       /\ DOMAIN indexedAsyncState[initialContext][4] = 1..5
       /\ Len(indexedAsyncState[initialContext][5]) = 3
       /\ DOMAIN indexedAsyncState[initialContext][5] = 1..3

JoinedByContextShape ==
  joinedByContext \in [AdmissibleContextRecords -> SUBSET ValidatorIds]

GenesisContext == ContextRecord(0, <<>>)

CanonicalIndexedContext(blockHeight) ==
  Chain!ContextRecord(blockHeight, Chain!HistoryThrough(blockHeight))

JoinedContexts ==
  {initialContext \in AdmissibleContextRecords:
     joinedByContext[initialContext] # {}}

JoinedCanonicalDescendant(initialContext) ==
  \E descendantContext \in JoinedContexts:
    /\ descendantContext.height > initialContext.height
    /\ descendantContext =
         Chain!ContextRecord(descendantContext.height,
                             Chain!HistoryThrough(descendantContext.height))

IndexedNodeCurrentAt(initialContext, node) ==
  /\ node \in joinedByContext[initialContext]
  /\ nodeContext[node] = initialContext

ExactNodeLocationAt(initialContext, node) ==
  /\ nodeHeight[node] = initialContext.height
  /\ nodeContext[node] = initialContext

IndexedDecisions(initialContext) == IndexedCore(initialContext, 48)
IndexedApplications(initialContext) == IndexedCore(initialContext, 49)

(***************************************************************************
InitAt for a non-genesis context contains one synthetic parent receipt. It is
private bootstrap evidence for that exact one-height proof, not a receipt
created by the context itself. The ChainEpoch projection therefore selects
only receipts whose frozen context and height are exactly this instance. This
keeps every bootstrap receipt available internally while making the indexed
genesis receipt union genuinely empty.
***************************************************************************)
IndexedCurrentDecisions(initialContext) ==
  {decision \in IndexedDecisions(initialContext):
     /\ decision.qc.context = initialContext
     /\ decision.qc.height = initialContext.height}

IndexedCurrentApplications(initialContext) ==
  {application \in IndexedApplications(initialContext):
     /\ application.qc.context = initialContext
     /\ application.qc.height = initialContext.height}

IndexedDecisionEvidence ==
  UNION {IndexedCurrentDecisions(initialContext):
           initialContext \in AdmissibleContextRecords}

IndexedApplicationEvidence ==
  UNION {IndexedCurrentApplications(initialContext):
           initialContext \in AdmissibleContextRecords}

SuccessorActivationStatusValues ==
  {"Idle", "Queued", "Running", "Complete"}

SuccessorPredecessorOwnershipValues == {"Published", "Absent"}

SuccessorActivationRequiredPrerequisites ==
  {"DeferredStatus", "AdapterReady", "RuntimeReady", "ServicesReady",
   "StartupApplied", "ClocksArmed", "IngressOpen"}

SuccessorActivationAdapterPrerequisites ==
  {"DeferredStatus", "AdapterReady"}

SuccessorActivationRuntimePrerequisites ==
  SuccessorActivationAdapterPrerequisites \cup {"RuntimeReady"}

SuccessorActivationServicePrerequisites ==
  SuccessorActivationRuntimePrerequisites \cup {"ServicesReady"}

SuccessorActivationStartupPrerequisites ==
  SuccessorActivationServicePrerequisites \cup {"StartupApplied"}

SuccessorActivationClockPrerequisites ==
  SuccessorActivationStartupPrerequisites \cup {"ClocksArmed"}

SuccessorActivationOwnerSet ==
  [parentContext: AdmissibleContextRecords, node: ValidatorIds]

SuccessorActivationTokenSet ==
  [kind: {"Applied", "Recovered"},
   parentContext: AdmissibleContextRecords,
   node: ValidatorIds,
   successorContext: AdmissibleContextRecords]

CompleteTipRecoveryAuthoritySet ==
  [kind: {"CompleteTip"},
   parentContext: AdmissibleContextRecords,
   node: ValidatorIds,
   successorContext: AdmissibleContextRecords,
   application: Chain!DecisionEvidenceSet]

SnapshotBootstrapRecoveryAuthoritySet ==
  [kind: {"SnapshotBootstrap"},
   parentContext: AdmissibleContextRecords,
   node: ValidatorIds,
   successorContext: AdmissibleContextRecords]

SuccessorRecoveryAuthoritySet ==
  CompleteTipRecoveryAuthoritySet \cup
    SnapshotBootstrapRecoveryAuthoritySet

SuccessorActivationMarkerSet ==
  [parentContext: AdmissibleContextRecords,
   node: ValidatorIds,
   successorContext: AdmissibleContextRecords,
   successorHeight: Heights,
   generation: Generations,
   view: Views,
   transition: {"SuccessorHeightActivated"}]

SuccessorActivationOwner(parentContext, node) ==
  [parentContext |-> parentContext, node |-> node]

SuccessorActivationToken(kind, parentContext, node, successorContext) ==
  [kind |-> kind,
   parentContext |-> parentContext,
   node |-> node,
   successorContext |-> successorContext]

CompleteTipRecoveryAuthorityRecord(parentContext, node,
                                   successorContext, application) ==
  [kind |-> "CompleteTip",
   parentContext |-> parentContext,
   node |-> node,
   successorContext |-> successorContext,
   application |-> application]

SnapshotBootstrapRecoveryAuthorityRecord(parentContext, node,
                                         successorContext) ==
  [kind |-> "SnapshotBootstrap",
   parentContext |-> parentContext,
   node |-> node,
   successorContext |-> successorContext]

SuccessorActivationMarker(parentContext, node, successorContext) ==
  [parentContext |-> parentContext,
   node |-> node,
   successorContext |-> successorContext,
   successorHeight |-> successorContext.height,
   generation |-> 0,
   view |-> 0,
   transition |-> "SuccessorHeightActivated"]

SuccessorActivationVars ==
  <<successorActivationStatus,
    successorPredecessorStatusOwnership,
    successorActivationPrerequisites,
    successorActivationTokens,
    successorRecoveryAuthorities,
    preparedSuccessorActivationMarkers,
    publishedSuccessorActivationMarkers,
    successorActivationFailures,
    successorActivationFailureHistory,
    successorActivationCompletions>>

SuccessorActivationShape ==
  /\ successorActivationStatus
       \in [AdmissibleContextRecords ->
             [ValidatorIds -> SuccessorActivationStatusValues]]
  /\ successorPredecessorStatusOwnership
       \in [AdmissibleContextRecords ->
             [ValidatorIds -> SuccessorPredecessorOwnershipValues]]
  /\ successorActivationPrerequisites
       \in [AdmissibleContextRecords ->
             [ValidatorIds -> SUBSET SuccessorActivationRequiredPrerequisites]]
  /\ successorActivationTokens \subseteq SuccessorActivationTokenSet
  /\ successorRecoveryAuthorities \subseteq SuccessorRecoveryAuthoritySet
  /\ preparedSuccessorActivationMarkers
       \subseteq SuccessorActivationMarkerSet
  /\ publishedSuccessorActivationMarkers
       \subseteq SuccessorActivationMarkerSet
  /\ successorActivationFailures \subseteq SuccessorActivationOwnerSet
  /\ successorActivationFailureHistory \subseteq SuccessorActivationOwnerSet
  /\ successorActivationCompletions \subseteq SuccessorActivationTokenSet

IndexedDecisionReceiptProjection ==
  durableDecisionEvidence = IndexedDecisionEvidence

IndexedApplicationReceiptProjection ==
  durableApplicationEvidence = IndexedApplicationEvidence

IndexedTotalReceiptProjection ==
  /\ IndexedDecisionReceiptProjection
  /\ IndexedApplicationReceiptProjection

(***************************************************************************
Authenticated historical recovery is owned by the exact Async instance.

The chain wrapper may open recovery only for a responsive node located at the
frozen context and only when a joined responsive applied archive still holds
the body certified by the exact CommitQC.  The archive authenticates its own
response but need not be one of the old QC's signers: the QC authorizes the
immutable subject, while the frozen-roster archive and exact body hash bind the
historical source. `OpenHistoricalRecovery` records that exact target in
scheduler component 44. From then on the ordinary Async reducer persists the
decision, recovers and stores the body, validates it, and appends the
application to the same per-context `decisions` and `applied` sets used by
ordinary consensus. There is no shadow receipt set, stage variable, or
independent recovery step.
***************************************************************************)
HistoricalRecoveryRecord(node, source) ==
  [node |-> node, qc |-> source.qc]

IndexedHistoricalRecoveryTargetReady(initialContext, node) ==
  /\ node \in Responsive
  /\ node \in IndexedCore(initialContext, 6)
  /\ node \in joinedByContext[initialContext]
  /\ ExactNodeLocationAt(initialContext, node)
  /\ ~IndexedAsync(initialContext)!NodeHasDecision(node)
  /\ ~IndexedProjectedNodeHasApplication(initialContext, node)
  /\ ~IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)

IndexedHistoricalRecoverySourceReady(initialContext, server, source) ==
  /\ initialContext \in JoinedContexts
  /\ source \in IndexedCurrentDecisions(initialContext)
  /\ source \in IndexedCurrentApplications(initialContext)
  /\ source \in durableDecisionEvidence
  /\ source \in durableApplicationEvidence
  /\ source.node = server
  /\ \/ /\ initialContext.height < MaxHeight
        /\ Chain!CanonicalCommitForSlot(
             source.qc, initialContext.height + 1)
     \/ /\ initialContext.height = MaxHeight
        /\ Chain!ReceiptOutsideChainHorizon(source)
  /\ server \in IndexedAsync(initialContext)!
                 AsyncCurrentResponsiveVoters
  /\ server \in IndexedCore(initialContext, 6)
  /\ server \in joinedByContext[initialContext]
  /\ BodyHeldBy(IndexedCore(initialContext, 9), server,
                 initialContext, source.qc.view, source.qc.subject)

IndexedHistoricalRecoveryReady(initialContext, node) ==
  /\ node \in Responsive
  /\ node \in IndexedCore(initialContext, 6)
  /\ node \in joinedByContext[initialContext]
  /\ ExactNodeLocationAt(initialContext, node)
  /\ ~IndexedAsync(initialContext)!NodeHasDecision(node)
  /\ ~IndexedProjectedNodeHasApplication(initialContext, node)
  /\ \E server \in ValidatorIds,
       source \in Chain!DecisionEvidenceSet:
       IndexedHistoricalRecoverySourceReady(
         initialContext, server, source)

IndexedOpenHistoricalRecovery(initialContext, node, server, source) ==
  /\ IndexedHistoricalRecoveryTargetReady(initialContext, node)
  /\ IndexedHistoricalRecoverySourceReady(
       initialContext, server, source)
  /\ IndexedAsync(initialContext)!OpenHistoricalRecovery(node)

IndexedChainVars ==
  <<indexedAsyncState, joinedByContext,
    SuccessorActivationVars, Chain!ChainEpochVars>>

(***************************************************************************
The joined runner is a restriction of the exact AsyncNext relation, never an
alternate step. Current consensus work requires only the selected node's join;
both RunNode and direct Commit-certificate discovery also require that this is
the node's authoritative current context. Historical serving and outstanding
IO remain enabled for every node that ever joined the context, even after its
authoritative nodeContext advances.
***************************************************************************)
IndexedJoinedRunnerStep(initialContext) ==
  \/ \E node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters:
       /\ IndexedNodeCurrentAt(initialContext, node)
       /\ IndexedAsync(initialContext)!RunNode(node)
  \/ \E node \in Responsive:
       IndexedAsync(initialContext)!RunHistoricalRecoveryNode(node)
  \/ \E node \in Responsive:
       /\ node \in joinedByContext[initialContext]
       /\ IndexedAsync(initialContext)!RunHistoricalServer(node)

IndexedJoinedNonRunnerStep(initialContext) ==
  /\ \/ IndexedAsync(initialContext)!AsyncSetGST
     \/ IndexedAsync(initialContext)!AsyncTick
     \/ \E node \in IndexedAsync(initialContext)!
                    AsyncCurrentResponsiveVoters:
          /\ IndexedNodeCurrentAt(initialContext, node)
          /\ IndexedAsync(initialContext)!
               DirectCommitCertificateDiscoveryStep(node)
     \/ \E node \in Responsive:
          IndexedAsync(initialContext)!
            DirectHistoricalCommitCertificateDiscoveryStep(node)
     \/ \E node \in Responsive:
          IndexedAsync(initialContext)!
            ServiceHistoricalRecoveryIoWorker(node)
     \/ \E node \in Responsive:
          IndexedAsync(initialContext)!
            EnqueueHistoricalRecoveryIoLocalControl(node)
     \/ \E node \in ValidatorIds,
           server \in ValidatorIds,
           source \in Chain!DecisionEvidenceSet:
          IndexedOpenHistoricalRecovery(
            initialContext, node, server, source)
     \/ \E node \in Responsive:
          /\ node \in joinedByContext[initialContext]
          /\ IndexedAsync(initialContext)!ServiceIoWorker(node)
     \/ \E node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters:
          /\ node \in joinedByContext[initialContext]
          /\ IndexedAsync(initialContext)!EnqueueIoLocalControl(node)
     \/ \E node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters:
          /\ IndexedNodeCurrentAt(initialContext, node)
          /\ IndexedAsync(initialContext)!
               ResolveCandidateProducerContinuation(node)
     \/ IndexedAsync(initialContext)!AsyncNetworkStep
     \/ IndexedAsync(initialContext)!AsyncFaultStep
  /\ UNCHANGED IndexedScheduler(initialContext, 33)

IndexedJoinedNonCrashStep(initialContext) ==
  /\ (IndexedJoinedRunnerStep(initialContext)
        \/ IndexedJoinedNonRunnerStep(initialContext))
  /\ UNCHANGED <<IndexedCore(initialContext, 6),
                 IndexedAsync(initialContext)!AsyncRecoveryControlVars>>

IndexedJoinedAsyncNext(initialContext) ==
  /\ (IndexedJoinedNonCrashStep(initialContext)
        \/ \E node \in ValidatorIds:
             IndexedAsync(initialContext)!PreGstCrash(node))
  /\ IndexedAsync(initialContext)!
       AsyncHistoricalLockRestartAuthorityTransition
  /\ IndexedAsync(initialContext)!AsyncProducerProjectionStep
  \* Service activation belongs only to the atomic successor-publication
  \* actions below.  An ordinary joined-context step must retain component 46;
  \* the unchanged arm of AsyncServiceActivationTransition then refines the
  \* exact standalone AsyncNext relation without activating an unjoined peer.
  /\ UNCHANGED IndexedScheduler(initialContext, 46)
  /\ UNCHANGED <<IndexedCore(initialContext, 1),
                 IndexedCore(initialContext, 2)>>
  /\ [IndexedAsync(initialContext)!Next]_(
       IndexedAsync(initialContext)!vars)

NewIndexedDecisionReceipt(initialContext, decision) ==
  /\ decision \notin IndexedDecisions(initialContext)
  /\ IndexedDecisions(initialContext)' =
       IndexedDecisions(initialContext) \cup {decision}
  /\ IndexedApplications(initialContext)' =
       IndexedApplications(initialContext)

NewIndexedApplicationReceipt(initialContext, application) ==
  /\ application \notin IndexedApplications(initialContext)
  /\ IndexedApplications(initialContext)' =
       IndexedApplications(initialContext) \cup {application}
  /\ IndexedDecisions(initialContext)' =
       IndexedDecisions(initialContext)

NoNewIndexedDurableReceipt(initialContext) ==
  /\ IndexedDecisions(initialContext)' =
       IndexedDecisions(initialContext)
  /\ IndexedApplications(initialContext)' =
       IndexedApplications(initialContext)

IndexedDecisionReceiptHandoff(initialContext, decision) ==
  /\ NewIndexedDecisionReceipt(initialContext, decision)
  /\ UNCHANGED <<joinedByContext, SuccessorActivationVars>>
  /\ \/ Chain!RecordCertifiedNext(decision)
     \/ Chain!RecordKnownDecision(decision)

SuccessorContextFor(application) ==
  CanonicalIndexedContext(application.qc.context.height + 1)

ExactDurableParentApplication(parentContext, node, application) ==
  /\ parentContext.height < MaxHeight
  /\ application \in durableDecisionEvidence
  /\ application \in durableApplicationEvidence
  /\ application.node = node
  /\ application.qc.context = parentContext
  /\ application.qc.height = parentContext.height
  /\ Chain!CanonicalCommitForSlot(
       application.qc, parentContext.height + 1)
  /\ nodeHeight[node] = parentContext.height + 1
  /\ nodeContext[node] =
       CanonicalIndexedContext(parentContext.height + 1)

ExactSuccessorActivationToken(kind, parentContext, node,
                              successorContext) ==
  /\ successorContext =
       CanonicalIndexedContext(parentContext.height + 1)
  /\ SuccessorActivationToken(
       kind, parentContext, node, successorContext)
       \in successorActivationTokens

ExactCompleteTipRecoveryAuthority(parentContext, node,
                                  successorContext, application) ==
  /\ ExactDurableParentApplication(parentContext, node, application)
  /\ successorContext =
       CanonicalIndexedContext(parentContext.height + 1)
  /\ CompleteTipRecoveryAuthorityRecord(
       parentContext, node, successorContext, application)
       \in successorRecoveryAuthorities

ExactSnapshotBootstrapRecoveryAuthority(parentContext, node,
                                        successorContext) ==
  /\ successorContext =
       CanonicalIndexedContext(parentContext.height + 1)
  /\ SnapshotBootstrapRecoveryAuthorityRecord(
       parentContext, node, successorContext)
       \in successorRecoveryAuthorities

(***************************************************************************
Snapshot bootstrap is an initialization authority, not evidence that the
local database contains the exact CommitQC-backed complete tip.  Its tagged
record deliberately has no application field, and the complete-tip
credential predicate above accepts only the independently tagged authority
whose application is present in both durable evidence sets.  A later genesis
handoff proof may refine snapshot startup; it cannot discharge exact-tip
recovery by projecting the imported height into an `Option<Height>`.
***************************************************************************)
THEOREM SnapshotBootstrapAuthorityIsDistinctFromCompleteTipAuthority ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds,
     successorContext \in AdmissibleContextRecords,
     application \in Chain!DecisionEvidenceSet:
    SnapshotBootstrapRecoveryAuthorityRecord(
      parentContext, node, successorContext)
      # CompleteTipRecoveryAuthorityRecord(
          parentContext, node, successorContext, application)
BY Isa DEF SnapshotBootstrapRecoveryAuthorityRecord,
           CompleteTipRecoveryAuthorityRecord

QueueSuccessorActivation(parentContext, node) ==
  /\ parentContext.height < MaxHeight
  /\ successorActivationStatus[parentContext][node] = "Idle"
  /\ successorPredecessorStatusOwnership[parentContext][node] = "Absent"
  /\ SuccessorActivationOwner(parentContext, node)
       \notin successorActivationFailures
  /\ successorActivationStatus' =
       [successorActivationStatus EXCEPT
          ![parentContext][node] = "Queued"]
  /\ successorPredecessorStatusOwnership' =
       [successorPredecessorStatusOwnership EXCEPT
          ![parentContext][node] = "Published"]
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = {}]
  /\ UNCHANGED <<successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions>>

(***************************************************************************
RecordAppliedNext is defined only below the finite chain horizon. The certified
prefix supplies a valid subject at every position in its exact successor
lineage, so the context inserted into joinedByContext is always an existing
member of the pre-created admissible domain.
***************************************************************************)
THEOREM AppliedSuccessorIsAdmissible ==
  \A application \in Chain!DecisionEvidenceSet:
    Chain!ChainEpochInvariant /\ Chain!RecordAppliedNext(application)
      => /\ SuccessorContextFor(application)
               \in AdmissibleContextRecords
         /\ SuccessorContextFor(application).height
               = nodeHeight[application.node] + 1
BY Isa DEF SuccessorContextFor, AdmissibleContextRecords,
           FrozenContextAdmissible, ContextRecords, LineagesAt,
           Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
           Chain!CertifiedPrefixBacked, Chain!RecordAppliedNext,
           Chain!CanonicalCommitForSlot, Chain!HistoryThrough

IndexedApplicationReceiptHandoff(initialContext, application) ==
  /\ NewIndexedApplicationReceipt(initialContext, application)
  /\ \/ /\ ExactNodeLocationAt(initialContext, application.node)
        /\ Chain!RecordAppliedNext(application)
        /\ QueueSuccessorActivation(initialContext, application.node)
        /\ UNCHANGED joinedByContext
     \/ /\ Chain!RecordKnownApplication(application)
        /\ UNCHANGED <<joinedByContext, SuccessorActivationVars>>

IndexedReceiptFreeChainStutter(initialContext) ==
  /\ NoNewIndexedDurableReceipt(initialContext)
  /\ UNCHANGED <<joinedByContext, SuccessorActivationVars,
                  Chain!ChainEpochVars>>

IndexedReceiptClassification(initialContext) ==
  \/ IndexedReceiptFreeChainStutter(initialContext)
  \/ \E decision \in Chain!DecisionEvidenceSet:
       IndexedDecisionReceiptHandoff(initialContext, decision)
  \/ \E application \in Chain!DecisionEvidenceSet:
       IndexedApplicationReceiptHandoff(initialContext, application)

(***************************************************************************
Successor activation is a durable, ordered protocol rather than an atomic
side effect of application.  `successorPredecessorStatusOwnership` models the
process-visible predecessor registry entry independently from durable parent
application evidence. Applied startup may publish that predecessor entry. A
clean process restart and a restart after a latched failure both rehydrate the
exact durable complete tip while the predecessor entry is absent. Failure and
restart are separate lifecycle actions: an Applied failure leaves its visible
status Running until restart, and a Recovered attempt may fail again. Both
paths share the ordered startup pipeline, but only Applied publication writes
physical `Complete`. Snapshot bootstrap remains a separate initialization
authority and is never accepted by the complete-tip credential below.
***************************************************************************)
SuccessorActivationEnvironmentStutter ==
  /\ UNCHANGED indexedAsyncState
  /\ UNCHANGED Chain!ChainEpochVars

(***************************************************************************
The final publication step also activates the exact successor-height service
owner.  Every pre-created Async instance starts from the canonical standalone
initializer.  Its first independent join irreversibly restricts service to
that node; every later join monotonically adds one node and rearms both local
service clocks from the successor instance's current proof time.  No other
context changes, and the activation record is internal scheduler metadata.
***************************************************************************)
SuccessorActivationEnvironmentActivatesNode(successorContext, node) ==
  /\ IF joinedByContext[successorContext] = {}
     THEN IndexedAsync(successorContext)!
            AsyncEnterIndexedServiceActivation(node)
     ELSE IndexedAsync(successorContext)!AsyncActivateServiceNode(node)
  /\ \A otherContext \in
          AdmissibleContextRecords \ {successorContext}:
       UNCHANGED IndexedAsyncStateAt(otherContext)
  /\ UNCHANGED Chain!ChainEpochVars

SuccessorActivationCredentialReady(parentContext, node,
                                   successorContext) ==
  /\ successorActivationStatus[parentContext][node] = "Running"
  /\ SuccessorActivationOwner(parentContext, node)
       \notin successorActivationFailures
  /\ \/ /\ successorPredecessorStatusOwnership[parentContext][node]
              = "Published"
        /\ ExactSuccessorActivationToken(
             "Applied", parentContext, node, successorContext)
     \/ /\ successorPredecessorStatusOwnership[parentContext][node]
              = "Absent"
        /\ ExactSuccessorActivationToken(
             "Recovered", parentContext, node, successorContext)
        /\ \E application \in Chain!DecisionEvidenceSet:
             ExactCompleteTipRecoveryAuthority(
               parentContext, node, successorContext, application)

BeginSuccessorActivation(parentContext, node, successorContext) ==
  LET token == SuccessorActivationToken(
                 "Applied", parentContext, node, successorContext)
  IN /\ successorContext =
          CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node] = "Queued"
     /\ successorPredecessorStatusOwnership[parentContext][node] = "Published"
     /\ successorActivationPrerequisites[parentContext][node] = {}
     /\ token \notin successorActivationTokens
     /\ \E application \in Chain!DecisionEvidenceSet:
          ExactDurableParentApplication(parentContext, node, application)
     /\ successorActivationStatus' =
          [successorActivationStatus EXCEPT
             ![parentContext][node] = "Running"]
     /\ UNCHANGED <<successorPredecessorStatusOwnership,
                     successorActivationPrerequisites,
                     successorActivationTokens,
                     successorRecoveryAuthorities,
                     preparedSuccessorActivationMarkers,
                     publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

BindAppliedSuccessorActivationToken(parentContext, node,
                                    successorContext) ==
  LET token == SuccessorActivationToken(
                  "Applied", parentContext, node, successorContext)
  IN /\ successorContext =
           CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorPredecessorStatusOwnership[parentContext][node]
          = "Published"
     /\ successorActivationPrerequisites[parentContext][node] = {}
     /\ SuccessorActivationOwner(parentContext, node)
          \notin successorActivationFailures
     /\ \E application \in Chain!DecisionEvidenceSet:
          ExactDurableParentApplication(parentContext, node, application)
     /\ token \notin successorActivationTokens
     /\ successorActivationTokens' =
          successorActivationTokens \cup {token}
     /\ UNCHANGED <<successorActivationStatus,
                     successorPredecessorStatusOwnership,
                     successorActivationPrerequisites,
                     successorRecoveryAuthorities,
                     preparedSuccessorActivationMarkers,
                     publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

LatchAppliedSuccessorStartupFailure(parentContext, node) ==
  LET owner == SuccessorActivationOwner(parentContext, node)
  IN /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorPredecessorStatusOwnership[parentContext][node]
          = "Published"
     /\ owner \notin successorActivationFailures
     /\ \E application \in Chain!DecisionEvidenceSet:
          ExactDurableParentApplication(parentContext, node, application)
     /\ successorActivationPrerequisites' =
          [successorActivationPrerequisites EXCEPT
             ![parentContext][node] = {}]
     /\ successorActivationTokens' =
          {token \in successorActivationTokens:
             \/ token.parentContext # parentContext
             \/ token.node # node}
     /\ successorRecoveryAuthorities' =
          {authority \in successorRecoveryAuthorities:
             \/ authority.parentContext # parentContext
             \/ authority.node # node}
     /\ preparedSuccessorActivationMarkers' =
          {marker \in preparedSuccessorActivationMarkers:
             \/ marker.parentContext # parentContext
             \/ marker.node # node}
     /\ successorActivationFailures' =
          successorActivationFailures \cup {owner}
     /\ successorActivationFailureHistory' =
          successorActivationFailureHistory \cup {owner}
     /\ UNCHANGED <<successorActivationStatus,
                     successorPredecessorStatusOwnership,
                     publishedSuccessorActivationMarkers,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

LatchRecoveredSuccessorStartupFailure(parentContext, node,
                                      successorContext, application) ==
  LET owner == SuccessorActivationOwner(parentContext, node)
      token == SuccessorActivationToken(
                 "Recovered", parentContext, node, successorContext)
  IN /\ successorContext =
           CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorPredecessorStatusOwnership[parentContext][node] = "Absent"
     /\ owner \notin successorActivationFailures
     /\ token \in successorActivationTokens
     /\ ExactCompleteTipRecoveryAuthority(
          parentContext, node, successorContext, application)
     /\ successorActivationPrerequisites' =
          [successorActivationPrerequisites EXCEPT
             ![parentContext][node] = {}]
     /\ successorActivationTokens' =
          {candidate \in successorActivationTokens:
             \/ candidate.parentContext # parentContext
             \/ candidate.node # node}
     /\ successorRecoveryAuthorities' =
          {authority \in successorRecoveryAuthorities:
             \/ authority.parentContext # parentContext
             \/ authority.node # node}
     /\ preparedSuccessorActivationMarkers' =
          {marker \in preparedSuccessorActivationMarkers:
             \/ marker.parentContext # parentContext
             \/ marker.node # node}
     /\ successorActivationFailures' =
          successorActivationFailures \cup {owner}
     /\ successorActivationFailureHistory' =
          successorActivationFailureHistory \cup {owner}
     /\ UNCHANGED <<successorActivationStatus,
                     successorPredecessorStatusOwnership,
                     publishedSuccessorActivationMarkers,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

RehydrateCleanCompleteTipSuccessorStartup(parentContext, node,
                                          successorContext, application) ==
  LET owner == SuccessorActivationOwner(parentContext, node)
      authority == CompleteTipRecoveryAuthorityRecord(
                     parentContext, node, successorContext, application)
  IN /\ successorContext =
           CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node]
          \in {"Queued", "Running"}
     /\ successorPredecessorStatusOwnership[parentContext][node]
          = "Published"
     /\ owner \notin successorActivationFailures
     /\ ExactDurableParentApplication(parentContext, node, application)
     /\ successorActivationStatus' =
          [successorActivationStatus EXCEPT
             ![parentContext][node] = "Queued"]
     /\ successorPredecessorStatusOwnership' =
          [successorPredecessorStatusOwnership EXCEPT
             ![parentContext][node] = "Absent"]
     /\ successorActivationPrerequisites' =
          [successorActivationPrerequisites EXCEPT
             ![parentContext][node] = {}]
     /\ successorActivationTokens' =
          {token \in successorActivationTokens:
             \/ token.parentContext # parentContext
             \/ token.node # node}
     /\ successorRecoveryAuthorities' =
          {candidate \in successorRecoveryAuthorities:
             \/ candidate.parentContext # parentContext
             \/ candidate.node # node}
            \cup {authority}
     /\ preparedSuccessorActivationMarkers' =
          {marker \in preparedSuccessorActivationMarkers:
             \/ marker.parentContext # parentContext
             \/ marker.node # node}
     /\ UNCHANGED <<publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

RehydrateFailedSuccessorStartup(parentContext, node,
                                successorContext, application) ==
  LET owner == SuccessorActivationOwner(parentContext, node)
      authority == CompleteTipRecoveryAuthorityRecord(
                     parentContext, node, successorContext, application)
  IN /\ successorContext =
           CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorPredecessorStatusOwnership[parentContext][node]
          \in {"Published", "Absent"}
     /\ owner \in successorActivationFailures
     /\ ExactDurableParentApplication(parentContext, node, application)
     /\ successorActivationStatus' =
          [successorActivationStatus EXCEPT
             ![parentContext][node] = "Queued"]
     /\ successorPredecessorStatusOwnership' =
          [successorPredecessorStatusOwnership EXCEPT
             ![parentContext][node] = "Absent"]
     /\ successorActivationPrerequisites' =
          [successorActivationPrerequisites EXCEPT
             ![parentContext][node] = {}]
     /\ successorActivationTokens' =
          {token \in successorActivationTokens:
             \/ token.parentContext # parentContext
             \/ token.node # node}
     /\ successorRecoveryAuthorities' =
          {candidate \in successorRecoveryAuthorities:
             \/ candidate.parentContext # parentContext
             \/ candidate.node # node}
            \cup {authority}
     /\ preparedSuccessorActivationMarkers' =
          {marker \in preparedSuccessorActivationMarkers:
             \/ marker.parentContext # parentContext
             \/ marker.node # node}
     /\ successorActivationFailures' =
          successorActivationFailures \ {owner}
     /\ UNCHANGED <<publishedSuccessorActivationMarkers,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

AuthenticateRecoveredSuccessorActivation(parentContext, node,
                                          successorContext, application) ==
  LET owner == SuccessorActivationOwner(parentContext, node)
      token == SuccessorActivationToken(
                 "Recovered", parentContext, node, successorContext)
      authority == CompleteTipRecoveryAuthorityRecord(
                     parentContext, node, successorContext, application)
  IN /\ successorContext =
           CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node] = "Queued"
     /\ successorPredecessorStatusOwnership[parentContext][node] = "Absent"
     /\ successorActivationPrerequisites[parentContext][node] = {}
     /\ owner \notin successorActivationFailures
     /\ ExactDurableParentApplication(parentContext, node, application)
     /\ authority \in successorRecoveryAuthorities
     /\ token \notin successorActivationTokens
     /\ successorActivationStatus' =
          [successorActivationStatus EXCEPT
             ![parentContext][node] = "Running"]
     /\ successorActivationTokens' =
          successorActivationTokens \cup {token}
     /\ UNCHANGED <<successorPredecessorStatusOwnership,
                     successorActivationPrerequisites,
                     successorRecoveryAuthorities,
                     preparedSuccessorActivationMarkers,
                     publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

OpenDeferredSuccessorAdapter(parentContext, node, successorContext) ==
  /\ SuccessorActivationCredentialReady(
       parentContext, node, successorContext)
  /\ successorActivationPrerequisites[parentContext][node] = {}
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = SuccessorActivationAdapterPrerequisites]
  /\ UNCHANGED <<successorActivationStatus,
                  successorPredecessorStatusOwnership,
                  successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions,
                  joinedByContext>>
  /\ SuccessorActivationEnvironmentStutter

ConstructSuccessorRuntime(parentContext, node, successorContext) ==
  /\ SuccessorActivationCredentialReady(
       parentContext, node, successorContext)
  /\ successorActivationPrerequisites[parentContext][node]
       = SuccessorActivationAdapterPrerequisites
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = SuccessorActivationRuntimePrerequisites]
  /\ UNCHANGED <<successorActivationStatus,
                  successorPredecessorStatusOwnership,
                  successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions,
                  joinedByContext>>
  /\ SuccessorActivationEnvironmentStutter

StartSuccessorServices(parentContext, node, successorContext) ==
  /\ SuccessorActivationCredentialReady(
       parentContext, node, successorContext)
  /\ successorActivationPrerequisites[parentContext][node]
       = SuccessorActivationRuntimePrerequisites
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = SuccessorActivationServicePrerequisites]
  /\ UNCHANGED <<successorActivationStatus,
                  successorPredecessorStatusOwnership,
                  successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions,
                  joinedByContext>>
  /\ SuccessorActivationEnvironmentStutter

ApplySuccessorStartupEffects(parentContext, node, successorContext) ==
  /\ SuccessorActivationCredentialReady(
       parentContext, node, successorContext)
  /\ successorActivationPrerequisites[parentContext][node]
       = SuccessorActivationServicePrerequisites
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = SuccessorActivationStartupPrerequisites]
  /\ UNCHANGED <<successorActivationStatus,
                  successorPredecessorStatusOwnership,
                  successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions,
                  joinedByContext>>
  /\ SuccessorActivationEnvironmentStutter

ArmSuccessorClocks(parentContext, node, successorContext) ==
  /\ SuccessorActivationCredentialReady(
       parentContext, node, successorContext)
  /\ successorActivationPrerequisites[parentContext][node]
       = SuccessorActivationStartupPrerequisites
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = SuccessorActivationClockPrerequisites]
  /\ UNCHANGED <<successorActivationStatus,
                  successorPredecessorStatusOwnership,
                  successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions,
                  joinedByContext>>
  /\ SuccessorActivationEnvironmentStutter

PrepareSuccessorActivationMarker(parentContext, node, successorContext) ==
  LET marker == SuccessorActivationMarker(
                  parentContext, node, successorContext)
  IN /\ SuccessorActivationCredentialReady(
           parentContext, node, successorContext)
     /\ successorActivationPrerequisites[parentContext][node]
          = SuccessorActivationClockPrerequisites
     /\ marker \notin preparedSuccessorActivationMarkers
     /\ marker \notin publishedSuccessorActivationMarkers
     /\ preparedSuccessorActivationMarkers' =
          preparedSuccessorActivationMarkers \cup {marker}
     /\ UNCHANGED <<successorActivationStatus,
                     successorPredecessorStatusOwnership,
                     successorActivationPrerequisites,
                     successorActivationTokens,
                     successorRecoveryAuthorities,
                     publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

OpenSuccessorIngress(parentContext, node, successorContext) ==
  LET marker == SuccessorActivationMarker(
                  parentContext, node, successorContext)
  IN /\ SuccessorActivationCredentialReady(
           parentContext, node, successorContext)
     /\ successorActivationPrerequisites[parentContext][node]
          = SuccessorActivationClockPrerequisites
     /\ marker \in preparedSuccessorActivationMarkers
     /\ successorActivationPrerequisites' =
          [successorActivationPrerequisites EXCEPT
             ![parentContext][node] =
               SuccessorActivationRequiredPrerequisites]
     /\ UNCHANGED <<successorActivationStatus,
                     successorPredecessorStatusOwnership,
                     successorActivationTokens,
                     successorRecoveryAuthorities,
                     preparedSuccessorActivationMarkers,
                     publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

ActivateAppliedSuccessorHeight(parentContext, node, successorContext) ==
  LET token == SuccessorActivationToken(
                  "Applied", parentContext, node, successorContext)
      marker == SuccessorActivationMarker(
                   parentContext, node, successorContext)
  IN /\ SuccessorActivationCredentialReady(
           parentContext, node, successorContext)
     /\ ExactSuccessorActivationToken(
          "Applied", parentContext, node, successorContext)
     /\ successorPredecessorStatusOwnership[parentContext][node]
          = "Published"
     /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorActivationPrerequisites[parentContext][node]
          = SuccessorActivationRequiredPrerequisites
     /\ marker \in preparedSuccessorActivationMarkers
     /\ SuccessorActivationOwner(parentContext, node)
          \notin successorActivationFailures
     /\ successorActivationStatus' =
          [successorActivationStatus EXCEPT
             ![parentContext][node] = "Complete"]
     /\ successorPredecessorStatusOwnership' =
          [successorPredecessorStatusOwnership EXCEPT
             ![parentContext][node] = "Absent"]
     /\ successorActivationTokens' = successorActivationTokens \ {token}
     /\ preparedSuccessorActivationMarkers' =
          preparedSuccessorActivationMarkers \ {marker}
     /\ publishedSuccessorActivationMarkers' =
          publishedSuccessorActivationMarkers \cup {marker}
     /\ successorActivationCompletions' =
          successorActivationCompletions \cup {token}
     /\ joinedByContext' =
          [joinedByContext EXCEPT ![successorContext] = @ \cup {node}]
     /\ UNCHANGED <<successorActivationPrerequisites,
                     successorRecoveryAuthorities,
                     successorActivationFailures,
                     successorActivationFailureHistory>>
     /\ SuccessorActivationEnvironmentActivatesNode(
          successorContext, node)

ActivateRecoveredSuccessorHeight(parentContext, node, successorContext) ==
  LET token == SuccessorActivationToken(
                  "Recovered", parentContext, node, successorContext)
      marker == SuccessorActivationMarker(
                   parentContext, node, successorContext)
  IN /\ SuccessorActivationCredentialReady(
           parentContext, node, successorContext)
     /\ ExactSuccessorActivationToken(
          "Recovered", parentContext, node, successorContext)
     /\ successorPredecessorStatusOwnership[parentContext][node] = "Absent"
     /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorActivationPrerequisites[parentContext][node]
          = SuccessorActivationRequiredPrerequisites
     /\ marker \in preparedSuccessorActivationMarkers
     /\ SuccessorActivationOwner(parentContext, node)
          \notin successorActivationFailures
     /\ \E application \in Chain!DecisionEvidenceSet:
          ExactCompleteTipRecoveryAuthority(
            parentContext, node, successorContext, application)
     /\ UNCHANGED successorActivationStatus
     /\ successorActivationTokens' = successorActivationTokens \ {token}
     /\ successorRecoveryAuthorities' =
          {authority \in successorRecoveryAuthorities:
             \/ authority.parentContext # parentContext
             \/ authority.node # node}
     /\ preparedSuccessorActivationMarkers' =
          preparedSuccessorActivationMarkers \ {marker}
     /\ publishedSuccessorActivationMarkers' =
          publishedSuccessorActivationMarkers \cup {marker}
     /\ successorActivationCompletions' =
          successorActivationCompletions \cup {token}
     /\ joinedByContext' =
          [joinedByContext EXCEPT ![successorContext] = @ \cup {node}]
     /\ UNCHANGED <<successorPredecessorStatusOwnership,
                     successorActivationPrerequisites,
                     successorActivationFailures,
                     successorActivationFailureHistory>>
     /\ SuccessorActivationEnvironmentActivatesNode(
          successorContext, node)

SuccessorHeightActivated(parentContext, node) ==
  LET successorContext ==
        CanonicalIndexedContext(parentContext.height + 1)
      marker == SuccessorActivationMarker(
                  parentContext, node, successorContext)
  IN /\ parentContext.height < MaxHeight
     /\ successorPredecessorStatusOwnership[parentContext][node] = "Absent"
     /\ marker \in publishedSuccessorActivationMarkers
     /\ node \in joinedByContext[successorContext]
     /\ \/ /\ SuccessorActivationToken(
                  "Applied", parentContext, node, successorContext)
                  \in successorActivationCompletions
            /\ successorActivationStatus[parentContext][node] = "Complete"
        \/ /\ SuccessorActivationToken(
                  "Recovered", parentContext, node, successorContext)
                  \in successorActivationCompletions
            /\ successorActivationStatus[parentContext][node] = "Running"

IndexedSuccessorActivationProgressStep(parentContext, node) ==
  /\ SuccessorActivationShape
  /\ \E successorContext \in AdmissibleContextRecords:
       \/ BeginSuccessorActivation(parentContext, node, successorContext)
       \/ BindAppliedSuccessorActivationToken(
            parentContext, node, successorContext)
       \/ \E application \in Chain!DecisionEvidenceSet:
            \/ LatchRecoveredSuccessorStartupFailure(
                 parentContext, node, successorContext, application)
            \/ RehydrateCleanCompleteTipSuccessorStartup(
                 parentContext, node, successorContext, application)
            \/ RehydrateFailedSuccessorStartup(
                 parentContext, node, successorContext, application)
            \/ AuthenticateRecoveredSuccessorActivation(
                 parentContext, node, successorContext, application)
       \/ LatchAppliedSuccessorStartupFailure(parentContext, node)
       \/ OpenDeferredSuccessorAdapter(
            parentContext, node, successorContext)
       \/ ConstructSuccessorRuntime(parentContext, node, successorContext)
       \/ StartSuccessorServices(parentContext, node, successorContext)
       \/ ApplySuccessorStartupEffects(
            parentContext, node, successorContext)
       \/ ArmSuccessorClocks(parentContext, node, successorContext)
       \/ PrepareSuccessorActivationMarker(
            parentContext, node, successorContext)
       \/ OpenSuccessorIngress(parentContext, node, successorContext)
       \/ ActivateAppliedSuccessorHeight(
            parentContext, node, successorContext)
       \/ ActivateRecoveredSuccessorHeight(
            parentContext, node, successorContext)
  /\ SuccessorActivationShape'

SuccessorStartupFailureStep(parentContext, node) ==
  \/ LatchAppliedSuccessorStartupFailure(parentContext, node)
  \/ \E successorContext \in AdmissibleContextRecords,
       application \in Chain!DecisionEvidenceSet:
       LatchRecoveredSuccessorStartupFailure(
         parentContext, node, successorContext, application)

SuccessorStartupRestartStep(parentContext, node) ==
  \E successorContext \in AdmissibleContextRecords,
     application \in Chain!DecisionEvidenceSet:
    \/ RehydrateCleanCompleteTipSuccessorStartup(
         parentContext, node, successorContext, application)
    \/ RehydrateFailedSuccessorStartup(
         parentContext, node, successorContext, application)

SuccessorActivationAdvancingStep(parentContext, node) ==
  /\ IndexedSuccessorActivationProgressStep(parentContext, node)
  /\ ~SuccessorStartupFailureStep(parentContext, node)
  /\ ~SuccessorStartupRestartStep(parentContext, node)

(***************************************************************************
This is the runtime premise excluded by FLP-style reasoning: for every
responsive local owner there is a suffix with no further startup failure
transition. It does not bound or count failures before that suffix. Combined
with weak fairness of the terminating local activation worker, a rehydrated
attempt can traverse the finite startup pipeline.
***************************************************************************)
EventualFailureFreeSuccessorStartupSuffix ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    <>[](SuccessorActivationOwner(parentContext, node)
           \notin successorActivationFailures)

FiniteHorizonSuccessorProjectionDormant ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    terminalContext.height = MaxHeight
      => /\ successorActivationStatus[terminalContext][node] = "Idle"
         /\ successorPredecessorStatusOwnership[terminalContext][node]
              = "Absent"

(***************************************************************************
The indexed product deliberately excludes responsive-process crash/restart
transitions.  Every pre-created Async instance therefore remains in its
initialized recovery phase.  This state invariant is the exact reason that
the six responsive restart/replay weak-fairness clauses of AsyncSpecAt are
vacuous in the product projection; absence from the product action inventory
alone would not be sufficient without the recovery-control frame above.
***************************************************************************)
IndexedResponsiveRecoveryDormant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedRecovery(initialContext, 1) = "Eligible"

SuccessorPublicationOrSuperseded(parentContext, node) ==
  \/ SuccessorHeightActivated(parentContext, node)
  \/ nodeHeight[node] > parentContext.height + 1

IndexedSuccessorActivationPending(parentContext, node) ==
  /\ parentContext \in AdmissibleContextRecords
  /\ node \in ValidatorIds
  /\ parentContext.height < MaxHeight
  /\ successorActivationStatus[parentContext][node]
       \in {"Queued", "Running"}
  /\ ~SuccessorPublicationOrSuperseded(parentContext, node)

IndexedSuccessorActivationProgress ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedSuccessorActivationPending(parentContext, node)
      ~> SuccessorPublicationOrSuperseded(parentContext, node)

IndexedReceiptFreeAsyncAction(initialContext) ==
  /\ IndexedJoinedAsyncNext(initialContext)
  /\ NoNewIndexedDurableReceipt(initialContext)

IndexedFreshReceiptAsyncAction(initialContext) ==
  /\ IndexedJoinedAsyncNext(initialContext)
  /\ \/ \E decision \in Chain!DecisionEvidenceSet:
            NewIndexedDecisionReceipt(initialContext, decision)
     \/ \E application \in Chain!DecisionEvidenceSet:
            NewIndexedApplicationReceipt(initialContext, application)

IndexedProductActionAt(initialContext) ==
  /\ IndexedJoinedAsyncNext(initialContext)
  /\ \A otherContext \in AdmissibleContextRecords \ {initialContext}:
       UNCHANGED IndexedAsyncStateAt(otherContext)
  /\ IndexedAsyncStateShape'
  /\ JoinedByContextShape'
  /\ SuccessorActivationShape'
  /\ IndexedReceiptClassification(initialContext)

IndexedChainNext ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ SuccessorActivationShape
  /\ \/ \E initialContext \in JoinedContexts:
          IndexedProductActionAt(initialContext)
     \/ \E parentContext \in AdmissibleContextRecords,
           node \in ValidatorIds:
          IndexedSuccessorActivationProgressStep(parentContext, node)

IndexedChainInit ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ SuccessorActivationShape
  /\ \A initialContext \in AdmissibleContextRecords:
       IndexedAsync(initialContext)!AsyncInitAt(initialContext)
  /\ Chain!ChainEpochInit
  /\ joinedByContext =
       [initialContext \in AdmissibleContextRecords |->
          IF initialContext = GenesisContext
          THEN ValidatorIds
          ELSE {}]
  /\ successorActivationStatus =
       [parentContext \in AdmissibleContextRecords |->
          [node \in ValidatorIds |-> "Idle"]]
  /\ successorPredecessorStatusOwnership =
       [parentContext \in AdmissibleContextRecords |->
          [node \in ValidatorIds |-> "Absent"]]
  /\ successorActivationPrerequisites =
       [parentContext \in AdmissibleContextRecords |->
          [node \in ValidatorIds |-> {}]]
  /\ successorActivationTokens = {}
  /\ successorRecoveryAuthorities = {}
  /\ preparedSuccessorActivationMarkers = {}
  /\ publishedSuccessorActivationMarkers = {}
  /\ successorActivationFailures = {}
  /\ successorActivationFailureHistory = {}
  /\ successorActivationCompletions = {}
  /\ IndexedTotalReceiptProjection

(***************************************************************************
Fairness is attached to full indexed-product steps. Dormant contexts make each
action disabled. After the first independent join, the instance scheduler and
transport become fair. Node-attributed consensus work is fair after that node
joins, while direct Commit-certificate discovery is fair only for its current
context. Successor activation is weakly fair only for Responsive validators;
an honest validator outside Responsive may retain queued local work forever
without strengthening the conditional production liveness target.
***************************************************************************)
IndexedSetGstStep(initialContext) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!AsyncSetGST

IndexedTickStep(initialContext) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!AsyncTick

\* Bare AsyncTick is enabled before GST, including in a dormant pre-created
\* instance which the product intentionally cannot schedule.  Fixed-clock
\* historical recovery only consumes Tick after GST, so this is the exact
\* local action whose fairness can soundly refine the indexed product.
IndexedPostGstTick(initialContext) ==
  /\ IndexedCore(initialContext, 7)
  /\ IndexedAsync(initialContext)!AsyncTick

IndexedRunNodeStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!PostGstRunNode(node)

IndexedOpenHistoricalRecoveryStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ \E server \in ValidatorIds,
       source \in Chain!DecisionEvidenceSet:
       IndexedOpenHistoricalRecovery(
         initialContext, node, server, source)

IndexedRunHistoricalRecoveryStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstRunHistoricalRecoveryNode(node)

IndexedCommitCertificateDiscoveryStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!
       PostGstCommitCertificateDiscovery(node)

IndexedHistoricalCommitCertificateDiscoveryStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstHistoricalCommitCertificateDiscovery(node)

IndexedHistoricalServerStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ node \in joinedByContext[initialContext]
  /\ IndexedAsync(initialContext)!PostGstRunHistoricalServer(node)

IndexedIoWorkerStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ node \in joinedByContext[initialContext]
  /\ IndexedAsync(initialContext)!PostGstServiceIoWorker(node)

IndexedHistoricalRecoveryIoWorkerStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstServiceHistoricalRecoveryIoWorker(node)

IndexedResolveLocalProducerContinuationStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!
       PostGstResolveLocalCandidateProducerContinuation(node)

IndexedServiceConditionalProducerContinuationStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!
       PostGstServiceConditionalTransportProducerContinuation(node)

IndexedServiceVolatileProducerContinuationStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!
       PostGstServiceVolatileBodyProducerContinuation(node)

IndexedRetireLeaderWireLifecycleStep(initialContext, slot) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstRetireLeaderWireLifecycleSlot(slot)

IndexedAdmitPacketStep(initialContext, recipient, source) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstAdmitHiddenPacket(recipient, source)

IndexedAdmitHistoricalRecoveryPacketStep(
    initialContext, recipient, source) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstAdmitHistoricalRecoveryPacket(recipient, source)

IndexedFairness ==
  \A initialContext \in AdmissibleContextRecords:
    /\ WF_IndexedChainVars(IndexedSetGstStep(initialContext))
    /\ WF_IndexedChainVars(IndexedTickStep(initialContext))
    /\ \A node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext):
         WF_IndexedChainVars(
           IndexedRunNodeStep(initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedOpenHistoricalRecoveryStep(initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedRunHistoricalRecoveryStep(initialContext, node))
    /\ \A node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext):
         WF_IndexedChainVars(
           IndexedCommitCertificateDiscoveryStep(
             initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedHistoricalCommitCertificateDiscoveryStep(
             initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedHistoricalServerStep(initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedIoWorkerStep(initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedHistoricalRecoveryIoWorkerStep(
             initialContext, node))
    /\ \A node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext):
         /\ WF_IndexedChainVars(
              IndexedResolveLocalProducerContinuationStep(
                initialContext, node))
         /\ WF_IndexedChainVars(
              IndexedServiceConditionalProducerContinuationStep(
                initialContext, node))
         /\ WF_IndexedChainVars(
              IndexedServiceVolatileProducerContinuationStep(
                initialContext, node))
    /\ \A slot \in IndexedAsync(initialContext)!
                   AsyncLeaderWireLifecycleSlotSet:
         WF_IndexedChainVars(
           IndexedRetireLeaderWireLifecycleStep(initialContext, slot))
    /\ \A recipient \in Responsive,
          source \in IndexedAsync(initialContext)!
                     AsyncIngressSources:
         WF_IndexedChainVars(
           IndexedAdmitPacketStep(initialContext, recipient, source))
    /\ \A recipient \in ValidatorIds,
          source \in IndexedAsync(initialContext)!AsyncIngressSources:
         WF_IndexedChainVars(
           IndexedAdmitHistoricalRecoveryPacketStep(
             initialContext, recipient, source))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedSuccessorActivationProgressStep(
             initialContext, node))

IndexedChainSpec ==
  /\ IndexedChainInit
  /\ [][IndexedChainNext]_IndexedChainVars
  /\ IndexedFairness
  /\ EventualFailureFreeSuccessorStartupSuffix

(***************************************************************************
GST is an environmental liveness condition, never a consequence of a finite
process-generation budget.  The live transition surface stays identical to
the safety spec, while the live-only wrapper records the stated representative
deployment boundary.  Safety and bounded TLC configurations remain valid below
four peers because IndexedChainSpec itself is unchanged.  Aggregate height
induction receives the condition below as an explicit release premise;
per-context theorems expose its exact instance.
***************************************************************************)
IndexedLiveChainSpec ==
  /\ AsyncRepresentativeLiveConfiguration
  /\ IndexedChainSpec

IndexedGstEventuallyCondition ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!AsyncLiveSpecAt(initialContext)
      => <>IndexedCore(initialContext, 7)

THEOREM IndexedLiveChainSpecProjectsIndexedChainSpec ==
  IndexedLiveChainSpec => IndexedChainSpec
BY DEF IndexedLiveChainSpec

(***************************************************************************
`MaxHeight` is only the finite verification projection. A fresh exact Async
application receipt at that boundary is handed to `RecordKnownApplication`,
so the bounded successor projection stutters while preserving node
height/context and the activation tuple. Production has no terminal height,
kernel, or trace claim corresponding to this model-checking horizon.
***************************************************************************)
THEOREM TerminalContextCannotQueueSuccessorActivation ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    terminalContext.height = MaxHeight
      => ~QueueSuccessorActivation(terminalContext, node)
BY DEF QueueSuccessorActivation

THEOREM TerminalExactApplicationPreservesHorizon ==
  \A terminalContext \in AdmissibleContextRecords,
     application \in Chain!DecisionEvidenceSet:
    terminalContext.height = MaxHeight
      /\ IndexedApplicationReceiptHandoff(terminalContext, application)
      => /\ nodeHeight'[application.node] = terminalContext.height
         /\ nodeContext'[application.node] = terminalContext
         /\ UNCHANGED SuccessorActivationVars
BY Isa DEF IndexedApplicationReceiptHandoff,
           ExactNodeLocationAt, QueueSuccessorActivation,
           Chain!RecordAppliedNext, Chain!RecordKnownApplication,
           SuccessorActivationVars

THEOREM IndexedInitEstablishesTerminalActivationExclusion ==
  IndexedChainInit => FiniteHorizonSuccessorProjectionDormant
BY Isa DEF IndexedChainInit, FiniteHorizonSuccessorProjectionDormant

THEOREM IndexedActionPreservesTerminalActivationExclusion ==
  FiniteHorizonSuccessorProjectionDormant
    /\ IndexedChainNext
    => FiniteHorizonSuccessorProjectionDormant'
BY Isa DEF FiniteHorizonSuccessorProjectionDormant,
           IndexedChainNext, IndexedProductActionAt,
           IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           QueueSuccessorActivation,
           IndexedSuccessorActivationProgressStep,
           BeginSuccessorActivation,
           BindAppliedSuccessorActivationToken,
           LatchAppliedSuccessorStartupFailure,
           LatchRecoveredSuccessorStartupFailure,
           RehydrateCleanCompleteTipSuccessorStartup,
           RehydrateFailedSuccessorStartup,
           AuthenticateRecoveredSuccessorActivation,
           OpenDeferredSuccessorAdapter,
           ConstructSuccessorRuntime,
           StartSuccessorServices,
           ApplySuccessorStartupEffects,
           ArmSuccessorClocks,
           PrepareSuccessorActivationMarker,
           OpenSuccessorIngress,
           ActivateAppliedSuccessorHeight,
           ActivateRecoveredSuccessorHeight

THEOREM IndexedStepPreservesTerminalActivationExclusion ==
  FiniteHorizonSuccessorProjectionDormant
    /\ [IndexedChainNext]_IndexedChainVars
    => FiniteHorizonSuccessorProjectionDormant'
BY Isa, IndexedActionPreservesTerminalActivationExclusion
   DEF IndexedChainVars, FiniteHorizonSuccessorProjectionDormant

THEOREM IndexedChainAlwaysExcludesTerminalActivation ==
  IndexedChainSpec => []FiniteHorizonSuccessorProjectionDormant
PROOF
  <1>1. IndexedChainInit => FiniteHorizonSuccessorProjectionDormant
    BY IndexedInitEstablishesTerminalActivationExclusion
  <1>2. FiniteHorizonSuccessorProjectionDormant
           /\ [IndexedChainNext]_IndexedChainVars
           => FiniteHorizonSuccessorProjectionDormant'
    BY IndexedStepPreservesTerminalActivationExclusion
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

(***************************************************************************
Composition invariant.

Every joined context is a canonical prefix no higher than the globally
certified prefix. Application advances the durable per-node chain first, then
queues a context-exact activation. A node joins the successor only after the
Applied or Recovered publication action records the exact activation marker.
Terminal historical recovery writes the exact instance application evidence
without a fictitious successor or node-height advance.
***************************************************************************)
IndexedEveryInstanceStrongInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!StrongInductiveInvariant

\* The scheduler-side service-activation pair is part of AsyncStrongType,
\* not the Core-only StrongInductiveInvariant above.  Retain the complete
\* typed Async boundary so component 46 and its paired deadlines can be
\* preserved through both ordinary product steps and final join actions.
IndexedEveryInstanceAsyncStrongTypeInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!AsyncStrongTypeInvariant

JoinedContextCertificationInvariant ==
  \A initialContext \in JoinedContexts:
    /\ initialContext =
         Chain!ContextRecord(initialContext.height,
                             Chain!HistoryThrough(initialContext.height))
    /\ initialContext.height <= certifiedHeight

IndexedJoinedThroughLocalHeight ==
  \A node \in ValidatorIds, blockHeight \in Heights:
    blockHeight <= nodeHeight[node]
      => /\ CanonicalIndexedContext(blockHeight)
               \in AdmissibleContextRecords
         /\ \/ node \in joinedByContext[
                        CanonicalIndexedContext(blockHeight)]
            \/ /\ blockHeight = nodeHeight[node]
               /\ blockHeight > 0
               /\ LET parentContext ==
                         CanonicalIndexedContext(blockHeight - 1)
                  IN /\ successorActivationStatus[parentContext][node]
                           \in {"Queued", "Running"}
                     /\ \E application \in Chain!DecisionEvidenceSet:
                          ExactDurableParentApplication(
                            parentContext, node, application)

JoinedRoutingInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in joinedByContext[initialContext]:
      \/ IndexedNodeCurrentAt(initialContext, node)
      \/ /\ nodeHeight[node] > initialContext.height
         /\ IndexedAsync(initialContext)!NodeHasApplication(node)

IndexedApplicationsRespectNodeHeight ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in Responsive:
      IndexedAsync(initialContext)!NodeHasApplication(node)
        => \/ initialContext.height = MaxHeight
           \/ nodeHeight[node] > initialContext.height

IndexedHistoricalRecoveryTargetCoherence ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
      => /\ node \in Responsive
         /\ node \in joinedByContext[initialContext]
         /\ ExactNodeLocationAt(initialContext, node)
         /\ ~IndexedAsync(initialContext)!NodeHasApplication(node)

(***************************************************************************
Service activation is the exact scheduler-side mirror of independent joined
membership.  Dormant pre-created instances and genesis retain the canonical
unrestricted initializer.  The first non-genesis join burns the irreversible
restriction tombstone; thereafter the active set equals joined membership and
can grow only in the same atomic publication action.  The paired deadline
invariant prevents a zeroed inactive owner from entering any timed blocker.
***************************************************************************)
IndexedServiceActivationMembershipCoherenceAt(initialContext) ==
  IF IndexedAsync(initialContext)!AsyncServiceActivationRestricted
  THEN /\ joinedByContext[initialContext] # {}
       /\ IndexedAsync(initialContext)!AsyncActiveServiceNodes
            = joinedByContext[initialContext]
  ELSE /\ IndexedAsync(initialContext)!AsyncActiveServiceNodes
               = ValidatorIds
       /\ IF initialContext = GenesisContext
          THEN joinedByContext[initialContext] = ValidatorIds
          ELSE /\ joinedByContext[initialContext] = {}
               /\ IndexedAsync(initialContext)!
                    AsyncServiceActivationClockPristine

IndexedServiceActivationCoherence ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedAsync(initialContext)!AsyncServiceActivationPairInvariant
    /\ IndexedServiceActivationMembershipCoherenceAt(initialContext)

(***************************************************************************
GST cannot become true in a dormant pre-created instance.  AsyncSetGST is an
ordinary product action and IndexedChainNext selects such actions only from
JoinedContexts; successor activation may join a context but frames GST.  This
one-way coherence is the product-local enabledness bridge used by historical
post-GST fairness and does not require all responsive peers to have joined.
***************************************************************************)
IndexedPostGstContextJoinedCoherence ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCore(initialContext, 7)
      => initialContext \in JoinedContexts

\* `AsyncSetGST` requires the complete Responsive service roster to be active.
\* The restriction tombstone can only be installed while `~gst`, and every
\* later activation grows the active set.  Retain that exact executable guard
\* as reachable-state evidence instead of treating GST as enabled by one
\* joined owner.
IndexedPostGstResponsiveActiveRosterCoherence ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCore(initialContext, 7)
      => Responsive \subseteq
           IndexedAsync(initialContext)!AsyncActiveServiceNodes

IndexedTerminalExactApplicationBoundaryInvariant ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in Responsive:
    terminalContext.height = MaxHeight
      /\ IndexedAsync(terminalContext)!NodeHasApplication(node)
      => ExactNodeLocationAt(terminalContext, node)

IndexedCompositionInvariant ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ SuccessorActivationShape
  /\ Chain!ChainEpochInvariant
  /\ IndexedTotalReceiptProjection
  /\ IndexedEveryInstanceStrongInvariant
  /\ IndexedEveryInstanceAsyncStrongTypeInvariant
  /\ JoinedContextCertificationInvariant
  /\ JoinedRoutingInvariant
  /\ IndexedApplicationsRespectNodeHeight
  /\ IndexedHistoricalRecoveryTargetCoherence
  /\ IndexedServiceActivationCoherence
  /\ IndexedPostGstContextJoinedCoherence
  /\ IndexedPostGstResponsiveActiveRosterCoherence
  /\ IndexedTerminalExactApplicationBoundaryInvariant

(***************************************************************************
Non-temporal composition and refinement kernels.
***************************************************************************)
THEOREM AdmissibleContextDomainIsFinite ==
  ModelConfiguration => IsFiniteSet(AdmissibleContextRecords)
BY Isa DEF AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, LineagesAt, Heights, Subjects

THEOREM IndexedInstanceVariablesAreExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncAllVars =
           IndexedAsyncStateAt(initialContext)
BY Isa DEF IndexedAsyncStateShape, IndexedAsyncStateAt,
           IndexedAsync!AsyncAllVars,
           IndexedAsync!AsyncSchedulerVars,
           IndexedAsync!AsyncRecoveryVars,
           IndexedAsync!AsyncProducerVars,
           IndexedAsync!vars,
           IndexedDuplicatedGst, IndexedCore, IndexedScheduler,
           IndexedRecovery, IndexedProducer,
           IndexedFixedCorridorDeadlines,
           IndexedServeProducerTurnDue

(***************************************************************************
Exact indexed field-order pins.

Arity alone is insufficient at this boundary: an insertion in Core or the
scheduler can leave every tuple well typed while shifting a later durable or
fairness owner onto the wrong state component.  These extensional equalities
pin the duplicated GST scalar, all 49 Core fields, all 46 scheduler fields,
the five recovery fields, all three producer-journal fields, and the proof-only
fixed-corridor receipt, and the per-node post-Serve producer debt.
***************************************************************************)
THEOREM IndexedDuplicatedGstProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         /\ IndexedDuplicatedGst(initialContext)
              = IndexedCore(initialContext, 7)
         /\ IndexedAsync(initialContext)!AsyncAllVars[1]
              = IndexedDuplicatedGst(initialContext)
BY DEF IndexedAsyncStateShape,
       IndexedAsync!AsyncAllVars,
       IndexedDuplicatedGst, IndexedCore

THEOREM IndexedFortyNineFieldCoreProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!vars =
           <<IndexedCore(initialContext, 1),
             IndexedCore(initialContext, 2),
             IndexedCore(initialContext, 3),
             IndexedCore(initialContext, 4),
             IndexedCore(initialContext, 5),
             IndexedCore(initialContext, 6),
             IndexedCore(initialContext, 7),
             IndexedCore(initialContext, 8),
             IndexedCore(initialContext, 9),
             IndexedCore(initialContext, 10),
             IndexedCore(initialContext, 11),
             IndexedCore(initialContext, 12),
             IndexedCore(initialContext, 13),
             IndexedCore(initialContext, 14),
             IndexedCore(initialContext, 15),
             IndexedCore(initialContext, 16),
             IndexedCore(initialContext, 17),
             IndexedCore(initialContext, 18),
             IndexedCore(initialContext, 19),
             IndexedCore(initialContext, 20),
             IndexedCore(initialContext, 21),
             IndexedCore(initialContext, 22),
             IndexedCore(initialContext, 23),
             IndexedCore(initialContext, 24),
             IndexedCore(initialContext, 25),
             IndexedCore(initialContext, 26),
             IndexedCore(initialContext, 27),
             IndexedCore(initialContext, 28),
             IndexedCore(initialContext, 29),
             IndexedCore(initialContext, 30),
             IndexedCore(initialContext, 31),
             IndexedCore(initialContext, 32),
             IndexedCore(initialContext, 33),
             IndexedCore(initialContext, 34),
             IndexedCore(initialContext, 35),
             IndexedCore(initialContext, 36),
             IndexedCore(initialContext, 37),
             IndexedCore(initialContext, 38),
             IndexedCore(initialContext, 39),
             IndexedCore(initialContext, 40),
             IndexedCore(initialContext, 41),
             IndexedCore(initialContext, 42),
             IndexedCore(initialContext, 43),
             IndexedCore(initialContext, 44),
             IndexedCore(initialContext, 45),
             IndexedCore(initialContext, 46),
             IndexedCore(initialContext, 47),
             IndexedCore(initialContext, 48),
             IndexedCore(initialContext, 49)>>
BY DEF IndexedAsync!vars, IndexedCore

THEOREM IndexedFortySixFieldSchedulerProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncSchedulerVars =
           <<IndexedScheduler(initialContext, 1),
             IndexedScheduler(initialContext, 2),
             IndexedScheduler(initialContext, 3),
             IndexedScheduler(initialContext, 4),
             IndexedScheduler(initialContext, 5),
             IndexedScheduler(initialContext, 6),
             IndexedScheduler(initialContext, 7),
             IndexedScheduler(initialContext, 8),
             IndexedScheduler(initialContext, 9),
             IndexedScheduler(initialContext, 10),
             IndexedScheduler(initialContext, 11),
             IndexedScheduler(initialContext, 12),
             IndexedScheduler(initialContext, 13),
             IndexedScheduler(initialContext, 14),
             IndexedScheduler(initialContext, 15),
             IndexedScheduler(initialContext, 16),
             IndexedScheduler(initialContext, 17),
             IndexedScheduler(initialContext, 18),
             IndexedScheduler(initialContext, 19),
             IndexedScheduler(initialContext, 20),
             IndexedScheduler(initialContext, 21),
             IndexedScheduler(initialContext, 22),
             IndexedScheduler(initialContext, 23),
             IndexedScheduler(initialContext, 24),
             IndexedScheduler(initialContext, 25),
             IndexedScheduler(initialContext, 26),
             IndexedScheduler(initialContext, 27),
             IndexedScheduler(initialContext, 28),
             IndexedScheduler(initialContext, 29),
             IndexedScheduler(initialContext, 30),
             IndexedScheduler(initialContext, 31),
             IndexedScheduler(initialContext, 32),
             IndexedScheduler(initialContext, 33),
             IndexedScheduler(initialContext, 34),
             IndexedScheduler(initialContext, 35),
             IndexedScheduler(initialContext, 36),
             IndexedScheduler(initialContext, 37),
             IndexedScheduler(initialContext, 38),
             IndexedScheduler(initialContext, 39),
             IndexedScheduler(initialContext, 40),
             IndexedScheduler(initialContext, 41),
             IndexedScheduler(initialContext, 42),
             IndexedScheduler(initialContext, 43),
             IndexedScheduler(initialContext, 44),
             IndexedScheduler(initialContext, 45),
             IndexedScheduler(initialContext, 46)>>
BY DEF IndexedAsync!AsyncSchedulerVars, IndexedScheduler

=============================================================================
