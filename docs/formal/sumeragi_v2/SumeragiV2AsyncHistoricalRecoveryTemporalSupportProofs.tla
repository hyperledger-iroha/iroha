---- MODULE SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryTransportClosureProofs,
        SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs,
        SumeragiV2AsyncHistoricalCandidateProducerContinuationProofs

(***************************************************************************
Historical temporal support classification.

This module is the non-indexed action/rank layer used by the indexed
historical-recovery closure.  It deliberately does not state target-to-
Decision, Decision-to-application, one-height, or indexed-height progress.
Every temporal leaf below is attached to one concrete production owner:

  * the fixed-clock blocker rank and its candidate/Serve episode;
  * one protected historical candidate at service stages 2..6;
  * one exact Commit-certificate transport lifecycle; or
  * one exact certified-body transport lifecycle.

The owner predicates retain the historical-target guard.  They therefore do
not project an out-of-roster target into
`ResponsiveProtectedCandidateOwned`, and the only local fair action selected
for such a candidate is `PostGstRunHistoricalRecoveryNode`.
***************************************************************************)

HistoricalTemporalCandidateAtStage(candidate, stage, position) ==
  /\ gst
  /\ HistoricalProtectedCandidateOwned(candidate)
  /\ CandidateServiceRank(candidate) = <<stage, position>>
  /\ stage \in 2..6
  /\ position \in Nat

HistoricalTemporalCandidateStrictGoal(candidate, stage, position) ==
  \/ HistoricalProtectedServiceOwnershipExit(candidate)
  \/ \E lower \in SetLessThan(
       <<stage, position>>,
       OwnedServiceRankOrdering,
       OwnedServiceRankCarrier):
       HistoricalProtectedOwnedAtServiceRank(candidate, lower)

HistoricalTemporalCandidateFairAction(candidate) ==
  PostGstRunHistoricalRecoveryNode(candidate.node)

THEOREM HistoricalTemporalCandidateClassification ==
  \A candidate, stage, position:
    HistoricalTemporalCandidateAtStage(candidate, stage, position)
      => /\ candidate \in AsyncCandidateSet
         /\ candidate.node \in Responsive
         /\ HistoricalRecoveryTarget(candidate.node)
         /\ ProtectedCandidateOwned(candidate)
         /\ CandidateScheduled(candidate)
         /\ <<stage, position>> \in OwnedServiceRankCarrier
         /\ HistoricalProtectedOwnedAtServiceRank(
              candidate, <<stage, position>>)
BY ScheduledCandidateServiceRankInCarrier, Isa
   DEF HistoricalTemporalCandidateAtStage,
       HistoricalProtectedCandidateOwned,
       HistoricalProtectedOwnedAtServiceRank,
       ProtectedCandidateOwned, OwnedServiceRankCarrier

THEOREM HistoricalTemporalCandidateUsesOnlyHistoricalRunner ==
  \A candidate, stage, position:
    HistoricalTemporalCandidateAtStage(candidate, stage, position)
      => /\ HistoricalTemporalCandidateFairAction(candidate)
              = PostGstRunHistoricalRecoveryNode(candidate.node)
         /\ HistoricalRecoveryTarget(candidate.node)
         /\ candidate.node \in Responsive
BY Isa
   DEF HistoricalTemporalCandidateAtStage,
       HistoricalTemporalCandidateFairAction,
       HistoricalProtectedCandidateOwned

THEOREM AsyncSpecProvidesHistoricalCandidateRunnerFairness ==
  \A initialContext, candidate, stage, position:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalTemporalCandidateAtStage(candidate, stage, position)
    => WF_AsyncAllVars(HistoricalTemporalCandidateFairAction(candidate))
BY Isa
   DEF HistoricalTemporalCandidateAtStage,
       HistoricalTemporalCandidateFairAction,
       HistoricalProtectedCandidateOwned,
       AsyncSpecAt, AsyncFairnessAt

(***************************************************************************
Historical rank exit normalization.

The production rank lemmas use `ServiceRankLess` as their action-local exit,
while the historical release property exposes the corresponding
well-founded-set witness.  This bridge is independent of current-roster
membership: the exact historical owner remains scheduled, hence its new
service rank remains in the same finite carrier.
***************************************************************************)

HistoricalTemporalRankProgressExit(candidate, rank) ==
  \/ HistoricalProtectedServiceOwnershipExit(candidate)
  \/ ServiceRankLess(CandidateServiceRank(candidate), rank)

THEOREM HistoricalTemporalRankExitHasWellFoundedSuccessor ==
  \A candidate:
    \A rank \in OwnedServiceRankCarrier:
      /\ AsyncTypeInvariant
      /\ gst
      /\ ~HistoricalProtectedServiceOwnershipExit(candidate)
      /\ ServiceRankLess(CandidateServiceRank(candidate), rank)
      => \E lower \in SetLessThan(
                       rank, OwnedServiceRankOrdering,
                       OwnedServiceRankCarrier):
           HistoricalProtectedOwnedAtServiceRank(candidate, lower)
PROOF
  <1>1. ASSUME NEW candidate,
                NEW rank \in OwnedServiceRankCarrier,
                AsyncTypeInvariant,
                gst,
                ~HistoricalProtectedServiceOwnershipExit(candidate),
                ServiceRankLess(CandidateServiceRank(candidate), rank)
         PROVE \E lower \in SetLessThan(
                         rank, OwnedServiceRankOrdering,
                         OwnedServiceRankCarrier):
                 HistoricalProtectedOwnedAtServiceRank(candidate, lower)
    <2>1. CandidateScheduled(candidate)
      BY <1>1
         DEF HistoricalProtectedServiceOwnershipExit,
             HistoricalProtectedCandidateOwned,
             ProtectedCandidateOwned
    <2>2. CandidateServiceRank(candidate) \in OwnedServiceRankCarrier
      BY <1>1, <2>1, ScheduledCandidateServiceRankInCarrier
    <2>3. <<CandidateServiceRank(candidate), rank>>
             \in OwnedServiceRankOrdering
      BY <1>1, <2>2, OwnedServiceRankOrderingMatchesLess
    <2>4. CandidateServiceRank(candidate)
             \in SetLessThan(
                  rank, OwnedServiceRankOrdering,
                  OwnedServiceRankCarrier)
      BY <2>2, <2>3 DEF SetLessThan
    <2> QED BY <1>1, <2>4
         DEF HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedServiceOwnershipExit
  <1> QED BY <1>1

THEOREM HistoricalTemporalRunNodeIsNonstuttering ==
  \A node:
    /\ HistoricalRecoveryTarget(node)
    /\ AsyncTypeInvariant
    /\ PostGstRunHistoricalRecoveryNode(node)
    => <<PostGstRunHistoricalRecoveryNode(node)>>_AsyncAllVars
BY Isa
   DEF PostGstRunHistoricalRecoveryNode,
       RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncSchedulerExceptCausalControlAndNodeService,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncSchedulerExceptCausalControlRunnerAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       LocalAdmissionStep, IngressDrainStep, EnqueueCandidate,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncAllVars, AsyncSchedulerVars,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant

THEOREM HistoricalTemporalProtectedOwnerEnablesFairRunner ==
  \A candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ gst
    /\ HistoricalProtectedCandidateOwned(candidate)
    => ENABLED
         <<PostGstRunHistoricalRecoveryNode(
             candidate.node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW candidate,
                AsyncStrongTypeInvariant,
                AsyncCandidateProducerContinuationExternalCoverageInvariant,
                AsyncCandidateProducerContinuationLocalReplayCapacityInvariant,
                gst,
                HistoricalProtectedCandidateOwned(candidate)
         PROVE ENABLED
                 <<PostGstRunHistoricalRecoveryNode(
                     candidate.node)>>_AsyncAllVars
    <2>1. /\ candidate.node \in asyncHistoricalRecoveryTargets
           /\ HistoricalRecoveryTarget(candidate.node)
           /\ AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
         DEF HistoricalProtectedCandidateOwned,
             HistoricalRecoveryTarget
    <2>2. ENABLED
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>1, HistoricalRecoveryRunnerEnabledAfterGst
    <2>3. PostGstRunHistoricalRecoveryNode(candidate.node)
             => <<PostGstRunHistoricalRecoveryNode(
                    candidate.node)>>_AsyncAllVars
      BY <1>1, <2>1, HistoricalTemporalRunNodeIsNonstuttering
    <2> QED BY <2>2, <2>3, ENABLEDaxioms
  <1> QED BY <1>1

(***************************************************************************
Stage 3: historical-target cyclic Runtime service.

The ordinary Stage-3 theorem selects `PostGstRunNode`, whose owner is a
current responsive voter.  A historical target can be outside that roster.
The kernel below retains the same finite Runtime-prefix rank and queue
arithmetic, but selects the already-fair
`PostGstRunHistoricalRecoveryNode` action.  No additional fairness clause is
introduced.
***************************************************************************)

HistoricalTemporalStage3Pending(candidate, position) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalProtectedOwnedAtServiceRank(
       candidate, <<3, position>>)

HistoricalTemporalStage3RankExit(candidate, position) ==
  HistoricalTemporalRankProgressExit(candidate, <<3, position>>)

THEOREM HistoricalTemporalStage3CarrierFacts ==
  \A candidate, position:
    HistoricalTemporalStage3Pending(candidate, position)
      => /\ candidate.node \in Responsive
         /\ HistoricalRecoveryTarget(candidate.node)
         /\ candidate.node \in ValidatorIds
         /\ candidate \in QueuedCandidates
         /\ candidate \in
              SequenceSet(asyncCommandQueues[candidate.node])
         /\ AsyncQueueTyped(asyncCommandQueues[candidate.node])
         /\ SequenceHasUniqueValues(
              asyncCommandQueues[candidate.node])
         /\ NodeQueueNonempty(candidate.node)
         /\ SchedulerServiceRank(candidate.node, candidate) = position
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                HistoricalTemporalStage3Pending(candidate, position)
         PROVE /\ candidate.node \in Responsive
               /\ HistoricalRecoveryTarget(candidate.node)
               /\ candidate.node \in ValidatorIds
               /\ candidate \in QueuedCandidates
               /\ candidate \in
                    SequenceSet(asyncCommandQueues[candidate.node])
               /\ AsyncQueueTyped(asyncCommandQueues[candidate.node])
               /\ SequenceHasUniqueValues(
                    asyncCommandQueues[candidate.node])
               /\ NodeQueueNonempty(candidate.node)
               /\ SchedulerServiceRank(candidate.node, candidate) = position
    <2>1. /\ AsyncTypeInvariant
           /\ candidate.node \in Responsive
           /\ HistoricalRecoveryTarget(candidate.node)
           /\ CandidateScheduled(candidate)
           /\ CandidateServiceRank(candidate) = <<3, position>>
      BY <1>1, AsyncStrongTypeProjectsAsyncType
         DEF HistoricalTemporalStage3Pending,
             HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedCandidateOwned,
             ProtectedCandidateOwned
    <2>2. candidate.node \in ValidatorIds
      BY <2>1, ScheduledCandidateProjectsToOwner
    <2>3. candidate \notin DeferredCandidates
      BY <2>1, SMT DEF CandidateServiceRank
    <2>4. candidate \in QueuedCandidates
      BY <2>1, <2>3, SMT
         DEF CandidateServiceRank, CandidateScheduled
    <2>5. candidate \in
             SequenceSet(asyncCommandQueues[candidate.node])
      BY <2>1, <2>4, ScheduledCandidateProjectsToOwner
    <2>6. /\ AsyncQueueTyped(asyncCommandQueues[candidate.node])
           /\ SequenceHasUniqueValues(
                asyncCommandQueues[candidate.node])
      BY <1>1, <2>2
         DEF HistoricalTemporalStage3Pending,
             AsyncStrongTypeInvariant,
             AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant,
             AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>7. PICK matching \in
                    1..Len(asyncCommandQueues[candidate.node]):
             asyncCommandQueues[candidate.node][matching] = candidate
      BY <2>5 DEF SequenceSet
    <2>8. NodeQueueNonempty(candidate.node)
      BY <2>7, SMT DEF NodeQueueNonempty
    <2>9. SchedulerServiceRank(candidate.node, candidate) = position
      BY <2>1, <2>3, <2>4, SMT DEF CandidateServiceRank
    <2> QED BY <2>1, <2>2, <2>4, <2>5, <2>6, <2>8, <2>9
  <1> QED BY <1>1

THEOREM HistoricalTemporalSelectedTargetLeavesRuntimeQueue ==
  \A candidate, position:
    /\ HistoricalTemporalStage3Pending(candidate, position)
    /\ SerializedRunnerRuntimeStep(candidate.node)
    /\ FifoRuntimeStep(candidate.node)
    /\ NextNodeCommand(candidate.node) = candidate
    => candidate \notin
         SequenceSet(asyncCommandQueues'[candidate.node])
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                HistoricalTemporalStage3Pending(candidate, position),
                SerializedRunnerRuntimeStep(candidate.node),
                FifoRuntimeStep(candidate.node),
                NextNodeCommand(candidate.node) = candidate
         PROVE candidate \notin
                 SequenceSet(asyncCommandQueues'[candidate.node])
    <2>1. /\ candidate.node \in ValidatorIds
           /\ AsyncQueueTyped(asyncCommandQueues[candidate.node])
           /\ SequenceHasUniqueValues(
                asyncCommandQueues[candidate.node])
           /\ NodeQueueNonempty(candidate.node)
      BY <1>1, HistoricalTemporalStage3CarrierFacts
    <2>2. LET selected == NextNodeCommandIndex(candidate.node)
           IN /\ selected \in
                    1..Len(asyncCommandQueues[candidate.node])
              /\ asyncCommandQueues[candidate.node][selected] = candidate
              /\ asyncCommandQueues'[candidate.node] =
                   SequenceWithoutIndex(
                     asyncCommandQueues[candidate.node], selected)
      BY <1>1, <2>1, FifoRuntimeQueueCursorFacts,
         NextNodeCommandIndexFacts
         DEF NextNodeCommand
    <2> QED BY <2>1, <2>2, UniqueRemovalDeletesSelectedValue
         DEF AsyncQueueTyped
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage3FifoStrictlyProgresses ==
  \A candidate, position:
    /\ HistoricalTemporalStage3Pending(candidate, position)
    /\ SerializedRunnerRuntimeStep(candidate.node)
    /\ FifoRuntimeStep(candidate.node)
    => HistoricalTemporalStage3RankExit(candidate, position)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HistoricalTemporalSelectedTargetLeavesRuntimeQueue,
   UniqueSchedulerPrefixUsesTargetIndex,
   Stage3CandidateSequenceIndexCharacterization,
   PrefixCardinalityAfterNonTargetRemoval,
   NonTargetRemovalPreservesShiftedTarget,
   SelectedSameClassPrecedesTarget,
   SelectedDifferentClassStrictlyAdvancesCursor,
   FifoRuntimeQueueCursorFacts, NextNodeCommandIndexFacts,
   FS_RemoveElement, Isa
   DEF HistoricalTemporalStage3RankExit,
       HistoricalTemporalRankProgressExit,
       HistoricalTemporalStage3Pending,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, SchedulerServiceRank,
       SchedulerClassPrefixIndices, SchedulerCandidateIndices,
       ClassPrefixThrough, RemovalTargetIndex, FifoRuntimeStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       RemoveNextNodeCommand, NextNodeCommand, SequenceSet,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant, AsyncAllVars

HistoricalTemporalStage3AuxBlocked(
    candidate, position, rank) ==
  /\ HistoricalTemporalStage3Pending(candidate, position)
  /\ ~HistoricalTemporalStage3RankExit(candidate, position)
  /\ ReadyRunAuxRank(candidate.node) = rank

HistoricalTemporalStage3AuxProgress(
    candidate, position, rank) ==
  \/ HistoricalTemporalStage3RankExit(candidate, position)
  \/ \E lower \in SetLessThan(
       rank, ReadyRunAuxOrdering, ReadyRunAuxCarrier):
       HistoricalTemporalStage3AuxBlocked(
         candidate, position, lower)

HistoricalTemporalStage3ServeEpisodeResidual(
    candidate, position, rank) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalTemporalStage3Pending(candidate, position)
  /\ ~HistoricalTemporalStage3AuxProgress(candidate, position, rank)
  /\ \/ /\ AsyncServeIngressLifecycleOwnerIdentities(candidate.node) # {}
        /\ asyncRunnerPhase[candidate.node] = "Ingress"
     \/ AsyncCandidateProducerContinuationRunnerResolutionRequired(
          candidate.node)

HistoricalTemporalStage3CandidateProducerContinuationReentry(
    candidate, position, rank) ==
  /\ HistoricalTemporalStage3AuxBlocked(candidate, position, rank)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
       candidate.node)

HistoricalTemporalStage3FiniteServeEpisodeResidualProperty(
    specification) ==
  specification
    => \A candidate, position:
         \A rank \in ReadyRunAuxCarrier:
           HistoricalTemporalStage3ServeEpisodeResidual(
             candidate, position, rank)
             ~> HistoricalTemporalStage3AuxProgress(
                  candidate, position, rank)

THEOREM HistoricalTemporalStage3AuxRankInCarrier ==
  \A candidate, position:
    HistoricalTemporalStage3Pending(candidate, position)
      => ReadyRunAuxRank(candidate.node) \in ReadyRunAuxCarrier
BY HistoricalTemporalStage3CarrierFacts,
   ReadyRunAuxRankInCarrier,
   AsyncStrongTypeProjectsAsyncType
   DEF HistoricalTemporalStage3Pending

THEOREM HistoricalTemporalStage3SameRunnerAuxOutcome ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      /\ HistoricalTemporalStage3AuxBlocked(
           candidate, position, rank)
      /\ PostGstRunHistoricalRecoveryNode(candidate.node)
      => \/ HistoricalTemporalStage3AuxProgress(
              candidate, position, rank)'
         \/ HistoricalTemporalStage3ServeEpisodeResidual(
              candidate, position, rank)'
         \/ /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(
                  candidate.node)
            /\ HistoricalTemporalStage3CandidateProducerContinuationReentry(
                 candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
   IngressDrainStrictlyDecreasesRuntimeReach,
   HistoricalTemporalStage3FifoStrictlyProgresses,
   RunNodeWorkConcreteActionCaseSplit,
   ReadyRunAuxRankInCarrier,
   IsaT(600)
   DEF HistoricalTemporalStage3AuxProgress,
       HistoricalTemporalStage3ServeEpisodeResidual,
       HistoricalTemporalStage3CandidateProducerContinuationReentry,
       HistoricalTemporalStage3AuxBlocked,
       HistoricalTemporalStage3Pending,
       HistoricalTemporalStage3RankExit,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering,
       ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
       ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
       ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
       ReadyRunInnerCarrier, ReadyFifoDebt,
       ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       PostGstRunHistoricalRecoveryNode,
       RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncSchedulerExceptCausalControlAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       AsyncServeIngressLifecycleOwnerIdentities,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       IngressDrainStep, DrainFairIngressSelected,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, RuntimeStep,
       DeferredDrainStep, RemoveNextDeferredCommand,
       DiscardCommand, AdvanceNextDeferredClass,
       DeferredQueueNonempty, DeferredTagStep,
       DeferredTimeoutStep, DeferredRetransmitStep,
       DirectTimeoutStep, TimeoutDue, FifoRuntimeStep,
       DirectRetransmitStep, ProducerCompletionCanAdmit,
       CanEnqueueClass, AsyncQueueDepth, NodeQueueNonempty,
       IdleRuntimeStep, CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalStage3OtherStepUnlessAuxDescent ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      /\ HistoricalTemporalStage3AuxBlocked(
           candidate, position, rank)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~PostGstRunHistoricalRecoveryNode(candidate.node)
      => \/ HistoricalTemporalStage3AuxBlocked(
              candidate, position, rank)'
         \/ HistoricalTemporalStage3AuxProgress(
              candidate, position, rank)'
         \/ HistoricalTemporalStage3ServeEpisodeResidual(
              candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HistoricalTemporalStage3SameRunnerAuxOutcome,
   HeadTailProperties, SequenceSetAfterAppend,
   IsaT(600)
   DEF HistoricalTemporalStage3AuxBlocked,
       HistoricalTemporalStage3AuxProgress,
       HistoricalTemporalStage3ServeEpisodeResidual,
       HistoricalTemporalStage3CandidateProducerContinuationReentry,
       HistoricalTemporalStage3Pending,
       HistoricalTemporalStage3RankExit,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering,
       ReadyRunAuxCarrier, ReadyRunDeferredOrdering,
       ReadyRunDeferredCarrier, ReadyRunTimeoutOrdering,
       ReadyRunTimeoutCarrier, ReadyRunInnerOrdering,
       ReadyRunInnerCarrier, ReadyFifoDebt,
       ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       PostGstRunHistoricalRecoveryNode,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       RunHistoricalServer, OpenHistoricalRecovery,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, RuntimeStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncTick, AsyncSetGST,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstCrash,
       CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       AsyncAllVars

THEOREM HistoricalTemporalStage3RunnerIsNonstuttering ==
  \A candidate, position:
    /\ HistoricalTemporalStage3Pending(candidate, position)
    /\ PostGstRunHistoricalRecoveryNode(candidate.node)
    => <<PostGstRunHistoricalRecoveryNode(
           candidate.node)>>_AsyncAllVars
BY HistoricalTemporalStage3CarrierFacts,
   AsyncStrongTypeProjectsAsyncType,
   HistoricalTemporalRunNodeIsNonstuttering

THEOREM HistoricalTemporalStage3EnablesFairRunner ==
  \A candidate, position:
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ HistoricalTemporalStage3Pending(candidate, position)
      => ENABLED
           <<PostGstRunHistoricalRecoveryNode(
               candidate.node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                AsyncCandidateProducerContinuationExternalCoverageInvariant,
                AsyncCandidateProducerContinuationLocalReplayCapacityInvariant,
                HistoricalTemporalStage3Pending(candidate, position)
         PROVE ENABLED
                 <<PostGstRunHistoricalRecoveryNode(
                     candidate.node)>>_AsyncAllVars
    <2>1. /\ candidate.node \in asyncHistoricalRecoveryTargets
           /\ AsyncStrongTypeInvariant
           /\ gst
      BY <1>1, HistoricalTemporalStage3CarrierFacts
         DEF HistoricalTemporalStage3Pending,
             HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedCandidateOwned,
             HistoricalRecoveryTarget
    <2>2. ENABLED
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>1, HistoricalRecoveryRunnerEnabledAfterGst
    <2>3. PostGstRunHistoricalRecoveryNode(candidate.node)
             => <<PostGstRunHistoricalRecoveryNode(
                    candidate.node)>>_AsyncAllVars
      BY <1>1, HistoricalTemporalStage3RunnerIsNonstuttering
    <2> QED BY <2>2, <2>3, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage3AuxOneStep ==
  \A initialContext, candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      HistoricalTemporalStage3FiniteServeEpisodeResidualProperty(
        AsyncSpecAt(initialContext))
        => (AsyncSpecAt(initialContext)
              => (HistoricalTemporalStage3AuxBlocked(
                    candidate, position, rank)
                    ~> HistoricalTemporalStage3AuxProgress(
                         candidate, position, rank)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW rank \in ReadyRunAuxCarrier,
                HistoricalTemporalStage3FiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage3AuxBlocked(
                       candidate, position, rank)
                       ~> HistoricalTemporalStage3AuxProgress(
                            candidate, position, rank))
    <2>1. /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
           /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
             /\ HistoricalTemporalStage3AuxBlocked(
               candidate, position, rank)
             /\ ~(HistoricalTemporalStage3AuxProgress(
                    candidate, position, rank)
                    \/ HistoricalTemporalStage3ServeEpisodeResidual(
                         candidate, position, rank))
            => ENABLED
                 <<PostGstRunHistoricalRecoveryNode(
                     candidate.node)>>_AsyncAllVars
      BY HistoricalTemporalStage3EnablesFairRunner
         DEF HistoricalTemporalStage3AuxBlocked
    <2>2. /\ HistoricalTemporalStage3AuxBlocked(
               candidate, position, rank)
             /\ ~(HistoricalTemporalStage3AuxProgress(
                    candidate, position, rank)
                    \/ HistoricalTemporalStage3ServeEpisodeResidual(
                         candidate, position, rank))
             /\ <<PostGstRunHistoricalRecoveryNode(
                    candidate.node)>>_AsyncAllVars
            => \/ HistoricalTemporalStage3AuxProgress(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage3ServeEpisodeResidual(
                    candidate, position, rank)'
      BY HistoricalTemporalStage3SameRunnerAuxOutcome
         DEF HistoricalTemporalStage3ServeEpisodeResidual,
             HistoricalTemporalStage3CandidateProducerContinuationReentry
    <2>3. HistoricalTemporalStage3AuxBlocked(
             candidate, position, rank)
             /\ [AsyncNext]_AsyncAllVars
            => \/ HistoricalTemporalStage3AuxBlocked(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage3AuxProgress(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage3ServeEpisodeResidual(
                    candidate, position, rank)'
      BY HistoricalTemporalStage3SameRunnerAuxOutcome,
         HistoricalTemporalStage3OtherStepUnlessAuxDescent, Isa
         DEF HistoricalTemporalStage3CandidateProducerContinuationReentry
    <2>4. CASE candidate.node \in Responsive
      <3>1. AsyncSpecAt(initialContext)
               => /\ []AsyncCandidateProducerContinuationExternalCoverageInvariant
                  /\ []AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
                  /\ WF_AsyncAllVars(
                       PostGstRunHistoricalRecoveryNode(candidate.node))
        BY <2>4,
           AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
           AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity
           DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage3AuxBlocked(
                     candidate, position, rank)
                     ~> (HistoricalTemporalStage3AuxProgress(
                           candidate, position, rank)
                          \/ HistoricalTemporalStage3ServeEpisodeResidual(
                               candidate, position, rank)))
        BY <2>1, <2>2, <2>3, <3>1, PTL DEF AsyncSpecAt
      <3>3. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage3ServeEpisodeResidual(
                     candidate, position, rank)
                     ~> HistoricalTemporalStage3AuxProgress(
                          candidate, position, rank))
        BY <1>1
           DEF HistoricalTemporalStage3FiniteServeEpisodeResidualProperty
      <3>4. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage3AuxBlocked(
                     candidate, position, rank)
                     ~> HistoricalTemporalStage3AuxProgress(
                          candidate, position, rank))
        BY <3>2, <3>3, PTL
      <3> QED BY <3>4
    <2>5. CASE candidate.node \notin Responsive
      <3>1. AsyncSpecAt(initialContext)
               => []~HistoricalTemporalStage3AuxBlocked(
                      candidate, position, rank)
        BY <2>5, PTL
           DEF HistoricalTemporalStage3AuxBlocked,
               HistoricalTemporalStage3Pending,
               HistoricalProtectedOwnedAtServiceRank,
               HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage3AuxRankDescent ==
  \A initialContext, candidate, position:
    HistoricalTemporalStage3FiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => \A rank \in ReadyRunAuxCarrier:
                 HistoricalTemporalStage3AuxBlocked(
                   candidate, position, rank)
                   ~> HistoricalTemporalStage3RankExit(
                        candidate, position))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                HistoricalTemporalStage3FiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => \A rank \in ReadyRunAuxCarrier:
                      HistoricalTemporalStage3AuxBlocked(
                        candidate, position, rank)
                        ~> HistoricalTemporalStage3RankExit(
                             candidate, position)
    <2>1. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  HistoricalTemporalStage3AuxBlocked(
                    candidate, position, rank)
                    ~> (HistoricalTemporalStage3RankExit(
                          candidate, position)
                         \/ \E lower \in SetLessThan(
                              rank, ReadyRunAuxOrdering,
                              ReadyRunAuxCarrier):
                              HistoricalTemporalStage3AuxBlocked(
                                candidate, position, lower))
      BY HistoricalTemporalFairStage3AuxOneStep
         DEF HistoricalTemporalStage3AuxProgress
    <2> QED BY <2>1, ReadyRunAuxOrderingIsWellFounded,
         WellFoundedLeadsTo
  <1> QED BY <1>1

THEOREM AsyncSpecClosesHistoricalTemporalStage3Leaf ==
  \A initialContext:
    HistoricalTemporalStage3FiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => HistoricalProtectedStage3RankProgressProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                HistoricalTemporalStage3FiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext)),
                NEW candidate \in AsyncCandidateSet,
                NEW position \in Nat
         PROVE AsyncSpecAt(initialContext)
                 => ((gst
                       /\ HistoricalProtectedCandidateOwned(candidate)
                       /\ CandidateServiceRank(candidate)
                            = <<3, position>>)
                      ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                           \/ \E lower \in SetLessThan(
                                <<3, position>>,
                                OwnedServiceRankOrdering,
                                OwnedServiceRankCarrier):
                                HistoricalProtectedOwnedAtServiceRank(
                                  candidate, lower)))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>2. AsyncSpecAt(initialContext)
             => ((gst
                   /\ HistoricalProtectedCandidateOwned(candidate)
                   /\ CandidateServiceRank(candidate) = <<3, position>>)
                  ~> HistoricalTemporalStage3AuxBlocked(
                       candidate, position,
                       ReadyRunAuxRank(candidate.node)))
      BY <2>1, HistoricalTemporalStage3AuxRankInCarrier, PTL
         DEF HistoricalTemporalStage3Pending,
             HistoricalTemporalStage3AuxBlocked,
             HistoricalTemporalStage3RankExit,
             HistoricalTemporalRankProgressExit,
             HistoricalProtectedOwnedAtServiceRank
    <2>3. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  HistoricalTemporalStage3AuxBlocked(
                    candidate, position, rank)
                    ~> HistoricalTemporalStage3RankExit(
                         candidate, position)
      BY HistoricalTemporalFairStage3AuxRankDescent
    <2>4. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage3RankExit(
                   candidate, position)
                   ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                        \/ \E lower \in SetLessThan(
                             <<3, position>>,
                             OwnedServiceRankOrdering,
                             OwnedServiceRankCarrier):
                             HistoricalProtectedOwnedAtServiceRank(
                               candidate, lower)))
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType,
         HistoricalTemporalRankExitHasWellFoundedSuccessor, PTL
         DEF HistoricalTemporalStage3RankExit,
             HistoricalTemporalRankProgressExit
    <2> QED BY <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1
       DEF HistoricalProtectedStage3RankProgressProperty,
           HistoricalProtectedStageRankProgressProperty

(***************************************************************************
Stage 4: historical-target ready-completion service.

The branch tag makes the ordinary three-part Stage-4 proof explicit in one
well-founded rank: an actionable Local owner is lowest, causal-capacity debt
is next, and the remaining runner prefix is highest.  The tail retains the
existing capacity and runner-prefix ranks.  Consequently a transition from a
prefix blocker to capacity debt, or from capacity debt to an actionable
owner, is strict independently of the unused tail coordinates.
***************************************************************************)

HistoricalTemporalStage4Pending(candidate, position) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalProtectedOwnedAtServiceRank(
       candidate, <<4, position>>)

HistoricalTemporalStage4Branch(candidate) ==
  IF ReadyStage4Actionable(candidate)
  THEN 0
  ELSE IF NonCompletionCausalAdmissionDebt(candidate.node)
       THEN 1
       ELSE 2

HistoricalTemporalStage4TailCarrier ==
  Stage4CapacityCarrier \X ReadyRunAuxCarrier

HistoricalTemporalStage4TailOrdering ==
  LexPairOrdering(
    Stage4CapacityOrdering, ReadyRunAuxOrdering,
    Stage4CapacityCarrier, ReadyRunAuxCarrier)

HistoricalTemporalStage4EpisodeCarrier ==
  (0..2) \X HistoricalTemporalStage4TailCarrier

HistoricalTemporalStage4EpisodeOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), HistoricalTemporalStage4TailOrdering,
    0..2, HistoricalTemporalStage4TailCarrier)

HistoricalTemporalStage4EpisodeRank(candidate) ==
  <<HistoricalTemporalStage4Branch(candidate),
    <<Stage4CapacityRank(candidate.node),
      ReadyRunAuxRank(candidate.node)>>>

HistoricalTemporalStage4BlockedAtRank(
    candidate, position, rank) ==
  /\ HistoricalTemporalStage4Pending(candidate, position)
  /\ ~HistoricalTemporalRankProgressExit(
       candidate, <<4, position>>)
  /\ HistoricalTemporalStage4EpisodeRank(candidate) = rank

HistoricalTemporalStage4Progress(candidate, position, rank) ==
  \/ HistoricalTemporalRankProgressExit(candidate, <<4, position>>)
  \/ \E lower \in SetLessThan(
       rank,
       HistoricalTemporalStage4EpisodeOrdering,
       HistoricalTemporalStage4EpisodeCarrier):
       HistoricalTemporalStage4BlockedAtRank(
         candidate, position, lower)

HistoricalTemporalStage4ServeEpisodeResidual(
    candidate, position, rank) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalTemporalStage4Pending(candidate, position)
  /\ ~HistoricalTemporalStage4Progress(candidate, position, rank)
  /\ \/ /\ AsyncServeIngressLifecycleOwnerIdentities(candidate.node) # {}
        /\ asyncRunnerPhase[candidate.node] = "Ingress"
     \/ AsyncCandidateProducerContinuationRunnerResolutionRequired(
          candidate.node)

HistoricalTemporalStage4CandidateProducerContinuationReentry(
    candidate, position, rank) ==
  /\ HistoricalTemporalStage4BlockedAtRank(candidate, position, rank)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
       candidate.node)

HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
    specification) ==
  specification
    => \A candidate, position:
         \A rank \in HistoricalTemporalStage4EpisodeCarrier:
           HistoricalTemporalStage4ServeEpisodeResidual(
             candidate, position, rank)
             ~> HistoricalTemporalStage4Progress(
                  candidate, position, rank)

