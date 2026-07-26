---- MODULE SumeragiV2HeightProductivityFrontierProofs ----
EXTENDS SumeragiV2ProgressWitnessFinalClosureProofs

(***************************************************************************
Height-productivity frontier decomposition.

This leaf keeps the executable frontier exact.  It neither adds a favourable
network action nor treats bare scheduler enabledness as height progress.  The
normal form below intersects each concrete post-GST fair action with the
already-declared productive effect and preserves the production domains:

  * consensus runners and discovery remain responsive-voter scoped;
  * historical serving remains responsive applied-archive scoped;
  * ordinary I/O remains archive-I/O-service scoped; and
  * historical recovery keeps its separate responsive/target corridor.

The first concrete certificates close clock movement and the Local/Ingress
parts of a responsive runner cycle.  The final reduction exposes the exact
remaining reset-boundary state instead of asserting that a Runtime-to-Local
phase reset, a blocked packet, or a same-rank protected action is productive.
No theorem from SumeragiV2AsyncTemporalClosureProofs is imported.
***************************************************************************)

ImmediateProductiveFairActionReady ==
  \/ ENABLED (AsyncTick /\ PostGstProductiveEffect)
  \/ \E node \in AsyncCurrentResponsiveVoters:
       \/ ENABLED (PostGstRunNode(node) /\ PostGstProductiveEffect)
       \/ ENABLED (
            PostGstCommitCertificateDiscovery(node)
              /\ PostGstProductiveEffect)
  \/ \E node \in AsyncResponsiveAppliedArchiveServers:
       ENABLED (
         PostGstRunHistoricalServer(node) /\ PostGstProductiveEffect)
  \/ \E node \in AsyncArchiveIoServiceNodes:
       ENABLED (PostGstServiceIoWorker(node) /\ PostGstProductiveEffect)
  \/ \E node \in Responsive:
       \/ ENABLED (
            PostGstOpenHistoricalRecovery(node)
              /\ PostGstProductiveEffect)
       \/ ENABLED (
            PostGstRunHistoricalRecoveryNode(node)
              /\ PostGstProductiveEffect)
       \/ ENABLED (
            PostGstHistoricalCommitCertificateDiscovery(node)
              /\ PostGstProductiveEffect)
       \/ ENABLED (
            PostGstServiceHistoricalRecoveryIoWorker(node)
              /\ PostGstProductiveEffect)
  \/ \E recipient \in AsyncArchiveIoServiceNodes,
       source \in AsyncIngressSources:
       ENABLED (
         PostGstAdmitHiddenPacket(recipient, source)
           /\ PostGstProductiveEffect)
  \/ \E recipient \in ValidatorIds,
       source \in AsyncIngressSources:
       ENABLED (
         PostGstAdmitHistoricalRecoveryPacket(recipient, source)
           /\ PostGstProductiveEffect)

THEOREM PostGstProductiveEnabledNormalForm ==
  gst
    => (PostGstProductiveActionEnabled
          <=> ImmediateProductiveFairActionReady)
BY Isa
   DEF PostGstProductiveActionEnabled, PostGstProductiveStep,
       PostGstProductiveSchedulerStep,
       ImmediateProductiveFairActionReady

(***************************************************************************
Clock certificate.

An undecided responsive voter witnesses a positive node-service distance
whenever AsyncTick is enabled after GST.  AsyncTick changes only asyncNow, so
that concrete distance strictly decreases.  This avoids counting a tick once
an overdue node, I/O job, or authenticated packet has stopped the clock.
***************************************************************************)

