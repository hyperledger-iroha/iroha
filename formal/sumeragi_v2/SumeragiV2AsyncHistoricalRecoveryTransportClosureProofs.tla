---- MODULE SumeragiV2AsyncHistoricalRecoveryTransportClosureProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryLivenessProofs,
        SumeragiV2ExactDecisionStageServiceClosureProofs,
        SumeragiV2AsyncDecisionApplicationProofs

(***************************************************************************
Exact historical-recovery transport closure.

This leaf separates the requester, the archive which serves an immutable
artifact, and the aggregate outer relay used by a response.  In particular,
a CommitCertificateResponse has `AsyncUntrustedSource` as its transport
source; that value is never treated as the archive which owns the applied
Commit receipt.  The exact request record carries that server identity.

The physical ownership predicates below use only:

  * the active-request registry;
  * a concrete packet in `asyncTransport`;
  * a concrete per-source ingress occurrence with its immutable local
    admission ordinal;
  * the exact logical Serve lifecycle reservation and, when materialized, its
    identity-bound FIFO occurrence;
  * the retained terminal lifecycle tombstone and cached response outputs;
  * a concrete response packet/ingress occurrence; or
  * the exact current-consumer DeliverQC / FetchCertifiedBody owner.

`asyncSentItems` appears only where the model rechecks immutable
authentication history.  It is append-only evidence, not a physical
retention owner.

Every transport temporal operator is parameterized by an arbitrary
specification.  The proved reductions therefore remain reusable after
instantiating this module inside one indexed chain context.  The two
`AsyncSpecAt` corollaries below establish safety invariants only; no temporal
transport theorem derives its fairness from the aggregate one-height
specification, assumes that every responsive node is already joined, or
derives a historical-target runner from current-voter ownership.
***************************************************************************)

(***************************************************************************
Exact Commit-certificate request identity and fanout completeness.

The request's view is deliberately not compared with the requester's current
`nodeView`: discovery freezes the exact request occurrence, while the target
may later advance its pacemaker.  The immutable occurrence variable retains
that old view without reconstructing the request from mutable state.
***************************************************************************)

HistoricalCommitRequestOccurrence(target, server, request) ==
  /\ target \in Responsive
  /\ server \in CurrentVoters \ {target}
  /\ request \in AsyncNetworkItems
  /\ request.kind = "CommitCertificateRequest"
  /\ request.source = target
  /\ request.envelope.recipient = server
  /\ request.envelope.height = context.height
  /\ request.envelope.subject = AsyncHeartbeatSubject
  /\ request.envelope.chunk = NoAsyncChunk
  /\ request.envelope.nonce = 0

HistoricalCommitRequestRegistered(target, server, request) ==
  /\ gst
  /\ HistoricalRecoveryTarget(target)
  /\ ~NodeHasDecision(target)
  /\ ~NodeHasApplication(target)
  /\ HistoricalCommitRequestOccurrence(target, server, request)
  /\ request \in asyncActiveRequests

HistoricalCommitCertificateRequestSetComplete(target) ==
  /\ ActiveCommitCertificateRequests(target) # {}
  /\ \A server \in CurrentVoters \ {target}:
       \E request \in ActiveCommitCertificateRequests(target):
         HistoricalCommitRequestOccurrence(target, server, request)

HistoricalCommitCertificateRequestCompletenessInvariant ==
  \A target \in Responsive:
    /\ gst
    /\ HistoricalRecoveryTarget(target)
    /\ ~NodeHasDecision(target)
    /\ ~NodeHasApplication(target)
    /\ ActiveCommitCertificateRequests(target) # {}
    => HistoricalCommitCertificateRequestSetComplete(target)

THEOREM AsyncInitEstablishesHistoricalCommitRequestCompleteness ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => HistoricalCommitCertificateRequestCompletenessInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       HistoricalCommitCertificateRequestCompletenessInvariant,
       HistoricalCommitCertificateRequestSetComplete,
       HistoricalCommitRequestOccurrence,
       ActiveCommitCertificateRequests, HistoricalRecoveryTarget

(***************************************************************************
Publication is the only action which creates a Commit-certificate
registration.  It registers the whole canonical fanout.  A matching
authenticated response retires the complete registration identity
atomically while installing DeliverQC; restart reset can remove a request
only before GST and also removes the historical target.  All other actions
retain every active occurrence.
***************************************************************************)

THEOREM AsyncNextPreservesHistoricalCommitRequestCompleteness ==
  /\ AsyncStrongTypeInvariant
  /\ HistoricalCommitCertificateRequestCompletenessInvariant
  /\ [AsyncNext]_AsyncAllVars
  => HistoricalCommitCertificateRequestCompletenessInvariant'
BY GstAsyncStepIsMonotone,
   HistoricalRecoveryTargetPersistsUnlessDecision,
   IsaT(600)
   DEF HistoricalCommitCertificateRequestCompletenessInvariant,
       HistoricalCommitCertificateRequestSetComplete,
       HistoricalCommitRequestOccurrence,
       ActiveCommitCertificateRequests,
       CommitCertificateRequestOutbox,
       PublishCommitCertificateRequests,
       MatchingCommitCertificateRequests,
       DrainFairIngressSelected,
       PersistInstalledControlAfterInstall,
       PersistDecisionControl,
       RetireCompletedBodyCertifiedResponseAuthority,
       FilterCertifiedResponseAuthority,
       ResetNodeSchedulerForRestart,
       OpenHistoricalRecovery,
       ExecuteApply,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RunHistoricalServer,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstCrash,
       PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep, FifoRuntimeStep,
       DeferredDrainStep, DeferredTagStep,
       ExecuteCommand, ExecuteRegularCommand,
       AsyncAllVars

THEOREM AsyncBracketPreservesHistoricalCommitRequestCompleteness ==
  /\ AsyncStrongTypeInvariant
  /\ HistoricalCommitCertificateRequestCompletenessInvariant
  /\ [AsyncNext]_AsyncAllVars
  => HistoricalCommitCertificateRequestCompletenessInvariant'
BY AsyncNextPreservesHistoricalCommitRequestCompleteness

THEOREM HistoricalCommitRequestCompletenessObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []HistoricalCommitCertificateRequestCompletenessInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []HistoricalCommitCertificateRequestCompletenessInvariant
    <2>1. AsyncInitAt(initialContext)
            => HistoricalCommitCertificateRequestCompletenessInvariant
      BY AsyncInitEstablishesHistoricalCommitRequestCompleteness
    <2>2. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>3. /\ AsyncStrongTypeInvariant
           /\ HistoricalCommitCertificateRequestCompletenessInvariant
           /\ [AsyncNext]_AsyncAllVars
          => HistoricalCommitCertificateRequestCompletenessInvariant'
      BY AsyncBracketPreservesHistoricalCommitRequestCompleteness
    <2> QED BY <2>1, <2>2, <2>3, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

HistoricalCommitRequestCompletenessProperty(specification) ==
  specification
    => []HistoricalCommitCertificateRequestCompletenessInvariant

(***************************************************************************
One exact applied archive route.

The Async open guard proves that some responsive archive has an applied
receipt, and request publication proves a nonempty current-voter fanout.
Those two facts do not, in this one-height model, prove that the applied
archive belongs to that fanout.  The indexed chain source record does carry
the missing exact server identity.  Keep that seam explicit instead of
silently choosing unrelated members of the two nonempty sets.

The indexed chain wrapper already carries the minimal sound repair:
`IndexedHistoricalRecoverySourceReady` selects an exact server in
`AsyncCurrentResponsiveVoters` whose exact source record is also a current
application.  Instantiating this predicate at `IndexedOpenHistoricalRecovery`
therefore establishes the route below, and monotone application/up/context
state can preserve it until the target exits.  No production fanout change is
needed for that bridge.  If this one-height model were used without the
indexed wrapper, the smaller model repair would be to strengthen
`HistoricalRecoverySourceReady` to require precisely that intersection;
broadening Commit-certificate fanout to all authenticated archive peers would
instead be a protocol change.
***************************************************************************)

HistoricalCommitArchiveRouteAvailable(target, server) ==
  /\ gst
  /\ HistoricalRecoveryTarget(target)
  /\ server \in (CurrentVoters \ {target})
                 \cap AsyncResponsiveAppliedArchiveServers

HistoricalCommitArchiveRouteAvailabilityInvariant ==
  \A target \in Responsive:
    /\ gst
    /\ HistoricalRecoveryTarget(target)
    /\ ~NodeHasDecision(target)
    /\ ~NodeHasApplication(target)
    /\ ActiveCommitCertificateRequests(target) # {}
    => \E server:
         HistoricalCommitArchiveRouteAvailable(target, server)

HistoricalCommitArchiveRouteAvailabilityProperty(specification) ==
  specification => []HistoricalCommitArchiveRouteAvailabilityInvariant

THEOREM CompleteHistoricalCommitFanoutSelectsExactAppliedRoute ==
  \A target \in Responsive:
    /\ HistoricalCommitCertificateRequestCompletenessInvariant
    /\ HistoricalCommitArchiveRouteAvailabilityInvariant
    /\ gst
    /\ HistoricalRecoveryTarget(target)
    /\ ~NodeHasDecision(target)
    /\ ~NodeHasApplication(target)
    /\ ActiveCommitCertificateRequests(target) # {}
    => \E server, request:
         /\ HistoricalCommitArchiveRouteAvailable(target, server)
         /\ HistoricalCommitRequestRegistered(target, server, request)
BY Isa
   DEF HistoricalCommitCertificateRequestCompletenessInvariant,
       HistoricalCommitCertificateRequestSetComplete,
       HistoricalCommitArchiveRouteAvailabilityInvariant,
       HistoricalCommitArchiveRouteAvailable,
       HistoricalCommitRequestRegistered,
       ActiveCommitCertificateRequests

(***************************************************************************
Exact physical request/response owners.

Packet admission atomically allocates the route-local logical Serve identity,
an immutable lifecycle ordinal, and an immutable ingress ordinal.  The FIFO
nonce is only the physical occurrence tag after that reservation materializes;
it is never the logical owner.  A drained identity is retained as a tombstone
with its exact cached response, so retransmission coalesces into the same
monotone lifecycle and cannot resurrect the old Serve stage.
***************************************************************************)

HistoricalCommitRequestPacketOwned(target, server, request, packet) ==
  /\ HistoricalCommitRequestRegistered(target, server, request)
  /\ packet \in asyncTransport
  /\ packet.item = request

HistoricalCommitServeLifecycleIdentity(server, request) ==
  AsyncServeLogicalRequestIdentity(server, request)

HistoricalCommitRequestIngressOwned(target, server, request) ==
  LET identity == HistoricalCommitServeLifecycleIdentity(server, request)
  IN /\ HistoricalCommitRequestRegistered(target, server, request)
     /\ request \in
          SequenceSet(
            IngressLane(server, IngressResourceSource(request)))
     /\ AsyncServeIngressAdmissionOwned(server, identity)
     /\ AsyncServeIngressAdmissionOrdinal(server, identity) \in Nat \ {0}

HistoricalCommitServeAdmissionOwned(server, request) ==
  LET identity == HistoricalCommitServeLifecycleIdentity(server, request)
  IN /\ AsyncServeLiveReservationOwned(server, identity)
     /\ AsyncServeAdmissionOrdinal(server, identity) \in Nat \ {0}

HistoricalCommitServeOccurrenceOwned(server, request, job) ==
  LET identity == HistoricalCommitServeLifecycleIdentity(server, request)
  IN /\ HistoricalCommitServeAdmissionOwned(server, request)
     /\ AsyncServeJobQueued(server, identity)
     /\ job \in SequenceSet(asyncIoQueues[server])
     /\ job.class = "Serve"
     /\ job.candidate.item = request
     /\ AsyncIoServeJobIdentity(server, job) = identity
     /\ job.nonce \in 0..AsyncIoAuxCapacity

HistoricalCommitServeJobOwned(target, server, request, job) ==
  /\ HistoricalCommitRequestRegistered(target, server, request)
  /\ HistoricalCommitArchiveRouteAvailable(target, server)
  /\ HistoricalCommitServeOccurrenceOwned(server, request, job)

HistoricalCommitResponseLineage(
    target, server, request, qc, response) ==
  /\ HistoricalCommitRequestOccurrence(target, server, request)
  /\ AppliedDecisionCertificateAuthority(server, qc)
  /\ response = CommitCertificateResponseItem(request, qc)
  /\ response.kind = "CommitCertificateResponse"
  /\ response.source = AsyncUntrustedSource
  /\ response.envelope.recipient = target
  /\ response.envelope.request = request
  /\ response.envelope.qc = qc

HistoricalCommitServeTombstoneOwned(target, server, request) ==
  LET identity == HistoricalCommitServeLifecycleIdentity(server, request)
  IN /\ HistoricalCommitRequestRegistered(target, server, request)
     /\ HistoricalCommitArchiveRouteAvailable(target, server)
     /\ AsyncServeLifecycleTombstone(server, identity)
     /\ AsyncServeTombstoneOutputs(server, identity) # {}
     /\ \A response \in AsyncServeTombstoneOutputs(server, identity):
          \E qc:
            HistoricalCommitResponseLineage(
              target, server, request, qc, response)

HistoricalCommitResponsePacketOwned(
    target, server, request, qc, response, packet) ==
  /\ HistoricalCommitRequestRegistered(target, server, request)
  /\ HistoricalCommitResponseLineage(
       target, server, request, qc, response)
  /\ response \in asyncSentItems
  /\ packet \in asyncTransport
  /\ packet.item = response

HistoricalCommitResponseIngressOwned(
    target, server, request, qc, response) ==
  /\ HistoricalCommitRequestRegistered(target, server, request)
  /\ HistoricalCommitResponseLineage(
       target, server, request, qc, response)
  /\ response \in asyncSentItems
  /\ response \in
       SequenceSet(
         IngressLane(target, IngressResourceSource(response)))

HistoricalCommitDeliverQcOwner(
    target, server, request, qc, response) ==
  LET candidate == CommitCertificateResponseCandidate(response)
  IN /\ HistoricalCommitResponseLineage(
          target, server, request, qc, response)
     /\ gst
     /\ HistoricalRecoveryTarget(target)
     /\ ~NodeHasApplication(target)
     /\ candidate \in AsyncCandidateSet
     /\ candidate.node = target
     /\ candidate.kind = "DeliverQC"
     /\ candidate.item = DiscoveredCommitQcItem(response)
     /\ candidate.evidence = response
     /\ CandidateConsumerCurrent(candidate)
     /\ HistoricalProtectedCandidateOwned(candidate)

HistoricalCommitTransportGoal(target) ==
  \/ NodeHasDecision(target)
  \/ NodeHasApplication(target)
  \/ ~HistoricalRecoveryTarget(target)
  \/ \E server, request, qc, response:
       HistoricalCommitDeliverQcOwner(
         target, server, request, qc, response)

(***************************************************************************
Concrete action-local handoffs.  None of these theorems assumes fairness.
They prove what the exact production-model action does after a scheduler has
selected it.
***************************************************************************)

THEOREM HistoricalCommitRetransmissionCreatesExactPacket ==
  \A target, server, request:
    /\ HistoricalCommitRequestRegistered(target, server, request)
    /\ SendNodeRetransmissions(target)
    => \E packet:
         HistoricalCommitRequestPacketOwned(
           target, server, request, packet)'
BY Isa
   DEF HistoricalCommitRequestPacketOwned,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       SendNodeRetransmissions, RetryableItems,
       ActiveRequestItems, PacketsForItems, PacketForItem

THEOREM HistoricalCommitPacketAdmissionCreatesExactIngressOwner ==
  \A target, server, request, packet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitRequestPacketOwned(
         target, server, request, packet)
    /\ packet = OldestDueSourcePacket(server, request.source)
    /\ AdmitIngressPacket(server, request.source)
    => HistoricalCommitRequestIngressOwned(
         target, server, request)'
BY IsaT(240)
   DEF HistoricalCommitRequestPacketOwned,
       HistoricalCommitRequestIngressOwned,
       HistoricalCommitServeLifecycleIdentity,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       CommitCertificateRequestAuthorized,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       AcceptOrReserveExactServeIngress,
       ReserveExactServeCapacity, AdvanceExactServeCapacity,
       CoalesceExactServeIngressCapacity,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionOrdinal,
       DropPolicyRejectedHiddenPacket,
       IngressPacketPolicyRejected,
       CertifiedResponsePacketPolicyRejected,
       UntrustedGenericCompletionPacketPolicyRejected,
       IngressResourceSource, IngressLane, SequenceSet,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       AsyncActiveRequestsType