THEOREM HistoricalTemporalStage4EpisodeOrderingIsWellFounded ==
  IsWellFoundedOn(
    HistoricalTemporalStage4EpisodeOrdering,
    HistoricalTemporalStage4EpisodeCarrier)
PROOF
  <1>1. IsWellFoundedOn(
          HistoricalTemporalStage4TailOrdering,
          HistoricalTemporalStage4TailCarrier)
    BY Stage4CapacityOrderingIsWellFounded,
       ReadyRunAuxOrderingIsWellFounded,
       WFLexPairOrdering
       DEF HistoricalTemporalStage4TailOrdering,
           HistoricalTemporalStage4TailCarrier
  <1>2. IsWellFoundedOn(OpToRel(<, Nat), 0..2)
    BY NatLessThanWellFounded, IsWellFoundedOnSubset, Isa
  <1> QED BY <1>1, <1>2, WFLexPairOrdering
       DEF HistoricalTemporalStage4EpisodeOrdering,
           HistoricalTemporalStage4EpisodeCarrier

THEOREM HistoricalTemporalStage4CarrierFacts ==
  \A candidate, position:
    HistoricalTemporalStage4Pending(candidate, position)
      => /\ candidate.node \in Responsive
         /\ HistoricalRecoveryTarget(candidate.node)
         /\ candidate.node \in ValidatorIds
         /\ candidate \in asyncOutstandingWork[candidate.node]
         /\ CandidateInReadyQueue(candidate)
         /\ ReadyCandidatePosition(candidate) = position
         /\ SelectedCompletionQueueNonempty(candidate.node)
         /\ HistoricalTemporalStage4EpisodeRank(candidate)
              \in HistoricalTemporalStage4EpisodeCarrier
BY AsyncStrongTypeProjectsAsyncType,
   HistoricalRecoveryTargetsAreValidators,
   Stage4CapacityRankInCarrier,
   ReadyRunAuxRankInCarrier, Isa
   DEF HistoricalTemporalStage4Pending,
       HistoricalTemporalStage4EpisodeRank,
       HistoricalTemporalStage4Branch,
       HistoricalTemporalStage4EpisodeCarrier,
       HistoricalTemporalStage4TailCarrier,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       CandidateInReadyQueue, ReadyCandidatePosition,
       ReadyCandidateSource, ReadyCompletionQueue,
       SelectedCompletionQueueNonempty, SelectedCompletionSource,
       AsyncProgressOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, CandidateScheduled, SequenceSet

THEOREM HistoricalTemporalStage4SameRunnerProducesOutcome ==
  \A candidate, position:
    \A rank \in HistoricalTemporalStage4EpisodeCarrier:
      /\ HistoricalTemporalStage4BlockedAtRank(
           candidate, position, rank)
      /\ PostGstRunHistoricalRecoveryNode(candidate.node)
      => \/ HistoricalTemporalStage4Progress(
              candidate, position, rank)'
         \/ HistoricalTemporalStage4ServeEpisodeResidual(
              candidate, position, rank)'
         \/ /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(
                  candidate.node)
            /\ HistoricalTemporalStage4CandidateProducerContinuationReentry(
                 candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   ProducerAdmissionRecordsCausalDebt,
   OwedAdmissibleCausalCannotBeOvertaken,
   CandidateSequenceIndexIsPosition,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
   IngressDrainStrictlyDecreasesRuntimeReach,
   CausalCommandCapacityDebtIsNatural,
   Stage4CapacityRankInCarrier,
   ReadyRunAuxRankInCarrier,
   RunNodeWorkConcreteActionCaseSplit,
   HeadTailProperties, SequenceSetAfterAppend,
   IsaT(600)
   DEF HistoricalTemporalStage4Progress,
       HistoricalTemporalStage4ServeEpisodeResidual,
       HistoricalTemporalStage4CandidateProducerContinuationReentry,
       HistoricalTemporalStage4BlockedAtRank,
       HistoricalTemporalStage4Pending,
       HistoricalTemporalRankProgressExit,
       HistoricalTemporalStage4EpisodeRank,
       HistoricalTemporalStage4Branch,
       HistoricalTemporalStage4EpisodeOrdering,
       HistoricalTemporalStage4EpisodeCarrier,
       HistoricalTemporalStage4TailOrdering,
       HistoricalTemporalStage4TailCarrier,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, ReadyStage4Actionable,
       NonCompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive, CausalHeadCanAdvance,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier,
       ReadyRunAuxRank, ReadyRunDeferredRank,
       ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       CandidateInReadyQueue, ReadyCandidatePosition,
       ReadyCandidateSource, ReadyCompletionQueue,
       SelectedCompletionSource, SelectedCompletionCandidate,
       SelectedCompletionQueueNonempty, OtherLocalSource,
       LocalSourceDistance, CandidateSequenceIndex,
       PostGstRunHistoricalRecoveryNode,
       RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncSchedulerExceptCausalControlAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       AsyncServeIngressLifecycleOwnerIdentities,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata,
       SelectedLocalSource, PreferredLocalSource,
       IngressDrainStep, DrainFairIngressSelected,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, RuntimeStep,
       DeferredDrainStep, DeferredTagStep,
       DirectTimeoutStep, DirectRetransmitStep,
       FifoRuntimeStep, IdleRuntimeStep,
       EnqueueCandidate, RemoveNextNodeCommand,
       DeferCommand, DiscardCommand,
       CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       AsyncIoVars, AsyncDeferredVars, AsyncAllVars, vars

THEOREM HistoricalTemporalStage4OtherStepUnlessProgress ==
  \A candidate, position:
    \A rank \in HistoricalTemporalStage4EpisodeCarrier:
      /\ HistoricalTemporalStage4BlockedAtRank(
           candidate, position, rank)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~PostGstRunHistoricalRecoveryNode(candidate.node)
      => \/ HistoricalTemporalStage4BlockedAtRank(
              candidate, position, rank)'
         \/ HistoricalTemporalStage4Progress(
              candidate, position, rank)'
         \/ HistoricalTemporalStage4ServeEpisodeResidual(
              candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HistoricalTemporalStage4SameRunnerProducesOutcome,
   CandidateSequenceIndexIsPosition,
   CausalCommandCapacityDebtIsNatural,
   Stage4CapacityRankInCarrier,
   ReadyRunAuxRankInCarrier,
   HeadTailProperties, SequenceSetAfterAppend,
   IsaT(600)
   DEF HistoricalTemporalStage4BlockedAtRank,
       HistoricalTemporalStage4Progress,
       HistoricalTemporalStage4ServeEpisodeResidual,
       HistoricalTemporalStage4CandidateProducerContinuationReentry,
       HistoricalTemporalStage4Pending,
       HistoricalTemporalRankProgressExit,
       HistoricalTemporalStage4EpisodeRank,
       HistoricalTemporalStage4Branch,
       HistoricalTemporalStage4EpisodeOrdering,
       HistoricalTemporalStage4EpisodeCarrier,
       HistoricalTemporalStage4TailOrdering,
       HistoricalTemporalStage4TailCarrier,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, ReadyStage4Actionable,
       NonCompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive, CausalHeadCanAdvance,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier,
       ReadyRunAuxRank, ReadyRunDeferredRank,
       ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       CandidateInReadyQueue, ReadyCandidatePosition,
       ReadyCandidateSource, ReadyCompletionQueue,
       SelectedCompletionSource, SelectedCompletionCandidate,
       SelectedCompletionQueueNonempty,
       PostGstRunHistoricalRecoveryNode,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       RunHistoricalServer, OpenHistoricalRecovery,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncTick, AsyncSetGST,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstCrash,
       CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       AsyncAllVars

THEOREM HistoricalTemporalStage4UnlessProgress ==
  \A candidate, position:
    /\ HistoricalTemporalStage4Pending(candidate, position)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalTemporalStage4Pending(candidate, position)'
       \/ HistoricalTemporalRankProgressExit(
            candidate, <<4, position>>)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                HistoricalTemporalStage4Pending(candidate, position),
                [AsyncNext]_AsyncAllVars
         PROVE \/ HistoricalTemporalStage4Pending(
                      candidate, position)'
               \/ HistoricalTemporalRankProgressExit(
                    candidate, <<4, position>>)'
    <2>1. HistoricalTemporalStage4EpisodeRank(candidate)
             \in HistoricalTemporalStage4EpisodeCarrier
      BY <1>1, HistoricalTemporalStage4CarrierFacts
    <2>2. HistoricalTemporalStage4BlockedAtRank(
             candidate, position,
             HistoricalTemporalStage4EpisodeRank(candidate))
      BY <1>1, Isa
         DEF HistoricalTemporalStage4BlockedAtRank,
             HistoricalTemporalRankProgressExit,
             HistoricalTemporalStage4Pending,
             HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedServiceOwnershipExit,
             ServiceRankLess
    <2>3. CASE PostGstRunHistoricalRecoveryNode(candidate.node)
      <3>1. \/ HistoricalTemporalStage4Progress(
                    candidate, position,
                    HistoricalTemporalStage4EpisodeRank(candidate))'
             \/ HistoricalTemporalStage4ServeEpisodeResidual(
                  candidate, position,
                  HistoricalTemporalStage4EpisodeRank(candidate))'
        BY <2>1, <2>2, <3>1,
           HistoricalTemporalStage4SameRunnerProducesOutcome
      <3> QED BY <3>1, Isa
           DEF HistoricalTemporalStage4Progress,
               HistoricalTemporalStage4ServeEpisodeResidual,
               HistoricalTemporalStage4CandidateProducerContinuationReentry,
               HistoricalTemporalStage4BlockedAtRank
    <2>4. CASE ~PostGstRunHistoricalRecoveryNode(candidate.node)
      <3>1. \/ HistoricalTemporalStage4BlockedAtRank(
                    candidate, position,
                    HistoricalTemporalStage4EpisodeRank(candidate))'
             \/ HistoricalTemporalStage4Progress(
                  candidate, position,
                  HistoricalTemporalStage4EpisodeRank(candidate))'
             \/ HistoricalTemporalStage4ServeEpisodeResidual(
                  candidate, position,
                  HistoricalTemporalStage4EpisodeRank(candidate))'
        BY <2>1, <2>2, <1>1, <2>4,
           HistoricalTemporalStage4OtherStepUnlessProgress
      <3> QED BY <3>1, Isa
           DEF HistoricalTemporalStage4Progress,
               HistoricalTemporalStage4ServeEpisodeResidual,
               HistoricalTemporalStage4BlockedAtRank
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage4RunnerIsNonstuttering ==
  \A candidate, position, rank:
    /\ HistoricalTemporalStage4BlockedAtRank(
         candidate, position, rank)
    /\ PostGstRunHistoricalRecoveryNode(candidate.node)
    => <<PostGstRunHistoricalRecoveryNode(
           candidate.node)>>_AsyncAllVars
BY HistoricalTemporalStage4CarrierFacts,
   AsyncStrongTypeProjectsAsyncType,
   HistoricalTemporalRunNodeIsNonstuttering
   DEF HistoricalTemporalStage4BlockedAtRank

THEOREM HistoricalTemporalStage4EnablesFairRunner ==
  \A candidate, position, rank:
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ HistoricalTemporalStage4BlockedAtRank(
         candidate, position, rank)
      => ENABLED
           <<PostGstRunHistoricalRecoveryNode(
               candidate.node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW candidate, NEW position, NEW rank,
                AsyncCandidateProducerContinuationExternalCoverageInvariant,
                AsyncCandidateProducerContinuationLocalReplayCapacityInvariant,
                HistoricalTemporalStage4BlockedAtRank(
                  candidate, position, rank)
         PROVE ENABLED
                 <<PostGstRunHistoricalRecoveryNode(
                     candidate.node)>>_AsyncAllVars
    <2>1. /\ candidate.node \in asyncHistoricalRecoveryTargets
           /\ AsyncStrongTypeInvariant
           /\ gst
      BY <1>1, HistoricalTemporalStage4CarrierFacts
         DEF HistoricalTemporalStage4BlockedAtRank,
             HistoricalTemporalStage4Pending,
             HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedCandidateOwned,
             HistoricalRecoveryTarget
    <2>2. ENABLED
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>1, HistoricalRecoveryRunnerEnabledAfterGst
    <2>3. PostGstRunHistoricalRecoveryNode(candidate.node)
             => <<PostGstRunHistoricalRecoveryNode(
                    candidate.node)>>_AsyncAllVars
      BY <1>1, HistoricalTemporalStage4RunnerIsNonstuttering
    <2> QED BY <2>2, <2>3, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage4OneStep ==
  \A initialContext, candidate, position:
    \A rank \in HistoricalTemporalStage4EpisodeCarrier:
      HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
        AsyncSpecAt(initialContext))
        => (AsyncSpecAt(initialContext)
              => (HistoricalTemporalStage4BlockedAtRank(
                    candidate, position, rank)
                    ~> HistoricalTemporalStage4Progress(
                         candidate, position, rank)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW rank \in HistoricalTemporalStage4EpisodeCarrier,
                HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage4BlockedAtRank(
                       candidate, position, rank)
                       ~> HistoricalTemporalStage4Progress(
                            candidate, position, rank))
    <2>1. /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
           /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
             /\ HistoricalTemporalStage4BlockedAtRank(
               candidate, position, rank)
             /\ ~(HistoricalTemporalStage4Progress(
                    candidate, position, rank)
                    \/ HistoricalTemporalStage4ServeEpisodeResidual(
                         candidate, position, rank))
            => ENABLED
                 <<PostGstRunHistoricalRecoveryNode(
                     candidate.node)>>_AsyncAllVars
      BY HistoricalTemporalStage4EnablesFairRunner
    <2>2. /\ HistoricalTemporalStage4BlockedAtRank(
               candidate, position, rank)
             /\ ~(HistoricalTemporalStage4Progress(
                    candidate, position, rank)
                    \/ HistoricalTemporalStage4ServeEpisodeResidual(
                         candidate, position, rank))
             /\ <<PostGstRunHistoricalRecoveryNode(
                    candidate.node)>>_AsyncAllVars
            => \/ HistoricalTemporalStage4Progress(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage4ServeEpisodeResidual(
                    candidate, position, rank)'
      BY HistoricalTemporalStage4SameRunnerProducesOutcome
         DEF HistoricalTemporalStage4ServeEpisodeResidual,
             HistoricalTemporalStage4CandidateProducerContinuationReentry
    <2>3. HistoricalTemporalStage4BlockedAtRank(
             candidate, position, rank)
             /\ [AsyncNext]_AsyncAllVars
            => \/ HistoricalTemporalStage4BlockedAtRank(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage4Progress(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage4ServeEpisodeResidual(
                    candidate, position, rank)'
      BY HistoricalTemporalStage4SameRunnerProducesOutcome,
         HistoricalTemporalStage4OtherStepUnlessProgress, Isa
         DEF HistoricalTemporalStage4CandidateProducerContinuationReentry
    <2>4. CASE candidate.node \in Responsive
      <3>1. AsyncSpecAt(initialContext)
               => /\ []AsyncCandidateProducerContinuationExternalCoverageInvariant
                  /\ []AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
                  /\ WF_AsyncAllVars(
                       PostGstRunHistoricalRecoveryNode(candidate.node))
        BY <2>4,
           AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
           AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity
           DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage4BlockedAtRank(
                     candidate, position, rank)
                     ~> (HistoricalTemporalStage4Progress(
                           candidate, position, rank)
                          \/ HistoricalTemporalStage4ServeEpisodeResidual(
                               candidate, position, rank)))
        BY <2>1, <2>2, <2>3, <3>1, PTL DEF AsyncSpecAt
      <3>3. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage4ServeEpisodeResidual(
                     candidate, position, rank)
                     ~> HistoricalTemporalStage4Progress(
                          candidate, position, rank))
        BY <1>1
           DEF HistoricalTemporalStage4FiniteServeEpisodeResidualProperty
      <3>4. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage4BlockedAtRank(
                     candidate, position, rank)
                     ~> HistoricalTemporalStage4Progress(
                          candidate, position, rank))
        BY <3>2, <3>3, PTL
      <3> QED BY <3>4
    <2>5. CASE candidate.node \notin Responsive
      <3>1. AsyncSpecAt(initialContext)
               => []~HistoricalTemporalStage4BlockedAtRank(
                      candidate, position, rank)
        BY <2>5, PTL
           DEF HistoricalTemporalStage4BlockedAtRank,
               HistoricalTemporalStage4Pending,
               HistoricalProtectedOwnedAtServiceRank,
               HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage4RankDescent ==
  \A initialContext, candidate, position:
    HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => \A rank \in HistoricalTemporalStage4EpisodeCarrier:
                 HistoricalTemporalStage4BlockedAtRank(
                   candidate, position, rank)
                   ~> HistoricalTemporalRankProgressExit(
                        candidate, <<4, position>>))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => \A rank \in HistoricalTemporalStage4EpisodeCarrier:
                      HistoricalTemporalStage4BlockedAtRank(
                        candidate, position, rank)
                        ~> HistoricalTemporalRankProgressExit(
                             candidate, <<4, position>>)
    <2>1. AsyncSpecAt(initialContext)
             => \A rank \in HistoricalTemporalStage4EpisodeCarrier:
                  HistoricalTemporalStage4BlockedAtRank(
                    candidate, position, rank)
                    ~> (HistoricalTemporalRankProgressExit(
                          candidate, <<4, position>>)
                         \/ \E lower \in SetLessThan(
                              rank,
                              HistoricalTemporalStage4EpisodeOrdering,
                              HistoricalTemporalStage4EpisodeCarrier):
                              HistoricalTemporalStage4BlockedAtRank(
                                candidate, position, lower))
      BY HistoricalTemporalFairStage4OneStep
         DEF HistoricalTemporalStage4Progress
    <2> QED BY <2>1,
         HistoricalTemporalStage4EpisodeOrderingIsWellFounded,
         WellFoundedLeadsTo
  <1> QED BY <1>1

THEOREM AsyncSpecClosesHistoricalTemporalStage4Leaf ==
  \A initialContext:
    HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => HistoricalProtectedStage4RankProgressProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext)),
                NEW candidate \in AsyncCandidateSet,
                NEW position \in Nat
         PROVE AsyncSpecAt(initialContext)
                 => ((gst
                       /\ HistoricalProtectedCandidateOwned(candidate)
                       /\ CandidateServiceRank(candidate)
                            = <<4, position>>)
                      ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                           \/ \E lower \in SetLessThan(
                                <<4, position>>,
                                OwnedServiceRankOrdering,
                                OwnedServiceRankCarrier):
                                HistoricalProtectedOwnedAtServiceRank(
                                  candidate, lower)))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>2. AsyncSpecAt(initialContext)
             => ((gst
                   /\ HistoricalProtectedCandidateOwned(candidate)
                   /\ CandidateServiceRank(candidate) = <<4, position>>)
                  ~> HistoricalTemporalStage4BlockedAtRank(
                       candidate, position,
                       HistoricalTemporalStage4EpisodeRank(candidate)))
      BY <2>1, HistoricalTemporalStage4CarrierFacts, PTL
         DEF HistoricalTemporalStage4BlockedAtRank,
             HistoricalTemporalStage4Pending,
             HistoricalTemporalRankProgressExit,
             HistoricalProtectedOwnedAtServiceRank
    <2>3. AsyncSpecAt(initialContext)
             => \A rank \in HistoricalTemporalStage4EpisodeCarrier:
                  HistoricalTemporalStage4BlockedAtRank(
                    candidate, position, rank)
                    ~> HistoricalTemporalRankProgressExit(
                         candidate, <<4, position>>)
      BY HistoricalTemporalFairStage4RankDescent
    <2>4. AsyncSpecAt(initialContext)
             => (HistoricalTemporalRankProgressExit(
                   candidate, <<4, position>>)
                   ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                        \/ \E lower \in SetLessThan(
                             <<4, position>>,
                             OwnedServiceRankOrdering,
                             OwnedServiceRankCarrier):
                             HistoricalProtectedOwnedAtServiceRank(
                               candidate, lower)))
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType,
         HistoricalTemporalRankExitHasWellFoundedSuccessor, PTL
         DEF HistoricalTemporalRankProgressExit
    <2> QED BY <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1
       DEF HistoricalProtectedStage4RankProgressProperty,
           HistoricalProtectedStageRankProgressProperty

(***************************************************************************
Stage 6: historical-target causal FIFO service.

The causal owner has four concrete subkernels.  The historical runner closes
the pre-admission prefix, the exact owed head, and non-Completion capacity.
Completion capacity first drains the physical FIFO with the dedicated
historical I/O worker and then delegates the selected ready Completion owner
to the historical Stage-4 leaf above.  Each owner consumes an existing
individually quantified fairness clause.
***************************************************************************)

HistoricalTemporalStage6Pending(candidate, position) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalProtectedOwnedAtServiceRank(
       candidate, <<6, position>>)

HistoricalTemporalStage6OwedCausalReady(candidate, position) ==
  /\ HistoricalTemporalStage6Pending(candidate, position)
  /\ CausalAdmissionDebtActive(candidate.node)
  /\ CausalHeadCanAdvance(candidate.node)

HistoricalTemporalStage6NonCompletionCapacityBlocked(
    candidate, position) ==
  /\ HistoricalTemporalStage6Pending(candidate, position)
  /\ NonCompletionCausalAdmissionDebt(candidate.node)
  /\ ~CausalHeadCanAdvance(candidate.node)

HistoricalTemporalStage6CompletionCapacityBlocked(
    candidate, position) ==
  /\ HistoricalTemporalStage6Pending(candidate, position)
  /\ CompletionCausalAdmissionDebt(candidate.node)
  /\ ~CausalHeadCanAdvance(candidate.node)

HistoricalTemporalStage6PreAdmissionGoal(candidate, position) ==
  \/ HistoricalTemporalRankProgressExit(candidate, <<6, position>>)
  \/ HistoricalTemporalStage6OwedCausalReady(candidate, position)
  \/ HistoricalTemporalStage6NonCompletionCapacityBlocked(
       candidate, position)
  \/ HistoricalTemporalStage6CompletionCapacityBlocked(
       candidate, position)

HistoricalTemporalStage6PreAdmissionBlockedAtAux(
    candidate, position, rank) ==
  /\ HistoricalTemporalStage6Pending(candidate, position)
  /\ ~HistoricalTemporalStage6PreAdmissionGoal(candidate, position)
  /\ ReadyRunAuxRank(candidate.node) = rank

HistoricalTemporalStage6PreAdmissionAuxProgress(
    candidate, position, rank) ==
  \/ HistoricalTemporalStage6PreAdmissionGoal(candidate, position)
  \/ \E lower \in SetLessThan(
       rank, ReadyRunAuxOrdering, ReadyRunAuxCarrier):
       HistoricalTemporalStage6PreAdmissionBlockedAtAux(
         candidate, position, lower)

HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
    candidate, position, rank) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalTemporalStage6Pending(candidate, position)
  /\ ~HistoricalTemporalStage6PreAdmissionAuxProgress(
       candidate, position, rank)
  /\ \/ asyncRunnerPhase[candidate.node] \in {"Local", "Ingress"}
     \/ AsyncCandidateProducerContinuationRunnerResolutionRequired(
          candidate.node)

HistoricalTemporalStage6PreAdmissionCandidateProducerContinuationReentry(
    candidate, position, rank) ==
  /\ HistoricalTemporalStage6PreAdmissionBlockedAtAux(
       candidate, position, rank)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
       candidate.node)

HistoricalTemporalStage6PreAdmissionFiniteRunnerEpisodeResidualProperty(
    specification) ==
  specification
    => \A candidate, position:
         \A rank \in ReadyRunAuxCarrier:
           HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
             candidate, position, rank)
             ~> HistoricalTemporalStage6PreAdmissionAuxProgress(
                  candidate, position, rank)

THEOREM HistoricalTemporalStage6CarrierFacts ==
  \A candidate, position:
    HistoricalTemporalStage6Pending(candidate, position)
      => /\ candidate.node \in Responsive
         /\ HistoricalRecoveryTarget(candidate.node)
         /\ candidate.node \in ValidatorIds
         /\ candidate \in CausalCandidates
         /\ candidate \in
              SequenceSet(asyncCausalQueues[candidate.node])
         /\ AsyncQueueTyped(asyncCausalQueues[candidate.node])
         /\ SequenceHasUniqueValues(
              asyncCausalQueues[candidate.node])
         /\ CausalQueueNonempty(candidate.node)
         /\ CausalCandidatePosition(candidate) = position
         /\ CandidateSequenceIndex(
              candidate, asyncCausalQueues[candidate.node])
              \in 1..Len(asyncCausalQueues[candidate.node])
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                HistoricalTemporalStage6Pending(candidate, position)
         PROVE /\ candidate.node \in Responsive
               /\ HistoricalRecoveryTarget(candidate.node)
               /\ candidate.node \in ValidatorIds
               /\ candidate \in CausalCandidates
               /\ candidate \in
                    SequenceSet(asyncCausalQueues[candidate.node])
               /\ AsyncQueueTyped(asyncCausalQueues[candidate.node])
               /\ SequenceHasUniqueValues(
                    asyncCausalQueues[candidate.node])
               /\ CausalQueueNonempty(candidate.node)
               /\ CausalCandidatePosition(candidate) = position
               /\ CandidateSequenceIndex(
                    candidate, asyncCausalQueues[candidate.node])
                    \in 1..Len(asyncCausalQueues[candidate.node])
    <2>1. /\ AsyncTypeInvariant
           /\ candidate.node \in Responsive
           /\ HistoricalRecoveryTarget(candidate.node)
           /\ CandidateScheduled(candidate)
           /\ CandidateServiceRank(candidate) = <<6, position>>
      BY <1>1, AsyncStrongTypeProjectsAsyncType
         DEF HistoricalTemporalStage6Pending,
             HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedCandidateOwned,
             ProtectedCandidateOwned
    <2>2. candidate.node \in ValidatorIds
      BY <2>1, ScheduledCandidateProjectsToOwner
    <2>3. candidate \in CausalCandidates
      BY <2>1, SMT DEF CandidateServiceRank, CandidateScheduled
    <2>4. candidate \in
             SequenceSet(asyncCausalQueues[candidate.node])
      BY <2>1, <2>3, ScheduledCandidateProjectsToOwner
    <2>5. /\ AsyncQueueTyped(asyncCausalQueues[candidate.node])
           /\ SequenceHasUniqueValues(
                asyncCausalQueues[candidate.node])
      BY <1>1, <2>2
         DEF HistoricalTemporalStage6Pending,
             AsyncStrongTypeInvariant,
             AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncCausalTypeInvariant,
             AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>6. CandidateSequenceIndex(
             candidate, asyncCausalQueues[candidate.node])
             \in 1..Len(asyncCausalQueues[candidate.node])
      BY <2>4, <2>5, CandidateSequenceIndexIsPosition
         DEF AsyncQueueTyped
    <2>7. CausalQueueNonempty(candidate.node)
      BY <2>5, <2>6, LenProperties, SMTT(30)
         DEF AsyncQueueTyped, CausalQueueNonempty
    <2>8. CausalCandidatePosition(candidate) = position
      BY <2>1, <2>3, SMT DEF CandidateServiceRank
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
         <2>8
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage6CausalAdmissionStrictlyProgresses ==
  \A candidate, position:
    /\ HistoricalTemporalStage6Pending(candidate, position)
    /\ AsyncStrongTypeInvariant'
    /\ AsyncProgressOwnershipInvariant'
    /\ AdmitCausalHead(candidate.node)
    => HistoricalTemporalRankProgressExit(
         candidate, <<6, position>>)'
