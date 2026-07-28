---- MODULE SumeragiV2HeightResetBoundaryClosureProofs ----
EXTENDS SumeragiV2HeightProductivityFrontierProofs

(***************************************************************************
Exact reset-boundary closure decomposition.

This leaf discharges every due scheduler owner without confusing a merely
enabled action with an immediately productive one:

  * a due node-service turn removes a weight-two node blocker and can create
    at most one weight-one I/O blocker;
  * a due nonempty I/O turn removes its weight-one blocker;
  * an admissible overdue lane head consumes that exact overdue packet; and
  * candidate and Serve rank descent is retained as an explicit productive
    fair-action class.

The remaining packet case is exact.  An overdue packet can be hidden behind
an older due packet in the same source lane.  If the lane head is itself
overdue, it can still be fresh and rejected by the capacity, timeout-byte, or
transport-completion ownership gates.  Neither case is an immediate
`PostGstProductiveEffect`, and an idle Runtime-to-Local reset is not promoted
to one.  Instead, the temporal suffix proves that, while no immediate
productive action exists, every still-undecided fixed voter is at Runtime;
its exact weakly fair runner resets it to Local, which exposes productivity or
a durable Decision.  Stable Decision receipts are then accumulated by finite
induction over the frozen roster.  The final equivalence therefore discharges
the strictly smaller ingress-ownership residual without relabelling a reset as
productive work.

The dependency audit below also records the repaired certified-response
corridor exactly: CertifiedResponse and generic TransportCompletion are
distinct logical classes, both spend the same finite physical completion
owner, the response itself acquires a route-neutral exact claim, and generic
aggregate-lane completion traffic cannot refill that owner while a certified
request remains live.  A physical owner already present before the request is
still genuine finite debt, even though the reset proof does not need to assume
its individual rank descent.
***************************************************************************)

(***************************************************************************
Weighted blocker arithmetic.

`NodeBlockerHandoff` is the precise frame promised by a node-service turn:
the serviced node leaves the node-blocker set and only that same node may
newly enter the I/O-blocker set.  The weight 2/1 split makes the handoff
strict.  `IoBlockerDischarge` is the corresponding exact FIFO-worker frame.
***************************************************************************)

NodeBlockerHandoff(node) ==
  /\ node \in PostGstNodeServiceBlockers
  /\ IsFiniteSet(PostGstNodeServiceBlockers)
  /\ IsFiniteSet(PostGstIoServiceBlockers)
  /\ PostGstNodeServiceBlockers'
       = PostGstNodeServiceBlockers \ {node}
  /\ PostGstIoServiceBlockers'
       \subseteq PostGstIoServiceBlockers \cup {node}

IoBlockerDischarge(node) ==
  /\ node \in PostGstIoServiceBlockers
  /\ IsFiniteSet(PostGstNodeServiceBlockers)
  /\ IsFiniteSet(PostGstIoServiceBlockers)
  /\ PostGstNodeServiceBlockers'
       = PostGstNodeServiceBlockers
  /\ PostGstIoServiceBlockers'
       = PostGstIoServiceBlockers \ {node}

THEOREM StrongTypeHasFiniteServiceBlockers ==
  AsyncStrongTypeInvariant
    => /\ IsFiniteSet(PostGstNodeServiceBlockers)
       /\ IsFiniteSet(PostGstIoServiceBlockers)
BY AsyncCurrentResponsiveVotersAreValidators,
   AsyncResponsiveAppliedArchiveServersAreValidators,
   HistoricalRecoveryTargetsAreValidators,
   FS_Union, FS_Subset, Isa
   DEF AsyncStrongTypeInvariant, PostGstNodeServiceBlockers,
       PostGstIoServiceBlockers, PostGstServiceNodes,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       ValidatorIds

THEOREM NodeBlockerHandoffStrictlyDecreasesDebt ==
  \A node:
    NodeBlockerHandoff(node)
      => PostGstNodeIoBlockerDebtDecreases
