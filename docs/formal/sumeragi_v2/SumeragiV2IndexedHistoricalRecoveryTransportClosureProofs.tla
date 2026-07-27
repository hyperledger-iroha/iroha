---- MODULE SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs ----
EXTENDS SumeragiV2ChainEpochRefinement

(***************************************************************************
Indexed exact historical-recovery archive route.

The one-height transport leaf deliberately does not identify its arbitrary
applied archive with a Commit-certificate fanout recipient.  The indexed
wrapper has the missing identity: `IndexedHistoricalRecoverySourceReady`
chooses one exact current responsive voter whose exact current application is
also durable.  This module projects the one-height transport vocabulary onto
that same indexed Async state and retains the chosen route as a safety
witness.

Only the route-availability invariant is discharged.  No aggregate indexed
fairness premise, eventual service claim, or all-responsive-joined premise is
introduced here.
***************************************************************************)

IndexedHistoricalTransport(initialContext) ==
  INSTANCE SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs
    WITH
       height <- IndexedCore(initialContext, 1),
       context <- IndexedCore(initialContext, 2),
       contextHistory <- IndexedCore(initialContext, 3),
       nodeView <- IndexedCore(initialContext, 4),
       generation <- IndexedCore(initialContext, 5),
       up <- IndexedCore(initialContext, 6),
       gst <- IndexedCore(initialContext, 7),
       availableBodies <- IndexedCore(initialContext, 8),
       durableBodies <- IndexedCore(initialContext, 9),
       retainedLockedBodies <- IndexedCore(initialContext, 10),
       validatedBodies <- IndexedCore(initialContext, 11),
       invalidBodies <- IndexedCore(initialContext, 12),
       seenProposals <- IndexedCore(initialContext, 13),
       receivedVotes <- IndexedCore(initialContext, 14),
       receivedQCs <- IndexedCore(initialContext, 15),
       receivedTimeoutVotes <- IndexedCore(initialContext, 16),
       receivedTCs <- IndexedCore(initialContext, 17),
       proposalIntents <- IndexedCore(initialContext, 18),
       prepareIntents <- IndexedCore(initialContext, 19),
       commitIntents <- IndexedCore(initialContext, 20),
       timeoutIntents <- IndexedCore(initialContext, 21),
       prepareQCs <- IndexedCore(initialContext, 22),
       commitQCs <- IndexedCore(initialContext, 23),
       formedTCs <- IndexedCore(initialContext, 24),
       installedTCs <- IndexedCore(initialContext, 25),
       lockRank <- IndexedCore(initialContext, 26),
       lockSubject <- IndexedCore(initialContext, 27),
       highestRank <- IndexedCore(initialContext, 28),
       highestSubject <- IndexedCore(initialContext, 29),
       pendingProposal <- IndexedCore(initialContext, 30),
       pendingPrepare <- IndexedCore(initialContext, 31),
       pendingObservePrepare <- IndexedCore(initialContext, 32),
       pendingLockCommit <- IndexedCore(initialContext, 33),
       pendingTimeout <- IndexedCore(initialContext, 34),
       pendingInstallTC <- IndexedCore(initialContext, 35),
       pendingDecision <- IndexedCore(initialContext, 36),
       signProposals <- IndexedCore(initialContext, 37),
       signVotes <- IndexedCore(initialContext, 38),
       signTimeouts <- IndexedCore(initialContext, 39),
       proposalNetwork <- IndexedCore(initialContext, 40),
       voteNetwork <- IndexedCore(initialContext, 41),
       qcNetwork <- IndexedCore(initialContext, 42),
       timeoutNetwork <- IndexedCore(initialContext, 43),
       tcNetwork <- IndexedCore(initialContext, 44),
       decisions <- IndexedCore(initialContext, 45),
       applied <- IndexedCore(initialContext, 46),
       asyncNow <- IndexedScheduler(initialContext, 1),
       asyncCommandQueues <- IndexedScheduler(initialContext, 2),
       asyncNextCommandClass <- IndexedScheduler(initialContext, 3),
       asyncFifoOwed <- IndexedScheduler(initialContext, 4),
       asyncTimeoutEmitted <- IndexedScheduler(initialContext, 5),
       asyncRunnerPhase <- IndexedScheduler(initialContext, 6),
       asyncRunnerBudget <- IndexedScheduler(initialContext, 7),
       asyncCausalAdmissionOwed <- IndexedScheduler(initialContext, 8),
       asyncNextLocalSource <- IndexedScheduler(initialContext, 9),
       asyncIoQueues <- IndexedScheduler(initialContext, 10),
       asyncOutstandingWork <- IndexedScheduler(initialContext, 11),
       asyncIoReadyCompletions <- IndexedScheduler(initialContext, 12),
       asyncLocalReadyCompletions <- IndexedScheduler(initialContext, 13),
       asyncNextCompletionSource <- IndexedScheduler(initialContext, 14),
       asyncIoControlAvailable <- IndexedScheduler(initialContext, 15),
       asyncDeferredCompletionQueues <- IndexedScheduler(initialContext, 16),
       asyncDeferredProgressQueues <- IndexedScheduler(initialContext, 17),
       asyncDeferredNormalQueues <- IndexedScheduler(initialContext, 18),
       asyncDeferredHandoffs <- IndexedScheduler(initialContext, 19),
       asyncNextDeferredClass <- IndexedScheduler(initialContext, 20),
       asyncDeferredDrainOwed <- IndexedScheduler(initialContext, 21),
       asyncCausalQueues <- IndexedScheduler(initialContext, 22),
       asyncOutstandingTags <- IndexedScheduler(initialContext, 23),
       asyncNodeDeadlines <- IndexedScheduler(initialContext, 24),
       asyncRetransmitDeadlines <- IndexedScheduler(initialContext, 25),
       asyncNodeServiceDeadlines <- IndexedScheduler(initialContext, 26),
       asyncIoServiceDeadlines <- IndexedScheduler(initialContext, 27),
       asyncSentItems <- IndexedScheduler(initialContext, 28),
       asyncRetainedControl <- IndexedScheduler(initialContext, 29),
       asyncActiveRequests <- IndexedScheduler(initialContext, 30),
       asyncCertifiedResponseClaim <- IndexedScheduler(initialContext, 31),
       asyncTransport <- IndexedScheduler(initialContext, 32),
       asyncIngressLanes <- IndexedScheduler(initialContext, 33),
       asyncIngressReady <- IndexedScheduler(initialContext, 34),
       asyncHeldChunks <- IndexedScheduler(initialContext, 35),
       asyncHistoricalRecoveryTargets <- IndexedScheduler(initialContext, 36),
       asyncControlServiceState <- IndexedScheduler(initialContext, 37),
       asyncRecoveryPhase <- IndexedRecovery(initialContext, 1),
       asyncRecoveryNode <- IndexedRecovery(initialContext, 2),
       asyncRecoveryGeneration <- IndexedRecovery(initialContext, 3),
       asyncRecoveryReplayQueue <- IndexedRecovery(initialContext, 4),
       asyncHistoricalLockRestartAuthorities <-
         IndexedRecovery(initialContext, 5)

(***************************************************************************
Exact projection.

These are the same 46 Core, 37 scheduler, and five recovery substitutions as
`IndexedAsync` and `IndexedDecisionWitness`.  The extensional equality is what
permits the product bracket to feed the transport instance without defining a
second state machine.
***************************************************************************)

THEOREM IndexedHistoricalTransportVariablesAreExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedHistoricalTransport(initialContext)!AsyncAllVars =
           IndexedAsyncStateAt(initialContext)
BY Isa
   DEF IndexedAsyncStateShape, IndexedAsyncStateAt,
       IndexedHistoricalTransport!AsyncAllVars,
       IndexedHistoricalTransport!AsyncSchedulerVars,
       IndexedHistoricalTransport!AsyncRecoveryVars,
       IndexedHistoricalTransport!vars,
       IndexedCore, IndexedScheduler, IndexedRecovery

THEOREM IndexedInitProjectsEveryHistoricalTransportInit ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainInit
      => IndexedHistoricalTransport(initialContext)!
           AsyncInitAt(initialContext)
BY IndexedInitProjectsEveryAsyncInit
   DEF IndexedHistoricalTransport!AsyncInitAt,
       IndexedAsync!AsyncInitAt