BY HistoricalTemporalStage6CarrierFacts,
   CandidateSequenceIndexAfterNonTargetHead,
   CausalPredecessorPositionDrops,
   ConsensusIoCandidateProjection,
   UniqueSequenceTailSetFacts,
   UnionOfSequenceSetsAfterTailAtKey,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HeadTailProperties, SequenceSetAfterAppend,
   IsaT(600)
   DEF HistoricalTemporalStage6Pending,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, CausalCandidatePosition,
       LocalSourceDistance, PreferredLocalSource,
       AdmitCausalHead, HeadCausalCandidate,
       CandidateInReadyQueue, CandidateInIoQueue,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       CandidateScheduled, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalStage6PreAdmissionSameRunnerOutcome ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      /\ HistoricalTemporalStage6PreAdmissionBlockedAtAux(
           candidate, position, rank)
      /\ PostGstRunHistoricalRecoveryNode(candidate.node)
      => \/ HistoricalTemporalStage6PreAdmissionAuxProgress(
              candidate, position, rank)'
         \/ HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
              candidate, position, rank)'
         \/ /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(
                  candidate.node)
            /\ HistoricalTemporalStage6PreAdmissionCandidateProducerContinuationReentry(
                 candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
   IngressDrainStrictlyDecreasesRuntimeReach,
   HistoricalTemporalStage6CausalAdmissionStrictlyProgresses,
   RunNodeWorkConcreteActionCaseSplit,
   ReadyRunAuxRankInCarrier,
   HeadTailProperties, SequenceSetAfterAppend,
   IsaT(600)
   DEF HistoricalTemporalStage6PreAdmissionAuxProgress,
       HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual,
       HistoricalTemporalStage6PreAdmissionCandidateProducerContinuationReentry,
       HistoricalTemporalStage6PreAdmissionCandidateProducerContinuationReentry,
       HistoricalTemporalStage6PreAdmissionBlockedAtAux,
       HistoricalTemporalStage6PreAdmissionGoal,
       HistoricalTemporalStage6OwedCausalReady,
       HistoricalTemporalStage6NonCompletionCapacityBlocked,
       HistoricalTemporalStage6CompletionCapacityBlocked,
       HistoricalTemporalStage6Pending,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, CausalCandidatePosition,
       LocalSourceDistance, PreferredLocalSource,
       ReadyRunAuxRank, ReadyRunDeferredRank,
       ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       CausalAdmissionDebtActive,
       NonCompletionCausalAdmissionDebt,
       CompletionCausalAdmissionDebt,
       CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueClass, CanEnqueueIoClass,
       AsyncQueueDepth, AsyncIoQueueDepth,
       AsyncOutstandingWorkCount,
       PostGstRunHistoricalRecoveryNode,
       RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncSchedulerExceptCausalControlAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       AsyncServeIngressLifecycleOwnerIdentities,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       SelectedLocalSource, LocalSourceCanAdmit,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       IngressDrainStep, DrainFairIngressSelected,
       IngressItemCanDrain, SerializedRunnerRuntimeStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       CandidateInReadyQueue, QueuedCandidates,
       DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalStage6PreAdmissionOtherStep ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      /\ HistoricalTemporalStage6PreAdmissionBlockedAtAux(
           candidate, position, rank)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~PostGstRunHistoricalRecoveryNode(candidate.node)
      => \/ HistoricalTemporalStage6PreAdmissionBlockedAtAux(
              candidate, position, rank)'
         \/ HistoricalTemporalStage6PreAdmissionAuxProgress(
              candidate, position, rank)'
         \/ HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
              candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HistoricalTemporalStage6PreAdmissionSameRunnerOutcome,
   ReadyRunAuxRankInCarrier,
   HeadTailProperties, SequenceSetAfterAppend,
   IsaT(600)
   DEF HistoricalTemporalStage6PreAdmissionAuxProgress,
       HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual,
       HistoricalTemporalStage6PreAdmissionBlockedAtAux,
       HistoricalTemporalStage6PreAdmissionGoal,
       HistoricalTemporalStage6OwedCausalReady,
       HistoricalTemporalStage6NonCompletionCapacityBlocked,
       HistoricalTemporalStage6CompletionCapacityBlocked,
       HistoricalTemporalStage6Pending,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, CausalCandidatePosition,
       LocalSourceDistance, PreferredLocalSource,
       ReadyRunAuxRank, ReadyRunDeferredRank,
       ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       CausalAdmissionDebtActive,
       NonCompletionCausalAdmissionDebt,
       CompletionCausalAdmissionDebt,
       CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueClass, CanEnqueueIoClass,
       AsyncQueueDepth, AsyncIoQueueDepth,
       AsyncOutstandingWorkCount,
       PostGstRunHistoricalRecoveryNode,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       RunHistoricalServer, OpenHistoricalRecovery,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       SelectedLocalSource, LocalSourceCanAdmit,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       IngressDrainStep, DrainFairIngressSelected,
       IngressItemCanDrain, PopSelectedIngress,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstCrash,
       AsyncTick, AsyncNonClockVars, AsyncSetGST,
       EnqueueCandidate, AppendCausalSuccessors,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       SequenceWithoutIndex, DeferCommand, DiscardCommand,
       CandidateInReadyQueue, QueuedCandidates,
       DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalStage6RunnerIsNonstuttering ==
  \A candidate, position:
    /\ HistoricalTemporalStage6Pending(candidate, position)
    /\ PostGstRunHistoricalRecoveryNode(candidate.node)
    => <<PostGstRunHistoricalRecoveryNode(
           candidate.node)>>_AsyncAllVars
BY HistoricalTemporalStage6CarrierFacts,
   AsyncStrongTypeProjectsAsyncType,
   HistoricalTemporalRunNodeIsNonstuttering

THEOREM HistoricalTemporalStage6EnablesFairRunner ==
  \A candidate, position:
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ HistoricalTemporalStage6Pending(candidate, position)
      => ENABLED
           <<PostGstRunHistoricalRecoveryNode(
               candidate.node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                AsyncCandidateProducerContinuationExternalCoverageInvariant,
                AsyncCandidateProducerContinuationLocalReplayCapacityInvariant,
                HistoricalTemporalStage6Pending(candidate, position)
         PROVE ENABLED
                 <<PostGstRunHistoricalRecoveryNode(
                     candidate.node)>>_AsyncAllVars
    <2>1. /\ candidate.node \in asyncHistoricalRecoveryTargets
           /\ AsyncStrongTypeInvariant
           /\ gst
      BY <1>1, HistoricalTemporalStage6CarrierFacts
         DEF HistoricalTemporalStage6Pending,
             HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedCandidateOwned,
             HistoricalRecoveryTarget
    <2>2. ENABLED
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>1, HistoricalRecoveryRunnerEnabledAfterGst
    <2>3. PostGstRunHistoricalRecoveryNode(candidate.node)
             => <<PostGstRunHistoricalRecoveryNode(
                    candidate.node)>>_AsyncAllVars
      BY <1>1, HistoricalTemporalStage6RunnerIsNonstuttering
    <2> QED BY <2>2, <2>3, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage6PreAdmissionOneStep ==
  \A initialContext, candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      HistoricalTemporalStage6PreAdmissionFiniteRunnerEpisodeResidualProperty(
        AsyncSpecAt(initialContext))
        => (AsyncSpecAt(initialContext)
              => (HistoricalTemporalStage6PreAdmissionBlockedAtAux(
                    candidate, position, rank)
                    ~> HistoricalTemporalStage6PreAdmissionAuxProgress(
                         candidate, position, rank)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW rank \in ReadyRunAuxCarrier,
                HistoricalTemporalStage6PreAdmissionFiniteRunnerEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage6PreAdmissionBlockedAtAux(
                       candidate, position, rank)
                       ~> HistoricalTemporalStage6PreAdmissionAuxProgress(
                            candidate, position, rank))
    <2>1. /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
           /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
             /\ HistoricalTemporalStage6PreAdmissionBlockedAtAux(
               candidate, position, rank)
             /\ ~(HistoricalTemporalStage6PreAdmissionAuxProgress(
                    candidate, position, rank)
                    \/ HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
                         candidate, position, rank))
            => ENABLED
                 <<PostGstRunHistoricalRecoveryNode(
                     candidate.node)>>_AsyncAllVars
      BY HistoricalTemporalStage6EnablesFairRunner
         DEF HistoricalTemporalStage6PreAdmissionBlockedAtAux
    <2>2. /\ HistoricalTemporalStage6PreAdmissionBlockedAtAux(
               candidate, position, rank)
             /\ ~(HistoricalTemporalStage6PreAdmissionAuxProgress(
                    candidate, position, rank)
                    \/ HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
                         candidate, position, rank))
             /\ <<PostGstRunHistoricalRecoveryNode(
                    candidate.node)>>_AsyncAllVars
            => \/ HistoricalTemporalStage6PreAdmissionAuxProgress(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
                    candidate, position, rank)'
      BY HistoricalTemporalStage6PreAdmissionSameRunnerOutcome
         DEF HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual,
             HistoricalTemporalStage6PreAdmissionCandidateProducerContinuationReentry
    <2>3. HistoricalTemporalStage6PreAdmissionBlockedAtAux(
             candidate, position, rank)
             /\ [AsyncNext]_AsyncAllVars
            => \/ HistoricalTemporalStage6PreAdmissionBlockedAtAux(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage6PreAdmissionAuxProgress(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
                    candidate, position, rank)'
      BY HistoricalTemporalStage6PreAdmissionSameRunnerOutcome,
         HistoricalTemporalStage6PreAdmissionOtherStep, Isa
         DEF HistoricalTemporalStage6PreAdmissionCandidateProducerContinuationReentry
    <2>4. CASE candidate.node \in Responsive
      <3>1. AsyncSpecAt(initialContext)
               => /\ []AsyncCandidateProducerContinuationExternalCoverageInvariant
                  /\ []AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
                  /\ WF_AsyncAllVars(
                       PostGstRunHistoricalRecoveryNode(candidate.node))
        BY <2>4,
           AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
           AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity
           DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage6PreAdmissionBlockedAtAux(
                     candidate, position, rank)
                     ~> (HistoricalTemporalStage6PreAdmissionAuxProgress(
                           candidate, position, rank)
                          \/ HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
                               candidate, position, rank)))
        BY <2>1, <2>2, <2>3, <3>1, PTL DEF AsyncSpecAt
      <3>3. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
                     candidate, position, rank)
                     ~> HistoricalTemporalStage6PreAdmissionAuxProgress(
                          candidate, position, rank))
        BY <1>1
           DEF HistoricalTemporalStage6PreAdmissionFiniteRunnerEpisodeResidualProperty
      <3>4. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage6PreAdmissionBlockedAtAux(
                     candidate, position, rank)
                     ~> HistoricalTemporalStage6PreAdmissionAuxProgress(
                          candidate, position, rank))
        BY <3>2, <3>3, PTL
      <3> QED BY <3>4
    <2>5. CASE candidate.node \notin Responsive
      <3>1. AsyncSpecAt(initialContext)
               => []~HistoricalTemporalStage6PreAdmissionBlockedAtAux(
                      candidate, position, rank)
        BY <2>5, PTL
           DEF HistoricalTemporalStage6PreAdmissionBlockedAtAux,
               HistoricalTemporalStage6Pending,
               HistoricalProtectedOwnedAtServiceRank,
               HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage6PreAdmissionProgress ==
  \A initialContext, candidate, position:
    HistoricalTemporalStage6PreAdmissionFiniteRunnerEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => (HistoricalTemporalStage6Pending(candidate, position)
                  ~> HistoricalTemporalStage6PreAdmissionGoal(
                       candidate, position)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                HistoricalTemporalStage6PreAdmissionFiniteRunnerEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage6Pending(
                       candidate, position)
                       ~> HistoricalTemporalStage6PreAdmissionGoal(
                            candidate, position))
    <2>1. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2>2. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage6Pending(candidate, position)
                   ~> (HistoricalTemporalStage6PreAdmissionGoal(
                         candidate, position)
                        \/ HistoricalTemporalStage6PreAdmissionBlockedAtAux(
                             candidate, position,
                             ReadyRunAuxRank(candidate.node))))
      BY <2>1, ReadyRunAuxRankInCarrier, PTL
         DEF HistoricalTemporalStage6PreAdmissionBlockedAtAux
    <2>3. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  HistoricalTemporalStage6PreAdmissionBlockedAtAux(
                    candidate, position, rank)
                    ~> (HistoricalTemporalStage6PreAdmissionGoal(
                          candidate, position)
                         \/ \E lower \in SetLessThan(
                              rank, ReadyRunAuxOrdering,
                              ReadyRunAuxCarrier):
                              HistoricalTemporalStage6PreAdmissionBlockedAtAux(
                                candidate, position, lower))
      BY HistoricalTemporalFairStage6PreAdmissionOneStep
         DEF HistoricalTemporalStage6PreAdmissionAuxProgress
    <2>4. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  HistoricalTemporalStage6PreAdmissionBlockedAtAux(
                    candidate, position, rank)
                    ~> HistoricalTemporalStage6PreAdmissionGoal(
                         candidate, position)
      BY <2>3, ReadyRunAuxOrderingIsWellFounded,
         WellFoundedLeadsTo
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

HistoricalTemporalStage6OwedBlockedAtAux(
    candidate, position, rank) ==
  /\ HistoricalTemporalStage6OwedCausalReady(candidate, position)
  /\ ~HistoricalTemporalRankProgressExit(candidate, <<6, position>>)
  /\ ReadyRunAuxRank(candidate.node) = rank

HistoricalTemporalStage6OwedAuxProgress(
    candidate, position, rank) ==
  \/ HistoricalTemporalRankProgressExit(candidate, <<6, position>>)
  \/ \E lower \in SetLessThan(
       rank, ReadyRunAuxOrdering, ReadyRunAuxCarrier):
       HistoricalTemporalStage6OwedBlockedAtAux(
         candidate, position, lower)

HistoricalTemporalStage6OwedRunnerEpisodeResidual(
    candidate, position, rank) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalTemporalStage6OwedCausalReady(candidate, position)
  /\ ~HistoricalTemporalStage6OwedAuxProgress(
       candidate, position, rank)
  /\ \/ asyncRunnerPhase[candidate.node] \in {"Local", "Ingress"}
     \/ AsyncCandidateProducerContinuationRunnerResolutionRequired(
          candidate.node)

HistoricalTemporalStage6OwedCandidateProducerContinuationReentry(
    candidate, position, rank) ==
  /\ HistoricalTemporalStage6OwedBlockedAtAux(candidate, position, rank)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
       candidate.node)

HistoricalTemporalStage6OwedFiniteRunnerEpisodeResidualProperty(
    specification) ==
  specification
    => \A candidate, position:
         \A rank \in ReadyRunAuxCarrier:
           HistoricalTemporalStage6OwedRunnerEpisodeResidual(
             candidate, position, rank)
             ~> HistoricalTemporalStage6OwedAuxProgress(
                  candidate, position, rank)

THEOREM HistoricalTemporalStage6OwedSameRunnerOutcome ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      /\ HistoricalTemporalStage6OwedBlockedAtAux(
           candidate, position, rank)
      /\ PostGstRunHistoricalRecoveryNode(candidate.node)
      => \/ HistoricalTemporalStage6OwedAuxProgress(
              candidate, position, rank)'
         \/ HistoricalTemporalStage6OwedRunnerEpisodeResidual(
              candidate, position, rank)'
         \/ /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(
                  candidate.node)
            /\ HistoricalTemporalStage6OwedCandidateProducerContinuationReentry(
                 candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
   IngressDrainStrictlyDecreasesRuntimeReach,
   HistoricalTemporalStage6CausalAdmissionStrictlyProgresses,
   RunNodeWorkConcreteActionCaseSplit,
   ReadyRunAuxRankInCarrier,
   HeadTailProperties, SequenceSetAfterAppend,
   IsaT(600)
   DEF HistoricalTemporalStage6OwedAuxProgress,
       HistoricalTemporalStage6OwedRunnerEpisodeResidual,
       HistoricalTemporalStage6OwedCandidateProducerContinuationReentry,
       HistoricalTemporalStage6OwedCandidateProducerContinuationReentry,
       HistoricalTemporalStage6OwedBlockedAtAux,
       HistoricalTemporalStage6OwedCausalReady,
       HistoricalTemporalStage6Pending,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, CausalCandidatePosition,
       LocalSourceDistance, PreferredLocalSource,
       ReadyRunAuxRank, ReadyRunDeferredRank,
       ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       CausalAdmissionDebtActive, CandidateInFlight,
       CausalHeadCanAdvance, CanEnqueueClass, CanEnqueueIoClass,
       AsyncQueueDepth, AsyncIoQueueDepth,
       AsyncOutstandingWorkCount,
       PostGstRunHistoricalRecoveryNode,
       RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncSchedulerExceptCausalControlAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       AsyncServeIngressLifecycleOwnerIdentities,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       SelectedLocalSource, LocalSourceCanAdmit,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       IngressDrainStep, DrainFairIngressSelected,
       IngressItemCanDrain, SerializedRunnerRuntimeStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       CandidateInReadyQueue, QueuedCandidates,
       DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalStage6OwedOtherStep ==
  \A candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      /\ HistoricalTemporalStage6OwedBlockedAtAux(
           candidate, position, rank)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~PostGstRunHistoricalRecoveryNode(candidate.node)
      => \/ HistoricalTemporalStage6OwedBlockedAtAux(
              candidate, position, rank)'
         \/ HistoricalTemporalStage6OwedAuxProgress(
              candidate, position, rank)'
         \/ HistoricalTemporalStage6OwedRunnerEpisodeResidual(
              candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HistoricalTemporalStage6OwedSameRunnerOutcome,
   ReadyRunAuxRankInCarrier,
   HeadTailProperties, SequenceSetAfterAppend,
   IsaT(600)
   DEF HistoricalTemporalStage6OwedAuxProgress,
       HistoricalTemporalStage6OwedRunnerEpisodeResidual,
       HistoricalTemporalStage6OwedBlockedAtAux,
       HistoricalTemporalStage6OwedCausalReady,
       HistoricalTemporalStage6Pending,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, CausalCandidatePosition,
       LocalSourceDistance, PreferredLocalSource,
       ReadyRunAuxRank, ReadyRunDeferredRank,
       ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       CausalAdmissionDebtActive, CandidateInFlight,
       CausalHeadCanAdvance, CanEnqueueClass, CanEnqueueIoClass,
       AsyncQueueDepth, AsyncIoQueueDepth,
       AsyncOutstandingWorkCount,
       PostGstRunHistoricalRecoveryNode,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       RunHistoricalServer, OpenHistoricalRecovery,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       SelectedLocalSource, LocalSourceCanAdmit,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, RecordBlockedCausalDebt,
       IngressDrainStep, DrainFairIngressSelected,
       IngressItemCanDrain, PopSelectedIngress,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstCrash,
       AsyncTick, AsyncNonClockVars, AsyncSetGST,
       EnqueueCandidate, AppendCausalSuccessors,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       SequenceWithoutIndex, DeferCommand, DiscardCommand,
       CandidateInReadyQueue, QueuedCandidates,
       DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalFairStage6OwedOneStep ==
  \A initialContext, candidate, position:
    \A rank \in ReadyRunAuxCarrier:
      HistoricalTemporalStage6OwedFiniteRunnerEpisodeResidualProperty(
        AsyncSpecAt(initialContext))
        => (AsyncSpecAt(initialContext)
              => (HistoricalTemporalStage6OwedBlockedAtAux(
                    candidate, position, rank)
                    ~> HistoricalTemporalStage6OwedAuxProgress(
                         candidate, position, rank)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW rank \in ReadyRunAuxCarrier,
                HistoricalTemporalStage6OwedFiniteRunnerEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage6OwedBlockedAtAux(
                       candidate, position, rank)
                       ~> HistoricalTemporalStage6OwedAuxProgress(
                            candidate, position, rank))
    <2>1. /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
           /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
             /\ HistoricalTemporalStage6OwedBlockedAtAux(
               candidate, position, rank)
             /\ ~(HistoricalTemporalStage6OwedAuxProgress(
                    candidate, position, rank)
                    \/ HistoricalTemporalStage6OwedRunnerEpisodeResidual(
                         candidate, position, rank))
            => ENABLED
                 <<PostGstRunHistoricalRecoveryNode(
                     candidate.node)>>_AsyncAllVars
      BY HistoricalTemporalStage6EnablesFairRunner
         DEF HistoricalTemporalStage6OwedBlockedAtAux,
             HistoricalTemporalStage6OwedCausalReady
    <2>2. /\ HistoricalTemporalStage6OwedBlockedAtAux(
               candidate, position, rank)
             /\ ~(HistoricalTemporalStage6OwedAuxProgress(
                    candidate, position, rank)
                    \/ HistoricalTemporalStage6OwedRunnerEpisodeResidual(
                         candidate, position, rank))
             /\ <<PostGstRunHistoricalRecoveryNode(
                    candidate.node)>>_AsyncAllVars
            => \/ HistoricalTemporalStage6OwedAuxProgress(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage6OwedRunnerEpisodeResidual(
                    candidate, position, rank)'
      BY HistoricalTemporalStage6OwedSameRunnerOutcome
         DEF HistoricalTemporalStage6OwedRunnerEpisodeResidual,
             HistoricalTemporalStage6OwedCandidateProducerContinuationReentry
    <2>3. HistoricalTemporalStage6OwedBlockedAtAux(
             candidate, position, rank)
             /\ [AsyncNext]_AsyncAllVars
            => \/ HistoricalTemporalStage6OwedBlockedAtAux(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage6OwedAuxProgress(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage6OwedRunnerEpisodeResidual(
                    candidate, position, rank)'
      BY HistoricalTemporalStage6OwedSameRunnerOutcome,
         HistoricalTemporalStage6OwedOtherStep, Isa
         DEF HistoricalTemporalStage6OwedCandidateProducerContinuationReentry
    <2>4. CASE candidate.node \in Responsive
      <3>1. AsyncSpecAt(initialContext)
               => /\ []AsyncCandidateProducerContinuationExternalCoverageInvariant
                  /\ []AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
                  /\ WF_AsyncAllVars(
                       PostGstRunHistoricalRecoveryNode(candidate.node))
        BY <2>4,
           AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
           AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity
           DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage6OwedBlockedAtAux(
                     candidate, position, rank)
                     ~> (HistoricalTemporalStage6OwedAuxProgress(
                           candidate, position, rank)
                          \/ HistoricalTemporalStage6OwedRunnerEpisodeResidual(
                               candidate, position, rank)))
        BY <2>1, <2>2, <2>3, <3>1, PTL DEF AsyncSpecAt
      <3>3. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage6OwedRunnerEpisodeResidual(
                     candidate, position, rank)
                     ~> HistoricalTemporalStage6OwedAuxProgress(
                          candidate, position, rank))
        BY <1>1
           DEF HistoricalTemporalStage6OwedFiniteRunnerEpisodeResidualProperty
      <3>4. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage6OwedBlockedAtAux(
                     candidate, position, rank)
                     ~> HistoricalTemporalStage6OwedAuxProgress(
                          candidate, position, rank))
        BY <3>2, <3>3, PTL
      <3> QED BY <3>4
    <2>5. CASE candidate.node \notin Responsive
      <3>1. AsyncSpecAt(initialContext)
               => []~HistoricalTemporalStage6OwedBlockedAtAux(
                      candidate, position, rank)
        BY <2>5, PTL
           DEF HistoricalTemporalStage6OwedBlockedAtAux,
               HistoricalTemporalStage6OwedCausalReady,
               HistoricalTemporalStage6Pending,
               HistoricalProtectedOwnedAtServiceRank,
               HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage6OwedCausalAdmission ==
  \A initialContext, candidate, position:
    HistoricalTemporalStage6OwedFiniteRunnerEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => (HistoricalTemporalStage6OwedCausalReady(
                  candidate, position)
                  ~> HistoricalTemporalRankProgressExit(
                       candidate, <<6, position>>)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                HistoricalTemporalStage6OwedFiniteRunnerEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage6OwedCausalReady(
                       candidate, position)
                       ~> HistoricalTemporalRankProgressExit(
                            candidate, <<6, position>>))
    <2>1. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2>2. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage6OwedCausalReady(
                   candidate, position)
                   ~> HistoricalTemporalStage6OwedBlockedAtAux(
                        candidate, position,
                        ReadyRunAuxRank(candidate.node)))
      BY <2>1, ReadyRunAuxRankInCarrier, PTL
         DEF HistoricalTemporalStage6OwedBlockedAtAux
    <2>3. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  HistoricalTemporalStage6OwedBlockedAtAux(
                    candidate, position, rank)
                    ~> (HistoricalTemporalRankProgressExit(
                          candidate, <<6, position>>)
                         \/ \E lower \in SetLessThan(
                              rank, ReadyRunAuxOrdering,
                              ReadyRunAuxCarrier):
                              HistoricalTemporalStage6OwedBlockedAtAux(
                                candidate, position, lower))
      BY HistoricalTemporalFairStage6OwedOneStep
         DEF HistoricalTemporalStage6OwedAuxProgress
    <2>4. AsyncSpecAt(initialContext)
             => \A rank \in ReadyRunAuxCarrier:
                  HistoricalTemporalStage6OwedBlockedAtAux(
                    candidate, position, rank)
                    ~> HistoricalTemporalRankProgressExit(
                         candidate, <<6, position>>)
      BY <2>3, ReadyRunAuxOrderingIsWellFounded,
         WellFoundedLeadsTo
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

HistoricalTemporalStage6NonCompletionBlockedAtRank(
    candidate, position, rank) ==
  /\ HistoricalTemporalStage6NonCompletionCapacityBlocked(
       candidate, position)
  /\ Stage4CapacityRank(candidate.node) = rank

HistoricalTemporalStage6NonCompletionGoal(candidate, position) ==
  \/ HistoricalTemporalRankProgressExit(candidate, <<6, position>>)
  \/ HistoricalTemporalStage6OwedCausalReady(candidate, position)

HistoricalTemporalStage6NonCompletionProgress(
    candidate, position, rank) ==
  \/ HistoricalTemporalStage6NonCompletionGoal(candidate, position)
  \/ \E lower \in SetLessThan(
       rank, Stage4CapacityOrdering, Stage4CapacityCarrier):
       HistoricalTemporalStage6NonCompletionBlockedAtRank(
         candidate, position, lower)

HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
    candidate, position, rank) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalTemporalStage6Pending(candidate, position)
  /\ ~HistoricalTemporalStage6NonCompletionProgress(
       candidate, position, rank)
  /\ \/ /\ AsyncServeIngressLifecycleOwnerIdentities(candidate.node) # {}
        /\ asyncRunnerPhase[candidate.node] = "Ingress"
     \/ AsyncCandidateProducerContinuationRunnerResolutionRequired(
          candidate.node)

HistoricalTemporalStage6NonCompletionCandidateProducerContinuationReentry(
    candidate, position, rank) ==
  /\ HistoricalTemporalStage6NonCompletionBlockedAtRank(
       candidate, position, rank)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
       candidate.node)

HistoricalTemporalStage6NonCompletionFiniteServeEpisodeResidualProperty(
    specification) ==
  specification
    => \A candidate, position:
         \A rank \in Stage4CapacityCarrier:
           HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
             candidate, position, rank)
             ~> HistoricalTemporalStage6NonCompletionProgress(
                  candidate, position, rank)

THEOREM HistoricalTemporalStage6NonCompletionSameRunnerOutcome ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
      /\ HistoricalTemporalStage6NonCompletionBlockedAtRank(
           candidate, position, rank)
      /\ PostGstRunHistoricalRecoveryNode(candidate.node)
      => \/ HistoricalTemporalStage6NonCompletionProgress(
              candidate, position, rank)'
         \/ HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
              candidate, position, rank)'
         \/ /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(
                  candidate.node)
            /\ HistoricalTemporalStage6NonCompletionCandidateProducerContinuationReentry(
                 candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
   IngressDrainStrictlyDecreasesRuntimeReach,
   NonCompletionDebtFifoCapacityProgress,
   SerializedFifoRetainsNonCompletionCausalDebt,
   SerializedFifoRetainsExistingCausalHead,
   Stage4CapacityRankInCarrier,
   ReadyRunAuxRankInCarrier,
   RunNodeWorkConcreteActionCaseSplit,
   HeadTailProperties, SequenceSetAfterAppend,
   IsaT(600)
   DEF HistoricalTemporalStage6NonCompletionProgress,
       HistoricalTemporalStage6NonCompletionServeEpisodeResidual,
       HistoricalTemporalStage6NonCompletionCandidateProducerContinuationReentry,
       HistoricalTemporalStage6NonCompletionCandidateProducerContinuationReentry,
       HistoricalTemporalStage6NonCompletionGoal,
       HistoricalTemporalStage6NonCompletionBlockedAtRank,
       HistoricalTemporalStage6NonCompletionCapacityBlocked,
       HistoricalTemporalStage6OwedCausalReady,
       HistoricalTemporalStage6Pending,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, CausalCandidatePosition,
       LocalSourceDistance, PreferredLocalSource,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       NonCompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive,
       ReadyRunAuxRank, ReadyRunDeferredRank,
       ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueClass, AsyncQueueDepth, NodeQueueNonempty,
       PostGstRunHistoricalRecoveryNode,
       RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncSchedulerExceptCausalControlAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       AsyncServeIngressLifecycleOwnerIdentities,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       ProducerCompletionCanAdvance, ProducerCompletionCanAdmit,
       AdmitProducerCompletion, AdmitCausalHead,
       RecordBlockedCausalDebt,
       IngressDrainStep, DrainFairIngressSelected,
       IngressItemCanDrain, PopSelectedIngress,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       RemoveNextDeferredCommand, DiscardCommand,
       AdvanceNextDeferredClass, DeferredQueueNonempty,
       DeferredHandoffActive, DeferredHandoffMatches,
       DeferredHandoffAllowsExecution,
       DeferredHandoffBlocksExecution,
       InstallDeferredHandoff, RetainDeferredHandoffs,
       ClearDeferredHandoff,
       AppendCausalSuccessors, FreshCommandSuccessors,
       CandidateScheduled, CausalCandidates, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalStage6NonCompletionOtherStep ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
      /\ HistoricalTemporalStage6NonCompletionBlockedAtRank(
           candidate, position, rank)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~PostGstRunHistoricalRecoveryNode(candidate.node)
      => \/ HistoricalTemporalStage6NonCompletionBlockedAtRank(
              candidate, position, rank)'
         \/ HistoricalTemporalStage6NonCompletionProgress(
              candidate, position, rank)'
         \/ HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
              candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HistoricalTemporalStage6NonCompletionSameRunnerOutcome,
   Stage4CapacityRankInCarrier,
   ReadyRunAuxRankInCarrier,
   HeadTailProperties, SequenceSetAfterAppend,
   IsaT(600)
   DEF HistoricalTemporalStage6NonCompletionProgress,
       HistoricalTemporalStage6NonCompletionServeEpisodeResidual,
       HistoricalTemporalStage6NonCompletionGoal,
       HistoricalTemporalStage6NonCompletionBlockedAtRank,
       HistoricalTemporalStage6NonCompletionCapacityBlocked,
       HistoricalTemporalStage6OwedCausalReady,
       HistoricalTemporalStage6Pending,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, CausalCandidatePosition,
       LocalSourceDistance, PreferredLocalSource,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       NonCompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive,
       ReadyRunAuxRank, ReadyRunDeferredRank,
       ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueClass, AsyncQueueDepth, NodeQueueNonempty,
       PostGstRunHistoricalRecoveryNode,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       RunHistoricalServer, OpenHistoricalRecovery,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       ProducerCompletionCanAdvance, ProducerCompletionCanAdmit,
       AdmitProducerCompletion, AdmitCausalHead,
       RecordBlockedCausalDebt,
       IngressDrainStep, DrainFairIngressSelected,
       IngressItemCanDrain, PopSelectedIngress,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstCrash,
       AsyncTick, AsyncNonClockVars, AsyncSetGST,
       EnqueueCandidate, AppendCausalSuccessors,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       SequenceWithoutIndex, DeferCommand, DiscardCommand,
       CandidateScheduled, CandidateInReadyQueue,
       QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       ConsensusIoCandidates, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalFairStage6NonCompletionOneStep ==
  \A initialContext, candidate, position:
    \A rank \in Stage4CapacityCarrier:
      HistoricalTemporalStage6NonCompletionFiniteServeEpisodeResidualProperty(
        AsyncSpecAt(initialContext))
        => (AsyncSpecAt(initialContext)
              => (HistoricalTemporalStage6NonCompletionBlockedAtRank(
                    candidate, position, rank)
                    ~> HistoricalTemporalStage6NonCompletionProgress(
                         candidate, position, rank)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW rank \in Stage4CapacityCarrier,
                HistoricalTemporalStage6NonCompletionFiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage6NonCompletionBlockedAtRank(
                       candidate, position, rank)
                       ~> HistoricalTemporalStage6NonCompletionProgress(
                            candidate, position, rank))
    <2>1. /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
           /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
             /\ HistoricalTemporalStage6NonCompletionBlockedAtRank(
               candidate, position, rank)
             /\ ~(HistoricalTemporalStage6NonCompletionProgress(
                    candidate, position, rank)
                    \/ HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
                         candidate, position, rank))
            => ENABLED
                 <<PostGstRunHistoricalRecoveryNode(
                     candidate.node)>>_AsyncAllVars
      BY HistoricalTemporalStage6EnablesFairRunner
         DEF HistoricalTemporalStage6NonCompletionBlockedAtRank,
             HistoricalTemporalStage6NonCompletionCapacityBlocked
    <2>2. /\ HistoricalTemporalStage6NonCompletionBlockedAtRank(
               candidate, position, rank)
             /\ ~(HistoricalTemporalStage6NonCompletionProgress(
                    candidate, position, rank)
                    \/ HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
                         candidate, position, rank))
             /\ <<PostGstRunHistoricalRecoveryNode(
                    candidate.node)>>_AsyncAllVars
            => \/ HistoricalTemporalStage6NonCompletionProgress(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
                    candidate, position, rank)'
      BY HistoricalTemporalStage6NonCompletionSameRunnerOutcome
         DEF HistoricalTemporalStage6NonCompletionServeEpisodeResidual,
             HistoricalTemporalStage6NonCompletionCandidateProducerContinuationReentry
    <2>3. HistoricalTemporalStage6NonCompletionBlockedAtRank(
             candidate, position, rank)
             /\ [AsyncNext]_AsyncAllVars
            => \/ HistoricalTemporalStage6NonCompletionBlockedAtRank(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage6NonCompletionProgress(
                    candidate, position, rank)'
               \/ HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
                    candidate, position, rank)'
      BY HistoricalTemporalStage6NonCompletionSameRunnerOutcome,
         HistoricalTemporalStage6NonCompletionOtherStep, Isa
         DEF HistoricalTemporalStage6NonCompletionCandidateProducerContinuationReentry
    <2>4. CASE candidate.node \in Responsive
      <3>1. AsyncSpecAt(initialContext)
               => /\ []AsyncCandidateProducerContinuationExternalCoverageInvariant
                  /\ []AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
                  /\ WF_AsyncAllVars(
                       PostGstRunHistoricalRecoveryNode(candidate.node))
        BY <2>4,
           AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
           AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity
           DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage6NonCompletionBlockedAtRank(
                     candidate, position, rank)
                     ~> (HistoricalTemporalStage6NonCompletionProgress(
                           candidate, position, rank)
                          \/ HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
                               candidate, position, rank)))
        BY <2>1, <2>2, <2>3, <3>1, PTL DEF AsyncSpecAt
      <3>3. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
                     candidate, position, rank)
                     ~> HistoricalTemporalStage6NonCompletionProgress(
                          candidate, position, rank))
        BY <1>1
           DEF HistoricalTemporalStage6NonCompletionFiniteServeEpisodeResidualProperty
      <3>4. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage6NonCompletionBlockedAtRank(
                     candidate, position, rank)
                     ~> HistoricalTemporalStage6NonCompletionProgress(
                          candidate, position, rank))
        BY <3>2, <3>3, PTL
      <3> QED BY <3>4
    <2>5. CASE candidate.node \notin Responsive
      <3>1. AsyncSpecAt(initialContext)
               => []~HistoricalTemporalStage6NonCompletionBlockedAtRank(
                      candidate, position, rank)
        BY <2>5, PTL
           DEF HistoricalTemporalStage6NonCompletionBlockedAtRank,
               HistoricalTemporalStage6NonCompletionCapacityBlocked,
               HistoricalTemporalStage6Pending,
               HistoricalProtectedOwnedAtServiceRank,
               HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage6NonCompletionOpens ==
  \A initialContext, candidate, position:
    HistoricalTemporalStage6NonCompletionFiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => (HistoricalTemporalStage6NonCompletionCapacityBlocked(
                  candidate, position)
                  ~> HistoricalTemporalStage6NonCompletionGoal(
                       candidate, position)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                HistoricalTemporalStage6NonCompletionFiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage6NonCompletionCapacityBlocked(
                       candidate, position)
                       ~> HistoricalTemporalStage6NonCompletionGoal(
                            candidate, position))
    <2>1. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2>2. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage6NonCompletionCapacityBlocked(
                   candidate, position)
                   ~> HistoricalTemporalStage6NonCompletionBlockedAtRank(
                        candidate, position,
                        Stage4CapacityRank(candidate.node)))
      BY <2>1, Stage4CapacityRankInCarrier, PTL
         DEF HistoricalTemporalStage6NonCompletionBlockedAtRank
    <2>3. AsyncSpecAt(initialContext)
             => \A rank \in Stage4CapacityCarrier:
                  HistoricalTemporalStage6NonCompletionBlockedAtRank(
                    candidate, position, rank)
                    ~> (HistoricalTemporalStage6NonCompletionGoal(
                          candidate, position)
                         \/ \E lower \in SetLessThan(
                              rank, Stage4CapacityOrdering,
                              Stage4CapacityCarrier):
                              HistoricalTemporalStage6NonCompletionBlockedAtRank(
                                candidate, position, lower))
      BY HistoricalTemporalFairStage6NonCompletionOneStep
         DEF HistoricalTemporalStage6NonCompletionProgress
    <2>4. AsyncSpecAt(initialContext)
             => \A rank \in Stage4CapacityCarrier:
                  HistoricalTemporalStage6NonCompletionBlockedAtRank(
                    candidate, position, rank)
                    ~> HistoricalTemporalStage6NonCompletionGoal(
                         candidate, position)
      BY <2>3, Stage4CapacityOrderingIsWellFounded,
         WellFoundedLeadsTo
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

(***************************************************************************
Stage 5: historical-target Consensus-I/O FIFO service.

An out-of-roster recovery target is not in `AsyncArchiveIoServiceNodes`, so
the ordinary `PostGstServiceIoWorker` proof cannot be reused.  The dedicated
historical worker executes the same `ServiceIoWorkerWork` transition under
the exact retained `HistoricalRecoveryTarget` guard.  The queue occurrence is
therefore removed or shifted strictly left under the worker's own weak
fairness clause.
***************************************************************************)

HistoricalTemporalStage5Pending(candidate, position) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalProtectedOwnedAtServiceRank(
       candidate, <<5, position>>)

THEOREM HistoricalTemporalStage5CarrierFacts ==
  \A candidate, position:
    HistoricalTemporalStage5Pending(candidate, position)
      => /\ candidate.node \in Responsive
         /\ HistoricalRecoveryTarget(candidate.node)
         /\ candidate \in asyncOutstandingWork[candidate.node]
         /\ CandidateInIoQueue(candidate)
         /\ CandidateIoIndex(
              candidate, asyncIoQueues[candidate.node]) = position
         /\ AsyncIoQueueDepth(candidate.node) > 0
         /\ AsyncIoSequenceTyped(asyncIoQueues[candidate.node])
         /\ AsyncIoConsensusQueueOwnership(
              asyncIoQueues[candidate.node],
              asyncIoReadyCompletions[candidate.node],
              asyncLocalReadyCompletions[candidate.node])