PROOF
  <1>1. ASSUME NEW node, NodeBlockerHandoff(node)
         PROVE PostGstNodeIoBlockerDebtDecreases
    <2>1. Cardinality(PostGstNodeServiceBlockers')
             = Cardinality(PostGstNodeServiceBlockers) - 1
      BY <1>1, FS_RemoveElement
         DEF NodeBlockerHandoff
    <2>2. /\ IsFiniteSet(PostGstIoServiceBlockers \cup {node})
           /\ Cardinality(PostGstIoServiceBlockers \cup {node})
                <= Cardinality(PostGstIoServiceBlockers) + 1
      BY <1>1, FS_AddElement, FS_CardinalityType, SMT
         DEF NodeBlockerHandoff
    <2>3. Cardinality(PostGstIoServiceBlockers')
             <= Cardinality(PostGstIoServiceBlockers) + 1
      BY <1>1, <2>2, FS_Subset
         DEF NodeBlockerHandoff
    <2> QED BY <1>1, <2>1, <2>3,
         FS_CardinalityType, SMT
         DEF PostGstNodeIoBlockerDebtDecreases,
             PostGstNodeIoBlockerDebt
  <1> QED BY <1>1

THEOREM IoBlockerDischargeStrictlyDecreasesDebt ==
  \A node:
    IoBlockerDischarge(node)
      => PostGstNodeIoBlockerDebtDecreases
PROOF
  <1>1. ASSUME NEW node, IoBlockerDischarge(node)
         PROVE PostGstNodeIoBlockerDebtDecreases
    <2>1. Cardinality(PostGstIoServiceBlockers')
             = Cardinality(PostGstIoServiceBlockers) - 1
      BY <1>1, FS_RemoveElement
         DEF IoBlockerDischarge
    <2> QED BY <1>1, <2>1, FS_CardinalityType, SMT
         DEF IoBlockerDischarge,
             PostGstNodeIoBlockerDebtDecreases,
             PostGstNodeIoBlockerDebt
  <1> QED BY <1>1

(***************************************************************************
Concrete service-action projections.

All three node actions reset exactly one node-service deadline to
`asyncNow + AsyncDeliveryBound`, leave the clock fixed, and preserve the
timed-service union.  They can enqueue I/O only for the serviced node.  The
two worker actions remove exactly the FIFO head and reset exactly that
node's I/O deadline.  These are action facts, not temporal rank claims.
***************************************************************************)

THEOREM DueResponsiveRunNodeHasBlockerHandoff ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ asyncNodeServiceDeadlines[node] <= asyncNow
    /\ PostGstRunNode(node)
    => NodeBlockerHandoff(node)
BY AsyncStrongTypeProjectsAsyncType,
   StrongTypeHasFiniteServiceBlockers,
   AsyncArchiveIoServiceNodesAreValidators,
   HistoricalRecoveryTargetsAreValidators,
   Isa
   DEF NodeBlockerHandoff, PostGstNodeServiceBlockers,
       PostGstIoServiceBlockers, PostGstServiceNodes,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers, AsyncIoQueueDepth,
       PostGstRunNode, RunNode, RunNodeWork,
       LocalAdmissionStep, IngressDrainStep, SerializedRuntimeStep,
       RuntimeStep, AsyncConfiguration

THEOREM DueHistoricalRecoveryRunNodeHasBlockerHandoff ==
  \A node \in asyncHistoricalRecoveryTargets:
    /\ AsyncStrongTypeInvariant
    /\ asyncNodeServiceDeadlines[node] <= asyncNow
    /\ PostGstRunHistoricalRecoveryNode(node)
    => NodeBlockerHandoff(node)
BY AsyncStrongTypeProjectsAsyncType,
   StrongTypeHasFiniteServiceBlockers,
   HistoricalRecoveryTargetsAreValidators,
   Isa
   DEF NodeBlockerHandoff, PostGstNodeServiceBlockers,
       PostGstIoServiceBlockers, PostGstServiceNodes,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers, AsyncIoQueueDepth,
       PostGstRunHistoricalRecoveryNode,
       RunHistoricalRecoveryNode, RunNodeWork,
       LocalAdmissionStep, IngressDrainStep, SerializedRuntimeStep,
       RuntimeStep, AsyncConfiguration

THEOREM DueHistoricalServerHasBlockerHandoff ==
  \A node \in AsyncResponsiveAppliedArchiveServers:
    /\ AsyncStrongTypeInvariant
    /\ asyncNodeServiceDeadlines[node] <= asyncNow
    /\ PostGstRunHistoricalServer(node)
    => NodeBlockerHandoff(node)
BY AsyncStrongTypeProjectsAsyncType,
   StrongTypeHasFiniteServiceBlockers,
   AsyncResponsiveAppliedArchiveServersAreValidators,
   Isa
   DEF NodeBlockerHandoff, PostGstNodeServiceBlockers,
       PostGstIoServiceBlockers, PostGstServiceNodes,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncIoQueueDepth, PostGstRunHistoricalServer,
       RunHistoricalServer, DrainHistoricalIngressSelected,
       HistoricalIdleStep, AsyncConfiguration

THEOREM DueOrdinaryIoWorkerDischargesBlocker ==
  \A node \in AsyncArchiveIoServiceNodes:
    /\ AsyncStrongTypeInvariant
    /\ AsyncIoQueueDepth(node) > 0
    /\ asyncIoServiceDeadlines[node] <= asyncNow
    /\ PostGstServiceIoWorker(node)
    => IoBlockerDischarge(node)
BY AsyncStrongTypeProjectsAsyncType,
   StrongTypeHasFiniteServiceBlockers,
   AsyncArchiveIoServiceNodesAreValidators,
   HistoricalRecoveryTargetsAreValidators,
   HeadTailProperties, Isa
   DEF IoBlockerDischarge, PostGstNodeServiceBlockers,
       PostGstIoServiceBlockers, PostGstServiceNodes,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncIoQueueDepth, PostGstServiceIoWorker,
       ServiceIoWorker, ServiceIoWorkerWork,
       PublishEphemeralItems, AsyncConfiguration

THEOREM DueHistoricalIoWorkerDischargesBlocker ==
  \A node \in asyncHistoricalRecoveryTargets:
    /\ AsyncStrongTypeInvariant
    /\ AsyncIoQueueDepth(node) > 0
    /\ asyncIoServiceDeadlines[node] <= asyncNow
    /\ PostGstServiceHistoricalRecoveryIoWorker(node)
    => IoBlockerDischarge(node)
BY AsyncStrongTypeProjectsAsyncType,
   StrongTypeHasFiniteServiceBlockers,
   HistoricalRecoveryTargetsAreValidators,
   HeadTailProperties, Isa
   DEF IoBlockerDischarge, PostGstNodeServiceBlockers,
       PostGstIoServiceBlockers, PostGstServiceNodes,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncIoQueueDepth,
       PostGstServiceHistoricalRecoveryIoWorker,
       ServiceHistoricalRecoveryIoWorker, ServiceIoWorkerWork,
       PublishEphemeralItems, AsyncConfiguration

THEOREM DueResponsiveRunNodeDecreasesBlockerDebt ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ asyncNodeServiceDeadlines[node] <= asyncNow
    /\ PostGstRunNode(node)
    => PostGstNodeIoBlockerDebtDecreases
BY DueResponsiveRunNodeHasBlockerHandoff,
   NodeBlockerHandoffStrictlyDecreasesDebt

THEOREM DueHistoricalRecoveryRunNodeDecreasesBlockerDebt ==
  \A node \in asyncHistoricalRecoveryTargets:
    /\ AsyncStrongTypeInvariant
    /\ asyncNodeServiceDeadlines[node] <= asyncNow
    /\ PostGstRunHistoricalRecoveryNode(node)
    => PostGstNodeIoBlockerDebtDecreases
BY DueHistoricalRecoveryRunNodeHasBlockerHandoff,
   NodeBlockerHandoffStrictlyDecreasesDebt

THEOREM DueHistoricalServerDecreasesBlockerDebt ==
  \A node \in AsyncResponsiveAppliedArchiveServers:
    /\ AsyncStrongTypeInvariant
    /\ asyncNodeServiceDeadlines[node] <= asyncNow
    /\ PostGstRunHistoricalServer(node)
    => PostGstNodeIoBlockerDebtDecreases
BY DueHistoricalServerHasBlockerHandoff,
   NodeBlockerHandoffStrictlyDecreasesDebt

THEOREM DueOrdinaryIoWorkerDecreasesBlockerDebt ==
  \A node \in AsyncArchiveIoServiceNodes:
    /\ AsyncStrongTypeInvariant
    /\ AsyncIoQueueDepth(node) > 0
    /\ asyncIoServiceDeadlines[node] <= asyncNow
    /\ PostGstServiceIoWorker(node)
    => PostGstNodeIoBlockerDebtDecreases
BY DueOrdinaryIoWorkerDischargesBlocker,
   IoBlockerDischargeStrictlyDecreasesDebt

THEOREM DueHistoricalIoWorkerDecreasesBlockerDebt ==
  \A node \in asyncHistoricalRecoveryTargets:
    /\ AsyncStrongTypeInvariant
    /\ AsyncIoQueueDepth(node) > 0
    /\ asyncIoServiceDeadlines[node] <= asyncNow
    /\ PostGstServiceHistoricalRecoveryIoWorker(node)
    => PostGstNodeIoBlockerDebtDecreases
BY DueHistoricalIoWorkerDischargesBlocker,
   IoBlockerDischargeStrictlyDecreasesDebt

(***************************************************************************
Enabledness for the three exact node-service domains and the historical I/O
domain.  The ordinary I/O theorem already exists as
`QueuedIoEnablesPostGstService`.
***************************************************************************)

THEOREM GstResponsiveUnappliedRunNodeIsEnabled ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ ~NodeHasApplication(node)
    => ENABLED PostGstRunNode(node)
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncStrongTypeInvariant,
                gst,
                ~NodeHasApplication(node)
         PROVE ENABLED PostGstRunNode(node)
    <2>1. /\ AsyncTypeInvariant
           /\ node \in up
           /\ RecoveryRunNodeGuard(node)
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         GstResponsiveNodesAreUp,
         GstExcludesResponsiveReplayQuarantine, Isa
         DEF AsyncStrongTypeInvariant,
             AsyncCurrentResponsiveVoters,
             RecoveryRunNodeGuard
    <2>2. ENABLED RunNode(node)
      BY <1>1, <2>1, ResponsiveUnappliedRunNodeIsEnabled
    <2> QED BY <1>1, <2>2, EnabledRunNodeLiftsPostGst
  <1> QED BY <1>1

THEOREM GstHistoricalRecoveryRunNodeIsEnabled ==
  \A node \in asyncHistoricalRecoveryTargets:
    /\ AsyncStrongTypeInvariant
    /\ gst
    => ENABLED PostGstRunHistoricalRecoveryNode(node)
BY AsyncStrongTypeProjectsAsyncType,
   HistoricalRecoveryTargetsAreValidators,
   LocalAdmissionStepIsEnabled, IngressDrainStepIsEnabled,
   SerializedRuntimeStepIsEnabled,
   GstExcludesResponsiveReplayQuarantine,
   ExpandENABLED, Isa
   DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
       AsyncRuntimeScalarTypeInvariant,
       AsyncHistoricalRecoveryTypeInvariant,
       PostGstRunHistoricalRecoveryNode,
       RunHistoricalRecoveryNode, RunNodeWork,
       LocalAdmissionStep, IngressDrainStep, SerializedRuntimeStep,
       RuntimeStep, AsyncNonCrashOuterFrame, AsyncCoreOuterFrame,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars, vars

THEOREM GstHistoricalServerIsEnabled ==
  \A node \in AsyncResponsiveAppliedArchiveServers:
    /\ AsyncStrongTypeInvariant
    /\ gst
    => ENABLED PostGstRunHistoricalServer(node)
BY GstExcludesResponsiveReplayQuarantine,
   ExpandENABLED, Isa
   DEF AsyncStrongTypeInvariant,
       PostGstRunHistoricalServer, RunHistoricalServer,
       DrainHistoricalIngressSelected, HistoricalIdleStep,
       AsyncNonCrashOuterFrame, AsyncCoreOuterFrame,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       LeaveCausalQueues, vars

THEOREM GstHistoricalIoWorkerIsEnabled ==
  \A node \in asyncHistoricalRecoveryTargets:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AsyncIoQueueDepth(node) > 0
    => ENABLED PostGstServiceHistoricalRecoveryIoWorker(node)
BY AsyncStrongTypeProjectsAsyncType,
   HistoricalRecoveryTargetsAreValidators,
   ExpandENABLED, Isa
   DEF AsyncStrongTypeInvariant,
       AsyncHistoricalRecoveryTypeInvariant,
       PostGstServiceHistoricalRecoveryIoWorker,
       ServiceHistoricalRecoveryIoWorker, ServiceIoWorkerWork,
       PublishEphemeralItems, LeaveCausalQueues,
       AsyncIoQueueDepth, AsyncNonRunnerOuterFrame,
       AsyncNonCrashOuterFrame, AsyncCoreOuterFrame,
       AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars, vars

(***************************************************************************
Due service owners expose an enabled action conjoined with the weighted debt
effect.  Current voters split into unapplied runners and applied historical
servers; active historical-recovery targets are necessarily unapplied.
***************************************************************************)

THEOREM DueNodeServiceBlockerIsImmediatelyProductive ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ PostGstNodeServiceBlockers # {}
  => ImmediateProductiveFairActionReady
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              gst,
              PostGstNodeServiceBlockers # {}
         PROVE ImmediateProductiveFairActionReady
    <2>1. PICK node \in PostGstNodeServiceBlockers: TRUE
      BY <1>1
    <2>2. /\ node \in AsyncTimedServiceNodes
           /\ asyncNodeServiceDeadlines[node] <= asyncNow
      BY <2>1
         DEF PostGstNodeServiceBlockers, PostGstServiceNodes
    <2>3. CASE node \in AsyncCurrentResponsiveVoters
      <3>1. CASE ~NodeHasApplication(node)
        <4>1. ENABLED PostGstRunNode(node)
          BY <1>1, <2>3, <3>1,
             GstResponsiveUnappliedRunNodeIsEnabled
        <4>2. PostGstRunNode(node)
                 => PostGstRunNode(node)
                      /\ PostGstProductiveEffect
          BY <1>1, <2>2, <2>3,
             DueResponsiveRunNodeDecreasesBlockerDebt
             DEF PostGstProductiveEffect
        <4>3. ENABLED (
                 PostGstRunNode(node)
                   /\ PostGstProductiveEffect)
          BY <4>1, <4>2, ENABLEDaxioms
        <4> QED BY <2>3, <4>3
             DEF ImmediateProductiveFairActionReady
      <3>2. CASE NodeHasApplication(node)
        <4>1. node \in AsyncResponsiveAppliedArchiveServers
          BY <1>1, <2>3, <3>2,
             GstResponsiveNodesAreUp, Isa
             DEF AsyncStrongTypeInvariant,
                 AsyncCurrentResponsiveVoters,
                 AsyncResponsiveAppliedArchiveServers,
                 AsyncResponsiveOnlineArchiveServers,
                 AsyncResponsiveArchiveServers,
                 AsyncArchiveServerIds
        <4>2. ENABLED PostGstRunHistoricalServer(node)
          BY <1>1, <4>1, GstHistoricalServerIsEnabled
        <4>3. PostGstRunHistoricalServer(node)
                 => PostGstRunHistoricalServer(node)
                      /\ PostGstProductiveEffect
          BY <1>1, <2>2, <4>1,
             DueHistoricalServerDecreasesBlockerDebt
             DEF PostGstProductiveEffect
        <4>4. ENABLED (
                 PostGstRunHistoricalServer(node)
                   /\ PostGstProductiveEffect)
          BY <4>2, <4>3, ENABLEDaxioms
        <4> QED BY <4>1, <4>4
             DEF ImmediateProductiveFairActionReady
      <3> QED BY <3>1, <3>2
    <2>4. CASE node \notin AsyncCurrentResponsiveVoters
      <3>1. CASE node \in AsyncResponsiveAppliedArchiveServers
        <4>1. ENABLED PostGstRunHistoricalServer(node)
          BY <1>1, <3>1, GstHistoricalServerIsEnabled
        <4>2. PostGstRunHistoricalServer(node)
                 => PostGstRunHistoricalServer(node)
                      /\ PostGstProductiveEffect
          BY <1>1, <2>2, <3>1,
             DueHistoricalServerDecreasesBlockerDebt
             DEF PostGstProductiveEffect
        <4>3. ENABLED (
                 PostGstRunHistoricalServer(node)
                   /\ PostGstProductiveEffect)
          BY <4>1, <4>2, ENABLEDaxioms
        <4> QED BY <3>1, <4>3
             DEF ImmediateProductiveFairActionReady
      <3>2. CASE node \notin AsyncResponsiveAppliedArchiveServers
        <4>1. node \in asyncHistoricalRecoveryTargets
          BY <2>2, <2>4, <3>2, Isa
             DEF AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes
        <4>2. ENABLED PostGstRunHistoricalRecoveryNode(node)
          BY <1>1, <4>1, GstHistoricalRecoveryRunNodeIsEnabled
        <4>3. PostGstRunHistoricalRecoveryNode(node)
                 => PostGstRunHistoricalRecoveryNode(node)
                      /\ PostGstProductiveEffect
          BY <1>1, <2>2, <4>1,
             DueHistoricalRecoveryRunNodeDecreasesBlockerDebt
             DEF PostGstProductiveEffect
        <4>4. ENABLED (
                 PostGstRunHistoricalRecoveryNode(node)
                   /\ PostGstProductiveEffect)
          BY <4>2, <4>3, ENABLEDaxioms
        <4> QED BY <1>1, <4>1, <4>4,
             AsyncHistoricalRecoveryTypeInvariant, Isa
             DEF AsyncStrongTypeInvariant,
                 ImmediateProductiveFairActionReady
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM DueIoServiceBlockerIsImmediatelyProductive ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ PostGstIoServiceBlockers # {}
  => ImmediateProductiveFairActionReady
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              gst,
              PostGstIoServiceBlockers # {}
         PROVE ImmediateProductiveFairActionReady
    <2>1. PICK node \in PostGstIoServiceBlockers: TRUE
      BY <1>1
    <2>2. /\ node \in AsyncTimedServiceNodes
           /\ AsyncIoQueueDepth(node) > 0
           /\ asyncIoServiceDeadlines[node] <= asyncNow
      BY <2>1
         DEF PostGstIoServiceBlockers, PostGstServiceNodes
    <2>3. CASE node \in AsyncArchiveIoServiceNodes
      <3>1. AsyncTypeInvariant
        BY <1>1, AsyncStrongTypeProjectsAsyncType
      <3>2. ENABLED PostGstServiceIoWorker(node)
        BY <1>1, <2>2, <2>3, <3>1,
           QueuedIoEnablesPostGstService
      <3>3. PostGstServiceIoWorker(node)
               => PostGstServiceIoWorker(node)
                    /\ PostGstProductiveEffect
        BY <1>1, <2>2, <2>3,
           DueOrdinaryIoWorkerDecreasesBlockerDebt
           DEF PostGstProductiveEffect
      <3>4. ENABLED (
               PostGstServiceIoWorker(node)
                 /\ PostGstProductiveEffect)
        BY <3>2, <3>3, ENABLEDaxioms
      <3> QED BY <2>3, <3>4
           DEF ImmediateProductiveFairActionReady
    <2>4. CASE node \notin AsyncArchiveIoServiceNodes
      <3>1. node \in asyncHistoricalRecoveryTargets
        BY <2>2, <2>4, Isa DEF AsyncTimedServiceNodes
      <3>2. ENABLED
               PostGstServiceHistoricalRecoveryIoWorker(node)
        BY <1>1, <2>2, <3>1,
           GstHistoricalIoWorkerIsEnabled
      <3>3. PostGstServiceHistoricalRecoveryIoWorker(node)
               => PostGstServiceHistoricalRecoveryIoWorker(node)
                    /\ PostGstProductiveEffect
        BY <1>1, <2>2, <3>1,
           DueHistoricalIoWorkerDecreasesBlockerDebt
           DEF PostGstProductiveEffect
      <3>4. ENABLED (
               PostGstServiceHistoricalRecoveryIoWorker(node)
                 /\ PostGstProductiveEffect)
        BY <3>2, <3>3, ENABLEDaxioms
      <3> QED BY <1>1, <3>1, <3>4,
           AsyncHistoricalRecoveryTypeInvariant, Isa
           DEF AsyncStrongTypeInvariant,
               ImmediateProductiveFairActionReady
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

(***************************************************************************
Overdue packet lane classification.

Admission always selects `OldestDueSourcePacket`, not an arbitrary overdue
occurrence.  The selected head must therefore itself be overdue before its
removal can witness `PostGstOverduePacketOwnershipExits`.
***************************************************************************)

IngressCoalescingGateAllows(item) ==
  /\ IngressHasCoalescingOwner(item)
  /\ \/ item.kind # "CertifiedResponse"
     \/ CertifiedResponseClaimMatches(item)

IngressPacketCanLeaveTransport(item) ==
  \/ CanAdmitIngressItem(item)
  \/ IngressCoalescingGateAllows(item)
  \/ IngressPacketPolicyRejected(item)

AdmissibleOverdueLaneHead(recipient, source) ==
  /\ DueSourcePackets(recipient, source) # {}
  /\ LET packet == OldestDueSourcePacket(recipient, source)
         item == packet.item
     IN /\ packet \in OverdueResponsivePackets
        /\ IngressPacketCanLeaveTransport(item)

FiniteIngressResourceOwnershipBlocked(item) ==
  \/ ~(IngressDepth(item.envelope.recipient)
         < IngressUsableCapacityAfterAdmission(item))
  \/ ~AsyncTimeoutVoteByteGateAllows(item)
  \/ ~AsyncTransportCompletionOwnerGateAllows(item)

IngressCapacityOrDeferredOwnershipBlocked(item) ==
  \/ FiniteIngressResourceOwnershipBlocked(item)
  \/ ~CertifiedResponseFreshClaimGateAllows(item)
  \/ ~AsyncUntrustedGenericCompletionGateAllows(item)

THEOREM IngressGateBlockIsExact ==
  \A item:
    ~CanAdmitIngressItem(item)
      <=> IngressCapacityOrDeferredOwnershipBlocked(item)
BY Isa
   DEF CanAdmitIngressItem,
       IngressCapacityOrDeferredOwnershipBlocked

CertifiedResponseRecipientClaimContention(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ CertifiedResponseAuthorized(item)
  /\ ~CertifiedResponseClaimMatches(item)
  /\ ~CertifiedResponseRecipientClaimAvailable(item)

THEOREM CertifiedResponseClaimGateBlockIsCoalescedRejectedOrRetained ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ ~CertifiedResponseFreshClaimGateAllows(item)
      => \/ IngressCoalescingGateAllows(item)
         \/ IngressPacketPolicyRejected(item)
         \/ CertifiedResponseRecipientClaimContention(item)
BY Isa
   DEF CertifiedResponseFreshClaimGateAllows,
       CertifiedResponseAuthorityReady,
       CertifiedResponseAuthorityClaimed,
       CertifiedResponseRecipientClaimAvailable,
       CertifiedResponseClaimsAt,
       CertifiedResponseClaimMatches,
       CertifiedResponseAuthorized,
       MatchingCertifiedRequests,
       ActiveCertifiedRequestHashes,
       ActiveCertifiedRequestHashesIn,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       AsyncCertifiedResponseClaimIngressOwnershipInvariant,
       CertifiedResponseClaimIngressOwner,
       IngressCoalescingGateAllows,
       CertifiedResponsePacketPolicyRejected,
       IngressPacketPolicyRejected,
       IngressHasCoalescingOwner, IngressCoalescingIdentity,
       IngressResourceSource, IngressLaneDepth, SequenceSet,
       CertifiedResponseRecipientClaimContention

THEOREM GenericUntrustedGateBlockIsPolicyRejected ==
  \A item:
    ~AsyncUntrustedGenericCompletionGateAllows(item)
      => IngressPacketPolicyRejected(item)
BY Isa
   DEF AsyncUntrustedGenericCompletionGateAllows,
       UntrustedGenericCompletionPacketPolicyRejected,
       IngressPacketPolicyRejected

THEOREM NonLeavingIngressPacketHasFiniteResourceOrClaimDebt ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ ~IngressPacketCanLeaveTransport(item)
      => \/ FiniteIngressResourceOwnershipBlocked(item)
         \/ CertifiedResponseRecipientClaimContention(item)
BY IngressGateBlockIsExact,
   CertifiedResponseClaimGateBlockIsCoalescedRejectedOrRetained,
   GenericUntrustedGateBlockIsPolicyRejected, Isa
   DEF IngressPacketCanLeaveTransport,
       IngressCapacityOrDeferredOwnershipBlocked,
       FiniteIngressResourceOwnershipBlocked

OverduePacketLaneHeadBlocked(packet) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
      head == OldestDueSourcePacket(recipient, source)
  IN \/ head \notin OverdueResponsivePackets
     \/ ~IngressPacketCanLeaveTransport(head.item)

THEOREM OverdueLaneHeadBlockIsShadowResourceOrClaimDebt ==
  \A packet:
    /\ AsyncStrongTypeInvariant
    /\ OverduePacketLaneHeadBlocked(packet)
      => LET recipient == packet.item.envelope.recipient
             source == packet.item.source
             head == OldestDueSourcePacket(recipient, source)
         IN \/ head \notin OverdueResponsivePackets
            \/ FiniteIngressResourceOwnershipBlocked(head.item)
            \/ CertifiedResponseRecipientClaimContention(head.item)
BY NonLeavingIngressPacketHasFiniteResourceOrClaimDebt, Isa
   DEF OverduePacketLaneHeadBlocked

AllOverduePacketLaneHeadsBlocked ==
  \A packet \in OverdueResponsivePackets:
    OverduePacketLaneHeadBlocked(packet)

THEOREM OverduePacketHasTypedDueLane ==
  AsyncStrongTypeInvariant
    => \A packet \in OverdueResponsivePackets:
         LET recipient == packet.item.envelope.recipient
             source == packet.item.source
         IN /\ recipient \in ValidatorIds
            /\ source \in AsyncIngressSources
            /\ DueSourcePackets(recipient, source) # {}
BY AsyncStrongTypeProjectsAsyncType,
   HistoricalRecoveryTargetsAreValidators,
   AsyncArchiveIoServiceNodesAreValidators,
   Isa
   DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncSchedulerTypeInvariant, AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncPacketContentTypeInvariant, AsyncPacketTyped,
       AsyncItemTyped, OverdueResponsivePackets,
       AsyncPacketOwnsClockDeadline,
       DueSourcePackets, AsyncTimedServiceNodes

THEOREM ArchiveAdmissibleOverdueHeadEnablesOwnershipExit ==
  \A recipient \in AsyncArchiveIoServiceNodes,
     source \in AsyncIngressSources:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AdmissibleOverdueLaneHead(recipient, source)
    => ENABLED (
         PostGstAdmitHiddenPacket(recipient, source)
           /\ PostGstOverduePacketOwnershipExits)
BY AsyncStrongTypeProjectsAsyncType,
   AsyncArchiveIoServiceNodesAreValidators,
   AsyncArchiveIoServiceNodesAreResponsive,
   GstResponsiveNodesAreUp,
   GstExcludesResponsiveReplayQuarantine,
   OldestDueSourcePacketFacts,
   ExpandENABLED, Isa
   DEF AdmissibleOverdueLaneHead,
       PostGstOverduePacketOwnershipExits,
       PostGstAdmitHiddenPacket, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       IngressPacketCanLeaveTransport,
       IngressCoalescingGateAllows,
       CanAdmitIngressItem, DueSourcePackets,
       PostGstServiceNodes, AsyncTimedServiceNodes,
       AsyncNonRunnerOuterFrame, AsyncNonCrashOuterFrame,
       AsyncCoreOuterFrame, AsyncAllVars, AsyncSchedulerVars,
       AsyncRecoveryVars, AsyncIoVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars, LeaveCausalQueues, vars

THEOREM HistoricalAdmissibleOverdueHeadEnablesOwnershipExit ==
  \A recipient \in asyncHistoricalRecoveryTargets,
     source \in AsyncIngressSources:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AdmissibleOverdueLaneHead(recipient, source)
    => ENABLED (
         PostGstAdmitHistoricalRecoveryPacket(recipient, source)
           /\ PostGstOverduePacketOwnershipExits)
BY AsyncStrongTypeProjectsAsyncType,
   HistoricalRecoveryTargetsAreValidators,
   GstExcludesResponsiveReplayQuarantine,
   OldestDueSourcePacketFacts,
   ExpandENABLED, Isa
   DEF AsyncStrongTypeInvariant,
       AsyncHistoricalRecoveryTypeInvariant,
       AdmissibleOverdueLaneHead,
       PostGstOverduePacketOwnershipExits,
       PostGstAdmitHistoricalRecoveryPacket,
       HistoricalRecoveryPacketCorridor,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       IngressPacketCanLeaveTransport,
       IngressCoalescingGateAllows,
       CanAdmitIngressItem, DueSourcePackets,
       PostGstServiceNodes, AsyncTimedServiceNodes,
       AsyncNonRunnerOuterFrame, AsyncNonCrashOuterFrame,
       AsyncCoreOuterFrame, AsyncAllVars, AsyncSchedulerVars,
       AsyncRecoveryVars, AsyncIoVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars, LeaveCausalQueues, vars

THEOREM AdmissibleOverdueLaneHeadIsImmediatelyProductive ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ recipient \in AsyncTimedServiceNodes
    /\ AdmissibleOverdueLaneHead(recipient, source)
    => ImmediateProductiveFairActionReady
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncStrongTypeInvariant,
                gst,
                recipient \in AsyncTimedServiceNodes,
                AdmissibleOverdueLaneHead(recipient, source)
         PROVE ImmediateProductiveFairActionReady
    <2>1. CASE recipient \in AsyncArchiveIoServiceNodes
      <3>1. ENABLED (
               PostGstAdmitHiddenPacket(recipient, source)
                 /\ PostGstOverduePacketOwnershipExits)
        BY <1>1, <2>1,
           ArchiveAdmissibleOverdueHeadEnablesOwnershipExit
      <3>2. PostGstOverduePacketOwnershipExits
               => PostGstProductiveEffect
        BY DEF PostGstProductiveEffect
      <3>3. ENABLED (
               PostGstAdmitHiddenPacket(recipient, source)
                 /\ PostGstProductiveEffect)
        BY <3>1, <3>2, ENABLEDaxioms
      <3> QED BY <2>1, <3>3
           DEF ImmediateProductiveFairActionReady
    <2>2. CASE recipient \notin AsyncArchiveIoServiceNodes
      <3>1. recipient \in asyncHistoricalRecoveryTargets
        BY <1>1, <2>2, Isa DEF AsyncTimedServiceNodes
      <3>2. ENABLED (
               PostGstAdmitHistoricalRecoveryPacket(recipient, source)
                 /\ PostGstOverduePacketOwnershipExits)
        BY <1>1, <3>1,
           HistoricalAdmissibleOverdueHeadEnablesOwnershipExit
      <3>3. PostGstOverduePacketOwnershipExits
               => PostGstProductiveEffect
        BY DEF PostGstProductiveEffect
      <3>4. ENABLED (
               PostGstAdmitHistoricalRecoveryPacket(recipient, source)
                 /\ PostGstProductiveEffect)
        BY <3>2, <3>3, ENABLEDaxioms
      <3> QED BY <1>1, <3>1, <3>4,
           AsyncHistoricalRecoveryTypeInvariant, Isa
           DEF AsyncStrongTypeInvariant,
               ImmediateProductiveFairActionReady
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM UnblockedOverdueLaneHeadIsImmediatelyProductive ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ OverdueResponsivePackets # {}
  /\ ~AllOverduePacketLaneHeadsBlocked
  => ImmediateProductiveFairActionReady
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              gst,
              OverdueResponsivePackets # {},
              ~AllOverduePacketLaneHeadsBlocked
         PROVE ImmediateProductiveFairActionReady
    <2>1. PICK packet \in OverdueResponsivePackets:
             ~OverduePacketLaneHeadBlocked(packet)
      BY <1>1 DEF AllOverduePacketLaneHeadsBlocked
    <2> DEFINE Recipient == packet.item.envelope.recipient
    <2> DEFINE Source == packet.item.source
    <2>1a. /\ Recipient \in ValidatorIds
            /\ Source \in AsyncIngressSources
            /\ DueSourcePackets(Recipient, Source) # {}
      BY <1>1, <2>1, OverduePacketHasTypedDueLane
         DEF Recipient, Source
    <2>2. LET head == OldestDueSourcePacket(Recipient, Source)
           IN /\ head \in OverdueResponsivePackets
              /\ IngressPacketCanLeaveTransport(head.item)
      BY <2>1, Isa
         DEF OverduePacketLaneHeadBlocked, Recipient, Source
    <2>3. AdmissibleOverdueLaneHead(Recipient, Source)
      BY <2>1a, <2>2 DEF AdmissibleOverdueLaneHead
    <2>4. Recipient \in AsyncTimedServiceNodes
      BY <2>2 DEF OverdueResponsivePackets,
                     AsyncPacketOwnsClockDeadline, Recipient
    <2> QED BY <1>1, <2>1a, <2>3, <2>4,
         AdmissibleOverdueLaneHeadIsImmediatelyProductive
  <1> QED BY <1>1

(***************************************************************************
Candidate/Serve rank actions.

This normal form mirrors every fair-action domain.  It is intentionally
separate from temporal `~>` descent: only an ENABLED concrete action already
conjoined with a strict successor-state rank effect is counted.
***************************************************************************)

ImmediateProtectedRankEffect ==
  \/ LiveProtectedServiceRankDecreaseStep
  \/ LiveProtectedServeRankDecreaseStep

ImmediateProtectedRankFairActionReady ==
  \/ ENABLED (AsyncTick /\ ImmediateProtectedRankEffect)
  \/ \E node \in AsyncCurrentResponsiveVoters:
       \/ ENABLED (
            PostGstRunNode(node) /\ ImmediateProtectedRankEffect)
       \/ ENABLED (
            PostGstCommitCertificateDiscovery(node)
              /\ ImmediateProtectedRankEffect)
  \/ \E node \in AsyncResponsiveAppliedArchiveServers:
       ENABLED (
         PostGstRunHistoricalServer(node)
           /\ ImmediateProtectedRankEffect)
  \/ \E node \in AsyncArchiveIoServiceNodes:
       ENABLED (
         PostGstServiceIoWorker(node) /\ ImmediateProtectedRankEffect)
  \/ \E node \in Responsive:
       \/ ENABLED (
            PostGstOpenHistoricalRecovery(node)
              /\ ImmediateProtectedRankEffect)
       \/ ENABLED (
            PostGstRunHistoricalRecoveryNode(node)
              /\ ImmediateProtectedRankEffect)
       \/ ENABLED (
            PostGstHistoricalCommitCertificateDiscovery(node)
              /\ ImmediateProtectedRankEffect)
       \/ ENABLED (
            PostGstServiceHistoricalRecoveryIoWorker(node)
              /\ ImmediateProtectedRankEffect)
  \/ \E recipient \in AsyncArchiveIoServiceNodes,
       source \in AsyncIngressSources:
       ENABLED (
         PostGstAdmitHiddenPacket(recipient, source)
           /\ ImmediateProtectedRankEffect)
  \/ \E recipient \in ValidatorIds,
       source \in AsyncIngressSources:
       ENABLED (
         PostGstAdmitHistoricalRecoveryPacket(recipient, source)
           /\ ImmediateProtectedRankEffect)

THEOREM ImmediateProtectedRankEffectIsProductive ==
  ImmediateProtectedRankEffect => PostGstProductiveEffect
BY DEF ImmediateProtectedRankEffect, PostGstProductiveEffect

THEOREM ImmediateProtectedRankFairActionIsImmediatelyProductive ==
  ImmediateProtectedRankFairActionReady
    => ImmediateProductiveFairActionReady
BY ImmediateProtectedRankEffectIsProductive,
   ENABLEDaxioms, Isa
   DEF ImmediateProtectedRankFairActionReady,
       ImmediateProductiveFairActionReady

(***************************************************************************
Exact ingress dependency owners.

The residual is not one anonymous "full ingress" state.  A blocked overdue
packet has four concrete owners before ordinary scheduler ownership begins:

  * an older due but non-overdue packet selected by the same source-lane
    oldest-packet rule;
  * the finite set of occupied ingress slots which supplies capacity debt;
  * the earlier TimeoutVote byte owner in the same lane; or
  * the earlier shared physical completion owner in the normalized resource
    lane.

The first component is occurrence-based.  In particular, an authenticated
CertifiedResponse can be overdue through its authentication history while its
outer relay source differs from its aggregate untrusted resource lane.  An
older unauthenticated packet in the same outer transport lane can therefore
remain due but not overdue and is a genuine head-of-line shadow.  Ingress
capacity and owner accounting, unlike transport ordering, always use
`IngressResourceSource`.
***************************************************************************)

OlderDueNonOverdueLaneShadows(packet) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
  IN {shadow \in DueSourcePackets(recipient, source):
        /\ shadow \notin OverdueResponsivePackets
        /\ shadow.sentAt <= packet.sentAt}

OlderDueNonOverdueShadowDebt(packet) ==
  Cardinality(OlderDueNonOverdueLaneShadows(packet))

FreshIngressCapacityOwnerDebt(item) ==
  IF IngressDepth(item.envelope.recipient)
       < IngressUsableCapacityAfterAdmission(item)
  THEN 0
  ELSE IngressDepth(item.envelope.recipient) + 1

TimeoutVoteByteOwnerIndices(item) ==
  {index \in 1..Len(
      IngressLane(
        item.envelope.recipient, IngressResourceSource(item))):
     IngressLane(
       item.envelope.recipient,
       IngressResourceSource(item))[index].kind
       = "TimeoutVote"}

TimeoutVoteByteOwnerDebt(item) ==
  Cardinality(TimeoutVoteByteOwnerIndices(item))

TransportCompletionOwnerIndices(item) ==
  {index \in 1..Len(
      IngressLane(
        item.envelope.recipient, IngressResourceSource(item))):
     IngressUsesPhysicalCompletionOwner(
       IngressLane(
         item.envelope.recipient,
         IngressResourceSource(item))[index])}

TransportCompletionOwnerDebt(item) ==
  Cardinality(TransportCompletionOwnerIndices(item))

(***************************************************************************
Exact shared-completion debt under indexed lane removal.

Ingress removes an arbitrary drainable position, not necessarily the lane
head.  The index-shift bijection below proves that removing a physical
completion owner lowers the exact owner count by one even when equal item
values occur elsewhere in the lane.  Counting positions rather than values is
essential: two byte-identical occurrences are still two physical queue owners.
***************************************************************************)

IngressPhysicalCompletionPositions(sequence) ==
  {index \in 1..Len(sequence):
     IngressUsesPhysicalCompletionOwner(sequence[index])}

THEOREM NonPhysicalAppendPreservesIngressPhysicalCompletionPositions ==
  \A sequence, item:
    /\ sequence \in Seq(Range(sequence))
    /\ ~IngressUsesPhysicalCompletionOwner(item)
    => IngressPhysicalCompletionPositions(Append(sequence, item))
         = IngressPhysicalCompletionPositions(sequence)
BY AppendSequenceFacts, IsaT(180)
   DEF IngressPhysicalCompletionPositions

IngressRemovalIndexShift(removed, index) ==
  IF index < removed THEN index ELSE index - 1

THEOREM IngressRemovalIndexShiftIsInjectiveAwayFromRemoved ==
  \A removed, left, right \in Nat:
    /\ left # removed
    /\ right # removed
    /\ IngressRemovalIndexShift(removed, left) =
         IngressRemovalIndexShift(removed, right)
    => left = right
BY SMT DEF IngressRemovalIndexShift

IngressPhysicalCompletionShift(sequence, removed) ==
  [oldIndex \in
     IngressPhysicalCompletionPositions(sequence) \ {removed} |->
     IngressRemovalIndexShift(removed, oldIndex)]

THEOREM IngressPhysicalCompletionShiftIsBijection ==
  \A sequence, removed:
    /\ sequence \in Seq(Range(sequence))
    /\ removed \in 1..Len(sequence)
    => LET after == SequenceWithoutIndex(sequence, removed)
       IN IngressPhysicalCompletionShift(sequence, removed)
            \in Bijection(
                 IngressPhysicalCompletionPositions(sequence) \ {removed},
                 IngressPhysicalCompletionPositions(after))
PROOF
  <1>1. ASSUME NEW sequence, NEW removed,
                sequence \in Seq(Range(sequence)),
                removed \in 1..Len(sequence)
         PROVE LET after == SequenceWithoutIndex(sequence, removed)
               IN IngressPhysicalCompletionShift(sequence, removed)
                    \in Bijection(
                         IngressPhysicalCompletionPositions(sequence)
                           \ {removed},
                         IngressPhysicalCompletionPositions(after))
    <2> DEFINE After == SequenceWithoutIndex(sequence, removed)
    <2> DEFINE Old ==
          IngressPhysicalCompletionPositions(sequence) \ {removed}
    <2> DEFINE New == IngressPhysicalCompletionPositions(After)
    <2>1. /\ After \in Seq(Range(sequence))
           /\ Len(After) = Len(sequence) - 1
           /\ \A newIndex \in 1..Len(After):
                After[newIndex] =
                  IF newIndex < removed
                  THEN sequence[newIndex]
                  ELSE sequence[newIndex + 1]
      BY <1>1, SequenceWithoutIndexFacts DEF After
    <2>2. /\ Old \subseteq 1..Len(sequence)
           /\ New \subseteq 1..Len(After)
      BY DEF Old, New, IngressPhysicalCompletionPositions
    <2>3. \A oldIndex \in Old:
             IngressRemovalIndexShift(removed, oldIndex) \in New
      BY <1>1, <2>1, <2>2, SMT
         DEF Old, New, IngressPhysicalCompletionPositions,
             IngressRemovalIndexShift
    <2>4. \A newIndex \in New:
             IF newIndex < removed
             THEN newIndex \in Old
             ELSE newIndex + 1 \in Old
      BY <1>1, <2>1, <2>2, SMT
         DEF Old, New, IngressPhysicalCompletionPositions,
             IngressRemovalIndexShift
    <2>5. IngressPhysicalCompletionShift(sequence, removed)
             \in [Old -> New]
      BY <2>3, Isa DEF IngressPhysicalCompletionShift, Old
    <2>6. ASSUME NEW left \in Old, NEW right \in Old,
                  IngressPhysicalCompletionShift(
                    sequence, removed)[left] =
                    IngressPhysicalCompletionShift(
                      sequence, removed)[right]
           PROVE left = right
      <3>1. /\ IngressPhysicalCompletionShift(
                    sequence, removed)[left] =
                  IngressRemovalIndexShift(removed, left)
             /\ IngressPhysicalCompletionShift(
                    sequence, removed)[right] =
                  IngressRemovalIndexShift(removed, right)
        BY <2>6, Isa DEF IngressPhysicalCompletionShift, Old
      <3>2. /\ left \in Nat
             /\ right \in Nat
             /\ left # removed
             /\ right # removed
        BY <2>2, <2>6, Isa DEF Old
      <3> QED BY <2>6, <3>1, <3>2,
           IngressRemovalIndexShiftIsInjectiveAwayFromRemoved
    <2>7. IngressPhysicalCompletionShift(sequence, removed)
             \in Injection(Old, New)
      BY <2>5, <2>6 DEF Injection, IsInjective
    <2>8. ASSUME NEW newIndex \in New
           PROVE \E oldIndex \in Old:
                   IngressPhysicalCompletionShift(
                     sequence, removed)[oldIndex] = newIndex
      <3>1. CASE newIndex < removed
        <4>1. newIndex \in Old
          BY <2>4, <2>8, <3>1
        <4>2. IngressPhysicalCompletionShift(
                 sequence, removed)[newIndex] = newIndex
          BY <3>1, <4>1, Isa
             DEF IngressPhysicalCompletionShift,
                 IngressRemovalIndexShift, Old
        <4> QED BY <4>1, <4>2
      <3>2. CASE ~(newIndex < removed)
        <4>1. newIndex + 1 \in Old
          BY <2>4, <2>8, <3>2
        <4>2. /\ newIndex \in Nat
               /\ ~(newIndex + 1 < removed)
               /\ (newIndex + 1) - 1 = newIndex
          BY <2>2, <2>8, <3>2, SMT
        <4>3. IngressPhysicalCompletionShift(
                 sequence, removed)[newIndex + 1] = newIndex
          BY <4>1, <4>2, Isa
             DEF IngressPhysicalCompletionShift,
                 IngressRemovalIndexShift, Old
        <4> QED BY <4>1, <4>3
      <3> QED BY <3>1, <3>2
    <2>9. IngressPhysicalCompletionShift(sequence, removed)
             \in Surjection(Old, New)
      BY <2>5, <2>8 DEF Surjection
    <2> QED BY <2>7, <2>9 DEF Bijection, Old, New
  <1> QED BY <1>1

THEOREM IngressPhysicalCompletionCountDropsAfterOwnerRemoval ==
  \A sequence, removed:
    /\ sequence \in Seq(Range(sequence))
    /\ removed \in 1..Len(sequence)
    /\ IngressUsesPhysicalCompletionOwner(sequence[removed])
    => LET after == SequenceWithoutIndex(sequence, removed)
       IN Cardinality(IngressPhysicalCompletionPositions(after)) + 1 =
            Cardinality(IngressPhysicalCompletionPositions(sequence))
PROOF
  <1>1. ASSUME NEW sequence, NEW removed,
                sequence \in Seq(Range(sequence)),
                removed \in 1..Len(sequence),
                IngressUsesPhysicalCompletionOwner(sequence[removed])
         PROVE LET after == SequenceWithoutIndex(sequence, removed)
               IN Cardinality(
                    IngressPhysicalCompletionPositions(after)) + 1 =
                    Cardinality(
                      IngressPhysicalCompletionPositions(sequence))
    <2> DEFINE All == IngressPhysicalCompletionPositions(sequence)
    <2> DEFINE Remaining == All \ {removed}
    <2> DEFINE After == SequenceWithoutIndex(sequence, removed)
    <2> DEFINE New == IngressPhysicalCompletionPositions(After)
    <2>1. /\ removed \in All
           /\ All \subseteq 1..Len(sequence)
           /\ IsFiniteSet(All)
      BY <1>1, FS_Interval, FS_Subset
         DEF All, IngressPhysicalCompletionPositions
    <2>2. /\ IsFiniteSet(Remaining)
           /\ Cardinality(Remaining) + 1 = Cardinality(All)
      BY <2>1, FS_RemoveElement DEF Remaining
    <2>3. ExistsBijection(Remaining, New)
      BY <1>1, IngressPhysicalCompletionShiftIsBijection
         DEF ExistsBijection, Remaining, New, All, After
    <2>4. Cardinality(New) = Cardinality(Remaining)
      BY <2>2, <2>3, FS_Bijection
    <2> QED BY <2>2, <2>4 DEF All, Remaining, New, After
  <1> QED BY <1>1

(***************************************************************************
The aggregate ingress bound already reserves the first physical-completion
slot of every source.  A CertifiedResponse is charged to the aggregate
untrusted source.  When that source has no physical owner, appending the
response removes its missing-completion reservation.  If the lane was empty,
the new one-element continuation reservation replaces the empty-source
reservation; otherwise both are absent.  Thus the total protected-slot count
drops by exactly one, and the capacity invariant supplies the response's
fresh admission slot without waiting for unrelated ingress drainage.
***************************************************************************)

THEOREM FreshCertifiedResponseConsumesExactlyOneReservedIngressSlot ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ AsyncItemTyped(item)
    /\ item.kind = "CertifiedResponse"
    /\ AsyncTransportCompletionOwnerGateAllows(item)
    => IngressProtectedSlotCountAfterAdmission(item) + 1 =
         IngressProtectedSlotCountFor(
           asyncIngressLanes, item.envelope.recipient)
BY AsyncStrongTypeProjectsAsyncType,
   IngressProtectedSlotCountIsNatural,
   IsaT(300)
   DEF AsyncStrongTypeInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIngressTypeInvariant,
       AsyncIngressTopologyTypeInvariant,
       AsyncIngressCapacityTypeInvariant,
       AsyncIngressContentTypeInvariant,
       AsyncConfiguration, ValidatorIds,
       AsyncIngressSources, AsyncArchiveServerIds,
       AsyncUntrustedSource,
       IngressResourceSource,
       IngressAdmissionClass,
       IngressUsesPhysicalCompletionOwner,
       AsyncTransportCompletionOwnerGateAllows,
       IngressLaneHasTransportCompletionIn,
       IngressLanesAfterAdmission,
       IngressProtectedSlotCountAfterAdmission,
       IngressProtectedSlotCountFor,
       IngressProtectedSourcesFor,
       IngressTimeoutVoteProtectedSourcesFor,
       IngressTransportCompletionProtectedSourcesFor,
       IngressContinuationProtectedSourcesFor,
       IngressLane, IngressLaneDepth,
       SequenceSet

THEOREM FreshCertifiedResponsePhysicalGateSuppliesIngressCapacity ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ AsyncItemTyped(item)
    /\ item.kind = "CertifiedResponse"
    /\ AsyncTransportCompletionOwnerGateAllows(item)
    => IngressDepth(item.envelope.recipient)
         < IngressUsableCapacityAfterAdmission(item)
BY FreshCertifiedResponseConsumesExactlyOneReservedIngressSlot,
   IngressProtectedSlotCountIsNatural,
   SMT
   DEF AsyncStrongTypeInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIngressTypeInvariant,
       AsyncIngressCapacityTypeInvariant,
       AsyncConfiguration,
       IngressUsableCapacityAfterAdmission,
       IngressProtectedSlotCountAfterAdmission

(***************************************************************************
A positive physical owner gate is non-refillable.

Network admission may append an ordinary item to the same lane, but it cannot
append another Chunk/CertifiedResponse while any physical owner is present.
Coalescing and policy rejection do not touch the lane.  Consequently exact
physical-owner cardinality is invariant under every admission action until a
runner removes an owner.
***************************************************************************)

THEOREM PositivePhysicalCompletionDebtAdmissionPreservesDebt ==
  \A blocked,
     recipient \in ValidatorIds,
     source \in AsyncIngressSources:
    /\ AsyncStrongTypeInvariant
    /\ AsyncItemTyped(blocked)
    /\ TransportCompletionOwnerDebt(blocked) > 0
    /\ AdmitIngressPacket(recipient, source)
    => TransportCompletionOwnerDebt(blocked)' =
         TransportCompletionOwnerDebt(blocked)
PROOF
  <1>1. ASSUME NEW blocked,
                NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncStrongTypeInvariant,
                AsyncItemTyped(blocked),
                TransportCompletionOwnerDebt(blocked) > 0,
                AdmitIngressPacket(recipient, source)
         PROVE TransportCompletionOwnerDebt(blocked)' =
                 TransportCompletionOwnerDebt(blocked)
    <2>1. CASE CoalesceHiddenPacket(recipient, source)
      BY <2>1
         DEF CoalesceHiddenPacket, TransportCompletionOwnerDebt,
             TransportCompletionOwnerIndices, IngressLane
    <2>2. CASE DropPolicyRejectedHiddenPacket(recipient, source)
      BY <2>2
         DEF DropPolicyRejectedHiddenPacket,
             TransportCompletionOwnerDebt,
             TransportCompletionOwnerIndices, IngressLane
    <2>3. CASE AdmitHiddenPacket(recipient, source)
      <3> DEFINE Packet == OldestDueSourcePacket(recipient, source)
      <3> DEFINE Item == Packet.item
      <3> DEFINE Resource == IngressResourceSource(Item)
      <3> DEFINE BlockedRecipient == blocked.envelope.recipient
      <3> DEFINE BlockedResource == IngressResourceSource(blocked)
      <3> DEFINE BlockedLane ==
             IngressLane(BlockedRecipient, BlockedResource)
      <3>1. /\ asyncIngressLanes' =
                    [asyncIngressLanes EXCEPT
                       ![recipient][Resource] = Append(@, Item)]
             /\ CanAdmitIngressItem(Item)
        BY <2>3
           DEF AdmitHiddenPacket, Packet, Item, Resource
      <3>2. /\ BlockedLane \in Seq(Range(BlockedLane))
             /\ TransportCompletionOwnerIndices(blocked) # {}
             /\ IngressLaneHasTransportCompletionIn(
                  asyncIngressLanes,
                  BlockedRecipient,
                  BlockedResource)
        BY <1>1, FS_CardinalityType, Isa
           DEF AsyncStrongTypeInvariant,
               AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncIngressTypeInvariant,
               AsyncIngressTopologyTypeInvariant,
               AsyncIngressContentTypeInvariant,
               TransportCompletionOwnerDebt,
               TransportCompletionOwnerIndices,
               IngressLaneHasTransportCompletionIn,
               IngressPhysicalCompletionPositions,
               IngressLane, BlockedLane,
               BlockedRecipient, BlockedResource,
               SequenceSet
      <3>3. CASE \/ recipient # BlockedRecipient
                   \/ Resource # BlockedResource
        BY <3>1, <3>3, Isa
           DEF TransportCompletionOwnerDebt,
               TransportCompletionOwnerIndices,
               IngressLane, Resource,
               BlockedRecipient, BlockedResource
      <3>4. CASE /\ recipient = BlockedRecipient
                   /\ Resource = BlockedResource
        <4>1. ~IngressUsesPhysicalCompletionOwner(Item)
          BY <3>1, <3>2, <3>4, Isa
             DEF CanAdmitIngressItem,
                 AsyncTransportCompletionOwnerGateAllows,
                 Resource, BlockedRecipient, BlockedResource
        <4>2. IngressPhysicalCompletionPositions(
                   Append(BlockedLane, Item))
                 = IngressPhysicalCompletionPositions(BlockedLane)
          BY <3>2, <4>1,
             NonPhysicalAppendPreservesIngressPhysicalCompletionPositions
        <4>3. TransportCompletionOwnerDebt(blocked)' =
                    TransportCompletionOwnerDebt(blocked)
          BY <3>1, <3>4, <4>2, Isa
             DEF TransportCompletionOwnerDebt,
                 TransportCompletionOwnerIndices,
                 IngressPhysicalCompletionPositions,
                 IngressLane, Resource,
                 BlockedRecipient, BlockedResource, BlockedLane
        <4> QED BY <4>3
      <3> QED BY <3>3, <3>4
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF AdmitIngressPacket
  <1> QED BY <1>1

(***************************************************************************
The existing RuntimeReachRank deliberately maps Runtime to zero.  That is the
right rank for Local/Ingress descent, but it cannot certify the serialized
Runtime-to-Local reset.  The reset-aware rank reverses exactly that edge.
It is a dependency-path component, not a replacement for RuntimeReachRank:
an Ingress-to-Runtime phase advance raises it, as proved below.
***************************************************************************)

ResetAwareIngressReachRank(node) ==
  CASE asyncRunnerPhase[node] = "Runtime" ->
         AsyncQueueCapacity + AsyncIngressCapacity + 3
    [] asyncRunnerPhase[node] = "Local" ->
         asyncRunnerBudget[node] + AsyncIngressCapacity + 2
    [] OTHER -> asyncRunnerBudget[node] + 1

THEOREM StrongTypeHasFiniteOlderNonOverdueShadows ==
  AsyncStrongTypeInvariant
    => \A packet \in asyncTransport:
         /\ IsFiniteSet(OlderDueNonOverdueLaneShadows(packet))
         /\ OlderDueNonOverdueShadowDebt(packet) \in Nat
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant
         PROVE \A packet \in asyncTransport:
                 /\ IsFiniteSet(
                      OlderDueNonOverdueLaneShadows(packet))
                 /\ OlderDueNonOverdueShadowDebt(packet) \in Nat
    <2>1. ASSUME NEW packet \in asyncTransport
           PROVE /\ IsFiniteSet(
                       OlderDueNonOverdueLaneShadows(packet))
                 /\ OlderDueNonOverdueShadowDebt(packet) \in Nat
      <3>1. IsFiniteSet(asyncTransport)
        BY <1>1
           DEF AsyncStrongTypeInvariant,
               AsyncTransportTypeInvariant,
               AsyncPacketContentTypeInvariant
      <3>2. OlderDueNonOverdueLaneShadows(packet)
               \subseteq asyncTransport
        BY Isa
           DEF OlderDueNonOverdueLaneShadows,
               DueSourcePackets
      <3>3. IsFiniteSet(
               OlderDueNonOverdueLaneShadows(packet))
        BY <3>1, <3>2, FS_Subset
      <3> QED BY <3>3, FS_CardinalityType
           DEF OlderDueNonOverdueShadowDebt
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM IngressGateOwnerDebtsAreFiniteNaturals ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ AsyncItemTyped(item)
    => /\ FreshIngressCapacityOwnerDebt(item) \in Nat
       /\ TimeoutVoteByteOwnerDebt(item) \in Nat
       /\ TransportCompletionOwnerDebt(item) \in Nat
       /\ TimeoutVoteByteOwnerDebt(item) <= AsyncIngressCapacity
       /\ TransportCompletionOwnerDebt(item) <= AsyncIngressCapacity
BY AsyncIngressDepthForIsNatural,
   FS_Interval, FS_Subset, FS_CardinalityType, SMT, Isa
   DEF AsyncStrongTypeInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncIngressTypeInvariant,
       AsyncIngressCapacityTypeInvariant,
       AsyncIngressDepthFor, AsyncIngressPairIndicesFor,
       IngressDepth, IngressLaneDepth,
       IngressResourceSource,
       FreshIngressCapacityOwnerDebt,
       TimeoutVoteByteOwnerDebt, TimeoutVoteByteOwnerIndices,
       TransportCompletionOwnerDebt,
       TransportCompletionOwnerIndices,
       IngressUsesPhysicalCompletionOwner

THEOREM TimeoutByteGateHasExactFiniteOwner ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ AsyncItemTyped(item)
    => (~AsyncTimeoutVoteByteGateAllows(item)
          <=> /\ item.kind = "TimeoutVote"
              /\ IngressResourceSource(item) \in ValidatorIds
              /\ TimeoutVoteByteOwnerDebt(item) > 0)
BY IngressGateOwnerDebtsAreFiniteNaturals, Isa
   DEF AsyncStrongTypeInvariant, AsyncConfiguration,
       AsyncTimeoutVoteByteGateAllows,
       IngressLaneHasTimeoutVoteIn,
       TimeoutVoteByteOwnerDebt, TimeoutVoteByteOwnerIndices,
       IngressLane

THEOREM TransportCompletionGateHasExactFiniteOwner ==
  \A item:
    /\ AsyncStrongTypeInvariant
    /\ AsyncItemTyped(item)
    => (~AsyncTransportCompletionOwnerGateAllows(item)
          <=> /\ IngressUsesPhysicalCompletionOwner(item)
              /\ TransportCompletionOwnerDebt(item) > 0)
BY IngressGateOwnerDebtsAreFiniteNaturals, Isa
   DEF AsyncTransportCompletionOwnerGateAllows,
       IngressLaneHasTransportCompletionIn,
       IngressUsesPhysicalCompletionOwner,
       TransportCompletionOwnerDebt,
       TransportCompletionOwnerIndices, IngressLane

THEOREM ResetAwareIngressReachRankIsNatural ==
  AsyncTypeInvariant
    => \A node \in ValidatorIds:
         /\ ResetAwareIngressReachRank(node) \in Nat
         /\ ResetAwareIngressReachRank(node)
              < AsyncQueueCapacity
                  + 2 * AsyncIngressCapacity + 4
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration, ResetAwareIngressReachRank

THEOREM BoundedTransportServiceRankIsNatural ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    AsyncTypeInvariant
      => BoundedTransportServiceRank(recipient, source) \in Nat
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW source \in AsyncIngressSources,
                AsyncTypeInvariant
         PROVE BoundedTransportServiceRank(recipient, source) \in Nat
    <2>1. CASE source \in SequenceSet(
                         asyncIngressReady[recipient])
      <3>1. \E index \in 1..Len(asyncIngressReady[recipient]):
               asyncIngressReady[recipient][index] = source
        BY <2>1 DEF SequenceSet
      <3>2. IngressSourceServiceRank(recipient, source)
               \in 1..Len(asyncIngressReady[recipient])
        BY <3>1 DEF IngressSourceServiceRank
      <3> QED BY <2>1, <3>2, SMT
           DEF BoundedTransportServiceRank
    <2>2. CASE source \notin SequenceSet(
                         asyncIngressReady[recipient])
      BY <2>2 DEF BoundedTransportServiceRank
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Well-founded dependency product.

The prefix is the exact packet/ingress ownership spine.  The suffix reuses
the already-proved runner auxiliary, Stage-4 capacity, candidate-service, and
Serve-job orders.  Candidate and Serve ranks are parameters because those
owners have independent occurrence identities; the two concrete certificate
operators below insert an actual live owner and a canonical zero-position
placeholder for the other branch.
***************************************************************************)

IngressCandidateServeTailCarrier ==
  OwnedServiceRankCarrier \X OwnedServiceRankCarrier

IngressCandidateServeTailOrdering ==
  LexPairOrdering(
    OwnedServiceRankOrdering, OwnedServiceRankOrdering,
    OwnedServiceRankCarrier, OwnedServiceRankCarrier)

IngressStage4TailCarrier ==
  Stage4CapacityCarrier \X IngressCandidateServeTailCarrier

IngressStage4TailOrdering ==
  LexPairOrdering(
    Stage4CapacityOrdering, IngressCandidateServeTailOrdering,
    Stage4CapacityCarrier, IngressCandidateServeTailCarrier)

IngressReadyTailCarrier ==
  ReadyRunAuxCarrier \X IngressStage4TailCarrier

IngressReadyTailOrdering ==
  LexPairOrdering(
    ReadyRunAuxOrdering, IngressStage4TailOrdering,
    ReadyRunAuxCarrier, IngressStage4TailCarrier)

IngressResetTailCarrier == Nat \X IngressReadyTailCarrier

IngressResetTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), IngressReadyTailOrdering,
    Nat, IngressReadyTailCarrier)

IngressTransportTailCarrier == Nat \X IngressResetTailCarrier

IngressTransportTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), IngressResetTailOrdering,
    Nat, IngressResetTailCarrier)

