---- MODULE SumeragiV2AdequateLeaderFixedCorridorClockProofs ----
EXTENDS SumeragiV2AdequateLeaderCorridorEntryContinuationProofs

(***************************************************************************
Quantitative fixed-corridor service.

The fixed deadline is measured in logical clock ticks, not in TLA steps.
Weak fairness alone can therefore close a zero-clock scheduler episode, but
it cannot justify the configured number of clock ticks.  This module keeps
the two arguments separate.

The outer accounting has exactly four immutable protocol tokens per member of
the frozen responsive roster: Proposal receipt, durable Prepare intent,
durable Commit intent, and local Decision.  Token completion is cumulative:
a later durable milestone retires every skipped earlier token, and any
same-context Commit Decision retires all four.  This is required because an
authenticated PrepareQC or CommitQC may legitimately let a node join after it
missed the exact Proposal or its own earlier vote.  A retry, equal-count owner
replacement, or count-increasing physical replenishment remains inside the
same token.  It may consume finite scheduler/transport debt, but it never
creates another protocol token and is never called progress.

The token carrier deliberately omits payload and route coordinates.  Those
coordinates remain frozen in the candidate, leader-wire, and continuation
owner identities proved by the lower modules.  This projection is only the
coarse cardinality certificate which explains the `4 * N` factor in
`AsyncProposalPipelineBudget`.
***************************************************************************)

AdequateLeaderFixedPipelinePhases ==
  {"Proposal", "Prepare", "Commit", "Decision"}

AdequateLeaderFixedPipelineTokenCarrier(leaderContext) ==
  AdequateLeaderFrozenResponsiveRoster(leaderContext)
    \X AdequateLeaderFixedPipelinePhases

AdequateLeaderFixedPipelineProposalSeen(
    node, leaderContext, leader, leaderView, subject) ==
  \E proposal \in ProposalRecordSet:
    /\ proposal.context = leaderContext
    /\ proposal.height = leaderContext.height
    /\ proposal.view = leaderView
    /\ proposal.subject = subject
    /\ proposal.proposer = leader
    /\ ProposalAt(node, proposal) \in seenProposals

AdequateLeaderFixedPipelinePrepareDurable(
    node, leaderContext, leaderView, subject) ==
  Vote(leaderContext, leaderView, "Prepare", subject, node)
    \in prepareIntents

AdequateLeaderFixedPipelineCommitDurable(
    node, leaderContext, leaderView, subject) ==
  Vote(leaderContext, leaderView, "Commit", subject, node)
    \in commitIntents

AdequateLeaderFixedPipelineDecisionDurable(
    node, leaderContext, leaderView, subject) ==
  \E decision \in decisions:
    /\ decision.node = node
    /\ decision.qc.context = leaderContext
    /\ decision.qc.view = leaderView
    /\ decision.qc.phase = "Commit"
    /\ decision.qc.subject = subject

\* A node may decide from an authenticated CommitQC without observing the
\* exact Proposal or recording local Prepare/Commit intents.  It may also
\* already hold a same-context Decision from another view.  Such a Decision
\* is terminal for every token owned by that node; insisting on the exact
\* leader-view subject here would leave unreachable prefix tokens forever.
AdequateLeaderFixedPipelineNodeTerminal(node, leaderContext) ==
  \E decision \in decisions:
    /\ decision.node = node
    /\ decision.qc.context = leaderContext
    /\ decision.qc.phase = "Commit"

AdequateLeaderFixedPipelineTokenCompleted(
    token, leaderContext, leader, leaderView, subject) ==
  CASE token[2] = "Proposal" ->
         \/ AdequateLeaderFixedPipelineProposalSeen(
              token[1], leaderContext, leader, leaderView, subject)
         \/ AdequateLeaderFixedPipelinePrepareDurable(
              token[1], leaderContext, leaderView, subject)
         \/ AdequateLeaderFixedPipelineCommitDurable(
              token[1], leaderContext, leaderView, subject)
         \/ AdequateLeaderFixedPipelineNodeTerminal(
              token[1], leaderContext)
    [] token[2] = "Prepare" ->
         \/ AdequateLeaderFixedPipelinePrepareDurable(
              token[1], leaderContext, leaderView, subject)
         \/ AdequateLeaderFixedPipelineCommitDurable(
              token[1], leaderContext, leaderView, subject)
         \/ AdequateLeaderFixedPipelineNodeTerminal(
              token[1], leaderContext)
    [] token[2] = "Commit" ->
         \/ AdequateLeaderFixedPipelineCommitDurable(
              token[1], leaderContext, leaderView, subject)
         \/ AdequateLeaderFixedPipelineNodeTerminal(
              token[1], leaderContext)
    [] token[2] = "Decision" ->
         AdequateLeaderFixedPipelineNodeTerminal(
           token[1], leaderContext)
    [] OTHER -> FALSE

THEOREM AdequateLeaderFixedPipelineExactDecisionIsNodeTerminal ==
  \A node \in ValidatorIds,
     leaderContext \in ContextRecords,
     leaderView \in Views,
     subject \in Subjects:
    AdequateLeaderFixedPipelineDecisionDurable(
      node, leaderContext, leaderView, subject)
      => AdequateLeaderFixedPipelineNodeTerminal(node, leaderContext)
BY SMT
   DEF AdequateLeaderFixedPipelineDecisionDurable,
       AdequateLeaderFixedPipelineNodeTerminal

THEOREM AdequateLeaderFixedPipelineLaterMilestonesRetirePrefix ==
  \A node \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     subject \in Subjects:
    /\ (AdequateLeaderFixedPipelineNodeTerminal(node, leaderContext)
          => \A phase \in AdequateLeaderFixedPipelinePhases:
               AdequateLeaderFixedPipelineTokenCompleted(
                 <<node, phase>>, leaderContext,
                 leader, leaderView, subject))
    /\ (AdequateLeaderFixedPipelineCommitDurable(
          node, leaderContext, leaderView, subject)
          => /\ AdequateLeaderFixedPipelineTokenCompleted(
                  <<node, "Proposal">>, leaderContext,
                  leader, leaderView, subject)
             /\ AdequateLeaderFixedPipelineTokenCompleted(
                  <<node, "Prepare">>, leaderContext,
                  leader, leaderView, subject)
             /\ AdequateLeaderFixedPipelineTokenCompleted(
                  <<node, "Commit">>, leaderContext,
                  leader, leaderView, subject))
    /\ (AdequateLeaderFixedPipelinePrepareDurable(
          node, leaderContext, leaderView, subject)
          => /\ AdequateLeaderFixedPipelineTokenCompleted(
                  <<node, "Proposal">>, leaderContext,
                  leader, leaderView, subject)
             /\ AdequateLeaderFixedPipelineTokenCompleted(
                  <<node, "Prepare">>, leaderContext,
                  leader, leaderView, subject))
