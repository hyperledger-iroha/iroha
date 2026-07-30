---- MODULE SumeragiV2AsyncCausalWorkBudgetProofs ----
EXTENDS SumeragiV2AsyncInstallRunnerContinuationProofs

(***************************************************************************
Finite causal-successor work.

`CommandSuccessors` is a branching relation: one serialized reducer command
can append as many as three children.  Cardinality of the live candidate set
is therefore not a progress rank by itself.  The closed command inventory is,
however, acyclic.  The stage below is a topological height of that exact
inventory.  Every child has a smaller stage, including all conditional
Decision-recovery and installed-TC successors.

The weight uses radix four because the exact fanout bound is three.  A parent
therefore owns strictly more remaining-work credit than the sum of every
possible successor batch.  This is structural reducer work only: scheduler
fairness is still required to service the selected parent, and creating a
child is never classified as progress.
***************************************************************************)

AsyncCausalRemainingWorkStage(kind) ==
  CASE kind = "BeginTimeout" -> 9
    [] kind \in
         {"DeliverVote", "DeliverQC", "DeliverTC",
          "PersistTimeout"} -> 8
    [] kind \in
         {"FormCommitQC", "BeginDecision", "BeginInstallTC",
          "SignTimeout", "DeliverTimeout"} -> 7
    [] kind \in
         {"DeliverProposal", "DeliverChunk", "PersistDecision",
          "PersistInstallTC"} -> 6
    [] kind \in
         {"FetchBody", "RebindRetainedBody",
          "FetchCertifiedBody"} -> 5
    [] kind \in {"StoreBody", "BeginObservePrepare"} -> 4
    [] kind \in
         {"AssembleBody", "ValidateBody",
          "PersistObservePrepare"} -> 3
    [] kind \in
         {"BeginProposal", "BeginPrepare", "BeginLockCommit"} -> 2
    [] kind \in
         {"PersistProposal", "PersistPrepare",
          "PersistLockCommit"} -> 1
    [] OTHER -> 0

AsyncCausalRemainingWorkStageCarrier == 0..9

AsyncCausalRemainingWorkStageOrdering ==
  OpToRel(<, AsyncCausalRemainingWorkStageCarrier)

THEOREM AsyncCausalRemainingWorkStageIsTyped ==
  \A kind \in AsyncWorkKinds:
    AsyncCausalRemainingWorkStage(kind)
      \in AsyncCausalRemainingWorkStageCarrier
BY SMT
   DEF AsyncCausalRemainingWorkStage,
       AsyncCausalRemainingWorkStageCarrier,
       AsyncWorkKinds, AsyncCompletionTags,
       AsyncDeliveryKinds, AsyncReducerKinds

