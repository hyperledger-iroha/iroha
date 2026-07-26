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

The packet dependency now carries the minimal live historical candidate and
Serve ranks simultaneously.  Shadow admission lowers the first component of
`IngressBoundaryDependencyRank`, so lexicographic descent is independent of
how either later live tail changes.  This is the exact reason the old
base-only descent can soundly apply to the composite rank.
***************************************************************************)

THEOREM HistoricalDiscoveryNonOverdueShadowRetainsOverduePacket ==
  \A packet \in OverdueResponsivePackets:
    /\ AsyncStrongTypeInvariant
    /\ NonOverdueShadowAtLaneHead(packet)
    /\ AdmitNonOverdueShadowFor(packet)
    => packet \in OverdueResponsivePackets'
BY NonOverdueShadowAdmissionRemovesExactShadow, Isa
   DEF NonOverdueShadowAtLaneHead,
       AdmitNonOverdueShadowFor,
       PostGstAdmitHiddenPacket,
       PostGstAdmitHistoricalRecoveryPacket,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       OverdueResponsivePackets,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       NodeHasApplication, CurrentVoters, CurrentEpoch

THEOREM HistoricalDiscoverySelectedNonOverdueShadowStrictlyDescends ==
  \A packet \in OverdueResponsivePackets:
    /\ packet = HistoricalDiscoverySelectedOverduePacket
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ NonOverdueShadowAtLaneHead(packet)
    /\ AdmitNonOverdueShadowFor(packet)
    => <<HistoricalDiscoveryPacketDependencyRank(packet)',
          HistoricalDiscoveryPacketDependencyRank(packet)>>
         \in IngressBoundaryDependencyOrdering
BY HistoricalDiscoveryNonOverdueShadowRetainsOverduePacket,
   NonOverdueShadowAdmissionRemovesExactShadow,
   HistoricalDiscoveryPacketDependencyRankInCarrier, Isa
   DEF HistoricalDiscoveryPacketDependencyRank,
       IngressBoundaryDependencyRank,
       IngressBoundaryDependencyOrdering,
       IngressCapacityTailOrdering,
       LexPairOrdering, OpToRel

(***************************************************************************
One-step classification of deterministic candidate and Serve minima.

These predicates classify a retained overdue packet across one bracketed
`AsyncNext` step.  Persistence requires the exact old minimizing owner to
remain at the same rank and excludes a newly lower successor rank.  Exit
names removal of that exact owner.  A retained owner whose rank changes and
a retained unchanged owner shadowed by a lower successor rank are separate
cases, so the classification does not smuggle either case into "unchanged".

Inserted owners are selected by taking the minimum of the ranks of exactly
the successor owners absent from the old owner set.  Thus the insertion
witness is deterministic up to equal-rank duplicate owners; its rank is
deterministic.  A lower inserted rank forces the successor minimum lower.
The inserted witness is used only under the corresponding nonempty insertion
guard; outside that guard its `CHOOSE` value is deliberately unconstrained.
Exit or arbitrary reselection alone does not: the algebraic counterexample
`HistoricalDiscoveryPlainMinimumRemovalCanIncrease` remains the explicit
residual requiring an earlier dependency component or a multiplicity rank.
***************************************************************************)

HistoricalDiscoveryCandidateMinimumPersistenceStep(packet) ==
  LET selected ==
        HistoricalDiscoveryPacketCandidateDebtWitness(packet)
      rank ==
        HistoricalDiscoveryPacketCandidateDebtRank(packet)
  IN /\ HistoricalDiscoveryPacketCandidateOwners(packet) # {}
     /\ selected
          \in HistoricalDiscoveryPacketCandidateOwners(packet)'
     /\ CandidateServiceRank(selected)' = rank
     /\ \A other
             \in HistoricalDiscoveryPacketCandidateRanks(packet)':
          <<other, rank>>
            \notin OwnedServiceRankOrdering

HistoricalDiscoveryServeMinimumPersistenceStep(packet) ==
  LET selected ==
        HistoricalDiscoveryPacketServeDebtWitness(packet)
      rank ==
        HistoricalDiscoveryPacketServeDebtRank(packet)
      recipient == packet.item.envelope.recipient
  IN /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
     /\ selected \in HistoricalDiscoveryPacketServeOwners(packet)'
     /\ ServeJobRank(recipient, selected)' = rank
     /\ \A other \in HistoricalDiscoveryPacketServeRanks(packet)':
          <<other, rank>>
            \notin OwnedServiceRankOrdering

HistoricalDiscoveryCandidateMinimumExitStep(packet) ==
  LET selected ==
        HistoricalDiscoveryPacketCandidateDebtWitness(packet)
  IN /\ HistoricalDiscoveryPacketCandidateOwners(packet) # {}
     /\ selected
          \notin HistoricalDiscoveryPacketCandidateOwners(packet)'

HistoricalDiscoveryServeMinimumExitStep(packet) ==
  LET selected ==
        HistoricalDiscoveryPacketServeDebtWitness(packet)
  IN /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
     /\ selected \notin HistoricalDiscoveryPacketServeOwners(packet)'

HistoricalDiscoveryCandidateMinimumOwnerRankChangeStep(packet) ==
  LET selected ==
        HistoricalDiscoveryPacketCandidateDebtWitness(packet)
      rank ==
        HistoricalDiscoveryPacketCandidateDebtRank(packet)
  IN /\ HistoricalDiscoveryPacketCandidateOwners(packet) # {}
     /\ selected
          \in HistoricalDiscoveryPacketCandidateOwners(packet)'
     /\ CandidateServiceRank(selected)' # rank

HistoricalDiscoveryServeMinimumOwnerRankChangeStep(packet) ==
  LET selected ==
        HistoricalDiscoveryPacketServeDebtWitness(packet)
      rank ==
        HistoricalDiscoveryPacketServeDebtRank(packet)
      recipient == packet.item.envelope.recipient
  IN /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
     /\ selected \in HistoricalDiscoveryPacketServeOwners(packet)'
     /\ ServeJobRank(recipient, selected)' # rank