BY SMT
   DEF AdequateLeaderFixedPipelinePhases,
       AdequateLeaderFixedPipelineTokenCompleted

AdequateLeaderFixedPipelineRemainingTokens(
    leaderContext, leader, leaderView, subject) ==
  {token \in AdequateLeaderFixedPipelineTokenCarrier(leaderContext):
     ~AdequateLeaderFixedPipelineTokenCompleted(
        token, leaderContext, leader, leaderView, subject)}

AdequateLeaderFixedPipelineWindowsRemaining(
    leaderContext, leader, leaderView, subject) ==
  Cardinality(
    AdequateLeaderFixedPipelineRemainingTokens(
      leaderContext, leader, leaderView, subject))

\* This is the exact frame needed for milestone monotonicity.  Post-GST
\* `AsyncNext` should supply it because crash is pre-GST-only and the three
\* durable sets never shrink.  Keep that action classification separate from
\* the set argument so a future reset action cannot silently enter the proof.
AdequateLeaderFixedPipelineEvidenceMonotoneFrame ==
  /\ seenProposals \subseteq seenProposals'
  /\ prepareIntents \subseteq prepareIntents'
  /\ commitIntents \subseteq commitIntents'
  /\ decisions \subseteq decisions'

THEOREM AdequateLeaderFixedPipelineMonotoneEvidenceCannotAddRemainingToken ==
  \A leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     subject \in Subjects:
    AdequateLeaderFixedPipelineEvidenceMonotoneFrame
      => (AdequateLeaderFixedPipelineRemainingTokens(
            leaderContext, leader, leaderView, subject))'
            \subseteq
          AdequateLeaderFixedPipelineRemainingTokens(
            leaderContext, leader, leaderView, subject)
BY Isa
   DEF AdequateLeaderFixedPipelineEvidenceMonotoneFrame,
       AdequateLeaderFixedPipelineRemainingTokens,
       AdequateLeaderFixedPipelineTokenCompleted,
       AdequateLeaderFixedPipelineProposalSeen,
       AdequateLeaderFixedPipelinePrepareDurable,
       AdequateLeaderFixedPipelineCommitDurable,
       AdequateLeaderFixedPipelineNodeTerminal

THEOREM AdequateLeaderFixedPipelinePhasesHaveCardinalityFour ==
  /\ IsFiniteSet(AdequateLeaderFixedPipelinePhases)
  /\ Cardinality(AdequateLeaderFixedPipelinePhases) = 4
BY FS_EmptySet, FS_AddElement, SMT
   DEF AdequateLeaderFixedPipelinePhases

THEOREM AdequateLeaderFrozenResponsiveRosterHasConfiguredBound ==
  \A leaderContext \in ContextRecords:
    ModelConfiguration
      => /\ IsFiniteSet(
               AdequateLeaderFrozenResponsiveRoster(leaderContext))
         /\ Cardinality(
              AdequateLeaderFrozenResponsiveRoster(leaderContext))
              <= N
PROOF
  <1>1. ASSUME NEW leaderContext \in ContextRecords,
                ModelConfiguration
         PROVE /\ IsFiniteSet(
                      AdequateLeaderFrozenResponsiveRoster(leaderContext))
                /\ Cardinality(
                     AdequateLeaderFrozenResponsiveRoster(leaderContext))
                     <= N
    <2>1. /\ IsFiniteSet(ValidatorIds)
           /\ Cardinality(ValidatorIds) = N
      BY <1>1, FS_Interval, SMT
         DEF ValidatorIds, ModelConfiguration, QuorumConfiguration
    <2>2. AdequateLeaderFrozenResponsiveRoster(leaderContext)
               \subseteq ValidatorIds
      BY Isa
         DEF AdequateLeaderFrozenResponsiveRoster,
             VotingRoster, ValidatorIds
    <2> QED BY <2>1, <2>2, FS_Subset
  <1> QED BY <1>1

THEOREM AdequateLeaderFixedPipelineTokenCarrierHasFourNBound ==
  \A leaderContext \in ContextRecords:
    ModelConfiguration
      => /\ IsFiniteSet(
               AdequateLeaderFixedPipelineTokenCarrier(leaderContext))
         /\ Cardinality(
              AdequateLeaderFixedPipelineTokenCarrier(leaderContext))
              <= 4 * N
PROOF
  <1>1. ASSUME NEW leaderContext \in ContextRecords,
                ModelConfiguration
         PROVE /\ IsFiniteSet(
                      AdequateLeaderFixedPipelineTokenCarrier(
                        leaderContext))
                /\ Cardinality(
                     AdequateLeaderFixedPipelineTokenCarrier(
                       leaderContext))
                     <= 4 * N
    <2>1. /\ IsFiniteSet(
                 AdequateLeaderFrozenResponsiveRoster(leaderContext))
           /\ Cardinality(
                AdequateLeaderFrozenResponsiveRoster(leaderContext))
                <= N
      BY <1>1, AdequateLeaderFrozenResponsiveRosterHasConfiguredBound
    <2>2. /\ IsFiniteSet(AdequateLeaderFixedPipelinePhases)
           /\ Cardinality(AdequateLeaderFixedPipelinePhases) = 4
      BY AdequateLeaderFixedPipelinePhasesHaveCardinalityFour
    <2> QED BY <2>1, <2>2, FS_Product, SMT
         DEF AdequateLeaderFixedPipelineTokenCarrier
  <1> QED BY <1>1

THEOREM AdequateLeaderFixedPipelineRemainingTokensHaveFourNBound ==
  \A leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views,
     subject \in Subjects:
    ModelConfiguration
      => /\ IsFiniteSet(
               AdequateLeaderFixedPipelineRemainingTokens(
                 leaderContext, leader, leaderView, subject))
         /\ AdequateLeaderFixedPipelineWindowsRemaining(
              leaderContext, leader, leaderView, subject)
              \in Nat
         /\ AdequateLeaderFixedPipelineWindowsRemaining(
              leaderContext, leader, leaderView, subject)
              <= 4 * N
