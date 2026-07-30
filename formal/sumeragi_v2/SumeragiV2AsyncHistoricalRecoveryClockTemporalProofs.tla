---- MODULE SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryClockRankActionProofs,
        SumeragiV2AsyncCausalWorkBudgetProofs

(***************************************************************************
Temporal composition for the historical discovery clock.

The concrete action modules below this leaf prove the fixed-clock rank,
well-foundedness, publication/no-refill frames, exact ingress removal, latent
owner entry, dormant-I/O handoff, and due runner/I/O service edges.  This
module supplies the temporal composition shape without hiding the remaining
candidate/Serve producer episode.

The fixed-clock split has three deliberately separate leaves:

  * non-packet service closes below from the individually fair runner, I/O,
    and final Tick actions;
  * packet service reaches strict rank descent or exposes a genuinely new
    candidate/Serve service identity relative to the source snapshot; and
  * the finite identity-budget bridge closes that non-descent episode.

The shared target-neutral lifecycle algebra below proves that the source
snapshot is contained in one finite frozen semantic universe and that every
genuine discovery consumes its complement budget.  Four exact physical
corridor clauses remain named at the bottom of the module: packet action
selection, service of that fixed action, exact Candidate-minimum runner
service, and exact Serve-minimum worker service.  The packet-prefix edges are
action-local below this module, while the reusable Stage fair-service
theorems live above it, so importing the latter here would create a dependency
cycle.  They are not replaced by weak fairness of an existential action
union.  The global `AsyncCandidateSet` is deliberately not used as a frozen
universe: production proofs run with `ViewDomain = Nat`.
Replenishment is never a progress goal here.  No current-voter Decision
convergence, rotating-leader convergence, application liveness, or clock
shortcut is assumed.
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
Finite owner-mode handoff.

A responsive timed owner can be served by three separately fair concrete
runner/worker families.  The owner can move only from the ordinary current-
voter arm to historical recovery and then to the applied archive arm (or
directly to the applied arm).  This small rank lets the temporal layer switch
between those individually quantified fairness clauses without assuming
weak fairness of their union.  A mode handoff is bookkeeping, not fixed-clock
progress.
***************************************************************************)

HistoricalDiscoveryTimedOwnerModeCarrier == 0..2

HistoricalDiscoveryTimedOwnerMode(owner) ==
  IF owner \in AsyncResponsiveAppliedArchiveServers
  THEN 0
  ELSE IF HistoricalRecoveryTarget(owner)
       THEN 1
       ELSE 2

HistoricalDiscoveryTimedOwnerModeOrdering ==
  OpToRel(<, HistoricalDiscoveryTimedOwnerModeCarrier)

THEOREM HistoricalDiscoveryTimedOwnerHasFiniteMode ==
  \A owner \in AsyncTimedServiceNodes:
    AsyncStrongTypeInvariant
      => HistoricalDiscoveryTimedOwnerMode(owner)
           \in HistoricalDiscoveryTimedOwnerModeCarrier
BY Isa
   DEF HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier

THEOREM HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst ==
  \A owner \in AsyncTimedServiceNodes:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => HistoricalDiscoveryTimedOwnerMode(owner)'
         <= HistoricalDiscoveryTimedOwnerMode(owner)
BY HistoricalTargetOwnerSurvivesOrTransfersAfterGst,
   HistoricalAppliedArchiveOwnersAreMonotoneAfterGst,
   HistoricalTimedServiceNodesAreMonotoneAfterGst,
   IsaT(300)
   DEF HistoricalDiscoveryTimedOwnerMode,
       HistoricalRecoveryTarget,
       AsyncTimedServiceNodes,
       AsyncArchiveIoServiceNodes

THEOREM HistoricalDiscoveryTimedOwnerModeOrderingIsWellFounded ==
  IsWellFoundedOn(
    HistoricalDiscoveryTimedOwnerModeOrdering,
    HistoricalDiscoveryTimedOwnerModeCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset, Isa
   DEF HistoricalDiscoveryTimedOwnerModeOrdering,
       HistoricalDiscoveryTimedOwnerModeCarrier

(***************************************************************************
Logical-lineage identities exposed by the producer episode.

The finite complement at this layer deliberately records one Candidate
causal lineage, not one physical Candidate occurrence.  That is sufficient
to coalesce exact transport retry, but it must not be used as the identity of
the runner occurrence: one serviced parent may be replaced by as many as
three distinct causal children with the same origin.  The exact physical
identity, lifecycle stage, and immutable admission ordinal are frozen below
where the runner kernel is stated.  Serve identities use the existing exact
logical request key, not the fresh physical job nonce.  The outer tag keeps
the two lifecycle namespaces disjoint.
***************************************************************************)

HistoricalDiscoveryCandidateOwnerIdentity(candidate) ==
  <<"Candidate", candidate.causalOrigin>>

HistoricalDiscoveryServeOwnerIdentity(node, job) ==
  <<"Serve", AsyncIoServeJobIdentity(node, job)>>

HistoricalDiscoveryPacketCandidateOwnerIdentityCarrier(packet) ==
  {"Candidate"}
    \X HistoricalDiscoveryPacketCandidateCausalOriginCarrier(packet)

HistoricalDiscoveryPacketServeOwnerIdentityCarrier(packet) ==
  {"Serve"} \X HistoricalDiscoveryPacketServeIdentityCarrier(packet)

HistoricalDiscoveryPacketProducerIdentityCarrier(packet) ==
  AsyncTargetNeutralLifecycleOwnerCarrier(
    HistoricalDiscoveryPacketCandidateCausalOriginCarrier(packet),
    HistoricalDiscoveryPacketServeIdentityCarrier(packet))

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
  {<<"Candidate", record.identity.payload.causalOrigin>>:
     record \in AsyncCandidateServiceTombstones,
     record.identity.payload.causalOrigin
       \in HistoricalDiscoveryPacketCandidateCausalOriginCarrier(packet)}
    \cup
  {<<"Candidate", record.origin>>:
     record \in AsyncCandidateLifecycleAdmissions,
     record.origin
       \in HistoricalDiscoveryPacketCandidateCausalOriginCarrier(packet)}

HistoricalDiscoveryPacketServeCoveredIdentitySet(packet) ==
  LET carrier == HistoricalDiscoveryPacketServeIdentityCarrier(packet)
  IN HistoricalDiscoveryPacketServeIdentitySet(packet)
       \cup
     {<<"Serve", reservation.identity>>:
        reservation \in asyncServeReservations,
        reservation.identity \in carrier}
       \cup
     {<<"Serve", tombstone.identity>>:
        tombstone \in asyncServeTombstones,
        tombstone.identity \in carrier}
       \cup
     UNION {
       {<<"Serve", tombstone.identity>>:
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
   AsyncTargetNeutralLifecycleOwnerCarrierIsFinite
   DEF HistoricalDiscoveryPacketProducerIdentityCarrier,
       HistoricalDiscoveryPacketCandidateOwnerIdentityCarrier,
       HistoricalDiscoveryPacketServeOwnerIdentityCarrier

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
       HistoricalDiscoveryPacketCandidateOwnerIdentityCarrier,
       HistoricalDiscoveryPacketServeOwnerIdentityCarrier

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

(***************************************************************************
Endpoint-neutral finite-complement episode.

`known` is a temporal parameter, not mutable model state.  Intersecting it
with the frozen packet carrier preserves every relevant discovery test while
also making the shared endpoint-neutral algebra applicable when a caller
supplies an arbitrary finite set.  The live set charged here includes both
physical Candidate/Serve owners and their durable admission/tombstone
coverage.  Consequently retry coalescing is already represented before an
occurrence-rank consumer is selected.
***************************************************************************)

HistoricalDiscoveryPacketFrozenKnownIdentitySet(packet, known) ==
  known \cap HistoricalDiscoveryPacketProducerIdentityCarrier(packet)

HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
    node, clockValue, sourceRank, packet, known, budget) ==
  LET currentRank ==
        HistoricalDiscoveryConcreteFixedClockRank(clockValue)
      frozenKnown ==
        HistoricalDiscoveryPacketFrozenKnownIdentitySet(packet, known)
  IN /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
     /\ sourceRank
          \in HistoricalDiscoveryFixedClockBlockerCarrier
     /\ currentRank
          \in HistoricalDiscoveryFixedClockBlockerCarrier
     /\ packet \in OverdueResponsivePackets
     /\ packet = HistoricalDiscoverySelectedOverduePacket
     /\ HistoricalDiscoveryFixedClockProducerPrefix(currentRank)
          = HistoricalDiscoveryFixedClockProducerPrefix(sourceRank)
     /\ ~HistoricalDiscoveryFixedClockStrictRankGoal(
          node, clockValue, sourceRank)
     /\ AsyncTargetNeutralLifecycleEpisodeAtBudget(
          HistoricalDiscoveryPacketCandidateCausalOriginCarrier(packet),
          HistoricalDiscoveryPacketServeIdentityCarrier(packet),
          HistoricalDiscoveryPacketProducerCoveredIdentitySet(packet),
          frozenKnown, budget)

HistoricalDiscoveryCandidateServeLifecycleDiscovery(
    node, clockValue, sourceRank, packet, known, budget) ==
  /\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
       node, clockValue, sourceRank, packet, known, budget)
  /\ AsyncTargetNeutralLifecycleDiscoveredOwnerSet(
       HistoricalDiscoveryPacketProducerCoveredIdentitySet(packet),
       HistoricalDiscoveryPacketFrozenKnownIdentitySet(packet, known))
       # {}

HistoricalDiscoveryCandidateServeLifecycleBudgetAtRank(
    node, clockValue, sourceRank, budget) ==
  /\ budget \in Nat
  /\ \E packet, known:
       HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
         node, clockValue, sourceRank, packet, known, budget)

THEOREM HistoricalDiscoveryPacketFrozenKnownStaysInCarrier ==
  \A packet, known:
    HistoricalDiscoveryPacketFrozenKnownIdentitySet(packet, known)
      \subseteq HistoricalDiscoveryPacketProducerIdentityCarrier(packet)
BY Isa DEF HistoricalDiscoveryPacketFrozenKnownIdentitySet

THEOREM HistoricalDiscoveryCandidateServeSourceStartsNeutralEpisode ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known:
      HistoricalDiscoveryCandidateServeEpisodeSource(
        node, clockValue, sourceRank, packet, known)
        => \E budget \in Nat:
             HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
               node, clockValue, sourceRank, packet, known, budget)
BY HistoricalDiscoveryPacketProducerIdentityCarrierIsFinite,
   HistoricalDiscoveryPacketProducerCoverageStaysInFrozenCarrier,
   AsyncTargetNeutralLifecycleEpisodeBudgetIsFiniteAndCoalesced,
   IsaT(240)
   DEF HistoricalDiscoveryCandidateServeEpisodeSource,
       HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
       HistoricalDiscoveryPacketFrozenKnownIdentitySet,
       HistoricalDiscoveryPacketProducerIdentityCarrier,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       AsyncTargetNeutralLifecycleEpisodeAtBudget,
       AsyncTargetNeutralLifecycleKnownOwnerSet,
       AsyncTargetNeutralLifecycleEpisodeBudget

THEOREM HistoricalDiscoveryCandidateServeResidualStartsNeutralDiscovery ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known:
      HistoricalDiscoveryCandidateServeEpisodeResidual(
        node, clockValue, sourceRank, packet, known)
        => \E budget \in Nat:
             HistoricalDiscoveryCandidateServeLifecycleDiscovery(
               node, clockValue, sourceRank, packet, known, budget)
BY HistoricalDiscoveryPacketProducerIdentityCarrierIsFinite,
   HistoricalDiscoveryPacketProducerCoverageStaysInFrozenCarrier,
   AsyncTargetNeutralLifecycleEpisodeBudgetIsFiniteAndCoalesced,
   IsaT(300)
   DEF HistoricalDiscoveryCandidateServeEpisodeResidual,
       HistoricalDiscoveryCandidateServeLifecycleDiscovery,
       HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
       HistoricalDiscoveryPacketFrozenKnownIdentitySet,
       HistoricalDiscoveryPacketProducerIdentityCarrier,
       AsyncTargetNeutralLifecycleEpisodeAtBudget,
       AsyncTargetNeutralLifecycleKnownOwnerSet,
       AsyncTargetNeutralLifecycleEpisodeBudget,
       AsyncTargetNeutralLifecycleDiscoveredOwnerSet

THEOREM HistoricalDiscoveryCandidateServeDiscoveryConsumesNeutralBudget ==
  \A node, clockValue, sourceRank, packet, known, budget:
    HistoricalDiscoveryCandidateServeLifecycleDiscovery(
      node, clockValue, sourceRank, packet, known, budget)
      => \E known2:
           \E budget2 \in
                SetLessThan(
                  budget,
                  AsyncTargetNeutralLifecycleBudgetOrdering,
                  Nat):
             HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
               node, clockValue, sourceRank, packet, known2, budget2)
BY AsyncTargetNeutralLifecycleDiscoveryStrictlyConsumesBudget,
   IsaT(300)
   DEF HistoricalDiscoveryCandidateServeLifecycleDiscovery,
       HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
       HistoricalDiscoveryPacketFrozenKnownIdentitySet,
       AsyncTargetNeutralLifecycleKnownAdvanceGoal,
       AsyncTargetNeutralLifecycleBudgetOrdering,
       SetLessThan, OpToRel

(***************************************************************************
Concrete fixed-clock non-packet owners.

The mode-indexed predicates below retain one exact owner while allowing the
owner's service family to hand off monotonically.  At mode 2 the owner is an
ordinary current voter, at mode 1 an active historical-recovery target, and
at mode 0 an applied archive server.  Each mode names one action which has
its own quantified clause in `AsyncFairnessAt`; no fairness of the action
union is used.
***************************************************************************)

