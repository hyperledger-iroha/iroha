---- MODULE SumeragiV2AdequateLeaderSelectedOwnerContinuationProofs ----
EXTENDS SumeragiV2AdequateLeaderProducerTransportClosureProofs

(***************************************************************************
Exact continuation boundary after selected-owner physical drain.

The candidate lifecycle table alone cannot close
`AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt`.  Its dormant key is
only `(node, causalOrigin)`, while an ignored no-state-change producer episode
may compact that key without installing a service marker or terminal
tombstone.  Neither representation retains the selected frozen work phase and
route-neutral payload required to identify one occurrence owner.

The producer-continuation table in `SumeragiV2AsyncNetwork` retains exactly
that missing transition identity.  The predicate below binds an active record
to a candidate which was frozen in the physical episode source; it never
reconstructs an owner from the current selector or from a different causal
stage.  An active continuation is still an obligation, not Decision, rank
descent, retirement, or producer progress.

This module therefore preserves the original semantic debt and splits it into
two smaller obligations:

  1. an unrecorded-origin debt, where the last physical carrier has drained
     but no full-identity continuation is active; and
  2. an active-continuation residual, which still must materialize into the
     existing universal occurrence-service goal.

The transition-level source theorem may close the first obligation only after
it covers every ignored/no-state-change departure.  The second obligation is
closed below for the exact frozen-voter owner: ordinary timeout progress
either makes a dormant Reserved record ready/stale, then the existing three
source-class actions fairly drain the finite retained prefix.  Preservation
alone remains insufficient, and no fairness is added for non-voter historical
targets.
***************************************************************************)

AdequateLeaderTargetSelectedOwnerExactProducerContinuation(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates) ==
  \E candidate \in sourceCandidates:
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, sourceOccurrenceRank[1],
         target, leaderContext, leader, leaderView, subject)
    /\ owner =
         AdequateLeaderFrozenCandidateOwnerIdentity(
           candidate, sourceOccurrenceRank[1],
           target, leaderContext, leader, leaderView, subject)
    /\ AsyncCandidateProducerContinuationActiveForIdentity(
         AsyncCandidateServiceIdentity(candidate))

(***************************************************************************
Status-exact continuation witnesses.

`Materialized` is a monotone high-watermark saying that the causal origin
reached a scheduled or durable carrier; it is not current physical ownership.
`Terminal` is the continuation-table endpoint and, after the exact physical
owner drains, feeds only the bounded durable-retirement budget.  Neither
status is silently projected to Decision, occurrence-rank descent, corridor
exit, or a new producer.  The residuals below retain the original semantic
handoff debt so a status change cannot discharge the selected occurrence by
itself.
***************************************************************************)
AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates, status) ==
  /\ status \in AsyncCandidateProducerContinuationStatuses
  /\ \E candidate \in sourceCandidates:
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, sourceOccurrenceRank[1],
         target, leaderContext, leader, leaderView, subject)
    /\ owner =
         AdequateLeaderFrozenCandidateOwnerIdentity(
           candidate, sourceOccurrenceRank[1],
           target, leaderContext, leader, leaderView, subject)
    /\ \E record \in
         AsyncCandidateProducerContinuationRecordsForIdentity(
           AsyncCandidateServiceIdentity(candidate)):
         record.status = status

AdequateLeaderTargetSelectedOwnerExactReservedContinuation(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates) ==
  AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates, "Reserved")

AdequateLeaderTargetSelectedOwnerExactMaterializedContinuation(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates) ==
  AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates, "Materialized")

AdequateLeaderTargetSelectedOwnerExactTerminalContinuation(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates) ==
  AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates, "Terminal")

AdequateLeaderTargetSelectedOwnerExactRestartStableTerminalContinuation(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, owner, sourceCandidates) ==
  \E candidate \in sourceCandidates:
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, sourceOccurrenceRank[1],
         target, leaderContext, leader, leaderView, subject)
    /\ owner =
         AdequateLeaderFrozenCandidateOwnerIdentity(
           candidate, sourceOccurrenceRank[1],
           target, leaderContext, leader, leaderView, subject)
    /\ \E record \in
         AsyncCandidateProducerContinuationRecordsForIdentity(
           AsyncCandidateServiceIdentity(candidate)):
         AsyncCandidateProducerContinuationRestartStableTerminalIn(
           asyncControlServiceState, record)

AdequateLeaderTargetSelectedOwnerActiveContinuationResidual(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner, sourceCandidates) ==
  /\ AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner, sourceCandidates)
  /\ AdequateLeaderTargetSelectedOwnerExactProducerContinuation(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)

AdequateLeaderTargetSelectedOwnerReservedContinuationResidual(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner, sourceCandidates) ==
  /\ AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner, sourceCandidates)
  /\ AdequateLeaderTargetSelectedOwnerExactReservedContinuation(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)

AdequateLeaderTargetSelectedOwnerMaterializedContinuationResidual(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner, sourceCandidates) ==
  /\ AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner, sourceCandidates)
  /\ AdequateLeaderTargetSelectedOwnerExactMaterializedContinuation(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)

AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner, sourceCandidates) ==
  /\ AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner, sourceCandidates)
  /\ AdequateLeaderTargetSelectedOwnerExactTerminalContinuation(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)

AdequateLeaderTargetSelectedOwnerRestartStableTerminalContinuationResidual(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner, sourceCandidates) ==
  /\ AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner, sourceCandidates)
  /\ AdequateLeaderTargetSelectedOwnerExactRestartStableTerminalContinuation(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)

(***************************************************************************
Post-GST Terminal is retirement bookkeeping, not productive progress.

The semantic handoff debt already proves that the selected physical owner is
drained and retains the frozen post-GST corridor.  Because restart/replay is
disabled there, the exact Terminal continuation places that same immutable
owner in the bounded producer-continuation retirement set.  The stricter
restart-stable predicate above remains available to pre-GST/reset consumers.
Retirement may be remembered by the subject-switch budget, but it is
deliberately not a universal occurrence-service goal, Decision, rank descent,
or a new producer.
***************************************************************************)
THEOREM AdequateLeaderSelectedOwnerTerminalContinuationIsRetirementBookkeeping ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner, sourceCandidates:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner, sourceCandidates)
    => /\ owner \in
              AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet(
                target, leaderContext, leader, leaderView, subject)
       /\ AdequateLeaderTargetOccurrenceOwnerRetirementClosed(
            target, leaderContext, leader, leaderView,
            subject, sourceOccurrenceRank, owner)
       /\ owner \in
              AdequateLeaderTargetDurablyRetiredOwnerIdentitySet(
                target, leaderContext, leader, leaderView)
BY AdequateLeaderSelectedOwnerNonContinuationDrainsExactLiveOwner,
   AdequateLeaderClosedOccurrenceOwnerIsDurablyRetired,
   IsaT(300)
   DEF AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual,
       AdequateLeaderTargetSelectedOwnerExactTerminalContinuation,
       AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus,
       AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt,
       AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet,
       AdequateLeaderCandidateProducerContinuationRetirementMemory,
       AdequateLeaderTargetOccurrenceOwnerRetirementClosed,
       AsyncCandidateProducerContinuationTerminalForIdentity,
       AsyncCandidateProducerContinuationTerminalForIdentityIn,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationRestartStableTerminalIn

THEOREM AdequateLeaderOffSubjectTerminalContinuationProjectsUniversalGoal ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner, sourceCandidates:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner, sourceCandidates)
    /\ ~AdequateLeaderTargetProtocolSubjectSource(
         target, leaderContext, leader, leaderView, subject)
      => AdequateLeaderTargetUniversalOccurrenceServiceGoal(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, known, owner)