THEOREM AsyncCausalRemainingWorkStageOrderingIsWellFounded ==
  IsWellFoundedOn(
    AsyncCausalRemainingWorkStageOrdering,
    AsyncCausalRemainingWorkStageCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset
   DEF AsyncCausalRemainingWorkStageOrdering,
       AsyncCausalRemainingWorkStageCarrier

THEOREM AsyncCommandSuccessorsStrictlyLowerRemainingWorkStage ==
  \A command \in AsyncCandidateSet:
    \A successor \in SequenceSet(CommandSuccessors(command)):
      AsyncCausalRemainingWorkStage(successor.kind)
        < AsyncCausalRemainingWorkStage(command.kind)
BY SMTT(180)
   DEF AsyncCausalRemainingWorkStage,
       CommandSuccessors, CausalCandidate,
       CausalCandidateWithEvidence, RetainedBodyRebindCandidate,
       PersistDecisionRecoveryKind, PersistDecisionRecoverySuccessor,
       InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallLockedFetchSuccessor,
       InstallCommitSignSuccessors, InstallCommitSignSuccessor,
       InstallProposalSuccessor, AsyncCandidateFrom,
       AsyncCandidateAtConsumerWithOrigin,
       AsyncCandidateWithIdentityAndOrigin, SequenceSet

(***************************************************************************
Exact remaining command occurrences.

The radix-four weight below is convenient for a generic well-foundedness
argument, but it is intentionally too coarse for the fixed-corridor clock.
This second weight is the exact maximum size of the closed successor subtree,
including its root.  The largest root is BeginTimeout:

  BeginTimeout -> PersistTimeout -> SignTimeout -> PersistInstallTC

and the final install can emit the locked FetchBody tail, one SignVote, and
the next-view AssembleBody tail.  Expanding every exact branch gives nineteen
occurrences.  No arbitrary producer constant is introduced: the source
checker pins this CASE table to `CommandSuccessors` and its closed parent-kind
inventory.
***************************************************************************)
AsyncCausalExactRemainingOccurrenceBudget(kind) ==
  CASE kind = "BeginTimeout" -> 19
    [] kind \in {"PersistTimeout", "DeliverTC"} -> 18
    [] kind \in
         {"SignTimeout", "DeliverTimeout", "BeginInstallTC"} -> 17
    [] kind = "PersistInstallTC" -> 16
    [] kind = "DeliverProposal" -> 14
    [] kind \in {"DeliverVote", "DeliverQC"} -> 13
    [] kind \in {"FormCommitQC", "BeginDecision"} -> 12
    [] kind \in {"DeliverChunk", "PersistDecision"} -> 11
    [] kind \in
         {"FetchBody", "RebindRetainedBody",
          "FetchCertifiedBody"} -> 10
    [] kind = "StoreBody" -> 9
    [] kind = "ValidateBody" -> 8
    [] kind = "BeginObservePrepare" -> 5
    [] kind \in {"AssembleBody", "PersistObservePrepare"} -> 4
    [] kind \in
         {"BeginProposal", "BeginPrepare", "BeginLockCommit"} -> 3
    [] kind \in
         {"PersistProposal", "PersistPrepare",
          "PersistLockCommit"} -> 2
    [] OTHER -> 1

THEOREM AsyncCausalExactRemainingOccurrenceBudgetIsBounded ==
  \A kind \in AsyncWorkKinds:
    AsyncCausalExactRemainingOccurrenceBudget(kind) \in 1..19
BY SMT
   DEF AsyncCausalExactRemainingOccurrenceBudget,
       AsyncWorkKinds, AsyncCompletionTags,
       AsyncDeliveryKinds, AsyncReducerKinds

AsyncCommandExactSuccessorBatchOccurrenceBudget(command) ==
  LET successors == CommandSuccessors(command)
  IN CASE Len(successors) = 0 -> 0
       [] Len(successors) = 1 ->
            AsyncCausalExactRemainingOccurrenceBudget(successors[1].kind)
       [] Len(successors) = 2 ->
            AsyncCausalExactRemainingOccurrenceBudget(successors[1].kind)
              + AsyncCausalExactRemainingOccurrenceBudget(
                  successors[2].kind)
       [] Len(successors) = 3 ->
            AsyncCausalExactRemainingOccurrenceBudget(successors[1].kind)
              + AsyncCausalExactRemainingOccurrenceBudget(
                  successors[2].kind)
              + AsyncCausalExactRemainingOccurrenceBudget(
                  successors[3].kind)
       [] OTHER ->
            AsyncCausalExactRemainingOccurrenceBudget(command.kind)

THEOREM AsyncCommandExactSuccessorBatchStrictlyConsumesOccurrenceBudget ==
  \A command \in AsyncCandidateSet:
    AsyncCommandExactSuccessorBatchOccurrenceBudget(command)
      < AsyncCausalExactRemainingOccurrenceBudget(command.kind)
BY CommandSuccessorsHaveBoundedLength,
   SMTT(300)
   DEF AsyncCommandExactSuccessorBatchOccurrenceBudget,
       AsyncCausalExactRemainingOccurrenceBudget,
       CommandSuccessors, CausalCandidate,
       CausalCandidateWithEvidence, RetainedBodyRebindCandidate,
       PersistDecisionRecoveryKind, PersistDecisionRecoverySuccessor,
       InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallLockedFetchSuccessor,
       InstallCommitSignSuccessors, InstallCommitSignSuccessor,
       InstallProposalSuccessor, AsyncCandidateFrom,
       AsyncCandidateAtConsumerWithOrigin,
       AsyncCandidateWithIdentityAndOrigin,
       SequenceSet, AsyncCandidateSet

AsyncCausalRemainingWorkWeight(kind) ==
  CASE AsyncCausalRemainingWorkStage(kind) = 9 -> 262144
    [] AsyncCausalRemainingWorkStage(kind) = 8 -> 65536
    [] AsyncCausalRemainingWorkStage(kind) = 7 -> 16384
    [] AsyncCausalRemainingWorkStage(kind) = 6 -> 4096
    [] AsyncCausalRemainingWorkStage(kind) = 5 -> 1024
    [] AsyncCausalRemainingWorkStage(kind) = 4 -> 256
    [] AsyncCausalRemainingWorkStage(kind) = 3 -> 64
    [] AsyncCausalRemainingWorkStage(kind) = 2 -> 16
    [] AsyncCausalRemainingWorkStage(kind) = 1 -> 4
    [] OTHER -> 1

THEOREM AsyncCausalRemainingWorkWeightIsPositive ==
  \A kind \in AsyncWorkKinds:
    AsyncCausalRemainingWorkWeight(kind) \in Nat \ {0}
BY AsyncCausalRemainingWorkStageIsTyped, SMT
   DEF AsyncCausalRemainingWorkWeight,
       AsyncCausalRemainingWorkStageCarrier

THEOREM AsyncCommandSuccessorConsumesAtMostOneWeightQuarter ==
  \A command \in AsyncCandidateSet:
    \A successor \in SequenceSet(CommandSuccessors(command)):
      4 * AsyncCausalRemainingWorkWeight(successor.kind)
        <= AsyncCausalRemainingWorkWeight(command.kind)
BY AsyncCommandSuccessorsStrictlyLowerRemainingWorkStage, SMT
   DEF AsyncCausalRemainingWorkWeight,
       AsyncCausalRemainingWorkStageCarrier

AsyncCommandSuccessorBatchRemainingWork(command) ==
  LET successors == CommandSuccessors(command)
  IN CASE Len(successors) = 0 -> 0
       [] Len(successors) = 1 ->
            AsyncCausalRemainingWorkWeight(successors[1].kind)
       [] Len(successors) = 2 ->
            AsyncCausalRemainingWorkWeight(successors[1].kind)
              + AsyncCausalRemainingWorkWeight(successors[2].kind)
       [] Len(successors) = 3 ->
            AsyncCausalRemainingWorkWeight(successors[1].kind)
              + AsyncCausalRemainingWorkWeight(successors[2].kind)
              + AsyncCausalRemainingWorkWeight(successors[3].kind)
       [] OTHER -> AsyncCausalRemainingWorkWeight(command.kind)

THEOREM AsyncCommandSuccessorBatchStrictlyConsumesRemainingWork ==
  \A command \in AsyncCandidateSet:
    AsyncCommandSuccessorBatchRemainingWork(command)
      < AsyncCausalRemainingWorkWeight(command.kind)
BY CommandSuccessorsHaveBoundedLength,
   AsyncCommandSuccessorConsumesAtMostOneWeightQuarter,
   AsyncCausalRemainingWorkWeightIsPositive, SMTT(180)
   DEF AsyncCommandSuccessorBatchRemainingWork,
       SequenceSet, AsyncCandidateSet,
       AsyncCandidateTyped, AsyncWorkKinds,
       AsyncCompletionTags, AsyncDeliveryKinds, AsyncReducerKinds

(***************************************************************************
Frozen protected-runner episode.

The command weight above is useful only after fixing the owner universe.  A
protected candidate supplies that cut: its immutable local lifecycle ordinal
is shared with exact Serve ingress admissions.  Candidate roots at or below
the cut retain their causal origin across every carrier transfer, while every
successor has smaller topological weight.  Exact Serve tickets at or below
the same cut retain one ingress record, one frozen per-source prefix, and one
atomic future-slot reservation.  A retry coalesces with that lifecycle; after
drain it receives a strictly later shared ordinal and cannot re-enter this
episode.  The retained Serve tombstone prevents the drained logical request
from recreating its retired I/O job.

`AsyncCausalEpisodeStructuralRank` is not a service-progress rank.  Its first
component charges the finite Candidate producer episode, its second charges
the frozen Serve occurrence and predecessor prefixes, and its final component
charges only the deterministic runner path to the next drainable ingress
turn.  Replenishment is therefore represented as strict consumption of a
finite producer episode.  Only the higher Stage-3/4/6 composition may turn
exhaustion of this rank into occurrence-rank descent.
***************************************************************************)

AsyncCausalEpisodeFrozenPredecessorOrigins(node, cutoffOrdinal) ==
  {record.origin:
     record \in AsyncCandidateLifecycleAdmissions,
     /\ record.node = node
     /\ record.ordinal <= cutoffOrdinal}

\* The cut is owned by the immutable lifecycle admission, rather than by one
\* transient parent command.  A serviced parent may disappear while its
\* causal child is appended in the same transition; the admission survives
\* that handoff with the same origin, slot, and ordinal.
AsyncCausalEpisodeLifecycleCutOwned(node, origin, cutoffOrdinal) ==
  \E record \in AsyncCandidateLifecycleAdmissions:
    /\ record.node = node
    /\ record.origin = origin
    /\ record.ordinal = cutoffOrdinal

AsyncCausalEpisodeCandidates(node, cutoffOrdinal) ==
  {candidate \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates:
     /\ candidate.node = node
     /\ candidate.causalOrigin
          \in AsyncCausalEpisodeFrozenPredecessorOrigins(
               node, cutoffOrdinal)
     /\ AsyncCandidateLifecycleOrdinal(candidate) <= cutoffOrdinal}

AsyncCausalEpisodeCandidateWorkTokens(node, cutoffOrdinal) ==
  {<<candidate, token>>:
     candidate \in AsyncCausalEpisodeCandidates(node, cutoffOrdinal),
     token \in 1..AsyncCausalRemainingWorkWeight(candidate.kind)}

AsyncCausalEpisodeCandidateWorkBudget(node, cutoffOrdinal) ==
  Cardinality(
    AsyncCausalEpisodeCandidateWorkTokens(node, cutoffOrdinal))

AsyncCausalEpisodeExactCandidateOccurrenceTokens(node, cutoffOrdinal) ==
  {<<candidate, token>>:
     candidate \in AsyncCausalEpisodeCandidates(node, cutoffOrdinal),
     token
       \in 1..AsyncCausalExactRemainingOccurrenceBudget(candidate.kind)}

AsyncCausalEpisodeExactCandidateOccurrenceBudget(node, cutoffOrdinal) ==
  Cardinality(
    AsyncCausalEpisodeExactCandidateOccurrenceTokens(
      node, cutoffOrdinal))

THEOREM AsyncCausalEpisodeCandidateCarrierHasConfiguredBound ==
  \A node \in ValidatorIds, cutoffOrdinal \in Nat:
    AsyncStrongTypeInvariant
      => /\ IsFiniteSet(
               AsyncCausalEpisodeCandidates(node, cutoffOrdinal))
         /\ Cardinality(
              AsyncCausalEpisodeCandidates(node, cutoffOrdinal))
              <= AsyncCandidateProducerEpisodeCapacity
BY FS_Interval, FS_Image, FS_Union, FS_Subset,
   FS_CardinalityType, IsaT(1800)
   DEF AsyncCausalEpisodeCandidates,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncCandidateProducerEpisodeCapacity,
       QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       SequenceSet, AsyncQueueDepth,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoCapacityTypeInvariant,
       AsyncIoContentTypeInvariant, AsyncIoWorkContentTypeInvariant,
       AsyncDeferredTypeInvariant, AsyncDeferredContentTypeInvariant

THEOREM AsyncCausalEpisodeExactOccurrenceBudgetFitsConfiguredEpisode ==
  \A node \in ValidatorIds, cutoffOrdinal \in Nat:
    AsyncStrongTypeInvariant
      => /\ IsFiniteSet(
               AsyncCausalEpisodeExactCandidateOccurrenceTokens(
                 node, cutoffOrdinal))
         /\ AsyncCausalEpisodeExactCandidateOccurrenceBudget(
              node, cutoffOrdinal)
              <= AsyncCandidateProducerEpisodeBudget
BY AsyncCausalEpisodeCandidateCarrierHasConfiguredBound,
   AsyncCausalExactRemainingOccurrenceBudgetIsBounded,
   FS_Interval, FS_Product, FS_Subset,
   FS_CardinalityType, IsaT(1200)
   DEF AsyncCausalEpisodeExactCandidateOccurrenceBudget,
       AsyncCausalEpisodeExactCandidateOccurrenceTokens,
       AsyncCandidateProducerEpisodeBudget

AsyncCausalEpisodeServeIngressIdentities(node, cutoffOrdinal) ==
  {identity \in AsyncServeIngressLifecycleOwnerIdentities(node):
     /\ AsyncServeIngressAdmissionOwned(node, identity)
     /\ AsyncServeIngressAdmissionSchedulerOrdinal(node, identity)
          <= cutoffOrdinal}

AsyncCausalEpisodeServeIngressPrefixTokens(node, cutoffOrdinal) ==
  UNION {
    {<<"ServeIngress", identity, slot>>:
       slot \in
         AsyncServeIngressAdmissionPredecessorDebtSlots(node, identity)}:
    identity \in
      AsyncCausalEpisodeServeIngressIdentities(node, cutoffOrdinal)}

AsyncCausalEpisodeServeIoPredecessorTokens(node, cutoffOrdinal) ==
  UNION {
    {<<"ServeIo", identity, job>>:
       job \in AsyncServeFrozenPredecessorSet(node, identity)}:
    identity \in
      AsyncCausalEpisodeServeIngressIdentities(node, cutoffOrdinal)}

AsyncCausalEpisodeServeOccurrenceTokens(node, cutoffOrdinal) ==
  {<<"ServeOccurrence", identity>>:
     identity \in
       AsyncCausalEpisodeServeIngressIdentities(node, cutoffOrdinal)}

AsyncCausalEpisodeServeWorkTokens(node, cutoffOrdinal) ==
  AsyncCausalEpisodeServeOccurrenceTokens(node, cutoffOrdinal)
    \cup AsyncCausalEpisodeServeIngressPrefixTokens(node, cutoffOrdinal)
    \cup AsyncCausalEpisodeServeIoPredecessorTokens(node, cutoffOrdinal)

AsyncCausalEpisodeServeWorkBudget(node, cutoffOrdinal) ==
  Cardinality(AsyncCausalEpisodeServeWorkTokens(node, cutoffOrdinal))

AsyncCausalEpisodeServeReachDebt(node, cutoffOrdinal) ==
  IF AsyncCausalEpisodeServeIngressIdentities(node, cutoffOrdinal) = {}
  THEN 0
  ELSE DrainableIngressTurnReachRank(node)

AsyncCausalEpisodeStructuralRank(node, cutoffOrdinal) ==
  <<AsyncCausalEpisodeCandidateWorkBudget(node, cutoffOrdinal),
    <<AsyncCausalEpisodeServeWorkBudget(node, cutoffOrdinal),
      AsyncCausalEpisodeServeReachDebt(node, cutoffOrdinal)>>>

AsyncCausalEpisodeServeRankCarrier == Nat \X Nat
AsyncCausalEpisodeStructuralRankCarrier ==
  Nat \X AsyncCausalEpisodeServeRankCarrier

AsyncCausalEpisodeServeRankOrdering ==
  LexPairOrdering(OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)

AsyncCausalEpisodeStructuralRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), AsyncCausalEpisodeServeRankOrdering,
    Nat, AsyncCausalEpisodeServeRankCarrier)

