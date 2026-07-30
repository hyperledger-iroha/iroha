---- MODULE SumeragiV2AsyncHistoricalRecoveryLivenessProofs ----
EXTENDS SumeragiV2AsyncTimeoutOwnershipProofs, TLAPS

(***************************************************************************
Exact historical-recovery liveness child.

The one-height proof deliberately scopes its fair scheduler owner to the
frozen current voting roster.  Historical recovery is wider: a responsive
validator can lag at an old context after it has left that roster.  This child
therefore does not reuse `ResponsiveProtectedCandidateOwned`.  It keeps the
same immutable candidate and well-founded service-rank kernels, but gives the
candidate an explicit historical target owner and uses the historical runner
and I/O-worker actions already present in `AsyncFairnessAt`.

The two release prerequisites are stated exactly below.  Neither is declared
as an assumption or a proofless theorem.  The proved lemmas discharge the
well-founded rank composition and the weak-fair discovery step.  The remaining
clock/readiness, next-step preservation, authenticated request corridor, and
historical Decision-source corridor stay visible as operator premises of the
conditional closure theorems at the end of this module.
***************************************************************************)

HistoricalRecoveryTargetDecisionProgressProperty(specification) ==
  specification
    => \A node \in Responsive:
         (gst /\ HistoricalRecoveryTarget(node))
           ~> NodeHasDecision(node)

ResponsiveDecisionApplicationProgressProperty(specification) ==
  specification
    => \A node \in Responsive:
         (gst /\ NodeHasDecision(node))
           ~> NodeHasApplication(node)

HistoricalRecoveryAsyncTemporalPrerequisites(specification) ==
  /\ HistoricalRecoveryTargetDecisionProgressProperty(specification)
  /\ ResponsiveDecisionApplicationProgressProperty(specification)

(***************************************************************************
Historical candidate ownership and well-founded rank composition.

`CandidateServiceRank` is structural and applies to any scheduled candidate.
The temporal owner used by the parent is not structural: it requires current
voter membership.  The owner below replaces only that scope.  A historical
candidate must belong to the responsive recovery target itself; unrelated
current-height candidates at the same node cannot satisfy this predicate.
***************************************************************************)

HistoricalProtectedCandidateOwned(candidate) ==
  /\ candidate.node \in Responsive
  /\ HistoricalRecoveryTarget(candidate.node)
  /\ ProtectedCandidateOwned(candidate)

HistoricalProtectedOwnedAtServiceRank(candidate, rank) ==
  /\ gst
  /\ HistoricalProtectedCandidateOwned(candidate)
  /\ CandidateServiceRank(candidate) = rank

HistoricalProtectedServiceOwnershipExit(candidate) ==
  ~HistoricalProtectedCandidateOwned(candidate)

HistoricalProtectedServiceRankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet,
          rank \in OwnedServiceRankCarrier:
         HistoricalProtectedOwnedAtServiceRank(candidate, rank)
           ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                \/ \E lower \in SetLessThan(
                     rank, OwnedServiceRankOrdering,
                     OwnedServiceRankCarrier):
                     HistoricalProtectedOwnedAtServiceRank(candidate, lower))

HistoricalProtectedStageRankProgressProperty(specification, stage) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ HistoricalProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<stage, position>>)
           ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                \/ \E lower \in SetLessThan(
                     <<stage, position>>, OwnedServiceRankOrdering,
                     OwnedServiceRankCarrier):
                     HistoricalProtectedOwnedAtServiceRank(candidate, lower))

HistoricalProtectedStage2RankProgressProperty(specification) ==
  HistoricalProtectedStageRankProgressProperty(specification, 2)

HistoricalProtectedStage3RankProgressProperty(specification) ==
  HistoricalProtectedStageRankProgressProperty(specification, 3)

HistoricalProtectedStage4RankProgressProperty(specification) ==
  HistoricalProtectedStageRankProgressProperty(specification, 4)

HistoricalProtectedStage5RankProgressProperty(specification) ==
  HistoricalProtectedStageRankProgressProperty(specification, 5)

HistoricalProtectedStage6RankProgressProperty(specification) ==
  HistoricalProtectedStageRankProgressProperty(specification, 6)

HistoricalProtectedServiceRankLeafProperties(specification) ==
  /\ HistoricalProtectedStage2RankProgressProperty(specification)
  /\ HistoricalProtectedStage3RankProgressProperty(specification)
  /\ HistoricalProtectedStage4RankProgressProperty(specification)
  /\ HistoricalProtectedStage5RankProgressProperty(specification)
  /\ HistoricalProtectedStage6RankProgressProperty(specification)

HistoricalProtectedCandidateStarvationProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet:
         (gst /\ HistoricalProtectedCandidateOwned(candidate))
           ~> HistoricalProtectedServiceOwnershipExit(candidate)

THEOREM HistoricalProtectedServiceRankProgressFromStageLeaves ==
  \A specification:
    HistoricalProtectedServiceRankLeafProperties(specification)
      => HistoricalProtectedServiceRankProgressProperty(specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalProtectedServiceRankLeafProperties(specification)
         PROVE HistoricalProtectedServiceRankProgressProperty(specification)
    <2>1. CASE specification
      <3>1. ASSUME NEW candidate \in AsyncCandidateSet,
                    NEW rank \in OwnedServiceRankCarrier
             PROVE HistoricalProtectedOwnedAtServiceRank(candidate, rank)
                     ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                          \/ \E lower \in SetLessThan(
                               rank, OwnedServiceRankOrdering,
                               OwnedServiceRankCarrier):
                               HistoricalProtectedOwnedAtServiceRank(
                                 candidate, lower))
        <4>1. /\ rank[1] \in 2..6
               /\ rank[2] \in Nat
               /\ rank = <<rank[1], rank[2]>>
          BY <3>1 DEF OwnedServiceRankCarrier
        <4>2. CASE rank[1] = 2
          BY <1>1, <2>1, <3>1, <4>1, <4>2, PTL
             DEF HistoricalProtectedServiceRankLeafProperties,
                 HistoricalProtectedStage2RankProgressProperty,
                 HistoricalProtectedStageRankProgressProperty,
                 HistoricalProtectedOwnedAtServiceRank
        <4>3. CASE rank[1] = 3
          BY <1>1, <2>1, <3>1, <4>1, <4>3, PTL
             DEF HistoricalProtectedServiceRankLeafProperties,
                 HistoricalProtectedStage3RankProgressProperty,
                 HistoricalProtectedStageRankProgressProperty,
                 HistoricalProtectedOwnedAtServiceRank
        <4>4. CASE rank[1] = 4
          BY <1>1, <2>1, <3>1, <4>1, <4>4, PTL
             DEF HistoricalProtectedServiceRankLeafProperties,
                 HistoricalProtectedStage4RankProgressProperty,
                 HistoricalProtectedStageRankProgressProperty,
                 HistoricalProtectedOwnedAtServiceRank
        <4>5. CASE rank[1] = 5
          BY <1>1, <2>1, <3>1, <4>1, <4>5, PTL
             DEF HistoricalProtectedServiceRankLeafProperties,
                 HistoricalProtectedStage5RankProgressProperty,
                 HistoricalProtectedStageRankProgressProperty,
                 HistoricalProtectedOwnedAtServiceRank
        <4>6. CASE rank[1] = 6
          BY <1>1, <2>1, <3>1, <4>1, <4>6, PTL
             DEF HistoricalProtectedServiceRankLeafProperties,
                 HistoricalProtectedStage6RankProgressProperty,
                 HistoricalProtectedStageRankProgressProperty,
                 HistoricalProtectedOwnedAtServiceRank
        <4> QED BY <4>1, <4>2, <4>3, <4>4, <4>5, <4>6, Isa
      <3> QED BY <3>1
           DEF HistoricalProtectedServiceRankProgressProperty
    <2>2. CASE ~specification
      BY <2>2 DEF HistoricalProtectedServiceRankProgressProperty
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM HistoricalProtectedCandidateHasServiceRank ==
  \A candidate:
    /\ AsyncTypeInvariant
    /\ gst
    /\ HistoricalProtectedCandidateOwned(candidate)
    => \E rank \in OwnedServiceRankCarrier:
         HistoricalProtectedOwnedAtServiceRank(candidate, rank)
PROOF
  <1>1. ASSUME NEW candidate,
                AsyncTypeInvariant,
                gst,
                HistoricalProtectedCandidateOwned(candidate)
         PROVE \E rank \in OwnedServiceRankCarrier:
                 HistoricalProtectedOwnedAtServiceRank(candidate, rank)
    <2>1. CandidateScheduled(candidate)
      BY <1>1
         DEF HistoricalProtectedCandidateOwned,
             ProtectedCandidateOwned
    <2>2. CandidateServiceRank(candidate) \in OwnedServiceRankCarrier
      BY <1>1, <2>1, ScheduledCandidateServiceRankInCarrier
    <2> QED BY <1>1, <2>2
         DEF HistoricalProtectedOwnedAtServiceRank
  <1> QED BY <1>1

