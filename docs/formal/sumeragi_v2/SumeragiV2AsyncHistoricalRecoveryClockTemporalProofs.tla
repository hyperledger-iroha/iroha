---- MODULE SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryClockRankActionProofs

(***************************************************************************
Temporal composition for the historical discovery clock.

The concrete action modules below this leaf prove the fixed-clock rank,
well-foundedness, publication/no-refill frames, exact ingress removal, latent
owner entry, dormant-I/O handoff, and due runner/I/O service edges.  This
module supplies the temporal composition shape without hiding the remaining
candidate/Serve producer episode.

There are deliberately three inputs:

  * non-packet service closes the runner, I/O, and final Tick branches;
  * packet service reaches strict rank descent or exposes a genuinely new
    candidate/Serve service identity relative to the source snapshot; and
  * the finite identity-budget bridge closes that non-descent episode.

The third input remains explicit until the candidate tombstone bridge proves
that the source snapshot is contained in one finite frozen semantic universe,
every new identity consumes its complement budget, and a serviced identity
cannot be resurrected before view/Decision/height exit.  The global
`AsyncCandidateSet` is deliberately not such a universe: production proofs
run with `ViewDomain = Nat`.  Replenishment is never a
progress goal here.  No fairness of an existential action union, current
voter Decision convergence, rotating-leader convergence, application
liveness, or clock shortcut is assumed.
***************************************************************************)

HistoricalDiscoveryFixedClockProgressExit(node, clockValue) ==
  \/ NodeHasDecision(node)
  \/ /\ HistoricalRecoveryTarget(node)
        /\ \/ ActiveCommitCertificateRequests(node) # {}
           \/ asyncNow > clockValue

HistoricalDiscoveryFixedClockStrictRankGoal(
    node, clockValue, sourceRank) ==
  \/ HistoricalDiscoveryFixedClockProgressExit(node, clockValue)
  \/ \E lowerRank \in
       SetLessThan(
         sourceRank,
         HistoricalDiscoveryFixedClockBlockerOrdering,
         HistoricalDiscoveryFixedClockBlockerCarrier):
       HistoricalDiscoveryFixedClockBlockedAtRank(
         node, clockValue, lowerRank)

THEOREM HistoricalDiscoveryProgressExitIsConcreteExit ==
  \A node, clockValue:
    HistoricalDiscoveryFixedClockProgressExit(node, clockValue)
      => HistoricalDiscoveryFixedClockExit(node, clockValue)
BY Isa
   DEF HistoricalDiscoveryFixedClockProgressExit,
       HistoricalDiscoveryFixedClockExit

(***************************************************************************
Exact logical identities exposed by the producer episode.

Candidate identities use the durable route-neutral key from AsyncNetwork;
consumer view/generation are absent by construction.  Serve identities use
the existing exact logical request key, not the fresh physical job nonce.
The outer tag keeps the two lifecycle namespaces disjoint.
***************************************************************************)

HistoricalDiscoveryCandidateOwnerIdentity(candidate) ==
  [ownerKind |-> "Candidate",
   identity |-> AsyncCandidateServiceIdentity(candidate)]

HistoricalDiscoveryServeOwnerIdentity(node, job) ==
  [ownerKind |-> "Serve",
   identity |-> AsyncIoServeJobIdentity(node, job)]

HistoricalDiscoveryPacketCandidateIdentityCarrier(packet) ==
  {[ownerKind |-> "Candidate", identity |-> identity]:
     identity \in
       HistoricalDiscoveryPacketCandidateServiceIdentityCarrier(packet)}

HistoricalDiscoveryPacketServeIdentityCarrier(packet) ==
  {[ownerKind |-> "Serve", identity |-> identity]:
     identity \in HistoricalDiscoveryPacketServeIdentityCarrier(packet)}

HistoricalDiscoveryPacketProducerIdentityCarrier(packet) ==
  HistoricalDiscoveryPacketCandidateIdentityCarrier(packet)
    \cup HistoricalDiscoveryPacketServeIdentityCarrier(packet)

HistoricalDiscoveryPacketCandidateIdentitySet(packet) ==
  {HistoricalDiscoveryCandidateOwnerIdentity(candidate):
     candidate \in HistoricalDiscoveryPacketCandidateOwners(packet)}