THEOREM AsyncCausalEpisodeStructuralRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AsyncCausalEpisodeStructuralRankOrdering,
    AsyncCausalEpisodeStructuralRankCarrier)
BY NatLessThanWellFounded, WFLexPairOrdering
   DEF AsyncCausalEpisodeStructuralRankOrdering,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankOrdering,
       AsyncCausalEpisodeServeRankCarrier

THEOREM AsyncCausalEpisodeStructuralRankIsFinite ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ProtectedCandidateOwned(candidate)
    => LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
       IN /\ IsFiniteSet(
                 AsyncCausalEpisodeFrozenPredecessorOrigins(
                   candidate.node, cutoffOrdinal))
          /\ IsFiniteSet(
                 AsyncCausalEpisodeCandidateWorkTokens(
                   candidate.node, cutoffOrdinal))
          /\ IsFiniteSet(
                 AsyncCausalEpisodeExactCandidateOccurrenceTokens(
                   candidate.node, cutoffOrdinal))
          /\ IsFiniteSet(
                 AsyncCausalEpisodeServeWorkTokens(
                   candidate.node, cutoffOrdinal))
          /\ AsyncCausalEpisodeStructuralRank(
               candidate.node, cutoffOrdinal)
               \in AsyncCausalEpisodeStructuralRankCarrier