IngressCompletionTailCarrier == Nat \X IngressTransportTailCarrier

IngressCompletionTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), IngressTransportTailOrdering,
    Nat, IngressTransportTailCarrier)

IngressTimeoutTailCarrier == Nat \X IngressCompletionTailCarrier

IngressTimeoutTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), IngressCompletionTailOrdering,
    Nat, IngressCompletionTailCarrier)

IngressCapacityTailCarrier == Nat \X IngressTimeoutTailCarrier

IngressCapacityTailOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), IngressTimeoutTailOrdering,
    Nat, IngressTimeoutTailCarrier)

IngressBoundaryDependencyCarrier ==
  Nat \X IngressCapacityTailCarrier

IngressBoundaryDependencyOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), IngressCapacityTailOrdering,
    Nat, IngressCapacityTailCarrier)

IngressBoundaryDependencyRank(packet, node, serviceRank, serveRank) ==
  <<OlderDueNonOverdueShadowDebt(packet),
    <<FreshIngressCapacityOwnerDebt(packet.item),
      <<TimeoutVoteByteOwnerDebt(packet.item),
        <<TransportCompletionOwnerDebt(packet.item),
          <<BoundedTransportServiceRank(
               packet.item.envelope.recipient,
               packet.item.source),
            <<ResetAwareIngressReachRank(node),
              <<ReadyRunAuxRank(node),
                <<Stage4CapacityRank(node),
                  <<serviceRank, serveRank>>>>>>>>>>>>>>>>>>

