---- MODULE SumeragiV2AsyncHistoricalRecoveryServiceClosureProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryLivenessProofs,
        SumeragiV2HeightResetBoundaryClosureProofs

(***************************************************************************
Historical discovery-clock service closure.

Weak fairness of `AsyncTick` is not a clock-progress proof.  After GST the
tick is disabled by any overdue authenticated packet, due serialized runner,
or due nonempty I/O queue.  Moreover, an empty I/O queue with a stale service
deadline and a responsive validator which has not yet entered the timed
service set are latent blockers: enqueueing work or opening historical
recovery can disable a tick which was enabled in the preceding state.

This module isolates the finite, non-circular rank scaffold needed to close
that gap.  Its owner cohort is the whole finite responsive validator set, not
the frozen current voting roster.  The concrete runner and I/O partitions
therefore include all of the following:

  * current responsive voter runners;
  * responsive applied-archive server runners;
  * historical-recovery target runners outside the current roster;
  * ordinary archive I/O workers; and
  * historical-recovery I/O workers outside the current roster.

At a fixed clock value the lexicographic prefix precharges:

  1. responsive validators which may still enter `AsyncTimedServiceNodes`;
  2. every transport occurrence whose immutable deadline is already due;
  3. stale empty I/O gates which can become active without a clock step; and
  4. the exact active blocker class.

The final component refines the already-proved ingress dependency spine with
count-first candidate and Serve occurrence tails: lane-head shadows, ingress
capacity, timeout-byte and shared-completion owners, transport service
position, reset-aware runner reach, auxiliary/capacity work, and exact
candidate/Serve ownership.

Only structural facts are proved here.  In particular, this module does not
turn well-foundedness into temporal convergence by assuming an unproved
no-refill or fair-descent property.  The exact missing action-local edges are
listed at the end.  No theorem below assumes current-voter Decision
convergence, target-to-Decision progress, application liveness, or fairness
of an existential action union.
***************************************************************************)

HistoricalDiscoveryTerminal(node) ==
  \/ NodeHasDecision(node)
  \/ ~HistoricalRecoveryTarget(node)

HistoricalDiscoveryFixedClockExit(node, clockValue) ==
  \/ NodeHasDecision(node)
  \/ ~HistoricalRecoveryTarget(node)
  \/ /\ HistoricalRecoveryTarget(node)
        /\ ActiveCommitCertificateRequests(node) # {}
  \/ asyncNow > clockValue

HistoricalDiscoveryFixedClockPending(node, clockValue) ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ HistoricalRecoveryTarget(node)
  /\ ~NodeHasDecision(node)
  /\ ActiveCommitCertificateRequests(node) = {}
  /\ asyncNow = clockValue
  /\ clockValue < AsyncRoundTimeout

(***************************************************************************
Exact finite owner cohorts.

`HistoricalDiscoveryDuePacketsAt` deliberately includes every packet due at
the frozen clock, not only packets which are already classified as responsive
clock blockers.  If a responsive validator enters the timed-service set, a
pre-existing due packet can become overdue, but it was already charged here.
The temporal closure must separately prove from each publication action that
a fresh packet cannot refill this set while `asyncNow = clockValue`.
***************************************************************************)

HistoricalDiscoveryPotentialServiceCohort == Responsive

HistoricalDiscoveryRunnerOwners ==
  AsyncCurrentResponsiveVoters
    \cup AsyncResponsiveAppliedArchiveServers
    \cup asyncHistoricalRecoveryTargets

HistoricalDiscoveryIoOwners ==
  AsyncArchiveIoServiceNodes
    \cup asyncHistoricalRecoveryTargets

HistoricalDiscoveryNonVoterTargets ==
  asyncHistoricalRecoveryTargets
    \ AsyncCurrentResponsiveVoters

HistoricalDiscoveryLatentTimedOwners ==
  HistoricalDiscoveryPotentialServiceCohort
    \ AsyncTimedServiceNodes

HistoricalDiscoveryDuePacketsAt(clockValue) ==
  {packet \in asyncTransport: packet.deadline <= clockValue}

HistoricalDiscoveryNodeBlockersAt(clockValue) ==
  {owner \in AsyncTimedServiceNodes:
     asyncNodeServiceDeadlines[owner] <= clockValue}

HistoricalDiscoveryDormantIoGatesAt(clockValue) ==
  {owner \in AsyncTimedServiceNodes:
     /\ AsyncIoQueueDepth(owner) = 0
     /\ asyncIoServiceDeadlines[owner] <= clockValue}

HistoricalDiscoveryActiveIoBlockersAt(clockValue) ==
  {owner \in AsyncTimedServiceNodes:
     /\ AsyncIoQueueDepth(owner) > 0
     /\ asyncIoServiceDeadlines[owner] <= clockValue}

HistoricalDiscoveryLatentOwnerDebt ==
  Cardinality(HistoricalDiscoveryLatentTimedOwners)

HistoricalDiscoveryDuePacketDebt(clockValue) ==
  Cardinality(HistoricalDiscoveryDuePacketsAt(clockValue))

HistoricalDiscoveryDormantIoDebt(clockValue) ==
  Cardinality(HistoricalDiscoveryDormantIoGatesAt(clockValue))

HistoricalDiscoveryNodeBlockerDebt(clockValue) ==
  Cardinality(HistoricalDiscoveryNodeBlockersAt(clockValue))

HistoricalDiscoveryActiveIoBlockerDebt(clockValue) ==
  Cardinality(HistoricalDiscoveryActiveIoBlockersAt(clockValue))

THEOREM HistoricalDiscoveryOwnersIncludeNonVoterService ==
  /\ HistoricalDiscoveryRunnerOwners
       = AsyncCurrentResponsiveVoters
           \cup AsyncResponsiveAppliedArchiveServers
           \cup asyncHistoricalRecoveryTargets
  /\ HistoricalDiscoveryIoOwners
       = AsyncArchiveIoServiceNodes
           \cup asyncHistoricalRecoveryTargets
  /\ HistoricalDiscoveryRunnerOwners = AsyncTimedServiceNodes
  /\ HistoricalDiscoveryIoOwners = AsyncTimedServiceNodes
  /\ HistoricalDiscoveryNonVoterTargets
       \subseteq HistoricalDiscoveryRunnerOwners
  /\ HistoricalDiscoveryNonVoterTargets
       \subseteq HistoricalDiscoveryIoOwners
BY Isa
   DEF HistoricalDiscoveryRunnerOwners,
       HistoricalDiscoveryIoOwners,
       HistoricalDiscoveryNonVoterTargets,
       AsyncTimedServiceNodes,
       AsyncArchiveIoServiceNodes

THEOREM StrongTypeHasFiniteHistoricalDiscoveryCohorts ==
  \A clockValue \in Nat:
    AsyncStrongTypeInvariant
      => /\ IsFiniteSet(HistoricalDiscoveryPotentialServiceCohort)
         /\ IsFiniteSet(HistoricalDiscoveryRunnerOwners)
         /\ IsFiniteSet(HistoricalDiscoveryIoOwners)
         /\ IsFiniteSet(HistoricalDiscoveryNonVoterTargets)
         /\ IsFiniteSet(HistoricalDiscoveryLatentTimedOwners)
         /\ IsFiniteSet(HistoricalDiscoveryDuePacketsAt(clockValue))
         /\ IsFiniteSet(HistoricalDiscoveryNodeBlockersAt(clockValue))
         /\ IsFiniteSet(
              HistoricalDiscoveryDormantIoGatesAt(clockValue))
         /\ IsFiniteSet(
              HistoricalDiscoveryActiveIoBlockersAt(clockValue))
         /\ HistoricalDiscoveryLatentOwnerDebt \in Nat
         /\ HistoricalDiscoveryDuePacketDebt(clockValue) \in Nat
         /\ HistoricalDiscoveryDormantIoDebt(clockValue) \in Nat
         /\ HistoricalDiscoveryNodeBlockerDebt(clockValue) \in Nat
         /\ HistoricalDiscoveryActiveIoBlockerDebt(clockValue) \in Nat
BY AsyncStrongTypeProjectsAsyncType,
   ResponsiveAreValidators,
   AsyncTimedServiceNodesAreValidators,
   HistoricalDiscoveryOwnersIncludeNonVoterService,
   FS_Subset, FS_CardinalityType, Isa
   DEF AsyncStrongTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ModelConfiguration, ValidatorIds,
       HistoricalDiscoveryPotentialServiceCohort,
       HistoricalDiscoveryRunnerOwners,
       HistoricalDiscoveryIoOwners,
       HistoricalDiscoveryNonVoterTargets,
       HistoricalDiscoveryLatentTimedOwners,
       HistoricalDiscoveryDuePacketsAt,
       HistoricalDiscoveryNodeBlockersAt,
       HistoricalDiscoveryDormantIoGatesAt,
       HistoricalDiscoveryActiveIoBlockersAt,
       HistoricalDiscoveryLatentOwnerDebt,
       HistoricalDiscoveryDuePacketDebt,
       HistoricalDiscoveryDormantIoDebt,
       HistoricalDiscoveryNodeBlockerDebt,
       HistoricalDiscoveryActiveIoBlockerDebt,
       AsyncTransportTypeInvariant,
       AsyncPacketContentTypeInvariant