HistoricalDiscoveryDueNodeOwnerAtMode(
    node, clockValue, sourceRank, owner, mode) ==
  /\ HistoricalDiscoveryFixedClockBlockedAtRank(
       node, clockValue, sourceRank)
  /\ OverdueResponsivePackets = {}
  /\ owner \in HistoricalDiscoveryNodeBlockersAt(clockValue)
  /\ mode \in HistoricalDiscoveryTimedOwnerModeCarrier
  /\ HistoricalDiscoveryTimedOwnerMode(owner) = mode

HistoricalDiscoveryDueIoOwnerAtMode(
    node, clockValue, sourceRank, owner, mode) ==
  /\ HistoricalDiscoveryFixedClockBlockedAtRank(
       node, clockValue, sourceRank)
  /\ OverdueResponsivePackets = {}
  /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
  /\ owner \in HistoricalDiscoveryActiveIoBlockersAt(clockValue)
  /\ mode \in HistoricalDiscoveryTimedOwnerModeCarrier
  /\ HistoricalDiscoveryTimedOwnerMode(owner) = mode

HistoricalDiscoveryDueNodeModeProgressGoal(
    node, clockValue, sourceRank, owner, mode) ==
  \/ HistoricalDiscoveryFixedClockStrictRankGoal(
       node, clockValue, sourceRank)
  \/ \E lowerMode \in
       SetLessThan(
         mode,
         HistoricalDiscoveryTimedOwnerModeOrdering,
         HistoricalDiscoveryTimedOwnerModeCarrier):
       HistoricalDiscoveryDueNodeOwnerAtMode(
         node, clockValue, sourceRank, owner, lowerMode)

HistoricalDiscoveryDueIoModeProgressGoal(
    node, clockValue, sourceRank, owner, mode) ==
  \/ HistoricalDiscoveryFixedClockStrictRankGoal(
       node, clockValue, sourceRank)
  \/ \E lowerMode \in
       SetLessThan(
         mode,
         HistoricalDiscoveryTimedOwnerModeOrdering,
         HistoricalDiscoveryTimedOwnerModeCarrier):
       HistoricalDiscoveryDueIoOwnerAtMode(
         node, clockValue, sourceRank, owner, lowerMode)

HistoricalDiscoveryDueNodeModeFairAction(owner, mode) ==
  CASE mode = 0 -> PostGstRunHistoricalServer(owner)
    [] mode = 1 -> PostGstRunHistoricalRecoveryNode(owner)
    [] mode = 2 -> PostGstRunNode(owner)
    [] OTHER -> FALSE

HistoricalDiscoveryDueIoModeFairAction(owner, mode) ==
  CASE mode = 1 -> PostGstServiceHistoricalRecoveryIoWorker(owner)
    [] mode \in {0, 2} -> PostGstServiceIoWorker(owner)
    [] OTHER -> FALSE

THEOREM HistoricalDiscoveryDueNodeModeHasEnabledExactFairAction ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    HistoricalDiscoveryDueNodeOwnerAtMode(
      node, clockValue, sourceRank, owner, mode)
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
   DEF HistoricalDiscoveryDueNodeOwnerAtMode,
       HistoricalDiscoveryDueNodeModeFairAction,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       HistoricalDiscoveryFixedClockPending,
       HistoricalRecoveryTarget,
       HistoricalDiscoveryNodeBlockersAt,
       HistoricalDiscoveryDueNodeService,
       AsyncTimedServiceNodes,
       AsyncArchiveIoServiceNodes,
       AsyncAllVars

THEOREM HistoricalDiscoveryDueIoModeHasEnabledExactFairAction ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    HistoricalDiscoveryDueIoOwnerAtMode(
      node, clockValue, sourceRank, owner, mode)
      => ENABLED
           <<HistoricalDiscoveryDueIoModeFairAction(
               owner, mode)>>_AsyncAllVars
BY QueuedIoEnablesPostGstService,
   GstHistoricalIoWorkerIsEnabled,
   ConcreteDueIoServiceActionsRemoveExactActiveBlocker,
   HistoricalDiscoveryOwnersIncludeNonVoterService,
   AsyncStrongTypeProjectsAsyncType,
   ExpandENABLED, IsaT(300)
   DEF HistoricalDiscoveryDueIoOwnerAtMode,
       HistoricalDiscoveryDueIoModeFairAction,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       HistoricalDiscoveryFixedClockPending,
       HistoricalDiscoveryActiveIoBlockersAt,
       HistoricalDiscoveryDueIoQueue,
       AsyncTimedServiceNodes,
       AsyncArchiveIoServiceNodes,
       AsyncAllVars

THEOREM HistoricalDiscoveryDueNodeModeFairOccurrenceReachesRankGoal ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ HistoricalDiscoveryDueNodeOwnerAtMode(
         node, clockValue, sourceRank, owner, mode)
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryDueNodeModeFairAction(owner, mode)
    => HistoricalDiscoveryFixedClockStrictRankGoal(
         node, clockValue, sourceRank)'
BY HistoricalDiscoveryFixedClockLatentOwnerEntryStrictlyDescends,
   HistoricalDiscoveryFixedClockDormantHandoffStrictlyDescends,
   HistoricalDiscoveryDueNodeServiceStrictlyDescends,
   ConcreteDueNodeServiceActionsResetDeadlineAboveFixedClock,
   HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock,
   HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame,
   HistoricalDiscoveryBroadcastControlHelpersHaveFixedClockFrame,
   HistoricalDiscoveryRetransmissionHelpersHaveFixedClockFrame,
   HistoricalDiscoveryDirectRequestPublicationHasFixedClockFrame,
   IsaT(900)
   DEF HistoricalDiscoveryDueNodeOwnerAtMode,
       HistoricalDiscoveryDueNodeModeFairAction,
       HistoricalDiscoveryFixedClockStrictRankGoal,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       HistoricalDiscoveryFixedClockActionGoal,
       HistoricalDiscoveryFixedClockOuterPrefixEqual,
       HistoricalDiscoveryFixedClockPublicationFrame,
       HistoricalDiscoveryNodeServiceOutcome,
       HistoricalDiscoveryDormantGateHandoff,
       HistoricalDiscoveryNewTransportPacketsAreFuture,
       PostGstRunNode, PostGstRunHistoricalRecoveryNode,
       PostGstRunHistoricalServer,
       AsyncAllVars

THEOREM HistoricalDiscoveryDueIoModeFairOccurrenceReachesRankGoal ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ HistoricalDiscoveryDueIoOwnerAtMode(
         node, clockValue, sourceRank, owner, mode)
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryDueIoModeFairAction(owner, mode)
    => HistoricalDiscoveryFixedClockStrictRankGoal(
         node, clockValue, sourceRank)'
BY HistoricalDiscoveryFixedClockLatentOwnerEntryStrictlyDescends,
   HistoricalDiscoveryDueIoServiceStrictlyDescends,
   ConcreteDueIoServiceActionsRemoveExactActiveBlocker,
   HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock,
   HistoricalDiscoveryResponsePublicationHasFixedClockFrame,
   IsaT(900)
   DEF HistoricalDiscoveryDueIoOwnerAtMode,
       HistoricalDiscoveryDueIoModeFairAction,
       HistoricalDiscoveryFixedClockStrictRankGoal,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       HistoricalDiscoveryFixedClockActionGoal,
       HistoricalDiscoveryFixedClockOuterPrefixEqual,
       HistoricalDiscoveryFixedClockPublicationFrame,
       HistoricalDiscoveryDueIoQueueServiceOutcome,
       HistoricalDiscoveryNewTransportPacketsAreFuture,
       PostGstServiceIoWorker,
       PostGstServiceHistoricalRecoveryIoWorker,
       AsyncAllVars

THEOREM HistoricalDiscoveryDueNodeModeStepPreservesOrProgresses ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ HistoricalDiscoveryDueNodeOwnerAtMode(
         node, clockValue, sourceRank, owner, mode)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDiscoveryDueNodeModeProgressGoal(
            node, clockValue, sourceRank, owner, mode)'
       \/ HistoricalDiscoveryDueNodeOwnerAtMode(
            node, clockValue, sourceRank, owner, mode)'
BY HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   HistoricalDiscoveryFixedClockIngressStrictlyDescends,
   HistoricalDiscoveryFixedClockLatentOwnerEntryStrictlyDescends,
   HistoricalDiscoveryFixedClockDormantHandoffStrictlyDescends,
   HistoricalDiscoveryDueNodeServiceStrictlyDescends,
   HistoricalDiscoveryDueIoServiceStrictlyDescends,
   HistoricalDiscoveryFixedClockTickReachesExit,
   HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame,
   HistoricalDiscoveryBroadcastControlHelpersHaveFixedClockFrame,
   HistoricalDiscoveryRetransmissionHelpersHaveFixedClockFrame,
   HistoricalDiscoveryDirectRequestPublicationHasFixedClockFrame,
   HistoricalDiscoveryResponsePublicationHasFixedClockFrame,
   HistoricalDiscoveryByzantineCertifiedRequestHasFixedClockFrame,
   HistoricalDiscoverySingletonFaultInjectorsHaveFixedClockFrame,
   AsyncBracketNextPreservesStrongTypeInvariant,
   IsaT(1200)
   DEF HistoricalDiscoveryDueNodeOwnerAtMode,
       HistoricalDiscoveryDueNodeModeProgressGoal,
       HistoricalDiscoveryFixedClockStrictRankGoal,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       HistoricalDiscoveryTimedOwnerModeOrdering,
       SetLessThan, OpToRel,
       HistoricalDiscoveryFixedClockPublicationFrame,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncAllVars

THEOREM HistoricalDiscoveryDueIoModeStepPreservesOrProgresses ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ HistoricalDiscoveryDueIoOwnerAtMode(
         node, clockValue, sourceRank, owner, mode)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDiscoveryDueIoModeProgressGoal(
            node, clockValue, sourceRank, owner, mode)'
       \/ HistoricalDiscoveryDueIoOwnerAtMode(
            node, clockValue, sourceRank, owner, mode)'
BY HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   HistoricalDiscoveryFixedClockIngressStrictlyDescends,
   HistoricalDiscoveryFixedClockLatentOwnerEntryStrictlyDescends,
   HistoricalDiscoveryFixedClockDormantHandoffStrictlyDescends,
   HistoricalDiscoveryDueNodeServiceStrictlyDescends,
   HistoricalDiscoveryDueIoServiceStrictlyDescends,
   HistoricalDiscoveryFixedClockTickReachesExit,
   HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame,
   HistoricalDiscoveryBroadcastControlHelpersHaveFixedClockFrame,
   HistoricalDiscoveryRetransmissionHelpersHaveFixedClockFrame,
   HistoricalDiscoveryDirectRequestPublicationHasFixedClockFrame,
   HistoricalDiscoveryResponsePublicationHasFixedClockFrame,
   HistoricalDiscoveryByzantineCertifiedRequestHasFixedClockFrame,
   HistoricalDiscoverySingletonFaultInjectorsHaveFixedClockFrame,
   AsyncBracketNextPreservesStrongTypeInvariant,
   IsaT(1200)
   DEF HistoricalDiscoveryDueIoOwnerAtMode,
       HistoricalDiscoveryDueIoModeProgressGoal,
       HistoricalDiscoveryFixedClockStrictRankGoal,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       HistoricalDiscoveryTimedOwnerModeOrdering,
       SetLessThan, OpToRel,
       HistoricalDiscoveryFixedClockPublicationFrame,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncAllVars

THEOREM AsyncSpecProvidesHistoricalDiscoveryDueNodeModeFairness ==
  \A initialContext,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalDiscoveryDueNodeOwnerAtMode(
         node, clockValue, sourceRank, owner, mode)
    => WF_AsyncAllVars(
         HistoricalDiscoveryDueNodeModeFairAction(owner, mode))
BY AsyncSpecAlwaysUsesFixedResponsiveVoters, PTL, Isa
   DEF HistoricalDiscoveryDueNodeOwnerAtMode,
       HistoricalDiscoveryDueNodeModeFairAction,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       HistoricalDiscoveryFixedClockPending,
       HistoricalDiscoveryNodeBlockersAt,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncSpecAt, AsyncFairnessAt

THEOREM AsyncSpecProvidesHistoricalDiscoveryDueIoModeFairness ==
  \A initialContext,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalDiscoveryDueIoOwnerAtMode(
         node, clockValue, sourceRank, owner, mode)
    => WF_AsyncAllVars(
         HistoricalDiscoveryDueIoModeFairAction(owner, mode))
BY AsyncSpecAlwaysUsesFixedResponsiveVoters, PTL, Isa
   DEF HistoricalDiscoveryDueIoOwnerAtMode,
       HistoricalDiscoveryDueIoModeFairAction,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       HistoricalDiscoveryFixedClockPending,
       HistoricalDiscoveryActiveIoBlockersAt,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncSpecAt, AsyncFairnessAt