CandidateIngressDependencyRank(packet, candidate) ==
  IngressBoundaryDependencyRank(
    packet, candidate.node, CandidateServiceRank(candidate), <<5, 0>>)

ServeIngressDependencyRank(packet, node, job) ==
  IngressBoundaryDependencyRank(
    packet, node, <<2, 0>>, ServeJobRank(node, job))

THEOREM IngressBoundaryDependencyOrderingIsWellFounded ==
  IsWellFoundedOn(
    IngressBoundaryDependencyOrdering,
    IngressBoundaryDependencyCarrier)
PROOF
  <1>1. IsWellFoundedOn(
           IngressCandidateServeTailOrdering,
           IngressCandidateServeTailCarrier)
    BY OwnedServiceRankOrderingWellFounded,
       WFLexPairOrdering
       DEF IngressCandidateServeTailOrdering,
           IngressCandidateServeTailCarrier
  <1>2. IsWellFoundedOn(
           IngressStage4TailOrdering,
           IngressStage4TailCarrier)
    BY Stage4CapacityOrderingIsWellFounded,
       <1>1, WFLexPairOrdering
       DEF IngressStage4TailOrdering,
           IngressStage4TailCarrier
  <1>3. IsWellFoundedOn(
           IngressReadyTailOrdering,
           IngressReadyTailCarrier)
    BY ReadyRunAuxOrderingIsWellFounded,
       <1>2, WFLexPairOrdering
       DEF IngressReadyTailOrdering,
           IngressReadyTailCarrier
  <1>4. IsWellFoundedOn(
           IngressResetTailOrdering,
           IngressResetTailCarrier)
    BY NatLessThanWellFounded, <1>3, WFLexPairOrdering
       DEF IngressResetTailOrdering,
           IngressResetTailCarrier
  <1>5. IsWellFoundedOn(
           IngressTransportTailOrdering,
           IngressTransportTailCarrier)
    BY NatLessThanWellFounded, <1>4, WFLexPairOrdering
       DEF IngressTransportTailOrdering,
           IngressTransportTailCarrier
  <1>6. IsWellFoundedOn(
           IngressCompletionTailOrdering,
           IngressCompletionTailCarrier)
    BY NatLessThanWellFounded, <1>5, WFLexPairOrdering
       DEF IngressCompletionTailOrdering,
           IngressCompletionTailCarrier
  <1>7. IsWellFoundedOn(
           IngressTimeoutTailOrdering,
           IngressTimeoutTailCarrier)
    BY NatLessThanWellFounded, <1>6, WFLexPairOrdering
       DEF IngressTimeoutTailOrdering,
           IngressTimeoutTailCarrier
  <1>8. IsWellFoundedOn(
           IngressCapacityTailOrdering,
           IngressCapacityTailCarrier)
    BY NatLessThanWellFounded, <1>7, WFLexPairOrdering
       DEF IngressCapacityTailOrdering,
           IngressCapacityTailCarrier
  <1> QED
    BY NatLessThanWellFounded, <1>8, WFLexPairOrdering
       DEF IngressBoundaryDependencyOrdering,
           IngressBoundaryDependencyCarrier