HistoricalDiscoveryCandidateMinimumLowerReselectionStep(packet) ==
  LET selected ==
        HistoricalDiscoveryPacketCandidateDebtWitness(packet)
      rank ==
        HistoricalDiscoveryPacketCandidateDebtRank(packet)
  IN /\ HistoricalDiscoveryPacketCandidateOwners(packet) # {}
     /\ selected
          \in HistoricalDiscoveryPacketCandidateOwners(packet)'
     /\ CandidateServiceRank(selected)' = rank
     /\ \E other
             \in HistoricalDiscoveryPacketCandidateRanks(packet)':
          <<other, rank>>
            \in OwnedServiceRankOrdering

HistoricalDiscoveryServeMinimumLowerReselectionStep(packet) ==
  LET selected ==
        HistoricalDiscoveryPacketServeDebtWitness(packet)
      rank ==
        HistoricalDiscoveryPacketServeDebtRank(packet)
      recipient == packet.item.envelope.recipient
  IN /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
     /\ selected \in HistoricalDiscoveryPacketServeOwners(packet)'
     /\ ServeJobRank(recipient, selected)' = rank
     /\ \E other \in HistoricalDiscoveryPacketServeRanks(packet)':
          <<other, rank>>
            \in OwnedServiceRankOrdering

HistoricalDiscoveryPacketCandidateInsertedOwners(packet) ==
  HistoricalDiscoveryPacketCandidateOwners(packet)'
    \ HistoricalDiscoveryPacketCandidateOwners(packet)

HistoricalDiscoveryPacketServeInsertedOwners(packet) ==
  HistoricalDiscoveryPacketServeOwners(packet)'
    \ HistoricalDiscoveryPacketServeOwners(packet)

HistoricalDiscoveryPacketCandidateInsertedRanks(packet) ==
  {CandidateServiceRank(candidate)':
     candidate
       \in HistoricalDiscoveryPacketCandidateInsertedOwners(packet)}