BY AsyncCausalRemainingWorkWeightIsPositive,
   DrainableIngressTurnReachRankIsNatural,
   FS_Image, FS_Product, FS_Union, FS_Subset, FS_Interval,
   FS_CardinalityType, IsaT(600)
   DEF AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncCausalEpisodeCandidates,
       AsyncCausalEpisodeCandidateWorkTokens,
       AsyncCausalEpisodeExactCandidateOccurrenceTokens,
       AsyncCausalExactRemainingOccurrenceBudget,
       AsyncCausalEpisodeServeIngressIdentities,
       AsyncCausalEpisodeServeIngressPrefixTokens,
       AsyncCausalEpisodeServeIoPredecessorTokens,
       AsyncCausalEpisodeServeOccurrenceTokens,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeStructuralRank,
       AsyncCausalEpisodeCandidateWorkBudget,
       AsyncCausalEpisodeServeWorkBudget,
       AsyncCausalEpisodeServeReachDebt,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankCarrier,
       ProtectedCandidateOwned,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncServeLifecycleTypeInvariant

THEOREM AsyncCausalEpisodeTargetLifecycleOrdinalPersists ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ProtectedCandidateOwned(candidate)
    /\ [AsyncNext]_AsyncAllVars
    /\ ProtectedCandidateOwned(candidate)'
    => AsyncCandidateLifecycleOrdinal(candidate)'
         = AsyncCandidateLifecycleOrdinal(candidate)