THEOREM IngressBoundaryDependencyRankInCarrier ==
  \A packet \in asyncTransport, node \in ValidatorIds,
     serviceRank \in OwnedServiceRankCarrier,
     serveRank \in OwnedServiceRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ node = packet.item.envelope.recipient
    => IngressBoundaryDependencyRank(
         packet, node, serviceRank, serveRank)
         \in IngressBoundaryDependencyCarrier
BY AsyncStrongTypeProjectsAsyncType,
   StrongTypeHasFiniteOlderNonOverdueShadows,
   IngressGateOwnerDebtsAreFiniteNaturals,
   BoundedTransportServiceRankIsNatural,
   ResetAwareIngressReachRankIsNatural,
   ReadyRunAuxRankInCarrier,
   Stage4CapacityRankInCarrier, Isa
   DEF AsyncStrongTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncPacketContentTypeInvariant,
       IngressBoundaryDependencyRank,
       IngressBoundaryDependencyCarrier,
       IngressCapacityTailCarrier,
       IngressTimeoutTailCarrier,
       IngressCompletionTailCarrier,
       IngressTransportTailCarrier,
       IngressResetTailCarrier,
       IngressReadyTailCarrier,
       IngressStage4TailCarrier,
       IngressCandidateServeTailCarrier

THEOREM LiveCandidateIngressDependencyHasWellFoundedRank ==
  \A packet \in asyncTransport,
     candidate \in ActiveScheduledCandidates:
    /\ AsyncStrongTypeInvariant
    /\ candidate.node = packet.item.envelope.recipient
    /\ LiveResponsiveProtectedCandidateOwned(candidate)
    => CandidateIngressDependencyRank(packet, candidate)
         \in IngressBoundaryDependencyCarrier
BY AsyncStrongTypeProjectsAsyncType,
   ScheduledCandidateServiceRankInCarrier,
   IngressBoundaryDependencyRankInCarrier, Isa
   DEF CandidateIngressDependencyRank,
       LiveResponsiveProtectedCandidateOwned,
       CandidateScheduled, ActiveScheduledCandidates,
       OwnedServiceRankCarrier

THEOREM LiveServeIngressDependencyHasWellFoundedRank ==
  \A packet \in asyncTransport, node \in ValidatorIds,
     job \in ActiveIoJobs:
    /\ AsyncStrongTypeInvariant
    /\ node = packet.item.envelope.recipient
    /\ LiveResponsiveProtectedServeJobOwned(node, job)
    => ServeIngressDependencyRank(packet, node, job)
         \in IngressBoundaryDependencyCarrier
BY LiveResponsiveServeOwnerIsCanonical,
   ResponsiveProtectedServeJobPositionIsNatural,
   IngressBoundaryDependencyRankInCarrier, Isa
   DEF ServeIngressDependencyRank, ServeJobRank,
       OwnedServiceRankCarrier

(***************************************************************************
Two exact preparatory edges.

Removing a non-overdue lane shadow strictly lowers the first dependency
component but cannot witness overdue-packet ownership exit.  The serialized
idle Runtime reset strictly lowers ResetAwareIngressReachRank while raising
the currently declared RuntimeReachRank from zero.  Thus both are genuine
well-founded dependency progress and neither is a current productive effect
merely by virtue of that progress.
***************************************************************************)

NonOverdueShadowAtLaneHead(packet) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
      head == OldestDueSourcePacket(recipient, source)
  IN /\ packet \in OverdueResponsivePackets
     /\ head \notin OverdueResponsivePackets

AdmitNonOverdueShadowFor(packet) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
  IN \/ PostGstAdmitHiddenPacket(recipient, source)
     \/ PostGstAdmitHistoricalRecoveryPacket(recipient, source)

IdleSerializedRuntimeReset(node) ==
  /\ PostGstRunNode(node)
  /\ SerializedRuntimeStep(node)
  /\ IdleRuntimeStep(node)

IdleRuntimeResetReady(node) ==
  /\ asyncRunnerPhase[node] = "Runtime"
  /\ ~DeferredWorkServiceable(node)
  /\ ~DeferredTagExecutable(node)
  /\ ~TimeoutDue(node)
  /\ ~RetransmitDue(node)
  /\ ~NodeQueueNonempty(node)

THEOREM NonOverdueShadowAdmissionRemovesExactShadow ==
  \A packet \in OverdueResponsivePackets:
    /\ AsyncStrongTypeInvariant
    /\ NonOverdueShadowAtLaneHead(packet)
    /\ AdmitNonOverdueShadowFor(packet)
    => LET recipient == packet.item.envelope.recipient
           source == packet.item.source
           head == OldestDueSourcePacket(recipient, source)
       IN /\ packet \in asyncTransport'
          /\ OlderDueNonOverdueLaneShadows(packet)'
               = OlderDueNonOverdueLaneShadows(packet) \ {head}
          /\ OlderDueNonOverdueShadowDebt(packet)' + 1
               = OlderDueNonOverdueShadowDebt(packet)