HistoricalDiscoveryPacketServeInsertedRanks(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN {ServeJobRank(recipient, job)':
        job \in HistoricalDiscoveryPacketServeInsertedOwners(packet)}

HistoricalDiscoveryPacketCandidateInsertedDebtRank(packet) ==
  LET ranks ==
        HistoricalDiscoveryPacketCandidateInsertedRanks(packet)
  IN IF ranks = {}
     THEN HistoricalDiscoveryCandidateDebtBottom
     ELSE HistoricalDiscoveryOwnedRankMinimum(ranks)

HistoricalDiscoveryPacketServeInsertedDebtRank(packet) ==
  LET ranks ==
        HistoricalDiscoveryPacketServeInsertedRanks(packet)
  IN IF ranks = {}
     THEN HistoricalDiscoveryServeDebtBottom
     ELSE HistoricalDiscoveryOwnedRankMinimum(ranks)

HistoricalDiscoveryPacketCandidateInsertedDebtWitness(packet) ==
  CHOOSE candidate
    \in HistoricalDiscoveryPacketCandidateInsertedOwners(packet):
      CandidateServiceRank(candidate)'
        = HistoricalDiscoveryPacketCandidateInsertedDebtRank(packet)

HistoricalDiscoveryPacketServeInsertedDebtWitness(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN CHOOSE job
       \in HistoricalDiscoveryPacketServeInsertedOwners(packet):
         ServeJobRank(recipient, job)'
           = HistoricalDiscoveryPacketServeInsertedDebtRank(packet)

HistoricalDiscoveryCandidateOwnerInsertionStep(packet) ==
  HistoricalDiscoveryPacketCandidateInsertedOwners(packet) # {}

HistoricalDiscoveryServeOwnerInsertionStep(packet) ==
  HistoricalDiscoveryPacketServeInsertedOwners(packet) # {}

THEOREM HistoricalDiscoveryCandidateMinimumPersistenceKeepsTail ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryCandidateMinimumPersistenceStep(packet)
    => LET selected ==
             HistoricalDiscoveryPacketCandidateDebtWitness(packet)
           rank ==
             HistoricalDiscoveryPacketCandidateDebtRank(packet)
       IN /\ selected
               \in HistoricalDiscoveryPacketCandidateOwners(packet)
          /\ CandidateServiceRank(selected) = rank
          /\ selected
               \in HistoricalDiscoveryPacketCandidateOwners(packet)'
          /\ CandidateServiceRank(selected)' = rank
          /\ HistoricalDiscoveryPacketCandidateDebtRank(packet)' =
               rank
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryLiveCandidateDebtHasExactFairOwner,
   HistoricalDiscoveryPacketCandidateRanksInCarrier,
   HistoricalDiscoveryOwnedRankMinimumStable, Isa
   DEF HistoricalDiscoveryCandidateMinimumPersistenceStep,
       HistoricalDiscoveryPacketCandidateDebtRank,
       HistoricalDiscoveryPacketCandidateRanks

THEOREM HistoricalDiscoveryServeMinimumPersistenceKeepsTail ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryServeMinimumPersistenceStep(packet)
    => LET selected ==
             HistoricalDiscoveryPacketServeDebtWitness(packet)
           rank ==
             HistoricalDiscoveryPacketServeDebtRank(packet)
           recipient == packet.item.envelope.recipient
       IN /\ selected
               \in HistoricalDiscoveryPacketServeOwners(packet)
          /\ ServeJobRank(recipient, selected) = rank
          /\ selected \in HistoricalDiscoveryPacketServeOwners(packet)'
          /\ ServeJobRank(recipient, selected)' = rank
          /\ HistoricalDiscoveryPacketServeDebtRank(packet)' = rank
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryLiveServeDebtHasExactFairOwner,
   HistoricalDiscoveryPacketServeRanksInCarrier,
   HistoricalDiscoveryOwnedRankMinimumStable, Isa
   DEF HistoricalDiscoveryServeMinimumPersistenceStep,
       HistoricalDiscoveryPacketServeDebtRank,
       HistoricalDiscoveryPacketServeRanks

THEOREM HistoricalDiscoveryCandidateMinimumExitClassifiesTail ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryCandidateMinimumExitStep(packet)
    => LET selected ==
             HistoricalDiscoveryPacketCandidateDebtWitness(packet)
           oldRank ==
             HistoricalDiscoveryPacketCandidateDebtRank(packet)
           nextRank ==
             HistoricalDiscoveryPacketCandidateDebtRank(packet)'
       IN /\ selected
               \in HistoricalDiscoveryPacketCandidateOwners(packet)
          /\ CandidateServiceRank(selected) = oldRank
          /\ selected
               \notin HistoricalDiscoveryPacketCandidateOwners(packet)'
          /\ \/ /\ HistoricalDiscoveryPacketCandidateOwners(packet)'
                       = {}
                /\ nextRank =
                     HistoricalDiscoveryCandidateDebtBottom
             \/ /\ HistoricalDiscoveryPacketCandidateOwners(packet)'
                       # {}
                /\ HistoricalDiscoveryPacketCandidateDebtWitness(packet)'
                     \in
                     HistoricalDiscoveryPacketCandidateOwners(packet)'
                /\ CandidateServiceRank(
                     HistoricalDiscoveryPacketCandidateDebtWitness(
                       packet))'
                     = nextRank
                /\ nextRank \in OwnedServiceRankCarrier
                /\ \A other
                        \in
                        HistoricalDiscoveryPacketCandidateRanks(packet)':
                     <<other, nextRank>>
                       \notin OwnedServiceRankOrdering
                /\ \/ nextRank = oldRank
                   \/ <<nextRank, oldRank>>
                        \in OwnedServiceRankOrdering
                   \/ <<oldRank, nextRank>>
                        \in OwnedServiceRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryLiveCandidateDebtHasExactFairOwner,
   HistoricalDiscoveryEmptyPacketDebtUsesExactBottoms,
   HistoricalDiscoveryOwnedRankTrichotomy, Isa
   DEF HistoricalDiscoveryCandidateMinimumExitStep

THEOREM HistoricalDiscoveryServeMinimumExitClassifiesTail ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryServeMinimumExitStep(packet)
    => LET selected ==
             HistoricalDiscoveryPacketServeDebtWitness(packet)
           oldRank ==
             HistoricalDiscoveryPacketServeDebtRank(packet)
           nextRank ==
             HistoricalDiscoveryPacketServeDebtRank(packet)'
           recipient == packet.item.envelope.recipient
       IN /\ selected
               \in HistoricalDiscoveryPacketServeOwners(packet)
          /\ ServeJobRank(recipient, selected) = oldRank
          /\ selected
               \notin HistoricalDiscoveryPacketServeOwners(packet)'
          /\ \/ /\ HistoricalDiscoveryPacketServeOwners(packet)' = {}
                /\ nextRank = HistoricalDiscoveryServeDebtBottom
             \/ /\ HistoricalDiscoveryPacketServeOwners(packet)' # {}
                /\ HistoricalDiscoveryPacketServeDebtWitness(packet)'
                     \in HistoricalDiscoveryPacketServeOwners(packet)'
                /\ ServeJobRank(
                     recipient,
                     HistoricalDiscoveryPacketServeDebtWitness(packet))'
                     = nextRank
                /\ nextRank \in OwnedServiceRankCarrier
                /\ \A other
                        \in HistoricalDiscoveryPacketServeRanks(packet)':
                     <<other, nextRank>>
                       \notin OwnedServiceRankOrdering
                /\ \/ nextRank = oldRank
                   \/ <<nextRank, oldRank>>
                        \in OwnedServiceRankOrdering
                   \/ <<oldRank, nextRank>>
                        \in OwnedServiceRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryLiveServeDebtHasExactFairOwner,
   HistoricalDiscoveryEmptyPacketDebtUsesExactBottoms,
   HistoricalDiscoveryOwnedRankTrichotomy, Isa
   DEF HistoricalDiscoveryServeMinimumExitStep

THEOREM HistoricalDiscoveryCandidateInsertionHasExactMinimum ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryCandidateOwnerInsertionStep(packet)
    => LET inserted ==
             HistoricalDiscoveryPacketCandidateInsertedOwners(packet)
           witness ==
             HistoricalDiscoveryPacketCandidateInsertedDebtWitness(
               packet)
           insertedRank ==
             HistoricalDiscoveryPacketCandidateInsertedDebtRank(packet)
           oldRank ==
             HistoricalDiscoveryPacketCandidateDebtRank(packet)
       IN /\ witness \in inserted
          /\ witness
               \in HistoricalDiscoveryPacketCandidateOwners(packet)'
          /\ witness
               \notin HistoricalDiscoveryPacketCandidateOwners(packet)
          /\ CandidateServiceRank(witness)' = insertedRank
          /\ insertedRank \in OwnedServiceRankCarrier
          /\ \A other
                  \in
                  HistoricalDiscoveryPacketCandidateInsertedRanks(
                    packet):
               <<other, insertedRank>>
                 \notin OwnedServiceRankOrdering
          /\ \/ /\ HistoricalDiscoveryPacketCandidateOwners(packet)
                       = {}
                /\ HistoricalDiscoveryPacketCandidateDebtRank(packet)'
                     = insertedRank
             \/ /\ HistoricalDiscoveryPacketCandidateOwners(packet)
                       # {}
                /\ \/ insertedRank = oldRank
                   \/ <<insertedRank, oldRank>>
                        \in OwnedServiceRankOrdering
                   \/ <<oldRank, insertedRank>>
                        \in OwnedServiceRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryPacketCandidateRanksInCarrier,
   HistoricalDiscoveryOwnedRankMinimumFacts,
   HistoricalDiscoveryOwnedRankTrichotomy, Isa
   DEF HistoricalDiscoveryCandidateOwnerInsertionStep,
       HistoricalDiscoveryPacketCandidateInsertedOwners,
       HistoricalDiscoveryPacketCandidateInsertedRanks,
       HistoricalDiscoveryPacketCandidateInsertedDebtRank,
       HistoricalDiscoveryPacketCandidateInsertedDebtWitness,
       HistoricalDiscoveryPacketCandidateOwners,
       HistoricalDiscoveryPacketCandidateRanks,
       HistoricalDiscoveryPacketCandidateDebtRank

THEOREM HistoricalDiscoveryServeInsertionHasExactMinimum ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryServeOwnerInsertionStep(packet)
    => LET inserted ==
             HistoricalDiscoveryPacketServeInsertedOwners(packet)
           witness ==
             HistoricalDiscoveryPacketServeInsertedDebtWitness(packet)
           insertedRank ==
             HistoricalDiscoveryPacketServeInsertedDebtRank(packet)
           oldRank ==
             HistoricalDiscoveryPacketServeDebtRank(packet)
           recipient == packet.item.envelope.recipient
       IN /\ witness \in inserted
          /\ witness \in HistoricalDiscoveryPacketServeOwners(packet)'
          /\ witness
               \notin HistoricalDiscoveryPacketServeOwners(packet)
          /\ ServeJobRank(recipient, witness)' = insertedRank
          /\ insertedRank \in OwnedServiceRankCarrier
          /\ \A other
                  \in HistoricalDiscoveryPacketServeInsertedRanks(
                    packet):
               <<other, insertedRank>>
                 \notin OwnedServiceRankOrdering
          /\ \/ /\ HistoricalDiscoveryPacketServeOwners(packet) = {}
                /\ HistoricalDiscoveryPacketServeDebtRank(packet)' =
                     insertedRank
             \/ /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
                /\ \/ insertedRank = oldRank
                   \/ <<insertedRank, oldRank>>
                        \in OwnedServiceRankOrdering
                   \/ <<oldRank, insertedRank>>
                        \in OwnedServiceRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryPacketServeRanksInCarrier,
   HistoricalDiscoveryOwnedRankMinimumFacts,
   HistoricalDiscoveryOwnedRankTrichotomy, Isa
   DEF HistoricalDiscoveryServeOwnerInsertionStep,
       HistoricalDiscoveryPacketServeInsertedOwners,
       HistoricalDiscoveryPacketServeInsertedRanks,
       HistoricalDiscoveryPacketServeInsertedDebtRank,
       HistoricalDiscoveryPacketServeInsertedDebtWitness,
       HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryPacketServeRanks,
       HistoricalDiscoveryPacketServeDebtRank

THEOREM HistoricalDiscoveryLowerCandidateInsertionReselectsLower ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryCandidateOwnerInsertionStep(packet)
    /\ HistoricalDiscoveryPacketCandidateOwners(packet) # {}
    /\ <<HistoricalDiscoveryPacketCandidateInsertedDebtRank(packet),
          HistoricalDiscoveryPacketCandidateDebtRank(packet)>>
         \in OwnedServiceRankOrdering
    => <<HistoricalDiscoveryPacketCandidateDebtRank(packet)',
          HistoricalDiscoveryPacketCandidateDebtRank(packet)>>
         \in OwnedServiceRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryCandidateInsertionHasExactMinimum,
   HistoricalDiscoveryPacketCandidateRanksInCarrier,
   HistoricalDiscoveryLowerOwnedRankForcesMinimumDescent, Isa
   DEF HistoricalDiscoveryPacketCandidateInsertedRanks,
       HistoricalDiscoveryPacketCandidateDebtRank

THEOREM HistoricalDiscoveryLowerServeInsertionReselectsLower ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryServeOwnerInsertionStep(packet)
    /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
    /\ <<HistoricalDiscoveryPacketServeInsertedDebtRank(packet),
          HistoricalDiscoveryPacketServeDebtRank(packet)>>
         \in OwnedServiceRankOrdering
    => <<HistoricalDiscoveryPacketServeDebtRank(packet)',
          HistoricalDiscoveryPacketServeDebtRank(packet)>>
         \in OwnedServiceRankOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryServeInsertionHasExactMinimum,
   HistoricalDiscoveryPacketServeRanksInCarrier,
   HistoricalDiscoveryLowerOwnedRankForcesMinimumDescent, Isa
   DEF HistoricalDiscoveryPacketServeInsertedRanks,
       HistoricalDiscoveryPacketServeDebtRank

THEOREM HistoricalDiscoveryRetainedPacketMinimumStepCases ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    => /\ (HistoricalDiscoveryPacketCandidateOwners(packet) = {}
             => \/ /\ HistoricalDiscoveryPacketCandidateOwners(packet)'
                          = {}
                    /\ HistoricalDiscoveryPacketCandidateDebtRank(packet)
                         = HistoricalDiscoveryCandidateDebtBottom
                    /\ HistoricalDiscoveryPacketCandidateDebtRank(packet)'
                         = HistoricalDiscoveryCandidateDebtBottom
                \/ HistoricalDiscoveryCandidateOwnerInsertionStep(
                     packet))
       /\ (HistoricalDiscoveryPacketCandidateOwners(packet) # {}
             => \/ HistoricalDiscoveryCandidateMinimumPersistenceStep(
                       packet)
                \/ HistoricalDiscoveryCandidateMinimumExitStep(packet)
                \/ HistoricalDiscoveryCandidateMinimumOwnerRankChangeStep(
                     packet)
                \/ HistoricalDiscoveryCandidateMinimumLowerReselectionStep(
                     packet))
       /\ (HistoricalDiscoveryPacketServeOwners(packet) = {}
             => \/ /\ HistoricalDiscoveryPacketServeOwners(packet)' = {}
                    /\ HistoricalDiscoveryPacketServeDebtRank(packet)
                         = HistoricalDiscoveryServeDebtBottom
                    /\ HistoricalDiscoveryPacketServeDebtRank(packet)'
                         = HistoricalDiscoveryServeDebtBottom
                \/ HistoricalDiscoveryServeOwnerInsertionStep(packet))
       /\ (HistoricalDiscoveryPacketServeOwners(packet) # {}
             => \/ HistoricalDiscoveryServeMinimumPersistenceStep(
                       packet)
                \/ HistoricalDiscoveryServeMinimumExitStep(packet)
                \/ HistoricalDiscoveryServeMinimumOwnerRankChangeStep(
                     packet)
                \/ HistoricalDiscoveryServeMinimumLowerReselectionStep(
                     packet))
BY HistoricalDiscoveryEmptyPacketDebtUsesExactBottoms, Isa
   DEF HistoricalDiscoveryCandidateMinimumPersistenceStep,
       HistoricalDiscoveryServeMinimumPersistenceStep,
       HistoricalDiscoveryCandidateMinimumExitStep,
       HistoricalDiscoveryServeMinimumExitStep,
       HistoricalDiscoveryCandidateMinimumOwnerRankChangeStep,
       HistoricalDiscoveryServeMinimumOwnerRankChangeStep,
       HistoricalDiscoveryCandidateMinimumLowerReselectionStep,
       HistoricalDiscoveryServeMinimumLowerReselectionStep,
       HistoricalDiscoveryCandidateOwnerInsertionStep,
       HistoricalDiscoveryServeOwnerInsertionStep,
       HistoricalDiscoveryPacketCandidateInsertedOwners,
       HistoricalDiscoveryPacketServeInsertedOwners

(***************************************************************************
Logical-owner/occurrence-count refinement across concrete actions.

The count-first rank below closes exactly the removal case which the plain
minimum cannot orient.  It deliberately does not hide replacement: if the
selected candidate exits while the same runner step creates enough fresh
protected owners to preserve or grow the owner count, the named replenishment
residual remains.  Candidate cardinality is only the number of distinct
logical owner values.  `AsyncProgressOwnershipInvariant` makes that count
exact on the live specification.

The action classes are intentionally separate.  Under that ownership
invariant, exact no-refill removal is a concrete logical-owner removal.
Executable `FifoRuntimeStep` and executable `DeferredDrainStep` can instead
remove one parent and call `AppendCausalSuccessors`, whose closed
`CommandSuccessors` inventory can produce up to three fresh children.  Pure
producer edges also exist: `DrainFairIngressSelected` can enqueue an
authenticated delivery candidate; `DirectTimeoutStep` and
`DeferredTimeoutStep` can append the `PersistTimeout` child of
`TimeoutCausalCommand`; and
`DirectRetransmitStep` and `DeferredRetransmitStep` can append the historical
locked-body retry.  If `AsyncProgressOwnershipInvariant` is absent, identical
candidate values may represent collapsed physical copies; that is a distinct
duplicate-occurrence residual, not replenishment.

Serve service is different.  The exact ordinary/historical I/O fair action
tails one queue and never appends to it.  A Serve head removes one unique
nonce-owned occurrence; a non-Serve head preserves the Serve owner count and
lowers every retained Serve position.  The two cases give strict descent of
the occurrence rank.  Serve replenishment is confined to the authorized
request branches of `DrainFairIngressSelected` and
`DrainHistoricalIngressSelected`, which append `AsyncIoCertifiedServeJob`.

The finite per-action fanout theorem is not a total production bound.  A
temporal closure must place a well-founded producer budget before this
count-first consumer rank and prove that every candidate/Serve insertion or
count-preserving-or-growing replacement strictly lowers that earlier budget.
No such producer-budget descent is asserted in this action-local module.
***************************************************************************)

HistoricalDiscoveryCandidateExactLogicalOwnerNoRefillExitStep(packet) ==
  LET selected ==
        HistoricalDiscoveryPacketCandidateDebtWitness(packet)
  IN /\ AsyncProgressOwnershipInvariant
     /\ HistoricalDiscoveryCandidateMinimumExitStep(packet)
     /\ HistoricalDiscoveryPacketCandidateOwners(packet)' =
          HistoricalDiscoveryPacketCandidateOwners(packet)
            \ {selected}

HistoricalDiscoveryServeExactNoRefillExitStep(packet) ==
  LET selected ==
        HistoricalDiscoveryPacketServeDebtWitness(packet)
  IN /\ HistoricalDiscoveryServeMinimumExitStep(packet)
     /\ HistoricalDiscoveryPacketServeOwners(packet)' =
          HistoricalDiscoveryPacketServeOwners(packet)
            \ {selected}

HistoricalDiscoveryCandidateExitReplenishmentResidual(packet) ==
  /\ HistoricalDiscoveryCandidateMinimumExitStep(packet)
  /\ AsyncProgressOwnershipInvariant
  /\ Cardinality(HistoricalDiscoveryPacketCandidateOwners(packet))
       <= Cardinality(
            HistoricalDiscoveryPacketCandidateOwners(packet)')

HistoricalDiscoveryCandidateDuplicateOccurrenceResidual(packet) ==
  /\ HistoricalDiscoveryCandidateMinimumExitStep(packet)
  /\ ~AsyncProgressOwnershipInvariant

HistoricalDiscoveryServeExitReplenishmentResidual(packet) ==
  /\ HistoricalDiscoveryServeMinimumExitStep(packet)
  /\ Cardinality(HistoricalDiscoveryPacketServeOwners(packet))
       <= Cardinality(HistoricalDiscoveryPacketServeOwners(packet)')

HistoricalDiscoveryPacketServeHeadIsOwner(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
     /\ Head(asyncIoQueues[recipient])
          \in HistoricalDiscoveryPacketServeOwners(packet)

HistoricalDiscoveryPacketServeHeadIsNotOwner(packet) ==
  LET recipient == packet.item.envelope.recipient
  IN /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
     /\ Head(asyncIoQueues[recipient])
          \notin HistoricalDiscoveryPacketServeOwners(packet)

HistoricalDiscoveryPacketProducerResidual(packet) ==
  \/ HistoricalDiscoveryCandidateOwnerInsertionStep(packet)
  \/ HistoricalDiscoveryCandidateExitReplenishmentResidual(packet)
  \/ HistoricalDiscoveryServeOwnerInsertionStep(packet)
  \/ HistoricalDiscoveryServeExitReplenishmentResidual(packet)

(***************************************************************************
A Serve owner at the FIFO head has occurrence index one.  Every packet Serve
rank has stage five and a positive index, so <<5, 1>> is the exact minimum.
Nonce uniqueness then makes the owner at index one equal to the chosen
minimum witness.  The service proof below consumes this named bridge before
claiming exact selected-owner removal.
***************************************************************************)

THEOREM HistoricalDiscoveryServeHeadOwnerIsExactMinimumWitness ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryPacketServeHeadIsOwner(packet)
    => LET recipient == packet.item.envelope.recipient
           queue == asyncIoQueues[recipient]
           headJob == Head(queue)
           selected ==
             HistoricalDiscoveryPacketServeDebtWitness(packet)
       IN /\ ServeJobRank(recipient, headJob) = <<5, 1>>
          /\ HistoricalDiscoveryPacketServeDebtRank(packet) = <<5, 1>>
          /\ selected = headJob
PROOF
  <1>1. ASSUME NEW packet,
                packet \in OverdueResponsivePackets,
                AsyncStrongTypeInvariant,
                HistoricalDiscoveryPacketServeHeadIsOwner(packet)
         PROVE LET recipient == packet.item.envelope.recipient
                   queue == asyncIoQueues[recipient]
                   headJob == Head(queue)
                   selected ==
                     HistoricalDiscoveryPacketServeDebtWitness(packet)
               IN /\ ServeJobRank(recipient, headJob) = <<5, 1>>
                  /\ HistoricalDiscoveryPacketServeDebtRank(packet)
                       = <<5, 1>>
                  /\ selected = headJob
    <2> DEFINE Recipient == packet.item.envelope.recipient
    <2> DEFINE Queue == asyncIoQueues[Recipient]
    <2> DEFINE HeadJob == Head(Queue)
    <2> DEFINE Selected ==
           HistoricalDiscoveryPacketServeDebtWitness(packet)
    <2>1. /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
           /\ HeadJob
                \in HistoricalDiscoveryPacketServeOwners(packet)
      BY <1>1
         DEF HistoricalDiscoveryPacketServeHeadIsOwner,
             Recipient, Queue, HeadJob
    <2>2. /\ Recipient \in ValidatorIds
           /\ AsyncIoSequenceTyped(Queue)
           /\ AsyncIoServeNonceOwnership(Queue)
      BY <1>1, <2>1,
         AsyncStrongTypeProjectsAsyncType,
         HistoricalDiscoveryOwnersIncludeNonVoterService,
         AsyncTimedServiceNodesAreValidators, Isa
         DEF Recipient, Queue,
             HistoricalDiscoveryPacketServeOwners,
             HistoricalDiscoveryServeJobOwned,
             HistoricalDiscoveryIoOwners,
             AsyncStrongTypeInvariant, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant
    <2>3. /\ HeadJob \in AsyncServeJobSet
           /\ HeadJob \in SequenceSet(Queue)
           /\ Len(Queue) > 0
           /\ HeadJob = Queue[1]
           /\ 1 \in AsyncIoServeIndices(Queue)
      BY <2>1, <2>2, PositiveSequenceIsNonempty,
         NonemptySequenceHeadIsFirst, Isa
         DEF HeadJob, HistoricalDiscoveryPacketServeOwners,
             HistoricalDiscoveryServeJobOwned, SequenceSet,
             AsyncIoServeIndices, AsyncServeJobSet, AsyncIoJob
    <2>4. ServeOccurrenceIndex(HeadJob, Queue) = 1
      BY <2>2, <2>3, ServeOccurrenceIndexCharacterization, Isa
    <2>5. ServeJobRank(Recipient, HeadJob) = <<5, 1>>
      BY <2>4, ServeJobIndexMatchesOccurrenceIndex
         DEF ServeJobRank
    <2>6. /\ Selected
                \in HistoricalDiscoveryPacketServeOwners(packet)
           /\ ServeJobRank(Recipient, Selected)
                = HistoricalDiscoveryPacketServeDebtRank(packet)
           /\ \A other
                   \in HistoricalDiscoveryPacketServeRanks(packet):
                <<other,
                  HistoricalDiscoveryPacketServeDebtRank(packet)>>
                  \notin OwnedServiceRankOrdering
      BY <1>1, <2>1,
         HistoricalDiscoveryLiveServeDebtHasExactFairOwner
         DEF Recipient, Selected
    <2>7. <<5, 1>>
             \in HistoricalDiscoveryPacketServeRanks(packet)
      BY <2>1, <2>5
         DEF HistoricalDiscoveryPacketServeRanks, Recipient, HeadJob
    <2>8. /\ HistoricalDiscoveryPacketServeDebtRank(packet)
                  \in ({5} \X (1..AsyncIoCapacity))
           /\ <<<<5, 1>>,
                 HistoricalDiscoveryPacketServeDebtRank(packet)>>
                \notin OwnedServiceRankOrdering
      BY <1>1, <2>6, <2>7,
         HistoricalDiscoveryPacketServeRanksHaveConcreteBound, Isa
         DEF HistoricalDiscoveryPacketServeRanks
    <2>9. HistoricalDiscoveryPacketServeDebtRank(packet)
             = <<5, 1>>
      BY <2>8, OwnedServiceRankOrderingMatchesLess, SMT
         DEF ServiceRankLess
    <2>10. /\ Selected \in AsyncServeJobSet
            /\ Selected \in SequenceSet(Queue)
      BY <2>6, Isa
         DEF HistoricalDiscoveryPacketServeOwners,
             HistoricalDiscoveryServeJobOwned, Recipient, Queue
    <2>11. ServeOccurrenceIndex(Selected, Queue) = 1
      BY <2>6, <2>9, ServeJobIndexMatchesOccurrenceIndex, Isa
         DEF ServeJobRank, Recipient, Queue
    <2>12. Queue[1] = Selected
      BY <2>2, <2>10, <2>11,
         ServeOccurrenceIndexCharacterization
    <2>13. Selected = HeadJob
      BY <2>3, <2>12
    <2> QED BY <2>5, <2>9, <2>13
         DEF Recipient, Queue, HeadJob, Selected
  <1> QED BY <1>1

THEOREM HistoricalDiscoveryCandidateExactLogicalOwnerRemovalLowersDebt ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryCandidateExactLogicalOwnerNoRefillExitStep(packet)
    => <<HistoricalDiscoveryPacketCandidateOccurrenceDebtRank(packet)',
          HistoricalDiscoveryPacketCandidateOccurrenceDebtRank(packet)>>
         \in HistoricalDiscoveryOccurrenceDebtOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   AsyncBracketNextPreservesProgressOwnership,
   HistoricalDiscoveryPacketOccurrenceDebtRanksInCarrier,
   HistoricalDiscoveryLiveCandidateDebtHasExactFairOwner,
   FS_RemoveElement, FS_CardinalityType, Isa
   DEF HistoricalDiscoveryCandidateExactLogicalOwnerNoRefillExitStep,
       HistoricalDiscoveryCandidateMinimumExitStep,
       HistoricalDiscoveryPacketCandidateOccurrenceDebtRank,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       LexPairOrdering, OpToRel

THEOREM HistoricalDiscoveryServeExactRemovalLowersOccurrenceDebt ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryServeExactNoRefillExitStep(packet)
    => <<HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)',
          HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)>>
         \in HistoricalDiscoveryOccurrenceDebtOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryPacketOccurrenceDebtRanksInCarrier,
   HistoricalDiscoveryLiveServeDebtHasExactFairOwner,
   FS_RemoveElement, FS_CardinalityType, Isa
   DEF HistoricalDiscoveryServeExactNoRefillExitStep,
       HistoricalDiscoveryServeMinimumExitStep,
       HistoricalDiscoveryPacketServeOccurrenceDebtRank,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       LexPairOrdering, OpToRel

THEOREM HistoricalDiscoveryCandidateExitClassifiesOccurrenceDebt ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryCandidateMinimumExitStep(packet)
    => \/ <<HistoricalDiscoveryPacketCandidateOccurrenceDebtRank(packet)',
             HistoricalDiscoveryPacketCandidateOccurrenceDebtRank(packet)>>
            \in HistoricalDiscoveryOccurrenceDebtOrdering
       \/ HistoricalDiscoveryCandidateExitReplenishmentResidual(packet)
       \/ HistoricalDiscoveryCandidateDuplicateOccurrenceResidual(packet)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryPacketOccurrenceDebtRanksInCarrier,
   FS_CardinalityType, SMT
   DEF HistoricalDiscoveryCandidateExitReplenishmentResidual,
       HistoricalDiscoveryCandidateDuplicateOccurrenceResidual,
       HistoricalDiscoveryPacketCandidateOccurrenceDebtRank,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       LexPairOrdering, OpToRel

THEOREM HistoricalDiscoveryServeExitEitherLowersOrReplenishes ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryServeMinimumExitStep(packet)
    => \/ <<HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)',
             HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)>>
            \in HistoricalDiscoveryOccurrenceDebtOrdering
       \/ HistoricalDiscoveryServeExitReplenishmentResidual(packet)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryPacketOccurrenceDebtRanksInCarrier,
   FS_CardinalityType, SMT
   DEF HistoricalDiscoveryServeExitReplenishmentResidual,
       HistoricalDiscoveryPacketServeOccurrenceDebtRank,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       LexPairOrdering, OpToRel

THEOREM HistoricalDiscoveryServeHeadFairServiceRemovesExactOccurrence ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryPacketServeHeadIsOwner(packet)
    /\ HistoricalDiscoveryPacketServeDebtFairAction(packet)
    => LET recipient == packet.item.envelope.recipient
           head == Head(asyncIoQueues[recipient])
           selected ==
             HistoricalDiscoveryPacketServeDebtWitness(packet)
       IN /\ selected = head
          /\ head
               \in HistoricalDiscoveryPacketServeOwners(packet)
          /\ head
               \notin HistoricalDiscoveryPacketServeOwners(packet)'
          /\ HistoricalDiscoveryPacketServeOwners(packet)' =
               HistoricalDiscoveryPacketServeOwners(packet) \ {head}
          /\ HistoricalDiscoveryServeExactNoRefillExitStep(packet)
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryServeHeadOwnerIsExactMinimumWitness,
   HistoricalDiscoveryPacketServeRanksHaveConcreteBound,
   TailRemovesUniqueServeOccurrence,
   SequenceSetHeadTailDecomposition, Isa
   DEF HistoricalDiscoveryPacketServeHeadIsOwner,
       HistoricalDiscoveryPacketServeDebtFairAction,
       HistoricalDiscoveryServeExactNoRefillExitStep,
       HistoricalDiscoveryServeMinimumExitStep,
       HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryServeJobOwned,
       HistoricalDiscoveryIoOwners,
       ActiveIoJobs, PostGstServiceIoWorker,
       PostGstServiceHistoricalRecoveryIoWorker,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, AsyncNonRunnerOuterFrame,
       AsyncNonCrashOuterFrame, AsyncCoreOuterFrame,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       SequenceSet, vars

THEOREM HistoricalDiscoveryServeHeadFairServiceLowersOccurrenceDebt ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryPacketServeHeadIsOwner(packet)
    /\ HistoricalDiscoveryPacketServeDebtFairAction(packet)
    => <<HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)',
          HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)>>
         \in HistoricalDiscoveryOccurrenceDebtOrdering