THEOREM AsyncSpecHistoricalDiscoveryDueNodeModeMakesProgress ==
  \A initialContext,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    AsyncSpecAt(initialContext)
      => (HistoricalDiscoveryDueNodeOwnerAtMode(
            node, clockValue, sourceRank, owner, mode)
           ~> HistoricalDiscoveryDueNodeModeProgressGoal(
                node, clockValue, sourceRank, owner, mode))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank \in
                  HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                NEW mode \in HistoricalDiscoveryTimedOwnerModeCarrier,
                AsyncSpecAt(initialContext)
         PROVE HistoricalDiscoveryDueNodeOwnerAtMode(
                 node, clockValue, sourceRank, owner, mode)
                 ~>
               HistoricalDiscoveryDueNodeModeProgressGoal(
                 node, clockValue, sourceRank, owner, mode)
    <2>1. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>2. [](HistoricalDiscoveryDueNodeOwnerAtMode(
                node, clockValue, sourceRank, owner, mode)
              /\ ~HistoricalDiscoveryDueNodeModeProgressGoal(
                   node, clockValue, sourceRank, owner, mode)
             => ENABLED
                  <<HistoricalDiscoveryDueNodeModeFairAction(
                      owner, mode)>>_AsyncAllVars)
      BY <2>1,
         HistoricalDiscoveryDueNodeModeHasEnabledExactFairAction,
         PTL
    <2>3. [](HistoricalDiscoveryDueNodeOwnerAtMode(
                node, clockValue, sourceRank, owner, mode)
              /\ ~HistoricalDiscoveryDueNodeModeProgressGoal(
                   node, clockValue, sourceRank, owner, mode)
              /\ <<HistoricalDiscoveryDueNodeModeFairAction(
                       owner, mode)>>_AsyncAllVars
             => HistoricalDiscoveryDueNodeModeProgressGoal(
                  node, clockValue, sourceRank, owner, mode)')
      BY HistoricalDiscoveryDueNodeModeFairOccurrenceReachesRankGoal,
         PTL
         DEF HistoricalDiscoveryDueNodeModeProgressGoal
    <2>4. [](HistoricalDiscoveryDueNodeOwnerAtMode(
                node, clockValue, sourceRank, owner, mode)
              /\ ~HistoricalDiscoveryDueNodeModeProgressGoal(
                   node, clockValue, sourceRank, owner, mode)
              /\ [AsyncNext]_AsyncAllVars
             => \/ HistoricalDiscoveryDueNodeModeProgressGoal(
                     node, clockValue, sourceRank, owner, mode)'
                \/ HistoricalDiscoveryDueNodeOwnerAtMode(
                     node, clockValue, sourceRank, owner, mode)')
      BY HistoricalDiscoveryDueNodeModeStepPreservesOrProgresses, PTL
    <2>5. WF_AsyncAllVars(
             HistoricalDiscoveryDueNodeModeFairAction(owner, mode))
      BY <1>1,
         AsyncSpecProvidesHistoricalDiscoveryDueNodeModeFairness
    <2>6. [][AsyncNext]_AsyncAllVars
      BY <1>1 DEF AsyncSpecAt
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM AsyncSpecHistoricalDiscoveryDueIoModeMakesProgress ==
  \A initialContext,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
    AsyncSpecAt(initialContext)
      => (HistoricalDiscoveryDueIoOwnerAtMode(
            node, clockValue, sourceRank, owner, mode)
           ~> HistoricalDiscoveryDueIoModeProgressGoal(
                node, clockValue, sourceRank, owner, mode))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank \in
                  HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                NEW mode \in HistoricalDiscoveryTimedOwnerModeCarrier,
                AsyncSpecAt(initialContext)
         PROVE HistoricalDiscoveryDueIoOwnerAtMode(
                 node, clockValue, sourceRank, owner, mode)
                 ~>
               HistoricalDiscoveryDueIoModeProgressGoal(
                 node, clockValue, sourceRank, owner, mode)
    <2>1. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>2. [](HistoricalDiscoveryDueIoOwnerAtMode(
                node, clockValue, sourceRank, owner, mode)
              /\ ~HistoricalDiscoveryDueIoModeProgressGoal(
                   node, clockValue, sourceRank, owner, mode)
             => ENABLED
                  <<HistoricalDiscoveryDueIoModeFairAction(
                      owner, mode)>>_AsyncAllVars)
      BY <2>1,
         HistoricalDiscoveryDueIoModeHasEnabledExactFairAction,
         PTL
    <2>3. [](HistoricalDiscoveryDueIoOwnerAtMode(
                node, clockValue, sourceRank, owner, mode)
              /\ ~HistoricalDiscoveryDueIoModeProgressGoal(
                   node, clockValue, sourceRank, owner, mode)
              /\ <<HistoricalDiscoveryDueIoModeFairAction(
                       owner, mode)>>_AsyncAllVars
             => HistoricalDiscoveryDueIoModeProgressGoal(
                  node, clockValue, sourceRank, owner, mode)')
      BY HistoricalDiscoveryDueIoModeFairOccurrenceReachesRankGoal,
         PTL
         DEF HistoricalDiscoveryDueIoModeProgressGoal
    <2>4. [](HistoricalDiscoveryDueIoOwnerAtMode(
                node, clockValue, sourceRank, owner, mode)
              /\ ~HistoricalDiscoveryDueIoModeProgressGoal(
                   node, clockValue, sourceRank, owner, mode)
              /\ [AsyncNext]_AsyncAllVars
             => \/ HistoricalDiscoveryDueIoModeProgressGoal(
                     node, clockValue, sourceRank, owner, mode)'
                \/ HistoricalDiscoveryDueIoOwnerAtMode(
                     node, clockValue, sourceRank, owner, mode)')
      BY HistoricalDiscoveryDueIoModeStepPreservesOrProgresses, PTL
    <2>5. WF_AsyncAllVars(
             HistoricalDiscoveryDueIoModeFairAction(owner, mode))
      BY <1>1,
         AsyncSpecProvidesHistoricalDiscoveryDueIoModeFairness
    <2>6. [][AsyncNext]_AsyncAllVars
      BY <1>1 DEF AsyncSpecAt
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM AsyncSpecHistoricalDiscoveryDueNodeOwnerReachesRankGoal ==
  \A initialContext,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner:
    AsyncSpecAt(initialContext)
      => ((HistoricalDiscoveryFixedClockBlockedAtRank(
              node, clockValue, sourceRank)
            /\ OverdueResponsivePackets = {}
            /\ owner \in HistoricalDiscoveryNodeBlockersAt(clockValue))
           ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                node, clockValue, sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank \in
                  HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                AsyncSpecAt(initialContext)
         PROVE (HistoricalDiscoveryFixedClockBlockedAtRank(
                   node, clockValue, sourceRank)
                  /\ OverdueResponsivePackets = {}
                  /\ owner
                       \in HistoricalDiscoveryNodeBlockersAt(clockValue))
                 ~>
               HistoricalDiscoveryFixedClockStrictRankGoal(
                 node, clockValue, sourceRank)
    <2>1. \A mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
             HistoricalDiscoveryDueNodeOwnerAtMode(
               node, clockValue, sourceRank, owner, mode)
               ~> HistoricalDiscoveryDueNodeModeProgressGoal(
                    node, clockValue, sourceRank, owner, mode)
      BY <1>1,
         AsyncSpecHistoricalDiscoveryDueNodeModeMakesProgress
    <2>2. \A mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
             HistoricalDiscoveryDueNodeOwnerAtMode(
               node, clockValue, sourceRank, owner, mode)
               ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                    node, clockValue, sourceRank)
      BY <2>1,
         HistoricalDiscoveryTimedOwnerModeOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF HistoricalDiscoveryDueNodeModeProgressGoal
    <2>3. [](AsyncStrongTypeInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>4. (HistoricalDiscoveryFixedClockBlockedAtRank(
               node, clockValue, sourceRank)
             /\ OverdueResponsivePackets = {}
             /\ owner
                  \in HistoricalDiscoveryNodeBlockersAt(clockValue))
            ~> \E mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
                 HistoricalDiscoveryDueNodeOwnerAtMode(
                   node, clockValue, sourceRank, owner, mode)
      BY <2>3,
         HistoricalDiscoveryTimedOwnerHasFiniteMode, PTL
         DEF HistoricalDiscoveryDueNodeOwnerAtMode,
             HistoricalDiscoveryNodeBlockersAt
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

THEOREM AsyncSpecHistoricalDiscoveryDueIoOwnerReachesRankGoal ==
  \A initialContext,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier,
     owner:
    AsyncSpecAt(initialContext)
      => ((HistoricalDiscoveryFixedClockBlockedAtRank(
              node, clockValue, sourceRank)
            /\ OverdueResponsivePackets = {}
            /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
            /\ owner
                 \in HistoricalDiscoveryActiveIoBlockersAt(clockValue))
           ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                node, clockValue, sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank \in
                  HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                AsyncSpecAt(initialContext)
         PROVE (HistoricalDiscoveryFixedClockBlockedAtRank(
                   node, clockValue, sourceRank)
                  /\ OverdueResponsivePackets = {}
                  /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
                  /\ owner
                       \in HistoricalDiscoveryActiveIoBlockersAt(
                            clockValue))
                 ~>
               HistoricalDiscoveryFixedClockStrictRankGoal(
                 node, clockValue, sourceRank)
    <2>1. \A mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
             HistoricalDiscoveryDueIoOwnerAtMode(
               node, clockValue, sourceRank, owner, mode)
               ~> HistoricalDiscoveryDueIoModeProgressGoal(
                    node, clockValue, sourceRank, owner, mode)
      BY <1>1,
         AsyncSpecHistoricalDiscoveryDueIoModeMakesProgress
    <2>2. \A mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
             HistoricalDiscoveryDueIoOwnerAtMode(
               node, clockValue, sourceRank, owner, mode)
               ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                    node, clockValue, sourceRank)
      BY <2>1,
         HistoricalDiscoveryTimedOwnerModeOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF HistoricalDiscoveryDueIoModeProgressGoal
    <2>3. [](AsyncStrongTypeInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>4. (HistoricalDiscoveryFixedClockBlockedAtRank(
               node, clockValue, sourceRank)
             /\ OverdueResponsivePackets = {}
             /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
             /\ owner
                  \in HistoricalDiscoveryActiveIoBlockersAt(clockValue))
            ~> \E mode \in HistoricalDiscoveryTimedOwnerModeCarrier:
                 HistoricalDiscoveryDueIoOwnerAtMode(
                   node, clockValue, sourceRank, owner, mode)
      BY <2>3,
         HistoricalDiscoveryTimedOwnerHasFiniteMode, PTL
         DEF HistoricalDiscoveryDueIoOwnerAtMode,
             HistoricalDiscoveryActiveIoBlockersAt
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

HistoricalDiscoveryTickBlockedAtRank(
    node, clockValue, sourceRank) ==
  /\ HistoricalDiscoveryFixedClockBlockedAtRank(
       node, clockValue, sourceRank)
  /\ OverdueResponsivePackets = {}
  /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
  /\ HistoricalDiscoveryActiveIoBlockersAt(clockValue) = {}

THEOREM HistoricalDiscoveryTickBlockedHasEnabledExactTick ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    HistoricalDiscoveryTickBlockedAtRank(
      node, clockValue, sourceRank)
      => ENABLED <<AsyncTick>>_AsyncAllVars
BY HistoricalDiscoveryFixedClockBlockerCharacterization,
   AsyncTickEnabledHasConcreteSuccessor,
   ExpandENABLED, Isa
   DEF HistoricalDiscoveryTickBlockedAtRank,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       HistoricalDiscoveryFixedClockPending,
       AsyncTick, AsyncAllVars

THEOREM HistoricalDiscoveryExactTickReachesStrictRankGoal ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    /\ HistoricalDiscoveryTickBlockedAtRank(
         node, clockValue, sourceRank)
    /\ AsyncTick
    => HistoricalDiscoveryFixedClockStrictRankGoal(
         node, clockValue, sourceRank)'
BY HistoricalDiscoveryFixedClockTickReachesExit, Isa
   DEF HistoricalDiscoveryTickBlockedAtRank,
       HistoricalDiscoveryFixedClockStrictRankGoal,
       HistoricalDiscoveryFixedClockProgressExit,
       HistoricalDiscoveryFixedClockBlockedAtRank

THEOREM HistoricalDiscoveryTickStepPreservesOrProgresses ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    /\ HistoricalDiscoveryTickBlockedAtRank(
         node, clockValue, sourceRank)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDiscoveryFixedClockStrictRankGoal(
            node, clockValue, sourceRank)'
       \/ HistoricalDiscoveryTickBlockedAtRank(
            node, clockValue, sourceRank)'
BY HistoricalDiscoveryFixedClockIngressStrictlyDescends,
   HistoricalDiscoveryFixedClockLatentOwnerEntryStrictlyDescends,
   HistoricalDiscoveryFixedClockDormantHandoffStrictlyDescends,
   HistoricalDiscoveryDueNodeServiceStrictlyDescends,
   HistoricalDiscoveryDueIoServiceStrictlyDescends,
   HistoricalDiscoveryFixedClockTickReachesExit,
   HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame,
   HistoricalDiscoveryBroadcastControlHelpersHaveFixedClockFrame,
   HistoricalDiscoveryRetransmissionHelpersHaveFixedClockFrame,
   HistoricalDiscoveryDirectRequestPublicationHasFixedClockFrame,
   HistoricalDiscoveryResponsePublicationHasFixedClockFrame,
   HistoricalDiscoveryByzantineCertifiedRequestHasFixedClockFrame,
   HistoricalDiscoverySingletonFaultInjectorsHaveFixedClockFrame,
   AsyncBracketNextPreservesStrongTypeInvariant,
   IsaT(1200)
   DEF HistoricalDiscoveryTickBlockedAtRank,
       HistoricalDiscoveryFixedClockStrictRankGoal,
       HistoricalDiscoveryFixedClockBlockedAtRank,
       HistoricalDiscoveryFixedClockPublicationFrame,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncAllVars

THEOREM AsyncSpecHistoricalDiscoveryTickReachesRankGoal ==
  \A initialContext,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    AsyncSpecAt(initialContext)
      => (HistoricalDiscoveryTickBlockedAtRank(
            node, clockValue, sourceRank)
           ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                node, clockValue, sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank \in
                  HistoricalDiscoveryFixedClockBlockerCarrier,
                AsyncSpecAt(initialContext)
         PROVE HistoricalDiscoveryTickBlockedAtRank(
                 node, clockValue, sourceRank)
                 ~>
               HistoricalDiscoveryFixedClockStrictRankGoal(
                 node, clockValue, sourceRank)
    <2>1. [](HistoricalDiscoveryTickBlockedAtRank(
                node, clockValue, sourceRank)
              /\ ~HistoricalDiscoveryFixedClockStrictRankGoal(
                   node, clockValue, sourceRank)
             => ENABLED <<AsyncTick>>_AsyncAllVars)
      BY HistoricalDiscoveryTickBlockedHasEnabledExactTick, PTL
    <2>2. [](HistoricalDiscoveryTickBlockedAtRank(
                node, clockValue, sourceRank)
              /\ ~HistoricalDiscoveryFixedClockStrictRankGoal(
                   node, clockValue, sourceRank)
              /\ <<AsyncTick>>_AsyncAllVars
             => HistoricalDiscoveryFixedClockStrictRankGoal(
                  node, clockValue, sourceRank)')
      BY HistoricalDiscoveryExactTickReachesStrictRankGoal, PTL
         DEF AsyncTick
    <2>3. [](HistoricalDiscoveryTickBlockedAtRank(
                node, clockValue, sourceRank)
              /\ ~HistoricalDiscoveryFixedClockStrictRankGoal(
                   node, clockValue, sourceRank)
              /\ [AsyncNext]_AsyncAllVars
             => \/ HistoricalDiscoveryFixedClockStrictRankGoal(
                     node, clockValue, sourceRank)'
                \/ HistoricalDiscoveryTickBlockedAtRank(
                     node, clockValue, sourceRank)')
      BY HistoricalDiscoveryTickStepPreservesOrProgresses, PTL
    <2>4. WF_AsyncAllVars(AsyncTick)
      BY <1>1 DEF AsyncSpecAt, AsyncFairnessAt
    <2>5. [][AsyncNext]_AsyncAllVars
      BY <1>1 DEF AsyncSpecAt
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

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
cannot hold trivially in the source state: `known` is exactly the then-covered
identity set, whereas the residual requires a covered identity outside
`known`.  Packet service and the Candidate/Serve budget property are exported
interfaces; both are derived below from the exact physical action kernels and
the finite frozen semantic universe.  Durable lifecycle markers cannot be
replaced by fairness of replenishment or by the infinite global candidate
type carrier.
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

THEOREM AsyncSpecClosesHistoricalDiscoveryFixedClockNonPacketService ==
  \A initialContext:
    HistoricalDiscoveryFixedClockNonPacketServiceProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE HistoricalDiscoveryFixedClockNonPacketServiceProperty(
                 AsyncSpecAt(initialContext))
    <2>1. CASE AsyncSpecAt(initialContext)
      <3>1. ASSUME NEW node \in Responsive,
                    NEW clockValue \in Nat,
                    NEW sourceRank \in
                      HistoricalDiscoveryFixedClockBlockerCarrier
             PROVE (HistoricalDiscoveryFixedClockBlockedAtRank(
                       node, clockValue, sourceRank)
                      /\ OverdueResponsivePackets = {})
                     ~>
                   HistoricalDiscoveryFixedClockStrictRankGoal(
                     node, clockValue, sourceRank)
        <4>1. \A owner:
                 (HistoricalDiscoveryFixedClockBlockedAtRank(
                    node, clockValue, sourceRank)
                   /\ OverdueResponsivePackets = {}
                   /\ owner
                        \in HistoricalDiscoveryNodeBlockersAt(clockValue))
                  ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                       node, clockValue, sourceRank)
          BY <2>1,
             AsyncSpecHistoricalDiscoveryDueNodeOwnerReachesRankGoal
        <4>2. \A owner:
                 (HistoricalDiscoveryFixedClockBlockedAtRank(
                    node, clockValue, sourceRank)
                   /\ OverdueResponsivePackets = {}
                   /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
                   /\ owner
                        \in HistoricalDiscoveryActiveIoBlockersAt(
                             clockValue))
                  ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                       node, clockValue, sourceRank)
          BY <2>1,
             AsyncSpecHistoricalDiscoveryDueIoOwnerReachesRankGoal
        <4>3. HistoricalDiscoveryTickBlockedAtRank(
                 node, clockValue, sourceRank)
                ~>
              HistoricalDiscoveryFixedClockStrictRankGoal(
                node, clockValue, sourceRank)
          BY <2>1,
             AsyncSpecHistoricalDiscoveryTickReachesRankGoal
        <4>4. (HistoricalDiscoveryFixedClockBlockedAtRank(
                  node, clockValue, sourceRank)
                 /\ OverdueResponsivePackets = {})
                ~>
              (HistoricalDiscoveryFixedClockStrictRankGoal(
                 node, clockValue, sourceRank)
               \/ \E owner:
                    /\ HistoricalDiscoveryFixedClockBlockedAtRank(
                         node, clockValue, sourceRank)
                    /\ OverdueResponsivePackets = {}
                    /\ owner
                         \in HistoricalDiscoveryNodeBlockersAt(
                              clockValue)
               \/ \E owner:
                    /\ HistoricalDiscoveryFixedClockBlockedAtRank(
                         node, clockValue, sourceRank)
                    /\ OverdueResponsivePackets = {}
                    /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
                    /\ owner
                         \in HistoricalDiscoveryActiveIoBlockersAt(
                              clockValue)
               \/ HistoricalDiscoveryTickBlockedAtRank(
                    node, clockValue, sourceRank))
          BY Isa, PTL
             DEF HistoricalDiscoveryTickBlockedAtRank
        <4> QED BY <4>1, <4>2, <4>3, <4>4, PTL
      <3> QED BY <3>1
    <2>2. CASE ~AsyncSpecAt(initialContext)
      BY <2>2
         DEF HistoricalDiscoveryFixedClockNonPacketServiceProperty
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

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

HistoricalDiscoveryCandidateServeLifecycleGoal(
    node, clockValue, sourceRank, packet, known, budget) ==
  \/ HistoricalDiscoveryFixedClockStrictRankGoal(
       node, clockValue, sourceRank)
  \/ HistoricalDiscoveryCandidateServeLifecycleDiscovery(
       node, clockValue, sourceRank, packet, known, budget)

HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty(
    specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
               node, clockValue, sourceRank, packet, known, budget)
               ~> HistoricalDiscoveryCandidateServeLifecycleGoal(
                    node, clockValue, sourceRank,
                    packet, known, budget)

(***************************************************************************
Exact physical packet and retained-lifecycle kernels.

The former lifecycle property hid three different schedulers.  The selected
packet can still be below logical Candidate/Serve admission, the exact
minimum Candidate occurrence can be owned by the historical runner, or the
exact minimum Serve occurrence can be owned by one of the two I/O workers.
The definitions below retain the packet, frozen known set, budget, immutable
logical identity, and exact occurrence rank.

`HistoricalDiscoveryPacketConcreteAction` is an indexed family, not an
existential action union.  Its members are the exact local projections of
`IndexedAdmitPacketStep`, `IndexedAdmitHistoricalRecoveryPacketStep`,
`IndexedRunNodeStep`, `IndexedRunHistoricalRecoveryStep`,
`IndexedHistoricalServerStep`, `IndexedIoWorkerStep`, and
`IndexedHistoricalRecoveryIoWorkerStep`, respectively.  The residual
properties quantify one fixed member at a time, so no weak fairness of their
disjunction is assumed.
***************************************************************************)

HistoricalDiscoveryPacketConcreteActionKindCarrier ==
  {"Admit", "AdmitHistorical",
   "RunNode", "RunHistoricalRecovery", "RunHistoricalServer",
   "ServiceIo", "ServiceHistoricalIo"}

HistoricalDiscoveryPacketConcreteAction(packet, actionKind) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
  IN CASE actionKind = "Admit" ->
            PostGstAdmitHiddenPacket(recipient, source)
       [] actionKind = "AdmitHistorical" ->
            PostGstAdmitHistoricalRecoveryPacket(recipient, source)
       [] actionKind = "RunNode" ->
            PostGstRunNode(recipient)
       [] actionKind = "RunHistoricalRecovery" ->
            PostGstRunHistoricalRecoveryNode(recipient)
       [] actionKind = "RunHistoricalServer" ->
            PostGstRunHistoricalServer(recipient)
       [] actionKind = "ServiceIo" ->
            PostGstServiceIoWorker(recipient)
       [] actionKind = "ServiceHistoricalIo" ->
            PostGstServiceHistoricalRecoveryIoWorker(recipient)
       [] OTHER -> FALSE

HistoricalDiscoveryPacketConcreteActionPending(
    node, clockValue, sourceRank, packet, known, budget,
    dependencyRank, actionKind) ==
  /\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
       node, clockValue, sourceRank, packet, known, budget)
  /\ HistoricalDiscoveryPacketProducerIdentitySet(packet) = {}
  /\ dependencyRank = HistoricalDiscoveryPacketDependencyRank(packet)
  /\ dependencyRank \in HistoricalDiscoveryPacketDependencyCarrier
  /\ actionKind \in HistoricalDiscoveryPacketConcreteActionKindCarrier
  /\ ENABLED
       HistoricalDiscoveryPacketConcreteAction(packet, actionKind)

(***************************************************************************
Frozen exact-action witness.

This is the action-valued form of the overdue-packet deadlock case split.
Unlike `OverdueResponsivePacketEnablesConcreteCorridorProgress`, it does not
forget which physical action is enabled behind an existential productive
step.  The witness is exported as an ordinary quantified value so a temporal
consumer can retain it across a handoff.  No `CHOOSE` is evaluated again in a
successor state.
***************************************************************************)

THEOREM HistoricalDiscoveryPacketTailHasFrozenConcreteAction ==
  \A initialContext \in ContextRecords:
    \A node \in Responsive,
       clockValue \in Nat,
       sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
      \A packet, known, budget:
        /\ AsyncFrozenContextAt(initialContext)
        /\ PostGstReplayQuarantineExcluded
        /\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
             node, clockValue, sourceRank, packet, known, budget)
        /\ HistoricalDiscoveryPacketProducerIdentitySet(packet) = {}
        => \E dependencyRank
                 \in HistoricalDiscoveryPacketDependencyCarrier:
             \E actionKind
                  \in HistoricalDiscoveryPacketConcreteActionKindCarrier:
               HistoricalDiscoveryPacketConcreteActionPending(
                 node, clockValue, sourceRank, packet, known, budget,
                 dependencyRank, actionKind)
PROOF
  <1>1. ASSUME NEW initialContext \in ContextRecords,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank
                  \in HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW packet, NEW known, NEW budget,
                AsyncFrozenContextAt(initialContext),
                PostGstReplayQuarantineExcluded,
                HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
                  node, clockValue, sourceRank, packet, known, budget),
                HistoricalDiscoveryPacketProducerIdentitySet(packet) = {}
         PROVE \E dependencyRank
                   \in HistoricalDiscoveryPacketDependencyCarrier:
                 \E actionKind
                      \in HistoricalDiscoveryPacketConcreteActionKindCarrier:
                   HistoricalDiscoveryPacketConcreteActionPending(
                     node, clockValue, sourceRank, packet, known, budget,
                     dependencyRank, actionKind)
    <2> DEFINE Recipient == packet.item.envelope.recipient
    <2> DEFINE Source == packet.item.source
    <2> DEFINE Item == OldestDueSourcePacket(Recipient, Source).item
    <2>1. /\ AsyncStrongTypeInvariant
           /\ gst
           /\ packet \in OverdueResponsivePackets
           /\ packet = HistoricalDiscoverySelectedOverduePacket
      BY <1>1
         DEF HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
             HistoricalDiscoveryFixedClockPending
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
    <2>5. HistoricalDiscoveryPacketDependencyRank(packet)
             \in HistoricalDiscoveryPacketDependencyCarrier
      BY <2>1, HistoricalDiscoveryPacketDependencyRankInCarrier
    <2>6. CASE DueIngressPacketCanEnter(Recipient, Source)
      <3>1. ENABLED
               (PostGstAdmitHiddenPacket(Recipient, Source)
                  \/ PostGstAdmitHistoricalRecoveryPacket(
                       Recipient, Source))
        BY <1>1, <2>1, <2>3, <2>6,
           DueIngressPacketAdmissionIsEnabled
      <3> QED BY <1>1, <2>5, <3>1, ExpandENABLED, Isa
           DEF HistoricalDiscoveryPacketConcreteActionPending,
               HistoricalDiscoveryPacketConcreteAction,
               HistoricalDiscoveryPacketConcreteActionKindCarrier,
               Recipient, Source
    <2>7. CASE ~DueIngressPacketCanEnter(Recipient, Source)
      <3>1. CASE ~NodeHasApplication(Recipient)
        <4>1. CASE Recipient \in asyncHistoricalRecoveryTargets
          <5>1. ENABLED
                   PostGstRunHistoricalRecoveryNode(Recipient)
            BY <2>1, <4>1,
               GstHistoricalRecoveryRunNodeIsEnabled
          <5> QED BY <1>1, <2>5, <5>1, Isa
               DEF HistoricalDiscoveryPacketConcreteActionPending,
                   HistoricalDiscoveryPacketConcreteAction,
                   HistoricalDiscoveryPacketConcreteActionKindCarrier,
                   Recipient
        <4>2. CASE Recipient \notin asyncHistoricalRecoveryTargets
          <5>1. Recipient \in AsyncCurrentResponsiveVoters
            BY <2>3, <4>2
          <5>2. ENABLED PostGstRunNode(Recipient)
            BY <2>1, <3>1, <5>1,
               GstResponsiveUnappliedRunNodeIsEnabled
          <5> QED BY <1>1, <2>5, <5>2, Isa
               DEF HistoricalDiscoveryPacketConcreteActionPending,
                   HistoricalDiscoveryPacketConcreteAction,
                   HistoricalDiscoveryPacketConcreteActionKindCarrier,
                   Recipient
        <4> QED BY <4>1, <4>2
      <3>2. CASE NodeHasApplication(Recipient)
        <4>1. Recipient \in AsyncCurrentResponsiveVoters
          BY <2>2, <2>3, <3>2, Isa
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncHistoricalRecoveryTypeInvariant
        <4>2. IngressDepth(Recipient) > 0
          BY <2>2, <2>3, <2>4, <2>7,
             EmptyIngressAdmitsTypedPacket,
             ZeroIngressDepthMeansEveryLaneEmpty, Isa
             DEF DueIngressPacketCanEnter,
                 DueIngressPacketCanCoalesce,
                 IngressHasCoalescingOwner,
                 IngressLane, SequenceSet, Item
        <4>3. CASE HistoricalDrainableIngressIndices(Recipient) # {}
          <5>1. ENABLED PostGstRunHistoricalServer(Recipient)
            BY <1>1, <2>1, <3>2, <4>1, <4>3,
               AppliedResponsiveHistoricalServerEnabledAfterGst
          <5> QED BY <1>1, <2>5, <5>1, Isa
               DEF HistoricalDiscoveryPacketConcreteActionPending,
                   HistoricalDiscoveryPacketConcreteAction,
                   HistoricalDiscoveryPacketConcreteActionKindCarrier,
                   Recipient
        <4>4. CASE HistoricalDrainableIngressIndices(Recipient) = {}
          <5>1. AsyncIoQueueDepth(Recipient) > 0
            BY <2>2, <2>3, <4>2, <4>4,
               NonemptyUndrainableHistoricalIngressHasIoWork
          <5>2. Recipient \in AsyncArchiveIoServiceNodes
            BY <2>1, <3>2, <4>1, Isa
               DEF AsyncArchiveIoServiceNodes,
                   AsyncResponsiveAppliedArchiveServers,
                   AsyncResponsiveOnlineArchiveServers,
                   AsyncResponsiveArchiveServers
          <5>3. ENABLED PostGstServiceIoWorker(Recipient)
            BY <2>1, <5>1, <5>2,
               AsyncStrongTypeProjectsAsyncType,
               QueuedIoEnablesPostGstService
          <5> QED BY <1>1, <2>5, <5>3, Isa
               DEF HistoricalDiscoveryPacketConcreteActionPending,
                   HistoricalDiscoveryPacketConcreteAction,
                   HistoricalDiscoveryPacketConcreteActionKindCarrier,
                   Recipient
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

HistoricalDiscoveryCandidateExactPhysicalIdentity(candidate) ==
  [exactCandidate |-> ExactAsyncCandidateIdentity(candidate),
   serviceIdentity |-> AsyncCandidateServiceIdentity(candidate),
   lifecycleStage |->
     AsyncCandidateServiceStageForKind(candidate.kind),
   lifecycleOrdinal |-> AsyncCandidateLifecycleOrdinal(candidate)]

HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet(packet) ==
  {HistoricalDiscoveryCandidateExactPhysicalIdentity(candidate):
     candidate \in HistoricalDiscoveryPacketCandidateOwners(packet)}

HistoricalDiscoveryCandidateExactPhysicalRankCarrier ==
  HistoricalDiscoveryOccurrenceDebtCarrier
    \X (OwnedServiceRankCarrier \X Nat)

HistoricalDiscoveryCandidateExactPhysicalRank(packet, candidate) ==
  <<HistoricalDiscoveryPacketCandidateOccurrenceDebtRank(packet),
    <<CandidateServiceRank(candidate),
      AsyncCandidateLifecycleOrdinal(candidate)>>>

HistoricalDiscoveryCandidateFrozenPhysicalCoordinates(
    identity, candidate, occurrenceRank) ==
  /\ identity.exactCandidate = ExactAsyncCandidateIdentity(candidate)
  /\ identity.serviceIdentity = AsyncCandidateServiceIdentity(candidate)
  /\ identity.lifecycleStage =
       AsyncCandidateServiceStageForKind(candidate.kind)
  /\ identity.lifecycleOrdinal = occurrenceRank[2][2]

HistoricalDiscoveryCandidateExactActionOwnerAtRank(
    node, clockValue, sourceRank, packet, known, budget,
    identity, candidate, occurrenceRank, physicalKnown) ==
  LET recipient == packet.item.envelope.recipient
  IN /\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
          node, clockValue, sourceRank, packet, known, budget)
     /\ HistoricalDiscoveryPacketCandidateOwners(packet) # {}
     /\ candidate =
          HistoricalDiscoveryPacketCandidateDebtWitness(packet)
     /\ physicalKnown =
          HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet(
            packet)
     /\ IsFiniteSet(physicalKnown)
     /\ identity =
          HistoricalDiscoveryCandidateExactPhysicalIdentity(candidate)
     /\ <<"Candidate", candidate.causalOrigin>>
          \in HistoricalDiscoveryPacketFrozenKnownIdentitySet(
               packet, known)
     /\ occurrenceRank =
          HistoricalDiscoveryCandidateExactPhysicalRank(packet, candidate)
     /\ occurrenceRank
          \in HistoricalDiscoveryCandidateExactPhysicalRankCarrier
     /\ HistoricalDiscoveryCandidateFrozenPhysicalCoordinates(
          identity, candidate, occurrenceRank)
     /\ candidate.node = recipient
     /\ ENABLED PostGstRunHistoricalRecoveryNode(recipient)

HistoricalDiscoveryCandidateIntroducedPhysicalIdentitySet(
    packet, physicalKnown) ==
  HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet(packet)
    \ physicalKnown

(***************************************************************************
Candidate runner non-descent split.

The source freezes the complete candidate value, its route-neutral service
identity, projected lifecycle stage, immutable lifecycle admission ordinal,
minimum service rank, and the complete live physical-identity set.  A runner
step which does not reach the fixed-clock goal can therefore be classified as
an equal-count owner replacement or a count-increasing replenishment.  Both
arms require a newly exposed exact physical identity and neither is progress.

`SumeragiV2AsyncCausalWorkBudgetProofs` audits the closed
`CommandSuccessors` graph with an exact 9-to-0 remaining-work rank.  Every
parent has at most three children and every child is strictly lower, so its
radix-four weight leaves room for the complete successor batch while
strictly consuming a natural-number budget.  The eleven adapter stages remain
part of the physical identity/tombstone bridge; they are not misused as a
rank for internal Persist/Sign commands.

The transition-level statement that every same-origin replacement either
reaches the fixed-clock goal or strictly consumes this token budget remains a
named seam.  It needs the existing lifecycle-stage tombstones, ordinal
retention, exact retry coalescing, and the complete producer-action audit.
The definitions below never turn child creation itself into progress.
***************************************************************************)

HistoricalDiscoveryCandidateCausalWorkTokenSet(packet) ==
  {<<HistoricalDiscoveryCandidateExactPhysicalIdentity(candidate), token>>:
     candidate \in HistoricalDiscoveryPacketCandidateOwners(packet),
     token \in
       1..AsyncCausalRemainingWorkWeight(candidate.kind)}

HistoricalDiscoveryCandidateCausalWorkBudget(packet) ==
  Cardinality(HistoricalDiscoveryCandidateCausalWorkTokenSet(packet))

THEOREM HistoricalDiscoveryCandidateCausalWorkBudgetIsNatural ==
  \A packet \in OverdueResponsivePackets:
    AsyncStrongTypeInvariant
      => HistoricalDiscoveryCandidateCausalWorkBudget(packet) \in Nat
BY AsyncCausalRemainingWorkWeightIsPositive,
   StrongTypeHasFiniteHistoricalDiscoveryRankOwners,
   FS_Image, FS_Product, FS_Subset, FS_Interval,
   FS_CardinalityType, IsaT(240)
   DEF HistoricalDiscoveryCandidateCausalWorkBudget,
       HistoricalDiscoveryCandidateCausalWorkTokenSet,
       HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet,
       HistoricalDiscoveryPacketCandidateOwners

HistoricalDiscoveryCandidateEqualCountOwnerReplacementResidual(
    node, clockValue, sourceRank, packet, known, budget,
    identity, candidate, occurrenceRank, physicalKnown) ==
  /\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
       node, clockValue, sourceRank, packet, known, budget)
  /\ ~HistoricalDiscoveryCandidateServeLifecycleGoal(
       node, clockValue, sourceRank, packet, known, budget)
  /\ occurrenceRank
       \in HistoricalDiscoveryCandidateExactPhysicalRankCarrier
  /\ HistoricalDiscoveryCandidateFrozenPhysicalCoordinates(
       identity, candidate, occurrenceRank)
  /\ IsFiniteSet(physicalKnown)
  /\ Cardinality(HistoricalDiscoveryPacketCandidateOwners(packet))
       = occurrenceRank[1][1]
  /\ HistoricalDiscoveryCandidateIntroducedPhysicalIdentitySet(
       packet, physicalKnown) # {}

HistoricalDiscoveryCandidateCountIncreasingReplenishmentResidual(
    node, clockValue, sourceRank, packet, known, budget,
    identity, candidate, occurrenceRank, physicalKnown) ==
  /\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
       node, clockValue, sourceRank, packet, known, budget)
  /\ ~HistoricalDiscoveryCandidateServeLifecycleGoal(
       node, clockValue, sourceRank, packet, known, budget)
  /\ occurrenceRank
       \in HistoricalDiscoveryCandidateExactPhysicalRankCarrier
  /\ HistoricalDiscoveryCandidateFrozenPhysicalCoordinates(
       identity, candidate, occurrenceRank)
  /\ IsFiniteSet(physicalKnown)
  /\ Cardinality(HistoricalDiscoveryPacketCandidateOwners(packet))
       > occurrenceRank[1][1]
  /\ HistoricalDiscoveryCandidateIntroducedPhysicalIdentitySet(
       packet, physicalKnown) # {}

HistoricalDiscoveryCandidateNonDescentEpisodeResidual(
    node, clockValue, sourceRank, packet, known, budget,
    identity, candidate, occurrenceRank, physicalKnown) ==
  \/ HistoricalDiscoveryCandidateEqualCountOwnerReplacementResidual(
       node, clockValue, sourceRank, packet, known, budget,
       identity, candidate, occurrenceRank, physicalKnown)
  \/ HistoricalDiscoveryCandidateCountIncreasingReplenishmentResidual(
       node, clockValue, sourceRank, packet, known, budget,
       identity, candidate, occurrenceRank, physicalKnown)

HistoricalDiscoveryCandidateCausalDagBudgetFrontier(
    node, clockValue, sourceRank, packet, known, budget,
    identity, candidate, occurrenceRank, physicalKnown, workBudget) ==
  /\ HistoricalDiscoveryCandidateNonDescentEpisodeResidual(
       node, clockValue, sourceRank, packet, known, budget,
       identity, candidate, occurrenceRank, physicalKnown)
  /\ workBudget \in Nat
  /\ workBudget =
       HistoricalDiscoveryCandidateCausalWorkBudget(packet)

HistoricalDiscoveryCandidateStrictCausalDagBudgetGoal(
    node, clockValue, sourceRank, packet, known, budget,
    identity, candidate, occurrenceRank, physicalKnown, workBudget) ==
  \/ HistoricalDiscoveryCandidateServeLifecycleGoal(
       node, clockValue, sourceRank, packet, known, budget)
  \/ \E lowerWorkBudget \in
       SetLessThan(workBudget, OpToRel(<, Nat), Nat):
       HistoricalDiscoveryCandidateCausalDagBudgetFrontier(
         node, clockValue, sourceRank, packet, known, budget,
         identity, candidate, occurrenceRank,
         physicalKnown, lowerWorkBudget)

THEOREM HistoricalDiscoveryCandidateNonDescentStartsCausalDagBudget ==
  \A node, clockValue, sourceRank, packet, known, budget,
     identity, candidate, occurrenceRank, physicalKnown:
    HistoricalDiscoveryCandidateNonDescentEpisodeResidual(
      node, clockValue, sourceRank, packet, known, budget,
      identity, candidate, occurrenceRank, physicalKnown)
      => \E workBudget \in Nat:
           HistoricalDiscoveryCandidateCausalDagBudgetFrontier(
             node, clockValue, sourceRank, packet, known, budget,
             identity, candidate, occurrenceRank,
             physicalKnown, workBudget)
BY HistoricalDiscoveryCandidateCausalWorkBudgetIsNatural, Isa
   DEF HistoricalDiscoveryCandidateNonDescentEpisodeResidual,
       HistoricalDiscoveryCandidateEqualCountOwnerReplacementResidual,
       HistoricalDiscoveryCandidateCountIncreasingReplenishmentResidual,
       HistoricalDiscoveryCandidateCausalDagBudgetFrontier,
       HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
       HistoricalDiscoveryFixedClockPending

HistoricalDiscoveryServeExactWorkerActionKindCarrier ==
  {"ServiceIo", "ServiceHistoricalIo"}

HistoricalDiscoveryServeExactWorkerModeCarrier == 0..1

HistoricalDiscoveryServeExactWorkerModeOrdering ==
  OpToRel(<, HistoricalDiscoveryServeExactWorkerModeCarrier)

HistoricalDiscoveryServeExactWorkerKindForMode(workerMode) ==
  IF workerMode = 1
  THEN "ServiceHistoricalIo"
  ELSE "ServiceIo"

HistoricalDiscoveryServeExactWorkerAction(packet, workerKind) ==
  LET recipient == packet.item.envelope.recipient
  IN CASE workerKind = "ServiceIo" ->
            PostGstServiceIoWorker(recipient)
       [] workerKind = "ServiceHistoricalIo" ->
            PostGstServiceHistoricalRecoveryIoWorker(recipient)
       [] OTHER -> FALSE

HistoricalDiscoveryServeExactActionOwnerAtRank(
    node, clockValue, sourceRank, packet, known, budget,
    identity, job, occurrenceRank, workerKind, workerMode) ==
  LET recipient == packet.item.envelope.recipient
  IN /\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
          node, clockValue, sourceRank, packet, known, budget)
     /\ HistoricalDiscoveryPacketCandidateOwners(packet) = {}
     /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
     /\ job = HistoricalDiscoveryPacketServeDebtWitness(packet)
     /\ identity = AsyncIoServeJobIdentity(recipient, job)
     /\ <<"Serve", identity>>
          \in HistoricalDiscoveryPacketFrozenKnownIdentitySet(
               packet, known)
     /\ occurrenceRank =
          HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)
     /\ occurrenceRank \in HistoricalDiscoveryOccurrenceDebtCarrier
     /\ workerMode \in
          HistoricalDiscoveryServeExactWorkerModeCarrier
     /\ workerMode = HistoricalDiscoveryTimedOwnerMode(recipient)
     /\ workerKind \in
          HistoricalDiscoveryServeExactWorkerActionKindCarrier
     /\ workerKind =
          HistoricalDiscoveryServeExactWorkerKindForMode(workerMode)
     /\ ENABLED
          HistoricalDiscoveryServeExactWorkerAction(packet, workerKind)