PROOF
  <1>1. ASSUME NEW leaderContext \in ContextRecords,
                NEW leader \in ValidatorIds,
                NEW leaderView \in Views,
                NEW subject \in Subjects,
                ModelConfiguration
         PROVE /\ IsFiniteSet(
                      AdequateLeaderFixedPipelineRemainingTokens(
                        leaderContext, leader, leaderView, subject))
                /\ AdequateLeaderFixedPipelineWindowsRemaining(
                     leaderContext, leader, leaderView, subject)
                     \in Nat
                /\ AdequateLeaderFixedPipelineWindowsRemaining(
                     leaderContext, leader, leaderView, subject)
                     <= 4 * N
    <2>1. /\ IsFiniteSet(
                 AdequateLeaderFixedPipelineTokenCarrier(leaderContext))
           /\ Cardinality(
                AdequateLeaderFixedPipelineTokenCarrier(leaderContext))
                <= 4 * N
      BY <1>1, AdequateLeaderFixedPipelineTokenCarrierHasFourNBound
    <2>2. AdequateLeaderFixedPipelineRemainingTokens(
               leaderContext, leader, leaderView, subject)
             \subseteq
           AdequateLeaderFixedPipelineTokenCarrier(leaderContext)
      BY DEF AdequateLeaderFixedPipelineRemainingTokens
    <2>3. /\ IsFiniteSet(
                 AdequateLeaderFixedPipelineRemainingTokens(
                   leaderContext, leader, leaderView, subject))
           /\ Cardinality(
                AdequateLeaderFixedPipelineRemainingTokens(
                  leaderContext, leader, leaderView, subject))
                <= Cardinality(
                     AdequateLeaderFixedPipelineTokenCarrier(
                       leaderContext))
      BY <2>1, <2>2, FS_Subset
    <2> QED BY <2>1, <2>3, FS_CardinalityType, SMT
         DEF AdequateLeaderFixedPipelineWindowsRemaining
  <1> QED BY <1>1

(***************************************************************************
Bounded physical position inside one protocol token.

`CandidateServiceRank` is lexicographic, so its raw second coordinate cannot
be compared across carrier stages.  The ceilings below scalarize that rank.
They are deliberately derived from the existing queue, deferred, causal, and
I/O capacities.  No environment switch or proof-only capacity is introduced.

The deferred ceiling charges all three independent queues.  The completion
load ceiling separately includes the outstanding executor set, the serialized
runtime queue, and the Busy-deferred Completion lane.  Adding every ceiling
is conservative, but makes any strict lexicographic service-rank descent a
strict natural-number descent.
***************************************************************************)

AdequateLeaderFixedDeferredPositionCeiling ==
  3 * AsyncDeferredDrainBudget + 2

AdequateLeaderFixedRuntimePositionCeiling ==
  3 * AsyncQueueCapacity + 2

AdequateLeaderFixedReadyPositionCeiling ==
  4 * AsyncIoWorkCapacity + 3

AdequateLeaderFixedIoPositionCeiling ==
  AsyncIoCapacity + AsyncIoWorkCapacity
    + AsyncQueueCapacity + AsyncDeferredNormalCapacity

AdequateLeaderFixedCausalPositionCeiling ==
  2 * AsyncCausalCandidateLifecycleCapacity + 1

AdequateLeaderFixedCandidatePhysicalWindowBudget ==
  AdequateLeaderFixedDeferredPositionCeiling
    + AdequateLeaderFixedRuntimePositionCeiling
    + AdequateLeaderFixedReadyPositionCeiling
    + AdequateLeaderFixedIoPositionCeiling
    + AdequateLeaderFixedCausalPositionCeiling
    + 4

THEOREM AdequateLeaderFixedCandidatePhysicalWindowFitsConfiguredBudget ==
  AsyncConfiguration
    => AdequateLeaderFixedCandidatePhysicalWindowBudget
         <= AsyncCandidatePhysicalServiceBudget
BY SMT
   DEF AdequateLeaderFixedCandidatePhysicalWindowBudget,
       AdequateLeaderFixedDeferredPositionCeiling,
       AdequateLeaderFixedRuntimePositionCeiling,
       AdequateLeaderFixedReadyPositionCeiling,
       AdequateLeaderFixedIoPositionCeiling,
       AdequateLeaderFixedCausalPositionCeiling,
       AsyncCandidatePhysicalServiceBudget,
       AsyncCandidateProducerActionEpisodeBudget,
       AsyncCandidateProducerEpisodeBudget,
       AsyncCandidateProducerEpisodeCapacity,
       AsyncRuntimeCycleBudget, AsyncRunnerCycleBudget,
       AsyncDeferredDrainBudget, AsyncIoDrainBudget,
       AsyncCausalCandidateLifecycleCapacity,
       AsyncIoCapacity, AsyncConfiguration

THEOREM AdequateLeaderFixedProducerAndPhysicalWindowFitConfiguredBudget ==
  AsyncConfiguration
    => AsyncCandidateProducerActionEpisodeBudget
         + AdequateLeaderFixedCandidatePhysicalWindowBudget
         <= AsyncCandidatePhysicalServiceBudget
BY SMT
   DEF AdequateLeaderFixedCandidatePhysicalWindowBudget,
       AdequateLeaderFixedDeferredPositionCeiling,
       AdequateLeaderFixedRuntimePositionCeiling,
       AdequateLeaderFixedReadyPositionCeiling,
       AdequateLeaderFixedIoPositionCeiling,
       AdequateLeaderFixedCausalPositionCeiling,
       AsyncCandidatePhysicalServiceBudget,
       AsyncCandidateProducerActionEpisodeBudget,
       AsyncCandidateProducerEpisodeBudget,
       AsyncCandidateProducerEpisodeCapacity,
       AsyncRuntimeCycleBudget, AsyncRunnerCycleBudget,
       AsyncDeferredDrainBudget, AsyncIoDrainBudget,
       AsyncCausalCandidateLifecycleCapacity,
       AsyncIoCapacity, AsyncConfiguration

AdequateLeaderFixedCandidatePhysicalRankFrom(rank) ==
  CASE rank[1] = 2 -> rank[2]
    [] rank[1] = 3 ->
         AdequateLeaderFixedDeferredPositionCeiling + 1 + rank[2]
    [] rank[1] = 4 ->
         AdequateLeaderFixedDeferredPositionCeiling + 1
           + AdequateLeaderFixedRuntimePositionCeiling + 1
           + rank[2]
    [] rank[1] = 5 ->
         AdequateLeaderFixedDeferredPositionCeiling + 1
           + AdequateLeaderFixedRuntimePositionCeiling + 1
           + AdequateLeaderFixedReadyPositionCeiling + 1
           + rank[2]
    [] rank[1] = 6 ->
         AdequateLeaderFixedDeferredPositionCeiling + 1
           + AdequateLeaderFixedRuntimePositionCeiling + 1
           + AdequateLeaderFixedReadyPositionCeiling + 1
           + AdequateLeaderFixedIoPositionCeiling + 1
           + rank[2]
    [] OTHER -> 0