BY AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   IsaT(600)
   DEF ProtectedCandidateOwned, CandidateScheduled,
       AsyncCandidateLifecycleOrdinal,
       AsyncCandidateLifecycleRecordsFor,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncAllVars

(***************************************************************************
Cross-child lifecycle carry.

This is the boundary the per-candidate service rank cannot express: the
parent may leave every physical carrier while a same-origin child is appended.
The scheduler-coverage invariant and the atomic carrier update keep the
existing lifecycle admission active, so the child inherits the exact cut
instead of receiving a fresh ordinal.
***************************************************************************)
THEOREM AsyncCausalEpisodeSameOriginHandoffRetainsLifecycleCut ==
  \A parent, child \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(parent)
    IN /\ gst
       /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateLifecycleSchedulerCoverageInvariant
       /\ CandidateScheduled(parent)
       /\ [AsyncNext]_AsyncAllVars
       /\ AsyncCandidateLifecycleSchedulerCoverageInvariant'
       /\ child.node = parent.node
       /\ child.causalOrigin = parent.causalOrigin
       /\ CandidateScheduled(child)'
       => /\ AsyncCausalEpisodeLifecycleCutOwned(
                parent.node, parent.causalOrigin, cutoffOrdinal)
          /\ (AsyncCausalEpisodeLifecycleCutOwned(
                parent.node, parent.causalOrigin, cutoffOrdinal))'
          /\ AsyncCandidateLifecycleOrdinal(child)' = cutoffOrdinal
BY AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   IsaT(1800)
   DEF AsyncCausalEpisodeLifecycleCutOwned,
       AsyncCandidateLifecycleSchedulerCoverageInvariant,
       AsyncCandidateLifecycleRecordCoversScheduledOrigin,
       AsyncCandidateLifecycleActiveRecords,
       AsyncCandidateLifecycleOrdinal,
       AsyncCandidateLifecycleRecordsFor,
       AsyncScheduledCandidateOriginsForNode,
       AsyncControlServiceSlotTransition,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleCarrierUpdatedAdmissions,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleRetirementCoveredIn,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleNewAdmissions,
       AsyncNewCandidateLifecycleOriginsForNodeIn,
       AsyncCandidateLifecycleOriginsRecordedForNodeIn,
       AsyncAllVars

THEOREM AsyncCausalEpisodeFrozenOriginsCannotReplenish ==
  \A candidate \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ProtectedCandidateOwned(candidate)
       /\ [AsyncNext]_AsyncAllVars
       /\ ProtectedCandidateOwned(candidate)'
       => AsyncCausalEpisodeFrozenPredecessorOrigins(
            candidate.node, cutoffOrdinal)'
            \subseteq
              AsyncCausalEpisodeFrozenPredecessorOrigins(
                candidate.node, cutoffOrdinal)
BY AsyncCausalEpisodeTargetLifecycleOrdinalPersists,
   AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   IsaT(900)
   DEF AsyncCausalEpisodeFrozenPredecessorOrigins,
       ProtectedCandidateOwned, CandidateScheduled,
       AsyncAllVars

THEOREM AsyncCausalEpisodeOwnedLifecycleCutCannotReplenish ==
  \A node \in ValidatorIds, origin, cutoffOrdinal \in Nat \ {0}:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCausalEpisodeLifecycleCutOwned(
         node, origin, cutoffOrdinal)
    /\ [AsyncNext]_AsyncAllVars
    /\ (AsyncCausalEpisodeLifecycleCutOwned(
          node, origin, cutoffOrdinal))'
      => (AsyncCausalEpisodeFrozenPredecessorOrigins(
             node, cutoffOrdinal))'
           \subseteq
             AsyncCausalEpisodeFrozenPredecessorOrigins(
               node, cutoffOrdinal)