BY CandidateIoIndexCharacterization, Isa
   DEF HistoricalTemporalStage5Pending,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CandidateInIoQueue,
       AsyncProgressOwnershipInvariant,
       AsyncOutstandingCarrierInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
       AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant,
       AsyncIoWorkContentTypeInvariant,
       AsyncIoConsensusCandidateOwnership,
       ConsensusIoCandidates, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, CandidateScheduled,
       CandidateInReadyQueue, SequenceSet, AsyncIoQueueDepth

THEOREM HistoricalTemporalQueuedIoServiceIsNonstuttering ==
  \A node:
    /\ HistoricalRecoveryTarget(node)
    /\ AsyncTypeInvariant
    /\ AsyncIoQueueDepth(node) > 0
    /\ PostGstServiceHistoricalRecoveryIoWorker(node)
    => <<PostGstServiceHistoricalRecoveryIoWorker(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW node,
                HistoricalRecoveryTarget(node),
                AsyncTypeInvariant,
                AsyncIoQueueDepth(node) > 0,
                PostGstServiceHistoricalRecoveryIoWorker(node)
         PROVE
           <<PostGstServiceHistoricalRecoveryIoWorker(node)>>_AsyncAllVars
    <2>1. /\ node \in ValidatorIds
           /\ AsyncIoSequenceTyped(asyncIoQueues[node])
      BY <1>1
         DEF HistoricalRecoveryTarget,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncHistoricalRecoveryTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant
    <2>2. /\ asyncIoQueues[node] \in Seq(Range(asyncIoQueues[node]))
           /\ Len(asyncIoQueues[node]) > 0
      BY <1>1, <2>1
         DEF AsyncIoSequenceTyped, AsyncIoQueueDepth
    <2>3. /\ asyncIoQueues[node] # <<>>
           /\ Len(Tail(asyncIoQueues[node]))
                = Len(asyncIoQueues[node]) - 1
      BY <2>2, PositiveSequenceIsNonempty, HeadTailProperties
    <2>4. Tail(asyncIoQueues[node]) # asyncIoQueues[node]
      BY <2>2, <2>3, LenProperties, Isa
    <2>5. asyncIoQueues'[node] = Tail(asyncIoQueues[node])
      BY <1>1, <2>1
         DEF PostGstServiceHistoricalRecoveryIoWorker,
             ServiceHistoricalRecoveryIoWorker,
             ServiceIoWorkerWork
    <2>6. asyncIoQueues' # asyncIoQueues
      BY <2>4, <2>5, Isa
    <2> QED BY <1>1, <2>6, Isa
         DEF AsyncAllVars, AsyncSchedulerVars
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage5EnablesFairWorker ==
  \A candidate, position:
    HistoricalTemporalStage5Pending(candidate, position)
      => ENABLED
           <<PostGstServiceHistoricalRecoveryIoWorker(
               candidate.node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                HistoricalTemporalStage5Pending(candidate, position)
         PROVE ENABLED
                 <<PostGstServiceHistoricalRecoveryIoWorker(
                     candidate.node)>>_AsyncAllVars
    <2>1. /\ candidate.node \in asyncHistoricalRecoveryTargets
           /\ AsyncStrongTypeInvariant
           /\ gst
           /\ AsyncIoQueueDepth(candidate.node) > 0
      BY <1>1, HistoricalTemporalStage5CarrierFacts
         DEF HistoricalTemporalStage5Pending,
             HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedCandidateOwned,
             HistoricalRecoveryTarget
    <2>2. ENABLED
             PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <2>1, HistoricalRecoveryIoWorkerEnabledAfterGst
    <2>3. PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
             => <<PostGstServiceHistoricalRecoveryIoWorker(
                    candidate.node)>>_AsyncAllVars
      BY <1>1, HistoricalTemporalQueuedIoServiceIsNonstuttering
         DEF HistoricalTemporalStage5Pending
    <2>4. ENABLED
             <<PostGstServiceHistoricalRecoveryIoWorker(
                 candidate.node)>>_AsyncAllVars
      BY <2>2, <2>3, ENABLEDaxioms
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage5WorkerStrictlyProgresses ==
  \A candidate, position:
    /\ HistoricalTemporalStage5Pending(candidate, position)
    /\ PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
    => HistoricalTemporalRankProgressExit(
         candidate, <<5, position>>)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                HistoricalTemporalStage5Pending(candidate, position),
                PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
         PROVE HistoricalTemporalRankProgressExit(
                 candidate, <<5, position>>)'
    <2> DEFINE Queue == asyncIoQueues[candidate.node]
    <2>1. /\ AsyncTypeInvariant
           /\ candidate \in asyncOutstandingWork[candidate.node]
           /\ CandidateInIoQueue(candidate)
           /\ CandidateIoIndex(candidate, Queue) = position
           /\ AsyncIoQueueDepth(candidate.node) > 0
           /\ AsyncIoSequenceTyped(Queue)
           /\ AsyncIoConsensusQueueOwnership(
                Queue, asyncIoReadyCompletions[candidate.node],
                asyncLocalReadyCompletions[candidate.node])
      BY <1>1, HistoricalTemporalStage5CarrierFacts,
         AsyncStrongTypeProjectsAsyncType DEF Queue
    <2>2. /\ asyncIoQueues'[candidate.node] = Tail(Queue)
           /\ asyncOutstandingWork'[candidate.node]
                = asyncOutstandingWork[candidate.node]
           /\ asyncCommandQueues' = asyncCommandQueues
           /\ asyncCausalQueues' = asyncCausalQueues
           /\ asyncDeferredCompletionQueues'
                = asyncDeferredCompletionQueues
           /\ asyncDeferredProgressQueues'
                = asyncDeferredProgressQueues
           /\ asyncDeferredNormalQueues' = asyncDeferredNormalQueues
      BY <1>1, Isa
         DEF PostGstServiceHistoricalRecoveryIoWorker,
             ServiceHistoricalRecoveryIoWorker,
             ServiceIoWorkerWork, LeaveCausalQueues,
             AsyncDeferredVars, vars, Queue
    <2>3. CASE /\ Head(Queue).class = "Consensus"
                   /\ Head(Queue).candidate = candidate
      <3>1. Head(Queue).class = "Consensus"
        BY <2>3
      <3>2. candidate \in
               SequenceSet(asyncIoReadyCompletions'[candidate.node])
        BY <1>1, <2>3, <3>1, SequenceSetAfterAppend, Isa
           DEF PostGstServiceHistoricalRecoveryIoWorker,
               ServiceHistoricalRecoveryIoWorker,
               ServiceIoWorkerWork, Queue
      <3>3. CandidateServiceRank(candidate)'[1] = 4
        BY <1>1, <2>1, <2>2, <3>2, Isa
           DEF CandidateServiceRank, CandidateInReadyQueue,
               QueuedCandidates, DeferredCandidates, CausalCandidates,
               TrackedWorkCandidates, SequenceSet
      <3> QED BY <3>3, Isa
           DEF HistoricalTemporalRankProgressExit,
               ServiceRankLess
    <2>4. CASE ~(Head(Queue).class = "Consensus"
                   /\ Head(Queue).candidate = candidate)
      <3>1. CandidateIoIndex(candidate, Tail(Queue)) + 1 = position
        BY <1>1, <2>1, <2>4,
           CandidateIoIndexAfterNonTargetHead
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
               AsyncIoWorkContentTypeInvariant, Queue
      <3>2. /\ CandidateInIoQueue(candidate)'
             /\ CandidateIoIndex(
                  candidate, asyncIoQueues'[candidate.node]) < position
        BY <2>1, <2>2, <3>1, Isa
           DEF CandidateInIoQueue, AsyncIoConsensusIndices, Queue
      <3>3. /\ ~HistoricalProtectedServiceOwnershipExit(candidate)'
                 => CandidateServiceRank(candidate)' =
                      <<5, CandidateIoIndex(
                              candidate,
                              asyncIoQueues'[candidate.node])>>
        BY <1>1, <2>1, <2>2, <3>2, Isa
           DEF HistoricalProtectedServiceOwnershipExit,
               HistoricalProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               CandidateInReadyQueue, QueuedCandidates,
               DeferredCandidates, CausalCandidates,
               TrackedWorkCandidates, CandidateScheduled, SequenceSet
      <3> QED BY <3>2, <3>3, Isa
           DEF HistoricalTemporalRankProgressExit, ServiceRankLess
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage5UnlessProgress ==
  \A candidate, position:
    HistoricalTemporalStage5Pending(candidate, position)
      /\ [AsyncNext]_AsyncAllVars
      => HistoricalTemporalStage5Pending(candidate, position)'
           \/ HistoricalTemporalRankProgressExit(
                candidate, <<5, position>>)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   CandidateIoIndexAfterNonTargetHead,
   AppendProperties, HeadTailProperties, Isa
   DEF HistoricalTemporalStage5Pending,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess, CandidateInIoQueue,
       CandidateInReadyQueue, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, CandidateScheduled,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       RunHistoricalServer, OpenHistoricalRecovery,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       LocalAdmissionStep, AdmitProducerCompletion, AdmitCausalHead,
       IngressDrainStep, DrainFairIngressSelected,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       PreGstCrash, EnqueueCandidate, AppendCausalSuccessors,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       SequenceWithoutIndex, DeferCommand, DiscardCommand,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, ConsensusIoCandidates,
       SequenceHasUniqueValues, SequenceSet, AsyncIoConsensusIndices,
       AsyncAllVars

THEOREM HistoricalTemporalFairStage5RankDescent ==
  \A initialContext, candidate:
    \A position \in Nat:
      AsyncSpecAt(initialContext)
        => (HistoricalProtectedOwnedAtServiceRank(
              candidate, <<5, position>>)
              ~> HistoricalTemporalRankProgressExit(
                   candidate, <<5, position>>))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                NEW position \in Nat
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalProtectedOwnedAtServiceRank(
                       candidate, <<5, position>>)
                       ~> HistoricalTemporalRankProgressExit(
                            candidate, <<5, position>>))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>2. HistoricalTemporalStage5Pending(candidate, position)
               /\ ~HistoricalTemporalRankProgressExit(
                    candidate, <<5, position>>)
              => ENABLED
                   <<PostGstServiceHistoricalRecoveryIoWorker(
                       candidate.node)>>_AsyncAllVars
      BY HistoricalTemporalStage5EnablesFairWorker
    <2>3. /\ HistoricalTemporalStage5Pending(candidate, position)
             /\ ~HistoricalTemporalRankProgressExit(
                  candidate, <<5, position>>)
             /\ <<PostGstServiceHistoricalRecoveryIoWorker(
                    candidate.node)>>_AsyncAllVars
            => HistoricalTemporalRankProgressExit(
                 candidate, <<5, position>>)'
      BY HistoricalTemporalStage5WorkerStrictlyProgresses, Isa
         DEF HistoricalTemporalStage5Pending,
             HistoricalTemporalRankProgressExit
    <2>4. HistoricalTemporalStage5Pending(candidate, position)
               /\ [AsyncNext]_AsyncAllVars
              => HistoricalTemporalStage5Pending(candidate, position)'
                   \/ HistoricalTemporalRankProgressExit(
                        candidate, <<5, position>>)'
      BY HistoricalTemporalStage5UnlessProgress
    <2>5. CASE candidate.node \in Responsive
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(
                    PostGstServiceHistoricalRecoveryIoWorker(
                      candidate.node))
        BY <2>5 DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage5Pending(candidate, position)
                     ~> HistoricalTemporalRankProgressExit(
                          candidate, <<5, position>>))
        BY <2>2, <2>3, <2>4, <3>1, PTL DEF AsyncSpecAt
      <3> QED BY <3>2
    <2>6. CASE candidate.node \notin Responsive
      <3>1. AsyncSpecAt(initialContext)
               => []~HistoricalProtectedOwnedAtServiceRank(
                      candidate, <<5, position>>)
        BY <2>6, PTL
           DEF HistoricalProtectedOwnedAtServiceRank,
               HistoricalProtectedCandidateOwned
      <3>2. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage5Pending(candidate, position)
                     ~> HistoricalTemporalRankProgressExit(
                          candidate, <<5, position>>))
        BY <3>1, PTL DEF HistoricalTemporalStage5Pending
      <3> QED BY <3>2
    <2>7. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage5Pending(candidate, position)
                   ~> HistoricalTemporalRankProgressExit(
                        candidate, <<5, position>>))
      BY <2>5, <2>6
    <2>8. AsyncSpecAt(initialContext)
             => (HistoricalProtectedOwnedAtServiceRank(
                   candidate, <<5, position>>)
                   ~> HistoricalTemporalStage5Pending(
                        candidate, position))
      BY <2>1, PTL DEF HistoricalTemporalStage5Pending
    <2> QED BY <2>7, <2>8, PTL
  <1> QED BY <1>1

(***************************************************************************
The stage split is exact.  It is intentionally kept separate from the
temporal service theorem so a later proof cannot hide an omitted stage in an
aggregate starvation claim.
***************************************************************************)

HistoricalTemporalStage2Leaf(specification) ==
  HistoricalProtectedStage2RankProgressProperty(specification)

HistoricalTemporalStage3Leaf(specification) ==
  HistoricalProtectedStage3RankProgressProperty(specification)

HistoricalTemporalStage4Leaf(specification) ==
  HistoricalProtectedStage4RankProgressProperty(specification)

HistoricalTemporalStage5Leaf(specification) ==
  HistoricalProtectedStage5RankProgressProperty(specification)

HistoricalTemporalStage6Leaf(specification) ==
  HistoricalProtectedStage6RankProgressProperty(specification)

THEOREM AsyncSpecClosesHistoricalTemporalStage5Leaf ==
  \A initialContext:
    HistoricalTemporalStage5Leaf(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW candidate \in AsyncCandidateSet,
                NEW position \in Nat
         PROVE AsyncSpecAt(initialContext)
                 => ((gst
                       /\ HistoricalProtectedCandidateOwned(candidate)
                       /\ CandidateServiceRank(candidate)
                            = <<5, position>>)
                      ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                           \/ \E lower \in SetLessThan(
                                <<5, position>>,
                                OwnedServiceRankOrdering,
                                OwnedServiceRankCarrier):
                                HistoricalProtectedOwnedAtServiceRank(
                                  candidate, lower)))
    <2>1. AsyncSpecAt(initialContext)
             => []AsyncTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2>2. AsyncSpecAt(initialContext)
             => (HistoricalProtectedOwnedAtServiceRank(
                   candidate, <<5, position>>)
                   ~> HistoricalTemporalRankProgressExit(
                        candidate, <<5, position>>))
      BY HistoricalTemporalFairStage5RankDescent
    <2>3. AsyncSpecAt(initialContext)
             => (HistoricalTemporalRankProgressExit(
                   candidate, <<5, position>>)
                   ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                        \/ \E lower \in SetLessThan(
                             <<5, position>>,
                             OwnedServiceRankOrdering,
                             OwnedServiceRankCarrier):
                             HistoricalProtectedOwnedAtServiceRank(
                               candidate, lower)))
      BY <2>1,
         HistoricalTemporalRankExitHasWellFoundedSuccessor, PTL
         DEF HistoricalTemporalRankProgressExit
    <2> QED BY <2>2, <2>3, PTL
         DEF HistoricalProtectedOwnedAtServiceRank
  <1> QED BY <1>1
       DEF HistoricalTemporalStage5Leaf,
           HistoricalProtectedStage5RankProgressProperty,
           HistoricalProtectedStageRankProgressProperty

(***************************************************************************
Stage 6 Completion capacity.

The physical FIFO and selected ready Completion are two different owners.
The first rank is the concrete I/O depth and consumes only the existing
historical-I/O-worker fairness clause.  Once that FIFO is empty, the selected
ready owner is protected by the historical Stage-4 theorem.
***************************************************************************)

HistoricalTemporalStage6CompletionGoal(candidate, position) ==
  \/ HistoricalTemporalRankProgressExit(candidate, <<6, position>>)
  \/ HistoricalTemporalStage6OwedCausalReady(candidate, position)

HistoricalTemporalStage6CompletionIoDrainGoal(candidate, position) ==
  \/ HistoricalTemporalStage6CompletionGoal(candidate, position)
  \/ /\ HistoricalTemporalStage6CompletionCapacityBlocked(
          candidate, position)
     /\ AsyncIoQueueDepth(candidate.node) = 0

HistoricalTemporalStage6CompletionIoBlockedAtDepth(
    candidate, position, depth) ==
  /\ HistoricalTemporalStage6CompletionCapacityBlocked(
       candidate, position)
  /\ AsyncIoQueueDepth(candidate.node) > 0
  /\ AsyncIoQueueDepth(candidate.node) = depth

HistoricalTemporalStage6CompletionIoProgress(
    candidate, position, depth) ==
  \/ HistoricalTemporalStage6CompletionIoDrainGoal(
       candidate, position)
  \/ \E lower \in SetLessThan(depth, OpToRel(<, Nat), Nat):
       HistoricalTemporalStage6CompletionIoBlockedAtDepth(
         candidate, position, lower)

THEOREM HistoricalTemporalStage6CompletionIoBlockedCoreFacts ==
  \A candidate, position, depth:
    HistoricalTemporalStage6CompletionIoBlockedAtDepth(
      candidate, position, depth)
      => /\ candidate.node \in ValidatorIds
         /\ candidate.node \in Responsive
         /\ HistoricalRecoveryTarget(candidate.node)
         /\ AsyncStrongTypeInvariant
         /\ AsyncTypeInvariant
         /\ AsyncProgressOwnershipInvariant
         /\ CompletionCausalAdmissionDebt(candidate.node)
         /\ ~CausalHeadCanAdvance(candidate.node)
         /\ AsyncIoQueueDepth(candidate.node) \in Nat \ {0}
         /\ depth \in Nat \ {0}
PROOF
  <1>1. ASSUME NEW candidate, NEW position, NEW depth,
                HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                  candidate, position, depth)
         PROVE /\ candidate.node \in ValidatorIds
               /\ candidate.node \in Responsive
               /\ HistoricalRecoveryTarget(candidate.node)
               /\ AsyncStrongTypeInvariant
               /\ AsyncTypeInvariant
               /\ AsyncProgressOwnershipInvariant
               /\ CompletionCausalAdmissionDebt(candidate.node)
               /\ ~CausalHeadCanAdvance(candidate.node)
               /\ AsyncIoQueueDepth(candidate.node) \in Nat \ {0}
               /\ depth \in Nat \ {0}
    <2>1. /\ HistoricalTemporalStage6Pending(candidate, position)
           /\ AsyncIoQueueDepth(candidate.node) > 0
           /\ AsyncIoQueueDepth(candidate.node) = depth
      BY <1>1
         DEF HistoricalTemporalStage6CompletionIoBlockedAtDepth,
             HistoricalTemporalStage6CompletionCapacityBlocked
    <2>2. /\ candidate.node \in ValidatorIds
           /\ candidate.node \in Responsive
           /\ HistoricalRecoveryTarget(candidate.node)
      BY <2>1, HistoricalTemporalStage6CarrierFacts
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
      BY <2>1 DEF HistoricalTemporalStage6Pending
    <2>4. AsyncTypeInvariant
      BY <2>3, AsyncStrongTypeProjectsAsyncType
    <2>5. AsyncIoSequenceTyped(asyncIoQueues[candidate.node])
      BY <2>2, <2>4
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant
    <2>6. AsyncIoQueueDepth(candidate.node) \in Nat \ {0}
      BY <2>1, <2>5, LenProperties, SMT
         DEF AsyncIoSequenceTyped, AsyncIoQueueDepth
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>6
         DEF HistoricalTemporalStage6CompletionIoBlockedAtDepth,
             HistoricalTemporalStage6CompletionCapacityBlocked
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage6CompletionIoWorkerStrictlyProgresses ==
  \A candidate, position:
    \A depth \in Nat:
      /\ HistoricalTemporalStage6CompletionIoBlockedAtDepth(
           candidate, position, depth)
      /\ [AsyncNext]_AsyncAllVars
      /\ PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      => HistoricalTemporalStage6CompletionIoProgress(
           candidate, position, depth)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position, NEW depth \in Nat,
                HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                  candidate, position, depth),
                [AsyncNext]_AsyncAllVars,
                PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
         PROVE HistoricalTemporalStage6CompletionIoProgress(
                 candidate, position, depth)'
    <2>1. /\ candidate.node \in ValidatorIds
           /\ AsyncTypeInvariant
           /\ AsyncIoQueueDepth(candidate.node) \in Nat \ {0}
           /\ AsyncIoQueueDepth(candidate.node) = depth
      BY <1>1,
         HistoricalTemporalStage6CompletionIoBlockedCoreFacts
    <2>2. ServiceIoWorkerWork(candidate.node)
      BY <1>1
         DEF PostGstServiceHistoricalRecoveryIoWorker,
             ServiceHistoricalRecoveryIoWorker
    <2>3. AsyncIoQueueDepth(candidate.node)' + 1 = depth
      BY <2>1, <2>2, ServiceIoWorkerDropsQueueDepth
    <2>4. CASE HistoricalTemporalStage6CompletionIoDrainGoal(
                  candidate, position)'
      BY <2>4
         DEF HistoricalTemporalStage6CompletionIoProgress
    <2>5. CASE ~HistoricalTemporalStage6CompletionIoDrainGoal(
                  candidate, position)'
      <3>1. /\ HistoricalTemporalStage6Pending(candidate, position)'
             /\ CompletionCausalAdmissionDebt(candidate.node)'
             /\ ~CausalHeadCanAdvance(candidate.node)'
             /\ AsyncIoQueueDepth(candidate.node)' > 0
        BY <1>1, <2>5,
           AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncBracketNextPreservesProgressOwnership, Isa
           DEF HistoricalTemporalStage6CompletionIoDrainGoal,
               HistoricalTemporalStage6CompletionGoal,
               HistoricalTemporalStage6Pending,
               HistoricalTemporalRankProgressExit,
               HistoricalTemporalStage6OwedCausalReady,
               HistoricalProtectedOwnedAtServiceRank,
               HistoricalProtectedServiceOwnershipExit,
               HistoricalProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateServiceRank,
               ServiceRankLess, CausalCandidatePosition,
               CompletionCausalAdmissionDebt,
               CausalAdmissionDebtActive,
               PostGstServiceHistoricalRecoveryIoWorker,
               ServiceHistoricalRecoveryIoWorker,
               ServiceIoWorkerWork, LeaveCausalQueues,
               AsyncLocalAdmissionVars, CandidateScheduled,
               CandidateInFlight, CausalHeadCanAdvance,
               CausalCandidates, SequenceSet,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncOutstandingCarrierInvariant, AsyncAllVars
      <3>2. AsyncIoQueueDepth(candidate.node)' \in Nat \ {0}
        BY <2>3, <3>1, SMT
      <3>3. AsyncIoQueueDepth(candidate.node)'
               \in SetLessThan(depth, OpToRel(<, Nat), Nat)
        BY <2>3, <3>2, SMT DEF SetLessThan, OpToRel
      <3>4. \E lower \in SetLessThan(
                    depth, OpToRel(<, Nat), Nat):
                 HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                   candidate, position, lower)'
        BY <3>1, <3>3, Isa
           DEF HistoricalTemporalStage6CompletionIoBlockedAtDepth,
               HistoricalTemporalStage6CompletionCapacityBlocked
      <3> QED BY <3>4
           DEF HistoricalTemporalStage6CompletionIoProgress
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage6CompletionIoOtherStep ==
  \A candidate, position:
    \A depth \in Nat:
      /\ HistoricalTemporalStage6CompletionIoBlockedAtDepth(
           candidate, position, depth)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      => \/ HistoricalTemporalStage6CompletionIoBlockedAtDepth(
              candidate, position, depth)'
         \/ HistoricalTemporalStage6CompletionIoProgress(
              candidate, position, depth)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HeadTailProperties, SequenceSetAfterAppend, IsaT(600)
   DEF HistoricalTemporalStage6CompletionIoBlockedAtDepth,
       HistoricalTemporalStage6CompletionIoProgress,
       HistoricalTemporalStage6CompletionIoDrainGoal,
       HistoricalTemporalStage6CompletionGoal,
       HistoricalTemporalStage6CompletionCapacityBlocked,
       HistoricalTemporalStage6OwedCausalReady,
       HistoricalTemporalStage6Pending,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateServiceRank,
       ServiceRankLess, CausalCandidatePosition,
       LocalSourceDistance, CompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive, CandidateScheduled,
       CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueIoClass, AsyncIoAdmissionLimit,
       AsyncIoQueueDepth, AsyncOutstandingWorkCount,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       RunHistoricalServer, OpenHistoricalRecovery,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       LocalAdmissionStep, AdmitProducerCompletion,
       AdmitCausalHead, RecordBlockedCausalDebt,
       UpdateLocalAdmissionMetadata, IngressDrainStep,
       DrainFairIngressSelected, IngressItemCanDrain,
       PopSelectedIngress, SerializedRunnerRuntimeStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, RuntimeStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, AsyncNetworkStep,
       AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
       AsyncTick, AsyncNonClockVars, AsyncSetGST,
       EnqueueCandidate, AppendCausalSuccessors,
       CandidateInReadyQueue, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalStage6CompletionIoEnablesWorker ==
  \A candidate, position, depth:
    HistoricalTemporalStage6CompletionIoBlockedAtDepth(
      candidate, position, depth)
      => ENABLED
           <<PostGstServiceHistoricalRecoveryIoWorker(
               candidate.node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW candidate, NEW position, NEW depth,
                HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                  candidate, position, depth)
         PROVE ENABLED
                 <<PostGstServiceHistoricalRecoveryIoWorker(
                     candidate.node)>>_AsyncAllVars
    <2>1. /\ candidate.node \in asyncHistoricalRecoveryTargets
           /\ AsyncStrongTypeInvariant
           /\ AsyncTypeInvariant
           /\ gst
           /\ AsyncIoQueueDepth(candidate.node) > 0
      BY <1>1,
         HistoricalTemporalStage6CompletionIoBlockedCoreFacts
         DEF HistoricalTemporalStage6CompletionIoBlockedAtDepth,
             HistoricalTemporalStage6CompletionCapacityBlocked,
             HistoricalTemporalStage6Pending,
             HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedCandidateOwned,
             HistoricalRecoveryTarget
    <2>2. ENABLED
             PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <2>1, HistoricalRecoveryIoWorkerEnabledAfterGst
    <2>3. PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
             => <<PostGstServiceHistoricalRecoveryIoWorker(
                    candidate.node)>>_AsyncAllVars
      BY <2>1, HistoricalTemporalQueuedIoServiceIsNonstuttering
    <2> QED BY <2>2, <2>3, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage6CompletionIoOneStep ==
  \A initialContext, candidate, position:
    \A depth \in Nat:
      AsyncSpecAt(initialContext)
        => (HistoricalTemporalStage6CompletionIoBlockedAtDepth(
              candidate, position, depth)
              ~> HistoricalTemporalStage6CompletionIoProgress(
                   candidate, position, depth))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW depth \in Nat
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                       candidate, position, depth)
                       ~> HistoricalTemporalStage6CompletionIoProgress(
                            candidate, position, depth))
    <2>1. /\ HistoricalTemporalStage6CompletionIoBlockedAtDepth(
               candidate, position, depth)
             /\ ~HistoricalTemporalStage6CompletionIoProgress(
                  candidate, position, depth)
            => ENABLED
                 <<PostGstServiceHistoricalRecoveryIoWorker(
                     candidate.node)>>_AsyncAllVars
      BY HistoricalTemporalStage6CompletionIoEnablesWorker
    <2>2. /\ HistoricalTemporalStage6CompletionIoBlockedAtDepth(
               candidate, position, depth)
             /\ ~HistoricalTemporalStage6CompletionIoProgress(
                  candidate, position, depth)
             /\ <<PostGstServiceHistoricalRecoveryIoWorker(
                    candidate.node)>>_AsyncAllVars
            => HistoricalTemporalStage6CompletionIoProgress(
                 candidate, position, depth)'
      BY HistoricalTemporalStage6CompletionIoWorkerStrictlyProgresses,
         Isa DEF PostGstServiceHistoricalRecoveryIoWorker
    <2>3. HistoricalTemporalStage6CompletionIoBlockedAtDepth(
             candidate, position, depth)
             /\ [AsyncNext]_AsyncAllVars
            => \/ HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                    candidate, position, depth)'
               \/ HistoricalTemporalStage6CompletionIoProgress(
                    candidate, position, depth)'
      BY HistoricalTemporalStage6CompletionIoWorkerStrictlyProgresses,
         HistoricalTemporalStage6CompletionIoOtherStep, Isa
    <2>4. CASE candidate.node \in Responsive
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(
                    PostGstServiceHistoricalRecoveryIoWorker(
                      candidate.node))
        BY <2>4 DEF AsyncSpecAt, AsyncFairnessAt
      <3> QED BY <2>1, <2>2, <2>3, <3>1, PTL
           DEF AsyncSpecAt
    <2>5. CASE candidate.node \notin Responsive
      <3>1. AsyncSpecAt(initialContext)
               => []~HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                      candidate, position, depth)
        BY <2>5, PTL
           DEF HistoricalTemporalStage6CompletionIoBlockedAtDepth,
               HistoricalTemporalStage6CompletionCapacityBlocked,
               HistoricalTemporalStage6Pending,
               HistoricalProtectedOwnedAtServiceRank,
               HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage6CompletionIoDrains ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => \A depth \in Nat:
           HistoricalTemporalStage6CompletionIoBlockedAtDepth(
             candidate, position, depth)
             ~> HistoricalTemporalStage6CompletionIoDrainGoal(
                  candidate, position)
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position
         PROVE AsyncSpecAt(initialContext)
                 => \A depth \in Nat:
                      HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                        candidate, position, depth)
                        ~> HistoricalTemporalStage6CompletionIoDrainGoal(
                             candidate, position)
    <2>1. AsyncSpecAt(initialContext)
             => \A depth \in Nat:
                  HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                    candidate, position, depth)
                    ~> (HistoricalTemporalStage6CompletionIoDrainGoal(
                          candidate, position)
                         \/ \E lower \in SetLessThan(
                              depth, OpToRel(<, Nat), Nat):
                              HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                                candidate, position, lower))
      BY HistoricalTemporalFairStage6CompletionIoOneStep
         DEF HistoricalTemporalStage6CompletionIoProgress
    <2> QED BY <2>1, NatLessThanWellFounded,
         WellFoundedLeadsTo
  <1> QED BY <1>1

HistoricalTemporalStage6CompletionReadyBlocked(candidate, position) ==
  /\ HistoricalTemporalStage6CompletionCapacityBlocked(
       candidate, position)
  /\ AsyncIoQueueDepth(candidate.node) = 0

HistoricalTemporalStage6CompletionReadyWitnessBlocked(
    candidate, position, readyCandidate, readyPosition) ==
  /\ HistoricalTemporalStage6CompletionReadyBlocked(
       candidate, position)
  /\ readyCandidate \in AsyncCandidateSet
  /\ readyPosition \in Nat
  /\ HistoricalProtectedOwnedAtServiceRank(
       readyCandidate, <<4, readyPosition>>)