THEOREM GstUndecidedTickStrictlyDecreasesDeadlineDebt ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ ~ResponsiveNodesDecide
  /\ AsyncTick
  => PostGstDeadlineDebtDecreases
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              gst,
              ~ResponsiveNodesDecide,
              AsyncTick
         PROVE PostGstDeadlineDebtDecreases
    <2>1. PICK node \in AsyncCurrentResponsiveVoters:
             ~NodeHasDecision(node)
      BY <1>1, Isa DEF ResponsiveNodesDecide
    <2>2. /\ AsyncTypeInvariant
           /\ node \in ValidatorIds
           /\ node \in AsyncTimedServiceNodes
           /\ asyncNow \in Nat
           /\ asyncNodeServiceDeadlines[node] \in Nat
      BY <1>1, <2>1, AsyncStrongTypeProjectsAsyncType,
         AsyncCurrentResponsiveVotersAreValidators, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncTransportTypeInvariant,
             AsyncTransportClockTypeInvariant,
             AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes
    <2>3. /\ asyncNodeServiceDeadlines[node] > asyncNow
           /\ asyncNow' = asyncNow + 1
           /\ asyncNodeServiceDeadlines'[node]
                = asyncNodeServiceDeadlines[node]
      BY <1>1, <2>1, <2>2, Isa
         DEF AsyncTick, AsyncTickEnabled, AsyncNonClockVars,
             AsyncArchiveIoServiceNodes
    <2>4. DeadlineDistance(
             asyncNodeServiceDeadlines'[node], asyncNow')
             < DeadlineDistance(
                 asyncNodeServiceDeadlines[node], asyncNow)
      BY <2>2, <2>3, SMT DEF DeadlineDistance
    <2> QED BY <2>1, <2>4
         DEF PostGstDeadlineDebtDecreases
  <1> QED BY <1>1

THEOREM AsyncTickGuardHasConcreteSuccessor ==
  AsyncTickEnabled => ENABLED AsyncTick
BY ExpandENABLED, Isa
   DEF AsyncTick, AsyncNonRunnerOuterFrame,
       AsyncNonCrashOuterFrame, AsyncCoreOuterFrame,
       AsyncNonClockVars, AsyncAllVars, AsyncSchedulerVars,
       AsyncRecoveryVars, vars

THEOREM GstUndecidedEnabledTickIsImmediatelyProductive ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ ~ResponsiveNodesDecide
  /\ AsyncTickEnabled
  => ImmediateProductiveFairActionReady
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              gst,
              ~ResponsiveNodesDecide,
              AsyncTickEnabled
         PROVE ImmediateProductiveFairActionReady
    <2>1. ENABLED AsyncTick
      BY <1>1, AsyncTickGuardHasConcreteSuccessor
    <2>2. AsyncTick \in BOOLEAN
      BY Isa DEF AsyncTick
    <2>3. (AsyncTick /\ PostGstProductiveEffect) \in BOOLEAN
      BY Isa
    <2>4. AsyncTick
             => AsyncTick /\ PostGstProductiveEffect
      BY <1>1, GstUndecidedTickStrictlyDecreasesDeadlineDebt
         DEF PostGstProductiveEffect
    <2>5. ENABLED (AsyncTick /\ PostGstProductiveEffect)
      BY <2>1, <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <2>5
         DEF ImmediateProductiveFairActionReady
  <1> QED BY <1>1

(***************************************************************************
Responsive Local/Ingress runner certificate.

The concrete RunNode wrapper resets the node-service deadline, but the
productive witness here is not that reset.  LocalAdmissionStep and
IngressDrainStep already have strict RuntimeReachRank descent within the
current runner cycle, including their blocked phase-advance arms.
***************************************************************************)

THEOREM GstUndecidedResponsiveRunNodeIsEnabled ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ ~NodeHasDecision(node)
    => ENABLED PostGstRunNode(node)
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncStrongTypeInvariant,
                gst,
                ~NodeHasDecision(node)
         PROVE ENABLED PostGstRunNode(node)
    <2>1. node \in ValidatorIds
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety
    <2>2. ~NodeHasApplication(node)
      BY <1>1, <2>1, AppliedNodeHasDecision
         DEF AsyncStrongTypeInvariant
    <2>3. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>4. Responsive \subseteq up
      BY <1>1, GstResponsiveNodesAreUp
         DEF AsyncStrongTypeInvariant
    <2>5. node \in up
      BY <1>1, <2>4 DEF AsyncCurrentResponsiveVoters
    <2>6. ~ResponsiveReplayQuarantined(node)
      BY <1>1, GstExcludesResponsiveReplayQuarantine
         DEF AsyncStrongTypeInvariant
    <2>7. RecoveryRunNodeGuard(node)
      BY <2>6 DEF RecoveryRunNodeGuard
    <2>8. ENABLED RunNode(node)
      BY <1>1, <2>2, <2>3, <2>5, <2>7,
         ResponsiveUnappliedRunNodeIsEnabled
    <2> QED BY <1>1, <2>8, EnabledRunNodeLiftsPostGst
  <1> QED BY <1>1