BY AdequateLeaderSelectedOwnerTerminalContinuationIsRetirementBookkeeping,
   IsaT(300)
   DEF AdequateLeaderTargetUniversalOccurrenceServiceGoal,
       AdequateLeaderTargetOccurrenceEqualOwnerOrProducerEpisodeGoal,
       AdequateLeaderTargetOffSubjectOccurrenceDrainGoal,
       AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual,
       AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt

AdequateLeaderTargetSelectedOwnerReservedContinuationStepGoal(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner, sourceCandidates) ==
  \/ AdequateLeaderTargetUniversalOccurrenceServiceGoal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner)
  \/ AdequateLeaderTargetSelectedOwnerMaterializedContinuationResidual(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner, sourceCandidates)
  \/ AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner, sourceCandidates)

AdequateLeaderTargetSelectedOwnerUnrecordedOriginDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner, sourceCandidates) ==
  /\ AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner, sourceCandidates)
  /\ ~AdequateLeaderTargetSelectedOwnerExactProducerContinuation(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)

THEOREM AdequateLeaderSelectedOwnerSemanticDebtIsActiveOrUnrecorded ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner, sourceCandidates:
    AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
      target, leaderContext, leader, leaderView,
      subject, sourceOccurrenceRank, known, owner, sourceCandidates)
      => \/ AdequateLeaderTargetSelectedOwnerActiveContinuationResidual(
               target, leaderContext, leader, leaderView,
               subject, sourceOccurrenceRank, known, owner, sourceCandidates)
         \/ AdequateLeaderTargetSelectedOwnerUnrecordedOriginDebt(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, known, owner, sourceCandidates)
BY Isa
   DEF AdequateLeaderTargetSelectedOwnerActiveContinuationResidual,
       AdequateLeaderTargetSelectedOwnerUnrecordedOriginDebt

THEOREM AdequateLeaderSelectedOwnerActiveResidualIsReservedOrMaterialized ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner, sourceCandidates:
    AdequateLeaderTargetSelectedOwnerActiveContinuationResidual(
      target, leaderContext, leader, leaderView,
      subject, sourceOccurrenceRank, known, owner, sourceCandidates)
      => \/ AdequateLeaderTargetSelectedOwnerReservedContinuationResidual(
               target, leaderContext, leader, leaderView,
               subject, sourceOccurrenceRank, known, owner, sourceCandidates)
         \/ AdequateLeaderTargetSelectedOwnerMaterializedContinuationResidual(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank, known, owner, sourceCandidates)
BY Isa
   DEF AdequateLeaderTargetSelectedOwnerActiveContinuationResidual,
       AdequateLeaderTargetSelectedOwnerReservedContinuationResidual,
       AdequateLeaderTargetSelectedOwnerMaterializedContinuationResidual,
       AdequateLeaderTargetSelectedOwnerExactProducerContinuation,
       AdequateLeaderTargetSelectedOwnerExactReservedContinuation,
       AdequateLeaderTargetSelectedOwnerExactMaterializedContinuation,
       AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus,
       AsyncCandidateProducerContinuationActiveForIdentity,
       AsyncCandidateProducerContinuationActiveForIdentityIn,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn

(***************************************************************************
Exact last-carrier transition gap.

The source is a real one-element physical frontier, not an arbitrary empty-set
terminal chosen in isolation.  `owner \in known` is inherited from the source
non-descent episode and selected occurrence.  The action is a gap only when
the corridor remains, the original universal goal is still false, and the
transition failed to install the exact producer continuation.
***************************************************************************)
AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetEpisodeKnownOwnerSet(
       target, leaderContext, leader, leaderView, subject, known)
  /\ owner \in known
  /\ AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates, 1)
  /\ candidate \in
       AdequateLeaderTargetSelectedOwnerCandidateSet(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, owner)
  /\ candidate \in AsyncCandidateSet
  /\ candidate.subject \in Subjects
  /\ AsyncNext
  /\ AdequateLeaderTargetSelectedOwnerNonContinuationTerminal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)'
  /\ ~AdequateLeaderTargetUniversalOccurrenceServiceGoal(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner)'
  /\ ~AdequateLeaderTargetSelectedOwnerExactProducerContinuation(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates)'

(***************************************************************************
Exact atomic-install boundary and the five remaining debts outside it.

The budget-one frontier identifies the sole selected physical carrier, but
that fact alone does not establish membership in the lifecycle departure set,
presence of its pre-state lifecycle admission, or absence of a prior
continuation record.  In particular, successful state-changing internal
phases outside `AsyncCandidateServiceTrackedKinds` are intentionally absent
from `AsyncCandidateServicesThisStep`, and an off-subject carrier is outside
the deterministic producer-continuation source.

The fresh tracked arm below states every premise of the lower atomic-install
theorem.  Scheduler coverage closes the apparent missing-lifecycle branch:
every candidate in the selected physical owner set is scheduled, and every
scheduled causal origin has an immutable lifecycle record.  The five
remaining alternatives are proof debt; their union is not a semantic terminal
and is never an occurrence-service goal.
***************************************************************************)
AdequateLeaderTargetSelectedOwnerTrackedFreshInstallBoundary(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ AsyncCandidateProducerContinuationSourceAfter(candidate)
  /\ AsyncCandidateLifecycleRecorded(
       candidate.node, candidate.causalOrigin)
  /\ ~AsyncCandidateProducerContinuationRecorded(candidate)

(***************************************************************************
Why the existing successor theorem does not close the untracked arm.

`AdequateLeaderNonDecisionDeclaredSuccessorStrictlyLowersStaticRank` proves
only an ordering fact for a child which is already assumed to have a static
rank.  `DeclaredSuccessorsOwned` adds only `CandidateScheduled(child)'`.
The occurrence terminal instead requires
`AdequateLeaderTargetOccurrenceRankFrontier`: a current-rank child satisfying
the complete frozen target identity, a finite live owner set, and an exact
positive owner count.  Moreover, this budget-one source freezes route-neutral
identity and may contain a stale consumer incarnation, so it cannot invoke
`RankedLeaderOwnerExitDecomposition`, whose source is
`ExactLeaderCandidateRank`.

The missing bridge is therefore an owned action theorem such as
`AdequateLeaderUntrackedDeclaredSuccessorCreatesStrictOccurrenceFrontier`,
plus separate exact projections for wire-only milestones, pacemaker exit,
and discard provenance.  Until those theorems exist, no static-rank child or
post milestone below is called the universal occurrence goal.
***************************************************************************)
AdequateLeaderTargetSelectedOwnerUntrackedPhaseDepartureDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ candidate.kind \notin AsyncCandidateServiceTrackedKinds

AdequateLeaderTargetSelectedOwnerOffSubjectDepartureDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ candidate.kind \in AsyncCandidateServiceTrackedKinds
  /\ candidate.subject
       # AsyncProposalSubject(
           Leader(candidate.consumerContext, candidate.view))

AdequateLeaderTargetSelectedOwnerTrackedNonDepartureDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ candidate.kind \in AsyncCandidateServiceTrackedKinds
  /\ candidate.subject
       = AsyncProposalSubject(
           Leader(candidate.consumerContext, candidate.view))
  /\ candidate \notin AsyncCandidateLifecycleDeparturesThisStep

AdequateLeaderTargetSelectedOwnerTrackedGoalProjectionDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ AsyncCandidateProducerContinuationDeparture(candidate)
  /\ AsyncCandidateProducerContinuationGoalAfter(candidate)

(***************************************************************************
External producer/transport departure debt.

Conditional responsive transport and volatile body reconstruction do not
install a local producer continuation.  Their exact departing candidate is
forwarded as physical producer/transport debt with the same immutable service
identity.  Merely observing the absence of a continuation is never a goal,
retirement, rank descent, or producer progress.
***************************************************************************)
AdequateLeaderTargetSelectedOwnerExternalProducerTransportDepartureDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ AsyncCandidateProducerTransportResidualAfter(candidate)

