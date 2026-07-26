---- MODULE SumeragiV2AsyncHistoricalRecoveryClockActionProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryServiceClosureProofs

(***************************************************************************
Action-local historical discovery-clock edges.

This module proves two concrete parts of the fixed-clock rank boundary:

  * every publication represented by `PacketsForItems` adds only packets
    whose immutable deadline is strictly after the frozen clock and therefore
    preserves both the global due-packet set and every due source prefix; and
  * ingress admission, coalescing, and policy drop remove their exact due lane
    head.  If that head is the selected overdue packet, the selected occurrence
    exits.  If it is a non-overdue shadow in front of the selected packet, the
    already-proved ingress dependency ordering strictly descends.

Removing any due lane head also strictly lowers the earlier global due-packet
debt.  This covers the third structural case in which the selected packet is
behind a different overdue head: that step need not lower the selected
packet's local ingress rank because the outer debt has already decreased.

All results are state/action implications.  There is no temporal property,
fairness union, Decision convergence, or proofless closure premise here.
***************************************************************************)

(***************************************************************************
Generic fixed-clock publication frame.
***************************************************************************)

HistoricalDiscoveryNewTransportPacketsAreFuture(clockValue) ==
  \A packet \in asyncTransport' \ asyncTransport:
    packet.deadline > clockValue

HistoricalDiscoveryFixedClockPublicationFrame(clockValue) ==
  /\ asyncNow = clockValue
  /\ asyncNow' = clockValue
  /\ HistoricalDiscoveryNewTransportPacketsAreFuture(clockValue)
  /\ HistoricalDiscoveryDuePacketsAt(clockValue)' =
       HistoricalDiscoveryDuePacketsAt(clockValue)
  /\ HistoricalDiscoveryDuePacketDebt(clockValue)' =
       HistoricalDiscoveryDuePacketDebt(clockValue)
  /\ \A recipient \in ValidatorIds, source \in AsyncIngressSources:
       DueSourcePackets(recipient, source)' =
         DueSourcePackets(recipient, source)

HistoricalDiscoveryFixedClockPacketUnion(
    items, clockValue) ==
  /\ asyncNow = clockValue
  /\ UNCHANGED asyncNow
  /\ asyncTransport' =
       asyncTransport \cup PacketsForItems(items)

HistoricalDiscoveryFixedClockOptionalPacketUnion(
    items, publish, clockValue) ==
  /\ publish \in BOOLEAN
  /\ asyncNow = clockValue
  /\ UNCHANGED asyncNow
  /\ asyncTransport' =
       asyncTransport
         \cup PacketsForItems(
                IF publish THEN items ELSE {})

THEOREM HistoricalDiscoveryPacketUnionHasFixedClockFrame ==
  \A items:
    \A clockValue \in Nat:
      /\ AsyncStrongTypeInvariant
      /\ HistoricalDiscoveryFixedClockPacketUnion(
           items, clockValue)
      => HistoricalDiscoveryFixedClockPublicationFrame(clockValue)
BY PacketsForItemsHaveStrictlyFutureDeadlines,
   PacketPublicationPreservesCurrentDueSourcePrefix, Isa
   DEF HistoricalDiscoveryFixedClockPacketUnion,
       HistoricalDiscoveryFixedClockPublicationFrame,
       HistoricalDiscoveryNewTransportPacketsAreFuture,
       HistoricalDiscoveryDuePacketsAt,
       HistoricalDiscoveryDuePacketDebt,
       AsyncStrongTypeInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration

THEOREM HistoricalDiscoveryOptionalPacketUnionHasFixedClockFrame ==
  \A items:
    \A publish \in BOOLEAN, clockValue \in Nat:
      /\ AsyncStrongTypeInvariant
      /\ HistoricalDiscoveryFixedClockOptionalPacketUnion(
           items, publish, clockValue)
      => HistoricalDiscoveryFixedClockPublicationFrame(clockValue)
BY HistoricalDiscoveryPacketUnionHasFixedClockFrame, Isa
   DEF HistoricalDiscoveryFixedClockOptionalPacketUnion,
       HistoricalDiscoveryFixedClockPacketUnion,
       HistoricalDiscoveryFixedClockPublicationFrame,
       HistoricalDiscoveryNewTransportPacketsAreFuture,
       HistoricalDiscoveryDuePacketsAt,
       HistoricalDiscoveryDuePacketDebt