THEOREM LocalOrIngressPostGstRunNodeDecreasesRuntimeReach ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ asyncRunnerPhase[node] \in {"Local", "Ingress"}
    /\ PostGstRunNode(node)
    => PostGstRuntimeReachDecreases
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncStrongTypeInvariant,
                asyncRunnerPhase[node] \in {"Local", "Ingress"},
                PostGstRunNode(node)
         PROVE PostGstRuntimeReachDecreases
    <2>1. /\ AsyncTypeInvariant
           /\ RunNodeWork(node)
           /\ ~ResponsiveReplayQuarantined(node)
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         GstExcludesResponsiveReplayQuarantine
         DEF PostGstRunNode, RunNode, AsyncStrongTypeInvariant
    <2>2. CASE asyncRunnerPhase[node] = "Local"
      <3>1. LocalAdmissionStep(node)
        BY <1>1, <2>1, <2>2, Isa DEF RunNodeWork
      <3>2. RuntimeReachRank(node)' < RuntimeReachRank(node)
        BY <2>1, <3>1, LocalAdmissionStrictlyDecreasesRuntimeReach
      <3> QED BY <1>1, <3>2
           DEF PostGstRuntimeReachDecreases,
               PostGstServiceNodes, AsyncTimedServiceNodes,
               AsyncArchiveIoServiceNodes
    <2>3. CASE asyncRunnerPhase[node] = "Ingress"
      <3>1. IngressDrainStep(node)
        BY <1>1, <2>1, <2>3, Isa DEF RunNodeWork
      <3>2. RuntimeReachRank(node)' < RuntimeReachRank(node)
        BY <2>1, <3>1, IngressDrainStrictlyDecreasesRuntimeReach
      <3> QED BY <1>1, <3>2
           DEF PostGstRuntimeReachDecreases,
               PostGstServiceNodes, AsyncTimedServiceNodes,
               AsyncArchiveIoServiceNodes
    <2> QED BY <1>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM GstUndecidedLocalOrIngressRunnerIsImmediatelyProductive ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ ~NodeHasDecision(node)
    /\ asyncRunnerPhase[node] \in {"Local", "Ingress"}
    => ImmediateProductiveFairActionReady
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncStrongTypeInvariant,
                gst,
                ~NodeHasDecision(node),
                asyncRunnerPhase[node] \in {"Local", "Ingress"}
         PROVE ImmediateProductiveFairActionReady
    <2>1. ENABLED PostGstRunNode(node)
      BY <1>1, GstUndecidedResponsiveRunNodeIsEnabled
    <2>2. PostGstRunNode(node) \in BOOLEAN
      BY Isa DEF PostGstRunNode
    <2>3. (PostGstRunNode(node) /\ PostGstProductiveEffect)
             \in BOOLEAN
      BY Isa
    <2>4. PostGstRunNode(node)
             => PostGstRunNode(node) /\ PostGstProductiveEffect
      BY <1>1, LocalOrIngressPostGstRunNodeDecreasesRuntimeReach
         DEF PostGstProductiveEffect
    <2>5. ENABLED (
             PostGstRunNode(node) /\ PostGstProductiveEffect)
      BY <2>1, <2>2, <2>3, <2>4, ENABLEDaxioms
    <2> QED BY <1>1, <2>5
         DEF ImmediateProductiveFairActionReady
  <1> QED BY <1>1

(***************************************************************************
Exact clock-blocking partition.

When the clock certificate is absent after GST, AsyncTickEnabled identifies
one of exactly three concrete owners: an overdue authenticated packet, a due
node-service turn, or a due nonempty I/O worker.  This partition includes
responsive applied archives and active historical-recovery targets through
AsyncTimedServiceNodes; it does not collapse them into the voter domain.
***************************************************************************)