THEOREM AdequateLeaderSelectedOwnerExternalProducerDebtSplitsPhysicalClass ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    AdequateLeaderTargetSelectedOwnerExternalProducerTransportDepartureDebt(
      target, leaderContext, leader, leaderView,
      subject, sourceOccurrenceRank, known, owner,
      sourceCandidates, candidate)
      => \/ AsyncCandidateConditionalResponsiveTransportResidualAfter(candidate)
         \/ AsyncCandidateVolatileBodyReconstructionResidualAfter(candidate)
BY AsyncCandidateProducerTransportResidualSplitsPhysicalClass, Isa
   DEF AdequateLeaderTargetSelectedOwnerExternalProducerTransportDepartureDebt

(***************************************************************************
The exact GoalAfter projection inside a still-frozen corridor.

The selected candidate freezes the same context, height, and view as the
corridor.  Consequently those three `GoalAfter` arms are false in the
post-state.  A target-local Decision is already the first universal occurrence
goal and contradicts the last-departure source.  The only remaining arm is a
Decision at the leader-owned candidate.  That Decision closes the exact owner
for retirement bookkeeping, but it is not target Decision, occurrence-rank
descent, or productive replenishment; a separate source-frontier theorem is
still required before this residual can satisfy the productive-subject goal.
***************************************************************************)
AdequateLeaderTargetSelectedOwnerTrackedLeaderDecisionRetirementDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetSelectedOwnerTrackedGoalProjectionDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ candidate.node = leader
  /\ NodeHasDecision(leader)'
  /\ AdequateLeaderTargetOccurrenceOwnerRetirementClosed(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner)'

THEOREM AdequateLeaderTrackedGoalProjectionIsLeaderRetirement ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ StrongInductiveInvariant'
    /\ AdequateLeaderTargetSelectedOwnerTrackedGoalProjectionDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    => AdequateLeaderTargetSelectedOwnerTrackedLeaderDecisionRetirementDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
BY AdequateLeaderSelectedOwnerNonContinuationDrainsExactLiveOwner,
   IsaT(600)
   DEF AdequateLeaderTargetSelectedOwnerTrackedGoalProjectionDebt,
       AdequateLeaderTargetSelectedOwnerTrackedLeaderDecisionRetirementDebt,
       AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AdequateLeaderTargetSelectedOwnerNonContinuationTerminal,
       AdequateLeaderTargetUniversalOccurrenceServiceGoal,
       AdequateLeaderTargetOccurrenceDecisionGoal,
       AdequateLeaderTargetOccurrenceOwnerRetirementClosed,
       AdequateLeaderTargetServicedCandidateOwnerIdentitySet,
       AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet,
       AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet,
       AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentityFromPayload,
       AdequateLeaderFrozenTargetCandidateRole,
       ExactLeaderFrozenSemanticIdentity,
       AsyncCandidateProducerContinuationGoalAfter,
       AdequateLeaderFrozenTargetCorridor,
       StrongInductiveInvariant, Safety, TypeInvariant,
       NodeHasDecision

(***************************************************************************
Tracked carrier removal outside the lifecycle-departure set.

This arm can be a producer/executor transfer rather than FIFO/deferred
service.  If `GoalAfter` is true, the same frozen-corridor projection above
leaves only the leader-Decision retirement.  Otherwise the exact candidate
must have left `CandidateScheduled`: the corridor keeps its node responsive,
its frozen identity is unchanged, and application would imply the excluded
node Decision.  The reviewed scheduled-departure theorem then leaves only a
service marker, terminal tombstone, same-origin physical/durable owner, or
monotone reducer milestone.  Those witnesses remain semantic handoff debt;
none is called occurrence-rank descent here.
***************************************************************************)
AdequateLeaderTargetSelectedOwnerTrackedNonDepartureLeaderRetirementDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetSelectedOwnerTrackedNonDepartureDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ AsyncCandidateProducerContinuationGoalAfter(candidate)
  /\ candidate.node = leader
  /\ NodeHasDecision(leader)'
  /\ AdequateLeaderTargetOccurrenceOwnerRetirementClosed(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner)'