HistoricalDiscoveryServeExactWorkerModeFrontier(
    node, clockValue, sourceRank, packet, known, budget,
    identity, job, occurrenceRank, workerMode) ==
  \E workerKind \in
       HistoricalDiscoveryServeExactWorkerActionKindCarrier:
    HistoricalDiscoveryServeExactActionOwnerAtRank(
      node, clockValue, sourceRank, packet, known, budget,
      identity, job, occurrenceRank, workerKind, workerMode)

HistoricalDiscoveryServeExactWorkerModeHandoffResidual(
    node, clockValue, sourceRank, packet, known, budget,
    identity, job, occurrenceRank, workerMode) ==
  /\ ~HistoricalDiscoveryCandidateServeLifecycleGoal(
       node, clockValue, sourceRank, packet, known, budget)
  /\ \E lowerMode \in
       SetLessThan(
         workerMode,
         HistoricalDiscoveryServeExactWorkerModeOrdering,
         HistoricalDiscoveryServeExactWorkerModeCarrier):
       HistoricalDiscoveryServeExactWorkerModeFrontier(
         node, clockValue, sourceRank, packet, known, budget,
         identity, job, occurrenceRank, lowerMode)

HistoricalDiscoveryServeExactWorkerModeProgressGoal(
    node, clockValue, sourceRank, packet, known, budget,
    identity, job, occurrenceRank, workerMode) ==
  \/ HistoricalDiscoveryCandidateServeLifecycleGoal(
       node, clockValue, sourceRank, packet, known, budget)
  \/ HistoricalDiscoveryServeExactWorkerModeHandoffResidual(
       node, clockValue, sourceRank, packet, known, budget,
       identity, job, occurrenceRank, workerMode)

