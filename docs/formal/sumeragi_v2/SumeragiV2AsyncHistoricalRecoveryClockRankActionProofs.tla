---- MODULE SumeragiV2AsyncHistoricalRecoveryClockRankActionProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryClockOwnerActionProofs

(***************************************************************************
Concrete outer-rank action edges for the historical discovery clock.

This module composes the independently proved owner-prefix and transport
facts into the lexicographic certificate consumed by
`HistoricalDiscoveryConcreteLexCertificateStrictlyDescends`.  Every theorem
is action-local.  No temporal theorem, existential action union, Decision
convergence result, or application-liveness result appears here.
***************************************************************************)

HistoricalDiscoveryFixedClockActionGoal(node, clockValue) ==
  \/ HistoricalDiscoveryFixedClockExit(node, clockValue)'
  \/ /\ HistoricalDiscoveryFixedClockPending(node, clockValue)'
        /\ <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
             HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
             \in HistoricalDiscoveryFixedClockBlockerOrdering

THEOREM HistoricalDiscoveryFixedClockTickReachesExit ==
  \A node \in Responsive, clockValue \in Nat:
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
    /\ AsyncTick
    => HistoricalDiscoveryFixedClockExit(node, clockValue)'
BY SMT
   DEF HistoricalDiscoveryFixedClockPending,
       HistoricalDiscoveryFixedClockExit,
       AsyncTick

(***************************************************************************
Any concrete ingress branch consumes one member of the frozen due set.
Newly published packets cannot participate in this step.  The latent-owner
prefix is antitone after GST, so either that earlier prefix falls or the due
packet cardinality itself supplies the strict lexicographic edge.
***************************************************************************)

THEOREM HistoricalDiscoveryFixedClockIngressStrictlyDescends ==
  \A node \in Responsive,
     recipient \in ValidatorIds,
     source \in AsyncIngressSources,
     clockValue \in Nat:
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)'
    /\ [AsyncNext]_AsyncAllVars
    /\ AdmitIngressPacket(recipient, source)
    => <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
          HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
         \in HistoricalDiscoveryFixedClockBlockerOrdering
PROOF
  <1>1. ASSUME NEW node \in Responsive,
                NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                NEW clockValue \in Nat,
                HistoricalDiscoveryFixedClockPending(node, clockValue),
                HistoricalDiscoveryFixedClockPending(node, clockValue)',
                [AsyncNext]_AsyncAllVars,
                AdmitIngressPacket(recipient, source)
         PROVE <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
                  HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
                 \in HistoricalDiscoveryFixedClockBlockerOrdering
    <2>1. HistoricalDiscoveryDuePacketDebt(clockValue)' + 1
             = HistoricalDiscoveryDuePacketDebt(clockValue)
      BY <1>1, HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket
         DEF HistoricalDiscoveryFixedClockPending
    <2>2. HistoricalDiscoveryLatentOwnerDebt'
             <= HistoricalDiscoveryLatentOwnerDebt
      BY <1>1, HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock
         DEF HistoricalDiscoveryFixedClockPending,
             AdmitIngressPacket, AdmitHiddenPacket,
             CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket
    <2>3. HistoricalDiscoveryFixedClockLexStep(
             clockValue,
             HistoricalDiscoveryConcreteBlockerStage(clockValue),
             HistoricalDiscoveryConcreteDependencyRank(clockValue),
             HistoricalDiscoveryConcreteBlockerStage(clockValue)',
             HistoricalDiscoveryConcreteDependencyRank(clockValue)')
      BY <1>1, <2>1, <2>2,
         StrongTypeHasFiniteHistoricalDiscoveryCohorts, SMT
         DEF HistoricalDiscoveryFixedClockPending,
             HistoricalDiscoveryFixedClockLexStep,
             HistoricalDiscoveryLatentOwnerDebt,
             HistoricalDiscoveryDuePacketDebt
    <2> QED BY <1>1, <2>3,
         HistoricalDiscoveryConcreteLexCertificateStrictlyDescends
  <1> QED BY <1>1

THEOREM HistoricalDiscoveryPostGstIngressStrictlyDescends ==
  \A node \in Responsive,
     recipient \in ValidatorIds,
     source \in AsyncIngressSources,
     clockValue \in Nat:
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)'
    /\ [AsyncNext]_AsyncAllVars
    => /\ (PostGstAdmitHiddenPacket(recipient, source)
              => <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
                    HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
                   \in HistoricalDiscoveryFixedClockBlockerOrdering)
       /\ (PostGstAdmitHistoricalRecoveryPacket(recipient, source)
              => <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
                    HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
                   \in HistoricalDiscoveryFixedClockBlockerOrdering)