PostGstClockBlockingOwnerExists ==
  \/ OverdueResponsivePackets # {}
  \/ \E node \in AsyncTimedServiceNodes:
       asyncNodeServiceDeadlines[node] <= asyncNow
  \/ \E node \in AsyncTimedServiceNodes:
       /\ AsyncIoQueueDepth(node) > 0
       /\ asyncIoServiceDeadlines[node] <= asyncNow

THEOREM GstClockDisabledHasExactBlockingOwner ==
  gst /\ ~AsyncTickEnabled
    => PostGstClockBlockingOwnerExists
BY Isa
   DEF AsyncTickEnabled, PostGstClockBlockingOwnerExists

(***************************************************************************
Residual frontier.

Outside the two concrete certificates above, every undecided responsive
voter is at the Runtime reset boundary and the clock has one exact blocking
owner.  The existing protected-rank temporal theorems show that admitted work
cannot starve, but they do not state that every Runtime successor immediately
decreases a rank: an idle Runtime step resets RuntimeReachRank upward, and a
fresh overdue packet can be capacity-blocked.  Those reachable action cases
are therefore retained explicitly here.
***************************************************************************)

HeightProductivityResetBoundary ==
  /\ gst
  /\ ~ResponsiveNodesDecide
  /\ ~AsyncTickEnabled
  /\ PostGstClockBlockingOwnerExists
  /\ \A node \in AsyncCurrentResponsiveVoters:
       ~NodeHasDecision(node)
         => asyncRunnerPhase[node] = "Runtime"

THEOREM GstUndecidedStateHasImmediateProductivityOrResetBoundary ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ ~ResponsiveNodesDecide
  => \/ ImmediateProductiveFairActionReady
     \/ HeightProductivityResetBoundary
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              gst,
              ~ResponsiveNodesDecide
         PROVE \/ ImmediateProductiveFairActionReady
               \/ HeightProductivityResetBoundary
    <2>1. CASE AsyncTickEnabled
      BY <1>1, <2>1,
         GstUndecidedEnabledTickIsImmediatelyProductive
    <2>2. CASE ~AsyncTickEnabled
      <3>1. PostGstClockBlockingOwnerExists
        BY <1>1, <2>2, GstClockDisabledHasExactBlockingOwner
      <3>2. CASE \E node \in AsyncCurrentResponsiveVoters:
                       /\ ~NodeHasDecision(node)
                       /\ asyncRunnerPhase[node]
                            \in {"Local", "Ingress"}
        <4>1. PICK node \in AsyncCurrentResponsiveVoters:
                 /\ ~NodeHasDecision(node)
                 /\ asyncRunnerPhase[node] \in {"Local", "Ingress"}
          BY <3>2
        <4> QED BY <1>1, <4>1,
             GstUndecidedLocalOrIngressRunnerIsImmediatelyProductive
      <3>3. CASE ~\E node \in AsyncCurrentResponsiveVoters:
                       /\ ~NodeHasDecision(node)
                       /\ asyncRunnerPhase[node]
                            \in {"Local", "Ingress"}
        <4>1. \A node \in AsyncCurrentResponsiveVoters:
                 asyncRunnerPhase[node]
                   \in {"Local", "Ingress", "Runtime"}
          BY <1>1, AsyncStrongTypeProjectsAsyncType, Isa
             DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncRuntimeTypeInvariant,
                 AsyncRuntimeScalarTypeInvariant
        <4>2. \A node \in AsyncCurrentResponsiveVoters:
                 ~NodeHasDecision(node)
                   => asyncRunnerPhase[node] = "Runtime"
          BY <3>3, <4>1, Isa
        <4> QED BY <1>1, <2>2, <3>1, <4>2
             DEF HeightProductivityResetBoundary
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Exact temporal reduction of the release property.

An undecided execution may decide while the Runtime reset and ingress owners
are being discharged.  The honest frontier therefore asks that a remaining
reset-boundary state eventually expose one concrete productive fair action or
reach aggregate responsive Decision.  The equivalence proves both that this
one temporal lemma is sufficient and that the release frontier itself
necessarily supplies it.
***************************************************************************)

HeightProductivityResetBoundaryProperty(specification) ==
  specification
    => (AsyncStrongTypeInvariant
          /\ HeightProductivityResetBoundary)
         ~> (ImmediateProductiveFairActionReady
               \/ ResponsiveNodesDecide)

