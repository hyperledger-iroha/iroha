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

HistoricalDiscoveryCandidateServeBottom ==
  <<<<2, 0>>, <<5, 0>>>>

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
Exact packet dependency choice.

The base choice records the packet-local gates and runner ranks with canonical
candidate/Serve tails.  When an exact historical candidate or Serve owner is
present, the choice set also contains its live service rank.  Selection is
restricted to the proved ingress carrier; the canonical base makes that
intersection nonempty for every overdue packet in a strong-typed state.
***************************************************************************)

HistoricalDiscoveryServeJobOwned(node, job) ==
  /\ node \in HistoricalDiscoveryIoOwners
  /\ job \in AsyncServeJobSet
  /\ job \in SequenceSet(asyncIoQueues[node])

HistoricalDiscoveryPacketDependencyRanks(packet) ==
  LET recipient == packet.item.envelope.recipient
      base ==
        IngressBoundaryDependencyRank(
          packet, recipient, <<2, 0>>, <<5, 0>>)
  IN {base}
       \cup {
         CandidateIngressDependencyRank(packet, candidate):
           candidate
             \in {owned \in ActiveScheduledCandidates:
                   /\ owned.node = recipient
                   /\ HistoricalProtectedCandidateOwned(owned)}}
       \cup {
         ServeIngressDependencyRank(packet, recipient, job):
           job
             \in {owned \in ActiveIoJobs:
                   HistoricalDiscoveryServeJobOwned(
                     recipient, owned)}}

HistoricalDiscoveryPacketCarrierRanks(packet) ==
  HistoricalDiscoveryPacketDependencyRanks(packet)
    \cap IngressBoundaryDependencyCarrier

HistoricalDiscoverySelectedOverduePacket ==
  CHOOSE packet \in OverdueResponsivePackets: TRUE

HistoricalDiscoverySelectedPacketDependencyRank ==
  CHOOSE rank \in
    HistoricalDiscoveryPacketCarrierRanks(
      HistoricalDiscoverySelectedOverduePacket):
      TRUE

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

THEOREM HistoricalDiscoveryBasePacketDependencyRankInCarrier ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => LET recipient == packet.item.envelope.recipient
             base ==
               IngressBoundaryDependencyRank(
                 packet, recipient, <<2, 0>>, <<5, 0>>)
         IN /\ recipient \in ValidatorIds
            /\ base \in IngressBoundaryDependencyCarrier
            /\ base \in HistoricalDiscoveryPacketDependencyRanks(packet)
            /\ base \in HistoricalDiscoveryPacketCarrierRanks(packet)
BY IngressBoundaryDependencyRankInCarrier, Isa
   DEF OverdueResponsivePackets,
       HistoricalDiscoveryPacketDependencyRanks,
       HistoricalDiscoveryPacketCarrierRanks,
       AsyncStrongTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncPacketContentTypeInvariant,
       AsyncPacketTyped, AsyncItemTyped,
       OwnedServiceRankCarrier

THEOREM HistoricalDiscoveryCandidatePacketDependencyRankInCarrier ==
  \A packet \in OverdueResponsivePackets,
     candidate \in ActiveScheduledCandidates:
    /\ AsyncStrongTypeInvariant
    /\ candidate.node = packet.item.envelope.recipient
    /\ HistoricalProtectedCandidateOwned(candidate)
    => /\ CandidateIngressDependencyRank(packet, candidate)
             \in IngressBoundaryDependencyCarrier
       /\ CandidateIngressDependencyRank(packet, candidate)
             \in HistoricalDiscoveryPacketDependencyRanks(packet)
       /\ CandidateIngressDependencyRank(packet, candidate)
             \in HistoricalDiscoveryPacketCarrierRanks(packet)
BY AsyncStrongTypeProjectsAsyncType,
   ScheduledCandidateServiceRankInCarrier,
   IngressBoundaryDependencyRankInCarrier, Isa
   DEF HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned,
       CandidateIngressDependencyRank,
       HistoricalDiscoveryPacketDependencyRanks,
       HistoricalDiscoveryPacketCarrierRanks,
       OverdueResponsivePackets,
       OwnedServiceRankCarrier

THEOREM HistoricalDiscoveryServePacketDependencyRankInCarrier ==
  \A packet \in OverdueResponsivePackets,
     job \in ActiveIoJobs:
    LET recipient == packet.item.envelope.recipient
    IN /\ AsyncStrongTypeInvariant
       /\ HistoricalDiscoveryServeJobOwned(recipient, job)
       => /\ ServeIngressDependencyRank(packet, recipient, job)
                \in IngressBoundaryDependencyCarrier
          /\ ServeIngressDependencyRank(packet, recipient, job)
                \in HistoricalDiscoveryPacketDependencyRanks(packet)
          /\ ServeIngressDependencyRank(packet, recipient, job)
                \in HistoricalDiscoveryPacketCarrierRanks(packet)
BY ServeOccurrenceIndexCharacterization,
   ServeJobIndexMatchesOccurrenceIndex,
   AsyncStrongTypeProjectsAsyncType,
   AsyncArchiveIoServiceNodesAreValidators,
   HistoricalRecoveryTargetsAreValidators,
   IngressBoundaryDependencyRankInCarrier, Isa
   DEF HistoricalDiscoveryServeJobOwned,
       HistoricalDiscoveryIoOwners,
       HistoricalDiscoveryPacketDependencyRanks,
       HistoricalDiscoveryPacketCarrierRanks,
       OverdueResponsivePackets,
       ServeIngressDependencyRank, ServeJobRank,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant,
       AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant,
       AsyncIoServeIndices, OwnedServiceRankCarrier

THEOREM HistoricalDiscoveryConcreteFixedClockRankInCarrier ==
  \A node \in Responsive, clockValue \in Nat:
    HistoricalDiscoveryFixedClockPending(node, clockValue)
      => HistoricalDiscoveryConcreteFixedClockRank(clockValue)
           \in HistoricalDiscoveryFixedClockBlockerCarrier
BY StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   HistoricalDiscoveryBasePacketDependencyRankInCarrier,
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
       HistoricalDiscoveryPacketDependencyRanks,
       HistoricalDiscoveryPacketCarrierRanks,
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