AdequateLeaderTargetSelectedOwnerTrackedNonDepartureCarrierHandoffDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetSelectedOwnerTrackedNonDepartureDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ ~AsyncCandidateProducerContinuationGoalAfter(candidate)
  /\ \/ AsyncCandidateServiceTombstoned(candidate)'
     \/ AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter(candidate)
     \/ AsyncCandidateMonotoneSemanticCoverageAfterIn(
          asyncControlServiceState', candidate)
     \/ AsyncCandidateTerminalTombstoned(candidate)'

AdequateLeaderTargetSelectedOwnerTrackedNonDepartureResidual(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  \/ AdequateLeaderTargetSelectedOwnerTrackedNonDepartureLeaderRetirementDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  \/ AdequateLeaderTargetSelectedOwnerTrackedNonDepartureCarrierHandoffDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)

THEOREM AdequateLeaderTrackedNonDepartureGoalAfterIsLeaderRetirement ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ StrongInductiveInvariant'
    /\ AdequateLeaderTargetSelectedOwnerTrackedNonDepartureDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    /\ AsyncCandidateProducerContinuationGoalAfter(candidate)
    => AdequateLeaderTargetSelectedOwnerTrackedNonDepartureLeaderRetirementDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
BY AdequateLeaderSelectedOwnerNonContinuationDrainsExactLiveOwner,
   IsaT(600)
   DEF AdequateLeaderTargetSelectedOwnerTrackedNonDepartureDebt,
       AdequateLeaderTargetSelectedOwnerTrackedNonDepartureLeaderRetirementDebt,
       AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AdequateLeaderTargetSelectedOwnerNonContinuationTerminal,
       AdequateLeaderTargetUniversalOccurrenceServiceGoal,
       AdequateLeaderTargetOccurrenceDecisionGoal,
       AdequateLeaderTargetOccurrenceOwnerRetirementClosed,
       AdequateLeaderTargetServicedCandidateOwnerIdentitySet,
       AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet,
       AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet,
       AdequateLeaderFrozenCandidateOwnerUniverse,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentity,
       AdequateLeaderFrozenCandidateOwnerIdentityFromPayload,
       AdequateLeaderFrozenTargetCandidateRole,
       ExactLeaderFrozenSemanticIdentity,
       AsyncCandidateProducerContinuationGoalAfter,
       AdequateLeaderFrozenTargetCorridor,
       StrongInductiveInvariant, Safety, TypeInvariant,
       NodeHasDecision

THEOREM AdequateLeaderTrackedNonDepartureWithoutGoalRemovesScheduledCarrier ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ StrongInductiveInvariant'
    /\ AdequateLeaderTargetSelectedOwnerTrackedNonDepartureDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    /\ ~AsyncCandidateProducerContinuationGoalAfter(candidate)
    => ~CandidateScheduledAfter(candidate)
BY AppliedNodeHasDecision, IsaT(600)
   DEF AdequateLeaderTargetSelectedOwnerTrackedNonDepartureDebt,
       AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AdequateLeaderTargetSelectedOwnerNonContinuationTerminal,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCandidateRole,
       ExactLeaderFrozenSemanticIdentity,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       AsyncCurrentResponsiveVoters, CurrentVoters,
       AsyncCandidateProducerContinuationGoalAfter,
       AdequateLeaderFrozenTargetCorridor,
       StrongInductiveInvariant, Safety, TypeInvariant,
       CandidateScheduledAfter, CandidateScheduledIn,
       NodeHasApplication, NodeHasDecision

THEOREM AdequateLeaderTrackedNonDepartureWithoutGoalLeavesExactHandoff ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ StrongInductiveInvariant'
    /\ AdequateLeaderTargetSelectedOwnerTrackedNonDepartureDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    /\ ~AsyncCandidateProducerContinuationGoalAfter(candidate)
    => AdequateLeaderTargetSelectedOwnerTrackedNonDepartureCarrierHandoffDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
BY AdequateLeaderTrackedNonDepartureWithoutGoalRemovesScheduledCarrier,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   IsaT(900)
   DEF AdequateLeaderTargetSelectedOwnerTrackedNonDepartureDebt,
       AdequateLeaderTargetSelectedOwnerTrackedNonDepartureCarrierHandoffDebt,
       AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AsyncCandidateLifecycleDeparturesThisStep,
       AsyncCandidateIgnoredWithoutApplicationThisStepSet,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned

THEOREM AdequateLeaderTrackedNonDepartureHasExactResidual ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ StrongInductiveInvariant'
    /\ AdequateLeaderTargetSelectedOwnerTrackedNonDepartureDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    => AdequateLeaderTargetSelectedOwnerTrackedNonDepartureResidual(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
BY AdequateLeaderTrackedNonDepartureGoalAfterIsLeaderRetirement,
   AdequateLeaderTrackedNonDepartureWithoutGoalLeavesExactHandoff,
   Isa
   DEF AdequateLeaderTargetSelectedOwnerTrackedNonDepartureResidual

AdequateLeaderTargetSelectedOwnerTrackedMissingLifecycleDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ AsyncCandidateProducerContinuationSourceAfter(candidate)
  /\ ~AsyncCandidateLifecycleRecorded(
       candidate.node, candidate.causalOrigin)

THEOREM AdequateLeaderSelectedOwnerCannotMissLifecycleRecord ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, owner, candidate:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ candidate \in
         AdequateLeaderTargetSelectedOwnerCandidateSet(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, owner)
    => AsyncCandidateLifecycleRecorded(
         candidate.node, candidate.causalOrigin)
BY Isa
   DEF AdequateLeaderTargetSelectedOwnerCandidateSet,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateLifecycleSchedulerCoverageInvariant,
       AsyncScheduledCandidateOriginsForNode,
       AsyncScheduledCandidateOriginsForNodeIn,
       AsyncCandidateLifecycleRecorded,
       AsyncCandidateLifecycleRecordsFor,
       QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       CandidateScheduled, CandidateScheduledIn, SequenceSet

THEOREM AdequateLeaderTrackedMissingLifecycleDebtIsImpossible ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerTrackedMissingLifecycleDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    => FALSE
BY AdequateLeaderSelectedOwnerCannotMissLifecycleRecord, Isa
   DEF AdequateLeaderTargetSelectedOwnerTrackedMissingLifecycleDebt,
       AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerCandidateSet

AdequateLeaderTargetSelectedOwnerTrackedPriorRecordDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  /\ AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ AsyncCandidateProducerContinuationSourceAfter(candidate)
  /\ AsyncCandidateLifecycleRecorded(
       candidate.node, candidate.causalOrigin)
  /\ AsyncCandidateProducerContinuationRecorded(candidate)

(***************************************************************************
Exact status split for a pre-existing continuation record.

The source candidate belongs to the frozen `sourceCandidates` set, so the
record found by `AsyncCandidateProducerContinuationRecorded` has the exact
route-neutral identity used by the selected-owner witness.  Type invariance
limits its status to Reserved, Materialized, or Terminal.  This theorem is
only a partition: Reserved/Materialized are still obligations, and Terminal
is only retirement bookkeeping.  The scheduled-exclusion invariant is used
separately below after its transition proof is sealed.
***************************************************************************)
AdequateLeaderTargetSelectedOwnerTrackedPriorRecordAtStatusDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate, status) ==
  /\ AdequateLeaderTargetSelectedOwnerTrackedPriorRecordDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  /\ AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, owner, sourceCandidates, status)

AdequateLeaderTargetSelectedOwnerTrackedPriorReservedDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  AdequateLeaderTargetSelectedOwnerTrackedPriorRecordAtStatusDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate, "Reserved")

AdequateLeaderTargetSelectedOwnerTrackedPriorMaterializedDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  AdequateLeaderTargetSelectedOwnerTrackedPriorRecordAtStatusDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate, "Materialized")

AdequateLeaderTargetSelectedOwnerTrackedPriorTerminalDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  AdequateLeaderTargetSelectedOwnerTrackedPriorRecordAtStatusDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate, "Terminal")

THEOREM AdequateLeaderTrackedPriorRecordSplitsExactStatus ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncControlServiceStateTypeInvariant
    /\ AdequateLeaderTargetSelectedOwnerTrackedPriorRecordDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    => \/ AdequateLeaderTargetSelectedOwnerTrackedPriorReservedDebt(
            target, leaderContext, leader, leaderView,
            subject, sourceOccurrenceRank, known, owner,
            sourceCandidates, candidate)
       \/ AdequateLeaderTargetSelectedOwnerTrackedPriorMaterializedDebt(
            target, leaderContext, leader, leaderView,
            subject, sourceOccurrenceRank, known, owner,
            sourceCandidates, candidate)
       \/ AdequateLeaderTargetSelectedOwnerTrackedPriorTerminalDebt(
            target, leaderContext, leader, leaderView,
            subject, sourceOccurrenceRank, known, owner,
            sourceCandidates, candidate)
BY IsaT(300)
   DEF AdequateLeaderTargetSelectedOwnerTrackedPriorRecordDebt,
       AdequateLeaderTargetSelectedOwnerTrackedPriorRecordAtStatusDebt,
       AdequateLeaderTargetSelectedOwnerTrackedPriorReservedDebt,
       AdequateLeaderTargetSelectedOwnerTrackedPriorMaterializedDebt,
       AdequateLeaderTargetSelectedOwnerTrackedPriorTerminalDebt,
       AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus,
       AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AsyncCandidateProducerContinuationRecorded,
       AsyncCandidateProducerContinuationRecordsFor,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncControlServiceStateTypeInvariant,
       AsyncCandidateProducerContinuationRecordSet,
       AsyncCandidateProducerContinuationStatuses

THEOREM AdequateLeaderSelectedOwnerScheduledExcludesProducerContinuation ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, owner, candidate:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ candidate \in
         AdequateLeaderTargetSelectedOwnerCandidateSet(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, owner)
    => ~AsyncCandidateProducerContinuationBlocks(candidate)
BY Isa
   DEF AdequateLeaderTargetSelectedOwnerCandidateSet,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateProducerContinuationScheduledExclusionInvariant,
       QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       CandidateScheduled, CandidateScheduledIn

THEOREM AdequateLeaderTrackedPriorReservedConflictsWithScheduledExclusion ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerTrackedPriorReservedDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    => FALSE
BY AdequateLeaderSelectedOwnerScheduledExcludesProducerContinuation, Isa
   DEF AdequateLeaderTargetSelectedOwnerTrackedPriorReservedDebt,
       AdequateLeaderTargetSelectedOwnerTrackedPriorRecordAtStatusDebt,
       AdequateLeaderTargetSelectedOwnerTrackedPriorRecordDebt,
       AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AsyncCandidateProducerContinuationRecorded,
       AsyncCandidateProducerContinuationBlocks

THEOREM AdequateLeaderTrackedPriorMaterializedConflictsWithScheduledExclusion ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerTrackedPriorMaterializedDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    => FALSE
BY AdequateLeaderSelectedOwnerScheduledExcludesProducerContinuation, Isa
   DEF AdequateLeaderTargetSelectedOwnerTrackedPriorMaterializedDebt,
       AdequateLeaderTargetSelectedOwnerTrackedPriorRecordAtStatusDebt,
       AdequateLeaderTargetSelectedOwnerTrackedPriorRecordDebt,
       AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AsyncCandidateProducerContinuationRecorded,
       AsyncCandidateProducerContinuationBlocks