THEOREM HistoricalDiscoveryServeExactWorkerModeOrderingIsWellFounded ==
  IsWellFoundedOn(
    HistoricalDiscoveryServeExactWorkerModeOrdering,
    HistoricalDiscoveryServeExactWorkerModeCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset
   DEF HistoricalDiscoveryServeExactWorkerModeOrdering,
       HistoricalDiscoveryServeExactWorkerModeCarrier

(***************************************************************************
The fixed Candidate identity cannot return behind the retained owner after
service.  These are direct specializations of the stable generic lifecycle
providers in `AsyncNetwork`; no adequate-leader theorem is imported.
***************************************************************************)

THEOREM HistoricalDiscoverySameGenerationCandidateServiceBlocksReentry ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCandidateTransientServiceActive(candidate)
    /\ candidate.consumerGeneration = generation[candidate.node]
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    /\ ~AsyncCandidateTransientMarkerExitThisStep(candidate)
    => /\ AsyncCandidateServiceTombstoned(candidate)'
       /\ ~CandidateScheduled(candidate)'
BY AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst,
   AsyncCandidateSameGenerationSuccessfulServiceIdentityPersistsUntilStrictExit,
   Isa
   DEF AsyncCandidateTransientServiceActive,
       AsyncCandidateServiceTombstoned,
       AsyncCandidateServiceCoalesced

THEOREM HistoricalDiscoveryServicedCandidateIdentityBlocksReentry ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncCandidateTransientServiceActive(candidate)
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    /\ ~AsyncCandidateServiceExitThisStep(candidate)
    => /\ AsyncCandidateTransientServiceActive(candidate)'
       /\ ~CandidateScheduled(candidate)'
BY AsyncCandidateServicedIdentityCannotReactivate

THEOREM HistoricalDiscoveryCandidateDepartureRetainsLifecycleCoverage ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ gst
    /\ AsyncNext
    /\ CandidateScheduled(candidate)
    /\ ~CandidateScheduledAfter(candidate)
    => \/ AsyncCandidateIgnoredWithoutApplicationThisStep(candidate)
       \/ AsyncCandidateServiceTombstoned(candidate)'
       \/ AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter(candidate)
       \/ AsyncCandidateMonotoneSemanticCoverageAfterIn(
            asyncControlServiceState', candidate)
       \/ AsyncCandidateTerminalTombstoned(candidate)'
BY AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst

THEOREM HistoricalDiscoveryRetiredServeIdentityBlocksReentry ==
  \A recipient \in ValidatorIds,
     identity \in AsyncServeLogicalRequestIdentities:
    /\ AsyncServeLifecycleTypeInvariant
    /\ AsyncServeLogicalIdentityRetiredOrSuperseded(
         recipient, identity)
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => /\ AsyncServeLogicalIdentityRetiredOrSuperseded(
            recipient, identity)'
       /\ ~AsyncServeJobQueued(recipient, identity)'
BY AsyncServeRetiredIdentityCannotRequeueAtGst

THEOREM HistoricalDiscoveryServeDepartureInstallsDurableCoverage ==
  \A recipient \in ValidatorIds,
     identity \in AsyncServeLogicalRequestIdentities:
    /\ AsyncServeLifecycleTypeInvariant
    /\ gst
    /\ AsyncServeJobQueued(recipient, identity)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~AsyncServeJobQueued(recipient, identity)'
    => AsyncServeLifecycleTombstone(recipient, identity)'
BY AsyncServeQueuedIdentityDepartureInstallsTombstone

THEOREM HistoricalDiscoveryLiveCandidateHasExactActionOwner ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known, budget:
      LET candidate ==
            HistoricalDiscoveryPacketCandidateDebtWitness(packet)
      IN /\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
              node, clockValue, sourceRank, packet, known, budget)
         /\ HistoricalDiscoveryPacketCandidateOwners(packet) # {}
         /\ <<"Candidate", candidate.causalOrigin>>
              \in HistoricalDiscoveryPacketFrozenKnownIdentitySet(
                   packet, known)
         => \E identity, exactCandidate, occurrenceRank, physicalKnown:
              HistoricalDiscoveryCandidateExactActionOwnerAtRank(
                node, clockValue, sourceRank, packet, known, budget,
                identity, exactCandidate, occurrenceRank, physicalKnown)
BY HistoricalDiscoveryLiveCandidateDebtHasExactFairOwner,
   HistoricalDiscoveryPacketOccurrenceDebtRanksInCarrier,
   ScheduledCandidateServiceRankInCarrier,
   GstHistoricalRecoveryRunNodeIsEnabled,
   FS_Image, Isa
   DEF HistoricalDiscoveryCandidateExactActionOwnerAtRank,
       HistoricalDiscoveryCandidateExactPhysicalRank,
       HistoricalDiscoveryCandidateExactPhysicalRankCarrier,
       HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet,
       HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
       HistoricalDiscoveryFixedClockPending,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncControlServiceStateTypeInvariant,
       AsyncCandidateLifecycleOrdinal,
       AsyncCandidateLifecycleRecordsFor,
       HistoricalRecoveryTarget

THEOREM HistoricalDiscoveryLiveServeHasExactActionOwner ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known, budget:
      /\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
           node, clockValue, sourceRank, packet, known, budget)
      /\ HistoricalDiscoveryPacketCandidateOwners(packet) = {}
      /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
      /\ <<"Serve",
            AsyncIoServeJobIdentity(
              packet.item.envelope.recipient,
              HistoricalDiscoveryPacketServeDebtWitness(packet))>>
           \in HistoricalDiscoveryPacketFrozenKnownIdentitySet(
                packet, known)
      => \E identity, job, occurrenceRank, workerKind, workerMode:
           HistoricalDiscoveryServeExactActionOwnerAtRank(
             node, clockValue, sourceRank, packet, known, budget,
             identity, job, occurrenceRank, workerKind, workerMode)
