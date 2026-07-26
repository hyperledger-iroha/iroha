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
  INSTANCE SumeragiV2AsyncHistoricalRecoveryTransportClosureProofs
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
       asyncRecoveryPhase <- IndexedRecovery(initialContext, 1),
       asyncRecoveryNode <- IndexedRecovery(initialContext, 2),
       asyncRecoveryGeneration <- IndexedRecovery(initialContext, 3),
       asyncRecoveryReplayQueue <- IndexedRecovery(initialContext, 4),
       asyncHistoricalLockRestartAuthorities <-
         IndexedRecovery(initialContext, 5)

(***************************************************************************
Exact projection.

These are the same 46 Core, 36 scheduler, and five recovery substitutions as
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
          BY <1>1, <2>1, <2>5, <4>1,
             IndexedHistoricalArchiveRoutePersistsUntilTargetDecision
             DEF IndexedHistoricalCommitArchiveRouteWitnessAt
        <4> QED BY <4>1, <4>2
      <3>4. CASE ~IndexedHistoricalTransport(initialContext)!
                     HistoricalRecoveryTarget(target)
        <4>1. \E server \in ValidatorIds,
                  source \in Chain!DecisionEvidenceSet:
                 IndexedOpenHistoricalRecovery(
                   initialContext, target, server, source)
          BY <1>1, <2>5, <3>4,
             IndexedNewHistoricalTargetHasExactOpenSource
        <4>2. PICK server \in ValidatorIds,
                     source \in Chain!DecisionEvidenceSet:
                 IndexedOpenHistoricalRecovery(
                   initialContext, target, server, source)
          BY <4>1
        <4>3. IndexedHistoricalCommitArchiveRouteAvailable(
                 initialContext, target, server)'
          BY <2>1, <4>2,
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

THEOREM IndexedChainSpecDischargesHistoricalCommitArchiveRouteAvailability ==
  IndexedHistoricalCommitArchiveRouteAvailabilityProperty
PROOF
  <1>1. IndexedChainSpec
          => []IndexedHistoricalCommitArchiveRouteWitnessInvariant
    BY IndexedChainSpecAlwaysHasHistoricalArchiveRouteWitness
  <1>2. IndexedChainSpec
          => []IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
    BY <1>1,
       IndexedHistoricalArchiveRouteWitnessImpliesAvailability, PTL
  <1>3. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           IndexedHistoricalTransport(initialContext)!
             HistoricalCommitArchiveRouteAvailabilityProperty(
               IndexedChainSpec)
    <2>1. IndexedChainSpec
            => []IndexedHistoricalTransport(initialContext)!
                  HistoricalCommitArchiveRouteAvailabilityInvariant
      BY <1>2, PTL
         DEF IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
    <2> QED BY <2>1
         DEF IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailabilityProperty
  <1> QED BY <1>3
       DEF IndexedHistoricalCommitArchiveRouteAvailabilityProperty

=============================================================================