(***************************************************************************
Exact fixed-clock blocker partition.

The three non-tick branches are precisely the three conjuncts which disable
`AsyncTick` after GST.  In particular, the node and I/O sets range over
`AsyncTimedServiceNodes`, so the witnesses are not restricted to current
voters.
***************************************************************************)

HistoricalDiscoveryFixedClockServiceCase(node, clockValue) ==
  \/ HistoricalDiscoveryTerminal(node)
  \/ OverdueResponsivePackets # {}
  \/ HistoricalDiscoveryNodeBlockersAt(clockValue) # {}
  \/ HistoricalDiscoveryActiveIoBlockersAt(clockValue) # {}
  \/ AsyncTickEnabled

THEOREM HistoricalDiscoveryFixedClockBlockerCharacterization ==
  \A clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ asyncNow = clockValue
    => (AsyncTickEnabled
          <=> /\ OverdueResponsivePackets = {}
              /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
              /\ HistoricalDiscoveryActiveIoBlockersAt(clockValue) = {})
BY AsyncStrongTypeProjectsAsyncType, Isa
   DEF AsyncStrongTypeInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncTickEnabled, AsyncIoQueueDepth,
       HistoricalDiscoveryNodeBlockersAt,
       HistoricalDiscoveryActiveIoBlockersAt

THEOREM HistoricalDiscoveryFixedClockPendingHasServiceCase ==
  \A node \in Responsive, clockValue \in Nat:
    HistoricalDiscoveryFixedClockPending(node, clockValue)
      => HistoricalDiscoveryFixedClockServiceCase(node, clockValue)
BY HistoricalDiscoveryFixedClockBlockerCharacterization
   DEF HistoricalDiscoveryFixedClockPending,
       HistoricalDiscoveryFixedClockServiceCase,
       HistoricalDiscoveryTerminal

(***************************************************************************
Canonical lower components.

The non-packet blocker classes do not invent a second ingress order.  They use
a canonical member of the existing product and place their exact finite
cardinality in its first natural component.  Packet blockers use the concrete
`IngressBoundaryDependencyRank` itself.
***************************************************************************)

HistoricalDiscoveryReadyInnerBottom == <<0, 0>>

HistoricalDiscoveryReadyTimeoutBottom ==
  <<0, HistoricalDiscoveryReadyInnerBottom>>

HistoricalDiscoveryReadyDeferredBottom ==
  <<0, HistoricalDiscoveryReadyTimeoutBottom>>

HistoricalDiscoveryReadyAuxBottom ==
  <<0, HistoricalDiscoveryReadyDeferredBottom>>

HistoricalDiscoveryStage4Bottom ==
  <<0, HistoricalDiscoveryReadyAuxBottom>>

HistoricalDiscoveryCandidateDebtBottom == <<2, 0>>

HistoricalDiscoveryServeDebtBottom == <<5, 0>>

HistoricalDiscoveryOccurrenceDebtCarrier ==
  Nat \X OwnedServiceRankCarrier

HistoricalDiscoveryOccurrenceDebtOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OwnedServiceRankOrdering,
    Nat, OwnedServiceRankCarrier)

HistoricalDiscoveryCandidateOccurrenceBottom ==
  <<0, HistoricalDiscoveryCandidateDebtBottom>>

HistoricalDiscoveryServeOccurrenceBottom ==
  <<0, HistoricalDiscoveryServeDebtBottom>>

HistoricalDiscoveryCandidateServeBottom ==
  <<HistoricalDiscoveryCandidateOccurrenceBottom,
    HistoricalDiscoveryServeOccurrenceBottom>>

HistoricalDiscoveryIngressStage4Bottom ==
  <<HistoricalDiscoveryStage4Bottom,
    HistoricalDiscoveryCandidateServeBottom>>

HistoricalDiscoveryIngressReadyBottom ==
  <<HistoricalDiscoveryReadyAuxBottom,
    HistoricalDiscoveryIngressStage4Bottom>>

HistoricalDiscoveryIngressResetBottom ==
  <<0, HistoricalDiscoveryIngressReadyBottom>>

HistoricalDiscoveryIngressTransportBottom ==
  <<0, HistoricalDiscoveryIngressResetBottom>>

HistoricalDiscoveryIngressCompletionBottom ==
  <<0, HistoricalDiscoveryIngressTransportBottom>>

HistoricalDiscoveryIngressTimeoutBottom ==
  <<0, HistoricalDiscoveryIngressCompletionBottom>>

HistoricalDiscoveryIngressCapacityBottom ==
  <<0, HistoricalDiscoveryIngressTimeoutBottom>>

HistoricalDiscoveryIngressCounterRank(counter) ==
  <<counter, HistoricalDiscoveryIngressCapacityBottom>>

(***************************************************************************
Historical packet dependency product.

The generic ingress product carries one plain candidate minimum and one plain
Serve minimum.  That shape is sufficient for a retained owner whose position
falls, but it is not monotone when the selected minimum is removed: the next
minimum may be larger.  Historical clock closure needs removal itself to be
progress, so both independent tails are refined with their exact occurrence
counts.  Every earlier capacity/selector/runner component stays in the same
order and therefore still dominates later producer handoffs.
***************************************************************************)

HistoricalDiscoveryCandidateServeTailCarrier ==
  HistoricalDiscoveryOccurrenceDebtCarrier
    \X HistoricalDiscoveryOccurrenceDebtCarrier

HistoricalDiscoveryCandidateServeTailOrdering ==
  LexPairOrdering(
    HistoricalDiscoveryOccurrenceDebtOrdering,
    HistoricalDiscoveryOccurrenceDebtOrdering,
    HistoricalDiscoveryOccurrenceDebtCarrier,
    HistoricalDiscoveryOccurrenceDebtCarrier)

HistoricalDiscoveryStage4TailCarrier ==
  Stage4CapacityCarrier
    \X HistoricalDiscoveryCandidateServeTailCarrier

HistoricalDiscoveryStage4TailOrdering ==
  LexPairOrdering(
    Stage4CapacityOrdering,
    HistoricalDiscoveryCandidateServeTailOrdering,
    Stage4CapacityCarrier,
    HistoricalDiscoveryCandidateServeTailCarrier)

HistoricalDiscoveryReadyTailCarrier ==
  ReadyRunAuxCarrier \X HistoricalDiscoveryStage4TailCarrier

HistoricalDiscoveryReadyTailOrdering ==
  LexPairOrdering(
    ReadyRunAuxOrdering,
    HistoricalDiscoveryStage4TailOrdering,
    ReadyRunAuxCarrier,
    HistoricalDiscoveryStage4TailCarrier)

HistoricalDiscoveryResetTailCarrier ==
  Nat \X HistoricalDiscoveryReadyTailCarrier

HistoricalDiscoveryResetTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), HistoricalDiscoveryReadyTailOrdering,
    Nat, HistoricalDiscoveryReadyTailCarrier)

HistoricalDiscoveryTransportTailCarrier ==
  Nat \X HistoricalDiscoveryResetTailCarrier

HistoricalDiscoveryTransportTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), HistoricalDiscoveryResetTailOrdering,
    Nat, HistoricalDiscoveryResetTailCarrier)

HistoricalDiscoveryCompletionTailCarrier ==
  Nat \X HistoricalDiscoveryTransportTailCarrier

HistoricalDiscoveryCompletionTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), HistoricalDiscoveryTransportTailOrdering,
    Nat, HistoricalDiscoveryTransportTailCarrier)

HistoricalDiscoveryTimeoutTailCarrier ==
  Nat \X HistoricalDiscoveryCompletionTailCarrier

HistoricalDiscoveryTimeoutTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), HistoricalDiscoveryCompletionTailOrdering,
    Nat, HistoricalDiscoveryCompletionTailCarrier)

HistoricalDiscoveryCapacityTailCarrier ==
  Nat \X HistoricalDiscoveryTimeoutTailCarrier

HistoricalDiscoveryCapacityTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), HistoricalDiscoveryTimeoutTailOrdering,
    Nat, HistoricalDiscoveryTimeoutTailCarrier)

HistoricalDiscoveryPacketDependencyCarrier ==
  Nat \X HistoricalDiscoveryCapacityTailCarrier

HistoricalDiscoveryPacketDependencyOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), HistoricalDiscoveryCapacityTailOrdering,
    Nat, HistoricalDiscoveryCapacityTailCarrier)

THEOREM HistoricalDiscoveryOccurrenceDebtOrderingIsWellFounded ==
  IsWellFoundedOn(
    HistoricalDiscoveryOccurrenceDebtOrdering,
    HistoricalDiscoveryOccurrenceDebtCarrier)