BY AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   IsaT(1200)
   DEF AsyncCausalEpisodeLifecycleCutOwned,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncAllVars

THEOREM AsyncCausalEpisodeServeCutCannotReplenish ==
  \A candidate \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ProtectedCandidateOwned(candidate)
       /\ [AsyncNext]_AsyncAllVars
       /\ ProtectedCandidateOwned(candidate)'
       => AsyncCausalEpisodeServeIngressIdentities(
            candidate.node, cutoffOrdinal)'
            \subseteq
              AsyncCausalEpisodeServeIngressIdentities(
                candidate.node, cutoffOrdinal)
BY AsyncCausalEpisodeTargetLifecycleOrdinalPersists,
   AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal,
   AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   IsaT(900)
   DEF AsyncCausalEpisodeServeIngressIdentities,
       AsyncFreshServeIngressAdmissionsForNodeThisStep,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionSchedulerOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       ProtectedCandidateOwned, CandidateScheduled,
       AsyncAllVars

THEOREM AsyncCausalEpisodeOwnedLifecycleServeCutCannotReplenish ==
  \A node \in ValidatorIds, origin, cutoffOrdinal \in Nat \ {0}:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCausalEpisodeLifecycleCutOwned(
         node, origin, cutoffOrdinal)
    /\ [AsyncNext]_AsyncAllVars
    /\ (AsyncCausalEpisodeLifecycleCutOwned(
          node, origin, cutoffOrdinal))'
      => (AsyncCausalEpisodeServeIngressIdentities(
             node, cutoffOrdinal))'
           \subseteq
             AsyncCausalEpisodeServeIngressIdentities(
               node, cutoffOrdinal)
BY AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal,
   AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   IsaT(1200)
   DEF AsyncCausalEpisodeLifecycleCutOwned,
       AsyncCausalEpisodeServeIngressIdentities,
       AsyncFreshServeIngressAdmissionsForNodeThisStep,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionSchedulerOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncAllVars

THEOREM AsyncCausalEpisodeServicedCandidateConsumesTopologicalWeight ==
  \A target, serviced \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(target)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ProtectedCandidateOwned(target)
       /\ serviced
            \in AsyncCausalEpisodeCandidates(
                 target.node, cutoffOrdinal)
       /\ [AsyncNext]_AsyncAllVars
       /\ ~CandidateScheduled(serviced)'
       /\ ProtectedCandidateOwned(target)'
       => AsyncCausalEpisodeCandidateWorkBudget(
            target.node, cutoffOrdinal)'
            < AsyncCausalEpisodeCandidateWorkBudget(
                target.node, cutoffOrdinal)
BY AsyncCausalEpisodeFrozenOriginsCannotReplenish,
   AsyncCommandSuccessorsStrictlyLowerRemainingWorkStage,
   AsyncCommandSuccessorBatchStrictlyConsumesRemainingWork,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   FS_CardinalityType, FS_Subset, IsaT(1200)
   DEF AsyncCausalEpisodeCandidateWorkBudget,
       AsyncCausalEpisodeCandidateWorkTokens,
       AsyncCausalEpisodeCandidates,
       CandidateScheduled, AsyncAllVars

THEOREM AsyncCausalEpisodeOwnedCutServiceConsumesExactOccurrenceBudget ==
  \A node \in ValidatorIds,
     origin,
     cutoffOrdinal \in Nat \ {0},
     serviced \in AsyncCandidateSet:
    /\ gst
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCausalEpisodeLifecycleCutOwned(
         node, origin, cutoffOrdinal)
    /\ serviced
         \in AsyncCausalEpisodeCandidates(node, cutoffOrdinal)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~CandidateScheduled(serviced)'
    /\ (AsyncCausalEpisodeLifecycleCutOwned(
          node, origin, cutoffOrdinal))'
      => AsyncCausalEpisodeExactCandidateOccurrenceBudget(
           node, cutoffOrdinal)'
           < AsyncCausalEpisodeExactCandidateOccurrenceBudget(
               node, cutoffOrdinal)
BY AsyncCausalEpisodeOwnedLifecycleCutCannotReplenish,
   AsyncCommandExactSuccessorBatchStrictlyConsumesOccurrenceBudget,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   FS_CardinalityType, FS_Subset, IsaT(1500)
   DEF AsyncCausalEpisodeLifecycleCutOwned,
       AsyncCausalEpisodeExactCandidateOccurrenceBudget,
       AsyncCausalEpisodeExactCandidateOccurrenceTokens,
       AsyncCausalEpisodeCandidates,
       CandidateScheduled, AsyncAllVars

THEOREM AsyncCausalEpisodeServicedCandidateConsumesExactOccurrenceBudget ==
  \A target, serviced \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(target)
    IN /\ gst
       /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ProtectedCandidateOwned(target)
       /\ serviced
            \in AsyncCausalEpisodeCandidates(
                 target.node, cutoffOrdinal)
       /\ [AsyncNext]_AsyncAllVars
       /\ ~CandidateScheduled(serviced)'
       /\ ProtectedCandidateOwned(target)'
       => AsyncCausalEpisodeExactCandidateOccurrenceBudget(
            target.node, cutoffOrdinal)'
            < AsyncCausalEpisodeExactCandidateOccurrenceBudget(
                target.node, cutoffOrdinal)