THEOREM IndexedStepProjectsEveryHistoricalTransportStep ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainNext
      => [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
           IndexedHistoricalTransport(initialContext)!AsyncAllVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                IndexedChainNext
         PROVE [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
                 IndexedHistoricalTransport(initialContext)!AsyncAllVars)
    <2>1. IndexedAsyncStateShape
      BY <1>1 DEF IndexedChainNext
    <2>2. IndexedHistoricalTransport(initialContext)!AsyncAllVars =
             IndexedAsyncStateAt(initialContext)
      BY <1>1, <2>1, IndexedHistoricalTransportVariablesAreExact
    <2>3. [IndexedAsync(initialContext)!AsyncNext]_(
             IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedStepProjectsEveryAsyncStep
    <2> QED BY <2>2, <2>3, Isa
         DEF IndexedHistoricalTransport!AsyncNext,
             IndexedAsync!AsyncNext
  <1> QED BY <1>1

THEOREM IndexedBracketStepProjectsEveryHistoricalTransportStep ==
  \A initialContext \in AdmissibleContextRecords:
    [IndexedChainNext]_IndexedChainVars
      => [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
           IndexedHistoricalTransport(initialContext)!AsyncAllVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                [IndexedChainNext]_IndexedChainVars
         PROVE [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
                 IndexedHistoricalTransport(initialContext)!AsyncAllVars)
    <2>1. CASE IndexedChainNext
      BY <1>1, <2>1,
         IndexedStepProjectsEveryHistoricalTransportStep
    <2>2. CASE UNCHANGED IndexedChainVars
      <3>1. UNCHANGED indexedAsyncState
        BY <2>2 DEF IndexedChainVars
      <3>2. UNCHANGED
               (IndexedHistoricalTransport(initialContext)!AsyncAllVars)
        BY <3>1, Isa
           DEF IndexedHistoricalTransport!AsyncAllVars,
               IndexedHistoricalTransport!AsyncSchedulerVars,
               IndexedHistoricalTransport!AsyncRecoveryVars,
               IndexedHistoricalTransport!vars,
               IndexedCore, IndexedScheduler, IndexedRecovery
      <3> QED BY <3>2
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Historical scheduler support invariant.

The full exact-instance liveness projection waits for every responsive node
to join a context.  A historical target already carries the narrower joined
owner needed by its dedicated runner and I/O worker, so the product proof
uses only the two inductive scheduler invariants consumed by the historical
rank kernels.  This is a safety projection, not an added fairness premise.
The Candidate tombstone and Serve reservation/tombstone high-watermark are
included explicitly because the fixed-clock producer episode consumes those
finite namespaces before occurrence-rank descent; that episode itself is not
called progress.
***************************************************************************)

IndexedHistoricalTemporalSupportAt(initialContext) ==
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncStrongTypeInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncProgressOwnershipInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       Stage2BusyKernelInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncDeferredHandoffOwnershipInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalTemporalIdentityLifecycleInvariant

THEOREM IndexedInitEstablishesHistoricalTemporalSupport ==
  IndexedChainInit
    => \A initialContext \in AdmissibleContextRecords:
         IndexedHistoricalTemporalSupportAt(initialContext)
PROOF
  <1>1. ASSUME IndexedChainInit,
                NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalTemporalSupportAt(initialContext)
    <2>1. IndexedHistoricalTransport(initialContext)!
             AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryHistoricalTransportInit
    <2>2. IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesStrongTypeInvariant
    <2>3. IndexedHistoricalTransport(initialContext)!
             AsyncProgressOwnershipInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesProgressOwnership
    <2>4. IndexedHistoricalTransport(initialContext)!
             Stage2BusyKernelInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           Stage2BusyKernelInitObligation
    <2>5. IndexedHistoricalTransport(initialContext)!
             AsyncDeferredHandoffOwnershipInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesDeferredHandoffOwnership
    <2>6. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalIdentityLifecycleInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalInitEstablishesIdentityLifecycle
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6
         DEF IndexedHistoricalTemporalSupportAt
  <1> QED BY <1>1

THEOREM IndexedBracketStepPreservesHistoricalTemporalSupport ==
  /\ \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalTemporalSupportAt(initialContext)
  /\ [IndexedChainNext]_IndexedChainVars
  => \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalTemporalSupportAt(initialContext)'
PROOF
  <1>1. ASSUME
          \A initialContext \in AdmissibleContextRecords:
            IndexedHistoricalTemporalSupportAt(initialContext),
          [IndexedChainNext]_IndexedChainVars
         PROVE
          \A initialContext \in AdmissibleContextRecords:
            IndexedHistoricalTemporalSupportAt(initialContext)'
    <2>1. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTemporalSupportAt(initialContext)'
      <3>1. IndexedHistoricalTemporalSupportAt(initialContext)
        BY <1>1, <2>1
      <3>2. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
               IndexedHistoricalTransport(initialContext)!AsyncAllVars)
        BY <1>1, <2>1,
           IndexedBracketStepProjectsEveryHistoricalTransportStep
      <3>3. IndexedHistoricalTransport(initialContext)!
               AsyncStrongTypeInvariant'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncBracketNextPreservesStrongTypeInvariant
           DEF IndexedHistoricalTemporalSupportAt
      <3>4. IndexedHistoricalTransport(initialContext)!
               AsyncProgressOwnershipInvariant'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncBracketNextPreservesProgressOwnership
      <3>5. IndexedHistoricalTransport(initialContext)!
               Stage2BusyKernelInvariant'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             Stage2BusyKernelNextObligation
           DEF IndexedHistoricalTemporalSupportAt
      <3>6. IndexedHistoricalTransport(initialContext)!
               AsyncDeferredHandoffOwnershipInvariant'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             Stage2AsyncNextPreservesDeferredHandoffOwnership
           DEF IndexedHistoricalTemporalSupportAt
      <3>7. (IndexedHistoricalTransport(initialContext)!
               HistoricalTemporalIdentityLifecycleInvariant)'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalBracketPreservesIdentityLifecycle
           DEF IndexedHistoricalTemporalSupportAt
      <3> QED BY <3>3, <3>4, <3>5, <3>6, <3>7
           DEF IndexedHistoricalTemporalSupportAt
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM IndexedChainSpecAlwaysHistoricalTemporalSupport ==
  IndexedChainSpec
    => [](\A initialContext \in AdmissibleContextRecords:
           IndexedHistoricalTemporalSupportAt(initialContext))
PROOF
  <1>1. IndexedChainInit
           => \A initialContext \in AdmissibleContextRecords:
                IndexedHistoricalTemporalSupportAt(initialContext)
    BY IndexedInitEstablishesHistoricalTemporalSupport
  <1>2. /\ \A initialContext \in AdmissibleContextRecords:
              IndexedHistoricalTemporalSupportAt(initialContext)
         /\ [IndexedChainNext]_IndexedChainVars
         => \A initialContext \in AdmissibleContextRecords:
              IndexedHistoricalTemporalSupportAt(initialContext)'
    BY IndexedBracketStepPreservesHistoricalTemporalSupport
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

(***************************************************************************
Indexed fixed-clock identity prerequisite.

The product safety projection now carries both logical namespaces consumed by
the fixed-clock producer episode.  This theorem does not close that temporal
episode: it proves only finite reservation/tombstone accounting, immutable
retry identity, monotone terminal coverage, and no resurrection.  The
separate leaf premise below must still prove that every finite non-descent
episode is exhausted before occurrence-rank descent is composed.
***************************************************************************)

IndexedHistoricalFixedClockIdentityBridgeProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty(
        IndexedChainSpec)

THEOREM IndexedChainSpecProvidesHistoricalFixedClockIdentityBridge ==
  IndexedChainSpec
    => IndexedHistoricalFixedClockIdentityBridgeProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE IndexedHistoricalFixedClockIdentityBridgeProperty
    <2>1. [](\A initialContext \in AdmissibleContextRecords:
               IndexedHistoricalTemporalSupportAt(initialContext))
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport
    <2>2. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty(
                     IndexedChainSpec)
      <3>1. []IndexedHistoricalTemporalSupportAt(initialContext)
        BY <2>1, <2>2
      <3> QED
        BY <1>1, <3>1,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateTombstoneSubsetIsBoundedByFrozenOwnerCarrier,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateServicedIdentityCannotReactivate,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateTerminalIdentityCannotReactivateAtGst,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateServiceRouteNeutralResponseRetryIsStable,
           IndexedHistoricalTransport(initialContext)!
             AsyncServeQueuedIdentityDepartureInstallsTombstone,
           IndexedHistoricalTransport(initialContext)!
             AsyncServeRetiredIdentityCannotRequeueAtGst,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalServeExactRetryKeepsAdmissionHighWatermark,
           FS_Union, FS_Subset, Isa, PTL
           DEF IndexedHistoricalTemporalSupportAt,
               IndexedHistoricalTransport!
                 HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty,
               IndexedHistoricalTransport!
                 HistoricalTemporalCandidateIdentityBudgetBridgeProperty,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeIdentityBudgetBridgeProperty,
               IndexedHistoricalTransport!
                 HistoricalTemporalCandidateServiceTombstonesInIdentityCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeReservationsInIdentityCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeTombstonesInIdentityCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeRollbackTombstonesInIdentityCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeRetiredRecordsInIdentityCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeExactRetryCoalescingAction,
               IndexedHistoricalTransport!
                 HistoricalTemporalIdentityLifecycleInvariant,
               IndexedHistoricalTransport!AsyncStrongTypeInvariant,
               IndexedHistoricalTransport!AsyncSchedulerTypeInvariant,
               IndexedHistoricalTransport!AsyncIoTypeInvariant,
               IndexedHistoricalTransport!AsyncProgressOwnershipInvariant,
               IndexedHistoricalTransport!
                 AsyncCandidateServiceTombstoneLifecycleInvariant,
               IndexedHistoricalTransport!AsyncServeLifecycleTypeInvariant,
               IndexedHistoricalTransport!
                 AsyncServeLifecyclePartitionInvariant,
               IndexedHistoricalTransport!
                 AsyncServeFamilyHighWatermarkInvariant,
               IndexedHistoricalTransport!
                 AsyncServeReservationOwnershipInvariant,
               IndexedHistoricalTransport!AsyncServeOrdinalInvariant,
               IndexedHistoricalTransport!AsyncAllVars
    <2> QED BY <2>2
         DEF IndexedHistoricalFixedClockIdentityBridgeProperty
  <1> QED BY <1>1

IndexedHistoricalFixedClockTemporalLeafProperties ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalTemporalFixedClockLeaves(IndexedChainSpec)

IndexedHistoricalFixedClockPrerequisiteSurface ==
  /\ IndexedHistoricalFixedClockIdentityBridgeProperty
  /\ \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryFixedClockTemporalPrerequisites(
           IndexedChainSpec)

THEOREM IndexedHistoricalFixedClockLeavesEstablishPrerequisiteSurface ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFixedClockTemporalLeafProperties
  => IndexedHistoricalFixedClockPrerequisiteSurface
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFixedClockTemporalLeafProperties
         PROVE IndexedHistoricalFixedClockPrerequisiteSurface
    <2>1. IndexedHistoricalFixedClockIdentityBridgeProperty
      BY <1>1,
         IndexedChainSpecProvidesHistoricalFixedClockIdentityBridge
    <2>2. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryFixedClockTemporalPrerequisites(
                     IndexedChainSpec)
      <3>1. IndexedHistoricalTransport(initialContext)!
               HistoricalTemporalFixedClockLeaves(IndexedChainSpec)
        BY <1>1, <2>2
           DEF IndexedHistoricalFixedClockTemporalLeafProperties
      <3> QED BY <3>1,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalFixedClockLeavesAreExact
    <2> QED BY <2>1, <2>2
         DEF IndexedHistoricalFixedClockPrerequisiteSurface
  <1> QED BY <1>1

(***************************************************************************
Reusable indexed historical-runner bridge.

Every historical protected candidate identifies its exact historical target.
Composition coherence places that target in the context's joined set, so the
one-height historical runner has a product extension.  The bridge consumes
the existing product weak-fairness occurrence; it does not activate the full
`AsyncSpecAt` instance or require unrelated responsive peers to join.
***************************************************************************)

IndexedHistoricalTemporalCandidateRunnerPending(
    initialContext, candidate) ==
  /\ IndexedHistoricalTemporalSupportAt(initialContext)
  /\ IndexedHistoricalTransport(initialContext)!gst
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedCandidateOwned(candidate)

THEOREM IndexedHistoricalCandidateRunnerHasJoinedFairOwner ==
  \A initialContext \in AdmissibleContextRecords, candidate:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalCandidateRunnerPending(
         initialContext, candidate)
    => /\ candidate.node \in Responsive
       /\ candidate.node \in joinedByContext[initialContext]
       /\ initialContext \in JoinedContexts
       /\ IndexedHistoricalTransport(initialContext)!
            HistoricalRecoveryTarget(candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate,
                IndexedCompositionInvariant,
                IndexedHistoricalTemporalCandidateRunnerPending(
                  initialContext, candidate)
         PROVE /\ candidate.node \in Responsive
               /\ candidate.node \in joinedByContext[initialContext]
               /\ initialContext \in JoinedContexts
               /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalRecoveryTarget(candidate.node)
    <2>1. /\ candidate.node \in Responsive
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTarget(candidate.node)
      BY <1>1
         DEF IndexedHistoricalTemporalCandidateRunnerPending,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned
    <2>2. IndexedAsync(initialContext)!
             HistoricalRecoveryTarget(candidate.node)
      BY <2>1, Isa
         DEF IndexedHistoricalTransport!HistoricalRecoveryTarget,
             IndexedAsync!HistoricalRecoveryTarget,
             IndexedScheduler
    <2>3. candidate.node \in joinedByContext[initialContext]
      BY <1>1, <2>2
         DEF IndexedCompositionInvariant,
             IndexedHistoricalRecoveryTargetCoherence
    <2>4. initialContext \in JoinedContexts
      BY <2>3 DEF JoinedContexts
    <2> QED BY <2>1, <2>3, <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalCandidateRunnerEnablesExactAction ==
  \A initialContext \in AdmissibleContextRecords, candidate:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalCandidateRunnerPending(
         initialContext, candidate)
    => ENABLED
         IndexedRunHistoricalRecoveryStep(
           initialContext, candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate,
                IndexedCompositionInvariant,
                IndexedHistoricalTemporalCandidateRunnerPending(
                  initialContext, candidate)
         PROVE ENABLED
                 IndexedRunHistoricalRecoveryStep(
                   initialContext, candidate.node)
    <2>1. /\ candidate.node \in Responsive
           /\ candidate.node \in joinedByContext[initialContext]
           /\ initialContext \in JoinedContexts
      BY <1>1, IndexedHistoricalCandidateRunnerHasJoinedFairOwner
    <2>2. ENABLED
             <<IndexedHistoricalTransport(initialContext)!
                 PostGstRunHistoricalRecoveryNode(
                   candidate.node)>>_(
               IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalProtectedOwnerEnablesFairRunner,
         ENABLEDaxioms
         DEF IndexedHistoricalTemporalCandidateRunnerPending,
             IndexedHistoricalTemporalSupportAt
    <2>3. ENABLED IndexedAsync(initialContext)!
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <2>2, Isa
         DEF IndexedHistoricalTransport!
               PostGstRunHistoricalRecoveryNode,
             IndexedAsync!PostGstRunHistoricalRecoveryNode,
             IndexedHistoricalTransport!
               RunHistoricalRecoveryNode,
             IndexedAsync!RunHistoricalRecoveryNode,
             IndexedHistoricalTransport!RunNodeWork,
             IndexedAsync!RunNodeWork
    <2>4. ENABLED
             IndexedRunHistoricalRecoveryStep(
               initialContext, candidate.node)
      BY <1>1, <2>1, <2>3,
         IndexedFairActionsRemainEnabledInProduct
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalCandidateRunnerIsNonstuttering ==
  \A initialContext \in AdmissibleContextRecords, candidate:
    /\ IndexedHistoricalTemporalCandidateRunnerPending(
         initialContext, candidate)
    /\ IndexedRunHistoricalRecoveryStep(
         initialContext, candidate.node)
    => <<IndexedRunHistoricalRecoveryStep(
           initialContext, candidate.node)>>_IndexedChainVars
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate,
                IndexedHistoricalTemporalCandidateRunnerPending(
                  initialContext, candidate),
                IndexedRunHistoricalRecoveryStep(
                  initialContext, candidate.node)
         PROVE <<IndexedRunHistoricalRecoveryStep(
                   initialContext, candidate.node)>>_IndexedChainVars
    <2>1. <<IndexedHistoricalTransport(initialContext)!
               PostGstRunHistoricalRecoveryNode(candidate.node)>>_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRunNodeIsNonstuttering, Isa
         DEF IndexedHistoricalTemporalCandidateRunnerPending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned,
             IndexedHistoricalTransport!HistoricalRecoveryTarget,
             IndexedRunHistoricalRecoveryStep,
             IndexedHistoricalTransport!
               PostGstRunHistoricalRecoveryNode,
             IndexedAsync!PostGstRunHistoricalRecoveryNode
    <2>2. IndexedAsyncStateShape
      BY <1>1
         DEF IndexedRunHistoricalRecoveryStep, IndexedChainNext
    <2>3. IndexedHistoricalTransport(initialContext)!AsyncAllVars =
             IndexedAsyncStateAt(initialContext)
      BY <1>1, <2>2, IndexedHistoricalTransportVariablesAreExact
    <2>4. IndexedChainVars' # IndexedChainVars
      BY <2>1, <2>3, Isa
         DEF IndexedChainVars, IndexedAsyncStateAt
    <2> QED BY <1>1, <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalCandidateRunnerEnablesFairOccurrence ==
  \A initialContext \in AdmissibleContextRecords, candidate:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalCandidateRunnerPending(
         initialContext, candidate)
    => ENABLED
         <<IndexedRunHistoricalRecoveryStep(
             initialContext, candidate.node)>>_IndexedChainVars
BY IndexedHistoricalCandidateRunnerEnablesExactAction,
   IndexedHistoricalCandidateRunnerIsNonstuttering,
   ENABLEDaxioms

IndexedHistoricalTemporalRankProgressExit(
    initialContext, candidate, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalRankProgressExit(candidate, rank)

(***************************************************************************
Indexed historical Stage-3 lift.

The product owns the same exact Runtime-prefix auxiliary rank as the
one-height proof.  The only fair consumer is the joined historical runner
bridged above.
***************************************************************************)

IndexedHistoricalTemporalStage3Pending(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage3Pending(candidate, position)

IndexedHistoricalTemporalStage3BlockedAtAux(
    initialContext, candidate, position, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage3AuxBlocked(
      candidate, position, rank)

IndexedHistoricalTemporalStage3AuxProgress(
    initialContext, candidate, position, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage3AuxProgress(
      candidate, position, rank)

THEOREM IndexedHistoricalStage3BlockedHasRunnerPending ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, rank:
    IndexedHistoricalTemporalStage3BlockedAtAux(
      initialContext, candidate, position, rank)
      => IndexedHistoricalTemporalCandidateRunnerPending(
           initialContext, candidate)
BY DEF IndexedHistoricalTemporalStage3BlockedAtAux,
       IndexedHistoricalTemporalCandidateRunnerPending,
       IndexedHistoricalTemporalSupportAt,
       IndexedHistoricalTransport!
         HistoricalTemporalStage3AuxBlocked,
       IndexedHistoricalTransport!
         HistoricalTemporalStage3Pending,
       IndexedHistoricalTransport!
         HistoricalProtectedOwnedAtServiceRank

THEOREM IndexedHistoricalStage3RunnerStrictlyProgresses ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A rank \in IndexedHistoricalTransport(initialContext)!
                   ReadyRunAuxCarrier:
      /\ IndexedHistoricalTemporalStage3BlockedAtAux(
           initialContext, candidate, position, rank)
      /\ <<IndexedRunHistoricalRecoveryStep(
             initialContext, candidate.node)>>_IndexedChainVars
      => IndexedHistoricalTemporalStage3AuxProgress(
           initialContext, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    ReadyRunAuxCarrier,
                IndexedHistoricalTemporalStage3BlockedAtAux(
                  initialContext, candidate, position, rank),
                <<IndexedRunHistoricalRecoveryStep(
                    initialContext, candidate.node)>>_IndexedChainVars
         PROVE IndexedHistoricalTemporalStage3AuxProgress(
                 initialContext, candidate, position, rank)'
    <2>1. IndexedHistoricalTransport(initialContext)!
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, Isa
         DEF IndexedRunHistoricalRecoveryStep,
             IndexedAsync!PostGstRunHistoricalRecoveryNode,
             IndexedHistoricalTransport!
               PostGstRunHistoricalRecoveryNode
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage3SameRunnerAuxDescent
         DEF IndexedHistoricalTemporalStage3BlockedAtAux,
             IndexedHistoricalTemporalStage3AuxProgress
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage3UnlessAuxProgress ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A rank \in IndexedHistoricalTransport(initialContext)!
                   ReadyRunAuxCarrier:
      /\ IndexedHistoricalTemporalStage3BlockedAtAux(
           initialContext, candidate, position, rank)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedHistoricalTemporalStage3BlockedAtAux(
              initialContext, candidate, position, rank)'
         \/ IndexedHistoricalTemporalStage3AuxProgress(
              initialContext, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    ReadyRunAuxCarrier,
                IndexedHistoricalTemporalStage3BlockedAtAux(
                  initialContext, candidate, position, rank),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalTemporalStage3BlockedAtAux(
                initialContext, candidate, position, rank)'
           \/ IndexedHistoricalTemporalStage3AuxProgress(
                initialContext, candidate, position, rank)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. CASE IndexedHistoricalTransport(initialContext)!
                  PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage3SameRunnerAuxDescent, Isa
         DEF IndexedHistoricalTemporalStage3BlockedAtAux,
             IndexedHistoricalTemporalStage3AuxProgress
    <2>3. CASE ~IndexedHistoricalTransport(initialContext)!
                   PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage3OtherStepUnlessAuxDescent
         DEF IndexedHistoricalTemporalStage3BlockedAtAux,
             IndexedHistoricalTemporalStage3AuxProgress
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalTemporalStage3Rank ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position \in Nat:
         IndexedHistoricalTemporalStage3Pending(
           initialContext, candidate, position)
           ~> IndexedHistoricalTemporalRankProgressExit(
                initialContext, candidate, <<3, position>>)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage3Pending(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<3, position>>)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>4. IndexedHistoricalTemporalStage3Pending(
             initialContext, candidate, position)
             ~> \E rank \in
                  IndexedHistoricalTransport(initialContext)!
                    ReadyRunAuxCarrier:
                  IndexedHistoricalTemporalStage3BlockedAtAux(
                    initialContext, candidate, position, rank)
      BY <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage3AuxRankInCarrier, PTL
         DEF IndexedHistoricalTemporalStage3Pending,
             IndexedHistoricalTemporalStage3BlockedAtAux,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3AuxBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3Pending,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3RankExit,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!ServiceRankLess
    <2>5. ASSUME NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    ReadyRunAuxCarrier
           PROVE IndexedHistoricalTemporalStage3BlockedAtAux(
                   initialContext, candidate, position, rank)
                   ~> IndexedHistoricalTemporalStage3AuxProgress(
                        initialContext, candidate, position, rank)
      <3>1. IndexedHistoricalTemporalStage3BlockedAtAux(
               initialContext, candidate, position, rank)
               /\ [IndexedChainNext]_IndexedChainVars
              => \/ IndexedHistoricalTemporalStage3BlockedAtAux(
                      initialContext, candidate, position, rank)'
                 \/ IndexedHistoricalTemporalStage3AuxProgress(
                      initialContext, candidate, position, rank)'
        BY <1>1, IndexedHistoricalStage3UnlessAuxProgress
      <3>2. IndexedCompositionInvariant
               /\ IndexedHistoricalTemporalStage3BlockedAtAux(
                    initialContext, candidate, position, rank)
              => ENABLED
                   <<IndexedRunHistoricalRecoveryStep(
                       initialContext, candidate.node)>>_IndexedChainVars
        BY <1>1, IndexedHistoricalStage3BlockedHasRunnerPending,
           IndexedHistoricalCandidateRunnerEnablesFairOccurrence
      <3>3. /\ IndexedHistoricalTemporalStage3BlockedAtAux(
                   initialContext, candidate, position, rank)
               /\ <<IndexedRunHistoricalRecoveryStep(
                      initialContext, candidate.node)>>_IndexedChainVars
              => IndexedHistoricalTemporalStage3AuxProgress(
                   initialContext, candidate, position, rank)'
        BY <1>1, IndexedHistoricalStage3RunnerStrictlyProgresses
      <3>4. CASE candidate.node \in Responsive
        <4>1. WF_IndexedChainVars(
                 IndexedRunHistoricalRecoveryStep(
                   initialContext, candidate.node))
          BY <1>1, <3>4 DEF IndexedChainSpec, IndexedFairness
        <4> QED BY <2>1, <2>2, <3>1, <3>2, <3>3, <4>1, PTL
      <3>5. CASE candidate.node \notin Responsive
        <4>1. []~IndexedHistoricalTemporalStage3BlockedAtAux(
                      initialContext, candidate, position, rank)
          BY <3>5, PTL
             DEF IndexedHistoricalTemporalStage3BlockedAtAux,
                 IndexedHistoricalTransport!
                   HistoricalTemporalStage3AuxBlocked,
                 IndexedHistoricalTransport!
                   HistoricalTemporalStage3Pending,
                 IndexedHistoricalTransport!
                   HistoricalProtectedOwnedAtServiceRank,
                 IndexedHistoricalTransport!
                   HistoricalProtectedCandidateOwned
        <4> QED BY <4>1, PTL
      <3> QED BY <3>4, <3>5
    <2>6. \A rank \in
               IndexedHistoricalTransport(initialContext)!
                 ReadyRunAuxCarrier:
             IndexedHistoricalTemporalStage3BlockedAtAux(
               initialContext, candidate, position, rank)
               ~> IndexedHistoricalTemporalRankProgressExit(
                    initialContext, candidate, <<3, position>>)
      BY <2>5,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF IndexedHistoricalTemporalStage3AuxProgress,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3AuxProgress,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3RankExit
    <2> QED BY <2>4, <2>6, PTL
  <1> QED BY <1>1

IndexedHistoricalTemporalStage3Source(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalProtectedOwnedAtServiceRank(
      candidate, <<3, position>>)

IndexedHistoricalTemporalStage3Goal(
    initialContext, candidate, position) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedServiceOwnershipExit(candidate)
  \/ \E lower \in SetLessThan(
       <<3, position>>,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankCarrier):
       IndexedHistoricalTransport(initialContext)!
         HistoricalProtectedOwnedAtServiceRank(candidate, lower)

IndexedHistoricalTemporalStage3LeafProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate \in IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
     position \in Nat:
    IndexedHistoricalTemporalStage3Source(
      initialContext, candidate, position)
      ~> IndexedHistoricalTemporalStage3Goal(
           initialContext, candidate, position)

THEOREM IndexedChainSpecClosesHistoricalTemporalStage3Leaf ==
  IndexedChainSpec
    => IndexedHistoricalTemporalStage3LeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage3Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage3Goal(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalStage3Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage3Pending(
                  initialContext, candidate, position)
      BY <2>1, PTL
         DEF IndexedHistoricalTemporalStage3Source,
             IndexedHistoricalTemporalStage3Pending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3Pending
    <2>3. IndexedHistoricalTemporalStage3Pending(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<3, position>>)
      BY <1>1,
         IndexedChainSpecClosesHistoricalTemporalStage3Rank
    <2>4. IndexedHistoricalTemporalSupportAt(initialContext)
             /\ IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<3, position>>)
             => IndexedHistoricalTemporalStage3Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRankExitHasWellFoundedSuccessor, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTemporalStage3Goal,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!OwnedServiceRankCarrier
    <2>5. IndexedHistoricalTemporalRankProgressExit(
             initialContext, candidate, <<3, position>>)
             ~> IndexedHistoricalTemporalStage3Goal(
                  initialContext, candidate, position)
      BY <2>1, <2>4, PTL
    <2> QED BY <2>2, <2>3, <2>5, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalTemporalStage3LeafProperty

(***************************************************************************
Indexed historical Stage-4 lift.

The one-height source has already combined actionable, causal-capacity, and
runner-prefix work into one well-founded episode rank.  The product lift
therefore needs only the same exact historical runner occurrence.
***************************************************************************)

IndexedHistoricalTemporalStage4Pending(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage4Pending(candidate, position)

IndexedHistoricalTemporalStage4BlockedAtRank(
    initialContext, candidate, position, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage4BlockedAtRank(
      candidate, position, rank)

IndexedHistoricalTemporalStage4Progress(
    initialContext, candidate, position, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage4Progress(
      candidate, position, rank)

THEOREM IndexedHistoricalStage4BlockedHasRunnerPending ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, rank:
    IndexedHistoricalTemporalStage4BlockedAtRank(
      initialContext, candidate, position, rank)
      => IndexedHistoricalTemporalCandidateRunnerPending(
           initialContext, candidate)
BY DEF IndexedHistoricalTemporalStage4BlockedAtRank,
       IndexedHistoricalTemporalCandidateRunnerPending,
       IndexedHistoricalTemporalSupportAt,
       IndexedHistoricalTransport!
         HistoricalTemporalStage4BlockedAtRank,
       IndexedHistoricalTransport!
         HistoricalTemporalStage4Pending,
       IndexedHistoricalTransport!
         HistoricalProtectedOwnedAtServiceRank

THEOREM IndexedHistoricalStage4RunnerStrictlyProgresses ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A rank \in IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage4EpisodeCarrier:
      /\ IndexedHistoricalTemporalStage4BlockedAtRank(
           initialContext, candidate, position, rank)
      /\ <<IndexedRunHistoricalRecoveryStep(
             initialContext, candidate.node)>>_IndexedChainVars
      => IndexedHistoricalTemporalStage4Progress(
           initialContext, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalTemporalStage4EpisodeCarrier,
                IndexedHistoricalTemporalStage4BlockedAtRank(
                  initialContext, candidate, position, rank),
                <<IndexedRunHistoricalRecoveryStep(
                    initialContext, candidate.node)>>_IndexedChainVars
         PROVE IndexedHistoricalTemporalStage4Progress(
                 initialContext, candidate, position, rank)'
    <2>1. IndexedHistoricalTransport(initialContext)!
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, Isa
         DEF IndexedRunHistoricalRecoveryStep,
             IndexedAsync!PostGstRunHistoricalRecoveryNode,
             IndexedHistoricalTransport!
               PostGstRunHistoricalRecoveryNode
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4SameRunnerStrictlyProgresses
         DEF IndexedHistoricalTemporalStage4BlockedAtRank,
             IndexedHistoricalTemporalStage4Progress
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage4UnlessProgress ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A rank \in IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage4EpisodeCarrier:
      /\ IndexedHistoricalTemporalStage4BlockedAtRank(
           initialContext, candidate, position, rank)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedHistoricalTemporalStage4BlockedAtRank(
              initialContext, candidate, position, rank)'
         \/ IndexedHistoricalTemporalStage4Progress(
              initialContext, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalTemporalStage4EpisodeCarrier,
                IndexedHistoricalTemporalStage4BlockedAtRank(
                  initialContext, candidate, position, rank),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalTemporalStage4BlockedAtRank(
                initialContext, candidate, position, rank)'
           \/ IndexedHistoricalTemporalStage4Progress(
                initialContext, candidate, position, rank)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. CASE IndexedHistoricalTransport(initialContext)!
                  PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4SameRunnerStrictlyProgresses, Isa
         DEF IndexedHistoricalTemporalStage4BlockedAtRank,
             IndexedHistoricalTemporalStage4Progress
    <2>3. CASE ~IndexedHistoricalTransport(initialContext)!
                   PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4OtherStepUnlessProgress
         DEF IndexedHistoricalTemporalStage4BlockedAtRank,
             IndexedHistoricalTemporalStage4Progress
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalTemporalStage4Rank ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position \in Nat:
         IndexedHistoricalTemporalStage4Pending(
           initialContext, candidate, position)
           ~> IndexedHistoricalTemporalRankProgressExit(
                initialContext, candidate, <<4, position>>)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage4Pending(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<4, position>>)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>4. IndexedHistoricalTemporalStage4Pending(
             initialContext, candidate, position)
             ~> \E rank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalTemporalStage4EpisodeCarrier:
                  IndexedHistoricalTemporalStage4BlockedAtRank(
                    initialContext, candidate, position, rank)
      BY <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4CarrierFacts, PTL
         DEF IndexedHistoricalTemporalStage4Pending,
             IndexedHistoricalTemporalStage4BlockedAtRank,
             IndexedHistoricalTransport!
               HistoricalTemporalStage4BlockedAtRank,
             IndexedHistoricalTransport!
               HistoricalTemporalStage4Pending,
             IndexedHistoricalTransport!
               HistoricalTemporalStage4EpisodeRank,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!ServiceRankLess
    <2>5. ASSUME NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalTemporalStage4EpisodeCarrier
           PROVE IndexedHistoricalTemporalStage4BlockedAtRank(
                   initialContext, candidate, position, rank)
                   ~> IndexedHistoricalTemporalStage4Progress(
                        initialContext, candidate, position, rank)
      <3>1. IndexedHistoricalTemporalStage4BlockedAtRank(
               initialContext, candidate, position, rank)
               /\ [IndexedChainNext]_IndexedChainVars
              => \/ IndexedHistoricalTemporalStage4BlockedAtRank(
                      initialContext, candidate, position, rank)'
                 \/ IndexedHistoricalTemporalStage4Progress(
                      initialContext, candidate, position, rank)'
        BY <1>1, IndexedHistoricalStage4UnlessProgress
      <3>2. IndexedCompositionInvariant
               /\ IndexedHistoricalTemporalStage4BlockedAtRank(
                    initialContext, candidate, position, rank)
              => ENABLED
                   <<IndexedRunHistoricalRecoveryStep(
                       initialContext, candidate.node)>>_IndexedChainVars
        BY <1>1, IndexedHistoricalStage4BlockedHasRunnerPending,
           IndexedHistoricalCandidateRunnerEnablesFairOccurrence
      <3>3. /\ IndexedHistoricalTemporalStage4BlockedAtRank(
                   initialContext, candidate, position, rank)
               /\ <<IndexedRunHistoricalRecoveryStep(
                      initialContext, candidate.node)>>_IndexedChainVars
              => IndexedHistoricalTemporalStage4Progress(
                   initialContext, candidate, position, rank)'
        BY <1>1, IndexedHistoricalStage4RunnerStrictlyProgresses
      <3>4. CASE candidate.node \in Responsive
        <4>1. WF_IndexedChainVars(
                 IndexedRunHistoricalRecoveryStep(
                   initialContext, candidate.node))
          BY <1>1, <3>4 DEF IndexedChainSpec, IndexedFairness
        <4> QED BY <2>1, <2>2, <3>1, <3>2, <3>3, <4>1, PTL
      <3>5. CASE candidate.node \notin Responsive
        <4>1. []~IndexedHistoricalTemporalStage4BlockedAtRank(
                      initialContext, candidate, position, rank)
          BY <3>5, PTL
             DEF IndexedHistoricalTemporalStage4BlockedAtRank,
                 IndexedHistoricalTransport!
                   HistoricalTemporalStage4BlockedAtRank,
                 IndexedHistoricalTransport!
                   HistoricalTemporalStage4Pending,
                 IndexedHistoricalTransport!
                   HistoricalProtectedOwnedAtServiceRank,
                 IndexedHistoricalTransport!
                   HistoricalProtectedCandidateOwned
        <4> QED BY <4>1, PTL
      <3> QED BY <3>4, <3>5
    <2>6. \A rank \in
               IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage4EpisodeCarrier:
             IndexedHistoricalTemporalStage4BlockedAtRank(
               initialContext, candidate, position, rank)
               ~> IndexedHistoricalTemporalRankProgressExit(
                    initialContext, candidate, <<4, position>>)
      BY <2>5,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4EpisodeOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF IndexedHistoricalTemporalStage4Progress,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalStage4Progress
    <2> QED BY <2>4, <2>6, PTL
  <1> QED BY <1>1

IndexedHistoricalTemporalStage4Source(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalProtectedOwnedAtServiceRank(
      candidate, <<4, position>>)

IndexedHistoricalTemporalStage4Goal(
    initialContext, candidate, position) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedServiceOwnershipExit(candidate)
  \/ \E lower \in SetLessThan(
       <<4, position>>,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankCarrier):
       IndexedHistoricalTransport(initialContext)!
         HistoricalProtectedOwnedAtServiceRank(candidate, lower)

IndexedHistoricalTemporalStage4LeafProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate \in IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
     position \in Nat:
    IndexedHistoricalTemporalStage4Source(
      initialContext, candidate, position)
      ~> IndexedHistoricalTemporalStage4Goal(
           initialContext, candidate, position)

THEOREM IndexedChainSpecClosesHistoricalTemporalStage4Leaf ==
  IndexedChainSpec
    => IndexedHistoricalTemporalStage4LeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage4Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage4Goal(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalStage4Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage4Pending(
                  initialContext, candidate, position)
      BY <2>1, PTL
         DEF IndexedHistoricalTemporalStage4Source,
             IndexedHistoricalTemporalStage4Pending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage4Pending
    <2>3. IndexedHistoricalTemporalStage4Pending(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<4, position>>)
      BY <1>1,
         IndexedChainSpecClosesHistoricalTemporalStage4Rank
    <2>4. IndexedHistoricalTemporalSupportAt(initialContext)
             /\ IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<4, position>>)
             => IndexedHistoricalTemporalStage4Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRankExitHasWellFoundedSuccessor, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTemporalStage4Goal,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!OwnedServiceRankCarrier
    <2>5. IndexedHistoricalTemporalRankProgressExit(
             initialContext, candidate, <<4, position>>)
             ~> IndexedHistoricalTemporalStage4Goal(
                  initialContext, candidate, position)
      BY <2>1, <2>4, PTL
    <2> QED BY <2>2, <2>3, <2>5, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalTemporalStage4LeafProperty

(***************************************************************************
Indexed historical Stage-5 lift.

This lift intentionally avoids `AsyncSpecAt`: a lagging historical target can
own a joined exact context before every other responsive peer joins it.  The
product already supplies weak fairness for the target's exact historical I/O
step.  The action-local theorem above supplies enabledness, nonstuttering
queue removal, and strict FIFO descent for that one owner.
***************************************************************************)

IndexedHistoricalTemporalStage5Pending(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage5Pending(candidate, position)

THEOREM IndexedHistoricalStage5PendingHasJoinedFairOwner ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    => /\ candidate.node \in Responsive
       /\ candidate.node \in joinedByContext[initialContext]
       /\ initialContext \in JoinedContexts
       /\ IndexedHistoricalTransport(initialContext)!
            HistoricalRecoveryTarget(candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedCompositionInvariant,
                IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position)
         PROVE /\ candidate.node \in Responsive
               /\ candidate.node \in joinedByContext[initialContext]
               /\ initialContext \in JoinedContexts
               /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalRecoveryTarget(candidate.node)
    <2>1. /\ candidate.node \in Responsive
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTarget(candidate.node)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage5CarrierFacts
         DEF IndexedHistoricalTemporalStage5Pending
    <2>2. IndexedAsync(initialContext)!
             HistoricalRecoveryTarget(candidate.node)
      BY <2>1, Isa
         DEF IndexedHistoricalTransport!HistoricalRecoveryTarget,
             IndexedAsync!HistoricalRecoveryTarget,
             IndexedScheduler
    <2>3. candidate.node \in joinedByContext[initialContext]
      BY <1>1, <2>2
         DEF IndexedCompositionInvariant,
             IndexedHistoricalRecoveryTargetCoherence
    <2>4. initialContext \in JoinedContexts
      BY <2>3 DEF JoinedContexts
    <2> QED BY <2>1, <2>3, <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage5PendingEnablesExactWorker ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    => ENABLED
         IndexedHistoricalRecoveryIoWorkerStep(
           initialContext, candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedCompositionInvariant,
                IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position)
         PROVE ENABLED
                 IndexedHistoricalRecoveryIoWorkerStep(
                   initialContext, candidate.node)
    <2>1. /\ candidate.node \in Responsive
           /\ candidate.node \in joinedByContext[initialContext]
           /\ initialContext \in JoinedContexts
      BY <1>1, IndexedHistoricalStage5PendingHasJoinedFairOwner
    <2>2. ENABLED
             IndexedHistoricalTransport(initialContext)!
               PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalRecoveryIoWorkerEnabledAfterGst
         DEF IndexedHistoricalTemporalStage5Pending,
             IndexedHistoricalTransport!
               HistoricalTemporalStage5Pending,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned,
             IndexedHistoricalTransport!HistoricalRecoveryTarget
    <2>3. ENABLED IndexedAsync(initialContext)!
             PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <2>2, Isa
         DEF IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!
               ServiceHistoricalRecoveryIoWorker,
             IndexedAsync!ServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!ServiceIoWorkerWork,
             IndexedAsync!ServiceIoWorkerWork
    <2>4. ENABLED
             IndexedHistoricalRecoveryIoWorkerStep(
               initialContext, candidate.node)
      BY <1>1, <2>1, <2>3,
         IndexedFairActionsRemainEnabledInProduct
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage5WorkerIsNonstuttering ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    /\ IndexedHistoricalRecoveryIoWorkerStep(
         initialContext, candidate.node)
    => <<IndexedHistoricalRecoveryIoWorkerStep(
           initialContext, candidate.node)>>_IndexedChainVars
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position),
                IndexedHistoricalRecoveryIoWorkerStep(
                  initialContext, candidate.node)
         PROVE <<IndexedHistoricalRecoveryIoWorkerStep(
                   initialContext, candidate.node)>>_IndexedChainVars
    <2>1. <<IndexedHistoricalTransport(initialContext)!
               PostGstServiceHistoricalRecoveryIoWorker(
                 candidate.node)>>_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalQueuedIoServiceIsNonstuttering, Isa
         DEF IndexedHistoricalTemporalStage5Pending,
             IndexedHistoricalRecoveryIoWorkerStep,
             IndexedHistoricalTransport!
               HistoricalTemporalStage5Pending,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned,
             IndexedHistoricalTransport!HistoricalRecoveryTarget,
             IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker
    <2>2. IndexedAsyncStateShape
      BY <1>1
         DEF IndexedHistoricalRecoveryIoWorkerStep, IndexedChainNext
    <2>3. IndexedHistoricalTransport(initialContext)!AsyncAllVars =
             IndexedAsyncStateAt(initialContext)
      BY <1>1, <2>2, IndexedHistoricalTransportVariablesAreExact
    <2>4. IndexedChainVars' # IndexedChainVars
      BY <2>1, <2>3, Isa
         DEF IndexedChainVars, IndexedAsyncStateAt
    <2> QED BY <1>1, <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage5PendingEnablesFairOccurrence ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    => ENABLED
         <<IndexedHistoricalRecoveryIoWorkerStep(
             initialContext, candidate.node)>>_IndexedChainVars
BY IndexedHistoricalStage5PendingEnablesExactWorker,
   IndexedHistoricalStage5WorkerIsNonstuttering,
   ENABLEDaxioms

THEOREM IndexedHistoricalStage5FairOccurrenceStrictlyProgresses ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    /\ <<IndexedHistoricalRecoveryIoWorkerStep(
           initialContext, candidate.node)>>_IndexedChainVars
    => IndexedHistoricalTemporalRankProgressExit(
         initialContext, candidate, <<5, position>>)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position),
                <<IndexedHistoricalRecoveryIoWorkerStep(
                    initialContext, candidate.node)>>_IndexedChainVars
         PROVE IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<5, position>>)'
    <2>1. IndexedHistoricalTransport(initialContext)!
             PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <1>1, Isa
         DEF IndexedHistoricalRecoveryIoWorkerStep,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage5WorkerStrictlyProgresses
         DEF IndexedHistoricalTemporalStage5Pending,
             IndexedHistoricalTemporalRankProgressExit
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage5UnlessProgress ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalTemporalStage5Pending(
            initialContext, candidate, position)'
       \/ IndexedHistoricalTemporalRankProgressExit(
            initialContext, candidate, <<5, position>>)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalTemporalStage5Pending(
                initialContext, candidate, position)'
           \/ IndexedHistoricalTemporalRankProgressExit(
                initialContext, candidate, <<5, position>>)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage5UnlessProgress
         DEF IndexedHistoricalTemporalStage5Pending,
             IndexedHistoricalTemporalRankProgressExit
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalTemporalStage5Rank ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position \in Nat:
         IndexedHistoricalTemporalStage5Pending(
           initialContext, candidate, position)
           ~> IndexedHistoricalTemporalRankProgressExit(
                initialContext, candidate, <<5, position>>)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage5Pending(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<5, position>>)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. IndexedHistoricalTemporalStage5Pending(
             initialContext, candidate, position)
             /\ [IndexedChainNext]_IndexedChainVars
             => \/ IndexedHistoricalTemporalStage5Pending(
                    initialContext, candidate, position)'
                \/ IndexedHistoricalTemporalRankProgressExit(
                     initialContext, candidate, <<5, position>>)'
      BY <1>1, IndexedHistoricalStage5UnlessProgress
    <2>4. IndexedCompositionInvariant
             /\ IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position)
             => ENABLED
                  <<IndexedHistoricalRecoveryIoWorkerStep(
                      initialContext, candidate.node)>>_IndexedChainVars
      BY <1>1, IndexedHistoricalStage5PendingEnablesFairOccurrence
    <2>5. /\ IndexedHistoricalTemporalStage5Pending(
               initialContext, candidate, position)
             /\ <<IndexedHistoricalRecoveryIoWorkerStep(
                    initialContext, candidate.node)>>_IndexedChainVars
            => IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<5, position>>)'
      BY <1>1,
         IndexedHistoricalStage5FairOccurrenceStrictlyProgresses
    <2>6. CASE candidate.node \in Responsive
      <3>1. WF_IndexedChainVars(
               IndexedHistoricalRecoveryIoWorkerStep(
                 initialContext, candidate.node))
        BY <1>1, <2>6 DEF IndexedChainSpec, IndexedFairness
      <3> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <3>1, PTL
    <2>7. CASE candidate.node \notin Responsive
      <3>1. []~IndexedHistoricalTemporalStage5Pending(
                    initialContext, candidate, position)
        BY <2>7, PTL
           DEF IndexedHistoricalTemporalStage5Pending,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage5Pending,
               IndexedHistoricalTransport!
                 HistoricalProtectedOwnedAtServiceRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

IndexedHistoricalTemporalStage5Source(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalProtectedOwnedAtServiceRank(
      candidate, <<5, position>>)

IndexedHistoricalTemporalStage5Goal(
    initialContext, candidate, position) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedServiceOwnershipExit(candidate)
  \/ \E lower \in SetLessThan(
       <<5, position>>,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankCarrier):
       IndexedHistoricalTransport(initialContext)!
         HistoricalProtectedOwnedAtServiceRank(candidate, lower)

IndexedHistoricalTemporalStage5LeafProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate \in IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
     position \in Nat:
    IndexedHistoricalTemporalStage5Source(
      initialContext, candidate, position)
      ~> IndexedHistoricalTemporalStage5Goal(
           initialContext, candidate, position)

THEOREM IndexedChainSpecClosesHistoricalTemporalStage5Leaf ==
  IndexedChainSpec
    => IndexedHistoricalTemporalStage5LeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage5Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage5Goal(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalStage5Source(
             initialContext, candidate, position)
             ~>
           IndexedHistoricalTemporalStage5Pending(
             initialContext, candidate, position)
      BY <2>1, PTL
         DEF IndexedHistoricalTemporalStage5Source,
             IndexedHistoricalTemporalStage5Pending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage5Pending
    <2>3. IndexedHistoricalTemporalStage5Pending(
             initialContext, candidate, position)
             ~>
           IndexedHistoricalTemporalRankProgressExit(
             initialContext, candidate, <<5, position>>)
      BY <1>1,
         IndexedChainSpecClosesHistoricalTemporalStage5Rank
    <2>4. IndexedHistoricalTemporalSupportAt(initialContext)
             /\ IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<5, position>>)
             => IndexedHistoricalTemporalStage5Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRankExitHasWellFoundedSuccessor, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTemporalStage5Goal,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!OwnedServiceRankCarrier
    <2>5. IndexedHistoricalTemporalRankProgressExit(
             initialContext, candidate, <<5, position>>)
             ~>
           IndexedHistoricalTemporalStage5Goal(
             initialContext, candidate, position)
      BY <2>1, <2>4, PTL
    <2> QED BY <2>2, <2>3, <2>5, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalTemporalStage5LeafProperty

(***************************************************************************
Indexed historical Stage-6 runner subkernels.

Pre-admission, the owed causal head, and non-Completion capacity all consume
the same joined historical runner.  Their rank carriers differ, so the mode
tag below selects the existing one-height carrier and ordering without
flattening or inventing a new fairness action.
***************************************************************************)

IndexedHistoricalStage6RunnerModes ==
  {"PreAdmission", "Owed", "NonCompletion"}

IndexedHistoricalStage6RunnerCarrier(initialContext, mode) ==
  IF mode = "NonCompletion"
  THEN IndexedHistoricalTransport(initialContext)!
         Stage4CapacityCarrier
  ELSE IndexedHistoricalTransport(initialContext)!
         ReadyRunAuxCarrier

IndexedHistoricalStage6RunnerOrdering(initialContext, mode) ==
  IF mode = "NonCompletion"
  THEN IndexedHistoricalTransport(initialContext)!
         Stage4CapacityOrdering
  ELSE IndexedHistoricalTransport(initialContext)!
         ReadyRunAuxOrdering

IndexedHistoricalStage6RunnerBlocked(
    initialContext, mode, candidate, position, rank) ==
  CASE mode = "PreAdmission" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6PreAdmissionBlockedAtAux(
             candidate, position, rank)
    [] mode = "Owed" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6OwedBlockedAtAux(
             candidate, position, rank)
    [] mode = "NonCompletion" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6NonCompletionBlockedAtRank(
             candidate, position, rank)
    [] OTHER -> FALSE

IndexedHistoricalStage6RunnerProgress(
    initialContext, mode, candidate, position, rank) ==
  CASE mode = "PreAdmission" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6PreAdmissionAuxProgress(
             candidate, position, rank)
    [] mode = "Owed" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6OwedAuxProgress(
             candidate, position, rank)
    [] mode = "NonCompletion" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6NonCompletionProgress(
             candidate, position, rank)
    [] OTHER -> FALSE

IndexedHistoricalStage6RunnerGoal(
    initialContext, mode, candidate, position) ==
  CASE mode = "PreAdmission" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6PreAdmissionGoal(
             candidate, position)
    [] mode = "Owed" ->
         IndexedHistoricalTemporalRankProgressExit(
           initialContext, candidate, <<6, position>>)
    [] mode = "NonCompletion" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6NonCompletionGoal(
             candidate, position)
    [] OTHER -> FALSE

THEOREM IndexedHistoricalStage6RunnerOrderingIsWellFounded ==
  \A initialContext \in AdmissibleContextRecords,
     mode \in IndexedHistoricalStage6RunnerModes:
    IsWellFoundedOn(
      IndexedHistoricalStage6RunnerOrdering(initialContext, mode),
      IndexedHistoricalStage6RunnerCarrier(initialContext, mode))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW mode \in IndexedHistoricalStage6RunnerModes
         PROVE IsWellFoundedOn(
                 IndexedHistoricalStage6RunnerOrdering(
                   initialContext, mode),
                 IndexedHistoricalStage6RunnerCarrier(
                   initialContext, mode))
    <2>1. CASE mode = "NonCompletion"
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           Stage4CapacityOrderingIsWellFounded
         DEF IndexedHistoricalStage6RunnerOrdering,
             IndexedHistoricalStage6RunnerCarrier
    <2>2. CASE mode # "NonCompletion"
      BY <2>2,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxOrderingIsWellFounded
         DEF IndexedHistoricalStage6RunnerOrdering,
             IndexedHistoricalStage6RunnerCarrier
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6RunnerBlockedHasPendingOwner ==
  \A initialContext \in AdmissibleContextRecords,
     mode \in IndexedHistoricalStage6RunnerModes,
     candidate, position, rank:
    IndexedHistoricalStage6RunnerBlocked(
      initialContext, mode, candidate, position, rank)
      => IndexedHistoricalTemporalCandidateRunnerPending(
           initialContext, candidate)
BY Isa
   DEF IndexedHistoricalStage6RunnerModes,
       IndexedHistoricalStage6RunnerBlocked,
       IndexedHistoricalTemporalCandidateRunnerPending,
       IndexedHistoricalTemporalSupportAt,
       IndexedHistoricalTransport!
         HistoricalTemporalStage6PreAdmissionBlockedAtAux,
       IndexedHistoricalTransport!
         HistoricalTemporalStage6OwedBlockedAtAux,
       IndexedHistoricalTransport!
         HistoricalTemporalStage6NonCompletionBlockedAtRank,
       IndexedHistoricalTransport!
         HistoricalTemporalStage6Pending,
       IndexedHistoricalTransport!
         HistoricalProtectedOwnedAtServiceRank

THEOREM IndexedHistoricalStage6RunnerStrictlyProgresses ==
  \A initialContext \in AdmissibleContextRecords,
     mode \in IndexedHistoricalStage6RunnerModes,
     candidate, position:
    \A rank \in
         IndexedHistoricalStage6RunnerCarrier(initialContext, mode):
      /\ IndexedHistoricalStage6RunnerBlocked(
           initialContext, mode, candidate, position, rank)
      /\ <<IndexedRunHistoricalRecoveryStep(
             initialContext, candidate.node)>>_IndexedChainVars
      => IndexedHistoricalStage6RunnerProgress(
           initialContext, mode, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW mode \in IndexedHistoricalStage6RunnerModes,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalStage6RunnerCarrier(
                    initialContext, mode),
                IndexedHistoricalStage6RunnerBlocked(
                  initialContext, mode, candidate, position, rank),
                <<IndexedRunHistoricalRecoveryStep(
                    initialContext, candidate.node)>>_IndexedChainVars
         PROVE IndexedHistoricalStage6RunnerProgress(
                 initialContext, mode, candidate, position, rank)'
    <2>1. IndexedHistoricalTransport(initialContext)!
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, Isa
         DEF IndexedRunHistoricalRecoveryStep,
             IndexedAsync!PostGstRunHistoricalRecoveryNode,
             IndexedHistoricalTransport!
               PostGstRunHistoricalRecoveryNode
    <2>2. CASE mode = "PreAdmission"
      BY <1>1, <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6PreAdmissionSameRunnerProgress
         DEF IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalStage6RunnerProgress,
             IndexedHistoricalStage6RunnerCarrier
    <2>3. CASE mode = "Owed"
      BY <1>1, <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6OwedSameRunnerProgress
         DEF IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalStage6RunnerProgress,
             IndexedHistoricalStage6RunnerCarrier
    <2>4. CASE mode = "NonCompletion"
      BY <1>1, <2>1, <2>4,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6NonCompletionSameRunnerProgress
         DEF IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalStage6RunnerProgress,
             IndexedHistoricalStage6RunnerCarrier
    <2> QED BY <1>1, <2>2, <2>3, <2>4
         DEF IndexedHistoricalStage6RunnerModes
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6RunnerUnlessProgress ==
  \A initialContext \in AdmissibleContextRecords,
     mode \in IndexedHistoricalStage6RunnerModes,
     candidate, position:
    \A rank \in
         IndexedHistoricalStage6RunnerCarrier(initialContext, mode):
      /\ IndexedHistoricalStage6RunnerBlocked(
           initialContext, mode, candidate, position, rank)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedHistoricalStage6RunnerBlocked(
              initialContext, mode, candidate, position, rank)'
         \/ IndexedHistoricalStage6RunnerProgress(
              initialContext, mode, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW mode \in IndexedHistoricalStage6RunnerModes,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalStage6RunnerCarrier(
                    initialContext, mode),
                IndexedHistoricalStage6RunnerBlocked(
                  initialContext, mode, candidate, position, rank),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalStage6RunnerBlocked(
                initialContext, mode, candidate, position, rank)'
           \/ IndexedHistoricalStage6RunnerProgress(
                initialContext, mode, candidate, position, rank)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. CASE IndexedHistoricalTransport(initialContext)!
                  PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>2,
         IndexedHistoricalStage6RunnerStrictlyProgresses, Isa
         DEF IndexedRunHistoricalRecoveryStep
    <2>3. CASE ~IndexedHistoricalTransport(initialContext)!
                   PostGstRunHistoricalRecoveryNode(candidate.node)
      <3>1. CASE mode = "PreAdmission"
        BY <1>1, <2>1, <2>3, <3>1,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6PreAdmissionOtherStep
           DEF IndexedHistoricalStage6RunnerBlocked,
               IndexedHistoricalStage6RunnerProgress,
               IndexedHistoricalStage6RunnerCarrier
      <3>2. CASE mode = "Owed"
        BY <1>1, <2>1, <2>3, <3>2,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6OwedOtherStep
           DEF IndexedHistoricalStage6RunnerBlocked,
               IndexedHistoricalStage6RunnerProgress,
               IndexedHistoricalStage6RunnerCarrier
      <3>3. CASE mode = "NonCompletion"
        BY <1>1, <2>1, <2>3, <3>3,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6NonCompletionOtherStep
           DEF IndexedHistoricalStage6RunnerBlocked,
               IndexedHistoricalStage6RunnerProgress,
               IndexedHistoricalStage6RunnerCarrier
      <3> QED BY <1>1, <3>1, <3>2, <3>3
           DEF IndexedHistoricalStage6RunnerModes
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6FairRunnerOneStep ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          mode \in IndexedHistoricalStage6RunnerModes,
          candidate, position:
         \A rank \in
              IndexedHistoricalStage6RunnerCarrier(
                initialContext, mode):
           IndexedHistoricalStage6RunnerBlocked(
             initialContext, mode, candidate, position, rank)
             ~> IndexedHistoricalStage6RunnerProgress(
                  initialContext, mode, candidate, position, rank)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW mode \in IndexedHistoricalStage6RunnerModes,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalStage6RunnerCarrier(
                    initialContext, mode)
         PROVE IndexedHistoricalStage6RunnerBlocked(
                 initialContext, mode, candidate, position, rank)
                 ~> IndexedHistoricalStage6RunnerProgress(
                      initialContext, mode, candidate, position, rank)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. IndexedHistoricalStage6RunnerBlocked(
             initialContext, mode, candidate, position, rank)
             /\ [IndexedChainNext]_IndexedChainVars
            => \/ IndexedHistoricalStage6RunnerBlocked(
                    initialContext, mode, candidate, position, rank)'
               \/ IndexedHistoricalStage6RunnerProgress(
                    initialContext, mode, candidate, position, rank)'
      BY <1>1, IndexedHistoricalStage6RunnerUnlessProgress
    <2>4. IndexedCompositionInvariant
             /\ IndexedHistoricalStage6RunnerBlocked(
                  initialContext, mode, candidate, position, rank)
            => ENABLED
                 <<IndexedRunHistoricalRecoveryStep(
                     initialContext, candidate.node)>>_IndexedChainVars
      BY <1>1,
         IndexedHistoricalStage6RunnerBlockedHasPendingOwner,
         IndexedHistoricalCandidateRunnerEnablesFairOccurrence
    <2>5. /\ IndexedHistoricalStage6RunnerBlocked(
                 initialContext, mode, candidate, position, rank)
             /\ <<IndexedRunHistoricalRecoveryStep(
                    initialContext, candidate.node)>>_IndexedChainVars
            => IndexedHistoricalStage6RunnerProgress(
                 initialContext, mode, candidate, position, rank)'
      BY <1>1,
         IndexedHistoricalStage6RunnerStrictlyProgresses
    <2>6. CASE candidate.node \in Responsive
      <3>1. WF_IndexedChainVars(
               IndexedRunHistoricalRecoveryStep(
                 initialContext, candidate.node))
        BY <1>1, <2>6 DEF IndexedChainSpec, IndexedFairness
      <3> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <3>1, PTL
    <2>7. CASE candidate.node \notin Responsive
      <3>1. []~IndexedHistoricalStage6RunnerBlocked(
                    initialContext, mode, candidate, position, rank)
        BY <2>7, PTL
           DEF IndexedHistoricalStage6RunnerBlocked,
               IndexedHistoricalStage6RunnerModes,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6PreAdmissionBlockedAtAux,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6OwedBlockedAtAux,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6NonCompletionBlockedAtRank,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6Pending,
               IndexedHistoricalTransport!
                 HistoricalProtectedOwnedAtServiceRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6RunnerModeDescent ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          mode \in IndexedHistoricalStage6RunnerModes,
          candidate, position:
         \A rank \in
              IndexedHistoricalStage6RunnerCarrier(
                initialContext, mode):
           IndexedHistoricalStage6RunnerBlocked(
             initialContext, mode, candidate, position, rank)
             ~> IndexedHistoricalStage6RunnerGoal(
                  initialContext, mode, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW mode \in IndexedHistoricalStage6RunnerModes,
                NEW candidate, NEW position
         PROVE \A rank \in
                    IndexedHistoricalStage6RunnerCarrier(
                      initialContext, mode):
                  IndexedHistoricalStage6RunnerBlocked(
                    initialContext, mode, candidate, position, rank)
                    ~> IndexedHistoricalStage6RunnerGoal(
                         initialContext, mode, candidate, position)
    <2>1. \A rank \in
               IndexedHistoricalStage6RunnerCarrier(
                 initialContext, mode):
             IndexedHistoricalStage6RunnerBlocked(
               initialContext, mode, candidate, position, rank)
               ~> IndexedHistoricalStage6RunnerProgress(
                    initialContext, mode, candidate, position, rank)
      BY <1>1, IndexedHistoricalStage6FairRunnerOneStep
    <2> QED BY <2>1,
         IndexedHistoricalStage6RunnerOrderingIsWellFounded,
         WellFoundedLeadsTo, Isa
         DEF IndexedHistoricalStage6RunnerProgress,
             IndexedHistoricalStage6RunnerGoal,
             IndexedHistoricalStage6RunnerModes,
             IndexedHistoricalStage6RunnerCarrier,
             IndexedHistoricalStage6RunnerOrdering,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6PreAdmissionAuxProgress,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6OwedAuxProgress,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6NonCompletionProgress,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6PreAdmissionGoal,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6NonCompletionGoal,
             IndexedHistoricalTemporalRankProgressExit
  <1> QED BY <1>1

(***************************************************************************
Each runner mode starts at the exact current one-height rank.  These three
entry lemmas keep that type argument explicit and then reuse the common
product descent above.
***************************************************************************)

THEOREM IndexedChainSpecClosesHistoricalStage6PreAdmission ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6Pending(candidate, position)
           ~> IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6PreAdmissionGoal(
                   candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6Pending(candidate, position)
                 ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6PreAdmissionGoal(
                   candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6Pending(candidate, position)
             ~> (IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage6PreAdmissionGoal(
                     candidate, position)
                  \/ IndexedHistoricalStage6RunnerBlocked(
                       initialContext, "PreAdmission",
                       candidate, position,
                       IndexedHistoricalTransport(initialContext)!
                         ReadyRunAuxRank(candidate.node)))
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxRankInCarrier, PTL
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6PreAdmissionBlockedAtAux
    <2>3. \A rank \in
               IndexedHistoricalStage6RunnerCarrier(
                 initialContext, "PreAdmission"):
             IndexedHistoricalStage6RunnerBlocked(
               initialContext, "PreAdmission",
               candidate, position, rank)
               ~> IndexedHistoricalTransport(initialContext)!
                     HistoricalTemporalStage6PreAdmissionGoal(
                       candidate, position)
      BY <1>1, IndexedHistoricalStage6RunnerModeDescent, Isa
         DEF IndexedHistoricalStage6RunnerModes,
             IndexedHistoricalStage6RunnerGoal
    <2>4. IndexedHistoricalStage6RunnerBlocked(
             initialContext, "PreAdmission", candidate, position,
             IndexedHistoricalTransport(initialContext)!
               ReadyRunAuxRank(candidate.node))
             ~> IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage6PreAdmissionGoal(
                     candidate, position)
      BY <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxRankInCarrier
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerCarrier
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalStage6Owed ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6OwedCausalReady(candidate, position)
           ~> IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<6, position>>)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6OwedCausalReady(
                   candidate, position)
                 ~>
               IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<6, position>>)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6OwedCausalReady(
               candidate, position)
             ~> IndexedHistoricalStage6RunnerBlocked(
                   initialContext, "Owed", candidate, position,
                   IndexedHistoricalTransport(initialContext)!
                     ReadyRunAuxRank(candidate.node))
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxRankInCarrier, PTL
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6OwedBlockedAtAux
    <2>3. \A rank \in
               IndexedHistoricalStage6RunnerCarrier(
                 initialContext, "Owed"):
             IndexedHistoricalStage6RunnerBlocked(
               initialContext, "Owed", candidate, position, rank)
               ~> IndexedHistoricalTemporalRankProgressExit(
                     initialContext, candidate, <<6, position>>)
      BY <1>1, IndexedHistoricalStage6RunnerModeDescent, Isa
         DEF IndexedHistoricalStage6RunnerModes,
             IndexedHistoricalStage6RunnerGoal
    <2>4. IndexedHistoricalStage6RunnerBlocked(
             initialContext, "Owed", candidate, position,
             IndexedHistoricalTransport(initialContext)!
               ReadyRunAuxRank(candidate.node))
             ~> IndexedHistoricalTemporalRankProgressExit(
                   initialContext, candidate, <<6, position>>)
      BY <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxRankInCarrier
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerCarrier
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalStage6NonCompletion ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6NonCompletionCapacityBlocked(
             candidate, position)
           ~> IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6NonCompletionGoal(
                   candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6NonCompletionCapacityBlocked(
                   candidate, position)
                 ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6NonCompletionGoal(
                   candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6NonCompletionCapacityBlocked(
               candidate, position)
             ~> IndexedHistoricalStage6RunnerBlocked(
                   initialContext, "NonCompletion",
                   candidate, position,
                   IndexedHistoricalTransport(initialContext)!
                     Stage4CapacityRank(candidate.node))
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           Stage4CapacityRankInCarrier, PTL
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6NonCompletionBlockedAtRank
    <2>3. \A rank \in
               IndexedHistoricalStage6RunnerCarrier(
                 initialContext, "NonCompletion"):
             IndexedHistoricalStage6RunnerBlocked(
               initialContext, "NonCompletion",
               candidate, position, rank)
               ~> IndexedHistoricalTransport(initialContext)!
                     HistoricalTemporalStage6NonCompletionGoal(
                       candidate, position)
      BY <1>1, IndexedHistoricalStage6RunnerModeDescent, Isa
         DEF IndexedHistoricalStage6RunnerModes,
             IndexedHistoricalStage6RunnerGoal
    <2>4. IndexedHistoricalStage6RunnerBlocked(
             initialContext, "NonCompletion", candidate, position,
             IndexedHistoricalTransport(initialContext)!
               Stage4CapacityRank(candidate.node))
             ~> IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage6NonCompletionGoal(
                     candidate, position)
      BY <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           Stage4CapacityRankInCarrier
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerCarrier
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

(***************************************************************************
Indexed Stage-6 Completion physical-I/O drain.

The exact historical I/O action is already weakly fair in the product.  A
positive queue depth makes that action nonstuttering, and the one-height
action lemma decreases the natural depth or opens the Completion goal.
***************************************************************************)

IndexedHistoricalStage6CompletionIoBlockedAtDepth(
    initialContext, candidate, position, depth) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionIoBlockedAtDepth(
      candidate, position, depth)

IndexedHistoricalStage6CompletionIoDrainGoal(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionIoDrainGoal(
      candidate, position)

IndexedHistoricalStage6CompletionIoProgress(
    initialContext, candidate, position, depth) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionIoProgress(
      candidate, position, depth)

THEOREM IndexedHistoricalStage6CompletionIoBlockedHasFairOwner ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, depth:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
         initialContext, candidate, position, depth)
    => /\ candidate.node \in Responsive
       /\ candidate.node \in joinedByContext[initialContext]
       /\ initialContext \in JoinedContexts
       /\ IndexedHistoricalTransport(initialContext)!
            HistoricalRecoveryTarget(candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth,
                IndexedCompositionInvariant,
                IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth)
         PROVE /\ candidate.node \in Responsive
               /\ candidate.node \in joinedByContext[initialContext]
               /\ initialContext \in JoinedContexts
               /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalRecoveryTarget(candidate.node)
    <2>1. IndexedHistoricalTemporalCandidateRunnerPending(
             initialContext, candidate)
      BY <1>1
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
             IndexedHistoricalTemporalCandidateRunnerPending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6CompletionIoBlockedAtDepth,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6CompletionCapacityBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6Pending,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalCandidateRunnerHasJoinedFairOwner
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionIoBlockedEnablesExactWorker ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, depth:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
         initialContext, candidate, position, depth)
    => ENABLED
         IndexedHistoricalRecoveryIoWorkerStep(
           initialContext, candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth,
                IndexedCompositionInvariant,
                IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth)
         PROVE ENABLED
                 IndexedHistoricalRecoveryIoWorkerStep(
                   initialContext, candidate.node)
    <2>1. /\ candidate.node \in Responsive
           /\ candidate.node \in joinedByContext[initialContext]
           /\ initialContext \in JoinedContexts
      BY <1>1,
         IndexedHistoricalStage6CompletionIoBlockedHasFairOwner
    <2>2. /\ IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTarget(candidate.node)
           /\ IndexedHistoricalTransport(initialContext)!
                AsyncStrongTypeInvariant
           /\ IndexedHistoricalTransport(initialContext)!gst
           /\ IndexedHistoricalTransport(initialContext)!
                AsyncIoQueueDepth(candidate.node) > 0
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionIoBlockedCoreFacts
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth
    <2>3. ENABLED
             IndexedHistoricalTransport(initialContext)!
               PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalRecoveryIoWorkerEnabledAfterGst
    <2>4. ENABLED IndexedAsync(initialContext)!
             PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <2>3, Isa
         DEF IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!
               ServiceHistoricalRecoveryIoWorker,
             IndexedAsync!ServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!ServiceIoWorkerWork,
             IndexedAsync!ServiceIoWorkerWork
    <2> QED BY <1>1, <2>1, <2>4,
         IndexedFairActionsRemainEnabledInProduct
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionIoWorkerIsNonstuttering ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, depth:
    /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
         initialContext, candidate, position, depth)
    /\ IndexedHistoricalRecoveryIoWorkerStep(
         initialContext, candidate.node)
    => <<IndexedHistoricalRecoveryIoWorkerStep(
           initialContext, candidate.node)>>_IndexedChainVars
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth,
                IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth),
                IndexedHistoricalRecoveryIoWorkerStep(
                  initialContext, candidate.node)
         PROVE <<IndexedHistoricalRecoveryIoWorkerStep(
                   initialContext, candidate.node)>>_IndexedChainVars
    <2>1. /\ IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTarget(candidate.node)
           /\ IndexedHistoricalTransport(initialContext)!
                AsyncTypeInvariant
           /\ IndexedHistoricalTransport(initialContext)!
                AsyncIoQueueDepth(candidate.node) > 0
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionIoBlockedCoreFacts
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth
    <2>2. <<IndexedHistoricalTransport(initialContext)!
               PostGstServiceHistoricalRecoveryIoWorker(
                 candidate.node)>>_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalQueuedIoServiceIsNonstuttering, Isa
         DEF IndexedHistoricalRecoveryIoWorkerStep,
             IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker
    <2>3. IndexedAsyncStateShape
      BY <1>1
         DEF IndexedHistoricalRecoveryIoWorkerStep, IndexedChainNext
    <2>4. IndexedHistoricalTransport(initialContext)!AsyncAllVars =
             IndexedAsyncStateAt(initialContext)
      BY <1>1, <2>3,
         IndexedHistoricalTransportVariablesAreExact
    <2>5. IndexedChainVars' # IndexedChainVars
      BY <2>2, <2>4, Isa
         DEF IndexedChainVars, IndexedAsyncStateAt
    <2> QED BY <1>1, <2>5
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionIoBlockedEnablesFairOccurrence ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, depth:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
         initialContext, candidate, position, depth)
    => ENABLED
         <<IndexedHistoricalRecoveryIoWorkerStep(
             initialContext, candidate.node)>>_IndexedChainVars
BY IndexedHistoricalStage6CompletionIoBlockedEnablesExactWorker,
   IndexedHistoricalStage6CompletionIoWorkerIsNonstuttering,
   ENABLEDaxioms

THEOREM IndexedHistoricalStage6CompletionIoFairOccurrenceProgresses ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A depth \in Nat:
      /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
           initialContext, candidate, position, depth)
      /\ <<IndexedHistoricalRecoveryIoWorkerStep(
             initialContext, candidate.node)>>_IndexedChainVars
      => IndexedHistoricalStage6CompletionIoProgress(
           initialContext, candidate, position, depth)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth \in Nat,
                IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth),
                <<IndexedHistoricalRecoveryIoWorkerStep(
                    initialContext, candidate.node)>>_IndexedChainVars
         PROVE IndexedHistoricalStage6CompletionIoProgress(
                 initialContext, candidate, position, depth)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. IndexedHistoricalTransport(initialContext)!
             PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <1>1, Isa
         DEF IndexedHistoricalRecoveryIoWorkerStep,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker
    <2> QED BY <1>1, <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionIoWorkerStrictlyProgresses
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
             IndexedHistoricalStage6CompletionIoProgress
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionIoUnlessProgress ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A depth \in Nat:
      /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
           initialContext, candidate, position, depth)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
              initialContext, candidate, position, depth)'
         \/ IndexedHistoricalStage6CompletionIoProgress(
              initialContext, candidate, position, depth)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth \in Nat,
                IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                initialContext, candidate, position, depth)'
           \/ IndexedHistoricalStage6CompletionIoProgress(
                initialContext, candidate, position, depth)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. CASE IndexedHistoricalTransport(initialContext)!
                  PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <1>1, <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionIoWorkerStrictlyProgresses
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
             IndexedHistoricalStage6CompletionIoProgress
    <2>3. CASE ~IndexedHistoricalTransport(initialContext)!
                   PostGstServiceHistoricalRecoveryIoWorker(
                     candidate.node)
      BY <1>1, <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionIoOtherStep
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
             IndexedHistoricalStage6CompletionIoProgress
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM IndexedChainSpecHistoricalStage6CompletionIoOneStep ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         \A depth \in Nat:
           IndexedHistoricalStage6CompletionIoBlockedAtDepth(
             initialContext, candidate, position, depth)
             ~> IndexedHistoricalStage6CompletionIoProgress(
                  initialContext, candidate, position, depth)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth \in Nat
         PROVE IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                 initialContext, candidate, position, depth)
                 ~>
               IndexedHistoricalStage6CompletionIoProgress(
                 initialContext, candidate, position, depth)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. IndexedHistoricalStage6CompletionIoBlockedAtDepth(
             initialContext, candidate, position, depth)
             /\ [IndexedChainNext]_IndexedChainVars
            => \/ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                    initialContext, candidate, position, depth)'
               \/ IndexedHistoricalStage6CompletionIoProgress(
                    initialContext, candidate, position, depth)'
      BY <1>1,
         IndexedHistoricalStage6CompletionIoUnlessProgress
    <2>4. IndexedCompositionInvariant
             /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth)
            => ENABLED
                 <<IndexedHistoricalRecoveryIoWorkerStep(
                     initialContext, candidate.node)>>_IndexedChainVars
      BY <1>1,
         IndexedHistoricalStage6CompletionIoBlockedEnablesFairOccurrence
    <2>5. /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                 initialContext, candidate, position, depth)
             /\ <<IndexedHistoricalRecoveryIoWorkerStep(
                    initialContext, candidate.node)>>_IndexedChainVars
            => IndexedHistoricalStage6CompletionIoProgress(
                 initialContext, candidate, position, depth)'
      BY <1>1,
         IndexedHistoricalStage6CompletionIoFairOccurrenceProgresses
    <2>6. CASE candidate.node \in Responsive
      <3>1. WF_IndexedChainVars(
               IndexedHistoricalRecoveryIoWorkerStep(
                 initialContext, candidate.node))
        BY <1>1, <2>6 DEF IndexedChainSpec, IndexedFairness
      <3> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <3>1, PTL
    <2>7. CASE candidate.node \notin Responsive
      <3>1. []~IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                    initialContext, candidate, position, depth)
        BY <2>7, PTL
           DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionIoBlockedAtDepth,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionCapacityBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6Pending,
               IndexedHistoricalTransport!
                 HistoricalProtectedOwnedAtServiceRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM IndexedChainSpecDrainsHistoricalStage6CompletionIo ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         \A depth \in Nat:
           IndexedHistoricalStage6CompletionIoBlockedAtDepth(
             initialContext, candidate, position, depth)
             ~> IndexedHistoricalStage6CompletionIoDrainGoal(
                  initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE \A depth \in Nat:
                  IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                    initialContext, candidate, position, depth)
                    ~> IndexedHistoricalStage6CompletionIoDrainGoal(
                         initialContext, candidate, position)
    <2>1. \A depth \in Nat:
             IndexedHistoricalStage6CompletionIoBlockedAtDepth(
               initialContext, candidate, position, depth)
               ~> IndexedHistoricalStage6CompletionIoProgress(
                    initialContext, candidate, position, depth)
      BY <1>1,
         IndexedChainSpecHistoricalStage6CompletionIoOneStep
    <2> QED BY <2>1, NatLessThanWellFounded,
         WellFoundedLeadsTo
         DEF IndexedHistoricalStage6CompletionIoProgress,
             IndexedHistoricalStage6CompletionIoDrainGoal,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6CompletionIoProgress
  <1> QED BY <1>1

(***************************************************************************
Indexed Stage-6 Completion ready-owner handoff.

At zero physical depth, the selected Completion is a protected Stage-4
owner.  The already-closed indexed Stage-4 leaf advances that exact owner;
the one-height safety bridge then opens the blocked causal head.
***************************************************************************)

IndexedHistoricalStage6CompletionGoal(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionGoal(candidate, position)

IndexedHistoricalStage6CompletionReadyBlocked(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionReadyBlocked(
      candidate, position)

IndexedHistoricalStage6CompletionReadyWitnessBlocked(
    initialContext, candidate, position,
    readyCandidate, readyPosition) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionReadyWitnessBlocked(
      candidate, position, readyCandidate, readyPosition)

THEOREM IndexedHistoricalStage4GoalImpliesRankExit ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ position \in Nat
    /\ IndexedHistoricalTemporalStage4Goal(
         initialContext, candidate, position)
    => IndexedHistoricalTemporalRankProgressExit(
         initialContext, candidate, <<4, position>>)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                position \in Nat,
                IndexedHistoricalTemporalStage4Goal(
                  initialContext, candidate, position)
         PROVE IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<4, position>>)
    <2> QED BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4LeafGoalImpliesRankExit, Isa
         DEF IndexedHistoricalTemporalStage4Goal,
             IndexedHistoricalTemporalRankProgressExit
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalStage4ToRankExit ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate \in
            IndexedHistoricalTransport(initialContext)!AsyncCandidateSet,
          position \in Nat:
         IndexedHistoricalTemporalStage4Source(
           initialContext, candidate, position)
           ~> IndexedHistoricalTemporalRankProgressExit(
                initialContext, candidate, <<4, position>>)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage4Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<4, position>>)
    <2>1. IndexedHistoricalTemporalStage4Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage4Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedChainSpecClosesHistoricalTemporalStage4Leaf
         DEF IndexedHistoricalTemporalStage4LeafProperty
    <2>2. IndexedHistoricalTemporalStage4Goal(
             initialContext, candidate, position)
             => IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<4, position>>)
      BY <1>1, IndexedHistoricalStage4GoalImpliesRankExit
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionReadyWitnessExists ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    IndexedHistoricalStage6CompletionReadyBlocked(
      initialContext, candidate, position)
      => \E readyCandidate \in
              IndexedHistoricalTransport(initialContext)!
                AsyncCandidateSet,
            readyPosition \in Nat:
           IndexedHistoricalStage6CompletionReadyWitnessBlocked(
             initialContext, candidate, position,
             readyCandidate, readyPosition)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedHistoricalStage6CompletionReadyBlocked(
                  initialContext, candidate, position)
         PROVE \E readyCandidate \in
                       IndexedHistoricalTransport(initialContext)!
                         AsyncCandidateSet,
                     readyPosition \in Nat:
                   IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                     initialContext, candidate, position,
                     readyCandidate, readyPosition)
    <2> QED BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionReadyWitnessExists
         DEF IndexedHistoricalStage6CompletionReadyBlocked,
             IndexedHistoricalStage6CompletionReadyWitnessBlocked
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionReadyWitnessUnless ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, readyCandidate, readyPosition:
    /\ IndexedHistoricalStage6CompletionReadyWitnessBlocked(
         initialContext, candidate, position,
         readyCandidate, readyPosition)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalStage6CompletionReadyWitnessBlocked(
             initialContext, candidate, position,
             readyCandidate, readyPosition)'
       \/ IndexedHistoricalStage6CompletionGoal(
            initialContext, candidate, position)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW readyCandidate, NEW readyPosition,
                IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                  initialContext, candidate, position,
                  readyCandidate, readyPosition),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                initialContext, candidate, position,
                readyCandidate, readyPosition)'
           \/ IndexedHistoricalStage6CompletionGoal(
                initialContext, candidate, position)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionReadyWitnessUnless
         DEF IndexedHistoricalStage6CompletionReadyWitnessBlocked,
             IndexedHistoricalStage6CompletionGoal
  <1> QED BY <1>1

THEOREM IndexedChainSpecOpensHistoricalStage6CompletionReadyWitness ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         \A readyCandidate \in
              IndexedHistoricalTransport(initialContext)!
                AsyncCandidateSet,
            readyPosition \in Nat:
           IndexedHistoricalStage6CompletionReadyWitnessBlocked(
             initialContext, candidate, position,
             readyCandidate, readyPosition)
             ~> IndexedHistoricalStage6CompletionGoal(
                  initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW readyCandidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW readyPosition \in Nat
         PROVE IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                 initialContext, candidate, position,
                 readyCandidate, readyPosition)
                 ~>
               IndexedHistoricalStage6CompletionGoal(
                 initialContext, candidate, position)
    <2>1. IndexedHistoricalTemporalStage4Source(
             initialContext, readyCandidate, readyPosition)
             ~> IndexedHistoricalTemporalRankProgressExit(
                  initialContext, readyCandidate,
                  <<4, readyPosition>>)
      BY <1>1,
         IndexedChainSpecClosesHistoricalStage4ToRankExit
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. IndexedHistoricalStage6CompletionReadyWitnessBlocked(
             initialContext, candidate, position,
             readyCandidate, readyPosition)
             /\ [IndexedChainNext]_IndexedChainVars
            => \/ IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                    initialContext, candidate, position,
                    readyCandidate, readyPosition)'
               \/ IndexedHistoricalStage6CompletionGoal(
                    initialContext, candidate, position)'
      BY <1>1,
         IndexedHistoricalStage6CompletionReadyWitnessUnless
    <2>4. IndexedHistoricalStage6CompletionReadyWitnessBlocked(
             initialContext, candidate, position,
             readyCandidate, readyPosition)
            => /\ IndexedHistoricalTemporalStage4Source(
                     initialContext, readyCandidate, readyPosition)
               /\ ~IndexedHistoricalTemporalRankProgressExit(
                    initialContext, readyCandidate,
                    <<4, readyPosition>>)
      BY Isa
         DEF IndexedHistoricalStage6CompletionReadyWitnessBlocked,
             IndexedHistoricalTemporalStage4Source,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6CompletionReadyWitnessBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!ServiceRankLess
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecOpensHistoricalStage6CompletionReady ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalStage6CompletionReadyBlocked(
           initialContext, candidate, position)
           ~> IndexedHistoricalStage6CompletionGoal(
                initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalStage6CompletionReadyBlocked(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalStage6CompletionGoal(
                 initialContext, candidate, position)
    <2>1. IndexedHistoricalStage6CompletionReadyBlocked(
             initialContext, candidate, position)
             => \E readyCandidate \in
                    IndexedHistoricalTransport(initialContext)!
                      AsyncCandidateSet,
                  readyPosition \in Nat:
                  IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                    initialContext, candidate, position,
                    readyCandidate, readyPosition)
      BY IndexedHistoricalStage6CompletionReadyWitnessExists
    <2>2. \A readyCandidate \in
               IndexedHistoricalTransport(initialContext)!
                 AsyncCandidateSet,
             readyPosition \in Nat:
             IndexedHistoricalStage6CompletionReadyWitnessBlocked(
               initialContext, candidate, position,
               readyCandidate, readyPosition)
               ~> IndexedHistoricalStage6CompletionGoal(
                    initialContext, candidate, position)
      BY <1>1,
         IndexedChainSpecOpensHistoricalStage6CompletionReadyWitness
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecOpensHistoricalStage6CompletionCapacity ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionCapacityBlocked(
             candidate, position)
           ~> IndexedHistoricalStage6CompletionGoal(
                initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6CompletionCapacityBlocked(
                   candidate, position)
                 ~>
               IndexedHistoricalStage6CompletionGoal(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. []IndexedHistoricalTransport(initialContext)!
             AsyncTypeInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType, PTL
         DEF IndexedHistoricalTemporalSupportAt
    <2>3. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6CompletionCapacityBlocked(
               candidate, position)
             ~> (IndexedHistoricalStage6CompletionGoal(
                   initialContext, candidate, position)
                  \/ IndexedHistoricalStage6CompletionReadyBlocked(
                       initialContext, candidate, position))
      <3>1. (IndexedHistoricalTransport(initialContext)!
                HistoricalTemporalStage6CompletionCapacityBlocked(
                  candidate, position)
               /\ IndexedHistoricalTransport(initialContext)!
                    AsyncIoQueueDepth(candidate.node) > 0)
               ~> IndexedHistoricalStage6CompletionIoDrainGoal(
                    initialContext, candidate, position)
        <4>1. IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6CompletionCapacityBlocked(
                   candidate, position)
                 /\ IndexedHistoricalTransport(initialContext)!
                      AsyncIoQueueDepth(candidate.node) > 0
                ~> IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                     initialContext, candidate, position,
                     IndexedHistoricalTransport(initialContext)!
                       AsyncIoQueueDepth(candidate.node))
          BY <2>2, PTL
             DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
                 IndexedHistoricalTransport!
                   HistoricalTemporalStage6CompletionIoBlockedAtDepth
        <4>2. \A depth \in Nat:
                 IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                   initialContext, candidate, position, depth)
                   ~> IndexedHistoricalStage6CompletionIoDrainGoal(
                        initialContext, candidate, position)
          BY <1>1,
             IndexedChainSpecDrainsHistoricalStage6CompletionIo
        <4> QED BY <4>1, <4>2, PTL
      <3>2. IndexedHistoricalStage6CompletionIoDrainGoal(
               initialContext, candidate, position)
              => \/ IndexedHistoricalStage6CompletionGoal(
                       initialContext, candidate, position)
                 \/ IndexedHistoricalStage6CompletionReadyBlocked(
                      initialContext, candidate, position)
        BY Isa
           DEF IndexedHistoricalStage6CompletionIoDrainGoal,
               IndexedHistoricalStage6CompletionGoal,
               IndexedHistoricalStage6CompletionReadyBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionIoDrainGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionReadyBlocked
      <3>3. /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalTemporalStage6CompletionCapacityBlocked(
                      candidate, position)
              /\ IndexedHistoricalTransport(initialContext)!
                   AsyncIoQueueDepth(candidate.node) = 0
             => IndexedHistoricalStage6CompletionReadyBlocked(
                  initialContext, candidate, position)
        BY DEF IndexedHistoricalStage6CompletionReadyBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionReadyBlocked
      <3> QED BY <3>1, <3>2, <3>3, PTL
    <2>4. IndexedHistoricalStage6CompletionReadyBlocked(
             initialContext, candidate, position)
             ~> IndexedHistoricalStage6CompletionGoal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedChainSpecOpensHistoricalStage6CompletionReady
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

(***************************************************************************
Indexed historical Stage-6 leaf.
***************************************************************************)

IndexedHistoricalTemporalStage6Source(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalProtectedOwnedAtServiceRank(
      candidate, <<6, position>>)

IndexedHistoricalTemporalStage6Goal(
    initialContext, candidate, position) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedServiceOwnershipExit(candidate)
  \/ \E lower \in SetLessThan(
       <<6, position>>,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankCarrier):
       IndexedHistoricalTransport(initialContext)!
         HistoricalProtectedOwnedAtServiceRank(candidate, lower)

IndexedHistoricalTemporalStage6LeafProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate \in IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
     position \in Nat:
    IndexedHistoricalTemporalStage6Source(
      initialContext, candidate, position)
      ~> IndexedHistoricalTemporalStage6Goal(
           initialContext, candidate, position)

THEOREM IndexedChainSpecClosesHistoricalTemporalStage6Leaf ==
  IndexedChainSpec
    => IndexedHistoricalTemporalStage6LeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage6Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage6Goal(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalStage6Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage6Pending(
                     candidate, position)
      BY <2>1, PTL
         DEF IndexedHistoricalTemporalStage6Source,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6Pending
    <2>3. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6Pending(candidate, position)
             ~> IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage6PreAdmissionGoal(
                     candidate, position)
      BY <1>1,
         IndexedChainSpecClosesHistoricalStage6PreAdmission
    <2>4. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6OwedCausalReady(
               candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                   initialContext, candidate, <<6, position>>)
      BY <1>1, IndexedChainSpecClosesHistoricalStage6Owed
    <2>5. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6NonCompletionCapacityBlocked(
               candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                   initialContext, candidate, <<6, position>>)
      <3>1. IndexedHistoricalTransport(initialContext)!
               HistoricalTemporalStage6NonCompletionCapacityBlocked(
                 candidate, position)
               ~> IndexedHistoricalTransport(initialContext)!
                     HistoricalTemporalStage6NonCompletionGoal(
                       candidate, position)
        BY <1>1,
           IndexedChainSpecClosesHistoricalStage6NonCompletion
      <3> QED BY <3>1, <2>4, PTL
           DEF IndexedHistoricalTransport!
                 HistoricalTemporalStage6NonCompletionGoal
    <2>6. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6CompletionCapacityBlocked(
               candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                   initialContext, candidate, <<6, position>>)
      <3>1. IndexedHistoricalTransport(initialContext)!
               HistoricalTemporalStage6CompletionCapacityBlocked(
                 candidate, position)
               ~> IndexedHistoricalStage6CompletionGoal(
                     initialContext, candidate, position)
        BY <1>1,
           IndexedChainSpecOpensHistoricalStage6CompletionCapacity
      <3> QED BY <3>1, <2>4, PTL
           DEF IndexedHistoricalStage6CompletionGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionGoal
    <2>7. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6PreAdmissionGoal(
               candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                   initialContext, candidate, <<6, position>>)
      BY <2>4, <2>5, <2>6, PTL
         DEF IndexedHistoricalTransport!
               HistoricalTemporalStage6PreAdmissionGoal
    <2>8. IndexedHistoricalTemporalSupportAt(initialContext)
             /\ IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<6, position>>)
             => IndexedHistoricalTemporalStage6Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRankExitHasWellFoundedSuccessor, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTemporalStage6Goal,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!OwnedServiceRankCarrier
    <2>9. IndexedHistoricalTemporalRankProgressExit(
             initialContext, candidate, <<6, position>>)
             ~> IndexedHistoricalTemporalStage6Goal(
                  initialContext, candidate, position)
      BY <2>1, <2>8, PTL
    <2> QED BY <2>2, <2>3, <2>7, <2>9, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalTemporalStage6LeafProperty

(***************************************************************************
Indexed Stage-2 post-deferred support.

A Busy Stage-2 owner is witnessed by a Completion already in stages 3..6.
The four indexed leaves above therefore close the exact restricted product
rank before the deferred handoff itself is considered.
***************************************************************************)

IndexedHistoricalTemporalPostDeferredExit(initialContext, candidate) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalPostDeferredExit(candidate)

IndexedHistoricalTemporalPostDeferredAtRank(
    initialContext, candidate, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalPostDeferredAtRank(candidate, rank)

THEOREM IndexedHistoricalStageLeafGoalImpliesStrictRank ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, stage, position:
    /\ stage \in 3..6
    /\ position \in Nat
    /\ (IndexedHistoricalTransport(initialContext)!
          HistoricalProtectedServiceOwnershipExit(candidate)
         \/ \E lower \in SetLessThan(
              <<stage, position>>,
              IndexedHistoricalTransport(initialContext)!
                OwnedServiceRankOrdering,
              IndexedHistoricalTransport(initialContext)!
                OwnedServiceRankCarrier):
              IndexedHistoricalTransport(initialContext)!
                HistoricalProtectedOwnedAtServiceRank(
                  candidate, lower))
    => \/ IndexedHistoricalTransport(initialContext)!
            HistoricalProtectedServiceOwnershipExit(candidate)
       \/ IndexedHistoricalTransport(initialContext)!
            ServiceRankLess(
              IndexedHistoricalTransport(initialContext)!
                CandidateServiceRank(candidate),
              <<stage, position>>)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW stage, NEW position,
                stage \in 3..6,
                position \in Nat,
                (IndexedHistoricalTransport(initialContext)!
                   HistoricalProtectedServiceOwnershipExit(candidate)
                  \/ \E lower \in SetLessThan(
                       <<stage, position>>,
                       IndexedHistoricalTransport(initialContext)!
                         OwnedServiceRankOrdering,
                       IndexedHistoricalTransport(initialContext)!
                         OwnedServiceRankCarrier):
                       IndexedHistoricalTransport(initialContext)!
                         HistoricalProtectedOwnedAtServiceRank(
                           candidate, lower))
         PROVE
           \/ IndexedHistoricalTransport(initialContext)!
                HistoricalProtectedServiceOwnershipExit(candidate)
           \/ IndexedHistoricalTransport(initialContext)!
                ServiceRankLess(
                  IndexedHistoricalTransport(initialContext)!
                    CandidateServiceRank(candidate),
                  <<stage, position>>)
    <2> QED BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStageLeafGoalImpliesStrictRank
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalPostDeferredRankStep ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate \in
            IndexedHistoricalTransport(initialContext)!AsyncCandidateSet,
          stage \in 3..6, position \in Nat:
         IndexedHistoricalTransport(initialContext)!
           HistoricalProtectedOwnedAtServiceRank(
             candidate, <<stage, position>>)
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalProtectedServiceOwnershipExit(candidate)
                \/ IndexedHistoricalTransport(initialContext)!
                     ServiceRankLess(
                       IndexedHistoricalTransport(initialContext)!
                         CandidateServiceRank(candidate),
                       <<stage, position>>))
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW stage \in 3..6, NEW position \in Nat
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalProtectedOwnedAtServiceRank(
                   candidate, <<stage, position>>)
                 ~>
               (IndexedHistoricalTransport(initialContext)!
                  HistoricalProtectedServiceOwnershipExit(candidate)
                 \/ IndexedHistoricalTransport(initialContext)!
                      ServiceRankLess(
                        IndexedHistoricalTransport(initialContext)!
                          CandidateServiceRank(candidate),
                        <<stage, position>>))
    <2> DEFINE Goal ==
           IndexedHistoricalTransport(initialContext)!
             HistoricalProtectedServiceOwnershipExit(candidate)
             \/ \E lower \in SetLessThan(
                  <<stage, position>>,
                  IndexedHistoricalTransport(initialContext)!
                    OwnedServiceRankOrdering,
                  IndexedHistoricalTransport(initialContext)!
                    OwnedServiceRankCarrier):
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalProtectedOwnedAtServiceRank(
                      candidate, lower)
    <2>1. CASE stage = 3
      BY <1>1, <2>1,
         IndexedChainSpecClosesHistoricalTemporalStage3Leaf
         DEF Goal, IndexedHistoricalTemporalStage3LeafProperty,
             IndexedHistoricalTemporalStage3Source,
             IndexedHistoricalTemporalStage3Goal
    <2>2. CASE stage = 4
      BY <1>1, <2>2,
         IndexedChainSpecClosesHistoricalTemporalStage4Leaf
         DEF Goal, IndexedHistoricalTemporalStage4LeafProperty,
             IndexedHistoricalTemporalStage4Source,
             IndexedHistoricalTemporalStage4Goal
    <2>3. CASE stage = 5
      BY <1>1, <2>3,
         IndexedChainSpecClosesHistoricalTemporalStage5Leaf
         DEF Goal, IndexedHistoricalTemporalStage5LeafProperty,
             IndexedHistoricalTemporalStage5Source,
             IndexedHistoricalTemporalStage5Goal
    <2>4. CASE stage = 6
      BY <1>1, <2>4,
         IndexedChainSpecClosesHistoricalTemporalStage6Leaf
         DEF Goal, IndexedHistoricalTemporalStage6LeafProperty,
             IndexedHistoricalTemporalStage6Source,
             IndexedHistoricalTemporalStage6Goal
    <2>5. IndexedHistoricalTransport(initialContext)!
             HistoricalProtectedOwnedAtServiceRank(
               candidate, <<stage, position>>)
             ~> Goal
      BY <1>1, <2>1, <2>2, <2>3, <2>4, Isa
    <2>6. Goal
             => (IndexedHistoricalTransport(initialContext)!
                   HistoricalProtectedServiceOwnershipExit(candidate)
                  \/ IndexedHistoricalTransport(initialContext)!
                       ServiceRankLess(
                         IndexedHistoricalTransport(initialContext)!
                           CandidateServiceRank(candidate),
                         <<stage, position>>))
      BY <1>1, IndexedHistoricalStageLeafGoalImpliesStrictRank
         DEF Goal
    <2> QED BY <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecConvergesHistoricalPostDeferredRank ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords, candidate:
         (IndexedHistoricalTransport(initialContext)!gst
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalProtectedCandidateOwned(candidate)
           /\ IndexedHistoricalTransport(initialContext)!
                CandidateServiceRank(candidate)[1] \in 3..6)
           ~> IndexedHistoricalTemporalPostDeferredExit(
                initialContext, candidate)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate
         PROVE (IndexedHistoricalTransport(initialContext)!gst
                 /\ IndexedHistoricalTransport(initialContext)!
                      HistoricalProtectedCandidateOwned(candidate)
                 /\ IndexedHistoricalTransport(initialContext)!
                      CandidateServiceRank(candidate)[1] \in 3..6)
                 ~>
               IndexedHistoricalTemporalPostDeferredExit(
                 initialContext, candidate)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. ASSUME NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    PostDeferredServiceRankCarrier
           PROVE IndexedHistoricalTemporalPostDeferredAtRank(
                   initialContext, candidate, rank)
                   ~> (IndexedHistoricalTemporalPostDeferredExit(
                         initialContext, candidate)
                        \/ \E lower \in SetLessThan(
                             rank,
                             IndexedHistoricalTransport(initialContext)!
                               PostDeferredServiceRankOrdering,
                             IndexedHistoricalTransport(initialContext)!
                               PostDeferredServiceRankCarrier):
                             IndexedHistoricalTemporalPostDeferredAtRank(
                               initialContext, candidate, lower))
      <3>1. PICK stage \in 3..6, position \in Nat:
               rank = <<stage, position>>
        BY <2>2
           DEF IndexedHistoricalTransport!
                 PostDeferredServiceRankCarrier
      <3>2. IndexedHistoricalTemporalPostDeferredAtRank(
               initialContext, candidate, rank)
               ~> (IndexedHistoricalTransport(initialContext)!
                     HistoricalProtectedServiceOwnershipExit(candidate)
                    \/ IndexedHistoricalTransport(initialContext)!
                         ServiceRankLess(
                           IndexedHistoricalTransport(initialContext)!
                             CandidateServiceRank(candidate),
                           rank))
        BY <1>1, <3>1,
           IndexedChainSpecClosesHistoricalPostDeferredRankStep
           DEF IndexedHistoricalTemporalPostDeferredAtRank,
               IndexedHistoricalTransport!
                 HistoricalTemporalPostDeferredAtRank
      <3>3. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalPostDeferredAtRank(
                    initialContext, candidate, rank)
               /\ IndexedHistoricalTransport(initialContext)!
                    ServiceRankLess(
                      IndexedHistoricalTransport(initialContext)!
                        CandidateServiceRank(candidate),
                      rank)
              => \/ IndexedHistoricalTemporalPostDeferredExit(
                      initialContext, candidate)
                 \/ \E lower \in SetLessThan(
                      rank,
                      IndexedHistoricalTransport(initialContext)!
                        PostDeferredServiceRankOrdering,
                      IndexedHistoricalTransport(initialContext)!
                        PostDeferredServiceRankCarrier):
                      IndexedHistoricalTemporalPostDeferredAtRank(
                        initialContext, candidate, lower)
        BY <2>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeProjectsAsyncType,
           IndexedHistoricalTransport(initialContext)!
             ScheduledCandidateServiceRankInCarrier,
           IndexedHistoricalTransport(initialContext)!
             OwnedServiceRankOrderingMatchesLess, Isa
           DEF IndexedHistoricalTemporalSupportAt,
               IndexedHistoricalTemporalPostDeferredExit,
               IndexedHistoricalTemporalPostDeferredAtRank,
               IndexedHistoricalTransport!
                 HistoricalTemporalPostDeferredExit,
               IndexedHistoricalTransport!
                 HistoricalTemporalPostDeferredAtRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned,
               IndexedHistoricalTransport!ProtectedCandidateOwned,
               IndexedHistoricalTransport!CandidateScheduled,
               IndexedHistoricalTransport!CandidateServiceRank,
               IndexedHistoricalTransport!ServiceRankLess,
               IndexedHistoricalTransport!
                 PostDeferredServiceRankOrdering,
               IndexedHistoricalTransport!
                 PostDeferredServiceRankCarrier,
               SetLessThan
      <3> QED BY <2>1, <3>2, <3>3, PTL
           DEF IndexedHistoricalTemporalPostDeferredExit
    <2>3. \A rank \in
               IndexedHistoricalTransport(initialContext)!
                 PostDeferredServiceRankCarrier:
             IndexedHistoricalTemporalPostDeferredAtRank(
               initialContext, candidate, rank)
               ~> IndexedHistoricalTemporalPostDeferredExit(
                    initialContext, candidate)
      BY <2>2,
         IndexedHistoricalTransport(initialContext)!
           PostDeferredServiceRankOrderingWellFoundedObligation,
         WellFoundedLeadsTo
    <2>4. (IndexedHistoricalTransport(initialContext)!gst
             /\ IndexedHistoricalTransport(initialContext)!
                  HistoricalProtectedCandidateOwned(candidate)
             /\ IndexedHistoricalTransport(initialContext)!
                  CandidateServiceRank(candidate)[1] \in 3..6)
             ~> \E rank \in
                   IndexedHistoricalTransport(initialContext)!
                     PostDeferredServiceRankCarrier:
                  IndexedHistoricalTemporalPostDeferredAtRank(
                    initialContext, candidate, rank)
      BY Isa, PTL
         DEF IndexedHistoricalTemporalPostDeferredAtRank,
             IndexedHistoricalTransport!
               HistoricalTemporalPostDeferredAtRank,
             IndexedHistoricalTransport!
               PostDeferredServiceRankCarrier
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

(***************************************************************************
Indexed Stage-2 Busy-phase descent.
***************************************************************************)

IndexedHistoricalTemporalStage2Owned(initialContext, candidate) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2Owned(candidate)

IndexedHistoricalTemporalBusyCompletionWitness(
    initialContext, target, witness) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalBusyCompletionWitness(target, witness)

IndexedHistoricalTemporalStage2BusyPhaseGoal(
    initialContext, target, phase) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2BusyPhaseGoal(target, phase)

IndexedHistoricalTemporalStage2BusyWitnessBlocked(
    initialContext, target, witness, phase) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2BusyWitnessBlocked(
      target, witness, phase)

THEOREM IndexedHistoricalBusyWitnessHasPostDeferredRank ==
  \A initialContext \in AdmissibleContextRecords,
     target, witness:
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalTemporalBusyCompletionWitness(
         initialContext, target, witness)
    => /\ IndexedHistoricalTransport(initialContext)!
            HistoricalProtectedCandidateOwned(witness)
       /\ IndexedHistoricalTransport(initialContext)!
            CandidateServiceRank(witness)
            \in IndexedHistoricalTransport(initialContext)!
                 PostDeferredServiceRankCarrier
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW target, NEW witness,
                IndexedHistoricalTemporalSupportAt(initialContext),
                IndexedHistoricalTemporalBusyCompletionWitness(
                  initialContext, target, witness)
         PROVE
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalProtectedCandidateOwned(witness)
           /\ IndexedHistoricalTransport(initialContext)!
                CandidateServiceRank(witness)
                \in IndexedHistoricalTransport(initialContext)!
                     PostDeferredServiceRankCarrier
    <2> QED BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalBusyWitnessHasPostDeferredRank
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalBusyCompletionWitness
  <1> QED BY <1>1

THEOREM IndexedHistoricalBusyWitnessPersistsOrPhaseDrops ==
  \A initialContext \in AdmissibleContextRecords,
     target, witness:
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalTemporalBusyCompletionWitness(
         initialContext, target, witness)
    /\ [IndexedChainNext]_IndexedChainVars
    /\ ~IndexedHistoricalTransport(initialContext)!
          HistoricalProtectedServiceOwnershipExit(target)'
    /\ IndexedHistoricalTransport(initialContext)!
         BusyPhaseRank(target.node)'
         >= IndexedHistoricalTransport(initialContext)!
              BusyPhaseRank(target.node)
    => IndexedHistoricalTemporalBusyCompletionWitness(
         initialContext, target, witness)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW target, NEW witness,
                IndexedHistoricalTemporalSupportAt(initialContext),
                IndexedHistoricalTemporalBusyCompletionWitness(
                  initialContext, target, witness),
                [IndexedChainNext]_IndexedChainVars,
                ~IndexedHistoricalTransport(initialContext)!
                   HistoricalProtectedServiceOwnershipExit(target)',
                IndexedHistoricalTransport(initialContext)!
                  BusyPhaseRank(target.node)'
                  >= IndexedHistoricalTransport(initialContext)!
                       BusyPhaseRank(target.node)
         PROVE IndexedHistoricalTemporalBusyCompletionWitness(
                 initialContext, target, witness)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalBusyWitnessOwnershipPersists
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalBusyCompletionWitness
  <1> QED BY <1>1

THEOREM IndexedHistoricalBusyPhaseCannotIncrease ==
  \A initialContext \in AdmissibleContextRecords, target:
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalTemporalStage2Owned(initialContext, target)
    /\ IndexedHistoricalTransport(initialContext)!
         BusyPhaseRank(target.node) \in 1..2
    /\ [IndexedChainNext]_IndexedChainVars
    /\ ~IndexedHistoricalTransport(initialContext)!
          HistoricalProtectedServiceOwnershipExit(target)'
    => IndexedHistoricalTransport(initialContext)!
         BusyPhaseRank(target.node)'
         <= IndexedHistoricalTransport(initialContext)!
              BusyPhaseRank(target.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW target,
                IndexedHistoricalTemporalSupportAt(initialContext),
                IndexedHistoricalTemporalStage2Owned(
                  initialContext, target),
                IndexedHistoricalTransport(initialContext)!
                  BusyPhaseRank(target.node) \in 1..2,
                [IndexedChainNext]_IndexedChainVars,
                ~IndexedHistoricalTransport(initialContext)!
                   HistoricalProtectedServiceOwnershipExit(target)'
         PROVE IndexedHistoricalTransport(initialContext)!
                 BusyPhaseRank(target.node)'
                 <= IndexedHistoricalTransport(initialContext)!
                      BusyPhaseRank(target.node)
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalBusyPhaseCannotIncrease
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalStage2Owned
  <1> QED BY <1>1

THEOREM IndexedChainSpecDescendsHistoricalStage2BusyPhase ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords, target:
         \A phase \in 1..2:
           (IndexedHistoricalTemporalStage2Owned(
              initialContext, target)
             /\ IndexedHistoricalTransport(initialContext)!
                  BusyPhaseRank(target.node) = phase)
             ~> IndexedHistoricalTemporalStage2BusyPhaseGoal(
                  initialContext, target, phase)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW target, NEW phase \in 1..2
         PROVE (IndexedHistoricalTemporalStage2Owned(
                  initialContext, target)
                 /\ IndexedHistoricalTransport(initialContext)!
                      BusyPhaseRank(target.node) = phase)
                 ~>
               IndexedHistoricalTemporalStage2BusyPhaseGoal(
                 initialContext, target, phase)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2Owned(
                    initialContext, target)
               /\ IndexedHistoricalTransport(initialContext)!
                    BusyPhaseRank(target.node) = phase
              => \E witness \in
                    IndexedHistoricalTransport(initialContext)!
                      AsyncCandidateSet:
                   IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                     initialContext, target, witness, phase)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           BusyPhaseOwnerPartitionObligation, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalStage2Owned,
             IndexedHistoricalTemporalStage2BusyWitnessBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2BusyWitnessBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalBusyCompletionWitness,
             IndexedHistoricalTransport!Stage2BusyKernelInvariant,
             IndexedHistoricalTransport!
               AsyncProgressOwnershipInvariant,
             IndexedHistoricalTransport!
               BusyCompletionWitnessInvariant
    <2>3. ASSUME NEW witness \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet
           PROVE IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                   initialContext, target, witness, phase)
                   ~> IndexedHistoricalTemporalStage2BusyPhaseGoal(
                        initialContext, target, phase)
      <3>1. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                    initialContext, target, witness, phase)
              => /\ IndexedHistoricalTransport(initialContext)!
                      HistoricalProtectedCandidateOwned(witness)
                 /\ IndexedHistoricalTransport(initialContext)!
                      CandidateServiceRank(witness)[1] \in 3..6
        BY <2>1,
           IndexedHistoricalBusyWitnessHasPostDeferredRank
           DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTemporalBusyCompletionWitness,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTransport!
                 PostDeferredServiceRankCarrier
      <3>2. (IndexedHistoricalTransport(initialContext)!gst
               /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalProtectedCandidateOwned(witness)
               /\ IndexedHistoricalTransport(initialContext)!
                    CandidateServiceRank(witness)[1] \in 3..6)
              ~> IndexedHistoricalTemporalPostDeferredExit(
                   initialContext, witness)
        BY <1>1,
           IndexedChainSpecConvergesHistoricalPostDeferredRank
      <3>3. IndexedHistoricalTemporalStage2BusyWitnessBlocked(
               initialContext, target, witness, phase)
               ~> IndexedHistoricalTemporalPostDeferredExit(
                    initialContext, witness)
        BY <2>1, <3>1, <3>2, PTL
           DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalBusyCompletionWitness,
               IndexedHistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2Owned
      <3>4. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                    initialContext, target, witness, phase)
               /\ [IndexedChainNext]_IndexedChainVars
              => \/ IndexedHistoricalTemporalStage2BusyPhaseGoal(
                      initialContext, target, phase)'
                 \/ IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                      initialContext, target, witness, phase)'
        <4>1. ASSUME
                 IndexedHistoricalTemporalSupportAt(initialContext),
                 IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                   initialContext, target, witness, phase),
                 [IndexedChainNext]_IndexedChainVars
               PROVE
                 \/ IndexedHistoricalTemporalStage2BusyPhaseGoal(
                      initialContext, target, phase)'
                 \/ IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                      initialContext, target, witness, phase)'
          <5>1. CASE IndexedHistoricalTemporalStage2BusyPhaseGoal(
                        initialContext, target, phase)'
            BY <5>1
          <5>2. CASE ~IndexedHistoricalTemporalStage2BusyPhaseGoal(
                        initialContext, target, phase)'
            <6>1. /\ ~IndexedHistoricalTransport(initialContext)!
                         HistoricalProtectedServiceOwnershipExit(target)'
                   /\ IndexedHistoricalTransport(initialContext)!
                        BusyPhaseRank(target.node)' >= phase
              BY <5>2
                 DEF IndexedHistoricalTemporalStage2BusyPhaseGoal,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2BusyPhaseGoal
            <6>2. IndexedHistoricalTransport(initialContext)!
                     BusyPhaseRank(target.node)'
                     <= IndexedHistoricalTransport(initialContext)!
                          BusyPhaseRank(target.node)
              BY <4>1, <6>1,
                 IndexedHistoricalBusyPhaseCannotIncrease
                 DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2BusyWitnessBlocked,
                     IndexedHistoricalTransport!
                       HistoricalTemporalBusyCompletionWitness,
                     IndexedHistoricalTemporalStage2Owned,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2Owned
            <6>3. IndexedHistoricalTransport(initialContext)!
                     BusyPhaseRank(target.node)' = phase
              BY <4>1, <6>1, <6>2
                 DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2BusyWitnessBlocked
            <6>4. IndexedHistoricalTemporalBusyCompletionWitness(
                     initialContext, target, witness)'
              BY <4>1, <6>1,
                 IndexedHistoricalBusyWitnessPersistsOrPhaseDrops
                 DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2BusyWitnessBlocked
            <6> QED BY <6>3, <6>4
                 DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2BusyWitnessBlocked
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3>5. [](IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                   initialContext, target, witness, phase)
                 /\ IndexedHistoricalTemporalPostDeferredExit(
                      initialContext, witness)
                => FALSE)
        BY <2>1,
           IndexedHistoricalBusyWitnessHasPostDeferredRank, PTL
           DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTemporalBusyCompletionWitness,
               IndexedHistoricalTemporalPostDeferredExit,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalPostDeferredExit,
               IndexedHistoricalTransport!
                 PostDeferredServiceRankCarrier
      <3> QED BY <2>1, <3>3, <3>4, <3>5, PTL
    <2>4. (IndexedHistoricalTemporalStage2Owned(
             initialContext, target)
             /\ IndexedHistoricalTransport(initialContext)!
                  BusyPhaseRank(target.node) = phase)
            ~> \E witness \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet:
                 IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                   initialContext, target, witness, phase)
      BY <2>1, <2>2, PTL
    <2> QED BY <2>3, <2>4, PTL
         DEF IndexedHistoricalTemporalStage2BusyPhaseGoal
  <1> QED BY <1>1