BY NatLessThanWellFounded,
   OwnedServiceRankOrderingWellFounded,
   WFLexPairOrdering
   DEF HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier

THEOREM HistoricalDiscoveryPacketDependencyOrderingIsWellFounded ==
  IsWellFoundedOn(
    HistoricalDiscoveryPacketDependencyOrdering,
    HistoricalDiscoveryPacketDependencyCarrier)
PROOF
  <1>1. IsWellFoundedOn(
           HistoricalDiscoveryCandidateServeTailOrdering,
           HistoricalDiscoveryCandidateServeTailCarrier)
    BY HistoricalDiscoveryOccurrenceDebtOrderingIsWellFounded,
       WFLexPairOrdering
       DEF HistoricalDiscoveryCandidateServeTailOrdering,
           HistoricalDiscoveryCandidateServeTailCarrier
  <1>2. IsWellFoundedOn(
           HistoricalDiscoveryStage4TailOrdering,
           HistoricalDiscoveryStage4TailCarrier)
    BY Stage4CapacityOrderingIsWellFounded, <1>1,
       WFLexPairOrdering
       DEF HistoricalDiscoveryStage4TailOrdering,
           HistoricalDiscoveryStage4TailCarrier
  <1>3. IsWellFoundedOn(
           HistoricalDiscoveryReadyTailOrdering,
           HistoricalDiscoveryReadyTailCarrier)
    BY ReadyRunAuxOrderingIsWellFounded, <1>2,
       WFLexPairOrdering
       DEF HistoricalDiscoveryReadyTailOrdering,
           HistoricalDiscoveryReadyTailCarrier
  <1>4. IsWellFoundedOn(
           HistoricalDiscoveryResetTailOrdering,
           HistoricalDiscoveryResetTailCarrier)
    BY NatLessThanWellFounded, <1>3, WFLexPairOrdering
       DEF HistoricalDiscoveryResetTailOrdering,
           HistoricalDiscoveryResetTailCarrier
  <1>5. IsWellFoundedOn(
           HistoricalDiscoveryTransportTailOrdering,
           HistoricalDiscoveryTransportTailCarrier)
    BY NatLessThanWellFounded, <1>4, WFLexPairOrdering
       DEF HistoricalDiscoveryTransportTailOrdering,
           HistoricalDiscoveryTransportTailCarrier
  <1>6. IsWellFoundedOn(
           HistoricalDiscoveryCompletionTailOrdering,
           HistoricalDiscoveryCompletionTailCarrier)
    BY NatLessThanWellFounded, <1>5, WFLexPairOrdering
       DEF HistoricalDiscoveryCompletionTailOrdering,
           HistoricalDiscoveryCompletionTailCarrier
  <1>7. IsWellFoundedOn(
           HistoricalDiscoveryTimeoutTailOrdering,
           HistoricalDiscoveryTimeoutTailCarrier)
    BY NatLessThanWellFounded, <1>6, WFLexPairOrdering
       DEF HistoricalDiscoveryTimeoutTailOrdering,
           HistoricalDiscoveryTimeoutTailCarrier
  <1>8. IsWellFoundedOn(
           HistoricalDiscoveryCapacityTailOrdering,
           HistoricalDiscoveryCapacityTailCarrier)
    BY NatLessThanWellFounded, <1>7, WFLexPairOrdering
       DEF HistoricalDiscoveryCapacityTailOrdering,
           HistoricalDiscoveryCapacityTailCarrier
  <1> QED
    BY NatLessThanWellFounded, <1>8, WFLexPairOrdering
       DEF HistoricalDiscoveryPacketDependencyOrdering,
           HistoricalDiscoveryPacketDependencyCarrier

THEOREM HistoricalDiscoveryIngressCounterRankInCarrier ==
  \A counter \in Nat:
    HistoricalDiscoveryIngressCounterRank(counter)
      \in HistoricalDiscoveryPacketDependencyCarrier
BY Isa
   DEF HistoricalDiscoveryIngressCounterRank,
       HistoricalDiscoveryIngressCapacityBottom,
       HistoricalDiscoveryIngressTimeoutBottom,
       HistoricalDiscoveryIngressCompletionBottom,
       HistoricalDiscoveryIngressTransportBottom,
       HistoricalDiscoveryIngressResetBottom,
       HistoricalDiscoveryIngressReadyBottom,
       HistoricalDiscoveryIngressStage4Bottom,
       HistoricalDiscoveryCandidateServeBottom,
       HistoricalDiscoveryCandidateOccurrenceBottom,
       HistoricalDiscoveryServeOccurrenceBottom,
       HistoricalDiscoveryCandidateDebtBottom,
       HistoricalDiscoveryServeDebtBottom,
       HistoricalDiscoveryStage4Bottom,
       HistoricalDiscoveryReadyAuxBottom,
       HistoricalDiscoveryReadyDeferredBottom,
       HistoricalDiscoveryReadyTimeoutBottom,
       HistoricalDiscoveryReadyInnerBottom,
       HistoricalDiscoveryPacketDependencyCarrier,
       HistoricalDiscoveryCapacityTailCarrier,
       HistoricalDiscoveryTimeoutTailCarrier,
       HistoricalDiscoveryCompletionTailCarrier,
       HistoricalDiscoveryTransportTailCarrier,
       HistoricalDiscoveryResetTailCarrier,
       HistoricalDiscoveryReadyTailCarrier,
       HistoricalDiscoveryStage4TailCarrier,
       HistoricalDiscoveryCandidateServeTailCarrier,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       Stage4CapacityCarrier, ReadyRunAuxCarrier,
       ReadyRunDeferredCarrier, ReadyRunTimeoutCarrier,
       ReadyRunInnerCarrier, OwnedServiceRankCarrier

(***************************************************************************
Fixed-clock blocker order.

The prefix order is:

  latent timed owner, due packet occurrence, dormant stale I/O gate,
  active blocker class, exact ingress dependency.

Class 3 is a due runner, class 2 a due active I/O worker, class 1 an overdue
packet, and class 0 the tick.  This is the concrete producer order: runner
service may expose I/O, and I/O service may publish a response packet, so both
handoffs move to a lower class.  Packet admission lowers the earlier due-set
cardinality before any successor blocker is considered.
***************************************************************************)

HistoricalDiscoveryBlockerStageCarrier == 0..3

HistoricalDiscoveryBlockerStageTailCarrier ==
  HistoricalDiscoveryBlockerStageCarrier
    \X HistoricalDiscoveryPacketDependencyCarrier

HistoricalDiscoveryBlockerStageTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), HistoricalDiscoveryPacketDependencyOrdering,
    HistoricalDiscoveryBlockerStageCarrier,
    HistoricalDiscoveryPacketDependencyCarrier)

HistoricalDiscoveryDormantTailCarrier ==
  Nat \X HistoricalDiscoveryBlockerStageTailCarrier

HistoricalDiscoveryDormantTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), HistoricalDiscoveryBlockerStageTailOrdering,
    Nat, HistoricalDiscoveryBlockerStageTailCarrier)

HistoricalDiscoveryDuePacketTailCarrier ==
  Nat \X HistoricalDiscoveryDormantTailCarrier

HistoricalDiscoveryDuePacketTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), HistoricalDiscoveryDormantTailOrdering,
    Nat, HistoricalDiscoveryDormantTailCarrier)

HistoricalDiscoveryFixedClockBlockerCarrier ==
  Nat \X HistoricalDiscoveryDuePacketTailCarrier

HistoricalDiscoveryFixedClockBlockerOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), HistoricalDiscoveryDuePacketTailOrdering,
    Nat, HistoricalDiscoveryDuePacketTailCarrier)

HistoricalDiscoveryFixedClockRank(
    clockValue, stage, dependencyRank) ==
  <<HistoricalDiscoveryLatentOwnerDebt,
    <<HistoricalDiscoveryDuePacketDebt(clockValue),
      <<HistoricalDiscoveryDormantIoDebt(clockValue),
        <<stage, dependencyRank>>>>>>>>

