---- MODULE SumeragiV2AsyncStage4RefinementProofs ----
EXTENDS SumeragiV2AsyncTemporalRankProofs

(***************************************************************************
The ordinary Stage-4 auxiliary proof stops when sticky non-Completion causal
debt owns a class-prefix reservation.  From that point onward the reservation
gates make command occupancy non-increasing.  This outer rank counts the FIFO
removals needed to open the exact Normal/Progress prefix; the existing
ReadyRunAuxRank is the inner rank which forces each such removal through the
runtime's concrete deferred/tag/timeout/retransmit priority order.
***************************************************************************)

Stage4CapacityRank(node) ==
  <<CausalCommandCapacityDebt(node), ReadyRunAuxRank(node)>>

Stage4CapacityCarrier == Nat \X ReadyRunAuxCarrier

Stage4CapacityOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), ReadyRunAuxOrdering,
    Nat, ReadyRunAuxCarrier)

THEOREM Stage4CapacityOrderingIsWellFounded ==
  IsWellFoundedOn(Stage4CapacityOrdering, Stage4CapacityCarrier)
BY NatLessThanWellFounded, ReadyRunAuxOrderingIsWellFounded,
   WFLexPairOrdering
   DEF Stage4CapacityOrdering, Stage4CapacityCarrier

THEOREM Stage4CapacityRankInCarrier ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant => Stage4CapacityRank(node) \in Stage4CapacityCarrier
BY CausalCommandCapacityDebtIsNatural, ReadyRunAuxRankInCarrier
   DEF Stage4CapacityRank, Stage4CapacityCarrier

Stage4CapacityPending(candidate, position) ==
  /\ ProtectedStage4Pending(candidate, position)
  /\ NonCompletionCausalAdmissionDebt(candidate.node)
  /\ ~ReadyStage4Actionable(candidate)

Stage4CapacityBlockedAtRank(candidate, position, rank) ==
  /\ ProtectedStage4Pending(candidate, position)
  /\ NonCompletionCausalAdmissionDebt(candidate.node)
  /\ ~ReadyStage4Actionable(candidate)
  /\ Stage4CapacityRank(candidate.node) = rank

Stage4CapacityGoal(candidate, position) ==
  \/ ProtectedRankProgressExit(candidate, <<4, position>>)
  \/ ReadyStage4Actionable(candidate)

Stage4CapacityProgress(candidate, position, rank) ==
  \/ Stage4CapacityGoal(candidate, position)
  \/ \E lower \in SetLessThan(
       rank, Stage4CapacityOrdering, Stage4CapacityCarrier):
       Stage4CapacityBlockedAtRank(candidate, position, lower)

Stage4CapacityStrictResult(candidate, position, rank) ==
  Stage4CapacityProgress(candidate, position, rank)'

Stage4CapacityServeEpisodeResidual(candidate, position, rank) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ ProtectedStage4Pending(candidate, position)
  /\ ~Stage4CapacityProgress(candidate, position, rank)
  /\ \/ /\ AsyncIngressSchedulerBarrierActive(candidate.node)
        /\ asyncRunnerPhase[candidate.node] = "Ingress"
     \/ AsyncCandidateProducerContinuationRunnerResolutionRequired(
          candidate.node)

Stage4CapacityCandidateProducerContinuationReentry(
    candidate, position, rank) ==
  /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
       candidate.node)

Stage4CapacityFiniteServeEpisodeResidualProperty(specification) ==
  specification
    => \A candidate, position:
         \A rank \in Stage4CapacityCarrier:
           Stage4CapacityServeEpisodeResidual(candidate, position, rank)
             ~> Stage4CapacityProgress(candidate, position, rank)

Stage4RefinementFiniteServeEpisodeResidualProperty(specification) ==
  /\ Stage4FiniteServeEpisodeResidualProperty(specification)
  /\ Stage4CapacityFiniteServeEpisodeResidualProperty(specification)

Stage4CapacityStepResult(candidate, position, rank) ==
  \/ Stage4CapacityStrictResult(candidate, position, rank)
  \/ Stage4CapacityBlockedAtRank(candidate, position, rank)'
  \/ Stage4CapacityServeEpisodeResidual(candidate, position, rank)'

THEOREM Stage4CapacityBlockedCoreFacts ==
  \A candidate, position, rank:
    Stage4CapacityBlockedAtRank(candidate, position, rank)
      => /\ candidate.node \in ValidatorIds
         /\ AsyncStrongTypeInvariant
         /\ AsyncTypeInvariant
         /\ AsyncProgressOwnershipInvariant
         /\ NonCompletionCausalAdmissionDebt(candidate.node)
PROOF
  <1>1. ASSUME NEW candidate, NEW position, NEW rank,
                Stage4CapacityBlockedAtRank(candidate, position, rank)
         PROVE /\ candidate.node \in ValidatorIds
               /\ AsyncStrongTypeInvariant
               /\ AsyncTypeInvariant
               /\ AsyncProgressOwnershipInvariant
               /\ NonCompletionCausalAdmissionDebt(candidate.node)
    <2>1. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ NonCompletionCausalAdmissionDebt(candidate.node)
           /\ candidate.node \in AsyncCurrentResponsiveVoters
      BY <1>1, Isa
         DEF Stage4CapacityBlockedAtRank, ProtectedStage4Pending,
             ProtectedOwnedAtServiceRank,
             ResponsiveProtectedCandidateOwned
    <2>2. TypeInvariant
      BY <2>1 DEF AsyncStrongTypeInvariant,
                    StrongInductiveInvariant, Safety
    <2>3. AsyncCurrentResponsiveVoters \subseteq ValidatorIds
      BY <2>2, AsyncCurrentResponsiveVotersAreValidators
    <2>4. candidate.node \in ValidatorIds
      BY <2>1, <2>3
    <2>5. AsyncTypeInvariant
      BY <2>1, AsyncStrongTypeProjectsAsyncType
    <2> QED BY <2>1, <2>4, <2>5
  <1> QED BY <1>1

THEOREM Stage4CapacityBlockedStepPreservesCoreInvariants ==
  \A candidate, position, rank:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    => /\ AsyncStrongTypeInvariant'
       /\ AsyncProgressOwnershipInvariant'
PROOF
  <1>1. ASSUME NEW candidate, NEW position, NEW rank,
                Stage4CapacityBlockedAtRank(candidate, position, rank),
                [AsyncNext]_AsyncAllVars
         PROVE /\ AsyncStrongTypeInvariant'
               /\ AsyncProgressOwnershipInvariant'
    <2>1. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
      BY <1>1 DEF Stage4CapacityBlockedAtRank,
                    ProtectedStage4Pending
    <2>2. AsyncStrongTypeInvariant'
      BY <1>1, <2>1, AsyncBracketNextPreservesStrongTypeInvariant
    <2>3. AsyncProgressOwnershipInvariant'
      BY <1>1, <2>1, AsyncBracketNextPreservesProgressOwnership
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM SerializedRuntimeReturnsToLocalWithBudget ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ SerializedRunnerRuntimeStep(node)
    => /\ asyncRunnerPhase'[node] = "Local"
       /\ asyncRunnerBudget'[node] = AsyncQueueCapacity
       /\ AsyncQueueCapacity > 0
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                SerializedRunnerRuntimeStep(node)
         PROVE /\ asyncRunnerPhase'[node] = "Local"
               /\ asyncRunnerBudget'[node] = AsyncQueueCapacity
               /\ AsyncQueueCapacity > 0
    <2>1. AsyncRuntimeScalarTypeInvariant
      BY <1>1 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                    AsyncRuntimeTypeInvariant
    <2>2. /\ DOMAIN asyncRunnerPhase = ValidatorIds
           /\ DOMAIN asyncRunnerBudget = ValidatorIds
           /\ AsyncQueueCapacity > 0
      BY <2>1, Isa
         DEF AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
    <2>3. /\ asyncRunnerPhase' =
                 [asyncRunnerPhase EXCEPT ![node] = "Local"]
           /\ asyncRunnerBudget' =
                 [asyncRunnerBudget EXCEPT
                    ![node] = AsyncQueueCapacity]
      BY <1>1
         DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep
    <2>4. asyncRunnerPhase'[node] = "Local"
      BY <1>1, <2>2, <2>3, FunctionalReplaceUpdateAtKey
    <2>5. asyncRunnerBudget'[node] = AsyncQueueCapacity
      BY <1>1, <2>2, <2>3, FunctionalReplaceUpdateAtKey
    <2> QED BY <2>2, <2>4, <2>5
  <1> QED BY <1>1