AdequateLeaderFixedCandidatePhysicalRank(candidate) ==
  AdequateLeaderFixedCandidatePhysicalRankFrom(
    CandidateServiceRank(candidate))

THEOREM AdequateLeaderScheduledCandidatePositionHasCapacityBound ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ CandidateScheduled(candidate)
    => LET rank == CandidateServiceRank(candidate)
       IN /\ rank[1] \in 2..6
          /\ rank[2] \in Nat
          /\ CASE rank[1] = 2 ->
                    rank[2]
                      <= AdequateLeaderFixedDeferredPositionCeiling
               [] rank[1] = 3 ->
                    rank[2]
                      <= AdequateLeaderFixedRuntimePositionCeiling
               [] rank[1] = 4 ->
                    rank[2]
                      <= AdequateLeaderFixedReadyPositionCeiling
               [] rank[1] = 5 ->
                    rank[2]
                      <= AdequateLeaderFixedIoPositionCeiling
               [] rank[1] = 6 ->
                    rank[2]
                      <= AdequateLeaderFixedCausalPositionCeiling
               [] OTHER -> FALSE
BY ScheduledCandidateServiceRankInCarrier,
   SchedulerClassPrefixRankBound,
   FS_Subset, FS_CardinalityType, IsaT(2400)
   DEF AdequateLeaderFixedDeferredPositionCeiling,
       AdequateLeaderFixedRuntimePositionCeiling,
       AdequateLeaderFixedReadyPositionCeiling,
       AdequateLeaderFixedIoPositionCeiling,
       AdequateLeaderFixedCausalPositionCeiling,
       AsyncDeferredDrainBudget,
       AsyncCausalCandidateLifecycleCapacity,
       CandidateServiceRank, CandidateScheduled,
       DeferredCandidates, QueuedCandidates,
       TrackedWorkCandidates, CausalCandidates,
       CandidateInReadyQueue, CandidateInIoQueue,
       DeferredCandidatePosition, DeferredClassPrefixIndices,
       DeferredCandidateIndices, DeferredClassQueue,
       ReadyCandidatePosition, ReadyCandidateSource,
       ReadyCompletionQueue, CandidateSequenceIndex,
       CausalCandidatePosition, CandidateIoIndex,
       AsyncCompletionLoad, AsyncOutstandingWorkCount,
       QueuedCompletionCount, QueuedCompletionIndices,
       DeferredCompletionCount,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant, AsyncIoTypeInvariant,
       AsyncIoCapacityTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncIoWorkContentTypeInvariant,
       AsyncDeferredTypeInvariant, AsyncDeferredContentTypeInvariant,
       AsyncProgressOwnershipInvariant, AsyncOutstandingCarrierInvariant,
       SequenceSet

THEOREM AdequateLeaderScheduledCandidatePhysicalRankIsBounded ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ CandidateScheduled(candidate)
    => /\ AdequateLeaderFixedCandidatePhysicalRank(candidate)
              \in Nat \ {0}
       /\ AdequateLeaderFixedCandidatePhysicalRank(candidate)
              <= AdequateLeaderFixedCandidatePhysicalWindowBudget
BY AdequateLeaderScheduledCandidatePositionHasCapacityBound, SMT
   DEF AdequateLeaderFixedCandidatePhysicalRank,
       AdequateLeaderFixedCandidatePhysicalRankFrom,
       AdequateLeaderFixedCandidatePhysicalWindowBudget

THEOREM AdequateLeaderStrictServiceRankDescentLowersPhysicalRank ==
  \A beforeRank, afterRank \in OwnedServiceRankCarrier:
    /\ beforeRank[1] \in 2..6
    /\ afterRank[1] \in 2..6
    /\ CASE beforeRank[1] = 2 ->
              beforeRank[2]
                <= AdequateLeaderFixedDeferredPositionCeiling
         [] beforeRank[1] = 3 ->
              beforeRank[2]
                <= AdequateLeaderFixedRuntimePositionCeiling
         [] beforeRank[1] = 4 ->
              beforeRank[2]
                <= AdequateLeaderFixedReadyPositionCeiling
         [] beforeRank[1] = 5 ->
              beforeRank[2]
                <= AdequateLeaderFixedIoPositionCeiling
         [] beforeRank[1] = 6 ->
              beforeRank[2]
                <= AdequateLeaderFixedCausalPositionCeiling
         [] OTHER -> FALSE
    /\ CASE afterRank[1] = 2 ->
              afterRank[2]
                <= AdequateLeaderFixedDeferredPositionCeiling
         [] afterRank[1] = 3 ->
              afterRank[2]
                <= AdequateLeaderFixedRuntimePositionCeiling
         [] afterRank[1] = 4 ->
              afterRank[2]
                <= AdequateLeaderFixedReadyPositionCeiling
         [] afterRank[1] = 5 ->
              afterRank[2]
                <= AdequateLeaderFixedIoPositionCeiling
         [] afterRank[1] = 6 ->
              afterRank[2]
                <= AdequateLeaderFixedCausalPositionCeiling
         [] OTHER -> FALSE
    /\ ServiceRankLess(afterRank, beforeRank)
    => AdequateLeaderFixedCandidatePhysicalRankFrom(afterRank)
         < AdequateLeaderFixedCandidatePhysicalRankFrom(beforeRank)
BY SMT
   DEF AdequateLeaderFixedCandidatePhysicalRankFrom,
       ServiceRankLess

(***************************************************************************
Open cross-child additive-accounting seam.

The static theorem above bounds one scheduled candidate.  It does not by
itself bound the complete same-origin producer episode.  In particular,
executing PersistPrepare removes a Runtime-stage parent and appends its
SignVote child to the Causal carrier.  The child inherits the parent's
lifecycle ordinal, but its individual `CandidateServiceRank` moves from
stage 3 to stage 6 and its scalar physical rank can therefore increase.
Begin/Commit and body-validation branches have the same shape.  Treating the
child's independent static ceiling as a fresh budget would recharge already
consumed physical debt.

The required additive argument must instead carry one cumulative frozen-prefix
debt across the parent/child handoff.  The exact occurrence budget from
`SumeragiV2AsyncCausalWorkBudgetProofs` assigns every candidate the minimal
remaining size of its closed `CommandSuccessors` subtree.  Its maximum is
nineteen, its successor batch consumes strictly less than its parent, and the
complete frozen candidate carrier fits `AsyncCandidateProducerEpisodeBudget`.
`AsyncCausalEpisodeOwnedCutServiceConsumesExactOccurrenceBudget` then proves
strict descent without requiring the departed parent itself to remain owned.
The property below is the exact still-open transition seam.  A provider must
prove that the immutable lifecycle admission which owns the frozen cut
survives the parent/child handoff, and that every materialized child retains
the same causal origin and cutoff ordinal.  Only then may the separate
`AsyncChunkCount + 8` semantic handoffs be added rather than multiplying a
fresh physical ceiling per child.

No theorem in this module asserts the temporal property.  The one-step
theorems below derive cut carry and exact occurrence-budget descent from GST,
the scheduler-coverage and progress invariants, one concrete bracketed
`AsyncNext`, and an exact scheduled successor.  The authority-deadline service
module supplies those antecedents at every fixed-pipeline handoff and composes
the resulting finite producer episode with token retirement and the
configured deadline service argument.
***************************************************************************)