BY HistoricalDiscoveryLiveServeDebtHasExactFairOwner,
   HistoricalDiscoveryPacketOccurrenceDebtRanksInCarrier,
   GstHistoricalIoWorkerIsEnabled,
   QueuedIoEnablesPostGstService,
   Isa
   DEF HistoricalDiscoveryServeExactActionOwnerAtRank,
       HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
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
       HistoricalRecoveryTarget

THEOREM HistoricalDiscoveryEpisodeHasKnownExactOwnerOrPacketTail ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known, budget:
      HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
        node, clockValue, sourceRank, packet, known, budget)
        => \/ HistoricalDiscoveryCandidateServeLifecycleGoal(
                node, clockValue, sourceRank, packet, known, budget)
           \/ /\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
                   node, clockValue, sourceRank, packet, known, budget)
              /\ HistoricalDiscoveryPacketProducerIdentitySet(packet) = {}
           \/ \E identity, candidate, occurrenceRank, physicalKnown:
                HistoricalDiscoveryCandidateExactActionOwnerAtRank(
                  node, clockValue, sourceRank, packet, known, budget,
                  identity, candidate, occurrenceRank, physicalKnown)
           \/ \E identity, job, occurrenceRank, workerKind, workerMode:
                HistoricalDiscoveryServeExactActionOwnerAtRank(
                  node, clockValue, sourceRank, packet, known, budget,
                  identity, job, occurrenceRank, workerKind, workerMode)
BY HistoricalDiscoveryPacketProducerCoverageStaysInFrozenCarrier,
   HistoricalDiscoveryLiveCandidateHasExactActionOwner,
   HistoricalDiscoveryLiveServeHasExactActionOwner,
   Isa
   DEF HistoricalDiscoveryCandidateServeLifecycleGoal,
       HistoricalDiscoveryCandidateServeLifecycleDiscovery,
       AsyncTargetNeutralLifecycleDiscoveredOwnerSet,
       HistoricalDiscoveryPacketProducerIdentitySet,
       HistoricalDiscoveryPacketCandidateIdentitySet,
       HistoricalDiscoveryPacketServeIdentitySet,
       HistoricalDiscoveryPacketProducerCoveredIdentitySet,
       HistoricalDiscoveryPacketFrozenKnownIdentitySet

(***************************************************************************
Four exact service families, with the Candidate family split in two.

The first exposes one concrete packet/runner/I/O action only while no logical
Candidate or Serve is live.  The second consumes that fixed action.  The
`actionKind` quantified by the service property is the witness chosen at the
source state; it is never recomputed by a state-dependent `CHOOSE`.  An
admission-to-runner or runner-to-worker handoff must therefore be proved by a
preservation/descent theorem rather than silently changing the selected
action.  Serve consumes the immutable minimum identity at its exact occurrence
rank.  Candidate service first reaches strict descent or the explicit
equal-count/count-increasing residual, then closes that finite non-descent
episode.  This split prevents causal-child creation from being called
progress.  Serve also carries an immutable `workerKind`; a target-to-archive
handoff cannot switch from the historical worker to the ordinary worker
inside one fairness premise.  Indexed proofs may discharge the clauses from
the corresponding per-context product actions; none assumes `AsyncSpecAt`,
an all-joined projection, or fairness of the action family as a union.
***************************************************************************)

HistoricalDiscoveryPacketConcreteActionSelectionProperty(specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             (/\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
                   node, clockValue, sourceRank, packet, known, budget)
              /\ HistoricalDiscoveryPacketProducerIdentitySet(packet) = {})
               ~> (HistoricalDiscoveryCandidateServeLifecycleGoal(
                     node, clockValue, sourceRank,
                     packet, known, budget)
                    \/ \E dependencyRank \in
                         HistoricalDiscoveryPacketDependencyCarrier:
                         \E actionKind \in
                              HistoricalDiscoveryPacketConcreteActionKindCarrier:
                           HistoricalDiscoveryPacketConcreteActionPending(
                             node, clockValue, sourceRank,
                             packet, known, budget,
                             dependencyRank, actionKind))