(***************************************************************************
Publication helper coverage.

These five helpers are the complete constructor surface for ordinary control,
ephemeral response, certified-body request, and commit-certificate discovery
publication.  The implication form lets a caller supply the concrete outer
action's `UNCHANGED asyncNow` frame without expanding the publisher again.
***************************************************************************)

THEOREM HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame ==
  \A items, controlItems, ephemeralItems:
    \A clockValue \in Nat:
      /\ AsyncStrongTypeInvariant
      /\ asyncNow = clockValue
      /\ UNCHANGED asyncNow
      => /\ (PublishControlItems(items)
                => HistoricalDiscoveryFixedClockPublicationFrame(
                     clockValue))
         /\ (PublishEphemeralItems(items)
                => HistoricalDiscoveryFixedClockPublicationFrame(
                     clockValue))
         /\ (PublishControlAndEphemeralItems(
               controlItems, ephemeralItems)
                => HistoricalDiscoveryFixedClockPublicationFrame(
                     clockValue))
         /\ (PublishCertifiedRequests(items)
                => HistoricalDiscoveryFixedClockPublicationFrame(
                     clockValue))
         /\ (PublishCommitCertificateRequests(items)
                => HistoricalDiscoveryFixedClockPublicationFrame(
                     clockValue))
BY HistoricalDiscoveryPacketUnionHasFixedClockFrame, Isa
   DEF PublishControlItems, PublishEphemeralItems,
       PublishControlAndEphemeralItems,
       PublishCertifiedRequests,
       PublishCommitCertificateRequests,
       HistoricalDiscoveryFixedClockPacketUnion

THEOREM HistoricalDiscoveryBroadcastControlHelpersHaveFixedClockFrame ==
  \A node, qc, items:
    \A broadcast \in BOOLEAN, clockValue \in Nat:
      /\ AsyncStrongTypeInvariant
      /\ asyncNow = clockValue
      /\ UNCHANGED asyncNow
      => /\ (PersistInstalledControl(
               node, items, broadcast)
                => HistoricalDiscoveryFixedClockPublicationFrame(
                     clockValue))
         /\ (PersistInstalledControlAfterInstall(
               node, qc, items, broadcast)
                => HistoricalDiscoveryFixedClockPublicationFrame(
                     clockValue))
         /\ (PersistDecisionControl(
               node, qc, items, broadcast)
                => HistoricalDiscoveryFixedClockPublicationFrame(
                     clockValue))
BY HistoricalDiscoveryOptionalPacketUnionHasFixedClockFrame, Isa
   DEF PersistInstalledControl,
       PersistInstalledControlAfterInstall,
       PersistDecisionControl,
       HistoricalDiscoveryFixedClockOptionalPacketUnion