THEOREM AdequateLeaderTrackedPriorTerminalConflictsWithScheduledExclusion ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerTrackedPriorTerminalDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    => FALSE
BY AdequateLeaderSelectedOwnerScheduledExcludesProducerContinuation, Isa
   DEF AdequateLeaderTargetSelectedOwnerTrackedPriorTerminalDebt,
       AdequateLeaderTargetSelectedOwnerTrackedPriorRecordAtStatusDebt,
       AdequateLeaderTargetSelectedOwnerTrackedPriorRecordDebt,
       AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AsyncCandidateProducerContinuationRecorded,
       AsyncCandidateProducerContinuationBlocks

THEOREM AdequateLeaderTrackedPriorRecordDebtIsImpossible ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerTrackedPriorRecordDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    => FALSE
BY AdequateLeaderTrackedPriorRecordSplitsExactStatus,
   AdequateLeaderTrackedPriorReservedConflictsWithScheduledExclusion,
   AdequateLeaderTrackedPriorMaterializedConflictsWithScheduledExclusion,
   AdequateLeaderTrackedPriorTerminalConflictsWithScheduledExclusion,
   Isa

AdequateLeaderTargetSelectedOwnerOutstandingLastDepartureBoundaryDebt(
    target, leaderContext, leader, leaderView,
    subject, sourceOccurrenceRank, known, owner,
    sourceCandidates, candidate) ==
  \/ AdequateLeaderTargetSelectedOwnerUntrackedPhaseDepartureDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  \/ AdequateLeaderTargetSelectedOwnerOffSubjectDepartureDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  \/ AdequateLeaderTargetSelectedOwnerTrackedNonDepartureResidual(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  \/ AdequateLeaderTargetSelectedOwnerTrackedLeaderDecisionRetirementDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)
  \/ AdequateLeaderTargetSelectedOwnerExternalProducerTransportDepartureDebt(
       target, leaderContext, leader, leaderView,
       subject, sourceOccurrenceRank, known, owner,
       sourceCandidates, candidate)

(***************************************************************************
The lower transition theorem closes the fresh tracked/productive subset of
the action above.  The explicit source predicate excludes off-subject and
untracked reducer tails; the lifecycle and no-existing-record premises keep
the theorem at the exact atomic-install boundary.  Removing those premises
would hide the still-open classification/preservation cases rather than prove
them.
***************************************************************************)
THEOREM AdequateLeaderTrackedProductiveLastDepartureCannotLoseExactOrigin ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    /\ AsyncCandidateProducerContinuationSourceAfter(candidate)
    /\ AsyncCandidateLifecycleRecorded(
         candidate.node, candidate.causalOrigin)
    /\ ~AsyncCandidateProducerContinuationRecorded(candidate)
    => FALSE
BY AsyncCandidateProducerSourceTransitionInstallsExactContinuation,
   IsaT(600)
   DEF AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction,
       AdequateLeaderTargetSelectedOwnerExactProducerContinuation,
       AdequateLeaderTargetSelectedOwnerPhysicalEpisodeFrontier,
       AdequateLeaderTargetSelectedOwnerCandidateSet

THEOREM AdequateLeaderUnrecordedLastDepartureRetainsExactBoundaryDebt ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ StrongInductiveInvariant'
    /\ AdequateLeaderTargetSelectedOwnerUnrecordedLastDepartureAction(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
    => AdequateLeaderTargetSelectedOwnerOutstandingLastDepartureBoundaryDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner,
         sourceCandidates, candidate)
BY AsyncCandidateProducerContinuationDepartureSplitsSourceOrGoal,
   AdequateLeaderTrackedProductiveLastDepartureCannotLoseExactOrigin,
   AdequateLeaderTrackedMissingLifecycleDebtIsImpossible,
   AdequateLeaderTrackedGoalProjectionIsLeaderRetirement,
   AdequateLeaderTrackedNonDepartureHasExactResidual,
   AdequateLeaderTrackedPriorRecordDebtIsImpossible,
   IsaT(600)
   DEF AdequateLeaderTargetSelectedOwnerOutstandingLastDepartureBoundaryDebt,
       AdequateLeaderTargetSelectedOwnerUntrackedPhaseDepartureDebt,
       AdequateLeaderTargetSelectedOwnerOffSubjectDepartureDebt,
       AdequateLeaderTargetSelectedOwnerTrackedNonDepartureResidual,
       AdequateLeaderTargetSelectedOwnerTrackedLeaderDecisionRetirementDebt,
       AdequateLeaderTargetSelectedOwnerExternalProducerTransportDepartureDebt,
       AsyncCandidateProducerTransportResidualAfter,
       AsyncCandidateProducerContinuationDeparture

(***************************************************************************
Reviewed weak fairness closes the immutable continuation prefix.

The prefix tokens and non-replenishment transition facts live in the producer
proof module. This higher module has the full Async liveness dependency chain,
so it can combine those facts with the three source-class-specific fairness
clauses without importing liveness theorems back into the base transition
module.
***************************************************************************)
THEOREM AdequateLeaderReservedContinuationStartsFiniteFrozenPrefix ==
  \A initialContext, target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner, sourceCandidates:
    /\ AsyncFrozenContextAt(initialContext)
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerReservedContinuationResidual(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner, sourceCandidates)
      => \E candidate \in sourceCandidates,
            record \in
              AsyncCandidateProducerContinuationRecordsForIdentity(
                AsyncCandidateServiceIdentity(candidate)),
            budget \in
              AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
           /\ record.status = "Reserved"
           /\ AdequateLeaderFrozenTargetCandidateIdentity(
                candidate, sourceOccurrenceRank[1],
                target, leaderContext, leader, leaderView, subject)
           /\ owner =
                AdequateLeaderFrozenCandidateOwnerIdentity(
                  candidate, sourceOccurrenceRank[1],
                  target, leaderContext, leader, leaderView, subject)
           /\ record.node \in AsyncVotersAt(initialContext)
           /\ AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
                record.node, record.identity, record.ordinal,
                record.address.stage, "Reserved", budget)
BY CandidateProducerContinuationFrozenPrefixRankIsFiniteAndPositive,
   FrozenContextFixesResponsiveVoters,
   IsaT(900)
   DEF AdequateLeaderTargetSelectedOwnerReservedContinuationResidual,
       AdequateLeaderTargetSelectedOwnerExactReservedContinuation,
       AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus,
       AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt,
       AdequateLeaderTargetSelectedOwnerNonContinuationTerminal,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCorridor,
       ExactLeaderFrozenSemanticIdentity,
       AsyncCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRecordSet,
       AsyncCandidateProducerContinuationRecord,
       AsyncCandidateServiceIdentity,
       AsyncCurrentResponsiveVoters, CurrentVoters,
       AsyncControlServiceStateTypeInvariant,
       AsyncStrongTypeInvariant

THEOREM AdequateLeaderMaterializedContinuationStartsFiniteFrozenPrefix ==
  \A initialContext, target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner, sourceCandidates:
    /\ AsyncFrozenContextAt(initialContext)
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerMaterializedContinuationResidual(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner, sourceCandidates)
      => \E candidate \in sourceCandidates,
            record \in
              AsyncCandidateProducerContinuationRecordsForIdentity(
                AsyncCandidateServiceIdentity(candidate)),
            budget \in
              AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
           /\ record.status = "Materialized"
           /\ AdequateLeaderFrozenTargetCandidateIdentity(
                candidate, sourceOccurrenceRank[1],
                target, leaderContext, leader, leaderView, subject)
           /\ owner =
                AdequateLeaderFrozenCandidateOwnerIdentity(
                  candidate, sourceOccurrenceRank[1],
                  target, leaderContext, leader, leaderView, subject)
           /\ record.node \in AsyncVotersAt(initialContext)
           /\ AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
                record.node, record.identity, record.ordinal,
                record.address.stage, "Materialized", budget)