HistoricalDiscoveryFixedClockLexStep(
    clockValue, oldStage, oldDependency,
    nextStage, nextDependency) ==
  \/ HistoricalDiscoveryLatentOwnerDebt'
       < HistoricalDiscoveryLatentOwnerDebt
  \/ /\ HistoricalDiscoveryLatentOwnerDebt'
          = HistoricalDiscoveryLatentOwnerDebt
     /\ HistoricalDiscoveryDuePacketDebt(clockValue)'
          < HistoricalDiscoveryDuePacketDebt(clockValue)
  \/ /\ HistoricalDiscoveryLatentOwnerDebt'
          = HistoricalDiscoveryLatentOwnerDebt
     /\ HistoricalDiscoveryDuePacketDebt(clockValue)'
          = HistoricalDiscoveryDuePacketDebt(clockValue)
     /\ HistoricalDiscoveryDormantIoDebt(clockValue)'
          < HistoricalDiscoveryDormantIoDebt(clockValue)
  \/ /\ HistoricalDiscoveryLatentOwnerDebt'
          = HistoricalDiscoveryLatentOwnerDebt
     /\ HistoricalDiscoveryDuePacketDebt(clockValue)'
          = HistoricalDiscoveryDuePacketDebt(clockValue)
     /\ HistoricalDiscoveryDormantIoDebt(clockValue)'
          = HistoricalDiscoveryDormantIoDebt(clockValue)
     /\ nextStage < oldStage
  \/ /\ HistoricalDiscoveryLatentOwnerDebt'
          = HistoricalDiscoveryLatentOwnerDebt
     /\ HistoricalDiscoveryDuePacketDebt(clockValue)'
          = HistoricalDiscoveryDuePacketDebt(clockValue)
     /\ HistoricalDiscoveryDormantIoDebt(clockValue)'
          = HistoricalDiscoveryDormantIoDebt(clockValue)
     /\ nextStage = oldStage
     /\ <<nextDependency, oldDependency>>
          \in HistoricalDiscoveryPacketDependencyOrdering

THEOREM HistoricalDiscoveryFixedClockRankShapeInCarrier ==
  \A clockValue \in Nat,
     stage \in HistoricalDiscoveryBlockerStageCarrier,
     dependency \in HistoricalDiscoveryPacketDependencyCarrier:
    /\ HistoricalDiscoveryLatentOwnerDebt \in Nat
    /\ HistoricalDiscoveryDuePacketDebt(clockValue) \in Nat
    /\ HistoricalDiscoveryDormantIoDebt(clockValue) \in Nat
    => HistoricalDiscoveryFixedClockRank(
         clockValue, stage, dependency)
         \in HistoricalDiscoveryFixedClockBlockerCarrier
BY Isa
   DEF HistoricalDiscoveryFixedClockRank,
       HistoricalDiscoveryFixedClockBlockerCarrier,
       HistoricalDiscoveryDuePacketTailCarrier,
       HistoricalDiscoveryDormantTailCarrier,
       HistoricalDiscoveryBlockerStageTailCarrier

THEOREM HistoricalDiscoveryFixedClockLexStepStrictlyDescends ==
  \A clockValue \in Nat,
     oldStage, nextStage \in HistoricalDiscoveryBlockerStageCarrier,
     oldDependency, nextDependency
       \in HistoricalDiscoveryPacketDependencyCarrier:
    /\ HistoricalDiscoveryLatentOwnerDebt \in Nat
    /\ HistoricalDiscoveryLatentOwnerDebt' \in Nat
    /\ HistoricalDiscoveryDuePacketDebt(clockValue) \in Nat
    /\ HistoricalDiscoveryDuePacketDebt(clockValue)' \in Nat
    /\ HistoricalDiscoveryDormantIoDebt(clockValue) \in Nat
    /\ HistoricalDiscoveryDormantIoDebt(clockValue)' \in Nat
    /\ HistoricalDiscoveryFixedClockLexStep(
         clockValue, oldStage, oldDependency,
         nextStage, nextDependency)
    => <<HistoricalDiscoveryFixedClockRank(
            clockValue, nextStage, nextDependency)',
          HistoricalDiscoveryFixedClockRank(
            clockValue, oldStage, oldDependency)>>
         \in HistoricalDiscoveryFixedClockBlockerOrdering
BY Isa
   DEF HistoricalDiscoveryFixedClockLexStep,
       HistoricalDiscoveryFixedClockRank,
       HistoricalDiscoveryFixedClockBlockerOrdering,
       HistoricalDiscoveryDuePacketTailOrdering,
       HistoricalDiscoveryDormantTailOrdering,
       HistoricalDiscoveryBlockerStageTailOrdering,
       LexPairOrdering, OpToRel

THEOREM HistoricalDiscoveryFixedClockBlockerOrderingIsWellFounded ==
  IsWellFoundedOn(
    HistoricalDiscoveryFixedClockBlockerOrdering,
    HistoricalDiscoveryFixedClockBlockerCarrier)
PROOF
  <1>1. IsWellFoundedOn(
           HistoricalDiscoveryBlockerStageTailOrdering,
           HistoricalDiscoveryBlockerStageTailCarrier)
    BY NatLessThanWellFounded, IsWellFoundedOnSubset,
       HistoricalDiscoveryPacketDependencyOrderingIsWellFounded,
       WFLexPairOrdering, Isa
       DEF HistoricalDiscoveryBlockerStageTailOrdering,
           HistoricalDiscoveryBlockerStageTailCarrier,
           HistoricalDiscoveryBlockerStageCarrier
  <1>2. IsWellFoundedOn(
           HistoricalDiscoveryDormantTailOrdering,
           HistoricalDiscoveryDormantTailCarrier)
    BY NatLessThanWellFounded, <1>1, WFLexPairOrdering
       DEF HistoricalDiscoveryDormantTailOrdering,
           HistoricalDiscoveryDormantTailCarrier
  <1>3. IsWellFoundedOn(
           HistoricalDiscoveryDuePacketTailOrdering,
           HistoricalDiscoveryDuePacketTailCarrier)
    BY NatLessThanWellFounded, <1>2, WFLexPairOrdering
       DEF HistoricalDiscoveryDuePacketTailOrdering,
           HistoricalDiscoveryDuePacketTailCarrier
  <1> QED
    BY NatLessThanWellFounded, <1>3, WFLexPairOrdering
       DEF HistoricalDiscoveryFixedClockBlockerOrdering,
           HistoricalDiscoveryFixedClockBlockerCarrier

(***************************************************************************
Exact packet dependency product.

The former scaffold formed a set containing three whole dependency ranks
(the base, one rank per live historical candidate, and one rank per live
Serve occurrence) and selected an arbitrary member with `CHOOSE`.  That was
not a usable descent measure: lowering the base left the selected candidate
or Serve alternative unconstrained, and changing either owner set could make
`CHOOSE` select an unrelated whole rank.

The construction below keeps the packet-local base spine exactly once and
fills both of its independent tail slots simultaneously.  Each nonempty live
owner set contributes its minimal `OwnedServiceRank`; an empty set contributes
an explicit typed bottom.  `CHOOSE` is now used only to name the mathematical
minimum rank and an exact owner which realizes that rank.  It no longer
chooses between different dependency shapes.

The exact owner witnesses expose the individually fair actions needed by a
later temporal proof.  This module still proves only structural membership
and witness facts; it does not assert that either fair action occurs or that
its tail descends.  A witness `CHOOSE` is intentionally unconstrained when
its owner set is empty; every theorem which uses that witness requires the
corresponding owner set to be nonempty.
***************************************************************************)

HistoricalDiscoveryServeJobOwned(node, job) ==
  /\ node \in HistoricalDiscoveryIoOwners
  /\ job \in AsyncServeJobSet
  /\ job \in SequenceSet(asyncIoQueues[node])

(***************************************************************************
Frozen packet-derived producer lineage.

The occurrence tail must not range over every historical candidate or Serve
job at the packet recipient.  Such a recipient-wide set admits unrelated
fresh-view work forever when `ViewDomain = Nat`.  Candidate construction now
freezes a normalized, route-neutral `causalOrigin` at first admission and
preserves it through every ordinary successor, evidence rewrite, TC-install
child, and exact transport retry.  A crash-replay constructor starts a
separate deterministic durable-authority lifecycle unless the earlier packet
already reached its durable milestone; it is not claimed as preservation of
the original packet origin.  The tail can therefore select the exact live
packet lineage directly; mutable evidence and view rewrites no longer need an
incomplete hand-enumerated carrier.

Requests enter the independent Serve lifecycle, chunks/noise never create a
reducer candidate, and the two authenticated response forms use their exact
production projection constructors.  All unrelated work remains charged by
the ingress, capacity, selector, causal, runner, and I/O predecessor
coordinates which precede this tail.
***************************************************************************)

HistoricalDiscoveryPacketCandidateCausalOriginCarrier(packet) ==
  LET item == packet.item
  IN CASE item.kind \in AsyncReplyRequestKinds -> {}
       [] item.kind \in {"Chunk", "Noise"} -> {}
       [] item.kind = "CertifiedResponse" ->
            {CertifiedResponseCandidate(item).causalOrigin}
       [] item.kind = "CommitCertificateResponse" ->
            {CommitCertificateResponseCandidate(item).causalOrigin}
       [] OTHER -> {DeliveryCandidate(item).causalOrigin}