THEOREM HistoricalTemporalStage6CompletionZeroIoReadyFacts ==
  \A candidate, position:
    HistoricalTemporalStage6CompletionReadyBlocked(
      candidate, position)
      => /\ candidate.node \in ValidatorIds
         /\ candidate.node \in Responsive
         /\ HistoricalRecoveryTarget(candidate.node)
         /\ AsyncOutstandingWorkCount(candidate.node)
               = AsyncIoWorkCapacity
         /\ SelectedCompletionQueueNonempty(candidate.node)
         /\ SelectedCompletionCandidate(candidate.node)
               \in AsyncCandidateSet
         /\ SelectedCompletionCandidate(candidate.node)
               \in asyncOutstandingWork[candidate.node]
         /\ SelectedCompletionCandidate(candidate.node).class
               = "Completion"
         /\ SelectedCompletionCandidate(candidate.node).node
               = candidate.node
         /\ CandidateInReadyQueue(
              SelectedCompletionCandidate(candidate.node))
         /\ ReadyCandidatePosition(
              SelectedCompletionCandidate(candidate.node)) \in Nat
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                HistoricalTemporalStage6CompletionReadyBlocked(
                  candidate, position)
         PROVE /\ candidate.node \in ValidatorIds
               /\ candidate.node \in Responsive
               /\ HistoricalRecoveryTarget(candidate.node)
               /\ AsyncOutstandingWorkCount(candidate.node)
                     = AsyncIoWorkCapacity
               /\ SelectedCompletionQueueNonempty(candidate.node)
               /\ SelectedCompletionCandidate(candidate.node)
                     \in AsyncCandidateSet
               /\ SelectedCompletionCandidate(candidate.node)
                     \in asyncOutstandingWork[candidate.node]
               /\ SelectedCompletionCandidate(candidate.node).class
                     = "Completion"
               /\ SelectedCompletionCandidate(candidate.node).node
                     = candidate.node
               /\ CandidateInReadyQueue(
                    SelectedCompletionCandidate(candidate.node))
               /\ ReadyCandidatePosition(
                    SelectedCompletionCandidate(candidate.node)) \in Nat
    <2> DEFINE Node == candidate.node
    <2> DEFINE Ready == SelectedCompletionCandidate(Node)
    <2>1. /\ HistoricalTemporalStage6Pending(candidate, position)
           /\ CompletionCausalAdmissionDebt(Node)
           /\ ~CausalHeadCanAdvance(Node)
           /\ AsyncIoQueueDepth(Node) = 0
      BY <1>1
         DEF HistoricalTemporalStage6CompletionReadyBlocked,
             HistoricalTemporalStage6CompletionCapacityBlocked, Node
    <2>2. /\ Node \in ValidatorIds
           /\ Node \in Responsive
           /\ HistoricalRecoveryTarget(Node)
           /\ AsyncStrongTypeInvariant
           /\ AsyncTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ CausalQueueNonempty(Node)
           /\ HeadCausalCandidate(Node).class = "Completion"
      BY <2>1, HistoricalTemporalStage6CarrierFacts,
         AsyncStrongTypeProjectsAsyncType
         DEF HistoricalTemporalStage6Pending,
             CompletionCausalAdmissionDebt,
             CausalAdmissionDebtActive
    <2>3. ~CandidateInFlight(HeadCausalCandidate(Node))
      BY <2>2, OwnedCausalHeadIsNotInFlight
    <2>4. CanEnqueueIoClass(Node, "Consensus")
      BY <2>1, <2>2, SMT
         DEF CanEnqueueIoClass, AsyncIoAdmissionLimit,
             AsyncIoQueueDepth, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
    <2>5. ~(AsyncOutstandingWorkCount(Node)
                  < AsyncIoWorkCapacity)
      BY <2>1, <2>2, <2>3, <2>4
         DEF CausalHeadCanAdvance
    <2>6. /\ AsyncOutstandingWorkCount(Node)
                  <= AsyncIoWorkCapacity
           /\ AsyncIoWorkCapacity \in Nat \ {0}
      BY <2>2
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoCapacityTypeInvariant,
             AsyncConfiguration
    <2>7. /\ AsyncOutstandingWorkCount(Node)
                  = AsyncIoWorkCapacity
           /\ AsyncOutstandingWorkCount(Node) > 0
      BY <2>5, <2>6, SMT
    <2>8. ConsensusIoCandidates(Node) = {}
      BY <2>1, Isa
         DEF ConsensusIoCandidates, AsyncIoQueueDepth, SequenceSet
    <2>9. asyncOutstandingWork[Node] =
             SequenceSet(asyncIoReadyCompletions[Node])
               \cup SequenceSet(asyncLocalReadyCompletions[Node])
      BY <2>2, <2>8
         DEF AsyncProgressOwnershipInvariant,
             AsyncOutstandingCarrierInvariant
    <2>10. asyncOutstandingWork[Node] # {}
      BY <2>2, <2>7, FS_CardinalityType, FS_EmptySet, SMT
         DEF AsyncOutstandingWorkCount, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant,
             AsyncIoWorkContentTypeInvariant
    <2>11. SelectedCompletionQueueNonempty(Node)
      BY <2>9, <2>10, Isa
         DEF SelectedCompletionQueueNonempty,
             SelectedCompletionSource, SequenceSet
    <2>12. /\ AsyncIoTopologyTypeInvariant
            /\ AsyncIoWorkContentTypeInvariant
      BY <2>2
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant
    <2>13. /\ Ready \in asyncOutstandingWork[Node]
            /\ AsyncCandidateTyped(Ready)
            /\ Ready.class = "Completion"
            /\ Ready.node = Node
            /\ Ready \in
                 SequenceSet(asyncIoReadyCompletions[Node])
                   \cup SequenceSet(asyncLocalReadyCompletions[Node])
      BY <2>11, <2>12,
         SelectedReadyCompletionFactsWithoutRuntimeCapacity, Isa
         DEF Ready, ProducerSelectedReadyQueue,
             SelectedCompletionSource
    <2>14. Ready \in AsyncCandidateSet
      BY <2>13, Isa
         DEF AsyncCandidateTyped, AsyncCandidateSet,
             AsyncCandidateDomain
    <2>15. /\ CandidateScheduled(Ready)
            /\ CandidateInReadyQueue(Ready)
      BY <2>13, Isa
         DEF CandidateScheduled, TrackedWorkCandidates,
             CandidateInReadyQueue
    <2>16. ReadyCandidatePosition(Ready) \in Nat
      BY <2>2, <2>15, ReadyCandidatePositionIsNatural
    <2> QED BY <2>2, <2>7, <2>11, <2>13, <2>14, <2>15,
         <2>16 DEF Node, Ready
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage6CompletionReadyWitnessExists ==
  \A candidate, position:
    HistoricalTemporalStage6CompletionReadyBlocked(
      candidate, position)
      => \E readyCandidate \in AsyncCandidateSet,
            readyPosition \in Nat:
           HistoricalTemporalStage6CompletionReadyWitnessBlocked(
             candidate, position, readyCandidate, readyPosition)
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                HistoricalTemporalStage6CompletionReadyBlocked(
                  candidate, position)
         PROVE \E readyCandidate \in AsyncCandidateSet,
                   readyPosition \in Nat:
                  HistoricalTemporalStage6CompletionReadyWitnessBlocked(
                    candidate, position,
                    readyCandidate, readyPosition)
    <2> DEFINE Ready == SelectedCompletionCandidate(candidate.node)
    <2> DEFINE Position == ReadyCandidatePosition(Ready)
    <2>1. /\ candidate.node \in ValidatorIds
           /\ candidate.node \in Responsive
           /\ HistoricalRecoveryTarget(candidate.node)
           /\ Ready \in AsyncCandidateSet
           /\ Ready \in asyncOutstandingWork[candidate.node]
           /\ CandidateInReadyQueue(Ready)
           /\ Position \in Nat
      BY <1>1,
         HistoricalTemporalStage6CompletionZeroIoReadyFacts
         DEF Ready, Position
    <2>2. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ gst
           /\ ~NodeHasApplication(candidate.node)
      BY <1>1
         DEF HistoricalTemporalStage6CompletionReadyBlocked,
             HistoricalTemporalStage6CompletionCapacityBlocked,
             HistoricalTemporalStage6Pending,
             HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedCandidateOwned,
             ProtectedCandidateOwned
    <2>3. /\ Ready.node = candidate.node
           /\ Ready.class = "Completion"
      BY <1>1, <2>1,
         HistoricalTemporalStage6CompletionZeroIoReadyFacts
         DEF Ready
    <2>4. /\ Ready \notin QueuedCandidates
           /\ Ready \notin DeferredCandidates
           /\ Ready \in TrackedWorkCandidates
      BY <2>1, <2>2, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant,
             TrackedWorkCandidates
    <2>5. CandidateServiceRank(Ready) = <<4, Position>>
      BY <2>1, <2>4, Isa
         DEF CandidateServiceRank, ReadyCandidatePosition
    <2>6. HistoricalProtectedCandidateOwned(Ready)
      BY <2>1, <2>2, <2>3, <2>4, Isa
         DEF HistoricalProtectedCandidateOwned,
             ProtectedCandidateOwned, ProtectedServiceCandidate,
             CandidateScheduled
    <2>7. HistoricalProtectedOwnedAtServiceRank(
             Ready, <<4, Position>>)
      BY <2>2, <2>5, <2>6
         DEF HistoricalProtectedOwnedAtServiceRank
    <2>8. HistoricalTemporalStage6CompletionReadyWitnessBlocked(
             candidate, position, Ready, Position)
      BY <1>1, <2>1, <2>7
         DEF HistoricalTemporalStage6CompletionReadyWitnessBlocked
    <2> QED BY <2>1, <2>8
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage6CompletionReadyProgressOpens ==
  \A candidate, position, readyCandidate, readyPosition:
    /\ HistoricalTemporalStage6CompletionReadyWitnessBlocked(
         candidate, position, readyCandidate, readyPosition)
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalTemporalRankProgressExit(
         readyCandidate, <<4, readyPosition>>)'
    => HistoricalTemporalStage6CompletionGoal(
         candidate, position)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   FS_RemoveElement, FS_CardinalityType,
   HeadTailProperties, SequenceSetAfterAppend, IsaT(600)
   DEF HistoricalTemporalStage6CompletionReadyWitnessBlocked,
       HistoricalTemporalStage6CompletionReadyBlocked,
       HistoricalTemporalStage6CompletionGoal,
       HistoricalTemporalStage6CompletionCapacityBlocked,
       HistoricalTemporalStage6OwedCausalReady,
       HistoricalTemporalStage6Pending,
       HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateServiceRank, ServiceRankLess,
       ReadyCandidatePosition, ReadyCandidateSource,
       ReadyCompletionQueue, LocalSourceDistance,
       PreferredLocalSource, CompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive, CandidateScheduled,
       CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueIoClass, AsyncIoAdmissionLimit,
       AsyncIoQueueDepth, AsyncOutstandingWorkCount,
       SelectedCompletionSource, SelectedCompletionCandidate,
       SelectedCompletionQueueNonempty,
       ProducerCompletionCanAdmit, ProducerCompletionCanAdvance,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       RunHistoricalServer, OpenHistoricalRecovery,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       LocalAdmissionStep, AdmitProducerCompletion,
       AdmitCausalHead, RecordBlockedCausalDebt,
       UpdateLocalAdmissionMetadata, IngressDrainStep,
       DrainFairIngressSelected, IngressItemCanDrain,
       PopSelectedIngress, SerializedRunnerRuntimeStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, EnqueueIoLocalControl,
       EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, AsyncNetworkStep,
       AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
       AsyncTick, AsyncNonClockVars, AsyncSetGST,
       EnqueueCandidate, AppendCausalSuccessors,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       SequenceWithoutIndex, DeferCommand, DiscardCommand,
       CandidateInReadyQueue, QueuedCandidates,
       DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, ConsensusIoCandidates,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalStage6CompletionReadyWitnessUnless ==
  \A candidate, position, readyCandidate, readyPosition:
    /\ HistoricalTemporalStage6CompletionReadyWitnessBlocked(
         candidate, position, readyCandidate, readyPosition)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalTemporalStage6CompletionReadyWitnessBlocked(
             candidate, position, readyCandidate, readyPosition)'
       \/ HistoricalTemporalStage6CompletionGoal(
            candidate, position)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                NEW readyCandidate, NEW readyPosition,
                HistoricalTemporalStage6CompletionReadyWitnessBlocked(
                  candidate, position,
                  readyCandidate, readyPosition),
                [AsyncNext]_AsyncAllVars
         PROVE \/ HistoricalTemporalStage6CompletionReadyWitnessBlocked(
                      candidate, position,
                      readyCandidate, readyPosition)'
               \/ HistoricalTemporalStage6CompletionGoal(
                    candidate, position)'
    <2>1. HistoricalTemporalStage4Pending(
             readyCandidate, readyPosition)
      BY <1>1
         DEF HistoricalTemporalStage6CompletionReadyWitnessBlocked,
             HistoricalTemporalStage4Pending
    <2>2. \/ HistoricalTemporalStage4Pending(
                 readyCandidate, readyPosition)'
           \/ HistoricalTemporalRankProgressExit(
                readyCandidate, <<4, readyPosition>>)'
      BY <1>1, <2>1, HistoricalTemporalStage4UnlessProgress
    <2>3. CASE HistoricalTemporalRankProgressExit(
                  readyCandidate, <<4, readyPosition>>)'
      BY <1>1, <2>3,
         HistoricalTemporalStage6CompletionReadyProgressOpens
    <2>4. CASE HistoricalTemporalStage4Pending(
                  readyCandidate, readyPosition)'
      <3>1. CASE HistoricalTemporalStage6CompletionGoal(
                    candidate, position)'
        BY <3>1
      <3>2. CASE ~HistoricalTemporalStage6CompletionGoal(
                    candidate, position)'
        <4>1. HistoricalTemporalStage6CompletionReadyBlocked(
                 candidate, position)'
          BY <1>1, <3>2,
             AsyncBracketNextPreservesStrongTypeInvariant,
             AsyncBracketNextPreservesProgressOwnership,
             HeadTailProperties, SequenceSetAfterAppend,
             IsaT(600)
             DEF HistoricalTemporalStage6CompletionReadyWitnessBlocked,
                 HistoricalTemporalStage6CompletionReadyBlocked,
                 HistoricalTemporalStage6CompletionGoal,
                 HistoricalTemporalStage6CompletionCapacityBlocked,
                 HistoricalTemporalStage6OwedCausalReady,
                 HistoricalTemporalStage6Pending,
                 HistoricalTemporalRankProgressExit,
                 HistoricalProtectedOwnedAtServiceRank,
                 HistoricalProtectedServiceOwnershipExit,
                 HistoricalProtectedCandidateOwned,
                 ProtectedCandidateOwned, CandidateServiceRank,
                 ServiceRankLess, CompletionCausalAdmissionDebt,
                 CausalAdmissionDebtActive, CandidateScheduled,
                 CandidateInFlight, CausalHeadCanAdvance,
                 CanEnqueueIoClass, AsyncIoAdmissionLimit,
                 AsyncIoQueueDepth, AsyncOutstandingWorkCount,
                 AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
                 AsyncNonRunnerStep, RunNode,
                 RunHistoricalRecoveryNode, RunNodeWork,
                 ResolveRunNodeCandidateProducerContinuation,
                 AsyncSchedulerExceptCausalControlAndNodeService,
                 SerializedLocalPrecedesServeIngressStep,
                 SelectedLocalAdmissionAdvance,
                 AsyncServeIngressTargetOnlyTurn,
                 SerializedRunnerRuntimeStep,
                 SerializedRuntimePrecedesServeIngressStep,
                 RunHistoricalServer, OpenHistoricalRecovery,
                 LocalAdmissionStep, AdmitProducerCompletion,
                 AdmitCausalHead, RecordBlockedCausalDebt,
                 UpdateLocalAdmissionMetadata, IngressDrainStep,
                 DrainFairIngressSelected, IngressItemCanDrain,
                 PopSelectedIngress, SerializedRuntimeStep,
                 RuntimeStep,
                 DirectCommitCertificateDiscoveryStep,
                 DirectHistoricalCommitCertificateDiscoveryStep,
                 CommitCertificateDiscoveryStepWork,
                 ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
                 ServiceIoWorkerWork, EnqueueIoLocalControl,
                 EnqueueHistoricalRecoveryIoLocalControl,
                 EnqueueIoLocalControlWork, AsyncNetworkStep,
                 AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
                 AsyncTick, AsyncNonClockVars, AsyncSetGST,
                 EnqueueCandidate, AppendCausalSuccessors,
                 CandidateInReadyQueue, CausalCandidates,
                 TrackedWorkCandidates, ConsensusIoCandidates,
                 SequenceSet, AsyncProgressOwnershipInvariant,
                 AsyncLogicalCandidateOwnershipInvariant,
                 AsyncOutstandingCarrierInvariant, AsyncAllVars
        <4>2. HistoricalTemporalStage6CompletionReadyWitnessBlocked(
                 candidate, position,
                 readyCandidate, readyPosition)'
          BY <1>1, <3>2, <4>1, <2>4
             DEF HistoricalTemporalStage6CompletionReadyWitnessBlocked,
                 HistoricalTemporalStage4Pending,
                 HistoricalProtectedOwnedAtServiceRank
        <4> QED BY <4>2
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage4LeafGoalImpliesRankExit ==
  \A candidate, position:
    /\ position \in Nat
    /\ (HistoricalProtectedServiceOwnershipExit(candidate)
         \/ \E lower \in SetLessThan(
              <<4, position>>,
              OwnedServiceRankOrdering,
              OwnedServiceRankCarrier):
              HistoricalProtectedOwnedAtServiceRank(
                candidate, lower))
    => HistoricalTemporalRankProgressExit(
         candidate, <<4, position>>)
BY OwnedServiceRankOrderingMatchesLess, Isa
   DEF HistoricalTemporalRankProgressExit,
       HistoricalProtectedOwnedAtServiceRank, SetLessThan,
       OwnedServiceRankCarrier

THEOREM HistoricalTemporalFairStage6CompletionReadyWitnessOpens ==
  \A initialContext, candidate, position:
    \A readyCandidate \in AsyncCandidateSet,
       readyPosition \in Nat:
      HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
        AsyncSpecAt(initialContext))
        => (AsyncSpecAt(initialContext)
              => (HistoricalTemporalStage6CompletionReadyWitnessBlocked(
                    candidate, position, readyCandidate, readyPosition)
                    ~> HistoricalTemporalStage6CompletionGoal(
                         candidate, position)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                NEW readyCandidate \in AsyncCandidateSet,
                NEW readyPosition \in Nat,
                HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage6CompletionReadyWitnessBlocked(
                       candidate, position,
                       readyCandidate, readyPosition)
                       ~> HistoricalTemporalStage6CompletionGoal(
                            candidate, position))
    <2>1. HistoricalProtectedStage4RankProgressProperty(
             AsyncSpecAt(initialContext))
      BY AsyncSpecClosesHistoricalTemporalStage4Leaf
    <2>2. AsyncSpecAt(initialContext)
             => (HistoricalProtectedOwnedAtServiceRank(
                   readyCandidate, <<4, readyPosition>>)
                   ~> (HistoricalProtectedServiceOwnershipExit(
                          readyCandidate)
                        \/ \E lower \in SetLessThan(
                             <<4, readyPosition>>,
                             OwnedServiceRankOrdering,
                             OwnedServiceRankCarrier):
                             HistoricalProtectedOwnedAtServiceRank(
                               readyCandidate, lower)))
      BY <2>1
         DEF HistoricalProtectedStage4RankProgressProperty,
             HistoricalProtectedStageRankProgressProperty,
             HistoricalProtectedOwnedAtServiceRank
    <2>3. AsyncSpecAt(initialContext)
             => (HistoricalProtectedOwnedAtServiceRank(
                   readyCandidate, <<4, readyPosition>>)
                   ~> HistoricalTemporalRankProgressExit(
                        readyCandidate, <<4, readyPosition>>))
      BY <2>2,
         HistoricalTemporalStage4LeafGoalImpliesRankExit, PTL
    <2>4. HistoricalTemporalStage6CompletionReadyWitnessBlocked(
             candidate, position, readyCandidate, readyPosition)
             /\ [AsyncNext]_AsyncAllVars
            => \/ HistoricalTemporalStage6CompletionReadyWitnessBlocked(
                    candidate, position,
                    readyCandidate, readyPosition)'
               \/ HistoricalTemporalStage6CompletionGoal(
                    candidate, position)'
      BY HistoricalTemporalStage6CompletionReadyWitnessUnless
    <2>5. HistoricalTemporalStage6CompletionReadyWitnessBlocked(
             candidate, position, readyCandidate, readyPosition)
            => /\ HistoricalProtectedOwnedAtServiceRank(
                     readyCandidate, <<4, readyPosition>>)
               /\ ~HistoricalTemporalRankProgressExit(
                    readyCandidate, <<4, readyPosition>>)
      BY Isa
         DEF HistoricalTemporalStage6CompletionReadyWitnessBlocked,
             HistoricalTemporalRankProgressExit,
             HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedServiceOwnershipExit,
             ServiceRankLess
    <2> QED BY <2>3, <2>4, <2>5, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage6CompletionReadyOpens ==
  \A initialContext, candidate, position:
    HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => (HistoricalTemporalStage6CompletionReadyBlocked(
                  candidate, position)
                  ~> HistoricalTemporalStage6CompletionGoal(
                       candidate, position)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage6CompletionReadyBlocked(
                       candidate, position)
                       ~> HistoricalTemporalStage6CompletionGoal(
                            candidate, position))
    <2>1. HistoricalTemporalStage6CompletionReadyBlocked(
             candidate, position)
             => \E readyCandidate \in AsyncCandidateSet,
                   readyPosition \in Nat:
                  HistoricalTemporalStage6CompletionReadyWitnessBlocked(
                    candidate, position,
                    readyCandidate, readyPosition)
      BY HistoricalTemporalStage6CompletionReadyWitnessExists
    <2>2. AsyncSpecAt(initialContext)
             => \A readyCandidate \in AsyncCandidateSet,
                   readyPosition \in Nat:
                  HistoricalTemporalStage6CompletionReadyWitnessBlocked(
                    candidate, position,
                    readyCandidate, readyPosition)
                    ~> HistoricalTemporalStage6CompletionGoal(
                         candidate, position)
      BY HistoricalTemporalFairStage6CompletionReadyWitnessOpens
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM HistoricalTemporalFairStage6CompletionCapacityOpens ==
  \A initialContext, candidate, position:
    HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => (HistoricalTemporalStage6CompletionCapacityBlocked(
                  candidate, position)
                  ~> HistoricalTemporalStage6CompletionGoal(
                       candidate, position)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (HistoricalTemporalStage6CompletionCapacityBlocked(
                       candidate, position)
                       ~> HistoricalTemporalStage6CompletionGoal(
                            candidate, position))
    <2>1. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2>2. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage6CompletionCapacityBlocked(
                   candidate, position)
                   ~> (HistoricalTemporalStage6CompletionGoal(
                         candidate, position)
                        \/ HistoricalTemporalStage6CompletionReadyBlocked(
                             candidate, position)))
      <3>1. AsyncSpecAt(initialContext)
               => ((HistoricalTemporalStage6CompletionCapacityBlocked(
                       candidate, position)
                      /\ AsyncIoQueueDepth(candidate.node) > 0)
                     ~> HistoricalTemporalStage6CompletionIoDrainGoal(
                          candidate, position))
        <4>1. AsyncSpecAt(initialContext)
                 => ((HistoricalTemporalStage6CompletionCapacityBlocked(
                         candidate, position)
                        /\ AsyncIoQueueDepth(candidate.node) > 0)
                       ~> HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                            candidate, position,
                            AsyncIoQueueDepth(candidate.node)))
          BY <2>1, PTL
             DEF HistoricalTemporalStage6CompletionIoBlockedAtDepth
        <4>2. AsyncSpecAt(initialContext)
                 => \A depth \in Nat:
                      HistoricalTemporalStage6CompletionIoBlockedAtDepth(
                        candidate, position, depth)
                        ~> HistoricalTemporalStage6CompletionIoDrainGoal(
                             candidate, position)
          BY HistoricalTemporalFairStage6CompletionIoDrains
        <4> QED BY <4>1, <4>2, PTL
      <3>2. HistoricalTemporalStage6CompletionIoDrainGoal(
               candidate, position)
               => \/ HistoricalTemporalStage6CompletionGoal(
                        candidate, position)
                  \/ HistoricalTemporalStage6CompletionReadyBlocked(
                       candidate, position)
        BY Isa
           DEF HistoricalTemporalStage6CompletionIoDrainGoal,
               HistoricalTemporalStage6CompletionReadyBlocked
      <3>3. /\ HistoricalTemporalStage6CompletionCapacityBlocked(
                    candidate, position)
              /\ AsyncIoQueueDepth(candidate.node) = 0
             => HistoricalTemporalStage6CompletionReadyBlocked(
                  candidate, position)
        BY DEF HistoricalTemporalStage6CompletionReadyBlocked
      <3> QED BY <3>1, <3>2, <3>3, PTL
    <2>3. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage6CompletionReadyBlocked(
                   candidate, position)
                   ~> HistoricalTemporalStage6CompletionGoal(
                        candidate, position))
      BY HistoricalTemporalFairStage6CompletionReadyOpens
    <2> QED BY <2>2, <2>3, PTL
  <1> QED BY <1>1

HistoricalTemporalStage6FiniteRunnerEpisodeClosureProperty(specification) ==
  /\ HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
       specification)
  /\ HistoricalTemporalStage6PreAdmissionFiniteRunnerEpisodeResidualProperty(
       specification)
  /\ HistoricalTemporalStage6OwedFiniteRunnerEpisodeResidualProperty(
       specification)
  /\ HistoricalTemporalStage6NonCompletionFiniteServeEpisodeResidualProperty(
       specification)

THEOREM AsyncSpecClosesHistoricalTemporalStage6Leaf ==
  \A initialContext:
    HistoricalTemporalStage6FiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => HistoricalProtectedStage6RankProgressProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                HistoricalTemporalStage6FiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
                NEW candidate \in AsyncCandidateSet,
                NEW position \in Nat
         PROVE AsyncSpecAt(initialContext)
                 => ((gst
                       /\ HistoricalProtectedCandidateOwned(candidate)
                       /\ CandidateServiceRank(candidate)
                            = <<6, position>>)
                      ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                           \/ \E lower \in SetLessThan(
                                <<6, position>>,
                                OwnedServiceRankOrdering,
                                OwnedServiceRankCarrier):
                                HistoricalProtectedOwnedAtServiceRank(
                                  candidate, lower)))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>2. AsyncSpecAt(initialContext)
             => ((gst
                   /\ HistoricalProtectedCandidateOwned(candidate)
                   /\ CandidateServiceRank(candidate) = <<6, position>>)
                  ~> HistoricalTemporalStage6Pending(
                       candidate, position))
      BY <2>1, PTL
         DEF HistoricalTemporalStage6Pending,
             HistoricalProtectedOwnedAtServiceRank
    <2>3. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage6Pending(candidate, position)
                   ~> HistoricalTemporalStage6PreAdmissionGoal(
                        candidate, position))
      BY HistoricalTemporalFairStage6PreAdmissionProgress
         DEF HistoricalTemporalStage6FiniteRunnerEpisodeClosureProperty
    <2>4. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage6OwedCausalReady(
                   candidate, position)
                   ~> HistoricalTemporalRankProgressExit(
                        candidate, <<6, position>>))
      BY HistoricalTemporalFairStage6OwedCausalAdmission
         DEF HistoricalTemporalStage6FiniteRunnerEpisodeClosureProperty
    <2>5. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage6NonCompletionCapacityBlocked(
                   candidate, position)
                   ~> HistoricalTemporalRankProgressExit(
                        candidate, <<6, position>>))
      <3>1. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage6NonCompletionCapacityBlocked(
                     candidate, position)
                     ~> HistoricalTemporalStage6NonCompletionGoal(
                          candidate, position))
        BY HistoricalTemporalFairStage6NonCompletionOpens
           DEF HistoricalTemporalStage6FiniteRunnerEpisodeClosureProperty
      <3> QED BY <3>1, <2>4, PTL
           DEF HistoricalTemporalStage6NonCompletionGoal
    <2>6. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage6CompletionCapacityBlocked(
                   candidate, position)
                   ~> HistoricalTemporalRankProgressExit(
                        candidate, <<6, position>>))
      <3>1. AsyncSpecAt(initialContext)
               => (HistoricalTemporalStage6CompletionCapacityBlocked(
                     candidate, position)
                     ~> HistoricalTemporalStage6CompletionGoal(
                          candidate, position))
        BY HistoricalTemporalFairStage6CompletionCapacityOpens
           DEF HistoricalTemporalStage6FiniteRunnerEpisodeClosureProperty
      <3> QED BY <3>1, <2>4, PTL
           DEF HistoricalTemporalStage6CompletionGoal
    <2>7. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage6PreAdmissionGoal(
                   candidate, position)
                   ~> HistoricalTemporalRankProgressExit(
                        candidate, <<6, position>>))
      BY <2>4, <2>5, <2>6, PTL
         DEF HistoricalTemporalStage6PreAdmissionGoal
    <2>8. AsyncSpecAt(initialContext)
             => (HistoricalTemporalRankProgressExit(
                   candidate, <<6, position>>)
                   ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                        \/ \E lower \in SetLessThan(
                             <<6, position>>,
                             OwnedServiceRankOrdering,
                             OwnedServiceRankCarrier):
                             HistoricalProtectedOwnedAtServiceRank(
                               candidate, lower)))
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType,
         HistoricalTemporalRankExitHasWellFoundedSuccessor, PTL
         DEF HistoricalTemporalRankProgressExit
    <2> QED BY <2>2, <2>3, <2>7, <2>8, PTL
  <1> QED BY <1>1
       DEF HistoricalProtectedStage6RankProgressProperty,
           HistoricalProtectedStageRankProgressProperty

HistoricalTemporalFiniteRunnerEpisodeClosureProperty(specification) ==
  /\ HistoricalTemporalStage3FiniteServeEpisodeResidualProperty(
       specification)
  /\ HistoricalTemporalStage4FiniteServeEpisodeResidualProperty(
       specification)
  /\ HistoricalTemporalStage6FiniteRunnerEpisodeClosureProperty(
       specification)

(***************************************************************************
Stage 2 post-deferred witnesses.

A Busy completion owned by the same historical target is served only after
it has left deferred Stage 2.  Its carrier is therefore exactly stages 3..6.
The four historical leaves above close that restricted rank without changing
the owner scope back to `AsyncCurrentResponsiveVoters`.
***************************************************************************)

HistoricalTemporalPostDeferredRankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet,
          stage \in 3..6, position \in Nat:
         (gst
           /\ HistoricalProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<stage, position>>)
           ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                \/ ServiceRankLess(
                     CandidateServiceRank(candidate),
                     <<stage, position>>))

HistoricalTemporalPostDeferredExit(candidate) ==
  \/ HistoricalProtectedServiceOwnershipExit(candidate)
  \/ CandidateServiceRank(candidate)[1] \notin 3..6

HistoricalTemporalPostDeferredAtRank(candidate, rank) ==
  /\ gst
  /\ HistoricalProtectedCandidateOwned(candidate)
  /\ CandidateServiceRank(candidate) = rank

THEOREM HistoricalTemporalStageLeafGoalImpliesStrictRank ==
  \A candidate, stage, position:
    /\ stage \in 3..6
    /\ position \in Nat
    /\ (HistoricalProtectedServiceOwnershipExit(candidate)
         \/ \E lower \in SetLessThan(
              <<stage, position>>,
              OwnedServiceRankOrdering,
              OwnedServiceRankCarrier):
              HistoricalProtectedOwnedAtServiceRank(
                candidate, lower))
    => \/ HistoricalProtectedServiceOwnershipExit(candidate)
       \/ ServiceRankLess(
            CandidateServiceRank(candidate), <<stage, position>>)
BY OwnedServiceRankOrderingMatchesLess, Isa
   DEF HistoricalProtectedOwnedAtServiceRank,
       OwnedServiceRankCarrier, SetLessThan

THEOREM HistoricalTemporalPostDeferredRanksComposeFromLeaves ==
  \A initialContext:
    /\ HistoricalProtectedStage3RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalProtectedStage4RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalProtectedStage5RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalProtectedStage6RankProgressProperty(
         AsyncSpecAt(initialContext))
    => HistoricalTemporalPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                HistoricalProtectedStage3RankProgressProperty(
                  AsyncSpecAt(initialContext)),
                HistoricalProtectedStage4RankProgressProperty(
                  AsyncSpecAt(initialContext)),
                HistoricalProtectedStage5RankProgressProperty(
                  AsyncSpecAt(initialContext)),
                HistoricalProtectedStage6RankProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE HistoricalTemporalPostDeferredRankProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME NEW candidate \in AsyncCandidateSet,
                  NEW stage \in 3..6, NEW position \in Nat,
                  AsyncSpecAt(initialContext)
           PROVE (gst
                    /\ HistoricalProtectedCandidateOwned(candidate)
                    /\ CandidateServiceRank(candidate)
                         = <<stage, position>>)
                   ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                        \/ ServiceRankLess(
                             CandidateServiceRank(candidate),
                             <<stage, position>>))
      <3>1. CASE stage = 3
        <4>1. (gst
                 /\ HistoricalProtectedCandidateOwned(candidate)
                 /\ CandidateServiceRank(candidate)
                      = <<stage, position>>)
                ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                     \/ \E lower \in SetLessThan(
                          <<stage, position>>,
                          OwnedServiceRankOrdering,
                          OwnedServiceRankCarrier):
                          HistoricalProtectedOwnedAtServiceRank(
                            candidate, lower))
          BY <1>1, <2>1, <3>1
             DEF HistoricalProtectedStage3RankProgressProperty,
                 HistoricalProtectedStageRankProgressProperty
        <4> QED BY <4>1,
             HistoricalTemporalStageLeafGoalImpliesStrictRank, PTL
      <3>2. CASE stage = 4
        <4>1. (gst
                 /\ HistoricalProtectedCandidateOwned(candidate)
                 /\ CandidateServiceRank(candidate)
                      = <<stage, position>>)
                ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                     \/ \E lower \in SetLessThan(
                          <<stage, position>>,
                          OwnedServiceRankOrdering,
                          OwnedServiceRankCarrier):
                          HistoricalProtectedOwnedAtServiceRank(
                            candidate, lower))
          BY <1>1, <2>1, <3>2
             DEF HistoricalProtectedStage4RankProgressProperty,
                 HistoricalProtectedStageRankProgressProperty
        <4> QED BY <4>1,
             HistoricalTemporalStageLeafGoalImpliesStrictRank, PTL
      <3>3. CASE stage = 5
        <4>1. (gst
                 /\ HistoricalProtectedCandidateOwned(candidate)
                 /\ CandidateServiceRank(candidate)
                      = <<stage, position>>)
                ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                     \/ \E lower \in SetLessThan(
                          <<stage, position>>,
                          OwnedServiceRankOrdering,
                          OwnedServiceRankCarrier):
                          HistoricalProtectedOwnedAtServiceRank(
                            candidate, lower))
          BY <1>1, <2>1, <3>3
             DEF HistoricalProtectedStage5RankProgressProperty,
                 HistoricalProtectedStageRankProgressProperty
        <4> QED BY <4>1,
             HistoricalTemporalStageLeafGoalImpliesStrictRank, PTL
      <3>4. CASE stage = 6
        <4>1. (gst
                 /\ HistoricalProtectedCandidateOwned(candidate)
                 /\ CandidateServiceRank(candidate)
                      = <<stage, position>>)
                ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                     \/ \E lower \in SetLessThan(
                          <<stage, position>>,
                          OwnedServiceRankOrdering,
                          OwnedServiceRankCarrier):
                          HistoricalProtectedOwnedAtServiceRank(
                            candidate, lower))
          BY <1>1, <2>1, <3>4
             DEF HistoricalProtectedStage6RankProgressProperty,
                 HistoricalProtectedStageRankProgressProperty
        <4> QED BY <4>1,
             HistoricalTemporalStageLeafGoalImpliesStrictRank, PTL
      <3> QED BY <3>1, <3>2, <3>3, <3>4, Isa
    <2> QED BY <2>1
         DEF HistoricalTemporalPostDeferredRankProgressProperty
  <1> QED BY <1>1

THEOREM AsyncSpecClosesHistoricalTemporalPostDeferredRanks ==
  \A initialContext:
    HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => HistoricalTemporalPostDeferredRankProgressProperty(
           AsyncSpecAt(initialContext))