IndexedHistoricalTemporalStage2BusyAtPhase(
    initialContext, target, phase) ==
  /\ IndexedHistoricalTemporalStage2Owned(initialContext, target)
  /\ IndexedHistoricalTransport(initialContext)!
       BusyPhaseRank(target.node) = phase

IndexedHistoricalTemporalStage2BusyTerminationGoal(
    initialContext, target) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2BusyTerminationGoal(target)

THEOREM IndexedChainSpecTerminatesHistoricalStage2Busy ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords, target:
         (IndexedHistoricalTemporalStage2Owned(initialContext, target)
           /\ ~IndexedHistoricalTransport(initialContext)!
                 NodeIdle(target.node))
           ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                initialContext, target)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW target
         PROVE (IndexedHistoricalTemporalStage2Owned(
                  initialContext, target)
                 /\ ~IndexedHistoricalTransport(initialContext)!
                       NodeIdle(target.node))
                 ~>
               IndexedHistoricalTemporalStage2BusyTerminationGoal(
                 initialContext, target)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalStage2BusyAtPhase(
             initialContext, target, 1)
             ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                  initialContext, target)
      <3>1. IndexedHistoricalTemporalStage2BusyAtPhase(
               initialContext, target, 1)
               ~> IndexedHistoricalTemporalStage2BusyPhaseGoal(
                    initialContext, target, 1)
        BY <1>1,
           IndexedChainSpecDescendsHistoricalStage2BusyPhase
           DEF IndexedHistoricalTemporalStage2BusyAtPhase
      <3>2. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2BusyPhaseGoal(
                    initialContext, target, 1)
              => IndexedHistoricalTemporalStage2BusyTerminationGoal(
                   initialContext, target)
        BY <1>1,
           IndexedHistoricalTransport(initialContext)!
             BusyPhaseOwnerPartitionObligation, Isa
           DEF IndexedHistoricalTemporalSupportAt,
               IndexedHistoricalTemporalStage2BusyPhaseGoal,
               IndexedHistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyPhaseGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTransport!Stage2BusyKernelInvariant,
               IndexedHistoricalTransport!BusyPhaseCarrier
      <3> QED BY <2>1, <3>1, <3>2, PTL
    <2>3. IndexedHistoricalTemporalStage2BusyAtPhase(
             initialContext, target, 2)
             ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                  initialContext, target)
      <3>1. IndexedHistoricalTemporalStage2BusyAtPhase(
               initialContext, target, 2)
               ~> IndexedHistoricalTemporalStage2BusyPhaseGoal(
                    initialContext, target, 2)
        BY <1>1,
           IndexedChainSpecDescendsHistoricalStage2BusyPhase
           DEF IndexedHistoricalTemporalStage2BusyAtPhase
      <3>2. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2BusyPhaseGoal(
                    initialContext, target, 2)
              => \/ IndexedHistoricalTemporalStage2BusyTerminationGoal(
                      initialContext, target)
                 \/ IndexedHistoricalTemporalStage2BusyAtPhase(
                      initialContext, target, 1)
        BY <1>1,
           IndexedHistoricalTransport(initialContext)!
             BusyPhaseOwnerPartitionObligation, Isa
           DEF IndexedHistoricalTemporalSupportAt,
               IndexedHistoricalTemporalStage2BusyPhaseGoal,
               IndexedHistoricalTemporalStage2BusyAtPhase,
               IndexedHistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyPhaseGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTransport!Stage2BusyKernelInvariant,
               IndexedHistoricalTransport!BusyPhaseCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalProtectedServiceOwnershipExit
      <3> QED BY <2>2, <3>1, <3>2, PTL
    <2>4. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2Owned(
                    initialContext, target)
               /\ ~IndexedHistoricalTransport(initialContext)!
                     NodeIdle(target.node)
              => \/ IndexedHistoricalTemporalStage2BusyAtPhase(
                      initialContext, target, 1)
                 \/ IndexedHistoricalTemporalStage2BusyAtPhase(
                      initialContext, target, 2)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           BusyPhaseOwnerPartitionObligation, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalStage2BusyAtPhase,
             IndexedHistoricalTemporalStage2Owned,
             IndexedHistoricalTransport!Stage2BusyKernelInvariant,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2Owned
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF IndexedHistoricalTemporalStage2BusyTerminationGoal
  <1> QED BY <1>1