THEOREM HistoricalCommitIngressCreatesFreshServeOwner ==
  \A target, server, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitArchiveRouteAvailable(target, server)
    /\ HistoricalCommitRequestIngressOwned(target, server, request)
    /\ HistoricalCommitServeAdmissionOwned(server, request)
    /\ HistoricalSelectedIngressItemAt(
         server,
         FirstHistoricalDrainableIngressIndex(server)) = request
    /\ DrainHistoricalIngressSelected(server)
    => \E job \in SequenceSet(asyncIoQueues'[server]):
         HistoricalCommitServeJobOwned(
           target, server, request, job)'
BY FreshAsyncIoServeNonceFacts,
   TypedRequestMakesTypedServeJob, IsaT(240)
   DEF HistoricalCommitRequestIngressOwned,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       HistoricalCommitArchiveRouteAvailable,
       HistoricalCommitServeJobOwned,
       HistoricalCommitServeOccurrenceOwned,
       HistoricalCommitServeAdmissionOwned,
       HistoricalCommitServeLifecycleIdentity,
       DrainHistoricalIngressSelected,
       AsyncIoCertifiedServeJob,
       CommitCertificateRequestAuthorized,
       FreshAsyncIoServeNonce, SequenceSet,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       AsyncActiveRequestsType

THEOREM HistoricalCommitCachedIngressCreatesExactResponsePacket ==
  \A target, server, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitRequestIngressOwned(target, server, request)
    /\ HistoricalCommitServeTombstoneOwned(target, server, request)
    /\ HistoricalSelectedIngressItemAt(
         server,
         FirstHistoricalDrainableIngressIndex(server)) = request
    /\ DrainHistoricalIngressSelected(server)
    => \E qc, response, packet:
         HistoricalCommitResponsePacketOwned(
           target, server, request, qc, response, packet)'
BY IsaT(300)
   DEF HistoricalCommitRequestIngressOwned,
       HistoricalCommitServeTombstoneOwned,
       HistoricalCommitServeLifecycleIdentity,
       HistoricalCommitResponsePacketOwned,
       HistoricalCommitResponseLineage,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       AsyncServeCachedReplayItems,
       AsyncServeTombstoneOutputs,
       DrainHistoricalIngressSelected,
       PublishEphemeralItems, PacketsForItems,
       IngressResourceSource, IngressLane, SequenceSet

THEOREM HistoricalCommitIngressCreatesLifecycleOutcome ==
  \A target, server, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitArchiveRouteAvailable(target, server)
    /\ HistoricalCommitRequestIngressOwned(target, server, request)
    /\ HistoricalSelectedIngressItemAt(
         server,
         FirstHistoricalDrainableIngressIndex(server)) = request
    /\ DrainHistoricalIngressSelected(server)
    => \/ \E job \in SequenceSet(asyncIoQueues'[server]):
              HistoricalCommitServeJobOwned(
                target, server, request, job)'
       \/ \E qc, response, packet:
              HistoricalCommitResponsePacketOwned(
                target, server, request, qc, response, packet)'
BY HistoricalCommitIngressCreatesFreshServeOwner,
   HistoricalCommitCachedIngressCreatesExactResponsePacket, IsaT(240)
   DEF HistoricalCommitRequestIngressOwned,
       HistoricalCommitServeAdmissionOwned,
       HistoricalCommitServeTombstoneOwned,
       HistoricalCommitServeLifecycleIdentity,
       AsyncServeLifecyclePartitionInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncServeLifecycleTypeInvariant

THEOREM HistoricalCommitServeJobUsesOrdinaryArchiveIoOwner ==
  \A target, server, request, job:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitServeJobOwned(
         target, server, request, job)
    => /\ server \in AsyncArchiveIoServiceNodes
       /\ job \in AsyncServeJobSet
       /\ ResponsiveProtectedServeJobOwned(server, job)
BY TypedCandidateIsInCarrier, IsaT(180)
   DEF HistoricalCommitServeJobOwned,
       HistoricalCommitServeOccurrenceOwned,
       HistoricalCommitServeAdmissionOwned,
       HistoricalCommitServeLifecycleIdentity,
       HistoricalCommitArchiveRouteAvailable,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       ResponsiveProtectedServeJobOwned,
       AsyncServeJobSet, AsyncIoJob,
       AsyncArchiveIoServiceNodes,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant,
       AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant,
       AsyncIoSequenceTyped, AsyncIoJobTyped,
       SequenceSet

THEOREM HistoricalCommitServeHeadCreatesExactResponsePacket ==
  \A target, server, request, job:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitServeJobOwned(
         target, server, request, job)
    /\ Head(asyncIoQueues[server]) = job
    /\ ServiceIoWorkerWork(server)
    => \E qc, response, packet:
         HistoricalCommitResponsePacketOwned(
           target, server, request, qc, response, packet)'
BY IsaT(300)
   DEF HistoricalCommitServeJobOwned,
       HistoricalCommitServeOccurrenceOwned,
       HistoricalCommitServeAdmissionOwned,
       HistoricalCommitServeLifecycleIdentity,
       HistoricalCommitArchiveRouteAvailable,
       HistoricalCommitResponsePacketOwned,
       HistoricalCommitResponseLineage,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       AppliedDecisionCertificateAuthority,
       ServiceIoWorkerWork,
       CommitCertificateServeCanRespond,
       CommitCertificateServiceApplication,
       CommitCertificateResponseItems,
       CommitCertificateResponseItem,
       PublishEphemeralItems, PacketsForItems,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       AsyncActiveRequestsType

THEOREM HistoricalCommitAdmittedServeInstallsExactTombstone ==
  \A target, server, request, job:
    LET identity == HistoricalCommitServeLifecycleIdentity(server, request)
    IN /\ AsyncStrongTypeInvariant
       /\ HistoricalCommitServeJobOwned(
            target, server, request, job)
       /\ Head(asyncIoQueues[server]) = job
       /\ ServiceIoWorkerWork(server)
       => /\ AsyncServeLifecycleTombstone(server, identity)'
          /\ AsyncServeTombstoneOutputs(server, identity)' # {}
          /\ ~AsyncServeLiveReservationOwned(server, identity)'
          /\ ~AsyncServeJobQueued(server, identity)'
BY HistoricalCommitServeHeadCreatesExactResponsePacket, IsaT(240)
   DEF HistoricalCommitServeJobOwned,
       HistoricalCommitServeOccurrenceOwned,
       HistoricalCommitServeAdmissionOwned,
       HistoricalCommitServeLifecycleIdentity,
       ServiceIoWorkerWork,
       AsyncServeLifecycleTombstone,
       AsyncServeTombstoneOutputs,
       AsyncServeTombstoneRecords,
       AsyncServeReservationRecord,
       AsyncServeTombstonesWithoutFamily,
       AsyncServeTombstone,
       AsyncServeLiveReservationOwned,
       AsyncServeJobQueued

THEOREM HistoricalCommitServeOrdinalIsImmutableUntilTerminalExit ==
  \A server, request:
    LET identity == HistoricalCommitServeLifecycleIdentity(server, request)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncServeLifecycleOwned(server, identity)
       /\ AsyncNext
       /\ AsyncServeLifecycleOwned(server, identity)'
       => AsyncServeAdmissionOrdinal(server, identity)'
            = AsyncServeAdmissionOrdinal(server, identity)
BY IsaT(300)
   DEF HistoricalCommitServeLifecycleIdentity,
       AsyncServeAdmissionOrdinal,
       AsyncServeLifecycleOwned,
       AsyncServeLiveReservationOwned,
       AsyncServeLifecycleTombstone,
       AsyncServeReservationRecord,
       AsyncServeTombstoneRecord,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart

THEOREM HistoricalCommitServeTombstoneCannotResurrectAtGst ==
  \A server, request:
    LET identity == HistoricalCommitServeLifecycleIdentity(server, request)
    IN /\ AsyncStrongTypeInvariant
       /\ gst
       /\ AsyncServeLifecycleTombstone(server, identity)
       /\ [AsyncNext]_AsyncAllVars
       => /\ AsyncServeLogicalIdentityRetiredOrSuperseded(
                server, identity)'
          /\ ~AsyncServeJobQueued(server, identity)'
BY AsyncServeRetiredIdentityCannotRequeueAtGst, Isa
   DEF HistoricalCommitServeLifecycleIdentity,
       AsyncServeLogicalIdentityRetiredOrSuperseded,
       AsyncServeLifecycleOwned,
       AsyncServeLifecycleTombstone,
       AsyncAllVars

THEOREM HistoricalCommitResponsePacketAdmissionCreatesIngressOwner ==
  \A target, server, request, qc, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitResponsePacketOwned(
         target, server, request, qc, response, packet)
    /\ packet = OldestDueSourcePacket(target, response.source)
    /\ AdmitIngressPacket(target, response.source)
    => HistoricalCommitResponseIngressOwned(
         target, server, request, qc, response)'
BY IsaT(300)
   DEF HistoricalCommitResponsePacketOwned,
       HistoricalCommitResponseIngressOwned,
       HistoricalCommitResponseLineage,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       CommitCertificateRequestAuthorized,
       CommitCertificateResponseAuthorized,
       MatchingCommitCertificateRequests,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       IngressPacketPolicyRejected,
       CertifiedResponsePacketPolicyRejected,
       UntrustedGenericCompletionPacketPolicyRejected,
       IngressResourceSource, IngressLane, SequenceSet,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       AsyncActiveRequestsType

THEOREM HistoricalCommitResponseIngressCreatesExactDeliverQcOwner ==
  \A target, server, request, qc, response:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ HistoricalCommitResponseIngressOwned(
         target, server, request, qc, response)
    /\ SelectedIngressItemAt(
         target, FirstDrainableIngressIndex(target)) = response
    /\ DrainFairIngressSelected(target)
    => HistoricalCommitDeliverQcOwner(
         target, server, request, qc, response)'
BY IsaT(360)
   DEF HistoricalCommitResponseIngressOwned,
       HistoricalCommitResponseLineage,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       HistoricalCommitDeliverQcOwner,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CommitCertificateRequestAuthorized,
       CommitCertificateResponseAuthorized,
       MatchingCommitCertificateRequests,
       CommitCertificateResponseCandidate,
       DiscoveredCommitQcItem,
       DrainFairIngressSelected,
       EnqueueCandidate, CandidateScheduled,
       CandidateConsumerCurrent,
       IngressResourceSource, IngressLane, SequenceSet,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       AsyncActiveRequestsType

(***************************************************************************
Occurrence retention.

These are one-step safety theorems, not scheduler claims.  A concrete packet,
ingress entry, or Serve job cannot disappear after GST without its exact next
owner or a semantic exit.  The append-only authentication history is never an
alternative arm.
***************************************************************************)

THEOREM HistoricalCommitRegisteredOwnerPersistsUntilDeliverOrExit ==
  \A target, server, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitRequestRegistered(target, server, request)
    /\ ~HistoricalCommitTransportGoal(target)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalCommitRequestRegistered(
            target, server, request)'
       \/ HistoricalCommitTransportGoal(target)'
BY HistoricalRecoveryTargetPersistsUnlessDecision,
   GstAsyncStepIsMonotone, IsaT(600)
   DEF HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       HistoricalCommitTransportGoal,
       HistoricalCommitDeliverQcOwner,
       HistoricalCommitResponseLineage,
       MatchingCommitCertificateRequests,
       DrainFairIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncAllVars

THEOREM HistoricalCommitPacketOwnerPersistsOrHandsOff ==
  \A target, server, request, packet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitRequestPacketOwned(
         target, server, request, packet)
    /\ ~HistoricalCommitTransportGoal(target)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalCommitRequestPacketOwned(
            target, server, request, packet)'
       \/ HistoricalCommitRequestIngressOwned(
            target, server, request)'
       \/ HistoricalCommitTransportGoal(target)'
BY HistoricalCommitPacketAdmissionCreatesExactIngressOwner,
   HistoricalRecoveryTargetPersistsUnlessDecision,
   GstAsyncStepIsMonotone, IsaT(600)
   DEF HistoricalCommitRequestPacketOwned,
       HistoricalCommitRequestIngressOwned,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       HistoricalCommitTransportGoal,
       HistoricalCommitDeliverQcOwner,
       HistoricalCommitResponseLineage,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstLosePacket,
       DrainFairIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply, AsyncAllVars

THEOREM HistoricalCommitIngressOwnerPersistsOrHandsOff ==
  \A target, server, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitArchiveRouteAvailable(target, server)
    /\ HistoricalCommitRequestIngressOwned(target, server, request)
    /\ ~HistoricalCommitTransportGoal(target)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalCommitRequestIngressOwned(
            target, server, request)'
       \/ \E job:
            HistoricalCommitServeJobOwned(
              target, server, request, job)'
       \/ \E qc, response, packet:
            HistoricalCommitResponsePacketOwned(
              target, server, request, qc, response, packet)'
       \/ HistoricalCommitTransportGoal(target)'
BY HistoricalCommitIngressCreatesLifecycleOutcome,
   HistoricalRecoveryTargetPersistsUnlessDecision,
   GstAsyncStepIsMonotone, IsaT(600)
   DEF HistoricalCommitRequestIngressOwned,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       HistoricalCommitArchiveRouteAvailable,
       HistoricalCommitServeJobOwned,
       HistoricalCommitServeOccurrenceOwned,
       HistoricalCommitServeAdmissionOwned,
       HistoricalCommitServeTombstoneOwned,
       HistoricalCommitServeLifecycleIdentity,
       HistoricalCommitResponsePacketOwned,
       HistoricalCommitTransportGoal,
       HistoricalCommitDeliverQcOwner,
       HistoricalCommitResponseLineage,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RunHistoricalServer, DrainHistoricalIngressSelected,
       DrainFairIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply, AsyncAllVars

THEOREM HistoricalCommitServeOwnerPersistsOrHandsOff ==
  \A target, server, request, job:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitServeJobOwned(
         target, server, request, job)
    /\ ~HistoricalCommitTransportGoal(target)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalCommitServeJobOwned(
            target, server, request, job)'
       \/ \E qc, response, packet:
            HistoricalCommitResponsePacketOwned(
              target, server, request, qc, response, packet)'
       \/ HistoricalCommitTransportGoal(target)'
BY HistoricalCommitServeHeadCreatesExactResponsePacket,
   HistoricalRecoveryTargetPersistsUnlessDecision,
   GstAsyncStepIsMonotone, HeadTailProperties, IsaT(600)
   DEF HistoricalCommitServeJobOwned,
       HistoricalCommitServeOccurrenceOwned,
       HistoricalCommitServeAdmissionOwned,
       HistoricalCommitServeLifecycleIdentity,
       HistoricalCommitArchiveRouteAvailable,
       HistoricalCommitResponsePacketOwned,
       HistoricalCommitResponseLineage,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       HistoricalCommitTransportGoal,
       HistoricalCommitDeliverQcOwner,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       DrainFairIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply, AsyncAllVars

THEOREM HistoricalCommitResponsePacketPersistsOrHandsOff ==
  \A target, server, request, qc, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitResponsePacketOwned(
         target, server, request, qc, response, packet)
    /\ ~HistoricalCommitTransportGoal(target)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalCommitResponsePacketOwned(
            target, server, request, qc, response, packet)'
       \/ HistoricalCommitResponseIngressOwned(
            target, server, request, qc, response)'
       \/ HistoricalCommitTransportGoal(target)'
BY HistoricalCommitResponsePacketAdmissionCreatesIngressOwner,
   HistoricalRecoveryTargetPersistsUnlessDecision,
   GstAsyncStepIsMonotone, IsaT(600)
   DEF HistoricalCommitResponsePacketOwned,
       HistoricalCommitResponseIngressOwned,
       HistoricalCommitResponseLineage,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       HistoricalCommitTransportGoal,
       HistoricalCommitDeliverQcOwner,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstLosePacket,
       DrainFairIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply, AsyncAllVars

THEOREM HistoricalCommitResponseIngressPersistsOrHandsOff ==
  \A target, server, request, qc, response:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ HistoricalCommitResponseIngressOwned(
         target, server, request, qc, response)
    /\ ~HistoricalCommitTransportGoal(target)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalCommitResponseIngressOwned(
            target, server, request, qc, response)'
       \/ HistoricalCommitTransportGoal(target)'
BY HistoricalCommitResponseIngressCreatesExactDeliverQcOwner,
   HistoricalRecoveryTargetPersistsUnlessDecision,
   GstAsyncStepIsMonotone, IsaT(600)
   DEF HistoricalCommitResponseIngressOwned,
       HistoricalCommitResponseLineage,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       HistoricalCommitTransportGoal,
       HistoricalCommitDeliverQcOwner,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, DrainFairIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply, AsyncAllVars

(***************************************************************************
Fairness-parametric Commit-certificate transport reduction.

The four kernels below are deliberately smaller than the broad historical
leaf in the parent module.  Each begins at one exact physical owner.  Their
eventual scheduling requires finite deadline/source-lane and target/server
runner ranks; those ranks can later be supplied directly by the matching
indexed fairness actions without activating another one-height specification.
***************************************************************************)

HistoricalCommitRequestSource(target) ==
  /\ gst
  /\ HistoricalRecoveryTarget(target)
  /\ ~NodeHasDecision(target)
  /\ ~NodeHasApplication(target)
  /\ ActiveCommitCertificateRequests(target) # {}

HistoricalCommitRequestPacketGoal(target, server, request) ==
  \/ HistoricalCommitTransportGoal(target)
  \/ /\ HistoricalCommitArchiveRouteAvailable(target, server)
     /\ \E packet:
          HistoricalCommitRequestPacketOwned(
            target, server, request, packet)

HistoricalCommitRequestServeGoal(target, server, request) ==
  \/ HistoricalCommitTransportGoal(target)
  \/ \E job:
       HistoricalCommitServeJobOwned(
         target, server, request, job)
  \/ \E qc, response, packet:
       HistoricalCommitResponsePacketOwned(
         target, server, request, qc, response, packet)

HistoricalCommitResponsePacketGoal(target, server, request) ==
  \/ HistoricalCommitTransportGoal(target)
  \/ \E qc, response, packet:
       HistoricalCommitResponsePacketOwned(
         target, server, request, qc, response, packet)

(***************************************************************************
Exact Commit-request ingress lifecycle rank.

This is the existing selector/lane/source/runner rank under a new outer
lifecycle component.  The outer stage follows the immutable admission identity
through its reserved future slot, queued Serve occurrence, and terminal cached
response.  Its predecessor set is frozen at atomic packet admission.  Later
causal, Control, Completion, priority, and Serve producers therefore cannot be
counted as new predecessors, and a tombstone retry retains the same identity
without recreating stage 2.
***************************************************************************)

HistoricalCommitRequestLifecycleResidual(target, server, request) ==
  /\ HistoricalCommitArchiveRouteAvailable(target, server)
  /\ HistoricalCommitRequestIngressOwned(target, server, request)
  /\ ~HistoricalCommitRequestServeGoal(target, server, request)

HistoricalCommitRequestLifecycleStage(target, server, request) ==
  IF HistoricalCommitRequestServeGoal(target, server, request)
  THEN 0
  ELSE IF HistoricalCommitServeTombstoneOwned(target, server, request)
       THEN 1
       ELSE IF HistoricalCommitServeAdmissionOwned(server, request)
            THEN 2
            ELSE 3

HistoricalCommitRequestFrozenPredecessorSet(server, request) ==
  LET identity == HistoricalCommitServeLifecycleIdentity(server, request)
  IN ({"Io"} \X AsyncServeFrozenPredecessorSet(server, identity))
       \cup
     ({"Ingress"} \X
        AsyncServeIngressAdmissionPredecessorDebtSlots(server, identity))
       \cup
     AsyncServePreexistingIngressOwnerPredecessorDebtSet(server, identity)
       \cup
     AsyncServePreexistingIngressBarrierPredecessorDebtSet(server, identity)

HistoricalCommitRequestFrozenPredecessorDebt(server, request) ==
  Cardinality(
    HistoricalCommitRequestFrozenPredecessorSet(server, request))

HistoricalCommitRequestNestedIngressRank(target, server, request) ==
  IF HistoricalCommitRequestLifecycleResidual(target, server, request)
  THEN ExactDecisionRequestIngressRank(server, request)
  ELSE ExactDecisionRequestIngressZeroRank

HistoricalCommitRequestLifecycleRank(target, server, request) ==
  <<HistoricalCommitRequestLifecycleStage(target, server, request),
    <<HistoricalCommitRequestFrozenPredecessorDebt(server, request),
      HistoricalCommitRequestNestedIngressRank(
        target, server, request)>>>>

HistoricalCommitRequestLifecycleRankCarrier ==
  ExactDecisionRequestLifecycleIngressRankCarrier

HistoricalCommitRequestLifecycleRankOrdering ==
  ExactDecisionRequestLifecycleIngressRankOrdering

THEOREM HistoricalCommitRequestLifecycleRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    HistoricalCommitRequestLifecycleRankOrdering,
    HistoricalCommitRequestLifecycleRankCarrier)
BY ExactDecisionRequestLifecycleIngressRankOrderingIsWellFounded
   DEF HistoricalCommitRequestLifecycleRankOrdering,
       HistoricalCommitRequestLifecycleRankCarrier

THEOREM HistoricalCommitRequestLifecycleRankInCarrier ==
  \A target, server, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitRequestLifecycleResidual(
         target, server, request)
    => HistoricalCommitRequestLifecycleRank(target, server, request)
         \in HistoricalCommitRequestLifecycleRankCarrier
BY ExactDecisionRequestIngressPriorityDebtIsNatural,
   ExactDecisionRequestIngressServeCapacityDebtIsNatural,
   CandidateSequenceIndexIsPosition,
   DrainableIngressTurnReachRankIsNatural,
   FS_Union, FS_Product, FS_CardinalityType, IsaT(300)
   DEF HistoricalCommitRequestLifecycleResidual,
       HistoricalCommitRequestLifecycleRank,
       HistoricalCommitRequestLifecycleStage,
       HistoricalCommitRequestFrozenPredecessorDebt,
       HistoricalCommitRequestFrozenPredecessorSet,
       HistoricalCommitRequestNestedIngressRank,
       HistoricalCommitRequestLifecycleRankCarrier,
       HistoricalCommitRequestIngressOwned,
       HistoricalCommitServeLifecycleIdentity,
       ExactDecisionRequestLifecycleIngressRankCarrier,
       ExactDecisionRequestLifecycleDebtCarrier,
       ExactDecisionRequestIngressRank,
       ExactDecisionRequestIngressRankCarrier,
       ExactDecisionRequestIngressCapacityRank,
       ExactDecisionRequestIngressReachSelectorRank,
       ExactDecisionRequestIngressSelectorRank,
       ExactDecisionRequestIngressLaneRank,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressServeCapacityDebt,
       ExactDecisionRequestIngressPriorityDebt,
       ExactDecisionRequestIngressPriorityOwners,
       ExactDecisionRequestIngressLanePosition,
       ExactDecisionRequestIngressLaneIndices,
       ExactDecisionRequestIngressSourcePosition,
       IngressSourceServiceRank,
       ExactDecisionRequestIngressReachRank,
       ExactDecisionRequestIngressZeroRank,
       ExactDecisionRequestIngressZeroCapacityRank,
       ExactDecisionRequestIngressZeroReachSelectorRank,
       ExactDecisionRequestIngressZeroSelectorRank,
       ExactDecisionRequestIngressZeroLaneRank,
       AsyncConfiguration

HistoricalCommitRequestPacketEmissionKernelProperty(specification) ==
  specification
    => \A target, server, request:
         HistoricalCommitRequestRegistered(target, server, request)
           /\ HistoricalCommitArchiveRouteAvailable(target, server)
           ~> HistoricalCommitRequestPacketGoal(
                target, server, request)

HistoricalCommitRequestIngressKernelProperty(specification) ==
  specification
    => \A target, server, request, packet:
         HistoricalCommitRequestPacketOwned(
           target, server, request, packet)
           /\ HistoricalCommitArchiveRouteAvailable(target, server)
           ~> HistoricalCommitRequestServeGoal(
                target, server, request)

HistoricalCommitServeResponseKernelProperty(specification) ==
  specification
    => \A target, server, request, job:
         HistoricalCommitServeJobOwned(
           target, server, request, job)
           ~> HistoricalCommitResponsePacketGoal(
                target, server, request)

HistoricalCommitResponseAdmissionKernelProperty(specification) ==
  specification
    => \A target, server, request, qc, response, packet:
         HistoricalCommitResponsePacketOwned(
           target, server, request, qc, response, packet)
           ~> HistoricalCommitTransportGoal(target)

HistoricalCommitPhysicalTransportKernelProperties(specification) ==
  /\ HistoricalCommitRequestPacketEmissionKernelProperty(specification)
  /\ HistoricalCommitRequestIngressKernelProperty(specification)
  /\ HistoricalCommitServeResponseKernelProperty(specification)
  /\ HistoricalCommitResponseAdmissionKernelProperty(specification)

HistoricalCommitCertificateTransportLeaf(specification) ==
  specification
    => \A target \in Responsive:
         HistoricalCommitRequestSource(target)
           ~> HistoricalCommitTransportGoal(target)

THEOREM HistoricalCommitTransportKernelsDischargeExactLeaf ==
  \A specification:
    /\ HistoricalCommitRequestCompletenessProperty(specification)
    /\ HistoricalCommitArchiveRouteAvailabilityProperty(specification)
    /\ HistoricalCommitPhysicalTransportKernelProperties(specification)
    => HistoricalCommitCertificateTransportLeaf(specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalCommitRequestCompletenessProperty(specification),
                HistoricalCommitArchiveRouteAvailabilityProperty(
                  specification),
                HistoricalCommitPhysicalTransportKernelProperties(
                  specification)
         PROVE HistoricalCommitCertificateTransportLeaf(specification)
    <2>1. specification
             => []HistoricalCommitCertificateRequestCompletenessInvariant
      BY <1>1 DEF HistoricalCommitRequestCompletenessProperty
    <2>2. specification
             => []HistoricalCommitArchiveRouteAvailabilityInvariant
      BY <1>1
         DEF HistoricalCommitArchiveRouteAvailabilityProperty
    <2>3. specification
             => \A target \in Responsive:
                  HistoricalCommitRequestSource(target)
                    => \E server, request:
                         /\ HistoricalCommitArchiveRouteAvailable(
                              target, server)
                         /\ HistoricalCommitRequestRegistered(
                              target, server, request)
      BY <2>1, <2>2,
         CompleteHistoricalCommitFanoutSelectsExactAppliedRoute, PTL
         DEF HistoricalCommitRequestSource
    <2>4. specification
             => \A target, server, request:
                  HistoricalCommitRequestRegistered(
                    target, server, request)
                    /\ HistoricalCommitArchiveRouteAvailable(
                         target, server)
                    ~> HistoricalCommitRequestPacketGoal(
                         target, server, request)
      BY <1>1
         DEF HistoricalCommitPhysicalTransportKernelProperties,
             HistoricalCommitRequestPacketEmissionKernelProperty
    <2>5. specification
             => \A target, server, request:
                  HistoricalCommitRequestPacketGoal(
                    target, server, request)
                    ~> HistoricalCommitRequestServeGoal(
                         target, server, request)
      BY <1>1, PTL
         DEF HistoricalCommitPhysicalTransportKernelProperties,
             HistoricalCommitRequestIngressKernelProperty,
             HistoricalCommitRequestPacketGoal,
             HistoricalCommitRequestServeGoal,
             HistoricalCommitTransportGoal
    <2>6. specification
             => \A target, server, request:
                  HistoricalCommitRequestServeGoal(
                    target, server, request)
                    ~> HistoricalCommitResponsePacketGoal(
                         target, server, request)
      BY <1>1, PTL
         DEF HistoricalCommitPhysicalTransportKernelProperties,
             HistoricalCommitServeResponseKernelProperty,
             HistoricalCommitRequestServeGoal,
             HistoricalCommitResponsePacketGoal,
             HistoricalCommitTransportGoal
    <2>7. specification
             => \A target, server, request:
                  HistoricalCommitResponsePacketGoal(
                    target, server, request)
                    ~> HistoricalCommitTransportGoal(target)
      BY <1>1, PTL
         DEF HistoricalCommitPhysicalTransportKernelProperties,
             HistoricalCommitResponseAdmissionKernelProperty,
             HistoricalCommitResponsePacketGoal,
             HistoricalCommitTransportGoal
    <2> QED BY <2>3, <2>4, <2>5, <2>6, <2>7, PTL
         DEF HistoricalCommitCertificateTransportLeaf
  <1> QED BY <1>1

(***************************************************************************
Broadened exact Decision certified-request completeness.

The earlier Decision application leaf scopes its completeness predicate to
`AsyncCurrentResponsiveVoters`.  The invariant below has no requester-roster
premise: every exact durable Decision record with one active exact request
retains its complete fanout.  This includes a responsive historical target
outside the frozen voting roster.
***************************************************************************)

HistoricalDecisionCertifiedRequestSetActive(node, qc) ==
  /\ CertifiedRequestOutbox(node, qc) # {}
  /\ CertifiedRequestOutbox(node, qc) \subseteq asyncActiveRequests

HistoricalDecisionCertifiedRequestCompletenessInvariant ==
  \A node, qc:
    /\ ExactDecisionRecord(node, qc)
    /\ DecisionCertifiedRequestActiveExact(node, qc)
    => HistoricalDecisionCertifiedRequestSetActive(node, qc)

THEOREM ExactFanoutRetentionImpliesHistoricalDecisionCompleteness ==
  ExactDecisionFanoutRetentionInvariant
    => HistoricalDecisionCertifiedRequestCompletenessInvariant
BY Isa
   DEF ExactDecisionFanoutRetentionInvariant,
       HistoricalDecisionCertifiedRequestCompletenessInvariant,
       HistoricalDecisionCertifiedRequestSetActive,
       DecisionCertifiedRequestActiveExact

THEOREM HistoricalDecisionCompletenessImpliesExactFanoutRetention ==
  HistoricalDecisionCertifiedRequestCompletenessInvariant
    => ExactDecisionFanoutRetentionInvariant
BY Isa
   DEF ExactDecisionFanoutRetentionInvariant,
       HistoricalDecisionCertifiedRequestCompletenessInvariant,
       HistoricalDecisionCertifiedRequestSetActive

THEOREM AsyncInitEstablishesHistoricalDecisionRequestCompleteness ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => HistoricalDecisionCertifiedRequestCompletenessInvariant
BY AsyncInitEstablishesExactDecisionFanoutRetention,
   ExactFanoutRetentionImpliesHistoricalDecisionCompleteness

THEOREM AsyncNextPreservesHistoricalDecisionRequestCompleteness ==
  /\ AsyncStrongTypeInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ HistoricalDecisionCertifiedRequestCompletenessInvariant
  /\ [AsyncNext]_AsyncAllVars
  => HistoricalDecisionCertifiedRequestCompletenessInvariant'
BY HistoricalDecisionCompletenessImpliesExactFanoutRetention,
   AsyncNextPreservesExactDecisionFanoutRetention,
   ExactFanoutRetentionImpliesHistoricalDecisionCompleteness

THEOREM HistoricalDecisionRequestCompletenessObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []HistoricalDecisionCertifiedRequestCompletenessInvariant
BY AsyncSpecAlwaysRetainsExactDecisionFanout,
   ExactFanoutRetentionImpliesHistoricalDecisionCompleteness, PTL

HistoricalDecisionCertifiedRequestCompletenessProperty(specification) ==
  specification
    => []HistoricalDecisionCertifiedRequestCompletenessInvariant

(***************************************************************************
Responsive historical body-existence witness.

The requester is only required to be a typed validator.  It need not belong
to `AsyncCurrentResponsiveVoters`.  This lemma uses quorum intersection only
to prove the exact V5 service authority: at least one responsive CommitQC
signer durably holds the body backing its honest intent.  Full frozen-roster
fanout ensures that this signer receives the request; a routed non-signer
terminalizes `LocalRetentionAuthorityAbsent` and never signs a response.  The
requester's missing local body proves that the selected signer is a different
node.
***************************************************************************)

THEOREM HistoricalDecisionRecoveryCertificateHasResponsiveRemoteBodySource ==
  \A node \in ValidatorIds,
     qc \in QcRecordSet,
     recoveryQc \in QcRecordSet:
    /\ StrongInductiveInvariant
    /\ DecisionRecoveryCertificate(node, qc, recoveryQc)
    /\ qc.context = context
    /\ ~BodyHeldBy(durableBodies, node, qc.context,
                    qc.view, qc.subject)
    => \E source \in
         (recoveryQc.signers \cap AsyncCurrentResponsiveVoters) \ {node}:
         BodyHeldBy(durableBodies, source, qc.context,
                    qc.view, qc.subject)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW qc \in QcRecordSet,
                NEW recoveryQc \in QcRecordSet,
                StrongInductiveInvariant,
                DecisionRecoveryCertificate(node, qc, recoveryQc),
                qc.context = context,
                ~BodyHeldBy(durableBodies, node, qc.context,
                             qc.view, qc.subject)
         PROVE \E source \in
                   (recoveryQc.signers
                      \cap AsyncCurrentResponsiveVoters) \ {node}:
                 BodyHeldBy(durableBodies, source, qc.context,
                            qc.view, qc.subject)
    <2>1. recoveryQc \in commitQCs
      BY <1>1, SMT
         DEF StrongInductiveInvariant, Safety, DecisionAgreement,
             DecisionRecoveryCertificate
    <2>2. /\ ModelConfiguration
           /\ QuorumConfiguration
           /\ CertificatesBackedByIntents
           /\ HonestIntentSound(
                commitIntents, durableBodies, ValidSubjects)
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             HonestDurableIntentsSound, Safety, TypeInvariant,
             ModelConfiguration
    <2>3. /\ recoveryQc.context = context
           /\ recoveryQc.context.epoch = CurrentEpoch
           /\ CurrentEpoch \in Epochs
      BY <1>1, <2>2, TypeInvariantMakesCurrentEpochTyped
         DEF DecisionRecoveryCertificate, CurrentEpoch,
             StrongInductiveInvariant, Safety
    <2>4. CertificateBackedBy(
             CurrentEpoch, recoveryQc, commitIntents)
      BY <2>1, <2>2, <2>3
         DEF CertificatesBackedByIntents
    <2>5. /\ DualQuorum(CurrentEpoch, recoveryQc.signers)
           /\ recoveryQc.signers
                \in SUBSET VotingRoster(CurrentEpoch)
      BY <2>4 DEF CertificateBackedBy, DualQuorum, CountQuorum
    <2>6. /\ DualQuorum(
                CurrentEpoch, AsyncCurrentResponsiveVoters)
           /\ AsyncCurrentResponsiveVoters
                \in SUBSET VotingRoster(CurrentEpoch)
      BY <2>2, <2>3, Isa
         DEF ModelConfiguration, AsyncCurrentResponsiveVoters,
             CurrentVoters
    <2>7. DualQuorumIntersectionHasHonest
      BY <2>2, DualQuorumHonestIntersection
    <2>8. (recoveryQc.signers
             \cap AsyncCurrentResponsiveVoters \cap Honest) # {}
      BY <2>3, <2>5, <2>6, <2>7
         DEF DualQuorumIntersectionHasHonest
    <2>9. PICK source \in
                recoveryQc.signers
                  \cap AsyncCurrentResponsiveVoters \cap Honest:
             TRUE
      BY <2>8
    <2>10. PICK vote \in commitIntents:
              VoteBacksCertificate(vote, recoveryQc, source)
      BY <2>4, <2>9 DEF CertificateBackedBy
    <2>11. /\ vote.signer = source
            /\ vote.context = recoveryQc.context
            /\ vote.view = recoveryQc.view
            /\ vote.subject = recoveryQc.subject
      BY <2>10 DEF VoteBacksCertificate
    <2>12. BodyHeldBy(durableBodies, source,
                      recoveryQc.context, recoveryQc.view,
                      recoveryQc.subject)
      BY <2>2, <2>9, <2>10, <2>11
         DEF HonestIntentSound
    <2>13. BodyHeldBy(durableBodies, source,
                      qc.context, qc.view, qc.subject)
      BY <1>1, <2>12 DEF DecisionRecoveryCertificate
    <2>14. source # node
      BY <1>1, <2>13
    <2> QED BY <2>9, <2>13, <2>14
  <1> QED BY <1>1

(***************************************************************************
Exact certified-body transport for a historical Decision requester.

This source differs from `ExactDecisionServiceSource` only in executor
ownership: the requester is the exact historical target and need not be a
member of the frozen responsive voter set.  For this existence proof the
serving archive selected below is the responsive CommitQC signer whose body
retention follows from honest-intent soundness.  V5 response authority is
exactly that signer/body intersection; other frozen-roster recipients close
their local lifecycle with a typed negative.  The requester consumes the
signer's response through its historical runner.
***************************************************************************)

HistoricalExactDecisionServiceSource(node, qc) ==
  /\ gst
  /\ node \in Responsive
  /\ HistoricalRecoveryTarget(node)
  /\ ExactDecisionRecord(node, qc)
  /\ DecisionRecoveryStageExact(node, qc)

HistoricalExactDecisionActiveRequestOwner(node, qc) ==
  /\ HistoricalExactDecisionServiceSource(node, qc)
  /\ ~NodeHasApplication(node)
  /\ ~BodyHeldBy(durableBodies, node, qc.context,
                  qc.view, qc.subject)
  /\ ~DecisionValidationHeld(node, qc)
  /\ DecisionCertifiedRequestActiveExact(node, qc)

HistoricalDecisionBodyHoldingAlias(node, qc, archive, request) ==
  /\ HistoricalExactDecisionActiveRequestOwner(node, qc)
  /\ archive \in AsyncCurrentResponsiveVoters \ {node}
  /\ BodyHeldBy(durableBodies, archive, qc.context,
                qc.view, qc.subject)
  /\ request \in CertifiedRequestOutbox(node, qc)
  /\ request.envelope.recipient = archive
  /\ request \in asyncActiveRequests
  /\ CertifiedServeCanRespond(archive, request)

THEOREM HistoricalDecisionRequestHasResponsiveBodyHoldingAlias ==
  \A node, qc:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionCertifiedRequestCompletenessInvariant
    /\ HistoricalExactDecisionActiveRequestOwner(node, qc)
    => \E archive \in AsyncCurrentResponsiveVoters,
          request \in asyncActiveRequests:
         HistoricalDecisionBodyHoldingAlias(
           node, qc, archive, request)
BY HistoricalDecisionRecoveryCertificateHasResponsiveRemoteBodySource,
   ExactDecisionCertifiedRequestBindsHashAndArchiveRoute, IsaT(300)
   DEF HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       HistoricalDecisionCertifiedRequestCompletenessInvariant,
       HistoricalDecisionCertifiedRequestSetActive,
       ExactDecisionRecord, DecisionRecoveryCertificate,
       DecisionCertifiedRequestActiveExact,
       CertifiedRequestOutbox, CertifiedArchiveRoutes,
       AsyncResponsiveArchiveServers,
       CertifiedServeCanRespond,
       AsyncStrongTypeInvariant, StrongInductiveInvariant

HistoricalDecisionRequestPacketOwned(
    node, qc, archive, request, packet) ==
  /\ HistoricalDecisionBodyHoldingAlias(
       node, qc, archive, request)
  /\ packet \in asyncTransport
  /\ packet.item = request

HistoricalDecisionServeLifecycleIdentity(archive, request) ==
  AsyncServeLogicalRequestIdentity(archive, request)

HistoricalDecisionRequestIngressOwned(
    node, qc, archive, request) ==
  LET identity ==
        HistoricalDecisionServeLifecycleIdentity(archive, request)
  IN /\ HistoricalDecisionBodyHoldingAlias(
          node, qc, archive, request)
     /\ request \in
          SequenceSet(
            IngressLane(archive, IngressResourceSource(request)))
     /\ AsyncServeIngressAdmissionOwned(archive, identity)
     /\ AsyncServeIngressAdmissionOrdinal(archive, identity) \in Nat \ {0}

HistoricalDecisionServeAdmissionOwned(archive, request) ==
  LET identity ==
        HistoricalDecisionServeLifecycleIdentity(archive, request)
  IN /\ AsyncServeLiveReservationOwned(archive, identity)
     /\ AsyncServeAdmissionOrdinal(archive, identity) \in Nat \ {0}

HistoricalDecisionServeOccurrenceOwned(archive, request, job) ==
  LET identity ==
        HistoricalDecisionServeLifecycleIdentity(archive, request)
  IN /\ HistoricalDecisionServeAdmissionOwned(archive, request)
     /\ AsyncServeJobQueued(archive, identity)
     /\ job \in SequenceSet(asyncIoQueues[archive])
     /\ job.class = "Serve"
     /\ job.candidate.item = request
     /\ AsyncIoServeJobIdentity(archive, job) = identity
     /\ job.nonce \in 0..AsyncIoAuxCapacity

HistoricalDecisionServeJobOwned(
    node, qc, archive, request, job) ==
  /\ HistoricalDecisionBodyHoldingAlias(
       node, qc, archive, request)
  /\ HistoricalDecisionServeOccurrenceOwned(
       archive, request, job)

HistoricalDecisionAuthenticatedResponse(
    node, qc, archive, request, response) ==
  /\ HistoricalDecisionBodyHoldingAlias(
       node, qc, archive, request)
  /\ response =
       CertifiedResponseItem(AsyncUntrustedSource, archive, request)
  /\ DecisionCertifiedResponseLineageExact(node, qc, response)

HistoricalDecisionServeTombstoneOwned(
    node, qc, archive, request) ==
  LET identity ==
        HistoricalDecisionServeLifecycleIdentity(archive, request)
  IN /\ HistoricalDecisionBodyHoldingAlias(
          node, qc, archive, request)
     /\ AsyncServeLifecycleTombstone(archive, identity)
     /\ AsyncServeTombstoneOutputs(archive, identity) # {}
     /\ \A response \in AsyncServeTombstoneOutputs(archive, identity):
          DecisionCertifiedResponseLineageExact(node, qc, response)

HistoricalDecisionResponsePacketOwned(
    node, qc, archive, request, response, packet) ==
  /\ HistoricalDecisionAuthenticatedResponse(
       node, qc, archive, request, response)
  /\ packet \in asyncTransport
  /\ packet.item = response

HistoricalDecisionClaimedResponseIngressOwned(node, qc, response) ==
  /\ HistoricalExactDecisionServiceSource(node, qc)
  /\ DecisionCertifiedResponseLineageExact(node, qc, response)
  /\ CertifiedResponseClaimAuthorized(response)
  /\ response \in
       SequenceSet(
         IngressLane(node, IngressResourceSource(response)))

HistoricalDecisionRouteNeutralClaimIngressOwned(node, qc, response) ==
  /\ HistoricalExactDecisionServiceSource(node, qc)
  /\ DecisionCertifiedResponseLineageExact(node, qc, response)
  /\ CertifiedResponseClaimMatches(response)
  /\ CertifiedResponseClaimIngressOwner(
       AsyncCertifiedResponseAuthProjection(response))

HistoricalDecisionCertifiedResponseGoal(node, qc) ==
  \/ NodeHasApplication(node)
  \/ ~HistoricalRecoveryTarget(node)
  \/ DecisionCertifiedFetchOwnedExact(node, qc)

(***************************************************************************
Certified-body action-local handoffs.
***************************************************************************)

THEOREM HistoricalDecisionRetransmissionCreatesExactRequestPacket ==
  \A node, qc, archive, request:
    /\ HistoricalDecisionBodyHoldingAlias(
         node, qc, archive, request)
    /\ SendNodeRetransmissions(node)
    => \E packet:
         HistoricalDecisionRequestPacketOwned(
           node, qc, archive, request, packet)'
BY Isa
   DEF HistoricalDecisionRequestPacketOwned,
       HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       SendNodeRetransmissions, RetryableItems,
       ActiveRequestItems, PacketsForItems, PacketForItem

THEOREM HistoricalDecisionRequestPacketCreatesIngressOwner ==
  \A node, qc, archive, request, packet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionRequestPacketOwned(
         node, qc, archive, request, packet)
    /\ packet = OldestDueSourcePacket(archive, request.source)
    /\ AdmitIngressPacket(archive, request.source)
    => HistoricalDecisionRequestIngressOwned(
         node, qc, archive, request)'
BY IsaT(240)
   DEF HistoricalDecisionRequestPacketOwned,
       HistoricalDecisionRequestIngressOwned,
       HistoricalDecisionServeLifecycleIdentity,
       HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       AcceptOrReserveExactServeIngress,
       ReserveExactServeCapacity, AdvanceExactServeCapacity,
       CoalesceExactServeIngressCapacity,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionOrdinal,
       DropPolicyRejectedHiddenPacket,
       IngressPacketPolicyRejected,
       CertifiedResponsePacketPolicyRejected,
       UntrustedGenericCompletionPacketPolicyRejected,
       IngressResourceSource, IngressLane, SequenceSet

THEOREM NormalHistoricalDecisionRequestCreatesFreshServeOwner ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionRequestIngressOwned(
         node, qc, archive, request)
    /\ HistoricalDecisionServeAdmissionOwned(archive, request)
    /\ ~NodeHasApplication(archive)
    /\ SelectedIngressItemAt(
         archive, FirstDrainableIngressIndex(archive)) = request
    /\ DrainFairIngressSelected(archive)
    => \E job \in SequenceSet(asyncIoQueues'[archive]):
         HistoricalDecisionServeJobOwned(
           node, qc, archive, request, job)'
BY FreshAsyncIoServeNonceFacts,
   TypedRequestMakesTypedServeJob, IsaT(240)
   DEF HistoricalDecisionRequestIngressOwned,
       HistoricalDecisionServeJobOwned,
       HistoricalDecisionServeOccurrenceOwned,
       HistoricalDecisionServeAdmissionOwned,
       HistoricalDecisionServeLifecycleIdentity,
       HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       DrainFairIngressSelected, AsyncIoCertifiedServeJob,
       CertifiedRequestAuthorized, CertifiedRequestAuthority,
       FreshAsyncIoServeNonce, SequenceSet

THEOREM AppliedHistoricalDecisionRequestCreatesFreshServeOwner ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionRequestIngressOwned(
         node, qc, archive, request)
    /\ HistoricalDecisionServeAdmissionOwned(archive, request)
    /\ NodeHasApplication(archive)
    /\ HistoricalSelectedIngressItemAt(
         archive,
         FirstHistoricalDrainableIngressIndex(archive)) = request
    /\ DrainHistoricalIngressSelected(archive)
    => \E job \in SequenceSet(asyncIoQueues'[archive]):
         HistoricalDecisionServeJobOwned(
           node, qc, archive, request, job)'
BY FreshAsyncIoServeNonceFacts,
   TypedRequestMakesTypedServeJob, IsaT(240)
   DEF HistoricalDecisionRequestIngressOwned,
       HistoricalDecisionServeJobOwned,
       HistoricalDecisionServeOccurrenceOwned,
       HistoricalDecisionServeAdmissionOwned,
       HistoricalDecisionServeLifecycleIdentity,
       HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       DrainHistoricalIngressSelected,
       AsyncIoCertifiedServeJob,
       CertifiedRequestAuthorized, CertifiedRequestAuthority,
       FreshAsyncIoServeNonce, SequenceSet

HistoricalDecisionRequestIngressRunnerAction(archive, request) ==
  \/ /\ ~NodeHasApplication(archive)
     /\ DrainFairIngressSelected(archive)
     /\ SelectedIngressItemAt(
          archive, FirstDrainableIngressIndex(archive)) = request
  \/ /\ NodeHasApplication(archive)
     /\ DrainHistoricalIngressSelected(archive)
     /\ HistoricalSelectedIngressItemAt(
          archive,
          FirstHistoricalDrainableIngressIndex(archive)) = request

THEOREM CachedHistoricalDecisionRequestCreatesResponseOwner ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionRequestIngressOwned(
         node, qc, archive, request)
    /\ HistoricalDecisionServeTombstoneOwned(
         node, qc, archive, request)
    /\ HistoricalDecisionRequestIngressRunnerAction(archive, request)
    => \E response, packet:
         HistoricalDecisionResponsePacketOwned(
           node, qc, archive, request, response, packet)'
BY IsaT(300)
   DEF HistoricalDecisionRequestIngressOwned,
       HistoricalDecisionRequestIngressRunnerAction,
       HistoricalDecisionServeTombstoneOwned,
       HistoricalDecisionServeLifecycleIdentity,
       HistoricalDecisionResponsePacketOwned,
       HistoricalDecisionAuthenticatedResponse,
       HistoricalDecisionBodyHoldingAlias,
       AsyncServeCachedReplayItems,
       AsyncServeTombstoneOutputs,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       PublishEphemeralItems, PacketsForItems,
       IngressResourceSource, IngressLane, SequenceSet

THEOREM HistoricalDecisionRequestIngressCreatesLifecycleOutcome ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionRequestIngressOwned(
         node, qc, archive, request)
    /\ HistoricalDecisionRequestIngressRunnerAction(archive, request)
    => \/ \E job \in SequenceSet(asyncIoQueues'[archive]):
              HistoricalDecisionServeJobOwned(
                node, qc, archive, request, job)'
       \/ \E response, packet:
              HistoricalDecisionResponsePacketOwned(
                node, qc, archive, request, response, packet)'
BY NormalHistoricalDecisionRequestCreatesFreshServeOwner,
   AppliedHistoricalDecisionRequestCreatesFreshServeOwner,
   CachedHistoricalDecisionRequestCreatesResponseOwner, IsaT(240)
   DEF HistoricalDecisionRequestIngressRunnerAction,
       HistoricalDecisionRequestIngressOwned,
       HistoricalDecisionServeAdmissionOwned,
       HistoricalDecisionServeTombstoneOwned,
       HistoricalDecisionServeLifecycleIdentity,
       AsyncServeLifecyclePartitionInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant, AsyncServeLifecycleTypeInvariant

THEOREM HistoricalDecisionServeJobUsesOrdinaryArchiveIoOwner ==
  \A node, qc, archive, request, job:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionServeJobOwned(
         node, qc, archive, request, job)
    => /\ archive \in AsyncArchiveIoServiceNodes
       /\ job \in AsyncServeJobSet
       /\ ResponsiveProtectedServeJobOwned(archive, job)
BY TypedCandidateIsInCarrier, IsaT(180)
   DEF HistoricalDecisionServeJobOwned,
       HistoricalDecisionServeOccurrenceOwned,
       HistoricalDecisionServeAdmissionOwned,
       HistoricalDecisionServeLifecycleIdentity,
       HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       ResponsiveProtectedServeJobOwned,
       AsyncServeJobSet, AsyncIoJob,
       AsyncArchiveIoServiceNodes,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncIoTypeInvariant,
       AsyncIoContentTypeInvariant,
       AsyncIoQueueContentTypeInvariant,
       AsyncIoSequenceTyped, AsyncIoJobTyped,
       SequenceSet

THEOREM HistoricalDecisionServeHeadCreatesAuthenticatedResponsePacket ==
  \A node, qc, archive, request, job:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionServeJobOwned(
         node, qc, archive, request, job)
    /\ Head(asyncIoQueues[archive]) = job
    /\ ServiceIoWorkerWork(archive)
    => \E response, packet:
         HistoricalDecisionResponsePacketOwned(
           node, qc, archive, request, response, packet)'
BY SentCertifiedResponseAuthenticatesEveryRelayOccurrence,
   ExactCertifiedResponseMatchesDecisionRequestHash, IsaT(240)
   DEF HistoricalDecisionServeJobOwned,
       HistoricalDecisionServeOccurrenceOwned,
       HistoricalDecisionServeAdmissionOwned,
       HistoricalDecisionServeLifecycleIdentity,
       HistoricalDecisionAuthenticatedResponse,
       HistoricalDecisionResponsePacketOwned,
       HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       ServiceIoWorkerWork, CertifiedServeCanRespond,
       CertifiedResponseItem, PublishEphemeralItems,
       PacketsForItems, DecisionCertifiedResponseLineageExact

THEOREM HistoricalDecisionAdmittedServeInstallsExactTombstone ==
  \A node, qc, archive, request, job:
    LET identity ==
          HistoricalDecisionServeLifecycleIdentity(archive, request)
    IN /\ AsyncStrongTypeInvariant
       /\ HistoricalDecisionServeJobOwned(
            node, qc, archive, request, job)
       /\ Head(asyncIoQueues[archive]) = job
       /\ ServiceIoWorkerWork(archive)
       => /\ AsyncServeLifecycleTombstone(archive, identity)'
          /\ AsyncServeTombstoneOutputs(archive, identity)' # {}
          /\ ~AsyncServeLiveReservationOwned(archive, identity)'
          /\ ~AsyncServeJobQueued(archive, identity)'
BY HistoricalDecisionServeHeadCreatesAuthenticatedResponsePacket,
   IsaT(240)
   DEF HistoricalDecisionServeJobOwned,
       HistoricalDecisionServeOccurrenceOwned,
       HistoricalDecisionServeAdmissionOwned,
       HistoricalDecisionServeLifecycleIdentity,
       ServiceIoWorkerWork,
       AsyncServeLifecycleTombstone,
       AsyncServeTombstoneOutputs,
       AsyncServeTombstoneRecords,
       AsyncServeReservationRecord,
       AsyncServeTombstonesWithoutFamily,
       AsyncServeTombstone,
       AsyncServeLiveReservationOwned,
       AsyncServeJobQueued

THEOREM HistoricalDecisionServeOrdinalIsImmutableUntilTerminalExit ==
  \A archive, request:
    LET identity ==
          HistoricalDecisionServeLifecycleIdentity(archive, request)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncServeLifecycleOwned(archive, identity)
       /\ AsyncNext
       /\ AsyncServeLifecycleOwned(archive, identity)'
       => AsyncServeAdmissionOrdinal(archive, identity)'
            = AsyncServeAdmissionOrdinal(archive, identity)
BY IsaT(300)
   DEF HistoricalDecisionServeLifecycleIdentity,
       AsyncServeAdmissionOrdinal,
       AsyncServeLifecycleOwned,
       AsyncServeLiveReservationOwned,
       AsyncServeLifecycleTombstone,
       AsyncServeReservationRecord,
       AsyncServeTombstoneRecord,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart

THEOREM HistoricalDecisionServeTombstoneCannotResurrectAtGst ==
  \A archive, request:
    LET identity ==
          HistoricalDecisionServeLifecycleIdentity(archive, request)
    IN /\ AsyncStrongTypeInvariant
       /\ gst
       /\ AsyncServeLifecycleTombstone(archive, identity)
       /\ [AsyncNext]_AsyncAllVars
       => /\ AsyncServeLogicalIdentityRetiredOrSuperseded(
                archive, identity)'
          /\ ~AsyncServeJobQueued(archive, identity)'
BY AsyncServeRetiredIdentityCannotRequeueAtGst, Isa
   DEF HistoricalDecisionServeLifecycleIdentity,
       AsyncServeLogicalIdentityRetiredOrSuperseded,
       AsyncServeLifecycleOwned,
       AsyncServeLifecycleTombstone,
       AsyncAllVars

THEOREM FreshHistoricalDecisionResponseAcquiresExactIngressOwner ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)
    /\ packet = OldestDueSourcePacket(node, response.source)
    /\ AdmitFreshHiddenPacket(node, response.source)
    => HistoricalDecisionClaimedResponseIngressOwned(
         node, qc, response)'
BY IsaT(300)
   DEF HistoricalDecisionResponsePacketOwned,
       HistoricalDecisionAuthenticatedResponse,
       HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       HistoricalDecisionClaimedResponseIngressOwned,
       AdmitFreshHiddenPacket, AdmitHiddenPacket,
       CertifiedResponseClaimAuthorized,
       CertifiedResponseAuthorized,
       CertifiedResponseFreshClaimGateAllows,
       IngressResourceSource, IngressLane, SequenceSet

THEOREM CoalescedHistoricalDecisionResponseRetainsRouteNeutralOwner ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)
    /\ packet = OldestDueSourcePacket(node, response.source)
    /\ CoalesceHiddenPacket(node, response.source)
    => HistoricalDecisionRouteNeutralClaimIngressOwned(
         node, qc, response)'
BY IsaT(300)
   DEF HistoricalDecisionResponsePacketOwned,
       HistoricalDecisionAuthenticatedResponse,
       HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       HistoricalDecisionRouteNeutralClaimIngressOwned,
       CoalesceHiddenPacket,
       CertifiedResponseClaimMatches,
       CertifiedResponseClaimIngressOwner,
       IngressHasCoalescingOwner,
       IngressCoalescingIdentity,
       AsyncCertifiedResponseAuthProjection,
       IngressResourceSource, IngressLane, SequenceSet

THEOREM HistoricalDecisionRouteNeutralOwnerHasExactIngressOccurrence ==
  \A node, qc, response:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionRouteNeutralClaimIngressOwned(
         node, qc, response)
    => \E admitted:
         /\ AsyncCertifiedResponseAuthProjection(admitted)
              = AsyncCertifiedResponseAuthProjection(response)
         /\ HistoricalDecisionClaimedResponseIngressOwned(
              node, qc, admitted)
BY ExactDecisionResponseLineageTransfersAcrossRouteNeutralIdentity,
   MatchingClaimedCertifiedResponseIsAuthorized, IsaT(240)
   DEF HistoricalDecisionRouteNeutralClaimIngressOwned,
       HistoricalDecisionClaimedResponseIngressOwned,
       CertifiedResponseClaimIngressOwner,
       CertifiedResponseClaimMatches,
       AsyncStrongTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       IngressResourceSource, IngressLaneDepth, SequenceSet

THEOREM HistoricalDecisionResponseIngressCreatesCertifiedFetch ==
  \A node, qc, response:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionClaimedResponseIngressOwned(node, qc, response)
    /\ SelectedIngressItemAt(
         node, FirstDrainableIngressIndex(node)) = response
    /\ DrainFairIngressSelected(node)
    => HistoricalDecisionCertifiedResponseGoal(node, qc)'
BY ExactCertifiedResponseCandidateRetainsOuterItem, IsaT(300)
   DEF HistoricalDecisionClaimedResponseIngressOwned,
       HistoricalExactDecisionServiceSource,
       HistoricalDecisionCertifiedResponseGoal,
       DecisionCertifiedFetchOwnedExact,
       DecisionCertifiedResponseLineageExact,
       DrainFairIngressSelected,
       CertifiedResponseCandidate,
       CertifiedResponseClaimAuthorized,
       MatchingCertifiedRequests,
       EnqueueCandidate, CandidateScheduled,
       CandidateConsumerCurrent,
       IngressResourceSource, IngressLane, SequenceSet

(***************************************************************************
Historical certified-request occurrence retention.

These are safety classifications only.  They retain the immutable request and
archive identity across retry, admission, and FIFO motion; the only terminal
alternative is the exact certified-response owner.  In particular, the
Serve-job arm is tied to the lifecycle identity and a queued identity may
depart only through the response-producing tombstone transition above.
***************************************************************************)

THEOREM HistoricalDecisionAliasPersistsOrGoals ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionBodyHoldingAlias(
         node, qc, archive, request)
    /\ ~HistoricalDecisionCertifiedResponseGoal(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionBodyHoldingAlias(
            node, qc, archive, request)'
       \/ HistoricalDecisionCertifiedResponseGoal(node, qc)'
BY AsyncBracketStepRetainsDurableBodies,
   HistoricalDecisionTargetPersistsUntilApplication,
   GstAsyncStepIsMonotone, IsaT(600)
   DEF HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       HistoricalDecisionCertifiedResponseGoal,
       DecisionCertifiedFetchOwnedExact,
       DecisionCertifiedRequestActiveExact,
       MatchingCertifiedRequests,
       DrainFairIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncAllVars

THEOREM HistoricalDecisionRequestPacketPersistsOrHandsOff ==
  \A node, qc, archive, request, packet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionRequestPacketOwned(
         node, qc, archive, request, packet)
    /\ ~HistoricalDecisionCertifiedResponseGoal(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionRequestPacketOwned(
            node, qc, archive, request, packet)'
       \/ HistoricalDecisionRequestIngressOwned(
            node, qc, archive, request)'
       \/ HistoricalDecisionCertifiedResponseGoal(node, qc)'
BY HistoricalDecisionRequestPacketCreatesIngressOwner,
   HistoricalDecisionAliasPersistsOrGoals, IsaT(600)
   DEF HistoricalDecisionRequestPacketOwned,
       HistoricalDecisionRequestIngressOwned,
       HistoricalDecisionServeLifecycleIdentity,
       HistoricalDecisionCertifiedResponseGoal,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstLosePacket,
       DrainFairIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply, AsyncAllVars

THEOREM HistoricalDecisionRequestIngressPersistsOrHandsOff ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionRequestIngressOwned(
         node, qc, archive, request)
    /\ ~HistoricalDecisionCertifiedResponseGoal(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionRequestIngressOwned(
            node, qc, archive, request)'
       \/ \E job:
            HistoricalDecisionServeJobOwned(
              node, qc, archive, request, job)'
       \/ \E response, packet:
            HistoricalDecisionResponsePacketOwned(
              node, qc, archive, request, response, packet)'
       \/ HistoricalDecisionCertifiedResponseGoal(node, qc)'
BY HistoricalDecisionRequestIngressCreatesLifecycleOutcome,
   HistoricalDecisionAliasPersistsOrGoals, IsaT(600)
   DEF HistoricalDecisionRequestIngressOwned,
       HistoricalDecisionServeJobOwned,
       HistoricalDecisionServeOccurrenceOwned,
       HistoricalDecisionServeAdmissionOwned,
       HistoricalDecisionServeTombstoneOwned,
       HistoricalDecisionServeLifecycleIdentity,
       HistoricalDecisionResponsePacketOwned,
       HistoricalDecisionCertifiedResponseGoal,
       HistoricalDecisionRequestIngressRunnerAction,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RunHistoricalServer,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply, AsyncAllVars

THEOREM HistoricalDecisionServePersistsOrResponds ==
  \A node, qc, archive, request, job:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionServeJobOwned(
         node, qc, archive, request, job)
    /\ ~HistoricalDecisionCertifiedResponseGoal(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionServeJobOwned(
            node, qc, archive, request, job)'
       \/ \E response, packet:
            HistoricalDecisionResponsePacketOwned(
              node, qc, archive, request, response, packet)'
       \/ HistoricalDecisionCertifiedResponseGoal(node, qc)'
BY HistoricalDecisionServeHeadCreatesAuthenticatedResponsePacket,
   HistoricalDecisionAliasPersistsOrGoals,
   ServeOccurrenceIndexAfterNonTargetHead,
   TailRemovesUniqueServeOccurrence,
   HeadTailProperties, IsaT(600)
   DEF HistoricalDecisionServeJobOwned,
       HistoricalDecisionServeOccurrenceOwned,
       HistoricalDecisionServeAdmissionOwned,
       HistoricalDecisionServeLifecycleIdentity,
       HistoricalDecisionResponsePacketOwned,
       HistoricalDecisionCertifiedResponseGoal,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       DrainFairIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply, AsyncAllVars

THEOREM HistoricalDecisionResponsePacketPersistsOrClaims ==
  \A node, qc, archive, request, response, packet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)
    /\ ~HistoricalDecisionCertifiedResponseGoal(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionResponsePacketOwned(
            node, qc, archive, request, response, packet)'
       \/ HistoricalDecisionRouteNeutralClaimIngressOwned(
            node, qc, response)'
       \/ HistoricalDecisionCertifiedResponseGoal(node, qc)'
BY FreshHistoricalDecisionResponseAcquiresExactIngressOwner,
   CoalescedHistoricalDecisionResponseRetainsRouteNeutralOwner,
   HistoricalDecisionAliasPersistsOrGoals, IsaT(600)
   DEF HistoricalDecisionResponsePacketOwned,
       HistoricalDecisionRouteNeutralClaimIngressOwned,
       HistoricalDecisionCertifiedResponseGoal,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       AsyncNetworkStep, AdmitIngressPacket,
       AsyncFaultStep, PreGstLosePacket,
       DrainFairIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply, AsyncAllVars

THEOREM HistoricalDecisionClaimIngressPersistsOrFetches ==
  \A node, qc, response:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionRouteNeutralClaimIngressOwned(
         node, qc, response)
    /\ ~HistoricalDecisionCertifiedResponseGoal(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionRouteNeutralClaimIngressOwned(
            node, qc, response)'
       \/ HistoricalDecisionCertifiedResponseGoal(node, qc)'
BY HistoricalDecisionRouteNeutralOwnerHasExactIngressOccurrence,
   HistoricalDecisionResponseIngressCreatesCertifiedFetch,
   HistoricalDecisionTargetPersistsUntilApplication,
   GstAsyncStepIsMonotone, IsaT(600)
   DEF HistoricalDecisionRouteNeutralClaimIngressOwned,
       HistoricalDecisionClaimedResponseIngressOwned,
       HistoricalDecisionCertifiedResponseGoal,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, DrainFairIngressSelected,
       ResetNodeSchedulerForRestart,
       ExecuteApply, AsyncAllVars

(***************************************************************************
Fairness-parametric certified-body reduction.

As above, the operators name exact residual kernels but do not assert them.
The reduction theorem proves only that discharging those kernels for an
arbitrary behavior is sufficient.  It does not smuggle `AsyncSpecAt` or
current-voter runner fairness into a historical requester.
***************************************************************************)

HistoricalDecisionRequestPacketGoal(node, qc, archive, request) ==
  \/ HistoricalDecisionCertifiedResponseGoal(node, qc)
  \/ \E packet:
       HistoricalDecisionRequestPacketOwned(
         node, qc, archive, request, packet)

HistoricalDecisionRequestServeGoal(node, qc, archive, request) ==
  \/ HistoricalDecisionCertifiedResponseGoal(node, qc)
  \/ \E job:
       HistoricalDecisionServeJobOwned(
         node, qc, archive, request, job)
  \/ \E response, packet:
       HistoricalDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)

HistoricalDecisionResponsePacketGoal(node, qc, archive, request) ==
  \/ HistoricalDecisionCertifiedResponseGoal(node, qc)
  \/ \E response, packet:
       HistoricalDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)

(***************************************************************************
Exact historical Decision request lifecycle rank.

The rank is endpoint-neutral: only the immutable requester/context/archive/
view/subject/request identity selects the lifecycle.  Retried transport keeps
that identity and ingress ordinal.  The same frozen predecessor set and
selector/lane/source/runner components used by the current-voter Decision
proof therefore apply to a historical requester without importing current-
voter liveness.
***************************************************************************)

HistoricalDecisionRequestLifecycleResidual(node, qc, archive, request) ==
  /\ HistoricalDecisionRequestIngressOwned(node, qc, archive, request)
  /\ ~HistoricalDecisionRequestServeGoal(node, qc, archive, request)

HistoricalDecisionRequestLifecycleStage(node, qc, archive, request) ==
  IF HistoricalDecisionRequestServeGoal(node, qc, archive, request)
  THEN 0
  ELSE IF HistoricalDecisionServeTombstoneOwned(
            node, qc, archive, request)
       THEN 1
       ELSE IF HistoricalDecisionServeAdmissionOwned(archive, request)
            THEN 2
            ELSE 3

HistoricalDecisionRequestFrozenPredecessorSet(archive, request) ==
  LET identity ==
        HistoricalDecisionServeLifecycleIdentity(archive, request)
  IN ({"Io"} \X AsyncServeFrozenPredecessorSet(archive, identity))
       \cup
     ({"Ingress"} \X
        AsyncServeIngressAdmissionPredecessorDebtSlots(archive, identity))
       \cup
     AsyncServePreexistingIngressOwnerPredecessorDebtSet(archive, identity)
       \cup
     AsyncServePreexistingIngressBarrierPredecessorDebtSet(archive, identity)

HistoricalDecisionRequestFrozenPredecessorDebt(archive, request) ==
  Cardinality(
    HistoricalDecisionRequestFrozenPredecessorSet(archive, request))

HistoricalDecisionRequestNestedIngressRank(node, qc, archive, request) ==
  IF HistoricalDecisionRequestLifecycleResidual(
       node, qc, archive, request)
  THEN ExactDecisionRequestIngressRank(archive, request)
  ELSE ExactDecisionRequestIngressZeroRank

HistoricalDecisionRequestLifecycleRank(node, qc, archive, request) ==
  <<HistoricalDecisionRequestLifecycleStage(node, qc, archive, request),
    <<HistoricalDecisionRequestFrozenPredecessorDebt(archive, request),
      HistoricalDecisionRequestNestedIngressRank(
        node, qc, archive, request)>>>>

HistoricalDecisionRequestLifecycleRankCarrier ==
  ExactDecisionRequestLifecycleIngressRankCarrier

HistoricalDecisionRequestLifecycleRankOrdering ==
  ExactDecisionRequestLifecycleIngressRankOrdering

THEOREM HistoricalDecisionRequestLifecycleRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    HistoricalDecisionRequestLifecycleRankOrdering,
    HistoricalDecisionRequestLifecycleRankCarrier)
BY ExactDecisionRequestLifecycleIngressRankOrderingIsWellFounded
   DEF HistoricalDecisionRequestLifecycleRankOrdering,
       HistoricalDecisionRequestLifecycleRankCarrier

THEOREM HistoricalDecisionRequestLifecycleRankInCarrier ==
  \A node, qc, archive, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDecisionRequestLifecycleResidual(
         node, qc, archive, request)
    => HistoricalDecisionRequestLifecycleRank(node, qc, archive, request)
         \in HistoricalDecisionRequestLifecycleRankCarrier
BY ExactDecisionRequestIngressPriorityDebtIsNatural,
   ExactDecisionRequestIngressServeCapacityDebtIsNatural,
   CandidateSequenceIndexIsPosition,
   DrainableIngressTurnReachRankIsNatural,
   FS_Union, FS_Product, FS_CardinalityType, IsaT(300)
   DEF HistoricalDecisionRequestLifecycleResidual,
       HistoricalDecisionRequestLifecycleRank,
       HistoricalDecisionRequestLifecycleStage,
       HistoricalDecisionRequestFrozenPredecessorDebt,
       HistoricalDecisionRequestFrozenPredecessorSet,
       HistoricalDecisionRequestNestedIngressRank,
       HistoricalDecisionRequestLifecycleRankCarrier,
       HistoricalDecisionRequestIngressOwned,
       HistoricalDecisionServeLifecycleIdentity,
       ExactDecisionRequestLifecycleIngressRankCarrier,
       ExactDecisionRequestLifecycleDebtCarrier,
       ExactDecisionRequestIngressRank,
       ExactDecisionRequestIngressRankCarrier,
       ExactDecisionRequestIngressCapacityRank,
       ExactDecisionRequestIngressReachSelectorRank,
       ExactDecisionRequestIngressSelectorRank,
       ExactDecisionRequestIngressLaneRank,
       ExactDecisionRequestIngressModeRank,
       ExactDecisionRequestIngressTargetServeCapacityDebt,
       ExactDecisionRequestIngressServeCapacityDebt,
       ExactDecisionRequestIngressPriorityDebt,
       ExactDecisionRequestIngressPriorityOwners,
       ExactDecisionRequestIngressLanePosition,
       ExactDecisionRequestIngressLaneIndices,
       ExactDecisionRequestIngressSourcePosition,
       IngressSourceServiceRank,
       ExactDecisionRequestIngressReachRank,
       ExactDecisionRequestIngressZeroRank,
       ExactDecisionRequestIngressZeroCapacityRank,
       ExactDecisionRequestIngressZeroReachSelectorRank,
       ExactDecisionRequestIngressZeroSelectorRank,
       ExactDecisionRequestIngressZeroLaneRank,
       AsyncConfiguration

HistoricalDecisionRequestPacketEmissionKernelProperty(specification) ==
  specification
    => \A node, qc, archive, request:
         HistoricalDecisionBodyHoldingAlias(
           node, qc, archive, request)
           ~> HistoricalDecisionRequestPacketGoal(
                node, qc, archive, request)

HistoricalDecisionRequestIngressKernelProperty(specification) ==
  specification
    => \A node, qc, archive, request, packet:
         HistoricalDecisionRequestPacketOwned(
           node, qc, archive, request, packet)
           ~> HistoricalDecisionRequestServeGoal(
                node, qc, archive, request)

HistoricalDecisionServeResponseKernelProperty(specification) ==
  specification
    => \A node, qc, archive, request, job:
         HistoricalDecisionServeJobOwned(
           node, qc, archive, request, job)
           ~> HistoricalDecisionResponsePacketGoal(
                node, qc, archive, request)

HistoricalDecisionResponseAdmissionKernelProperty(specification) ==
  specification
    => \A node, qc, archive, request, response, packet:
         HistoricalDecisionResponsePacketOwned(
           node, qc, archive, request, response, packet)
           ~> HistoricalDecisionCertifiedResponseGoal(node, qc)

HistoricalDecisionCertifiedTransportKernelProperties(specification) ==
  /\ HistoricalDecisionRequestPacketEmissionKernelProperty(specification)
  /\ HistoricalDecisionRequestIngressKernelProperty(specification)
  /\ HistoricalDecisionServeResponseKernelProperty(specification)
  /\ HistoricalDecisionResponseAdmissionKernelProperty(specification)

HistoricalDecisionCertifiedBodyTransportLeaf(specification) ==
  specification
    => \A node, qc:
         HistoricalExactDecisionActiveRequestOwner(node, qc)
           ~> HistoricalDecisionCertifiedResponseGoal(node, qc)

THEOREM HistoricalDecisionTransportKernelsDischargeExactLeaf ==
  \A specification:
    /\ (specification => []AsyncStrongTypeInvariant)
    /\ HistoricalDecisionCertifiedRequestCompletenessProperty(specification)
    /\ HistoricalDecisionCertifiedTransportKernelProperties(specification)
    => HistoricalDecisionCertifiedBodyTransportLeaf(specification)
PROOF
  <1>1. ASSUME NEW specification,
                specification => []AsyncStrongTypeInvariant,
                HistoricalDecisionCertifiedRequestCompletenessProperty(
                  specification),
                HistoricalDecisionCertifiedTransportKernelProperties(
                  specification)
         PROVE HistoricalDecisionCertifiedBodyTransportLeaf(specification)
    <2>1. specification
             => []HistoricalDecisionCertifiedRequestCompletenessInvariant
      BY <1>1
         DEF HistoricalDecisionCertifiedRequestCompletenessProperty
    <2>2. specification
             => \A node, qc:
                  HistoricalExactDecisionActiveRequestOwner(node, qc)
                    => \E archive, request:
                         HistoricalDecisionBodyHoldingAlias(
                           node, qc, archive, request)
      BY <1>1, <2>1,
         HistoricalDecisionRequestHasResponsiveBodyHoldingAlias, PTL
    <2>3. specification
             => \A node, qc, archive, request:
                  HistoricalDecisionBodyHoldingAlias(
                    node, qc, archive, request)
                    ~> HistoricalDecisionRequestPacketGoal(
                         node, qc, archive, request)
      BY <1>1
         DEF HistoricalDecisionCertifiedTransportKernelProperties,
             HistoricalDecisionRequestPacketEmissionKernelProperty
    <2>4. specification
             => \A node, qc, archive, request:
                  HistoricalDecisionRequestPacketGoal(
                    node, qc, archive, request)
                    ~> HistoricalDecisionRequestServeGoal(
                         node, qc, archive, request)
      BY <1>1, PTL
         DEF HistoricalDecisionCertifiedTransportKernelProperties,
             HistoricalDecisionRequestIngressKernelProperty,
             HistoricalDecisionRequestPacketGoal,
             HistoricalDecisionRequestServeGoal,
             HistoricalDecisionCertifiedResponseGoal
    <2>5. specification
             => \A node, qc, archive, request:
                  HistoricalDecisionRequestServeGoal(
                    node, qc, archive, request)
                    ~> HistoricalDecisionResponsePacketGoal(
                         node, qc, archive, request)
      BY <1>1, PTL
         DEF HistoricalDecisionCertifiedTransportKernelProperties,
             HistoricalDecisionServeResponseKernelProperty,
             HistoricalDecisionRequestServeGoal,
             HistoricalDecisionResponsePacketGoal,
             HistoricalDecisionCertifiedResponseGoal
    <2>6. specification
             => \A node, qc, archive, request:
                  HistoricalDecisionResponsePacketGoal(
                    node, qc, archive, request)
                    ~> HistoricalDecisionCertifiedResponseGoal(node, qc)
      BY <1>1, PTL
         DEF HistoricalDecisionCertifiedTransportKernelProperties,
             HistoricalDecisionResponseAdmissionKernelProperty,
             HistoricalDecisionResponsePacketGoal,
             HistoricalDecisionCertifiedResponseGoal
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, PTL
         DEF HistoricalDecisionCertifiedBodyTransportLeaf
  <1> QED BY <1>1

(***************************************************************************
Exact Commit-candidate tail handoffs.

Candidate starvation alone proves only departure from the scheduler carriers.
For DeliverQC, BeginDecision, and PersistDecision that departure must be tied
to the concrete reducer effect.  The following safety leaves retain the exact
context/view/subject/evidence/origin lineage across every non-consuming step;
the consuming step either installs the next causal owner or writes the exact
Decision.  A serviced/tombstoned identity cannot be counted as success by
itself.
***************************************************************************)

HistoricalCommitDecisionExactCarrierOwned(candidate, qc, kind) ==
  /\ candidate \in AsyncCandidateSet
  /\ qc \in commitQCs
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

HistoricalCommitDecisionExactLineageOwned(source, qc, kind) ==
  \E candidate \in AsyncCandidateSet:
    /\ HistoricalCommitDecisionExactCarrierOwned(candidate, qc, kind)
    /\ candidate.node = source.node
    /\ candidate.evidence = source.evidence
    /\ candidate.causalOrigin = source.causalOrigin

THEOREM HistoricalCommitDecisionOwnerHasExactCarrier ==
  \A node, kind:
    HistoricalCommitDecisionCandidateOwned(node, kind)
      <=> \E candidate \in AsyncCandidateSet, qc \in commitQCs:
            /\ candidate.node = node
            /\ HistoricalCommitDecisionExactCarrierOwned(
                 candidate, qc, kind)
BY Isa
   DEF HistoricalCommitDecisionCandidateOwned,
       HistoricalCommitDecisionExactCarrierOwned

THEOREM HistoricalCommitDeliveryOwnerPersistsOrBeginsDecision ==
  \A candidate, qc:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ HistoricalCommitDecisionExactCarrierOwned(
         candidate, qc, "DeliverQC")
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalCommitDecisionExactCarrierOwned(
            candidate, qc, "DeliverQC")'
       \/ NodeHasDecision(candidate.node)'
       \/ HistoricalCommitDecisionExactLineageOwned(
            candidate, qc, "BeginDecision")'
BY AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateIoCompletionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   AsyncCandidateBusyDeferralTransfersSameOwner,
   AsyncCandidateDeferredHandoffRetainsSameOwner,
   IsaT(1200)
   DEF HistoricalCommitDecisionExactCarrierOwned,
       HistoricalCommitDecisionCandidateOwned,
       HistoricalCommitDecisionExactLineageOwned,
       HistoricalCommitDecisionDirectEvidence,
       HistoricalCommitDecisionResponseEvidence,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned,
       ProtectedServiceCandidate,
       CandidateConsumerCurrent,
       CandidateScheduled,
       CandidateScheduledAfter,
       CommandDispatchable,
       CommandSuccessors,
       CausalCandidate,
       CausalCandidateWithEvidence,
       AppendCausalSuccessors,
       FreshCommandSuccessors,
       EnqueueCandidate,
       ExecuteCommand, ExecuteRegularCommand,
       BeginDecision, PersistDecision,
       AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       AsyncCandidateMonotoneSemanticCoverageAfterIn,
       AsyncCandidateReducerStageCoveredAfterIn,
       AsyncCandidateDecisionStageCoveredAfter,
       AsyncCandidateConsumerEpisodeObsoleteAfter,
       AsyncCandidateTerminalTombstoned,
       AsyncNext, AsyncNonCrashStep,
       AsyncEnterIndexedServiceActivation,
       AsyncActivateServiceNode,
       AsyncServiceActivationTransition,
       AsyncServiceActivationFrameVars,
       AsyncSchedulerExceptServiceActivation,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       AsyncNetworkStep, AsyncFaultStep,
       AsyncAllVars

THEOREM HistoricalBeginDecisionOwnerPersistsOrPersistsDecision ==
  \A candidate, qc:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ HistoricalCommitDecisionExactCarrierOwned(
         candidate, qc, "BeginDecision")
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalCommitDecisionExactCarrierOwned(
            candidate, qc, "BeginDecision")'
       \/ NodeHasDecision(candidate.node)'
       \/ HistoricalCommitDecisionExactLineageOwned(
            candidate, qc, "PersistDecision")'
BY AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateIoCompletionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   AsyncCandidateBusyDeferralTransfersSameOwner,
   AsyncCandidateDeferredHandoffRetainsSameOwner,
   IsaT(1200)
   DEF HistoricalCommitDecisionExactCarrierOwned,
       HistoricalCommitDecisionCandidateOwned,
       HistoricalCommitDecisionExactLineageOwned,
       HistoricalCommitDecisionDirectEvidence,
       HistoricalCommitDecisionResponseEvidence,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned,
       ProtectedServiceCandidate,
       CandidateConsumerCurrent,
       CandidateScheduled,
       CandidateScheduledAfter,
       CommandDispatchable,
       CommandSuccessors,
       CausalCandidate,
       CausalCandidateWithEvidence,
       AppendCausalSuccessors,
       FreshCommandSuccessors,
       EnqueueCandidate,
       ExecuteCommand, ExecuteRegularCommand,
       BeginDecision, PersistDecision,
       AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       AsyncCandidateMonotoneSemanticCoverageAfterIn,
       AsyncCandidateReducerStageCoveredAfterIn,
       AsyncCandidateDecisionStageCoveredAfter,
       AsyncCandidateConsumerEpisodeObsoleteAfter,
       AsyncCandidateTerminalTombstoned,
       AsyncNext, AsyncNonCrashStep,
       AsyncEnterIndexedServiceActivation,
       AsyncActivateServiceNode,
       AsyncServiceActivationTransition,
       AsyncServiceActivationFrameVars,
       AsyncSchedulerExceptServiceActivation,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       AsyncNetworkStep, AsyncFaultStep,
       AsyncAllVars

THEOREM HistoricalPersistDecisionOwnerPersistsOrWritesDecision ==
  \A candidate, qc:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ HistoricalCommitDecisionExactCarrierOwned(
         candidate, qc, "PersistDecision")
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalCommitDecisionExactCarrierOwned(
            candidate, qc, "PersistDecision")'
       \/ NodeHasDecision(candidate.node)'
BY AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateIoCompletionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   AsyncCandidateBusyDeferralTransfersSameOwner,
   AsyncCandidateDeferredHandoffRetainsSameOwner,
   IsaT(1200)
   DEF HistoricalCommitDecisionExactCarrierOwned,
       HistoricalCommitDecisionCandidateOwned,
       HistoricalCommitDecisionDirectEvidence,
       HistoricalCommitDecisionResponseEvidence,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned,
       ProtectedServiceCandidate,
       CandidateConsumerCurrent,
       CandidateScheduled,
       CandidateScheduledAfter,
       CommandDispatchable,
       CommandSuccessors,
       PersistDecisionRecoverySuccessor,
       AppendCausalSuccessors,
       FreshCommandSuccessors,
       EnqueueCandidate,
       ExecuteCommand, ExecuteRegularCommand,
       PersistDecision,
       AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       AsyncCandidateMonotoneSemanticCoverageAfterIn,
       AsyncCandidateReducerStageCoveredAfterIn,
       AsyncCandidateDecisionStageCoveredAfter,
       AsyncCandidateConsumerEpisodeObsoleteAfter,
       AsyncCandidateTerminalTombstoned,
       AsyncNext, AsyncNonCrashStep,
       AsyncEnterIndexedServiceActivation,
       AsyncActivateServiceNode,
       AsyncServiceActivationTransition,
       AsyncServiceActivationFrameVars,
       AsyncSchedulerExceptServiceActivation,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer,
       LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       AsyncNetworkStep, AsyncFaultStep,
       AsyncAllVars

HistoricalCommitDecisionTailTemporalSupportProperty(specification) ==
  /\ (specification => []AsyncStrongTypeInvariant)
  /\ (specification => []AsyncProgressOwnershipInvariant)
  /\ (specification => []AsyncCandidateServiceTombstoneLifecycleInvariant)
  /\ (specification => [][AsyncNext]_AsyncAllVars)
  /\ (specification => [](gst => []gst))

THEOREM HistoricalCommitDecisionExactCarrierTailClosesProgressLeaves ==
  \A specification:
    HistoricalCommitDecisionTailTemporalSupportProperty(specification)
      => /\ HistoricalCommitDeliveryProgressLeaf(specification)
         /\ HistoricalBeginDecisionProgressLeaf(specification)
         /\ HistoricalPersistDecisionProgressLeaf(specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalCommitDecisionTailTemporalSupportProperty(
                  specification)
         PROVE /\ HistoricalCommitDeliveryProgressLeaf(specification)
               /\ HistoricalBeginDecisionProgressLeaf(specification)
               /\ HistoricalPersistDecisionProgressLeaf(specification)
    <2>1. ASSUME ~HistoricalProtectedCandidateStarvationProperty(
                     specification)
           PROVE /\ HistoricalCommitDeliveryProgressLeaf(specification)
                 /\ HistoricalBeginDecisionProgressLeaf(specification)
                 /\ HistoricalPersistDecisionProgressLeaf(specification)
      BY <2>1
         DEF HistoricalCommitDeliveryProgressLeaf,
             HistoricalBeginDecisionProgressLeaf,
             HistoricalPersistDecisionProgressLeaf
    <2>2. ASSUME HistoricalProtectedCandidateStarvationProperty(
                    specification)
           PROVE /\ HistoricalCommitDeliveryProgressLeaf(specification)
                 /\ HistoricalBeginDecisionProgressLeaf(specification)
                 /\ HistoricalPersistDecisionProgressLeaf(specification)
      <3>1. ASSUME ~specification
             PROVE /\ HistoricalCommitDeliveryProgressLeaf(specification)
                   /\ HistoricalBeginDecisionProgressLeaf(specification)
                   /\ HistoricalPersistDecisionProgressLeaf(specification)
        BY <3>1
           DEF HistoricalCommitDeliveryProgressLeaf,
               HistoricalBeginDecisionProgressLeaf,
               HistoricalPersistDecisionProgressLeaf
      <3>2. ASSUME specification
             PROVE /\ HistoricalCommitDeliveryProgressLeaf(specification)
                   /\ HistoricalBeginDecisionProgressLeaf(specification)
                   /\ HistoricalPersistDecisionProgressLeaf(specification)
        <4>1. []AsyncStrongTypeInvariant
          BY <1>1, <3>2
             DEF HistoricalCommitDecisionTailTemporalSupportProperty
        <4>2. []AsyncProgressOwnershipInvariant
          BY <1>1, <3>2
             DEF HistoricalCommitDecisionTailTemporalSupportProperty
        <4>3. []AsyncCandidateServiceTombstoneLifecycleInvariant
          BY <1>1, <3>2
             DEF HistoricalCommitDecisionTailTemporalSupportProperty
        <4>4. [][AsyncNext]_AsyncAllVars
          BY <1>1, <3>2
             DEF HistoricalCommitDecisionTailTemporalSupportProperty
        <4>5. [](gst => []gst)
          BY <1>1, <3>2
             DEF HistoricalCommitDecisionTailTemporalSupportProperty
        <4>6. \A qc:
                \A candidate \in AsyncCandidateSet:
                  (gst
                    /\ HistoricalCommitDecisionExactCarrierOwned(
                         candidate, qc, "DeliverQC"))
                    ~> (NodeHasDecision(candidate.node)
                         \/ HistoricalCommitDecisionExactLineageOwned(
                              candidate, qc, "BeginDecision"))
          <5>1. ASSUME NEW candidate \in AsyncCandidateSet, NEW qc
                 PROVE
                   (gst
                     /\ HistoricalCommitDecisionExactCarrierOwned(
                          candidate, qc, "DeliverQC"))
                     ~> (NodeHasDecision(candidate.node)
                          \/ HistoricalCommitDecisionExactLineageOwned(
                               candidate, qc, "BeginDecision"))
            <6>1. (gst /\ HistoricalProtectedCandidateOwned(candidate))
                     ~> ~HistoricalProtectedCandidateOwned(candidate)
              BY <2>2, <3>2, <5>1
                 DEF HistoricalProtectedCandidateStarvationProperty,
                     HistoricalProtectedServiceOwnershipExit
            <6>2. [][(gst
                       /\ HistoricalCommitDecisionExactCarrierOwned(
                            candidate, qc, "DeliverQC")
                       /\ ~(NodeHasDecision(candidate.node)
                              \/ HistoricalCommitDecisionExactLineageOwned(
                                   candidate, qc, "BeginDecision"))
                      => (gst
                           /\ HistoricalCommitDecisionExactCarrierOwned(
                                candidate, qc, "DeliverQC"))'
                           \/ (NodeHasDecision(candidate.node)
                                \/ HistoricalCommitDecisionExactLineageOwned(
                                     candidate, qc, "BeginDecision"))')]_AsyncAllVars
              BY <4>1, <4>2, <4>3, <4>4, <4>5,
                 HistoricalCommitDeliveryOwnerPersistsOrBeginsDecision,
                 PTL
            <6> QED BY <6>1, <6>2, PTL
                 DEF HistoricalCommitDecisionExactCarrierOwned
          <5> QED BY <5>1
        <4>7. \A qc:
                \A candidate \in AsyncCandidateSet:
                  (gst
                    /\ HistoricalCommitDecisionExactCarrierOwned(
                         candidate, qc, "BeginDecision"))
                    ~> (NodeHasDecision(candidate.node)
                         \/ HistoricalCommitDecisionExactLineageOwned(
                              candidate, qc, "PersistDecision"))
          <5>1. ASSUME NEW candidate \in AsyncCandidateSet, NEW qc
                 PROVE
                   (gst
                     /\ HistoricalCommitDecisionExactCarrierOwned(
                          candidate, qc, "BeginDecision"))
                     ~> (NodeHasDecision(candidate.node)
                          \/ HistoricalCommitDecisionExactLineageOwned(
                               candidate, qc, "PersistDecision"))
            <6>1. (gst /\ HistoricalProtectedCandidateOwned(candidate))
                     ~> ~HistoricalProtectedCandidateOwned(candidate)
              BY <2>2, <3>2, <5>1
                 DEF HistoricalProtectedCandidateStarvationProperty,
                     HistoricalProtectedServiceOwnershipExit
            <6>2. [][(gst
                       /\ HistoricalCommitDecisionExactCarrierOwned(
                            candidate, qc, "BeginDecision")
                       /\ ~(NodeHasDecision(candidate.node)
                              \/ HistoricalCommitDecisionExactLineageOwned(
                                   candidate, qc, "PersistDecision"))
                      => (gst
                           /\ HistoricalCommitDecisionExactCarrierOwned(
                                candidate, qc, "BeginDecision"))'
                           \/ (NodeHasDecision(candidate.node)
                                \/ HistoricalCommitDecisionExactLineageOwned(
                                     candidate, qc, "PersistDecision"))')]_AsyncAllVars
              BY <4>1, <4>2, <4>3, <4>4, <4>5,
                 HistoricalBeginDecisionOwnerPersistsOrPersistsDecision,
                 PTL
            <6> QED BY <6>1, <6>2, PTL
                 DEF HistoricalCommitDecisionExactCarrierOwned
          <5> QED BY <5>1
        <4>8. \A qc:
                \A candidate \in AsyncCandidateSet:
                  (gst
                    /\ HistoricalCommitDecisionExactCarrierOwned(
                         candidate, qc, "PersistDecision"))
                    ~> NodeHasDecision(candidate.node)
          <5>1. ASSUME NEW candidate \in AsyncCandidateSet, NEW qc
                 PROVE
                   (gst
                     /\ HistoricalCommitDecisionExactCarrierOwned(
                          candidate, qc, "PersistDecision"))
                     ~> NodeHasDecision(candidate.node)
            <6>1. (gst /\ HistoricalProtectedCandidateOwned(candidate))
                     ~> ~HistoricalProtectedCandidateOwned(candidate)
              BY <2>2, <3>2, <5>1
                 DEF HistoricalProtectedCandidateStarvationProperty,
                     HistoricalProtectedServiceOwnershipExit
            <6>2. [][(gst
                       /\ HistoricalCommitDecisionExactCarrierOwned(
                            candidate, qc, "PersistDecision")
                       /\ ~NodeHasDecision(candidate.node)
                      => (gst
                           /\ HistoricalCommitDecisionExactCarrierOwned(
                                candidate, qc, "PersistDecision"))'
                           \/ NodeHasDecision(candidate.node)')]_AsyncAllVars
              BY <4>1, <4>2, <4>3, <4>4, <4>5,
                 HistoricalPersistDecisionOwnerPersistsOrWritesDecision,
                 PTL
            <6> QED BY <6>1, <6>2, PTL
                 DEF HistoricalCommitDecisionExactCarrierOwned
          <5> QED BY <5>1
        <4>9. \A node \in Responsive:
                 (gst
                   /\ HistoricalCommitDecisionCandidateOwned(
                        node, "DeliverQC"))
                   ~> (NodeHasDecision(node)
                        \/ HistoricalCommitDecisionCandidateOwned(
                             node, "BeginDecision"))
          BY <4>6, HistoricalCommitDecisionOwnerHasExactCarrier, PTL
             DEF HistoricalCommitDecisionExactLineageOwned,
                 HistoricalCommitDecisionExactCarrierOwned
        <4>10. \A node \in Responsive:
                  (gst
                    /\ HistoricalCommitDecisionCandidateOwned(
                         node, "BeginDecision"))
                    ~> (NodeHasDecision(node)
                         \/ HistoricalCommitDecisionCandidateOwned(
                              node, "PersistDecision"))
          BY <4>7, HistoricalCommitDecisionOwnerHasExactCarrier, PTL
             DEF HistoricalCommitDecisionExactLineageOwned,
                 HistoricalCommitDecisionExactCarrierOwned
        <4>11. \A node \in Responsive:
                  (gst
                    /\ HistoricalCommitDecisionCandidateOwned(
                         node, "PersistDecision"))
                    ~> NodeHasDecision(node)
          BY <4>8, HistoricalCommitDecisionOwnerHasExactCarrier, PTL
        <4> QED BY <2>2, <3>2, <4>9, <4>10, <4>11
             DEF HistoricalCommitDeliveryProgressLeaf,
                 HistoricalBeginDecisionProgressLeaf,
                 HistoricalPersistDecisionProgressLeaf
      <3> QED BY <3>1, <3>2, PTL
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

(***************************************************************************
Exact historical Decision-body handoffs.

The six body leaves cannot be obtained from candidate starvation alone:
departure of an existential owner could otherwise be witnessed by an
unrelated candidate at the same node.  Freeze the historical node, persisted
Decision QC, route-neutral candidate evidence, and first-admission causal
origin.  Every
non-consuming transition retains that exact carrier; consuming the carrier
either installs the exact next lineage owner, opens the exact certified-body
request, or records the Application.  A tombstone is lifecycle evidence, not
a successful handoff.
***************************************************************************)

HistoricalDecisionPipelineExactCarrierOwned(
    node, qc, evidence, origin, kind, candidate) ==
  /\ node \in Responsive
  /\ HistoricalRecoveryTarget(node)
  /\ \E decision \in decisions:
       /\ HistoricalDecisionRecordMatches(node, decision)
       /\ decision.qc = qc
  /\ kind \in DecisionPipelineKinds
  /\ candidate \in AsyncCandidateSet
  /\ candidate.kind = kind
  /\ AsyncRouteNeutralCandidateEvidence(candidate.evidence) = evidence
  /\ candidate.causalOrigin = origin
  /\ DecisionPipelineCandidate(node, qc, candidate)
  /\ HistoricalProtectedCandidateOwned(candidate)

HistoricalDecisionPipelineExactLineageOwned(
    node, qc, evidence, origin, kind) ==
  \E candidate \in AsyncCandidateSet:
    HistoricalDecisionPipelineExactCarrierOwned(
      node, qc, evidence, origin, kind, candidate)

HistoricalDecisionPipelineExactStageOutcome(
    node, qc, evidence, origin, kind) ==
  \/ NodeHasApplication(node)
  \/ CASE kind = "FetchBody" ->
            \/ DecisionCertifiedRequestActive(node, qc)
            \/ HistoricalDecisionPipelineExactLineageOwned(
                 node, qc, evidence, origin, "ValidateBody")
       [] kind = "RequestCertifiedBody" ->
            DecisionCertifiedRequestActive(node, qc)
       [] kind = "FetchCertifiedBody" ->
            HistoricalDecisionPipelineExactLineageOwned(
              node, qc, evidence, origin, "StoreBody")
       [] kind = "StoreBody" ->
            HistoricalDecisionPipelineExactLineageOwned(
              node, qc, evidence, origin, "ValidateBody")
       [] kind = "ValidateBody" ->
            HistoricalDecisionPipelineExactLineageOwned(
              node, qc, evidence, origin, "Apply")
       [] kind = "Apply" -> NodeHasApplication(node)
       [] OTHER -> FALSE

THEOREM HistoricalDecisionPipelineOwnerHasExactCarrier ==
  \A kind:
    \A node \in Responsive:
      HistoricalDecisionPipelineKindOwned(node, kind)
        <=> \E decision \in decisions,
                candidate \in AsyncCandidateSet:
              /\ HistoricalDecisionRecordMatches(node, decision)
              /\ HistoricalDecisionPipelineExactCarrierOwned(
                   node, decision.qc,
                   AsyncRouteNeutralCandidateEvidence(candidate.evidence),
                   candidate.causalOrigin, kind, candidate)
BY Isa
   DEF HistoricalDecisionPipelineKindOwned,
       HistoricalDecisionPipelineExactCarrierOwned,
       HistoricalDecisionRecordMatches,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       DecisionPipelineKindOwned, DecisionPipelineCandidate

(***************************************************************************
One bracket step cannot detach a live exact body carrier from its frozen
Decision lineage.  The proof expands all six production reducer actions and
the exact causal-successor constructor.  The named corollaries below expose
each action handoff separately for checker and mutation coverage.
***************************************************************************)
THEOREM HistoricalDecisionPipelineExactCarrierPersistsOrHandsOff ==
  \A node \in Responsive, qc, evidence, origin,
     kind \in DecisionPipelineKinds,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ HistoricalDecisionPipelineExactCarrierOwned(
         node, qc, evidence, origin, kind, candidate)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionPipelineExactCarrierOwned(
            node, qc, evidence, origin, kind, candidate)'
       \/ HistoricalDecisionPipelineExactStageOutcome(
            node, qc, evidence, origin, kind)'
BY CompletionDeferralRetainsCandidate,
   CoreBracketStepPreservesNodeApplication,
   CommandSuccessorsRetainCausalOrigin,
   IsaT(1200)
   DEF HistoricalDecisionPipelineExactCarrierOwned,
       HistoricalDecisionPipelineExactLineageOwned,
       HistoricalDecisionPipelineExactStageOutcome,
       HistoricalDecisionRecordMatches,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       DecisionPipelineKinds, DecisionPipelineKindOwned,
       DecisionPipelineCandidate, CandidateConsumerCurrent,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       DecisionCertifiedRequestActive, DecisionRecoveryCertificate,
       DecisionTimeoutFrontierInvariant,
       DecisionFrontierUniquenessInvariant,
       PostGstReplayQuarantineExcluded,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncEnterIndexedServiceActivation,
       AsyncActivateServiceNode,
       AsyncServiceActivationTransition,
       AsyncServiceActivationFrameVars,
       AsyncSchedulerExceptServiceActivation,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep,
       SerializedRunnerRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       LocalAdmissionStep, IngressDrainStep, SerializedRuntimeStep,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       ExecuteDecisionFetch, ExecuteRequestCertifiedBody, ExecuteApply,
       FetchCertifiedBody, StoreBody, ValidateDecidedBody, ApplyDecision,
       PublishCertifiedRequests, CertifiedRequestOutbox,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CommandSuccessors,
       CausalCandidate, AsyncCandidateFrom,
       AsyncCandidateWithIdentityAndOrigin,
       AsyncAllVars

THEOREM HistoricalDecisionFetchBodyOwnerPersistsOrHandsOff ==
  \A node \in Responsive, qc, evidence, origin,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ HistoricalDecisionPipelineExactCarrierOwned(
         node, qc, evidence, origin, "FetchBody", candidate)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionPipelineExactCarrierOwned(
            node, qc, evidence, origin, "FetchBody", candidate)'
       \/ NodeHasApplication(node)'
       \/ DecisionCertifiedRequestActive(node, qc)'
       \/ HistoricalDecisionPipelineExactLineageOwned(
            node, qc, evidence, origin, "ValidateBody")'
BY HistoricalDecisionPipelineExactCarrierPersistsOrHandsOff
   DEF HistoricalDecisionPipelineExactStageOutcome,
       DecisionPipelineKinds

THEOREM HistoricalDecisionRequestBodyOwnerPersistsOrRequests ==
  \A node \in Responsive, qc, evidence, origin,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ HistoricalDecisionPipelineExactCarrierOwned(
         node, qc, evidence, origin, "RequestCertifiedBody", candidate)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionPipelineExactCarrierOwned(
            node, qc, evidence, origin,
            "RequestCertifiedBody", candidate)'
       \/ NodeHasApplication(node)'
       \/ DecisionCertifiedRequestActive(node, qc)'
BY HistoricalDecisionPipelineExactCarrierPersistsOrHandsOff
   DEF HistoricalDecisionPipelineExactStageOutcome,
       DecisionPipelineKinds

THEOREM HistoricalDecisionFetchCertifiedOwnerPersistsOrStores ==
  \A node \in Responsive, qc, evidence, origin,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ HistoricalDecisionPipelineExactCarrierOwned(
         node, qc, evidence, origin, "FetchCertifiedBody", candidate)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionPipelineExactCarrierOwned(
            node, qc, evidence, origin,
            "FetchCertifiedBody", candidate)'
       \/ NodeHasApplication(node)'
       \/ HistoricalDecisionPipelineExactLineageOwned(
            node, qc, evidence, origin, "StoreBody")'
BY HistoricalDecisionPipelineExactCarrierPersistsOrHandsOff
   DEF HistoricalDecisionPipelineExactStageOutcome,
       DecisionPipelineKinds

THEOREM HistoricalDecisionStoreBodyOwnerPersistsOrValidates ==
  \A node \in Responsive, qc, evidence, origin,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ HistoricalDecisionPipelineExactCarrierOwned(
         node, qc, evidence, origin, "StoreBody", candidate)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionPipelineExactCarrierOwned(
            node, qc, evidence, origin, "StoreBody", candidate)'
       \/ NodeHasApplication(node)'
       \/ HistoricalDecisionPipelineExactLineageOwned(
            node, qc, evidence, origin, "ValidateBody")'
BY HistoricalDecisionPipelineExactCarrierPersistsOrHandsOff
   DEF HistoricalDecisionPipelineExactStageOutcome,
       DecisionPipelineKinds

THEOREM HistoricalDecisionValidateBodyOwnerPersistsOrApplies ==
  \A node \in Responsive, qc, evidence, origin,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ HistoricalDecisionPipelineExactCarrierOwned(
         node, qc, evidence, origin, "ValidateBody", candidate)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionPipelineExactCarrierOwned(
            node, qc, evidence, origin, "ValidateBody", candidate)'
       \/ NodeHasApplication(node)'
       \/ HistoricalDecisionPipelineExactLineageOwned(
            node, qc, evidence, origin, "Apply")'
BY HistoricalDecisionPipelineExactCarrierPersistsOrHandsOff
   DEF HistoricalDecisionPipelineExactStageOutcome,
       DecisionPipelineKinds

THEOREM HistoricalDecisionApplyOwnerPersistsOrWritesApplication ==
  \A node \in Responsive, qc, evidence, origin,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ HistoricalDecisionPipelineExactCarrierOwned(
         node, qc, evidence, origin, "Apply", candidate)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionPipelineExactCarrierOwned(
            node, qc, evidence, origin, "Apply", candidate)'
       \/ NodeHasApplication(node)'
BY HistoricalDecisionPipelineExactCarrierPersistsOrHandsOff
   DEF HistoricalDecisionPipelineExactStageOutcome,
       DecisionPipelineKinds

THEOREM HistoricalDecisionPipelinePerActionSafetyCoversEveryKind ==
  \A node \in Responsive, qc, evidence, origin,
     kind \in DecisionPipelineKinds,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncCandidateServiceTombstoneLifecycleInvariant
    /\ gst
    /\ HistoricalDecisionPipelineExactCarrierOwned(
         node, qc, evidence, origin, kind, candidate)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalDecisionPipelineExactCarrierOwned(
            node, qc, evidence, origin, kind, candidate)'
       \/ HistoricalDecisionPipelineExactStageOutcome(
            node, qc, evidence, origin, kind)'
BY HistoricalDecisionFetchBodyOwnerPersistsOrHandsOff,
   HistoricalDecisionRequestBodyOwnerPersistsOrRequests,
   HistoricalDecisionFetchCertifiedOwnerPersistsOrStores,
   HistoricalDecisionStoreBodyOwnerPersistsOrValidates,
   HistoricalDecisionValidateBodyOwnerPersistsOrApplies,
   HistoricalDecisionApplyOwnerPersistsOrWritesApplication,
   Isa
   DEF HistoricalDecisionPipelineExactStageOutcome,
       DecisionPipelineKinds

HistoricalDecisionPipelineTemporalSupportProperty(specification) ==
  /\ (specification => []AsyncStrongTypeInvariant)
  /\ (specification => []AsyncProgressOwnershipInvariant)
  /\ (specification => []DecisionTimeoutFrontierInvariant)
  /\ (specification => []DecisionFrontierUniquenessInvariant)
  /\ (specification => []PostGstReplayQuarantineExcluded)
  /\ (specification =>
        []AsyncCandidateServiceTombstoneLifecycleInvariant)
  /\ (specification => [][AsyncNext]_AsyncAllVars)
  /\ (specification => [](gst => []gst))

HistoricalDecisionPipelineExactCarrierHandoffProperty(specification) ==
  specification
    => \A qc, evidence, origin:
         \A node \in Responsive,
            kind \in DecisionPipelineKinds,
            candidate \in AsyncCandidateSet:
           (gst
             /\ HistoricalDecisionPipelineExactCarrierOwned(
                  node, qc, evidence, origin, kind, candidate))
             ~> HistoricalDecisionPipelineExactStageOutcome(
                  node, qc, evidence, origin, kind)

THEOREM HistoricalDecisionPipelineExactCarrierReachesExactHandoff ==
  \A specification:
    /\ HistoricalDecisionPipelineTemporalSupportProperty(specification)
    /\ HistoricalProtectedCandidateStarvationProperty(specification)
    => HistoricalDecisionPipelineExactCarrierHandoffProperty(specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalDecisionPipelineTemporalSupportProperty(
                  specification),
                HistoricalProtectedCandidateStarvationProperty(
                  specification)
         PROVE HistoricalDecisionPipelineExactCarrierHandoffProperty(
                   specification)
    <2>1. CASE ~specification
      BY <2>1
         DEF HistoricalDecisionPipelineExactCarrierHandoffProperty
    <2>2. CASE specification
      <3>1. []AsyncStrongTypeInvariant
        BY <1>1, <2>2
           DEF HistoricalDecisionPipelineTemporalSupportProperty
      <3>2. []AsyncProgressOwnershipInvariant
        BY <1>1, <2>2
           DEF HistoricalDecisionPipelineTemporalSupportProperty
      <3>3. []DecisionTimeoutFrontierInvariant
        BY <1>1, <2>2
           DEF HistoricalDecisionPipelineTemporalSupportProperty
      <3>4. []DecisionFrontierUniquenessInvariant
        BY <1>1, <2>2
           DEF HistoricalDecisionPipelineTemporalSupportProperty
      <3>5. []PostGstReplayQuarantineExcluded
        BY <1>1, <2>2
           DEF HistoricalDecisionPipelineTemporalSupportProperty
      <3>6. []AsyncCandidateServiceTombstoneLifecycleInvariant
        BY <1>1, <2>2
           DEF HistoricalDecisionPipelineTemporalSupportProperty
      <3>7. [][AsyncNext]_AsyncAllVars
        BY <1>1, <2>2
           DEF HistoricalDecisionPipelineTemporalSupportProperty
      <3>8. [](gst => []gst)
        BY <1>1, <2>2
           DEF HistoricalDecisionPipelineTemporalSupportProperty
      <3>9. \A qc, evidence, origin:
              \A node \in Responsive,
                 kind \in DecisionPipelineKinds,
                 candidate \in AsyncCandidateSet:
                (gst
                  /\ HistoricalDecisionPipelineExactCarrierOwned(
                       node, qc, evidence, origin, kind, candidate))
                  ~> HistoricalDecisionPipelineExactStageOutcome(
                       node, qc, evidence, origin, kind)
        <4>1. ASSUME NEW node \in Responsive,
                      NEW kind \in DecisionPipelineKinds,
                      NEW candidate \in AsyncCandidateSet,
                      NEW qc, NEW evidence, NEW origin
               PROVE
                 (gst
                   /\ HistoricalDecisionPipelineExactCarrierOwned(
                        node, qc, evidence, origin, kind, candidate))
                   ~> HistoricalDecisionPipelineExactStageOutcome(
                        node, qc, evidence, origin, kind)
          <5>1. (gst /\ HistoricalProtectedCandidateOwned(candidate))
                   ~> ~HistoricalProtectedCandidateOwned(candidate)
            BY <1>1, <2>2, <4>1
               DEF HistoricalProtectedCandidateStarvationProperty,
                   HistoricalProtectedServiceOwnershipExit
          <5>2. [][(gst
                     /\ HistoricalDecisionPipelineExactCarrierOwned(
                          node, qc, evidence, origin, kind, candidate)
                     /\ ~HistoricalDecisionPipelineExactStageOutcome(
                          node, qc, evidence, origin, kind)
                    => (gst
                         /\ HistoricalDecisionPipelineExactCarrierOwned(
                              node, qc, evidence, origin,
                              kind, candidate))'
                         \/ HistoricalDecisionPipelineExactStageOutcome(
                              node, qc, evidence, origin, kind)')]_AsyncAllVars
            BY <3>1, <3>2, <3>3, <3>4, <3>5, <3>6, <3>7, <3>8,
               HistoricalDecisionPipelinePerActionSafetyCoversEveryKind,
               PTL
          <5> QED BY <5>1, <5>2, PTL
               DEF HistoricalDecisionPipelineExactCarrierOwned
        <4> QED BY <4>1
      <3> QED BY <2>2, <3>9
           DEF HistoricalDecisionPipelineExactCarrierHandoffProperty
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM HistoricalDecisionPipelineExactCarrierClosesBodyLeaves ==
  \A specification:
    HistoricalDecisionPipelineTemporalSupportProperty(specification)
      => /\ HistoricalDecisionFetchProgressLeaf(specification)
         /\ HistoricalDecisionRequestBodyProgressLeaf(specification)
         /\ HistoricalDecisionFetchCertifiedProgressLeaf(specification)
         /\ HistoricalDecisionStoreProgressLeaf(specification)
         /\ HistoricalDecisionValidateProgressLeaf(specification)
         /\ HistoricalDecisionApplyProgressLeaf(specification)
PROOF
  <1>1. ASSUME NEW specification,
                HistoricalDecisionPipelineTemporalSupportProperty(
                  specification)
         PROVE /\ HistoricalDecisionFetchProgressLeaf(specification)
               /\ HistoricalDecisionRequestBodyProgressLeaf(specification)
               /\ HistoricalDecisionFetchCertifiedProgressLeaf(
                    specification)
               /\ HistoricalDecisionStoreProgressLeaf(specification)
               /\ HistoricalDecisionValidateProgressLeaf(specification)
               /\ HistoricalDecisionApplyProgressLeaf(specification)
    <2>1. ASSUME ~HistoricalProtectedCandidateStarvationProperty(
                     specification)
           PROVE /\ HistoricalDecisionFetchProgressLeaf(specification)
                 /\ HistoricalDecisionRequestBodyProgressLeaf(specification)
                 /\ HistoricalDecisionFetchCertifiedProgressLeaf(
                      specification)
                 /\ HistoricalDecisionStoreProgressLeaf(specification)
                 /\ HistoricalDecisionValidateProgressLeaf(specification)
                 /\ HistoricalDecisionApplyProgressLeaf(specification)
      BY <2>1
         DEF HistoricalDecisionFetchProgressLeaf,
             HistoricalDecisionRequestBodyProgressLeaf,
             HistoricalDecisionFetchCertifiedProgressLeaf,
             HistoricalDecisionStoreProgressLeaf,
             HistoricalDecisionValidateProgressLeaf,
             HistoricalDecisionApplyProgressLeaf
    <2>2. ASSUME HistoricalProtectedCandidateStarvationProperty(
                    specification)
           PROVE /\ HistoricalDecisionFetchProgressLeaf(specification)
                 /\ HistoricalDecisionRequestBodyProgressLeaf(specification)
                 /\ HistoricalDecisionFetchCertifiedProgressLeaf(
                      specification)
                 /\ HistoricalDecisionStoreProgressLeaf(specification)
                 /\ HistoricalDecisionValidateProgressLeaf(specification)
                 /\ HistoricalDecisionApplyProgressLeaf(specification)
      <3>1. HistoricalDecisionPipelineExactCarrierHandoffProperty(
               specification)
        BY <1>1, <2>2,
           HistoricalDecisionPipelineExactCarrierReachesExactHandoff
      <3>2. ASSUME ~specification
             PROVE /\ HistoricalDecisionFetchProgressLeaf(specification)
                   /\ HistoricalDecisionRequestBodyProgressLeaf(specification)
                   /\ HistoricalDecisionFetchCertifiedProgressLeaf(
                        specification)
                   /\ HistoricalDecisionStoreProgressLeaf(specification)
                   /\ HistoricalDecisionValidateProgressLeaf(specification)
                   /\ HistoricalDecisionApplyProgressLeaf(specification)
        BY <3>2
           DEF HistoricalDecisionFetchProgressLeaf,
               HistoricalDecisionRequestBodyProgressLeaf,
               HistoricalDecisionFetchCertifiedProgressLeaf,
               HistoricalDecisionStoreProgressLeaf,
               HistoricalDecisionValidateProgressLeaf,
               HistoricalDecisionApplyProgressLeaf
      <3>3. ASSUME specification
             PROVE /\ HistoricalDecisionFetchProgressLeaf(specification)
                   /\ HistoricalDecisionRequestBodyProgressLeaf(specification)
                   /\ HistoricalDecisionFetchCertifiedProgressLeaf(
                        specification)
                   /\ HistoricalDecisionStoreProgressLeaf(specification)
                   /\ HistoricalDecisionValidateProgressLeaf(specification)
                   /\ HistoricalDecisionApplyProgressLeaf(specification)
        <4>1. \A node \in Responsive:
                 (gst
                   /\ HistoricalDecisionPipelineKindOwned(
                        node, "FetchBody"))
                   ~> (NodeHasApplication(node)
                        \/ HistoricalDecisionPipelineKindOwned(
                             node, "RequestCertifiedBody")
                        \/ HistoricalDecisionCertifiedRequestActive(node)
                        \/ HistoricalDecisionPipelineKindOwned(
                             node, "ValidateBody"))
          BY <3>1, <3>3,
             HistoricalDecisionPipelineOwnerHasExactCarrier, PTL
             DEF HistoricalDecisionPipelineExactCarrierHandoffProperty,
                 HistoricalDecisionPipelineExactStageOutcome,
                 HistoricalDecisionPipelineExactLineageOwned,
                 HistoricalDecisionPipelineExactCarrierOwned,
                 HistoricalDecisionPipelineKindOwned,
                 HistoricalDecisionCertifiedRequestActive,
                 DecisionPipelineKindOwned, DecisionPipelineKinds
        <4>2. \A node \in Responsive:
                 (gst
                   /\ HistoricalDecisionPipelineKindOwned(
                        node, "RequestCertifiedBody"))
                   ~> (NodeHasApplication(node)
                        \/ HistoricalDecisionCertifiedRequestActive(node))
          BY <3>1, <3>3,
             HistoricalDecisionPipelineOwnerHasExactCarrier, PTL
             DEF HistoricalDecisionPipelineExactCarrierHandoffProperty,
                 HistoricalDecisionPipelineExactStageOutcome,
                 HistoricalDecisionPipelineExactLineageOwned,
                 HistoricalDecisionPipelineExactCarrierOwned,
                 HistoricalDecisionCertifiedRequestActive,
                 DecisionPipelineKinds
        <4>3. \A node \in Responsive:
                 (gst
                   /\ HistoricalDecisionPipelineKindOwned(
                        node, "FetchCertifiedBody"))
                   ~> (NodeHasApplication(node)
                        \/ HistoricalDecisionPipelineKindOwned(
                             node, "StoreBody"))
          BY <3>1, <3>3,
             HistoricalDecisionPipelineOwnerHasExactCarrier, PTL
             DEF HistoricalDecisionPipelineExactCarrierHandoffProperty,
                 HistoricalDecisionPipelineExactStageOutcome,
                 HistoricalDecisionPipelineExactLineageOwned,
                 HistoricalDecisionPipelineExactCarrierOwned,
                 HistoricalDecisionPipelineKindOwned,
                 DecisionPipelineKindOwned, DecisionPipelineKinds
        <4>4. \A node \in Responsive:
                 (gst
                   /\ HistoricalDecisionPipelineKindOwned(
                        node, "StoreBody"))
                   ~> (NodeHasApplication(node)
                        \/ HistoricalDecisionPipelineKindOwned(
                             node, "ValidateBody"))
          BY <3>1, <3>3,
             HistoricalDecisionPipelineOwnerHasExactCarrier, PTL
             DEF HistoricalDecisionPipelineExactCarrierHandoffProperty,
                 HistoricalDecisionPipelineExactStageOutcome,
                 HistoricalDecisionPipelineExactLineageOwned,
                 HistoricalDecisionPipelineExactCarrierOwned,
                 HistoricalDecisionPipelineKindOwned,
                 DecisionPipelineKindOwned, DecisionPipelineKinds
        <4>5. \A node \in Responsive:
                 (gst
                   /\ HistoricalDecisionPipelineKindOwned(
                        node, "ValidateBody"))
                   ~> (NodeHasApplication(node)
                        \/ HistoricalDecisionPipelineKindOwned(
                             node, "Apply"))
          BY <3>1, <3>3,
             HistoricalDecisionPipelineOwnerHasExactCarrier, PTL
             DEF HistoricalDecisionPipelineExactCarrierHandoffProperty,
                 HistoricalDecisionPipelineExactStageOutcome,
                 HistoricalDecisionPipelineExactLineageOwned,
                 HistoricalDecisionPipelineExactCarrierOwned,
                 HistoricalDecisionPipelineKindOwned,
                 DecisionPipelineKindOwned, DecisionPipelineKinds
        <4>6. \A node \in Responsive:
                 (gst
                   /\ HistoricalDecisionPipelineKindOwned(node, "Apply"))
                   ~> NodeHasApplication(node)
          BY <3>1, <3>3,
             HistoricalDecisionPipelineOwnerHasExactCarrier, PTL
             DEF HistoricalDecisionPipelineExactCarrierHandoffProperty,
                 HistoricalDecisionPipelineExactStageOutcome,
                 HistoricalDecisionPipelineExactLineageOwned,
                 HistoricalDecisionPipelineExactCarrierOwned,
                 DecisionPipelineKinds
        <4> QED BY <2>2, <3>3, <4>1, <4>2, <4>3, <4>4, <4>5, <4>6
             DEF HistoricalDecisionFetchProgressLeaf,
                 HistoricalDecisionRequestBodyProgressLeaf,
                 HistoricalDecisionFetchCertifiedProgressLeaf,
                 HistoricalDecisionStoreProgressLeaf,
                 HistoricalDecisionValidateProgressLeaf,
                 HistoricalDecisionApplyProgressLeaf
      <3> QED BY <3>2, <3>3, PTL
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

(***************************************************************************
Exact residual inventory.

This generic one-height module deliberately declares the exact transport
interfaces without importing product fairness.  The direct indexed product
module `SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs` now
instantiates every item below from fixed packet-action, historical-target
runner, historical-server runner, and ordinary archive-I/O fairness.

  1. `HistoricalCommitArchiveRouteAvailabilityProperty` is a chain/Async
     refinement seam.  `HistoricalRecoverySourceReady` supplies an applied
     responsive archive, while `CommitCertificateRequestOutbox` supplies
     current-voter routes; the one-height model has no theorem proving that
     the two sets intersect.  The existing indexed durable source record does
     supply a current-voter applied server.  A child of the indexed chain
     refinement must instantiate this module, prove that
     `IndexedOpenHistoricalRecovery` establishes
     `HistoricalCommitArchiveRouteAvailable`, and preserve that witness using
     monotone application, post-GST up-state, fixed context/roster, and
     historical-target exit.  This discharges the seam without changing
     `CommitCertificateRequestOutbox` or
     `CommitCertificateRequestAuthorized`.  A standalone one-height repair
     would instead strengthen `HistoricalRecoverySourceReady`; broadening
     those two request-route definitions would be a larger protocol change
     and would also require updating every occurrence/completeness predicate
     here to quantify over the new route operator.

  2. Commit request emission still needs the discovery-clock theorem followed
     by the historical target's finite Runtime/retransmit prefix.

  3. Both request classes need finite delivery-deadline and older same-source
     packet descent, exact admission/coalescing, and normal-or-historical
     ingress-runner descent to the fresh Serve occurrence.

  4. The exact Serve occurrence uses an ordinary responsive archive I/O
     owner.  Its action handoff is proved above; temporal FIFO exit must be
     instantiated from the archive worker fairness/rank theorem.

  5. CommitCertificateResponse uses the aggregate untrusted outer lane and
     needs its finite packet/admission prefix before DeliverQC.

  6. CertifiedResponse additionally needs the route-neutral recipient claim,
     finite normalized physical-completion owner, and historical target
     ingress-runner prefix before FetchCertifiedBody.  The fresh/coalesced
     claim handoffs and exact drain effect are proved above.

  7. DeliverQC and FetchCertifiedBody scheduler-rank descent after these
     transport leaves belongs to the separate historical-target reducer
     theorem; it is not archive transport fairness.

The indexed instantiation retains exact context, request, certificate,
subject, recipient, source, and lifecycle identities.  These declarations
must not be replaced with aggregate target-to-Decision, all-responsive-joined,
or application-liveness premises.
***************************************************************************)

HistoricalCommitTransportResidualKernels(specification) ==
  /\ HistoricalCommitArchiveRouteAvailabilityProperty(specification)
  /\ HistoricalCommitPhysicalTransportKernelProperties(specification)

HistoricalDecisionCertifiedTransportResidualKernels(specification) ==
  HistoricalDecisionCertifiedTransportKernelProperties(specification)

HistoricalRecoveryTransportResidualKernels(specification) ==
  /\ HistoricalCommitTransportResidualKernels(specification)
  /\ HistoricalDecisionCertifiedTransportResidualKernels(specification)

=============================================================================