AdequateLeaderFixedCrossChildPhysicalDebt(node, cutoffOrdinal) ==
  AsyncCausalEpisodeExactCandidateOccurrenceBudget(
    node, cutoffOrdinal)

THEOREM AdequateLeaderFixedCrossChildPhysicalDebtFitsProducerEpisode ==
  \A node \in ValidatorIds, cutoffOrdinal \in Nat:
    AsyncStrongTypeInvariant
      => /\ IsFiniteSet(
               AsyncCausalEpisodeExactCandidateOccurrenceTokens(
                 node, cutoffOrdinal))
         /\ AdequateLeaderFixedCrossChildPhysicalDebt(
              node, cutoffOrdinal)
              <= AsyncCandidateProducerEpisodeBudget
BY AsyncCausalEpisodeExactOccurrenceBudgetFitsConfiguredEpisode
   DEF AdequateLeaderFixedCrossChildPhysicalDebt

AdequateLeaderFixedPipelineExactParentDeparture(parent) ==
  /\ CandidateScheduled(parent)
  /\ ~CandidateScheduled(parent)'
  /\ \E rank \in AdequateLeaderTargetSemanticRankCarrier:
       ExactLeaderStaticSemanticRank(parent, rank)
  /\ \E child \in AsyncCandidateSet:
       /\ child \in SequenceSet(CommandSuccessors(parent))
       /\ CandidateScheduled(child)'

AdequateLeaderFixedCrossChildPhysicalRankResetThisStep(parent, child) ==
  /\ CandidateScheduled(parent)
  /\ ~CandidateScheduled(parent)'
  /\ child \in SequenceSet(CommandSuccessors(parent))
  /\ CandidateScheduled(child)'
  /\ child.causalOrigin = parent.causalOrigin
  /\ AsyncCandidateLifecycleOrdinal(child)'
       = AsyncCandidateLifecycleOrdinal(parent)
  /\ AdequateLeaderFixedCandidatePhysicalRank(child)'
       > AdequateLeaderFixedCandidatePhysicalRank(parent)

AdequateLeaderFixedCrossChildSuccessorBatchConsumes(parent) ==
  AsyncCommandExactSuccessorBatchOccurrenceBudget(parent)
    < AsyncCausalExactRemainingOccurrenceBudget(parent.kind)

THEOREM AdequateLeaderFixedCrossChildSuccessorBatchStrictlyConsumes ==
  \A parent \in AsyncCandidateSet:
    AdequateLeaderFixedCrossChildSuccessorBatchConsumes(parent)
BY AsyncCommandExactSuccessorBatchStrictlyConsumesOccurrenceBudget
   DEF AdequateLeaderFixedCrossChildSuccessorBatchConsumes

THEOREM AdequateLeaderFixedCommandSuccessorsRetainNodeAndOrigin ==
  \A parent \in AsyncCandidateSet:
    \A child \in SequenceSet(CommandSuccessors(parent)):
      /\ child.node = parent.node
      /\ child.causalOrigin = parent.causalOrigin
BY CommandSuccessorsRetainCausalOrigin, SMTT(300)
   DEF CommandSuccessors, CausalCandidate,
       CausalCandidateWithEvidence, RetainedBodyRebindCandidate,
       PersistDecisionRecoveryKind, PersistDecisionRecoverySuccessor,
       InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallLockedFetchSuccessor,
       InstallCommitSignSuccessors, InstallCommitSignSuccessor,
       InstallProposalSuccessor, AsyncCandidateFrom,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorSemanticPhase,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin, SequenceSet

AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep(parent) ==
  LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(parent)
      origin == parent.causalOrigin
  IN /\ AsyncCausalEpisodeLifecycleCutOwned(
           parent.node, origin, cutoffOrdinal)
     /\ (AsyncCausalEpisodeLifecycleCutOwned(
           parent.node, origin, cutoffOrdinal))'
     /\ \A child \in AsyncCandidateSet:
          /\ child \in SequenceSet(CommandSuccessors(parent))
          /\ CandidateScheduled(child)'
          => /\ child.causalOrigin = origin
             /\ AsyncCandidateLifecycleOrdinal(child)' = cutoffOrdinal

THEOREM AdequateLeaderFixedExactParentDepartureCarriesLifecycleCut ==
  \A parent \in AsyncCandidateSet:
    /\ gst
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateLifecycleSchedulerCoverageInvariant
    /\ AdequateLeaderFixedPipelineExactParentDeparture(parent)
    /\ [AsyncNext]_AsyncAllVars
    /\ AsyncCandidateLifecycleSchedulerCoverageInvariant'
    => AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep(parent)
BY AdequateLeaderFixedCommandSuccessorsRetainNodeAndOrigin,
   AsyncCausalEpisodeSameOriginHandoffRetainsLifecycleCut,
   IsaT(600)
   DEF AdequateLeaderFixedPipelineExactParentDeparture,
       AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep

THEOREM AdequateLeaderFixedOwnedParentDepartureConsumesCrossChildDebt ==
  \A parent \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(parent)
        origin == parent.causalOrigin
    IN /\ gst
       /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AdequateLeaderFixedPipelineExactParentDeparture(parent)
       /\ cutoffOrdinal \in Nat \ {0}
       /\ parent
            \in AsyncCausalEpisodeCandidates(
                 parent.node, cutoffOrdinal)
       /\ [AsyncNext]_AsyncAllVars
       /\ AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep(parent)
       => (AdequateLeaderFixedCrossChildPhysicalDebt(
             parent.node, cutoffOrdinal))'
            < AdequateLeaderFixedCrossChildPhysicalDebt(
                parent.node, cutoffOrdinal)
BY AsyncCausalEpisodeOwnedCutServiceConsumesExactOccurrenceBudget, Isa
   DEF AdequateLeaderFixedPipelineExactParentDeparture,
       AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep,
       AdequateLeaderFixedCrossChildPhysicalDebt,
       AsyncCandidateSet