(***************************************************************************
Indexed Stage-2 exact deferred handoff.

The handoff token, three-class cursor, and retry lifecycle are source safety
facts.  The only temporal consumer below is the exact joined historical
runner already bridged to its product weak-fairness clause.
***************************************************************************)

IndexedHistoricalTemporalStage2ExactIdleRetryPending(
    initialContext, candidate) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2ExactIdleRetryPending(candidate)

IndexedHistoricalTemporalStage2ExactIdleRetrySelected(
    initialContext, candidate) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2ExactIdleRetrySelected(candidate)

IndexedHistoricalTemporalStage2HandoffProgressExit(
    initialContext, candidate) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2HandoffProgressExit(candidate)

THEOREM IndexedChainSpecExitsHistoricalStage2IdleHandoff ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords, candidate:
         IndexedHistoricalTemporalStage2ExactIdleRetryPending(
           initialContext, candidate)
           ~> IndexedHistoricalTemporalStage2HandoffProgressExit(
                initialContext, candidate)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate
         PROVE IndexedHistoricalTemporalStage2ExactIdleRetryPending(
                 initialContext, candidate)
                 ~>
               IndexedHistoricalTemporalStage2HandoffProgressExit(
                 initialContext, candidate)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>3. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>4. IndexedHistoricalTemporalStage2ExactIdleRetryPending(
             initialContext, candidate)
             => IndexedHistoricalTemporalCandidateRunnerPending(
                  initialContext, candidate)
      BY <2>1
         DEF IndexedHistoricalTemporalStage2ExactIdleRetryPending,
             IndexedHistoricalTemporalCandidateRunnerPending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2ExactIdleRetryPending,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2Owned,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned
    <2>5. IndexedCompositionInvariant
             /\ IndexedHistoricalTemporalCandidateRunnerPending(
                  initialContext, candidate)
            => ENABLED
                 <<IndexedRunHistoricalRecoveryStep(
                     initialContext, candidate.node)>>_IndexedChainVars
      BY IndexedHistoricalCandidateRunnerEnablesFairOccurrence
    <2>6. CASE candidate.node \in Responsive
      <3>1. WF_IndexedChainVars(
               IndexedRunHistoricalRecoveryStep(
                 initialContext, candidate.node))
        BY <1>1, <2>6
           DEF IndexedChainSpec, IndexedFairness
      <3>2. (IndexedHistoricalTemporalStage2Owned(
               initialContext, candidate)
               /\ ~IndexedHistoricalTransport(initialContext)!
                     NodeIdle(candidate.node))
               ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                    initialContext, candidate)
        BY <1>1,
           IndexedChainSpecTerminatesHistoricalStage2Busy
      <3>3. IndexedHistoricalTemporalStage2ExactIdleRetryPending(
               initialContext, candidate)
               ~> IndexedHistoricalTemporalStage2HandoffProgressExit(
                    initialContext, candidate)
        BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5,
           <2>6, <3>1, <3>2,
           IndexedBracketStepProjectsEveryHistoricalTransportStep,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2BusyRetryClaimsHandoffAction,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2ForeignIdleSkipDropsDistance,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2HandoffDistanceInCarrier,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2ExactIdleRetryDrainConsumes,
           IndexedHistoricalTransport(initialContext)!
             Stage2DeferredHandoffTokenIsInjectiveObligation,
           IndexedHistoricalTransport(initialContext)!
             Stage2SelectedDifferentDeferredClassDropsDistance,
           IndexedHistoricalTransport(initialContext)!
             ReadyRunAuxOrderingIsWellFounded,
           IndexedHistoricalTransport(initialContext)!
             LocalAdmissionStrictlyDecreasesRuntimeReach,
           IndexedHistoricalTransport(initialContext)!
             IngressDrainStrictlyDecreasesRuntimeReach,
           HeadTailProperties, NatLessThanWellFounded,
           IsWellFoundedOnSubset, IsaT(1200), PTL
           DEF IndexedHistoricalTemporalStage2ExactIdleRetryPending,
               IndexedHistoricalTemporalStage2ExactIdleRetrySelected,
               IndexedHistoricalTemporalStage2HandoffProgressExit,
               IndexedHistoricalTemporalStage2Owned,
               IndexedHistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTemporalCandidateRunnerPending,
               IndexedRunHistoricalRecoveryStep,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2ExactIdleRetryPending,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2ExactIdleRetrySelected,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2HandoffProgressExit,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2IdleHandoffAwaitingRearm,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2IdleHandoffAtDistance,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2IdleHandoffCursorProgress,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2ForeignIdleSkip,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalProtectedServiceOwnershipExit,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned,
               IndexedHistoricalTransport!
                 Stage2DeferredHandoffOwned,
               IndexedHistoricalTransport!
                 Stage2ActiveDeferredHandoff,
               IndexedHistoricalTransport!
                 Stage2DeferredHandoffToken,
               IndexedHistoricalTransport!Stage2HandoffCursorDistance,
               IndexedHistoricalTransport!DeferredHandoffActive,
               IndexedHistoricalTransport!DeferredHandoffQueueHead,
               IndexedHistoricalTransport!DeferredHandoffMatches,
               IndexedHistoricalTransport!
                 DeferredHandoffAllowsExecution,
               IndexedHistoricalTransport!
                 DeferredHandoffBlocksExecution,
               IndexedHistoricalTransport!DeferredDrainStep,
               IndexedHistoricalTransport!NextDeferredCommand,
               IndexedHistoricalTransport!ReadyRunAuxRank,
               IndexedHistoricalTransport!ReadyRunAuxOrdering,
               IndexedHistoricalTransport!ReadyRunAuxCarrier,
               IndexedHistoricalTransport!
                 PostGstRunHistoricalRecoveryNode,
               IndexedHistoricalTransport!RunHistoricalRecoveryNode,
               IndexedHistoricalTransport!RunNodeWork,
               IndexedHistoricalTransport!AsyncNext,
               IndexedHistoricalTransport!AsyncAllVars
      <3> QED BY <3>3
    <2>7. CASE candidate.node \notin Responsive
      <3>1. []~IndexedHistoricalTemporalStage2ExactIdleRetryPending(
                    initialContext, candidate)
        BY <2>7, PTL
           DEF IndexedHistoricalTemporalStage2ExactIdleRetryPending,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2ExactIdleRetryPending,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