BY CandidateProducerContinuationFrozenPrefixRankIsFiniteAndPositive,
   FrozenContextFixesResponsiveVoters,
   IsaT(900)
   DEF AdequateLeaderTargetSelectedOwnerMaterializedContinuationResidual,
       AdequateLeaderTargetSelectedOwnerExactMaterializedContinuation,
       AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus,
       AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt,
       AdequateLeaderTargetSelectedOwnerNonContinuationTerminal,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderFrozenTargetCorridor,
       ExactLeaderFrozenSemanticIdentity,
       AsyncCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRecordSet,
       AsyncCandidateProducerContinuationRecord,
       AsyncCandidateServiceIdentity,
       AsyncCurrentResponsiveVoters, CurrentVoters,
       AsyncControlServiceStateTypeInvariant,
       AsyncStrongTypeInvariant

THEOREM AdequateLeaderSelectedOwnerSemanticDebtPersistsUnlessUniversalGoal ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner, sourceCandidates:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner, sourceCandidates)
    /\ [AsyncNext]_AsyncAllVars
      => \/ (AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
               target, leaderContext, leader, leaderView,
               subject, sourceOccurrenceRank,
               known, owner, sourceCandidates))'
         \/ (AdequateLeaderTargetUniversalOccurrenceServiceGoal(
               target, leaderContext, leader, leaderView,
               subject, sourceOccurrenceRank, known, owner))'
BY IsaT(2400)
   DEF AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt,
       AdequateLeaderTargetSelectedOwnerNonContinuationTerminal,
       AdequateLeaderTargetSelectedOwnerCandidateSet,
       AdequateLeaderFrozenTargetCandidateIdentity,
       AdequateLeaderTargetUniversalOccurrenceServiceGoal,
       AdequateLeaderTargetOccurrenceDecisionGoal,
       AdequateLeaderTargetOccurrenceStrictlyLowerGoal,
       AdequateLeaderTargetOccurrenceEqualOwnerOrProducerEpisodeGoal,
       AdequateLeaderTargetOccurrenceCorridorExitHandoff,
       AdequateLeaderFrozenTargetCorridor,
       AdequateLeaderTargetEpisodeKnownOwnerSet,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep

THEOREM AdequateLeaderReservedTargetStatusExitProjectsStepGoal ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner, sourceCandidates)
    /\ candidate \in sourceCandidates
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, sourceOccurrenceRank[1],
         target, leaderContext, leader, leaderView, subject)
    /\ owner =
         AdequateLeaderFrozenCandidateOwnerIdentity(
           candidate, sourceOccurrenceRank[1],
           target, leaderContext, leader, leaderView, subject)
    /\ AsyncCandidateProducerContinuationTargetStatusExit(
         AsyncCandidateServiceIdentity(candidate), "Reserved")
      => AdequateLeaderTargetSelectedOwnerReservedContinuationStepGoal(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, known, owner, sourceCandidates)
BY IsaT(1200)
   DEF AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationStatusRank,
       AdequateLeaderTargetSelectedOwnerReservedContinuationStepGoal,
       AdequateLeaderTargetSelectedOwnerMaterializedContinuationResidual,
       AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual,
       AdequateLeaderTargetSelectedOwnerExactMaterializedContinuation,
       AdequateLeaderTargetSelectedOwnerExactTerminalContinuation,
       AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AdequateLeaderFrozenTargetCorridor

THEOREM AdequateLeaderMaterializedTargetStatusExitProjectsTerminalOrGoal ==
  \A target, leaderContext, leader, leaderView,
     subject, sourceOccurrenceRank, known, owner,
     sourceCandidates, candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt(
         target, leaderContext, leader, leaderView,
         subject, sourceOccurrenceRank, known, owner, sourceCandidates)
    /\ candidate \in sourceCandidates
    /\ AdequateLeaderFrozenTargetCandidateIdentity(
         candidate, sourceOccurrenceRank[1],
         target, leaderContext, leader, leaderView, subject)
    /\ owner =
         AdequateLeaderFrozenCandidateOwnerIdentity(
           candidate, sourceOccurrenceRank[1],
           target, leaderContext, leader, leaderView, subject)
    /\ AsyncCandidateProducerContinuationTargetStatusExit(
         AsyncCandidateServiceIdentity(candidate), "Materialized")
      => \/ AdequateLeaderTargetUniversalOccurrenceServiceGoal(
               target, leaderContext, leader, leaderView,
               subject, sourceOccurrenceRank, known, owner)
         \/ AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual(
              target, leaderContext, leader, leaderView,
              subject, sourceOccurrenceRank,
              known, owner, sourceCandidates)
BY IsaT(1200)
   DEF AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationStatusRank,
       AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual,
       AdequateLeaderTargetSelectedOwnerExactTerminalContinuation,
       AdequateLeaderTargetSelectedOwnerExactProducerContinuationAtStatus,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AdequateLeaderFrozenTargetCorridor

(***************************************************************************
Unconditional voter-scoped dormant-reservation closure.

The retained record is not granted a new fair action.  Its frozen voter owns
the ordinary timeout/view corridor already provided by AsyncLiveSpec.  If no
concrete successor or deterministic retirement makes the Reserved record
ready first, timeout progress makes its immutable view stale (or installs a
Decision), which is exactly the existing durable-terminal arm.

The helper is deliberately scoped to AsyncVotersAt(initialContext).  The two
adequate-leader continuation entry theorems below derive that membership from
the selected frozen candidate identity before consuming the helper.

The timeout premise below is the direct lower per-voter residual/decomposition:
it does not use AsyncTemporalClosureTimeoutViewProgressObligation, rotating
leader convergence, adequate-leader closure, or any selected-owner/Authority
provider.  Although that lower theorem is visible here through the transitive
RotatingLeaderProgressProofs import, no rotating theorem is a proof dependency.
This keeps the continuation supplier below those composition layers.
***************************************************************************)
THEOREM AsyncLiveProvidesCandidateProducerContinuationDormantReservationClosure ==
  \A initialContext:
    AsyncCandidateProducerContinuationDormantReservationClosureProperty(
      AsyncLiveSpecAt(initialContext), initialContext)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE
           AsyncCandidateProducerContinuationDormantReservationClosureProperty(
             AsyncLiveSpecAt(initialContext), initialContext)
    <2>1. TimeoutViewProgressProperty(AsyncLiveSpecAt(initialContext))
      BY AsyncLiveProvidesDirectTimeoutViewClosureResidual,
         DirectTimeoutViewDecompositionClosesTimeoutViewProgress
    <2>2. AsyncLiveSpecAt(initialContext)
            => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysUsesFixedResponsiveVoters, PTL
    <2>3. ASSUME NEW node \in AsyncVotersAt(initialContext),
                  NEW record
                    \in AsyncCandidateProducerContinuationRecordSet,
                  AsyncLiveSpecAt(initialContext)
           PROVE
             /\ gst
             /\ record =
                  AsyncCandidateProducerContinuationSelectedResolutionRecord(
                    node)
             /\ record.status = "Reserved"
             /\ record
                  \in
                    AsyncCandidateProducerContinuationResolutionRecordsForNode(
                      node)
               ~>
                 AsyncCandidateProducerContinuationDormantReservationGoal(
                   record)
      <3>1. []( /\ gst
                 /\ record =
                      AsyncCandidateProducerContinuationSelectedResolutionRecord(
                        node)
                 /\ record.status = "Reserved"
                 /\ record
                      \in
                        AsyncCandidateProducerContinuationResolutionRecordsForNode(
                          node)
                   => /\ node \in AsyncCurrentResponsiveVoters
                      /\ record.node = node
                      /\ nodeView[node] = record.view
                      /\ ~NodeHasDecision(node))
        BY <2>2, <2>3, PTL, Isa
           DEF AsyncCandidateProducerContinuationResolutionRecordsForNode
      <3>2. (gst
                /\ nodeView[node] = record.view
                /\ ~NodeHasDecision(node))
               ~> (nodeView[node] > record.view
                     \/ NodeHasDecision(node))
        BY <2>1, <2>2, <2>3, PTL
           DEF TimeoutViewProgressProperty
      <3>3. []( /\ record.node = node
                 /\ (nodeView[node] > record.view
                       \/ NodeHasDecision(node))
                   => AsyncCandidateProducerContinuationDormantReservationGoal(
                        record))
        BY Isa, PTL
           DEF AsyncCandidateProducerContinuationDormantReservationGoal,
               AsyncCandidateProducerContinuationDurableTerminal
      <3> QED BY <3>1, <3>2, <3>3, PTL
    <2> QED BY <2>3
         DEF AsyncCandidateProducerContinuationDormantReservationClosureProperty
  <1> QED BY <1>1