HistoricalDiscoveryPacketServeIdentitySet(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN {HistoricalDiscoveryServeOwnerIdentity(recipient, job):
        job \in HistoricalDiscoveryPacketServeOwners(packet)}

HistoricalDiscoveryPacketProducerIdentitySet(packet) ==
  HistoricalDiscoveryPacketCandidateIdentitySet(packet)
    \cup HistoricalDiscoveryPacketServeIdentitySet(packet)

HistoricalDiscoveryPacketCandidateCoveredIdentitySet(packet) ==
  HistoricalDiscoveryPacketCandidateIdentitySet(packet)
    \cup
  {[ownerKind |-> "Candidate", identity |-> record.identity]:
     record \in AsyncCandidateServiceTombstones,
     record.identity
       \in HistoricalDiscoveryPacketCandidateServiceIdentityCarrier(packet)}

HistoricalDiscoveryPacketServeCoveredIdentitySet(packet) ==
  LET carrier == HistoricalDiscoveryPacketServeIdentityCarrier(packet)
  IN HistoricalDiscoveryPacketServeIdentitySet(packet)
       \cup
     {[ownerKind |-> "Serve", identity |-> reservation.identity]:
        reservation \in asyncServeReservations,
        reservation.identity \in carrier}
       \cup
     {[ownerKind |-> "Serve", identity |-> tombstone.identity]:
        tombstone \in asyncServeTombstones,
        tombstone.identity \in carrier}
       \cup
     UNION {
       {[ownerKind |-> "Serve", identity |-> tombstone.identity]:
          tombstone \in reservation.rollbackTombstones,
          tombstone.identity \in carrier}:
       reservation \in asyncServeReservations}

HistoricalDiscoveryPacketProducerCoveredIdentitySet(packet) ==
  HistoricalDiscoveryPacketCandidateCoveredIdentitySet(packet)
    \cup HistoricalDiscoveryPacketServeCoveredIdentitySet(packet)

THEOREM HistoricalDiscoveryPacketProducerIdentityCarrierIsFinite ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => IsFiniteSet(
           HistoricalDiscoveryPacketProducerIdentityCarrier(packet))
BY HistoricalDiscoveryPacketCausalCarriersAreFinite,
   FS_Image, FS_Union, Isa
   DEF HistoricalDiscoveryPacketProducerIdentityCarrier,
       HistoricalDiscoveryPacketCandidateIdentityCarrier,
       HistoricalDiscoveryPacketServeIdentityCarrier

THEOREM HistoricalDiscoveryPacketProducerCoverageStaysInFrozenCarrier ==
  \A packet \in OverdueResponsivePackets:
    /\ HistoricalDiscoveryPacketProducerIdentitySet(packet)
         \subseteq
           HistoricalDiscoveryPacketProducerIdentityCarrier(packet)
       /\ HistoricalDiscoveryPacketProducerCoveredIdentitySet(packet)
         \subseteq
           HistoricalDiscoveryPacketProducerIdentityCarrier(packet)
BY HistoricalDiscoveryPacketOwnersStayInFrozenCausalCarrier, Isa
   DEF HistoricalDiscoveryPacketProducerIdentitySet,
       HistoricalDiscoveryPacketCandidateIdentitySet,
       HistoricalDiscoveryPacketServeIdentitySet,
       HistoricalDiscoveryPacketProducerCoveredIdentitySet,
       HistoricalDiscoveryPacketCandidateCoveredIdentitySet,
       HistoricalDiscoveryPacketServeCoveredIdentitySet,
       HistoricalDiscoveryPacketProducerIdentityCarrier,
       HistoricalDiscoveryPacketCandidateIdentityCarrier,
       HistoricalDiscoveryPacketServeIdentityCarrier

THEOREM HistoricalDiscoveryPacketProducerIdentitySetIsFinite ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => IsFiniteSet(
           HistoricalDiscoveryPacketProducerIdentitySet(packet))
BY StrongTypeHasFiniteHistoricalDiscoveryRankOwners,
   FS_Subset, FS_Image, FS_Union, Isa
   DEF HistoricalDiscoveryPacketProducerIdentitySet,
       HistoricalDiscoveryPacketCandidateIdentitySet,
       HistoricalDiscoveryPacketServeIdentitySet,
       HistoricalDiscoveryPacketCandidateOwners,
       HistoricalDiscoveryPacketServeOwners

(***************************************************************************
Only the final candidate/Serve pair may vary inside one producer episode.
The helper below projects every earlier coordinate of the concrete packet
dependency product.  It includes the outer fixed-clock prefix, blocker class,
lane-shadow, capacity, timeout-byte, completion, transport, reset-aware
runner, Ready/auxiliary, and Stage-4 coordinates.
***************************************************************************)

HistoricalDiscoveryDependencyProducerPrefix(dependencyRank) ==
  <<dependencyRank[1],
    dependencyRank[2][1],
    dependencyRank[2][2][1],
    dependencyRank[2][2][2][1],
    dependencyRank[2][2][2][2][1],
    dependencyRank[2][2][2][2][2][1],
    dependencyRank[2][2][2][2][2][2][1],
    dependencyRank[2][2][2][2][2][2][2][1]>>

HistoricalDiscoveryFixedClockProducerPrefix(rank) ==
  <<rank[1],
    rank[2][1],
    rank[2][2][1],
    rank[2][2][2][1],
    HistoricalDiscoveryDependencyProducerPrefix(
      rank[2][2][2][2])>>

HistoricalDiscoveryCandidateServeEpisodeSource(
    node, clockValue, sourceRank, packet, known) ==
  /\ HistoricalDiscoveryFixedClockBlockedAtRank(
       node, clockValue, sourceRank)
  /\ OverdueResponsivePackets # {}
  /\ packet = HistoricalDiscoverySelectedOverduePacket
  /\ known =
       HistoricalDiscoveryPacketProducerCoveredIdentitySet(packet)
  /\ known
       \subseteq
         HistoricalDiscoveryPacketProducerIdentityCarrier(packet)

HistoricalDiscoveryCandidateServeEpisodeResidual(
    node, clockValue, sourceRank, packet, known) ==
  LET currentRank ==
        HistoricalDiscoveryConcreteFixedClockRank(clockValue)
  IN /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
     /\ sourceRank
          \in HistoricalDiscoveryFixedClockBlockerCarrier
     /\ currentRank
          \in HistoricalDiscoveryFixedClockBlockerCarrier
     /\ packet \in OverdueResponsivePackets
     /\ packet = HistoricalDiscoverySelectedOverduePacket
     /\ IsFiniteSet(known)
     /\ HistoricalDiscoveryFixedClockProducerPrefix(currentRank)
          = HistoricalDiscoveryFixedClockProducerPrefix(sourceRank)
     /\ ~HistoricalDiscoveryFixedClockStrictRankGoal(
          node, clockValue, sourceRank)
     /\ HistoricalDiscoveryPacketProducerCoveredIdentitySet(packet)
          \ known # {}

THEOREM HistoricalDiscoveryCandidateServeEpisodeSourceHasFiniteKnownSet ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known:
      HistoricalDiscoveryCandidateServeEpisodeSource(
        node, clockValue, sourceRank, packet, known)
        => /\ packet \in OverdueResponsivePackets
           /\ IsFiniteSet(known)
BY HistoricalDiscoveryPacketProducerIdentitySetIsFinite, Isa
   DEF HistoricalDiscoveryCandidateServeEpisodeSource,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       HistoricalDiscoveryFixedClockPending

(***************************************************************************
Temporal input surface.

The packet-service property is source-snapshot indexed, so its episode target
cannot hold trivially in the source state: `known` is exactly the then-live
identity set, whereas the residual requires a live identity outside `known`.
The candidate/Serve budget property is the one intentionally explicit input.
It must be proved from a finite frozen semantic universe and durable lifecycle
markers; it cannot be replaced by fairness of replenishment or by the
infinite global candidate type carrier.
***************************************************************************)

HistoricalDiscoveryFixedClockNonPacketServiceProperty(specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         (HistoricalDiscoveryFixedClockBlockedAtRank(
            node, clockValue, sourceRank)
            /\ OverdueResponsivePackets = {})
           ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                node, clockValue, sourceRank)

HistoricalDiscoveryFixedClockPacketServiceProperty(specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet \in AsyncPacketSet:
           \A known:
             HistoricalDiscoveryCandidateServeEpisodeSource(
               node, clockValue, sourceRank, packet, known)
               ~> (HistoricalDiscoveryFixedClockStrictRankGoal(
                     node, clockValue, sourceRank)
                    \/ HistoricalDiscoveryCandidateServeEpisodeResidual(
                         node, clockValue, sourceRank, packet, known))

HistoricalDiscoveryCandidateServeIdentityBudgetProperty(specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet \in AsyncPacketSet:
           \A known:
             HistoricalDiscoveryCandidateServeEpisodeResidual(
               node, clockValue, sourceRank, packet, known)
               ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                    node, clockValue, sourceRank)

HistoricalDiscoveryFixedClockConcreteServiceProperties(specification) ==
  /\ HistoricalDiscoveryFixedClockNonPacketServiceProperty(specification)
  /\ HistoricalDiscoveryFixedClockPacketServiceProperty(specification)

HistoricalDiscoveryFixedClockTemporalPrerequisites(specification) ==
  /\ HistoricalDiscoveryFixedClockConcreteServiceProperties(specification)
  /\ HistoricalDiscoveryCandidateServeIdentityBudgetProperty(specification)

HistoricalDiscoveryFixedClockRankDescentProperty(specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         HistoricalDiscoveryFixedClockBlockedAtRank(
           node, clockValue, sourceRank)
           ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                node, clockValue, sourceRank)

THEOREM HistoricalDiscoveryTemporalPrerequisitesCloseOneRankStep ==
  \A specification:
    HistoricalDiscoveryFixedClockTemporalPrerequisites(specification)
      => HistoricalDiscoveryFixedClockRankDescentProperty(specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalDiscoveryFixedClockTemporalPrerequisites(
                  specification)
         PROVE HistoricalDiscoveryFixedClockRankDescentProperty(
                 specification)
    <2>1. CASE specification
      <3>1. ASSUME NEW node \in Responsive,
                    NEW clockValue \in Nat,
                    NEW sourceRank \in
                      HistoricalDiscoveryFixedClockBlockerCarrier
             PROVE HistoricalDiscoveryFixedClockBlockedAtRank(
                     node, clockValue, sourceRank)
                     ~>
                   HistoricalDiscoveryFixedClockStrictRankGoal(
                     node, clockValue, sourceRank)
        <4>1. (HistoricalDiscoveryFixedClockBlockedAtRank(
                 node, clockValue, sourceRank)
                 /\ OverdueResponsivePackets = {})
                ~>
              HistoricalDiscoveryFixedClockStrictRankGoal(
                node, clockValue, sourceRank)
          BY <1>1, <2>1
             DEF HistoricalDiscoveryFixedClockTemporalPrerequisites,
                 HistoricalDiscoveryFixedClockConcreteServiceProperties,
                 HistoricalDiscoveryFixedClockNonPacketServiceProperty
        <4>2. \A packet \in AsyncPacketSet:
                 \A known:
                   HistoricalDiscoveryCandidateServeEpisodeSource(
                     node, clockValue, sourceRank, packet, known)
                     ~>
                   (HistoricalDiscoveryFixedClockStrictRankGoal(
                      node, clockValue, sourceRank)
                     \/ HistoricalDiscoveryCandidateServeEpisodeResidual(
                          node, clockValue, sourceRank, packet, known))
          BY <1>1, <2>1
             DEF HistoricalDiscoveryFixedClockTemporalPrerequisites,
                 HistoricalDiscoveryFixedClockConcreteServiceProperties,
                 HistoricalDiscoveryFixedClockPacketServiceProperty
        <4>3. \A packet \in AsyncPacketSet:
                 \A known:
                   HistoricalDiscoveryCandidateServeEpisodeResidual(
                     node, clockValue, sourceRank, packet, known)
                     ~>
                   HistoricalDiscoveryFixedClockStrictRankGoal(
                     node, clockValue, sourceRank)
          BY <1>1, <2>1
             DEF HistoricalDiscoveryFixedClockTemporalPrerequisites,
                 HistoricalDiscoveryCandidateServeIdentityBudgetProperty
        <4>4. \A packet \in AsyncPacketSet:
                 \A known:
                   HistoricalDiscoveryCandidateServeEpisodeSource(
                     node, clockValue, sourceRank, packet, known)
                     ~>
                   HistoricalDiscoveryFixedClockStrictRankGoal(
                     node, clockValue, sourceRank)
          BY <4>2, <4>3, PTL
        <4>5. HistoricalDiscoveryFixedClockBlockedAtRank(
                 node, clockValue, sourceRank)
                 /\ OverdueResponsivePackets # {}
                =>
              \E packet \in AsyncPacketSet:
                \E known:
                  HistoricalDiscoveryCandidateServeEpisodeSource(
                    node, clockValue, sourceRank, packet, known)
          BY HistoricalDiscoveryPacketProducerIdentitySetIsFinite,
             Isa
             DEF HistoricalDiscoveryCandidateServeEpisodeSource,
                 HistoricalDiscoveryFixedClockBlockedAtRank,
                 HistoricalDiscoveryFixedClockPending,
                 HistoricalDiscoverySelectedOverduePacket,
                 OverdueResponsivePackets,
                 AsyncPacketOwnsClockDeadline,
                 AsyncStrongTypeInvariant,
                 AsyncTypeInvariant, AsyncTransportTypeInvariant,
                 AsyncPacketContentTypeInvariant
        <4>6. (HistoricalDiscoveryFixedClockBlockedAtRank(
                 node, clockValue, sourceRank)
                 /\ OverdueResponsivePackets # {})
                ~>
              HistoricalDiscoveryFixedClockStrictRankGoal(
                node, clockValue, sourceRank)
          BY <4>4, <4>5, PTL
        <4> QED BY <4>1, <4>6, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1
         DEF HistoricalDiscoveryFixedClockRankDescentProperty
  <1> QED BY <1>1

(***************************************************************************
Well-founded fixed-clock closure.

This theorem consumes the exact concrete carrier/order already proved below
the action layer.  It does not inspect or strengthen the identity-budget
premise: once each source rank leads to exit or a strictly smaller rank, the
standard mechanized well-founded leads-to rule is the only composition step.
***************************************************************************)

HistoricalDiscoveryFixedClockClosureProperty(specification) ==
  specification
    => \A node \in Responsive, clockValue \in Nat:
         HistoricalDiscoveryFixedClockPending(node, clockValue)
           ~> HistoricalDiscoveryFixedClockProgressExit(
                node, clockValue)

THEOREM HistoricalDiscoveryRankDescentClosesFixedClock ==
  \A specification:
    HistoricalDiscoveryFixedClockRankDescentProperty(specification)
      => HistoricalDiscoveryFixedClockClosureProperty(specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalDiscoveryFixedClockRankDescentProperty(
                  specification)
         PROVE HistoricalDiscoveryFixedClockClosureProperty(
                 specification)
    <2>1. CASE specification
      <3>1. ASSUME NEW node \in Responsive,
                    NEW clockValue \in Nat
             PROVE HistoricalDiscoveryFixedClockPending(
                     node, clockValue)
                     ~>
                   HistoricalDiscoveryFixedClockProgressExit(
                     node, clockValue)
        <4>1. \A rank \in
                   HistoricalDiscoveryFixedClockBlockerCarrier:
                 HistoricalDiscoveryFixedClockBlockedAtRank(
                   node, clockValue, rank)
                   ~>
                 (HistoricalDiscoveryFixedClockProgressExit(
                    node, clockValue)
                   \/ \E lowerRank \in
                        SetLessThan(
                          rank,
                          HistoricalDiscoveryFixedClockBlockerOrdering,
                          HistoricalDiscoveryFixedClockBlockerCarrier):
                        HistoricalDiscoveryFixedClockBlockedAtRank(
                          node, clockValue, lowerRank))
          BY <1>1, <2>1
             DEF HistoricalDiscoveryFixedClockRankDescentProperty,
                 HistoricalDiscoveryFixedClockStrictRankGoal
        <4>2. \A rank \in
                   HistoricalDiscoveryFixedClockBlockerCarrier:
                 HistoricalDiscoveryFixedClockBlockedAtRank(
                   node, clockValue, rank)
                   ~>
                 HistoricalDiscoveryFixedClockProgressExit(
                   node, clockValue)
          BY <4>1,
             HistoricalDiscoveryFixedClockBlockerOrderingIsWellFounded,
             WellFoundedLeadsTo
        <4>3. HistoricalDiscoveryFixedClockPending(node, clockValue)
                =>
              \E rank \in
                   HistoricalDiscoveryFixedClockBlockerCarrier:
                HistoricalDiscoveryFixedClockBlockedAtRank(
                  node, clockValue, rank)
          BY HistoricalDiscoveryFixedClockPendingHasFiniteRank
        <4> QED BY <4>2, <4>3, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1
         DEF HistoricalDiscoveryFixedClockClosureProperty
  <1> QED BY <1>1

THEOREM HistoricalDiscoveryTemporalPrerequisitesCloseFixedClock ==
  \A specification:
    HistoricalDiscoveryFixedClockTemporalPrerequisites(specification)
      => HistoricalDiscoveryFixedClockClosureProperty(specification)
BY HistoricalDiscoveryTemporalPrerequisitesCloseOneRankStep,
   HistoricalDiscoveryRankDescentClosesFixedClock

(***************************************************************************
Finite clock-budget composition.

A fixed-clock exit at `asyncNow > clockValue` is not yet the release goal
when the round timeout is farther away.  The remaining distance to
`AsyncRoundTimeout` is therefore a second, independent natural-number rank.
It is represented by exact addition rather than truncated subtraction.
Every fixed-clock closure either reaches Decision/request/timeout or exposes a
strictly smaller positive remaining distance.
***************************************************************************)

HistoricalDiscoveryPositiveClockDistance(budget) ==
  /\ asyncNow \in Nat
  /\ budget \in Nat \ {0}
  /\ asyncNow + budget = AsyncRoundTimeout

(***************************************************************************
Every source action other than Tick freezes `asyncNow`.

This case split is intentionally over the executable transition relation,
including same-height crash/restart/replay and the atomic Serve receiver-close
rollback.  It prevents the outer natural-number budget from silently changing
under a non-clock producer or recovery step.
***************************************************************************)

THEOREM HistoricalDiscoveryNonTickNonRunnerStepLeavesClock ==
  /\ AsyncNonRunnerStep
  /\ ~AsyncTick
  => asyncNow' = asyncNow
PROOF
  <1>1. ASSUME AsyncNonRunnerStep, ~AsyncTick
         PROVE asyncNow' = asyncNow
    <2>1. CASE AsyncSetGST
      BY <2>1, Isa DEF AsyncSetGST, AsyncSchedulerVars
    <2>2. CASE AsyncTick
      BY <1>1, <2>2
    <2>3. CASE \E node \in ValidatorIds:
                  OpenHistoricalRecovery(node)
      BY <2>3, Isa
         DEF OpenHistoricalRecovery,
             AsyncSchedulerExceptHistoricalRecoveryTargets
    <2>4. CASE \E node \in AsyncCurrentResponsiveVoters:
                  DirectCommitCertificateDiscoveryStep(node)
      BY <2>4, Isa DEF DirectCommitCertificateDiscoveryStep
    <2>5. CASE \E node \in asyncHistoricalRecoveryTargets:
                  DirectHistoricalCommitCertificateDiscoveryStep(node)
      BY <2>5, Isa
         DEF DirectHistoricalCommitCertificateDiscoveryStep,
             CommitCertificateDiscoveryStepWork
    <2>6. CASE \E node \in AsyncArchiveIoServiceNodes:
                  ServiceIoWorker(node)
      BY <2>6, Isa DEF ServiceIoWorker, ServiceIoWorkerWork
    <2>7. CASE \E node \in asyncHistoricalRecoveryTargets:
                  ServiceHistoricalRecoveryIoWorker(node)
      BY <2>7, Isa
         DEF ServiceHistoricalRecoveryIoWorker, ServiceIoWorkerWork
    <2>8. CASE \E node \in AsyncCurrentResponsiveVoters:
                  EnqueueIoLocalControl(node)
      BY <2>8, Isa
         DEF EnqueueIoLocalControl, EnqueueIoLocalControlWork
    <2>9. CASE \E node \in asyncHistoricalRecoveryTargets:
                  EnqueueHistoricalRecoveryIoLocalControl(node)
      BY <2>9, Isa
         DEF EnqueueHistoricalRecoveryIoLocalControl,
             EnqueueIoLocalControlWork
    <2>10. CASE AsyncNetworkStep
      BY <2>10, Isa
         DEF AsyncNetworkStep, AdmitIngressPacket,
             AdmitHiddenPacket, CoalesceHiddenPacket
    <2>11. CASE AsyncFaultStep
      BY <2>11, AsyncFaultStepLeavesDiscoveryClock
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11
         DEF AsyncNonRunnerStep
  <1> QED BY <1>1

THEOREM HistoricalDiscoveryNonTickAsyncNextLeavesClock ==
  /\ AsyncNext
  /\ ~AsyncTick
  => asyncNow' = asyncNow
PROOF
  <1>1. ASSUME AsyncNext, ~AsyncTick
         PROVE asyncNow' = asyncNow
    <2>1. CASE AsyncNonCrashStep
      <3>1. CASE AsyncRunnerStep
        BY <3>1, AsyncRunnerStepLeavesDiscoveryClock
      <3>2. CASE AsyncNonRunnerStep
        BY <1>1, <3>2,
           HistoricalDiscoveryNonTickNonRunnerStepLeavesClock
      <3>3. CASE DriveResponsiveReplayHead \/ FinishResponsiveReplay
        BY <3>3, Isa
           DEF DriveResponsiveReplayHead, FinishResponsiveReplay
      <3>4. CASE RearmResponsiveRecovery
        BY <3>4, Isa DEF RearmResponsiveRecovery, AsyncSchedulerVars
      <3> QED BY <2>1, <3>1, <3>2, <3>3, <3>4
           DEF AsyncNonCrashStep
    <2>2. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <2>2, Isa DEF PreGstCrash, AsyncSchedulerVars
    <2>3. CASE \E node \in ValidatorIds:
                  PreGstResponsiveCrash(node)
      BY <2>3, Isa
         DEF PreGstResponsiveCrash, AsyncSchedulerVars
    <2>4. CASE PreGstResponsiveRestart
      BY <2>4, Isa
         DEF PreGstResponsiveRestart, AsyncSchedulerVars
    <2>5. CASE PreGstResponsiveReplay
      BY <2>5, Isa
         DEF PreGstResponsiveReplay, ResetNodeSchedulerForRestart
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5
         DEF AsyncNext
  <1> QED BY <1>1

THEOREM HistoricalDiscoveryEveryNonTickSourceStepLeavesClock ==
  /\ [AsyncNext]_AsyncAllVars
  /\ ~AsyncTick
  => asyncNow' = asyncNow
PROOF
  <1>1. ASSUME [AsyncNext]_AsyncAllVars, ~AsyncTick
         PROVE asyncNow' = asyncNow
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <2>1, Isa DEF AsyncAllVars, AsyncSchedulerVars
    <2>2. CASE AsyncNext
      BY <1>1, <2>2,
         HistoricalDiscoveryNonTickAsyncNextLeavesClock
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM HistoricalDiscoveryEveryNonTickSourceStepPreservesPositiveDistance ==
  \A budget:
    /\ HistoricalDiscoveryPositiveClockDistance(budget)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~AsyncTick
    => HistoricalDiscoveryPositiveClockDistance(budget)'
BY HistoricalDiscoveryEveryNonTickSourceStepLeavesClock, Isa
   DEF HistoricalDiscoveryPositiveClockDistance

HistoricalDiscoveryClockProgressGoal(node) ==
  \/ NodeHasDecision(node)
  \/ /\ HistoricalRecoveryTarget(node)
        /\ \/ ActiveCommitCertificateRequests(node) # {}
           \/ asyncNow >= AsyncRoundTimeout

HistoricalDiscoveryClockBudgetFrontier(node, budget) ==
  /\ gst
  /\ HistoricalRecoveryTarget(node)
  /\ ~NodeHasDecision(node)
  /\ ActiveCommitCertificateRequests(node) = {}
  /\ asyncNow \in Nat
  /\ budget \in Nat \ {0}
  /\ asyncNow + budget = AsyncRoundTimeout

HistoricalDiscoveryClockStrictBudgetGoal(node, budget) ==
  \/ HistoricalDiscoveryClockProgressGoal(node)
  \/ \E lowerBudget \in
       SetLessThan(budget, OpToRel(<, Nat), Nat):
       HistoricalDiscoveryClockBudgetFrontier(node, lowerBudget)

HistoricalDiscoveryFixedClockBudgetedPending(
    node, clockValue, budget) ==
  /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
  /\ clockValue + budget = AsyncRoundTimeout

HistoricalDiscoveryFixedClockBudgetedExit(
    node, clockValue, budget) ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ clockValue + budget = AsyncRoundTimeout
  /\ HistoricalDiscoveryFixedClockProgressExit(node, clockValue)

THEOREM HistoricalDiscoveryBudgetedFixedClockExitConsumesBudget ==
  \A node \in Responsive, clockValue, budget \in Nat:
    HistoricalDiscoveryFixedClockBudgetedExit(
      node, clockValue, budget)
      => HistoricalDiscoveryClockStrictBudgetGoal(node, budget)
BY SMT
   DEF HistoricalDiscoveryFixedClockBudgetedExit,
       HistoricalDiscoveryFixedClockProgressExit,
       HistoricalDiscoveryClockStrictBudgetGoal,
       HistoricalDiscoveryClockProgressGoal,
       HistoricalDiscoveryClockBudgetFrontier,
       SetLessThan, OpToRel,
       AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration, ModelConfiguration

HistoricalDiscoveryClockBudgetDescentProperty(specification) ==
  specification
    => \A node \in Responsive, budget \in Nat:
         HistoricalDiscoveryClockBudgetFrontier(node, budget)
           ~> HistoricalDiscoveryClockStrictBudgetGoal(node, budget)

THEOREM HistoricalDiscoveryFixedClockClosureLowersClockBudget ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalDiscoveryFixedClockClosureProperty(
         AsyncSpecAt(initialContext))
    => HistoricalDiscoveryClockBudgetDescentProperty(
         AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                HistoricalDiscoveryFixedClockClosureProperty(
                  AsyncSpecAt(initialContext))
         PROVE HistoricalDiscoveryClockBudgetDescentProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME NEW node \in Responsive, NEW budget \in Nat
           PROVE HistoricalDiscoveryClockBudgetFrontier(node, budget)
                   ~>
                 HistoricalDiscoveryClockStrictBudgetGoal(node, budget)
      <3>1. \A clockValue \in Nat:
               HistoricalDiscoveryFixedClockPending(node, clockValue)
                 ~>
               HistoricalDiscoveryFixedClockProgressExit(
                 node, clockValue)
        BY <1>1
           DEF HistoricalDiscoveryFixedClockClosureProperty
      <3>2. [](gst => []gst)
        BY <1>1, AsyncSpecKeepsGstOnceSet
      <3>3. []AsyncStrongTypeInvariant
        BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
      <3>4. \A clockValue \in Nat:
               HistoricalDiscoveryFixedClockBudgetedPending(
                 node, clockValue, budget)
                 ~>
               HistoricalDiscoveryFixedClockBudgetedExit(
                 node, clockValue, budget)
        BY <3>1, <3>2, <3>3, PTL
           DEF HistoricalDiscoveryFixedClockBudgetedPending,
               HistoricalDiscoveryFixedClockBudgetedExit
      <3>5. \A clockValue \in Nat:
               HistoricalDiscoveryFixedClockBudgetedPending(
                 node, clockValue, budget)
                 ~>
               HistoricalDiscoveryClockStrictBudgetGoal(node, budget)
        BY <3>4,
           HistoricalDiscoveryBudgetedFixedClockExitConsumesBudget,
           PTL
      <3>6. /\ AsyncStrongTypeInvariant
              /\ HistoricalDiscoveryClockBudgetFrontier(node, budget)
             =>
           \E clockValue \in Nat:
             HistoricalDiscoveryFixedClockBudgetedPending(
               node, clockValue, budget)
        BY Isa
           DEF HistoricalDiscoveryClockBudgetFrontier,
               HistoricalDiscoveryFixedClockBudgetedPending,
               HistoricalDiscoveryFixedClockPending
      <3> QED BY <3>3, <3>5, <3>6, PTL
    <2> QED BY <1>1, <2>1
         DEF HistoricalDiscoveryClockBudgetDescentProperty
  <1> QED BY <1>1

HistoricalDiscoveryClockBudgetClosureProperty(specification) ==
  specification
    => \A node \in Responsive, budget \in Nat:
         HistoricalDiscoveryClockBudgetFrontier(node, budget)
           ~> HistoricalDiscoveryClockProgressGoal(node)

THEOREM HistoricalDiscoveryClockBudgetDescentClosesTimeout ==
  \A specification:
    HistoricalDiscoveryClockBudgetDescentProperty(specification)
      => HistoricalDiscoveryClockBudgetClosureProperty(specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalDiscoveryClockBudgetDescentProperty(
                  specification)
         PROVE HistoricalDiscoveryClockBudgetClosureProperty(
                 specification)
    <2>1. CASE specification
      <3>1. ASSUME NEW node \in Responsive
             PROVE \A budget \in Nat:
                     HistoricalDiscoveryClockBudgetFrontier(node, budget)
                       ~>
                     HistoricalDiscoveryClockProgressGoal(node)
        <4>1. \A budget \in Nat:
                 HistoricalDiscoveryClockBudgetFrontier(node, budget)
                   ~>
                 (HistoricalDiscoveryClockProgressGoal(node)
                   \/ \E lowerBudget \in
                        SetLessThan(
                          budget, OpToRel(<, Nat), Nat):
                        HistoricalDiscoveryClockBudgetFrontier(
                          node, lowerBudget))
          BY <1>1, <2>1
             DEF HistoricalDiscoveryClockBudgetDescentProperty,
                 HistoricalDiscoveryClockStrictBudgetGoal
        <4> QED BY <4>1, NatLessThanWellFounded,
             WellFoundedLeadsTo
      <3> QED BY <3>1
    <2> QED BY <2>1
         DEF HistoricalDiscoveryClockBudgetClosureProperty
  <1> QED BY <1>1

THEOREM HistoricalDiscoveryClockBudgetClosureReachesReleaseGoal ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalDiscoveryClockBudgetClosureProperty(
         AsyncSpecAt(initialContext))
    => HistoricalCommitCertificateDiscoveryClockProgressProperty(
         AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                HistoricalDiscoveryClockBudgetClosureProperty(
                  AsyncSpecAt(initialContext))
         PROVE
           HistoricalCommitCertificateDiscoveryClockProgressProperty(
             AsyncSpecAt(initialContext))
    <2>1. ASSUME NEW node \in Responsive
           PROVE (gst /\ HistoricalRecoveryTarget(node))
                   ~>
                 HistoricalDiscoveryClockProgressGoal(node)
      <3>1. \A budget \in Nat:
               HistoricalDiscoveryClockBudgetFrontier(node, budget)
                 ~>
               HistoricalDiscoveryClockProgressGoal(node)
        BY <1>1
           DEF HistoricalDiscoveryClockBudgetClosureProperty
      <3>2. []AsyncStrongTypeInvariant
        BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
      <3>3. /\ AsyncStrongTypeInvariant
              /\ gst
              /\ HistoricalRecoveryTarget(node)
              /\ ~HistoricalDiscoveryClockProgressGoal(node)
             =>
           \E budget \in Nat:
             HistoricalDiscoveryClockBudgetFrontier(node, budget)
        BY SMT
           DEF HistoricalDiscoveryClockProgressGoal,
               HistoricalDiscoveryClockBudgetFrontier,
               AsyncStrongTypeInvariant, AsyncTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
               AsyncConfiguration, ModelConfiguration
      <3> QED BY <3>1, <3>2, <3>3, PTL
    <2> QED BY <1>1, <2>1
         DEF HistoricalCommitCertificateDiscoveryClockProgressProperty,
             HistoricalDiscoveryClockProgressGoal
  <1> QED BY <1>1

THEOREM HistoricalDiscoveryTemporalPrerequisitesCloseClockProgress ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalDiscoveryFixedClockTemporalPrerequisites(
         AsyncSpecAt(initialContext))
    => HistoricalCommitCertificateDiscoveryClockProgressProperty(
         AsyncSpecAt(initialContext))
BY HistoricalDiscoveryTemporalPrerequisitesCloseFixedClock,
   HistoricalDiscoveryFixedClockClosureLowersClockBudget,
   HistoricalDiscoveryClockBudgetDescentClosesTimeout,
   HistoricalDiscoveryClockBudgetClosureReachesReleaseGoal

=============================================================================