IndexedHistoricalTemporalStage2RankProgressExit(
    initialContext, candidate, position) ==
  IndexedHistoricalTemporalRankProgressExit(
    initialContext, candidate, <<2, position>>)

IndexedHistoricalTemporalStage2HandoffRankBlocked(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2HandoffRankBlocked(candidate, position)

IndexedHistoricalTemporalStage2RankOrHandoffProgress(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2RankOrHandoffProgress(
      candidate, position)

THEOREM IndexedChainSpecReachesHistoricalStage2ExitOrHandoff ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTransport(initialContext)!
           HistoricalProtectedOwnedAtServiceRank(
             candidate, <<2, position>>)
           ~> IndexedHistoricalTemporalStage2RankOrHandoffProgress(
                initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalProtectedOwnedAtServiceRank(
                   candidate, <<2, position>>)
                 ~>
               IndexedHistoricalTemporalStage2RankOrHandoffProgress(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>3. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>4. (IndexedHistoricalTemporalStage2Owned(
             initialContext, candidate)
             /\ ~IndexedHistoricalTransport(initialContext)!
                   NodeIdle(candidate.node))
             ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                  initialContext, candidate)
      BY <1>1,
         IndexedChainSpecTerminatesHistoricalStage2Busy
    <2>5. IndexedHistoricalTemporalStage2ExactIdleRetryPending(
             initialContext, candidate)
             ~> IndexedHistoricalTemporalStage2HandoffProgressExit(
                  initialContext, candidate)
      BY <1>1,
         IndexedChainSpecExitsHistoricalStage2IdleHandoff
    <2>6. CASE candidate.node \in Responsive
      <3>1. WF_IndexedChainVars(
               IndexedRunHistoricalRecoveryStep(
                 initialContext, candidate.node))
        BY <1>1, <2>6
           DEF IndexedChainSpec, IndexedFairness
      <3>2. IndexedHistoricalTransport(initialContext)!
               HistoricalProtectedOwnedAtServiceRank(
                 candidate, <<2, position>>)
               ~> IndexedHistoricalTemporalStage2RankOrHandoffProgress(
                    initialContext, candidate, position)
        BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5,
           <2>6, <3>1,
           IndexedHistoricalCandidateRunnerEnablesFairOccurrence,
           IndexedBracketStepProjectsEveryHistoricalTransportStep,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2BusyRetryClaimsHandoffAction,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2ForeignIdleSkipDropsDistance,
           IndexedHistoricalTransport(initialContext)!
             Stage2DeferredHandoffTokenIsInjectiveObligation,
           IndexedHistoricalTransport(initialContext)!
             Stage2SelectedDifferentDeferredClassDropsDistance,
           IndexedHistoricalTransport(initialContext)!
             ReadyRunAuxOrderingIsWellFounded,
           IndexedHistoricalTransport(initialContext)!
             ReadyRunAuxRankInCarrier,
           IndexedHistoricalTransport(initialContext)!
             LocalAdmissionStrictlyDecreasesRuntimeReach,
           IndexedHistoricalTransport(initialContext)!
             IngressDrainStrictlyDecreasesRuntimeReach,
           HeadTailProperties, FS_CardinalityType,
           IsaT(1200), PTL
           DEF IndexedHistoricalTemporalStage2RankOrHandoffProgress,
               IndexedHistoricalTemporalStage2RankProgressExit,
               IndexedHistoricalTemporalStage2HandoffRankBlocked,
               IndexedHistoricalTemporalStage2ExactIdleRetryPending,
               IndexedHistoricalTemporalStage2HandoffProgressExit,
               IndexedHistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTemporalStage2Owned,
               IndexedHistoricalTemporalCandidateRunnerPending,
               IndexedRunHistoricalRecoveryStep,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2RankOrHandoffProgress,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2RankProgressExit,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2HandoffRankBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyRejectedSelected,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyRetryClaimsHandoff,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalProtectedOwnedAtServiceRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedServiceOwnershipExit,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned,
               IndexedHistoricalTransport!CandidateServiceRank,
               IndexedHistoricalTransport!ServiceRankLess,
               IndexedHistoricalTransport!Stage2DeferredHandoffOwned,
               IndexedHistoricalTransport!Stage2ActiveDeferredHandoff,
               IndexedHistoricalTransport!Stage2DeferredHandoffToken,
               IndexedHistoricalTransport!DeferredHandoffActive,
               IndexedHistoricalTransport!DeferredHandoffMatches,
               IndexedHistoricalTransport!DeferredHandoffQueueHead,
               IndexedHistoricalTransport!DeferredHandoffCandidate,
               IndexedHistoricalTransport!
                 DeferredHandoffAllowsExecution,
               IndexedHistoricalTransport!
                 DeferredHandoffBlocksExecution,
               IndexedHistoricalTransport!DeferredDrainStep,
               IndexedHistoricalTransport!
                 PostGstRunHistoricalRecoveryNode,
               IndexedHistoricalTransport!RunHistoricalRecoveryNode,
               IndexedHistoricalTransport!RunNodeWork,
               IndexedHistoricalTransport!AsyncNext,
               IndexedHistoricalTransport!AsyncAllVars
      <3> QED BY <3>2
    <2>7. CASE candidate.node \notin Responsive
      <3>1. []~IndexedHistoricalTransport(initialContext)!
                    HistoricalProtectedOwnedAtServiceRank(
                      candidate, <<2, position>>)
        BY <2>7, PTL
           DEF IndexedHistoricalTransport!
                 HistoricalProtectedOwnedAtServiceRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM IndexedChainSpecExitsHistoricalStage2HandoffRank ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTemporalStage2HandoffRankBlocked(
           initialContext, candidate, position)
           ~> IndexedHistoricalTemporalStage2RankProgressExit(
                initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTemporalStage2HandoffRankBlocked(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage2RankProgressExit(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. (IndexedHistoricalTemporalStage2Owned(
             initialContext, candidate)
             /\ ~IndexedHistoricalTransport(initialContext)!
                   NodeIdle(candidate.node))
             ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                  initialContext, candidate)
      BY <1>1,
         IndexedChainSpecTerminatesHistoricalStage2Busy
    <2>4. IndexedHistoricalTemporalStage2ExactIdleRetryPending(
             initialContext, candidate)
             ~> IndexedHistoricalTemporalStage2HandoffProgressExit(
                  initialContext, candidate)
      BY <1>1,
         IndexedChainSpecExitsHistoricalStage2IdleHandoff
    <2>5. IndexedHistoricalTemporalStage2HandoffRankBlocked(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage2RankProgressExit(
                  initialContext, candidate, position)
      BY <1>1, <2>1, <2>2, <2>3, <2>4,
         IndexedBracketStepProjectsEveryHistoricalTransportStep,
         IndexedHistoricalTransport(initialContext)!
           Stage2DeferredHandoffTokenIsInjectiveObligation,
         HeadTailProperties, IsaT(1200), PTL
         DEF IndexedHistoricalTemporalStage2HandoffRankBlocked,
             IndexedHistoricalTemporalStage2RankProgressExit,
             IndexedHistoricalTemporalStage2HandoffProgressExit,
             IndexedHistoricalTemporalStage2ExactIdleRetryPending,
             IndexedHistoricalTemporalStage2Owned,
             IndexedHistoricalTemporalStage2BusyTerminationGoal,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2HandoffRankBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2RankProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2HandoffProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2ExactIdleRetryPending,
             IndexedHistoricalTransport!Stage2DeferredHandoffOwned,
             IndexedHistoricalTransport!Stage2ActiveDeferredHandoff,
             IndexedHistoricalTransport!Stage2DeferredHandoffToken,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2Owned,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2BusyTerminationGoal,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned,
             IndexedHistoricalTransport!CandidateServiceRank,
             IndexedHistoricalTransport!ServiceRankLess,
             IndexedHistoricalTransport!DeferredHandoffActive,
             IndexedHistoricalTransport!DeferredHandoffCandidate,
             IndexedHistoricalTransport!DeferredHandoffQueueHead,
             IndexedHistoricalTransport!DeferredHandoffMatches,
             IndexedHistoricalTransport!DeferredHandoffAllowsExecution,
             IndexedHistoricalTransport!DeferredHandoffBlocksExecution,
             IndexedHistoricalTransport!RemoveNextDeferredCommand,
             IndexedHistoricalTransport!ClearDeferredHandoff,
             IndexedHistoricalTransport!RetainDeferredHandoffs,
             IndexedHistoricalTransport!DeferredDrainStep,
             IndexedHistoricalTransport!RunHistoricalRecoveryNode,
             IndexedHistoricalTransport!RunNodeWork,
             IndexedHistoricalTransport!AsyncNext,
             IndexedHistoricalTransport!AsyncAllVars
    <2> QED BY <2>5
  <1> QED BY <1>1

IndexedHistoricalTemporalStage2Source(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalProtectedOwnedAtServiceRank(
      candidate, <<2, position>>)

IndexedHistoricalTemporalStage2Goal(
    initialContext, candidate, position) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedServiceOwnershipExit(candidate)
  \/ \E lower \in SetLessThan(
       <<2, position>>,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankCarrier):
       IndexedHistoricalTransport(initialContext)!
         HistoricalProtectedOwnedAtServiceRank(candidate, lower)

IndexedHistoricalTemporalStage2LeafProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate \in IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
     position \in Nat:
    IndexedHistoricalTemporalStage2Source(
      initialContext, candidate, position)
      ~> IndexedHistoricalTemporalStage2Goal(
           initialContext, candidate, position)

THEOREM IndexedChainSpecClosesHistoricalTemporalStage2Leaf ==
  IndexedChainSpec
    => IndexedHistoricalTemporalStage2LeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage2Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage2Goal(
                 initialContext, candidate, position)
    <2>1. IndexedHistoricalTemporalStage2Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage2RankOrHandoffProgress(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedChainSpecReachesHistoricalStage2ExitOrHandoff
         DEF IndexedHistoricalTemporalStage2Source
    <2>2. IndexedHistoricalTemporalStage2HandoffRankBlocked(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage2RankProgressExit(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedChainSpecExitsHistoricalStage2HandoffRank
    <2>3. IndexedHistoricalTemporalStage2Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage2RankProgressExit(
                  initialContext, candidate, position)
      BY <2>1, <2>2, PTL
         DEF IndexedHistoricalTemporalStage2RankOrHandoffProgress,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2RankOrHandoffProgress
    <2>4. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>5. IndexedHistoricalTemporalSupportAt(initialContext)
             /\ IndexedHistoricalTemporalStage2RankProgressExit(
                  initialContext, candidate, position)
             => IndexedHistoricalTemporalStage2Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRankExitHasWellFoundedSuccessor, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalStage2RankProgressExit,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTemporalStage2Goal,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!OwnedServiceRankCarrier
    <2>6. IndexedHistoricalTemporalStage2RankProgressExit(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage2Goal(
                  initialContext, candidate, position)
      BY <2>4, <2>5, PTL
    <2> QED BY <2>3, <2>6, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalTemporalStage2LeafProperty

IndexedHistoricalTemporalCandidateStageLeafProperties ==
  /\ IndexedHistoricalTemporalStage2LeafProperty
  /\ IndexedHistoricalTemporalStage3LeafProperty
  /\ IndexedHistoricalTemporalStage4LeafProperty
  /\ IndexedHistoricalTemporalStage5LeafProperty
  /\ IndexedHistoricalTemporalStage6LeafProperty

THEOREM IndexedChainSpecClosesAllHistoricalTemporalCandidateStageLeaves ==
  IndexedChainSpec
    => IndexedHistoricalTemporalCandidateStageLeafProperties
BY IndexedChainSpecClosesHistoricalTemporalStage2Leaf,
   IndexedChainSpecClosesHistoricalTemporalStage3Leaf,
   IndexedChainSpecClosesHistoricalTemporalStage4Leaf,
   IndexedChainSpecClosesHistoricalTemporalStage5Leaf,
   IndexedChainSpecClosesHistoricalTemporalStage6Leaf
   DEF IndexedHistoricalTemporalCandidateStageLeafProperties

IndexedHistoricalProtectedServiceRankLeafProperties ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalProtectedServiceRankLeafProperties(IndexedChainSpec)

THEOREM IndexedChainSpecClosesHistoricalProtectedServiceRankLeaves ==
  IndexedChainSpec
    => IndexedHistoricalProtectedServiceRankLeafProperties
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalProtectedServiceRankLeafProperties(
                   IndexedChainSpec)
    <2>1. IndexedHistoricalTemporalCandidateStageLeafProperties
      BY <1>1,
         IndexedChainSpecClosesAllHistoricalTemporalCandidateStageLeaves
    <2> QED BY <1>1, <2>1
         DEF IndexedHistoricalProtectedServiceRankLeafProperties,
             IndexedHistoricalTemporalCandidateStageLeafProperties,
             IndexedHistoricalTemporalStage2LeafProperty,
             IndexedHistoricalTemporalStage3LeafProperty,
             IndexedHistoricalTemporalStage4LeafProperty,
             IndexedHistoricalTemporalStage5LeafProperty,
             IndexedHistoricalTemporalStage6LeafProperty,
             IndexedHistoricalTemporalStage2Source,
             IndexedHistoricalTemporalStage2Goal,
             IndexedHistoricalTemporalStage3Source,
             IndexedHistoricalTemporalStage3Goal,
             IndexedHistoricalTemporalStage4Source,
             IndexedHistoricalTemporalStage4Goal,
             IndexedHistoricalTemporalStage5Source,
             IndexedHistoricalTemporalStage5Goal,
             IndexedHistoricalTemporalStage6Source,
             IndexedHistoricalTemporalStage6Goal,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceRankLeafProperties,
             IndexedHistoricalTransport!
               HistoricalProtectedStage2RankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedStage3RankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedStage4RankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedStage5RankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedStage6RankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedStageRankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank
  <1> QED BY <1>1
       DEF IndexedHistoricalProtectedServiceRankLeafProperties

IndexedHistoricalProtectedCandidateStarvationProperties ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalProtectedCandidateStarvationProperty(IndexedChainSpec)

THEOREM IndexedChainSpecClosesHistoricalProtectedCandidateStarvation ==
  IndexedChainSpec
    => IndexedHistoricalProtectedCandidateStarvationProperties
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalProtectedCandidateStarvationProperty(
                   IndexedChainSpec)
    <2>1. IndexedHistoricalTransport(initialContext)!
             HistoricalProtectedServiceRankLeafProperties(
               IndexedChainSpec)
      BY <1>1,
         IndexedChainSpecClosesHistoricalProtectedServiceRankLeaves
         DEF IndexedHistoricalProtectedServiceRankLeafProperties
    <2>2. IndexedHistoricalTransport(initialContext)!
             HistoricalProtectedServiceRankProgressProperty(
               IndexedChainSpec)
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalProtectedServiceRankProgressFromStageLeaves
    <2> QED BY <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalProtectedServiceRankProgressImpliesStarvation
  <1> QED BY <1>1
       DEF IndexedHistoricalProtectedCandidateStarvationProperties

IndexedHistoricalDecisionCandidateProgressLeaves ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalCommitDeliveryProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalBeginDecisionProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalPersistDecisionProgressLeaf(IndexedChainSpec)

THEOREM IndexedChainSpecClosesHistoricalDecisionCandidateProgressLeaves ==
  IndexedChainSpec
    => IndexedHistoricalDecisionCandidateProgressLeaves
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords
         PROVE
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalCommitDeliveryProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalBeginDecisionProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalPersistDecisionProgressLeaf(IndexedChainSpec)
    <2>1. IndexedHistoricalTransport(initialContext)!
             HistoricalProtectedCandidateStarvationProperty(
               IndexedChainSpec)
      BY <1>1,
         IndexedChainSpecClosesHistoricalProtectedCandidateStarvation
         DEF IndexedHistoricalProtectedCandidateStarvationProperties
    <2> QED BY <1>1, <2>1
         DEF IndexedHistoricalTransport!
               HistoricalCommitDeliveryProgressLeaf,
             IndexedHistoricalTransport!
               HistoricalBeginDecisionProgressLeaf,
             IndexedHistoricalTransport!
               HistoricalPersistDecisionProgressLeaf
  <1> QED BY <1>1
       DEF IndexedHistoricalDecisionCandidateProgressLeaves

IndexedHistoricalDecisionBodyCandidateProgressLeaves ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionFetchProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionRequestBodyProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionFetchCertifiedProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionStoreProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionValidateProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionApplyProgressLeaf(IndexedChainSpec)

THEOREM IndexedChainSpecClosesHistoricalDecisionBodyCandidateLeaves ==
  IndexedChainSpec
    => IndexedHistoricalDecisionBodyCandidateProgressLeaves
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords
         PROVE
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionFetchProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionRequestBodyProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionFetchCertifiedProgressLeaf(
                  IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionStoreProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionValidateProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionApplyProgressLeaf(IndexedChainSpec)
    <2>1. IndexedHistoricalTransport(initialContext)!
             HistoricalProtectedCandidateStarvationProperty(
               IndexedChainSpec)
      BY <1>1,
         IndexedChainSpecClosesHistoricalProtectedCandidateStarvation
         DEF IndexedHistoricalProtectedCandidateStarvationProperties
    <2> QED BY <1>1, <2>1
         DEF IndexedHistoricalTransport!
               HistoricalDecisionFetchProgressLeaf,
             IndexedHistoricalTransport!
               HistoricalDecisionRequestBodyProgressLeaf,
             IndexedHistoricalTransport!
               HistoricalDecisionFetchCertifiedProgressLeaf,
             IndexedHistoricalTransport!
               HistoricalDecisionStoreProgressLeaf,
             IndexedHistoricalTransport!
               HistoricalDecisionValidateProgressLeaf,
             IndexedHistoricalTransport!
               HistoricalDecisionApplyProgressLeaf
  <1> QED BY <1>1
       DEF IndexedHistoricalDecisionBodyCandidateProgressLeaves

(***************************************************************************
Indexed route witness.

The helper invariant is intentionally stronger than the requested property:
an exact route is retained for every live historical target which has neither
a Decision nor an Application, whether or not that target has published its
Commit-certificate request yet.  Publication can therefore only expose an
already-retained route; it cannot create the missing archive identity.
***************************************************************************)

IndexedHistoricalCommitArchiveRouteAvailable(
    initialContext, target, server) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalCommitArchiveRouteAvailable(target, server)

IndexedHistoricalCommitArchiveRouteExit(initialContext, target) ==
  \/ IndexedHistoricalTransport(initialContext)!NodeHasDecision(target)
  \/ IndexedHistoricalTransport(initialContext)!NodeHasApplication(target)
  \/ ~IndexedHistoricalTransport(initialContext)!
        HistoricalRecoveryTarget(target)

IndexedHistoricalCommitArchiveRouteWitnessAt(initialContext) ==
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncFrozenContextAt(initialContext)
  /\ IndexedHistoricalTransport(initialContext)!AsyncStrongTypeInvariant
  /\ \A target \in Responsive:
       /\ IndexedHistoricalTransport(initialContext)!
            HistoricalRecoveryTarget(target)
       /\ ~IndexedHistoricalTransport(initialContext)!
              NodeHasDecision(target)
       /\ ~IndexedHistoricalTransport(initialContext)!
              NodeHasApplication(target)
       => \E server \in ValidatorIds:
            IndexedHistoricalCommitArchiveRouteAvailable(
              initialContext, target, server)

IndexedHistoricalCommitArchiveRouteWitnessInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalCommitArchiveRouteWitnessAt(initialContext)

IndexedHistoricalCommitArchiveRouteAvailabilityInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalCommitArchiveRouteAvailabilityInvariant

IndexedHistoricalCommitArchiveRouteAvailabilityProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalCommitArchiveRouteAvailabilityProperty(IndexedChainSpec)

(***************************************************************************
The exact indexed source selects the intersection member.

The source is a current application owned by `server`, and the frozen
transport context is `initialContext`.  The target has no such application,
so `server # target`.  The remaining source guards put `server` in the current
responsive voter set and in `up`.
***************************************************************************)

THEOREM IndexedHistoricalSourceSelectsExactAppliedCurrentVoter ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds,
     server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncFrozenContextAt(initialContext)
    /\ IndexedHistoricalRecoveryTargetReady(initialContext, target)
    /\ IndexedHistoricalRecoverySourceReady(
         initialContext, server, source)
    => /\ server \in
              IndexedHistoricalTransport(initialContext)!CurrentVoters
                \ {target}
       /\ server \in
              IndexedHistoricalTransport(initialContext)!
                AsyncResponsiveAppliedArchiveServers
BY Isa
   DEF IndexedHistoricalRecoveryTargetReady,
       IndexedHistoricalRecoverySourceReady,
       IndexedCurrentApplications,
       IndexedProjectedNodeHasApplication,
       HistoricalRecoveryNodeHasApplicationProjection,
       IndexedHistoricalTransport!AsyncFrozenContextAt,
       IndexedHistoricalTransport!CurrentVoters,
       IndexedHistoricalTransport!CurrentEpoch,
       IndexedHistoricalTransport!AsyncResponsiveAppliedArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveOnlineArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveArchiveServers,
       IndexedHistoricalTransport!AsyncArchiveServerIds,
       IndexedHistoricalTransport!NodeHasApplication,
       IndexedAsync!AsyncCurrentResponsiveVoters,
       IndexedAsync!CurrentVoters, IndexedAsync!CurrentEpoch

THEOREM IndexedOpenHistoricalRecoveryEstablishesPostStateArchiveRoute ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds,
     server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncFrozenContextAt(initialContext)
    /\ IndexedOpenHistoricalRecovery(
         initialContext, target, server, source)
    => IndexedHistoricalCommitArchiveRouteAvailable(
         initialContext, target, server)'
BY IndexedHistoricalSourceSelectsExactAppliedCurrentVoter, Isa
   DEF IndexedOpenHistoricalRecovery,
       IndexedHistoricalCommitArchiveRouteAvailable,
       IndexedHistoricalTransport!
         HistoricalCommitArchiveRouteAvailable,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedHistoricalTransport!AsyncResponsiveAppliedArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveOnlineArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveArchiveServers,
       IndexedHistoricalTransport!NodeHasApplication,
       IndexedHistoricalTransport!CurrentVoters,
       IndexedHistoricalTransport!CurrentEpoch,
       IndexedAsync!OpenHistoricalRecovery,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!AsyncSchedulerExceptHistoricalRecoveryTargets,
       IndexedAsync!AsyncAllVars,
       IndexedAsync!AsyncSchedulerVars,
       IndexedAsync!AsyncRecoveryVars,
       IndexedAsync!vars

(***************************************************************************
Route retention for an existing unresolved target.

Applications and Decisions are durable, GST is monotone, the one-height
context is frozen, and the strong type invariant makes every responsive node
up after GST.  Hence the chosen server remains both an applied archive and a
current voter.  The only excluded semantic case is the target acquiring its
Decision, after which the requested availability predicate is already
vacuous.
***************************************************************************)

THEOREM IndexedHistoricalArchiveRoutePersistsUntilTargetDecision ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds,
     server \in ValidatorIds:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncFrozenContextAt(initialContext)
    /\ IndexedHistoricalTransport(initialContext)!AsyncStrongTypeInvariant
    /\ IndexedHistoricalCommitArchiveRouteAvailable(
         initialContext, target, server)
    /\ [IndexedChainNext]_IndexedChainVars
    /\ ~IndexedHistoricalTransport(initialContext)!
           NodeHasDecision(target)'
    => IndexedHistoricalCommitArchiveRouteAvailable(
         initialContext, target, server)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW target \in ValidatorIds,
                NEW server \in ValidatorIds,
                IndexedHistoricalTransport(initialContext)!
                  AsyncFrozenContextAt(initialContext),
                IndexedHistoricalTransport(initialContext)!
                  AsyncStrongTypeInvariant,
                IndexedHistoricalCommitArchiveRouteAvailable(
                  initialContext, target, server),
                [IndexedChainNext]_IndexedChainVars,
                ~IndexedHistoricalTransport(initialContext)!
                   NodeHasDecision(target)'
         PROVE IndexedHistoricalCommitArchiveRouteAvailable(
                 initialContext, target, server)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. (IndexedHistoricalTransport(initialContext)!
             AsyncFrozenContextAt(initialContext))'
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncNextPreservesFrozenContext
    <2>3. (IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant)'
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncBracketNextPreservesStrongTypeInvariant
    <2>4. IndexedCore(initialContext, 7)'
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           GstAsyncStepIsMonotone
         DEF IndexedHistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailable
    <2>5. (IndexedHistoricalTransport(initialContext)!
             HistoricalRecoveryTarget(target))'
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalRecoveryTargetPersistsUnlessDecision
         DEF IndexedHistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailable
    <2>6. (IndexedHistoricalTransport(initialContext)!
             NodeHasApplication(server))'
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncBracketStepPreservesNodeApplication
         DEF IndexedHistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               AsyncResponsiveAppliedArchiveServers,
             IndexedHistoricalTransport!
               AsyncResponsiveOnlineArchiveServers,
             IndexedHistoricalTransport!AsyncResponsiveArchiveServers
    <2>7. (Responsive \subseteq IndexedCore(initialContext, 6))'
      BY <2>3, <2>4,
         IndexedHistoricalTransport(initialContext)!
           GstResponsiveNodesAreUp
         DEF IndexedHistoricalTransport!AsyncStrongTypeInvariant
    <2> QED BY <1>1, <2>2, <2>4, <2>5, <2>6, <2>7, Isa
         DEF IndexedHistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               AsyncResponsiveAppliedArchiveServers,
             IndexedHistoricalTransport!
               AsyncResponsiveOnlineArchiveServers,
             IndexedHistoricalTransport!AsyncResponsiveArchiveServers,
             IndexedHistoricalTransport!AsyncArchiveServerIds,
             IndexedHistoricalTransport!AsyncFrozenContextAt,
             IndexedHistoricalTransport!CurrentVoters,
             IndexedHistoricalTransport!CurrentEpoch
  <1> QED BY <1>1

THEOREM IndexedHistoricalArchiveRoutePersistsUntilSemanticExit ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds,
     server \in ValidatorIds:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncFrozenContextAt(initialContext)
    /\ IndexedHistoricalTransport(initialContext)!AsyncStrongTypeInvariant
    /\ IndexedHistoricalCommitArchiveRouteAvailable(
         initialContext, target, server)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalCommitArchiveRouteAvailable(
             initialContext, target, server)'
       \/ IndexedHistoricalCommitArchiveRouteExit(
            initialContext, target)'
BY IndexedHistoricalArchiveRoutePersistsUntilTargetDecision, Isa
   DEF IndexedHistoricalCommitArchiveRouteExit

(***************************************************************************
Wrapper-only target creation.

All ordinary product actions preserve the historical-target set or remove a
target at Apply.  The sole addition is the indexed open disjunct, whose guard
retains the exact `server` and `source`.  This is the only action
classification which is specific to the chain wrapper.
***************************************************************************)

THEOREM IndexedNonOpenProductStepCannotCreateHistoricalTarget ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds:
    /\ IndexedChainNext
    /\ ~IndexedHistoricalTransport(initialContext)!
           HistoricalRecoveryTarget(target)
    /\ ~(\E server \in ValidatorIds,
             source \in Chain!DecisionEvidenceSet:
          IndexedOpenHistoricalRecovery(
            initialContext, target, server, source))
    => ~IndexedHistoricalTransport(initialContext)!
          HistoricalRecoveryTarget(target)'
BY IndexedSuccessorActivationStepStuttersAsyncState, IsaT(600)
   DEF IndexedChainNext, IndexedProductActionAt,
       IndexedJoinedAsyncNext, IndexedJoinedNonCrashStep,
       IndexedJoinedRunnerStep, IndexedJoinedNonRunnerStep,
       IndexedOpenHistoricalRecovery,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!AsyncNext, IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncRunnerStep, IndexedAsync!AsyncNonRunnerStep,
       IndexedAsync!OpenHistoricalRecovery,
       IndexedAsync!HistoricalRecoverySourceReady,
       IndexedAsync!AsyncSetGST, IndexedAsync!AsyncTick,
       IndexedAsync!DirectCommitCertificateDiscoveryStep,
       IndexedAsync!DirectHistoricalCommitCertificateDiscoveryStep,
       IndexedAsync!ServiceIoWorker,
       IndexedAsync!ServiceHistoricalRecoveryIoWorker,
       IndexedAsync!EnqueueIoLocalControl,
       IndexedAsync!EnqueueHistoricalRecoveryIoLocalControl,
       IndexedAsync!AsyncNetworkStep, IndexedAsync!AsyncFaultStep,
       IndexedAsync!RunNode,
       IndexedAsync!RunHistoricalRecoveryNode,
       IndexedAsync!RunNodeWork,
       IndexedAsync!RunHistoricalServer,
       IndexedAsync!ServiceIoWorkerWork,
       IndexedAsync!EnqueueIoLocalControlWork,
       IndexedAsync!CommitCertificateDiscoveryStepWork,
       IndexedAsync!LocalAdmissionStep,
       IndexedAsync!IngressDrainStep,
       IndexedAsync!SerializedRuntimeStep,
       IndexedAsync!RuntimeStep,
       IndexedAsync!FifoRuntimeStep,
       IndexedAsync!ExecuteCommand,
       IndexedAsync!ExecuteApply,
       IndexedAsync!PreGstCrash,
       IndexedAsync!PreGstResponsiveCrash,
       IndexedAsync!PreGstResponsiveRestart,
       IndexedAsync!PreGstResponsiveReplay,
       IndexedAsync!ResetNodeSchedulerForRestart,
       IndexedAsync!AsyncHistoricalLockRestartAuthorityTransition,
       IndexedAsync!AsyncAllVars, IndexedAsync!AsyncSchedulerVars,
       IndexedAsync!AsyncRecoveryVars, IndexedAsync!vars,
       IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       IndexedSuccessorActivationProgressStep,
       SuccessorActivationEnvironmentStutter

THEOREM IndexedNewHistoricalTargetHasExactOpenSource ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds:
    /\ [IndexedChainNext]_IndexedChainVars
    /\ ~IndexedHistoricalTransport(initialContext)!
           HistoricalRecoveryTarget(target)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalRecoveryTarget(target)'
    => \E server \in ValidatorIds,
          source \in Chain!DecisionEvidenceSet:
         IndexedOpenHistoricalRecovery(
           initialContext, target, server, source)
BY IndexedNonOpenProductStepCannotCreateHistoricalTarget, Isa
   DEF IndexedChainVars,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedHistoricalTransport!AsyncAllVars,
       IndexedHistoricalTransport!AsyncSchedulerVars,
       IndexedHistoricalTransport!AsyncRecoveryVars,
       IndexedHistoricalTransport!vars,
       IndexedCore, IndexedScheduler, IndexedRecovery

(***************************************************************************
Initialization and product preservation of the exact witness.
***************************************************************************)

THEOREM IndexedChainInitEstablishesHistoricalArchiveRouteWitness ==
  IndexedChainInit
    => IndexedHistoricalCommitArchiveRouteWitnessInvariant
PROOF
  <1>1. ASSUME IndexedChainInit,
               NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalCommitArchiveRouteWitnessAt(initialContext)
    <2>1. IndexedHistoricalTransport(initialContext)!
             AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryHistoricalTransportInit
    <2>2. IndexedHistoricalTransport(initialContext)!
             AsyncFrozenContextAt(initialContext)
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesFrozenContext
    <2>3. IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesStrongTypeInvariant
    <2>4. \A target \in Responsive:
             ~IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTarget(target)
      BY <2>1, Isa
         DEF IndexedHistoricalTransport!AsyncInitAt,
             IndexedHistoricalTransport!AsyncBaseInitAt,
             IndexedHistoricalTransport!AsyncTransportInit,
             IndexedHistoricalTransport!HistoricalRecoveryTarget
    <2> QED BY <2>2, <2>3, <2>4
         DEF IndexedHistoricalCommitArchiveRouteWitnessAt
  <1> QED BY <1>1
       DEF IndexedHistoricalCommitArchiveRouteWitnessInvariant

THEOREM IndexedBracketStepPreservesHistoricalArchiveRouteWitness ==
  /\ IndexedHistoricalCommitArchiveRouteWitnessInvariant
  /\ [IndexedChainNext]_IndexedChainVars
  => IndexedHistoricalCommitArchiveRouteWitnessInvariant'
PROOF
  <1>1. ASSUME IndexedHistoricalCommitArchiveRouteWitnessInvariant,
              [IndexedChainNext]_IndexedChainVars,
              NEW initialContext \in AdmissibleContextRecords
         PROVE
           IndexedHistoricalCommitArchiveRouteWitnessAt(initialContext)'
    <2>1. IndexedHistoricalCommitArchiveRouteWitnessAt(initialContext)
      BY <1>1
         DEF IndexedHistoricalCommitArchiveRouteWitnessInvariant
    <2>2. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>3. (IndexedHistoricalTransport(initialContext)!
             AsyncFrozenContextAt(initialContext))'
      BY <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           AsyncNextPreservesFrozenContext
         DEF IndexedHistoricalCommitArchiveRouteWitnessAt
    <2>4. (IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant)'
      BY <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           AsyncBracketNextPreservesStrongTypeInvariant
         DEF IndexedHistoricalCommitArchiveRouteWitnessAt
    <2>5. ASSUME NEW target \in Responsive,
                   IndexedHistoricalTransport(initialContext)!
                     HistoricalRecoveryTarget(target)',
                   ~IndexedHistoricalTransport(initialContext)!
                      NodeHasDecision(target)',
                   ~IndexedHistoricalTransport(initialContext)!
                      NodeHasApplication(target)'
           PROVE \E server \in ValidatorIds:
                   IndexedHistoricalCommitArchiveRouteAvailable(
                     initialContext, target, server)'
      <3>0. target \in ValidatorIds
        BY <2>1, <2>5, Isa
           DEF IndexedHistoricalCommitArchiveRouteWitnessAt,
               IndexedHistoricalTransport!AsyncStrongTypeInvariant,
               IndexedHistoricalTransport!StrongInductiveInvariant,
               IndexedHistoricalTransport!Safety,
               IndexedHistoricalTransport!TypeInvariant,
               IndexedHistoricalTransport!ModelConfiguration,
               IndexedHistoricalTransport!QuorumConfiguration
      <3>1. ~IndexedHistoricalTransport(initialContext)!
                 NodeHasDecision(target)
        BY <2>2, <2>5,
           IndexedHistoricalTransport(initialContext)!
             AsyncBracketStepPreservesNodeDecision
      <3>2. ~IndexedHistoricalTransport(initialContext)!
                 NodeHasApplication(target)
        BY <2>2, <2>5,
           IndexedHistoricalTransport(initialContext)!
             AsyncBracketStepPreservesNodeApplication
      <3>3. CASE IndexedHistoricalTransport(initialContext)!
                    HistoricalRecoveryTarget(target)
        <4>1. PICK server \in ValidatorIds:
                 IndexedHistoricalCommitArchiveRouteAvailable(
                   initialContext, target, server)
          BY <2>1, <3>1, <3>2, <3>3
             DEF IndexedHistoricalCommitArchiveRouteWitnessAt
        <4>2. IndexedHistoricalCommitArchiveRouteAvailable(
                 initialContext, target, server)'
          BY <1>1, <2>1, <2>5, <3>0, <4>1,
             IndexedHistoricalArchiveRoutePersistsUntilTargetDecision
             DEF IndexedHistoricalCommitArchiveRouteWitnessAt
        <4> QED BY <4>1, <4>2
      <3>4. CASE ~IndexedHistoricalTransport(initialContext)!
                     HistoricalRecoveryTarget(target)
        <4>1. \E server \in ValidatorIds,
                  source \in Chain!DecisionEvidenceSet:
                 IndexedOpenHistoricalRecovery(
                   initialContext, target, server, source)
          BY <1>1, <2>5, <3>0, <3>4,
             IndexedNewHistoricalTargetHasExactOpenSource
        <4>2. PICK server \in ValidatorIds,
                     source \in Chain!DecisionEvidenceSet:
                 IndexedOpenHistoricalRecovery(
                   initialContext, target, server, source)
          BY <4>1
        <4>3. IndexedHistoricalCommitArchiveRouteAvailable(
                 initialContext, target, server)'
          BY <2>1, <3>0, <4>2,
             IndexedOpenHistoricalRecoveryEstablishesPostStateArchiveRoute
             DEF IndexedHistoricalCommitArchiveRouteWitnessAt
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>3, <3>4
    <2> QED BY <2>3, <2>4, <2>5
         DEF IndexedHistoricalCommitArchiveRouteWitnessAt
  <1> QED BY <1>1
       DEF IndexedHistoricalCommitArchiveRouteWitnessInvariant

THEOREM IndexedChainSpecAlwaysHasHistoricalArchiveRouteWitness ==
  IndexedChainSpec
    => []IndexedHistoricalCommitArchiveRouteWitnessInvariant
PROOF
  <1>1. IndexedChainInit
          => IndexedHistoricalCommitArchiveRouteWitnessInvariant
    BY IndexedChainInitEstablishesHistoricalArchiveRouteWitness
  <1>2. /\ IndexedHistoricalCommitArchiveRouteWitnessInvariant
         /\ [IndexedChainNext]_IndexedChainVars
        => IndexedHistoricalCommitArchiveRouteWitnessInvariant'
    BY IndexedBracketStepPreservesHistoricalArchiveRouteWitness
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

(***************************************************************************
Requested indexed property.

The auxiliary witness drops directly into the one-height implication.  The
extra active-request, no-Decision, and no-Application guards are consumed
unchanged; no transport liveness or additional responsive-join assumption is
used.
***************************************************************************)

THEOREM IndexedHistoricalArchiveRouteWitnessImpliesAvailability ==
  IndexedHistoricalCommitArchiveRouteWitnessInvariant
    => IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
BY Isa
   DEF IndexedHistoricalCommitArchiveRouteWitnessInvariant,
       IndexedHistoricalCommitArchiveRouteWitnessAt,
       IndexedHistoricalCommitArchiveRouteAvailabilityInvariant,
       IndexedHistoricalCommitArchiveRouteAvailable,
       IndexedHistoricalTransport!
         HistoricalCommitArchiveRouteAvailabilityInvariant

THEOREM IndexedChainSpecAlwaysHasHistoricalCommitArchiveRoute ==
  IndexedChainSpec
    => []IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
BY IndexedChainSpecAlwaysHasHistoricalArchiveRouteWitness,
   IndexedHistoricalArchiveRouteWitnessImpliesAvailability, PTL

THEOREM IndexedChainSpecDischargesHistoricalCommitArchiveRouteAvailability ==
  IndexedHistoricalCommitArchiveRouteAvailabilityProperty
PROOF
  <1>1. IndexedChainSpec
          => []IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
    BY IndexedChainSpecAlwaysHasHistoricalCommitArchiveRoute
  <1>2. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           IndexedHistoricalTransport(initialContext)!
             HistoricalCommitArchiveRouteAvailabilityProperty(
               IndexedChainSpec)
    <2>1. IndexedChainSpec
            => []IndexedHistoricalTransport(initialContext)!
                  HistoricalCommitArchiveRouteAvailabilityInvariant
      BY <1>1, PTL
         DEF IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
    <2> QED BY <2>1
         DEF IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailabilityProperty
  <1> QED BY <1>2
       DEF IndexedHistoricalCommitArchiveRouteAvailabilityProperty

=============================================================================