THEOREM SerializedRuntimeStage4CapacityRankInCarrier ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ SerializedRunnerRuntimeStep(node)
    => Stage4CapacityRank(node)' \in Stage4CapacityCarrier
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AsyncStrongTypeInvariant',
                SerializedRunnerRuntimeStep(node)
         PROVE Stage4CapacityRank(node)' \in Stage4CapacityCarrier
    <2>1. AsyncTypeInvariant'
      BY <1>1 DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
                    StrongInductiveInvariant, Safety
    <2>2. CausalCommandCapacityDebt(node)' \in Nat
      <3>1. /\ AsyncQueueDepth(node)' \in Nat
             /\ AsyncNormalLimit \in Nat
             /\ AsyncProgressLimit \in Nat
             /\ AsyncQueueCapacity \in Nat
        BY <2>1, LenProperties, SMT
           DEF AsyncQueueDepth, AsyncNormalLimit,
               AsyncProgressLimit, AsyncTypeInvariant,
               AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
               AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped,
               AsyncConfiguration
      <3>2. CausalHeadCommandLimit(node)' \in Nat
        <4>1. CASE HeadCausalCandidate(node).class' = "Normal"
          BY <3>1, <4>1 DEF CausalHeadCommandLimit
        <4>2. CASE HeadCausalCandidate(node).class' = "Progress"
          BY <3>1, <4>2 DEF CausalHeadCommandLimit
        <4>3. CASE /\ HeadCausalCandidate(node).class' # "Normal"
                      /\ HeadCausalCandidate(node).class' # "Progress"
          BY <3>1, <4>3 DEF CausalHeadCommandLimit
        <4> QED BY <4>1, <4>2, <4>3
      <3>3. CASE ~(/\ NonCompletionCausalAdmissionDebt(node)'
                    /\ ~CandidateInFlight(
                         HeadCausalCandidate(node))'
                    /\ ~CanEnqueueClass(
                         node, HeadCausalCandidate(node).class)')
        <4>1. CausalCommandCapacityDebt(node)' = 0
          BY <3>3 DEF CausalCommandCapacityDebt
        <4> QED BY <4>1
      <3>4. CASE /\ NonCompletionCausalAdmissionDebt(node)'
                    /\ ~CandidateInFlight(
                         HeadCausalCandidate(node))'
                    /\ ~CanEnqueueClass(
                         node, HeadCausalCandidate(node).class)'
        <4>1. /\ NonCompletionCausalAdmissionDebt(node)'
               /\ ~CandidateInFlight(HeadCausalCandidate(node))'
               /\ ~CanEnqueueClass(
                    node, HeadCausalCandidate(node).class)'
          BY <3>4
        <4>2. AsyncCandidateTyped(HeadCausalCandidate(node))'
          <5>1. /\ AsyncQueueTyped(asyncCausalQueues'[node])
                 /\ Len(asyncCausalQueues'[node]) > 0
            BY <2>1, <4>1
               DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                   AsyncRuntimeTypeInvariant, AsyncCausalTypeInvariant,
                   NonCompletionCausalAdmissionDebt,
                   CausalAdmissionDebtActive, CausalQueueNonempty
          <5> QED BY <5>1, NonemptyTypedQueueHeadIsTyped
               DEF HeadCausalCandidate
        <4>3. HeadCausalCandidate(node).class'
                   \in {"Normal", "Progress"}
          BY <4>1, <4>2, SMT
             DEF AsyncCandidateTyped, AsyncCommandClasses,
                 NonCompletionCausalAdmissionDebt
        <4>4. CanEnqueueClass(
                  node, HeadCausalCandidate(node).class)'
                  <=> AsyncQueueDepth(node)'
                        < CausalHeadCommandLimit(node)'
          <5>1. CASE HeadCausalCandidate(node).class' = "Normal"
            BY <5>1 DEF CanEnqueueClass, CausalHeadCommandLimit
          <5>2. CASE HeadCausalCandidate(node).class' = "Progress"
            BY <5>2 DEF CanEnqueueClass, CausalHeadCommandLimit
          <5> QED BY <4>3, <5>1, <5>2, SMT
        <4>5. ~(AsyncQueueDepth(node)'
                    < CausalHeadCommandLimit(node)')
          BY <4>1, <4>4
        <4>6. AsyncQueueDepth(node)'
                    - CausalHeadCommandLimit(node)' + 1 \in Nat
          BY <3>1, <3>2, <4>5, SMT
        <4>7. CausalCommandCapacityDebt(node)' =
                 AsyncQueueDepth(node)'
                   - CausalHeadCommandLimit(node)' + 1
          BY <4>1 DEF CausalCommandCapacityDebt
        <4> QED BY <4>6, <4>7
      <3> QED BY <3>3, <3>4
    <2>3. ReadyFifoDebt(node)' \in 0..1
      BY <2>1, Isa
         DEF ReadyFifoDebt, NodeQueueNonempty,
             AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant
    <2>4. ReadyDeferredCount(node)' \in Nat
      BY <2>1, Isa
         DEF ReadyDeferredCount, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncDeferredTypeInvariant,
             AsyncDeferredContentTypeInvariant,
             AsyncQueueTyped, AsyncCompletionSequenceTyped
    <2>5. ReadyTimeoutDebt(node)' \in Nat
      <3>1. asyncNow' \in Nat
        BY <2>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant,
               AsyncRuntimeScalarTypeInvariant
      <3>2. asyncNodeDeadlines' \in [ValidatorIds -> Nat]
        BY <2>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncTransportTypeInvariant,
               AsyncTransportClockTypeInvariant
      <3>3. asyncNodeDeadlines'[node] \in Nat
        BY <1>1, <3>2, Isa
      <3>4. asyncTimeoutEmitted' \in [ValidatorIds -> BOOLEAN]
        BY <2>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant,
               AsyncRuntimeScalarTypeInvariant
      <3>6. asyncOutstandingTags'
                  \in [ValidatorIds -> SUBSET AsyncCompletionTags]
        BY <2>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncTransportTypeInvariant,
               AsyncTransportClockTypeInvariant
      <3>7. asyncOutstandingTags'[node]
                  \subseteq AsyncCompletionTags
        BY <1>1, <3>6, Isa
      <3>8. CASE /\ ~NodeHasDecision(node)'
                    /\ ~NodeTimedOut(node, nodeView[node])'
                    /\ ~asyncTimeoutEmitted'[node]
                    /\ "TimeoutElapsed"
                         \notin asyncOutstandingTags'[node]
                    /\ asyncNow' < asyncNodeDeadlines'[node]
        BY <3>1, <3>3, <3>8, SMT DEF ReadyTimeoutDebt
      <3>9. CASE /\ ~NodeHasDecision(node)'
                    /\ ~NodeTimedOut(node, nodeView[node])'
                    /\ ~asyncTimeoutEmitted'[node]
                    /\ "TimeoutElapsed"
                         \notin asyncOutstandingTags'[node]
                    /\ ~(asyncNow' < asyncNodeDeadlines'[node])
        BY <3>9 DEF ReadyTimeoutDebt
      <3>10. CASE \/ NodeHasDecision(node)'
                    \/ NodeTimedOut(node, nodeView[node])'
                    \/ asyncTimeoutEmitted'[node]
                    \/ "TimeoutElapsed"
                         \in asyncOutstandingTags'[node]
        BY <3>10 DEF ReadyTimeoutDebt
      <3> QED BY <3>8, <3>9, <3>10
    <2>6. ReadyTagDrainDebt(node)' \in Nat
      <3>1. asyncOutstandingTags'
                  \in [ValidatorIds -> SUBSET AsyncCompletionTags]
        BY <2>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncTransportTypeInvariant,
               AsyncTransportClockTypeInvariant
      <3>2. asyncOutstandingTags'[node]
                  \subseteq AsyncCompletionTags
        BY <1>1, <3>1, Isa
      <3>2a. IsFiniteSet(AsyncCompletionTags)
        BY FS_EmptySet, FS_AddElement DEF AsyncCompletionTags
      <3>3. IsFiniteSet(asyncOutstandingTags'[node])
        BY <3>2, <3>2a, FS_Subset
      <3>4. IsFiniteSet(
               asyncOutstandingTags'[node]
                 \cap {"TimeoutElapsed", "RetransmitElapsed"})
        BY <3>3, FS_Intersection
      <3>5. Cardinality(
               asyncOutstandingTags'[node]
                 \cap {"TimeoutElapsed", "RetransmitElapsed"}) \in Nat
        BY <3>4, FS_CardinalityType
      <3>6. ReadyTagCount(node)' =
               Cardinality(
                 asyncOutstandingTags'[node]
                   \cap {"TimeoutElapsed", "RetransmitElapsed"})
        BY DEF ReadyTagCount
      <3>7. ReadyTagCount(node)' \in Nat
        BY <3>5, <3>6
      <3>8. CASE DeferredWorkServiceable(node)'
        BY <3>7, <3>8, SMT DEF ReadyTagDrainDebt
      <3>9. CASE ~DeferredWorkServiceable(node)'
        BY <3>7, <3>9, SMT DEF ReadyTagDrainDebt
      <3> QED BY <3>8, <3>9
    <2>7. RuntimeReachRank(node)' \in Nat
      BY <1>1, <2>1, SerializedRuntimeReturnsToLocalWithBudget,
         Isa
         DEF RuntimeReachRank, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
    <2>8. ReadyRunAuxRank(node)' \in ReadyRunAuxCarrier
      BY <2>3, <2>4, <2>5, <2>6, <2>7, Isa
         DEF ReadyRunAuxRank, ReadyRunDeferredRank,
             ReadyRunTimeoutRank, ReadyRunInnerRank,
             ReadyRunAuxCarrier, ReadyRunDeferredCarrier,
             ReadyRunTimeoutCarrier, ReadyRunInnerCarrier
    <2> QED BY <2>2, <2>8
         DEF Stage4CapacityRank, Stage4CapacityCarrier
  <1> QED BY <1>1

THEOREM SerializedRuntimeKeepsStage4PendingUnlessProgress ==
  \A candidate, position, rank:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ SerializedRunnerRuntimeStep(candidate.node)
    /\ ~ProtectedRankProgressExit(candidate, <<4, position>>)'
    => ProtectedStage4Pending(candidate, position)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position, NEW rank,
                Stage4CapacityBlockedAtRank(candidate, position, rank),
                [AsyncNext]_AsyncAllVars,
                SerializedRunnerRuntimeStep(candidate.node),
                ~ProtectedRankProgressExit(
                   candidate, <<4, position>>)'
         PROVE ProtectedStage4Pending(candidate, position)'
    <2>1. /\ candidate.node \in ValidatorIds
           /\ candidate \in asyncOutstandingWork[candidate.node]
           /\ CandidateInReadyQueue(candidate)
           /\ ReadyCandidatePosition(candidate) = position
      BY <1>1, ProtectedStage4CarrierFacts,
         Stage4CapacityBlockedCoreFacts
         DEF Stage4CapacityBlockedAtRank
    <2>2. /\ AsyncStrongTypeInvariant'
           /\ AsyncProgressOwnershipInvariant'
      BY <1>1, Stage4CapacityBlockedStepPreservesCoreInvariants
    <2>3. gst'
      BY <1>1, GstAsyncStepIsMonotone
         DEF Stage4CapacityBlockedAtRank, ProtectedStage4Pending,
             ProtectedOwnedAtServiceRank
    <2>4. ResponsiveProtectedCandidateOwned(candidate)'
      BY <1>1
         DEF ProtectedRankProgressExit,
             ProtectedServiceOwnershipExit
    <2>5. /\ asyncIoReadyCompletions' = asyncIoReadyCompletions
           /\ asyncLocalReadyCompletions'
                = asyncLocalReadyCompletions
           /\ asyncNextCompletionSource'
                = asyncNextCompletionSource
           /\ asyncCausalAdmissionOwed'
                = asyncCausalAdmissionOwed
           /\ asyncNextLocalSource' = asyncNextLocalSource
      BY <1>1 DEF SerializedRunnerRuntimeStep,
                    SerializedRuntimeStep,
                    SerializedRuntimePrecedesServeIngressStep,
                    AsyncIoVars, AsyncLocalAdmissionVars
    <2>6. /\ CandidateInReadyQueue(candidate)'
           /\ ReadyCandidatePosition(candidate)' = position
      BY <2>1, <2>5, Isa
         DEF CandidateInReadyQueue, ReadyCandidatePosition,
             ReadyCandidateSource, ReadyCompletionQueue,
             SelectedCompletionSource, LocalSourceDistance,
             PreferredLocalSource, SequenceSet
    <2>7. asyncOutstandingWork' = asyncOutstandingWork
      BY <1>1
         DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep, AsyncIoVars
    <2>8. candidate \in asyncOutstandingWork'[candidate.node]
      BY <2>1, <2>7
    <2>9. candidate \in TrackedWorkCandidates'
      BY <2>1, <2>8, Isa DEF TrackedWorkCandidates
    <2>10. candidate \notin QueuedCandidates'
      BY <2>2, <2>9, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>11. candidate \notin DeferredCandidates'
      BY <2>2, <2>9, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>12. CandidateServiceRank(candidate)' = <<4, position>>
      BY <2>6, <2>9, <2>10, <2>11, Isa
         DEF CandidateServiceRank, CandidateInReadyQueue
    <2> QED BY <2>2, <2>3, <2>4, <2>12
         DEF ProtectedStage4Pending, ProtectedOwnedAtServiceRank
  <1> QED BY <1>1

THEOREM Stage4CapacityLocalAdmissionStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ LocalAdmissionStep(candidate.node)
    => Stage4CapacityStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage4CapacityStrictResult, Stage4CapacityProgress,
       Stage4CapacityGoal, Stage4CapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ReadyStage4CausalCapacityBlocked,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       NonCompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       ProducerCompletionCanAdvance, ProducerCompletionCanAdmit,
       RecordBlockedCausalDebt, CandidateInReadyQueue,
       CandidateScheduled, CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueClass, AsyncQueueDepth, NodeQueueNonempty,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacityLocalPredecessorStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ SerializedLocalPrecedesServeIngressStep(candidate.node)
    => Stage4CapacityStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage4CapacityStrictResult, Stage4CapacityProgress,
       Stage4CapacityGoal, Stage4CapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ReadyStage4CausalCapacityBlocked,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       NonCompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, LocalAdmissionCanAdvance,
       ProducerCompletionCanAdvance, ProducerCompletionCanAdmit,
       CandidateInReadyQueue, CandidateScheduled, CandidateInFlight,
       CausalHeadCanAdvance, CanEnqueueClass, AsyncQueueDepth,
       NodeQueueNonempty, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacityIngressStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ IngressDrainStep(candidate.node)
    => Stage4CapacityStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   IngressDrainStrictlyDecreasesRuntimeReach,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage4CapacityStrictResult, Stage4CapacityProgress,
       Stage4CapacityGoal, Stage4CapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ReadyStage4CausalCapacityBlocked,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       NonCompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       IngressDrainStep, DrainFairIngressSelected,
       IngressItemCanDrain, PopSelectedIngress,
       CandidateInReadyQueue, CandidateScheduled, CandidateInFlight,
       CausalHeadCanAdvance, CanEnqueueClass, AsyncQueueDepth,
       NodeQueueNonempty, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacityDeferredDrainStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ SerializedRunnerRuntimeStep(candidate.node)
    /\ DeferredDrainStep(candidate.node)
    => Stage4CapacityStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage4CapacityStrictResult, Stage4CapacityProgress,
       Stage4CapacityGoal, Stage4CapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ReadyStage4CausalCapacityBlocked,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       NonCompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, DeferredDrainStep,
       RemoveNextDeferredCommand, DiscardCommand,
       AdvanceNextDeferredClass, DeferredQueueNonempty,
       AppendCausalSuccessors, FreshCommandSuccessors,
       CandidateInReadyQueue, CandidateScheduled, CandidateInFlight,
       CausalHeadCanAdvance, CanEnqueueClass, AsyncQueueDepth,
       NodeQueueNonempty, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacityDeferredTagStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ SerializedRunnerRuntimeStep(candidate.node)
    /\ DeferredTagStep(candidate.node)
    => Stage4CapacityStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage4CapacityStrictResult, Stage4CapacityProgress,
       Stage4CapacityGoal, Stage4CapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ReadyStage4CausalCapacityBlocked,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       NonCompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, DeferredTagStep,
       DeferredTimeoutStep, DeferredRetransmitStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       CandidateInReadyQueue, CandidateScheduled, CandidateInFlight,
       CausalHeadCanAdvance, CanEnqueueClass, AsyncQueueDepth,
       NodeQueueNonempty, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacityDirectTimeoutStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ SerializedRunnerRuntimeStep(candidate.node)
    /\ DirectTimeoutStep(candidate.node)
    => Stage4CapacityStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage4CapacityStrictResult, Stage4CapacityProgress,
       Stage4CapacityGoal, Stage4CapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ReadyStage4CausalCapacityBlocked,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       NonCompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       DirectTimeoutStep, TimeoutDue,
       AppendCausalSuccessors, FreshCommandSuccessors,
       CandidateInReadyQueue, CandidateScheduled, CandidateInFlight,
       CausalHeadCanAdvance, CanEnqueueClass, AsyncQueueDepth,
       NodeQueueNonempty, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM NonemptyCausalHeadIsCausalCandidate ==
  \A node \in ValidatorIds:
    /\ AsyncCausalTypeInvariant
    /\ CausalQueueNonempty(node)
    => HeadCausalCandidate(node) \in CausalCandidates
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncCausalTypeInvariant,
                CausalQueueNonempty(node)
         PROVE HeadCausalCandidate(node) \in CausalCandidates
    <2>1. /\ AsyncQueueTyped(asyncCausalQueues[node])
           /\ Len(asyncCausalQueues[node]) > 0
      BY <1>1 DEF AsyncCausalTypeInvariant, CausalQueueNonempty
    <2>2. /\ asyncCausalQueues[node]
                  \in Seq(Range(asyncCausalQueues[node]))
           /\ asyncCausalQueues[node] # <<>>
           /\ Head(asyncCausalQueues[node])
                = asyncCausalQueues[node][1]
           /\ 1 \in 1..Len(asyncCausalQueues[node])
      BY <2>1, PositiveSequenceIsNonempty,
         NonemptySequenceHeadIsFirst, SMT
         DEF AsyncQueueTyped
    <2>3. HeadCausalCandidate(node) \in
             SequenceSet(asyncCausalQueues[node])
      BY <2>2 DEF HeadCausalCandidate, SequenceSet
    <2> QED BY <1>1, <2>3, Isa DEF CausalCandidates
  <1> QED BY <1>1

THEOREM OwnedCausalHeadIsNotInFlight ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ CausalQueueNonempty(node)
    => ~CandidateInFlight(HeadCausalCandidate(node))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AsyncProgressOwnershipInvariant,
                CausalQueueNonempty(node)
         PROVE ~CandidateInFlight(HeadCausalCandidate(node))
    <2>1. AsyncCausalTypeInvariant
      BY <1>1 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                    AsyncRuntimeTypeInvariant
    <2>2. HeadCausalCandidate(node) \in CausalCandidates
      BY <1>1, <2>1, NonemptyCausalHeadIsCausalCandidate
    <2>3. HeadCausalCandidate(node) \notin QueuedCandidates
      BY <1>1, <2>2, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>4. HeadCausalCandidate(node) \notin DeferredCandidates
      BY <1>1, <2>2, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2>5. HeadCausalCandidate(node) \notin TrackedWorkCandidates
      BY <1>1, <2>2, Isa
         DEF AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant
    <2> QED BY <2>3, <2>4, <2>5 DEF CandidateInFlight
  <1> QED BY <1>1

THEOREM SerializedFifoRemovesOneCommand ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ SerializedRunnerRuntimeStep(node)
    /\ FifoRuntimeStep(node)
    => AsyncQueueDepth(node)' = AsyncQueueDepth(node) - 1
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                SerializedRunnerRuntimeStep(node),
                FifoRuntimeStep(node)
         PROVE AsyncQueueDepth(node)' = AsyncQueueDepth(node) - 1
    <2> DEFINE Queue == asyncCommandQueues[node]
    <2> DEFINE Index == NextNodeCommandIndex(node)
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncQueueTyped(Queue)
           /\ NodeQueueNonempty(node)
      BY <1>1 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                    AsyncRuntimeTypeInvariant,
                    AsyncRuntimeScalarTypeInvariant,
                    FifoRuntimeStep, Queue
    <2>2. Index \in 1..Len(Queue)
      BY <1>1, <2>1, NextNodeCommandIndexFacts
         DEF Index, Queue
    <2>3. Len(SequenceWithoutIndex(Queue, Index)) = Len(Queue) - 1
      BY <2>1, <2>2, SequenceWithoutIndexFacts
         DEF AsyncQueueTyped
    <2>4. RemoveNextNodeCommand(node)
      BY <1>1, Isa DEF FifoRuntimeStep
    <2>5. asyncCommandQueues' =
             [asyncCommandQueues EXCEPT
                ![node] = SequenceWithoutIndex(
                             asyncCommandQueues[node],
                             NextNodeCommandIndex(node))]
      BY <2>4 DEF RemoveNextNodeCommand
    <2>6. node \in DOMAIN asyncCommandQueues
      BY <1>1, <2>1 DEF AsyncRuntimeScalarTypeInvariant
    <2>7. [asyncCommandQueues EXCEPT
             ![node] = SequenceWithoutIndex(
                          asyncCommandQueues[node],
                          NextNodeCommandIndex(node))][node]
             = SequenceWithoutIndex(
                 asyncCommandQueues[node],
                 NextNodeCommandIndex(node))
      BY <2>6, FunctionalReplaceUpdateAtKey
    <2>8. asyncCommandQueues'[node] =
             SequenceWithoutIndex(Queue, Index)
      BY <2>5, <2>7 DEF Queue, Index
    <2> QED BY <2>3, <2>8 DEF AsyncQueueDepth, Queue
  <1> QED BY <1>1

THEOREM FreshCandidateSequenceFormsSequence ==
  \A candidate:
    FreshCandidateSequence(candidate)
      \in Seq(Range(FreshCandidateSequence(candidate)))
PROOF
  <1>1. ASSUME NEW candidate
         PROVE FreshCandidateSequence(candidate)
                 \in Seq(Range(FreshCandidateSequence(candidate)))
    <2>1. CASE CandidateScheduled(candidate)
      <3>1. FreshCandidateSequence(candidate) = <<>>
        BY <2>1 DEF FreshCandidateSequence
      <3>2. <<>> \in Seq({})
        BY EmptySeq
      <3> QED BY <3>1, <3>2, SeqOfRange
    <2>2. CASE ~CandidateScheduled(candidate)
      <3>1. FreshCandidateSequence(candidate) = <<candidate>>
        BY <2>2 DEF FreshCandidateSequence
      <3>2. <<candidate>> \in Seq({candidate})
        BY SingletonSequenceFacts
      <3> QED BY <3>1, <3>2, SeqOfRange
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM SequencesShareUnionCarrier ==
  \A left, right:
    /\ left \in Seq(Range(left))
    /\ right \in Seq(Range(right))
    => /\ left \in Seq(Range(left) \cup Range(right))
       /\ right \in Seq(Range(left) \cup Range(right))
PROOF
  <1>1. ASSUME NEW left, NEW right,
                left \in Seq(Range(left)),
                right \in Seq(Range(right))
         PROVE /\ left \in Seq(Range(left) \cup Range(right))
               /\ right \in Seq(Range(left) \cup Range(right))
    <2>1. Range(left) \subseteq Range(left) \cup Range(right)
      BY SMT
    <2>2. Range(right) \subseteq Range(left) \cup Range(right)
      BY SMT
    <2>3. left \in Seq(Range(left) \cup Range(right))
      BY <1>1, <2>1, SeqMonotonic
    <2>4. right \in Seq(Range(left) \cup Range(right))
      BY <1>1, <2>2, SeqMonotonic
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM ConcatenatedSequencesFormSequence ==
  \A left, right:
    /\ left \in Seq(Range(left))
    /\ right \in Seq(Range(right))
    => left \o right \in Seq(Range(left \o right))
PROOF
  <1>1. ASSUME NEW left, NEW right,
                left \in Seq(Range(left)),
                right \in Seq(Range(right))
         PROVE left \o right \in Seq(Range(left \o right))
    <2> DEFINE Carrier == Range(left) \cup Range(right)
    <2>1. /\ left \in Seq(Carrier)
           /\ right \in Seq(Carrier)
      BY <1>1, SequencesShareUnionCarrier DEF Carrier
    <2>2. left \o right \in Seq(Carrier)
      BY <2>1, ConcatProperties
    <2> QED BY <2>2, SeqOfRange
  <1> QED BY <1>1

THEOREM FreshCommandSuccessorsFormSequence ==
  \A command:
    FreshCommandSuccessors(command)
      \in Seq(Range(FreshCommandSuccessors(command)))
PROOF
  <1>1. ASSUME NEW command
         PROVE FreshCommandSuccessors(command)
                 \in Seq(Range(FreshCommandSuccessors(command)))
    <2> DEFINE Successors == CommandSuccessors(command)
    <2>1. CASE Len(Successors) = 0
      <3>1. FreshCommandSuccessors(command) = <<>>
        BY <2>1 DEF FreshCommandSuccessors, Successors
      <3>2. <<>> \in Seq({})
        BY EmptySeq
      <3> QED BY <3>1, <3>2, SeqOfRange
    <2>2. CASE Len(Successors) = 1
      <3>1. FreshCommandSuccessors(command) =
               FreshCandidateSequence(Successors[1])
        BY <2>2 DEF FreshCommandSuccessors, Successors
      <3>2. FreshCandidateSequence(Successors[1])
                 \in Seq(Range(FreshCandidateSequence(Successors[1])))
        BY FreshCandidateSequenceFormsSequence
      <3> QED BY <3>1, <3>2
    <2>3. CASE Len(Successors) = 2
      <3>1. FreshCommandSuccessors(command) =
               FreshCandidateSequence(Successors[1])
                 \o FreshCandidateSequence(Successors[2])
        BY <2>3 DEF FreshCommandSuccessors, Successors
      <3>2. /\ FreshCandidateSequence(Successors[1])
                       \in Seq(Range(
                            FreshCandidateSequence(Successors[1])))
             /\ FreshCandidateSequence(Successors[2])
                       \in Seq(Range(
                            FreshCandidateSequence(Successors[2])))
        BY FreshCandidateSequenceFormsSequence
      <3>3. FreshCandidateSequence(Successors[1])
                 \o FreshCandidateSequence(Successors[2])
               \in Seq(Range(
                    FreshCandidateSequence(Successors[1])
                      \o FreshCandidateSequence(Successors[2])))
        BY <3>2, ConcatenatedSequencesFormSequence
      <3> QED BY <3>1, <3>3
    <2>4. CASE Len(Successors) = 3
      <3> DEFINE FirstTwo ==
            FreshCandidateSequence(Successors[1])
              \o FreshCandidateSequence(Successors[2])
      <3>1. FreshCommandSuccessors(command) =
               FirstTwo \o FreshCandidateSequence(Successors[3])
        BY <2>4 DEF FreshCommandSuccessors, Successors, FirstTwo
      <3>2. /\ FreshCandidateSequence(Successors[1])
                       \in Seq(Range(
                            FreshCandidateSequence(Successors[1])))
             /\ FreshCandidateSequence(Successors[2])
                       \in Seq(Range(
                            FreshCandidateSequence(Successors[2])))
             /\ FreshCandidateSequence(Successors[3])
                       \in Seq(Range(
                            FreshCandidateSequence(Successors[3])))
        BY FreshCandidateSequenceFormsSequence
      <3>3. FirstTwo \in Seq(Range(FirstTwo))
        BY <3>2, ConcatenatedSequencesFormSequence DEF FirstTwo
      <3>4. FirstTwo \o FreshCandidateSequence(Successors[3])
               \in Seq(Range(
                    FirstTwo \o FreshCandidateSequence(Successors[3])))
        BY <3>2, <3>3, ConcatenatedSequencesFormSequence
      <3> QED BY <3>1, <3>4
    <2>5. CASE Len(Successors) \notin {0, 1, 2, 3}
      <3>1. FreshCommandSuccessors(command) = <<>>
        BY <2>5, Isa DEF FreshCommandSuccessors, Successors
      <3>2. <<>> \in Seq({})
        BY EmptySeq
      <3> QED BY <3>1, <3>2, SeqOfRange
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, SMT
  <1> QED BY <1>1

THEOREM NonemptySequenceHeadSurvivesConcat ==
  \A prefix, suffix:
    /\ prefix \in Seq(Range(prefix))
    /\ suffix \in Seq(Range(suffix))
    /\ Len(prefix) > 0
    => /\ Len(prefix \o suffix) > 0
       /\ Head(prefix \o suffix) = Head(prefix)
PROOF
  <1>1. ASSUME NEW prefix, NEW suffix,
                prefix \in Seq(Range(prefix)),
                suffix \in Seq(Range(suffix)),
                Len(prefix) > 0
         PROVE /\ Len(prefix \o suffix) > 0
               /\ Head(prefix \o suffix) = Head(prefix)
    <2> DEFINE Carrier == Range(prefix) \cup Range(suffix)
    <2>1. /\ prefix \in Seq(Carrier)
           /\ suffix \in Seq(Carrier)
      BY <1>1, SequencesShareUnionCarrier DEF Carrier
    <2>2. /\ prefix \o suffix \in Seq(Carrier)
           /\ Len(prefix \o suffix) = Len(prefix) + Len(suffix)
           /\ (prefix \o suffix)[1] = prefix[1]
      BY <1>1, <2>1, ConcatProperties, SMT
    <2>3. /\ prefix # <<>>
           /\ Head(prefix) = prefix[1]
      BY <1>1, PositiveSequenceIsNonempty,
         NonemptySequenceHeadIsFirst
    <2>4. /\ Len(prefix \o suffix) > 0
           /\ prefix \o suffix # <<>>
           /\ Head(prefix \o suffix) = (prefix \o suffix)[1]
      BY <1>1, <2>2, PositiveSequenceIsNonempty,
         NonemptySequenceHeadIsFirst, SMT
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM SerializedFifoRetainsExistingCausalHead ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ CausalQueueNonempty(node)
    /\ SerializedRunnerRuntimeStep(node)
    /\ FifoRuntimeStep(node)
    => /\ CausalQueueNonempty(node)'
       /\ HeadCausalCandidate(node)' = HeadCausalCandidate(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                CausalQueueNonempty(node),
                SerializedRunnerRuntimeStep(node),
                FifoRuntimeStep(node)
         PROVE /\ CausalQueueNonempty(node)'
               /\ HeadCausalCandidate(node)'
                    = HeadCausalCandidate(node)
    <2> DEFINE Queue == asyncCausalQueues[node]
    <2> DEFINE Command == NextNodeCommand(node)
    <2> DEFINE Successors == FreshCommandSuccessors(Command)
    <2>1. /\ AsyncCausalTypeInvariant
           /\ AsyncQueueTyped(Queue)
           /\ Len(Queue) > 0
           /\ NodeQueueNonempty(node)
      BY <1>1 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                    AsyncRuntimeTypeInvariant,
                    AsyncCausalTypeInvariant, CausalQueueNonempty,
                    FifoRuntimeStep, Queue
    <2>2. /\ AsyncCandidateTyped(Command)
           /\ Command.node = node
      BY <1>1, <2>1, RuntimeSelectedCommandsAreTyped DEF Command
    <2>3. /\ Queue \in Seq(Range(Queue))
           /\ Successors \in Seq(Range(Successors))
           /\ Len(Queue \o Successors) > 0
           /\ Head(Queue \o Successors) = Head(Queue)
      BY <2>1, FreshCommandSuccessorsFormSequence,
         NonemptySequenceHeadSurvivesConcat
         DEF AsyncQueueTyped, Successors
    <2>4. CASE CommandDispatchable(Command)
      <3>1. AppendCausalSuccessors(Command)
        BY <1>1, <2>4, Isa DEF FifoRuntimeStep, Command
      <3>2. asyncCausalQueues' =
               [asyncCausalQueues EXCEPT
                  ![node] = @ \o Successors]
        BY <2>2, <3>1
           DEF AppendCausalSuccessors, Command, Successors
      <3>3. node \in DOMAIN asyncCausalQueues
        BY <1>1, <2>1 DEF AsyncCausalTypeInvariant
      <3>4. [asyncCausalQueues EXCEPT
               ![node] = @ \o Successors][node]
               = Queue \o Successors
        BY <3>3, FunctionalConcatUpdateAtKey DEF Queue
      <3>5. asyncCausalQueues'[node] = Queue \o Successors
        BY <3>2, <3>4
      <3> QED BY <2>3, <3>5
           DEF CausalQueueNonempty, HeadCausalCandidate, Queue
    <2>5. CASE ~CommandDispatchable(Command)
      <3>1. LeaveCausalQueues
        <4>1. CASE ~NodeIdle(node)
          BY <1>1, <2>5, <4>1, Isa
             DEF FifoRuntimeStep, Command
        <4>2. CASE NodeIdle(node)
          BY <1>1, <2>5, <4>2, Isa
             DEF FifoRuntimeStep, Command
        <4> QED BY <4>1, <4>2
      <3>2. asyncCausalQueues' = asyncCausalQueues
        BY <3>1 DEF LeaveCausalQueues
      <3> QED BY <1>1, <3>2
           DEF CausalQueueNonempty, HeadCausalCandidate
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM NonCompletionCausalHeadCapacityFacts ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ NonCompletionCausalAdmissionDebt(node)
    /\ FifoRuntimeStep(node)
    => /\ AsyncQueueDepth(node) \in Nat \ {0}
       /\ CausalHeadCommandLimit(node) \in Nat \ {0}
       /\ HeadCausalCandidate(node).class \in {"Normal", "Progress"}
       /\ (CanEnqueueClass(
              node, HeadCausalCandidate(node).class)
             <=> AsyncQueueDepth(node)
                   < CausalHeadCommandLimit(node))
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                NonCompletionCausalAdmissionDebt(node),
                FifoRuntimeStep(node)
         PROVE /\ AsyncQueueDepth(node) \in Nat \ {0}
               /\ CausalHeadCommandLimit(node) \in Nat \ {0}
               /\ HeadCausalCandidate(node).class
                    \in {"Normal", "Progress"}
               /\ (CanEnqueueClass(
                      node, HeadCausalCandidate(node).class)
                     <=> AsyncQueueDepth(node)
                           < CausalHeadCommandLimit(node))
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ CausalQueueNonempty(node)
           /\ NodeQueueNonempty(node)
      BY <1>1 DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                    AsyncRuntimeTypeInvariant,
                    NonCompletionCausalAdmissionDebt,
                    CausalAdmissionDebtActive, FifoRuntimeStep
    <2>2. AsyncCandidateTyped(HeadCausalCandidate(node))
      BY <1>1, <2>1, CausalHeadCandidateIsTyped
    <2>3. AsyncQueueDepth(node) \in Nat \ {0}
      BY <2>1, Isa
         DEF AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped,
             AsyncQueueDepth, NodeQueueNonempty
    <2>4. HeadCausalCandidate(node).class
             \in {"Normal", "Progress"}
      BY <1>1, <2>2, SMT
         DEF AsyncCandidateTyped, AsyncCommandClasses,
             NonCompletionCausalAdmissionDebt
    <2>5. /\ AsyncNormalLimit \in Nat \ {0}
           /\ AsyncProgressLimit \in Nat \ {0}
      BY <2>1, SMT
         DEF AsyncRuntimeScalarTypeInvariant, AsyncConfiguration,
             AsyncNormalLimit, AsyncProgressLimit
    <2>6. CausalHeadCommandLimit(node) \in Nat \ {0}
      BY <2>4, <2>5, Isa DEF CausalHeadCommandLimit
    <2>7. CanEnqueueClass(
             node, HeadCausalCandidate(node).class)
             <=> AsyncQueueDepth(node) < CausalHeadCommandLimit(node)
      <3>1. CASE HeadCausalCandidate(node).class = "Normal"
        BY <3>1 DEF CanEnqueueClass, CausalHeadCommandLimit
      <3>2. CASE HeadCausalCandidate(node).class = "Progress"
        BY <3>2 DEF CanEnqueueClass, CausalHeadCommandLimit
      <3> QED BY <2>4, <3>1, <3>2, SMT
    <2> QED BY <2>3, <2>4, <2>6, <2>7
  <1> QED BY <1>1

THEOREM SerializedFifoRetainsNonCompletionCausalDebt ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ NonCompletionCausalAdmissionDebt(node)
    /\ SerializedRunnerRuntimeStep(node)
    /\ FifoRuntimeStep(node)
    => /\ NonCompletionCausalAdmissionDebt(node)'
       /\ CausalHeadCommandLimit(node)'
            = CausalHeadCommandLimit(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                NonCompletionCausalAdmissionDebt(node),
                SerializedRunnerRuntimeStep(node),
                FifoRuntimeStep(node)
         PROVE /\ NonCompletionCausalAdmissionDebt(node)'
               /\ CausalHeadCommandLimit(node)'
                    = CausalHeadCommandLimit(node)
    <2>1. /\ asyncCausalAdmissionOwed[node]
           /\ CausalQueueNonempty(node)
           /\ HeadCausalCandidate(node).class # "Completion"
      BY <1>1 DEF NonCompletionCausalAdmissionDebt,
                    CausalAdmissionDebtActive
    <2>2. /\ CausalQueueNonempty(node)'
           /\ HeadCausalCandidate(node)'
                = HeadCausalCandidate(node)
      BY <1>1, <2>1, SerializedFifoRetainsExistingCausalHead
    <2>3. asyncCausalAdmissionOwed' = asyncCausalAdmissionOwed
      BY <1>1
         DEF SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             AsyncLocalAdmissionVars
    <2> QED BY <2>1, <2>2, <2>3, Isa
         DEF NonCompletionCausalAdmissionDebt,
             CausalAdmissionDebtActive, CausalHeadCommandLimit
  <1> QED BY <1>1

THEOREM NaturalCapacityDebtDropsAfterRemoval ==
  \A oldDepth, newDepth, limit \in Nat:
    /\ oldDepth > 0
    /\ newDepth = oldDepth - 1
    /\ ~(newDepth < limit)
    => newDepth - limit + 1 < oldDepth - limit + 1
BY SMT

THEOREM NonCompletionDebtFifoCapacityProgress ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ NonCompletionCausalAdmissionDebt(node)
    /\ SerializedRunnerRuntimeStep(node)
    /\ FifoRuntimeStep(node)
    => \/ CausalHeadCanAdvance(node)'
       \/ CausalCommandCapacityDebt(node)'
            < CausalCommandCapacityDebt(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                AsyncProgressOwnershipInvariant,
                NonCompletionCausalAdmissionDebt(node),
                SerializedRunnerRuntimeStep(node),
                FifoRuntimeStep(node)
         PROVE \/ CausalHeadCanAdvance(node)'
               \/ CausalCommandCapacityDebt(node)'
                    < CausalCommandCapacityDebt(node)
    <2>1. /\ AsyncQueueDepth(node) \in Nat \ {0}
           /\ CausalHeadCommandLimit(node) \in Nat \ {0}
           /\ HeadCausalCandidate(node).class
                \in {"Normal", "Progress"}
           /\ (CanEnqueueClass(
                  node, HeadCausalCandidate(node).class)
                 <=> AsyncQueueDepth(node)
                       < CausalHeadCommandLimit(node))
      BY <1>1, NonCompletionCausalHeadCapacityFacts
    <2>2. ~CandidateInFlight(HeadCausalCandidate(node))
      BY <1>1, OwnedCausalHeadIsNotInFlight
         DEF NonCompletionCausalAdmissionDebt,
             CausalAdmissionDebtActive
    <2>3. AsyncQueueDepth(node)' = AsyncQueueDepth(node) - 1
      BY <1>1, SerializedFifoRemovesOneCommand
    <2>4. /\ NonCompletionCausalAdmissionDebt(node)'
           /\ CausalHeadCommandLimit(node)'
                = CausalHeadCommandLimit(node)
      BY <1>1, SerializedFifoRetainsNonCompletionCausalDebt
    <2>5. /\ CausalQueueNonempty(node)'
           /\ HeadCausalCandidate(node)'
                = HeadCausalCandidate(node)
      BY <1>1, SerializedFifoRetainsExistingCausalHead
         DEF NonCompletionCausalAdmissionDebt,
             CausalAdmissionDebtActive
    <2>6. /\ NonCompletionCausalAdmissionDebt(node)'
           /\ CausalHeadCommandLimit(node)'
                = CausalHeadCommandLimit(node)
           /\ CausalQueueNonempty(node)'
           /\ HeadCausalCandidate(node)'
                = HeadCausalCandidate(node)
      BY <2>4, <2>5
    <2>7. CASE CandidateInFlight(HeadCausalCandidate(node))'
      BY <2>6, <2>7
         DEF CausalHeadCanAdvance
    <2>8. CASE ~CandidateInFlight(HeadCausalCandidate(node))'
      <3>1. CASE CanEnqueueClass(
                    node, HeadCausalCandidate(node).class)'
        BY <2>1, <2>6, <3>1, Isa
           DEF CausalHeadCanAdvance
      <3>2. CASE ~CanEnqueueClass(
                    node, HeadCausalCandidate(node).class)'
        <4>1. /\ ~CanEnqueueClass(
                       node, HeadCausalCandidate(node).class)
               /\ AsyncQueueDepth(node)' \in Nat
               /\ ~(AsyncQueueDepth(node)'
                      < CausalHeadCommandLimit(node))
          BY <2>1, <2>3, <2>6, <3>2, Isa
             DEF CanEnqueueClass, AsyncQueueDepth,
                 CausalHeadCommandLimit
        <4>2. CausalCommandCapacityDebt(node) =
                 AsyncQueueDepth(node)
                   - CausalHeadCommandLimit(node) + 1
          BY <1>1, <2>2, <4>1
             DEF CausalCommandCapacityDebt
        <4>3. CausalCommandCapacityDebt(node)' =
                 AsyncQueueDepth(node)'
                   - CausalHeadCommandLimit(node) + 1
          BY <2>6, <2>8, <3>2
             DEF CausalCommandCapacityDebt
        <4>4. AsyncQueueDepth(node)'
                   - CausalHeadCommandLimit(node) + 1
                 < AsyncQueueDepth(node)
                     - CausalHeadCommandLimit(node) + 1
          BY <2>1, <2>3, <4>1,
             NaturalCapacityDebtDropsAfterRemoval
        <4> QED BY <4>2, <4>3, <4>4
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>7, <2>8
  <1> QED BY <1>1

THEOREM Stage4CapacityFifoStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ SerializedRunnerRuntimeStep(candidate.node)
    /\ FifoRuntimeStep(candidate.node)
    => Stage4CapacityStrictResult(candidate, position, rank)
PROOF
  <1>1. ASSUME NEW candidate,
                NEW position,
                NEW rank \in Stage4CapacityCarrier,
                Stage4CapacityBlockedAtRank(candidate, position, rank),
                [AsyncNext]_AsyncAllVars,
                SerializedRunnerRuntimeStep(candidate.node),
                FifoRuntimeStep(candidate.node)
         PROVE Stage4CapacityStrictResult(candidate, position, rank)
    <2>1. /\ candidate.node \in ValidatorIds
           /\ AsyncStrongTypeInvariant
           /\ AsyncTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ NonCompletionCausalAdmissionDebt(candidate.node)
      BY <1>1, Stage4CapacityBlockedCoreFacts
    <2>2. \/ CausalHeadCanAdvance(candidate.node)'
             \/ CausalCommandCapacityDebt(candidate.node)'
                  < CausalCommandCapacityDebt(candidate.node)
      BY <1>1, <2>1, NonCompletionDebtFifoCapacityProgress
    <2>3. /\ AsyncStrongTypeInvariant'
           /\ AsyncProgressOwnershipInvariant'
      BY <1>1, Stage4CapacityBlockedStepPreservesCoreInvariants
    <2>4. CASE CausalHeadCanAdvance(candidate.node)'
      <3>1. ReadyStage4Actionable(candidate)'
        BY <1>1, <2>1, <2>4,
           SerializedRuntimeReturnsToLocalWithBudget, Isa
           DEF ReadyStage4Actionable, LocalAdmissionCanAdvance,
               SerializedRunnerRuntimeStep, SerializedRuntimeStep,
               SerializedRuntimePrecedesServeIngressStep
      <3> QED BY <3>1
           DEF Stage4CapacityStrictResult, Stage4CapacityProgress,
               Stage4CapacityGoal
    <2>5. CASE ~CausalHeadCanAdvance(candidate.node)'
      <3>1. CausalCommandCapacityDebt(candidate.node)'
               < CausalCommandCapacityDebt(candidate.node)
        BY <2>2, <2>5
      <3>2. CASE ProtectedRankProgressExit(
                    candidate, <<4, position>>)'
        BY <3>2
           DEF Stage4CapacityStrictResult, Stage4CapacityProgress,
               Stage4CapacityGoal
      <3>3. CASE ~ProtectedRankProgressExit(
                    candidate, <<4, position>>)'
        <4>1. Stage4CapacityRank(candidate.node)'
                 \in Stage4CapacityCarrier
          BY <1>1, <2>1, <2>3,
             SerializedRuntimeStage4CapacityRankInCarrier
        <4>2. PICK lower \in Stage4CapacityCarrier:
                 lower = Stage4CapacityRank(candidate.node)'
          BY <4>1
        <4>3. <<lower, rank>>
                 \in Stage4CapacityOrdering
          BY <1>1, <3>1, <4>2, Isa
             DEF Stage4CapacityBlockedAtRank, Stage4CapacityRank,
                 Stage4CapacityOrdering, Stage4CapacityCarrier,
                 ReadyRunAuxCarrier, ReadyRunDeferredCarrier,
                 ReadyRunTimeoutCarrier, ReadyRunInnerCarrier,
                 LexPairOrdering, OpToRel
        <4>4. lower
                 \in SetLessThan(
                      rank, Stage4CapacityOrdering,
                      Stage4CapacityCarrier)
          BY <4>2, <4>3 DEF SetLessThan
        <4>5. NonCompletionCausalAdmissionDebt(candidate.node)'
          BY <1>1, <2>1,
             SerializedFifoRetainsNonCompletionCausalDebt
        <4>6. ~ReadyStage4Actionable(candidate)'
          BY <1>1, <2>5, <4>5,
             SerializedRuntimeReturnsToLocalWithBudget, Isa
             DEF ReadyStage4Actionable, LocalAdmissionCanAdvance,
                 ProducerCompletionCanAdvance
        <4>7. ProtectedStage4Pending(candidate, position)'
          BY <1>1, <3>3,
             SerializedRuntimeKeepsStage4PendingUnlessProgress
        <4>8. Stage4CapacityBlockedAtRank(
                 candidate, position, lower)'
          BY <4>2, <4>5, <4>6, <4>7
             DEF Stage4CapacityBlockedAtRank
        <4> QED BY <4>4, <4>8
             DEF Stage4CapacityStrictResult,
                 Stage4CapacityProgress
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>4, <2>5
  <1> QED BY <1>1

THEOREM Stage4CapacityRetransmitStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ SerializedRunnerRuntimeStep(candidate.node)
    /\ DirectRetransmitStep(candidate.node)
    /\ ~(NodeQueueNonempty(candidate.node)
           /\ asyncFifoOwed[candidate.node])
    => Stage4CapacityStrictResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage4CapacityStrictResult, Stage4CapacityProgress,
       Stage4CapacityGoal, Stage4CapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ReadyStage4CausalCapacityBlocked,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       NonCompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       DirectRetransmitStep,
       CandidateInReadyQueue, CandidateScheduled, CandidateInFlight,
       CausalHeadCanAdvance, CanEnqueueClass, AsyncQueueDepth,
       NodeQueueNonempty, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacityIdleRuntimeIsImpossible ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ IdleRuntimeStep(candidate.node)
    /\ ~NodeQueueNonempty(candidate.node)
    => FALSE
BY Isa
   DEF Stage4CapacityBlockedAtRank,
       ReadyStage4CausalCapacityBlocked,
       NonCompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive, CausalHeadCanAdvance,
       CandidateInFlight, CanEnqueueClass, AsyncQueueDepth,
       AsyncNormalLimit, AsyncProgressLimit, NodeQueueNonempty,
       ProtectedStage4Pending, AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration, IdleRuntimeStep

THEOREM Stage4CapacitySerializedRuntimeStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ SerializedRunnerRuntimeStep(candidate.node)
    => Stage4CapacityStrictResult(candidate, position, rank)
PROOF
  <1>1. ASSUME NEW candidate,
                NEW position,
                NEW rank \in Stage4CapacityCarrier,
                Stage4CapacityBlockedAtRank(candidate, position, rank),
                SerializedRunnerRuntimeStep(candidate.node)
         PROVE Stage4CapacityStrictResult(candidate, position, rank)
    <2>1. RuntimeStep(candidate.node)
      BY <1>1
         DEF SerializedRunnerRuntimeStep,
             SerializedRunnerRuntimeStep, SerializedRuntimeStep,
             SerializedRuntimePrecedesServeIngressStep,
             SerializedRuntimePrecedesServeIngressStep
    <2>2. CASE DeferredDrainStep(candidate.node)
      BY <1>1, <2>2, Stage4CapacityDeferredDrainStrictlyProgresses
    <2>3. CASE DeferredTagStep(candidate.node)
      BY <1>1, <2>3, Stage4CapacityDeferredTagStrictlyProgresses
    <2>4. CASE DirectTimeoutStep(candidate.node)
      BY <1>1, <2>4, Stage4CapacityDirectTimeoutStrictlyProgresses
    <2>5. CASE FifoRuntimeStep(candidate.node)
      BY <1>1, <2>5, Stage4CapacityFifoStrictlyProgresses
    <2>6. CASE /\ DirectRetransmitStep(candidate.node)
                 /\ ~(NodeQueueNonempty(candidate.node)
                       /\ asyncFifoOwed[candidate.node])
      BY <1>1, <2>6, Stage4CapacityRetransmitStrictlyProgresses
    <2>7. CASE /\ IdleRuntimeStep(candidate.node)
                 /\ ~NodeQueueNonempty(candidate.node)
      BY <1>1, <2>7, Stage4CapacityIdleRuntimeIsImpossible
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7
         DEF RuntimeStep
  <1> QED BY <1>1

THEOREM Stage4CapacityTargetOnlyCreatesServeEpisodeOutcome ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ PostGstRunNode(candidate.node)
    /\ AsyncServeIngressTargetOnlyTurn(candidate.node)
    => \/ Stage4CapacityStrictResult(candidate, position, rank)
       \/ Stage4CapacityServeEpisodeResidual(candidate, position, rank)'
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   Stage4CapacityRankInCarrier, IsaT(300)
   DEF Stage4CapacityStrictResult, Stage4CapacityProgress,
       Stage4CapacityServeEpisodeResidual,
       Stage4CapacityBlockedAtRank, Stage4CapacityRank,
       Stage4CapacityOrdering, Stage4CapacityCarrier,
       Stage4CapacityGoal, ReadyStage4CausalCapacityBlocked,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       PostGstRunNode, RunNode,
       AsyncServeIngressTargetOnlyTurn,
       NonCompletionCausalAdmissionDebt, CausalAdmissionDebtActive,
       CausalHeadCanAdvance, CandidateInReadyQueue,
       CandidateScheduled, CandidateInFlight, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacitySameNodeRunProducesOutcome ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ PostGstRunNode(candidate.node)
    => \/ Stage4CapacityStrictResult(candidate, position, rank)
       \/ Stage4CapacityServeEpisodeResidual(candidate, position, rank)'
       \/ /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(
                candidate.node)
          /\ Stage4CapacityCandidateProducerContinuationReentry(
               candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW candidate,
                NEW position,
                NEW rank \in Stage4CapacityCarrier,
                Stage4CapacityBlockedAtRank(candidate, position, rank),
                PostGstRunNode(candidate.node)
         PROVE \/ Stage4CapacityStrictResult(candidate, position, rank)
               \/ Stage4CapacityServeEpisodeResidual(
                    candidate, position, rank)'
               \/ /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(
                        candidate.node)
                  /\ Stage4CapacityCandidateProducerContinuationReentry(
                       candidate, position, rank)'
    <2>1. RunNode(candidate.node)
      BY <1>1 DEF PostGstRunNode
    <2>1c. CASE
              \/ ResolveRunNodeCandidateProducerContinuation(candidate.node)
              \/ ReplayRunNodeCandidateProducerContinuation(candidate.node)
      BY <1>1, <2>1c,
         AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership,
         Stage4CapacityRankInCarrier, HeadTailProperties,
         SequenceSetAfterAppend, IsaT(900)
         DEF Stage4CapacityCandidateProducerContinuationReentry,
             Stage4CapacityStrictResult, Stage4CapacityProgress,
             Stage4CapacityServeEpisodeResidual,
             Stage4CapacityBlockedAtRank, Stage4CapacityRank,
             Stage4CapacityOrdering, Stage4CapacityCarrier,
             Stage4CapacityGoal, ReadyStage4CausalCapacityBlocked,
             ProtectedStage4Pending, ReadyStage4Actionable,
             ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
             ProtectedServiceOwnershipExit,
             ResponsiveProtectedCandidateOwned,
             ProtectedCandidateOwned, CandidateServiceRank,
             ServiceRankLess, CausalCommandCapacityDebt,
             CausalHeadCommandLimit, ReadyRunAuxRank,
             ResolveRunNodeCandidateProducerContinuation,
             ReplayRunNodeCandidateProducerContinuation,
             AsyncCandidateProducerContinuationExactLocalReplayStep,
             AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
             AsyncCandidateProducerContinuationExactRuntimeReplayStep,
             EnqueueCandidate, CandidateScheduled,
             CandidateInFlight, SequenceSet,
             AsyncProgressOwnershipInvariant,
             AsyncLogicalCandidateOwnershipInvariant,
             AsyncOutstandingCarrierInvariant, AsyncAllVars
    <2>2. CASE LocalAdmissionStep(candidate.node)
      BY <1>1, <2>2, Stage4CapacityLocalAdmissionStrictlyProgresses
    <2>3. CASE IngressDrainStep(candidate.node)
      BY <1>1, <2>3, Stage4CapacityIngressStrictlyProgresses
    <2>4. CASE SerializedRunnerRuntimeStep(candidate.node)
      BY <1>1, <2>4,
         Stage4CapacitySerializedRuntimeStrictlyProgresses
    <2>5. CASE AsyncServeIngressTargetOnlyTurn(candidate.node)
      BY <1>1, <2>5,
         Stage4CapacityTargetOnlyCreatesServeEpisodeOutcome
    <2>6. CASE SerializedLocalPrecedesServeIngressStep(candidate.node)
      BY <1>1, <2>6,
         Stage4CapacityLocalPredecessorStrictlyProgresses
    <2> QED BY <2>1, <2>1c, <2>2, <2>3, <2>4, <2>5, <2>6,
         RunNodeWorkConcreteActionCaseSplit
         DEF RunNode
  <1> QED BY <1>1

THEOREM Stage4CapacityOtherRunnerPreservesOrProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ \/ \E node \in AsyncCurrentResponsiveVoters:
              /\ node # candidate.node
              /\ RunNode(node)
       \/ \E node \in AsyncResponsiveAppliedArchiveServers:
              RunHistoricalServer(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              /\ node # candidate.node
              /\ RunHistoricalRecoveryNode(node)
    => Stage4CapacityStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4CapacityStepResult, Stage4CapacityStrictResult,
       Stage4CapacityProgress, Stage4CapacityGoal,
       Stage4CapacityBlockedAtRank, Stage4CapacityRank,
       ReadyStage4CausalCapacityBlocked, ProtectedStage4Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CausalCommandCapacityDebt,
       CausalHeadCommandLimit, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank, ReadyRunInnerRank,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RunHistoricalServer, CandidateInReadyQueue,
       CandidateScheduled, CandidateInFlight, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacityClockPreservesOrProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ AsyncTick
    => Stage4CapacityStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4CapacityStepResult, Stage4CapacityStrictResult,
       Stage4CapacityProgress, Stage4CapacityGoal,
       Stage4CapacityBlockedAtRank, Stage4CapacityRank,
       Stage4CapacityOrdering, Stage4CapacityCarrier,
       ReadyStage4CausalCapacityBlocked, ProtectedStage4Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCommandCapacityDebt, CausalHeadCommandLimit,
       ReadyRunAuxRank, ReadyRunDeferredRank, ReadyRunTimeoutRank,
       ReadyRunInnerRank, ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       AsyncTick, AsyncNonClockVars, CandidateInReadyQueue,
       CandidateScheduled, CandidateInFlight, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacityDiscoveryPreservesOrProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ \/ \E node \in AsyncCurrentResponsiveVoters:
              DirectCommitCertificateDiscoveryStep(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              DirectHistoricalCommitCertificateDiscoveryStep(node)
    => Stage4CapacityStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4CapacityStepResult, Stage4CapacityStrictResult,
       Stage4CapacityProgress, Stage4CapacityGoal,
       Stage4CapacityBlockedAtRank, Stage4CapacityRank,
       ReadyStage4CausalCapacityBlocked, ProtectedStage4Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CausalCommandCapacityDebt,
       CausalHeadCommandLimit, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank, ReadyRunInnerRank,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork, CandidateInReadyQueue,
       CandidateScheduled, CandidateInFlight, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacityIoPreservesOrProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ \/ \E node \in AsyncArchiveIoServiceNodes:
              ServiceIoWorker(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              ServiceHistoricalRecoveryIoWorker(node)
       \/ \E node \in AsyncCurrentResponsiveVoters:
              EnqueueIoLocalControl(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              EnqueueHistoricalRecoveryIoLocalControl(node)
    => Stage4CapacityStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HeadTailProperties, SequenceSetAfterAppend, Isa
   DEF Stage4CapacityStepResult, Stage4CapacityStrictResult,
       Stage4CapacityProgress, Stage4CapacityGoal,
       Stage4CapacityBlockedAtRank, Stage4CapacityRank,
       ReadyStage4CausalCapacityBlocked, ProtectedStage4Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CausalCommandCapacityDebt,
       CausalHeadCommandLimit, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyCandidatePosition, ReadyCandidateSource,
       ReadyCompletionQueue, CandidateSequenceIndex,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       CandidateInReadyQueue, CandidateScheduled, CandidateInFlight,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacityNetworkOrFaultPreservesOrProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ (AsyncNetworkStep \/ AsyncFaultStep)
    => Stage4CapacityStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4CapacityStepResult, Stage4CapacityStrictResult,
       Stage4CapacityProgress, Stage4CapacityGoal,
       Stage4CapacityBlockedAtRank, Stage4CapacityRank,
       ReadyStage4CausalCapacityBlocked, ProtectedStage4Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CausalCommandCapacityDebt,
       CausalHeadCommandLimit, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank, ReadyRunInnerRank,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       PreGstCrash, CandidateInReadyQueue, CandidateScheduled,
       CandidateInFlight, SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4CapacityOpenHistoricalRecoveryPreservesOrProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ \E node \in ValidatorIds: OpenHistoricalRecovery(node)
    => Stage4CapacityStepResult(candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4CapacityStepResult, Stage4CapacityStrictResult,
       Stage4CapacityProgress, Stage4CapacityGoal,
       Stage4CapacityBlockedAtRank, Stage4CapacityRank,
       ReadyStage4CausalCapacityBlocked, ProtectedStage4Pending,
       ReadyStage4Actionable, ProtectedOwnedAtServiceRank,
       ProtectedRankProgressExit, ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CausalCommandCapacityDebt,
       CausalHeadCommandLimit, OpenHistoricalRecovery, AsyncAllVars

THEOREM Stage4CapacityStutterPreserves ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ UNCHANGED AsyncAllVars
    => Stage4CapacityBlockedAtRank(candidate, position, rank)'
BY Isa
   DEF Stage4CapacityBlockedAtRank, Stage4CapacityRank,
       ReadyStage4CausalCapacityBlocked, ProtectedStage4Pending,
       ProtectedOwnedAtServiceRank, CausalCommandCapacityDebt,
       CausalHeadCommandLimit, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank, ReadyRunInnerRank,
       AsyncAllVars, AsyncSchedulerVars, vars

THEOREM Stage4CapacityBlockedStep ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage4CapacityBlockedAtRank(candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    => Stage4CapacityStepResult(candidate, position, rank)
PROOF
  <1>1. ASSUME NEW candidate,
                NEW position,
                NEW rank \in Stage4CapacityCarrier,
                Stage4CapacityBlockedAtRank(candidate, position, rank),
                [AsyncNext]_AsyncAllVars
         PROVE Stage4CapacityStepResult(candidate, position, rank)
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>1, Stage4CapacityStutterPreserves
         DEF Stage4CapacityStepResult
    <2>2. CASE AsyncNext
      <3>1. CASE \E node \in AsyncCurrentResponsiveVoters:
                    RunNode(node)
        <4>1. CASE RunNode(candidate.node)
          BY <1>1, <2>2, <3>1, <4>1,
             Stage4CapacitySameNodeRunProducesOutcome
             DEF Stage4CapacityStepResult, PostGstRunNode,
                 Stage4CapacityCandidateProducerContinuationReentry,
                 Stage4CapacityBlockedAtRank,
                 ReadyStage4CausalCapacityBlocked,
                 ProtectedStage4Pending, ProtectedOwnedAtServiceRank
        <4>2. CASE ~RunNode(candidate.node)
          BY <1>1, <3>1, <4>2,
             Stage4CapacityOtherRunnerPreservesOrProgresses
        <4> QED BY <4>1, <4>2
      <3>2. CASE \E node \in AsyncResponsiveAppliedArchiveServers:
                    RunHistoricalServer(node)
        BY <1>1, <3>2,
           Stage4CapacityOtherRunnerPreservesOrProgresses
      <3>3. CASE \E node \in asyncHistoricalRecoveryTargets:
                    RunHistoricalRecoveryNode(node)
        <4>1. CASE RunNode(candidate.node)
          BY <1>1, <2>2, <3>3, <4>1,
             Stage4CapacitySameNodeRunProducesOutcome
             DEF Stage4CapacityStepResult, PostGstRunNode,
                 Stage4CapacityCandidateProducerContinuationReentry,
                 Stage4CapacityBlockedAtRank,
                 ReadyStage4CausalCapacityBlocked,
                 ProtectedStage4Pending, ProtectedOwnedAtServiceRank
        <4>2. CASE ~RunNode(candidate.node)
          BY <1>1, <3>3, <4>2,
             Stage4CapacityOtherRunnerPreservesOrProgresses
             DEF RunNode, RunHistoricalRecoveryNode
        <4> QED BY <4>1, <4>2
      <3>4. CASE AsyncTick
        BY <1>1, <3>4, Stage4CapacityClockPreservesOrProgresses
      <3>5. CASE \E node \in ValidatorIds:
                    OpenHistoricalRecovery(node)
        BY <1>1, <3>5,
           Stage4CapacityOpenHistoricalRecoveryPreservesOrProgresses
      <3>6. CASE \/ \E node \in AsyncCurrentResponsiveVoters:
                          DirectCommitCertificateDiscoveryStep(node)
                   \/ \E historicalNode \in asyncHistoricalRecoveryTargets:
                          DirectHistoricalCommitCertificateDiscoveryStep(
                            historicalNode)
        BY <1>1, <3>6, Stage4CapacityDiscoveryPreservesOrProgresses
      <3>7. CASE \/ \E ioNode \in AsyncArchiveIoServiceNodes:
                          ServiceIoWorker(ioNode)
                   \/ \E historicalIoNode \in asyncHistoricalRecoveryTargets:
                          ServiceHistoricalRecoveryIoWorker(historicalIoNode)
                   \/ \E controlNode \in AsyncCurrentResponsiveVoters:
                          EnqueueIoLocalControl(controlNode)
                   \/ \E historicalControlNode
                          \in asyncHistoricalRecoveryTargets:
                          EnqueueHistoricalRecoveryIoLocalControl(
                            historicalControlNode)
        BY <1>1, <3>7, Stage4CapacityIoPreservesOrProgresses
      <3>8. CASE AsyncNetworkStep \/ AsyncFaultStep
        BY <1>1, <3>8,
           Stage4CapacityNetworkOrFaultPreservesOrProgresses
      <3>9. CASE AsyncSetGST
        BY <1>1, <3>9
           DEF Stage4CapacityStepResult, Stage4CapacityStrictResult,
               Stage4CapacityProgress, Stage4CapacityGoal,
               Stage4CapacityServeEpisodeResidual,
               Stage4CapacityBlockedAtRank,
               ReadyStage4CausalCapacityBlocked,
               ProtectedStage4Pending, ProtectedOwnedAtServiceRank,
               AsyncSetGST
      <3>10. CASE \E node \in ValidatorIds: PreGstCrash(node)
        BY <1>1, <3>10
           DEF Stage4CapacityStepResult, Stage4CapacityStrictResult,
               Stage4CapacityProgress, Stage4CapacityGoal,
               Stage4CapacityBlockedAtRank,
               ReadyStage4CausalCapacityBlocked,
               ProtectedStage4Pending, ProtectedOwnedAtServiceRank,
               PreGstCrash
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
           <3>7, <3>8, <3>9, <3>10
           DEF AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
               AsyncNonRunnerStep
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM FairStage4CapacityOneStep ==
  \A initialContext, candidate, position:
    \A rank \in Stage4CapacityCarrier:
      Stage4CapacityFiniteServeEpisodeResidualProperty(
        AsyncSpecAt(initialContext))
        => (AsyncSpecAt(initialContext)
              => (Stage4CapacityBlockedAtRank(candidate, position, rank)
                    ~> Stage4CapacityProgress(candidate, position, rank)))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW candidate,
                NEW position,
                NEW rank \in Stage4CapacityCarrier,
                Stage4CapacityFiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (Stage4CapacityBlockedAtRank(
                       candidate, position, rank)
                       ~> Stage4CapacityProgress(
                            candidate, position, rank))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
                    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
         AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity, PTL
    <2>2. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>3. /\ Stage4CapacityBlockedAtRank(
                  candidate, position, rank)
             /\ ~(Stage4CapacityProgress(candidate, position, rank)
                    \/ Stage4CapacityServeEpisodeResidual(
                         candidate, position, rank))
            => ENABLED
                 <<PostGstRunNode(candidate.node)>>_AsyncAllVars
      BY <2>1, ProtectedOwnedCandidateEnablesFairRunNode, PTL
         DEF Stage4CapacityBlockedAtRank,
             ReadyStage4CausalCapacityBlocked,
             ProtectedStage4Pending, ProtectedOwnedAtServiceRank,
             Stage4CapacityProgress, Stage4CapacityGoal
    <2>4. /\ Stage4CapacityBlockedAtRank(
                  candidate, position, rank)
             /\ ~(Stage4CapacityProgress(candidate, position, rank)
                    \/ Stage4CapacityServeEpisodeResidual(
                         candidate, position, rank))
             /\ <<PostGstRunNode(
                    candidate.node)>>_AsyncAllVars
            => \/ Stage4CapacityProgress(candidate, position, rank)'
               \/ Stage4CapacityServeEpisodeResidual(
                    candidate, position, rank)'
      BY Stage4CapacitySameNodeRunProducesOutcome
         DEF Stage4CapacityStrictResult,
             Stage4CapacityServeEpisodeResidual,
             Stage4CapacityCandidateProducerContinuationReentry
    <2>5. Stage4CapacityBlockedAtRank(candidate, position, rank)
              /\ [AsyncNext]_AsyncAllVars
            => Stage4CapacityBlockedAtRank(
                 candidate, position, rank)'
                 \/ Stage4CapacityProgress(candidate, position, rank)'
                 \/ Stage4CapacityServeEpisodeResidual(
                      candidate, position, rank)'
      BY Stage4CapacityBlockedStep
         DEF Stage4CapacityStepResult, Stage4CapacityStrictResult
    <2>6. CASE candidate.node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(
                    PostGstRunNode(candidate.node))
        BY <2>6 DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (Stage4CapacityBlockedAtRank(
                     candidate, position, rank)
                     ~> (Stage4CapacityProgress(
                           candidate, position, rank)
                          \/ Stage4CapacityServeEpisodeResidual(
                               candidate, position, rank)))
        BY <2>3, <2>4, <2>5, <3>1, PTL DEF AsyncSpecAt
      <3>3. AsyncSpecAt(initialContext)
               => (Stage4CapacityServeEpisodeResidual(
                     candidate, position, rank)
                     ~> Stage4CapacityProgress(
                          candidate, position, rank))
        BY <1>1
           DEF Stage4CapacityFiniteServeEpisodeResidualProperty
      <3>4. AsyncSpecAt(initialContext)
               => (Stage4CapacityBlockedAtRank(
                     candidate, position, rank)
                     ~> Stage4CapacityProgress(
                          candidate, position, rank))
        BY <3>2, <3>3, PTL
      <3> QED BY <3>4
    <2>7. CASE candidate.node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~Stage4CapacityBlockedAtRank(
                      candidate, position, rank)
        BY <2>2, <2>7, PTL
           DEF Stage4CapacityBlockedAtRank,
               ReadyStage4CausalCapacityBlocked,
               ProtectedStage4Pending, ProtectedOwnedAtServiceRank,
               ResponsiveProtectedCandidateOwned
      <3>2. AsyncSpecAt(initialContext)
               => (Stage4CapacityBlockedAtRank(
                     candidate, position, rank)
                     ~> Stage4CapacityProgress(
                          candidate, position, rank))
        BY <3>1, PTL
      <3> QED BY <3>2
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM FairStage4CapacityRankDescent ==
  \A initialContext, candidate, position:
    Stage4CapacityFiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => \A rank \in Stage4CapacityCarrier:
                 Stage4CapacityBlockedAtRank(candidate, position, rank)
                   ~> Stage4CapacityGoal(candidate, position))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                Stage4CapacityFiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => \A rank \in Stage4CapacityCarrier:
                      Stage4CapacityBlockedAtRank(
                        candidate, position, rank)
                        ~> Stage4CapacityGoal(candidate, position)
    <2>1. ASSUME NEW rank \in Stage4CapacityCarrier
           PROVE AsyncSpecAt(initialContext)
                   => (Stage4CapacityBlockedAtRank(
                         candidate, position, rank)
                         ~> (Stage4CapacityGoal(candidate, position)
                              \/ \E lower \in SetLessThan(
                                   rank, Stage4CapacityOrdering,
                                   Stage4CapacityCarrier):
                                   Stage4CapacityBlockedAtRank(
                                     candidate, position, lower)))
      BY FairStage4CapacityOneStep
         DEF Stage4CapacityProgress
    <2>2. AsyncSpecAt(initialContext)
             => \A rank \in Stage4CapacityCarrier:
                  Stage4CapacityBlockedAtRank(
                    candidate, position, rank)
                    ~> (Stage4CapacityGoal(candidate, position)
                         \/ \E lower \in SetLessThan(
                              rank, Stage4CapacityOrdering,
                              Stage4CapacityCarrier):
                              Stage4CapacityBlockedAtRank(
                                candidate, position, lower))
      BY <2>1
    <2>3. AsyncSpecAt(initialContext)
             => \A rank \in Stage4CapacityCarrier:
                  Stage4CapacityBlockedAtRank(
                    candidate, position, rank)
                    ~> Stage4CapacityGoal(candidate, position)
      BY <2>2, Stage4CapacityOrderingIsWellFounded,
         WellFoundedLeadsTo
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM FairNonCompletionCausalCapacityOpens ==
  \A initialContext, candidate, position:
    Stage4CapacityFiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => (AsyncSpecAt(initialContext)
            => (ReadyStage4CausalCapacityBlocked(candidate, position)
                  ~> Stage4CapacityGoal(candidate, position)))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position,
                Stage4CapacityFiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (ReadyStage4CausalCapacityBlocked(
                       candidate, position)
                       ~> Stage4CapacityGoal(candidate, position))
    <2>1. AsyncSpecAt(initialContext) => []AsyncTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2>2. AsyncSpecAt(initialContext)
             => (ReadyStage4CausalCapacityBlocked(candidate, position)
                   ~> \E rank \in Stage4CapacityCarrier:
                        Stage4CapacityBlockedAtRank(
                          candidate, position, rank))
      BY <2>1, Stage4CapacityRankInCarrier, PTL
         DEF Stage4CapacityBlockedAtRank
    <2>3. AsyncSpecAt(initialContext)
             => \A rank \in Stage4CapacityCarrier:
                  Stage4CapacityBlockedAtRank(
                    candidate, position, rank)
                    ~> Stage4CapacityGoal(candidate, position)
      BY FairStage4CapacityRankDescent
    <2> QED BY <2>2, <2>3, PTL
  <1> QED BY <1>1

ProtectedStage4Actionable(candidate, position) ==
  /\ ProtectedStage4Pending(candidate, position)
  /\ ReadyStage4Actionable(candidate)

Stage4ActionableStepResult(candidate, position) ==
  \/ ProtectedStage4Actionable(candidate, position)'
  \/ ProtectedRankProgressExit(candidate, <<4, position>>)'

THEOREM Stage4ActionableOtherRunnerStep ==
  \A candidate, position:
    /\ ProtectedStage4Actionable(candidate, position)
    /\ \/ \E node \in AsyncCurrentResponsiveVoters:
              /\ node # candidate.node
              /\ RunNode(node)
       \/ \E node \in AsyncResponsiveAppliedArchiveServers:
              RunHistoricalServer(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              /\ node # candidate.node
              /\ RunHistoricalRecoveryNode(node)
    => Stage4ActionableStepResult(candidate, position)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4ActionableStepResult, ProtectedStage4Actionable,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CandidateInReadyQueue,
       LocalAdmissionCanAdvance, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RunHistoricalServer,
       AsyncProgressOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4ActionableClockStep ==
  \A candidate, position:
    /\ ProtectedStage4Actionable(candidate, position)
    /\ AsyncTick
    => Stage4ActionableStepResult(candidate, position)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4ActionableStepResult, ProtectedStage4Actionable,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CandidateInReadyQueue,
       LocalAdmissionCanAdvance, AsyncTick, AsyncNonClockVars,
       AsyncProgressOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4ActionableDiscoveryPrefix ==
  \A candidate, position:
    /\ ProtectedStage4Actionable(candidate, position)
    /\ \/ \E node \in AsyncCurrentResponsiveVoters:
              DirectCommitCertificateDiscoveryStep(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              DirectHistoricalCommitCertificateDiscoveryStep(node)
    => Stage4ActionableStepResult(candidate, position)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4ActionableStepResult, ProtectedStage4Actionable,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CandidateInReadyQueue,
       LocalAdmissionCanAdvance,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       AsyncProgressOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4ActionableIoStep ==
  \A candidate, position:
    /\ ProtectedStage4Actionable(candidate, position)
    /\ \/ \E node \in AsyncArchiveIoServiceNodes: ServiceIoWorker(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              ServiceHistoricalRecoveryIoWorker(node)
       \/ \E node \in AsyncCurrentResponsiveVoters:
              EnqueueIoLocalControl(node)
       \/ \E node \in asyncHistoricalRecoveryTargets:
              EnqueueHistoricalRecoveryIoLocalControl(node)
    => Stage4ActionableStepResult(candidate, position)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HeadTailProperties, SequenceSetAfterAppend, Isa
   DEF Stage4ActionableStepResult, ProtectedStage4Actionable,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CandidateInReadyQueue,
       LocalAdmissionCanAdvance,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, AsyncProgressOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4ActionableNetworkOrFaultStep ==
  \A candidate, position:
    /\ ProtectedStage4Actionable(candidate, position)
    /\ (AsyncNetworkStep \/ AsyncFaultStep)
    => Stage4ActionableStepResult(candidate, position)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4ActionableStepResult, ProtectedStage4Actionable,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CandidateInReadyQueue,
       LocalAdmissionCanAdvance, AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstCrash, AsyncProgressOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4ActionableOpenHistoricalRecoveryStep ==
  \A candidate, position:
    /\ ProtectedStage4Actionable(candidate, position)
    /\ \E node \in ValidatorIds: OpenHistoricalRecovery(node)
    => Stage4ActionableStepResult(candidate, position)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership, Isa
   DEF Stage4ActionableStepResult, ProtectedStage4Actionable,
       ProtectedStage4Pending, ReadyStage4Actionable,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, CandidateInReadyQueue,
       LocalAdmissionCanAdvance, OpenHistoricalRecovery,
       AsyncProgressOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage4ActionableStutterStep ==
  \A candidate, position:
    /\ ProtectedStage4Actionable(candidate, position)
    /\ UNCHANGED AsyncAllVars
    => ProtectedStage4Actionable(candidate, position)'
BY Isa DEF ProtectedStage4Actionable, ProtectedStage4Pending,
           ReadyStage4Actionable, ProtectedOwnedAtServiceRank,
           AsyncAllVars, AsyncSchedulerVars, vars

THEOREM Stage4ActionableUnlessProgress ==
  \A candidate, position:
    /\ ProtectedStage4Actionable(candidate, position)
    /\ [AsyncNext]_AsyncAllVars
    => Stage4ActionableStepResult(candidate, position)
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                ProtectedStage4Actionable(candidate, position),
                [AsyncNext]_AsyncAllVars
         PROVE Stage4ActionableStepResult(candidate, position)
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>1, Stage4ActionableStutterStep
         DEF Stage4ActionableStepResult
    <2>2. CASE AsyncNext
      <3>1. CASE RunNode(candidate.node)
        BY <1>1, <3>1, Stage4LocalAdvanceStrictlyProgresses
           DEF Stage4ActionableStepResult,
               ProtectedStage4Actionable, PostGstRunNode,
               ProtectedStage4Pending, ProtectedOwnedAtServiceRank
      <3>2. CASE \/ \E runnerNode \in AsyncCurrentResponsiveVoters:
                          /\ runnerNode # candidate.node
                          /\ RunNode(runnerNode)
                   \/ \E historicalNode
                          \in AsyncResponsiveAppliedArchiveServers:
                          RunHistoricalServer(historicalNode)
                   \/ \E recoveryNode \in asyncHistoricalRecoveryTargets:
                          /\ recoveryNode # candidate.node
                          /\ RunHistoricalRecoveryNode(recoveryNode)
        BY <1>1, <3>2, Stage4ActionableOtherRunnerStep
      <3>3. CASE \E historicalNode \in asyncHistoricalRecoveryTargets:
                    /\ historicalNode = candidate.node
                    /\ RunHistoricalRecoveryNode(historicalNode)
        BY <1>1, <3>3, Stage4LocalAdvanceStrictlyProgresses, Isa
           DEF Stage4ActionableStepResult,
               ProtectedStage4Actionable, PostGstRunNode,
               ProtectedStage4Pending, ProtectedOwnedAtServiceRank,
               RunNode, RunHistoricalRecoveryNode
      <3>4. CASE AsyncTick
        BY <1>1, <3>4, Stage4ActionableClockStep
      <3>5. CASE \E node \in ValidatorIds:
                    OpenHistoricalRecovery(node)
        BY <1>1, <3>5, Stage4ActionableOpenHistoricalRecoveryStep
      <3>6. CASE \/ \E node \in AsyncCurrentResponsiveVoters:
                          DirectCommitCertificateDiscoveryStep(node)
                   \/ \E historicalDiscoveryNode
                          \in asyncHistoricalRecoveryTargets:
                          DirectHistoricalCommitCertificateDiscoveryStep(
                            historicalDiscoveryNode)
        BY <1>1, <3>6, Stage4ActionableDiscoveryPrefix
      <3>7. CASE \/ \E ioNode \in AsyncArchiveIoServiceNodes:
                          ServiceIoWorker(ioNode)
                   \/ \E historicalIoNode \in asyncHistoricalRecoveryTargets:
                          ServiceHistoricalRecoveryIoWorker(historicalIoNode)
                   \/ \E controlNode \in AsyncCurrentResponsiveVoters:
                          EnqueueIoLocalControl(controlNode)
                   \/ \E historicalControlNode
                          \in asyncHistoricalRecoveryTargets:
                          EnqueueHistoricalRecoveryIoLocalControl(
                            historicalControlNode)
        BY <1>1, <3>7, Stage4ActionableIoStep
      <3>8. CASE AsyncNetworkStep \/ AsyncFaultStep
        BY <1>1, <3>8, Stage4ActionableNetworkOrFaultStep
      <3>9. CASE AsyncSetGST
        BY <1>1, <3>9
           DEF ProtectedStage4Actionable, ProtectedStage4Pending,
               ProtectedOwnedAtServiceRank, AsyncSetGST
      <3>10. CASE \E node \in ValidatorIds: PreGstCrash(node)
        BY <1>1, <3>10
           DEF ProtectedStage4Actionable, ProtectedStage4Pending,
               ProtectedOwnedAtServiceRank, PreGstCrash
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
           <3>7, <3>8, <3>9, <3>10
           DEF AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
               AsyncNonRunnerStep
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Every stage-4 owner is in exactly one of the actionable, causal-capacity, or
auxiliary scheduler branches.  The branch-local one-step lemmas therefore
compose into a non-vacuous safety step: the exact owner either remains at
stage 4 or leaves through the protected-rank progress exit.  Stage 6 uses this
fact when a ready stage-4 witness temporarily owns its completion capacity.
***************************************************************************)
THEOREM ProtectedStage4UnlessProgress ==
  \A candidate, position:
    /\ ProtectedStage4Pending(candidate, position)
    /\ [AsyncNext]_AsyncAllVars
    => \/ ProtectedStage4Pending(candidate, position)'
       \/ ProtectedRankProgressExit(candidate, <<4, position>>)'
PROOF
  <1>1. ASSUME NEW candidate, NEW position,
                ProtectedStage4Pending(candidate, position),
                [AsyncNext]_AsyncAllVars
         PROVE \/ ProtectedStage4Pending(candidate, position)'
               \/ ProtectedRankProgressExit(
                    candidate, <<4, position>>)'
    <2>1. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
         DEF ProtectedStage4Pending
    <2>2. CASE ReadyStage4Actionable(candidate)
      <3>1. ProtectedStage4Actionable(candidate, position)
        BY <1>1, <2>2 DEF ProtectedStage4Actionable
      <3>2. Stage4ActionableStepResult(candidate, position)
        BY <1>1, <3>1, Stage4ActionableUnlessProgress
      <3> QED BY <3>2, Isa
           DEF Stage4ActionableStepResult,
               ProtectedStage4Actionable
    <2>3. CASE /\ ~ReadyStage4Actionable(candidate)
                 /\ NonCompletionCausalAdmissionDebt(candidate.node)
      <3>1. Stage4CapacityRank(candidate.node)
                 \in Stage4CapacityCarrier
        BY <1>1, <2>1, Stage4CapacityRankInCarrier, Isa
           DEF ProtectedStage4Pending, ProtectedOwnedAtServiceRank,
               ResponsiveProtectedCandidateOwned
      <3>2. Stage4CapacityBlockedAtRank(
                 candidate, position, Stage4CapacityRank(candidate.node))
        BY <1>1, <2>3 DEF Stage4CapacityBlockedAtRank
      <3>3. Stage4CapacityStepResult(
                 candidate, position, Stage4CapacityRank(candidate.node))
        BY <1>1, <3>1, <3>2, Stage4CapacityBlockedStep
      <3> QED BY <3>3, Isa
           DEF Stage4CapacityStepResult, Stage4CapacityStrictResult,
               Stage4CapacityProgress, Stage4CapacityGoal,
               Stage4CapacityBlockedAtRank,
               ProtectedStage4Actionable
    <2>4. CASE /\ ~ReadyStage4Actionable(candidate)
                 /\ ~NonCompletionCausalAdmissionDebt(candidate.node)
      <3>1. ReadyRunAuxRank(candidate.node) \in ReadyRunAuxCarrier
        BY <1>1, <2>1, ReadyRunAuxRankInCarrier, Isa
           DEF ProtectedStage4Pending, ProtectedOwnedAtServiceRank,
               ResponsiveProtectedCandidateOwned
      <3>2. ReadyBlockedAtAux(
                 candidate, position, ReadyRunAuxRank(candidate.node))
        BY <1>1, <2>4, Isa
           DEF ReadyBlockedAtAux, ReadyStage4CausalCapacityBlocked
      <3>3. Stage4AuxStepResult(
                 candidate, position, ReadyRunAuxRank(candidate.node))
        BY <1>1, <3>1, <3>2, Stage4BlockedAuxStep
      <3> QED BY <3>3, Isa
           DEF Stage4AuxStepResult, Stage4AuxStrictResult,
               Stage4AuxProgress, Stage4ServeEpisodeResidual,
               ReadyBlockedAtAux,
               ReadyStage4CausalCapacityBlocked,
               ProtectedStage4Actionable
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM FairStage4ActionableProgress ==
  \A initialContext, candidate, position:
    AsyncSpecAt(initialContext)
      => (ProtectedStage4Actionable(candidate, position)
            ~> ProtectedRankProgressExit(candidate, <<4, position>>))
PROOF
  <1>1. ASSUME NEW initialContext, NEW candidate, NEW position
         PROVE AsyncSpecAt(initialContext)
                 => (ProtectedStage4Actionable(candidate, position)
                       ~> ProtectedRankProgressExit(
                            candidate, <<4, position>>))
    <2>1. AsyncSpecAt(initialContext)
             => /\ [](AsyncCurrentResponsiveVoters
                       = AsyncVotersAt(initialContext))
                /\ []AsyncCandidateProducerContinuationExternalCoverageInvariant
                /\ []AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
         AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity
    <2>2. /\ ProtectedStage4Actionable(candidate, position)
             /\ ~ProtectedRankProgressExit(candidate, <<4, position>>)
            => ENABLED
                 <<PostGstRunNode(candidate.node)>>_AsyncAllVars
      BY <2>1, ProtectedOwnedCandidateEnablesFairRunNode, PTL
         DEF ProtectedStage4Actionable, ProtectedStage4Pending,
             ProtectedOwnedAtServiceRank
    <2>3. /\ ProtectedStage4Actionable(candidate, position)
             /\ ~ProtectedRankProgressExit(candidate, <<4, position>>)
             /\ <<PostGstRunNode(candidate.node)>>_AsyncAllVars
            => ProtectedRankProgressExit(candidate, <<4, position>>)'
      BY Stage4LocalAdvanceStrictlyProgresses
         DEF ProtectedStage4Actionable, PostGstRunNode
    <2>4. ProtectedStage4Actionable(candidate, position)
              /\ [AsyncNext]_AsyncAllVars
            => ProtectedStage4Actionable(candidate, position)'
                 \/ ProtectedRankProgressExit(
                      candidate, <<4, position>>)'
      BY Stage4ActionableUnlessProgress
         DEF Stage4ActionableStepResult
    <2>5. CASE candidate.node \in AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => WF_AsyncAllVars(
                    PostGstRunNode(candidate.node))
        BY <2>5 DEF AsyncSpecAt, AsyncFairnessAt
      <3>2. AsyncSpecAt(initialContext)
               => (ProtectedStage4Actionable(candidate, position)
                     ~> ProtectedRankProgressExit(
                          candidate, <<4, position>>))
        BY <2>2, <2>3, <2>4, <3>1, PTL DEF AsyncSpecAt
      <3> QED BY <3>2
    <2>6. CASE candidate.node \notin AsyncVotersAt(initialContext)
      <3>1. AsyncSpecAt(initialContext)
               => []~ProtectedStage4Actionable(candidate, position)
        BY <2>1, <2>6, PTL
           DEF ProtectedStage4Actionable, ProtectedStage4Pending,
               ProtectedOwnedAtServiceRank,
               ResponsiveProtectedCandidateOwned
      <3>2. AsyncSpecAt(initialContext)
               => (ProtectedStage4Actionable(candidate, position)
                     ~> ProtectedRankProgressExit(
                          candidate, <<4, position>>))
        BY <3>1, PTL
      <3> QED BY <3>2
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM FairProtectedStage4RankDescent ==
  \A initialContext, candidate:
    \A position \in Nat:
      Stage4RefinementFiniteServeEpisodeResidualProperty(
        AsyncSpecAt(initialContext))
        => (AsyncSpecAt(initialContext)
              => (ProtectedOwnedAtServiceRank(candidate, <<4, position>>)
                    ~> ProtectedRankProgressExit(
                         candidate, <<4, position>>)))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW candidate,
                NEW position \in Nat,
                Stage4RefinementFiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE AsyncSpecAt(initialContext)
                 => (ProtectedOwnedAtServiceRank(
                       candidate, <<4, position>>)
                       ~> ProtectedRankProgressExit(
                            candidate, <<4, position>>))
    <2>1. AsyncSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant)
      BY AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>2. AsyncSpecAt(initialContext)
             => (ProtectedStage4Pending(candidate, position)
                   ~> (ProtectedRankProgressExit(
                         candidate, <<4, position>>)
                        \/ ReadyStage4Actionable(candidate)
                        \/ ReadyStage4CausalCapacityBlocked(
                             candidate, position)))
      <3>1. AsyncSpecAt(initialContext)
               => (ProtectedStage4Pending(candidate, position)
                     ~> (ReadyStage4Actionable(candidate)
                          \/ ReadyStage4CausalCapacityBlocked(
                               candidate, position)
                          \/ \E rank \in ReadyRunAuxCarrier:
                               ReadyBlockedAtAux(
                                 candidate, position, rank)))
      BY <2>1, ReadyRunAuxRankInCarrier, PTL
           DEF ReadyBlockedAtAux
      <3>2. AsyncSpecAt(initialContext)
               => \A rank \in ReadyRunAuxCarrier:
                    ReadyBlockedAtAux(candidate, position, rank)
                      ~> Stage4AuxGoal(candidate, position)
        BY FairStage4AuxRankDescent
           DEF Stage4RefinementFiniteServeEpisodeResidualProperty
      <3> QED BY <3>1, <3>2, PTL DEF Stage4AuxGoal
    <2>3. AsyncSpecAt(initialContext)
             => (ReadyStage4CausalCapacityBlocked(candidate, position)
                   ~> (ProtectedRankProgressExit(
                         candidate, <<4, position>>)
                        \/ ReadyStage4Actionable(candidate)))
      BY FairNonCompletionCausalCapacityOpens
         DEF Stage4CapacityGoal,
             Stage4RefinementFiniteServeEpisodeResidualProperty
    <2>4. AsyncSpecAt(initialContext)
             => (ProtectedStage4Actionable(candidate, position)
                   ~> ProtectedRankProgressExit(
                        candidate, <<4, position>>))
      BY FairStage4ActionableProgress
    <2>5. AsyncSpecAt(initialContext)
             => (ProtectedStage4Pending(candidate, position)
                   ~> ProtectedRankProgressExit(
                        candidate, <<4, position>>))
      BY <2>2, <2>3, <2>4, PTL DEF ProtectedStage4Actionable
    <2>6. AsyncSpecAt(initialContext)
             => (ProtectedOwnedAtServiceRank(candidate, <<4, position>>)
                   ~> ProtectedStage4Pending(candidate, position))
      BY <2>1, PTL DEF ProtectedStage4Pending
    <2> QED BY <2>5, <2>6, PTL
  <1> QED BY <1>1

(***************************************************************************
Normal proposal/Prepare ownership closure.

These lemmas pin the canonical constructor families which cover every
production source of a protected Normal proposal/Prepare candidate.  The
families deliberately over-approximate reachable provenance, while excluding
RejectNormal and mismatched class/kind/item Cartesian records.
***************************************************************************)

THEOREM ExactCandidateConstructorIsInCarrier ==
  \A commandClass, kind, node, blockHeight, roundView, subject, item,
     candidateConsumerContext, candidateConsumerView,
     candidateConsumerGeneration, evidence,
     bodyIdentity, manifestIdentity, commitmentIdentity:
    /\ commandClass \in AsyncCommandClasses
    /\ kind \in AsyncWorkKinds
    /\ node \in ValidatorIds
    /\ blockHeight \in Heights
    /\ roundView \in Views
    /\ subject \in SubjectOrNone
    /\ item \in AsyncNetworkItems \cup {NoAsyncItem}
    /\ candidateConsumerContext \in ContextRecords
    /\ candidateConsumerView \in Views
    /\ candidateConsumerGeneration \in Generations
    /\ evidence \in AsyncEvidenceSet
    /\ bodyIdentity \in SubjectOrNone
    /\ manifestIdentity \in SubjectOrNone
    /\ commitmentIdentity \in SubjectOrNone
    => AsyncCandidateWithIdentity(
         commandClass, kind, node, blockHeight, roundView, subject, item,
         candidateConsumerContext, candidateConsumerView,
         candidateConsumerGeneration, evidence,
         bodyIdentity, manifestIdentity, commitmentIdentity)
         \in AsyncCandidateSet
BY SMTT(60) DEF AsyncCandidateSet, AsyncCandidateWithIdentity

THEOREM TypedEvidenceIsInCarrier ==
  \A evidence:
    AsyncEvidenceTyped(evidence) => evidence \in AsyncEvidenceSet
BY TypedItemIsInNetworkCarrier, Isa
   DEF AsyncEvidenceTyped, AsyncEvidenceSet,
       AsyncTcRecordTyped, TcRecordSet

THEOREM TypedCandidateIsInCarrier ==
  \A candidate:
    AsyncCandidateTyped(candidate) => candidate \in AsyncCandidateSet
BY TypedItemIsInNetworkCarrier, TypedEvidenceIsInCarrier, SMTT(60)
   DEF AsyncCandidateTyped, AsyncCandidateSet, AsyncCandidateDomain

(***************************************************************************
Executable live-owner normalization.

These are semantic equivalences, not alternate ownership rules.  The live
predicates avoid constructing the Cartesian candidate/job carriers while the
canonical predicates retain the immutable historical-constructor statement.
Both current and successor-state typing are required for the action-level
equivalences because rank exit is tested in the primed state.
***************************************************************************)

THEOREM ActiveScheduledCandidatesAreTyped ==
  AsyncTypeInvariant
    => \A candidate \in ActiveScheduledCandidates:
         AsyncCandidateTyped(candidate)
BY Isa
   DEF ActiveScheduledCandidates, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, SequenceSet,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant, AsyncIoTypeInvariant,
       AsyncIoContentTypeInvariant, AsyncIoWorkContentTypeInvariant,
       AsyncDeferredTypeInvariant, AsyncDeferredContentTypeInvariant,
       AsyncQueueTyped, AsyncCompletionSequenceTyped

THEOREM TypedLiveNormalProposalPrepareIsCanonical ==
  \A candidate:
    AsyncCandidateTyped(candidate)
      => (LiveNormalProposalPrepareCandidate(candidate)
            <=> NormalProposalPrepareCandidate(candidate))
BY TypedItemIsInNetworkCarrier, TypedCandidateIsInCarrier, Isa
   DEF LiveNormalProposalPrepareCandidate,
       NormalProposalPrepareCandidate,
       NormalProposalPrepareNoItemCandidate,
       NormalProposalPrepareNetworkCandidate,
       FrozenNormalAssemblyCandidate, FrozenInstallProposalSuccessor,
       FrozenNormalBeginPrepareCandidate, FrozenNormalDeliveryCandidate,
       NextCandidateGeneration, NormalProposalPrepareNoItemKinds,
       NormalProposalPrepareNetworkKinds, NormalBeginPrepareParentKinds,
       AsyncCandidateTyped, AsyncCandidateSet, AsyncCandidateDomain,
       AsyncCandidateWithIdentity, AsyncEvidenceTyped, AsyncItemTyped,
       Views, Generations, Heights

THEOREM TypedLiveProtectedServiceIsCanonical ==
  \A candidate:
    AsyncCandidateTyped(candidate)
      => (LiveProtectedServiceCandidate(candidate)
            <=> ProtectedServiceCandidate(candidate))
BY TypedCandidateIsInCarrier,
   TypedLiveNormalProposalPrepareIsCanonical, Isa
   DEF LiveProtectedServiceCandidate, ProtectedServiceCandidate

THEOREM LiveResponsiveProtectedOwnerIsCanonical ==
  AsyncTypeInvariant
    => \A candidate \in ActiveScheduledCandidates:
         (LiveResponsiveProtectedCandidateOwned(candidate)
           <=> ResponsiveProtectedCandidateOwned(candidate))
BY ActiveScheduledCandidatesAreTyped,
   TypedLiveProtectedServiceIsCanonical, Isa
   DEF AsyncTypeInvariant, LiveResponsiveProtectedCandidateOwned,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateScheduled, ActiveScheduledCandidates

THEOREM LiveResponsiveServeOwnerIsCanonical ==
  AsyncTypeInvariant
    => \A node \in ValidatorIds:
         \A job \in SequenceSet(asyncIoQueues[node]):
           (LiveResponsiveProtectedServeJobOwned(node, job)
             <=> ResponsiveProtectedServeJobOwned(node, job))
BY TypedCandidateIsInCarrier, Isa
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant, AsyncIoSequenceTyped,
       AsyncIoJobTyped, AsyncServeJobSet, AsyncIoJob,
       LiveResponsiveProtectedServeJobOwned,
       ResponsiveProtectedServeJobOwned

THEOREM LiveProtectedRankStepIsCanonical ==
  /\ AsyncTypeInvariant
  /\ AsyncTypeInvariant'
  => (LiveProtectedServiceRankDecreaseStep
        <=> ProtectedServiceRankDecreaseStep)
BY LiveResponsiveProtectedOwnerIsCanonical, Isa
   DEF LiveProtectedServiceRankDecreaseStep,
       ProtectedServiceRankDecreaseStep

THEOREM LiveProtectedServeRankStepIsCanonical ==
  /\ AsyncTypeInvariant
  /\ AsyncTypeInvariant'
  => (LiveProtectedServeRankDecreaseStep
        <=> ProtectedServeRankDecreaseStep)
BY LiveResponsiveServeOwnerIsCanonical, Isa
   DEF LiveProtectedServeRankDecreaseStep,
       ProtectedServeRankDecreaseStep, ActiveIoJobs

THEOREM CanonicalCandidateConstructorIsInCarrier ==
  TypeInvariant =>
  \A commandClass \in AsyncCommandClasses,
     kind \in AsyncWorkKinds,
     node \in ValidatorIds,
     blockHeight \in Heights,
     roundView \in Views,
     subject \in SubjectOrNone,
     item \in AsyncNetworkItems \cup {NoAsyncItem}:
    AsyncCandidate(commandClass, kind, node, blockHeight, roundView,
                   subject, item)
      \in AsyncCandidateSet
BY SMTT(60)
   DEF TypeInvariant, AsyncCandidate, AsyncEvidenceSet,
       AsyncCandidateSet, AsyncCandidateWithIdentity

THEOREM CanonicalNormalProposalPrepareCandidateIsProtected ==
  \A candidate:
    NormalProposalPrepareCandidate(candidate)
      => ProtectedServiceCandidate(candidate)
PROOF
  <1>1. ASSUME NEW candidate,
                NormalProposalPrepareCandidate(candidate)
         PROVE ProtectedServiceCandidate(candidate)
    <2>1. candidate \in AsyncCandidateSet
      BY <1>1 DEF NormalProposalPrepareCandidate
    <2> QED BY <1>1, <2>1 DEF ProtectedServiceCandidate
  <1> QED BY <1>1

THEOREM TypedItemMakesTypedNormalDeliveryCandidate ==
  \A item:
    /\ TypeInvariant
    /\ AsyncItemTyped(item)
      => AsyncCandidateTyped(NormalDeliveryCandidate(item))
BY TypedItemMakesTypedDeliveryCandidate, SMT
   DEF NormalDeliveryCandidate, FrozenNormalDeliveryCandidate,
       DeliveryCandidate, AsyncCandidate, AsyncCandidateWithIdentity,
       AsyncCandidateTyped, AsyncCommandClasses

THEOREM InitialProposalAssemblyIsProtected ==
  TypeInvariant
    => \A node \in ValidatorIds:
         ProtectedServiceCandidate(InitialCausalCandidate(node))
PROOF
  <1>1. ASSUME TypeInvariant,
                NEW node \in ValidatorIds
         PROVE ProtectedServiceCandidate(InitialCausalCandidate(node))
    <2>1. AsyncCandidateTyped(InitialCausalCandidate(node))
      BY <1>1, InitialCausalCandidateIsTyped
    <2>2. /\ context.height \in Heights
           /\ context \in ContextRecords
           /\ nodeView[node] \in Views
           /\ generation[node] \in Generations
           /\ AsyncProposalSubject(node) \in SubjectOrNone
      BY <1>1, <2>1, InitialCausalCandidateShape
         DEF TypeInvariant, AsyncCandidateTyped
    <2>3. InitialCausalCandidate(node) \in AsyncCandidateSet
      BY <2>1, TypedCandidateIsInCarrier
    <2>4. NormalProposalPrepareNoItemCandidate(
             InitialCausalCandidate(node))
      <3>1. /\ InitialCausalCandidate(node).item = NoAsyncItem
             /\ InitialCausalCandidate(node).kind
                  \in NormalProposalPrepareNoItemKinds
        BY InitialCausalCandidateShape
           DEF InitialCausalCandidate, NoItemCandidate, AsyncCandidate,
               AsyncCandidateWithIdentity,
               NormalProposalPrepareNoItemKinds
      <3>2. \E blockContext \in ContextRecords,
                   owner \in ValidatorIds, roundView \in Views,
                   frozenConsumerGeneration \in Generations,
                   subject \in SubjectOrNone:
               InitialCausalCandidate(node) =
                 FrozenNormalAssemblyCandidate(
                   blockContext, owner, roundView,
                   frozenConsumerGeneration, subject, NoAsyncItem)
        <4>1. USE <1>1, <2>2
        <4>2. WITNESS context \in ContextRecords,
                        node \in ValidatorIds,
                        nodeView[node] \in Views,
                        generation[node] \in Generations,
                        AsyncProposalSubject(node) \in SubjectOrNone
        <4> QED BY <4>2
             DEF InitialCausalCandidate, NoItemCandidate,
                 FrozenNormalAssemblyCandidate, AsyncCandidate,
                 AsyncCandidateWithIdentity
      <3> QED BY <3>1, <3>2
           DEF NormalProposalPrepareNoItemCandidate
    <2>5. InitialCausalCandidate(node).class = "Normal"
      BY DEF InitialCausalCandidate, NoItemCandidate, AsyncCandidate,
             AsyncCandidateWithIdentity
    <2>6. NormalProposalPrepareCandidate(
             InitialCausalCandidate(node))
      BY <2>3, <2>4, <2>5 DEF NormalProposalPrepareCandidate
    <2> QED BY <2>6,
         CanonicalNormalProposalPrepareCandidateIsProtected
  <1> QED BY <1>1

THEOREM CausalBeginPrepareIsProtected ==
  \A command \in AsyncCandidateSet:
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ command.kind \in NormalBeginPrepareParentKinds
    => ProtectedServiceCandidate(
         CausalCandidate("Normal", "BeginPrepare", command))
PROOF
  <1>1. ASSUME NEW command \in AsyncCandidateSet,
                AsyncTypeInvariant,
                AsyncCandidateTyped(command),
                command.kind \in NormalBeginPrepareParentKinds
         PROVE ProtectedServiceCandidate(
                 CausalCandidate("Normal", "BeginPrepare", command))
    <2>1. AsyncCandidateTyped(
             CausalCandidate("Normal", "BeginPrepare", command))
      BY <1>1, CausalCandidateFromTypedCommand
         DEF AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds
    <2>2. /\ context.height \in Heights
           /\ command.node \in ValidatorIds
           /\ command.view \in Views
           /\ command.subject \in SubjectOrNone
      BY <2>1
         DEF AsyncCandidateTyped, CausalCandidate,
             NoItemCandidate, AsyncCandidate
    <2>3. CausalCandidate("Normal", "BeginPrepare", command)
             \in AsyncCandidateSet
      BY <2>1, TypedCandidateIsInCarrier
    <2>4. NormalProposalPrepareNoItemCandidate(
             CausalCandidate("Normal", "BeginPrepare", command))
      <3>1. /\ CausalCandidate("Normal", "BeginPrepare", command).item
                    = NoAsyncItem
             /\ CausalCandidate("Normal", "BeginPrepare", command).kind
                  \in NormalProposalPrepareNoItemKinds
        BY DEF CausalCandidate, AsyncCandidateFrom,
               AsyncCandidateWithIdentity,
               NormalProposalPrepareNoItemKinds
      <3>2. \E parent \in AsyncCandidateSet,
                   blockHeight \in Heights:
               /\ parent.kind \in NormalBeginPrepareParentKinds
               /\ CausalCandidate("Normal", "BeginPrepare", command) =
                    FrozenNormalBeginPrepareCandidate(
                      parent, blockHeight)
        <4>1. USE <1>1
        <4>2. WITNESS command \in AsyncCandidateSet,
                        context.height \in Heights
        <4> QED BY <1>1, <2>2, <4>2
             DEF CausalCandidate, AsyncCandidateFrom,
                 FrozenNormalBeginPrepareCandidate,
                 AsyncCandidateWithIdentity
      <3> QED BY <3>1, <3>2
           DEF NormalProposalPrepareNoItemCandidate
    <2>5. CausalCandidate("Normal", "BeginPrepare", command).class =
             "Normal"
      BY DEF CausalCandidate, AsyncCandidateFrom,
             AsyncCandidateWithIdentity
    <2>6. NormalProposalPrepareCandidate(
             CausalCandidate("Normal", "BeginPrepare", command))
      BY <2>3, <2>4, <2>5 DEF NormalProposalPrepareCandidate
    <2> QED BY <2>6,
         CanonicalNormalProposalPrepareCandidateIsProtected
  <1> QED BY <1>1

THEOREM FrozenNormalProposalOrVoteDeliveryIsProtected ==
  \A item:
    /\ TypeInvariant
    /\ item \in AsyncNetworkItems
    /\ AsyncItemTyped(item)
    /\ item.kind \in NormalProposalPrepareNetworkKinds
    => ProtectedServiceCandidate(NormalDeliveryCandidate(item))
PROOF
  <1>1. ASSUME TypeInvariant,
                NEW item \in AsyncNetworkItems,
                AsyncItemTyped(item),
                item.kind \in NormalProposalPrepareNetworkKinds
         PROVE ProtectedServiceCandidate(NormalDeliveryCandidate(item))
    <2>1. AsyncCandidateTyped(NormalDeliveryCandidate(item))
      BY <1>1, TypedItemMakesTypedNormalDeliveryCandidate
    <2>2. /\ DeliveryKind(item) \in AsyncWorkKinds
           /\ item.envelope.recipient \in ValidatorIds
           /\ DeliveryHeight(item) \in Heights
           /\ DeliveryView(item) \in Views
           /\ DeliverySubject(item) \in SubjectOrNone
           /\ context \in ContextRecords
           /\ nodeView[item.envelope.recipient] \in Views
           /\ generation[item.envelope.recipient] \in Generations
      BY <2>1
         DEF AsyncCandidateTyped, NormalDeliveryCandidate,
             FrozenNormalDeliveryCandidate, AsyncCandidateWithIdentity
    <2>3. NormalDeliveryCandidate(item) \in AsyncCandidateSet
      BY <2>1, TypedCandidateIsInCarrier
    <2>4. NormalProposalPrepareCandidate(
             NormalDeliveryCandidate(item))
      <3>1. NormalProposalPrepareNetworkCandidate(
               NormalDeliveryCandidate(item))
        <4>1. WITNESS item \in AsyncNetworkItems,
                        context \in ContextRecords,
                        nodeView[item.envelope.recipient] \in Views,
                        generation[item.envelope.recipient] \in Generations
        <4> QED BY <1>1, <2>2, <4>1
             DEF NormalDeliveryCandidate
      <3>2. NormalDeliveryCandidate(item).class = "Normal"
        BY DEF NormalDeliveryCandidate, FrozenNormalDeliveryCandidate,
               AsyncCandidateWithIdentity
      <3> QED BY <2>3, <3>1, <3>2
           DEF NormalProposalPrepareCandidate
    <2> QED BY <2>4,
         CanonicalNormalProposalPrepareCandidateIsProtected
  <1> QED BY <1>1

THEOREM NormalProposalOrVoteDeliveryIsProtected ==
  \A item:
    /\ TypeInvariant
    /\ item \in AsyncNetworkItems
    /\ AsyncItemTyped(item)
    /\ item.kind \in NormalProposalPrepareNetworkKinds
    /\ DeliveryClass(item) = "Normal"
    => ProtectedServiceCandidate(DeliveryCandidate(item))
PROOF
  <1>1. ASSUME TypeInvariant,
                NEW item \in AsyncNetworkItems,
                AsyncItemTyped(item),
                item.kind \in NormalProposalPrepareNetworkKinds,
                DeliveryClass(item) = "Normal"
         PROVE ProtectedServiceCandidate(DeliveryCandidate(item))
    <2>1. DeliveryCandidate(item) = NormalDeliveryCandidate(item)
      BY <1>1
         DEF DeliveryCandidate, NormalDeliveryCandidate,
             FrozenNormalDeliveryCandidate, AsyncCandidate,
             AsyncCandidateWithIdentity
    <2>2. ProtectedServiceCandidate(NormalDeliveryCandidate(item))
      BY <1>1, FrozenNormalProposalOrVoteDeliveryIsProtected
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ExecutedInstallProposalAssemblyIsProtected ==
  \A command \in AsyncCandidateSet:
    /\ StrongInductiveInvariant
    /\ AsyncTypeInvariant
    /\ AsyncCandidateTyped(command)
    /\ command.kind = "PersistInstallTC"
    /\ ExecuteCommand(command)
    => ProtectedServiceCandidate(InstallProposalSuccessor(command))
PROOF
  <1>1. ASSUME NEW command \in AsyncCandidateSet,
                StrongInductiveInvariant,
                AsyncTypeInvariant,
                AsyncCandidateTyped(command),
                command.kind = "PersistInstallTC",
                ExecuteCommand(command)
         PROVE ProtectedServiceCandidate(
                 InstallProposalSuccessor(command))
    <2>1. AsyncCandidateTyped(InstallProposalSuccessor(command))
      BY <1>1, ExecutedInstallSuccessorIsTypedAndOwned
         DEF InstallProposalSuccessor
    <2>2. /\ context \in ContextRecords
           /\ command.node \in ValidatorIds
           /\ command.view + 1 \in Views
           /\ generation[command.node] \in Generations
           /\ NextCandidateGeneration(generation[command.node])
                \in Generations
           /\ InstallProposalSubject(command) \in SubjectOrNone
      BY <1>1, <2>1, SMT
         DEF AsyncTypeInvariant, TypeInvariant, AsyncCandidateTyped,
             InstallProposalSuccessor, AsyncCandidateAtConsumer,
             AsyncCandidateWithIdentity, NextCandidateGeneration,
             Generations
    <2>3. InstallProposalSuccessor(command) \in AsyncCandidateSet
      BY <2>1, TypedCandidateIsInCarrier
    <2>4. NormalProposalPrepareNoItemCandidate(
             InstallProposalSuccessor(command))
      <3>1. /\ InstallProposalSuccessor(command).item = NoAsyncItem
             /\ InstallProposalSuccessor(command).kind
                  \in NormalProposalPrepareNoItemKinds
        BY DEF InstallProposalSuccessor, AsyncCandidateAtConsumer,
               AsyncCandidateWithIdentity,
               NormalProposalPrepareNoItemKinds
      <3>2. InstallProposalSuccessor(command) =
               FrozenInstallProposalSuccessor(
                 command, context, generation[command.node],
                 InstallProposalSubject(command))
        BY DEF InstallProposalSuccessor, AsyncCandidateAtConsumer,
               FrozenInstallProposalSuccessor,
               NextCandidateGeneration, AsyncCandidateWithIdentity
      <3>3. \E installCommand \in AsyncCandidateSet,
                   installedContext \in ContextRecords,
                   priorGeneration \in Generations,
                   subject \in SubjectOrNone:
               /\ installCommand.kind = "PersistInstallTC"
               /\ installCommand.view + 1 \in Views
               /\ InstallProposalSuccessor(command) =
                    FrozenInstallProposalSuccessor(
                      installCommand, installedContext,
                      priorGeneration, subject)
        <4>1. WITNESS command \in AsyncCandidateSet,
                        context \in ContextRecords,
                        generation[command.node] \in Generations,
                        InstallProposalSubject(command) \in SubjectOrNone
        <4> QED BY <1>1, <2>2, <3>2, <4>1
      <3> QED BY <3>1, <3>3
           DEF NormalProposalPrepareNoItemCandidate
    <2>5. InstallProposalSuccessor(command).class = "Normal"
      BY DEF InstallProposalSuccessor, AsyncCandidateAtConsumer,
             AsyncCandidateWithIdentity
    <2>6. NormalProposalPrepareCandidate(
             InstallProposalSuccessor(command))
      BY <2>3, <2>4, <2>5 DEF NormalProposalPrepareCandidate
    <2> QED BY <2>6,
         CanonicalNormalProposalPrepareCandidateIsProtected
  <1> QED BY <1>1

(***************************************************************************
The protected classification is stable across every concrete asynchronous
step.  This is the reset-boundary acceptance condition: a stored Normal
CommitVote remains protected even if a TC changes the dynamic
HistoricalLockedCommitItem/DeliveryClass result in the successor state.
***************************************************************************)
THEOREM AsyncNextPreservesNormalProposalPrepareCandidate ==
  \A candidate:
    /\ NormalProposalPrepareCandidate(candidate)
    /\ AsyncNext
    => NormalProposalPrepareCandidate(candidate)'
BY Isa
   DEF NormalProposalPrepareCandidate,
       NormalProposalPrepareNoItemCandidate,
       NormalProposalPrepareNetworkCandidate,
       FrozenNormalAssemblyCandidate,
       FrozenInstallProposalSuccessor,
       FrozenNormalBeginPrepareCandidate,
       FrozenNormalDeliveryCandidate, NextCandidateGeneration,
       NormalProposalPrepareNoItemKinds,
       NormalProposalPrepareNetworkKinds,
       AsyncCandidateWithIdentity

THEOREM ProtectedRankProgressCoversNormalProposalPrepare ==
  \A initialContext:
    ProtectedServiceRankProgressProperty(AsyncSpecAt(initialContext))
      => NormalProposalPrepareRankProgressProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ProtectedServiceRankProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE NormalProposalPrepareRankProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A candidate \in AsyncCandidateSet,
                     stage \in 2..6, position \in Nat:
              (gst
                /\ ResponsiveProtectedCandidateOwned(candidate)
                /\ NormalProposalPrepareCandidate(candidate)
                /\ CandidateServiceRank(candidate) = <<stage, position>>)
                ~> (~ResponsiveProtectedCandidateOwned(candidate)
                     \/ ServiceRankLess(CandidateServiceRank(candidate),
                          <<stage, position>>))
      <3>1. ASSUME NEW candidate \in AsyncCandidateSet,
                    NEW stage \in 2..6,
                    NEW position \in Nat
             PROVE (gst
                      /\ ResponsiveProtectedCandidateOwned(candidate)
                      /\ NormalProposalPrepareCandidate(candidate)
                      /\ CandidateServiceRank(candidate) =
                           <<stage, position>>)
                     ~> (~ResponsiveProtectedCandidateOwned(candidate)
                          \/ ServiceRankLess(
                               CandidateServiceRank(candidate),
                               <<stage, position>>))
        <4>1. (gst
                 /\ ResponsiveProtectedCandidateOwned(candidate)
                 /\ CandidateServiceRank(candidate) = <<stage, position>>)
                ~> (~ResponsiveProtectedCandidateOwned(candidate)
                     \/ ServiceRankLess(CandidateServiceRank(candidate),
                          <<stage, position>>))
          BY <1>1, <2>1 DEF ProtectedServiceRankProgressProperty
        <4>2. (gst
                 /\ ResponsiveProtectedCandidateOwned(candidate)
                 /\ NormalProposalPrepareCandidate(candidate)
                 /\ CandidateServiceRank(candidate) = <<stage, position>>)
               => (gst
                     /\ ResponsiveProtectedCandidateOwned(candidate)
                     /\ CandidateServiceRank(candidate) =
                          <<stage, position>>)
          OBVIOUS
        <4> QED BY <4>1, <4>2, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1 DEF NormalProposalPrepareRankProgressProperty
  <1> QED BY <1>1

THEOREM UnchangedHeightEvidenceIsNotProtocolProgress ==
  UNCHANGED
    <<availableBodies, durableBodies, retainedLockedBodies,
      validatedBodies, seenProposals, receivedVotes, receivedQCs,
      proposalIntents, prepareIntents, commitIntents,
      prepareQCs, commitQCs, decisions, applied>>
    => ~HeightProtocolEvidenceGrows
BY Isa DEF HeightProtocolEvidenceGrows, SetGains

THEOREM PostGstProductiveStepHasConcreteWitness ==
  PostGstProductiveStep
    => /\ gst
       /\ AsyncNext
       /\ \/ HeightProtocolEvidenceGrows
          \/ PostGstDeadlineDebtDecreases
          \/ PostGstNodeIoBlockerDebtDecreases
          \/ PostGstOverduePacketOwnershipExits
          \/ PostGstRuntimeReachDecreases
          \/ LiveProtectedServiceRankDecreaseStep
          \/ LiveProtectedServeRankDecreaseStep
BY DEF PostGstProductiveStep

OneHeightFrameAt(initialContext) ==
  /\ AsyncFrozenContextAt(initialContext)
  /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
  /\ (ResponsiveNodesApply
        <=> AsyncAllResponsiveAppliedAt(initialContext))

THEOREM FrozenContextEstablishesOneHeightFrame ==
  \A initialContext:
    AsyncFrozenContextAt(initialContext)
      => OneHeightFrameAt(initialContext)
BY FrozenContextFixesResponsiveVoters, Isa
   DEF OneHeightFrameAt, ResponsiveNodesApply,
       AsyncAllResponsiveAppliedAt

THEOREM AsyncStepPreservesOneHeightFrame ==
  \A initialContext:
    OneHeightFrameAt(initialContext)
      /\ [AsyncNext]_AsyncAllVars
      => OneHeightFrameAt(initialContext)'
PROOF
  <1>1. ASSUME NEW initialContext,
                OneHeightFrameAt(initialContext),
                [AsyncNext]_AsyncAllVars
         PROVE OneHeightFrameAt(initialContext)'
    <2>1. AsyncFrozenContextAt(initialContext)
      BY <1>1 DEF OneHeightFrameAt
    <2>2. AsyncFrozenContextAt(initialContext)'
      BY <1>1, <2>1, AsyncNextPreservesFrozenContext
    <2> QED BY <2>2, Isa
         DEF OneHeightFrameAt, AsyncFrozenContextAt,
             AsyncCurrentResponsiveVoters, AsyncVotersAt,
             CurrentVoters, CurrentEpoch, ResponsiveNodesApply,
             AsyncAllResponsiveAppliedAt
  <1> QED BY <1>1

THEOREM AsyncSpecAlwaysKeepsOneHeightFrame ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []OneHeightFrameAt(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []OneHeightFrameAt(initialContext)
    <2>1. AsyncInitAt(initialContext)
            => OneHeightFrameAt(initialContext)
      BY AsyncInitEstablishesFrozenContext,
         FrozenContextEstablishesOneHeightFrame
    <2>2. OneHeightFrameAt(initialContext)
            /\ [AsyncNext]_AsyncAllVars
            => OneHeightFrameAt(initialContext)'
      BY AsyncStepPreservesOneHeightFrame
    <2> QED BY <2>1, <2>2, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM OneHeightCompletionFromProgressProperties ==
  \A initialContext:
    /\ AsyncLiveSpecAt(initialContext)
    /\ RotatingLeaderProgressProperty(
         AsyncLiveSpecAt(initialContext))
    /\ ApplicationLivenessProperty(AsyncSpecAt(initialContext))
    => OneHeightCompletionLiveness(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext,
                /\ AsyncLiveSpecAt(initialContext)
                /\ RotatingLeaderProgressProperty(
                     AsyncLiveSpecAt(initialContext))
                /\ ApplicationLivenessProperty(
                     AsyncSpecAt(initialContext))
         PROVE OneHeightCompletionLiveness(initialContext)
    <2>0. AsyncSpecAt(initialContext)
      BY <1>1, AsyncLiveSpecProjectsAsyncSpec
    <2>2. AsyncSpecAt(initialContext) => [](gst => []gst)
      BY AsyncSpecKeepsGstOnceSet
    <2>3. []OneHeightFrameAt(initialContext)
      BY <2>0, AsyncSpecAlwaysKeepsOneHeightFrame
    <2>4. (gst /\ ~ResponsiveNodesDecide)
             ~> ResponsiveNodesDecide
      BY <1>1, <2>2, PTL DEF RotatingLeaderProgressProperty
    <2>5. (gst /\ ResponsiveNodesDecide)
             ~> ResponsiveNodesApply
      BY <1>1, <2>0 DEF ApplicationLivenessProperty
    <2>6. gst ~> (gst /\ ResponsiveNodesDecide)
      BY <1>1, <2>2, <2>4, PTL
    <2>7. gst ~> ResponsiveNodesApply
      BY <2>5, <2>6, PTL
    <2>8. [](ResponsiveNodesApply
               <=> AsyncAllResponsiveAppliedAt(initialContext))
      BY <2>3, PTL DEF OneHeightFrameAt
    <2> QED BY <1>1, <2>7, <2>8, PTL
         DEF OneHeightCompletionLiveness
  <1> QED BY <1>1

(***************************************************************************
Decision persistence is terminal for timeout production.  The induction above
covers every Core action explicitly, including crash and durable-timeout replay,
then lifts that invariant through the asynchronous scheduler bracket.  Delayed
timeout traffic remains consumable but cannot recreate control work or causal
successors after the Decision frontier is durable.
***************************************************************************)
THEOREM PostDecisionTimeoutExclusionObligation ==
  \A initialContext:
    PostDecisionTimeoutExclusionProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE PostDecisionTimeoutExclusionProperty(
                 AsyncSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
             => []DecisionTimeoutFrontierInvariant
      BY DecisionTimeoutFrontierInvariantFromAsyncSpec
    <2>2. PostDecisionTimeoutControlExcluded
      BY PostDecisionTimeoutControlGuardsAreStructural
    <2>3. PostDecisionTimeoutTrafficConsumeOnly
      BY PostDecisionTimeoutDeliveryIsConsumeOnly
    <2>4. PostDecisionTimeoutCausalSuccessorsExcluded
      BY PostDecisionTimeoutCausalSuccessorsAreEmpty
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF PostDecisionTimeoutExclusionProperty
  <1> QED BY <1>1

THEOREM DecisionRecoveryAcrossRestartObligation ==
  \A initialContext:
    DecisionRecoveryAcrossRestartProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE DecisionRecoveryAcrossRestartProperty(
                 AsyncSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
             => []DecisionFrontierUniquenessInvariant
      BY DecisionFrontierUniquenessInvariantFromAsyncSpec
    <2>2. AsyncSpecAt(initialContext) => []StrongInductiveInvariant
      BY StrongInductiveInvariantFromAsyncSpec
    <2>3. StrongInductiveInvariant
             => DurableDecisionRecoveryLifecycleTransition
      BY ExactDurableDecisionRecoveryLifecycleTransition
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF DecisionRecoveryAcrossRestartProperty
  <1> QED BY <1>1

(***************************************************************************
The abstract witness may constrain a model while production uses a different
owner identity, reset boundary, or completion ordering.  The model-side
projection below therefore records the exact already-proved facts to which the
six production traces must connect: the strong invariant, disjoint scheduler
ownership, generation-scoped delivery, post-Decision timeout exclusion,
durable-Decision recovery, and stable application receipts.  It deliberately
does not assume the still-open temporal `ProgressWitnessObligation`.

The production half additionally requires the unified V2 and lane-local
fair-ingress path to keep semantic origin distinct from the authenticated
resource-owning hop while preserving the modeled admission class, and reliable
delivery to retain the exact source, target/cursor, bytes, and occurrence
through actor, encode, frame, batch, write, and flush stages until the matching
flush acknowledgement.  Source-fidelity and mutation tests constrain these
six external propositions but cannot prove them.
***************************************************************************)
ProgressWitnessAbstractOwnerProjection ==
  /\ \A initialContext:
       AsyncSpecAt(initialContext) => []StrongInductiveInvariant
  /\ \A initialContext:
       AsyncSpecAt(initialContext) => []AsyncProgressOwnershipInvariant
  /\ \A initialContext:
       GenerationScopedVoteDeliveryProperty(AsyncSpecAt(initialContext))
  /\ \A initialContext:
       PostDecisionTimeoutExclusionProperty(AsyncSpecAt(initialContext))
  /\ \A initialContext:
       DecisionRecoveryAcrossRestartProperty(AsyncSpecAt(initialContext))
  /\ \A initialContext, node:
       AsyncSpecAt(initialContext)
         => [](NodeHasApplication(node)
                => []NodeHasApplication(node))

THEOREM ProgressWitnessAbstractOwnerProjectionObligation ==
  ProgressWitnessAbstractOwnerProjection
PROOF
  <1>1. \A initialContext:
           AsyncSpecAt(initialContext) => []StrongInductiveInvariant
    BY StrongInductiveInvariantFromAsyncSpec
  <1>2. \A initialContext:
           AsyncSpecAt(initialContext) => []AsyncProgressOwnershipInvariant
    BY AsyncSpecAlwaysProgressOwnershipInvariant
  <1>3. \A initialContext:
           GenerationScopedVoteDeliveryProperty(
             AsyncSpecAt(initialContext))
    BY GenerationScopedVoteDeliveryObligation
  <1>4. \A initialContext:
           PostDecisionTimeoutExclusionProperty(
             AsyncSpecAt(initialContext))
    BY PostDecisionTimeoutExclusionObligation
  <1>5. \A initialContext:
           DecisionRecoveryAcrossRestartProperty(
             AsyncSpecAt(initialContext))
    BY DecisionRecoveryAcrossRestartObligation
  <1>6. \A initialContext, node:
           AsyncSpecAt(initialContext)
             => [](NodeHasApplication(node)
                    => []NodeHasApplication(node))
    <2>1. ASSUME NEW initialContext, NEW node
           PROVE AsyncSpecAt(initialContext)
                   => [](NodeHasApplication(node)
                          => []NodeHasApplication(node))
      <3>1. AsyncSpecAt(initialContext)
               => [][AsyncNext]_AsyncAllVars
        BY DEF AsyncSpecAt
      <3>2. NodeHasApplication(node)
               /\ [AsyncNext]_AsyncAllVars
              => NodeHasApplication(node)'
        BY AsyncBracketStepPreservesNodeApplication
      <3> QED BY <3>1, <3>2, PTL
    <2> QED BY <2>1
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6
       DEF ProgressWitnessAbstractOwnerProjection

(***************************************************************************
Composition debt beyond the production-checked EnterView selection relation.

The proposition requires production EnterView to select the post-install
effective lock; Fetch and queued BodyAvailable owners to preserve immutable
acquisition identity while advancing only their reducer consumer; Store and
Validate owners to detach obsolete consumers; every superseded request,
ready-completion, work, byte, and queue-capacity owner to retire exactly once;
and the rebound FetchBody -> BodyAvailable -> StoreBody -> ValidateBody chain
to refine the asynchronous fairness model.  The bounded-service theorem is
conditional on at least one class being ready; the production no-ready call is
accepted only as an idle cursor-preserving step and makes no service-progress
claim.  The four production propositions are supplied only by source-bound
Rust/Verus evidence.
***************************************************************************)
EffectiveLockBodyAcquisitionProductionRefinementObligation ==
  /\ ProductionEffectiveLockBodyAcquisitionRefinement
  /\ EffectiveLockAcquisitionModelObligation

THEOREM EffectiveLockBodyAcquisitionCrossToolRefinement ==
  ProductionEffectiveLockBodyAcquisitionRefinement
    => EffectiveLockBodyAcquisitionProductionRefinementObligation
PROOF
  BY EffectiveLockAcquisitionModelObligation
     DEF EffectiveLockBodyAcquisitionProductionRefinementObligation

THEOREM ProtectedStage4RankProgressFromFairScheduler ==
  \A initialContext:
    Stage4RefinementFiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
      => ProtectedStage4RankProgressProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                Stage4RefinementFiniteServeEpisodeResidualProperty(
                  AsyncSpecAt(initialContext))
         PROVE ProtectedStage4RankProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME NEW candidate \in AsyncCandidateSet,
                  NEW position \in Nat
           PROVE AsyncSpecAt(initialContext)
                   => ((gst
                          /\ ResponsiveProtectedCandidateOwned(candidate)
                          /\ CandidateServiceRank(candidate)
                               = <<4, position>>)
                         ~> (~ResponsiveProtectedCandidateOwned(candidate)
                              \/ ServiceRankLess(
                                   CandidateServiceRank(candidate),
                                   <<4, position>>)))
      <3>1. AsyncSpecAt(initialContext)
               => (ProtectedOwnedAtServiceRank(
                     candidate, <<4, position>>)
                     ~> ProtectedRankProgressExit(
                          candidate, <<4, position>>))
        BY FairProtectedStage4RankDescent
      <3> QED BY <3>1, PTL
           DEF ProtectedOwnedAtServiceRank,
               ProtectedRankProgressExit,
               ProtectedServiceOwnershipExit
    <2> QED BY <2>1 DEF ProtectedStage4RankProgressProperty
  <1> QED BY <1>1

THEOREM ProtectedStage5RankProgressFromFairFifo ==
  \A initialContext:
    ProtectedStage5RankProgressProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ProtectedStage5RankProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME NEW candidate \in AsyncCandidateSet,
                  NEW position \in Nat
           PROVE AsyncSpecAt(initialContext)
                   => ((gst
                          /\ ResponsiveProtectedCandidateOwned(candidate)
                          /\ CandidateServiceRank(candidate)
                               = <<5, position>>)
                         ~> (~ResponsiveProtectedCandidateOwned(candidate)
                              \/ ServiceRankLess(
                                   CandidateServiceRank(candidate),
                                   <<5, position>>)))
      <3>1. AsyncSpecAt(initialContext)
               => (ProtectedOwnedAtServiceRank(
                     candidate, <<5, position>>)
                     ~> ProtectedRankProgressExit(
                          candidate, <<5, position>>))
        BY FairProtectedStage5RankDescent
      <3> QED BY <3>1, PTL
           DEF ProtectedOwnedAtServiceRank,
               ProtectedRankProgressExit,
               ProtectedServiceOwnershipExit
    <2> QED BY <2>1 DEF ProtectedStage5RankProgressProperty
  <1> QED BY <1>1

=============================================================================