BY HistoricalDiscoveryServeHeadFairServiceRemovesExactOccurrence,
   HistoricalDiscoveryServeExactRemovalLowersOccurrenceDebt

THEOREM HistoricalDiscoveryServeNonOwnerHeadFairServiceLowersMinimum ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryPacketServeHeadIsNotOwner(packet)
    /\ HistoricalDiscoveryPacketServeDebtFairAction(packet)
    => /\ HistoricalDiscoveryPacketServeOwners(packet)' =
             HistoricalDiscoveryPacketServeOwners(packet)
       /\ <<HistoricalDiscoveryPacketServeDebtRank(packet)',
             HistoricalDiscoveryPacketServeDebtRank(packet)>>
            \in OwnedServiceRankOrdering
       /\ <<HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)',
             HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)>>
            \in HistoricalDiscoveryOccurrenceDebtOrdering
BY AsyncBracketNextPreservesStrongTypeInvariant,
   HistoricalDiscoveryLiveServeDebtHasExactFairOwner,
   HistoricalDiscoveryPacketOccurrenceDebtRanksInCarrier,
   HistoricalDiscoveryLowerOwnedRankForcesMinimumDescent,
   ServeOccurrenceIndexAfterNonTargetHead,
   ServeJobIndexMatchesOccurrenceIndex,
   SequenceSetHeadTailDecomposition, FS_CardinalityType, Isa
   DEF HistoricalDiscoveryPacketServeHeadIsNotOwner,
       HistoricalDiscoveryPacketServeDebtFairAction,
       HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryPacketServeRanks,
       HistoricalDiscoveryPacketServeDebtRank,
       HistoricalDiscoveryPacketServeOccurrenceDebtRank,
       HistoricalDiscoveryServeJobOwned,
       HistoricalDiscoveryIoOwners,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       ActiveIoJobs, ServeJobRank,
       PostGstServiceIoWorker,
       PostGstServiceHistoricalRecoveryIoWorker,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, AsyncNonRunnerOuterFrame,
       AsyncNonCrashOuterFrame, AsyncCoreOuterFrame,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       LexPairOrdering, OpToRel, SequenceSet, vars