BY OldestDueSourcePacketFacts,
   StrongTypeHasFiniteOlderNonOverdueShadows,
   FS_RemoveElement, Isa
   DEF NonOverdueShadowAtLaneHead,
       AdmitNonOverdueShadowFor,
       OlderDueNonOverdueLaneShadows,
       OlderDueNonOverdueShadowDebt,
       PostGstAdmitHiddenPacket,
       PostGstAdmitHistoricalRecoveryPacket,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DueSourcePackets,
       OverdueResponsivePackets

THEOREM NonOverdueShadowAdmissionIsNotOverdueOwnershipExit ==
  \A packet \in OverdueResponsivePackets:
    /\ AsyncStrongTypeInvariant
    /\ NonOverdueShadowAtLaneHead(packet)
    /\ AdmitNonOverdueShadowFor(packet)
    => ~PostGstOverduePacketOwnershipExits
BY OldestDueSourcePacketFacts, Isa
   DEF NonOverdueShadowAtLaneHead,
       AdmitNonOverdueShadowFor,
       PostGstOverduePacketOwnershipExits,
       PostGstAdmitHiddenPacket,
       PostGstAdmitHistoricalRecoveryPacket,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       OverdueResponsivePackets, AsyncPacketOwnsClockDeadline,
       DueSourcePackets

THEOREM NonOverdueShadowAdmissionDecreasesDependencyRank ==
  \A packet \in OverdueResponsivePackets,
     node \in ValidatorIds,
     serviceRank \in OwnedServiceRankCarrier,
     serveRank \in OwnedServiceRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ node = packet.item.envelope.recipient
    /\ NonOverdueShadowAtLaneHead(packet)
    /\ AdmitNonOverdueShadowFor(packet)
    => <<IngressBoundaryDependencyRank(
            packet, node, serviceRank, serveRank)',
          IngressBoundaryDependencyRank(
            packet, node, serviceRank, serveRank)>>
         \in IngressBoundaryDependencyOrdering
BY NonOverdueShadowAdmissionRemovesExactShadow,
   IngressBoundaryDependencyRankInCarrier,
   FS_CardinalityType, Isa
   DEF IngressBoundaryDependencyRank,
       IngressBoundaryDependencyOrdering,
       IngressCapacityTailOrdering,
       LexPairOrdering, OpToRel

THEOREM IdleRuntimeResetDecreasesOnlyResetAwareReach ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ IdleSerializedRuntimeReset(node)
    => /\ ResetAwareIngressReachRank(node)'
              < ResetAwareIngressReachRank(node)
       /\ RuntimeReachRank(node)'
              > RuntimeReachRank(node)
BY SMT
   DEF IdleSerializedRuntimeReset,
       PostGstRunNode, RunNode, RunNodeWork,
       SerializedRuntimeStep, IdleRuntimeStep,
       RuntimeStep, ResetAwareIngressReachRank,
       RuntimeReachRank

THEOREM IdleRuntimeResetDecreasesDependencyRank ==
  \A packet \in asyncTransport,
     node \in ValidatorIds,
     serviceRank \in OwnedServiceRankCarrier,
     serveRank \in OwnedServiceRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ node = packet.item.envelope.recipient
    /\ IdleSerializedRuntimeReset(node)
    => <<IngressBoundaryDependencyRank(
            packet, node, serviceRank, serveRank)',
          IngressBoundaryDependencyRank(
            packet, node, serviceRank, serveRank)>>
         \in IngressBoundaryDependencyOrdering
BY IdleRuntimeResetDecreasesOnlyResetAwareReach,
   IngressBoundaryDependencyRankInCarrier, Isa
   DEF IdleSerializedRuntimeReset,
       PostGstRunNode, RunNode, RunNodeWork,
       SerializedRuntimeStep, RuntimeStep, IdleRuntimeStep,
       IngressBoundaryDependencyRank,
       IngressBoundaryDependencyOrdering,
       IngressCapacityTailOrdering,
       IngressTimeoutTailOrdering,
       IngressCompletionTailOrdering,
       IngressTransportTailOrdering,
       IngressResetTailOrdering,
       LexPairOrdering, OpToRel