THEOREM CandidateProducerDormantClosureProvidesFrozenPrefixDescent ==
  \A initialContext:
    AsyncCandidateProducerContinuationDormantReservationClosureProperty(
      AsyncLiveSpecAt(initialContext), initialContext)
      => AsyncCandidateProducerContinuationFrozenPrefixDescentProperty(
           AsyncLiveSpecAt(initialContext), initialContext)
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   CandidateProducerContinuationResolutionSplitsReviewedSourceClass,
   ConditionalTransportContinuationReadyEnablesFairService,
   VolatileBodyContinuationReadyEnablesFairService,
   LocalContinuationReadyEnablesFairResolution,
   CandidateProducerContinuationDormantGoalIsReadyOrExited,
   CandidateProducerContinuationFrozenPrefixStepCannotReplenish,
   CandidateProducerContinuationFairResolutionStrictlyDescendsFrozenPrefix,
   AsyncLiveSpecProjectsAsyncSpec,
   PTL, IsaT(1800)
   DEF AsyncCandidateProducerContinuationDormantReservationClosureProperty,
       AsyncCandidateProducerContinuationDormantReservationGoal,
       AsyncCandidateProducerContinuationFrozenPrefixDescentProperty,
       AsyncCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationSelectedSourceClass,
       AsyncCandidateProducerContinuationSourceClasses,
       AsyncLiveSpecAt, AsyncFairnessAt

THEOREM CandidateProducerDormantClosureClosesFrozenPrefix ==
  \A initialContext:
    AsyncCandidateProducerContinuationDormantReservationClosureProperty(
      AsyncLiveSpecAt(initialContext), initialContext)
      => AsyncCandidateProducerContinuationFrozenPrefixClosureProperty(
           AsyncLiveSpecAt(initialContext), initialContext)
BY CandidateProducerDormantClosureProvidesFrozenPrefixDescent,
   CandidateProducerContinuationFrozenPrefixRankOrderingIsWellFounded,
   WellFoundedLeadsTo
   DEF AsyncCandidateProducerContinuationFrozenPrefixDescentProperty,
       AsyncCandidateProducerContinuationFrozenPrefixClosureProperty,
       AsyncCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier

THEOREM AsyncLiveProvidesCandidateProducerContinuationFrozenPrefixClosure ==
  \A initialContext:
    AsyncCandidateProducerContinuationFrozenPrefixClosureProperty(
      AsyncLiveSpecAt(initialContext), initialContext)
BY AsyncLiveProvidesCandidateProducerContinuationDormantReservationClosure,
   CandidateProducerDormantClosureClosesFrozenPrefix

AdequateLeaderTargetSelectedOwnerContinuationOriginExposureProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          known \in
            SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          sourceCandidates \in SUBSET AsyncCandidateSet:
         AdequateLeaderTargetSelectedOwnerUnrecordedOriginDebt(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, known, owner, sourceCandidates)
           ~> (AdequateLeaderTargetUniversalOccurrenceServiceGoal(
                 target, leaderContext, leader, leaderView,
                 subject, sourceOccurrenceRank, known, owner)
                \/ AdequateLeaderTargetSelectedOwnerActiveContinuationResidual(
                     target, leaderContext, leader, leaderView,
                     subject, sourceOccurrenceRank, known, owner,
                     sourceCandidates))

AdequateLeaderTargetSelectedOwnerReservedContinuationStepProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          known \in
            SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          sourceCandidates \in SUBSET AsyncCandidateSet:
         AdequateLeaderTargetSelectedOwnerReservedContinuationResidual(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, known, owner, sourceCandidates)
           ~> AdequateLeaderTargetSelectedOwnerReservedContinuationStepGoal(
                target, leaderContext, leader, leaderView,
                subject, sourceOccurrenceRank, known, owner,
                sourceCandidates)

AdequateLeaderTargetSelectedOwnerMaterializedContinuationClosureProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          known \in
            SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          sourceCandidates \in SUBSET AsyncCandidateSet:
         AdequateLeaderTargetSelectedOwnerMaterializedContinuationResidual(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, known, owner, sourceCandidates)
           ~> AdequateLeaderTargetUniversalOccurrenceServiceGoal(
                target, leaderContext, leader, leaderView,
                subject, sourceOccurrenceRank, known, owner)

AdequateLeaderTargetSelectedOwnerMaterializedContinuationStepProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          known \in
            SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          sourceCandidates \in SUBSET AsyncCandidateSet:
         AdequateLeaderTargetSelectedOwnerMaterializedContinuationResidual(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, known, owner, sourceCandidates)
           ~> (AdequateLeaderTargetUniversalOccurrenceServiceGoal(
                 target, leaderContext, leader, leaderView,
                 subject, sourceOccurrenceRank, known, owner)
                \/ AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual(
                     target, leaderContext, leader, leaderView,
                     subject, sourceOccurrenceRank,
                     known, owner, sourceCandidates))

AdequateLeaderTargetSelectedOwnerTerminalContinuationProjectionProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          known \in
            SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          sourceCandidates \in SUBSET AsyncCandidateSet:
         AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, known, owner, sourceCandidates)
           ~> AdequateLeaderTargetUniversalOccurrenceServiceGoal(
                target, leaderContext, leader, leaderView,
                subject, sourceOccurrenceRank, known, owner)

(***************************************************************************
Conditional authority-receipt bridge.

The exact selected-owner debts retain the frozen target corridor.  Therefore
the independent authority-bound deadline receipt/carry interface may close
either debt through the target-local Decision arm of the universal goal.
These are deliberately conditional compositions: this module does not assert
the receipt/carry property for AsyncLiveSpec and does not feed occurrence
service or producer closure back into its own premise.
***************************************************************************)
THEOREM AdequateLeaderAuthorityReceiptCarryProvidesSelectedOwnerOriginExposure ==
  \A specification:
    AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty(
      specification)
      =>
        AdequateLeaderTargetSelectedOwnerContinuationOriginExposureProperty(
          specification)
BY PTL
   DEF AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty,
       AdequateLeaderAuthorityBoundReceiptAcquisitionProperty,
       AdequateLeaderAuthorityBoundActiveReceiptServiceProperty,
       AdequateLeaderTargetAuthorityBoundActiveReceiptSource,
       AdequateLeaderTargetSelectedOwnerContinuationOriginExposureProperty,
       AdequateLeaderTargetSelectedOwnerUnrecordedOriginDebt,
       AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt,
       AdequateLeaderTargetSelectedOwnerNonContinuationTerminal,
       AdequateLeaderTargetUniversalOccurrenceServiceGoal,
       AdequateLeaderTargetOccurrenceDecisionGoal