AdequateLeaderFixedCrossChildPhysicalBudgetCarryAction ==
  \A parent \in AsyncCandidateSet:
    AdequateLeaderFixedPipelineExactParentDeparture(parent)
      => LET cutoffOrdinal ==
               AsyncCandidateLifecycleOrdinal(parent)
             beforeDebt ==
               AdequateLeaderFixedCrossChildPhysicalDebt(
                 parent.node, cutoffOrdinal)
             afterDebt ==
               (AdequateLeaderFixedCrossChildPhysicalDebt(
                  parent.node, cutoffOrdinal))'
         IN /\ cutoffOrdinal \in Nat \ {0}
            /\ parent
                 \in AsyncCausalEpisodeCandidates(
                      parent.node, cutoffOrdinal)
            /\ AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep(
                 parent)
            /\ beforeDebt <= AsyncCandidateProducerEpisodeBudget
            /\ afterDebt < beforeDebt
            /\ AdequateLeaderFixedCrossChildSuccessorBatchConsumes(
                 parent)

AdequateLeaderFixedCrossChildPhysicalBudgetCarryProperty(specification) ==
  specification
    => [][AdequateLeaderFixedCrossChildPhysicalBudgetCarryAction]_AsyncAllVars

(***************************************************************************
Exact cumulative action credit across causal handoffs.

A fresh Normal or Progress child needs two candidate-owned service actions:
causal admission and reducer dispatch.  A fresh Completion child needs four:
causal admission, I/O service, ready admission, and reducer dispatch.  Once
the child moves, its remaining route credit falls with stages
6 -> 5 -> 4 -> 3/2 -> departure.  Deferred execution retains the final unit;
it does not allocate another route.

The successor-tail CASE is the exact maximum sum for the closed
`CommandSuccessors` table.  It is not the occurrence table multiplied by a
generic physical window.  For example, ValidateBody reserves the exact
Normal/BeginPrepare, Completion/BeginLockCommit, and Completion/Apply tails;
PersistInstallTC reserves the exact Completion/FetchBody,
Completion/SignVote, and Normal/AssembleBody tails.  The maximum complete
credit is 72 for Completion/BeginTimeout.  Parent dispatch consumes its last
route unit while its materialized children consume only the tail which was
already reserved by that parent.

The producer-continuation record cannot allocate a later arbitrary batch:
`AsyncCandidateProducerContinuationHandoffCandidatesThisStep` freezes exactly
`CommandSuccessors(parent)` on a successful service step and freezes the empty
set on a discard/residual step.  A scheduled or coalesced exact child therefore
uses the same tail below; an external transport/body residual remains in the
separate finite continuation prefix rank and has no latent child credit to
recreate.

This closes the arithmetic rank-reset defect for exact candidate handoffs.
This module deliberately leaves the separate frozen runner/deferred/I/O
selector episodes as an interface and asserts no temporal provider or ledger
status; `SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs` supplies their
additive configured terms in the release composition.
***************************************************************************)

AdequateLeaderFixedInitialCandidateRouteActionCredit(commandClass) ==
  IF commandClass = "Completion" THEN 4 ELSE 2

AdequateLeaderFixedCandidateSuccessorTailActionCredit(kind) ==
  CASE kind = "BeginTimeout" -> 68
    [] kind = "PersistTimeout" -> 64
    [] kind = "DeliverTC" -> 62
    [] kind \in
         {"SignTimeout", "DeliverTimeout", "BeginInstallTC"} -> 60
    [] kind = "PersistInstallTC" -> 56
    [] kind = "DeliverProposal" -> 48
    [] kind \in {"DeliverVote", "DeliverQC"} -> 44
    [] kind \in {"FormCommitQC", "BeginDecision"} -> 42
    [] kind \in {"DeliverChunk", "PersistDecision"} -> 38
    [] kind \in
         {"FetchBody", "RebindRetainedBody",
          "FetchCertifiedBody"} -> 34
    [] kind = "StoreBody" -> 30
    [] kind = "ValidateBody" -> 26
    [] kind = "BeginObservePrepare" -> 16
    [] kind \in {"AssembleBody", "PersistObservePrepare"} -> 12
    [] kind \in
         {"BeginProposal", "BeginPrepare", "BeginLockCommit"} -> 8
    [] kind \in
         {"PersistProposal", "PersistPrepare",
          "PersistLockCommit"} -> 4
    [] OTHER -> 0

AdequateLeaderFixedExactCandidateActionCredit(commandClass, kind) ==
  AdequateLeaderFixedInitialCandidateRouteActionCredit(commandClass)
    + AdequateLeaderFixedCandidateSuccessorTailActionCredit(kind)

AdequateLeaderFixedCommandSuccessorBatchActionCredit(command) ==
  LET successors == CommandSuccessors(command)
  IN CASE Len(successors) = 0 -> 0
       [] Len(successors) = 1 ->
            AdequateLeaderFixedExactCandidateActionCredit(
              successors[1].class, successors[1].kind)
       [] Len(successors) = 2 ->
            AdequateLeaderFixedExactCandidateActionCredit(
              successors[1].class, successors[1].kind)
              + AdequateLeaderFixedExactCandidateActionCredit(
                  successors[2].class, successors[2].kind)
       [] Len(successors) = 3 ->
            AdequateLeaderFixedExactCandidateActionCredit(
              successors[1].class, successors[1].kind)
              + AdequateLeaderFixedExactCandidateActionCredit(
                  successors[2].class, successors[2].kind)
              + AdequateLeaderFixedExactCandidateActionCredit(
                  successors[3].class, successors[3].kind)
       [] OTHER ->
            AdequateLeaderFixedCandidateSuccessorTailActionCredit(
              command.kind)

THEOREM AdequateLeaderFixedExactCandidateActionCreditIsBounded ==
  \A commandClass \in AsyncCommandClasses, kind \in AsyncWorkKinds:
    AdequateLeaderFixedExactCandidateActionCredit(commandClass, kind)
      \in 2..72
BY SMT
   DEF AdequateLeaderFixedExactCandidateActionCredit,
       AdequateLeaderFixedInitialCandidateRouteActionCredit,
       AdequateLeaderFixedCandidateSuccessorTailActionCredit,
       AsyncCommandClasses, AsyncWorkKinds,
       AsyncCompletionTags, AsyncDeliveryKinds, AsyncReducerKinds