THEOREM ResetBoundaryCoverageImpliesHeightProductivityFrontier ==
  \A initialContext:
    HeightProductivityResetBoundaryProperty(
      AsyncLiveSpecAt(initialContext))
      => HeightProductivityFrontierProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                HeightProductivityResetBoundaryProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE HeightProductivityFrontierProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE (gst /\ ~ResponsiveNodesDecide)
                    ~> (PostGstProductiveActionEnabled
                          \/ ResponsiveNodesDecide)
      <3>1. AsyncSpecAt(initialContext)
        BY <2>1, AsyncLiveSpecProjectsAsyncSpec
      <3>2. []AsyncStrongTypeInvariant
        BY <3>1, AsyncSpecAlwaysStrongTypeInvariant
      <3>3. [](gst => []gst)
        BY <3>1, AsyncSpecKeepsGstOnceSet
      <3>4. (AsyncStrongTypeInvariant
               /\ HeightProductivityResetBoundary)
                ~> (ImmediateProductiveFairActionReady
                      \/ ResponsiveNodesDecide)
        BY <1>1, <2>1
           DEF HeightProductivityResetBoundaryProperty
      <3>5. [](AsyncStrongTypeInvariant
                 /\ gst
                 /\ ~ResponsiveNodesDecide
                => \/ ImmediateProductiveFairActionReady
                   \/ HeightProductivityResetBoundary)
        BY GstUndecidedStateHasImmediateProductivityOrResetBoundary, PTL
      <3>6. (gst /\ ~ResponsiveNodesDecide)
                 ~> (ImmediateProductiveFairActionReady
                       \/ ResponsiveNodesDecide)
        BY <3>2, <3>4, <3>5, PTL
      <3>7. [](gst
                 /\ ImmediateProductiveFairActionReady
                => PostGstProductiveActionEnabled)
        BY PostGstProductiveEnabledNormalForm, PTL
      <3> QED BY <3>3, <3>6, <3>7, PTL
    <2> QED BY <2>1 DEF HeightProductivityFrontierProperty
  <1> QED BY <1>1

THEOREM HeightProductivityFrontierImpliesResetBoundaryCoverage ==
  \A initialContext:
    HeightProductivityFrontierProperty(
      AsyncLiveSpecAt(initialContext))
      => HeightProductivityResetBoundaryProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                HeightProductivityFrontierProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE HeightProductivityResetBoundaryProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE (AsyncStrongTypeInvariant
                    /\ HeightProductivityResetBoundary)
                    ~> (ImmediateProductiveFairActionReady
                          \/ ResponsiveNodesDecide)
      <3>1. AsyncSpecAt(initialContext)
        BY <2>1, AsyncLiveSpecProjectsAsyncSpec
      <3>2. [](gst => []gst)
        BY <3>1, AsyncSpecKeepsGstOnceSet
      <3>3. (gst /\ ~ResponsiveNodesDecide)
                 ~> (PostGstProductiveActionEnabled
                       \/ ResponsiveNodesDecide)
        BY <1>1, <2>1 DEF HeightProductivityFrontierProperty
      <3>4. [](HeightProductivityResetBoundary
                 => gst /\ ~ResponsiveNodesDecide)
        BY PTL DEF HeightProductivityResetBoundary
      <3>5. [](gst
                 /\ PostGstProductiveActionEnabled
                => ImmediateProductiveFairActionReady)
        BY PostGstProductiveEnabledNormalForm, PTL
      <3> QED BY <3>2, <3>3, <3>4, <3>5, PTL
    <2> QED BY <2>1
         DEF HeightProductivityResetBoundaryProperty
  <1> QED BY <1>1

THEOREM HeightProductivityFrontierExactResidualEquivalence ==
  \A initialContext:
    HeightProductivityFrontierProperty(
      AsyncLiveSpecAt(initialContext))
      <=> HeightProductivityResetBoundaryProperty(
            AsyncLiveSpecAt(initialContext))
BY ResetBoundaryCoverageImpliesHeightProductivityFrontier,
   HeightProductivityFrontierImpliesResetBoundaryCoverage

=============================================================================