BY AsyncSpecClosesHistoricalTemporalStage3Leaf,
   AsyncSpecClosesHistoricalTemporalStage4Leaf,
   AsyncSpecClosesHistoricalTemporalStage5Leaf,
   AsyncSpecClosesHistoricalTemporalStage6Leaf,
   HistoricalTemporalPostDeferredRanksComposeFromLeaves
   DEF HistoricalTemporalFiniteRunnerEpisodeClosureProperty,
       HistoricalTemporalStage6FiniteRunnerEpisodeClosureProperty

THEOREM HistoricalTemporalPostDeferredRankConverges ==
  \A initialContext, candidate:
    HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => ((gst
                  /\ HistoricalProtectedCandidateOwned(candidate)
                  /\ CandidateServiceRank(candidate)[1] \in 3..6)
                 ~> HistoricalTemporalPostDeferredExit(candidate)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
                AsyncSpecAt(initialContext)
         PROVE (gst
                  /\ HistoricalProtectedCandidateOwned(candidate)
                  /\ CandidateServiceRank(candidate)[1] \in 3..6)
                 ~> HistoricalTemporalPostDeferredExit(candidate)
    <2>1. HistoricalTemporalPostDeferredRankProgressProperty(
             AsyncSpecAt(initialContext))
      BY AsyncSpecClosesHistoricalTemporalPostDeferredRanks
    <2>2. ASSUME NEW rank \in PostDeferredServiceRankCarrier
           PROVE HistoricalTemporalPostDeferredAtRank(candidate, rank)
                   ~> (HistoricalTemporalPostDeferredExit(candidate)
                        \/ \E lower \in SetLessThan(
                             rank, PostDeferredServiceRankOrdering,
                             PostDeferredServiceRankCarrier):
                             HistoricalTemporalPostDeferredAtRank(
                               candidate, lower))
      <3>1. PICK stage \in 3..6, position \in Nat:
               rank = <<stage, position>>
        BY <2>2 DEF PostDeferredServiceRankCarrier
      <3>2. HistoricalTemporalPostDeferredAtRank(candidate, rank)
               ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                    \/ ServiceRankLess(
                         CandidateServiceRank(candidate), rank))
        BY <1>1, <2>1, <3>1
           DEF HistoricalTemporalPostDeferredRankProgressProperty,
               HistoricalTemporalPostDeferredAtRank
      <3>3. []AsyncTypeInvariant
        BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
           AsyncStrongTypeProjectsAsyncType, PTL
      <3>4. /\ AsyncTypeInvariant
             /\ gst
             /\ HistoricalProtectedCandidateOwned(candidate)
             /\ ServiceRankLess(
                  CandidateServiceRank(candidate), rank)
            => \/ HistoricalTemporalPostDeferredExit(candidate)
               \/ \E lower \in SetLessThan(
                    rank, PostDeferredServiceRankOrdering,
                    PostDeferredServiceRankCarrier):
                    HistoricalTemporalPostDeferredAtRank(candidate, lower)
        BY <2>2, ScheduledCandidateServiceRankInCarrier,
           OwnedServiceRankOrderingMatchesLess, Isa
           DEF HistoricalTemporalPostDeferredExit,
               HistoricalTemporalPostDeferredAtRank,
               HistoricalProtectedCandidateOwned,
               ProtectedCandidateOwned, CandidateScheduled,
               CandidateServiceRank, ServiceRankLess,
               PostDeferredServiceRankOrdering,
               PostDeferredServiceRankCarrier, SetLessThan
      <3> QED BY <3>2, <3>3, <3>4, PTL
           DEF HistoricalTemporalPostDeferredExit
    <2>3. \A rank \in PostDeferredServiceRankCarrier:
             HistoricalTemporalPostDeferredAtRank(candidate, rank)
               ~> HistoricalTemporalPostDeferredExit(candidate)
      BY <2>2, PostDeferredServiceRankOrderingWellFoundedObligation,
         WellFoundedLeadsTo
    <2>4. (gst
             /\ HistoricalProtectedCandidateOwned(candidate)
             /\ CandidateServiceRank(candidate)[1] \in 3..6)
             ~> \E rank \in PostDeferredServiceRankCarrier:
                  HistoricalTemporalPostDeferredAtRank(candidate, rank)
      BY Isa, PTL
         DEF HistoricalTemporalPostDeferredAtRank,
             PostDeferredServiceRankCarrier
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

HistoricalTemporalStage2Owned(candidate) ==
  /\ gst
  /\ HistoricalProtectedCandidateOwned(candidate)
  /\ candidate \in DeferredCandidates

HistoricalTemporalBusyCompletionWitness(target, witness) ==
  /\ HistoricalTemporalStage2Owned(target)
  /\ BusyPhaseRank(target.node) \in 1..2
  /\ witness \in BusyCompletionCandidates(target.node)

THEOREM HistoricalTemporalBusyWitnessHasPostDeferredRank ==
  \A target, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ HistoricalTemporalBusyCompletionWitness(target, witness)
    => /\ HistoricalProtectedCandidateOwned(witness)
       /\ CandidateServiceRank(witness)
            \in PostDeferredServiceRankCarrier
BY AsyncStrongTypeProjectsAsyncType,
   ScheduledCandidateServiceRankInCarrier, Isa
   DEF HistoricalTemporalBusyCompletionWitness,
       HistoricalTemporalStage2Owned,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateScheduled, BusyCompletionCandidates,
       ActiveBusyCompletionCarrier,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, CandidateServiceRank,
       PostDeferredServiceRankCarrier,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant

THEOREM HistoricalTemporalBusyWitnessOwnershipPersists ==
  \A target, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ HistoricalTemporalBusyCompletionWitness(target, witness)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~HistoricalProtectedServiceOwnershipExit(target)'
    /\ BusyPhaseRank(target.node)'
         >= BusyPhaseRank(target.node)
    => HistoricalTemporalBusyCompletionWitness(target, witness)'
BY BusyPhaseOwnerPartitionObligation,
   BusyCompletionExecutionDropsPhaseObligation,
   BusyCompletionCandidateDispatchableObligation,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   RuntimeSelectedCommandsAreTyped,
   ProgressCoreStutterAndCarrierGrowthRetainsBusyCandidates,
   ProgressCoreStutterKeepsBusyWitnessWhenCarried,
   HeadTailProperties, IsaT(300)
   DEF Stage2BusyKernelInvariant,
       HistoricalTemporalBusyCompletionWitness,
       HistoricalTemporalStage2Owned,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateScheduled, CandidateServiceRank,
       BusyPhaseRank, Stage2TwoStepBusyNodes,
       Stage2OneStepBusyNodes, Stage2TwoStepBusyOwners,
       Stage2OneStepBusyOwners, BusyCompletionCandidates,
       ActiveBusyCompletionCarrier, SerializedBusyOwners,
       SerializedBusyOwnershipInvariant, RequestsUniqueByNode,
       RequestNodeSet, NodeIdle, PendingNodes, SigningNodes,
       AllPendingRequests, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       CandidateConsumerCurrent, CommandDispatchable,
       CommandExecutionReady, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       LocalAdmissionStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AdmitProducerCompletion,
       AdmitCausalHead, IngressDrainStep, SerializedRuntimeStep,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep, DirectRetransmitStep,
       IdleRuntimeStep, RemoveNextNodeCommand,
       RemoveNextDeferredCommand, DeferCommand, DiscardCommand,
       ExecuteCommand, ExecuteRegularCommand,
       ExecuteDecisionFetch, ExecuteSignProposal, ExecuteSignVote,
       ExecuteFormPrepareQC, ExecuteSignTimeout,
       ExecutePersistInstall, ExecutePersistDecision,
       ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk, ServiceIoWorker,
       ServiceHistoricalRecoveryIoWorker, ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, OpenHistoricalRecovery,
       CommitCertificateDiscoveryStepWork, AsyncNetworkStep,
       AdmitIngressPacket, AsyncFaultStep, PreGstCrash,
       PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       AsyncTick, AsyncSetGST, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalBusyWitnessPersistsAtPostDeferredStage ==
  \A target, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ HistoricalTemporalBusyCompletionWitness(target, witness)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~HistoricalProtectedServiceOwnershipExit(target)'
    /\ BusyPhaseRank(target.node)'
         >= BusyPhaseRank(target.node)
    => /\ HistoricalProtectedCandidateOwned(witness)'
       /\ CandidateServiceRank(witness)'[1] \in 3..6
PROOF
  <1>1. ASSUME NEW target, NEW witness,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                Stage2BusyKernelInvariant,
                HistoricalTemporalBusyCompletionWitness(target, witness),
                [AsyncNext]_AsyncAllVars,
                ~HistoricalProtectedServiceOwnershipExit(target)',
                BusyPhaseRank(target.node)'
                  >= BusyPhaseRank(target.node)
         PROVE /\ HistoricalProtectedCandidateOwned(witness)'
               /\ CandidateServiceRank(witness)'[1] \in 3..6
    <2>1. /\ AsyncStrongTypeInvariant'
           /\ AsyncProgressOwnershipInvariant'
           /\ HistoricalTemporalBusyCompletionWitness(target, witness)'
      BY <1>1, AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership,
         HistoricalTemporalBusyWitnessOwnershipPersists
    <2>2. /\ HistoricalProtectedCandidateOwned(witness)'
           /\ CandidateServiceRank(witness)'
                \in PostDeferredServiceRankCarrier
      BY <2>1, HistoricalTemporalBusyWitnessHasPostDeferredRank
    <2> QED BY <2>2 DEF PostDeferredServiceRankCarrier
  <1> QED BY <1>1

THEOREM HistoricalTemporalBusyPhaseCannotIncrease ==
  \A target:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ HistoricalTemporalStage2Owned(target)
    /\ BusyPhaseRank(target.node) \in 1..2
    /\ [AsyncNext]_AsyncAllVars
    /\ ~HistoricalProtectedServiceOwnershipExit(target)'
    => BusyPhaseRank(target.node)' <= BusyPhaseRank(target.node)
BY BusyPhaseOwnerPartitionObligation,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   RuntimeSelectedCommandsAreTyped, IsaT(240)
   DEF Stage2BusyKernelInvariant,
       HistoricalTemporalStage2Owned,
       HistoricalProtectedServiceOwnershipExit,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, BusyPhaseRank,
       Stage2TwoStepBusyNodes, Stage2OneStepBusyNodes,
       Stage2TwoStepBusyOwners, Stage2OneStepBusyOwners,
       SerializedBusyOwners, SerializedBusyOwnershipInvariant,
       RequestsUniqueByNode, RequestNodeSet, NodeIdle,
       PendingNodes, SigningNodes, AllPendingRequests,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DeferredTagStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep, ExecuteCommand,
       ExecuteRegularCommand, ExecuteSignProposal, ExecuteSignVote,
       ExecuteFormPrepareQC, ExecuteSignTimeout,
       ExecutePersistInstall, ExecutePersistDecision,
       ExecuteRequestCertifiedBody, ExecuteApply,
       ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk, AsyncNext,
       AsyncNonCrashStep, AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncAllVars

HistoricalTemporalStage2BusyPhaseGoal(target, phase) ==
  \/ HistoricalProtectedServiceOwnershipExit(target)
  \/ BusyPhaseRank(target.node) < phase

HistoricalTemporalStage2BusyWitnessBlocked(target, witness, phase) ==
  /\ HistoricalTemporalBusyCompletionWitness(target, witness)
  /\ BusyPhaseRank(target.node) = phase