THEOREM AdequateLeaderAuthorityReceiptCarryProvidesTerminalContinuationProjection ==
  \A specification:
    AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty(
      specification)
      =>
        AdequateLeaderTargetSelectedOwnerTerminalContinuationProjectionProperty(
          specification)
BY PTL
   DEF AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty,
       AdequateLeaderAuthorityBoundReceiptAcquisitionProperty,
       AdequateLeaderAuthorityBoundActiveReceiptServiceProperty,
       AdequateLeaderTargetAuthorityBoundActiveReceiptSource,
       AdequateLeaderTargetSelectedOwnerTerminalContinuationProjectionProperty,
       AdequateLeaderTargetSelectedOwnerTerminalContinuationResidual,
       AdequateLeaderTargetSelectedOwnerSemanticHandoffDebt,
       AdequateLeaderTargetSelectedOwnerNonContinuationTerminal,
       AdequateLeaderTargetUniversalOccurrenceServiceGoal,
       AdequateLeaderTargetOccurrenceDecisionGoal

AdequateLeaderTargetSelectedOwnerActiveContinuationClosureProperty(
    specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          sourceOccurrenceRank \in
            AdequateLeaderTargetOccurrenceRankCarrier,
          known \in
            SUBSET AdequateLeaderFrozenOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          owner \in
            AdequateLeaderFrozenCandidateOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          sourceCandidates \in SUBSET AsyncCandidateSet:
         AdequateLeaderTargetSelectedOwnerActiveContinuationResidual(
           target, leaderContext, leader, leaderView,
           subject, sourceOccurrenceRank, known, owner, sourceCandidates)
           ~> AdequateLeaderTargetUniversalOccurrenceServiceGoal(
                target, leaderContext, leader, leaderView,
                subject, sourceOccurrenceRank, known, owner)

THEOREM CandidateProducerDormantClosureProvidesReservedContinuationStep ==
  \A initialContext:
    AsyncCandidateProducerContinuationDormantReservationClosureProperty(
      AsyncLiveSpecAt(initialContext), initialContext)
      => AdequateLeaderTargetSelectedOwnerReservedContinuationStepProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysKeepsFrozenContext,
   CandidateProducerDormantClosureClosesFrozenPrefix,
   AdequateLeaderReservedContinuationStartsFiniteFrozenPrefix,
   AdequateLeaderSelectedOwnerSemanticDebtPersistsUnlessUniversalGoal,
   AdequateLeaderReservedTargetStatusExitProjectsStepGoal,
   AsyncLiveSpecProjectsAsyncSpec,
   PTL, IsaT(1800)
   DEF AdequateLeaderTargetSelectedOwnerReservedContinuationStepProperty,
       AdequateLeaderTargetSelectedOwnerReservedContinuationResidual,
       AdequateLeaderTargetSelectedOwnerReservedContinuationStepGoal,
       AsyncCandidateProducerContinuationFrozenPrefixClosureProperty,
       AsyncCandidateProducerContinuationTargetStatusExit

THEOREM CandidateProducerDormantClosureProvidesMaterializedContinuationStep ==
  \A initialContext:
    AsyncCandidateProducerContinuationDormantReservationClosureProperty(
      AsyncLiveSpecAt(initialContext), initialContext)
      => AdequateLeaderTargetSelectedOwnerMaterializedContinuationStepProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncSpecAlwaysKeepsFrozenContext,
   CandidateProducerDormantClosureClosesFrozenPrefix,
   AdequateLeaderMaterializedContinuationStartsFiniteFrozenPrefix,
   AdequateLeaderSelectedOwnerSemanticDebtPersistsUnlessUniversalGoal,
   AdequateLeaderMaterializedTargetStatusExitProjectsTerminalOrGoal,
   AsyncLiveSpecProjectsAsyncSpec,
   PTL, IsaT(1800)
   DEF AdequateLeaderTargetSelectedOwnerMaterializedContinuationStepProperty,
       AdequateLeaderTargetSelectedOwnerMaterializedContinuationResidual,
       AsyncCandidateProducerContinuationFrozenPrefixClosureProperty,
       AsyncCandidateProducerContinuationTargetStatusExit

THEOREM AsyncLiveProvidesAdequateLeaderTargetSelectedOwnerReservedContinuationStep ==
  \A initialContext:
    AdequateLeaderTargetSelectedOwnerReservedContinuationStepProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesCandidateProducerContinuationDormantReservationClosure,
   CandidateProducerDormantClosureProvidesReservedContinuationStep

THEOREM AsyncLiveProvidesAdequateLeaderTargetSelectedOwnerMaterializedContinuationStep ==
  \A initialContext:
    AdequateLeaderTargetSelectedOwnerMaterializedContinuationStepProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesCandidateProducerContinuationDormantReservationClosure,
   CandidateProducerDormantClosureProvidesMaterializedContinuationStep

THEOREM AdequateLeaderMaterializedStepAndTerminalProjectionProvideClosure ==
  \A specification:
    /\ AdequateLeaderTargetSelectedOwnerMaterializedContinuationStepProperty(
         specification)
    /\ AdequateLeaderTargetSelectedOwnerTerminalContinuationProjectionProperty(
         specification)
      => AdequateLeaderTargetSelectedOwnerMaterializedContinuationClosureProperty(
           specification)
BY PTL
   DEF AdequateLeaderTargetSelectedOwnerMaterializedContinuationStepProperty,
       AdequateLeaderTargetSelectedOwnerTerminalContinuationProjectionProperty,
       AdequateLeaderTargetSelectedOwnerMaterializedContinuationClosureProperty

THEOREM AdequateLeaderSelectedOwnerStatusClosureProvidesActiveClosure ==
  \A specification:
    /\ AdequateLeaderTargetSelectedOwnerReservedContinuationStepProperty(
         specification)
    /\ AdequateLeaderTargetSelectedOwnerMaterializedContinuationClosureProperty(
         specification)
    /\ AdequateLeaderTargetSelectedOwnerTerminalContinuationProjectionProperty(
         specification)
    => AdequateLeaderTargetSelectedOwnerActiveContinuationClosureProperty(
         specification)
BY AdequateLeaderSelectedOwnerActiveResidualIsReservedOrMaterialized, PTL
   DEF AdequateLeaderTargetSelectedOwnerReservedContinuationStepProperty,
       AdequateLeaderTargetSelectedOwnerMaterializedContinuationClosureProperty,
       AdequateLeaderTargetSelectedOwnerTerminalContinuationProjectionProperty,
       AdequateLeaderTargetSelectedOwnerActiveContinuationClosureProperty,
       AdequateLeaderTargetSelectedOwnerReservedContinuationStepGoal

THEOREM AdequateLeaderSelectedOwnerContinuationExposureAndClosureProvideSemanticHandoff ==
  \A specification:
    /\ AdequateLeaderTargetSelectedOwnerContinuationOriginExposureProperty(
         specification)
    /\ AdequateLeaderTargetSelectedOwnerActiveContinuationClosureProperty(
         specification)
    => AdequateLeaderTargetSelectedOwnerSemanticHandoffProperty(
         specification)
BY AdequateLeaderSelectedOwnerSemanticDebtIsActiveOrUnrecorded, PTL
   DEF AdequateLeaderTargetSelectedOwnerContinuationOriginExposureProperty,
       AdequateLeaderTargetSelectedOwnerActiveContinuationClosureProperty,
       AdequateLeaderTargetSelectedOwnerSemanticHandoffProperty

=============================================================================