BY HistoricalDiscoveryFixedClockIngressStrictlyDescends
   DEF PostGstAdmitHiddenPacket,
       PostGstAdmitHistoricalRecoveryPacket

(***************************************************************************
Opening a previously latent owner consumes the first component.  This covers
targets outside the current voting roster because the owner domain is the
whole responsive set.
***************************************************************************)

THEOREM HistoricalDiscoveryFixedClockLatentOwnerEntryStrictlyDescends ==
  \A node, owner \in Responsive, clockValue \in Nat:
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)'
    /\ [AsyncNext]_AsyncAllVars
    /\ owner \in HistoricalDiscoveryLatentTimedOwners
    /\ owner \in AsyncTimedServiceNodes'
    => <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
          HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
         \in HistoricalDiscoveryFixedClockBlockerOrdering
PROOF
  <1>1. ASSUME NEW node \in Responsive,
                NEW owner \in Responsive,
                NEW clockValue \in Nat,
                HistoricalDiscoveryFixedClockPending(node, clockValue),
                HistoricalDiscoveryFixedClockPending(node, clockValue)',
                [AsyncNext]_AsyncAllVars,
                owner \in HistoricalDiscoveryLatentTimedOwners,
                owner \in AsyncTimedServiceNodes'
         PROVE <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
                  HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
                 \in HistoricalDiscoveryFixedClockBlockerOrdering
    <2>1. HistoricalDiscoveryLatentOwnerDebt'
             < HistoricalDiscoveryLatentOwnerDebt
      BY <1>1, HistoricalLatentOwnerEntryStrictlyDecreasesDebt
         DEF HistoricalDiscoveryFixedClockPending
    <2>2. HistoricalDiscoveryFixedClockLexStep(
             clockValue,
             HistoricalDiscoveryConcreteBlockerStage(clockValue),
             HistoricalDiscoveryConcreteDependencyRank(clockValue),
             HistoricalDiscoveryConcreteBlockerStage(clockValue)',
             HistoricalDiscoveryConcreteDependencyRank(clockValue)')
      BY <2>1 DEF HistoricalDiscoveryFixedClockLexStep
    <2> QED BY <1>1, <2>2,
         HistoricalDiscoveryConcreteLexCertificateStrictlyDescends
  <1> QED BY <1>1

(***************************************************************************
An enqueue into a due empty queue spends the dormant gate before exposing an
active worker.  The theorem keeps the due-packet frame explicit: concrete
runner/server enqueue projections supply it from the fixed-clock publication
lemmas rather than silently assuming that the action cannot publish.
***************************************************************************)

THEOREM HistoricalDiscoveryFixedClockDormantHandoffStrictlyDescends ==
  \A node \in Responsive,
     owner \in ValidatorIds,
     clockValue \in Nat:
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)'
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryDuePacketDebt(clockValue)' =
         HistoricalDiscoveryDuePacketDebt(clockValue)
    /\ HistoricalDiscoveryDormantGateHandoff(owner, clockValue)
    => <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
          HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
         \in HistoricalDiscoveryFixedClockBlockerOrdering
