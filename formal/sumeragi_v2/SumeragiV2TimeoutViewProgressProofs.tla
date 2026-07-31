---- MODULE SumeragiV2TimeoutViewProgressProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs,
        SumeragiV2AsyncCausalWorkBudgetProofs,
        SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs

(***************************************************************************
Direct timeout/view temporal decomposition.

This leaf is below locked-body reproposal and rotating-leader progress.  It
therefore cannot use either rotating-leader clause, aggregate responsive
Decision convergence, or a terminal Decision reached through adequate-leader
service to discharge the timeout property.

For one frozen `(target, roundView)` the direct corridor is:

  1. source ownership exposes a higher-view TC/Decision frontier or a finite
     catch-up rank;
  2. catch-up follows the target's retained previous-view TC to one lagging
     source and consumes every missing `(source, intermediate view)` slot;
  3. once every responsive voter is at `roundView`, receipt service consumes
     the finite set of missing `(target, signer, roundView)` receipts;
  4. the exact last receipt exposes the target's TC formation/install
     frontier; and
  5. PersistInstallTC strictly advances the target view.

The catch-up rank is deliberately not the number of lagging validators.  One
validator may advance several views while remaining below `roundView`, which
would leave that count unchanged.  `TimeoutCatchupDebtSlots` instead contains
one slot for every missing intermediate view, so every certified view advance
removes concrete debt.

Four temporal corridors are explicit below.  The ownership seam and the
finite catch-up/receipt coordination are discharged from the asynchronous
invariant and exact lower corridors.  The armed-Runtime corridor is discharged
from source-derived immutable ticket ownership; the other three remain
factored into exact physical kernel properties rather than hidden in one
composite claim:

  * selected packet, Candidate, and Serve lifecycle service while Tick is
    blocked;
  * armed Runtime scheduling and the exact BeginTimeout reducer/WAL handoff;
  * source-isolated retained-control, packet, ingress, and vote-reducer
    delivery; and
  * exact TC/Commit-certificate transport, reducer/WAL, and causal origin.

No scheduler action, fairness domain, or network transition is redefined.
The armed-Runtime corridor below discharges its finite immutable Candidate
prefix and names the separate exact-ingress closure for predecessors whose
shared scheduler ordinals are no later than the timeout owner.  Its one
aggregate Serve seam also states the complementary repaired rule: every later
ticket interleaves the bounded older Runtime episode before its target-only
turn.  No eventual ticket-absence claim, finite `Views` bound, or aggregate
fairness premise is introduced to hide either side of that ordinal cut.
***************************************************************************)

TimeoutDirectReleaseSource(target, roundView) ==
  /\ gst
  /\ target \in AsyncCurrentResponsiveVoters
  /\ nodeView[target] = roundView
  /\ ~NodeHasDecision(target)

TimeoutDirectGoal(target, roundView) ==
  TimeoutViewGoal(target, roundView)

TimeoutDeadlineArmedOwner(node, roundView) ==
  /\ TimeoutRoundStable(node, roundView)
  /\ ~NodeTimedOut(node, roundView)
  /\ \/ asyncNow >= asyncNodeDeadlines[node]
     \/ "TimeoutElapsed" \in asyncOutstandingTags[node]

(***************************************************************************
Finite two-stage rank.

Stage 2 is voter catch-up and stage 1 is timeout-receipt collection.  The
ordinary lexicographic natural ordering therefore permits an arbitrary finite
receipt rank after catch-up reaches zero while still making the stage change
strictly descend.
***************************************************************************)

TimeoutCatchupDebtSlots(roundView) ==
  {slot \in AsyncCurrentResponsiveVoters \X (0..roundView):
     /\ nodeView[slot[1]] <= slot[2]
     /\ slot[2] < roundView}

TimeoutCatchupDebtRank(roundView) ==
  Cardinality(TimeoutCatchupDebtSlots(roundView))

TimeoutCatchupDebtAtRank(target, roundView, rank) ==
  /\ TimeoutRoundStable(target, roundView)
  /\ ~ResponsiveDecisionExists
  /\ \A source \in AsyncCurrentResponsiveVoters:
       nodeView[source] <= roundView
  /\ TimeoutCatchupDebtRank(roundView) = rank

TimeoutProgressRankCarrier == (1..2) \X Nat

TimeoutProgressRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), 1..2, Nat)

TimeoutProgressRankFrontier(target, roundView, rank) ==
  /\ rank \in TimeoutProgressRankCarrier
  /\ IF rank[1] = 2
     THEN TimeoutCatchupDebtAtRank(target, roundView, rank[2])
     ELSE TimeoutReceiptAtRank(target, roundView, rank[2])

TimeoutProgressRankedFrontier(target, roundView) ==
  \E rank \in TimeoutProgressRankCarrier:
    TimeoutProgressRankFrontier(target, roundView, rank)

(***************************************************************************
Exact non-rank owners.

Every disjunct retains the target identity.  In particular, a global formed
TC is not accepted: the certificate must be in this target's exact retained,
transport, ingress, reducer, or install frontier.
***************************************************************************)

TimeoutDirectOwnerFrontier(target, roundView) ==
  \/ DecisionPropagationFrontier(target)
  \/ TcFrontier(target, roundView)
  \/ TimeoutCertificateFormationFrontier(target, roundView)

TimeoutProgressRankStrictGoal(target, roundView, sourceRank) ==
  \/ TimeoutDirectGoal(target, roundView)
  \/ TimeoutDirectOwnerFrontier(target, roundView)
  \/ \E lowerRank \in
       SetLessThan(
         sourceRank,
         TimeoutProgressRankOrdering,
         TimeoutProgressRankCarrier):
       TimeoutProgressRankFrontier(
         target, roundView, lowerRank)

(***************************************************************************
Static rank and source-exposure facts.
***************************************************************************)

THEOREM TimeoutCatchupDebtRankIsNatural ==
  \A roundView \in Views:
    AsyncTypeInvariant
      => TimeoutCatchupDebtRank(roundView) \in Nat
PROOF
  <1>1. ASSUME NEW roundView \in Views,
                AsyncTypeInvariant
         PROVE TimeoutCatchupDebtRank(roundView) \in Nat
    <2>1. /\ roundView \in Nat
           /\ IsFiniteSet(AsyncCurrentResponsiveVoters)
      BY <1>1, RuntimeValidatorIdsAreFinite, FS_Subset, Isa
         DEF AsyncTypeInvariant, AsyncCurrentResponsiveVoters,
             CurrentVoters, CurrentEpoch, Views
    <2>2. IsFiniteSet(
             AsyncCurrentResponsiveVoters \X (0..roundView))
      BY <2>1, FS_Interval, FS_Product
    <2>3. TimeoutCatchupDebtSlots(roundView)
             \subseteq
               AsyncCurrentResponsiveVoters \X (0..roundView)
      BY DEF TimeoutCatchupDebtSlots
    <2>4. IsFiniteSet(TimeoutCatchupDebtSlots(roundView))
      BY <2>2, <2>3, FS_Subset
    <2> QED BY <2>4, FS_CardinalityType
         DEF TimeoutCatchupDebtRank
  <1> QED BY <1>1

THEOREM TimeoutDirectReleaseSourceIsRoundStable ==
  \A target \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutDirectReleaseSource(target, roundView)
    => TimeoutRoundStable(target, roundView)
BY GstResponsiveNodesAreUp, Isa
   DEF TimeoutDirectReleaseSource, TimeoutRoundStable,
       AsyncStrongTypeInvariant

THEOREM TimeoutRoundStableExposesRankOrExactOwner ==
  \A target \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    /\ AsyncTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ TimeoutRoundStable(target, roundView)
    => \/ TimeoutDirectGoal(target, roundView)
       \/ TimeoutDirectOwnerFrontier(target, roundView)
       \/ TimeoutProgressRankedFrontier(target, roundView)
BY TimeoutCatchupDebtRankIsNatural,
   ResponsiveAuthoritySuppliesEveryTcFrontier,
   IsaT(240)
   DEF TimeoutDirectGoal, TimeoutDirectOwnerFrontier,
       TimeoutProgressRankedFrontier,
       TimeoutProgressRankFrontier,
       TimeoutCatchupDebtAtRank, TimeoutCatchupDebtRank,
       TimeoutRoundStable, ResponsiveDecisionExists,
       TimeoutViewOwnershipInvariant,
       ResponsiveViewCertificateAuthority,
       DecisionPropagationFrontier, DecisionSourceAt,
       NodeHasDecision, TcFrontier

(***************************************************************************
Exact catch-up owner.

A lagging source must not be advanced by treating the target's already-true
`TimeoutViewGoal(target, nodeView[laggingSource])` as progress.  The real
owner is the target's retained TC for `roundView - 1`.  Installing that exact
certificate at the lagging source raises its view to at least `roundView` and
therefore retires every debt slot for that source.
***************************************************************************)

THEOREM TimeoutLaggingSourceHasExactTcFrontier ==
  \A target, laggingSource \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    /\ AsyncTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ TimeoutRoundStable(target, roundView)
    /\ nodeView[laggingSource] < roundView
    => /\ roundView > 0
       /\ TcFrontier(laggingSource, roundView - 1)
BY ResponsiveAuthoritySuppliesEveryTcFrontier,
   IsaT(180)
   DEF TimeoutViewOwnershipInvariant,
       TimeoutRoundStable,
       ResponsiveViewCertificateAuthority,
       AsyncTypeInvariant, CurrentVoters, CurrentEpoch,
       AsyncCurrentResponsiveVoters, Views

TimeoutLaggingSourceCatchupOutcome(
    target, roundView, laggingSource) ==
  \/ TimeoutDirectGoal(target, roundView)
  \/ TimeoutDirectOwnerFrontier(target, roundView)
  \/ nodeView[laggingSource] >= roundView

THEOREM TimeoutLaggingSourceGoalSuppliesCatchupOutcome ==
  \A target, laggingSource \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ roundView > 0
    /\ laggingSource # target
    /\ TimeoutDirectGoal(laggingSource, roundView - 1)
    => TimeoutLaggingSourceCatchupOutcome(
         target, roundView, laggingSource)
BY IsaT(240)
   DEF TimeoutLaggingSourceCatchupOutcome,
       TimeoutDirectGoal, TimeoutDirectOwnerFrontier,
       TimeoutViewOwnershipInvariant,
       DecisionPropagationFrontier, DecisionSourceAt,
       NodeHasDecision, AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       Views

(***************************************************************************
Timeout proof seams and derived closures.

The ownership seam is discharged immediately below.  The clock and finite
coordination properties are then derived from concrete action/rank kernels;
the aggregate residual at the end contains only the exact physical kernels
listed in the module header.
***************************************************************************)

TimeoutViewOwnershipPreservationProperty(specification) ==
  specification => []TimeoutViewOwnershipInvariant

THEOREM TimeoutViewOwnershipPreservationObligation ==
  \A initialContext:
    TimeoutViewOwnershipPreservationProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   TimeoutViewOwnershipInvariantFromAsyncSpec
   DEF TimeoutViewOwnershipPreservationProperty

TimeoutDeadlineClockConvergenceProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views:
         TimeoutRoundTrigger(source, sourceView)
           ~> (TimeoutDirectGoal(source, sourceView)
                \/ TimeoutDeadlineArmedOwner(source, sourceView)
                \/ \E vote \in TimeoutVoteRecordSet:
                     TimeoutOrigin(source, sourceView, vote))

TimeoutPredeadlineClockExit(source, sourceView) ==
  \/ TimeoutDirectGoal(source, sourceView)
  \/ TimeoutDeadlineArmedOwner(source, sourceView)
  \/ \E vote \in TimeoutVoteRecordSet:
       TimeoutOrigin(source, sourceView, vote)

TimeoutPredeadlineClockAtRank(source, sourceView, rank) ==
  /\ rank \in Nat
  /\ rank > 0
  /\ TimeoutRoundTrigger(source, sourceView)
  /\ asyncNow < asyncNodeDeadlines[source]
  /\ asyncNodeDeadlines[source] = asyncNow + rank

THEOREM TimeoutPredeadlineClockSourceHasPositiveNaturalRank ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutRoundTrigger(source, sourceView)
    /\ ~TimeoutPredeadlineClockExit(source, sourceView)
    => \E rank \in Nat:
         TimeoutPredeadlineClockAtRank(
           source, sourceView, rank)
BY SMT
   DEF TimeoutPredeadlineClockExit,
       TimeoutPredeadlineClockAtRank,
       TimeoutDeadlineArmedOwner,
       TimeoutRoundTrigger, TimeoutRoundStable,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncTransportTypeInvariant, AsyncTransportClockTypeInvariant

THEOREM AsyncTickStrictlyLowersPredeadlineClockRankOrExits ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     rank \in Nat:
    /\ TimeoutPredeadlineClockAtRank(source, sourceView, rank)
    /\ AsyncTick
    => \/ TimeoutPredeadlineClockExit(source, sourceView)'
       \/ \E lowerRank \in Nat:
            /\ lowerRank < rank
            /\ TimeoutPredeadlineClockAtRank(
                 source, sourceView, lowerRank)'
BY SMT
   DEF TimeoutPredeadlineClockAtRank,
       TimeoutPredeadlineClockExit,
       TimeoutDirectGoal, TimeoutDeadlineArmedOwner,
       TimeoutRoundTrigger, TimeoutRoundStable,
       TimeoutOrigin, AsyncTick, AsyncNonClockVars


\* The natural clock rank and the exact Tick descent are discharged above.
\* The remaining pre-deadline prefix is the fixed-clock blocker episode:
\* weak fairness of AsyncTick is insufficient while overdue packet,
\* node-service, or I/O-service owners disable it.  Its finite semantic owner
\* universe must be derived from immutable admission/lifecycle metadata; a
\* state-dependent CHOOSE owner or an unproved ordinal ceiling is not a
\* descent argument.

(***************************************************************************
Timeout-owned fixed-clock blocker corridor.

The tuple carrier below is shared with historical recovery, but its use here
does not assume `HistoricalRecoveryTarget` or a discovery endpoint.  The
three concrete blocker cohorts are exactly the global post-GST predicates
which disable `AsyncTick`: overdue transport, due timed runner service, and
due nonempty I/O service.  `TimeoutFixedClockBlockersAreExactlyTickBlockers`
records that equivalence under this timeout source predicate.

Only the last Candidate/Serve producer pair can replenish without changing
an earlier rank coordinate.  Its carrier is frozen from the selected packet's
immutable causal origin and exact Serve request identity.  Durable admission,
reservation, and tombstone records coalesce physical retries into those same
logical identities.  A discovered identity consumes the finite complement
budget; discovery itself is not called progress.  The timeout-specific
selected-owner property below must still reach either strict blocker-rank
descent or such a genuine discovery.
***************************************************************************)

TimeoutFixedClockPending(
    source, sourceView, clockValue, deadlineValue) ==
  /\ AsyncStrongTypeInvariant
  /\ TimeoutRoundTrigger(source, sourceView)
  /\ ~TimeoutPredeadlineClockExit(source, sourceView)
  /\ asyncNow = clockValue
  /\ asyncNodeDeadlines[source] = deadlineValue
  /\ clockValue < deadlineValue

TimeoutFixedClockProgressExit(
    source, sourceView, clockValue, deadlineValue) ==
  \/ TimeoutPredeadlineClockExit(source, sourceView)
  \/ /\ TimeoutRoundTrigger(source, sourceView)
     /\ asyncNodeDeadlines[source] = deadlineValue
     /\ clockValue < asyncNow
     /\ asyncNow < deadlineValue

TimeoutFixedClockBlockedAtRank(
    source, sourceView, clockValue, deadlineValue, rank) ==
  /\ TimeoutFixedClockPending(
       source, sourceView, clockValue, deadlineValue)
  /\ HistoricalDiscoveryConcreteFixedClockRank(clockValue) = rank

TimeoutFixedClockStrictRankGoal(
    source, sourceView, clockValue, deadlineValue, sourceRank) ==
  \/ TimeoutFixedClockProgressExit(
       source, sourceView, clockValue, deadlineValue)
  \/ \E lowerRank \in
       SetLessThan(
         sourceRank,
         HistoricalDiscoveryFixedClockBlockerOrdering,
         HistoricalDiscoveryFixedClockBlockerCarrier):
       TimeoutFixedClockBlockedAtRank(
         source, sourceView, clockValue, deadlineValue, lowerRank)

THEOREM TimeoutFixedClockBlockersAreExactlyTickBlockers ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat:
    TimeoutFixedClockPending(
      source, sourceView, clockValue, deadlineValue)
      => (AsyncTickEnabled
            <=> /\ OverdueResponsivePackets = {}
                /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
                /\ HistoricalDiscoveryActiveIoBlockersAt(clockValue) = {})
BY HistoricalDiscoveryFixedClockBlockerCharacterization
   DEF TimeoutFixedClockPending, TimeoutRoundTrigger

THEOREM TimeoutFixedClockConcreteRankInStructuralCarrier ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat:
    TimeoutFixedClockPending(
      source, sourceView, clockValue, deadlineValue)
      => HistoricalDiscoveryConcreteFixedClockRank(clockValue)
           \in HistoricalDiscoveryFixedClockBlockerCarrier
BY StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   HistoricalDiscoveryPacketDependencyRankInCarrier,
   HistoricalDiscoveryIngressCounterRankInCarrier,
   HistoricalDiscoveryFixedClockRankShapeInCarrier,
   FS_CardinalityType, Isa
   DEF TimeoutFixedClockPending,
       HistoricalDiscoveryConcreteFixedClockRank,
       HistoricalDiscoveryConcreteBlockerStage,
       HistoricalDiscoveryConcreteDependencyRank,
       HistoricalDiscoverySelectedOverduePacket,
       HistoricalDiscoverySelectedPacketDependencyRank,
       HistoricalDiscoveryNodeBlockerDebt,
       HistoricalDiscoveryActiveIoBlockerDebt,
       HistoricalDiscoveryBlockerStageCarrier,
       HistoricalDiscoveryLatentOwnerDebt,
       HistoricalDiscoveryDuePacketDebt,
       HistoricalDiscoveryDormantIoDebt

THEOREM TimeoutFixedClockPendingHasStructuralRank ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat:
    TimeoutFixedClockPending(
      source, sourceView, clockValue, deadlineValue)
      => \E rank \in HistoricalDiscoveryFixedClockBlockerCarrier:
           TimeoutFixedClockBlockedAtRank(
             source, sourceView, clockValue, deadlineValue, rank)
BY TimeoutFixedClockConcreteRankInStructuralCarrier
   DEF TimeoutFixedClockBlockedAtRank

TimeoutFixedPacketCandidateCarrier(packet) ==
  HistoricalDiscoveryPacketCandidateCausalOriginCarrier(packet)

TimeoutFixedPacketServeCarrier(packet) ==
  HistoricalDiscoveryPacketServeIdentityCarrier(packet)

TimeoutFixedPacketLiveCandidateIdentities(packet) ==
  {candidate.causalOrigin:
     candidate \in HistoricalDiscoveryPacketCandidateOwners(packet)}

TimeoutFixedPacketLiveServeIdentities(packet) ==
  {AsyncIoServeJobIdentity(packet.item.envelope.recipient, job):
     job \in HistoricalDiscoveryPacketServeOwners(packet)}

TimeoutFixedPacketLiveOwners(packet) ==
  ({"Candidate"} \X TimeoutFixedPacketLiveCandidateIdentities(packet))
    \cup
  ({"Serve"} \X TimeoutFixedPacketLiveServeIdentities(packet))

TimeoutFixedPacketCoveredCandidateIdentities(packet) ==
  TimeoutFixedPacketLiveCandidateIdentities(packet)
    \cup
  {record.identity.payload.causalOrigin:
     record \in AsyncCandidateServiceTombstones,
     record.identity.payload.causalOrigin
       \in TimeoutFixedPacketCandidateCarrier(packet)}
    \cup
  {record.origin:
     record \in AsyncCandidateLifecycleAdmissions,
     record.origin \in TimeoutFixedPacketCandidateCarrier(packet)}

TimeoutFixedPacketCoveredServeIdentities(packet) ==
  TimeoutFixedPacketLiveServeIdentities(packet)
    \cup
  {reservation.identity:
     reservation \in asyncServeReservations,
     reservation.identity \in TimeoutFixedPacketServeCarrier(packet)}
    \cup
  {tombstone.identity:
     tombstone \in asyncServeTombstones,
     tombstone.identity \in TimeoutFixedPacketServeCarrier(packet)}
    \cup
  UNION {
    {tombstone.identity:
       tombstone \in reservation.rollbackTombstones,
       tombstone.identity \in TimeoutFixedPacketServeCarrier(packet)}:
    reservation \in asyncServeReservations}

TimeoutFixedPacketCoveredOwners(packet) ==
  ({"Candidate"}
     \X TimeoutFixedPacketCoveredCandidateIdentities(packet))
    \cup
  ({"Serve"} \X TimeoutFixedPacketCoveredServeIdentities(packet))

THEOREM TimeoutFixedPacketFrozenCarriersAreFinite ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => /\ IsFiniteSet(TimeoutFixedPacketCandidateCarrier(packet))
         /\ IsFiniteSet(TimeoutFixedPacketServeCarrier(packet))
         /\ IsFiniteSet(
              AsyncTargetNeutralLifecycleOwnerCarrier(
                TimeoutFixedPacketCandidateCarrier(packet),
                TimeoutFixedPacketServeCarrier(packet)))
BY HistoricalDiscoveryPacketCausalCarriersAreFinite,
   AsyncTargetNeutralLifecycleOwnerCarrierIsFinite
   DEF TimeoutFixedPacketCandidateCarrier,
       TimeoutFixedPacketServeCarrier

THEOREM TimeoutFixedPacketLiveAndCoveredOwnersStayFrozen ==
  \A packet \in OverdueResponsivePackets:
    /\ TimeoutFixedPacketLiveOwners(packet)
         \subseteq
           AsyncTargetNeutralLifecycleOwnerCarrier(
             TimeoutFixedPacketCandidateCarrier(packet),
             TimeoutFixedPacketServeCarrier(packet))
    /\ TimeoutFixedPacketCoveredOwners(packet)
         \subseteq
           AsyncTargetNeutralLifecycleOwnerCarrier(
             TimeoutFixedPacketCandidateCarrier(packet),
             TimeoutFixedPacketServeCarrier(packet))
BY HistoricalDiscoveryPacketOwnersStayInFrozenCausalCarrier, Isa
   DEF TimeoutFixedPacketLiveOwners,
       TimeoutFixedPacketCoveredOwners,
       TimeoutFixedPacketLiveCandidateIdentities,
       TimeoutFixedPacketLiveServeIdentities,
       TimeoutFixedPacketCoveredCandidateIdentities,
       TimeoutFixedPacketCoveredServeIdentities,
       TimeoutFixedPacketCandidateCarrier,
       TimeoutFixedPacketServeCarrier,
       AsyncTargetNeutralLifecycleOwnerCarrier

TimeoutFixedClockDependencyProducerPrefix(dependencyRank) ==
  <<dependencyRank[1],
    dependencyRank[2][1],
    dependencyRank[2][2][1],
    dependencyRank[2][2][2][1],
    dependencyRank[2][2][2][2][1],
    dependencyRank[2][2][2][2][2][1],
    dependencyRank[2][2][2][2][2][2][1],
    dependencyRank[2][2][2][2][2][2][2][1]>>

TimeoutFixedClockProducerPrefix(rank) ==
  <<rank[1],
    rank[2][1],
    rank[2][2][1],
    rank[2][2][2][1],
    TimeoutFixedClockDependencyProducerPrefix(
      rank[2][2][2][2])>>

TimeoutFixedClockLifecycleEpisodeAtBudget(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget) ==
  LET currentRank ==
        HistoricalDiscoveryConcreteFixedClockRank(clockValue)
  IN /\ TimeoutFixedClockPending(
          source, sourceView, clockValue, deadlineValue)
     /\ sourceRank
          \in HistoricalDiscoveryFixedClockBlockerCarrier
     /\ currentRank
          \in HistoricalDiscoveryFixedClockBlockerCarrier
     /\ packet \in OverdueResponsivePackets
     /\ packet = HistoricalDiscoverySelectedOverduePacket
     /\ TimeoutFixedClockProducerPrefix(currentRank)
          = TimeoutFixedClockProducerPrefix(sourceRank)
     /\ ~TimeoutFixedClockStrictRankGoal(
          source, sourceView, clockValue, deadlineValue, sourceRank)
     /\ AsyncTargetNeutralLifecycleEpisodeAtBudget(
          TimeoutFixedPacketCandidateCarrier(packet),
          TimeoutFixedPacketServeCarrier(packet),
          TimeoutFixedPacketLiveOwners(packet),
          known, budget)

TimeoutFixedClockLifecycleDiscovery(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget) ==
  /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget)
  /\ AsyncTargetNeutralLifecycleDiscoveredOwnerSet(
       TimeoutFixedPacketLiveOwners(packet), known) # {}

TimeoutFixedClockPacketEpisodeSource(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget) ==
  /\ TimeoutFixedClockBlockedAtRank(
       source, sourceView, clockValue, deadlineValue, sourceRank)
  /\ OverdueResponsivePackets # {}
  /\ packet = HistoricalDiscoverySelectedOverduePacket
  /\ known = TimeoutFixedPacketCoveredOwners(packet)
  /\ budget =
       AsyncTargetNeutralLifecycleEpisodeBudget(
         TimeoutFixedPacketCandidateCarrier(packet),
         TimeoutFixedPacketServeCarrier(packet), known)
  /\ ~TimeoutFixedClockStrictRankGoal(
       source, sourceView, clockValue, deadlineValue, sourceRank)

THEOREM TimeoutFixedClockPacketSourceStartsNeutralEpisode ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known, budget:
      TimeoutFixedClockPacketEpisodeSource(
        source, sourceView, clockValue, deadlineValue,
        sourceRank, packet, known, budget)
        => /\ budget \in Nat
           /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, packet, known, budget)
BY TimeoutFixedPacketFrozenCarriersAreFinite,
   TimeoutFixedPacketLiveAndCoveredOwnersStayFrozen,
   AsyncTargetNeutralLifecycleEpisodeBudgetIsFiniteAndCoalesced,
   IsaT(240)
   DEF TimeoutFixedClockPacketEpisodeSource,
       TimeoutFixedClockLifecycleEpisodeAtBudget,
       TimeoutFixedClockBlockedAtRank,
       AsyncTargetNeutralLifecycleEpisodeAtBudget,
       AsyncTargetNeutralLifecycleKnownOwnerSet

THEOREM TimeoutFixedClockDiscoveryConsumesNeutralBudget ==
  \A source, sourceView, clockValue, deadlineValue,
     sourceRank, packet, known, budget:
    TimeoutFixedClockLifecycleDiscovery(
      source, sourceView, clockValue, deadlineValue,
      sourceRank, packet, known, budget)
      => \E known2:
           \E budget2 \in
                SetLessThan(
                  budget,
                  AsyncTargetNeutralLifecycleBudgetOrdering,
                  Nat):
             TimeoutFixedClockLifecycleEpisodeAtBudget(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, packet, known2, budget2)
BY AsyncTargetNeutralLifecycleDiscoveryStrictlyConsumesBudget,
   IsaT(240)
   DEF TimeoutFixedClockLifecycleDiscovery,
       TimeoutFixedClockLifecycleEpisodeAtBudget,
       AsyncTargetNeutralLifecycleKnownAdvanceGoal,
       AsyncTargetNeutralLifecycleBudgetOrdering,
       SetLessThan, OpToRel

TimeoutFixedClockLifecycleBudgetAtRank(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, budget) ==
  /\ budget \in Nat
  /\ \E packet, known:
       TimeoutFixedClockLifecycleEpisodeAtBudget(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, packet, known, budget)

TimeoutFixedClockLifecycleOwnerServiceProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
             \A budget \in Nat:
               TimeoutFixedClockLifecycleEpisodeAtBudget(
                 source, sourceView, clockValue, deadlineValue,
                 sourceRank, packet, known, budget)
                 ~> (TimeoutFixedClockStrictRankGoal(
                       source, sourceView, clockValue, deadlineValue,
                       sourceRank)
                      \/ TimeoutFixedClockLifecycleDiscovery(
                           source, sourceView, clockValue, deadlineValue,
                           sourceRank, packet, known, budget))

(***************************************************************************
Exact fixed-clock packet/lifecycle kernels.

The former lifecycle seam mixed three physically different owners.  The
selected overdue packet may still be below Candidate/Serve admission, or one
already-known logical Candidate or Serve lifecycle may be live.  A lifecycle
which is not in `known` is the finite-budget discovery outcome, not progress.
The properties below retain the selected packet, frozen rank prefix, logical
identity, known set, and budget.  They are the exact remaining temporal
kernels; the theorem following them only performs the disjoint-owner case
split and does not add fairness.
***************************************************************************)

TimeoutFixedClockLifecycleGoal(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget) ==
  \/ TimeoutFixedClockStrictRankGoal(
       source, sourceView, clockValue, deadlineValue, sourceRank)
  \/ TimeoutFixedClockLifecycleDiscovery(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget)

TimeoutFixedClockPacketDependencyKernelProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank, packet, known, budget)
             /\ TimeoutFixedPacketLiveOwners(packet) = {}
             ~> TimeoutFixedClockLifecycleGoal(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank, packet, known, budget)

TimeoutFixedClockCandidateLifecycleKernelProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known, identity:
           \A budget \in Nat:
             /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank, packet, known, budget)
             /\ <<"Candidate", identity>>
                  \in TimeoutFixedPacketLiveOwners(packet)
             /\ <<"Candidate", identity>> \in known
             ~> TimeoutFixedClockLifecycleGoal(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank, packet, known, budget)

TimeoutFixedClockServeLifecycleKernelProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known, identity:
           \A budget \in Nat:
             /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank, packet, known, budget)
             /\ <<"Serve", identity>>
                  \in TimeoutFixedPacketLiveOwners(packet)
             /\ <<"Serve", identity>> \in known
             ~> TimeoutFixedClockLifecycleGoal(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank, packet, known, budget)

TimeoutFixedClockLifecyclePhysicalKernelProperties(specification) ==
  /\ TimeoutFixedClockPacketDependencyKernelProperty(specification)
  /\ TimeoutFixedClockCandidateLifecycleKernelProperty(specification)
  /\ TimeoutFixedClockServeLifecycleKernelProperty(specification)

THEOREM AsyncSpecProvidesTimeoutFixedPacketCandidateFairness ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     packet, known, budget:
    /\ AsyncSpecAt(initialContext)
    /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, packet, known, budget)
    /\ HistoricalDiscoveryPacketCandidateOwners(packet) # {}
    => WF_AsyncAllVars(
         HistoricalDiscoveryPacketCandidateDebtFairAction(packet))
BY AsyncSpecAlwaysStrongTypeInvariant,
   HistoricalDiscoveryLiveCandidateDebtHasExactFairOwner,
   Isa, PTL
   DEF TimeoutFixedClockLifecycleEpisodeAtBudget,
       AsyncSpecAt, AsyncFairnessAt

THEOREM AsyncSpecProvidesTimeoutFixedPacketHistoricalServeFairness ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     packet, known, budget:
    /\ AsyncSpecAt(initialContext)
    /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, packet, known, budget)
    /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
    /\ HistoricalRecoveryTarget(packet.item.envelope.recipient)
    => WF_AsyncAllVars(
         PostGstServiceHistoricalRecoveryIoWorker(
           packet.item.envelope.recipient))
BY AsyncSpecAlwaysStrongTypeInvariant,
   HistoricalDiscoveryLiveServeDebtHasExactFairOwner,
   Isa, PTL
   DEF TimeoutFixedClockLifecycleEpisodeAtBudget,
       AsyncSpecAt, AsyncFairnessAt

THEOREM AsyncSpecProvidesTimeoutFixedPacketOrdinaryServeFairness ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     packet, known, budget:
    /\ AsyncSpecAt(initialContext)
    /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, packet, known, budget)
    /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
    /\ packet.item.envelope.recipient
         \in AsyncArchiveIoServiceNodes
    => WF_AsyncAllVars(
         PostGstServiceIoWorker(packet.item.envelope.recipient))
BY AsyncSpecAlwaysStrongTypeInvariant,
   HistoricalDiscoveryLiveServeDebtHasExactFairOwner,
   Isa, PTL
   DEF TimeoutFixedClockLifecycleEpisodeAtBudget,
       AsyncSpecAt, AsyncFairnessAt

THEOREM TimeoutFixedClockPhysicalKernelsDischargeLifecycleService ==
  \A specification:
    TimeoutFixedClockLifecyclePhysicalKernelProperties(specification)
      => TimeoutFixedClockLifecycleOwnerServiceProperty(specification)
PROOF
  <1>1. ASSUME NEW specification,
                TimeoutFixedClockLifecyclePhysicalKernelProperties(
                  specification)
         PROVE TimeoutFixedClockLifecycleOwnerServiceProperty(
                 specification)
    <2>1. CASE specification
      <3>1. ASSUME NEW source \in AsyncCurrentResponsiveVoters,
                    NEW sourceView \in Views,
                    NEW clockValue, NEW deadlineValue \in Nat,
                    NEW sourceRank \in
                      HistoricalDiscoveryFixedClockBlockerCarrier,
                    NEW packet, NEW known, NEW budget \in Nat
             PROVE TimeoutFixedClockLifecycleEpisodeAtBudget(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank, packet, known, budget)
                     ~> TimeoutFixedClockLifecycleGoal(
                          source, sourceView, clockValue, deadlineValue,
                          sourceRank, packet, known, budget)
        <4>1. TimeoutFixedClockLifecycleEpisodeAtBudget(
                 source, sourceView, clockValue, deadlineValue,
                 sourceRank, packet, known, budget)
                ~> \/ TimeoutFixedClockLifecycleGoal(
                       source, sourceView, clockValue, deadlineValue,
                       sourceRank, packet, known, budget)
                   \/ /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
                            source, sourceView, clockValue, deadlineValue,
                            sourceRank, packet, known, budget)
                      /\ TimeoutFixedPacketLiveOwners(packet) = {}
                   \/ \E identity:
                        /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
                             source, sourceView, clockValue, deadlineValue,
                             sourceRank, packet, known, budget)
                        /\ <<"Candidate", identity>>
                             \in TimeoutFixedPacketLiveOwners(packet)
                        /\ <<"Candidate", identity>> \in known
                   \/ \E identity:
                        /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
                             source, sourceView, clockValue, deadlineValue,
                             sourceRank, packet, known, budget)
                        /\ <<"Serve", identity>>
                             \in TimeoutFixedPacketLiveOwners(packet)
                        /\ <<"Serve", identity>> \in known
          BY Isa, PTL
             DEF TimeoutFixedClockLifecycleGoal,
                 TimeoutFixedClockLifecycleDiscovery,
                 AsyncTargetNeutralLifecycleDiscoveredOwnerSet,
                 TimeoutFixedPacketLiveOwners
        <4>2. (TimeoutFixedClockLifecycleEpisodeAtBudget(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank, packet, known, budget)
                 /\ TimeoutFixedPacketLiveOwners(packet) = {})
                ~> TimeoutFixedClockLifecycleGoal(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank, packet, known, budget)
          BY <1>1, <2>1
             DEF TimeoutFixedClockLifecyclePhysicalKernelProperties,
                 TimeoutFixedClockPacketDependencyKernelProperty
        <4>3. \A identity:
                 (TimeoutFixedClockLifecycleEpisodeAtBudget(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank, packet, known, budget)
                   /\ <<"Candidate", identity>>
                        \in TimeoutFixedPacketLiveOwners(packet)
                   /\ <<"Candidate", identity>> \in known)
                  ~> TimeoutFixedClockLifecycleGoal(
                       source, sourceView, clockValue, deadlineValue,
                       sourceRank, packet, known, budget)
          BY <1>1, <2>1
             DEF TimeoutFixedClockLifecyclePhysicalKernelProperties,
                 TimeoutFixedClockCandidateLifecycleKernelProperty
        <4>4. \A identity:
                 (TimeoutFixedClockLifecycleEpisodeAtBudget(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank, packet, known, budget)
                   /\ <<"Serve", identity>>
                        \in TimeoutFixedPacketLiveOwners(packet)
                   /\ <<"Serve", identity>> \in known)
                  ~> TimeoutFixedClockLifecycleGoal(
                       source, sourceView, clockValue, deadlineValue,
                       sourceRank, packet, known, budget)
          BY <1>1, <2>1
             DEF TimeoutFixedClockLifecyclePhysicalKernelProperties,
                 TimeoutFixedClockServeLifecycleKernelProperty
        <4> QED BY <4>1, <4>2, <4>3, <4>4, PTL
      <3> QED BY <3>1
    <2>2. CASE ~specification
      BY <2>2 DEF TimeoutFixedClockLifecycleOwnerServiceProperty
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1
       DEF TimeoutFixedClockLifecycleOwnerServiceProperty,
           TimeoutFixedClockLifecycleGoal

TimeoutFixedClockNonPacketServiceProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         (/\ TimeoutFixedClockBlockedAtRank(
                source, sourceView, clockValue, deadlineValue,
                sourceRank)
          /\ OverdueResponsivePackets = {})
           ~> TimeoutFixedClockStrictRankGoal(
                source, sourceView, clockValue, deadlineValue,
                sourceRank)

(***************************************************************************
Concrete non-packet closure.

This is the timeout specialization of the target-neutral clock service
kernel.  The owner mode and fair actions are shared with historical recovery,
but the pending predicate is not: a current-voter timeout owner need not be a
historical-recovery target.  The lemmas below therefore re-establish the
action edges against `TimeoutFixedClockPending` instead of importing the
historical endpoint as an assumption.
***************************************************************************)

TimeoutFixedClockDueNodeOwnerAtMode(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, owner, mode) ==
  /\ TimeoutFixedClockBlockedAtRank(
       source, sourceView, clockValue, deadlineValue, sourceRank)
  /\ OverdueResponsivePackets = {}
  /\ owner \in HistoricalDiscoveryNodeBlockersAt(clockValue)
  /\ mode \in HistoricalDiscoveryTimedOwnerModeCarrier
  /\ HistoricalDiscoveryTimedOwnerMode(owner) = mode

TimeoutFixedClockDueIoOwnerAtMode(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, owner, mode) ==
  /\ TimeoutFixedClockBlockedAtRank(
       source, sourceView, clockValue, deadlineValue, sourceRank)
  /\ OverdueResponsivePackets = {}
  /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
  /\ owner \in HistoricalDiscoveryActiveIoBlockersAt(clockValue)
  /\ mode \in HistoricalDiscoveryTimedOwnerModeCarrier
  /\ HistoricalDiscoveryTimedOwnerMode(owner) = mode

TimeoutFixedClockDueNodeModeGoal(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, owner, mode) ==
  \/ TimeoutFixedClockStrictRankGoal(
       source, sourceView, clockValue, deadlineValue, sourceRank)
  \/ \E lowerMode \in
       SetLessThan(
         mode,
         HistoricalDiscoveryTimedOwnerModeOrdering,
         HistoricalDiscoveryTimedOwnerModeCarrier):
       TimeoutFixedClockDueNodeOwnerAtMode(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, owner, lowerMode)

TimeoutFixedClockDueIoModeGoal(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, owner, mode) ==
  \/ TimeoutFixedClockStrictRankGoal(
       source, sourceView, clockValue, deadlineValue, sourceRank)
  \/ \E lowerMode \in
       SetLessThan(
         mode,
         HistoricalDiscoveryTimedOwnerModeOrdering,
         HistoricalDiscoveryTimedOwnerModeCarrier):
       TimeoutFixedClockDueIoOwnerAtMode(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, owner, lowerMode)

THEOREM TimeoutFixedClockConcreteLexCertificateStrictlyDescends ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat:
    /\ TimeoutFixedClockPending(
         source, sourceView, clockValue, deadlineValue)
    /\ TimeoutFixedClockPending(
         source, sourceView, clockValue, deadlineValue)'
    /\ HistoricalDiscoveryFixedClockLexStep(
         clockValue,
         HistoricalDiscoveryConcreteBlockerStage(clockValue),
         HistoricalDiscoveryConcreteDependencyRank(clockValue),
         HistoricalDiscoveryConcreteBlockerStage(clockValue)',
         HistoricalDiscoveryConcreteDependencyRank(clockValue)')
    => <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
          HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
         \in HistoricalDiscoveryFixedClockBlockerOrdering
BY HistoricalDiscoveryConcreteBlockerCoordinatesInCarrier,
   HistoricalDiscoveryFixedClockRankShapeInCarrier,
   HistoricalDiscoveryFixedClockLexStepStrictlyDescends,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts, Isa
   DEF TimeoutFixedClockPending,
       HistoricalDiscoveryConcreteFixedClockRank,
       HistoricalDiscoveryLatentOwnerDebt,
       HistoricalDiscoveryDuePacketDebt,
       HistoricalDiscoveryDormantIoDebt

THEOREM TimeoutFixedClockDueNodeModeHasEnabledFairAction ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ TimeoutFixedClockDueNodeOwnerAtMode(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, owner, mode)
      => ENABLED
           <<HistoricalDiscoveryDueNodeModeFairAction(
               owner, mode)>>_AsyncAllVars
BY GstResponsiveUnappliedRunNodeIsEnabled,
   GstHistoricalRecoveryRunNodeIsEnabled,
   GstHistoricalServerIsEnabled,
   ConcreteDueNodeServiceActionsResetDeadlineAboveFixedClock,
   HistoricalDiscoveryOwnersIncludeNonVoterService,
   AsyncStrongTypeProjectsAsyncType,
   ExpandENABLED, IsaT(300)
   DEF TimeoutFixedClockDueNodeOwnerAtMode,
       HistoricalDiscoveryDueNodeModeFairAction,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockPending,
       HistoricalRecoveryTarget,
       HistoricalDiscoveryNodeBlockersAt,
       HistoricalDiscoveryDueNodeService,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncAllVars

THEOREM TimeoutFixedClockDueIoModeHasEnabledFairAction ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    TimeoutFixedClockDueIoOwnerAtMode(
      source, sourceView, clockValue, deadlineValue,
      sourceRank, owner, mode)
      => ENABLED
           <<HistoricalDiscoveryDueIoModeFairAction(
               owner, mode)>>_AsyncAllVars
BY QueuedIoEnablesPostGstService,
   GstHistoricalIoWorkerIsEnabled,
   ConcreteDueIoServiceActionsRemoveExactActiveBlocker,
   HistoricalDiscoveryOwnersIncludeNonVoterService,
   AsyncStrongTypeProjectsAsyncType,
   ExpandENABLED, IsaT(300)
   DEF TimeoutFixedClockDueIoOwnerAtMode,
       HistoricalDiscoveryDueIoModeFairAction,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockPending,
       HistoricalDiscoveryActiveIoBlockersAt,
       HistoricalDiscoveryDueIoQueue,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncAllVars

THEOREM TimeoutFixedClockDueNodeFairOccurrenceReachesRankGoal ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ TimeoutFixedClockDueNodeOwnerAtMode(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, owner, mode)
    /\ AsyncStrongTypeInvariant'
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryDueNodeModeFairAction(owner, mode)
    => TimeoutFixedClockStrictRankGoal(
         source, sourceView, clockValue, deadlineValue, sourceRank)'
BY TimeoutFixedClockConcreteLexCertificateStrictlyDescends,
   ConcreteDueNodeServiceActionsResetDeadlineAboveFixedClock,
   HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock,
   HistoricalLatentOwnerEntryStrictlyDecreasesDebt,
   HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame,
   HistoricalDiscoveryBroadcastControlHelpersHaveFixedClockFrame,
   HistoricalDiscoveryRetransmissionHelpersHaveFixedClockFrame,
   HistoricalDiscoveryDirectRequestPublicationHasFixedClockFrame,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, FS_CardinalityType, IsaT(1200)
   DEF TimeoutFixedClockDueNodeOwnerAtMode,
       TimeoutFixedClockStrictRankGoal,
       TimeoutFixedClockProgressExit,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockPending,
       HistoricalDiscoveryDueNodeModeFairAction,
       HistoricalDiscoveryFixedClockLexStep,
       HistoricalDiscoveryFixedClockOuterPrefixEqual,
       HistoricalDiscoveryFixedClockPublicationFrame,
       HistoricalDiscoveryNodeServiceOutcome,
       HistoricalDiscoveryDormantGateHandoff,
       HistoricalDiscoveryNewTransportPacketsAreFuture,
       PostGstRunNode, PostGstRunHistoricalRecoveryNode,
       PostGstRunHistoricalServer, AsyncAllVars

THEOREM TimeoutFixedClockDueIoFairOccurrenceReachesRankGoal ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ TimeoutFixedClockDueIoOwnerAtMode(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, owner, mode)
    /\ AsyncStrongTypeInvariant'
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryDueIoModeFairAction(owner, mode)
    => TimeoutFixedClockStrictRankGoal(
         source, sourceView, clockValue, deadlineValue, sourceRank)'
BY TimeoutFixedClockConcreteLexCertificateStrictlyDescends,
   ConcreteDueIoServiceActionsRemoveExactActiveBlocker,
   HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock,
   HistoricalLatentOwnerEntryStrictlyDecreasesDebt,
   HistoricalDiscoveryResponsePublicationHasFixedClockFrame,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, FS_CardinalityType, IsaT(1200)
   DEF TimeoutFixedClockDueIoOwnerAtMode,
       TimeoutFixedClockStrictRankGoal,
       TimeoutFixedClockProgressExit,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockPending,
       HistoricalDiscoveryDueIoModeFairAction,
       HistoricalDiscoveryFixedClockLexStep,
       HistoricalDiscoveryFixedClockOuterPrefixEqual,
       HistoricalDiscoveryFixedClockPublicationFrame,
       HistoricalDiscoveryDueIoQueueServiceOutcome,
       HistoricalDiscoveryNewTransportPacketsAreFuture,
       PostGstServiceIoWorker,
       PostGstServiceHistoricalRecoveryIoWorker, AsyncAllVars

THEOREM TimeoutFixedClockDueNodeModeStepPreservesOrProgresses ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ TimeoutFixedClockDueNodeOwnerAtMode(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, owner, mode)
    /\ AsyncStrongTypeInvariant'
    /\ [AsyncNext]_AsyncAllVars
    => \/ TimeoutFixedClockDueNodeModeGoal(
            source, sourceView, clockValue, deadlineValue,
            sourceRank, owner, mode)'
       \/ TimeoutFixedClockDueNodeOwnerAtMode(
            source, sourceView, clockValue, deadlineValue,
            sourceRank, owner, mode)'
BY HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   TimeoutFixedClockConcreteLexCertificateStrictlyDescends,
   HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket,
   HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock,
   HistoricalLatentOwnerEntryStrictlyDecreasesDebt,
   ConcreteDueNodeServiceActionsResetDeadlineAboveFixedClock,
   ConcreteDueIoServiceActionsRemoveExactActiveBlocker,
   HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame,
   HistoricalDiscoveryBroadcastControlHelpersHaveFixedClockFrame,
   HistoricalDiscoveryRetransmissionHelpersHaveFixedClockFrame,
   HistoricalDiscoveryDirectRequestPublicationHasFixedClockFrame,
   HistoricalDiscoveryResponsePublicationHasFixedClockFrame,
   HistoricalDiscoveryByzantineCertifiedRequestHasFixedClockFrame,
   HistoricalDiscoverySingletonFaultInjectorsHaveFixedClockFrame,
   AsyncBracketNextPreservesStrongTypeInvariant,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, FS_CardinalityType, IsaT(1800)
   DEF TimeoutFixedClockDueNodeOwnerAtMode,
       TimeoutFixedClockDueNodeModeGoal,
       TimeoutFixedClockStrictRankGoal,
       TimeoutFixedClockProgressExit,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockPending,
       HistoricalDiscoveryTimedOwnerModeOrdering,
       HistoricalDiscoveryFixedClockLexStep,
       HistoricalDiscoveryFixedClockPublicationFrame,
       HistoricalDiscoveryDormantGateHandoff,
       HistoricalDiscoveryNodeServiceOutcome,
       HistoricalDiscoveryDueIoQueueServiceOutcome,
       HistoricalDiscoveryNewTransportPacketsAreFuture,
       SetLessThan, OpToRel,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep, AsyncAllVars

THEOREM TimeoutFixedClockDueIoModeStepPreservesOrProgresses ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ TimeoutFixedClockDueIoOwnerAtMode(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, owner, mode)
    /\ AsyncStrongTypeInvariant'
    /\ [AsyncNext]_AsyncAllVars
    => \/ TimeoutFixedClockDueIoModeGoal(
            source, sourceView, clockValue, deadlineValue,
            sourceRank, owner, mode)'
       \/ TimeoutFixedClockDueIoOwnerAtMode(
            source, sourceView, clockValue, deadlineValue,
            sourceRank, owner, mode)'
BY HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   TimeoutFixedClockConcreteLexCertificateStrictlyDescends,
   HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket,
   HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock,
   HistoricalLatentOwnerEntryStrictlyDecreasesDebt,
   ConcreteDueNodeServiceActionsResetDeadlineAboveFixedClock,
   ConcreteDueIoServiceActionsRemoveExactActiveBlocker,
   HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame,
   HistoricalDiscoveryBroadcastControlHelpersHaveFixedClockFrame,
   HistoricalDiscoveryRetransmissionHelpersHaveFixedClockFrame,
   HistoricalDiscoveryDirectRequestPublicationHasFixedClockFrame,
   HistoricalDiscoveryResponsePublicationHasFixedClockFrame,
   HistoricalDiscoveryByzantineCertifiedRequestHasFixedClockFrame,
   HistoricalDiscoverySingletonFaultInjectorsHaveFixedClockFrame,
   AsyncBracketNextPreservesStrongTypeInvariant,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, FS_CardinalityType, IsaT(1800)
   DEF TimeoutFixedClockDueIoOwnerAtMode,
       TimeoutFixedClockDueIoModeGoal,
       TimeoutFixedClockStrictRankGoal,
       TimeoutFixedClockProgressExit,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockPending,
       HistoricalDiscoveryTimedOwnerModeOrdering,
       HistoricalDiscoveryFixedClockLexStep,
       HistoricalDiscoveryFixedClockPublicationFrame,
       HistoricalDiscoveryDormantGateHandoff,
       HistoricalDiscoveryNodeServiceOutcome,
       HistoricalDiscoveryDueIoQueueServiceOutcome,
       HistoricalDiscoveryNewTransportPacketsAreFuture,
       SetLessThan, OpToRel,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep, AsyncAllVars

THEOREM AsyncSpecProvidesTimeoutDueNodeModeFairness ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ AsyncSpecAt(initialContext)
    /\ TimeoutFixedClockDueNodeOwnerAtMode(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, owner, mode)
    => WF_AsyncAllVars(
         HistoricalDiscoveryDueNodeModeFairAction(owner, mode))
BY AsyncSpecAlwaysUsesFixedResponsiveVoters, PTL, Isa
   DEF TimeoutFixedClockDueNodeOwnerAtMode,
       HistoricalDiscoveryDueNodeModeFairAction,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockPending,
       HistoricalDiscoveryNodeBlockersAt,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncSpecAt, AsyncFairnessAt

THEOREM AsyncSpecProvidesTimeoutDueIoModeFairness ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ AsyncSpecAt(initialContext)
    /\ TimeoutFixedClockDueIoOwnerAtMode(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, owner, mode)
    => WF_AsyncAllVars(
         HistoricalDiscoveryDueIoModeFairAction(owner, mode))
BY AsyncSpecAlwaysUsesFixedResponsiveVoters, PTL, Isa
   DEF TimeoutFixedClockDueIoOwnerAtMode,
       HistoricalDiscoveryDueIoModeFairAction,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockPending,
       HistoricalDiscoveryActiveIoBlockersAt,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncSpecAt, AsyncFairnessAt

THEOREM AsyncSpecTimeoutDueNodeModeMakesProgress ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    AsyncSpecAt(initialContext)
      => (TimeoutFixedClockDueNodeOwnerAtMode(
            source, sourceView, clockValue, deadlineValue,
            sourceRank, owner, mode)
           ~> TimeoutFixedClockDueNodeModeGoal(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, owner, mode))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW source \in AsyncCurrentResponsiveVoters,
                NEW sourceView \in Views,
                NEW clockValue, NEW deadlineValue \in Nat,
                NEW sourceRank \in
                  HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                NEW mode \in HistoricalDiscoveryTimedOwnerModeCarrier,
                AsyncSpecAt(initialContext)
         PROVE TimeoutFixedClockDueNodeOwnerAtMode(
                 source, sourceView, clockValue, deadlineValue,
                 sourceRank, owner, mode)
                 ~> TimeoutFixedClockDueNodeModeGoal(
                      source, sourceView, clockValue, deadlineValue,
                      sourceRank, owner, mode)
    <2>1. [](/\ AsyncStrongTypeInvariant
              /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
              /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
         AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity, PTL
    <2>2. [](TimeoutFixedClockDueNodeOwnerAtMode(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, owner, mode)
              /\ ~TimeoutFixedClockDueNodeModeGoal(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, owner, mode)
             => ENABLED
                  <<HistoricalDiscoveryDueNodeModeFairAction(
                      owner, mode)>>_AsyncAllVars)
      BY <2>1, TimeoutFixedClockDueNodeModeHasEnabledFairAction,
         PTL
    <2>3. [](TimeoutFixedClockDueNodeOwnerAtMode(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, owner, mode)
              /\ ~TimeoutFixedClockDueNodeModeGoal(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, owner, mode)
              /\ <<HistoricalDiscoveryDueNodeModeFairAction(
                       owner, mode)>>_AsyncAllVars
             => TimeoutFixedClockDueNodeModeGoal(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank, owner, mode)')
      BY <2>1,
         TimeoutFixedClockDueNodeFairOccurrenceReachesRankGoal,
         PTL
         DEF TimeoutFixedClockDueNodeModeGoal
    <2>4. [](TimeoutFixedClockDueNodeOwnerAtMode(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, owner, mode)
              /\ ~TimeoutFixedClockDueNodeModeGoal(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, owner, mode)
              /\ [AsyncNext]_AsyncAllVars
             => \/ TimeoutFixedClockDueNodeModeGoal(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank, owner, mode)'
                \/ TimeoutFixedClockDueNodeOwnerAtMode(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank, owner, mode)')
      BY <2>1,
         TimeoutFixedClockDueNodeModeStepPreservesOrProgresses,
         PTL
    <2>5. WF_AsyncAllVars(
             HistoricalDiscoveryDueNodeModeFairAction(owner, mode))
      BY <1>1, AsyncSpecProvidesTimeoutDueNodeModeFairness
    <2>6. [][AsyncNext]_AsyncAllVars
      BY <1>1 DEF AsyncSpecAt
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM AsyncSpecTimeoutDueIoModeMakesProgress ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    AsyncSpecAt(initialContext)
      => (TimeoutFixedClockDueIoOwnerAtMode(
            source, sourceView, clockValue, deadlineValue,
            sourceRank, owner, mode)
           ~> TimeoutFixedClockDueIoModeGoal(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, owner, mode))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW source \in AsyncCurrentResponsiveVoters,
                NEW sourceView \in Views,
                NEW clockValue, NEW deadlineValue \in Nat,
                NEW sourceRank \in
                  HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                NEW mode \in HistoricalDiscoveryTimedOwnerModeCarrier,
                AsyncSpecAt(initialContext)
         PROVE TimeoutFixedClockDueIoOwnerAtMode(
                 source, sourceView, clockValue, deadlineValue,
                 sourceRank, owner, mode)
                 ~> TimeoutFixedClockDueIoModeGoal(
                      source, sourceView, clockValue, deadlineValue,
                      sourceRank, owner, mode)
    <2>1. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>2. [](TimeoutFixedClockDueIoOwnerAtMode(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, owner, mode)
              /\ ~TimeoutFixedClockDueIoModeGoal(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, owner, mode)
             => ENABLED
                  <<HistoricalDiscoveryDueIoModeFairAction(
                      owner, mode)>>_AsyncAllVars)
      BY <2>1, TimeoutFixedClockDueIoModeHasEnabledFairAction,
         PTL
    <2>3. [](TimeoutFixedClockDueIoOwnerAtMode(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, owner, mode)
              /\ ~TimeoutFixedClockDueIoModeGoal(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, owner, mode)
              /\ <<HistoricalDiscoveryDueIoModeFairAction(
                       owner, mode)>>_AsyncAllVars
             => TimeoutFixedClockDueIoModeGoal(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank, owner, mode)')
      BY <2>1,
         TimeoutFixedClockDueIoFairOccurrenceReachesRankGoal,
         PTL
         DEF TimeoutFixedClockDueIoModeGoal
    <2>4. [](TimeoutFixedClockDueIoOwnerAtMode(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, owner, mode)
              /\ ~TimeoutFixedClockDueIoModeGoal(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, owner, mode)
              /\ [AsyncNext]_AsyncAllVars
             => \/ TimeoutFixedClockDueIoModeGoal(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank, owner, mode)'
                \/ TimeoutFixedClockDueIoOwnerAtMode(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank, owner, mode)')
      BY <2>1,
         TimeoutFixedClockDueIoModeStepPreservesOrProgresses,
         PTL
    <2>5. WF_AsyncAllVars(
             HistoricalDiscoveryDueIoModeFairAction(owner, mode))
      BY <1>1, AsyncSpecProvidesTimeoutDueIoModeFairness
    <2>6. [][AsyncNext]_AsyncAllVars
      BY <1>1 DEF AsyncSpecAt
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM AsyncSpecTimeoutDueNodeOwnerReachesRankGoal ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner:
    AsyncSpecAt(initialContext)
      => ((/\ TimeoutFixedClockBlockedAtRank(
                source, sourceView, clockValue, deadlineValue,
                sourceRank)
            /\ OverdueResponsivePackets = {}
            /\ owner \in HistoricalDiscoveryNodeBlockersAt(clockValue))
           ~> TimeoutFixedClockStrictRankGoal(
                source, sourceView, clockValue, deadlineValue,
                sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW source \in AsyncCurrentResponsiveVoters,
                NEW sourceView \in Views,
                NEW clockValue, NEW deadlineValue \in Nat,
                NEW sourceRank \in
                  HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                AsyncSpecAt(initialContext)
         PROVE (/\ TimeoutFixedClockBlockedAtRank(
                       source, sourceView, clockValue, deadlineValue,
                       sourceRank)
                  /\ OverdueResponsivePackets = {}
                  /\ owner
                       \in HistoricalDiscoveryNodeBlockersAt(clockValue))
                 ~> TimeoutFixedClockStrictRankGoal(
                      source, sourceView, clockValue, deadlineValue,
                      sourceRank)
    <2>1. \A mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
             TimeoutFixedClockDueNodeOwnerAtMode(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, owner, mode)
               ~> TimeoutFixedClockDueNodeModeGoal(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank, owner, mode)
      BY <1>1, AsyncSpecTimeoutDueNodeModeMakesProgress
    <2>2. \A mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
             TimeoutFixedClockDueNodeOwnerAtMode(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, owner, mode)
               ~> TimeoutFixedClockStrictRankGoal(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank)
      BY <2>1,
         HistoricalDiscoveryTimedOwnerModeOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF TimeoutFixedClockDueNodeModeGoal
    <2>3. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>4. (/\ TimeoutFixedClockBlockedAtRank(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank)
             /\ OverdueResponsivePackets = {}
             /\ owner
                  \in HistoricalDiscoveryNodeBlockersAt(clockValue))
            ~> \E mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
                 TimeoutFixedClockDueNodeOwnerAtMode(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, owner, mode)
      BY <2>3, HistoricalDiscoveryTimedOwnerHasFiniteMode,
         PTL
         DEF TimeoutFixedClockDueNodeOwnerAtMode,
             HistoricalDiscoveryNodeBlockersAt
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

THEOREM AsyncSpecTimeoutDueIoOwnerReachesRankGoal ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner:
    AsyncSpecAt(initialContext)
      => ((/\ TimeoutFixedClockBlockedAtRank(
                source, sourceView, clockValue, deadlineValue,
                sourceRank)
            /\ OverdueResponsivePackets = {}
            /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
            /\ owner
                 \in HistoricalDiscoveryActiveIoBlockersAt(clockValue))
           ~> TimeoutFixedClockStrictRankGoal(
                source, sourceView, clockValue, deadlineValue,
                sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW source \in AsyncCurrentResponsiveVoters,
                NEW sourceView \in Views,
                NEW clockValue, NEW deadlineValue \in Nat,
                NEW sourceRank \in
                  HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                AsyncSpecAt(initialContext)
         PROVE (/\ TimeoutFixedClockBlockedAtRank(
                       source, sourceView, clockValue, deadlineValue,
                       sourceRank)
                  /\ OverdueResponsivePackets = {}
                  /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
                  /\ owner
                       \in HistoricalDiscoveryActiveIoBlockersAt(
                            clockValue))
                 ~> TimeoutFixedClockStrictRankGoal(
                      source, sourceView, clockValue, deadlineValue,
                      sourceRank)
    <2>1. \A mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
             TimeoutFixedClockDueIoOwnerAtMode(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, owner, mode)
               ~> TimeoutFixedClockDueIoModeGoal(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank, owner, mode)
      BY <1>1, AsyncSpecTimeoutDueIoModeMakesProgress
    <2>2. \A mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
             TimeoutFixedClockDueIoOwnerAtMode(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, owner, mode)
               ~> TimeoutFixedClockStrictRankGoal(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank)
      BY <2>1,
         HistoricalDiscoveryTimedOwnerModeOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF TimeoutFixedClockDueIoModeGoal
    <2>3. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>4. (/\ TimeoutFixedClockBlockedAtRank(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank)
             /\ OverdueResponsivePackets = {}
             /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
             /\ owner
                  \in HistoricalDiscoveryActiveIoBlockersAt(clockValue))
            ~> \E mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
                 TimeoutFixedClockDueIoOwnerAtMode(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, owner, mode)
      BY <2>3, HistoricalDiscoveryTimedOwnerHasFiniteMode,
         PTL
         DEF TimeoutFixedClockDueIoOwnerAtMode,
             HistoricalDiscoveryActiveIoBlockersAt
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

TimeoutFixedClockTickBlockedAtRank(
    source, sourceView, clockValue, deadlineValue, sourceRank) ==
  /\ TimeoutFixedClockBlockedAtRank(
       source, sourceView, clockValue, deadlineValue, sourceRank)
  /\ OverdueResponsivePackets = {}
  /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
  /\ HistoricalDiscoveryActiveIoBlockersAt(clockValue) = {}

THEOREM TimeoutFixedClockTickBlockedHasEnabledExactTick ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    TimeoutFixedClockTickBlockedAtRank(
      source, sourceView, clockValue, deadlineValue, sourceRank)
      => ENABLED <<AsyncTick>>_AsyncAllVars
BY TimeoutFixedClockBlockersAreExactlyTickBlockers,
   ExpandENABLED, Isa
   DEF TimeoutFixedClockTickBlockedAtRank,
       TimeoutFixedClockBlockedAtRank,
       AsyncTickEnabled, AsyncTick, AsyncAllVars

THEOREM TimeoutFixedClockExactTickReachesRankGoal ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    /\ TimeoutFixedClockTickBlockedAtRank(
         source, sourceView, clockValue, deadlineValue, sourceRank)
    /\ AsyncTick
    => TimeoutFixedClockStrictRankGoal(
         source, sourceView, clockValue, deadlineValue, sourceRank)'
BY SMT
   DEF TimeoutFixedClockTickBlockedAtRank,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockPending,
       TimeoutFixedClockStrictRankGoal,
       TimeoutFixedClockProgressExit,
       TimeoutPredeadlineClockExit,
       TimeoutDeadlineArmedOwner,
       TimeoutRoundTrigger, TimeoutRoundStable,
       AsyncTick, AsyncNonClockVars

THEOREM TimeoutFixedClockTickStepPreservesOrProgresses ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    /\ TimeoutFixedClockTickBlockedAtRank(
         source, sourceView, clockValue, deadlineValue, sourceRank)
    /\ AsyncStrongTypeInvariant'
    /\ [AsyncNext]_AsyncAllVars
    => \/ TimeoutFixedClockStrictRankGoal(
            source, sourceView, clockValue, deadlineValue, sourceRank)'
       \/ TimeoutFixedClockTickBlockedAtRank(
            source, sourceView, clockValue, deadlineValue, sourceRank)'
BY TimeoutFixedClockConcreteLexCertificateStrictlyDescends,
   HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket,
   HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock,
   HistoricalLatentOwnerEntryStrictlyDecreasesDebt,
   ConcreteDueNodeServiceActionsResetDeadlineAboveFixedClock,
   ConcreteDueIoServiceActionsRemoveExactActiveBlocker,
   HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame,
   HistoricalDiscoveryBroadcastControlHelpersHaveFixedClockFrame,
   HistoricalDiscoveryRetransmissionHelpersHaveFixedClockFrame,
   HistoricalDiscoveryDirectRequestPublicationHasFixedClockFrame,
   HistoricalDiscoveryResponsePublicationHasFixedClockFrame,
   HistoricalDiscoveryByzantineCertifiedRequestHasFixedClockFrame,
   HistoricalDiscoverySingletonFaultInjectorsHaveFixedClockFrame,
   AsyncBracketNextPreservesStrongTypeInvariant,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, FS_CardinalityType, IsaT(1800)
   DEF TimeoutFixedClockTickBlockedAtRank,
       TimeoutFixedClockStrictRankGoal,
       TimeoutFixedClockProgressExit,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockPending,
       HistoricalDiscoveryFixedClockLexStep,
       HistoricalDiscoveryFixedClockPublicationFrame,
       HistoricalDiscoveryDormantGateHandoff,
       HistoricalDiscoveryNodeServiceOutcome,
       HistoricalDiscoveryDueIoQueueServiceOutcome,
       HistoricalDiscoveryNewTransportPacketsAreFuture,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep, AsyncAllVars

THEOREM AsyncSpecTimeoutFixedClockTickReachesRankGoal ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    AsyncSpecAt(initialContext)
      => (TimeoutFixedClockTickBlockedAtRank(
            source, sourceView, clockValue, deadlineValue, sourceRank)
           ~> TimeoutFixedClockStrictRankGoal(
                source, sourceView, clockValue, deadlineValue,
                sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW source \in AsyncCurrentResponsiveVoters,
                NEW sourceView \in Views,
                NEW clockValue, NEW deadlineValue \in Nat,
                NEW sourceRank \in
                  HistoricalDiscoveryFixedClockBlockerCarrier,
                AsyncSpecAt(initialContext)
         PROVE TimeoutFixedClockTickBlockedAtRank(
                 source, sourceView, clockValue, deadlineValue,
                 sourceRank)
                 ~> TimeoutFixedClockStrictRankGoal(
                      source, sourceView, clockValue, deadlineValue,
                      sourceRank)
    <2>1. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>2. [](TimeoutFixedClockTickBlockedAtRank(
                source, sourceView, clockValue, deadlineValue,
                sourceRank)
              /\ ~TimeoutFixedClockStrictRankGoal(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank)
             => ENABLED <<AsyncTick>>_AsyncAllVars)
      BY TimeoutFixedClockTickBlockedHasEnabledExactTick, PTL
    <2>3. [](TimeoutFixedClockTickBlockedAtRank(
                source, sourceView, clockValue, deadlineValue,
                sourceRank)
              /\ ~TimeoutFixedClockStrictRankGoal(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank)
              /\ <<AsyncTick>>_AsyncAllVars
             => TimeoutFixedClockStrictRankGoal(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank)')
      BY TimeoutFixedClockExactTickReachesRankGoal, PTL
         DEF AsyncTick
    <2>4. [](TimeoutFixedClockTickBlockedAtRank(
                source, sourceView, clockValue, deadlineValue,
                sourceRank)
              /\ ~TimeoutFixedClockStrictRankGoal(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank)
              /\ [AsyncNext]_AsyncAllVars
             => \/ TimeoutFixedClockStrictRankGoal(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank)'
                \/ TimeoutFixedClockTickBlockedAtRank(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank)')
      BY <2>1, TimeoutFixedClockTickStepPreservesOrProgresses,
         PTL
    <2>5. WF_AsyncAllVars(AsyncTick)
      BY <1>1 DEF AsyncSpecAt, AsyncFairnessAt
    <2>6. [][AsyncNext]_AsyncAllVars
      BY <1>1 DEF AsyncSpecAt
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM AsyncSpecClosesTimeoutFixedClockNonPacketService ==
  \A initialContext:
    TimeoutFixedClockNonPacketServiceProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE TimeoutFixedClockNonPacketServiceProperty(
                 AsyncSpecAt(initialContext))
    <2>1. CASE AsyncSpecAt(initialContext)
      <3>1. ASSUME NEW source \in AsyncCurrentResponsiveVoters,
                    NEW sourceView \in Views,
                    NEW clockValue, NEW deadlineValue \in Nat,
                    NEW sourceRank \in
                      HistoricalDiscoveryFixedClockBlockerCarrier
             PROVE (/\ TimeoutFixedClockBlockedAtRank(
                           source, sourceView, clockValue,
                           deadlineValue, sourceRank)
                      /\ OverdueResponsivePackets = {})
                     ~> TimeoutFixedClockStrictRankGoal(
                          source, sourceView, clockValue,
                          deadlineValue, sourceRank)
        <4>1. \A owner:
                 (/\ TimeoutFixedClockBlockedAtRank(
                        source, sourceView, clockValue,
                        deadlineValue, sourceRank)
                  /\ OverdueResponsivePackets = {}
                  /\ owner
                       \in HistoricalDiscoveryNodeBlockersAt(clockValue))
                  ~> TimeoutFixedClockStrictRankGoal(
                       source, sourceView, clockValue,
                       deadlineValue, sourceRank)
          BY <2>1, AsyncSpecTimeoutDueNodeOwnerReachesRankGoal
        <4>2. \A owner:
                 (/\ TimeoutFixedClockBlockedAtRank(
                        source, sourceView, clockValue,
                        deadlineValue, sourceRank)
                  /\ OverdueResponsivePackets = {}
                  /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
                  /\ owner
                       \in HistoricalDiscoveryActiveIoBlockersAt(
                            clockValue))
                  ~> TimeoutFixedClockStrictRankGoal(
                       source, sourceView, clockValue,
                       deadlineValue, sourceRank)
          BY <2>1, AsyncSpecTimeoutDueIoOwnerReachesRankGoal
        <4>3. TimeoutFixedClockTickBlockedAtRank(
                 source, sourceView, clockValue, deadlineValue,
                 sourceRank)
                ~> TimeoutFixedClockStrictRankGoal(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank)
          BY <2>1, AsyncSpecTimeoutFixedClockTickReachesRankGoal
        <4>4. (/\ TimeoutFixedClockBlockedAtRank(
                       source, sourceView, clockValue,
                       deadlineValue, sourceRank)
                 /\ OverdueResponsivePackets = {})
                ~> (TimeoutFixedClockStrictRankGoal(
                       source, sourceView, clockValue,
                       deadlineValue, sourceRank)
                     \/ \E owner:
                          /\ TimeoutFixedClockBlockedAtRank(
                               source, sourceView, clockValue,
                               deadlineValue, sourceRank)
                          /\ OverdueResponsivePackets = {}
                          /\ owner
                               \in HistoricalDiscoveryNodeBlockersAt(
                                    clockValue)
                     \/ \E owner:
                          /\ TimeoutFixedClockBlockedAtRank(
                               source, sourceView, clockValue,
                               deadlineValue, sourceRank)
                          /\ OverdueResponsivePackets = {}
                          /\ HistoricalDiscoveryNodeBlockersAt(
                               clockValue) = {}
                          /\ owner
                               \in HistoricalDiscoveryActiveIoBlockersAt(
                                    clockValue)
                     \/ TimeoutFixedClockTickBlockedAtRank(
                          source, sourceView, clockValue,
                          deadlineValue, sourceRank))
          BY Isa, PTL
             DEF TimeoutFixedClockTickBlockedAtRank
        <4> QED BY <4>1, <4>2, <4>3, <4>4, PTL
      <3> QED BY <3>1
    <2>2. CASE ~AsyncSpecAt(initialContext)
      BY <2>2 DEF TimeoutFixedClockNonPacketServiceProperty
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncLiveClosesTimeoutFixedClockNonPacketService ==
  \A initialContext:
    TimeoutFixedClockNonPacketServiceProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecClosesTimeoutFixedClockNonPacketService,
   AsyncLiveSpecProjectsAsyncSpec, PTL
   DEF TimeoutFixedClockNonPacketServiceProperty

TimeoutFixedClockServicePrerequisites(specification) ==
  /\ TimeoutFixedClockNonPacketServiceProperty(specification)
  /\ TimeoutFixedClockLifecycleOwnerServiceProperty(specification)

THEOREM TimeoutLifecycleOwnerServiceSuppliesLiveFixedClockPrerequisites ==
  \A initialContext:
    TimeoutFixedClockLifecycleOwnerServiceProperty(
      AsyncLiveSpecAt(initialContext))
      => TimeoutFixedClockServicePrerequisites(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveClosesTimeoutFixedClockNonPacketService, Isa
   DEF TimeoutFixedClockServicePrerequisites

THEOREM TimeoutFixedClockLifecycleServiceLowersBudgetOrExits ==
  \A specification:
    TimeoutFixedClockLifecycleOwnerServiceProperty(specification)
      => (specification
            => \A source \in AsyncCurrentResponsiveVoters,
                  sourceView \in Views,
                  clockValue, deadlineValue \in Nat,
                  sourceRank \in
                    HistoricalDiscoveryFixedClockBlockerCarrier:
                 \A budget \in Nat:
                   TimeoutFixedClockLifecycleBudgetAtRank(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank, budget)
                     ~> (TimeoutFixedClockStrictRankGoal(
                           source, sourceView, clockValue,
                           deadlineValue, sourceRank)
                          \/ \E lowerBudget \in
                               SetLessThan(
                                 budget,
                                 AsyncTargetNeutralLifecycleBudgetOrdering,
                                 Nat):
                               TimeoutFixedClockLifecycleBudgetAtRank(
                                 source, sourceView, clockValue,
                                 deadlineValue, sourceRank,
                                 lowerBudget)))
BY TimeoutFixedClockDiscoveryConsumesNeutralBudget, PTL, Isa
   DEF TimeoutFixedClockLifecycleOwnerServiceProperty,
       TimeoutFixedClockLifecycleBudgetAtRank

THEOREM TimeoutFixedClockLifecycleServiceClosesFiniteEpisode ==
  \A specification:
    TimeoutFixedClockLifecycleOwnerServiceProperty(specification)
      => (specification
            => \A source \in AsyncCurrentResponsiveVoters,
                  sourceView \in Views,
                  clockValue, deadlineValue \in Nat,
                  sourceRank \in
                    HistoricalDiscoveryFixedClockBlockerCarrier,
                  budget \in Nat:
                 TimeoutFixedClockLifecycleBudgetAtRank(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, budget)
                   ~> TimeoutFixedClockStrictRankGoal(
                        source, sourceView, clockValue,
                        deadlineValue, sourceRank))
BY TimeoutFixedClockLifecycleServiceLowersBudgetOrExits,
   AsyncTargetNeutralLifecycleBudgetOrderingIsWellFounded,
   WellFoundedLeadsTo

THEOREM TimeoutFixedClockPacketServiceClosesFiniteEpisode ==
  \A specification:
    TimeoutFixedClockLifecycleOwnerServiceProperty(specification)
      => (specification
            => \A source \in AsyncCurrentResponsiveVoters,
                  sourceView \in Views,
                  clockValue, deadlineValue \in Nat,
                  sourceRank \in
                    HistoricalDiscoveryFixedClockBlockerCarrier:
                 (/\ TimeoutFixedClockBlockedAtRank(
                        source, sourceView, clockValue, deadlineValue,
                        sourceRank)
                  /\ OverdueResponsivePackets # {})
                   ~> TimeoutFixedClockStrictRankGoal(
                        source, sourceView, clockValue,
                        deadlineValue, sourceRank))
PROOF
  <1>1. ASSUME NEW specification,
                TimeoutFixedClockLifecycleOwnerServiceProperty(
                  specification)
         PROVE specification
                 => \A source \in AsyncCurrentResponsiveVoters,
                       sourceView \in Views,
                       clockValue, deadlineValue \in Nat,
                       sourceRank \in
                         HistoricalDiscoveryFixedClockBlockerCarrier:
                      (/\ TimeoutFixedClockBlockedAtRank(
                             source, sourceView, clockValue,
                             deadlineValue, sourceRank)
                       /\ OverdueResponsivePackets # {})
                        ~> TimeoutFixedClockStrictRankGoal(
                             source, sourceView, clockValue,
                             deadlineValue, sourceRank)
    <2>1. CASE specification
      <3>1. ASSUME NEW source \in AsyncCurrentResponsiveVoters,
                    NEW sourceView \in Views,
                    NEW clockValue, NEW deadlineValue \in Nat,
                    NEW sourceRank \in
                      HistoricalDiscoveryFixedClockBlockerCarrier
             PROVE (/\ TimeoutFixedClockBlockedAtRank(
                           source, sourceView, clockValue,
                           deadlineValue, sourceRank)
                      /\ OverdueResponsivePackets # {})
                     ~> TimeoutFixedClockStrictRankGoal(
                          source, sourceView, clockValue,
                          deadlineValue, sourceRank)
        <4>1. (/\ TimeoutFixedClockBlockedAtRank(
                       source, sourceView, clockValue,
                       deadlineValue, sourceRank)
                 /\ OverdueResponsivePackets # {}
                 /\ ~TimeoutFixedClockStrictRankGoal(
                      source, sourceView, clockValue,
                      deadlineValue, sourceRank))
                => \E budget \in Nat:
                     TimeoutFixedClockLifecycleBudgetAtRank(
                       source, sourceView, clockValue,
                       deadlineValue, sourceRank, budget)
          BY TimeoutFixedClockPacketSourceStartsNeutralEpisode, Isa
             DEF TimeoutFixedClockPacketEpisodeSource,
                 TimeoutFixedClockLifecycleBudgetAtRank,
                 HistoricalDiscoverySelectedOverduePacket,
                 OverdueResponsivePackets,
                 AsyncPacketOwnsClockDeadline,
                 AsyncStrongTypeInvariant,
                 AsyncTypeInvariant,
                 AsyncTransportTypeInvariant,
                 AsyncPacketContentTypeInvariant
        <4>2. \A budget \in Nat:
                 TimeoutFixedClockLifecycleBudgetAtRank(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, budget)
                   ~> TimeoutFixedClockStrictRankGoal(
                        source, sourceView, clockValue,
                        deadlineValue, sourceRank)
          BY <1>1, <2>1,
             TimeoutFixedClockLifecycleServiceClosesFiniteEpisode
        <4> QED BY <4>1, <4>2, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1
  <1> QED BY <1>1

TimeoutFixedClockRankDescentProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         TimeoutFixedClockBlockedAtRank(
           source, sourceView, clockValue, deadlineValue, sourceRank)
           ~> TimeoutFixedClockStrictRankGoal(
                source, sourceView, clockValue, deadlineValue,
                sourceRank)

THEOREM TimeoutFixedClockPrerequisitesCloseOneStructuralRankStep ==
  \A specification:
    TimeoutFixedClockServicePrerequisites(specification)
      => TimeoutFixedClockRankDescentProperty(specification)
BY TimeoutFixedClockPacketServiceClosesFiniteEpisode, PTL
   DEF TimeoutFixedClockServicePrerequisites,
       TimeoutFixedClockRankDescentProperty,
       TimeoutFixedClockNonPacketServiceProperty

TimeoutFixedClockClosureProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat:
         TimeoutFixedClockPending(
           source, sourceView, clockValue, deadlineValue)
           ~> TimeoutFixedClockProgressExit(
                source, sourceView, clockValue, deadlineValue)

THEOREM TimeoutFixedClockRankDescentClosesFixedClock ==
  \A specification:
    TimeoutFixedClockRankDescentProperty(specification)
      => TimeoutFixedClockClosureProperty(specification)
PROOF
  <1>1. ASSUME NEW specification,
                TimeoutFixedClockRankDescentProperty(specification)
         PROVE TimeoutFixedClockClosureProperty(specification)
    <2>1. CASE specification
      <3>1. ASSUME NEW source \in AsyncCurrentResponsiveVoters,
                    NEW sourceView \in Views,
                    NEW clockValue, NEW deadlineValue \in Nat
             PROVE TimeoutFixedClockPending(
                     source, sourceView, clockValue, deadlineValue)
                     ~>
                   TimeoutFixedClockProgressExit(
                     source, sourceView, clockValue, deadlineValue)
        <4>1. \A rank \in
                   HistoricalDiscoveryFixedClockBlockerCarrier:
                 TimeoutFixedClockBlockedAtRank(
                   source, sourceView, clockValue, deadlineValue, rank)
                   ~> (TimeoutFixedClockProgressExit(
                         source, sourceView, clockValue, deadlineValue)
                        \/ \E lowerRank \in
                             SetLessThan(
                               rank,
                               HistoricalDiscoveryFixedClockBlockerOrdering,
                               HistoricalDiscoveryFixedClockBlockerCarrier):
                             TimeoutFixedClockBlockedAtRank(
                               source, sourceView, clockValue,
                               deadlineValue, lowerRank))
          BY <1>1, <2>1
             DEF TimeoutFixedClockRankDescentProperty,
                 TimeoutFixedClockStrictRankGoal
        <4>2. \A rank \in
                   HistoricalDiscoveryFixedClockBlockerCarrier:
                 TimeoutFixedClockBlockedAtRank(
                   source, sourceView, clockValue, deadlineValue, rank)
                   ~> TimeoutFixedClockProgressExit(
                        source, sourceView, clockValue, deadlineValue)
          BY <4>1,
             HistoricalDiscoveryFixedClockBlockerOrderingIsWellFounded,
             WellFoundedLeadsTo
        <4>3. TimeoutFixedClockPending(
                 source, sourceView, clockValue, deadlineValue)
                => \E rank \in
                     HistoricalDiscoveryFixedClockBlockerCarrier:
                     TimeoutFixedClockBlockedAtRank(
                       source, sourceView, clockValue,
                       deadlineValue, rank)
          BY TimeoutFixedClockPendingHasStructuralRank
        <4> QED BY <4>2, <4>3, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1 DEF TimeoutFixedClockClosureProperty
  <1> QED BY <1>1

THEOREM TimeoutFixedClockPrerequisitesCloseFixedClock ==
  \A specification:
    TimeoutFixedClockServicePrerequisites(specification)
      => TimeoutFixedClockClosureProperty(specification)
BY TimeoutFixedClockPrerequisitesCloseOneStructuralRankStep,
   TimeoutFixedClockRankDescentClosesFixedClock

TimeoutPredeadlineClockRankStepProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          rank \in Nat:
         TimeoutPredeadlineClockAtRank(source, sourceView, rank)
           ~> (TimeoutPredeadlineClockExit(source, sourceView)
                \/ \E lowerRank \in Nat:
                     /\ lowerRank < rank
                     /\ TimeoutPredeadlineClockAtRank(
                          source, sourceView, lowerRank))

THEOREM TimeoutFixedClockProgressExitLowersPredeadlineRank ==
  \A source, sourceView,
     clockValue, deadlineValue, rank:
    /\ rank \in Nat
    /\ rank > 0
    /\ deadlineValue = clockValue + rank
    /\ TimeoutFixedClockProgressExit(
         source, sourceView, clockValue, deadlineValue)
    => \/ TimeoutPredeadlineClockExit(source, sourceView)
       \/ \E lowerRank \in Nat:
            /\ lowerRank < rank
            /\ TimeoutPredeadlineClockAtRank(
                 source, sourceView, lowerRank)
BY SMT
   DEF TimeoutFixedClockProgressExit,
       TimeoutPredeadlineClockAtRank

THEOREM TimeoutFixedClockServiceComposesWithPredeadlineRank ==
  \A initialContext:
    TimeoutFixedClockServicePrerequisites(
      AsyncLiveSpecAt(initialContext))
      => TimeoutPredeadlineClockRankStepProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                TimeoutFixedClockServicePrerequisites(
                  AsyncLiveSpecAt(initialContext))
         PROVE TimeoutPredeadlineClockRankStepProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. AsyncLiveSpecAt(initialContext)
             => []AsyncStrongTypeInvariant
      BY AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysStrongTypeInvariant
    <2>2. TimeoutFixedClockClosureProperty(
             AsyncLiveSpecAt(initialContext))
      BY <1>1, TimeoutFixedClockPrerequisitesCloseFixedClock
    <2>3. ASSUME NEW source \in AsyncCurrentResponsiveVoters,
                  NEW sourceView \in Views,
                  NEW rank \in Nat,
                  AsyncLiveSpecAt(initialContext)
           PROVE TimeoutPredeadlineClockAtRank(
                   source, sourceView, rank)
                   ~>
                 (TimeoutPredeadlineClockExit(source, sourceView)
                  \/ \E lowerRank \in Nat:
                       /\ lowerRank < rank
                       /\ TimeoutPredeadlineClockAtRank(
                            source, sourceView, lowerRank))
      <3>1. TimeoutPredeadlineClockAtRank(
               source, sourceView, rank)
              => /\ rank > 0
                 /\ \E clockValue, deadlineValue \in Nat:
                      /\ deadlineValue = clockValue + rank
                      /\ (\/ TimeoutPredeadlineClockExit(
                               source, sourceView)
                          \/ TimeoutFixedClockPending(
                               source, sourceView,
                               clockValue, deadlineValue))
        BY <2>1, <2>3, Isa, PTL
           DEF TimeoutPredeadlineClockAtRank,
               TimeoutFixedClockPending,
               TimeoutRoundTrigger,
               AsyncStrongTypeInvariant,
               AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant,
               AsyncRuntimeScalarTypeInvariant,
               AsyncTransportTypeInvariant,
               AsyncTransportClockTypeInvariant
      <3>2. \A clockValue, deadlineValue \in Nat:
               TimeoutFixedClockPending(
                 source, sourceView, clockValue, deadlineValue)
                 ~>
               TimeoutFixedClockProgressExit(
                 source, sourceView, clockValue, deadlineValue)
        BY <2>2, <2>3 DEF TimeoutFixedClockClosureProperty
      <3>3. \A clockValue, deadlineValue \in Nat:
               /\ deadlineValue = clockValue + rank
               /\ TimeoutFixedClockProgressExit(
                    source, sourceView, clockValue, deadlineValue)
              => (TimeoutPredeadlineClockExit(source, sourceView)
                   \/ \E lowerRank \in Nat:
                        /\ lowerRank < rank
                        /\ TimeoutPredeadlineClockAtRank(
                             source, sourceView, lowerRank))
        BY <3>1, TimeoutFixedClockProgressExitLowersPredeadlineRank
      <3> QED BY <3>1, <3>2, <3>3, PTL
    <2> QED BY <2>3
         DEF TimeoutPredeadlineClockRankStepProperty
  <1> QED BY <1>1

THEOREM TimeoutPredeadlineRankDescentClosesDeadlineClock ==
  \A initialContext:
    TimeoutFixedClockServicePrerequisites(
      AsyncLiveSpecAt(initialContext))
      => TimeoutDeadlineClockConvergenceProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                TimeoutFixedClockServicePrerequisites(
                  AsyncLiveSpecAt(initialContext))
         PROVE TimeoutDeadlineClockConvergenceProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. TimeoutPredeadlineClockRankStepProperty(
             AsyncLiveSpecAt(initialContext))
      BY <1>1, TimeoutFixedClockServiceComposesWithPredeadlineRank
    <2>2. AsyncLiveSpecAt(initialContext)
             => []AsyncStrongTypeInvariant
      BY AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysStrongTypeInvariant
    <2>3. ASSUME NEW source \in AsyncCurrentResponsiveVoters,
                  NEW sourceView \in Views,
                  AsyncLiveSpecAt(initialContext)
           PROVE TimeoutRoundTrigger(source, sourceView)
                   ~> TimeoutPredeadlineClockExit(
                        source, sourceView)
      <3>1. \A rank \in Nat:
               TimeoutPredeadlineClockAtRank(
                 source, sourceView, rank)
                 ~> TimeoutPredeadlineClockExit(
                      source, sourceView)
        BY <2>1, <2>3, NatLessThanWellFounded,
           WellFoundedLeadsTo
           DEF TimeoutPredeadlineClockRankStepProperty,
               SetLessThan, OpToRel
      <3>2. (/\ AsyncStrongTypeInvariant
              /\ TimeoutRoundTrigger(source, sourceView)
              /\ ~TimeoutPredeadlineClockExit(source, sourceView))
             => \E rank \in Nat:
                  TimeoutPredeadlineClockAtRank(
                    source, sourceView, rank)
        BY TimeoutPredeadlineClockSourceHasPositiveNaturalRank
      <3> QED BY <2>2, <3>1, <3>2, PTL
    <2> QED BY <2>3
         DEF TimeoutDeadlineClockConvergenceProperty,
             TimeoutPredeadlineClockExit
  <1> QED BY <1>1

(***************************************************************************
Exact timeout semantic continuation.

The armed-clock prefix and the already-created Core owner are different
obligations.  Once `BeginTimeout` has installed the exact WAL record, the
serialized Busy kernel gives that record one matching Completion candidate.
The candidate remains protected while the record remains live; executing it
installs the exact signature owner.  The same argument for the signature
candidate publishes the immutable vote to every current recipient.

These predicates retain the full vote record.  Matching only
`(source, sourceView)` would permit a changed highest-PrepareQC payload to be
substituted between WAL persistence and signing.
***************************************************************************)

TimeoutPendingWalOwner(source, sourceView, vote) ==
  /\ TimeoutVoteSemanticIdentity(source, sourceView, vote)
  /\ TimeoutWal(source, vote) \in pendingTimeout

TimeoutSigningOwner(source, sourceView, vote) ==
  /\ TimeoutVoteSemanticIdentity(source, sourceView, vote)
  /\ vote \in timeoutIntents
  /\ TimeoutSign(source, vote) \in signTimeouts

TimeoutPendingCandidateWitness(
    source, sourceView, vote, candidate) ==
  /\ gst
  /\ source \in AsyncCurrentResponsiveVoters
  /\ TimeoutPendingWalOwner(source, sourceView, vote)
  /\ candidate \in BusyCompletionCandidates(source)
  /\ candidate.kind = "PersistTimeout"
  /\ candidate.node = source
  /\ candidate.height = context.height
  /\ candidate.view = sourceView
  /\ candidate.subject = vote.highSubject

TimeoutSigningCandidateWitness(
    source, sourceView, vote, candidate) ==
  /\ gst
  /\ source \in AsyncCurrentResponsiveVoters
  /\ TimeoutSigningOwner(source, sourceView, vote)
  /\ candidate \in BusyCompletionCandidates(source)
  /\ candidate.kind = "SignTimeout"
  /\ candidate.node = source
  /\ candidate.height = context.height
  /\ candidate.view = sourceView
  /\ candidate.subject = vote.highSubject

TimeoutPendingContinuationGoal(
    source, sourceView, vote, recipient) ==
  \/ TimeoutSigningOwner(source, sourceView, vote)
  \/ TimeoutOriginOutcome(source, sourceView, vote, recipient)

THEOREM ExactPendingTimeoutOwnerHasMatchingBusyCandidate ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     vote \in TimeoutVoteRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ gst
    /\ TimeoutPendingWalOwner(source, sourceView, vote)
    => \E candidate \in AsyncCandidateSet:
         TimeoutPendingCandidateWitness(
           source, sourceView, vote, candidate)
BY BusyPhaseOwnerPartitionObligation, IsaT(180)
   DEF TimeoutPendingWalOwner,
       TimeoutPendingCandidateWitness,
       AsyncProgressOwnershipInvariant,
       SerializedBusyOwnershipInvariant,
       BusyCompletionWitnessInvariant,
       BusyCompletionCandidates, ActiveBusyCompletionCarrier,
       SerializedBusyOwners, Stage2BusyKernelInvariant,
       Stage2BusyPhaseSeparated, NodeIdle, PendingNodes,
       SigningNodes, AllPendingRequests, RequestNodeSet,
       RequestsUniqueByNode, InstallGenerationExhausted,
       AsyncCurrentResponsiveVoters, TimeoutWal, TimeoutSign

THEOREM ExactSigningTimeoutOwnerHasMatchingBusyCandidate ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     vote \in TimeoutVoteRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ gst
    /\ TimeoutSigningOwner(source, sourceView, vote)
    => \E candidate \in AsyncCandidateSet:
         TimeoutSigningCandidateWitness(
           source, sourceView, vote, candidate)
BY BusyPhaseOwnerPartitionObligation, IsaT(180)
   DEF TimeoutSigningOwner,
       TimeoutSigningCandidateWitness,
       AsyncProgressOwnershipInvariant,
       SerializedBusyOwnershipInvariant,
       BusyCompletionWitnessInvariant,
       BusyCompletionCandidates, ActiveBusyCompletionCarrier,
       SerializedBusyOwners, Stage2BusyKernelInvariant,
       Stage2BusyPhaseSeparated, NodeIdle, PendingNodes,
       SigningNodes, AllPendingRequests, RequestNodeSet,
       RequestsUniqueByNode, InstallGenerationExhausted,
       AsyncCurrentResponsiveVoters, TimeoutWal, TimeoutSign

(***************************************************************************
One-transition exact-owner retention.

Post-GST crashes are absent.  While the exact semantic goal is false, a
matching Busy candidate cannot be discarded: it is current-consumer
Completion work and `BusyCompletionCandidateDispatchableObligation` makes it
executable even while the reducer is Busy.  The only execution which removes
the WAL owner installs the matching signature owner; the only execution which
removes that signature owner publishes `TimeoutOutbox` for the same vote.
***************************************************************************)

THEOREM PendingTimeoutCandidatePersistsUntilSigningOrOutcome ==
  \A source, sourceView, vote, recipient, candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ TimeoutPendingCandidateWitness(
         source, sourceView, vote, candidate)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~TimeoutPendingContinuationGoal(
         source, sourceView, vote, recipient)'
    => TimeoutPendingCandidateWitness(
         source, sourceView, vote, candidate)'
BY BusyPhaseOwnerPartitionObligation,
   BusyCompletionExecutionDropsPhaseObligation,
   BusyCompletionCandidateDispatchableObligation,
   RuntimeSelectedCommandsAreTyped,
   ProgressCoreStutterAndCarrierGrowthRetainsBusyCandidates,
   ProgressCoreStutterKeepsBusyWitnessWhenCarried,
   GstResponsiveNodesAreUp, HeadTailProperties, IsaT(420)
   DEF TimeoutPendingCandidateWitness,
       TimeoutPendingWalOwner, TimeoutSigningOwner,
       TimeoutPendingContinuationGoal, TimeoutOriginOutcome,
       TimeoutSourceDominated, TimeoutViewGoal,
       DecisionPropagationFrontier, TimeoutDelivery,
       TimeoutReceipt, TimeoutVoteSemanticIdentity,
       Stage2BusyKernelInvariant, BusyCompletionCandidates,
       BusyCompletionExecution, ActiveBusyCompletionCarrier,
       AsyncProgressOwnershipInvariant,
       SerializedBusyOwnershipInvariant, SerializedBusyOwners,
       RequestsUniqueByNode, NodeIdle, PendingNodes, SigningNodes,
       AllPendingRequests, RequestNodeSet,
       CandidateConsumerCurrent, CommandDispatchable,
       CommandExecutionReady, RunNode, RunNodeWork,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep, DirectRetransmitStep,
       IdleRuntimeStep, RemoveNextNodeCommand,
       RemoveNextDeferredCommand, DeferCommand, DiscardCommand,
       ExecuteCommand, ExecuteRegularCommand,
       ExecuteSignTimeout, CompleteTimeoutSignature,
       PersistTimeout, AppendCausalSuccessors,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncAllVars

THEOREM SigningTimeoutCandidatePersistsUntilExactOutcome ==
  \A source, sourceView, vote, recipient, candidate:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ Stage2BusyKernelInvariant
    /\ recipient \in AsyncCurrentResponsiveVoters
    /\ TimeoutSigningCandidateWitness(
         source, sourceView, vote, candidate)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~TimeoutOriginOutcome(
         source, sourceView, vote, recipient)'
    => TimeoutSigningCandidateWitness(
         source, sourceView, vote, candidate)'
BY BusyPhaseOwnerPartitionObligation,
   BusyCompletionExecutionDropsPhaseObligation,
   BusyCompletionCandidateDispatchableObligation,
   RuntimeSelectedCommandsAreTyped,
   ProgressCoreStutterAndCarrierGrowthRetainsBusyCandidates,
   ProgressCoreStutterKeepsBusyWitnessWhenCarried,
   GstResponsiveNodesAreUp, HeadTailProperties, IsaT(420)
   DEF TimeoutSigningCandidateWitness,
       TimeoutSigningOwner, TimeoutOriginOutcome,
       TimeoutSourceDominated, TimeoutViewGoal,
       DecisionPropagationFrontier, TimeoutDelivery,
       TimeoutReceipt, TimeoutVoteSemanticIdentity,
       TimeoutVoteItem, ExactPacketOwns, ExactIngressOwns,
       ExactDeliveryCandidateOwns,
       Stage2BusyKernelInvariant, BusyCompletionCandidates,
       BusyCompletionExecution, ActiveBusyCompletionCarrier,
       AsyncProgressOwnershipInvariant,
       SerializedBusyOwnershipInvariant, SerializedBusyOwners,
       RequestsUniqueByNode, NodeIdle, PendingNodes, SigningNodes,
       AllPendingRequests, RequestNodeSet,
       CandidateConsumerCurrent, CommandDispatchable,
       CommandExecutionReady, RunNode, RunNodeWork,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DirectTimeoutStep, DirectRetransmitStep,
       IdleRuntimeStep, RemoveNextNodeCommand,
       RemoveNextDeferredCommand, DeferCommand, DiscardCommand,
       ExecuteCommand, ExecuteRegularCommand,
       ExecuteSignTimeout, CompleteTimeoutSignature,
       PublishControlItems, TimeoutOutbox,
       AppendCausalSuccessors, AsyncNetworkItem, TimeoutEnvelope,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncAllVars

TimeoutConcreteOriginContinuationProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          vote \in TimeoutVoteRecordSet,
          recipient \in AsyncCurrentResponsiveVoters:
         (/\ gst
          /\ TimeoutOrigin(source, sourceView, vote))
           ~> TimeoutOriginOutcome(
                source, sourceView, vote, recipient)

THEOREM AsyncLiveTimeoutConcreteOriginContinuation ==
  \A initialContext:
    ProtectedServiceFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => TimeoutConcreteOriginContinuationProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext))
         PROVE TimeoutConcreteOriginContinuationProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. AsyncLiveSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ AsyncProgressOwnershipInvariant
                    /\ Stage2BusyKernelInvariant)
      BY AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncSpecAlwaysStage2BusyKernelObligation, PTL
         DEF Stage2BusyKernelProperty
    <2>2. StarvationFreedomProperty(
             AsyncLiveSpecAt(initialContext))
      BY StarvationFreedomObligation, PTL
    <2>3. ASSUME NEW source \in AsyncCurrentResponsiveVoters,
                  NEW sourceView \in Views,
                  NEW vote \in TimeoutVoteRecordSet,
                  NEW recipient \in AsyncCurrentResponsiveVoters,
                  AsyncLiveSpecAt(initialContext)
           PROVE (/\ gst
                    /\ TimeoutOrigin(source, sourceView, vote))
                   ~> TimeoutOriginOutcome(
                        source, sourceView, vote, recipient)
      <3>0. gst => []gst
        BY <2>3, AsyncLiveSpecProjectsAsyncSpec,
           AsyncSpecKeepsGstOnceSet, PTL
      <3>1. (gst
               /\ TimeoutSigningOwner(source, sourceView, vote))
               ~> TimeoutOriginOutcome(
                    source, sourceView, vote, recipient)
        <4>1. (gst
                   /\ TimeoutSigningOwner(source, sourceView, vote))
                   ~> \E candidate \in AsyncCandidateSet:
                        TimeoutSigningCandidateWitness(
                          source, sourceView, vote, candidate)
          BY <2>1,
             ExactSigningTimeoutOwnerHasMatchingBusyCandidate, PTL
             DEF TimeoutSigningCandidateWitness
        <4>2. ASSUME NEW candidate \in AsyncCandidateSet
               PROVE TimeoutSigningCandidateWitness(
                       source, sourceView, vote, candidate)
                       ~> TimeoutOriginOutcome(
                            source, sourceView, vote, recipient)
          <5>1. (gst
                   /\ ResponsiveProtectedCandidateOwned(candidate))
                  ~> ~ResponsiveProtectedCandidateOwned(candidate)
            BY <2>2, <2>3 DEF StarvationFreedomProperty
          <5>2. [](TimeoutSigningCandidateWitness(
                       source, sourceView, vote, candidate)
                     => (/\ gst
                          /\ ResponsiveProtectedCandidateOwned(
                               candidate)))
            BY <2>1, <2>3, Isa, PTL
               DEF TimeoutSigningCandidateWitness,
                   BusyCompletionCandidates,
                   ActiveBusyCompletionCarrier,
                   ResponsiveProtectedCandidateOwned,
                   ProtectedCandidateOwned, ProtectedServiceCandidate,
                   CandidateScheduled, AsyncProgressOwnershipInvariant,
                   AsyncOutstandingCarrierInvariant,
                   AsyncLogicalCandidateOwnershipInvariant
          <5>3. [](TimeoutSigningCandidateWitness(
                       source, sourceView, vote, candidate)
                     /\ ~TimeoutOriginOutcome(
                          source, sourceView, vote, recipient)
                    => \/ TimeoutSigningCandidateWitness(
                            source, sourceView, vote, candidate)'
                       \/ TimeoutOriginOutcome(
                            source, sourceView, vote, recipient)')
            BY <2>1, <2>3,
               SigningTimeoutCandidatePersistsUntilExactOutcome, PTL
          <5> QED BY <5>1, <5>2, <5>3, PTL
        <4> QED BY <4>1, <4>2, PTL
      <3>2. (gst
               /\ TimeoutPendingWalOwner(source, sourceView, vote))
               ~> TimeoutOriginOutcome(
                    source, sourceView, vote, recipient)
        <4>1. (gst
                   /\ TimeoutPendingWalOwner(source, sourceView, vote))
                   ~> \E candidate \in AsyncCandidateSet:
                        TimeoutPendingCandidateWitness(
                          source, sourceView, vote, candidate)
          BY <2>1,
             ExactPendingTimeoutOwnerHasMatchingBusyCandidate, PTL
             DEF TimeoutPendingCandidateWitness
        <4>2. ASSUME NEW candidate \in AsyncCandidateSet
               PROVE TimeoutPendingCandidateWitness(
                       source, sourceView, vote, candidate)
                       ~> TimeoutPendingContinuationGoal(
                            source, sourceView, vote, recipient)
          <5>1. (gst
                   /\ ResponsiveProtectedCandidateOwned(candidate))
                  ~> ~ResponsiveProtectedCandidateOwned(candidate)
            BY <2>2, <2>3 DEF StarvationFreedomProperty
          <5>2. [](TimeoutPendingCandidateWitness(
                       source, sourceView, vote, candidate)
                     => (/\ gst
                          /\ ResponsiveProtectedCandidateOwned(
                               candidate)))
            BY <2>1, <2>3, Isa, PTL
               DEF TimeoutPendingCandidateWitness,
                   BusyCompletionCandidates,
                   ActiveBusyCompletionCarrier,
                   ResponsiveProtectedCandidateOwned,
                   ProtectedCandidateOwned, ProtectedServiceCandidate,
                   CandidateScheduled, AsyncProgressOwnershipInvariant,
                   AsyncOutstandingCarrierInvariant,
                   AsyncLogicalCandidateOwnershipInvariant
          <5>3. [](TimeoutPendingCandidateWitness(
                       source, sourceView, vote, candidate)
                     /\ ~TimeoutPendingContinuationGoal(
                          source, sourceView, vote, recipient)
                    => \/ TimeoutPendingCandidateWitness(
                            source, sourceView, vote, candidate)'
                       \/ TimeoutPendingContinuationGoal(
                            source, sourceView, vote, recipient)')
            BY <2>1, <2>3,
               PendingTimeoutCandidatePersistsUntilSigningOrOutcome,
               PTL
          <5> QED BY <5>1, <5>2, <5>3, PTL
        <4>3. (gst
                   /\ TimeoutPendingWalOwner(source, sourceView, vote))
                   ~> TimeoutPendingContinuationGoal(
                        source, sourceView, vote, recipient)
          BY <4>1, <4>2, PTL
        <4> QED BY <3>0, <3>1, <4>3, PTL
             DEF TimeoutPendingContinuationGoal
      <3>3. []((/\ gst
                 /\ TimeoutOrigin(source, sourceView, vote))
                => \/ TimeoutPendingWalOwner(
                        source, sourceView, vote)
                   \/ TimeoutSigningOwner(source, sourceView, vote)
                   \/ TimeoutOriginOutcome(
                        source, sourceView, vote, recipient))
        BY <2>1, <2>3, Isa, PTL
           DEF TimeoutOrigin, TimeoutPendingWalOwner,
               TimeoutSigningOwner, TimeoutOriginOutcome
      <3> QED BY <3>0, <3>1, <3>2, <3>3, PTL
    <2> QED BY <2>3
         DEF TimeoutConcreteOriginContinuationProperty
  <1> QED BY <1>1

(***************************************************************************
The pre-WAL runtime corridor must consume every older immutable lifecycle
ordinal before executing `DirectTimeoutStep` or `DeferredTimeoutStep`; the
theorem above begins only after that exact prefix has installed
`TimeoutWal(source, vote)`.  The scheduler and reducer/WAL portions are split
into separate exact kernels below.
***************************************************************************)

TimeoutDeferredRuntimeOwner(source, sourceView) ==
  /\ TimeoutRoundStable(source, sourceView)
  /\ ~NodeTimedOut(source, sourceView)
  /\ "TimeoutElapsed" \in asyncOutstandingTags[source]

THEOREM BeginTimeoutCreatesExactPendingWalOwner ==
  \A source, sourceView:
    /\ nodeView[source] = sourceView
    /\ BeginTimeout(source)
    => \E vote \in TimeoutVoteRecordSet:
         TimeoutPendingWalOwner(source, sourceView, vote)'
BY Isa
   DEF TimeoutPendingWalOwner, TimeoutVoteSemanticIdentity,
       BeginTimeout, BeginTimeoutReady,
       TimeoutRequestFor, LocalTimeoutVoteFor,
       TimeoutWal, TimeoutWalSet, TimeoutVoteRecordSet

THEOREM DirectTimeoutCreatesExactWalOrDeferredOwner ==
  \A source, sourceView:
    /\ TimeoutDeadlineArmedOwner(source, sourceView)
    /\ DirectTimeoutStep(source)
    => \/ \E vote \in TimeoutVoteRecordSet:
            TimeoutPendingWalOwner(source, sourceView, vote)'
       \/ TimeoutDeferredRuntimeOwner(source, sourceView)'
BY BeginTimeoutCreatesExactPendingWalOwner, Isa
   DEF TimeoutDeadlineArmedOwner,
       TimeoutDeferredRuntimeOwner,
       TimeoutRoundStable, DirectTimeoutStep,
       BeginTimeoutEnabled, NodeTimedOut

THEOREM DeferredTimeoutOwnerStepSelectsBeginTimeout ==
  \A source, sourceView:
    /\ TimeoutDeferredRuntimeOwner(source, sourceView)
    /\ DeferredTimeoutStep(source)
    => BeginTimeout(source)
BY Isa
   DEF TimeoutDeferredRuntimeOwner,
       TimeoutRoundStable, DeferredTimeoutStep,
       DeferredTimeoutExecutable, BeginTimeoutEnabled

THEOREM DeferredTimeoutOwnerCreatesExactPendingWalOwner ==
  \A source, sourceView:
    /\ TimeoutDeferredRuntimeOwner(source, sourceView)
    /\ DeferredTimeoutStep(source)
    => \E vote \in TimeoutVoteRecordSet:
         TimeoutPendingWalOwner(source, sourceView, vote)'
BY DeferredTimeoutOwnerStepSelectsBeginTimeout,
   BeginTimeoutCreatesExactPendingWalOwner, Isa
   DEF TimeoutDeferredRuntimeOwner,
       TimeoutRoundStable

TimeoutArmedRuntimePrefixProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views:
         TimeoutDeadlineArmedOwner(source, sourceView)
           ~> (TimeoutDirectGoal(source, sourceView)
                \/ \E vote \in TimeoutVoteRecordSet:
                     TimeoutOrigin(source, sourceView, vote))

TimeoutArmedExactWalEndpointProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views:
         TimeoutDeadlineArmedOwner(source, sourceView)
           ~> (TimeoutDirectGoal(source, sourceView)
                \/ \E vote \in TimeoutVoteRecordSet:
                     TimeoutPendingWalOwner(
                       source, sourceView, vote))

(***************************************************************************
Exact armed-Runtime and reducer/WAL kernels.

The clock owner does not itself authorize a WAL write.  It first reaches the
deterministic node runner, and a disabled direct BeginTimeout becomes the same
retained `TimeoutElapsed` owner.  The four kernels below preserve that
distinction.  The two action-owner predicates name the exact nonstuttering
RunNode occurrence; `DirectTimeoutCreatesExactWalOrDeferredOwner` and
`DeferredTimeoutOwnerCreatesExactPendingWalOwner` already prove the local
reducer endpoints once those occurrences execute.
***************************************************************************)

TimeoutExactWalEndpoint(source, sourceView) ==
  \E vote \in TimeoutVoteRecordSet:
    TimeoutPendingWalOwner(source, sourceView, vote)

TimeoutDirectRuntimeActionOwner(source, sourceView) ==
  /\ TimeoutDeadlineArmedOwner(source, sourceView)
  /\ ENABLED
       <<PostGstRunNode(source)
           /\ DirectTimeoutStep(source)>>_AsyncAllVars

TimeoutDeferredRuntimeActionOwner(source, sourceView) ==
  /\ TimeoutDeferredRuntimeOwner(source, sourceView)
  /\ ENABLED
       <<PostGstRunNode(source)
           /\ DeferredTimeoutStep(source)>>_AsyncAllVars

THEOREM TimeoutDirectRuntimeActionOwnerEnablesFairOccurrence ==
  \A source, sourceView:
    TimeoutDirectRuntimeActionOwner(source, sourceView)
      => ENABLED <<PostGstRunNode(source)>>_AsyncAllVars
BY ENABLEDaxioms
   DEF TimeoutDirectRuntimeActionOwner

THEOREM TimeoutDeferredRuntimeActionOwnerEnablesFairOccurrence ==
  \A source, sourceView:
    TimeoutDeferredRuntimeActionOwner(source, sourceView)
      => ENABLED <<PostGstRunNode(source)>>_AsyncAllVars
BY ENABLEDaxioms
   DEF TimeoutDeferredRuntimeActionOwner

THEOREM AsyncSpecProvidesTimeoutRuntimeRunNodeFairness ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views:
    /\ AsyncSpecAt(initialContext)
    /\ (TimeoutDeadlineArmedOwner(source, sourceView)
          \/ TimeoutDeferredRuntimeOwner(source, sourceView))
    => WF_AsyncAllVars(PostGstRunNode(source))
BY AsyncSpecAlwaysUsesFixedResponsiveVoters, Isa, PTL
   DEF TimeoutDeadlineArmedOwner,
       TimeoutDeferredRuntimeOwner,
       TimeoutRoundStable,
       AsyncSpecAt, AsyncFairnessAt

(***************************************************************************
Frozen timeout owner and finite Candidate predecessor prefix.

The timeout clock owns one immutable causal origin and lifecycle ordinal.
Only Candidate roots admitted below that ordinal may precede it.  Their
successors retain the same origin, and the closed command graph supplies a
radix-four remaining-work budget: one serviced parent has at most three
strictly lower-stage children.  Later Candidate, causal, Control, Completion,
and priority work therefore cannot enter this frozen prefix.

Serve ingress tickets are deliberately absent from this Candidate rank.  They
now share the actor-global scheduler ordinal but retain their own exact
ingress lifecycle rank.  Tickets no later than the timeout owner form the
separate frozen ingress prefix below; every later ticket is handled by the
single per-ticket Runtime interleaving seam.
***************************************************************************)

TimeoutRuntimeModeCarrier == {"Direct", "Deferred"}

TimeoutRuntimeModeOwner(mode, source, sourceView) ==
  CASE mode = "Direct" ->
         TimeoutDeadlineArmedOwner(source, sourceView)
    [] mode = "Deferred" ->
         TimeoutDeferredRuntimeOwner(source, sourceView)
    [] OTHER -> FALSE

TimeoutRuntimeModeActionOwner(mode, source, sourceView) ==
  CASE mode = "Direct" ->
         TimeoutDirectRuntimeActionOwner(source, sourceView)
    [] mode = "Deferred" ->
         TimeoutDeferredRuntimeActionOwner(source, sourceView)
    [] OTHER -> FALSE

TimeoutRuntimeModeEndpoint(mode, source, sourceView) ==
  \/ TimeoutDirectGoal(source, sourceView)
  \/ TimeoutExactWalEndpoint(source, sourceView)
  \/ /\ mode = "Direct"
     /\ TimeoutDeferredRuntimeOwner(source, sourceView)

TimeoutFrozenRuntimeLifecycleOwner(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal) ==
  /\ mode \in TimeoutRuntimeModeCarrier
  /\ TimeoutRuntimeModeOwner(mode, source, sourceView)
  /\ ownerContext = context
  /\ AsyncTimeoutLifecycleOwned(source)
  /\ ownerOrigin = AsyncTimeoutLifecycleOrigin(source)
  /\ ownerOrdinal = AsyncTimeoutLifecycleOrdinal(source)
  /\ ownerOrigin \in AsyncCandidateCausalOriginSet
  /\ ownerOrdinal \in Nat \ {0}
  /\ ownerOrigin.target = source
  /\ ownerOrigin.owner = source
  /\ ownerOrigin.context = ownerContext
  /\ ownerOrigin.view = sourceView
  /\ ownerOrigin.phase = "BeginTimeout"

TimeoutOlderLifecycleOriginsBelow(source, ownerOrdinal) ==
  {record.origin:
     record \in AsyncCandidateLifecycleAdmissions,
     /\ record.node = source
     /\ ~record.retired
     /\ record.ordinal < ownerOrdinal}

TimeoutFrozenRuntimeLifecycleSnapshot(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins) ==
  /\ TimeoutFrozenRuntimeLifecycleOwner(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal)
  /\ predecessorOrigins =
       TimeoutOlderLifecycleOriginsBelow(source, ownerOrdinal)

TimeoutFrozenRuntimeLifecyclePending(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins) ==
  /\ TimeoutFrozenRuntimeLifecycleOwner(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal)
  /\ TimeoutOlderLifecycleOriginsBelow(source, ownerOrdinal)
       \subseteq predecessorOrigins

TimeoutFrozenOlderLifecycleCandidates(
    source, ownerOrdinal, predecessorOrigins) ==
  {candidate \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates:
     /\ candidate.node = source
     /\ candidate.causalOrigin \in predecessorOrigins
     /\ AsyncCandidateLifecycleOrdinal(candidate) < ownerOrdinal}

TimeoutFrozenOlderCandidateWorkTokens(
    source, ownerOrdinal, predecessorOrigins) ==
  {<<candidate, token>>:
     candidate \in TimeoutFrozenOlderLifecycleCandidates(
                       source, ownerOrdinal, predecessorOrigins),
     token \in 1..AsyncCausalRemainingWorkWeight(candidate.kind)}

TimeoutFrozenOlderCandidateWorkBudget(
    source, ownerOrdinal, predecessorOrigins) ==
  Cardinality(
    TimeoutFrozenOlderCandidateWorkTokens(
      source, ownerOrdinal, predecessorOrigins))

TimeoutFrozenOlderCandidatePrefixGoal(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins) ==
  \/ TimeoutRuntimeModeEndpoint(mode, source, sourceView)
  \/ /\ TimeoutFrozenRuntimeLifecyclePending(
          mode, source, sourceView,
          ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins)
     /\ ~AsyncOlderCandidateLifecycleBlocksTimeout(source)

TimeoutFrozenOlderCandidatePrefixAtRank(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins, rank) ==
  /\ rank \in Nat
  /\ TimeoutFrozenRuntimeLifecyclePending(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins)
  /\ ~TimeoutRuntimeModeEndpoint(mode, source, sourceView)
  /\ TimeoutFrozenOlderCandidateWorkBudget(
       source, ownerOrdinal, predecessorOrigins) = rank

THEOREM TimeoutRuntimeModeOwnerHasExactFrozenLifecycleSnapshot ==
  \A mode \in TimeoutRuntimeModeCarrier,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ TimeoutRuntimeModeOwner(mode, source, sourceView)
    => \E ownerContext \in ContextRecords,
          ownerOrigin \in AsyncCandidateCausalOriginSet,
          ownerOrdinal \in Nat \ {0},
          predecessorOrigins:
         TimeoutFrozenRuntimeLifecycleSnapshot(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins)
BY IsaT(600)
   DEF TimeoutRuntimeModeCarrier,
       TimeoutRuntimeModeOwner,
       TimeoutFrozenRuntimeLifecycleSnapshot,
       TimeoutFrozenRuntimeLifecycleOwner,
       TimeoutOlderLifecycleOriginsBelow,
       TimeoutDeadlineArmedOwner,
       TimeoutDeferredRuntimeOwner,
       TimeoutRoundStable,
       AsyncTimeoutLifecycleOwned,
       AsyncTimeoutLifecycleOrdinal,
       AsyncTimeoutLifecycleOrigin,
       AsyncStrongTypeInvariant,
       AsyncControlServiceStateTypeInvariant

THEOREM TimeoutFrozenOlderCandidateWorkBudgetIsNatural ==
  \A mode \in TimeoutRuntimeModeCarrier,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     ownerContext \in ContextRecords,
     ownerOrigin \in AsyncCandidateCausalOriginSet,
     ownerOrdinal \in Nat \ {0},
     predecessorOrigins:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutFrozenRuntimeLifecyclePending(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins)
    => TimeoutFrozenOlderCandidateWorkBudget(
         source, ownerOrdinal, predecessorOrigins) \in Nat
BY AsyncCausalRemainingWorkWeightIsPositive,
   FS_Image, FS_Product, FS_Subset, FS_Interval,
   FS_CardinalityType, IsaT(300)
   DEF TimeoutFrozenOlderCandidateWorkBudget,
       TimeoutFrozenOlderCandidateWorkTokens,
       TimeoutFrozenOlderLifecycleCandidates,
       TimeoutFrozenRuntimeLifecyclePending,
       TimeoutOlderLifecycleOriginsBelow,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates

THEOREM TimeoutFrozenOlderBlockerHasExactCandidateWitness ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ TimeoutFrozenRuntimeLifecyclePending(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins)
    /\ AsyncOlderCandidateLifecycleBlocksTimeout(source)
    => TimeoutFrozenOlderLifecycleCandidates(
         source, ownerOrdinal, predecessorOrigins) # {}
BY IsaT(300)
   DEF TimeoutFrozenRuntimeLifecyclePending,
       TimeoutFrozenRuntimeLifecycleOwner,
       TimeoutOlderLifecycleOriginsBelow,
       TimeoutFrozenOlderLifecycleCandidates,
       AsyncOlderCandidateLifecycleBlocksTimeout,
       AsyncEffectiveTimeoutLifecycleOrdinal,
       AsyncCandidateLifecycleOrdinal,
       AsyncCandidateLifecycleRecordsFor,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant

THEOREM TimeoutFrozenOlderCandidateIsIndividuallyProtected ==
  \A mode \in TimeoutRuntimeModeCarrier,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins:
    \A candidate \in TimeoutFrozenOlderLifecycleCandidates(
                       source, ownerOrdinal, predecessorOrigins):
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ TimeoutFrozenRuntimeLifecyclePending(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins)
      => /\ gst
         /\ ResponsiveProtectedCandidateOwned(candidate)
BY IsaT(300)
   DEF TimeoutFrozenOlderLifecycleCandidates,
       TimeoutFrozenRuntimeLifecyclePending,
       TimeoutFrozenRuntimeLifecycleOwner,
       TimeoutRuntimeModeOwner,
       TimeoutDeadlineArmedOwner,
       TimeoutDeferredRuntimeOwner,
       TimeoutRoundStable,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateScheduled,
       AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant

(***************************************************************************
Action-local predecessor safety.

The first theorem excludes a later root from the frozen predecessor set.  The
second classifies a physical departure: an endpoint may be reached, or the
radix-four budget strictly decreases after replacing the serviced parent by
its bounded lower-stage successor batch.  No replacement is itself called
progress; only the strictly smaller remaining-work budget is.
***************************************************************************)

THEOREM TimeoutFrozenOlderPredecessorSetCannotBeReplenished ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ TimeoutFrozenRuntimeLifecyclePending(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~TimeoutRuntimeModeEndpoint(mode, source, sourceView)'
    => TimeoutOlderLifecycleOriginsBelow(source, ownerOrdinal)'
         \subseteq predecessorOrigins
BY AsyncTimeoutLifecycleOrdinalPersistsUntilEndpoint,
   AsyncTimeoutLifecycleOrdinalClearsOnlyAtEndpoint,
   AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   IsaT(900)
   DEF TimeoutFrozenRuntimeLifecyclePending,
       TimeoutFrozenRuntimeLifecycleOwner,
       TimeoutOlderLifecycleOriginsBelow,
       TimeoutRuntimeModeEndpoint,
       TimeoutRuntimeModeOwner,
       AsyncAllVars

THEOREM TimeoutFrozenOlderCandidateDepartureConsumesWorkBudget ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins,
     rank \in Nat:
    \A candidate \in TimeoutFrozenOlderLifecycleCandidates(
                       source, ownerOrdinal, predecessorOrigins):
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ TimeoutFrozenOlderCandidatePrefixAtRank(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal,
           predecessorOrigins, rank)
      /\ [AsyncNext]_AsyncAllVars
      /\ ~ResponsiveProtectedCandidateOwned(candidate)'
      => \/ TimeoutFrozenOlderCandidatePrefixGoal(
              mode, source, sourceView,
              ownerContext, ownerOrigin, ownerOrdinal,
              predecessorOrigins)'
         \/ \E lowerRank \in SetLessThan(rank, OpToRel(<, Nat), Nat):
              TimeoutFrozenOlderCandidatePrefixAtRank(
                mode, source, sourceView,
                ownerContext, ownerOrigin, ownerOrdinal,
                predecessorOrigins, lowerRank)'
BY TimeoutFrozenOlderPredecessorSetCannotBeReplenished,
   AsyncCommandSuccessorsStrictlyLowerRemainingWorkStage,
   AsyncCommandSuccessorBatchStrictlyConsumesRemainingWork,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   AsyncCausalRemainingWorkWeightIsPositive,
   IsaT(1200)
   DEF TimeoutFrozenOlderCandidatePrefixAtRank,
       TimeoutFrozenOlderCandidatePrefixGoal,
       TimeoutFrozenOlderCandidateWorkBudget,
       TimeoutFrozenOlderCandidateWorkTokens,
       TimeoutFrozenOlderLifecycleCandidates,
       TimeoutFrozenRuntimeLifecyclePending,
       SetLessThan, OpToRel, AsyncAllVars

THEOREM TimeoutFrozenOlderCandidateRankCellIsSafe ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins,
     rank \in Nat:
    \A candidate \in TimeoutFrozenOlderLifecycleCandidates(
                       source, ownerOrdinal, predecessorOrigins):
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ TimeoutFrozenOlderCandidatePrefixAtRank(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal,
           predecessorOrigins, rank)
      /\ ResponsiveProtectedCandidateOwned(candidate)
      /\ [AsyncNext]_AsyncAllVars
      => \/ TimeoutFrozenOlderCandidatePrefixGoal(
              mode, source, sourceView,
              ownerContext, ownerOrigin, ownerOrdinal,
              predecessorOrigins)'
         \/ \E lowerRank \in SetLessThan(rank, OpToRel(<, Nat), Nat):
              TimeoutFrozenOlderCandidatePrefixAtRank(
                mode, source, sourceView,
                ownerContext, ownerOrigin, ownerOrdinal,
                predecessorOrigins, lowerRank)'
         \/ /\ TimeoutFrozenOlderCandidatePrefixAtRank(
                  mode, source, sourceView,
                  ownerContext, ownerOrigin, ownerOrdinal,
                  predecessorOrigins, rank)'
            /\ ResponsiveProtectedCandidateOwned(candidate)'
BY TimeoutFrozenOlderPredecessorSetCannotBeReplenished,
   TimeoutFrozenOlderCandidateDepartureConsumesWorkBudget,
   AsyncCommandSuccessorBatchStrictlyConsumesRemainingWork,
   IsaT(1200)
   DEF TimeoutFrozenOlderCandidatePrefixAtRank,
       TimeoutFrozenOlderCandidatePrefixGoal,
       TimeoutFrozenOlderCandidateWorkBudget,
       TimeoutFrozenOlderCandidateWorkTokens,
       TimeoutFrozenOlderLifecycleCandidates,
       AsyncAllVars

TimeoutFrozenOlderCandidateRankStepProperty(specification) ==
  specification
    => \A mode \in TimeoutRuntimeModeCarrier,
          source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          ownerContext \in ContextRecords,
          ownerOrigin \in AsyncCandidateCausalOriginSet,
          ownerOrdinal \in Nat \ {0},
          predecessorOrigins,
          rank \in Nat:
         TimeoutFrozenOlderCandidatePrefixAtRank(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal,
           predecessorOrigins, rank)
           ~> (TimeoutFrozenOlderCandidatePrefixGoal(
                 mode, source, sourceView,
                 ownerContext, ownerOrigin, ownerOrdinal,
                 predecessorOrigins)
                \/ \E lowerRank \in
                     SetLessThan(rank, OpToRel(<, Nat), Nat):
                     TimeoutFrozenOlderCandidatePrefixAtRank(
                       mode, source, sourceView,
                       ownerContext, ownerOrigin, ownerOrdinal,
                       predecessorOrigins, lowerRank))

THEOREM AsyncLiveClosesTimeoutFrozenOlderCandidateRankStep ==
  \A initialContext:
    ProtectedServiceFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => TimeoutFrozenOlderCandidateRankStepProperty(
           AsyncLiveSpecAt(initialContext))
BY StarvationFreedomObligation,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   TimeoutFrozenOlderBlockerHasExactCandidateWitness,
   TimeoutFrozenOlderCandidateIsIndividuallyProtected,
   TimeoutFrozenOlderCandidateRankCellIsSafe,
   TimeoutFrozenOlderCandidateDepartureConsumesWorkBudget,
   PTL, IsaT(900)
   DEF TimeoutFrozenOlderCandidateRankStepProperty,
       TimeoutFrozenOlderCandidatePrefixAtRank,
       TimeoutFrozenOlderCandidatePrefixGoal,
       StarvationFreedomProperty,
       AsyncLiveSpecAt

TimeoutFrozenOlderCandidatePrefixClosureProperty(specification) ==
  specification
    => \A mode \in TimeoutRuntimeModeCarrier,
          source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          ownerContext \in ContextRecords,
          ownerOrigin \in AsyncCandidateCausalOriginSet,
          ownerOrdinal \in Nat \ {0},
          predecessorOrigins:
         TimeoutFrozenRuntimeLifecycleSnapshot(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal, predecessorOrigins)
           ~> TimeoutFrozenOlderCandidatePrefixGoal(
                 mode, source, sourceView,
                 ownerContext, ownerOrigin, ownerOrdinal,
                 predecessorOrigins)

THEOREM AsyncLiveClosesTimeoutFrozenOlderCandidatePrefix ==
  \A initialContext:
    ProtectedServiceFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => TimeoutFrozenOlderCandidatePrefixClosureProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveClosesTimeoutFrozenOlderCandidateRankStep,
   TimeoutFrozenOlderCandidateWorkBudgetIsNatural,
   NatLessThanWellFounded, WellFoundedLeadsTo, PTL
   DEF TimeoutFrozenOlderCandidatePrefixClosureProperty,
       TimeoutFrozenOlderCandidateRankStepProperty,
       TimeoutFrozenRuntimeLifecycleSnapshot,
       TimeoutFrozenRuntimeLifecyclePending,
       TimeoutFrozenOlderCandidatePrefixAtRank,
       TimeoutFrozenOlderCandidatePrefixGoal

(***************************************************************************
Frozen earlier Serve predecessors and the sole later-ticket residual.

A Serve ticket whose actor-global `schedulerOrdinal` is no later than the
frozen timeout ordinal is a legitimate predecessor.  Its named closure must
be discharged by the exact ingress lifecycle rank over the frozen ingress,
selector, lane, source, I/O, and runner components; cardinality alone is not
used as a temporal proof.  Retries coalesce and a retired identity cannot be
resurrected.  This is separate from the repaired later-ticket rule below and
is never inferred from eventual Serve absence.

Once the earlier prefix is empty, every currently owned Serve identity has a
strictly later scheduler ordinal.  At Runtime such a ticket must yield one
bounded older Runtime episode before its target-only ingress turn.  The
remaining Local/Ingress path is measured only by `RuntimeReachRank`; the
ticket itself is keyed by its immutable identity and scheduler ordinal.  An
unbounded sequence of distinct higher-view tickets is permitted because each
one interleaves the same monotone frozen-timeout episode.
***************************************************************************)

TimeoutFrozenEarlierServeTicketIdentities(source, ownerOrdinal) ==
  {identity \in AsyncServeIngressLifecycleOwnerIdentities(source):
     /\ AsyncServeIngressAdmissionOwned(source, identity)
     /\ AsyncServeIngressAdmissionSchedulerOrdinal(
          source, identity) <= ownerOrdinal}

TimeoutFrozenEarlierServeTicketBudget(source, ownerOrdinal) ==
  Cardinality(
    TimeoutFrozenEarlierServeTicketIdentities(source, ownerOrdinal))

TimeoutFrozenEarlierServePrefixPending(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal) ==
  /\ TimeoutFrozenRuntimeLifecycleOwner(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal)
  /\ ~TimeoutRuntimeModeEndpoint(mode, source, sourceView)
  /\ ~AsyncOlderCandidateLifecycleBlocksTimeout(source)

TimeoutFrozenEarlierServePrefixGoal(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal) ==
  \/ TimeoutRuntimeModeEndpoint(mode, source, sourceView)
  \/ /\ TimeoutFrozenEarlierServePrefixPending(
          mode, source, sourceView,
          ownerContext, ownerOrigin, ownerOrdinal)
     /\ TimeoutFrozenEarlierServeTicketIdentities(
          source, ownerOrdinal) = {}

TimeoutFrozenEarlierServePrefixClosureProperty(specification) ==
  specification
    => \A mode \in TimeoutRuntimeModeCarrier,
          source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          ownerContext \in ContextRecords,
          ownerOrigin \in AsyncCandidateCausalOriginSet,
          ownerOrdinal \in Nat \ {0}:
         TimeoutFrozenEarlierServePrefixPending(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal)
           ~> TimeoutFrozenEarlierServePrefixGoal(
                 mode, source, sourceView,
                 ownerContext, ownerOrigin, ownerOrdinal)

THEOREM TimeoutFrozenEarlierServeTicketBudgetIsNatural ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutFrozenEarlierServePrefixPending(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal)
    => TimeoutFrozenEarlierServeTicketBudget(
         source, ownerOrdinal) \in Nat
BY FS_Subset, FS_CardinalityType, IsaT(180)
   DEF TimeoutFrozenEarlierServeTicketBudget,
       TimeoutFrozenEarlierServeTicketIdentities,
       TimeoutFrozenEarlierServePrefixPending,
       TimeoutFrozenRuntimeLifecycleOwner,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant

(***************************************************************************
Generic exact-ingress rank for one frozen earlier Serve ticket.

This interface is intentionally parameterized only by the physical Serve
identity.  It therefore covers both `CertifiedRequest` and
`CommitCertificateRequest`; no Decision, body-holding, or application alias
is assumed.  The lifecycle rank contains the exact admission/tombstone stage,
the reservation's immutable I/O and ingress predecessors, every smaller
ingress owner/barrier, target capacity, selector priority, lane/source
position, and the normal-runner phase.  The episode budget is the cardinality
of those concrete owners, not the number of logical retries.
***************************************************************************)

TimeoutEarlierServeTicketLaneIndicesForSource(node, identity, laneSource) ==
  {index \in 1..Len(asyncIngressLanes[node][laneSource]):
     LET item == asyncIngressLanes[node][laneSource][index]
     IN /\ item.kind \in AsyncReplyRequestKinds
        /\ AsyncServeLogicalRequestIdentity(node, item) = identity}

TimeoutEarlierServeTicketSources(node, identity) ==
  {laneSource \in AsyncIngressSources:
     TimeoutEarlierServeTicketLaneIndicesForSource(
       node, identity, laneSource) # {}}

TimeoutEarlierServeTicketSource(node, identity) ==
  CHOOSE laneSource \in TimeoutEarlierServeTicketSources(node, identity):
    \A other \in TimeoutEarlierServeTicketSources(node, identity):
      IngressSourceServiceRank(node, laneSource)
        <= IngressSourceServiceRank(node, other)

TimeoutEarlierServeTicketLanePosition(node, identity) ==
  LET laneSource == TimeoutEarlierServeTicketSource(node, identity)
  IN CHOOSE least \in
       TimeoutEarlierServeTicketLaneIndicesForSource(
         node, identity, laneSource):
       \A other \in
         TimeoutEarlierServeTicketLaneIndicesForSource(
           node, identity, laneSource):
         least <= other

TimeoutEarlierServeTicketSourcePosition(node, identity) ==
  IngressSourceServiceRank(
    node, TimeoutEarlierServeTicketSource(node, identity))

TimeoutEarlierServeExactTicketResidual(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, identity) ==
  /\ TimeoutFrozenEarlierServePrefixPending(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal)
  /\ identity \in
       TimeoutFrozenEarlierServeTicketIdentities(source, ownerOrdinal)
  /\ TimeoutEarlierServeTicketSources(source, identity) # {}

TimeoutEarlierServeExactTicketGoal(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, identity) ==
  \/ TimeoutRuntimeModeEndpoint(mode, source, sourceView)
  \/ identity \notin
       TimeoutFrozenEarlierServeTicketIdentities(source, ownerOrdinal)

TimeoutEarlierServeLifecycleStage(source, identity) ==
  IF ~AsyncServeIngressAdmissionOwned(source, identity)
  THEN 0
  ELSE IF AsyncServeLifecycleTombstone(source, identity)
       THEN 1
       ELSE IF AsyncServeLiveReservationOwned(source, identity)
            THEN 2
            ELSE 3

TimeoutEarlierServeFrozenPredecessorSet(source, identity) ==
  ({"Io"} \X AsyncServeFrozenPredecessorSet(source, identity))
    \cup
  ({"Ingress"} \X
     AsyncServeIngressAdmissionPredecessorDebtSlots(source, identity))
    \cup
  AsyncServePreexistingIngressOwnerPredecessorDebtSet(source, identity)
    \cup
  AsyncServePreexistingIngressBarrierPredecessorDebtSet(source, identity)

TimeoutEarlierServeFrozenPredecessorDebt(source, identity) ==
  Cardinality(TimeoutEarlierServeFrozenPredecessorSet(source, identity))

TimeoutEarlierServeModeRank(source) ==
  IF NodeHasApplication(source) THEN 0 ELSE 1

TimeoutEarlierServeCapacityDebt(source) ==
  IF AsyncIoQueueDepth(source) < AsyncIoAuxCapacity
  THEN 0
  ELSE AsyncIoQueueDepth(source) - AsyncIoAuxCapacity + 1

TimeoutEarlierServeTargetCapacityDebt(source, identity) ==
  IF /\ AsyncServeLiveReservationOwned(source, identity)
     /\ ~AsyncServeJobQueued(source, identity)
  THEN TimeoutEarlierServeCapacityDebt(source)
  ELSE 0

TimeoutEarlierServePriorityOwners(source) ==
  {pair \in AsyncIngressSources \X (1..AsyncIngressCapacity):
     \/ pair[2] \in
          DrainableClaimedResponseLaneIndices(source, pair[1])
     \/ pair[2] \in
          DrainableRequestFencedCompletionLaneIndices(
            source, pair[1])}

TimeoutEarlierServePriorityDebt(source) ==
  Cardinality(TimeoutEarlierServePriorityOwners(source))

TimeoutEarlierServeReachRank(source) ==
  IF NodeHasApplication(source)
  THEN 0
  ELSE DrainableIngressTurnReachRank(source)

TimeoutEarlierServeLaneRank(source, identity) ==
  <<TimeoutEarlierServeTicketLanePosition(source, identity),
    TimeoutEarlierServeTicketSourcePosition(source, identity)>>

TimeoutEarlierServeSelectorRank(source, identity) ==
  <<TimeoutEarlierServePriorityDebt(source),
    TimeoutEarlierServeLaneRank(source, identity)>>

TimeoutEarlierServeReachSelectorRank(source, identity) ==
  <<TimeoutEarlierServeReachRank(source),
    TimeoutEarlierServeSelectorRank(source, identity)>>

TimeoutEarlierServeCapacityRank(source, identity) ==
  <<TimeoutEarlierServeTargetCapacityDebt(source, identity),
    TimeoutEarlierServeReachSelectorRank(source, identity)>>

TimeoutEarlierServeIngressRank(source, identity) ==
  <<TimeoutEarlierServeModeRank(source),
    TimeoutEarlierServeCapacityRank(source, identity)>>

TimeoutEarlierServeIngressLaneCarrier == Nat \X Nat
TimeoutEarlierServeIngressSelectorCarrier ==
  Nat \X TimeoutEarlierServeIngressLaneCarrier
TimeoutEarlierServeIngressReachSelectorCarrier ==
  Nat \X TimeoutEarlierServeIngressSelectorCarrier
TimeoutEarlierServeIngressCapacityCarrier ==
  Nat \X TimeoutEarlierServeIngressReachSelectorCarrier
TimeoutEarlierServeIngressRankCarrier ==
  (0..1) \X TimeoutEarlierServeIngressCapacityCarrier

TimeoutEarlierServeIngressLaneOrdering ==
  LexPairOrdering(OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)
TimeoutEarlierServeIngressSelectorOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), TimeoutEarlierServeIngressLaneOrdering,
    Nat, TimeoutEarlierServeIngressLaneCarrier)
TimeoutEarlierServeIngressReachSelectorOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), TimeoutEarlierServeIngressSelectorOrdering,
    Nat, TimeoutEarlierServeIngressSelectorCarrier)
TimeoutEarlierServeIngressCapacityOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), TimeoutEarlierServeIngressReachSelectorOrdering,
    Nat, TimeoutEarlierServeIngressReachSelectorCarrier)
TimeoutEarlierServeIngressRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), TimeoutEarlierServeIngressCapacityOrdering,
    0..1, TimeoutEarlierServeIngressCapacityCarrier)

TimeoutEarlierServeZeroLaneRank == <<0, 0>>
TimeoutEarlierServeZeroSelectorRank ==
  <<0, TimeoutEarlierServeZeroLaneRank>>
TimeoutEarlierServeZeroReachSelectorRank ==
  <<0, TimeoutEarlierServeZeroSelectorRank>>
TimeoutEarlierServeZeroCapacityRank ==
  <<0, TimeoutEarlierServeZeroReachSelectorRank>>
TimeoutEarlierServeZeroIngressRank ==
  <<0, TimeoutEarlierServeZeroCapacityRank>>

TimeoutEarlierServeNestedIngressRank(source, identity) ==
  IF AsyncServeIngressAdmissionOwned(source, identity)
  THEN TimeoutEarlierServeIngressRank(source, identity)
  ELSE TimeoutEarlierServeZeroIngressRank

TimeoutEarlierServeLifecycleRank(source, identity) ==
  <<TimeoutEarlierServeLifecycleStage(source, identity),
    <<TimeoutEarlierServeFrozenPredecessorDebt(source, identity),
      TimeoutEarlierServeNestedIngressRank(source, identity)>>>>

TimeoutEarlierServeLifecycleDebtCarrier ==
  Nat \X TimeoutEarlierServeIngressRankCarrier
TimeoutEarlierServeLifecycleRankCarrier ==
  (0..3) \X TimeoutEarlierServeLifecycleDebtCarrier
TimeoutEarlierServeLifecycleDebtOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), TimeoutEarlierServeIngressRankOrdering,
    Nat, TimeoutEarlierServeIngressRankCarrier)
TimeoutEarlierServeLifecycleRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), TimeoutEarlierServeLifecycleDebtOrdering,
    0..3, TimeoutEarlierServeLifecycleDebtCarrier)

TimeoutEarlierServeEpisodeOwnerSet(source, identity) ==
  TimeoutEarlierServeFrozenPredecessorSet(source, identity)
    \cup
  ({"Mode"} \X (1..TimeoutEarlierServeModeRank(source)))
    \cup
  ({"Capacity"} \X
     (1..TimeoutEarlierServeTargetCapacityDebt(source, identity)))
    \cup
  ({"Runner"} \X (1..TimeoutEarlierServeReachRank(source)))
    \cup
  ({"Selector"} \X (1..TimeoutEarlierServePriorityDebt(source)))
    \cup
  ({"Lane"} \X
     (1..TimeoutEarlierServeTicketLanePosition(source, identity)))
    \cup
  ({"Source"} \X
     (1..TimeoutEarlierServeTicketSourcePosition(source, identity)))

TimeoutEarlierServeEpisodeBudget(source, identity) ==
  Cardinality(TimeoutEarlierServeEpisodeOwnerSet(source, identity))

TimeoutEarlierServeEpisodeStaticBound ==
  3 * AsyncIoCapacity
    + 8 * Cardinality(AsyncIngressSources) * AsyncIngressCapacity
    + AsyncServeLifecycleFamilyBudget
    + AsyncRunnerCycleBudget + 8

THEOREM TimeoutEarlierServeIngressRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    TimeoutEarlierServeIngressRankOrdering,
    TimeoutEarlierServeIngressRankCarrier)
BY NatLessThanWellFounded, WFLexPairOrdering
   DEF TimeoutEarlierServeIngressRankOrdering,
       TimeoutEarlierServeIngressRankCarrier,
       TimeoutEarlierServeIngressCapacityOrdering,
       TimeoutEarlierServeIngressCapacityCarrier,
       TimeoutEarlierServeIngressReachSelectorOrdering,
       TimeoutEarlierServeIngressReachSelectorCarrier,
       TimeoutEarlierServeIngressSelectorOrdering,
       TimeoutEarlierServeIngressSelectorCarrier,
       TimeoutEarlierServeIngressLaneOrdering,
       TimeoutEarlierServeIngressLaneCarrier

THEOREM TimeoutEarlierServeLifecycleRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    TimeoutEarlierServeLifecycleRankOrdering,
    TimeoutEarlierServeLifecycleRankCarrier)
BY NatLessThanWellFounded,
   TimeoutEarlierServeIngressRankOrderingIsWellFounded,
   WFLexPairOrdering
   DEF TimeoutEarlierServeLifecycleRankOrdering,
       TimeoutEarlierServeLifecycleRankCarrier,
       TimeoutEarlierServeLifecycleDebtOrdering,
       TimeoutEarlierServeLifecycleDebtCarrier

THEOREM TimeoutEarlierServeOwnedTicketHasExactLaneOccurrence ==
  \A source, identity:
    /\ AsyncStrongTypeInvariant
    /\ AsyncServeIngressAdmissionOwned(source, identity)
    => TimeoutEarlierServeTicketSources(source, identity) # {}
BY IsaT(600)
   DEF TimeoutEarlierServeTicketSources,
       TimeoutEarlierServeTicketLaneIndicesForSource,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, SequenceSet

THEOREM TimeoutEarlierServeLifecycleRankInCarrier ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, identity:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutEarlierServeExactTicketResidual(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)
    => TimeoutEarlierServeLifecycleRank(source, identity)
         \in TimeoutEarlierServeLifecycleRankCarrier
BY TimeoutEarlierServeOwnedTicketHasExactLaneOccurrence,
   FS_Union, FS_Product, FS_CardinalityType, IsaT(600)
   DEF TimeoutEarlierServeExactTicketResidual,
       TimeoutFrozenEarlierServeTicketIdentities,
       TimeoutEarlierServeLifecycleRank,
       TimeoutEarlierServeLifecycleRankCarrier,
       TimeoutEarlierServeLifecycleDebtCarrier,
       TimeoutEarlierServeLifecycleStage,
       TimeoutEarlierServeFrozenPredecessorDebt,
       TimeoutEarlierServeFrozenPredecessorSet,
       TimeoutEarlierServeNestedIngressRank,
       TimeoutEarlierServeIngressRank,
       TimeoutEarlierServeIngressRankCarrier,
       TimeoutEarlierServeCapacityRank,
       TimeoutEarlierServeIngressCapacityCarrier,
       TimeoutEarlierServeReachSelectorRank,
       TimeoutEarlierServeIngressReachSelectorCarrier,
       TimeoutEarlierServeSelectorRank,
       TimeoutEarlierServeIngressSelectorCarrier,
       TimeoutEarlierServeLaneRank,
       TimeoutEarlierServeIngressLaneCarrier,
       TimeoutEarlierServeModeRank,
       TimeoutEarlierServeTargetCapacityDebt,
       TimeoutEarlierServeCapacityDebt,
       TimeoutEarlierServeReachRank,
       TimeoutEarlierServePriorityDebt,
       TimeoutEarlierServePriorityOwners,
       TimeoutEarlierServeTicketLanePosition,
       TimeoutEarlierServeTicketSourcePosition,
       TimeoutEarlierServeTicketSource,
       TimeoutEarlierServeTicketSources,
       TimeoutEarlierServeTicketLaneIndicesForSource,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncServeLifecycleTypeInvariant

THEOREM TimeoutEarlierServeEpisodeBudgetIsFinite ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, identity:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutEarlierServeExactTicketResidual(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)
    => /\ IsFiniteSet(
             TimeoutEarlierServeEpisodeOwnerSet(source, identity))
       /\ TimeoutEarlierServeEpisodeBudget(source, identity) \in Nat
       /\ TimeoutEarlierServeEpisodeBudget(source, identity)
            <= TimeoutEarlierServeEpisodeStaticBound
BY FS_Union, FS_Product, FS_Interval, FS_Subset,
   FS_CardinalityType, IsaT(600)
   DEF TimeoutEarlierServeEpisodeBudget,
       TimeoutEarlierServeEpisodeStaticBound,
       TimeoutEarlierServeEpisodeOwnerSet,
       TimeoutEarlierServeExactTicketResidual,
       TimeoutEarlierServeFrozenPredecessorSet,
       TimeoutEarlierServeModeRank,
       TimeoutEarlierServeTargetCapacityDebt,
       TimeoutEarlierServeCapacityDebt,
       TimeoutEarlierServeReachRank,
       TimeoutEarlierServePriorityDebt,
       TimeoutEarlierServePriorityOwners,
       TimeoutEarlierServeTicketLanePosition,
       TimeoutEarlierServeTicketSourcePosition,
       TimeoutEarlierServeTicketSource,
       TimeoutEarlierServeTicketSources,
       TimeoutEarlierServeTicketLaneIndicesForSource,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncServeLifecycleTypeInvariant, AsyncConfiguration

(***************************************************************************
Source-level ownership facts for the generic ticket.

These facts do not depend on a Decision/body-holding alias.  An ingress
record keeps its shared scheduler ordinal while it remains live, every frozen
predecessor component is monotone, and a fresh record is allocated strictly
after the frozen timeout cut.  A tombstone may emit cached response bytes,
but it cannot recreate the retired Serve job.  Thus a retry after drain can
re-enter only as a new ingress occurrence with the same logical identity and
a higher scheduler ordinal.
***************************************************************************)

TimeoutEarlierServeFrozenBarrierIdentities(source, identity) ==
  AsyncServePreexistingIngressBarrierIdentities(source, identity)

TimeoutEarlierServeFrozenBarrierIdentity(source, identity) ==
  CHOOSE barrierIdentity \in
    TimeoutEarlierServeFrozenBarrierIdentities(source, identity): TRUE

THEOREM TimeoutEarlierServeDuplicateRetainsFrozenTicketOrdinal ==
  \A source \in ValidatorIds,
     laneSource \in AsyncIngressSources:
    LET item == OldestDueSourcePacket(source, laneSource).item
        identity == AsyncServeLogicalRequestIdentity(source, item)
    IN /\ item.kind \in AsyncReplyRequestKinds
       /\ AsyncServeIngressAdmissionOwned(source, identity)
       /\ CoalesceHiddenPacket(source, laneSource)
       => /\ AsyncServeIngressAdmissionOwned(source, identity)'
          /\ AsyncServeIngressAdmissionSchedulerOrdinal(
                 source, identity)'
               = AsyncServeIngressAdmissionSchedulerOrdinal(
                   source, identity)
BY AsyncLiveServeIngressDuplicateRetainsSchedulerOrdinal

THEOREM TimeoutEarlierServeTicketSchedulerOrdinalPersistsUntilDrain ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, identity:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutEarlierServeExactTicketResidual(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)
    /\ AsyncNext
    /\ TimeoutEarlierServeExactTicketResidual(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)'
    => AsyncServeIngressAdmissionSchedulerOrdinal(source, identity)'
         = AsyncServeIngressAdmissionSchedulerOrdinal(source, identity)
BY IsaT(300)
   DEF TimeoutEarlierServeExactTicketResidual,
       TimeoutFrozenEarlierServeTicketIdentities,
       AsyncServeIngressAdmissionSchedulerOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       ReserveExactServeCapacity, AdvanceExactServeCapacity,
       CoalesceExactServeIngressCapacity,
       AcceptOrReserveExactServeIngress,
       PopSelectedIngress,
       ResetNodeSchedulerForRestart,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay

THEOREM TimeoutEarlierServeFrozenPredecessorsDoNotReplenish ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, identity:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutEarlierServeExactTicketResidual(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)
    /\ AsyncNext
    /\ TimeoutEarlierServeExactTicketResidual(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)'
    => TimeoutEarlierServeFrozenPredecessorSet(source, identity)'
         \subseteq
           TimeoutEarlierServeFrozenPredecessorSet(source, identity)
BY IsaT(600)
   DEF TimeoutEarlierServeExactTicketResidual,
       TimeoutEarlierServeFrozenPredecessorSet,
       AsyncServeFrozenPredecessorSet,
       AsyncServeFrozenIngressPredecessorSet,
       AsyncServeFrozenIngressPredecessorDebtSlots,
       AsyncServeFrozenIngressPredecessorCounts,
       AsyncServeIngressAdmissionPredecessorDebtSlots,
       AsyncServeIngressAdmissionPredecessorCounts,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServePreexistingIngressOwnerPredecessorDebtSet,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressBarrierPredecessorDebtSet,
       AsyncServeIngressIdentityFrozenByReservation,
       AsyncServeIngressLiveReservations,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeSingularOffQueueBarrierInvariant,
       AsyncServeBarrierOwnsEarliestIngressOrdinalInvariant,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ReserveExactServeCapacity, AdvanceExactServeCapacity,
       CoalesceExactServeIngressCapacity,
       AcceptOrReserveExactServeIngress,
       ExactServeTransportAdmissionCanAdvance,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart

THEOREM TimeoutEarlierServeFrozenSelectorRejectsPostCutIngress ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, identity,
     laneSource, index:
    LET item == asyncIngressLanes[source][laneSource][index]
    IN /\ AsyncStrongTypeInvariant
       /\ TimeoutEarlierServeExactTicketResidual(
            mode, source, sourceView,
            ownerContext, ownerOrigin, ownerOrdinal, identity)
       /\ identity =
            AsyncServeEarliestIngressLifecycleOwnerIdentity(source)
       /\ laneSource \in AsyncIngressSources
       /\ index \in 1..Len(asyncIngressLanes[source][laneSource])
       /\ index >
            AsyncServeIngressAdmissionPredecessorCounts(
              source, identity)[laneSource]
       /\ \/ item.kind \notin AsyncReplyRequestKinds
          \/ AsyncServeLogicalRequestIdentity(source, item) # identity
       => ~AsyncServeIngressIndexMayPrecedeAdmittedTarget(
             source, laneSource, index)
BY Isa
   DEF TimeoutEarlierServeExactTicketResidual,
       AsyncServeIngressIndexMayPrecedeAdmittedTarget,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionPredecessorCounts

THEOREM TimeoutEarlierServeReservationFencesLaterIoProducer ==
  \A source, identity:
    \A commandClass \in AsyncIoCommandClasses:
      /\ AsyncServeLiveReservationOwned(source, identity)
      /\ ~AsyncServeJobQueued(source, identity)
      => /\ ~CanEnqueueIoClass(source, commandClass)
         /\ ~EnqueueIoLocalControlWork(source)
BY Isa
   DEF CanEnqueueIoClass, AsyncIoEffectiveQueueDepth,
       AsyncIoAdmissionLimit, AsyncIoCommandClasses,
       AsyncServeOffQueueReservations,
       EnqueueIoLocalControlWork

THEOREM TimeoutEarlierServeFreshAdmissionIsAfterFrozenOwner ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal:
    \A admission \in
         AsyncFreshServeIngressAdmissionsForNodeThisStep(source):
      /\ AsyncStrongTypeInvariant
      /\ TimeoutFrozenEarlierServePrefixPending(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal)
      /\ AsyncNext
      => ownerOrdinal < admission.schedulerOrdinal
BY AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal,
   IsaT(300)
   DEF TimeoutFrozenEarlierServePrefixPending,
       TimeoutFrozenRuntimeLifecycleOwner,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncControlServiceStateTypeInvariant,
       AsyncNext, AsyncControlServiceSlotTransition

THEOREM TimeoutEarlierServeTombstoneCannotResurrectServeJob ==
  \A source \in ValidatorIds,
     identity \in AsyncServeLogicalRequestIdentities:
    /\ AsyncServeLifecycleTypeInvariant
    /\ AsyncServeLifecycleTombstone(source, identity)
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => /\ AsyncServeLogicalIdentityRetiredOrSuperseded(
            source, identity)'
       /\ ~AsyncServeJobQueued(source, identity)'
BY AsyncServeTombstonedIdentityCannotRequeueAtGst

THEOREM TimeoutEarlierServeIngressPopRetiresSelectedTicket ==
  \A source, readyIndex, laneIndex:
    LET laneSource == asyncIngressReady[source][readyIndex]
        item == asyncIngressLanes[source][laneSource][laneIndex]
        identity == AsyncServeLogicalRequestIdentity(source, item)
    IN /\ item.kind \in AsyncReplyRequestKinds
       /\ AsyncServeIngressAdmissionOwned(source, identity)
       /\ PopSelectedIngress(source, readyIndex, laneIndex)
       => ~AsyncServeIngressAdmissionOwned(source, identity)'
BY Isa
   DEF PopSelectedIngress,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout

THEOREM TimeoutFrozenEarlierServeTicketSetCannotReplenish ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutFrozenEarlierServePrefixPending(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal)
    /\ AsyncNext
    /\ TimeoutFrozenEarlierServePrefixPending(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal)'
    => TimeoutFrozenEarlierServeTicketIdentities(
         source, ownerOrdinal)'
         \subseteq
           TimeoutFrozenEarlierServeTicketIdentities(
             source, ownerOrdinal)
BY TimeoutEarlierServeFreshAdmissionIsAfterFrozenOwner,
   TimeoutEarlierServeTicketSchedulerOrdinalPersistsUntilDrain,
   AsyncTimeoutLifecycleOrdinalPersistsUntilEndpoint,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   IsaT(900)
   DEF TimeoutFrozenEarlierServeTicketIdentities,
       TimeoutFrozenEarlierServePrefixPending,
       TimeoutFrozenRuntimeLifecycleOwner,
       AsyncFreshServeIngressAdmissionsForNodeThisStep,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionSchedulerOrdinal,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncNext, AsyncAllVars

(***************************************************************************
Exact action-step seam.

All state-derived rank geometry is now explicit above.  The local proof
boundary is the classification/origin theorem below: each
concrete `AsyncNext` either reaches the exact ticket goal, strictly lowers the
lifecycle rank, consumes one member of the finite frozen producer episode, or
is exact noninterference; while a rank cell stays open, one of the three
already-fair physical actions is enabled, stable, and consumes that cell.
The action lemmas below discharge that boundary from the source transition
relation before the temporal lift is composed.
No eventual quiescence, cardinality-as-progress shortcut, or new fairness
action is assumed.
***************************************************************************)

TimeoutEarlierServeFiniteProducerEpisodeAction(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, identity) ==
  /\ TimeoutEarlierServeExactTicketResidual(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal, identity)
  /\ AsyncNext
  /\ TimeoutEarlierServeExactTicketResidual(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal, identity)'
  /\ TimeoutEarlierServeLifecycleRank(source, identity)'
       = TimeoutEarlierServeLifecycleRank(source, identity)
  /\ TimeoutEarlierServeEpisodeBudget(source, identity)'
       < TimeoutEarlierServeEpisodeBudget(source, identity)

TimeoutEarlierServeNoninterferenceAction(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, identity) ==
  /\ TimeoutEarlierServeExactTicketResidual(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal, identity)
  /\ AsyncNext
  /\ TimeoutEarlierServeExactTicketResidual(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal, identity)'
  /\ TimeoutEarlierServeLifecycleRank(source, identity)'
       = TimeoutEarlierServeLifecycleRank(source, identity)
  /\ TimeoutEarlierServeEpisodeBudget(source, identity)'
       = TimeoutEarlierServeEpisodeBudget(source, identity)

TimeoutEarlierServeIngressStrictComponentDecrease(source, identity) ==
  \/ TimeoutEarlierServeModeRank(source)'
       < TimeoutEarlierServeModeRank(source)
  \/ /\ TimeoutEarlierServeModeRank(source)'
          = TimeoutEarlierServeModeRank(source)
     /\ TimeoutEarlierServeTargetCapacityDebt(source, identity)'
          < TimeoutEarlierServeTargetCapacityDebt(source, identity)
  \/ /\ TimeoutEarlierServeModeRank(source)'
          = TimeoutEarlierServeModeRank(source)
     /\ TimeoutEarlierServeTargetCapacityDebt(source, identity)'
          = TimeoutEarlierServeTargetCapacityDebt(source, identity)
     /\ TimeoutEarlierServeReachRank(source)'
          < TimeoutEarlierServeReachRank(source)
  \/ /\ TimeoutEarlierServeModeRank(source)'
          = TimeoutEarlierServeModeRank(source)
     /\ TimeoutEarlierServeTargetCapacityDebt(source, identity)'
          = TimeoutEarlierServeTargetCapacityDebt(source, identity)
     /\ TimeoutEarlierServeReachRank(source)'
          = TimeoutEarlierServeReachRank(source)
     /\ TimeoutEarlierServePriorityDebt(source)'
          < TimeoutEarlierServePriorityDebt(source)
  \/ /\ TimeoutEarlierServeModeRank(source)'
          = TimeoutEarlierServeModeRank(source)
     /\ TimeoutEarlierServeTargetCapacityDebt(source, identity)'
          = TimeoutEarlierServeTargetCapacityDebt(source, identity)
     /\ TimeoutEarlierServeReachRank(source)'
          = TimeoutEarlierServeReachRank(source)
     /\ TimeoutEarlierServePriorityDebt(source)'
          = TimeoutEarlierServePriorityDebt(source)
     /\ TimeoutEarlierServeTicketLanePosition(source, identity)'
          < TimeoutEarlierServeTicketLanePosition(source, identity)
  \/ /\ TimeoutEarlierServeModeRank(source)'
          = TimeoutEarlierServeModeRank(source)
     /\ TimeoutEarlierServeTargetCapacityDebt(source, identity)'
          = TimeoutEarlierServeTargetCapacityDebt(source, identity)
     /\ TimeoutEarlierServeReachRank(source)'
          = TimeoutEarlierServeReachRank(source)
     /\ TimeoutEarlierServePriorityDebt(source)'
          = TimeoutEarlierServePriorityDebt(source)
     /\ TimeoutEarlierServeTicketLanePosition(source, identity)'
          = TimeoutEarlierServeTicketLanePosition(source, identity)
     /\ TimeoutEarlierServeTicketSourcePosition(source, identity)'
          < TimeoutEarlierServeTicketSourcePosition(source, identity)

THEOREM TimeoutEarlierServeStrictComponentLowersIngressRank ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, identity:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutEarlierServeExactTicketResidual(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)
    /\ AsyncStrongTypeInvariant'
    /\ TimeoutEarlierServeExactTicketResidual(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)'
    /\ TimeoutEarlierServeIngressStrictComponentDecrease(source, identity)
    => <<TimeoutEarlierServeIngressRank(source, identity)',
          TimeoutEarlierServeIngressRank(source, identity)>>
         \in TimeoutEarlierServeIngressRankOrdering
BY Isa
   DEF TimeoutEarlierServeIngressStrictComponentDecrease,
       TimeoutEarlierServeIngressRank,
       TimeoutEarlierServeCapacityRank,
       TimeoutEarlierServeReachSelectorRank,
       TimeoutEarlierServeSelectorRank,
       TimeoutEarlierServeLaneRank,
       TimeoutEarlierServeIngressRankOrdering,
       TimeoutEarlierServeIngressCapacityOrdering,
       TimeoutEarlierServeIngressReachSelectorOrdering,
       TimeoutEarlierServeIngressSelectorOrdering,
       TimeoutEarlierServeIngressLaneOrdering,
       LexPairOrdering, OpToRel

THEOREM TimeoutEarlierServeStutterPreservesRankAndBudget ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, identity:
    /\ TimeoutEarlierServeExactTicketResidual(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)
    /\ UNCHANGED AsyncAllVars
    => /\ TimeoutEarlierServeExactTicketResidual(
            mode, source, sourceView,
            ownerContext, ownerOrigin, ownerOrdinal, identity)'
       /\ TimeoutEarlierServeLifecycleRank(source, identity)'
            = TimeoutEarlierServeLifecycleRank(source, identity)
       /\ TimeoutEarlierServeEpisodeBudget(source, identity)'
            = TimeoutEarlierServeEpisodeBudget(source, identity)
BY Isa
   DEF TimeoutEarlierServeExactTicketResidual,
       TimeoutEarlierServeLifecycleRank,
       TimeoutEarlierServeLifecycleStage,
       TimeoutEarlierServeFrozenPredecessorDebt,
       TimeoutEarlierServeFrozenPredecessorSet,
       TimeoutEarlierServeNestedIngressRank,
       TimeoutEarlierServeIngressRank,
       TimeoutEarlierServeCapacityRank,
       TimeoutEarlierServeReachSelectorRank,
       TimeoutEarlierServeSelectorRank,
       TimeoutEarlierServeLaneRank,
       TimeoutEarlierServeModeRank,
       TimeoutEarlierServeTargetCapacityDebt,
       TimeoutEarlierServeCapacityDebt,
       TimeoutEarlierServeReachRank,
       TimeoutEarlierServePriorityDebt,
       TimeoutEarlierServePriorityOwners,
       TimeoutEarlierServeTicketLanePosition,
       TimeoutEarlierServeTicketSourcePosition,
       TimeoutEarlierServeTicketSource,
       TimeoutEarlierServeTicketSources,
       TimeoutEarlierServeTicketLaneIndicesForSource,
       TimeoutEarlierServeEpisodeBudget,
       TimeoutEarlierServeEpisodeOwnerSet,
       AsyncAllVars, AsyncSchedulerVars, vars

TimeoutEarlierServeLifecycleStepClassification(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, identity) ==
  /\ TimeoutEarlierServeExactTicketResidual(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal, identity)
  /\ AsyncNext
  => \/ TimeoutEarlierServeExactTicketGoal(
          mode, source, sourceView,
          ownerContext, ownerOrigin, ownerOrdinal, identity)'
     \/ <<TimeoutEarlierServeLifecycleRank(source, identity)',
           TimeoutEarlierServeLifecycleRank(source, identity)>>
          \in TimeoutEarlierServeLifecycleRankOrdering
     \/ TimeoutEarlierServeFiniteProducerEpisodeAction(
          mode, source, sourceView,
          ownerContext, ownerOrigin, ownerOrdinal, identity)
     \/ TimeoutEarlierServeNoninterferenceAction(
          mode, source, sourceView,
          ownerContext, ownerOrigin, ownerOrdinal, identity)

TimeoutEarlierServeAtRank(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, identity, rank) ==
  /\ TimeoutEarlierServeExactTicketResidual(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal, identity)
  /\ TimeoutEarlierServeLifecycleRank(source, identity) = rank

TimeoutEarlierServeAtRankAndBudget(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, identity, rank, budget) ==
  /\ TimeoutEarlierServeAtRank(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal, identity, rank)
  /\ TimeoutEarlierServeEpisodeBudget(source, identity) = budget

TimeoutEarlierServeRankGoal(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, identity, rank) ==
  \/ TimeoutEarlierServeExactTicketGoal(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal, identity)
  \/ <<TimeoutEarlierServeLifecycleRank(source, identity), rank>>
       \in TimeoutEarlierServeLifecycleRankOrdering

TimeoutEarlierServeConcreteFairOwnerKinds ==
  {"NormalRunner", "HistoricalServer", "IoWorker"}

TimeoutEarlierServeIoOwnerRequired(source, identity) ==
  LET barriers ==
        TimeoutEarlierServeFrozenBarrierIdentities(source, identity)
  IN \/ /\ AsyncServeLiveReservationOwned(source, identity)
           /\ ~AsyncServeJobQueued(source, identity)
           /\ ~CanResumeExactServeCapacity(source, identity)
     \/ /\ barriers # {}
           /\ ~CanResumeExactServeCapacity(
                source,
                TimeoutEarlierServeFrozenBarrierIdentity(
                  source, identity))

TimeoutEarlierServeConcreteFairOwner(source, identity) ==
  IF TimeoutEarlierServeIoOwnerRequired(source, identity)
  THEN "IoWorker"
  ELSE IF NodeHasApplication(source)
       THEN "HistoricalServer"
       ELSE "NormalRunner"

TimeoutEarlierServeConcreteFairAction(source, ownerKind) ==
  CASE ownerKind = "NormalRunner" -> PostGstRunNode(source)
    [] ownerKind = "HistoricalServer" ->
         PostGstRunHistoricalServer(source)
    [] ownerKind = "IoWorker" -> PostGstServiceIoWorker(source)
    [] OTHER -> FALSE

TimeoutEarlierServeSelectedConcreteFairAction(source, identity) ==
  TimeoutEarlierServeConcreteFairAction(
    source, TimeoutEarlierServeConcreteFairOwner(source, identity))

TimeoutEarlierServeRankCellOutcome(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, identity, rank, budget) ==
  \/ TimeoutEarlierServeRankGoal(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal, identity, rank)
  \/ \E lowerBudget \in
       SetLessThan(budget, OpToRel(<, Nat), Nat):
       TimeoutEarlierServeAtRankAndBudget(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal,
         identity, rank, lowerBudget)

TimeoutEarlierServeConcreteActionOriginProperty(specification) ==
  /\ specification
       => [](\A mode \in TimeoutRuntimeModeCarrier,
                  source \in AsyncCurrentResponsiveVoters,
                  sourceView \in Views,
                  ownerContext \in ContextRecords,
                  ownerOrigin \in AsyncCandidateCausalOriginSet,
                  ownerOrdinal \in Nat \ {0},
                  identity \in AsyncServeLogicalRequestIdentities,
                  rank \in TimeoutEarlierServeLifecycleRankCarrier,
                  budget \in Nat:
              /\ TimeoutEarlierServeAtRankAndBudget(
                   mode, source, sourceView,
                   ownerContext, ownerOrigin, ownerOrdinal,
                   identity, rank, budget)
              /\ ~TimeoutEarlierServeRankGoal(
                   mode, source, sourceView,
                   ownerContext, ownerOrigin, ownerOrdinal,
                   identity, rank)
              => /\ TimeoutEarlierServeConcreteFairOwner(source, identity)
                       \in TimeoutEarlierServeConcreteFairOwnerKinds
                 /\ ENABLED
                      <<TimeoutEarlierServeSelectedConcreteFairAction(
                          source, identity)>>_AsyncAllVars)
  /\ specification
       => [](\A mode \in TimeoutRuntimeModeCarrier,
                  source \in AsyncCurrentResponsiveVoters,
                  sourceView \in Views,
                  ownerContext \in ContextRecords,
                  ownerOrigin \in AsyncCandidateCausalOriginSet,
                  ownerOrdinal \in Nat \ {0},
                  identity \in AsyncServeLogicalRequestIdentities,
                  rank \in TimeoutEarlierServeLifecycleRankCarrier,
                  budget \in Nat:
              /\ TimeoutEarlierServeAtRankAndBudget(
                   mode, source, sourceView,
                   ownerContext, ownerOrigin, ownerOrdinal,
                   identity, rank, budget)
              /\ ~TimeoutEarlierServeRankGoal(
                   mode, source, sourceView,
                   ownerContext, ownerOrigin, ownerOrdinal,
                   identity, rank)
              /\ [AsyncNext]_AsyncAllVars
              => \/ TimeoutEarlierServeRankCellOutcome(
                      mode, source, sourceView,
                      ownerContext, ownerOrigin, ownerOrdinal,
                      identity, rank, budget)'
                 \/ /\ TimeoutEarlierServeAtRankAndBudget(
                           mode, source, sourceView,
                           ownerContext, ownerOrigin, ownerOrdinal,
                           identity, rank, budget)'
                    /\ TimeoutEarlierServeConcreteFairOwner(
                         source, identity)'
                         = TimeoutEarlierServeConcreteFairOwner(
                             source, identity))
  /\ specification
       => [](\A mode \in TimeoutRuntimeModeCarrier,
                  source \in AsyncCurrentResponsiveVoters,
                  sourceView \in Views,
                  ownerContext \in ContextRecords,
                  ownerOrigin \in AsyncCandidateCausalOriginSet,
                  ownerOrdinal \in Nat \ {0},
                  identity \in AsyncServeLogicalRequestIdentities,
                  rank \in TimeoutEarlierServeLifecycleRankCarrier,
                  budget \in Nat:
              /\ TimeoutEarlierServeAtRankAndBudget(
                   mode, source, sourceView,
                   ownerContext, ownerOrigin, ownerOrdinal,
                   identity, rank, budget)
              /\ ~TimeoutEarlierServeRankGoal(
                   mode, source, sourceView,
                   ownerContext, ownerOrigin, ownerOrdinal,
                   identity, rank)
              /\ <<TimeoutEarlierServeSelectedConcreteFairAction(
                     source, identity)>>_AsyncAllVars
              => TimeoutEarlierServeRankCellOutcome(
                   mode, source, sourceView,
                   ownerContext, ownerOrigin, ownerOrdinal,
                   identity, rank, budget)')

TimeoutEarlierServeExactIngressRankStepProperty(specification) ==
  /\ specification
       => [](\A mode \in TimeoutRuntimeModeCarrier,
                  source \in AsyncCurrentResponsiveVoters,
                  sourceView \in Views,
                  ownerContext \in ContextRecords,
                  ownerOrigin \in AsyncCandidateCausalOriginSet,
                  ownerOrdinal \in Nat \ {0},
                  identity \in AsyncServeLogicalRequestIdentities:
              TimeoutEarlierServeLifecycleStepClassification(
                mode, source, sourceView,
                ownerContext, ownerOrigin, ownerOrdinal, identity))
  /\ TimeoutEarlierServeConcreteActionOriginProperty(specification)

THEOREM TimeoutEarlierServeLifecycleStepClassificationIsExhaustive ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, identity:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ TimeoutEarlierServeExactTicketResidual(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)
    /\ AsyncNext
    => TimeoutEarlierServeLifecycleStepClassification(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)
BY TimeoutEarlierServeFrozenPredecessorsDoNotReplenish,
   TimeoutEarlierServeTicketSchedulerOrdinalPersistsUntilDrain,
   TimeoutEarlierServeTombstoneCannotResurrectServeJob,
   TimeoutEarlierServeIngressPopRetiresSelectedTicket,
   TimeoutEarlierServeStrictComponentLowersIngressRank,
   AsyncServeIngressTicketExcludesLaterLocalWork,
   ExactTicketTurnDecreasesDrainableIngressTurnReach,
   ExhaustedIngressStepDecreasesDrainableIngressTurnReach,
   LocalStepDecreasesDrainableIngressTurnReach,
   SerializedLocalPredecessorDecreasesDrainableIngressTurnReach,
   RuntimeStepDecreasesDrainableIngressTurnReach,
   OlderRuntimeInterleaveDecreasesDrainableIngressTurnReach,
   AsyncBracketNextPreservesStrongTypeInvariant,
   FS_CardinalityType, FS_Subset, IsaT(2400)
   DEF TimeoutEarlierServeLifecycleStepClassification,
       TimeoutEarlierServeFiniteProducerEpisodeAction,
       TimeoutEarlierServeNoninterferenceAction,
       TimeoutEarlierServeExactTicketGoal,
       TimeoutEarlierServeExactTicketResidual,
       TimeoutEarlierServeLifecycleRank,
       TimeoutEarlierServeLifecycleStage,
       TimeoutEarlierServeFrozenPredecessorDebt,
       TimeoutEarlierServeNestedIngressRank,
       TimeoutEarlierServeIngressRank,
       TimeoutEarlierServeCapacityRank,
       TimeoutEarlierServeReachSelectorRank,
       TimeoutEarlierServeSelectorRank,
       TimeoutEarlierServeLaneRank,
       TimeoutEarlierServeModeRank,
       TimeoutEarlierServeTargetCapacityDebt,
       TimeoutEarlierServeCapacityDebt,
       TimeoutEarlierServeReachRank,
       TimeoutEarlierServePriorityDebt,
       TimeoutEarlierServePriorityOwners,
       TimeoutEarlierServeTicketLanePosition,
       TimeoutEarlierServeTicketSourcePosition,
       TimeoutEarlierServeTicketSource,
       TimeoutEarlierServeTicketSources,
       TimeoutEarlierServeTicketLaneIndicesForSource,
       TimeoutEarlierServeEpisodeBudget,
       TimeoutEarlierServeEpisodeOwnerSet,
       TimeoutEarlierServeFrozenBarrierIdentities,
       TimeoutEarlierServeFrozenBarrierIdentity,
       TimeoutFrozenEarlierServeTicketIdentities,
       TimeoutFrozenEarlierServePrefixPending,
       TimeoutFrozenRuntimeLifecycleOwner,
       AsyncServeIngressTargetOnlyTurn,
       AsyncServeIngressIndexMayPrecedeAdmittedTarget,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ServiceIoWorkerWork, PopSelectedIngress,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       LocalAdmissionStep, IngressDrainStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep, RuntimeStep,
       AsyncNext, AsyncAllVars,
       SetLessThan, LexPairOrdering, OpToRel

THEOREM TimeoutEarlierServeSelectedActionEnabledAtEpisode ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal,
     identity, rank, budget:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ TimeoutEarlierServeAtRankAndBudget(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal,
         identity, rank, budget)
    /\ ~TimeoutEarlierServeRankGoal(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity, rank)
    => ENABLED
         <<TimeoutEarlierServeSelectedConcreteFairAction(
             source, identity)>>_AsyncAllVars
BY QueuedIoEnablesPostGstService,
   QueuedIoServiceIsNonstuttering,
   GstResponsiveUnappliedRunNodeIsEnabled,
   RunNodeIsNonstuttering,
   GstHistoricalServerIsEnabled,
   ExpandENABLED, ENABLEDaxioms, IsaT(1200)
   DEF TimeoutEarlierServeSelectedConcreteFairAction,
       TimeoutEarlierServeConcreteFairAction,
       TimeoutEarlierServeConcreteFairOwner,
       TimeoutEarlierServeConcreteFairOwnerKinds,
       TimeoutEarlierServeIoOwnerRequired,
       TimeoutEarlierServeFrozenBarrierIdentities,
       TimeoutEarlierServeFrozenBarrierIdentity,
       TimeoutEarlierServeAtRankAndBudget,
       TimeoutEarlierServeAtRank,
       TimeoutEarlierServeRankGoal,
       TimeoutEarlierServeExactTicketGoal,
       TimeoutEarlierServeExactTicketResidual,
       TimeoutFrozenEarlierServeTicketIdentities,
       TimeoutFrozenEarlierServePrefixPending,
       TimeoutFrozenRuntimeLifecycleOwner,
       CanResumeExactServeCapacity,
       AsyncServeJobQueued,
       AsyncServeLiveReservationOwned,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServeIngressIdentityFrozenByReservation,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeEarliestIngressLifecycleOwnerIdentity,
       AsyncServeBarrierOwnsEarliestIngressOrdinalInvariant,
       AsyncServeIngressIndexMayPrecedeAdmittedTarget,
       AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncCurrentResponsiveVoters,
       AsyncIoQueueDepth, AsyncAllVars

THEOREM TimeoutEarlierServeBracketStepPreservesEpisodeOrGoal ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal,
     identity, rank, budget:
    /\ TimeoutEarlierServeLifecycleStepClassification(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity)
    /\ TimeoutEarlierServeAtRankAndBudget(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal,
         identity, rank, budget)
    /\ [AsyncNext]_AsyncAllVars
    => \/ TimeoutEarlierServeRankCellOutcome(
            mode, source, sourceView,
            ownerContext, ownerOrigin, ownerOrdinal,
            identity, rank, budget)'
       \/ TimeoutEarlierServeAtRankAndBudget(
            mode, source, sourceView,
            ownerContext, ownerOrigin, ownerOrdinal,
            identity, rank, budget)'
BY TimeoutEarlierServeStutterPreservesRankAndBudget, IsaT(300)
   DEF TimeoutEarlierServeLifecycleStepClassification,
       TimeoutEarlierServeFiniteProducerEpisodeAction,
       TimeoutEarlierServeNoninterferenceAction,
       TimeoutEarlierServeRankCellOutcome,
       TimeoutEarlierServeAtRankAndBudget,
       TimeoutEarlierServeAtRank,
       TimeoutEarlierServeRankGoal,
       SetLessThan, AsyncAllVars

THEOREM TimeoutEarlierServeConcreteOwnerPersistsInRankCell ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal,
     identity, rank, budget:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ TimeoutEarlierServeAtRankAndBudget(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal,
         identity, rank, budget)
    /\ ~TimeoutEarlierServeRankGoal(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ TimeoutEarlierServeAtRankAndBudget(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal,
         identity, rank, budget)'
    => TimeoutEarlierServeConcreteFairOwner(source, identity)'
         = TimeoutEarlierServeConcreteFairOwner(source, identity)
BY TimeoutEarlierServeFrozenPredecessorsDoNotReplenish,
   TimeoutEarlierServeTicketSchedulerOrdinalPersistsUntilDrain,
   TimeoutEarlierServeTombstoneCannotResurrectServeJob,
   TimeoutFrozenEarlierServeTicketSetCannotReplenish,
   IsaT(900)
   DEF TimeoutEarlierServeConcreteFairOwner,
       TimeoutEarlierServeIoOwnerRequired,
       TimeoutEarlierServeFrozenBarrierIdentities,
       TimeoutEarlierServeFrozenBarrierIdentity,
       TimeoutEarlierServeAtRankAndBudget,
       TimeoutEarlierServeAtRank,
       TimeoutEarlierServeRankGoal,
       TimeoutEarlierServeLifecycleRank,
       TimeoutEarlierServeLifecycleStage,
       TimeoutEarlierServeFrozenPredecessorDebt,
       TimeoutEarlierServeFrozenPredecessorSet,
       TimeoutEarlierServeEpisodeBudget,
       TimeoutEarlierServeEpisodeOwnerSet,
       CanResumeExactServeCapacity,
       AsyncServeJobQueued,
       AsyncServeLiveReservationOwned,
       AsyncServeFrozenPredecessorSet,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServeIngressIdentityFrozenByReservation,
       AsyncServeIngressAdmissionOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeEarliestIngressLifecycleOwnerIdentity,
       AsyncServeBarrierOwnsEarliestIngressOrdinalInvariant,
       AsyncAllVars

THEOREM TimeoutEarlierServeSelectedActionConsumesEpisode ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal,
     identity, rank, budget:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ TimeoutEarlierServeAtRankAndBudget(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal,
         identity, rank, budget)
    /\ ~TimeoutEarlierServeRankGoal(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, identity, rank)
    /\ <<TimeoutEarlierServeSelectedConcreteFairAction(
           source, identity)>>_AsyncAllVars
    => TimeoutEarlierServeRankCellOutcome(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal,
         identity, rank, budget)'
BY TimeoutEarlierServeFrozenPredecessorsDoNotReplenish,
   TimeoutEarlierServeFrozenSelectorRejectsPostCutIngress,
   TimeoutEarlierServeReservationFencesLaterIoProducer,
   TimeoutEarlierServeIngressPopRetiresSelectedTicket,
   TimeoutEarlierServeTombstoneCannotResurrectServeJob,
   TimeoutEarlierServeStrictComponentLowersIngressRank,
   AsyncServeIngressTicketExcludesLaterLocalWork,
   ExactTicketTurnDecreasesDrainableIngressTurnReach,
   ExhaustedIngressStepDecreasesDrainableIngressTurnReach,
   LocalStepDecreasesDrainableIngressTurnReach,
   RuntimeStepDecreasesDrainableIngressTurnReach,
   ServiceIoWorkerDropsQueueDepth,
   AsyncBracketNextPreservesStrongTypeInvariant,
   FS_CardinalityType, FS_Subset, IsaT(3000)
   DEF TimeoutEarlierServeSelectedConcreteFairAction,
       TimeoutEarlierServeConcreteFairAction,
       TimeoutEarlierServeConcreteFairOwner,
       TimeoutEarlierServeIoOwnerRequired,
       TimeoutEarlierServeFrozenBarrierIdentities,
       TimeoutEarlierServeFrozenBarrierIdentity,
       TimeoutEarlierServeRankCellOutcome,
       TimeoutEarlierServeAtRankAndBudget,
       TimeoutEarlierServeAtRank,
       TimeoutEarlierServeRankGoal,
       TimeoutEarlierServeExactTicketGoal,
       TimeoutEarlierServeExactTicketResidual,
       TimeoutEarlierServeLifecycleRank,
       TimeoutEarlierServeLifecycleStage,
       TimeoutEarlierServeFrozenPredecessorDebt,
       TimeoutEarlierServeFrozenPredecessorSet,
       TimeoutEarlierServeNestedIngressRank,
       TimeoutEarlierServeIngressRank,
       TimeoutEarlierServeCapacityRank,
       TimeoutEarlierServeReachSelectorRank,
       TimeoutEarlierServeSelectorRank,
       TimeoutEarlierServeLaneRank,
       TimeoutEarlierServeModeRank,
       TimeoutEarlierServeTargetCapacityDebt,
       TimeoutEarlierServeCapacityDebt,
       TimeoutEarlierServeReachRank,
       TimeoutEarlierServePriorityDebt,
       TimeoutEarlierServePriorityOwners,
       TimeoutEarlierServeTicketLanePosition,
       TimeoutEarlierServeTicketSourcePosition,
       TimeoutEarlierServeTicketSource,
       TimeoutEarlierServeTicketSources,
       TimeoutEarlierServeTicketLaneIndicesForSource,
       TimeoutEarlierServeEpisodeBudget,
       TimeoutEarlierServeEpisodeOwnerSet,
       TimeoutFrozenEarlierServeTicketIdentities,
       TimeoutFrozenEarlierServePrefixPending,
       TimeoutFrozenRuntimeLifecycleOwner,
       AsyncServeIngressTargetOnlyTurn,
       AsyncServeIngressIndexMayPrecedeAdmittedTarget,
       AsyncServeIngressAdmissionPredecessorDebtSlots,
       AsyncServeIngressAdmissionPredecessorCounts,
       AsyncServePreexistingIngressOwnerIdentities,
       AsyncServePreexistingIngressOwnerPredecessorDebtSet,
       AsyncServePreexistingIngressBarrierIdentities,
       AsyncServePreexistingIngressBarrierPredecessorDebtSet,
       AsyncServeIngressIdentityFrozenByReservation,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ServiceIoWorkerWork, PopSelectedIngress,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       PostGstRunNode, RunNode, RunNodeWork,
       PostGstRunHistoricalServer, RunHistoricalServer,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       SetLessThan, LexPairOrdering, OpToRel, AsyncAllVars

THEOREM AsyncLiveClosesTimeoutEarlierServeExactIngressRankStep ==
  \A initialContext:
    TimeoutEarlierServeExactIngressRankStepProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   TimeoutEarlierServeLifecycleStepClassificationIsExhaustive,
   TimeoutEarlierServeSelectedActionEnabledAtEpisode,
   TimeoutEarlierServeBracketStepPreservesEpisodeOrGoal,
   TimeoutEarlierServeConcreteOwnerPersistsInRankCell,
   TimeoutEarlierServeSelectedActionConsumesEpisode,
   PTL, IsaT(1200)
   DEF TimeoutEarlierServeExactIngressRankStepProperty,
       TimeoutEarlierServeConcreteActionOriginProperty,
       TimeoutEarlierServeRankCellOutcome

TimeoutEarlierServeFiniteEpisodeClosureProperty(specification) ==
  specification
    => \A mode \in TimeoutRuntimeModeCarrier,
          source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          ownerContext \in ContextRecords,
          ownerOrigin \in AsyncCandidateCausalOriginSet,
          ownerOrdinal \in Nat \ {0},
          identity \in AsyncServeLogicalRequestIdentities,
          rank \in TimeoutEarlierServeLifecycleRankCarrier,
          budget \in Nat:
         TimeoutEarlierServeAtRankAndBudget(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal,
           identity, rank, budget)
           ~> TimeoutEarlierServeRankCellOutcome(
                 mode, source, sourceView,
                 ownerContext, ownerOrigin, ownerOrdinal,
                 identity, rank, budget)

TimeoutEarlierServeRankCellClosureProperty(specification) ==
  specification
    => \A mode \in TimeoutRuntimeModeCarrier,
          source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          ownerContext \in ContextRecords,
          ownerOrigin \in AsyncCandidateCausalOriginSet,
          ownerOrdinal \in Nat \ {0},
          identity \in AsyncServeLogicalRequestIdentities,
          rank \in TimeoutEarlierServeLifecycleRankCarrier:
         TimeoutEarlierServeAtRank(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal, identity, rank)
           ~> TimeoutEarlierServeRankGoal(
                 mode, source, sourceView,
                 ownerContext, ownerOrigin, ownerOrdinal, identity, rank)

TimeoutEarlierServeExactTicketConvergenceProperty(specification) ==
  specification
    => \A mode \in TimeoutRuntimeModeCarrier,
          source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          ownerContext \in ContextRecords,
          ownerOrigin \in AsyncCandidateCausalOriginSet,
          ownerOrdinal \in Nat \ {0},
          identity \in AsyncServeLogicalRequestIdentities:
         TimeoutEarlierServeExactTicketResidual(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal, identity)
           ~> TimeoutEarlierServeExactTicketGoal(
                 mode, source, sourceView,
                 ownerContext, ownerOrigin, ownerOrdinal, identity)

THEOREM TimeoutEarlierServeConcreteOwnerUsesAsyncFairness ==
  \A initialContext, source, ownerKind:
    /\ source \in AsyncCurrentResponsiveVoters
    /\ ownerKind \in TimeoutEarlierServeConcreteFairOwnerKinds
    => AsyncLiveSpecAt(initialContext)
         => WF_AsyncAllVars(
              TimeoutEarlierServeConcreteFairAction(source, ownerKind))
BY AsyncSpecAlwaysUsesFixedResponsiveVoters, Isa, PTL
   DEF TimeoutEarlierServeConcreteFairOwnerKinds,
       TimeoutEarlierServeConcreteFairAction,
       AsyncLiveSpecAt, AsyncSpecAt, AsyncFairnessAt

THEOREM TimeoutEarlierServeRankStepDerivesFiniteEpisodeClosure ==
  \A initialContext:
    TimeoutEarlierServeExactIngressRankStepProperty(
      AsyncLiveSpecAt(initialContext))
      => TimeoutEarlierServeFiniteEpisodeClosureProperty(
           AsyncLiveSpecAt(initialContext))
BY TimeoutEarlierServeConcreteOwnerUsesAsyncFairness,
   PTL, IsaT(600)
   DEF TimeoutEarlierServeExactIngressRankStepProperty,
       TimeoutEarlierServeConcreteActionOriginProperty,
       TimeoutEarlierServeFiniteEpisodeClosureProperty,
       TimeoutEarlierServeRankCellOutcome

THEOREM TimeoutEarlierServeFiniteEpisodeClosesRankCell ==
  \A initialContext:
    TimeoutEarlierServeExactIngressRankStepProperty(
      AsyncLiveSpecAt(initialContext))
      => TimeoutEarlierServeRankCellClosureProperty(
           AsyncLiveSpecAt(initialContext))
BY TimeoutEarlierServeRankStepDerivesFiniteEpisodeClosure,
   TimeoutEarlierServeEpisodeBudgetIsFinite,
   NatLessThanWellFounded, WellFoundedLeadsTo,
   AsyncSpecAlwaysStrongTypeInvariant, PTL
   DEF TimeoutEarlierServeFiniteEpisodeClosureProperty,
       TimeoutEarlierServeRankCellClosureProperty,
       TimeoutEarlierServeAtRankAndBudget,
       TimeoutEarlierServeAtRank,
       TimeoutEarlierServeRankGoal,
       TimeoutEarlierServeRankCellOutcome

THEOREM TimeoutEarlierServeRankStepClosesExactTicket ==
  \A initialContext:
    TimeoutEarlierServeExactIngressRankStepProperty(
      AsyncLiveSpecAt(initialContext))
      => TimeoutEarlierServeExactTicketConvergenceProperty(
           AsyncLiveSpecAt(initialContext))
BY TimeoutEarlierServeFiniteEpisodeClosesRankCell,
   TimeoutEarlierServeLifecycleRankOrderingIsWellFounded,
   TimeoutEarlierServeLifecycleRankInCarrier,
   WellFoundedLeadsTo,
   AsyncSpecAlwaysStrongTypeInvariant, PTL
   DEF TimeoutEarlierServeExactTicketConvergenceProperty,
       TimeoutEarlierServeRankCellClosureProperty,
       TimeoutEarlierServeAtRank,
       TimeoutEarlierServeRankGoal

TimeoutFrozenEarlierServePrefixAtBudget(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, budget) ==
  /\ TimeoutFrozenEarlierServePrefixPending(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal)
  /\ TimeoutFrozenEarlierServeTicketBudget(source, ownerOrdinal) = budget

TimeoutFrozenEarlierServeTicketIdentity(source, ownerOrdinal) ==
  CHOOSE identity \in
    TimeoutFrozenEarlierServeTicketIdentities(source, ownerOrdinal):
      \A other \in
        TimeoutFrozenEarlierServeTicketIdentities(source, ownerOrdinal):
        AsyncServeIngressAdmissionSchedulerOrdinal(source, identity)
          <= AsyncServeIngressAdmissionSchedulerOrdinal(source, other)

THEOREM TimeoutFrozenEarlierServeTicketIdentityIsOwned ==
  \A source, ownerOrdinal:
    TimeoutFrozenEarlierServeTicketIdentities(source, ownerOrdinal) # {}
      => TimeoutFrozenEarlierServeTicketIdentity(source, ownerOrdinal)
           \in TimeoutFrozenEarlierServeTicketIdentities(
                source, ownerOrdinal)
BY Isa DEF TimeoutFrozenEarlierServeTicketIdentity

THEOREM TimeoutEarlierServeTicketDepartureLowersFrozenPrefixBudget ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal, identity:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutFrozenEarlierServePrefixPending(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal)
    /\ identity \in
         TimeoutFrozenEarlierServeTicketIdentities(source, ownerOrdinal)
    /\ [AsyncNext]_AsyncAllVars
    /\ TimeoutFrozenEarlierServePrefixPending(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal)'
    /\ identity \notin
         TimeoutFrozenEarlierServeTicketIdentities(source, ownerOrdinal)'
    => TimeoutFrozenEarlierServeTicketBudget(source, ownerOrdinal)'
         < TimeoutFrozenEarlierServeTicketBudget(source, ownerOrdinal)
BY TimeoutFrozenEarlierServeTicketSetCannotReplenish,
   FS_CardinalityType, FS_Subset, IsaT(300)
   DEF TimeoutFrozenEarlierServeTicketBudget, AsyncAllVars

THEOREM TimeoutEarlierServeRankStepLowersFrozenPrefixBudget ==
  \A initialContext:
    TimeoutEarlierServeExactIngressRankStepProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A mode \in TimeoutRuntimeModeCarrier,
                  source \in AsyncCurrentResponsiveVoters,
                  sourceView \in Views,
                  ownerContext \in ContextRecords,
                  ownerOrigin \in AsyncCandidateCausalOriginSet,
                  ownerOrdinal \in Nat \ {0},
                  budget \in Nat:
                 TimeoutFrozenEarlierServePrefixAtBudget(
                   mode, source, sourceView,
                   ownerContext, ownerOrigin, ownerOrdinal, budget)
                   ~> (TimeoutFrozenEarlierServePrefixGoal(
                         mode, source, sourceView,
                         ownerContext, ownerOrigin, ownerOrdinal)
                        \/ \E lowerBudget \in
                             SetLessThan(
                               budget, OpToRel(<, Nat), Nat):
                             TimeoutFrozenEarlierServePrefixAtBudget(
                               mode, source, sourceView,
                               ownerContext, ownerOrigin,
                               ownerOrdinal, lowerBudget)))
BY TimeoutEarlierServeRankStepClosesExactTicket,
   TimeoutEarlierServeOwnedTicketHasExactLaneOccurrence,
   TimeoutFrozenEarlierServeTicketIdentityIsOwned,
   TimeoutEarlierServeTicketDepartureLowersFrozenPrefixBudget,
   TimeoutFrozenEarlierServeTicketSetCannotReplenish,
   AsyncSpecAlwaysStrongTypeInvariant, PTL, IsaT(600)
   DEF TimeoutEarlierServeExactTicketConvergenceProperty,
       TimeoutEarlierServeExactTicketResidual,
       TimeoutEarlierServeExactTicketGoal,
       TimeoutFrozenEarlierServePrefixAtBudget,
       TimeoutFrozenEarlierServePrefixGoal,
       TimeoutFrozenEarlierServeTicketIdentity,
       TimeoutFrozenEarlierServeTicketBudget

THEOREM TimeoutEarlierServeExactIngressRankStepClosesFrozenPrefix ==
  \A initialContext:
    TimeoutEarlierServeExactIngressRankStepProperty(
      AsyncLiveSpecAt(initialContext))
      => TimeoutFrozenEarlierServePrefixClosureProperty(
           AsyncLiveSpecAt(initialContext))
BY TimeoutEarlierServeRankStepLowersFrozenPrefixBudget,
   TimeoutFrozenEarlierServeTicketBudgetIsNatural,
   NatLessThanWellFounded, WellFoundedLeadsTo, PTL
   DEF TimeoutFrozenEarlierServePrefixClosureProperty,
       TimeoutFrozenEarlierServePrefixAtBudget,
       TimeoutFrozenEarlierServePrefixGoal

TimeoutFixedOwnerLaterServeTicket(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, identity) ==
  /\ TimeoutFrozenEarlierServePrefixPending(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal)
  /\ TimeoutFrozenEarlierServeTicketIdentities(
       source, ownerOrdinal) = {}
  /\ AsyncServeIngressLifecycleOwnerIdentities(source) # {}
  /\ identity = AsyncServeEarliestIngressSchedulerOwnerIdentity(source)
  /\ AsyncServeIngressAdmissionOwned(source, identity)
  /\ AsyncServeIngressAdmissionSchedulerOrdinal(source, identity)
       = AsyncServeEarliestIngressSchedulerOrdinal(source)
  /\ ownerOrdinal = AsyncTimeoutLifecycleOrdinal(source)
  /\ ownerOrdinal
       < AsyncServeEarliestIngressSchedulerOrdinal(source)
  /\ AsyncOlderRuntimeLifecyclePrecedesServeIngress(source)

TimeoutFixedOwnerLaterServeTicketAtRuntimeRank(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, identity, rank) ==
  /\ rank \in Nat
  /\ TimeoutFixedOwnerLaterServeTicket(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal, identity)
  /\ asyncRunnerPhase[source] = "Runtime"
  /\ RuntimeReachRank(source) = rank

TimeoutFixedOwnerLaterServeOvertakeAction(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal) ==
  \E identity:
    \E rank \in Nat:
      /\ TimeoutFixedOwnerLaterServeTicketAtRuntimeRank(
           mode, source, sourceView,
           ownerContext, ownerOrigin, ownerOrdinal, identity, rank)
      /\ PostGstRunNode(source)
      /\ AsyncServeIngressTargetOnlyTurn(source)

TimeoutFixedOwnerLaterServeInterleavingProperty(specification) ==
  specification
    => \A mode \in TimeoutRuntimeModeCarrier,
          source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          ownerContext \in ContextRecords,
          ownerOrigin \in AsyncCandidateCausalOriginSet,
          ownerOrdinal \in Nat \ {0}:
         /\ \A identity:
              \A rank \in Nat:
                [](TimeoutFixedOwnerLaterServeTicketAtRuntimeRank(
                     mode, source, sourceView,
                     ownerContext, ownerOrigin, ownerOrdinal,
                     identity, rank)
                     => ENABLED
                          <<PostGstRunNode(source)
                              /\ SerializedRuntimePrecedesServeIngressStep(
                                   source)>>_AsyncAllVars)
         /\ []
              [~TimeoutFixedOwnerLaterServeOvertakeAction(
                  mode, source, sourceView,
                  ownerContext, ownerOrigin, ownerOrdinal)]_AsyncAllVars

TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty(specification) ==
  /\ TimeoutEarlierServeExactIngressRankStepProperty(specification)
  /\ TimeoutFixedOwnerLaterServeInterleavingProperty(specification)

THEOREM TimeoutLaterServeTicketFairOccurrenceInterleavesExactRuntimeEpisode ==
  \A mode \in TimeoutRuntimeModeCarrier,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     ownerContext \in ContextRecords,
     ownerOrigin \in AsyncCandidateCausalOriginSet,
     ownerOrdinal \in Nat \ {0}:
    \A identity:
      \A rank \in Nat:
        /\ AsyncStrongTypeInvariant
        /\ TimeoutFixedOwnerLaterServeTicketAtRuntimeRank(
             mode, source, sourceView,
             ownerContext, ownerOrigin, ownerOrdinal, identity, rank)
        /\ PostGstRunNode(source)
        => /\ SerializedRuntimePrecedesServeIngressStep(source)
           /\ TimeoutRuntimeModeEndpoint(mode, source, sourceView)'
BY AsyncLaterServeTicketInterleavesOlderRuntimeEpisode,
   DirectTimeoutCreatesExactWalOrDeferredOwner,
   DeferredTimeoutOwnerCreatesExactPendingWalOwner,
   IsaT(600)
   DEF TimeoutFixedOwnerLaterServeTicketAtRuntimeRank,
       TimeoutFixedOwnerLaterServeTicket,
       TimeoutFrozenEarlierServePrefixPending,
       TimeoutRuntimeModeEndpoint,
       TimeoutRuntimeModeOwner,
       TimeoutRuntimeModeCarrier,
       TimeoutDeadlineArmedOwner,
       TimeoutDeferredRuntimeOwner,
       TimeoutRoundStable,
       SerializedRuntimePrecedesServeIngressStep,
       PostGstRunNode, RunNode, RunNodeWork,
       SerializedRuntimeStep, RuntimeStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       DeferredWorkOwnsRuntimeTurn,
       DeferredTagExecutable, DeferredTagStep,
       TimeoutDue, DirectTimeoutStep,
       DeferredTimeoutStep, DeferredTimeoutExecutable,
       TimeoutExactWalEndpoint, AsyncAllVars

THEOREM TimeoutFixedOwnerLaterServeTicketEnablesExactRuntimeEpisode ==
  \A mode \in TimeoutRuntimeModeCarrier,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     ownerContext \in ContextRecords,
     ownerOrigin \in AsyncCandidateCausalOriginSet,
     ownerOrdinal \in Nat \ {0}:
    \A identity:
      \A rank \in Nat:
        /\ AsyncStrongTypeInvariant
        /\ AsyncProgressOwnershipInvariant
        /\ TimeoutFixedOwnerLaterServeTicketAtRuntimeRank(
             mode, source, sourceView,
             ownerContext, ownerOrigin, ownerOrdinal, identity, rank)
        => ENABLED
             <<PostGstRunNode(source)
                 /\ SerializedRuntimePrecedesServeIngressStep(
                      source)>>_AsyncAllVars
BY ENABLEDaxioms, IsaT(900)
   DEF TimeoutFixedOwnerLaterServeTicketAtRuntimeRank,
       TimeoutFixedOwnerLaterServeTicket,
       TimeoutFrozenEarlierServePrefixPending,
       TimeoutRuntimeModeOwner,
       TimeoutRuntimeModeCarrier,
       TimeoutDeadlineArmedOwner,
       TimeoutDeferredRuntimeOwner,
       TimeoutRoundStable,
       PostGstRunNode, RunNode, RunNodeWork,
       AsyncOlderRuntimeLifecyclePrecedesServeIngress,
       AsyncOlderFrozenTimeoutLifecyclePrecedesServeIngress,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       RuntimeStep, DeferredWorkOwnsRuntimeTurn,
       DeferredTagExecutable, DeferredTagStep,
       TimeoutDue, DirectTimeoutStep,
       DeferredTimeoutStep, DeferredTimeoutExecutable,
       AsyncAllVars

THEOREM AsyncLiveClosesTimeoutFixedOwnerLaterServeInterleaving ==
  \A initialContext:
    TimeoutFixedOwnerLaterServeInterleavingProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   TimeoutFixedOwnerLaterServeTicketEnablesExactRuntimeEpisode,
   AsyncLaterServeTicketInterleavesOlderRuntimeEpisode,
   PTL, IsaT(900)
   DEF TimeoutFixedOwnerLaterServeInterleavingProperty,
       TimeoutFixedOwnerLaterServeOvertakeAction,
       TimeoutFixedOwnerLaterServeTicketAtRuntimeRank,
       TimeoutFixedOwnerLaterServeTicket,
       PostGstRunNode, RunNode,
       AsyncLiveSpecAt, AsyncAllVars

THEOREM AsyncLiveClosesTimeoutFixedOwnerPriorityTicketNonReplenishment ==
  \A initialContext:
    TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveClosesTimeoutEarlierServeExactIngressRankStep,
   AsyncLiveClosesTimeoutFixedOwnerLaterServeInterleaving
   DEF TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty

(***************************************************************************
Individual-runner temporal lift.

The earlier exact-ingress predecessor leaf first publishes a state in which
every active Serve ticket is later than the frozen timeout owner.  Local and
Ingress turns consume the finite `RuntimeReachRank`.  At Runtime the residual
forbids a target-only overtake and the source action theorem selects the exact
bounded serialized Runtime episode.  Only
`WF_AsyncAllVars(PostGstRunNode(source))` is used.
***************************************************************************)

TimeoutRuntimePriorityClearPending(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal) ==
  /\ TimeoutFrozenEarlierServePrefixPending(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal)
  /\ TimeoutFrozenEarlierServeTicketIdentities(
       source, ownerOrdinal) = {}

TimeoutRuntimePriorityClearAtRank(
    mode, source, sourceView,
    ownerContext, ownerOrigin, ownerOrdinal, rank) ==
  /\ rank \in Nat
  /\ TimeoutRuntimePriorityClearPending(
       mode, source, sourceView,
       ownerContext, ownerOrigin, ownerOrdinal)
  /\ ~TimeoutRuntimeModeActionOwner(mode, source, sourceView)
  /\ RuntimeReachRank(source) = rank

TimeoutRuntimePriorityClearGoal(mode, source, sourceView) ==
  \/ TimeoutRuntimeModeEndpoint(mode, source, sourceView)
  \/ TimeoutRuntimeModeActionOwner(mode, source, sourceView)

THEOREM TimeoutRuntimePriorityClearRankIsNatural ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutRuntimePriorityClearPending(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal)
    => RuntimeReachRank(source) \in Nat
BY RuntimeReachRankIsNatural, Isa
   DEF TimeoutRuntimePriorityClearPending,
       TimeoutFrozenEarlierServePrefixPending,
       TimeoutFrozenRuntimeLifecycleOwner,
       TimeoutRuntimeModeOwner,
       TimeoutDeadlineArmedOwner,
       TimeoutDeferredRuntimeOwner,
       TimeoutRoundStable

THEOREM TimeoutPriorityClearRunNodeStrictlyReachesModeAction ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal,
     rank \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ TimeoutRuntimePriorityClearAtRank(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, rank)
    /\ PostGstRunNode(source)
    /\ ~TimeoutFixedOwnerLaterServeOvertakeAction(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal)
    => \/ TimeoutRuntimePriorityClearGoal(
            mode, source, sourceView)'
       \/ \E lowerRank \in SetLessThan(rank, OpToRel(<, Nat), Nat):
            TimeoutRuntimePriorityClearAtRank(
              mode, source, sourceView,
              ownerContext, ownerOrigin, ownerOrdinal, lowerRank)'
BY AsyncLaterServeTicketInterleavesOlderRuntimeEpisode,
   TimeoutLaterServeTicketFairOccurrenceInterleavesExactRuntimeEpisode,
   IsaT(1200)
   DEF TimeoutRuntimePriorityClearAtRank,
       TimeoutRuntimePriorityClearPending,
       TimeoutRuntimePriorityClearGoal,
       TimeoutFixedOwnerLaterServeOvertakeAction,
       TimeoutFixedOwnerLaterServeTicketAtRuntimeRank,
       TimeoutFixedOwnerLaterServeTicket,
       TimeoutFrozenEarlierServeTicketIdentities,
       TimeoutRuntimeModeActionOwner,
       TimeoutDirectRuntimeActionOwner,
       TimeoutDeferredRuntimeActionOwner,
       TimeoutRuntimeModeEndpoint,
       TimeoutRuntimeModeOwner,
       TimeoutDeadlineArmedOwner,
       TimeoutDeferredRuntimeOwner,
       TimeoutRoundStable,
       PostGstRunNode, RunNode, RunNodeWork,
       AsyncServeIngressTargetOnlyTurn,
       AsyncOlderRuntimeLifecyclePrecedesServeIngress,
       AsyncServeEarliestIngressSchedulerOrdinal,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep, RuntimeStep,
       DeferredWorkOwnsRuntimeTurn,
       DeferredTagExecutable, DeferredTagStep,
       TimeoutDue, DirectTimeoutStep,
       DeferredTimeoutStep, DeferredTimeoutExecutable,
       AsyncTimeoutPriorityPrecedesCandidate,
       AsyncOlderCandidateLifecycleBlocksTimeout,
       RuntimeReachRank, AsyncAllVars,
       SetLessThan, OpToRel

THEOREM TimeoutPriorityClearRankCellIsSafe ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal,
     rank \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ TimeoutRuntimePriorityClearAtRank(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal, rank)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~TimeoutFixedOwnerLaterServeOvertakeAction(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal)
    => \/ TimeoutRuntimePriorityClearGoal(
            mode, source, sourceView)'
       \/ \E lowerRank \in SetLessThan(rank, OpToRel(<, Nat), Nat):
            TimeoutRuntimePriorityClearAtRank(
              mode, source, sourceView,
              ownerContext, ownerOrigin, ownerOrdinal, lowerRank)'
       \/ TimeoutRuntimePriorityClearAtRank(
            mode, source, sourceView,
            ownerContext, ownerOrigin, ownerOrdinal, rank)'
BY TimeoutFrozenOlderPredecessorSetCannotBeReplenished,
   TimeoutPriorityClearRunNodeStrictlyReachesModeAction,
   AsyncTimeoutLifecycleOrdinalPersistsUntilEndpoint,
   AsyncTimeoutLifecycleOrdinalClearsOnlyAtEndpoint,
   IsaT(1500)
   DEF TimeoutRuntimePriorityClearAtRank,
       TimeoutRuntimePriorityClearPending,
       TimeoutRuntimePriorityClearGoal,
       TimeoutFrozenEarlierServePrefixPending,
       TimeoutFrozenEarlierServeTicketIdentities,
       TimeoutRuntimeModeEndpoint,
       TimeoutRuntimeModeOwner,
       TimeoutFrozenRuntimeLifecycleOwner,
       RuntimeReachRank, AsyncAllVars,
       SetLessThan, OpToRel

THEOREM TimeoutPriorityClearSuffixReachesModeAction ==
  \A initialContext,
     mode \in TimeoutRuntimeModeCarrier,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     ownerContext \in ContextRecords,
     ownerOrigin \in AsyncCandidateCausalOriginSet,
     ownerOrdinal \in Nat \ {0}:
    /\ AsyncLiveSpecAt(initialContext)
    /\ TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty(
         AsyncLiveSpecAt(initialContext))
    => TimeoutRuntimePriorityClearPending(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal)
         ~> TimeoutRuntimePriorityClearGoal(mode, source, sourceView)
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecProvidesTimeoutRuntimeRunNodeFairness,
   TimeoutRuntimePriorityClearRankIsNatural,
   TimeoutPriorityClearRankCellIsSafe,
   TimeoutPriorityClearRunNodeStrictlyReachesModeAction,
   NatLessThanWellFounded, WellFoundedLeadsTo,
   PTL, Isa
   DEF TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty,
       TimeoutRuntimePriorityClearAtRank,
       TimeoutRuntimePriorityClearPending,
       TimeoutRuntimePriorityClearGoal,
       TimeoutRuntimeModeOwner,
       AsyncLiveSpecAt

THEOREM TimeoutModeFairOccurrenceReachesExactReducerEndpoint ==
  \A mode, source, sourceView:
    /\ mode \in TimeoutRuntimeModeCarrier
    /\ TimeoutRuntimeModeActionOwner(mode, source, sourceView)
    /\ PostGstRunNode(source)
    => TimeoutRuntimeModeEndpoint(mode, source, sourceView)'
BY DirectTimeoutCreatesExactWalOrDeferredOwner,
   DeferredTimeoutOwnerCreatesExactPendingWalOwner,
   AsyncLaterServeTicketInterleavesOlderRuntimeEpisode,
   IsaT(600)
   DEF TimeoutRuntimeModeCarrier,
       TimeoutRuntimeModeActionOwner,
       TimeoutRuntimeModeEndpoint,
       TimeoutDirectRuntimeActionOwner,
       TimeoutDeferredRuntimeActionOwner,
       TimeoutExactWalEndpoint,
       PostGstRunNode, RunNode, RunNodeWork,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       SerializedRuntimeStep, RuntimeStep,
       DirectTimeoutStep, DeferredTimeoutStep,
       AsyncAllVars

THEOREM TimeoutModeActionOwnerIsSafeUntilExactReducerEndpoint ==
  \A mode, source, sourceView,
     ownerContext, ownerOrigin, ownerOrdinal:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutRuntimePriorityClearPending(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal)
    /\ TimeoutRuntimeModeActionOwner(mode, source, sourceView)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~TimeoutFixedOwnerLaterServeOvertakeAction(
         mode, source, sourceView,
         ownerContext, ownerOrigin, ownerOrdinal)
    => \/ TimeoutRuntimeModeEndpoint(mode, source, sourceView)'
       \/ /\ TimeoutRuntimePriorityClearPending(
                mode, source, sourceView,
                ownerContext, ownerOrigin, ownerOrdinal)'
          /\ TimeoutRuntimeModeActionOwner(
               mode, source, sourceView)'
BY TimeoutModeFairOccurrenceReachesExactReducerEndpoint,
   AsyncTimeoutLifecycleOrdinalPersistsUntilEndpoint,
   AsyncTimeoutLifecycleOrdinalClearsOnlyAtEndpoint,
   TimeoutFrozenOlderPredecessorSetCannotBeReplenished,
   IsaT(1200)
   DEF TimeoutRuntimePriorityClearPending,
       TimeoutRuntimeModeActionOwner,
       TimeoutRuntimeModeEndpoint,
       TimeoutFrozenEarlierServePrefixPending,
       TimeoutFrozenEarlierServeTicketIdentities,
       TimeoutFrozenRuntimeLifecycleOwner,
       AsyncAllVars

THEOREM TimeoutPriorityClearSuffixConsumesExactModeAction ==
  \A initialContext,
     mode \in TimeoutRuntimeModeCarrier,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     ownerContext \in ContextRecords,
     ownerOrigin \in AsyncCandidateCausalOriginSet,
     ownerOrdinal \in Nat \ {0}:
    /\ AsyncLiveSpecAt(initialContext)
    /\ TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty(
         AsyncLiveSpecAt(initialContext))
    => (/\ TimeoutRuntimePriorityClearPending(
               mode, source, sourceView,
               ownerContext, ownerOrigin, ownerOrdinal)
         /\ TimeoutRuntimeModeActionOwner(mode, source, sourceView))
         ~> TimeoutRuntimeModeEndpoint(mode, source, sourceView)
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecProvidesTimeoutRuntimeRunNodeFairness,
   TimeoutModeActionOwnerIsSafeUntilExactReducerEndpoint,
   TimeoutModeFairOccurrenceReachesExactReducerEndpoint,
   PTL, Isa
   DEF TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty,
       TimeoutRuntimeModeOwner,
       TimeoutRuntimePriorityClearPending,
       AsyncLiveSpecAt

THEOREM TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesMode ==
  \A initialContext:
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty(
         AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A mode \in TimeoutRuntimeModeCarrier,
                  source \in AsyncCurrentResponsiveVoters,
                  sourceView \in Views:
                 TimeoutRuntimeModeOwner(mode, source, sourceView)
                   ~> TimeoutRuntimeModeEndpoint(
                        mode, source, sourceView))
BY AsyncLiveClosesTimeoutFrozenOlderCandidatePrefix,
   TimeoutEarlierServeExactIngressRankStepClosesFrozenPrefix,
   AsyncLiveClosesTimeoutFixedOwnerLaterServeInterleaving,
   TimeoutRuntimeModeOwnerHasExactFrozenLifecycleSnapshot,
   TimeoutPriorityClearSuffixReachesModeAction,
   TimeoutPriorityClearSuffixConsumesExactModeAction,
   PTL, IsaT(900)
   DEF TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty,
       TimeoutFrozenOlderCandidatePrefixClosureProperty,
       TimeoutFrozenOlderCandidatePrefixGoal,
       TimeoutFrozenEarlierServePrefixGoal,
       TimeoutRuntimePriorityClearPending,
       TimeoutRuntimePriorityClearGoal,
       TimeoutRuntimeModeEndpoint,
       AsyncLiveSpecAt

TimeoutArmedRuntimeSchedulerKernelProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views:
         TimeoutDeadlineArmedOwner(source, sourceView)
           ~> (TimeoutDirectGoal(source, sourceView)
                \/ TimeoutExactWalEndpoint(source, sourceView)
                \/ TimeoutDirectRuntimeActionOwner(source, sourceView)
                \/ TimeoutDeferredRuntimeOwner(source, sourceView))

TimeoutDirectRuntimeReducerWalKernelProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views:
         TimeoutDirectRuntimeActionOwner(source, sourceView)
           ~> (TimeoutDirectGoal(source, sourceView)
                \/ TimeoutExactWalEndpoint(source, sourceView)
                \/ TimeoutDeferredRuntimeOwner(source, sourceView))

TimeoutDeferredRuntimeSchedulerKernelProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views:
         TimeoutDeferredRuntimeOwner(source, sourceView)
           ~> (TimeoutDirectGoal(source, sourceView)
                \/ TimeoutExactWalEndpoint(source, sourceView)
                \/ TimeoutDeferredRuntimeActionOwner(
                     source, sourceView))

TimeoutDeferredRuntimeReducerWalKernelProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views:
         TimeoutDeferredRuntimeActionOwner(source, sourceView)
           ~> (TimeoutDirectGoal(source, sourceView)
                \/ TimeoutExactWalEndpoint(source, sourceView))

TimeoutArmedWalPhysicalKernelProperties(specification) ==
  /\ TimeoutArmedRuntimeSchedulerKernelProperty(specification)
  /\ TimeoutDirectRuntimeReducerWalKernelProperty(specification)
  /\ TimeoutDeferredRuntimeSchedulerKernelProperty(specification)
  /\ TimeoutDeferredRuntimeReducerWalKernelProperty(specification)

THEOREM TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesArmedScheduler ==
  \A initialContext:
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty(
         AsyncLiveSpecAt(initialContext))
      => TimeoutArmedRuntimeSchedulerKernelProperty(
           AsyncLiveSpecAt(initialContext))
BY TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesMode,
   PTL, Isa
   DEF TimeoutArmedRuntimeSchedulerKernelProperty,
       TimeoutRuntimeModeCarrier,
       TimeoutRuntimeModeOwner,
       TimeoutRuntimeModeEndpoint

THEOREM TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesDirectReducer ==
  \A initialContext:
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty(
         AsyncLiveSpecAt(initialContext))
      => TimeoutDirectRuntimeReducerWalKernelProperty(
           AsyncLiveSpecAt(initialContext))
BY TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesMode,
   PTL, Isa
   DEF TimeoutDirectRuntimeReducerWalKernelProperty,
       TimeoutRuntimeModeCarrier,
       TimeoutRuntimeModeOwner,
       TimeoutRuntimeModeEndpoint,
       TimeoutDirectRuntimeActionOwner

THEOREM TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesDeferredScheduler ==
  \A initialContext:
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty(
         AsyncLiveSpecAt(initialContext))
      => TimeoutDeferredRuntimeSchedulerKernelProperty(
           AsyncLiveSpecAt(initialContext))
BY TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesMode,
   PTL, Isa
   DEF TimeoutDeferredRuntimeSchedulerKernelProperty,
       TimeoutRuntimeModeCarrier,
       TimeoutRuntimeModeOwner,
       TimeoutRuntimeModeEndpoint

THEOREM TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesDeferredReducer ==
  \A initialContext:
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty(
         AsyncLiveSpecAt(initialContext))
      => TimeoutDeferredRuntimeReducerWalKernelProperty(
           AsyncLiveSpecAt(initialContext))
BY TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesMode,
   PTL, Isa
   DEF TimeoutDeferredRuntimeReducerWalKernelProperty,
       TimeoutRuntimeModeCarrier,
       TimeoutRuntimeModeOwner,
       TimeoutRuntimeModeEndpoint,
       TimeoutDeferredRuntimeActionOwner

THEOREM TimeoutFixedOwnerPriorityTicketNonReplenishmentDischargesArmedWalPhysicalKernels ==
  \A initialContext:
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ TimeoutFixedOwnerPriorityTicketNonReplenishmentProperty(
         AsyncLiveSpecAt(initialContext))
      => TimeoutArmedWalPhysicalKernelProperties(
           AsyncLiveSpecAt(initialContext))
BY TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesArmedScheduler,
   TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesDirectReducer,
   TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesDeferredScheduler,
   TimeoutFixedOwnerPriorityTicketNonReplenishmentClosesDeferredReducer
   DEF TimeoutArmedWalPhysicalKernelProperties

THEOREM TimeoutArmedWalPhysicalKernelsDischargeExactEndpoint ==
  \A specification:
    TimeoutArmedWalPhysicalKernelProperties(specification)
      => TimeoutArmedExactWalEndpointProperty(specification)
BY PTL
   DEF TimeoutArmedWalPhysicalKernelProperties,
       TimeoutArmedRuntimeSchedulerKernelProperty,
       TimeoutDirectRuntimeReducerWalKernelProperty,
       TimeoutDeferredRuntimeSchedulerKernelProperty,
       TimeoutDeferredRuntimeReducerWalKernelProperty,
       TimeoutArmedExactWalEndpointProperty,
       TimeoutExactWalEndpoint

THEOREM TimeoutArmedExactWalEndpointClosesRuntimePrefix ==
  \A specification:
    TimeoutArmedExactWalEndpointProperty(specification)
      => TimeoutArmedRuntimePrefixProperty(specification)
BY PTL
   DEF TimeoutArmedExactWalEndpointProperty,
       TimeoutArmedRuntimePrefixProperty,
       TimeoutPendingWalOwner, TimeoutOrigin

TimeoutSemanticOwnerHandoffProperty(specification) ==
  specification
    => /\ \A source \in AsyncCurrentResponsiveVoters,
              sourceView \in Views:
             TimeoutDeadlineArmedOwner(source, sourceView)
               ~> (TimeoutDirectGoal(source, sourceView)
                    \/ \E vote \in TimeoutVoteRecordSet:
                         TimeoutOrigin(source, sourceView, vote))
       /\ \A source \in AsyncCurrentResponsiveVoters,
              sourceView \in Views,
              vote \in TimeoutVoteRecordSet,
              recipient \in AsyncCurrentResponsiveVoters:
             (/\ gst
              /\ TimeoutOrigin(source, sourceView, vote))
               ~> TimeoutOriginOutcome(
                    source, sourceView, vote, recipient)

\* GST is explicit on the semantic continuation.  Before GST a responsive
\* crash may discard an unpersisted BeginTimeout request; the liveness
\* contract begins only after the advertised GST/recovery boundary.  The
\* exact PersistTimeout/SignTimeout continuation is discharged above.  The
\* derived Runtime prefix identifies the exact BeginTimeout successor after
\* every older lifecycle ordinal exits.  The four armed-Runtime physical
\* kernels above are discharged by the immutable priority-ticket theorem, not
\* retained as assumptions of the aggregate residual.

THEOREM TimeoutSemanticOwnerHandoffFromArmedRuntimePrefix ==
  \A initialContext:
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ TimeoutArmedRuntimePrefixProperty(
         AsyncLiveSpecAt(initialContext))
      => TimeoutSemanticOwnerHandoffProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveTimeoutConcreteOriginContinuation, PTL
   DEF TimeoutArmedRuntimePrefixProperty,
       TimeoutSemanticOwnerHandoffProperty,
       TimeoutConcreteOriginContinuationProperty

THEOREM TimeoutSemanticOwnerHandoffFromExactWalEndpoint ==
  \A initialContext:
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ TimeoutArmedExactWalEndpointProperty(
         AsyncLiveSpecAt(initialContext))
      => TimeoutSemanticOwnerHandoffProperty(
           AsyncLiveSpecAt(initialContext))
BY TimeoutArmedExactWalEndpointClosesRuntimePrefix,
   TimeoutSemanticOwnerHandoffFromArmedRuntimePrefix

(***************************************************************************
Exact reducer milestones below the remaining temporal corridors.

The transport and scheduler-origin prefixes must not be conflated with the
Core effects at their endpoints.  These predicates bind the complete wire or
certificate evidence before execution.  The lemmas then close the
product-independent action tails:

  * a current-view responsive TimeoutVote delivery records that exact vote;
  * exact TC delivery either exposes the exact local install owner or finds
    the target already decided;
  * BeginInstallTC installs the exact evidence-bound WAL owner and
    PersistInstallTC reaches the requested minimum view; and
  * exact CommitQC delivery records that certificate, after which
    BeginDecision exposes a target-local Decision WAL owner whose persistence
    reaches the target Decision.

Consequently, the residual properties below need only establish physical
delivery and causal scheduler origin up to these exact action boundaries.
***************************************************************************)

ExactTimeoutVoteDeliveryCommand(vote, recipient, command) ==
  /\ command.kind = "DeliverTimeout"
  /\ command.node = recipient
  /\ command.item = TimeoutVoteItem(vote, recipient)

ExactTimeoutCertificateDeliveryCommand(
    source, recipient, tc, command) ==
  /\ command.kind = "DeliverTC"
  /\ command.node = recipient
  /\ command.item = TimeoutCertificateItem(source, recipient, tc)

ExactCommitCertificateDeliveryCommand(
    source, target, qc, command) ==
  /\ command.kind = "DeliverQC"
  /\ command.node = target
  /\ command.item = CommitCertificateItem(source, target, qc)

ExactBeginInstallTcCommand(recipient, tc, command) ==
  /\ command.kind = "BeginInstallTC"
  /\ command.node = recipient
  /\ command.view = tc.view
  /\ InstallTcEvidenceMatches(command, tc)

ExactPersistInstallTcCommand(recipient, tc, command) ==
  /\ command.kind = "PersistInstallTC"
  /\ command.node = recipient
  /\ command.view = tc.view
  /\ InstallTcEvidenceMatches(command, tc)

TimeoutTargetDecisionWalOwner(target) ==
  \E request \in pendingDecision:
    /\ request.node = target
    /\ request.qc.context = context
    /\ request.qc.phase = "Commit"

TargetBeginDecisionCommand(target, command) ==
  /\ command.kind = "BeginDecision"
  /\ command.node = target

TargetPersistDecisionCommand(target, command) ==
  /\ command.kind = "PersistDecision"
  /\ command.node = target

THEOREM ExecuteExactCurrentViewTimeoutDeliveryRecordsExactReceipt ==
  \A vote \in TimeoutVoteRecordSet,
     recipient \in AsyncCurrentResponsiveVoters,
     command:
    /\ AsyncStrongTypeInvariant
    /\ vote.signer \in AsyncCurrentResponsiveVoters
    /\ vote \in timeoutIntents
    /\ nodeView[recipient] = vote.view
    /\ ExactTimeoutVoteDeliveryCommand(vote, recipient, command)
    /\ ExecuteCoreDelivery(command)
    => TimeoutReceipt(vote, recipient)'
BY ExecuteCoreTimeoutDeliveryRecordsReceipt, IsaT(360)
   DEF ExactTimeoutVoteDeliveryCommand,
       TimeoutReceipt, ReceivedTimeoutVoteAt,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, ReducerProvenanceInvariant,
       HonestTimeoutTransportBacked,
       HonestTimeoutUniqueness, HonestTimeoutUnique,
       SameTimeoutSlot, SameTimeoutContent,
       TimeoutVoteAt, TimeoutVoteItem,
       AsyncNetworkItem, TimeoutEnvelope,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM ExecuteTimeoutFormingReducerCreatesExactInstallOwner ==
  \A command:
    (\/ /\ ExecuteSignTimeout(command)
           /\ SignTimeoutFormsTC(command)
     \/ /\ ExecuteCoreDelivery(command)
           /\ DeliverTimeoutFormsTC(command))
      => LET tc == ExactFormedTcForTimeoutCommand(command)
         IN /\ TimeoutCertificateSemanticIdentity(tc, tc.view)
            /\ TimeoutCertificateInstallOwner(command.node, tc)'
BY IsaT(300)
   DEF ExecuteSignTimeout, ExecuteCoreDelivery,
       CompleteTimeoutSignature, DeliverTimeout,
       SignTimeoutFormsTC, SignTimeoutRequests,
       DeliverTimeoutFormsTC,
       ExactFormedTcForTimeoutCommand,
       TimeoutCertificateInstallOwner,
       TimeoutCertificateSemanticIdentity,
       TimeoutCertificateAfterReceipt,
       TimeoutInstallRequestAfterReceipt,
       TimeoutReceiptFormsTC, TimeoutReceiptsAfter,
       TimeoutReceiptAdmitted, InstallTcWal

THEOREM ExecuteExactTimeoutCertificateDeliveryCreatesInstallOrGoal ==
  \A source, recipient, tc, minimumView, command:
    /\ TimeoutCertificateSemanticIdentity(tc, minimumView)
    /\ ExactTimeoutCertificateDeliveryCommand(
         source, recipient, tc, command)
    /\ ExecuteCoreDelivery(command)
    => \/ TimeoutCertificateInstallOwner(recipient, tc)'
       \/ TimeoutViewGoal(recipient, minimumView)'
BY IsaT(180)
   DEF ExactTimeoutCertificateDeliveryCommand,
       TimeoutCertificateItem, AsyncNetworkItem, TcEnvelope,
       ExecuteCoreDelivery, DeliverTC,
       TimeoutCertificateInstallOwner,
       TimeoutCertificateSemanticIdentity,
       TimeoutViewGoal, NodeHasDecision, NoDecisionForNode, TcAt

THEOREM ExecuteExactCommitCertificateDeliveryRecordsExactReceipt ==
  \A source, target, qc, command:
    /\ qc.phase = "Commit"
    /\ ExactCommitCertificateDeliveryCommand(
         source, target, qc, command)
    /\ ExecuteCoreDelivery(command)
    => QcAt(target, qc) \in receivedQCs'
BY Isa
   DEF ExactCommitCertificateDeliveryCommand,
       CommitCertificateItem, AsyncNetworkItem, QcEnvelope,
       ExecuteCoreDelivery, DeliverQC,
       QcDeliveryCreatesReceipt, QcAt

THEOREM ExecuteExactBeginInstallCreatesExactWalOwner ==
  \A recipient, tc, command:
    /\ ExactBeginInstallTcCommand(recipient, tc, command)
    /\ ExecuteRegularCommand(command)
    => TimeoutCertificateInstallOwner(recipient, tc)'
BY IsaT(240)
   DEF ExactBeginInstallTcCommand,
       ExecuteRegularCommand, RegularCoreCommand,
       InstallTcEvidenceMatches, BeginInstallTC,
       TimeoutCertificateInstallOwner, InstallTcWal,
       ReceivedTcValues

THEOREM ExecuteExactPersistInstallReachesMinimumView ==
  \A recipient, tc, minimumView, command:
    /\ TypeInvariant
    /\ TimeoutCertificateSemanticIdentity(tc, minimumView)
    /\ ExactPersistInstallTcCommand(recipient, tc, command)
    /\ ExecutePersistInstall(command)
    => TimeoutViewGoal(recipient, minimumView)'
BY ExecutePersistInstallAdvancesCertifiedView, Isa
   DEF ExactPersistInstallTcCommand,
       TimeoutCertificateSemanticIdentity, TimeoutViewGoal

THEOREM ExecuteTargetBeginDecisionCreatesWalOwner ==
  \A target, command:
    /\ TargetBeginDecisionCommand(target, command)
    /\ ExecuteRegularCommand(command)
    => TimeoutTargetDecisionWalOwner(target)'
BY IsaT(180)
   DEF TargetBeginDecisionCommand,
       TimeoutTargetDecisionWalOwner,
       ExecuteRegularCommand, RegularCoreCommand,
       BeginDecision, DecisionWal, ReceivedQcValues

THEOREM ExecuteTargetPersistDecisionReachesDecision ==
  \A target, command:
    /\ TargetPersistDecisionCommand(target, command)
    /\ ExecutePersistDecision(command)
    => NodeHasDecision(target)'
BY Isa
   DEF TargetPersistDecisionCommand,
       ExecutePersistDecision, CommandMatches,
       PersistDecision, NodeHasDecision

THEOREM RetiredStandaloneFormTcActionIsDisabled ==
  \A node, roundView: ~FormTC(node, roundView)
BY DEF FormTC

(***************************************************************************
Monotone timeout-rank substrate.

The catch-up and receipt coordinates are cardinalities, but their temporal
meaning is the underlying frozen debt set.  After GST node views cannot move
backward and timeout receipts cannot be removed: the only Core action which
filters a receipt pool is Crash, and every asynchronous crash arm is guarded
by `~gst`.  The two subset theorems below are therefore the replacement-free
facts needed by the cardinality rank.  They rule out a same-count owner swap;
the selected source/signer can leave only by consuming a member of the
frozen debt set or by exposing the target's exact TC/Decision frontier.
***************************************************************************)

THEOREM PostGstAsyncBracketAdvancesEveryNodeView ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ [AsyncNext]_AsyncAllVars
  => \A node \in ValidatorIds:
       nodeView'[node] >= nodeView[node]
BY AsyncNextAdvancesNodeViews, Isa
   DEF AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars, vars

THEOREM PostGstAsyncBracketRetainsEveryTimeoutReceipt ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ [AsyncNext]_AsyncAllVars
  => receivedTimeoutVotes \subseteq receivedTimeoutVotes'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              gst,
              [AsyncNext]_AsyncAllVars
         PROVE receivedTimeoutVotes \subseteq receivedTimeoutVotes'
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <2>1 DEF AsyncAllVars, AsyncSchedulerVars, vars
    <2>2. CASE AsyncNext
      <3>1. ~\E node \in ValidatorIds: Crash(node)
        BY <1>1, <2>2, Isa
           DEF AsyncNext, AsyncNonCrashStep,
               PreGstCrash, PreGstResponsiveCrash
      <3>2. CASE receivedTimeoutVotes' = receivedTimeoutVotes
        BY <3>2
      <3>3. CASE receivedTimeoutVotes' # receivedTimeoutVotes
        <4>1. \E command: ExecuteCommand(command)
          BY <2>2, <3>1, <3>3,
             ChangedAsyncNextExecutesCommandOrCrashes
        <4>2. ASSUME NEW command, ExecuteCommand(command)
               PROVE receivedTimeoutVotes
                       \subseteq receivedTimeoutVotes'
          <5>1. ExecuteCoreDelivery(command)
            BY <3>3, <4>2,
               ChangedExecuteCommandIsCoreTimeoutDelivery
          <5>2. \E envelope: DeliverTimeout(envelope)
            BY <3>3, <5>1, Isa
               DEF ExecuteCoreDelivery, DeliverProposal,
                   DeliverVote, DeliverQC, DeliverTC
          <5> QED BY <5>2, Isa DEF DeliverTimeout
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>2, <3>3
    <2> QED BY <1>1, <2>1, <2>2, Isa
  <1> QED BY <1>1

THEOREM PostGstTimeoutCatchupDebtSlotsCannotGrow ==
  \A roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => TimeoutCatchupDebtSlots(roundView)'
         \subseteq TimeoutCatchupDebtSlots(roundView)
BY PostGstAsyncBracketAdvancesEveryNodeView, Isa
   DEF TimeoutCatchupDebtSlots,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       AsyncAllVars, vars

THEOREM PostGstTimeoutMissingReceiptVotersCannotGrow ==
  \A target \in ValidatorIds, roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => TimeoutMissingReceiptVoters(target, roundView)'
         \subseteq TimeoutMissingReceiptVoters(target, roundView)
BY PostGstAsyncBracketRetainsEveryTimeoutReceipt, Isa
   DEF TimeoutMissingReceiptVoters, ReceivedTimeoutVoteAt,
       TimeoutVoteAt, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, AsyncAllVars, vars

THEOREM PostGstCatchupDebtRankCannotIncrease ==
  \A roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => TimeoutCatchupDebtRank(roundView)'
         <= TimeoutCatchupDebtRank(roundView)
BY PostGstTimeoutCatchupDebtSlotsCannotGrow,
   TimeoutCatchupDebtRankIsNatural,
   FS_Subset, FS_CardinalityType
   DEF TimeoutCatchupDebtRank

THEOREM PostGstMissingReceiptRankCannotIncrease ==
  \A target \in ValidatorIds, roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => TimeoutMissingReceiptRank(target, roundView)'
         <= TimeoutMissingReceiptRank(target, roundView)
BY PostGstTimeoutMissingReceiptVotersCannotGrow,
   TimeoutMissingRanksAreNatural,
   FS_Subset, FS_CardinalityType,
   AsyncStrongTypeProjectsAsyncType
   DEF TimeoutMissingReceiptRank

TimeoutCatchupSourcePendingAtRank(
    target, roundView, sourceRank, laggingSource) ==
  /\ TimeoutProgressRankFrontier(
       target, roundView, sourceRank)
  /\ sourceRank[1] = 2
  /\ laggingSource \in AsyncCurrentResponsiveVoters
  /\ nodeView[laggingSource] < roundView

TimeoutReceiptSignerPendingAtRank(
    target, roundView, sourceRank, signer) ==
  /\ TimeoutProgressRankFrontier(
       target, roundView, sourceRank)
  /\ sourceRank[1] = 1
  /\ signer \in TimeoutMissingReceiptVoters(target, roundView)

THEOREM TimeoutCatchupDebtZeroEntersReceiptStage ==
  \A target \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutCatchupDebtAtRank(target, roundView, 0)
    => \E receiptRank \in Nat:
         /\ <<1, receiptRank>>
              \in SetLessThan(
                   <<2, 0>>,
                   TimeoutProgressRankOrdering,
                   TimeoutProgressRankCarrier)
         /\ TimeoutProgressRankFrontier(
              target, roundView, <<1, receiptRank>>)
BY TimeoutMissingRanksAreNatural, FS_EmptySet,
   FS_CardinalityType, IsaT(240)
   DEF TimeoutCatchupDebtAtRank, TimeoutCatchupDebtRank,
       TimeoutCatchupDebtSlots, TimeoutReceiptAtRank,
       TimeoutProgressRankFrontier,
       TimeoutProgressRankOrdering, TimeoutProgressRankCarrier,
       LexPairOrdering, SetLessThan, OpToRel,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, Views

THEOREM TimeoutPositiveCatchupDebtSelectsLaggingSource ==
  \A target \in AsyncCurrentResponsiveVoters,
     roundView \in Views,
     rank \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ rank > 0
    /\ TimeoutCatchupDebtAtRank(target, roundView, rank)
    => \E laggingSource \in AsyncCurrentResponsiveVoters:
         nodeView[laggingSource] < roundView
BY FS_EmptySet, FS_CardinalityType, IsaT(240)
   DEF TimeoutCatchupDebtAtRank, TimeoutCatchupDebtRank,
       TimeoutCatchupDebtSlots,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, Views

THEOREM TimeoutReceiptDebtZeroExposesExactFormationFrontier ==
  \A target \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ TimeoutReceiptAtRank(target, roundView, 0)
    => TimeoutCertificateFormationFrontier(target, roundView)
BY FS_EmptySet, FS_CardinalityType, IsaT(240)
   DEF TimeoutReceiptAtRank,
       TimeoutMissingReceiptRank, TimeoutMissingReceiptVoters,
       ResponsiveTimeoutReceiptQuorumAt, ReceivedTimeoutVoteAt,
       TimeoutViewOwnershipInvariant,
       TimeoutCertificateFormationFrontier

THEOREM TimeoutPositiveReceiptDebtSelectsMissingSigner ==
  \A target \in AsyncCurrentResponsiveVoters,
     roundView \in Views,
     rank \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ rank > 0
    /\ TimeoutReceiptAtRank(target, roundView, rank)
    => \E signer \in AsyncCurrentResponsiveVoters:
         ~ReceivedTimeoutVoteAt(target, signer, roundView)
BY FS_EmptySet, FS_CardinalityType, IsaT(180)
   DEF TimeoutReceiptAtRank,
       TimeoutMissingReceiptRank, TimeoutMissingReceiptVoters

THEOREM TimeoutResponsiveSourceGoalSuppliesTargetOwner ==
  \A target, source \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ TimeoutRoundStable(target, roundView)
    /\ TimeoutViewGoal(source, roundView)
    => \/ TimeoutDirectGoal(target, roundView)
       \/ TimeoutDirectOwnerFrontier(target, roundView)
BY ResponsiveAuthoritySuppliesEveryTcFrontier, IsaT(360)
   DEF TimeoutDirectGoal, TimeoutDirectOwnerFrontier,
       TimeoutViewGoal, TimeoutRoundStable,
       TimeoutViewOwnershipInvariant,
       ResponsiveViewCertificateAuthority,
       DecisionPropagationFrontier, DecisionSourceAt,
       NodeHasDecision, TcFrontier,
       TimeoutCertificateSemanticIdentity,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, Views

THEOREM TimeoutDominatedExactVoteSuppliesTargetOwner ==
  \A target, signer \in AsyncCurrentResponsiveVoters,
     roundView \in Views,
     vote \in TimeoutVoteRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ TimeoutRoundStable(target, roundView)
    /\ TimeoutVoteSemanticIdentity(signer, roundView, vote)
    /\ TimeoutSourceDominated(vote)
    => \/ TimeoutDirectGoal(target, roundView)
       \/ TimeoutDirectOwnerFrontier(target, roundView)
BY TimeoutResponsiveSourceGoalSuppliesTargetOwner, Isa
   DEF TimeoutSourceDominated, TimeoutViewGoal,
       TimeoutVoteSemanticIdentity

THEOREM TimeoutCatchupPendingPersistsOrStrictlyDescends ==
  \A target \in AsyncCurrentResponsiveVoters,
     roundView \in Views,
     sourceRank \in TimeoutProgressRankCarrier,
     laggingSource \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ TimeoutViewOwnershipInvariant'
    /\ TimeoutCatchupSourcePendingAtRank(
         target, roundView, sourceRank, laggingSource)
    /\ [AsyncNext]_AsyncAllVars
    => \/ TimeoutProgressRankStrictGoal(
            target, roundView, sourceRank)'
       \/ TimeoutCatchupSourcePendingAtRank(
            target, roundView, sourceRank, laggingSource)'
BY PostGstCatchupDebtRankCannotIncrease,
   TimeoutResponsiveSourceGoalSuppliesTargetOwner,
   TimeoutCatchupDebtRankIsNatural,
   FS_Subset, FS_RemoveElement, FS_CardinalityType,
   IsaT(900)
   DEF TimeoutCatchupSourcePendingAtRank,
       TimeoutProgressRankStrictGoal,
       TimeoutProgressRankFrontier,
       TimeoutCatchupDebtAtRank, TimeoutCatchupDebtRank,
       TimeoutCatchupDebtSlots,
       TimeoutProgressRankOrdering, TimeoutProgressRankCarrier,
       TimeoutDirectGoal, TimeoutDirectOwnerFrontier,
       TimeoutRoundStable, ResponsiveDecisionExists,
       SetLessThan, LexPairOrdering, OpToRel

THEOREM TimeoutReceiptPendingPersistsOrStrictlyDescends ==
  \A target \in AsyncCurrentResponsiveVoters,
     roundView \in Views,
     sourceRank \in TimeoutProgressRankCarrier,
     signer \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ TimeoutViewOwnershipInvariant'
    /\ TimeoutReceiptSignerPendingAtRank(
         target, roundView, sourceRank, signer)
    /\ [AsyncNext]_AsyncAllVars
    => \/ TimeoutProgressRankStrictGoal(
            target, roundView, sourceRank)'
       \/ TimeoutReceiptSignerPendingAtRank(
            target, roundView, sourceRank, signer)'
BY PostGstMissingReceiptRankCannotIncrease,
   TimeoutResponsiveSourceGoalSuppliesTargetOwner,
   FS_Subset, FS_RemoveElement, FS_CardinalityType,
   IsaT(900)
   DEF TimeoutReceiptSignerPendingAtRank,
       TimeoutProgressRankStrictGoal,
       TimeoutProgressRankFrontier, TimeoutReceiptAtRank,
       TimeoutMissingReceiptRank, TimeoutMissingReceiptVoters,
       TimeoutProgressRankOrdering, TimeoutProgressRankCarrier,
       TimeoutDirectGoal, TimeoutDirectOwnerFrontier,
       TimeoutRoundStable, ResponsiveDecisionExists,
       ReceivedTimeoutVoteAt, SetLessThan,
       LexPairOrdering, OpToRel

TimeoutSourceIsolatedDeliveryConvergenceProperty(specification) ==
  specification
    => \A vote \in TimeoutVoteRecordSet,
          recipient \in AsyncCurrentResponsiveVoters:
         (/\ gst
          /\ vote.signer \in AsyncCurrentResponsiveVoters
          /\ TimeoutDelivery(vote, recipient))
           ~> TimeoutDeliveryOutcome(vote, recipient)

(***************************************************************************
Source-isolated TimeoutVote physical kernels.

Both guards are part of the declared liveness contract.  Packet loss is
permitted before GST, and a Byzantine ephemeral TimeoutVote has no durable
retransmission owner.  Every predicate below retains the complete vote and
recipient; traffic or backpressure from another authenticated source cannot
be used as its successor.  The final kernel ends at the exact reducer
candidate, whose successful local effect is already proved by
`ExecuteExactCurrentViewTimeoutDeliveryRecordsExactReceipt`.
***************************************************************************)

TimeoutVoteDeliveryKernelSource(vote, recipient) ==
  /\ gst
  /\ vote.signer \in AsyncCurrentResponsiveVoters
  /\ recipient \in AsyncCurrentResponsiveVoters
  /\ TimeoutVoteSemanticIdentity(vote.signer, vote.view, vote)

TimeoutVoteRetainedControlOwner(vote, recipient) ==
  LET item == TimeoutVoteItem(vote, recipient)
  IN /\ TimeoutVoteDeliveryKernelSource(vote, recipient)
     /\ item \in asyncRetainedControl

TimeoutVotePacketOwner(vote, recipient) ==
  LET item == TimeoutVoteItem(vote, recipient)
  IN /\ TimeoutVoteDeliveryKernelSource(vote, recipient)
     /\ ExactPacketOwns(item)

TimeoutVoteIngressOwner(vote, recipient) ==
  LET item == TimeoutVoteItem(vote, recipient)
  IN /\ TimeoutVoteDeliveryKernelSource(vote, recipient)
     /\ ExactIngressOwns(item)

TimeoutVoteReducerCandidateOwner(vote, recipient) ==
  LET item == TimeoutVoteItem(vote, recipient)
  IN /\ TimeoutVoteDeliveryKernelSource(vote, recipient)
     /\ ExactDeliveryCandidateOwns(item)

TimeoutVoteRetainedControlKernelProperty(specification) ==
  specification
    => \A vote \in TimeoutVoteRecordSet,
          recipient \in AsyncCurrentResponsiveVoters:
         TimeoutVoteRetainedControlOwner(vote, recipient)
           ~> (TimeoutDeliveryOutcome(vote, recipient)
                \/ TimeoutVotePacketOwner(vote, recipient)
                \/ TimeoutVoteIngressOwner(vote, recipient)
                \/ TimeoutVoteReducerCandidateOwner(vote, recipient))

TimeoutVotePacketKernelProperty(specification) ==
  specification
    => \A vote \in TimeoutVoteRecordSet,
          recipient \in AsyncCurrentResponsiveVoters:
         TimeoutVotePacketOwner(vote, recipient)
           ~> (TimeoutDeliveryOutcome(vote, recipient)
                \/ TimeoutVoteIngressOwner(vote, recipient)
                \/ TimeoutVoteReducerCandidateOwner(vote, recipient))

TimeoutVoteIngressKernelProperty(specification) ==
  specification
    => \A vote \in TimeoutVoteRecordSet,
          recipient \in AsyncCurrentResponsiveVoters:
         TimeoutVoteIngressOwner(vote, recipient)
           ~> (TimeoutDeliveryOutcome(vote, recipient)
                \/ TimeoutVoteReducerCandidateOwner(vote, recipient))

TimeoutVoteReducerCandidateKernelProperty(specification) ==
  specification
    => \A vote \in TimeoutVoteRecordSet,
          recipient \in AsyncCurrentResponsiveVoters:
         TimeoutVoteReducerCandidateOwner(vote, recipient)
           ~> TimeoutDeliveryOutcome(vote, recipient)

TimeoutVoteDeliveryPhysicalKernelProperties(specification) ==
  /\ TimeoutVoteRetainedControlKernelProperty(specification)
  /\ TimeoutVotePacketKernelProperty(specification)
  /\ TimeoutVoteIngressKernelProperty(specification)
  /\ TimeoutVoteReducerCandidateKernelProperty(specification)

THEOREM TimeoutVotePhysicalKernelsDischargeSourceIsolatedDelivery ==
  \A specification:
    TimeoutVoteDeliveryPhysicalKernelProperties(specification)
      => TimeoutSourceIsolatedDeliveryConvergenceProperty(specification)
BY Isa, PTL
   DEF TimeoutVoteDeliveryPhysicalKernelProperties,
       TimeoutVoteRetainedControlKernelProperty,
       TimeoutVotePacketKernelProperty,
       TimeoutVoteIngressKernelProperty,
       TimeoutVoteReducerCandidateKernelProperty,
       TimeoutSourceIsolatedDeliveryConvergenceProperty,
       TimeoutVoteRetainedControlOwner,
       TimeoutVotePacketOwner,
       TimeoutVoteIngressOwner,
       TimeoutVoteReducerCandidateOwner,
       TimeoutVoteDeliveryKernelSource,
       TimeoutDelivery

TimeoutFiniteRankDescentProperty(specification) ==
  (/\ TimeoutDeadlineClockConvergenceProperty(specification)
   /\ TimeoutSemanticOwnerHandoffProperty(specification)
   /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(specification))
    => (specification
          => \A target \in AsyncCurrentResponsiveVoters,
                roundView \in Views,
                rank \in TimeoutProgressRankCarrier:
               TimeoutProgressRankFrontier(target, roundView, rank)
                 ~> (TimeoutDirectGoal(target, roundView)
                      \/ TimeoutDirectOwnerFrontier(target, roundView)
                      \/ \E lowerRank \in
                             SetLessThan(
                               rank,
                               TimeoutProgressRankOrdering,
                               TimeoutProgressRankCarrier):
                           TimeoutProgressRankFrontier(
                             target, roundView, lowerRank)))

\* Catch-up deliberately consumes exact TC convergence in the closure theorem
\* below.  Leaving that dependency out would be unsound: timeout-vote delivery
\* alone does not service the retained previous-view TC owned by the target.
\* The aggregate residual already carries the exact certificate physical
\* kernels which derive that property, so this is a dependency edge rather
\* than an additional fairness assumption.

TimeoutCertificateAndDecisionConvergenceProperty(specification) ==
  specification
    => \A target \in AsyncCurrentResponsiveVoters,
          roundView \in Views:
         /\ DecisionPropagationFrontier(target)
              ~> NodeHasDecision(target)
         /\ TcFrontier(target, roundView)
              ~> TimeoutDirectGoal(target, roundView)
         /\ TimeoutCertificateFormationFrontier(target, roundView)
              ~> TimeoutDirectGoal(target, roundView)

(***************************************************************************
Exact TC transport and reducer/WAL kernels.

These predicates keep the complete certificate and requested minimum view.
In particular, another target's received TC or a global formed certificate
cannot satisfy any stage.  The action-local reducer effects were proved
above; the properties here isolate only the temporal selection of their
retained-control, packet, ingress, delivery-candidate, received, and WAL
owners.
***************************************************************************)

TimeoutTcKernelSource(source, target, tc, minimumView) ==
  /\ source \in AsyncCurrentResponsiveVoters
  /\ target \in AsyncCurrentResponsiveVoters
  /\ TimeoutCertificateSemanticIdentity(tc, minimumView)

TimeoutTcRetainedControlOwner(source, target, tc, minimumView) ==
  LET item == TimeoutCertificateItem(source, target, tc)
  IN /\ TimeoutTcKernelSource(source, target, tc, minimumView)
     /\ item \in asyncRetainedControl

TimeoutTcPacketOwner(source, target, tc, minimumView) ==
  LET item == TimeoutCertificateItem(source, target, tc)
  IN /\ TimeoutTcKernelSource(source, target, tc, minimumView)
     /\ ExactPacketOwns(item)

TimeoutTcIngressOwner(source, target, tc, minimumView) ==
  LET item == TimeoutCertificateItem(source, target, tc)
  IN /\ TimeoutTcKernelSource(source, target, tc, minimumView)
     /\ ExactIngressOwns(item)

TimeoutTcReducerCandidateOwner(source, target, tc, minimumView) ==
  LET item == TimeoutCertificateItem(source, target, tc)
  IN /\ TimeoutTcKernelSource(source, target, tc, minimumView)
     /\ ExactDeliveryCandidateOwns(item)

TimeoutTcReceivedReducerOwner(target, tc, minimumView) ==
  /\ target \in AsyncCurrentResponsiveVoters
  /\ TimeoutCertificateSemanticIdentity(tc, minimumView)
  /\ TcAt(target, tc) \in receivedTCs

TimeoutTcInstallWalOwner(target, tc, minimumView) ==
  /\ target \in AsyncCurrentResponsiveVoters
  /\ TimeoutCertificateSemanticIdentity(tc, minimumView)
  /\ \E request \in pendingInstallTC:
       /\ request.node = target
       /\ request.tc = tc

TimeoutTcDeliveryTerminalOwner(target, tc, minimumView) ==
  \/ TimeoutDirectGoal(target, minimumView)
  \/ TimeoutTcReceivedReducerOwner(target, tc, minimumView)
  \/ TimeoutTcInstallWalOwner(target, tc, minimumView)

TimeoutTcRetainedControlKernelProperty(specification) ==
  specification
    => \A source, target, tc, minimumView:
         TimeoutTcRetainedControlOwner(
           source, target, tc, minimumView)
           ~> (TimeoutTcDeliveryTerminalOwner(
                 target, tc, minimumView)
                \/ TimeoutTcPacketOwner(
                     source, target, tc, minimumView)
                \/ TimeoutTcIngressOwner(
                     source, target, tc, minimumView)
                \/ TimeoutTcReducerCandidateOwner(
                     source, target, tc, minimumView))

TimeoutTcPacketKernelProperty(specification) ==
  specification
    => \A source, target, tc, minimumView:
         TimeoutTcPacketOwner(source, target, tc, minimumView)
           ~> (TimeoutTcDeliveryTerminalOwner(
                 target, tc, minimumView)
                \/ TimeoutTcIngressOwner(
                     source, target, tc, minimumView)
                \/ TimeoutTcReducerCandidateOwner(
                     source, target, tc, minimumView))

TimeoutTcIngressKernelProperty(specification) ==
  specification
    => \A source, target, tc, minimumView:
         TimeoutTcIngressOwner(source, target, tc, minimumView)
           ~> (TimeoutTcDeliveryTerminalOwner(
                 target, tc, minimumView)
                \/ TimeoutTcReducerCandidateOwner(
                     source, target, tc, minimumView))

TimeoutTcDeliveryCandidateKernelProperty(specification) ==
  specification
    => \A source, target, tc, minimumView:
         TimeoutTcReducerCandidateOwner(
           source, target, tc, minimumView)
           ~> TimeoutTcDeliveryTerminalOwner(
                target, tc, minimumView)

TimeoutTcReceivedReducerKernelProperty(specification) ==
  specification
    => \A target, tc, minimumView:
         TimeoutTcReceivedReducerOwner(target, tc, minimumView)
           ~> (TimeoutDirectGoal(target, minimumView)
                \/ TimeoutTcInstallWalOwner(
                     target, tc, minimumView))

TimeoutTcInstallWalKernelProperty(specification) ==
  specification
    => \A target, tc, minimumView:
         TimeoutTcInstallWalOwner(target, tc, minimumView)
           ~> TimeoutDirectGoal(target, minimumView)

(***************************************************************************
The install-WAL leaf uses the same protected Busy service as other durable
completions.  Checked overflow remains fail-closed in the transition relation,
but `InstallRankInvariantExcludesGenerationExhaustion` proves that a valid
same-round upgrade cannot reach it: every increment consumes a strictly higher
Prepare rank in the bounded timed-out view.  No finite-counter liveness premise
is used here.
***************************************************************************)

TimeoutTcPhysicalKernelProperties(specification) ==
  /\ TimeoutTcRetainedControlKernelProperty(specification)
  /\ TimeoutTcPacketKernelProperty(specification)
  /\ TimeoutTcIngressKernelProperty(specification)
  /\ TimeoutTcDeliveryCandidateKernelProperty(specification)
  /\ TimeoutTcReceivedReducerKernelProperty(specification)
  /\ TimeoutTcInstallWalKernelProperty(specification)

THEOREM TimeoutTcPhysicalKernelsDischargeFrontier ==
  \A specification:
    TimeoutTcPhysicalKernelProperties(specification)
      => (specification
            => \A target \in AsyncCurrentResponsiveVoters,
                  minimumView \in Views:
                 TcFrontier(target, minimumView)
                   ~> TimeoutDirectGoal(target, minimumView))
BY Isa, PTL
   DEF TimeoutTcPhysicalKernelProperties,
       TimeoutTcRetainedControlKernelProperty,
       TimeoutTcPacketKernelProperty,
       TimeoutTcIngressKernelProperty,
       TimeoutTcDeliveryCandidateKernelProperty,
       TimeoutTcReceivedReducerKernelProperty,
       TimeoutTcInstallWalKernelProperty,
       TimeoutTcRetainedControlOwner,
       TimeoutTcPacketOwner,
       TimeoutTcIngressOwner,
       TimeoutTcReducerCandidateOwner,
       TimeoutTcDeliveryTerminalOwner,
       TimeoutTcReceivedReducerOwner,
       TimeoutTcInstallWalOwner,
       TimeoutTcKernelSource,
       TimeoutCertificateDelivery,
       TimeoutCertificateInstallOwner,
       TcFrontier

TimeoutTcFormationReducerKernelProperty(specification) ==
  specification
    => \A target \in AsyncCurrentResponsiveVoters,
          roundView \in Views:
         TimeoutFormTcCandidateOwned(target, roundView)
           ~> (TimeoutDirectGoal(target, roundView)
                \/ TcFrontier(target, roundView))

THEOREM TimeoutTcPhysicalKernelsDischargeFormationFrontier ==
  \A specification:
    /\ TimeoutTcPhysicalKernelProperties(specification)
    /\ TimeoutTcFormationReducerKernelProperty(specification)
    => (specification
          => \A target \in AsyncCurrentResponsiveVoters,
                roundView \in Views:
               TimeoutCertificateFormationFrontier(target, roundView)
                 ~> TimeoutDirectGoal(target, roundView))
BY TimeoutTcPhysicalKernelsDischargeFrontier, PTL
   DEF TimeoutTcFormationReducerKernelProperty,
       TimeoutCertificateFormationFrontier,
       TimeoutDirectGoal

(***************************************************************************
Exact Commit-certificate propagation kernels.

The direct transport path is split at the same physical boundaries as the
TimeoutVote and TC paths.  The last two kernels retain the exact Commit QC
through the target's received-QC reducer owner and evidence-bound Decision
WAL.  Applied-certificate discovery and the already-materialized exact
request/response round trip remain separate causal-origin kernels; neither
may be discharged by a Decision at another node.
***************************************************************************)

TimeoutDecisionKernelSource(source, target, qc) ==
  /\ source \in AsyncCurrentResponsiveVoters
  /\ target \in AsyncCurrentResponsiveVoters
  /\ source # target
  /\ DecisionSourceAt(source, qc)

TimeoutDecisionRetainedControlOwner(source, target, qc) ==
  LET item == CommitCertificateItem(source, target, qc)
  IN /\ TimeoutDecisionKernelSource(source, target, qc)
     /\ item \in asyncRetainedControl

TimeoutDecisionPacketOwner(source, target, qc) ==
  LET item == CommitCertificateItem(source, target, qc)
  IN /\ TimeoutDecisionKernelSource(source, target, qc)
     /\ ExactPacketOwns(item)

TimeoutDecisionIngressOwner(source, target, qc) ==
  LET item == CommitCertificateItem(source, target, qc)
  IN /\ TimeoutDecisionKernelSource(source, target, qc)
     /\ ExactIngressOwns(item)

TimeoutDecisionReducerCandidateOwner(source, target, qc) ==
  LET item == CommitCertificateItem(source, target, qc)
  IN /\ TimeoutDecisionKernelSource(source, target, qc)
     /\ ExactDeliveryCandidateOwns(item)

TimeoutDecisionReceivedReducerOwner(target, qc) ==
  /\ target \in AsyncCurrentResponsiveVoters
  /\ qc \in QcRecordSet
  /\ qc.context = context
  /\ qc.height = height
  /\ qc.phase = "Commit"
  /\ QcAt(target, qc) \in receivedQCs

TimeoutExactDecisionWalOwner(target, qc) ==
  /\ target \in AsyncCurrentResponsiveVoters
  /\ qc \in QcRecordSet
  /\ qc.context = context
  /\ qc.height = height
  /\ qc.phase = "Commit"
  /\ DecisionWal(target, qc, FALSE) \in pendingDecision

TimeoutDecisionDeliveryTerminalOwner(target, qc) ==
  \/ NodeHasDecision(target)
  \/ TimeoutDecisionReceivedReducerOwner(target, qc)
  \/ TimeoutExactDecisionWalOwner(target, qc)

TimeoutDecisionRetainedControlKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         TimeoutDecisionRetainedControlOwner(source, target, qc)
           ~> (TimeoutDecisionDeliveryTerminalOwner(target, qc)
                \/ TimeoutDecisionPacketOwner(source, target, qc)
                \/ TimeoutDecisionIngressOwner(source, target, qc)
                \/ TimeoutDecisionReducerCandidateOwner(
                     source, target, qc))

TimeoutDecisionPacketKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         TimeoutDecisionPacketOwner(source, target, qc)
           ~> (TimeoutDecisionDeliveryTerminalOwner(target, qc)
                \/ TimeoutDecisionIngressOwner(source, target, qc)
                \/ TimeoutDecisionReducerCandidateOwner(
                     source, target, qc))

TimeoutDecisionIngressKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         TimeoutDecisionIngressOwner(source, target, qc)
           ~> (TimeoutDecisionDeliveryTerminalOwner(target, qc)
                \/ TimeoutDecisionReducerCandidateOwner(
                     source, target, qc))

TimeoutDecisionDeliveryCandidateKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         TimeoutDecisionReducerCandidateOwner(source, target, qc)
           ~> TimeoutDecisionDeliveryTerminalOwner(target, qc)

TimeoutDecisionReceivedReducerKernelProperty(specification) ==
  specification
    => \A target, qc:
         TimeoutDecisionReceivedReducerOwner(target, qc)
           ~> (NodeHasDecision(target)
                \/ TimeoutExactDecisionWalOwner(target, qc))

TimeoutDecisionWalKernelProperty(specification) ==
  specification
    => \A target, qc:
         TimeoutExactDecisionWalOwner(target, qc)
           ~> NodeHasDecision(target)

TimeoutDecisionDirectPhysicalKernelProperties(specification) ==
  /\ TimeoutDecisionRetainedControlKernelProperty(specification)
  /\ TimeoutDecisionPacketKernelProperty(specification)
  /\ TimeoutDecisionIngressKernelProperty(specification)
  /\ TimeoutDecisionDeliveryCandidateKernelProperty(specification)
  /\ TimeoutDecisionReceivedReducerKernelProperty(specification)
  /\ TimeoutDecisionWalKernelProperty(specification)

THEOREM TimeoutDecisionDirectPhysicalKernelsDischargeDelivery ==
  \A specification:
    TimeoutDecisionDirectPhysicalKernelProperties(specification)
      => (specification
            => \A source, target, qc:
                 /\ TimeoutDecisionKernelSource(source, target, qc)
                 /\ CommitCertificateDelivery(source, target, qc)
                 ~> NodeHasDecision(target))
BY Isa, PTL
   DEF TimeoutDecisionDirectPhysicalKernelProperties,
       TimeoutDecisionRetainedControlKernelProperty,
       TimeoutDecisionPacketKernelProperty,
       TimeoutDecisionIngressKernelProperty,
       TimeoutDecisionDeliveryCandidateKernelProperty,
       TimeoutDecisionReceivedReducerKernelProperty,
       TimeoutDecisionWalKernelProperty,
       TimeoutDecisionRetainedControlOwner,
       TimeoutDecisionPacketOwner,
       TimeoutDecisionIngressOwner,
       TimeoutDecisionReducerCandidateOwner,
       TimeoutDecisionDeliveryTerminalOwner,
       TimeoutDecisionReceivedReducerOwner,
       TimeoutExactDecisionWalOwner,
       TimeoutDecisionKernelSource,
       CommitCertificateDelivery

TimeoutDecisionRoundTripTerminalOwner(source, target, qc) ==
  \/ NodeHasDecision(target)
  \/ CommitCertificateDelivery(source, target, qc)

TimeoutDecisionActiveRequestOwner(source, target, qc) ==
  /\ TimeoutDecisionKernelSource(source, target, qc)
  /\ \E request \in asyncActiveRequests:
       CommitCertificateRequestTo(target, source, request)

TimeoutDecisionRequestPacketOwner(source, target, qc) ==
  /\ TimeoutDecisionKernelSource(source, target, qc)
  /\ \E request \in AsyncNetworkItems:
       /\ CommitCertificateRequestTo(target, source, request)
       /\ ExactPacketOwns(request)

TimeoutDecisionRequestIngressOwner(source, target, qc) ==
  /\ TimeoutDecisionKernelSource(source, target, qc)
  /\ \E request \in AsyncNetworkItems:
       /\ CommitCertificateRequestTo(target, source, request)
       /\ ExactIngressOwns(request)

TimeoutDecisionRequestServeOwner(source, target, qc) ==
  /\ TimeoutDecisionKernelSource(source, target, qc)
  /\ \E request \in AsyncNetworkItems:
       /\ CommitCertificateRequestTo(target, source, request)
       /\ \E job \in AsyncServeJobSet:
            /\ job.candidate.item = request
            /\ ResponsiveProtectedServeJobOwned(source, job)

TimeoutDecisionResponsePacketOwner(source, target, qc) ==
  /\ TimeoutDecisionKernelSource(source, target, qc)
  /\ \E response \in AsyncNetworkItems:
       /\ CommitCertificateResponseFor(
            target, source, qc, response)
       /\ ExactPacketOwns(response)

TimeoutDecisionResponseIngressOwner(source, target, qc) ==
  /\ TimeoutDecisionKernelSource(source, target, qc)
  /\ \E response \in AsyncNetworkItems:
       /\ CommitCertificateResponseFor(
            target, source, qc, response)
       /\ ExactIngressOwns(response)

TimeoutDecisionResponseCandidateOwner(source, target, qc) ==
  /\ TimeoutDecisionKernelSource(source, target, qc)
  /\ \E response \in AsyncNetworkItems:
       /\ CommitCertificateResponseFor(
            target, source, qc, response)
       /\ CandidateScheduled(
            CommitCertificateResponseCandidate(response))

TimeoutDecisionAppliedAuthorityOriginKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         /\ TimeoutDecisionKernelSource(source, target, qc)
         /\ AppliedDecisionCertificateAuthority(source, qc)
         ~> (TimeoutDecisionRoundTripTerminalOwner(
               source, target, qc)
              \/ TimeoutDecisionActiveRequestOwner(source, target, qc)
              \/ TimeoutDecisionRequestPacketOwner(source, target, qc)
              \/ TimeoutDecisionRequestIngressOwner(source, target, qc)
              \/ TimeoutDecisionRequestServeOwner(source, target, qc)
              \/ TimeoutDecisionResponsePacketOwner(source, target, qc)
              \/ TimeoutDecisionResponseIngressOwner(source, target, qc)
              \/ TimeoutDecisionResponseCandidateOwner(
                   source, target, qc))

TimeoutDecisionActiveRequestKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         TimeoutDecisionActiveRequestOwner(source, target, qc)
           ~> (TimeoutDecisionRoundTripTerminalOwner(
                 source, target, qc)
                \/ TimeoutDecisionRequestPacketOwner(source, target, qc)
                \/ TimeoutDecisionRequestIngressOwner(source, target, qc)
                \/ TimeoutDecisionRequestServeOwner(source, target, qc)
                \/ TimeoutDecisionResponsePacketOwner(source, target, qc)
                \/ TimeoutDecisionResponseIngressOwner(source, target, qc)
                \/ TimeoutDecisionResponseCandidateOwner(
                     source, target, qc))

TimeoutDecisionRequestPacketKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         TimeoutDecisionRequestPacketOwner(source, target, qc)
           ~> (TimeoutDecisionRoundTripTerminalOwner(
                 source, target, qc)
                \/ TimeoutDecisionRequestIngressOwner(source, target, qc)
                \/ TimeoutDecisionRequestServeOwner(source, target, qc)
                \/ TimeoutDecisionResponsePacketOwner(source, target, qc)
                \/ TimeoutDecisionResponseIngressOwner(source, target, qc)
                \/ TimeoutDecisionResponseCandidateOwner(
                     source, target, qc))

TimeoutDecisionRequestIngressKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         TimeoutDecisionRequestIngressOwner(source, target, qc)
           ~> (TimeoutDecisionRoundTripTerminalOwner(
                 source, target, qc)
                \/ TimeoutDecisionRequestServeOwner(source, target, qc)
                \/ TimeoutDecisionResponsePacketOwner(source, target, qc)
                \/ TimeoutDecisionResponseIngressOwner(source, target, qc)
                \/ TimeoutDecisionResponseCandidateOwner(
                     source, target, qc))

TimeoutDecisionRequestServeKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         TimeoutDecisionRequestServeOwner(source, target, qc)
           ~> (TimeoutDecisionRoundTripTerminalOwner(
                 source, target, qc)
                \/ TimeoutDecisionResponsePacketOwner(source, target, qc)
                \/ TimeoutDecisionResponseIngressOwner(source, target, qc)
                \/ TimeoutDecisionResponseCandidateOwner(
                     source, target, qc))

TimeoutDecisionResponsePacketKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         TimeoutDecisionResponsePacketOwner(source, target, qc)
           ~> (TimeoutDecisionRoundTripTerminalOwner(
                 source, target, qc)
                \/ TimeoutDecisionResponseIngressOwner(source, target, qc)
                \/ TimeoutDecisionResponseCandidateOwner(
                     source, target, qc))

TimeoutDecisionResponseIngressKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         TimeoutDecisionResponseIngressOwner(source, target, qc)
           ~> (TimeoutDecisionRoundTripTerminalOwner(
                 source, target, qc)
                \/ TimeoutDecisionResponseCandidateOwner(
                     source, target, qc))

TimeoutDecisionResponseCandidateKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         TimeoutDecisionResponseCandidateOwner(source, target, qc)
           ~> TimeoutDecisionRoundTripTerminalOwner(
                source, target, qc)

TimeoutDecisionRoundTripPhysicalKernelProperties(specification) ==
  /\ TimeoutDecisionActiveRequestKernelProperty(specification)
  /\ TimeoutDecisionRequestPacketKernelProperty(specification)
  /\ TimeoutDecisionRequestIngressKernelProperty(specification)
  /\ TimeoutDecisionRequestServeKernelProperty(specification)
  /\ TimeoutDecisionResponsePacketKernelProperty(specification)
  /\ TimeoutDecisionResponseIngressKernelProperty(specification)
  /\ TimeoutDecisionResponseCandidateKernelProperty(specification)

TimeoutDecisionRoundTripCausalOriginKernelProperty(specification) ==
  specification
    => \A source, target, qc:
         /\ TimeoutDecisionKernelSource(source, target, qc)
         /\ CommitCertificateRoundTrip(target, source, qc)
         ~> TimeoutDecisionRoundTripTerminalOwner(
              source, target, qc)

THEOREM TimeoutDecisionRoundTripPhysicalKernelsDischargeCausalOrigin ==
  \A specification:
    TimeoutDecisionRoundTripPhysicalKernelProperties(specification)
      => TimeoutDecisionRoundTripCausalOriginKernelProperty(
           specification)
BY Isa, PTL
   DEF TimeoutDecisionRoundTripPhysicalKernelProperties,
       TimeoutDecisionActiveRequestKernelProperty,
       TimeoutDecisionRequestPacketKernelProperty,
       TimeoutDecisionRequestIngressKernelProperty,
       TimeoutDecisionRequestServeKernelProperty,
       TimeoutDecisionResponsePacketKernelProperty,
       TimeoutDecisionResponseIngressKernelProperty,
       TimeoutDecisionResponseCandidateKernelProperty,
       TimeoutDecisionRoundTripCausalOriginKernelProperty,
       TimeoutDecisionRoundTripTerminalOwner,
       TimeoutDecisionActiveRequestOwner,
       TimeoutDecisionRequestPacketOwner,
       TimeoutDecisionRequestIngressOwner,
       TimeoutDecisionRequestServeOwner,
       TimeoutDecisionResponsePacketOwner,
       TimeoutDecisionResponseIngressOwner,
       TimeoutDecisionResponseCandidateOwner,
       TimeoutDecisionKernelSource,
       CommitCertificateRoundTrip

TimeoutDecisionOriginKernelProperties(specification) ==
  /\ TimeoutDecisionAppliedAuthorityOriginKernelProperty(specification)
  /\ TimeoutDecisionRoundTripPhysicalKernelProperties(specification)

THEOREM TimeoutDecisionPhysicalKernelsDischargePropagationFrontier ==
  \A specification:
    /\ TimeoutDecisionDirectPhysicalKernelProperties(specification)
    /\ TimeoutDecisionOriginKernelProperties(specification)
    => (specification
          => \A target \in AsyncCurrentResponsiveVoters:
               DecisionPropagationFrontier(target)
                 ~> NodeHasDecision(target))
BY TimeoutDecisionDirectPhysicalKernelsDischargeDelivery,
   TimeoutDecisionRoundTripPhysicalKernelsDischargeCausalOrigin,
   Isa, PTL
   DEF TimeoutDecisionOriginKernelProperties,
       TimeoutDecisionAppliedAuthorityOriginKernelProperty,
       TimeoutDecisionRoundTripCausalOriginKernelProperty,
       TimeoutDecisionRoundTripTerminalOwner,
       TimeoutDecisionKernelSource,
       DecisionPropagationFrontier

TimeoutCertificateDecisionPhysicalKernelProperties(specification) ==
  /\ TimeoutTcPhysicalKernelProperties(specification)
  /\ TimeoutTcFormationReducerKernelProperty(specification)
  /\ TimeoutDecisionDirectPhysicalKernelProperties(specification)
  /\ TimeoutDecisionOriginKernelProperties(specification)

THEOREM TimeoutCertificateDecisionPhysicalKernelsDischargeConvergence ==
  \A specification:
    TimeoutCertificateDecisionPhysicalKernelProperties(specification)
      => TimeoutCertificateAndDecisionConvergenceProperty(specification)
BY TimeoutTcPhysicalKernelsDischargeFrontier,
   TimeoutTcPhysicalKernelsDischargeFormationFrontier,
   TimeoutDecisionPhysicalKernelsDischargePropagationFrontier,
   Isa
   DEF TimeoutCertificateDecisionPhysicalKernelProperties,
       TimeoutCertificateAndDecisionConvergenceProperty

\* Honest residuals are now only the exact physical kernels above.  The four
\* armed-Runtime clauses have been reduced to, and discharged through, one
\* fixed-timeout exact-ingress action-step theorem.  Its lifecycle rank,
\* finite frozen owner episode, exhaustive concrete action classification,
\* and already-fair owner origin are all source-derived.  Consequently the
\* derived priority-ticket property is consumed below as a theorem of
\* AsyncLiveSpecAt, not retained as a residual premise.  AsyncLiveSpecAt is
\* exactly AsyncSpecAt and carries no install-generation assumption.  In particular,
\* `FormTC` is disabled, so the formation kernel must project the concrete
\* pending-install authority produced by the receipt reducer; it may not count
\* a synthetic `FormTC` occurrence as progress.  The applied-authority origin
\* kernel and the separately split request/response kernels retain
\* `(target, source, qc)` throughout discovery.  The remaining transport
\* kernels retain one immutable wire item through retained control, packet,
\* ingress, delivery candidate, reducer receipt, and WAL.

DirectTimeoutViewClosureResidualProperty(specification) ==
  /\ TimeoutVoteDeliveryPhysicalKernelProperties(specification)
  /\ TimeoutCertificateDecisionPhysicalKernelProperties(specification)

THEOREM DirectTimeoutPhysicalKernelsDischargeCompositeSeams ==
  \A initialContext:
    DirectTimeoutViewClosureResidualProperty(
         AsyncLiveSpecAt(initialContext))
      => /\ TimeoutFixedClockLifecycleOwnerServiceProperty(
               AsyncLiveSpecAt(initialContext))
         /\ TimeoutArmedExactWalEndpointProperty(
              AsyncLiveSpecAt(initialContext))
         /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(
              AsyncLiveSpecAt(initialContext))
         /\ TimeoutCertificateAndDecisionConvergenceProperty(
              AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesProtectedServiceFiniteRunnerEpisodeClosure,
   AsyncLiveProvidesTimeoutFixedClockLifecyclePhysicalKernels,
   TimeoutFixedClockPhysicalKernelsDischargeLifecycleService,
   AsyncLiveClosesTimeoutFixedOwnerPriorityTicketNonReplenishment,
   TimeoutFixedOwnerPriorityTicketNonReplenishmentDischargesArmedWalPhysicalKernels,
   TimeoutArmedWalPhysicalKernelsDischargeExactEndpoint,
   TimeoutVotePhysicalKernelsDischargeSourceIsolatedDelivery,
   TimeoutCertificateDecisionPhysicalKernelsDischargeConvergence
   DEF DirectTimeoutViewClosureResidualProperty

(***************************************************************************
Catch-up temporal composition.

This corridor deliberately consumes the exact TC property, not a timeout-vote
outcome for a recipient which is already above the lagging source.  It is
stated before the complete rank theorem so the dependency remains visible to
the proof checker and mutation suite.
***************************************************************************)

THEOREM TimeoutRetainedTcClosesLaggingSource ==
  \A initialContext:
    /\ TimeoutViewOwnershipPreservationProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutCertificateAndDecisionConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    => (AsyncLiveSpecAt(initialContext)
          => \A target, laggingSource
                 \in AsyncCurrentResponsiveVoters,
                roundView \in Views:
               (/\ TimeoutRoundStable(target, roundView)
                /\ nodeView[laggingSource] < roundView)
                 ~> TimeoutLaggingSourceCatchupOutcome(
                      target, roundView, laggingSource))
PROOF
  <1>1. ASSUME NEW initialContext,
                TimeoutViewOwnershipPreservationProperty(
                  AsyncLiveSpecAt(initialContext)),
                TimeoutCertificateAndDecisionConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE AsyncLiveSpecAt(initialContext)
                 => \A target, laggingSource
                        \in AsyncCurrentResponsiveVoters,
                       roundView \in Views:
                      (/\ TimeoutRoundStable(target, roundView)
                       /\ nodeView[laggingSource] < roundView)
                        ~> TimeoutLaggingSourceCatchupOutcome(
                             target, roundView, laggingSource)
    <2>1. AsyncLiveSpecAt(initialContext)
             => []AsyncStrongTypeInvariant
      BY AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysStrongTypeInvariant, PTL
    <2>2. AsyncLiveSpecAt(initialContext)
             => []TimeoutViewOwnershipInvariant
      BY <1>1 DEF TimeoutViewOwnershipPreservationProperty
    <2>3. ASSUME NEW target, NEW laggingSource
                    \in AsyncCurrentResponsiveVoters,
                  NEW roundView \in Views,
                  AsyncLiveSpecAt(initialContext)
           PROVE (/\ TimeoutRoundStable(target, roundView)
                    /\ nodeView[laggingSource] < roundView)
                   ~> TimeoutLaggingSourceCatchupOutcome(
                        target, roundView, laggingSource)
      <3>1. CASE roundView = 0
        BY <2>1, <2>3, <3>1, PTL
           DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
               Safety, TypeInvariant, Views
      <3>2. CASE laggingSource = target
        BY <2>3, <3>2, PTL
      <3>3. CASE /\ roundView > 0
                   /\ laggingSource # target
        <4>1. roundView - 1 \in Views
          BY <2>3, <3>3, Isa
             DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
                 Safety, TypeInvariant, ModelConfiguration, Views
        <4>2. [](/\ AsyncTypeInvariant
                  /\ TimeoutViewOwnershipInvariant
                  /\ TimeoutRoundStable(target, roundView)
                  /\ nodeView[laggingSource] < roundView
                 => TcFrontier(laggingSource, roundView - 1))
          BY <2>1, <2>2,
             AsyncStrongTypeProjectsAsyncType,
             TimeoutLaggingSourceHasExactTcFrontier, PTL
        <4>3. (/\ TimeoutRoundStable(target, roundView)
                 /\ nodeView[laggingSource] < roundView)
                ~> TcFrontier(laggingSource, roundView - 1)
          BY <4>2, PTL
        <4>4. TcFrontier(laggingSource, roundView - 1)
                 ~> TimeoutDirectGoal(
                      laggingSource, roundView - 1)
          BY <1>1, <2>3, <4>1
             DEF TimeoutCertificateAndDecisionConvergenceProperty
        <4>5. [](TimeoutDirectGoal(
                    laggingSource, roundView - 1)
                   => TimeoutLaggingSourceCatchupOutcome(
                        target, roundView, laggingSource))
          BY <2>1, <2>2, <2>3, <3>3,
             TimeoutLaggingSourceGoalSuppliesCatchupOutcome, PTL, Isa
        <4> QED BY <4>3, <4>4, <4>5, PTL
      <3> QED BY <2>3, <3>1, <3>2, <3>3, Isa
    <2> QED BY <2>3
  <1> QED BY <1>1

(***************************************************************************
Receipt-stage temporal composition.

One missing signer is frozen as a temporal parameter.  It either already
owns its exact timeout vote, or the proved deadline corridor exposes the
armed Runtime owner.  The semantic handoff then reaches the exact vote
origin, and source-isolated delivery reaches this target's receipt.  A source
view/Decision escape is not counted as a receipt: the ownership invariant
maps it to the target's exact TC/Decision frontier.
***************************************************************************)

TimeoutMissingSignerServiceGoal(target, roundView, signer) ==
  \/ TimeoutDirectGoal(target, roundView)
  \/ TimeoutDirectOwnerFrontier(target, roundView)
  \/ ReceivedTimeoutVoteAt(target, signer, roundView)

THEOREM TimeoutRoundStablePersistsUnlessDirectGoal ==
  \A target \in AsyncCurrentResponsiveVoters,
     roundView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ TimeoutRoundStable(target, roundView)
    /\ [AsyncNext]_AsyncAllVars
    => \/ TimeoutDirectGoal(target, roundView)'
       \/ TimeoutRoundStable(target, roundView)'
BY PostGstAsyncBracketAdvancesEveryNodeView,
   GstResponsiveNodesAreUp, IsaT(240)
   DEF TimeoutRoundStable, TimeoutDirectGoal,
       TimeoutViewGoal, NodeHasDecision,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       AsyncAllVars, vars

THEOREM TimeoutExactDeliveryOutcomeSuppliesMissingSignerServiceGoal ==
  \A target, signer \in AsyncCurrentResponsiveVoters,
     roundView \in Views,
     vote \in TimeoutVoteRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ TimeoutRoundStable(target, roundView)
    /\ TimeoutVoteSemanticIdentity(signer, roundView, vote)
    /\ TimeoutDeliveryOutcome(vote, target)
    => TimeoutMissingSignerServiceGoal(target, roundView, signer)
BY TimeoutDominatedExactVoteSuppliesTargetOwner, Isa
   DEF TimeoutDeliveryOutcome,
       TimeoutMissingSignerServiceGoal,
       TimeoutReceipt, ReceivedTimeoutVoteAt,
       TimeoutVoteSemanticIdentity

THEOREM TimeoutExactOriginOutcomeSuppliesReceiptDeliveryOrTargetOwner ==
  \A target, signer \in AsyncCurrentResponsiveVoters,
     roundView \in Views,
     vote \in TimeoutVoteRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ TimeoutRoundStable(target, roundView)
    /\ TimeoutVoteSemanticIdentity(signer, roundView, vote)
    /\ TimeoutOriginOutcome(signer, roundView, vote, target)
    => \/ TimeoutMissingSignerServiceGoal(
            target, roundView, signer)
       \/ TimeoutDelivery(vote, target)
BY TimeoutDominatedExactVoteSuppliesTargetOwner, Isa
   DEF TimeoutOriginOutcome,
       TimeoutMissingSignerServiceGoal,
       TimeoutReceipt, ReceivedTimeoutVoteAt,
       TimeoutVoteSemanticIdentity

THEOREM TimeoutExactOriginReachesMissingSignerServiceGoal ==
  \A initialContext:
    /\ TimeoutViewOwnershipPreservationProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSemanticOwnerHandoffProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    => (AsyncLiveSpecAt(initialContext)
          => \A target, signer
                 \in AsyncCurrentResponsiveVoters,
                roundView \in Views,
                vote \in TimeoutVoteRecordSet:
               (/\ TimeoutRoundStable(target, roundView)
                /\ TimeoutOrigin(signer, roundView, vote))
                 ~> TimeoutMissingSignerServiceGoal(
                      target, roundView, signer))
PROOF
  <1>1. ASSUME NEW initialContext,
                TimeoutViewOwnershipPreservationProperty(
                  AsyncLiveSpecAt(initialContext)),
                TimeoutSemanticOwnerHandoffProperty(
                  AsyncLiveSpecAt(initialContext)),
                TimeoutSourceIsolatedDeliveryConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE AsyncLiveSpecAt(initialContext)
                 => \A target, signer
                        \in AsyncCurrentResponsiveVoters,
                       roundView \in Views,
                       vote \in TimeoutVoteRecordSet:
                      (/\ TimeoutRoundStable(target, roundView)
                       /\ TimeoutOrigin(signer, roundView, vote))
                        ~> TimeoutMissingSignerServiceGoal(
                             target, roundView, signer)
    <2>1. AsyncLiveSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ TimeoutViewOwnershipInvariant)
      BY AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysStrongTypeInvariant, <1>1, PTL
         DEF TimeoutViewOwnershipPreservationProperty
    <2>2. AsyncLiveSpecAt(initialContext)
             => [](gst => []gst)
      BY AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecKeepsGstOnceSet, PTL
    <2>3. ASSUME NEW target, NEW signer
                    \in AsyncCurrentResponsiveVoters,
                  NEW roundView \in Views,
                  NEW vote \in TimeoutVoteRecordSet,
                  AsyncLiveSpecAt(initialContext)
           PROVE (/\ TimeoutRoundStable(target, roundView)
                    /\ TimeoutOrigin(signer, roundView, vote))
                   ~> TimeoutMissingSignerServiceGoal(
                        target, roundView, signer)
      <3>1. [](TimeoutRoundStable(target, roundView)
                 /\ ~TimeoutDirectGoal(target, roundView)
                 /\ [AsyncNext]_AsyncAllVars
                => TimeoutRoundStable(target, roundView)')
        BY <2>1, TimeoutRoundStablePersistsUnlessDirectGoal, PTL
      <3>2. (gst
               /\ TimeoutOrigin(signer, roundView, vote))
              ~> TimeoutOriginOutcome(
                   signer, roundView, vote, target)
        BY <1>1, <2>3
           DEF TimeoutSemanticOwnerHandoffProperty
      <3>3. [](TimeoutRoundStable(target, roundView)
                 /\ TimeoutOriginOutcome(
                      signer, roundView, vote, target)
                => \/ TimeoutMissingSignerServiceGoal(
                        target, roundView, signer)
                   \/ TimeoutDelivery(vote, target))
        BY <2>1, <2>3,
           TimeoutExactOriginOutcomeSuppliesReceiptDeliveryOrTargetOwner,
           PTL, Isa
           DEF TimeoutOrigin, TimeoutVoteSemanticIdentity
      <3>4. (gst
               /\ TimeoutDelivery(vote, target))
              ~> TimeoutDeliveryOutcome(vote, target)
        BY <1>1, <2>3
           DEF TimeoutSourceIsolatedDeliveryConvergenceProperty,
               TimeoutOrigin, TimeoutVoteSemanticIdentity
      <3>5. [](TimeoutRoundStable(target, roundView)
                 /\ TimeoutDeliveryOutcome(vote, target)
                => TimeoutMissingSignerServiceGoal(
                     target, roundView, signer))
        BY <2>1, <2>3,
           TimeoutExactDeliveryOutcomeSuppliesMissingSignerServiceGoal,
           PTL, Isa
           DEF TimeoutOrigin, TimeoutVoteSemanticIdentity
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4, <3>5,
                    PTL
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM TimeoutMissingReceiptSignerReachesServiceGoal ==
  \A initialContext:
    /\ TimeoutViewOwnershipPreservationProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutDeadlineClockConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSemanticOwnerHandoffProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    => (AsyncLiveSpecAt(initialContext)
          => \A target, signer
                 \in AsyncCurrentResponsiveVoters,
                roundView \in Views,
                sourceRank \in TimeoutProgressRankCarrier:
               TimeoutReceiptSignerPendingAtRank(
                 target, roundView, sourceRank, signer)
                 ~> TimeoutMissingSignerServiceGoal(
                      target, roundView, signer))
PROOF
  <1>1. ASSUME NEW initialContext,
                TimeoutViewOwnershipPreservationProperty(
                  AsyncLiveSpecAt(initialContext)),
                TimeoutDeadlineClockConvergenceProperty(
                  AsyncLiveSpecAt(initialContext)),
                TimeoutSemanticOwnerHandoffProperty(
                  AsyncLiveSpecAt(initialContext)),
                TimeoutSourceIsolatedDeliveryConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE AsyncLiveSpecAt(initialContext)
                 => \A target, signer
                        \in AsyncCurrentResponsiveVoters,
                       roundView \in Views,
                       sourceRank \in TimeoutProgressRankCarrier:
                      TimeoutReceiptSignerPendingAtRank(
                        target, roundView, sourceRank, signer)
                        ~> TimeoutMissingSignerServiceGoal(
                             target, roundView, signer)
    <2>1. AsyncLiveSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ TimeoutViewOwnershipInvariant)
      BY AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysStrongTypeInvariant, <1>1, PTL
         DEF TimeoutViewOwnershipPreservationProperty
    <2>2. TimeoutExactOriginReachesMissingSignerServiceGoal
      BY <1>1
    <2>3. ASSUME NEW target, NEW signer
                    \in AsyncCurrentResponsiveVoters,
                  NEW roundView \in Views,
                  NEW sourceRank \in TimeoutProgressRankCarrier,
                  AsyncLiveSpecAt(initialContext)
           PROVE TimeoutReceiptSignerPendingAtRank(
                   target, roundView, sourceRank, signer)
                   ~> TimeoutMissingSignerServiceGoal(
                        target, roundView, signer)
      <3>1. [](TimeoutReceiptSignerPendingAtRank(
                   target, roundView, sourceRank, signer)
                 => (/\ TimeoutRoundStable(target, roundView)
                      /\ gst
                      /\ nodeView[signer] = roundView
                      /\ ~NodeHasDecision(signer)))
        BY <2>3, PTL, Isa
           DEF TimeoutReceiptSignerPendingAtRank,
               TimeoutProgressRankFrontier, TimeoutReceiptAtRank,
               TimeoutRoundStable, ResponsiveDecisionExists
      <3>2. [](TimeoutReceiptSignerPendingAtRank(
                   target, roundView, sourceRank, signer)
                 /\ NodeTimedOut(signer, roundView)
                => \E vote \in TimeoutVoteRecordSet:
                     TimeoutOrigin(signer, roundView, vote))
        BY <2>1, <2>3, IsaT(180), PTL
           DEF TimeoutReceiptSignerPendingAtRank,
               TimeoutProgressRankFrontier, TimeoutReceiptAtRank,
               NodeTimedOut, TimeoutViewOwnershipInvariant,
               TimeoutVoteSemanticIdentity,
               AsyncStrongTypeInvariant, StrongInductiveInvariant,
               Safety, TypeInvariant
      <3>3. [](TimeoutReceiptSignerPendingAtRank(
                   target, roundView, sourceRank, signer)
                 /\ ~NodeTimedOut(signer, roundView)
                => TimeoutRoundTrigger(signer, roundView))
        BY <2>1, <2>3, GstResponsiveNodesAreUp, PTL, Isa
           DEF TimeoutReceiptSignerPendingAtRank,
               TimeoutProgressRankFrontier, TimeoutReceiptAtRank,
               TimeoutRoundTrigger, TimeoutRoundStable,
               ResponsiveDecisionExists
      <3>4. TimeoutRoundTrigger(signer, roundView)
               ~> (TimeoutDirectGoal(signer, roundView)
                    \/ TimeoutDeadlineArmedOwner(signer, roundView)
                    \/ \E vote \in TimeoutVoteRecordSet:
                         TimeoutOrigin(signer, roundView, vote))
        BY <1>1, <2>3
           DEF TimeoutDeadlineClockConvergenceProperty
      <3>5. TimeoutDeadlineArmedOwner(signer, roundView)
               ~> (TimeoutDirectGoal(signer, roundView)
                    \/ \E vote \in TimeoutVoteRecordSet:
                         TimeoutOrigin(signer, roundView, vote))
        BY <1>1, <2>3
           DEF TimeoutSemanticOwnerHandoffProperty
      <3>6. [](TimeoutRoundStable(target, roundView)
                 /\ TimeoutDirectGoal(signer, roundView)
                => TimeoutMissingSignerServiceGoal(
                     target, roundView, signer))
        BY <2>1, <2>3,
           TimeoutResponsiveSourceGoalSuppliesTargetOwner,
           PTL
           DEF TimeoutMissingSignerServiceGoal
      <3>7. \A vote \in TimeoutVoteRecordSet:
               (/\ TimeoutRoundStable(target, roundView)
                /\ TimeoutOrigin(signer, roundView, vote))
                 ~> TimeoutMissingSignerServiceGoal(
                      target, roundView, signer)
        BY <1>1, <2>3,
           TimeoutExactOriginReachesMissingSignerServiceGoal
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5,
                    <3>6, <3>7, PTL
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM TimeoutCatchupSourcePendingClosesOneRankStep ==
  \A initialContext:
    /\ TimeoutViewOwnershipPreservationProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutCertificateAndDecisionConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    => (AsyncLiveSpecAt(initialContext)
          => \A target, laggingSource
                 \in AsyncCurrentResponsiveVoters,
                roundView \in Views,
                sourceRank \in TimeoutProgressRankCarrier:
               TimeoutCatchupSourcePendingAtRank(
                 target, roundView, sourceRank, laggingSource)
                 ~> TimeoutProgressRankStrictGoal(
                      target, roundView, sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext,
                TimeoutViewOwnershipPreservationProperty(
                  AsyncLiveSpecAt(initialContext)),
                TimeoutCertificateAndDecisionConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE AsyncLiveSpecAt(initialContext)
                 => \A target, laggingSource
                        \in AsyncCurrentResponsiveVoters,
                       roundView \in Views,
                       sourceRank \in TimeoutProgressRankCarrier:
                      TimeoutCatchupSourcePendingAtRank(
                        target, roundView, sourceRank, laggingSource)
                        ~> TimeoutProgressRankStrictGoal(
                             target, roundView, sourceRank)
    <2>1. AsyncLiveSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ TimeoutViewOwnershipInvariant)
      BY AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysStrongTypeInvariant, <1>1, PTL
         DEF TimeoutViewOwnershipPreservationProperty
    <2>2. ASSUME NEW target, NEW laggingSource
                    \in AsyncCurrentResponsiveVoters,
                  NEW roundView \in Views,
                  NEW sourceRank \in TimeoutProgressRankCarrier,
                  AsyncLiveSpecAt(initialContext)
           PROVE TimeoutCatchupSourcePendingAtRank(
                   target, roundView, sourceRank, laggingSource)
                   ~> TimeoutProgressRankStrictGoal(
                        target, roundView, sourceRank)
      <3>1. TimeoutCatchupSourcePendingAtRank(
               target, roundView, sourceRank, laggingSource)
              ~> TimeoutLaggingSourceCatchupOutcome(
                   target, roundView, laggingSource)
        BY <1>1, <2>2, TimeoutRetainedTcClosesLaggingSource,
           PTL
           DEF TimeoutCatchupSourcePendingAtRank,
               TimeoutProgressRankFrontier,
               TimeoutCatchupDebtAtRank
      <3>2. [](TimeoutCatchupSourcePendingAtRank(
                   target, roundView, sourceRank, laggingSource)
                 /\ ~TimeoutProgressRankStrictGoal(
                      target, roundView, sourceRank)
                 /\ [AsyncNext]_AsyncAllVars
                => TimeoutCatchupSourcePendingAtRank(
                     target, roundView, sourceRank, laggingSource)')
        BY <2>1, <2>2,
           TimeoutCatchupPendingPersistsOrStrictlyDescends, PTL
      <3>3. [](TimeoutCatchupSourcePendingAtRank(
                   target, roundView, sourceRank, laggingSource)
                 /\ TimeoutLaggingSourceCatchupOutcome(
                      target, roundView, laggingSource)
                => TimeoutProgressRankStrictGoal(
                     target, roundView, sourceRank))
        BY <2>2, PTL, Isa
           DEF TimeoutCatchupSourcePendingAtRank,
               TimeoutLaggingSourceCatchupOutcome,
               TimeoutProgressRankStrictGoal
      <3> QED BY <3>1, <3>2, <3>3, PTL
    <2> QED BY <2>2
  <1> QED BY <1>1

THEOREM TimeoutReceiptSignerPendingClosesOneRankStep ==
  \A initialContext:
    /\ TimeoutViewOwnershipPreservationProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutDeadlineClockConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSemanticOwnerHandoffProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    => (AsyncLiveSpecAt(initialContext)
          => \A target, signer
                 \in AsyncCurrentResponsiveVoters,
                roundView \in Views,
                sourceRank \in TimeoutProgressRankCarrier:
               TimeoutReceiptSignerPendingAtRank(
                 target, roundView, sourceRank, signer)
                 ~> TimeoutProgressRankStrictGoal(
                      target, roundView, sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext,
                TimeoutViewOwnershipPreservationProperty(
                  AsyncLiveSpecAt(initialContext)),
                TimeoutDeadlineClockConvergenceProperty(
                  AsyncLiveSpecAt(initialContext)),
                TimeoutSemanticOwnerHandoffProperty(
                  AsyncLiveSpecAt(initialContext)),
                TimeoutSourceIsolatedDeliveryConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE AsyncLiveSpecAt(initialContext)
                 => \A target, signer
                        \in AsyncCurrentResponsiveVoters,
                       roundView \in Views,
                       sourceRank \in TimeoutProgressRankCarrier:
                      TimeoutReceiptSignerPendingAtRank(
                        target, roundView, sourceRank, signer)
                        ~> TimeoutProgressRankStrictGoal(
                             target, roundView, sourceRank)
    <2>1. AsyncLiveSpecAt(initialContext)
             => [](AsyncStrongTypeInvariant
                    /\ TimeoutViewOwnershipInvariant)
      BY AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysStrongTypeInvariant, <1>1, PTL
         DEF TimeoutViewOwnershipPreservationProperty
    <2>2. ASSUME NEW target, NEW signer
                    \in AsyncCurrentResponsiveVoters,
                  NEW roundView \in Views,
                  NEW sourceRank \in TimeoutProgressRankCarrier,
                  AsyncLiveSpecAt(initialContext)
           PROVE TimeoutReceiptSignerPendingAtRank(
                   target, roundView, sourceRank, signer)
                   ~> TimeoutProgressRankStrictGoal(
                        target, roundView, sourceRank)
      <3>1. TimeoutReceiptSignerPendingAtRank(
               target, roundView, sourceRank, signer)
              ~> TimeoutMissingSignerServiceGoal(
                   target, roundView, signer)
        BY <1>1, <2>2,
           TimeoutMissingReceiptSignerReachesServiceGoal
      <3>2. [](TimeoutReceiptSignerPendingAtRank(
                   target, roundView, sourceRank, signer)
                 /\ ~TimeoutProgressRankStrictGoal(
                      target, roundView, sourceRank)
                 /\ [AsyncNext]_AsyncAllVars
                => TimeoutReceiptSignerPendingAtRank(
                     target, roundView, sourceRank, signer)')
        BY <2>1, <2>2,
           TimeoutReceiptPendingPersistsOrStrictlyDescends, PTL
      <3>3. [](TimeoutReceiptSignerPendingAtRank(
                   target, roundView, sourceRank, signer)
                 /\ TimeoutMissingSignerServiceGoal(
                      target, roundView, signer)
                => TimeoutProgressRankStrictGoal(
                     target, roundView, sourceRank))
        BY <2>2, PTL, Isa
           DEF TimeoutReceiptSignerPendingAtRank,
               TimeoutMissingReceiptVoters,
               TimeoutMissingSignerServiceGoal,
               TimeoutProgressRankStrictGoal
      <3> QED BY <3>1, <3>2, <3>3, PTL
    <2> QED BY <2>2
  <1> QED BY <1>1

THEOREM TimeoutFiniteRankCoordinationIsDischarged ==
  \A initialContext:
    /\ TimeoutViewOwnershipPreservationProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutCertificateAndDecisionConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    => TimeoutFiniteRankDescentProperty(
         AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                TimeoutViewOwnershipPreservationProperty(
                  AsyncLiveSpecAt(initialContext)),
                TimeoutCertificateAndDecisionConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE TimeoutFiniteRankDescentProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. CASE /\ TimeoutDeadlineClockConvergenceProperty(
                      AsyncLiveSpecAt(initialContext))
                 /\ TimeoutSemanticOwnerHandoffProperty(
                      AsyncLiveSpecAt(initialContext))
                 /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(
                      AsyncLiveSpecAt(initialContext))
      <3>1. AsyncLiveSpecAt(initialContext)
               => []AsyncStrongTypeInvariant
        BY AsyncLiveSpecProjectsAsyncSpec,
           AsyncSpecAlwaysStrongTypeInvariant
      <3>2. ASSUME NEW target \in AsyncCurrentResponsiveVoters,
                    NEW roundView \in Views,
                    NEW rank \in TimeoutProgressRankCarrier,
                    AsyncLiveSpecAt(initialContext)
             PROVE TimeoutProgressRankFrontier(
                     target, roundView, rank)
                     ~> TimeoutProgressRankStrictGoal(
                          target, roundView, rank)
        <4>1. CASE rank[1] = 2
          <5>1. CASE rank[2] = 0
            BY <1>1, <3>1, <3>2, <4>1, <5>1,
               TimeoutCatchupDebtZeroEntersReceiptStage, PTL
               DEF TimeoutProgressRankFrontier,
                   TimeoutProgressRankStrictGoal
          <5>2. CASE rank[2] > 0
            <6>1. TimeoutProgressRankFrontier(
                     target, roundView, rank)
                    => \E laggingSource
                           \in AsyncCurrentResponsiveVoters:
                         TimeoutCatchupSourcePendingAtRank(
                           target, roundView, rank, laggingSource)
              BY <3>1, <3>2, <4>1, <5>2,
                 TimeoutPositiveCatchupDebtSelectsLaggingSource,
                 Isa, PTL
                 DEF TimeoutCatchupSourcePendingAtRank,
                     TimeoutProgressRankFrontier,
                     TimeoutProgressRankCarrier
            <6>2. \A laggingSource
                       \in AsyncCurrentResponsiveVoters:
                     TimeoutCatchupSourcePendingAtRank(
                       target, roundView, rank, laggingSource)
                       ~> TimeoutProgressRankStrictGoal(
                            target, roundView, rank)
              BY <1>1, <3>2,
                 TimeoutCatchupSourcePendingClosesOneRankStep
            <6> QED BY <6>1, <6>2, PTL
          <5> QED BY <3>2, <5>1, <5>2, Isa
             DEF TimeoutProgressRankCarrier
        <4>2. CASE rank[1] = 1
          <5>1. CASE rank[2] = 0
            BY <1>1, <3>1, <3>2, <4>2, <5>1,
               TimeoutReceiptDebtZeroExposesExactFormationFrontier,
               PTL
               DEF TimeoutProgressRankFrontier,
                   TimeoutProgressRankStrictGoal,
                   TimeoutDirectOwnerFrontier
          <5>2. CASE rank[2] > 0
            <6>1. TimeoutProgressRankFrontier(
                     target, roundView, rank)
                    => \E signer
                           \in AsyncCurrentResponsiveVoters:
                         TimeoutReceiptSignerPendingAtRank(
                           target, roundView, rank, signer)
              BY <3>1, <3>2, <4>2, <5>2,
                 TimeoutPositiveReceiptDebtSelectsMissingSigner,
                 Isa, PTL
                 DEF TimeoutReceiptSignerPendingAtRank,
                     TimeoutProgressRankFrontier,
                     TimeoutProgressRankCarrier,
                     TimeoutMissingReceiptVoters
            <6>2. \A signer \in AsyncCurrentResponsiveVoters:
                     TimeoutReceiptSignerPendingAtRank(
                       target, roundView, rank, signer)
                       ~> TimeoutProgressRankStrictGoal(
                            target, roundView, rank)
              BY <1>1, <2>1, <3>2,
                 TimeoutReceiptSignerPendingClosesOneRankStep
            <6> QED BY <6>1, <6>2, PTL
          <5> QED BY <3>2, <5>1, <5>2, Isa
             DEF TimeoutProgressRankCarrier
        <4> QED BY <3>2, <4>1, <4>2, Isa
           DEF TimeoutProgressRankCarrier
      <3>3. TimeoutFiniteRankDescentProperty(
               AsyncLiveSpecAt(initialContext))
        BY <2>1, <3>1, <3>2
           DEF TimeoutFiniteRankDescentProperty,
               TimeoutProgressRankStrictGoal
      <3> QED BY <3>3
    <2>2. CASE ~(/\ TimeoutDeadlineClockConvergenceProperty(
                       AsyncLiveSpecAt(initialContext))
                  /\ TimeoutSemanticOwnerHandoffProperty(
                       AsyncLiveSpecAt(initialContext))
                  /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(
                       AsyncLiveSpecAt(initialContext)))
      BY <2>2 DEF TimeoutFiniteRankDescentProperty
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Derived source exposure.

This theorem consumes only the ownership-preservation seam plus already
proved type/recovery invariants.  It does not assume any temporal service
claim for the exposed rank or owner.
***************************************************************************)

THEOREM TimeoutOwnershipPreservationSuppliesExactFrontierExposure ==
  \A initialContext:
    TimeoutViewOwnershipPreservationProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
                 TimeoutDirectReleaseSource(target, roundView)
                   ~> (TimeoutDirectGoal(target, roundView)
                        \/ TimeoutDirectOwnerFrontier(
                             target, roundView)
                        \/ TimeoutProgressRankedFrontier(
                             target, roundView)))
PROOF
  <1>1. ASSUME NEW initialContext,
                TimeoutViewOwnershipPreservationProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE AsyncLiveSpecAt(initialContext)
                 => \A target \in AsyncCurrentResponsiveVoters,
                       roundView \in Views:
                      TimeoutDirectReleaseSource(target, roundView)
                        ~> (TimeoutDirectGoal(target, roundView)
                             \/ TimeoutDirectOwnerFrontier(
                                  target, roundView)
                             \/ TimeoutProgressRankedFrontier(
                                  target, roundView))
    <2>1. AsyncSpecAt(initialContext)
              => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>2. AsyncLiveSpecAt(initialContext)
              => []TimeoutViewOwnershipInvariant
      BY <1>1 DEF TimeoutViewOwnershipPreservationProperty
    <2>3. [](\A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
               /\ AsyncStrongTypeInvariant
               /\ TimeoutViewOwnershipInvariant
               /\ TimeoutDirectReleaseSource(target, roundView)
              => \/ TimeoutDirectGoal(target, roundView)
                 \/ TimeoutDirectOwnerFrontier(target, roundView)
                 \/ TimeoutProgressRankedFrontier(
                      target, roundView))
      BY TimeoutDirectReleaseSourceIsRoundStable,
         TimeoutRoundStableExposesRankOrExactOwner,
         AsyncStrongTypeProjectsAsyncType, PTL
    <2> QED BY <2>1, <2>2, <2>3, PTL
  <1> QED BY <1>1

(***************************************************************************
Well-founded rank closure.
***************************************************************************)

THEOREM TimeoutProgressRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    TimeoutProgressRankOrdering,
    TimeoutProgressRankCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset,
   WFLexPairOrdering, SMT
   DEF TimeoutProgressRankOrdering,
       TimeoutProgressRankCarrier

THEOREM TimeoutFiniteRankDescentClosesExactRank ==
  \A initialContext:
    /\ TimeoutDeadlineClockConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSemanticOwnerHandoffProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutFiniteRankDescentProperty(
         AsyncLiveSpecAt(initialContext))
    => (AsyncLiveSpecAt(initialContext)
          => \A target \in AsyncCurrentResponsiveVoters,
                roundView \in Views,
                rank \in TimeoutProgressRankCarrier:
               TimeoutProgressRankFrontier(target, roundView, rank)
                 ~> (TimeoutDirectGoal(target, roundView)
                      \/ TimeoutDirectOwnerFrontier(
                           target, roundView)))
BY TimeoutProgressRankOrderingIsWellFounded,
   WellFoundedLeadsTo
   DEF TimeoutFiniteRankDescentProperty

THEOREM TimeoutFiniteRankDescentClosesRankedFrontier ==
  \A initialContext:
    /\ TimeoutDeadlineClockConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSemanticOwnerHandoffProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutSourceIsolatedDeliveryConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    /\ TimeoutFiniteRankDescentProperty(
         AsyncLiveSpecAt(initialContext))
    => (AsyncLiveSpecAt(initialContext)
          => \A target \in AsyncCurrentResponsiveVoters,
                roundView \in Views:
               TimeoutProgressRankedFrontier(target, roundView)
                 ~> (TimeoutDirectGoal(target, roundView)
                      \/ TimeoutDirectOwnerFrontier(
                           target, roundView)))
BY TimeoutFiniteRankDescentClosesExactRank, PTL
   DEF TimeoutProgressRankedFrontier

THEOREM TimeoutExactOwnerConvergenceClosesOwnerFrontier ==
  \A initialContext:
    TimeoutCertificateAndDecisionConvergenceProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
                 TimeoutDirectOwnerFrontier(target, roundView)
                   ~> TimeoutDirectGoal(target, roundView))
BY PTL
   DEF TimeoutCertificateAndDecisionConvergenceProperty,
       TimeoutDirectOwnerFrontier, TimeoutDirectGoal,
       TimeoutCertificateFormationFrontier

(***************************************************************************
Direct release reduction.

Every premise is below timeout/view progress.  No rotating-leader or aggregate
Decision-convergence result appears in this module's dependency path.
***************************************************************************)

THEOREM DirectTimeoutViewDecompositionClosesTimeoutViewProgress ==
  \A initialContext:
    DirectTimeoutViewClosureResidualProperty(
         AsyncLiveSpecAt(initialContext))
      => TimeoutViewProgressProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                DirectTimeoutViewClosureResidualProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE TimeoutViewProgressProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>0f. ProtectedServiceFiniteRunnerEpisodeClosureProperty(
               AsyncSpecAt(initialContext))
      BY AsyncSpecProvidesProtectedServiceFiniteRunnerEpisodeClosure
    <2>0o. TimeoutViewOwnershipPreservationProperty(
               AsyncLiveSpecAt(initialContext))
      BY TimeoutViewOwnershipPreservationObligation
    <2>0k. TimeoutFixedClockLifecycleOwnerServiceProperty(
               AsyncLiveSpecAt(initialContext))
      BY AsyncLiveProvidesTimeoutFixedClockLifecyclePhysicalKernels,
         TimeoutFixedClockPhysicalKernelsDischargeLifecycleService
    <2>0p. TimeoutArmedWalPhysicalKernelProperties(
               AsyncLiveSpecAt(initialContext))
      BY <2>0f,
         AsyncLiveClosesTimeoutFixedOwnerPriorityTicketNonReplenishment,
         TimeoutFixedOwnerPriorityTicketNonReplenishmentDischargesArmedWalPhysicalKernels
    <2>0w. TimeoutArmedExactWalEndpointProperty(
               AsyncLiveSpecAt(initialContext))
      BY <2>0p,
         TimeoutArmedWalPhysicalKernelsDischargeExactEndpoint
    <2>0v. TimeoutSourceIsolatedDeliveryConvergenceProperty(
               AsyncLiveSpecAt(initialContext))
      BY <1>1,
         TimeoutVotePhysicalKernelsDischargeSourceIsolatedDelivery
         DEF DirectTimeoutViewClosureResidualProperty
    <2>0c. TimeoutCertificateAndDecisionConvergenceProperty(
               AsyncLiveSpecAt(initialContext))
      BY <1>1,
         TimeoutCertificateDecisionPhysicalKernelsDischargeConvergence
         DEF DirectTimeoutViewClosureResidualProperty
    <2>0a. TimeoutFixedClockServicePrerequisites(
               AsyncLiveSpecAt(initialContext))
      BY <2>0k,
         TimeoutLifecycleOwnerServiceSuppliesLiveFixedClockPrerequisites
    <2>0. TimeoutDeadlineClockConvergenceProperty(
              AsyncLiveSpecAt(initialContext))
      BY <2>0a, TimeoutPredeadlineRankDescentClosesDeadlineClock
    <2>1. TimeoutSemanticOwnerHandoffProperty(
            AsyncLiveSpecAt(initialContext))
      BY <2>0f, <2>0w,
         TimeoutSemanticOwnerHandoffFromExactWalEndpoint
    <2>1a. TimeoutFiniteRankDescentProperty(
              AsyncLiveSpecAt(initialContext))
      BY <2>0o, <2>0c, TimeoutFiniteRankCoordinationIsDischarged
    <2>2. AsyncLiveSpecAt(initialContext)
            => \A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
                 TimeoutDirectReleaseSource(target, roundView)
                   ~> (TimeoutDirectGoal(target, roundView)
                        \/ TimeoutDirectOwnerFrontier(
                             target, roundView)
                        \/ TimeoutProgressRankedFrontier(
                             target, roundView))
      BY <2>0o,
         TimeoutOwnershipPreservationSuppliesExactFrontierExposure
    <2>3. AsyncLiveSpecAt(initialContext)
            => \A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
                 TimeoutProgressRankedFrontier(target, roundView)
                   ~> (TimeoutDirectGoal(target, roundView)
                        \/ TimeoutDirectOwnerFrontier(
                             target, roundView))
      BY <2>0, <2>1, <2>0v, <2>1a,
         TimeoutFiniteRankDescentClosesRankedFrontier
    <2>4. AsyncLiveSpecAt(initialContext)
            => \A target \in AsyncCurrentResponsiveVoters,
                  roundView \in Views:
                 TimeoutDirectOwnerFrontier(target, roundView)
                   ~> TimeoutDirectGoal(target, roundView)
      BY <2>0c, TimeoutExactOwnerConvergenceClosesOwnerFrontier
    <2> QED BY <2>2, <2>3, <2>4, PTL
         DEF TimeoutViewProgressProperty,
             TimeoutDirectReleaseSource,
             TimeoutDirectGoal
  <1> QED BY <1>1

(***************************************************************************
Route-neutral exact Candidate and Serve service.

`HistoricalDiscoverySelectedOverduePacket` is global.  Its recipient may be
a current voter, a historical-recovery target, or an applied archive even
when the timeout source belongs to another context.  The shared packet owner
set is therefore route-neutral: Candidate owners use
`ProtectedCandidateOwned` at an exact timed-service node, and the frozen
physical identity records the monotone timed-owner mode and the one concrete
runner action for that mode.  Current-voter and historical runner fairness
remain separate quantified clauses; no fairness of their disjunction is
introduced.  Archive mode is terminal for Candidate ownership because
`ProtectedCandidateOwned` excludes an applied node.

The Candidate non-descent episode covers equal-count owner replacement and
count-increasing lower causal successors.  Its radix-four work tokens are
also multiplied by the finite owner-mode credit, so an action-family handoff
strictly consumes the episode rather than silently changing the fair action.
Serve freezes its exact FIFO occurrence, worker kind, and worker mode and
uses the existing two-element mode descent.  Neither replenishment nor a mode
handoff is called fixed-clock progress.
***************************************************************************)

TimeoutFixedClockCandidateExactActionOwnerAtRank(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, logicalIdentity,
    physicalIdentity, candidate, occurrenceRank, physicalKnown) ==
  LET recipient == packet.item.envelope.recipient
  IN /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
          source, sourceView, clockValue, deadlineValue,
          sourceRank, packet, known, budget)
     /\ HistoricalDiscoveryPacketCandidateOwners(packet) # {}
     /\ candidate =
          HistoricalDiscoveryPacketCandidateDebtWitness(packet)
     /\ candidate.causalOrigin = logicalIdentity
     /\ <<"Candidate", logicalIdentity>> \in known
     /\ physicalKnown =
          HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet(
            packet)
     /\ IsFiniteSet(physicalKnown)
     /\ physicalIdentity =
          HistoricalDiscoveryCandidateExactPhysicalIdentity(candidate)
     /\ occurrenceRank =
          HistoricalDiscoveryCandidateExactPhysicalRank(packet, candidate)
     /\ occurrenceRank
          \in HistoricalDiscoveryCandidateExactPhysicalRankCarrier
     /\ HistoricalDiscoveryCandidateFrozenPhysicalCoordinates(
          physicalIdentity, candidate, occurrenceRank)
     /\ candidate.node = recipient
     /\ ENABLED
          HistoricalDiscoveryCandidateExactRunnerAction(
            packet, physicalIdentity.runnerKind)

TimeoutFixedClockCandidateIntroducedPhysicalIdentitySet(
    packet, physicalKnown) ==
  HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet(packet)
    \ physicalKnown

TimeoutFixedClockCandidateIntroducedOwners(packet, physicalKnown) ==
  {candidate \in HistoricalDiscoveryPacketCandidateOwners(packet):
     HistoricalDiscoveryCandidateExactPhysicalIdentity(candidate)
       \notin physicalKnown}

TimeoutFixedClockCandidateEqualCountReplacementResidual(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, logicalIdentity,
    physicalIdentity, candidate, occurrenceRank, physicalKnown) ==
  /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget)
  /\ ~TimeoutFixedClockLifecycleGoal(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget)
  /\ candidate.causalOrigin = logicalIdentity
  /\ occurrenceRank
       \in HistoricalDiscoveryCandidateExactPhysicalRankCarrier
  /\ HistoricalDiscoveryCandidateFrozenPhysicalCoordinates(
       physicalIdentity, candidate, occurrenceRank)
  /\ IsFiniteSet(physicalKnown)
  /\ Cardinality(HistoricalDiscoveryPacketCandidateOwners(packet))
       = occurrenceRank[1][1]
  /\ TimeoutFixedClockCandidateIntroducedPhysicalIdentitySet(
       packet, physicalKnown) # {}

TimeoutFixedClockCandidateIncreasingReplenishmentResidual(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, logicalIdentity,
    physicalIdentity, candidate, occurrenceRank, physicalKnown) ==
  /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget)
  /\ ~TimeoutFixedClockLifecycleGoal(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget)
  /\ candidate.causalOrigin = logicalIdentity
  /\ occurrenceRank
       \in HistoricalDiscoveryCandidateExactPhysicalRankCarrier
  /\ HistoricalDiscoveryCandidateFrozenPhysicalCoordinates(
       physicalIdentity, candidate, occurrenceRank)
  /\ IsFiniteSet(physicalKnown)
  /\ Cardinality(HistoricalDiscoveryPacketCandidateOwners(packet))
       > occurrenceRank[1][1]
  /\ TimeoutFixedClockCandidateIntroducedPhysicalIdentitySet(
       packet, physicalKnown) # {}

TimeoutFixedClockCandidateNonDescentEpisodeResidual(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, logicalIdentity,
    physicalIdentity, candidate, occurrenceRank, physicalKnown) ==
  \/ TimeoutFixedClockCandidateEqualCountReplacementResidual(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget, logicalIdentity,
       physicalIdentity, candidate, occurrenceRank, physicalKnown)
  \/ TimeoutFixedClockCandidateIncreasingReplenishmentResidual(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget, logicalIdentity,
       physicalIdentity, candidate, occurrenceRank, physicalKnown)

TimeoutFixedClockCandidateCausalDagFrontier(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, logicalIdentity,
    physicalIdentity, candidate, occurrenceRank, physicalKnown,
    workBudget) ==
  /\ TimeoutFixedClockCandidateNonDescentEpisodeResidual(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget, logicalIdentity,
       physicalIdentity, candidate, occurrenceRank, physicalKnown)
  /\ workBudget \in Nat
  /\ workBudget = HistoricalDiscoveryCandidateCausalWorkBudget(packet)

TimeoutFixedClockCandidateStrictCausalDagGoal(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, logicalIdentity,
    physicalIdentity, candidate, occurrenceRank, physicalKnown,
    workBudget) ==
  \/ TimeoutFixedClockLifecycleGoal(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget)
  \/ \E lowerWorkBudget \in
       SetLessThan(workBudget, OpToRel(<, Nat), Nat):
       TimeoutFixedClockCandidateCausalDagFrontier(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, packet, known, budget, logicalIdentity,
         physicalIdentity, candidate, occurrenceRank, physicalKnown,
         lowerWorkBudget)

TimeoutFixedClockCandidateCausalDagWitnessEpisode(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, logicalIdentity,
    physicalIdentity, candidate, occurrenceRank, physicalKnown,
    workBudget, witness) ==
  /\ TimeoutFixedClockCandidateCausalDagFrontier(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget, logicalIdentity,
       physicalIdentity, candidate, occurrenceRank, physicalKnown,
       workBudget)
  /\ witness \in
       TimeoutFixedClockCandidateIntroducedOwners(
         packet, physicalKnown)

THEOREM TimeoutFixedClockKnownCandidateHasExactActionOwner ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known, logicalIdentity:
      \A budget \in Nat:
        /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
             source, sourceView, clockValue, deadlineValue,
             sourceRank, packet, known, budget)
        /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
        /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
        /\ <<"Candidate", logicalIdentity>>
             \in TimeoutFixedPacketLiveOwners(packet)
        /\ <<"Candidate", logicalIdentity>> \in known
        => \E physicalIdentity, candidate, occurrenceRank, physicalKnown:
             TimeoutFixedClockCandidateExactActionOwnerAtRank(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, packet, known, budget, logicalIdentity,
               physicalIdentity, candidate, occurrenceRank, physicalKnown)
BY HistoricalDiscoveryLiveCandidateDebtHasExactFairOwner,
   HistoricalDiscoveryPacketOccurrenceDebtRanksInCarrier,
   ScheduledCandidateServiceRankInCarrier,
   GstResponsiveUnappliedRunNodeIsEnabled,
   GstHistoricalRecoveryRunNodeIsEnabled,
   GstHistoricalServerIsEnabled,
   StrongTypeHasFiniteHistoricalDiscoveryRankOwners,
   FS_Image, IsaT(1200)
   DEF TimeoutFixedClockCandidateExactActionOwnerAtRank,
       TimeoutFixedClockLifecycleEpisodeAtBudget,
       TimeoutFixedPacketLiveOwners,
       TimeoutFixedPacketLiveCandidateIdentities,
       HistoricalDiscoveryCandidateExactPhysicalRank,
       HistoricalDiscoveryCandidateExactPhysicalRankCarrier,
       HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet,
       HistoricalDiscoveryCandidateExactPhysicalIdentity,
       HistoricalDiscoveryCandidateFrozenPhysicalCoordinates,
       HistoricalDiscoveryCandidateExactRunnerAction,
       HistoricalDiscoveryCandidateExactRunnerActionKindCarrier,
       HistoricalDiscoveryCandidateExactRunnerKindForMode,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       HistoricalDiscoveryPacketCandidateOwners,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       HistoricalRecoveryTarget

THEOREM TimeoutFixedClockCandidateExactStepIsGoalNonDescentOrFrame ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known, logicalIdentity,
       physicalIdentity, candidate, occurrenceRank, physicalKnown:
      \A budget \in Nat:
        /\ TimeoutFixedClockCandidateExactActionOwnerAtRank(
             source, sourceView, clockValue, deadlineValue,
             sourceRank, packet, known, budget, logicalIdentity,
             physicalIdentity, candidate, occurrenceRank, physicalKnown)
        /\ [AsyncNext]_AsyncAllVars
        => \/ TimeoutFixedClockLifecycleGoal(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, packet, known, budget)'
           \/ TimeoutFixedClockCandidateNonDescentEpisodeResidual(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, packet, known, budget, logicalIdentity,
                physicalIdentity, candidate,
                occurrenceRank, physicalKnown)'
           \/ TimeoutFixedClockCandidateExactActionOwnerAtRank(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, packet, known, budget, logicalIdentity,
                physicalIdentity, candidate,
                occurrenceRank, physicalKnown)'
BY HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryCandidateMinimumPersistenceKeepsTail,
   HistoricalDiscoveryCandidateMinimumExitClassifiesTail,
   HistoricalDiscoveryCandidateExitClassifiesOccurrenceDebt,
   HistoricalDiscoveryLowerCandidateInsertionReselectsLower,
   HistoricalDiscoveryCandidateDepartureRetainsLifecycleCoverage,
   HistoricalDiscoverySameGenerationCandidateServiceBlocksReentry,
   HistoricalDiscoveryServicedCandidateIdentityBlocksReentry,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   AsyncCausalEpisodeFrozenOriginsCannotReplenish,
   AsyncCommandSuccessorsStrictlyLowerRemainingWorkStage,
   IsaT(3600)
   DEF TimeoutFixedClockCandidateExactActionOwnerAtRank,
       TimeoutFixedClockCandidateNonDescentEpisodeResidual,
       TimeoutFixedClockCandidateEqualCountReplacementResidual,
       TimeoutFixedClockCandidateIncreasingReplenishmentResidual,
       TimeoutFixedClockCandidateIntroducedPhysicalIdentitySet,
       TimeoutFixedClockLifecycleGoal,
       TimeoutFixedClockLifecycleDiscovery,
       TimeoutFixedClockLifecycleEpisodeAtBudget,
       TimeoutFixedClockStrictRankGoal,
       TimeoutFixedClockBlockedAtRank,
       HistoricalDiscoveryCandidateExactPhysicalIdentity,
       HistoricalDiscoveryCandidateFrozenPhysicalCoordinates,
       HistoricalDiscoveryCandidateExactRunnerAction,
       HistoricalDiscoveryCandidateExactRunnerActionKindCarrier,
       HistoricalDiscoveryCandidateExactRunnerKindForMode,
       HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet,
       HistoricalDiscoveryPacketCandidateOccurrenceDebtRank,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       HistoricalDiscoveryConcreteFixedClockRank,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       AsyncTargetNeutralLifecycleDiscoveredOwnerSet,
       HistoricalDiscoveryTimedProtectedCandidateOwned,
       ProtectedCandidateOwned, CandidateScheduled,
       CandidateServiceRank, ServiceRankLess,
       LexPairOrdering, OpToRel, AsyncAllVars

TimeoutFixedClockCandidateExactRunnerStepProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known, logicalIdentity,
            physicalIdentity, candidate, occurrenceRank, physicalKnown:
           \A budget \in Nat:
             TimeoutFixedClockCandidateExactActionOwnerAtRank(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, packet, known, budget, logicalIdentity,
               physicalIdentity, candidate, occurrenceRank, physicalKnown)
               ~> (TimeoutFixedClockLifecycleGoal(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank, packet, known, budget)
                    \/ TimeoutFixedClockCandidateNonDescentEpisodeResidual(
                         source, sourceView, clockValue, deadlineValue,
                         sourceRank, packet, known, budget, logicalIdentity,
                         physicalIdentity, candidate,
                         occurrenceRank, physicalKnown))

THEOREM AsyncSpecProvidesTimeoutFixedClockCandidateExactRunnerStep ==
  \A initialContext:
    TimeoutFixedClockCandidateExactRunnerStepProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalDiscoveryTimedCandidateStarvation,
   TimeoutFixedClockCandidateExactStepIsGoalNonDescentOrFrame,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   PTL, IsaT(2400)
   DEF TimeoutFixedClockCandidateExactRunnerStepProperty,
       TimeoutFixedClockCandidateExactActionOwnerAtRank,
       HistoricalDiscoveryTimedCandidateStarvationProperty,
       HistoricalDiscoveryTimedProtectedCandidateOwned,
       ProtectedCandidateOwned

THEOREM TimeoutFixedClockCandidateCausalFrontierHasProtectedWitness ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known, logicalIdentity,
       physicalIdentity, candidate, occurrenceRank, physicalKnown:
      \A budget, workBudget \in Nat:
        TimeoutFixedClockCandidateCausalDagFrontier(
          source, sourceView, clockValue, deadlineValue,
          sourceRank, packet, known, budget, logicalIdentity,
          physicalIdentity, candidate, occurrenceRank, physicalKnown,
          workBudget)
          => \E witness:
               /\ TimeoutFixedClockCandidateCausalDagWitnessEpisode(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank, packet, known, budget, logicalIdentity,
                    physicalIdentity, candidate, occurrenceRank,
                    physicalKnown, workBudget, witness)
               /\ witness \in AsyncCandidateSet
               /\ HistoricalDiscoveryTimedProtectedCandidateOwned(witness)
BY StrongTypeHasFiniteHistoricalDiscoveryRankOwners,
   TimeoutFixedPacketLiveAndCoveredOwnersStayFrozen,
   IsaT(1200)
   DEF TimeoutFixedClockCandidateCausalDagWitnessEpisode,
       TimeoutFixedClockCandidateCausalDagFrontier,
       TimeoutFixedClockCandidateNonDescentEpisodeResidual,
       TimeoutFixedClockCandidateEqualCountReplacementResidual,
       TimeoutFixedClockCandidateIncreasingReplenishmentResidual,
       TimeoutFixedClockCandidateIntroducedPhysicalIdentitySet,
       TimeoutFixedClockCandidateIntroducedOwners,
       HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet,
       HistoricalDiscoveryCandidateExactPhysicalIdentity,
       HistoricalDiscoveryPacketCandidateOwners,
       HistoricalDiscoveryTimedProtectedCandidateOwned,
       TimeoutFixedClockLifecycleEpisodeAtBudget,
       TimeoutFixedClockPending,
       AsyncTimedServiceNodes, ProtectedCandidateOwned

THEOREM TimeoutFixedClockCandidateCausalWitnessStepIsGoalDescentOrFrame ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known, logicalIdentity,
       physicalIdentity, candidate, occurrenceRank, physicalKnown,
       witness:
      \A budget, workBudget \in Nat:
        /\ TimeoutFixedClockCandidateCausalDagWitnessEpisode(
             source, sourceView, clockValue, deadlineValue,
             sourceRank, packet, known, budget, logicalIdentity,
             physicalIdentity, candidate, occurrenceRank, physicalKnown,
             workBudget, witness)
        /\ [AsyncNext]_AsyncAllVars
        => \/ TimeoutFixedClockCandidateStrictCausalDagGoal(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, packet, known, budget, logicalIdentity,
                physicalIdentity, candidate, occurrenceRank,
                physicalKnown, workBudget)'
           \/ TimeoutFixedClockCandidateCausalDagWitnessEpisode(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, packet, known, budget, logicalIdentity,
                physicalIdentity, candidate, occurrenceRank,
                physicalKnown, workBudget, witness)'
BY HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryCandidateDepartureRetainsLifecycleCoverage,
   HistoricalDiscoverySameGenerationCandidateServiceBlocksReentry,
   HistoricalDiscoveryServicedCandidateIdentityBlocksReentry,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   CommandSuccessorsRetainCausalOrigin,
   AsyncCommandSuccessorsStrictlyLowerRemainingWorkStage,
   AsyncCommandSuccessorBatchStrictlyConsumesRemainingWork,
   AsyncCausalEpisodeFrozenOriginsCannotReplenish,
   AsyncCausalEpisodeServicedCandidateConsumesTopologicalWeight,
   FS_CardinalityType, FS_Subset, IsaT(3600)
   DEF TimeoutFixedClockCandidateCausalDagWitnessEpisode,
       TimeoutFixedClockCandidateCausalDagFrontier,
       TimeoutFixedClockCandidateStrictCausalDagGoal,
       TimeoutFixedClockCandidateNonDescentEpisodeResidual,
       TimeoutFixedClockCandidateEqualCountReplacementResidual,
       TimeoutFixedClockCandidateIncreasingReplenishmentResidual,
       TimeoutFixedClockCandidateIntroducedPhysicalIdentitySet,
       TimeoutFixedClockCandidateIntroducedOwners,
       TimeoutFixedClockLifecycleGoal,
       TimeoutFixedClockLifecycleEpisodeAtBudget,
       HistoricalDiscoveryCandidateCausalWorkBudget,
       HistoricalDiscoveryCandidateCausalWorkTokenSet,
       HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet,
       HistoricalDiscoveryCandidateExactPhysicalIdentity,
       HistoricalDiscoveryPacketCandidateOwners,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       AsyncCausalEpisodeCandidateWorkBudget,
       AsyncCausalEpisodeCandidateWorkTokens,
       AsyncCausalEpisodeCandidates,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       CandidateScheduled, CommandSuccessors,
       LexPairOrdering, OpToRel, SetLessThan, AsyncAllVars

TimeoutFixedClockCandidateCausalDagBudgetDescentProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known, logicalIdentity,
            physicalIdentity, candidate, occurrenceRank, physicalKnown:
           \A budget, workBudget \in Nat:
             TimeoutFixedClockCandidateCausalDagFrontier(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, packet, known, budget, logicalIdentity,
               physicalIdentity, candidate, occurrenceRank, physicalKnown,
               workBudget)
               ~> TimeoutFixedClockCandidateStrictCausalDagGoal(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank, packet, known, budget, logicalIdentity,
                    physicalIdentity, candidate, occurrenceRank,
                    physicalKnown, workBudget)

THEOREM AsyncSpecProvidesTimeoutFixedClockCandidateCausalDagBudgetDescent ==
  \A initialContext:
    TimeoutFixedClockCandidateCausalDagBudgetDescentProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalDiscoveryTimedCandidateStarvation,
   TimeoutFixedClockCandidateCausalFrontierHasProtectedWitness,
   TimeoutFixedClockCandidateCausalWitnessStepIsGoalDescentOrFrame,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   PTL, IsaT(3000)
   DEF TimeoutFixedClockCandidateCausalDagBudgetDescentProperty,
       TimeoutFixedClockCandidateCausalDagWitnessEpisode,
       HistoricalDiscoveryTimedCandidateStarvationProperty,
       HistoricalDiscoveryTimedProtectedCandidateOwned,
       ProtectedCandidateOwned

TimeoutFixedClockCandidateNonDescentClosureProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known, logicalIdentity,
            physicalIdentity, candidate, occurrenceRank, physicalKnown:
           \A budget \in Nat:
             TimeoutFixedClockCandidateNonDescentEpisodeResidual(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, packet, known, budget, logicalIdentity,
               physicalIdentity, candidate, occurrenceRank, physicalKnown)
               ~> TimeoutFixedClockLifecycleGoal(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank, packet, known, budget)

THEOREM TimeoutFixedClockFiniteCausalDagClosesNonDescentEpisode ==
  \A specification:
    TimeoutFixedClockCandidateCausalDagBudgetDescentProperty(specification)
      => TimeoutFixedClockCandidateNonDescentClosureProperty(specification)
BY HistoricalDiscoveryCandidateCausalWorkBudgetIsNatural,
   NatLessThanWellFounded, WellFoundedLeadsTo, Isa, PTL
   DEF TimeoutFixedClockCandidateCausalDagBudgetDescentProperty,
       TimeoutFixedClockCandidateNonDescentClosureProperty,
       TimeoutFixedClockCandidateCausalDagFrontier,
       TimeoutFixedClockCandidateStrictCausalDagGoal

THEOREM AsyncSpecProvidesTimeoutFixedClockCandidateLifecycleKernel ==
  \A initialContext:
    TimeoutFixedClockCandidateLifecycleKernelProperty(
      AsyncSpecAt(initialContext))
BY TimeoutFixedClockKnownCandidateHasExactActionOwner,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AsyncSpecProvidesTimeoutFixedClockCandidateExactRunnerStep,
   AsyncSpecProvidesTimeoutFixedClockCandidateCausalDagBudgetDescent,
   TimeoutFixedClockFiniteCausalDagClosesNonDescentEpisode,
   PTL, IsaT(2400)
   DEF TimeoutFixedClockCandidateLifecycleKernelProperty,
       TimeoutFixedClockCandidateExactRunnerStepProperty,
       TimeoutFixedClockCandidateNonDescentClosureProperty

(***************************************************************************
Exact Serve occurrence with immutable worker mode.
***************************************************************************)

TimeoutFixedClockServeExactActionOwnerAtRank(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, logicalIdentity,
    job, occurrenceRank, workerKind, workerMode) ==
  LET recipient == packet.item.envelope.recipient
  IN /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
          source, sourceView, clockValue, deadlineValue,
          sourceRank, packet, known, budget)
     /\ HistoricalDiscoveryPacketCandidateOwners(packet) = {}
     /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
     /\ job = HistoricalDiscoveryPacketServeDebtWitness(packet)
     /\ logicalIdentity = AsyncIoServeJobIdentity(recipient, job)
     /\ <<"Serve", logicalIdentity>> \in known
     /\ occurrenceRank =
          HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)
     /\ occurrenceRank \in HistoricalDiscoveryOccurrenceDebtCarrier
     /\ workerMode
          \in HistoricalDiscoveryServeExactWorkerModeCarrier
     /\ workerMode = HistoricalDiscoveryTimedOwnerMode(recipient)
     /\ workerKind
          \in HistoricalDiscoveryServeExactWorkerActionKindCarrier
     /\ workerKind =
          HistoricalDiscoveryServeExactWorkerKindForMode(workerMode)
     /\ ENABLED
          HistoricalDiscoveryServeExactWorkerAction(packet, workerKind)

TimeoutFixedClockServeExactWorkerModeFrontier(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, logicalIdentity,
    job, occurrenceRank, workerMode) ==
  \E workerKind \in
       HistoricalDiscoveryServeExactWorkerActionKindCarrier:
    TimeoutFixedClockServeExactActionOwnerAtRank(
      source, sourceView, clockValue, deadlineValue,
      sourceRank, packet, known, budget, logicalIdentity,
      job, occurrenceRank, workerKind, workerMode)

TimeoutFixedClockServeExactWorkerModeHandoffResidual(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, logicalIdentity,
    job, occurrenceRank, workerMode) ==
  /\ ~TimeoutFixedClockLifecycleGoal(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget)
  /\ \E lowerMode \in
       SetLessThan(
         workerMode,
         HistoricalDiscoveryServeExactWorkerModeOrdering,
         HistoricalDiscoveryServeExactWorkerModeCarrier):
       TimeoutFixedClockServeExactWorkerModeFrontier(
         source, sourceView, clockValue, deadlineValue,
         sourceRank, packet, known, budget, logicalIdentity,
         job, occurrenceRank, lowerMode)

TimeoutFixedClockServeExactWorkerModeProgressGoal(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, logicalIdentity,
    job, occurrenceRank, workerMode) ==
  \/ TimeoutFixedClockLifecycleGoal(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget)
  \/ TimeoutFixedClockServeExactWorkerModeHandoffResidual(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget, logicalIdentity,
       job, occurrenceRank, workerMode)

THEOREM TimeoutFixedClockKnownServeHasExactActionOwner ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known, logicalIdentity:
      \A budget \in Nat:
        /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
             source, sourceView, clockValue, deadlineValue,
             sourceRank, packet, known, budget)
        /\ HistoricalDiscoveryPacketCandidateOwners(packet) = {}
        /\ <<"Serve", logicalIdentity>>
             \in TimeoutFixedPacketLiveOwners(packet)
        /\ <<"Serve", logicalIdentity>> \in known
        => \E job, occurrenceRank, workerKind, workerMode:
             TimeoutFixedClockServeExactActionOwnerAtRank(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, packet, known, budget, logicalIdentity,
               job, occurrenceRank, workerKind, workerMode)
BY HistoricalDiscoveryLiveServeDebtHasExactFairOwner,
   HistoricalDiscoveryPacketOccurrenceDebtRanksInCarrier,
   GstHistoricalIoWorkerIsEnabled,
   QueuedIoEnablesPostGstService,
   IsaT(1200)
   DEF TimeoutFixedClockServeExactActionOwnerAtRank,
       TimeoutFixedClockLifecycleEpisodeAtBudget,
       TimeoutFixedPacketLiveOwners,
       TimeoutFixedPacketLiveServeIdentities,
       HistoricalDiscoveryPacketServeDebtFairAction,
       HistoricalDiscoveryServeExactWorkerAction,
       HistoricalDiscoveryServeExactWorkerActionKindCarrier,
       HistoricalDiscoveryServeExactWorkerModeCarrier,
       HistoricalDiscoveryServeExactWorkerKindForMode,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryServeJobOwned,
       HistoricalDiscoveryIoOwners,
       ActiveIoJobs, AsyncIoQueueDepth,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       HistoricalRecoveryTarget

THEOREM TimeoutFixedClockServeExactWorkerStepIsModeGoalOrFrame ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known, logicalIdentity,
       job, occurrenceRank, workerKind, workerMode:
      \A budget \in Nat:
        /\ TimeoutFixedClockServeExactActionOwnerAtRank(
             source, sourceView, clockValue, deadlineValue,
             sourceRank, packet, known, budget, logicalIdentity,
             job, occurrenceRank, workerKind, workerMode)
        /\ [AsyncNext]_AsyncAllVars
        => \/ TimeoutFixedClockServeExactWorkerModeProgressGoal(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, packet, known, budget, logicalIdentity,
                job, occurrenceRank, workerMode)'
           \/ TimeoutFixedClockServeExactActionOwnerAtRank(
                source, sourceView, clockValue, deadlineValue,
                sourceRank, packet, known, budget, logicalIdentity,
                job, occurrenceRank, workerKind, workerMode)'
BY HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryServeMinimumPersistenceKeepsTail,
   HistoricalDiscoveryServeMinimumExitClassifiesTail,
   HistoricalDiscoveryServeExitEitherLowersOrReplenishes,
   HistoricalDiscoveryRetiredServeIdentityBlocksReentry,
   HistoricalDiscoveryServeDepartureInstallsDurableCoverage,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   IsaT(3000)
   DEF TimeoutFixedClockServeExactActionOwnerAtRank,
       TimeoutFixedClockServeExactWorkerModeProgressGoal,
       TimeoutFixedClockServeExactWorkerModeHandoffResidual,
       TimeoutFixedClockServeExactWorkerModeFrontier,
       TimeoutFixedClockLifecycleGoal,
       TimeoutFixedClockLifecycleEpisodeAtBudget,
       TimeoutFixedClockStrictRankGoal,
       TimeoutFixedClockBlockedAtRank,
       HistoricalDiscoveryServeExactWorkerAction,
       HistoricalDiscoveryServeExactWorkerKindForMode,
       HistoricalDiscoveryServeExactWorkerModeOrdering,
       HistoricalDiscoveryServeExactWorkerModeCarrier,
       HistoricalDiscoveryPacketServeOccurrenceDebtRank,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryServeJobOwned,
       AsyncServeJobQueued, AsyncServeLifecycleTombstone,
       SetLessThan, OpToRel, LexPairOrdering, AsyncAllVars

THEOREM TimeoutFixedClockServeExactFairActionConsumesModeCell ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known, logicalIdentity,
       job, occurrenceRank, workerKind, workerMode:
      \A budget \in Nat:
        /\ TimeoutFixedClockServeExactActionOwnerAtRank(
             source, sourceView, clockValue, deadlineValue,
             sourceRank, packet, known, budget, logicalIdentity,
             job, occurrenceRank, workerKind, workerMode)
        /\ HistoricalDiscoveryServeExactWorkerAction(packet, workerKind)
        => TimeoutFixedClockServeExactWorkerModeProgressGoal(
             source, sourceView, clockValue, deadlineValue,
             sourceRank, packet, known, budget, logicalIdentity,
             job, occurrenceRank, workerMode)'
BY HistoricalDiscoveryServeFairActionLowersOccurrenceDebt,
   HistoricalDiscoveryServeHeadFairServiceLowersOccurrenceDebt,
   HistoricalDiscoveryServeNonOwnerHeadFairServiceLowersMinimum,
   HistoricalDiscoveryServeExactRemovalLowersOccurrenceDebt,
   IsaT(2400)
   DEF TimeoutFixedClockServeExactActionOwnerAtRank,
       TimeoutFixedClockServeExactWorkerModeProgressGoal,
       TimeoutFixedClockServeExactWorkerModeHandoffResidual,
       TimeoutFixedClockServeExactWorkerModeFrontier,
       TimeoutFixedClockLifecycleGoal,
       TimeoutFixedClockStrictRankGoal,
       TimeoutFixedClockBlockedAtRank,
       HistoricalDiscoveryServeExactWorkerAction,
       HistoricalDiscoveryPacketServeDebtFairAction,
       HistoricalDiscoveryPacketServeOccurrenceDebtRank,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       HistoricalDiscoveryPacketServeHeadIsOwner,
       HistoricalDiscoveryPacketServeHeadIsNotOwner,
       SetLessThan, OpToRel, LexPairOrdering, AsyncAllVars

TimeoutFixedClockServeExactWorkerStepProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known, logicalIdentity,
            job, occurrenceRank, workerKind, workerMode:
           \A budget \in Nat:
             TimeoutFixedClockServeExactActionOwnerAtRank(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, packet, known, budget, logicalIdentity,
               job, occurrenceRank, workerKind, workerMode)
               ~> TimeoutFixedClockServeExactWorkerModeProgressGoal(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank, packet, known, budget, logicalIdentity,
                    job, occurrenceRank, workerMode)

THEOREM AsyncSpecProvidesTimeoutFixedClockServeExactWorkerStep ==
  \A initialContext:
    TimeoutFixedClockServeExactWorkerStepProperty(
      AsyncSpecAt(initialContext))
BY TimeoutFixedClockServeExactWorkerStepIsModeGoalOrFrame,
   TimeoutFixedClockServeExactFairActionConsumesModeCell,
   HistoricalDiscoveryServeExactWorkerUsesAsyncFairness,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   PTL, IsaT(2400)
   DEF TimeoutFixedClockServeExactWorkerStepProperty,
       TimeoutFixedClockServeExactActionOwnerAtRank,
       HistoricalDiscoveryServeExactWorkerAction,
       HistoricalDiscoveryServeExactWorkerActionKindCarrier

TimeoutFixedClockServeExactWorkerModeClosureProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known, logicalIdentity, job, occurrenceRank:
           \A budget \in Nat:
             \A workerMode \in
                 HistoricalDiscoveryServeExactWorkerModeCarrier:
               TimeoutFixedClockServeExactWorkerModeFrontier(
                 source, sourceView, clockValue, deadlineValue,
                 sourceRank, packet, known, budget, logicalIdentity,
                 job, occurrenceRank, workerMode)
                 ~> TimeoutFixedClockLifecycleGoal(
                      source, sourceView, clockValue, deadlineValue,
                      sourceRank, packet, known, budget)

THEOREM TimeoutFixedClockServeModeDescentClosesExactWorker ==
  \A specification:
    TimeoutFixedClockServeExactWorkerStepProperty(specification)
      => TimeoutFixedClockServeExactWorkerModeClosureProperty(
           specification)
BY HistoricalDiscoveryServeExactWorkerModeOrderingIsWellFounded,
   WellFoundedLeadsTo, Isa, PTL
   DEF TimeoutFixedClockServeExactWorkerStepProperty,
       TimeoutFixedClockServeExactWorkerModeClosureProperty,
       TimeoutFixedClockServeExactWorkerModeFrontier,
       TimeoutFixedClockServeExactWorkerModeProgressGoal,
       TimeoutFixedClockServeExactWorkerModeHandoffResidual

THEOREM AsyncSpecProvidesTimeoutFixedClockServeLifecycleKernel ==
  \A initialContext:
    TimeoutFixedClockServeLifecycleKernelProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesTimeoutFixedClockCandidateLifecycleKernel,
   TimeoutFixedClockKnownServeHasExactActionOwner,
   AsyncSpecProvidesTimeoutFixedClockServeExactWorkerStep,
   TimeoutFixedClockServeModeDescentClosesExactWorker,
   PTL, IsaT(3000)
   DEF TimeoutFixedClockServeLifecycleKernelProperty,
       TimeoutFixedClockCandidateLifecycleKernelProperty,
       TimeoutFixedClockServeExactWorkerStepProperty,
       TimeoutFixedClockServeExactWorkerModeClosureProperty,
       TimeoutFixedClockLifecycleGoal,
       TimeoutFixedClockLifecycleDiscovery,
       AsyncTargetNeutralLifecycleDiscoveredOwnerSet,
       TimeoutFixedPacketLiveOwners

TimeoutFixedClockDerivedCandidateServeKernelProperties(specification) ==
  /\ TimeoutFixedClockCandidateLifecycleKernelProperty(specification)
  /\ TimeoutFixedClockServeLifecycleKernelProperty(specification)

THEOREM AsyncSpecProvidesTimeoutFixedClockDerivedCandidateServeKernels ==
  \A initialContext:
    TimeoutFixedClockDerivedCandidateServeKernelProperties(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesTimeoutFixedClockCandidateLifecycleKernel,
   AsyncSpecProvidesTimeoutFixedClockServeLifecycleKernel
   DEF TimeoutFixedClockDerivedCandidateServeKernelProperties

(***************************************************************************
Frozen exact packet action.

The packet-only arm is intentionally below Candidate/Serve admission.  Its
owner is one member of the existing seven-action family, retained as an
ordinary quantified value.  No fairness of their disjunction is introduced:
each member is already an individually fair action of `AsyncFairnessAt`.
An unrelated step must either expose strict blocker-rank descent or a newly
live lifecycle identity, or preserve the same enabled member.  This is the
missing handoff argument which an enabled-action case split alone cannot
supply.
***************************************************************************)

TimeoutFixedClockPacketConcreteActionPending(
    source, sourceView, clockValue, deadlineValue,
    sourceRank, packet, known, budget, actionKind, actionSource) ==
  /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
       source, sourceView, clockValue, deadlineValue,
       sourceRank, packet, known, budget)
  /\ TimeoutFixedPacketLiveOwners(packet) = {}
  /\ actionKind
       \in HistoricalDiscoveryPacketConcreteActionKindCarrier
  /\ actionSource \in AsyncIngressSources
  /\ ENABLED
       HistoricalDiscoveryPacketConcreteAction(
         packet, actionKind, actionSource)

THEOREM TimeoutFixedClockPacketTailHasFrozenConcreteAction ==
  \A initialContext \in ContextRecords:
    \A source \in AsyncCurrentResponsiveVoters,
       sourceView \in Views,
       clockValue, deadlineValue \in Nat,
       sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
      \A packet, known:
        \A budget \in Nat:
          /\ AsyncFrozenContextAt(initialContext)
          /\ PostGstReplayQuarantineExcluded
          /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
          /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
          /\ TimeoutFixedClockLifecycleEpisodeAtBudget(
               source, sourceView, clockValue, deadlineValue,
               sourceRank, packet, known, budget)
          /\ TimeoutFixedPacketLiveOwners(packet) = {}
          => \E actionKind
                   \in HistoricalDiscoveryPacketConcreteActionKindCarrier:
               \E actionSource \in AsyncIngressSources:
                 TimeoutFixedClockPacketConcreteActionPending(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, packet, known, budget,
                   actionKind, actionSource)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW source \in AsyncCurrentResponsiveVoters,
                NEW sourceView \in Views,
                NEW clockValue, NEW deadlineValue \in Nat,
                NEW sourceRank
                  \in HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW packet, NEW known, NEW budget \in Nat,
                AsyncFrozenContextAt(initialContext),
                PostGstReplayQuarantineExcluded,
                AsyncCandidateProducerContinuationExternalCoverageInvariant,
                AsyncCandidateProducerContinuationLocalReplayCapacityInvariant,
                TimeoutFixedClockLifecycleEpisodeAtBudget(
                  source, sourceView, clockValue, deadlineValue,
                  sourceRank, packet, known, budget),
                TimeoutFixedPacketLiveOwners(packet) = {}
         PROVE \E actionKind
                   \in HistoricalDiscoveryPacketConcreteActionKindCarrier:
                 \E actionSource \in AsyncIngressSources:
                   TimeoutFixedClockPacketConcreteActionPending(
                     source, sourceView, clockValue, deadlineValue,
                     sourceRank, packet, known, budget,
                     actionKind, actionSource)
    <2> DEFINE Recipient == packet.item.envelope.recipient
    <2> DEFINE Source == packet.item.source
    <2> DEFINE Item == OldestDueSourcePacket(Recipient, Source).item
    <2>1. /\ AsyncStrongTypeInvariant
           /\ gst
           /\ packet \in OverdueResponsivePackets
           /\ packet = HistoricalDiscoverySelectedOverduePacket
      BY <1>1 DEF TimeoutFixedClockLifecycleEpisodeAtBudget,
                    TimeoutFixedClockPending
    <2>2. /\ AsyncTypeInvariant
           /\ AsyncCurrentResponsiveVoters =
                AsyncVotersAt(initialContext)
      BY <1>1, <2>1, AsyncStrongTypeProjectsAsyncType,
         FrozenContextFixesResponsiveVoters
    <2>3. /\ Recipient \in ValidatorIds
           /\ Source \in AsyncIngressSources
           /\ DueSourcePackets(Recipient, Source) # {}
           /\ ResponsivePacketPairAt(
                initialContext, Recipient, Source)
           /\ Recipient \in AsyncCurrentResponsiveVoters
                           \cup asyncHistoricalRecoveryTargets
      BY <2>1, <2>2,
         OverdueResponsivePacketUsesFairIngressPair, Isa
         DEF Recipient, Source, ResponsivePacketPairAt,
             HistoricalRecoveryPacketCorridor,
             HistoricalRecoveryTarget,
             AsyncArchiveIoServiceNodes,
             AsyncResponsiveAppliedArchiveServers,
             AsyncResponsiveOnlineArchiveServers,
             AsyncResponsiveArchiveServers
    <2>4. /\ AsyncItemTyped(Item)
           /\ Item.envelope.recipient = Recipient
           /\ Item.source = Source
      BY <2>2, <2>3, OldestDueSourcePacketFacts
         DEF Item, AsyncPacketTyped
    <2>5. CASE DueIngressPacketCanEnter(Recipient, Source)
      <3>1. ENABLED
               (PostGstAdmitHiddenPacket(Recipient, Source)
                  \/ PostGstAdmitHistoricalRecoveryPacket(
                       Recipient, Source))
        BY <1>1, <2>1, <2>3, <2>5,
           DueIngressPacketAdmissionIsEnabled
      <3> QED BY <1>1, <3>1, ExpandENABLED, Isa
           DEF TimeoutFixedClockPacketConcreteActionPending,
               HistoricalDiscoveryPacketConcreteAction,
               HistoricalDiscoveryPacketConcreteActionKindCarrier,
               Recipient, Source
    <2>6. CASE ~DueIngressPacketCanEnter(Recipient, Source)
      <3>1. CASE ~NodeHasApplication(Recipient)
        <4>1. CASE Recipient \in asyncHistoricalRecoveryTargets
          <5>1. ENABLED
                   PostGstRunHistoricalRecoveryNode(Recipient)
            BY <2>1, <4>1,
               GstHistoricalRecoveryRunNodeIsEnabled
          <5> QED BY <1>1, <5>1, Isa
               DEF TimeoutFixedClockPacketConcreteActionPending,
                   HistoricalDiscoveryPacketConcreteAction,
                   HistoricalDiscoveryPacketConcreteActionKindCarrier,
                   Recipient
        <4>2. CASE Recipient \notin asyncHistoricalRecoveryTargets
          <5>1. Recipient \in AsyncCurrentResponsiveVoters
            BY <2>3, <4>2
          <5>2. ENABLED PostGstRunNode(Recipient)
            BY <2>1, <3>1, <5>1,
               GstResponsiveUnappliedRunNodeIsEnabled
          <5> QED BY <1>1, <5>2, Isa
               DEF TimeoutFixedClockPacketConcreteActionPending,
                   HistoricalDiscoveryPacketConcreteAction,
                   HistoricalDiscoveryPacketConcreteActionKindCarrier,
                   Recipient
        <4> QED BY <4>1, <4>2
      <3>2. CASE NodeHasApplication(Recipient)
        <4>1. Recipient \in AsyncCurrentResponsiveVoters
          BY <2>2, <2>3, <3>2, Isa
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncHistoricalRecoveryTypeInvariant
        <4>2. CASE IngressDepth(Recipient) = 0
          <5>1. OldestDueSourcePacket(Recipient, Source)
                   \in OverdueResponsivePackets
            BY <2>1, <2>2, <2>3, Isa
               DEF OverdueResponsivePackets,
                   AsyncPacketOwnsClockDeadline,
                   DueSourcePackets, OldestDueSourcePacket
          <5>2. \E actionSource \in AsyncIngressSources:
                   ENABLED
                     (PostGstAdmitHiddenPacket(
                        Recipient, actionSource)
                        \/ PostGstAdmitHistoricalRecoveryPacket(
                             Recipient, actionSource))
            BY <1>1, <2>3, <2>6, <3>2, <4>1, <4>2, <5>1,
               EmptyAppliedOverduePacketExposesAdmission
          <5> QED BY <1>1, <5>2, ExpandENABLED, Isa
               DEF TimeoutFixedClockPacketConcreteActionPending,
                   HistoricalDiscoveryPacketConcreteAction,
                   HistoricalDiscoveryPacketConcreteActionKindCarrier,
                   Recipient, Source
        <4>3. CASE IngressDepth(Recipient) > 0
          <5>1. CASE HistoricalDrainableIngressIndices(Recipient) # {}
            <6>1. ENABLED PostGstRunHistoricalServer(Recipient)
              BY <1>1, <2>1, <3>2, <4>1, <5>1,
                 AppliedResponsiveHistoricalServerEnabledAfterGst
            <6> QED BY <1>1, <6>1, Isa
                 DEF TimeoutFixedClockPacketConcreteActionPending,
                     HistoricalDiscoveryPacketConcreteAction,
                     HistoricalDiscoveryPacketConcreteActionKindCarrier,
                     Recipient
          <5>2. CASE HistoricalDrainableIngressIndices(Recipient) = {}
            <6>1. AsyncIoQueueDepth(Recipient) > 0
              BY <2>2, <2>3, <4>3, <5>2,
                 NonemptyUndrainableHistoricalIngressHasIoWork
            <6>2. Recipient \in AsyncArchiveIoServiceNodes
              BY <2>1, <3>2, <4>1, Isa
                 DEF AsyncArchiveIoServiceNodes,
                     AsyncResponsiveAppliedArchiveServers,
                     AsyncResponsiveOnlineArchiveServers,
                     AsyncResponsiveArchiveServers
            <6>3. ENABLED PostGstServiceIoWorker(Recipient)
              BY <2>1, <6>1, <6>2,
                 AsyncStrongTypeProjectsAsyncType,
                 QueuedIoEnablesPostGstService
            <6> QED BY <1>1, <6>3, Isa
                 DEF TimeoutFixedClockPacketConcreteActionPending,
                     HistoricalDiscoveryPacketConcreteAction,
                     HistoricalDiscoveryPacketConcreteActionKindCarrier,
                     Recipient
          <5> QED BY <5>1, <5>2
        <4> QED BY <2>2, <4>2, <4>3, SMT
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM TimeoutFixedClockPacketConcreteActionStepIsGoalOrFrame ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known:
      \A budget \in Nat:
        \A actionKind
             \in HistoricalDiscoveryPacketConcreteActionKindCarrier:
          \A actionSource \in AsyncIngressSources:
            /\ TimeoutFixedClockPacketConcreteActionPending(
                 source, sourceView, clockValue, deadlineValue,
                 sourceRank, packet, known, budget,
                 actionKind, actionSource)
            /\ [AsyncNext]_AsyncAllVars
            => \/ TimeoutFixedClockLifecycleGoal(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank, packet, known, budget)'
               \/ TimeoutFixedClockPacketConcreteActionPending(
                    source, sourceView, clockValue, deadlineValue,
                    sourceRank, packet, known, budget,
                    actionKind, actionSource)'
BY HistoricalDiscoveryFixedClockIngressStrictlyDescends,
   HistoricalDiscoverySelectedNonOverdueShadowStrictlyDescends,
   HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryLowerCandidateInsertionReselectsLower,
   HistoricalDiscoveryLowerServeInsertionReselectsLower,
   HistoricalDiscoveryCandidateExitClassifiesOccurrenceDebt,
   HistoricalDiscoveryServeExitEitherLowersOrReplenishes,
   HistoricalDiscoveryCandidateDepartureRetainsLifecycleCoverage,
   HistoricalDiscoveryServeDepartureInstallsDurableCoverage,
   HistoricalDiscoveryServicedCandidateIdentityBlocksReentry,
   HistoricalDiscoveryRetiredServeIdentityBlocksReentry,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   AsyncBracketNextPreservesStrongTypeInvariant,
   IsaT(4800)
   DEF TimeoutFixedClockPacketConcreteActionPending,
       TimeoutFixedClockLifecycleGoal,
       TimeoutFixedClockLifecycleDiscovery,
       TimeoutFixedClockLifecycleEpisodeAtBudget,
       TimeoutFixedClockStrictRankGoal,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockProducerPrefix,
       TimeoutFixedClockDependencyProducerPrefix,
       TimeoutFixedPacketLiveOwners,
       TimeoutFixedPacketCoveredOwners,
       HistoricalDiscoveryPacketConcreteAction,
       HistoricalDiscoveryPacketConcreteActionKindCarrier,
       HistoricalDiscoveryPacketDependencyRank,
       HistoricalDiscoveryPacketDependencyOrdering,
       HistoricalDiscoveryConcreteFixedClockRank,
       HistoricalDiscoveryConcreteBlockerStage,
       HistoricalDiscoveryConcreteDependencyRank,
       AsyncTargetNeutralLifecycleDiscoveredOwnerSet,
       AsyncAllVars

THEOREM TimeoutFixedClockPacketConcreteActionOccurrenceReachesGoal ==
  \A source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known:
      \A budget \in Nat:
        \A actionKind
             \in HistoricalDiscoveryPacketConcreteActionKindCarrier:
          \A actionSource \in AsyncIngressSources:
            /\ TimeoutFixedClockPacketConcreteActionPending(
                 source, sourceView, clockValue, deadlineValue,
                 sourceRank, packet, known, budget,
                 actionKind, actionSource)
            /\ <<HistoricalDiscoveryPacketConcreteAction(
                    packet, actionKind, actionSource)>>_AsyncAllVars
            => TimeoutFixedClockLifecycleGoal(
                 source, sourceView, clockValue, deadlineValue,
                 sourceRank, packet, known, budget)'
BY HistoricalDiscoveryFixedClockIngressStrictlyDescends,
   HistoricalDiscoverySelectedNonOverdueShadowStrictlyDescends,
   HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryLowerCandidateInsertionReselectsLower,
   HistoricalDiscoveryLowerServeInsertionReselectsLower,
   HistoricalDiscoveryCandidateExitClassifiesOccurrenceDebt,
   HistoricalDiscoveryServeExitEitherLowersOrReplenishes,
   HistoricalDiscoveryCandidateDepartureRetainsLifecycleCoverage,
   HistoricalDiscoveryServeDepartureInstallsDurableCoverage,
   HistoricalDiscoveryServicedCandidateIdentityBlocksReentry,
   HistoricalDiscoveryRetiredServeIdentityBlocksReentry,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   IsaT(4800)
   DEF TimeoutFixedClockPacketConcreteActionPending,
       TimeoutFixedClockLifecycleGoal,
       TimeoutFixedClockLifecycleDiscovery,
       TimeoutFixedClockLifecycleEpisodeAtBudget,
       TimeoutFixedClockStrictRankGoal,
       TimeoutFixedClockBlockedAtRank,
       TimeoutFixedClockProducerPrefix,
       TimeoutFixedClockDependencyProducerPrefix,
       TimeoutFixedPacketLiveOwners,
       TimeoutFixedPacketCoveredOwners,
       HistoricalDiscoveryPacketConcreteAction,
       HistoricalDiscoveryPacketConcreteActionKindCarrier,
       HistoricalDiscoveryPacketDependencyRank,
       HistoricalDiscoveryPacketDependencyOrdering,
       HistoricalDiscoveryConcreteFixedClockRank,
       HistoricalDiscoveryConcreteBlockerStage,
       HistoricalDiscoveryConcreteDependencyRank,
       AsyncTargetNeutralLifecycleDiscoveredOwnerSet,
       AsyncAllVars

THEOREM AsyncSpecProvidesTimeoutFixedClockPacketConcreteActionFairness ==
  \A initialContext,
     source \in AsyncCurrentResponsiveVoters,
     sourceView \in Views,
     clockValue, deadlineValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known:
      \A budget \in Nat:
        \A actionKind
             \in HistoricalDiscoveryPacketConcreteActionKindCarrier:
          \A actionSource \in AsyncIngressSources:
            /\ AsyncSpecAt(initialContext)
            /\ TimeoutFixedClockPacketConcreteActionPending(
                 source, sourceView, clockValue, deadlineValue,
                 sourceRank, packet, known, budget,
                 actionKind, actionSource)
            => WF_AsyncAllVars(
                 HistoricalDiscoveryPacketConcreteAction(
                   packet, actionKind, actionSource))
BY AsyncSpecAlwaysUsesFixedResponsiveVoters, Isa, PTL
   DEF TimeoutFixedClockPacketConcreteActionPending,
       TimeoutFixedClockLifecycleEpisodeAtBudget,
       TimeoutFixedClockPending,
       HistoricalDiscoveryPacketConcreteAction,
       HistoricalDiscoveryPacketConcreteActionKindCarrier,
       HistoricalRecoveryTarget,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncSpecAt, AsyncFairnessAt

TimeoutFixedClockPacketConcreteActionServiceProperty(specification) ==
  specification
    => \A source \in AsyncCurrentResponsiveVoters,
          sourceView \in Views,
          clockValue, deadlineValue \in Nat,
          sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             \A actionKind
                  \in HistoricalDiscoveryPacketConcreteActionKindCarrier:
               \A actionSource \in AsyncIngressSources:
                 TimeoutFixedClockPacketConcreteActionPending(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, packet, known, budget,
                   actionKind, actionSource)
                   ~> TimeoutFixedClockLifecycleGoal(
                        source, sourceView, clockValue, deadlineValue,
                        sourceRank, packet, known, budget)

THEOREM AsyncSpecProvidesTimeoutFixedClockPacketConcreteActionService ==
  \A initialContext:
    TimeoutFixedClockPacketConcreteActionServiceProperty(
      AsyncSpecAt(initialContext))
BY TimeoutFixedClockPacketConcreteActionStepIsGoalOrFrame,
   TimeoutFixedClockPacketConcreteActionOccurrenceReachesGoal,
   AsyncSpecProvidesTimeoutFixedClockPacketConcreteActionFairness,
   PTL
   DEF TimeoutFixedClockPacketConcreteActionServiceProperty,
       TimeoutFixedClockPacketConcreteActionPending,
       TimeoutFixedClockLifecycleGoal

THEOREM AsyncSpecProvidesTimeoutFixedClockPacketDependencyKernel ==
  \A initialContext:
    TimeoutFixedClockPacketDependencyKernelProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext, AsyncSpecAt(initialContext)
         PROVE \A source \in AsyncCurrentResponsiveVoters,
                  sourceView \in Views,
                  clockValue, deadlineValue \in Nat,
                  sourceRank
                    \in HistoricalDiscoveryFixedClockBlockerCarrier:
                 \A packet, known:
                   \A budget \in Nat:
                     (/\ TimeoutFixedClockLifecycleEpisodeAtBudget(
                           source, sourceView,
                           clockValue, deadlineValue,
                           sourceRank, packet, known, budget)
                      /\ TimeoutFixedPacketLiveOwners(packet) = {})
                       ~> TimeoutFixedClockLifecycleGoal(
                            source, sourceView,
                            clockValue, deadlineValue,
                            sourceRank, packet, known, budget)
    <2>0. /\ initialContext \in ContextRecords
           /\ []AsyncFrozenContextAt(initialContext)
           /\ []PostGstReplayQuarantineExcluded
           /\ []AsyncCandidateProducerContinuationExternalCoverageInvariant
           /\ []AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysKeepsFrozenContext,
         AsyncSpecAlwaysExcludesPostGstReplayQuarantine,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
         AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity, PTL
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
             Safety, TypeInvariant, AsyncFrozenContextAt
    <2>1. ASSUME NEW source \in AsyncCurrentResponsiveVoters,
                  NEW sourceView \in Views,
                  NEW clockValue, NEW deadlineValue \in Nat,
                  NEW sourceRank
                    \in HistoricalDiscoveryFixedClockBlockerCarrier,
                  NEW packet, NEW known, NEW budget \in Nat
           PROVE (/\ TimeoutFixedClockLifecycleEpisodeAtBudget(
                         source, sourceView, clockValue, deadlineValue,
                         sourceRank, packet, known, budget)
                    /\ TimeoutFixedPacketLiveOwners(packet) = {})
                     ~> TimeoutFixedClockLifecycleGoal(
                          source, sourceView, clockValue, deadlineValue,
                          sourceRank, packet, known, budget)
      <3>1. []((/\ TimeoutFixedClockLifecycleEpisodeAtBudget(
                        source, sourceView, clockValue, deadlineValue,
                        sourceRank, packet, known, budget)
                   /\ TimeoutFixedPacketLiveOwners(packet) = {})
                  => \E actionKind
                         \in HistoricalDiscoveryPacketConcreteActionKindCarrier:
                       \E actionSource \in AsyncIngressSources:
                         TimeoutFixedClockPacketConcreteActionPending(
                           source, sourceView,
                           clockValue, deadlineValue,
                           sourceRank, packet, known, budget,
                           actionKind, actionSource))
        BY <2>0,
           TimeoutFixedClockPacketTailHasFrozenConcreteAction, PTL
      <3>2. \A actionKind
                   \in HistoricalDiscoveryPacketConcreteActionKindCarrier:
               \A actionSource \in AsyncIngressSources:
                 TimeoutFixedClockPacketConcreteActionPending(
                   source, sourceView, clockValue, deadlineValue,
                   sourceRank, packet, known, budget,
                   actionKind, actionSource)
                   ~> TimeoutFixedClockLifecycleGoal(
                        source, sourceView, clockValue, deadlineValue,
                        sourceRank, packet, known, budget)
        BY <1>1,
           AsyncSpecProvidesTimeoutFixedClockPacketConcreteActionService
           DEF TimeoutFixedClockPacketConcreteActionServiceProperty
      <3> QED BY <3>1, <3>2, PTL
    <2> QED BY <2>1
  <1> QED BY <1>1
       DEF TimeoutFixedClockPacketDependencyKernelProperty

THEOREM AsyncSpecProvidesTimeoutFixedClockLifecyclePhysicalKernels ==
  \A initialContext:
    TimeoutFixedClockLifecyclePhysicalKernelProperties(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesTimeoutFixedClockPacketDependencyKernel,
   AsyncSpecProvidesTimeoutFixedClockDerivedCandidateServeKernels
   DEF TimeoutFixedClockLifecyclePhysicalKernelProperties,
       TimeoutFixedClockDerivedCandidateServeKernelProperties

THEOREM AsyncLiveProvidesTimeoutFixedClockLifecyclePhysicalKernels ==
  \A initialContext:
    TimeoutFixedClockLifecyclePhysicalKernelProperties(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesTimeoutFixedClockLifecyclePhysicalKernels
   DEF AsyncLiveSpecAt

(***************************************************************************
Exact delivery-candidate service.

These three leaves start after packet/ingress service has already installed
the immutable delivery candidate.  They therefore use only ordinary
protected-candidate starvation plus the action-local reducer theorems above.
Candidate retirement is not itself success: the preservation lemmas map
every stale/ignored/serviced departure to the exact semantic endpoint for the
same vote, TC, or Commit QC.
***************************************************************************)

THEOREM TimeoutVoteDeliveryCandidatePersistsOrReachesExactOutcome ==
  \A vote \in TimeoutVoteRecordSet,
     recipient \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ TimeoutVoteReducerCandidateOwner(vote, recipient)
    /\ [AsyncNext]_AsyncAllVars
    => \/ TimeoutDeliveryOutcome(vote, recipient)'
       \/ TimeoutVoteReducerCandidateOwner(vote, recipient)'
BY ExecuteExactCurrentViewTimeoutDeliveryRecordsExactReceipt,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncCandidateSameGenerationSuccessfulServiceIdentityPersistsUntilStrictExit,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   GstAsyncStepIsMonotone, IsaT(2400)
   DEF TimeoutVoteReducerCandidateOwner,
       TimeoutVoteDeliveryKernelSource,
       TimeoutDeliveryOutcome, TimeoutReceipt,
       TimeoutSourceDominated, TimeoutViewGoal,
       DecisionPropagationFrontier,
       ExactTimeoutVoteDeliveryCommand,
       ExactDeliveryCandidateOwns,
       DeliveryCandidate, DeliveryClass, DeliveryKind,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateScheduled, CandidateScheduledAfter,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       AsyncCandidateMonotoneSemanticCoverageAfterIn,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateTerminalTombstoned,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, ExecuteCommand, ExecuteCoreDelivery,
       AsyncAllVars

THEOREM TimeoutTcDeliveryCandidatePersistsOrReachesExactOwner ==
  \A source, target, tc, minimumView:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ TimeoutTcReducerCandidateOwner(
         source, target, tc, minimumView)
    /\ [AsyncNext]_AsyncAllVars
    => \/ TimeoutTcDeliveryTerminalOwner(
            target, tc, minimumView)'
       \/ TimeoutTcReducerCandidateOwner(
            source, target, tc, minimumView)'
BY ExecuteExactTimeoutCertificateDeliveryCreatesInstallOrGoal,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncCandidateSameGenerationSuccessfulServiceIdentityPersistsUntilStrictExit,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   GstAsyncStepIsMonotone, IsaT(2400)
   DEF TimeoutTcReducerCandidateOwner,
       TimeoutTcDeliveryTerminalOwner,
       TimeoutTcReceivedReducerOwner,
       TimeoutTcInstallWalOwner,
       TimeoutTcKernelSource,
       TimeoutDirectGoal,
       ExactTimeoutCertificateDeliveryCommand,
       ExactDeliveryCandidateOwns,
       DeliveryCandidate, DeliveryClass, DeliveryKind,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateScheduled, CandidateScheduledAfter,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       AsyncCandidateMonotoneSemanticCoverageAfterIn,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateTerminalTombstoned,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, ExecuteCommand, ExecuteCoreDelivery,
       AsyncAllVars

THEOREM TimeoutDecisionDeliveryCandidatePersistsOrReachesExactOwner ==
  \A source, target, qc:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ TimeoutDecisionReducerCandidateOwner(source, target, qc)
    /\ [AsyncNext]_AsyncAllVars
    => \/ TimeoutDecisionDeliveryTerminalOwner(target, qc)'
       \/ TimeoutDecisionReducerCandidateOwner(source, target, qc)'
BY ExecuteExactCommitCertificateDeliveryRecordsExactReceipt,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncCandidateSameGenerationSuccessfulServiceIdentityPersistsUntilStrictExit,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   GstAsyncStepIsMonotone, IsaT(2400)
   DEF TimeoutDecisionReducerCandidateOwner,
       TimeoutDecisionDeliveryTerminalOwner,
       TimeoutDecisionReceivedReducerOwner,
       TimeoutExactDecisionWalOwner,
       TimeoutDecisionKernelSource,
       ExactCommitCertificateDeliveryCommand,
       ExactDeliveryCandidateOwns,
       DeliveryCandidate, DeliveryClass, DeliveryKind,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateScheduled, CandidateScheduledAfter,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       AsyncCandidateMonotoneSemanticCoverageAfterIn,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateTerminalTombstoned,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, ExecuteCommand, ExecuteCoreDelivery,
       AsyncAllVars

TimeoutExactDeliveryCandidateKernelProperties(specification) ==
  /\ TimeoutVoteReducerCandidateKernelProperty(specification)
  /\ TimeoutTcDeliveryCandidateKernelProperty(specification)
  /\ TimeoutDecisionDeliveryCandidateKernelProperty(specification)

THEOREM AsyncSpecProvidesTimeoutExactDeliveryCandidateKernels ==
  \A initialContext:
    TimeoutExactDeliveryCandidateKernelProperties(
      AsyncSpecAt(initialContext))
BY StarvationFreedomObligation,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   ExactDecisionAsyncSpecAlwaysCandidateTombstones,
   TimeoutVoteDeliveryCandidatePersistsOrReachesExactOutcome,
   TimeoutTcDeliveryCandidatePersistsOrReachesExactOwner,
   TimeoutDecisionDeliveryCandidatePersistsOrReachesExactOwner,
   PTL, IsaT(1800)
   DEF TimeoutExactDeliveryCandidateKernelProperties,
       TimeoutVoteReducerCandidateKernelProperty,
       TimeoutTcDeliveryCandidateKernelProperty,
       TimeoutDecisionDeliveryCandidateKernelProperty,
       TimeoutVoteReducerCandidateOwner,
       TimeoutTcReducerCandidateOwner,
       TimeoutDecisionReducerCandidateOwner,
       TimeoutVoteDeliveryKernelSource,
       TimeoutTcKernelSource, TimeoutDecisionKernelSource,
       ExactDeliveryCandidateOwns,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       DeliveryCandidate, DeliveryClass,
       StarvationFreedomProperty

THEOREM AsyncLiveProvidesTimeoutExactDeliveryCandidateKernels ==
  \A initialContext:
    TimeoutExactDeliveryCandidateKernelProperties(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesTimeoutExactDeliveryCandidateKernels
   DEF AsyncLiveSpecAt

(***************************************************************************
Context-stable imported certificate reducer tails.

A receipt or WAL entry is not itself a fair scheduler owner.  The invariant
below therefore projects every exact imported TC/CommitQC receipt and every
exact pending WAL to its causal Begin/Persist candidate, unless the requested
view or Decision has already been reached.  The candidate keeps the original
authenticated evidence and causal origin.  `CandidateConsumerCurrent` now
ignores only local view/generation movement for these imported tails; height,
context, certificate lineage, and Decision/Apply authority remain exact.
***************************************************************************)

TimeoutTcImportedBeginCandidateOwner(target, tc, minimumView) ==
  /\ target \in AsyncCurrentResponsiveVoters
  /\ TimeoutCertificateSemanticIdentity(tc, minimumView)
  /\ \E candidate \in AsyncCandidateSet:
       /\ candidate.kind = "BeginInstallTC"
       /\ candidate.node = target
       /\ candidate.view = tc.view
       /\ InstallTcEvidenceMatches(candidate, tc)
       /\ ImportedTimeoutCertificateTail(candidate)
       /\ CandidateConsumerCurrent(candidate)
       /\ ResponsiveProtectedCandidateOwned(candidate)

TimeoutTcExactPersistCandidateOwner(target, tc, minimumView) ==
  /\ target \in AsyncCurrentResponsiveVoters
  /\ TimeoutCertificateSemanticIdentity(tc, minimumView)
  /\ \E candidate \in AsyncCandidateSet:
       /\ candidate.kind = "PersistInstallTC"
       /\ candidate.node = target
       /\ candidate.view = tc.view
       /\ InstallTcEvidenceMatches(candidate, tc)
       /\ CandidateConsumerCurrent(candidate)
       /\ ResponsiveProtectedCandidateOwned(candidate)

TimeoutDecisionImportedBeginCandidateOwner(target, qc) ==
  /\ target \in AsyncCurrentResponsiveVoters
  /\ qc \in QcRecordSet
  /\ qc.context = context
  /\ qc.height = height
  /\ qc.phase = "Commit"
  /\ \E candidate \in AsyncCandidateSet:
       /\ candidate.kind = "BeginDecision"
       /\ candidate.node = target
       /\ AsyncCommitImportCandidateLineage(candidate, qc)
       /\ CandidateConsumerCurrent(candidate)
       /\ ResponsiveProtectedCandidateOwned(candidate)

TimeoutDecisionImportedPersistCandidateOwner(target, qc) ==
  /\ target \in AsyncCurrentResponsiveVoters
  /\ qc \in QcRecordSet
  /\ qc.context = context
  /\ qc.height = height
  /\ qc.phase = "Commit"
  /\ \E candidate \in AsyncCandidateSet:
       /\ candidate.kind = "PersistDecision"
       /\ candidate.node = target
       /\ AsyncCommitImportCandidateLineage(candidate, qc)
       /\ CandidateConsumerCurrent(candidate)
       /\ ResponsiveProtectedCandidateOwned(candidate)

TimeoutImportedCertificateReducerTailInvariant ==
  /\ \A target, tc, minimumView:
       TimeoutTcReceivedReducerOwner(target, tc, minimumView)
         => \/ TimeoutDirectGoal(target, minimumView)
            \/ TimeoutTcImportedBeginCandidateOwner(
                 target, tc, minimumView)
            \/ TimeoutTcInstallWalOwner(target, tc, minimumView)
  /\ \A target, tc, minimumView:
       TimeoutTcInstallWalOwner(target, tc, minimumView)
         => \/ TimeoutDirectGoal(target, minimumView)
            \/ TimeoutTcExactPersistCandidateOwner(
                 target, tc, minimumView)
  /\ \A target, qc:
       TimeoutDecisionReceivedReducerOwner(target, qc)
         => \/ NodeHasDecision(target)
            \/ TimeoutDecisionImportedBeginCandidateOwner(target, qc)
            \/ TimeoutExactDecisionWalOwner(target, qc)
  /\ \A target, qc:
       TimeoutExactDecisionWalOwner(target, qc)
         => \/ NodeHasDecision(target)
            \/ TimeoutDecisionImportedPersistCandidateOwner(target, qc)

THEOREM AsyncInitEstablishesTimeoutImportedCertificateReducerTail ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => TimeoutImportedCertificateReducerTailInvariant
BY IsaT(300)
   DEF TimeoutImportedCertificateReducerTailInvariant,
       TimeoutTcReceivedReducerOwner,
       TimeoutTcInstallWalOwner,
       TimeoutDecisionReceivedReducerOwner,
       TimeoutExactDecisionWalOwner,
       AsyncInitAt, AsyncBaseInitAt, InitAt

THEOREM AsyncBracketPreservesTimeoutImportedCertificateReducerTail ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceLifecycleInvariant
  /\ TimeoutImportedCertificateReducerTailInvariant
  /\ [AsyncNext]_AsyncAllVars
  => TimeoutImportedCertificateReducerTailInvariant'
BY DirectCommitQcCandidateHasExactImportLineage,
   CommitCertificateResponseCandidateHasExactImportLineage,
   CommitImportCausalSuccessorRetainsExactLineage,
   ImportedCertificateTailCannotRetireOnLocalIncarnationChange,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateIoCompletionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   AsyncCandidateBusyDeferralTransfersSameOwner,
   AsyncCandidateDeferredHandoffRetainsSameOwner,
   IsaT(4200)
   DEF TimeoutImportedCertificateReducerTailInvariant,
       TimeoutTcImportedBeginCandidateOwner,
       TimeoutTcExactPersistCandidateOwner,
       TimeoutDecisionImportedBeginCandidateOwner,
       TimeoutDecisionImportedPersistCandidateOwner,
       TimeoutTcReceivedReducerOwner,
       TimeoutTcInstallWalOwner,
       TimeoutDecisionReceivedReducerOwner,
       TimeoutExactDecisionWalOwner,
       TimeoutDirectGoal, TimeoutViewGoal,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateConsumerCurrent, CandidateScheduled,
       CandidateScheduledAfter,
       AsyncCommitImportCandidateLineage,
       AsyncCommitImportDirectEvidence,
       AsyncCommitImportResponseEvidence,
       ImportedTimeoutCertificateTail,
       InstallTcEvidenceMatches,
       CommandSuccessors, CausalCandidate,
       CausalCandidateWithEvidence,
       AppendCausalSuccessors, FreshCommandSuccessors,
       EnqueueCandidate,
       ExecuteCommand, ExecuteRegularCommand,
       ExecuteCoreDelivery, ExecutePersistInstall,
       ExecutePersistDecision,
       DeliverTC, DeliverQC, BeginInstallTC, PersistInstallTC,
       BeginDecision, PersistDecision,
       AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       AsyncCandidateMonotoneSemanticCoverageAfterIn,
       AsyncCandidateReducerStageCoveredAfterIn,
       AsyncCandidateDecisionStageCoveredAfter,
       AsyncCandidateInstallTcStageCoveredAfter,
       AsyncCandidateConsumerEpisodeObsoleteAfter,
       AsyncCandidateTerminalTombstoned,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep,
       IngressDrainStep, SerializedRuntimeStep,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       AsyncNetworkStep, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart, AsyncAllVars

THEOREM AsyncSpecAlwaysTimeoutImportedCertificateReducerTail ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []TimeoutImportedCertificateReducerTailInvariant
PROOF
  <1>1. AsyncInitAt(initialContext)
           => TimeoutImportedCertificateReducerTailInvariant
    BY AsyncInitEstablishesTimeoutImportedCertificateReducerTail
  <1>2. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
    BY AsyncSpecAlwaysStrongTypeInvariant
  <1>3. AsyncSpecAt(initialContext) => []AsyncProgressOwnershipInvariant
    BY AsyncSpecAlwaysProgressOwnershipInvariant
  <1>4. AsyncSpecAt(initialContext)
           => []AsyncCandidateServiceLifecycleInvariant
    BY ExactDecisionAsyncSpecAlwaysCandidateTombstones
  <1>5. /\ AsyncStrongTypeInvariant
         /\ AsyncProgressOwnershipInvariant
         /\ AsyncCandidateServiceLifecycleInvariant
         /\ TimeoutImportedCertificateReducerTailInvariant
         /\ [AsyncNext]_AsyncAllVars
         => TimeoutImportedCertificateReducerTailInvariant'
    BY AsyncBracketPreservesTimeoutImportedCertificateReducerTail
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, PTL
       DEF AsyncSpecAt

TimeoutImportedCertificateCandidateTailKernelProperties(specification) ==
  /\ (specification
        => \A target, tc, minimumView:
             TimeoutTcImportedBeginCandidateOwner(
               target, tc, minimumView)
               ~> (TimeoutDirectGoal(target, minimumView)
                    \/ TimeoutTcExactPersistCandidateOwner(
                         target, tc, minimumView)))
  /\ (specification
        => \A target, tc, minimumView:
             TimeoutTcExactPersistCandidateOwner(
               target, tc, minimumView)
               ~> TimeoutDirectGoal(target, minimumView))
  /\ (specification
        => \A target, qc:
             TimeoutDecisionImportedBeginCandidateOwner(target, qc)
               ~> (NodeHasDecision(target)
                    \/ TimeoutDecisionImportedPersistCandidateOwner(
                         target, qc)))
  /\ (specification
        => \A target, qc:
             TimeoutDecisionImportedPersistCandidateOwner(target, qc)
               ~> NodeHasDecision(target))

THEOREM AsyncSpecProvidesTimeoutImportedCertificateCandidateTailKernels ==
  \A initialContext:
    TimeoutImportedCertificateCandidateTailKernelProperties(
      AsyncSpecAt(initialContext))
BY StarvationFreedomObligation,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   ExactDecisionAsyncSpecAlwaysCandidateTombstones,
   ExecuteExactBeginInstallCreatesExactWalOwner,
   ExecuteExactPersistInstallReachesMinimumView,
   ExecuteTargetBeginDecisionCreatesWalOwner,
   ExecuteTargetPersistDecisionReachesDecision,
   ImportedCertificateTailCannotRetireOnLocalIncarnationChange,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateIoCompletionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   AsyncCandidateBusyDeferralTransfersSameOwner,
   AsyncCandidateDeferredHandoffRetainsSameOwner,
   PTL, IsaT(4200)
   DEF TimeoutImportedCertificateCandidateTailKernelProperties,
       TimeoutTcImportedBeginCandidateOwner,
       TimeoutTcExactPersistCandidateOwner,
       TimeoutDecisionImportedBeginCandidateOwner,
       TimeoutDecisionImportedPersistCandidateOwner,
       TimeoutDirectGoal, TimeoutViewGoal,
       ExactBeginInstallTcCommand,
       ExactPersistInstallTcCommand,
       TargetBeginDecisionCommand,
       TargetPersistDecisionCommand,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateConsumerCurrent, CandidateScheduled,
       CandidateScheduledAfter,
       AsyncCommitImportCandidateLineage,
       ImportedTimeoutCertificateTail,
       InstallTcEvidenceMatches,
       CommandSuccessors, CausalCandidate,
       CausalCandidateWithEvidence,
       AppendCausalSuccessors, FreshCommandSuccessors,
       EnqueueCandidate,
       CommandDispatchable, ExecuteCommand,
       ExecuteRegularCommand, ExecutePersistInstall,
       ExecutePersistDecision,
       BeginInstallTC, PersistInstallTC,
       BeginDecision, PersistDecision,
       AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       AsyncCandidateMonotoneSemanticCoverageAfterIn,
       AsyncCandidateReducerStageCoveredAfterIn,
       AsyncCandidateDecisionStageCoveredAfter,
       AsyncCandidateInstallTcStageCoveredAfter,
       AsyncCandidateConsumerEpisodeObsoleteAfter,
       AsyncCandidateTerminalTombstoned,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep,
       IngressDrainStep, SerializedRuntimeStep,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       AsyncNetworkStep, AsyncFaultStep, AsyncAllVars,
       StarvationFreedomProperty

TimeoutImportedCertificateReducerWalKernelProperties(specification) ==
  /\ TimeoutTcReceivedReducerKernelProperty(specification)
  /\ TimeoutTcInstallWalKernelProperty(specification)
  /\ TimeoutDecisionReceivedReducerKernelProperty(specification)
  /\ TimeoutDecisionWalKernelProperty(specification)

THEOREM AsyncSpecProvidesTimeoutImportedCertificateReducerWalKernels ==
  \A initialContext:
    TimeoutImportedCertificateReducerWalKernelProperties(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysTimeoutImportedCertificateReducerTail,
   AsyncSpecProvidesTimeoutImportedCertificateCandidateTailKernels,
   PTL, IsaT(1200)
   DEF TimeoutImportedCertificateReducerWalKernelProperties,
       TimeoutImportedCertificateReducerTailInvariant,
       TimeoutImportedCertificateCandidateTailKernelProperties,
       TimeoutTcReceivedReducerKernelProperty,
       TimeoutTcInstallWalKernelProperty,
       TimeoutDecisionReceivedReducerKernelProperty,
       TimeoutDecisionWalKernelProperty

THEOREM AsyncLiveProvidesTimeoutImportedCertificateReducerWalKernels ==
  \A initialContext:
    TimeoutImportedCertificateReducerWalKernelProperties(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesTimeoutImportedCertificateReducerWalKernels
   DEF AsyncLiveSpecAt

(***************************************************************************
Exact retained-control transport closure.

The three control families below share the physical scheduler, but not their
semantic terminal.  `TimeoutPhysicalControlItem` freezes the complete wire
item.  The TC arm deliberately uses the certificate's own view; projection
to a lower requested view is proved only after the exact item reaches its
terminal.  Thus replacement by a different certificate or delivery to a
different target cannot discharge this corridor.

The stage number records only boundary handoff.  It is not used as a claim
that waiting within retained control, packet, or ingress is progress.  Those
non-descent episodes use the existing concrete fixed-clock packet rank, the
exact ingress rank, and the frozen Candidate/Serve ordinal budget.  The
snapshot's logical scheduler cut and physical admission cut prevent later
causal, Control, Completion, priority, retry, or Serve work from acquiring a
predecessor position ahead of the admitted exact item.
***************************************************************************)

TimeoutPhysicalControlItem(item) ==
  /\ item \in AsyncNetworkItems
  /\ \/ \E vote \in TimeoutVoteRecordSet,
             recipient \in AsyncCurrentResponsiveVoters:
          /\ TimeoutVoteDeliveryKernelSource(vote, recipient)
          /\ item = TimeoutVoteItem(vote, recipient)
     \/ \E source, target, tc:
          /\ TimeoutTcKernelSource(source, target, tc, tc.view)
          /\ item = TimeoutCertificateItem(source, target, tc)
     \/ \E source, target, qc:
          /\ TimeoutDecisionKernelSource(source, target, qc)
          /\ item = CommitCertificateItem(source, target, qc)

TimeoutPhysicalControlTerminal(item) ==
  \/ \E vote \in TimeoutVoteRecordSet,
         recipient \in AsyncCurrentResponsiveVoters:
       /\ item = TimeoutVoteItem(vote, recipient)
       /\ TimeoutDeliveryOutcome(vote, recipient)
  \/ \E source, target, tc:
       /\ item = TimeoutCertificateItem(source, target, tc)
       /\ TimeoutTcKernelSource(source, target, tc, tc.view)
       /\ TimeoutTcDeliveryTerminalOwner(target, tc, tc.view)
  \/ \E source, target, qc:
       /\ item = CommitCertificateItem(source, target, qc)
       /\ TimeoutDecisionKernelSource(source, target, qc)
       /\ TimeoutDecisionDeliveryTerminalOwner(target, qc)

TimeoutPhysicalControlRetainedOwner(item) ==
  /\ TimeoutPhysicalControlItem(item)
  /\ item \in asyncRetainedControl

TimeoutPhysicalControlPacketOwner(item) ==
  /\ TimeoutPhysicalControlItem(item)
  /\ ExactPacketOwns(item)

TimeoutPhysicalControlIngressOwner(item) ==
  /\ TimeoutPhysicalControlItem(item)
  /\ ExactIngressOwns(item)

TimeoutPhysicalControlCandidateOwner(item) ==
  /\ TimeoutPhysicalControlItem(item)
  /\ ExactDeliveryCandidateOwns(item)

TimeoutPhysicalControlGoal(item) ==
  \/ TimeoutPhysicalControlTerminal(item)
  \/ TimeoutPhysicalControlCandidateOwner(item)

TimeoutPhysicalControlExactPackets(item) ==
  {packet \in asyncTransport: packet.item = item}

TimeoutPhysicalControlSelectedPacket(item) ==
  CHOOSE packet \in TimeoutPhysicalControlExactPackets(item):
    \A other \in TimeoutPhysicalControlExactPackets(item):
      packet.sentAt <= other.sentAt

TimeoutPhysicalControlPacketDependencyRank(item, snapshot) ==
  ExactDecisionTargetNeutralPacketDependencyRankForSnapshot(
    snapshot, TimeoutPhysicalControlSelectedPacket(item))

TimeoutPhysicalControlIngressDependencyRank(item) ==
  ExactDecisionRequestIngressRank(item.envelope.recipient, item)

TimeoutPhysicalControlLifecycleStageRank(item) ==
  IF TimeoutPhysicalControlGoal(item)
  THEN 0
  ELSE IF TimeoutPhysicalControlIngressOwner(item)
       THEN 1
       ELSE IF TimeoutPhysicalControlPacketOwner(item)
            THEN 2
            ELSE 3

TimeoutPhysicalControlLifecycleStageCarrier == 0..3

TimeoutPhysicalControlLifecycleStageOrdering ==
  OpToRel(<, TimeoutPhysicalControlLifecycleStageCarrier)

TimeoutPhysicalControlFrozenSnapshot(clockValue) ==
  ExactDecisionTargetNeutralFixedClockSnapshot(clockValue)

TimeoutPhysicalControlPacketSnapshotAtCut(
    item, snapshot, clockValue) ==
  /\ TimeoutPhysicalControlPacketOwner(item)
  /\ clockValue = asyncNow
  /\ snapshot = TimeoutPhysicalControlFrozenSnapshot(clockValue)
  /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue)

TimeoutPhysicalControlFrozenPredecessorSet(snapshot) ==
  snapshot.predecessors

TimeoutPhysicalControlFrozenPhysicalCut(item, snapshot) ==
  snapshot.physicalCuts[item.envelope.recipient]

TimeoutPhysicalControlFrozenCausalEpisodeRank(item, snapshot) ==
  ExactDecisionTargetNeutralCausalEpisodeRankForSnapshot(
    snapshot, item.envelope.recipient)

TimeoutPhysicalControlFrozenProoflessProducerRank(item, snapshot) ==
  ExactDecisionTargetNeutralProoflessProducerRankForSnapshot(
    snapshot, item.envelope.recipient)

TimeoutPhysicalControlFrozenComposedCausalEpisodeRank(item, snapshot) ==
  ExactDecisionTargetNeutralComposedCausalEpisodeRankForSnapshot(
    snapshot, item.envelope.recipient)

TimeoutPhysicalControlFrozenProducerEpisodeRank(snapshot) ==
  ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)

TimeoutPhysicalControlDependencyCertificate(item, snapshot) ==
  [stage |-> TimeoutPhysicalControlLifecycleStageRank(item),
   packetRank |-> TimeoutPhysicalControlPacketDependencyRank(item, snapshot),
   ingressRank |-> TimeoutPhysicalControlIngressDependencyRank(item),
   physicalCut |->
     TimeoutPhysicalControlFrozenPhysicalCut(item, snapshot),
   predecessors |->
     TimeoutPhysicalControlFrozenPredecessorSet(snapshot),
   prooflessProducerRank |->
     TimeoutPhysicalControlFrozenProoflessProducerRank(item, snapshot),
   causalEpisodeRank |->
     TimeoutPhysicalControlFrozenCausalEpisodeRank(item, snapshot),
   composedCausalEpisodeRank |->
     TimeoutPhysicalControlFrozenComposedCausalEpisodeRank(item, snapshot),
   producerEpisodeRank |->
     TimeoutPhysicalControlFrozenProducerEpisodeRank(snapshot)]

THEOREM TimeoutPhysicalControlLifecycleStageOrderingIsWellFounded ==
  IsWellFoundedOn(
    TimeoutPhysicalControlLifecycleStageOrdering,
    TimeoutPhysicalControlLifecycleStageCarrier)
BY NatLessThanWellFounded, Isa
   DEF TimeoutPhysicalControlLifecycleStageOrdering,
       TimeoutPhysicalControlLifecycleStageCarrier

THEOREM TimeoutPhysicalControlSnapshotPinsPastPhysicalCut ==
  \A item \in AsyncNetworkItems, snapshot:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, asyncNow)
    /\ TimeoutPhysicalControlItem(item)
    /\ [AsyncNext]_AsyncAllVars
    => /\ TimeoutPhysicalControlFrozenPhysicalCut(item, snapshot) \in Nat
       /\ TimeoutPhysicalControlFrozenPhysicalCut(item, snapshot)
            <= AsyncNextIngressPhysicalOrdinal(
                 item.envelope.recipient)
       /\ TimeoutPhysicalControlFrozenPhysicalCut(item, snapshot)
            <= AsyncNextIngressPhysicalOrdinal(
                 item.envelope.recipient)'
       /\ TimeoutPhysicalControlFrozenPhysicalCut(item, snapshot)'
            = TimeoutPhysicalControlFrozenPhysicalCut(item, snapshot)
BY ExactDecisionTargetNeutralFrozenPhysicalCutsRemainPastOrCurrent,
   ExactDecisionTargetNeutralFrozenSnapshotCarriersArePrimeInvariant,
   IsaT(120)
   DEF TimeoutPhysicalControlFrozenPhysicalCut,
       TimeoutPhysicalControlItem,
       ExactDecisionTargetNeutralSnapshotActive,
       AsyncAllVars

\* A timeout/control item is not itself a leader-wire lifecycle.  Its packet
\* frontier therefore freezes the physical cut of the competing leader-wire
\* producer lifecycles directly, before the control item enters ingress.
THEOREM TimeoutPhysicalControlPacketSnapshotCapturesPhysicalCut ==
  \A item \in AsyncNetworkItems,
     snapshot, clockValue:
    TimeoutPhysicalControlPacketSnapshotAtCut(
      item, snapshot, clockValue)
      => TimeoutPhysicalControlFrozenPhysicalCut(item, snapshot)
           = AsyncNextIngressPhysicalOrdinal(item.envelope.recipient)
BY Isa
   DEF TimeoutPhysicalControlPacketSnapshotAtCut,
       TimeoutPhysicalControlFrozenSnapshot,
       TimeoutPhysicalControlFrozenPhysicalCut,
       ExactDecisionTargetNeutralFixedClockSnapshot,
       ExactDecisionTargetNeutralCurrentPhysicalCuts

THEOREM TimeoutPhysicalControlPacketRankUsesFrozenExactOccurrence ==
  \A item, snapshot:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, asyncNow)
    /\ TimeoutPhysicalControlPacketOwner(item)
    /\ TimeoutPhysicalControlSelectedPacket(item)
         \in OverdueResponsivePackets
    => TimeoutPhysicalControlPacketDependencyRank(item, snapshot)
         \in HistoricalDiscoveryPacketDependencyCarrier
BY ExactDecisionTargetNeutralPacketDependencyRankForSnapshotInCarrier,
   Isa
   DEF TimeoutPhysicalControlPacketDependencyRank

THEOREM TimeoutPhysicalControlProducerEpisodeRankUsesFrozenPastCut ==
  \A item \in AsyncNetworkItems, snapshot:
    /\ AsyncStrongTypeInvariant
    /\ ExactDecisionTargetNeutralSnapshotActive(snapshot, asyncNow)
    /\ TimeoutPhysicalControlItem(item)
    => /\ TimeoutPhysicalControlFrozenProoflessProducerRank(item, snapshot)
             \in ExactDecisionTargetNeutralProoflessProducerCarrier
       /\ TimeoutPhysicalControlFrozenCausalEpisodeRank(item, snapshot)
             \in AsyncCausalEpisodeStructuralRankCarrier
       /\ TimeoutPhysicalControlFrozenComposedCausalEpisodeRank(
             item, snapshot)
             \in ExactDecisionTargetNeutralComposedCausalEpisodeCarrier
       /\ TimeoutPhysicalControlFrozenProducerEpisodeRank(snapshot)
             \in ExactDecisionTargetNeutralProducerEpisodeCarrier
       /\ IsWellFoundedOn(
            ExactDecisionTargetNeutralProducerEpisodeOrdering,
            ExactDecisionTargetNeutralProducerEpisodeCarrier)
BY ExactDecisionTargetNeutralEpisodeRankIsInCarrier,
   ExactDecisionTargetNeutralProoflessProducerOrderingIsWellFounded,
   ExactDecisionTargetNeutralComposedCausalEpisodeOrderingIsWellFounded,
   ExactDecisionTargetNeutralProducerEpisodeOrderingIsWellFounded
   DEF TimeoutPhysicalControlFrozenProoflessProducerRank,
       TimeoutPhysicalControlFrozenCausalEpisodeRank,
       TimeoutPhysicalControlFrozenComposedCausalEpisodeRank,
       TimeoutPhysicalControlFrozenProducerEpisodeRank

THEOREM TimeoutPhysicalControlIngressRankUsesExactAdmissionOrdinal ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutPhysicalControlIngressOwner(item)
    => TimeoutPhysicalControlIngressDependencyRank(item)
         \in ExactDecisionRequestIngressRankCarrier
BY ExactDecisionRequestIngressPriorityDebtIsNatural,
   ExactDecisionRequestIngressServeCapacityDebtIsNatural,
   CandidateSequenceIndexIsPosition,
   DrainableIngressTurnReachRankIsNatural, IsaT(300)
   DEF TimeoutPhysicalControlIngressDependencyRank,
       TimeoutPhysicalControlIngressOwner,
       TimeoutPhysicalControlItem,
       ExactDecisionRequestIngressRank,
       ExactDecisionRequestIngressRankCarrier,
       ExactDecisionRequestIngressCapacityRank,
       ExactDecisionRequestIngressReachSelectorRank,
       ExactDecisionRequestIngressSelectorRank,
       ExactDecisionRequestIngressLaneRank,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressServeCapacityDebt,
       ExactDecisionRequestIngressPriorityDebt,
       ExactDecisionRequestIngressPriorityOwners,
       ExactDecisionRequestIngressLanePosition,
       ExactDecisionRequestIngressLaneIndices,
       ExactDecisionRequestIngressSourcePosition,
       ExactDecisionRequestIngressReachRank,
       ExactDecisionServeLifecycleIdentity,
       AsyncServeLiveReservationOwned, AsyncServeJobQueued,
       IngressSourceServiceRank, IngressResourceSource,
       IngressLane, SequenceSet,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIngressTypeInvariant, AsyncIngressTopologyTypeInvariant,
       AsyncIngressContentTypeInvariant

TimeoutPhysicalControlRetainedKernelProperty(specification) ==
  specification
    => \A item \in AsyncNetworkItems:
         TimeoutPhysicalControlRetainedOwner(item)
           ~> (TimeoutPhysicalControlTerminal(item)
                \/ TimeoutPhysicalControlPacketOwner(item)
                \/ TimeoutPhysicalControlIngressOwner(item)
                \/ TimeoutPhysicalControlCandidateOwner(item))

TimeoutPhysicalControlPacketKernelProperty(specification) ==
  specification
    => \A item \in AsyncNetworkItems:
         TimeoutPhysicalControlPacketOwner(item)
           ~> (TimeoutPhysicalControlTerminal(item)
                \/ TimeoutPhysicalControlIngressOwner(item)
                \/ TimeoutPhysicalControlCandidateOwner(item))

TimeoutPhysicalControlIngressKernelProperty(specification) ==
  specification
    => \A item \in AsyncNetworkItems:
         TimeoutPhysicalControlIngressOwner(item)
           ~> (TimeoutPhysicalControlTerminal(item)
                \/ TimeoutPhysicalControlCandidateOwner(item))

TimeoutPhysicalControlTransportKernelProperties(specification) ==
  /\ TimeoutPhysicalControlRetainedKernelProperty(specification)
  /\ TimeoutPhysicalControlPacketKernelProperty(specification)
  /\ TimeoutPhysicalControlIngressKernelProperty(specification)

TimeoutPhysicalControlRetainedClockAtRank(item, rank) ==
  /\ rank \in Nat
  /\ rank > 0
  /\ TimeoutPhysicalControlRetainedOwner(item)
  /\ ~TimeoutPhysicalControlGoal(item)
  /\ ~TimeoutPhysicalControlPacketOwner(item)
  /\ ~TimeoutPhysicalControlIngressOwner(item)
  /\ asyncNow < asyncRetransmitDeadlines[item.source]
  /\ asyncRetransmitDeadlines[item.source] = asyncNow + rank

TimeoutPhysicalControlRetainedDueOwner(item) ==
  /\ TimeoutPhysicalControlRetainedOwner(item)
  /\ ~TimeoutPhysicalControlGoal(item)
  /\ ~TimeoutPhysicalControlPacketOwner(item)
  /\ ~TimeoutPhysicalControlIngressOwner(item)
  /\ asyncNow >= asyncRetransmitDeadlines[item.source]

THEOREM TimeoutPhysicalControlRetainedClockHasNaturalRankOrIsDue ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutPhysicalControlRetainedOwner(item)
    /\ ~TimeoutPhysicalControlGoal(item)
    /\ ~TimeoutPhysicalControlPacketOwner(item)
    /\ ~TimeoutPhysicalControlIngressOwner(item)
    => \/ TimeoutPhysicalControlRetainedDueOwner(item)
       \/ \E rank \in Nat:
            TimeoutPhysicalControlRetainedClockAtRank(item, rank)
BY SMT
   DEF TimeoutPhysicalControlRetainedClockAtRank,
       TimeoutPhysicalControlRetainedDueOwner,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant

THEOREM TimeoutPhysicalControlTickLowersRetainedClockRank ==
  \A item, rank \in Nat:
    /\ TimeoutPhysicalControlRetainedClockAtRank(item, rank)
    /\ AsyncTick
    => \/ TimeoutPhysicalControlGoal(item)'
       \/ TimeoutPhysicalControlPacketOwner(item)'
       \/ TimeoutPhysicalControlIngressOwner(item)'
       \/ TimeoutPhysicalControlRetainedDueOwner(item)'
       \/ \E lowerRank \in SetLessThan(
              rank, OpToRel(<, Nat), Nat):
            TimeoutPhysicalControlRetainedClockAtRank(
              item, lowerRank)'
BY SMT
   DEF TimeoutPhysicalControlRetainedClockAtRank,
       TimeoutPhysicalControlRetainedDueOwner,
       TimeoutPhysicalControlGoal,
       TimeoutPhysicalControlRetainedOwner,
       TimeoutPhysicalControlPacketOwner,
       TimeoutPhysicalControlIngressOwner,
       AsyncTick, AsyncNonClockVars

THEOREM TimeoutPhysicalControlRetransmissionCreatesExactPacket ==
  \A node \in ValidatorIds, item:
    /\ TimeoutPhysicalControlRetainedDueOwner(item)
    /\ item.source = node
    /\ UNCHANGED vars
    /\ SendNodeRetransmissions(node)
    => TimeoutPhysicalControlPacketOwner(item)'
BY IsaT(240)
   DEF TimeoutPhysicalControlRetainedDueOwner,
       TimeoutPhysicalControlRetainedOwner,
       TimeoutPhysicalControlPacketOwner,
       TimeoutPhysicalControlItem,
       SendNodeRetransmissions, RetryableItems,
       RetainedControlEmissionItems, SendableItems,
       PacketsForItems, PacketForItem, ExactPacketOwns

THEOREM TimeoutPhysicalControlPacketAdmissionPreservesExactHandoff ==
  \A item, packet:
    LET recipient == item.envelope.recipient
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ TimeoutPhysicalControlPacketOwner(item)
       /\ packet \in TimeoutPhysicalControlExactPackets(item)
       /\ packet = OldestDueSourcePacket(recipient, item.source)
       /\ AdmitIngressPacket(recipient, item.source)
       => \/ TimeoutPhysicalControlTerminal(item)'
          \/ TimeoutPhysicalControlIngressOwner(item)'
          \/ TimeoutPhysicalControlCandidateOwner(item)'
BY AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   IsaT(900)
   DEF TimeoutPhysicalControlPacketOwner,
       TimeoutPhysicalControlIngressOwner,
       TimeoutPhysicalControlCandidateOwner,
       TimeoutPhysicalControlTerminal,
       TimeoutPhysicalControlItem,
       TimeoutPhysicalControlExactPackets,
       ExactPacketOwns, ExactIngressOwns,
       ExactDeliveryCandidateOwns,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       IngressHasCoalescingOwner, IngressPacketPolicyRejected,
       IngressLane, IngressResourceSource, SequenceSet,
       CandidateScheduled, DeliveryCandidate

THEOREM TimeoutPhysicalControlIngressDrainPreservesExactHandoff ==
  \A item:
    LET node == item.envelope.recipient
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ TimeoutPhysicalControlIngressOwner(item)
       /\ SelectedIngressItemAt(
            node, FirstDrainableIngressIndex(node)) = item
       /\ DrainFairIngressSelected(node)
       => \/ TimeoutPhysicalControlTerminal(item)'
          \/ TimeoutPhysicalControlCandidateOwner(item)'
BY AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   IsaT(1200)
   DEF TimeoutPhysicalControlIngressOwner,
       TimeoutPhysicalControlCandidateOwner,
       TimeoutPhysicalControlTerminal,
       TimeoutPhysicalControlItem,
       ExactIngressOwns, ExactDeliveryCandidateOwns,
       DrainFairIngressSelected, EnqueueCandidate,
       CandidateAdmissionCoalesced, CandidateScheduled,
       CandidateScheduledIn, DeliveryCandidate,
       IngressLane, IngressResourceSource, SequenceSet

(***************************************************************************
The proof below uses the exact source retransmission deadline before a packet
exists.  Once the immutable packet exists, the route-neutral packet rank
consumes the finite due prefix.  Admission transfers the same item to its
reserved ingress ordinal, whose exact lexicographic rank drains all frozen
priority/lane/source/runner predecessors.  Equal-count Candidate/Serve owner
replacement is charged by the exact occurrence/work/reach tail below the
frozen past scheduler cut.  Count-increasing source replenishment consumes
the source-qualified ingress journal coordinate; it is not itself called
progress.  Tombstones make the final item-to-candidate handoff monotone under
duplicate retransmission.
***************************************************************************)

THEOREM AsyncSpecProvidesTimeoutPhysicalControlTransportKernels ==
  \A initialContext:
    TimeoutPhysicalControlTransportKernelProperties(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   TimeoutViewOwnershipKernelInvariantFromAsyncSpec,
   ExactDecisionAsyncSpecAlwaysCandidateTombstones,
   ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded,
   ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets,
   ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor,
   ExactDecisionTargetNeutralRetainedEpisodesDoNotReplenish,
   ExactDecisionTargetNeutralRetainedEpisodeConsumptionLowersRank,
   ExactDecisionTargetNeutralProducerEpisodeBottomForcesStrictRankGoal,
   ExactDecisionTargetNeutralEpisodeRankIsInCarrier,
   ExactDecisionTargetNeutralProducerEpisodeOrderingIsWellFounded,
   ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness,
   ExactDecisionRequestIngressRankOrderingIsWellFounded,
   TimeoutPhysicalControlLifecycleStageOrderingIsWellFounded,
   TimeoutPhysicalControlSnapshotPinsPastPhysicalCut,
   TimeoutPhysicalControlPacketSnapshotCapturesPhysicalCut,
   TimeoutPhysicalControlPacketRankUsesFrozenExactOccurrence,
   TimeoutPhysicalControlProducerEpisodeRankUsesFrozenPastCut,
   TimeoutPhysicalControlIngressRankUsesExactAdmissionOrdinal,
   TimeoutPhysicalControlRetainedClockHasNaturalRankOrIsDue,
   TimeoutPhysicalControlTickLowersRetainedClockRank,
   TimeoutPhysicalControlRetransmissionCreatesExactPacket,
   TimeoutPhysicalControlPacketAdmissionPreservesExactHandoff,
   TimeoutPhysicalControlIngressDrainPreservesExactHandoff,
   AsyncRetainedCommitQcRetransmissionCreatesExactPacket,
   AsyncRetainedCommitQcPacketAdmissionCreatesExactIngressOwner,
   AsyncRetainedCommitQcIngressCreatesExactDeliverQcOwner,
   PTL, IsaT(7200)
   DEF TimeoutPhysicalControlTransportKernelProperties,
       TimeoutPhysicalControlRetainedKernelProperty,
       TimeoutPhysicalControlPacketKernelProperty,
       TimeoutPhysicalControlIngressKernelProperty,
       TimeoutPhysicalControlRetainedOwner,
       TimeoutPhysicalControlPacketOwner,
       TimeoutPhysicalControlIngressOwner,
       TimeoutPhysicalControlCandidateOwner,
       TimeoutPhysicalControlTerminal,
       TimeoutPhysicalControlGoal,
       TimeoutPhysicalControlItem,
       TimeoutPhysicalControlRetainedClockAtRank,
       TimeoutPhysicalControlRetainedDueOwner,
       TimeoutPhysicalControlDependencyCertificate,
       TimeoutPhysicalControlLifecycleStageRank,
       TimeoutPhysicalControlPacketDependencyRank,
       TimeoutPhysicalControlIngressDependencyRank,
       TimeoutPhysicalControlFrozenSnapshot,
       TimeoutPhysicalControlPacketSnapshotAtCut,
       TimeoutPhysicalControlFrozenPredecessorSet,
       TimeoutPhysicalControlFrozenPhysicalCut,
       TimeoutPhysicalControlFrozenCausalEpisodeRank,
       TimeoutPhysicalControlFrozenProducerEpisodeRank,
       TimeoutPhysicalControlExactPackets,
       TimeoutPhysicalControlSelectedPacket,
       TimeoutVoteRetainedControlOwner,
       TimeoutVotePacketOwner, TimeoutVoteIngressOwner,
       TimeoutVoteReducerCandidateOwner,
       TimeoutTcRetainedControlOwner, TimeoutTcPacketOwner,
       TimeoutTcIngressOwner, TimeoutTcReducerCandidateOwner,
       TimeoutDecisionRetainedControlOwner,
       TimeoutDecisionPacketOwner, TimeoutDecisionIngressOwner,
       TimeoutDecisionReducerCandidateOwner,
       TimeoutVoteItem, TimeoutCertificateItem,
       CommitCertificateItem, ExactPacketOwns, ExactIngressOwns,
       ExactDeliveryCandidateOwns, AsyncSpecAt, AsyncFairnessAt,
       AsyncAllVars

TimeoutRetainedPacketIngressKernelProperties(specification) ==
  /\ TimeoutVoteRetainedControlKernelProperty(specification)
  /\ TimeoutVotePacketKernelProperty(specification)
  /\ TimeoutVoteIngressKernelProperty(specification)
  /\ TimeoutTcRetainedControlKernelProperty(specification)
  /\ TimeoutTcPacketKernelProperty(specification)
  /\ TimeoutTcIngressKernelProperty(specification)
  /\ TimeoutDecisionRetainedControlKernelProperty(specification)
  /\ TimeoutDecisionPacketKernelProperty(specification)
  /\ TimeoutDecisionIngressKernelProperty(specification)

THEOREM TimeoutPhysicalControlTransportKernelsProjectDeclaredLeaves ==
  \A specification:
    TimeoutPhysicalControlTransportKernelProperties(specification)
      => TimeoutRetainedPacketIngressKernelProperties(specification)
BY Isa, PTL
   DEF TimeoutPhysicalControlTransportKernelProperties,
       TimeoutPhysicalControlRetainedKernelProperty,
       TimeoutPhysicalControlPacketKernelProperty,
       TimeoutPhysicalControlIngressKernelProperty,
       TimeoutRetainedPacketIngressKernelProperties,
       TimeoutPhysicalControlRetainedOwner,
       TimeoutPhysicalControlPacketOwner,
       TimeoutPhysicalControlIngressOwner,
       TimeoutPhysicalControlCandidateOwner,
       TimeoutPhysicalControlTerminal,
       TimeoutPhysicalControlItem,
       TimeoutVoteRetainedControlKernelProperty,
       TimeoutVotePacketKernelProperty,
       TimeoutVoteIngressKernelProperty,
       TimeoutTcRetainedControlKernelProperty,
       TimeoutTcPacketKernelProperty,
       TimeoutTcIngressKernelProperty,
       TimeoutDecisionRetainedControlKernelProperty,
       TimeoutDecisionPacketKernelProperty,
       TimeoutDecisionIngressKernelProperty,
       TimeoutVoteRetainedControlOwner,
       TimeoutVotePacketOwner, TimeoutVoteIngressOwner,
       TimeoutVoteReducerCandidateOwner,
       TimeoutTcRetainedControlOwner, TimeoutTcPacketOwner,
       TimeoutTcIngressOwner, TimeoutTcReducerCandidateOwner,
       TimeoutTcDeliveryTerminalOwner,
       TimeoutDecisionRetainedControlOwner,
       TimeoutDecisionPacketOwner, TimeoutDecisionIngressOwner,
       TimeoutDecisionReducerCandidateOwner,
       TimeoutDecisionDeliveryTerminalOwner,
       TimeoutVoteDeliveryKernelSource,
       TimeoutTcKernelSource, TimeoutDecisionKernelSource,
       TimeoutCertificateSemanticIdentity,
       TimeoutDirectGoal, TimeoutViewGoal

THEOREM AsyncSpecProvidesTimeoutRetainedPacketIngressKernels ==
  \A initialContext:
    TimeoutRetainedPacketIngressKernelProperties(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesTimeoutPhysicalControlTransportKernels,
   TimeoutPhysicalControlTransportKernelsProjectDeclaredLeaves

THEOREM AsyncLiveProvidesTimeoutRetainedPacketIngressKernels ==
  \A initialContext:
    TimeoutRetainedPacketIngressKernelProperties(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesTimeoutRetainedPacketIngressKernels
   DEF AsyncLiveSpecAt

(***************************************************************************
Retired standalone TC formation.

Receipt processing forms and schedules the exact install authority
atomically.  `FormTC` is a compatibility tombstone and no transition may
schedule a `FormTC` candidate.  The formation leaf is therefore discharged
by reachable absence, not by treating the disabled action as progress.
***************************************************************************)

TimeoutRetiredFormTcCandidateAbsent ==
  \A candidate \in AsyncCandidateSet:
    candidate.kind = "FormTC" => ~CandidateScheduled(candidate)

THEOREM AsyncInitEstablishesRetiredFormTcCandidateAbsence ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => TimeoutRetiredFormTcCandidateAbsent
BY IsaT(300)
   DEF TimeoutRetiredFormTcCandidateAbsent,
       CandidateScheduled, CandidateScheduledIn,
       AsyncInitAt, AsyncBaseInitAt, InitAt

THEOREM AsyncBracketPreservesRetiredFormTcCandidateAbsence ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ TimeoutRetiredFormTcCandidateAbsent
  /\ [AsyncNext]_AsyncAllVars
  => TimeoutRetiredFormTcCandidateAbsent'
BY RetiredStandaloneFormTcActionIsDisabled,
   CommandSuccessorInventoryIsClosed, IsaT(3600)
   DEF TimeoutRetiredFormTcCandidateAbsent,
       CandidateScheduled, CandidateScheduledAfter,
       CommandSuccessors, CausalCandidate,
       CausalCandidateWithEvidence,
       AppendCausalSuccessors, FreshCommandSuccessors,
       EnqueueCandidate, DeliveryCandidate,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunHistoricalServer, RunNodeWork, LocalAdmissionStep,
       SelectedLocalAdmissionAdvance, IngressDrainStep,
       SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, AsyncAllVars

THEOREM AsyncSpecAlwaysExcludesRetiredFormTcCandidates ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []TimeoutRetiredFormTcCandidateAbsent
BY AsyncInitEstablishesRetiredFormTcCandidateAbsence,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncBracketPreservesRetiredFormTcCandidateAbsence,
   PTL
   DEF AsyncSpecAt

THEOREM AsyncSpecProvidesTimeoutTcFormationReducerKernel ==
  \A initialContext:
    TimeoutTcFormationReducerKernelProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysExcludesRetiredFormTcCandidates,
   RetiredStandaloneFormTcActionIsDisabled,
   PTL, Isa
   DEF TimeoutTcFormationReducerKernelProperty,
       TimeoutRetiredFormTcCandidateAbsent,
       TimeoutFormTcCandidateOwned

THEOREM AsyncLiveProvidesTimeoutTcFormationReducerKernel ==
  \A initialContext:
    TimeoutTcFormationReducerKernelProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesTimeoutTcFormationReducerKernel
   DEF AsyncLiveSpecAt

(***************************************************************************
CommitQC origin and request/response closure.

Every exact `DecisionSourceAt` owned by a responsive voter retains its full
CommitQC outbox for the frozen roster.  Consequently the exact direct
delivery disjunct is already true for every applied-authority and every
request/response physical owner.  This is a direct-propagation handoff for
the same `(source,target,qc)`, not Decision at another node and not a claim
that request replenishment itself made progress.
***************************************************************************)

THEOREM TimeoutDecisionSourceRetainsExactDirectDelivery ==
  \A source, target, qc:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipKernelInvariant
    /\ TimeoutDecisionKernelSource(source, target, qc)
    => /\ TimeoutDecisionRetainedControlOwner(source, target, qc)
       /\ CommitCertificateDelivery(source, target, qc)
       /\ TimeoutDecisionRoundTripTerminalOwner(source, target, qc)
BY IsaT(300)
   DEF TimeoutViewOwnershipKernelInvariant,
       ResponsiveDecisionCertificateAuthorityInvariant,
       DecisionCertificateRetainedAuthority,
       TimeoutDecisionKernelSource,
       TimeoutDecisionRetainedControlOwner,
       TimeoutDecisionRoundTripTerminalOwner,
       CommitCertificateDelivery, CommitCertificateItem,
       QcOutbox, AsyncCurrentResponsiveVoters,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant

THEOREM AsyncSpecProvidesTimeoutDecisionOriginKernels ==
  \A initialContext:
    TimeoutDecisionOriginKernelProperties(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   TimeoutViewOwnershipKernelInvariantFromAsyncSpec,
   TimeoutDecisionSourceRetainsExactDirectDelivery,
   PTL, IsaT(900)
   DEF TimeoutDecisionOriginKernelProperties,
       TimeoutDecisionAppliedAuthorityOriginKernelProperty,
       TimeoutDecisionRoundTripPhysicalKernelProperties,
       TimeoutDecisionActiveRequestKernelProperty,
       TimeoutDecisionRequestPacketKernelProperty,
       TimeoutDecisionRequestIngressKernelProperty,
       TimeoutDecisionRequestServeKernelProperty,
       TimeoutDecisionResponsePacketKernelProperty,
       TimeoutDecisionResponseIngressKernelProperty,
       TimeoutDecisionResponseCandidateKernelProperty,
       TimeoutDecisionRoundTripTerminalOwner,
       TimeoutDecisionActiveRequestOwner,
       TimeoutDecisionRequestPacketOwner,
       TimeoutDecisionRequestIngressOwner,
       TimeoutDecisionRequestServeOwner,
       TimeoutDecisionResponsePacketOwner,
       TimeoutDecisionResponseIngressOwner,
       TimeoutDecisionResponseCandidateOwner

THEOREM AsyncLiveProvidesTimeoutDecisionOriginKernels ==
  \A initialContext:
    TimeoutDecisionOriginKernelProperties(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesTimeoutDecisionOriginKernels
   DEF AsyncLiveSpecAt

(***************************************************************************
Unconditional direct timeout residual.

Each conjunct below is provided independently: retained/packet/ingress,
exact delivery candidates, imported certificate reducer/WAL tails, retired
formation absence, and exact Decision-origin propagation.  No aggregate
timeout theorem appears in the dependency list.
***************************************************************************)

THEOREM AsyncSpecProvidesDirectTimeoutViewClosureResidual ==
  \A initialContext:
    DirectTimeoutViewClosureResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesTimeoutRetainedPacketIngressKernels,
   AsyncSpecProvidesTimeoutExactDeliveryCandidateKernels,
   AsyncSpecProvidesTimeoutImportedCertificateReducerWalKernels,
   AsyncSpecProvidesTimeoutTcFormationReducerKernel,
   AsyncSpecProvidesTimeoutDecisionOriginKernels
   DEF DirectTimeoutViewClosureResidualProperty,
       TimeoutVoteDeliveryPhysicalKernelProperties,
       TimeoutTcPhysicalKernelProperties,
       TimeoutDecisionDirectPhysicalKernelProperties,
       TimeoutCertificateDecisionPhysicalKernelProperties,
       TimeoutRetainedPacketIngressKernelProperties,
       TimeoutExactDeliveryCandidateKernelProperties,
       TimeoutImportedCertificateReducerWalKernelProperties,
       TimeoutDecisionOriginKernelProperties

THEOREM AsyncLiveProvidesDirectTimeoutViewClosureResidual ==
  \A initialContext:
    DirectTimeoutViewClosureResidualProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesDirectTimeoutViewClosureResidual
   DEF AsyncLiveSpecAt

=============================================================================