HistoricalDiscoveryPacketServeIdentityCarrier(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN IF packet.item.kind \in AsyncReplyRequestKinds
     THEN {[owner |-> recipient,
            request |->
              AsyncReplySemanticIdentity(
                packet.item.kind, packet.item.source,
                packet.item.envelope)]}
     ELSE {}

HistoricalDiscoveryPacketCandidateInCausalLineage(packet, candidate) ==
  candidate.causalOrigin
    \in HistoricalDiscoveryPacketCandidateCausalOriginCarrier(packet)

HistoricalDiscoveryPacketServeInCausalLineage(packet, job) ==
  AsyncIoServeJobIdentity(packet.item.envelope.recipient, job)
    \in HistoricalDiscoveryPacketServeIdentityCarrier(packet)

HistoricalDiscoveryPacketCandidateOwners(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN {candidate \in ActiveScheduledCandidates:
        /\ candidate.node = recipient
        /\ candidate.node \in AsyncTimedServiceNodes
        /\ ProtectedCandidateOwned(candidate)
        /\ HistoricalDiscoveryPacketCandidateInCausalLineage(
             packet, candidate)}

HistoricalDiscoveryPacketServeOwners(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN {job \in ActiveIoJobs:
        /\ HistoricalDiscoveryServeJobOwned(recipient, job)
        /\ HistoricalDiscoveryPacketServeInCausalLineage(packet, job)}

THEOREM HistoricalDiscoveryPacketCausalCarriersAreFinite ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => /\ IsFiniteSet(
              HistoricalDiscoveryPacketCandidateCausalOriginCarrier(
                packet))
         /\ IsFiniteSet(
              HistoricalDiscoveryPacketServeIdentityCarrier(packet))
BY FS_Image, FS_Union, FS_Subset, Isa
   DEF HistoricalDiscoveryPacketCandidateCausalOriginCarrier,
       HistoricalDiscoveryPacketServeIdentityCarrier,
       OverdueResponsivePackets, AsyncPacketOwnsClockDeadline,
       AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety,
       TypeInvariant, ModelConfiguration, AsyncConfiguration

THEOREM HistoricalDiscoveryPacketOwnersStayInFrozenCausalCarrier ==
  \A packet \in OverdueResponsivePackets:
    /\ {candidate.causalOrigin:
          candidate \in
            HistoricalDiscoveryPacketCandidateOwners(packet)}
         \subseteq
           HistoricalDiscoveryPacketCandidateCausalOriginCarrier(packet)
       /\ {AsyncIoServeJobIdentity(
               packet.item.envelope.recipient, job):
             job \in HistoricalDiscoveryPacketServeOwners(packet)}
            \subseteq
              HistoricalDiscoveryPacketServeIdentityCarrier(packet)
BY Isa
   DEF HistoricalDiscoveryPacketCandidateOwners,
       HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryPacketCandidateInCausalLineage,
       HistoricalDiscoveryPacketServeInCausalLineage

HistoricalDiscoveryPacketCandidateRanks(packet) ==
  {CandidateServiceRank(candidate):
     candidate \in HistoricalDiscoveryPacketCandidateOwners(packet)}

HistoricalDiscoveryPacketServeRanks(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN {ServeJobRank(recipient, job):
        job \in HistoricalDiscoveryPacketServeOwners(packet)}

HistoricalDiscoveryOwnedRankMinimum(ranks) ==
  CHOOSE rank \in ranks:
    \A other \in ranks:
      <<other, rank>> \notin OwnedServiceRankOrdering

HistoricalDiscoveryPacketCandidateDebtRank(packet) ==
  LET ranks == HistoricalDiscoveryPacketCandidateRanks(packet)
  IN IF ranks = {}
     THEN HistoricalDiscoveryCandidateDebtBottom
     ELSE HistoricalDiscoveryOwnedRankMinimum(ranks)

HistoricalDiscoveryPacketServeDebtRank(packet) ==
  LET ranks == HistoricalDiscoveryPacketServeRanks(packet)
  IN IF ranks = {}
     THEN HistoricalDiscoveryServeDebtBottom
     ELSE HistoricalDiscoveryOwnedRankMinimum(ranks)

\* This is a distinct-logical-owner count.  Reachable `AsyncSpec` states use
\* `AsyncProgressOwnershipInvariant` to rule out collapsed physical copies.
HistoricalDiscoveryPacketCandidateOccurrenceDebtRank(packet) ==
  <<Cardinality(
       HistoricalDiscoveryPacketCandidateOwners(packet)),
    HistoricalDiscoveryPacketCandidateDebtRank(packet)>>

HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet) ==
  <<Cardinality(
       HistoricalDiscoveryPacketServeOwners(packet)),
    HistoricalDiscoveryPacketServeDebtRank(packet)>>

HistoricalDiscoveryPacketCandidateDebtWitness(packet) ==
  CHOOSE candidate
    \in HistoricalDiscoveryPacketCandidateOwners(packet):
      CandidateServiceRank(candidate)
        = HistoricalDiscoveryPacketCandidateDebtRank(packet)

HistoricalDiscoveryPacketServeDebtWitness(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN CHOOSE job \in HistoricalDiscoveryPacketServeOwners(packet):
       ServeJobRank(recipient, job)
         = HistoricalDiscoveryPacketServeDebtRank(packet)

HistoricalDiscoveryPacketCandidateDebtFairAction(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN IF recipient \in AsyncResponsiveAppliedArchiveServers
     THEN PostGstRunHistoricalServer(recipient)
     ELSE IF HistoricalRecoveryTarget(recipient)
          THEN PostGstRunHistoricalRecoveryNode(recipient)
          ELSE PostGstRunNode(recipient)

HistoricalDiscoveryPacketServeDebtFairAction(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN IF HistoricalRecoveryTarget(recipient)
     THEN PostGstServiceHistoricalRecoveryIoWorker(recipient)
     ELSE PostGstServiceIoWorker(recipient)

HistoricalDiscoveryPacketDependencyRank(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN <<OlderDueNonOverdueShadowDebt(packet),
       <<FreshIngressCapacityOwnerDebt(
            packet.item, packet.authenticatedSource),
         <<TimeoutVoteByteOwnerDebt(
              packet.item, packet.authenticatedSource),
           <<TransportCompletionOwnerDebt(
                packet.item, packet.authenticatedSource),
             <<BoundedTransportServiceRank(
                  packet.item.envelope.recipient,
                  packet.authenticatedSource),
               <<ResetAwareIngressReachRank(recipient),
                 <<ReadyRunAuxRank(recipient),
                   <<Stage4CapacityRank(recipient),
                     <<HistoricalDiscoveryPacketCandidateOccurrenceDebtRank(
                          packet),
                       HistoricalDiscoveryPacketServeOccurrenceDebtRank(
                         packet)>>>>>>>>>>>>>>>>>>

HistoricalDiscoverySelectedOverduePacket ==
  CHOOSE packet \in OverdueResponsivePackets: TRUE

HistoricalDiscoverySelectedPacketDependencyRank ==
  HistoricalDiscoveryPacketDependencyRank(
    HistoricalDiscoverySelectedOverduePacket)

HistoricalDiscoveryPacketBlockerRank(clockValue) ==
  HistoricalDiscoveryFixedClockRank(
    clockValue, 1,
    HistoricalDiscoverySelectedPacketDependencyRank)

HistoricalDiscoveryNodeBlockerRank(clockValue) ==
  HistoricalDiscoveryFixedClockRank(
    clockValue, 3,
    HistoricalDiscoveryIngressCounterRank(
      HistoricalDiscoveryNodeBlockerDebt(clockValue)))

HistoricalDiscoveryIoBlockerRank(clockValue) ==
  HistoricalDiscoveryFixedClockRank(
    clockValue, 2,
    HistoricalDiscoveryIngressCounterRank(
      HistoricalDiscoveryActiveIoBlockerDebt(clockValue)))

HistoricalDiscoveryTickRank(clockValue) ==
  HistoricalDiscoveryFixedClockRank(
    clockValue, 0,
    HistoricalDiscoveryIngressCounterRank(0))

HistoricalDiscoveryConcreteBlockerStage(clockValue) ==
  IF OverdueResponsivePackets # {}
  THEN 1
  ELSE IF HistoricalDiscoveryNodeBlockersAt(clockValue) # {}
       THEN 3
       ELSE IF HistoricalDiscoveryActiveIoBlockersAt(clockValue) # {}
            THEN 2
            ELSE 0

HistoricalDiscoveryConcreteDependencyRank(clockValue) ==
  IF OverdueResponsivePackets # {}
  THEN HistoricalDiscoverySelectedPacketDependencyRank
  ELSE IF HistoricalDiscoveryNodeBlockersAt(clockValue) # {}
       THEN HistoricalDiscoveryIngressCounterRank(
              HistoricalDiscoveryNodeBlockerDebt(clockValue))
       ELSE IF HistoricalDiscoveryActiveIoBlockersAt(clockValue) # {}
            THEN HistoricalDiscoveryIngressCounterRank(
                   HistoricalDiscoveryActiveIoBlockerDebt(clockValue))
            ELSE HistoricalDiscoveryIngressCounterRank(0)

HistoricalDiscoveryConcreteFixedClockRank(clockValue) ==
  HistoricalDiscoveryFixedClockRank(
    clockValue,
    HistoricalDiscoveryConcreteBlockerStage(clockValue),
    HistoricalDiscoveryConcreteDependencyRank(clockValue))

THEOREM HistoricalDiscoveryConcreteRankMatchesNamedBranches ==
  \A clockValue \in Nat:
    HistoricalDiscoveryConcreteFixedClockRank(clockValue)
      = IF OverdueResponsivePackets # {}
        THEN HistoricalDiscoveryPacketBlockerRank(clockValue)
        ELSE IF HistoricalDiscoveryNodeBlockersAt(clockValue) # {}
             THEN HistoricalDiscoveryNodeBlockerRank(clockValue)
             ELSE IF
                    HistoricalDiscoveryActiveIoBlockersAt(clockValue)
                      # {}
                  THEN HistoricalDiscoveryIoBlockerRank(clockValue)
                  ELSE HistoricalDiscoveryTickRank(clockValue)
BY Isa
   DEF HistoricalDiscoveryConcreteFixedClockRank,
       HistoricalDiscoveryConcreteBlockerStage,
       HistoricalDiscoveryConcreteDependencyRank,
       HistoricalDiscoveryPacketBlockerRank,
       HistoricalDiscoveryNodeBlockerRank,
       HistoricalDiscoveryIoBlockerRank,
       HistoricalDiscoveryTickRank,
       HistoricalDiscoverySelectedPacketDependencyRank

HistoricalDiscoveryFixedClockBlockedAtRank(
    node, clockValue, rank) ==
  /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
  /\ HistoricalDiscoveryConcreteFixedClockRank(clockValue) = rank

THEOREM HistoricalDiscoveryOwnedRankMinimumFacts ==
  \A ranks \in SUBSET OwnedServiceRankCarrier:
    ranks # {}
      => LET minimum == HistoricalDiscoveryOwnedRankMinimum(ranks)
         IN /\ minimum \in ranks
            /\ \A other \in ranks:
                 <<other, minimum>>
                   \notin OwnedServiceRankOrdering
PROOF
  <1>1. ASSUME NEW ranks \in SUBSET OwnedServiceRankCarrier,
                ranks # {}
         PROVE LET minimum ==
                     HistoricalDiscoveryOwnedRankMinimum(ranks)
               IN /\ minimum \in ranks
                  /\ \A other \in ranks:
                       <<other, minimum>>
                         \notin OwnedServiceRankOrdering
    <2>1. \E minimum \in ranks:
             \A other \in ranks:
               <<other, minimum>>
                 \notin OwnedServiceRankOrdering
      <3>1. ASSUME
               ~(\E minimum \in ranks:
                   \A other \in ranks:
                     <<other, minimum>>
                       \notin OwnedServiceRankOrdering)
             PROVE FALSE
        <4>1. \A rank \in OwnedServiceRankCarrier:
                 (\A lower \in SetLessThan(
                      rank, OwnedServiceRankOrdering,
                      OwnedServiceRankCarrier):
                    lower \notin ranks)
                   => rank \notin ranks
          BY <1>1, <3>1, Isa DEF SetLessThan
        <4>2. \A rank \in OwnedServiceRankCarrier:
                 rank \notin ranks
          BY OwnedServiceRankOrderingWellFounded, <4>1
             DEF IsWellFoundedOn
        <4>3. ranks = {}
          BY <1>1, <4>2, Isa
        <4> QED BY <1>1, <4>3
      <3> QED BY <3>1
    <2> QED BY <2>1, Isa
         DEF HistoricalDiscoveryOwnedRankMinimum
  <1> QED BY <1>1

THEOREM HistoricalDiscoveryOwnedRankMinimumIsUnique ==
  \A ranks \in SUBSET OwnedServiceRankCarrier:
    ranks # {}
      => \A candidate \in ranks:
           (\A other \in ranks:
              <<other, candidate>>
                \notin OwnedServiceRankOrdering)
             => candidate
                  = HistoricalDiscoveryOwnedRankMinimum(ranks)
BY HistoricalDiscoveryOwnedRankMinimumFacts,
   OwnedServiceRankOrderingMatchesLess, SMT
   DEF OwnedServiceRankCarrier, ServiceRankLess

THEOREM HistoricalDiscoveryOwnedRankTrichotomy ==
  \A left, right \in OwnedServiceRankCarrier:
    \/ left = right
    \/ <<left, right>> \in OwnedServiceRankOrdering
    \/ <<right, left>> \in OwnedServiceRankOrdering
BY OwnedServiceRankOrderingMatchesLess, SMT
   DEF OwnedServiceRankCarrier, ServiceRankLess

THEOREM HistoricalDiscoveryOwnedRankMinimumStable ==
  \A beforeRanks, afterRanks \in SUBSET OwnedServiceRankCarrier:
    /\ beforeRanks # {}
    /\ afterRanks # {}
    /\ HistoricalDiscoveryOwnedRankMinimum(beforeRanks)
         \in afterRanks
    /\ \A rank \in afterRanks:
         <<rank,
           HistoricalDiscoveryOwnedRankMinimum(beforeRanks)>>
           \notin OwnedServiceRankOrdering
    => HistoricalDiscoveryOwnedRankMinimum(afterRanks)
         = HistoricalDiscoveryOwnedRankMinimum(beforeRanks)
BY HistoricalDiscoveryOwnedRankMinimumFacts,
   HistoricalDiscoveryOwnedRankMinimumIsUnique, Isa

THEOREM HistoricalDiscoveryLowerOwnedRankForcesMinimumDescent ==
  \A beforeRanks, afterRanks \in SUBSET OwnedServiceRankCarrier:
    /\ beforeRanks # {}
    /\ afterRanks # {}
    /\ \E lower \in afterRanks:
         <<lower,
           HistoricalDiscoveryOwnedRankMinimum(beforeRanks)>>
           \in OwnedServiceRankOrdering
    => <<HistoricalDiscoveryOwnedRankMinimum(afterRanks),
          HistoricalDiscoveryOwnedRankMinimum(beforeRanks)>>
         \in OwnedServiceRankOrdering
BY HistoricalDiscoveryOwnedRankMinimumFacts,
   HistoricalDiscoveryOwnedRankTrichotomy,
   OwnedServiceRankOrderingMatchesLess, SMT
   DEF OwnedServiceRankCarrier, ServiceRankLess

(***************************************************************************
Removing the unique selected minimum is not, by itself, rank descent.

The two-element witness below is deliberately algebraic rather than a
reachability claim.  It prevents a later action proof from treating arbitrary
selected-owner exit as a smaller plain minimum: after removing <<2,1>>, the
remaining minimum is <<2,2>>, which is strictly greater.  Distinct owners
with equal rank values can likewise make one removal leave the set-valued
minimum unchanged.
Any temporal proof must either show that an earlier dependency component
falls on such an exit or refine this tail with occurrence debt.  The
count-first refinement below closes exact no-refill removal without importing
an abstract multiset theorem; replacement and growth remain separate
action-local obligations.
***************************************************************************)

THEOREM HistoricalDiscoveryPlainMinimumRemovalCanIncrease ==
  LET lower == <<2, 1>>
      higher == <<2, 2>>
  IN /\ lower \in OwnedServiceRankCarrier
     /\ higher \in OwnedServiceRankCarrier
     /\ HistoricalDiscoveryOwnedRankMinimum({lower, higher})
          = lower
     /\ HistoricalDiscoveryOwnedRankMinimum({higher})
          = higher
     /\ <<lower, higher>> \in OwnedServiceRankOrdering
BY HistoricalDiscoveryOwnedRankMinimumFacts,
   HistoricalDiscoveryOwnedRankMinimumIsUnique,
   OwnedServiceRankOrderingMatchesLess, SMT
   DEF OwnedServiceRankCarrier, ServiceRankLess

(***************************************************************************
Concrete logical-owner/occurrence refinement and rank-bound audit.

Serve positions have a configuration bound: a live Serve job occupies one
unique nonce-owned position in an I/O queue of length at most
`AsyncIoCapacity`.  Candidate positions do not have a corresponding global
configuration bound.  `ModelConfiguration` allows the explicit
`ViewDomain = Nat` mode used by `AsyncInit`; moreover,
`AsyncCausalTypeInvariant` still types the causal queues without a length cap.
The Busy-deferred Completion and Normal lanes now share the production-derived
`AsyncDeferredNormalCapacity`, while Progress retains its roster-derived cap;
those bounds do not manufacture a global causal-queue bound.  The proved
candidate carrier is therefore exactly
`(2..6) \X Nat`, not a finite interval suitable for a fixed-width histogram.

Every concrete queue and outstanding-work owner set is nevertheless finite
in each strong-typed state.  On the candidate side, set cardinality counts
distinct logical owner values, not raw queue occurrences.  It agrees with
physical scheduler ownership only under the separately proved
`AsyncProgressOwnershipInvariant`; equal candidate values can otherwise
collapse and remain an explicit duplicate-occurrence residual.  Serve
cardinality is exact already under strong typing because fresh nonces make
Serve queue occurrences unique.  Prefixing either count to the existing
minimum records equal-rank logical owners or exact Serve occurrences.  An
exact owner removal with no replacement lowers the first natural component,
regardless of how the remaining minimum changes.  This is a plain
lexicographic product of two already-proved well-founded orders.
***************************************************************************)

THEOREM StrongTypeHasFiniteHistoricalDiscoveryRankOwners ==
  AsyncStrongTypeInvariant
    => /\ IsFiniteSet(ActiveScheduledCandidates)
       /\ IsFiniteSet(ActiveIoJobs)
BY AsyncStrongTypeProjectsAsyncType,
   FS_Interval, FS_Image, FS_Union, FS_Subset, Isa
   DEF ActiveScheduledCandidates, ActiveIoJobs,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncCausalTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoTopologyTypeInvariant,
       AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant,
       AsyncIoWorkContentTypeInvariant,
       AsyncDeferredTypeInvariant,
       AsyncDeferredTopologyTypeInvariant,
       AsyncDeferredContentTypeInvariant,
       AsyncQueueTyped, AsyncIoSequenceTyped,
       AsyncConfiguration, ModelConfiguration, ValidatorIds

THEOREM HistoricalDiscoveryPacketServeRanksHaveConcreteBound ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => LET recipient == packet.item.envelope.recipient
             queue == asyncIoQueues[recipient]
             indices == AsyncIoServeIndices(queue)
         IN /\ HistoricalDiscoveryPacketServeOwners(packet)
                  = {queue[index]: index \in indices}
            /\ \A left, right \in indices:
                 queue[left] = queue[right] => left = right
            /\ Cardinality(
                 HistoricalDiscoveryPacketServeOwners(packet))
                 <= AsyncIoCapacity
            /\ HistoricalDiscoveryPacketServeRanks(packet)
                 \subseteq ({5} \X (1..AsyncIoCapacity))
BY HistoricalDiscoveryOwnersIncludeNonVoterService,
   ServeOccurrenceIndexCharacterization,
   ServeJobIndexMatchesOccurrenceIndex,
   FS_Interval, FS_Image, FS_Subset, FS_CardinalityType, Isa
   DEF HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryPacketServeRanks,
       HistoricalDiscoveryServeJobOwned,
       HistoricalDiscoveryIoOwners,
       ActiveIoJobs, AsyncIoServeIndices,
       AsyncIoQueueDepth, ServeJobRank,
       OverdueResponsivePackets, AsyncPacketOwnsClockDeadline,
       AsyncTimedServiceNodes,
       AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant,
       AsyncIoCapacityTypeInvariant,
       AsyncConfiguration, SequenceSet

THEOREM HistoricalDiscoveryPacketCandidateRanksInCarrier ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => HistoricalDiscoveryPacketCandidateRanks(packet)
           \subseteq OwnedServiceRankCarrier
BY AsyncStrongTypeProjectsAsyncType,
   ScheduledCandidateServiceRankInCarrier, Isa
   DEF HistoricalDiscoveryPacketCandidateRanks,
       HistoricalDiscoveryPacketCandidateOwners,
       ProtectedCandidateOwned

THEOREM HistoricalDiscoveryPacketServeOwnerRankInCarrier ==
  \A packet \in OverdueResponsivePackets:
    \A job \in HistoricalDiscoveryPacketServeOwners(packet):
      AsyncStrongTypeInvariant
        => ServeJobRank(
             packet.item.envelope.recipient, job)
             \in OwnedServiceRankCarrier
BY ServeOccurrenceIndexCharacterization,
   ServeJobIndexMatchesOccurrenceIndex,
   AsyncStrongTypeProjectsAsyncType,
   AsyncArchiveIoServiceNodesAreValidators,
   HistoricalRecoveryTargetsAreValidators, Isa
   DEF HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryServeJobOwned,
       HistoricalDiscoveryIoOwners,
       OverdueResponsivePackets, AsyncPacketOwnsClockDeadline,
       ServeJobRank,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant,
       AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant,
       AsyncIoServeIndices, OwnedServiceRankCarrier

THEOREM HistoricalDiscoveryPacketServeRanksInCarrier ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => HistoricalDiscoveryPacketServeRanks(packet)
           \subseteq OwnedServiceRankCarrier
BY HistoricalDiscoveryPacketServeOwnerRankInCarrier, Isa
   DEF HistoricalDiscoveryPacketServeRanks

THEOREM HistoricalDiscoveryPacketDebtRanksInCarrier ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => /\ HistoricalDiscoveryPacketCandidateDebtRank(packet)
               \in OwnedServiceRankCarrier
         /\ HistoricalDiscoveryPacketServeDebtRank(packet)
               \in OwnedServiceRankCarrier
BY HistoricalDiscoveryPacketCandidateRanksInCarrier,
   HistoricalDiscoveryPacketServeRanksInCarrier,
   HistoricalDiscoveryOwnedRankMinimumFacts, Isa
   DEF HistoricalDiscoveryPacketCandidateDebtRank,
       HistoricalDiscoveryPacketServeDebtRank,
       HistoricalDiscoveryCandidateDebtBottom,
       HistoricalDiscoveryServeDebtBottom,
       OwnedServiceRankCarrier

THEOREM HistoricalDiscoveryPacketOccurrenceDebtRanksInCarrier ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => /\ IsFiniteSet(
              HistoricalDiscoveryPacketCandidateOwners(packet))
         /\ IsFiniteSet(
              HistoricalDiscoveryPacketServeOwners(packet))
         /\ HistoricalDiscoveryPacketCandidateOccurrenceDebtRank(packet)
              \in HistoricalDiscoveryOccurrenceDebtCarrier
         /\ HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)
              \in HistoricalDiscoveryOccurrenceDebtCarrier
BY StrongTypeHasFiniteHistoricalDiscoveryRankOwners,
   HistoricalDiscoveryPacketDebtRanksInCarrier,
   FS_Subset, FS_CardinalityType, Isa
   DEF HistoricalDiscoveryPacketCandidateOwners,
       HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryPacketCandidateOccurrenceDebtRank,
       HistoricalDiscoveryPacketServeOccurrenceDebtRank,
       HistoricalDiscoveryOccurrenceDebtCarrier

THEOREM HistoricalDiscoveryEmptyPacketDebtUsesExactBottoms ==
  \A packet:
    /\ (HistoricalDiscoveryPacketCandidateOwners(packet) = {}
          => HistoricalDiscoveryPacketCandidateDebtRank(packet)
               = HistoricalDiscoveryCandidateDebtBottom)
    /\ (HistoricalDiscoveryPacketServeOwners(packet) = {}
          => HistoricalDiscoveryPacketServeDebtRank(packet)
               = HistoricalDiscoveryServeDebtBottom)
BY Isa
   DEF HistoricalDiscoveryPacketCandidateOwners,
       HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryPacketCandidateRanks,
       HistoricalDiscoveryPacketServeRanks,
       HistoricalDiscoveryPacketCandidateDebtRank,
       HistoricalDiscoveryPacketServeDebtRank

THEOREM HistoricalDiscoveryLiveCandidateDebtHasExactFairOwner ==
  \A packet \in OverdueResponsivePackets:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryPacketCandidateOwners(packet) # {}
    => LET candidate ==
             HistoricalDiscoveryPacketCandidateDebtWitness(packet)
           rank ==
             HistoricalDiscoveryPacketCandidateDebtRank(packet)
           recipient == packet.item.envelope.recipient
       IN /\ candidate
               \in HistoricalDiscoveryPacketCandidateOwners(packet)
          /\ ProtectedCandidateOwned(candidate)
          /\ CandidateServiceRank(candidate) = rank
          /\ rank \in OwnedServiceRankCarrier
          /\ \A other
                  \in HistoricalDiscoveryPacketCandidateRanks(packet):
               <<other, rank>>
                 \notin OwnedServiceRankOrdering
          /\ candidate.node = recipient
          /\ recipient \in Responsive
          /\ recipient \in AsyncTimedServiceNodes
          /\ \/ /\ recipient
                        \in AsyncResponsiveAppliedArchiveServers
                    /\ HistoricalDiscoveryPacketCandidateDebtFairAction(
                         packet) = PostGstRunHistoricalServer(recipient)
             \/ /\ HistoricalRecoveryTarget(recipient)
                    /\ recipient
                         \notin AsyncResponsiveAppliedArchiveServers
                    /\ HistoricalDiscoveryPacketCandidateDebtFairAction(
                         packet) =
                         PostGstRunHistoricalRecoveryNode(recipient)
             \/ /\ recipient \in AsyncCurrentResponsiveVoters
                    /\ ~HistoricalRecoveryTarget(recipient)
                    /\ recipient
                         \notin AsyncResponsiveAppliedArchiveServers
                    /\ HistoricalDiscoveryPacketCandidateDebtFairAction(
                         packet) = PostGstRunNode(recipient)
BY HistoricalDiscoveryPacketCandidateRanksInCarrier,
   HistoricalDiscoveryPacketDebtRanksInCarrier,
   HistoricalDiscoveryOwnedRankMinimumFacts,
   AsyncStrongTypeProjectsAsyncType, Isa
   DEF HistoricalDiscoveryPacketCandidateDebtWitness,
       HistoricalDiscoveryPacketCandidateDebtRank,
       HistoricalDiscoveryPacketCandidateRanks,
       HistoricalDiscoveryPacketCandidateOwners,
       HistoricalDiscoveryPacketCandidateDebtFairAction,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       ProtectedCandidateOwned

THEOREM HistoricalDiscoveryLiveServeDebtHasExactFairOwner ==
  \A packet \in OverdueResponsivePackets:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
    => LET job ==
             HistoricalDiscoveryPacketServeDebtWitness(packet)
           rank ==
             HistoricalDiscoveryPacketServeDebtRank(packet)
           recipient == packet.item.envelope.recipient
       IN /\ job \in HistoricalDiscoveryPacketServeOwners(packet)
          /\ ServeJobRank(recipient, job) = rank
          /\ rank \in OwnedServiceRankCarrier
          /\ \A other
                  \in HistoricalDiscoveryPacketServeRanks(packet):
               <<other, rank>>
                 \notin OwnedServiceRankOrdering
          /\ HistoricalDiscoveryServeJobOwned(recipient, job)
          /\ recipient \in Responsive
          /\ \/ /\ HistoricalRecoveryTarget(recipient)
                /\ HistoricalDiscoveryPacketServeDebtFairAction(packet)
                     =
                     PostGstServiceHistoricalRecoveryIoWorker(
                       recipient)
             \/ /\ recipient \in AsyncArchiveIoServiceNodes
                /\ HistoricalDiscoveryPacketServeDebtFairAction(packet)
                     = PostGstServiceIoWorker(recipient)
BY HistoricalDiscoveryPacketServeRanksInCarrier,
   HistoricalDiscoveryPacketDebtRanksInCarrier,
   HistoricalDiscoveryOwnedRankMinimumFacts,
   AsyncStrongTypeProjectsAsyncType, Isa
   DEF HistoricalDiscoveryPacketServeDebtWitness,
       HistoricalDiscoveryPacketServeDebtRank,
       HistoricalDiscoveryPacketServeRanks,
       HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryPacketServeDebtFairAction,
       HistoricalDiscoveryServeJobOwned,
       HistoricalDiscoveryIoOwners,
       AsyncArchiveIoServiceNodes,
       AsyncCurrentResponsiveVoters,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncHistoricalRecoveryTypeInvariant

THEOREM HistoricalDiscoveryPacketDependencyRankInCarrier ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => LET recipient == packet.item.envelope.recipient
         IN /\ recipient \in ValidatorIds
            /\ HistoricalDiscoveryPacketDependencyRank(packet)
                 \in HistoricalDiscoveryPacketDependencyCarrier
BY AsyncStrongTypeProjectsAsyncType,
   StrongTypeHasFiniteOlderNonOverdueShadows,
   IngressGateOwnerDebtsAreFiniteNaturals,
   BoundedTransportServiceRankIsNatural,
   ResetAwareIngressReachRankIsNatural,
   ReadyRunAuxRankInCarrier,
   Stage4CapacityRankInCarrier,
   HistoricalDiscoveryPacketOccurrenceDebtRanksInCarrier, Isa
   DEF HistoricalDiscoveryPacketDependencyRank,
       HistoricalDiscoveryPacketDependencyCarrier,
       HistoricalDiscoveryCapacityTailCarrier,
       HistoricalDiscoveryTimeoutTailCarrier,
       HistoricalDiscoveryCompletionTailCarrier,
       HistoricalDiscoveryTransportTailCarrier,
       HistoricalDiscoveryResetTailCarrier,
       HistoricalDiscoveryReadyTailCarrier,
       HistoricalDiscoveryStage4TailCarrier,
       HistoricalDiscoveryCandidateServeTailCarrier,
       OverdueResponsivePackets, AsyncPacketOwnsClockDeadline,
       AsyncStrongTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncPacketContentTypeInvariant,
       AsyncPacketTyped, AsyncItemTyped

THEOREM HistoricalDiscoveryConcreteBlockerCoordinatesInCarrier ==
  \A node \in Responsive, clockValue \in Nat:
    HistoricalDiscoveryFixedClockPending(node, clockValue)
      => /\ HistoricalDiscoveryConcreteBlockerStage(clockValue)
              \in HistoricalDiscoveryBlockerStageCarrier
         /\ HistoricalDiscoveryConcreteDependencyRank(clockValue)
              \in HistoricalDiscoveryPacketDependencyCarrier
BY StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   HistoricalDiscoveryPacketDependencyRankInCarrier,
   HistoricalDiscoveryIngressCounterRankInCarrier,
   FS_CardinalityType, Isa
   DEF HistoricalDiscoveryFixedClockPending,
       HistoricalDiscoveryConcreteBlockerStage,
       HistoricalDiscoveryConcreteDependencyRank,
       HistoricalDiscoverySelectedOverduePacket,
       HistoricalDiscoverySelectedPacketDependencyRank,
       HistoricalDiscoveryNodeBlockerDebt,
       HistoricalDiscoveryActiveIoBlockerDebt,
       HistoricalDiscoveryBlockerStageCarrier

THEOREM HistoricalDiscoveryConcreteFixedClockRankInCarrier ==
  \A node \in Responsive, clockValue \in Nat:
    HistoricalDiscoveryFixedClockPending(node, clockValue)
      => HistoricalDiscoveryConcreteFixedClockRank(clockValue)
           \in HistoricalDiscoveryFixedClockBlockerCarrier
BY HistoricalDiscoveryConcreteBlockerCoordinatesInCarrier,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   HistoricalDiscoveryFixedClockRankShapeInCarrier, Isa
   DEF HistoricalDiscoveryFixedClockPending,
       HistoricalDiscoveryConcreteFixedClockRank,
       HistoricalDiscoveryLatentOwnerDebt,
       HistoricalDiscoveryDuePacketDebt,
       HistoricalDiscoveryDormantIoDebt

THEOREM HistoricalDiscoveryFixedClockPendingHasFiniteRank ==
  \A node \in Responsive, clockValue \in Nat:
    HistoricalDiscoveryFixedClockPending(node, clockValue)
      => \E rank \in HistoricalDiscoveryFixedClockBlockerCarrier:
           HistoricalDiscoveryFixedClockBlockedAtRank(
             node, clockValue, rank)
BY HistoricalDiscoveryConcreteFixedClockRankInCarrier
   DEF HistoricalDiscoveryFixedClockBlockedAtRank

THEOREM HistoricalDiscoveryConcreteLexCertificateStrictlyDescends ==
  \A node \in Responsive, clockValue \in Nat:
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)'
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
   DEF HistoricalDiscoveryFixedClockPending,
       HistoricalDiscoveryConcreteFixedClockRank,
       HistoricalDiscoveryLatentOwnerDebt,
       HistoricalDiscoveryDuePacketDebt,
       HistoricalDiscoveryDormantIoDebt

(***************************************************************************
Exact remaining action-local proof debt.

The finite carrier and concrete membership theorem above do not themselves
establish a temporal descent.  A deductive proof of
`HistoricalCommitCertificateDiscoveryClockProgressProperty` still requires
all of the following transition facts, proved from the concrete actions and
their individually quantified fairness clauses:

  * timed-owner non-refill: while the fixed target remains live and undecided,
    opening a new timed owner lowers the latent-owner component, and retirement
    of any other timed owner cannot raise that component without an earlier
    component decreasing;
  * due-packet non-refill: publication at a fixed clock has a strictly future
    deadline, and admission, coalescing, or policy drop removes the selected
    due occurrence or strictly lowers its ingress dependency;
  * stale-empty-I/O handoff: enqueueing into a due empty I/O queue spends the
    dormant-gate component before exposing an active I/O blocker;
  * every capacity, timeout-byte, shared-completion, transport-position,
    reset-aware runner, auxiliary, and Stage-4 owner either exits or strictly
    lowers `IngressBoundaryDependencyOrdering`;
  * historical candidate and historical Serve owners outside the current
    voting roster exit or strictly lower their exact service ranks under
    `PostGstRunHistoricalRecoveryNode`,
    `PostGstRunHistoricalServer`, and
    `PostGstServiceHistoricalRecoveryIoWorker`; and
  * after every earlier blocker retires, the continuously enabled
    `AsyncTick` occurs by its own weak-fairness clause.

The height-reset dependency module currently proves only the non-overdue
lane-shadow admission and idle Runtime-reset edges of that ingress list.  Its
temporal escape through `ResponsiveNodesDecide` is scoped to the frozen
current voting roster and therefore cannot close discovery for an
out-of-roster historical target.  No theorem or assumption in this module
claims the missing action-local closure.
***************************************************************************)

=============================================================================