BY AsyncCausalEpisodeOwnedCutServiceConsumesExactOccurrenceBudget,
   AsyncCausalEpisodeTargetLifecycleOrdinalPersists,
   IsaT(600)
   DEF AsyncCausalEpisodeLifecycleCutOwned,
       ProtectedCandidateOwned,
       AsyncCandidateLifecycleOrdinal,
       AsyncCandidateLifecycleRecordsFor,
       AsyncCausalEpisodeExactCandidateOccurrenceBudget,
       AsyncCausalEpisodeExactCandidateOccurrenceTokens,
       AsyncCausalEpisodeCandidates,
       CandidateScheduled, AsyncAllVars

AsyncCausalEpisodeIoOwnerRequired(node, cutoffOrdinal) ==
  LET identities ==
        AsyncCausalEpisodeServeIngressIdentities(node, cutoffOrdinal)
  IN identities # {}
       /\ LET identity ==
                CHOOSE owned \in identities:
                  \A other \in identities:
                    AsyncServeIngressAdmissionSchedulerOrdinal(node, owned)
                      <= AsyncServeIngressAdmissionSchedulerOrdinal(
                           node, other)
          IN /\ AsyncServeLiveReservationOwned(node, identity)
             /\ ~AsyncServeJobQueued(node, identity)
             /\ ~CanResumeExactServeCapacity(node, identity)

AsyncCausalEpisodeFairOwnerKinds == {"Runner", "IoWorker"}

AsyncCausalEpisodeFairOwner(node, cutoffOrdinal) ==
  IF AsyncCausalEpisodeIoOwnerRequired(node, cutoffOrdinal)
  THEN "IoWorker"
  ELSE "Runner"

AsyncCausalEpisodeFairAction(node, ownerKind) ==
  CASE ownerKind = "Runner" -> PostGstRunNode(node)
    [] ownerKind = "IoWorker" -> PostGstServiceIoWorker(node)
    [] OTHER -> FALSE

AsyncCausalEpisodeSelectedFairAction(node, cutoffOrdinal) ==
  AsyncCausalEpisodeFairAction(
    node, AsyncCausalEpisodeFairOwner(node, cutoffOrdinal))

(***************************************************************************
Concrete Serve-owner partition.

The selected immutable ingress owner is always in exactly one physical
carrier class relevant to service: already queued, resumable from its atomic
off-queue reservation, physically full off queue, or ingress/tombstone only.
Only the physically-full class selects the I/O worker.  The other three
classes select the runner, which either drains/materializes the same owner or
advances the frozen per-source prefix.  This partition is deliberately about
the owner action, not about progress: materializing or replacing an owner is
charged by the structural episode rank below.
***************************************************************************)
THEOREM AsyncCausalEpisodeSelectedServeOwnerGeometryIsComplete ==
  \A node, cutoffOrdinal:
    LET identities ==
          AsyncCausalEpisodeServeIngressIdentities(node, cutoffOrdinal)
    IN identities # {}
         => LET identity ==
                  CHOOSE owned \in identities:
                    \A other \in identities:
                      AsyncServeIngressAdmissionSchedulerOrdinal(
                        node, owned)
                        <= AsyncServeIngressAdmissionSchedulerOrdinal(
                             node, other)
            IN \/ /\ AsyncServeJobQueued(node, identity)
                   /\ AsyncCausalEpisodeFairOwner(node, cutoffOrdinal)
                        = "Runner"
               \/ /\ AsyncServeLiveReservationOwned(node, identity)
                   /\ ~AsyncServeJobQueued(node, identity)
                   /\ CanResumeExactServeCapacity(node, identity)
                   /\ AsyncCausalEpisodeFairOwner(node, cutoffOrdinal)
                        = "Runner"
               \/ /\ AsyncServeLiveReservationOwned(node, identity)
                   /\ ~AsyncServeJobQueued(node, identity)
                   /\ ~CanResumeExactServeCapacity(node, identity)
                   /\ AsyncCausalEpisodeFairOwner(node, cutoffOrdinal)
                        = "IoWorker"
               \/ /\ ~AsyncServeLiveReservationOwned(node, identity)
                   /\ AsyncCausalEpisodeFairOwner(node, cutoffOrdinal)
                        = "Runner"
BY Isa
   DEF AsyncCausalEpisodeFairOwner,
       AsyncCausalEpisodeIoOwnerRequired

THEOREM AsyncCausalEpisodeSelectedOwnerIsConcreteAndEnabled ==
  \A candidate \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ResponsiveProtectedCandidateOwned(candidate)
       => /\ AsyncCausalEpisodeFairOwner(
                candidate.node, cutoffOrdinal)
                \in AsyncCausalEpisodeFairOwnerKinds
          /\ ENABLED
               <<AsyncCausalEpisodeSelectedFairAction(
                   candidate.node, cutoffOrdinal)>>_AsyncAllVars
BY QueuedIoEnablesPostGstService,
   QueuedIoServiceIsNonstuttering,
   ResponsiveUnappliedRunNodeIsEnabled,
   EnabledRunNodeLiftsPostGst,
   ExpandENABLED, ENABLEDaxioms, IsaT(900)
   DEF AsyncCausalEpisodeSelectedFairAction,
       AsyncCausalEpisodeFairAction,
       AsyncCausalEpisodeFairOwner,
       AsyncCausalEpisodeFairOwnerKinds,
       AsyncCausalEpisodeIoOwnerRequired,
       AsyncCausalEpisodeServeIngressIdentities,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned,
       AsyncCurrentResponsiveVoters,
       AsyncArchiveIoServiceNodes,
       PostGstRunNode, RunNode, RunNodeWork,
       AsyncAllVars