THEOREM HistoricalTemporalStage2BusyPhaseDescent ==
  \A initialContext, target:
    \A phase \in 1..2:
      HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
        AsyncSpecAt(initialContext))
        => (AsyncSpecAt(initialContext)
              => ((HistoricalTemporalStage2Owned(target)
                    /\ BusyPhaseRank(target.node) = phase)
                   ~> HistoricalTemporalStage2BusyPhaseGoal(
                        target, phase)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW target,
                NEW phase \in 1..2,
                HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
                AsyncSpecAt(initialContext)
         PROVE (HistoricalTemporalStage2Owned(target)
                  /\ BusyPhaseRank(target.node) = phase)
                 ~> HistoricalTemporalStage2BusyPhaseGoal(
                      target, phase)
    <2>1. [](AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ Stage2BusyKernelInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysStage2BusyKernelObligation, PTL
         DEF Stage2BusyKernelProperty
    <2>2. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ HistoricalTemporalStage2Owned(target)
           /\ BusyPhaseRank(target.node) = phase
          => \E witness \in AsyncCandidateSet:
               HistoricalTemporalStage2BusyWitnessBlocked(
                 target, witness, phase)
      BY <1>1, <2>1, BusyPhaseOwnerPartitionObligation, Isa
         DEF Stage2BusyKernelInvariant,
             HistoricalTemporalStage2BusyWitnessBlocked,
             HistoricalTemporalBusyCompletionWitness,
             AsyncProgressOwnershipInvariant,
             BusyCompletionWitnessInvariant
    <2>3. ASSUME NEW witness \in AsyncCandidateSet
           PROVE HistoricalTemporalStage2BusyWitnessBlocked(
                   target, witness, phase)
                   ~> HistoricalTemporalStage2BusyPhaseGoal(
                        target, phase)
      <3>1. HistoricalTemporalStage2BusyWitnessBlocked(
               target, witness, phase)
               => /\ HistoricalProtectedCandidateOwned(witness)
                  /\ CandidateServiceRank(witness)[1] \in 3..6
        BY <2>1, HistoricalTemporalBusyWitnessHasPostDeferredRank
           DEF HistoricalTemporalStage2BusyWitnessBlocked
      <3>2. (gst
               /\ HistoricalProtectedCandidateOwned(witness)
               /\ CandidateServiceRank(witness)[1] \in 3..6)
              ~> HistoricalTemporalPostDeferredExit(witness)
        BY <1>1, HistoricalTemporalPostDeferredRankConverges
      <3>3. HistoricalTemporalStage2BusyWitnessBlocked(
               target, witness, phase)
               ~> HistoricalTemporalPostDeferredExit(witness)
        BY <3>1, <3>2, PTL
           DEF HistoricalTemporalStage2BusyWitnessBlocked,
               HistoricalTemporalBusyCompletionWitness,
               HistoricalTemporalStage2Owned
      <3>4. /\ AsyncStrongTypeInvariant
             /\ AsyncProgressOwnershipInvariant
             /\ Stage2BusyKernelInvariant
             /\ HistoricalTemporalStage2BusyWitnessBlocked(
                  target, witness, phase)
             /\ [AsyncNext]_AsyncAllVars
            => \/ HistoricalTemporalStage2BusyPhaseGoal(
                    target, phase)'
               \/ HistoricalTemporalStage2BusyWitnessBlocked(
                    target, witness, phase)'
        <4>1. ASSUME AsyncStrongTypeInvariant,
                      AsyncProgressOwnershipInvariant,
                      Stage2BusyKernelInvariant,
                      HistoricalTemporalStage2BusyWitnessBlocked(
                        target, witness, phase),
                      [AsyncNext]_AsyncAllVars
               PROVE \/ HistoricalTemporalStage2BusyPhaseGoal(
                           target, phase)'
                     \/ HistoricalTemporalStage2BusyWitnessBlocked(
                          target, witness, phase)'
          <5>1. CASE HistoricalTemporalStage2BusyPhaseGoal(
                        target, phase)'
            BY <5>1
          <5>2. CASE ~HistoricalTemporalStage2BusyPhaseGoal(
                        target, phase)'
            <6>1. /\ ~HistoricalProtectedServiceOwnershipExit(target)'
                   /\ BusyPhaseRank(target.node)' >= phase
              BY <5>2
                 DEF HistoricalTemporalStage2BusyPhaseGoal
            <6>2. BusyPhaseRank(target.node)'
                     <= BusyPhaseRank(target.node)
              BY <4>1, <6>1,
                 HistoricalTemporalBusyPhaseCannotIncrease
                 DEF HistoricalTemporalStage2BusyWitnessBlocked,
                     HistoricalTemporalBusyCompletionWitness
            <6>3. BusyPhaseRank(target.node)' = phase
              BY <4>1, <6>1, <6>2
                 DEF HistoricalTemporalStage2BusyWitnessBlocked
            <6>4. /\ HistoricalProtectedCandidateOwned(witness)'
                   /\ CandidateServiceRank(witness)'[1] \in 3..6
              BY <4>1, <6>1,
                 HistoricalTemporalBusyWitnessPersistsAtPostDeferredStage
                 DEF HistoricalTemporalStage2BusyWitnessBlocked
            <6>5. HistoricalTemporalBusyCompletionWitness(
                     target, witness)'
              BY <4>1, <6>1,
                 HistoricalTemporalBusyWitnessOwnershipPersists
                 DEF HistoricalTemporalStage2BusyWitnessBlocked
            <6> QED BY <6>3, <6>4, <6>5
                 DEF HistoricalTemporalStage2BusyWitnessBlocked
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3>5. [](HistoricalTemporalStage2BusyWitnessBlocked(
                   target, witness, phase)
                 /\ HistoricalTemporalPostDeferredExit(witness)
                => FALSE)
        BY <2>1, HistoricalTemporalBusyWitnessHasPostDeferredRank,
           PTL
           DEF HistoricalTemporalStage2BusyWitnessBlocked,
               HistoricalTemporalPostDeferredExit,
               PostDeferredServiceRankCarrier
      <3> QED BY <2>1, <3>3, <3>4, <3>5, PTL
    <2>4. (HistoricalTemporalStage2Owned(target)
             /\ BusyPhaseRank(target.node) = phase)
            ~> \E witness \in AsyncCandidateSet:
                 HistoricalTemporalStage2BusyWitnessBlocked(
                   target, witness, phase)
      BY <2>1, <2>2, PTL
    <2> QED BY <2>3, <2>4, PTL
         DEF HistoricalTemporalStage2BusyPhaseGoal
  <1> QED BY <1>1

HistoricalTemporalStage2BusyAtPhase(target, phase) ==
  /\ HistoricalTemporalStage2Owned(target)
  /\ BusyPhaseRank(target.node) = phase

HistoricalTemporalStage2BusyTerminationGoal(target) ==
  \/ HistoricalProtectedServiceOwnershipExit(target)
  \/ NodeIdle(target.node)

THEOREM HistoricalTemporalStage2BusyTerminates ==
  \A initialContext, target:
    HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => ((HistoricalTemporalStage2Owned(target)
                  /\ ~NodeIdle(target.node))
                 ~> HistoricalTemporalStage2BusyTerminationGoal(target)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW target,
                HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
                AsyncSpecAt(initialContext)
         PROVE (HistoricalTemporalStage2Owned(target)
                  /\ ~NodeIdle(target.node))
                 ~> HistoricalTemporalStage2BusyTerminationGoal(target)
    <2>1. [](AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ Stage2BusyKernelInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysStage2BusyKernelObligation, PTL
         DEF Stage2BusyKernelProperty
    <2>2. HistoricalTemporalStage2BusyAtPhase(target, 1)
             ~> HistoricalTemporalStage2BusyTerminationGoal(target)
      <3>1. HistoricalTemporalStage2BusyAtPhase(target, 1)
               ~> HistoricalTemporalStage2BusyPhaseGoal(target, 1)
        BY <1>1, HistoricalTemporalStage2BusyPhaseDescent
           DEF HistoricalTemporalStage2BusyAtPhase
      <3>2. [](HistoricalTemporalStage2BusyPhaseGoal(target, 1)
                 => HistoricalTemporalStage2BusyTerminationGoal(target))
        BY <2>1, BusyPhaseOwnerPartitionObligation, Isa, PTL
           DEF HistoricalTemporalStage2BusyPhaseGoal,
               HistoricalTemporalStage2BusyTerminationGoal,
               Stage2BusyKernelInvariant, BusyPhaseCarrier
      <3> QED BY <3>1, <3>2, PTL
    <2>3. HistoricalTemporalStage2BusyAtPhase(target, 2)
             ~> HistoricalTemporalStage2BusyTerminationGoal(target)
      <3>1. HistoricalTemporalStage2BusyAtPhase(target, 2)
               ~> HistoricalTemporalStage2BusyPhaseGoal(target, 2)
        BY <1>1, HistoricalTemporalStage2BusyPhaseDescent
           DEF HistoricalTemporalStage2BusyAtPhase
      <3>2. [](HistoricalTemporalStage2BusyPhaseGoal(target, 2)
                 => \/ HistoricalTemporalStage2BusyTerminationGoal(target)
                    \/ HistoricalTemporalStage2BusyAtPhase(target, 1))
        BY <2>1, BusyPhaseOwnerPartitionObligation, Isa, PTL
           DEF HistoricalTemporalStage2BusyPhaseGoal,
               HistoricalTemporalStage2BusyAtPhase,
               HistoricalTemporalStage2BusyTerminationGoal,
               Stage2BusyKernelInvariant, BusyPhaseCarrier,
               HistoricalTemporalStage2Owned,
               HistoricalProtectedServiceOwnershipExit
      <3> QED BY <2>2, <3>1, <3>2, PTL
    <2>4. [](HistoricalTemporalStage2Owned(target)
               /\ ~NodeIdle(target.node)
              => \/ HistoricalTemporalStage2BusyAtPhase(target, 1)
                 \/ HistoricalTemporalStage2BusyAtPhase(target, 2))
      BY <2>1, BusyPhaseOwnerPartitionObligation, Isa, PTL
         DEF HistoricalTemporalStage2BusyAtPhase,
             Stage2BusyKernelInvariant,
             HistoricalTemporalStage2Owned
    <2> QED BY <2>2, <2>3, <2>4, PTL
         DEF HistoricalTemporalStage2BusyTerminationGoal
  <1> QED BY <1>1

HistoricalTemporalStage2BusyRejectedSelected(candidate) ==
  /\ HistoricalTemporalStage2Owned(candidate)
  /\ ~NodeIdle(candidate.node)
  /\ asyncDeferredDrainOwed[candidate.node]
  /\ DeferredQueueNonempty(candidate.node)
  /\ NextDeferredCommand(candidate.node) = candidate
  /\ ~CommandDispatchable(candidate)

HistoricalTemporalStage2BusyRetryClaimsHandoff(candidate) ==
  /\ HistoricalTemporalStage2BusyRejectedSelected(candidate)
  /\ ~DeferredHandoffActive(candidate.node)
  /\ DeferredDrainStep(candidate.node)
  /\ asyncDeferredHandoffs'[candidate.node]
       = Stage2ActiveDeferredHandoff(candidate)
  /\ \A other \in ValidatorIds \ {candidate.node}:
       asyncDeferredHandoffs'[other] = asyncDeferredHandoffs[other]

HistoricalTemporalStage2ExactIdleRetryPending(candidate) ==
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ HistoricalTemporalStage2Owned(candidate)
  /\ NodeIdle(candidate.node)
  /\ DeferredHandoffQueueHead(candidate.node)

HistoricalTemporalStage2ExactIdleRetrySelected(candidate) ==
  /\ HistoricalTemporalStage2ExactIdleRetryPending(candidate)
  /\ asyncDeferredDrainOwed[candidate.node]
  /\ DeferredQueueNonempty(candidate.node)
  /\ Stage2DeferredHandoffToken(NextDeferredCommand(candidate.node))
       = Stage2DeferredHandoffToken(candidate)

HistoricalTemporalStage2HandoffProgressExit(candidate) ==
  \/ ~Stage2DeferredHandoffOwned(candidate)
  \/ HistoricalProtectedServiceOwnershipExit(candidate)

HistoricalTemporalStage2IdleHandoffAwaitingRearm(candidate) ==
  /\ HistoricalTemporalStage2ExactIdleRetryPending(candidate)
  /\ ~asyncDeferredDrainOwed[candidate.node]

HistoricalTemporalStage2IdleHandoffAtDistance(candidate, distance) ==
  /\ HistoricalTemporalStage2ExactIdleRetryPending(candidate)
  /\ Stage2HandoffCursorDistance(candidate) = distance

HistoricalTemporalStage2IdleHandoffCursorProgress(
    candidate, distance) ==
  \/ HistoricalTemporalStage2HandoffProgressExit(candidate)
  \/ HistoricalTemporalStage2ExactIdleRetrySelected(candidate)
  \/ \E lower \in 0..2:
       /\ lower < distance
       /\ HistoricalTemporalStage2IdleHandoffAtDistance(
            candidate, lower)

THEOREM HistoricalTemporalStage2BusyRetryClaimsHandoffAction ==
  \A candidate \in AsyncCandidateSet:
    /\ HistoricalTemporalStage2BusyRejectedSelected(candidate)
    /\ ~DeferredHandoffActive(candidate.node)
    /\ DeferredDrainStep(candidate.node)
    => HistoricalTemporalStage2BusyRetryClaimsHandoff(candidate)
BY IsaT(120)
   DEF HistoricalTemporalStage2BusyRejectedSelected,
       HistoricalTemporalStage2BusyRetryClaimsHandoff,
       Stage2ActiveDeferredHandoff,
       HistoricalTemporalStage2Owned, DeferredDrainStep,
       DeferredHandoffAllowsExecution,
       DeferredHandoffBlocksExecution,
       DeferredHandoffActive, DeferredHandoffMatches,
       InstallDeferredHandoff, RetainDeferredHandoffs,
       AsyncDeferredHandoff, NoAsyncDeferredHandoff

HistoricalTemporalStage2ForeignIdleSkip(candidate) ==
  /\ HistoricalTemporalStage2ExactIdleRetryPending(candidate)
  /\ asyncDeferredDrainOwed[candidate.node]
  /\ NextDeferredCommand(candidate.node) # candidate
  /\ DeferredHandoffBlocksExecution(
       candidate.node, NextDeferredCommand(candidate.node))
  /\ ~DeferredHandoffAllowsExecution(
       candidate.node, NextDeferredCommand(candidate.node))
  /\ DeferredDrainStep(candidate.node)

THEOREM HistoricalTemporalStage2ForeignIdleSkipDropsDistance ==
  \A candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ HistoricalTemporalStage2ForeignIdleSkip(candidate)
    => /\ Stage2DeferredHandoffOwned(candidate)'
       /\ NodeIdle(candidate.node)'
       /\ ~asyncDeferredDrainOwed'[candidate.node]
       /\ Stage2HandoffCursorDistance(candidate)'
            < Stage2HandoffCursorDistance(candidate)
BY AsyncStrongTypeProjectsAsyncType,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   Stage2DeferredHandoffTokenIsInjectiveObligation,
   Stage2SelectedDifferentDeferredClassDropsDistance,
   RuntimeSelectedCommandsAreTyped, IsaT(180)
   DEF HistoricalTemporalStage2ForeignIdleSkip,
       HistoricalTemporalStage2ExactIdleRetryPending,
       Stage2DeferredHandoffOwned, Stage2ActiveDeferredHandoff,
       Stage2DeferredHandoffToken, Stage2HandoffCursorDistance,
       HistoricalTemporalStage2Owned, DeferredDrainStep,
       DeferredHandoffAllowsExecution,
       DeferredHandoffBlocksExecution, DeferredHandoffActive,
       DeferredHandoffMatches, DeferredHandoffQueueHead,
       DeferredHandoffCandidate, RetainDeferredHandoffs,
       AdvanceNextDeferredClass, NextDeferredCommand,
       SelectedDeferredClass, DeferredClassQueue,
       DeferredClassNonempty, NodeIdle,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateScheduled,
       DeferredCandidates, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant, AsyncAllVars

THEOREM HistoricalTemporalStage2HandoffDistanceInCarrier ==
  \A candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncDeferredHandoffOwnershipInvariant
    /\ HistoricalTemporalStage2ExactIdleRetryPending(candidate)
    => Stage2HandoffCursorDistance(candidate) \in 0..2
BY AsyncStrongTypeProjectsAsyncType, SMTT(30)
   DEF Stage2HandoffCursorDistance,
       HistoricalTemporalStage2ExactIdleRetryPending,
       Stage2DeferredHandoffOwned,
       HistoricalTemporalStage2Owned,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncDeferredTypeInvariant,
       AsyncDeferredTopologyTypeInvariant, AsyncCandidateTyped,
       AsyncCandidateSet, AsyncCommandClasses,
       CommandClassDistance, NextCommandClass

THEOREM HistoricalTemporalStage2IdleHandoffDrainRearmed ==
  \A initialContext, candidate:
    AsyncSpecAt(initialContext)
      => (HistoricalTemporalStage2IdleHandoffAwaitingRearm(candidate)
           ~> (HistoricalTemporalStage2HandoffProgressExit(candidate)
                \/ (HistoricalTemporalStage2ExactIdleRetryPending(candidate)
                     /\ asyncDeferredDrainOwed[candidate.node])))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                AsyncSpecAt(initialContext)
         PROVE HistoricalTemporalStage2IdleHandoffAwaitingRearm(
                 candidate)
                 ~> (HistoricalTemporalStage2HandoffProgressExit(candidate)
                      \/ (HistoricalTemporalStage2ExactIdleRetryPending(
                            candidate)
                           /\ asyncDeferredDrainOwed[candidate.node]))
    <2> DEFINE Goal ==
           HistoricalTemporalStage2HandoffProgressExit(candidate)
             \/ (HistoricalTemporalStage2ExactIdleRetryPending(candidate)
                  /\ asyncDeferredDrainOwed[candidate.node])
    <2>1. [](AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ AsyncDeferredHandoffOwnershipInvariant
              /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
              /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
         AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity, PTL
    <2>2. (HistoricalTemporalStage2Owned(candidate)
             /\ ~NodeIdle(candidate.node))
             ~> HistoricalTemporalStage2BusyTerminationGoal(candidate)
      BY <1>1, HistoricalTemporalStage2BusyTerminates
         DEF HistoricalTemporalStage2BusyTerminationGoal
    <2>3. HistoricalTemporalStage2IdleHandoffAwaitingRearm(candidate)
             ~> (Goal
                  \/ (Stage2DeferredHandoffOwned(candidate)
                       /\ HistoricalTemporalStage2Owned(candidate)
                       /\ ~NodeIdle(candidate.node)
                       /\ asyncDeferredDrainOwed[candidate.node]))
      BY <1>1, <2>1,
         Stage2DeferredHandoffTokenIsInjectiveObligation,
         ReadyRunAuxOrderingIsWellFounded,
         HistoricalTemporalProtectedOwnerEnablesFairRunner,
         LocalAdmissionStrictlyDecreasesRuntimeReach,
         SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
         IngressDrainStrictlyDecreasesRuntimeReach,
         IsaT(300), PTL
         DEF Goal,
             HistoricalTemporalStage2IdleHandoffAwaitingRearm,
             HistoricalTemporalStage2HandoffProgressExit,
             HistoricalTemporalStage2ExactIdleRetryPending,
             Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             Stage2DeferredHandoffToken,
             HistoricalTemporalStage2Owned,
             HistoricalProtectedServiceOwnershipExit,
             HistoricalProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateScheduled,
             DeferredCandidates, CandidateServiceRank,
             ReadyRunAuxRank, ReadyRunAuxOrdering,
             ReadyRunAuxCarrier, ReadyRunDeferredRank,
             ReadyRunTimeoutRank, ReadyRunInnerRank,
             ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
             ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
             PostGstRunHistoricalRecoveryNode,
             RunHistoricalRecoveryNode, RunNodeWork,
             ResolveRunNodeCandidateProducerContinuation,
             AsyncSchedulerExceptCausalControlAndNodeService,
             SerializedLocalPrecedesServeIngressStep,
             SelectedLocalAdmissionAdvance,
             AsyncServeIngressTargetOnlyTurn,
             LocalAdmissionStep, IngressDrainStep,
             SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             RuntimeStep,
             DirectTimeoutStep, DirectRetransmitStep,
             DeferredTagStep, DeferredTimeoutStep,
             DeferredRetransmitStep, FifoRuntimeStep,
             IdleRuntimeStep, AsyncTick, AsyncTickEnabled,
             TimeoutDue, RetransmitDue,
             DeferredHandoffQueueHead, DeferredHandoffMatches,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution, DeferredDrainStep,
             AsyncSpecAt, AsyncFairnessAt, AsyncFairActionAt,
             AsyncNext, AsyncAllVars
    <2>4. (Stage2DeferredHandoffOwned(candidate)
             /\ HistoricalTemporalStage2Owned(candidate)
             /\ ~NodeIdle(candidate.node)
             /\ asyncDeferredDrainOwed[candidate.node])
             ~> Goal
      BY <2>1, <2>2, PTL
         DEF Goal, HistoricalTemporalStage2HandoffProgressExit,
             HistoricalTemporalStage2ExactIdleRetryPending,
             Stage2DeferredHandoffOwned,
             HistoricalTemporalStage2Owned,
             HistoricalProtectedServiceOwnershipExit,
             AsyncDeferredHandoffOwnershipInvariant,
             DeferredHandoffActive, DeferredHandoffCandidate,
             DeferredHandoffQueueHead
    <2> QED BY <2>3, <2>4, PTL DEF Goal
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage2IdleHandoffCursorOneStep ==
  \A initialContext, candidate:
    \A distance \in 0..2:
      AsyncSpecAt(initialContext)
        => (HistoricalTemporalStage2IdleHandoffAtDistance(
              candidate, distance)
             ~> HistoricalTemporalStage2IdleHandoffCursorProgress(
                  candidate, distance))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                NEW distance \in 0..2,
                AsyncSpecAt(initialContext)
         PROVE HistoricalTemporalStage2IdleHandoffAtDistance(
                 candidate, distance)
                 ~> HistoricalTemporalStage2IdleHandoffCursorProgress(
                      candidate, distance)
    <2>1. HistoricalTemporalStage2IdleHandoffAwaitingRearm(candidate)
               ~> (HistoricalTemporalStage2HandoffProgressExit(candidate)
                    \/ (HistoricalTemporalStage2ExactIdleRetryPending(
                          candidate)
                         /\ asyncDeferredDrainOwed[candidate.node]))
      BY <1>1, HistoricalTemporalStage2IdleHandoffDrainRearmed
    <2>2. [](AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ AsyncDeferredHandoffOwnershipInvariant
              /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
              /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
         AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity, PTL
    <2>3. (HistoricalTemporalStage2ExactIdleRetryPending(candidate)
              /\ asyncDeferredDrainOwed[candidate.node]
              /\ Stage2HandoffCursorDistance(candidate) = distance)
             ~> HistoricalTemporalStage2IdleHandoffCursorProgress(
                  candidate, distance)
      BY <1>1, <2>2,
         HistoricalTemporalStage2ForeignIdleSkipDropsDistance,
         Stage2DeferredHandoffTokenIsInjectiveObligation,
         Stage2SelectedDifferentDeferredClassDropsDistance,
         ReadyRunAuxOrderingIsWellFounded,
         HistoricalTemporalProtectedOwnerEnablesFairRunner,
         LocalAdmissionStrictlyDecreasesRuntimeReach,
         SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
         IngressDrainStrictlyDecreasesRuntimeReach,
         HeadTailProperties, IsaT(300), PTL
         DEF HistoricalTemporalStage2IdleHandoffCursorProgress,
             HistoricalTemporalStage2IdleHandoffAtDistance,
             HistoricalTemporalStage2HandoffProgressExit,
             Stage2HandoffCursorDistance,
             HistoricalTemporalStage2ExactIdleRetrySelected,
             HistoricalTemporalStage2ExactIdleRetryPending,
             HistoricalTemporalStage2ForeignIdleSkip,
             Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             Stage2DeferredHandoffToken,
             HistoricalTemporalStage2Owned,
             HistoricalProtectedServiceOwnershipExit,
             HistoricalProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateScheduled,
             DeferredCandidates, CandidateServiceRank,
             ServiceRankLess, ReadyRunAuxRank,
             ReadyRunAuxOrdering, ReadyRunAuxCarrier,
             ReadyRunDeferredRank, ReadyRunTimeoutRank,
             ReadyRunInnerRank, ReadyFifoDebt,
             ReadyDeferredCount, ReadyTimeoutDebt,
             ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
             PostGstRunHistoricalRecoveryNode,
             RunHistoricalRecoveryNode, RunNodeWork,
             ResolveRunNodeCandidateProducerContinuation,
             AsyncSchedulerExceptCausalControlAndNodeService,
             SerializedLocalPrecedesServeIngressStep,
             SelectedLocalAdmissionAdvance,
             AsyncServeIngressTargetOnlyTurn,
             LocalAdmissionStep, IngressDrainStep,
             SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             RuntimeStep,
             DeferredDrainStep, RemoveNextDeferredCommand,
             AdvanceNextDeferredClass, DeferredHandoffQueueHead,
             DeferredHandoffMatches,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution,
             InstallDeferredHandoff, RetainDeferredHandoffs,
             ClearDeferredHandoff, NextDeferredCommand,
             SelectedDeferredClass, DeferredClassQueue,
             CommandDispatchable, DiscardCommand,
             AsyncSpecAt, AsyncFairnessAt, AsyncFairActionAt,
             AsyncNext, AsyncAllVars
    <2>4. HistoricalTemporalStage2IdleHandoffAtDistance(
             candidate, distance)
             ~> (HistoricalTemporalStage2HandoffProgressExit(candidate)
                  \/ (HistoricalTemporalStage2ExactIdleRetryPending(
                        candidate)
                       /\ asyncDeferredDrainOwed[candidate.node]
                       /\ Stage2HandoffCursorDistance(candidate)
                            = distance))
      BY <2>1, PTL
         DEF HistoricalTemporalStage2IdleHandoffAtDistance,
             HistoricalTemporalStage2IdleHandoffAwaitingRearm,
             HistoricalTemporalStage2HandoffProgressExit
    <2> QED BY <2>3, <2>4, PTL
         DEF HistoricalTemporalStage2IdleHandoffCursorProgress
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage2ExactIdleRetryEventuallySelected ==
  \A initialContext, candidate:
    AsyncSpecAt(initialContext)
      => (HistoricalTemporalStage2ExactIdleRetryPending(candidate)
           ~> (HistoricalTemporalStage2HandoffProgressExit(candidate)
                \/ HistoricalTemporalStage2ExactIdleRetrySelected(
                     candidate)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                AsyncSpecAt(initialContext)
         PROVE HistoricalTemporalStage2ExactIdleRetryPending(candidate)
                 ~> (HistoricalTemporalStage2HandoffProgressExit(candidate)
                      \/ HistoricalTemporalStage2ExactIdleRetrySelected(
                           candidate))
    <2> DEFINE Goal ==
           HistoricalTemporalStage2HandoffProgressExit(candidate)
             \/ HistoricalTemporalStage2ExactIdleRetrySelected(candidate)
    <2>1. IsWellFoundedOn(OpToRel(<, Nat), 0..2)
      BY NatLessThanWellFounded, IsWellFoundedOnSubset, Isa
    <2>2. ASSUME NEW distance \in 0..2
           PROVE HistoricalTemporalStage2IdleHandoffAtDistance(
                   candidate, distance)
                   ~> (Goal
                        \/ \E lower \in SetLessThan(
                             distance, OpToRel(<, Nat), 0..2):
                             HistoricalTemporalStage2IdleHandoffAtDistance(
                               candidate, lower))
      BY <1>1, HistoricalTemporalStage2IdleHandoffCursorOneStep
         DEF Goal,
             HistoricalTemporalStage2IdleHandoffCursorProgress,
             SetLessThan
    <2>3. \A distance \in 0..2:
             HistoricalTemporalStage2IdleHandoffAtDistance(
               candidate, distance)
               ~> Goal
      BY <2>1, <2>2, WellFoundedLeadsTo
    <2>4. [](AsyncStrongTypeInvariant
              /\ AsyncDeferredHandoffOwnershipInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation, PTL
    <2>5. HistoricalTemporalStage2ExactIdleRetryPending(candidate)
             ~> \E distance \in 0..2:
                  HistoricalTemporalStage2IdleHandoffAtDistance(
                    candidate, distance)
      BY <2>4,
         HistoricalTemporalStage2HandoffDistanceInCarrier, PTL
         DEF HistoricalTemporalStage2IdleHandoffAtDistance
    <2> QED BY <2>3, <2>5, PTL DEF Goal
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage2ExactIdleRetryDrainConsumes ==
  \A candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncDeferredHandoffOwnershipInvariant
    /\ HistoricalTemporalStage2ExactIdleRetrySelected(candidate)
    /\ DeferredDrainStep(candidate.node)
    => /\ ~HistoricalProtectedCandidateOwned(candidate)'
       /\ ~Stage2DeferredHandoffOwned(candidate)'
BY AsyncStrongTypeProjectsAsyncType,
   AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   Stage2DeferredHandoffTokenIsInjectiveObligation,
   RuntimeSelectedCommandsAreTyped, HeadTailProperties,
   IsaT(240)
   DEF HistoricalTemporalStage2ExactIdleRetrySelected,
       HistoricalTemporalStage2ExactIdleRetryPending,
       Stage2DeferredHandoffOwned,
       Stage2ActiveDeferredHandoff,
       Stage2DeferredHandoffToken,
       HistoricalTemporalStage2Owned,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateScheduled, DeferredCandidates,
       DeferredDrainStep, DeferredHandoffActive,
       DeferredHandoffMatches, DeferredHandoffQueueHead,
       DeferredHandoffCandidate, DeferredHandoffAllowsExecution,
       DeferredHandoffBlocksExecution, RemoveNextDeferredCommand,
       ClearDeferredHandoff, RetainDeferredHandoffs,
       DiscardCommand, ExecuteCommand, AppendCausalSuccessors,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM HistoricalTemporalStage2ExactIdleRetryServed ==
  \A initialContext, candidate:
    AsyncSpecAt(initialContext)
      => (HistoricalTemporalStage2ExactIdleRetrySelected(candidate)
           ~> HistoricalTemporalStage2HandoffProgressExit(candidate))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                AsyncSpecAt(initialContext)
         PROVE HistoricalTemporalStage2ExactIdleRetrySelected(candidate)
                 ~> HistoricalTemporalStage2HandoffProgressExit(candidate)
    <2>1. [](AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ AsyncDeferredHandoffOwnershipInvariant
              /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
              /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
         AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity, PTL
    <2>2. [] [(\/ ~HistoricalTemporalStage2ExactIdleRetrySelected(
                       candidate)
                  \/ ~DeferredDrainStep(candidate.node)
                  \/ HistoricalTemporalStage2HandoffProgressExit(
                       candidate)')]_AsyncAllVars
      BY <2>1,
         HistoricalTemporalStage2ExactIdleRetryDrainConsumes, PTL
         DEF HistoricalTemporalStage2HandoffProgressExit
    <2>3. HistoricalTemporalStage2ExactIdleRetrySelected(candidate)
             ~> HistoricalTemporalStage2HandoffProgressExit(candidate)
      BY <1>1, <2>1, <2>2,
         Stage2DeferredHandoffTokenIsInjectiveObligation,
         ReadyRunAuxOrderingIsWellFounded,
         HistoricalTemporalProtectedOwnerEnablesFairRunner,
         LocalAdmissionStrictlyDecreasesRuntimeReach,
         SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
         IngressDrainStrictlyDecreasesRuntimeReach,
         HeadTailProperties, IsaT(300), PTL
         DEF HistoricalTemporalStage2HandoffProgressExit,
             HistoricalTemporalStage2ExactIdleRetrySelected,
             HistoricalTemporalStage2ExactIdleRetryPending,
             Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             Stage2DeferredHandoffToken,
             HistoricalTemporalStage2Owned,
             HistoricalProtectedServiceOwnershipExit,
             HistoricalProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateScheduled,
             DeferredCandidates, CandidateServiceRank,
             ServiceRankLess, ReadyRunAuxRank,
             ReadyRunAuxOrdering, ReadyRunAuxCarrier,
             ReadyRunDeferredRank, ReadyRunTimeoutRank,
             ReadyRunInnerRank, ReadyFifoDebt,
             ReadyDeferredCount, ReadyTimeoutDebt,
             ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
             PostGstRunHistoricalRecoveryNode,
             RunHistoricalRecoveryNode, RunNodeWork,
             ResolveRunNodeCandidateProducerContinuation,
             AsyncSchedulerExceptCausalControlAndNodeService,
             SerializedLocalPrecedesServeIngressStep,
             SelectedLocalAdmissionAdvance,
             AsyncServeIngressTargetOnlyTurn,
             LocalAdmissionStep, IngressDrainStep,
             SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             RuntimeStep,
             DeferredDrainStep, RemoveNextDeferredCommand,
             AdvanceNextDeferredClass, DeferredHandoffQueueHead,
             DeferredHandoffMatches,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution,
             InstallDeferredHandoff, RetainDeferredHandoffs,
             ClearDeferredHandoff, NextDeferredCommand,
             SelectedDeferredClass, DeferredClassQueue,
             CommandDispatchable, DiscardCommand,
             AsyncSpecAt, AsyncFairnessAt, AsyncFairActionAt,
             AsyncNext, AsyncAllVars
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage2IdleHandoffEventuallyExits ==
  \A initialContext, candidate:
    AsyncSpecAt(initialContext)
      => (HistoricalTemporalStage2ExactIdleRetryPending(candidate)
           ~> HistoricalTemporalStage2HandoffProgressExit(candidate))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate,
                AsyncSpecAt(initialContext)
         PROVE HistoricalTemporalStage2ExactIdleRetryPending(candidate)
                 ~> HistoricalTemporalStage2HandoffProgressExit(candidate)
    <2>1. HistoricalTemporalStage2ExactIdleRetryPending(candidate)
             ~> (HistoricalTemporalStage2HandoffProgressExit(candidate)
                  \/ HistoricalTemporalStage2ExactIdleRetrySelected(
                       candidate))
      BY <1>1,
         HistoricalTemporalStage2ExactIdleRetryEventuallySelected
    <2>2. HistoricalTemporalStage2ExactIdleRetrySelected(candidate)
             ~> HistoricalTemporalStage2HandoffProgressExit(candidate)
      BY <1>1, HistoricalTemporalStage2ExactIdleRetryServed
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

HistoricalTemporalStage2RankProgressExit(candidate, position) ==
  HistoricalTemporalRankProgressExit(candidate, <<2, position>>)

HistoricalTemporalStage2HandoffRankBlocked(candidate, position) ==
  /\ gst
  /\ HistoricalProtectedCandidateOwned(candidate)
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ ~ServiceRankLess(
       CandidateServiceRank(candidate), <<2, position>>)

HistoricalTemporalStage2RankOrHandoffProgress(candidate, position) ==
  \/ HistoricalTemporalStage2RankProgressExit(candidate, position)
  \/ HistoricalTemporalStage2HandoffRankBlocked(candidate, position)

THEOREM HistoricalTemporalStage2DeferredRankReachesExitOrHandoff ==
  \A initialContext, candidate, position:
    HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => (HistoricalProtectedOwnedAtServiceRank(
                  candidate, <<2, position>>)
                 ~> HistoricalTemporalStage2RankOrHandoffProgress(
                      candidate, position)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
                AsyncSpecAt(initialContext)
         PROVE HistoricalProtectedOwnedAtServiceRank(
                 candidate, <<2, position>>)
                 ~> HistoricalTemporalStage2RankOrHandoffProgress(
                      candidate, position)
    <2>1. [](AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ AsyncDeferredHandoffOwnershipInvariant
              /\ Stage2BusyKernelInvariant
              /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
              /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation,
         AsyncSpecAlwaysStage2BusyKernelObligation,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
         AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity, PTL
         DEF Stage2BusyKernelProperty
    <2>2. (HistoricalTemporalStage2Owned(candidate)
             /\ ~NodeIdle(candidate.node))
             ~> HistoricalTemporalStage2BusyTerminationGoal(candidate)
      BY <1>1, HistoricalTemporalStage2BusyTerminates
    <2>3. Stage2ExactDeferredHandoffProperty(
             AsyncSpecAt(initialContext))
      BY AsyncSpecHasExactDeferredHandoffObligation
    <2>4. HistoricalProtectedOwnedAtServiceRank(
             candidate, <<2, position>>)
             ~> HistoricalTemporalStage2RankOrHandoffProgress(
                  candidate, position)
      BY <1>1, <2>1, <2>2, <2>3,
         HistoricalTemporalStage2BusyRetryClaimsHandoffAction,
         Stage2SelectedDifferentDeferredClassDropsDistance,
         HistoricalTemporalStage2ForeignIdleSkipDropsDistance,
         HistoricalTemporalStage2IdleHandoffEventuallyExits,
         Stage2DeferredHandoffTokenIsInjectiveObligation,
         ReadyRunAuxOrderingIsWellFounded,
         ReadyRunAuxRankInCarrier,
         HistoricalTemporalProtectedOwnerEnablesFairRunner,
         LocalAdmissionStrictlyDecreasesRuntimeReach,
         SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
         IngressDrainStrictlyDecreasesRuntimeReach,
         HeadTailProperties, FS_CardinalityType,
         IsaT(600), PTL
         DEF HistoricalTemporalStage2RankOrHandoffProgress,
             HistoricalTemporalStage2RankProgressExit,
             HistoricalTemporalStage2HandoffRankBlocked,
             HistoricalTemporalRankProgressExit,
             HistoricalTemporalStage2BusyRejectedSelected,
             HistoricalTemporalStage2BusyRetryClaimsHandoff,
             HistoricalTemporalStage2BusyTerminationGoal,
             Stage2BusyKernelProperty,
             Stage2BusyKernelInvariant,
             Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             Stage2DeferredHandoffToken,
             HistoricalTemporalStage2Owned,
             HistoricalProtectedOwnedAtServiceRank,
             HistoricalProtectedServiceOwnershipExit,
             HistoricalProtectedCandidateOwned,
             ProtectedCandidateOwned, ProtectedServiceCandidate,
             CandidateScheduled, CandidateServiceRank,
             ServiceRankLess, DeferredCandidatePosition,
             DeferredCandidateIndices, DeferredClassPrefixIndices,
             DeferredCandidates, DeferredClassQueue,
             DeferredClassNonempty, DeferredQueueNonempty,
             DeferredHandoffActive, DeferredHandoffMatches,
             DeferredHandoffQueueHead, DeferredHandoffCandidate,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution,
             InstallDeferredHandoff, RetainDeferredHandoffs,
             ClearDeferredHandoff, NextDeferredCommand,
             SelectedDeferredClass, AdvanceNextDeferredClass,
             RemoveNextDeferredCommand, CommandClassDistance,
             NextCommandClass, SequenceSet,
             ReadyRunAuxRank, ReadyRunAuxOrdering,
             ReadyRunAuxCarrier, ReadyRunDeferredRank,
             ReadyRunTimeoutRank, ReadyRunInnerRank,
             ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
             ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
             PostGstRunHistoricalRecoveryNode,
             RunHistoricalRecoveryNode, RunNodeWork,
             ResolveRunNodeCandidateProducerContinuation,
             AsyncSchedulerExceptCausalControlAndNodeService,
             SerializedLocalPrecedesServeIngressStep,
             SelectedLocalAdmissionAdvance,
             AsyncServeIngressTargetOnlyTurn,
             LocalAdmissionStep, AdmitProducerCompletion,
             AdmitCausalHead, IngressDrainStep,
             SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             RuntimeStep,
             DeferredDrainStep, DeferredTagStep,
             DirectTimeoutStep, DirectRetransmitStep,
             FifoRuntimeStep, IdleRuntimeStep,
             RemoveNextNodeCommand, DeferCommand,
             DiscardCommand, CommandDispatchable,
             AsyncSpecAt, AsyncFairnessAt, AsyncFairActionAt,
             AsyncNext, AsyncNonCrashStep,
             AsyncRunnerStep, AsyncNonRunnerStep, AsyncAllVars
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM HistoricalTemporalStage2HandoffReachesRankExit ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => (HistoricalTemporalStage2HandoffRankBlocked(
            candidate, position)
           ~> HistoricalTemporalStage2RankProgressExit(
                candidate, position))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                AsyncSpecAt(initialContext)
         PROVE HistoricalTemporalStage2HandoffRankBlocked(
                 candidate, position)
                 ~> HistoricalTemporalStage2RankProgressExit(
                      candidate, position)
    <2>1. [](AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ AsyncDeferredHandoffOwnershipInvariant
              /\ Stage2BusyKernelInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysDeferredHandoffOwnershipObligation,
         AsyncSpecAlwaysStage2BusyKernelObligation, PTL
         DEF Stage2BusyKernelProperty
    <2>2. (HistoricalTemporalStage2Owned(candidate)
             /\ ~NodeIdle(candidate.node))
             ~> HistoricalTemporalStage2BusyTerminationGoal(candidate)
      BY <1>1, HistoricalTemporalStage2BusyTerminates
    <2>3. HistoricalTemporalStage2ExactIdleRetryPending(candidate)
             ~> HistoricalTemporalStage2HandoffProgressExit(candidate)
      BY <1>1, HistoricalTemporalStage2IdleHandoffEventuallyExits
    <2>4. Stage2ExactDeferredHandoffProperty(
             AsyncSpecAt(initialContext))
      BY AsyncSpecHasExactDeferredHandoffObligation
    <2>5. HistoricalTemporalStage2HandoffRankBlocked(
             candidate, position)
             ~> HistoricalTemporalStage2RankProgressExit(
                  candidate, position)
      BY <1>1, <2>1, <2>2, <2>3, <2>4,
         Stage2DeferredHandoffTokenIsInjectiveObligation,
         HeadTailProperties, IsaT(600), PTL
         DEF HistoricalTemporalStage2HandoffRankBlocked,
             HistoricalTemporalStage2RankProgressExit,
             HistoricalTemporalRankProgressExit,
             HistoricalTemporalStage2HandoffProgressExit,
             HistoricalTemporalStage2ExactIdleRetryPending,
             Stage2DeferredHandoffOwned,
             Stage2ActiveDeferredHandoff,
             Stage2DeferredHandoffToken,
             Stage2HandoffRetentionAction,
             Stage2HandoffClearOnlyOnExitAction,
             Stage2DeferredHandoffIdleReadyInvariant,
             Stage2ExactDeferredHandoffProperty,
             HistoricalTemporalStage2Owned,
             HistoricalTemporalStage2BusyTerminationGoal,
             HistoricalProtectedServiceOwnershipExit,
             HistoricalProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateScheduled,
             CandidateServiceRank, ServiceRankLess,
             DeferredCandidates, DeferredClassQueue,
             DeferredHandoffActive, DeferredHandoffCandidate,
             DeferredHandoffQueueHead, DeferredHandoffMatches,
             DeferredHandoffAllowsExecution,
             DeferredHandoffBlocksExecution,
             RemoveNextDeferredCommand, ClearDeferredHandoff,
             RetainDeferredHandoffs, DeferredDrainStep,
             FifoRuntimeStep, DeferCommand, DiscardCommand,
             RuntimeStep, SerializedRunnerRuntimeStep,
             SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep, RunNode,
             RunHistoricalRecoveryNode, RunNodeWork,
             ResolveRunNodeCandidateProducerContinuation,
             AsyncSchedulerExceptCausalControlAndNodeService,
             SerializedLocalPrecedesServeIngressStep,
             SelectedLocalAdmissionAdvance,
             AsyncServeIngressTargetOnlyTurn,
             AsyncNext, AsyncNonCrashStep,
             AsyncRunnerStep, AsyncNonRunnerStep, AsyncAllVars
    <2> QED BY <2>5
  <1> QED BY <1>1

THEOREM AsyncSpecClosesHistoricalTemporalStage2Leaf ==
  \A initialContext:
    HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => HistoricalProtectedStage2RankProgressProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
                NEW candidate \in AsyncCandidateSet,
                NEW position \in Nat
         PROVE AsyncSpecAt(initialContext)
                 => ((gst
                       /\ HistoricalProtectedCandidateOwned(candidate)
                       /\ CandidateServiceRank(candidate)
                            = <<2, position>>)
                      ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                           \/ \E lower \in SetLessThan(
                                <<2, position>>,
                                OwnedServiceRankOrdering,
                                OwnedServiceRankCarrier):
                                HistoricalProtectedOwnedAtServiceRank(
                                  candidate, lower)))
    <2>1. AsyncSpecAt(initialContext)
             => (HistoricalProtectedOwnedAtServiceRank(
                   candidate, <<2, position>>)
                   ~> HistoricalTemporalStage2RankOrHandoffProgress(
                        candidate, position))
      BY HistoricalTemporalStage2DeferredRankReachesExitOrHandoff
    <2>2. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage2HandoffRankBlocked(
                   candidate, position)
                   ~> HistoricalTemporalStage2RankProgressExit(
                        candidate, position))
      BY HistoricalTemporalStage2HandoffReachesRankExit
    <2>3. AsyncSpecAt(initialContext)
             => (HistoricalProtectedOwnedAtServiceRank(
                   candidate, <<2, position>>)
                   ~> HistoricalTemporalStage2RankProgressExit(
                        candidate, position))
      BY <2>1, <2>2, PTL
         DEF HistoricalTemporalStage2RankOrHandoffProgress
    <2>4. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2>5. AsyncSpecAt(initialContext)
             => (HistoricalTemporalStage2RankProgressExit(
                   candidate, position)
                   ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                        \/ \E lower \in SetLessThan(
                             <<2, position>>,
                             OwnedServiceRankOrdering,
                             OwnedServiceRankCarrier):
                             HistoricalProtectedOwnedAtServiceRank(
                               candidate, lower)))
      BY <2>4,
         HistoricalTemporalRankExitHasWellFoundedSuccessor, PTL
         DEF HistoricalTemporalStage2RankProgressExit
    <2> QED BY <2>3, <2>5, PTL
         DEF HistoricalProtectedOwnedAtServiceRank
  <1> QED BY <1>1
       DEF HistoricalProtectedStage2RankProgressProperty,
           HistoricalProtectedStageRankProgressProperty

HistoricalTemporalCandidateStageLeaves(specification) ==
  /\ HistoricalTemporalStage2Leaf(specification)
  /\ HistoricalTemporalStage3Leaf(specification)
  /\ HistoricalTemporalStage4Leaf(specification)
  /\ HistoricalTemporalStage5Leaf(specification)
  /\ HistoricalTemporalStage6Leaf(specification)

HistoricalTemporalRemainingCandidateStageLeaves(specification) ==
  TRUE

THEOREM AsyncSpecClosesAllHistoricalTemporalCandidateStageLeaves ==
  \A initialContext:
    HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => HistoricalTemporalCandidateStageLeaves(
           AsyncSpecAt(initialContext))
BY AsyncSpecClosesHistoricalTemporalStage2Leaf,
   AsyncSpecClosesHistoricalTemporalStage3Leaf,
   AsyncSpecClosesHistoricalTemporalStage4Leaf,
   AsyncSpecClosesHistoricalTemporalStage5Leaf,
   AsyncSpecClosesHistoricalTemporalStage6Leaf
   DEF HistoricalTemporalFiniteRunnerEpisodeClosureProperty,
       HistoricalTemporalStage6FiniteRunnerEpisodeClosureProperty,
       HistoricalTemporalCandidateStageLeaves,
       HistoricalTemporalStage2Leaf,
       HistoricalTemporalStage3Leaf,
       HistoricalTemporalStage4Leaf,
       HistoricalTemporalStage5Leaf,
       HistoricalTemporalStage6Leaf

THEOREM AsyncSpecAndRemainingHistoricalStagesCloseAllStageLeaves ==
  \A initialContext:
    /\ HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalTemporalRemainingCandidateStageLeaves(
         AsyncSpecAt(initialContext))
    => HistoricalTemporalCandidateStageLeaves(
         AsyncSpecAt(initialContext))
BY AsyncSpecClosesAllHistoricalTemporalCandidateStageLeaves
   DEF HistoricalTemporalRemainingCandidateStageLeaves,
       HistoricalTemporalCandidateStageLeaves

THEOREM HistoricalTemporalCandidateStageLeavesAreExact ==
  \A specification:
    HistoricalTemporalCandidateStageLeaves(specification)
      <=> HistoricalProtectedServiceRankLeafProperties(specification)
BY DEF HistoricalTemporalCandidateStageLeaves,
       HistoricalTemporalStage2Leaf,
       HistoricalTemporalStage3Leaf,
       HistoricalTemporalStage4Leaf,
       HistoricalTemporalStage5Leaf,
       HistoricalTemporalStage6Leaf,
       HistoricalProtectedServiceRankLeafProperties

(***************************************************************************
Exact packet/service corridor split.

Commit-certificate and certified-body transport remain distinct.  Both name
the four concrete transitions already proved in
`SumeragiV2AsyncHistoricalRecoveryTransportClosureProofs`: retransmission
publication, packet-to-ingress/Serve handoff, ordinary archive-I/O response,
and response admission at the historical recipient.
***************************************************************************)

HistoricalTemporalCommitTransportLeaves(specification) ==
  HistoricalCommitPhysicalTransportKernelProperties(specification)

HistoricalTemporalDecisionTransportLeaves(specification) ==
  HistoricalDecisionCertifiedTransportKernelProperties(specification)

HistoricalTemporalTransportLeaves(specification) ==
  /\ HistoricalTemporalCommitTransportLeaves(specification)
  /\ HistoricalTemporalDecisionTransportLeaves(specification)

THEOREM HistoricalTemporalTransportLeavesAreExact ==
  \A specification:
    HistoricalTemporalTransportLeaves(specification)
      <=> /\ HistoricalCommitPhysicalTransportKernelProperties(
                specification)
          /\ HistoricalDecisionCertifiedTransportKernelProperties(
                specification)
BY DEF HistoricalTemporalTransportLeaves,
       HistoricalTemporalCommitTransportLeaves,
       HistoricalTemporalDecisionTransportLeaves

(***************************************************************************
Fixed-clock prerequisite split.

The finite producer episode is not called progress.  Candidate lifecycle
coalescing/tombstones and Serve lifecycle ordinals are charged before the
occurrence-rank consumer.  Only exhaustion of that finite episode may feed
the strict fixed-clock rank goal.
***************************************************************************)

HistoricalTemporalFixedClockNonPacketLeaf(specification) ==
  HistoricalDiscoveryFixedClockNonPacketServiceProperty(specification)

HistoricalTemporalFixedClockPacketLeaf(specification) ==
  HistoricalDiscoveryFixedClockPacketServiceProperty(specification)

HistoricalTemporalFixedClockIdentityBudgetLeaf(specification) ==
  HistoricalDiscoveryCandidateServeIdentityBudgetProperty(specification)

HistoricalTemporalFixedClockLeaves(specification) ==
  /\ HistoricalTemporalFixedClockNonPacketLeaf(specification)
  /\ HistoricalTemporalFixedClockPacketLeaf(specification)
  /\ HistoricalTemporalFixedClockIdentityBudgetLeaf(specification)

THEOREM HistoricalTemporalFixedClockLeavesAreExact ==
  \A specification:
    HistoricalTemporalFixedClockLeaves(specification)
      <=> HistoricalDiscoveryFixedClockTemporalPrerequisites(specification)
BY DEF HistoricalTemporalFixedClockLeaves,
       HistoricalTemporalFixedClockNonPacketLeaf,
       HistoricalTemporalFixedClockPacketLeaf,
       HistoricalTemporalFixedClockIdentityBudgetLeaf,
       HistoricalDiscoveryFixedClockTemporalPrerequisites,
       HistoricalDiscoveryFixedClockConcreteServiceProperties

(***************************************************************************
Historical producer-continuation ingress-cut corridor.

The target remains in the global durable continuation table while a physical
Serve or leader-wire barrier owns the historical runner.  The immutable
target ordinal freezes a finite logical leader-wire universe.  Dormant
identities in that universe contribute two prepaid non-descent stages:
reactivation retains the identity but allocates a fresh physical carrier and
the current per-source predecessor snapshot, then Ingress drain consumes the
second stage.  Fresh identities use the current shared high-watermark and
cannot enter the old cut; terminal identities cannot resurrect after GST.
For the positive target ordinal, the strict scheduler cut is exactly the
Logical barrier cutoff `<= targetOrdinal - 1`; an equal-ordinal leader-wire
carrier is the target lifecycle cell and coalesces instead of being charged.

The inner continuation prefix already charges Candidate, Serve, and
Reserved/Materialized continuation tokens.  The leader-wire stage budget is
outside it, so admission or ingress-to-Candidate transfer cannot replenish
the composite rank.  The physical dependency tail carries the existing
earlier-carrier/frozen-prefix pair and explicit
mode/capacity/runner/priority-selector/lane/source components.  Reaching a
lower auxiliary rank is not target progress, and clearing the barrier is not
target progress: the only exposed exit is the same framed continuation prefix
becoming runner-eligible (or its independently proved strict prefix descent).
***************************************************************************)

HistoricalCandidateProducerContinuationIngressCutEpisodeRank(node, record) ==
  AsyncCandidateProducerContinuationIngressBarrierRank(
    node, record.ordinal)

HistoricalCandidateProducerContinuationIngressCutEpisodeRankCarrier ==
  AsyncFrozenLeaderWireBarrierRankCarrier

HistoricalCandidateProducerContinuationIngressCutEpisodeRankOrdering ==
  AsyncFrozenLeaderWireBarrierRankOrdering

HistoricalCandidateProducerContinuationIngressCutEpisodeAtRank(
    node, record, status, budget, episodeRank) ==
  /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
       node, record, status, budget)
  /\ episodeRank
       \in
         HistoricalCandidateProducerContinuationIngressCutEpisodeRankCarrier
  /\ episodeRank =
       HistoricalCandidateProducerContinuationIngressCutEpisodeRank(
         node, record)

HistoricalCandidateProducerContinuationIngressCutEpisodeGoal(
    node, record, status, budget, episodeRank) ==
  \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
       node, record, status, budget)
  \/ HistoricalCandidateProducerContinuationFrozenPrefixRunnerEligible(
       node, record, status, budget)
  \/ \E lower \in
       SetLessThan(
         episodeRank,
         HistoricalCandidateProducerContinuationIngressCutEpisodeRankOrdering,
         HistoricalCandidateProducerContinuationIngressCutEpisodeRankCarrier):
       HistoricalCandidateProducerContinuationIngressCutEpisodeAtRank(
         node, record, status, budget, lower)

HistoricalCandidateProducerContinuationIngressCutLeaderWireOwnsTurn(
    node, targetOrdinal) ==
  /\ AsyncLeaderWireIngressOwnsSharedPhysicalTurn(node)
  /\ (AsyncLeaderWireEarliestPhysicalIngressRecord(node)).schedulerOrdinal
       < targetOrdinal

HistoricalCandidateProducerContinuationIngressCutFairOwnerKinds ==
  {"HistoricalRunner", "HistoricalIoWorker"}

HistoricalCandidateProducerContinuationIngressCutFairOwner(
    node, targetOrdinal) ==
  IF HistoricalCandidateProducerContinuationIngressCutLeaderWireOwnsTurn(
       node, targetOrdinal)
  THEN "HistoricalRunner"
  ELSE IF AsyncCausalEpisodeIoOwnerRequired(node, targetOrdinal)
       THEN "HistoricalIoWorker"
       ELSE "HistoricalRunner"

HistoricalCandidateProducerContinuationIngressCutFairAction(
    node, ownerKind) ==
  CASE ownerKind = "HistoricalRunner" ->
         PostGstRunHistoricalRecoveryNode(node)
    [] ownerKind = "HistoricalIoWorker" ->
         PostGstServiceHistoricalRecoveryIoWorker(node)
    [] OTHER -> FALSE

THEOREM HistoricalCandidateProducerContinuationIngressCutRankIsFinite ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
         node, record, status, budget)
      => HistoricalCandidateProducerContinuationIngressCutEpisodeRank(
           node, record)
           \in
             HistoricalCandidateProducerContinuationIngressCutEpisodeRankCarrier
BY CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier,
   AsyncFrozenLeaderWireBarrierRankIsFinite, IsaT(600)
   DEF HistoricalCandidateProducerContinuationIngressCutEpisodeRank,
       HistoricalCandidateProducerContinuationIngressCutEpisodeRankCarrier,
       HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationIngressBarrierRank,
       AsyncFrozenLeaderWireBarrierModes

THEOREM HistoricalCandidateProducerContinuationIngressCutClassifiesPhysicalOwner ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
         node, record, status, budget)
      => /\ AsyncIngressSchedulerBarrierActive(node)
         /\ AsyncEarliestIngressSchedulerOrdinal(node) < record.ordinal
         /\ \/ HistoricalCandidateProducerContinuationIngressCutLeaderWireOwnsTurn(
                  node, record.ordinal)
            \/ /\ AsyncServeIngressOwnsSharedPhysicalTurn(node)
               /\ AsyncServeEarliestIngressSchedulerOrdinal(node)
                    < record.ordinal
         /\ HistoricalCandidateProducerContinuationIngressCutFairOwner(
              node, record.ordinal)
              \in
                HistoricalCandidateProducerContinuationIngressCutFairOwnerKinds
BY AsyncCandidateProducerContinuationLaterOrdinalCannotOwnRunnerTurn,
   AsyncCandidateProducerContinuationRunnerSelectionIsGlobalMinimum,
   AsyncSelectedLeaderWirePhysicalCarrierDefinesIngressScheduler,
   FS_CardinalityType, IsaT(1800)
   DEF HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationAtStatus,
       HistoricalCandidateProducerContinuationIngressCutLeaderWireOwnsTurn,
       HistoricalCandidateProducerContinuationIngressCutFairOwner,
       HistoricalCandidateProducerContinuationIngressCutFairOwnerKinds,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerMayPrecedeIngress,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncIngressSchedulerBarrierActive,
       AsyncEarliestIngressSchedulerOrdinal,
       AsyncLeaderWireIngressOwnsSharedPhysicalTurn,
       AsyncServeIngressOwnsSharedPhysicalTurn

THEOREM HistoricalCandidateProducerContinuationIngressCutPersistsTargetAndBudgetOrExits ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
         node, record, status, budget)
    /\ [AsyncNext]_AsyncAllVars
      => \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
               node, record, status, budget)'
         \/ HistoricalCandidateProducerContinuationFrozenPrefixRunnerEligible(
               node, record, status, budget)'
         \/ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
               node, record, status, budget)'
BY HistoricalCandidateProducerContinuationStepPersistsOrExits,
   HistoricalCandidateProducerContinuationFrozenPrefixStepCannotReplenish,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   IsaT(1800)
   DEF HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual,
       HistoricalCandidateProducerContinuationFrozenPrefixRunnerEligible,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       HistoricalCandidateProducerContinuationAtStatus,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncAllVars

THEOREM HistoricalCandidateProducerContinuationIngressCutStepIsGoalDescentOrFrame ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
         node, record, status, budget)
    /\ [AsyncNext]_AsyncAllVars
      => \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
               node, record, status, budget)'
         \/ HistoricalCandidateProducerContinuationFrozenPrefixRunnerEligible(
               node, record, status, budget)'
         \/ /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
                  node, record, status, budget)'
            /\ <<HistoricalCandidateProducerContinuationIngressCutEpisodeRank(
                    node, record)',
                  HistoricalCandidateProducerContinuationIngressCutEpisodeRank(
                    node, record)>>
                 \in
                   HistoricalCandidateProducerContinuationIngressCutEpisodeRankOrdering
         \/ /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
                  node, record, status, budget)'
            /\ HistoricalCandidateProducerContinuationIngressCutEpisodeRank(
                 node, record)'
                 =
               HistoricalCandidateProducerContinuationIngressCutEpisodeRank(
                 node, record)
BY HistoricalCandidateProducerContinuationIngressCutPersistsTargetAndBudgetOrExits,
   CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier,
   CandidateProducerContinuationEqualOrdinalLeaderWireCoalescesTargetCell,
   CandidateProducerContinuationFrozenPrefixStepCannotReplenish,
   CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight,
   CandidateProducerContinuationDormantLocalReplayChargeCannotAppearAtGst,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   PostGstStepCannotCreateDormantLeaderWirePotential,
   AdmitDormantLeaderWireRetainsLifecycleTokenAndFrozenPrefix,
   LeaderWireIngressDrainNeverInventsRuntimeOwner,
   RuntimeLeaderWireCannotRetireMerelyFromIngressPop,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   ExactTicketTurnDecreasesDrainableIngressTurnReach,
   ExhaustedIngressStepDecreasesDrainableIngressTurnReach,
   LocalStepDecreasesDrainableIngressTurnReach,
   SerializedLocalPredecessorDecreasesDrainableIngressTurnReach,
   RuntimeStepDecreasesDrainableIngressTurnReach,
   OlderRuntimeInterleaveDecreasesDrainableIngressTurnReach,
   FS_CardinalityType, FS_Subset, IsaT(7200)
   DEF HistoricalCandidateProducerContinuationIngressCutEpisodeRank,
       HistoricalCandidateProducerContinuationIngressCutEpisodeRankOrdering,
       AsyncCandidateProducerContinuationIngressBarrierRank,
       AsyncFrozenLeaderWireBarrierRank,
       AsyncFrozenLeaderWireBarrierStageBudget,
       AsyncFrozenLeaderWireBarrierStageTokens,
       AsyncFrozenLeaderWireBarrierRemainingStage,
       AsyncFrozenLeaderWireBarrierRecords,
       AsyncFrozenLeaderWireBarrierTailRank,
       AsyncFrozenLeaderWireIngressDependencyRank,
       AsyncFrozenLeaderWireIngressRecords,
       AsyncFrozenLeaderWireSelectedIngressRecord,
       AsyncFrozenLeaderWireIngressRank,
       AsyncFrozenLeaderWireIngressModeRank,
       AsyncFrozenLeaderWireIngressCapacityRank,
       AsyncFrozenLeaderWireIngressRunnerRank,
       AsyncFrozenLeaderWireIngressPriorityRank,
       AsyncFrozenLeaderWireIngressPriorityOwners,
       AsyncFrozenLeaderWireIngressLanePosition,
       AsyncFrozenLeaderWireIngressLaneIndices,
       AsyncFrozenLeaderWireIngressSourcePosition,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenProducerBudget,
       AsyncCandidateProducerContinuationFrozenProducerTokens,
       AsyncCandidateProducerContinuationFrozenCandidateTokens,
       AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCausalEpisodeServeWorkBudget,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeServeOccurrenceTokens,
       AsyncCausalEpisodeServeIngressPrefixTokens,
       AsyncCausalEpisodeServeIoPredecessorTokens,
       AsyncCausalEpisodeServeReachDebt,
       AsyncLeaderWirePhysicalIngressRank,
       AsyncLeaderWireEarlierPhysicalOwners,
       AsyncLeaderWireFrozenIngressPredecessorDebtSet,
       AsyncLeaderWireLifecycleStateAfterIngressAdmission,
       AsyncLeaderWireLifecycleRecordAfterIngressDrain,
       AsyncLeaderWireLifecyclesAfterIngressDrain,
       AsyncLeaderWireIngressPrefixSnapshot,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ServiceIoWorkerWork, PopSelectedIngress,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       LocalAdmissionStep, IngressDrainStep,
       SerializedLocalPrecedesServeIngressStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       AsyncNext, AsyncAllVars,
       LexPairOrdering, OpToRel

THEOREM HistoricalCandidateProducerContinuationIngressCutSelectedOwnerIsEnabled ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
         node, record, status, budget)
      => LET owner ==
               HistoricalCandidateProducerContinuationIngressCutFairOwner(
                 node, record.ordinal)
         IN /\ owner
                  \in
                    HistoricalCandidateProducerContinuationIngressCutFairOwnerKinds
            /\ ENABLED
                 <<HistoricalCandidateProducerContinuationIngressCutFairAction(
                     node, owner)>>_AsyncAllVars
BY HistoricalCandidateProducerContinuationIngressCutClassifiesPhysicalOwner,
   HistoricalRecoveryRunnerEnabledAfterGst,
   HistoricalRecoveryIoWorkerEnabledAfterGst,
   HistoricalTemporalQueuedIoServiceIsNonstuttering,
   ENABLEDaxioms, IsaT(1800)
   DEF HistoricalCandidateProducerContinuationIngressCutFairOwner,
       HistoricalCandidateProducerContinuationIngressCutFairOwnerKinds,
       HistoricalCandidateProducerContinuationIngressCutFairAction,
       HistoricalCandidateProducerContinuationIngressCutLeaderWireOwnsTurn,
       AsyncCausalEpisodeIoOwnerRequired,
       AsyncCausalEpisodeServeIngressIdentities,
       HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationAtStatus,
       HistoricalRecoveryTarget,
       PostGstServiceHistoricalRecoveryIoWorker,
       ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, AsyncAllVars

THEOREM HistoricalCandidateProducerContinuationIngressCutSelectedActionConsumesCell ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    LET owner ==
          HistoricalCandidateProducerContinuationIngressCutFairOwner(
            node, record.ordinal)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
           node, record, status, budget)
       /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
       /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
       /\ <<HistoricalCandidateProducerContinuationIngressCutFairAction(
               node, owner)>>_AsyncAllVars
       => \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
                node, record, status, budget)'
          \/ HistoricalCandidateProducerContinuationFrozenPrefixRunnerEligible(
                node, record, status, budget)'
          \/ /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
                   node, record, status, budget)'
             /\ <<HistoricalCandidateProducerContinuationIngressCutEpisodeRank(
                     node, record)',
                   HistoricalCandidateProducerContinuationIngressCutEpisodeRank(
                     node, record)>>
                  \in
                    HistoricalCandidateProducerContinuationIngressCutEpisodeRankOrdering