THEOREM AdequateLeaderFixedSuccessorBatchFitsReservedActionTail ==
  \A command \in AsyncCandidateSet:
    AdequateLeaderFixedCommandSuccessorBatchActionCredit(command)
      <= AdequateLeaderFixedCandidateSuccessorTailActionCredit(command.kind)
BY CommandSuccessorsHaveBoundedLength, SMTT(600)
   DEF AdequateLeaderFixedCommandSuccessorBatchActionCredit,
       AdequateLeaderFixedExactCandidateActionCredit,
       AdequateLeaderFixedInitialCandidateRouteActionCredit,
       AdequateLeaderFixedCandidateSuccessorTailActionCredit,
       CommandSuccessors, CausalCandidate,
       CausalCandidateWithEvidence, RetainedBodyRebindCandidate,
       PersistDecisionRecoveryKind, PersistDecisionRecoverySuccessor,
       InstallCommandSuccessors,
       InstallLockedFetchSuccessors, InstallLockedFetchSuccessor,
       InstallCommitSignSuccessors, InstallCommitSignSuccessor,
       InstallProposalSuccessor, AsyncCandidateFrom,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorSemanticPhase,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin,
       SequenceSet, AsyncCandidateSet

THEOREM AdequateLeaderFixedSuccessorBatchStrictlyConsumesActionCredit ==
  \A command \in AsyncCandidateSet:
    AdequateLeaderFixedCommandSuccessorBatchActionCredit(command)
      < AdequateLeaderFixedExactCandidateActionCredit(
          command.class, command.kind)
BY AdequateLeaderFixedSuccessorBatchFitsReservedActionTail, SMT
   DEF AdequateLeaderFixedExactCandidateActionCredit,
       AdequateLeaderFixedInitialCandidateRouteActionCredit,
       AsyncCandidateSet, AsyncCandidateTyped, AsyncCommandClasses

AdequateLeaderFixedRouteActionCreditFromStage(commandClass, stage) ==
  CASE stage = 6 ->
         AdequateLeaderFixedInitialCandidateRouteActionCredit(commandClass)
    [] stage = 5 -> 3
    [] stage = 4 -> 2
    [] stage \in 2..3 -> 1
    [] OTHER -> 0

AdequateLeaderFixedCandidateRemainingRouteActionCredit(candidate) ==
  AdequateLeaderFixedRouteActionCreditFromStage(
    candidate.class, CandidateServiceRank(candidate)[1])

AdequateLeaderFixedCandidateRemainingActionCredit(candidate) ==
  AdequateLeaderFixedCandidateRemainingRouteActionCredit(candidate)
    + AdequateLeaderFixedCandidateSuccessorTailActionCredit(candidate.kind)

AdequateLeaderFixedCutCumulativeActionTokens(node, cutoffOrdinal) ==
  {<<candidate, token>>:
     candidate \in AsyncCausalEpisodeCandidates(node, cutoffOrdinal),
     token
       \in 1..AdequateLeaderFixedCandidateRemainingActionCredit(candidate)}

AdequateLeaderFixedCutCumulativeActionDebt(node, cutoffOrdinal) ==
  Cardinality(
    AdequateLeaderFixedCutCumulativeActionTokens(node, cutoffOrdinal))

THEOREM AdequateLeaderFixedScheduledCandidateRemainingActionCreditIsBounded ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ CandidateScheduled(candidate)
    => AdequateLeaderFixedCandidateRemainingActionCredit(candidate)
         \in 1..72
BY AdequateLeaderScheduledCandidatePositionHasCapacityBound, SMT
   DEF AdequateLeaderFixedCandidateRemainingActionCredit,
       AdequateLeaderFixedCandidateRemainingRouteActionCredit,
       AdequateLeaderFixedRouteActionCreditFromStage,
       AdequateLeaderFixedInitialCandidateRouteActionCredit,
       AdequateLeaderFixedCandidateSuccessorTailActionCredit,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncCandidateSet, AsyncCandidateTyped,
       AsyncCommandClasses, AsyncWorkKinds,
       AsyncCompletionTags, AsyncDeliveryKinds, AsyncReducerKinds

(***************************************************************************
Intermediate carrier replacement keeps the candidate value itself as the
token identity.  Only the stage-derived interval may shrink.  The reviewed
route admits ordinary lower-stage movement and the deferred 2 -> 3 retry,
whose final route unit is unchanged.  A non-Completion 6 -> 5 move is
excluded: those classes have no I/O leg, and admitting that invented move
would increase their route credit from two to three.

The action shape explicitly freezes the cut carrier and every other
candidate's route credit.  Lower modules must establish this shape for a
concrete carrier transition.  Thus this theorem is a one-step accounting
interface, not a temporal service or fairness provider.
***************************************************************************)

AdequateLeaderFixedIntermediateRouteStageMove(
    commandClass, beforeStage, afterStage) ==
  /\ commandClass \in AsyncCommandClasses
  /\ beforeStage \in 2..6
  /\ afterStage \in 2..6
  /\ \/ afterStage = beforeStage
     \/ /\ afterStage < beforeStage
        /\ ~(commandClass # "Completion"
              /\ beforeStage = 6
              /\ afterStage = 5)
     \/ /\ beforeStage = 2
        /\ afterStage = 3

THEOREM AdequateLeaderFixedIntermediateRouteStageCannotRecharge ==
  \A commandClass \in AsyncCommandClasses,
     beforeStage, afterStage \in 2..6:
    AdequateLeaderFixedIntermediateRouteStageMove(
      commandClass, beforeStage, afterStage)
      => AdequateLeaderFixedRouteActionCreditFromStage(
           commandClass, afterStage)
           <= AdequateLeaderFixedRouteActionCreditFromStage(
                commandClass, beforeStage)
BY SMT
   DEF AdequateLeaderFixedIntermediateRouteStageMove,
       AdequateLeaderFixedRouteActionCreditFromStage,
       AdequateLeaderFixedInitialCandidateRouteActionCredit,
       AsyncCommandClasses

AdequateLeaderFixedIntermediateRouteCarrierMove(
    candidate, node, cutoffOrdinal) ==
  LET frozenCandidates ==
        AsyncCausalEpisodeCandidates(node, cutoffOrdinal)
      beforeStage == CandidateServiceRank(candidate)[1]
      afterStage == CandidateServiceRank(candidate)'[1]
  IN /\ candidate \in frozenCandidates
     /\ candidate
          \in (AsyncCausalEpisodeCandidates(node, cutoffOrdinal))'
     /\ (AsyncCausalEpisodeCandidates(node, cutoffOrdinal))'
          = frozenCandidates
     /\ AdequateLeaderFixedIntermediateRouteStageMove(
          candidate.class, beforeStage, afterStage)
     /\ \A other \in frozenCandidates \ {candidate}:
          AdequateLeaderFixedCandidateRemainingRouteActionCredit(other)'
            = AdequateLeaderFixedCandidateRemainingRouteActionCredit(other)