(***************************************************************************
Action-local structural classification.  It is deliberately a non-ascent
statement: reaching a lower producer rank consumes a finite episode, while
equality leaves the caller's Stage rank responsible for the next descent.
The theorem depends on the exact target-only turn and both non-resurrection
facts; deleting any of them reintroduces the replenishment lasso.
***************************************************************************)
THEOREM AsyncCausalEpisodeStructuralStepIsDescentOrFrame ==
  \A candidate \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
        rank == AsyncCausalEpisodeStructuralRank(
                  candidate.node, cutoffOrdinal)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ProtectedCandidateOwned(candidate)
       /\ [AsyncNext]_AsyncAllVars
       /\ ProtectedCandidateOwned(candidate)'
       => \/ <<AsyncCausalEpisodeStructuralRank(
                   candidate.node, cutoffOrdinal)', rank>>
                  \in AsyncCausalEpisodeStructuralRankOrdering
          \/ AsyncCausalEpisodeStructuralRank(
               candidate.node, cutoffOrdinal)' = rank
BY AsyncCausalEpisodeTargetLifecycleOrdinalPersists,
   AsyncCausalEpisodeFrozenOriginsCannotReplenish,
   AsyncCausalEpisodeServeCutCannotReplenish,
   AsyncCausalEpisodeServicedCandidateConsumesTopologicalWeight,
   AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   AsyncServeIngressTargetOnlyCannotOvertakeOlderRuntimeLifecycle,
   AsyncServeIngressTargetOnlyCannotOvertakeOlderLocalLifecycle,
   ExactTicketTurnDecreasesDrainableIngressTurnReach,
   ExhaustedIngressStepDecreasesDrainableIngressTurnReach,
   LocalStepDecreasesDrainableIngressTurnReach,
   SerializedLocalPredecessorDecreasesDrainableIngressTurnReach,
   RuntimeStepDecreasesDrainableIngressTurnReach,
   OlderRuntimeInterleaveDecreasesDrainableIngressTurnReach,
   FS_CardinalityType, FS_Subset, IsaT(2400)
   DEF AsyncCausalEpisodeStructuralRank,
       AsyncCausalEpisodeCandidateWorkBudget,
       AsyncCausalEpisodeCandidateWorkTokens,
       AsyncCausalEpisodeCandidates,
       AsyncCausalEpisodeServeWorkBudget,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeServeOccurrenceTokens,
       AsyncCausalEpisodeServeIngressPrefixTokens,
       AsyncCausalEpisodeServeIoPredecessorTokens,
       AsyncCausalEpisodeServeReachDebt,
       AsyncCausalEpisodeStructuralRankOrdering,
       AsyncCausalEpisodeServeRankOrdering,
       AsyncServeIngressTargetOnlyTurn,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ServiceIoWorkerWork, PopSelectedIngress,
       DrainFairIngressSelected,
       LocalAdmissionStep, IngressDrainStep,
       SerializedLocalPrecedesServeIngressStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncNext, AsyncAllVars,
       LexPairOrdering, OpToRel

(***************************************************************************
An ingress admission may disappear before its reserved Serve job or
tombstone leaves the downstream lifecycle.  That transfer is not a missing
owner and it is not replenishment: the immutable admission occurrence token
is consumed at the exact `PopSelectedIngress` cut.  A later retry receives a
strictly larger shared scheduler ordinal, so it cannot restore this rank
cell.  This named projection keeps that otherwise easy-to-miss case explicit
for both the ordinary and indexed historical providers.
***************************************************************************)
THEOREM AsyncCausalEpisodeIngressOwnerDepartureStrictlyDescends ==
  \A candidate \in AsyncCandidateSet, identity:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
        rank == AsyncCausalEpisodeStructuralRank(
                  candidate.node, cutoffOrdinal)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ProtectedCandidateOwned(candidate)
       /\ identity
            \in AsyncCausalEpisodeServeIngressIdentities(
                 candidate.node, cutoffOrdinal)
       /\ [AsyncNext]_AsyncAllVars
       /\ ProtectedCandidateOwned(candidate)'
       /\ identity
            \notin AsyncCausalEpisodeServeIngressIdentities(
                     candidate.node, cutoffOrdinal)'
       => <<AsyncCausalEpisodeStructuralRank(
               candidate.node, cutoffOrdinal)', rank>>
            \in AsyncCausalEpisodeStructuralRankOrdering
BY AsyncCausalEpisodeStructuralStepIsDescentOrFrame,
   AsyncCausalEpisodeServeCutCannotReplenish,
   FS_CardinalityType, FS_Subset, IsaT(1200)
   DEF AsyncCausalEpisodeStructuralRank,
       AsyncCausalEpisodeServeWorkBudget,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeServeOccurrenceTokens,
       AsyncCausalEpisodeStructuralRankOrdering,
       AsyncCausalEpisodeServeRankOrdering,
       LexPairOrdering, OpToRel

=============================================================================