HistoricalDiscoveryPacketConcreteActionServiceProperty(specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             \A dependencyRank \in
                  HistoricalDiscoveryPacketDependencyCarrier:
               \A actionKind \in
                    HistoricalDiscoveryPacketConcreteActionKindCarrier:
                 HistoricalDiscoveryPacketConcreteActionPending(
                   node, clockValue, sourceRank,
                   packet, known, budget, dependencyRank, actionKind)
                   ~> HistoricalDiscoveryCandidateServeLifecycleGoal(
                        node, clockValue, sourceRank,
                        packet, known, budget)

HistoricalDiscoveryCandidateExactRunnerStepProperty(specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             \A identity, candidate, occurrenceRank, physicalKnown:
               HistoricalDiscoveryCandidateExactActionOwnerAtRank(
                 node, clockValue, sourceRank, packet, known, budget,
                 identity, candidate, occurrenceRank, physicalKnown)
                 ~> (HistoricalDiscoveryCandidateServeLifecycleGoal(
                       node, clockValue, sourceRank,
                       packet, known, budget)
                      \/ HistoricalDiscoveryCandidateNonDescentEpisodeResidual(
                           node, clockValue, sourceRank,
                           packet, known, budget,
                           identity, candidate,
                           occurrenceRank, physicalKnown))

HistoricalDiscoveryCandidateCausalDagBudgetDescentProperty(
    specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             \A identity, candidate, occurrenceRank, physicalKnown:
               \A workBudget \in Nat:
                 HistoricalDiscoveryCandidateCausalDagBudgetFrontier(
                   node, clockValue, sourceRank, packet, known, budget,
                   identity, candidate, occurrenceRank,
                   physicalKnown, workBudget)
                   ~> HistoricalDiscoveryCandidateStrictCausalDagBudgetGoal(
                        node, clockValue, sourceRank, packet, known, budget,
                        identity, candidate, occurrenceRank,
                        physicalKnown, workBudget)

HistoricalDiscoveryCandidateCausalDagBudgetClosureProperty(
    specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             \A identity, candidate, occurrenceRank, physicalKnown:
               \A workBudget \in Nat:
                 HistoricalDiscoveryCandidateCausalDagBudgetFrontier(
                   node, clockValue, sourceRank, packet, known, budget,
                   identity, candidate, occurrenceRank,
                   physicalKnown, workBudget)
                   ~> HistoricalDiscoveryCandidateServeLifecycleGoal(
                        node, clockValue, sourceRank,
                        packet, known, budget)

THEOREM HistoricalDiscoveryFiniteCausalDagBudgetClosesFrontier ==
  \A specification:
    HistoricalDiscoveryCandidateCausalDagBudgetDescentProperty(
      specification)
      => HistoricalDiscoveryCandidateCausalDagBudgetClosureProperty(
           specification)
BY NatLessThanWellFounded, WellFoundedLeadsTo
   DEF HistoricalDiscoveryCandidateCausalDagBudgetDescentProperty,
       HistoricalDiscoveryCandidateCausalDagBudgetClosureProperty,
       HistoricalDiscoveryCandidateStrictCausalDagBudgetGoal

HistoricalDiscoveryCandidateNonDescentEpisodeClosureProperty(
    specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             \A identity, candidate, occurrenceRank, physicalKnown:
               HistoricalDiscoveryCandidateNonDescentEpisodeResidual(
                 node, clockValue, sourceRank, packet, known, budget,
                 identity, candidate, occurrenceRank, physicalKnown)
                 ~> HistoricalDiscoveryCandidateServeLifecycleGoal(
                      node, clockValue, sourceRank,
                      packet, known, budget)

THEOREM HistoricalDiscoveryCausalDagBudgetClosesNonDescentEpisode ==
  \A specification:
    HistoricalDiscoveryCandidateCausalDagBudgetDescentProperty(
      specification)
      => HistoricalDiscoveryCandidateNonDescentEpisodeClosureProperty(
           specification)
BY HistoricalDiscoveryFiniteCausalDagBudgetClosesFrontier,
   HistoricalDiscoveryCandidateNonDescentStartsCausalDagBudget,
   Isa, PTL
   DEF HistoricalDiscoveryCandidateCausalDagBudgetClosureProperty,
       HistoricalDiscoveryCandidateNonDescentEpisodeClosureProperty

HistoricalDiscoveryCandidateExactRunnerServiceProperty(specification) ==
  /\ HistoricalDiscoveryCandidateExactRunnerStepProperty(specification)
  /\ HistoricalDiscoveryCandidateCausalDagBudgetDescentProperty(
       specification)

HistoricalDiscoveryServeExactWorkerStepProperty(specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             \A identity, job, occurrenceRank, workerKind, workerMode:
               HistoricalDiscoveryServeExactActionOwnerAtRank(
                 node, clockValue, sourceRank, packet, known, budget,
                 identity, job, occurrenceRank, workerKind, workerMode)
                 ~> HistoricalDiscoveryServeExactWorkerModeProgressGoal(
                      node, clockValue, sourceRank, packet, known, budget,
                      identity, job, occurrenceRank, workerMode)

HistoricalDiscoveryServeExactWorkerModeStepProperty(specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             \A identity, job, occurrenceRank:
               \A workerMode \in
                    HistoricalDiscoveryServeExactWorkerModeCarrier:
                 HistoricalDiscoveryServeExactWorkerModeFrontier(
                   node, clockValue, sourceRank, packet, known, budget,
                   identity, job, occurrenceRank, workerMode)
                   ~> HistoricalDiscoveryServeExactWorkerModeProgressGoal(
                        node, clockValue, sourceRank, packet, known, budget,
                        identity, job, occurrenceRank, workerMode)

THEOREM HistoricalDiscoveryExactWorkerStepsProvideModeSteps ==
  \A specification:
    HistoricalDiscoveryServeExactWorkerStepProperty(specification)
      => HistoricalDiscoveryServeExactWorkerModeStepProperty(
           specification)
BY Isa, PTL
   DEF HistoricalDiscoveryServeExactWorkerStepProperty,
       HistoricalDiscoveryServeExactWorkerModeStepProperty,
       HistoricalDiscoveryServeExactWorkerModeFrontier

HistoricalDiscoveryServeExactWorkerModeClosureProperty(specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             \A identity, job, occurrenceRank:
               \A workerMode \in
                    HistoricalDiscoveryServeExactWorkerModeCarrier:
                 HistoricalDiscoveryServeExactWorkerModeFrontier(
                   node, clockValue, sourceRank, packet, known, budget,
                   identity, job, occurrenceRank, workerMode)
                   ~> HistoricalDiscoveryCandidateServeLifecycleGoal(
                        node, clockValue, sourceRank,
                        packet, known, budget)

HistoricalDiscoveryServeExactWorkerClosureProperty(specification) ==
  specification
    => \A node \in Responsive,
          clockValue \in Nat,
          sourceRank \in
            HistoricalDiscoveryFixedClockBlockerCarrier:
         \A packet, known:
           \A budget \in Nat:
             \A identity, job, occurrenceRank, workerKind, workerMode:
               HistoricalDiscoveryServeExactActionOwnerAtRank(
                 node, clockValue, sourceRank, packet, known, budget,
                 identity, job, occurrenceRank, workerKind, workerMode)
                 ~> HistoricalDiscoveryCandidateServeLifecycleGoal(
                      node, clockValue, sourceRank,
                      packet, known, budget)

THEOREM HistoricalDiscoveryServeWorkerModeDescentClosesService ==
  \A specification:
    HistoricalDiscoveryServeExactWorkerStepProperty(specification)
      => HistoricalDiscoveryServeExactWorkerClosureProperty(
           specification)
BY HistoricalDiscoveryExactWorkerStepsProvideModeSteps,
   HistoricalDiscoveryServeExactWorkerModeOrderingIsWellFounded,
   WellFoundedLeadsTo, Isa, PTL
   DEF HistoricalDiscoveryServeExactWorkerModeStepProperty,
       HistoricalDiscoveryServeExactWorkerModeClosureProperty,
       HistoricalDiscoveryServeExactWorkerClosureProperty,
       HistoricalDiscoveryServeExactWorkerModeFrontier,
       HistoricalDiscoveryServeExactWorkerModeProgressGoal,
       HistoricalDiscoveryServeExactWorkerModeHandoffResidual

HistoricalDiscoveryServeExactWorkerServiceProperty(specification) ==
  HistoricalDiscoveryServeExactWorkerStepProperty(specification)

HistoricalDiscoveryCandidateServeLifecyclePhysicalKernelProperties(
    specification) ==
  /\ HistoricalDiscoveryPacketConcreteActionSelectionProperty(
       specification)
  /\ HistoricalDiscoveryPacketConcreteActionServiceProperty(
       specification)
  /\ HistoricalDiscoveryCandidateExactRunnerServiceProperty(
       specification)
  /\ HistoricalDiscoveryServeExactWorkerServiceProperty(
       specification)

THEOREM HistoricalDiscoveryPhysicalKernelsDischargeLifecycleService ==
  \A specification:
    HistoricalDiscoveryCandidateServeLifecyclePhysicalKernelProperties(
      specification)
      => HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty(
           specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalDiscoveryCandidateServeLifecyclePhysicalKernelProperties(
                  specification)
         PROVE HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty(
                 specification)
    <2>1. CASE specification
      <3>1. ASSUME NEW node \in Responsive,
                    NEW clockValue \in Nat,
                    NEW sourceRank \in
                      HistoricalDiscoveryFixedClockBlockerCarrier,
                    NEW packet, NEW known, NEW budget \in Nat
             PROVE HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
                     node, clockValue, sourceRank,
                     packet, known, budget)
                     ~> HistoricalDiscoveryCandidateServeLifecycleGoal(
                          node, clockValue, sourceRank,
                          packet, known, budget)
        <4>1. HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
                 node, clockValue, sourceRank, packet, known, budget)
                => \/ HistoricalDiscoveryCandidateServeLifecycleGoal(
                        node, clockValue, sourceRank,
                        packet, known, budget)
                   \/ /\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
                           node, clockValue, sourceRank,
                           packet, known, budget)
                      /\ HistoricalDiscoveryPacketProducerIdentitySet(packet)
                           = {}
                   \/ \E identity, candidate, occurrenceRank,
                         physicalKnown:
                        HistoricalDiscoveryCandidateExactActionOwnerAtRank(
                          node, clockValue, sourceRank,
                          packet, known, budget,
                          identity, candidate,
                          occurrenceRank, physicalKnown)
                   \/ \E identity, job, occurrenceRank,
                         workerKind, workerMode:
                        HistoricalDiscoveryServeExactActionOwnerAtRank(
                          node, clockValue, sourceRank,
                          packet, known, budget,
                          identity, job, occurrenceRank,
                          workerKind, workerMode)
          BY HistoricalDiscoveryEpisodeHasKnownExactOwnerOrPacketTail
        <4>2. (/\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
                        node, clockValue, sourceRank,
                        packet, known, budget)
                 /\ HistoricalDiscoveryPacketProducerIdentitySet(packet) = {})
                ~> HistoricalDiscoveryCandidateServeLifecycleGoal(
                     node, clockValue, sourceRank, packet, known, budget)
          BY <1>1, <2>1, PTL
             DEF HistoricalDiscoveryCandidateServeLifecyclePhysicalKernelProperties,
                 HistoricalDiscoveryPacketConcreteActionSelectionProperty,
                 HistoricalDiscoveryPacketConcreteActionServiceProperty
        <4>3. \A identity, candidate, occurrenceRank, physicalKnown:
                 HistoricalDiscoveryCandidateExactActionOwnerAtRank(
                   node, clockValue, sourceRank, packet, known, budget,
                   identity, candidate, occurrenceRank, physicalKnown)
                   ~> HistoricalDiscoveryCandidateServeLifecycleGoal(
                        node, clockValue, sourceRank,
                        packet, known, budget)
          BY <1>1, <2>1,
             HistoricalDiscoveryCausalDagBudgetClosesNonDescentEpisode,
             PTL
             DEF HistoricalDiscoveryCandidateServeLifecyclePhysicalKernelProperties,
                 HistoricalDiscoveryCandidateExactRunnerServiceProperty,
                 HistoricalDiscoveryCandidateExactRunnerStepProperty,
                 HistoricalDiscoveryCandidateCausalDagBudgetDescentProperty,
                 HistoricalDiscoveryCandidateNonDescentEpisodeClosureProperty
        <4>4. \A identity, job, occurrenceRank, workerKind, workerMode:
                 HistoricalDiscoveryServeExactActionOwnerAtRank(
                   node, clockValue, sourceRank, packet, known, budget,
                   identity, job, occurrenceRank, workerKind, workerMode)
                   ~> HistoricalDiscoveryCandidateServeLifecycleGoal(
                        node, clockValue, sourceRank,
                        packet, known, budget)
          BY <1>1, <2>1,
             HistoricalDiscoveryServeWorkerModeDescentClosesService
             DEF HistoricalDiscoveryCandidateServeLifecyclePhysicalKernelProperties,
                 HistoricalDiscoveryServeExactWorkerServiceProperty,
                 HistoricalDiscoveryServeExactWorkerStepProperty,
                 HistoricalDiscoveryServeExactWorkerClosureProperty
        <4> QED BY <4>1, <4>2, <4>3, <4>4, PTL
      <3> QED BY <3>1
    <2>2. CASE ~specification
      BY <2>2
         DEF HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM HistoricalDiscoveryLifecycleServiceDischargesPacketService ==
  \A specification:
    HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty(
      specification)
      => HistoricalDiscoveryFixedClockPacketServiceProperty(
           specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty(
                  specification)
         PROVE HistoricalDiscoveryFixedClockPacketServiceProperty(
                 specification)
    <2>1. CASE specification
      <3>1. ASSUME NEW node \in Responsive,
                    NEW clockValue \in Nat,
                    NEW sourceRank \in
                      HistoricalDiscoveryFixedClockBlockerCarrier,
                    NEW packet \in AsyncPacketSet, NEW known
             PROVE HistoricalDiscoveryCandidateServeEpisodeSource(
                     node, clockValue, sourceRank, packet, known)
                     ~> (HistoricalDiscoveryFixedClockStrictRankGoal(
                           node, clockValue, sourceRank)
                          \/ HistoricalDiscoveryCandidateServeEpisodeResidual(
                               node, clockValue, sourceRank, packet, known))
        <4>0. HistoricalDiscoveryCandidateServeEpisodeSource(
                 node, clockValue, sourceRank, packet, known)
                => IsFiniteSet(known)
          BY HistoricalDiscoveryCandidateServeEpisodeSourceHasFiniteKnownSet
        <4>1. HistoricalDiscoveryCandidateServeEpisodeSource(
                 node, clockValue, sourceRank, packet, known)
                ~> \E budget \in Nat:
                     HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
                       node, clockValue, sourceRank,
                       packet, known, budget)
          BY HistoricalDiscoveryCandidateServeSourceStartsNeutralEpisode,
             PTL
        <4>2. \A budget \in Nat:
                 HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
                   node, clockValue, sourceRank, packet, known, budget)
                   ~> HistoricalDiscoveryCandidateServeLifecycleGoal(
                        node, clockValue, sourceRank,
                        packet, known, budget)
          BY <1>1, <2>1
             DEF HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty
        <4>3. \A budget \in Nat:
                 /\ IsFiniteSet(known)
                 /\ HistoricalDiscoveryCandidateServeLifecycleGoal(
                      node, clockValue, sourceRank,
                      packet, known, budget)
                => (HistoricalDiscoveryFixedClockStrictRankGoal(
                      node, clockValue, sourceRank)
                     \/ HistoricalDiscoveryCandidateServeEpisodeResidual(
                          node, clockValue, sourceRank, packet, known))
          BY Isa
             DEF HistoricalDiscoveryCandidateServeLifecycleGoal,
                 HistoricalDiscoveryCandidateServeLifecycleDiscovery,
                 HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
                 HistoricalDiscoveryCandidateServeEpisodeResidual,
                 HistoricalDiscoveryPacketFrozenKnownIdentitySet,
                 AsyncTargetNeutralLifecycleEpisodeAtBudget,
                 AsyncTargetNeutralLifecycleDiscoveredOwnerSet
        <4> QED BY <4>0, <4>1, <4>2, <4>3, PTL
      <3> QED BY <3>1
    <2>2. CASE ~specification
      BY <2>2 DEF HistoricalDiscoveryFixedClockPacketServiceProperty
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM HistoricalDiscoveryCandidateServeServiceLowersBudgetOrExits ==
  \A specification:
    HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty(
      specification)
      => (specification
            => \A node \in Responsive,
                  clockValue \in Nat,
                  sourceRank \in
                    HistoricalDiscoveryFixedClockBlockerCarrier:
                 \A budget \in Nat:
                   HistoricalDiscoveryCandidateServeLifecycleBudgetAtRank(
                     node, clockValue, sourceRank, budget)
                     ~> (HistoricalDiscoveryFixedClockStrictRankGoal(
                           node, clockValue, sourceRank)
                          \/ \E lowerBudget \in
                               SetLessThan(
                                 budget,
                                 AsyncTargetNeutralLifecycleBudgetOrdering,
                                 Nat):
                               HistoricalDiscoveryCandidateServeLifecycleBudgetAtRank(
                                 node, clockValue, sourceRank,
                                 lowerBudget)))
BY HistoricalDiscoveryCandidateServeDiscoveryConsumesNeutralBudget,
   PTL, Isa
   DEF HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty,
       HistoricalDiscoveryCandidateServeLifecycleGoal,
       HistoricalDiscoveryCandidateServeLifecycleBudgetAtRank

THEOREM HistoricalDiscoveryCandidateServeServiceClosesFiniteEpisode ==
  \A specification:
    HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty(
      specification)
      => (specification
            => \A node \in Responsive,
                  clockValue \in Nat,
                  sourceRank \in
                    HistoricalDiscoveryFixedClockBlockerCarrier,
                  budget \in Nat:
                 HistoricalDiscoveryCandidateServeLifecycleBudgetAtRank(
                   node, clockValue, sourceRank, budget)
                   ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                        node, clockValue, sourceRank))
BY HistoricalDiscoveryCandidateServeServiceLowersBudgetOrExits,
   AsyncTargetNeutralLifecycleBudgetOrderingIsWellFounded,
   WellFoundedLeadsTo

THEOREM HistoricalDiscoveryCandidateServeOwnerServiceClosesIdentityBudget ==
  \A specification:
    HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty(
      specification)
      => HistoricalDiscoveryCandidateServeIdentityBudgetProperty(
           specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalDiscoveryCandidateServeLifecycleOwnerServiceProperty(
                  specification)
         PROVE HistoricalDiscoveryCandidateServeIdentityBudgetProperty(
                 specification)
    <2>1. CASE specification
      <3>1. ASSUME NEW node \in Responsive,
                    NEW clockValue \in Nat,
                    NEW sourceRank \in
                      HistoricalDiscoveryFixedClockBlockerCarrier,
                    NEW packet, NEW known
             PROVE HistoricalDiscoveryCandidateServeEpisodeResidual(
                     node, clockValue, sourceRank, packet, known)
                     ~>
                   HistoricalDiscoveryFixedClockStrictRankGoal(
                     node, clockValue, sourceRank)
        <4>1. HistoricalDiscoveryCandidateServeEpisodeResidual(
                 node, clockValue, sourceRank, packet, known)
                ~>
              \E budget \in Nat:
                HistoricalDiscoveryCandidateServeLifecycleBudgetAtRank(
                  node, clockValue, sourceRank, budget)
          BY HistoricalDiscoveryCandidateServeResidualStartsNeutralDiscovery,
             PTL
             DEF HistoricalDiscoveryCandidateServeLifecycleDiscovery,
                 HistoricalDiscoveryCandidateServeLifecycleBudgetAtRank
        <4>2. \A budget \in Nat:
                 HistoricalDiscoveryCandidateServeLifecycleBudgetAtRank(
                   node, clockValue, sourceRank, budget)
                   ~> HistoricalDiscoveryFixedClockStrictRankGoal(
                        node, clockValue, sourceRank)
          BY <1>1, <2>1,
             HistoricalDiscoveryCandidateServeServiceClosesFiniteEpisode
        <4> QED BY <4>1, <4>2, PTL
      <3> QED BY <3>1
    <2> QED BY <2>1
         DEF HistoricalDiscoveryCandidateServeIdentityBudgetProperty
  <1> QED BY <1>1

(***************************************************************************
Exact residual packet corridor.

This is the smallest temporal interface still below the fixed-clock release
theorem.  The former packet-service conjunct was redundant: every packet
source snapshot starts the same finite lifecycle episode, so lifecycle
service derives packet service directly.  The residual now exposes only the
four physical seams above: exact packet action selection, service of that
fixed packet/runner/I/O action, exact Candidate-minimum runner service, and
exact Serve-minimum worker service.  The action-local source algebra is
already proved by:

  * `HistoricalDiscoveryFixedClockIngressStrictlyDescends` for exact due-head
    admission and older lane-shadow removal;
  * `HistoricalDiscoveryCandidateExitClassifiesOccurrenceDebt` and
    `HistoricalDiscoveryServeFairActionLowersOccurrenceDebt` for the two
    occurrence tails; and
  * `AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst`,
    `AsyncServeQueuedIdentityDepartureInstallsTombstone`, and the Serve
    no-requeue theorems for durable coalescing.

Only product-local temporal selection and service of those explicitly named
actions is left in this interface.  It contains no Decision, height,
application, rotating-leader, clock, aggregate `AsyncSpecAt`, all-joined, or
replenishment-as-progress conclusion.
***************************************************************************)

HistoricalDiscoveryFixedClockPacketCorridorTemporalResidual(
    specification) ==
  HistoricalDiscoveryCandidateServeLifecyclePhysicalKernelProperties(
    specification)

THEOREM HistoricalDiscoveryPacketCorridorResidualClosesPacketLeaves ==
  \A specification:
    HistoricalDiscoveryFixedClockPacketCorridorTemporalResidual(
      specification)
      => /\ HistoricalDiscoveryFixedClockPacketServiceProperty(
               specification)
         /\ HistoricalDiscoveryCandidateServeIdentityBudgetProperty(
              specification)
BY HistoricalDiscoveryPhysicalKernelsDischargeLifecycleService,
   HistoricalDiscoveryLifecycleServiceDischargesPacketService,
   HistoricalDiscoveryCandidateServeOwnerServiceClosesIdentityBudget,
   Isa
   DEF HistoricalDiscoveryFixedClockPacketCorridorTemporalResidual

HistoricalDiscoveryFixedClockConcreteServiceProperties(specification) ==
  /\ HistoricalDiscoveryFixedClockNonPacketServiceProperty(specification)
  /\ HistoricalDiscoveryFixedClockPacketServiceProperty(specification)

HistoricalDiscoveryFixedClockTemporalPrerequisites(specification) ==
  /\ HistoricalDiscoveryFixedClockConcreteServiceProperties(specification)
  /\ HistoricalDiscoveryCandidateServeIdentityBudgetProperty(specification)

THEOREM AsyncSpecAndPacketCorridorResidualCloseFixedClockPrerequisites ==
  \A initialContext:
    HistoricalDiscoveryFixedClockPacketCorridorTemporalResidual(
      AsyncSpecAt(initialContext))
      => HistoricalDiscoveryFixedClockTemporalPrerequisites(
           AsyncSpecAt(initialContext))
BY AsyncSpecClosesHistoricalDiscoveryFixedClockNonPacketService,
   HistoricalDiscoveryPacketCorridorResidualClosesPacketLeaves,
   Isa
   DEF HistoricalDiscoveryFixedClockTemporalPrerequisites,
       HistoricalDiscoveryFixedClockConcreteServiceProperties

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

HistoricalDiscoveryClockTemporalSupportProperty(specification) ==
  /\ (specification => []AsyncStrongTypeInvariant)
  /\ (specification => [](gst => []gst))

THEOREM HistoricalDiscoveryFixedClockClosureLowersClockBudgetFromSupport ==
  \A specification:
    /\ HistoricalDiscoveryClockTemporalSupportProperty(specification)
    /\ HistoricalDiscoveryFixedClockClosureProperty(specification)
    => HistoricalDiscoveryClockBudgetDescentProperty(specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalDiscoveryClockTemporalSupportProperty(
                  specification),
                HistoricalDiscoveryFixedClockClosureProperty(specification)
         PROVE HistoricalDiscoveryClockBudgetDescentProperty(specification)
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
        BY <1>1
           DEF HistoricalDiscoveryClockTemporalSupportProperty
      <3>3. []AsyncStrongTypeInvariant
        BY <1>1
           DEF HistoricalDiscoveryClockTemporalSupportProperty
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

THEOREM AsyncSpecProvidesHistoricalDiscoveryClockTemporalSupport ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => HistoricalDiscoveryClockTemporalSupportProperty(
           AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecKeepsGstOnceSet
   DEF HistoricalDiscoveryClockTemporalSupportProperty

THEOREM HistoricalDiscoveryFixedClockClosureLowersClockBudget ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalDiscoveryFixedClockClosureProperty(
         AsyncSpecAt(initialContext))
    => HistoricalDiscoveryClockBudgetDescentProperty(
         AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalDiscoveryClockTemporalSupport,
   HistoricalDiscoveryFixedClockClosureLowersClockBudgetFromSupport

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

THEOREM HistoricalDiscoveryClockBudgetClosureReachesReleaseGoalFromSupport ==
  \A specification:
    /\ HistoricalDiscoveryClockTemporalSupportProperty(specification)
    /\ HistoricalDiscoveryClockBudgetClosureProperty(specification)
    => HistoricalCommitCertificateDiscoveryClockProgressProperty(
         specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalDiscoveryClockTemporalSupportProperty(
                  specification),
                HistoricalDiscoveryClockBudgetClosureProperty(specification)
         PROVE
           HistoricalCommitCertificateDiscoveryClockProgressProperty(
             specification)
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
        BY <1>1
           DEF HistoricalDiscoveryClockTemporalSupportProperty
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

THEOREM HistoricalDiscoveryClockBudgetClosureReachesReleaseGoal ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalDiscoveryClockBudgetClosureProperty(
         AsyncSpecAt(initialContext))
    => HistoricalCommitCertificateDiscoveryClockProgressProperty(
         AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalDiscoveryClockTemporalSupport,
   HistoricalDiscoveryClockBudgetClosureReachesReleaseGoalFromSupport

THEOREM HistoricalDiscoveryTemporalPrerequisitesCloseClockProgressFromSupport ==
  \A specification:
    /\ HistoricalDiscoveryClockTemporalSupportProperty(specification)
    /\ HistoricalDiscoveryFixedClockTemporalPrerequisites(specification)
    => HistoricalCommitCertificateDiscoveryClockProgressProperty(
         specification)
BY HistoricalDiscoveryTemporalPrerequisitesCloseFixedClock,
   HistoricalDiscoveryFixedClockClosureLowersClockBudgetFromSupport,
   HistoricalDiscoveryClockBudgetDescentClosesTimeout,
   HistoricalDiscoveryClockBudgetClosureReachesReleaseGoalFromSupport

THEOREM HistoricalDiscoveryTemporalPrerequisitesCloseClockProgress ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalDiscoveryFixedClockTemporalPrerequisites(
         AsyncSpecAt(initialContext))
    => HistoricalCommitCertificateDiscoveryClockProgressProperty(
         AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalDiscoveryClockTemporalSupport,
   HistoricalDiscoveryTemporalPrerequisitesCloseClockProgressFromSupport

=============================================================================