BY HistoricalCandidateProducerContinuationIngressCutStepIsGoalDescentOrFrame,
   HistoricalCandidateProducerContinuationIngressCutClassifiesPhysicalOwner,
   HistoricalCandidateProducerContinuationIngressCutSelectedOwnerIsEnabled,
   ServiceIoWorkerDropsQueueDepth,
   IsaT(3600)
   DEF HistoricalCandidateProducerContinuationIngressCutFairAction,
       HistoricalCandidateProducerContinuationIngressCutFairOwner,
       HistoricalCandidateProducerContinuationIngressCutLeaderWireOwnsTurn,
       AsyncCausalEpisodeIoOwnerRequired,
       PostGstRunHistoricalRecoveryNode,
       PostGstServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, AsyncAllVars

THEOREM HistoricalCandidateProducerContinuationIngressCutOwnerPersistsInCell ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    LET owner ==
          HistoricalCandidateProducerContinuationIngressCutFairOwner(
            node, record.ordinal)
        episodeRank ==
          HistoricalCandidateProducerContinuationIngressCutEpisodeRank(
            node, record)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
            node, record, status, budget)
       /\ [AsyncNext]_AsyncAllVars
       /\ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
            node, record, status, budget)'
       /\ HistoricalCandidateProducerContinuationIngressCutEpisodeRank(
            node, record)' = episodeRank
       => HistoricalCandidateProducerContinuationIngressCutFairOwner(
            node, record.ordinal)' = owner
BY HistoricalCandidateProducerContinuationIngressCutPersistsTargetAndBudgetOrExits,
   CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   PostGstStepCannotCreateDormantLeaderWirePotential,
   IsaT(2400)
   DEF HistoricalCandidateProducerContinuationIngressCutFairOwner,
       HistoricalCandidateProducerContinuationIngressCutLeaderWireOwnsTurn,
       HistoricalCandidateProducerContinuationIngressCutEpisodeRank,
       AsyncCandidateProducerContinuationIngressBarrierRank,
       AsyncFrozenLeaderWireBarrierRank,
       AsyncFrozenLeaderWireBarrierStageBudget,
       AsyncFrozenLeaderWireBarrierStageTokens,
       AsyncFrozenLeaderWireBarrierRecords,
       AsyncFrozenLeaderWireIngressRecords,
       AsyncCausalEpisodeIoOwnerRequired,
       AsyncCausalEpisodeServeIngressIdentities,
       AsyncAllVars

THEOREM HistoricalCandidateProducerContinuationIngressCutOwnerUsesAsyncFairness ==
  \A initialContext,
     node \in Responsive,
     ownerKind \in
       HistoricalCandidateProducerContinuationIngressCutFairOwnerKinds:
    AsyncLiveSpecAt(initialContext)
      => WF_AsyncAllVars(
           HistoricalCandidateProducerContinuationIngressCutFairAction(
             node, ownerKind))
BY Isa
   DEF HistoricalCandidateProducerContinuationIngressCutFairOwnerKinds,
       HistoricalCandidateProducerContinuationIngressCutFairAction,
       AsyncLiveSpecAt, AsyncSpecAt, AsyncFairnessAt

HistoricalCandidateProducerContinuationIngressCutEpisodeRankStepProperty(
    specification) ==
  specification
    => \A node \in ValidatorIds,
          record \in AsyncCandidateProducerContinuationRecordSet,
          status \in {"Reserved", "Materialized"},
          budget \in
            AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
          episodeRank \in
            HistoricalCandidateProducerContinuationIngressCutEpisodeRankCarrier:
         HistoricalCandidateProducerContinuationIngressCutEpisodeAtRank(
           node, record, status, budget, episodeRank)
           ~> HistoricalCandidateProducerContinuationIngressCutEpisodeGoal(
                node, record, status, budget, episodeRank)

THEOREM AsyncLiveProvidesHistoricalCandidateProducerContinuationIngressCutRankStep ==
  \A initialContext:
    HistoricalCandidateProducerContinuationIngressCutEpisodeRankStepProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncFiniteRunnerSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   HistoricalCandidateProducerContinuationIngressCutRankIsFinite,
   HistoricalCandidateProducerContinuationIngressCutSelectedOwnerIsEnabled,
   HistoricalCandidateProducerContinuationIngressCutStepIsGoalDescentOrFrame,
   HistoricalCandidateProducerContinuationIngressCutSelectedActionConsumesCell,
   HistoricalCandidateProducerContinuationIngressCutOwnerPersistsInCell,
   HistoricalCandidateProducerContinuationIngressCutOwnerUsesAsyncFairness,
   AsyncLiveSpecProjectsAsyncSpec,
   WF1, PTL, IsaT(2400)
   DEF HistoricalCandidateProducerContinuationIngressCutEpisodeRankStepProperty,
       HistoricalCandidateProducerContinuationIngressCutEpisodeAtRank,
       HistoricalCandidateProducerContinuationIngressCutEpisodeGoal,
       HistoricalCandidateProducerContinuationIngressCutFairOwner,
       HistoricalCandidateProducerContinuationIngressCutFairAction,
       HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationAtStatus,
       AsyncLiveSpecAt

HistoricalCandidateProducerContinuationIngressCutRankedClosureProperty(
    specification) ==
  specification
    => \A node \in ValidatorIds,
          record \in AsyncCandidateProducerContinuationRecordSet,
          status \in {"Reserved", "Materialized"},
          budget \in
            AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
         HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
           node, record, status, budget)
           ~> \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
                    node, record, status, budget)
               \/ HistoricalCandidateProducerContinuationFrozenPrefixRunnerEligible(
                    node, record, status, budget)

THEOREM AsyncLiveProvidesHistoricalCandidateProducerContinuationIngressCutRankedClosure ==
  \A initialContext:
    HistoricalCandidateProducerContinuationIngressCutRankedClosureProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesHistoricalCandidateProducerContinuationIngressCutRankStep,
   HistoricalCandidateProducerContinuationIngressCutRankIsFinite,
   AsyncFrozenLeaderWireBarrierRankOrderingIsWellFounded,
   WellFoundedLeadsTo, PTL
   DEF HistoricalCandidateProducerContinuationIngressCutRankedClosureProperty,
       HistoricalCandidateProducerContinuationIngressCutEpisodeRankStepProperty,
       HistoricalCandidateProducerContinuationIngressCutEpisodeAtRank,
       HistoricalCandidateProducerContinuationIngressCutEpisodeGoal,
       HistoricalCandidateProducerContinuationIngressCutEpisodeRankOrdering,
       HistoricalCandidateProducerContinuationIngressCutEpisodeRankCarrier

THEOREM AsyncLiveProvidesHistoricalCandidateProducerContinuationIngressCutClosure ==
  \A initialContext:
    HistoricalCandidateProducerContinuationIngressCutClosureProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesHistoricalCandidateProducerContinuationIngressCutRankedClosure
   DEF HistoricalCandidateProducerContinuationIngressCutClosureProperty,
       HistoricalCandidateProducerContinuationIngressCutRankedClosureProperty

THEOREM AsyncLiveProvidesHistoricalCandidateProducerContinuationResolutionClosure ==
  \A initialContext:
    HistoricalCandidateProducerContinuationResolutionClosureProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesHistoricalCandidateProducerContinuationIngressCutClosure,
   HistoricalCandidateProducerContinuationIngressCutClosureClosesResolution

(***************************************************************************
Historical Candidate/Serve lifecycle bridge.

This is proved directly from `AsyncNext`, rather than imported from the
adequate-leader closure.  The bridge is durable identity accounting only: it
does not assert that a producer action occurs and it does not treat
replenishment as progress.  Candidate identity uses the route-neutral
semantic service key plus the immutable consumer event tag.  Serve identity
uses the exact logical request key and the family high-watermark already
stored by the atomic reservation transition.
***************************************************************************)

HistoricalTemporalCandidateServiceTombstonesInIdentityCarrier(carrier) ==
  {record \in AsyncCandidateServiceTombstones:
     record.identity \in carrier}

HistoricalTemporalCandidateIdentityBudgetBridgeProperty(specification) ==
  /\ (specification
        => []AsyncCandidateServiceTombstoneLifecycleInvariant)
  /\ (specification
        => \A carrier:
             IsFiniteSet(carrier)
               => [](
                    Cardinality(
                      HistoricalTemporalCandidateServiceTombstonesInIdentityCarrier(
                        carrier))
                      <= Cardinality(carrier)))
  /\ (specification
        => [](\A candidate \in AsyncCandidateSet:
               /\ AsyncCandidateServiceActiveTombstone(candidate)
               /\ [AsyncNext]_AsyncAllVars
               /\ ~AsyncCandidateServiceExitThisStep(candidate)
               => AsyncCandidateServiceActiveTombstone(candidate)'))
  /\ (specification
        => [](\A left, right \in AsyncCandidateSet:
               /\ left.node = right.node
               /\ left.consumerContext = right.consumerContext
               /\ left.height = right.height
               /\ left.view = right.view
               /\ left.subject = right.subject
               /\ left.kind = right.kind
               /\ left.class = right.class
               /\ left.item # NoAsyncItem
               /\ right.item # NoAsyncItem
               /\ left.item.kind = "CertifiedResponse"
               /\ right.item =
                    [left.item EXCEPT !.source = right.item.source]
               /\ AsyncRouteNeutralCandidateEvidence(left.evidence)
                    = AsyncRouteNeutralCandidateEvidence(right.evidence)
               /\ left.causalOrigin = right.causalOrigin
               /\ left.bodyIdentity = right.bodyIdentity
               /\ left.manifestIdentity = right.manifestIdentity
               /\ left.commitmentIdentity = right.commitmentIdentity
               => AsyncCandidateServiceIdentity(left)
                    = AsyncCandidateServiceIdentity(right)))
  /\ (specification
        => [](\A identity \in AsyncCandidateAdmissionIdentitySet:
               /\ AsyncCandidateAdmissionIdentityObsolete(identity)
               /\ identity
                    \notin AsyncScheduledCandidateAdmissionIdentities
               /\ gst
               /\ [AsyncNext]_AsyncAllVars
               => /\ AsyncCandidateAdmissionIdentityObsolete(identity)'
                  /\ identity
                       \notin AsyncScheduledCandidateAdmissionIdentities'))
  /\ (specification
        => [](\A identity \in AsyncCandidateAdmissionIdentitySet:
               /\ identity.service.phase = "DeliverChunk"
               /\ AsyncCandidateAdmissionIdentityTerminallyCovered(identity)
               /\ identity
                    \notin AsyncScheduledCandidateAdmissionIdentities
               /\ gst
               /\ [AsyncNext]_AsyncAllVars
               => /\ AsyncCandidateAdmissionIdentityTerminallyCovered(
                       identity)'
                  /\ identity
                       \notin AsyncScheduledCandidateAdmissionIdentities'))
  /\ (specification
        => [](\A identity \in AsyncCandidateAdmissionIdentitySet:
               /\ identity.service.phase = "DeliverChunk"
               /\ identity \in AsyncScheduledCandidateAdmissionIdentities
               /\ gst
               /\ [AsyncNext]_AsyncAllVars
               /\ identity
                    \notin AsyncScheduledCandidateAdmissionIdentities'
               => AsyncCandidateAdmissionIdentityLifecycleCovered(
                    identity)'))

HistoricalTemporalServeReservationsInIdentityCarrier(carrier) ==
  {reservation \in asyncServeReservations:
     reservation.identity \in carrier}

HistoricalTemporalServeTombstonesInIdentityCarrier(carrier) ==
  {tombstone \in asyncServeTombstones:
     tombstone.identity \in carrier}

HistoricalTemporalServeRollbackTombstonesInIdentityCarrier(carrier) ==
  UNION {
    {tombstone \in reservation.rollbackTombstones:
       tombstone.identity \in carrier}:
    reservation \in asyncServeReservations}

HistoricalTemporalServeRetiredRecordsInIdentityCarrier(carrier) ==
  HistoricalTemporalServeTombstonesInIdentityCarrier(carrier)
    \cup
  HistoricalTemporalServeRollbackTombstonesInIdentityCarrier(carrier)

HistoricalTemporalServeExactRetryCoalescingAction(node, candidate) ==
  \/ CoalesceExactServeIngressCapacity(node, candidate)
  \/ ResumeExactServeCapacity(node, candidate)
  \/ CoalesceExactServeCapacity(node, candidate)
  \/ CoalesceSupersededExactServeRequest(node, candidate)
  \/ RejectConflictingExactServeRequest(node, candidate)

THEOREM HistoricalTemporalServeExactRetryKeepsAdmissionHighWatermark ==
  \A node, candidate:
    HistoricalTemporalServeExactRetryCoalescingAction(node, candidate)
      => UNCHANGED asyncNextServeAdmissionOrdinal
BY Isa
   DEF HistoricalTemporalServeExactRetryCoalescingAction,
       CoalesceExactServeIngressCapacity,
       ResumeExactServeCapacity,
       CoalesceExactServeCapacity,
       CoalesceSupersededExactServeRequest,
       RejectConflictingExactServeRequest,
       AsyncServeLifecycleVars

HistoricalTemporalServeIdentityBudgetBridgeProperty(specification) ==
  /\ (specification => []AsyncServeLifecycleTypeInvariant)
  /\ (specification
        => [](/\ IsFiniteSet(asyncServeReservations)
              /\ IsFiniteSet(asyncServeTombstones)
              /\ Cardinality(asyncServeTombstones)
                   <= Cardinality(AsyncServeLifecycleFamilies)))
  /\ (specification
        => \A carrier:
             IsFiniteSet(carrier)
               => [](/\ IsFiniteSet(
                            HistoricalTemporalServeReservationsInIdentityCarrier(
                              carrier))
                      /\ IsFiniteSet(
                            HistoricalTemporalServeTombstonesInIdentityCarrier(
                              carrier))
                      /\ IsFiniteSet(
                            HistoricalTemporalServeRollbackTombstonesInIdentityCarrier(
                              carrier))
                      /\ IsFiniteSet(
                            HistoricalTemporalServeRetiredRecordsInIdentityCarrier(
                              carrier))
                      /\ Cardinality(
                           HistoricalTemporalServeTombstonesInIdentityCarrier(
                             carrier))
                           <= Cardinality(carrier)))
  /\ (specification
        => [](\A node \in ValidatorIds,
                   identity \in AsyncServeLogicalRequestIdentities:
               AsyncServeLiveReservationOwned(node, identity)
                 => /\ Cardinality(
                          AsyncServeReservationRecords(node, identity)) = 1
                    /\ AsyncServeAdmissionOrdinal(node, identity)
                         < asyncNextServeAdmissionOrdinal[node]))
  /\ (specification
        => [](\A node \in ValidatorIds,
                   family \in AsyncServeLifecycleFamilies:
               AsyncServeLifecycleFamilyOwned(node, family)
                 => /\ Cardinality(
                          AsyncServeFamilyAdmissionRecords(node, family)
                            \cup
                          AsyncServeFamilyTombstoneRecords(node, family)) = 1
                    /\ AsyncServeFamilyOwnerIdentity(node, family)
                         \in AsyncServeLogicalRequestIdentities
                    /\ AsyncServeFamilyHighWatermark(node, family)
                         \in Views))
  /\ (specification
        => [](\A node \in ValidatorIds,
                   identity \in AsyncServeLogicalRequestIdentities:
               /\ AsyncServeJobQueued(node, identity)
               /\ gst
               /\ [AsyncNext]_AsyncAllVars
               /\ ~AsyncServeJobQueued(node, identity)'
               => AsyncServeLifecycleTombstone(node, identity)'))
  /\ (specification
        => [](\A node \in ValidatorIds,
                   identity \in AsyncServeLogicalRequestIdentities:
               /\ AsyncServeLogicalIdentityRetiredOrSuperseded(
                    node, identity)
               /\ gst
               /\ [AsyncNext]_AsyncAllVars
               => /\ AsyncServeLogicalIdentityRetiredOrSuperseded(
                       node, identity)'
                  /\ ~AsyncServeJobQueued(node, identity)'))
  /\ (specification
        => [](\A node, candidate:
               HistoricalTemporalServeExactRetryCoalescingAction(
                 node, candidate)
                 => UNCHANGED asyncNextServeAdmissionOrdinal))
  /\ (specification
        => [](\A node \in ValidatorIds,
                   left, right \in
                     AsyncCertifiedRequestItems
                       \cup AsyncCommitCertificateRequestItems:
               AsyncServeLogicalRequestIdentity(node, left)
                 = AsyncServeLogicalRequestIdentity(node, right)
                 => LET identity ==
                          AsyncServeLogicalRequestIdentity(node, left)
                    IN /\ AsyncServeReservationRecords(node, identity)
                            =
                          AsyncServeReservationRecords(
                            node,
                            AsyncServeLogicalRequestIdentity(node, right))
                       /\ AsyncServeTombstoneRecords(node, identity)
                            =
                          AsyncServeTombstoneRecords(
                            node,
                            AsyncServeLogicalRequestIdentity(node, right))
                       /\ (AsyncServeLifecycleOwned(node, identity)
                             =>
                           AsyncServeAdmissionOrdinal(node, identity)
                             =
                           AsyncServeAdmissionOrdinal(
                             node,
                             AsyncServeLogicalRequestIdentity(
                               node, right)))))

HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty(specification) ==
  /\ HistoricalTemporalCandidateIdentityBudgetBridgeProperty(specification)
  /\ HistoricalTemporalServeIdentityBudgetBridgeProperty(specification)

HistoricalTemporalIdentityLifecycleInvariant ==
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant
  /\ AsyncServeLifecycleTypeInvariant

THEOREM HistoricalTemporalInitEstablishesCandidateServiceTombstones ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncCandidateServiceTombstoneLifecycleInvariant
BY AsyncInitEstablishesLeaderWireContinuationSharedOrdinalNoCollision,
   Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncRuntimeInit, AsyncIoInit, AsyncDeferredInit,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateProducerSemanticHandoffCoverageInvariant,
       AsyncCandidateLifecycleAdmissions,
       AsyncInitialCandidateLifecycleAdmissions,
       AsyncCandidateLifecycleAdmission,
       AsyncControlServiceStateTypeInvariant,
       AsyncCandidateServiceTombstones,
       AsyncCandidateServiceRecordsFor,
       AsyncCandidateServiceRecordsForIdentity,
       QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       SequenceSet

THEOREM HistoricalTemporalNextPreservesCandidateServiceTombstones ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant
  /\ AsyncNext
  => AsyncCandidateServiceTombstoneLifecycleInvariant'
BY AsyncNextPreservesControlServiceStateTypeInvariant,
   AsyncNextPreservesLeaderWireContinuationSharedOrdinalNoCollision,
   AsyncControlServiceTransitionPreservesSemanticHandoffCoverage,
   AsyncCandidateServicesThisStepIsSingleton,
   AsyncCandidateTerminalRetirementsThisStepIsSingleton,
   AsyncCandidateSuccessfulServiceInstallsTombstone,
   AsyncCandidateDiscardInstallsTerminalTombstone,
   AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateIoCompletionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   AsyncCandidateBusyDeferralTransfersSameOwner,
   AsyncCandidateDeferredHandoffRetainsSameOwner,
   AsyncCandidateDiscardIsNotSemanticService,
   AsyncCandidateServiceTombstoneCoalescesFreshCandidate,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateSameHeightRestartPreservesServicedIdentity,
   IsaT(600)
   DEF AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncStrongTypeInvariant,
       AsyncProgressOwnershipInvariant,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       AsyncSchedulerExceptCausalControlAndNodeService,
       LocalAdmissionStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, RuntimeStep,
       DrainFairIngressSelected, AdmitCausalHead,
       AdmitProducerCompletion, ServiceIoWorkerWork,
       FifoRuntimeStep, DeferredDrainStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       FreshCandidateSequence, CandidateAdmissionCoalesced,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       FreshRestartCandidateSequence,
       CandidateScheduled, CandidateScheduledAfter

THEOREM HistoricalTemporalInitEstablishesIdentityLifecycle ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => HistoricalTemporalIdentityLifecycleInvariant
BY HistoricalTemporalInitEstablishesCandidateServiceTombstones,
   AsyncInitEstablishesStrongTypeInvariant
   DEF HistoricalTemporalIdentityLifecycleInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant

THEOREM HistoricalTemporalBracketPreservesIdentityLifecycle ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ HistoricalTemporalIdentityLifecycleInvariant
  /\ [AsyncNext]_AsyncAllVars
  => HistoricalTemporalIdentityLifecycleInvariant'
BY HistoricalTemporalNextPreservesCandidateServiceTombstones,
   AsyncBracketNextPreservesStrongTypeInvariant, Isa
   DEF HistoricalTemporalIdentityLifecycleInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncAllVars

THEOREM AsyncSpecAlwaysHistoricalTemporalCandidateServiceTombstones ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []AsyncCandidateServiceTombstoneLifecycleInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext)
         PROVE []AsyncCandidateServiceTombstoneLifecycleInvariant
    <2>1. AsyncInitAt(initialContext)
             => AsyncCandidateServiceTombstoneLifecycleInvariant
      BY HistoricalTemporalInitEstablishesCandidateServiceTombstones
    <2>2. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>3. []AsyncProgressOwnershipInvariant
      BY <1>1, AsyncSpecAlwaysProgressOwnershipInvariant
    <2>4. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ AsyncCandidateServiceTombstoneLifecycleInvariant
           /\ [AsyncNext]_AsyncAllVars
          => AsyncCandidateServiceTombstoneLifecycleInvariant'
      BY HistoricalTemporalNextPreservesCandidateServiceTombstones, Isa
         DEF AsyncAllVars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM AsyncSpecProvidesHistoricalTemporalCandidateIdentityBridge ==
  \A initialContext:
    HistoricalTemporalCandidateIdentityBudgetBridgeProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysHistoricalTemporalCandidateServiceTombstones,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncCandidateTombstoneSubsetIsBoundedByFrozenOwnerCarrier,
   AsyncCandidateServicedIdentityCannotReactivate,
   AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst,
   AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncCandidateServiceRouteNeutralResponseRetryIsStable,
   Isa, PTL
   DEF HistoricalTemporalCandidateIdentityBudgetBridgeProperty,
       HistoricalTemporalCandidateServiceTombstonesInIdentityCarrier,
       AsyncStrongTypeInvariant,
       AsyncAllVars

THEOREM AsyncSpecProvidesHistoricalTemporalServeIdentityBridge ==
  \A initialContext:
    HistoricalTemporalServeIdentityBudgetBridgeProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeRetiredIdentityCannotRequeueAtGst,
   HistoricalTemporalServeExactRetryKeepsAdmissionHighWatermark,
   FS_Union, FS_Subset, Isa, PTL
   DEF HistoricalTemporalServeIdentityBudgetBridgeProperty,
       HistoricalTemporalServeReservationsInIdentityCarrier,
       HistoricalTemporalServeTombstonesInIdentityCarrier,
       HistoricalTemporalServeRollbackTombstonesInIdentityCarrier,
       HistoricalTemporalServeRetiredRecordsInIdentityCarrier,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncServeLifecycleTypeInvariant,
       AsyncServeLifecyclePartitionInvariant,
       AsyncServeFamilyHighWatermarkInvariant,
       AsyncServeReservationOwnershipInvariant,
       AsyncServeOrdinalInvariant,
       AsyncServeLiveReservationOwned,
       AsyncServeLifecycleOwned,
       AsyncServeLifecycleFamilyOwned,
       AsyncServeAdmissionOrdinal,
       AsyncServeFamilyOwnerIdentity,
       AsyncServeFamilyHighWatermark,
       AsyncAllVars

THEOREM AsyncSpecProvidesHistoricalTemporalCandidateServeIdentityBridge ==
  \A initialContext:
    HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalTemporalCandidateIdentityBridge,
   AsyncSpecProvidesHistoricalTemporalServeIdentityBridge
   DEF HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty

HistoricalAsyncTemporalSupportLeaves(specification) ==
  /\ HistoricalTemporalFixedClockLeaves(specification)
  /\ HistoricalTemporalCandidateStageLeaves(specification)
  /\ HistoricalTemporalTransportLeaves(specification)

=============================================================================