THEOREM AdequateLeaderFixedIntermediateRouteCarrierCannotRechargeCut ==
  \A candidate \in AsyncCandidateSet,
     node \in ValidatorIds,
     cutoffOrdinal \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AdequateLeaderFixedIntermediateRouteCarrierMove(
         candidate, node, cutoffOrdinal)
      => /\ AdequateLeaderFixedCutCumulativeActionTokens(
               node, cutoffOrdinal)'
              \subseteq
              AdequateLeaderFixedCutCumulativeActionTokens(
                node, cutoffOrdinal)
         /\ AdequateLeaderFixedCutCumulativeActionDebt(
               node, cutoffOrdinal)'
              <= AdequateLeaderFixedCutCumulativeActionDebt(
                   node, cutoffOrdinal)
BY AsyncCausalEpisodeCandidateCarrierHasConfiguredBound,
   AdequateLeaderFixedIntermediateRouteStageCannotRecharge,
   FS_Interval, FS_Product, FS_Subset, FS_CardinalityType, IsaT(1200)
   DEF AdequateLeaderFixedIntermediateRouteCarrierMove,
       AdequateLeaderFixedCutCumulativeActionDebt,
       AdequateLeaderFixedCutCumulativeActionTokens,
       AdequateLeaderFixedCandidateRemainingActionCredit,
       AdequateLeaderFixedCandidateRemainingRouteActionCredit,
       AdequateLeaderFixedRouteActionCreditFromStage

THEOREM AdequateLeaderFixedCutCumulativeActionDebtFitsEpisodeBudget ==
  \A node \in ValidatorIds, cutoffOrdinal \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
      => /\ IsFiniteSet(
             AdequateLeaderFixedCutCumulativeActionTokens(
               node, cutoffOrdinal))
         /\ AdequateLeaderFixedCutCumulativeActionDebt(
              node, cutoffOrdinal)
              \in Nat
         /\ AdequateLeaderFixedCutCumulativeActionDebt(
              node, cutoffOrdinal)
              <= AsyncCandidateProducerActionEpisodeBudget
BY AsyncCausalEpisodeCandidateCarrierHasConfiguredBound,
   AdequateLeaderFixedScheduledCandidateRemainingActionCreditIsBounded,
   FS_Interval, FS_Product, FS_Subset,
   FS_CardinalityType, IsaT(1200)
   DEF AdequateLeaderFixedCutCumulativeActionDebt,
       AdequateLeaderFixedCutCumulativeActionTokens,
       AdequateLeaderFixedCandidateRemainingActionCredit,
       AdequateLeaderFixedCandidateRemainingRouteActionCredit,
       AsyncCandidateProducerActionEpisodeBudget

THEOREM AdequateLeaderFixedCutCumulativeActionDebtFitsPhysicalBudget ==
  \A node \in ValidatorIds, cutoffOrdinal \in Nat:
    /\ AsyncConfiguration
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    => /\ AdequateLeaderFixedCutCumulativeActionDebt(
              node, cutoffOrdinal)
              \in Nat
       /\ AdequateLeaderFixedCutCumulativeActionDebt(
              node, cutoffOrdinal)
              <= AsyncCandidatePhysicalServiceBudget
BY AdequateLeaderFixedCutCumulativeActionDebtFitsEpisodeBudget, SMT
   DEF AsyncCandidatePhysicalServiceBudget,
       AsyncCandidateProducerActionEpisodeBudget,
       AsyncRuntimeCycleBudget, AsyncRunnerCycleBudget,
       AsyncDeferredDrainBudget, AsyncIoDrainBudget,
       AsyncConfiguration

AdequateLeaderFixedFinalRouteParentDeparture(parent) ==
  /\ AdequateLeaderFixedPipelineExactParentDeparture(parent)
  /\ CandidateServiceRank(parent)[1] \in 2..3
  /\ AdequateLeaderFixedCandidateRemainingRouteActionCredit(parent) = 1

THEOREM AdequateLeaderFixedOwnedFinalRouteParentConsumesCumulativeDebt ==
  \A parent \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(parent)
        origin == parent.causalOrigin
    IN /\ gst
       /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateLifecycleSchedulerCoverageInvariant
       /\ AdequateLeaderFixedFinalRouteParentDeparture(parent)
       /\ cutoffOrdinal \in Nat \ {0}
       /\ parent
            \in AsyncCausalEpisodeCandidates(
                 parent.node, cutoffOrdinal)
       /\ [AsyncNext]_AsyncAllVars
       /\ AsyncCandidateLifecycleSchedulerCoverageInvariant'
       => /\ AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep(parent)
          /\ (AdequateLeaderFixedCutCumulativeActionDebt(
                parent.node, cutoffOrdinal))'
               < AdequateLeaderFixedCutCumulativeActionDebt(
                   parent.node, cutoffOrdinal)
BY AdequateLeaderFixedExactParentDepartureCarriesLifecycleCut,
   AdequateLeaderFixedSuccessorBatchFitsReservedActionTail,
   AsyncCausalEpisodeOwnedLifecycleCutCannotReplenish,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   FS_CardinalityType, FS_Subset, IsaT(2400)
   DEF AdequateLeaderFixedFinalRouteParentDeparture,
       AdequateLeaderFixedPipelineExactParentDeparture,
       AdequateLeaderFixedCutCumulativeActionDebt,
       AdequateLeaderFixedCutCumulativeActionTokens,
       AdequateLeaderFixedCandidateRemainingActionCredit,
       AdequateLeaderFixedCandidateRemainingRouteActionCredit,
       AdequateLeaderFixedCommandSuccessorBatchActionCredit,
       AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep,
       AsyncCausalEpisodeCandidates,
       CandidateScheduled, AsyncAllVars

AdequateLeaderFixedCumulativeActionDebtCarryAction ==
  \A parent \in AsyncCandidateSet:
    AdequateLeaderFixedFinalRouteParentDeparture(parent)
      => LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(parent)
         IN /\ AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep(parent)
            /\ (AdequateLeaderFixedCutCumulativeActionDebt(
                  parent.node, cutoffOrdinal))'
                 < AdequateLeaderFixedCutCumulativeActionDebt(
                     parent.node, cutoffOrdinal)

AdequateLeaderFixedCumulativeActionDebtCarryProperty(specification) ==
  specification
    => [][AdequateLeaderFixedCumulativeActionDebtCarryAction]_AsyncAllVars

=============================================================================
