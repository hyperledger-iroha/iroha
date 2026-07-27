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

The final component is the already-proved
`IngressBoundaryDependencyOrdering`: lane-head shadows, ingress capacity,
timeout-byte and shared-completion owners, transport service position,
reset-aware runner reach, auxiliary/capacity work, and exact candidate/Serve
ownership.

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

HistoricalDiscoveryCandidateServeBottom ==
  <<HistoricalDiscoveryCandidateDebtBottom,
    HistoricalDiscoveryServeDebtBottom>>

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

THEOREM HistoricalDiscoveryIngressCounterRankInCarrier ==
  \A counter \in Nat:
    HistoricalDiscoveryIngressCounterRank(counter)
      \in IngressBoundaryDependencyCarrier
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
       HistoricalDiscoveryCandidateDebtBottom,
       HistoricalDiscoveryServeDebtBottom,
       HistoricalDiscoveryStage4Bottom,
       HistoricalDiscoveryReadyAuxBottom,
       HistoricalDiscoveryReadyDeferredBottom,
       HistoricalDiscoveryReadyTimeoutBottom,
       HistoricalDiscoveryReadyInnerBottom,
       IngressBoundaryDependencyCarrier,
       IngressCapacityTailCarrier,
       IngressTimeoutTailCarrier,
       IngressCompletionTailCarrier,
       IngressTransportTailCarrier,
       IngressResetTailCarrier,
       IngressReadyTailCarrier,
       IngressStage4TailCarrier,
       IngressCandidateServeTailCarrier,
       Stage4CapacityCarrier, ReadyRunAuxCarrier,
       ReadyRunDeferredCarrier, ReadyRunTimeoutCarrier,
       ReadyRunInnerCarrier, OwnedServiceRankCarrier

(***************************************************************************
Fixed-clock blocker order.

The prefix order is:

  latent timed owner, due packet occurrence, dormant stale I/O gate,
  active blocker class, exact ingress dependency.

Class 3 is an overdue packet, class 2 a due runner, class 1 a due active I/O
worker, and class 0 the tick.  A higher class is earlier work, so the ordinary
natural less-than relation places a retired class below an earlier class when
the preceding components are equal.
***************************************************************************)

HistoricalDiscoveryBlockerStageCarrier == 0..3

HistoricalDiscoveryBlockerStageTailCarrier ==
  HistoricalDiscoveryBlockerStageCarrier
    \X IngressBoundaryDependencyCarrier

HistoricalDiscoveryBlockerStageTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), IngressBoundaryDependencyOrdering,
    HistoricalDiscoveryBlockerStageCarrier,
    IngressBoundaryDependencyCarrier)

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

THEOREM HistoricalDiscoveryFixedClockBlockerOrderingIsWellFounded ==
  IsWellFoundedOn(
    HistoricalDiscoveryFixedClockBlockerOrdering,
    HistoricalDiscoveryFixedClockBlockerCarrier)
PROOF
  <1>1. IsWellFoundedOn(
           HistoricalDiscoveryBlockerStageTailOrdering,
           HistoricalDiscoveryBlockerStageTailCarrier)
    BY NatLessThanWellFounded, IsWellFoundedOnSubset,
       IngressBoundaryDependencyOrderingIsWellFounded,
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