THEOREM HistoricalDiscoveryServeFairActionLowersOccurrenceDebt ==
  \A packet:
    /\ packet \in OverdueResponsivePackets
    /\ packet \in OverdueResponsivePackets'
    /\ AsyncStrongTypeInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalDiscoveryPacketServeOwners(packet) # {}
    /\ HistoricalDiscoveryPacketServeDebtFairAction(packet)
    => <<HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)',
          HistoricalDiscoveryPacketServeOccurrenceDebtRank(packet)>>
         \in HistoricalDiscoveryOccurrenceDebtOrdering
BY HistoricalDiscoveryServeHeadFairServiceLowersOccurrenceDebt,
   HistoricalDiscoveryServeNonOwnerHeadFairServiceLowersMinimum, Isa
   DEF HistoricalDiscoveryPacketServeHeadIsOwner,
       HistoricalDiscoveryPacketServeHeadIsNotOwner,
       HistoricalDiscoveryServeExactNoRefillExitStep,
       HistoricalDiscoveryServeMinimumExitStep

(***************************************************************************
Exact residual action edges.

The request/response/control publisher surface and all four direct singleton
fault injectors are closed through the generic `PacketForItem` /
`PacketsForItems` frame.

For selected-packet service, the exact-head removal, composite non-overdue
shadow dependency descent, and arbitrary due-head global-debt descent are
proved.  The composite rank also exposes the minimal exact historical
candidate and Serve witnesses.  Their retained-packet one-step minima are now
classified into persistence, exact-owner exit, retained-owner rank change,
and lower reselection.  Lower insertion gives genuine tail descent; arbitrary
exit/reselection deliberately remains non-monotone by
`HistoricalDiscoveryPlainMinimumRemovalCanIncrease`.
The count-first refinement now records distinct equal-rank candidate owners
and, under `AsyncProgressOwnershipInvariant`, gives their exact
scheduler-owner count.  Serve nonces make the Serve cardinality exact
directly.  It proves no-refill logical-owner removal and the concrete Serve
fair worker strictly descending without an abstract multiset theorem.  Runner
steps which replace one parent by up to three fresh causal children remain in
`HistoricalDiscoveryCandidateExitReplenishmentResidual`; collapsed identical
values outside the ownership invariant remain separately in
`HistoricalDiscoveryCandidateDuplicateOccurrenceResidual`.
`HistoricalDiscoveryPacketProducerResidual` collects packet-local candidate
or Serve insertion and nondecreasing replacement.  Closing it requires an
earlier well-founded producer budget; the existing per-action fanout bound
does not bound total production, so no candidate temporal descent is claimed.
The remaining ingress work is not packet removal: blocked capacity,
timeout-byte, shared-completion, claim, runner, auxiliary, Stage-4,
historical candidate, and historical Serve owners still require their
individual no-refill/descent action lemmas.
***************************************************************************)

=============================================================================