THEOREM HistoricalDiscoveryRetransmissionHelpersHaveFixedClockFrame ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ asyncNow = clockValue
    /\ UNCHANGED asyncNow
    => /\ (SendAllItems(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
       /\ (SendNodeRetransmissions(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
BY HistoricalDiscoveryPacketUnionHasFixedClockFrame, Isa
   DEF SendAllItems, SendNodeRetransmissions,
       HistoricalDiscoveryFixedClockPacketUnion

(***************************************************************************
Concrete request and response publishers.

The discovery prefix and both I/O-worker domains carry their own fixed-clock
frame, so these corollaries need no external scheduler assumption.  In
particular the historical worker and discovery actions range over recovery
targets outside the current voting roster.
***************************************************************************)

THEOREM HistoricalDiscoveryDirectRequestPublicationHasFixedClockFrame ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ asyncNow = clockValue
    => /\ (CommitCertificateDiscoveryStepWork(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
       /\ (DirectCommitCertificateDiscoveryStep(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
       /\ (DirectHistoricalCommitCertificateDiscoveryStep(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
       /\ (PostGstCommitCertificateDiscovery(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
       /\ (PostGstHistoricalCommitCertificateDiscovery(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
BY HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame, Isa
   DEF CommitCertificateDiscoveryStepWork,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       PostGstCommitCertificateDiscovery,
       PostGstHistoricalCommitCertificateDiscovery

THEOREM HistoricalDiscoveryResponsePublicationHasFixedClockFrame ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ asyncNow = clockValue
    => /\ (ServiceIoWorkerWork(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
       /\ (ServiceIoWorker(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
       /\ (ServiceHistoricalRecoveryIoWorker(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
       /\ (PostGstServiceIoWorker(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
       /\ (PostGstServiceHistoricalRecoveryIoWorker(node)
              => HistoricalDiscoveryFixedClockPublicationFrame(
                   clockValue))
BY HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame, Isa
   DEF ServiceIoWorkerWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       PostGstServiceIoWorker,
       PostGstServiceHistoricalRecoveryIoWorker

(***************************************************************************
Direct singleton publication.

`InjectByzantineCertifiedRequest` does not call a `Publish*` helper, but its
packet is still exactly `PacketForItem(item)` and its action fixes the clock.
This keeps Byzantine request injection from refilling the frozen due set.
The three non-protocol singleton fault injectors have the same constructor
and therefore satisfy the same frame.
***************************************************************************)

THEOREM HistoricalDiscoveryByzantineCertifiedRequestHasFixedClockFrame ==
  \A source, recipient, qc, nonce:
    \A clockValue \in Nat:
      /\ AsyncStrongTypeInvariant
      /\ asyncNow = clockValue
      /\ InjectByzantineCertifiedRequest(
           source, recipient, qc, nonce)
      => HistoricalDiscoveryFixedClockPublicationFrame(clockValue)
BY HistoricalDiscoveryPacketUnionHasFixedClockFrame, Isa
   DEF InjectByzantineCertifiedRequest,
       PacketForItem, PacketsForItems,
       HistoricalDiscoveryFixedClockPacketUnion

THEOREM HistoricalDiscoverySingletonFaultInjectorsHaveFixedClockFrame ==
  \A kind, source, recipient, nonce:
    \A clockValue \in Nat:
      /\ AsyncStrongTypeInvariant
      /\ asyncNow = clockValue
      => /\ (InjectByzantineNoise(
                source, recipient, nonce)
                  => HistoricalDiscoveryFixedClockPublicationFrame(
                       clockValue))
         /\ (InjectUntrustedTransportCompletion(
                kind, recipient, nonce)
                  => HistoricalDiscoveryFixedClockPublicationFrame(
                       clockValue))
         /\ (InjectAuthenticatedJunk(
                kind, source, recipient, nonce)
                  => HistoricalDiscoveryFixedClockPublicationFrame(
                       clockValue))
BY HistoricalDiscoveryPacketUnionHasFixedClockFrame, Isa
   DEF InjectByzantineNoise,
       InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk,
       PacketForItem, PacketsForItems,
       HistoricalDiscoveryFixedClockPacketUnion

(***************************************************************************
Exact due-head removal.
***************************************************************************)

HistoricalDiscoverySelectedPacketAtLaneHead(recipient, source) ==
  LET packet == HistoricalDiscoverySelectedOverduePacket
  IN /\ OverdueResponsivePackets # {}
     /\ packet \in OverdueResponsivePackets
     /\ DueSourcePackets(recipient, source) # {}
     /\ packet = OldestDueSourcePacket(recipient, source)

THEOREM HistoricalDiscoverySelectedPacketIsOverdue ==
  OverdueResponsivePackets # {}
    => HistoricalDiscoverySelectedOverduePacket
         \in OverdueResponsivePackets
BY Isa DEF HistoricalDiscoverySelectedOverduePacket

THEOREM HistoricalDiscoverySelectedHeadBranchActionsRemoveExactPacket ==
  \A recipient \in ValidatorIds,
     source \in AsyncIngressSources:
    HistoricalDiscoverySelectedPacketAtLaneHead(recipient, source)
      => /\ (AdmitHiddenPacket(recipient, source)
                => /\ HistoricalDiscoverySelectedOverduePacket
                         \notin asyncTransport'
                   /\ UNCHANGED asyncNow)
         /\ (CoalesceHiddenPacket(recipient, source)
                => /\ HistoricalDiscoverySelectedOverduePacket
                         \notin asyncTransport'
                   /\ UNCHANGED asyncNow)
         /\ (DropPolicyRejectedHiddenPacket(recipient, source)
                => /\ HistoricalDiscoverySelectedOverduePacket
                         \notin asyncTransport'
                   /\ UNCHANGED asyncNow)
         /\ (AdmitIngressPacket(recipient, source)
                => /\ HistoricalDiscoverySelectedOverduePacket
                         \notin asyncTransport'
                   /\ UNCHANGED asyncNow)
BY Isa
   DEF HistoricalDiscoverySelectedPacketAtLaneHead,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket

THEOREM HistoricalDiscoverySelectedHeadPostGstActionsRemoveExactPacket ==
  \A recipient \in ValidatorIds,
     source \in AsyncIngressSources:
    HistoricalDiscoverySelectedPacketAtLaneHead(recipient, source)
      => /\ (PostGstAdmitHiddenPacket(recipient, source)
                => HistoricalDiscoverySelectedOverduePacket
                     \notin asyncTransport')
         /\ (PostGstAdmitHistoricalRecoveryPacket(
               recipient, source)
                => HistoricalDiscoverySelectedOverduePacket
                     \notin asyncTransport')
BY HistoricalDiscoverySelectedHeadBranchActionsRemoveExactPacket
   DEF PostGstAdmitHiddenPacket,
       PostGstAdmitHistoricalRecoveryPacket

(***************************************************************************
Every ingress branch removes exactly one due lane head and publishes no
replacement packet.  Consequently the global due-packet debt strictly
decreases at the fixed clock, regardless of whether that head is the selected
packet, a non-overdue shadow, or a different overdue packet.
***************************************************************************)

THEOREM HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket ==
  \A recipient \in ValidatorIds,
     source \in AsyncIngressSources,
     clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ asyncNow = clockValue
    /\ AdmitIngressPacket(recipient, source)
    => LET head ==
             OldestDueSourcePacket(recipient, source)
       IN /\ asyncNow' = clockValue
          /\ head \in HistoricalDiscoveryDuePacketsAt(clockValue)
          /\ HistoricalDiscoveryDuePacketsAt(clockValue)' =
               HistoricalDiscoveryDuePacketsAt(clockValue)
                 \ {head}
          /\ HistoricalDiscoveryDuePacketDebt(clockValue)' + 1 =
               HistoricalDiscoveryDuePacketDebt(clockValue)
BY OldestDueSourcePacketFacts,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, Isa
   DEF AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       HistoricalDiscoveryDuePacketsAt,
       HistoricalDiscoveryDuePacketDebt,
       DueSourcePackets,
       AsyncStrongTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncPacketContentTypeInvariant

(***************************************************************************
Selected non-overdue shadow descent.
***************************************************************************)

HistoricalDiscoveryBaseIngressDependencyRank(packet) ==
  IngressBoundaryDependencyRank(
    packet, packet.item.envelope.recipient,
    <<2, 0>>, <<5, 0>>)

THEOREM HistoricalDiscoverySelectedNonOverdueShadowStrictlyDescends ==
  \A packet \in OverdueResponsivePackets:
    /\ packet = HistoricalDiscoverySelectedOverduePacket
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ NonOverdueShadowAtLaneHead(packet)
    /\ AdmitNonOverdueShadowFor(packet)
    => <<HistoricalDiscoveryBaseIngressDependencyRank(packet)',
          HistoricalDiscoveryBaseIngressDependencyRank(packet)>>
         \in IngressBoundaryDependencyOrdering
BY OverduePacketHasTypedDueLane,
   NonOverdueShadowAdmissionDecreasesDependencyRank, Isa
   DEF HistoricalDiscoveryBaseIngressDependencyRank,
       OwnedServiceRankCarrier

(***************************************************************************
Exact residual action edges.

The request/response/control publisher surface and all four direct singleton
fault injectors are closed through the generic `PacketForItem` /
`PacketsForItems` frame.

For selected-packet service, the exact-head removal, non-overdue-shadow
dependency descent, and arbitrary due-head global-debt descent are proved.
The remaining ingress work is not packet removal: blocked capacity,
timeout-byte, shared-completion, claim, runner, auxiliary, Stage-4,
historical candidate, and historical Serve owners still require their
individual no-refill/descent action lemmas.
***************************************************************************)

=============================================================================