PROOF
  <1>1. ASSUME NEW node \in Responsive,
                NEW owner \in ValidatorIds,
                NEW clockValue \in Nat,
                HistoricalDiscoveryFixedClockPending(node, clockValue),
                HistoricalDiscoveryFixedClockPending(node, clockValue)',
                [AsyncNext]_AsyncAllVars,
                HistoricalDiscoveryDuePacketDebt(clockValue)' =
                  HistoricalDiscoveryDuePacketDebt(clockValue),
                HistoricalDiscoveryDormantGateHandoff(owner, clockValue)
         PROVE <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
                  HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
                 \in HistoricalDiscoveryFixedClockBlockerOrdering
    <2>1. HistoricalDiscoveryLatentOwnerDebt'
             <= HistoricalDiscoveryLatentOwnerDebt
      BY <1>1, HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock
         DEF HistoricalDiscoveryFixedClockPending,
             HistoricalDiscoveryDormantGateHandoff
    <2>2. HistoricalDiscoveryDormantIoDebt(clockValue)'
             < HistoricalDiscoveryDormantIoDebt(clockValue)
      BY <1>1, SMT
         DEF HistoricalDiscoveryDormantGateHandoff
    <2>3. HistoricalDiscoveryFixedClockLexStep(
             clockValue,
             HistoricalDiscoveryConcreteBlockerStage(clockValue),
             HistoricalDiscoveryConcreteDependencyRank(clockValue),
             HistoricalDiscoveryConcreteBlockerStage(clockValue)',
             HistoricalDiscoveryConcreteDependencyRank(clockValue)')
      BY <1>1, <2>1, <2>2, SMT
         DEF HistoricalDiscoveryFixedClockLexStep
    <2> QED BY <1>1, <2>3,
         HistoricalDiscoveryConcreteLexCertificateStrictlyDescends
  <1> QED BY <1>1

(***************************************************************************
Due runner and I/O service edges.

The prefix equalities are named because a runner may publish strictly-future
packets and may spend a dormant gate.  The concrete classification theorem in
the temporal closure supplies one of: an earlier latent/dormant descent, these
equalities, or an exit.  Under the equality branch, removing the selected
blocker lowers its exact natural counter or the blocker stage.
***************************************************************************)

HistoricalDiscoveryFixedClockOuterPrefixEqual(clockValue) ==
  /\ HistoricalDiscoveryLatentOwnerDebt'
       = HistoricalDiscoveryLatentOwnerDebt
  /\ HistoricalDiscoveryDuePacketDebt(clockValue)'
       = HistoricalDiscoveryDuePacketDebt(clockValue)
  /\ HistoricalDiscoveryDormantIoDebt(clockValue)'
       = HistoricalDiscoveryDormantIoDebt(clockValue)

THEOREM HistoricalDiscoveryDueNodeServiceStrictlyDescends ==
  \A node \in Responsive,
     owner \in ValidatorIds,
     clockValue \in Nat:
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)'
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryFixedClockOuterPrefixEqual(clockValue)
    /\ OverdueResponsivePackets = {}
    /\ owner \in HistoricalDiscoveryNodeBlockersAt(clockValue)
    /\ HistoricalDiscoveryNodeServiceOutcome(owner, clockValue)
    /\ HistoricalDiscoveryNodeBlockersAt(clockValue)' =
         HistoricalDiscoveryNodeBlockersAt(clockValue) \ {owner}
    => <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
          HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
         \in HistoricalDiscoveryFixedClockBlockerOrdering
BY HistoricalDiscoveryConcreteLexCertificateStrictlyDescends,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, FS_CardinalityType, Isa
   DEF HistoricalDiscoveryFixedClockPending,
       HistoricalDiscoveryFixedClockOuterPrefixEqual,
       HistoricalDiscoveryFixedClockLexStep,
       HistoricalDiscoveryConcreteBlockerStage,
       HistoricalDiscoveryConcreteDependencyRank,
       HistoricalDiscoveryNodeServiceOutcome,
       HistoricalDiscoveryNodeBlockerDebt,
       HistoricalDiscoveryIngressCounterRank,
       HistoricalDiscoveryPacketDependencyOrdering,
       HistoricalDiscoveryCapacityTailOrdering,
       LexPairOrdering, OpToRel

THEOREM HistoricalDiscoveryDueIoServiceStrictlyDescends ==
  \A node \in Responsive,
     owner \in ValidatorIds,
     clockValue \in Nat:
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)
    /\ HistoricalDiscoveryFixedClockPending(node, clockValue)'
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryFixedClockOuterPrefixEqual(clockValue)
    /\ OverdueResponsivePackets = {}
    /\ HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
    /\ owner
         \in HistoricalDiscoveryActiveIoBlockersAt(clockValue)
    /\ HistoricalDiscoveryDueIoQueueServiceOutcome(owner, clockValue)
    => <<HistoricalDiscoveryConcreteFixedClockRank(clockValue)',
          HistoricalDiscoveryConcreteFixedClockRank(clockValue)>>
         \in HistoricalDiscoveryFixedClockBlockerOrdering
BY HistoricalDiscoveryConcreteLexCertificateStrictlyDescends,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, FS_CardinalityType, Isa
   DEF HistoricalDiscoveryFixedClockPending,
       HistoricalDiscoveryFixedClockOuterPrefixEqual,
       HistoricalDiscoveryFixedClockLexStep,
       HistoricalDiscoveryConcreteBlockerStage,
       HistoricalDiscoveryConcreteDependencyRank,
       HistoricalDiscoveryDueIoQueueServiceOutcome,
       HistoricalDiscoveryActiveIoBlockerDebt,
       HistoricalDiscoveryIngressCounterRank,
       HistoricalDiscoveryPacketDependencyOrdering,
       HistoricalDiscoveryCapacityTailOrdering,
       LexPairOrdering, OpToRel

=============================================================================