THEOREM BlockedIngressAdvanceReversesResetAwareReach ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ asyncRunnerPhase[node] = "Ingress"
    /\ ~(asyncRunnerBudget[node] > 0
           /\ asyncIngressReady[node] # <<>>
           /\ DrainableIngressIndices(node) # {})
    /\ IngressDrainStep(node)
    => /\ RuntimeReachRank(node)'
              < RuntimeReachRank(node)
       /\ ResetAwareIngressReachRank(node)'
              > ResetAwareIngressReachRank(node)
BY SMT
   DEF IngressDrainStep, RuntimeReachRank,
       ResetAwareIngressReachRank

(***************************************************************************
Concrete semantic gap kernel.

This is intentionally a state/action witness rather than an unsupported
reachability assertion.  It isolates the reachable shape used by the packet
trace: an undecided current voter is at Runtime, all runtime priorities and
all admitted candidate/Serve owners are empty, service is not due, and an
older non-overdue packet hides an overdue response.  The only runner action
is the idle reset.  It is enabled and lowers the dependency rank, but every
currently declared productive effect is false.
***************************************************************************)

IdleRuntimeProductGapState(node) ==
  /\ AsyncStrongTypeInvariant
  /\ HeightProductivityResetBoundary
  /\ PostGstNodeServiceBlockers = {}
  /\ PostGstIoServiceBlockers = {}
  /\ OverdueResponsivePackets # {}
  /\ AllOverduePacketLaneHeadsBlocked
  /\ ~ImmediateProtectedRankFairActionReady
  /\ node \in AsyncCurrentResponsiveVoters
  /\ ~NodeHasDecision(node)
  /\ ~NodeHasApplication(node)
  /\ node \in up
  /\ ~ResponsiveReplayQuarantined(node)
  /\ IdleRuntimeResetReady(node)
  /\ asyncNodeServiceDeadlines[node] > asyncNow
  /\ asyncNodeServiceDeadlines[node]
       <= asyncNow + AsyncDeliveryBound
  /\ ActiveScheduledCandidates = {}
  /\ ActiveIoJobs = {}

THEOREM IdleRuntimeProductGapEnablesExactReset ==
  \A node \in ValidatorIds:
    IdleRuntimeProductGapState(node)
      => ENABLED IdleSerializedRuntimeReset(node)
BY ExpandENABLED, Isa
   DEF IdleRuntimeProductGapState,
       IdleRuntimeResetReady,
       IdleSerializedRuntimeReset,
       HeightProductivityResetBoundary,
       PostGstRunNode, RunNode, RunNodeWork,
       SerializedRuntimeStep, RuntimeStep,
       IdleRuntimeStep, AsyncNonCrashOuterFrame,
       AsyncCoreOuterFrame, AsyncAllVars,
       AsyncSchedulerVars, AsyncRecoveryVars,
       AsyncIoVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars, LeaveCausalQueues, vars

THEOREM IdleRuntimeProductGapResetIsNotProductive ==
  \A node \in ValidatorIds:
    /\ IdleRuntimeProductGapState(node)
    /\ IdleSerializedRuntimeReset(node)
    => ~PostGstProductiveEffect
BY Isa
   DEF IdleRuntimeProductGapState,
       IdleSerializedRuntimeReset,
       PostGstProductiveEffect,
       HeightProtocolEvidenceGrows, SetGains,
       PostGstDeadlineDebtDecreases, DeadlineDistance,
       PostGstNodeIoBlockerDebtDecreases,
       PostGstNodeIoBlockerDebt,
       PostGstOverduePacketOwnershipExits,
       PostGstRuntimeReachDecreases,
       LiveProtectedServiceRankDecreaseStep,
       LiveProtectedServeRankDecreaseStep,
       ActiveScheduledCandidates, ActiveIoJobs,
       PostGstRunNode, RunNode, RunNodeWork,
       SerializedRuntimeStep, RuntimeStep,
       IdleRuntimeStep, RuntimeReachRank,
       PostGstNodeServiceBlockers,
       PostGstIoServiceBlockers,
       PostGstServiceNodes

THEOREM IdleRuntimeProductGapRefutesImmediateResetPromotion ==
  \A node \in ValidatorIds:
    IdleRuntimeProductGapState(node)
      => ENABLED (
           IdleSerializedRuntimeReset(node)
             /\ ~PostGstProductiveEffect)
BY IdleRuntimeProductGapEnablesExactReset,
   IdleRuntimeProductGapResetIsNotProductive,
   ENABLEDaxioms

(***************************************************************************
Exact certified-response corridor.

Response authentication is over the signed envelope projection, not the relay
via.  Every CertifiedResponse therefore uses the aggregate untrusted ingress
resource lane even when the same signed envelope arrives through a different
outer transport source.  Its logical class is CertifiedResponse, distinct
from a Chunk's TransportCompletion class, while both classes spend the same
finite physical completion owner.

The recipient-local singleton response claim is the separate logical owner.
Fresh admission requires an authenticated exact request in Ready state and
atomically acquires the signed-envelope projection for that recipient; claims
at other recipients are independent.  An exact retransmission coalesces by
that projection across relay vias.  While any certified request remains live,
generic aggregate-lane TransportCompletion traffic is policy-rejected and
cannot refill the physical owner.  A generic physical owner which predates
the request can still delay the response, but its debt is finite and remains
part of the temporal residual below.
***************************************************************************)

OutstandingRequestCertifiedResponseAuthorized(item) ==
  /\ item.kind = "CertifiedResponse"
  /\ CertifiedResponseAuthenticatedOccurrence(item)
  /\ item.envelope.archiveServer \in AsyncArchiveServerIds
  /\ MatchingCertifiedRequests(item) # {}
  /\ \E request \in MatchingCertifiedRequests(item):
       FrozenCertifiedResponseBinding(item, request)

GenericCompletionOwnsCertifiedResponsePhysicalSlot(response, completion) ==
  /\ response.kind = "CertifiedResponse"
  /\ IngressAdmissionClass(completion) = "TransportCompletion"
  /\ response.envelope.recipient = completion.envelope.recipient
  /\ completion \in SequenceSet(
       IngressLane(
         response.envelope.recipient,
         IngressResourceSource(response)))

AuthenticatedAggregateCertifiedResponsePacket(packet) ==
  /\ packet.item.kind = "CertifiedResponse"
  /\ packet.item.source = AsyncUntrustedSource
  /\ IngressItemHasAuthenticatedHistory(packet.item)
  /\ packet.item.envelope.recipient \in AsyncTimedServiceNodes
  /\ packet.deadline <= asyncNow

THEOREM CertifiedResponseAuthorizationUsesExactRequestAuthority ==
  \A item:
    CertifiedResponseAuthorized(item)
      <=> OutstandingRequestCertifiedResponseAuthorized(item)
BY Isa
   DEF CertifiedResponseAuthorized,
       OutstandingRequestCertifiedResponseAuthorized

THEOREM CertifiedResponseAuthenticationAndAuthorizationIgnoreRelayVia ==
  \A left, right:
    /\ left.kind = "CertifiedResponse"
    /\ right.kind = "CertifiedResponse"
    /\ left.envelope = right.envelope
    => /\ (CertifiedResponseAuthenticatedOccurrence(left)
              <=> CertifiedResponseAuthenticatedOccurrence(right))
       /\ (CertifiedResponseAuthorized(left)
              <=> CertifiedResponseAuthorized(right))
       /\ IngressResourceSource(left) = AsyncUntrustedSource
       /\ IngressResourceSource(right) = AsyncUntrustedSource
       /\ IngressCoalescingIdentity(left)
            = IngressCoalescingIdentity(right)
BY Isa
   DEF CertifiedResponseAuthenticatedOccurrence,
       CertifiedResponseAuthorized, MatchingCertifiedRequests,
       IngressResourceSource, IngressCoalescingIdentity,
       AsyncCertifiedResponseAuthProjection

THEOREM AuthenticatedAggregateCertifiedResponseIsTimed ==
  \A packet \in asyncTransport:
    AuthenticatedAggregateCertifiedResponsePacket(packet)
      => packet \in OverdueResponsivePackets
BY DEF AuthenticatedAggregateCertifiedResponsePacket,
       OverdueResponsivePackets, AsyncPacketOwnsClockDeadline,
       AsyncServeTransportAdmissionGateAllows,
       AsyncServeRequestAuthorized

THEOREM ChunkAndCertifiedResponseHaveDistinctLogicalClassesAndSharedPhysicalOwner ==
  \A response, chunk:
    /\ response.kind = "CertifiedResponse"
    /\ chunk.kind = "Chunk"
    => /\ IngressAdmissionClass(response)
              = "CertifiedResponse"
       /\ IngressAdmissionClass(chunk)
              = "TransportCompletion"
       /\ IngressUsesPhysicalCompletionOwner(response)
       /\ IngressUsesPhysicalCompletionOwner(chunk)
BY DEF IngressAdmissionClass,
       IngressTransportCompletionKinds,
       IngressUsesPhysicalCompletionOwner

THEOREM PreExistingGenericCompletionBlocksCertifiedResponsePhysicalGate ==
  \A response, completion:
    GenericCompletionOwnsCertifiedResponsePhysicalSlot(
      response, completion)
      => ~AsyncTransportCompletionOwnerGateAllows(response)
BY Isa
   DEF GenericCompletionOwnsCertifiedResponsePhysicalSlot,
       AsyncTransportCompletionOwnerGateAllows,
       IngressLaneHasTransportCompletionIn,
       IngressUsesPhysicalCompletionOwner,
       IngressResourceSource

THEOREM PreExistingGenericCompletionIsFiniteCertifiedResponseDebt ==
  \A response, completion:
    /\ AsyncStrongTypeInvariant
    /\ AsyncItemTyped(response)
    /\ GenericCompletionOwnsCertifiedResponsePhysicalSlot(
         response, completion)
    => /\ TransportCompletionOwnerDebt(response) \in Nat
       /\ TransportCompletionOwnerDebt(response) > 0
       /\ TransportCompletionOwnerDebt(response)
            <= AsyncIngressCapacity
BY IngressGateOwnerDebtsAreFiniteNaturals,
   PreExistingGenericCompletionBlocksCertifiedResponsePhysicalGate,
   TransportCompletionGateHasExactFiniteOwner, SMT

THEOREM FreshCertifiedResponseAdmissionAcquiresRouteNeutralClaim ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    LET packet == OldestDueSourcePacket(recipient, source)
        item == packet.item
    IN /\ DueSourcePackets(recipient, source) # {}
       /\ item.kind = "CertifiedResponse"
       /\ AdmitHiddenPacket(recipient, source)
       => /\ CertifiedResponseAuthorized(item)
          /\ CertifiedResponseAuthorityReady(
               item.envelope.requestHash)
          /\ CertifiedResponseRecipientClaimAvailable(item)
          /\ IngressResourceSource(item) = AsyncUntrustedSource
          /\ CertifiedResponseClaimsAt(recipient)' =
               {AsyncCertifiedResponseAuthProjection(item)}
BY Isa
   DEF AdmitHiddenPacket, CanAdmitIngressItem,
       CertifiedResponseFreshClaimGateAllows,
       CertifiedResponseRecipientClaimAvailable,
       CertifiedResponseClaimsAt,
       IngressResourceSource

THEOREM ExactCertifiedResponseCoalescingPreservesClaim ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    LET packet == OldestDueSourcePacket(recipient, source)
        item == packet.item
    IN /\ DueSourcePackets(recipient, source) # {}
       /\ item.kind = "CertifiedResponse"
       /\ CoalesceHiddenPacket(recipient, source)
       => /\ CertifiedResponseClaimMatches(item)
          /\ UNCHANGED asyncCertifiedResponseClaim
BY Isa DEF CoalesceHiddenPacket

THEOREM LiveCertifiedRequestRejectsGenericUntrustedCompletionRefill ==
  \A item:
    /\ ActiveCertifiedRequestHashesAt(item.envelope.recipient) # {}
    /\ IngressAdmissionClass(item) = "TransportCompletion"
    /\ IngressResourceSource(item) = AsyncUntrustedSource
    => /\ ~AsyncUntrustedGenericCompletionGateAllows(item)
       /\ ~CanAdmitIngressItem(item)
       /\ UntrustedGenericCompletionPacketPolicyRejected(item)
       /\ IngressPacketPolicyRejected(item)
BY Isa
   DEF AsyncUntrustedGenericCompletionGateAllows,
       CanAdmitIngressItem,
       UntrustedGenericCompletionPacketPolicyRejected,
       IngressPacketPolicyRejected

THEOREM PolicyRejectedIngressAttemptCannotRefillPhysicalOwner ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    DropPolicyRejectedHiddenPacket(recipient, source)
      => /\ UNCHANGED asyncIngressLanes
         /\ UNCHANGED asyncCertifiedResponseClaim
BY DEF DropPolicyRejectedHiddenPacket

(***************************************************************************
Strict residual.

All weighted service blockers are absent.  The exact clock blocker is
therefore a nonempty overdue packet set, every such packet's lane is either
head-of-line shadowed or blocked by finite capacity, timeout-byte, or shared
physical-completion debt, and no concrete candidate/Serve rank-decrease action
is already enabled.  A recipient-local response-claim conflict remains an
explicit finite ingress owner; generic aggregate-lane refill remains a
policy-drop action.
***************************************************************************)

HeightResetIngressOwnershipResidual ==
  /\ HeightProductivityResetBoundary
  /\ PostGstNodeServiceBlockers = {}
  /\ PostGstIoServiceBlockers = {}
  /\ OverdueResponsivePackets # {}
  /\ AllOverduePacketLaneHeadsBlocked
  /\ ~ImmediateProtectedRankFairActionReady

THEOREM HeightResetResidualIsStrictSubboundary ==
  HeightResetIngressOwnershipResidual
    => HeightProductivityResetBoundary
BY DEF HeightResetIngressOwnershipResidual

THEOREM ResetBoundaryHasImmediateProductivityOrIngressResidual ==
  /\ AsyncStrongTypeInvariant
  /\ HeightProductivityResetBoundary
  => \/ ImmediateProductiveFairActionReady
     \/ HeightResetIngressOwnershipResidual
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              HeightProductivityResetBoundary
         PROVE \/ ImmediateProductiveFairActionReady
               \/ HeightResetIngressOwnershipResidual
    <2>1. gst
      BY <1>1 DEF HeightProductivityResetBoundary
    <2>2. CASE ImmediateProductiveFairActionReady
      BY <2>2
    <2>3. CASE ~ImmediateProductiveFairActionReady
      <3>1. PostGstNodeServiceBlockers = {}
        BY <1>1, <2>1, <2>3,
           DueNodeServiceBlockerIsImmediatelyProductive
      <3>2. PostGstIoServiceBlockers = {}
        BY <1>1, <2>1, <2>3,
           DueIoServiceBlockerIsImmediatelyProductive
      <3>3. OverdueResponsivePackets # {}
        BY <1>1, <3>1, <3>2, Isa
           DEF HeightProductivityResetBoundary,
               PostGstClockBlockingOwnerExists,
               PostGstNodeServiceBlockers,
               PostGstIoServiceBlockers,
               PostGstServiceNodes
      <3>4. AllOverduePacketLaneHeadsBlocked
        BY <1>1, <2>1, <2>3, <3>3,
           UnblockedOverdueLaneHeadIsImmediatelyProductive
      <3>5. ~ImmediateProtectedRankFairActionReady
        BY <2>3,
           ImmediateProtectedRankFairActionIsImmediatelyProductive
      <3> QED BY <1>1, <3>1, <3>2, <3>3, <3>4, <3>5
           DEF HeightResetIngressOwnershipResidual
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

(***************************************************************************
Fair Runtime-reset closure.

The ingress dependency product above explains the concrete packet owners, but
the temporal reset-boundary exit has a smaller scheduler proof.  In any state
without an immediately productive action, every still-undecided responsive
voter must be at Runtime: Local and Ingress are already covered by the strict
`RuntimeReachRank` certificate.  The exact weakly fair `PostGstRunNode` action
then returns that voter to Local.  Its successor either records a Decision or
immediately exposes the Local certificate.

The per-voter result is accumulated over the frozen one-height roster.  Durable
Decision receipts make each finite prefix stable; no fairness over an
existential action union is assumed.
***************************************************************************)

HeightResetNodeExit(node) ==
  \/ ImmediateProductiveFairActionReady
  \/ NodeHasDecision(node)

HeightResetNodePending(initialContext, node) ==
  /\ AsyncStrongTypeInvariant
  /\ OneHeightFrameAt(initialContext)
  /\ gst
  /\ node \in AsyncVotersAt(initialContext)
  /\ ~HeightResetNodeExit(node)

HeightResetDecisionPrefixAt(initialContext, limit) ==
  \A node \in AsyncVotersAt(initialContext) \cap (0..limit):
    NodeHasDecision(node)

THEOREM UndecidedCurrentVoterWithoutImmediateProductivityIsAtRuntime ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ ~NodeHasDecision(node)
    /\ ~ImmediateProductiveFairActionReady
    => asyncRunnerPhase[node] = "Runtime"
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncStrongTypeInvariant,
                gst,
                ~NodeHasDecision(node),
                ~ImmediateProductiveFairActionReady
         PROVE asyncRunnerPhase[node] = "Runtime"
    <2>1. /\ AsyncTypeInvariant
           /\ node \in ValidatorIds
           /\ asyncRunnerPhase[node]
                \in {"Local", "Ingress", "Runtime"}
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         AsyncCurrentResponsiveVotersAreValidators, Isa
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant
    <2>2. ~(
             asyncRunnerPhase[node] \in {"Local", "Ingress"})
      BY <1>1,
         GstUndecidedLocalOrIngressRunnerIsImmediatelyProductive
    <2> QED BY <2>1, <2>2, Isa
  <1> QED BY <1>1

THEOREM RuntimePostGstRunNodeIsNonstuttering ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncTypeInvariant
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ PostGstRunNode(node)
    => <<PostGstRunNode(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncTypeInvariant,
                asyncRunnerPhase[node] = "Runtime",
                PostGstRunNode(node)
         PROVE <<PostGstRunNode(node)>>_AsyncAllVars
    <2>1. /\ node \in ValidatorIds
           /\ SerializedRuntimeStep(node)
      BY <1>1, AsyncCurrentResponsiveVotersAreValidators, Isa
         DEF PostGstRunNode, RunNode, RunNodeWork
    <2>2. asyncRunnerPhase'[node] = "Local"
      BY <1>1, <2>1, SerializedRuntimeReturnsToLocalWithBudget
    <2>3. asyncRunnerPhase'[node] # asyncRunnerPhase[node]
      BY <1>1, <2>2
    <2> QED BY <1>1, <2>3, Isa
         DEF AsyncAllVars, AsyncSchedulerVars
  <1> QED BY <1>1

THEOREM RuntimePostGstRunNodeExposesProductivityOrDecision ==
  \A node \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ gst
    /\ ~NodeHasDecision(node)
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ PostGstRunNode(node)
    => HeightResetNodeExit(node)'
PROOF
  <1>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                AsyncStrongTypeInvariant,
                AsyncStrongTypeInvariant',
                gst,
                ~NodeHasDecision(node),
                asyncRunnerPhase[node] = "Runtime",
                PostGstRunNode(node)
         PROVE HeightResetNodeExit(node)'
    <2>1. /\ AsyncTypeInvariant
           /\ node \in ValidatorIds
           /\ SerializedRuntimeStep(node)
      BY <1>1, AsyncStrongTypeProjectsAsyncType,
         AsyncCurrentResponsiveVotersAreValidators, Isa
         DEF PostGstRunNode, RunNode, RunNodeWork
    <2>2. /\ asyncRunnerPhase'[node] = "Local"
           /\ gst'
           /\ AsyncCurrentResponsiveVoters'
                = AsyncCurrentResponsiveVoters
      BY <1>1, <2>1,
         SerializedRuntimeReturnsToLocalWithBudget, Isa
         DEF PostGstRunNode, RunNode, RunNodeWork,
             AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch
    <2>3. CASE NodeHasDecision(node)'
      BY <2>3 DEF HeightResetNodeExit
    <2>4. CASE ~NodeHasDecision(node)'
      <3>1. ImmediateProductiveFairActionReady'
        BY <1>1, <2>2, <2>4,
           GstUndecidedLocalOrIngressRunnerIsImmediatelyProductive
      <3> QED BY <3>1 DEF HeightResetNodeExit
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM HeightResetNodePendingEnablesFairRuntimeReset ==
  \A initialContext:
    \A node \in AsyncVotersAt(initialContext):
      HeightResetNodePending(initialContext, node)
        => ENABLED
             <<PostGstRunNode(node)>>_AsyncAllVars
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node \in AsyncVotersAt(initialContext),
                HeightResetNodePending(initialContext, node)
         PROVE ENABLED
                 <<PostGstRunNode(node)>>_AsyncAllVars
    <2>1. /\ node \in AsyncCurrentResponsiveVoters
           /\ ~NodeHasDecision(node)
           /\ ~ImmediateProductiveFairActionReady
      BY <1>1 DEF HeightResetNodePending, HeightResetNodeExit,
                    OneHeightFrameAt
    <2>2. asyncRunnerPhase[node] = "Runtime"
      BY <1>1, <2>1,
         UndecidedCurrentVoterWithoutImmediateProductivityIsAtRuntime
         DEF HeightResetNodePending
    <2>3. ENABLED PostGstRunNode(node)
      BY <1>1, <2>1, GstUndecidedResponsiveRunNodeIsEnabled
         DEF HeightResetNodePending
    <2>4. PostGstRunNode(node)
             => <<PostGstRunNode(node)>>_AsyncAllVars
      BY <1>1, <2>1, <2>2,
         AsyncStrongTypeProjectsAsyncType,
         RuntimePostGstRunNodeIsNonstuttering
         DEF HeightResetNodePending
    <2> QED BY <2>3, <2>4, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM HeightResetNodePendingRuntimeResetReachesExit ==
  \A initialContext:
    \A node \in AsyncVotersAt(initialContext):
      /\ HeightResetNodePending(initialContext, node)
      /\ <<PostGstRunNode(node)>>_AsyncAllVars
      => HeightResetNodeExit(node)'
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node \in AsyncVotersAt(initialContext),
                HeightResetNodePending(initialContext, node),
                <<PostGstRunNode(node)>>_AsyncAllVars
         PROVE HeightResetNodeExit(node)'
    <2>1. /\ node \in AsyncCurrentResponsiveVoters
           /\ ~NodeHasDecision(node)
           /\ ~ImmediateProductiveFairActionReady
      BY <1>1 DEF HeightResetNodePending, HeightResetNodeExit,
                    OneHeightFrameAt
    <2>2. asyncRunnerPhase[node] = "Runtime"
      BY <1>1, <2>1,
         UndecidedCurrentVoterWithoutImmediateProductivityIsAtRuntime
         DEF HeightResetNodePending
    <2>3. PostGstRunNode(node)
      BY <1>1
    <2>4. AsyncStrongTypeInvariant'
      BY <1>1, AsyncBracketNextPreservesStrongTypeInvariant
         DEF HeightResetNodePending
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
         RuntimePostGstRunNodeExposesProductivityOrDecision
         DEF HeightResetNodePending
  <1> QED BY <1>1

THEOREM HeightResetNodePendingUnlessExit ==
  \A initialContext:
    \A node \in AsyncVotersAt(initialContext):
      /\ HeightResetNodePending(initialContext, node)
      /\ [AsyncNext]_AsyncAllVars
      => \/ HeightResetNodePending(initialContext, node)'
         \/ HeightResetNodeExit(node)'
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node \in AsyncVotersAt(initialContext),
                HeightResetNodePending(initialContext, node),
                [AsyncNext]_AsyncAllVars
         PROVE \/ HeightResetNodePending(initialContext, node)'
               \/ HeightResetNodeExit(node)'
    <2>1. CASE HeightResetNodeExit(node)'
      BY <2>1
    <2>2. CASE ~HeightResetNodeExit(node)'
      <3>1. /\ AsyncStrongTypeInvariant'
             /\ OneHeightFrameAt(initialContext)'
             /\ gst'
        BY <1>1, AsyncBracketNextPreservesStrongTypeInvariant,
           AsyncStepPreservesOneHeightFrame,
           GstAsyncStepIsMonotone
           DEF HeightResetNodePending
      <3>2. HeightResetNodePending(initialContext, node)'
        BY <1>1, <2>2, <3>1 DEF HeightResetNodePending
      <3> QED BY <3>2
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM FairRuntimeResetExposesProductivityOrDecision ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => \A node \in AsyncVotersAt(initialContext):
           HeightResetNodePending(initialContext, node)
             ~> HeightResetNodeExit(node)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncLiveSpecAt(initialContext)
                 => \A node \in AsyncVotersAt(initialContext):
                      HeightResetNodePending(initialContext, node)
                        ~> HeightResetNodeExit(node)
    <2>1. ASSUME NEW node \in AsyncVotersAt(initialContext)
           PROVE AsyncLiveSpecAt(initialContext)
                   => (HeightResetNodePending(initialContext, node)
                         ~> HeightResetNodeExit(node))
      <3>1. HeightResetNodePending(initialContext, node)
               /\ ~HeightResetNodeExit(node)
              => ENABLED
                   <<PostGstRunNode(node)>>_AsyncAllVars
        BY <2>1, HeightResetNodePendingEnablesFairRuntimeReset
      <3>2. HeightResetNodePending(initialContext, node)
               /\ ~HeightResetNodeExit(node)
               /\ <<PostGstRunNode(node)>>_AsyncAllVars
              => HeightResetNodeExit(node)'
        BY <2>1, HeightResetNodePendingRuntimeResetReachesExit
      <3>3. HeightResetNodePending(initialContext, node)
               /\ [AsyncNext]_AsyncAllVars
              => HeightResetNodePending(initialContext, node)'
                   \/ HeightResetNodeExit(node)'
        BY <2>1, HeightResetNodePendingUnlessExit
      <3>4. AsyncLiveSpecAt(initialContext)
               => WF_AsyncAllVars(PostGstRunNode(node))
        BY <2>1 DEF AsyncLiveSpecAt, AsyncSpecAt,
                       AsyncFairnessAt
      <3> QED BY <3>1, <3>2, <3>3, <3>4, PTL
           DEF AsyncLiveSpecAt, AsyncSpecAt
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM FairRuntimeResetCoversFixedVoter ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => \A node \in AsyncVotersAt(initialContext):
           (gst /\ ~ImmediateProductiveFairActionReady)
             ~> HeightResetNodeExit(node)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncLiveSpecAt(initialContext)
         PROVE \A node \in AsyncVotersAt(initialContext):
                 (gst /\ ~ImmediateProductiveFairActionReady)
                   ~> HeightResetNodeExit(node)
    <2>0. AsyncSpecAt(initialContext)
      BY <1>1, AsyncLiveSpecProjectsAsyncSpec
    <2>1. /\ []AsyncStrongTypeInvariant
           /\ []OneHeightFrameAt(initialContext)
      BY <2>0, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysKeepsOneHeightFrame
    <2>2. ASSUME NEW node \in AsyncVotersAt(initialContext)
           PROVE (gst /\ ~ImmediateProductiveFairActionReady)
                   ~> HeightResetNodeExit(node)
      <3>1. HeightResetNodePending(initialContext, node)
               ~> HeightResetNodeExit(node)
        BY <1>1, <2>2, FairRuntimeResetExposesProductivityOrDecision
      <3>2. [](gst /\ ~ImmediateProductiveFairActionReady
                 => \/ HeightResetNodePending(initialContext, node)
                    \/ HeightResetNodeExit(node))
        BY <2>1, <2>2, PTL
           DEF HeightResetNodePending, HeightResetNodeExit
      <3> QED BY <3>1, <3>2, PTL
    <2> QED BY <2>2
  <1> QED BY <1>1

THEOREM CoreBracketStepPreservesNodeDecision ==
  \A node:
    NodeHasDecision(node)
      /\ [Next]_vars
      => NodeHasDecision(node)'
PROOF
  <1>1. ASSUME NEW node,
                NodeHasDecision(node),
                [Next]_vars
         PROVE NodeHasDecision(node)'
    <2>1. CASE UNCHANGED vars
      BY <1>1, <2>1, Isa DEF NodeHasDecision, vars
    <2>2. CASE Next
      <3>1. UNCHANGED context
        BY <2>2, CoreNextLeavesContext
      <3>2. \/ UNCHANGED <<decisions, applied>>
             \/ (\E request \in pendingDecision:
                   PersistDecision(request))
             \/ (\E owner \in ValidatorIds,
                        qc \in DecisionQcValues:
                   ApplyDecision(owner, qc))
        BY <2>2, NextDurableReceiptActionClassification
      <3>3. CASE UNCHANGED <<decisions, applied>>
        BY <1>1, <3>1, <3>3, Isa DEF NodeHasDecision
      <3>4. CASE \E request \in pendingDecision:
                    PersistDecision(request)
        <4>1. PICK request \in pendingDecision:
                 PersistDecision(request)
          BY <3>4
        <4> QED BY <1>1, <3>1, <4>1, Isa
             DEF PersistDecision, NodeHasDecision
      <3>5. CASE \E owner \in ValidatorIds,
                        qc \in DecisionQcValues:
                    ApplyDecision(owner, qc)
        <4>1. PICK owner \in ValidatorIds,
                     qc \in DecisionQcValues:
                 ApplyDecision(owner, qc)
          BY <3>5
        <4> QED BY <1>1, <3>1, <4>1, Isa
             DEF ApplyDecision, NodeHasDecision
      <3> QED BY <3>2, <3>3, <3>4, <3>5
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncBracketStepPreservesNodeDecision ==
  \A node:
    NodeHasDecision(node)
      /\ [AsyncNext]_AsyncAllVars
      => NodeHasDecision(node)'
PROOF
  <1>1. ASSUME NEW node,
                NodeHasDecision(node),
                [AsyncNext]_AsyncAllVars
         PROVE NodeHasDecision(node)'
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>1, Isa
         DEF NodeHasDecision, AsyncAllVars, AsyncSchedulerVars, vars
    <2>2. CASE AsyncNext
      <3>1. [Next]_vars
        BY <2>2, AsyncStepRefinementObligation
      <3> QED BY <1>1, <3>1,
           CoreBracketStepPreservesNodeDecision
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM HeightResetDecisionPrefixAtIsStable ==
  \A initialContext:
    \A limit \in Nat:
      HeightResetDecisionPrefixAt(initialContext, limit)
        /\ [AsyncNext]_AsyncAllVars
        => HeightResetDecisionPrefixAt(initialContext, limit)'
BY Isa, AsyncBracketStepPreservesNodeDecision
   DEF HeightResetDecisionPrefixAt

THEOREM FrozenContextFullDecisionPrefixImpliesResponsiveDecide ==
  \A initialContext:
    /\ ModelConfiguration
    /\ AsyncFrozenContextAt(initialContext)
    /\ HeightResetDecisionPrefixAt(initialContext, N - 1)
    => ResponsiveNodesDecide
BY FrozenContextFixesResponsiveVoters, Isa
   DEF HeightResetDecisionPrefixAt, ResponsiveNodesDecide,
       AsyncVotersAt, ValidatorIds, ModelConfiguration,
       QuorumConfiguration

THEOREM FairRuntimeResetReachesEveryDecisionPrefix ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => \A limit \in Nat:
           (gst /\ ~ImmediateProductiveFairActionReady)
             ~> (ImmediateProductiveFairActionReady
                   \/ HeightResetDecisionPrefixAt(
                        initialContext, limit))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncLiveSpecAt(initialContext)
         PROVE \A limit \in Nat:
                 (gst /\ ~ImmediateProductiveFairActionReady)
                   ~> (ImmediateProductiveFairActionReady
                         \/ HeightResetDecisionPrefixAt(
                              initialContext, limit))
    <2> DEFINE P(limit) ==
           (gst /\ ~ImmediateProductiveFairActionReady)
             ~> (ImmediateProductiveFairActionReady
                   \/ HeightResetDecisionPrefixAt(
                        initialContext, limit))
    <2>1. P(0)
      <3>1. CASE 0 \in AsyncVotersAt(initialContext)
        <4>1. (gst /\ ~ImmediateProductiveFairActionReady)
                 ~> HeightResetNodeExit(0)
          BY <1>1, <3>1, FairRuntimeResetCoversFixedVoter
        <4> QED BY <4>1, PTL
             DEF P, HeightResetNodeExit,
                 HeightResetDecisionPrefixAt
      <3>2. CASE 0 \notin AsyncVotersAt(initialContext)
        BY <3>2, PTL DEF P, HeightResetDecisionPrefixAt
      <3> QED BY <3>1, <3>2
    <2>2. ASSUME NEW limit \in Nat,
                  P(limit)
           PROVE P(limit + 1)
      <3>1. CASE limit + 1 \in AsyncVotersAt(initialContext)
        <4>1. (gst /\ ~ImmediateProductiveFairActionReady)
                 ~> HeightResetNodeExit(limit + 1)
          BY <1>1, <3>1, FairRuntimeResetCoversFixedVoter
        <4>2. HeightResetDecisionPrefixAt(initialContext, limit)
                 /\ [AsyncNext]_AsyncAllVars
                 => HeightResetDecisionPrefixAt(
                      initialContext, limit)'
          BY <2>2, HeightResetDecisionPrefixAtIsStable
        <4>3. NodeHasDecision(limit + 1)
                 /\ [AsyncNext]_AsyncAllVars
                 => NodeHasDecision(limit + 1)'
          BY AsyncBracketStepPreservesNodeDecision
        <4>4. HeightResetDecisionPrefixAt(
                 initialContext, limit + 1)
                 <=> /\ HeightResetDecisionPrefixAt(
                           initialContext, limit)
                     /\ NodeHasDecision(limit + 1)
          BY <2>2, <3>1, Isa DEF HeightResetDecisionPrefixAt
        <4> QED BY <2>2, <4>1, <4>2, <4>3, <4>4, PTL
             DEF P, HeightResetNodeExit, AsyncLiveSpecAt, AsyncSpecAt
      <3>2. CASE limit + 1 \notin AsyncVotersAt(initialContext)
        <4>1. HeightResetDecisionPrefixAt(initialContext, limit)
                 => HeightResetDecisionPrefixAt(
                      initialContext, limit + 1)
          BY <2>2, <3>2, Isa DEF HeightResetDecisionPrefixAt
        <4> QED BY <2>2, <4>1, PTL DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM FairRuntimeResetReachesProductivityOrAggregateDecision ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => (gst /\ ~ImmediateProductiveFairActionReady)
           ~> (ImmediateProductiveFairActionReady
                 \/ ResponsiveNodesDecide)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncLiveSpecAt(initialContext)
         PROVE (gst /\ ~ImmediateProductiveFairActionReady)
                 ~> (ImmediateProductiveFairActionReady
                       \/ ResponsiveNodesDecide)
    <2>1. ModelConfiguration
      BY <1>1, AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysStrongTypeInvariant, PTL
         DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
             Safety, TypeInvariant
    <2>2. N - 1 \in Nat
      BY <2>1, ModelConfigurationMakesLastValidatorNatural
    <2>3. (gst /\ ~ImmediateProductiveFairActionReady)
             ~> (ImmediateProductiveFairActionReady
                   \/ HeightResetDecisionPrefixAt(
                        initialContext, N - 1))
      BY <1>1, <2>2, FairRuntimeResetReachesEveryDecisionPrefix
    <2>4. []AsyncFrozenContextAt(initialContext)
      BY <1>1, AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysKeepsFrozenContext
    <2>5. [](HeightResetDecisionPrefixAt(initialContext, N - 1)
               => ResponsiveNodesDecide)
      BY <2>1, <2>4,
         FrozenContextFullDecisionPrefixImpliesResponsiveDecide, PTL
    <2> QED BY <2>3, <2>5, PTL
  <1> QED BY <1>1

(***************************************************************************
Exact temporal reduction.

The residual may be left because a concrete productive action becomes ready
or because all responsive voters decide while its finite owners are being
served.  The new residual property is a definition, not an assumption.  The
two directions show that proving this convergence is necessary and sufficient
for the temporal reset-boundary release obligation on every live initial
context.
***************************************************************************)

HeightResetIngressOwnershipResidualProperty(specification) ==
  specification
    => (AsyncStrongTypeInvariant
          /\ HeightResetIngressOwnershipResidual)
         ~> (ImmediateProductiveFairActionReady
               \/ ResponsiveNodesDecide)

THEOREM HeightResetIngressOwnershipResidualConvergence ==
  \A initialContext:
    HeightResetIngressOwnershipResidualProperty(
      AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE HeightResetIngressOwnershipResidualProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE (AsyncStrongTypeInvariant
                    /\ HeightResetIngressOwnershipResidual)
                    ~> (ImmediateProductiveFairActionReady
                          \/ ResponsiveNodesDecide)
      <3>1. (gst /\ ~ImmediateProductiveFairActionReady)
               ~> (ImmediateProductiveFairActionReady
                     \/ ResponsiveNodesDecide)
        BY <2>1,
           FairRuntimeResetReachesProductivityOrAggregateDecision
      <3>2. [](AsyncStrongTypeInvariant
                 /\ HeightResetIngressOwnershipResidual
                => \/ ImmediateProductiveFairActionReady
                   \/ (gst /\ ~ImmediateProductiveFairActionReady))
        BY PTL DEF HeightResetIngressOwnershipResidual,
                       HeightProductivityResetBoundary
      <3> QED BY <3>1, <3>2, PTL
    <2> QED BY <2>1
         DEF HeightResetIngressOwnershipResidualProperty
  <1> QED BY <1>1

THEOREM IngressResidualCoverageImpliesResetBoundaryCoverage ==
  \A initialContext:
    HeightResetIngressOwnershipResidualProperty(
      AsyncLiveSpecAt(initialContext))
      => HeightProductivityResetBoundaryProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                HeightResetIngressOwnershipResidualProperty(
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
      <3>2. []AsyncStrongTypeInvariant
        BY <3>1, AsyncSpecAlwaysStrongTypeInvariant
      <3>3. (AsyncStrongTypeInvariant
               /\ HeightResetIngressOwnershipResidual)
                ~> (ImmediateProductiveFairActionReady
                      \/ ResponsiveNodesDecide)
        BY <1>1, <2>1
           DEF HeightResetIngressOwnershipResidualProperty
      <3>4. [](AsyncStrongTypeInvariant
                 /\ HeightProductivityResetBoundary
                => \/ ImmediateProductiveFairActionReady
                   \/ HeightResetIngressOwnershipResidual)
        BY ResetBoundaryHasImmediateProductivityOrIngressResidual,
           PTL
      <3> QED BY <3>2, <3>3, <3>4, PTL
    <2> QED BY <2>1
         DEF HeightProductivityResetBoundaryProperty
  <1> QED BY <1>1

THEOREM ResetBoundaryCoverageImpliesIngressResidualCoverage ==
  \A initialContext:
    HeightProductivityResetBoundaryProperty(
      AsyncLiveSpecAt(initialContext))
      => HeightResetIngressOwnershipResidualProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                HeightProductivityResetBoundaryProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE HeightResetIngressOwnershipResidualProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE (AsyncStrongTypeInvariant
                    /\ HeightResetIngressOwnershipResidual)
                    ~> (ImmediateProductiveFairActionReady
                          \/ ResponsiveNodesDecide)
      <3>1. (AsyncStrongTypeInvariant
               /\ HeightProductivityResetBoundary)
                ~> (ImmediateProductiveFairActionReady
                      \/ ResponsiveNodesDecide)
        BY <1>1, <2>1
           DEF HeightProductivityResetBoundaryProperty
      <3>2. [](HeightResetIngressOwnershipResidual
                 => HeightProductivityResetBoundary)
        BY HeightResetResidualIsStrictSubboundary, PTL
      <3> QED BY <3>1, <3>2, PTL
    <2> QED BY <2>1
         DEF HeightResetIngressOwnershipResidualProperty
  <1> QED BY <1>1

THEOREM HeightResetBoundaryExactIngressResidualEquivalence ==
  \A initialContext:
    HeightProductivityResetBoundaryProperty(
      AsyncLiveSpecAt(initialContext))
      <=> HeightResetIngressOwnershipResidualProperty(
            AsyncLiveSpecAt(initialContext))
BY IngressResidualCoverageImpliesResetBoundaryCoverage,
   ResetBoundaryCoverageImpliesIngressResidualCoverage

THEOREM HeightProductivityFrontierExactIngressResidualEquivalence ==
  \A initialContext:
    HeightProductivityFrontierProperty(
      AsyncLiveSpecAt(initialContext))
      <=> HeightResetIngressOwnershipResidualProperty(
            AsyncLiveSpecAt(initialContext))
BY HeightProductivityFrontierExactResidualEquivalence,
   HeightResetBoundaryExactIngressResidualEquivalence

=============================================================================
