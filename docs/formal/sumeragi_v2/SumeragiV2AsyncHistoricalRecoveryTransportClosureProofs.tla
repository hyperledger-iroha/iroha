---- MODULE SumeragiV2AsyncHistoricalRecoveryTransportClosureProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryLivenessProofs,
        SumeragiV2ExactDecisionStageServiceClosureProofs

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
  * a concrete per-source ingress occurrence;
  * a concrete fresh-nonce Serve FIFO occurrence;
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
       RunHistoricalServer,
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
       SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
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

The Serve record itself is the occurrence identity, including its nonce.
`AsyncIoServeNonceOwnership`, maintained by the strong type invariant,
ensures that the appended in-range nonce is fresh in that server FIFO.
***************************************************************************)

HistoricalCommitRequestPacketOwned(target, server, request, packet) ==
  /\ HistoricalCommitRequestRegistered(target, server, request)
  /\ packet \in asyncTransport
  /\ packet.item = request

HistoricalCommitRequestIngressOwned(target, server, request) ==
  /\ HistoricalCommitRequestRegistered(target, server, request)
  /\ request \in
       SequenceSet(
         IngressLane(server, IngressResourceSource(request)))

HistoricalCommitServeOccurrenceOwned(server, request, job) ==
  /\ job \in SequenceSet(asyncIoQueues[server])
  /\ job.class = "Serve"
  /\ job.candidate.item = request
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
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       CommitCertificateRequestAuthorized,
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

THEOREM HistoricalCommitIngressCreatesFreshServeOwner ==
  \A target, server, request:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCommitArchiveRouteAvailable(target, server)
    /\ HistoricalCommitRequestIngressOwned(target, server, request)
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
       DrainHistoricalIngressSelected,
       AsyncIoCertifiedServeJob,
       CommitCertificateRequestAuthorized,
       FreshAsyncIoServeNonce, SequenceSet,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncTransportContentTypeInvariant,
       AsyncTransportHistoryTypeInvariant,
       AsyncActiveRequestsType

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
       \/ HistoricalCommitTransportGoal(target)'
BY HistoricalCommitIngressCreatesFreshServeOwner,
   HistoricalRecoveryTargetPersistsUnlessDecision,
   GstAsyncStepIsMonotone, IsaT(600)
   DEF HistoricalCommitRequestIngressOwned,
       HistoricalCommitRequestRegistered,
       HistoricalCommitRequestOccurrence,
       HistoricalCommitArchiveRouteAvailable,
       HistoricalCommitServeJobOwned,
       HistoricalCommitServeOccurrenceOwned,
       HistoricalCommitTransportGoal,
       HistoricalCommitDeliverQcOwner,
       HistoricalCommitResponseLineage,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
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
       DrainFairIngressSelected,
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

HistoricalCommitResponsePacketGoal(target, server, request) ==
  \/ HistoricalCommitTransportGoal(target)
  \/ \E qc, response, packet:
       HistoricalCommitResponsePacketOwned(
         target, server, request, qc, response, packet)

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
Responsive historical-QC signer/body-source lemma.

The requester is only required to be a typed validator.  It need not belong
to `AsyncCurrentResponsiveVoters`.  Quorum intersection is between the
CommitQC signer set and the fixed responsive voting quorum; the requester's
missing local body then proves that the selected body-holding signer is a
different node.
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
member of the frozen responsive voter set.  The serving archive selected
below is a responsive current voter and therefore uses the ordinary archive
I/O worker.  The requester consumes the response through its historical
runner.
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

HistoricalDecisionRequestIngressOwned(
    node, qc, archive, request) ==
  /\ HistoricalDecisionBodyHoldingAlias(
       node, qc, archive, request)
  /\ request \in
       SequenceSet(
         IngressLane(archive, IngressResourceSource(request)))

HistoricalDecisionServeOccurrenceOwned(archive, request, job) ==
  /\ job \in SequenceSet(asyncIoQueues[archive])
  /\ job.class = "Serve"
  /\ job.candidate.item = request
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
       HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
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
       HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       DrainHistoricalIngressSelected,
       AsyncIoCertifiedServeJob,
       CertifiedRequestAuthorized, CertifiedRequestAuthority,
       FreshAsyncIoServeNonce, SequenceSet

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
       HistoricalDecisionAuthenticatedResponse,
       HistoricalDecisionResponsePacketOwned,
       HistoricalDecisionBodyHoldingAlias,
       HistoricalExactDecisionActiveRequestOwner,
       HistoricalExactDecisionServiceSource,
       ServiceIoWorkerWork, CertifiedServeCanRespond,
       CertifiedResponseItem, PublishEphemeralItems,
       PacketsForItems, DecisionCertifiedResponseLineageExact

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

HistoricalDecisionResponsePacketGoal(node, qc, archive, request) ==
  \/ HistoricalDecisionCertifiedResponseGoal(node, qc)
  \/ \E response, packet:
       HistoricalDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)

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
Exact residual inventory.

No theorem in this module asserts any operator in the inventory below.

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

TODO: instantiate these exact residual kernels from the direct indexed
packet, historical-target runner, historical-server runner, and ordinary
archive-I/O fairness clauses.  Do not replace them with aggregate
target-to-Decision, all-responsive-joined, or application-liveness premises.
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
