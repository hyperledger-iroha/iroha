---- MODULE SumeragiV2Stage6CapacityScratch ----
EXTENDS SumeragiV2Stage6CausalRankStrengthenedScratch

(***************************************************************************
Scratch temporal shell for a protected causal owner blocked by a
non-Completion command-prefix reservation.  The outer debt and inner runner
rank are the already checked Stage-4 capacity ordering; only the ownership
goal changes from a ready completion to the exact stage-6 causal candidate.
***************************************************************************)

Stage6NonCompletionCapacityBlockedAtRank(candidate, position, rank) ==
  /\ ProtectedStage6Pending(candidate, position)
  /\ NonCompletionCausalAdmissionDebt(candidate.node)
  /\ ~CausalHeadCanAdvance(candidate.node)
  /\ Stage4CapacityRank(candidate.node) = rank

Stage6NonCompletionCapacityGoal(candidate, position) ==
  \/ ProtectedRankProgressExit(candidate, <<6, position>>)
  \/ CausalHeadCanAdvance(candidate.node)

Stage6NonCompletionCapacityProgress(candidate, position, rank) ==
  \/ Stage6NonCompletionCapacityGoal(candidate, position)
  \/ \E lower \in SetLessThan(
       rank, Stage4CapacityOrdering, Stage4CapacityCarrier):
       Stage6NonCompletionCapacityBlockedAtRank(
         candidate, position, lower)

Stage6NonCompletionCapacityStrictResult(candidate, position, rank) ==
  Stage6NonCompletionCapacityProgress(candidate, position, rank)'

Stage6NonCompletionCapacityStepResult(candidate, position, rank) ==
  \/ Stage6NonCompletionCapacityStrictResult(candidate, position, rank)
  \/ Stage6NonCompletionCapacityBlockedAtRank(
       candidate, position, rank)'

THEOREM Stage6NonCompletionCapacityBlockedCoreFacts ==
  \A candidate, position, rank:
    Stage6NonCompletionCapacityBlockedAtRank(candidate, position, rank)
      => /\ candidate.node \in ValidatorIds
         /\ candidate.node \in AsyncCurrentResponsiveVoters
         /\ AsyncStrongTypeInvariant
         /\ AsyncTypeInvariant
         /\ AsyncProgressOwnershipInvariant
         /\ NonCompletionCausalAdmissionDebt(candidate.node)
         /\ ~CausalHeadCanAdvance(candidate.node)
PROOF
  <1>1. ASSUME NEW candidate, NEW position, NEW rank,
                Stage6NonCompletionCapacityBlockedAtRank(
                  candidate, position, rank)
         PROVE /\ candidate.node \in ValidatorIds
               /\ candidate.node \in AsyncCurrentResponsiveVoters
               /\ AsyncStrongTypeInvariant
               /\ AsyncTypeInvariant
               /\ AsyncProgressOwnershipInvariant
               /\ NonCompletionCausalAdmissionDebt(candidate.node)
               /\ ~CausalHeadCanAdvance(candidate.node)
    <2>1. ProtectedStage6Pending(candidate, position)
      BY <1>1 DEF Stage6NonCompletionCapacityBlockedAtRank
    <2>2. /\ candidate.node \in ValidatorIds
           /\ candidate.node \in AsyncCurrentResponsiveVoters
      BY <2>1, ProtectedStage6CarrierFacts
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
      BY <2>1 DEF ProtectedStage6Pending
    <2>4. AsyncTypeInvariant
      BY <2>3, AsyncStrongTypeProjectsAsyncType
    <2> QED BY <1>1, <2>2, <2>3, <2>4
         DEF Stage6NonCompletionCapacityBlockedAtRank
  <1> QED BY <1>1

THEOREM Stage6NonCompletionCapacityLocalAdmissionStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage6NonCompletionCapacityBlockedAtRank(
         candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ PostGstRunNode(candidate.node)
    /\ LocalAdmissionStep(candidate.node)
    => Stage6NonCompletionCapacityStrictResult(
         candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   LocalAdmissionStrictlyDecreasesRuntimeReach,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage6NonCompletionCapacityStrictResult,
       Stage6NonCompletionCapacityProgress,
       Stage6NonCompletionCapacityGoal,
       Stage6NonCompletionCapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ProtectedStage6Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCandidatePosition, LocalSourceDistance,
       PreferredLocalSource, CausalCommandCapacityDebt,
       CausalHeadCommandLimit, NonCompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       LocalAdmissionStep, LocalAdmissionCanAdvance,
       ProducerCompletionCanAdvance, ProducerCompletionCanAdmit,
       RecordBlockedCausalDebt, CandidateScheduled,
       CandidateInFlight, CausalHeadCanAdvance, CanEnqueueClass,
       AsyncQueueDepth, NodeQueueNonempty, CausalCandidates,
       SequenceSet, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

THEOREM Stage6NonCompletionCapacityIngressStrictlyProgresses ==
  \A candidate, position:
    \A rank \in Stage4CapacityCarrier:
    /\ Stage6NonCompletionCapacityBlockedAtRank(
         candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ PostGstRunNode(candidate.node)
    /\ IngressDrainStep(candidate.node)
    => Stage6NonCompletionCapacityStrictResult(
         candidate, position, rank)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   IngressDrainStrictlyDecreasesRuntimeReach,
   Stage4CapacityRankInCarrier, Isa
   DEF Stage6NonCompletionCapacityStrictResult,
       Stage6NonCompletionCapacityProgress,
       Stage6NonCompletionCapacityGoal,
       Stage6NonCompletionCapacityBlockedAtRank,
       Stage4CapacityRank, Stage4CapacityOrdering,
       Stage4CapacityCarrier, ProtectedStage6Pending,
       ProtectedOwnedAtServiceRank, ProtectedRankProgressExit,
       ProtectedServiceOwnershipExit,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       CandidateServiceRank, ServiceRankLess,
       CausalCandidatePosition, LocalSourceDistance,
       PreferredLocalSource, CausalCommandCapacityDebt,
       CausalHeadCommandLimit, NonCompletionCausalAdmissionDebt,
       CausalAdmissionDebtActive, ReadyRunAuxRank,
       ReadyRunDeferredRank, ReadyRunTimeoutRank, ReadyRunInnerRank,
       ReadyRunAuxOrdering, ReadyRunAuxCarrier,
       ReadyRunDeferredOrdering, ReadyRunDeferredCarrier,
       ReadyRunTimeoutOrdering, ReadyRunTimeoutCarrier,
       ReadyRunInnerOrdering, ReadyRunInnerCarrier,
       ReadyFifoDebt, ReadyDeferredCount, ReadyTimeoutDebt,
       ReadyTagDrainDebt, ReadyTagCount, RuntimeReachRank,
       IngressDrainStep, DrainFairIngressSelected,
       IngressItemCanDrain, PopSelectedIngress,
       CandidateScheduled, CandidateInFlight, CausalHeadCanAdvance,
       CanEnqueueClass, AsyncQueueDepth, NodeQueueNonempty,
       CausalCandidates, SequenceSet,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncOutstandingCarrierInvariant, AsyncAllVars

=============================================================================