THEOREM HistoricalProtectedServiceRankProgressImpliesStarvation ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalProtectedServiceRankProgressProperty(
         AsyncSpecAt(initialContext))
    => HistoricalProtectedCandidateStarvationProperty(
         AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                HistoricalProtectedServiceRankProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE HistoricalProtectedCandidateStarvationProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME NEW candidate \in AsyncCandidateSet
           PROVE
             (gst /\ HistoricalProtectedCandidateOwned(candidate))
               ~> HistoricalProtectedServiceOwnershipExit(candidate)
      <3>1. \A rank \in OwnedServiceRankCarrier:
               HistoricalProtectedOwnedAtServiceRank(candidate, rank)
                 ~> (HistoricalProtectedServiceOwnershipExit(candidate)
                      \/ \E lower \in SetLessThan(
                           rank, OwnedServiceRankOrdering,
                           OwnedServiceRankCarrier):
                           HistoricalProtectedOwnedAtServiceRank(
                             candidate, lower))
        BY <1>1, <2>1
           DEF HistoricalProtectedServiceRankProgressProperty
      <3>2. \A rank \in OwnedServiceRankCarrier:
               HistoricalProtectedOwnedAtServiceRank(candidate, rank)
                 ~> HistoricalProtectedServiceOwnershipExit(candidate)
        BY <3>1, OwnedServiceRankOrderingWellFounded,
           WellFoundedLeadsTo
      <3>3. []AsyncTypeInvariant
        BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
           AsyncStrongTypeProjectsAsyncType, PTL
      <3>4. AsyncTypeInvariant
               /\ gst
               /\ HistoricalProtectedCandidateOwned(candidate)
              => \E rank \in OwnedServiceRankCarrier:
                   HistoricalProtectedOwnedAtServiceRank(candidate, rank)
        BY HistoricalProtectedCandidateHasServiceRank
      <3> QED BY <3>2, <3>3, <3>4, PTL
    <2> QED BY <1>1, <2>1
         DEF HistoricalProtectedCandidateStarvationProperty
  <1> QED BY <1>1

(***************************************************************************
Authenticated CommitQC discovery through the exact historical action.

The discovery prefix can publish only the canonical request outbox, and its
fair action is quantified over every responsive validator.  Preservation of
the pending guard across all unrelated Async steps is kept as a separate
operator below: accepting only enabledness and a one-step effect would be the
classic weak-fairness fallacy.
***************************************************************************)

HistoricalCommitCertificateDiscoveryPending(node) ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ HistoricalCommitCertificateDiscoveryDue(node)

HistoricalCommitCertificateDiscoveryOutcome(node) ==
  \/ NodeHasDecision(node)
  \/ /\ HistoricalRecoveryTarget(node)
        /\ ActiveCommitCertificateRequests(node) # {}

HistoricalCommitCertificateDiscoveryPersistenceObligation ==
  \A node \in Responsive:
    HistoricalCommitCertificateDiscoveryPending(node)
      /\ [AsyncNext]_AsyncAllVars
    => HistoricalCommitCertificateDiscoveryPending(node)'
         \/ HistoricalCommitCertificateDiscoveryOutcome(node)'

HistoricalCommitCertificateDiscoveryPersistenceUnless(node) ==
  [][HistoricalCommitCertificateDiscoveryPending(node)
       /\ ~HistoricalCommitCertificateDiscoveryOutcome(node)
      => HistoricalCommitCertificateDiscoveryPending(node)'
           \/ HistoricalCommitCertificateDiscoveryOutcome(node)']_AsyncAllVars

HistoricalCommitCertificateDiscoveryPersistenceProperty(specification) ==
  specification
    => \A node \in Responsive:
         HistoricalCommitCertificateDiscoveryPersistenceUnless(node)

HistoricalRecoveryTargetRemoteServerInvariant ==
  \A node \in Responsive:
    HistoricalRecoveryTarget(node)
      => CommitCertificateRequestOutbox(node) # {}

HistoricalRecoveryTargetRemoteServerProperty(specification) ==
  specification => []HistoricalRecoveryTargetRemoteServerInvariant

HistoricalCommitCertificateDiscoveryClockProgressProperty(specification) ==
  specification
    => \A node \in Responsive:
         (gst /\ HistoricalRecoveryTarget(node))
           ~> (NodeHasDecision(node)
                \/ /\ HistoricalRecoveryTarget(node)
                      /\ \/ ActiveCommitCertificateRequests(node) # {}
                         \/ asyncNow >= AsyncRoundTimeout)

(***************************************************************************
The historical target set is not a free liveness premise.  A target can be
introduced only by `OpenHistoricalRecovery`, whose source guard names a
different applied current voter.  The one-height frame keeps that roster
fixed, and the only post-GST target removal is the target's exact Apply step.
Consequently every live target has a nonempty canonical remote request
outbox.  These lemmas make that reachable-state fact inductive before it is
used by discovery readiness.
***************************************************************************)

THEOREM AsyncInitEstablishesHistoricalRecoveryTargetRemoteServer ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => HistoricalRecoveryTargetRemoteServerInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       HistoricalRecoveryTargetRemoteServerInvariant,
       HistoricalRecoveryTarget

THEOREM HistoricalRecoveryTargetPersistsUnlessDecision ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalRecoveryTarget(node)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~NodeHasDecision(node)'
    => HistoricalRecoveryTarget(node)'
BY Isa
   DEF HistoricalRecoveryTarget, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep, RunNode,
       RunHistoricalRecoveryNode, RunNodeWork, LocalAdmissionStep,
       SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       FifoRuntimeStep, ExecuteCommand,
       ExecuteApply, OpenHistoricalRecovery, PreGstCrash,
       PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       AsyncSetGST, AsyncAllVars

(***************************************************************************
The target itself is retired only by its exact Apply transition.  Decision
installation does not retire the historical executor: Fetch/Store/Validate
and Apply still need that same joined owner.  This stronger frame is used by
the indexed discovery corridor to carry target ownership across the
clock-or-Decision disjunction without treating Decision as application.
***************************************************************************)

THEOREM HistoricalRecoveryTargetPersistsUnlessApplication ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalRecoveryTarget(node)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~NodeHasApplication(node)'
    => HistoricalRecoveryTarget(node)'
BY IsaT(600)
   DEF HistoricalRecoveryTarget, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep, RunNode,
       RunHistoricalRecoveryNode, RunNodeWork, LocalAdmissionStep,
       SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       FifoRuntimeStep, ExecuteCommand, ExecuteApply,
       OpenHistoricalRecovery, PreGstCrash,
       PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       AsyncSetGST, AsyncAllVars

THEOREM AsyncBracketPreservesHistoricalRecoveryTargetRemoteServer ==
  /\ AsyncStrongTypeInvariant
  /\ HistoricalRecoveryTargetRemoteServerInvariant
  /\ [AsyncNext]_AsyncAllVars
  => HistoricalRecoveryTargetRemoteServerInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              HistoricalRecoveryTargetRemoteServerInvariant,
              [AsyncNext]_AsyncAllVars
         PROVE HistoricalRecoveryTargetRemoteServerInvariant'
    <2>1. /\ context' = context
           /\ CurrentVoters' = CurrentVoters
      BY <1>1, Isa
         DEF AsyncNext, AsyncAllVars, vars, CurrentVoters, CurrentEpoch
    <2>2. ASSUME NEW node \in Responsive,
                  HistoricalRecoveryTarget(node)'
           PROVE CommitCertificateRequestOutbox(node)' # {}
      <3>1. CASE HistoricalRecoveryTarget(node)
        <4>1. CommitCertificateRequestOutbox(node) # {}
          BY <1>1, <2>2, <3>1
             DEF HistoricalRecoveryTargetRemoteServerInvariant
        <4> QED BY <2>1, <4>1, Isa
             DEF CommitCertificateRequestOutbox, AsyncNetworkItem
      <3>2. CASE ~HistoricalRecoveryTarget(node)
        <4>1. (CurrentVoters' \ {node}) # {}
          BY <1>1, <2>1, <2>2, <3>2, Isa
             DEF HistoricalRecoveryTarget, AsyncNext,
                 AsyncNonCrashStep, AsyncRunnerStep,
                 AsyncNonRunnerStep, OpenHistoricalRecovery,
                 HistoricalRecoverySourceReady, RunNode,
                 RunHistoricalRecoveryNode, RunNodeWork,
                 LocalAdmissionStep, SelectedLocalAdmissionAdvance,
                 SerializedLocalPrecedesServeIngressStep,
                 IngressDrainStep, SerializedRunnerRuntimeStep,
                 SerializedRuntimeStep,
                 SerializedRuntimePrecedesServeIngressStep,
                 AsyncServeIngressTargetOnlyTurn, RuntimeStep,
                 ExecuteCommand, ExecuteApply,
                 PreGstCrash, PreGstResponsiveCrash,
                 PreGstResponsiveRestart, PreGstResponsiveReplay,
                 ResetNodeSchedulerForRestart, AsyncAllVars
        <4> QED BY <4>1,
             CommitCertificateRequestOutboxNonemptyIffRemoteVoter
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>2
         DEF HistoricalRecoveryTargetRemoteServerInvariant
  <1> QED BY <1>1

THEOREM AsyncSpecAlwaysHistoricalRecoveryTargetRemoteServer ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []HistoricalRecoveryTargetRemoteServerInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []HistoricalRecoveryTargetRemoteServerInvariant
    <2>1. AsyncInitAt(initialContext)
            => HistoricalRecoveryTargetRemoteServerInvariant
      BY AsyncInitEstablishesHistoricalRecoveryTargetRemoteServer
    <2>2. AsyncSpecAt(initialContext)
             => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>3. /\ AsyncStrongTypeInvariant
           /\ HistoricalRecoveryTargetRemoteServerInvariant
           /\ [AsyncNext]_AsyncAllVars
          => HistoricalRecoveryTargetRemoteServerInvariant'
      BY AsyncBracketPreservesHistoricalRecoveryTargetRemoteServer
    <2> QED BY <2>1, <2>2, <2>3, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM HistoricalRecoveryTargetRemoteServerFromAsyncSpec ==
  \A initialContext:
    HistoricalRecoveryTargetRemoteServerProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysHistoricalRecoveryTargetRemoteServer
   DEF HistoricalRecoveryTargetRemoteServerProperty

(***************************************************************************
Discovery enabledness is persistent, not sampled.  Once the absolute clock
threshold is reached, GST and the frozen roster cannot regress.  A target can
leave only through an exact Decision/Apply outcome, and publication of the
canonical request set is itself the other terminal discovery outcome.
***************************************************************************)

THEOREM HistoricalCommitCertificateDiscoveryPendingUnlessOutcome ==
  \A node \in Responsive:
    /\ HistoricalRecoveryTargetRemoteServerInvariant
    /\ HistoricalCommitCertificateDiscoveryPending(node)
    /\ [AsyncNext]_AsyncAllVars
    => HistoricalCommitCertificateDiscoveryPending(node)'
         \/ HistoricalCommitCertificateDiscoveryOutcome(node)'
PROOF
  <1>1. ASSUME NEW node \in Responsive,
                HistoricalRecoveryTargetRemoteServerInvariant,
                HistoricalCommitCertificateDiscoveryPending(node),
                [AsyncNext]_AsyncAllVars
         PROVE HistoricalCommitCertificateDiscoveryPending(node)'
                 \/ HistoricalCommitCertificateDiscoveryOutcome(node)'
    <2>1. AsyncStrongTypeInvariant'
      BY <1>1, AsyncBracketNextPreservesStrongTypeInvariant
         DEF HistoricalCommitCertificateDiscoveryPending
    <2>2. gst'
      BY <1>1, GstAsyncStepIsMonotone
         DEF HistoricalCommitCertificateDiscoveryPending
    <2>3. asyncNow' >= AsyncRoundTimeout
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         AsyncBracketNextPreservesDiscoveryClockThreshold
         DEF HistoricalCommitCertificateDiscoveryPending,
             HistoricalCommitCertificateDiscoveryDue
    <2>4. HistoricalRecoveryTargetRemoteServerInvariant'
      BY <1>1, AsyncBracketPreservesHistoricalRecoveryTargetRemoteServer
         DEF HistoricalCommitCertificateDiscoveryPending
    <2>5. CASE HistoricalCommitCertificateDiscoveryOutcome(node)'
      BY <2>5
    <2>6. CASE ~HistoricalCommitCertificateDiscoveryOutcome(node)'
      <3>1. /\ ~NodeHasDecision(node)'
             /\ ActiveCommitCertificateRequests(node)' = {}
        BY <2>6 DEF HistoricalCommitCertificateDiscoveryOutcome
      <3>2. HistoricalRecoveryTarget(node)'
        BY <1>1, <3>1,
           HistoricalRecoveryTargetPersistsUnlessDecision
           DEF HistoricalCommitCertificateDiscoveryPending,
               HistoricalCommitCertificateDiscoveryDue
      <3>3. CommitCertificateRequestOutbox(node)' # {}
        BY <2>4, <3>2
           DEF HistoricalRecoveryTargetRemoteServerInvariant
      <3>4. HistoricalCommitCertificateDiscoveryDue(node)'
        BY <2>3, <3>1, <3>2, <3>3
           DEF HistoricalCommitCertificateDiscoveryDue,
               CommitCertificateDiscoveryReady
      <3>5. HistoricalCommitCertificateDiscoveryPending(node)'
        BY <2>1, <2>2, <3>4
           DEF HistoricalCommitCertificateDiscoveryPending
      <3> QED BY <3>5
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM HistoricalCommitCertificateDiscoveryPersistenceFromAsyncSpec ==
  \A initialContext:
    HistoricalCommitCertificateDiscoveryPersistenceProperty(
      AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE HistoricalCommitCertificateDiscoveryPersistenceProperty(
                 AsyncSpecAt(initialContext))
    <2>1. CASE AsyncSpecAt(initialContext)
      <3>1. []HistoricalRecoveryTargetRemoteServerInvariant
        BY <2>1, AsyncSpecAlwaysHistoricalRecoveryTargetRemoteServer
      <3>2. [][AsyncNext]_AsyncAllVars
        BY <2>1 DEF AsyncSpecAt
      <3>3. \A node \in Responsive:
               HistoricalCommitCertificateDiscoveryPersistenceUnless(node)
        BY <3>1, <3>2,
           HistoricalCommitCertificateDiscoveryPendingUnlessOutcome,
           PTL
           DEF HistoricalCommitCertificateDiscoveryPersistenceUnless
      <3> QED BY <2>1, <3>3
           DEF HistoricalCommitCertificateDiscoveryPersistenceProperty
    <2>2. CASE ~AsyncSpecAt(initialContext)
      BY <2>2
         DEF HistoricalCommitCertificateDiscoveryPersistenceProperty
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM HistoricalCommitCertificateDiscoveryReadinessFromClock ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalRecoveryTargetRemoteServerProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalCommitCertificateDiscoveryClockProgressProperty(
         AsyncSpecAt(initialContext))
    => \A node \in Responsive:
         (gst /\ HistoricalRecoveryTarget(node))
           ~> (HistoricalCommitCertificateDiscoveryPending(node)
                \/ HistoricalCommitCertificateDiscoveryOutcome(node))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                HistoricalRecoveryTargetRemoteServerProperty(
                  AsyncSpecAt(initialContext)),
                HistoricalCommitCertificateDiscoveryClockProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE \A node \in Responsive:
                 (gst /\ HistoricalRecoveryTarget(node))
                   ~> (HistoricalCommitCertificateDiscoveryPending(node)
                        \/ HistoricalCommitCertificateDiscoveryOutcome(node))
    <2>1. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>2. []HistoricalRecoveryTargetRemoteServerInvariant
      BY <1>1 DEF HistoricalRecoveryTargetRemoteServerProperty
    <2>3. \A node \in Responsive:
             (gst /\ HistoricalRecoveryTarget(node))
               ~> (NodeHasDecision(node)
                    \/ /\ HistoricalRecoveryTarget(node)
                          /\ \/ ActiveCommitCertificateRequests(node) # {}
                             \/ asyncNow >= AsyncRoundTimeout)
      BY <1>1
         DEF HistoricalCommitCertificateDiscoveryClockProgressProperty
    <2>4. \A node \in Responsive:
             /\ AsyncStrongTypeInvariant
             /\ gst
             /\ HistoricalRecoveryTargetRemoteServerInvariant
             /\ HistoricalRecoveryTarget(node)
             /\ asyncNow >= AsyncRoundTimeout
             /\ ~NodeHasDecision(node)
             /\ ActiveCommitCertificateRequests(node) = {}
             => HistoricalCommitCertificateDiscoveryPending(node)
      BY Isa
         DEF HistoricalCommitCertificateDiscoveryPending,
             HistoricalCommitCertificateDiscoveryDue,
             CommitCertificateDiscoveryReady,
             HistoricalRecoveryTargetRemoteServerInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF HistoricalCommitCertificateDiscoveryOutcome
  <1> QED BY <1>1

THEOREM DirectHistoricalCommitCertificateDiscoveryPublishes ==
  \A node \in ValidatorIds:
    DirectHistoricalCommitCertificateDiscoveryStep(node)
      => /\ HistoricalRecoveryTarget(node)'
         /\ ActiveCommitCertificateRequests(node)' # {}
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                DirectHistoricalCommitCertificateDiscoveryStep(node)
         PROVE /\ HistoricalRecoveryTarget(node)'
               /\ ActiveCommitCertificateRequests(node)' # {}
    <2>1. /\ HistoricalRecoveryTarget(node)
           /\ CommitCertificateRequestOutbox(node) # {}
           /\ asyncActiveRequests' =
                asyncActiveRequests
                  \cup CommitCertificateRequestOutbox(node)
           /\ asyncHistoricalRecoveryTargets' =
                asyncHistoricalRecoveryTargets
      BY <1>1
         DEF DirectHistoricalCommitCertificateDiscoveryStep,
             HistoricalCommitCertificateDiscoveryDue,
             CommitCertificateDiscoveryReady,
             CommitCertificateDiscoveryStepWork,
             PublishCommitCertificateRequests,
             HistoricalRecoveryTarget
    <2>2. \A item \in CommitCertificateRequestOutbox(node):
             /\ item.source = node
             /\ item.kind = "CommitCertificateRequest"
      BY Isa DEF CommitCertificateRequestOutbox, AsyncNetworkItem
    <2> QED BY <2>1, <2>2, Isa
         DEF HistoricalRecoveryTarget,
             ActiveCommitCertificateRequests
  <1> QED BY <1>1

THEOREM HistoricalCommitCertificateDiscoveryPrefixIsEnabled ==
  \A node \in ValidatorIds:
    HistoricalCommitCertificateDiscoveryDue(node)
      => ENABLED DirectHistoricalCommitCertificateDiscoveryStep(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                HistoricalCommitCertificateDiscoveryDue(node)
         PROVE ENABLED DirectHistoricalCommitCertificateDiscoveryStep(node)
    <2>1. ENABLED DirectHistoricalCommitCertificateDiscoveryStep(node)
      BY <1>1, ExpandENABLED, Isa
         DEF DirectHistoricalCommitCertificateDiscoveryStep,
             CommitCertificateDiscoveryStepWork,
             PublishCommitCertificateRequests, LeaveCausalQueues,
             AsyncIoVars, AsyncDeferredVars, vars
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM HistoricalCommitCertificateDiscoveryPendingEnablesFairPrefix ==
  \A node \in Responsive:
    HistoricalCommitCertificateDiscoveryPending(node)
      => ENABLED
           <<PostGstHistoricalCommitCertificateDiscovery(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW node \in Responsive,
                HistoricalCommitCertificateDiscoveryPending(node)
         PROVE ENABLED
                 <<PostGstHistoricalCommitCertificateDiscovery(node)>>_(
                   AsyncAllVars)
    <2>1. node \in ValidatorIds
      BY <1>1, HistoricalRecoveryTargetsAreValidators
         DEF HistoricalCommitCertificateDiscoveryPending,
             HistoricalCommitCertificateDiscoveryDue,
             AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety
    <2>2. ENABLED DirectHistoricalCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>1,
         HistoricalCommitCertificateDiscoveryPrefixIsEnabled
         DEF HistoricalCommitCertificateDiscoveryPending
    <2>3. DirectHistoricalCommitCertificateDiscoveryStep(node) \in BOOLEAN
      BY Isa DEF DirectHistoricalCommitCertificateDiscoveryStep
    <2>4. <<PostGstHistoricalCommitCertificateDiscovery(node)>>_AsyncAllVars
             \in BOOLEAN
      BY Isa DEF PostGstHistoricalCommitCertificateDiscovery
    <2>5. DirectHistoricalCommitCertificateDiscoveryStep(node)
             => <<PostGstHistoricalCommitCertificateDiscovery(node)>>_(
                  AsyncAllVars)
      BY <1>1, <2>1,
         DirectHistoricalCommitCertificateDiscoveryPublishes, Isa
         DEF HistoricalCommitCertificateDiscoveryPending,
             PostGstHistoricalCommitCertificateDiscovery,
             AsyncAllVars
    <2>6. ENABLED DirectHistoricalCommitCertificateDiscoveryStep(node)
             => ENABLED
                  <<PostGstHistoricalCommitCertificateDiscovery(node)>>_(
                    AsyncAllVars)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

THEOREM HistoricalCommitCertificateDiscoveryFairStepPublishes ==
  \A node \in Responsive:
    /\ HistoricalCommitCertificateDiscoveryPending(node)
    /\ <<PostGstHistoricalCommitCertificateDiscovery(node)>>_AsyncAllVars
    => HistoricalCommitCertificateDiscoveryOutcome(node)'
BY DirectHistoricalCommitCertificateDiscoveryPublishes, Isa
   DEF HistoricalCommitCertificateDiscoveryPending,
       HistoricalCommitCertificateDiscoveryOutcome,
       PostGstHistoricalCommitCertificateDiscovery,
       AsyncAllVars

THEOREM FairHistoricalCommitCertificateDiscoveryFromPersistence ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalCommitCertificateDiscoveryPersistenceProperty(
         AsyncSpecAt(initialContext))
    => \A node \in Responsive:
         HistoricalCommitCertificateDiscoveryPending(node)
           ~> HistoricalCommitCertificateDiscoveryOutcome(node)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                HistoricalCommitCertificateDiscoveryPersistenceProperty(
                  AsyncSpecAt(initialContext))
         PROVE \A node \in Responsive:
                 HistoricalCommitCertificateDiscoveryPending(node)
                   ~> HistoricalCommitCertificateDiscoveryOutcome(node)
    <2>1. ASSUME NEW node \in Responsive
           PROVE HistoricalCommitCertificateDiscoveryPending(node)
                   ~> HistoricalCommitCertificateDiscoveryOutcome(node)
      <3>1. (HistoricalCommitCertificateDiscoveryPending(node)
                /\ ~HistoricalCommitCertificateDiscoveryOutcome(node))
               => ENABLED
                    <<PostGstHistoricalCommitCertificateDiscovery(node)>>_(
                      AsyncAllVars)
        BY <2>1,
           HistoricalCommitCertificateDiscoveryPendingEnablesFairPrefix
      <3>2. (HistoricalCommitCertificateDiscoveryPending(node)
                /\ ~HistoricalCommitCertificateDiscoveryOutcome(node)
                /\ <<PostGstHistoricalCommitCertificateDiscovery(node)>>_(
                     AsyncAllVars))
               => HistoricalCommitCertificateDiscoveryOutcome(node)'
        BY <2>1, HistoricalCommitCertificateDiscoveryFairStepPublishes
      <3>3. HistoricalCommitCertificateDiscoveryPersistenceUnless(node)
        BY <1>1, <2>1
           DEF HistoricalCommitCertificateDiscoveryPersistenceProperty
      <3>4. WF_AsyncAllVars(
               PostGstHistoricalCommitCertificateDiscovery(node))
        BY <1>1, <2>1 DEF AsyncSpecAt, AsyncFairnessAt
      <3>5. [][AsyncNext]_AsyncAllVars
        BY <1>1 DEF AsyncSpecAt
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5, PTL
    <2> QED BY <2>1
  <1> QED BY <1>1

(***************************************************************************
Concrete authenticated request-to-Decision leaves.

These predicates name the exact production owners in order: the retained
active request, transport/ingress/Serve ownership, the authenticated response,
and the three reducer candidates which import, begin, and persist Decision.
No leaf below states target-to-Decision directly.
***************************************************************************)

HistoricalCommitCertificateRequestScheduled(node) ==
  /\ HistoricalRecoveryTarget(node)
  /\ \E request \in ActiveCommitCertificateRequests(node):
       ItemScheduled(request)

HistoricalCommitCertificateResponseScheduled(node) ==
  /\ HistoricalRecoveryTarget(node)
  /\ \E response \in AsyncNetworkItems:
       /\ response.kind = "CommitCertificateResponse"
       /\ response.envelope.recipient = node
       /\ CommitCertificateResponseAuthorized(response)
       /\ ItemScheduled(response)

HistoricalCommitDecisionDirectEvidence(candidate, qc) ==
  /\ candidate.evidence \in asyncSentItems
  /\ candidate.evidence.kind = "CommitQC"
  /\ candidate.evidence.envelope = QcEnvelope(candidate.node, qc)
  /\ candidate.causalOrigin =
       AsyncDeliveryCandidateCausalOriginAt(candidate.evidence, context)

HistoricalCommitDecisionResponseEvidence(candidate, qc) ==
  /\ candidate.evidence \in asyncSentItems
  /\ candidate.evidence.kind = "CommitCertificateResponse"
  /\ candidate.evidence.envelope.recipient = candidate.node
  /\ candidate.evidence.envelope.qc = qc
  /\ CommitCertificateRequestAuthorized(
       candidate.evidence.envelope.request)
  /\ candidate.causalOrigin =
       AsyncCommitCertificateResponseCandidateCausalOriginAt(
         candidate.evidence, context)

(***************************************************************************
The ingress DeliverQC candidate carries the authenticated CommitQC as its
`item`.  Every reducer successor deliberately replaces `item` with
`NoAsyncItem` while retaining the immutable evidence and causal origin.  The
historical owner must follow that real representation; requiring a CommitQC
item for BeginDecision or PersistDecision would describe no reachable causal
successor and could turn a proofless leaf into an impossible antecedent.
***************************************************************************)
HistoricalCommitDecisionCandidateOwned(node, kind) ==
  \E candidate \in AsyncCandidateSet, qc \in commitQCs:
    /\ candidate.node = node
    /\ candidate.kind = kind
    /\ kind \in {"DeliverQC", "BeginDecision", "PersistDecision"}
    /\ qc.context = context
    /\ qc.phase = "Commit"
    /\ candidate.consumerContext = context
    /\ candidate.view = qc.view
    /\ candidate.subject = qc.subject
    /\ HistoricalProtectedCandidateOwned(candidate)
    /\ \/ HistoricalCommitDecisionDirectEvidence(candidate, qc)
       \/ HistoricalCommitDecisionResponseEvidence(candidate, qc)
    /\ IF kind = "DeliverQC"
       THEN candidate.item =
              IF candidate.evidence.kind = "CommitQC"
              THEN candidate.evidence
              ELSE DiscoveredCommitQcItem(candidate.evidence)
       ELSE candidate.item = NoAsyncItem

HistoricalActiveRequestRetransmissionProgressLeaf(specification) ==
  specification
    => \A node \in Responsive:
         (gst
           /\ HistoricalRecoveryTarget(node)
           /\ ActiveCommitCertificateRequests(node) # {})
           ~> (NodeHasDecision(node)
                \/ HistoricalCommitCertificateRequestScheduled(node))

HistoricalCommitRequestServeProgressLeaf(specification) ==
  StarvationFreedomProperty(specification)
    => (specification
          => \A node \in Responsive:
               (gst /\ HistoricalCommitCertificateRequestScheduled(node))
                 ~> (NodeHasDecision(node)
                      \/ HistoricalCommitCertificateResponseScheduled(node)))

HistoricalCommitResponseAdmissionProgressLeaf(specification) ==
  specification
    => \A node \in Responsive:
         (gst /\ HistoricalCommitCertificateResponseScheduled(node))
           ~> (NodeHasDecision(node)
                \/ HistoricalCommitDecisionCandidateOwned(
                     node, "DeliverQC"))

HistoricalCommitDeliveryProgressLeaf(specification) ==
  HistoricalProtectedCandidateStarvationProperty(specification)
    => (specification
          => \A node \in Responsive:
               (gst
                 /\ HistoricalCommitDecisionCandidateOwned(node, "DeliverQC"))
                 ~> (NodeHasDecision(node)
                      \/ HistoricalCommitDecisionCandidateOwned(
                           node, "BeginDecision")))

HistoricalBeginDecisionProgressLeaf(specification) ==
  HistoricalProtectedCandidateStarvationProperty(specification)
    => (specification
          => \A node \in Responsive:
               (gst
                 /\ HistoricalCommitDecisionCandidateOwned(
                      node, "BeginDecision"))
                 ~> (NodeHasDecision(node)
                      \/ HistoricalCommitDecisionCandidateOwned(
                           node, "PersistDecision")))

HistoricalPersistDecisionProgressLeaf(specification) ==
  HistoricalProtectedCandidateStarvationProperty(specification)
    => (specification
          => \A node \in Responsive:
               (gst
                 /\ HistoricalCommitDecisionCandidateOwned(
                      node, "PersistDecision"))
                 ~> NodeHasDecision(node))

HistoricalCommitCertificateConcreteLeafProperties(specification) ==
  /\ HistoricalActiveRequestRetransmissionProgressLeaf(specification)
  /\ HistoricalCommitRequestServeProgressLeaf(specification)
  /\ HistoricalCommitResponseAdmissionProgressLeaf(specification)
  /\ HistoricalCommitDeliveryProgressLeaf(specification)
  /\ HistoricalBeginDecisionProgressLeaf(specification)
  /\ HistoricalPersistDecisionProgressLeaf(specification)

(***************************************************************************
Concrete historical Decision-to-application leaves.

The frontier uses the exact persisted Decision QC and the existing
recovery-aware candidate predicates.  Its finite acyclic order is FetchBody,
optional RequestCertifiedBody, authenticated certified response,
FetchCertifiedBody, StoreBody, ValidateBody, and Apply.
***************************************************************************)

HistoricalDecisionRecordMatches(node, decision) ==
  /\ decision \in decisions
  /\ decision.node = node
  /\ decision.qc.context = context
  /\ decision.qc.phase = "Commit"

HistoricalDecisionPipelineKindOwned(node, kind) ==
  /\ HistoricalRecoveryTarget(node)
  /\ \E decision \in decisions:
       /\ HistoricalDecisionRecordMatches(node, decision)
       /\ DecisionPipelineKindOwned(node, decision.qc, kind)

HistoricalDecisionCertifiedRequestActive(node) ==
  /\ HistoricalRecoveryTarget(node)
  /\ \E decision \in decisions:
       /\ HistoricalDecisionRecordMatches(node, decision)
       /\ DecisionCertifiedRequestActive(node, decision.qc)

HistoricalDecisionRecoveryFrontier(node) ==
  \/ NodeHasApplication(node)
  \/ HistoricalDecisionPipelineKindOwned(node, "FetchBody")
  \/ HistoricalDecisionPipelineKindOwned(node, "RequestCertifiedBody")
  \/ HistoricalDecisionCertifiedRequestActive(node)
  \/ HistoricalDecisionPipelineKindOwned(node, "FetchCertifiedBody")
  \/ HistoricalDecisionPipelineKindOwned(node, "StoreBody")
  \/ HistoricalDecisionPipelineKindOwned(node, "ValidateBody")
  \/ HistoricalDecisionPipelineKindOwned(node, "Apply")

HistoricalDecisionFrontierAvailabilityProperty(specification) ==
  specification
    => []\A node \in Responsive:
         (gst
           /\ HistoricalRecoveryTarget(node)
           /\ NodeHasDecision(node))
           => HistoricalDecisionRecoveryFrontier(node)

HistoricalDecisionFetchProgressLeaf(specification) ==
  HistoricalProtectedCandidateStarvationProperty(specification)
    => (specification
          => \A node \in Responsive:
               (gst
                 /\ HistoricalDecisionPipelineKindOwned(node, "FetchBody"))
                 ~> (NodeHasApplication(node)
                      \/ HistoricalDecisionPipelineKindOwned(
                           node, "RequestCertifiedBody")
                      \/ HistoricalDecisionCertifiedRequestActive(node)
                      \/ HistoricalDecisionPipelineKindOwned(
                           node, "ValidateBody")))

HistoricalDecisionRequestBodyProgressLeaf(specification) ==
  HistoricalProtectedCandidateStarvationProperty(specification)
    => (specification
          => \A node \in Responsive:
               (gst
                 /\ HistoricalDecisionPipelineKindOwned(
                      node, "RequestCertifiedBody"))
                 ~> (NodeHasApplication(node)
                      \/ HistoricalDecisionCertifiedRequestActive(node)))

HistoricalDecisionCertifiedResponseProgressLeaf(specification) ==
  (/\ StarvationFreedomProperty(specification)
   /\ HistoricalProtectedCandidateStarvationProperty(specification))
    => (specification
          => \A node \in Responsive:
               (gst /\ HistoricalDecisionCertifiedRequestActive(node))
                 ~> (NodeHasApplication(node)
                      \/ HistoricalDecisionPipelineKindOwned(
                           node, "FetchCertifiedBody")))

HistoricalDecisionFetchCertifiedProgressLeaf(specification) ==
  HistoricalProtectedCandidateStarvationProperty(specification)
    => (specification
          => \A node \in Responsive:
               (gst
                 /\ HistoricalDecisionPipelineKindOwned(
                      node, "FetchCertifiedBody"))
                 ~> (NodeHasApplication(node)
                      \/ HistoricalDecisionPipelineKindOwned(
                           node, "StoreBody")))

HistoricalDecisionStoreProgressLeaf(specification) ==
  HistoricalProtectedCandidateStarvationProperty(specification)
    => (specification
          => \A node \in Responsive:
               (gst
                 /\ HistoricalDecisionPipelineKindOwned(node, "StoreBody"))
                 ~> (NodeHasApplication(node)
                      \/ HistoricalDecisionPipelineKindOwned(
                           node, "ValidateBody")))

HistoricalDecisionValidateProgressLeaf(specification) ==
  HistoricalProtectedCandidateStarvationProperty(specification)
    => (specification
          => \A node \in Responsive:
               (gst
                 /\ HistoricalDecisionPipelineKindOwned(node, "ValidateBody"))
                 ~> (NodeHasApplication(node)
                      \/ HistoricalDecisionPipelineKindOwned(node, "Apply")))

HistoricalDecisionApplyProgressLeaf(specification) ==
  HistoricalProtectedCandidateStarvationProperty(specification)
    => (specification
          => \A node \in Responsive:
               (gst /\ HistoricalDecisionPipelineKindOwned(node, "Apply"))
                 ~> NodeHasApplication(node))

HistoricalDecisionConcreteLeafProperties(specification) ==
  /\ HistoricalDecisionFetchProgressLeaf(specification)
  /\ HistoricalDecisionRequestBodyProgressLeaf(specification)
  /\ HistoricalDecisionCertifiedResponseProgressLeaf(specification)
  /\ HistoricalDecisionFetchCertifiedProgressLeaf(specification)
  /\ HistoricalDecisionStoreProgressLeaf(specification)
  /\ HistoricalDecisionValidateProgressLeaf(specification)
  /\ HistoricalDecisionApplyProgressLeaf(specification)

ResponsiveDecisionServiceOwnershipInvariant ==
  \A node \in Responsive:
    (gst /\ NodeHasDecision(node) /\ ~NodeHasApplication(node))
      => \/ node \in AsyncCurrentResponsiveVoters
         \/ HistoricalRecoveryTarget(node)

(***************************************************************************
Reachable Decision-service ownership.

The temporal interface above used to leave ownership as a caller premise.
That is too weak for the indexed historical authority proof: after a typed
archive appears, an already-persisted Decision must still name a runner which
is covered by concrete fairness.  The stronger invariant below is independent
of GST.  A current-context Decision is created only by the selected serialized
runner; that runner is either a current responsive voter or an exact
historical target.  The only action which removes the latter owner is the same
exact Apply which installs the terminal application.

The proof deliberately classifies new durable Decision records rather than
assuming that every responsive validator belongs to the voting roster.
***************************************************************************)

ReachableResponsiveDecisionServiceOwnershipInvariant ==
  \A node \in Responsive:
    (NodeHasDecision(node) /\ ~NodeHasApplication(node))
      => \/ node \in AsyncCurrentResponsiveVoters
         \/ HistoricalRecoveryTarget(node)

HistoricalNewDecisionOwnedBy(node) ==
  \A decision \in decisions' \ decisions:
    decision.node = node

THEOREM HistoricalExecuteCommandCreatesDecisionOnlyForCommandOwner ==
  \A command:
    ExecuteCommand(command)
      => HistoricalNewDecisionOwnedBy(command.node)
BY IsaT(240)
   DEF HistoricalNewDecisionOwnedBy,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       CommandMatches, AssembleLocalBody, BeginLocalProposal,
       PersistProposal, FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, ValidateDecidedBody, ValidateLockedBody, RejectBody,
       BeginPrepare, PersistPrepare, BeginObservePrepare,
       PersistObservePrepare, BeginLockCommit, PersistLockCommit,
       FormCommitQC, BeginDecision, PersistTimeout, FormTC,
       BeginInstallTC, FetchCertifiedBody,
       AcceptCertifiedResponseCapability, InstallCertifiedBodyEffect,
       ExecuteDecisionFetch, ExecuteSignProposal, ExecuteSignVote,
       ExecuteFormPrepareQC, ExecuteSignTimeout, ExecutePersistInstall,
       ExecutePersistDecision, ExecuteRequestCertifiedBody,
       ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk,
       CompleteProposalSignature, CompleteVoteSignature,
       CompleteTimeoutSignature, FormPrepareQC, PersistInstallTC,
       PersistDecision, ApplyDecision, DeliverProposal, DeliverVote,
       DeliverQC, DeliverTimeout, DeliverTC, vars

THEOREM HistoricalFifoRuntimeCreatesDecisionOnlyForRunner ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ FifoRuntimeStep(node)
    => HistoricalNewDecisionOwnedBy(node)
BY RuntimeSelectedCommandsAreTyped,
   HistoricalExecuteCommandCreatesDecisionOnlyForCommandOwner,
   AsyncStrongTypeProjectsAsyncType, IsaT(90)
   DEF HistoricalNewDecisionOwnedBy, FifoRuntimeStep,
       DeferCommand, DiscardCommand, vars

THEOREM HistoricalDeferredDrainCreatesDecisionOnlyForRunner ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ DeferredDrainStep(node)
    => HistoricalNewDecisionOwnedBy(node)
BY RuntimeSelectedCommandsAreTyped,
   HistoricalExecuteCommandCreatesDecisionOnlyForCommandOwner,
   AsyncStrongTypeProjectsAsyncType, IsaT(90)
   DEF HistoricalNewDecisionOwnedBy, DeferredDrainStep,
       DiscardCommand, vars

THEOREM HistoricalRuntimeCreatesDecisionOnlyForRunner ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ RuntimeStep(node)
    => HistoricalNewDecisionOwnedBy(node)
BY HistoricalFifoRuntimeCreatesDecisionOnlyForRunner,
   HistoricalDeferredDrainCreatesDecisionOnlyForRunner, IsaT(120)
   DEF HistoricalNewDecisionOwnedBy, RuntimeStep,
       DeferredTagStep, DeferredTimeoutStep, DeferredRetransmitStep,
       DirectTimeoutStep, DirectRetransmitStep, IdleRuntimeStep,
       BeginTimeout, vars

THEOREM HistoricalRunNodeWorkCreatesDecisionOnlyForRunner ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ RunNodeWork(node)
    => HistoricalNewDecisionOwnedBy(node)
BY HistoricalRuntimeCreatesDecisionOnlyForRunner, IsaT(90)
   DEF HistoricalNewDecisionOwnedBy, RunNodeWork,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       AdmitProducerCompletion, AdmitCausalHead,
       UpdateLocalAdmissionMetadata, DrainFairIngressSelected, vars

THEOREM HistoricalAsyncNextCreatesDecisionOnlyForServiceOwner ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncNext
  => \A decision \in decisions' \ decisions:
       \/ decision.node \in AsyncCurrentResponsiveVoters'
       \/ HistoricalRecoveryTarget(decision.node)'
BY HistoricalRunNodeWorkCreatesDecisionOnlyForRunner, IsaT(600)
   DEF HistoricalNewDecisionOwnedBy,
       HistoricalRecoveryTarget,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, ExecuteCommand, ExecutePersistDecision,
       PersistDecision, ExecuteApply, ApplyDecision,
       OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       AsyncNetworkStep, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       RearmResponsiveRecovery, ResetNodeSchedulerForRestart,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars, vars

THEOREM HistoricalBracketCreatesDecisionOnlyForServiceOwner ==
  /\ AsyncStrongTypeInvariant
  /\ [AsyncNext]_AsyncAllVars
  => \A decision \in decisions' \ decisions:
       \/ decision.node \in AsyncCurrentResponsiveVoters'
       \/ HistoricalRecoveryTarget(decision.node)'
PROOF
  <1>1. CASE AsyncNext
    BY <1>1, HistoricalAsyncNextCreatesDecisionOnlyForServiceOwner
  <1>2. CASE UNCHANGED AsyncAllVars
    BY <1>2, Isa
       DEF HistoricalRecoveryTarget,
           AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
           AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars, vars
  <1> QED BY <1>1, <1>2

THEOREM HistoricalDecisionTargetPersistsUntilApplication ==
  \A node:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalRecoveryTarget(node)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~NodeHasApplication(node)'
    => HistoricalRecoveryTarget(node)'
BY IsaT(600)
   DEF HistoricalRecoveryTarget, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep, RunNode,
       RunHistoricalRecoveryNode, RunNodeWork, LocalAdmissionStep,
       SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep, ExecuteCommand,
       ExecuteApply, ApplyDecision, OpenHistoricalRecovery,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart, AsyncSetGST,
       NodeHasApplication, AsyncAllVars

THEOREM HistoricalBracketKeepsResponsiveVoterSet ==
  [AsyncNext]_AsyncAllVars
    => /\ context' = context
       /\ AsyncCurrentResponsiveVoters' = AsyncCurrentResponsiveVoters
BY Isa
   DEF AsyncNext, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars, vars

THEOREM AsyncInitEstablishesReachableResponsiveDecisionServiceOwnership ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => ReachableResponsiveDecisionServiceOwnershipInvariant
BY BootstrapParentContextPrecedes, IsaT(120)
   DEF ReachableResponsiveDecisionServiceOwnershipInvariant,
       AsyncInitAt, AsyncBaseInitAt, InitAt,
       NodeHasDecision, NodeHasApplication,
       HistoricalRecoveryTarget,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       BootstrapParentDecision, BootstrapParentCommitQC,
       BootstrapParentContext, ContextRecord

THEOREM AsyncBracketPreservesReachableResponsiveDecisionServiceOwnership ==
  /\ AsyncStrongTypeInvariant
  /\ ReachableResponsiveDecisionServiceOwnershipInvariant
  /\ [AsyncNext]_AsyncAllVars
  => ReachableResponsiveDecisionServiceOwnershipInvariant'
BY HistoricalBracketCreatesDecisionOnlyForServiceOwner,
   HistoricalDecisionTargetPersistsUntilApplication,
   HistoricalBracketKeepsResponsiveVoterSet, IsaT(300)
   DEF ReachableResponsiveDecisionServiceOwnershipInvariant,
       NodeHasDecision, NodeHasApplication,
       HistoricalRecoveryTarget,
       AsyncCurrentResponsiveVoters

THEOREM ReachableResponsiveDecisionServiceOwnershipObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []ReachableResponsiveDecisionServiceOwnershipInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []ReachableResponsiveDecisionServiceOwnershipInvariant
    <2>1. AsyncInitAt(initialContext)
             => ReachableResponsiveDecisionServiceOwnershipInvariant
      BY AsyncInitEstablishesReachableResponsiveDecisionServiceOwnership
    <2>2. AsyncSpecAt(initialContext)
             => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>3. /\ AsyncStrongTypeInvariant
           /\ ReachableResponsiveDecisionServiceOwnershipInvariant
           /\ [AsyncNext]_AsyncAllVars
          => ReachableResponsiveDecisionServiceOwnershipInvariant'
      BY AsyncBracketPreservesReachableResponsiveDecisionServiceOwnership
    <2> QED BY <2>1, <2>2, <2>3, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM ReachableOwnershipImpliesPostGstDecisionServiceOwnership ==
  ReachableResponsiveDecisionServiceOwnershipInvariant
    => ResponsiveDecisionServiceOwnershipInvariant
BY Isa
   DEF ReachableResponsiveDecisionServiceOwnershipInvariant,
       ResponsiveDecisionServiceOwnershipInvariant

THEOREM ResponsiveDecisionServiceOwnershipObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []ResponsiveDecisionServiceOwnershipInvariant
BY ReachableResponsiveDecisionServiceOwnershipObligation,
   ReachableOwnershipImpliesPostGstDecisionServiceOwnership, PTL

ResponsiveDecisionServiceOwnershipProperty(specification) ==
  specification => []ResponsiveDecisionServiceOwnershipInvariant

HistoricalRecoveryAsyncTemporalClosurePremises(specification) ==
  /\ HistoricalCommitCertificateDiscoveryPersistenceProperty(specification)
  /\ HistoricalRecoveryTargetRemoteServerProperty(specification)
  /\ HistoricalCommitCertificateDiscoveryClockProgressProperty(specification)
  /\ HistoricalProtectedServiceRankLeafProperties(specification)
  /\ HistoricalCommitCertificateConcreteLeafProperties(specification)
  /\ HistoricalDecisionFrontierAvailabilityProperty(specification)
  /\ HistoricalDecisionConcreteLeafProperties(specification)
  /\ ResponsiveDecisionServiceOwnershipProperty(specification)
  /\ ApplicationCompletionProgressProperty(specification)

(***************************************************************************
Exact premise accounting.

The first three closure premises are consequences of the actual Async
transition system: target creation records a nonempty canonical remote
CommitQC request outbox, the discovery guard persists until either the
request set is published or Decision is installed, and every reachable
unapplied responsive Decision retains either its current-voter runner or its
exact historical target.  They therefore must not remain hidden among the
temporal assumptions supplied to the corridor.

The other six predicates are the smallest remaining service boundary in
this child.  Five are historical-service predicates.  The sixth is the
current-voter application-completion property supplied by the higher
proof-bearing closure; keeping it explicit prevents this lower historical
module from importing the post-debt DecisionApplication facade and creating a
dependency cycle.  In particular, the clock predicate is not inferred from
weak fairness of `AsyncTick`: an overdue packet or local-service owner can
disable that action, so a proof must first discharge the concrete terminating
work/rank corridor.  Keeping this partition exact prevents a future proof from
silently replacing one of those dependencies with target-to-Decision itself.
***************************************************************************)

HistoricalRecoveryAsyncModelDerivedPremises(specification) ==
  /\ HistoricalCommitCertificateDiscoveryPersistenceProperty(specification)
  /\ HistoricalRecoveryTargetRemoteServerProperty(specification)
  /\ ResponsiveDecisionServiceOwnershipProperty(specification)

HistoricalRecoveryAsyncRemainingCorridorPremises(specification) ==
  /\ HistoricalCommitCertificateDiscoveryClockProgressProperty(specification)
  /\ HistoricalProtectedServiceRankLeafProperties(specification)
  /\ HistoricalCommitCertificateConcreteLeafProperties(specification)
  /\ HistoricalDecisionFrontierAvailabilityProperty(specification)
  /\ HistoricalDecisionConcreteLeafProperties(specification)
  /\ ApplicationCompletionProgressProperty(specification)

HistoricalRecoveryAsyncRemainingCorridorObligation ==
  \A initialContext:
    HistoricalRecoveryAsyncRemainingCorridorPremises(
      AsyncSpecAt(initialContext))

THEOREM HistoricalRecoveryAsyncModelDerivedPremisesFromAsyncSpec ==
  \A initialContext:
    HistoricalRecoveryAsyncModelDerivedPremises(
      AsyncSpecAt(initialContext))
BY HistoricalCommitCertificateDiscoveryPersistenceFromAsyncSpec,
   HistoricalRecoveryTargetRemoteServerFromAsyncSpec,
   ResponsiveDecisionServiceOwnershipObligation
   DEF HistoricalRecoveryAsyncModelDerivedPremises

THEOREM HistoricalRecoveryAsyncTemporalClosurePremisesPartition ==
  \A specification:
    HistoricalRecoveryAsyncTemporalClosurePremises(specification)
      <=> /\ HistoricalRecoveryAsyncModelDerivedPremises(specification)
          /\ HistoricalRecoveryAsyncRemainingCorridorPremises(specification)
BY DEF HistoricalRecoveryAsyncTemporalClosurePremises,
       HistoricalRecoveryAsyncModelDerivedPremises,
       HistoricalRecoveryAsyncRemainingCorridorPremises

THEOREM HistoricalActiveCommitCertificateRequestReachesDecision ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalProtectedServiceRankLeafProperties(
         AsyncSpecAt(initialContext))
    /\ HistoricalCommitCertificateConcreteLeafProperties(
         AsyncSpecAt(initialContext))
    => \A node \in Responsive:
         (gst
           /\ HistoricalRecoveryTarget(node)
           /\ ActiveCommitCertificateRequests(node) # {})
           ~> NodeHasDecision(node)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
                HistoricalProtectedServiceRankLeafProperties(
                  AsyncSpecAt(initialContext)),
                HistoricalCommitCertificateConcreteLeafProperties(
                  AsyncSpecAt(initialContext))
         PROVE \A node \in Responsive:
                 (gst
                   /\ HistoricalRecoveryTarget(node)
                   /\ ActiveCommitCertificateRequests(node) # {})
                   ~> NodeHasDecision(node)
    <2>1. HistoricalProtectedServiceRankProgressProperty(
             AsyncSpecAt(initialContext))
      BY <1>1, HistoricalProtectedServiceRankProgressFromStageLeaves
    <2>2. HistoricalProtectedCandidateStarvationProperty(
             AsyncSpecAt(initialContext))
      BY <1>1, <2>1,
         HistoricalProtectedServiceRankProgressImpliesStarvation
    <2>3. StarvationFreedomProperty(AsyncSpecAt(initialContext))
      BY StarvationFreedomObligation
    <2>4. \A node \in Responsive:
             (gst
               /\ HistoricalRecoveryTarget(node)
               /\ ActiveCommitCertificateRequests(node) # {})
               ~> (NodeHasDecision(node)
                    \/ HistoricalCommitCertificateRequestScheduled(node))
      BY <1>1
         DEF HistoricalCommitCertificateConcreteLeafProperties,
             HistoricalActiveRequestRetransmissionProgressLeaf
    <2>5. \A node \in Responsive:
             (gst /\ HistoricalCommitCertificateRequestScheduled(node))
               ~> (NodeHasDecision(node)
                    \/ HistoricalCommitCertificateResponseScheduled(node))
      BY <1>1, <2>3
         DEF HistoricalCommitCertificateConcreteLeafProperties,
             HistoricalCommitRequestServeProgressLeaf
    <2>6. \A node \in Responsive:
             (gst /\ HistoricalCommitCertificateResponseScheduled(node))
               ~> (NodeHasDecision(node)
                    \/ HistoricalCommitDecisionCandidateOwned(
                         node, "DeliverQC"))
      BY <1>1
         DEF HistoricalCommitCertificateConcreteLeafProperties,
             HistoricalCommitResponseAdmissionProgressLeaf
    <2>7. \A node \in Responsive:
             (gst
               /\ HistoricalCommitDecisionCandidateOwned(node, "DeliverQC"))
               ~> (NodeHasDecision(node)
                    \/ HistoricalCommitDecisionCandidateOwned(
                         node, "BeginDecision"))
      BY <1>1, <2>2
         DEF HistoricalCommitCertificateConcreteLeafProperties,
             HistoricalCommitDeliveryProgressLeaf
    <2>8. \A node \in Responsive:
             (gst
               /\ HistoricalCommitDecisionCandidateOwned(
                    node, "BeginDecision"))
               ~> (NodeHasDecision(node)
                    \/ HistoricalCommitDecisionCandidateOwned(
                         node, "PersistDecision"))
      BY <1>1, <2>2
         DEF HistoricalCommitCertificateConcreteLeafProperties,
             HistoricalBeginDecisionProgressLeaf
    <2>9. \A node \in Responsive:
             (gst
               /\ HistoricalCommitDecisionCandidateOwned(
                    node, "PersistDecision"))
               ~> NodeHasDecision(node)
      BY <1>1, <2>2
         DEF HistoricalCommitCertificateConcreteLeafProperties,
             HistoricalPersistDecisionProgressLeaf
    <2> QED BY <2>4, <2>5, <2>6, <2>7, <2>8, <2>9, PTL
  <1> QED BY <1>1

THEOREM HistoricalTargetDecisionReachesApplicationFromConcreteLeaves ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalProtectedServiceRankLeafProperties(
         AsyncSpecAt(initialContext))
    /\ HistoricalDecisionFrontierAvailabilityProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalDecisionConcreteLeafProperties(
         AsyncSpecAt(initialContext))
    => \A node \in Responsive:
         (gst
           /\ HistoricalRecoveryTarget(node)
           /\ NodeHasDecision(node))
           ~> NodeHasApplication(node)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
                HistoricalProtectedServiceRankLeafProperties(
                  AsyncSpecAt(initialContext)),
                HistoricalDecisionFrontierAvailabilityProperty(
                  AsyncSpecAt(initialContext)),
                HistoricalDecisionConcreteLeafProperties(
                  AsyncSpecAt(initialContext))
         PROVE \A node \in Responsive:
                 (gst
                   /\ HistoricalRecoveryTarget(node)
                   /\ NodeHasDecision(node))
                   ~> NodeHasApplication(node)
    <2>1. HistoricalProtectedServiceRankProgressProperty(
             AsyncSpecAt(initialContext))
      BY <1>1, HistoricalProtectedServiceRankProgressFromStageLeaves
    <2>2. HistoricalProtectedCandidateStarvationProperty(
             AsyncSpecAt(initialContext))
      BY <1>1, <2>1,
         HistoricalProtectedServiceRankProgressImpliesStarvation
    <2>3. StarvationFreedomProperty(AsyncSpecAt(initialContext))
      BY StarvationFreedomObligation
    <2>4. []\A node \in Responsive:
             (gst
               /\ HistoricalRecoveryTarget(node)
               /\ NodeHasDecision(node))
               => HistoricalDecisionRecoveryFrontier(node)
      BY <1>1 DEF HistoricalDecisionFrontierAvailabilityProperty
    <2>5. \A node \in Responsive:
             (gst
               /\ HistoricalDecisionPipelineKindOwned(node, "FetchBody"))
               ~> (NodeHasApplication(node)
                    \/ HistoricalDecisionPipelineKindOwned(
                         node, "RequestCertifiedBody")
                    \/ HistoricalDecisionCertifiedRequestActive(node)
                    \/ HistoricalDecisionPipelineKindOwned(
                         node, "ValidateBody"))
      BY <1>1, <2>2
         DEF HistoricalDecisionConcreteLeafProperties,
             HistoricalDecisionFetchProgressLeaf
    <2>6. \A node \in Responsive:
             (gst
               /\ HistoricalDecisionPipelineKindOwned(
                    node, "RequestCertifiedBody"))
               ~> (NodeHasApplication(node)
                    \/ HistoricalDecisionCertifiedRequestActive(node))
      BY <1>1, <2>2
         DEF HistoricalDecisionConcreteLeafProperties,
             HistoricalDecisionRequestBodyProgressLeaf
    <2>7. \A node \in Responsive:
             (gst /\ HistoricalDecisionCertifiedRequestActive(node))
               ~> (NodeHasApplication(node)
                    \/ HistoricalDecisionPipelineKindOwned(
                         node, "FetchCertifiedBody"))
      BY <1>1, <2>2, <2>3
         DEF HistoricalDecisionConcreteLeafProperties,
             HistoricalDecisionCertifiedResponseProgressLeaf
    <2>8. \A node \in Responsive:
             (gst
               /\ HistoricalDecisionPipelineKindOwned(
                    node, "FetchCertifiedBody"))
               ~> (NodeHasApplication(node)
                    \/ HistoricalDecisionPipelineKindOwned(
                         node, "StoreBody"))
      BY <1>1, <2>2
         DEF HistoricalDecisionConcreteLeafProperties,
             HistoricalDecisionFetchCertifiedProgressLeaf
    <2>9. \A node \in Responsive:
             (gst
               /\ HistoricalDecisionPipelineKindOwned(node, "StoreBody"))
               ~> (NodeHasApplication(node)
                    \/ HistoricalDecisionPipelineKindOwned(
                         node, "ValidateBody"))
      BY <1>1, <2>2
         DEF HistoricalDecisionConcreteLeafProperties,
             HistoricalDecisionStoreProgressLeaf
    <2>10. \A node \in Responsive:
              (gst
                /\ HistoricalDecisionPipelineKindOwned(
                     node, "ValidateBody"))
                ~> (NodeHasApplication(node)
                     \/ HistoricalDecisionPipelineKindOwned(node, "Apply"))
      BY <1>1, <2>2
         DEF HistoricalDecisionConcreteLeafProperties,
             HistoricalDecisionValidateProgressLeaf
    <2>11. \A node \in Responsive:
              (gst /\ HistoricalDecisionPipelineKindOwned(node, "Apply"))
                ~> NodeHasApplication(node)
      BY <1>1, <2>2
         DEF HistoricalDecisionConcreteLeafProperties,
             HistoricalDecisionApplyProgressLeaf
    <2> QED BY <2>4, <2>5, <2>6, <2>7, <2>8, <2>9, <2>10,
                 <2>11, PTL
         DEF HistoricalDecisionRecoveryFrontier
  <1> QED BY <1>1

THEOREM HistoricalRecoveryTargetDecisionFromExactCorridor ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalRecoveryAsyncTemporalClosurePremises(
         AsyncSpecAt(initialContext))
    => HistoricalRecoveryTargetDecisionProgressProperty(
         AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
                HistoricalRecoveryAsyncTemporalClosurePremises(
                  AsyncSpecAt(initialContext))
         PROVE HistoricalRecoveryTargetDecisionProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. \A node \in Responsive:
             HistoricalCommitCertificateDiscoveryPending(node)
               ~> HistoricalCommitCertificateDiscoveryOutcome(node)
      BY <1>1, FairHistoricalCommitCertificateDiscoveryFromPersistence
         DEF HistoricalRecoveryAsyncTemporalClosurePremises
    <2>2. \A node \in Responsive:
             (gst /\ HistoricalRecoveryTarget(node))
               ~> (HistoricalCommitCertificateDiscoveryPending(node)
                    \/ HistoricalCommitCertificateDiscoveryOutcome(node))
      BY <1>1, HistoricalCommitCertificateDiscoveryReadinessFromClock
         DEF HistoricalRecoveryAsyncTemporalClosurePremises
    <2>3. \A node \in Responsive:
             (gst
               /\ HistoricalRecoveryTarget(node)
               /\ ActiveCommitCertificateRequests(node) # {})
               ~> NodeHasDecision(node)
      BY <1>1, HistoricalActiveCommitCertificateRequestReachesDecision
         DEF HistoricalRecoveryAsyncTemporalClosurePremises
    <2>4. \A node \in Responsive:
             HistoricalCommitCertificateDiscoveryOutcome(node)
               ~> NodeHasDecision(node)
      BY <2>3, PTL DEF HistoricalCommitCertificateDiscoveryOutcome
    <2>5. \A node \in Responsive:
             (gst /\ HistoricalRecoveryTarget(node))
               ~> NodeHasDecision(node)
      BY <2>1, <2>2, <2>4, PTL
    <2> QED BY <1>1, <2>5
         DEF HistoricalRecoveryTargetDecisionProgressProperty
  <1> QED BY <1>1

THEOREM ResponsiveDecisionApplicationFromExactCorridor ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalRecoveryAsyncTemporalClosurePremises(
         AsyncSpecAt(initialContext))
    => ResponsiveDecisionApplicationProgressProperty(
         AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
                HistoricalRecoveryAsyncTemporalClosurePremises(
                  AsyncSpecAt(initialContext))
         PROVE ResponsiveDecisionApplicationProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. \A node \in Responsive:
             (gst
               /\ HistoricalRecoveryTarget(node)
               /\ NodeHasDecision(node))
               ~> NodeHasApplication(node)
      BY <1>1,
         HistoricalTargetDecisionReachesApplicationFromConcreteLeaves
         DEF HistoricalRecoveryAsyncTemporalClosurePremises
    <2>2. \A node \in AsyncCurrentResponsiveVoters:
             (gst /\ NodeHasDecision(node))
               ~> NodeHasApplication(node)
      BY <1>1
         DEF HistoricalRecoveryAsyncTemporalClosurePremises,
             ApplicationCompletionProgressProperty
    <2>3. []ResponsiveDecisionServiceOwnershipInvariant
      BY <1>1
         DEF HistoricalRecoveryAsyncTemporalClosurePremises,
             ResponsiveDecisionServiceOwnershipProperty
    <2>4. [](AsyncCurrentResponsiveVoters
               = AsyncVotersAt(initialContext))
      BY <1>1, AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>5. \A node \in Responsive:
             (gst /\ NodeHasDecision(node))
               ~> NodeHasApplication(node)
      BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF ResponsiveDecisionServiceOwnershipInvariant
    <2> QED BY <1>1, <2>5
         DEF ResponsiveDecisionApplicationProgressProperty
  <1> QED BY <1>1

THEOREM HistoricalRecoveryAsyncTemporalPrerequisitesFromExactCorridor ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalRecoveryAsyncTemporalClosurePremises(
         AsyncSpecAt(initialContext))
    => HistoricalRecoveryAsyncTemporalPrerequisites(
         AsyncSpecAt(initialContext))
BY HistoricalRecoveryTargetDecisionFromExactCorridor,
   ResponsiveDecisionApplicationFromExactCorridor
   DEF HistoricalRecoveryAsyncTemporalPrerequisites

THEOREM HistoricalRecoveryAsyncTemporalPrerequisitesFromRemainingCorridor ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedServiceFiniteRunnerEpisodeClosureProperty(
         AsyncSpecAt(initialContext))
    /\ HistoricalRecoveryAsyncRemainingCorridorPremises(
         AsyncSpecAt(initialContext))
    => HistoricalRecoveryAsyncTemporalPrerequisites(
         AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext)),
                HistoricalRecoveryAsyncRemainingCorridorPremises(
                  AsyncSpecAt(initialContext))
         PROVE HistoricalRecoveryAsyncTemporalPrerequisites(
                 AsyncSpecAt(initialContext))
    <2>1. HistoricalRecoveryAsyncModelDerivedPremises(
             AsyncSpecAt(initialContext))
      BY HistoricalRecoveryAsyncModelDerivedPremisesFromAsyncSpec
    <2>2. HistoricalRecoveryAsyncTemporalClosurePremises(
             AsyncSpecAt(initialContext))
      BY <1>1, <2>1,
         HistoricalRecoveryAsyncTemporalClosurePremisesPartition
    <2> QED BY <1>1, <2>2,
         HistoricalRecoveryAsyncTemporalPrerequisitesFromExactCorridor
  <1> QED BY <1>1

(***************************************************************************
Ordinary historical locked-body recovery cone.

The parent module now proves action-by-action preservation for the semantic
PrepareQC source and its concrete Fetch/Serve/Ingress/Store/Validate handoffs.
The operators below expose the remaining temporal leaves without collapsing
them into a direct source-to-terminal assumption.  In particular, the active
request leaf is the exact source-isolated authenticated network corridor.  Its
production refinement must remain tied to the configuration-derived reserve
for every bounded local producer; no fixed caller count is assumed here.  The
composition theorem is deductive, but does not promote that production premise
or the progress-witness ledger entry before strict TLAPS verification.
***************************************************************************)

HistoricalLockedBodyRecoveryOutcome(node, qc) ==
  \/ HistoricalLockedBodySourceRetired(node, qc)
  \/ HistoricalLockedBodyRecoveryTerminal(node, qc)

HistoricalLockedCommitCarrierRecoveryProgressLeaf(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
         (/\ gst
          /\ HistoricalLockedPrepareSource(node, qc)
          /\ HistoricalLockedCommitRecoveryWitness(node, qc)
          /\ ~HistoricalLockedBodyValidated(node, qc))
           ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                \/ HistoricalLockedBodyRestartAuthority(node, qc)
                \/ HistoricalLockedBodyFetchOwned(node, qc)
                \/ HistoricalLockedCertifiedRequestActive(node, qc)
                \/ HistoricalLockedBodyValidateOwned(node, qc))

HistoricalLockedRestartRecoveryProgressLeaf(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
         (/\ gst
          /\ HistoricalLockedPrepareSource(node, qc)
          /\ HistoricalLockedBodyRestartAuthority(node, qc))
           ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                \/ HistoricalLockedBodyFetchOwned(node, qc))

HistoricalLockedFetchRecoveryProgressLeaf(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
         (/\ gst
          /\ HistoricalLockedPrepareSource(node, qc)
          /\ HistoricalLockedBodyFetchOwned(node, qc))
           ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                \/ HistoricalLockedCertifiedRequestActive(node, qc)
                \/ HistoricalLockedBodyValidateOwned(node, qc))

HistoricalLockedRequestCandidateProgressLeaf(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
         (/\ gst
          /\ HistoricalLockedPrepareSource(node, qc)
          /\ HistoricalLockedBodyRequestOwned(node, qc))
           ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                \/ HistoricalLockedCertifiedRequestActive(node, qc))

HistoricalLockedActiveRequestProgressLeaf(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
         (/\ gst
          /\ HistoricalLockedPrepareSource(node, qc)
          /\ HistoricalLockedCertifiedRequestActive(node, qc))
           ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                \/ HistoricalLockedBodyCertifiedFetchOwned(node, qc))

HistoricalLockedCertifiedFetchProgressLeaf(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
         (/\ gst
          /\ HistoricalLockedPrepareSource(node, qc)
          /\ HistoricalLockedBodyCertifiedFetchOwned(node, qc))
           ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                \/ HistoricalLockedBodyStoreOwned(node, qc))

HistoricalLockedStoreRecoveryProgressLeaf(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
         (/\ gst
          /\ HistoricalLockedPrepareSource(node, qc)
          /\ HistoricalLockedBodyStoreOwned(node, qc))
           ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                \/ HistoricalLockedBodyValidateOwned(node, qc))

HistoricalLockedValidateRecoveryProgressLeaf(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
         (/\ gst
          /\ HistoricalLockedPrepareSource(node, qc)
          /\ HistoricalLockedBodyValidateOwned(node, qc))
           ~> HistoricalLockedBodyRecoveryOutcome(node, qc)

HistoricalLockedBodyRecoveryConeLeafProperties(specification) ==
  /\ HistoricalLockedCommitCarrierRecoveryProgressLeaf(specification)
  /\ HistoricalLockedRestartRecoveryProgressLeaf(specification)
  /\ HistoricalLockedFetchRecoveryProgressLeaf(specification)
  /\ HistoricalLockedRequestCandidateProgressLeaf(specification)
  /\ HistoricalLockedActiveRequestProgressLeaf(specification)
  /\ HistoricalLockedCertifiedFetchProgressLeaf(specification)
  /\ HistoricalLockedStoreRecoveryProgressLeaf(specification)
  /\ HistoricalLockedValidateRecoveryProgressLeaf(specification)

HistoricalLockedBodyRecoveryConeProperty(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
         (gst /\ HistoricalLockedPrepareSource(node, qc))
           ~> HistoricalLockedBodyRecoveryOutcome(node, qc)

THEOREM HistoricalLockedBodyRecoveryConeComposesFromExactLeaves ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ HistoricalLockedBodyRecoveryConeLeafProperties(
         AsyncSpecAt(initialContext))
    => HistoricalLockedBodyRecoveryConeProperty(
         AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
              AsyncSpecAt(initialContext),
              HistoricalLockedBodyRecoveryConeLeafProperties(
                AsyncSpecAt(initialContext))
         PROVE HistoricalLockedBodyRecoveryConeProperty(
                 AsyncSpecAt(initialContext))
    <2>1. []HistoricalLockedBodyRecoveryStageInvariant
      BY <1>1, AsyncSpecAlwaysHistoricalLockedBodyRecoveryStage
    <2>2. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                  NEW qc \in prepareQCs
           PROVE (gst /\ HistoricalLockedPrepareSource(node, qc))
                   ~> HistoricalLockedBodyRecoveryOutcome(node, qc)
      <3>1. (/\ gst
              /\ HistoricalLockedPrepareSource(node, qc)
              /\ HistoricalLockedCommitRecoveryWitness(node, qc)
              /\ ~HistoricalLockedBodyValidated(node, qc))
               ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                    \/ HistoricalLockedBodyRestartAuthority(node, qc)
                    \/ HistoricalLockedBodyFetchOwned(node, qc)
                    \/ HistoricalLockedCertifiedRequestActive(node, qc)
                    \/ HistoricalLockedBodyValidateOwned(node, qc))
        BY <1>1, <2>2
           DEF HistoricalLockedBodyRecoveryConeLeafProperties,
               HistoricalLockedCommitCarrierRecoveryProgressLeaf
      <3>2. (/\ gst
              /\ HistoricalLockedPrepareSource(node, qc)
              /\ HistoricalLockedBodyRestartAuthority(node, qc))
               ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                    \/ HistoricalLockedBodyFetchOwned(node, qc))
        BY <1>1, <2>2
           DEF HistoricalLockedBodyRecoveryConeLeafProperties,
               HistoricalLockedRestartRecoveryProgressLeaf
      <3>3. (/\ gst
              /\ HistoricalLockedPrepareSource(node, qc)
              /\ HistoricalLockedBodyFetchOwned(node, qc))
               ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                    \/ HistoricalLockedCertifiedRequestActive(node, qc)
                    \/ HistoricalLockedBodyValidateOwned(node, qc))
        BY <1>1, <2>2
           DEF HistoricalLockedBodyRecoveryConeLeafProperties,
               HistoricalLockedFetchRecoveryProgressLeaf
      <3>4. (/\ gst
              /\ HistoricalLockedPrepareSource(node, qc)
              /\ HistoricalLockedBodyRequestOwned(node, qc))
               ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                    \/ HistoricalLockedCertifiedRequestActive(node, qc))
        BY <1>1, <2>2
           DEF HistoricalLockedBodyRecoveryConeLeafProperties,
               HistoricalLockedRequestCandidateProgressLeaf
      <3>5. (/\ gst
              /\ HistoricalLockedPrepareSource(node, qc)
              /\ HistoricalLockedCertifiedRequestActive(node, qc))
               ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                    \/ HistoricalLockedBodyCertifiedFetchOwned(node, qc))
        BY <1>1, <2>2
           DEF HistoricalLockedBodyRecoveryConeLeafProperties,
               HistoricalLockedActiveRequestProgressLeaf
      <3>6. (/\ gst
              /\ HistoricalLockedPrepareSource(node, qc)
              /\ HistoricalLockedBodyCertifiedFetchOwned(node, qc))
               ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                    \/ HistoricalLockedBodyStoreOwned(node, qc))
        BY <1>1, <2>2
           DEF HistoricalLockedBodyRecoveryConeLeafProperties,
               HistoricalLockedCertifiedFetchProgressLeaf
      <3>7. (/\ gst
              /\ HistoricalLockedPrepareSource(node, qc)
              /\ HistoricalLockedBodyStoreOwned(node, qc))
               ~> (HistoricalLockedBodyRecoveryOutcome(node, qc)
                    \/ HistoricalLockedBodyValidateOwned(node, qc))
        BY <1>1, <2>2
           DEF HistoricalLockedBodyRecoveryConeLeafProperties,
               HistoricalLockedStoreRecoveryProgressLeaf
      <3>8. (/\ gst
              /\ HistoricalLockedPrepareSource(node, qc)
              /\ HistoricalLockedBodyValidateOwned(node, qc))
               ~> HistoricalLockedBodyRecoveryOutcome(node, qc)
        BY <1>1, <2>2
           DEF HistoricalLockedBodyRecoveryConeLeafProperties,
               HistoricalLockedValidateRecoveryProgressLeaf
      <3>9. [](HistoricalLockedBodyRecoveryTerminal(node, qc)
                  => HistoricalLockedBodyRecoveryOutcome(node, qc))
        BY PTL DEF HistoricalLockedBodyRecoveryOutcome
      <3>10. [](HistoricalLockedBodyValidated(node, qc)
                   /\ HistoricalLockedCommitRecoveryWitness(node, qc)
                  => HistoricalLockedBodyRecoveryOutcome(node, qc))
        BY PTL
           DEF HistoricalLockedBodyRecoveryOutcome,
               HistoricalLockedBodyRecoveryTerminal
      <3>11. []((gst /\ HistoricalLockedPrepareSource(node, qc))
                   => HistoricalLockedBodyRecoveryStage(node, qc))
        BY <2>1, <2>2, PTL
           DEF HistoricalLockedBodyRecoveryStageInvariant
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5, <3>6,
                   <3>7, <3>8, <3>9, <3>10, <3>11, PTL
           DEF HistoricalLockedBodyRecoveryStage,
               HistoricalLockedBodyRecoveryOutcome
    <2> QED BY <1>1, <2>2
         DEF HistoricalLockedBodyRecoveryConeProperty
  <1> QED BY <1>1

=============================================================================