HistoricalDiscoveryPacketCandidateOwners(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN {candidate \in ActiveScheduledCandidates:
        /\ candidate.node = recipient
        /\ HistoricalProtectedCandidateOwned(candidate)}

HistoricalDiscoveryPacketServeOwners(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN {job \in ActiveIoJobs:
        HistoricalDiscoveryServeJobOwned(recipient, job)}

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
  PostGstRunHistoricalRecoveryNode(
    packet.item.envelope.recipient)

HistoricalDiscoveryPacketServeDebtFairAction(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN IF HistoricalRecoveryTarget(recipient)
     THEN PostGstServiceHistoricalRecoveryIoWorker(recipient)
     ELSE PostGstServiceIoWorker(recipient)

HistoricalDiscoveryPacketDependencyRank(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN IngressBoundaryDependencyRank(
       packet, recipient,
       HistoricalDiscoveryPacketCandidateDebtRank(packet),
       HistoricalDiscoveryPacketServeDebtRank(packet))

HistoricalDiscoverySelectedOverduePacket ==
  CHOOSE packet \in OverdueResponsivePackets: TRUE

HistoricalDiscoverySelectedPacketDependencyRank ==
  HistoricalDiscoveryPacketDependencyRank(
    HistoricalDiscoverySelectedOverduePacket)

HistoricalDiscoveryPacketBlockerRank(clockValue) ==
  HistoricalDiscoveryFixedClockRank(
    clockValue, 3,
    HistoricalDiscoverySelectedPacketDependencyRank)

HistoricalDiscoveryNodeBlockerRank(clockValue) ==
  HistoricalDiscoveryFixedClockRank(
    clockValue, 2,
    HistoricalDiscoveryIngressCounterRank(
      HistoricalDiscoveryNodeBlockerDebt(clockValue)))

HistoricalDiscoveryIoBlockerRank(clockValue) ==
  HistoricalDiscoveryFixedClockRank(
    clockValue, 1,
    HistoricalDiscoveryIngressCounterRank(
      HistoricalDiscoveryActiveIoBlockerDebt(clockValue)))

HistoricalDiscoveryTickRank(clockValue) ==
  HistoricalDiscoveryFixedClockRank(
    clockValue, 0,
    HistoricalDiscoveryIngressCounterRank(0))

HistoricalDiscoveryConcreteFixedClockRank(clockValue) ==
  IF OverdueResponsivePackets # {}
  THEN HistoricalDiscoveryPacketBlockerRank(clockValue)
  ELSE IF HistoricalDiscoveryNodeBlockersAt(clockValue) # {}
       THEN HistoricalDiscoveryNodeBlockerRank(clockValue)
       ELSE IF HistoricalDiscoveryActiveIoBlockersAt(clockValue) # {}
            THEN HistoricalDiscoveryIoBlockerRank(clockValue)
            ELSE HistoricalDiscoveryTickRank(clockValue)

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
`AsyncCausalTypeInvariant` types the causal queues without a length cap,
`AsyncDeferredContentTypeInvariant` does not cap the deferred Completion
queue, and `AsyncCompletionLoad` includes that unbounded-but-finite deferred
count.  The proved candidate carrier is therefore exactly
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

HistoricalDiscoveryOccurrenceDebtCarrier ==
  Nat \X OwnedServiceRankCarrier

HistoricalDiscoveryOccurrenceDebtOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OwnedServiceRankOrdering,
    Nat, OwnedServiceRankCarrier)

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

THEOREM HistoricalDiscoveryOccurrenceDebtOrderingIsWellFounded ==
  IsWellFoundedOn(
    HistoricalDiscoveryOccurrenceDebtOrdering,
    HistoricalDiscoveryOccurrenceDebtCarrier)
BY NatLessThanWellFounded,
   OwnedServiceRankOrderingWellFounded,
   WFLexPairOrdering
   DEF HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier

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
       OverdueResponsivePackets,
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
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned

THEOREM HistoricalDiscoveryPacketServeOwnerRankInCarrier ==
  \A packet \in OverdueResponsivePackets,
     job \in HistoricalDiscoveryPacketServeOwners(packet):
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
       OverdueResponsivePackets,
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
          /\ HistoricalProtectedCandidateOwned(candidate)
          /\ CandidateServiceRank(candidate) = rank
          /\ rank \in OwnedServiceRankCarrier
          /\ \A other
                  \in HistoricalDiscoveryPacketCandidateRanks(packet):
               <<other, rank>>
                 \notin OwnedServiceRankOrdering
          /\ candidate.node = recipient
          /\ recipient \in Responsive
          /\ HistoricalRecoveryTarget(recipient)
          /\ HistoricalDiscoveryPacketCandidateDebtFairAction(packet)
               = PostGstRunHistoricalRecoveryNode(recipient)
BY HistoricalDiscoveryPacketCandidateRanksInCarrier,
   HistoricalDiscoveryPacketDebtRanksInCarrier,
   HistoricalDiscoveryOwnedRankMinimumFacts, Isa
   DEF HistoricalDiscoveryPacketCandidateDebtWitness,
       HistoricalDiscoveryPacketCandidateDebtRank,
       HistoricalDiscoveryPacketCandidateRanks,
       HistoricalDiscoveryPacketCandidateOwners,
       HistoricalDiscoveryPacketCandidateDebtFairAction,
       HistoricalProtectedCandidateOwned

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
                 \in IngressBoundaryDependencyCarrier
BY HistoricalDiscoveryPacketDebtRanksInCarrier,
   IngressBoundaryDependencyRankInCarrier, Isa
   DEF HistoricalDiscoveryPacketDependencyRank,
       OverdueResponsivePackets,
       AsyncStrongTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncPacketContentTypeInvariant,
       AsyncPacketTyped, AsyncItemTyped

THEOREM HistoricalDiscoveryConcreteFixedClockRankInCarrier ==
  \A node \in Responsive, clockValue \in Nat:
    HistoricalDiscoveryFixedClockPending(node, clockValue)
      => HistoricalDiscoveryConcreteFixedClockRank(clockValue)
           \in HistoricalDiscoveryFixedClockBlockerCarrier
BY StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   HistoricalDiscoveryPacketDependencyRankInCarrier,
   HistoricalDiscoveryIngressCounterRankInCarrier,
   FS_CardinalityType, Isa
   DEF HistoricalDiscoveryFixedClockPending,
       HistoricalDiscoveryConcreteFixedClockRank,
       HistoricalDiscoveryPacketBlockerRank,
       HistoricalDiscoveryNodeBlockerRank,
       HistoricalDiscoveryIoBlockerRank,
       HistoricalDiscoveryTickRank,
       HistoricalDiscoveryFixedClockRank,
       HistoricalDiscoverySelectedOverduePacket,
       HistoricalDiscoverySelectedPacketDependencyRank,
       HistoricalDiscoveryPacketDependencyRank,
       HistoricalDiscoveryFixedClockBlockerCarrier,
       HistoricalDiscoveryDuePacketTailCarrier,
       HistoricalDiscoveryDormantTailCarrier,
       HistoricalDiscoveryBlockerStageTailCarrier,
       HistoricalDiscoveryBlockerStageCarrier,
       HistoricalDiscoveryLatentOwnerDebt,
       HistoricalDiscoveryDuePacketDebt,
       HistoricalDiscoveryDormantIoDebt,
       HistoricalDiscoveryNodeBlockerDebt,
       HistoricalDiscoveryActiveIoBlockerDebt

THEOREM HistoricalDiscoveryFixedClockPendingHasFiniteRank ==
  \A node \in Responsive, clockValue \in Nat:
    HistoricalDiscoveryFixedClockPending(node, clockValue)
      => \E rank \in HistoricalDiscoveryFixedClockBlockerCarrier:
           HistoricalDiscoveryFixedClockBlockedAtRank(
             node, clockValue, rank)
BY HistoricalDiscoveryConcreteFixedClockRankInCarrier
   DEF HistoricalDiscoveryFixedClockBlockedAtRank

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
